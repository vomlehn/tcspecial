//! Writing this build's interface digest into a configuration, in place.
//!
//! A configuration names the interface it is written for as its `tcspecial`
//! section's `interface`, and every program refuses one that is not its
//! build's. So a change to `tcslibgs/xtce/link.xml` -- a new interface --
//! leaves every configuration to be given the new digest. This does it to a
//! file as it stands: the one value is rewritten, or the attribute added
//! where the section has none, and every other byte of the file -- its
//! comments, its order, its layout, its quoting -- is left as it was. The
//! file it writes is read back before it replaces the old one, and must
//! name this build's interface, or nothing is written.
//!
//! `tcsverify --set-interface <configuration>...` is the command; see "The
//! configuration names its interface" in `docs/design.rst`.

use std::path::Path;

use tcslibgs::InterfaceDigest;

/// What setting the interface did to a file's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Set {
    /// It named this build's interface already, and nothing was changed.
    Already,
    /// It named another, this, which is now this build's.
    Changed(String),
    /// It named none, and now names this build's.
    Added,
}

/// Gives the text of a configuration with its interface set to `ours`.
///
/// # Arguments
///
/// * `text` - The configuration, as its file holds it.
/// * `xml`  - Whether it is written in XML, rather than YAML.
/// * `ours` - The digest to write, as 32 lower-case hex digits.
///
/// # Returns
///
/// The new text, and what was done; the text unchanged where it named
/// `ours` already.
///
/// # Errors
///
/// A message saying why the attribute cannot be found or written: the file
/// has no `tcspecial` section, or writes it in a form this does not edit.
pub fn set_in_text(
    text: &str,
    xml: bool,
    ours: &str,
) -> Result<(String, Set), String> {
    if xml {
        set_in_xml(text, ours)
    } else {
        set_in_yaml(text, ours)
    }
}

/// [`set_in_text`] for YAML: the section is a block at the top level,
/// `tcspecial:` on a line of its own, and `interface` one of its lines.
///
/// # Arguments
///
/// * `text` - The configuration, as its file holds it.
/// * `ours` - The digest to write.
///
/// # Returns
///
/// The new text, and what was done.
///
/// # Errors
///
/// Why the attribute cannot be found or written.
fn set_in_yaml(text: &str, ours: &str) -> Result<(String, Set), String> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let start = lines
        .iter()
        .position(|line| {
            let bare = line.trim_end();
            bare == "tcspecial:"
                || bare
                    .strip_prefix("tcspecial:")
                    .is_some_and(|rest| rest.trim_start().starts_with('#'))
        })
        .ok_or_else(|| {
            if text.lines().any(|line| line.starts_with("tcspecial:")) {
                "its tcspecial section is written on one line, which this \
                 does not edit: write it as a block, one attribute to a line"
                    .to_string()
            } else {
                "it has no tcspecial section, so it names no interface"
                    .to_string()
            }
        })?;
    // The block: every line after it that is indented, blank or a comment.
    let end = lines[start + 1..]
        .iter()
        .position(|line| {
            let bare = line.trim_end_matches(['\n', '\r']);
            !(bare.is_empty()
                || bare.starts_with(' ')
                || bare.starts_with('\t')
                || bare.trim_start().starts_with('#'))
        })
        .map_or(lines.len(), |at| start + 1 + at);
    let block = start + 1..end;
    let indent_of = |line: &str| line.len() - line.trim_start().len();
    let key_indent = lines[block.clone()]
        .iter()
        .find(|line| {
            let bare = line.trim();
            !bare.is_empty() && !bare.starts_with('#')
        })
        .map(|line| indent_of(line))
        .ok_or("its tcspecial section is empty")?;

    let mut out: Vec<String> =
        lines.iter().map(|line| line.to_string()).collect();
    for at in block.clone() {
        let line = lines[at];
        if indent_of(line) != key_indent {
            continue;
        }
        let Some(rest) = line.trim_start().strip_prefix("interface:") else {
            continue;
        };
        let ending = &line[line.trim_end_matches(['\n', '\r']).len()..];
        let rest = rest.trim_end_matches(['\n', '\r']);
        // The value, and whatever follows it on the line: a comment.
        let value_start = rest.len() - rest.trim_start().len();
        let after = &rest[value_start..];
        let (value, quote, tail) = match after.chars().next() {
            Some(q @ ('"' | '\'')) => {
                let close = after[1..].find(q).ok_or_else(|| {
                    format!(
                        "its interface value on line {} is not closed",
                        at + 1
                    )
                })?;
                (&after[1..1 + close], Some(q), &after[close + 2..])
            }
            _ => {
                let end = after.find(" #").unwrap_or(after.len());
                (after[..end].trim_end(), None, &after[end..])
            }
        };
        if value == ours {
            return Ok((text.to_string(), Set::Already));
        }
        let was = value.to_string();
        // A digest is quoted where it was not, since one of digits alone, or
        // of digits and an `e`, would read as a number.
        let q = quote.unwrap_or('"');
        out[at] = format!(
            "{}interface:{}{q}{ours}{q}{tail}{ending}",
            &line[..key_indent],
            &rest[..value_start]
        );
        return Ok((out.concat(), Set::Changed(was)));
    }

    // None: added after the gateway, where the section has one, and else
    // first, at the section's indentation.
    let after = block
        .clone()
        .find(|&at| {
            indent_of(lines[at]) == key_indent
                && lines[at].trim_start().starts_with("gateway:")
        })
        .unwrap_or(start);
    let ending = if lines[after].ends_with("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    if !lines[after].ends_with('\n') {
        out[after].push_str(ending);
    }
    out.insert(
        after + 1,
        format!("{}interface: \"{ours}\"{ending}", " ".repeat(key_indent)),
    );
    Ok((out.concat(), Set::Added))
}

/// [`set_in_text`] for XML: `<interface>` inside `<tcspecial>`.
///
/// # Arguments
///
/// * `text` - The configuration, as its file holds it.
/// * `ours` - The digest to write.
///
/// # Returns
///
/// The new text, and what was done.
///
/// # Errors
///
/// Why the attribute cannot be found or written.
fn set_in_xml(text: &str, ours: &str) -> Result<(String, Set), String> {
    let open = text
        .find("<tcspecial>")
        .ok_or("it has no tcspecial section, so it names no interface")?;
    let close = text[open..]
        .find("</tcspecial>")
        .map(|at| open + at)
        .ok_or("its tcspecial section is not closed")?;
    let section = &text[open..close];
    if let Some(at) = section.find("<interface>") {
        let value_start = open + at + "<interface>".len();
        let value_end = text[value_start..close]
            .find("</interface>")
            .map(|at| value_start + at)
            .ok_or("its interface element is not closed")?;
        let value = text[value_start..value_end].trim();
        if value == ours {
            return Ok((text.to_string(), Set::Already));
        }
        let was = value.to_string();
        let mut out = String::with_capacity(text.len());
        out.push_str(&text[..value_start]);
        out.push_str(ours);
        out.push_str(&text[value_end..]);
        return Ok((out, Set::Changed(was)));
    }

    // None: added on a line of its own after the gateway, where the section
    // names one, and else first, indented as the element after it is.
    let insert_after = match section.find("</gateway>") {
        Some(at) => open + at + "</gateway>".len(),
        None => open + "<tcspecial>".len(),
    };
    let next = text[insert_after..]
        .find('<')
        .map(|at| insert_after + at)
        .unwrap_or(insert_after);
    let between = &text[insert_after..next];
    let indent = between
        .rfind('\n')
        .map(|at| &between[at..])
        .unwrap_or(between);
    let mut out = String::with_capacity(text.len() + 64);
    out.push_str(&text[..insert_after]);
    out.push_str(indent);
    out.push_str(&format!("<interface>{ours}</interface>"));
    out.push_str(&text[insert_after..]);
    Ok((out, Set::Added))
}

/// Sets the interface of the configuration at `path` to this build's, in
/// place.
///
/// The new text is written beside the file, read back as a configuration
/// and held to this build's interface, and only then renamed over the file,
/// with the file's own permissions: a rewrite that went wrong leaves the
/// file as it was.
///
/// # Arguments
///
/// * `path` - The configuration, `.yaml`, `.yml` or `.xml`.
///
/// # Returns
///
/// What was done.
///
/// # Errors
///
/// Why it was not done, naming the file.
pub fn set_in_file(path: &Path) -> Result<Set, String> {
    let said = |why: String| format!("{}: {why}", path.display());
    let ours = InterfaceDigest::of_this_build().to_string();
    let xml = matches!(
        tcslibgs::ConfigFormat::of_file(path)
            .map_err(|e| said(e.to_string()))?,
        tcslibgs::ConfigFormat::Xml
    );
    let text = std::fs::read_to_string(path)
        .map_err(|e| said(format!("cannot be read: {e}")))?;
    let (new, set) = set_in_text(&text, xml, &ours).map_err(said)?;
    if set == Set::Already {
        return Ok(set);
    }
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".interface-new");
    // Beside it, with its extension last, so that it is read as what it is.
    let beside = path.with_file_name({
        let mut beside = std::ffi::OsString::from(".");
        beside.push(&name);
        beside.push(".");
        beside.push(path.extension().unwrap_or_default());
        beside
    });
    std::fs::write(&beside, &new)
        .map_err(|e| said(format!("cannot be written: {e}")))?;
    let checked = tcslibgs::graph::Graph::load(&beside)
        .map_err(|e| e.to_string())
        .and_then(|graph| {
            let section = graph
                .tcspecial
                .ok_or("it lost its tcspecial section".to_string())?;
            let flat = tcslibgs::graph::parameters::flatten(&section);
            let named = flat
                .get("interface")
                .map(|value| value.as_str().trim().to_string())
                .unwrap_or_default();
            tcslibgs::graph::parameters::interface_of(&named).map(|_| ())
        });
    if let Err(why) = checked {
        let _ = std::fs::remove_file(&beside);
        return Err(said(format!(
            "rewritten, it would not read as naming this build's interface \
             ({why}), so it is left as it was"
        )));
    }
    if let Ok(meta) = std::fs::metadata(path) {
        let _ = std::fs::set_permissions(&beside, meta.permissions());
    }
    std::fs::rename(&beside, path)
        .map_err(|e| said(format!("cannot be replaced: {e}")))?;
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OURS: &str = "0123456789abcdef0123456789abcdef";

    /// Only the value changes, and its quotes and the comment after it stay.
    #[test]
    fn a_yaml_value_is_rewritten_and_nothing_else() {
        let text = "# a vehicle\ntcspecial:\n  gateway: \"radio\"\n  \
                    interface: \"ad9aac4dd5a6d88dd5e78824ba8a83ac\"  # set\n  \
                    tcs_cmd:\n    recv: \"0.0.0.0:1\"\nvehicle: []\n";
        let (new, set) = set_in_text(text, false, OURS).unwrap();
        assert_eq!(
            set,
            Set::Changed("ad9aac4dd5a6d88dd5e78824ba8a83ac".to_string())
        );
        assert_eq!(new, text.replace("ad9aac4dd5a6d88dd5e78824ba8a83ac", OURS));
        assert_eq!(set_in_text(&new, false, OURS).unwrap().1, Set::Already);

        // A nested `interface` is another attribute's, and not touched.
        let nested = "tcspecial:\n  gateway: radio\n  beacon:\n    \
                      interface: lo\n";
        let (new, set) = set_in_text(nested, false, OURS).unwrap();
        assert_eq!(set, Set::Added);
        assert_eq!(
            new,
            format!(
                "tcspecial:\n  gateway: radio\n  interface: \"{OURS}\"\n  \
                 beacon:\n    interface: lo\n"
            )
        );

        let bare = "tcspecial:\n  interface: 0123\n";
        let (new, _) = set_in_text(bare, false, OURS).unwrap();
        assert_eq!(new, format!("tcspecial:\n  interface: \"{OURS}\"\n"));

        assert!(set_in_text("vehicle: []\n", false, OURS).is_err());
        assert!(set_in_text("tcspecial: { gateway: radio }\n", false, OURS)
            .unwrap_err()
            .contains("one line"));
    }

    /// The same in XML: the element's text, or a new element where there is
    /// none, indented as its neighbours.
    #[test]
    fn an_xml_value_is_rewritten_and_nothing_else() {
        let text = "<configuration>\n    <tcspecial>\n        \
                    <gateway>radio</gateway>\n        <interface>old\
                    </interface>\n    </tcspecial>\n</configuration>\n";
        let (new, set) = set_in_text(text, true, OURS).unwrap();
        assert_eq!(set, Set::Changed("old".to_string()));
        assert_eq!(new, text.replace(">old<", &format!(">{OURS}<")));
        assert_eq!(set_in_text(&new, true, OURS).unwrap().1, Set::Already);

        let without = "<configuration>\n    <tcspecial>\n        \
                       <gateway>radio</gateway>\n        <state_file>s\
                       </state_file>\n    </tcspecial>\n</configuration>\n";
        let (new, set) = set_in_text(without, true, OURS).unwrap();
        assert_eq!(set, Set::Added);
        assert_eq!(
            new,
            without.replace(
                "</gateway>",
                &format!("</gateway>\n        <interface>{OURS}</interface>")
            )
        );
    }

    /// A file is rewritten in place, read back first, and a second time
    /// changes nothing.
    #[test]
    fn a_file_is_set_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.yaml");
        let example = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../examples/ping/ping.yaml");
        let good = std::fs::read_to_string(example).unwrap();
        let ours = InterfaceDigest::of_this_build().to_string();
        let stale = good.replace(&ours, OURS);
        std::fs::write(&path, &stale).unwrap();
        assert_eq!(set_in_file(&path).unwrap(), Set::Changed(OURS.to_string()));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), good);
        assert_eq!(set_in_file(&path).unwrap(), Set::Already);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
