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
    bad, parse_flag, parse_u32, reject_foreign_fields, require, EndpointConfigError,
    EndpointConfigResult, GroupKind, GroupWire, Scalar,
};

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

/// Whose clock, for a synchronous line.
fn parse_clock_type(group: &str, s: &Scalar) -> EndpointConfigResult<ClockType> {
    ClockType::from_spelling(&s.as_str().to_ascii_lowercase()).ok_or_else(|| {
        bad(
            group,
            &format!(
                "clock_type: \"{}\" is not one of external, internal, tx_internal, \
                 or tx_from_rx",
                s.as_str()
            ),
        )
    })
}

/// How the bits are carried, for a synchronous line.
fn parse_encoding(group: &str, s: &Scalar) -> EndpointConfigResult<Encoding> {
    Encoding::from_spelling(&s.as_str().to_ascii_lowercase()).ok_or_else(|| {
        bad(
            group,
            &format!(
                "encoding: \"{}\" is not one of nrz, nrzi, fm_mark, fm_space, or \
                 manchester",
                s.as_str()
            ),
        )
    })
}

/// What checks a frame, for a synchronous line. Written as the line's parity,
/// which is what the kernel calls it.
fn parse_frame_check(group: &str, s: &Scalar) -> EndpointConfigResult<FrameCheck> {
    FrameCheck::from_spelling(&s.as_str().to_ascii_lowercase()).ok_or_else(|| {
        bad(
            group,
            &format!(
                "parity: \"{}\" is not a frame check: a synchronous line takes none, \
                 crc16_pr0, crc16_pr1, crc16_pr0_ccitt, crc16_pr1_ccitt, \
                 crc32_pr0_ccitt, or crc32_pr1_ccitt",
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
    /// Whether the line is start-stop framed, each byte delimited by its own
    /// start and stop bits, or synchronous, the bits carried on a clock.
    ///
    /// Stated rather than assumed: what else a group may say follows from it,
    /// and a line read as the wrong one of the two is a line read as noise.
    pub asynchronous: bool,
    /// Stop bits following each byte, for an asynchronous line. A synchronous
    /// line has none: a stop bit is what start-stop framing uses in place of
    /// a clock, so there is nothing for it to delimit.
    pub stop_bits: Option<StopBits>,
    /// Parity on an asynchronous line, which termios sets per character.
    /// `None` for a synchronous line, whose `parity` is a frame check and is
    /// kept in `frame_check`.
    pub parity: Option<Parity>,
    /// Whose clock a synchronous line's bits are on, and `None` for an
    /// asynchronous one, which has no clock to share.
    pub clock_type: Option<ClockType>,
    /// How a synchronous line's bits are carried, and `None` for an
    /// asynchronous one.
    pub encoding: Option<Encoding>,
    /// What checks a frame on a synchronous line -- the kernel's own `parity`
    /// for raw HDLC -- and `None` for an asynchronous one.
    pub frame_check: Option<FrameCheck>,
    /// Whether a synchronous line is put in loopback, and `None` for an
    /// asynchronous one.
    pub loopback: Option<bool>,
    /// Data bits in each byte.
    pub byte_length: ByteLength,
    /// Stream payload protocol attributes. Required: a serial port is a
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

/// Whose clock a synchronous line's bits are on.
///
/// The kernel's four, from `linux/hdlc/ioctl.h`: a payload link is commonly
/// `external`, the equipment at the far end driving the clock, and
/// `internal` where this end drives it. The two mixed settings exist because
/// some links take their transmit clock from one place and their receive
/// clock from another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockType {
    External,
    Internal,
    TxInternal,
    TxFromRx,
}

impl ClockType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClockType::External => "external",
            ClockType::Internal => "internal",
            ClockType::TxInternal => "tx_internal",
            ClockType::TxFromRx => "tx_from_rx",
        }
    }

    pub fn from_spelling(text: &str) -> Option<ClockType> {
        [
            ClockType::External,
            ClockType::Internal,
            ClockType::TxInternal,
            ClockType::TxFromRx,
        ]
        .into_iter()
        .find(|clock| clock.as_str() == text)
    }

    /// Whether this end generates the clock the data is sent on.
    ///
    /// Which decides what the data rate means: a rate this end generates, or
    /// a rate recorded against what the far end is expected to clock.
    pub fn is_ours(&self) -> bool {
        matches!(self, ClockType::Internal | ClockType::TxInternal)
    }
}

impl fmt::Display for ClockType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a synchronous line's bits are carried on the wire.
///
/// The kernel's five. NRZ is the plain one; NRZI and the FM codings carry
/// their own clock recovery, which a long cable needs and a short one does
/// not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Encoding {
    Nrz,
    Nrzi,
    FmMark,
    FmSpace,
    Manchester,
}

impl Encoding {
    pub fn as_str(&self) -> &'static str {
        match self {
            Encoding::Nrz => "nrz",
            Encoding::Nrzi => "nrzi",
            Encoding::FmMark => "fm_mark",
            Encoding::FmSpace => "fm_space",
            Encoding::Manchester => "manchester",
        }
    }

    pub fn from_spelling(text: &str) -> Option<Encoding> {
        [
            Encoding::Nrz,
            Encoding::Nrzi,
            Encoding::FmMark,
            Encoding::FmSpace,
            Encoding::Manchester,
        ]
        .into_iter()
        .find(|encoding| encoding.as_str() == text)
    }
}

impl fmt::Display for Encoding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What checks a frame on a synchronous line.
///
/// The kernel calls this a line's parity too, and for a synchronous line it
/// is a frame check rather than a bit per character: the variants are the
/// CRCs `linux/hdlc/ioctl.h` offers, which differ in width, in the value the
/// register starts at, and in whether the ITU-T polynomial is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameCheck {
    None,
    Crc16Pr0,
    Crc16Pr1,
    Crc16Pr0Ccitt,
    Crc16Pr1Ccitt,
    Crc32Pr0Ccitt,
    Crc32Pr1Ccitt,
}

impl FrameCheck {
    pub fn as_str(&self) -> &'static str {
        match self {
            FrameCheck::None => "none",
            FrameCheck::Crc16Pr0 => "crc16_pr0",
            FrameCheck::Crc16Pr1 => "crc16_pr1",
            FrameCheck::Crc16Pr0Ccitt => "crc16_pr0_ccitt",
            FrameCheck::Crc16Pr1Ccitt => "crc16_pr1_ccitt",
            FrameCheck::Crc32Pr0Ccitt => "crc32_pr0_ccitt",
            FrameCheck::Crc32Pr1Ccitt => "crc32_pr1_ccitt",
        }
    }

    pub fn from_spelling(text: &str) -> Option<FrameCheck> {
        [
            FrameCheck::None,
            FrameCheck::Crc16Pr0,
            FrameCheck::Crc16Pr1,
            FrameCheck::Crc16Pr0Ccitt,
            FrameCheck::Crc16Pr1Ccitt,
            FrameCheck::Crc32Pr0Ccitt,
            FrameCheck::Crc32Pr1Ccitt,
        ]
        .into_iter()
        .find(|check| check.as_str() == text)
    }
}

impl fmt::Display for FrameCheck {
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
pub(crate) fn group_kind_of(
    name: &str,
    g: GroupWire,
) -> EndpointConfigResult<GroupKind> {

    reject_foreign_fields(
        name,
        "serial",
        &g,
        &[
            "datarate",
            "stop_bits",
            "byte_length",
            "asynchronous",
            "parity",
            "clock_type",
            "encoding",
            "loopback",
        ],
    )?;

    // Which kind of line this is, which decides what else the group may say.
    // Required, because the two are read differently and a group that did not
    // say would be guessed at.
    let asynchronous = require(name, "serial", "asynchronous", g.asynchronous.as_ref())?;
    let asynchronous = parse_flag(name, "asynchronous", Some(asynchronous))?
        .expect("a value was required");

    let datarate = require(name, "serial", "datarate", g.datarate.as_ref())?;
    let datarate = parse_u32(name, "datarate", datarate)?;
    if datarate == 0 {
        return Err(bad(name, "datarate must be greater than zero"));
    }

    // Stop bits belong to start-stop framing and to nothing else: they are
    // what an asynchronous line uses in place of a clock, so a synchronous
    // group stating them has described two kinds of line at once.
    let stop_bits = match (asynchronous, g.stop_bits.as_ref()) {
        (true, Some(stated)) => Some(parse_stop_bits(name, stated)?),
        (true, None) => {
            return Err(EndpointConfigError::MissingGroupField {
                group: name.to_string(),
                kind: "serial",
                field: "stop_bits",
            })
        }
        (false, Some(_)) => {
            return Err(bad(
                name,
                "stop_bits does not apply to a synchronous line: a stop bit is what \
                 start-stop framing uses in place of a clock, so a line whose bits \
                 are on a clock has none",
            ))
        }
        (false, None) => None,
    };

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

    // What each kind of line offers beyond the above, and nothing of the
    // other kind's. Each is what the Linux driver for such a line can be
    // told: termios sets a parity per character on a start-stop line, and the
    // kernel's generic HDLC takes a clock, an encoding, a frame check and a
    // loopback for a synchronous one.
    let (parity, clock_type, encoding, frame_check, loopback) = if asynchronous {
        for (field, stated) in [
            ("clock_type", g.clock_type.is_some()),
            ("encoding", g.encoding.is_some()),
            ("loopback", g.loopback.is_some()),
        ] {
            if stated {
                return Err(bad(
                    name,
                    &format!(
                        "{field} does not apply to an asynchronous line: it is a \
                         setting of the clock a synchronous line's bits are carried \
                         on, and a start-stop line shares no clock"
                    ),
                ));
            }
        }

        // Absent is no parity, which is what a line was set to before this
        // could be stated and what nearly every payload link uses.
        let parity = match g.parity.as_ref() {
            Some(stated) => Some(parse_parity(name, stated)?),
            None => Some(Parity::None),
        };
        (parity, None, None, None, None)
    } else {
        // Whose clock, which a synchronous line has no default for: the two
        // ends must agree, and guessing which drives it would be guessing at
        // the cable.
        let clock_type = require(name, "serial", "clock_type", g.clock_type.as_ref())?;
        let clock_type = parse_clock_type(name, clock_type)?;

        let encoding = match g.encoding.as_ref() {
            Some(stated) => parse_encoding(name, stated)?,
            None => Encoding::Nrz,
        };
        let frame_check = match g.parity.as_ref() {
            Some(stated) => parse_frame_check(name, stated)?,
            None => FrameCheck::None,
        };
        let loopback = parse_flag(name, "loopback", g.loopback.as_ref())?.unwrap_or(false);

        (
            None,
            Some(clock_type),
            Some(encoding),
            Some(frame_check),
            Some(loopback),
        )
    };

    Ok(GroupKind::Serial(SerialParams {
        datarate,
        asynchronous,
        stop_bits,
        parity,
        clock_type,
        encoding,
        frame_check,
        loopback,
        byte_length,
        stream,
    }))
}
