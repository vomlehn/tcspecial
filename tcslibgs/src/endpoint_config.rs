//! Endpoint configuration files, in YAML or XML
//!
//! Both formats describe the same thing and parse into the same Rust types:
//! a general section, a section naming groups of endpoints, and a section
//! naming the endpoints themselves. A group carries every attribute shared by
//! endpoints of its type; what it deliberately does not carry is the device
//! name or network address, because that is what distinguishes one endpoint
//! in a group from another and so belongs to the endpoint.
//!
//! The syntax of both formats is specified in `docs/design.rst`, under
//! "Endpoint Configuration Files".
//!
//! One set of wire types serves both formats. XML attributes reach serde with
//! an `@` prefix, so every field carries that spelling as an alias alongside
//! its YAML spelling, and a section arrives either as a YAML sequence or as
//! an XML element with repeated children (see [`Section`]).

use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;
use std::path::Path;
use std::time::Duration;

use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::format::ConfigFormat;
use crate::types::NetworkProtocol;
use crate::TcsError;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// What can be wrong with an endpoint configuration file.
///
/// Parse errors come from the format's own reader; everything else is a rule
/// this module enforces after a syntactically valid file has been read.
#[derive(Error, Debug)]
pub enum EndpointConfigError {
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_norway::Error),

    #[error("XML parse error: {0}")]
    Xml(#[from] quick_xml::DeError),

    #[error("group \"{0}\" is defined more than once")]
    DuplicateGroup(String),

    #[error("endpoint \"{0}\" is defined more than once")]
    DuplicateEndpoint(String),

    #[error("endpoint \"{endpoint}\" refers to group \"{group}\", which is not defined")]
    UnknownGroup { endpoint: String, group: String },

    #[error(
        "group \"{group}\": unknown endpoint type \"{kind}\": expected \"serial\", \
         \"network\", \"i2c\", or \"spi\""
    )]
    UnknownGroupKind { group: String, kind: String },

    #[error("group \"{group}\", a {kind} group, has no {field}")]
    MissingGroupField {
        group: String,
        kind: &'static str,
        field: &'static str,
    },

    #[error("group \"{group}\" is a {kind} group, so {field} does not apply to it")]
    UnusedGroupField {
        group: String,
        kind: &'static str,
        field: &'static str,
    },

    #[error("endpoint \"{endpoint}\" is in {kind} group \"{group}\", so it needs {field}")]
    MissingEndpointField {
        endpoint: String,
        group: String,
        kind: &'static str,
        field: &'static str,
    },

    #[error("endpoint \"{endpoint}\" is in {kind} group \"{group}\", so {field} does not apply to it")]
    UnusedEndpointField {
        endpoint: String,
        group: String,
        kind: &'static str,
        field: &'static str,
    },

    #[error("group \"{0}\": a stream section must give max_length")]
    StreamMissingMaxLength(String),

    #[error(
        "group \"{0}\": a stream section must give a timeout or a terminator list, or both. \
         To read a fixed number of bytes with neither, say timeout: none explicitly"
    )]
    StreamNeedsTimeoutOrTerminators(String),

    #[error("group \"{0}\": an empty terminator list is not a terminator list; omit it or give at least one byte")]
    StreamEmptyTerminators(String),

    #[error("group \"{group}\": {message}")]
    BadGroupValue { group: String, message: String },
}

/// Result of reading an endpoint configuration file.
pub type EndpointConfigResult<T> = Result<T, EndpointConfigError>;

impl From<EndpointConfigError> for TcsError {
    fn from(e: EndpointConfigError) -> Self {
        TcsError::Config(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Domain types -- what a caller gets back
// ---------------------------------------------------------------------------

/// A whole endpoint configuration file, validated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EndpointConfigDoc {
    /// The general section.
    pub general: GeneralSection,
    /// Endpoint groups, in the order the file gave them.
    pub groups: Vec<EndpointGroup>,
    /// Endpoints, in the order the file gave them.
    pub endpoints: Vec<EndpointDef>,
}

impl EndpointConfigDoc {
    /// Look up a group by name.
    pub fn group(&self, name: &str) -> Option<&EndpointGroup> {
        self.groups.iter().find(|g| g.name == name)
    }

    /// The group an endpoint belongs to.
    ///
    /// Never `None` for an endpoint of this document: a reference to an
    /// undefined group is rejected at parse time.
    pub fn group_of(&self, endpoint: &EndpointDef) -> Option<&EndpointGroup> {
        self.group(&endpoint.group)
    }
}

/// The general section: settings that are not specific to any one group.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct GeneralSection {
    /// Version of the configuration file format.
    pub version: Option<String>,
    /// Free text describing what this file configures.
    pub description: Option<String>,
}

/// A named group of endpoints that share every attribute but their device
/// name or network address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EndpointGroup {
    /// Name endpoints use to refer to this group.
    pub name: String,
    /// The attributes, which depend on the type of endpoint.
    pub kind: GroupKind,
}

/// Per-type group attributes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum GroupKind {
    Serial(SerialParams),
    Network(NetworkParams),
    I2c(I2cParams),
    Spi(SpiParams),
}

impl GroupKind {
    /// The name this type goes by in a configuration file.
    pub fn type_name(&self) -> &'static str {
        match self {
            GroupKind::Serial(_) => "serial",
            GroupKind::Network(_) => "network",
            GroupKind::I2c(_) => "i2c",
            GroupKind::Spi(_) => "spi",
        }
    }

    /// The stream payload protocol attributes, if this group has them.
    ///
    /// `None` for the bus types: a master clocks exactly as many bytes as it
    /// asks for, so a transfer is already bounded and needs no rule for where
    /// a read ends.
    pub fn stream(&self) -> Option<&StreamParams> {
        match self {
            GroupKind::Serial(s) => Some(&s.stream),
            GroupKind::Network(n) => n.stream.as_ref(),
            GroupKind::I2c(_) | GroupKind::Spi(_) => None,
        }
    }
}

/// Attributes of a serial port group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SerialParams {
    /// Line rate in bits per second.
    pub datarate: u32,
    /// Stop bits following each byte.
    pub stop_bits: StopBits,
    /// Data bits in each byte.
    pub byte_length: ByteLength,
    /// Stream payload protocol attributes. Required: a serial port is a
    /// stream, so there is always a rule for where one read ends.
    pub stream: StreamParams,
}

/// Attributes of a network group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NetworkParams {
    /// Transport carrying the payload data.
    pub protocol: NetworkProtocol,
    /// Stream payload protocol attributes. Present for the stream protocols
    /// and absent for the datagram protocols, where a datagram is the frame.
    pub stream: Option<StreamParams>,
}

/// Attributes of an I2C group.
///
/// There is no byte length, parity, or stop bits here, because the protocol
/// fixes the framing of a byte: eight data bits, most significant first,
/// followed by an acknowledge bit. What is left to configure is how the
/// master addresses a device and how hard it tries to complete a transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct I2cParams {
    /// Address devices with ten address bits rather than seven.
    pub ten_bit: bool,
    /// Append an SMBus packet error check, a CRC-8, to each transfer.
    pub pec: bool,
    /// Times a transfer is retried after a lost arbitration or an
    /// unexpected NAK.
    pub retries: u32,
    /// How long a transfer waits before it fails, rounded up to the 10 ms
    /// the bus driver keeps it in.
    pub timeout: Option<Duration>,
    /// Bus clock rate in Hz, as the file recorded it.
    ///
    /// Recorded, never applied: the rate belongs to the bus controller, which
    /// the platform configures from the device tree or from ACPI, and a
    /// program holding an endpoint open cannot change it. Keeping it lets the
    /// rate a group expects be checked against the platform and reported
    /// plainly, rather than inferred later from corrupted transfers.
    pub bus_speed: Option<u32>,
}

impl I2cParams {
    /// The bus driver keeps a timeout in units of 10 ms, so a timeout that is
    /// not a whole number of them takes the next one up.
    pub fn effective_timeout(&self) -> Option<Duration> {
        self.timeout.map(|t| {
            let units = t.as_millis().div_ceil(10).max(1);
            Duration::from_millis(units as u64 * 10)
        })
    }
}

/// Attributes of a SPI group.
///
/// As with I2C there is no parity and there are no stop bits: SPI is clocked
/// and full duplex, and a transfer is delimited by the chip select rather
/// than by framing bits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpiParams {
    /// Greatest clock rate the peripheral accepts, in Hz.
    ///
    /// An upper bound, not an exact rate: a controller divides its own clock
    /// down and runs at the fastest rate it can produce that does not exceed
    /// this one.
    pub max_speed: u32,
    /// Clock polarity and phase.
    pub mode: SpiMode,
    /// Bits in each word.
    pub bits_per_word: BitsPerWord,
    /// Which bit of a word goes first.
    pub bit_order: BitOrder,
    /// The level at which the chip select is asserted.
    pub cs_active: CsActive,
}

/// Clock polarity and phase, the four combinations of which are numbered 0
/// to 3 by every SPI peripheral's datasheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SpiMode {
    Mode0,
    Mode1,
    Mode2,
    Mode3,
}

impl SpiMode {
    /// The mode number, 0 to 3.
    pub fn number(&self) -> u8 {
        match self {
            SpiMode::Mode0 => 0,
            SpiMode::Mode1 => 1,
            SpiMode::Mode2 => 2,
            SpiMode::Mode3 => 3,
        }
    }

    /// Clock polarity: the idle level of the clock.
    pub fn cpol(&self) -> bool {
        self.number() & 2 != 0
    }

    /// Clock phase: false samples on the leading edge, true on the trailing.
    pub fn cpha(&self) -> bool {
        self.number() & 1 != 0
    }
}

impl fmt::Display for SpiMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.number())
    }
}

/// Which bit of a word is sent first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BitOrder {
    MsbFirst,
    LsbFirst,
}

/// The level at which a chip select is asserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CsActive {
    Low,
    High,
}

/// Bits in each word on a SPI bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BitsPerWord(u8);

impl BitsPerWord {
    /// Narrowest word a controller offers.
    pub const MIN: u8 = 1;
    /// Widest word a controller offers.
    pub const MAX: u8 = 32;
    /// The width nearly every peripheral uses.
    pub const DEFAULT: u8 = 8;

    /// Build a word width, rejecting one no controller can produce.
    pub fn new(bits: u8) -> Result<Self, String> {
        if (Self::MIN..=Self::MAX).contains(&bits) {
            Ok(BitsPerWord(bits))
        } else {
            Err(format!(
                "bits_per_word {bits} is out of range: expected {} to {}",
                Self::MIN,
                Self::MAX
            ))
        }
    }

    /// The number of bits.
    pub fn bits(&self) -> u8 {
        self.0
    }
}

impl Default for BitsPerWord {
    fn default() -> Self {
        BitsPerWord(Self::DEFAULT)
    }
}

/// Stop bits following each byte on a serial line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum StopBits {
    One,
    OnePointFive,
    Two,
}

impl StopBits {
    /// How this value is spelled in a configuration file.
    pub fn as_str(&self) -> &'static str {
        match self {
            StopBits::One => "1",
            StopBits::OnePointFive => "1.5",
            StopBits::Two => "2",
        }
    }
}

impl fmt::Display for StopBits {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Data bits in each byte on a serial line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ByteLength(u8);

impl ByteLength {
    /// Smallest byte length a UART offers.
    pub const MIN: u8 = 5;
    /// Largest byte length a UART offers.
    pub const MAX: u8 = 8;

    /// Build a byte length, rejecting one no UART can produce.
    pub fn new(bits: u8) -> Result<Self, String> {
        if (Self::MIN..=Self::MAX).contains(&bits) {
            Ok(ByteLength(bits))
        } else {
            Err(format!(
                "byte_length {bits} is out of range: expected {} to {}",
                Self::MIN,
                Self::MAX
            ))
        }
    }

    /// The number of bits.
    pub fn bits(&self) -> u8 {
        self.0
    }
}

/// Where one read of a stream payload protocol ends.
///
/// `max_length` always bounds a read. A read also ends on whichever of the
/// other two conditions the file supplied, and at least one of them was
/// supplied: a file giving neither is rejected, because it would describe a
/// read with no way to end but filling the buffer -- which is a real intent,
/// but one a file must state by giving `timeout: none`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StreamParams {
    /// Longest payload one read may deliver, in bytes. Always present.
    pub max_length: u32,
    /// How long a read waits, or `None` for a read that does not time out.
    pub timeout: Option<Duration>,
    /// Bytes that each end a read. Empty when the file gave no terminators.
    pub terminators: Vec<u8>,
}

impl StreamParams {
    /// Whether a read of this stream ends only on filling `max_length`.
    ///
    /// True exactly for the file that said `timeout: none` and gave no
    /// terminators -- the "hand me this many bytes at a time" case.
    pub fn is_fixed_length(&self) -> bool {
        self.timeout.is_none() && self.terminators.is_empty()
    }
}

/// One endpoint: a member of a group, plus the one thing the group left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EndpointDef {
    /// Name of this endpoint.
    pub name: String,
    /// Name of the group supplying its attributes.
    pub group: String,
    /// Where this endpoint is.
    pub location: EndpointLocation,
}

/// What locates one endpoint of a group, and so distinguishes it from the
/// others: a device name, a network address, or, on a bus that addresses its
/// devices, the address of the device on that bus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum EndpointLocation {
    /// A path in the filesystem: a serial port, a SPI device node, or a
    /// Unix-domain socket.
    ///
    /// A SPI device node names the bus and the chip select together, so it
    /// needs no separate address.
    Device { path: String },
    /// A network host and port.
    Network { address: String, port: u16 },
    /// A bus device and the address of one device on that bus.
    ///
    /// Both are needed because two endpoints of one I2C group commonly sit
    /// on the same bus and differ only in which device the master addresses.
    I2c { bus: String, address: u16 },
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Read an endpoint configuration from text in the given format.
pub fn from_str(text: &str, format: ConfigFormat) -> EndpointConfigResult<EndpointConfigDoc> {
    let wire: DocWire = match format {
        ConfigFormat::Json => serde_json::from_str(text)?,
        ConfigFormat::Yaml => serde_norway::from_str(text)?,
        ConfigFormat::Xml => quick_xml::de::from_str(text)?,
    };
    validate(wire)
}

/// Read an endpoint configuration from YAML text.
pub fn from_yaml_str(text: &str) -> EndpointConfigResult<EndpointConfigDoc> {
    from_str(text, ConfigFormat::Yaml)
}

/// Read an endpoint configuration from XML text.
pub fn from_xml_str(text: &str) -> EndpointConfigResult<EndpointConfigDoc> {
    from_str(text, ConfigFormat::Xml)
}

/// Read an endpoint configuration file, choosing the format from the file
/// name the same way every other configuration file in the project does --
/// see [`ConfigFormat::from_path`].
pub fn load<P: AsRef<Path>>(path: P) -> EndpointConfigResult<EndpointConfigDoc> {
    let path = path.as_ref();
    let text = read_to_string(path)?;
    from_str(&text, ConfigFormat::from_path(path))
}

fn read_to_string(path: &Path) -> EndpointConfigResult<String> {
    std::fs::read_to_string(path).map_err(|source| EndpointConfigError::Io {
        path: path.display().to_string(),
        source,
    })
}

// ---------------------------------------------------------------------------
// Wire types -- one set, both formats
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct DocWire {
    #[serde(default, alias = "general_configuration")]
    general: GeneralWire,
    #[serde(default, alias = "groups", alias = "endpoint-groups")]
    endpoint_groups: Section<GroupWire>,
    #[serde(default)]
    endpoints: Section<EndpointWire>,
}

#[derive(Debug, Default, Deserialize)]
struct GeneralWire {
    #[serde(default, alias = "@version")]
    version: Option<String>,
    #[serde(default, alias = "@description")]
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GroupWire {
    #[serde(alias = "@name")]
    name: String,
    #[serde(rename = "type", alias = "@type")]
    kind: String,
    // Serial attributes.
    #[serde(default, alias = "@datarate")]
    datarate: Option<Scalar>,
    #[serde(default, alias = "@stop_bits", alias = "stop-bits", alias = "@stop-bits")]
    stop_bits: Option<Scalar>,
    #[serde(
        default,
        alias = "@byte_length",
        alias = "byte-length",
        alias = "@byte-length"
    )]
    byte_length: Option<Scalar>,
    // Network attributes.
    #[serde(default, alias = "@protocol")]
    protocol: Option<String>,
    // I2C attributes.
    #[serde(default, alias = "@ten_bit", alias = "ten-bit", alias = "@ten-bit")]
    ten_bit: Option<Scalar>,
    #[serde(default, alias = "@pec")]
    pec: Option<Scalar>,
    #[serde(default, alias = "@retries")]
    retries: Option<Scalar>,
    #[serde(default, alias = "@timeout")]
    timeout: Option<Scalar>,
    #[serde(
        default,
        alias = "@bus_speed",
        alias = "bus-speed",
        alias = "@bus-speed"
    )]
    bus_speed: Option<Scalar>,
    // SPI attributes.
    #[serde(
        default,
        alias = "@max_speed",
        alias = "max-speed",
        alias = "@max-speed"
    )]
    max_speed: Option<Scalar>,
    #[serde(default, alias = "@mode")]
    mode: Option<Scalar>,
    #[serde(
        default,
        alias = "@bits_per_word",
        alias = "bits-per-word",
        alias = "@bits-per-word"
    )]
    bits_per_word: Option<Scalar>,
    #[serde(
        default,
        alias = "@bit_order",
        alias = "bit-order",
        alias = "@bit-order"
    )]
    bit_order: Option<Scalar>,
    #[serde(
        default,
        alias = "@cs_active",
        alias = "cs-active",
        alias = "@cs-active"
    )]
    cs_active: Option<Scalar>,
    // Shared: stream payload protocol attributes.
    #[serde(default)]
    stream: Option<StreamWire>,
}

#[derive(Debug, Deserialize)]
struct StreamWire {
    #[serde(
        default,
        alias = "@max_length",
        alias = "max-length",
        alias = "@max-length"
    )]
    max_length: Option<Scalar>,
    #[serde(default, alias = "@timeout")]
    timeout: Option<Scalar>,
    #[serde(
        default,
        alias = "@terminators",
        alias = "terminator",
        alias = "@terminator"
    )]
    terminators: Option<ByteList>,
}

#[derive(Debug, Deserialize)]
struct EndpointWire {
    #[serde(alias = "@name")]
    name: String,
    #[serde(alias = "@group")]
    group: String,
    #[serde(default, alias = "@device", alias = "path", alias = "@path")]
    device: Option<String>,
    #[serde(default, alias = "@address")]
    address: Option<String>,
    #[serde(default, alias = "@port")]
    port: Option<Scalar>,
}

// ---------------------------------------------------------------------------
// Shapes that differ between the two formats
// ---------------------------------------------------------------------------

/// A document section holding a list of items.
///
/// YAML gives a section as a sequence; XML gives it as an element whose
/// children repeat. This accepts either, so one set of wire types serves
/// both formats.
#[derive(Debug)]
struct Section<T>(Vec<T>);

impl<T> Default for Section<T> {
    fn default() -> Self {
        Section(Vec::new())
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Section<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V<T>(PhantomData<T>);

        impl<'de, T: Deserialize<'de>> Visitor<'de> for V<T> {
            type Value = Section<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a sequence of entries, or an element with repeated children")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut out = Vec::new();
                while let Some(v) = a.next_element()? {
                    out.push(v);
                }
                Ok(Section(out))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut out = Vec::new();
                while let Some(key) = a.next_key::<String>()? {
                    // One child element arrives as a single value, several of
                    // the same name as a sequence.
                    match a.next_value::<OneOrMany<T>>() {
                        Ok(OneOrMany::One(v)) => out.push(v),
                        Ok(OneOrMany::Many(vs)) => out.extend(vs),
                        Err(e) => {
                            return Err(de::Error::custom(format!("in <{key}>: {e}")));
                        }
                    }
                }
                Ok(Section(out))
            }
        }

        d.deserialize_any(V(PhantomData))
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OneOrMany<T> {
    Many(Vec<T>),
    One(T),
}

/// A scalar as the file spelled it.
///
/// Every value in an XML attribute is text, and YAML distinguishes numbers
/// from strings, so a field that is conceptually a number arrives as either.
/// Both are kept as text and parsed by the rule for that field, which gives
/// one spelling of each rule and one wording of each error.
#[derive(Debug, Clone)]
struct Scalar(String);

impl Scalar {
    fn as_str(&self) -> &str {
        self.0.trim()
    }
}

impl<'de> Deserialize<'de> for Scalar {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = Scalar;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a number or a string")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Scalar, E> {
                Ok(Scalar(v.to_string()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Scalar, E> {
                Ok(Scalar(v.to_string()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Scalar, E> {
                Ok(Scalar(v.to_string()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Scalar, E> {
                Ok(Scalar(v.to_string()))
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Scalar, E> {
                // Reached only by a YAML 1.1 reader folding yes/no/on/off to
                // a boolean. Put it back as written so the field's own parser
                // reports it.
                Ok(Scalar(v.to_string()))
            }
        }

        d.deserialize_any(V)
    }
}

/// A list of byte values, written as a sequence or as one delimited string.
///
/// XML has no sequences in an attribute, so `terminators="0x0D,0x0A"` is the
/// form there; YAML may use either that or `[0x0D, 0x0A]`. Repeated
/// `<terminator>` children work too, by way of [`Section`]-like flattening
/// in the untagged enum below.
#[derive(Debug, Clone)]
struct ByteList(Vec<String>);

impl<'de> Deserialize<'de> for ByteList {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = ByteList;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a list of byte values, or a comma- or space-separated string of them")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<ByteList, E> {
                Ok(ByteList(split_list(v)))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<ByteList, E> {
                Ok(ByteList(vec![v.to_string()]))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<ByteList, A::Error> {
                let mut out = Vec::new();
                while let Some(s) = a.next_element::<Scalar>()? {
                    out.push(s.as_str().to_string());
                }
                Ok(ByteList(out))
            }
        }

        d.deserialize_any(V)
    }
}

fn split_list(text: &str) -> Vec<String> {
    text.split([',', ' ', '\t', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

// ---------------------------------------------------------------------------
// Validation -- wire types to domain types
// ---------------------------------------------------------------------------

fn validate(doc: DocWire) -> EndpointConfigResult<EndpointConfigDoc> {
    let general = GeneralSection {
        version: doc.general.version,
        description: doc.general.description,
    };

    let mut groups: Vec<EndpointGroup> = Vec::with_capacity(doc.endpoint_groups.0.len());
    let mut by_name: BTreeMap<String, usize> = BTreeMap::new();

    for g in doc.endpoint_groups.0 {
        if by_name.contains_key(&g.name) {
            return Err(EndpointConfigError::DuplicateGroup(g.name));
        }
        let group = validate_group(g)?;
        by_name.insert(group.name.clone(), groups.len());
        groups.push(group);
    }

    let mut endpoints: Vec<EndpointDef> = Vec::with_capacity(doc.endpoints.0.len());
    let mut seen: BTreeMap<String, ()> = BTreeMap::new();

    for e in doc.endpoints.0 {
        if seen.contains_key(&e.name) {
            return Err(EndpointConfigError::DuplicateEndpoint(e.name));
        }
        let idx = *by_name
            .get(&e.group)
            .ok_or_else(|| EndpointConfigError::UnknownGroup {
                endpoint: e.name.clone(),
                group: e.group.clone(),
            })?;
        seen.insert(e.name.clone(), ());
        endpoints.push(validate_endpoint(e, &groups[idx])?);
    }

    Ok(EndpointConfigDoc {
        general,
        groups,
        endpoints,
    })
}

fn validate_group(g: GroupWire) -> EndpointConfigResult<EndpointGroup> {
    // Cloned rather than moved out: the checks below read the whole wire
    // group, to reject any attribute belonging to another type.
    let name = g.name.clone();
    let kind_text = g.kind.trim().to_ascii_lowercase();

    let kind = match kind_text.as_str() {
        "serial" => {
            reject_foreign_fields(
                &name,
                "serial",
                &g,
                &["datarate", "stop_bits", "byte_length"],
            )?;

            let datarate = require(&name, "serial", "datarate", g.datarate.as_ref())?;
            let datarate = parse_u32(&name, "datarate", datarate)?;
            if datarate == 0 {
                return Err(bad(&name, "datarate must be greater than zero"));
            }

            let stop_bits = require(&name, "serial", "stop_bits", g.stop_bits.as_ref())?;
            let stop_bits = parse_stop_bits(&name, stop_bits)?;

            let byte_length = require(&name, "serial", "byte_length", g.byte_length.as_ref())?;
            let byte_length = parse_u32(&name, "byte_length", byte_length)?;
            let byte_length = ByteLength::new(byte_length.min(u8::MAX as u32) as u8)
                .map_err(|m| bad(&name, &m))?;

            // A serial port is a stream, so a read needs an end.
            let stream = g.stream.ok_or(EndpointConfigError::MissingGroupField {
                group: name.clone(),
                kind: "serial",
                field: "a stream section",
            })?;
            let stream = validate_stream(&name, stream)?;

            GroupKind::Serial(SerialParams {
                datarate,
                stop_bits,
                byte_length,
                stream,
            })
        }

        "network" => {
            reject_foreign_fields(&name, "network", &g, &["protocol"])?;

            let protocol = require(&name, "network", "protocol", g.protocol.as_ref())?;
            let protocol = parse_protocol(&name, protocol)?;

            // The stream protocols need a rule for where a read ends; for the
            // datagram protocols the datagram is already the frame.
            let stream = match (is_stream_protocol(protocol), g.stream) {
                (true, Some(s)) => Some(validate_stream(&name, s)?),
                (true, None) => {
                    return Err(EndpointConfigError::MissingGroupField {
                        group: name,
                        kind: "network",
                        field: "a stream section",
                    })
                }
                (false, Some(_)) => {
                    return Err(EndpointConfigError::UnusedGroupField {
                        group: name,
                        kind: "datagram network",
                        field: "a stream section",
                    })
                }
                (false, None) => None,
            };

            GroupKind::Network(NetworkParams { protocol, stream })
        }

        "i2c" => {
            reject_foreign_fields(
                &name,
                "i2c",
                &g,
                &["ten_bit", "pec", "retries", "timeout", "bus_speed"],
            )?;
            reject_stream(&name, "i2c", g.stream.is_some())?;

            let ten_bit = parse_flag(&name, "ten_bit", g.ten_bit.as_ref())?.unwrap_or(false);
            let pec = parse_flag(&name, "pec", g.pec.as_ref())?.unwrap_or(false);

            let retries = match g.retries.as_ref() {
                Some(s) => parse_u32(&name, "retries", s)?,
                None => 0,
            };

            let timeout = match g.timeout.as_ref() {
                Some(s) => parse_timeout(&name, s)?,
                None => None,
            };

            // Recorded, not applied: see I2cParams::bus_speed. A rate of zero
            // is still rejected, because a file that records one is making a
            // claim about the platform and zero is not a claim.
            let bus_speed = match g.bus_speed.as_ref() {
                Some(s) => {
                    let hz = parse_u32(&name, "bus_speed", s)?;
                    if hz == 0 {
                        return Err(bad(&name, "bus_speed must be greater than zero"));
                    }
                    Some(hz)
                }
                None => None,
            };

            GroupKind::I2c(I2cParams {
                ten_bit,
                pec,
                retries,
                timeout,
                bus_speed,
            })
        }

        "spi" => {
            reject_foreign_fields(
                &name,
                "spi",
                &g,
                &["max_speed", "mode", "bits_per_word", "bit_order", "cs_active"],
            )?;
            reject_stream(&name, "spi", g.stream.is_some())?;

            let max_speed = require(&name, "spi", "max_speed", g.max_speed.as_ref())?;
            let max_speed = parse_u32(&name, "max_speed", max_speed)?;
            if max_speed == 0 {
                return Err(bad(&name, "max_speed must be greater than zero"));
            }

            // Required: no controller default is right for every peripheral,
            // and a mismatched mode fails the way a mismatched data rate
            // fails on a serial line.
            let mode = require(&name, "spi", "mode", g.mode.as_ref())?;
            let mode = parse_spi_mode(&name, mode)?;

            let bits_per_word = match g.bits_per_word.as_ref() {
                Some(s) => {
                    let bits = parse_u32(&name, "bits_per_word", s)?;
                    BitsPerWord::new(bits.min(u8::MAX as u32) as u8).map_err(|m| bad(&name, &m))?
                }
                None => BitsPerWord::default(),
            };

            let bit_order = match g.bit_order.as_ref() {
                Some(s) => parse_bit_order(&name, s)?,
                None => BitOrder::MsbFirst,
            };

            let cs_active = match g.cs_active.as_ref() {
                Some(s) => parse_cs_active(&name, s)?,
                None => CsActive::Low,
            };

            GroupKind::Spi(SpiParams {
                max_speed,
                mode,
                bits_per_word,
                bit_order,
                cs_active,
            })
        }

        other => {
            return Err(EndpointConfigError::UnknownGroupKind {
                group: name,
                kind: other.to_string(),
            })
        }
    };

    Ok(EndpointGroup { name, kind })
}

/// Apply the three rules governing a stream section.
fn validate_stream(group: &str, s: StreamWire) -> EndpointConfigResult<StreamParams> {
    // 1. max_length is required.
    let max_length = s
        .max_length
        .as_ref()
        .ok_or_else(|| EndpointConfigError::StreamMissingMaxLength(group.to_string()))?;
    let max_length = parse_u32(group, "max_length", max_length)?;
    if max_length == 0 {
        return Err(bad(group, "max_length must be greater than zero"));
    }

    // 2. A timeout is either a duration or the word "none", which says
    //    explicitly that a read does not time out.
    let timeout = match s.timeout.as_ref() {
        Some(t) => Some(parse_timeout(group, t)?),
        None => None,
    };

    // 3. Terminators, if given, must actually name a byte.
    let terminators = match s.terminators.as_ref() {
        Some(list) => {
            if list.0.is_empty() {
                return Err(EndpointConfigError::StreamEmptyTerminators(
                    group.to_string(),
                ));
            }
            let mut out = Vec::with_capacity(list.0.len());
            for item in &list.0 {
                out.push(parse_byte(group, item)?);
            }
            Some(out)
        }
        None => None,
    };

    // At least one of the two end conditions must have been supplied. Note
    // that `timeout: none` counts: it is how a file asks for a read that ends
    // only on max_length, and saying so is what distinguishes that intent
    // from having forgotten to say anything.
    if timeout.is_none() && terminators.is_none() {
        return Err(EndpointConfigError::StreamNeedsTimeoutOrTerminators(
            group.to_string(),
        ));
    }

    Ok(StreamParams {
        max_length,
        // Flattened: an omitted timeout and an explicit "none" both mean a
        // read that does not time out. They differ only in whether they
        // satisfy the rule above, which has already been checked.
        timeout: timeout.flatten(),
        terminators: terminators.unwrap_or_default(),
    })
}

/// The shape of the thing that locates an endpoint, which follows from the
/// type of its group.
enum LocationShape {
    /// A device name alone: a serial port, a SPI device node, or a
    /// Unix-domain socket.
    Device,
    /// A host and a port.
    Network,
    /// A bus device and the address of a device on that bus.
    I2cBusAndAddress,
}

fn validate_endpoint(e: EndpointWire, group: &EndpointGroup) -> EndpointConfigResult<EndpointDef> {
    let kind = group.kind.type_name();

    let shape = match &group.kind {
        GroupKind::Serial(_) => LocationShape::Device,
        // A SPI device node names the bus and the chip select together.
        GroupKind::Spi(_) => LocationShape::Device,
        GroupKind::I2c(_) => LocationShape::I2cBusAndAddress,
        // A Unix-domain socket is named by a path, not by host and port.
        GroupKind::Network(n) => {
            if matches!(
                n.protocol,
                NetworkProtocol::UnixStream | NetworkProtocol::UnixDgram
            ) {
                LocationShape::Device
            } else {
                LocationShape::Network
            }
        }
    };

    if let LocationShape::I2cBusAndAddress = shape {
        // An I2C endpoint gives both, and a port is no part of a bus.
        if e.port.is_some() {
            return Err(EndpointConfigError::UnusedEndpointField {
                endpoint: e.name,
                group: group.name.clone(),
                kind,
                field: "a port",
            });
        }

        let bus = e
            .device
            .ok_or_else(|| EndpointConfigError::MissingEndpointField {
                endpoint: e.name.clone(),
                group: group.name.clone(),
                kind,
                field: "a bus device",
            })?;

        let address = e
            .address
            .ok_or_else(|| EndpointConfigError::MissingEndpointField {
                endpoint: e.name.clone(),
                group: group.name.clone(),
                kind,
                field: "a slave address",
            })?;

        let ten_bit = match &group.kind {
            GroupKind::I2c(p) => p.ten_bit,
            _ => unreachable!("shape follows from the group kind"),
        };
        let address = parse_i2c_address(&group.name, &address, ten_bit)?;

        return Ok(EndpointDef {
            name: e.name,
            group: e.group,
            location: EndpointLocation::I2c { bus, address },
        });
    }

    let wants_network = matches!(shape, LocationShape::Network);

    let location = if wants_network {
        for (field, present) in [("device", e.device.is_some())] {
            if present {
                return Err(EndpointConfigError::UnusedEndpointField {
                    endpoint: e.name,
                    group: group.name.clone(),
                    kind,
                    field,
                });
            }
        }
        let address = e
            .address
            .ok_or_else(|| EndpointConfigError::MissingEndpointField {
                endpoint: e.name.clone(),
                group: group.name.clone(),
                kind,
                field: "an address",
            })?;
        let port = e
            .port
            .as_ref()
            .ok_or_else(|| EndpointConfigError::MissingEndpointField {
                endpoint: e.name.clone(),
                group: group.name.clone(),
                kind,
                field: "a port",
            })?;
        let port = parse_u32(&group.name, "port", port)?;
        if port > u16::MAX as u32 {
            return Err(bad(&group.name, &format!("port {port} is out of range")));
        }
        EndpointLocation::Network {
            address,
            port: port as u16,
        }
    } else {
        for (field, present) in [("an address", e.address.is_some()), ("a port", e.port.is_some())] {
            if present {
                return Err(EndpointConfigError::UnusedEndpointField {
                    endpoint: e.name,
                    group: group.name.clone(),
                    kind,
                    field,
                });
            }
        }
        let path = e
            .device
            .ok_or_else(|| EndpointConfigError::MissingEndpointField {
                endpoint: e.name.clone(),
                group: group.name.clone(),
                kind,
                field: "a device",
            })?;
        EndpointLocation::Device { path }
    };

    Ok(EndpointDef {
        name: e.name,
        group: e.group,
        location,
    })
}

// ---------------------------------------------------------------------------
// Scalar parsers
// ---------------------------------------------------------------------------

fn bad(group: &str, message: &str) -> EndpointConfigError {
    EndpointConfigError::BadGroupValue {
        group: group.to_string(),
        message: message.to_string(),
    }
}

fn require<'a, T>(
    group: &str,
    kind: &'static str,
    field: &'static str,
    value: Option<&'a T>,
) -> EndpointConfigResult<&'a T> {
    value.ok_or(EndpointConfigError::MissingGroupField {
        group: group.to_string(),
        kind,
        field,
    })
}

fn reject_unused(
    group: &str,
    kind: &'static str,
    field: &'static str,
    present: bool,
) -> EndpointConfigResult<()> {
    if present {
        Err(EndpointConfigError::UnusedGroupField {
            group: group.to_string(),
            kind,
            field,
        })
    } else {
        Ok(())
    }
}

/// Every type-specific group attribute, paired with whether the file gave it.
///
/// Listed in one place so that adding an attribute to one type cannot quietly
/// make it accepted by the others.
fn type_specific_fields(g: &GroupWire) -> [(&'static str, bool); 14] {
    [
        // Serial.
        ("datarate", g.datarate.is_some()),
        ("stop_bits", g.stop_bits.is_some()),
        ("byte_length", g.byte_length.is_some()),
        // Network.
        ("protocol", g.protocol.is_some()),
        // I2C.
        ("ten_bit", g.ten_bit.is_some()),
        ("pec", g.pec.is_some()),
        ("retries", g.retries.is_some()),
        ("timeout", g.timeout.is_some()),
        ("bus_speed", g.bus_speed.is_some()),
        // SPI.
        ("max_speed", g.max_speed.is_some()),
        ("mode", g.mode.is_some()),
        ("bits_per_word", g.bits_per_word.is_some()),
        ("bit_order", g.bit_order.is_some()),
        ("cs_active", g.cs_active.is_some()),
    ]
}

/// Reject any attribute that belongs to some other type of group.
///
/// An attribute that does not apply is an error rather than being ignored, so
/// that a misspelled or misplaced attribute is reported where it was written.
fn reject_foreign_fields(
    group: &str,
    kind: &'static str,
    g: &GroupWire,
    own: &[&str],
) -> EndpointConfigResult<()> {
    for (field, present) in type_specific_fields(g) {
        if present && !own.contains(&field) {
            return reject_unused(group, kind, field, true);
        }
    }
    Ok(())
}

/// Reject a stream section on a bus type, where a transfer is already bounded
/// by the number of bytes the master clocks.
fn reject_stream(group: &str, kind: &'static str, present: bool) -> EndpointConfigResult<()> {
    reject_unused(group, kind, "a stream section", present)
}

/// Parse an unsigned value written in decimal, or in hex with an `0x` prefix.
fn parse_u32(group: &str, field: &str, s: &Scalar) -> EndpointConfigResult<u32> {
    let text = s.as_str();
    let parsed = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => text.parse::<u32>(),
    };
    parsed.map_err(|_| bad(group, &format!("{field}: \"{text}\" is not a whole number")))
}

/// Parse a byte value: decimal, or hex with an `0x` prefix.
fn parse_byte(group: &str, text: &str) -> EndpointConfigResult<u8> {
    let text = text.trim();
    let parsed = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u16::from_str_radix(hex, 16),
        None => text.parse::<u16>(),
    };
    match parsed {
        Ok(v) if v <= u8::MAX as u16 => Ok(v as u8),
        Ok(v) => Err(bad(
            group,
            &format!("terminator {v} is not a byte value: expected 0 to 255"),
        )),
        Err(_) => Err(bad(
            group,
            &format!("terminator \"{text}\" is not a byte value"),
        )),
    }
}

fn parse_stop_bits(group: &str, s: &Scalar) -> EndpointConfigResult<StopBits> {
    match s.as_str() {
        "1" | "1.0" => Ok(StopBits::One),
        "1.5" => Ok(StopBits::OnePointFive),
        "2" | "2.0" => Ok(StopBits::Two),
        other => Err(bad(
            group,
            &format!("stop_bits: \"{other}\" is not one of 1, 1.5, or 2"),
        )),
    }
}

/// Parse a flag written `true` or `false`.
///
/// A YAML 1.1 reader folds `yes`, `no`, `on`, and `off` to a boolean before
/// this sees them, and [`Scalar`] puts the result back as `true` or `false`,
/// so those spellings work too without being named here.
fn parse_flag(group: &str, field: &str, s: Option<&Scalar>) -> EndpointConfigResult<Option<bool>> {
    let Some(s) = s else { return Ok(None) };
    match s.as_str().to_ascii_lowercase().as_str() {
        "true" => Ok(Some(true)),
        "false" => Ok(Some(false)),
        other => Err(bad(
            group,
            &format!("{field}: \"{other}\" is not true or false"),
        )),
    }
}

fn parse_spi_mode(group: &str, s: &Scalar) -> EndpointConfigResult<SpiMode> {
    match s.as_str() {
        "0" => Ok(SpiMode::Mode0),
        "1" => Ok(SpiMode::Mode1),
        "2" => Ok(SpiMode::Mode2),
        "3" => Ok(SpiMode::Mode3),
        other => Err(bad(
            group,
            &format!("mode: \"{other}\" is not one of 0, 1, 2, or 3"),
        )),
    }
}

fn parse_bit_order(group: &str, s: &Scalar) -> EndpointConfigResult<BitOrder> {
    match s.as_str().to_ascii_lowercase().as_str() {
        "msb" | "msb_first" | "msb-first" => Ok(BitOrder::MsbFirst),
        "lsb" | "lsb_first" | "lsb-first" => Ok(BitOrder::LsbFirst),
        other => Err(bad(
            group,
            &format!("bit_order: \"{other}\" is not msb or lsb"),
        )),
    }
}

fn parse_cs_active(group: &str, s: &Scalar) -> EndpointConfigResult<CsActive> {
    match s.as_str().to_ascii_lowercase().as_str() {
        "low" => Ok(CsActive::Low),
        "high" => Ok(CsActive::High),
        other => Err(bad(
            group,
            &format!("cs_active: \"{other}\" is not low or high"),
        )),
    }
}

/// Addresses the I2C specification keeps for itself, and so which cannot
/// name a device.
///
/// `0x00` to `0x07` carry the general call, the CBUS address, and the
/// high-speed master code; `0x78` to `0x7F` carry the 10-bit addressing
/// prefix and the block the specification reserves for future use. What is
/// left, `0x08` to `0x77`, is the 112 addresses a 7-bit bus really offers.
///
/// These apply to 7-bit addressing only. A 10-bit transfer is introduced by
/// the `0x78` prefix and then carries its address in the bytes that follow,
/// so the whole 10-bit space is available.
const I2C_RESERVED_LOW: std::ops::RangeInclusive<u32> = 0x00..=0x07;
const I2C_RESERVED_HIGH: std::ops::RangeInclusive<u32> = 0x78..=0x7F;

/// Parse an I2C slave address, in decimal or in hex with an `0x` prefix.
///
/// How wide an address may be follows from the group's addressing mode, so
/// a file that writes a 10-bit address in a 7-bit group is told which of the
/// two it got wrong.
fn parse_i2c_address(group: &str, text: &str, ten_bit: bool) -> EndpointConfigResult<u16> {
    let text = text.trim();
    let parsed = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => text.parse::<u32>(),
    };
    let value =
        parsed.map_err(|_| bad(group, &format!("address: \"{text}\" is not a whole number")))?;

    let (limit, width) = if ten_bit { (0x3FF, 10) } else { (0x7F, 7) };
    if value > limit {
        // Worth suggesting the wider mode only to a group that is not
        // already in it.
        let hint = if ten_bit {
            ""
        } else {
            ". A group addressing its devices with ten bits says ten_bit: true"
        };
        return Err(bad(
            group,
            &format!("address {text} does not fit in {width} bits: expected 0 to {limit:#X}{hint}"),
        ));
    }

    // A reserved address is in range and still cannot name a device, so it is
    // worth catching here rather than as a silent failure to answer on a bus.
    if !ten_bit && (I2C_RESERVED_LOW.contains(&value) || I2C_RESERVED_HIGH.contains(&value)) {
        return Err(bad(
            group,
            &format!(
                "address {text} is reserved by the I2C specification and cannot name a \
                 device: {:#04X} to {:#04X} and {:#04X} to {:#04X} are reserved, leaving \
                 {:#04X} to {:#04X} for devices",
                I2C_RESERVED_LOW.start(),
                I2C_RESERVED_LOW.end(),
                I2C_RESERVED_HIGH.start(),
                I2C_RESERVED_HIGH.end(),
                I2C_RESERVED_LOW.end() + 1,
                I2C_RESERVED_HIGH.start() - 1,
            ),
        ));
    }

    Ok(value as u16)
}

fn parse_protocol(group: &str, text: &str) -> EndpointConfigResult<NetworkProtocol> {
    match text.trim().to_ascii_lowercase().as_str() {
        "tcp" => Ok(NetworkProtocol::Tcp),
        "udp" => Ok(NetworkProtocol::Udp),
        "unix_stream" | "unix-stream" => Ok(NetworkProtocol::UnixStream),
        "unix_dgram" | "unix-dgram" => Ok(NetworkProtocol::UnixDgram),
        other => Err(bad(
            group,
            &format!(
                "protocol: \"{other}\" is not one of tcp, udp, unix_stream, or unix_dgram"
            ),
        )),
    }
}

fn is_stream_protocol(p: NetworkProtocol) -> bool {
    matches!(
        p,
        NetworkProtocol::Tcp | NetworkProtocol::UnixStream
    )
}

/// Parse a timeout: the word `none`, or a whole number with a unit suffix of
/// `us`, `ms`, or `s`.
///
/// `Ok(None)` is the word `none`: a read that does not time out.
fn parse_timeout(group: &str, s: &Scalar) -> EndpointConfigResult<Option<Duration>> {
    let text = s.as_str();
    let lower = text.to_ascii_lowercase();

    if lower == "none" {
        return Ok(None);
    }

    let (digits, unit) = match lower.strip_suffix("us") {
        Some(d) => (d, "us"),
        None => match lower.strip_suffix("ms") {
            Some(d) => (d, "ms"),
            None => match lower.strip_suffix('s') {
                Some(d) => (d, "s"),
                None => {
                    return Err(bad(
                        group,
                        &format!(
                            "timeout: \"{text}\" has no unit: expected a number followed by \
                             us, ms, or s, or the word none"
                        ),
                    ))
                }
            },
        },
    };

    let n: u64 = digits.trim().parse().map_err(|_| {
        bad(
            group,
            &format!("timeout: \"{text}\" is not a whole number of {unit}"),
        )
    })?;

    let d = match unit {
        "us" => Duration::from_micros(n),
        "ms" => Duration::from_millis(n),
        _ => Duration::from_secs(n),
    };

    if d.is_zero() {
        return Err(bad(
            group,
            "timeout: a zero timeout is not a timeout; say none to mean a read that does not \
             time out",
        ));
    }

    Ok(Some(d))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// The same configuration written both ways. These two must parse into
    /// structures that compare equal; that is the point of the module.
    const YAML: &str = r#"
general:
  version: "1.0"
  description: Payload endpoints

endpoint_groups:
  - name: payload_serial
    type: serial
    datarate: 115200
    stop_bits: 1
    byte_length: 8
    stream:
      max_length: 1024
      timeout: 500ms
      terminators: [0x0D, 0x0A]

  - name: payload_tcp
    type: network
    protocol: tcp
    stream:
      max_length: 4096
      timeout: none

  - name: payload_udp
    type: network
    protocol: udp

endpoints:
  - name: pay0
    group: payload_serial
    device: /dev/ttyS0
  - name: pay1
    group: payload_serial
    device: /dev/ttyS1
  - name: pay2
    group: payload_tcp
    address: 192.168.1.10
    port: 5000
  - name: pay3
    group: payload_udp
    address: 192.168.1.11
    port: 5001
"#;

    const XML: &str = r#"<endpoint-configuration>
  <general version="1.0" description="Payload endpoints"/>

  <endpoint-groups>
    <group name="payload_serial" type="serial"
           datarate="115200" stop_bits="1" byte_length="8">
      <stream max_length="1024" timeout="500ms" terminators="0x0D,0x0A"/>
    </group>

    <group name="payload_tcp" type="network" protocol="tcp">
      <stream max_length="4096" timeout="none"/>
    </group>

    <group name="payload_udp" type="network" protocol="udp"/>
  </endpoint-groups>

  <endpoints>
    <endpoint name="pay0" group="payload_serial" device="/dev/ttyS0"/>
    <endpoint name="pay1" group="payload_serial" device="/dev/ttyS1"/>
    <endpoint name="pay2" group="payload_tcp" address="192.168.1.10" port="5000"/>
    <endpoint name="pay3" group="payload_udp" address="192.168.1.11" port="5001"/>
  </endpoints>
</endpoint-configuration>"#;

    #[test]
    fn yaml_and_xml_produce_the_same_structures() {
        let from_yaml = from_yaml_str(YAML).expect("YAML should parse");
        let from_xml = from_xml_str(XML).expect("XML should parse");
        assert_eq!(from_yaml, from_xml);
    }

    #[test]
    fn general_section_is_read() {
        let doc = from_yaml_str(YAML).unwrap();
        assert_eq!(doc.general.version.as_deref(), Some("1.0"));
        assert_eq!(doc.general.description.as_deref(), Some("Payload endpoints"));
    }

    #[test]
    fn serial_group_attributes_are_read() {
        let doc = from_yaml_str(YAML).unwrap();
        let g = doc.group("payload_serial").expect("group present");
        match &g.kind {
            GroupKind::Serial(s) => {
                assert_eq!(s.datarate, 115_200);
                assert_eq!(s.stop_bits, StopBits::One);
                assert_eq!(s.byte_length.bits(), 8);
                assert_eq!(s.stream.max_length, 1024);
                assert_eq!(s.stream.timeout, Some(Duration::from_millis(500)));
                assert_eq!(s.stream.terminators, vec![0x0D, 0x0A]);
            }
            other => panic!("expected a serial group, got {other:?}"),
        }
    }

    #[test]
    fn groups_hold_no_device_or_address_and_endpoints_hold_nothing_else() {
        // The division of labour the file format exists to express.
        let doc = from_yaml_str(YAML).unwrap();
        let serial: Vec<_> = doc
            .endpoints
            .iter()
            .filter(|e| e.group == "payload_serial")
            .collect();
        assert_eq!(serial.len(), 2);
        assert_eq!(
            serial[0].location,
            EndpointLocation::Device {
                path: "/dev/ttyS0".to_string()
            }
        );
        assert_eq!(
            serial[1].location,
            EndpointLocation::Device {
                path: "/dev/ttyS1".to_string()
            }
        );
        // Both endpoints share one set of attributes, held once, in the group.
        assert_eq!(
            doc.group_of(serial[0]).unwrap().kind,
            doc.group_of(serial[1]).unwrap().kind
        );
    }

    #[test]
    fn network_endpoints_carry_address_and_port() {
        let doc = from_xml_str(XML).unwrap();
        let e = doc.endpoints.iter().find(|e| e.name == "pay2").unwrap();
        assert_eq!(
            e.location,
            EndpointLocation::Network {
                address: "192.168.1.10".to_string(),
                port: 5000
            }
        );
    }

    // -- the stream rules ---------------------------------------------------

    fn serial_stream(stream_body: &str) -> EndpointConfigResult<EndpointConfigDoc> {
        from_yaml_str(&format!(
            "endpoint_groups:\n  \
             - name: g\n    \
               type: serial\n    \
               datarate: 9600\n    \
               stop_bits: 1\n    \
               byte_length: 8\n    \
               stream:\n{stream_body}"
        ))
    }

    fn stream_of(doc: &EndpointConfigDoc) -> &StreamParams {
        doc.group("g").unwrap().kind.stream().unwrap()
    }

    #[test]
    fn max_length_is_required() {
        let e = serial_stream("      timeout: 1s\n").unwrap_err();
        assert!(
            matches!(e, EndpointConfigError::StreamMissingMaxLength(_)),
            "got {e:?}"
        );
    }

    #[test]
    fn timeout_alone_is_accepted() {
        let doc = serial_stream("      max_length: 64\n      timeout: 250ms\n").unwrap();
        let s = stream_of(&doc);
        assert_eq!(s.timeout, Some(Duration::from_millis(250)));
        assert!(s.terminators.is_empty());
        assert!(!s.is_fixed_length());
    }

    #[test]
    fn terminators_alone_are_accepted() {
        let doc = serial_stream("      max_length: 64\n      terminators: [10]\n").unwrap();
        let s = stream_of(&doc);
        assert_eq!(s.timeout, None);
        assert_eq!(s.terminators, vec![10]);
        assert!(!s.is_fixed_length());
    }

    #[test]
    fn both_timeout_and_terminators_are_accepted() {
        let doc =
            serial_stream("      max_length: 64\n      timeout: 1s\n      terminators: [0x04]\n")
                .unwrap();
        let s = stream_of(&doc);
        assert_eq!(s.timeout, Some(Duration::from_secs(1)));
        assert_eq!(s.terminators, vec![0x04]);
    }

    #[test]
    fn neither_timeout_nor_terminators_is_rejected() {
        let e = serial_stream("      max_length: 64\n").unwrap_err();
        assert!(
            matches!(e, EndpointConfigError::StreamNeedsTimeoutOrTerminators(_)),
            "got {e:?}"
        );
    }

    #[test]
    fn timeout_none_is_how_a_fixed_length_read_is_requested() {
        // The documented way to say "hand me max_length bytes at a time".
        let doc = serial_stream("      max_length: 12\n      timeout: none\n").unwrap();
        let s = stream_of(&doc);
        assert_eq!(s.max_length, 12);
        assert_eq!(s.timeout, None);
        assert!(s.terminators.is_empty());
        assert!(s.is_fixed_length());
    }

    #[test]
    fn empty_terminator_list_is_rejected() {
        let e = serial_stream("      max_length: 64\n      terminators: []\n").unwrap_err();
        assert!(
            matches!(e, EndpointConfigError::StreamEmptyTerminators(_)),
            "got {e:?}"
        );
    }

    // -- scalar spellings ---------------------------------------------------

    #[test]
    fn timeout_units_are_understood() {
        for (text, want) in [
            ("900us", Duration::from_micros(900)),
            ("250ms", Duration::from_millis(250)),
            ("3s", Duration::from_secs(3)),
        ] {
            let doc =
                serial_stream(&format!("      max_length: 8\n      timeout: {text}\n")).unwrap();
            assert_eq!(stream_of(&doc).timeout, Some(want), "for {text}");
        }
    }

    #[test]
    fn a_timeout_without_a_unit_is_rejected() {
        let e = serial_stream("      max_length: 8\n      timeout: 250\n").unwrap_err();
        assert!(format!("{e}").contains("no unit"), "got {e}");
    }

    #[test]
    fn a_zero_timeout_is_rejected_in_favour_of_none() {
        let e = serial_stream("      max_length: 8\n      timeout: 0ms\n").unwrap_err();
        assert!(format!("{e}").contains("say none"), "got {e}");
    }

    #[test]
    fn terminators_accept_a_sequence_or_a_delimited_string() {
        let seq = serial_stream("      max_length: 8\n      terminators: [0x0D, 0x0A]\n").unwrap();
        let text = serial_stream("      max_length: 8\n      terminators: \"0x0D, 0x0A\"\n").unwrap();
        assert_eq!(stream_of(&seq).terminators, vec![0x0D, 0x0A]);
        assert_eq!(stream_of(&seq).terminators, stream_of(&text).terminators);
    }

    #[test]
    fn terminators_accept_decimal_and_hex() {
        let doc = serial_stream("      max_length: 8\n      terminators: [13, 0x0A]\n").unwrap();
        assert_eq!(stream_of(&doc).terminators, vec![13, 10]);
    }

    #[test]
    fn a_terminator_above_a_byte_is_rejected() {
        let e = serial_stream("      max_length: 8\n      terminators: [256]\n").unwrap_err();
        assert!(format!("{e}").contains("not a byte value"), "got {e}");
    }

    #[test]
    fn stop_bits_accept_one_one_and_a_half_and_two() {
        for (text, want) in [
            ("1", StopBits::One),
            ("1.5", StopBits::OnePointFive),
            ("2", StopBits::Two),
        ] {
            let yaml = format!(
                "endpoint_groups:\n  - name: g\n    type: serial\n    datarate: 9600\n    \
                 stop_bits: {text}\n    byte_length: 8\n    stream:\n      max_length: 8\n      \
                 timeout: none\n"
            );
            let doc = from_yaml_str(&yaml).unwrap();
            match &doc.group("g").unwrap().kind {
                GroupKind::Serial(s) => assert_eq!(s.stop_bits, want, "for {text}"),
                other => panic!("expected serial, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_byte_length_no_uart_offers_is_rejected() {
        let yaml = "endpoint_groups:\n  - name: g\n    type: serial\n    datarate: 9600\n    \
                    stop_bits: 1\n    byte_length: 9\n    stream:\n      max_length: 8\n      \
                    timeout: none\n";
        let e = from_yaml_str(yaml).unwrap_err();
        assert!(format!("{e}").contains("out of range"), "got {e}");
    }

    // -- cross-section rules ------------------------------------------------

    #[test]
    fn an_endpoint_naming_an_undefined_group_is_rejected() {
        let yaml = "endpoints:\n  - name: e\n    group: nope\n    device: /dev/ttyS0\n";
        let e = from_yaml_str(yaml).unwrap_err();
        assert!(
            matches!(e, EndpointConfigError::UnknownGroup { .. }),
            "got {e:?}"
        );
    }

    #[test]
    fn a_duplicate_group_name_is_rejected() {
        let one = "  - name: g\n    type: network\n    protocol: udp\n";
        let e = from_yaml_str(&format!("endpoint_groups:\n{one}{one}")).unwrap_err();
        assert!(
            matches!(e, EndpointConfigError::DuplicateGroup(_)),
            "got {e:?}"
        );
    }

    #[test]
    fn a_serial_endpoint_given_an_address_is_rejected() {
        let yaml = "endpoint_groups:\n  - name: g\n    type: serial\n    datarate: 9600\n    \
                    stop_bits: 1\n    byte_length: 8\n    stream:\n      max_length: 8\n      \
                    timeout: none\n\
                    endpoints:\n  - name: e\n    group: g\n    address: 10.0.0.1\n    port: 1\n";
        let e = from_yaml_str(yaml).unwrap_err();
        assert!(
            matches!(e, EndpointConfigError::UnusedEndpointField { .. }),
            "got {e:?}"
        );
    }

    #[test]
    fn a_serial_group_given_a_protocol_is_rejected() {
        let yaml = "endpoint_groups:\n  - name: g\n    type: serial\n    protocol: tcp\n    \
                    datarate: 9600\n    stop_bits: 1\n    byte_length: 8\n    stream:\n      \
                    max_length: 8\n      timeout: none\n";
        let e = from_yaml_str(yaml).unwrap_err();
        assert!(
            matches!(e, EndpointConfigError::UnusedGroupField { .. }),
            "got {e:?}"
        );
    }

    #[test]
    fn a_datagram_group_needs_no_stream_section() {
        let doc =
            from_yaml_str("endpoint_groups:\n  - name: g\n    type: network\n    protocol: udp\n")
                .unwrap();
        assert!(doc.group("g").unwrap().kind.stream().is_none());
    }

    #[test]
    fn a_tcp_group_without_a_stream_section_is_rejected() {
        let e =
            from_yaml_str("endpoint_groups:\n  - name: g\n    type: network\n    protocol: tcp\n")
                .unwrap_err();
        assert!(
            matches!(e, EndpointConfigError::MissingGroupField { .. }),
            "got {e:?}"
        );
    }

    #[test]
    fn an_unknown_group_type_is_rejected() {
        let e = from_yaml_str("endpoint_groups:\n  - name: g\n    type: carrier_pigeon\n")
            .unwrap_err();
        assert!(
            matches!(e, EndpointConfigError::UnknownGroupKind { .. }),
            "got {e:?}"
        );
    }

    #[test]
    fn a_unix_socket_endpoint_is_named_by_a_path() {
        let yaml = "endpoint_groups:\n  - name: g\n    type: network\n    \
                    protocol: unix_dgram\n\
                    endpoints:\n  - name: e\n    group: g\n    device: /run/pay.sock\n";
        let doc = from_yaml_str(yaml).unwrap();
        assert_eq!(
            doc.endpoints[0].location,
            EndpointLocation::Device {
                path: "/run/pay.sock".to_string()
            }
        );
    }

    // -- the bus types ------------------------------------------------------

    /// A group of the given type, plus one endpoint, written compactly.
    fn bus(group_body: &str, endpoint_body: &str) -> EndpointConfigResult<EndpointConfigDoc> {
        from_yaml_str(&format!(
            "endpoint_groups:\n  - name: g\n{group_body}\
             endpoints:\n  - name: e\n    group: g\n{endpoint_body}"
        ))
    }

    #[test]
    fn i2c_group_attributes_are_read() {
        let doc = bus(
            "    type: i2c\n    ten_bit: false\n    pec: true\n    retries: 3\n    \
             timeout: 50ms\n    bus_speed: 400000\n",
            "    device: /dev/i2c-1\n    address: 0x48\n",
        )
        .unwrap();
        match &doc.group("g").unwrap().kind {
            GroupKind::I2c(p) => {
                assert!(!p.ten_bit);
                assert!(p.pec);
                assert_eq!(p.retries, 3);
                assert_eq!(p.timeout, Some(Duration::from_millis(50)));
                assert_eq!(p.bus_speed, Some(400_000));
            }
            other => panic!("expected i2c, got {other:?}"),
        }
    }

    #[test]
    fn i2c_attributes_all_have_defaults() {
        // Nothing but the type is required: the protocol fixes the framing,
        // and every knob here has a sensible quiet setting.
        let doc = bus("    type: i2c\n", "    device: /dev/i2c-1\n    address: 16\n").unwrap();
        match &doc.group("g").unwrap().kind {
            GroupKind::I2c(p) => {
                assert!(!p.ten_bit);
                assert!(!p.pec);
                assert_eq!(p.retries, 0);
                assert_eq!(p.timeout, None);
                assert_eq!(p.bus_speed, None);
            }
            other => panic!("expected i2c, got {other:?}"),
        }
    }

    #[test]
    fn an_i2c_endpoint_carries_both_a_bus_and_an_address() {
        let doc = bus(
            "    type: i2c\n",
            "    device: /dev/i2c-2\n    address: 0x49\n",
        )
        .unwrap();
        assert_eq!(
            doc.endpoints[0].location,
            EndpointLocation::I2c {
                bus: "/dev/i2c-2".to_string(),
                address: 0x49
            }
        );
    }

    #[test]
    fn an_i2c_endpoint_without_an_address_is_rejected() {
        let e = bus("    type: i2c\n", "    device: /dev/i2c-2\n").unwrap_err();
        assert!(
            matches!(e, EndpointConfigError::MissingEndpointField { .. }),
            "got {e:?}"
        );
        assert!(format!("{e}").contains("slave address"), "got {e}");
    }

    #[test]
    fn an_i2c_endpoint_without_a_bus_is_rejected() {
        let e = bus("    type: i2c\n", "    address: 0x48\n").unwrap_err();
        assert!(format!("{e}").contains("bus device"), "got {e}");
    }

    #[test]
    fn an_address_too_wide_for_the_groups_addressing_is_rejected() {
        let e = bus("    type: i2c\n", "    device: /dev/i2c-2\n    address: 0x90\n").unwrap_err();
        assert!(format!("{e}").contains("does not fit in 7 bits"), "got {e}");
        // ...and the same address is fine once the group says so.
        let doc = bus(
            "    type: i2c\n    ten_bit: true\n",
            "    device: /dev/i2c-2\n    address: 0x90\n",
        )
        .unwrap();
        assert_eq!(
            doc.endpoints[0].location,
            EndpointLocation::I2c {
                bus: "/dev/i2c-2".to_string(),
                address: 0x90
            }
        );
    }

    #[test]
    fn a_reserved_seven_bit_address_is_rejected() {
        // In range, but the specification keeps these, so no device can
        // answer to one.
        for addr in ["0x00", "0x01", "0x07", "0x78", "0x7B", "0x7F"] {
            let e = bus(
                "    type: i2c\n",
                &format!("    device: /dev/i2c-1\n    address: {addr}\n"),
            )
            .unwrap_err();
            assert!(
                format!("{e}").contains("reserved by the I2C specification"),
                "for {addr}: got {e}"
            );
        }
    }

    #[test]
    fn the_addresses_either_side_of_the_reserved_blocks_are_accepted() {
        for (addr, want) in [("0x08", 0x08), ("0x77", 0x77)] {
            let doc = bus(
                "    type: i2c\n",
                &format!("    device: /dev/i2c-1\n    address: {addr}\n"),
            )
            .unwrap_or_else(|e| panic!("{addr} should be usable: {e}"));
            assert_eq!(
                doc.endpoints[0].location,
                EndpointLocation::I2c {
                    bus: "/dev/i2c-1".to_string(),
                    address: want
                }
            );
        }
    }

    #[test]
    fn ten_bit_addressing_has_no_reserved_block() {
        // A 10-bit transfer carries its address after the 0x78 prefix, so the
        // whole space is available and the 7-bit reservations do not apply.
        for addr in ["0x00", "0x78", "0x3FF"] {
            bus(
                "    type: i2c\n    ten_bit: true\n",
                &format!("    device: /dev/i2c-1\n    address: {addr}\n"),
            )
            .unwrap_or_else(|e| panic!("{addr} should be usable with ten_bit: {e}"));
        }
    }

    #[test]
    fn an_i2c_timeout_is_rounded_up_to_the_drivers_resolution() {
        let doc = bus(
            "    type: i2c\n    timeout: 25ms\n",
            "    device: /dev/i2c-1\n    address: 8\n",
        )
        .unwrap();
        match &doc.group("g").unwrap().kind {
            GroupKind::I2c(p) => {
                assert_eq!(p.timeout, Some(Duration::from_millis(25)));
                assert_eq!(p.effective_timeout(), Some(Duration::from_millis(30)));
            }
            other => panic!("expected i2c, got {other:?}"),
        }
    }

    #[test]
    fn spi_group_attributes_are_read() {
        let doc = bus(
            "    type: spi\n    max_speed: 10000000\n    mode: 3\n    bits_per_word: 16\n    \
             bit_order: lsb\n    cs_active: high\n",
            "    device: /dev/spidev0.1\n",
        )
        .unwrap();
        match &doc.group("g").unwrap().kind {
            GroupKind::Spi(p) => {
                assert_eq!(p.max_speed, 10_000_000);
                assert_eq!(p.mode, SpiMode::Mode3);
                assert_eq!(p.bits_per_word.bits(), 16);
                assert_eq!(p.bit_order, BitOrder::LsbFirst);
                assert_eq!(p.cs_active, CsActive::High);
            }
            other => panic!("expected spi, got {other:?}"),
        }
    }

    #[test]
    fn spi_defaults_are_the_usual_eight_bit_msb_first_active_low() {
        let doc = bus(
            "    type: spi\n    max_speed: 1000000\n    mode: 0\n",
            "    device: /dev/spidev0.0\n",
        )
        .unwrap();
        match &doc.group("g").unwrap().kind {
            GroupKind::Spi(p) => {
                assert_eq!(p.bits_per_word.bits(), 8);
                assert_eq!(p.bit_order, BitOrder::MsbFirst);
                assert_eq!(p.cs_active, CsActive::Low);
            }
            other => panic!("expected spi, got {other:?}"),
        }
    }

    #[test]
    fn a_spi_group_without_a_mode_is_rejected() {
        let e = bus(
            "    type: spi\n    max_speed: 1000000\n",
            "    device: /dev/spidev0.0\n",
        )
        .unwrap_err();
        assert!(
            matches!(e, EndpointConfigError::MissingGroupField { field: "mode", .. }),
            "got {e:?}"
        );
    }

    #[test]
    fn a_spi_mode_outside_zero_to_three_is_rejected() {
        let e = bus(
            "    type: spi\n    max_speed: 1000000\n    mode: 4\n",
            "    device: /dev/spidev0.0\n",
        )
        .unwrap_err();
        assert!(format!("{e}").contains("not one of 0, 1, 2, or 3"), "got {e}");
    }

    #[test]
    fn spi_modes_carry_the_polarity_and_phase_their_numbers_mean() {
        for (mode, cpol, cpha) in [
            (SpiMode::Mode0, false, false),
            (SpiMode::Mode1, false, true),
            (SpiMode::Mode2, true, false),
            (SpiMode::Mode3, true, true),
        ] {
            assert_eq!(mode.cpol(), cpol, "cpol of mode {mode}");
            assert_eq!(mode.cpha(), cpha, "cpha of mode {mode}");
        }
    }

    #[test]
    fn a_spi_endpoint_is_named_by_its_device_node_alone() {
        // The node names the bus and the chip select, so an address is not
        // merely unnecessary but wrong.
        let e = bus(
            "    type: spi\n    max_speed: 1000000\n    mode: 0\n",
            "    device: /dev/spidev0.0\n    address: 0x10\n    port: 1\n",
        )
        .unwrap_err();
        assert!(
            matches!(e, EndpointConfigError::UnusedEndpointField { .. }),
            "got {e:?}"
        );
    }

    #[test]
    fn a_bus_group_given_a_stream_section_is_rejected() {
        // A master clocks exactly as many bytes as it asks for, so there is
        // no rule to give for where a read ends.
        for kind in ["    type: i2c\n", "    type: spi\n    max_speed: 1\n    mode: 0\n"] {
            let yaml = format!(
                "endpoint_groups:\n  - name: g\n{kind}    stream:\n      max_length: 8\n      \
                 timeout: none\n"
            );
            let e = from_yaml_str(&yaml).unwrap_err();
            assert!(
                matches!(e, EndpointConfigError::UnusedGroupField { .. }),
                "for {kind}: got {e:?}"
            );
        }
    }

    #[test]
    fn an_attribute_of_another_type_is_rejected_on_every_type() {
        // The rule that a misplaced attribute is reported where it was
        // written, checked across the cross-product rather than one way.
        for (group, foreign) in [
            ("    type: i2c\n", "    datarate: 9600\n"),
            ("    type: i2c\n", "    mode: 0\n"),
            ("    type: spi\n    max_speed: 1\n    mode: 0\n", "    pec: true\n"),
            ("    type: spi\n    max_speed: 1\n    mode: 0\n", "    protocol: tcp\n"),
            ("    type: network\n    protocol: udp\n", "    bus_speed: 100000\n"),
            (
                "    type: serial\n    datarate: 9600\n    stop_bits: 1\n    byte_length: 8\n    \
                 stream:\n      max_length: 8\n      timeout: none\n",
                "    cs_active: low\n",
            ),
        ] {
            let yaml = format!("endpoint_groups:\n  - name: g\n{group}{foreign}");
            let e = from_yaml_str(&yaml).unwrap_err();
            assert!(
                matches!(e, EndpointConfigError::UnusedGroupField { .. }),
                "for {foreign}on {group}: got {e:?}"
            );
        }
    }

    #[test]
    fn bus_groups_round_trip_between_yaml_and_xml() {
        let yaml = "endpoint_groups:\n  \
                    - name: bus\n    type: i2c\n    pec: true\n    retries: 2\n    \
                    bus_speed: 400000\n  \
                    - name: chip\n    type: spi\n    max_speed: 8000000\n    mode: 1\n    \
                    bit_order: lsb\n\
                    endpoints:\n  \
                    - name: a\n    group: bus\n    device: /dev/i2c-0\n    address: 0x2A\n  \
                    - name: b\n    group: chip\n    device: /dev/spidev1.0\n";
        let xml = r#"<endpoint-configuration>
  <endpoint-groups>
    <group name="bus" type="i2c" pec="true" retries="2" bus_speed="400000"/>
    <group name="chip" type="spi" max_speed="8000000" mode="1" bit_order="lsb"/>
  </endpoint-groups>
  <endpoints>
    <endpoint name="a" group="bus" device="/dev/i2c-0" address="0x2A"/>
    <endpoint name="b" group="chip" device="/dev/spidev1.0"/>
  </endpoints>
</endpoint-configuration>"#;
        assert_eq!(from_yaml_str(yaml).unwrap(), from_xml_str(xml).unwrap());
    }

    #[test]
    fn hyphenated_and_underscored_spellings_both_work() {
        let a = from_yaml_str(
            "endpoint_groups:\n  - name: g\n    type: serial\n    datarate: 9600\n    \
             stop_bits: 1\n    byte_length: 8\n    stream:\n      max_length: 8\n      \
             timeout: none\n",
        )
        .unwrap();
        let b = from_yaml_str(
            "endpoint-groups:\n  - name: g\n    type: serial\n    datarate: 9600\n    \
             stop-bits: 1\n    byte-length: 8\n    stream:\n      max-length: 8\n      \
             timeout: none\n",
        )
        .unwrap();
        assert_eq!(a, b);
    }
}
