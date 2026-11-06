//! The GPIO endpoint: discrete lines, as events a conduit can carry.
//!
//! **Supported.** Run against the kernel's simulated chip, by
//! `tests/kernel-dummies.rs`, and against no hardware.
//!
//! This is the one endpoint whose device data does not exist until it is
//! made. Every other kind carries bytes somebody sent; a discrete line has no
//! bytes at all -- it is high or low, and what is worth telling the ground is
//! when it changed. So the handler asks the chip for the configured lines with
//! edge detection, and each event the kernel reports becomes a record of
//! [`EVENT_BYTES`] bytes: when it happened, which line, and what it became.
//!
//! The time is a [`Timestamp`] -- microseconds since the UNIX epoch, eight
//! bytes, most significant first -- which is what every other time in this
//! project is, so a line's change and a telemetry response can be read
//! against each other. The kernel's own timestamp is
//! nanoseconds on a monotonic clock, which says nothing to a ground station;
//! it is converted here, once, where the event is made.
//!
//! Whether the lines are watched or driven is the node's type -- see
//! `tcslibgs::node::payload::gpio::Kind`. Watched lines report: their events
//! and their levels go out groundward, by `world-out`, and a datum arriving
//! at `world-in` asks for their levels now. Driven lines are set: what
//! arrives at `moc-in` is the value they are driven to, and a value that
//! changes them is passed on vehicleward, by `moc-out`, to whatever they
//! drive. A `gpio_inout`'s driven lines also report, groundward by
//! `world-out` every `state_interval`, the last value they were told.
//!
//! The reader and the writer are two halves of one request, each on a
//! duplicate of its descriptor, and what one is told the other has to say: a
//! report asked for through the writer is made by the reader, and a value
//! driven through the writer is passed on and reported by it. What they share
//! is made once, when the node is opened, and is three numbers -- see
//! [`Shared`] -- so that nothing is allocated as data passes.
//!
//! In a run that simulates them, a node that gives a `sim_address` has its
//! lines answered for by tcssim over UDP rather than by a chip: see
//! [`UdpLines`]. Everything above is the same either way -- the records, the
//! reports, what is passed on -- and only where a level comes from and where
//! a drive goes differ.
//!
//! The kernel's structures are declared here, unlike the CAN endpoint's:
//! `libc` has no GPIO character-device definitions. Their sizes are asserted
//! against the kernel's own in the tests below, because a structure of the
//! wrong size is an ioctl that reads past what it was given and nothing else
//! would notice.

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read};
use std::mem;
use std::net::{SocketAddr, UdpSocket};
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use nix::poll::PollFlags;
use tcslibgs::{
    graph::terminals::Stream,
    node::payload::gpio::{state_read, state_said, EVENT_BYTES},
    node::vehicle::compute::from_decision,
    Bias, GpioConfig, LineEdge, TcsError, TcsResult, Timestamp,
};

use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable,
    WaitResult,
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

/// What the two halves of one request tell each other.
///
/// Atomics, because the halves are read and written on whichever thread the
/// node's work is on, and made once, when the node is opened, so that what
/// passes between them allocates nothing.
#[derive(Debug, Default)]
struct Shared {
    /// A report of the watched lines' levels was asked for, by a datum at
    /// `world-in`, and has not been made.
    asked: AtomicBool,
    /// What the driven lines were last driven to: [`UNSET`], [`LOW`] or
    /// [`HIGH`].
    driven: AtomicU8,
    /// A value to pass on by `moc-out`, as the same three: what the lines
    /// were driven to, where it changed them, and it has not been read.
    passing: AtomicU8,
}

/// Nothing driven yet, or nothing to pass on.
const UNSET: u8 = 0;
/// Driven low.
const LOW: u8 = 1;
/// Driven high.
const HIGH: u8 = 2;

/// Where the lines are: a chip's, or tcssim's over UDP.
enum Lines {
    /// A chip's.
    Chip {
        /// The chip's character device, kept open: the line request belongs
        /// to it for as long as it lives.
        _chip: File,
        /// The request's own descriptor, which the events are read from and
        /// the line states driven through.
        request: File,
    },
    /// tcssim's, at the node's `sim_address`.
    Udp(UdpLines),
}

/// Lines tcssim answers for over UDP, in a run that simulates them.
///
/// One datagram is one state of the lines: an ASCII `1` or `0` for each line
/// of the node, in the order of the request, lowest offset first -- `10` for
/// lines 4 and 5 with 4 asserted. The vehicle sends what it drives its lines
/// to; tcssim sends what watched lines are, whenever that changes. An empty
/// datagram from the vehicle, sent when it opens and again while it has heard
/// nothing, says where to answer.
///
/// What the kernel does for a chip is done here from the levels tcssim sends,
/// so that what goes out is what a chip's would be: an event for each edge the
/// node asks for, as the same record, and the levels when a report is due. The
/// first state tcssim sends is what the lines are, not a change, as a chip's
/// lines have no event for being what they were when asked for.
struct UdpLines {
    /// Connected to the `sim_address`, so that nothing else's datagrams are
    /// taken for the lines'.
    socket: UdpSocket,
    /// Where tcssim answers, for what a complaint says.
    at: SocketAddr,
    /// Which changes of a watched line are events.
    edge: LineEdge,
    /// The levels tcssim last sent, which both halves of the node read.
    seen: Arc<Seen>,
}

/// What tcssim last said the lines are.
#[derive(Debug, Default)]
struct Seen {
    /// A bit for each line by its place in the request, set where it is
    /// asserted.
    levels: AtomicU64,
    /// Whether tcssim has said anything yet.
    known: AtomicBool,
}

/// A set of discrete lines: watched, or driven.
pub struct GpioEndpoint {
    /// The handler this endpoint belongs to, as the device configuration
    /// names it.
    what: String,
    /// Where the lines are.
    lines: Lines,
    /// Which of them are watched and which driven, a bit for each by its
    /// place in the request: the kernel counts lines by their place, and
    /// these are what a report and a drive ask it about.
    watched: u64,
    driven: u64,
    /// Where the chip is, for the error that says a write has nowhere to go.
    path: String,
    /// The offsets of the lines, in the order they were asked for, which is
    /// the order the kernel reports their values in.
    offsets: Vec<u32>,
    /// Whether the driven lines report what they were last told: a
    /// `gpio_inout`.
    reports: bool,
    /// How often what the watched lines are, or what the driven ones were
    /// told, is reported, where the configuration asks for it.
    state_interval: Option<Duration>,
    /// When the next report is due.
    ///
    /// From when the last one was due rather than from when it was made, so
    /// a report that went out late does not push the next one later.
    next_report: Option<Instant>,
    /// What the two halves of the request tell each other.
    shared: Arc<Shared>,
    /// Which stream the last read's data is on: groundward for what the
    /// watched lines said, vehicleward for a value passed on.
    last: Option<Stream>,
}

/// The bits of the request that are these lines, by their places in it.
///
/// # Arguments
///
/// * `offsets` - Every line of the request, in its order.
/// * `these`   - The lines asked about.
///
/// # Returns
///
/// A bit for each of `these`, at its place in `offsets`.
fn mask_of(offsets: &[u32], these: &[u32]) -> u64 {
    offsets
        .iter()
        .enumerate()
        .filter(|(_, offset)| these.contains(offset))
        .fold(0, |mask, (n, _)| mask | (1u64 << n))
}

/// Reads what every line of the request is, now.
///
/// One ioctl on the descriptor the events are read from: the request is
/// already open and already names the lines, so nothing is opened and nothing
/// is asked twice. The kernel answers with a bit per line, in the order the
/// lines were asked for.
///
/// # Arguments
///
/// * `what` - The handler, for what a complaint says.
/// * `fd`   - The line request's descriptor.
/// * `mask` - Which lines of the request to ask about, by place.
///
/// # Returns
///
/// A bit per line, bit 0 the first line asked for, set where the line is
/// asserted.
///
/// # Errors
///
/// `TcsError::Endpoint` if the kernel will not say.
fn levels_of(what: &str, fd: RawFd, mask: u64) -> TcsResult<u64> {
    let mut values = LineValues { bits: 0, mask };

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

/// Gives one line's level as the bytes a conduit carries.
///
/// The record an event becomes -- the time, the line, the value -- said of a
/// line that has not changed: what it is now, observed now. Both kinds say
/// "this line had this value at this time", which is what a ground station
/// plots; the difference is that an event's time is the kernel's for the edge
/// and this one's is when the vehicle looked.
///
/// # Arguments
///
/// * `offset` - The line's offset on the chip.
/// * `high`   - Whether the line is asserted.
/// * `now`    - When the vehicle looked.
///
/// # Returns
///
/// The record: the time as a [`Timestamp`], the offset and the value (1 or
/// 0) as a 32-bit word, each most significant byte first.
fn level_record(offset: u32, high: bool, now: SystemTime) -> [u8; EVENT_BYTES] {
    let mut record = [0u8; EVENT_BYTES];
    record[..8].copy_from_slice(&Timestamp::at(now).to_be_bytes());
    record[8..12].copy_from_slice(&offset.to_be_bytes());
    record[12..].copy_from_slice(&u32::from(high).to_be_bytes());
    record
}

impl UdpLines {
    /// Reaches tcssim's lines at a node's `sim_address`, and says where to
    /// answer.
    ///
    /// # Arguments
    ///
    /// * `what` - The handler, for what a complaint says.
    /// * `at`   - The `sim_address`.
    /// * `edge` - Which changes of a watched line are events.
    ///
    /// # Returns
    ///
    /// The lines, on a non-blocking socket connected to `at`.
    ///
    /// # Errors
    ///
    /// `TcsError::Endpoint` if the socket cannot be made, connected or made
    /// non-blocking.
    fn open(what: &str, at: SocketAddr, edge: LineEdge) -> TcsResult<Self> {
        let failed = |e: std::io::Error| {
            TcsError::Endpoint(format!(
                "{what}: cannot reach the simulated lines at {at}: {e}"
            ))
        };
        let socket = UdpSocket::bind((at.ip(), 0)).map_err(failed)?;
        socket.connect(at).map_err(failed)?;
        socket.set_nonblocking(true).map_err(failed)?;
        let lines = Self {
            socket,
            at,
            edge,
            seen: Arc::new(Seen::default()),
        };
        lines.hello();
        Ok(lines)
    }

    /// The same lines, on a duplicate of the socket, with what tcssim said
    /// shared rather than copied.
    ///
    /// # Returns
    ///
    /// The second half.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if the socket cannot be duplicated.
    fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            socket: self.socket.try_clone()?,
            at: self.at,
            edge: self.edge,
            seen: Arc::clone(&self.seen),
        })
    }

    /// Tells tcssim where to answer: an empty datagram. Lost if tcssim is
    /// not there yet, which is why it is said again until tcssim answers.
    fn hello(&self) {
        let _ = self.socket.send(&[]);
    }

    /// What the lines are, as tcssim last said.
    ///
    /// # Arguments
    ///
    /// * `mask` - Which lines, by place.
    ///
    /// # Returns
    ///
    /// A bit per line, set where it is asserted; none, until tcssim has said.
    fn levels(&self, mask: u64) -> u64 {
        if !self.seen.known.load(Ordering::Acquire) {
            self.hello();
        }
        self.seen.levels.load(Ordering::Acquire) & mask
    }

    /// Sends what the lines are driven to.
    ///
    /// # Arguments
    ///
    /// * `what`    - The handler, for what a complaint says.
    /// * `count`   - How many lines the request has.
    /// * `values`  - What they are driven to.
    ///
    /// # Errors
    ///
    /// `TcsError::Endpoint` if the datagram cannot be sent.
    fn drive(
        &self,
        what: &str,
        count: usize,
        values: &LineValues,
    ) -> TcsResult<()> {
        let datagram = state_said(count, values.bits & values.mask);
        self.socket.send(&datagram).map_err(|e| {
            TcsError::Endpoint(format!(
                "{what}: cannot drive the simulated lines at {}: {e}",
                self.at
            ))
        })?;
        Ok(())
    }

    /// Takes what tcssim has sent, as records of the edges the node asks for,
    /// while the buffer has room for every watched line's.
    ///
    /// # Arguments
    ///
    /// * `what`    - The handler, for what a complaint says.
    /// * `offsets` - Every line of the request, in its order.
    /// * `watched` - The watched lines, a bit for each by its place.
    /// * `buffer`  - Where the records go.
    /// * `filled`  - How much of `buffer` is already used.
    ///
    /// # Returns
    ///
    /// How much of `buffer` is used afterwards.
    ///
    /// # Errors
    ///
    /// `TcsError::Endpoint` for a datagram that is not a state of the lines;
    /// `TcsError::Io` for a socket that fails.
    fn edges(
        &self,
        what: &str,
        offsets: &[u32],
        watched: u64,
        buffer: &mut [u8],
        mut filled: usize,
    ) -> TcsResult<usize> {
        let most = EVENT_BYTES * watched.count_ones() as usize;
        let mut datagram = [0u8; GPIO_V2_LINES_MAX + 1];
        while filled + most <= buffer.len() {
            let n = match self.socket.recv(&mut datagram) {
                Ok(n) => n,
                // Nothing yet, or tcssim not there yet: the hello was
                // refused, and is said again until it is answered.
                Err(ref e)
                    if matches!(
                        e.kind(),
                        ErrorKind::WouldBlock | ErrorKind::ConnectionRefused
                    ) =>
                {
                    break
                }
                Err(e) => return Err(TcsError::Io(e)),
            };
            let now =
                state_read(offsets.len(), &datagram[..n]).map_err(|why| {
                    TcsError::Endpoint(format!(
                        "{what}: {} sent {why}",
                        self.at
                    ))
                })?;
            let was = self.seen.levels.swap(now, Ordering::AcqRel);
            if !self.seen.known.swap(true, Ordering::AcqRel) {
                continue;
            }
            let at = SystemTime::now();
            for (place, offset) in offsets.iter().enumerate() {
                let bit = 1u64 << place;
                if watched & bit == 0 || (was ^ now) & bit == 0 {
                    continue;
                }
                let high = now & bit != 0;
                let wanted = match self.edge {
                    LineEdge::Both => true,
                    LineEdge::Rising => high,
                    LineEdge::Falling => !high,
                    LineEdge::None => false,
                };
                if wanted {
                    buffer[filled..filled + EVENT_BYTES]
                        .copy_from_slice(&level_record(*offset, high, at));
                    filled += EVENT_BYTES;
                }
            }
        }
        Ok(filled)
    }
}

impl Lines {
    /// The descriptor to wait on: the request's, or the socket's.
    fn fd(&self) -> RawFd {
        match self {
            Lines::Chip { request, .. } => request.as_raw_fd(),
            Lines::Udp(udp) => udp.socket.as_raw_fd(),
        }
    }

    /// The same lines, for the other half of the node.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if a descriptor cannot be duplicated.
    fn try_clone(&self) -> TcsResult<Self> {
        Ok(match self {
            Lines::Chip { _chip, request } => Lines::Chip {
                _chip: _chip.try_clone()?,
                request: request.try_clone()?,
            },
            Lines::Udp(udp) => Lines::Udp(udp.try_clone()?),
        })
    }
}

impl GpioEndpoint {
    /// Asks the chip for the configured lines: the watched ones with edge
    /// detection where it is asked for, and the driven ones as outputs. Or,
    /// where the run simulates them, reaches tcssim's at the node's
    /// `sim_address`: see [`UdpLines`].
    ///
    /// # Arguments
    ///
    /// * `what`   - The handler this endpoint belongs to, as the device
    ///              configuration names it; also the consumer the lines
    ///              are held under.
    /// * `config` - The chip, the lines watched and driven, their edges,
    ///              bias and debounce, and how often their state is
    ///              reported.
    ///
    /// # Returns
    ///
    /// The endpoint, holding the lines, with its first state report (if
    /// any) due at once.
    ///
    /// # Errors
    ///
    /// `TcsError::Config` if the number of lines is not one a request can
    /// hold; `TcsError::Endpoint` if the chip cannot be opened or will not
    /// give up the lines, or the simulated lines cannot be reached.
    pub fn new(what: &str, config: &GpioConfig) -> TcsResult<Self> {
        let offsets = config.lines();
        let request = request_for(what, config, &offsets)?;
        let lines = match config.sim_address {
            Some(at) => Lines::Udp(UdpLines::open(what, at, config.edge)?),
            None => {
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
                let request = ask_for_lines(
                    what,
                    chip.as_raw_fd(),
                    request,
                    &config.path,
                )?;
                Lines::Chip {
                    _chip: chip,
                    request,
                }
            }
        };

        // Watched lines are reported on, and the driven lines of a
        // gpio_inout; a gpio_out's say nothing groundward.
        let reports = config.kind.reports();
        let state_interval = config
            .state_interval
            .filter(|_| !config.inputs.is_empty() || reports);

        Ok(Self {
            what: what.to_string(),
            lines,
            watched: mask_of(&offsets, &config.inputs),
            driven: mask_of(&offsets, &config.outputs),
            path: config.path.clone(),
            offsets,
            reports,
            state_interval,
            // The first report is due at once, so that a ground station
            // connecting to a vehicle that has just started learns what the
            // lines are without waiting an interval for it.
            next_report: state_interval.map(|_| Instant::now()),
            shared: Arc::new(Shared::default()),
            last: None,
        })
    }

    /// A second endpoint on the same line request.
    ///
    /// The request's descriptor is duplicated rather than the request made
    /// again: the kernel gives a line to one request at a time, so a second
    /// request for the same lines would be refused. What the two tell each
    /// other is shared, not copied: see [`Shared`].
    ///
    /// # Returns
    ///
    /// The second endpoint, on duplicates of this one's descriptors.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if a descriptor cannot be duplicated.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            what: self.what.clone(),
            lines: self.lines.try_clone()?,
            watched: self.watched,
            driven: self.driven,
            path: self.path.clone(),
            offsets: self.offsets.clone(),
            reports: self.reports,
            state_interval: self.state_interval,
            next_report: self.next_report,
            shared: Arc::clone(&self.shared),
            last: None,
        })
    }

    /// Whether a report of the watched lines' levels is due: by the clock,
    /// or because one was asked for.
    fn report_due(&self) -> bool {
        self.watched != 0
            && (self.shared.asked.load(Ordering::Acquire) || self.on_time())
    }

    /// Whether a report of what the driven lines were last told is due: by
    /// the clock, once they have been told something.
    fn told_due(&self) -> bool {
        self.reports
            && self.shared.driven.load(Ordering::Acquire) != UNSET
            && self.on_time()
    }

    /// Whether the report clock has come round.
    fn on_time(&self) -> bool {
        self.next_report.is_some_and(|due| Instant::now() >= due)
    }

    /// Sets the report clock going round again: from when this report was
    /// due, so a report that went out late does not push the next one later,
    /// and not making up for one missed by more than an interval, what the
    /// ground wants being what the lines are now.
    fn wind(&mut self) {
        if let (Some(interval), Some(due)) =
            (self.state_interval, self.next_report)
        {
            let now = Instant::now();
            if now >= due {
                let mut next = due + interval;
                if next <= now {
                    next = now + interval;
                }
                self.next_report = Some(next);
            }
        }
    }
}

/// Builds the request this configuration asks for.
///
/// One request for every line: watched lines flagged as inputs, with the
/// edges, bias and debounce asked for, and driven lines as outputs.
///
/// # Arguments
///
/// * `what`    - The handler, for what a complaint says and as the
///               consumer the lines are held under.
/// * `config`  - The lines and how they are to be configured.
/// * `offsets` - Every line of the request, in the order it is asked for.
///
/// # Returns
///
/// The request, ready for `GPIO_V2_GET_LINE_IOCTL`.
///
/// # Errors
///
/// `TcsError::Config` if there are no lines or more than
/// `GPIO_V2_LINES_MAX`.
fn request_for(
    what: &str,
    config: &GpioConfig,
    offsets: &[u32],
) -> TcsResult<LineRequest> {
    if offsets.is_empty() || offsets.len() > GPIO_V2_LINES_MAX {
        return Err(TcsError::Config(format!(
            "{what}: {} lines is not between 1 and {GPIO_V2_LINES_MAX}",
            offsets.len()
        )));
    }

    let mut request = LineRequest::default();
    for (n, line) in offsets.iter().enumerate() {
        request.offsets[n] = *line;
    }
    request.num_lines = offsets.len() as u32;

    // Whose lines these are, which is what `gpioinfo` shows beside them: a
    // line held by something unnamed is the hardest kind of conflict to find.
    for (n, byte) in what.bytes().take(GPIO_MAX_NAME_SIZE - 1).enumerate() {
        request.consumer[n] = byte as libc::c_char;
    }

    // Driven lines ask for none of the watching terms, which the
    // configuration refuses to state for them: an output has no edges to
    // report and nothing to debounce.
    if config.inputs.is_empty() {
        request.config.flags = GPIO_V2_LINE_FLAG_OUTPUT;
        return Ok(request);
    }

    let edges = match config.edge {
        LineEdge::Rising => GPIO_V2_LINE_FLAG_EDGE_RISING,
        LineEdge::Falling => GPIO_V2_LINE_FLAG_EDGE_FALLING,
        LineEdge::Both => {
            GPIO_V2_LINE_FLAG_EDGE_RISING | GPIO_V2_LINE_FLAG_EDGE_FALLING
        }
        // Asked for as a plain input: there are no events to read, and what
        // the lines are is taken when the report is due.
        LineEdge::None => 0,
    };
    let bias = match config.bias {
        Bias::None => 0,
        Bias::PullUp => GPIO_V2_LINE_FLAG_BIAS_PULL_UP,
        Bias::PullDown => GPIO_V2_LINE_FLAG_BIAS_PULL_DOWN,
    };
    request.config.flags = GPIO_V2_LINE_FLAG_INPUT | edges | bias;

    // The debounce the kernel does, where the file asked for one, applied to
    // every line of the request: a bit per line.
    if let Some(steady) = config.debounce {
        request.config.attrs[0] = ConfigAttribute {
            attr: LineAttribute {
                id: GPIO_V2_LINE_ATTR_ID_DEBOUNCE,
                padding: 0,
                // The kernel's own attribute counts microseconds, which
                // is where a length of time becomes a number.
                value: steady.as_micros() as u64,
            },
            mask: mask_of(offsets, &config.inputs),
        };
        request.config.num_attrs = 1;
    }

    Ok(request)
}

/// Makes the request, and hands back the descriptor the events arrive on.
///
/// # Arguments
///
/// * `what`    - The handler, for what a complaint says.
/// * `chip`    - The chip's open character device.
/// * `request` - The request, which the kernel fills with the descriptor.
/// * `path`    - Where the chip is, for what a complaint says.
///
/// # Returns
///
/// The line request's descriptor, non-blocking.
///
/// # Errors
///
/// `TcsError::Endpoint` if the chip refuses the request, gives no
/// descriptor, or the descriptor cannot be made non-blocking.
fn ask_for_lines(
    what: &str,
    chip: RawFd,
    mut request: LineRequest,
    path: &str,
) -> TcsResult<File> {
    // SAFETY: the structure is laid out as the kernel's own -- the tests
    // below hold it to the kernel's sizes -- and lives across the call, which
    // writes the descriptor into it.
    let asked =
        unsafe { libc::ioctl(chip, GPIO_V2_GET_LINE_IOCTL, &mut request) };
    if asked < 0 {
        return Err(TcsError::Endpoint(format!(
            "{what}: {path} will not give up its lines: {}. A line is given to \
             one request at a time, so this is commonly a line another program \
             holds",
            std::io::Error::last_os_error()
        )));
    }

    if request.fd < 0 {
        return Err(TcsError::Endpoint(format!(
            "{what}: {path} took the request and gave no descriptor"
        )));
    }

    // Owned at once, so that a failure below closes it rather than leaving
    // the lines held by a descriptor nobody has.
    // SAFETY: the kernel has just given this descriptor, and nothing else
    // owns it.
    let lines = unsafe { File::from_raw_fd(request.fd) };

    // Read without waiting, as every other endpoint does, and for a sharper
    // reason here: a read gathers every event that is waiting, so a blocking
    // descriptor would have it wait for the next one after collecting the
    // last -- a conduit stopped until a line happened to change again, with
    // the events it had already gathered unreported.
    // SAFETY: both calls take the descriptor by value and return a status,
    // and `lines` keeps it open across them.
    let flags = unsafe { libc::fcntl(lines.as_raw_fd(), libc::F_GETFL) };
    if flags < 0
        // SAFETY: as for F_GETFL: `lines` keeps the descriptor open.
        || unsafe {
            libc::fcntl(
                lines.as_raw_fd(),
                libc::F_SETFL,
                flags | libc::O_NONBLOCK,
            )
        } < 0
    {
        return Err(TcsError::Endpoint(format!(
            "{what}: cannot make {path}'s line request non-blocking: {}",
            std::io::Error::last_os_error()
        )));
    }

    Ok(lines)
}

/// Reads what arrived as the state the lines are to be driven to.
///
/// The text `1` to assert and `0` to unassert, which is what a compute
/// element's decision looks like on a link, and what a ground station can
/// send by hand. Anything else is refused rather than taken for one of them:
/// the thing on the other end of this is a heater or a valve, and a datum
/// that cannot be read is not guessed at. What a decision is, is
/// `tcslibgs::node::vehicle::compute::from_decision`, which the ground reads
/// one by too.
///
/// # Arguments
///
/// * `what` - The handler, for what a complaint says.
/// * `data` - What arrived, surrounding white space allowed.
///
/// # Returns
///
/// True to assert the lines, false to unassert them.
///
/// # Errors
///
/// `TcsError::Endpoint` if `data` is not the text `1` or `0`.
fn state_of(what: &str, data: &[u8]) -> TcsResult<bool> {
    from_decision(data, what).map_err(TcsError::Endpoint)
}

/// Builds what the kernel is asked for: every driven line of the request at
/// the one state.
///
/// Indexed by a line's place in the request and not by its offset on the
/// chip, which is how the v2 interface counts: lines 3, 7 and 11 asked for in
/// that order are bits 0, 1 and 2 here. The mask says which bits were meant,
/// which is the driven lines and only them: a watched line is not set.
///
/// # Arguments
///
/// * `driven` - The driven lines, a bit for each by its place.
/// * `wanted` - Whether they are to be asserted.
///
/// # Returns
///
/// The values, with a mask covering every driven line.
fn values_for(driven: u64, wanted: bool) -> LineValues {
    LineValues {
        bits: if wanted { driven } else { 0 },
        mask: driven,
    }
}

/// Gives one event as the bytes a conduit carries: the time, the line, the
/// value.
///
/// Most significant byte first, as everything this software puts on the
/// network is unless the protocol it goes in says otherwise. That is the
/// order a [`Timestamp`] goes on the wire in, so the eight bytes of time
/// here are the eight bytes of time in a telemetry response.
///
/// # Arguments
///
/// * `event` - The event the kernel reported.
/// * `now`   - The time to give it, in place of the kernel's monotonic
///             one.
///
/// # Returns
///
/// The record: the time, the line's offset, and 1 for a rising edge or 0
/// for a falling one.
fn record_of(event: &LineEvent, now: SystemTime) -> [u8; EVENT_BYTES] {
    let mut record = [0u8; EVENT_BYTES];

    // The kernel's timestamp is nanoseconds on a monotonic clock, which says
    // nothing to a ground station: what goes out is a Timestamp, which is
    // what every other time in this project is, so a line's change and a
    // telemetry response can be read against each other.
    let value: u32 = if event.id == GPIO_V2_LINE_EVENT_RISING_EDGE {
        1
    } else {
        0
    };

    record[..8].copy_from_slice(&Timestamp::at(now).to_be_bytes());
    record[8..12].copy_from_slice(&event.offset.to_be_bytes());
    record[12..].copy_from_slice(&value.to_be_bytes());
    record
}

impl EndpointWaitable for GpioEndpoint {
    fn io_fd(&self) -> RawFd {
        self.lines.fd()
    }

    fn wait_for_event(
        &self,
        cmd_fd: RawFd,
        timeout_ms: i32,
    ) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_fd(), cmd_fd, PollFlags::POLLIN, timeout_ms)
    }
}

impl EndpointReadable for GpioEndpoint {
    /// Whether there is something to read whatever the descriptor says: a
    /// report of the watched lines, due by the clock or asked for -- lines
    /// watched for no edge have a descriptor nothing ever makes readable --
    /// a value the driven lines were set to, to pass on, or a report of what
    /// they were last told.
    fn due(&self) -> bool {
        self.report_due()
            || self.told_due()
            || self.shared.passing.load(Ordering::Acquire) != UNSET
    }

    /// Which stream the last read was on: see
    /// [`EndpointReadable::stream`].
    fn stream(&self) -> Option<Stream> {
        self.last
    }

    /// A value to pass on, a report of what the lines were last told, or
    /// every event that is waiting, as records.
    ///
    /// A value passed on is read alone, being vehicleward where the rest is
    /// groundward, and one read is on one stream. Otherwise as many records
    /// as the buffer has room for: the kernel hands over whole events, and
    /// each becomes one record, so a buffer that cannot hold another record
    /// is where this stops.
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        // What the driven lines were set to, passed on as the decision it
        // was: the text a thermostat's decision is, which is what whatever
        // the lines drive reads one as.
        let passing = self.shared.passing.swap(UNSET, Ordering::AcqRel);
        if passing != UNSET && !buffer.is_empty() {
            buffer[0] = if passing == HIGH { b'1' } else { b'0' };
            self.last = Some(Stream::Vehicleward);
            return Ok(1);
        }
        self.last = Some(Stream::Groundward);

        // What a gpio_inout's lines were last told, as the decision it was,
        // on the report clock: the ground follows a heater's switch on a
        // beat, and a report lost is made good by the next.
        if self.told_due() && !buffer.is_empty() {
            let told = self.shared.driven.load(Ordering::Acquire);
            buffer[0] = if told == HIGH { b'1' } else { b'0' };
            self.wind();
            return Ok(1);
        }
        if self.watched == 0 {
            return Ok(0);
        }

        let mut filled = 0;

        // What the lines are, where the configuration asks for it and the
        // report is due, or a datum at world-in has asked for it. Before the
        // events rather than after, so that a report and the edges that
        // followed it go out in the order they happened.
        //
        // The clock runs from when the report was due: a vehicle busy enough
        // to be late with one is a vehicle that should not then be late with
        // every one after it. A report missed by more than an interval --
        // which a long pause would do -- is not made up for; what the ground
        // wants is what the lines are now. One asked for leaves the clock as
        // it was.
        if self.report_due() {
            self.shared.asked.store(false, Ordering::Release);
            let bits = match &self.lines {
                Lines::Chip { request, .. } => {
                    levels_of(&self.what, request.as_raw_fd(), self.watched)?
                }
                Lines::Udp(udp) => udp.levels(self.watched),
            };
            let at = SystemTime::now();
            for (n, offset) in self.offsets.iter().enumerate() {
                if self.watched & (1u64 << n) == 0 {
                    continue;
                }
                if filled + EVENT_BYTES > buffer.len() {
                    break;
                }
                let high = bits & (1u64 << n) != 0;
                buffer[filled..filled + EVENT_BYTES]
                    .copy_from_slice(&level_record(*offset, high, at));
                filled += EVENT_BYTES;
            }
            self.wind();
        }

        let request = match &mut self.lines {
            Lines::Chip { request, .. } => request,
            Lines::Udp(udp) => {
                return udp.edges(
                    &self.what,
                    &self.offsets,
                    self.watched,
                    buffer,
                    filled,
                )
            }
        };
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

            match request.read(slot) {
                Ok(0) => break,
                Ok(n) if n < mem::size_of::<LineEvent>() => {
                    // Part of an event is not an event. Nothing to report:
                    // the kernel does not do this, and a conduit counts what
                    // it moved.
                    break;
                }
                Ok(_) => {
                    let record = record_of(&event, SystemTime::now());
                    buffer[filled..filled + EVENT_BYTES]
                        .copy_from_slice(&record);
                    filled += EVENT_BYTES;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    break
                }
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
    /// What arrived: a `gpio_in`'s request for a report, which is its one way
    /// in, or what a `gpio_out`'s or a `gpio_inout`'s lines are driven to.
    ///
    /// A value and not a pattern: the driven lines of one node are driven
    /// together, to the same state. Three heater circuits switched one at a
    /// time would be three different thermal states of a vehicle on the way
    /// to the one that was asked for, and the configuration has no way to say
    /// which line is which -- they are a set, named by offsets, held lowest
    /// first. What arrives is a value as everything on a link is: see
    /// `state_of` for how it is read and `values_for` for what the kernel is
    /// then asked.
    ///
    /// A value that changes the lines is passed on, by `moc-out`; one that
    /// says again what they are is not. A thermostat says its decision on
    /// every sample, and whatever the lines drive is told when they switch,
    /// not every time they did not.
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        if self.driven == 0 {
            // Asked, whatever was said: the request is the datum arriving.
            self.shared.asked.store(true, Ordering::Release);
            return Ok(data.len());
        }

        let wanted = state_of(&self.what, data)?;
        let values = values_for(self.driven, wanted);

        match &self.lines {
            Lines::Chip { request, .. } => {
                // SAFETY: a structure laid out as the kernel's own -- the
                // tests below hold it to the kernel's size -- living across a
                // call on a descriptor this owns.
                let result = unsafe {
                    libc::ioctl(
                        request.as_raw_fd(),
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
            }
            Lines::Udp(udp) => {
                udp.drive(&self.what, self.offsets.len(), &values)?
            }
        }

        let now = if wanted { HIGH } else { LOW };
        if self.shared.driven.swap(now, Ordering::AcqRel) != now {
            self.shared.passing.store(now, Ordering::Release);
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

    /// Lines watched, as a file's `gpio_in` states them.
    ///
    /// # Arguments
    ///
    /// * `lines` - The offsets.
    /// * `edge`  - Which edges are events.
    ///
    /// # Returns
    ///
    /// The lines' configuration, on `/dev/gpiochip0`.
    fn watched(lines: Vec<u32>, edge: LineEdge) -> GpioConfig {
        GpioConfig {
            kind: tcslibgs::node::payload::gpio::Kind::In,
            path: "/dev/gpiochip0".to_string(),
            inputs: lines,
            outputs: Vec::new(),
            edge,
            bias: Bias::None,
            debounce: None,
            state_interval: None,
            sim_address: None,
        }
    }

    /// An endpoint on a pair of pipes in place of a chip and a request, for
    /// what can be tested without one: what is refused, what is asked for,
    /// and what is passed on.
    ///
    /// # Arguments
    ///
    /// * `what`    - The node.
    /// * `offsets` - Every line of the request, in its order.
    /// * `watched` - The watched lines, a bit for each by place.
    /// * `driven`  - The driven lines, the same way.
    ///
    /// # Returns
    ///
    /// The endpoint, and the write end of the pipe its `lines` reads.
    fn on_a_pipe(
        what: &str,
        offsets: Vec<u32>,
        watched: u64,
        driven: u64,
    ) -> (GpioEndpoint, RawFd) {
        let mut fds = [0 as RawFd; 2];
        // SAFETY: pipe needs room for two descriptors to write, which `fds`
        // is: a local array of two.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0, "a pipe");

        // Non-blocking, as the kernel's own descriptor is made in
        // ask_for_lines: a read gathers everything waiting and must not then
        // wait for more.
        // SAFETY: the descriptor is the pipe's read end, just made.
        unsafe {
            let flags = libc::fcntl(fds[0], libc::F_GETFL);
            libc::fcntl(fds[0], libc::F_SETFL, flags | libc::O_NONBLOCK);
        }

        // SAFETY: dup takes the descriptor by value; the pipe is open.
        let chip = unsafe { libc::dup(fds[1]) };
        assert!(chip >= 0, "a copy of the pipe's end");
        let endpoint = GpioEndpoint {
            what: what.to_string(),
            lines: Lines::Chip {
                // SAFETY: the copy was just made and nothing else owns it.
                _chip: unsafe { File::from_raw_fd(chip) },
                // SAFETY: the pipe's read end, just made, owned by the
                // endpoint from here and not used through `fds[0]` again.
                request: unsafe { File::from_raw_fd(fds[0]) },
            },
            watched,
            driven,
            path: "/dev/gpiochip0".to_string(),
            offsets,
            reports: false,
            state_interval: None,
            next_report: None,
            shared: Arc::new(Shared::default()),
            last: None,
        };
        (endpoint, fds[1])
    }

    /// Closes the write end of a test's pipe.
    fn close(fd: RawFd) {
        // SAFETY: close needs a descriptor nothing else owns or will use,
        // which the write end is: only the test holds it -- the endpoint
        // owns a dup of it -- and it is not used after this.
        unsafe {
            libc::close(fd);
        }
    }

    /// A request asks for the lines the file named, as inputs, with the edges
    /// and the bias it asked for.
    #[test]
    fn a_request_asks_for_what_the_file_said() {
        let config = GpioConfig {
            bias: Bias::PullUp,
            debounce: Some(Duration::from_micros(5_000)),
            ..watched(vec![3, 7, 11], LineEdge::Falling)
        };

        let request = request_for("discretes", &config, &config.lines())
            .expect("a request");

        assert_eq!(request.num_lines, 3);
        assert_eq!(&request.offsets[..3], &[3, 7, 11]);
        assert_eq!(
            request.offsets[3], 0,
            "the rest of the array is left as the kernel wants it"
        );

        assert_eq!(
            request.config.flags,
            GPIO_V2_LINE_FLAG_INPUT
                | GPIO_V2_LINE_FLAG_EDGE_FALLING
                | GPIO_V2_LINE_FLAG_BIAS_PULL_UP
        );

        // The debounce, applied to every line of the request: a bit per line.
        assert_eq!(request.config.num_attrs, 1);
        assert_eq!(
            request.config.attrs[0].attr.id,
            GPIO_V2_LINE_ATTR_ID_DEBOUNCE
        );
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
        let config = watched(vec![0], LineEdge::Both);
        let request = request_for("discretes", &config, &config.lines())
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
        let at =
            UNIX_EPOCH + std::time::Duration::from_millis(1_700_000_000_123);

        let record = record_of(&rising, at);
        assert_eq!(record.len(), EVENT_BYTES);
        assert_eq!(
            Timestamp::from_be_bytes(record[..8].try_into().unwrap()),
            Timestamp {
                micros: 1_700_000_000_123_000
            },
            "the time is a Timestamp, not the kernel's own clock"
        );
        assert_eq!(u32::from_be_bytes(record[8..12].try_into().unwrap()), 7);
        assert_eq!(
            u32::from_be_bytes(record[12..].try_into().unwrap()),
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
        assert_eq!(u32::from_be_bytes(record[12..].try_into().unwrap()), 0);
    }

    /// A chip that is not there is reported.
    #[test]
    fn a_chip_that_is_not_there_is_reported() {
        let config = GpioConfig {
            path: "/dev/gpiochip-nonesuch".to_string(),
            ..watched(vec![0], LineEdge::Both)
        };
        let said = match GpioEndpoint::new("discretes", &config) {
            Err(e) => format!("{e}"),
            Ok(_) => panic!("this host has a chip called gpiochip-nonesuch"),
        };
        assert!(
            said.contains("gpiochip-nonesuch"),
            "the error should name the chip: {said}"
        );
    }

    /// What arrives at watched lines asks for a report, through either half
    /// of the request, and drives nothing: a `gpio_in`'s one way in.
    #[test]
    fn what_arrives_at_watched_lines_asks_for_a_report() {
        let (mut writer, end) = on_a_pipe("switches", vec![0], 0b1, 0);
        let reader = writer.try_clone().expect("the other half");
        assert!(!reader.due(), "nothing asked yet");
        assert_eq!(writer.write(b"anything").expect("taken"), 8);
        assert!(reader.due(), "the other half has the report to make");

        close(end);
    }

    /// Several events waiting become several records in one read, on the
    /// stream world data goes by.
    ///
    /// Against a pipe in place of the request's descriptor, which is what
    /// the kernel's end of it looks like from here: whole events, one after
    /// another.
    #[test]
    fn every_event_waiting_becomes_a_record() {
        let (mut endpoint, end) = on_a_pipe("discretes", vec![3], 0b1, 0);

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
                    end,
                    &event as *const LineEvent as *const libc::c_void,
                    mem::size_of::<LineEvent>(),
                )
            };
            assert_eq!(wrote as usize, mem::size_of::<LineEvent>());
        }

        let mut buffer = [0u8; 64];
        let got = endpoint.read(&mut buffer).expect("the events are read");
        assert_eq!(got, 2 * EVENT_BYTES, "two events, two records");
        assert_eq!(endpoint.stream(), Some(Stream::Groundward));

        // The first is the rising edge and the second the falling one, in the
        // order they happened.
        assert_eq!(u32::from_be_bytes(buffer[8..12].try_into().unwrap()), 3);
        assert_eq!(u32::from_be_bytes(buffer[12..16].try_into().unwrap()), 1);
        assert_eq!(
            u32::from_be_bytes(
                buffer[EVENT_BYTES + 12..EVENT_BYTES + 16]
                    .try_into()
                    .unwrap()
            ),
            0
        );
        close(end);
    }

    /// A driven set of lines asks for the output flag and nothing else.
    ///
    /// None of the watching terms, which the configuration refuses to state
    /// for an output: an edge reported from a line this node drives would be
    /// this node hearing its own voice.
    #[test]
    fn driven_lines_ask_only_to_be_driven() {
        let config = GpioConfig {
            kind: tcslibgs::node::payload::gpio::Kind::Out,
            path: "/dev/gpiochip0".to_string(),
            inputs: Vec::new(),
            outputs: vec![17, 18, 19],
            // What the parser leaves here for a gpio_out, and what must not
            // reach the request.
            edge: LineEdge::Both,
            bias: Bias::None,
            debounce: None,
            state_interval: None,
            sim_address: None,
        };
        let request = request_for("heaters", &config, &config.lines())
            .expect("a request");

        assert_eq!(
            request.config.flags, GPIO_V2_LINE_FLAG_OUTPUT,
            "a driven line asks to be driven and nothing more"
        );
        assert_eq!(request.num_lines, 3);
        assert_eq!(&request.offsets[..3], &[17, 18, 19]);
        assert_eq!(request.config.num_attrs, 0);
    }

    /// The bits and the mask count lines by their place in the request, and
    /// only the driven lines are meant.
    #[test]
    fn the_values_are_indexed_by_place_in_the_request() {
        // Three lines asserted: the low three bits, whatever their offsets on
        // the chip are.
        let on = values_for(0b111, true);
        assert_eq!(on.mask, 0b111);
        assert_eq!(on.bits, 0b111);

        // And unasserted: the same lines meant, all of them low. A mask of
        // zero would be a call that said nothing.
        let off = values_for(0b111, false);
        assert_eq!(off.mask, 0b111);
        assert_eq!(off.bits, 0);

        // Driven lines among watched ones: the watched are not set.
        assert_eq!(values_for(0b1100, true).bits, 0b1100);

        // Which places a set of lines is at in the request.
        assert_eq!(mask_of(&[2, 3, 4, 5], &[5, 3]), 0b1010);
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

    /// Driven lines report nothing of their own: what is read of them is
    /// what they were driven to, passed on vehicleward, once each time it
    /// changes them.
    #[test]
    fn driven_lines_pass_on_what_they_were_driven_to() {
        let (mut reader, end) = on_a_pipe("heaters", vec![4], 0, 0b1);
        let mut buffer = [0u8; 64];
        assert_eq!(reader.read(&mut buffer).expect("nothing"), 0);
        assert!(!reader.due());

        // As a drive that reached the kernel leaves it: see write_on.
        reader.shared.driven.store(HIGH, Ordering::Release);
        reader.shared.passing.store(HIGH, Ordering::Release);
        assert!(reader.due());
        assert_eq!(reader.read(&mut buffer).expect("passed on"), 1);
        assert_eq!(&buffer[..1], b"1");
        assert_eq!(reader.stream(), Some(Stream::Vehicleward));
        assert!(!reader.due(), "passed on once");
        assert_eq!(reader.read(&mut buffer).expect("nothing"), 0);
        close(end);
    }

    /// A gpio_inout's lines report what they were last told on the report
    /// clock, groundward -- and nothing until they have been told something
    /// -- while what changes them is passed on vehicleward as a gpio_out's
    /// is, the one read first.
    #[test]
    fn a_gpio_inout_reports_what_its_lines_were_told() {
        let (mut reader, end) = on_a_pipe("heater", vec![4], 0, 0b1);
        reader.reports = true;
        reader.state_interval = Some(Duration::from_secs(60));
        reader.next_report = Some(Instant::now());
        let mut buffer = [0u8; 64];
        assert!(!reader.due(), "nothing told yet");

        // As a drive that reached the kernel leaves it: see write.
        reader.shared.driven.store(HIGH, Ordering::Release);
        reader.shared.passing.store(HIGH, Ordering::Release);
        assert_eq!(reader.read(&mut buffer).expect("passed on"), 1);
        assert_eq!(reader.stream(), Some(Stream::Vehicleward));
        assert_eq!(reader.read(&mut buffer).expect("reported"), 1);
        assert_eq!(&buffer[..1], b"1");
        assert_eq!(reader.stream(), Some(Stream::Groundward));

        // And not again until the clock comes round.
        assert!(!reader.due());
        assert_eq!(reader.read(&mut buffer).expect("nothing"), 0);
        close(end);
    }

    /// A write to a driven set reaches the kernel, which is as far as a test
    /// without a chip can see.
    ///
    /// A pipe is not a line request, so the ioctl is refused -- the point is
    /// that the refusal comes from the kernel and names the chip, rather than
    /// the endpoint taking the datum as a request for a report as watched
    /// lines would. Refused, nothing is passed on.
    #[test]
    fn a_write_to_driven_lines_is_attempted() {
        let (mut endpoint, end) = on_a_pipe("heaters", vec![4], 0, 0b1);

        let said = match endpoint.write(b"1") {
            Err(e) => format!("{e}"),
            Ok(n) => {
                panic!("a pipe took a line request's ioctl, and {n} bytes")
            }
        };
        assert!(
            said.contains("cannot drive") && said.contains("/dev/gpiochip0"),
            "the error should be the kernel's, and name the chip: {said}"
        );
        assert!(!endpoint.due(), "nothing to pass on from a failed drive");
        close(end);
    }

    /// tcssim's end of simulated lines: a socket on the loopback interface,
    /// and the lines' configuration at it.
    ///
    /// # Arguments
    ///
    /// * `config` - The lines, which are given the socket's address.
    ///
    /// # Returns
    ///
    /// The socket, waiting at most a second for what it is sent, and the
    /// configuration.
    fn simulated(mut config: GpioConfig) -> (UdpSocket, GpioConfig) {
        let tcssim = UdpSocket::bind("127.0.0.1:0").expect("a socket");
        tcssim
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("a timeout");
        config.sim_address = Some(tcssim.local_addr().expect("an address"));
        (tcssim, config)
    }

    /// Reads from an endpoint until something arrives, for at most a second:
    /// a datagram on the loopback interface is quick, but not instant.
    ///
    /// # Arguments
    ///
    /// * `endpoint` - The endpoint.
    ///
    /// # Returns
    ///
    /// What was read.
    fn read_some(endpoint: &mut GpioEndpoint) -> Vec<u8> {
        let mut buffer = [0u8; 256];
        let until = Instant::now() + Duration::from_secs(1);
        while Instant::now() < until {
            let n = endpoint.read(&mut buffer).expect("it reads");
            if n > 0 {
                return buffer[..n].to_vec();
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Vec::new()
    }

    /// Simulated watched lines say hello, take tcssim's first state as what
    /// they are rather than as a change, and make a record of each edge after
    /// it, as a chip's would; a datagram that is not a state is refused.
    #[test]
    fn simulated_watched_lines_make_the_records_a_chip_would() {
        let (tcssim, config) = simulated(watched(vec![3, 7], LineEdge::Both));
        let mut endpoint = GpioEndpoint::new("switches", &config)
            .expect("the lines are reached");
        let mut hello = [0u8; 8];
        let (n, vehicle) = tcssim.recv_from(&mut hello).expect("a hello");
        assert_eq!(n, 0, "the hello is empty");

        tcssim.send_to(b"00", vehicle).expect("sent");
        tcssim.send_to(b"10", vehicle).expect("sent");
        let read = read_some(&mut endpoint);
        assert_eq!(read.len(), EVENT_BYTES, "one edge, not the first state");
        assert_eq!(&read[8..12], &3u32.to_be_bytes());
        assert_eq!(&read[12..], &1u32.to_be_bytes());

        // Both lines change: line 3 falls and line 7 rises.
        tcssim.send_to(b"01", vehicle).expect("sent");
        let read = read_some(&mut endpoint);
        assert_eq!(read.len(), 2 * EVENT_BYTES);
        assert_eq!(&read[8..16], &[0, 0, 0, 3, 0, 0, 0, 0]);
        assert_eq!(&read[24..32], &[0, 0, 0, 7, 0, 0, 0, 1]);

        tcssim.send_to(b"2", vehicle).expect("sent");
        std::thread::sleep(Duration::from_millis(50));
        let mut buffer = [0u8; 256];
        let refused = endpoint.read(&mut buffer).expect_err("not a state");
        assert!(refused.to_string().contains("not a 1 or a 0"), "{refused}");
    }

    /// Simulated lines watched for rising edges make records of those alone,
    /// and a report gives the levels tcssim last said.
    #[test]
    fn simulated_lines_keep_to_their_edges_and_report_their_levels() {
        let (tcssim, config) = simulated(watched(vec![5], LineEdge::Rising));
        let mut endpoint = GpioEndpoint::new("switch", &config)
            .expect("the lines are reached");
        let (_, vehicle) = tcssim.recv_from(&mut [0u8; 8]).expect("a hello");

        for state in [b"0", b"1", b"0", b"1"] {
            tcssim.send_to(state, vehicle).expect("sent");
        }
        let mut records = Vec::new();
        let until = Instant::now() + Duration::from_millis(300);
        while Instant::now() < until {
            records.extend(read_some(&mut endpoint));
        }
        assert_eq!(records.len(), 2 * EVENT_BYTES, "the two rises alone");
        assert!(records
            .chunks(EVENT_BYTES)
            .all(|r| r[12..] == 1u32.to_be_bytes()));

        // Asked, as a datum at world-in asks: the level now.
        endpoint.shared.asked.store(true, Ordering::Release);
        let report = read_some(&mut endpoint);
        assert_eq!(report.len(), EVENT_BYTES);
        assert_eq!(&report[8..], &[0, 0, 0, 5, 0, 0, 0, 1]);
    }

    /// Simulated driven lines send tcssim what they are driven to, one
    /// character for each line, and pass on and report it as a chip's do.
    #[test]
    fn simulated_driven_lines_send_what_they_are_driven_to() {
        let config = GpioConfig {
            kind: tcslibgs::node::payload::gpio::Kind::Inout,
            path: "/dev/gpiochip1".to_string(),
            inputs: Vec::new(),
            outputs: vec![4, 5],
            edge: LineEdge::None,
            bias: Bias::None,
            debounce: None,
            state_interval: Some(Duration::from_secs(60)),
            sim_address: None,
        };
        let (tcssim, config) = simulated(config);
        let mut endpoint = GpioEndpoint::new("heaters", &config)
            .expect("the lines are reached");
        let mut said = [0u8; 8];
        assert_eq!(tcssim.recv(&mut said).expect("a hello"), 0);

        endpoint.write(b"1").expect("driven");
        let n = tcssim.recv(&mut said).expect("the drive");
        assert_eq!(&said[..n], b"11");

        let mut buffer = [0u8; 16];
        assert_eq!(endpoint.read(&mut buffer).expect("passed on"), 1);
        assert_eq!(
            (buffer[0], endpoint.stream()),
            (b'1', Some(Stream::Vehicleward))
        );
        assert_eq!(endpoint.read(&mut buffer).expect("reported"), 1);
        assert_eq!(
            (buffer[0], endpoint.stream()),
            (b'1', Some(Stream::Groundward))
        );

        endpoint.write(b"0").expect("driven");
        let n = tcssim.recv(&mut said).expect("the drive");
        assert_eq!(&said[..n], b"00");
    }
}
