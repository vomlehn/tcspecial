//! A link's types as a program reads and writes them: one line of text.
//!
//! A `process` computation hands its program a line per arrival and reads a
//! line per answer, and each type has one way of being written as text: a
//! value in decimal in its unit; a whole number in decimal; a `boolean` as `1`
//! or `0`; and a structure as its fields, `name=value`, separated by spaces --
//! `x=0.12 y=-0.40 z=9.81`. A string and bytes have none, a line being no
//! place for either, and a `process` that states one is refused. See "Typed
//! links" in `docs/tcspecial-design.rst`.

use std::fmt::Write as _;

use crate::link_type::{Basic, LinkType};
use crate::node::vehicle::compute::{from_text, from_value, value_bytes};

/// Says whether a type has a text form.
///
/// # Arguments
///
/// * `kind` - The type.
///
/// # Returns
///
/// True but for a string, bytes, and a structure, whose fields are of fixed
/// width and so never either.
pub fn has_text(kind: &LinkType) -> bool {
    !matches!(kind, LinkType::Basic(Basic::String | Basic::Bytes))
}

/// Writes a datum as a program reads it, on the end of a line.
///
/// # Arguments
///
/// * `kind` - The datum's type, which it has been checked to be.
/// * `data` - The datum, in its type's own layout.
/// * `out`  - Where the text goes, appended to; it allocates nothing where
///            it has room.
///
/// # Errors
///
/// A message for a type with no text form, or a datum not of its type.
pub fn write_text(
    kind: &LinkType,
    data: &[u8],
    out: &mut String,
) -> Result<(), String> {
    match kind {
        LinkType::Basic(basic) => write_basic(*basic, data, out),
        LinkType::Structure(fields) => {
            let mut at = 0;
            for (n, (name, basic)) in fields.iter().enumerate() {
                let wide = basic.width().unwrap_or(0);
                let field = data.get(at..at + wide).ok_or_else(|| {
                    format!("a {kind} is longer than what arrived")
                })?;
                if n > 0 {
                    out.push(' ');
                }
                let _ = write!(out, "{name}=");
                write_basic(*basic, field, out)?;
                at += wide;
            }
            Ok(())
        }
    }
}

/// Reads a datum a program wrote as text.
///
/// # Arguments
///
/// * `kind` - The type it is to be.
/// * `said` - The text, without its newline.
/// * `what` - Who wrote it, for the messages.
/// * `out`  - Where the datum goes, in its type's own layout: room for its
///            width.
///
/// # Returns
///
/// How many bytes of `out` the datum takes.
///
/// # Errors
///
/// A message naming `what` for text that is not that type: a number that is
/// not one or does not fit, a boolean that is neither `1` nor `0`, a
/// structure missing a field or naming one twice or one it does not have.
pub fn read_text(
    kind: &LinkType,
    said: &str,
    what: &str,
    out: &mut [u8],
) -> Result<usize, String> {
    match kind {
        LinkType::Basic(basic) => read_basic(*basic, said.trim(), what, out),
        LinkType::Structure(fields) => {
            let mut seen = [false; crate::link_type::MOST_FIELDS];
            for pair in said.split_whitespace() {
                let Some((name, value)) = pair.split_once('=') else {
                    return Err(format!(
                        "{what}: \"{pair}\" is not a field, written as \
                         name=value"
                    ));
                };
                let Some(n) = fields.iter().position(|(f, _)| f == name) else {
                    return Err(format!(
                        "{what}: a {kind} has no field \"{name}\""
                    ));
                };
                if seen[n] {
                    return Err(format!(
                        "{what}: field \"{name}\" is written twice"
                    ));
                }
                seen[n] = true;
                let at: usize = fields[..n]
                    .iter()
                    .map(|(_, basic)| basic.width().unwrap_or(0))
                    .sum();
                read_basic(fields[n].1, value, what, &mut out[at..])?;
            }
            if let Some(n) = (0..fields.len()).find(|&n| !seen[n]) {
                return Err(format!(
                    "{what}: field \"{}\" is not written: a {kind} is every \
                     one of its fields",
                    fields[n].0
                ));
            }
            Ok(fields
                .iter()
                .map(|(_, basic)| basic.width().unwrap_or(0))
                .sum())
        }
    }
}

/// Writes one of `types.xtce`'s types as text: see [`write_text`].
///
/// # Arguments
///
/// * `basic` - The type.
/// * `data`  - The datum.
/// * `out`   - Where the text goes.
///
/// # Errors
///
/// A message for a string or bytes, or a datum not of the type.
fn write_basic(
    basic: Basic,
    data: &[u8],
    out: &mut String,
) -> Result<(), String> {
    match basic {
        Basic::Value(unit) => {
            let _ = write!(out, "{}", from_value(data, unit, "what arrived")?);
        }
        Basic::Boolean => {
            let _ = write!(out, "{}", data.first().copied().unwrap_or(0));
        }
        Basic::String | Basic::Bytes => {
            return Err(format!("a {} has no text form", basic.name()))
        }
        whole => {
            let (number, negative) = whole_of(whole, data)?;
            if negative {
                let _ = write!(out, "-{number}");
            } else {
                let _ = write!(out, "{number}");
            }
        }
    }
    Ok(())
}

/// Reads one of `types.xtce`'s types from text: see [`read_text`].
///
/// # Arguments
///
/// * `basic` - The type.
/// * `said`  - The text.
/// * `what`  - Who wrote it.
/// * `out`   - Where the datum goes.
///
/// # Returns
///
/// The type's width.
///
/// # Errors
///
/// A message naming `what` for text that is not the type.
fn read_basic(
    basic: Basic,
    said: &str,
    what: &str,
    out: &mut [u8],
) -> Result<usize, String> {
    match basic {
        Basic::Value(unit) => {
            let number = from_text(said, unit, what)?;
            let bytes = value_bytes(number, unit)
                .map_err(|why| format!("{what}: {why}"))?;
            out[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }
        Basic::Boolean => {
            out[0] = match said {
                "1" => 1,
                "0" => 0,
                _ => {
                    return Err(format!(
                        "{what}: \"{said}\" is not a boolean, which is 1 or 0"
                    ))
                }
            };
            Ok(1)
        }
        Basic::String | Basic::Bytes => {
            Err(format!("{what}: a {} has no text form", basic.name()))
        }
        whole => {
            let bits = whole.bits().expect("a whole number") as usize;
            let signed = matches!(
                whole,
                Basic::I8 | Basic::I16 | Basic::I32 | Basic::I64
            );
            let number: i128 = said.parse().map_err(|_| {
                format!("{what}: \"{said}\" is not a whole number")
            })?;
            let (least, most) = if signed {
                (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
            } else {
                (0, (1i128 << bits) - 1)
            };
            if number < least || number > most {
                return Err(format!(
                    "{what}: {number} is not a {}, which holds {least} to \
                     {most}",
                    whole.name()
                ));
            }
            let width = bits / 8;
            out[..width].copy_from_slice(&number.to_be_bytes()[16 - width..]);
            Ok(width)
        }
    }
}

/// Reads a whole number of its type's own layout.
///
/// # Arguments
///
/// * `whole` - The type.
/// * `data`  - The datum.
///
/// # Returns
///
/// Its size, and whether it is negative.
///
/// # Errors
///
/// A message for a datum not of the type's width.
fn whole_of(whole: Basic, data: &[u8]) -> Result<(u64, bool), String> {
    let width = whole.width().expect("a whole number");
    if data.len() != width {
        return Err(format!(
            "a {} is {width} bytes and {} arrived",
            whole.name(),
            data.len()
        ));
    }
    let mut bytes = [0u8; 8];
    bytes[8 - width..].copy_from_slice(data);
    let raw = u64::from_be_bytes(bytes);
    if !matches!(whole, Basic::I8 | Basic::I16 | Basic::I32 | Basic::I64) {
        return Ok((raw, false));
    }
    // Sign-extended into the 64 bits: shifted to the top and back.
    let shift = 64 - 8 * width as u32;
    let number = ((raw << shift) as i64) >> shift;
    Ok((number.unsigned_abs(), number < 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::Unit;

    /// Writes a datum's text and reads it back.
    ///
    /// # Arguments
    ///
    /// * `said` - The type as a configuration writes it.
    /// * `data` - The datum.
    ///
    /// # Returns
    ///
    /// The text, and the datum read back from it.
    fn round(said: &str, data: &[u8]) -> (String, Vec<u8>) {
        let kind = LinkType::of(said).expect(said);
        let mut text = String::new();
        write_text(&kind, data, &mut text).expect("written");
        let mut out = [0u8; 64];
        let n = read_text(&kind, &text, "test", &mut out).expect("read");
        (text, out[..n].to_vec())
    }

    /// Every type with a text form writes it and reads it back the same.
    #[test]
    fn each_type_writes_and_reads_its_text() {
        assert_eq!(round("u8", &[200]), ("200".to_string(), vec![200]));
        assert_eq!(
            round("i16", &[0xff, 0xfe]),
            ("-2".to_string(), vec![0xff, 0xfe])
        );
        assert_eq!(round("i8", &[0x80]), ("-128".to_string(), vec![0x80]));
        assert_eq!(
            round("i64", &(-5i64).to_be_bytes()),
            ("-5".to_string(), (-5i64).to_be_bytes().to_vec())
        );
        assert_eq!(round("boolean", &[1]), ("1".to_string(), vec![1]));
        let kelvin = value_bytes(293.15, Unit::Kelvin)
            .expect("a temperature")
            .to_vec();
        assert_eq!(round("kelvin", &kelvin), ("293.15".to_string(), kelvin));
        let (text, back) = round("x: i16, on: boolean", &[0, 3, 1]);
        assert_eq!((text.as_str(), back), ("x=3 on=1", vec![0, 3, 1]));
    }

    /// Text that is not the type is refused, naming who wrote it.
    #[test]
    fn text_that_is_not_the_type_is_refused() {
        let mut out = [0u8; 64];
        for (said, text) in [
            ("u8", "256"),
            ("u8", "-1"),
            ("i8", "200"),
            ("boolean", "2"),
            ("u16", "many"),
            ("x: i16, y: i16", "x=1"),
            ("x: i16, y: i16", "x=1 x=2 y=3"),
            ("x: i16, y: i16", "x=1 y=2 z=3"),
        ] {
            let kind = LinkType::of(said).expect(said);
            let why = read_text(&kind, text, "the program", &mut out)
                .expect_err(text);
            assert!(why.contains("the program"), "{said} {text}: {why}");
        }
        assert!(!has_text(&LinkType::of("string").expect("a type")));
        assert!(has_text(&LinkType::of("x: u8").expect("a type")));
    }
}
