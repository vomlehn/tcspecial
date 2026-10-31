// @generated automatically by tcs-xtce from tcslibgs/xtce/fixed.xml
// (SpaceSystem 'TcsFixed').
// Any changes made to this file will be lost when it is regenerated;
// change the XTCE document instead and rerun `python3 -m tcs_xtce`.
// tcs-xtce: https://github.com/vomlehn/tcs-xtce
// Rust crate: https://crates.io/crates/tcs-xtce
//! Packet codecs for the `TcsFixed` mission.
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

/// Enumeration `/TcsFixed/StatusType`; holds the raw value so unlisted values
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
    pub const STALE: Self = Self(9);
    pub const UNMATCHED: Self = Self(10);

    /// Names this value of enumeration `/TcsFixed/StatusType`.
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

/// Sequence container `/TcsFixed/RestartArmAnswer`.
///
/// Fixed by its inheritance: `Opcode` = 2.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RestartArmAnswer {
    /// `/TcsFixed/Sequence` (/TcsFixed/U32Type)
    pub sequence: u32,
    /// `/TcsFixed/Status` (/TcsFixed/StatusType)
    pub status: StatusType,
    /// `/TcsFixed/Timestamp` (/TcsFixed/U64Type)
    pub timestamp: u64,
}

impl RestartArmAnswer {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsFixed/RestartArmAnswer` packet's
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

    /// Evaluates the alarms of this `/TcsFixed/RestartArmAnswer` packet's
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

    /// Evaluates the MathAlgorithms that a `/TcsFixed/RestartArmAnswer` packet
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
    const NAME: &'static str = "/TcsFixed/RestartArmAnswer";
    const MAX_BYTES: usize = 14;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = r.uint_order(8, false, "Opcode")? as u8;
        let v2 = r.uint_order(32, false, "Sequence")? as u32;
        let v3 = StatusType(r.uint_order(8, false, "Status")? as i64);
        let v4 = r.uint_order(64, false, "Timestamp")? as u64;
        if !((v1 as i128) == 2) {
            return Err(PacketError::Mismatch {
                container: "/TcsFixed/RestartArmAnswer",
                criterion: "/TcsFixed/Opcode == 2",
            });
        }
        Ok((
            RestartArmAnswer {
                sequence: v2,
                status: v3,
                timestamp: v4,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(8, false, pk::from_unsigned(2 as u64, 8, "Opcode")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
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
        Ok(())
    }
}

impl fmt::Display for RestartArmAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RestartArmAnswer{")?;
        f.write_str("sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", status=")?;
        write!(f, "{}", self.status)?;
        f.write_str(", timestamp=")?;
        write!(f, "{}", self.timestamp)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsFixed/RestartAnswer`.
///
/// Fixed by its inheritance: `Opcode` = 3.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RestartAnswer {
    /// `/TcsFixed/Sequence` (/TcsFixed/U32Type)
    pub sequence: u32,
    /// `/TcsFixed/Status` (/TcsFixed/StatusType)
    pub status: StatusType,
    /// `/TcsFixed/Timestamp` (/TcsFixed/U64Type)
    pub timestamp: u64,
}

impl RestartAnswer {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsFixed/RestartAnswer` packet's fields.
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

    /// Evaluates the alarms of this `/TcsFixed/RestartAnswer` packet's fields
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

    /// Evaluates the MathAlgorithms that a `/TcsFixed/RestartAnswer` packet
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
    const NAME: &'static str = "/TcsFixed/RestartAnswer";
    const MAX_BYTES: usize = 14;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = r.uint_order(8, false, "Opcode")? as u8;
        let v2 = r.uint_order(32, false, "Sequence")? as u32;
        let v3 = StatusType(r.uint_order(8, false, "Status")? as i64);
        let v4 = r.uint_order(64, false, "Timestamp")? as u64;
        if !((v1 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsFixed/RestartAnswer",
                criterion: "/TcsFixed/Opcode == 3",
            });
        }
        Ok((
            RestartAnswer {
                sequence: v2,
                status: v3,
                timestamp: v4,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(8, false, pk::from_unsigned(3 as u64, 8, "Opcode")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
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
        Ok(())
    }
}

impl fmt::Display for RestartAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RestartAnswer{")?;
        f.write_str("sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", status=")?;
        write!(f, "{}", self.status)?;
        f.write_str(", timestamp=")?;
        write!(f, "{}", self.timestamp)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsFixed/ConnectAnswer`.
///
/// Fixed by its inheritance: `Opcode` = 4.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConnectAnswer {
    /// `/TcsFixed/Sequence` (/TcsFixed/U32Type)
    pub sequence: u32,
    /// `/TcsFixed/Status` (/TcsFixed/StatusType)
    pub status: StatusType,
    /// `/TcsFixed/Timestamp` (/TcsFixed/U64Type)
    pub timestamp: u64,
    /// `/TcsFixed/Running` (/TcsFixed/ChecksumType)
    pub running: pk::FixedBytes<16>,
    /// `/TcsFixed/Major` (/TcsFixed/U8Type)
    pub major: u8,
    /// `/TcsFixed/Minor` (/TcsFixed/U8Type)
    pub minor: u8,
    /// `/TcsFixed/Patch` (/TcsFixed/U8Type)
    pub patch: u8,
    /// `/TcsFixed/NewestTaken` (/TcsFixed/U32Type)
    pub newest_taken: u32,
}

impl ConnectAnswer {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsFixed/ConnectAnswer` packet's fields.
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

    /// Evaluates the alarms of this `/TcsFixed/ConnectAnswer` packet's fields
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

    /// Evaluates the MathAlgorithms that a `/TcsFixed/ConnectAnswer` packet
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
    const NAME: &'static str = "/TcsFixed/ConnectAnswer";
    const MAX_BYTES: usize = 37;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = r.uint_order(8, false, "Opcode")? as u8;
        let v2 = r.uint_order(32, false, "Sequence")? as u32;
        let v3 = StatusType(r.uint_order(8, false, "Status")? as i64);
        let v4 = r.uint_order(64, false, "Timestamp")? as u64;
        if !((v1 as i128) == 4) {
            return Err(PacketError::Mismatch {
                container: "/TcsFixed/ConnectAnswer",
                criterion: "/TcsFixed/Opcode == 4",
            });
        }
        let v5 = r.fixed_bytes::<16>(16, "Running")?;
        let v6 = r.uint_order(8, false, "Major")? as u8;
        let v7 = r.uint_order(8, false, "Minor")? as u8;
        let v8 = r.uint_order(8, false, "Patch")? as u8;
        let v9 = r.uint_order(32, false, "NewestTaken")? as u32;
        Ok((
            ConnectAnswer {
                sequence: v2,
                status: v3,
                timestamp: v4,
                running: v5,
                major: v6,
                minor: v7,
                patch: v8,
                newest_taken: v9,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(8, false, pk::from_unsigned(4 as u64, 8, "Opcode")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
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
        pk::write_binary(w, &self.running, 16, "Running")?;
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
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.newest_taken as u64, 32, "NewestTaken")?,
        );
        Ok(())
    }
}

impl fmt::Display for ConnectAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ConnectAnswer{")?;
        f.write_str("sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", status=")?;
        write!(f, "{}", self.status)?;
        f.write_str(", timestamp=")?;
        write!(f, "{}", self.timestamp)?;
        f.write_str(", running=")?;
        pk::describe_bytes(&self.running, f)?;
        f.write_str(", major=")?;
        write!(f, "{}", self.major)?;
        f.write_str(", minor=")?;
        write!(f, "{}", self.minor)?;
        f.write_str(", patch=")?;
        write!(f, "{}", self.patch)?;
        f.write_str(", newest_taken=")?;
        write!(f, "{}", self.newest_taken)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsFixed/Beacon`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Beacon {
    /// `/TcsFixed/BeaconName` (/TcsFixed/BeaconNameType)
    pub beacon_name: pk::FixedString<9>,
    /// `/TcsFixed/BeaconVersion` (/TcsFixed/BeaconVersionType)
    pub beacon_version: pk::FixedString<11>,
    /// `/TcsFixed/BeaconDigest` (/TcsFixed/BeaconDigestType)
    pub beacon_digest: pk::FixedString<32>,
    /// `/TcsFixed/BeaconNumber` (/TcsFixed/BeaconNumberType)
    pub beacon_number: pk::FixedString<10>,
    /// `/TcsFixed/BeaconFlag` (/TcsFixed/BeaconFlagType)
    pub beacon_flag: pk::FixedString<7>,
    /// `/TcsFixed/BeaconInterface` (/TcsFixed/BeaconInterfaceType)
    pub beacon_interface: pk::FixedString<32>,
}

impl Beacon {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsFixed/Beacon` packet's fields. Alarms
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

    /// Evaluates the alarms of this `/TcsFixed/Beacon` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsFixed/Beacon` packet triggers,
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

impl Packet for Beacon {
    const NAME: &'static str = "/TcsFixed/Beacon";
    const MAX_BYTES: usize = 106;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v10 = pk::read_fixed_string::<9, 10>(
            &mut r,
            &StringSpec {
                fixed_bits: None,
                max_bits: 80,
                terminator: Some(&[0x3a]),
                size_tag_bits: 0,
                charset: pk::Charset::Utf8,
            },
            "BeaconName",
        )?;
        let v11 = pk::read_fixed_string::<11, 12>(
            &mut r,
            &StringSpec {
                fixed_bits: None,
                max_bits: 96,
                terminator: Some(&[0x3a]),
                size_tag_bits: 0,
                charset: pk::Charset::Utf8,
            },
            "BeaconVersion",
        )?;
        let v12 = pk::read_fixed_string::<32, 33>(
            &mut r,
            &StringSpec {
                fixed_bits: None,
                max_bits: 264,
                terminator: Some(&[0x3a]),
                size_tag_bits: 0,
                charset: pk::Charset::Utf8,
            },
            "BeaconDigest",
        )?;
        let v13 = pk::read_fixed_string::<10, 11>(
            &mut r,
            &StringSpec {
                fixed_bits: None,
                max_bits: 88,
                terminator: Some(&[0x3a]),
                size_tag_bits: 0,
                charset: pk::Charset::Utf8,
            },
            "BeaconNumber",
        )?;
        let v14 = pk::read_fixed_string::<7, 8>(
            &mut r,
            &StringSpec {
                fixed_bits: None,
                max_bits: 64,
                terminator: Some(&[0x3a]),
                size_tag_bits: 0,
                charset: pk::Charset::Utf8,
            },
            "BeaconFlag",
        )?;
        let v15 = pk::read_fixed_string::<32, 32>(
            &mut r,
            &StringSpec {
                fixed_bits: Some(256),
                max_bits: 256,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Utf8,
            },
            "BeaconInterface",
        )?;
        Ok((
            Beacon {
                beacon_name: v10,
                beacon_version: v11,
                beacon_digest: v12,
                beacon_number: v13,
                beacon_flag: v14,
                beacon_interface: v15,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: None,
                max_bits: 80,
                terminator: Some(&[0x3a]),
                size_tag_bits: 0,
                charset: pk::Charset::Utf8,
            },
            &self.beacon_name,
            "BeaconName",
        )?;
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: None,
                max_bits: 96,
                terminator: Some(&[0x3a]),
                size_tag_bits: 0,
                charset: pk::Charset::Utf8,
            },
            &self.beacon_version,
            "BeaconVersion",
        )?;
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: None,
                max_bits: 264,
                terminator: Some(&[0x3a]),
                size_tag_bits: 0,
                charset: pk::Charset::Utf8,
            },
            &self.beacon_digest,
            "BeaconDigest",
        )?;
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: None,
                max_bits: 88,
                terminator: Some(&[0x3a]),
                size_tag_bits: 0,
                charset: pk::Charset::Utf8,
            },
            &self.beacon_number,
            "BeaconNumber",
        )?;
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: None,
                max_bits: 64,
                terminator: Some(&[0x3a]),
                size_tag_bits: 0,
                charset: pk::Charset::Utf8,
            },
            &self.beacon_flag,
            "BeaconFlag",
        )?;
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: Some(256),
                max_bits: 256,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Utf8,
            },
            &self.beacon_interface,
            "BeaconInterface",
        )?;
        Ok(())
    }
}

impl fmt::Display for Beacon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Beacon{")?;
        f.write_str("beacon_name=")?;
        pk::describe_str(&self.beacon_name, f)?;
        f.write_str(", beacon_version=")?;
        pk::describe_str(&self.beacon_version, f)?;
        f.write_str(", beacon_digest=")?;
        pk::describe_str(&self.beacon_digest, f)?;
        f.write_str(", beacon_number=")?;
        pk::describe_str(&self.beacon_number, f)?;
        f.write_str(", beacon_flag=")?;
        pk::describe_str(&self.beacon_flag, f)?;
        f.write_str(", beacon_interface=")?;
        pk::describe_str(&self.beacon_interface, f)?;
        f.write_char('}')
    }
}

/// Meta-command `/TcsFixed/RestartArm`.
///
/// Fixed by its inheritance: `Opcode` = 2.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RestartArm {
    /// `/TcsFixed/FixedCommand#Sequence` (/TcsFixed/U32Arg)
    pub sequence: u32,
    /// `/TcsFixed/RestartArm#ArmKey` (/TcsFixed/U64Arg)
    pub arm_key: u64,
    /// `/TcsFixed/RestartArm#Target` (/TcsFixed/ChecksumArg)
    pub target: pk::FixedBytes<16>,
}

impl RestartArm {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsFixed/RestartArm` packet's fields.
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

    /// Evaluates the alarms of this `/TcsFixed/RestartArm` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsFixed/RestartArm` packet
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
    const NAME: &'static str = "/TcsFixed/RestartArm";
    const MAX_BYTES: usize = 29;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v16 = r.uint_order(8, false, "Opcode")? as u8;
        let v17 = r.uint_order(32, false, "Sequence")? as u32;
        if !((v16 as i128) == 2) {
            return Err(PacketError::Mismatch {
                container: "/TcsFixed/RestartArm",
                criterion: "Opcode = 2",
            });
        }
        let v18 = r.uint_order(64, false, "ArmKey")? as u64;
        let v19 = r.fixed_bytes::<16>(16, "Target")?;
        Ok((
            RestartArm {
                sequence: v17,
                arm_key: v18,
                target: v19,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
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
        pk::write_binary(w, &self.target, 16, "Target")?;
        Ok(())
    }
}

impl fmt::Display for RestartArm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RestartArm{")?;
        f.write_str("sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", arm_key=")?;
        write!(f, "{}", self.arm_key)?;
        f.write_str(", target=")?;
        pk::describe_bytes(&self.target, f)?;
        f.write_char('}')
    }
}

/// Meta-command `/TcsFixed/Restart`.
///
/// Fixed by its inheritance: `Opcode` = 3.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Restart {
    /// `/TcsFixed/FixedCommand#Sequence` (/TcsFixed/U32Arg)
    pub sequence: u32,
    /// `/TcsFixed/Restart#ArmKey` (/TcsFixed/U64Arg)
    pub arm_key: u64,
    /// `/TcsFixed/Restart#Target` (/TcsFixed/ChecksumArg)
    pub target: pk::FixedBytes<16>,
}

impl Restart {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsFixed/Restart` packet's fields. Alarms
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

    /// Evaluates the alarms of this `/TcsFixed/Restart` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsFixed/Restart` packet triggers,
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
    const NAME: &'static str = "/TcsFixed/Restart";
    const MAX_BYTES: usize = 29;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v16 = r.uint_order(8, false, "Opcode")? as u8;
        let v17 = r.uint_order(32, false, "Sequence")? as u32;
        if !((v16 as i128) == 3) {
            return Err(PacketError::Mismatch {
                container: "/TcsFixed/Restart",
                criterion: "Opcode = 3",
            });
        }
        let v20 = r.uint_order(64, false, "ArmKey")? as u64;
        let v21 = r.fixed_bytes::<16>(16, "Target")?;
        Ok((
            Restart {
                sequence: v17,
                arm_key: v20,
                target: v21,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
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
        pk::write_binary(w, &self.target, 16, "Target")?;
        Ok(())
    }
}

impl fmt::Display for Restart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Restart{")?;
        f.write_str("sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", arm_key=")?;
        write!(f, "{}", self.arm_key)?;
        f.write_str(", target=")?;
        pk::describe_bytes(&self.target, f)?;
        f.write_char('}')
    }
}

/// Meta-command `/TcsFixed/Connect`.
///
/// Fixed by its inheritance: `Opcode` = 4.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Connect {
    /// `/TcsFixed/FixedCommand#Sequence` (/TcsFixed/U32Arg)
    pub sequence: u32,
    /// `/TcsFixed/Connect#Checksum` (/TcsFixed/ChecksumArg)
    pub checksum: pk::FixedBytes<16>,
}

impl Connect {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsFixed/Connect` packet's fields. Alarms
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

    /// Evaluates the alarms of this `/TcsFixed/Connect` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsFixed/Connect` packet triggers,
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
    const NAME: &'static str = "/TcsFixed/Connect";
    const MAX_BYTES: usize = 21;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v16 = r.uint_order(8, false, "Opcode")? as u8;
        let v17 = r.uint_order(32, false, "Sequence")? as u32;
        if !((v16 as i128) == 4) {
            return Err(PacketError::Mismatch {
                container: "/TcsFixed/Connect",
                criterion: "Opcode = 4",
            });
        }
        let v22 = r.fixed_bytes::<16>(16, "Checksum")?;
        Ok((
            Connect {
                sequence: v17,
                checksum: v22,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(8, false, pk::from_unsigned(4 as u64, 8, "Opcode")?);
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.sequence as u64, 32, "Sequence")?,
        );
        pk::write_binary(w, &self.checksum, 16, "Checksum")?;
        Ok(())
    }
}

impl fmt::Display for Connect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Connect{")?;
        f.write_str("sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", checksum=")?;
        pk::describe_bytes(&self.checksum, f)?;
        f.write_char('}')
    }
}

/// Every concrete packet derived from `/TcsFixed/FixedAnswer`.
#[derive(Debug, Clone, PartialEq)]
pub enum FixedAnswerKind {
    RestartArmAnswer(RestartArmAnswer),
    RestartAnswer(RestartAnswer),
    ConnectAnswer(ConnectAnswer),
}

impl Packet for FixedAnswerKind {
    const NAME: &'static str = "/TcsFixed/FixedAnswer";
    const MAX_BYTES: usize = 37;

    /// Decodes the root's entries, then picks the first child (in
    /// document order) whose criteria match, recursively.
    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = r.uint_order(8, false, "Opcode")? as u8;
        let v2 = r.uint_order(32, false, "Sequence")? as u32;
        let v3 = StatusType(r.uint_order(8, false, "Status")? as i64);
        let v4 = r.uint_order(64, false, "Timestamp")? as u64;
        if (v1 as i128) == 2 {
            return Ok((
                Self::RestartArmAnswer(RestartArmAnswer {
                    sequence: v2,
                    status: v3,
                    timestamp: v4,
                }),
                r.used(),
            ));
        }
        if (v1 as i128) == 3 {
            return Ok((
                Self::RestartAnswer(RestartAnswer {
                    sequence: v2,
                    status: v3,
                    timestamp: v4,
                }),
                r.used(),
            ));
        }
        if (v1 as i128) == 4 {
            let v5 = r.fixed_bytes::<16>(16, "Running")?;
            let v6 = r.uint_order(8, false, "Major")? as u8;
            let v7 = r.uint_order(8, false, "Minor")? as u8;
            let v8 = r.uint_order(8, false, "Patch")? as u8;
            let v9 = r.uint_order(32, false, "NewestTaken")? as u32;
            return Ok((
                Self::ConnectAnswer(ConnectAnswer {
                    sequence: v2,
                    status: v3,
                    timestamp: v4,
                    running: v5,
                    major: v6,
                    minor: v7,
                    patch: v8,
                    newest_taken: v9,
                }),
                r.used(),
            ));
        }
        return Err(PacketError::NoMatch {
            container: "/TcsFixed/FixedAnswer",
        });
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        match self {
            Self::RestartArmAnswer(p) => p.encode_into(w),
            Self::RestartAnswer(p) => p.encode_into(w),
            Self::ConnectAnswer(p) => p.encode_into(w),
        }
    }
}

impl fmt::Display for FixedAnswerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RestartArmAnswer(p) => p.fmt(f),
            Self::RestartAnswer(p) => p.fmt(f),
            Self::ConnectAnswer(p) => p.fmt(f),
        }
    }
}

impl FixedAnswerKind {
    /// The most fields with an alarm that a member has.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of the fields of this packet derived from
    /// `/TcsFixed/FixedAnswer`, as its own `alarms()` does.
    ///
    /// # Returns
    ///
    /// The alarm level of each field with an alarm, `None` when the field is
    /// absent.
    pub fn alarms(&self) -> pk::Alarms<0> {
        let mut out = pk::Alarms::new();
        match self {
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
        }
        out
    }

    /// The most MathAlgorithms that a member triggers.
    pub const ALGORITHMS: usize = 0;

    /// Evaluates the MathAlgorithms that this packet derived from
    /// `/TcsFixed/FixedAnswer` triggers, as its own `algorithms()` does.
    ///
    /// # Returns
    ///
    /// The output of each, `None` when it can't be computed.
    pub fn algorithms(&self) -> pk::Outputs<0> {
        let mut out = pk::Outputs::new();
        match self {
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
        }
        out
    }

    /// Evaluates the alarms of the fields of this packet derived from
    /// `/TcsFixed/FixedAnswer`, as its own `monitor()` does.
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
        }
        out
    }
}

impl From<RestartArmAnswer> for FixedAnswerKind {
    fn from(p: RestartArmAnswer) -> Self {
        Self::RestartArmAnswer(p)
    }
}

impl From<RestartAnswer> for FixedAnswerKind {
    fn from(p: RestartAnswer) -> Self {
        Self::RestartAnswer(p)
    }
}

impl From<ConnectAnswer> for FixedAnswerKind {
    fn from(p: ConnectAnswer) -> Self {
        Self::ConnectAnswer(p)
    }
}

/// Every concrete packet derived from `/TcsFixed/FixedCommand`.
#[derive(Debug, Clone, PartialEq)]
pub enum FixedCommandKind {
    RestartArm(RestartArm),
    Restart(Restart),
    Connect(Connect),
}

impl Packet for FixedCommandKind {
    const NAME: &'static str = "/TcsFixed/FixedCommand";
    const MAX_BYTES: usize = 29;

    /// Decodes the root's entries, then picks the first child (in
    /// document order) whose criteria match, recursively.
    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v16 = r.uint_order(8, false, "Opcode")? as u8;
        let v17 = r.uint_order(32, false, "Sequence")? as u32;
        if (v16 as i128) == 2 {
            let v18 = r.uint_order(64, false, "ArmKey")? as u64;
            let v19 = r.fixed_bytes::<16>(16, "Target")?;
            return Ok((
                Self::RestartArm(RestartArm {
                    sequence: v17,
                    arm_key: v18,
                    target: v19,
                }),
                r.used(),
            ));
        }
        if (v16 as i128) == 3 {
            let v20 = r.uint_order(64, false, "ArmKey")? as u64;
            let v21 = r.fixed_bytes::<16>(16, "Target")?;
            return Ok((
                Self::Restart(Restart {
                    sequence: v17,
                    arm_key: v20,
                    target: v21,
                }),
                r.used(),
            ));
        }
        if (v16 as i128) == 4 {
            let v22 = r.fixed_bytes::<16>(16, "Checksum")?;
            return Ok((
                Self::Connect(Connect {
                    sequence: v17,
                    checksum: v22,
                }),
                r.used(),
            ));
        }
        return Err(PacketError::NoMatch {
            container: "/TcsFixed/FixedCommand",
        });
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        match self {
            Self::RestartArm(p) => p.encode_into(w),
            Self::Restart(p) => p.encode_into(w),
            Self::Connect(p) => p.encode_into(w),
        }
    }
}

impl fmt::Display for FixedCommandKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RestartArm(p) => p.fmt(f),
            Self::Restart(p) => p.fmt(f),
            Self::Connect(p) => p.fmt(f),
        }
    }
}

impl FixedCommandKind {
    /// The most fields with an alarm that a member has.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of the fields of this packet derived from
    /// `/TcsFixed/FixedCommand`, as its own `alarms()` does.
    ///
    /// # Returns
    ///
    /// The alarm level of each field with an alarm, `None` when the field is
    /// absent.
    pub fn alarms(&self) -> pk::Alarms<0> {
        let mut out = pk::Alarms::new();
        match self {
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
        }
        out
    }

    /// The most MathAlgorithms that a member triggers.
    pub const ALGORITHMS: usize = 0;

    /// Evaluates the MathAlgorithms that this packet derived from
    /// `/TcsFixed/FixedCommand` triggers, as its own `algorithms()` does.
    ///
    /// # Returns
    ///
    /// The output of each, `None` when it can't be computed.
    pub fn algorithms(&self) -> pk::Outputs<0> {
        let mut out = pk::Outputs::new();
        match self {
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
        }
        out
    }

    /// Evaluates the alarms of the fields of this packet derived from
    /// `/TcsFixed/FixedCommand`, as its own `monitor()` does.
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
        }
        out
    }
}

impl From<RestartArm> for FixedCommandKind {
    fn from(p: RestartArm) -> Self {
        Self::RestartArm(p)
    }
}

impl From<Restart> for FixedCommandKind {
    fn from(p: Restart) -> Self {
        Self::Restart(p)
    }
}

impl From<Connect> for FixedCommandKind {
    fn from(p: Connect) -> Self {
        Self::Connect(p)
    }
}
