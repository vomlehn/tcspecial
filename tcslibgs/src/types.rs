//! Type definitions shared between ground and space software


use crate::endpoint_config::{ByteList, Scalar};

use serde::{Deserialize, Serialize};

use crate::node::payload::hdlc::{ClockType, Encoding, FrameCheck};
use crate::node::payload::serial::{Parity, StopBits};
use crate::node::payload::spi::{BitOrder, CsActive, SpiMode};
use crate::verify::Problem;
use std::collections::{BTreeMap, BTreeSet};
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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

/// Something one handler takes for itself, and so which no other may take.
///
/// A handler's own address is exclusive, and so is the address it reaches the
/// OC on: two handlers on one of them is a start that fails at the second,
/// or worse, two handlers splitting one stream between them and each
/// reporting half of it. The whole point of a bus is the exception, which is
/// why a device on one is a claim on the address and not on the bus.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Claim {
    /// A port on a host.
    Port { host: String, port: u16 },
    /// A path in the filesystem: a device, a line, a peripheral, or the file
    /// a Unix socket is named by. One kind of claim for all of them, since
    /// what collides is the path and not what opens it.
    Path(String),
    /// A payload on a CAN interface. The bus is shared by design; the
    /// identifier its frames carry is not.
    OnCanBus { interface: String, can_id: u32 },
    /// One line of a GPIO chip. The chip is shared by design; a line is given
    /// to one request at a time.
    OnChip { chip: String, line: u32 },
    /// Nothing, for a kind whose claims are made one at a time: a GPIO
    /// payload claims a line at a time and has no single claim of its own.
    None,
    /// A device on a bus. The bus is shared by design; the address on it is
    /// not.
    OnBus { bus: String, address: u16 },
}

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

/// What this handler claims, and what to call each claim in an error.
fn claims_of(dh: &DHConfig) -> Vec<(Claim, String)> {
    let mut claims = Vec::new();

    // What the payload side takes. One claim for every kind but GPIO, which
    // takes a line at a time: a chip is shared the way a bus is, and several
    // payloads commonly watch lines of one chip.
    let payload = match &dh.endpoint {
        EndpointConfig::Network(net) => match net.protocol {
            // A Unix socket is named by a file rather than by a port.
            NetworkProtocol::UnixStream | NetworkProtocol::UnixDgram => {
                Claim::Path(net.address.clone())
            }
            NetworkProtocol::Tcp | NetworkProtocol::Udp => Claim::Port {
                host: one_host(&net.address),
                port: net.port,
            },
        },
        EndpointConfig::Device(device) => Claim::Path(device.path.clone()),
        EndpointConfig::Serial(serial) => Claim::Path(serial.path.clone()),
        // The whole interface, not a part of it. A raw HDLC frame carries no
        // address, so there is nothing to tell two payloads on one line
        // apart -- which is what makes this a claim on the interface where a
        // CAN payload's is a claim on an identifier.
        EndpointConfig::Hdlc(hdlc) => Claim::Path(hdlc.interface.clone()),
        EndpointConfig::Iio(iio) => Claim::Path(iio.path.clone()),
        EndpointConfig::Rpmsg(rpmsg) => Claim::Path(rpmsg.path.clone()),
        EndpointConfig::Spi(spi) => Claim::Path(spi.path.clone()),
        EndpointConfig::I2c(i2c) => Claim::OnBus {
            bus: i2c.bus.clone(),
            address: i2c.address,
        },
        // A CAN bus is shared by design, like an I2C bus, and what one
        // payload takes on it is an identifier: two payloads answering to one
        // identifier on one interface are two handlers reading each other's
        // frames.
        EndpointConfig::Can(can) => Claim::OnCanBus {
            interface: can.interface.clone(),
            can_id: can.can_id,
        },
        // A line at a time, and the chip not at all: two payloads may watch
        // lines of one chip and may not watch one line, the kernel giving a
        // line to one request at a time -- so the second handler's start
        // would fail.
        EndpointConfig::Gpio(gpio) => {
            for line in &gpio.lines {
                claims.push((
                    Claim::OnChip {
                        chip: gpio.path.clone(),
                        line: *line,
                    },
                    format!("{}'s line {line}", dh.name.0),
                ));
            }
            // Its claims are made above, so there is no single one to make
            // here.
            Claim::None
        }
    };
    if payload != Claim::None {
        claims.push((payload, format!("{}'s payload", dh.name.0)));
    }

    if let Some(oc) = &dh.oc {
        claims.push((
            Claim::Port {
                host: one_host(&oc.address),
                port: oc.port,
            },
            format!("{}'s OC address", dh.name.0),
        ));
    }

    claims
}

/// How a claim reads in an error.
fn claim_text(claim: &Claim) -> String {
    match claim {
        Claim::Port { host, port } => format!("{host}:{port}"),
        Claim::Path(path) => path.clone(),
        Claim::OnBus { bus, address } => format!("address {address:#04X} on {bus}"),
        Claim::OnCanBus { interface, can_id } => {
            format!("can_id {can_id:#x} on {interface}")
        }
        Claim::OnChip { chip, line } => format!("line {line} on {chip}"),
        // Never in an error: a payload with no single claim makes its claims
        // one at a time, and each of those is named above.
        Claim::None => "nothing".to_string(),
    }
}

/// Refuse two handlers that want the same thing.
///
/// Both configuration formats end here, because the hazard is in the handlers
/// rather than in the words that described them: whichever file they were
/// written in, two handlers at one address cannot both be started, and two
/// handlers on one device file split its stream between them and each report
/// part of it as though it were the whole. A handler's own address and the
/// address it reaches the OC on are compared together, since a port is a port
/// whatever means to claim it.
pub fn no_two_handlers_claim_one_thing(handlers: &[DHConfig]) -> Result<(), String> {
    match collisions(handlers).into_iter().next() {
        Some((_, message)) => Err(message),
        None => Ok(()),
    }
}

/// Every pair of handlers that want one thing, each named by the handler
/// whose claim came second.
///
/// The second one, because the first claim is the one that stands: a reader
/// given both names has to change one of them, and the one that came second
/// is the one the file can lose without the other becoming unreachable.
pub fn collisions(handlers: &[DHConfig]) -> Vec<(DHName, String)> {
    let mut taken: BTreeMap<Claim, String> = BTreeMap::new();
    let mut found = Vec::new();

    for dh in handlers {
        for (claim, what) in claims_of(dh) {
            match taken.get(&claim) {
                Some(first) => found.push((
                    dh.name.clone(),
                    format!("{first} and {what} both want {}", claim_text(&claim)),
                )),
                None => {
                    taken.insert(claim, what);
                }
            }
        }
    }

    found
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
pub fn came_from(stated: &HostPort, from: &SocketAddr) -> bool {
    if from.port() != stated.port {
        return false;
    }
    if binds_every_interface(&stated.address) {
        return true;
    }
    one_host(&stated.address) == one_host(&from.ip().to_string())
}

/// Whether an address binds every interface rather than one of them.
///
/// A wildcard claims a port on every address at once, which is why it has to
/// be told from a host: a handler at `localhost:4000` and a command link at
/// `0.0.0.0:4000` do collide, without naming one host between them.
fn binds_every_interface(address: &str) -> bool {
    address.is_empty() || address == "0.0.0.0" || address == "::" || address == "*"
}

/// What the command interpreter's own two links take from the handlers.
///
/// A handler that binds a port a command link binds takes the commanding with
/// it: the link that bound second fails at startup, and which of them that is
/// depends on the order two programs happened to start in. Worth refusing
/// where the file is read, where it is one line to change.
///
/// Only a payload set that states a `tcspecial` section can be asked this.
/// For a set that states none, the ports are in the command interpreter's own
/// file and nothing here has seen it.
fn interpreter_collisions(ci: &CIConfig, handlers: &[DHConfig]) -> Vec<(DHName, String)> {
    // Every socket the command interpreter binds, where the file says it is:
    // the spacecraft command link, and one command socket per payload counted
    // from the base by that payload's own id. All of them, for every payload,
    // because a payload may collide with another payload's command socket as
    // easily as with its own.
    let mut links = vec![(
        ci.address.clone(),
        ci.port,
        "spacecraft command".to_string(),
    )];
    // The position in this list is the payload's id: the file's order is what
    // decides it, and this list is in that order.
    for (id, _) in handlers.iter().enumerate() {
        match ci.payload_command_dest_base.at(id) {
            Ok(at) => links.push((
                at.address,
                at.port,
                format!("command link of payload {id}"),
            )),
            // A base that leaves no room for this payload's port is refused
            // where the bases are read; nothing to add here.
            Err(_) => continue,
        }
    }

    let mut found = Vec::new();
    for dh in handlers {
        for (claim, what) in claims_of(dh) {
            let Claim::Port { host: theirs, port } = &claim else {
                continue;
            };
            for (address, mine, which) in &links {
                let address = address.as_str();
                let mine = *mine;
                let wildcard = binds_every_interface(address);
                if *port == mine && (wildcard || *theirs == one_host(address)) {
                    found.push((
                        dh.name.clone(),
                        format!(
                            "{what} and the {which} link both want port {port}{}",
                            if wildcard {
                                format!(
                                    ", which the link takes on every interface ({address})"
                                )
                            } else {
                                String::new()
                            }
                        ),
                    ));
                }
            }
        }
    }

    found
}

impl DHType {
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
        [
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
        ]
        .into_iter()
        .find(|kind| kind.spelling() == text)
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

impl NetworkProtocol {
    /// The word a configuration file uses for this transport.
    pub fn spelling(&self) -> &'static str {
        match self {
            NetworkProtocol::Tcp => "tcp",
            NetworkProtocol::Udp => "udp",
            NetworkProtocol::UnixStream => "unix_stream",
            NetworkProtocol::UnixDgram => "unix_dgram",
        }
    }

    /// Which transport a file's word names, if it names one.
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
        "tcp, udp, unix_stream, or unix_dgram".to_string()
    }
}

/// What a data handler is doing, as the spacecraft knows it.
///
/// In the library rather than in tcspecial because the ground reads it: a
/// QUERY_DH answer carries it, which is how a panel can say what a handler is
/// doing rather than what the last command it was sent implied.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DHState {
    /// Created but not activated
    Created,
    /// Active and conduiting data
    Active,
    /// Stopped
    Stopped,
}

/// Data handler name
///
/// What a handler is called and what it is addressed by: commands name a
/// handler with it, telemetry answers with it, and tcspecial keeps its
/// handlers in order of it. Ordered and hashed for that reason -- a name is a
/// map key -- and the configuration holds every name in a file to being
/// distinct, there being nothing else to tell two handlers apart.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DHName(pub String);

impl DHName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// Which node of a version 2 configuration a command is about.
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

/// What one node of a version 2 graph has moved.
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
/// Named for what a node does rather than for what a data handler did. A
/// handler had two conduits and talked about what it received from the ground
/// and sent back; a node has one interface and two directions -- what the
/// vehicle takes from it and what the vehicle gives it -- so those are the
/// words. See [`Statistics`] for version 1's.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
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
    /// When this node last moved anything, either way.
    ///
    /// `None` for a node that has moved nothing, which is not the same as a
    /// node that moved something a long time ago: the first has never
    /// spoken, and the second has gone quiet.
    pub last_moved: Option<Timestamp>,
}

/// Statistics for data handler operations
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Statistics {
    /// Timestamp when statistics were collected
    pub timestamp: Option<Timestamp>,
    /// Number of bytes received
    pub bytes_received: u64,
    /// Number of successful read operations
    pub reads_completed: u64,
    /// Number of failed read operations
    pub reads_failed: u64,
    /// Number of bytes sent
    pub bytes_sent: u64,
    /// Number of successful write operations
    pub writes_completed: u64,
    /// Number of failed write operations
    pub writes_failed: u64,
    /// Triggers sent to a triggered payload.
    ///
    /// Counted on its own because nothing else counts it. A handler's
    /// `bytes_received` is the ground's data and its `bytes_sent` is the
    /// data that went back, so the payload side of each conduit is left out
    /// of both -- those are the same bytes seen twice. A trigger is a write
    /// with no read behind it, so without this it would appear in no
    /// statistic at all, and a payload that had stopped answering would look
    /// exactly like a handler that had stopped asking.
    #[serde(default)]
    pub triggers_sent: u64,
    /// Triggers a triggered payload would not take.
    ///
    /// Apart from `writes_failed` for the reason above, and apart from
    /// `triggers_sent` because the two say different things: a payload that
    /// is answering, one that has gone away, and a handler that has given up
    /// asking are three different states, and one count of attempts could
    /// not tell them apart. `writes_failed` is data for the ground that did
    /// not reach it, which is the ground's loss; a trigger that fails is the
    /// payload's silence, and a handler goes on asking through it.
    #[serde(default)]
    pub triggers_failed: u64,
}

impl Statistics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_timestamp(mut self) -> Self {
        self.timestamp = Some(Timestamp::now());
        self
    }
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
    pub fn panel_lines(&self) -> (String, String) {
        match self.time {
            Some(time) => {
                // The sample truncated the transfer when it was recorded, so
                // bytes_to_hex is never the one doing it here; the ellipsis
                // comes from how much of the whole the sample kept.
                let mut data = bytes_to_hex(self.data(), DH_SAMPLE_BYTES);
                if self.was_truncated() {
                    data.push_str("...");
                }
                (time.time_of_day(), data)
            }
            None => (NO_TRANSFER_TIME.to_string(), String::new()),
        }
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
}

/// Configuration for a network endpoint
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkConfig {
    pub protocol: NetworkProtocol,
    pub address: String,
    pub port: u16,
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
            DHMode::Triggered { trigger, interval } => Some((trigger, *interval)),
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
}

/// What tcspecial opens to reach a payload on a line, a bus or a peripheral.
///
/// These three kinds carry attributes a network or device payload has no use
/// for -- a data rate, a slave address, a clock mode -- and every rule about
/// them already exists: which attributes a kind requires, which it refuses,
/// what each value may be, and what the whole becomes at run time. This
/// assembles what those rules expect out of a payload and its group and calls
/// them, so there is one statement of each rule rather than one per language.
///
/// The rules used to be reachable only from an endpoint configuration file,
/// which is why a tcspecial configuration could describe a network or a device
/// payload and nothing else.
fn link_of(
    name: &str,
    kind: DHType,
    stated: &DHConfigJson,
    group: Option<&DHGroupJson>,
) -> Result<EndpointConfig, String> {
    let kind_params = link_params_of(name, kind, stated, group)?;

    // A CAN payload is on a network interface rather than at a path, and is
    // addressed by the identifier its frames carry. Both are needed for the
    // reason a device on an I2C bus needs both: the bus is shared, and the
    // identifier is what tells one payload on it from another.
    if kind == DHType::Can {
        let interface = stated
            .interface
            .clone()
            .or_else(|| group.and_then(|g| g.interface.clone()))
            .ok_or_else(|| {
                format!(
                    "payload \"{name}\" is a can payload, so it needs the interface its \
                     bus is reached through"
                )
            })?;
        let can_id = stated
            .can_id
            .clone()
            .or_else(|| group.and_then(|g| g.can_id.clone()))
            .ok_or_else(|| {
                format!(
                    "payload \"{name}\" is on a CAN bus, so it needs the can_id its \
                     frames carry"
                )
            })?;
        let can_id = crate::endpoint_config::parse_u32(name, "can_id", &can_id)
            .and_then(|id| crate::node::payload::can::check_can_id(name, id))
            .map_err(|e| format!("{e}"))?;

        return crate::endpoint_config::endpoint_of(
            name,
            &crate::endpoint_config::EndpointLocation::Can { interface, can_id },
            &kind_params,
        );
    }

    // A synchronous line is a network interface and nothing else: a raw HDLC
    // frame carries no address, so the interface is the whole of where the
    // payload is.
    if kind == DHType::Hdlc {
        let interface = stated
            .interface
            .clone()
            .or_else(|| group.and_then(|g| g.interface.clone()))
            .ok_or_else(|| {
                format!(
                    "payload \"{name}\" is an hdlc payload, so it needs the interface \
                     its line is: say interface, not path"
                )
            })?;

        return crate::endpoint_config::endpoint_of(
            name,
            &crate::endpoint_config::EndpointLocation::Interface { interface },
            &kind_params,
        );
    }

    // Where it is. A line and a peripheral are a device node; a device on a
    // bus is the bus and the address on it, which is what tells one device on
    // a bus from another.
    let path = stated
        .path
        .clone()
        .or_else(|| group.and_then(|g| g.path.clone()))
        .ok_or_else(|| {
            format!(
                "payload \"{name}\" is {} {} payload, so it needs the device it is \
                 reached through",
                kind.article(),
                kind.spelling()
            )
        })?;

    let location = match kind {
        DHType::I2c => {
            let address = stated
                .payload_address
                .clone()
                .or_else(|| group.and_then(|g| g.payload_address.clone()))
                .ok_or_else(|| {
                    format!("payload \"{name}\" is on an I2C bus, so it needs an address on it")
                })?;
            let ten_bit = stated
                .ten_bit
                .clone()
                .or_else(|| group.and_then(|g| g.ten_bit.clone()))
                .map(|t| t.as_str() == "true")
                .unwrap_or(false);
            crate::endpoint_config::EndpointLocation::I2c {
                bus: path,
                address: crate::node::payload::i2c::parse_i2c_address(name, &address, ten_bit)
                    .map_err(|e| format!("{e}"))?,
            }
        }
        _ => crate::endpoint_config::EndpointLocation::Path { path },
    };

    crate::endpoint_config::endpoint_of(name, &location, &kind_params)
}

/// What kind of link a payload is on, by the rules for that kind.
///
/// Split from [`link_of`] because the rules settle more than reaches a data
/// handler: the stream rule says where one read ends, is checked here, and is
/// then dropped -- no handler carries it, and none did when an endpoint
/// configuration was what stated it. Tests of those rules reach them here.
pub(crate) fn link_params_of(
    name: &str,
    kind: DHType,
    stated: &DHConfigJson,
    group: Option<&DHGroupJson>,
) -> Result<crate::endpoint_config::GroupKind, String> {
    kind_of_terms(name, kind, link_terms_of(stated, group))
}

/// What the rules for `kind` make of these terms.
///
/// Apart from [`link_params_of`] because the terms arrive from two schemas
/// now: a version 1 payload and the group it names state them between them,
/// and a version 2 node states them as its attributes -- see
/// [`crate::graph::reach`]. The rules themselves are asked here, once, so
/// that the two schemas cannot come to be read differently.
pub(crate) fn kind_of_terms(
    name: &str,
    kind: DHType,
    wire: crate::endpoint_config::LinkTerms,
) -> Result<crate::endpoint_config::GroupKind, String> {
    match kind {
        DHType::Serial => crate::node::payload::serial::group_kind_of(name, wire),
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

impl DHConfigJson {
    /// A term this payload states that nothing can apply, if it states one.
    ///
    /// Four words were terms once and are not any more: `bus_speed`,
    /// `bitrate`, `channels` and `sample_rate`. Each said something true about
    /// the equipment and none of them could be set by a program that opens the
    /// interface -- a bus clock belongs to the controller the platform
    /// configures, a CAN bitrate to the interface as it is brought up, and a
    /// sensor's channels and rate to the `sysfs` entries whoever brought the
    /// sensor up wrote. A payload documents and takes only what it can set, so
    /// they are gone.
    ///
    /// Refused by name rather than left unknown. A file that states one wrote
    /// something that was right once, and the useful answer says which word
    /// and why -- as a file writing `trigger_interval_ms` is told the name that
    /// has now.
    fn a_term_nothing_applies(
        &self,
        group: Option<&DHGroupJson>,
    ) -> Option<(&'static str, &'static str)> {
        let stated = |mine: &Option<Scalar>, theirs: fn(&DHGroupJson) -> bool| {
            mine.is_some() || group.is_some_and(theirs)
        };

        [
            (
                "bus_speed",
                stated(&self.bus_speed, |g| g.bus_speed.is_some()),
                "a bus clock belongs to the controller, which the platform                  configures from the device tree or from ACPI",
            ),
            (
                "bitrate",
                stated(&self.bitrate, |g| g.bitrate.is_some()),
                "a CAN bitrate belongs to the interface and is set where it is                  brought up, by the platform or by ip link",
            ),
            (
                "channels",
                stated(&self.channels, |g| g.channels.is_some()),
                "which channels of a sensor are enabled is set through sysfs by                  whoever brought the sensor up",
            ),
            (
                "sample_rate",
                stated(&self.sample_rate, |g| g.sample_rate.is_some()),
                "how fast a sensor samples is set through sysfs by whoever                  brought the sensor up",
            ),
        ]
        .into_iter()
        .find(|(_, present, _)| *present)
        .map(|(field, _, why)| (field, why))
    }
}

/// The terms a payload states about its link, with its group laid under it.
///
/// Apart from [`link_params_of`] because two callers want them. That one asks
/// the rules for the payload's own kind what they mean; the other asks only
/// whether any of them was stated at all, for the two kinds that are not on a
/// link and so have no rules to refuse one -- see
/// [`DHConfigJson::to_dh_config_in`].
fn link_terms_of(
    stated: &DHConfigJson,
    group: Option<&DHGroupJson>,
) -> crate::endpoint_config::LinkTerms {
    // Each attribute as the payload states it, or as the group it names does.
    // The same inheritance every other attribute of a payload gets.
    macro_rules! inherited {
        ($field:ident) => {
            stated
                .$field
                .clone()
                .or_else(|| group.and_then(|g| g.$field.clone()))
        };
    }

    let wire = crate::endpoint_config::LinkTerms {
        datarate: inherited!(datarate),
        stop_bits: inherited!(stop_bits),
        parity: inherited!(parity),
        clock_type: inherited!(clock_type),
        encoding: inherited!(encoding),
        frame_check: inherited!(frame_check),
        clock_rate: inherited!(clock_rate),
        standard: inherited!(standard),
        loopback: inherited!(loopback),
        byte_length: inherited!(byte_length),
        protocol: inherited!(protocol),
        ten_bit: inherited!(ten_bit),
        pec: inherited!(pec),
        retries: inherited!(retries),
        timeout: inherited!(timeout),
        max_speed: inherited!(max_speed),
        spi_mode: inherited!(spi_mode),
        bits_per_word: inherited!(bits_per_word),
        bit_order: inherited!(bit_order),
        cs_active: inherited!(cs_active),
        rs485: inherited!(rs485),
        rs485_rts_on_send: inherited!(rs485_rts_on_send),
        rs485_delay_before_send: inherited!(rs485_delay_before_send),
        rs485_delay_after_send: inherited!(rs485_delay_after_send),
        can_fd: inherited!(can_fd),
        service: inherited!(service),
        max_message_bytes: inherited!(max_message_bytes),
        lines: inherited!(lines),
        edge: inherited!(edge),
        bias: inherited!(bias),
        debounce: inherited!(debounce),
        // Version 1 has no state report: its gpio payloads are watched for
        // edges and nothing else.
        state_interval: None,
        direction: inherited!(direction),
        stream: inherited!(stream).map(|s| crate::endpoint_config::StreamTerms {
            max_length: s.max_length,
            timeout: s.timeout,
            terminators: s.terminators,
        }),
    };

    wire
}

/// A length of time, named so that a bad one says which attribute it was.
///
/// Kept as a [`Duration`] from here down, because what these values are for
/// is waiting: a timer is set from one, a sleep takes one, and a staleness is
/// one subtraction. A count of milliseconds in between would be a unit for
/// every reader to remember and for one of them to forget.
///
/// The one grammar is [`crate::graph::parse_interval`]; nought is no length
/// of time and is refused there, which is what a trigger sent without pause
/// would have been.
fn as_length_of_time(attribute: &str, said: &str) -> Result<Duration, String> {
    crate::graph::parse_interval(said).map_err(|why| format!("{attribute}: {why}"))
}

/// Payload configuration file structure
///
/// A tcspecial configuration describes payloads and nothing else. It carries no
/// packet interval: how fast a payload produces packets is a property of a
/// simulation rather than of a payload, so it belongs to the simulator's own
/// configuration file, and one stated here is refused rather than ignored.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PayloadConfig {
    pub version: String,
    pub description: String,
    /// What tcspecial itself is configured with for this payload set: the
    /// same section a `tcspecial.yaml` holds, carried here so that one file
    /// can describe a whole set, the command interpreter included.
    ///
    /// First of the sections, in the files and here, because it is about the
    /// set as a whole where the two below are about the payloads in it.
    ///
    /// Optional. `beacon_send` is read from here -- where a set's beacons
    /// go is the set's own business, its ground station being what listens
    /// for them -- and the rest is carried and checked and not yet read,
    /// tcspecial taking the rest of its configuration from the file
    /// `TCSPECIAL_CONFIG_PATH` names. A malformed section is refused either
    /// way, so a file that states one is checked rather than ignored.
    #[serde(default)]
    pub tcspecial: Option<CIConfigJson>,
    /// Named groups of attributes that several payloads share.
    ///
    /// Optional: a file whose payloads have nothing in common, or that prefers
    /// to spell every one of them out, has no groups.
    #[serde(default)]
    pub payload_groups: Vec<DHGroupJson>,
    /// The payloads, each knowing which one of the file it is.
    #[serde(deserialize_with = "payloads_in_file_order")]
    pub payloads: Vec<DHConfigJson>,
}

impl PayloadConfig {
    /// The version this file states, as the three bytes a beacon carries.
    ///
    /// `None` for a file whose version is not one, two or three decimal
    /// parts, which [`Self::check`] refuses.
    pub fn config_version(&self) -> Option<crate::ConfigVersion> {
        crate::ConfigVersion::of_text(&self.version)
    }

    pub fn len(&self) -> usize {
        self.payloads.len()
    }

    /// Look up a data handler group by name.
    pub fn group(&self, name: &str) -> Option<&DHGroupJson> {
        self.payload_groups.iter().find(|g| g.name == name)
    }

    /// Settle every data handler into its runtime configuration.
    ///
    /// This is where a handler is laid over the group it names, so it is the
    /// only way to convert a file that uses groups. A group defined twice,
    /// named by a handler and defined nowhere, or defined and named by no
    /// handler is an error here rather than a handler quietly taking the wrong
    /// attributes or none at all.
    ///
    /// The first thing wrong with the file, which is what a program loading
    /// one can do something about: it cannot run, and which of several
    /// mistakes it names changes nothing. [`Self::problems`] is the same
    /// rules asked for all of their answers.
    pub fn to_dh_configs(&self) -> Result<Vec<DHConfig>, String> {
        let (configs, problems) = self.check();
        match problems.into_iter().next() {
            Some(problem) => Err(problem.message),
            None => Ok(configs),
        }
    }

    /// Everything wrong with this file, in the order the rules are asked.
    ///
    /// For a reader checking a file rather than a program running one: see
    /// [`crate::verify`]. Each problem says which payload or group it is
    /// about, which is how `tcsverify` finds the line it belongs to.
    ///
    /// One mistake can answer two rules, and both are reported. A payload
    /// that misspells its group's name both names a group that is not defined
    /// and leaves the group named by nobody; the file has one line to change
    /// and a reader is told where both ends of it are.
    pub fn problems(&self) -> Vec<Problem> {
        self.check().1
    }

    /// Every handler this file describes, and everything wrong with it.
    ///
    /// The handlers are the ones that converted, so a file with a problem still
    /// yields the payloads that have none -- which is what lets a simulator
    /// file be checked against a tcspecial configuration that is itself being
    /// reported on.
    pub fn check(&self) -> (Vec<DHConfig>, Vec<Problem>) {
        let mut problems: Vec<Problem> = Vec::new();

        // The version this file states, which every beacon carries as three
        // bytes: see `crate::BeaconTelemetry`. A file whose version cannot be
        // read as three decimal parts would be announced as something it is
        // not, so it is refused here rather than rounded to nought.
        if self.config_version().is_none() {
            problems.push(Problem::section(
                "version",
                format!(
                    "version \"{}\" is not one, two or three decimal parts, and a \
                     payload set says its version in every beacon",
                    self.version
                ),
            ));
        }

        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for group in &self.payload_groups {
            if !seen.insert(group.name.as_str()) {
                problems.push(Problem::group(
                    &group.name,
                    format!("payload group \"{}\" is defined more than once", group.name),
                ));
            }
        }

        // A payload is addressed by its name and found by its name, so a
        // name has to pick out one payload. A repeat parses cleanly and then
        // loses one: tcspecial keeps its handlers by name, so a repeated name
        // has one silently replace the other, and a simulator configuration
        // is joined to this one by name, so it would drive two payloads from
        // one entry.
        let mut by_name: BTreeSet<&str> = BTreeSet::new();
        for payload in &self.payloads {
            if !by_name.insert(payload.name.as_str()) {
                problems.push(Problem::payload(
                    &payload.name,
                    format!("payload \"{}\" is defined more than once", payload.name),
                ));
            }
        }

        let mut configs: Vec<DHConfig> = Vec::new();
        for dh in &self.payloads {
            let group = match &dh.group {
                Some(name) => match self.group(name) {
                    Some(group) => Some(group),
                    None => {
                        problems.push(Problem::payload(
                            &dh.name,
                            format!(
                                "data handler \"{}\" names group \"{}\", which is not \
                                 defined",
                                dh.name, name
                            ),
                        ));
                        continue;
                    }
                },
                None => None,
            };

            match dh.to_dh_config_in(group) {
                Ok(mut config) => {
                    // Where this payload exchanges data with the ground:
                    // tcspecial's own base counted on by the payload's id,
                    // which is the position it has in this file. tcspecial's
                    // and not the MOC's, because this is the socket tcspecial
                    // *binds* -- the payload is the end that speaks first over
                    // UDP, so it is the end that has to be findable, and the
                    // MOC says hello to it. The MOC's own side is counted the
                    // same way from `payload_telemetry.dest_base`.
                    //
                    // One base and an arithmetic rule rather than an address
                    // and a port on every payload -- forty payloads were forty
                    // chances to repeat a port, and a repeated port is two
                    // payloads whose data arrives interleaved.
                    //
                    // A set with no tcspecial section has nowhere to send,
                    // which is what `oc: None` has always meant: a file
                    // describing payloads is complete without it, and a
                    // handler cannot be started without one.
                    if let Some(Ok(ci)) =
                        self.tcspecial.as_ref().map(CIConfigJson::to_ci_config)
                    {
                        match (
                            ci.payload_source_base.at(dh.sequence),
                            ci.payload_dest_base.at(dh.sequence),
                        ) {
                            (Ok(oc), Ok(ground)) => {
                                config.oc = Some(oc);
                                config.ground = Some(ground);
                            }
                            (Err(why), _) | (_, Err(why)) => {
                                problems.push(Problem::payload(&dh.name, why));
                                continue;
                            }
                        }
                    }
                    configs.push(config);
                }
                Err(message) => problems.push(Problem::payload(&dh.name, message)),
            }
        }

        // Over the handlers that converted. A payload the rules refused is
        // not a payload claiming anything, so leaving it out is the only way
        // this stage can say something true.
        for (name, message) in collisions(&configs) {
            problems.push(Problem::payload(&name.0, message));
        }

        // And against the command links, when the file says where they are.
        // A handler on a command port is the one collision that does not stop
        // a handler from working: it stops the commanding that would have
        // started it.
        if let Some(Ok(ci)) = self.tcspecial.as_ref().map(CIConfigJson::to_ci_config) {
            for (name, message) in interpreter_collisions(&ci, &configs) {
                problems.push(Problem::payload(&name.0, message));
            }
        }

        // The tcspecial section is checked here whether or not anything reads
        // the attribute in question.
        // A section a file states is a section its author meant, and one that
        // could never be used -- a protocol that is not a protocol, say -- is
        // better refused by whoever loads the file than discovered by whoever
        // first tries to use it.
        if let Some(ci) = &self.tcspecial {
            if let Err(e) = ci.to_ci_config() {
                problems.push(Problem::section("tcspecial", format!("tcspecial: {e}")));
            }
        }

        // A group no handler names has no effect on the configuration, which
        // is exactly what a group whose name a handler misspelled looks like.
        // Checked after the handlers, so that a reader following the problems
        // in order meets the misspelling at the handler's end first, where
        // the name actually is.
        let named: BTreeSet<&str> = self
            .payloads
            .iter()
            .filter_map(|dh| dh.group.as_deref())
            .collect();
        for unused in self
            .payload_groups
            .iter()
            .filter(|group| !named.contains(group.name.as_str()))
        {
            // Worded exactly as the simulator configuration format words the
            // same rule, so that one rule reads as one rule wherever it is
            // met.
            problems.push(Problem::group(
                &unused.name,
                format!(
                    "data handler group \"{}\" is named by no data handler: name it \
                     from one, or remove the group",
                    unused.name
                ),
            ));
        }

        (configs, problems)
    }
}

/// A named group of attributes that several data handlers share.
///
/// Every attribute is optional, so a group carries exactly what its handlers
/// have in common and no more. What a group deliberately cannot carry is a
/// `name`: that is what tells one handler of a group from another, and so
/// belongs to the handler.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DHGroupJson {
    /// Name data handlers use to refer to this group.
    pub name: String,
    #[serde(rename = "type", default)]
    pub dh_type: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
    /// Where the payloads of this group are; see
    /// [`DHConfigJson::payload_address`] and
    /// [`DHConfigJson::payload_interface`].
    #[serde(default)]
    pub payload_address: Option<String>,
    #[serde(default)]
    pub payload_interface: Option<String>,
    #[serde(default)]
    pub payload_port: Option<u16>,
    /// How long the ground waits for these payloads' data before calling it
    /// stale, as a length of time: `3 s`. See [`DHConfig::stale_interval_ms`],
    /// which is the same figure in the milliseconds the ground counts in.
    #[serde(default)]
    pub stale_interval: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub packet_size: Option<usize>,
    /// ``periodic`` or ``triggered``; see [`DHMode`]. Absent is periodic.
    #[serde(default)]
    pub mode: Option<String>,
    /// What to send to make a triggered payload answer. Triggered only.
    #[serde(default)]
    pub trigger: Option<String>,
    /// How often to send it, as a length of time: `500 ms`. Triggered only.
    #[serde(default)]
    pub packet_interval: Option<String>,
    /// Named here only to be refused, with the four others whose name said
    /// what unit they were in.
    ///
    /// A length of time says its own unit now, so the name does not have to:
    /// `packet_interval: 500 ms` where this was `packet_interval_ms: 500`. A
    /// file written for the older spelling is told the name the attribute has
    /// now rather than told its word is unknown, because the value changes
    /// with the name and a reader who was told only "unknown" would write the
    /// number again.
    #[serde(default)]
    pub packet_interval_ms: Option<u32>,
    /// Named here only to be refused; see [`DHConfigJson::packet_interval_ms`].
    #[serde(default)]
    pub trigger_interval_ms: Option<u32>,
    #[serde(default)]
    pub stale_interval_ms: Option<u32>,
    /// How fast a serial line runs. Serial payloads only.
    #[serde(default)]
    pub datarate: Option<Scalar>,
    /// Which electrical standard a synchronous line's port is wired as, how
    /// fast this end clocks it, and what checks a frame. HDLC payloads only,
    /// and experimental; see [`crate::node::payload::hdlc`].
    #[serde(default)]
    pub standard: Option<Scalar>,
    #[serde(default)]
    pub clock_rate: Option<Scalar>,
    #[serde(default)]
    pub frame_check: Option<Scalar>,
    /// Stop bits per character, on an asynchronous line.
    #[serde(default)]
    pub stop_bits: Option<Scalar>,
    /// The parity of an asynchronous line, or the frame check of a
    /// synchronous one: the kernel calls both a line's parity.
    #[serde(default)]
    pub parity: Option<Scalar>,
    /// Where a synchronous line's clock comes from.
    #[serde(default)]
    pub clock_type: Option<Scalar>,
    /// How a synchronous line encodes bits on the wire.
    #[serde(default)]
    pub encoding: Option<Scalar>,
    /// Whether a synchronous line is looped back.
    #[serde(default)]
    pub loopback: Option<Scalar>,
    /// Bits in a character on a serial line, or in a word on a SPI bus.
    #[serde(default)]
    pub byte_length: Option<Scalar>,
    /// Whether an I2C bus addresses its devices with ten bits.
    #[serde(default)]
    pub ten_bit: Option<Scalar>,
    /// Whether an I2C bus checks packets. I2C payloads only.
    #[serde(default)]
    pub pec: Option<Scalar>,
    /// How many times an I2C transfer is retried.
    #[serde(default)]
    pub retries: Option<Scalar>,
    /// How long an I2C transfer waits.
    #[serde(default)]
    pub timeout: Option<Scalar>,
    /// No longer a term: see [`DHConfigJson::a_term_nothing_applies`].
    #[serde(default)]
    pub bus_speed: Option<Scalar>,
    /// The fastest a SPI peripheral may be clocked. SPI payloads only.
    #[serde(default)]
    pub max_speed: Option<Scalar>,
    /// Which of the four SPI clock modes a peripheral expects.
    ///
    /// `spi_mode` rather than `mode`, which in this language says whether a
    /// payload sends of its own accord or answers requests.
    #[serde(default)]
    pub spi_mode: Option<Scalar>,
    /// Bits in one SPI word.
    #[serde(default)]
    pub bits_per_word: Option<Scalar>,
    /// Which end of a SPI word goes first.
    #[serde(default)]
    pub bit_order: Option<Scalar>,
    /// Whether a SPI chip select is active high or low.
    #[serde(default)]
    pub cs_active: Option<Scalar>,
    /// Whether a start-stop line is a half-duplex RS-485 bus. Serial
    /// payloads only, and experimental; see
    /// [`crate::node::payload::serial::rs485`].
    #[serde(default)]
    pub rs485: Option<Scalar>,
    /// The logical level of RTS while an RS-485 line transmits.
    #[serde(default)]
    pub rs485_rts_on_send: Option<Scalar>,
    /// How long an RS-485 line holds the bus before its first bit, as a
    /// length of time: `2 ms`.
    #[serde(default)]
    pub rs485_delay_before_send: Option<Scalar>,
    /// And how long it holds the bus after its last.
    #[serde(default)]
    pub rs485_delay_after_send: Option<Scalar>,
    /// Named here only to be refused: the names these had when the name said
    /// the unit. See [`DHConfigJson::packet_interval_ms`].
    #[serde(default)]
    pub rs485_delay_before_send_ms: Option<Scalar>,
    #[serde(default)]
    pub rs485_delay_after_send_ms: Option<Scalar>,
    /// The network interface a CAN payload is on: `can0`, or `vcan0` for the
    /// kernel's virtual bus. CAN payloads only, and experimental; see
    /// [`crate::node::payload::can`].
    #[serde(default)]
    pub interface: Option<String>,
    /// The identifier a CAN payload's frames carry. CAN payloads only.
    #[serde(default)]
    pub can_id: Option<Scalar>,
    /// No longer a term: see [`DHConfigJson::a_term_nothing_applies`].
    #[serde(default)]
    pub bitrate: Option<Scalar>,
    /// Whether a CAN payload's frames are CAN FD rather than classic CAN.
    #[serde(default)]
    pub can_fd: Option<Scalar>,
    /// No longer terms: see [`DHConfigJson::a_term_nothing_applies`].
    #[serde(default)]
    pub channels: Option<Scalar>,
    #[serde(default)]
    pub sample_rate: Option<Scalar>,
    /// The service name an rpmsg channel was announced with. rpmsg payloads
    /// only, and experimental; see [`crate::node::payload::rpmsg`].
    #[serde(default)]
    pub service: Option<Scalar>,
    /// The greatest number of bytes one rpmsg message carries.
    #[serde(default)]
    pub max_message_bytes: Option<Scalar>,
    /// Which discrete lines a GPIO payload watches: offsets, separated by
    /// commas or spaces. GPIO payloads only, and experimental; see
    /// [`crate::node::payload::gpio`].
    #[serde(default)]
    pub lines: Option<Scalar>,
    /// Which changes of a line are events: rising, falling or both.
    #[serde(default)]
    pub edge: Option<Scalar>,
    /// What holds a line nothing is driving: none, pull_up or pull_down.
    #[serde(default)]
    pub bias: Option<Scalar>,
    /// How long a line must be steady before a change counts, as a length of
    /// time: `50 us`.
    #[serde(default)]
    pub debounce: Option<Scalar>,
    /// Named here only to be refused; see
    /// [`DHConfigJson::packet_interval_ms`].
    #[serde(default)]
    pub debounce_us: Option<Scalar>,
    /// Whether a set of discrete lines is read or driven.
    #[serde(default)]
    pub direction: Option<Scalar>,
    /// Where one read of a stream ends. Stream links only.
    #[serde(default)]
    pub stream: Option<StreamJson>,
}

/// The rule for where one read of a stream ends.
///
/// A nested section rather than flattened attributes, because that is what it
/// is: three settings that only mean anything together, and only for a link
/// that carries a stream. XML nests it as a child element, as it does the
/// `tcspecial` section.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StreamJson {
    /// The most bytes one read returns.
    #[serde(default)]
    pub max_length: Option<Scalar>,
    /// How long a read waits, or `none` for one that does not.
    #[serde(default)]
    pub timeout: Option<Scalar>,
    /// Bytes that end a read wherever they appear.
    ///
    /// A sequence -- `[0x0D, 0x0A]` -- or one string of them, which is the
    /// form XML has to use.
    #[serde(default)]
    pub terminators: Option<ByteList>,
}

/// The file-level form of a data handler's configuration
///
/// Every attribute but `name` is optional, because a handler naming a group
/// need only state what it does not take from that group.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DHConfigJson {
    pub name: String,
    /// Name of the group supplying the attributes this handler does not state.
    #[serde(default)]
    pub group: Option<String>,
    #[serde(rename = "type", default)]
    pub dh_type: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
    /// Where the payload itself is, for a payload that is not on a network:
    /// the address a peripheral has on its bus, or the interface a CAN
    /// payload is on.
    ///
    /// Named for the payload because that is whose address it is. The ground
    /// link is not here and cannot be: it is `payload_telemetry` counted on by
    /// the payload's own id, which only the file as a whole knows.
    #[serde(default)]
    pub payload_address: Option<String>,
    /// Where a payload on a network is: `address:port`, or the path of a Unix
    /// socket, which has no port.
    ///
    /// One attribute and not two, as every link in the `tcspecial` section
    /// is: an address and a port only ever used together are one fact about a
    /// payload, and two attributes are two chances to state half of it. A
    /// network payload states this and not `payload_address`, and every other
    /// kind the other way about -- each kind is told by name which of the two
    /// is its own.
    #[serde(default)]
    pub payload_interface: Option<String>,
    /// Named here only to be refused.
    ///
    /// A network payload's port is the second half of `payload_interface`
    /// now, and no other kind of payload has one at all. Kept as a field
    /// rather than left unknown so that the refusal can say where the port
    /// went.
    #[serde(default)]
    pub payload_port: Option<u16>,
    /// How long the ground waits for this payload's data before calling it
    /// stale, as a length of time: `3 s`. See [`DHConfig::stale_interval_ms`],
    /// which is the same figure in the milliseconds the ground counts in.
    #[serde(default)]
    pub stale_interval: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub packet_size: Option<usize>,
    /// ``periodic`` or ``triggered``; see [`DHMode`]. Absent is periodic.
    #[serde(default)]
    pub mode: Option<String>,
    /// What to send to make a triggered payload answer. Triggered only.
    #[serde(default)]
    pub trigger: Option<String>,
    /// How often to send it, as a length of time: `500 ms`. Triggered only.
    #[serde(default)]
    pub packet_interval: Option<String>,
    /// Named here only to be refused; see [`DHGroupJson::packet_interval_ms`].
    #[serde(default)]
    pub packet_interval_ms: Option<u32>,
    #[serde(default)]
    pub trigger_interval_ms: Option<u32>,
    #[serde(default)]
    pub stale_interval_ms: Option<u32>,
    /// How fast a serial line runs. Serial payloads only.
    #[serde(default)]
    pub datarate: Option<Scalar>,
    /// Which electrical standard a synchronous line's port is wired as, how
    /// fast this end clocks it, and what checks a frame. HDLC payloads only,
    /// and experimental; see [`crate::node::payload::hdlc`].
    #[serde(default)]
    pub standard: Option<Scalar>,
    #[serde(default)]
    pub clock_rate: Option<Scalar>,
    #[serde(default)]
    pub frame_check: Option<Scalar>,
    /// Stop bits per character, on an asynchronous line.
    #[serde(default)]
    pub stop_bits: Option<Scalar>,
    /// The parity of an asynchronous line, or the frame check of a
    /// synchronous one: the kernel calls both a line's parity.
    #[serde(default)]
    pub parity: Option<Scalar>,
    /// Where a synchronous line's clock comes from.
    #[serde(default)]
    pub clock_type: Option<Scalar>,
    /// How a synchronous line encodes bits on the wire.
    #[serde(default)]
    pub encoding: Option<Scalar>,
    /// Whether a synchronous line is looped back.
    #[serde(default)]
    pub loopback: Option<Scalar>,
    /// Bits in a character on a serial line, or in a word on a SPI bus.
    #[serde(default)]
    pub byte_length: Option<Scalar>,
    /// Whether an I2C bus addresses its devices with ten bits.
    #[serde(default)]
    pub ten_bit: Option<Scalar>,
    /// Whether an I2C bus checks packets. I2C payloads only.
    #[serde(default)]
    pub pec: Option<Scalar>,
    /// How many times an I2C transfer is retried.
    #[serde(default)]
    pub retries: Option<Scalar>,
    /// How long an I2C transfer waits.
    #[serde(default)]
    pub timeout: Option<Scalar>,
    /// No longer a term: see [`DHConfigJson::a_term_nothing_applies`].
    #[serde(default)]
    pub bus_speed: Option<Scalar>,
    /// The fastest a SPI peripheral may be clocked. SPI payloads only.
    #[serde(default)]
    pub max_speed: Option<Scalar>,
    /// Which of the four SPI clock modes a peripheral expects.
    ///
    /// `spi_mode` rather than `mode`, which in this language says whether a
    /// payload sends of its own accord or answers requests.
    #[serde(default)]
    pub spi_mode: Option<Scalar>,
    /// Bits in one SPI word.
    #[serde(default)]
    pub bits_per_word: Option<Scalar>,
    /// Which end of a SPI word goes first.
    #[serde(default)]
    pub bit_order: Option<Scalar>,
    /// Whether a SPI chip select is active high or low.
    #[serde(default)]
    pub cs_active: Option<Scalar>,
    /// Whether a start-stop line is a half-duplex RS-485 bus. Serial
    /// payloads only, and experimental; see
    /// [`crate::node::payload::serial::rs485`].
    #[serde(default)]
    pub rs485: Option<Scalar>,
    /// The logical level of RTS while an RS-485 line transmits.
    #[serde(default)]
    pub rs485_rts_on_send: Option<Scalar>,
    /// How long an RS-485 line holds the bus before its first bit, as a
    /// length of time: `2 ms`.
    #[serde(default)]
    pub rs485_delay_before_send: Option<Scalar>,
    /// And how long it holds the bus after its last.
    #[serde(default)]
    pub rs485_delay_after_send: Option<Scalar>,
    /// Named here only to be refused: the names these had when the name said
    /// the unit. See [`DHConfigJson::packet_interval_ms`].
    #[serde(default)]
    pub rs485_delay_before_send_ms: Option<Scalar>,
    #[serde(default)]
    pub rs485_delay_after_send_ms: Option<Scalar>,
    /// The network interface a CAN payload is on: `can0`, or `vcan0` for the
    /// kernel's virtual bus. CAN payloads only, and experimental; see
    /// [`crate::node::payload::can`].
    #[serde(default)]
    pub interface: Option<String>,
    /// The identifier a CAN payload's frames carry. CAN payloads only.
    #[serde(default)]
    pub can_id: Option<Scalar>,
    /// No longer a term: see [`DHConfigJson::a_term_nothing_applies`].
    #[serde(default)]
    pub bitrate: Option<Scalar>,
    /// Whether a CAN payload's frames are CAN FD rather than classic CAN.
    #[serde(default)]
    pub can_fd: Option<Scalar>,
    /// No longer terms: see [`DHConfigJson::a_term_nothing_applies`].
    #[serde(default)]
    pub channels: Option<Scalar>,
    #[serde(default)]
    pub sample_rate: Option<Scalar>,
    /// The service name an rpmsg channel was announced with. rpmsg payloads
    /// only, and experimental; see [`crate::node::payload::rpmsg`].
    #[serde(default)]
    pub service: Option<Scalar>,
    /// The greatest number of bytes one rpmsg message carries.
    #[serde(default)]
    pub max_message_bytes: Option<Scalar>,
    /// Which discrete lines a GPIO payload watches: offsets, separated by
    /// commas or spaces. GPIO payloads only, and experimental; see
    /// [`crate::node::payload::gpio`].
    #[serde(default)]
    pub lines: Option<Scalar>,
    /// Which changes of a line are events: rising, falling or both.
    #[serde(default)]
    pub edge: Option<Scalar>,
    /// What holds a line nothing is driving: none, pull_up or pull_down.
    #[serde(default)]
    pub bias: Option<Scalar>,
    /// How long a line must be steady before a change counts, as a length of
    /// time: `50 us`.
    #[serde(default)]
    pub debounce: Option<Scalar>,
    /// Named here only to be refused; see
    /// [`DHConfigJson::packet_interval_ms`].
    #[serde(default)]
    pub debounce_us: Option<Scalar>,
    /// Whether a set of discrete lines is read or driven.
    #[serde(default)]
    pub direction: Option<Scalar>,
    /// Where one read of a stream ends. Stream links only.
    #[serde(default)]
    pub stream: Option<StreamJson>,
    /// Which payload of the file this is: the first is 0.
    ///
    /// Numbered as the list is read -- see `payloads_in_file_order` -- on the
    /// assumption that a parser hands entries over in the order the file gave
    /// them, which every format here does.
    ///
    /// It is here because that order is configuration rather than spelling:
    /// ids will be assigned in the order the payloads appear. Recording it
    /// means the order is carried by the data instead of living in whatever
    /// order a `Vec` happens to keep, so the configuration digest can walk
    /// the payloads by sequence number and two ends that read the same file
    /// agree about which payload came first -- whatever either of them has
    /// since done with its own list.
    ///
    /// A sequence number and not a line number. A line is not something these
    /// parsers hand to a `Deserialize`, and it would be a different number in
    /// each of the three formats one payload set is written in, where the
    /// sequence number is the same in all three.
    ///
    /// Written out when this struct is, and never read in: a file that stated
    /// it would be refused as naming an attribute that does not exist.
    #[serde(default, skip_deserializing)]
    pub sequence: usize,
}

/// Deserialize the payloads, numbering each as it is read.
///
/// Here rather than in a pass afterwards, so that nothing can hold a
/// `PayloadConfig` whose payloads do not know which ones they are. Every
/// format arrives through this one function.
fn payloads_in_file_order<'de, D>(d: D) -> Result<Vec<DHConfigJson>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let mut payloads = Vec::<DHConfigJson>::deserialize(d)?;
    for (sequence, payload) in payloads.iter_mut().enumerate() {
        payload.sequence = sequence;
    }
    Ok(payloads)
}

impl DHConfigJson {
    /// Convert a handler that states every attribute for itself.
    ///
    /// A handler naming a group needs that group's attributes, which only the
    /// file as a whole knows; use [`PayloadConfig::to_dh_configs`] for those.
    pub fn to_dh_config(&self) -> Result<DHConfig, String> {
        self.to_dh_config_in(None)
    }

    /// Convert a handler, taking from `group` whatever the handler does not
    /// state itself.
    pub fn to_dh_config_in(&self, group: Option<&DHGroupJson>) -> Result<DHConfig, String> {
        // The handler wins wherever it says anything, so a group holds what
        // its handlers share without preventing one of them from differing.
        let dh_type = self
            .dh_type
            .as_deref()
            .or_else(|| group.and_then(|g| g.dh_type.as_deref()));
        let protocol = self
            .protocol
            .as_deref()
            .or_else(|| group.and_then(|g| g.protocol.as_deref()));
        let address = self
            .payload_address
            .as_deref()
            .or_else(|| group.and_then(|g| g.payload_address.as_deref()));
        let payload_interface = self
            .payload_interface
            .as_deref()
            .or_else(|| group.and_then(|g| g.payload_interface.as_deref()));
        let path = self
            .path
            .as_deref()
            .or_else(|| group.and_then(|g| g.path.as_deref()));
        let packet_size = self
            .packet_size
            .or_else(|| group.and_then(|g| g.packet_size));
        // A packet of no bytes is not a packet, and the sizes and rates a
        // link states refuse zero for the same reason. The rule used to be
        // asked only of an endpoint configuration's groups, which is where a
        // packet size was stated as text and checked as it was read; here it
        // is a number already, so this is the only place to ask.
        if packet_size == Some(0) {
            return Err(format!(
                "payload \"{}\": packet_size must be greater than zero",
                self.name
            ));
        }
        let mode = self
            .mode
            .as_deref()
            .or_else(|| group.and_then(|g| g.mode.as_deref()));
        let trigger = self
            .trigger
            .as_deref()
            .or_else(|| group.and_then(|g| g.trigger.as_deref()));
        // The rate a triggered payload's trigger goes out at, which is the
        // one interval this file states. A length of time in the file and
        // milliseconds below it: the protocol counts in those, and the one
        // grammar for a length of time is `graph::parse_interval`.
        let stated = |field: Option<&String>, group: Option<&String>| field.or(group).cloned();
        let packet_interval = stated(
            self.packet_interval.as_ref(),
            group.and_then(|g| g.packet_interval.as_ref()),
        );
        let packet_interval = match packet_interval.as_deref() {
            Some(said) => Some(as_length_of_time("packet_interval", said)?),
            None => None,
        };

        let kind = match dh_type {
            Some(text) => DHType::from_spelling(text).ok_or_else(|| {
                match DHType::renamed(text) {
                    Some(now) => format!(
                        "{text} is the name a {now} payload used to have, and {now} is \
                         the name it has now: \"device\" meant a file in the filesystem \
                         and a piece of hardware on a bus at once"
                    ),
                    None => format!(
                        "{text} is not a kind of payload: expected {}",
                        DHType::spellings()
                    ),
                }
            })?,
            None => return Err("Missing type".to_string()),
        };

        // An attribute of another kind is an error rather than something
        // ignored. Ignoring one is how a device payload comes to carry a
        // port nothing reads, and
        // how a network payload carrying a path looks configured and is not.
        // Checked after the group has been laid under the payload, because an
        // attribute inherited from a group reaches the handler exactly as one
        // the payload states does.
        // Each entry is the attribute, whether this payload stated it, and
        // whether it is there at all once the group is under it.
        // What locates a payload differs by kind, so what does not apply to
        // it does too. A network payload is a host and a port; everything
        // else is a device node -- and a device on an I2C bus is that node
        // and an address on it, which is the one other kind that has an
        // address at all. The attributes of a *link* -- a data rate, a clock
        // mode -- are refused by the rules for each kind instead; see
        // `link_of`.
        //
        // A stream rule -- where one read ends -- is a line's, and the two
        // kinds that are not on a link at all are told so here. A bus and a
        // peripheral are told by the rules for their kind, which have always
        // refused one: a master clocks exactly the bytes it asks for.
        // A word that was a term once and is not any more, before anything
        // else is read: a file that states one is told which word and why,
        // rather than having the value taken and never applied.
        if let Some((field, why)) = self.a_term_nothing_applies(group) {
            return Err(format!(
                "payload \"{}\": {field} is not a term any more -- {why}, so nothing \
                 here could apply what the file says. A payload states only what it \
                 can set",
                self.name
            ));
        }

        // A port stated on a payload, which is the second half of a network
        // payload's payload_interface now and nothing at all to any other
        // kind. Said by name rather than left to the table below, because the
        // remedy is to join it to the address rather than to delete it.
        if self.payload_port.is_some() || group.is_some_and(|g| g.payload_port.is_some()) {
            return Err(format!(
                "payload \"{}\": payload_port is not an attribute any more -- a \
                 payload on a network states payload_interface, which is its \
                 address and its port with a colon between them, and no other \
                 kind of payload has a port at all",
                self.name
            ));
        }

        let stream = self.stream.is_some() || group.is_some_and(|g| g.stream.is_some());
        let on_a_network = (
            "payload_interface",
            self.payload_interface.is_some(),
            payload_interface.is_some(),
        );
        let its_own_address = (
            "payload_address",
            self.payload_address.is_some(),
            address.is_some(),
        );
        let foreign: &[(&str, bool, bool)] = match kind {
            // A payload on a network says where it is in one attribute, so
            // the address by itself is not its own.
            DHType::Network => &[
                ("path", self.path.is_some(), path.is_some()),
                its_own_address,
                ("stream", self.stream.is_some(), stream),
            ],
            DHType::Device => &[
                ("protocol", self.protocol.is_some(), protocol.is_some()),
                its_own_address,
                on_a_network,
                ("stream", self.stream.is_some(), stream),
            ],
            DHType::I2c => &[
                ("protocol", self.protocol.is_some(), protocol.is_some()),
                on_a_network,
            ],
            // A CAN payload is on an interface: it has no path, no host and
            // no port, and a frame is its own length so it has no stream
            // rule either.
            DHType::Can => &[
                ("protocol", self.protocol.is_some(), protocol.is_some()),
                its_own_address,
                on_a_network,
                ("path", self.path.is_some(), path.is_some()),
                ("stream", self.stream.is_some(), stream),
            ],
            _ => &[
                ("protocol", self.protocol.is_some(), protocol.is_some()),
                its_own_address,
                on_a_network,
            ],
        };
        // And the two attributes that locate a CAN payload, which belong to
        // no other kind: a line stating an interface has been written for a
        // bus it is not on.
        let on_a_can_bus = kind == DHType::Can;
        let can_only: &[(&str, bool, bool)] = &[
            (
                "interface",
                self.interface.is_some(),
                self.interface.is_some() || group.is_some_and(|g| g.interface.is_some()),
            ),
            (
                "can_id",
                self.can_id.is_some(),
                self.can_id.is_some() || group.is_some_and(|g| g.can_id.is_some()),
            ),
        ];
        let foreign: &[(&str, bool, bool)] = if on_a_can_bus {
            foreign
        } else {
            &[foreign, can_only].concat()
        };

        // And the terms of a link, for the two kinds that are not on one. Every
        // other kind refuses what belongs to another kind as its own rules are
        // asked -- see link_params_of -- and these two have no rules to ask,
        // so a term written on one of them used to be read and dropped: a
        // device payload stating a data rate, or a network payload stating a
        // sensor's channels, looked configured and was not. A network
        // payload's own `protocol` is a term of its kind and is allowed.
        if matches!(kind, DHType::Network | DHType::Device) {
            let allowed: &[&str] = match kind {
                DHType::Network => &["protocol"],
                _ => &[],
            };
            let ours = link_terms_of(self, group);
            if let Some(field) = crate::endpoint_config::a_link_term(&ours, allowed) {
                // Where it came from, as above: one line is deleted from the
                // payload, and the other means a payload is in a group that
                // was not written for it.
                let mine = link_terms_of(self, None);
                let whence = match (
                    crate::endpoint_config::a_link_term(&mine, allowed),
                    group,
                ) {
                    (None, Some(group)) => {
                        format!(", which it takes from group \"{}\"", group.name)
                    }
                    _ => String::new(),
                };
                return Err(format!(
                    "payload \"{}\" is {} {} payload, so {field}{whence} does not \
                     apply to it",
                    self.name,
                    kind.article(),
                    kind.spelling()
                ));
            }
        }

        if let Some((field, stated, _)) = foreign.iter().find(|(_, _, present)| *present) {
            // Where it came from, because the two are fixed differently: one
            // line is deleted from the payload, and the other means a payload
            // is in a group that was not written for it.
            let whence = match (stated, group) {
                (false, Some(group)) => format!(", which it takes from group \"{}\"", group.name),
                _ => String::new(),
            };
            return Err(format!(
                "payload \"{}\" is {} {} payload, so {field}{whence} does not apply to it",
                self.name,
                kind.article(),
                kind.spelling()
            ));
        }

        let endpoint = match kind {
            DHType::Network => {
                let protocol = match protocol {
                    Some(text) => NetworkProtocol::from_spelling(text).ok_or_else(|| {
                        format!(
                            "{text} is not a protocol: expected {}",
                            NetworkProtocol::spellings()
                        )
                    })?,
                    None => return Err("Invalid or missing protocol".to_string()),
                };
                // One attribute, split where the protocol has a port to
                // split off. A Unix socket is a path and has none, so the
                // whole of it is the address -- which is why this is not
                // `host_port_of`: that is for the links in the tcspecial
                // section, every one of which has a port.
                let said = payload_interface.ok_or_else(|| {
                    format!(
                        "payload \"{}\" is on a network, so it needs a \
                         payload_interface saying where: address:port, or the \
                         path of a Unix socket",
                        self.name
                    )
                })?;
                let (address, port) = match protocol {
                    NetworkProtocol::UnixStream | NetworkProtocol::UnixDgram => {
                        (said.trim().to_string(), 0)
                    }
                    NetworkProtocol::Tcp | NetworkProtocol::Udp => {
                        let (address, port) = said.trim().rsplit_once(':').ok_or_else(|| {
                            format!(
                                "payload \"{}\": payload_interface \"{said}\" is not \
                                 an address and a port: it is written as \
                                 address:port, as localhost:5000",
                                self.name
                            )
                        })?;
                        let port: u16 = port.trim().parse().map_err(|_| {
                            format!(
                                "payload \"{}\": payload_interface \"{said}\" ends in \
                                 \"{port}\", which is not a port: a port is a whole \
                                 number between 1 and 65535",
                                self.name
                            )
                        })?;
                        if address.trim().is_empty() {
                            return Err(format!(
                                "payload \"{}\": payload_interface \"{said}\" states no \
                                 address",
                                self.name
                            ));
                        }
                        (address.trim().to_string(), port)
                    }
                };
                EndpointConfig::Network(NetworkConfig {
                    protocol,
                    address,
                    port,
                })
            }
            DHType::Device => EndpointConfig::Device(DeviceConfig {
                path: path.ok_or("Missing path")?.to_string(),
            }),
            // A line, a bus and a peripheral, each of which carries
            // attributes of its own: a data rate, a slave address, a clock
            // mode. See link_of, which is where every rule about them lives.
            kind @ (DHType::Serial
            | DHType::I2c
            | DHType::Spi
            | DHType::Can
            | DHType::Iio
            | DHType::Rpmsg
            | DHType::Gpio
            | DHType::Hdlc) => {
                link_of(&self.name, kind, self, group)?
            }
        };

        // A CAN frame is its own length and there is no fragmentation: a
        // classic frame carries eight bytes and an FD frame sixty-four,
        // exactly. A payload whose packets will not fit in one is refused
        // here rather than found out a frame at a time at run time.
        if let EndpointConfig::Can(can) = &endpoint {
            let room = crate::node::payload::can::frame_bytes(can.fd);
            if let Some(packet_size) = packet_size {
                if packet_size > room {
                    return Err(format!(
                        "payload \"{}\": packet_size {packet_size} will not fit in a \
                         {} frame, which carries {room} bytes",
                        self.name,
                        if can.fd { "CAN FD" } else { "classic CAN" }
                    ));
                }
            }
        }

        // A GPIO payload's packets are events this handler makes rather than
        // bytes anything sent, and an event is a fixed size: a packet that
        // was part of a record would be a reading nobody could use.
        if let EndpointConfig::Gpio(_) = &endpoint {
            let event = crate::node::payload::gpio::EVENT_BYTES;
            if let Some(packet_size) = packet_size {
                if packet_size == 0 || packet_size % event != 0 {
                    return Err(format!(
                        "payload \"{}\": packet_size {packet_size} is not a whole \
                         number of {event}-byte line events, which is what a gpio \
                         payload's packets are made of",
                        self.name
                    ));
                }
            }
        }

        // An rpmsg message goes in a buffer the transport fixes the size of,
        // and rpmsg does not fragment: a payload whose packets are larger
        // would have every write refused by the driver.
        if let EndpointConfig::Rpmsg(rpmsg) = &endpoint {
            let room = crate::node::payload::rpmsg::message_bytes(rpmsg.max_message_bytes);
            if let Some(packet_size) = packet_size {
                if packet_size > room as usize {
                    return Err(format!(
                        "payload \"{}\": packet_size {packet_size} will not fit in an \
                         rpmsg message, which carries {room} bytes",
                        self.name
                    ));
                }
            }
        }

        // Where this payload exchanges data with the ground is not stated on
        // the payload at all: it is the `payload_telemetry.dest_base` of the
        // tcspecial section counted on by the payload's id, which only the
        // file as a whole
        // knows. `PayloadConfig::check` fills it in; this path is for a payload
        // converted on its own, which has no section to ask.
        let oc = None;
        let ground = None;

        // The names these attributes had when the name said the unit, each
        // refused by name: a length of time says its own unit now, so the
        // value changes with the name and a reader told only "unknown" would
        // write the number again.
        for (old, now, example) in [
            ("packet_interval_ms", "packet_interval", "500 ms"),
            ("trigger_interval_ms", "packet_interval", "500 ms"),
            ("stale_interval_ms", "stale_interval", "3 s"),
            ("rs485_delay_before_send_ms", "rs485_delay_before_send", "2 ms"),
            ("rs485_delay_after_send_ms", "rs485_delay_after_send", "2 ms"),
            ("debounce_us", "debounce", "50 us"),
        ] {
            let said = match old {
                "packet_interval_ms" => self.packet_interval_ms.is_some(),
                "trigger_interval_ms" => self.trigger_interval_ms.is_some(),
                "stale_interval_ms" => self.stale_interval_ms.is_some(),
                "rs485_delay_before_send_ms" => self.rs485_delay_before_send_ms.is_some(),
                "rs485_delay_after_send_ms" => self.rs485_delay_after_send_ms.is_some(),
                _ => self.debounce_us.is_some(),
            };
            if said {
                return Err(format!(
                    "payload \"{}\" states {old}, which is {now} now and is a length \
                     of time: {example} rather than a bare number",
                    self.name
                ));
            }
        }

        // Which kind of payload this is, and the fields that belong to that
        // kind and to no other. A periodic payload carrying a trigger, or a
        // triggered one carrying no interval, is a file that has not decided
        // which kind it describes -- reported rather than resolved by
        // guessing, because either guess would run something nobody asked
        // for.
        let mode = match mode {
            None | Some("periodic") => {
                if trigger.is_some() || packet_interval.is_some() {
                    return Err(
                        "a periodic payload sends on its own, so it takes no trigger \
                         and no packet interval here: how fast a simulated one sends \
                         belongs in the simulator configuration"
                            .to_string(),
                    );
                }
                DHMode::Periodic
            }
            Some("triggered") => {
                // A datagram handler learns where its payload is from the
                // payload's first packet -- a datagram's sender is the only
                // statement of it -- so it has nowhere to send a trigger
                // until it has been spoken to, which is exactly what a
                // triggered payload will not do. Such a payload is periodic
                // whether the file says so or not, so the file is corrected
                // rather than run.
                if let Some(protocol) = protocol {
                    if matches!(protocol, "udp" | "unix_dgram") {
                        return Err(format!(
                            "a {protocol} payload cannot be triggered: its handler \
                             learns where to send from the payload's first packet, so \
                             there is nowhere to send a trigger until the payload has \
                             spoken -- which a triggered payload does not do"
                        ));
                    }
                }

                // Discrete lines are inputs: writing one would be driving
                // an output, which is commanding a payload rather than
                // asking it for data. So there is nowhere to send a trigger.
                if kind == DHType::Gpio {
                    return Err(
                        "a gpio payload cannot be triggered: its lines are inputs, \
                         and driving one would be commanding a payload rather than \
                         reading it"
                            .to_string(),
                    );
                }

                // A sample buffer is a stream out of a sensor: there is
                // nothing on that descriptor to write, and what would change
                // what it produces is in sysfs, which belongs to whoever
                // brought the sensor up. So there is nowhere to send a
                // trigger, and a file asking for one is corrected rather
                // than run.
                if kind == DHType::Iio {
                    return Err(
                        "an iio payload cannot be triggered: a sample buffer is read \
                         and not written, so there is nowhere to send a trigger -- a \
                         sensor's channels and rate are set through sysfs"
                            .to_string(),
                    );
                }

                let written = trigger.ok_or(
                    "a triggered payload answers a request, so it states the trigger \
                     to send",
                )?;
                // Either notation, hexadecimal or a C string; see
                // crate::trigger. The error says which was being read.
                let trigger = crate::trigger::trigger_bytes(written)?;

                let interval = packet_interval.ok_or(
                    "a triggered payload states how often its trigger is sent, as \
                     packet_interval: tcspecial does the sending, so the rate is \
                     flight behaviour and not a simulation setting",
                )?;
                DHMode::Triggered { trigger, interval }
            }
            Some(other) => {
                return Err(format!(
                    "{other} is not a mode: a payload is periodic or triggered"
                ))
            }
        };

        Ok(DHConfig {
            ground,
            name: DHName::new(&self.name),
            endpoint,
            packet_size: packet_size.ok_or("Missing packet_size")?,
            oc,
            mode,
            // Read by the ground alone, and taken from the group like
            // anything else a payload does not state for itself.
            stale_interval: match stated(
                self.stale_interval.as_ref(),
                group.and_then(|g| g.stale_interval.as_ref()),
            )
            .as_deref()
            {
                Some(said) => Some(as_length_of_time("stale_interval", said)?),
                None => None,
            },
        })
    }
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
    /// configuration -- see [`DHConfigJson::sequence`].
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
            "{attribute} \"{said}\" is not an address and a port: it is written \
             as address:port, as 0.0.0.0:4000"
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
             whatever port is free -- and a link nobody can predict the port of \
             is one the other end cannot reach"
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
/// names is a subsection somebody declined to write. The names under one are short for the same reason: `beacon.send`
/// says what `beacon_send` said, and says it where the reader is already
/// looking at the beacon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CIConfigJson {
    /// The spacecraft command link: both ends of it.
    pub tcs_cmd: TcsCmdJson,
    /// Where each payload's telemetry comes from and goes to.
    pub payload_telemetry: PayloadTelemetryJson,
    /// Where commands about a payload are taken, which is not the link above.
    ///
    /// A command about a payload -- start it, stop it, ask it for its
    /// statistics -- arrives on that payload's own socket here; a command
    /// about the spacecraft -- ping it, arm it, restart it -- arrives on
    /// `tcs_cmd.recv`. See [`crate::CommandType::is_payload_command`], which
    /// is the one table that says which is which, read by both ends.
    ///
    /// Separate links because what the ground needs in a hurry must never be
    /// queued behind what it asked for at leisure: a RESTART waiting behind a
    /// payload's statistics is a spacecraft that cannot be rescued while it is
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
        let beacon_interval = crate::graph::parse_interval(&self.beacon.interval)
            .map_err(|why| format!("beacon.interval: {why}"))?;
        if u32::try_from(beacon_interval.as_millis()).is_err() {
            return Err("beacon.interval: longer than the protocol can carry, which \
                        counts milliseconds in four bytes"
                .to_string());
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

    use crate::ConfigFormat;

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
        assert!(!came_from(&stated("127.0.0.1", 4100), &at("127.0.0.1:4101")));
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

    /// A tcspecial configuration that says nothing about the mode describes the
    /// kind of payload that sends on its own, which is what every file written
    /// before there was a mode describes.
    #[test]
    fn a_payload_that_states_no_mode_sends_on_its_own() {
        use super::*;

        let dh = DHConfigJson {
            name: "DH0".to_string(),
            group: None,
            dh_type: Some("device-file".to_string()),
            protocol: None,
            payload_address: None,
            payload_interface: None,
            payload_port: None,
            path: Some("/dev/urandom".to_string()),
            packet_size: Some(4),
            mode: None,
            trigger: None,
            trigger_interval_ms: None,
            packet_interval: None,
            ..Default::default()
        };

        assert_eq!(dh.to_dh_config().expect("converts").mode, DHMode::Periodic);
    }

    /// A triggered payload carries what to send and how often, and both reach
    /// the handler: tcspecial does the sending, so both are its business.
    #[test]
    fn a_triggered_payload_carries_its_trigger_and_interval() {
        use super::*;

        let dh = DHConfigJson {
            name: "DH1".to_string(),
            group: None,
            dh_type: Some("network".to_string()),
            protocol: Some("tcp".to_string()),
            payload_interface: Some("localhost:5000".to_string()),
            path: None,
            packet_size: Some(8),
            mode: Some("triggered".to_string()),
            // As a file writes it: a C string, quotes and all, where the
            // escape is the carriage return the payload's interface asks
            // for. The quotes are the mark that says this is text rather
            // than a list of byte values, so they are part of the value.
            trigger: Some("\"READ\r\"".to_string()),
            trigger_interval_ms: None,
            packet_interval: Some("500 ms".to_string()),
            ..Default::default()
        };

        let config = dh.to_dh_config().expect("converts");
        assert_eq!(
            config.mode,
            DHMode::Triggered {
                trigger: b"READ\r".to_vec(),
                interval: Duration::from_millis(500),
            }
        );
        assert_eq!(
            config.mode.polling(),
            Some((b"READ\r".as_slice(), Duration::from_millis(500)))
        );
    }

    /// The names that said their own unit are refused by name, each told the
    /// name it has now.
    ///
    /// The value changes with the name -- `500` became `500 ms` -- so a file
    /// told only that a word is unknown would write the number again under
    /// the new name and be wrong by a factor nobody would see.
    #[test]
    fn a_name_that_said_its_unit_is_told_the_name_it_has_now() {
        let payload_file = |extra: &str| {
            format!(
                "version: \"1.0\"
description: a payload
payloads:
  - name: DH0
    type: network
    protocol: tcp
    payload_interface: localhost:5000
    packet_size: 12
{extra}
"
            )
        };

        // The two any payload may state. The three that belong to a line or
        // to a set of discrete lines -- the two rs485 turnarounds and the
        // debounce -- go through the same loop, keyed by the same names.
        for (old, now) in [
            ("    packet_interval_ms: 500", "packet_interval"),
            ("    stale_interval_ms: 3000", "stale_interval"),
        ] {
            let said = payload(&payload_file(old))
                .to_dh_configs()
                .expect_err(&format!("{old} is not a word this file knows"));
            assert!(
                said.contains(now) && said.contains("length of time"),
                "{old}: {said}"
            );
        }
    }

    /// The two kinds do not mix, in either direction, and the complaint says
    /// which file the misplaced setting belongs in.
    #[test]
    fn the_two_kinds_of_payload_do_not_mix() {
        use super::*;

        let bare = |mode: Option<&str>, trigger: Option<&str>, interval: Option<&str>| {
            DHConfigJson {
                name: "DH2".to_string(),
                group: None,
                dh_type: Some("network".to_string()),
                protocol: Some("tcp".to_string()),
                payload_interface: Some("localhost:5000".to_string()),
                path: None,
                packet_size: Some(8),
                mode: mode.map(str::to_string),
                trigger: trigger.map(str::to_string),
                trigger_interval_ms: None,
                packet_interval: interval.map(str::to_string),
                ..Default::default()
            }
        };

        // A payload that sends on its own has nothing to be triggered by, and
        // its rate is the simulator's business.
        let e = bare(None, Some("READ"), None).to_dh_config().unwrap_err();
        assert!(e.contains("no trigger"), "{e}");
        let e = bare(Some("periodic"), None, Some("500 ms"))
            .to_dh_config()
            .unwrap_err();
        assert!(e.contains("simulator configuration"), "{e}");

        // A payload that answers requests needs both halves of the request.
        let e = bare(Some("triggered"), None, Some("500 ms"))
            .to_dh_config()
            .unwrap_err();
        assert!(e.contains("states the trigger"), "{e}");
        let e = bare(Some("triggered"), Some("\"READ\""), None)
            .to_dh_config()
            .unwrap_err();
        assert!(e.contains("how often"), "{e}");

        // And a mode that is neither.
        let e = bare(Some("occasional"), None, None)
            .to_dh_config()
            .unwrap_err();
        assert!(e.contains("periodic or triggered"), "{e}");
    }

    /// A datagram payload cannot be triggered, whatever the file says: its
    /// handler has nowhere to send a trigger until the payload has spoken,
    /// and a triggered payload does not speak first.
    #[test]
    fn a_datagram_payload_cannot_be_triggered() {
        use super::*;

        for protocol in ["udp", "unix_dgram"] {
            let dh = DHConfigJson {
                name: "DH3".to_string(),
                group: None,
                dh_type: Some("network".to_string()),
                protocol: Some(protocol.to_string()),
                payload_interface: Some("localhost:5000".to_string()),
                path: None,
                packet_size: Some(8),
                mode: Some("triggered".to_string()),
                trigger: Some("\"READ\"".to_string()),
                trigger_interval_ms: None,
                packet_interval: Some("500 ms".to_string()),
                ..Default::default()
            };

            let e = dh.to_dh_config().unwrap_err();
            assert!(
                e.contains("cannot be triggered") && e.contains(protocol),
                "{e}"
            );
        }
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
        assert!(!ts.clock_failed(), "the clock on the machine running this reads");
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
        let at = Timestamp { micros: 0x0807_0605_0403_0201 };
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
        sample.time = Some(Timestamp {
            micros: 0,
        });
        let (_, data) = sample.panel_lines();
        assert_eq!(data, "AB AB AB AB AB AB AB AB...");
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
    fn test_statistics_with_timestamp() {
        let stats = Statistics::new().with_timestamp();
        assert!(stats.timestamp.is_some());
    }

    /// A tcspecial configuration as it is really read, so these tests go
    /// through the same deserialization a file on disk does.
    fn payload(text: &str) -> PayloadConfig {
        crate::ConfigFormat::Yaml.parse(text).expect("parses")
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

    #[test]
    fn a_handler_takes_its_groups_attributes() {
        let config = payload(
            "
version: \"1.0\"
description: one group, two handlers in it
payload_groups:
  - name: udp_localhost
    type: network
    protocol: udp
payloads:
  - name: DH1
    group: udp_localhost
    payload_interface: localhost:5001
    packet_size: 11
  - name: DH3
    group: udp_localhost
    payload_interface: localhost:5003
    packet_size: 15
",
        );

        let handlers = config.to_dh_configs().unwrap();

        // Everything shared comes from the group; the port and packet size,
        // which are what tell the two apart, come from each handler.
        assert_eq!(
            handlers[0].endpoint,
            EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: "localhost".to_string(),
                port: 5001,
            })
        );
        assert_eq!(handlers[0].packet_size, 11);
        assert_eq!(
            handlers[1].endpoint,
            EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: "localhost".to_string(),
                port: 5003,
            })
        );
        assert_eq!(handlers[1].packet_size, 15);
    }

    #[test]
    fn a_handler_overrides_its_group() {
        let config = payload(
            "
version: \"1.0\"
description: a handler differing from the group it is in
payload_groups:
  - name: udp_localhost
    type: network
    protocol: udp
    packet_size: 11
payloads:
  - name: DH0
    group: udp_localhost
    protocol: tcp
    payload_interface: localhost:5000
",
        );

        let handlers = config.to_dh_configs().unwrap();

        assert_eq!(
            handlers[0].endpoint,
            EndpointConfig::Network(NetworkConfig {
                // The handler's own, not the group's udp.
                protocol: NetworkProtocol::Tcp,
                address: "localhost".to_string(),
                port: 5000,
            })
        );
        // Not overridden, so still the group's.
        assert_eq!(handlers[0].packet_size, 11);
    }

    #[test]
    fn a_handler_needs_no_group() {
        let config = payload(
            "
version: \"1.0\"
description: a handler stating everything for itself
payloads:
  - name: DH2
    type: device-file
    path: /dev/urandom
    packet_size: 1
",
        );

        let handlers = config.to_dh_configs().unwrap();
        assert_eq!(
            handlers[0].endpoint,
            EndpointConfig::Device(DeviceConfig {
                path: "/dev/urandom".to_string()
            })
        );
    }

    /// Where a payload exchanges data with the ground is counted from the
    /// section's base by the payload's own id.
    ///
    /// Not written on the payload, which is what it used to be: `oc_address`
    /// and `oc_port` on every entry were as many chances to repeat a port as
    /// there were payloads, and a repeated port is two payloads whose data
    /// arrives interleaved. One base and one arithmetic rule cannot be
    /// written inconsistently.
    #[test]
    fn a_payloads_ground_link_is_counted_from_the_base() {
        let config = payload(
            "
version: \"1.0\"
description: three payloads counted from one base
tcspecial:
  tcs_cmd:
    recv: 0.0.0.0:4000
    send: 127.0.0.1:4100
  payload_command:
    source_base: 127.0.0.1:4300
    dest_base: 0.0.0.0:4200
  payload_telemetry:
    source_base: 0.0.0.0:6000
    dest_base: 127.0.0.1:6100
  beacon:
    interval: 5 s
    send: 0.0.0.0:5550
    recv: 127.0.0.1:5650
payloads:
  - name: DH0
    type: device-file
    path: /dev/zero
    packet_size: 1
  - name: DH1
    type: device-file
    path: /dev/urandom
    packet_size: 1
  - name: DH2
    type: device-file
    path: /dev/null
    packet_size: 1
",
        );

        let handlers = config.to_dh_configs().expect("a sound set");
        for (id, handler) in handlers.iter().enumerate() {
            let oc = handler.oc.as_ref().expect("a ground link");
            assert_eq!(oc.protocol, NetworkProtocol::Udp);
            // tcspecial's own base: this is the socket tcspecial binds, and
            // the one the MOC says hello to.
            assert_eq!(oc.address, "0.0.0.0", "{}", handler.name.0);
            assert_eq!(oc.port, 6000 + id as u16, "{}", handler.name.0);
        }
    }

    /// A set with no tcspecial section has no ground link for its payloads.
    ///
    /// Which is what `oc: None` has always meant: a file describing payloads
    /// is complete without one, and only starting a handler needs it.
    #[test]
    fn a_set_with_no_section_has_no_ground_link() {
        let config = payload(
            "
version: \"1.0\"
description: no section at all
payloads:
  - name: DH2
    type: device-file
    path: /dev/urandom
    packet_size: 1
",
        );

        assert_eq!(config.to_dh_configs().unwrap()[0].oc, None);
    }

    /// A base too high for the payloads counted from it is refused.
    ///
    /// By name, and while somebody is reading the file: a sum past 65535 is a
    /// configuration describing something no socket can be, and the useful
    /// moment to say so is not when the socket is made.
    #[test]
    fn a_base_that_leaves_no_room_is_refused() {
        let config = payload(
            "
version: \"1.0\"
description: a base at the top of the range
tcspecial:
  tcs_cmd:
    recv: 0.0.0.0:4000
    send: 127.0.0.1:4100
  payload_command:
    source_base: 127.0.0.1:4300
    dest_base: 0.0.0.0:4200
  payload_telemetry:
    source_base: 0.0.0.0:65535
    dest_base: 127.0.0.1:6100
  beacon:
    interval: 5 s
    send: 0.0.0.0:5550
    recv: 127.0.0.1:5650
payloads:
  - name: DH0
    type: device-file
    path: /dev/zero
    packet_size: 1
  - name: DH1
    type: device-file
    path: /dev/urandom
    packet_size: 1
",
        );

        let message = config
            .to_dh_configs()
            .expect_err("the second payload has no port to be at");
        assert!(message.contains("65535") || message.contains("65536"), "{message}");
    }

    #[test]
    fn a_payload_file_describes_a_serial_payload() {
        let yaml = "version: \"1.0\"\ndescription: a line\n\
                    payloads:\n  - name: line\n    type: serial\n    \
                    path: /dev/ttyS0\n    datarate: 115200\n    \
                    stop_bits: 1\n    parity: none\n    byte_length: 8\n    \
                    packet_size: 12\n    stream:\n      max_length: 12\n      \
                    timeout: none\n";
        let config: PayloadConfig = ConfigFormat::Yaml.parse(yaml).expect("it parses");
        let handlers = config.to_dh_configs().expect("it resolves");

        match &handlers[0].endpoint {
            crate::EndpointConfig::Serial(line) => {
                assert_eq!(line.path, "/dev/ttyS0");
                assert_eq!(line.datarate, 115_200);
                assert_eq!(line.byte_length, 8);
                assert_eq!(line.stop_bits, StopBits::One);
                assert_eq!(line.parity, Parity::None);
            }
            other => panic!("a serial payload resolved to {other:?}"),
        }
    }

    /// And a payload on an I2C bus, and a SPI peripheral.
    #[test]
    fn a_payload_file_describes_a_bus_and_a_peripheral() {
        let i2c = "version: \"1.0\"\ndescription: a bus\n\
                   payloads:\n  - name: sensor\n    type: i2c\n    \
                   path: /dev/i2c-1\n    payload_address: 0x48\n    pec: true\n    \
                   packet_size: 8\n";
        let config: PayloadConfig = ConfigFormat::Yaml.parse(i2c).expect("it parses");
        match &config.to_dh_configs().expect("it resolves")[0].endpoint {
            crate::EndpointConfig::I2c(bus) => {
                assert_eq!(bus.bus, "/dev/i2c-1");
                assert_eq!(bus.address, 0x48);
                assert!(bus.pec);
            }
            other => panic!("an I2C payload resolved to {other:?}"),
        }

        let spi = "version: \"1.0\"\ndescription: a peripheral\n\
                   payloads:\n  - name: imu\n    type: spi\n    \
                   path: /dev/spidev0.0\n    max_speed: 1000000\n    spi_mode: 3\n    \
                   bits_per_word: 8\n    packet_size: 8\n";
        let config: PayloadConfig = ConfigFormat::Yaml.parse(spi).expect("it parses");
        match &config.to_dh_configs().expect("it resolves")[0].endpoint {
            crate::EndpointConfig::Spi(chip) => {
                assert_eq!(chip.path, "/dev/spidev0.0");
                assert_eq!(chip.max_speed, 1_000_000);
                assert_eq!(chip.bits_per_word, 8);
            }
            other => panic!("a SPI payload resolved to {other:?}"),
        }
    }

    /// The rules each kind had are the rules it still has.
    ///
    /// Three of them, one per kind, checked through the payload language: a
    /// start-stop line may not state a clocked line's clock, a SPI peripheral
    /// must say which clock mode it expects, and an I2C address the
    /// specification reserves is refused.
    #[test]
    fn a_links_own_rules_survived_the_move() {
        let serial = "version: \"1.0\"\ndescription: a line\n\
                      payloads:\n  - name: line\n    type: serial\n    \
                      path: /dev/ttyS0\n    datarate: 9600\n    \
                      stop_bits: 1\n    byte_length: 8\n    clock_type: txint\n    \
                      packet_size: 1\n    stream:\n      max_length: 1\n      \
                      timeout: none\n";
        let said = ConfigFormat::Yaml
            .parse::<PayloadConfig>(serial)
            .expect("it parses")
            .to_dh_configs()
            .expect_err("a start-stop line has no clock to state");
        assert!(said.contains("clock_type"), "{said}");

        let spi = "version: \"1.0\"\ndescription: a peripheral\n\
                   payloads:\n  - name: imu\n    type: spi\n    \
                   path: /dev/spidev0.0\n    max_speed: 1000000\n    \
                   bits_per_word: 8\n    packet_size: 8\n";
        let said = ConfigFormat::Yaml
            .parse::<PayloadConfig>(spi)
            .expect("it parses")
            .to_dh_configs()
            .expect_err("a peripheral states its mode");
        assert!(said.contains("mode"), "{said}");

        let reserved = "version: \"1.0\"\ndescription: a bus\n\
                        payloads:\n  - name: sensor\n    type: i2c\n    \
                        path: /dev/i2c-1\n    payload_address: 0x00\n    packet_size: 8\n";
        let said = ConfigFormat::Yaml
            .parse::<PayloadConfig>(reserved)
            .expect("it parses")
            .to_dh_configs()
            .expect_err("0x00 is reserved");
        assert!(said.contains("reserved"), "{said}");
    }

    /// Two payloads, written in each format.
    fn two_payloads() -> [(ConfigFormat, String); 2] {
        [
            (
                ConfigFormat::Yaml,
                "version: \"1.0\"\ndescription: two\npayloads:\n  \
                 - name: first\n    type: device-file\n    path: /dev/null\n    \
                 packet_size: 1\n  \
                 - name: second\n    type: device-file\n    path: /dev/zero\n    \
                 packet_size: 2\n"
                    .to_string(),
            ),
            (
                ConfigFormat::Xml,
                "<payload><version>1.0</version><description>two</description>\
                 <payloads><name>first</name><type>device-file</type>\
                 <path>/dev/null</path><packet_size>1</packet_size></payloads>\
                 <payloads><name>second</name><type>device-file</type>\
                 <path>/dev/zero</path><packet_size>2</packet_size></payloads></payload>"
                    .to_string(),
            ),
        ]
    }

    /// A payload knows where in the file it stood, in every format.
    ///
    /// The position is recorded as the list is read, so it is the same number
    /// in all three spellings of one set -- which a line number could not be,
    /// and which is what lets the three digest alike.
    /// A set states its version in parts a beacon can carry.
    ///
    /// The version goes out in every beacon as three bytes, so a file whose
    /// version is not one, two or three decimal parts is refused rather than
    /// announced as something it is not.
    /// A set states both command links, and they are two places.
    ///
    /// One socket cannot be two links: a file that put both at the same
    /// address and port would describe a split it has not made, and the half
    /// that bound second would fail at startup with the other half's address.
    /// The whole interface is compared and not only the port, each link saying
    /// where it is in full -- two links may share a port on two interfaces,
    /// which is a configuration somebody may mean.
    #[test]
    fn a_set_states_two_command_links() {
        let with = |links: &str| {
            format!(
                "\
version: \"1.0\"
description: a set
tcspecial:
{links}  beacon:
    interval: 5 s
    send: 0.0.0.0:5550
    recv: 127.0.0.1:5650
payloads:
  - name: p
    type: device-file
    path: /dev/null
    packet_size: 1
"
            )
        };

        // Both links, each where the file says it is.
        const BOTH: &str = "  tcs_cmd:
    recv: 0.0.0.0:4000
    send: 127.0.0.1:4100
  payload_command:
    source_base: 127.0.0.1:4300
    dest_base: 0.0.0.0:4200
  payload_telemetry:
    source_base: 0.0.0.0:6000
    dest_base: 127.0.0.1:6100
";

        payload(&with(BOTH))
            .to_dh_configs()
            .expect("two bases are two links");

        // The first payload's command socket is the base itself, so a base
        // equal to the spacecraft link is one socket asked to be two.
        let one_socket = BOTH.replace("dest_base: 0.0.0.0:4200", "dest_base: 0.0.0.0:4000");
        let said = payload(&with(&one_socket))
            .to_dh_configs()
            .expect_err("one socket cannot be two links");
        assert!(
            said.contains("payload_command.dest_base") && said.contains("4000"),
            "{said}"
        );

        // And a section that names only one of them has not been asked the
        // question: the parser refuses it, as it refuses a section with no
        // beacon.
        let only_one = BOTH.replace("    dest_base: 0.0.0.0:4200\n", "");
        assert_ne!(only_one, BOTH, "the test removed nothing");
        assert!(
            ConfigFormat::Yaml
                .parse::<PayloadConfig>(&with(&only_one))
                .is_err(),
            "a section with no payload_command.dest_base was accepted"
        );
    }

    /// No payload may bind a port the commanding is taken on.
    ///
    /// The one collision that does not stop a payload from working: it stops
    /// the commanding that would have started it, and which of the two fails
    /// depends on the order they were started in.
    #[test]
    fn no_payload_may_bind_a_command_port() {
        let with = |address: &str, port: u16| {
            format!(
                "\
version: \"1.0\"
description: a set
tcspecial:
  tcs_cmd:
    recv: {address}:4000
    send: 127.0.0.1:4100
  payload_command:
    source_base: 127.0.0.1:4300
    dest_base: 0.0.0.0:4200
  payload_telemetry:
    source_base: 0.0.0.0:6000
    dest_base: 127.0.0.1:6100
  beacon:
    interval: 5 s
    send: 0.0.0.0:5550
    recv: 127.0.0.1:5650
payloads:
  - name: p
    type: network
    protocol: udp
    payload_interface: localhost:{port}
    packet_size: 1
"
            )
        };

        // Either link -- the spacecraft's, or the one payload's own command
        // socket, which is its base counted on by its id of nought -- and a
        // wildcard link takes its port on every interface, so a payload on
        // localhost collides with one on 0.0.0.0 without either naming the
        // other's host.
        for port in [4000, 4200] {
            let said = payload(&with("0.0.0.0", port))
                .to_dh_configs()
                .unwrap_err();
            assert!(
                said.contains(&port.to_string()) && said.contains("command"),
                "port {port}: {said}"
            );
        }

        // A link bound to one interface takes the port on that one alone, so
        // a payload on another host may have it.
        payload(&with("192.0.2.1", 4000))
            .to_dh_configs()
            .expect("another interface's port 4000 is not this link's");

        // A port neither link wants is nobody's business. 4201 is the
        // command socket of payload 1, and this set has only payload 0.
        payload(&with("0.0.0.0", 4201))
            .to_dh_configs()
            .expect("4201 is no payload of this set's command port");
    }

    #[test]
    fn a_set_says_its_version_in_parts_a_beacon_can_carry() {
        let with = |version: &str| {
            format!(
                "version: \"{version}\"\ndescription: one payload\npayloads:\n  \
                 - name: p\n    type: device-file\n    path: /dev/null\n    \
                 packet_size: 1\n"
            )
        };

        for version in ["1.0", "2", "3.4.5"] {
            payload(&with(version))
                .to_dh_configs()
                .unwrap_or_else(|e| panic!("{version}: {e}"));
        }

        for version in ["one", "1.0-draft", "1.2.3.4", "256.0"] {
            let said = payload(&with(version))
                .to_dh_configs()
                .expect_err(version);
            assert!(
                said.contains("version") && said.contains(version),
                "{version}: {said}"
            );
        }
    }

    #[test]
    fn a_payload_knows_where_in_the_file_it_stood() {
        for (format, text) in two_payloads() {
            let config: PayloadConfig = format.parse(&text).expect("it parses");

            assert_eq!(config.payloads[0].name, "first", "{format:?}");
            assert_eq!(config.payloads[0].sequence, 0, "{format:?}");
            assert_eq!(config.payloads[1].name, "second", "{format:?}");
            assert_eq!(config.payloads[1].sequence, 1, "{format:?}");
        }
    }

    /// The file order survives the list being reordered.
    ///
    /// This is the whole point of recording it. The order used to live only in
    /// the order a `Vec` happened to keep, so anything that sorted the
    /// payloads -- for a lookup, for a display -- would have silently changed
    /// what the configuration said about which payload came first, and the
    /// only thing standing in the way was that nothing did it yet.
    #[test]
    fn the_file_order_survives_a_reordered_list() {
        let (format, text) = &two_payloads()[0];
        let mut config: PayloadConfig = format.parse(text).expect("it parses");

        config.payloads.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(config.payloads[0].name, "first", "the sort did nothing");

        config.payloads.reverse();
        assert_eq!(config.payloads[0].name, "second");
        // And it still knows it was second.
        assert_eq!(config.payloads[0].sequence, 1);
        assert_eq!(config.payloads[1].sequence, 0);
    }

    /// A file stating a position of its own is refused.
    ///
    /// The position is this program's record of what it read, not something a
    /// file gets to assert: one that could would be able to say its payloads
    /// came in an order they did not.
    #[test]
    fn a_file_may_not_state_a_position() {
        let text = "version: \"1.0\"\ndescription: one\npayloads:\n  \
                    - name: first\n    type: device-file\n    path: /dev/null\n    \
                    packet_size: 1\n    sequence: 7\n";

        let message = ConfigFormat::Yaml
            .parse::<PayloadConfig>(text)
            .expect_err("sequence is not an attribute")
            .to_string();
        assert!(
            message.contains("sequence"),
            "the refusal does not name it: {message}"
        );
    }

    #[test]
    fn a_group_no_handler_names_is_an_error() {
        // A group with no members has no effect on the configuration, so a
        // file carrying one is more likely wrong than deliberate.
        let config = payload(
            "
version: \"1.0\"
description: a group nothing is in
payload_groups:
  - name: udp_localhost
    type: network
    protocol: udp
payloads:
  - name: DH0
    type: network
    protocol: tcp
    payload_interface: localhost:5000
    packet_size: 12
",
        );

        let message = config.to_dh_configs().expect_err("must be rejected");
        assert!(
            message.contains("udp_localhost"),
            "the error should name the group nothing is in, but said: {message}"
        );
    }

    /// A name and an id each pick out one payload.
    ///
    /// Both repeat silently otherwise, and each loses a payload in its own
    /// way: tcspecial keeps its handlers by id, so a repeated id has one
    /// replace the other, and a simulator file is joined to this one by name,
    /// so a repeated name has one entry drive two payloads.
    #[test]
    fn a_payload_name_and_a_payload_id_are_each_defined_once() {
        let two = |second: &str| {
            format!(
                "
version: \"1.0\"
description: two payloads
payloads:
  - name: DH0
    type: network
    protocol: udp
    payload_interface: localhost:5000
    packet_size: 12
{second}
"
            )
        };

        let same_name = payload(&two(
            "  - name: DH0\n    type: network\n    protocol: udp\n    \
             payload_interface: localhost:5001\n    packet_size: 12",
        ));
        let said = same_name
            .to_dh_configs()
            .expect_err("two payloads, one name");
        assert!(
            said.contains("DH0") && said.contains("more than once"),
            "the error should name the payload: {said}"
        );

        // And two payloads that differ in both are fine, which is what the
        // checks must not get in the way of.
        let distinct = payload(&two(
            "  - name: DH1\n    type: network\n    protocol: udp\n    \
             payload_interface: localhost:5001\n    packet_size: 12",
        ));
        assert_eq!(distinct.to_dh_configs().expect("both convert").len(), 2);
    }

    /// No two handlers want the same thing.
    ///
    /// Two at one port is a start that fails at the second; two on one device
    /// file is worse, each taking part of the one stream and reporting it as
    /// though it were the whole. Checked for both at once, and for a
    /// handler's OC address beside its payload address, since a port is a
    /// port whatever claims it.
    #[test]
    fn no_two_handlers_want_one_address_or_one_device() {
        let pair = |first: &str, second: &str| {
            format!(
                "
version: \"1.0\"
description: two payloads
payloads:
  - name: DH0
    packet_size: 12
{first}
  - name: DH1
    packet_size: 12
{second}
"
            )
        };

        let udp = |port: u16| {
            format!(
                "    type: network\n    protocol: udp\n    payload_interface: localhost:{port}"
            )
        };
        let device = |path: &str| format!("    type: device-file\n    path: {path}");

        // The same host and port, written two ways: localhost and 127.0.0.1
        // are one host, so naming it differently does not make it a different
        // port.
        let said = payload(&pair(
            &udp(5000),
            "    type: network\n    protocol: udp\n    payload_interface: 127.0.0.1:5000",
        ))
        .to_dh_configs()
        .expect_err("two payloads at one port");
        assert!(
            said.contains("DH0") && said.contains("DH1") && said.contains("5000"),
            "the error should name both handlers and what they want: {said}"
        );

        // The same device file.
        let said = payload(&pair(&device("/dev/ttyS0"), &device("/dev/ttyS0")))
            .to_dh_configs()
            .expect_err("two payloads on one device file");
        assert!(said.contains("/dev/ttyS0"), "{said}");

        // A payload's ground link is as exclusive as its payload address, and
        // is compared against it: this one would have to bind the port twice.
        // The ground link is counted from the section's base now, so the
        // collision is written there rather than on the payload.
        let clash = format!(
            "
version: \"1.0\"
description: a ground link on the payload's own port
tcspecial:
  tcs_cmd:
    recv: 0.0.0.0:4000
    send: 127.0.0.1:4100
  payload_command:
    source_base: 127.0.0.1:4300
    dest_base: 0.0.0.0:4200
  payload_telemetry:
    source_base: localhost:5000
    dest_base: 127.0.0.1:6100
  beacon:
    interval: 5 s
    send: 0.0.0.0:5550
    recv: 127.0.0.1:5650
payloads:
  - name: DH0
    packet_size: 12
{}
",
            udp(5000)
        );
        let said = payload(&clash)
            .to_dh_configs()
            .expect_err("one port for the payload and the ground link");
        assert!(
            said.contains("payload") && said.contains("OC"),
            "the error should say which of the two is which: {said}"
        );

        // And two handlers that want different things convert, which is what
        // the rule must not get in the way of.
        let fine = payload(&pair(&udp(5000), &udp(5001)));
        assert_eq!(fine.to_dh_configs().expect("both convert").len(), 2);
    }

    /// A bus is shared; a place on it is not.
    ///
    /// The one exception the rule has to make, and the reason a claim is on the
    /// address rather than on the bus device: several devices on one I2C bus is
    /// what a bus is for, and refusing that would refuse the normal case. These
    /// handlers cannot be written in a tcspecial configuration, so they are
    /// built directly.
    #[test]
    fn a_bus_is_shared_and_a_place_on_it_is_not() {
        let on_bus = |name: &str, address: u16| DHConfig {
            name: DHName::new(name),
            endpoint: EndpointConfig::I2c(I2cConfig {
                bus: "/dev/i2c-1".to_string(),
                address,
                ten_bit: false,
                pec: false,
            }),
            packet_size: 4,
            oc: None,
            ground: None,
            mode: DHMode::Periodic,
            stale_interval: None,
        };

        no_two_handlers_claim_one_thing(&[on_bus("DH0", 0x48), on_bus("DH1", 0x49)])
            .expect("two devices on one bus is what a bus is for");

        let said = no_two_handlers_claim_one_thing(&[on_bus("DH0", 0x48), on_bus("DH1", 0x48)])
            .expect_err("two devices at one address on one bus");
        assert!(
            said.contains("0x48") && said.contains("/dev/i2c-1"),
            "the error should name the address and the bus: {said}"
        );
    }

    /// A Unix socket is named by a file, so it collides like a file.
    ///
    /// Its port is meaningless -- two Unix payloads with different ports and
    /// one path are one socket -- and a device handler opening that same path
    /// is the same collision from the other side, which is why every path is
    /// one kind of claim.
    #[test]
    fn a_unix_socket_collides_by_its_path() {
        let socket = |name: &str, port: u16| DHConfig {
            name: DHName::new(name),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::UnixStream,
                address: "/tmp/dh.sock".to_string(),
                port,
            }),
            packet_size: 4,
            oc: None,
            ground: None,
            mode: DHMode::Periodic,
            stale_interval: None,
        };

        let said = no_two_handlers_claim_one_thing(&[socket("DH0", 0), socket("DH1", 7)])
            .expect_err("one socket path, whatever the ports say");
        assert!(said.contains("/tmp/dh.sock"), "{said}");

        let mut device = socket("DH1", 0);
        device.endpoint = EndpointConfig::Device(DeviceConfig {
            path: "/tmp/dh.sock".to_string(),
        });
        assert!(
            no_two_handlers_claim_one_thing(&[socket("DH0", 0), device]).is_err(),
            "a path is a path, whichever kind of handler opens it"
        );
    }

    /// A name is unique within its file and nowhere wider.
    ///
    /// Two payload sets are two descriptions of what a mission flies, and a
    /// set that had to rename its payloads because another set in the same
    /// directory had used the names would be renaming them for no reason. A
    /// set is read on its own; nothing compares the names in one file with
    /// the names in another, and the shipped sets rely on it -- more than one
    /// of them names a payload DH0.
    #[test]
    fn two_files_may_name_their_payloads_alike() {
        let one_set = |port: u16, packet_size: usize| {
            format!(
                "
version: \"1.0\"
description: a set of its own
payload_groups:
  - name: localhost_udp
    type: network
    protocol: udp
payloads:
  - name: DH0
    group: localhost_udp
    payload_interface: localhost:{port}
    packet_size: {packet_size}
"
            )
        };

        // The same payload name, the same group name, and the same id, in two
        // files that know nothing of each other.
        let first = payload(&one_set(5000, 12))
            .to_dh_configs()
            .expect("the first set");
        let second = payload(&one_set(6000, 8))
            .to_dh_configs()
            .expect("the second set");

        assert_eq!(first[0].name.0, second[0].name.0);
        assert_ne!(
            first[0].packet_size, second[0].packet_size,
            "the two sets describe different payloads under the one name"
        );
    }

    /// An attribute of another kind of payload is refused.
    ///
    /// Ignoring one is how a device payload comes to carry a port nothing
    /// reads, and how a network payload carrying a path looks configured and
    /// is not.
    #[test]
    fn an_attribute_of_another_kind_does_not_apply() {
        let device = payload(
            "
version: \"1.0\"
description: a device stating network attributes
payloads:
  - name: DH0
    type: device-file
    path: /dev/urandom
    protocol: udp
    payload_interface: localhost:5000
    packet_size: 1
",
        );
        let said = device
            .to_dh_configs()
            .expect_err("a device has no protocol");
        assert!(
            said.contains("device") && said.contains("protocol") && !said.contains("group"),
            "an attribute the payload states itself is not blamed on a group: {said}"
        );

        let network = payload(
            "
version: \"1.0\"
description: a network payload stating a path
payloads:
  - name: DH0
    type: network
    protocol: udp
    payload_interface: localhost:5000
    path: /dev/urandom
    packet_size: 12
",
        );
        let said = network
            .to_dh_configs()
            .expect_err("a network payload has no path");
        assert!(said.contains("network") && said.contains("path"), "{said}");

        // An attribute inherited from a group applies to a payload exactly as
        // one it states itself, so it is refused exactly as one would be.
        // This is the case worth catching: the group is written for the
        // payloads that are reached that way, and a payload of another kind
        // joining it takes attributes nobody wrote for it.
        let inherited = payload(
            "
version: \"1.0\"
description: a device in a group of network payloads
payload_groups:
  - name: udp_localhost
    type: network
    protocol: udp
payloads:
  - name: DH0
    group: udp_localhost
    type: device-file
    path: /dev/urandom
    packet_size: 1
",
        );
        let said = inherited
            .to_dh_configs()
            .expect_err("a device takes no address from a group");
        assert!(said.contains("device") && said.contains("group"), "{said}");
    }

    /// An attribute this language does not know is refused.
    ///
    /// A misspelled attribute is otherwise ignored, which is silence about
    /// something the file plainly meant: an `oc_prot` is a payload with
    /// nowhere to send its data, and nothing says so.
    #[test]
    fn a_word_that_is_not_an_attribute_is_refused() {
        for text in [
            // On a payload.
            "
version: \"1.0\"
description: a misspelled attribute
payloads:
  - name: DH0
    type: network
    protocol: udp
    payload_interface: localhost:5000
    packet_size: 12
    oc_adress: 127.0.0.1
",
            // On a group.
            "
version: \"1.0\"
description: a misspelled group attribute
payload_groups:
  - name: g
    type: network
    protocl: udp
payloads:
  - name: DH0
    group: g
    protocol: udp
    payload_interface: localhost:5000
    packet_size: 12
",
            // And on the file.
            "
version: \"1.0\"
description: a misspelled section
payload:
  - name: DH0
",
        ] {
            let e = crate::ConfigFormat::Yaml
                .parse::<PayloadConfig>(text)
                .expect_err("a word this language does not know is refused");
            let said = format!("{e}");
            assert!(
                said.contains("unknown field"),
                "the error should say the word is unknown: {said}"
            );
        }
    }

    /// A trigger of bytes is written without quotes, and a trigger of text
    /// with them.
    ///
    /// Which is a convention for writing the file rather than something the
    /// parser can enforce: every format hands over the same string whichever
    /// way the value was written, so the quotes are invisible here. What
    /// matters is that the unquoted form works -- YAML's core schema does not
    /// read `0x55AA` as a number, so the digits arrive intact, leading zeros
    /// and all, which is what makes the quote-free spelling safe to
    /// recommend.
    #[test]
    fn a_trigger_is_a_list_of_values_or_a_quoted_string() {
        let with = |trigger: &str| {
            payload(&format!(
                "
version: \"1.0\"
description: a trigger, written one way
payloads:
  - name: DH0
    type: network
    protocol: tcp
    payload_interface: localhost:5000
    packet_size: 12
    mode: triggered
    packet_interval: 500 ms
    trigger: {trigger}
"
            ))
            .to_dh_configs()
            .map(|c| match &c[0].mode {
                DHMode::Triggered { trigger, .. } => trigger.clone(),
                DHMode::Periodic => panic!("this payload is triggered"),
            })
        };

        // Several bytes, which is a C string: single-quoted in the YAML so
        // that its own quotes and escapes reach the trigger reader as
        // written.
        assert_eq!(with("'\"\\x55\\xAA\"'").unwrap(), vec![0x55, 0xAA]);
        // One byte, however it was padded: a leading zero is a zero and not a
        // notation.
        assert_eq!(with("0D").unwrap(), vec![0x0D]);
        assert_eq!(with("d").unwrap(), vec![0x0D]);

        // Several bytes are a C string and nothing else: the list of values
        // this took is gone, a value's length no longer deciding how many
        // bytes it meant.
        for several in ["0D 0A", "55 AA 0F", "55AA"] {
            assert!(with(several).is_err(), "{several} is not one byte");
        }

        // And a trigger of text, which is a C string: the quotes are part of
        // the value, so a YAML file writes them inside its own.
        assert_eq!(with("'\"READ\\r\"'").unwrap(), b"READ\r".to_vec());
        assert_eq!(
            with("'\"\\x52\\x45\\x41\\x44\\r\"'").unwrap(),
            with("'\"READ\\r\"'").unwrap()
        );

        // The spelling this replaced, caught by name rather than puzzled
        // over: a file written for the older rule says 0x.
        let said = with("0x55AA").expect_err("the older spelling");
        assert!(said.contains("no longer"), "{said}");
    }

    /// Which file an interval belongs in follows from the kind of payload.
    ///
    /// A periodic payload sends on its own, so how fast a simulated one sends
    /// is the simulator's business and an interval here is refused. A
    /// triggered payload sends nothing until it is asked, and the asking is
    /// tcspecial's: the rate the trigger goes out at is flight behaviour, so
    /// it is stated here and nowhere else. One name, `packet_interval_ms`, in
    /// whichever file the payload's kind puts it.
    #[test]
    fn which_file_states_an_interval_follows_from_the_kind() {
        let payload_file = |extra: &str| {
            format!(
                "
version: \"1.0\"
description: a payload and an interval
payloads:
  - name: DH0
    type: network
    protocol: tcp
    payload_interface: localhost:5000
    packet_size: 12
{extra}
"
            )
        };

        // Periodic, which is what a file states by saying nothing: the
        // interval is refused and told which file it belongs in.
        for stated in [
            "    packet_interval: 250 ms",
            "    mode: periodic\n    packet_interval: 250 ms",
        ] {
            let said = payload(&payload_file(stated))
                .to_dh_configs()
                .expect_err("a periodic payload's rate is the simulator's");
            assert!(
                said.contains("simulator configuration") && said.contains("no packet interval"),
                "{said}"
            );
        }

        // Triggered: the interval is this file's, and required.
        let handlers = payload(&payload_file(
            "    mode: triggered\n    trigger: '\"READ\\r\"'\n    packet_interval: 500 ms",
        ))
        .to_dh_configs()
        .expect("a triggered payload states its trigger's rate here");
        assert_eq!(
            handlers[0].mode,
            DHMode::Triggered {
                trigger: b"READ\r".to_vec(),
                interval: Duration::from_millis(500),
            }
        );

        let said = payload(&payload_file(
            "    mode: triggered\n    trigger: \'\"READ\\r\"\'",
        ))
        .to_dh_configs()
        .expect_err("a triggered payload with no rate for its trigger");
        assert!(said.contains("packet_interval"), "{said}");

        // And the name that rate used to have is told the name it has now.
        let said = payload(&payload_file(
            "    mode: triggered\n    trigger: \'\"READ\\r\"\'\n    trigger_interval_ms: 500",
        ))
        .to_dh_configs()
        .expect_err("trigger_interval_ms is not a word this file knows");
        assert!(
            said.contains("trigger_interval_ms") && said.contains("packet_interval now"),
            "{said}"
        );

        // A group's interval reaches the payload exactly as its own would.
        let grouped = "
version: \"1.0\"
description: a group with the rate in it
payload_groups:
  - name: asked
    type: network
    protocol: tcp
    mode: triggered
    trigger: \'\"READ\\r\"\'
    packet_interval: 500 ms
payloads:
  - name: DH0
    group: asked
    payload_interface: localhost:5000
    packet_size: 12
";
        let handlers = payload(grouped)
            .to_dh_configs()
            .expect("the group states it");
        assert!(matches!(
            &handlers[0].mode,
            DHMode::Triggered { interval, .. } if *interval == Duration::from_millis(500)
        ));
    }

    #[test]
    fn a_misspelled_group_is_reported_from_the_handler_not_the_group() {
        // A typo leaves the group unnamed and the name undefined at once. The
        // handler's end is where the misspelling actually is, so that is the
        // error worth giving.
        let config = payload(
            "
version: \"1.0\"
description: a handler misspelling the one group
payload_groups:
  - name: udp_localhost
    type: network
    protocol: udp
payloads:
  - name: DH0
    group: udp_localhst
    payload_interface: localhost:5000
    packet_size: 12
",
        );

        let message = config.to_dh_configs().expect_err("must be rejected");
        assert!(
            message.contains("udp_localhst") && message.contains("DH0"),
            "the error should name the handler and its misspelling, but said: {message}"
        );
    }

    #[test]
    fn a_handler_with_no_packet_size_anywhere_is_an_error() {
        // A missing packet size would otherwise resolve to zero, and a handler
        // whose packets are zero bytes long reads nothing forever.
        let config = payload(
            "
version: \"1.0\"
description: a handler nothing gives a packet size
payload_groups:
  - name: udp_localhost
    type: network
    protocol: udp
payloads:
  - name: DH0
    group: udp_localhost
    payload_interface: localhost:5000
",
        );

        assert!(config.to_dh_configs().is_err());
    }
    // -- the rules of a link ------------------------------------------------
    //
    // A line, a bus and a peripheral each have rules of their own: which terms
    // the kind takes, which it must be told, and which belong to another kind.
    // The rules are stated once, in `endpoint_config` and its per-kind modules,
    // and these are the tests of them. They used to read an endpoint
    // configuration file, which was the only way to reach them; they read a
    // tcspecial configuration now, which is the only way to reach them.

    /// One payload of the given kind, with `attributes` folded into it.
    fn link(kind: &str, attributes: &str) -> Result<PayloadConfig, String> {
        let text = format!(
            "version: \"1.0\"\ndescription: one payload\npayloads:\n  \
             - name: p\n    type: {kind}\n{attributes}"
        );
        ConfigFormat::Yaml
            .parse::<PayloadConfig>(&text)
            .map_err(|e| format!("{e}"))
    }

    /// What the rules settle for the one payload of such a file.
    ///
    /// A handler carries less than the rules check -- the stream rule says
    /// where one read ends and reaches no handler, as it reached none when an
    /// endpoint configuration was what stated it -- so a test of a rule asks
    /// for the terms the rules settled rather than for the handler.
    fn link_terms(
        kind: &str,
        attributes: &str,
    ) -> Result<crate::endpoint_config::GroupKind, String> {
        let config = link(kind, attributes)?;
        let payload = &config.payloads[0];
        let group = payload.group.as_deref().and_then(|g| config.group(g));
        link_params_of(
            &payload.name,
            DHType::from_spelling(kind).expect("a kind this knows"),
            payload,
            group,
        )
    }

    /// The terms of a line, which is what the stream rule belongs to.
    const A_LINE: &str = "    path: /dev/ttyS0\n    \
                          datarate: 9600\n    stop_bits: 1\n    byte_length: 8\n";

    /// An RS-485 bus reaches a handler with its turnaround on it.
    ///
    /// The rule itself is tested where it lives; this is the other half --
    /// that a file stating it produces a handler carrying it, through the
    /// terms, the group rules and the endpoint. A term that is read and
    /// dropped on the way is a line that is set to something nobody asked
    /// for, and nothing in the rule's own tests would notice.
    #[test]
    fn an_rs485_line_carries_its_turnaround_to_the_handler() {
        let bus = format!(
            "{A_LINE}    rs485: true\n    rs485_rts_on_send: false\n    \
             rs485_delay_after_send: 2 ms\n    packet_size: 8\n    stream:\n      \
             timeout: none\n      max_length: 64\n"
        );
        let handlers = link("serial", &bus)
            .expect("a line that is a two-wire bus")
            .to_dh_configs()
            .expect("it becomes a handler");

        match &handlers[0].endpoint {
            EndpointConfig::Serial(serial) => {
                let rs485 = serial.rs485.expect("the bus reached the handler");
                assert!(!rs485.rts_on_send, "RTS is low while sending on this board");
                assert_eq!(rs485.delay_before_send, Duration::ZERO, "nothing was asked for");
                assert_eq!(rs485.delay_after_send, Duration::from_millis(2));
            }
            other => panic!("expected a serial endpoint, got {other:?}"),
        }

        // And a line that says nothing is not a bus, which is what every set
        // written before this says.
        let plain = format!(
            "{A_LINE}    packet_size: 8\n    stream:\n      timeout: none\n      \
             max_length: 64\n"
        );
        let plain = link("serial", &plain)
            .expect("a plain line")
            .to_dh_configs()
            .expect("it becomes a handler");
        match &plain[0].endpoint {
            EndpointConfig::Serial(serial) => assert_eq!(serial.rs485, None),
            other => panic!("expected a serial endpoint, got {other:?}"),
        }
    }

    /// A term of a link is refused on the two kinds that are not on one.
    ///
    /// Every other kind refuses what belongs to another kind as its own rules
    /// are asked, and these two have no rules to ask -- so a term written on
    /// one of them was read and dropped. A device payload could state a data
    /// rate, or a sensor's channels, and be read as a device payload that had
    /// said nothing of the sort: a misspelling that configured nothing and
    /// reported nothing.
    #[test]
    fn a_link_term_is_refused_on_a_kind_that_is_not_on_a_link() {
        // One term from each kind of link, on each kind that is not one.
        for term in [
            "datarate: 9600",
            "stop_bits: 1",
            "pec: true",
            "max_speed: 1000000",
            "can_fd: true",
            "service: payload",
            "lines: 0",
            "rs485: true",
        ] {
            let said = link(
                "device-file",
                &format!("    path: /dev/urandom\n    packet_size: 1\n    {term}\n"),
            )
            .expect("it parses")
            .to_dh_configs()
            .expect_err("a device file is not a link");
            let field = term.split(':').next().expect("a term");
            assert!(
                said.contains(field) && said.contains("device-file payload"),
                "the error should name the term and the kind: {said}"
            );

            let said = link(
                "network",
                &format!(
                    "    protocol: udp\n    payload_interface: localhost:5000\n    \
                     packet_size: 8\n    {term}\n"
                ),
            )
            .expect("it parses")
            .to_dh_configs()
            .expect_err("a network address is not a link");
            assert!(
                said.contains(field) && said.contains("network payload"),
                "the error should name the term and the kind: {said}"
            );
        }
    }

    /// A network payload's own transport is not a term of somebody else's
    /// link.
    ///
    /// The one term in that table that belongs to the network kind, so the
    /// rule above has to let it through: refusing it would refuse every
    /// network payload there is.
    #[test]
    fn a_network_payloads_transport_is_its_own() {
        let fine = link(
            "network",
            "    protocol: udp\n    payload_interface: localhost:5000\n    \
             packet_size: 8\n",
        )
        .expect("it parses")
        .to_dh_configs()
        .expect("a network payload states its transport");
        assert_eq!(fine[0].endpoint.kind(), DHType::Network);
    }

    /// A term a payload takes from its group is refused as one it states, and
    /// the error says which.
    ///
    /// The two are fixed differently: a line is deleted from the payload, or
    /// a payload is in a group that was not written for it.
    #[test]
    fn a_link_term_from_a_group_names_the_group() {
        let text = "version: \"1.0\"\ndescription: a device in a serial group\n\
                    payload_groups:\n  - name: lines\n    datarate: 9600\n\
                    payloads:\n  - name: p\n    type: device-file\n    group: lines\n    \
                    path: /dev/urandom\n    packet_size: 1\n";
        let said = ConfigFormat::Yaml
            .parse::<PayloadConfig>(text)
            .expect("it parses")
            .to_dh_configs()
            .expect_err("a device payload in a group written for a line");
        assert!(
            said.contains("datarate") && said.contains("group \"lines\""),
            "the error should name the term and where it came from: {said}"
        );
    }

    /// A GPIO payload reaches a handler with the lines it watches and how.
    #[test]
    fn a_gpio_payload_is_a_chip_and_the_lines_it_watches() {
        let discretes = "    path: /dev/gpiochip0\n    lines: 3, 7\n    edge: rising\n    \
                         bias: pull_up\n    debounce: 5000 us\n    packet_size: 32\n";
        let handlers = link("gpio", discretes)
            .expect("a set of discretes")
            .to_dh_configs()
            .expect("it becomes a handler");

        match &handlers[0].endpoint {
            EndpointConfig::Gpio(gpio) => {
                assert_eq!(gpio.path, "/dev/gpiochip0");
                assert_eq!(gpio.lines, vec![3, 7]);
                assert_eq!(gpio.edge, crate::LineEdge::Rising);
                assert_eq!(gpio.bias, crate::Bias::PullUp);
                assert_eq!(gpio.debounce, Some(Duration::from_micros(5_000)));
            }
            other => panic!("expected a GPIO endpoint, got {other:?}"),
        }
        assert_eq!(handlers[0].endpoint.kind(), DHType::Gpio);
    }

    /// A packet that is not a whole number of events is refused.
    ///
    /// What a GPIO payload produces is records this handler makes, each of a
    /// fixed size, so a packet that was part of one would be a reading nobody
    /// could use.
    #[test]
    fn a_gpio_packet_is_whole_events() {
        let said = link(
            "gpio",
            "    path: /dev/gpiochip0\n    lines: 0\n    packet_size: 12\n",
        )
        .expect("it parses")
        .to_dh_configs()
        .expect_err("three quarters of an event");
        assert!(
            said.contains("16-byte line events"),
            "the error should say what a packet is made of: {said}"
        );

        // One event, and several, are both whole numbers of them.
        for size in [16usize, 160] {
            let good = link(
                "gpio",
                &format!("    path: /dev/gpiochip0\n    lines: 0\n    packet_size: {size}\n"),
            )
            .expect("it parses")
            .to_dh_configs()
            .expect("a whole number of events");
            assert_eq!(good[0].packet_size, size);
        }
    }

    /// Two payloads may watch one chip and may not watch one line.
    #[test]
    fn a_chip_is_shared_and_a_line_is_not() {
        let two = |first: &str, second: &str| {
            format!(
                "version: \"1.0\"\ndescription: two sets of discretes\npayloads:\n  \
                 - name: a\n    type: gpio\n    path: /dev/gpiochip0\n    lines: {first}\n    \
                 packet_size: 16\n  - name: b\n    type: gpio\n    path: /dev/gpiochip0\n    \
                 lines: {second}\n    packet_size: 16\n"
            )
        };

        let apart = ConfigFormat::Yaml
            .parse::<PayloadConfig>(&two("0, 1", "2, 3"))
            .expect("it parses")
            .to_dh_configs()
            .expect("different lines of one chip");
        no_two_handlers_claim_one_thing(&apart).expect("a chip is shared by design");

        let said = ConfigFormat::Yaml
            .parse::<PayloadConfig>(&two("0, 1", "1, 2"))
            .expect("it parses")
            .to_dh_configs()
            .expect_err("two payloads watching line 1");
        assert!(
            said.contains("line 1") && said.contains("/dev/gpiochip0"),
            "the error should name the line and the chip: {said}"
        );
    }

    /// A GPIO payload cannot be triggered: its lines are inputs.
    #[test]
    fn a_gpio_payload_cannot_be_triggered() {
        let said = link(
            "gpio",
            "    path: /dev/gpiochip0\n    lines: 0\n    packet_size: 16\n    \
             mode: triggered\n    trigger: '\"READ\"'\n    packet_interval: 100 ms\n",
        )
        .expect("it parses")
        .to_dh_configs()
        .expect_err("a discrete cannot be asked");
        assert!(
            said.contains("inputs") && said.contains("commanding"),
            "the error should say why it cannot be asked: {said}"
        );
    }

    /// An rpmsg payload reaches a handler with its channel and its service.
    #[test]
    fn an_rpmsg_payload_is_a_node_and_the_service_it_should_be() {
        let channel = "    path: /dev/rpmsg0\n    service: payload-telemetry\n    \
                       packet_size: 64\n";
        let handlers = link("rpmsg", channel)
            .expect("a channel")
            .to_dh_configs()
            .expect("it becomes a handler");

        match &handlers[0].endpoint {
            EndpointConfig::Rpmsg(rpmsg) => {
                assert_eq!(rpmsg.path, "/dev/rpmsg0");
                assert_eq!(rpmsg.service, "payload-telemetry");
                assert_eq!(rpmsg.max_message_bytes, None, "the usual transport");
            }
            other => panic!("expected an rpmsg endpoint, got {other:?}"),
        }
        assert_eq!(handlers[0].endpoint.kind(), DHType::Rpmsg);

        // The service is what identifies a channel, so a payload without one
        // is refused: a node's number is assigned as channels appear.
        let said = link("rpmsg", "    path: /dev/rpmsg0\n    packet_size: 8\n")
            .expect("it parses")
            .to_dh_configs()
            .expect_err("a channel with no service");
        assert!(
            said.contains("service"),
            "the error should name what is missing: {said}"
        );
    }

    /// A packet no message can carry is refused where the file is read.
    #[test]
    fn a_packet_larger_than_an_rpmsg_message_is_refused() {
        let said = link(
            "rpmsg",
            "    path: /dev/rpmsg0\n    service: payload\n    packet_size: 600\n",
        )
        .expect("it parses")
        .to_dh_configs()
        .expect_err("six hundred bytes in a 496-byte message");
        assert!(
            said.contains("600") && said.contains("496"),
            "the error should say the packet and the message: {said}"
        );

        // And a platform with larger buffers says so, which makes it fit.
        let larger = link(
            "rpmsg",
            "    path: /dev/rpmsg0\n    service: payload\n    \
             max_message_bytes: 2048\n    packet_size: 600\n",
        )
        .expect("it parses")
        .to_dh_configs()
        .expect("room for it");
        assert_eq!(larger[0].packet_size, 600);
    }

    /// An rpmsg payload may be triggered: the channel carries both
    /// directions.
    #[test]
    fn an_rpmsg_payload_may_be_triggered() {
        let asked = link(
            "rpmsg",
            "    path: /dev/rpmsg0\n    service: payload\n    packet_size: 8\n    \
             mode: triggered\n    trigger: '\"READ\"'\n    packet_interval: 100 ms\n",
        )
        .expect("it parses")
        .to_dh_configs()
        .expect("a channel may be asked");
        assert!(
            asked[0].mode.polling().is_some(),
            "an rpmsg payload that answers requests"
        );
    }

    /// An IIO payload reaches a handler as the buffer it is, and no more.
    ///
    /// A path and nothing else. It took two terms once -- how many channels
    /// were enabled and how fast the device samples -- and neither could be
    /// applied by a program that opens the character device, so a payload
    /// states neither and a file that states one is refused.
    #[test]
    fn an_iio_payload_is_a_buffer_and_nothing_else() {
        let handlers = link("iio", "    path: /dev/iio:device0\n    packet_size: 12\n")
            .expect("a sensor")
            .to_dh_configs()
            .expect("it becomes a handler");

        match &handlers[0].endpoint {
            EndpointConfig::Iio(iio) => assert_eq!(iio.path, "/dev/iio:device0"),
            other => panic!("expected an IIO endpoint, got {other:?}"),
        }
        assert_eq!(handlers[0].endpoint.kind(), DHType::Iio);

        for term in ["channels: 3", "sample_rate: 100"] {
            let said = link(
                "iio",
                &format!("    path: /dev/iio:device0\n    packet_size: 12\n    {term}\n"),
            )
            .expect("it parses")
            .to_dh_configs()
            .expect_err("a term nothing can apply");
            let field = term.split(':').next().expect("a term");
            assert!(
                said.contains(field) && said.contains("sysfs"),
                "the complaint should name the term and say why: {said}"
            );
        }
    }

    /// An IIO payload cannot be triggered: there is nowhere to send a
    /// request.
    ///
    /// The refusal belongs here rather than in the simulator, as a datagram
    /// payload's does: it is not that the stand-in cannot hear a trigger, it
    /// is that a sample buffer has nothing to write to.
    #[test]
    fn an_iio_payload_cannot_be_triggered() {
        let said = link(
            "iio",
            "    path: /dev/iio:device0\n    packet_size: 4\n    mode: triggered\n    \
             trigger: '\"READ\"'\n    packet_interval: 100 ms\n",
        )
        .expect("it parses")
        .to_dh_configs()
        .expect_err("a sensor cannot be asked");
        assert!(
            said.contains("read and not written") && said.contains("sysfs"),
            "the error should say why it cannot be asked: {said}"
        );
    }

    /// What a sensor says belongs to a sensor and to no other kind of link.
    #[test]
    fn iio_terms_are_refused_on_a_kind_that_is_not_a_sensor() {
        let said = link(
            "spi",
            "    path: /dev/spidev0.0\n    max_speed: 1000000\n    spi_mode: 0\n    \
             packet_size: 4\n    channels: 2\n",
        )
        .expect("it parses")
        .to_dh_configs()
        .expect_err("a peripheral has no channels");
        assert!(
            said.contains("channels"),
            "the error should name the term: {said}"
        );
    }

    /// A CAN payload reaches a handler with its interface and identifier.
    ///
    /// Which is the whole of where it is: a CAN controller is a network
    /// interface rather than a device file, and the identifier its frames
    /// carry is what tells one payload on a shared bus from another.
    #[test]
    fn a_can_payload_is_an_interface_and_an_identifier() {
        let bus = "    interface: vcan0\n    can_id: 0x123\n    packet_size: 8\n";
        let handlers = link("can", bus)
            .expect("a payload on a CAN bus")
            .to_dh_configs()
            .expect("it becomes a handler");

        match &handlers[0].endpoint {
            EndpointConfig::Can(can) => {
                assert_eq!(can.interface, "vcan0");
                assert_eq!(can.can_id, 0x123);
                assert!(!can.fd, "classic CAN unless the file says otherwise");
                assert!(can.loopback, "and looped back, which is what the kernel does");
            }
            other => panic!("expected a CAN endpoint, got {other:?}"),
        }
        assert_eq!(handlers[0].endpoint.kind(), DHType::Can);
    }

    /// A CAN payload needs both halves of where it is.
    #[test]
    fn a_can_payload_without_an_interface_or_an_id_is_refused() {
        for (missing, stated) in [
            ("interface", "    can_id: 1\n    packet_size: 8\n"),
            ("can_id", "    interface: vcan0\n    packet_size: 8\n"),
        ] {
            let said = link("can", stated)
                .expect("it parses")
                .to_dh_configs()
                .expect_err("half of where it is");
            assert!(
                said.contains(missing),
                "the error should name what is missing: {said}"
            );
        }
    }

    /// A packet no frame can carry is refused where the file is read.
    ///
    /// There is no fragmentation in CAN: a classic frame carries eight bytes
    /// and an FD frame sixty-four, so a payload whose packets are larger
    /// would be a handler that could never send one.
    #[test]
    fn a_packet_larger_than_a_frame_is_refused() {
        let said = link(
            "can",
            "    interface: vcan0\n    can_id: 1\n    packet_size: 12\n",
        )
        .expect("it parses")
        .to_dh_configs()
        .expect_err("twelve bytes in an eight-byte frame");
        assert!(
            said.contains("12") && said.contains("8"),
            "the error should say the packet and the frame: {said}"
        );

        // And the same payload in FD frames is fine, which is what the flag
        // is for.
        let fd = link(
            "can",
            "    interface: vcan0\n    can_id: 1\n    can_fd: true\n    packet_size: 12\n",
        )
        .expect("it parses")
        .to_dh_configs()
        .expect("sixty-four bytes of room");
        assert_eq!(fd[0].packet_size, 12);
    }

    /// Two payloads may share a CAN bus and may not share an identifier.
    ///
    /// A bus is shared by design -- the reason an I2C payload claims its
    /// address rather than its bus -- and two handlers filtering on one
    /// identifier would each read the other's frames.
    #[test]
    fn a_can_bus_is_shared_and_an_identifier_is_not() {
        let two = |first: u32, second: u32| {
            format!(
                "version: \"1.0\"\ndescription: two payloads on one bus\npayloads:\n  \
                 - name: a\n    type: can\n    interface: vcan0\n    can_id: {first}\n    \
                 packet_size: 8\n  - name: b\n    type: can\n    interface: vcan0\n    \
                 can_id: {second}\n    packet_size: 8\n"
            )
        };

        let apart = ConfigFormat::Yaml
            .parse::<PayloadConfig>(&two(1, 2))
            .expect("it parses")
            .to_dh_configs()
            .expect("two identifiers on one bus");
        no_two_handlers_claim_one_thing(&apart).expect("a bus is shared by design");

        // Refused where the file is read, the collision rule being one of the
        // rules a file is held to.
        let said = ConfigFormat::Yaml
            .parse::<PayloadConfig>(&two(1, 1))
            .expect("it parses")
            .to_dh_configs()
            .expect_err("two handlers reading each other's frames");
        assert!(
            said.contains("vcan0") && said.contains("0x1"),
            "the error should name the bus and the identifier: {said}"
        );
    }

    /// An interface belongs to a CAN payload and to no other kind.
    #[test]
    fn an_interface_is_refused_on_a_kind_that_is_not_a_can_bus() {
        let said = link(
            "device-file",
            "    path: /dev/urandom\n    packet_size: 1\n    interface: vcan0\n",
        )
        .expect("it parses")
        .to_dh_configs()
        .expect_err("a device file is not a CAN interface");
        assert!(
            said.contains("interface"),
            "the error should name the attribute: {said}"
        );
    }

    /// The turnaround belongs to a line and to no other kind.
    #[test]
    fn rs485_terms_are_refused_on_a_kind_that_is_not_a_line() {
        let peripheral = "    path: /dev/spidev0.0\n    max_speed: 1000000\n    \
                          spi_mode: 0\n    rs485: true\n";
        let said = link("spi", peripheral)
            .expect("it parses")
            .to_dh_configs()
            .expect_err("a peripheral is not an RS-485 bus");
        assert!(
            said.contains("rs485"),
            "the error should name the term: {said}"
        );
    }

    /// A line whose stream section is the given body.
    fn line_stream(body: &str) -> Result<crate::endpoint_config::StreamParams, String> {
        let terms = link_terms("serial", &format!("{A_LINE}    stream:\n{body}"))?;
        Ok(terms.stream().expect("a line has a stream rule").clone())
    }

    #[test]
    fn a_stream_rule_must_say_how_long_a_read_may_be() {
        let said = line_stream("      timeout: 1 s\n").expect_err("max_length is required");
        assert!(said.contains("max_length"), "{said}");
    }

    #[test]
    fn a_timeout_alone_ends_a_read() {
        let stream = line_stream("      max_length: 64\n      timeout: 250 ms\n").expect("accepted");
        assert_eq!(stream.timeout, Some(Duration::from_millis(250)));
        assert!(stream.terminators.is_empty());
        assert!(!stream.is_fixed_length());
    }

    #[test]
    fn terminators_alone_end_a_read() {
        let stream =
            line_stream("      max_length: 64\n      terminators: [10]\n").expect("accepted");
        assert_eq!(stream.timeout, None);
        assert_eq!(stream.terminators, vec![10]);
        assert!(!stream.is_fixed_length());
    }

    #[test]
    fn a_timeout_and_terminators_may_both_end_a_read() {
        let stream =
            line_stream("      max_length: 64\n      timeout: 1 s\n      terminators: [0x04]\n")
                .expect("accepted");
        assert_eq!(stream.timeout, Some(Duration::from_secs(1)));
        assert_eq!(stream.terminators, vec![0x04]);
    }

    #[test]
    fn a_stream_rule_that_ends_no_read_is_refused() {
        let said = line_stream("      max_length: 64\n").expect_err("nothing ends the read");
        assert!(
            said.contains("timeout") && said.contains("terminator"),
            "{said}"
        );
    }

    #[test]
    fn timeout_none_is_how_a_fixed_length_read_is_asked_for() {
        // The documented way to say "hand me max_length bytes at a time".
        let stream = line_stream("      max_length: 12\n      timeout: none\n").expect("accepted");
        assert_eq!(stream.max_length, 12);
        assert_eq!(stream.timeout, None);
        assert!(stream.terminators.is_empty());
        assert!(stream.is_fixed_length());
    }

    #[test]
    fn an_empty_terminator_list_is_refused() {
        let said = line_stream("      max_length: 64\n      terminators: []\n")
            .expect_err("an empty list is not a list");
        assert!(said.contains("terminator list"), "{said}");
    }

    #[test]
    fn a_timeouts_units_are_understood() {
        for (text, want) in [
            ("900 us", Duration::from_micros(900)),
            ("250 ms", Duration::from_millis(250)),
            ("3 s", Duration::from_secs(3)),
        ] {
            let stream = line_stream(&format!("      max_length: 8\n      timeout: {text}\n"))
                .unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!(stream.timeout, Some(want), "for {text}");
        }
    }

    /// A number with no unit after it is refused, and so is one written
    /// against its unit.
    ///
    /// Two halves of the same rule: a length of time is a number, whitespace
    /// and a unit, so "250" is missing the unit and "250ms" is missing the
    /// whitespace. Each is told which, rather than both being told that the
    /// value is not a length of time.
    #[test]
    fn a_timeout_without_a_unit_is_refused() {
        let said =
            line_stream("      max_length: 8\n      timeout: 250\n").expect_err("250 of what");
        assert!(said.contains("a number, a space, and a unit"), "{said}");

        let said = line_stream("      max_length: 8\n      timeout: 250ms\n")
            .expect_err("written together");
        assert!(said.contains("\"250 ms\""), "{said}");
    }

    #[test]
    fn a_zero_timeout_is_refused_in_favour_of_none() {
        let said = line_stream("      max_length: 8\n      timeout: 0 ms\n")
            .expect_err("a read that times out at once reads nothing");
        assert!(
            said.contains("no interval at all") && said.contains("none"),
            "{said}"
        );
    }

    /// A timeout takes every unit and every number an interval takes.
    ///
    /// One grammar for a length of time: see `graph::parse_interval`, which
    /// this reads through. What a timeout has of its own is the word `none`.
    #[test]
    fn a_timeout_takes_what_an_interval_takes() {
        for (text, want) in [
            ("1.5 s", Duration::from_millis(1_500)),
            ("5 min", Duration::from_secs(300)),
            ("1 h", Duration::from_secs(3_600)),
            ("2_000 ms", Duration::from_secs(2)),
        ] {
            let stream = line_stream(&format!("      max_length: 8\n      timeout: {text}\n"))
                .unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!(stream.timeout, Some(want), "for {text}");
        }

        // And what neither takes: an ambiguous unit, by the name the one
        // grammar refuses it with.
        for text in ["5m", "1y", "6mon", "0x8ms"] {
            let said = line_stream(&format!("      max_length: 8\n      timeout: {text}\n"))
                .expect_err("not a length of time");
            assert!(said.contains("timeout"), "{text}: {said}");
        }
    }

    #[test]
    fn terminators_are_a_sequence_or_a_string_of_them() {
        let sequence = line_stream("      max_length: 8\n      terminators: [0x0D, 0x0A]\n")
            .expect("a sequence");
        let text = line_stream("      max_length: 8\n      terminators: \"0x0D, 0x0A\"\n")
            .expect("a string");
        assert_eq!(sequence.terminators, vec![0x0D, 0x0A]);
        assert_eq!(sequence.terminators, text.terminators);
    }

    #[test]
    fn terminators_are_written_in_decimal_or_hexadecimal() {
        let stream = line_stream("      max_length: 8\n      terminators: [13, 0x0A]\n")
            .expect("either spelling");
        assert_eq!(stream.terminators, vec![13, 10]);
    }

    #[test]
    fn a_terminator_wider_than_a_byte_is_refused() {
        let said = line_stream("      max_length: 8\n      terminators: [256]\n")
            .expect_err("a line carries bytes");
        assert!(said.contains("not a byte value"), "{said}");
    }

    /// A line says its framing, and the words of a clocked line are not its.
    ///
    /// This type is start-stop framing and nothing else now. A line whose bits
    /// are carried on a clock is an `hdlc` payload -- a network interface, with
    /// the clock and the coding set on it -- so the words that belong to one
    /// are refused here, where they used to be accepted and recorded.
    #[test]
    fn a_line_is_start_stop_and_says_so() {
        let line = |extra: &str| {
            link_terms(
                "serial",
                &format!(
                    "    path: /dev/ttyS0\n    datarate: 9600\n    byte_length: 8\n    \
                     stream:\n      max_length: 8\n      timeout: none\n{extra}"
                ),
            )
        };
        let serial = |extra: &str| match line(extra) {
            Ok(crate::endpoint_config::GroupKind::Serial(serial)) => serial,
            Ok(other) => panic!("{other:?}"),
            Err(e) => panic!("{extra}: {e}"),
        };

        // Stop bits are required: they are what start-stop framing uses in
        // place of a clock, so a line without them is not one of these.
        let start_stop = serial("    stop_bits: 2\n");
        assert_eq!(start_stop.stop_bits, crate::StopBits::Two);
        let said = line("").expect_err("stop bits are required");
        assert!(said.contains("stop_bits"), "{said}");

        // Parity defaults to none, which is what nearly every device link
        // uses and what a port was set to before this could be stated.
        assert_eq!(start_stop.parity, crate::Parity::None);
        assert_eq!(
            serial("    stop_bits: 1\n    parity: even\n").parity,
            crate::Parity::Even
        );

        // A clocked line's words belong to the hdlc type and are refused
        // here, each by name.
        for stated in [
            "    clock_type: external\n",
            "    encoding: nrzi\n",
            "    clock_rate: 64000\n",
            "    frame_check: crc16_pr1\n",
            "    standard: v35\n",
            "    loopback: true\n",
        ] {
            let said = line(&format!("    stop_bits: 1\n{stated}"))
                .expect_err("a clocked line's word on a start-stop line");
            let named = stated.trim().split(':').next().unwrap();
            assert!(said.contains(named), "{stated}: {said}");
        }

        // And a parity that is a frame check is refused with the five a
        // character can have: a CRC is not one of them.
        let said = line("    stop_bits: 1\n    parity: crc16_pr1\n")
            .expect_err("a character's parity is not a frame check");
        assert!(said.contains("none, even, odd, mark, or space"), "{said}");
    }

    #[test]
    fn stop_bits_are_one_one_and_a_half_or_two() {
        for (text, want) in [
            ("1", crate::StopBits::One),
            ("1.5", crate::StopBits::OnePointFive),
            ("2", crate::StopBits::Two),
        ] {
            let terms = link_terms(
                "serial",
                &format!(
                    "    path: /dev/ttyS0\n    datarate: 9600\n    \
                     stop_bits: {text}\n    byte_length: 8\n    stream:\n      \
                     max_length: 8\n      timeout: none\n"
                ),
            )
            .unwrap_or_else(|e| panic!("{text}: {e}"));
            match terms {
                crate::endpoint_config::GroupKind::Serial(serial) => {
                    assert_eq!(serial.stop_bits, want, "for {text}")
                }
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn a_byte_length_no_uart_offers_is_refused() {
        let said = link_terms(
            "serial",
            "    path: /dev/ttyS0\n    datarate: 9600\n    \
             stop_bits: 1\n    byte_length: 9\n    stream:\n      max_length: 8\n      \
             timeout: none\n",
        )
        .expect_err("a UART offers five to eight bits");
        assert!(said.contains("out of range"), "{said}");
    }
    #[test]
    fn a_packet_size_of_zero_is_refused() {
        // A packet of no bytes is not a packet. Every kind of payload is
        // asked, because the size is one every kind states.
        for (kind, rest) in [
            ("device-file", "    path: /dev/null\n"),
            (
                "network",
                "    protocol: udp\n    payload_interface: 127.0.0.1:5000\n",
            ),
            ("i2c", "    path: /dev/i2c-1\n    payload_address: 0x40\n"),
            (
                "spi",
                "    path: /dev/spidev0.0\n    max_speed: 1000000\n    spi_mode: 0\n",
            ),
        ] {
            let said = link(kind, &format!("{rest}    packet_size: 0\n"))
                .expect("it parses")
                .to_dh_configs()
                .expect_err(kind);
            assert!(said.contains("greater than zero"), "{kind}: {said}");
        }
    }

    // -- a bus and what sits on it ------------------------------------------

    /// One peripheral on a bus, with the given terms.
    fn bus(terms: &str) -> Result<crate::node::payload::i2c::I2cParams, String> {
        match link_terms("i2c", &format!("    path: /dev/i2c-1\n{terms}"))? {
            crate::endpoint_config::GroupKind::I2c(i2c) => Ok(i2c),
            other => panic!("{other:?}"),
        }
    }

    /// Where a payload on an I2C bus ends up, which is the bus and a place on
    /// it rather than a device node alone.
    fn bus_place(terms: &str) -> Result<EndpointConfig, String> {
        let handlers = link("i2c", &format!("    packet_size: 8\n{terms}"))?.to_dh_configs()?;
        Ok(handlers[0].endpoint.clone())
    }

    #[test]
    fn a_bus_reads_its_terms() {
        let i2c = bus(
            "    ten_bit: false\n    pec: true\n    retries: 3\n    timeout: 50 ms\n    \
             payload_address: 0x48\n",
        )
        .expect("every term of a bus");
        assert!(!i2c.ten_bit);
        assert!(i2c.pec);
        assert_eq!(i2c.retries, 3);
        assert_eq!(i2c.timeout, Some(Duration::from_millis(50)));
    }

    #[test]
    fn every_term_of_a_bus_has_a_default() {
        // Nothing but the kind and where it is is required: the protocol
        // fixes the framing, and every knob here has a quiet setting.
        let i2c = bus("    payload_address: 16\n").expect("a bus states little");
        assert!(!i2c.ten_bit);
        assert!(!i2c.pec);
        assert_eq!(i2c.retries, 0);
        assert_eq!(i2c.timeout, None);
    }

    #[test]
    fn a_bus_timeout_is_rounded_up_to_the_drivers_resolution() {
        let i2c = bus("    timeout: 25 ms\n    payload_address: 8\n").expect("a timeout");
        assert_eq!(i2c.timeout, Some(Duration::from_millis(25)));
        assert_eq!(i2c.effective_timeout(), Some(Duration::from_millis(30)));
    }

    #[test]
    fn a_payload_on_a_bus_carries_the_bus_and_its_place_on_it() {
        let place = bus_place("    path: /dev/i2c-2\n    payload_address: 0x49\n").expect("a place");
        assert_eq!(
            place,
            EndpointConfig::I2c(I2cConfig {
                bus: "/dev/i2c-2".to_string(),
                address: 0x49,
                ten_bit: false,
                pec: false,
            })
        );
    }

    #[test]
    fn an_address_too_wide_for_the_addressing_is_refused() {
        let said = bus_place("    path: /dev/i2c-2\n    payload_address: 0x90\n")
            .expect_err("0x90 needs eight bits");
        assert!(said.contains("does not fit in 7 bits"), "{said}");

        // ...and the same address is fine once the payload says so.
        let place = bus_place("    path: /dev/i2c-2\n    payload_address: 0x90\n    ten_bit: true\n")
            .expect("ten bits hold it");
        match place {
            EndpointConfig::I2c(i2c) => {
                assert_eq!(i2c.address, 0x90);
                assert!(i2c.ten_bit);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_address_the_specification_reserves_is_refused() {
        // In range, but the specification keeps these, so no device can
        // answer to one.
        for address in ["0x00", "0x01", "0x07", "0x78", "0x7B", "0x7F"] {
            let said = bus_place(&format!("    path: /dev/i2c-1\n    payload_address: {address}\n"))
                .expect_err(address);
            assert!(
                said.contains("reserved by the I2C specification"),
                "for {address}: {said}"
            );
        }
    }

    #[test]
    fn the_addresses_either_side_of_the_reserved_blocks_are_accepted() {
        for (address, want) in [("0x08", 0x08), ("0x77", 0x77)] {
            let place = bus_place(&format!("    path: /dev/i2c-1\n    payload_address: {address}\n"))
                .unwrap_or_else(|e| panic!("{address} should be usable: {e}"));
            match place {
                EndpointConfig::I2c(i2c) => assert_eq!(i2c.address, want, "for {address}"),
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn ten_bit_addressing_has_no_reserved_block() {
        // A 10-bit transfer carries its address after the 0x78 prefix, so the
        // whole space is available and the 7-bit reservations do not apply.
        for address in ["0x00", "0x78", "0x3FF"] {
            bus_place(&format!(
                "    path: /dev/i2c-1\n    ten_bit: true\n    payload_address: {address}\n"
            ))
            .unwrap_or_else(|e| panic!("{address} should be usable with ten_bit: {e}"));
        }
    }

    #[test]
    fn a_payload_on_a_bus_says_where_on_it_it_is() {
        let said = bus_place("    path: /dev/i2c-1\n").expect_err("which device on the bus");
        assert!(said.contains("address"), "{said}");

        // And which bus, the two together being what tells one device on a
        // bus from another.
        let said = bus_place("    payload_address: 0x48\n").expect_err("which bus");
        assert!(said.contains("reached through"), "{said}");
    }

    #[test]
    fn a_payload_that_is_on_no_link_is_given_no_stream_rule() {
        // The rule says where one read of a line ends. A device read whole
        // and a datagram that arrives whole have no use for one, so a file
        // that gives them one is told, rather than having the section read
        // and dropped.
        for (kind, rest) in [
            ("device-file", "    path: /dev/null\n"),
            (
                "network",
                "    protocol: udp\n    payload_interface: 127.0.0.1:5000\n",
            ),
        ] {
            let said = link(
                kind,
                &format!(
                    "{rest}    packet_size: 8\n    stream:\n      max_length: 8\n      \
                     timeout: none\n"
                ),
            )
            .expect("it parses")
            .to_dh_configs()
            .expect_err(kind);
            assert!(
                said.contains("stream") && said.contains("does not apply"),
                "{kind}: {said}"
            );
        }
    }

    // -- a peripheral -------------------------------------------------------

    /// One peripheral with the given terms.
    fn peripheral(terms: &str) -> Result<crate::node::payload::spi::SpiParams, String> {
        match link_terms("spi", &format!("    path: /dev/spidev0.0\n{terms}"))? {
            crate::endpoint_config::GroupKind::Spi(spi) => Ok(spi),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_peripheral_reads_its_terms() {
        let spi = peripheral(
            "    max_speed: 10000000\n    spi_mode: 3\n    bits_per_word: 16\n    \
             bit_order: lsb\n    cs_active: high\n",
        )
        .expect("every term of a peripheral");
        assert_eq!(spi.max_speed, 10_000_000);
        assert_eq!(spi.mode, crate::SpiMode::Mode3);
        assert_eq!(spi.bits_per_word.bits(), 16);
        assert_eq!(spi.bit_order, crate::BitOrder::LsbFirst);
        assert_eq!(spi.cs_active, crate::CsActive::High);
    }

    #[test]
    fn a_peripherals_defaults_are_eight_bit_msb_first_and_active_low() {
        let spi = peripheral("    max_speed: 1000000\n    spi_mode: 0\n").expect("the usual");
        assert_eq!(spi.bits_per_word.bits(), 8);
        assert_eq!(spi.bit_order, crate::BitOrder::MsbFirst);
        assert_eq!(spi.cs_active, crate::CsActive::Low);
    }

    #[test]
    fn a_peripheral_says_which_mode_it_is_clocked_in() {
        let said = peripheral("    max_speed: 1000000\n").expect_err("a mode is required");
        assert!(said.contains("mode"), "{said}");

        let said = peripheral("    max_speed: 1000000\n    spi_mode: 4\n")
            .expect_err("there are four of them");
        assert!(said.contains("not one of 0, 1, 2, or 3"), "{said}");
    }

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

    #[test]
    fn a_peripheral_is_named_by_its_device_node_alone() {
        // The node names the bus and the chip select, so an address is not
        // merely unnecessary but wrong.
        let said = link(
            "spi",
            "    path: /dev/spidev0.0\n    max_speed: 1000000\n    spi_mode: 0\n    \
             packet_size: 8\n    payload_address: 0x10\n",
        )
        .expect("it parses")
        .to_dh_configs()
        .expect_err("a peripheral has no address");
        assert!(said.contains("address"), "{said}");
    }

    #[test]
    fn a_bus_is_given_no_stream_rule() {
        // A master clocks exactly as many bytes as it asks for, so there is
        // no rule to give for where a read ends.
        for (kind, rest) in [
            ("i2c", "    path: /dev/i2c-1\n    payload_address: 0x40\n"),
            (
                "spi",
                "    path: /dev/spidev0.0\n    max_speed: 1\n    spi_mode: 0\n",
            ),
        ] {
            let said = link_terms(
                kind,
                &format!("{rest}    stream:\n      max_length: 8\n      timeout: none\n"),
            )
            .expect_err(kind);
            assert!(
                said.contains("stream") && said.contains("does not apply"),
                "{kind}: {said}"
            );
        }
    }

    #[test]
    fn a_term_of_another_kind_of_link_is_refused() {
        // The rule that a misplaced term is reported where it was written,
        // checked across the cross-product rather than one way.
        for (kind, rest, foreign) in [
            (
                "i2c",
                "    path: /dev/i2c-1\n    payload_address: 0x40\n",
                "    datarate: 9600\n",
            ),
            (
                "i2c",
                "    path: /dev/i2c-1\n    payload_address: 0x40\n",
                "    spi_mode: 0\n",
            ),
            (
                "spi",
                "    path: /dev/spidev0.0\n    max_speed: 1\n    spi_mode: 0\n",
                "    pec: true\n",
            ),
            (
                "serial",
                "    path: /dev/ttyS0\n    datarate: 9600\n    \
                 stop_bits: 1\n    byte_length: 8\n    stream:\n      max_length: 8\n      \
                 timeout: none\n",
                "    cs_active: low\n",
            ),
        ] {
            let said = link_terms(kind, &format!("{rest}{foreign}")).expect_err(foreign);
            let named = foreign.trim().split(':').next().unwrap();
            assert!(
                said.contains(named) && said.contains("does not apply"),
                "{foreign} on {kind}: {said}"
            );
        }
    }

    #[test]
    fn a_kind_of_payload_that_is_not_one_is_refused() {
        let said = link("rs485", "    path: /dev/ttyS0\n    packet_size: 8\n")
            .expect("it parses")
            .to_dh_configs()
            .expect_err("there is no such kind");
        assert!(
            said.contains("rs485") && said.contains("serial"),
            "the refusal says what the kinds are: {said}"
        );
    }

    #[test]
    fn the_links_round_trip_between_yaml_and_xml() {
        let yaml = "version: \"1.0\"\ndescription: a bus and a peripheral\n\
                    payloads:\n  \
                    - name: a\n    type: i2c\n    path: /dev/i2c-0\n    \
                    payload_address: 0x2A\n    pec: true\n    retries: 2\n    \
                    packet_size: 8\n  \
                    - name: b\n    type: spi\n    path: /dev/spidev1.0\n    \
                    max_speed: 8000000\n    spi_mode: 1\n    bit_order: lsb\n    \
                    packet_size: 8\n";
        let xml = r#"<payload-configuration>
  <version>1.0</version>
  <description>a bus and a peripheral</description>
  <payloads>
    
    <name>a</name>
    <type>i2c</type>
    <path>/dev/i2c-0</path>
    <payload_address>0x2A</payload_address>
    <pec>true</pec>
    <retries>2</retries>
    <packet_size>8</packet_size>
  </payloads>
  <payloads>
    
    <name>b</name>
    <type>spi</type>
    <path>/dev/spidev1.0</path>
    <max_speed>8000000</max_speed>
    <spi_mode>1</spi_mode>
    <bit_order>lsb</bit_order>
    <packet_size>8</packet_size>
  </payloads>
</payload-configuration>"#;
        let from_yaml = ConfigFormat::Yaml
            .parse::<PayloadConfig>(yaml)
            .expect("the YAML parses");
        let from_xml = ConfigFormat::Xml
            .parse::<PayloadConfig>(xml)
            .expect("the XML parses");
        assert_eq!(from_yaml, from_xml);
        assert_eq!(
            from_yaml.to_dh_configs().expect("the handlers"),
            from_xml.to_dh_configs().expect("the handlers")
        );
    }
}
