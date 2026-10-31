// @generated automatically by tcs-xtce from tcslibgs/xtce/link.xml
// (SpaceSystem 'TcsLink').
// Any changes made to this file will be lost when it is regenerated;
// change the XTCE document instead and rerun `python3 -m tcs_xtce`.
// tcs-xtce: https://github.com/vomlehn/tcs-xtce
// Rust crate: https://crates.io/crates/tcs-xtce
//! Packet codecs for the `TcsLink` mission.
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
use tcs_xtce::packet::{
    self as pk, BitReader, BitWriter, IntEncoding, Packet, PacketError, Result,
    StringSpec,
};

/// Enumeration `/TcsLink/StatusType`; holds the raw value so unlisted values
/// survive decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StatusType(pub i64);

impl StatusType {
    pub const SUCCESS: Self = Self(0);
    pub const FAILURE: Self = Self(1);
    pub const INVALID_COMMAND: Self = Self(2);
    pub const INVALID_PARAMETER: Self = Self(3);
    pub const NOT_ARMED: Self = Self(4);
    pub const NOT_FOUND: Self = Self(5);
    pub const ALREADY_EXISTS: Self = Self(6);
    pub const TIMEOUT: Self = Self(7);
    pub const STALE: Self = Self(9);
    pub const UNMATCHED: Self = Self(10);

    /// Names this value of enumeration `/TcsLink/StatusType`.
    ///
    /// # Returns
    ///
    /// The value's label, or `None` if the enumeration doesn't list it.
    pub fn label(self) -> Option<&'static str> {
        match self.0 {
            0 => Some("Success"),
            1 => Some("Failure"),
            2 => Some("InvalidCommand"),
            3 => Some("InvalidParameter"),
            4 => Some("NotArmed"),
            5 => Some("NotFound"),
            6 => Some("AlreadyExists"),
            7 => Some("Timeout"),
            9 => Some("Stale"),
            10 => Some("Unmatched"),
            _ => None,
        }
    }
}

impl fmt::Display for StatusType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.label() {
            Some(l) => f.write_str(l),
            None => write!(f, "{}", self.0),
        }
    }
}

/// The functions the application supplies for the document's custom
/// algorithms: set them before decoding or encoding a packet that needs
/// them, or it fails with `NoAlgorithm`.
pub mod hooks {}

/// The alarm state of each parameter whose alarms depend on earlier
/// samples, which the packets' `monitor()` reads and updates.
#[derive(Debug, Clone, Default)]
pub struct AlarmMonitor {}

/// Sequence container `/TcsLink/PingAnswer`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 0, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Kind` = 129.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PingAnswer {
    /// `/TcsLink/SeqCount` (/TcsLink/U14Type)
    pub seq_count: u16,
    /// `/TcsLink/Sequence` (/TcsLink/U32Type)
    pub sequence: u32,
    /// `/TcsLink/Generation` (/TcsLink/U32Type)
    pub generation: u32,
    /// `/TcsLink/Status` (/TcsLink/StatusType)
    pub status: StatusType,
    /// `/TcsLink/Timestamp` (/TcsLink/U64Type)
    pub timestamp: u64,
}

impl PingAnswer {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/PingAnswer` packet's fields.
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

    /// Evaluates the alarms of this `/TcsLink/PingAnswer` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/PingAnswer` packet
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

impl Packet for PingAnswer {
    const NAME: &'static str = "/TcsLink/PingAnswer";
    const MAX_BYTES: usize = 24;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = r.uint_order(3, false, "Version")? as u8;
        let v2 = r.uint_order(1, false, "PacketType")? as u8;
        let v3 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v4 = r.uint_order(11, false, "APID")? as u16;
        let v5 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v6 = r.uint_order(14, false, "SeqCount")? as u16;
        let v7 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v7 as u64, -1, "PacketLength")?;
        let v8 = r.uint_order(8, false, "Kind")? as u8;
        let v9 = r.uint_order(32, false, "Sequence")? as u32;
        let v10 = r.uint_order(32, false, "Generation")? as u32;
        let v11 = StatusType(r.uint_order(8, false, "Status")? as i64);
        let v12 = r.uint_order(64, false, "Timestamp")? as u64;
        if !((v1 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/Version == 0",
            });
        }
        if !((v2 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/PacketType == 0",
            });
        }
        if !((v3 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SecHdrFlag == 0",
            });
        }
        if !((v4 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/APID == 256",
            });
        }
        if !((v5 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SeqFlags == 3",
            });
        }
        if !((v8 as i128) == 129) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/PingAnswer",
                criterion: "/TcsLink/Kind == 129",
            });
        }
        r.finish("PacketLength")?;
        Ok((
            PingAnswer {
                seq_count: v6,
                sequence: v9,
                generation: v10,
                status: v11,
                timestamp: v12,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l7 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(129 as u64, 8, "Kind")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.generation as u64, 32, "Generation")?,
        );
        w.uint_order(
            8,
            false,
            pk::from_signed(self.status.0, 8, IntEncoding::Unsigned, "Status")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.timestamp as u64, 64, "Timestamp")?,
        );
        w.patch_length(l7, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for PingAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PingAnswer{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", generation=")?;
        write!(f, "{}", self.generation)?;
        f.write_str(", status=")?;
        write!(f, "{}", self.status)?;
        f.write_str(", timestamp=")?;
        write!(f, "{}", self.timestamp)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsLink/RestartArmAnswer`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 0, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Kind` = 130.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RestartArmAnswer {
    /// `/TcsLink/SeqCount` (/TcsLink/U14Type)
    pub seq_count: u16,
    /// `/TcsLink/Sequence` (/TcsLink/U32Type)
    pub sequence: u32,
    /// `/TcsLink/Generation` (/TcsLink/U32Type)
    pub generation: u32,
    /// `/TcsLink/Status` (/TcsLink/StatusType)
    pub status: StatusType,
    /// `/TcsLink/Timestamp` (/TcsLink/U64Type)
    pub timestamp: u64,
}

impl RestartArmAnswer {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/RestartArmAnswer` packet's
    /// fields. Alarms that depend on earlier samples count only this one (see
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

    /// Evaluates the alarms of this `/TcsLink/RestartArmAnswer` packet's fields
    /// like `alarms()`, counting the earlier samples for change ranges,
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/RestartArmAnswer` packet
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

impl Packet for RestartArmAnswer {
    const NAME: &'static str = "/TcsLink/RestartArmAnswer";
    const MAX_BYTES: usize = 24;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = r.uint_order(3, false, "Version")? as u8;
        let v2 = r.uint_order(1, false, "PacketType")? as u8;
        let v3 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v4 = r.uint_order(11, false, "APID")? as u16;
        let v5 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v6 = r.uint_order(14, false, "SeqCount")? as u16;
        let v7 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v7 as u64, -1, "PacketLength")?;
        let v8 = r.uint_order(8, false, "Kind")? as u8;
        let v9 = r.uint_order(32, false, "Sequence")? as u32;
        let v10 = r.uint_order(32, false, "Generation")? as u32;
        let v11 = StatusType(r.uint_order(8, false, "Status")? as i64);
        let v12 = r.uint_order(64, false, "Timestamp")? as u64;
        if !((v1 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/Version == 0",
            });
        }
        if !((v2 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/PacketType == 0",
            });
        }
        if !((v3 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SecHdrFlag == 0",
            });
        }
        if !((v4 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/APID == 256",
            });
        }
        if !((v5 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SeqFlags == 3",
            });
        }
        if !((v8 as i128) == 130) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/RestartArmAnswer",
                criterion: "/TcsLink/Kind == 130",
            });
        }
        r.finish("PacketLength")?;
        Ok((
            RestartArmAnswer {
                seq_count: v6,
                sequence: v9,
                generation: v10,
                status: v11,
                timestamp: v12,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l7 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(130 as u64, 8, "Kind")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.generation as u64, 32, "Generation")?,
        );
        w.uint_order(
            8,
            false,
            pk::from_signed(self.status.0, 8, IntEncoding::Unsigned, "Status")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.timestamp as u64, 64, "Timestamp")?,
        );
        w.patch_length(l7, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for RestartArmAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RestartArmAnswer{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", generation=")?;
        write!(f, "{}", self.generation)?;
        f.write_str(", status=")?;
        write!(f, "{}", self.status)?;
        f.write_str(", timestamp=")?;
        write!(f, "{}", self.timestamp)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsLink/RestartAnswer`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 0, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Kind` = 131.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RestartAnswer {
    /// `/TcsLink/SeqCount` (/TcsLink/U14Type)
    pub seq_count: u16,
    /// `/TcsLink/Sequence` (/TcsLink/U32Type)
    pub sequence: u32,
    /// `/TcsLink/Generation` (/TcsLink/U32Type)
    pub generation: u32,
    /// `/TcsLink/Status` (/TcsLink/StatusType)
    pub status: StatusType,
    /// `/TcsLink/Timestamp` (/TcsLink/U64Type)
    pub timestamp: u64,
}

impl RestartAnswer {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/RestartAnswer` packet's fields.
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

    /// Evaluates the alarms of this `/TcsLink/RestartAnswer` packet's fields
    /// like `alarms()`, counting the earlier samples for change ranges,
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/RestartAnswer` packet
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

impl Packet for RestartAnswer {
    const NAME: &'static str = "/TcsLink/RestartAnswer";
    const MAX_BYTES: usize = 24;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = r.uint_order(3, false, "Version")? as u8;
        let v2 = r.uint_order(1, false, "PacketType")? as u8;
        let v3 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v4 = r.uint_order(11, false, "APID")? as u16;
        let v5 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v6 = r.uint_order(14, false, "SeqCount")? as u16;
        let v7 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v7 as u64, -1, "PacketLength")?;
        let v8 = r.uint_order(8, false, "Kind")? as u8;
        let v9 = r.uint_order(32, false, "Sequence")? as u32;
        let v10 = r.uint_order(32, false, "Generation")? as u32;
        let v11 = StatusType(r.uint_order(8, false, "Status")? as i64);
        let v12 = r.uint_order(64, false, "Timestamp")? as u64;
        if !((v1 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/Version == 0",
            });
        }
        if !((v2 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/PacketType == 0",
            });
        }
        if !((v3 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SecHdrFlag == 0",
            });
        }
        if !((v4 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/APID == 256",
            });
        }
        if !((v5 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SeqFlags == 3",
            });
        }
        if !((v8 as i128) == 131) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/RestartAnswer",
                criterion: "/TcsLink/Kind == 131",
            });
        }
        r.finish("PacketLength")?;
        Ok((
            RestartAnswer {
                seq_count: v6,
                sequence: v9,
                generation: v10,
                status: v11,
                timestamp: v12,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l7 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(131 as u64, 8, "Kind")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.generation as u64, 32, "Generation")?,
        );
        w.uint_order(
            8,
            false,
            pk::from_signed(self.status.0, 8, IntEncoding::Unsigned, "Status")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.timestamp as u64, 64, "Timestamp")?,
        );
        w.patch_length(l7, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for RestartAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RestartAnswer{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", generation=")?;
        write!(f, "{}", self.generation)?;
        f.write_str(", status=")?;
        write!(f, "{}", self.status)?;
        f.write_str(", timestamp=")?;
        write!(f, "{}", self.timestamp)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsLink/ConnectAnswer`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 0, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Kind` = 132.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConnectAnswer {
    /// `/TcsLink/SeqCount` (/TcsLink/U14Type)
    pub seq_count: u16,
    /// `/TcsLink/Sequence` (/TcsLink/U32Type)
    pub sequence: u32,
    /// `/TcsLink/Generation` (/TcsLink/U32Type)
    pub generation: u32,
    /// `/TcsLink/Status` (/TcsLink/StatusType)
    pub status: StatusType,
    /// `/TcsLink/Timestamp` (/TcsLink/U64Type)
    pub timestamp: u64,
    /// `/TcsLink/Major` (/TcsLink/U8Type)
    pub major: u8,
    /// `/TcsLink/Minor` (/TcsLink/U8Type)
    pub minor: u8,
    /// `/TcsLink/Patch` (/TcsLink/U8Type)
    pub patch: u8,
    /// `/TcsLink/Digest` (/TcsLink/DigestType)
    pub digest: pk::FixedBytes<16>,
    /// `/TcsLink/NewestTaken` (/TcsLink/U32Type)
    pub newest_taken: u32,
}

impl ConnectAnswer {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/ConnectAnswer` packet's fields.
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

    /// Evaluates the alarms of this `/TcsLink/ConnectAnswer` packet's fields
    /// like `alarms()`, counting the earlier samples for change ranges,
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/ConnectAnswer` packet
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

impl Packet for ConnectAnswer {
    const NAME: &'static str = "/TcsLink/ConnectAnswer";
    const MAX_BYTES: usize = 47;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = r.uint_order(3, false, "Version")? as u8;
        let v2 = r.uint_order(1, false, "PacketType")? as u8;
        let v3 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v4 = r.uint_order(11, false, "APID")? as u16;
        let v5 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v6 = r.uint_order(14, false, "SeqCount")? as u16;
        let v7 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v7 as u64, -1, "PacketLength")?;
        let v8 = r.uint_order(8, false, "Kind")? as u8;
        let v9 = r.uint_order(32, false, "Sequence")? as u32;
        let v10 = r.uint_order(32, false, "Generation")? as u32;
        let v11 = StatusType(r.uint_order(8, false, "Status")? as i64);
        let v12 = r.uint_order(64, false, "Timestamp")? as u64;
        if !((v1 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/Version == 0",
            });
        }
        if !((v2 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/PacketType == 0",
            });
        }
        if !((v3 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SecHdrFlag == 0",
            });
        }
        if !((v4 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/APID == 256",
            });
        }
        if !((v5 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SeqFlags == 3",
            });
        }
        if !((v8 as i128) == 132) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/ConnectAnswer",
                criterion: "/TcsLink/Kind == 132",
            });
        }
        let v13 = r.uint_order(8, false, "Major")? as u8;
        let v14 = r.uint_order(8, false, "Minor")? as u8;
        let v15 = r.uint_order(8, false, "Patch")? as u8;
        let v16 = r.fixed_bytes::<16>(16, "Digest")?;
        let v17 = r.uint_order(32, false, "NewestTaken")? as u32;
        r.finish("PacketLength")?;
        Ok((
            ConnectAnswer {
                seq_count: v6,
                sequence: v9,
                generation: v10,
                status: v11,
                timestamp: v12,
                major: v13,
                minor: v14,
                patch: v15,
                digest: v16,
                newest_taken: v17,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l7 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(132 as u64, 8, "Kind")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.generation as u64, 32, "Generation")?,
        );
        w.uint_order(
            8,
            false,
            pk::from_signed(self.status.0, 8, IntEncoding::Unsigned, "Status")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.timestamp as u64, 64, "Timestamp")?,
        );
        w.uint_order(
            8,
            false,
            pk::from_unsigned(self.major as u64, 8, "Major")?,
        );
        w.uint_order(
            8,
            false,
            pk::from_unsigned(self.minor as u64, 8, "Minor")?,
        );
        w.uint_order(
            8,
            false,
            pk::from_unsigned(self.patch as u64, 8, "Patch")?,
        );
        pk::write_binary(w, &self.digest, 16, "Digest")?;
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.newest_taken as u64, 32, "NewestTaken")?,
        );
        w.patch_length(l7, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for ConnectAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ConnectAnswer{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", generation=")?;
        write!(f, "{}", self.generation)?;
        f.write_str(", status=")?;
        write!(f, "{}", self.status)?;
        f.write_str(", timestamp=")?;
        write!(f, "{}", self.timestamp)?;
        f.write_str(", major=")?;
        write!(f, "{}", self.major)?;
        f.write_str(", minor=")?;
        write!(f, "{}", self.minor)?;
        f.write_str(", patch=")?;
        write!(f, "{}", self.patch)?;
        f.write_str(", digest=")?;
        pk::describe_bytes(&self.digest, f)?;
        f.write_str(", newest_taken=")?;
        write!(f, "{}", self.newest_taken)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsLink/StartNodeAnswer`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 0, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Kind` = 48.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StartNodeAnswer {
    /// `/TcsLink/SeqCount` (/TcsLink/U14Type)
    pub seq_count: u16,
    /// `/TcsLink/Sequence` (/TcsLink/U32Type)
    pub sequence: u32,
    /// `/TcsLink/Generation` (/TcsLink/U32Type)
    pub generation: u32,
    /// `/TcsLink/Status` (/TcsLink/StatusType)
    pub status: StatusType,
    /// `/TcsLink/Timestamp` (/TcsLink/U64Type)
    pub timestamp: u64,
    /// `/TcsLink/Node` (/TcsLink/U32Type)
    pub node: u32,
}

impl StartNodeAnswer {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/StartNodeAnswer` packet's fields.
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

    /// Evaluates the alarms of this `/TcsLink/StartNodeAnswer` packet's fields
    /// like `alarms()`, counting the earlier samples for change ranges,
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/StartNodeAnswer` packet
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

impl Packet for StartNodeAnswer {
    const NAME: &'static str = "/TcsLink/StartNodeAnswer";
    const MAX_BYTES: usize = 28;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = r.uint_order(3, false, "Version")? as u8;
        let v2 = r.uint_order(1, false, "PacketType")? as u8;
        let v3 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v4 = r.uint_order(11, false, "APID")? as u16;
        let v5 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v6 = r.uint_order(14, false, "SeqCount")? as u16;
        let v7 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v7 as u64, -1, "PacketLength")?;
        let v8 = r.uint_order(8, false, "Kind")? as u8;
        let v9 = r.uint_order(32, false, "Sequence")? as u32;
        let v10 = r.uint_order(32, false, "Generation")? as u32;
        let v11 = StatusType(r.uint_order(8, false, "Status")? as i64);
        let v12 = r.uint_order(64, false, "Timestamp")? as u64;
        if !((v1 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/Version == 0",
            });
        }
        if !((v2 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/PacketType == 0",
            });
        }
        if !((v3 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SecHdrFlag == 0",
            });
        }
        if !((v4 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/APID == 256",
            });
        }
        if !((v5 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SeqFlags == 3",
            });
        }
        if !((v8 as i128) == 48) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/StartNodeAnswer",
                criterion: "/TcsLink/Kind == 48",
            });
        }
        let v18 = r.uint_order(32, false, "Node")? as u32;
        r.finish("PacketLength")?;
        Ok((
            StartNodeAnswer {
                seq_count: v6,
                sequence: v9,
                generation: v10,
                status: v11,
                timestamp: v12,
                node: v18,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l7 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(48 as u64, 8, "Kind")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.generation as u64, 32, "Generation")?,
        );
        w.uint_order(
            8,
            false,
            pk::from_signed(self.status.0, 8, IntEncoding::Unsigned, "Status")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.timestamp as u64, 64, "Timestamp")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.node as u64, 32, "Node")?,
        );
        w.patch_length(l7, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for StartNodeAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StartNodeAnswer{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", generation=")?;
        write!(f, "{}", self.generation)?;
        f.write_str(", status=")?;
        write!(f, "{}", self.status)?;
        f.write_str(", timestamp=")?;
        write!(f, "{}", self.timestamp)?;
        f.write_str(", node=")?;
        write!(f, "{}", self.node)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsLink/StopNodeAnswer`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 0, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Kind` = 49.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StopNodeAnswer {
    /// `/TcsLink/SeqCount` (/TcsLink/U14Type)
    pub seq_count: u16,
    /// `/TcsLink/Sequence` (/TcsLink/U32Type)
    pub sequence: u32,
    /// `/TcsLink/Generation` (/TcsLink/U32Type)
    pub generation: u32,
    /// `/TcsLink/Status` (/TcsLink/StatusType)
    pub status: StatusType,
    /// `/TcsLink/Timestamp` (/TcsLink/U64Type)
    pub timestamp: u64,
    /// `/TcsLink/Node` (/TcsLink/U32Type)
    pub node: u32,
}

impl StopNodeAnswer {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/StopNodeAnswer` packet's fields.
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

    /// Evaluates the alarms of this `/TcsLink/StopNodeAnswer` packet's fields
    /// like `alarms()`, counting the earlier samples for change ranges,
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/StopNodeAnswer` packet
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

impl Packet for StopNodeAnswer {
    const NAME: &'static str = "/TcsLink/StopNodeAnswer";
    const MAX_BYTES: usize = 28;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = r.uint_order(3, false, "Version")? as u8;
        let v2 = r.uint_order(1, false, "PacketType")? as u8;
        let v3 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v4 = r.uint_order(11, false, "APID")? as u16;
        let v5 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v6 = r.uint_order(14, false, "SeqCount")? as u16;
        let v7 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v7 as u64, -1, "PacketLength")?;
        let v8 = r.uint_order(8, false, "Kind")? as u8;
        let v9 = r.uint_order(32, false, "Sequence")? as u32;
        let v10 = r.uint_order(32, false, "Generation")? as u32;
        let v11 = StatusType(r.uint_order(8, false, "Status")? as i64);
        let v12 = r.uint_order(64, false, "Timestamp")? as u64;
        if !((v1 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/Version == 0",
            });
        }
        if !((v2 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/PacketType == 0",
            });
        }
        if !((v3 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SecHdrFlag == 0",
            });
        }
        if !((v4 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/APID == 256",
            });
        }
        if !((v5 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SeqFlags == 3",
            });
        }
        if !((v8 as i128) == 49) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/StopNodeAnswer",
                criterion: "/TcsLink/Kind == 49",
            });
        }
        let v19 = r.uint_order(32, false, "Node")? as u32;
        r.finish("PacketLength")?;
        Ok((
            StopNodeAnswer {
                seq_count: v6,
                sequence: v9,
                generation: v10,
                status: v11,
                timestamp: v12,
                node: v19,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l7 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(49 as u64, 8, "Kind")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.generation as u64, 32, "Generation")?,
        );
        w.uint_order(
            8,
            false,
            pk::from_signed(self.status.0, 8, IntEncoding::Unsigned, "Status")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.timestamp as u64, 64, "Timestamp")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.node as u64, 32, "Node")?,
        );
        w.patch_length(l7, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for StopNodeAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StopNodeAnswer{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", generation=")?;
        write!(f, "{}", self.generation)?;
        f.write_str(", status=")?;
        write!(f, "{}", self.status)?;
        f.write_str(", timestamp=")?;
        write!(f, "{}", self.timestamp)?;
        f.write_str(", node=")?;
        write!(f, "{}", self.node)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsLink/SetNodeAnswer`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 0, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Kind` = 50.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SetNodeAnswer {
    /// `/TcsLink/SeqCount` (/TcsLink/U14Type)
    pub seq_count: u16,
    /// `/TcsLink/Sequence` (/TcsLink/U32Type)
    pub sequence: u32,
    /// `/TcsLink/Generation` (/TcsLink/U32Type)
    pub generation: u32,
    /// `/TcsLink/Status` (/TcsLink/StatusType)
    pub status: StatusType,
    /// `/TcsLink/Timestamp` (/TcsLink/U64Type)
    pub timestamp: u64,
    /// `/TcsLink/Node` (/TcsLink/U32Type)
    pub node: u32,
    /// `/TcsLink/Reason` (/TcsLink/ReasonType)
    pub reason: pk::FixedString<160>,
}

impl SetNodeAnswer {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/SetNodeAnswer` packet's fields.
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

    /// Evaluates the alarms of this `/TcsLink/SetNodeAnswer` packet's fields
    /// like `alarms()`, counting the earlier samples for change ranges,
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/SetNodeAnswer` packet
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

impl Packet for SetNodeAnswer {
    const NAME: &'static str = "/TcsLink/SetNodeAnswer";
    const MAX_BYTES: usize = 189;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = r.uint_order(3, false, "Version")? as u8;
        let v2 = r.uint_order(1, false, "PacketType")? as u8;
        let v3 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v4 = r.uint_order(11, false, "APID")? as u16;
        let v5 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v6 = r.uint_order(14, false, "SeqCount")? as u16;
        let v7 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v7 as u64, -1, "PacketLength")?;
        let v8 = r.uint_order(8, false, "Kind")? as u8;
        let v9 = r.uint_order(32, false, "Sequence")? as u32;
        let v10 = r.uint_order(32, false, "Generation")? as u32;
        let v11 = StatusType(r.uint_order(8, false, "Status")? as i64);
        let v12 = r.uint_order(64, false, "Timestamp")? as u64;
        if !((v1 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/Version == 0",
            });
        }
        if !((v2 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/PacketType == 0",
            });
        }
        if !((v3 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SecHdrFlag == 0",
            });
        }
        if !((v4 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/APID == 256",
            });
        }
        if !((v5 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SeqFlags == 3",
            });
        }
        if !((v8 as i128) == 50) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/SetNodeAnswer",
                criterion: "/TcsLink/Kind == 50",
            });
        }
        let v20 = r.uint_order(32, false, "Node")? as u32;
        let v21 = pk::read_fixed_string::<160, 161>(
            &mut r,
            &StringSpec {
                fixed_bits: None,
                max_bits: 1288,
                terminator: None,
                size_tag_bits: 8,
                charset: pk::Charset::Utf8,
            },
            "Reason",
        )?;
        r.finish("PacketLength")?;
        Ok((
            SetNodeAnswer {
                seq_count: v6,
                sequence: v9,
                generation: v10,
                status: v11,
                timestamp: v12,
                node: v20,
                reason: v21,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l7 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(50 as u64, 8, "Kind")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.generation as u64, 32, "Generation")?,
        );
        w.uint_order(
            8,
            false,
            pk::from_signed(self.status.0, 8, IntEncoding::Unsigned, "Status")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.timestamp as u64, 64, "Timestamp")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.node as u64, 32, "Node")?,
        );
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: None,
                max_bits: 1288,
                terminator: None,
                size_tag_bits: 8,
                charset: pk::Charset::Utf8,
            },
            &self.reason,
            "Reason",
        )?;
        w.patch_length(l7, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for SetNodeAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SetNodeAnswer{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", generation=")?;
        write!(f, "{}", self.generation)?;
        f.write_str(", status=")?;
        write!(f, "{}", self.status)?;
        f.write_str(", timestamp=")?;
        write!(f, "{}", self.timestamp)?;
        f.write_str(", node=")?;
        write!(f, "{}", self.node)?;
        f.write_str(", reason=")?;
        pk::describe_str(&self.reason, f)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsLink/QueryNodeAnswer`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 0, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Kind` = 51.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QueryNodeAnswer {
    /// `/TcsLink/SeqCount` (/TcsLink/U14Type)
    pub seq_count: u16,
    /// `/TcsLink/Sequence` (/TcsLink/U32Type)
    pub sequence: u32,
    /// `/TcsLink/Generation` (/TcsLink/U32Type)
    pub generation: u32,
    /// `/TcsLink/Status` (/TcsLink/StatusType)
    pub status: StatusType,
    /// `/TcsLink/Timestamp` (/TcsLink/U64Type)
    pub timestamp: u64,
    /// `/TcsLink/Node` (/TcsLink/U32Type)
    pub node: u32,
    /// `/TcsLink/Enabled` (/TcsLink/FlagType)
    pub enabled: bool,
    /// `/TcsLink/TakenBytes` (/TcsLink/U64Type)
    pub taken_bytes: u64,
    /// `/TcsLink/Reads` (/TcsLink/U64Type)
    pub reads: u64,
    /// `/TcsLink/ReadsFailed` (/TcsLink/U64Type)
    pub reads_failed: u64,
    /// `/TcsLink/GivenBytes` (/TcsLink/U64Type)
    pub given_bytes: u64,
    /// `/TcsLink/Writes` (/TcsLink/U64Type)
    pub writes: u64,
    /// `/TcsLink/WritesFailed` (/TcsLink/U64Type)
    pub writes_failed: u64,
    /// `/TcsLink/Dropped` (/TcsLink/U64Type)
    pub dropped: u64,
    /// `/TcsLink/Asks` (/TcsLink/U64Type)
    pub asks: u64,
    /// `/TcsLink/AsksFailed` (/TcsLink/U64Type)
    pub asks_failed: u64,
    /// `/TcsLink/Strangers` (/TcsLink/U64Type)
    pub strangers: u64,
    /// `/TcsLink/MovedKnown` (/TcsLink/FlagType)
    pub moved_known: bool,
    /// `/TcsLink/LastMoved` (/TcsLink/U64Type)
    pub last_moved: u64,
}

impl QueryNodeAnswer {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/QueryNodeAnswer` packet's fields.
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

    /// Evaluates the alarms of this `/TcsLink/QueryNodeAnswer` packet's fields
    /// like `alarms()`, counting the earlier samples for change ranges,
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/QueryNodeAnswer` packet
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

impl Packet for QueryNodeAnswer {
    const NAME: &'static str = "/TcsLink/QueryNodeAnswer";
    const MAX_BYTES: usize = 118;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = r.uint_order(3, false, "Version")? as u8;
        let v2 = r.uint_order(1, false, "PacketType")? as u8;
        let v3 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v4 = r.uint_order(11, false, "APID")? as u16;
        let v5 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v6 = r.uint_order(14, false, "SeqCount")? as u16;
        let v7 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v7 as u64, -1, "PacketLength")?;
        let v8 = r.uint_order(8, false, "Kind")? as u8;
        let v9 = r.uint_order(32, false, "Sequence")? as u32;
        let v10 = r.uint_order(32, false, "Generation")? as u32;
        let v11 = StatusType(r.uint_order(8, false, "Status")? as i64);
        let v12 = r.uint_order(64, false, "Timestamp")? as u64;
        if !((v1 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/Version == 0",
            });
        }
        if !((v2 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/PacketType == 0",
            });
        }
        if !((v3 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SecHdrFlag == 0",
            });
        }
        if !((v4 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/APID == 256",
            });
        }
        if !((v5 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SeqFlags == 3",
            });
        }
        if !((v8 as i128) == 51) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/QueryNodeAnswer",
                criterion: "/TcsLink/Kind == 51",
            });
        }
        let v22 = r.uint_order(32, false, "Node")? as u32;
        let v23 = r.uint_order(8, false, "Enabled")? != 0;
        let v24 = r.uint_order(64, false, "TakenBytes")? as u64;
        let v25 = r.uint_order(64, false, "Reads")? as u64;
        let v26 = r.uint_order(64, false, "ReadsFailed")? as u64;
        let v27 = r.uint_order(64, false, "GivenBytes")? as u64;
        let v28 = r.uint_order(64, false, "Writes")? as u64;
        let v29 = r.uint_order(64, false, "WritesFailed")? as u64;
        let v30 = r.uint_order(64, false, "Dropped")? as u64;
        let v31 = r.uint_order(64, false, "Asks")? as u64;
        let v32 = r.uint_order(64, false, "AsksFailed")? as u64;
        let v33 = r.uint_order(64, false, "Strangers")? as u64;
        let v34 = r.uint_order(8, false, "MovedKnown")? != 0;
        let v35 = r.uint_order(64, false, "LastMoved")? as u64;
        r.finish("PacketLength")?;
        Ok((
            QueryNodeAnswer {
                seq_count: v6,
                sequence: v9,
                generation: v10,
                status: v11,
                timestamp: v12,
                node: v22,
                enabled: v23,
                taken_bytes: v24,
                reads: v25,
                reads_failed: v26,
                given_bytes: v27,
                writes: v28,
                writes_failed: v29,
                dropped: v30,
                asks: v31,
                asks_failed: v32,
                strangers: v33,
                moved_known: v34,
                last_moved: v35,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l7 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(51 as u64, 8, "Kind")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.generation as u64, 32, "Generation")?,
        );
        w.uint_order(
            8,
            false,
            pk::from_signed(self.status.0, 8, IntEncoding::Unsigned, "Status")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.timestamp as u64, 64, "Timestamp")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.node as u64, 32, "Node")?,
        );
        w.uint_order(8, false, self.enabled as u64);
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.taken_bytes as u64, 64, "TakenBytes")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.reads as u64, 64, "Reads")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.reads_failed as u64, 64, "ReadsFailed")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.given_bytes as u64, 64, "GivenBytes")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.writes as u64, 64, "Writes")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.writes_failed as u64, 64, "WritesFailed")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.dropped as u64, 64, "Dropped")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.asks as u64, 64, "Asks")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.asks_failed as u64, 64, "AsksFailed")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.strangers as u64, 64, "Strangers")?,
        );
        w.uint_order(8, false, self.moved_known as u64);
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.last_moved as u64, 64, "LastMoved")?,
        );
        w.patch_length(l7, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for QueryNodeAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("QueryNodeAnswer{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", generation=")?;
        write!(f, "{}", self.generation)?;
        f.write_str(", status=")?;
        write!(f, "{}", self.status)?;
        f.write_str(", timestamp=")?;
        write!(f, "{}", self.timestamp)?;
        f.write_str(", node=")?;
        write!(f, "{}", self.node)?;
        f.write_str(", enabled=")?;
        write!(f, "{}", self.enabled)?;
        f.write_str(", taken_bytes=")?;
        write!(f, "{}", self.taken_bytes)?;
        f.write_str(", reads=")?;
        write!(f, "{}", self.reads)?;
        f.write_str(", reads_failed=")?;
        write!(f, "{}", self.reads_failed)?;
        f.write_str(", given_bytes=")?;
        write!(f, "{}", self.given_bytes)?;
        f.write_str(", writes=")?;
        write!(f, "{}", self.writes)?;
        f.write_str(", writes_failed=")?;
        write!(f, "{}", self.writes_failed)?;
        f.write_str(", dropped=")?;
        write!(f, "{}", self.dropped)?;
        f.write_str(", asks=")?;
        write!(f, "{}", self.asks)?;
        f.write_str(", asks_failed=")?;
        write!(f, "{}", self.asks_failed)?;
        f.write_str(", strangers=")?;
        write!(f, "{}", self.strangers)?;
        f.write_str(", moved_known=")?;
        write!(f, "{}", self.moved_known)?;
        f.write_str(", last_moved=")?;
        write!(f, "{}", self.last_moved)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsLink/QueryNodeSampleAnswer`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 0, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Kind` = 52.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QueryNodeSampleAnswer {
    /// `/TcsLink/SeqCount` (/TcsLink/U14Type)
    pub seq_count: u16,
    /// `/TcsLink/Sequence` (/TcsLink/U32Type)
    pub sequence: u32,
    /// `/TcsLink/Generation` (/TcsLink/U32Type)
    pub generation: u32,
    /// `/TcsLink/Status` (/TcsLink/StatusType)
    pub status: StatusType,
    /// `/TcsLink/Timestamp` (/TcsLink/U64Type)
    pub timestamp: u64,
    /// `/TcsLink/Node` (/TcsLink/U32Type)
    pub node: u32,
    /// `/TcsLink/TakenKnown` (/TcsLink/FlagType)
    pub taken_known: bool,
    /// `/TcsLink/TakenTime` (/TcsLink/U64Type)
    pub taken_time: u64,
    /// `/TcsLink/TakenLength` (/TcsLink/U8Type)
    pub taken_length: u8,
    /// `/TcsLink/TakenTotal` (/TcsLink/U32Type)
    pub taken_total: u32,
    /// `/TcsLink/TakenBytesShown` (/TcsLink/SampleBytesType)
    pub taken_bytes_shown: pk::FixedBytes<8>,
    /// `/TcsLink/GivenKnown` (/TcsLink/FlagType)
    pub given_known: bool,
    /// `/TcsLink/GivenTime` (/TcsLink/U64Type)
    pub given_time: u64,
    /// `/TcsLink/GivenLength` (/TcsLink/U8Type)
    pub given_length: u8,
    /// `/TcsLink/GivenTotal` (/TcsLink/U32Type)
    pub given_total: u32,
    /// `/TcsLink/GivenBytesShown` (/TcsLink/SampleBytesType)
    pub given_bytes_shown: pk::FixedBytes<8>,
}

impl QueryNodeSampleAnswer {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/QueryNodeSampleAnswer` packet's
    /// fields. Alarms that depend on earlier samples count only this one (see
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

    /// Evaluates the alarms of this `/TcsLink/QueryNodeSampleAnswer` packet's
    /// fields like `alarms()`, counting the earlier samples for change ranges,
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/QueryNodeSampleAnswer`
    /// packet triggers, from this one's values.
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

impl Packet for QueryNodeSampleAnswer {
    const NAME: &'static str = "/TcsLink/QueryNodeSampleAnswer";
    const MAX_BYTES: usize = 72;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = r.uint_order(3, false, "Version")? as u8;
        let v2 = r.uint_order(1, false, "PacketType")? as u8;
        let v3 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v4 = r.uint_order(11, false, "APID")? as u16;
        let v5 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v6 = r.uint_order(14, false, "SeqCount")? as u16;
        let v7 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v7 as u64, -1, "PacketLength")?;
        let v8 = r.uint_order(8, false, "Kind")? as u8;
        let v9 = r.uint_order(32, false, "Sequence")? as u32;
        let v10 = r.uint_order(32, false, "Generation")? as u32;
        let v11 = StatusType(r.uint_order(8, false, "Status")? as i64);
        let v12 = r.uint_order(64, false, "Timestamp")? as u64;
        if !((v1 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/Version == 0",
            });
        }
        if !((v2 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/PacketType == 0",
            });
        }
        if !((v3 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SecHdrFlag == 0",
            });
        }
        if !((v4 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/APID == 256",
            });
        }
        if !((v5 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Answer",
                criterion: "/TcsLink/SeqFlags == 3",
            });
        }
        if !((v8 as i128) == 52) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/QueryNodeSampleAnswer",
                criterion: "/TcsLink/Kind == 52",
            });
        }
        let v36 = r.uint_order(32, false, "Node")? as u32;
        let v37 = r.uint_order(8, false, "TakenKnown")? != 0;
        let v38 = r.uint_order(64, false, "TakenTime")? as u64;
        let v39 = r.uint_order(8, false, "TakenLength")? as u8;
        let v40 = r.uint_order(32, false, "TakenTotal")? as u32;
        let v41 = r.fixed_bytes::<8>(8, "TakenBytesShown")?;
        let v42 = r.uint_order(8, false, "GivenKnown")? != 0;
        let v43 = r.uint_order(64, false, "GivenTime")? as u64;
        let v44 = r.uint_order(8, false, "GivenLength")? as u8;
        let v45 = r.uint_order(32, false, "GivenTotal")? as u32;
        let v46 = r.fixed_bytes::<8>(8, "GivenBytesShown")?;
        r.finish("PacketLength")?;
        Ok((
            QueryNodeSampleAnswer {
                seq_count: v6,
                sequence: v9,
                generation: v10,
                status: v11,
                timestamp: v12,
                node: v36,
                taken_known: v37,
                taken_time: v38,
                taken_length: v39,
                taken_total: v40,
                taken_bytes_shown: v41,
                given_known: v42,
                given_time: v43,
                given_length: v44,
                given_total: v45,
                given_bytes_shown: v46,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l7 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(52 as u64, 8, "Kind")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.generation as u64, 32, "Generation")?,
        );
        w.uint_order(
            8,
            false,
            pk::from_signed(self.status.0, 8, IntEncoding::Unsigned, "Status")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.timestamp as u64, 64, "Timestamp")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.node as u64, 32, "Node")?,
        );
        w.uint_order(8, false, self.taken_known as u64);
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.taken_time as u64, 64, "TakenTime")?,
        );
        w.uint_order(
            8,
            false,
            pk::from_unsigned(self.taken_length as u64, 8, "TakenLength")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.taken_total as u64, 32, "TakenTotal")?,
        );
        pk::write_binary(w, &self.taken_bytes_shown, 8, "TakenBytesShown")?;
        w.uint_order(8, false, self.given_known as u64);
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.given_time as u64, 64, "GivenTime")?,
        );
        w.uint_order(
            8,
            false,
            pk::from_unsigned(self.given_length as u64, 8, "GivenLength")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.given_total as u64, 32, "GivenTotal")?,
        );
        pk::write_binary(w, &self.given_bytes_shown, 8, "GivenBytesShown")?;
        w.patch_length(l7, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for QueryNodeSampleAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("QueryNodeSampleAnswer{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", generation=")?;
        write!(f, "{}", self.generation)?;
        f.write_str(", status=")?;
        write!(f, "{}", self.status)?;
        f.write_str(", timestamp=")?;
        write!(f, "{}", self.timestamp)?;
        f.write_str(", node=")?;
        write!(f, "{}", self.node)?;
        f.write_str(", taken_known=")?;
        write!(f, "{}", self.taken_known)?;
        f.write_str(", taken_time=")?;
        write!(f, "{}", self.taken_time)?;
        f.write_str(", taken_length=")?;
        write!(f, "{}", self.taken_length)?;
        f.write_str(", taken_total=")?;
        write!(f, "{}", self.taken_total)?;
        f.write_str(", taken_bytes_shown=")?;
        pk::describe_bytes(&self.taken_bytes_shown, f)?;
        f.write_str(", given_known=")?;
        write!(f, "{}", self.given_known)?;
        f.write_str(", given_time=")?;
        write!(f, "{}", self.given_time)?;
        f.write_str(", given_length=")?;
        write!(f, "{}", self.given_length)?;
        f.write_str(", given_total=")?;
        write!(f, "{}", self.given_total)?;
        f.write_str(", given_bytes_shown=")?;
        pk::describe_bytes(&self.given_bytes_shown, f)?;
        f.write_char('}')
    }
}

/// Meta-command `/TcsLink/Ping`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 1, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Opcode` = 1.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ping {
    /// `/TcsLink/Command#SeqCount` (/TcsLink/U14Arg)
    pub seq_count: u16,
    /// `/TcsLink/Command#Sequence` (/TcsLink/U32Arg)
    pub sequence: u32,
}

impl Ping {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/Ping` packet's fields. Alarms
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

    /// Evaluates the alarms of this `/TcsLink/Ping` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/Ping` packet triggers,
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
    const NAME: &'static str = "/TcsLink/Ping";
    const MAX_BYTES: usize = 11;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v47 = r.uint_order(3, false, "Version")? as u8;
        let v48 = r.uint_order(1, false, "PacketType")? as u8;
        let v49 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v50 = r.uint_order(11, false, "APID")? as u16;
        let v51 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v52 = r.uint_order(14, false, "SeqCount")? as u16;
        let v53 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v53 as u64, -1, "PacketLength")?;
        let v54 = r.uint_order(8, false, "Opcode")? as u8;
        let v55 = r.uint_order(32, false, "Sequence")? as u32;
        if !((v47 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "Version = 0",
            });
        }
        if !((v48 as i128) == 1) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "PacketType = 1",
            });
        }
        if !((v49 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SecHdrFlag = 0",
            });
        }
        if !((v50 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "APID = 256",
            });
        }
        if !((v51 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SeqFlags = 3",
            });
        }
        if !((v54 as i128) == 1) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Ping",
                criterion: "Opcode = 1",
            });
        }
        r.finish("PacketLength")?;
        Ok((
            Ping {
                seq_count: v52,
                sequence: v55,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(1 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l53 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(1 as u64, 8, "Opcode")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.patch_length(l53, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for Ping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Ping{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_char('}')
    }
}

/// Meta-command `/TcsLink/RestartArm`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 1, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Opcode` = 2.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RestartArm {
    /// `/TcsLink/Command#SeqCount` (/TcsLink/U14Arg)
    pub seq_count: u16,
    /// `/TcsLink/Command#Sequence` (/TcsLink/U32Arg)
    pub sequence: u32,
    /// `/TcsLink/RestartArm#ArmKey` (/TcsLink/U64Arg)
    pub arm_key: u64,
}

impl RestartArm {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/RestartArm` packet's fields.
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

    /// Evaluates the alarms of this `/TcsLink/RestartArm` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/RestartArm` packet
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

impl Packet for RestartArm {
    const NAME: &'static str = "/TcsLink/RestartArm";
    const MAX_BYTES: usize = 19;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v47 = r.uint_order(3, false, "Version")? as u8;
        let v48 = r.uint_order(1, false, "PacketType")? as u8;
        let v49 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v50 = r.uint_order(11, false, "APID")? as u16;
        let v51 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v52 = r.uint_order(14, false, "SeqCount")? as u16;
        let v53 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v53 as u64, -1, "PacketLength")?;
        let v54 = r.uint_order(8, false, "Opcode")? as u8;
        let v55 = r.uint_order(32, false, "Sequence")? as u32;
        if !((v47 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "Version = 0",
            });
        }
        if !((v48 as i128) == 1) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "PacketType = 1",
            });
        }
        if !((v49 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SecHdrFlag = 0",
            });
        }
        if !((v50 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "APID = 256",
            });
        }
        if !((v51 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SeqFlags = 3",
            });
        }
        if !((v54 as i128) == 2) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/RestartArm",
                criterion: "Opcode = 2",
            });
        }
        let v56 = r.uint_order(64, false, "ArmKey")? as u64;
        r.finish("PacketLength")?;
        Ok((
            RestartArm {
                seq_count: v52,
                sequence: v55,
                arm_key: v56,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(1 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l53 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(2 as u64, 8, "Opcode")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.arm_key as u64, 64, "ArmKey")?,
        );
        w.patch_length(l53, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for RestartArm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RestartArm{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", arm_key=")?;
        write!(f, "{}", self.arm_key)?;
        f.write_char('}')
    }
}

/// Meta-command `/TcsLink/Restart`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 1, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Opcode` = 3.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Restart {
    /// `/TcsLink/Command#SeqCount` (/TcsLink/U14Arg)
    pub seq_count: u16,
    /// `/TcsLink/Command#Sequence` (/TcsLink/U32Arg)
    pub sequence: u32,
    /// `/TcsLink/Restart#ArmKey` (/TcsLink/U64Arg)
    pub arm_key: u64,
}

impl Restart {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/Restart` packet's fields. Alarms
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

    /// Evaluates the alarms of this `/TcsLink/Restart` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/Restart` packet triggers,
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

impl Packet for Restart {
    const NAME: &'static str = "/TcsLink/Restart";
    const MAX_BYTES: usize = 19;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v47 = r.uint_order(3, false, "Version")? as u8;
        let v48 = r.uint_order(1, false, "PacketType")? as u8;
        let v49 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v50 = r.uint_order(11, false, "APID")? as u16;
        let v51 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v52 = r.uint_order(14, false, "SeqCount")? as u16;
        let v53 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v53 as u64, -1, "PacketLength")?;
        let v54 = r.uint_order(8, false, "Opcode")? as u8;
        let v55 = r.uint_order(32, false, "Sequence")? as u32;
        if !((v47 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "Version = 0",
            });
        }
        if !((v48 as i128) == 1) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "PacketType = 1",
            });
        }
        if !((v49 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SecHdrFlag = 0",
            });
        }
        if !((v50 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "APID = 256",
            });
        }
        if !((v51 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SeqFlags = 3",
            });
        }
        if !((v54 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Restart",
                criterion: "Opcode = 3",
            });
        }
        let v57 = r.uint_order(64, false, "ArmKey")? as u64;
        r.finish("PacketLength")?;
        Ok((
            Restart {
                seq_count: v52,
                sequence: v55,
                arm_key: v57,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(1 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l53 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(3 as u64, 8, "Opcode")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.arm_key as u64, 64, "ArmKey")?,
        );
        w.patch_length(l53, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for Restart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Restart{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", arm_key=")?;
        write!(f, "{}", self.arm_key)?;
        f.write_char('}')
    }
}

/// Meta-command `/TcsLink/Connect`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 1, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Opcode` = 4.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Connect {
    /// `/TcsLink/Command#SeqCount` (/TcsLink/U14Arg)
    pub seq_count: u16,
    /// `/TcsLink/Command#Sequence` (/TcsLink/U32Arg)
    pub sequence: u32,
    /// `/TcsLink/Connect#Major` (/TcsLink/U8Arg)
    pub major: u8,
    /// `/TcsLink/Connect#Minor` (/TcsLink/U8Arg)
    pub minor: u8,
    /// `/TcsLink/Connect#Patch` (/TcsLink/U8Arg)
    pub patch: u8,
    /// `/TcsLink/Connect#Digest` (/TcsLink/DigestArg)
    pub digest: pk::FixedBytes<16>,
}

impl Connect {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/Connect` packet's fields. Alarms
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

    /// Evaluates the alarms of this `/TcsLink/Connect` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/Connect` packet triggers,
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

impl Packet for Connect {
    const NAME: &'static str = "/TcsLink/Connect";
    const MAX_BYTES: usize = 30;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v47 = r.uint_order(3, false, "Version")? as u8;
        let v48 = r.uint_order(1, false, "PacketType")? as u8;
        let v49 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v50 = r.uint_order(11, false, "APID")? as u16;
        let v51 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v52 = r.uint_order(14, false, "SeqCount")? as u16;
        let v53 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v53 as u64, -1, "PacketLength")?;
        let v54 = r.uint_order(8, false, "Opcode")? as u8;
        let v55 = r.uint_order(32, false, "Sequence")? as u32;
        if !((v47 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "Version = 0",
            });
        }
        if !((v48 as i128) == 1) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "PacketType = 1",
            });
        }
        if !((v49 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SecHdrFlag = 0",
            });
        }
        if !((v50 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "APID = 256",
            });
        }
        if !((v51 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SeqFlags = 3",
            });
        }
        if !((v54 as i128) == 4) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/Connect",
                criterion: "Opcode = 4",
            });
        }
        let v58 = r.uint_order(8, false, "Major")? as u8;
        let v59 = r.uint_order(8, false, "Minor")? as u8;
        let v60 = r.uint_order(8, false, "Patch")? as u8;
        let v61 = r.fixed_bytes::<16>(16, "Digest")?;
        r.finish("PacketLength")?;
        Ok((
            Connect {
                seq_count: v52,
                sequence: v55,
                major: v58,
                minor: v59,
                patch: v60,
                digest: v61,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(1 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l53 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(4 as u64, 8, "Opcode")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.uint_order(
            8,
            false,
            pk::from_unsigned(self.major as u64, 8, "Major")?,
        );
        w.uint_order(
            8,
            false,
            pk::from_unsigned(self.minor as u64, 8, "Minor")?,
        );
        w.uint_order(
            8,
            false,
            pk::from_unsigned(self.patch as u64, 8, "Patch")?,
        );
        pk::write_binary(w, &self.digest, 16, "Digest")?;
        w.patch_length(l53, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for Connect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Connect{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", major=")?;
        write!(f, "{}", self.major)?;
        f.write_str(", minor=")?;
        write!(f, "{}", self.minor)?;
        f.write_str(", patch=")?;
        write!(f, "{}", self.patch)?;
        f.write_str(", digest=")?;
        pk::describe_bytes(&self.digest, f)?;
        f.write_char('}')
    }
}

/// Meta-command `/TcsLink/StartNode`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 1, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Opcode` = 48.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StartNode {
    /// `/TcsLink/Command#SeqCount` (/TcsLink/U14Arg)
    pub seq_count: u16,
    /// `/TcsLink/Command#Sequence` (/TcsLink/U32Arg)
    pub sequence: u32,
    /// `/TcsLink/StartNode#Node` (/TcsLink/U32Arg)
    pub node: u32,
}

impl StartNode {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/StartNode` packet's fields.
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

    /// Evaluates the alarms of this `/TcsLink/StartNode` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/StartNode` packet
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

impl Packet for StartNode {
    const NAME: &'static str = "/TcsLink/StartNode";
    const MAX_BYTES: usize = 15;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v47 = r.uint_order(3, false, "Version")? as u8;
        let v48 = r.uint_order(1, false, "PacketType")? as u8;
        let v49 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v50 = r.uint_order(11, false, "APID")? as u16;
        let v51 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v52 = r.uint_order(14, false, "SeqCount")? as u16;
        let v53 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v53 as u64, -1, "PacketLength")?;
        let v54 = r.uint_order(8, false, "Opcode")? as u8;
        let v55 = r.uint_order(32, false, "Sequence")? as u32;
        if !((v47 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "Version = 0",
            });
        }
        if !((v48 as i128) == 1) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "PacketType = 1",
            });
        }
        if !((v49 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SecHdrFlag = 0",
            });
        }
        if !((v50 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "APID = 256",
            });
        }
        if !((v51 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SeqFlags = 3",
            });
        }
        if !((v54 as i128) == 48) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/StartNode",
                criterion: "Opcode = 48",
            });
        }
        let v62 = r.uint_order(32, false, "Node")? as u32;
        r.finish("PacketLength")?;
        Ok((
            StartNode {
                seq_count: v52,
                sequence: v55,
                node: v62,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(1 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l53 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(48 as u64, 8, "Opcode")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.node as u64, 32, "Node")?,
        );
        w.patch_length(l53, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for StartNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StartNode{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", node=")?;
        write!(f, "{}", self.node)?;
        f.write_char('}')
    }
}

/// Meta-command `/TcsLink/StopNode`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 1, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Opcode` = 49.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StopNode {
    /// `/TcsLink/Command#SeqCount` (/TcsLink/U14Arg)
    pub seq_count: u16,
    /// `/TcsLink/Command#Sequence` (/TcsLink/U32Arg)
    pub sequence: u32,
    /// `/TcsLink/StopNode#Node` (/TcsLink/U32Arg)
    pub node: u32,
}

impl StopNode {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/StopNode` packet's fields. Alarms
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

    /// Evaluates the alarms of this `/TcsLink/StopNode` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/StopNode` packet triggers,
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

impl Packet for StopNode {
    const NAME: &'static str = "/TcsLink/StopNode";
    const MAX_BYTES: usize = 15;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v47 = r.uint_order(3, false, "Version")? as u8;
        let v48 = r.uint_order(1, false, "PacketType")? as u8;
        let v49 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v50 = r.uint_order(11, false, "APID")? as u16;
        let v51 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v52 = r.uint_order(14, false, "SeqCount")? as u16;
        let v53 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v53 as u64, -1, "PacketLength")?;
        let v54 = r.uint_order(8, false, "Opcode")? as u8;
        let v55 = r.uint_order(32, false, "Sequence")? as u32;
        if !((v47 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "Version = 0",
            });
        }
        if !((v48 as i128) == 1) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "PacketType = 1",
            });
        }
        if !((v49 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SecHdrFlag = 0",
            });
        }
        if !((v50 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "APID = 256",
            });
        }
        if !((v51 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SeqFlags = 3",
            });
        }
        if !((v54 as i128) == 49) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/StopNode",
                criterion: "Opcode = 49",
            });
        }
        let v63 = r.uint_order(32, false, "Node")? as u32;
        r.finish("PacketLength")?;
        Ok((
            StopNode {
                seq_count: v52,
                sequence: v55,
                node: v63,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(1 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l53 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(49 as u64, 8, "Opcode")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.node as u64, 32, "Node")?,
        );
        w.patch_length(l53, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for StopNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StopNode{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", node=")?;
        write!(f, "{}", self.node)?;
        f.write_char('}')
    }
}

/// Meta-command `/TcsLink/SetNode`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 1, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Opcode` = 50.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
///
/// `ValueLength` gives the size of `Value`: computed from its length when
/// encoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SetNode {
    /// `/TcsLink/Command#SeqCount` (/TcsLink/U14Arg)
    pub seq_count: u16,
    /// `/TcsLink/Command#Sequence` (/TcsLink/U32Arg)
    pub sequence: u32,
    /// `/TcsLink/SetNode#Node` (/TcsLink/U32Arg)
    pub node: u32,
    /// `/TcsLink/SetNode#Attribute` (/TcsLink/AttributeArg)
    pub attribute: pk::FixedString<64>,
    /// `/TcsLink/SetNode#Value` (/TcsLink/ValueArg)
    pub value_: pk::FixedBytes<256>,
}

impl SetNode {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/SetNode` packet's fields. Alarms
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

    /// Evaluates the alarms of this `/TcsLink/SetNode` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/SetNode` packet triggers,
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

impl Packet for SetNode {
    const NAME: &'static str = "/TcsLink/SetNode";
    const MAX_BYTES: usize = 338;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v47 = r.uint_order(3, false, "Version")? as u8;
        let v48 = r.uint_order(1, false, "PacketType")? as u8;
        let v49 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v50 = r.uint_order(11, false, "APID")? as u16;
        let v51 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v52 = r.uint_order(14, false, "SeqCount")? as u16;
        let v53 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v53 as u64, -1, "PacketLength")?;
        let v54 = r.uint_order(8, false, "Opcode")? as u8;
        let v55 = r.uint_order(32, false, "Sequence")? as u32;
        if !((v47 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "Version = 0",
            });
        }
        if !((v48 as i128) == 1) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "PacketType = 1",
            });
        }
        if !((v49 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SecHdrFlag = 0",
            });
        }
        if !((v50 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "APID = 256",
            });
        }
        if !((v51 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SeqFlags = 3",
            });
        }
        if !((v54 as i128) == 50) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/SetNode",
                criterion: "Opcode = 50",
            });
        }
        let v64 = r.uint_order(32, false, "Node")? as u32;
        let v65 = pk::read_fixed_string::<64, 65>(
            &mut r,
            &StringSpec {
                fixed_bits: None,
                max_bits: 520,
                terminator: None,
                size_tag_bits: 8,
                charset: pk::Charset::Utf8,
            },
            "Attribute",
        )?;
        let v66 = r.uint_order(16, false, "ValueLength")? as u16;
        let v67 = r.fixed_bytes::<256>(
            pk::dynamic_size(v66 as f64, 8.0, 0.0, 8, 256, "Value")?,
            "Value",
        )?;
        r.finish("PacketLength")?;
        Ok((
            SetNode {
                seq_count: v52,
                sequence: v55,
                node: v64,
                attribute: v65,
                value_: v67,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        let d66 = u16::try_from(pk::size_source(
            self.value_.len(),
            8.0,
            0.0,
            8,
            1,
            "Value",
        )?)
        .map_err(|_| PacketError::OutOfRange {
            field: "ValueLength",
        })?;
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(1 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l53 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(50 as u64, 8, "Opcode")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.node as u64, 32, "Node")?,
        );
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: None,
                max_bits: 520,
                terminator: None,
                size_tag_bits: 8,
                charset: pk::Charset::Utf8,
            },
            &self.attribute,
            "Attribute",
        )?;
        w.uint_order(
            16,
            false,
            pk::from_unsigned(d66 as u64, 16, "ValueLength")?,
        );
        {
            let n = pk::dynamic_size(d66 as f64, 8.0, 0.0, 8, 256, "Value")?;
            if self.value_.len() != n {
                return Err(PacketError::BadSize { field: "Value" });
            }
            w.bytes(&self.value_);
        }
        w.patch_length(l53, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for SetNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SetNode{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", node=")?;
        write!(f, "{}", self.node)?;
        f.write_str(", attribute=")?;
        pk::describe_str(&self.attribute, f)?;
        f.write_str(", value_=")?;
        pk::describe_bytes(&self.value_, f)?;
        f.write_char('}')
    }
}

/// Meta-command `/TcsLink/QueryNode`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 1, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Opcode` = 51.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QueryNode {
    /// `/TcsLink/Command#SeqCount` (/TcsLink/U14Arg)
    pub seq_count: u16,
    /// `/TcsLink/Command#Sequence` (/TcsLink/U32Arg)
    pub sequence: u32,
    /// `/TcsLink/QueryNode#Node` (/TcsLink/U32Arg)
    pub node: u32,
}

impl QueryNode {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/QueryNode` packet's fields.
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

    /// Evaluates the alarms of this `/TcsLink/QueryNode` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/QueryNode` packet
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

impl Packet for QueryNode {
    const NAME: &'static str = "/TcsLink/QueryNode";
    const MAX_BYTES: usize = 15;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v47 = r.uint_order(3, false, "Version")? as u8;
        let v48 = r.uint_order(1, false, "PacketType")? as u8;
        let v49 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v50 = r.uint_order(11, false, "APID")? as u16;
        let v51 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v52 = r.uint_order(14, false, "SeqCount")? as u16;
        let v53 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v53 as u64, -1, "PacketLength")?;
        let v54 = r.uint_order(8, false, "Opcode")? as u8;
        let v55 = r.uint_order(32, false, "Sequence")? as u32;
        if !((v47 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "Version = 0",
            });
        }
        if !((v48 as i128) == 1) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "PacketType = 1",
            });
        }
        if !((v49 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SecHdrFlag = 0",
            });
        }
        if !((v50 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "APID = 256",
            });
        }
        if !((v51 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SeqFlags = 3",
            });
        }
        if !((v54 as i128) == 51) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/QueryNode",
                criterion: "Opcode = 51",
            });
        }
        let v68 = r.uint_order(32, false, "Node")? as u32;
        r.finish("PacketLength")?;
        Ok((
            QueryNode {
                seq_count: v52,
                sequence: v55,
                node: v68,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(1 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l53 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(51 as u64, 8, "Opcode")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.node as u64, 32, "Node")?,
        );
        w.patch_length(l53, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for QueryNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("QueryNode{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", node=")?;
        write!(f, "{}", self.node)?;
        f.write_char('}')
    }
}

/// Meta-command `/TcsLink/QueryNodeSample`.
///
/// Fixed by its inheritance: `Version` = 0, `PacketType` = 1, `SecHdrFlag` = 0,
/// `APID` = 256, `SeqFlags` = 3, `Opcode` = 52.
///
/// `PacketLength` is the packet length: computed when encoding, checked when
/// decoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QueryNodeSample {
    /// `/TcsLink/Command#SeqCount` (/TcsLink/U14Arg)
    pub seq_count: u16,
    /// `/TcsLink/Command#Sequence` (/TcsLink/U32Arg)
    pub sequence: u32,
    /// `/TcsLink/QueryNodeSample#Node` (/TcsLink/U32Arg)
    pub node: u32,
}

impl QueryNodeSample {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsLink/QueryNodeSample` packet's fields.
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

    /// Evaluates the alarms of this `/TcsLink/QueryNodeSample` packet's fields
    /// like `alarms()`, counting the earlier samples for change ranges,
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

    /// Evaluates the MathAlgorithms that a `/TcsLink/QueryNodeSample` packet
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

impl Packet for QueryNodeSample {
    const NAME: &'static str = "/TcsLink/QueryNodeSample";
    const MAX_BYTES: usize = 15;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v47 = r.uint_order(3, false, "Version")? as u8;
        let v48 = r.uint_order(1, false, "PacketType")? as u8;
        let v49 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v50 = r.uint_order(11, false, "APID")? as u16;
        let v51 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v52 = r.uint_order(14, false, "SeqCount")? as u16;
        let v53 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v53 as u64, -1, "PacketLength")?;
        let v54 = r.uint_order(8, false, "Opcode")? as u8;
        let v55 = r.uint_order(32, false, "Sequence")? as u32;
        if !((v47 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "Version = 0",
            });
        }
        if !((v48 as i128) == 1) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "PacketType = 1",
            });
        }
        if !((v49 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SecHdrFlag = 0",
            });
        }
        if !((v50 as i128) == 256) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "APID = 256",
            });
        }
        if !((v51 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/TcsCommand",
                criterion: "SeqFlags = 3",
            });
        }
        if !((v54 as i128) == 52) {
            return Err(PacketError::Mismatch {
                container: "/TcsLink/QueryNodeSample",
                criterion: "Opcode = 52",
            });
        }
        let v69 = r.uint_order(32, false, "Node")? as u32;
        r.finish("PacketLength")?;
        Ok((
            QueryNodeSample {
                seq_count: v52,
                sequence: v55,
                node: v69,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(3, false, pk::from_unsigned(0 as u64, 3, "Version")?);
        w.uint_order(1, false, pk::from_unsigned(1 as u64, 1, "PacketType")?);
        w.uint_order(1, false, pk::from_unsigned(0 as u64, 1, "SecHdrFlag")?);
        w.uint_order(11, false, pk::from_unsigned(256 as u64, 11, "APID")?);
        w.uint_order(2, false, pk::from_unsigned(3 as u64, 2, "SeqFlags")?);
        w.uint_order(
            14,
            false,
            pk::from_unsigned(self.seq_count as u64, 14, "SeqCount")?,
        );
        let l53 = w.position();
        w.zeros(16);
        w.uint_order(8, false, pk::from_unsigned(52 as u64, 8, "Opcode")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.node as u64, 32, "Node")?,
        );
        w.patch_length(l53, 16, false, -1, "PacketLength")?;
        Ok(())
    }
}

impl fmt::Display for QueryNodeSample {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("QueryNodeSample{")?;
        f.write_str("seq_count=")?;
        write!(f, "{}", self.seq_count)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", node=")?;
        write!(f, "{}", self.node)?;
        f.write_char('}')
    }
}

/// Every concrete packet derived from `/TcsLink/AnswerLayout`.
#[derive(Debug, Clone, PartialEq)]
pub enum AnswerLayoutKind {
    PingAnswer(PingAnswer),
    RestartArmAnswer(RestartArmAnswer),
    RestartAnswer(RestartAnswer),
    ConnectAnswer(ConnectAnswer),
    StartNodeAnswer(StartNodeAnswer),
    StopNodeAnswer(StopNodeAnswer),
    SetNodeAnswer(SetNodeAnswer),
    QueryNodeAnswer(QueryNodeAnswer),
    QueryNodeSampleAnswer(QueryNodeSampleAnswer),
}

impl Packet for AnswerLayoutKind {
    const NAME: &'static str = "/TcsLink/AnswerLayout";
    const MAX_BYTES: usize = 189;

    /// Decodes the root's entries, then picks the first child (in
    /// document order) whose criteria match, recursively.
    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = r.uint_order(3, false, "Version")? as u8;
        let v2 = r.uint_order(1, false, "PacketType")? as u8;
        let v3 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v4 = r.uint_order(11, false, "APID")? as u16;
        let v5 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v6 = r.uint_order(14, false, "SeqCount")? as u16;
        let v7 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v7 as u64, -1, "PacketLength")?;
        let v8 = r.uint_order(8, false, "Kind")? as u8;
        let v9 = r.uint_order(32, false, "Sequence")? as u32;
        let v10 = r.uint_order(32, false, "Generation")? as u32;
        let v11 = StatusType(r.uint_order(8, false, "Status")? as i64);
        let v12 = r.uint_order(64, false, "Timestamp")? as u64;
        if ((v1 as i128) == 0)
            && ((v2 as i128) == 0)
            && ((v3 as i128) == 0)
            && ((v4 as i128) == 256)
            && ((v5 as i128) == 3)
        {
            if (v8 as i128) == 129 {
                r.finish("PacketLength")?;
                return Ok((
                    Self::PingAnswer(PingAnswer {
                        seq_count: v6,
                        sequence: v9,
                        generation: v10,
                        status: v11,
                        timestamp: v12,
                    }),
                    r.used(),
                ));
            }
            if (v8 as i128) == 130 {
                r.finish("PacketLength")?;
                return Ok((
                    Self::RestartArmAnswer(RestartArmAnswer {
                        seq_count: v6,
                        sequence: v9,
                        generation: v10,
                        status: v11,
                        timestamp: v12,
                    }),
                    r.used(),
                ));
            }
            if (v8 as i128) == 131 {
                r.finish("PacketLength")?;
                return Ok((
                    Self::RestartAnswer(RestartAnswer {
                        seq_count: v6,
                        sequence: v9,
                        generation: v10,
                        status: v11,
                        timestamp: v12,
                    }),
                    r.used(),
                ));
            }
            if (v8 as i128) == 132 {
                let v13 = r.uint_order(8, false, "Major")? as u8;
                let v14 = r.uint_order(8, false, "Minor")? as u8;
                let v15 = r.uint_order(8, false, "Patch")? as u8;
                let v16 = r.fixed_bytes::<16>(16, "Digest")?;
                let v17 = r.uint_order(32, false, "NewestTaken")? as u32;
                r.finish("PacketLength")?;
                return Ok((
                    Self::ConnectAnswer(ConnectAnswer {
                        seq_count: v6,
                        sequence: v9,
                        generation: v10,
                        status: v11,
                        timestamp: v12,
                        major: v13,
                        minor: v14,
                        patch: v15,
                        digest: v16,
                        newest_taken: v17,
                    }),
                    r.used(),
                ));
            }
            if (v8 as i128) == 48 {
                let v18 = r.uint_order(32, false, "Node")? as u32;
                r.finish("PacketLength")?;
                return Ok((
                    Self::StartNodeAnswer(StartNodeAnswer {
                        seq_count: v6,
                        sequence: v9,
                        generation: v10,
                        status: v11,
                        timestamp: v12,
                        node: v18,
                    }),
                    r.used(),
                ));
            }
            if (v8 as i128) == 49 {
                let v19 = r.uint_order(32, false, "Node")? as u32;
                r.finish("PacketLength")?;
                return Ok((
                    Self::StopNodeAnswer(StopNodeAnswer {
                        seq_count: v6,
                        sequence: v9,
                        generation: v10,
                        status: v11,
                        timestamp: v12,
                        node: v19,
                    }),
                    r.used(),
                ));
            }
            if (v8 as i128) == 50 {
                let v20 = r.uint_order(32, false, "Node")? as u32;
                let v21 = pk::read_fixed_string::<160, 161>(
                    &mut r,
                    &StringSpec {
                        fixed_bits: None,
                        max_bits: 1288,
                        terminator: None,
                        size_tag_bits: 8,
                        charset: pk::Charset::Utf8,
                    },
                    "Reason",
                )?;
                r.finish("PacketLength")?;
                return Ok((
                    Self::SetNodeAnswer(SetNodeAnswer {
                        seq_count: v6,
                        sequence: v9,
                        generation: v10,
                        status: v11,
                        timestamp: v12,
                        node: v20,
                        reason: v21,
                    }),
                    r.used(),
                ));
            }
            if (v8 as i128) == 51 {
                let v22 = r.uint_order(32, false, "Node")? as u32;
                let v23 = r.uint_order(8, false, "Enabled")? != 0;
                let v24 = r.uint_order(64, false, "TakenBytes")? as u64;
                let v25 = r.uint_order(64, false, "Reads")? as u64;
                let v26 = r.uint_order(64, false, "ReadsFailed")? as u64;
                let v27 = r.uint_order(64, false, "GivenBytes")? as u64;
                let v28 = r.uint_order(64, false, "Writes")? as u64;
                let v29 = r.uint_order(64, false, "WritesFailed")? as u64;
                let v30 = r.uint_order(64, false, "Dropped")? as u64;
                let v31 = r.uint_order(64, false, "Asks")? as u64;
                let v32 = r.uint_order(64, false, "AsksFailed")? as u64;
                let v33 = r.uint_order(64, false, "Strangers")? as u64;
                let v34 = r.uint_order(8, false, "MovedKnown")? != 0;
                let v35 = r.uint_order(64, false, "LastMoved")? as u64;
                r.finish("PacketLength")?;
                return Ok((
                    Self::QueryNodeAnswer(QueryNodeAnswer {
                        seq_count: v6,
                        sequence: v9,
                        generation: v10,
                        status: v11,
                        timestamp: v12,
                        node: v22,
                        enabled: v23,
                        taken_bytes: v24,
                        reads: v25,
                        reads_failed: v26,
                        given_bytes: v27,
                        writes: v28,
                        writes_failed: v29,
                        dropped: v30,
                        asks: v31,
                        asks_failed: v32,
                        strangers: v33,
                        moved_known: v34,
                        last_moved: v35,
                    }),
                    r.used(),
                ));
            }
            if (v8 as i128) == 52 {
                let v36 = r.uint_order(32, false, "Node")? as u32;
                let v37 = r.uint_order(8, false, "TakenKnown")? != 0;
                let v38 = r.uint_order(64, false, "TakenTime")? as u64;
                let v39 = r.uint_order(8, false, "TakenLength")? as u8;
                let v40 = r.uint_order(32, false, "TakenTotal")? as u32;
                let v41 = r.fixed_bytes::<8>(8, "TakenBytesShown")?;
                let v42 = r.uint_order(8, false, "GivenKnown")? != 0;
                let v43 = r.uint_order(64, false, "GivenTime")? as u64;
                let v44 = r.uint_order(8, false, "GivenLength")? as u8;
                let v45 = r.uint_order(32, false, "GivenTotal")? as u32;
                let v46 = r.fixed_bytes::<8>(8, "GivenBytesShown")?;
                r.finish("PacketLength")?;
                return Ok((
                    Self::QueryNodeSampleAnswer(QueryNodeSampleAnswer {
                        seq_count: v6,
                        sequence: v9,
                        generation: v10,
                        status: v11,
                        timestamp: v12,
                        node: v36,
                        taken_known: v37,
                        taken_time: v38,
                        taken_length: v39,
                        taken_total: v40,
                        taken_bytes_shown: v41,
                        given_known: v42,
                        given_time: v43,
                        given_length: v44,
                        given_total: v45,
                        given_bytes_shown: v46,
                    }),
                    r.used(),
                ));
            }
            return Err(PacketError::NoMatch {
                container: "/TcsLink/Answer",
            });
        }
        return Err(PacketError::NoMatch {
            container: "/TcsLink/AnswerLayout",
        });
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        match self {
            Self::PingAnswer(p) => p.encode_into(w),
            Self::RestartArmAnswer(p) => p.encode_into(w),
            Self::RestartAnswer(p) => p.encode_into(w),
            Self::ConnectAnswer(p) => p.encode_into(w),
            Self::StartNodeAnswer(p) => p.encode_into(w),
            Self::StopNodeAnswer(p) => p.encode_into(w),
            Self::SetNodeAnswer(p) => p.encode_into(w),
            Self::QueryNodeAnswer(p) => p.encode_into(w),
            Self::QueryNodeSampleAnswer(p) => p.encode_into(w),
        }
    }
}

impl fmt::Display for AnswerLayoutKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PingAnswer(p) => p.fmt(f),
            Self::RestartArmAnswer(p) => p.fmt(f),
            Self::RestartAnswer(p) => p.fmt(f),
            Self::ConnectAnswer(p) => p.fmt(f),
            Self::StartNodeAnswer(p) => p.fmt(f),
            Self::StopNodeAnswer(p) => p.fmt(f),
            Self::SetNodeAnswer(p) => p.fmt(f),
            Self::QueryNodeAnswer(p) => p.fmt(f),
            Self::QueryNodeSampleAnswer(p) => p.fmt(f),
        }
    }
}

impl AnswerLayoutKind {
    /// The most fields with an alarm that a member has.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of the fields of this packet derived from
    /// `/TcsLink/AnswerLayout`, as its own `alarms()` does.
    ///
    /// # Returns
    ///
    /// The alarm level of each field with an alarm, `None` when the field is
    /// absent.
    pub fn alarms(&self) -> pk::Alarms<0> {
        let mut out = pk::Alarms::new();
        match self {
            Self::PingAnswer(p) => {
                for a in p.alarms().as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::RestartArmAnswer(p) => {
                for a in p.alarms().as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::RestartAnswer(p) => {
                for a in p.alarms().as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::ConnectAnswer(p) => {
                for a in p.alarms().as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::StartNodeAnswer(p) => {
                for a in p.alarms().as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::StopNodeAnswer(p) => {
                for a in p.alarms().as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::SetNodeAnswer(p) => {
                for a in p.alarms().as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::QueryNodeAnswer(p) => {
                for a in p.alarms().as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::QueryNodeSampleAnswer(p) => {
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
    /// `/TcsLink/AnswerLayout` triggers, as its own `algorithms()` does.
    ///
    /// # Returns
    ///
    /// The output of each, `None` when it can't be computed.
    pub fn algorithms(&self) -> pk::Outputs<0> {
        let mut out = pk::Outputs::new();
        match self {
            Self::PingAnswer(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
            Self::RestartArmAnswer(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
            Self::RestartAnswer(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
            Self::ConnectAnswer(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
            Self::StartNodeAnswer(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
            Self::StopNodeAnswer(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
            Self::SetNodeAnswer(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
            Self::QueryNodeAnswer(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
            Self::QueryNodeSampleAnswer(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
        }
        out
    }

    /// Evaluates the alarms of the fields of this packet derived from
    /// `/TcsLink/AnswerLayout`, as its own `monitor()` does.
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
            Self::PingAnswer(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::RestartArmAnswer(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::RestartAnswer(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::ConnectAnswer(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::StartNodeAnswer(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::StopNodeAnswer(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::SetNodeAnswer(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::QueryNodeAnswer(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::QueryNodeSampleAnswer(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
        }
        out
    }
}

impl From<PingAnswer> for AnswerLayoutKind {
    fn from(p: PingAnswer) -> Self {
        Self::PingAnswer(p)
    }
}

impl From<RestartArmAnswer> for AnswerLayoutKind {
    fn from(p: RestartArmAnswer) -> Self {
        Self::RestartArmAnswer(p)
    }
}

impl From<RestartAnswer> for AnswerLayoutKind {
    fn from(p: RestartAnswer) -> Self {
        Self::RestartAnswer(p)
    }
}

impl From<ConnectAnswer> for AnswerLayoutKind {
    fn from(p: ConnectAnswer) -> Self {
        Self::ConnectAnswer(p)
    }
}

impl From<StartNodeAnswer> for AnswerLayoutKind {
    fn from(p: StartNodeAnswer) -> Self {
        Self::StartNodeAnswer(p)
    }
}

impl From<StopNodeAnswer> for AnswerLayoutKind {
    fn from(p: StopNodeAnswer) -> Self {
        Self::StopNodeAnswer(p)
    }
}

impl From<SetNodeAnswer> for AnswerLayoutKind {
    fn from(p: SetNodeAnswer) -> Self {
        Self::SetNodeAnswer(p)
    }
}

impl From<QueryNodeAnswer> for AnswerLayoutKind {
    fn from(p: QueryNodeAnswer) -> Self {
        Self::QueryNodeAnswer(p)
    }
}

impl From<QueryNodeSampleAnswer> for AnswerLayoutKind {
    fn from(p: QueryNodeSampleAnswer) -> Self {
        Self::QueryNodeSampleAnswer(p)
    }
}

/// Every concrete packet derived from `/TcsLink/Command`.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandKind {
    Ping(Ping),
    RestartArm(RestartArm),
    Restart(Restart),
    Connect(Connect),
    StartNode(StartNode),
    StopNode(StopNode),
    SetNode(SetNode),
    QueryNode(QueryNode),
    QueryNodeSample(QueryNodeSample),
}

impl Packet for CommandKind {
    const NAME: &'static str = "/TcsLink/Command";
    const MAX_BYTES: usize = 338;

    /// Decodes the root's entries, then picks the first child (in
    /// document order) whose criteria match, recursively.
    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v47 = r.uint_order(3, false, "Version")? as u8;
        let v48 = r.uint_order(1, false, "PacketType")? as u8;
        let v49 = r.uint_order(1, false, "SecHdrFlag")? as u8;
        let v50 = r.uint_order(11, false, "APID")? as u16;
        let v51 = r.uint_order(2, false, "SeqFlags")? as u8;
        let v52 = r.uint_order(14, false, "SeqCount")? as u16;
        let v53 = r.uint_order(16, false, "PacketLength")? as u16;
        r.limit(v53 as u64, -1, "PacketLength")?;
        let v54 = r.uint_order(8, false, "Opcode")? as u8;
        let v55 = r.uint_order(32, false, "Sequence")? as u32;
        if ((v47 as i128) == 0)
            && ((v48 as i128) == 1)
            && ((v49 as i128) == 0)
            && ((v50 as i128) == 256)
            && ((v51 as i128) == 3)
        {
            if (v54 as i128) == 1 {
                r.finish("PacketLength")?;
                return Ok((
                    Self::Ping(Ping {
                        seq_count: v52,
                        sequence: v55,
                    }),
                    r.used(),
                ));
            }
            if (v54 as i128) == 2 {
                let v56 = r.uint_order(64, false, "ArmKey")? as u64;
                r.finish("PacketLength")?;
                return Ok((
                    Self::RestartArm(RestartArm {
                        seq_count: v52,
                        sequence: v55,
                        arm_key: v56,
                    }),
                    r.used(),
                ));
            }
            if (v54 as i128) == 3 {
                let v57 = r.uint_order(64, false, "ArmKey")? as u64;
                r.finish("PacketLength")?;
                return Ok((
                    Self::Restart(Restart {
                        seq_count: v52,
                        sequence: v55,
                        arm_key: v57,
                    }),
                    r.used(),
                ));
            }
            if (v54 as i128) == 4 {
                let v58 = r.uint_order(8, false, "Major")? as u8;
                let v59 = r.uint_order(8, false, "Minor")? as u8;
                let v60 = r.uint_order(8, false, "Patch")? as u8;
                let v61 = r.fixed_bytes::<16>(16, "Digest")?;
                r.finish("PacketLength")?;
                return Ok((
                    Self::Connect(Connect {
                        seq_count: v52,
                        sequence: v55,
                        major: v58,
                        minor: v59,
                        patch: v60,
                        digest: v61,
                    }),
                    r.used(),
                ));
            }
            if (v54 as i128) == 48 {
                let v62 = r.uint_order(32, false, "Node")? as u32;
                r.finish("PacketLength")?;
                return Ok((
                    Self::StartNode(StartNode {
                        seq_count: v52,
                        sequence: v55,
                        node: v62,
                    }),
                    r.used(),
                ));
            }
            if (v54 as i128) == 49 {
                let v63 = r.uint_order(32, false, "Node")? as u32;
                r.finish("PacketLength")?;
                return Ok((
                    Self::StopNode(StopNode {
                        seq_count: v52,
                        sequence: v55,
                        node: v63,
                    }),
                    r.used(),
                ));
            }
            if (v54 as i128) == 50 {
                let v64 = r.uint_order(32, false, "Node")? as u32;
                let v65 = pk::read_fixed_string::<64, 65>(
                    &mut r,
                    &StringSpec {
                        fixed_bits: None,
                        max_bits: 520,
                        terminator: None,
                        size_tag_bits: 8,
                        charset: pk::Charset::Utf8,
                    },
                    "Attribute",
                )?;
                let v66 = r.uint_order(16, false, "ValueLength")? as u16;
                let v67 = r.fixed_bytes::<256>(
                    pk::dynamic_size(v66 as f64, 8.0, 0.0, 8, 256, "Value")?,
                    "Value",
                )?;
                r.finish("PacketLength")?;
                return Ok((
                    Self::SetNode(SetNode {
                        seq_count: v52,
                        sequence: v55,
                        node: v64,
                        attribute: v65,
                        value_: v67,
                    }),
                    r.used(),
                ));
            }
            if (v54 as i128) == 51 {
                let v68 = r.uint_order(32, false, "Node")? as u32;
                r.finish("PacketLength")?;
                return Ok((
                    Self::QueryNode(QueryNode {
                        seq_count: v52,
                        sequence: v55,
                        node: v68,
                    }),
                    r.used(),
                ));
            }
            if (v54 as i128) == 52 {
                let v69 = r.uint_order(32, false, "Node")? as u32;
                r.finish("PacketLength")?;
                return Ok((
                    Self::QueryNodeSample(QueryNodeSample {
                        seq_count: v52,
                        sequence: v55,
                        node: v69,
                    }),
                    r.used(),
                ));
            }
            return Err(PacketError::NoMatch {
                container: "/TcsLink/TcsCommand",
            });
        }
        return Err(PacketError::NoMatch {
            container: "/TcsLink/Command",
        });
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        match self {
            Self::Ping(p) => p.encode_into(w),
            Self::RestartArm(p) => p.encode_into(w),
            Self::Restart(p) => p.encode_into(w),
            Self::Connect(p) => p.encode_into(w),
            Self::StartNode(p) => p.encode_into(w),
            Self::StopNode(p) => p.encode_into(w),
            Self::SetNode(p) => p.encode_into(w),
            Self::QueryNode(p) => p.encode_into(w),
            Self::QueryNodeSample(p) => p.encode_into(w),
        }
    }
}

impl fmt::Display for CommandKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ping(p) => p.fmt(f),
            Self::RestartArm(p) => p.fmt(f),
            Self::Restart(p) => p.fmt(f),
            Self::Connect(p) => p.fmt(f),
            Self::StartNode(p) => p.fmt(f),
            Self::StopNode(p) => p.fmt(f),
            Self::SetNode(p) => p.fmt(f),
            Self::QueryNode(p) => p.fmt(f),
            Self::QueryNodeSample(p) => p.fmt(f),
        }
    }
}

impl CommandKind {
    /// The most fields with an alarm that a member has.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of the fields of this packet derived from
    /// `/TcsLink/Command`, as its own `alarms()` does.
    ///
    /// # Returns
    ///
    /// The alarm level of each field with an alarm, `None` when the field is
    /// absent.
    pub fn alarms(&self) -> pk::Alarms<0> {
        let mut out = pk::Alarms::new();
        match self {
            Self::Ping(p) => {
                for a in p.alarms().as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::RestartArm(p) => {
                for a in p.alarms().as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::Restart(p) => {
                for a in p.alarms().as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::Connect(p) => {
                for a in p.alarms().as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::StartNode(p) => {
                for a in p.alarms().as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::StopNode(p) => {
                for a in p.alarms().as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::SetNode(p) => {
                for a in p.alarms().as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::QueryNode(p) => {
                for a in p.alarms().as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::QueryNodeSample(p) => {
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
    /// `/TcsLink/Command` triggers, as its own `algorithms()` does.
    ///
    /// # Returns
    ///
    /// The output of each, `None` when it can't be computed.
    pub fn algorithms(&self) -> pk::Outputs<0> {
        let mut out = pk::Outputs::new();
        match self {
            Self::Ping(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
            Self::RestartArm(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
            Self::Restart(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
            Self::Connect(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
            Self::StartNode(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
            Self::StopNode(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
            Self::SetNode(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
            Self::QueryNode(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
            Self::QueryNodeSample(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
        }
        out
    }

    /// Evaluates the alarms of the fields of this packet derived from
    /// `/TcsLink/Command`, as its own `monitor()` does.
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
            Self::Ping(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::RestartArm(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::Restart(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::Connect(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::StartNode(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::StopNode(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::SetNode(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::QueryNode(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
            Self::QueryNodeSample(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
        }
        out
    }
}

impl From<Ping> for CommandKind {
    fn from(p: Ping) -> Self {
        Self::Ping(p)
    }
}

impl From<RestartArm> for CommandKind {
    fn from(p: RestartArm) -> Self {
        Self::RestartArm(p)
    }
}

impl From<Restart> for CommandKind {
    fn from(p: Restart) -> Self {
        Self::Restart(p)
    }
}

impl From<Connect> for CommandKind {
    fn from(p: Connect) -> Self {
        Self::Connect(p)
    }
}

impl From<StartNode> for CommandKind {
    fn from(p: StartNode) -> Self {
        Self::StartNode(p)
    }
}

impl From<StopNode> for CommandKind {
    fn from(p: StopNode) -> Self {
        Self::StopNode(p)
    }
}

impl From<SetNode> for CommandKind {
    fn from(p: SetNode) -> Self {
        Self::SetNode(p)
    }
}

impl From<QueryNode> for CommandKind {
    fn from(p: QueryNode) -> Self {
        Self::QueryNode(p)
    }
}

impl From<QueryNodeSample> for CommandKind {
    fn from(p: QueryNodeSample) -> Self {
        Self::QueryNodeSample(p)
    }
}
