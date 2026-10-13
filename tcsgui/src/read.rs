//! Reading a configuration into a document.
//!
//! The other half of `document`'s writers, and it was missing: tcsgui wrote
//! configurations and never read one, so opening a file that already existed
//! showed an empty canvas -- and the first edit wrote that emptiness over the
//! file. An editor that cannot open what it writes is a program for making
//! configurations once.
//!
//! The structure comes from `tcslibgs::graph::Graph`, which is the one reader
//! of the version 2 schema: it reads YAML and XML, follows `include`, and has
//! the rules about what a node may say. Nothing here parses a configuration a
//! second time.
//!
//! # The comments
//!
//! Those do not come from the graph, and they are the reason this file is
//! longer than a mapping. A comment is written as a comment -- `#` above the
//! entry it belongs to, or `<!-- -->` -- because that is what somebody hand
//! editing the file expects to find, and a parser throws those away before
//! anything sees them. So they are read from the text: the lines immediately
//! above an entry belong to that entry.
//!
//! Lost comments would be the editor quietly deleting the sentence that says
//! why a sensor is at address 0x49 -- the one thing in a configuration that
//! cannot be worked out again from what is left.

use std::collections::BTreeMap;
use std::path::Path;

use tcslibgs::graph::{Destination, Graph};

use crate::document::{Command, Document, Node};
use crate::journal::Link;
use crate::subdiagram::Qualified;

/// Read the configuration at `path`, with everything it includes.
///
/// A file that is not there is not an error: a name that does not exist yet is
/// how a new configuration is begun, and an empty document is what that is.
pub fn read(path: &Path) -> Result<Document, String> {
    if !path.exists() {
        return Ok(Document::default());
    }

    let graph = Graph::load(path).map_err(|e| format!("{e}"))?;
    let mut document = Document {
        description: graph.description.clone(),
        ..Document::default()
    };

    // What tcspecial is configured with, as the words the file holds. The
    // reader is deliberately not strict about it -- see
    // tcslibgs::graph::parameters -- so a section somebody left halfway
    // through opens, says what is missing, and can be put right.
    if let Some(said) = &graph.tcspecial {
        // Flat here, a subsection's attribute written `parent.child`: a field
        // in a window is one name, and so is a complaint about it. The
        // writers put the subsections back.
        document.tcspecial = tcslibgs::graph::parameters::flatten(said)
            .into_iter()
            .map(|(attribute, value)| (attribute, value.as_str().to_string()))
            .collect();
    }

    for group in &graph.groups {
        document
            .groups
            .insert(group.name.clone(), group.interval.clone());
    }

    // Which file each node came from, which is what says whether it is in the
    // top diagram or in a subdiagram: a node from an included file belongs to
    // that file's page, under the file's own stem.
    let home_of = |name: &str| -> Option<String> {
        let from = graph.homes.get(name)?;
        if from == path {
            return None;
        }
        from.file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::to_string)
    };

    let said = comments_of(path);

    for node in &graph.nodes {
        let home = home_of(&node.name);

        // The group is a field of the schema and an attribute here: the
        // editor's panel shows a node's words in one list, and which of them
        // the reader happens to declare is not a distinction a person editing
        // one has any use for.
        let mut attributes: BTreeMap<String, String> = node
            .attributes
            .iter()
            .map(|(attribute, value)| (attribute.clone(), value.as_str().to_string()))
            .collect();
        if let Some(group) = &node.group {
            attributes.insert("group".to_string(), group.clone());
        }

        document.nodes.push(Node {
            name: Qualified::of(&node.name, &home).absolute(),
            node_type: node.node_type.clone(),
            attributes,
            at: node.editor.as_ref().map_or((0, 0), |at| (at.x, at.y)),
            home,
            comment: said.nodes.get(&node.name).cloned(),
        });
    }

    for link in &graph.links {
        // One document link per destination: a link to three places is three
        // lines on the canvas, and each is drawn and deleted on its own.
        for destination in link.to.each() {
            let Destination { node, input } = Destination::parse(destination);
            let from = Qualified::parse(&link.from);
            let to = Qualified::parse(&node);
            let mut line = Link::new(&from.absolute(), &to.absolute(), input.as_deref());
            line.comment = link
                .comment
                .clone()
                .or_else(|| said.links.get(&(link.from.clone(), node.clone())).cloned());
            document.links.push(line);
        }
    }

    for binding in &graph.commands {
        document.commands.push(Command {
            from: binding.from.clone(),
            sets: binding.sets.clone(),
            comment: said.commands.get(&binding.sets).cloned(),
        });
    }

    Ok(document)
}

/// What a file says about its own entries, by the thing each is about.
#[derive(Debug, Default)]
struct Said {
    /// By node name.
    nodes: BTreeMap<String, String>,
    /// By the two ends of the link, as the file wrote them.
    links: BTreeMap<(String, String), String>,
    /// By what the binding sets.
    commands: BTreeMap<String, String>,
}

/// Every comment in the file, attached to the entry below it.
///
/// Read from the text because a comment is not in the parsed document: the
/// rule is the one the writers follow, which is that an entry's comment is the
/// run of comment lines immediately above it. A comment with a blank line
/// under it belongs to nothing and is left where it is.
///
/// Both languages, since a configuration may be written in either, and the
/// scan is the same shape for both: find the lines that are comments, find the
/// entry that follows, and pair them.
fn comments_of(path: &Path) -> Said {
    let mut said = Said::default();
    let Ok(text) = std::fs::read_to_string(path) else {
        return said;
    };
    let xml = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("xml"));

    let mut waiting: Vec<String> = Vec::new();
    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        // A comment, which is held until the thing it is about turns up.
        if let Some(comment) = comment_in(trimmed, xml) {
            waiting.push(comment);
            continue;
        }

        // A blank line ends a comment's claim on whatever comes next: a
        // heading at the top of a file is not a note about the first node.
        if trimmed.is_empty() {
            waiting.clear();
            continue;
        }

        if waiting.is_empty() {
            continue;
        }

        // An XML entry opens with a tag and names itself on the next line, so
        // the opening tag passes a comment through to the line that says what
        // the entry is. Nothing else does: a comment that reached a line it
        // is not about would be a comment moved to another node.
        if xml && line_opens_an_entry(trimmed) {
            continue;
        }

        let comment = waiting.join("\n");
        waiting.clear();

        // What the comment was about. A node and a command are named by the
        // entry itself; a link is named by its two ends, which in YAML are on
        // the one line and in XML are the two lines after it.
        if let Some(name) = node_named(trimmed, xml) {
            said.nodes.insert(name, comment);
        } else if let Some((from, to)) = link_in(trimmed, xml, &mut lines) {
            said.links.insert((from, to), comment);
        } else if let Some(sets) = command_in(trimmed, xml, &mut lines) {
            said.commands.insert(sets, comment);
        }
    }

    said
}

/// Whether this XML line opens an entry whose name is on the next line.
///
/// `<nodes>` is the one: a node's comment sits above the tag and the name is
/// inside it. `<links>` and `<commands>` are matched by their opening tags
/// instead, since what identifies one of those is two of its children rather
/// than a name of its own.
fn line_opens_an_entry(line: &str) -> bool {
    line == "<nodes>"
}

/// The text of a comment line, if this line is one.
fn comment_in(line: &str, xml: bool) -> Option<String> {
    if xml {
        let body = line.strip_prefix("<!--")?.strip_suffix("-->")?;
        Some(body.trim().to_string())
    } else {
        let body = line.strip_prefix('#')?;
        Some(body.trim().to_string())
    }
}

/// The name of the node this line starts, if it starts one.
fn node_named(line: &str, xml: bool) -> Option<String> {
    if xml {
        // `<nodes>` opens an entry and `<name>` is its first child, so the
        // name is on the next line -- but the comment is attached to the
        // entry, so the opening tag is what is matched and the name is found
        // by the caller's next line. Handled by matching the name element
        // itself, which the writers put first.
        let name = line.strip_prefix("<name>")?.strip_suffix("</name>")?;
        Some(name.to_string())
    } else {
        let name = line.strip_prefix("- name: ")?;
        Some(unquoted(name))
    }
}

/// The two ends of the link this line starts, if it starts one.
fn link_in<'a, I: Iterator<Item = &'a str>>(
    line: &str,
    xml: bool,
    lines: &mut std::iter::Peekable<I>,
) -> Option<(String, String)> {
    if xml {
        if line != "<links>" {
            return None;
        }
        let from = element(lines.next()?, "from")?;
        let to = element(lines.next()?, "to")?;
        Some((from, bare(&to)))
    } else {
        // `- { from: a, to: b }` or `- { from: a, to: b.in0 }`.
        let inside = line.strip_prefix("- {")?.strip_suffix('}')?;
        let mut from = None;
        let mut to = None;
        for part in inside.split(',') {
            let (key, value) = part.split_once(':')?;
            match key.trim() {
                "from" => from = Some(unquoted(value.trim())),
                "to" => to = Some(unquoted(value.trim())),
                _ => {}
            }
        }
        Some((from?, bare(&to?)))
    }
}

/// What the command binding this line starts sets, if it starts one.
fn command_in<'a, I: Iterator<Item = &'a str>>(
    line: &str,
    xml: bool,
    lines: &mut std::iter::Peekable<I>,
) -> Option<String> {
    if xml {
        if line != "<commands>" {
            return None;
        }
        let _from = element(lines.next()?, "from")?;
        element(lines.next()?, "sets")
    } else {
        let inside = line.strip_prefix("- {")?.strip_suffix('}')?;
        for part in inside.split(',') {
            let (key, value) = part.split_once(':')?;
            if key.trim() == "sets" {
                return Some(unquoted(value.trim()));
            }
        }
        None
    }
}

/// The text of a one-line XML element, if this line is one.
fn element(line: &str, name: &str) -> Option<String> {
    let line = line.trim();
    let body = line
        .strip_prefix(&format!("<{name}>"))?
        .strip_suffix(&format!("</{name}>"))?;
    Some(unescape(body))
}

/// A destination without the input it names: the node is what a comment is
/// keyed by, an input being part of the one link.
fn bare(destination: &str) -> String {
    destination
        .split_once('.')
        .map_or(destination.to_string(), |(node, _)| node.to_string())
}

/// A value without the quotes a writer puts round it.
fn unquoted(value: &str) -> String {
    let value = value.trim();
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .map(|v| v.replace("\\\"", "\"").replace("\\\\", "\\"))
        .unwrap_or_else(|| value.to_string())
}

/// XML's five, back the way they were written.
fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::thermostat_example;

    /// What the editor writes, it reads back.
    ///
    /// The round trip, on the shipped example, in both formats: a document
    /// written out and read in is the document that was written. This is the
    /// whole claim -- an editor that opened its own file and showed something
    /// else would be worse than one that could not open it at all.
    #[test]
    fn what_is_written_is_read_back() {
        let where_ = tempfile::tempdir().expect("a directory");
        let document = thermostat_example();

        for (name, text) in [
            ("thermostat.yaml", document.to_yaml()),
            ("thermostat.xml", document.to_xml()),
        ] {
            let path = where_.path().join(name);
            std::fs::write(&path, &text).expect("the file is written");

            let read = read(&path).unwrap_or_else(|e| panic!("{name}: {e}"));

            assert_eq!(read.description, document.description, "{name}: description");
            assert_eq!(read.groups, document.groups, "{name}: groups");
            assert_eq!(read.commands, document.commands, "{name}: commands");
            assert_eq!(
                read.nodes.len(),
                document.nodes.len(),
                "{name}: {} nodes read, {} written",
                read.nodes.len(),
                document.nodes.len()
            );
            for written in &document.nodes {
                let found = read
                    .nodes
                    .iter()
                    .find(|n| n.name == written.name)
                    .unwrap_or_else(|| panic!("{name}: {} was not read back", written.name));
                assert_eq!(found, written, "{name}: {}", written.name);
            }
            assert_eq!(read.links.len(), document.links.len(), "{name}: links");
            for written in &document.links {
                assert!(
                    read.links.contains(written),
                    "{name}: {written:?} was not read back"
                );
            }
        }
    }

    /// The shipped examples open as what they say.
    #[test]
    fn the_shipped_examples_open() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

        for (name, nodes, links) in [("example", 2, 2), ("tests/manual/thermostat", 11, 16)] {
            for extension in ["yaml", "xml"] {
                let path = root.join(format!("{name}.{extension}"));
                let document = read(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
                assert_eq!(
                    document.nodes.len(),
                    nodes,
                    "{name}.{extension} has {} nodes",
                    document.nodes.len()
                );
                assert_eq!(
                    document.links.len(),
                    links,
                    "{name}.{extension} has {} links",
                    document.links.len()
                );
                assert!(
                    document.faults().is_empty(),
                    "{name}.{extension}: {:?}",
                    document.faults()
                );
            }
        }
    }

    /// A comment belongs to the entry under it, in either language.
    #[test]
    fn a_comment_is_read_back_with_what_it_is_about() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

        for extension in ["yaml", "xml"] {
            let path = root.join(format!("tests/manual/thermostat.{extension}"));
            let document = read(&path).expect("the example opens");

            let sensor = document
                .nodes
                .iter()
                .find(|n| n.name == "temp_fore")
                .expect("the forward sensor");
            assert_eq!(
                sensor.comment.as_deref(),
                Some("Forward bulkhead. Two bytes, signed, big endian."),
                "{extension}"
            );

            // Two lines, which is how the example writes the bay sensor's.
            let bay = document
                .nodes
                .iter()
                .find(|n| n.name == "temp_bay")
                .expect("the bay sensor");
            assert!(
                bay.comment
                    .as_deref()
                    .is_some_and(|said| said.contains("Equipment bay") && said.contains("own conversion")),
                "{extension}: {:?}",
                bay.comment
            );

            // A link's, and a command binding's.
            let heaters = document
                .links
                .iter()
                .find(|l| l.from == "thermostat" && l.to == "heaters")
                .expect("the link to the heaters");
            assert_eq!(
                heaters.comment.as_deref(),
                Some("asserted below the setpoint, unasserted above it"),
                "{extension}"
            );
            assert_eq!(
                document.commands[0].comment.as_deref(),
                Some("the cabin temperature the heaters are held at, in kelvin"),
                "{extension}"
            );

            // And a node with nothing said about it says nothing.
            let moc = document
                .nodes
                .iter()
                .find(|n| n.name == "moc")
                .expect("the ground link");
            assert_eq!(moc.comment, None, "{extension}");
        }
    }

    /// A file that is not there opens as a new configuration.
    #[test]
    fn a_name_that_does_not_exist_yet_is_a_new_document() {
        let where_ = tempfile::tempdir().expect("a directory");
        let document = read(&where_.path().join("nothing-here.yaml")).expect("a new document");
        assert!(document.nodes.is_empty() && document.links.is_empty());
    }

    /// A file that is not a configuration is reported rather than opened as
    /// an empty one.
    #[test]
    fn a_file_that_is_not_a_configuration_is_reported() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("notes.yaml");
        std::fs::write(&path, "this is not a configuration\n").expect("written");

        let said = read(&path).expect_err("a file that is not one");
        assert!(!said.is_empty(), "the complaint should say something");
    }
}
