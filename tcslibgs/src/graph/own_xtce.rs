//! A configuration's own XTCE: what the vehicle flying it sends the ground.
//!
//! Each node whose data reaches the ground -- drawn into a gateway node, or a
//! payload nothing touches, which reports over `payload_telemetry` -- is a
//! parameter named for the node, of the type its `world-out` sends, in a
//! container of its own, `<node>_packet`. The types are copied from
//! `tcslibgs/xtce/types.xtce` as it states them, and a structure is an
//! aggregate of them, so a ground station is given one file. How each
//! gateway frames what it sends -- a CCSDS header and an APID under Yamcs, a
//! port of its own over a radio -- is the ground station's to add.
//!
//! `tcsverify --xtce` writes it beside the configuration, and `make check`
//! fails when one committed there is not what it would write: everything it
//! says is said by the configuration and `types.xtce` already, and nobody
//! writes it by hand. See "The configuration's own XTCE" in
//! `docs/design.rst`.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::link_types::{carries, Carries};
use super::telemetry;
use super::Graph;
use crate::link_type::{Basic, LinkType};

/// What every type is copied from: `tcslibgs/xtce/types.xtce`.
const TYPES: &str = include_str!("../../xtce/types.xtce");

/// The namespace it is in, which this document is in too: XTCE 1.3's.
const NAMESPACE: &str = "http://www.omg.org/spec/XTCE/20250214";

/// The indent of each level, which is `types.xtce`'s, so that what is
/// copied from it sits as it does there.
const INDENT: &str = "  ";

/// The parameter a `bytes` datum's length is, which `types.xtce` sizes
/// every `bytes` by.
const BYTES_LENGTH: &str = "bytes_length";

/// Writes a configuration's own XTCE.
///
/// # Arguments
///
/// * `graph` - The configuration.
/// * `name`  - What its space system is called: the configuration's file's
///             name without its extension, as `thermostat`.
///
/// # Returns
///
/// The document, as it is written to a file.
///
/// # Errors
///
/// A message when a node that reaches the ground states a type that does not
/// read, or is not there: the configuration's check says each of these too.
pub fn written(graph: &Graph, name: &str) -> Result<String, String> {
    let mut sources: BTreeMap<String, LinkType> = BTreeMap::new();
    for source in telemetry::sources(graph) {
        let node = graph
            .node(&source)
            .ok_or_else(|| format!("\"{source}\" is no node"))?;
        let kind = match carries(node, "world-out")? {
            Carries::Only(kind) => kind,
            Carries::Any => LinkType::Basic(Basic::Bytes),
        };
        sources.insert(source, kind);
    }

    let mut out = String::new();
    head(&mut out, name);
    let _ = writeln!(out, "{INDENT}<TelemetryMetaData>");
    parameter_types(&mut out, &sources)?;
    parameters(&mut out, &sources);
    containers(&mut out, &sources);
    let _ = writeln!(out, "{INDENT}</TelemetryMetaData>");
    out.push_str("</SpaceSystem>\n");
    Ok(out)
}

/// Writes the declaration, the comment that says what the document is, and
/// the space system's opening tag.
///
/// # Arguments
///
/// * `out`  - Where it is written.
/// * `name` - The space system's name.
fn head(out: &mut String, name: &str) {
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = write!(
        out,
        "\
<!-- What the vehicle sends the ground, flying the configuration
     {name}: each node whose data reaches it, as a parameter named
     for the node, of the type the node sends, in a container of its own.
     The types are tcslibgs/xtce/types.xtce's, copied. A string or bytes
     is all of what carries it, and no count is sent before it: its
     length, and bytes_length, are what the ground is given by how it is
     framed. How each gateway frames what it sends is the ground
     station's to add.

     Written by tcsverify, asked for it by its xtce option, from the
     configuration and types.xtce, and not to be edited: see \"The
     configuration's own XTCE\" in tcspecial's docs/design.rst. -->
"
    );
    let one_line =
        format!("<SpaceSystem xmlns=\"{NAMESPACE}\" name=\"{name}\">");
    if one_line.len() <= 80 {
        let _ = writeln!(out, "{one_line}");
    } else {
        let _ = writeln!(
            out,
            "<SpaceSystem xmlns=\"{NAMESPACE}\"\n\
             {INDENT}{INDENT}name=\"{name}\">"
        );
    }
}

/// Writes the parameter types: each of `types.xtce`'s a source uses, in the
/// order it gives them, and then each source's structure.
///
/// # Arguments
///
/// * `out`     - Where they are written.
/// * `sources` - Each source's type, by its name.
///
/// # Errors
///
/// A message where `types.xtce` does not state a type it is to copy.
fn parameter_types(
    out: &mut String,
    sources: &BTreeMap<String, LinkType>,
) -> Result<(), String> {
    let mut used: Vec<Basic> = Vec::new();
    for kind in sources.values() {
        match kind {
            LinkType::Basic(basic) => used.push(*basic),
            LinkType::Structure(fields) => {
                used.extend(fields.iter().map(|(_, basic)| *basic))
            }
        }
    }
    let level = INDENT.repeat(2);
    let _ = writeln!(out, "{level}<ParameterTypeSet>");
    for (element, copied) in copied_types()? {
        if used.iter().any(|basic| basic.name() == element)
            || (element == BYTES_LENGTH && used.contains(&Basic::Bytes))
        {
            out.push_str(&copied);
        }
    }
    for (source, kind) in sources {
        let LinkType::Structure(fields) = kind else {
            continue;
        };
        let level = INDENT.repeat(3);
        let _ = writeln!(
            out,
            "{level}<AggregateParameterType name=\"{}\">",
            structure_name(source)
        );
        let _ = writeln!(out, "{level}{INDENT}<MemberList>");
        for (field, basic) in fields {
            let _ = writeln!(
                out,
                "{level}{INDENT}{INDENT}<Member name=\"{field}\" \
                 typeRef=\"{}\"/>",
                basic.name()
            );
        }
        let _ = writeln!(out, "{level}{INDENT}</MemberList>");
        let _ = writeln!(out, "{level}</AggregateParameterType>");
    }
    let _ = writeln!(out, "{level}</ParameterTypeSet>");
    Ok(())
}

/// Writes a parameter for each source, and `bytes_length` where a source
/// sends bytes: the length `types.xtce` sizes every `bytes` by, which is
/// in no container, because no count is sent before a datum.
///
/// # Arguments
///
/// * `out`     - Where they are written.
/// * `sources` - Each source's type, by its name.
fn parameters(out: &mut String, sources: &BTreeMap<String, LinkType>) {
    let level = INDENT.repeat(2);
    let _ = writeln!(out, "{level}<ParameterSet>");
    if sources
        .values()
        .any(|k| *k == LinkType::Basic(Basic::Bytes))
    {
        let _ = writeln!(
            out,
            "{level}{INDENT}<Parameter name=\"{BYTES_LENGTH}\" \
             parameterTypeRef=\"{BYTES_LENGTH}\"/>"
        );
    }
    for (source, kind) in sources {
        let type_name = match kind {
            LinkType::Basic(basic) => basic.name().to_string(),
            LinkType::Structure(_) => structure_name(source),
        };
        let one_line = format!(
            "{level}{INDENT}<Parameter name=\"{source}\" \
             parameterTypeRef=\"{type_name}\"/>"
        );
        if one_line.len() <= 80 {
            let _ = writeln!(out, "{one_line}");
        } else {
            let _ = writeln!(
                out,
                "{level}{INDENT}<Parameter name=\"{source}\"\n\
                 {level}{INDENT}{INDENT}{INDENT}\
                 parameterTypeRef=\"{type_name}\"/>"
            );
        }
    }
    let _ = writeln!(out, "{level}</ParameterSet>");
}

/// Writes a container for each source, holding its parameter.
///
/// # Arguments
///
/// * `out`     - Where they are written.
/// * `sources` - Each source's type, by its name.
fn containers(out: &mut String, sources: &BTreeMap<String, LinkType>) {
    let level = INDENT.repeat(2);
    let _ = writeln!(out, "{level}<ContainerSet>");
    for source in sources.keys() {
        let inner = INDENT.repeat(3);
        let _ = writeln!(
            out,
            "{inner}<SequenceContainer name=\"{}\">",
            container_name(source)
        );
        let _ = writeln!(out, "{inner}{INDENT}<EntryList>");
        let _ = writeln!(
            out,
            "{inner}{INDENT}{INDENT}<ParameterRefEntry \
             parameterRef=\"{source}\"/>"
        );
        let _ = writeln!(out, "{inner}{INDENT}</EntryList>");
        let _ = writeln!(out, "{inner}</SequenceContainer>");
    }
    let _ = writeln!(out, "{level}</ContainerSet>");
}

/// The name of the container a source's data is in.
///
/// # Arguments
///
/// * `source` - The source's name.
///
/// # Returns
///
/// `<source>_packet`.
pub fn container_name(source: &str) -> String {
    format!("{source}_packet")
}

/// The name of a source's structure's type.
///
/// # Arguments
///
/// * `source` - The source's name.
///
/// # Returns
///
/// `<source>_fields`.
pub fn structure_name(source: &str) -> String {
    format!("{source}_fields")
}

/// Finds each parameter type `types.xtce` states, as it writes it.
///
/// # Returns
///
/// Each type's name and its element's lines, in the order the document
/// gives them.
///
/// # Errors
///
/// A message where an element's end cannot be found.
fn copied_types() -> Result<Vec<(String, String)>, String> {
    let mut found = Vec::new();
    let mut lines = TYPES.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        let Some(element) = trimmed
            .strip_prefix('<')
            .and_then(|rest| rest.split_once(' '))
            .map(|(element, _)| element)
            .filter(|element| element.ends_with("ParameterType"))
        else {
            continue;
        };
        let mut text = format!("{line}\n");
        let mut whole = line.to_string();
        // The name may be on a line after the element's, as a long one's
        // attributes are; the element ends at its closing tag, or at the
        // end of its opening one where it has no content.
        let closing = format!("</{element}>");
        let mut done = line.trim_end().ends_with("/>");
        while !done {
            let next = lines
                .next()
                .ok_or_else(|| format!("types.xtce: no end to {element}"))?;
            let _ = writeln!(text, "{next}");
            whole.push(' ');
            whole.push_str(next);
            done = next.trim() == closing;
        }
        let name = whole
            .split_once("name=\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(name, _)| name.to_string())
            .ok_or_else(|| format!("types.xtce: a {element} with no name"))?;
        found.push((name, text));
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// The thermostat example, as shipped.
    fn thermostat() -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        Graph::load(root.join("examples/thermostat/thermostat.ytcs"))
            .expect("the example reads")
    }

    /// Every type `types.xtce` states is found, by its name.
    #[test]
    fn every_type_is_found() {
        let names: Vec<String> = copied_types()
            .expect("types.xtce reads")
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        for name in ["u8", "i64", "boolean", "kelvin", "string", "bytes"] {
            assert!(names.iter().any(|n| n == name), "{name}: {names:?}");
        }
        assert!(names.iter().any(|n| n == BYTES_LENGTH));
    }

    /// Each source is a parameter of the type it sends, in a container of
    /// its own, and only the types used are copied.
    #[test]
    fn each_source_is_a_parameter_of_its_type() {
        let written = written(&thermostat(), "thermostat").expect("written");
        assert!(
            written.contains(
                "<Parameter name=\"consensus_temp\" \
                 parameterTypeRef=\"kelvin\"/>"
            ),
            "{written}"
        );
        assert!(written.contains("<FloatParameterType name=\"kelvin\""));
        assert!(written.contains("name=\"consensus_temp_packet\""));
        assert!(!written.contains("name=\"volt\""), "{written}");
        assert!(written.lines().all(|line| line.len() <= 80), "{written}");
        // An XML comment may not hold two hyphens together.
        let (_, comment) = written.split_once("<!--").expect("a comment");
        let (comment, _) = comment.split_once("-->").expect("its end");
        assert!(!comment.contains("--"), "{comment}");
    }
}
