//! What the ground may set aboard: each attribute a command binding names,
//! and the type its value is.
//!
//! A `SET_NODE` carries an attribute's value as a datum of the attribute's
//! type, in that type's own encoding, as `types.xtce` states it -- a
//! thermostat's `setpoint` as the value type its `type-in` is, four bytes of
//! a count of the unit's steps -- and not as text. So a value the ground
//! sends is held to its type as every datum on a link is, and refused by its
//! width before anything reads it. Only a thermostat takes settings: its
//! `setpoint` and its `half_band`, each in the unit of what it reads. A
//! binding naming anything else is refused where the configuration is read.
//! See "Typed links" in `docs/tcspecial-design.rst`.

use super::{Destination, Graph, Node};
use crate::link_text::read_text;
use crate::link_type::LinkType;
use crate::node::vehicle::compute::stated_type;

/// What a thermostat takes from the ground.
const THERMOSTAT_SETS: [&str; 2] = ["setpoint", "half_band"];

/// Says what type an attribute of a node is set from the ground as.
///
/// # Arguments
///
/// * `node`      - The node.
/// * `attribute` - The attribute.
///
/// # Returns
///
/// Its type: for a thermostat's `setpoint` and `half_band`, its `type-in`.
///
/// # Errors
///
/// A message naming the node where it takes no such setting, or the type it
/// would be cannot be read.
pub fn type_of(node: &Node, attribute: &str) -> Result<LinkType, String> {
    let thermostat = node.node_type == "compute"
        && node
            .attributes
            .get("function")
            .is_some_and(|said| said.as_str() == "thermostat");
    if thermostat && THERMOSTAT_SETS.contains(&attribute) {
        return stated_type(&node.attributes, "type-in")
            .map_err(|why| format!("\"{}\": {why}", node.name));
    }
    let takes = if thermostat {
        format!("a thermostat takes {}", THERMOSTAT_SETS.join(" and "))
    } else {
        "only a thermostat takes settings".to_string()
    };
    Err(format!(
        "\"{}\" has nothing called \"{attribute}\" the ground may set: \
         {takes}",
        node.name
    ))
}

/// Finds what type a bound attribute is set as, by the node's name.
///
/// # Arguments
///
/// * `graph`     - The configuration.
/// * `node`      - The node's name.
/// * `attribute` - The attribute.
///
/// # Returns
///
/// Its type.
///
/// # Errors
///
/// As [`type_of`], or a message where the configuration has no such node.
pub fn of(
    graph: &Graph,
    node: &str,
    attribute: &str,
) -> Result<LinkType, String> {
    let found = graph
        .node(node)
        .ok_or_else(|| format!("\"{node}\" is no node of the configuration"))?;
    type_of(found, attribute)
}

/// Writes a value as a `SET_NODE` carries it, from text: the attribute's
/// type, in its own encoding, read from what an operator typed as a
/// `process` writes it (see [`crate::link_text`]).
///
/// # Arguments
///
/// * `graph`     - The configuration.
/// * `node`      - The node's name.
/// * `attribute` - The attribute.
/// * `said`      - The value, as text: `293.15`.
///
/// # Returns
///
/// The value's bytes.
///
/// # Errors
///
/// As [`of`], or a message where the text is not a value of the type.
pub fn encoded(
    graph: &Graph,
    node: &str,
    attribute: &str,
    said: &str,
) -> Result<Vec<u8>, String> {
    let kind = of(graph, node, attribute)?;
    let mut out = [0u8; 8 * crate::link_type::MOST_FIELDS];
    let length = read_text(&kind, said, attribute, &mut out)?;
    Ok(out[..length].to_vec())
}

/// Everything wrong with what the command bindings name: an attribute the
/// node it names takes from no ground, or whose type cannot be read. A
/// binding naming no node, or no attribute, is the check's to say.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// Each problem, with the node it is about.
pub fn problems(graph: &Graph) -> Vec<(String, String)> {
    let mut problems: Vec<(String, String)> = Vec::new();
    for binding in &graph.commands {
        let Destination {
            node,
            input: Some(attribute),
        } = binding.target()
        else {
            continue;
        };
        let Some(found) = graph.node(&node) else {
            continue;
        };
        if let Err(why) = type_of(found, &attribute) {
            if !problems.iter().any(|(_, seen)| *seen == why) {
                problems.push((node, why));
            }
        }
    }
    problems
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

    /// A thermostat's setpoint is its type-in, and is written as that type
    /// is: a kelvin as four bytes of micro-kelvin.
    #[test]
    fn a_setpoint_is_its_thermostats_type_in() {
        let graph = thermostat();
        assert_eq!(
            of(&graph, "thermostat", "setpoint"),
            LinkType::of("kelvin")
        );
        assert_eq!(
            encoded(&graph, "thermostat", "setpoint", "293.15"),
            Ok(293_150_000i32.to_be_bytes().to_vec())
        );
        assert!(problems(&graph).is_empty(), "{:?}", problems(&graph));
    }

    /// What no node takes, and text that is not a value, are refused.
    #[test]
    fn what_cannot_be_set_is_refused() {
        let graph = thermostat();
        let why = of(&graph, "thermostat", "type-out").expect_err("no");
        assert!(why.contains("setpoint and half_band"), "{why}");
        let why = of(&graph, "to_k1", "scale").expect_err("no");
        assert!(why.contains("only a thermostat"), "{why}");
        assert!(encoded(&graph, "thermostat", "setpoint", "warm").is_err());
        let mut graph = graph;
        graph.commands[0].sets = "to_k1.scale".to_string();
        let found = problems(&graph);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].0, "to_k1");
    }
}
