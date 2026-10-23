//! The payloads of a configuration that a simulator stands in for.
//!
//! What the simulator takes of a payload is a [`DHConfig`]: where the payload
//! is, how big a packet is, and whether it sends of its own accord or answers
//! a trigger. This makes one from a payload node, so that every stand-in the
//! simulator has -- a socket, a pty, an I2C stub, a CAN bus -- needs to know
//! nothing of the graph.
//!
//! Where the payload is comes from [`reach_of`], which is what tcspecial opens
//! a node with: so the simulator stands where the vehicle reaches. A node with
//! a trigger answers one, as it does aboard.
//!
//! A configuration may leave payloads unsimulated on purpose -- a payload that
//! is a program is the program itself, and a payload on real hardware needs no
//! stand-in -- so only the payloads the simulator file names are simulated,
//! and a file naming none is one with nothing to do.

use super::reach::reach_of;
use super::{parse_interval, Destination, Graph};
use crate::sim_config::SimConfigFile;
use crate::types::{DHConfig, DHMode, DHName};

/// How big a simulated packet is where the node does not say.
///
/// A node states how much one read asks for as `read_size` when the part on
/// the other end decides it; one that does not -- a socket, whose reads are
/// bounded by what arrives -- leaves the size to the simulator, and this is
/// a reading's worth of bytes.
pub const DEFAULT_PACKET_BYTES: usize = 16;

/// What the simulator takes for each payload node the simulator file names, in
/// the order the configuration declares them.
///
/// A name in the simulator file that is not a payload node, or a payload node
/// the simulator cannot stand in for, is refused, saying which.
/// How a reading of this payload is encoded, from the conversion it feeds.
///
/// A simulated sensor attached to a thermal mass has to send bytes the vehicle
/// will read back as the mass's temperature, and what those bytes are is not
/// the sensor's own business: the payload says `read_size` and nothing else,
/// and the format, the scale and the offset are on the `to_kelvin` node its
/// readings go to. So the encoding is taken from there.
///
/// Taken rather than stated again in the simulator file. The two files are
/// joined by name and nothing else, and a scale written in both is a scale
/// that can disagree with itself -- which would show up as a simulated
/// temperature the vehicle reads back as a different one, with both files
/// looking right.
///
/// Refused where there is not exactly one conversion to take it from: a
/// sensor feeding none has no encoding to borrow, and one feeding two that
/// disagree has no single answer.
pub fn reading_of(
    graph: &Graph,
    payload: &str,
) -> Result<crate::node::vehicle::compute::ToKelvinParams, String> {
    use crate::node::vehicle::compute::{params_of, Computation};

    let mut found: Vec<(String, crate::node::vehicle::compute::ToKelvinParams)> = Vec::new();
    for link in graph.links.iter().filter(|l| l.origin().node == payload) {
        for destination in link.to.each() {
            let to = Destination::parse(destination).node;
            let Some(node) = graph.node(&to) else {
                continue;
            };
            if node.node_type != "compute" {
                continue;
            }
            if let Ok(Computation::ToKelvin(params)) = params_of(&node.name, &node.attributes) {
                if !found.iter().any(|(name, _)| name == &to) {
                    found.push((to, params));
                }
            }
        }
    }

    match found.len() {
        1 => Ok(found.remove(0).1),
        0 => Err(format!(
            "\"{payload}\" reads a thermal mass, and nothing converts its \
             readings: a simulated sensor sends the bytes a to_kelvin node \
             would read back as the temperature, so there has to be one to \
             take the format, the scale and the offset from"
        )),
        _ => {
            let named: Vec<&str> = found.iter().map(|(name, _)| name.as_str()).collect();
            Err(format!(
                "\"{payload}\" reads a thermal mass and its readings go to {} \
                 conversions ({}): there is no one encoding to send them in",
                found.len(),
                named.join(", ")
            ))
        }
    }
}

pub fn handlers(graph: &Graph, sim_file: &SimConfigFile) -> Result<Vec<DHConfig>, String> {
    for entry in &sim_file.simulated_payloads {
        match graph.node(&entry.name) {
            Some(node) if node.is_payload() => {}
            Some(node) => {
                return Err(format!(
                    "\"{}\" is simulated, and it is a {} node, not a payload",
                    entry.name, node.node_type
                ))
            }
            None => {
                return Err(format!(
                    "\"{}\" is simulated, and the configuration has no node of that name",
                    entry.name
                ))
            }
        }
    }

    // Those whose simulate is true. One that is false is the real device,
    // which tcssim makes nothing for; one whose simulate is missing or stated
    // twice is refused where the file is checked, and is not simulated here.
    let named = |name: &str| sim_file.simulates(name);
    let mut handlers = Vec::new();
    for node in graph
        .nodes
        .iter()
        .filter(|n| n.is_payload() && named(&n.name))
    {
        // A payload that is only driven -- a heater, a valve -- is written to
        // and never read. Its stand-in sends nothing and watches what
        // arrives, counting it, so that it is shown and switched as every
        // other simulated payload is; one that warms a thermal mass warms it
        // as well. See `crate::types::DHConfig::driven`.
        let leaves = graph.links.iter().any(|l| l.origin().node == node.name);
        let arrives = graph.links.iter().any(|l| {
            l.to.each()
                .iter()
                .any(|d| Destination::parse(d).node == node.name)
        });

        // A payload that is a program is the program itself: tcspecial runs
        // it, and there is no device or socket for a simulator to stand in
        // for.
        if crate::node::app::Kind::of(&node.node_type).is_some() {
            return Err(format!(
                "\"{}\" is an app node, a program tcspecial runs rather than \
                 something it reaches, so there is nothing for a simulator to \
                 stand in for: leave it out of the simulator file",
                node.name
            ));
        }

        let reach = reach_of(node)
            .map_err(|why| format!("\"{}\" cannot be simulated: {why}", node.name))?;

        let mode = match reach.trigger {
            None => DHMode::Periodic,
            Some(trigger) => {
                // How often it is asked is how often its group is read; a
                // triggered node in no group is asked as it is polled.
                let interval = node
                    .group
                    .as_deref()
                    .and_then(|g| graph.group(g))
                    .and_then(|g| parse_interval(&g.interval).ok())
                    .unwrap_or_default();
                DHMode::Triggered { trigger, interval }
            }
        };

        handlers.push(DHConfig {
            name: DHName::new(node.name.clone()),
            endpoint: reach.endpoint,
            packet_size: reach
                .read_size
                .map_or(DEFAULT_PACKET_BYTES, |size| size as usize),
            oc: None,
            ground: None,
            mode,
            stale_interval: None,
            sends_from: reach.sends_from,
            stand_in: reach.stand_in,
            driven: arrives && !leaves,
        });
    }
    Ok(handlers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn example(name: &str) -> (Graph, SimConfigFile) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let graph = Graph::load(root.join(format!("examples/{name}/{name}.yaml")))
            .expect("the example reads");
        let sim = SimConfigFile::load(root.join(format!("examples/{name}/{name}-sim.yaml")))
            .expect("its simulator file reads");
        (graph, sim)
    }

    /// The minimal example's sensor is simulated where tcspecial reaches it.
    #[test]
    fn the_minimal_sensor_is_simulated_where_the_vehicle_reaches_it() {
        let (graph, sim) = example("minimal");
        let handlers = handlers(&graph, &sim).expect("it simulates");
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].name.0, "sensor");
        assert_eq!(
            handlers[0].endpoint,
            reach_of(graph.node("sensor").expect("a sensor"))
                .expect("it reaches")
                .endpoint
        );
        assert_eq!(handlers[0].packet_size, DEFAULT_PACKET_BYTES);
        assert_eq!(handlers[0].mode, DHMode::Periodic);
    }

    /// A payload that is a program is not simulated, and a file naming none
    /// is one with nothing to do.
    #[test]
    fn simple_app_simulates_nothing() {
        let (graph, sim) = example("simple-app");
        assert_eq!(handlers(&graph, &sim), Ok(Vec::new()));
    }

    /// The thermostat's sensors are simulated with the sizes their nodes say.
    #[test]
    fn the_thermostat_sensors_are_simulated_at_their_read_sizes() {
        let (graph, sim) = example("thermostat");
        let handlers = handlers(&graph, &sim).expect("it simulates");
        let sized: Vec<(&str, usize)> = handlers
            .iter()
            .map(|h| (h.name.0.as_str(), h.packet_size))
            .collect();
        assert!(sized.contains(&("temp_1", 2)), "{sized:?}");
        assert!(sized.contains(&("temp_3", 3)), "{sized:?}");
    }
}
