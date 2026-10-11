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
//! The time is milliseconds since the UNIX epoch, which is the unit every
//! other time in this project is in, so a line's change and a telemetry
//! response can be read against each other. The kernel's own timestamp is
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
use std::time::{SystemTime, UNIX_EPOCH};

use nix::poll::PollFlags;
use tcslibgs::{node::gpio::EVENT_BYTES, Bias, GpioConfig, LineEdge, TcsError, TcsResult};

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
const GPIO_V2_LINE_FLAG_EDGE_RISING: u64 = 1 << 4;
const GPIO_V2_LINE_FLAG_EDGE_FALLING: u64 = 1 << 5;
const GPIO_V2_LINE_FLAG_BIAS_PULL_UP: u64 = 1 << 8;
const GPIO_V2_LINE_FLAG_BIAS_PULL_DOWN: u64 = 1 << 9;

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
    /// The request's own descriptor, which the events are read from.
    lines: File,
    /// Where the chip is, for the error that says a write has nowhere to go.
    path: String,
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

        Ok(Self {
            what: what.to_string(),
            _chip: chip,
            lines,
            path: config.path.clone(),
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
            path: self.path.clone(),
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
    };
    let bias = match config.bias {
        Bias::None => 0,
        Bias::PullUp => GPIO_V2_LINE_FLAG_BIAS_PULL_UP,
        Bias::PullDown => GPIO_V2_LINE_FLAG_BIAS_PULL_DOWN,
    };

    request.config.flags = GPIO_V2_LINE_FLAG_INPUT | edges | bias;

    // The debounce the kernel does, where the file asked for one, applied to
    // every line of the request: a mask with a bit per line.
    if let Some(us) = config.debounce_us {
        let mask = if config.lines.len() as u32 >= u64::BITS {
            u64::MAX
        } else {
            (1u64 << config.lines.len()) - 1
        };
        request.config.attrs[0] = ConfigAttribute {
            attr: LineAttribute {
                id: GPIO_V2_LINE_ATTR_ID_DEBOUNCE,
                padding: 0,
                value: us as u64,
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

/// One event as the bytes a conduit carries: the time, the line, the value.
///
/// Little-endian, because the two ends of this link are the same machine --
/// the handler makes these and the ground reads them, and a device set that
/// crossed architectures would have said so somewhere.
fn record_of(event: &LineEvent, now: SystemTime) -> [u8; EVENT_BYTES] {
    let mut record = [0u8; EVENT_BYTES];

    // The kernel's timestamp is nanoseconds on a monotonic clock, which says
    // nothing to a ground station: what goes out is milliseconds since the
    // epoch, which is what every other time in this project is.
    let millis = now
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_millis() as u64)
        .unwrap_or(0);

    let value: u32 = if event.id == GPIO_V2_LINE_EVENT_RISING_EDGE { 1 } else { 0 };

    record[..8].copy_from_slice(&millis.to_le_bytes());
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
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        let mut filled = 0;

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
    /// Nothing is written to a set of input lines.
    ///
    /// Reported rather than attempted, and the message says what a GPIO
    /// device is: these are inputs, and writing would mean driving an output
    /// -- commanding a device rather than reading one. A triggered GPIO
    /// device is refused where the file is read, so what reaches this is the
    /// ground sending data to a discrete.
    fn write(&mut self, _data: &[u8]) -> TcsResult<usize> {
        Err(TcsError::Endpoint(format!(
            "{}: the lines of {} are inputs, which are read and not written: driving \
             one would be commanding a device rather than reading it",
            self.what, self.path
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        // And the alignment the kernel's 64-bit fields need.
        assert_eq!(mem::align_of::<LineRequest>(), 8);
        assert_eq!(mem::align_of::<LineEvent>(), 8);
    }

    /// A request asks for the lines the file named, as inputs, with the edges
    /// and the bias it asked for.
    #[test]
    fn a_request_asks_for_what_the_file_said() {
        let config = GpioConfig {
            path: "/dev/gpiochip0".to_string(),
            lines: vec![3, 7, 11],
            edge: LineEdge::Falling,
            bias: Bias::PullUp,
            debounce_us: Some(5_000),
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
                path: "/dev/gpiochip0".to_string(),
                lines: vec![0],
                edge: LineEdge::Both,
                bias: Bias::None,
                debounce_us: None,
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

    /// An event becomes a record a ground station can read: a time in
    /// milliseconds, the line, and what it became.
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
            u64::from_le_bytes(record[..8].try_into().unwrap()),
            1_700_000_000_123,
            "the time is milliseconds since the epoch, not the kernel's own clock"
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
                path: "/dev/gpiochip-nonesuch".to_string(),
                lines: vec![0],
                edge: LineEdge::Both,
                bias: Bias::None,
                debounce_us: None,
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
            path: "/dev/gpiochip0".to_string(),
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
            path: "/dev/gpiochip0".to_string(),
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
}
