// @generated automatically by tcs-xtce from tcsprotocol/xtce/ping.xtce
// (SpaceSystem 'TcsPing').
// Any changes made to this file will be lost when it is regenerated;
// change the XTCE document instead and rerun `python3 -m tcs_xtce`.
// tcs-xtce: https://github.com/vomlehn/tcs-xtce
// Runtime crate: https://crates.io/crates/tcsxtce
//! Packet codecs for the `TcsPing` mission.
//!
//! Generated with `--dynamic-allocations false`: arrays, strings and binary
//! fields have fixed capacities, and decoding, encoding and describing a
//! packet never allocate.
#![allow(
    dead_code,
    unused_imports,
    unused_variables,
    unused_assignments,
    unused_mut,
    unreachable_code,
    unused_parens,
    clippy::all
)]

use std::fmt::{self, Write as _};
use tcsxtce::packet::{
    self as pk, BitReader, BitWriter, IntEncoding, Packet, PacketError, Result,
    StringSpec,
};

/// The functions the application supplies for the document's custom
/// algorithms: set them before decoding or encoding a packet that needs
/// them, or it fails with `NoAlgorithm`.
pub mod hooks {}

/// The alarm state of each parameter whose alarms depend on earlier
/// samples, which the packets' `monitor()` reads and updates.
#[derive(Debug, Clone, Default)]
pub struct AlarmMonitor {}

/// Sequence container `/TcsPing/PingReply`.
///
/// Fixed by its inheritance: `ReplyPrefix` = 'ping #', `ReplySpace` = ' '.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PingReply {
    /// `/TcsPing/ReplyCount` (/TcsPing/PingCountType)
    pub reply_count: pk::FixedString<20>,
    /// `/TcsPing/ReplySentSeconds` (/TcsPing/PingSecondsType)
    pub reply_sent_seconds: pk::FixedString<20>,
    /// `/TcsPing/ReplySentNanoseconds` (/TcsPing/PingNanosecondsType)
    pub reply_sent_nanoseconds: pk::FixedString<9>,
    /// `/TcsPing/ReplyAnsweredSeconds` (/TcsPing/PingSecondsType)
    pub reply_answered_seconds: pk::FixedString<20>,
    /// `/TcsPing/ReplyAnsweredNanoseconds` (/TcsPing/PingNanosecondsType)
    pub reply_answered_nanoseconds: pk::FixedString<9>,
}

impl PingReply {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsPing/PingReply` packet's fields.
    /// Alarms that depend on earlier samples count only this one (see
    /// `monitor`).
    ///
    /// # Returns
    ///
    /// The alarm level of each field with an alarm, `None` when the field is
    /// absent.
    pub fn alarms(&self) -> pk::Alarms<0> {
        let mut a = pk::Alarms::new();
        a
    }

    /// Evaluates the alarms of this `/TcsPing/PingReply` packet's fields like
    /// `alarms()`, counting the earlier samples for change ranges,
    /// minViolations and minConformance.
    ///
    /// # Arguments
    ///
    /// * `m` - The state of the alarms that depend on earlier samples, which
    ///         the method reads and updates.
    /// * `t` - This sample's time, in seconds.
    ///
    /// # Returns
    ///
    /// The alarm level of each field with an alarm, `None` when the field is
    /// absent.
    pub fn monitor(&self, m: &mut AlarmMonitor, t: f64) -> pk::Alarms<0> {
        let mut a = pk::Alarms::new();
        a
    }

    /// The number of MathAlgorithms this packet triggers.
    pub const ALGORITHMS: usize = 0;

    /// Evaluates the MathAlgorithms that a `/TcsPing/PingReply` packet
    /// triggers, from this one's values.
    ///
    /// # Returns
    ///
    /// The output of each, `None` when an input is absent or a custom
    /// algorithm's function isn't set or fails.
    pub fn algorithms(&self) -> pk::Outputs<0> {
        let mut o = pk::Outputs::new();
        o
    }
}

impl Packet for PingReply {
    const NAME: &'static str = "/TcsPing/PingReply";
    const MAX_BYTES: usize = 88;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = pk::read_fixed_string::<6, 6>(
            &mut r,
            &StringSpec {
                fixed_bits: Some(48),
                max_bits: 48,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "ReplyPrefix",
        )?;
        let v2 = pk::read_fixed_string::<20, 21>(
            &mut r,
            &StringSpec {
                fixed_bits: None,
                max_bits: 168,
                terminator: Some(&[0x20]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "ReplyCount",
        )?;
        let v3 = pk::read_fixed_string::<20, 21>(
            &mut r,
            &StringSpec {
                fixed_bits: None,
                max_bits: 168,
                terminator: Some(&[0x2e]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "ReplySentSeconds",
        )?;
        let v4 = pk::read_fixed_string::<9, 9>(
            &mut r,
            &StringSpec {
                fixed_bits: Some(72),
                max_bits: 72,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "ReplySentNanoseconds",
        )?;
        let v5 = pk::read_fixed_string::<1, 1>(
            &mut r,
            &StringSpec {
                fixed_bits: Some(8),
                max_bits: 8,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "ReplySpace",
        )?;
        let v6 = pk::read_fixed_string::<20, 21>(
            &mut r,
            &StringSpec {
                fixed_bits: None,
                max_bits: 168,
                terminator: Some(&[0x2e]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "ReplyAnsweredSeconds",
        )?;
        let v7 = pk::read_fixed_string::<9, 9>(
            &mut r,
            &StringSpec {
                fixed_bits: Some(72),
                max_bits: 72,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "ReplyAnsweredNanoseconds",
        )?;
        if !(v1 == "ping #") {
            return Err(PacketError::Mismatch {
                container: "/TcsPing/PingReply",
                criterion: "/TcsPing/ReplyPrefix == ping #",
            });
        }
        if !(v5 == " ") {
            return Err(PacketError::Mismatch {
                container: "/TcsPing/PingReply",
                criterion: "/TcsPing/ReplySpace ==  ",
            });
        }
        Ok((
            PingReply {
                reply_count: v2,
                reply_sent_seconds: v3,
                reply_sent_nanoseconds: v4,
                reply_answered_seconds: v6,
                reply_answered_nanoseconds: v7,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: Some(48),
                max_bits: 48,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            &"ping #",
            "ReplyPrefix",
        )?;
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: None,
                max_bits: 168,
                terminator: Some(&[0x20]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            &self.reply_count,
            "ReplyCount",
        )?;
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: None,
                max_bits: 168,
                terminator: Some(&[0x2e]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            &self.reply_sent_seconds,
            "ReplySentSeconds",
        )?;
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: Some(72),
                max_bits: 72,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            &self.reply_sent_nanoseconds,
            "ReplySentNanoseconds",
        )?;
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: Some(8),
                max_bits: 8,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            &" ",
            "ReplySpace",
        )?;
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: None,
                max_bits: 168,
                terminator: Some(&[0x2e]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            &self.reply_answered_seconds,
            "ReplyAnsweredSeconds",
        )?;
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: Some(72),
                max_bits: 72,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            &self.reply_answered_nanoseconds,
            "ReplyAnsweredNanoseconds",
        )?;
        Ok(())
    }
}

impl fmt::Display for PingReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PingReply{")?;
        f.write_str("reply_count=")?;
        pk::describe_str(&self.reply_count, f)?;
        f.write_str(", reply_sent_seconds=")?;
        pk::describe_str(&self.reply_sent_seconds, f)?;
        f.write_str(", reply_sent_nanoseconds=")?;
        pk::describe_str(&self.reply_sent_nanoseconds, f)?;
        f.write_str(", reply_answered_seconds=")?;
        pk::describe_str(&self.reply_answered_seconds, f)?;
        f.write_str(", reply_answered_nanoseconds=")?;
        pk::describe_str(&self.reply_answered_nanoseconds, f)?;
        f.write_char('}')
    }
}

/// Meta-command `/TcsPing/Ping`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ping {
    /// `/TcsPing/Ping#Count` (/TcsPing/PingCountArg)
    pub count: pk::FixedString<20>,
    /// `/TcsPing/Ping#SentSeconds` (/TcsPing/PingSecondsArg)
    pub sent_seconds: pk::FixedString<20>,
    /// `/TcsPing/Ping#SentNanoseconds` (/TcsPing/PingNanosecondsArg)
    pub sent_nanoseconds: pk::FixedString<9>,
}

impl Ping {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsPing/Ping` packet's fields. Alarms
    /// that depend on earlier samples count only this one (see `monitor`).
    ///
    /// # Returns
    ///
    /// The alarm level of each field with an alarm, `None` when the field is
    /// absent.
    pub fn alarms(&self) -> pk::Alarms<0> {
        let mut a = pk::Alarms::new();
        a
    }

    /// Evaluates the alarms of this `/TcsPing/Ping` packet's fields like
    /// `alarms()`, counting the earlier samples for change ranges,
    /// minViolations and minConformance.
    ///
    /// # Arguments
    ///
    /// * `m` - The state of the alarms that depend on earlier samples, which
    ///         the method reads and updates.
    /// * `t` - This sample's time, in seconds.
    ///
    /// # Returns
    ///
    /// The alarm level of each field with an alarm, `None` when the field is
    /// absent.
    pub fn monitor(&self, m: &mut AlarmMonitor, t: f64) -> pk::Alarms<0> {
        let mut a = pk::Alarms::new();
        a
    }

    /// The number of MathAlgorithms this packet triggers.
    pub const ALGORITHMS: usize = 0;

    /// Evaluates the MathAlgorithms that a `/TcsPing/Ping` packet triggers,
    /// from this one's values.
    ///
    /// # Returns
    ///
    /// The output of each, `None` when an input is absent or a custom
    /// algorithm's function isn't set or fails.
    pub fn algorithms(&self) -> pk::Outputs<0> {
        let mut o = pk::Outputs::new();
        o
    }
}

impl Packet for Ping {
    const NAME: &'static str = "/TcsPing/Ping";
    const MAX_BYTES: usize = 57;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        r.uint(48, "/TcsPing/Ping")?;
        let v8 = pk::read_fixed_string::<20, 21>(
            &mut r,
            &StringSpec {
                fixed_bits: None,
                max_bits: 168,
                terminator: Some(&[0x20]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "Count",
        )?;
        let v9 = pk::read_fixed_string::<20, 21>(
            &mut r,
            &StringSpec {
                fixed_bits: None,
                max_bits: 168,
                terminator: Some(&[0x2e]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "SentSeconds",
        )?;
        let v10 = pk::read_fixed_string::<9, 9>(
            &mut r,
            &StringSpec {
                fixed_bits: Some(72),
                max_bits: 72,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "SentNanoseconds",
        )?;
        Ok((
            Ping {
                count: v8,
                sent_seconds: v9,
                sent_nanoseconds: v10,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint(48, 0x70696e672023);
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: None,
                max_bits: 168,
                terminator: Some(&[0x20]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            &self.count,
            "Count",
        )?;
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: None,
                max_bits: 168,
                terminator: Some(&[0x2e]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            &self.sent_seconds,
            "SentSeconds",
        )?;
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: Some(72),
                max_bits: 72,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            &self.sent_nanoseconds,
            "SentNanoseconds",
        )?;
        Ok(())
    }
}

impl fmt::Display for Ping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Ping{")?;
        f.write_str("count=")?;
        pk::describe_str(&self.count, f)?;
        f.write_str(", sent_seconds=")?;
        pk::describe_str(&self.sent_seconds, f)?;
        f.write_str(", sent_nanoseconds=")?;
        pk::describe_str(&self.sent_nanoseconds, f)?;
        f.write_char('}')
    }
}

/// Every concrete packet derived from `/TcsPing/PingReplyFields`.
#[derive(Debug, Clone, PartialEq)]
pub enum PingReplyFieldsKind {
    PingReply(PingReply),
}

impl Packet for PingReplyFieldsKind {
    const NAME: &'static str = "/TcsPing/PingReplyFields";
    const MAX_BYTES: usize = 88;

    /// Decodes the root's entries, then picks the first child (in
    /// document order) whose criteria match, recursively.
    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = pk::read_fixed_string::<6, 6>(
            &mut r,
            &StringSpec {
                fixed_bits: Some(48),
                max_bits: 48,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "ReplyPrefix",
        )?;
        let v2 = pk::read_fixed_string::<20, 21>(
            &mut r,
            &StringSpec {
                fixed_bits: None,
                max_bits: 168,
                terminator: Some(&[0x20]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "ReplyCount",
        )?;
        let v3 = pk::read_fixed_string::<20, 21>(
            &mut r,
            &StringSpec {
                fixed_bits: None,
                max_bits: 168,
                terminator: Some(&[0x2e]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "ReplySentSeconds",
        )?;
        let v4 = pk::read_fixed_string::<9, 9>(
            &mut r,
            &StringSpec {
                fixed_bits: Some(72),
                max_bits: 72,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "ReplySentNanoseconds",
        )?;
        let v5 = pk::read_fixed_string::<1, 1>(
            &mut r,
            &StringSpec {
                fixed_bits: Some(8),
                max_bits: 8,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "ReplySpace",
        )?;
        let v6 = pk::read_fixed_string::<20, 21>(
            &mut r,
            &StringSpec {
                fixed_bits: None,
                max_bits: 168,
                terminator: Some(&[0x2e]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "ReplyAnsweredSeconds",
        )?;
        let v7 = pk::read_fixed_string::<9, 9>(
            &mut r,
            &StringSpec {
                fixed_bits: Some(72),
                max_bits: 72,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "ReplyAnsweredNanoseconds",
        )?;
        if (v1 == "ping #") && (v5 == " ") {
            return Ok((
                Self::PingReply(PingReply {
                    reply_count: v2,
                    reply_sent_seconds: v3,
                    reply_sent_nanoseconds: v4,
                    reply_answered_seconds: v6,
                    reply_answered_nanoseconds: v7,
                }),
                r.used(),
            ));
        }
        return Err(PacketError::NoMatch {
            container: "/TcsPing/PingReplyFields",
        });
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        match self {
            Self::PingReply(p) => p.encode_into(w),
        }
    }
}

impl fmt::Display for PingReplyFieldsKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PingReply(p) => p.fmt(f),
        }
    }
}

impl PingReplyFieldsKind {
    /// The most fields with an alarm that a member has.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of the fields of this packet derived from
    /// `/TcsPing/PingReplyFields`, as its own `alarms()` does.
    ///
    /// # Returns
    ///
    /// The alarm level of each field with an alarm, `None` when the field is
    /// absent.
    pub fn alarms(&self) -> pk::Alarms<0> {
        let mut out = pk::Alarms::new();
        match self {
            Self::PingReply(p) => {
                for a in p.alarms().as_slice() {
                    out.push(a.field, a.level);
                }
            }
        }
        out
    }

    /// The most MathAlgorithms that a member triggers.
    pub const ALGORITHMS: usize = 0;

    /// Evaluates the MathAlgorithms that this packet derived from
    /// `/TcsPing/PingReplyFields` triggers, as its own `algorithms()` does.
    ///
    /// # Returns
    ///
    /// The output of each, `None` when it can't be computed.
    pub fn algorithms(&self) -> pk::Outputs<0> {
        let mut out = pk::Outputs::new();
        match self {
            Self::PingReply(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
        }
        out
    }

    /// Evaluates the alarms of the fields of this packet derived from
    /// `/TcsPing/PingReplyFields`, as its own `monitor()` does.
    ///
    /// # Arguments
    ///
    /// * `m` - The state of the alarms that depend on earlier samples, which
    ///         the method reads and updates.
    /// * `t` - This sample's time, in seconds.
    ///
    /// # Returns
    ///
    /// The alarm level of each field with an alarm, `None` when the field is
    /// absent.
    pub fn monitor(&self, m: &mut AlarmMonitor, t: f64) -> pk::Alarms<0> {
        let mut out = pk::Alarms::new();
        match self {
            Self::PingReply(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
        }
        out
    }
}

impl From<PingReply> for PingReplyFieldsKind {
    fn from(p: PingReply) -> Self {
        Self::PingReply(p)
    }
}
