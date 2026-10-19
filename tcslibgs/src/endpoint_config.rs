//! The rules of a link: a serial line, an I2C bus, a SPI peripheral.
//!
//! What terms each kind of link takes, which of them it must be told, what each
//! means, and which belong to another kind. A node of a configuration states
//! them as its attributes -- see `crate::graph::reach`, which gathers them and
//! asks `types::kind_of_terms`, the only caller -- and the rules are here,
//! once, rather than beside each file format that can state them.
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
//! The syntax a tcspecial configuration states them in is specified in
//! `docs/design.rst`, under "Payload Configuration Files".

use std::fmt;
use std::time::Duration;

use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::node::payload::can::CanParams;
use crate::node::payload::iio::IioParams;
use crate::node::payload::gpio::GpioParams;
use crate::node::payload::rpmsg::RpmsgParams;
use crate::node::payload::i2c::I2cParams;
use crate::node::payload::serial::SerialParams;
use crate::node::payload::spi::SpiParams;

// The per-kind types are named from here as well as from their own files, so
// that a reader of a configuration document reaches everything it can hold
// through the module that parses it.
pub use crate::node::payload::i2c::I2cParams as I2cGroupParams;
pub use crate::node::payload::can::CanParams as CanGroupParams;
pub use crate::node::payload::iio::IioParams as IioGroupParams;
pub use crate::node::payload::gpio::GpioParams as GpioGroupParams;
pub use crate::node::payload::rpmsg::RpmsgParams as RpmsgGroupParams;
pub use crate::node::payload::serial::rs485::Rs485Params;
pub use crate::node::payload::serial::{ByteLength, StopBits};
pub use crate::node::payload::spi::{BitOrder, BitsPerWord, CsActive, SpiMode};
use crate::types::{
    CanConfig, EndpointConfig, GpioConfig, I2cConfig, IioConfig, RpmsgConfig, SerialConfig,
    SpiConfig,
};
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
        // A synchronous line, which the kernel drives as a network interface:
        // the interface is the whole of where it is, there being no address
        // on it -- a frame carries none, so one line is one payload.
        EndpointLocation::Interface { interface } => match kind {
            GroupKind::Hdlc(hdlc) => Ok(EndpointConfig::Hdlc(crate::types::HdlcConfig {
                interface: interface.clone(),
                standard: hdlc.standard,
                clock_type: hdlc.clock_type,
                clock_rate: hdlc.clock_rate,
                loopback: hdlc.loopback,
                encoding: hdlc.encoding,
                frame_check: hdlc.frame_check,
            })),
            // Only an hdlc payload is located by an interface alone, so this
            // is unreachable through the parser; an error rather than a
            // panic, for the reason the arms below give.
            _ => Err(format!(
                "\"{what}\" is on an interface, which a {} link does not put it on",
                kind.type_name()
            )),
        },
        EndpointLocation::Can { interface, can_id } => match kind {
            GroupKind::Can(can) => Ok(EndpointConfig::Can(CanConfig {
                interface: interface.clone(),
                can_id: *can_id,
                fd: can.fd,
                loopback: can.loopback,
            })),
            // Only a CAN payload is located on an interface, so this is
            // unreachable through the parser; an error rather than a panic,
            // for the reason the bus arm above gives.
            _ => Err(format!(
                "\"{what}\" is on a CAN interface, which a {} link does not put it on",
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
                stop_bits: serial.stop_bits,
                parity: serial.parity,
                byte_length: serial.byte_length.bits(),
                rs485: serial.rs485,
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
            // shape it cannot produce. A CAN payload is on an interface and
            // not a path at all, for the same reason.
            GroupKind::I2c(_) => Err(format!(
                "\"{what}\" names a device alone, but a device on an I2C bus needs the \
                 bus and the address on it"
            )),
            GroupKind::Iio(_) => Ok(EndpointConfig::Iio(IioConfig {
                path: path.clone(),
            })),
            GroupKind::Gpio(gpio) => Ok(EndpointConfig::Gpio(GpioConfig {
                path: path.clone(),
                lines: gpio.lines.clone(),
                edge: gpio.edge,
                bias: gpio.bias,
                debounce: gpio.debounce,
                state_interval: gpio.state_interval,
                direction: gpio.direction,
            })),
            GroupKind::Rpmsg(rpmsg) => Ok(EndpointConfig::Rpmsg(RpmsgConfig {
                path: path.clone(),
                service: rpmsg.service.clone(),
                max_message_bytes: rpmsg.max_message_bytes,
            })),
            GroupKind::Can(_) => Err(format!(
                "\"{what}\" names a device file, but a CAN payload is on an interface \
                 and the identifier its frames carry"
            )),
            // A synchronous line is a network interface and not a device
            // file, for the reason a CAN controller is not one.
            GroupKind::Hdlc(_) => Err(format!(
                "\"{what}\" names a device file, but a synchronous line is a network \
                 interface: say interface, not path"
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
    /// Experimental; see [`crate::node::payload::can`].
    Can(CanParams),
    /// Experimental; see [`crate::node::payload::iio`].
    Iio(IioParams),
    /// Experimental; see [`crate::node::payload::rpmsg`].
    Rpmsg(RpmsgParams),
    /// Experimental; see [`crate::node::payload::gpio`].
    Gpio(GpioParams),
    /// Experimental; see [`crate::node::payload::hdlc`].
    Hdlc(crate::node::payload::hdlc::HdlcParams),
}

impl GroupKind {
    /// The name this type goes by in a configuration file.
    pub fn type_name(&self) -> &'static str {
        match self {
            GroupKind::Serial(_) => "serial",
            GroupKind::I2c(_) => "i2c",
            GroupKind::Spi(_) => "spi",
            GroupKind::Can(_) => "can",
            GroupKind::Iio(_) => "iio",
            GroupKind::Rpmsg(_) => "rpmsg",
            GroupKind::Gpio(_) => "gpio",
            GroupKind::Hdlc(_) => "hdlc",
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
            GroupKind::I2c(_)
            | GroupKind::Spi(_)
            | GroupKind::Can(_)
            | GroupKind::Iio(_)
            | GroupKind::Rpmsg(_)
            | GroupKind::Gpio(_)
            | GroupKind::Hdlc(_) => None,
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
    /// A network interface, and nothing else.
    ///
    /// For a synchronous serial line, which Linux drives as a network
    /// interface. One interface is one payload: a raw HDLC frame carries no
    /// address, so there is nothing to tell two payloads on one line apart --
    /// which is what makes this different from a CAN interface. Experimental;
    /// see [`crate::node::payload::hdlc`].
    Interface { interface: String },
    /// A CAN interface and the identifier a payload's frames carry.
    ///
    /// An interface rather than a path: a CAN controller is a network
    /// interface, not a device file. Both are needed for the reason an I2C
    /// device needs both -- several payloads commonly share one bus and
    /// differ only in the identifier. Experimental; see
    /// [`crate::node::payload::can`].
    Can { interface: String, can_id: u32 },
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
/// configuration has its own shape, and `crate::graph::reach` fills this in
/// from a node's attributes -- so the aliases are gone and so is the name.
#[derive(Debug, Default)]
pub(crate) struct LinkTerms {
    // A line.
    pub(crate) datarate: Option<Scalar>,
    pub(crate) stop_bits: Option<Scalar>,
    /// A parity per character on an asynchronous line, and the frame check on
    /// a synchronous one: the kernel calls both a line's parity.
    pub(crate) parity: Option<Scalar>,
    /// Whose clock a synchronous line's bits are on, how they are carried,
    /// what checks a frame, how fast this end clocks them, and which
    /// electrical standard the port is wired as: an hdlc node's, every one.
    /// See [`crate::node::payload::hdlc`].
    pub(crate) clock_type: Option<Scalar>,
    pub(crate) encoding: Option<Scalar>,
    pub(crate) frame_check: Option<Scalar>,
    pub(crate) clock_rate: Option<Scalar>,
    pub(crate) standard: Option<Scalar>,
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
    /// Whether a start-stop line is a half-duplex RS-485 bus, and what its
    /// turnaround is; see [`crate::node::payload::serial::rs485`].
    pub(crate) rs485: Option<Scalar>,
    pub(crate) rs485_rts_on_send: Option<Scalar>,
    pub(crate) rs485_delay_before_send: Option<Scalar>,
    pub(crate) rs485_delay_after_send: Option<Scalar>,
    /// Bits per second on a CAN bus, and whether its frames are CAN FD; see
    /// [`crate::node::payload::can`].
    pub(crate) can_fd: Option<Scalar>,
    /// What an IIO device is expected to be producing; see
    /// [`crate::node::payload::iio`].
    /// Which rpmsg channel a payload is, and how much one message carries;
    /// see [`crate::node::payload::rpmsg`].
    pub(crate) service: Option<Scalar>,
    pub(crate) max_message_bytes: Option<Scalar>,
    /// Which discrete lines a payload watches and how; see
    /// [`crate::node::payload::gpio`].
    pub(crate) lines: Option<Scalar>,
    pub(crate) edge: Option<Scalar>,
    pub(crate) bias: Option<Scalar>,
    pub(crate) debounce: Option<Scalar>,
    /// How often the current level of every line is reported, where the file
    /// asks for it; see [`crate::node::payload::gpio`].
    pub(crate) state_interval: Option<Scalar>,
    /// Whether a set of discrete lines is read or driven. See
    /// `node::payload::gpio::Direction`.
    pub(crate) direction: Option<Scalar>,
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
    /// A scalar as a file would have spelled it.
    ///
    /// For the rules' own tests, which ask a rule what it makes of a term
    /// without a file to put it in -- and for anything building a
    /// configuration rather than reading one.
    pub fn of(text: impl Into<String>) -> Self {
        Scalar(text.into())
    }

    /// The text, trimmed.
    ///
    /// Public because a node's attributes are these, and a node's attributes
    /// are public: a caller handed a `Graph` has to be able to read what a
    /// node says about itself.
    pub fn as_str(&self) -> &str {
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

            /// A key with nothing after it: `payload_port:`, and nothing else
            /// on the line.
            ///
            /// Read as no value at all, which is what it says and what the
            /// rules already make of an empty one -- absent and empty mean the
            /// same thing throughout this project, so the one spelling a
            /// person would write is the one a reader should accept. It is
            /// also what the editor writes for a question nobody has answered
            /// yet: a form with `""` in every blank says somebody typed two
            /// quotation marks.
            fn visit_unit<E: de::Error>(self) -> Result<Scalar, E> {
                Ok(Scalar(String::new()))
            }

            /// And the same thing where the reader offers it as an absent
            /// option rather than as a unit.
            fn visit_none<E: de::Error>(self) -> Result<Scalar, E> {
                Ok(Scalar(String::new()))
            }
        }

        d.deserialize_any(V)
    }
}

#[cfg(test)]
mod a_scalar_of_nothing {
    use super::Scalar;

    /// A key with nothing after it is a value nobody has supplied.
    ///
    /// Which is what the editor writes into a form: every question present and
    /// the answers blank. `""` would read the same and say something else --
    /// that somebody typed two quotation marks -- and the rules make no
    /// distinction anyway, absent and empty meaning the same thing throughout.
    #[test]
    fn a_key_with_nothing_after_it_is_read_as_nothing() {
        #[derive(Debug, serde::Deserialize)]
        struct Said {
            payload_port: Option<Scalar>,
            packet_size: Option<Scalar>,
        }

        let said: Said =
            serde_norway::from_str("payload_port:\npacket_size: 4\n").expect("it reads");
        // An optional field never reaches the scalar: the reader answers the
        // Option itself, and nothing is None. Which is the same meaning by a
        // shorter road.
        assert_eq!(said.payload_port.as_ref().map(Scalar::as_str), None);
        assert_eq!(said.packet_size.as_ref().map(Scalar::as_str), Some("4"));

        // And a map of them, which is how a node's attributes and a simulated
        // payload's settings are read.
        let mut settings: std::collections::BTreeMap<String, Scalar> =
            serde_norway::from_str("drop_percent:\njitter_ms: 20\n").expect("it reads");
        assert_eq!(settings.remove("drop_percent").map(|s| s.as_str().to_string()), Some(String::new()));
        assert_eq!(settings.remove("jitter_ms").map(|s| s.as_str().to_string()), Some("20".to_string()));
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

impl ByteList {
    /// The bytes this text lists, as a file would have written them: commas
    /// or spaces between them, decimal or `0x`.
    ///
    /// For a caller building a configuration rather than reading one -- a
    /// node states its terminators as one attribute, and this is how that
    /// one attribute becomes the list.
    pub fn of(text: &str) -> ByteList {
        ByteList(split_list(text))
    }
}

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
fn type_specific_fields(g: &LinkTerms) -> [(&'static str, bool); 32] {
    [
        // Serial.
        ("datarate", g.datarate.is_some()),
        // HDLC: a synchronous line.
        ("standard", g.standard.is_some()),
        ("clock_type", g.clock_type.is_some()),
        ("clock_rate", g.clock_rate.is_some()),
        ("encoding", g.encoding.is_some()),
        ("frame_check", g.frame_check.is_some()),
        ("rs485", g.rs485.is_some()),
        ("rs485_rts_on_send", g.rs485_rts_on_send.is_some()),
        (
            "rs485_delay_before_send",
            g.rs485_delay_before_send.is_some(),
        ),
        (
            "rs485_delay_after_send",
            g.rs485_delay_after_send.is_some(),
        ),
        ("stop_bits", g.stop_bits.is_some()),
        ("parity", g.parity.is_some()),
        ("byte_length", g.byte_length.is_some()),
        // Network.
        ("protocol", g.protocol.is_some()),
        // I2C.
        ("ten_bit", g.ten_bit.is_some()),
        ("pec", g.pec.is_some()),
        ("retries", g.retries.is_some()),
        ("timeout", g.timeout.is_some()),
        // CAN, and the loopback a synchronous line shares the word with.
        ("can_fd", g.can_fd.is_some()),
        ("loopback", g.loopback.is_some()),
        // rpmsg.
        ("service", g.service.is_some()),
        ("max_message_bytes", g.max_message_bytes.is_some()),
        // GPIO.
        ("lines", g.lines.is_some()),
        ("edge", g.edge.is_some()),
        ("bias", g.bias.is_some()),
        ("debounce", g.debounce.is_some()),
        ("direction", g.direction.is_some()),
        // SPI.
        ("max_speed", g.max_speed.is_some()),
        ("spi_mode", g.spi_mode.is_some()),
        ("bits_per_word", g.bits_per_word.is_some()),
        ("bit_order", g.bit_order.is_some()),
        ("cs_active", g.cs_active.is_some()),
    ]
}

/// A term of a link in these, apart from the ones `own` names.
///
/// For the two kinds that are not on a link -- a network address and a plain
/// device file -- which have no rules of their own to refuse what belongs to
/// another kind. Every other kind refuses them with
/// [`reject_foreign_fields`], and these two were left ignoring them: a device
/// payload could state a data rate, or a sensor's channels, and be read as a
/// device payload that said nothing of the sort.
pub(crate) fn a_link_term(g: &LinkTerms, own: &[&str]) -> Option<&'static str> {
    type_specific_fields(g)
        .into_iter()
        .find(|(field, present)| *present && !own.contains(field))
        .map(|(field, _)| field)
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

/// Parse an unsigned value, in whatever way a configuration may write one.
///
/// The grammar is [`crate::whole`]'s and is the same for every integer in
/// every one of these files: decimal, or hexadecimal with an `0x` prefix,
/// underscores allowed among the digits.
pub(crate) fn parse_u32(group: &str, field: &str, s: &Scalar) -> EndpointConfigResult<u32> {
    crate::whole::whole_u32(s.as_str()).map_err(|why| bad(group, &format!("{field}: {why}")))
}

/// Parse a byte value, by the one grammar: see [`crate::whole`].
fn parse_byte(group: &str, text: &str) -> EndpointConfigResult<u8> {
    let text = text.trim();
    match crate::whole::whole(text) {
        Ok(v) if v <= u8::MAX as u64 => Ok(v as u8),
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

/// Parse a timeout: a length of time, or the word `none`.
///
/// Every value [`crate::graph::parse_interval`] takes, in every unit it takes
/// -- that being the one grammar for a length of time in a configuration, so
/// a reader who has learned how a group's interval is written has learned
/// this. There was a second grammar here, narrower by two units and by the
/// decimal fraction, and a configuration could say `1.5 s` in one place and
/// not the other with nothing to say why.
///
/// And the word `none`, which a length of time cannot say: a read that does
/// not time out at all. `Ok(None)` is that word. It is the difference between
/// a timeout and an interval, and the reason the two are different kinds of
/// value rather than one -- see `tcsgui::nodes::KINDS`, where an attribute
/// says which it takes.
///
/// Nought is refused, as it is of any interval: a read that times out at once
/// reads nothing, and `none` is the way to ask for a read that is waited out.
pub(crate) fn parse_timeout(group: &str, s: &Scalar) -> EndpointConfigResult<Option<Duration>> {
    let text = s.as_str();
    if text.trim().eq_ignore_ascii_case("none") {
        return Ok(None);
    }

    crate::graph::parse_interval(text.trim())
        .map(Some)
        .map_err(|why| bad(group, &format!("timeout: {why}, or the word none")))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
