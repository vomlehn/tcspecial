//! The GPIO endpoint: discrete lines, as one whole number a conduit can
//! carry.
//!
//! **Supported.** Run against the kernel's simulated chip, by
//! `tests/kernel-dummies.rs`, and against no hardware.
//!
//! This is the one endpoint whose device data does not exist until it is
//! made. Every other kind carries bytes somebody sent; a discrete line has no
//! bytes at all -- it is high or low. So what a set of lines is, is one
//! unsigned whole number, the narrowest of `u8`, `u16`, `u32` and `u64` with
//! a bit for each line, bit 0 the first line the node lists, most
//! significant byte first: see "Typed links" in the design.
//!
//! Whether the lines are watched or driven is the node's type -- see
//! `tcsprotocol::node::payload::gpio::Kind`. Watched lines report: what
//! they are goes out groundward, by `world-out`, every `state_interval`, and a
//! datum arriving at `world-in` asks for it now. Driven lines are set: what
//! arrives at `moc-in` is the value they are driven to, and a value that
//! changes them is passed on vehicleward, by `moc-out`, to whatever they drive.
//! A `gpio_inout`'s driven lines also report, groundward by `world-out` every
//! `state_interval`, the last value they were told.
//!
//! The reader and the writer are two halves of one request, each on a
//! duplicate of its descriptor, and what one is told the other has to say: a
//! report asked for through the writer is made by the reader, and a value
//! driven through the writer is passed on and reported by it. What they share
//! is made once, when the node is opened, and is three numbers -- see
//! [`Shared`] -- so that nothing is allocated as data passes.
//!
//! In a run that simulates them, a node that gives a `sim_address` has its
//! lines answered for by tcs-sim over UDP rather than by a chip: see
//! [`UdpLines`]. Everything above is the same either way -- the reports,
//! what is passed on -- and only where a level comes from and where a drive
//! goes differ.
//!
//! The kernel's side of a chip -- its structures and ioctls -- is
//! `tcsos::linux::gpio`'s, on Linux alone: elsewhere only simulated lines
//! can be had.

use std::io::ErrorKind;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tcsos::net::{SocketAddr, UdpSocket};
use tcsos::time::{Duration, Instant};
use tcsos::wait::{Ready, Source, Waitable};

#[cfg(target_os = "linux")]
use tcsprotocol::Bias;
use tcsprotocol::{
    graph::terminals::Stream,
    link_type::Basic,
    node::payload::gpio::{state_read, state_said},
    GpioConfig, TcsError, TcsResult,
};

use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable,
    WaitResult,
};

/// What lines are driven to, and which lines it is about: a bit of each
/// for each line by its place in the request rather than by its offset on
/// the chip -- lines 3, 4 and 5 of a chip are bits 0, 1 and 2 here.
#[derive(Debug, Default, Clone, Copy)]
struct LineValues {
    /// The values.
    bits: u64,
    /// Which lines they are of.
    mask: u64,
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
    /// What the lines were last driven to, a bit for each by its place,
    /// where they have been driven at all: see `told`.
    driven: AtomicU64,
    /// Whether the lines have been driven at all.
    told: AtomicBool,
    /// A value to pass on by `moc-out`: what the lines were driven to, where
    /// it changed them, and it has not been read -- see `passes`.
    passing: AtomicU64,
    /// Whether there is a value to pass on.
    passes: AtomicBool,
}

/// Where the lines are: a chip's, or tcs-sim's over UDP.
enum Lines {
    /// A chip's, through the kernel's GPIO interface: Linux alone.
    #[cfg(target_os = "linux")]
    Chip(tcsos::linux::gpio::Lines),
    /// tcs-sim's, at the node's `sim_address`.
    Udp(UdpLines),
}

/// Lines tcs-sim answers for over UDP, in a run that simulates them.
///
/// One datagram is one state of the lines: an ASCII `1` or `0` for each line
/// of the node, in the order of the request, lowest offset first -- `10` for
/// lines 4 and 5 with 4 asserted. The vehicle sends what it drives its lines
/// to; tcs-sim sends what watched lines are, whenever that changes. An empty
/// datagram from the vehicle, sent when it opens and again while it has heard
/// nothing, says where to answer.
///
/// What the kernel does for a chip is done here from the levels tcs-sim sends,
/// so that what goes out is what a chip's would be: the levels when a report
/// is due.
struct UdpLines {
    /// Connected to the `sim_address`, so that nothing else's datagrams are
    /// taken for the lines'.
    socket: UdpSocket,
    /// Where tcs-sim answers, for what a complaint says.
    at: SocketAddr,
    /// The levels tcs-sim last sent, which both halves of the node read.
    seen: Arc<Seen>,
}

/// What tcs-sim last said the lines are.
#[derive(Debug, Default)]
struct Seen {
    /// A bit for each line by its place in the request, set where it is
    /// asserted.
    levels: AtomicU64,
    /// Whether tcs-sim has said anything yet.
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

impl UdpLines {
    /// Reaches tcs-sim's lines at a node's `sim_address`, and says where to
    /// answer.
    ///
    /// # Arguments
    ///
    /// * `what` - The handler, for what a complaint says.
    /// * `at`   - The `sim_address`.
    ///
    /// # Returns
    ///
    /// The lines, on a non-blocking socket connected to `at`.
    ///
    /// # Errors
    ///
    /// `TcsError::Endpoint` if the socket cannot be made, connected or made
    /// non-blocking.
    fn open(what: &str, at: SocketAddr) -> TcsResult<Self> {
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
            seen: Arc::new(Seen::default()),
        };
        lines.hello();
        Ok(lines)
    }

    /// The same lines, on a duplicate of the socket, with what tcs-sim said
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
            seen: Arc::clone(&self.seen),
        })
    }

    /// Tells tcs-sim where to answer: an empty datagram. Lost if tcs-sim is
    /// not there yet, which is why it is said again until tcs-sim answers.
    fn hello(&self) {
        let _ = self.socket.send(&[]);
    }

    /// What the lines are, as tcs-sim last said.
    ///
    /// # Arguments
    ///
    /// * `mask` - Which lines, by place.
    ///
    /// # Returns
    ///
    /// A bit per line, set where it is asserted; none, until tcs-sim has said.
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

    /// Takes what tcs-sim has sent: the latest is what the lines are.
    ///
    /// # Arguments
    ///
    /// * `what`  - The handler, for what a complaint says.
    /// * `count` - How many lines the request has.
    ///
    /// # Errors
    ///
    /// `TcsError::Endpoint` for a datagram that is not a state of the lines;
    /// `TcsError::Io` for a socket that fails.
    fn take(&self, what: &str, count: usize) -> TcsResult<()> {
        let mut datagram = [0u8; LINES_MAX + 1];
        loop {
            let n = match self.socket.recv(&mut datagram) {
                Ok(n) => n,
                // Nothing yet, or tcs-sim not there yet: the hello was
                // refused, and is said again until it is answered.
                Err(ref e)
                    if matches!(
                        e.kind(),
                        ErrorKind::WouldBlock | ErrorKind::ConnectionRefused
                    ) =>
                {
                    return Ok(())
                }
                Err(e) => return Err(TcsError::Io(e)),
            };
            let now = state_read(count, &datagram[..n]).map_err(|why| {
                TcsError::Endpoint(format!("{what}: {} sent {why}", self.at))
            })?;
            self.seen.levels.store(now, Ordering::Release);
            self.seen.known.store(true, Ordering::Release);
        }
    }
}

impl Lines {
    /// What to wait on.
    ///
    /// # Returns
    ///
    /// The request's source, or the socket's.
    fn source(&self) -> Source {
        match self {
            #[cfg(target_os = "linux")]
            Lines::Chip(chip) => chip.source(),
            Lines::Udp(udp) => udp.socket.source(),
        }
    }

    /// The same lines, for the other half of the node.
    ///
    /// # Returns
    ///
    /// The lines, on duplicates of these descriptors.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if a descriptor cannot be duplicated.
    fn try_clone(&self) -> TcsResult<Self> {
        Ok(match self {
            #[cfg(target_os = "linux")]
            Lines::Chip(chip) => Lines::Chip(chip.try_clone()?),
            Lines::Udp(udp) => Lines::Udp(udp.try_clone()?),
        })
    }
}

impl GpioEndpoint {
    /// Asks the chip for the configured lines: the watched ones with edge
    /// detection where it is asked for, and the driven ones as outputs. Or,
    /// where the run simulates them, reaches tcs-sim's at the node's
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
        if offsets.is_empty() || offsets.len() > LINES_MAX {
            return Err(TcsError::Config(format!(
                "{what}: {} lines is not between 1 and {LINES_MAX}",
                offsets.len()
            )));
        }
        let lines = match config.sim_address {
            Some(at) => Lines::Udp(UdpLines::open(what, at)?),
            None => on_the_chip(what, config, &offsets)?,
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
            && self.shared.told.load(Ordering::Acquire)
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

/// The most lines one request may hold: the kernel's limit, which the
/// simulated lines keep to as well, so that a configuration a desk runs is
/// one a vehicle can.
const LINES_MAX: usize = 64;

/// How the kernel is to set a node's lines.
///
/// Driven lines ask for none of the watching terms, which the
/// configuration refuses to state for them: an output has nothing to hold
/// or debounce. Watched lines are asked for as plain inputs, with no edge
/// detection: what they are is taken when a report is due, and nothing else
/// is read. The debounce, where the file asks for one, applies to every
/// watched line, a bit for each.
///
/// # Arguments
///
/// * `config`  - The node's configuration.
/// * `offsets` - Every line of the request, in its order.
///
/// # Returns
///
/// The settings.
#[cfg(target_os = "linux")]
fn settings_for(
    config: &GpioConfig,
    offsets: &[u32],
) -> tcsos::linux::gpio::Settings {
    use tcsos::linux::gpio::{Bias as Pull, Settings};

    if config.inputs.is_empty() {
        return Settings::Output;
    }
    Settings::Input {
        bias: match config.bias {
            Bias::None => Pull::None,
            Bias::PullUp => Pull::PullUp,
            Bias::PullDown => Pull::PullDown,
        },
        debounce: config
            .debounce
            .map(|steady| (steady, mask_of(offsets, &config.inputs))),
    }
}

/// Asks the chip for the node's lines.
///
/// # Arguments
///
/// * `what`    - The handler, for what a complaint says, and whose the
///               lines are held as.
/// * `config`  - The node's configuration.
/// * `offsets` - Every line of the request, in its order.
///
/// # Returns
///
/// The lines.
///
/// # Errors
///
/// `TcsError::Endpoint` if the chip cannot be opened, refuses the request
/// or gives no descriptor, or the descriptor cannot be made non-blocking.
#[cfg(target_os = "linux")]
fn on_the_chip(
    what: &str,
    config: &GpioConfig,
    offsets: &[u32],
) -> TcsResult<Lines> {
    use tcsos::linux::gpio::Refused;

    let path = &config.path;
    let settings = settings_for(config, offsets);
    tcsos::linux::gpio::Lines::request(path, offsets, what, &settings)
        .map(Lines::Chip)
        .map_err(|refused| {
            TcsError::Endpoint(match refused {
                Refused::HowMany(n) => {
                    format!(
                        "{what}: {n} lines is not between 1 and {LINES_MAX}"
                    )
                }
                Refused::Open(e) => {
                    format!("{what}: cannot open the GPIO chip {path}: {e}")
                }
                Refused::Request(e) => format!(
                    "{what}: {path} will not give up its lines: {e}. A line \
                     is given to one request at a time, so this is commonly \
                     a line another program holds"
                ),
                Refused::NoDescriptor => format!(
                    "{what}: {path} took the request and gave no descriptor"
                ),
                Refused::NonBlocking(e) => format!(
                    "{what}: cannot make {path}'s line request non-blocking: \
                     {e}"
                ),
            })
        })
}

/// Where the system has no GPIO chips to ask -- a web page -- lines that
/// are not simulated cannot be had.
///
/// # Arguments
///
/// * `what`     - The handler, for what a complaint says.
/// * `config`   - The node's configuration.
/// * `_offsets` - Every line of the request, in its order.
///
/// # Returns
///
/// Never.
///
/// # Errors
///
/// Always `TcsError::Endpoint`, saying that only simulated lines can be.
#[cfg(not(target_os = "linux"))]
fn on_the_chip(
    what: &str,
    config: &GpioConfig,
    _offsets: &[u32],
) -> TcsResult<Lines> {
    Err(TcsError::Endpoint(format!(
        "{what}: the GPIO chip {} cannot be reached here, where only \
         simulated lines can be: run with --simulate",
        config.path
    )))
}

/// Reads what arrived as the number the lines are to be driven to.
///
/// One unsigned whole number, the narrowest of `u8`, `u16`, `u32` and `u64`
/// with a bit for each line, most significant byte first: bit 0 is the first
/// line the node lists, bit 1 the next. A bit set where the node has no line
/// is refused, and the lines are left as they were: the bit names no line,
/// so driving from it would be guessing what was meant. See "GPIO lines" in
/// the design.
///
/// # Arguments
///
/// * `what`  - The handler, for what a complaint says.
/// * `count` - How many lines the node has.
/// * `data`  - What arrived.
///
/// # Returns
///
/// The lines' bits, by their places.
///
/// # Errors
///
/// `TcsError::Endpoint` if `data` is not that number's width, or sets a bit
/// beyond the node's lines.
fn lines_read(what: &str, count: usize, data: &[u8]) -> TcsResult<u64> {
    let width = lines_width(count);
    if data.len() != width {
        return Err(TcsError::Endpoint(format!(
            "{what}: {count} lines are driven with a {} of {width} bytes, and \
             {} arrived",
            Basic::lines(count).name(),
            data.len()
        )));
    }
    let mut bytes = [0u8; 8];
    bytes[8 - width..].copy_from_slice(data);
    let bits = u64::from_be_bytes(bytes);
    if count < 64 && bits >> count != 0 {
        return Err(TcsError::Endpoint(format!(
            "{what}: {bits:#x} sets a bit beyond its {count} lines, which no \
             line answers to: refused, the lines left as they were"
        )));
    }
    Ok(bits)
}

/// Writes a set of lines' number, as [`lines_read`] reads it.
///
/// # Arguments
///
/// * `count`  - How many lines the node has.
/// * `bits`   - The lines' bits, by their places.
/// * `buffer` - Where it is written, with room for the number's width.
///
/// # Returns
///
/// How many bytes were written: the number's width.
fn lines_said(count: usize, bits: u64, buffer: &mut [u8]) -> usize {
    let width = lines_width(count);
    buffer[..width].copy_from_slice(&bits.to_be_bytes()[8 - width..]);
    width
}

/// Says how many bytes a set of lines' number is.
///
/// # Arguments
///
/// * `count` - How many lines.
///
/// # Returns
///
/// The width of the narrowest unsigned whole number with a bit for each.
fn lines_width(count: usize) -> usize {
    Basic::lines(count)
        .width()
        .expect("a set of lines is a whole number")
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
fn values_for(driven: u64, wanted: u64) -> LineValues {
    LineValues {
        bits: wanted & driven,
        mask: driven,
    }
}

impl EndpointWaitable for GpioEndpoint {
    fn io_source(&self) -> Source {
        self.lines.source()
    }

    fn wait_for_event(
        &self,
        cmd: Source,
        timeout_ms: i32,
    ) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_source(), cmd, Ready::READ, timeout_ms)
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
            || self.shared.passes.load(Ordering::Acquire)
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
        // What the driven lines were set to, passed on as the number it
        // was, which is what whatever the lines drive reads.
        let count = self.offsets.len();
        // Room for the lines' number, as wide as they need and no wider: the
        // buffer is sized by the node's type, a byte for up to eight lines.
        let room = buffer.len() >= lines_width(count);
        if room && self.shared.passes.swap(false, Ordering::AcqRel) {
            let passing = self.shared.passing.load(Ordering::Acquire);
            self.last = Some(Stream::Vehicleward);
            return Ok(lines_said(count, passing, buffer));
        }
        self.last = Some(Stream::Groundward);

        // What a gpio_inout's lines were last told, as the decision it was,
        // on the report clock: the ground follows a heater's switch on a
        // beat, and a report lost is made good by the next.
        if self.told_due() && room {
            let told = self.shared.driven.load(Ordering::Acquire);
            self.wind();
            return Ok(lines_said(count, told, buffer));
        }
        if self.watched == 0 {
            return Ok(0);
        }

        // What tcs-sim has said of simulated lines, taken whether or not a
        // report is due, so that what is reported is the latest. Where a
        // chip cannot be had, every set of lines is simulated.
        #[cfg_attr(not(target_os = "linux"), allow(irrefutable_let_patterns))]
        if let Lines::Udp(udp) = &self.lines {
            udp.take(&self.what, count)?;
        }

        // What the lines are, where the report is due by the clock or a
        // datum at world-in has asked for it.
        //
        // The clock runs from when the report was due: a vehicle busy enough
        // to be late with one is a vehicle that should not then be late with
        // every one after it. A report missed by more than an interval --
        // which a long pause would do -- is not made up for; what the ground
        // wants is what the lines are now. One asked for leaves the clock as
        // it was.
        if !self.report_due() || !room {
            return Ok(0);
        }
        self.shared.asked.store(false, Ordering::Release);
        let bits = match &self.lines {
            #[cfg(target_os = "linux")]
            Lines::Chip(chip) => chip.levels(self.watched).map_err(|e| {
                TcsError::Endpoint(format!(
                    "{}: cannot read what the lines are: {e}",
                    self.what
                ))
            })?,
            Lines::Udp(udp) => udp.levels(self.watched),
        };
        self.wind();
        Ok(lines_said(count, bits, buffer))
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

        let wanted = lines_read(&self.what, self.offsets.len(), data)?;
        let values = values_for(self.driven, wanted);

        match &self.lines {
            #[cfg(target_os = "linux")]
            Lines::Chip(chip) => {
                chip.drive(values.bits, values.mask).map_err(|e| {
                    TcsError::Endpoint(format!(
                        "{}: cannot drive the lines of {}: {e}",
                        self.what, self.path
                    ))
                })?
            }
            Lines::Udp(udp) => {
                udp.drive(&self.what, self.offsets.len(), &values)?
            }
        }

        let now = values.bits;
        let was = self.shared.driven.swap(now, Ordering::AcqRel);
        if !self.shared.told.swap(true, Ordering::AcqRel) || was != now {
            self.shared.passing.store(now, Ordering::Release);
            self.shared.passes.store(true, Ordering::Release);
        }

        // The whole of what arrived, which is what a caller counts as sent.
        Ok(data.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lines watched, as a file's `gpio_in` states them.
    ///
    /// # Arguments
    ///
    /// * `lines` - The offsets.
    ///
    /// # Returns
    ///
    /// The lines' configuration, on `/dev/gpiochip0`, reported every minute.
    fn watched(lines: Vec<u32>) -> GpioConfig {
        GpioConfig {
            kind: tcsprotocol::node::payload::gpio::Kind::In,
            path: "/dev/gpiochip0".to_string(),
            inputs: lines,
            outputs: Vec::new(),
            bias: Bias::None,
            debounce: None,
            state_interval: Some(Duration::from_secs(60)),
            sim_address: None,
        }
    }

    /// An endpoint on a pipe in place of a chip and a request, for what can
    /// be tested without one: what is refused, what is asked for, and what
    /// is passed on.
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
    /// The endpoint, and the write end of the pipe its lines read, held
    /// for as long as the test needs the pipe open.
    fn on_a_pipe(
        what: &str,
        offsets: Vec<u32>,
        watched: u64,
        driven: u64,
    ) -> (GpioEndpoint, tcsos::fs::File) {
        let (lines, end) =
            tcsos::linux::gpio::Lines::on_a_pipe().expect("a pipe");
        let endpoint = GpioEndpoint {
            what: what.to_string(),
            lines: Lines::Chip(lines),
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
        (endpoint, end)
    }

    /// Lines watched are asked for as inputs, with the bias the file asked
    /// for, and the debounce on every watched line.
    #[test]
    fn watched_lines_are_asked_for_as_the_file_said() {
        use tcsos::linux::gpio::{Bias as Pull, Settings};

        let config = GpioConfig {
            bias: Bias::PullUp,
            debounce: Some(Duration::from_micros(5_000)),
            ..watched(vec![3, 7, 11])
        };
        assert_eq!(
            settings_for(&config, &config.lines()),
            Settings::Input {
                bias: Pull::PullUp,
                debounce: Some((Duration::from_micros(5_000), 0b111)),
            }
        );
    }

    /// A chip that is not there is reported.
    #[test]
    fn a_chip_that_is_not_there_is_reported() {
        let config = GpioConfig {
            path: "/dev/gpiochip-nonesuch".to_string(),
            ..watched(vec![0])
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

        drop(end);
    }

    /// A report gives what the lines are as one number, a bit for each by
    /// its place, on the stream world data goes by -- here asked for, as a
    /// datum at world-in asks.
    ///
    /// Against a pipe in place of the request's descriptor, which the kernel
    /// refuses a values ioctl on: the refusal names the chip, which is as far
    /// as a test without a chip can see.
    #[test]
    fn a_report_is_asked_of_the_chip() {
        let (mut endpoint, end) = on_a_pipe("discretes", vec![3], 0b1, 0);
        endpoint.shared.asked.store(true, Ordering::Release);
        let mut buffer = [0u8; 64];
        let said = match endpoint.read(&mut buffer) {
            Err(e) => format!("{e}"),
            Ok(n) => panic!("a pipe gave the lines' values, {n} bytes"),
        };
        assert!(said.contains("discretes"), "{said}");
        drop(end);
    }

    /// A driven set of lines asks for the output flag and nothing else.
    ///
    /// None of the watching terms, which the configuration refuses to state
    /// for an output: a line this node drives has nothing to hold or
    /// debounce.
    #[test]
    fn driven_lines_ask_only_to_be_driven() {
        let config = GpioConfig {
            kind: tcsprotocol::node::payload::gpio::Kind::Out,
            path: "/dev/gpiochip0".to_string(),
            inputs: Vec::new(),
            outputs: vec![17, 18, 19],
            // What the parser leaves here for a gpio_out, and what must not
            // reach the request.
            bias: Bias::None,
            debounce: None,
            state_interval: None,
            sim_address: None,
        };
        assert_eq!(
            settings_for(&config, &config.lines()),
            tcsos::linux::gpio::Settings::Output,
            "a driven line asks to be driven and nothing more"
        );
    }

    /// The bits and the mask count lines by their place in the request, and
    /// only the driven lines are meant.
    #[test]
    fn the_values_are_indexed_by_place_in_the_request() {
        // Three lines asserted: the low three bits, whatever their offsets on
        // the chip are.
        let on = values_for(0b111, 0b111);
        assert_eq!(on.mask, 0b111);
        assert_eq!(on.bits, 0b111);

        // And unasserted: the same lines meant, all of them low. A mask of
        // zero would be a call that said nothing.
        let off = values_for(0b111, 0);
        assert_eq!(off.mask, 0b111);
        assert_eq!(off.bits, 0);

        // Some lines of the three, by their bits.
        assert_eq!(values_for(0b111, 0b101).bits, 0b101);

        // Driven lines among watched ones: the watched are not set.
        assert_eq!(values_for(0b1100, 0b1111).bits, 0b1100);

        // Which places a set of lines is at in the request.
        assert_eq!(mask_of(&[2, 3, 4, 5], &[5, 3]), 0b1010);
    }

    /// What drives a set of lines is one number with a bit per line, as
    /// wide as the narrowest type with room for them, and nothing else.
    #[test]
    fn lines_are_driven_with_a_number_of_their_width() {
        assert_eq!(lines_read("heaters", 3, &[0b101]).expect("a u8"), 0b101);
        assert_eq!(lines_read("heater", 1, &[1]).expect("one line"), 1);
        assert_eq!(
            lines_read("bank", 12, &[0x0f, 0xff]).expect("a u16"),
            0x0fff
        );
        assert_eq!(lines_read("all", 64, &[0xff; 8]).expect("a u64"), u64::MAX);

        for (count, refused) in [
            (3, &[0b1000][..]),
            (1, &[2]),
            (3, &[0, 1]),
            (12, &[1]),
            (3, &[]),
            (3, b"1"),
        ] {
            let said = match lines_read("heaters", count, refused) {
                Err(e) => format!("{e}"),
                Ok(bits) => panic!("{refused:?} was taken for {bits:#b}"),
            };
            assert!(
                said.contains("heaters"),
                "the refusal should name the node: {said}"
            );
        }

        let mut buffer = [0u8; 8];
        assert_eq!(lines_said(3, 0b101, &mut buffer), 1);
        assert_eq!(buffer[0], 0b101);
        assert_eq!(lines_said(12, 0x0fff, &mut buffer), 2);
        assert_eq!(&buffer[..2], &[0x0f, 0xff]);
    }

    /// Leaves an endpoint as a drive that reached the kernel leaves it.
    ///
    /// # Arguments
    ///
    /// * `reader` - The endpoint.
    /// * `bits`   - What its lines were driven to.
    fn drove(reader: &GpioEndpoint, bits: u64) {
        reader.shared.driven.store(bits, Ordering::Release);
        reader.shared.told.store(true, Ordering::Release);
        reader.shared.passing.store(bits, Ordering::Release);
        reader.shared.passes.store(true, Ordering::Release);
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

        // As a drive that reached the kernel leaves it: see write.
        drove(&reader, 1);
        assert!(reader.due());
        assert_eq!(reader.read(&mut buffer).expect("passed on"), 1);
        assert_eq!(&buffer[..1], &[1]);
        assert_eq!(reader.stream(), Some(Stream::Vehicleward));
        assert!(!reader.due(), "passed on once");
        assert_eq!(reader.read(&mut buffer).expect("nothing"), 0);
        drop(end);
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
        drove(&reader, 1);
        assert_eq!(reader.read(&mut buffer).expect("passed on"), 1);
        assert_eq!(reader.stream(), Some(Stream::Vehicleward));
        assert_eq!(reader.read(&mut buffer).expect("reported"), 1);
        assert_eq!(&buffer[..1], &[1]);
        assert_eq!(reader.stream(), Some(Stream::Groundward));

        // And not again until the clock comes round.
        assert!(!reader.due());
        assert_eq!(reader.read(&mut buffer).expect("nothing"), 0);
        drop(end);
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

        let said = match endpoint.write(&[1]) {
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
        drop(end);
    }

    /// tcs-sim's end of simulated lines: a socket on the loopback interface,
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
        let tcs_sim = UdpSocket::bind("127.0.0.1:0").expect("a socket");
        tcs_sim
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("a timeout");
        config.sim_address = Some(tcs_sim.local_addr().expect("an address"));
        (tcs_sim, config)
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
            tcsos::thread::sleep(Duration::from_millis(5));
        }
        Vec::new()
    }

    /// Simulated watched lines say hello, keep the last state tcs-sim sends,
    /// and report it as one number when asked or when the report is due; a
    /// datagram that is not a state is refused.
    #[test]
    fn simulated_watched_lines_report_what_tcs_sim_last_said() {
        let (tcs_sim, config) = simulated(watched(vec![3, 7]));
        let mut endpoint = GpioEndpoint::new("switches", &config)
            .expect("the lines are reached");
        let mut hello = [0u8; 8];
        let (n, vehicle) = tcs_sim.recv_from(&mut hello).expect("a hello");
        assert_eq!(n, 0, "the hello is empty");

        // The first report is due as the node opens: the latest state, line
        // 3 high -- bit 0 of a u8 -- and not the one before it.
        tcs_sim.send_to(b"00", vehicle).expect("sent");
        tcs_sim.send_to(b"10", vehicle).expect("sent");
        tcsos::thread::sleep(Duration::from_millis(50));
        assert_eq!(read_some(&mut endpoint), vec![0b01]);
        assert_eq!(endpoint.stream(), Some(Stream::Groundward));

        // Then nothing until the next is due or one is asked for.
        let mut buffer = [0u8; 16];
        assert_eq!(endpoint.read(&mut buffer).expect("nothing due"), 0);

        // Line 3 falls and line 7 rises: bit 1.
        tcs_sim.send_to(b"01", vehicle).expect("sent");
        tcsos::thread::sleep(Duration::from_millis(50));
        endpoint.shared.asked.store(true, Ordering::Release);
        assert_eq!(read_some(&mut endpoint), vec![0b10]);

        tcs_sim.send_to(b"2", vehicle).expect("sent");
        tcsos::thread::sleep(Duration::from_millis(50));
        let refused = endpoint.read(&mut buffer).expect_err("not a state");
        assert!(refused.to_string().contains("not a 1 or a 0"), "{refused}");
    }

    /// Simulated driven lines send tcs-sim what they are driven to, one
    /// character for each line, and pass on and report it as a chip's do.
    #[test]
    fn simulated_driven_lines_send_what_they_are_driven_to() {
        let config = GpioConfig {
            kind: tcsprotocol::node::payload::gpio::Kind::Inout,
            path: "/dev/gpiochip1".to_string(),
            inputs: Vec::new(),
            outputs: vec![4, 5],
            bias: Bias::None,
            debounce: None,
            state_interval: Some(Duration::from_secs(60)),
            sim_address: None,
        };
        let (tcs_sim, config) = simulated(config);
        let mut endpoint = GpioEndpoint::new("heaters", &config)
            .expect("the lines are reached");
        let mut said = [0u8; 8];
        assert_eq!(tcs_sim.recv(&mut said).expect("a hello"), 0);

        // Both lines on: bits 0 and 1, which tcs-sim is sent a character
        // each for.
        endpoint.write(&[0b11]).expect("driven");
        let n = tcs_sim.recv(&mut said).expect("the drive");
        assert_eq!(&said[..n], b"11");

        let mut buffer = [0u8; 16];
        assert_eq!(endpoint.read(&mut buffer).expect("passed on"), 1);
        assert_eq!(
            (buffer[0], endpoint.stream()),
            (0b11, Some(Stream::Vehicleward))
        );
        assert_eq!(endpoint.read(&mut buffer).expect("reported"), 1);
        assert_eq!(
            (buffer[0], endpoint.stream()),
            (0b11, Some(Stream::Groundward))
        );

        endpoint.write(&[0]).expect("driven");
        let n = tcs_sim.recv(&mut said).expect("the drive");
        assert_eq!(&said[..n], b"00");
    }
}
