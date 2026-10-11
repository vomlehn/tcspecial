//! I2C groups: a bus, and the devices addressed on it.
//!
//! The kind of endpoint that is not located by a name alone. Every endpoint
//! of an I2C group opens the same bus device and is told apart by the address
//! the master sends to, which is why an I2C endpoint carries both and why the
//! addresses the specification reserves are refused here rather than at the
//! first transfer.
//!
//! There is no framing to configure: the protocol fixes it at eight data bits,
//! most significant first, followed by an acknowledge bit.

use std::time::Duration;

use serde::Serialize;

use crate::endpoint_config::{parse_flag, parse_timeout, reject_stream};
use crate::endpoint_config::{
    bad, parse_u32, reject_foreign_fields, EndpointConfigResult, GroupKind, LinkTerms,
};

/// Attributes of an I2C group.
///
/// There is no byte length, parity, or stop bits here, because the protocol
/// fixes the framing of a byte: eight data bits, most significant first,
/// followed by an acknowledge bit. What is left to configure is how the
/// master addresses a device and how hard it tries to complete a transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct I2cParams {
    /// Address devices with ten address bits rather than seven.
    pub ten_bit: bool,
    /// Append an SMBus packet error check, a CRC-8, to each transfer.
    pub pec: bool,
    /// Times a transfer is retried after a lost arbitration or an
    /// unexpected NAK.
    pub retries: u32,
    /// How long a transfer waits before it fails, rounded up to the 10 ms
    /// the bus driver keeps it in.
    pub timeout: Option<Duration>,
    /// Bus clock rate in Hz, as the file recorded it.
    ///
    /// Recorded, never applied: the rate belongs to the bus controller, which
    /// the platform configures from the device tree or from ACPI, and a
    /// program holding an endpoint open cannot change it. Keeping it lets the
    /// rate a group expects be checked against the platform and reported
    /// plainly, rather than inferred later from corrupted transfers.
    pub bus_speed: Option<u32>,
}

impl I2cParams {
    /// The bus driver keeps a timeout in units of 10 ms, so a timeout that is
    /// not a whole number of them takes the next one up.
    pub fn effective_timeout(&self) -> Option<Duration> {
        self.timeout.map(|t| {
            let units = t.as_millis().div_ceil(10).max(1);
            Duration::from_millis(units as u64 * 10)
        })
    }
}

/// Addresses the I2C specification keeps for itself, and so which cannot
/// name a device.
///
/// `0x00` to `0x07` carry the general call, the CBUS address, and the
/// high-speed master code; `0x78` to `0x7F` carry the 10-bit addressing
/// prefix and the block the specification reserves for future use. What is
/// left, `0x08` to `0x77`, is the 112 addresses a 7-bit bus really offers.
///
/// These apply to 7-bit addressing only. A 10-bit transfer is introduced by
/// the `0x78` prefix and then carries its address in the bytes that follow,
/// so the whole 10-bit space is available.
const I2C_RESERVED_LOW: std::ops::RangeInclusive<u32> = 0x00..=0x07;
const I2C_RESERVED_HIGH: std::ops::RangeInclusive<u32> = 0x78..=0x7F;

/// Parse an I2C slave address, in decimal or in hex with an `0x` prefix.
///
/// How wide an address may be follows from the group's addressing mode, so
/// a file that writes a 10-bit address in a 7-bit group is told which of the
/// two it got wrong.
pub(crate) fn parse_i2c_address(group: &str, text: &str, ten_bit: bool) -> EndpointConfigResult<u16> {
    let text = text.trim();
    let parsed = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => text.parse::<u32>(),
    };
    let value =
        parsed.map_err(|_| bad(group, &format!("address: \"{text}\" is not a whole number")))?;

    let (limit, width) = if ten_bit { (0x3FF, 10) } else { (0x7F, 7) };
    if value > limit {
        // Worth suggesting the wider mode only to a group that is not
        // already in it.
        let hint = if ten_bit {
            ""
        } else {
            ". A group addressing its devices with ten bits says ten_bit: true"
        };
        return Err(bad(
            group,
            &format!("address {text} does not fit in {width} bits: expected 0 to {limit:#X}{hint}"),
        ));
    }

    // A reserved address is in range and still cannot name a device, so it is
    // worth catching here rather than as a silent failure to answer on a bus.
    if !ten_bit && (I2C_RESERVED_LOW.contains(&value) || I2C_RESERVED_HIGH.contains(&value)) {
        return Err(bad(
            group,
            &format!(
                "address {text} is reserved by the I2C specification and cannot name a \
                 device: {:#04X} to {:#04X} and {:#04X} to {:#04X} are reserved, leaving \
                 {:#04X} to {:#04X} for devices",
                I2C_RESERVED_LOW.start(),
                I2C_RESERVED_LOW.end(),
                I2C_RESERVED_HIGH.start(),
                I2C_RESERVED_HIGH.end(),
                I2C_RESERVED_LOW.end() + 1,
                I2C_RESERVED_HIGH.start() - 1,
            ),
        ));
    }

    Ok(value as u16)
}

/// What an I2C group says, and what it must say.
///
/// Nothing is required: a bus with no attributes stated is a 7-bit bus with
/// no packet error check, no retries, and whatever rate the platform has it
/// at. There is no stream section, because a transfer on a bus is delimited
/// by the transfer itself rather than by anything in the data.
pub(crate) fn group_kind_of(
    name: &str,
    g: LinkTerms,
) -> EndpointConfigResult<GroupKind> {

    reject_foreign_fields(
        name,
        "i2c",
        &g,
        &["ten_bit", "pec", "retries", "timeout", "bus_speed"],
    )?;
    reject_stream(name, "i2c", g.stream.is_some())?;

    let ten_bit = parse_flag(name, "ten_bit", g.ten_bit.as_ref())?.unwrap_or(false);
    let pec = parse_flag(name, "pec", g.pec.as_ref())?.unwrap_or(false);

    let retries = match g.retries.as_ref() {
        Some(s) => parse_u32(name, "retries", s)?,
        None => 0,
    };

    let timeout = match g.timeout.as_ref() {
        Some(s) => parse_timeout(name, s)?,
        None => None,
    };

    // Recorded, not applied: see I2cParams::bus_speed. A rate of zero
    // is still rejected, because a file that records one is making a
    // claim about the platform and zero is not a claim.
    let bus_speed = match g.bus_speed.as_ref() {
        Some(s) => {
            let hz = parse_u32(name, "bus_speed", s)?;
            if hz == 0 {
                return Err(bad(name, "bus_speed must be greater than zero"));
            }
            Some(hz)
        }
        None => None,
    };

    Ok(GroupKind::I2c(I2cParams {
        ten_bit,
        pec,
        retries,
        timeout,
        bus_speed,
    }))
}
