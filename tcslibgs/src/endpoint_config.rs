//! Endpoint configuration files, in YAML or XML
//!
//! Both formats describe the same thing and parse into the same Rust types:
//! a general section, a section naming groups of endpoints, and a section
//! naming the endpoints themselves. A group carries every attribute shared by
//! endpoints of its type; what it deliberately does not carry is the device
//! name or network address, because that is what distinguishes one endpoint
//! in a group from another and so belongs to the endpoint. Every group has an
//! endpoint in it: a group nothing names has no effect on the configuration,
//! which is also what a misspelled group name looks like.
//!
//! The syntax of both formats is specified in `docs/design.rst`, under
//! "Endpoint Configuration Files".
//!
//! One set of wire types serves both formats. XML attributes reach serde with
//! an `@` prefix, so every field carries that spelling as an alias alongside
//! its YAML spelling, and a section arrives either as a YAML sequence or as
//! an XML element with repeated children (see [`Section`]).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::marker::PhantomData;
use std::path::Path;
use std::time::Duration;

use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::endpoint_config_i2c::{parse_i2c_address, I2cParams};
use crate::endpoint_config_network::NetworkParams;
use crate::endpoint_config_serial::SerialParams;
use crate::endpoint_config_spi::SpiParams;
use crate::format::ConfigFormat;

// The per-kind types are named from here as well as from their own files, so
// that a reader of a configuration document reaches everything it can hold
// through the module that parses it.
pub use crate::endpoint_config_i2c::I2cParams as I2cGroupParams;
pub use crate::endpoint_config_serial::{ByteLength, StopBits};
pub use crate::endpoint_config_spi::{BitOrder, BitsPerWord, CsActive, SpiMode};
use crate::types::{
    DHConfig, DHId, DHName, EndpointConfig, I2cConfig, NetworkConfig,
    NetworkProtocol, SerialConfig, SpiConfig,
};
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

    /// Worded exactly as the payload and simulator configuration formats word
    /// the same rule, so that one rule reads as one rule wherever it is met.
    #[error(
        "endpoint group \"{0}\" is named by no endpoint: name it from one, or remove \
         the group"
    )]
    UnusedGroup(String),

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

    // The four below arise only when converting endpoints into data handlers,
    // not when reading a file. An endpoint configuration describing how to
    // reach a device is complete without any of them.
    #[error("endpoint \"{0}\" has no dh_id, and a data handler is addressed by id")]
    EndpointHasNoDhId(String),

    #[error("endpoints \"{first}\" and \"{second}\" share dh_id {dh_id}")]
    DuplicateDhId {
        first: String,
        second: String,
        dh_id: u32,
    },

    #[error(
        "endpoint \"{endpoint}\" is in group \"{group}\", which states no packet size, \
         and a data handler needs one"
    )]
    GroupHasNoPacketSize { endpoint: String, group: String },

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

    /// Turn these endpoints into data handler configurations.
    ///
    /// The two formats describe overlapping things: a payload configuration
    /// says which data handlers exist and how tcspecial reaches each one, and
    /// an endpoint configuration says how to reach a device in far more
    /// detail. This is the bridge, so that the richer description can serve
    /// where the payload format does today.
    ///
    /// Nothing calls it yet. It exists so that the conversion is settled and
    /// tested before any program depends on it; which file each program reads
    /// is a separate question, and one that has to be answered for all three
    /// at once, since tcsmoc's panels, tcssim's payloads and tcspecial's
    /// handlers must describe the same thing.
    ///
    /// Not every endpoint can become a data handler. A file describing only
    /// how to reach a device is complete without an id or a packet size, and
    /// an I2C endpoint has no transport tcspecial can open; each of those is
    /// an error here rather than at the file's own validation, because none of
    /// them is wrong about the endpoint.
    pub fn to_dh_configs(&self) -> EndpointConfigResult<Vec<DHConfig>> {
        let mut by_id: BTreeMap<u32, &str> = BTreeMap::new();
        let mut configs = Vec::with_capacity(self.endpoints.len());

        for endpoint in &self.endpoints {
            let group = self
                .group_of(endpoint)
                .ok_or_else(|| EndpointConfigError::UnknownGroup {
                    endpoint: endpoint.name.clone(),
                    group: endpoint.group.clone(),
                })?;

            let dh_id = endpoint
                .dh_id
                .ok_or_else(|| EndpointConfigError::EndpointHasNoDhId(endpoint.name.clone()))?;

            // A duplicate parses cleanly and then has one handler shadow
            // another, as it would in a payload file.
            if let Some(first) = by_id.insert(dh_id, endpoint.name.as_str()) {
                return Err(EndpointConfigError::DuplicateDhId {
                    first: first.to_string(),
                    second: endpoint.name.clone(),
                    dh_id,
                });
            }

            let packet_size = group.packet_size.ok_or_else(|| {
                EndpointConfigError::GroupHasNoPacketSize {
                    endpoint: endpoint.name.clone(),
                    group: group.name.clone(),
                }
            })?;

            configs.push(DHConfig {
                dh_id: DHId(dh_id),
                name: DHName::new(&endpoint.name),
                endpoint: endpoint_config_of(endpoint, group)?,
                packet_size: packet_size as usize,
                oc: endpoint.oc.clone(),
                // An endpoint configuration has no mode to give yet, so its
                // endpoints describe the kind of payload that sends on its
                // own. A triggered endpoint would need the trigger and its
                // interval here, which is a group's business -- several
                // endpoints of one group are commonly polled alike.
                mode: Default::default(),
            });
        }

        Ok(configs)
    }
}

/// What tcspecial opens to reach one endpoint.
///
/// The transport follows from the group and the address from the endpoint,
/// which is the division the format is built around. A Unix socket is the one
/// case where the two disagree about shape: its group is a network group, but
/// it is located by a path rather than by host and port, so it becomes a
/// network endpoint whose address is that path. Its port is meaningless and
/// set to zero, which is how a payload file spells the same thing.
fn endpoint_config_of(
    endpoint: &EndpointDef,
    group: &EndpointGroup,
) -> EndpointConfigResult<EndpointConfig> {
    match &endpoint.location {
        EndpointLocation::I2c { bus, address } => match &group.kind {
            GroupKind::I2c(i2c) => Ok(EndpointConfig::I2c(I2cConfig {
                bus: bus.clone(),
                address: *address,
                ten_bit: i2c.ten_bit,
                pec: i2c.pec,
            })),
            // Only an I2C group gives an endpoint a bus and an address, so
            // this is unreachable through the parser; an error rather than a
            // panic, because a library should not bring a caller down.
            _ => Err(bad(
                &group.name,
                &format!(
                    "endpoint \"{}\" is on a bus, which a {} group does not put it on",
                    endpoint.name,
                    group.kind.type_name()
                ),
            )),
        },
        EndpointLocation::Network { address, port } => match &group.kind {
            GroupKind::Network(net) => Ok(EndpointConfig::Network(NetworkConfig {
                protocol: net.protocol,
                address: address.clone(),
                port: *port,
            })),
            // Only a network group gives an endpoint a host and a port, so
            // this is unreachable through the parser; it is an error rather
            // than a panic because a library should not bring a caller down.
            _ => Err(bad(
                &group.name,
                &format!(
                    "endpoint \"{}\" has a network address, which a {} group does not give it",
                    endpoint.name,
                    group.kind.type_name()
                ),
            )),
        },
        // Every kind but a network address is located by a device node, and
        // which kind it is decides what else goes with it. They used all to
        // become a plain Device, which opened the right file and then talked
        // to it as though it had no terms of its own: a serial line at
        // whatever rate the port was last left at, a SPI peripheral at
        // whatever mode.
        EndpointLocation::Path { path } => match &group.kind {
            GroupKind::Network(net) => Ok(EndpointConfig::Network(NetworkConfig {
                protocol: net.protocol,
                address: path.clone(),
                port: 0,
            })),
            GroupKind::Serial(serial) => Ok(EndpointConfig::Serial(SerialConfig {
                path: path.clone(),
                datarate: serial.datarate,
                stop_bits: serial.stop_bits,
                byte_length: serial.byte_length.bits(),
            })),
            GroupKind::Spi(spi) => Ok(EndpointConfig::Spi(SpiConfig {
                path: path.clone(),
                max_speed: spi.max_speed,
                mode: spi.mode,
                bits_per_word: spi.bits_per_word.bits(),
                bit_order: spi.bit_order,
                cs_active: spi.cs_active,
            })),
            // An I2C group's endpoints carry a bus and an address, which the
            // parser requires of them, so a device node alone in one is a
            // shape it cannot produce.
            GroupKind::I2c(_) => Err(bad(
                &group.name,
                &format!(
                    "endpoint \"{}\" names a device alone, but a device on an I2C bus \
                     needs the bus and the address on it",
                    endpoint.name
                ),
            )),
        },
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
    /// Bytes in one packet exchanged with an endpoint of this group.
    ///
    /// Shared by every endpoint of the group, like every other group
    /// attribute, and applying to all four types: a packet has a size
    /// whether it travels over a serial line, a socket, or a bus.
    ///
    /// `None` for a file that did not state one. An endpoint configuration
    /// describing only how to reach a device need not, so this is optional;
    /// a data handler built from one needs it.
    pub packet_size: Option<u32>,
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
    /// Identifier of the data handler this endpoint becomes.
    ///
    /// `None` for a file that assigns none. An endpoint configuration
    /// describing only how to reach a device need not, so this is optional
    /// exactly as a group's packet size is; converting the endpoint into a
    /// data handler needs it, because a handler is addressed by id.
    pub dh_id: Option<u32>,
    /// UDP address the data handler this endpoint becomes exchanges payload
    /// data with the OC on.
    ///
    /// Optional, and belonging to the endpoint rather than its group for the
    /// reason the endpoint's own address does: it is what distinguishes one
    /// handler from another. Nothing but starting a handler needs it, so an
    /// endpoint configuration that only says how to reach devices has none.
    pub oc: Option<NetworkConfig>,
}

/// What locates one endpoint of a group, and so distinguishes it from the
/// others: a device name, a network address, or, on a bus that addresses its
/// devices, the address of the device on that bus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum EndpointLocation {
    /// A path in the filesystem, whatever is at the end of it: a serial port,
    /// a SPI device node, a plain device, or a Unix-domain socket.
    ///
    /// Not named `Device`, which is one *kind* of endpoint: this is how an
    /// endpoint is located rather than what it is, and four of the five kinds
    /// are located this way. Which kind it turns out to be follows from the
    /// group it is in -- a path in a serial group is a line, the same path in
    /// a SPI group is a peripheral -- and a SPI device node needs no address
    /// beside it because it names the bus and the chip select together.
    Path { path: String },
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
pub(crate) struct GroupWire {
    #[serde(alias = "@name")]
    pub(crate) name: String,
    #[serde(rename = "type", alias = "@type")]
    pub(crate) kind: String,
    // Serial attributes.
    #[serde(default, alias = "@datarate")]
    pub(crate) datarate: Option<Scalar>,
    #[serde(default, alias = "@stop_bits", alias = "stop-bits", alias = "@stop-bits")]
    pub(crate) stop_bits: Option<Scalar>,
    #[serde(
        default,
        alias = "@byte_length",
        alias = "byte-length",
        alias = "@byte-length"
    )]
    pub(crate) byte_length: Option<Scalar>,
    // Network attributes.
    #[serde(default, alias = "@protocol")]
    pub(crate) protocol: Option<String>,
    // I2C attributes.
    #[serde(default, alias = "@ten_bit", alias = "ten-bit", alias = "@ten-bit")]
    pub(crate) ten_bit: Option<Scalar>,
    #[serde(default, alias = "@pec")]
    pub(crate) pec: Option<Scalar>,
    #[serde(default, alias = "@retries")]
    pub(crate) retries: Option<Scalar>,
    #[serde(default, alias = "@timeout")]
    pub(crate) timeout: Option<Scalar>,
    #[serde(
        default,
        alias = "@bus_speed",
        alias = "bus-speed",
        alias = "@bus-speed"
    )]
    pub(crate) bus_speed: Option<Scalar>,
    // SPI attributes.
    #[serde(
        default,
        alias = "@max_speed",
        alias = "max-speed",
        alias = "@max-speed"
    )]
    pub(crate) max_speed: Option<Scalar>,
    #[serde(default, alias = "@mode")]
    pub(crate) mode: Option<Scalar>,
    #[serde(
        default,
        alias = "@bits_per_word",
        alias = "bits-per-word",
        alias = "@bits-per-word"
    )]
    pub(crate) bits_per_word: Option<Scalar>,
    #[serde(
        default,
        alias = "@bit_order",
        alias = "bit-order",
        alias = "@bit-order"
    )]
    pub(crate) bit_order: Option<Scalar>,
    #[serde(
        default,
        alias = "@cs_active",
        alias = "cs-active",
        alias = "@cs-active"
    )]
    pub(crate) cs_active: Option<Scalar>,
    // Shared: every type of group may state a packet size.
    #[serde(
        default,
        alias = "@packet_size",
        alias = "packet-size",
        alias = "@packet-size"
    )]
    pub(crate) packet_size: Option<Scalar>,
    // Shared: stream payload protocol attributes.
    #[serde(default)]
    pub(crate) stream: Option<StreamWire>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StreamWire {
    #[serde(
        default,
        alias = "@max_length",
        alias = "max-length",
        alias = "@max-length"
    )]
    pub(crate) max_length: Option<Scalar>,
    #[serde(default, alias = "@timeout")]
    pub(crate) timeout: Option<Scalar>,
    #[serde(
        default,
        alias = "@terminators",
        alias = "terminator",
        alias = "@terminator"
    )]
    pub(crate) terminators: Option<ByteList>,
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
    #[serde(default, alias = "@dh_id", alias = "dh-id", alias = "@dh-id")]
    dh_id: Option<Scalar>,
    #[serde(
        default,
        alias = "@oc_address",
        alias = "oc-address",
        alias = "@oc-address"
    )]
    oc_address: Option<String>,
    #[serde(default, alias = "@oc_port", alias = "oc-port", alias = "@oc-port")]
    oc_port: Option<Scalar>,
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
pub(crate) struct Scalar(String);

impl Scalar {
    pub(crate) fn as_str(&self) -> &str {
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
pub(crate) struct ByteList(Vec<String>);

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
    let mut used: BTreeSet<String> = BTreeSet::new();

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
        used.insert(e.group.clone());
        endpoints.push(validate_endpoint(e, &groups[idx])?);
    }

    // A group no endpoint is in has no effect on the configuration, which is
    // also what a group whose name an endpoint misspelled looks like. Checked
    // after the endpoints, so that a misspelling is reported from the
    // endpoint's end, where the name actually is.
    if let Some(group) = groups.iter().find(|g| !used.contains(&g.name)) {
        return Err(EndpointConfigError::UnusedGroup(group.name.clone()));
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

    // Shared across the four types, so it is read before the match and is
    // not listed among the type-specific fields any type would reject.
    let packet_size = match g.packet_size.as_ref() {
        Some(s) => {
            let bytes = parse_u32(&name, "packet_size", s)?;
            if bytes == 0 {
                return Err(bad(&name, "packet_size must be greater than zero"));
            }
            Some(bytes)
        }
        None => None,
    };

    // One arm per kind of group, each in the file that owns that kind: what
    // a group of that kind may say, what it must say, and what the values it
    // gives mean are all one subject, and not this function's.
    let kind = match kind_text.as_str() {
        "serial" => crate::endpoint_config_serial::group_kind_of(&name, g)?,
        "network" => crate::endpoint_config_network::group_kind_of(&name, g)?,
        "i2c" => crate::endpoint_config_i2c::group_kind_of(&name, g)?,
        "spi" => crate::endpoint_config_spi::group_kind_of(&name, g)?,
        other => {
            return Err(EndpointConfigError::UnknownGroupKind {
                group: name,
                kind: other.to_string(),
            })
        }
    };

    Ok(EndpointGroup {
        name,
        packet_size,
        kind,
    })
}

/// Apply the three rules governing a stream section.
pub(crate) fn validate_stream(group: &str, s: StreamWire) -> EndpointConfigResult<StreamParams> {
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
    /// A path alone, whatever kind of endpoint is at the end of it.
    Path,
    /// A host and a port.
    Network,
    /// A bus device and the address of a device on that bus.
    I2cBusAndAddress,
}

fn validate_endpoint(e: EndpointWire, group: &EndpointGroup) -> EndpointConfigResult<EndpointDef> {
    let kind = group.kind.type_name();

    let dh_id = match e.dh_id.as_ref() {
        Some(s) => Some(parse_u32(&group.name, "dh_id", s)?),
        None => None,
    };

    let oc_port = match e.oc_port.as_ref() {
        Some(s) => {
            let port = parse_u32(&group.name, "oc_port", s)?;
            if port > u16::MAX as u32 {
                return Err(bad(&group.name, &format!("oc_port {port} is out of range")));
            }
            Some(port as u16)
        }
        None => None,
    };

    // Half an OC address reaches nothing and says nothing about which half was
    // meant, so it is an error rather than a handler with no OC side.
    let oc = match (e.oc_address.clone(), oc_port) {
        // The OC link is UDP whatever the payload side of the handler is.
        (Some(address), Some(port)) => Some(NetworkConfig {
            protocol: NetworkProtocol::Udp,
            address,
            port,
        }),
        (None, None) => None,
        (Some(_), None) => {
            return Err(bad(
                &group.name,
                &format!("endpoint \"{}\": oc_address without oc_port", e.name),
            ))
        }
        (None, Some(_)) => {
            return Err(bad(
                &group.name,
                &format!("endpoint \"{}\": oc_port without oc_address", e.name),
            ))
        }
    };

    let shape = match &group.kind {
        GroupKind::Serial(_) => LocationShape::Path,
        // A SPI device node names the bus and the chip select together.
        GroupKind::Spi(_) => LocationShape::Path,
        GroupKind::I2c(_) => LocationShape::I2cBusAndAddress,
        // A Unix-domain socket is named by a path, not by host and port.
        GroupKind::Network(n) => {
            if matches!(
                n.protocol,
                NetworkProtocol::UnixStream | NetworkProtocol::UnixDgram
            ) {
                LocationShape::Path
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
            dh_id,
            oc,
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
        EndpointLocation::Path { path }
    };

    Ok(EndpointDef {
        name: e.name,
        group: e.group,
        location,
        dh_id,
        oc,
    })
}

// ---------------------------------------------------------------------------
// Scalar parsers
// ---------------------------------------------------------------------------

pub(crate) fn bad(group: &str, message: &str) -> EndpointConfigError {
    EndpointConfigError::BadGroupValue {
        group: group.to_string(),
        message: message.to_string(),
    }
}

pub(crate) fn require<'a, T>(
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
pub(crate) fn reject_foreign_fields(
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
pub(crate) fn reject_stream(group: &str, kind: &'static str, present: bool) -> EndpointConfigResult<()> {
    reject_unused(group, kind, "a stream section", present)
}

/// Parse an unsigned value written in decimal, or in hex with an `0x` prefix.
pub(crate) fn parse_u32(group: &str, field: &str, s: &Scalar) -> EndpointConfigResult<u32> {
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

/// Parse a flag written `true` or `false`.
///
/// A YAML 1.1 reader folds `yes`, `no`, `on`, and `off` to a boolean before
/// this sees them, and [`Scalar`] puts the result back as `true` or `false`,
/// so those spellings work too without being named here.
pub(crate) fn parse_flag(group: &str, field: &str, s: Option<&Scalar>) -> EndpointConfigResult<Option<bool>> {
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

/// Parse a timeout: the word `none`, or a whole number with a unit suffix of
/// `us`, `ms`, or `s`.
///
/// `Ok(None)` is the word `none`: a read that does not time out.
pub(crate) fn parse_timeout(group: &str, s: &Scalar) -> EndpointConfigResult<Option<Duration>> {
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
    use crate::types::DeviceConfig;

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
            EndpointLocation::Path {
                path: "/dev/ttyS0".to_string()
            }
        );
        assert_eq!(
            serial[1].location,
            EndpointLocation::Path {
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

    /// An endpoints section putting one endpoint in group `g`.
    ///
    /// A group no endpoint is in is rejected, so a test of group parsing has
    /// to put something in the group it parses. What an endpoint must give
    /// depends on its group's type, so there is one of these per shape.
    const DEVICE_ENDPOINT: &str = "endpoints:\n  - name: e\n    group: g\n    \
                                   device: /dev/ttyS0\n";
    const NETWORK_ENDPOINT: &str = "endpoints:\n  - name: e\n    group: g\n    \
                                    address: 192.168.1.10\n    port: 5000\n";
    const I2C_ENDPOINT: &str = "endpoints:\n  - name: e\n    group: g\n    \
                                device: /dev/i2c-1\n    address: 0x40\n";

    fn serial_stream(stream_body: &str) -> EndpointConfigResult<EndpointConfigDoc> {
        from_yaml_str(&format!(
            "endpoint_groups:\n  \
             - name: g\n    \
               type: serial\n    \
               datarate: 9600\n    \
               stop_bits: 1\n    \
               byte_length: 8\n    \
               stream:\n{stream_body}{DEVICE_ENDPOINT}"
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

    /// A one-group file of the given type, with `extra` folded into the group.
    fn group_of_type(kind: &str, extra: &str) -> EndpointConfigResult<EndpointConfigDoc> {
        let body = match kind {
            "serial" => {
                "    datarate: 9600\n    stop_bits: 1\n    byte_length: 8\n\
                 \x20   stream:\n      max_length: 8\n      timeout: none\n"
            }
            "network" => "    protocol: udp\n",
            "i2c" => "",
            _ => "    max_speed: 1000000\n    mode: 0\n",
        };
        let endpoint = match kind {
            "network" => NETWORK_ENDPOINT,
            "i2c" => I2C_ENDPOINT,
            // A serial line and a SPI chip select are both named by a device.
            _ => DEVICE_ENDPOINT,
        };
        from_yaml_str(&format!(
            "endpoint_groups:\n  - name: g\n    type: {kind}\n{body}{extra}{endpoint}"
        ))
    }

    #[test]
    fn packet_size_is_read_for_every_type_of_group() {
        // It is a shared attribute, so no type may treat it as foreign.
        for kind in ["serial", "network", "i2c", "spi"] {
            let doc = group_of_type(kind, "    packet_size: 128\n")
                .unwrap_or_else(|e| panic!("{kind}: {e}"));
            assert_eq!(doc.group("g").unwrap().packet_size, Some(128), "{kind}");
        }
    }

    #[test]
    fn packet_size_is_optional() {
        // A file describing only how to reach a device need not state one.
        for kind in ["serial", "network", "i2c", "spi"] {
            let doc = group_of_type(kind, "").unwrap_or_else(|e| panic!("{kind}: {e}"));
            assert_eq!(doc.group("g").unwrap().packet_size, None, "{kind}");
        }
    }

    #[test]
    fn a_zero_packet_size_is_rejected() {
        // A packet of no bytes is not a packet, and the other sizes and rates
        // in this format reject zero for the same reason.
        let e = group_of_type("network", "    packet_size: 0\n").unwrap_err();
        assert!(format!("{e}").contains("greater than zero"), "got {e}");
    }

    #[test]
    fn packet_size_accepts_hex_and_a_hyphenated_name() {
        let hex = group_of_type("network", "    packet_size: 0x80\n").unwrap();
        let hyphen = group_of_type("network", "    packet-size: 128\n").unwrap();
        assert_eq!(hex.group("g").unwrap().packet_size, Some(128));
        assert_eq!(hyphen.group("g").unwrap().packet_size, Some(128));
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
                 timeout: none\n{DEVICE_ENDPOINT}"
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
    fn a_group_no_endpoint_is_in_is_rejected() {
        // A group with no members has no effect on the configuration, so a
        // file carrying one is more likely wrong than deliberate.
        let yaml = format!(
            "endpoint_groups:\n  - name: g\n    type: network\n    protocol: udp\n\
             \x20 - name: spare\n    type: network\n    protocol: udp\n{NETWORK_ENDPOINT}"
        );
        let e = from_yaml_str(&yaml).unwrap_err();
        assert!(
            matches!(&e, EndpointConfigError::UnusedGroup(name) if name == "spare"),
            "got {e:?}"
        );
    }

    #[test]
    fn a_misspelled_group_is_reported_from_the_endpoint_not_the_group() {
        // A typo leaves the group unused and the name undefined at once. The
        // endpoint's end is where the misspelling actually is, so that is the
        // error worth giving.
        let yaml = "endpoint_groups:\n  - name: payload_udp\n    type: network\n    \
                    protocol: udp\nendpoints:\n  - name: e\n    group: payload_upd\n    \
                    address: 192.168.1.10\n    port: 5000\n";
        let e = from_yaml_str(yaml).unwrap_err();
        assert!(
            matches!(&e, EndpointConfigError::UnknownGroup { group, .. } if group == "payload_upd"),
            "got {e:?}"
        );
    }

    // -- endpoints as data handlers -----------------------------------------

    /// A whole set of endpoints becoming data handlers.
    ///
    /// The transports come from the groups and the addresses from the
    /// endpoints, which is the division the format exists for.
    #[test]
    fn endpoints_become_data_handlers() {
        let doc = from_yaml_str(
            "endpoint_groups:\n  \
             - name: payload_tcp\n    type: network\n    protocol: tcp\n    \
               packet_size: 1024\n    stream:\n      max_length: 1024\n      \
               timeout: none\n  \
             - name: payload_unix\n    type: network\n    protocol: unix_dgram\n    \
               packet_size: 64\n  \
             - name: rs422\n    type: serial\n    datarate: 9600\n    stop_bits: 1\n    \
               byte_length: 8\n    packet_size: 512\n    stream:\n      \
               max_length: 512\n      timeout: none\n\
             endpoints:\n  \
             - name: camera\n    group: payload_tcp\n    dh_id: 0\n    \
               address: 192.168.1.10\n    port: 5000\n  \
             - name: recorder\n    group: payload_unix\n    dh_id: 1\n    \
               device: /run/tcs/recorder.sock\n  \
             - name: magnetometer\n    group: rs422\n    dh_id: 2\n    \
               device: /dev/ttyS0\n",
        )
        .expect("parses");

        let handlers = doc.to_dh_configs().expect("converts");
        assert_eq!(handlers.len(), 3);

        assert_eq!(handlers[0].dh_id, DHId(0));
        assert_eq!(handlers[0].name, DHName::new("camera"));
        assert_eq!(handlers[0].packet_size, 1024);
        assert_eq!(
            handlers[0].endpoint,
            EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Tcp,
                address: "192.168.1.10".to_string(),
                port: 5000,
            })
        );

        // A Unix socket is a network endpoint named by a path, with no port.
        assert_eq!(
            handlers[1].endpoint,
            EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::UnixDgram,
                address: "/run/tcs/recorder.sock".to_string(),
                port: 0,
            })
        );
        assert_eq!(handlers[1].packet_size, 64);

        // A serial line is its own kind of endpoint: a device node, and the
        // framing of the line it opens. It used to be a plain device, which
        // is the right file read at whatever rate the port was left at.
        assert_eq!(
            handlers[2].endpoint,
            EndpointConfig::Serial(SerialConfig {
                path: "/dev/ttyS0".to_string(),
                datarate: 9600,
                stop_bits: StopBits::One,
                byte_length: 8,
            })
        );
    }

    /// A SPI endpoint is named by a device node -- which names the bus and
    /// the chip select together -- and carries the terms it is clocked on.
    #[test]
    fn a_spi_endpoint_becomes_a_spi_handler_with_its_terms() {
        let doc = from_yaml_str(
            "endpoint_groups:\n  - name: g\n    type: spi\n    max_speed: 1000000\n    \
             mode: 0\n    packet_size: 32\n\
             endpoints:\n  - name: imu\n    group: g\n    dh_id: 7\n    \
             device: /dev/spidev0.0\n",
        )
        .expect("parses");

        let handlers = doc.to_dh_configs().expect("converts");
        assert_eq!(handlers[0].dh_id, DHId(7));
        assert_eq!(
            handlers[0].endpoint,
            EndpointConfig::Spi(SpiConfig {
                path: "/dev/spidev0.0".to_string(),
                max_speed: 1_000_000,
                mode: SpiMode::Mode0,
                bits_per_word: 8,
                bit_order: BitOrder::MsbFirst,
                cs_active: CsActive::Low,
            }),
            "a SPI endpoint used to become a plain device handler, which opened \
             the node and then clocked it however it had been left"
        );
    }

    /// An I2C endpoint becomes a handler carrying both halves of where it is.
    ///
    /// It used to become nothing at all: there was no `EndpointConfig` that
    /// could hold a bus and an address, so the conversion refused, and an I2C
    /// bus could be described in a file and never run.
    #[test]
    fn an_i2c_endpoint_becomes_an_i2c_handler_with_bus_and_address() {
        let doc = from_yaml_str(
            "endpoint_groups:\n  - name: g\n    type: i2c\n    packet_size: 8\n    \
             pec: true\n\
             endpoints:\n  - name: thermal_a\n    group: g\n    dh_id: 0\n    \
             device: /dev/i2c-1\n    address: 0x48\n",
        )
        .expect("parses: an I2C endpoint is a perfectly good endpoint");

        let handlers = doc.to_dh_configs().expect("converts");
        assert_eq!(
            handlers[0].endpoint,
            EndpointConfig::I2c(I2cConfig {
                bus: "/dev/i2c-1".to_string(),
                address: 0x48,
                ten_bit: false,
                pec: true,
            })
        );
    }

    #[test]
    fn an_endpoint_with_no_dh_id_cannot_become_a_data_handler() {
        let doc = from_yaml_str(
            "endpoint_groups:\n  - name: g\n    type: network\n    protocol: udp\n    \
             packet_size: 8\n\
             endpoints:\n  - name: e\n    group: g\n    address: 10.0.0.1\n    \
             port: 5000\n",
        )
        .expect("parses: an id is only needed to become a handler");

        let e = doc.to_dh_configs().unwrap_err();
        assert!(
            matches!(&e, EndpointConfigError::EndpointHasNoDhId(name) if name == "e"),
            "got {e:?}"
        );
    }

    #[test]
    fn two_endpoints_sharing_a_dh_id_are_rejected() {
        // A duplicate converts cleanly and then has one handler shadow the
        // other, which is what the payload format rejects as well.
        let doc = from_yaml_str(
            "endpoint_groups:\n  - name: g\n    type: network\n    protocol: udp\n    \
             packet_size: 8\n\
             endpoints:\n  - name: first\n    group: g\n    dh_id: 3\n    \
             address: 10.0.0.1\n    port: 5000\n  \
             - name: second\n    group: g\n    dh_id: 3\n    address: 10.0.0.2\n    \
             port: 5001\n",
        )
        .expect("parses");

        let e = doc.to_dh_configs().unwrap_err();
        assert!(
            matches!(&e, EndpointConfigError::DuplicateDhId { dh_id: 3, .. }),
            "got {e:?}"
        );
    }

    #[test]
    fn an_endpoint_whose_group_states_no_packet_size_cannot_become_a_handler() {
        // The group attribute is optional, because a file saying only how to
        // reach a device need not state one. A data handler must have one.
        let doc = from_yaml_str(
            "endpoint_groups:\n  - name: g\n    type: network\n    protocol: udp\n\
             endpoints:\n  - name: e\n    group: g\n    dh_id: 0\n    \
             address: 10.0.0.1\n    port: 5000\n",
        )
        .expect("parses: packet_size is optional");

        let e = doc.to_dh_configs().unwrap_err();
        assert!(
            matches!(
                &e,
                EndpointConfigError::GroupHasNoPacketSize { endpoint, group }
                    if endpoint == "e" && group == "g"
            ),
            "got {e:?}"
        );
    }

    #[test]
    fn a_dh_id_is_read_from_every_format_and_either_spelling() {
        let yaml = "endpoint_groups:\n  - name: g\n    type: network\n    protocol: udp\n    \
                    packet_size: 8\n\
                    endpoints:\n  - name: e\n    group: g\n    dh_id: 0x2a\n    \
                    address: 10.0.0.1\n    port: 5000\n";
        let hyphen = yaml.replace("dh_id", "dh-id");
        // XML carries values as attributes, which reach serde with an `@`
        // prefix -- so this exercises the `@dh_id` alias rather than `dh_id`.
        let xml = r#"<endpoint-configuration>
                       <endpoint-groups>
                         <group name="g" type="network" protocol="udp" packet_size="8"/>
                       </endpoint-groups>
                       <endpoints>
                         <endpoint name="e" group="g" dh_id="42"
                                   address="10.0.0.1" port="5000"/>
                       </endpoints>
                     </endpoint-configuration>"#;
        let json = r#"{"endpoint_groups":[{"name":"g","type":"network","protocol":"udp",
                       "packet_size":8}],
                       "endpoints":[{"name":"e","group":"g","dh_id":42,
                       "address":"10.0.0.1","port":5000}]}"#;

        // 0x2a is 42: the id accepts hex as every other number in this format
        // does.
        for (label, doc) in [
            ("yaml", from_yaml_str(yaml).unwrap()),
            ("hyphenated", from_yaml_str(&hyphen).unwrap()),
            ("xml", from_xml_str(xml).unwrap()),
            ("json", from_str(json, ConfigFormat::Json).unwrap()),
        ] {
            assert_eq!(doc.endpoints[0].dh_id, Some(42), "{label}");
            assert_eq!(doc.to_dh_configs().unwrap()[0].dh_id, DHId(42), "{label}");
        }
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
        let doc = from_yaml_str(&format!(
            "endpoint_groups:\n  - name: g\n    type: network\n    protocol: udp\n\
             {NETWORK_ENDPOINT}"
        ))
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
            EndpointLocation::Path {
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
        let a = from_yaml_str(&format!(
            "endpoint_groups:\n  - name: g\n    type: serial\n    datarate: 9600\n    \
             stop_bits: 1\n    byte_length: 8\n    stream:\n      max_length: 8\n      \
             timeout: none\n{DEVICE_ENDPOINT}"
        ))
        .unwrap();
        let b = from_yaml_str(&format!(
            "endpoint-groups:\n  - name: g\n    type: serial\n    datarate: 9600\n    \
             stop-bits: 1\n    byte-length: 8\n    stream:\n      max-length: 8\n      \
             timeout: none\n{DEVICE_ENDPOINT}"
        ))
        .unwrap();
        assert_eq!(a, b);
    }
}
