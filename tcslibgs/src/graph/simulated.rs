//! The payloads of a version 2 configuration that a simulator stands in for.
//!
//! The simulator was written for version 1 payload sets, and what it takes of
//! a payload is a [`DHConfig`]: where the payload is, how big a packet is, and
//! whether it sends of its own accord or answers a trigger. This makes the
//! same from a version 2 payload node, so that every stand-in the simulator
//! has -- a socket, a pty, an I2C stub, a CAN bus -- is reused unchanged.
//!
//! Where the payload is comes from [`reach_of`], which is what tcspecial opens
//! a node with: so the simulator stands where the vehicle reaches. A node with
//! a trigger answers one, as it does aboard.
//!
//! A version 2 configuration may leave payloads unsimulated on purpose -- a
//! payload that is a program is the program itself, and a payload on real
//! hardware needs no stand-in -- so only the payloads the simulator file
//! names are simulated, and a file naming none is one with nothing to do.

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

    let named = |name: &str| sim_file.simulated_payloads.iter().any(|e| e.name == name);
    let mut handlers = Vec::new();
    for node in graph.nodes.iter().filter(|n| n.is_payload() && named(&n.name)) {
        // A payload that is only driven -- a heater, a valve -- is written to
        // and never read, and a stand-in for it would be sending data nothing
        // reads into a device the vehicle is trying to drive.
        let leaves = graph.links.iter().any(|l| l.origin().node == node.name);
        let arrives = graph.links.iter().any(|l| {
            l.to.each()
                .iter()
                .any(|d| Destination::parse(d).node == node.name)
        });
        if arrives && !leaves {
            return Err(format!(
                "\"{}\" is driven and never read, so there is nothing for a \
                 simulator to send in its place: leave it out of the simulator file",
                node.name
            ));
        }

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

        let reach = reach_of(node).map_err(|why| {
            format!("\"{}\" cannot be simulated: {why}", node.name)
        })?;

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
