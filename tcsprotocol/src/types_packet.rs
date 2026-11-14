// @generated automatically by tcs-xtce from tcsprotocol/xtce/types.xtce
// (SpaceSystem 'TcsTypes').
// Any changes made to this file will be lost when it is regenerated;
// change the XTCE document instead and rerun `python3 -m tcs_xtce`.
// tcs-xtce: https://github.com/vomlehn/tcs-xtce
// Rust crate: https://crates.io/crates/tcs-xtce
//! Packet codecs for the `TcsTypes` mission.
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

/// Sequence container `/TcsTypes/u8_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct U8Packet {
    /// `/TcsTypes/u8_datum` (/TcsTypes/u8)
    pub u8_datum: u8,
}

impl U8Packet {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/u8_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/u8_packet` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/u8_packet` packet
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

impl Packet for U8Packet {
    const NAME: &'static str = "/TcsTypes/u8_packet";
    const MAX_BYTES: usize = 1;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v1 = r.uint_order(8, false, "u8_datum")? as u8;
        Ok((U8Packet { u8_datum: v1 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(
            8,
            false,
            pk::from_unsigned(self.u8_datum as u64, 8, "u8_datum")?,
        );
        Ok(())
    }
}

impl fmt::Display for U8Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("U8Packet{")?;
        f.write_str("u8_datum=")?;
        write!(f, "{}", self.u8_datum)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/u16_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct U16Packet {
    /// `/TcsTypes/u16_datum` (/TcsTypes/u16)
    pub u16_datum: u16,
}

impl U16Packet {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/u16_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/u16_packet` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/u16_packet` packet
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

impl Packet for U16Packet {
    const NAME: &'static str = "/TcsTypes/u16_packet";
    const MAX_BYTES: usize = 2;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v2 = r.uint_order(16, false, "u16_datum")? as u16;
        Ok((U16Packet { u16_datum: v2 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(
            16,
            false,
            pk::from_unsigned(self.u16_datum as u64, 16, "u16_datum")?,
        );
        Ok(())
    }
}

impl fmt::Display for U16Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("U16Packet{")?;
        f.write_str("u16_datum=")?;
        write!(f, "{}", self.u16_datum)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/u32_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct U32Packet {
    /// `/TcsTypes/u32_datum` (/TcsTypes/u32)
    pub u32_datum: u32,
}

impl U32Packet {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/u32_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/u32_packet` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/u32_packet` packet
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

impl Packet for U32Packet {
    const NAME: &'static str = "/TcsTypes/u32_packet";
    const MAX_BYTES: usize = 4;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v3 = r.uint_order(32, false, "u32_datum")? as u32;
        Ok((U32Packet { u32_datum: v3 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(
            32,
            false,
            pk::from_unsigned(self.u32_datum as u64, 32, "u32_datum")?,
        );
        Ok(())
    }
}

impl fmt::Display for U32Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("U32Packet{")?;
        f.write_str("u32_datum=")?;
        write!(f, "{}", self.u32_datum)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/u64_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct U64Packet {
    /// `/TcsTypes/u64_datum` (/TcsTypes/u64)
    pub u64_datum: u64,
}

impl U64Packet {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/u64_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/u64_packet` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/u64_packet` packet
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

impl Packet for U64Packet {
    const NAME: &'static str = "/TcsTypes/u64_packet";
    const MAX_BYTES: usize = 8;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v4 = r.uint_order(64, false, "u64_datum")? as u64;
        Ok((U64Packet { u64_datum: v4 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(
            64,
            false,
            pk::from_unsigned(self.u64_datum as u64, 64, "u64_datum")?,
        );
        Ok(())
    }
}

impl fmt::Display for U64Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("U64Packet{")?;
        f.write_str("u64_datum=")?;
        write!(f, "{}", self.u64_datum)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/i8_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct I8Packet {
    /// `/TcsTypes/i8_datum` (/TcsTypes/i8)
    pub i8_datum: i8,
}

impl I8Packet {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/i8_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/i8_packet` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/i8_packet` packet
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

impl Packet for I8Packet {
    const NAME: &'static str = "/TcsTypes/i8_packet";
    const MAX_BYTES: usize = 1;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v5 = pk::to_signed(
            r.uint_order(8, false, "i8_datum")?,
            8,
            IntEncoding::TwosComplement,
        ) as i8;
        Ok((I8Packet { i8_datum: v5 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(
            8,
            false,
            pk::from_signed(
                self.i8_datum as i64,
                8,
                IntEncoding::TwosComplement,
                "i8_datum",
            )?,
        );
        Ok(())
    }
}

impl fmt::Display for I8Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("I8Packet{")?;
        f.write_str("i8_datum=")?;
        write!(f, "{}", self.i8_datum)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/i16_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct I16Packet {
    /// `/TcsTypes/i16_datum` (/TcsTypes/i16)
    pub i16_datum: i16,
}

impl I16Packet {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/i16_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/i16_packet` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/i16_packet` packet
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

impl Packet for I16Packet {
    const NAME: &'static str = "/TcsTypes/i16_packet";
    const MAX_BYTES: usize = 2;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v6 = pk::to_signed(
            r.uint_order(16, false, "i16_datum")?,
            16,
            IntEncoding::TwosComplement,
        ) as i16;
        Ok((I16Packet { i16_datum: v6 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(
            16,
            false,
            pk::from_signed(
                self.i16_datum as i64,
                16,
                IntEncoding::TwosComplement,
                "i16_datum",
            )?,
        );
        Ok(())
    }
}

impl fmt::Display for I16Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("I16Packet{")?;
        f.write_str("i16_datum=")?;
        write!(f, "{}", self.i16_datum)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/i32_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct I32Packet {
    /// `/TcsTypes/i32_datum` (/TcsTypes/i32)
    pub i32_datum: i32,
}

impl I32Packet {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/i32_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/i32_packet` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/i32_packet` packet
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

impl Packet for I32Packet {
    const NAME: &'static str = "/TcsTypes/i32_packet";
    const MAX_BYTES: usize = 4;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v7 = pk::to_signed(
            r.uint_order(32, false, "i32_datum")?,
            32,
            IntEncoding::TwosComplement,
        ) as i32;
        Ok((I32Packet { i32_datum: v7 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(
            32,
            false,
            pk::from_signed(
                self.i32_datum as i64,
                32,
                IntEncoding::TwosComplement,
                "i32_datum",
            )?,
        );
        Ok(())
    }
}

impl fmt::Display for I32Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("I32Packet{")?;
        f.write_str("i32_datum=")?;
        write!(f, "{}", self.i32_datum)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/i64_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct I64Packet {
    /// `/TcsTypes/i64_datum` (/TcsTypes/i64)
    pub i64_datum: i64,
}

impl I64Packet {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/i64_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/i64_packet` packet's fields like
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/i64_packet` packet
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

impl Packet for I64Packet {
    const NAME: &'static str = "/TcsTypes/i64_packet";
    const MAX_BYTES: usize = 8;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v8 = pk::to_signed(
            r.uint_order(64, false, "i64_datum")?,
            64,
            IntEncoding::TwosComplement,
        ) as i64;
        Ok((I64Packet { i64_datum: v8 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(
            64,
            false,
            pk::from_signed(
                self.i64_datum as i64,
                64,
                IntEncoding::TwosComplement,
                "i64_datum",
            )?,
        );
        Ok(())
    }
}

impl fmt::Display for I64Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("I64Packet{")?;
        f.write_str("i64_datum=")?;
        write!(f, "{}", self.i64_datum)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/boolean_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BooleanPacket {
    /// `/TcsTypes/boolean_datum` (/TcsTypes/boolean)
    pub boolean_datum: bool,
}

impl BooleanPacket {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/boolean_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/boolean_packet` packet's fields
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/boolean_packet` packet
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

impl Packet for BooleanPacket {
    const NAME: &'static str = "/TcsTypes/boolean_packet";
    const MAX_BYTES: usize = 1;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v9 = r.uint_order(8, false, "boolean_datum")? != 0;
        Ok((BooleanPacket { boolean_datum: v9 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(8, false, self.boolean_datum as u64);
        Ok(())
    }
}

impl fmt::Display for BooleanPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BooleanPacket{")?;
        f.write_str("boolean_datum=")?;
        write!(f, "{}", self.boolean_datum)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/kelvin_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct KelvinPacket {
    /// `/TcsTypes/kelvin_datum` (/TcsTypes/kelvin)
    pub kelvin_datum: f64,
}

impl KelvinPacket {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/kelvin_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/kelvin_packet` packet's fields
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/kelvin_packet` packet
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

impl Packet for KelvinPacket {
    const NAME: &'static str = "/TcsTypes/kelvin_packet";
    const MAX_BYTES: usize = 4;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v10 = pk::polynomial(
            pk::int_to_f64(
                r.uint_order(32, false, "kelvin_datum")?,
                32,
                IntEncoding::TwosComplement,
            ),
            &[(1e-06, 1)],
        );
        Ok((KelvinPacket { kelvin_datum: v10 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(
            32,
            false,
            pk::f64_to_int(
                pk::polynomial_inverse(self.kelvin_datum, 0.0, 1e-06),
                32,
                IntEncoding::TwosComplement,
                "kelvin_datum",
            )?,
        );
        Ok(())
    }
}

impl fmt::Display for KelvinPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KelvinPacket{")?;
        f.write_str("kelvin_datum=")?;
        pk::describe_f64(self.kelvin_datum, f)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/volt_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VoltPacket {
    /// `/TcsTypes/volt_datum` (/TcsTypes/volt)
    pub volt_datum: f64,
}

impl VoltPacket {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/volt_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/volt_packet` packet's fields
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/volt_packet` packet
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

impl Packet for VoltPacket {
    const NAME: &'static str = "/TcsTypes/volt_packet";
    const MAX_BYTES: usize = 4;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v11 = pk::polynomial(
            pk::int_to_f64(
                r.uint_order(32, false, "volt_datum")?,
                32,
                IntEncoding::TwosComplement,
            ),
            &[(1e-06, 1)],
        );
        Ok((VoltPacket { volt_datum: v11 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(
            32,
            false,
            pk::f64_to_int(
                pk::polynomial_inverse(self.volt_datum, 0.0, 1e-06),
                32,
                IntEncoding::TwosComplement,
                "volt_datum",
            )?,
        );
        Ok(())
    }
}

impl fmt::Display for VoltPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VoltPacket{")?;
        f.write_str("volt_datum=")?;
        pk::describe_f64(self.volt_datum, f)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/ampere_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AmperePacket {
    /// `/TcsTypes/ampere_datum` (/TcsTypes/ampere)
    pub ampere_datum: f64,
}

impl AmperePacket {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/ampere_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/ampere_packet` packet's fields
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/ampere_packet` packet
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

impl Packet for AmperePacket {
    const NAME: &'static str = "/TcsTypes/ampere_packet";
    const MAX_BYTES: usize = 4;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v12 = pk::polynomial(
            pk::int_to_f64(
                r.uint_order(32, false, "ampere_datum")?,
                32,
                IntEncoding::TwosComplement,
            ),
            &[(1e-06, 1)],
        );
        Ok((AmperePacket { ampere_datum: v12 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(
            32,
            false,
            pk::f64_to_int(
                pk::polynomial_inverse(self.ampere_datum, 0.0, 1e-06),
                32,
                IntEncoding::TwosComplement,
                "ampere_datum",
            )?,
        );
        Ok(())
    }
}

impl fmt::Display for AmperePacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AmperePacket{")?;
        f.write_str("ampere_datum=")?;
        pk::describe_f64(self.ampere_datum, f)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/watt_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WattPacket {
    /// `/TcsTypes/watt_datum` (/TcsTypes/watt)
    pub watt_datum: f64,
}

impl WattPacket {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/watt_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/watt_packet` packet's fields
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/watt_packet` packet
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

impl Packet for WattPacket {
    const NAME: &'static str = "/TcsTypes/watt_packet";
    const MAX_BYTES: usize = 4;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v13 = pk::polynomial(
            pk::int_to_f64(
                r.uint_order(32, false, "watt_datum")?,
                32,
                IntEncoding::TwosComplement,
            ),
            &[(0.001, 1)],
        );
        Ok((WattPacket { watt_datum: v13 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(
            32,
            false,
            pk::f64_to_int(
                pk::polynomial_inverse(self.watt_datum, 0.0, 0.001),
                32,
                IntEncoding::TwosComplement,
                "watt_datum",
            )?,
        );
        Ok(())
    }
}

impl fmt::Display for WattPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WattPacket{")?;
        f.write_str("watt_datum=")?;
        pk::describe_f64(self.watt_datum, f)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/metre_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MetrePacket {
    /// `/TcsTypes/metre_datum` (/TcsTypes/metre)
    pub metre_datum: f64,
}

impl MetrePacket {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/metre_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/metre_packet` packet's fields
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/metre_packet` packet
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

impl Packet for MetrePacket {
    const NAME: &'static str = "/TcsTypes/metre_packet";
    const MAX_BYTES: usize = 4;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v14 = pk::polynomial(
            pk::int_to_f64(
                r.uint_order(32, false, "metre_datum")?,
                32,
                IntEncoding::TwosComplement,
            ),
            &[(0.001, 1)],
        );
        Ok((MetrePacket { metre_datum: v14 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(
            32,
            false,
            pk::f64_to_int(
                pk::polynomial_inverse(self.metre_datum, 0.0, 0.001),
                32,
                IntEncoding::TwosComplement,
                "metre_datum",
            )?,
        );
        Ok(())
    }
}

impl fmt::Display for MetrePacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MetrePacket{")?;
        f.write_str("metre_datum=")?;
        pk::describe_f64(self.metre_datum, f)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/metre_per_second_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MetrePerSecondPacket {
    /// `/TcsTypes/metre_per_second_datum` (/TcsTypes/metre_per_second)
    pub metre_per_second_datum: f64,
}

impl MetrePerSecondPacket {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/metre_per_second_packet`
    /// packet's fields. Alarms that depend on earlier samples count only this
    /// one (see `monitor`).
    ///
    /// # Returns
    ///
    /// The alarm level of each field with an alarm, `None` when the field is
    /// absent.
    pub fn alarms(&self) -> pk::Alarms<0> {
        let mut a = pk::Alarms::new();
        a
    }

    /// Evaluates the alarms of this `/TcsTypes/metre_per_second_packet`
    /// packet's fields like `alarms()`, counting the earlier samples for change
    /// ranges, minViolations and minConformance.
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/metre_per_second_packet`
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

impl Packet for MetrePerSecondPacket {
    const NAME: &'static str = "/TcsTypes/metre_per_second_packet";
    const MAX_BYTES: usize = 4;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v15 = pk::polynomial(
            pk::int_to_f64(
                r.uint_order(32, false, "metre_per_second_datum")?,
                32,
                IntEncoding::TwosComplement,
            ),
            &[(1e-06, 1)],
        );
        Ok((
            MetrePerSecondPacket {
                metre_per_second_datum: v15,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(
            32,
            false,
            pk::f64_to_int(
                pk::polynomial_inverse(self.metre_per_second_datum, 0.0, 1e-06),
                32,
                IntEncoding::TwosComplement,
                "metre_per_second_datum",
            )?,
        );
        Ok(())
    }
}

impl fmt::Display for MetrePerSecondPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MetrePerSecondPacket{")?;
        f.write_str("metre_per_second_datum=")?;
        pk::describe_f64(self.metre_per_second_datum, f)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/newton_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NewtonPacket {
    /// `/TcsTypes/newton_datum` (/TcsTypes/newton)
    pub newton_datum: f64,
}

impl NewtonPacket {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/newton_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/newton_packet` packet's fields
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/newton_packet` packet
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

impl Packet for NewtonPacket {
    const NAME: &'static str = "/TcsTypes/newton_packet";
    const MAX_BYTES: usize = 4;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v16 = pk::polynomial(
            pk::int_to_f64(
                r.uint_order(32, false, "newton_datum")?,
                32,
                IntEncoding::TwosComplement,
            ),
            &[(0.001, 1)],
        );
        Ok((NewtonPacket { newton_datum: v16 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(
            32,
            false,
            pk::f64_to_int(
                pk::polynomial_inverse(self.newton_datum, 0.0, 0.001),
                32,
                IntEncoding::TwosComplement,
                "newton_datum",
            )?,
        );
        Ok(())
    }
}

impl fmt::Display for NewtonPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NewtonPacket{")?;
        f.write_str("newton_datum=")?;
        pk::describe_f64(self.newton_datum, f)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/kilogram_per_second_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct KilogramPerSecondPacket {
    /// `/TcsTypes/kilogram_per_second_datum` (/TcsTypes/kilogram_per_second)
    pub kilogram_per_second_datum: f64,
}

impl KilogramPerSecondPacket {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/kilogram_per_second_packet`
    /// packet's fields. Alarms that depend on earlier samples count only this
    /// one (see `monitor`).
    ///
    /// # Returns
    ///
    /// The alarm level of each field with an alarm, `None` when the field is
    /// absent.
    pub fn alarms(&self) -> pk::Alarms<0> {
        let mut a = pk::Alarms::new();
        a
    }

    /// Evaluates the alarms of this `/TcsTypes/kilogram_per_second_packet`
    /// packet's fields like `alarms()`, counting the earlier samples for change
    /// ranges, minViolations and minConformance.
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

    /// Evaluates the MathAlgorithms that a
    /// `/TcsTypes/kilogram_per_second_packet` packet triggers, from this one's
    /// values.
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

impl Packet for KilogramPerSecondPacket {
    const NAME: &'static str = "/TcsTypes/kilogram_per_second_packet";
    const MAX_BYTES: usize = 4;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v17 = pk::polynomial(
            pk::int_to_f64(
                r.uint_order(32, false, "kilogram_per_second_datum")?,
                32,
                IntEncoding::TwosComplement,
            ),
            &[(1e-06, 1)],
        );
        Ok((
            KilogramPerSecondPacket {
                kilogram_per_second_datum: v17,
            },
            r.used(),
        ))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(
            32,
            false,
            pk::f64_to_int(
                pk::polynomial_inverse(
                    self.kilogram_per_second_datum,
                    0.0,
                    1e-06,
                ),
                32,
                IntEncoding::TwosComplement,
                "kilogram_per_second_datum",
            )?,
        );
        Ok(())
    }
}

impl fmt::Display for KilogramPerSecondPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KilogramPerSecondPacket{")?;
        f.write_str("kilogram_per_second_datum=")?;
        pk::describe_f64(self.kilogram_per_second_datum, f)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/ratio_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RatioPacket {
    /// `/TcsTypes/ratio_datum` (/TcsTypes/ratio)
    pub ratio_datum: f64,
}

impl RatioPacket {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/ratio_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/ratio_packet` packet's fields
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/ratio_packet` packet
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

impl Packet for RatioPacket {
    const NAME: &'static str = "/TcsTypes/ratio_packet";
    const MAX_BYTES: usize = 4;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v18 = pk::polynomial(
            pk::int_to_f64(
                r.uint_order(32, false, "ratio_datum")?,
                32,
                IntEncoding::TwosComplement,
            ),
            &[(1e-06, 1)],
        );
        Ok((RatioPacket { ratio_datum: v18 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        w.uint_order(
            32,
            false,
            pk::f64_to_int(
                pk::polynomial_inverse(self.ratio_datum, 0.0, 1e-06),
                32,
                IntEncoding::TwosComplement,
                "ratio_datum",
            )?,
        );
        Ok(())
    }
}

impl fmt::Display for RatioPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RatioPacket{")?;
        f.write_str("ratio_datum=")?;
        pk::describe_f64(self.ratio_datum, f)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/string_packet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StringPacket {
    /// `/TcsTypes/string_datum` (/TcsTypes/string)
    pub string_datum: pk::FixedString<4096>,
}

impl StringPacket {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/string_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/string_packet` packet's fields
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/string_packet` packet
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

impl Packet for StringPacket {
    const NAME: &'static str = "/TcsTypes/string_packet";
    const MAX_BYTES: usize = 4100;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v19 = pk::read_fixed_string::<4096, 4100>(
            &mut r,
            &StringSpec {
                fixed_bits: None,
                max_bits: 32800,
                terminator: None,
                size_tag_bits: 32,
                charset: pk::Charset::Utf8,
            },
            "string_datum",
        )?;
        Ok((StringPacket { string_datum: v19 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        pk::write_string(
            w,
            &StringSpec {
                fixed_bits: None,
                max_bits: 32800,
                terminator: None,
                size_tag_bits: 32,
                charset: pk::Charset::Utf8,
            },
            &self.string_datum,
            "string_datum",
        )?;
        Ok(())
    }
}

impl fmt::Display for StringPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StringPacket{")?;
        f.write_str("string_datum=")?;
        pk::describe_str(&self.string_datum, f)?;
        f.write_char('}')
    }
}

/// Sequence container `/TcsTypes/bytes_packet`.
///
/// `bytes_length` gives the size of `bytes_datum`: computed from its length
/// when encoding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BytesPacket {
    /// `/TcsTypes/bytes_datum` (/TcsTypes/bytes)
    pub bytes_datum: pk::FixedBytes<4096>,
}

impl BytesPacket {
    /// The number of fields with an alarm.
    pub const ALARMS: usize = 0;

    /// Evaluates the alarms of this `/TcsTypes/bytes_packet` packet's fields.
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

    /// Evaluates the alarms of this `/TcsTypes/bytes_packet` packet's fields
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

    /// Evaluates the MathAlgorithms that a `/TcsTypes/bytes_packet` packet
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

impl Packet for BytesPacket {
    const NAME: &'static str = "/TcsTypes/bytes_packet";
    const MAX_BYTES: usize = 4100;

    fn decode_prefix(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = BitReader::new(data);
        let v20 = r.uint_order(32, false, "bytes_length")? as u32;
        let v21 = r.fixed_bytes::<4096>(
            pk::dynamic_size(v20 as f64, 8.0, 0.0, 8, 4096, "bytes_datum")?,
            "bytes_datum",
        )?;
        Ok((BytesPacket { bytes_datum: v21 }, r.used()))
    }

    fn encode_into(&self, w: &mut BitWriter<'_>) -> Result<()> {
        let d20 = u32::try_from(pk::size_source(
            self.bytes_datum.len(),
            8.0,
            0.0,
            8,
            1,
            "bytes_datum",
        )?)
        .map_err(|_| PacketError::OutOfRange {
            field: "bytes_length",
        })?;
        w.uint_order(
            32,
            false,
            pk::from_unsigned(d20 as u64, 32, "bytes_length")?,
        );
        {
            let n =
                pk::dynamic_size(d20 as f64, 8.0, 0.0, 8, 4096, "bytes_datum")?;
            if self.bytes_datum.len() != n {
                return Err(PacketError::BadSize {
                    field: "bytes_datum",
                });
            }
            w.bytes(&self.bytes_datum);
        }
        Ok(())
    }
}

impl fmt::Display for BytesPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BytesPacket{")?;
        f.write_str("bytes_datum=")?;
        pk::describe_bytes(&self.bytes_datum, f)?;
        f.write_char('}')
    }
}
