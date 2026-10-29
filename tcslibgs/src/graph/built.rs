//! Which payload types a tcspecial contains, and the configurations it can
//! therefore fly.
//!
//! A payload type is chosen when tcspecial is built, as a Cargo feature named
//! as the type is written in a configuration: see `tcspecial/Cargo.toml` and
//! `tcspecial::node::payload::BUILT`. The rule about what a build can fly is
//! here, beside the other rules about configurations, because two programs ask
//! it: tcspecial of the configuration it is about to load, with the types it
//! was built with, and tcsverify of a configuration on the ground, with the
//! types `--types` names. One rule and one sentence, so the ground is told
//! what the vehicle would say.
//!
//! What a configuration may say about a type is not here and is not gated.
//! Those rules are built into every program, because the ground checks
//! configurations for any vehicle, whatever its tcspecial contains.

use super::Node;
use crate::graph::check::Problem;
use crate::types::{DHType, RAW};

/// Every payload type a build may leave out, as a configuration writes it.
///
/// The types of `DHType` but the network type, and the raw protocol of the
/// network type. The network type over UDP, TCP and Unix-domain sockets is in
/// every build, because the link to the ground is UDP; and so is every node
/// that reaches no interface. These words are the Cargo features' names too,
/// which a test of tcspecial's holds them to.
pub const OPTIONAL_TYPES: [&str; 10] = [
    "device-file",
    "serial",
    "i2c",
    "spi",
    "can",
    "iio",
    "rpmsg",
    "gpio",
    "hdlc",
    RAW,
];

/// Finds the optional type a node needs a build to contain, if it needs one.
///
/// A node of a payload type needs that type, and a network node needs `raw`
/// where its protocol is raw and nothing otherwise. Every other node -- a
/// gateway, a computation, a log, an app -- needs nothing.
///
/// # Arguments
///
/// * `node` - The node.
///
/// # Returns
///
/// The type's spelling, or `None` when the node needs none.
pub fn needs(node: &Node) -> Option<&'static str> {
    match DHType::from_spelling(&node.node_type)? {
        DHType::Network => node
            .attributes
            .get("protocol")
            .filter(|protocol| protocol.as_str() == RAW)
            .map(|_| RAW),
        kind => Some(kind.spelling()),
    }
}

/// Finds every node whose type a build does not contain.
///
/// # Arguments
///
/// * `nodes` - A graph's nodes.
/// * `built` - The optional types the build contains: see
///             [`OPTIONAL_TYPES`].
///
/// # Returns
///
/// A [`Problem::NotBuilt`] for each node whose type is not built.
pub fn not_built(nodes: &[Node], built: &[&str]) -> Vec<Problem> {
    nodes
        .iter()
        .filter_map(|node| {
            let node_type = needs(node)?;
            (!built.contains(&node_type)).then(|| Problem::NotBuilt {
                node: node.name.clone(),
                node_type,
            })
        })
        .collect()
}

/// Says what is said of a node whose type a build does not contain.
///
/// The one sentence for it, which tcspecial says as it refuses a configuration
/// at load, tcsverify says of a configuration checked with `--types`, and the
/// open says if a node ever reaches it unchecked.
///
/// # Arguments
///
/// * `node`      - The node's name.
/// * `node_type` - Its type, which the build lacks.
///
/// # Returns
///
/// The message, saying how to build for it.
pub fn not_built_message(node: &str, node_type: &str) -> String {
    let article = match node_type {
        "i2c" | "iio" => "an",
        _ => "a",
    };
    format!(
        "\"{node}\" is {article} {node_type} node, and the tcspecial it is for \
         is built without {node_type}: build tcspecial with --features \
         {node_type}, or take the node out of the configuration"
    )
}

/// Reads the optional types a list names, as `--types` gives them: words
/// separated by commas, as `tcspecial --types` prints them.
///
/// An empty list is a build with none of them, which is a build that can fly
/// a configuration of network nodes and nothing else.
///
/// # Arguments
///
/// * `list` - The list as given.
///
/// # Returns
///
/// The types it names, each once, in the order first named.
///
/// # Errors
///
/// A word that names no optional type is refused, listing those that do: a
/// misspelling taken as a type the build lacks would report every node of
/// the type that was meant.
pub fn types_from(list: &str) -> Result<Vec<&'static str>, String> {
    let mut types = Vec::new();
    for word in list
        .split(',')
        .map(str::trim)
        .filter(|word| !word.is_empty())
    {
        match OPTIONAL_TYPES.iter().find(|known| **known == word) {
            Some(known) => {
                if !types.contains(known) {
                    types.push(*known);
                }
            }
            None => {
                return Err(format!(
                    "\"{word}\" is not a payload type a build may leave out: \
                     those are {}",
                    OPTIONAL_TYPES.join(", ")
                ))
            }
        }
    }
    Ok(types)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Makes a node of a type, with a protocol where one is given.
    ///
    /// # Arguments
    ///
    /// * `name`      - The node's name.
    /// * `node_type` - Its type.
    /// * `protocol`  - Its `protocol` attribute, if it has one.
    ///
    /// # Returns
    ///
    /// The node, with no group.
    fn node(name: &str, node_type: &str, protocol: Option<&str>) -> Node {
        Node {
            name: name.into(),
            node_type: node_type.into(),
            group: None,
            attributes: protocol
                .map(|protocol| {
                    ("protocol".to_string(), crate::Scalar::of(protocol))
                })
                .into_iter()
                .collect(),
        }
    }

    /// Every payload type but the network type may be left out, and nothing
    /// else may: a word here that named no type would be a feature nothing
    /// could need.
    #[test]
    fn the_optional_types_are_the_payload_types_and_raw() {
        let mut expected: Vec<&str> = DHType::ALL
            .into_iter()
            .filter(|kind| *kind != DHType::Network)
            .map(|kind| kind.spelling())
            .collect();
        expected.push(RAW);
        assert_eq!(OPTIONAL_TYPES.to_vec(), expected);
    }

    #[test]
    fn a_node_needs_its_own_type_and_a_raw_socket_needs_raw() {
        let nodes = [
            node("temp", "i2c", None),
            node("link", "network", Some("udp")),
            node("sniff", "network", Some("raw")),
            node("mean", "compute", None),
        ];
        let needed: Vec<Option<&str>> = nodes.iter().map(needs).collect();
        assert_eq!(needed, vec![Some("i2c"), None, Some("raw"), None]);
    }

    #[test]
    fn a_node_of_a_type_not_built_is_refused_and_nothing_else_is() {
        let nodes = [
            node("temp", "i2c", None),
            node("line", "serial", None),
            node("link", "network", Some("udp")),
        ];
        assert_eq!(
            not_built(&nodes, &["serial"]),
            vec![Problem::NotBuilt {
                node: "temp".into(),
                node_type: "i2c",
            }]
        );
        assert!(not_built(&nodes, &OPTIONAL_TYPES).is_empty());
    }

    /// The refusal names the node, its type and the feature that would build
    /// it, which is what somebody standing at the vehicle needs to do next.
    #[test]
    fn the_refusal_says_which_feature_would_build_it() {
        let said = not_built_message("temp_1", "i2c");
        assert!(said.contains("\"temp_1\" is an i2c node"), "{said}");
        assert!(said.contains("--features i2c"), "{said}");
    }

    #[test]
    fn a_list_of_types_is_read_and_a_word_that_is_none_is_refused() {
        assert_eq!(types_from("serial,i2c"), Ok(vec!["serial", "i2c"]));
        assert_eq!(types_from(" serial , serial "), Ok(vec!["serial"]));
        assert_eq!(types_from(""), Ok(vec![]));
        let said = types_from("serial,i3c").expect_err("i3c is no type");
        assert!(said.contains("\"i3c\""), "{said}");
        assert!(said.contains("device-file"), "{said}");
    }
}
