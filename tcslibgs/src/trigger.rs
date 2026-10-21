//! Bytes as a file writes them: what makes a triggered payload answer, what a
//! payload is set to as it opens, and what ends a packet of a stream.
//!
//! These are bytes, not text: a payload's interface document asks for what it
//! asks for, and that is as likely to be `55` as it is to be `"READ\r"`. So a
//! file writes them as one or more parts, one after another, each in one of
//! two notations -- a string in C's notation, in quotes, or bytes in
//! hexadecimal, in square brackets -- and they arrive here as the bytes the
//! parts make together. `"AT" [0D 0A]` is four bytes.
//!
//! Which notation a part is in is decided by its first character and nothing
//! else: a quote opens a C string and a bracket opens bytes. Guessing would be
//! worse: `52` is a plausible byte and a plausible pair of digits of something
//! longer, and a file that meant one and got the other would send something
//! nobody asked for. The marks are part of the value and not of the way the
//! file happens to be written -- in YAML a trigger is `trigger: '"READ\r"'` or
//! `trigger: '[55]'`, and in XML `<trigger>"READ\r"</trigger>`.

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
/// Parts in two notations, and each part's first character says which: a
/// quote opens text in C's notation, escapes and all, and a square bracket
/// opens bytes in hexadecimal. So `"READ\r"` is five bytes, `[55]` is one and
/// `"AT" [0D]` is three, and nothing has to be guessed from the length of
/// what was written.
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
    /// Whether a byte written bare, before brackets, was decimal unless it
    /// said `0x` -- as a terminator was -- rather than hexadecimal, as a
    /// trigger was. What the old spelling meant decides the brackets it is
    /// told to write: a terminator's `13` is `[0D]`, a trigger's is `[13]`.
    pub bare_was_decimal: bool,
}

/// A trigger: the bytes that make a payload answer.
pub const TRIGGER: Named = Named {
    word: "trigger",
    a: "a trigger",
    empty: "a payload that answers requests has to be asked something",
    bare_was_decimal: false,
};

/// An init: the bytes a payload is set to as the vehicle opens it.
pub const INIT: Named = Named {
    word: "init",
    a: "an init",
    empty: "an init sets a payload to something, and one that sets nothing is \
            left out",
    bare_was_decimal: false,
};

/// A terminator: bytes that end a packet of a stream where they arrive.
pub const TERMINATOR: Named = Named {
    word: "terminator",
    a: "a terminator",
    empty: "a terminator is the bytes that end a packet, and nought bytes would \
            end every packet before it began",
    bare_was_decimal: true,
};

/// The bytes a file wrote in the attribute `named`, or what is wrong with
/// them: see [`trigger_bytes`] for the two notations.
///
/// The parts may stand with space between them or none: `"AT"[0D]` and
/// `"AT" [0D]` are the same three bytes. What stands outside a part is
/// refused, and the spelling this replaced -- one bare byte, `55` or `0x55`
/// -- is refused by name, with the brackets it wants: a file written for the
/// old rule is told what to say rather than read as something else.
pub fn bytes_of(named: &Named, written: &str) -> Result<Vec<u8>, String> {
    let Named { word, a, empty, .. } = named;
    let written = written.trim();
    if let Some(bracketed) = bare_bytes(named, written) {
        return Err(format!(
            "the {word} {written:?} is bytes written bare, and bytes are written \
             in square brackets now: {bracketed}. Text is a C string in quotes"
        ));
    }

    let mut bytes = Vec::new();
    let mut rest = written;
    loop {
        rest = rest.trim_start();
        let Some(first) = rest.chars().next() else {
            break;
        };
        let (part, after) = match first {
            '"' => {
                let (text, after) = c_string_part(rest).ok_or_else(|| {
                    format!(
                        "the {word} {written:?} opens a quote that does not close: \
                         text in {a} is a C string, quotes and all"
                    )
                })?;
                (c_string_bytes(named, text)?, after)
            }
            '[' => {
                let close = rest.find(']').ok_or_else(|| {
                    format!(
                        "the {word} {written:?} opens a bracket that does not close: \
                         bytes in {a} are written [0D 0A], brackets and all"
                    )
                })?;
                (hex_bytes(named, &rest[1..close])?, &rest[close + 1..])
            }
            other => {
                return Err(format!(
                    "{other:?} in the {word} {written:?} begins neither text nor bytes: \
                     {a} is parts one after another, each a C string in quotes, \
                     \"READ\\r\", or bytes in square brackets, [0D 0A]"
                ))
            }
        };
        bytes.extend(part);
        rest = after;
    }

    if bytes.is_empty() {
        return Err(format!(
            "the {word} {written:?} is no bytes at all: {empty}"
        ));
    }

    Ok(bytes)
}

/// Each of the sequences a file wrote, where several are written as one
/// value with commas between them: `"\r\n", [03]`.
///
/// For terminators, of which a stream may have several, any one of which ends
/// a packet. A comma inside a C string or between brackets is part of what it
/// is in, so only a comma between parts separates sequences; each sequence is
/// read by [`bytes_of`], and an empty one -- two commas together, or one at
/// either end -- is refused as no bytes at all.
pub fn sequences_of(named: &Named, written: &str) -> Result<Vec<Vec<u8>>, String> {
    split_sequences(written)
        .into_iter()
        .map(|one| bytes_of(named, one))
        .collect()
}

/// The text of each sequence in `written`, split at the commas that stand
/// between parts and not inside one.
///
/// A quote or a bracket that does not close is left for [`bytes_of`] to
/// refuse in its own words, so this only has to find the commas.
pub fn split_sequences(written: &str) -> Vec<&str> {
    let mut sequences = Vec::new();
    let mut start = 0;
    let mut in_string = false;
    let mut in_brackets = false;
    let mut escaped = false;
    for (at, c) in written.char_indices() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' if !in_brackets => in_string = true,
            '[' => in_brackets = true,
            ']' => in_brackets = false,
            ',' if !in_brackets => {
                sequences.push(&written[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    sequences.push(&written[start..]);
    sequences
}

/// The text of a C string at the front of `rest`, which opens with its
/// quote, and what follows its closing quote: `None` if it does not close.
///
/// The closing quote is the first one no backslash escapes, as in C.
fn c_string_part(rest: &str) -> Option<(&str, &str)> {
    let mut escaped = false;
    for (at, c) in rest.char_indices().skip(1) {
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '"' => return Some((&rest[1..at], &rest[at + 1..])),
            _ => {}
        }
    }
    None
}

/// The bytes between a pair of square brackets: each one or two hexadecimal
/// digits, with space between one and the next.
///
/// Space between them, so that how many bytes a run of digits is never has
/// to be counted: `[0D 0A]` is two bytes, and `[0D0A]` is refused rather than
/// read as two. One digit is the same byte as two, a datasheet writing `5`
/// where it means `05`. No `0x`: everything between the brackets is
/// hexadecimal, so a prefix saying so would be saying it for a notation that
/// has no alternative.
fn hex_bytes(named: &Named, inside: &str) -> Result<Vec<u8>, String> {
    let Named { a, .. } = named;
    let mut bytes = Vec::new();
    for byte in inside.split_whitespace() {
        if byte.starts_with("0x") || byte.starts_with("0X") {
            return Err(format!(
                "[{inside}] has {byte:?} in it, and inside brackets a byte is its \
                 digits alone: [{}]",
                &byte[2..]
            ));
        }
        if let Some(bad) = byte.chars().find(|c| !c.is_ascii_hexdigit()) {
            return Err(format!(
                "{bad:?} in [{inside}] is not a hexadecimal digit: bytes in {a} are \
                 0-9, a-f or A-F, with a space between one and the next"
            ));
        }
        if byte.len() > 2 {
            return Err(format!(
                "{byte:?} in [{inside}] is {} digits, which is more than one byte: \
                 write each byte apart, as in [0D 0A]",
                byte.len()
            ));
        }
        bytes.push(u8::from_str_radix(byte, 16).expect("checked digits"));
    }
    if bytes.is_empty() {
        return Err(format!(
            "[{inside}] names no byte: bytes in {a} are written [0D 0A]"
        ));
    }
    Ok(bytes)
}

/// How bytes written bare in the spelling this replaced are written now, if
/// `written` is that: `55`, `0x55` or `55 AA` becomes `[55]` or `[55 AA]`,
/// and a terminator's decimal `13` becomes `[0D]`.
fn bare_bytes(named: &Named, written: &str) -> Option<String> {
    let mut each = Vec::new();
    for byte in written.split_whitespace() {
        let hex = byte.strip_prefix("0x").or_else(|| byte.strip_prefix("0X"));
        let value = match hex {
            Some(digits) => u8::from_str_radix(digits, 16).ok()?,
            None if named.bare_was_decimal => byte.parse::<u8>().ok()?,
            None => u8::from_str_radix(byte, 16).ok()?,
        };
        each.push(format!("{value:02X}"));
    }
    (!each.is_empty()).then(|| format!("[{}]", each.join(" ")))
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
                     \\' \\n \\r \\t \\0 \\a \\b \\f \\v and \\xNN -- and bytes \
                     that are not text are written in brackets instead, [0D 0A]"
                ))
            }
        }
    }

    Ok(bytes)
}

/// Bytes as a file could write them.
///
/// The notation a file would use for these bytes, so that what is shown can be
/// copied back into one. Text as the C string it is -- which is what makes a
/// trigger from an interface document readable: `"READ\r"` rather than five
/// bytes in hexadecimal -- and anything else as bytes in brackets, `[55 AA]`.
/// Text is printable ASCII and the three escapes a line of it ends in; the
/// two characters a C string cannot hold plainly, the quote and the
/// backslash, are escaped as C escapes them. See [`trigger_bytes`], which
/// reads what this writes.
pub fn trigger_as_written(bytes: &[u8]) -> String {
    let text = bytes
        .iter()
        .all(|byte| matches!(byte, 0x20..=0x7e | b'\r' | b'\n' | b'\t'));
    if !text {
        let each: Vec<String> = bytes.iter().map(|byte| format!("{byte:02X}")).collect();
        return format!("[{}]", each.join(" "));
    }

    let mut shown = String::from("\"");
    for byte in bytes {
        match byte {
            b'"' => shown.push_str("\\\""),
            b'\\' => shown.push_str("\\\\"),
            b'\r' => shown.push_str("\\r"),
            b'\n' => shown.push_str("\\n"),
            b'\t' => shown.push_str("\\t"),
            other => shown.push(*other as char),
        }
    }
    shown.push('"');
    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bytes in brackets, in either case, one or two digits each, with space
    /// between them.
    #[test]
    fn bytes_in_brackets_are_read_as_the_bytes_they_are() {
        assert_eq!(trigger_bytes("[55]").unwrap(), vec![0x55]);
        assert_eq!(trigger_bytes("[aa]").unwrap(), vec![0xAA]);
        assert_eq!(trigger_bytes("[Aa]").unwrap(), vec![0xAA]);
        assert_eq!(trigger_bytes("[55 AA 0D]").unwrap(), vec![0x55, 0xAA, 0x0D]);
        assert_eq!(trigger_bytes("  [ 0d\t0a ]  ").unwrap(), vec![0x0D, 0x0A]);

        // One digit is the same byte as two: a datasheet writes 5 where it
        // means 05, and counting zeroes is not what a configuration is for.
        assert_eq!(trigger_bytes("[5]").unwrap(), vec![0x05]);
        assert_eq!(
            trigger_bytes("[05]").unwrap(),
            trigger_bytes("[5]").unwrap()
        );
    }

    /// A run of digits is not split into bytes: how many bytes it meant
    /// would be a guess from its length.
    #[test]
    fn a_run_of_digits_is_refused() {
        for written in ["[55AA]", "[0400]", "[03 0400]"] {
            let said = trigger_bytes(written).expect_err("not one byte");
            assert!(said.contains("more than one byte"), "{written}: {said}");
        }
    }

    /// What is not a byte in brackets is said not to be.
    #[test]
    fn what_is_not_a_byte_in_brackets_is_refused() {
        let said = trigger_bytes("[5g]").expect_err("g is not a digit");
        assert!(said.contains("hexadecimal digit"), "{said}");
        let said = trigger_bytes("[0D, 0A]").expect_err("commas are not bytes");
        assert!(said.contains("hexadecimal digit"), "{said}");
        let said = trigger_bytes("[0x55]").expect_err("no 0x inside brackets");
        assert!(said.contains("[55]"), "{said}");
        let said = trigger_bytes("[]").expect_err("no byte");
        assert!(said.contains("names no byte"), "{said}");
        let said = trigger_bytes("[55").expect_err("open bracket");
        assert!(said.contains("does not close"), "{said}");
    }

    /// The spelling this replaced -- bytes written bare -- is refused with
    /// the brackets it wants, rather than puzzled over or read as something.
    #[test]
    fn bytes_written_bare_are_told_their_brackets() {
        for (written, wanted) in [
            ("55", "[55]"),
            ("0x55", "[55]"),
            ("0X55", "[55]"),
            ("0D", "[0D]"),
            ("5", "[05]"),
            ("55 AA", "[55 AA]"),
        ] {
            let said = trigger_bytes(written).expect_err("bare bytes");
            assert!(
                said.contains("square brackets") && said.contains(wanted),
                "{written}: {said}"
            );
        }

        // A terminator was decimal unless it said 0x, so its brackets are
        // the byte it meant.
        for (written, wanted) in [("13", "[0D]"), ("0x0A", "[0A]"), ("10", "[0A]")] {
            let said = bytes_of(&TERMINATOR, written).expect_err("bare bytes");
            assert!(
                said.contains("square brackets") && said.contains(wanted),
                "{written}: {said}"
            );
        }
    }

    /// A word without quotes is neither notation, and is said to be.
    #[test]
    fn a_word_without_quotes_is_refused() {
        let said = trigger_bytes("READ").expect_err("neither text nor bytes");
        assert!(said.contains("begins neither"), "{said}");
        let said = trigger_bytes("\"AT\" 0D").expect_err("a bare byte after text");
        assert!(said.contains("begins neither"), "{said}");
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
        // quotes being the mark makes possible -- and an escaped quote does
        // not close one.
        let said = trigger_bytes("\"READ").unwrap_err();
        assert!(said.contains("does not close"), "{said}");
        let said = trigger_bytes("\"READ\\\"").unwrap_err();
        assert!(said.contains("does not close"), "{said}");
    }

    /// Parts one after another are the bytes they make together, with space
    /// between them or none.
    #[test]
    fn parts_one_after_another_are_one_run_of_bytes() {
        assert_eq!(trigger_bytes("\"AT\" [0D 0A]").unwrap(), b"AT\r\n".to_vec());
        assert_eq!(trigger_bytes("\"AT\"[0D]").unwrap(), b"AT\r".to_vec());
        assert_eq!(
            trigger_bytes("[02] \"T=\" [03]").unwrap(),
            vec![0x02, b'T', b'=', 0x03]
        );
        assert_eq!(trigger_bytes("\"a\" \"b\"").unwrap(), b"ab".to_vec());
        // A bracket inside a string is a character of it.
        assert_eq!(trigger_bytes("\"[x]\"").unwrap(), b"[x]".to_vec());
    }

    /// An escape C does not have is refused rather than passed through.
    #[test]
    fn an_escape_c_does_not_have_is_refused() {
        let said = trigger_bytes("\"\\q\"").unwrap_err();
        assert!(
            said.contains("not an escape") && said.contains("in brackets"),
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
        for written in ["", "   ", "\"\"", "\"\" \"\""] {
            let said = trigger_bytes(written).unwrap_err();
            assert!(said.contains("no bytes"), "{written:?}: {said}");
        }
    }

    /// The two notations can write the same bytes, and do.
    #[test]
    fn the_two_notations_meet() {
        assert_eq!(
            trigger_bytes("[0D]").unwrap(),
            trigger_bytes("\"\\r\"").unwrap()
        );
        assert_eq!(
            trigger_bytes("[41 54]").unwrap(),
            trigger_bytes("\"AT\"").unwrap()
        );
    }

    /// Several sequences, any of which ends a packet, are written with commas
    /// between them; a comma inside a string is a character of it.
    #[test]
    fn sequences_are_split_at_the_commas_between_them() {
        assert_eq!(
            sequences_of(&TERMINATOR, "\"\\r\\n\", [03]").unwrap(),
            vec![b"\r\n".to_vec(), vec![0x03]]
        );
        assert_eq!(
            sequences_of(&TERMINATOR, "\",\"").unwrap(),
            vec![b",".to_vec()]
        );
        assert_eq!(
            sequences_of(&TERMINATOR, "\"\\\",\" , [0A]").unwrap(),
            vec![b"\",".to_vec(), vec![0x0A]]
        );
        for written in ["[0D],", ",[0D]", "[0D],,[0A]"] {
            let said = sequences_of(&TERMINATOR, written).unwrap_err();
            assert!(
                said.contains("terminator") && said.contains("no bytes"),
                "{written}: {said}"
            );
        }
    }

    /// Bytes read back as a file could have written them.
    #[test]
    fn bytes_are_shown_as_a_file_could_write_them() {
        assert_eq!(trigger_as_written(&[0x55, 0xAA]), "[55 AA]");
        assert_eq!(trigger_as_written(&[0x00]), "[00]");
        assert_eq!(trigger_as_written(b"READ"), "\"READ\"");
        assert_eq!(trigger_as_written(b"READ\r"), "\"READ\\r\"");
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
