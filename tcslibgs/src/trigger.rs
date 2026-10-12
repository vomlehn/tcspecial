//! The bytes that make a triggered payload answer, as a file writes them.
//!
//! A trigger is bytes, not text: a payload's interface document asks for what
//! it asks for, and that is as likely to be `55 AA` as it is to be `"READ\r"`.
//! So a file may write it either way -- a list of values, or a string in C's
//! notation -- and both arrive here as the bytes to send.
//!
//! Which notation a value is in is decided by its first character and nothing
//! else: a quote opens a C string, and anything else is a list of values.
//! Guessing would be worse: `52 45` is a plausible pair of bytes and a
//! plausible pair of digits, and a file that meant one and got the other would
//! send something nobody asked for. The quote is the mark, so it is part of the
//! value and not of the way the file happens to be written -- in YAML a C
//! string is `trigger: '"READ\r"'`, and in XML
//! `<trigger>"READ\r"</trigger>`.

/// The bytes a file's trigger names, or what is wrong with it.
///
/// Empty is refused: a payload that answers requests has to be asked
/// something, and nought bytes asks nothing. The error is the sentence shown
/// to whoever wrote the file, so it says which notation was being read.
pub fn trigger_bytes(written: &str) -> Result<Vec<u8>, String> {
    let written = written.trim();

    let bytes = match written.strip_prefix('"') {
        Some(rest) => {
            let text = rest.strip_suffix('"').ok_or_else(|| {
                format!(
                    "the trigger {written:?} opens with a quote and does not close: a \
                     trigger that is text is a C string, quotes and all"
                )
            })?;
            c_string_bytes(text)?
        }
        None => value_bytes(written)?,
    };

    if bytes.is_empty() {
        return Err(format!(
            "the trigger {written:?} is no bytes at all: a payload that answers \
             requests has to be asked something"
        ));
    }

    Ok(bytes)
}

/// A list of values, separated by spaces.
///
/// Each value is hexadecimal -- that being how an interface document writes
/// what it wants -- and is one byte where it has one or two digits, or two
/// bytes where it has three or four. A two-byte value goes out most
/// significant byte first, which is the order it is written in and the order
/// every other multi-byte quantity in this project uses.
///
/// No `0x`, on any of them. A trigger is a list of byte values and nothing
/// else, so a prefix saying "this is hexadecimal" would be saying it once per
/// value for a notation that has no alternative -- and a file that writes one
/// is told so rather than having `0x55` read as something.
fn value_bytes(written: &str) -> Result<Vec<u8>, String> {
    // The spelling this replaced, caught by name: a file written for the
    // older rule says 0x and would otherwise be refused for the x alone,
    // which says nothing about what to do instead.
    if written.starts_with("0x") || written.starts_with("0X") {
        return Err(format!(
            "the trigger {written:?} is written after 0x, and a trigger is no longer: \
             the values stand on their own, as in 55 AA 0F"
        ));
    }

    let mut bytes = Vec::new();

    for value in written.split_whitespace() {
        if let Some(bad) = value.chars().find(|c| !c.is_ascii_hexdigit()) {
            return Err(format!(
                "{bad:?} in {value:?} is not a hexadecimal digit: a trigger is a list \
                 of values in 0-9, a-f or A-F, separated by spaces -- or a C string \
                 in quotes"
            ));
        }

        match value.len() {
            // One byte, however it was padded: 5 and 05 are the same byte.
            1 | 2 => bytes.push(u8::from_str_radix(value, 16).expect("checked digits")),
            // Two bytes, most significant first.
            3 | 4 => {
                let whole = u16::from_str_radix(value, 16).expect("checked digits");
                bytes.extend_from_slice(&whole.to_be_bytes());
            }
            length => {
                return Err(format!(
                    "{value:?} is {length} digits, which is neither one byte nor two: \
                     a value is one or two digits for a byte, or three or four for two \
                     bytes. Separate them with spaces"
                ))
            }
        }
    }

    if bytes.is_empty() {
        return Err(
            "a trigger names at least one value: a payload that answers requests has \
             to be asked something"
                .to_string(),
        );
    }

    Ok(bytes)
}

/// A string in C's notation: its characters, with C's escapes.
///
/// The escapes are C's own, including `\xNN`, which is how a value that is
/// mostly text says the one byte that is not -- `"READ\r"` and
/// `"READ\x0D"` are the same trigger. An escape C does not have is refused
/// rather than passed through: a file that wrote `\q` meant something, and
/// sending a backslash and a q is unlikely to be it.
///
/// What the characters themselves become is their UTF-8, so a trigger of
/// plain text is the bytes that text is.
fn c_string_bytes(written: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::with_capacity(written.len());
    let mut chars = written.chars();

    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut utf8 = [0u8; 4];
            bytes.extend_from_slice(c.encode_utf8(&mut utf8).as_bytes());
            continue;
        }

        let escaped = chars.next().ok_or_else(|| {
            "a trigger ends in a backslash, which escapes nothing: write \\\\ for a \
             backslash"
                .to_string()
        })?;

        match escaped {
            '\\' => bytes.push(b'\\'),
            '"' => bytes.push(b'"'),
            '\'' => bytes.push(b'\''),
            '?' => bytes.push(b'?'),
            'n' => bytes.push(b'\n'),
            'r' => bytes.push(b'\r'),
            't' => bytes.push(b'\t'),
            '0' => bytes.push(0),
            'a' => bytes.push(0x07),
            'b' => bytes.push(0x08),
            'f' => bytes.push(0x0C),
            'v' => bytes.push(0x0B),
            'x' => {
                // Exactly two digits, so that the byte after an escape is
                // read as itself: `\x0D0A` is a carriage return then "0A",
                // which is what C's own two-digit habit makes of it here
                // rather than C's greedy run.
                let high = chars.next();
                let low = chars.next();
                let pair: String = [high, low].iter().flatten().collect();
                if pair.len() != 2 || !pair.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err(format!(
                        "\\x{pair} is not two hexadecimal digits: \\xNN is one byte, \
                         and a run of them is written \\x0D\\x0A"
                    ));
                }
                bytes.push(u8::from_str_radix(&pair, 16).expect("two checked digits"));
            }
            other => {
                return Err(format!(
                    "\\{other} is not an escape: a trigger takes C's own -- \\\\ \\\" \
                     \\' \\n \\r \\t \\0 \\a \\b \\f \\v and \\xNN -- and a list of \
                     byte values is written without quotes instead"
                ))
            }
        }
    }

    Ok(bytes)
}

/// Bytes as a file could have written them, for a window or a log to show.
///
/// Hexadecimal, because that is the notation every trigger can be written in:
/// text that is mostly printable is readable enough beside it, and a trigger
/// that is not text at all has nothing else to be shown as.
pub fn trigger_as_written(bytes: &[u8]) -> String {
    let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02X}")).collect();
    let mut shown = hex.join(" ");

    // And the text beside it where the bytes are text, since a trigger from
    // an interface document is commonly a word.
    if let Ok(text) = std::str::from_utf8(bytes) {
        if text.chars().all(|c| !c.is_control()) {
            shown.push_str(&format!(" ({text:?})"));
        }
    }

    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A list of values, in either case, one or two digits to a byte.
    #[test]
    fn a_list_of_values_is_read_as_bytes() {
        assert_eq!(trigger_bytes("55 AA 0F").unwrap(), vec![0x55, 0xAA, 0x0F]);
        assert_eq!(trigger_bytes("55 aa 0f").unwrap(), vec![0x55, 0xAA, 0x0F]);
        assert_eq!(trigger_bytes("55 Aa 0F").unwrap(), vec![0x55, 0xAA, 0x0F]);

        // One digit is the same byte as two: a datasheet writes 5 where it
        // means 05, and counting zeroes is not what a configuration is for.
        assert_eq!(trigger_bytes("5 a f").unwrap(), vec![0x05, 0x0A, 0x0F]);
        assert_eq!(trigger_bytes("05 0a 0f").unwrap(), trigger_bytes("5 a f").unwrap());

        // One value, and the byte a payload most often wants.
        assert_eq!(trigger_bytes("0D").unwrap(), vec![0x0D]);

        // However much space somebody left between them.
        assert_eq!(trigger_bytes("  55   AA  ").unwrap(), vec![0x55, 0xAA]);
    }

    /// A value of three or four digits is two bytes, most significant first.
    ///
    /// Which is the order it is written in, and the order every other
    /// multi-byte quantity in this project uses: a register address copied
    /// out of a datasheet as 0400 goes out as 04 then 00.
    #[test]
    fn a_longer_value_is_two_bytes_in_the_order_it_is_written() {
        assert_eq!(trigger_bytes("0400").unwrap(), vec![0x04, 0x00]);
        assert_eq!(trigger_bytes("400").unwrap(), vec![0x04, 0x00]);
        assert_eq!(trigger_bytes("55AA").unwrap(), vec![0x55, 0xAA]);
        assert_eq!(trigger_bytes("55AA").unwrap(), trigger_bytes("55 AA").unwrap());

        // And the two sizes in one trigger, which is what an interface
        // document that asks for a command and a register looks like.
        assert_eq!(trigger_bytes("03 0400").unwrap(), vec![0x03, 0x04, 0x00]);
    }

    /// What is not a list of values is said not to be, rather than read as
    /// something else.
    #[test]
    fn a_value_that_is_not_one_is_refused() {
        // A word, which is what a C string looks like without its quotes:
        // the quotes are the mark, so this is not one.
        let said = trigger_bytes("READ").unwrap_err();
        assert!(
            said.contains("hexadecimal digit") && said.contains("C string in quotes"),
            "{said}"
        );

        let said = trigger_bytes("55 GG").unwrap_err();
        assert!(said.contains("hexadecimal digit"), "{said}");

        // Five digits is neither one byte nor two, and which of the five was
        // meant to pair with which is not something to decide for a file.
        let said = trigger_bytes("55AA0F").unwrap_err();
        assert!(
            said.contains("neither one byte nor two") && said.contains("spaces"),
            "{said}"
        );
    }

    /// The spelling this replaced is caught by name.
    ///
    /// A file written for the older rule says 0x, and would otherwise be
    /// refused for the x alone -- which says nothing about what to do
    /// instead.
    #[test]
    fn the_older_spelling_is_named_rather_than_puzzled_over() {
        for written in ["0x55AA", "0X55 AA", "0x0D"] {
            let said = trigger_bytes(written).unwrap_err();
            assert!(
                said.contains("no longer") && said.contains("55 AA 0F"),
                "{written:?}: {said}"
            );
        }
    }

    /// A C string is its characters, with C's escapes, inside its quotes.
    #[test]
    fn a_c_string_is_read_as_c_reads_it() {
        assert_eq!(trigger_bytes("\"READ\"").unwrap(), b"READ".to_vec());
        assert_eq!(trigger_bytes("\"READ\\r\"").unwrap(), b"READ\r".to_vec());
        assert_eq!(trigger_bytes("\"\\r\\n\"").unwrap(), vec![0x0D, 0x0A]);
        assert_eq!(
            trigger_bytes("\"\\t\\0\\a\\b\\f\\v\"").unwrap(),
            vec![9, 0, 7, 8, 12, 11]
        );
        assert_eq!(trigger_bytes("\"a\\\\b\"").unwrap(), b"a\\b".to_vec());
        assert_eq!(trigger_bytes("\"\\\"\"").unwrap(), b"\"".to_vec());

        // \xNN is how a value that is mostly text says the byte that is not,
        // and is the same trigger as the escape it stands for.
        assert_eq!(
            trigger_bytes("\"READ\\x0D\"").unwrap(),
            trigger_bytes("\"READ\\r\"").unwrap()
        );

        // Text that is not ASCII is the bytes that text is.
        assert_eq!(trigger_bytes("\"\u{00B5}\"").unwrap(), vec![0xC2, 0xB5]);

        // A string that opens and does not close, which is the mistake the
        // quotes being the mark makes possible.
        let said = trigger_bytes("\"READ").unwrap_err();
        assert!(said.contains("does not close"), "{said}");
    }

    /// An escape C does not have is refused rather than passed through.
    #[test]
    fn an_escape_c_does_not_have_is_refused() {
        let said = trigger_bytes("\"\\q\"").unwrap_err();
        assert!(
            said.contains("not an escape") && said.contains("without quotes"),
            "{said}"
        );

        let said = trigger_bytes("\"READ\\\"").unwrap_err();
        assert!(
            said.contains("escapes nothing") || said.contains("does not close"),
            "{said}"
        );

        let said = trigger_bytes("\"\\xZZ\"").unwrap_err();
        assert!(said.contains("two hexadecimal digits"), "{said}");

        // One digit is not a byte, and the next character is not a digit to
        // borrow from.
        let said = trigger_bytes("\"\\x0\"").unwrap_err();
        assert!(said.contains("two hexadecimal digits"), "{said}");
    }

    /// Nothing is not a trigger.
    #[test]
    fn a_trigger_of_no_bytes_is_refused() {
        for written in ["", "   ", "\"\""] {
            let said = trigger_bytes(written).unwrap_err();
            assert!(
                said.contains("no bytes") || said.contains("at least one value"),
                "{written:?}: {said}"
            );
        }
    }

    /// The two notations can write the same trigger, and do.
    #[test]
    fn the_two_notations_meet() {
        assert_eq!(
            trigger_bytes("52 45 41 44 0D").unwrap(),
            trigger_bytes("\"READ\\r\"").unwrap()
        );
    }

    /// Bytes read back as a file could have written them.
    #[test]
    fn bytes_are_shown_as_a_file_could_write_them() {
        assert_eq!(trigger_as_written(&[0x55, 0xAA]), "55 AA");
        // Text beside the bytes, where the bytes are text: a trigger from an
        // interface document is commonly a word.
        assert_eq!(trigger_as_written(b"READ"), "52 45 41 44 (\"READ\")");
        // And no text where a byte is not printable, the quoted form of a
        // control character being less readable than the hex already shown.
        assert_eq!(trigger_as_written(b"READ\r"), "52 45 41 44 0D");

        // What is shown is read back as the bytes it was shown for, which is
        // what makes it a spelling rather than a decoration.
        for bytes in [&[0x55u8, 0xAA][..], b"READ\r", &[0x00]] {
            let shown = trigger_as_written(bytes);
            let hex = shown.split(" (").next().expect("the bytes");
            assert_eq!(trigger_bytes(hex).unwrap(), bytes.to_vec(), "{shown}");
        }
    }
}
