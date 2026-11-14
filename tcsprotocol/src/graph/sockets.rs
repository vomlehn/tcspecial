//! Every address and port a configuration states or implies, who binds each,
//! and the rule that no two of them are one socket.
//!
//! A configuration places sockets in several places at once: the `tcspecial`
//! section's links, each gateway node's own address and its far end counted
//! on by every source's device id, the `payload_telemetry` bases counted on
//! by every payload nothing touches, each network payload's address, and
//! each `sim_address` tcs-sim answers at. Some of those are held apart by
//! rules of their own already -- two nodes at one address
//! (`Problem::TwoNodesWantOneThing`), two sources landing on one port of the
//! ground ([`crate::graph::telemetry::of`]), `tcs_cmd.recv` and
//! `payload_command.dest_base` written alike -- and this is the rest: any
//! two of them, of different kinds, that are one socket.
//!
//! # One socket
//!
//! Two places are one socket when they are one port of one protocol on one
//! host, `localhost`, `127.0.0.1` and `::1` being one host as they are
//! everywhere else in these rules ([`crate::types::one_host`]) -- or when one
//! of them binds every interface and the other is that port on every
//! interface too, or on this host. A wildcard and a host named otherwise
//! are not compared: whether `10.0.0.5` is this machine is a question for the
//! network, and refusing it would refuse a vehicle whose payload is a box at
//! that address.
//!
//! A sender written with port nought -- the ground sending from whatever
//! port the system gives it -- claims no port, and is listed and not
//! compared.
//!
//! This is the one place the list is made: the check holds a configuration
//! to it, and `tcs-verify --summary` prints it.

use super::parameters::{ci_config_of, flatten};
use super::telemetry::{self, Reaches, Via};
use super::Graph;
use crate::types::{
    binds_every_interface, one_host, EndpointConfig, HostPort, Links,
    NetworkProtocol,
};

/// Who binds a socket, which is who would fail to start if another had it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoundBy {
    /// tcspecial, aboard.
    Vehicle,
    /// The ground station: tcs-moc, or whatever station reads the
    /// configuration.
    Ground,
    /// A payload, at its own address.
    Payload,
    /// tcs-sim, answering for a payload at its `sim_address` on a desk.
    Simulator,
    /// The far end of a software bus link: the cFS computer's SBN.
    FarEnd,
}

impl BoundBy {
    /// What a listing calls it.
    ///
    /// # Returns
    ///
    /// One word.
    pub fn word(self) -> &'static str {
        match self {
            BoundBy::Vehicle => "vehicle",
            BoundBy::Ground => "ground",
            BoundBy::Payload => "payload",
            BoundBy::Simulator => "tcs-sim",
            BoundBy::FarEnd => "far end",
        }
    }
}

/// Which rule a socket comes from, which says which pairs another check
/// holds apart already.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A link of the `tcspecial` section, or one of its SBN peer's ends.
    Section,
    /// The ground's end of a source's link: a gateway node's far end, or
    /// `payload_telemetry.dest_base`, counted on by the source's device id.
    /// Two of these on one port are refused by
    /// [`crate::graph::telemetry::of`].
    GroundLink,
    /// tcspecial's end of an untouched payload's link,
    /// `payload_telemetry.source_base` counted on by its device id.
    VehicleLink,
    /// A node's own address, a gateway's or a network payload's. Two of
    /// these at one place are refused as `Problem::TwoNodesWantOneThing`.
    Node,
    /// Where tcs-sim answers for a node on a desk.
    SimAddress,
}

/// One socket a configuration states or implies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socket {
    /// The host, as the configuration wrote it.
    pub address: String,
    /// The port; nought for a sender that sends from any port.
    pub port: u16,
    /// Its protocol: UDP for every link, and a network payload's own.
    pub protocol: NetworkProtocol,
    /// Who binds it.
    pub bound_by: BoundBy,
    /// Which rule it comes from.
    pub kind: Kind,
    /// What it is, in the configuration's words: the attribute that states
    /// it and, where it is counted, what it is counted on by.
    pub what: String,
    /// The node it belongs to, or `None` for the `tcspecial` section's own.
    pub node: Option<String>,
}

impl Socket {
    /// Whether this socket and another are one socket: see the module's
    /// words.
    ///
    /// # Arguments
    ///
    /// * `other` - The other socket.
    ///
    /// # Returns
    ///
    /// Whether one of them binding would keep the other from binding.
    pub fn is(&self, other: &Socket) -> bool {
        if self.port == 0 || self.port != other.port {
            return false;
        }
        if self.protocol != other.protocol {
            return false;
        }
        let (one, two) = (one_host(&self.address), one_host(&other.address));
        let (every_one, every_two) = (
            binds_every_interface(&self.address),
            binds_every_interface(&other.address),
        );
        one == two
            || (every_one && (every_two || two == "localhost"))
            || (every_two && one == "localhost")
    }
}

/// A UDP socket of the `tcspecial` section.
///
/// # Arguments
///
/// * `at`       - Where it is.
/// * `bound_by` - Who binds it.
/// * `what`     - What it is.
///
/// # Returns
///
/// The socket.
fn section(at: &HostPort, bound_by: BoundBy, what: &str) -> Socket {
    Socket {
        address: at.address.clone(),
        port: at.port,
        protocol: NetworkProtocol::Udp,
        bound_by,
        kind: Kind::Section,
        what: what.to_string(),
        node: None,
    }
}

/// Lists every socket a configuration states or implies, in the order a
/// reader meets them: the `tcspecial` section's, then each source's link to
/// the ground, by gateway node and device id, then each node's own address
/// and each `sim_address`, by node name.
///
/// What does not read is left out: what is wrong with it is said where it is
/// checked, and a list made of half-read terms would say something else.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// The sockets.
pub fn of(graph: &Graph) -> Vec<Socket> {
    let mut found = Vec::new();

    let ci = graph.tcspecial.as_ref().and_then(|section| {
        ci_config_of(&flatten(section)).ok()?.to_ci_config().ok()
    });
    match ci.as_ref().map(|ci| &ci.links) {
        Some(Links::Sockets(s)) => {
            let command = HostPort {
                address: s.address.clone(),
                port: s.port,
            };
            found.extend([
                section(
                    &command,
                    BoundBy::Vehicle,
                    "tcs_cmd.recv, where spacecraft commands are taken",
                ),
                section(
                    &s.moc_cmd_send,
                    BoundBy::Ground,
                    "tcs_cmd.send, where spacecraft commands come from",
                ),
                section(
                    &s.payload_command_dest_base,
                    BoundBy::Vehicle,
                    "payload_command.dest_base, where node commands are \
                     taken",
                ),
                section(
                    &s.payload_command_source_base,
                    BoundBy::Ground,
                    "payload_command.source_base, where node commands come \
                     from",
                ),
                section(
                    &s.beacon_send,
                    BoundBy::Vehicle,
                    "beacon.send, where beacons come from",
                ),
                section(
                    &s.beacon_recv,
                    BoundBy::Ground,
                    "beacon.recv, where beacons go",
                ),
            ]);
        }
        Some(Links::Bus(bus)) => {
            found.extend([
                section(
                    &bus.sbn.local,
                    BoundBy::Vehicle,
                    "sbn.local, tcspecial's end of the SBN peer",
                ),
                section(
                    &bus.sbn.peer,
                    BoundBy::FarEnd,
                    "sbn.peer, the cFS computer's end of the SBN peer",
                ),
            ]);
        }
        None => {}
    }

    // Each source's link to the ground, gathered by where it goes and in
    // device id order there, so that a gateway node's ports read upwards.
    let mut links = telemetry::of(graph).unwrap_or_default();
    links.sort_by(|a, b| {
        let via = |link: &telemetry::Telemetered| match &link.via {
            Via::Gateway(name) => (0, name.clone()),
            Via::PayloadTelemetry => (1, String::new()),
        };
        via(a).cmp(&via(b)).then(a.id.cmp(&b.id))
    });
    for link in &links {
        let Reaches::Socket { vehicle, ground } = &link.reaches else {
            continue;
        };
        let attribute = match &link.via {
            Via::Gateway(gateway) => {
                let base = graph
                    .node(gateway)
                    .and_then(|n| {
                        crate::gateway::GatewayKind::of_node_type(&n.node_type)
                    })
                    .map_or("ground_base", |kind| kind.base_attribute());
                format!("\"{gateway}\" {base}")
            }
            Via::PayloadTelemetry => "payload_telemetry.dest_base".to_string(),
        };
        found.push(Socket {
            address: ground.address.clone(),
            port: ground.port,
            protocol: NetworkProtocol::Udp,
            bound_by: BoundBy::Ground,
            kind: Kind::GroundLink,
            what: format!(
                "{attribute} plus device id {}: what \"{}\" sends",
                link.id, link.source
            ),
            node: Some(link.source.clone()),
        });
        // A link into a gateway node is sent from whatever port the system
        // gives, and claims none.
        if vehicle.port != 0 {
            found.push(Socket {
                address: vehicle.address.clone(),
                port: vehicle.port,
                protocol: NetworkProtocol::Udp,
                bound_by: BoundBy::Vehicle,
                kind: Kind::VehicleLink,
                what: format!(
                    "payload_telemetry.source_base plus device id {}: where \
                     \"{}\"'s telemetry comes from",
                    link.id, link.source
                ),
                node: Some(link.source.clone()),
            });
        }
    }

    // Each node's own address, where it has one at a port, and then where
    // tcs-sim answers for one on a desk: by name, an order no file can
    // change.
    let mut nodes: Vec<&super::Node> = graph.nodes.iter().collect();
    nodes.sort_by(|a, b| a.name.cmp(&b.name));
    for node in &nodes {
        let gateway = crate::gateway::is_gateway(&node.node_type);
        if !(gateway || node.is_payload()) {
            continue;
        }
        let Ok(reach) = super::reach::reach_of(node) else {
            continue;
        };
        let EndpointConfig::Network(net) = &reach.endpoint else {
            continue;
        };
        // Port nought is a gateway node that binds nothing aboard: one on a
        // software bus, or one that only sends.
        if net.port == 0
            || !matches!(
                net.protocol,
                NetworkProtocol::Tcp | NetworkProtocol::Udp
            )
        {
            continue;
        }
        let (bound_by, what) = if gateway {
            (
                BoundBy::Vehicle,
                "address and port, where the ground sends to it",
            )
        } else {
            (BoundBy::Payload, "address and port")
        };
        found.push(Socket {
            address: net.address.clone(),
            port: net.port,
            protocol: net.protocol,
            bound_by,
            kind: Kind::Node,
            what: format!("\"{}\" {what}", node.name),
            node: Some(node.name.clone()),
        });
    }
    for node in &nodes {
        let Ok(reach) = super::reach::reach_of(node) else {
            continue;
        };
        if let Some(at) = reach.sim_address {
            found.push(Socket {
                address: at.ip().to_string(),
                port: at.port(),
                protocol: NetworkProtocol::Udp,
                bound_by: BoundBy::Simulator,
                kind: Kind::SimAddress,
                what: format!("\"{}\" sim_address", node.name),
                node: Some(node.name.clone()),
            });
        }
    }

    found
}

/// Says which pairs of kinds another rule holds apart already, so that one
/// mistake is one complaint.
///
/// # Arguments
///
/// * `one` - One socket's kind.
/// * `two` - The other's.
///
/// # Returns
///
/// Whether a clash between the two is another rule's to say.
fn said_elsewhere(one: Kind, two: Kind) -> bool {
    matches!(
        (one, two),
        (Kind::Node, Kind::Node) | (Kind::GroundLink, Kind::GroundLink)
    )
}

/// Two sockets that are one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clash {
    /// The node to change, where either is a node's: the second's, as the
    /// claim that came second, or the first's where the second is the
    /// section's. `None` where both are the section's.
    pub node: Option<String>,
    /// What is wrong, naming both and what to do.
    pub why: String,
}

/// Finds every two sockets of a configuration that are one, other than the
/// pairs another rule refuses: two nodes at one place, and two sources on
/// one port of the ground.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// Each clash, the later of the two in [`of`]'s order being the one
/// complained about.
pub fn clashes(graph: &Graph) -> Vec<Clash> {
    let sockets = of(graph);
    let mut found = Vec::new();
    for (at, second) in sockets.iter().enumerate() {
        let Some(first) = sockets[..at].iter().find(|first| {
            !said_elsewhere(first.kind, second.kind) && first.is(second)
        }) else {
            continue;
        };
        found.push(Clash {
            node: second.node.clone().or_else(|| first.node.clone()),
            why: format!(
                "{} at {}:{} and {} at {}:{} are one socket, bound by the {} \
                 and the {}: whichever binds second fails as it starts, so \
                 give one of them another port",
                first.what,
                first.address,
                first.port,
                second.what,
                second.address,
                second.port,
                first.bound_by.word(),
                second.bound_by.word()
            ),
        });
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// A shipped example, loaded.
    ///
    /// # Arguments
    ///
    /// * `name` - Its path under `examples/`.
    ///
    /// # Returns
    ///
    /// The configuration.
    fn example(name: &str) -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
        Graph::load(root.join(name)).expect("the example loads")
    }

    /// A socket of the section, for comparing.
    ///
    /// # Arguments
    ///
    /// * `address` - Its host.
    /// * `port`    - Its port.
    ///
    /// # Returns
    ///
    /// The socket.
    fn at(address: &str, port: u16) -> Socket {
        section(
            &HostPort {
                address: address.to_string(),
                port,
            },
            BoundBy::Vehicle,
            "here",
        )
    }

    /// One host by any of its names, a wildcard against itself and this
    /// host, and not against a host named otherwise; port nought is no
    /// port.
    #[test]
    fn one_socket_is_one_port_on_one_host() {
        assert!(at("127.0.0.1", 4000).is(&at("localhost", 4000)));
        assert!(at("0.0.0.0", 4000).is(&at("127.0.0.1", 4000)));
        assert!(at("::1", 4000).is(&at("0.0.0.0", 4000)));
        assert!(at("0.0.0.0", 4000).is(&at("0.0.0.0", 4000)));
        assert!(!at("0.0.0.0", 4000).is(&at("10.0.0.5", 4000)));
        assert!(!at("127.0.0.1", 4000).is(&at("127.0.0.1", 4001)));
        assert!(!at("127.0.0.1", 0).is(&at("127.0.0.1", 0)));
        let mut tcp = at("127.0.0.1", 4000);
        tcp.protocol = NetworkProtocol::Tcp;
        assert!(!tcp.is(&at("127.0.0.1", 4000)));
    }

    /// The thermostat's gateway node's far end, counted on by each source's
    /// device id, and every shipped example free of clashes.
    #[test]
    fn the_thermostat_lists_its_ground_ports_by_source() {
        let graph = example("thermostat/thermostat.ytcs");
        let sockets = of(&graph);
        let ids = telemetry::ids(&graph);
        let to_k1 = sockets
            .iter()
            .find(|s| {
                s.kind == Kind::GroundLink && s.node.as_deref() == Some("to_k1")
            })
            .expect("to_k1 reaches the ground");
        assert_eq!(to_k1.port, 5100 + ids["to_k1"] as u16);
        assert!(to_k1.what.contains("\"gateway\" ground_base"));
        assert!(clashes(&graph).is_empty(), "{:?}", clashes(&graph));
    }

    /// A network payload at the port the vehicle takes commands on is one
    /// socket with it, and is named as the one to move.
    #[test]
    fn a_payload_on_the_command_port_clashes() {
        let mut graph = example("cfs/cfs.ytcs");
        // The cfs example's links are on the bus: sbn.local is the vehicle's.
        let heater = graph
            .nodes
            .iter_mut()
            .find(|n| n.name == "heater")
            .expect("the cfs example has a heater");
        heater
            .attributes
            .insert("port".to_string(), crate::Scalar::of("8634"));
        let found = clashes(&graph);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].node.as_deref(), Some("heater"));
        assert!(found[0].why.contains("sbn.local"), "{}", found[0].why);
    }
}
