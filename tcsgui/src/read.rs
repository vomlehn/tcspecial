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
            // Where it was drawn, from the editor's own section at the end of
            // the file: a node entry says what the vehicle carries, and this
            // says where somebody put the box.
            at: graph
                .placed
                .get(&node.name)
                .map_or((0, 0), |at| (at.x, at.y)),
            home,
            comment: said.nodes.get(&node.name).cloned(),
        });
    }

    for link in &graph.links {
        // One document link per destination: a link to three places is three
        // lines on the canvas, and each is drawn and deleted on its own.
        for destination in link.to.each() {
            let Destination { node, input } = Destination::parse(destination);
            // The leaving end, split from the terminal it names: everything in
            // the editor that looks a link's source up looks up a node.
            let origin = link.origin();
            let from = Qualified::parse(&origin.node);
            let to = Qualified::parse(&node);
            let mut line = Link::joining(
                &from.absolute(),
                origin.input.as_deref(),
                &to.absolute(),
                input.as_deref(),
            );
            // What somebody wrote above the attribute this link is written
            // under, which is a note about the connection: see Said::links.
            line.comment = link.comment.clone().or_else(|| {
                origin.input.as_ref().and_then(|terminal| {
                    said.links
                        .get(&(origin.node.clone(), terminal.clone()))
                        .cloned()
                })
            });
            document.links.push(line);
        }
    }

    // What the editor was told not to raise: the other half of its own
    // section, read back so that a decision survives being closed.
    document.aside = graph.set_aside.clone();

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
    /// By the node a link leaves and the data source it leaves by.
    ///
    /// Which is where a link is written now: an attribute of its own node,
    /// named after the terminal. So a comment above `payload-out:` is about the
    /// links that attribute makes -- all of them, where it names several
    /// destinations, because one line carries one comment.
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
    // Which node's entry the scan is inside, for the links written on it.
    let mut inside: Option<String> = None;
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

        // The node an attribute belongs to, whether or not anything is
        // waiting: a comment three attributes further down is still about this
        // node's link.
        if let Some(name) = node_named(trimmed, xml) {
            inside = Some(name);
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
        } else if let Some(terminal) = source_named(trimmed, xml) {
            // A link, which is an attribute of the node this entry is about.
            if let Some(node) = &inside {
                said.links.insert((node.clone(), terminal), comment);
            }
        } else if let Some(sets) = command_in(trimmed, xml, &mut lines) {
            said.commands.insert(sets, comment);
        }
    }

    said
}

/// Whether this XML line opens an entry whose name is on the next line.
///
/// The three node sections are the ones: a node's comment sits above its
/// opening tag and the name is inside it. `<commands>` is matched by its
/// opening tag instead, since what identifies one of those is its children
/// rather than a name of its own.
fn line_opens_an_entry(line: &str) -> bool {
    matches!(line, "<moc>" | "<vehicle>" | "<payloads>")
}

/// Which data source this line writes, if it writes one.
///
/// `payload-out: k_fore` in YAML and `<payload-out>k_fore</payload-out>` in XML -- and the
/// list form too, a comment above `payload-out: [a, b]` being about both of them.
/// The two names are the schema's: see `tcslibgs::graph::terminals`.
fn source_named(line: &str, xml: bool) -> Option<String> {
    for terminal in ["payload-out", "moc-out"] {
        let is = if xml {
            line.starts_with(&format!("<{terminal}>"))
        } else {
            line.starts_with(&format!("{terminal}:"))
        };
        if is {
            return Some(terminal.to_string());
        }
    }
    None
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

                // Everything comes back as it went out, with one exception
                // the XML spelling cannot avoid: a comment may not contain two
                // hyphens in a row, there being no escape for the sequence
                // that ends one, so a run of them is folded to a single
                // hyphen on the way out. See `document::comment_safe`.
                let mut written = written.clone();
                if name.ends_with(".xml") {
                    written.comment = written.comment.as_deref().map(|comment| {
                        comment
                            .lines()
                            .map(crate::document::comment_safe)
                            .collect::<Vec<String>>()
                            .join("\n")
                    });
                }
                assert_eq!(found, &written, "{name}: {}", written.name);
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

    /// A link that names its terminals is written and read back as one.
    ///
    /// The `null` node's case, and the reason a terminal is written into a
    /// file at all: a node with two data sources and two data sinks has to say
    /// which stream a link carries. Both spellings, because the terminal goes
    /// into the `from` of the link and each format writes that its own way --
    /// and a terminal that survived the YAML and not the XML would be a file
    /// that changed meaning by being saved under another name.
    #[test]
    fn a_link_that_names_its_terminals_is_read_back() {
        use crate::journal::{Edit, Link};

        let where_ = tempfile::tempdir().expect("a directory");
        let mut document = Document::new();
        for (name, node_type) in [("sensor", "i2c"), ("through", "null"), ("ground", "moc")] {
            document.apply(&Edit::AddNode {
                name: name.into(),
                node_type: node_type.into(),
                home: None,
            });
        }
        // Both streams through the null node, each saying which terminal it
        // leaves by and which it arrives at.
        // Every link names the data source it leaves by, that being the
        // attribute it is written under -- and the sink it arrives at where
        // the type has more than one, which is what the null node is for.
        for link in [
            Link::joining("sensor", Some("payload-out"), "through", Some("payload-in")),
            Link::joining("through", Some("payload-out"), "ground", None),
            Link::joining("ground", Some("moc-out"), "through", Some("moc-in")),
            Link::joining("through", Some("moc-out"), "sensor", None),
        ] {
            document.apply(&Edit::AddLink { link });
        }

        for (name, text) in [
            ("through.yaml", document.to_yaml()),
            ("through.xml", document.to_xml()),
        ] {
            let path = where_.path().join(name);
            std::fs::write(&path, &text).expect("the file is written");
            let read = read(&path).unwrap_or_else(|e| panic!("{name}: {e}"));

            assert_eq!(read.links.len(), document.links.len(), "{name}");
            for written in &document.links {
                assert!(
                    read.links.contains(written),
                    "{name}: {written:?} was not read back from\n{text}"
                );
            }
        }
    }

    /// The shipped examples open as what they say.
    #[test]
    fn the_shipped_examples_open() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

        // The example is what the editor opens by default and what somebody
        // draws their own graph in, so what it holds is whatever they last
        // drew -- including nothing at all, which is a configuration somebody
        // emptied and a state the editor starts new ones in. What is asked of
        // it is what is true however it is drawn: that both spellings read,
        // that they say the same thing, and that neither is faulty. The worked example is
        // the one with contents worth asserting, and
        // `the_worked_example_is_the_configuration_it_describes` asserts them.
        for name in ["example", "tests/manual/thermostat"] {
            let mut read_as: Vec<(usize, usize)> = Vec::new();
            for extension in ["yaml", "xml"] {
                let path = root.join(format!("{name}.{extension}"));
                let document = read(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
                read_as.push((document.nodes.len(), document.links.len()));
            }
            assert_eq!(
                read_as[0], read_as[1],
                "{name}: the two spellings read as different configurations"
            );
        }

        // And the worked example is sound, which the default file is not
        // asked to be: a fault drawn in `example.*` is somebody's work in
        // progress, and the editor's business is to say so rather than a
        // test's to refuse it.
        for extension in ["yaml", "xml"] {
            let path = root.join(format!("tests/manual/thermostat.{extension}"));
            let document = read(&path).expect("the worked example");
            assert!(
                document.faults().is_empty(),
                "thermostat.{extension}: {:?}",
                document.faults()
            );
        }
    }

    /// The worked example is the configuration it says it is.
    ///
    /// Contents rather than invariants, which is the difference between this
    /// and the example above: `tests/manual/thermostat.*` is the shipped
    /// worked example and nobody edits it by hand, where `example.*` is what
    /// the editor opens by default and holds whatever was last drawn in it.
    #[test]
    fn the_worked_example_is_the_configuration_it_describes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

        for extension in ["yaml", "xml"] {
            let path = root.join(format!("tests/manual/thermostat.{extension}"));
            let document = read(&path).expect("the worked example");
            assert_eq!(document.nodes.len(), 11, "thermostat.{extension}");
            assert_eq!(document.links.len(), 16, "thermostat.{extension}");
        }
    }

    /// The shipped example is one configuration in two spellings.
    ///
    /// `example.xml` is written from `example.yaml` rather than kept beside it
    /// by hand: the YAML is the one a person edits -- it is the one carrying
    /// the prose -- and the XML is the same configuration in the other
    /// language, which is what the XSD is validated against and what
    /// `tcslibgs` reads to prove its two readers agree. Two files kept in step
    /// by hand are two files that drift.
    ///
    /// Set TCSGUI_WRITE_EXAMPLE to write it out after a deliberate change to
    /// the YAML, as `tests/manual/thermostat.*` is written.
    #[test]
    fn the_shipped_example_is_the_same_in_both_spellings() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let document = read(&root.join("example.yaml")).expect("the example opens");
        let path = root.join("example.xml");
        // Written for the place it is going, which is what decides how it
        // names its schema: see `nodes::schema_reference`.
        let written = document.to_text(&path).expect("XML is a format this writes");

        if std::env::var("TCSGUI_WRITE_EXAMPLE").is_ok() {
            std::fs::write(&path, &written).expect("the example is writable");
            return;
        }

        let shipped = std::fs::read_to_string(&path).expect("the shipped XML");
        assert_eq!(
            shipped,
            written,
            "{} is not example.yaml in the other language any more. Run the \
             tests with TCSGUI_WRITE_EXAMPLE=1 to write it out",
            path.display()
        );
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
