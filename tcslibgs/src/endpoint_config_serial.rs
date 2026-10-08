//! Serial groups: what a line's framing is, and how a file states it.
//!
//! An asynchronous serial line carries no addressing and no framing of its
//! own beyond the byte: what a file has to say about one is the rate it runs
//! at and how a byte is delimited on it. Where a read of the resulting stream
//! ends is a stream section, which a serial group shares with the stream
//! network protocols rather than owning -- see [`crate::endpoint_config`].

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::endpoint_config::{validate_stream, StreamParams};
use crate::endpoint_config::{
    bad, parse_u32, reject_foreign_fields, require, EndpointConfigError, EndpointConfigResult,
    GroupKind, GroupWire, Scalar,
};

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
pub(crate) fn group_kind_of(
    name: &str,
    g: GroupWire,
) -> EndpointConfigResult<GroupKind> {

    reject_foreign_fields(
        name,
        "serial",
        &g,
        &["datarate", "stop_bits", "byte_length"],
    )?;

    let datarate = require(name, "serial", "datarate", g.datarate.as_ref())?;
    let datarate = parse_u32(name, "datarate", datarate)?;
    if datarate == 0 {
        return Err(bad(name, "datarate must be greater than zero"));
    }

    let stop_bits = require(name, "serial", "stop_bits", g.stop_bits.as_ref())?;
    let stop_bits = parse_stop_bits(name, stop_bits)?;

    let byte_length = require(name, "serial", "byte_length", g.byte_length.as_ref())?;
    let byte_length = parse_u32(name, "byte_length", byte_length)?;
    let byte_length = ByteLength::new(byte_length.min(u8::MAX as u32) as u8)
        .map_err(|m| bad(name, &m))?;

    // A serial port is a stream, so a read needs an end.
    let stream = g.stream.ok_or(EndpointConfigError::MissingGroupField {
        group: name.to_string(),
        kind: "serial",
        field: "a stream section",
    })?;
    let stream = validate_stream(name, stream)?;

    Ok(GroupKind::Serial(SerialParams {
        datarate,
        stop_bits,
        byte_length,
        stream,
    }))
}
