// @generated automatically by tcs-xtce from tcsprotocol/xtce/beacon.xtce
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
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Beacon {
    /// `/TcsBeacon/BeaconName` (/TcsBeacon/BeaconNameType)
    pub beacon_name: pk::FixedString<9>,
    /// `/TcsBeacon/BeaconVersion` (/TcsBeacon/BeaconVersionType)
    pub beacon_version: pk::FixedString<11>,
    /// `/TcsBeacon/BeaconDigest` (/TcsBeacon/BeaconDigestType)
    pub beacon_digest: pk::FixedString<32>,
    /// `/TcsBeacon/BeaconNumber` (/TcsBeacon/BeaconNumberType)
    pub beacon_number: pk::FixedString<10>,
    /// `/TcsBeacon/BeaconFlag` (/TcsBeacon/BeaconFlagType)
    pub beacon_flag: pk::FixedString<7>,
    /// `/TcsBeacon/BeaconInterface` (/TcsBeacon/BeaconInterfaceType)
    pub beacon_interface: pk::FixedString<32>,
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
    const MAX_BYTES: usize = 106;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = pk::read_fixed_string::<9, 10>(
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
        let v2 = pk::read_fixed_string::<11, 12>(
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
        let v3 = pk::read_fixed_string::<32, 33>(
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
        let v4 = pk::read_fixed_string::<10, 11>(
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
        let v5 = pk::read_fixed_string::<7, 8>(
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
        let v6 = pk::read_fixed_string::<32, 32>(
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
                beacon_name: v1,
                beacon_version: v2,
                beacon_digest: v3,
                beacon_number: v4,
                beacon_flag: v5,
                beacon_interface: v6,
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
