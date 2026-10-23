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

use crate::endpoint_config::{
    bad, parse_u32, reject_foreign_fields, EndpointConfigResult, GroupKind, LinkTerms,
};
use crate::endpoint_config::{parse_flag, parse_timeout, reject_stream};

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
pub(crate) fn parse_i2c_address(
    group: &str,
    text: &str,
    ten_bit: bool,
) -> EndpointConfigResult<u16> {
    let text = text.trim();
    // The one grammar every integer in these files is written in: decimal, or
    // hexadecimal with an `0x` prefix, underscores allowed among the digits.
    // An address is the field that wants the hexadecimal most -- a datasheet
    // says 0x48 and never 72 -- and the same spelling works everywhere else.
    let value =
        crate::whole::whole_u32(text).map_err(|why| bad(group, &format!("address: {why}")))?;

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
pub(crate) fn group_kind_of(name: &str, g: LinkTerms) -> EndpointConfigResult<GroupKind> {
    reject_foreign_fields(name, "i2c", &g, &["ten_bit", "pec", "retries", "timeout"])?;
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

    Ok(GroupKind::I2c(I2cParams {
        ten_bit,
        pec,
        retries,
        timeout,
    }))
}

/// The name the kernel's `i2c-stub` gives the adapter it makes, which is how
/// `bin/mki2c` finds it and how a stand-in bus is told from a real one.
pub const STUB_ADAPTER: &str = "SMBus stub driver";

/// What `--simulate` asks to change on an I2C node that is not a stand-in.
const TO_STAND_IN: &str = "point path at the i2c-stub bus, which bin/mki2c makes";

/// Whether the bus at `bus` is the kernel's `i2c-stub`, from the name of the
/// adapter behind it. See [`crate::simulate`].
///
/// By the adapter's name and not the bus number, which is given out as
/// adapters appear: `/dev/i2c-7` is the stub on one host and a display's
/// gmbus on the next.
pub fn judged(host: &crate::simulate::Host, bus: &str) -> crate::simulate::Judged {
    match host.device_of(bus) {
        Ok(device) => adapter_judged(bus, &device),
        Err(found) => crate::simulate::Judged::cannot_tell(found, "path", TO_STAND_IN),
    }
}

/// The judgement of a bus whose kernel directory is `device`: an i2c-dev
/// class device, whose `name` is its adapter's.
fn adapter_judged(bus: &str, device: &std::path::Path) -> crate::simulate::Judged {
    use crate::simulate::{read_line, Judged};
    match read_line(&device.join("name")) {
        Some(name) if name == STUB_ADAPTER => Judged::stand_in(
            format!("{bus} is the i2c-stub's adapter"),
            "path",
            TO_STAND_IN,
        ),
        Some(name) => Judged::hardware(
            format!("{bus} is a real adapter ({name})"),
            "path",
            TO_STAND_IN,
        ),
        None => Judged::cannot_tell(
            format!("{bus}'s adapter has no name in {}", device.display()),
            "path",
            TO_STAND_IN,
        ),
    }
}

#[cfg(test)]
mod stand_in_tests {
    use super::*;
    use crate::simulate::Verdict;

    fn adapter(name: Option<&str>) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        if let Some(name) = name {
            std::fs::write(dir.path().join("name"), format!("{name}\n")).unwrap();
        }
        dir
    }

    #[test]
    fn the_stub_is_a_stand_in_and_a_real_adapter_is_named() {
        let stub = adapter(Some(STUB_ADAPTER));
        assert_eq!(
            adapter_judged("/dev/i2c-7", stub.path()).verdict,
            Verdict::StandIn
        );

        let real = adapter(Some("i915 gmbus dpb"));
        let judged = adapter_judged("/dev/i2c-1", real.path());
        assert_eq!(judged.verdict, Verdict::Hardware);
        assert_eq!(
            judged.found,
            "/dev/i2c-1 is a real adapter (i915 gmbus dpb)"
        );

        let nameless = adapter(None);
        assert_eq!(
            adapter_judged("/dev/i2c-2", nameless.path()).verdict,
            Verdict::CannotTell
        );
    }

    #[test]
    fn a_bus_with_nothing_at_it_cannot_be_told() {
        let judged = judged(&crate::simulate::Host::this(), "/nonexistent/i2c-99");
        assert_eq!(judged.verdict, Verdict::CannotTell);
        assert_eq!(judged.found, "nothing is at /nonexistent/i2c-99");
    }
}
