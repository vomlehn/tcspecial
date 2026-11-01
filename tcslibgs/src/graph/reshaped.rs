//! The shape a configuration's nodes used to be written in, refused.
//!
//! A node was written once as a mapping or an element of its own, its name
//! and its type beside what it is told, and a section was a list of them:
//!
//! ```yaml
//! payloads:
//!   - name: temp_1
//!     type: i2c
//!     address: "0x48"
//! ```
//!
//! In XML each node was an element named for its section, `<payloads>`
//! repeated once for each payload. The nodes are grouped by type now -- see
//! [`super::grouped`] -- and the gateway section is `gateways`, so a file in
//! the old shape is refused here, at the line of its first node, and shown
//! how that node is written now. Here rather than in serde's words, because
//! "unknown field `gateway`" or "invalid type" says what is wrong and not
//! what to do, and a file that was right once deserves to be told.
//!
//! The editor's section had the old shape too: a position was `- { name:
//! temp_1, x: 34, y: 34 }`, and in XML a `<nodes>` element for each node.
//!
//! `tools/convert-format.py` rewrites a file in the old shape into the new
//! one, keeping its comments, and the refusal says so.
//!
//! What a file says is read as its language reads it, so that a word in a
//! comment is not taken for a key; the text is searched only for the line to
//! point at, as [`super::renamed`] does.

use quick_xml::events::{BytesStart, Event};

use super::renamed::{keys_and_values, uncommented};
use crate::format::ConfigFormat;

/// What the refusal ends with: where to find the converter.
const CONVERTER: &str =
    "tools/convert-format.py rewrites a file in the old shape into this one";

/// Says how a file still written in the old shape must be written now.
///
/// # Arguments
///
/// * `text`   - The file's text.
/// * `format` - What it is written in.
///
/// # Returns
///
/// A message naming the line of the first thing in the old shape and showing
/// it in the new; or `None` for a file in the new shape, or one that does not
/// parse, which the parser has its own words for.
pub fn refused(text: &str, format: ConfigFormat) -> Option<String> {
    match format {
        ConfigFormat::Yaml => in_yaml(text),
        ConfigFormat::Xml => in_xml(text),
    }
}

/// How a node in the old shape is written now, in YAML.
///
/// # Arguments
///
/// * `section`   - The section it belongs in, as it is named now.
/// * `word`      - The word that names a node of that section.
/// * `name`      - The node's name.
/// * `node_type` - Its type.
///
/// # Returns
///
/// The shape, in YAML's flow style so that it fits on the one line a
/// refusal is printed on.
fn yaml_now(section: &str, word: &str, name: &str, node_type: &str) -> String {
    format!(
        "`{section}: [ {{ type {node_type}: [ {{ {word}: {name}, ... }} ] }} \
         ]`"
    )
}

/// The section a node section of the old shape is written as now, and the
/// word for one of its nodes.
///
/// # Arguments
///
/// * `old` - The section as the old shape named it.
///
/// # Returns
///
/// The section and the word, or `None` for a section that held no nodes.
fn now_of(old: &str) -> Option<(&'static str, &'static str)> {
    match old {
        "gateway" => Some(("gateways", "gateway")),
        "vehicle" => Some(("vehicle", "compute")),
        "payloads" => Some(("payloads", "payload")),
        _ => None,
    }
}

/// Finds the first thing a YAML file writes in the old shape.
///
/// # Arguments
///
/// * `text` - The file's text.
///
/// # Returns
///
/// The refusal, or `None`.
fn in_yaml(text: &str) -> Option<String> {
    let file: serde_norway::Value = serde_norway::from_str(text).ok()?;
    let sections = file.as_mapping()?;
    let mut found: Vec<(usize, String)> = Vec::new();
    for (key, value) in sections {
        let Some(key) = key.as_str() else {
            continue;
        };
        if key == "tcsgui" {
            let placed = value
                .get("nodes")
                .and_then(serde_norway::Value::as_sequence)
                .into_iter()
                .flatten()
                .find(|entry| entry.get("name").is_some());
            if let Some(entry) = placed {
                let name = scalar(entry.get("name"));
                let line = line_of_key(text, "tcsgui", "name", &name);
                found.push((
                    line,
                    format!(
                        "at line {line}, the position of \"{name}\" is \
                         written as it was before the editor's section named \
                         nodes as the sections do: write it `- {{ node: \
                         {name}, x: ..., y: ... }}`; {CONVERTER}"
                    ),
                ));
            }
            continue;
        }
        let Some((section, word)) = now_of(key) else {
            continue;
        };
        let old = value
            .as_sequence()
            .into_iter()
            .flatten()
            .find(|entry| {
                entry.get("name").is_some() || entry.get("type").is_some()
            })
            .map(|entry| {
                (scalar(entry.get("name")), scalar(entry.get("type")))
            });
        let old = match (key, old) {
            (_, Some(old)) => old,
            // The gateway section is gateways now, whatever it holds.
            ("gateway", None) => ("<name>".to_string(), "<type>".to_string()),
            _ => continue,
        };
        let (name, node_type) = old;
        let line = if name == "<name>" {
            line_of_section(text, key)
        } else {
            line_of_key(text, key, "name", &name)
        };
        let what = if key == "gateway" {
            "the gateway section is gateways now, and".to_string()
        } else {
            format!("in {key},")
        };
        found.push((
            line,
            format!(
                "at line {line}, {what} the node \"{name}\" is written as a \
                 node was before nodes were grouped by type, with name: and \
                 type: of its own. Write it under its type now, named by the \
                 word {word}, as {}; {CONVERTER}",
                yaml_now(section, word, &name, &node_type)
            ),
        ));
    }
    found
        .into_iter()
        .min_by_key(|(line, _)| *line)
        .map(|(_, why)| why)
}

/// A YAML value as text, for a message.
///
/// # Arguments
///
/// * `value` - The value, if there is one.
///
/// # Returns
///
/// Its text, or `<name>`-like placeholders for none.
fn scalar(value: Option<&serde_norway::Value>) -> String {
    match value {
        Some(serde_norway::Value::String(s)) => s.clone(),
        Some(serde_norway::Value::Number(n)) => n.to_string(),
        Some(serde_norway::Value::Bool(b)) => b.to_string(),
        _ => "<missing>".to_string(),
    }
}

/// Finds the line a top-level section of a YAML file begins on.
///
/// # Arguments
///
/// * `text`    - The file's text.
/// * `section` - The section's key.
///
/// # Returns
///
/// The line, from 1, or 1 where it cannot be found.
fn line_of_section(text: &str, section: &str) -> usize {
    text.lines()
        .position(|line| {
            let line = uncommented(line);
            !line.starts_with([' ', '\t'])
                && keys_and_values(line).any(|(key, _)| key == section)
        })
        .map_or(1, |at| at + 1)
}

/// Finds the line, in a top-level section of a YAML file, a key with a
/// value is written on.
///
/// # Arguments
///
/// * `text`    - The file's text.
/// * `section` - The section's key.
/// * `key`     - The key.
/// * `value`   - What it says.
///
/// # Returns
///
/// The line, from 1, or the section's where it cannot be found.
fn line_of_key(text: &str, section: &str, key: &str, value: &str) -> usize {
    let start = line_of_section(text, section);
    text.lines()
        .enumerate()
        .skip(start)
        .take_while(|(_, line)| {
            line.is_empty() || line.starts_with([' ', '\t', '-', '#'])
        })
        .find(|(_, line)| {
            keys_and_values(uncommented(line))
                .any(|(k, v)| k == key && v == value)
        })
        .map_or(start, |(at, _)| at + 1)
}

/// Says whether an XML element has an attribute of this name.
///
/// # Arguments
///
/// * `tag`  - The element's start tag.
/// * `name` - The attribute.
///
/// # Returns
///
/// Whether it has one.
fn has_attribute(tag: &BytesStart, name: &str) -> bool {
    tag.attributes()
        .flatten()
        .any(|attribute| attribute.key.as_ref() == name)
}

/// What is known of an element in the old shape while it is being read.
struct Old {
    /// Its line.
    line: usize,
    /// The section, or `tcsgui`, it is in.
    section: String,
    /// What its `name` and `type` elements say, as far as they are read.
    name: Option<String>,
    node_type: Option<String>,
    /// Whether it is in the old shape: decided by what is in it.
    old: bool,
}

/// Finds the first thing an XML file writes in the old shape.
///
/// # Arguments
///
/// * `text` - The file's text.
///
/// # Returns
///
/// The refusal, or `None`.
fn in_xml(text: &str) -> Option<String> {
    let mut reader = quick_xml::Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let line_at = |at: u64| {
        let at = usize::try_from(at).unwrap_or(usize::MAX).min(text.len());
        text.as_bytes()[..at]
            .iter()
            .filter(|b| **b == b'\n')
            .count()
            + 1
    };
    let mut depth = 0usize;
    // The section a node of the old shape would be: a child of the root.
    let mut node: Option<Old> = None;
    // Inside the editor's section, and inside one of its lists.
    let mut in_tcsgui = false;
    let mut list: Option<(String, usize)> = None;
    // The element whose text is being read, as name or type.
    let mut reading: Option<String> = None;
    loop {
        let event = reader.read_event().ok()?;
        let at = line_at(reader.buffer_position());
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(tag) | Event::Empty(tag) => {
                let element = tag.name().as_ref().to_string();
                let level = depth + 1;
                if level == 2 {
                    in_tcsgui = element == "tcsgui";
                    if now_of(&element).is_some() {
                        node = Some(Old {
                            line: at,
                            section: element.clone(),
                            name: None,
                            node_type: None,
                            // A <gateway> at the top is a node of the old
                            // shape whatever is in it.
                            old: element == "gateway",
                        });
                    }
                }
                if level == 3 {
                    if let Some(old) = node.as_mut() {
                        if element == "name"
                            || element == "type" && !has_attribute(&tag, "name")
                        {
                            old.old = true;
                            reading = (!empty).then(|| element.clone());
                        }
                    }
                    if in_tcsgui
                        && (element == "nodes" || element == "set-aside")
                    {
                        list = Some((element.clone(), at));
                    }
                }
                if level == 4 {
                    if let Some((what, line)) = list.as_ref() {
                        let old = match what.as_str() {
                            "nodes" => element == "name",
                            _ => {
                                element == "concern"
                                    || element == "node"
                                        && !has_attribute(&tag, "name")
                            }
                        };
                        if old {
                            return Some(layout_refused(what, *line));
                        }
                    }
                }
                if !empty {
                    depth = level;
                }
            }
            Event::Text(said) => {
                if let (Some(field), Some(old)) =
                    (reading.as_ref(), node.as_mut())
                {
                    let said = said.into_inner();
                    let said = said.trim().to_string();
                    if field == "name" {
                        old.name = Some(said);
                    } else {
                        old.node_type = Some(said);
                    }
                }
            }
            Event::End(_) => {
                reading = None;
                if depth == 3 {
                    list = None;
                }
                if depth == 2 {
                    if let Some(old) = node.take().filter(|old| old.old) {
                        return Some(node_refused(&old));
                    }
                    in_tcsgui = false;
                }
                depth = depth.saturating_sub(1);
            }
            Event::Eof => return None,
            _ => {}
        }
    }
}

/// The refusal of an XML node in the old shape.
///
/// # Arguments
///
/// * `old` - The node, as far as it was read.
///
/// # Returns
///
/// The message.
fn node_refused(old: &Old) -> String {
    let (section, word) = now_of(&old.section).expect("a node section");
    let name = old.name.as_deref().unwrap_or("...");
    let node_type = old.node_type.as_deref().unwrap_or("...");
    let what = if old.section == "gateway" {
        "the gateway section is <gateways> now, and".to_string()
    } else {
        String::new()
    };
    let now = format!(
        "<{section}><type name=\"{node_type}\"><{word} name=\"{name}\">...\
         </{word}></type></{section}>"
    );
    format!(
        "at line {}, {what} the node \"{name}\" is written as a node was \
         before nodes were grouped by type, as a <{}> element with <name> and \
         <type> in it. Write it under its type now, as {now}; {CONVERTER}",
        old.line, old.section
    )
}

/// The refusal of an XML list of the editor's section in the old shape.
///
/// # Arguments
///
/// * `what` - `nodes` or `set-aside`.
/// * `line` - Where it is.
///
/// # Returns
///
/// The message.
fn layout_refused(what: &str, line: usize) -> String {
    let now = if what == "nodes" {
        "<nodes><node name=\"...\" x=\"...\" y=\"...\"/></nodes>"
    } else {
        "<set-aside><node name=\"...\" concern=\"...\"/></set-aside>"
    };
    format!(
        "at line {line}, <{what}> in the tcsgui section is written as it was \
         before the editor's section named nodes as the sections do: one \
         <{what}> holds an entry for each node now, as {now}; {CONVERTER}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A YAML node with name: and type: keys is refused at its line, shown
    /// written under its type.
    #[test]
    fn a_yaml_node_of_the_old_shape_is_refused_at_its_line() {
        let text = "description: d\n\
                    payloads:\n  \
                    # a sensor\n  \
                    - name: temp_1\n    \
                    type: i2c\n";
        let why = refused(text, ConfigFormat::Yaml).expect("refused");
        assert!(why.starts_with("at line 4, in payloads,"), "{why}");
        assert!(why.contains("type i2c:"), "{why}");
        assert!(why.contains("payload: temp_1"), "{why}");
        assert!(why.contains("convert-format.py"), "{why}");
    }

    /// The old gateway section is refused even when it holds nothing.
    #[test]
    fn the_old_gateway_section_is_refused() {
        let text = "gateway:\n  - name: down\n    type: gateway_radio\n";
        let why = refused(text, ConfigFormat::Yaml).expect("refused");
        assert!(why.starts_with("at line 2, the gateway section"), "{why}");
        assert!(why.contains("gateways:"), "{why}");
        assert!(why.contains("type gateway_radio:"), "{why}");
        assert!(why.contains("gateway: down"), "{why}");
        assert!(!why.contains('\n'), "one line: {why}");

        let why = refused("gateway: []\n", ConfigFormat::Yaml).expect("it");
        assert!(why.starts_with("at line 1, the gateway section"), "{why}");
    }

    /// A position in the old shape is refused, in either language.
    #[test]
    fn a_position_of_the_old_shape_is_refused() {
        let text = "tcsgui:\n  nodes:\n    - { name: a, x: 1, y: 2 }\n";
        let why = refused(text, ConfigFormat::Yaml).expect("refused");
        assert!(why.starts_with("at line 3,"), "{why}");
        assert!(why.contains("node: a"), "{why}");

        let text = "<configuration>\n<tcsgui>\n\
                    <nodes><name>a</name><x>1</x><y>2</y></nodes>\n\
                    </tcsgui>\n</configuration>\n";
        let why = refused(text, ConfigFormat::Xml).expect("refused");
        assert!(why.starts_with("at line 3,"), "{why}");

        let text = "<configuration>\n<tcsgui>\n\
                    <set-aside><node>a</node><concern>c</concern>\
                    </set-aside>\n</tcsgui>\n</configuration>\n";
        let why = refused(text, ConfigFormat::Xml).expect("refused");
        assert!(why.starts_with("at line 3,"), "{why}");
    }

    /// An XML node element with <name> and <type> in it is refused at its
    /// line, shown written under its type.
    #[test]
    fn an_xml_node_of_the_old_shape_is_refused_at_its_line() {
        let text = "<configuration>\n\
                    <description>d</description>\n\
                    <vehicle>\n<name>ping</name>\n<type>ping</type>\n\
                    </vehicle>\n</configuration>\n";
        let why = refused(text, ConfigFormat::Xml).expect("refused");
        assert!(why.starts_with("at line 3,"), "{why}");
        assert!(why.contains("<type name=\"ping\">"), "{why}");
        assert!(why.contains("<compute name=\"ping\">"), "{why}");

        let text = "<configuration>\n<gateway>\n<name>g</name>\n\
                    <type>gateway_radio</type>\n</gateway>\n</configuration>\n";
        let why = refused(text, ConfigFormat::Xml).expect("refused");
        assert!(why.starts_with("at line 2, the gateway section"), "{why}");
        assert!(why.contains("<gateways>"), "{why}");
        assert!(why.contains("<gateway name=\"g\">"), "{why}");
    }

    /// A file in the new shape is not refused, and nor is a word of the old
    /// shape only talked about.
    #[test]
    fn a_file_in_the_new_shape_is_taken() {
        let yaml = "# - name: a\n#   type: i2c\n\
                    description: \"name: type:\"\n\
                    gateways:\n  - type gateway_radio:\n      \
                    - gateway: g\n\
                    payloads:\n  - type i2c:\n      - payload: a\n\
                    tcsgui:\n  nodes:\n    - { node: a, x: 1, y: 2 }\n  \
                    set-aside:\n    - { node: a, concern: c }\n";
        assert_eq!(refused(yaml, ConfigFormat::Yaml), None);

        let xml = "<configuration>\n\
                   <!-- <payloads><name>a</name></payloads> -->\n\
                   <gateways><type name=\"gateway_radio\">\
                   <gateway name=\"g\"/></type></gateways>\n\
                   <payloads><type name=\"i2c\"><payload name=\"a\">\
                   <address>0x48</address></payload></type></payloads>\n\
                   <tcsgui><nodes><node name=\"a\" x=\"1\" y=\"2\"/></nodes>\
                   <set-aside><node name=\"a\" concern=\"c\"/></set-aside>\
                   </tcsgui>\n</configuration>\n";
        assert_eq!(refused(xml, ConfigFormat::Xml), None);
    }
}
