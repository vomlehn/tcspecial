//! Serial groups: what a line's framing is, and how a file states it.
//!
//! An asynchronous serial line carries no addressing and no framing of its
//! own beyond the byte: what a file has to say about one is the rate it runs
//! at and how a byte is delimited on it. Where a read of the resulting stream
//! ends is a stream section, which a serial group shares with the stream
//! network protocols rather than owning -- see [`crate::endpoint_config`].

// The two-wire variant, which is one of a line's own terms rather than
// an interface of its own -- so it nests here rather than sitting beside
// this in node/.
pub mod rs485;

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::endpoint_config::{
    bad, parse_u32, reject_foreign_fields, require, EndpointConfigError, EndpointConfigResult,
    GroupKind, LinkTerms, Scalar,
};
use crate::endpoint_config::{validate_stream, StreamParams};

/// Which parity a file's word names, for an asynchronous line.
fn parse_parity(group: &str, s: &Scalar) -> EndpointConfigResult<Parity> {
    Parity::from_spelling(&s.as_str().to_ascii_lowercase()).ok_or_else(|| {
        bad(
            group,
            &format!(
                "parity: \"{}\" is not one of none, even, odd, mark, or space",
                s.as_str()
            ),
        )
    })
}

/// Attributes of a serial port group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SerialParams {
    /// Line rate in bits per second.
    pub datarate: u32,
    /// Stop bits following each byte, which is what start-stop framing uses
    /// in place of a clock.
    pub stop_bits: StopBits,
    /// Parity, which termios sets per character.
    pub parity: Parity,
    /// Data bits in each byte.
    pub byte_length: ByteLength,
    /// The half-duplex turnaround, for a line that is an RS-485 bus, and
    /// `None` for one that is not; see [`crate::node::payload::serial::rs485`].
    pub rs485: Option<crate::node::payload::serial::rs485::Rs485Params>,
    /// Stream device protocol attributes. Required: a serial port is a
    /// stream, so there is always a rule for where one read ends.
    pub stream: StreamParams,
}

/// Parity on an asynchronous line, which termios sets per character.
///
/// The five a UART offers, and the two beyond even and odd are the ones a
/// file has to say out loud: mark and space send a constant bit rather than
/// one computed from the data, which is a protocol's way of marking a frame
/// rather than checking one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Parity {
    None,
    Even,
    Odd,
    Mark,
    Space,
}

impl Parity {
    /// How this value is spelled in a configuration file.
    pub fn as_str(&self) -> &'static str {
        match self {
            Parity::None => "none",
            Parity::Even => "even",
            Parity::Odd => "odd",
            Parity::Mark => "mark",
            Parity::Space => "space",
        }
    }

    /// Which parity a file's word names, if it names one. Named as the
    /// other spellings in this library are, and not `from_str`, which reads
    /// as the standard trait's method and is not one.
    pub fn from_spelling(text: &str) -> Option<Parity> {
        [
            Parity::None,
            Parity::Even,
            Parity::Odd,
            Parity::Mark,
            Parity::Space,
        ]
        .into_iter()
        .find(|parity| parity.as_str() == text)
    }
}

impl fmt::Display for Parity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Stop bits following each byte on a serial line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// What a serial group says, and what it must say.
///
/// A serial line is a stream, so a stream section is required: a read of one
/// needs a rule for where it ends. The framing attributes are required too --
/// no default data rate or byte length is right for a line somebody else's
/// hardware is at the other end of.
pub(crate) fn group_kind_of(name: &str, g: LinkTerms) -> EndpointConfigResult<GroupKind> {
    reject_foreign_fields(
        name,
        "serial",
        &g,
        &[
            "datarate",
            "stop_bits",
            "byte_length",
            "parity",
            "rs485",
            "rs485_rts_on_send",
            "rs485_delay_before_send",
            "rs485_delay_after_send",
        ],
    )?;

    let datarate = require(name, "serial", "datarate", g.datarate.as_ref())?;
    let datarate = parse_u32(name, "datarate", datarate)?;
    if datarate == 0 {
        return Err(bad(name, "datarate must be greater than zero"));
    }

    // Required: a stop bit is what start-stop framing uses in place of a
    // clock, so a line without one is not a line this type describes. A line
    // whose bits are on a clock is an hdlc node; see crate::node::payload::hdlc.
    let stop_bits = require(name, "serial", "stop_bits", g.stop_bits.as_ref())?;
    let stop_bits = parse_stop_bits(name, stop_bits)?;

    let byte_length = require(name, "serial", "byte_length", g.byte_length.as_ref())?;
    let byte_length = parse_u32(name, "byte_length", byte_length)?;
    let byte_length =
        ByteLength::new(byte_length.min(u8::MAX as u32) as u8).map_err(|m| bad(name, &m))?;

    // The electrical layer under a start-stop line, which a two-wire bus
    // needs told and a four-wire line says nothing about. Read before the
    // stream section below, which takes the terms apart.
    let rs485 = crate::node::payload::serial::rs485::rs485_of(name, &g, true)?;

    // A serial port is a stream, so a read needs an end.
    let stream = g.stream.ok_or(EndpointConfigError::MissingGroupField {
        group: name.to_string(),
        kind: "serial",
        field: "a stream section",
    })?;
    let stream = validate_stream(name, stream)?;

    // Absent is no parity, which is what a line was set to before this could
    // be stated and what nearly every device link uses. Termios sets it per
    // character, which is the whole of what a start-stop line's parity is: a
    // frame check belongs to a synchronous line and lives on the hdlc node.
    let parity = match g.parity.as_ref() {
        Some(stated) => parse_parity(name, stated)?,
        None => Parity::None,
    };

    Ok(GroupKind::Serial(SerialParams {
        datarate,
        stop_bits,
        parity,
        byte_length,
        stream,
        rs485,
    }))
}
