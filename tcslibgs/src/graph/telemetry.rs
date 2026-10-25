//! Where the vehicle's data goes on the ground, and on which port.
//!
//! What reaches the ground leaves the vehicle over links whose far ends are
//! written down, so the vehicle sends from the first reading without being
//! spoken to, and each source arrives at a port of its own -- which is how the
//! ground tells one node's data from another's. The port is a base plus the
//! source node's device id.
//!
//! # Device ids
//!
//! Every node has one: its position among the configuration's nodes, counting
//! from nought, in the order the files declare them -- the order they are
//! written, whatever the sections are, the top file first and then the files
//! it includes, in the order it names them (see `crate::graph::order`). So
//! adding or removing a node moves the ids of the nodes written after it, and
//! a ground station working from the same file counts the same way.
//!
//! # Two kinds of link
//!
//! * A link drawn into a `moc` node. What the source node produces is sent to
//!   the `moc` node's `ground_base` host, at its port plus the source's id. The
//!   vehicle's end is whatever port the system gives it: what the ground
//!   listens on is what is written down.
//!
//! * A payload that nothing in the drawing touches -- no link to it or from
//!   it, no command binding naming it -- reports over the `tcspecial` section's
//!   `payload_telemetry`: from `source_base` plus its id to `dest_base` plus
//!   its id. That is the only way such a payload's readings can reach the
//!   ground. A payload the drawing does touch goes where the drawing says and
//!   nowhere else.
//!
//! Two links landing on one port of one host would be two sources whose data
//! arrived interleaved with nothing to tell them apart, so that is refused.
//!
//! This is the one place that says all of it. The flight software routes by
//! it, the checking judges a configuration by it, and a ground station or
//! simulator working from the same file finds the same ports by it.

use std::collections::{BTreeMap, BTreeSet};

use super::parameters::{ci_config_of, flatten};
use super::{Destination, Graph};
use crate::types::{host_port_of, NetworkConfig, NetworkProtocol};

/// Which kind of link to the ground this is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Via {
    /// Drawn into this `moc` node, and sent to its `ground_base`.
    Moc(String),
    /// A payload nothing touches, over the section's `payload_telemetry`.
    PayloadTelemetry,
}

/// One source's link to the ground.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Telemetered {
    /// What the link is called where the running software names it.
    pub name: String,
    /// The node whose data it carries.
    pub source: String,
    /// That node's device id.
    pub id: usize,
    /// Which kind of link it is.
    pub via: Via,
    /// tcspecial's end, which it binds and sends from. Port nought for a link
    /// into a `moc` node, whose vehicle end is whatever the system gives.
    pub vehicle: NetworkConfig,
    /// The ground's end, which the ground binds and tcspecial sends to.
    pub ground: NetworkConfig,
}

/// What is wrong with a configuration's links to the ground.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// The node it is about, or none for the `tcspecial` section.
    pub node: Option<String>,
    pub why: String,
}

/// Every node's device id, by name.
pub fn ids(graph: &Graph) -> BTreeMap<String, usize> {
    graph
        .nodes
        .iter()
        .enumerate()
        .map(|(id, node)| (node.name.clone(), id))
        .collect()
}

/// What a source's link into a `moc` node is called.
pub fn moc_link_name(source: &str, moc: &str) -> String {
    format!("{source} to {moc}")
}

/// What an untouched payload's link over `payload_telemetry` is called.
pub fn payload_link_name(payload: &str) -> String {
    format!("{payload} telemetry")
}

/// Every link to the ground this configuration has.
///
/// A `moc` node with no `ground_base`, or one that does not read, contributes
/// nothing here: [`crate::graph::reach::reach_of`] says what is wrong with it,
/// and says it once.
pub fn of(graph: &Graph) -> Result<Vec<Telemetered>, Refused> {
    let ids = ids(graph);
    let mut links = Vec::new();

    // Links drawn into a moc node, in the order they are drawn.
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    for link in &graph.links {
        let source = link.origin().node;
        for destination in link.to.each() {
            let to = Destination::parse(destination).node;
            let Some(moc) = graph
                .node(&to)
                .filter(|n| crate::gateway::is_gateway(&n.node_type))
            else {
                continue;
            };
            // Each kind names its far end by what is there: a radio's
            // ground_base, an F´ gateway's bridge_base. One rule counts on
            // from either.
            let attribute =
                crate::gateway::GatewayKind::of_node_type(&moc.node_type)
                    .expect("a gateway node")
                    .base_attribute();
            let Some(base) = moc
                .attributes
                .get(attribute)
                .and_then(|b| host_port_of(attribute, b.as_str()).ok())
            else {
                continue;
            };
            if !seen.insert((source.clone(), to.clone())) {
                continue;
            }
            let Some(&id) = ids.get(&source) else {
                continue;
            };
            let ground = base.at(id).map_err(|why| Refused {
                node: Some(to.clone()),
                why: format!("\"{to}\": {attribute} {why}"),
            })?;
            links.push(Telemetered {
                name: moc_link_name(&source, &to),
                source: source.clone(),
                id,
                via: Via::Moc(to.clone()),
                vehicle: NetworkConfig {
                    protocol: NetworkProtocol::Udp,
                    address: "0.0.0.0".to_string(),
                    port: 0,
                },
                ground,
            });
        }
    }

    // Payloads nothing touches, over the section's payload_telemetry.
    if let Some(section) = graph.tcspecial.as_ref() {
        let refused = |why: String| Refused { node: None, why };
        let ci = ci_config_of(&flatten(section))
            .map_err(refused)?
            .to_ci_config()
            .map_err(refused)?;
        let drawn = touched(graph);
        for node in graph.nodes.iter().filter(|n| n.is_payload()) {
            if drawn.contains(node.name.as_str()) {
                continue;
            }
            let id = ids[&node.name];
            links.push(Telemetered {
                name: payload_link_name(&node.name),
                source: node.name.clone(),
                id,
                via: Via::PayloadTelemetry,
                vehicle: ci.payload_source_base.at(id).map_err(refused)?,
                ground: ci.payload_dest_base.at(id).map_err(refused)?,
            });
        }
    }

    // No two sources on one port of one host.
    let mut landed: BTreeMap<(String, u16), &Telemetered> = BTreeMap::new();
    for link in &links {
        let at = (link.ground.address.clone(), link.ground.port);
        if let Some(first) = landed.insert(at, link) {
            return Err(Refused {
                node: Some(link.source.clone()),
                why: format!(
                    "\"{}\" and \"{}\" would both send to the ground at {}:{}, \
                     and their data would arrive interleaved with nothing to \
                     tell it apart: move a base so the ports do not overlap",
                    first.source,
                    link.source,
                    link.ground.address,
                    link.ground.port,
                ),
            });
        }
    }

    Ok(links)
}

/// Every node the drawing touches: an end of a link, or of a command binding.
fn touched(graph: &Graph) -> BTreeSet<String> {
    let mut touched = BTreeSet::new();
    for link in &graph.links {
        touched.insert(link.origin().node);
        for destination in link.to.each() {
            touched.insert(Destination::parse(destination).node);
        }
    }
    for binding in &graph.commands {
        touched.insert(binding.from.clone());
        touched.insert(binding.target().node);
    }
    touched
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn example(name: &str) -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        Graph::load(root.join(format!("examples/{name}/{name}.yaml")))
            .expect("the example reads")
    }

    fn summary(links: &[Telemetered]) -> Vec<(&str, usize, &str, u16)> {
        links
            .iter()
            .map(|l| {
                (
                    l.source.as_str(),
                    l.id,
                    l.ground.address.as_str(),
                    l.ground.port,
                )
            })
            .collect()
    }

    /// Every node is counted, whatever its kind, in the order it is read.
    #[test]
    fn every_node_has_a_device_id() {
        let ids = ids(&example("thermostat"));
        assert_eq!(ids["moc"], 0);
        assert_eq!(ids["k_1"], 1);
        assert_eq!(ids["cabin_temp"], 4);
        assert_eq!(ids["temp_1"], 7);
        assert_eq!(ids["heaters_1"], 10);
        assert_eq!(ids["heaters_2"], 11);
    }

    /// What is drawn into a moc node goes to its ground_base plus the source's
    /// id, one port per source.
    #[test]
    fn a_link_into_a_moc_node_goes_to_its_ground_base_plus_the_id() {
        let links = of(&example("thermostat")).expect("it reads");
        assert_eq!(
            summary(&links),
            vec![
                ("k_1", 1, "127.0.0.1", 5101),
                ("k_2", 2, "127.0.0.1", 5102),
                ("k_3", 3, "127.0.0.1", 5103),
                ("cabin_temp", 4, "127.0.0.1", 5104),
                // The decision itself, which the ground sees as well as the
                // heaters: a reader of the telemetry can then say why the
                // heaters are on without inferring it from the temperature.
                ("thermostat", 5, "127.0.0.1", 5105),
            ]
        );
        assert!(links.iter().all(|l| l.via == Via::Moc("moc".to_string())));

        let links = of(&example("simple-app")).expect("it reads");
        assert_eq!(summary(&links), vec![("lorem", 1, "127.0.0.1", 5101)]);
    }

    /// A payload nothing touches reports over the section's payload_telemetry.
    #[test]
    fn an_untouched_payload_reports_over_payload_telemetry() {
        let links = of(&example("minimal")).expect("it reads");
        assert_eq!(summary(&links), vec![("sensor", 0, "127.0.0.1", 6100)]);
        assert_eq!(links[0].via, Via::PayloadTelemetry);
        assert_eq!(
            (links[0].vehicle.address.as_str(), links[0].vehicle.port),
            ("0.0.0.0", 6000)
        );
    }

    /// A payload the drawing touches goes only where it is drawn.
    #[test]
    fn a_drawn_payload_has_no_link_of_its_own() {
        let links = of(&example("thermostat")).expect("it reads");
        assert!(links.iter().all(|l| l.via != Via::PayloadTelemetry));
    }

    /// Two sources on one ground port are refused, naming both.
    #[test]
    fn two_sources_on_one_port_are_refused() {
        let mut graph = example("thermostat");
        // temp_2, id 8, unlinked: 6100 + 8 = 6108; a moc base of 6104 puts
        // k_1's 6105 ... cabin_temp's 6108 on the same port.
        graph.links.retain(|link| link.origin().node != "temp_2");
        let moc = graph
            .nodes
            .iter_mut()
            .find(|n| n.name == "moc")
            .expect("moc");
        moc.attributes.insert(
            "ground_base".to_string(),
            crate::Scalar::of("127.0.0.1:6104"),
        );
        let refused = of(&graph).expect_err("two sources on 6108");
        assert!(refused.why.contains("cabin_temp"), "{}", refused.why);
        assert!(refused.why.contains("temp_2"), "{}", refused.why);
        assert!(refused.why.contains("6108"), "{}", refused.why);
    }

    /// A base with no room for the ids counted from it is refused.
    #[test]
    fn a_base_with_no_room_is_refused() {
        let mut graph = example("thermostat");
        let moc = graph
            .nodes
            .iter_mut()
            .find(|n| n.name == "moc")
            .expect("moc");
        moc.attributes.insert(
            "ground_base".to_string(),
            crate::Scalar::of("127.0.0.1:65535"),
        );
        let refused = of(&graph).expect_err("port 65536 is not a port");
        assert_eq!(refused.node.as_deref(), Some("moc"));
        assert!(refused.why.contains("65535"), "{}", refused.why);
    }

    /// No section: the moc links stand, and no payload reports on its own.
    #[test]
    fn with_no_section_only_the_moc_links_remain() {
        let mut graph = example("minimal");
        graph.tcspecial = None;
        assert_eq!(of(&graph), Ok(Vec::new()));
    }
}
