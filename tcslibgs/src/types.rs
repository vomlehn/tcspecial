//! Type definitions shared between ground and space software

use serde::{Deserialize, Serialize};

use crate::endpoint_config_serial::{ClockType, Encoding, FrameCheck, Parity, StopBits};
use crate::endpoint_config_spi::{BitOrder, CsActive, SpiMode};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

/// Timestamp type for spacecraft time
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Timestamp {
    /// Seconds since UNIX epoch
    pub seconds: u64,
    /// Nanoseconds within the current second
    pub nanoseconds: u32,
}

impl Timestamp {
    /// Create a new timestamp from the current system time
    pub fn now() -> Self {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Self {
            seconds: duration.as_secs(),
            nanoseconds: duration.subsec_nanos(),
        }
    }

    /// When a system clock reading happened, as a timestamp.
    ///
    /// For the times a program takes from its own clock rather than from
    /// telemetry -- a beacon's arrival, say -- so that they are shown by the
    /// one function that shows every other time here.
    pub fn at(time: SystemTime) -> Self {
        let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
        Self {
            seconds: duration.as_secs(),
            nanoseconds: duration.subsec_nanos(),
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
    /// The nanoseconds are deliberately unused. A panel refreshed twice a
    /// second cannot show them, and a time that changed in its last digits
    /// between two looks would read as traffic that had not happened.
    pub fn time_of_day(&self) -> String {
        let within_a_day = self.seconds % 86_400;
        format!(
            "{:02}:{:02}:{:02}",
            within_a_day / 3600,
            (within_a_day % 3600) / 60,
            within_a_day % 60
        )
    }
}

/// Data handler identifier
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DHId(pub u32);

impl Ord for DHId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for DHId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
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
/// Public because two files naming one host differently is the same question
/// as two handlers doing so: a simulator configuration stating `localhost`
/// for a payload the payload file put at `127.0.0.1` has not disagreed with
/// it.
pub fn one_host(address: &str) -> String {
    match address {
        "localhost" | "127.0.0.1" | "::1" => "localhost".to_string(),
        other => other.to_string(),
    }
}

/// What this handler claims, and what to call each claim in an error.
fn claims_of(dh: &DHConfig) -> Vec<(Claim, String)> {
    let mut claims = Vec::new();

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
        EndpointConfig::Spi(spi) => Claim::Path(spi.path.clone()),
        EndpointConfig::I2c(i2c) => Claim::OnBus {
            bus: i2c.bus.clone(),
            address: i2c.address,
        },
    };
    claims.push((payload, format!("{}'s payload", dh.name.0)));

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
    let mut taken: BTreeMap<Claim, String> = BTreeMap::new();

    for dh in handlers {
        for (claim, what) in claims_of(dh) {
            if let Some(first) = taken.get(&claim) {
                return Err(format!(
                    "{first} and {what} both want {}",
                    claim_text(&claim)
                ));
            }
            taken.insert(claim, what);
        }
    }

    Ok(())
}

impl DHType {
    /// The word a configuration file uses for this kind.
    ///
    /// One table, read in both directions and from both files, so that the
    /// payload file and the simulator file cannot disagree about what a kind
    /// is called. The spellings are also what an error says, which is how a
    /// file is told what it could have written instead.
    pub fn spelling(&self) -> &'static str {
        match self {
            DHType::Network => "network",
            DHType::Device => "device",
            DHType::Serial => "serial",
            DHType::I2c => "i2c",
            DHType::Spi => "spi",
        }
    }

    /// Which kind a file's word names, if it names one.
    pub fn from_spelling(text: &str) -> Option<DHType> {
        [
            DHType::Network,
            DHType::Device,
            DHType::Serial,
            DHType::I2c,
            DHType::Spi,
        ]
        .into_iter()
        .find(|kind| kind.spelling() == text)
    }

    /// Every kind, as a file would write them, for an error to list.
    pub fn spellings() -> String {
        "network, device, serial, i2c, or spi".to_string()
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

/// Data handler name
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DHName(pub String);

impl DHName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// Arm key for restart commands
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArmKey(pub u64);

/// Beacon interval time in milliseconds
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct BeaconTime(pub u32);

impl Default for BeaconTime {
    fn default() -> Self {
        Self(5000) // 5 seconds default
    }
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
    /// Whether the line is start-stop framed or synchronous.
    pub asynchronous: bool,
    /// Stop bits following each byte, for a start-stop line. `None` for a
    /// synchronous one, which has no use for them: a stop bit is what
    /// start-stop framing uses in place of a clock.
    pub stop_bits: Option<StopBits>,
    /// Parity per character, for a start-stop line, which is what termios
    /// sets. `None` for a synchronous one.
    pub parity: Option<Parity>,
    /// What a synchronous line's clock, coding and frame check are, and
    /// `None` for a start-stop line. Recorded rather than applied: the kernel
    /// drives such a line through a network interface of its own rather than
    /// through the terms of a device file, so a handler reading a path cannot
    /// set them -- and a link whose configuration says what it expects can be
    /// checked against the equipment and reported plainly.
    pub clock_type: Option<ClockType>,
    pub encoding: Option<Encoding>,
    pub frame_check: Option<FrameCheck>,
    pub loopback: Option<bool>,
    /// Data bits per byte. A count rather than a `ByteLength`, which is the
    /// configuration language's checked form: by the time a handler is
    /// started the count has been checked, and this is the number the line
    /// is set to.
    pub byte_length: u8,
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
        }
    }
}

/// How a payload is made to send: on its own, or when asked.
///
/// Payloads are generally one of two kinds, and which one a payload is decides
/// where the timing of it is written down. A payload that sends on its own
/// needs nothing from tcspecial, and how fast it sends is a property of the
/// payload rather than of the handler -- so a payload file says nothing about
/// it, and a simulated one takes its rate from the simulator file. A payload
/// that answers a request needs tcspecial to send one, at some rate, and both
/// are flight behaviour: what to send comes from the payload's interface
/// document and so belongs in the payload file.
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
        /// How often to send it, in milliseconds. The payload file's
        /// `packet_interval_ms`: tcspecial does the sending, so the rate is
        /// flight behaviour.
        interval_ms: u32,
    },
}

impl Default for DHMode {
    /// Periodic, which is both the commoner kind and what every payload file
    /// written before there was a mode describes.
    fn default() -> Self {
        DHMode::Periodic
    }
}

impl DHMode {
    /// What to send and how often, for a handler that has to ask.
    pub fn polling(&self) -> Option<(&[u8], u32)> {
        match self {
            DHMode::Periodic => None,
            DHMode::Triggered {
                trigger,
                interval_ms,
            } => Some((trigger, *interval_ms)),
        }
    }
}

/// Data handler configuration
///
/// Compared as a value so that one file's handlers can be held against
/// another's: a payload set written in YAML, JSON and XML must produce the
/// same handlers, and that is a comparison rather than an inspection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DHConfig {
    pub dh_id: DHId,
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
    /// but a file describing payloads is complete without it, and so is a
    /// handler converted from an endpoint configuration, which has no OC side
    /// to give.
    pub oc: Option<NetworkConfig>,
    /// How this payload is made to send; see [`DHMode`].
    ///
    /// Defaulted rather than required, so that a payload file written before
    /// there was a mode describes what it always described: a payload that
    /// sends on its own.
    #[serde(default)]
    pub mode: DHMode,
}

/// Payload configuration file structure
///
/// A payload file describes payloads and nothing else. It carries no packet
/// interval: how fast a payload produces packets is a property of a
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
    /// Optional, and read by nothing yet: tcspecial still takes its own
    /// configuration from the file `TCSPECIAL_CONFIG_PATH` names, and the
    /// address it serves commands on from its command line before that. What
    /// this does now is carry the section and refuse a malformed one, so a
    /// file that states it is checked rather than ignored.
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
    pub fn to_dh_configs(&self) -> Result<Vec<DHConfig>, String> {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for group in &self.payload_groups {
            if !seen.insert(group.name.as_str()) {
                return Err(format!(
                    "payload group \"{}\" is defined more than once",
                    group.name
                ));
            }
        }

        // A payload is addressed by its id and found by its name, and both
        // have to pick out one payload. A repeat of either parses cleanly and
        // then loses a payload: tcspecial keeps its handlers by id, so a
        // repeated id has one silently replace the other, and a simulator
        // file is joined to this one by name, so a repeated name has one
        // entry drive two payloads. Worded as the endpoint configuration
        // format words the same rule, which has had these checks all along.
        let mut by_id: BTreeMap<u32, &str> = BTreeMap::new();
        let mut by_name: BTreeSet<&str> = BTreeSet::new();
        for payload in &self.payloads {
            if let Some(first) = by_id.insert(payload.dh_id, payload.name.as_str()) {
                return Err(format!(
                    "payloads \"{}\" and \"{}\" share dh_id {}",
                    first, payload.name, payload.dh_id
                ));
            }
            if !by_name.insert(payload.name.as_str()) {
                return Err(format!(
                    "payload \"{}\" is defined more than once",
                    payload.name
                ));
            }
        }

        let configs: Vec<DHConfig> = self
            .payloads
            .iter()
            .map(|dh| {
                let group = match &dh.group {
                    Some(name) => Some(self.group(name).ok_or_else(|| {
                        format!(
                            "data handler \"{}\" names group \"{}\", which is not defined",
                            dh.name, name
                        )
                    })?),
                    None => None,
                };
                dh.to_dh_config_in(group)
            })
            .collect::<Result<_, String>>()?;

        no_two_handlers_claim_one_thing(&configs)?;

        // The tcspecial section is checked here though nothing reads it yet.
        // A section a file states is a section its author meant, and one that
        // could never be used -- a protocol that is not a protocol, say -- is
        // better refused by whoever loads the file than discovered by whoever
        // first tries to use it.
        if let Some(ci) = &self.tcspecial {
            ci.to_ci_config().map_err(|e| format!("tcspecial: {e}"))?;
        }

        // A group no handler names has no effect on the configuration, which
        // is exactly what a group whose name a handler misspelled looks like.
        // Checked after the handlers, so that the misspelling is reported from
        // the handler's end, where the name actually is.
        let named: BTreeSet<&str> = self
            .payloads
            .iter()
            .filter_map(|dh| dh.group.as_deref())
            .collect();
        if let Some(unused) = self
            .payload_groups
            .iter()
            .find(|group| !named.contains(group.name.as_str()))
        {
            // Worded exactly as the endpoint and simulator configuration
            // formats word the same rule, so that one rule reads as one rule
            // wherever it is met.
            return Err(format!(
                "data handler group \"{}\" is named by no data handler: name it \
                 from one, or remove the group",
                unused.name
            ));
        }

        Ok(configs)
    }
}

/// A named group of attributes that several data handlers share.
///
/// Every attribute is optional, so a group carries exactly what its handlers
/// have in common and no more. What a group deliberately cannot carry is a
/// `dh_id` or a `name`: those are what tell one handler of a group from
/// another, and so belong to the handler.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DHGroupJson {
    /// Name data handlers use to refer to this group.
    pub name: String,
    #[serde(rename = "type", default)]
    pub dh_type: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub packet_size: Option<usize>,
    #[serde(default)]
    pub oc_address: Option<String>,
    #[serde(default)]
    pub oc_port: Option<u16>,
    /// ``periodic`` or ``triggered``; see [`DHMode`]. Absent is periodic.
    #[serde(default)]
    pub mode: Option<String>,
    /// What to send to make a triggered payload answer. Triggered only.
    #[serde(default)]
    pub trigger: Option<String>,
    /// How often to send it. Triggered only.
    #[serde(default)]
    pub trigger_interval_ms: Option<u32>,
    /// Named here only to be refused.
    ///
    /// How fast a payload sends is a property of the simulation, so an
    /// interval belongs in the simulator configuration file. It was ignored
    /// here for a while, which was the same silence the mode rules exist to
    /// prevent: a file that states an interval has said something about the
    /// payload's timing, and reading it as though it had not is reading a
    /// different file than the one that was written. Kept as a field rather
    /// than left unknown so that the error can name the file it belongs in.
    #[serde(default)]
    pub packet_interval_ms: Option<u32>,
}

/// JSON representation of DH config
///
/// Every attribute but `dh_id` and `name` is optional, because a handler
/// naming a group need only state what it does not take from that group.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DHConfigJson {
    pub dh_id: u32,
    pub name: String,
    /// Name of the group supplying the attributes this handler does not state.
    #[serde(default)]
    pub group: Option<String>,
    #[serde(rename = "type", default)]
    pub dh_type: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub packet_size: Option<usize>,
    /// Address this handler exchanges payload data with the OC on.
    ///
    /// Commonly shared by every handler of a group, which is why a group may
    /// carry it; the port is what tells one handler of a group from another,
    /// like a network endpoint's port.
    #[serde(default)]
    pub oc_address: Option<String>,
    #[serde(default)]
    pub oc_port: Option<u16>,
    /// ``periodic`` or ``triggered``; see [`DHMode`]. Absent is periodic.
    #[serde(default)]
    pub mode: Option<String>,
    /// What to send to make a triggered payload answer. Triggered only.
    #[serde(default)]
    pub trigger: Option<String>,
    /// How often to send it. Triggered only.
    #[serde(default)]
    pub trigger_interval_ms: Option<u32>,
    /// Named here only to be refused; see [`DHGroupJson::packet_interval_ms`].
    #[serde(default)]
    pub packet_interval_ms: Option<u32>,
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
            .address
            .as_deref()
            .or_else(|| group.and_then(|g| g.address.as_deref()));
        let port = self.port.or_else(|| group.and_then(|g| g.port));
        let path = self
            .path
            .as_deref()
            .or_else(|| group.and_then(|g| g.path.as_deref()));
        let packet_size = self
            .packet_size
            .or_else(|| group.and_then(|g| g.packet_size));
        let oc_address = self
            .oc_address
            .as_deref()
            .or_else(|| group.and_then(|g| g.oc_address.as_deref()));
        let oc_port = self.oc_port.or_else(|| group.and_then(|g| g.oc_port));
        let mode = self
            .mode
            .as_deref()
            .or_else(|| group.and_then(|g| g.mode.as_deref()));
        let trigger = self
            .trigger
            .as_deref()
            .or_else(|| group.and_then(|g| g.trigger.as_deref()));
        let trigger_interval_ms = self
            .trigger_interval_ms
            .or_else(|| group.and_then(|g| g.trigger_interval_ms));
        // The rate a triggered payload's trigger goes out at, which is the
        // one interval this file states.
        let packet_interval_ms = self
            .packet_interval_ms
            .or_else(|| group.and_then(|g| g.packet_interval_ms));

        let kind = match dh_type {
            Some(text) => DHType::from_spelling(text).ok_or_else(|| {
                format!("{text} is not a kind of payload: expected {}", DHType::spellings())
            })?,
            None => return Err("Missing type".to_string()),
        };

        // An attribute of another kind is an error rather than something
        // ignored, which is the rule the endpoint configuration format has
        // for its own groups and is worded the same way here. Ignoring one
        // is how a device payload comes to carry a port nothing reads, and
        // how a network payload carrying a path looks configured and is not.
        // Checked after the group has been laid under the payload, because an
        // attribute inherited from a group reaches the handler exactly as one
        // the payload states does.
        // Each entry is the attribute, whether this payload stated it, and
        // whether it is there at all once the group is under it.
        let foreign: &[(&str, bool, bool)] = match kind {
            DHType::Network => &[("path", self.path.is_some(), path.is_some())],
            _ => &[
                ("protocol", self.protocol.is_some(), protocol.is_some()),
                ("address", self.address.is_some(), address.is_some()),
                ("port", self.port.is_some(), port.is_some()),
            ],
        };
        if let Some((field, stated, _)) = foreign.iter().find(|(_, _, present)| *present) {
            // Where it came from, because the two are fixed differently: one
            // line is deleted from the payload, and the other means a payload
            // is in a group that was not written for it.
            let whence = match (stated, group) {
                (false, Some(group)) => format!(", which it takes from group \"{}\"", group.name),
                _ => String::new(),
            };
            return Err(format!(
                "payload \"{}\" is a {} payload, so {field}{whence} does not apply to it",
                self.name,
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
                EndpointConfig::Network(NetworkConfig {
                    protocol,
                    address: address.ok_or("Missing address")?.to_string(),
                    port: port.ok_or("Missing port")?,
                })
            }
            DHType::Device => EndpointConfig::Device(DeviceConfig {
                path: path.ok_or("Missing path")?.to_string(),
            }),
            // A line, a bus and a peripheral carry attributes this file has
            // no words for -- a baud rate, a slave address, a clock mode --
            // so they are described in an endpoint configuration file and
            // named here only by a simulator file checking what it is
            // simulating.
            other => {
                return Err(format!(
                    "a payload file describes a network or device payload, not a {} \
                     one: a {} payload is described in an endpoint configuration file",
                    other.spelling(),
                    other.spelling()
                ))
            }
        };

        // Half an OC address is a mistake rather than a configuration: a
        // handler given a port and no address, or the reverse, cannot be
        // reached and nothing about the file says which was meant.
        let oc = match (oc_address, oc_port) {
            (Some(address), Some(port)) => Some(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: address.to_string(),
                port,
            }),
            (None, None) => None,
            (Some(_), None) => return Err("oc_address without oc_port".to_string()),
            (None, Some(_)) => return Err("oc_port without oc_address".to_string()),
        };

        // What the trigger's rate used to be called. Named so that a file
        // using it is told the name it has now rather than told the word is
        // unknown.
        if trigger_interval_ms.is_some() {
            return Err(format!(
                "payload \"{}\" states trigger_interval_ms, which is \
                 packet_interval_ms now: a triggered payload's rate is the rate its \
                 trigger goes out at, and it is the only interval this file states",
                self.name
            ));
        }

        // Which kind of payload this is, and the fields that belong to that
        // kind and to no other. A periodic payload carrying a trigger, or a
        // triggered one carrying no interval, is a file that has not decided
        // which kind it describes -- reported rather than resolved by
        // guessing, because either guess would run something nobody asked
        // for.
        let mode = match mode {
            None | Some("periodic") => {
                if trigger.is_some() || packet_interval_ms.is_some() {
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

                let written = trigger.ok_or(
                    "a triggered payload answers a request, so it states the trigger \
                     to send",
                )?;
                // Either notation, hexadecimal or a C string; see
                // crate::trigger. The error says which was being read.
                let trigger = crate::trigger::trigger_bytes(written)?;

                let interval_ms = packet_interval_ms.ok_or(
                    "a triggered payload states how often its trigger is sent, as \
                     packet_interval_ms: tcspecial does the sending, so the rate is \
                     flight behaviour and not a simulation setting",
                )?;
                if interval_ms == 0 {
                    return Err("a packet interval of zero would send triggers without \
                                pause"
                        .to_string());
                }
                DHMode::Triggered {
                    trigger,
                    interval_ms,
                }
            }
            Some(other) => {
                return Err(format!(
                    "{other} is not a mode: a payload is periodic or triggered"
                ))
            }
        };

        Ok(DHConfig {
            dh_id: DHId(self.dh_id),
            name: DHName::new(&self.name),
            endpoint,
            packet_size: packet_size.ok_or("Missing packet_size")?,
            oc,
            mode,
        })
    }
}

/// Payload bytes per telemetry log segment file, when the configuration
/// does not say. The segment file header is added on top of this.
fn default_log_segment_bytes() -> u32 {
    65_536
}

/// JSON representation of CI config
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CIConfigJson {
    pub address: String,
    pub port: u16,
    pub protocol: String,
    pub beacon_interval_ms: u32,
    /// Directory holding the telemetry log's segment files. The directory
    /// must already exist. Telemetry logging is disabled when this is absent.
    #[serde(default)]
    pub log_dir: Option<String>,
    /// Payload bytes per segment file. The segment file header is added to
    /// this, so it is the space available for telemetry records.
    #[serde(default = "default_log_segment_bytes")]
    pub log_segment_bytes: u32,
}

/// Command interpreter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CIConfig {
    pub address: String,
    pub port: u16,
    pub protocol: NetworkProtocol,
    pub beacon_interval: BeaconTime,
    /// Directory holding the telemetry log's segment files, or `None` to
    /// run without a telemetry log.
    pub log_dir: Option<String>,
    /// Payload bytes per segment file, not counting the segment file header.
    pub log_segment_bytes: u32,
}

impl CIConfigJson {
    pub fn to_ci_config(&self) -> Result<CIConfig, String> {
        let protocol = match self.protocol.as_str() {
            "tcp" => NetworkProtocol::Tcp,
            "udp" => NetworkProtocol::Udp,
            _ => return Err(format!("Invalid protocol: {}", self.protocol)),
        };

        Ok(CIConfig {
            address: self.address.clone(),
            port: self.port,
            protocol,
            beacon_interval: BeaconTime(self.beacon_interval_ms),
            log_dir: self.log_dir.clone(),
            log_segment_bytes: self.log_segment_bytes,
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
    use crate::ConfigFormat;

    /// A payload file that says nothing about the mode describes the kind of
    /// payload that sends on its own, which is what every file written before
    /// there was a mode describes.
    #[test]
    fn a_payload_that_states_no_mode_sends_on_its_own() {
        use super::*;

        let dh = DHConfigJson {
            dh_id: 0,
            name: "DH0".to_string(),
            group: None,
            dh_type: Some("device".to_string()),
            protocol: None,
            address: None,
            port: None,
            path: Some("/dev/urandom".to_string()),
            packet_size: Some(4),
            oc_address: None,
            oc_port: None,
            mode: None,
            trigger: None,
            trigger_interval_ms: None,
            packet_interval_ms: None,
            sequence: 0,
        };

        assert_eq!(dh.to_dh_config().expect("converts").mode, DHMode::Periodic);
    }

    /// A triggered payload carries what to send and how often, and both reach
    /// the handler: tcspecial does the sending, so both are its business.
    #[test]
    fn a_triggered_payload_carries_its_trigger_and_interval() {
        use super::*;

        let dh = DHConfigJson {
            dh_id: 1,
            name: "DH1".to_string(),
            group: None,
            dh_type: Some("network".to_string()),
            protocol: Some("tcp".to_string()),
            address: Some("localhost".to_string()),
            port: Some(5000),
            path: None,
            packet_size: Some(8),
            oc_address: None,
            oc_port: None,
            mode: Some("triggered".to_string()),
            // As a file writes it: double quotes, where the escape is the
            // carriage return the payload's interface asks for.
            trigger: Some("READ\r".to_string()),
            trigger_interval_ms: None,
            packet_interval_ms: Some(500),
            sequence: 0,
        };

        let config = dh.to_dh_config().expect("converts");
        assert_eq!(
            config.mode,
            DHMode::Triggered {
                trigger: b"READ\r".to_vec(),
                interval_ms: 500,
            }
        );
        assert_eq!(config.mode.polling(), Some((b"READ\r".as_slice(), 500)));
    }

    /// The two kinds do not mix, in either direction, and the complaint says
    /// which file the misplaced setting belongs in.
    #[test]
    fn the_two_kinds_of_payload_do_not_mix() {
        use super::*;

        let bare = |mode: Option<&str>, trigger: Option<&str>, interval: Option<u32>| {
            DHConfigJson {
                dh_id: 2,
                name: "DH2".to_string(),
                group: None,
                dh_type: Some("network".to_string()),
                protocol: Some("tcp".to_string()),
                address: Some("localhost".to_string()),
                port: Some(5000),
                path: None,
                packet_size: Some(8),
                oc_address: None,
                oc_port: None,
                mode: mode.map(str::to_string),
                trigger: trigger.map(str::to_string),
                trigger_interval_ms: None,
                packet_interval_ms: interval,
                sequence: 0,
            }
        };

        // A payload that sends on its own has nothing to be triggered by, and
        // its rate is the simulator's business.
        let e = bare(None, Some("READ"), None).to_dh_config().unwrap_err();
        assert!(e.contains("no trigger"), "{e}");
        let e = bare(Some("periodic"), None, Some(500))
            .to_dh_config()
            .unwrap_err();
        assert!(e.contains("simulator configuration"), "{e}");

        // A payload that answers requests needs both halves of the request.
        let e = bare(Some("triggered"), None, Some(500))
            .to_dh_config()
            .unwrap_err();
        assert!(e.contains("states the trigger"), "{e}");
        let e = bare(Some("triggered"), Some("READ"), None)
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
                dh_id: 3,
                name: "DH3".to_string(),
                group: None,
                dh_type: Some("network".to_string()),
                protocol: Some(protocol.to_string()),
                address: Some("localhost".to_string()),
                port: Some(5000),
                path: None,
                packet_size: Some(8),
                oc_address: None,
                oc_port: None,
                mode: Some("triggered".to_string()),
                trigger: Some("READ".to_string()),
                trigger_interval_ms: None,
                packet_interval_ms: Some(500),
                sequence: 0,
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
            asynchronous: true,
            parity: Some(Parity::None),
            clock_type: None,
            encoding: None,
            frame_check: None,
            loopback: None,
            stop_bits: Some(StopBits::One),
            byte_length: 8,
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
    fn test_dhid_ordering() {
        let id1 = DHId(1);
        let id2 = DHId(2);
        assert!(id1 < id2);
    }

    #[test]
    fn test_timestamp_now() {
        let ts = Timestamp::now();
        assert!(ts.seconds > 0);
    }

    /// Bytes read as hex pairs, and a transfer longer than what is kept
    /// says so.
    #[test]
    fn bytes_read_as_hex_pairs() {
        assert_eq!(bytes_to_hex(&[0x01, 0x02, 0x03], 10), "01 02 03");
        assert_eq!(bytes_to_hex(&[0x01, 0x02, 0x03, 0x04, 0x05], 3), "01 02 03...");
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
            seconds: 3661,
            nanoseconds: 0,
        });
        assert_eq!(sample.panel_lines(), ("01:01:01".to_string(), "52 45 41 44 0D".to_string()));

        // And a transfer longer than the sample keeps shows a head and says
        // so: without the ellipsis a packet of twelve bytes and one of eight
        // read alike.
        let mut sample = DHSample::new();
        sample.record(&[0xAB; 12]);
        sample.time = Some(Timestamp {
            seconds: 0,
            nanoseconds: 0,
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
        let at = |seconds| {
            Timestamp {
                seconds,
                nanoseconds: 0,
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

        // The nanoseconds are not shown, so two times within one second read
        // alike: a panel refreshed twice a second cannot show them, and a
        // time whose last digits changed between two looks would read as
        // traffic that had not happened.
        assert_eq!(
            Timestamp {
                seconds: 1_700_000_000,
                nanoseconds: 999_999_999,
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

    /// A payload file as it is really read, so these tests go through the
    /// same deserialization a file on disk does.
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
    address: localhost
payloads:
  - dh_id: 1
    name: DH1
    group: udp_localhost
    port: 5001
    packet_size: 11
  - dh_id: 3
    name: DH3
    group: udp_localhost
    port: 5003
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
    address: localhost
    packet_size: 11
payloads:
  - dh_id: 0
    name: DH0
    group: udp_localhost
    protocol: tcp
    port: 5000
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
  - dh_id: 2
    name: DH2
    type: device
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

    #[test]
    fn an_oc_address_comes_from_the_handler_or_its_group() {
        let config = payload(
            "
version: \"1.0\"
description: an OC address shared, a port each
payload_groups:
  - name: udp_localhost
    type: network
    protocol: udp
    address: localhost
    oc_address: 127.0.0.1
payloads:
  - dh_id: 1
    name: DH1
    group: udp_localhost
    oc_port: 6001
    port: 5001
    packet_size: 11
",
        );

        let handlers = config.to_dh_configs().unwrap();
        assert_eq!(
            handlers[0].oc,
            Some(NetworkConfig {
                // The OC link is UDP whatever the payload side is.
                protocol: NetworkProtocol::Udp,
                address: "127.0.0.1".to_string(),
                port: 6001,
            })
        );
    }

    #[test]
    fn a_handler_needs_no_oc_address() {
        // A file describing payloads is complete without one; only starting a
        // handler needs it.
        let config = payload(
            "
version: \"1.0\"
description: no OC side at all
payloads:
  - dh_id: 2
    name: DH2
    type: device
    path: /dev/urandom
    packet_size: 1
",
        );

        assert_eq!(config.to_dh_configs().unwrap()[0].oc, None);
    }

    #[test]
    fn half_an_oc_address_is_an_error() {
        // A port with no address, or an address with no port, reaches nothing
        // and says nothing about which was meant.
        for (what, line) in [("port", "oc_address: 127.0.0.1"), ("address", "oc_port: 6000")] {
            let config = payload(&format!(
                "
version: \"1.0\"
description: half an OC address
payloads:
  - dh_id: 0
    name: DH0
    {line}
    type: device
    path: /dev/urandom
    packet_size: 1
"
            ));

            let message = config
                .to_dh_configs()
                .expect_err(&format!("an OC address with no {what} must be rejected"));
            assert!(
                message.contains("oc_"),
                "the error should name the attributes, but said: {message}"
            );
        }
    }

    /// Two payloads, written in each of the three formats.
    fn two_payloads() -> [(ConfigFormat, String); 3] {
        [
            (
                ConfigFormat::Yaml,
                "version: \"1.0\"\ndescription: two\npayloads:\n  \
                 - dh_id: 0\n    name: first\n    type: device\n    path: /dev/null\n    \
                 packet_size: 1\n  \
                 - dh_id: 1\n    name: second\n    type: device\n    path: /dev/zero\n    \
                 packet_size: 2\n"
                    .to_string(),
            ),
            (
                ConfigFormat::Json,
                r#"{"version":"1.0","description":"two","payloads":[
                   {"dh_id":0,"name":"first","type":"device","path":"/dev/null","packet_size":1},
                   {"dh_id":1,"name":"second","type":"device","path":"/dev/zero","packet_size":2}]}"#
                    .to_string(),
            ),
            (
                ConfigFormat::Xml,
                "<payload><version>1.0</version><description>two</description>\
                 <payloads><dh_id>0</dh_id><name>first</name><type>device</type>\
                 <path>/dev/null</path><packet_size>1</packet_size></payloads>\
                 <payloads><dh_id>1</dh_id><name>second</name><type>device</type>\
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
                    - dh_id: 0\n    name: first\n    type: device\n    path: /dev/null\n    \
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
    address: localhost
payloads:
  - dh_id: 0
    name: DH0
    type: network
    protocol: tcp
    address: localhost
    port: 5000
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
  - dh_id: 0
    name: DH0
    type: network
    protocol: udp
    address: localhost
    port: 5000
    packet_size: 12
{second}
"
            )
        };

        let same_id = payload(&two(
            "  - dh_id: 0\n    name: DH1\n    type: network\n    protocol: udp\n    \
             address: localhost\n    port: 5001\n    packet_size: 12",
        ));
        let said = same_id.to_dh_configs().expect_err("two payloads, one id");
        assert!(
            said.contains("DH0") && said.contains("DH1") && said.contains("dh_id 0"),
            "the error should name both payloads and the id: {said}"
        );

        let same_name = payload(&two(
            "  - dh_id: 1\n    name: DH0\n    type: network\n    protocol: udp\n    \
             address: localhost\n    port: 5001\n    packet_size: 12",
        ));
        let said = same_name.to_dh_configs().expect_err("two payloads, one name");
        assert!(
            said.contains("DH0") && said.contains("more than once"),
            "the error should name the payload: {said}"
        );

        // And two payloads that differ in both are fine, which is what the
        // checks must not get in the way of.
        let distinct = payload(&two(
            "  - dh_id: 1\n    name: DH1\n    type: network\n    protocol: udp\n    \
             address: localhost\n    port: 5001\n    packet_size: 12",
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
  - dh_id: 0
    name: DH0
    packet_size: 12
{first}
  - dh_id: 1
    name: DH1
    packet_size: 12
{second}
"
            )
        };

        let udp = |port: u16| {
            format!("    type: network\n    protocol: udp\n    address: localhost\n    port: {port}")
        };
        let device = |path: &str| format!("    type: device\n    path: {path}");

        // The same host and port, written two ways: localhost and 127.0.0.1
        // are one host, so naming it differently does not make it a different
        // port.
        let said = payload(&pair(&udp(5000), "    type: network\n    protocol: udp\n    address: 127.0.0.1\n    port: 5000"))
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

        // A handler's OC address is as exclusive as its payload address, and
        // is compared against it: this one would have to bind the port twice.
        let clash = format!(
            "
version: \"1.0\"
description: an OC address on the payload's own port
payloads:
  - dh_id: 0
    name: DH0
    packet_size: 12
    oc_address: 127.0.0.1
    oc_port: 5000
{}
",
            udp(5000)
        );
        let said = payload(&clash)
            .to_dh_configs()
            .expect_err("one port for the payload and the OC");
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
    /// The one exception the rule has to make, and the reason a claim is on
    /// the address rather than on the bus device: several devices on one I2C
    /// bus is what a bus is for, and refusing that would refuse the normal
    /// case. These handlers cannot be written in a payload file, so they are
    /// built directly.
    #[test]
    fn a_bus_is_shared_and_a_place_on_it_is_not() {
        let on_bus = |id: u32, name: &str, address: u16| DHConfig {
            dh_id: DHId(id),
            name: DHName::new(name),
            endpoint: EndpointConfig::I2c(I2cConfig {
                bus: "/dev/i2c-1".to_string(),
                address,
                ten_bit: false,
                pec: false,
            }),
            packet_size: 4,
            oc: None,
            mode: DHMode::Periodic,
        };

        no_two_handlers_claim_one_thing(&[on_bus(0, "DH0", 0x48), on_bus(1, "DH1", 0x49)])
            .expect("two devices on one bus is what a bus is for");

        let said = no_two_handlers_claim_one_thing(&[
            on_bus(0, "DH0", 0x48),
            on_bus(1, "DH1", 0x48),
        ])
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
        let socket = |id: u32, name: &str, port: u16| DHConfig {
            dh_id: DHId(id),
            name: DHName::new(name),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::UnixStream,
                address: "/tmp/dh.sock".to_string(),
                port,
            }),
            packet_size: 4,
            oc: None,
            mode: DHMode::Periodic,
        };

        let said = no_two_handlers_claim_one_thing(&[socket(0, "DH0", 0), socket(1, "DH1", 7)])
            .expect_err("one socket path, whatever the ports say");
        assert!(said.contains("/tmp/dh.sock"), "{said}");

        let mut device = socket(1, "DH1", 0);
        device.endpoint = EndpointConfig::Device(DeviceConfig {
            path: "/tmp/dh.sock".to_string(),
        });
        assert!(
            no_two_handlers_claim_one_thing(&[socket(0, "DH0", 0), device]).is_err(),
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
    address: localhost
payloads:
  - dh_id: 0
    name: DH0
    group: localhost_udp
    port: {port}
    packet_size: {packet_size}
"
            )
        };

        // The same payload name, the same group name, and the same id, in two
        // files that know nothing of each other.
        let first = payload(&one_set(5000, 12)).to_dh_configs().expect("the first set");
        let second = payload(&one_set(6000, 8)).to_dh_configs().expect("the second set");

        assert_eq!(first[0].name.0, second[0].name.0);
        assert_eq!(first[0].dh_id, second[0].dh_id);
        assert_ne!(
            first[0].packet_size, second[0].packet_size,
            "the two sets describe different payloads under the one name"
        );
    }

    /// An attribute of another kind of payload is refused.
    ///
    /// Ignoring one is how a device payload comes to carry a port nothing
    /// reads, and how a network payload carrying a path looks configured and
    /// is not. The endpoint configuration format has had this rule for its
    /// own groups all along.
    #[test]
    fn an_attribute_of_another_kind_does_not_apply() {
        let device = payload(
            "
version: \"1.0\"
description: a device stating network attributes
payloads:
  - dh_id: 0
    name: DH0
    type: device
    path: /dev/urandom
    protocol: udp
    address: localhost
    port: 5000
    packet_size: 1
",
        );
        let said = device.to_dh_configs().expect_err("a device has no protocol");
        assert!(
            said.contains("device") && said.contains("protocol") && !said.contains("group"),
            "an attribute the payload states itself is not blamed on a group: {said}"
        );

        let network = payload(
            "
version: \"1.0\"
description: a network payload stating a path
payloads:
  - dh_id: 0
    name: DH0
    type: network
    protocol: udp
    address: localhost
    port: 5000
    path: /dev/urandom
    packet_size: 12
",
        );
        let said = network.to_dh_configs().expect_err("a network payload has no path");
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
    address: localhost
payloads:
  - dh_id: 0
    name: DH0
    group: udp_localhost
    type: device
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
  - dh_id: 0
    name: DH0
    type: network
    protocol: udp
    address: localhost
    port: 5000
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
  - dh_id: 0
    name: DH0
    group: g
    protocol: udp
    address: localhost
    port: 5000
    packet_size: 12
",
            // And on the file.
            "
version: \"1.0\"
description: a misspelled section
payload:
  - dh_id: 0
    name: DH0
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
    fn a_trigger_of_bytes_needs_no_quotes() {
        let with = |trigger: &str| {
            payload(&format!(
                "
version: \"1.0\"
description: a trigger, written one way
payloads:
  - dh_id: 0
    name: DH0
    type: network
    protocol: tcp
    address: localhost
    port: 5000
    packet_size: 12
    mode: triggered
    packet_interval_ms: 500
    trigger: {trigger}
"
            ))
            .to_dh_configs()
            .map(|c| match &c[0].mode {
                DHMode::Triggered { trigger, .. } => trigger.clone(),
                DHMode::Periodic => panic!("this payload is triggered"),
            })
        };

        // Unquoted, which is how a series of bytes is written.
        assert_eq!(with("0x55AA").unwrap(), vec![0x55, 0xAA]);
        assert_eq!(
            with("0x0D0A").unwrap(),
            vec![0x0D, 0x0A],
            "a leading zero survives, which is why the unquoted form is safe"
        );
        assert_eq!(with("0x55 AA 0F").unwrap(), vec![0x55, 0xAA, 0x0F]);

        // Quoted, which the parser cannot tell from the above: the quotes
        // belong to the format and are gone before this sees the value.
        assert_eq!(with("\"0x55AA\"").unwrap(), with("0x55AA").unwrap());

        // And a trigger of text, written with quotes because its escapes are
        // what the quotes are for.
        assert_eq!(with("\"READ\\r\"").unwrap(), b"READ\r".to_vec());
        assert_eq!(with("0x52 45 41 44 0D").unwrap(), with("\"READ\\r\"").unwrap());
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
  - dh_id: 0
    name: DH0
    type: network
    protocol: tcp
    address: localhost
    port: 5000
    packet_size: 12
{extra}
"
            )
        };

        // Periodic, which is what a file states by saying nothing: the
        // interval is refused and told which file it belongs in.
        for stated in [
            "    packet_interval_ms: 250",
            "    mode: periodic\n    packet_interval_ms: 250",
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
            "    mode: triggered\n    trigger: \"READ\\r\"\n    packet_interval_ms: 500",
        ))
        .to_dh_configs()
        .expect("a triggered payload states its trigger's rate here");
        assert_eq!(
            handlers[0].mode,
            DHMode::Triggered {
                trigger: b"READ\r".to_vec(),
                interval_ms: 500,
            }
        );

        let said = payload(&payload_file(
            "    mode: triggered\n    trigger: \"READ\\r\"",
        ))
        .to_dh_configs()
        .expect_err("a triggered payload with no rate for its trigger");
        assert!(said.contains("packet_interval_ms"), "{said}");

        // And the name that rate used to have is told the name it has now.
        let said = payload(&payload_file(
            "    mode: triggered\n    trigger: \"READ\\r\"\n    trigger_interval_ms: 500",
        ))
        .to_dh_configs()
        .expect_err("trigger_interval_ms is not a word this file knows");
        assert!(
            said.contains("trigger_interval_ms") && said.contains("packet_interval_ms now"),
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
    address: localhost
    mode: triggered
    trigger: \"READ\\r\"
    packet_interval_ms: 500
payloads:
  - dh_id: 0
    name: DH0
    group: asked
    port: 5000
    packet_size: 12
";
        let handlers = payload(grouped).to_dh_configs().expect("the group states it");
        assert!(matches!(
            handlers[0].mode,
            DHMode::Triggered { interval_ms: 500, .. }
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
    address: localhost
payloads:
  - dh_id: 0
    name: DH0
    group: udp_localhst
    port: 5000
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
    address: localhost
payloads:
  - dh_id: 0
    name: DH0
    group: udp_localhost
    port: 5000
",
        );

        assert!(config.to_dh_configs().is_err());
    }
}
