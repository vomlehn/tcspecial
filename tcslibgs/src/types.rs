//! Type definitions shared between ground and space software

use serde::{Deserialize, Serialize};

use crate::node::payload::hdlc::{ClockType, Encoding, FrameCheck};
use crate::node::payload::serial::{Parity, StopBits};
use crate::node::payload::spi::{BitOrder, CsActive, SpiMode};
use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A time, in microseconds since the UNIX epoch.
///
/// One number, one unit, everywhere a time is carried: in telemetry headers,
/// in a handler's samples and statistics, and in the times a program reads
/// off its own clock. Two ends comparing times, or a log read later, then
/// have nothing to agree about beyond the epoch.
///
/// On the wire it is eight bytes, little-endian, and all ones says the
/// spacecraft could not tell the time; see [`Timestamp::CLOCK_FAULT`].
///
/// Eight bytes because four will not do. Four bytes of microseconds run out
/// 4294.967295 seconds after the epoch -- 1970-01-01 01:11:34 UTC -- so a
/// four-byte field would have overflowed before this software was first
/// written. Eight reach 586524-01-19, which is long enough for anything that
/// flies.
///
/// Microseconds rather than the milliseconds this was. A payload that is
/// triggered and read a fraction of a millisecond later is two transfers a
/// millisecond clock stamps with one time, and the ground cannot then tell
/// which happened first; microseconds are finer than any interval here is
/// written in and still leave the range above.
///
/// Before the milliseconds it was a pair -- seconds and nanoseconds within
/// the second -- which is two fields to carry, two to serialize, and two to
/// get wrong: arithmetic on it had to borrow across the second, and the
/// nanoseconds were never shown or read by anything.
#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct Timestamp {
    /// Microseconds since the UNIX epoch, or all ones for a clock that could
    /// not be read: [`Timestamp::CLOCK_FAULT`].
    pub micros: u64,
}

impl std::fmt::Display for Timestamp {
    /// The microseconds, which is what the value is.
    ///
    /// A time of day is [`Timestamp::time_of_day`]; this is for a message
    /// that wants the number -- a test saying which times it bracketed, a log
    /// line that will be read against another program's.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.clock_failed() {
            return write!(f, "no clock");
        }
        write!(f, "{} us", self.micros)
    }
}

impl Timestamp {
    /// What the spacecraft sends when it cannot tell the time: all ones.
    ///
    /// A clock nobody has set, one set to before the epoch, or one reading so
    /// far ahead that the count will not fit. The ground is told that instead
    /// of being given a number, because a time nobody can vouch for looks
    /// exactly like a time somebody can: a log full of 1970 reads as data
    /// from the epoch rather than as a clock that was never set, and every
    /// interval computed from it reads as fifty years.
    ///
    /// All ones rather than nought, because nought is a real time -- the
    /// epoch itself -- and a field of them is a time the ground might
    /// believe.
    ///
    /// It is the largest value, so a time that cannot be told sorts after
    /// every time that can. That is the safe direction for the comparisons
    /// here: [`Timestamp::since`] refuses it outright, and a caller taking
    /// the later of two times gets the fault and so gets no answer, rather
    /// than quietly getting a real time and believing it.
    pub const CLOCK_FAULT: Self = Self { micros: u64::MAX };

    /// Create a new timestamp from the current system time
    pub fn now() -> Self {
        Self::at(SystemTime::now())
    }

    /// When a system clock reading happened, as a timestamp.
    ///
    /// For the times a program takes from its own clock rather than from
    /// telemetry -- a beacon's arrival, say -- so that they are shown by the
    /// one function that shows every other time here.
    ///
    /// A reading before the epoch, or one too far past it to count in
    /// microseconds, is [`Timestamp::CLOCK_FAULT`]. A clock set that wrongly
    /// is a fault in its own right, and saying so is better both than a
    /// panic in a program watching a spacecraft and than the epoch, which is
    /// a time the ground would take for a real one.
    pub fn at(time: SystemTime) -> Self {
        let Ok(since) = time.duration_since(UNIX_EPOCH) else {
            return Self::CLOCK_FAULT;
        };
        match u64::try_from(since.as_micros()) {
            Ok(micros) if micros != Self::CLOCK_FAULT.micros => Self { micros },
            _ => Self::CLOCK_FAULT,
        }
    }

    /// This time in microseconds since the epoch, which is what it is.
    pub fn micros(&self) -> u64 {
        self.micros
    }

    /// Whether this is a clock that could not be read.
    ///
    /// Asked before a time is used for anything but showing it: see
    /// [`Timestamp::CLOCK_FAULT`].
    pub fn clock_failed(&self) -> bool {
        *self == Self::CLOCK_FAULT
    }

    /// How long after `earlier` this time is, or `None` if it is not after it.
    ///
    /// The one way an elapsed time is taken from two timestamps, so that no
    /// caller has to know the unit. `None` for a clock fault at either end as
    /// well as for this time being the earlier of the two: an interval
    /// measured against a clock that could not be read is a number with
    /// nothing behind it, and a time from the future is a clock that moved.
    /// Both are "cannot say", which is what every caller here does with the
    /// absence.
    pub fn since(&self, earlier: Timestamp) -> Option<Duration> {
        if self.clock_failed() || earlier.clock_failed() {
            return None;
        }
        self.micros
            .checked_sub(earlier.micros)
            .map(Duration::from_micros)
    }

    /// This time as it goes on the wire: eight bytes, little-endian.
    ///
    /// Little-endian because both ends run on little-endian machines, so
    /// network order would be a byte swap at each of them to agree with
    /// nothing, and because the records a payload reads are written that way
    /// already -- the GPIO node's, for one.
    ///
    /// Here rather than at each place that sends a time, so that the format
    /// is written down once and the test below is the whole of what holds it.
    pub fn to_le_bytes(&self) -> [u8; 8] {
        self.micros.to_le_bytes()
    }

    /// A time read off the wire, those eight bytes back again.
    ///
    /// All ones arrives as [`Timestamp::CLOCK_FAULT`] and is not rejected
    /// here: it is a value the spacecraft means to send, and what to do about
    /// it belongs to whatever was going to use the time.
    pub fn from_le_bytes(bytes: [u8; 8]) -> Self {
        Self {
            micros: u64::from_le_bytes(bytes),
        }
    }

    /// The time of day this names, as a panel shows it.
    ///
    /// UTC, and the date dropped: a panel has room for a time and wants it to
    /// be read at a glance, and every program here shows it the same way --
    /// tcsmoc's panels and tcssim's are read side by side while a link is
    /// being watched, so one of them showing local time would make a transfer
    /// look an hour old.
    ///
    /// A clock that could not be read shows [`NO_TRANSFER_TIME`], which is
    /// the shape every panel here already has for a time it cannot show.
    ///
    /// The fractions of a second are deliberately unused. A panel refreshed
    /// twice a second cannot show them, and a time that changed in its last
    /// digits between two looks would read as traffic that had not happened.
    pub fn time_of_day(&self) -> String {
        if self.clock_failed() {
            return NO_TRANSFER_TIME.to_string();
        }
        let within_a_day = (self.micros / 1_000_000) % 86_400;
        format!(
            "{:02}:{:02}:{:02}",
            within_a_day / 3600,
            (within_a_day % 3600) / 60,
            within_a_day % 60
        )
    }
}

/// Data handler type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DHType {
    /// A handler whose payload is at a network address.
    Network,
    /// A handler whose payload is a device file, read as it comes.
    Device,
    /// A handler whose payload is on a serial line.
    Serial,
    /// A handler whose payload is a device on an I2C bus.
    I2c,
    /// A handler whose payload is a SPI peripheral.
    Spi,
    /// A handler whose payload is on a CAN bus.
    ///
    /// Experimental; see [`crate::node::payload::can`].
    Can,
    /// A handler whose payload is a sensor's buffered samples.
    ///
    /// Experimental; see [`crate::node::payload::iio`].
    Iio,
    /// A handler whose payload is software on another core, reached over an
    /// rpmsg channel.
    ///
    /// Experimental; see [`crate::node::payload::rpmsg`].
    Rpmsg,
    /// A handler whose payload is a set of discrete lines.
    ///
    /// Experimental; see [`crate::node::payload::gpio`].
    Gpio,
    /// A handler whose payload is on a synchronous serial line, which Linux
    /// drives as a network interface.
    ///
    /// Experimental; see [`crate::node::payload::hdlc`].
    Hdlc,
}

// The rule about two nodes wanting one thing is `graph::check`'s. It was here
// as well, over the handlers a payload section produced: a `Claim`, a
// `claims_of` and a `no_two_handlers_claim_one_thing`, which nothing called
// once the rule moved to the graph. One copy, in the one place a configuration
// is checked -- two would be two answers to the question of what a bus shares.
// `one_host` stays here, because two files naming one host differently is
// asked about outside that rule too.

/// The names that mean this host, as one name.
///
/// Only the names that are certainly this host. Two names for some other host
/// -- a hostname and the address it resolves to -- are left as written, since
/// resolving them is a question for the network and not for a file.
///
/// Public because two files naming one host differently is the same question as
/// two handlers doing so: a simulator configuration stating `localhost` for a
/// payload the tcspecial configuration put at `127.0.0.1` has not disagreed
/// with it.
pub fn one_host(address: &str) -> String {
    match address {
        "localhost" | "127.0.0.1" | "::1" => "localhost".to_string(),
        other => other.to_string(),
    }
}

/// Whether a datagram came from the socket a configuration names.
///
/// Every link in the `tcspecial` section is written down at both ends, which
/// is what makes this possible at all: a beacon, a command, a response and a
/// payload's telemetry each have one place they are supposed to come from, and
/// anything else arriving at that socket is somebody else's traffic or
/// somebody's mistake. Acting on it would be acting on what the configuration
/// did not describe.
///
/// The port must match. The address must too -- `localhost`, `127.0.0.1` and
/// `::1` counting as one host, as they do everywhere else in these rules --
/// unless the configuration names every interface, in which case the address a
/// datagram carries is whichever interface the route chose and there is
/// nothing written down to compare it with.
///
/// Asked of every datagram on those links, so it allocates nothing: the
/// address a datagram came from is written into a buffer on the stack and
/// compared as text, by the same rule [`one_host`] states.
pub fn came_from(stated: &HostPort, from: &SocketAddr) -> bool {
    if from.port() != stated.port {
        return false;
    }
    if binds_every_interface(&stated.address) {
        return true;
    }

    // The longest an IP address is written: an IPv6 address with an IPv4
    // tail, which is 45 characters.
    let mut text = [0u8; 46];
    let Some(len) = written(&from.ip(), &mut text) else {
        return false;
    };
    let from = std::str::from_utf8(&text[..len]).unwrap_or("");
    let this_host =
        |name: &str| matches!(name, "localhost" | "127.0.0.1" | "::1");
    if this_host(&stated.address) {
        this_host(from)
    } else {
        stated.address == from
    }
}

/// `value` written into `out`, as `Display` writes it: the bytes it took, or
/// `None` where it does not fit.
fn written(value: &impl std::fmt::Display, out: &mut [u8]) -> Option<usize> {
    use std::fmt::Write;

    struct Into<'a> {
        out: &'a mut [u8],
        at: usize,
    }

    impl Write for Into<'_> {
        fn write_str(&mut self, said: &str) -> std::fmt::Result {
            let end = self.at + said.len();
            let room = self.out.get_mut(self.at..end).ok_or(std::fmt::Error)?;
            room.copy_from_slice(said.as_bytes());
            self.at = end;
            Ok(())
        }
    }

    let mut into = Into { out, at: 0 };
    write!(into, "{value}").ok()?;
    Some(into.at)
}

/// Whether an address binds every interface rather than one of them.
///
/// A wildcard claims a port on every address at once, which is why it has to
/// be told from a host: a handler at `localhost:4000` and a command link at
/// `0.0.0.0:4000` do collide, without naming one host between them.
fn binds_every_interface(address: &str) -> bool {
    address.is_empty()
        || address == "0.0.0.0"
        || address == "::"
        || address == "*"
}

impl DHType {
    /// Every kind, in the order [`DHType::spellings`] lists them: the
    /// interfaces a payload node may be reached over, and the kinds a
    /// simulator file may stand in for.
    pub const ALL: [DHType; 10] = [
        DHType::Network,
        DHType::Device,
        DHType::Serial,
        DHType::I2c,
        DHType::Spi,
        DHType::Can,
        DHType::Iio,
        DHType::Rpmsg,
        DHType::Gpio,
        DHType::Hdlc,
    ];

    /// The word a configuration file uses for this kind.
    ///
    /// One table, read in both directions and from both files, so that the
    /// tcspecial configuration and the simulator file cannot disagree about
    /// what a kind is called. The spellings are also what an error says, which
    /// is how a file is told what it could have written instead.
    pub fn spelling(&self) -> &'static str {
        match self {
            DHType::Network => "network",
            DHType::Device => "device-file",
            DHType::Serial => "serial",
            DHType::I2c => "i2c",
            DHType::Spi => "spi",
            DHType::Can => "can",
            DHType::Iio => "iio",
            DHType::Rpmsg => "rpmsg",
            DHType::Gpio => "gpio",
            DHType::Hdlc => "hdlc",
        }
    }

    /// Which kind a file's word names, if it names one.
    ///
    /// `device` was what a file in the filesystem used to be called, and is
    /// not a kind any more: "device" meant that and a piece of hardware on a
    /// bus at once, which is the one place the two had to be told apart. It
    /// is left out here rather than quietly accepted, so that a file still
    /// using it is told the name it has now -- see [`DHType::renamed`].
    pub fn from_spelling(text: &str) -> Option<DHType> {
        DHType::ALL.into_iter().find(|kind| kind.spelling() == text)
    }

    /// "a" or "an", for a sentence naming this kind.
    ///
    /// `i2c` is the reason this exists: "a i2c payload" is not a thing anyone
    /// writes, and these words appear in errors people read.
    pub fn article(&self) -> &'static str {
        match self {
            DHType::I2c | DHType::Iio => "an",
            _ => "a",
        }
    }

    /// Every kind, as a file would write them, for an error to list.
    pub fn spellings() -> String {
        "network, device-file, serial, i2c, spi, can, iio, rpmsg, gpio, or hdlc"
            .to_string()
    }

    /// The name this word used to be, for a file that still writes it.
    ///
    /// A file written against the older spelling is told the name the thing
    /// has now rather than being told its word is not a kind, which is what
    /// `trigger_interval_ms` is told about `packet_interval_ms`: the author of
    /// such a file wrote something that was right once, and the useful answer
    /// is the new word and not a list of nine.
    pub fn renamed(text: &str) -> Option<&'static str> {
        match text {
            "device" => Some("device-file"),
            _ => None,
        }
    }
}

/// The word a configuration file uses for a raw socket's transport.
pub const RAW: &str = "raw";

impl NetworkProtocol {
    /// The word a configuration file uses for this transport.
    pub fn spelling(&self) -> &'static str {
        match self {
            NetworkProtocol::Tcp => "tcp",
            NetworkProtocol::Udp => "udp",
            NetworkProtocol::UnixStream => "unix_stream",
            NetworkProtocol::UnixDgram => "unix_dgram",
            NetworkProtocol::Raw(_) => RAW,
        }
    }

    /// Which transport a file's word names, if it names one.
    ///
    /// Not `raw`, which names a transport only with the `ip_protocol` beside
    /// it: see [`crate::graph::reach`], which reads the two together.
    pub fn from_spelling(text: &str) -> Option<NetworkProtocol> {
        [
            NetworkProtocol::Tcp,
            NetworkProtocol::Udp,
            NetworkProtocol::UnixStream,
            NetworkProtocol::UnixDgram,
        ]
        .into_iter()
        .find(|protocol| protocol.spelling() == text)
    }

    /// Every transport, as a file would write them, for an error to list.
    pub fn spellings() -> String {
        "tcp, udp, unix_stream, unix_dgram, or raw".to_string()
    }

    /// Whether this transport is a stream of bytes, with no packets of its
    /// own -- so that a node on it says where one of its packets ends, as a
    /// line does. A datagram is a packet already.
    pub fn is_stream(&self) -> bool {
        matches!(self, NetworkProtocol::Tcp | NetworkProtocol::UnixStream)
    }
}

/// Data handler name
///
/// What a handler is called and what it is addressed by: commands name a
/// handler with it, telemetry answers with it, and tcspecial keeps its
/// handlers in order of it. Ordered and hashed for that reason -- a name is a
/// map key -- and the configuration holds every name in a file to being
/// distinct, there being nothing else to tell two handlers apart.
#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct DHName(pub String);

impl DHName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// Which node of a configuration a command is about.
///
/// A node's position among the configuration's nodes, counting from nought in
/// the order the files declare them: the device id
/// [`crate::graph::telemetry::ids`] gives, and not a number invented for
/// commands. The id in a command and the port that node's telemetry arrives
/// on are therefore the same number, worked out in one place.
///
/// A number and not a name, because every message carrying a name is a
/// message whose construction and parsing have a length to get right, where a
/// number is a field like the sequence and the type beside it. Four bytes,
/// least significant first, as the sequence number is.
///
/// What makes the two ends mean the same node by one id is the digest: it is
/// computed over the configuration including the order the nodes appear in,
/// so two ends whose digests agree have read the same nodes in the same
/// order. Agreeing about the digest is what CONNECT establishes, and a ground
/// station whose digest does not match the vehicle's does not send commands
/// about nodes at all.
#[derive(
    Debug,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
)]
pub struct NodeId(pub u32);

impl NodeId {
    /// The id a node's position gives it.
    ///
    /// A position too large for the field saturates, which is a vehicle of
    /// four thousand million nodes and so is not a case anything will meet; it
    /// is written down rather than left to wrap, a wrapped id being a command
    /// about the wrong node.
    pub fn of(position: usize) -> Self {
        Self(u32::try_from(position).unwrap_or(u32::MAX))
    }

    /// This id as a position in the configuration's nodes.
    pub fn position(&self) -> usize {
        self.0 as usize
    }

    /// This id as it goes on the wire: four bytes, little-endian.
    ///
    /// The order a [`Timestamp`] and a [`BeaconInterval`] go in, for the
    /// reason given on the first of them.
    pub fn to_le_bytes(&self) -> [u8; 4] {
        self.0.to_le_bytes()
    }

    /// An id read off the wire.
    ///
    /// Any four bytes are a well-formed id, which is the point of a
    /// fixed-length field: an id the configuration does not have is a thing to
    /// answer rather than a thing that fails to parse.
    pub fn from_le_bytes(bytes: [u8; 4]) -> Self {
        Self(u32::from_le_bytes(bytes))
    }
}

impl std::fmt::Display for NodeId {
    /// The number, which is what a message carries. The ground shows the
    /// node's name, which it has from the configuration it read.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// Arm key for restart commands
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArmKey(pub u64);

/// How often beacons are sent, in microseconds.
///
/// A length of time and not a time of day, which is what it was called:
/// `BeaconTime` read as the time a beacon was sent, and the time a beacon
/// carries is the [`Timestamp`] in its header. The two are nothing alike --
/// one is how often, the other is when -- and the only thing in the system
/// named "beacon time" was the one that is not a time.
///
/// Microseconds, as every other time on this wire is, so that a number
/// crossing the link means one thing. It cost nothing to agree: the longest
/// interval eight bytes of microseconds can state is longer than the mission
/// of anything that flies, and the shortest is a microsecond.
///
/// On the wire it is eight bytes, least significant byte first, like a
/// [`Timestamp`]. There is no reserved value: unlike a clock, an interval
/// cannot fail to be read, and nought is refused where the sender is retimed
/// rather than being given a meaning here.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct BeaconInterval(pub u64);

impl Default for BeaconInterval {
    /// Five seconds, which is what a configuration that states nothing gets.
    fn default() -> Self {
        Self(5 * 1_000_000)
    }
}

impl BeaconInterval {
    /// This interval as a length of time, for setting a timer by.
    ///
    /// The one way the number is read, so that nothing outside this type
    /// knows what unit it is in.
    pub fn duration(&self) -> Duration {
        Duration::from_micros(self.0)
    }

    /// A length of time as the interval that states it.
    ///
    /// A length too long to state saturates rather than wrapping: the
    /// alternative is a very long interval arriving as a very short one,
    /// which is a spacecraft beaconing as fast as it can because the ground
    /// asked it to beacon once a century.
    pub fn of(interval: Duration) -> Self {
        Self(u64::try_from(interval.as_micros()).unwrap_or(u64::MAX))
    }

    /// This interval as it goes on the wire: eight bytes, little-endian.
    ///
    /// The order a [`Timestamp`] goes in, for the reason given there.
    pub fn to_le_bytes(&self) -> [u8; 8] {
        self.0.to_le_bytes()
    }

    /// An interval read off the wire, those eight bytes back again.
    pub fn from_le_bytes(bytes: [u8; 8]) -> Self {
        Self(u64::from_le_bytes(bytes))
    }
}

/// What one node of a graph has moved.
///
/// Since the vehicle started, and not since the node was last enabled: a node
/// that was stopped and started again has moved what it moved before, and an
/// operator asking what a payload has done wants the answer for the flight
/// rather than for the latest spell of it.
///
/// Not the same thing as what a pass reports. `Moved` says what one pass did,
/// which is how the loop reports itself; this says what this node has done
/// since the vehicle came up. A query answered from a pass's figures would
/// answer about the last twenty milliseconds.
///
/// Named for what a node does: a node has one interface and two directions --
/// what the vehicle takes from it and what the vehicle gives it -- so those
/// are the words.
#[derive(
    Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq,
)]
pub struct NodeCounts {
    /// Bytes the vehicle has taken from this node.
    pub taken_bytes: u64,
    /// Reads that produced them.
    ///
    /// A read of nought bytes is not counted: it is the ordinary answer from
    /// a device with nothing to say, and counting it would make a silent
    /// payload the busiest thing in the telemetry.
    pub reads: u64,
    /// Reads that failed.
    pub reads_failed: u64,
    /// Bytes the vehicle has given this node.
    pub given_bytes: u64,
    /// Deliveries that carried them.
    pub writes: u64,
    /// Deliveries this node refused.
    pub writes_failed: u64,
    /// Deliveries dropped because the node was disabled.
    ///
    /// Apart from `writes_failed`, because they are different news: a refused
    /// delivery is a node that would not take what it was given, and a
    /// dropped one is the configuration working as the ground asked.
    pub dropped: u64,
    /// Triggers sent to a node that has to be asked.
    ///
    /// Counted on its own because nothing else counts it: a trigger is a
    /// write with no read behind it, so without this it would appear in no
    /// count at all, and a payload that had stopped answering would look
    /// exactly like a vehicle that had stopped asking.
    pub asks: u64,
    /// Triggers the node would not take.
    ///
    /// Apart from `asks` because the two say different things: a payload that
    /// is answering, one that has gone away, and a vehicle that has given up
    /// asking are three states, and one count of attempts could not tell them
    /// apart.
    pub asks_failed: u64,
    /// Datagrams turned away from this node's socket because they came from
    /// somewhere other than its payload.
    ///
    /// A datagram socket that knows where its payload is reads only what
    /// comes from there; anything else would be a reading nobody could tell
    /// from the payload's own. Counted rather than named, so that a flood of
    /// them is one number climbing and not a log filling: a count that moves
    /// at all is something on the vehicle's network that should not be
    /// talking to this node.
    ///
    /// Defaulted where it is read, so that an answer from a vehicle built
    /// before it was counted still reads, as nought.
    #[serde(default)]
    pub strangers: u64,
    /// When this node last moved anything, either way.
    ///
    /// `None` for a node that has moved nothing, which is not the same as a
    /// node that moved something a long time ago: the first has never
    /// spoken, and the second has gone quiet.
    pub last_moved: Option<Timestamp>,
}
/// Bytes of one transfer kept for display.
///
/// A data handler's panel shows what it last sent and last received. Keeping
/// the whole of a packet to show eight bytes of it would cost a buffer per
/// direction per handler, so only the head is kept, and only this much of it.
pub const DH_SAMPLE_BYTES: usize = 8;

/// What a panel shows where a time would go before there is one.
///
/// A direction that has carried nothing has no time to show, and showing one
/// anyway -- the hour the window opened, or midnight -- would read as a
/// transfer that never happened. One constant, because tcsmoc's panels and
/// tcssim's are read side by side and have to say it the same way.
pub const NO_TRANSFER_TIME: &str = "--:--:--";

/// Bytes as a panel shows them: hex pairs, and an ellipsis if there were
/// more than `max_len` of them.
pub fn bytes_to_hex(bytes: &[u8], max_len: usize) -> String {
    let shown = &bytes[..bytes.len().min(max_len)];
    let hex: Vec<String> = shown.iter().map(|b| format!("{:02X}", b)).collect();
    let mut result = hex.join(" ");

    if bytes.len() > max_len {
        result.push_str("...");
    }

    result
}

/// The time and the first few bytes of one transfer.
///
/// Fixed size and `Copy`, so that recording one costs no allocation on the
/// path that moves payload data, and so that telemetry carrying one has a
/// layout that does not depend on what the payload sent.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DHSample {
    /// When the transfer happened, or `None` if none has.
    ///
    /// This is the time the data moved, not the time it was asked about: a
    /// panel showing the latter would count every query as activity.
    pub time: Option<Timestamp>,
    /// How many of `bytes` are data.
    pub len: u8,
    /// Bytes in the whole transfer, of which `bytes` holds the head.
    ///
    /// Kept so that a display can say a packet was longer than what is shown.
    /// Without it, a transfer of exactly [`DH_SAMPLE_BYTES`] and one of a
    /// thousand look identical.
    pub total: u32,
    /// The first [`DH_SAMPLE_BYTES`] bytes of the transfer, or fewer.
    pub bytes: [u8; DH_SAMPLE_BYTES],
}

impl Default for DHSample {
    fn default() -> Self {
        Self {
            time: None,
            len: 0,
            total: 0,
            bytes: [0; DH_SAMPLE_BYTES],
        }
    }
}

impl DHSample {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a transfer that has just happened.
    ///
    /// Longer data is truncated to what fits; the panel shows a head and says
    /// so, and the rest is of no use to it.
    pub fn record(&mut self, data: &[u8]) {
        let len = data.len().min(DH_SAMPLE_BYTES);
        // Cleared rather than overwritten in place, or a short transfer after
        // a long one would leave the tail of the long one behind it.
        self.bytes = [0; DH_SAMPLE_BYTES];
        self.bytes[..len].copy_from_slice(&data[..len]);
        self.len = len as u8;
        self.total = u32::try_from(data.len()).unwrap_or(u32::MAX);
        self.time = Some(Timestamp::now());
    }

    /// The bytes recorded, which is at most [`DH_SAMPLE_BYTES`] of them.
    pub fn data(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len).min(DH_SAMPLE_BYTES)]
    }

    /// Whether any transfer has been recorded.
    pub fn is_empty(&self) -> bool {
        self.time.is_none()
    }

    /// The two strings a panel shows for this sample: when, and what.
    ///
    /// Both programs' panels have a line of this shape, and they show the
    /// same transfer from opposite ends of it, so there is one description of
    /// how it reads. A sample with no time is a direction that has carried
    /// nothing, which is not the same as one that carried no bytes: it shows
    /// [`NO_TRANSFER_TIME`] and no data.
    ///
    /// The bytes, as bytes. What came from a node that sends values is better
    /// shown as the number it is: see [`DHSample::panel_lines_of`], which is
    /// what a panel with the configuration in hand calls.
    pub fn panel_lines(&self) -> (String, String) {
        self.panel_lines_of(false)
    }

    /// The same two strings, with the data read as a temperature where the
    /// node that sent it sends values.
    ///
    /// `as_value` is the configuration's answer about that node -- see
    /// `crate::node::vehicle::compute::node_sends_a_value` -- and not a guess
    /// from the data: every four bytes are a value, so a datum of four
    /// characters would otherwise be shown as a temperature.
    ///
    /// A reading shows as the shortest decimal that is the number, with the
    /// unit after it, because what a reader of a panel wants from this line
    /// is the temperature. Hexadecimal was what every line showed, and for a
    /// reading it was eight characters of nothing anybody could act on.
    ///
    /// Anything that cannot be read as a value falls back to the bytes --
    /// including a sample of a value that was truncated, which cannot happen
    /// today, a value being shorter than a sample. A node that sends values
    /// and sent something else has something wrong with it, and the bytes are
    /// what says so; a temperature made out of the wrong four bytes would
    /// hide it.
    pub fn panel_lines_of(&self, as_value: bool) -> (String, String) {
        let Some(time) = self.time else {
            return (NO_TRANSFER_TIME.to_string(), String::new());
        };

        if as_value && !self.was_truncated() {
            if let Ok(number) = crate::node::vehicle::compute::from_value(
                self.data(),
                "what arrived",
            ) {
                let shown = crate::node::vehicle::compute::as_text(number);
                return (time.time_of_day(), format!("{shown} K"));
            }
        }

        // The sample truncated the transfer when it was recorded, so
        // bytes_to_hex is never the one doing it here; the ellipsis comes
        // from how much of the whole the sample kept.
        let mut data = bytes_to_hex(self.data(), DH_SAMPLE_BYTES);
        if self.was_truncated() {
            data.push_str("...");
        }
        (time.time_of_day(), data)
    }

    /// Whether the transfer was longer than the bytes kept from it.
    pub fn was_truncated(&self) -> bool {
        self.total as usize > self.data().len()
    }
}

/// Network protocol type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum NetworkProtocol {
    Tcp,
    Udp,
    UnixStream,
    UnixDgram,
    /// IP datagrams of one protocol number, carried on a raw socket:
    /// `SOCK_RAW` with the number the payload's `ip_protocol` states, from
    /// IANA's list of protocol numbers. The payload's own bytes are what the
    /// datagram carries; the IP header is the kernel's, on the way out and on
    /// the way in.
    Raw(IpProtocol),
}

/// An IP protocol number a raw socket may be opened with: 1 to 254.
///
/// Not 0, which on a raw socket is no protocol at all -- `IPPROTO_IP` is a
/// level for socket options, and a raw socket opened with it is refused by
/// the kernel -- and not 255, `IPPROTO_RAW`, which can only send: a payload
/// that could never be read would be a payload nobody could tell was there.
#[derive(
    Debug,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
)]
pub struct IpProtocol(u8);

impl IpProtocol {
    /// The number, if a raw socket may be opened with it.
    pub fn of(number: u8) -> Option<IpProtocol> {
        (1..=254).contains(&number).then_some(IpProtocol(number))
    }

    /// The number the socket is opened with.
    pub fn number(&self) -> u8 {
        self.0
    }
}

impl std::fmt::Display for IpProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The two ends of the beacon link.
///
/// Both ends resolve this the same way, out of the same two files, which is
/// why it is one value: tcspecial makes its sending socket out of `send` and
/// sends to `recv`, a ground station makes its receiving socket out of `recv`
/// and accepts beacons from `send` and nowhere else, and a pair that
/// disagreed would lose every beacon with nothing to show for it.
///
/// Here rather than in either program because both of them need it and
/// neither owns it. It was in tcspecial, which made a ground library wanting
/// the same two addresses either depend on the flight software or declare a
/// second type for the same pair -- and two types for one pair is how the two
/// ends come to disagree about where the link is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeaconLink {
    /// Where beacons originate: the address and port tcspecial binds the
    /// socket it sends them on to.
    pub send: HostPort,
    /// Where they are received: the address and port a ground station binds
    /// the socket it receives them on to.
    pub recv: HostPort,
}

impl BeaconLink {
    /// The beacon link a command interpreter's configuration states.
    ///
    /// What a configuration with no payload set's `tcspecial` section to
    /// prefer comes to; see `tcspecial::config::beacon` for the pair of
    /// files.
    pub fn of(ci: &CIConfig) -> BeaconLink {
        BeaconLink {
            send: ci.beacon_send.clone(),
            recv: ci.beacon_recv.clone(),
        }
    }
}

/// Configuration for a network endpoint
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkConfig {
    pub protocol: NetworkProtocol,
    pub address: String,
    pub port: u16,
}

/// Where a datagram payload that speaks first sends from, where its node
/// says: its `sends_from`.
///
/// A payload that speaks first is learnt from what arrives, so without this
/// the vehicle reads the first sender it hears as the payload, whoever that
/// is. With it, the vehicle reads that address and nothing else, as it reads
/// a payload it reaches at a stated address.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SendsFrom {
    /// A UDP payload's host and port.
    Socket(std::net::SocketAddr),
    /// A Unix datagram payload's path.
    Path(String),
}

/// Configuration for a device endpoint
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceConfig {
    pub path: String,
}

/// Configuration for a serial endpoint
///
/// A device node and the framing of the line it opens. The framing is here
/// rather than left to whatever the port was last set to because a serial
/// line carries no negotiation: both ends must be told the same thing, and a
/// line read at the wrong rate delivers bytes that are wrong rather than
/// absent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerialConfig {
    pub path: String,
    /// Line rate in bits per second.
    pub datarate: u32,
    /// Stop bits following each byte, which is what start-stop framing uses
    /// in place of a clock.
    pub stop_bits: StopBits,
    /// Parity per character, which is what termios sets.
    pub parity: Parity,
    /// Data bits per byte. A count rather than a `ByteLength`, which is the
    /// configuration language's checked form: by the time a handler is
    /// started the count has been checked, and this is the number the line
    /// is set to.
    pub byte_length: u8,
    /// The half-duplex turnaround of an RS-485 bus, and `None` for a line
    /// that is not one.
    ///
    /// Applied rather than recorded, unlike the synchronous terms above: the
    /// kernel takes it through an ioctl on the same device file, which is the
    /// file a handler has open. Experimental; see
    /// [`crate::node::payload::serial::rs485`].
    #[serde(default)]
    pub rs485: Option<crate::Rs485Params>,
}

/// Configuration for an endpoint on an I2C bus
///
/// The one kind of endpoint that two of its own cannot be told apart by
/// name: several devices sit on one bus, so the bus device and the address
/// the master sends to are both needed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct I2cConfig {
    /// The bus device, such as `/dev/i2c-1`.
    pub bus: String,
    /// Address of the device on that bus.
    pub address: u16,
    /// Address with ten address bits rather than seven.
    pub ten_bit: bool,
    /// Append an SMBus packet error check to each transfer.
    pub pec: bool,
}

/// Configuration for a SPI endpoint
///
/// A device node, which names the bus and the chip select together, and the
/// terms the peripheral is clocked on. Like a serial line and unlike a
/// network address, none of it is negotiated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpiConfig {
    pub path: String,
    /// Greatest clock rate the peripheral accepts, in Hz.
    pub max_speed: u32,
    pub mode: SpiMode,
    /// Bits per word, as a count: see `SerialConfig::byte_length`.
    pub bits_per_word: u8,
    pub bit_order: BitOrder,
    pub cs_active: CsActive,
}

/// Configuration for an endpoint that is a set of discrete lines
///
/// Experimental; see [`crate::node::payload::gpio`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GpioConfig {
    /// The chip's character device: `/dev/gpiochip0`.
    pub path: String,
    /// The offsets of the lines, in the order the file gave them.
    pub lines: Vec<u32>,
    /// Whether they are watched or driven.
    pub direction: crate::node::payload::gpio::Direction,
    /// Which changes of a line are events. Only meaningful on lines that are
    /// watched, and refused on lines that are driven.
    pub edge: crate::LineEdge,
    /// What holds a line nothing is driving.
    pub bias: crate::Bias,
    /// How long a line must be steady before a change counts, where the file
    /// asks for it.
    pub debounce: Option<Duration>,
    /// How often the current level of every line is reported, where the file
    /// asks for it.
    ///
    /// Events say when a line changed; this says what it is now. A ground
    /// station that connected after the last edge knows nothing about a line
    /// until it changes again, and the kernel's event queue is finite, so a
    /// flood can leave the ground holding a value that is simply wrong. A
    /// report every so often closes both: after one interval at most, what
    /// the ground has is the truth.
    pub state_interval: Option<Duration>,
}

/// Configuration for an endpoint that is an rpmsg channel
///
/// Experimental; see [`crate::node::payload::rpmsg`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RpmsgConfig {
    /// The character device the channel is: `/dev/rpmsg0`.
    pub path: String,
    /// The service name the remote announced it with, which is what
    /// identifies the channel: a node's number is assigned as channels
    /// appear.
    pub service: String,
    /// The greatest number of bytes one message carries, where the platform
    /// is not the usual one; see
    /// [`crate::node::payload::rpmsg::USUAL_MESSAGE_BYTES`].
    pub max_message_bytes: Option<u32>,
}

/// Configuration for an endpoint that is a sensor's sample buffer
///
/// Experimental; see [`crate::node::payload::iio`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IioConfig {
    /// The character device the buffer is read from: `/dev/iio:device0`.
    ///
    /// All there is. Which channels are enabled and how fast the device
    /// samples are set through `sysfs` by whoever brought the sensor up, so a
    /// payload says neither: see [`crate::node::payload::iio`].
    pub path: String,
}

/// Configuration for an endpoint on a CAN bus
///
/// Experimental; see [`crate::node::payload::can`] for what each term means
/// and which of them are applied.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanConfig {
    /// The network interface the controller is: `can0`, or `vcan0` for the
    /// kernel's virtual bus.
    pub interface: String,
    /// The identifier this payload's frames carry, which the handler filters
    /// on and sends its triggers with.
    pub can_id: u32,
    /// Whether frames are CAN FD, which is also what lets a packet be longer
    /// than eight bytes.
    pub fd: bool,
    /// Whether the controller loops frames back to other sockets on this
    /// host, which is what lets tcssim stand in for the payload.
    pub loopback: bool,
}

/// Configuration for an endpoint that is a synchronous serial line
///
/// An interface rather than a path: the kernel drives such a line as a network
/// interface, and the terms of the line are set with an ioctl on it rather
/// than with termios on a file descriptor. Experimental; see
/// [`crate::node::payload::hdlc`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HdlcConfig {
    /// The network interface the card presents: `hdlc0`.
    pub interface: String,
    /// Which electrical standard the port is wired as.
    pub standard: crate::node::payload::hdlc::Standard,
    /// Whose clock the bits are on.
    pub clock_type: ClockType,
    /// The clock rate in bits per second where this end generates the clock,
    /// and `None` where the far end does.
    pub clock_rate: Option<u32>,
    /// Whether the port is put in loopback.
    pub loopback: bool,
    /// How the bits are carried on the wire.
    pub encoding: Encoding,
    /// What checks a frame: the kernel's own `parity` for raw HDLC.
    pub frame_check: FrameCheck,
}

/// What a data handler does its payload I/O on, and so what kind it is.
///
/// One variant per kind of endpoint, each carrying what that kind needs to be
/// opened and operated. A kind is not a protocol: `Network` carries the
/// protocol it runs over, and the other four are not addressed that way at
/// all.
///
/// `Device` is a device file opened and read as it comes, and nothing more:
/// the kinds that are also device files but have terms of their own -- a
/// serial line's framing, a bus address, a clock mode -- are their own
/// variants, because a handler that lost those on the way to being started
/// would open the right file and talk to it wrongly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EndpointConfig {
    Network(NetworkConfig),
    Device(DeviceConfig),
    Serial(SerialConfig),
    I2c(I2cConfig),
    Spi(SpiConfig),
    /// Experimental; see [`CanConfig`].
    Can(CanConfig),
    /// Experimental; see [`IioConfig`].
    Iio(IioConfig),
    /// Experimental; see [`RpmsgConfig`].
    Rpmsg(RpmsgConfig),
    /// Experimental; see [`GpioConfig`].
    Gpio(GpioConfig),
    /// Experimental; see [`HdlcConfig`].
    Hdlc(HdlcConfig),
}

impl EndpointConfig {
    /// Which kind of data handler this endpoint makes.
    ///
    /// The one mapping from an endpoint to a kind, so that the ground asking
    /// for a handler and the spacecraft checking what was asked for cannot
    /// read the same configuration differently. START_DH carries the kind,
    /// and tcspecial refuses one that is not this.
    pub fn kind(&self) -> DHType {
        match self {
            EndpointConfig::Network(_) => DHType::Network,
            EndpointConfig::Device(_) => DHType::Device,
            EndpointConfig::Serial(_) => DHType::Serial,
            EndpointConfig::I2c(_) => DHType::I2c,
            EndpointConfig::Spi(_) => DHType::Spi,
            EndpointConfig::Can(_) => DHType::Can,
            EndpointConfig::Iio(_) => DHType::Iio,
            EndpointConfig::Rpmsg(_) => DHType::Rpmsg,
            EndpointConfig::Gpio(_) => DHType::Gpio,
            EndpointConfig::Hdlc(_) => DHType::Hdlc,
        }
    }
}

/// How a payload is made to send: on its own, or when asked.
///
/// Payloads are generally one of two kinds, and which one a payload is decides
/// where the timing of it is written down. A payload that sends on its own
/// needs nothing from tcspecial, and how fast it sends is a property of the
/// payload rather than of the handler -- so a tcspecial configuration says
/// nothing about it, and a simulated one takes its rate from the simulator
/// file. A payload that answers a request needs tcspecial to send one, at some
/// rate, and both are flight behaviour: what to send comes from the payload's
/// interface document and so belongs in the tcspecial configuration.
///
/// The two are exclusive by construction here: a periodic handler cannot be
/// given a trigger, because the variant has nowhere to put one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DHMode {
    /// The payload sends on its own. Tcspecial reads what arrives.
    Periodic,
    /// The payload answers a request, which tcspecial sends at an interval.
    Triggered {
        /// What to send, as bytes: a trigger is what a payload's interface
        /// document asks for, which is as likely to be `0x55AA` as `READ\r`.
        /// A file may write it either way; see [`crate::trigger`].
        trigger: Vec<u8>,
        /// How often to send it. The tcspecial configuration's
        /// `packet_interval`: tcspecial does the sending, so the rate is
        /// flight behaviour.
        ///
        /// A `Duration` and not a count, because what is done with it is to
        /// wait: the file says a length of time and the code that sleeps,
        /// schedules and compares wants one too, so nothing in between has to
        /// remember which unit a number was in.
        interval: Duration,
    },
}

impl Default for DHMode {
    /// Periodic, which is both the commoner kind and what every tcspecial
    /// configuration written before there was a mode describes.
    fn default() -> Self {
        DHMode::Periodic
    }
}

impl DHMode {
    /// What to send and how often, for a handler that has to ask.
    pub fn polling(&self) -> Option<(&[u8], Duration)> {
        match self {
            DHMode::Periodic => None,
            DHMode::Triggered { trigger, interval } => {
                Some((trigger, *interval))
            }
        }
    }
}

/// Data handler configuration
///
/// Compared as a value so that one file's handlers can be held against
/// another's: a payload set written in YAML and in XML must produce the same
/// handlers, and that is a comparison rather than an inspection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DHConfig {
    pub name: DHName,
    pub endpoint: EndpointConfig,
    pub packet_size: usize,
    /// Where this handler exchanges payload data with the OC.
    ///
    /// Always UDP: that is the link between the OC and a data handler. Each
    /// handler has its own address, because the OC addresses a handler rather
    /// than the spacecraft and then the handler within it.
    ///
    /// `None` for a configuration that assigns none. A handler cannot be
    /// started without one -- it would have nowhere to send what it reads --
    /// but a file describing payloads is complete without it.
    pub oc: Option<NetworkConfig>,
    /// The *ground's* end of this payload's data link.
    ///
    /// The MOC binds this and says hello to [`DHConfig::oc`]; tcspecial binds
    /// that and answers where the hello came from. Two ends, two bases, one
    /// arithmetic rule -- `payload_telemetry.dest_base` counted on by the
    /// payload's own id --
    /// so both ends compute the same pair of ports from the same file and
    /// neither has to be told the other's.
    ///
    /// `None` for a set with no `tcspecial` section, as `oc` is: there is
    /// nothing for either end to bind.
    pub ground: Option<NetworkConfig>,
    /// How this payload is made to send; see [`DHMode`].
    ///
    /// Defaulted rather than required, so that a tcspecial configuration
    /// written before there was a mode describes what it always described: a
    /// payload that sends on its own.
    #[serde(default)]
    pub mode: DHMode,
    /// How long the ground waits for this payload's data before calling it
    /// stale, in milliseconds.
    ///
    /// Read by the ground and by nothing aboard: an enabled payload that has
    /// sent nothing for this long is a payload whose panel says so. It is a
    /// property of the payload rather than of the display -- how often a
    /// particular instrument has something to say is what decides it, and the
    /// same figure is right for whoever is watching.
    ///
    /// `None` for a payload that does not say. The ground then has nothing to
    /// judge by and says that rather than guessing: a figure nobody chose
    /// would make a quiet payload look late, or a late one look well.
    ///
    /// **Needs examination.** It is the one attribute here that only the
    /// ground reads, which is a shape this schema has not had before, and
    /// what a sensible interval is for a payload that answers requests -- its
    /// data arriving only when tcspecial asks -- has not been worked through.
    #[serde(default)]
    pub stale_interval: Option<Duration>,
    /// Where this payload sends from, where its node says: see [`SendsFrom`].
    ///
    /// Read by the simulator, whose stand-in sends from there; aboard it is
    /// [`crate::graph::reach::Reach::sends_from`] that is read.
    #[serde(default)]
    pub sends_from: Option<SendsFrom>,
    /// Where this payload is reached over UDP when it is simulated, where its
    /// node says: see [`crate::graph::reach::Reach::stand_in`].
    ///
    /// Read by the simulator, whose stand-in for it is then a UDP socket
    /// there rather than anything at [`DHConfig::endpoint`], which is still
    /// what the payload is and what its simulator entry is held to.
    #[serde(default)]
    pub stand_in: Option<std::net::SocketAddr>,
    /// Whether the payload is only driven: written to by the vehicle and
    /// never read, as a heater or a valve is.
    ///
    /// A stand-in for one sends nothing and watches what arrives, counting
    /// it, so that tcssim shows a driven payload as it shows every other. Its
    /// simulator entry then needs no packet interval, there being nothing to
    /// send.
    #[serde(default)]
    pub driven: bool,
}

/// What the rules for `kind` make of these terms.
///
/// A node states them as its attributes, and [`crate::graph::reach`] gathers
/// them and asks here: the rules themselves are asked in one place, so that
/// no reader of a configuration can come to read them differently.
pub(crate) fn kind_of_terms(
    name: &str,
    kind: DHType,
    wire: crate::endpoint_config::LinkTerms,
) -> Result<crate::endpoint_config::GroupKind, String> {
    match kind {
        DHType::Serial => {
            crate::node::payload::serial::group_kind_of(name, wire)
        }
        DHType::I2c => crate::node::payload::i2c::group_kind_of(name, wire),
        DHType::Spi => crate::node::payload::spi::group_kind_of(name, wire),
        DHType::Can => crate::node::payload::can::group_kind_of(name, wire),
        DHType::Iio => crate::node::payload::iio::group_kind_of(name, wire),
        DHType::Rpmsg => crate::node::payload::rpmsg::group_kind_of(name, wire),
        DHType::Gpio => crate::node::payload::gpio::group_kind_of(name, wire),
        DHType::Hdlc => crate::node::payload::hdlc::group_kind_of(name, wire),
        other => {
            return Err(format!(
                "{} is not a kind of link this describes",
                other.spelling()
            ))
        }
    }
    .map_err(|e| format!("{e}"))
}

/// Payload bytes per telemetry log segment file, when the configuration
/// does not say. The segment file header is added on top of this.
/// Payload bytes per segment file, for a configuration that does not say.
///
/// A size rather than a required value because it is a tuning choice with a
/// defensible answer, where the beacon attributes beside it are mission facts
/// with none: 64KiB is small enough that a lost segment loses little and large
/// enough that the per-file overhead is nothing.
pub fn default_log_segment_bytes() -> u32 {
    65_536
}

/// How large a datum may be.
///
/// The buffer every read aboard is given, and the most one delivery can carry.
/// A sensor's reading is a handful of bytes and a line event is sixteen; this
/// is room for a whole segment of anything the vehicle moves, and a node that
/// wants less says so with `read_size`. Here rather than in tcspecial so that
/// a configuration asking for more -- a line whose `max_length` no datum
/// could carry -- is refused where the file is checked.
pub const DATUM_BYTES: usize = 4096;

/// A host and a port, as one attribute writes them.
///
/// Four of the tcspecial section's attributes are one of these: where tcspecial
/// takes commands, where the MOC takes them, and the two bases the per-payload
/// data links are counted from. One attribute apiece rather than two, as a
/// beacon address is: an address and a port only ever used together are one
/// fact about a configuration, and writing them apart invites a file that
/// states one and not the other.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostPort {
    /// The host, as the file spelled it.
    ///
    /// Kept as text and not resolved when the file is read: what a name
    /// resolves to is the business of the moment the socket is made. It may be
    /// `0.0.0.0`, which is every interface, or one interface's own address, or
    /// a name.
    pub address: String,
    pub port: u16,
}

impl HostPort {
    /// This base, counted on by a payload's own id.
    ///
    /// The one place the sum is taken, so that both ends of a link compute the
    /// same port from the same file: a payload's data link is the base port
    /// plus the payload's id, and the id is the position the payload has in the
    /// configuration -- see [`crate::graph::order`].
    ///
    /// Refused rather than wrapped where the sum leaves the range a port has:
    /// a configuration of three hundred payloads based at 65400 describes
    /// something no socket can be, and the useful moment to say so is while
    /// somebody is reading the file.
    pub fn at(&self, id: usize) -> Result<NetworkConfig, String> {
        let port = (self.port as usize).checked_add(id).ok_or_else(|| {
            format!("{}:{} plus {id} is not a port", self.address, self.port)
        })?;
        if port > u16::MAX as usize {
            return Err(format!(
                "{}:{} plus {id} is port {port}, and a port is at most 65535: \
                 a base this high leaves no room for the payloads counted from \
                 it",
                self.address, self.port
            ));
        }
        Ok(NetworkConfig {
            protocol: NetworkProtocol::Udp,
            address: self.address.clone(),
            port: port as u16,
        })
    }
}

impl std::fmt::Display for HostPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.address, self.port)
    }
}

/// A host and a port, as `address:port`, or what is wrong with how it was
/// written.
///
/// `attribute` is the name the complaint uses, because a configuration with
/// four of these is read by somebody who needs to know which one is wrong.
///
/// Split at the last colon, so that whatever a host is called the port is the
/// part after it.
pub fn host_port_of(attribute: &str, stated: &str) -> Result<HostPort, String> {
    let said = stated.trim();
    let (address, port) = said.rsplit_once(':').ok_or_else(|| {
        format!(
            "{attribute} \"{said}\" is not an address and a port: it is \
             written as address:port, as 0.0.0.0:4000"
        )
    })?;

    if address.trim().is_empty() {
        return Err(format!(
            "{attribute} \"{said}\" states no address. Every interface is \
             0.0.0.0, which is what a vehicle usually wants"
        ));
    }
    let port: u16 = port.trim().parse().map_err(|_| {
        format!(
            "{attribute} \"{said}\" ends in \"{port}\", which is not a port: a \
             port is a whole number between 1 and 65535"
        )
    })?;
    if port == 0 {
        return Err(format!(
            "{attribute} \"{said}\" asks for port 0, which is a request for \
             whatever port is free -- and a link nobody can predict the port \
             of is one the other end cannot reach"
        ));
    }

    Ok(HostPort {
        address: address.trim().to_string(),
        port,
    })
}

/// The spacecraft command link, as a file states it.
///
/// One conversation and two sockets, each named for the end it belongs to and
/// written `address:port`. Both ends are in one file because both ends read
/// this file: a configuration that stated only its own end would leave the
/// other to be told separately, and two places to say one thing is one place
/// for them to disagree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TcsCmdJson {
    /// tcspecial's own socket: it receives commands from the MOC here, and
    /// sends its responses to the MOC from here.
    ///
    /// One socket for both directions, which is what a UDP exchange is -- a
    /// response goes back out of the socket the command arrived on, to the
    /// address it arrived from.
    ///
    /// The protocol is not stated and cannot be: every link in this section is
    /// UDP. A configuration that could say otherwise would be a configuration
    /// that could say something untrue.
    pub recv: String,
    /// The MOC's own socket: it sends commands to tcspecial from here, and
    /// receives tcspecial's responses here.
    ///
    /// The names say which end and which way, which an attribute saying only
    /// *where* a link is does not: a reader is left to work out which of the
    /// two ends it belongs to.
    pub send: String,
}

/// The beacon link and its timing, as a file states it.
///
/// All three are required of every command interpreter configuration, with no
/// defaults: beacons are how the ground knows the spacecraft is alive, so
/// where they come from, where they go and how often is not something to be
/// left to a default that a file silently takes. A payload set's `tcspecial`
/// section states them for the set, and the command interpreter's own file
/// states them for a set that has no section.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BeaconJson {
    /// How long between beacons, as a length of time: `5 s`, `500 ms`.
    ///
    /// One grammar for every length of time a configuration states; see
    /// [`crate::graph::parse_interval`]. It was `interval_ms`, a bare count of
    /// milliseconds, and [`BeaconJson::interval_ms`] is named below so that a
    /// file still writing it is told the name this has now.
    pub interval: String,
    /// Named here only to be refused: see [`BeaconJson::interval`].
    ///
    /// A file written before the rename would otherwise be told that
    /// `interval` is missing, and left to notice that the word it wrote says
    /// the same thing.
    #[serde(default)]
    pub interval_ms: Option<u32>,
    /// Where beacons originate, as `address:port`.
    ///
    /// tcspecial makes the socket it sends beacons on out of this: it is the
    /// address and port that socket is bound to, so every beacon leaves from
    /// here.
    pub send: String,
    /// And where they are received, as `address:port`.
    ///
    /// tcsmoc makes the socket it receives beacons on out of this: it is the
    /// address and port that socket is bound to, and it is where tcspecial
    /// sends. Plain UDP between two stated ends.
    ///
    /// Beacons went to a multicast group, which bought something real -- any
    /// number of ground stations could join it and neither end needed the
    /// other's address -- and cost two things to get right: a group address
    /// that is nobody's, and an interface attribute saying which wire to join
    /// it on, whose being wrong lost every beacon silently. One named sender
    /// and one named listener is what a configuration can state plainly and a
    /// firewall rule can describe.
    pub recv: String,
}

/// Where a payload's telemetry goes, as a file states it.
///
/// Two bases and one arithmetic rule. Each payload has a UDP interface of its
/// own for exchanging data with the ground, and its port on each side is that
/// side's base plus the payload's own id -- the position it has in the
/// configuration. One base and a rule rather than an address and a port
/// written on every payload: forty payloads were forty chances to repeat a
/// port, and a repeated port is two payloads whose data arrives interleaved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PayloadTelemetryJson {
    /// Where the telemetry comes from: the base for the vehicle's side of
    /// those links, as `address:port`.
    pub source_base: String,
    /// And where it goes: the base for the MOC's side, counted the same way by
    /// the payload's id.
    pub dest_base: String,
}

/// The telemetry log, as a file states it.
///
/// Both have defaults, and so does the subsection: a configuration that says
/// nothing about logging runs without one, which is a configuration somebody
/// may mean -- a vehicle with no writable storage -- and the segment size is a
/// tuning choice with a defensible answer, so it has one.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogJson {
    /// Directory holding the log's segment files. The directory must already
    /// exist. Telemetry logging is disabled when this is absent.
    #[serde(default)]
    pub directory: Option<String>,
    /// Payload bytes per segment file. The segment file header is added to
    /// this, so it is the space available for telemetry records.
    #[serde(default = "default_log_segment_bytes")]
    pub segment_size: u32,
}

impl Default for LogJson {
    fn default() -> LogJson {
        LogJson {
            directory: None,
            segment_size: default_log_segment_bytes(),
        }
    }
}

/// Where payload commands go, as a file states it.
///
/// One socket per payload on each side, counted from a base by the payload's
/// own id, exactly as its telemetry is: a command about a payload goes to that
/// payload's socket and the answer comes back on it. One base and an
/// arithmetic rule rather than a socket written on every payload.
///
/// It was one socket for all of them -- `payload_interface` -- which worked
/// and left two things undone. Nothing said which socket the ground sent from,
/// so tcspecial could not be told what a payload command should come from the
/// way it is told about every other link; and one socket meant one queue, so a
/// command about a slow payload was ahead of a command about a quick one.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PayloadCommandJson {
    /// Where payload commands come from, as `address:port`: the ground's end,
    /// which the MOC binds, payload *k* at the port plus *k*.
    pub source_base: String,
    /// And where they go: the vehicle's end, which tcspecial binds, counted
    /// the same way by the payload's id.
    pub dest_base: String,
}

/// The file-level form of the command interpreter's configuration
///
/// All but one of its entries are subsections -- `tcs_cmd`,
/// `payload_telemetry`, `beacon` and `log` -- because each is one thing with
/// more than one attribute to it, and a prefix repeated on two or three flat
/// names is a subsection somebody declined to write. The names under one are
/// short for the same reason: `beacon.send` says what `beacon_send` said, and
/// says it where the reader is already looking at the beacon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CIConfigJson {
    /// What the vehicle answers to, which decides how every link below is
    /// written. Required, with no default, for the reason the links are: a
    /// file given an answer it was never asked for is a vehicle that listens
    /// for commands where nobody sends them.
    pub gateway: crate::gateway::GatewayKind,
    /// The spacecraft command link: both ends of it.
    pub tcs_cmd: TcsCmdJson,
    /// Where each payload's telemetry comes from and goes to.
    pub payload_telemetry: PayloadTelemetryJson,
    /// Where commands about a node are taken, which is not the link above.
    ///
    /// A command about a node -- start it, stop it, set an attribute, ask what
    /// it has moved -- arrives here, at `dest_base` with nothing added, the
    /// node's id being in the message; a command about the spacecraft -- ping
    /// it, arm it, restart it -- arrives on `tcs_cmd.recv`. See
    /// [`crate::CommandType::link`], which is the one table that says which is
    /// which, read by both ends.
    ///
    /// Separate links because what the ground needs in a hurry must never be
    /// queued behind what it asked for at leisure: a RESTART waiting behind a
    /// node's statistics is a spacecraft that cannot be rescued while it is
    /// busy. On a space link these become virtual channels, which is where the
    /// priority between them is really decided; here they are ports, which is
    /// what an IP network has to divide traffic with.
    ///
    /// Required, with no default, for the reason the beacon attributes are:
    /// where commands are taken is not a question to be answered by whatever
    /// a file was silently given.
    pub payload_command: PayloadCommandJson,
    /// The beacon link and how often it carries one.
    pub beacon: BeaconJson,
    /// Where the telemetry log is written, and in what sized pieces.
    #[serde(default)]
    pub log: LogJson,
    /// How long a RESTART_ARM licenses a RESTART for, as a length of time:
    /// `20 min`, `90 s`. Absent is [`crate::RESTART_ARM_VALID_DEFAULT`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart_arm_valid: Option<String>,
}

/// How long an arm lasts, from what `restart_arm_valid` says.
///
/// Nought is refused: an arm that lapsed as it was taken would be a vehicle
/// that could never be restarted, which is a configuration nobody means and
/// would find out about only when they needed it.
pub fn restart_arm_valid_of(said: &str) -> Result<Duration, String> {
    let valid = crate::graph::parse_interval(said)
        .map_err(|why| format!("restart_arm_valid: {why}"))?;
    if valid.is_zero() {
        return Err(
            "restart_arm_valid is nought, which is an arm that lapses as it is \
             taken and a vehicle that can never be restarted: give it a length \
             of time, as 20 min"
                .to_string(),
        );
    }
    Ok(valid)
}

/// Command interpreter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CIConfig {
    /// The address the command links are taken on, from `tcs_cmd.recv`.
    pub address: String,
    /// And the port of it, for commands about the spacecraft.
    pub port: u16,
    /// Where the MOC sends commands from and receives responses, from
    /// `tcs_cmd.send`.
    pub moc_cmd_send: HostPort,
    /// The bases the per-payload data links are counted from: the vehicle's
    /// side, which the telemetry comes from, and the MOC's, which it goes to.
    /// From [`PayloadTelemetryJson`]; see [`HostPort::at`].
    pub payload_source_base: HostPort,
    pub payload_dest_base: HostPort,
    /// The bases each payload's command socket is counted from: the ground's
    /// side, which commands come from, and the vehicle's, which they go to.
    /// From [`PayloadCommandJson`]; see [`HostPort::at`].
    pub payload_command_source_base: HostPort,
    pub payload_command_dest_base: HostPort,
    pub beacon_interval: Duration,
    /// Where beacons originate: tcspecial binds this and sends from it.
    ///
    /// Settled when the file is read rather than where the beacon is sent,
    /// because a beacon goes out on a timer with nobody to report to: an
    /// address that is not one has to be refused while there is still somebody
    /// reading the error.
    pub beacon_send: HostPort,
    /// And where they are received: the MOC binds this and tcspecial sends to
    /// it.
    pub beacon_recv: HostPort,
    /// Directory holding the telemetry log's segment files, or `None` to
    /// run without a telemetry log.
    pub log_dir: Option<String>,
    /// Payload bytes per segment file, not counting the segment file header.
    pub log_segment_bytes: u32,
    /// How long after a RESTART_ARM a RESTART with its key is taken.
    pub restart_arm_valid: Duration,
    /// What the vehicle answers to.
    pub gateway: crate::gateway::GatewayKind,
}

impl CIConfigJson {
    pub fn to_ci_config(&self) -> Result<CIConfig, String> {
        let command = host_port_of("tcs_cmd.recv", &self.tcs_cmd.recv)?;
        let (address, port) = (command.address, command.port);
        let payload_command_source_base = host_port_of(
            "payload_command.source_base",
            &self.payload_command.source_base,
        )?;
        let payload_command_dest_base = host_port_of(
            "payload_command.dest_base",
            &self.payload_command.dest_base,
        )?;
        let moc_cmd_send = host_port_of("tcs_cmd.send", &self.tcs_cmd.send)?;
        let payload_source_base = host_port_of(
            "payload_telemetry.source_base",
            &self.payload_telemetry.source_base,
        )?;
        let payload_dest_base = host_port_of(
            "payload_telemetry.dest_base",
            &self.payload_telemetry.dest_base,
        )?;

        let beacon_send = host_port_of("beacon.send", &self.beacon.send)?;
        let beacon_recv = host_port_of("beacon.recv", &self.beacon.recv)?;

        // The attribute's old name, said by name rather than by its absence.
        if self.beacon.interval_ms.is_some() {
            return Err(
                "beacon.interval_ms is beacon.interval now, and is a length of \
                 time: 5 s rather than 5000"
                    .to_string(),
            );
        }
        // A length of time, which is what a beacon is sent on. The
        // protocol counts milliseconds in four bytes, so an interval longer
        // than that could be set here and never commanded -- refused where
        // the file is read rather than at the first CONFIG.
        let beacon_interval =
            crate::graph::parse_interval(&self.beacon.interval)
                .map_err(|why| format!("beacon.interval: {why}"))?;
        if u32::try_from(beacon_interval.as_millis()).is_err() {
            return Err(
                "beacon.interval: longer than the protocol can carry, which \
                        counts milliseconds in four bytes"
                    .to_string(),
            );
        }

        // One socket cannot be two links. A file that gave both the same port
        // would describe a split it has not made, and the half of it that
        // bound second would fail at startup with the other half's address.
        // One socket cannot be two links. A file that put both at the same
        // place would describe a split it has not made, and the half that
        // bound second would fail at startup with the other half's address.
        //
        // The whole interface and not only the port, now that each says where
        // it is: two links may share a port on different interfaces, which is
        // a configuration somebody may mean.
        // The first payload's command socket is the base itself, so a base
        // equal to the spacecraft link is a file that has put two links in one
        // socket. The rest of the sum is checked where the payloads are known
        // and the ids with them -- see `interpreter_collisions`.
        if payload_command_dest_base.address == address
            && payload_command_dest_base.port == port
        {
            return Err(format!(
                "tcs_cmd.recv and payload_command.dest_base are both \
                 {address}:{port}, and the two links cannot be one socket: \
                 spacecraft commands are taken on the one and the first \
                 payload's commands on the other"
            ));
        }

        Ok(CIConfig {
            address,
            port,
            payload_command_source_base,
            payload_command_dest_base,
            moc_cmd_send,
            payload_source_base,
            payload_dest_base,
            beacon_interval,
            beacon_send,
            beacon_recv,
            log_dir: self.log.directory.clone(),
            log_segment_bytes: self.log.segment_size,
            restart_arm_valid: match &self.restart_arm_valid {
                Some(said) => restart_arm_valid_of(said)?,
                None => crate::RESTART_ARM_VALID_DEFAULT,
            },
            gateway: self.gateway,
        })
    }
}

/// Result status for command responses
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CommandStatus {
    Success,
    Failure,
    InvalidCommand,
    InvalidParameter,
    NotArmed,
    NotFound,
    AlreadyExists,
    Timeout,
}

impl CommandStatus {
    pub fn is_success(&self) -> bool {
        matches!(self, CommandStatus::Success)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    /// A datagram is taken only from the socket the configuration names.
    ///
    /// Four programs ask this of four links -- a beacon, a command, a
    /// response, a payload's telemetry -- so the rule is here and not in any
    /// of them. What it has to get right is the wildcard: a link bound to
    /// every interface sends from whichever one the route chose, so there is
    /// nothing written down to compare the address with and only the port can
    /// be asked about.
    #[test]
    fn a_datagram_is_taken_only_from_where_the_configuration_says() {
        use super::*;
        use std::net::SocketAddr;

        let at = |text: &str| text.parse::<SocketAddr>().expect(text);
        let stated = |address: &str, port: u16| HostPort {
            address: address.to_string(),
            port,
        };

        // The plain case: one host, one port, both as written.
        assert!(came_from(&stated("127.0.0.1", 4100), &at("127.0.0.1:4100")));
        // The port is asked about always.
        assert!(!came_from(
            &stated("127.0.0.1", 4100),
            &at("127.0.0.1:4101")
        ));
        // And the address, where the configuration names one.
        assert!(!came_from(&stated("127.0.0.1", 4100), &at("10.0.0.1:4100")));
        // The names that mean this host are one host, as they are everywhere
        // else in these rules.
        assert!(came_from(&stated("localhost", 4100), &at("127.0.0.1:4100")));

        // Every interface: the port is all there is to go on, which is the
        // shipped case -- 0.0.0.0:5550 sends beacons that arrive from
        // whichever interface carried them.
        assert!(came_from(&stated("0.0.0.0", 5550), &at("127.0.0.1:5550")));
        assert!(came_from(&stated("0.0.0.0", 5550), &at("10.0.0.1:5550")));
        assert!(!came_from(&stated("0.0.0.0", 5550), &at("127.0.0.1:5551")));
    }

    /// Every kind of endpoint makes its own kind of handler.
    ///
    /// Written out one by one rather than derived, because this is the
    /// agreement between the ground and the spacecraft about what a START_DH
    /// may say: a kind added here and nowhere else would have the ground
    /// asking for something tcspecial would refuse.
    #[test]
    fn an_endpoint_makes_the_kind_of_handler_it_is() {
        use super::*;

        let network = EndpointConfig::Network(NetworkConfig {
            protocol: NetworkProtocol::Udp,
            address: "127.0.0.1".to_string(),
            port: 5000,
        });
        assert_eq!(network.kind(), DHType::Network);

        let device = EndpointConfig::Device(DeviceConfig {
            path: "/dev/urandom".to_string(),
        });
        assert_eq!(device.kind(), DHType::Device);

        let serial = EndpointConfig::Serial(SerialConfig {
            path: "/dev/ttyS0".to_string(),
            datarate: 9600,
            parity: Parity::None,
            stop_bits: StopBits::One,
            byte_length: 8,
            rs485: None,
        });
        assert_eq!(serial.kind(), DHType::Serial);

        let i2c = EndpointConfig::I2c(I2cConfig {
            bus: "/dev/i2c-1".to_string(),
            address: 0x48,
            ten_bit: false,
            pec: false,
        });
        assert_eq!(i2c.kind(), DHType::I2c);

        let spi = EndpointConfig::Spi(SpiConfig {
            path: "/dev/spidev0.0".to_string(),
            max_speed: 1_000_000,
            mode: SpiMode::Mode0,
            bits_per_word: 8,
            bit_order: BitOrder::MsbFirst,
            cs_active: CsActive::Low,
        });
        assert_eq!(spi.kind(), DHType::Spi);
    }

    use super::*;

    #[test]
    fn test_timestamp_now() {
        let ts = Timestamp::now();
        assert!(
            !ts.clock_failed(),
            "the clock on the machine running this reads"
        );
        assert!(
            ts.micros > 0,
            "microseconds since the epoch, and it is not 1970"
        );
    }

    /// A time goes on the wire as eight little-endian bytes and comes back.
    ///
    /// The byte order written out by hand rather than taken from
    /// `to_le_bytes` on both sides, which would pass whichever order it used:
    /// this is the format the ground has to agree with, so the test states it.
    #[test]
    fn a_time_is_eight_little_endian_bytes() {
        let at = Timestamp {
            micros: 0x0807_0605_0403_0201,
        };
        assert_eq!(
            at.to_le_bytes(),
            [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
            "least significant byte first"
        );
        assert_eq!(Timestamp::from_le_bytes(at.to_le_bytes()), at);

        // All ones is the clock fault, and it survives the round trip: a
        // ground station has to be able to read it as what it is.
        assert_eq!(Timestamp::CLOCK_FAULT.to_le_bytes(), [0xFF; 8]);
        assert_eq!(
            Timestamp::from_le_bytes([0xFF; 8]),
            Timestamp::CLOCK_FAULT,
            "all ones read back as the clock fault"
        );
        assert!(Timestamp::from_le_bytes([0xFF; 8]).clock_failed());

        // And the epoch itself is a real time, which is why nought is not
        // what a clock fault sends.
        assert!(!Timestamp { micros: 0 }.clock_failed());
    }

    /// The beacon interval goes on the wire as eight little-endian bytes,
    /// and is a length of time at both ends of that.
    ///
    /// The byte order written out by hand rather than taken from
    /// `to_le_bytes` on both sides, which would pass whichever order it used.
    #[test]
    fn a_beacon_interval_is_eight_little_endian_bytes() {
        let every = BeaconInterval::of(Duration::from_secs(5));
        assert_eq!(every.0, 5_000_000, "microseconds, as every time here is");
        assert_eq!(every.duration(), Duration::from_secs(5));
        assert_eq!(
            every.to_le_bytes(),
            [0x40, 0x4B, 0x4C, 0x00, 0x00, 0x00, 0x00, 0x00],
            "least significant byte first"
        );
        assert_eq!(BeaconInterval::from_le_bytes(every.to_le_bytes()), every);

        // Five seconds is also what a configuration stating nothing gets.
        assert_eq!(BeaconInterval::default(), every);

        // A microsecond is the shortest interval statable, and an interval
        // too long to state saturates rather than wrapping round to a short
        // one -- a spacecraft asked to beacon once a century must not answer
        // by beaconing as fast as it can.
        assert_eq!(BeaconInterval::of(Duration::from_micros(1)).0, 1);
        assert_eq!(
            BeaconInterval::of(Duration::from_secs(u64::MAX)).0,
            u64::MAX
        );
    }

    /// A clock that cannot be read says so, however it failed.
    #[test]
    fn a_clock_that_cannot_be_read_says_so() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};

        // Set to before the epoch.
        assert_eq!(
            Timestamp::at(UNIX_EPOCH - Duration::from_secs(1)),
            Timestamp::CLOCK_FAULT
        );
        // Set so far ahead that microseconds since the epoch will not fit.
        assert_eq!(
            Timestamp::at(UNIX_EPOCH + Duration::from_secs(1 << 60)),
            Timestamp::CLOCK_FAULT
        );
        // And a clock that reads is not a fault.
        assert!(!Timestamp::at(SystemTime::now()).clock_failed());

        // Shown as a time nobody can vouch for, in both the places a time is
        // shown: no number, and the panels' own placeholder.
        assert_eq!(Timestamp::CLOCK_FAULT.to_string(), "no clock");
        assert_eq!(Timestamp::CLOCK_FAULT.time_of_day(), NO_TRANSFER_TIME);
    }

    /// An elapsed time comes from the two timestamps, and refuses to be
    /// guessed at.
    #[test]
    fn an_elapsed_time_needs_two_clocks_that_read() {
        let early = Timestamp { micros: 1_000_000 };
        let late = Timestamp { micros: 1_500_000 };

        assert_eq!(late.since(early), Some(Duration::from_millis(500)));
        assert_eq!(late.since(late), Some(Duration::ZERO));
        assert_eq!(early.since(late), None, "a time from the future");
        assert_eq!(late.since(Timestamp::CLOCK_FAULT), None);
        assert_eq!(Timestamp::CLOCK_FAULT.since(early), None);
    }

    /// Bytes read as hex pairs, and a transfer longer than what is kept
    /// says so.
    #[test]
    fn bytes_read_as_hex_pairs() {
        assert_eq!(bytes_to_hex(&[0x01, 0x02, 0x03], 10), "01 02 03");
        assert_eq!(
            bytes_to_hex(&[0x01, 0x02, 0x03, 0x04, 0x05], 3),
            "01 02 03..."
        );
        assert_eq!(bytes_to_hex(&[], 8), "", "nothing reads as nothing");
    }

    /// A sample reads as a time and the head of what moved.
    ///
    /// Both programs' panels show it this way: the one at the spacecraft's
    /// end of a link and the one at the payload's end are read side by side,
    /// so a byte that read differently in the two would look like a
    /// difference in the data.
    #[test]
    fn a_sample_reads_as_a_time_and_the_head_of_the_transfer() {
        // Nothing recorded: the placeholder, and no bytes. A direction that
        // has carried nothing is not a direction that carried no bytes.
        let (time, data) = DHSample::new().panel_lines();
        assert_eq!(time, NO_TRANSFER_TIME);
        assert_eq!(data, "");

        let mut sample = DHSample::new();
        sample.record(&[0x52, 0x45, 0x41, 0x44, 0x0D]);
        sample.time = Some(Timestamp {
            micros: 3_661_000_000,
        });
        assert_eq!(
            sample.panel_lines(),
            ("01:01:01".to_string(), "52 45 41 44 0D".to_string())
        );

        // And a transfer longer than the sample keeps shows a head and says
        // so: without the ellipsis a packet of twelve bytes and one of eight
        // read alike.
        let mut sample = DHSample::new();
        sample.record(&[0xAB; 12]);
        sample.time = Some(Timestamp { micros: 0 });
        let (_, data) = sample.panel_lines();
        assert_eq!(data, "AB AB AB AB AB AB AB AB...");
    }

    /// A sample of a value reads as the temperature it is.
    ///
    /// What the ground shows for a reading, where it showed four bytes of
    /// hexadecimal. Asked of the configuration and not of the data: the same
    /// four bytes are shown as bytes for a node that does not send values,
    /// because every four bytes are a value and a guess from the width would
    /// show a four-character decision as a temperature.
    #[test]
    fn a_sample_of_a_value_reads_as_a_temperature() {
        use crate::node::vehicle::compute::as_value;

        let at = |sample: &mut DHSample| {
            sample.time = Some(Timestamp {
                micros: 3_661_000_000,
            })
        };

        let mut sample = DHSample::new();
        sample.record(&as_value(293.15).expect("a temperature"));
        at(&mut sample);
        assert_eq!(
            sample.panel_lines_of(true),
            ("01:01:01".to_string(), "293.15 K".to_string())
        );
        // The same sample from a node that sends something else: the bytes.
        assert_eq!(
            sample.panel_lines_of(false),
            ("01:01:01".to_string(), "30 1D 79 11".to_string())
        );

        // Below zero, which is what the sign on a value is for: a conversion
        // with the wrong offset reads as what it is and not as 4295 K.
        let mut sample = DHSample::new();
        sample.record(&as_value(-40.0).expect("below zero"));
        at(&mut sample);
        assert_eq!(sample.panel_lines_of(true).1, "-40 K");

        // A node that sends values and sent something that is not one: the
        // bytes, because that is what says something is wrong with it.
        let mut sample = DHSample::new();
        sample.record(b"293.15");
        at(&mut sample);
        assert_eq!(sample.panel_lines_of(true).1, "32 39 33 2E 31 35");

        // And a direction that has carried nothing is still the placeholder,
        // whichever kind of node it is.
        assert_eq!(
            DHSample::new().panel_lines_of(true),
            (NO_TRANSFER_TIME.to_string(), String::new())
        );
    }

    /// A timestamp reads as the time of day it names.
    ///
    /// Including the part that matters and is easiest to leave out: a real
    /// timestamp is tens of thousands of days past the epoch, so the day has
    /// to be taken off it. A panel showing 472222:13:20 would be a panel
    /// showing the epoch.
    #[test]
    fn a_timestamp_reads_as_a_time_of_day() {
        // A time is microseconds since the epoch, so a test that means a
        // second says so.
        let at = |seconds: u64| {
            Timestamp {
                micros: seconds * 1_000_000,
            }
            .time_of_day()
        };

        assert_eq!(at(0), "00:00:00");
        assert_eq!(at(3661), "01:01:01");
        assert_eq!(at(86_399), "23:59:59", "the last second of a day");
        assert_eq!(at(86_400), "00:00:00", "and the first of the next");
        assert_eq!(
            at(1_700_000_000),
            "22:13:20",
            "a timestamp of the kind a clock actually gives"
        );

        // The fractions of a second are not shown, so two times within one
        // second read alike: a panel refreshed twice a second cannot show
        // them, and a time whose last digits changed between two looks would
        // read as traffic that had not happened.
        assert_eq!(
            Timestamp {
                micros: 1_700_000_000_999_999,
            }
            .time_of_day(),
            at(1_700_000_000)
        );
    }

    #[test]
    fn a_sample_records_the_head_of_a_transfer() {
        let mut sample = DHSample::new();
        assert!(sample.is_empty(), "nothing has moved yet");
        assert_eq!(sample.data(), &[] as &[u8]);

        sample.record(&[1, 2, 3]);
        assert!(!sample.is_empty());
        assert_eq!(sample.data(), &[1, 2, 3]);
        assert_eq!(sample.total, 3);
        assert!(!sample.was_truncated());
        assert!(sample.time.is_some(), "a recorded transfer has a time");
    }

    #[test]
    fn a_sample_keeps_only_what_fits() {
        // A packet may be any size; a sample is one fixed-size value, so the
        // head is kept and the rest dropped.
        let long: Vec<u8> = (0..64).collect();
        let mut sample = DHSample::new();
        sample.record(&long);

        assert_eq!(sample.data().len(), DH_SAMPLE_BYTES);
        assert_eq!(sample.data(), &long[..DH_SAMPLE_BYTES]);
        // The whole length survives, so a display can say there was more.
        assert_eq!(sample.total, 64);
        assert!(sample.was_truncated());
    }

    #[test]
    fn recording_again_replaces_the_previous_sample() {
        // Including the bytes the shorter transfer does not reach, or a long
        // transfer followed by a short one would show the tail of the first.
        let mut sample = DHSample::new();
        sample.record(&[9; DH_SAMPLE_BYTES]);
        sample.record(&[1, 2]);

        assert_eq!(sample.data(), &[1, 2]);
        assert_eq!(sample.bytes[2..], [0; DH_SAMPLE_BYTES - 2]);
        assert_eq!(sample.total, 2);
        assert!(!sample.was_truncated());
    }

    #[test]
    fn a_zero_length_transfer_is_still_a_transfer() {
        // It has a time, so a panel shows when rather than the placeholder.
        let mut sample = DHSample::new();
        sample.record(&[]);

        assert!(!sample.is_empty());
        assert_eq!(sample.data(), &[] as &[u8]);
    }

    /// The four SPI modes carry the clock polarity and phase their numbers
    /// mean.
    #[test]
    fn spi_modes_carry_the_polarity_and_phase_their_numbers_mean() {
        for (mode, cpol, cpha) in [
            (crate::SpiMode::Mode0, false, false),
            (crate::SpiMode::Mode1, false, true),
            (crate::SpiMode::Mode2, true, false),
            (crate::SpiMode::Mode3, true, true),
        ] {
            assert_eq!(mode.cpol(), cpol, "cpol of mode {mode}");
            assert_eq!(mode.cpha(), cpha, "cpha of mode {mode}");
        }
    }
}
