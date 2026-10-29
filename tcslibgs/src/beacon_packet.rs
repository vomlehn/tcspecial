// @generated automatically by tcs-xtce from tcslibgs/xtce/beacon.xml
// (SpaceSystem 'TcsBeacon').
// Any changes made to this file will be lost when it is regenerated;
// change the XTCE document instead and rerun `python3 -m tcs_xtce`.
// tcs-xtce: https://github.com/vomlehn/tcs-xtce
// Rust crate: https://crates.io/crates/tcs-xtce
//! Packet codecs for the `TcsBeacon` mission.
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

/// The functions the application supplies for the document's custom
/// algorithms: set them before decoding or encoding a packet that needs
/// them, or it fails with `NoAlgorithm`.
pub mod hooks {}

/// The alarm state of each parameter whose alarms depend on earlier
/// samples, which the packets' `monitor()` reads and updates.
#[derive(Debug, Clone, Default)]
pub struct AlarmMonitor {}

/// Sequence container `/TcsBeacon/Beacon`.
///
/// Fixed by its inheritance: `Name` = 'tcspecial ', `Separator` = ' ', `Spare`
/// = 0.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Beacon {
    /// `/TcsBeacon/Major` (/TcsBeacon/U8Type)
    pub major: u8,
    /// `/TcsBeacon/Minor` (/TcsBeacon/U8Type)
    pub minor: u8,
    /// `/TcsBeacon/Patch` (/TcsBeacon/U8Type)
    pub patch: u8,
    /// `/TcsBeacon/Digest` (/TcsBeacon/DigestType)
    pub digest: pk::FixedBytes<16>,
    /// `/TcsBeacon/Sequence` (/TcsBeacon/U32Type)
    pub sequence: u32,
    /// `/TcsBeacon/Generation` (/TcsBeacon/U32Type)
    pub generation: u32,
    /// `/TcsBeacon/CommandedRestart` (/TcsBeacon/FlagType)
    pub commanded_restart: bool,
    /// `/TcsBeacon/Interface` (/TcsBeacon/DigestType)
    pub interface: pk::FixedBytes<16>,
}

impl Beacon {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsBeacon/Beacon` packet's fields. Alarms
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

    /// Evaluates the alarms of this `/TcsBeacon/Beacon` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsBeacon/Beacon` packet triggers,
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
    const NAME: &'static str = "/TcsBeacon/Beacon";
    const MAX_BYTES: usize = 55;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = pk::read_fixed_string::<10, 10>(
            &mut r,
            &StringSpec {
                fixed_bits: Some(80),
                max_bits: 80,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "Name",
        )?;
        let v2 = r.uint_order(8, false, "Major")? as u8;
        let v3 = r.uint_order(8, false, "Minor")? as u8;
        let v4 = r.uint_order(8, false, "Patch")? as u8;
        let v5 = pk::read_fixed_string::<1, 1>(
            &mut r,
            &StringSpec {
                fixed_bits: Some(8),
                max_bits: 8,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "Separator",
        )?;
        let v6 = r.fixed_bytes::<16>(16, "Digest")?;
        let v7 = r.uint_order(32, false, "Sequence")? as u32;
        let v8 = r.uint_order(32, false, "Generation")? as u32;
        let v9 = r.uint_order(1, false, "CommandedRestart")? != 0;
        let v10 = r.uint_order(7, false, "Spare")? as u8;
        let v11 = r.fixed_bytes::<16>(16, "Interface")?;
        if !(v1 == "tcspecial ") {
            return Err(PacketError::Mismatch {
                container: "/TcsBeacon/Beacon",
                criterion: "/TcsBeacon/Name == tcspecial ",
            });
        }
        if !(v5 == " ") {
            return Err(PacketError::Mismatch {
                container: "/TcsBeacon/Beacon",
                criterion: "/TcsBeacon/Separator ==  ",
            });
        }
        if !((v10 as i128) == 0) {
            return Err(PacketError::Mismatch {
                container: "/TcsBeacon/Beacon",
                criterion: "/TcsBeacon/Spare == 0",
            });
        }
        Ok((
            Beacon {
                major: v2,
                minor: v3,
                patch: v4,
                digest: v6,
                sequence: v7,
                generation: v8,
                commanded_restart: v9,
                interface: v11,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: Some(80),
                max_bits: 80,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            &"tcspecial ",
            "Name",
        )?;
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
            "Separator",
        )?;
        pk::write_binary(w, &self.digest, 16, "Digest")?;
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
        w.uint_order(1, false, self.commanded_restart as u64);
        w.uint_order(7, false, pk::from_unsigned(0 as u64, 7, "Spare")?);
        pk::write_binary(w, &self.interface, 16, "Interface")?;
        Ok(())
    }
}

impl fmt::Display for Beacon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Beacon{")?;
        f.write_str("major=")?;
        write!(f, "{}", self.major)?;
        f.write_str(", minor=")?;
        write!(f, "{}", self.minor)?;
        f.write_str(", patch=")?;
        write!(f, "{}", self.patch)?;
        f.write_str(", digest=")?;
        pk::describe_bytes(&self.digest, f)?;
        f.write_str(", sequence=")?;
        write!(f, "{}", self.sequence)?;
        f.write_str(", generation=")?;
        write!(f, "{}", self.generation)?;
        f.write_str(", commanded_restart=")?;
        write!(f, "{}", self.commanded_restart)?;
        f.write_str(", interface=")?;
        pk::describe_bytes(&self.interface, f)?;
        f.write_char('}')
    }
}

/// Every concrete packet derived from `/TcsBeacon/BeaconLayout`.
#[derive(Debug, Clone, PartialEq)]
pub enum BeaconLayoutKind {
    Beacon(Beacon),
}

impl Packet for BeaconLayoutKind {
    const NAME: &'static str = "/TcsBeacon/BeaconLayout";
    const MAX_BYTES: usize = 55;

    /// Decodes the root's entries, then picks the first child (in
    /// document order) whose criteria match, recursively.
    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = pk::read_fixed_string::<10, 10>(
            &mut r,
            &StringSpec {
                fixed_bits: Some(80),
                max_bits: 80,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "Name",
        )?;
        let v2 = r.uint_order(8, false, "Major")? as u8;
        let v3 = r.uint_order(8, false, "Minor")? as u8;
        let v4 = r.uint_order(8, false, "Patch")? as u8;
        let v5 = pk::read_fixed_string::<1, 1>(
            &mut r,
            &StringSpec {
                fixed_bits: Some(8),
                max_bits: 8,
                terminator: Some(&[0x00]),
                size_tag_bits: 0,
                charset: pk::Charset::Ascii,
            },
            "Separator",
        )?;
        let v6 = r.fixed_bytes::<16>(16, "Digest")?;
        let v7 = r.uint_order(32, false, "Sequence")? as u32;
        let v8 = r.uint_order(32, false, "Generation")? as u32;
        let v9 = r.uint_order(1, false, "CommandedRestart")? != 0;
        let v10 = r.uint_order(7, false, "Spare")? as u8;
        let v11 = r.fixed_bytes::<16>(16, "Interface")?;
        if (v1 == "tcspecial ") && (v5 == " ") && ((v10 as i128) == 0) {
            return Ok((
                Self::Beacon(Beacon {
                    major: v2,
                    minor: v3,
                    patch: v4,
                    digest: v6,
                    sequence: v7,
                    generation: v8,
                    commanded_restart: v9,
                    interface: v11,
                }),
                r.used(),
            ));
        }
        return Err(PacketError::NoMatch {
            container: "/TcsBeacon/BeaconLayout",
        });
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        match self {
            Self::Beacon(p) => p.encode_into(w),
        }
    }
}

impl fmt::Display for BeaconLayoutKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Beacon(p) => p.fmt(f),
        }
    }
}

impl BeaconLayoutKind {
    /// The most fields with an alarm that a member has.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of the fields of this packet derived from
    /// `/TcsBeacon/BeaconLayout`, as its own `alarms()` does.
    ///
    /// # Returns
    ///
    /// The alarm level of each field with an alarm, `None` when the field is
    /// absent.
    pub fn alarms(&self) -> pk::Alarms<0> {
        let mut out = pk::Alarms::new();
        match self {
            Self::Beacon(p) => {
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
    /// `/TcsBeacon/BeaconLayout` triggers, as its own `algorithms()` does.
    ///
    /// # Returns
    ///
    /// The output of each, `None` when it can't be computed.
    pub fn algorithms(&self) -> pk::Outputs<0> {
        let mut out = pk::Outputs::new();
        match self {
            Self::Beacon(p) => {
                for o in p.algorithms().as_slice() {
                    out.push(o.name, o.value);
                }
            }
        }
        out
    }

    /// Evaluates the alarms of the fields of this packet derived from
    /// `/TcsBeacon/BeaconLayout`, as its own `monitor()` does.
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
            Self::Beacon(p) => {
                for a in p.monitor(m, t).as_slice() {
                    out.push(a.field, a.level);
                }
            }
        }
        out
    }
}

impl From<Beacon> for BeaconLayoutKind {
    fn from(p: Beacon) -> Self {
        Self::Beacon(p)
    }
}
