//! The payloads of a configuration that a simulator simulates.
//!
//! What the simulator takes of a payload is a [`DHConfig`]: where the payload
//! is, how big a packet is, and whether it sends of its own accord or answers a
//! trigger. This makes one from a payload node, so that every simulated device
//! the simulator has -- a socket, a pty, an I2C stub, a CAN bus -- needs to
//! know nothing of the graph.
//!
//! Where the payload is comes from [`reach_of`], which is what tcspecial opens
//! a node with: so the simulator is where the vehicle reaches. A node with a
//! trigger answers one, as it does aboard.
//!
//! A configuration may leave payloads unsimulated on purpose -- a payload that
//! is a program is the program itself, and a payload on real hardware needs no
//! simulating -- so only the payloads the simulator file names are simulated,
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

/// Finds how a reading of this payload is encoded, from the conversion it
/// feeds.
///
/// A simulated sensor joined to a terminal of the physics model has to send
/// bytes the vehicle will read back as what the terminal says. How the part
/// lays a number out is the sensor's own -- its `format`, or failing that its
/// `world-out-type`'s own layout -- and the scale and the offset are on the
/// conversion -- `to_kelvin` or `to_value` -- its readings go to. So the
/// layout is taken from the one and the arithmetic from the other.
///
/// Taken rather than stated again in the simulator file. The two files are
/// joined by name and nothing else, and a scale written in both is a scale
/// that can disagree with itself -- which would show up as a simulated
/// temperature the vehicle reads back as a different one, with both files
/// looking right.
///
/// Refused where there is not exactly one conversion to take it from: a
/// sensor feeding none has no encoding to borrow, and one feeding two has no
/// single answer. And refused where the conversion is in another unit than
/// the terminal: a zone's temperature read back as volts is a number the
/// vehicle would act on, and wrong.
///
/// # Arguments
///
/// * `graph`   - The configuration.
/// * `payload` - The name of the payload node whose readings are encoded.
/// * `unit`    - The unit of the terminal it reports.
///
/// # Returns
///
/// The parameters of the one conversion its readings go to.
///
/// # Errors
///
/// A message saying so when its readings go to no conversion, or to more
/// than one, or to one in another unit.
pub fn reading_of(
    graph: &Graph,
    payload: &str,
    unit: crate::unit::Unit,
) -> Result<crate::node::vehicle::compute::ToValueParams, String> {
    use crate::node::vehicle::compute::{params_of, Computation};

    let mut found: Vec<(String, crate::node::vehicle::compute::ToValueParams)> =
        Vec::new();
    for link in graph.links.iter().filter(|l| l.origin().node == payload) {
        for destination in link.to.each() {
            let to = Destination::parse(destination).node;
            let Some(node) = graph.node(&to) else {
                continue;
            };
            if node.node_type != "compute" {
                continue;
            }
            if let Ok(
                Computation::ToKelvin(params) | Computation::ToValue(params),
            ) = params_of(&node.name, &node.attributes)
            {
                if !found.iter().any(|(name, _)| name == &to) {
                    found.push((to, params));
                }
            }
        }
    }

    match found.as_slice() {
        [(name, params)] if params.unit != unit => Err(format!(
            "\"{payload}\" reports {}, and its readings go to {name}, which \
             reads them as {}: the conversion is in the unit of what the \
             sensor is joined to",
            unit.name(),
            params.unit.name()
        )),
        [(_, params)] => Ok(crate::node::vehicle::compute::ToValueParams {
            format: laid_out(graph, payload).unwrap_or(params.format),
            ..*params
        }),
        [] => Err(format!(
            "\"{payload}\" reports {}, and nothing converts its readings: a \
             simulated sensor sends the bytes a to_kelvin or to_value node \
             would read back as what it reports, so there has to be one to \
             take the format, the scale and the offset from",
            unit.name()
        )),
        _ => {
            let named: Vec<&str> =
                found.iter().map(|(name, _)| name.as_str()).collect();
            Err(format!(
                "\"{payload}\" reports {} and its readings go to {} \
                 conversions ({}): there is no one encoding to send them in",
                unit.name(),
                found.len(),
                named.join(", ")
            ))
        }
    }
}

/// Says how a sensor lays out its readings, where it says.
///
/// # Arguments
///
/// * `graph`   - The configuration.
/// * `payload` - The sensor's name.
///
/// # Returns
///
/// Its `format`, or its `world-out-type`'s own layout, or `None` where it
/// states neither, or a type no conversion reads.
fn laid_out(
    graph: &Graph,
    payload: &str,
) -> Option<crate::node::vehicle::compute::Format> {
    use crate::graph::link_types::{carries, part_format, Carries};
    use crate::link_type::LinkType;

    let node = graph.node(payload)?;
    if let Ok(Some(format)) = part_format(node) {
        return Some(format);
    }
    match carries(node, "world-out") {
        Ok(Carries::Only(LinkType::Basic(basic))) => {
            crate::node::vehicle::compute::Format::standard(basic)
        }
        _ => None,
    }
}

/// Checks that what reaches a simulated payload driving a value input is in
/// that input's unit, which is how the simulated payload reads it.
///
/// A value on a link is a count of its unit's steps, so the simulated payload
/// at an engine's throttle reads what the vehicle sends it as a ratio, and the
/// node that sends it has to send ratios: a guidance program answering newtons
/// would have the engine read thousands of them as full throttle.
///
/// # Arguments
///
/// * `graph`   - The configuration.
/// * `payload` - The name of the simulated payload node.
/// * `unit`    - The unit of the input it drives.
///
/// # Returns
///
/// Nothing, where what reaches it is in that unit.
///
/// # Errors
///
/// A message naming the payload and both units, or saying that what reaches
/// it is not values, or that nothing does.
pub fn driving_of(
    graph: &Graph,
    payload: &str,
    unit: crate::unit::Unit,
) -> Result<(), String> {
    match crate::graph::units::input_unit(graph, payload) {
        Some(sent) if sent == unit => Ok(()),
        Some(sent) => Err(format!(
            "\"{payload}\" drives an input in {}, and what reaches it is in \
             {}: the node driving it sends values in the unit of what it \
             drives",
            unit.name(),
            sent.name()
        )),
        None => Err(format!(
            "\"{payload}\" drives an input in {}, and what reaches it is not \
             values in one unit, or nothing does: link a node that sends \
             {} to it, such as a process stating unit: {}",
            unit.name(),
            unit.name(),
            unit.symbol()
        )),
    }
}

/// Finds the setpoint and band of a thermostat node, for a simulator that
/// simulates it.
///
/// # Arguments
///
/// * `graph` - The configuration.
/// * `name`  - The node's name.
///
/// # Returns
///
/// Its setpoint and half band.
///
/// # Errors
///
/// A message saying so if there is no such node, or it is not a compute node
/// whose function is `thermostat`, or its attributes are wrong.
pub fn thermostat_of(
    graph: &Graph,
    name: &str,
) -> Result<crate::node::vehicle::compute::ThermostatParams, String> {
    use crate::node::vehicle::compute::{params_of, Computation};

    let node = graph.node(name).ok_or_else(|| {
        format!("thermostat: there is no node \"{name}\" in the configuration")
    })?;
    if node.node_type != "compute" {
        return Err(format!(
            "thermostat: \"{name}\" is a {} node, and a thermostat is a \
             compute node whose function is thermostat",
            node.node_type
        ));
    }
    match params_of(name, &node.attributes)? {
        Computation::Thermostat(params) => Ok(params),
        other => Err(format!(
            "thermostat: \"{name}\" computes {}, and a thermostat computes \
             thermostat",
            other.function()
        )),
    }
}

/// Says whether data leaving one node reaches another, over any number of
/// links and the nodes between them.
///
/// # Arguments
///
/// * `graph` - The configuration.
/// * `from`  - The node data leaves.
/// * `to`    - The node it is to reach.
///
/// # Returns
///
/// Whether some chain of links leads from `from` to `to`; a node does not
/// reach itself unless a chain leads back to it.
pub fn reaches(graph: &Graph, from: &str, to: &str) -> bool {
    let mut seen: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    let mut waiting = vec![from.to_string()];
    while let Some(node) = waiting.pop() {
        for link in graph.links.iter().filter(|l| l.origin().node == node) {
            for destination in link.to.each() {
                let next = Destination::parse(destination).node;
                if next == to {
                    return true;
                }
                if seen.insert(next.clone()) {
                    waiting.push(next);
                }
            }
        }
    }
    false
}

/// Makes what the simulator takes for each payload node the simulator file
/// names, in the order the configuration declares them.
///
/// # Arguments
///
/// * `graph`    - The configuration.
/// * `sim_file` - The simulator file, which says which payloads are
///                simulated.
///
/// # Returns
///
/// A handler for each payload whose `simulate` is true.
///
/// # Errors
///
/// A name in the simulator file that is not a payload node, or a payload node
/// the simulator cannot simulate (an app node, or one whose endpoint cannot be
/// reached), is refused, saying which.
pub fn handlers(
    graph: &Graph,
    sim_file: &SimConfigFile,
) -> Result<Vec<DHConfig>, String> {
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
                    "\"{}\" is simulated, and the configuration has no node of \
                     that name",
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
        // and never read. Simulated, it sends nothing and watches what arrives,
        // counting it, so that it is shown and switched as every other
        // simulated payload is; one joined to a heater switches it as well. See
        // `crate::types::DHConfig::driven`.
        let leaves = graph.links.iter().any(|l| l.origin().node == node.name);
        let arrives = graph.links.iter().any(|l| {
            l.to.each()
                .iter()
                .any(|d| Destination::parse(d).node == node.name)
        });

        // A payload that is a program is the program itself: tcspecial runs it,
        // and there is no device or socket for a simulator to simulate.
        if crate::node::app::Kind::of(&node.node_type).is_some() {
            return Err(format!(
                "\"{}\" is an app node, a program tcspecial runs rather than \
                 something it reaches, so there is nothing for a simulator to \
                 simulate: leave it out of the simulator file",
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
            sends_from: reach.sends_from,
            sim_address: reach.sim_address,
            // A set of lines says by its type, which the links cannot: a
            // gpio_out and a gpio_inout pass on what drives them, so they are
            // read too, and are driven all the same.
            driven: match crate::node::payload::gpio::Kind::of(&node.node_type)
            {
                Some(kind) => kind.drives(),
                None => arrives && !leaves,
            },
        });
    }
    Ok(handlers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Loads one of the examples and its simulator file.
    ///
    /// # Arguments
    ///
    /// * `name` - The example's directory under `examples/`, which is also
    ///            its files' stem.
    ///
    /// # Returns
    ///
    /// The configuration and the simulator file.
    ///
    /// # Panics
    ///
    /// If either does not read.
    fn example(name: &str) -> (Graph, SimConfigFile) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let graph =
            Graph::load(root.join(format!("examples/{name}/{name}.ytcs")))
                .expect("the example reads");
        let sim = SimConfigFile::load(
            root.join(format!("examples/{name}/{name}-sim.yaml")),
        )
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

    /// The thermostat's sensors are simulated with the sizes their nodes
    /// say: the manual's fixture, whose sensors are on buses with a read
    /// size, where the example's are sockets.
    #[test]
    fn the_thermostat_sensors_are_simulated_at_their_read_sizes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let graph = Graph::load(root.join("tests/manual/thermostat.ytcs"))
            .expect("the fixture reads");
        let sim =
            SimConfigFile::load(root.join("tests/manual/thermostat-sim.yaml"))
                .expect("its simulator file reads");
        let handlers = handlers(&graph, &sim).expect("it simulates");
        let sized: Vec<(&str, usize)> = handlers
            .iter()
            .map(|h| (h.name.0.as_str(), h.packet_size))
            .collect();
        assert!(sized.contains(&("temp_1", 2)), "{sized:?}");
        assert!(sized.contains(&("temp_3", 3)), "{sized:?}");
    }

    /// The thermostat example's sensors reach its thermostat node and the
    /// node reaches its heaters, and not the other way round.
    #[test]
    fn data_reaches_along_the_links() {
        let (graph, _) = example("thermostat");
        assert!(reaches(&graph, "temp_1", "thermostat"));
        assert!(reaches(&graph, "temp_3", "thermostat"));
        assert!(reaches(&graph, "thermostat", "gpio-out-tmass2"));
        assert!(!reaches(&graph, "thermostat", "temp_1"));
        assert!(!reaches(&graph, "gpio-out-tmass1", "thermostat"));
    }

    /// A sensor's encoding is its conversion's, in the unit of what it
    /// reports, and one reporting another unit than its conversion reads is
    /// refused naming both.
    #[test]
    fn a_reading_is_encoded_as_its_conversion_in_its_unit() {
        let (graph, _) = example("thermostat");
        let reading = reading_of(&graph, "temp_1", crate::unit::Unit::Kelvin)
            .expect("temp_1 feeds temp1");
        assert_eq!(reading.unit, crate::unit::Unit::Kelvin);
        let why = reading_of(&graph, "temp_1", crate::unit::Unit::Volt)
            .expect_err("temp1 reads kelvin");
        assert!(why.contains("volts") && why.contains("kelvin"), "{why}");
        let why = reading_of(&graph, "heaters_1", crate::unit::Unit::Kelvin)
            .expect_err("a heater feeds nothing");
        assert!(why.contains("nothing converts"), "{why}");
    }

    /// A thermostat node gives its setpoint and band; any other is refused.
    #[test]
    fn a_thermostat_node_gives_its_setpoint_and_band() {
        let (graph, _) = example("thermostat");
        let params = thermostat_of(&graph, "thermostat").unwrap();
        assert_eq!((params.setpoint, params.half_band), (293.15, 2.0));
        for name in ["temp1", "temp_1", "nowhere"] {
            assert!(thermostat_of(&graph, name).is_err(), "{name}");
        }
    }
}
