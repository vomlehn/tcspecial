//! The bytes that make a triggered payload answer, as a file writes them.
//!
//! A trigger is bytes, not text: a payload's interface document asks for what
//! it asks for, and that is as likely to be `55` as it is to be `"READ\r"`.
//! So a file may write it either way -- one byte in hexadecimal, or a string
//! in C's notation -- and both arrive here as the bytes to send.
//!
//! Which notation a value is in is decided by its first character and nothing
//! else: a quote opens a C string, and anything else is one byte in
//! hexadecimal. Guessing would be worse: `52` is a plausible byte and a
//! plausible pair of digits of something longer, and a file that meant one and
//! got the other would send something nobody asked for. The quote is the mark,
//! so it is part of the value and not of the way the file happens to be
//! written -- in YAML a C string is `trigger: '"READ\r"'`, and in XML
//! `<trigger>"READ\r"</trigger>`.

/// A node that is asked for its data, and what asks it.
///
/// A node with a trigger is a node that is asked: tcspecial writes these bytes
/// to it and reads what comes back. There is no separate word for the mode,
/// because a trigger and nothing else is what makes a payload triggered and an
/// extra word could only disagree with it.
///
/// A trait, because what sends the request has no business knowing which kind
/// of node it is sending to: a generated configuration's i2c node and its
/// serial node are two types and one question. Which is why it is declared
/// here rather than in the generated file -- the code that does the asking is
/// not generated, and it has to be able to name what it is asking.
///
/// Bytes and not text: [`trigger_bytes`] is where a file's `55` or
/// `"READ\r"` becomes what this answers with.
pub trait Triggered {
    /// What to write to make it answer, or `None` for a node that is only
    /// read.
    ///
    /// An `Option` because being triggered is a property of the node and not
    /// of its kind: every i2c node may be asked, and one is asked only if
    /// somebody wrote down what to ask it.
    ///
    /// Named for the bytes rather than for the trigger, because a node that
    /// holds its trigger in a field called `trigger` -- which is what the
    /// configuration calls it -- would have that field shadow a method of the
    /// same name, and the method would be unreachable through a value.
    fn trigger_bytes(&self) -> Option<&[u8]>;
}

/// The bytes a file's trigger names, or what is wrong with it.
///
/// Two notations, and the first character says which: a trigger that opens
/// with a quote is text in C's notation, escapes and all, and anything else is
/// one byte in hexadecimal. So `"READ\r"` is five bytes and `55` is one, and
/// nothing has to be guessed from the length of what was written.
///
/// Empty is refused: a payload that answers requests has to be asked
/// something, and nought bytes asks nothing. The error is the sentence shown
/// to whoever wrote the file, so it says which notation was being read.
pub fn trigger_bytes(written: &str) -> Result<Vec<u8>, String> {
    bytes_of(&TRIGGER, written)
}

/// Which attribute a run of bytes is written in, for what is said about it.
///
/// A trigger and an init are written the same way -- see [`bytes_of`] -- and
/// a mistake in either is told in the words of the one it was made in.
pub struct Named {
    /// The attribute: `trigger`.
    pub word: &'static str,
    /// And with its article: `a trigger`.
    pub a: &'static str,
    /// Why nought bytes is refused.
    pub empty: &'static str,
}

/// A trigger: the bytes that make a payload answer.
pub const TRIGGER: Named = Named {
    word: "trigger",
    a: "a trigger",
    empty: "a payload that answers requests has to be asked something",
};

/// An init: the bytes a payload is set to as the vehicle opens it.
pub const INIT: Named = Named {
    word: "init",
    a: "an init",
    empty: "an init sets a payload to something, and one that sets nothing is \
            left out",
};

/// The bytes a file wrote in the attribute `named`, or what is wrong with
/// them: see [`trigger_bytes`] for the two notations.
pub fn bytes_of(named: &Named, written: &str) -> Result<Vec<u8>, String> {
    let Named { word, a, empty } = named;
    let written = written.trim();

    let bytes = match written.strip_prefix('"') {
        Some(rest) => {
            let text = rest.strip_suffix('"').ok_or_else(|| {
                format!(
                    "the {word} {written:?} opens with a quote and does not close: \
                     {a} that is text is a C string, quotes and all"
                )
            })?;
            c_string_bytes(named, text)?
        }
        None => one_byte(named, written)?,
    };

    if bytes.is_empty() {
        return Err(format!(
            "the {word} {written:?} is no bytes at all: {empty}"
        ));
    }

    Ok(bytes)
}

/// One byte, as one or two hexadecimal digits.
///
/// Hexadecimal because that is how an interface document writes what it wants,
/// and one byte because a trigger of several is text: `"\x55\xAA"` says two
/// bytes and says which notation it is in, where `55AA` was a value whose
/// length decided how many bytes it meant. One digit is the same byte as two,
/// a datasheet writing `5` where it means `05`.
///
/// No `x` anywhere, `0x` included. A trigger written without quotes is
/// hexadecimal and nothing else, so a prefix saying so would be saying it for
/// a notation that has no alternative -- and a file that writes one is told
/// what to write instead rather than having `0x55` read as something.
fn one_byte(named: &Named, written: &str) -> Result<Vec<u8>, String> {
    let Named { word, a, empty } = named;
    // The spellings this replaced, each caught by name: a file written for an
    // older rule is told what to say rather than being refused for a
    // character it was right to write.
    if written.starts_with("0x") || written.starts_with("0X") {
        return Err(format!(
            "the {word} {written:?} is written after 0x, and {a} is no longer: \
             a byte stands on its own, as in 55"
        ));
    }
    if written.split_whitespace().count() > 1 {
        return Err(format!(
            "the {word} {written:?} is more than one value: {a} is one byte in \
             hexadecimal, as in 55, or text as a C string -- {a} of several \
             bytes is written \"\\x55\\xAA\""
        ));
    }

    if let Some(bad) = written.chars().find(|c| !c.is_ascii_hexdigit()) {
        return Err(format!(
            "{bad:?} in {written:?} is not a hexadecimal digit: {a} is one byte \
             in 0-9, a-f or A-F -- or a C string in quotes"
        ));
    }

    match written.len() {
        // One digit is the same byte as two: 5 and 05 are one byte.
        1 | 2 => Ok(vec![
            u8::from_str_radix(written, 16).expect("checked digits")
        ]),
        0 => Err(format!("{a} names a byte: {empty}")),
        length => Err(format!(
            "{written:?} is {length} digits, which is more than one byte: {a} of \
             several bytes is text, as in \"\\x55\\xAA\""
        )),
    }
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
fn c_string_bytes(named: &Named, written: &str) -> Result<Vec<u8>, String> {
    let Named { a, .. } = named;
    let mut bytes = Vec::with_capacity(written.len());
    let mut chars = written.chars();

    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut utf8 = [0u8; 4];
            bytes.extend_from_slice(c.encode_utf8(&mut utf8).as_bytes());
            continue;
        }

        let escaped = chars.next().ok_or_else(|| {
            format!(
                "{a} ends in a backslash, which escapes nothing: write \\\\ for a \
                 backslash"
            )
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
                    "\\{other} is not an escape: {a} takes C's own -- \\\\ \\\" \
                     \\' \\n \\r \\t \\0 \\a \\b \\f \\v and \\xNN -- and a list of \
                     byte values is written without quotes instead"
                ))
            }
        }
    }

    Ok(bytes)
}

/// Bytes as a file could write them.
///
/// The notation a file would use for these bytes, so that what is shown can be
/// copied back into one: a single byte as its two hexadecimal digits, and
/// anything longer as the C string it would have to be written as -- there
/// being one notation for more than one byte. See [`trigger_bytes`], which
/// reads what this writes.
///
/// A printable byte is shown as itself inside the string, which is what makes
/// a trigger from an interface document readable: `"READ\r"` rather than four
/// escapes and a fifth. The rest are `\xNN`, and the two characters a C string
/// cannot hold plainly -- the quote and the backslash -- are escaped as C
/// escapes them.
pub fn trigger_as_written(bytes: &[u8]) -> String {
    if let [byte] = bytes {
        return format!("{byte:02X}");
    }

    let mut shown = String::from("\"");
    for byte in bytes {
        match byte {
            b'"' => shown.push_str("\\\""),
            b'\\' => shown.push_str("\\\\"),
            b'\r' => shown.push_str("\\r"),
            b'\n' => shown.push_str("\\n"),
            b'\t' => shown.push_str("\\t"),
            // Printable ASCII, which is what a word of a trigger is.
            0x20..=0x7e => shown.push(*byte as char),
            other => shown.push_str(&format!("\\x{other:02X}")),
        }
    }
    shown.push('"');
    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One byte, in either case, one or two digits.
    #[test]
    fn a_byte_is_read_as_the_byte_it_is() {
        assert_eq!(trigger_bytes("55").unwrap(), vec![0x55]);
        assert_eq!(trigger_bytes("aa").unwrap(), vec![0xAA]);
        assert_eq!(trigger_bytes("Aa").unwrap(), vec![0xAA]);

        // One digit is the same byte as two: a datasheet writes 5 where it
        // means 05, and counting zeroes is not what a configuration is for.
        assert_eq!(trigger_bytes("5").unwrap(), vec![0x05]);
        assert_eq!(trigger_bytes("05").unwrap(), trigger_bytes("5").unwrap());

        // The byte a payload most often wants, and whatever space somebody
        // left around it.
        assert_eq!(trigger_bytes("0D").unwrap(), vec![0x0D]);
        assert_eq!(trigger_bytes("  0d  ").unwrap(), vec![0x0D]);
    }

    /// More than one byte is text, and a file that writes a longer value is
    /// told so.
    ///
    /// The notation was a list whose values meant one byte or two by their
    /// length, which made `55AA` two bytes and `55 AA` the same two written
    /// another way. A trigger of several bytes is a C string now -- one
    /// notation for "more than one byte", and the quote at the front says
    /// which notation is being read before anything is counted.
    #[test]
    fn more_than_one_byte_is_a_c_string() {
        assert_eq!(trigger_bytes("\"\\x55\\xAA\"").unwrap(), vec![0x55, 0xAA]);

        for written in ["55 AA", "55AA", "0400", "03 0400", "5 a f"] {
            let said = trigger_bytes(written).expect_err("not one byte");
            assert!(
                said.contains("one byte") || said.contains("more than one"),
                "{written}: {said}"
            );
        }
    }

    /// What is not a byte is said not to be, rather than read as something
    /// else.
    #[test]
    fn a_value_that_is_not_a_byte_is_refused() {
        // A word, which is what a C string looks like without its quotes:
        // the quotes are the mark, so this is not one.
        let said = trigger_bytes("READ").expect_err("a word is not hexadecimal");
        assert!(said.contains("hexadecimal digit"), "{said}");

        // And a digit string with something in it that is not a digit.
        let said = trigger_bytes("5g").expect_err("g is not a digit");
        assert!(said.contains("hexadecimal digit"), "{said}");
    }

    /// The spelling this replaced is named rather than puzzled over.
    #[test]
    fn the_older_spelling_is_named_rather_than_puzzled_over() {
        for written in ["0x55", "0X55"] {
            let said = trigger_bytes(written).expect_err("0x is no longer written");
            assert!(said.contains("0x"), "{said}");
            assert!(said.contains("stands on its own"), "{said}");
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
                said.contains("no bytes") || said.contains("names a byte"),
                "{written:?}: {said}"
            );
        }
    }

    /// The two notations can write the same trigger, and do.
    ///
    /// One byte is where they meet: beyond that there is one notation, a C
    /// string, and the hexadecimal form is the byte on its own.
    #[test]
    fn the_two_notations_meet() {
        assert_eq!(
            trigger_bytes("0D").unwrap(),
            trigger_bytes("\"\\r\"").unwrap()
        );
        assert_eq!(
            trigger_bytes("41").unwrap(),
            trigger_bytes("\"A\"").unwrap()
        );
    }

    /// Bytes read back as a file could have written them.
    #[test]
    fn bytes_are_shown_as_a_file_could_write_them() {
        // One byte is the hexadecimal form, which is what a file writes for
        // one byte.
        assert_eq!(trigger_as_written(&[0x55]), "55");
        assert_eq!(trigger_as_written(&[0x00]), "00");

        // More than one is the notation that can hold more than one, with a
        // word shown as a word: a trigger from an interface document is
        // commonly one.
        assert_eq!(trigger_as_written(b"READ"), "\"READ\"");
        assert_eq!(trigger_as_written(b"READ\r"), "\"READ\\r\"");
        assert_eq!(trigger_as_written(&[0x55, 0xAA]), "\"U\\xAA\"");
        assert_eq!(trigger_as_written(b"a\"b"), "\"a\\\"b\"");

        // What is shown is read back as the bytes it was shown for, which is
        // what makes it a spelling rather than a decoration.
        for bytes in [
            &[0x55u8, 0xAA][..],
            b"READ\r",
            &[0x00],
            &[0x55],
            b"a\"b",
            b"a\\b",
            &[0x00, 0x09, 0x0a],
        ] {
            let shown = trigger_as_written(bytes);
            assert_eq!(
                trigger_bytes(&shown).unwrap(),
                bytes.to_vec(),
                "{shown} should read back as what it was shown for"
            );
        }
    }
}
