//! How large a datum may be: the most the gateway's link carries, and the
//! largest datum each node can produce.
//!
//! A datum crosses a link as one datagram, so the most one may be is the
//! most one datagram of the gateway's link carries: [`most_datum`] asks the
//! gateway, from what the configuration already says of it. Each node's own
//! largest datum is worked out from what the configuration says of that node
//! -- the width of the type it sends, a string's or bytes' most, what one
//! read of its device can deliver -- by [`largest_datum`], the one statement
//! of it, which every check and every buffer sized by it calls. See "How
//! large a datum may be" in `docs/tcspecial-design.rst`.

use std::collections::BTreeSet;

use super::link_types::{carries, is_device, Carries};
use super::{Graph, Node};
use crate::gateway::GatewayKind;
use crate::types::EndpointConfig;

/// The most one UDP datagram carries over IPv4: 65,535 less the 20 bytes of
/// an IPv4 header and the 8 of a UDP one.
pub const UDP_IPV4_BYTES: usize = 65_507;

/// The most one UDP datagram carries over IPv6, short of a jumbogram: 65,535
/// less the 8 bytes of a UDP header, IPv6's own header not counting against
/// its payload length.
pub const UDP_IPV6_BYTES: usize = 65_527;

/// How large one node's largest datum is, and whether the configuration
/// says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Largest {
    /// The bytes.
    pub bytes: usize,
    /// True where the configuration bounds it; false where nothing does and
    /// it is the most any datum may be, which is warned of.
    pub bounded: bool,
}

/// Says whether a host, as a configuration spells it, is an IPv6 address.
///
/// # Arguments
///
/// * `host` - The host: `10.0.0.2`, `[::1]`, `::1`, a name.
///
/// # Returns
///
/// True for an IPv6 address written out; false for an IPv4 address and for
/// a name, which may resolve to either and so is held to the smaller.
fn is_ipv6(host: &str) -> bool {
    host.trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<std::net::Ipv6Addr>()
        .is_ok()
}

/// Says the most a datum may be in a configuration: the most one datagram of
/// its gateway's link carries, less what the link adds to each.
///
/// Each kind answers from what the configuration says of it: a cFS bus,
/// `sbn.max_message` less a cFS packet's headers; a link over UDP, 65,527
/// bytes where every gateway node's far end is an IPv6 address and 65,507
/// otherwise, less a CCSDS header where the gateway frames its packets in
/// one. A string or bytes is sent the ground after its count, so the count's
/// bytes come off as well. A configuration that names no gateway is held to
/// UDP over IPv4.
///
/// For now no more than `tcsprotocol::types::DATUM_BYTES` either, as every
/// buffer aboard is that size until each is sized by its node's largest
/// datum.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// The bytes.
pub fn most_datum(graph: &Graph) -> usize {
    let gateways: Vec<(&Node, GatewayKind)> = graph
        .nodes
        .iter()
        .filter_map(|node| {
            GatewayKind::of_node_type(&node.node_type).map(|kind| (node, kind))
        })
        .collect();
    let on_bus = gateways.iter().any(|(_, kind)| kind.on_bus());
    let datagram = if on_bus {
        bus_message(graph)
            .map(|most| most.saturating_sub(crate::cfs::HEADER_BYTES))
            .unwrap_or(UDP_IPV4_BYTES)
    } else {
        let every_ipv6 = !gateways.is_empty()
            && gateways.iter().all(|(node, _)| {
                super::reach::reach_of(node).is_ok_and(|reach| {
                    reach.ground.is_some_and(|far| is_ipv6(&far.address))
                })
            });
        let udp = if every_ipv6 {
            UDP_IPV6_BYTES
        } else {
            UDP_IPV4_BYTES
        };
        let framed = gateways.iter().any(|(_, kind)| kind.framed());
        udp - if framed {
            crate::ccsds::HEADER_BYTES
        } else {
            0
        }
    };
    datagram
        .saturating_sub(crate::link_type::COUNT_BYTES)
        .min(crate::types::DATUM_BYTES)
}

/// Says the longest message the configuration's cFS bus carries.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// `sbn.max_message`, or `None` where the section does not read or names
/// no bus.
fn bus_message(graph: &Graph) -> Option<usize> {
    let section = graph.tcspecial.as_ref()?;
    let ci =
        super::parameters::ci_config_of(&super::parameters::flatten(section))
            .and_then(|json| json.to_ci_config())
            .ok()?;
    ci.bus().map(|bus| bus.sbn.max_message as usize)
}

/// Says what one read of a device can deliver, where the configuration
/// bounds it.
///
/// # Arguments
///
/// * `node` - The device's node.
///
/// # Returns
///
/// Its `read_size`; the most a line of it is, where it is a stream; what one
/// CAN frame or rpmsg message carries; or `None` where nothing says.
fn one_read(node: &Node) -> Option<usize> {
    let reach = super::reach::reach_of(node).ok()?;
    if let Some(size) = reach.read_size {
        return Some(size as usize);
    }
    if let Some(rule) = &reach.stream {
        return Some(rule.max_length as usize);
    }
    match &reach.endpoint {
        EndpointConfig::Can(can) => {
            Some(crate::node::payload::can::frame_bytes(can.fd))
        }
        EndpointConfig::Rpmsg(rpmsg) => Some(
            crate::node::payload::rpmsg::message_bytes(rpmsg.max_message_bytes)
                as usize,
        ),
        _ => None,
    }
}

/// Says the largest datum a node can produce at its `world-out`.
///
/// The width of the type it sends, where every datum of it is one width; a
/// string's or bytes' most, where it states one; a ping node's reply; a
/// tee's sources' largest; what one read of a device delivers; and
/// otherwise the most any datum may be, [`most_datum`], unbounded.
///
/// # Arguments
///
/// * `graph` - The configuration.
/// * `node`  - The node.
///
/// # Returns
///
/// Its largest datum: what the configuration says, which may be more than
/// [`most_datum`] -- a configuration refused for it -- or the most, where
/// nothing bounds it.
pub fn largest_datum(graph: &Graph, node: &Node) -> Largest {
    let mut seen = BTreeSet::new();
    largest_of(graph, node, most_datum(graph), &mut seen)
}

/// [`largest_datum`], with the tees already asked, so that a cycle of them
/// ends.
///
/// # Arguments
///
/// * `graph` - The configuration.
/// * `node`  - The node.
/// * `most`  - The most any datum may be.
/// * `seen`  - The tees asked already.
///
/// # Returns
///
/// Its largest datum.
fn largest_of(
    graph: &Graph,
    node: &Node,
    most: usize,
    seen: &mut BTreeSet<String>,
) -> Largest {
    let unbounded = Largest {
        bytes: most,
        bounded: false,
    };
    let bounded = |bytes: usize| Largest {
        bytes,
        bounded: true,
    };
    let Ok(Carries::Only(kind)) = carries(node, "world-out") else {
        return unbounded;
    };
    if let Some(width) = kind.width() {
        return bounded(width);
    }
    if let crate::link_type::LinkType::Basic(basic) = &kind {
        if let Some(most) = basic.most() {
            return bounded(most);
        }
    }
    let node_type = node.node_type.as_str();
    if node_type == "ping" {
        return bounded(crate::node::vehicle::ping::REPLY_BYTES);
    }
    if node_type == "tee-gateway" || node_type == "tee-payload" {
        if !seen.insert(node.name.clone()) {
            return unbounded;
        }
        let sources: Vec<&Node> = graph
            .links
            .iter()
            .filter(|link| {
                link.to
                    .each()
                    .iter()
                    .any(|to| super::Destination::parse(to).node == node.name)
            })
            .filter_map(|link| graph.node(&link.origin().node))
            .collect();
        if sources.is_empty() {
            return unbounded;
        }
        return sources.iter().fold(bounded(0), |sum, source| {
            let one = largest_of(graph, source, most, seen);
            Largest {
                bytes: sum.bytes.max(one.bytes),
                bounded: sum.bounded && one.bounded,
            }
        });
    }
    if is_device(node) {
        if let Some(bytes) = one_read(node) {
            return bounded(bytes);
        }
    }
    unbounded
}

/// Lists each node that sends what nothing in the configuration bounds: one
/// linked from its `world-out` that carries `bytes` there because it states
/// no type, and that
/// says nothing else that bounds it -- no `read_size`, no line's
/// `max_length`, an interface that does not.
///
/// A node that states `string` or `bytes` alone is not listed: that is said
/// of the attribute that states it (see
/// [`super::link_types::unbounded`]).
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// The name of each.
pub fn unbounded_senders(graph: &Graph) -> Vec<String> {
    graph
        .nodes
        .iter()
        .filter(|node| {
            let stated = node.attributes.contains_key("world-out-type")
                || node.attributes.contains_key("carries");
            let producer = is_device(node)
                || crate::node::app::Kind::of(&node.node_type).is_some();
            // A device that is only driven -- a heater, a valve -- sends
            // nothing, and nothing need be sized by it.
            producer
                && sends(graph, node)
                && !stated
                && !largest_datum(graph, node).bounded
        })
        .map(|node| node.name.clone())
        .collect()
}

/// Lists each node that sends more at once than a datum may be: one whose
/// largest datum, as the configuration says it, is more than
/// [`most_datum`].
///
/// Not on a cFS bus, which says so its own way, naming `sbn.max_message`.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// Each node's name, with its largest datum.
pub fn too_large(graph: &Graph) -> Vec<(String, usize)> {
    let on_bus = graph.nodes.iter().any(|node| {
        GatewayKind::of_node_type(&node.node_type).is_some_and(|k| k.on_bus())
    });
    if on_bus {
        return Vec::new();
    }
    let most = most_datum(graph);
    graph
        .nodes
        .iter()
        .filter(|node| sends(graph, node))
        .filter_map(|node| {
            let largest = largest_datum(graph, node);
            (largest.bytes > most).then(|| (node.name.clone(), largest.bytes))
        })
        .collect()
}

/// Says whether anything is linked from a node's `world-out`.
///
/// # Arguments
///
/// * `graph` - The configuration.
/// * `node`  - The node.
///
/// # Returns
///
/// True where a link leaves it there.
fn sends(graph: &Graph, node: &Node) -> bool {
    graph.links.iter().any(|link| {
        let origin = link.origin();
        origin.node == node.name
            && origin.input.as_deref().is_none_or(|t| t == "world-out")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Reads a shipped example.
    ///
    /// # Arguments
    ///
    /// * `name` - The example's directory and file name.
    ///
    /// # Returns
    ///
    /// Its graph.
    fn example(name: &str) -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        Graph::load(root.join(format!("examples/{name}/{name}.ytcs")))
            .expect("the example reads")
    }

    /// A device's largest datum is its read, a tee's its source's, an
    /// app's its type's most, and none of them is warned of.
    #[test]
    fn each_node_is_sized_by_what_the_configuration_says() {
        let tees = example("tees");
        let sized = |graph: &Graph, name: &str| {
            largest_datum(graph, graph.node(name).expect(name))
        };
        let read = Largest {
            bytes: 16,
            bounded: true,
        };
        assert_eq!(sized(&tees, "sensor"), read);
        assert_eq!(sized(&tees, "readings"), read);
        assert!(unbounded_senders(&tees).is_empty());

        let app = example("simple-app");
        assert_eq!(sized(&app, "lorem").bytes, 64);
        assert!(unbounded_senders(&app).is_empty());
    }

    /// A device that sends and says nothing of its size is warned of, held
    /// to the most any datum may be; one that is only driven is not.
    #[test]
    fn an_unsized_sender_is_listed_and_held_to_the_most() {
        let mut tees = example("tees");
        for node in &mut tees.nodes {
            node.attributes.remove("read_size");
        }
        let senders = unbounded_senders(&tees);
        assert!(senders.contains(&"sensor".to_string()), "{senders:?}");
        assert!(!senders.contains(&"heater".to_string()), "{senders:?}");
        let sensor = tees.node("sensor").expect("sensor");
        assert_eq!(
            largest_datum(&tees, sensor),
            Largest {
                bytes: most_datum(&tees),
                bounded: false,
            }
        );
    }

    /// A node that says it sends more than a datum may be is listed.
    #[test]
    fn a_node_too_large_for_a_datum_is_listed() {
        let mut tees = example("tees");
        assert!(too_large(&tees).is_empty());
        let most = most_datum(&tees);
        tees.nodes
            .iter_mut()
            .find(|node| node.name == "sensor")
            .expect("sensor")
            .attributes
            .insert(
                "read_size".into(),
                crate::Scalar::of((most + 1).to_string()),
            );
        assert!(too_large(&tees).contains(&("sensor".to_string(), most + 1)));
    }

    /// Until buffers are sized by their nodes, the most is what they hold.
    #[test]
    fn the_most_is_held_to_what_a_buffer_holds_for_now() {
        assert_eq!(most_datum(&example("tees")), crate::types::DATUM_BYTES);
    }

    /// A host is IPv6 only where it is an IPv6 address written out.
    #[test]
    fn only_an_ipv6_address_is_ipv6() {
        assert!(is_ipv6("::1"));
        assert!(is_ipv6("[fe80::1]"));
        assert!(!is_ipv6("10.0.0.2"));
        assert!(!is_ipv6("ground.example"));
    }
}
