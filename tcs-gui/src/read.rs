//! Reading a configuration into a document.
//!
//! The other half of `document`'s writers, and it was missing: tcs-gui wrote
//! configurations and never read one, so opening a file that already existed
//! showed an empty canvas -- and the first edit wrote that emptiness over the
//! file. An editor that cannot open what it writes is a program for making
//! configurations once.
//!
//! The structure comes from `tcslib_protocol::graph::Graph`, which is the one
//! reader of the schema: it reads YAML and XML, follows `include`, and has the
//! rules about what a node may say. Nothing here parses a configuration a
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

use tcslib_protocol::graph::{Destination, Graph, Section};

use crate::document::{Command, Document, Heading, Node, Remark};
use crate::journal::Link;
use crate::subdiagram::Qualified;

/// Reads the configuration at `path`, with everything it includes.
///
/// A file that is not there is not an error: a name that does not exist yet is
/// how a new configuration is begun, and an empty document is what that is.
///
/// # Arguments
///
/// * `path` - The top configuration file, YAML or XML by its extension.
///
/// # Returns
///
/// The document, with the nodes of every subdiagram it includes and the
/// comments above each entry; an empty document for a file that is not there.
///
/// # Errors
///
/// The loader's message, as text, for a file that cannot be read, parsed or
/// included.
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
    // tcslib_protocol::graph::parameters -- so a section somebody left halfway
    // through opens, says what is missing, and can be put right.
    if let Some(said) = &graph.tcspecial {
        // Flat here, a subsection's attribute written `parent.child`: a field
        // in a window is one name, and so is a complaint about it. The
        // writers put the subsections back.
        document.tcspecial = tcslib_protocol::graph::parameters::flatten(said)
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
    // that file's page, under the name the include list gives the file.
    let home_of = |name: &str| -> Option<String> {
        graph.include_name(graph.homes.get(name)?)
    };

    // And which file each group and command binding is declared in, so that
    // a save writes each back where it was rather than into the top file.
    // The graph merges them, so each included file is read again on its own
    // for what it declares.
    let mut binding_homes: BTreeMap<(String, String), String> = BTreeMap::new();
    for file in graph.files.iter().skip(1) {
        let Some(name) = graph.include_name(file) else {
            continue;
        };
        let declared = tcslib_protocol::graph::read_one(file)
            .map_err(|e| format!("{e}"))?;
        if let Some(exports) = &declared.exports {
            document.exports.insert(name.clone(), exports.clone());
        }
        for group in &declared.payload_groups {
            document
                .group_homes
                .insert(group.name.clone(), name.clone());
        }
        for binding in &declared.commands {
            binding_homes.insert(
                (binding.from.clone(), binding.sets.clone()),
                name.clone(),
            );
        }
    }

    // The comments, from every file: a node's and a link's are found by the
    // node's name, which is one node's in the whole configuration, and what
    // is said about a file's structure is kept with that file.
    let mut said = comments_of(path);
    let mut remarks: Vec<(Heading, String, Option<String>)> = said
        .remarks
        .drain(..)
        .map(|(heading, text)| (heading, text, None))
        .collect();
    for file in graph.files.iter().skip(1) {
        let home = graph.include_name(file);
        let other = comments_of(file);
        said.nodes.extend(other.nodes);
        said.links.extend(other.links);
        said.commands.extend(other.commands);
        remarks.extend(
            other
                .remarks
                .into_iter()
                .map(|(heading, text)| (heading, text, home.clone())),
        );
    }

    for node in &graph.nodes {
        let home = home_of(&node.name);

        // The group is a field of the schema and an attribute here: the
        // editor's panel shows a node's words in one list, and which of them
        // the reader happens to declare is not a distinction a person editing
        // one has any use for.
        let mut attributes: BTreeMap<String, String> = node
            .attributes
            .iter()
            .map(|(attribute, value)| {
                (attribute.clone(), value.as_str().to_string())
            })
            .collect();
        if let Some(group) = &node.group {
            attributes.insert("group".to_string(), group.clone());
        }
        // The names a line may arrive by that no line does, which the panel
        // shows with the rest of the node's words and the writers put back in
        // the sink's list. A name a line arrives by is the line's -- see
        // `Link::name` -- and a vote's world-in is its inputs, which it holds
        // as that already, so neither is said twice.
        if let Some(sinks) = graph.named.sinks.get(&node.name) {
            let votes = attributes.contains_key("inputs");
            let used: Vec<&str> = graph
                .named
                .written
                .values()
                .filter_map(|written| written.split_once('.'))
                .filter(|(to, _)| *to == node.name)
                .map(|(_, name)| name)
                .collect();
            for (sink, names) in
                [("world-in", &sinks.world_in), ("moc-in", &sinks.moc_in)]
            {
                let spare: Vec<&str> = names
                    .iter()
                    .map(String::as_str)
                    .filter(|name| !used.contains(name))
                    .collect();
                if spare.is_empty() || (votes && sink == "world-in") {
                    continue;
                }
                attributes.insert(sink.to_string(), spare.join(", "));
            }
        }

        document.nodes.push(Node {
            // Its own name: the home says which page it is on, and the two
            // together are the name the editor holds it by elsewhere.
            name: node.name.clone(),
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

    // And a box in the top diagram for each file it includes, which is what
    // opens that file's page: named by its include, and drawn where the top
    // file's own section put it.
    for include in graph.included() {
        let name = include.name;
        document.nodes.push(Node {
            at: graph.placed.get(&name).map_or((0, 0), |at| (at.x, at.y)),
            name,
            node_type: "subdiagram".to_string(),
            // Where its file is, which is what the include says beside the
            // name: see `Document::include_path`.
            attributes: BTreeMap::from([("path".to_string(), include.path)]),
            home: None,
            comment: None,
        });
    }

    for link in &graph.links {
        // One document link per destination: a link to three places is three
        // lines on the canvas, and each is drawn and deleted on its own.
        for destination in link.to.each() {
            let Destination { node, input } = Destination::parse(destination);
            // The leaving end, split from the terminal it names: everything in
            // the editor that looks a link's source up looks up a node.
            // Each end in the file its node is in, which is what the editor
            // holds a name by: a link names a node by its own name whichever
            // file either is in.
            let origin = link.origin();
            let from = Qualified::of(&origin.node, &home_of(&origin.node));
            let to = Qualified::of(&node, &home_of(&node));
            let mut line = Link::joining(
                &from.absolute(),
                origin.input.as_deref(),
                &to.absolute(),
                input.as_deref(),
            );
            // The name it arrives by, where the file chose one of its own, so
            // that a save writes it back as it was; the one the editor would
            // give it is left to be given again. See `Link::name`.
            let given = line.arrives_by();
            line.name = graph
                .named
                .written
                .get(&(link.from.clone(), destination.to_string()))
                .and_then(|written| written.split_once('.'))
                .map(|(_, name)| name.to_string())
                .filter(|name| *name != given);
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
            home: binding_homes
                .get(&(binding.from.clone(), binding.sets.clone()))
                .cloned(),
        });
    }

    // What is said about each file's structure rather than its entries: the
    // file itself, a section, a type group, a line of the tcspecial section;
    // and only those whose subject was read, which is what the editor holds
    // them to from now on.
    document.remarks = remarks
        .into_iter()
        .map(|(heading, text, home)| Remark {
            home,
            heading,
            text,
        })
        .collect();
    document.tcspecial_comments = said.tcspecial;
    document.forget_lost_comments();

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
    /// named after the terminal. So a comment above `world-out:` is about the
    /// links that attribute makes -- all of them, where it names several
    /// destinations, because one line carries one comment.
    links: BTreeMap<(String, String), String>,
    /// By what the binding sets.
    commands: BTreeMap<String, String>,
    /// By the line of the file's structure each is above: see [`Heading`].
    remarks: Vec<(Heading, String)>,
    /// By the line of the `tcspecial` section each is above, named as
    /// `Document::tcspecial_comments` names it.
    tcspecial: BTreeMap<String, String>,
}

/// Where in a file's structure the scan in [`comments_of`] is.
///
/// What a line means depends on what it is inside: `recv: ...` is a line of
/// the `tcspecial` section in one place and nothing to do with it in another,
/// and the first `- type` of a section is where the section's own comment is.
#[derive(Debug, Default)]
struct Within {
    /// The node section the scan is in, as the file names it.
    section: Option<String>,
    /// Whether no type group of that section has been passed yet.
    first_type: bool,
    /// Whether the scan is in the `tcspecial` section.
    tcspecial: bool,
    /// How far the `tcspecial` section's own lines are indented, in YAML,
    /// once one has been seen: a line indented further is in a subsection.
    top: Option<usize>,
    /// The `tcspecial` subsection the scan is in.
    parent: Option<String>,
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
///
/// # Arguments
///
/// * `path` - The configuration file to scan, XML if its extension says so and
///            YAML otherwise.
///
/// # Returns
///
/// The comments found, by node, by link and by command; none for a file that
/// cannot be read.
fn comments_of(path: &Path) -> Said {
    let mut said = Said::default();
    let Ok(text) = std::fs::read_to_string(path) else {
        return said;
    };
    let xml = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("xtcs"));

    let mut waiting: Vec<String> = Vec::new();
    // Which node's entry the scan is inside, for the links written on it.
    let mut inside: Option<String> = None;
    let mut within = Within::default();
    let mut lines = text.lines().peekable();
    // Whether the file has begun to say anything, and in XML whether the
    // scan is inside the root element's opening tag, which is not saying
    // anything yet: a comment before then, with a blank line under it, is
    // the file's own -- see `Heading::File`.
    let mut begun = false;
    let mut opening = false;

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        // A comment, which is held until the thing it is about turns up.
        if let Some(comment) = comment_in(trimmed, xml) {
            waiting.push(comment);
            continue;
        }
        // An XML comment over several lines, which is how one written by
        // hand is usually laid out: each of its lines a line of the comment.
        if xml && trimmed.starts_with("<!--") {
            waiting.extend(comment_over_lines(trimmed, &mut lines));
            continue;
        }

        // A blank line ends a comment's claim on whatever comes next: a
        // heading at the top of a file is not a note about the first node.
        // It is the file's, unless it is the line that names the schema,
        // which is the writer's own.
        if trimmed.is_empty() {
            let header = waiting.join("\n");
            if !begun
                && !waiting.is_empty()
                && !header.starts_with("yaml-language-server")
            {
                said.remarks.push((Heading::File, header));
            }
            waiting.clear();
            continue;
        }

        // The XML declaration and the root element's opening tag, over
        // however many lines it takes, say nothing yet.
        if xml && !begun {
            if trimmed.starts_with("<?xml") {
                continue;
            }
            if trimmed.starts_with("<configuration") {
                opening = true;
            }
            if opening {
                opening = !trimmed.ends_with('>');
                continue;
            }
        }
        begun = true;

        // The node an attribute belongs to, whether or not anything is
        // waiting: a comment three attributes further down is still about this
        // node's link.
        if let Some(name) = node_named(trimmed, xml) {
            inside = Some(name);
        }

        // And where in the file's structure this line is, likewise whether or
        // not anything is waiting: what a later comment is about depends on
        // it.
        let structural = structure_of(line, xml, &mut within);

        if waiting.is_empty() {
            continue;
        }

        let comment = waiting.join("\n");
        waiting.clear();

        // What the comment was about. A node and a command are named by the
        // entry itself; a link is named by its two ends, which in YAML are on
        // the one line and in XML are the two lines after it.
        if let Some(about) = structural {
            match about {
                Structure::Heading(heading) => {
                    said.remarks.push((heading, comment));
                }
                Structure::Tcspecial(key) => {
                    said.tcspecial.insert(key, comment);
                }
            }
        } else if let Some(name) = node_named(trimmed, xml) {
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

/// What a line of a file's structure is, for the comment above it.
#[derive(Debug)]
enum Structure {
    /// A section's line or a type group's: see [`Heading`].
    Heading(Heading),
    /// A line of the `tcspecial` section, by the attribute it states.
    Tcspecial(String),
}

/// Follows a file's structure past one line, and says which structural line
/// it is.
///
/// The three node sections, an included file's `exports` and the `tcspecial`
/// section are what is tracked:
/// a section's own line and its type groups' lines are where comments about
/// a section or a type are written, and the `tcspecial` section's lines carry
/// comments about tcspecial's settings. In YAML a section begins with its key
/// at the left margin and ends at the next; in XML it is the element.
///
/// # Arguments
///
/// * `line`   - The line, untrimmed: in YAML its indent says what it is in.
/// * `xml`    - Whether the file is XML rather than YAML.
/// * `within` - Where the scan was before this line, brought up to date.
///
/// # Returns
///
/// What the line is, if it is one a structural comment is written above;
/// `None` for anything else, a node's own line among them.
fn structure_of(
    line: &str,
    xml: bool,
    within: &mut Within,
) -> Option<Structure> {
    let trimmed = line.trim();
    // An included file's exports, whose comment is about the section as a
    // whole: nothing inside it is a structural line.
    let exports = if xml {
        trimmed == "<exports>"
    } else {
        trimmed == "exports:" && !line.starts_with(' ')
    };
    if exports {
        *within = Within::default();
        return Some(Structure::Heading(Heading::Section(
            "exports".to_string(),
        )));
    }
    let section = Section::ALL.into_iter().map(Section::name).find(|name| {
        if xml {
            trimmed == format!("<{name}>")
        } else {
            trimmed == format!("{name}:") && !line.starts_with(' ')
        }
    });
    if let Some(name) = section {
        *within = Within {
            section: Some(name.to_string()),
            first_type: true,
            ..Within::default()
        };
        return Some(Structure::Heading(Heading::Section(name.to_string())));
    }

    // The end of a section: the closing element, or in YAML any other key
    // at the margin.
    let ends = if xml {
        trimmed.starts_with("</")
            && (within.tcspecial && trimmed == "</tcspecial>"
                || within
                    .section
                    .as_deref()
                    .is_some_and(|name| trimmed == format!("</{name}>")))
    } else {
        !line.starts_with(' ') && !trimmed.starts_with('-')
    };
    if ends {
        *within = Within::default();
    }
    let opens = if xml {
        trimmed == "<tcspecial>"
    } else {
        trimmed == "tcspecial:" && !line.starts_with(' ')
    };
    if opens {
        within.tcspecial = true;
        return None;
    }

    if let Some(section) = within.section.clone() {
        let node_type = if xml {
            trimmed
                .strip_prefix("<type name=\"")
                .and_then(|rest| rest.split_once('"'))
                .map(|(name, _)| unescape(name))
        } else {
            trimmed
                .strip_prefix("- type ")
                .and_then(|rest| rest.strip_suffix(':'))
                .map(unquoted)
        }?;
        let heading = if within.first_type {
            Heading::Opening(section)
        } else {
            Heading::Type(node_type)
        };
        within.first_type = false;
        return Some(Structure::Heading(heading));
    }

    if within.tcspecial {
        return tcspecial_line(line, xml, within).map(Structure::Tcspecial);
    }
    None
}

/// Which attribute of the `tcspecial` section a line states, keeping track
/// of the subsection it is in.
///
/// # Arguments
///
/// * `line`   - The line, untrimmed, inside the `tcspecial` section.
/// * `xml`    - Whether the file is XML rather than YAML.
/// * `within` - Where the scan is in the section, brought up to date.
///
/// # Returns
///
/// The attribute as `Document::tcspecial_comments` names it -- `state_file`,
/// `beacon`, `beacon.recv` -- or `None` for a line that states none, such as
/// the end of a subsection.
fn tcspecial_line(
    line: &str,
    xml: bool,
    within: &mut Within,
) -> Option<String> {
    let trimmed = line.trim();
    let under = |within: &Within, name: &str| match &within.parent {
        Some(parent) => format!("{parent}.{name}"),
        None => name.to_string(),
    };
    if xml {
        if trimmed.starts_with("</") {
            within.parent = None;
            return None;
        }
        let rest = trimmed.strip_prefix('<')?;
        let (name, after) = rest.split_once('>')?;
        if after.is_empty() {
            // A subsection's opening, with its attributes on the lines below.
            within.parent = Some(name.to_string());
            return Some(name.to_string());
        }
        return Some(under(within, name));
    }

    let indent = line.len() - line.trim_start().len();
    let (name, value) = trimmed.split_once(':')?;
    let top = *within.top.get_or_insert(indent);
    if indent <= top {
        within.parent = value.trim().is_empty().then(|| name.to_string());
        return Some(name.to_string());
    }
    Some(under(within, name))
}

/// The lines of an XML comment that does not end on the line it begins on.
///
/// # Arguments
///
/// * `first` - The line the comment begins on, trimmed.
/// * `lines` - The lines after it, from which the rest of the comment is
///             taken, up to and including the line it ends on.
///
/// # Returns
///
/// Each line of the comment's text, trimmed, leaving out any with nothing
/// on them at its start and its end. Not unescaped: XML reads no entity
/// inside a comment.
fn comment_over_lines<'a, I: Iterator<Item = &'a str>>(
    first: &str,
    lines: &mut I,
) -> Vec<String> {
    let mut said = Vec::new();
    let mut now = first.trim_start_matches("<!--").to_string();
    loop {
        if let Some((body, _)) = now.split_once("-->") {
            said.push(body.trim().to_string());
            break;
        }
        said.push(now.trim().to_string());
        match lines.next() {
            Some(next) => now = next.to_string(),
            None => break,
        }
    }
    let first = said.iter().position(|line| !line.is_empty());
    let last = said.iter().rposition(|line| !line.is_empty());
    match (first, last) {
        (Some(first), Some(last)) => said[first..=last].to_vec(),
        _ => Vec::new(),
    }
}

/// Which data source this line writes, if it writes one.
///
/// `world-out: k_1` in YAML and `<world-out>k_1</world-out>` in XML --
/// and the list form too, a comment above `world-out: [a, b]` being about
/// both of them. The two names are the schema's: see
/// `tcslib_protocol::graph::terminals`.
///
/// # Arguments
///
/// * `line` - The line, trimmed.
/// * `xml`  - Whether the file is XML rather than YAML.
///
/// # Returns
///
/// The terminal's name, `world-out` or `moc-out`, or `None` if the line
/// writes neither.
fn source_named(line: &str, xml: bool) -> Option<String> {
    for terminal in ["world-out", "moc-out"] {
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
///
/// # Arguments
///
/// * `line` - The line, trimmed.
/// * `xml`  - Whether the file is XML, whose comments are `<!-- -->`, rather
///            than YAML, whose comments start with `#`.
///
/// # Returns
///
/// The comment's text, trimmed; or `None` if the line is not a comment. Not
/// unescaped in XML, which reads no entity inside a comment.
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
///
/// A node names itself on the line it starts on, by the word for its
/// section: `- payload: temp_1` in YAML and `<payload name="temp_1">` in XML,
/// as the writers write it. A `type` line above the nodes of a type starts
/// none, so a comment above it is about no node.
///
/// # Arguments
///
/// * `line` - The line, trimmed.
/// * `xml`  - Whether the file is XML rather than YAML.
///
/// # Returns
///
/// The node's name, unquoted, or `None` if the line does not start a node.
fn node_named(line: &str, xml: bool) -> Option<String> {
    Section::ALL.into_iter().find_map(|section| {
        let word = section.node_word();
        if xml {
            let rest = line.strip_prefix(&format!("<{word} name=\""))?;
            let (name, _) = rest.split_once('"')?;
            Some(unescape(name))
        } else {
            let name = line.strip_prefix(&format!("- {word}: "))?;
            Some(unquoted(name))
        }
    })
}

/// What the command binding this line starts sets, if it starts one.
///
/// # Arguments
///
/// * `line`  - The line, trimmed.
/// * `xml`   - Whether the file is XML rather than YAML.
/// * `lines` - The lines after it. For XML the `<from>` and `<sets>` lines that
///             follow `<commands>` are taken from it.
///
/// # Returns
///
/// What the binding sets, or `None` if the line does not start a binding.
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
///
/// # Arguments
///
/// * `line` - The line, which may have spaces round it.
/// * `name` - The element's tag name.
///
/// # Returns
///
/// The element's text, unescaped, or `None` if the line is not that element.
fn element(line: &str, name: &str) -> Option<String> {
    let line = line.trim();
    let body = line
        .strip_prefix(&format!("<{name}>"))?
        .strip_suffix(&format!("</{name}>"))?;
    Some(unescape(body))
}

/// A value without the quotes a writer puts round it.
///
/// # Arguments
///
/// * `value` - The value as written.
///
/// # Returns
///
/// The value trimmed, and if double-quoted, without its quotes and with `\"`
/// and `\\` unescaped.
fn unquoted(value: &str) -> String {
    let value = value.trim();
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .map(|v| v.replace("\\\"", "\"").replace("\\\\", "\\"))
        .unwrap_or_else(|| value.to_string())
}

/// XML's five, back the way they were written.
///
/// # Arguments
///
/// * `text` - XML character data.
///
/// # Returns
///
/// The text with `&lt;`, `&gt;`, `&quot;`, `&apos;` and `&amp;` replaced.
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
            ("thermostat.ytcs", document.to_yaml()),
            ("thermostat.xtcs", document.to_xml()),
        ] {
            let path = where_.path().join(name);
            std::fs::write(&path, &text).expect("the file is written");

            let read = read(&path).unwrap_or_else(|e| panic!("{name}: {e}"));

            assert_eq!(
                read.description, document.description,
                "{name}: description"
            );
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
                    .unwrap_or_else(|| {
                        panic!("{name}: {} was not read back", written.name)
                    });

                // Everything comes back as it went out, with one exception
                // the XML spelling cannot avoid: a comment may not contain two
                // hyphens in a row, there being no escape for the sequence
                // that ends one, so a run of them is folded to a single
                // hyphen on the way out. See `document::comment_safe`.
                let mut written = written.clone();
                if name.ends_with(".xtcs") {
                    written.comment =
                        written.comment.as_deref().map(|comment| {
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
    /// A tee's case, and the reason a terminal is written into a file at all:
    /// a node with two data sources has to say which stream a link out of it
    /// carries. Both spellings, because the terminal goes
    /// into the `from` of the link and each format writes that its own way --
    /// and a terminal that survived the YAML and not the XML would be a file
    /// that changed meaning by being saved under another name.
    #[test]
    fn a_link_that_names_its_terminals_is_read_back() {
        use crate::journal::{Edit, Link};

        let where_ = tempfile::tempdir().expect("a directory");
        let mut document = Document::new();
        for (name, node_type) in [
            ("sensor", "i2c"),
            ("through", "tee-payload"),
            ("ground", "gateway_radio"),
        ] {
            document.apply(&Edit::AddNode {
                name: name.into(),
                node_type: node_type.into(),
                home: None,
            });
        }
        // The sensor's readings through the tee, which sends them on both
        // ways: each link names the data source it leaves by, that being the
        // attribute it is written under.
        for link in [
            Link::joining("sensor", Some("world-out"), "through", None),
            Link::joining("through", Some("world-out"), "ground", None),
            Link::joining("through", Some("moc-out"), "sensor", None),
        ] {
            document.apply(&Edit::AddLink { link });
        }

        for (name, text) in [
            ("through.ytcs", document.to_yaml()),
            ("through.xtcs", document.to_xml()),
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

    /// An example that includes a file in another directory opens with that
    /// file as a subdiagram, under its include name, and saving it writes
    /// every file back to where it was, saying what it said: the include as
    /// it was written, the common file's nodes, group, binding and layout in
    /// the common file, and every comment above the line it was above.
    #[test]
    fn an_example_with_a_common_file_is_saved_as_it_was() {
        let examples =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
        for (example, common) in [
            ("thermostat", "thermostat-common/thermostat"),
            ("thermostat-yamcs", "thermostat-common/thermostat"),
            ("ping", "ping-common/ping"),
            ("ping-yamcs", "ping-common/ping"),
        ] {
            for extension in ["ytcs", "xtcs"] {
                let xml = extension == "xtcs";
                let name = format!("{example}/{example}.{extension}");
                let top = examples.join(&name);
                let document = read(&top).unwrap_or_else(|e| panic!("{e}"));
                // Named for its directory, and at its path.
                let include =
                    common.split('/').next().expect("a directory").to_string();
                assert_eq!(document.subdiagrams(), vec![include.clone()]);
                assert_eq!(
                    document.include_path(&include),
                    format!("../{common}")
                );
                assert!(
                    document.node(&include).is_some_and(|box_| {
                        box_.node_type == "subdiagram" && box_.at != (0, 0)
                    }),
                    "{name}: the common file's box, where it was drawn"
                );

                // The top file, its other spelling, and the common file in
                // this spelling and the other, written where they are.
                let files = document.files(&top).expect("a known format");
                let shared = format!("{common}.{extension}");
                let paths: Vec<&Path> =
                    files.iter().map(|(path, _)| path.as_path()).collect();
                assert_eq!(
                    paths,
                    vec![
                        top.as_path(),
                        top.with_extension(if xml { "ytcs" } else { "xtcs" })
                            .as_path(),
                        top.with_file_name(format!("../{shared}")).as_path(),
                        top.with_file_name(format!("../{shared}"))
                            .with_extension(if xml { "ytcs" } else { "xtcs" })
                            .as_path(),
                    ],
                    "{name}"
                );

                // The group and the binding are the common file's, and stay
                // there.
                if example.starts_with("thermostat") {
                    let (top_text, common_text) = (&files[0].1, &files[2].1);
                    for said in ["zone_temps", "thermostat.setpoint"] {
                        assert!(common_text.contains(said), "{name}: {said}");
                    }
                    for section in
                        ["payload_groups", "\ncommands:", "<commands>"]
                    {
                        assert!(!top_text.contains(section), "{name}");
                    }
                }

                // Written somewhere else in the same shape, they read as the
                // shipped files do.
                let dir = tempfile::tempdir().expect("a directory");
                let common_dir = common.split('/').next().expect("a dir");
                for sub in [example, common_dir] {
                    std::fs::create_dir(dir.path().join(sub)).expect("made");
                }
                let shipped_files =
                    [examples.join(&name), examples.join(&shared)];
                let written_files =
                    [dir.path().join(&name), dir.path().join(&shared)];
                for ((_, text), at) in
                    [&files[0], &files[2]].into_iter().zip(&written_files)
                {
                    std::fs::write(at, text).expect("written");
                }
                let load = |path: &Path| {
                    let mut graph = Graph::load(path)
                        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                    let included = graph.included();
                    graph.files.clear();
                    graph.homes.clear();
                    (graph, included)
                };
                assert_eq!(
                    load(&written_files[0]),
                    load(&shipped_files[0]),
                    "{name}"
                );

                // And each file's comments are where they were.
                for (shipped, written) in
                    shipped_files.iter().zip(&written_files)
                {
                    let said = |path: &Path| {
                        let text = std::fs::read_to_string(path).expect("read");
                        let mut said = comments_in(&text, xml);
                        said.sort();
                        said
                    };
                    assert_eq!(
                        said(shipped),
                        said(written),
                        "{}",
                        shipped.display()
                    );
                }
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
        // that they say the same thing, and that neither is faulty. The
        // worked example is the one with contents worth asserting, and
        // `the_worked_example_is_the_configuration_it_describes` asserts them.
        for name in ["example", "tests/manual/thermostat"] {
            let mut read_as: Vec<(usize, usize)> = Vec::new();
            for extension in ["ytcs", "xtcs"] {
                let path = root.join(format!("{name}.{extension}"));
                let document =
                    read(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
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
        for extension in ["ytcs", "xtcs"] {
            let path =
                root.join(format!("tests/manual/thermostat.{extension}"));
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

        for extension in ["ytcs", "xtcs"] {
            let path =
                root.join(format!("tests/manual/thermostat.{extension}"));
            let document = read(&path).expect("the worked example");
            assert_eq!(document.nodes.len(), 12, "thermostat.{extension}");
            assert_eq!(document.links.len(), 18, "thermostat.{extension}");
        }
    }

    /// The shipped example is one configuration in two spellings.
    ///
    /// `example.xtcs` is written from `example.ytcs` rather than kept beside it
    /// by hand: the YAML is the one a person edits -- it is the one carrying
    /// the prose -- and the XML is the same configuration in the other
    /// language, which is what the XSD is validated against and what
    /// `tcslib-protocol` reads to prove its two readers agree. Two files kept
    /// in step by hand are two files that drift.
    ///
    /// Set TCS_GUI_WRITE_EXAMPLE to write it out after a deliberate change to
    /// the YAML, as `tests/manual/thermostat.*` is written.
    #[test]
    fn the_shipped_example_is_the_same_in_both_spellings() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let document =
            read(&root.join("example.ytcs")).expect("the example opens");
        let path = root.join("example.xtcs");
        // Written for the place it is going, which is what decides how it
        // names its schema: see `nodes::schema_reference`.
        let written = document
            .to_text(&path)
            .expect("XML is a format this writes");

        if std::env::var("TCS_GUI_WRITE_EXAMPLE").is_ok() {
            std::fs::write(&path, &written).expect("the example is writable");
            return;
        }

        let shipped = std::fs::read_to_string(&path).expect("the shipped XML");
        assert_eq!(
            shipped,
            written,
            "{} is not example.ytcs in the other language any more. Run the \
             tests with TCS_GUI_WRITE_EXAMPLE=1 to write it out",
            path.display()
        );
    }

    /// A comment belongs to the entry under it, in either language.
    #[test]
    fn a_comment_is_read_back_with_what_it_is_about() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

        for extension in ["ytcs", "xtcs"] {
            let path =
                root.join(format!("tests/manual/thermostat.{extension}"));
            let document = read(&path).expect("the example opens");

            let sensor = document
                .nodes
                .iter()
                .find(|n| n.name == "temp_1")
                .expect("the first of the trio");
            assert_eq!(
                sensor.comment.as_deref(),
                Some(
                    "One of three sensors of the same place, a trio, so \
                     that the\ntemperature survives one of them going \
                     wrong. Two bytes,\nsigned, big endian."
                ),
                "{extension}"
            );

            // Several lines, which is how the example writes the third
            // sensor's: a comment that wrapped is read back wrapped.
            let third = document
                .nodes
                .iter()
                .find(|n| n.name == "temp_3")
                .expect("the third of the trio");
            assert!(
                third.comment.as_deref().is_some_and(|said| {
                    said.contains("different bus")
                        && said.contains("own conversion")
                }),
                "{extension}: {:?}",
                third.comment
            );
            assert!(
                third
                    .comment
                    .as_deref()
                    .is_some_and(|said| said.contains('\n')),
                "{extension}: and over more than one line"
            );

            // A link's, and a command binding's. The comment is above the
            // terminal rather than one of its lines, so both heaters carry
            // it: it is about what leaves there and not about which heater.
            for heater in ["heaters_1", "heaters_2"] {
                let link = document
                    .links
                    .iter()
                    .find(|l| l.from == "thermostat" && l.to == heater)
                    .unwrap_or_else(|| panic!("the link to {heater}"));
                assert!(
                    link.comment.as_deref().is_some_and(
                        |said| said.contains("asserted below the setpoint")
                    ),
                    "{extension}: {heater}: {:?}",
                    link.comment
                );
            }
            assert_eq!(
                document.commands[0].comment.as_deref(),
                Some("the zone temperature the heaters are held at, in kelvin"),
                "{extension}"
            );

            // And a node with nothing said about it says nothing.
            let gateway = document
                .nodes
                .iter()
                .find(|n| n.name == "gateway")
                .expect("the ground link");
            assert_eq!(gateway.comment, None, "{extension}");
        }
    }

    /// The comments a configuration holds, each with the line it is above.
    ///
    /// For comparing what a file said with what the editor wrote back, so it
    /// is about the words and where they are and not their layout: a run of
    /// comment lines is one comment, an XML comment over several lines the
    /// same as several one-line ones, its words joined by single spaces. The
    /// line under each is the entry it is about, trimmed. The schema line at
    /// the top of a YAML file is the writer's own and is left out.
    ///
    /// # Arguments
    ///
    /// * `text` - The file's text.
    /// * `xml`  - Whether it is XML rather than YAML.
    ///
    /// # Returns
    ///
    /// Each comment's words, and the line after it, in the file's order.
    fn comments_in(text: &str, xml: bool) -> Vec<(String, String)> {
        let mut found = Vec::new();
        let mut waiting: Vec<String> = Vec::new();
        let mut lines = text.lines();
        while let Some(line) = lines.next() {
            let trimmed = line.trim();
            if let Some(comment) = comment_in(trimmed, xml) {
                waiting.push(comment);
                continue;
            }
            if xml && trimmed.starts_with("<!--") {
                waiting.extend(comment_over_lines(trimmed, &mut lines));
                continue;
            }
            if !waiting.is_empty() {
                let words = waiting.join(" ");
                let words: Vec<&str> = words.split_whitespace().collect();
                if !words.join(" ").starts_with("yaml-language-server") {
                    found.push((words.join(" "), trimmed.to_string()));
                }
                waiting.clear();
            }
        }
        found
    }

    /// Every shipped configuration keeps its comments through the editor.
    ///
    /// Read the way the editor opens a file and written the way it saves
    /// one, each example in its own language: every comment it held is in
    /// what is written, saying the same words above the same line. The
    /// examples are where a section's paragraph, a group's note and a
    /// tcspecial setting's reason are all found, and an editor that lost any
    /// of them would be one nobody could open an example in.
    #[test]
    fn every_example_keeps_its_comments() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut paths: Vec<std::path::PathBuf> = Vec::new();
        let mut directories = vec![root.join("tests/manual"), root.clone()];
        for entry in std::fs::read_dir(root.join("examples")).expect("there") {
            let path = entry.expect("an entry").path();
            // What two examples include is not a configuration on its own,
            // and is kept through the editor as part of each: see
            // `an_example_with_a_common_file_is_saved_as_it_was`.
            if !path.to_string_lossy().ends_with("-common") {
                directories.push(path);
            }
        }
        for directory in directories {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries {
                let path = entry.expect("an entry").path();
                let extension = path.extension().and_then(|e| e.to_str());
                if matches!(extension, Some("ytcs" | "xtcs")) {
                    paths.push(path);
                }
            }
        }
        paths.sort();
        assert!(paths.len() > 20, "the examples were not found: {paths:?}");

        let mut commented = 0;
        for path in &paths {
            let xml = path.extension().is_some_and(|e| e == "xtcs");
            let document = read(path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let written = document.to_text(path).expect("a known format");
            let shipped = std::fs::read_to_string(path).expect("readable");

            // In any order: a file written by hand may have its sections in
            // another, and the editor writes the one order a configuration
            // has. Which line each comment is above is what is compared.
            let mut before = comments_in(&shipped, xml);
            let mut after = comments_in(&written, xml);
            before.sort();
            after.sort();
            assert_eq!(
                before,
                after,
                "{}: the comments changed in\n{written}",
                path.display()
            );
            commented += usize::from(!before.is_empty());
        }
        assert!(commented > 20, "only {commented} files had comments");
    }

    /// A document with a payload of two types, for the comments about its
    /// structure to be about.
    ///
    /// # Returns
    ///
    /// The document: a ground link, and payloads of type `i2c` and
    /// `network`.
    fn two_types() -> Document {
        use crate::journal::Edit;

        let mut document = Document::new();
        for (name, node_type) in [
            ("ground", "gateway_radio"),
            ("sensor", "i2c"),
            ("socket", "network"),
        ] {
            document.apply(&Edit::AddNode {
                name: name.into(),
                node_type: node_type.into(),
                home: None,
            });
        }
        document
    }

    /// The comment for one line of the structure, in a document.
    ///
    /// # Arguments
    ///
    /// * `document` - The document.
    /// * `heading`  - The line.
    ///
    /// # Returns
    ///
    /// The comment's text, if it has one.
    fn remark_of<'a>(
        document: &'a Document,
        heading: &Heading,
    ) -> Option<&'a str> {
        document
            .remarks
            .iter()
            .find(|remark| &remark.heading == heading)
            .map(|remark| remark.text.as_str())
    }

    /// A comment about a section or a type group is read, kept, and written
    /// back where it was, in both languages.
    ///
    /// Three places: above the section's line, under it above its first
    /// group, and above a later group's line. Written by the editor, read,
    /// and written again, the second text is the first -- and the comments
    /// are where a person would look for them.
    #[test]
    fn a_comment_about_a_section_or_a_type_is_kept() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut document = two_types();
        let said = [
            (Heading::Section("payloads".into()), "what is carried"),
            (Heading::Opening("payloads".into()), "all of it\non a desk"),
            (Heading::Type("network".into()), "sockets -- for now"),
        ];
        for (heading, text) in &said {
            document.remarks.push(Remark {
                home: None,
                heading: heading.clone(),
                text: text.to_string(),
            });
        }

        for name in ["two.ytcs", "two.xtcs"] {
            let path = where_.path().join(name);
            let text = document.to_text(&path).expect("a known format");
            std::fs::write(&path, &text).expect("written");

            let read = read(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
            let xml = name.ends_with(".xtcs");
            for (heading, text) in &said {
                // The one change XML makes: see `document::comment_safe`.
                let text = if xml {
                    text.replace("--", "-")
                } else {
                    text.to_string()
                };
                assert_eq!(
                    remark_of(&read, heading),
                    Some(text.as_str()),
                    "{name}: {heading:?}"
                );
            }
            assert_eq!(
                read.to_text(&path).expect("a known format"),
                text,
                "{name}: written back differently"
            );

            let expected = if xml {
                "    <!-- what is carried -->\n    <payloads>\n        \
                 <!-- all of it\n             on a desk -->\n        \
                 <type name=\"i2c\">"
            } else {
                "# what is carried\npayloads:\n  # all of it\n  \
                 # on a desk\n  - type i2c:"
            };
            assert!(text.contains(expected), "{name}:\n{text}");
            let expected = if xml {
                "        <!-- sockets - for now -->\n        \
                 <type name=\"network\">"
            } else {
                "  # sockets -- for now\n  - type network:"
            };
            assert!(text.contains(expected), "{name}:\n{text}");
            if xml {
                for line in text.lines().filter(|l| l.contains("<!--")) {
                    let body = line
                        .trim()
                        .trim_start_matches("<!--")
                        .trim_end_matches("-->");
                    assert!(!body.contains("--"), "{name}: {line}");
                }
            }
        }
    }

    /// A long comment about a section is written as it was given, as a
    /// node's is, in both languages, and read back the same.
    #[test]
    fn a_long_comment_about_a_section_is_written_as_given() {
        let mut document = two_types();
        let long = "word ".repeat(40);
        for heading in [
            Heading::Section("payloads".into()),
            Heading::Opening("payloads".into()),
            Heading::Type("network".into()),
        ] {
            document.remarks.push(Remark {
                home: None,
                heading,
                text: long.trim().to_string(),
            });
        }
        document
            .tcspecial
            .insert("beacon.recv".into(), "127.0.0.1:5650".into());
        document
            .tcspecial_comments
            .insert("beacon.recv".into(), long.trim().to_string());

        let where_ = tempfile::tempdir().expect("a directory");
        for name in ["long.ytcs", "long.xtcs"] {
            let path = where_.path().join(name);
            let text = document.to_text(&path).expect("a known format");
            let comments = text
                .lines()
                .filter(|l| l.contains("word word"))
                .collect::<Vec<&str>>();
            assert_eq!(comments.len(), 4, "{text}");
            for line in comments {
                assert!(line.contains(long.trim()), "broken: {line}");
            }
            std::fs::write(&path, &text).expect("written");
            let read = read(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(read.remarks, document.remarks, "{name}");
            assert_eq!(read.to_text(&path).expect("known"), text, "{name}");
        }
    }

    /// A comment above a line of the tcspecial section is kept with it.
    #[test]
    fn a_comment_in_the_tcspecial_section_is_kept() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut document = two_types();
        for (attribute, value) in [
            ("beacon.recv", "127.0.0.1:5650"),
            ("beacon.send", "0.0.0.0:5550"),
            ("state_file", "/tmp/state"),
        ] {
            document.tcspecial.insert(attribute.into(), value.into());
        }
        for (key, text) in [
            ("beacon", "how the vehicle says it is alive"),
            ("beacon.send", "where it says so"),
            ("state_file", "on storage that survives a power cycle"),
        ] {
            document.tcspecial_comments.insert(key.into(), text.into());
        }

        for name in ["said.ytcs", "said.xtcs"] {
            let path = where_.path().join(name);
            let text = document.to_text(&path).expect("a known format");
            std::fs::write(&path, &text).expect("written");
            let read = read(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(
                read.tcspecial_comments, document.tcspecial_comments,
                "{name}:\n{text}"
            );
            assert_eq!(read.to_text(&path).expect("known"), text, "{name}");
        }
    }

    /// A section or a group the editor empties takes its comment with it,
    /// and one made again starts without.
    #[test]
    fn an_emptied_section_or_group_forgets_its_comment() {
        use crate::journal::Edit;

        let remove = |name: &str, node_type: &str| Edit::RemoveNode {
            name: name.into(),
            node_type: node_type.into(),
            attributes: Vec::new(),
            links: Vec::new(),
            home: None,
        };
        let mut document = two_types();
        for heading in [
            Heading::Section("payloads".into()),
            Heading::Opening("payloads".into()),
            Heading::Type("network".into()),
            Heading::Type("i2c".into()),
            Heading::Section("gateways".into()),
        ] {
            document.remarks.push(Remark {
                home: None,
                heading,
                text: "said".into(),
            });
        }

        // The group goes with its last node, and nothing else does.
        document.apply(&remove("socket", "network"));
        assert_eq!(
            remark_of(&document, &Heading::Type("network".into())),
            None
        );
        assert!(remark_of(&document, &Heading::Type("i2c".into())).is_some());
        assert!(remark_of(&document, &Heading::Opening("payloads".into()))
            .is_some());

        // Made again, the group has nothing said about it.
        document.apply(&Edit::AddNode {
            name: "socket".into(),
            node_type: "network".into(),
            home: None,
        });
        assert_eq!(
            remark_of(&document, &Heading::Type("network".into())),
            None
        );
        assert!(
            !document.to_yaml().contains("# said\n  - type network:"),
            "{}",
            document.to_yaml()
        );

        // And the section goes with its last node.
        document.apply(&remove("socket", "network"));
        document.apply(&remove("sensor", "i2c"));
        for heading in [
            Heading::Section("payloads".into()),
            Heading::Opening("payloads".into()),
            Heading::Type("i2c".into()),
        ] {
            assert_eq!(remark_of(&document, &heading), None, "{heading:?}");
        }
        assert!(remark_of(&document, &Heading::Section("gateways".into()))
            .is_some());
        document.apply(&Edit::AddNode {
            name: "sensor".into(),
            node_type: "i2c".into(),
            home: None,
        });
        let yaml = document.to_yaml();
        assert!(yaml.contains("\npayloads:\n  - type i2c:"), "{yaml}");
        assert!(yaml.contains("# said\ngateways:"), "{yaml}");
    }

    /// A file that is not there opens as a new configuration.
    #[test]
    fn a_name_that_does_not_exist_yet_is_a_new_document() {
        let where_ = tempfile::tempdir().expect("a directory");
        let document = read(&where_.path().join("nothing-here.ytcs"))
            .expect("a new document");
        assert!(document.nodes.is_empty() && document.links.is_empty());
    }

    /// A file that is not a configuration is reported rather than opened as
    /// an empty one.
    #[test]
    fn a_file_that_is_not_a_configuration_is_reported() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("notes.ytcs");
        std::fs::write(&path, "this is not a configuration\n")
            .expect("written");

        let said = read(&path).expect_err("a file that is not one");
        assert!(!said.is_empty(), "the complaint should say something");
    }
}
