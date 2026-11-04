//! Saving a file so that it changes only where it was edited.
//!
//! The editor holds a configuration as a model, and writes a file by writing
//! the model out. That is what makes a file it writes always say what the
//! model says -- and on its own it would make every save rewrite the whole
//! file in the editor's own hand: a comment broken where somebody else
//! broke it, a value quoted that was bare, a blank line gone, a description
//! folded onto one line. A person who moves one node and finds forty lines
//! of their file changed has been given a reason not to use the editor.
//!
//! So what is written is the file as it was read, with the edit made to it.
//! Three texts decide it, each held for each file the session writes:
//!
//! * the *base*: what the editor would have written for the file as it was
//!   read, or as it was last written;
//! * the *kept*: what the file actually held then, which differs from the
//!   base wherever its author wrote something the editor would have written
//!   otherwise;
//! * the *now*: what the editor writes for the model as it is.
//!
//! The difference between the base and the now is the edit, said in the
//! editor's hand, and the difference between the base and the kept is the
//! author's hand. Each is a line diff, and the two are merged: the edit's
//! lines go in, and every line the edit did not touch is the author's. A
//! save that edited nothing writes the file back as it was, byte for byte,
//! without a merge at all.
//!
//! Where the two touch the same lines, the edit's hand wins over those
//! lines: the line somebody edited is the one that has to change. A line
//! whose value only was edited keeps the quotes its author put round it,
//! where they read the same.
//!
//! And the merged text is then held to the model. It is read again and
//! compared, value for value, with what the editor would have written; if
//! the two say anything different -- a merge that put a line under the
//! wrong entry, an author's file that said something the model does not --
//! the editor's own text is written instead. A file that keeps its author's
//! hand and says the wrong thing would be worse than one that does not, and
//! the editor's own text is always right.

use std::path::Path;

use similar::{capture_diff_slices, Algorithm, DiffTag};

/// One file as the session last read or wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// What the editor wrote, or would have written, for it then.
    pub base: String,
    /// What the file held then.
    pub kept: String,
}

/// The text to write to a file, keeping what the edit did not change.
///
/// # Arguments
///
/// * `path`    - The file, whose extension says whether it is XML or YAML.
/// * `written` - What the editor wrote or would have written for it when it
///               was last read or written, and what it held then.
/// * `now`     - What the editor writes for the model as it is.
///
/// # Returns
///
/// The kept text unchanged where `now` is the base; otherwise the kept text
/// with the edit merged into it, where that reads as `now` does; otherwise
/// `now`.
pub fn text(path: &Path, written: &Written, now: &str) -> String {
    if now == written.base {
        return written.kept.clone();
    }
    let xml = is_xml(path);
    let merged = merged(&written.base, &written.kept, now, !xml);
    if says_what_it_should(&merged, written, now, xml) {
        merged
    } else {
        now.to_string()
    }
}

/// What a text says, one value or one element to a line, without its
/// comments, quoting or layout.
///
/// # Arguments
///
/// * `text` - The text.
/// * `xml`  - Whether it is XML rather than YAML.
///
/// # Returns
///
/// The lines, or `None` where the text does not read.
fn said(text: &str, xml: bool) -> Option<Vec<String>> {
    if xml {
        xml_said(text)
    } else {
        let value = serde_norway::from_str(text).ok()?;
        let mut lines = Vec::new();
        yaml_said(&value, &mut lines);
        Some(lines)
    }
}

/// Whether a merged text says what the edit should have made the kept text
/// say.
///
/// Each of the four texts is reduced to what it says -- see [`said`] -- and
/// the same merge is made of what the base, the kept text and the now say.
/// The merged text has to say what that merge does. So a value the kept
/// text has and the editor would not have written -- a node settled
/// elsewhere as the file was opened, a section a later build added -- is
/// expected to stay, a value the edit changed is expected to have changed,
/// and nothing else is.
///
/// # Arguments
///
/// * `merged`  - The merged text.
/// * `written` - The base and the kept text it was merged from.
/// * `now`     - What the editor writes now.
/// * `xml`     - Whether the texts are XML rather than YAML.
///
/// # Returns
///
/// Whether every text reads and the merged one says what it should.
fn says_what_it_should(
    merged: &str,
    written: &Written,
    now: &str,
    xml: bool,
) -> bool {
    let lines = |text: &str| -> Option<String> {
        let lines = said(text, xml)?;
        Some(lines.into_iter().map(|line| line + "\n").collect())
    };
    let (Some(merged), Some(base), Some(kept), Some(now)) = (
        lines(merged),
        lines(&written.base),
        lines(&written.kept),
        lines(now),
    ) else {
        return false;
    };
    merged == self::merged(&base, &kept, &now, false)
}

/// Whether a file is written as XML rather than as YAML.
///
/// # Arguments
///
/// * `path` - The file.
///
/// # Returns
///
/// True for a `.xtcs` file, false for anything else: every other file the
/// editor writes is YAML.
fn is_xml(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("xtcs"))
}

/// One change a diff makes to the base: the lines from `start` up to `end`
/// replaced by `lines`. An insertion has `start` and `end` equal.
#[derive(Debug, Clone)]
struct Hunk<'a> {
    start: usize,
    end: usize,
    lines: Vec<&'a str>,
}

/// A text as its lines, each with its line ending, so that a text without a
/// final newline is put back without one.
///
/// # Arguments
///
/// * `text` - The text.
///
/// # Returns
///
/// The lines.
fn lines_of(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

/// The changes that turn one text's lines into another's.
///
/// # Arguments
///
/// * `base`  - The lines changed.
/// * `other` - The lines they become.
///
/// # Returns
///
/// Each change, in order of where it is in the base; none overlap.
fn hunks<'a>(base: &[&'a str], other: &[&'a str]) -> Vec<Hunk<'a>> {
    capture_diff_slices(Algorithm::Patience, base, other)
        .into_iter()
        .map(|op| op.as_tag_tuple())
        .filter(|(tag, _, _)| *tag != DiffTag::Equal)
        .map(|(_, old, new)| Hunk {
            start: old.start,
            end: old.end,
            lines: other[new].to_vec(),
        })
        .collect()
}

/// Whether a change reaches into a stretch of the base: shares a line with
/// it, or is an insertion strictly inside it, or both are insertions at one
/// place.
///
/// # Arguments
///
/// * `hunk`  - The change.
/// * `start` - Where the stretch begins.
/// * `end`   - Where it ends.
///
/// # Returns
///
/// Whether the two cannot both be made without deciding between them.
fn reaches(hunk: &Hunk, start: usize, end: usize) -> bool {
    match (hunk.start == hunk.end, start == end) {
        (false, false) => hunk.start < end && start < hunk.end,
        (true, false) => start < hunk.start && hunk.start < end,
        (false, true) => hunk.start < start && start < hunk.end,
        (true, true) => hunk.start == start,
    }
}

/// A stretch of the base with changes made to it.
///
/// # Arguments
///
/// * `base`  - The base's lines.
/// * `start` - Where the stretch begins.
/// * `end`   - Where it ends.
/// * `made`  - The changes, all within the stretch, in order.
///
/// # Returns
///
/// The stretch's lines with the changes made.
fn applied<'a>(
    base: &[&'a str],
    start: usize,
    end: usize,
    made: &[&Hunk<'a>],
) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut at = start;
    for hunk in made {
        out.extend_from_slice(&base[at..hunk.start]);
        out.extend_from_slice(&hunk.lines);
        at = hunk.end;
    }
    out.extend_from_slice(&base[at..end]);
    out
}

/// The kept text with the change from the base to the now made to it.
///
/// A three-way merge of lines. A change only one side made is made; where
/// both sides changed one stretch of the base, the stretch is the now's,
/// except that, where the three say it in the same number of lines, a line
/// the now left as the base had it is the kept one, and in YAML an edited
/// value keeps the quotes the kept line had. Two insertions at one place are
/// the now's first: a line the editor adds before an entry goes above any
/// comment its author wrote over that entry.
///
/// # Arguments
///
/// * `base` - What the editor wrote for the file when it was last read.
/// * `kept` - What the file held then.
/// * `now`  - What the editor writes now.
/// * `yaml` - Whether the file is YAML, whose values can keep their quotes.
///
/// # Returns
///
/// The merged text.
pub fn merged(base: &str, kept: &str, now: &str, yaml: bool) -> String {
    let base = lines_of(base);
    let kept = lines_of(kept);
    let now = lines_of(now);
    let theirs = hunks(&base, &kept);
    let ours = hunks(&base, &now);

    let mut out: Vec<String> = Vec::new();
    let mut at = 0;
    let (mut k, mut n) = (0, 0);
    while k < theirs.len() || n < ours.len() {
        let next_kept = theirs.get(k);
        let next_now = ours.get(n);

        // Which comes first, and whether the other reaches into it.
        let (first_is_now, start, end) = match (next_kept, next_now) {
            (Some(t), Some(o)) => {
                let now_first = o.start < t.start
                    || (o.start == t.start && o.start == o.end);
                if now_first {
                    (true, o.start, o.end)
                } else {
                    (false, t.start, t.end)
                }
            }
            (None, Some(o)) => (true, o.start, o.end),
            (Some(t), None) => (false, t.start, t.end),
            (None, None) => unreachable!("the loop has a change left"),
        };

        // Two insertions at one place are no conflict: the now's, then the
        // kept's.
        if let (Some(t), Some(o)) = (next_kept, next_now) {
            let both_insert = t.start == t.end && o.start == o.end;
            if both_insert && t.start == o.start {
                out.extend(
                    base[at..o.start.max(at)].iter().map(|l| l.to_string()),
                );
                out.extend(o.lines.iter().map(|l| l.to_string()));
                out.extend(t.lines.iter().map(|l| l.to_string()));
                at = o.start;
                k += 1;
                n += 1;
                continue;
            }
        }

        let other = if first_is_now { next_kept } else { next_now };
        let conflict = other.is_some_and(|h| reaches(h, start, end));
        if !conflict {
            let hunk = if first_is_now {
                n += 1;
                next_now
            } else {
                k += 1;
                next_kept
            }
            .expect("the change chosen");
            out.extend(
                base[at..hunk.start.max(at)].iter().map(|l| l.to_string()),
            );
            out.extend(hunk.lines.iter().map(|l| l.to_string()));
            at = hunk.end;
            continue;
        }

        // A conflict: gather every change on either side that reaches into
        // the stretch, which grows as they are gathered.
        let (mut start, mut end) = (start, end);
        let mut kept_made: Vec<&Hunk> = Vec::new();
        let mut now_made: Vec<&Hunk> = Vec::new();
        if first_is_now {
            now_made.push(&ours[n]);
            n += 1;
        } else {
            kept_made.push(&theirs[k]);
            k += 1;
        }
        loop {
            let mut grew = false;
            for (side, made, i) in [
                (&theirs, &mut kept_made, &mut k),
                (&ours, &mut now_made, &mut n),
            ] {
                while let Some(h) = side.get(*i) {
                    if !reaches(h, start, end) {
                        break;
                    }
                    start = start.min(h.start);
                    end = end.max(h.end);
                    made.push(h);
                    *i += 1;
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }

        out.extend(base[at..start.max(at)].iter().map(|l| l.to_string()));
        let was = &base[start..end];
        let theirs_now = applied(&base, start, end, &kept_made);
        let ours_now = applied(&base, start, end, &now_made);
        if was.len() == theirs_now.len() && was.len() == ours_now.len() {
            for ((b, t), o) in was.iter().zip(&theirs_now).zip(&ours_now) {
                if o == b {
                    out.push(t.to_string());
                } else if yaml {
                    out.push(restyled(t, o).unwrap_or_else(|| o.to_string()));
                } else {
                    out.push(o.to_string());
                }
            }
        } else {
            out.extend(ours_now.iter().map(|l| l.to_string()));
        }
        at = end;
    }
    out.extend(base[at..].iter().map(|l| l.to_string()));
    out.concat()
}

/// A YAML line with an edited value, in the quotes the kept line had.
///
/// Only where the two lines are the same key at the same indent, the kept
/// value was quoted and the edited one is not, and the quoted value reads
/// as the same value: a number quoted would be a string, and is left bare.
///
/// # Arguments
///
/// * `kept` - The line as the file had it.
/// * `now`  - The line as the editor writes it.
///
/// # Returns
///
/// The edited line in the kept line's quotes, or `None` where that does not
/// apply.
fn restyled(kept: &str, now: &str) -> Option<String> {
    let (kept_key, kept_value) = kept.split_once(": ")?;
    let (now_key, now_value) = now.split_once(": ")?;
    if kept_key != now_key {
        return None;
    }
    let ending = if now_value.ends_with('\n') { "\n" } else { "" };
    let bare = now_value.trim_end_matches('\n');
    let quote = kept_value.chars().next()?;
    if !matches!(quote, '"' | '\'') || bare.starts_with(['"', '\'']) {
        return None;
    }
    let quoted = if quote == '"' {
        format!("\"{}\"", bare.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        format!("'{}'", bare.replace('\'', "''"))
    };
    let read = |value: &str| {
        serde_norway::from_str::<serde_norway::Value>(&format!("k: {value}"))
            .ok()
    };
    let same = read(&quoted).is_some() && read(&quoted) == read(bare);
    same.then(|| format!("{now_key}: {quoted}{ending}"))
}

/// What a YAML text says, one line to each value, key and list, without
/// its comments, quoting or layout.
///
/// A number, a boolean and a text are each the text of it, so that a value
/// written bare and one written in quotes say the same thing.
///
/// # Arguments
///
/// * `value` - The text, as it parsed.
/// * `out`   - Where the lines are written.
fn yaml_said(value: &serde_norway::Value, out: &mut Vec<String>) {
    use serde_norway::Value;
    match value {
        Value::Null => out.push("~".to_string()),
        Value::Bool(b) => out.push(format!("= {b:?}")),
        Value::Number(n) => out.push(format!("= {n}")),
        Value::String(s) => out.push(format!("= {s:?}")),
        Value::Sequence(items) => {
            out.push("[".to_string());
            for item in items {
                yaml_said(item, out);
            }
            out.push("]".to_string());
        }
        Value::Mapping(map) => {
            // By key, so that two files saying one thing in two orders say
            // it in one.
            let mut keyed: Vec<(String, &Value)> = map
                .iter()
                .map(|(key, value)| {
                    let mut said = Vec::new();
                    yaml_said(key, &mut said);
                    (said.join(" "), value)
                })
                .collect();
            keyed.sort_by(|a, b| a.0.cmp(&b.0));
            out.push("{".to_string());
            for (key, value) in keyed {
                out.push(format!("key {key}"));
                yaml_said(value, out);
            }
            out.push("}".to_string());
        }
        Value::Tagged(tagged) => yaml_said(&tagged.value, out),
    }
}

/// What an XML text says: its elements, their attributes and their text, in
/// order, without its comments or the white space between elements.
///
/// # Arguments
///
/// * `text` - The text.
///
/// # Returns
///
/// One entry for each element's start, its end and each run of its text, or
/// `None` where the text is not well-formed XML.
fn xml_said(text: &str) -> Option<Vec<String>> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(text);
    let mut out = Vec::new();
    let mut words = String::new();
    let flush = |words: &mut String, out: &mut Vec<String>| {
        let trimmed = words.trim();
        if !trimmed.is_empty() {
            out.push(format!("text {trimmed}"));
        }
        words.clear();
    };
    loop {
        match reader.read_event().ok()? {
            // An empty element is one that starts and ends at once.
            Event::Empty(e) => {
                flush(&mut words, &mut out);
                out.push(format!("start {}", element_of(&e)?));
                out.push(format!("end {}", e.name().as_ref()));
            }
            Event::Start(e) => {
                flush(&mut words, &mut out);
                out.push(format!("start {}", element_of(&e)?));
            }
            Event::End(e) => {
                flush(&mut words, &mut out);
                out.push(format!("end {}", e.name().as_ref()));
            }
            Event::Text(e) => words.push_str(&e),
            Event::CData(e) => words.push_str(&e.into_inner()),
            Event::GeneralRef(e) => {
                let resolved = match e.resolve_char_ref().ok()? {
                    Some(c) => c.to_string(),
                    None => unescaped(&format!("&{};", &*e)),
                };
                words.push_str(&resolved);
            }
            Event::Eof => break,
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) => {}
            Event::DocType(_) => {}
        }
    }
    flush(&mut words, &mut out);
    Some(out)
}

/// An element's name and its attributes, in the attributes' name order.
///
/// # Arguments
///
/// * `element` - The element's start.
///
/// # Returns
///
/// The name and each attribute as `name=value`, unescaped, or `None` where
/// an attribute is not well-formed.
fn element_of(element: &quick_xml::events::BytesStart) -> Option<String> {
    let mut attributes: Vec<String> = Vec::new();
    for attribute in element.attributes() {
        let attribute = attribute.ok()?;
        attributes.push(format!(
            "{}={}",
            attribute.key.as_ref(),
            unescaped(&attribute.value)
        ));
    }
    attributes.sort();
    Some(format!(
        "{} {}",
        element.name().as_ref(),
        attributes.join(" ")
    ))
}

/// XML's five entities, back to what they stand for.
///
/// # Arguments
///
/// * `text` - XML character data or an attribute's value.
///
/// # Returns
///
/// The text with `&lt;`, `&gt;`, `&quot;`, `&apos;` and `&amp;` replaced.
fn unescaped(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Whether two texts of one file say the same thing, whatever their
/// comments, quoting and layout.
///
/// # Arguments
///
/// * `one`   - One text.
/// * `other` - The other.
/// * `xml`   - Whether they are XML rather than YAML.
///
/// # Returns
///
/// Whether both read and say the same values in the same places.
#[cfg(test)]
fn same_meaning(one: &str, other: &str, xml: bool) -> bool {
    let one = said(one, xml);
    one.is_some() && one == said(other, xml)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The base, a hand-written copy of it and an edit, for the merging
    /// tests: a YAML list whose author commented it, quoted one value and
    /// left a blank line the editor does not write.
    const BASE: &str = "\
nodes:
  - { node: a, x: 1, y: 2 }
  - { node: b, x: 3, y: 4 }
  - { node: c, x: 5, y: 6 }
values:
  power: 4 W
  start: 1 K
";
    const KEPT: &str = "\
# Where the boxes are.
nodes:
  - { node: a, x: 1, y: 2 }
  # b is the one in the middle.
  - { node: b, x: 3, y: 4 }
  - { node: c, x: 5, y: 6 }

values:
  power: \"4 W\"
  start: 1 K
";

    /// A save that changed nothing writes the file as it was.
    #[test]
    fn nothing_edited_is_the_file_as_it_was() {
        let written = Written {
            base: BASE.to_string(),
            kept: KEPT.to_string(),
        };
        assert_eq!(text(Path::new("a.ytcs"), &written, BASE), KEPT);
    }

    /// An edited line is the one line that changes, and everything the
    /// author wrote round it stays.
    #[test]
    fn an_edit_changes_its_own_line_and_no_other() {
        let now =
            BASE.replace("{ node: b, x: 3, y: 4 }", "{ node: b, x: 9, y: 4 }");
        let written = Written {
            base: BASE.to_string(),
            kept: KEPT.to_string(),
        };
        assert_eq!(
            text(Path::new("a.ytcs"), &written, &now),
            KEPT.replace("{ node: b, x: 3, y: 4 }", "{ node: b, x: 9, y: 4 }")
        );
    }

    /// A value its author quoted keeps its quotes when it is edited, and a
    /// line added beside it is added and nothing else.
    #[test]
    fn an_edited_value_keeps_its_quotes() {
        let now = BASE
            .replace("power: 4 W", "power: 5 W")
            .replace("  start: 1 K\n", "  start: 1 K\n  swing: 2 K\n");
        let written = Written {
            base: BASE.to_string(),
            kept: KEPT.to_string(),
        };
        assert_eq!(
            text(Path::new("a.ytcs"), &written, &now),
            KEPT.replace("power: \"4 W\"", "power: \"5 W\"")
                .replace("  start: 1 K\n", "  start: 1 K\n  swing: 2 K\n")
        );
    }

    /// A line added where the author had written a comment goes above the
    /// comment, which is about what follows it.
    #[test]
    fn an_addition_goes_above_the_authors_comment() {
        let now = BASE.replace(
            "  - { node: b",
            "  - { node: ab, x: 7, y: 8 }\n  - { node: b",
        );
        let written = Written {
            base: BASE.to_string(),
            kept: KEPT.to_string(),
        };
        assert_eq!(
            text(Path::new("a.ytcs"), &written, &now),
            KEPT.replace("  # b is", "  - { node: ab, x: 7, y: 8 }\n  # b is")
        );
    }

    /// A value the author's file has and the editor's does not stays, and
    /// a merge that said anything else would not be written.
    #[test]
    fn a_merge_is_held_to_what_the_edit_says() {
        let kept = KEPT.replace("start: 1 K", "start: 3 K");
        let now = BASE.replace("power: 4 W", "power: 5 W");
        let written = Written {
            base: BASE.to_string(),
            kept: kept.clone(),
        };
        let merged = text(Path::new("a.ytcs"), &written, &now);
        assert_eq!(merged, kept.replace("\"4 W\"", "\"5 W\""));
        assert!(says_what_it_should(&merged, &written, &now, false));

        // Not the edit, the author's value lost, a value moved to another
        // entry: each says something else.
        for wrong in [
            kept.clone(),
            merged.replace("3 K", "1 K"),
            merged
                .replace("  start: 3 K\n", "")
                .replace("nodes:\n", "nodes:\n  - start: 3 K\n"),
        ] {
            assert!(
                !says_what_it_should(&wrong, &written, &now, false),
                "{wrong}"
            );
        }
    }

    /// A section the editor does not write is kept as it was, and does not
    /// stop the edit being merged.
    #[test]
    fn a_section_the_editor_does_not_write_is_kept() {
        let kept = format!("{KEPT}\nlater:\n  - thing: 1\n");
        let now = BASE.replace("power: 4 W", "power: 5 W");
        let written = Written {
            base: BASE.to_string(),
            kept: kept.clone(),
        };
        assert_eq!(
            text(Path::new("a.yaml"), &written, &now),
            kept.replace("power: \"4 W\"", "power: \"5 W\"")
        );
    }

    /// XML is compared by its elements and their text, not its comments or
    /// its layout.
    #[test]
    fn xml_is_compared_by_what_it_says() {
        let one = "<a>\n    <!-- said -->\n    <b x=\"1\">it&apos;s</b>\n\
                   <c/>\n</a>\n";
        let other = "<a><b x='1'>it's</b><c></c></a>";
        assert!(same_meaning(one, other, true));
        assert!(!same_meaning(one, "<a><b x='2'>it's</b><c/></a>", true));
    }
}
