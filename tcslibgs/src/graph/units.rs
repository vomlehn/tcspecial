//! What unit each node's values are in, as the configuration says.
//!
//! The unit of a value is never on the link -- see [`crate::unit`] -- so it
//! is worked out here, once, from the graph, for everything that reads a
//! value back: the vehicle, which decodes what arrives at a vote or a
//! thermostat, and the ground, which shows a reading with its unit. A node
//! says its unit only where nothing else can:
//!
//! * a conversion states it: `to_kelvin` is in kelvin, `to_value` names its
//!   own;
//! * a `process` states it, being a program whose answers are its own
//!   business;
//! * a `vote` is in its inputs' unit, which must be one.
//!
//! A `hysteresis` reads values and sends decisions; its setpoint and band
//! are in the unit of what it reads, and whether they fit is checked here
//! too, since that node alone cannot know the unit.

use super::{Destination, Graph};
use crate::node::vehicle::compute::{
    params_of, value_count, Computation, HysteresisParams,
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
/// own bytes, a decision, a log -- or a vote whose inputs do not agree,
/// which [`problems`] reports.
pub fn unit_of(graph: &Graph, node: &str) -> Option<Unit> {
    unit_seen(graph, node, &mut Vec::new())
}

/// [`unit_of`], with the nodes already being asked about, so that a loop of
/// votes feeding each other ends rather than recursing for ever.
///
/// # Arguments
///
/// * `graph`  - The configuration.
/// * `node`   - The node's name.
/// * `asking` - The nodes whose units are being worked out further up.
///
/// # Returns
///
/// The unit, or `None` as [`unit_of`] says, and for a node already being
/// asked about.
fn unit_seen<'a>(
    graph: &'a Graph,
    node: &'a str,
    asking: &mut Vec<&'a str>,
) -> Option<Unit> {
    if asking.contains(&node) {
        return None;
    }
    let found = graph.node(node)?;
    if found.node_type != "compute" {
        return None;
    }
    match params_of(&found.name, &found.attributes).ok()? {
        Computation::ToKelvin(params) | Computation::ToValue(params) => {
            Some(params.unit)
        }
        Computation::Process(params) => Some(params.unit),
        Computation::Hysteresis(_) => None,
        Computation::Vote(_) => {
            asking.push(node);
            let units = sources_units(graph, node, asking);
            asking.pop();
            match units.as_slice() {
                [(_, Some(first)), rest @ ..]
                    if rest.iter().all(|(_, unit)| *unit == Some(*first)) =>
                {
                    Some(*first)
                }
                _ => None,
            }
        }
    }
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
/// The one unit every node feeding it sends values in, or `None` where
/// nothing feeds it, something feeding it does not send values, or what
/// feeds it disagrees -- each of which [`problems`] reports where it matters.
pub fn input_unit(graph: &Graph, node: &str) -> Option<Unit> {
    let units = sources_units(graph, node, &mut Vec::new());
    let (_, first) = units.first()?;
    let first = (*first)?;
    units
        .iter()
        .all(|(_, unit)| *unit == Some(first))
        .then_some(first)
}

/// The unit each node feeding this one sends values in, by name.
///
/// # Arguments
///
/// * `graph`  - The configuration.
/// * `node`   - The node fed.
/// * `asking` - The nodes whose units are being worked out further up.
///
/// # Returns
///
/// Each node feeding it, once, with the unit it sends values in, or `None`
/// for one that sends none.
fn sources_units<'a>(
    graph: &'a Graph,
    node: &str,
    asking: &mut Vec<&'a str>,
) -> Vec<(&'a str, Option<Unit>)> {
    let mut found: Vec<(&'a str, Option<Unit>)> = Vec::new();
    for link in &graph.links {
        let arrives = link
            .to
            .each()
            .iter()
            .any(|to| Destination::parse(to).node == node);
        if !arrives {
            continue;
        }
        let from = link.from.split('.').next().unwrap_or(&link.from);
        if found.iter().any(|(name, _)| *name == from) {
            continue;
        }
        let unit = unit_seen(graph, from, asking);
        found.push((from, unit));
    }
    found
}

/// Everything wrong with the units of a configuration's computations.
///
/// A vote, a thermostat and a program read what arrives as values, so every
/// node feeding one must send values, and in one unit: a vote between a
/// temperature and a voltage, or a thermostat reading a sensor's raw bytes,
/// is a configuration whose arithmetic is meaningless however well it runs.
/// And a thermostat's setpoint and band must fit its input's unit, which is
/// the range check that unit's values are held to.
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
        let Ok(what) = params_of(&node.name, &node.attributes) else {
            // Refused already, with the reason, by the computations' check.
            continue;
        };
        let reads = match what {
            Computation::Vote(_) => "votes on",
            Computation::Hysteresis(_) => "decides on",
            Computation::Process(_) => "hands its program",
            Computation::ToKelvin(_) | Computation::ToValue(_) => continue,
        };
        let units = sources_units(graph, &node.name, &mut Vec::new());
        let mut sent: Option<(&str, Unit)> = None;
        let mut agree = true;
        for (source, unit) in &units {
            let Some(unit) = unit else {
                problems.push((
                    node.name.clone(),
                    format!(
                        "it {reads} what \"{source}\" sends, which is not a \
                         value: put a conversion -- to_kelvin or to_value -- \
                         between them"
                    ),
                ));
                agree = false;
                continue;
            };
            match sent {
                None => sent = Some((source, *unit)),
                Some((first, theirs)) if theirs != *unit => {
                    problems.push((
                        node.name.clone(),
                        format!(
                            "it {reads} values in {} from \"{first}\" and in \
                             {} from \"{source}\": what arrives at one node \
                             must be in one unit",
                            theirs.name(),
                            unit.name()
                        ),
                    ));
                    agree = false;
                }
                Some(_) => {}
            }
        }

        if let (Computation::Hysteresis(params), Some((_, unit)), true) =
            (&what, sent, agree)
        {
            band_fits(&node.name, params, unit, &mut problems);
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
    params: &HysteresisParams,
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

    /// A conversion's unit is its own, a vote's its inputs', and a decision
    /// or a sensor's bytes have none.
    #[test]
    fn each_node_sends_the_unit_the_configuration_says() {
        let graph = thermostat();
        assert_eq!(unit_of(&graph, "temp1"), Some(Unit::Kelvin));
        assert_eq!(unit_of(&graph, "ambient"), Some(Unit::Kelvin));
        assert_eq!(unit_of(&graph, "thermostat_1"), None);
        assert_eq!(unit_of(&graph, "temp_1"), None);
        assert_eq!(unit_of(&graph, "nobody"), None);
        assert_eq!(input_unit(&graph, "thermostat_1"), Some(Unit::Kelvin));
        assert!(problems(&graph).is_empty(), "{:?}", problems(&graph));
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

    /// A vote between two units is refused, naming both and their sources,
    /// and is in no unit.
    #[test]
    fn a_vote_between_two_units_is_refused() {
        let mut graph = thermostat();
        with(&mut graph, "temp3", "function", "to_value");
        with(&mut graph, "temp3", "unit", "V");
        assert_eq!(unit_of(&graph, "ambient"), None);
        let found = problems(&graph);
        assert!(
            found.iter().any(|(node, why)| node == "ambient"
                && why.contains("kelvin")
                && why.contains("volts")
                && why.contains("temp3")),
            "{found:?}"
        );
    }

    /// A thermostat reading a sensor's own bytes is refused, told to put a
    /// conversion between them.
    #[test]
    fn a_thermostat_reading_bytes_is_refused() {
        let mut graph = thermostat();
        for link in &mut graph.links {
            if link.from.starts_with("ambient") {
                link.from = "temp_1".to_string();
            }
        }
        let found = problems(&graph);
        assert!(
            found.iter().any(|(node, why)| node == "thermostat_1"
                && why.contains("temp_1")
                && why.contains("not a value")),
            "{found:?}"
        );
    }

    /// A setpoint its input's unit cannot carry is refused.
    #[test]
    fn a_setpoint_no_reading_could_reach_is_refused() {
        let mut graph = thermostat();
        with(&mut graph, "thermostat_1", "setpoint", "3000");
        let found = problems(&graph);
        assert!(
            found.iter().any(|(node, why)| node == "thermostat_1"
                && why.contains("setpoint")),
            "{found:?}"
        );
    }
}
