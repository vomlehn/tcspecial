//! The bytes that make a triggered payload answer, as a file writes them.
//!
//! A trigger is bytes, not text: a payload's interface document asks for what
//! it asks for, and that is as likely to be `0x55 0xAA` as it is to be
//! `READ\r`. So a file may write it either way -- a run of hexadecimal after
//! `0x`, or a string in C's notation -- and both arrive here as the bytes to
//! send.
//!
//! Which notation a value is in is decided by its first two characters and
//! nothing else. Guessing would be worse: a value of `52 45` is a plausible
//! pair of bytes and a plausible pair of digits, and a file that meant one
//! and got the other would send something nobody asked for.

/// The bytes a file's trigger names, or what is wrong with it.
///
/// Empty is refused: a payload that answers requests has to be asked
/// something, and nought bytes asks nothing. The error is the sentence shown
/// to whoever wrote the file, so it says which notation was being read.
pub fn trigger_bytes(written: &str) -> Result<Vec<u8>, String> {
    let bytes = match written.strip_prefix("0x").or_else(|| written.strip_prefix("0X")) {
        Some(hex) => hex_bytes(hex)?,
        None => c_string_bytes(written)?,
    };

    if bytes.is_empty() {
        return Err(format!(
            "the trigger {written:?} is no bytes at all: a payload that answers \
             requests has to be asked something"
        ));
    }

    Ok(bytes)
}

/// A run of hexadecimal, two digits to a byte.
///
/// Spaces and underscores are allowed between bytes and ignored, because a
/// trigger of any length is read by someone checking it against an interface
/// document: `0x55 AA 0F` is the same bytes as `0x55AA0F` and easier to
/// compare. Upper and lower case alike, since a datasheet uses both.
fn hex_bytes(hex: &str) -> Result<Vec<u8>, String> {
    let digits: String = hex.chars().filter(|c| !matches!(c, ' ' | '_')).collect();

    if digits.is_empty() {
        return Err("0x with no digits after it names no bytes".to_string());
    }
    if let Some(bad) = digits.chars().find(|c| !c.is_ascii_hexdigit()) {
        return Err(format!(
            "{bad:?} is not a hexadecimal digit: after 0x a trigger is 0-9, a-f or \
             A-F, two digits to a byte, with spaces or underscores allowed between \
             them"
        ));
    }
    // An odd number of digits is half a byte, and which half was meant --
    // the high or the low -- is not something to decide for a file.
    if !digits.len().is_multiple_of(2) {
        return Err(format!(
            "0x{digits} is {} digits, which is not a whole number of bytes: two \
             digits to a byte",
            digits.len()
        ));
    }

    Ok(digits
        .as_bytes()
        .chunks(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("hex digits are ASCII");
            u8::from_str_radix(text, 16).expect("two checked hex digits")
        })
        .collect())
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
                     \\' \\n \\r \\t \\0 \\a \\b \\f \\v and \\xNN -- and a run of \
                     bytes is written after 0x instead"
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
    let mut shown = format!("0x{}", hex.join(" "));

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

    /// Hexadecimal, in either case, with or without the spaces that make a
    /// long one readable.
    #[test]
    fn hexadecimal_is_read_two_digits_to_a_byte() {
        assert_eq!(trigger_bytes("0x55AA0F").unwrap(), vec![0x55, 0xAA, 0x0F]);
        assert_eq!(trigger_bytes("0x55aa0f").unwrap(), vec![0x55, 0xAA, 0x0F]);
        assert_eq!(trigger_bytes("0X55Aa0F").unwrap(), vec![0x55, 0xAA, 0x0F]);

        // Spaces and underscores between bytes, so a trigger can be compared
        // against an interface document without counting digits.
        assert_eq!(trigger_bytes("0x55 AA 0F").unwrap(), vec![0x55, 0xAA, 0x0F]);
        assert_eq!(trigger_bytes("0x55_AA_0F").unwrap(), vec![0x55, 0xAA, 0x0F]);

        // One byte, and the byte a payload most often wants.
        assert_eq!(trigger_bytes("0x0D").unwrap(), vec![0x0D]);
    }

    /// What is not hexadecimal after 0x is said to be, rather than read as
    /// something else.
    #[test]
    fn hexadecimal_that_is_not_is_refused() {
        // Half a byte: which half was meant is not something to decide for a
        // file.
        let said = trigger_bytes("0x55A").unwrap_err();
        assert!(said.contains("whole number of bytes"), "{said}");

        let said = trigger_bytes("0x55GG").unwrap_err();
        assert!(said.contains("hexadecimal digit"), "{said}");

        let said = trigger_bytes("0x").unwrap_err();
        assert!(said.contains("no digits"), "{said}");
    }

    /// A C string is its characters, with C's escapes.
    #[test]
    fn a_c_string_is_read_as_c_reads_it() {
        assert_eq!(trigger_bytes("READ").unwrap(), b"READ".to_vec());
        assert_eq!(trigger_bytes("READ\\r").unwrap(), b"READ\r".to_vec());
        assert_eq!(trigger_bytes("\\r\\n").unwrap(), vec![0x0D, 0x0A]);
        assert_eq!(trigger_bytes("\\t\\0\\a\\b\\f\\v").unwrap(), vec![9, 0, 7, 8, 12, 11]);
        assert_eq!(trigger_bytes("a\\\\b").unwrap(), b"a\\b".to_vec());
        assert_eq!(trigger_bytes("\\\"").unwrap(), b"\"".to_vec());

        // \xNN is how a value that is mostly text says the byte that is not,
        // and is the same trigger as the escape it stands for.
        assert_eq!(
            trigger_bytes("READ\\x0D").unwrap(),
            trigger_bytes("READ\\r").unwrap()
        );

        // Text that is not ASCII is the bytes that text is.
        assert_eq!(trigger_bytes("\u{00B5}").unwrap(), vec![0xC2, 0xB5]);
    }

    /// An escape C does not have is refused rather than passed through.
    #[test]
    fn an_escape_c_does_not_have_is_refused() {
        let said = trigger_bytes("\\q").unwrap_err();
        assert!(said.contains("not an escape") && said.contains("0x"), "{said}");

        let said = trigger_bytes("READ\\").unwrap_err();
        assert!(said.contains("escapes nothing"), "{said}");

        let said = trigger_bytes("\\xZZ").unwrap_err();
        assert!(said.contains("two hexadecimal digits"), "{said}");

        // One digit is not a byte, and the next character is not a digit to
        // borrow from.
        let said = trigger_bytes("\\x0").unwrap_err();
        assert!(said.contains("two hexadecimal digits"), "{said}");
    }

    /// Nothing is not a trigger.
    #[test]
    fn a_trigger_of_no_bytes_is_refused() {
        for written in ["", "0x_", "0x  "] {
            let said = trigger_bytes(written).unwrap_err();
            assert!(
                said.contains("no bytes") || said.contains("no digits"),
                "{written:?}: {said}"
            );
        }
    }

    /// The two notations can write the same trigger, and do.
    #[test]
    fn the_two_notations_meet() {
        assert_eq!(
            trigger_bytes("0x52 45 41 44 0D").unwrap(),
            trigger_bytes("READ\\r").unwrap()
        );
    }

    /// Bytes read back as a file could have written them.
    #[test]
    fn bytes_are_shown_as_a_file_could_write_them() {
        assert_eq!(trigger_as_written(&[0x55, 0xAA]), "0x55 AA");
        // Text beside the bytes, where the bytes are text: a trigger from an
        // interface document is commonly a word.
        assert_eq!(trigger_as_written(b"READ"), "0x52 45 41 44 (\"READ\")");
        // And no text where a byte is not printable, the quoted form of a
        // control character being less readable than the hex already shown.
        assert_eq!(trigger_as_written(b"READ\r"), "0x52 45 41 44 0D");
    }
}
