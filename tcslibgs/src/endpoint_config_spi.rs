//! SPI groups: the clock, the mode, and the word.
//!
//! A SPI endpoint is located by its device node alone -- the bus and the chip
//! select are both named by it -- so unlike I2C there is no address beside it.
//! What a file states instead is how the controller must clock the
//! peripheral: the rate it will stand, which edge of the clock samples, how
//! wide a word is, which end of it goes first, and which level of the chip
//! select means "selected".

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::endpoint_config::reject_stream;
use crate::endpoint_config::{
    bad, parse_u32, reject_foreign_fields, require, EndpointConfigResult, GroupKind,
    LinkTerms, Scalar,
};

/// Attributes of a SPI group.
///
/// As with I2C there is no parity and there are no stop bits: SPI is clocked
/// and full duplex, and a transfer is delimited by the chip select rather
/// than by framing bits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpiParams {
    /// Greatest clock rate the peripheral accepts, in Hz.
    ///
    /// An upper bound, not an exact rate: a controller divides its own clock
    /// down and runs at the fastest rate it can produce that does not exceed
    /// this one.
    pub max_speed: u32,
    /// Clock polarity and phase.
    pub mode: SpiMode,
    /// Bits in each word.
    pub bits_per_word: BitsPerWord,
    /// Which bit of a word goes first.
    pub bit_order: BitOrder,
    /// The level at which the chip select is asserted.
    pub cs_active: CsActive,
}

/// Clock polarity and phase, the four combinations of which are numbered 0
/// to 3 by every SPI peripheral's datasheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpiMode {
    Mode0,
    Mode1,
    Mode2,
    Mode3,
}

impl SpiMode {
    /// The mode number, 0 to 3.
    pub fn number(&self) -> u8 {
        match self {
            SpiMode::Mode0 => 0,
            SpiMode::Mode1 => 1,
            SpiMode::Mode2 => 2,
            SpiMode::Mode3 => 3,
        }
    }

    /// Clock polarity: the idle level of the clock.
    pub fn cpol(&self) -> bool {
        self.number() & 2 != 0
    }

    /// Clock phase: false samples on the leading edge, true on the trailing.
    pub fn cpha(&self) -> bool {
        self.number() & 1 != 0
    }
}

impl fmt::Display for SpiMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.number())
    }
}

/// Which bit of a word is sent first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BitOrder {
    MsbFirst,
    LsbFirst,
}

/// The level at which a chip select is asserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CsActive {
    Low,
    High,
}

/// Bits in each word on a SPI bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BitsPerWord(u8);

impl BitsPerWord {
    /// Narrowest word a controller offers.
    pub const MIN: u8 = 1;
    /// Widest word a controller offers.
    pub const MAX: u8 = 32;
    /// The width nearly every peripheral uses.
    pub const DEFAULT: u8 = 8;

    /// Build a word width, rejecting one no controller can produce.
    pub fn new(bits: u8) -> Result<Self, String> {
        if (Self::MIN..=Self::MAX).contains(&bits) {
            Ok(BitsPerWord(bits))
        } else {
            Err(format!(
                "bits_per_word {bits} is out of range: expected {} to {}",
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

impl Default for BitsPerWord {
    fn default() -> Self {
        BitsPerWord(Self::DEFAULT)
    }
}

fn parse_spi_mode(group: &str, s: &Scalar) -> EndpointConfigResult<SpiMode> {
    match s.as_str() {
        "0" => Ok(SpiMode::Mode0),
        "1" => Ok(SpiMode::Mode1),
        "2" => Ok(SpiMode::Mode2),
        "3" => Ok(SpiMode::Mode3),
        other => Err(bad(
            group,
            &format!("mode: \"{other}\" is not one of 0, 1, 2, or 3"),
        )),
    }
}

fn parse_bit_order(group: &str, s: &Scalar) -> EndpointConfigResult<BitOrder> {
    match s.as_str().to_ascii_lowercase().as_str() {
        "msb" | "msb_first" | "msb-first" => Ok(BitOrder::MsbFirst),
        "lsb" | "lsb_first" | "lsb-first" => Ok(BitOrder::LsbFirst),
        other => Err(bad(
            group,
            &format!("bit_order: \"{other}\" is not msb or lsb"),
        )),
    }
}

fn parse_cs_active(group: &str, s: &Scalar) -> EndpointConfigResult<CsActive> {
    match s.as_str().to_ascii_lowercase().as_str() {
        "low" => Ok(CsActive::Low),
        "high" => Ok(CsActive::High),
        other => Err(bad(
            group,
            &format!("cs_active: \"{other}\" is not low or high"),
        )),
    }
}


/// What a SPI group says, and what it must say.
///
/// The clock rate and the mode are required: no controller default is right
/// for every peripheral, and a mismatched mode fails the way a mismatched
/// data rate fails on a serial line. The rest have usual values and default
/// to them. There is no stream section, for the reason an I2C group has none.
pub(crate) fn group_kind_of(
    name: &str,
    g: LinkTerms,
) -> EndpointConfigResult<GroupKind> {

    reject_foreign_fields(
        name,
        "spi",
        &g,
        &["max_speed", "spi_mode", "bits_per_word", "bit_order", "cs_active"],
    )?;
    reject_stream(name, "spi", g.stream.is_some())?;

    let max_speed = require(name, "spi", "max_speed", g.max_speed.as_ref())?;
    let max_speed = parse_u32(name, "max_speed", max_speed)?;
    if max_speed == 0 {
        return Err(bad(name, "max_speed must be greater than zero"));
    }

    // Required: no controller default is right for every peripheral,
    // and a mismatched mode fails the way a mismatched data rate
    // fails on a serial line.
    let mode = require(name, "spi", "spi_mode", g.spi_mode.as_ref())?;
    let mode = parse_spi_mode(name, mode)?;

    let bits_per_word = match g.bits_per_word.as_ref() {
        Some(s) => {
            let bits = parse_u32(name, "bits_per_word", s)?;
            BitsPerWord::new(bits.min(u8::MAX as u32) as u8).map_err(|m| bad(name, &m))?
        }
        None => BitsPerWord::default(),
    };

    let bit_order = match g.bit_order.as_ref() {
        Some(s) => parse_bit_order(name, s)?,
        None => BitOrder::MsbFirst,
    };

    let cs_active = match g.cs_active.as_ref() {
        Some(s) => parse_cs_active(name, s)?,
        None => CsActive::Low,
    };

    Ok(GroupKind::Spi(SpiParams {
        max_speed,
        mode,
        bits_per_word,
        bit_order,
        cs_active,
    }))
}
