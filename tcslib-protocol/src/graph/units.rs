//! What unit each node's values are in, as the configuration says.
//!
//! The unit of a value is never on the link -- see [`crate::unit`] -- and it
//! is not worked out from what feeds a node either: every node states the
//! type of each of its terminals, and a value type is a unit (see
//! [`crate::graph::link_types`]). This is where the vehicle, which decodes
//! what arrives at a vote or a thermostat, and the ground, which shows a
//! reading with its unit, ask for it:
//!
//! * a conversion's is its output's: `to_kelvin` is in kelvin, `to_value`
//!   states its `type-out`;
//! * a `process`'s is its `type-out`, a vote's what it `carries`;
//! * a `thermostat` reads values of its `type-in` and sends decisions, and
//!   its setpoint and band are in the unit of its `type-in`, which is
//!   checked here.

use super::link_types::{sends_unit, takes_unit};
use super::Graph;
use crate::node::vehicle::compute::{
    params_of, value_count, Computation, ThermostatParams,
};
use crate::unit::Unit;

/// Says what unit a node's values are in.
///
/// # Arguments
///
/// * `graph` - The configuration.
/// * `node`  - The node's name.
///
/// # Returns
///
/// The unit, or `None` for a node that does not send values -- a sensor's
/// own numbers, a decision, a log -- or that is not in the configuration.
pub fn unit_of(graph: &Graph, node: &str) -> Option<Unit> {
    graph.node(node).and_then(sends_unit)
}

/// Says what unit the values arriving at a node are in.
///
/// # Arguments
///
/// * `graph` - The configuration.
/// * `node`  - The node's name.
///
/// # Returns
///
/// The unit of the value type the node takes, at its `world-in` or, for a
/// device the vehicle drives, its `moc-in`; `None` where it takes no values.
pub fn input_unit(graph: &Graph, node: &str) -> Option<Unit> {
    graph.node(node).and_then(takes_unit)
}

/// Everything wrong with the units of a configuration's computations: a
/// thermostat's setpoint and band must fit the unit of what it reads, which
/// is the range check that unit's values are held to.
///
/// That what reaches a vote, a thermostat or a program is a value, and in
/// one unit, is the links' to check, and they do: see
/// [`crate::graph::link_types`].
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// Each problem, with the node it is about.
pub fn problems(graph: &Graph) -> Vec<(String, String)> {
    let mut problems = Vec::new();
    for node in graph.nodes.iter().filter(|n| n.node_type == "compute") {
        let Ok(Computation::Thermostat(params)) =
            params_of(&node.name, &node.attributes)
        else {
            continue;
        };
        if let Some(unit) = takes_unit(node) {
            band_fits(&node.name, &params, unit, &mut problems);
        }
    }
    problems
}

/// Checks that a thermostat's setpoint and band are numbers its input's
/// unit can carry.
///
/// # Arguments
///
/// * `node`     - The thermostat's name.
/// * `params`   - Its setpoint and half-band.
/// * `unit`     - The unit of what it reads.
/// * `problems` - Where a problem is added.
fn band_fits(
    node: &str,
    params: &ThermostatParams,
    unit: Unit,
    problems: &mut Vec<(String, String)>,
) {
    for (name, number) in [
        ("setpoint", params.setpoint),
        ("half_band", params.half_band),
    ] {
        if let Err(why) = value_count(number, unit) {
            problems.push((
                node.to_string(),
                format!(
                    "{name}: {why}, which is the unit of what it reads, so \
                     no reading could reach it"
                ),
            ));
        }
    }
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

    /// Changes one node's attribute in a configuration.
    ///
    /// # Arguments
    ///
    /// * `graph`     - The configuration.
    /// * `node`      - The node's name.
    /// * `attribute` - The attribute.
    /// * `value`     - What it is set to.
    fn with(graph: &mut Graph, node: &str, attribute: &str, value: &str) {
        graph
            .nodes
            .iter_mut()
            .find(|n| n.name == node)
            .expect("the node")
            .attributes
            .insert(attribute.to_string(), crate::Scalar::of(value));
    }

    /// A conversion's unit is its own, a vote's what it carries, and a decision
    /// or a sensor's numbers have none.
    #[test]
    fn each_node_sends_the_unit_the_configuration_says() {
        let graph = thermostat();
        assert_eq!(unit_of(&graph, "to_k1"), Some(Unit::Kelvin));
        assert_eq!(unit_of(&graph, "consensus_temp"), Some(Unit::Kelvin));
        assert_eq!(unit_of(&graph, "thermostat"), None);
        assert_eq!(unit_of(&graph, "temp_1"), None);
        assert_eq!(unit_of(&graph, "nobody"), None);
        assert_eq!(input_unit(&graph, "thermostat"), Some(Unit::Kelvin));
        assert!(problems(&graph).is_empty(), "{:?}", problems(&graph));
    }

    /// A vote is in the unit it states, whatever feeds it: that what feeds
    /// it agrees is the links' to check.
    #[test]
    fn a_vote_is_in_the_unit_it_states() {
        let mut graph = thermostat();
        with(&mut graph, "consensus_temp", "carries", "volt");
        assert_eq!(unit_of(&graph, "consensus_temp"), Some(Unit::Volt));
        let found = super::super::link_types::problems(&graph);
        assert!(
            found.iter().any(|(node, why)| node == "consensus_temp"
                && why.contains("kelvin")
                && why.contains("volt")),
            "{found:?}"
        );
    }

    /// A setpoint its input's unit cannot carry is refused.
    #[test]
    fn a_setpoint_no_reading_could_reach_is_refused() {
        let mut graph = thermostat();
        with(&mut graph, "thermostat", "setpoint", "3000");
        let found = problems(&graph);
        assert!(
            found
                .iter()
                .any(|(node, why)| node == "thermostat"
                    && why.contains("setpoint")),
            "{found:?}"
        );
    }
}
