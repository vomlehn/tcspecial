//! Whole numbers as a configuration writes them.
//!
//! One grammar for every integer in every file this program reads: a bus
//! address, a port, a packet size, a timeout, a count of restarts, the number
//! in front of an interval's unit. They were parsed in five places with three
//! slightly different grammars -- one took hex, one did not, one took a sign
//! nothing wanted -- which is three answers to "may I write this in hex", and
//! the answer somebody gets depended on which field they were filling in.
//!
//! # What is allowed
//!
//! Decimal, or hexadecimal with an `0x` prefix, with underscores anywhere
//! among the digits:
//!
//! ```text
//! 1000      0x3e8     0X3E8
//! 1_000     0x3_e8    1_0_0_0
//! ```
//!
//! Hexadecimal because half of these numbers are hardware's own: an I2C
//! address is `0x48` in every datasheet and `72` in none of them, and a
//! configuration that made somebody convert it by hand would be a
//! configuration with a transcription error in it. Decimal because the other
//! half are not: a packet is 1024 bytes, not `0x400`.
//!
//! Underscores because a number long enough to be miscounted is a number
//! somebody will miscount: `604800` and `60_4800` are easy to tell apart only
//! when one of them is grouped. They are the same convention Rust and most
//! other languages use, so nobody has to learn one here, and they carry no
//! meaning -- `1_000`, `10_00` and `1000` are one number written three ways.
//!
//! No sign. Every integer in these files counts something -- bytes, ports,
//! microseconds, retries -- and a negative count is not a value with a
//! meaning to refuse later; it is a thing to refuse here.
//!
//! # What is said when it is wrong
//!
//! Two different complaints, because they have two different answers: a
//! number that is not one at all, and a number too large for what it is
//! counting. The second names the limit, since "65536 is not a port" is only
//! useful beside "a port is 1 to 65535".

/// A whole number, as large as any of these files needs.
///
/// The grammar is the module's: decimal or `0x` hexadecimal, underscores
/// allowed among the digits. Everything else here is this with a range check.
pub fn whole(text: &str) -> Result<u64, String> {
    let said = text.trim();
    let (digits, radix, how) = match said
        .strip_prefix("0x")
        .or_else(|| said.strip_prefix("0X"))
    {
        Some(digits) => (digits, 16, "hexadecimal"),
        None => (said, 10, "decimal"),
    };

    // The underscores come out before the parse rather than being checked
    // where they sit: they group digits for a reader and mean nothing to the
    // value, so `1_000` and `10_00` are one number written two ways.
    let bare: String = digits.chars().filter(|c| *c != '_').collect();
    if bare.is_empty() {
        return Err(format!("\"{text}\" is not a whole number"));
    }

    u64::from_str_radix(&bare, radix)
        .map_err(|_| format!("\"{text}\" is not a whole number in {how}"))
}

/// The same, and it has to fit in a `u32`.
pub fn whole_u32(text: &str) -> Result<u32, String> {
    fits(text, u32::MAX as u64).map(|n| n as u32)
}

/// The same, and it has to fit in a `u16`.
pub fn whole_u16(text: &str) -> Result<u16, String> {
    fits(text, u16::MAX as u64).map(|n| n as u16)
}

/// The same, and it has to fit in a byte.
pub fn whole_u8(text: &str) -> Result<u8, String> {
    fits(text, u8::MAX as u64).map(|n| n as u8)
}

/// A whole number no larger than `most`.
///
/// Said as its own complaint, naming the limit: "4294967296 is not a whole
/// number" would be a lie about a number that is perfectly whole and merely
/// too big for what it is counting.
fn fits(text: &str, most: u64) -> Result<u64, String> {
    let n = whole(text)?;
    if n > most {
        return Err(format!(
            "{n} does not fit in this field: the most it holds is {most}"
        ));
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three spellings of one number are one number.
    #[test]
    fn decimal_hexadecimal_and_grouped_are_the_same_number() {
        for said in ["1000", "1_000", "1_0_0_0", "0x3e8", "0X3E8", "0x3_e8", " 1000 "] {
            assert_eq!(whole(said), Ok(1000), "{said}");
        }
        // And nought is a number here: whether it means anything is the
        // field's business -- a port of nought is refused where it is read,
        // because there the number is wrong rather than unreadable.
        assert_eq!(whole("0"), Ok(0));
        assert_eq!(whole("0x0"), Ok(0));
    }

    /// What is not a whole number is said to be what it is.
    #[test]
    fn what_is_not_a_number_is_refused_in_its_own_words() {
        for said in ["", " ", "_", "0x", "0x_", "ten", "1.5", "-1", "0b101", "48g"] {
            let why = whole(said).expect_err(said);
            assert!(
                why.contains("whole number"),
                "{said} should be refused as not a whole number: {why}"
            );
        }
        // A decimal with hex digits in it says which base it was read in,
        // because that is the thing the writer got wrong.
        let why = whole("3e8").expect_err("3e8 is not decimal");
        assert!(why.contains("decimal"), "{why}");
        let why = whole("0xg").expect_err("g is not a hex digit");
        assert!(why.contains("hexadecimal"), "{why}");
    }

    /// A number too large for its field is said to be too large, and the
    /// limit is named.
    #[test]
    fn a_number_too_large_for_its_field_names_the_limit() {
        assert_eq!(whole_u8("255"), Ok(255));
        assert_eq!(whole_u8("0xff"), Ok(255));
        let why = whole_u8("256").expect_err("256 is not a byte");
        assert!(why.contains("255"), "the limit should be named: {why}");

        assert_eq!(whole_u16("65535"), Ok(65535));
        assert!(whole_u16("65536").is_err());

        assert_eq!(whole_u32("4294967295"), Ok(u32::MAX));
        assert!(whole_u32("4294967296").is_err());

        // And past what any of these fields hold at all.
        assert!(whole("18446744073709551616").is_err());
    }
}
