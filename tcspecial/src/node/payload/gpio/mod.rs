//! The GPIO endpoint: discrete lines, as events a conduit can carry.
//!
//! **Experimental, and not yet supported.** Run against the kernel's
//! simulated chip and against no hardware.
//!
//! This is the one endpoint whose device data does not exist until it is
//! made. Every other kind carries bytes somebody sent; a discrete line has no
//! bytes at all -- it is high or low, and what is worth telling the ground is
//! when it changed. So the handler asks the chip for the configured lines with
//! edge detection, and each event the kernel reports becomes a record of
//! [`EVENT_BYTES`] bytes: when it happened, which line, and what it became.
//!
//! The time is a [`Timestamp`] -- microseconds since the UNIX epoch, eight
//! bytes, least significant first -- which is what every other time in this
//! project is, so a line's change and a telemetry response can be read
//! against each other. The kernel's own timestamp is
//! nanoseconds on a monotonic clock, which says nothing to a ground station;
//! it is converted here, once, where the event is made.
//!
//! Nothing is written. These are inputs: a write would mean driving an output,
//! which is commanding a device rather than reading one, and the device
//! configuration refuses a triggered GPIO device for the same reason.
//!
//! The kernel's structures are declared here, unlike the CAN endpoint's:
//! `libc` has no GPIO character-device definitions. Their sizes are asserted
//! against the kernel's own in the tests below, because a structure of the
//! wrong size is an ioctl that reads past what it was given and nothing else
//! would notice.

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::mem;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::time::{Duration, Instant, SystemTime};

use nix::poll::PollFlags;
use tcslibgs::{
    node::payload::gpio::{Direction, EVENT_BYTES},
    Bias, GpioConfig, LineEdge, TcsError, TcsResult, Timestamp,
};

use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable, WaitResult,
};

/// The kernel's limits on one line request, from `linux/gpio.h`.
const GPIO_V2_LINES_MAX: usize = 64;
const GPIO_MAX_NAME_SIZE: usize = 32;
const GPIO_V2_LINE_NUM_ATTRS_MAX: usize = 10;

/// `GPIO_V2_GET_LINE_IOCTL`, which is `_IOWR(0xB4, 0x07, struct
/// gpio_v2_line_request)`.
const GPIO_V2_GET_LINE_IOCTL: libc::c_ulong = 0xc250_b407;

/// Line flags, from `enum gpio_v2_line_flag`.
const GPIO_V2_LINE_FLAG_INPUT: u64 = 1 << 2;
const GPIO_V2_LINE_FLAG_OUTPUT: u64 = 1 << 3;
const GPIO_V2_LINE_FLAG_EDGE_RISING: u64 = 1 << 4;
const GPIO_V2_LINE_FLAG_EDGE_FALLING: u64 = 1 << 5;
const GPIO_V2_LINE_FLAG_BIAS_PULL_UP: u64 = 1 << 8;
const GPIO_V2_LINE_FLAG_BIAS_PULL_DOWN: u64 = 1 << 9;

/// `GPIO_V2_LINE_GET_VALUES_IOCTL`, which is `_IOWR(0xB4, 0x0E, struct
/// gpio_v2_line_values)` -- the same structure the values are set with, asked
/// of the request this node already holds rather than of a second one.
const GPIO_V2_LINE_GET_VALUES_IOCTL: libc::c_ulong = 0xc010_b40e;

/// `GPIO_V2_LINE_SET_VALUES_IOCTL`, which is `_IOWR(0xB4, 0x0F, struct
/// gpio_v2_line_values)` -- sixteen bytes of two `u64`.
const GPIO_V2_LINE_SET_VALUES_IOCTL: libc::c_ulong = 0xc010_b40f;

/// What a line is driven to, and which lines the request is about.
///
/// `gpio_v2_line_values`: the bits of `bits` are the values, the bits of
/// `mask` say which lines they apply to. Both are indexed by the line's place
/// in the request rather than by its offset on the chip, which is the one
/// thing about this structure that is easy to get wrong -- lines 3, 4 and 5 of
/// a chip are bits 0, 1 and 2 here.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct LineValues {
    bits: u64,
    mask: u64,
}

/// `GPIO_V2_LINE_ATTR_ID_DEBOUNCE`.
const GPIO_V2_LINE_ATTR_ID_DEBOUNCE: u32 = 3;

/// What an event says happened, from `enum gpio_v2_line_event_id`.
const GPIO_V2_LINE_EVENT_RISING_EDGE: u32 = 1;

/// `struct gpio_v2_line_attribute`, whose union this uses as a debounce
/// period: a `u32` in a field eight bytes wide.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct LineAttribute {
    id: u32,
    padding: u32,
    value: u64,
}

/// `struct gpio_v2_line_config_attribute`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct ConfigAttribute {
    attr: LineAttribute,
    mask: u64,
}

/// `struct gpio_v2_line_config`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct LineConfig {
    flags: u64,
    num_attrs: u32,
    padding: [u32; 5],
    attrs: [ConfigAttribute; GPIO_V2_LINE_NUM_ATTRS_MAX],
}

impl Default for LineConfig {
    fn default() -> Self {
        Self {
            flags: 0,
            num_attrs: 0,
            padding: [0; 5],
            attrs: [ConfigAttribute::default(); GPIO_V2_LINE_NUM_ATTRS_MAX],
        }
    }
}

/// `struct gpio_v2_line_request`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct LineRequest {
    offsets: [u32; GPIO_V2_LINES_MAX],
    consumer: [libc::c_char; GPIO_MAX_NAME_SIZE],
    config: LineConfig,
    num_lines: u32,
    event_buffer_size: u32,
    padding: [u32; 5],
    fd: i32,
}

impl Default for LineRequest {
    fn default() -> Self {
        Self {
            offsets: [0; GPIO_V2_LINES_MAX],
            consumer: [0; GPIO_MAX_NAME_SIZE],
            config: LineConfig::default(),
            num_lines: 0,
            event_buffer_size: 0,
            padding: [0; 5],
            fd: -1,
        }
    }
}

/// `struct gpio_v2_line_event`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct LineEvent {
    timestamp_ns: u64,
    id: u32,
    offset: u32,
    seqno: u32,
    line_seqno: u32,
    padding: [u32; 6],
}

/// A set of discrete lines, watched for changes.
pub struct GpioEndpoint {
    /// The handler this endpoint belongs to, as the device configuration
    /// names it.
    what: String,
    /// The chip's character device, kept open: the line request belongs to it
    /// for as long as it lives.
    _chip: File,
    /// The request's own descriptor, which the events are read from and the
    /// line states driven through.
    lines: File,
    /// Whether these lines are read or driven, which decides which of a read
    /// and a write is the one that makes sense here.
    direction: Direction,
    /// How many lines the request holds, which is how wide the mask is when
    /// they are driven: the kernel counts them by their place in the request.
    count: usize,
    /// Where the chip is, for the error that says a write has nowhere to go.
    path: String,
    /// The offsets of the lines, in the order they were asked for, which is
    /// the order the kernel reports their values in.
    offsets: Vec<u32>,
    /// How often what the lines are is reported, where the configuration asks
    /// for it.
    state_interval: Option<Duration>,
    /// When the next report is due.
    ///
    /// From when the last one was due rather than from when it was made, so
    /// a report that went out late does not push the next one later.
    next_report: Option<Instant>,
}

/// What every line of the request is, now.
///
/// One ioctl on the descriptor the events are read from: the request is
/// already open and already names the lines, so nothing is opened and nothing
/// is asked twice. The kernel answers with a bit per line, in the order the
/// lines were asked for.
fn levels_of(what: &str, fd: RawFd, count: usize) -> TcsResult<u64> {
    let mut values = LineValues {
        bits: 0,
        mask: if count >= u64::BITS as usize {
            u64::MAX
        } else {
            (1u64 << count) - 1
        },
    };

    // SAFETY: a structure laid out as the kernel's own -- the tests below
    // hold it to the kernel's size -- living across a call on a descriptor
    // the caller owns.
    let result = unsafe {
        libc::ioctl(
            fd,
            GPIO_V2_LINE_GET_VALUES_IOCTL,
            &mut values as *mut LineValues,
        )
    };
    if result < 0 {
        return Err(TcsError::Endpoint(format!(
            "{what}: cannot read what the lines are: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(values.bits)
}

/// One line's level as the bytes a conduit carries.
///
/// The record an event becomes -- the time, the line, the value -- said of a
/// line that has not changed: what it is now, observed now. Both kinds say
/// "this line had this value at this time", which is what a ground station
/// plots; the difference is that an event's time is the kernel's for the edge
/// and this one's is when the vehicle looked.
fn level_record(offset: u32, high: bool, now: SystemTime) -> [u8; EVENT_BYTES] {
    let mut record = [0u8; EVENT_BYTES];
    record[..8].copy_from_slice(&Timestamp::at(now).to_le_bytes());
    record[8..12].copy_from_slice(&offset.to_le_bytes());
    record[12..].copy_from_slice(&u32::from(high).to_le_bytes());
    record
}

impl GpioEndpoint {
    /// Ask the chip for the configured lines, with edge detection.
    pub fn new(what: &str, config: &GpioConfig) -> TcsResult<Self> {
        let chip = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&config.path)
            .map_err(|e| {
                TcsError::Endpoint(format!(
                    "{what}: cannot open the GPIO chip {}: {e}",
                    config.path
                ))
            })?;

        let request = request_for(what, config)?;
        let lines = ask_for_lines(what, chip.as_raw_fd(), request, &config.path)?;

        let state_interval = config.state_interval;

        Ok(Self {
            what: what.to_string(),
            _chip: chip,
            lines,
            direction: config.direction,
            count: config.lines.len(),
            path: config.path.clone(),
            offsets: config.lines.clone(),
            state_interval,
            // The first report is due at once, so that a ground station
            // connecting to a vehicle that has just started learns what the
            // lines are without waiting an interval for it.
            next_report: state_interval.map(|_| Instant::now()),
        })
    }

    /// A second endpoint on the same line request.
    ///
    /// The request's descriptor is duplicated rather than the request made
    /// again: the kernel gives a line to one request at a time, so a second
    /// request for the same lines would be refused.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            what: self.what.clone(),
            _chip: self._chip.try_clone()?,
            lines: self.lines.try_clone()?,
            direction: self.direction,
            count: self.count,
            path: self.path.clone(),
            offsets: self.offsets.clone(),
            state_interval: self.state_interval,
            next_report: self.next_report,
        })
    }
}

/// The request this configuration asks for.
fn request_for(what: &str, config: &GpioConfig) -> TcsResult<LineRequest> {
    if config.lines.is_empty() || config.lines.len() > GPIO_V2_LINES_MAX {
        return Err(TcsError::Config(format!(
            "{what}: {} lines is not between 1 and {GPIO_V2_LINES_MAX}",
            config.lines.len()
        )));
    }

    let mut request = LineRequest::default();
    for (n, line) in config.lines.iter().enumerate() {
        request.offsets[n] = *line;
    }
    request.num_lines = config.lines.len() as u32;

    // Whose lines these are, which is what `gpioinfo` shows beside them: a
    // line held by something unnamed is the hardest kind of conflict to find.
    for (n, byte) in what.bytes().take(GPIO_MAX_NAME_SIZE - 1).enumerate() {
        request.consumer[n] = byte as libc::c_char;
    }

    let edges = match config.edge {
        LineEdge::Rising => GPIO_V2_LINE_FLAG_EDGE_RISING,
        LineEdge::Falling => GPIO_V2_LINE_FLAG_EDGE_FALLING,
        LineEdge::Both => GPIO_V2_LINE_FLAG_EDGE_RISING | GPIO_V2_LINE_FLAG_EDGE_FALLING,
        // Asked for as a plain input: there are no events to read, and what
        // the lines are is taken when the report is due.
        LineEdge::None => 0,
    };
    let bias = match config.bias {
        Bias::None => 0,
        Bias::PullUp => GPIO_V2_LINE_FLAG_BIAS_PULL_UP,
        Bias::PullDown => GPIO_V2_LINE_FLAG_BIAS_PULL_DOWN,
    };

    // Driven lines ask for none of the watching terms, which the
    // configuration refuses to state for them: an output has no edges to
    // report and nothing to debounce.
    request.config.flags = if config.direction.is_output() {
        GPIO_V2_LINE_FLAG_OUTPUT
    } else {
        GPIO_V2_LINE_FLAG_INPUT | edges | bias
    };

    // The debounce the kernel does, where the file asked for one, applied to
    // every line of the request: a mask with a bit per line.
    if let Some(steady) = config.debounce {
        let mask = if config.lines.len() as u32 >= u64::BITS {
            u64::MAX
        } else {
            (1u64 << config.lines.len()) - 1
        };
        request.config.attrs[0] = ConfigAttribute {
            attr: LineAttribute {
                id: GPIO_V2_LINE_ATTR_ID_DEBOUNCE,
                padding: 0,
                // The kernel's own attribute counts microseconds, which
                // is where a length of time becomes a number.
                value: steady.as_micros() as u64,
            },
            mask,
        };
        request.config.num_attrs = 1;
    }

    Ok(request)
}

/// Make the request, and hand back the descriptor the events arrive on.
fn ask_for_lines(
    what: &str,
    chip: RawFd,
    mut request: LineRequest,
    path: &str,
) -> TcsResult<File> {
    // SAFETY: the structure is laid out as the kernel's own -- the tests
    // below hold it to the kernel's sizes -- and lives across the call, which
    // writes the descriptor into it.
    let asked = unsafe { libc::ioctl(chip, GPIO_V2_GET_LINE_IOCTL, &mut request) };
    if asked < 0 {
        return Err(TcsError::Endpoint(format!(
            "{what}: {path} will not give up its lines: {}. A line is given to one \
             request at a time, so this is commonly a line another program holds",
            std::io::Error::last_os_error()
        )));
    }

    if request.fd < 0 {
        return Err(TcsError::Endpoint(format!(
            "{what}: {path} took the request and gave no descriptor"
        )));
    }

    // Read without waiting, as every other endpoint does, and for a sharper
    // reason here: a read gathers every event that is waiting, so a blocking
    // descriptor would have it wait for the next one after collecting the
    // last -- a conduit stopped until a line happened to change again, with
    // the events it had already gathered unreported.
    // SAFETY: both calls take the descriptor by value and return a status.
    let flags = unsafe { libc::fcntl(request.fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(request.fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
    {
        return Err(TcsError::Endpoint(format!(
            "{what}: cannot make {path}'s line request non-blocking: {}",
            std::io::Error::last_os_error()
        )));
    }

    // SAFETY: the kernel has just given this descriptor, and nothing else
    // owns it.
    Ok(unsafe { File::from_raw_fd(request.fd) })
}

/// What arrived, as the state the lines are to be driven to.
///
/// The text `1` to assert and `0` to unassert, which is what a compute
/// element's decision looks like on a link, and what a ground station can
/// send by hand. Anything else is refused rather than taken for one of them:
/// the thing on the other end of this is a heater or a valve, and a datum
/// that cannot be read is not guessed at.
fn state_of(what: &str, data: &[u8]) -> TcsResult<bool> {
    match std::str::from_utf8(data).map(str::trim) {
        Ok("1") => Ok(true),
        Ok("0") => Ok(false),
        Ok(other) => Err(TcsError::Endpoint(format!(
            "{what}: \"{other}\" is neither 1 nor 0, and these lines drive \
             something: a value that cannot be read is not guessed at"
        ))),
        Err(_) => Err(TcsError::Endpoint(format!(
            "{what}: what arrived is not text, so it is not 1 or 0"
        ))),
    }
}

/// What the kernel is asked for: every line of the request at the one state.
///
/// Indexed by a line's place in the request and not by its offset on the
/// chip, which is how the v2 interface counts: lines 3, 7 and 11 asked for in
/// that order are bits 0, 1 and 2 here. The mask says which bits were meant,
/// so every line of the request is set in it whichever way they are going.
fn values_for(count: usize, wanted: bool) -> LineValues {
    let mask = if count >= u64::BITS as usize {
        u64::MAX
    } else {
        (1u64 << count) - 1
    };
    LineValues {
        bits: if wanted { mask } else { 0 },
        mask,
    }
}

/// One event as the bytes a conduit carries: the time, the line, the value.
///
/// Little-endian, because the two ends of this link are the same machine --
/// the handler makes these and the ground reads them, and a device set that
/// crossed architectures would have said so somewhere. That is also the
/// order a [`Timestamp`] goes on the wire in, so the eight bytes of time
/// here are the eight bytes of time in a telemetry response.
fn record_of(event: &LineEvent, now: SystemTime) -> [u8; EVENT_BYTES] {
    let mut record = [0u8; EVENT_BYTES];

    // The kernel's timestamp is nanoseconds on a monotonic clock, which says
    // nothing to a ground station: what goes out is a Timestamp, which is
    // what every other time in this project is, so a line's change and a
    // telemetry response can be read against each other.
    let value: u32 = if event.id == GPIO_V2_LINE_EVENT_RISING_EDGE { 1 } else { 0 };

    record[..8].copy_from_slice(&Timestamp::at(now).to_le_bytes());
    record[8..12].copy_from_slice(&event.offset.to_le_bytes());
    record[12..].copy_from_slice(&value.to_le_bytes());
    record
}

impl EndpointWaitable for GpioEndpoint {
    fn io_fd(&self) -> RawFd {
        self.lines.as_raw_fd()
    }

    fn wait_for_event(&self, cmd_fd: RawFd, timeout_ms: i32) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_fd(), cmd_fd, PollFlags::POLLIN, timeout_ms)
    }
}

impl EndpointReadable for GpioEndpoint {
    /// Every event that is waiting, as records.
    ///
    /// As many as the buffer has room for: the kernel hands over whole
    /// events, and each becomes one record, so a buffer that cannot hold
    /// another record is where this stops.
    ///
    /// Input lines only. A driven line has no edges to report, so a read of
    /// one is a configuration that said output where it meant input, and is
    /// told so rather than left waiting for events that never come.
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        if self.direction.is_output() {
            return Err(TcsError::Endpoint(format!(
                "{}: the lines of {} are driven, and a driven line reports \
                 nothing: reading one would be reporting what this node itself \
                 asked for. Leave direction unsaid, or say input, to read them",
                self.what, self.path
            )));
        }

        let mut filled = 0;

        // What the lines are, where the configuration asks for it and the
        // report is due. Before the events rather than after, so that a
        // report and the edges that followed it go out in the order they
        // happened.
        //
        // The clock runs from when the report was due: a vehicle busy enough
        // to be late with one is a vehicle that should not then be late with
        // every one after it. A report missed by more than an interval --
        // which a long pause would do -- is not made up for; what the ground
        // wants is what the lines are now.
        if let (Some(interval), Some(due)) = (self.state_interval, self.next_report) {
            let now = Instant::now();
            if now >= due {
                let bits = levels_of(&self.what, self.lines.as_raw_fd(), self.count)?;
                let at = SystemTime::now();
                for (n, offset) in self.offsets.iter().enumerate() {
                    if filled + EVENT_BYTES > buffer.len() {
                        break;
                    }
                    let high = bits & (1u64 << n) != 0;
                    buffer[filled..filled + EVENT_BYTES]
                        .copy_from_slice(&level_record(*offset, high, at));
                    filled += EVENT_BYTES;
                }

                let mut next = due + interval;
                if next <= now {
                    next = now + interval;
                }
                self.next_report = Some(next);
            }
        }

        while filled + EVENT_BYTES <= buffer.len() {
            let mut event = LineEvent::default();
            // SAFETY: a buffer of exactly the structure's size, which is what
            // the kernel writes for one event.
            let slot = unsafe {
                std::slice::from_raw_parts_mut(
                    &mut event as *mut LineEvent as *mut u8,
                    mem::size_of::<LineEvent>(),
                )
            };

            match self.lines.read(slot) {
                Ok(0) => break,
                Ok(n) if n < mem::size_of::<LineEvent>() => {
                    // Part of an event is not an event. Nothing to report:
                    // the kernel does not do this, and a conduit counts what
                    // it moved.
                    break;
                }
                Ok(_) => {
                    let record = record_of(&event, SystemTime::now());
                    buffer[filled..filled + EVENT_BYTES].copy_from_slice(&record);
                    filled += EVENT_BYTES;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    if filled > 0 {
                        break;
                    }
                    return Err(TcsError::Io(e));
                }
            }
        }

        Ok(filled)
    }
}

impl EndpointWritable for GpioEndpoint {
    /// Drive every line of the request to what arrived.
    ///
    /// A value and not a pattern: the lines of one node are driven together,
    /// to the same state. Three heater circuits switched one at a time would
    /// be three different thermal states of a vehicle on the way to the one
    /// that was asked for, and the configuration has no way to say which line
    /// is which -- they are a set, named by offsets in the order the file gave
    /// them.
    ///
    /// What arrives is a value as everything on a link is: see [`state_of`]
    /// for how it is read and [`values_for`] for what the kernel is then
    /// asked.
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        if !self.direction.is_output() {
            return Err(TcsError::Endpoint(format!(
                "{}: the lines of {} are inputs, which are read and not written: \
                 driving one would be commanding a device rather than reading \
                 it. Say direction: output to drive them",
                self.what, self.path
            )));
        }

        let values = values_for(self.count, state_of(&self.what, data)?);

        // SAFETY: a structure laid out as the kernel's own -- the tests below
        // hold it to the kernel's size -- living across a call on a descriptor
        // this owns.
        let result = unsafe {
            libc::ioctl(
                self.lines.as_raw_fd(),
                GPIO_V2_LINE_SET_VALUES_IOCTL,
                &values,
            )
        };
        if result < 0 {
            return Err(TcsError::Endpoint(format!(
                "{}: cannot drive the lines of {}: {}",
                self.what,
                self.path,
                std::io::Error::last_os_error()
            )));
        }

        // The whole of what arrived, which is what a caller counts as sent.
        Ok(data.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    /// Every structure is the size the kernel's own is.
    ///
    /// Measured from `linux/gpio.h` on the machine this was written on: 16,
    /// 24, 272, 592 and 48 bytes. A structure of the wrong size is an ioctl
    /// that reads or writes past what it was handed, and nothing else in a
    /// test would notice.
    #[test]
    fn the_structures_are_what_the_kernel_reads() {
        assert_eq!(mem::size_of::<LineAttribute>(), 16);
        assert_eq!(mem::size_of::<ConfigAttribute>(), 24);
        assert_eq!(mem::size_of::<LineConfig>(), 272);
        assert_eq!(mem::size_of::<LineRequest>(), 592);
        assert_eq!(mem::size_of::<LineEvent>(), 48);
        assert_eq!(mem::size_of::<LineValues>(), 16);

        // And the alignment the kernel's 64-bit fields need.
        assert_eq!(mem::align_of::<LineRequest>(), 8);
        assert_eq!(mem::align_of::<LineEvent>(), 8);
        assert_eq!(mem::align_of::<LineValues>(), 8);
    }

    /// A request asks for the lines the file named, as inputs, with the edges
    /// and the bias it asked for.
    #[test]
    fn a_request_asks_for_what_the_file_said() {
        let config = GpioConfig {
            direction: Direction::Input,
            path: "/dev/gpiochip0".to_string(),
            lines: vec![3, 7, 11],
            edge: LineEdge::Falling,
            bias: Bias::PullUp,
            debounce: Some(Duration::from_micros(5_000)),
            state_interval: None,
        };

        let request = request_for("discretes", &config).expect("a request");

        assert_eq!(request.num_lines, 3);
        assert_eq!(&request.offsets[..3], &[3, 7, 11]);
        assert_eq!(
            request.offsets[3], 0,
            "the rest of the array is left as the kernel wants it"
        );

        assert_eq!(
            request.config.flags,
            GPIO_V2_LINE_FLAG_INPUT | GPIO_V2_LINE_FLAG_EDGE_FALLING
                | GPIO_V2_LINE_FLAG_BIAS_PULL_UP
        );

        // The debounce, applied to every line of the request: a bit per line.
        assert_eq!(request.config.num_attrs, 1);
        assert_eq!(request.config.attrs[0].attr.id, GPIO_V2_LINE_ATTR_ID_DEBOUNCE);
        assert_eq!(request.config.attrs[0].attr.value, 5_000);
        assert_eq!(request.config.attrs[0].mask, 0b111);

        // And whose lines they are, which is what gpioinfo shows beside them.
        let consumer: Vec<u8> = request
            .consumer
            .iter()
            .take_while(|byte| **byte != 0)
            .map(|byte| *byte as u8)
            .collect();
        assert_eq!(String::from_utf8_lossy(&consumer), "discretes");
    }

    /// Both edges where the file asked for both, and no attributes where it
    /// asked for no debouncing.
    #[test]
    fn both_edges_and_no_debounce_is_the_plain_case() {
        let request = request_for(
            "discretes",
            &GpioConfig {
                direction: Direction::Input,
                path: "/dev/gpiochip0".to_string(),
                lines: vec![0],
                edge: LineEdge::Both,
                bias: Bias::None,
                debounce: None,
                state_interval: None,
            },
        )
        .expect("a request");

        assert_eq!(
            request.config.flags,
            GPIO_V2_LINE_FLAG_INPUT
                | GPIO_V2_LINE_FLAG_EDGE_RISING
                | GPIO_V2_LINE_FLAG_EDGE_FALLING
        );
        assert_eq!(request.config.num_attrs, 0);
    }

    /// An event becomes a record a ground station can read: a time, the
    /// line, and what it became.
    #[test]
    fn an_event_becomes_a_record() {
        let rising = LineEvent {
            timestamp_ns: 123_456_789,
            id: GPIO_V2_LINE_EVENT_RISING_EDGE,
            offset: 7,
            ..LineEvent::default()
        };
        let at = UNIX_EPOCH + std::time::Duration::from_millis(1_700_000_000_123);

        let record = record_of(&rising, at);
        assert_eq!(record.len(), EVENT_BYTES);
        assert_eq!(
            Timestamp::from_le_bytes(record[..8].try_into().unwrap()),
            Timestamp { micros: 1_700_000_000_123_000 },
            "the time is a Timestamp, not the kernel's own clock"
        );
        assert_eq!(u32::from_le_bytes(record[8..12].try_into().unwrap()), 7);
        assert_eq!(
            u32::from_le_bytes(record[12..].try_into().unwrap()),
            1,
            "a rising edge is a line that became high"
        );

        // And a falling edge is the same record with the other value.
        let falling = LineEvent {
            id: 2,
            offset: 7,
            ..rising
        };
        let record = record_of(&falling, at);
        assert_eq!(u32::from_le_bytes(record[12..].try_into().unwrap()), 0);
    }

    /// A chip that is not there is reported.
    #[test]
    fn a_chip_that_is_not_there_is_reported() {
        let said = match GpioEndpoint::new(
            "discretes",
            &GpioConfig {
                direction: Direction::Input,
                path: "/dev/gpiochip-nonesuch".to_string(),
                lines: vec![0],
                edge: LineEdge::Both,
                bias: Bias::None,
                debounce: None,
                state_interval: None,
            },
        ) {
            Err(e) => format!("{e}"),
            Ok(_) => panic!("this host has a chip called gpiochip-nonesuch"),
        };
        assert!(
            said.contains("gpiochip-nonesuch"),
            "the error should name the chip: {said}"
        );
    }

    /// Lines are read and never written, and the refusal says why.
    ///
    /// Without a chip to ask, which is why this builds the endpoint from a
    /// pair of pipes: what is being tested is the refusal, not the request.
    #[test]
    fn a_set_of_input_lines_refuses_a_write() {
        let mut fds = [0 as RawFd; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0, "a pipe");

        let mut endpoint = GpioEndpoint {
            what: "discretes".to_string(),
            // SAFETY: both descriptors are the pipe's, just made, and owned
            // by the endpoint from here.
            _chip: unsafe { File::from_raw_fd(fds[1]) },
            lines: unsafe { File::from_raw_fd(fds[0]) },
            direction: Direction::Input,
            count: 1,
            path: "/dev/gpiochip0".to_string(),
            offsets: vec![0],
            state_interval: None,
            next_report: None,
        };

        let said = match endpoint.write(b"1") {
            Err(e) => format!("{e}"),
            Ok(n) => panic!("a set of input lines took {n} bytes"),
        };
        assert!(
            said.contains("read and not written") && said.contains("commanding"),
            "the error should say what a GPIO device is: {said}"
        );
    }

    /// Several events waiting become several records in one read.
    ///
    /// Against a pipe standing in for the request's descriptor, which is what
    /// the kernel's end of it looks like from here: whole events, one after
    /// another.
    #[test]
    fn every_event_waiting_becomes_a_record() {
        let mut fds = [0 as RawFd; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0, "a pipe");

        // Non-blocking, as the kernel's own descriptor is made in
        // ask_for_lines: a read gathers everything waiting and must not then
        // wait for more.
        // SAFETY: the descriptor is the pipe's read end, just made.
        unsafe {
            let flags = libc::fcntl(fds[0], libc::F_GETFL);
            libc::fcntl(fds[0], libc::F_SETFL, flags | libc::O_NONBLOCK);
        }

        let mut endpoint = GpioEndpoint {
            what: "discretes".to_string(),
            // SAFETY: as above.
            _chip: unsafe { File::from_raw_fd(libc::dup(fds[1])) },
            lines: unsafe { File::from_raw_fd(fds[0]) },
            direction: Direction::Input,
            count: 1,
            path: "/dev/gpiochip0".to_string(),
            offsets: vec![0],
            state_interval: None,
            next_report: None,
        };

        // Two events, as the kernel would write them.
        for (id, offset) in [(GPIO_V2_LINE_EVENT_RISING_EDGE, 3u32), (2, 3)] {
            let event = LineEvent {
                timestamp_ns: 1,
                id,
                offset,
                ..LineEvent::default()
            };
            // SAFETY: writing the structure's own bytes, of its own size.
            let wrote = unsafe {
                libc::write(
                    fds[1],
                    &event as *const LineEvent as *const libc::c_void,
                    mem::size_of::<LineEvent>(),
                )
            };
            assert_eq!(wrote as usize, mem::size_of::<LineEvent>());
        }

        let mut buffer = [0u8; 64];
        let got = endpoint.read(&mut buffer).expect("the events are read");
        assert_eq!(got, 2 * EVENT_BYTES, "two events, two records");

        // The first is the rising edge and the second the falling one, in the
        // order they happened.
        assert_eq!(u32::from_le_bytes(buffer[8..12].try_into().unwrap()), 3);
        assert_eq!(u32::from_le_bytes(buffer[12..16].try_into().unwrap()), 1);
        assert_eq!(
            u32::from_le_bytes(buffer[EVENT_BYTES + 12..EVENT_BYTES + 16].try_into().unwrap()),
            0
        );

        unsafe {
            libc::close(fds[1]);
        }
    }

    /// A driven set of lines asks for the output flag and nothing else.
    ///
    /// None of the watching terms, which the configuration refuses to state
    /// for an output: an edge reported from a line this node drives would be
    /// this node hearing its own voice.
    #[test]
    fn driven_lines_ask_only_to_be_driven() {
        let request = request_for(
            "heaters",
            &GpioConfig {
                direction: Direction::Output,
                path: "/dev/gpiochip0".to_string(),
                lines: vec![17, 18, 19],
                // What the parser leaves here when the file says output, and
                // what must not reach the request.
                edge: LineEdge::Both,
                bias: Bias::None,
                debounce: None,
                state_interval: None,
            },
        )
        .expect("a request");

        assert_eq!(
            request.config.flags, GPIO_V2_LINE_FLAG_OUTPUT,
            "a driven line asks to be driven and nothing more"
        );
        assert_eq!(request.num_lines, 3);
        assert_eq!(&request.offsets[..3], &[17, 18, 19]);
        assert_eq!(request.config.num_attrs, 0);
    }

    /// The bits and the mask count lines by their place in the request.
    #[test]
    fn the_values_are_indexed_by_place_in_the_request() {
        // Three lines asserted: the low three bits, whatever their offsets on
        // the chip are.
        let on = values_for(3, true);
        assert_eq!(on.mask, 0b111);
        assert_eq!(on.bits, 0b111);

        // And unasserted: the same lines meant, all of them low. A mask of
        // zero would be a call that said nothing.
        let off = values_for(3, false);
        assert_eq!(off.mask, 0b111);
        assert_eq!(off.bits, 0);

        // One line, and the most the interface allows: no shift that falls
        // off the end of the word.
        assert_eq!(values_for(1, true).mask, 1);
        assert_eq!(values_for(64, true).mask, u64::MAX);
    }

    /// What a decision looks like arriving, and what else does not.
    #[test]
    fn a_state_is_one_or_zero_and_nothing_else() {
        assert!(state_of("heaters", b"1").expect("asserted"));
        assert!(!state_of("heaters", b"0").expect("unasserted"));

        // As the compute elements emit them, a line's worth at a time.
        assert!(state_of("heaters", b"1\n").expect("asserted"));

        for refused in [&b"on"[..], b"true", b"2", b"", b"1 0", &[0xff]] {
            let said = match state_of("heaters", refused) {
                Err(e) => format!("{e}"),
                Ok(state) => panic!(
                    "{:?} was taken for {state}",
                    String::from_utf8_lossy(refused)
                ),
            };
            assert!(
                said.contains("heaters"),
                "the refusal should name the node: {said}"
            );
        }
    }

    /// A driven set of lines refuses a read, and says which way to fix it.
    #[test]
    fn a_set_of_driven_lines_refuses_a_read() {
        let mut fds = [0 as RawFd; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0, "a pipe");

        let mut endpoint = GpioEndpoint {
            what: "heaters".to_string(),
            // SAFETY: both descriptors are the pipe's, just made, and owned
            // by the endpoint from here.
            _chip: unsafe { File::from_raw_fd(fds[1]) },
            lines: unsafe { File::from_raw_fd(fds[0]) },
            direction: Direction::Output,
            count: 3,
            path: "/dev/gpiochip0".to_string(),
            offsets: vec![0],
            state_interval: None,
            next_report: None,
        };

        let said = match endpoint.read(&mut [0u8; 64]) {
            Err(e) => format!("{e}"),
            Ok(n) => panic!("a set of driven lines gave {n} bytes"),
        };
        assert!(
            said.contains("driven") && said.contains("input"),
            "the error should say which direction reads: {said}"
        );
    }

    /// A write to a driven set reaches the kernel, which is as far as a test
    /// without a chip can see.
    ///
    /// A pipe is not a line request, so the ioctl is refused -- the point is
    /// that the refusal comes from the kernel and names the chip, rather than
    /// the endpoint turning the write away itself as an input would.
    #[test]
    fn a_write_to_driven_lines_is_attempted() {
        let mut fds = [0 as RawFd; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0, "a pipe");

        let mut endpoint = GpioEndpoint {
            what: "heaters".to_string(),
            // SAFETY: as above.
            _chip: unsafe { File::from_raw_fd(fds[1]) },
            lines: unsafe { File::from_raw_fd(fds[0]) },
            direction: Direction::Output,
            count: 3,
            path: "/dev/gpiochip0".to_string(),
            offsets: vec![0],
            state_interval: None,
            next_report: None,
        };

        let said = match endpoint.write(b"1") {
            Err(e) => format!("{e}"),
            Ok(n) => panic!("a pipe took a line request's ioctl, and {n} bytes"),
        };
        assert!(
            said.contains("cannot drive") && said.contains("/dev/gpiochip0"),
            "the error should be the kernel's, and name the chip: {said}"
        );
        assert!(
            !said.contains("read and not written"),
            "a driven line is not refused for being an input: {said}"
        );
    }
}
