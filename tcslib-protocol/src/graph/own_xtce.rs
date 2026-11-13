//! A configuration's own XTCE: what the vehicle flying it sends the ground.
//!
//! Each node whose data reaches the ground -- drawn into a gateway node, or a
//! payload nothing touches, which reports over `payload_telemetry` -- is a
//! parameter named for the node, of the type its `world-out` sends, in a
//! container of its own, `<node>_packet`. The types are copied from
//! `tcslib-protocol/xtce/types.xtce` as it states them, and a structure is an
//! aggregate of them, so a ground station is given one file. A string or
//! bytes is sent after a count of its bytes: in XTCE's encoding, which this
//! states and which is what Yamcs is sent, a 32-bit count, as `types.xtce`
//! states it; where the ground reads what it is sent by no XTCE, the same
//! count in groups of seven bits ([`crate::link_type::Count`]). How each
//! gateway frames what it sends -- a CCSDS header and an APID under Yamcs, a
//! port of its own over a radio -- is the ground station's to add.
//!
//! Each attribute a command binding sets is a command, `set_<node>_<attribute>`,
//! laid out as the `SET_NODE` that sets it is, the whole of the datagram:
//! the opcode, the sequence the ground gives it, the node's device id and
//! the attribute's name, each fixed, and the value, an argument of the
//! attribute's type copied from `types.xtce` (see
//! [`crate::graph::settings`]).
//!
//! `tcs-verify --xtce` writes it beside the configuration, and `make check`
//! fails when one committed there is not what it would write: everything it
//! says is said by the configuration and `types.xtce` already, and nobody
//! writes it by hand. See "The configuration's own XTCE" in
//! `docs/tcspecial-design.rst`.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::link_types::{carries, Carries};
use super::telemetry;
use super::Graph;
use crate::link_type::{Basic, LinkType};

/// What every type is copied from: `tcslib-protocol/xtce/types.xtce`.
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
    commands(&mut out, graph)?;
    out.push_str("</SpaceSystem>\n");
    Ok(out)
}

/// The type a command's sequence is: what `link.xtce` sends it as.
const SEQUENCE_TYPE: Basic = Basic::U32;

/// Writes a command for each attribute a binding sets, where there is one.
///
/// # Arguments
///
/// * `out`   - Where they are written.
/// * `graph` - The configuration.
///
/// # Errors
///
/// A message where `types.xtce` does not state a type to copy, or a setting
/// is of a type with no width of its own, which a command here cannot lay
/// out: no node takes one.
fn commands(out: &mut String, graph: &Graph) -> Result<(), String> {
    // Each attribute once, by its node and name: one bound from two
    // gateway nodes is one thing the ground may set. One the node does not
    // take is the configuration's check's to refuse.
    let mut bound: BTreeMap<(String, String), Basic> = BTreeMap::new();
    for binding in &graph.commands {
        let target = binding.target();
        let Some(attribute) = target.input else {
            continue;
        };
        let Ok(kind) = super::settings::of(graph, &target.node, &attribute)
        else {
            continue;
        };
        let basic = match kind {
            LinkType::Basic(basic) if basic.width().is_some() => basic,
            other => {
                return Err(format!(
                    "\"{}.{attribute}\" is set as {other}, and a command here \
                     lays out a value of a type with a width of its own",
                    target.node
                ))
            }
        };
        bound.insert((target.node, attribute), basic);
    }
    if bound.is_empty() {
        return Ok(());
    }
    let ids = telemetry::ids(graph);

    let level = INDENT.repeat(2);
    let inner = INDENT.repeat(3);
    let _ = writeln!(out, "{INDENT}<CommandMetaData>");
    let _ = writeln!(out, "{level}<ArgumentTypeSet>");
    let mut used: Vec<Basic> = bound.values().copied().collect();
    used.push(SEQUENCE_TYPE);
    for (element, copied) in copied_types()? {
        if used.iter().any(|basic| basic.name() == element) {
            out.push_str(&as_argument_type(&copied));
        }
    }
    let _ = writeln!(out, "{level}</ArgumentTypeSet>");
    let _ = writeln!(out, "{level}<MetaCommandSet>");
    let opcode = crate::CommandType::SetNode.to_u8();
    for ((node, attribute), basic) in &bound {
        let name = format!("set_{node}_{attribute}");
        let id = ids.get(node).copied().unwrap_or_default();
        let width = basic.width().expect("a width, checked above");
        let mut said = format!("{:02X}", attribute.len());
        for byte in attribute.bytes() {
            let _ = write!(said, "{byte:02X}");
        }
        let entries = INDENT.repeat(6);
        let _ = writeln!(out, "{inner}<MetaCommand name=\"{name}\">");
        let _ = writeln!(out, "{inner}{INDENT}<ArgumentList>");
        for (argument, kind) in
            [("Sequence", SEQUENCE_TYPE.name()), ("Value", basic.name())]
        {
            let _ = writeln!(
                out,
                "{inner}{INDENT}{INDENT}<Argument name=\"{argument}\" \
                 argumentTypeRef=\"{kind}\"/>"
            );
        }
        let _ = writeln!(out, "{inner}{INDENT}</ArgumentList>");
        let container = format!("{name}_packet");
        if inner.len() + INDENT.len() + container.len() + 30 <= 80 {
            let _ = writeln!(
                out,
                "{inner}{INDENT}<CommandContainer name=\"{container}\">"
            );
        } else {
            let _ = writeln!(
                out,
                "{inner}{INDENT}<CommandContainer\n\
                 {inner}{INDENT}{INDENT}{INDENT}name=\"{container}\">"
            );
        }
        let _ = writeln!(out, "{inner}{INDENT}{INDENT}<EntryList>");
        fixed(out, &entries, "Opcode", &format!("{opcode:02X}"), 8);
        let _ = writeln!(
            out,
            "{entries}<ArgumentRefEntry argumentRef=\"Sequence\"/>"
        );
        fixed(out, &entries, "Node", &format!("{id:08X}"), 32);
        fixed(out, &entries, "Attribute", &said, 8 * (1 + attribute.len()));
        fixed(out, &entries, "ValueLength", &format!("{width:04X}"), 16);
        let _ =
            writeln!(out, "{entries}<ArgumentRefEntry argumentRef=\"Value\"/>");
        let _ = writeln!(out, "{inner}{INDENT}{INDENT}</EntryList>");
        let _ = writeln!(out, "{inner}{INDENT}</CommandContainer>");
        let _ = writeln!(out, "{inner}</MetaCommand>");
    }
    let _ = writeln!(out, "{level}</MetaCommandSet>");
    let _ = writeln!(out, "{INDENT}</CommandMetaData>");
    Ok(())
}

/// Writes an entry of a command that is always the same bits.
///
/// # Arguments
///
/// * `out`   - Where it is written.
/// * `at`    - Its indent.
/// * `name`  - What it is.
/// * `value` - Its bits, in hexadecimal.
/// * `bits`  - How many there are.
fn fixed(out: &mut String, at: &str, name: &str, value: &str, bits: usize) {
    let one_line = format!(
        "{at}<FixedValueEntry name=\"{name}\" binaryValue=\"{value}\" \
         sizeInBits=\"{bits}\"/>"
    );
    if one_line.len() <= 80 {
        let _ = writeln!(out, "{one_line}");
    } else {
        // An attribute's name may be long: its value goes on lines of its
        // own, as a long value does in a hand-written file.
        let _ = writeln!(
            out,
            "{at}<FixedValueEntry name=\"{name}\" sizeInBits=\"{bits}\"\n\
             {at}{INDENT}{INDENT}binaryValue=\"{value}\"/>"
        );
    }
}

/// Turns one of `types.xtce`'s parameter types into the argument type of
/// the same name: the same encoding, units and calibration, as a command's
/// argument is a datum of the type a link carries.
///
/// # Arguments
///
/// * `copied` - The parameter type's element, as `types.xtce` writes it.
///
/// # Returns
///
/// The argument type's element.
fn as_argument_type(copied: &str) -> String {
    let mut out = String::with_capacity(copied.len());
    for line in copied.lines() {
        let trimmed = line.trim_start();
        let changed = if trimmed.starts_with('<')
            && trimmed
                .split([' ', '>'])
                .next()
                .is_some_and(|tag| tag.ends_with("ParameterType"))
        {
            line.replacen("ParameterType", "ArgumentType", 1)
        } else {
            line.to_string()
        };
        out.push_str(&changed);
        out.push('\n');
    }
    out
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
     Each attribute a command binding sets is a command, as the SET_NODE
     that sets it goes, its value an argument of the attribute's type.
     The types are tcslib-protocol/xtce/types.xtce's, copied: a string or bytes
     after a 32-bit count of its bytes, as each is sent where the ground
     decodes it by XTCE (under Yamcs). Where no XTCE decodes it, the count
     is sent in groups of seven bits instead, least significant first, each
     in a byte whose top bit says another follows. How each gateway frames
     what it sends is the ground station's to add.

     Written by tcs-verify, asked for it by its xtce option, from the
     configuration and types.xtce, and not to be edited: see \"The
     configuration's own XTCE\" in tcspecial's docs/tcspecial-design.rst. -->
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
/// sends bytes: the count `types.xtce` sizes every `bytes` by.
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

/// Writes a container for each source, holding its parameter, after
/// `bytes_length` for bytes: a string's count is its type's own.
///
/// # Arguments
///
/// * `out`     - Where they are written.
/// * `sources` - Each source's type, by its name.
fn containers(out: &mut String, sources: &BTreeMap<String, LinkType>) {
    let level = INDENT.repeat(2);
    let _ = writeln!(out, "{level}<ContainerSet>");
    for (source, kind) in sources {
        let inner = INDENT.repeat(3);
        let _ = writeln!(
            out,
            "{inner}<SequenceContainer name=\"{}\">",
            container_name(source)
        );
        let _ = writeln!(out, "{inner}{INDENT}<EntryList>");
        if *kind == LinkType::Basic(Basic::Bytes) {
            let _ = writeln!(
                out,
                "{inner}{INDENT}{INDENT}<ParameterRefEntry \
                 parameterRef=\"{BYTES_LENGTH}\"/>"
            );
        }
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

    /// A binding's command is the SET_NODE that sets it, whole: its value an
    /// argument of the type the thermostat's setpoint is, a kelvin.
    #[test]
    fn a_binding_is_its_set_node() {
        let written = written(&thermostat(), "thermostat").expect("written");
        assert!(
            written.contains("<MetaCommand name=\"set_thermostat_setpoint\">"),
            "{written}"
        );
        assert!(written.contains("<FloatArgumentType name=\"kelvin\""));
        assert!(written.contains("<IntegerArgumentType name=\"u32\""));
        // "setpoint", after its length, eight.
        assert!(
            written.contains("binaryValue=\"08736574706F696E74\""),
            "{written}"
        );
        assert!(written.contains("binaryValue=\"0004\""), "{written}");
    }

    /// What a binding's command lays out, its fixed entries and its two
    /// arguments in order, is the datagram tcslib-protocol packs for that
    /// SET_NODE.
    #[test]
    fn a_bindings_command_packs_as_its_set_node_does() {
        let graph = thermostat();
        let written = written(&graph, "thermostat").expect("written");
        let command = written
            .split("<MetaCommand name=\"set_thermostat_setpoint\">")
            .nth(1)
            .expect("the command");
        let entries = command.split("</EntryList>").next().expect("entries");
        let sequence = 5u32;
        let value = crate::node::vehicle::compute::value_bytes(
            293.15,
            crate::unit::Unit::Kelvin,
        )
        .expect("a temperature");
        let mut packed = Vec::new();
        for entry in entries.split('<').skip(1) {
            if entry.starts_with("ArgumentRefEntry argumentRef=\"Sequence\"") {
                packed.extend_from_slice(&sequence.to_be_bytes());
            } else if entry
                .starts_with("ArgumentRefEntry argumentRef=\"Value\"")
            {
                packed.extend_from_slice(&value);
            } else if entry.starts_with("FixedValueEntry") {
                let hex = entry
                    .split("binaryValue=\"")
                    .nth(1)
                    .and_then(|rest| rest.split('"').next())
                    .expect("its bits");
                for at in (0..hex.len()).step_by(2) {
                    packed.push(
                        u8::from_str_radix(&hex[at..at + 2], 16).expect("hex"),
                    );
                }
            }
        }
        let id = telemetry::ids(&graph)["thermostat"];
        let theirs = crate::Command::SetNode(
            crate::SetNodeCommand::new(
                sequence,
                crate::NodeId::of(id),
                "setpoint",
                &value,
            )
            .expect("it fits"),
        )
        .to_bytes();
        assert_eq!(packed, theirs);
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
