//! The rules of a link: a serial line, an I2C bus, a SPI peripheral.
//!
//! What terms each kind of link takes, which of them it must be told, what
//! each means, and which belong to another kind. A payload file states them
//! -- see `types::link_params_of`, which is the only caller -- and the rules
//! are here, once, rather than beside each file format that can state them.
//!
//! There was a second file format that stated them: an endpoint
//! configuration, with a general section, groups of endpoints, and the
//! endpoints in them. It described the same payloads in other words, this
//! module parsed it, and the rules were reachable only through it. It is
//! gone; a payload states what an endpoint and its group stated between them.
//! What is left here is the part that was never about the file: the terms
//! ([`LinkTerms`]), what they mean once settled ([`GroupKind`] and the
//! per-kind parameters), where a payload is reached ([`EndpointLocation`]),
//! and the scalar spellings a file may use ([`Scalar`], [`ByteList`]).
//!
//! The syntax a payload file states them in is specified in
//! `docs/design.rst`, under "Payload Configuration Files".

use std::fmt;
use std::time::Duration;

use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::endpoint_config_i2c::I2cParams;
use crate::endpoint_config_serial::SerialParams;
use crate::endpoint_config_spi::SpiParams;

// The per-kind types are named from here as well as from their own files, so
// that a reader of a configuration document reaches everything it can hold
// through the module that parses it.
pub use crate::endpoint_config_i2c::I2cParams as I2cGroupParams;
pub use crate::endpoint_config_serial::{ByteLength, StopBits};
pub use crate::endpoint_config_spi::{BitOrder, BitsPerWord, CsActive, SpiMode};
use crate::types::{EndpointConfig, I2cConfig, SerialConfig, SpiConfig};
use crate::TcsError;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// What can be wrong with the terms a payload gave its link.
///
/// Parse errors come from the format's own reader; everything else is a rule
/// this module enforces after a syntactically valid file has been read.
#[derive(Error, Debug)]
pub enum EndpointConfigError {
    #[error("payload \"{group}\", on a {kind} link, has no {field}")]
    MissingGroupField {
        group: String,
        kind: &'static str,
        field: &'static str,
    },

    #[error("payload \"{group}\" is on a {kind} link, so {field} does not apply to it")]
    UnusedGroupField {
        group: String,
        kind: &'static str,
        field: &'static str,
    },

    #[error("payload \"{0}\": a stream section must give max_length")]
    StreamMissingMaxLength(String),

    #[error(
        "payload \"{0}\": a stream section must give a timeout or a terminator list, or \
         both. To read a fixed number of bytes with neither, say timeout: none explicitly"
    )]
    StreamNeedsTimeoutOrTerminators(String),

    #[error(
        "payload \"{0}\": an empty terminator list is not a terminator list; omit it or \
         give at least one byte"
    )]
    StreamEmptyTerminators(String),

    #[error("payload \"{group}\": {message}")]
    BadGroupValue { group: String, message: String },
}

/// The answer to asking the rules about a link.
pub type EndpointConfigResult<T> = Result<T, EndpointConfigError>;

impl From<EndpointConfigError> for TcsError {
    fn from(e: EndpointConfigError) -> Self {
        TcsError::Config(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Domain types -- what a caller gets back
// ---------------------------------------------------------------------------

/// What tcspecial opens to reach a payload, from where it is and what kind of
/// link it is on.
///
/// The same two facts whichever language described the payload: a payload
/// configuration states them as attributes of one payload, and an endpoint
/// configuration splits them between an endpoint and its group. `what` is the
/// payload's name, for the complaints.
pub(crate) fn endpoint_of(
    what: &str,
    location: &EndpointLocation,
    kind: &GroupKind,
) -> Result<EndpointConfig, String> {
    match location {
        EndpointLocation::I2c { bus, address } => match kind {
            GroupKind::I2c(i2c) => Ok(EndpointConfig::I2c(I2cConfig {
                bus: bus.clone(),
                address: *address,
                ten_bit: i2c.ten_bit,
                pec: i2c.pec,
            })),
            // Only an I2C group gives an endpoint a bus and an address, so
            // this is unreachable through the parser; an error rather than a
            // panic, because a library should not bring a caller down.
            _ => Err(format!(
                "\"{what}\" is on a bus, which a {} link does not put it on",
                kind.type_name()
            )),
        },
        // Every kind but a network address is located by a device node, and
        // which kind it is decides what else goes with it. They used all to
        // become a plain Device, which opened the right file and then talked
        // to it as though it had no terms of its own: a serial line at
        // whatever rate the port was last left at, a SPI peripheral at
        // whatever mode.
        EndpointLocation::Path { path } => match kind {
            GroupKind::Serial(serial) => Ok(EndpointConfig::Serial(SerialConfig {
                path: path.clone(),
                datarate: serial.datarate,
                asynchronous: serial.asynchronous,
                stop_bits: serial.stop_bits,
                parity: serial.parity,
                clock_type: serial.clock_type,
                encoding: serial.encoding,
                frame_check: serial.frame_check,
                loopback: serial.loopback,
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
            GroupKind::I2c(_) => Err(format!(
                "\"{what}\" names a device alone, but a device on an I2C bus needs the \
                 bus and the address on it"
            )),
        },
    }
}

/// Per-type group attributes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum GroupKind {
    Serial(SerialParams),
    I2c(I2cParams),
    Spi(SpiParams),
}

impl GroupKind {
    /// The name this type goes by in a configuration file.
    pub fn type_name(&self) -> &'static str {
        match self {
            GroupKind::Serial(_) => "serial",
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
    /// A bus device and the address of one device on that bus.
    ///
    /// Both are needed because two endpoints of one I2C group commonly sit
    /// on the same bus and differ only in which device the master addresses.
    I2c { bus: String, address: u16 },
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Wire types -- one set, both formats
// ---------------------------------------------------------------------------

/// The terms of a link, as a payload and the group it names state them
/// between them.
///
/// One struct for every kind, each kind reading the ones that are its own and
/// refusing the ones that are not -- which is how a term written on the wrong
/// kind of payload is reported rather than ignored. The terms arrive as text
/// [`Scalar`]s because a file may spell a number in decimal or hexadecimal
/// and a flag as a word; what each means is settled by the rules for the
/// kind.
///
/// It was a wire type once: an endpoint configuration file deserialized
/// straight into it, which is why every field carried an XML `@` alias and a
/// hyphenated spelling beside its own. Nothing deserializes into it now -- a
/// payload file has its own shape, and `types::link_params_of` fills this in
/// from a payload -- so the aliases are gone and so is the name.
#[derive(Debug)]
pub(crate) struct LinkTerms {
    // A line.
    pub(crate) datarate: Option<Scalar>,
    pub(crate) stop_bits: Option<Scalar>,
    pub(crate) asynchronous: Option<Scalar>,
    /// A parity per character on an asynchronous line, and the frame check on
    /// a synchronous one: the kernel calls both a line's parity.
    pub(crate) parity: Option<Scalar>,
    pub(crate) clock_type: Option<Scalar>,
    pub(crate) encoding: Option<Scalar>,
    pub(crate) loopback: Option<Scalar>,
    pub(crate) byte_length: Option<Scalar>,
    /// Which protocol a network payload speaks, which no link has: here so
    /// that a link stating one is told so.
    pub(crate) protocol: Option<String>,
    // A bus.
    pub(crate) ten_bit: Option<Scalar>,
    pub(crate) pec: Option<Scalar>,
    pub(crate) retries: Option<Scalar>,
    pub(crate) timeout: Option<Scalar>,
    pub(crate) bus_speed: Option<Scalar>,
    // A peripheral on one.
    pub(crate) max_speed: Option<Scalar>,
    /// Which of the four SPI modes a peripheral is clocked in.
    ///
    /// Named `spi_mode` because a payload already has a `mode` -- whether it
    /// sends on its own or on a trigger -- and one word cannot mean both.
    pub(crate) spi_mode: Option<Scalar>,
    pub(crate) bits_per_word: Option<Scalar>,
    pub(crate) bit_order: Option<Scalar>,
    pub(crate) cs_active: Option<Scalar>,
    /// Where one read of a line ends. A line's alone: see
    /// [`GroupKind::stream`].
    pub(crate) stream: Option<StreamTerms>,
}

/// Where one read of a line ends, as a file states it.
#[derive(Debug)]
pub(crate) struct StreamTerms {
    pub(crate) max_length: Option<Scalar>,
    pub(crate) timeout: Option<Scalar>,
    pub(crate) terminators: Option<ByteList>,
}

// ---------------------------------------------------------------------------
// Shapes that differ between the two formats
// ---------------------------------------------------------------------------

/// A scalar as the file spelled it.
///
/// Every value in an XML attribute is text, and YAML distinguishes numbers
/// from strings, so a field that is conceptually a number arrives as either.
/// Both are kept as text and parsed by the rule for that field, which gives
/// one spelling of each rule and one wording of each error.
#[derive(Debug, Clone)]
pub struct Scalar(String);

impl Scalar {
    pub(crate) fn as_str(&self) -> &str {
        self.0.trim()
    }
}

/// Compared and written out as the text it holds, trimmed.
///
/// A payload configuration carries these, and a payload configuration is
/// compared against another spelling of itself and written back out for the
/// configuration digest. Two files that spell one number differently --
/// `0x1E` and `30` -- are two different texts here and so digest
/// differently, which is the price of a rule that reads a value by the rule
/// for its own field rather than by its syntax.
impl PartialEq for Scalar {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Serialize for Scalar {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
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

            /// An XML element with text in it.
            ///
            /// A value given as an XML attribute arrives as a string, and one
            /// given as a child element arrives as a map of one entry whose
            /// value is the text -- `$text`, as quick-xml spells it. The
            /// payload configuration language writes every value as a child
            /// element, so this is the form it comes in there; the endpoint
            /// language wrote them as attributes, which is why this was not
            /// needed before.
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Scalar, A::Error> {
                let mut text: Option<String> = None;
                while let Some((_, value)) = map.next_entry::<String, String>()? {
                    text = Some(value);
                }
                text.map(Scalar)
                    .ok_or_else(|| de::Error::custom("an element with no value in it"))
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
pub struct ByteList(Vec<String>);

/// Compared and written out as the values it holds, for the reason
/// [`Scalar`] is.
impl PartialEq for ByteList {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Serialize for ByteList {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

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

            /// An XML element with text in it; see [`Scalar`]'s.
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<ByteList, A::Error> {
                let mut text: Option<String> = None;
                while let Some((_, value)) = map.next_entry::<String, String>()? {
                    text = Some(value);
                }
                text.map(|t| ByteList(split_list(&t)))
                    .ok_or_else(|| de::Error::custom("an element with no value in it"))
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

/// Apply the three rules governing a stream section.
pub(crate) fn validate_stream(group: &str, s: StreamTerms) -> EndpointConfigResult<StreamParams> {
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
fn type_specific_fields(g: &LinkTerms) -> [(&'static str, bool); 19] {
    [
        // Serial.
        ("datarate", g.datarate.is_some()),
        ("stop_bits", g.stop_bits.is_some()),
        ("asynchronous", g.asynchronous.is_some()),
        ("parity", g.parity.is_some()),
        ("clock_type", g.clock_type.is_some()),
        ("encoding", g.encoding.is_some()),
        ("loopback", g.loopback.is_some()),
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
        ("spi_mode", g.spi_mode.is_some()),
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
    g: &LinkTerms,
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
pub(crate) fn reject_stream(
    group: &str,
    kind: &'static str,
    present: bool,
) -> EndpointConfigResult<()> {
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
pub(crate) fn parse_flag(
    group: &str,
    field: &str,
    s: Option<&Scalar>,
) -> EndpointConfigResult<Option<bool>> {
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
