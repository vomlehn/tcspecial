//! What a node says about the thing it reaches.
//!
//! A node of a graph is text: a type and a map of attributes as somebody wrote
//! them. What the flight software opens is an [`EndpointConfig`] -- a bus and
//! an address, a chip and three line numbers, a line and the framing of it.
//! This is the piece between, and it is deliberately thin: the rules about
//! what each type of interface may be told, and what each term means, are the
//! rules in [`crate::endpoint_config`] and the per-type modules under
//! [`crate::node`], and they are reached from here rather than written again.
//!
//! That is the whole argument for the shape of this module. Those rules are
//! the ones that have been tested against real buses; a second set of them
//! written here would be two statements of every rule, drifting apart
//! one flight at a time, and the first symptom would be a configuration that
//! verifies on the ground and opens something else aboard.
//!
//! # What this refuses
//!
//! An attribute it does not recognise, by name, for the type of node it is
//! on. That is the point of doing it here: a term belonging to another type --
//! a data rate on an I2C node -- is passed on to that type's own rules, which
//! have always refused it in their own words, but a term belonging to no type
//! at all would otherwise vanish. `max_speeed` silently ignored is a
//! peripheral clocked at whatever the last program left the bus at, and
//! nothing in the telemetry would ever say so.

use crate::endpoint_config::{
    a_link_term, endpoint_of, parse_u32, EndpointLocation, LinkTerms, Scalar, StreamParams,
    StreamTerms,
};
use crate::gateway::{is_gateway, GatewayKind};
use crate::graph::Node;
use crate::types::{host_port_of, DeviceConfig, EndpointConfig, HostPort, NetworkConfig};
use crate::{DHType, NetworkProtocol};

/// Everything a node's attributes say about reaching it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reach {
    /// What to open, and how to set it up once open.
    pub endpoint: EndpointConfig,
    /// How many bytes one read asks for, where the node says.
    ///
    /// A bus master clocks exactly as many bytes as it asks for, so what a
    /// read asks for is a property of the part: the forward sensor answers in
    /// two bytes and the one in the equipment bay in three. Absent for a node
    /// whose reads are bounded by the thing itself -- a whole CAN frame, a
    /// whole rpmsg message, a line event.
    pub read_size: Option<u32>,
    /// What to write to make this node answer, where it must be asked.
    ///
    /// A node with a trigger is a node that is asked; there is no separate
    /// word for the mode, because a trigger and nothing else is what makes a
    /// node triggered and the extra word could only disagree with it.
    pub trigger: Option<Vec<u8>>,
    /// How long to wait between asking this node and reading it.
    ///
    /// For a node with a trigger, and for no other: it is the time the part
    /// takes to answer, which its interface document states and nothing here
    /// can guess. With one, a pass asks, waits, and reads what came back; with
    /// none, a pass reads what the last ask produced and asks again, so the
    /// reading is one interval old and nothing waits.
    pub read_delay: Option<std::time::Duration>,
    /// What to write to this node when the vehicle opens it, where anything.
    ///
    /// For a payload whose state matters before the vehicle's first word to
    /// it: a heater that must start off rather than in whatever state the
    /// hardware came up in, a sensor with a register to set before it reads
    /// sensibly. Written as the node is opened -- as the vehicle starts, and
    /// again whenever the ground starts it after stopping it -- and a write
    /// that fails is a node that did not open.
    pub init: Option<Vec<u8>>,
    /// Where a ground link's data goes: the `moc` node's `ground_base`.
    ///
    /// For a `moc` node, and only for one. What each node linked into the
    /// `moc` node produces is sent to this host, at this port plus that
    /// node's device id -- see `crate::graph::telemetry` -- so the ground
    /// tells the nodes apart by the port their data arrives at, and the
    /// vehicle sends from the first reading without being spoken to.
    pub ground: Option<HostPort>,
    /// Where one read of a line ends: the most it may hold, and the timeout,
    /// terminators or length prefix that may end it sooner.
    ///
    /// For a line, and only for one: a stream delivers bytes with no frame
    /// boundaries of its own, so the vehicle holds what has arrived until one
    /// of these says a read is done. Every other interface's read is already
    /// a frame -- a datagram, a bus transfer, a CAN frame, a line event.
    pub stream: Option<StreamParams>,
    /// Where a datagram payload that speaks first sends from, where its node
    /// says: the one address its datagrams are read from. See
    /// [`crate::SendsFrom`], and [`sends_from_of`] for what a node may say.
    pub sends_from: Option<crate::SendsFrom>,
    /// Where the node is reached over UDP when it is simulated, for a type
    /// the operating system has nothing faithful to stand in for: see
    /// [`stands_in_over_udp`] and [`Reach::opened`].
    pub stand_in: Option<std::net::SocketAddr>,
}

/// The types that may be reached over UDP when simulated, with `stand_in`,
/// because the operating system has nothing faithful to stand in for them: a
/// synchronous line has no virtual interface at all, and a pseudo-terminal
/// carries a SPI peripheral's bytes without its clock, an rpmsg channel's
/// messages without its channel and a sensor's samples without its sysfs.
///
/// Every other type has a stand-in the kernel or tcssim puts at the
/// endpoint tcspecial opens, which is the better test: the code that runs is
/// the code that flies.
pub fn stands_in_over_udp(kind: DHType) -> bool {
    matches!(
        kind,
        DHType::Hdlc | DHType::Spi | DHType::Rpmsg | DHType::Iio
    )
}

impl Reach {
    /// What tcspecial opens for this node under `mode`: its endpoint, or a
    /// UDP socket to its `stand_in` where the run has stand-ins and it states
    /// one.
    ///
    /// The one statement of the rule. Under the flight setting `stand_in` is
    /// never used, so the configuration that flies is the one that was tested
    /// and the device is what is opened; under `some` and `all` a node with
    /// one is reached there, and tcssim answers. See "Saying Which Payloads Are
    /// Simulated" in docs/design.rst.
    pub fn opened(&self, mode: crate::simulate::Mode) -> EndpointConfig {
        match self.stand_in {
            Some(at) if !mode.is_flight() => EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: at.ip().to_string(),
                port: at.port(),
            }),
            _ => self.endpoint.clone(),
        }
    }
}

/// What this node reaches, or what is wrong with what it says.
///
/// The name is in every complaint: a configuration is read by somebody who
/// needs to know which of forty nodes is the one being refused.
pub fn reach_of(node: &Node) -> Result<Reach, String> {
    let what = node.name.as_str();
    let kind = kind_of(node)?;

    let mut said = Said::default();
    for (attribute, value) in &node.attributes {
        if !take(&mut said, node, kind, attribute, value)? {
            // A retired word is not one to point a reader at: see
            // `retired_term`.
            let known = |name: &str| {
                retired_term(name).is_none()
                    && take(&mut Said::default(), node, kind, name, &Scalar::of("")) != Ok(false)
            };
            return Err(match crate::graph::spelled(attribute, known) {
                Some(hint) => format!(
                    "\"{what}\": there is nothing called \"{attribute}\" on {} {} \
                     node: {hint}",
                    kind.article(),
                    node.node_type
                ),
                None => format!(
                    "\"{what}\": there is nothing called \"{attribute}\" on {} \
                     {} node. An attribute nothing reads is worse than one that \
                     is refused: it looks like something that was set",
                    kind.article(),
                    node.node_type
                ),
            });
        }
    }
    let Said {
        mut terms,
        stream,
        stream_stated,
        read_size,
        trigger,
        init,
        read_delay,
        sends_from,
        stand_in,
    } = said;

    if stream_stated {
        terms.stream = Some(stream);
    }

    // The two types that are not on a link have no terms, so a term on one of
    // them is refused here: the per-type rules that would have refused it are
    // the ones that are never consulted for these two.
    if matches!(kind, DHType::Network | DHType::Device) {
        if let Some(term) = a_link_term(&terms, &[]) {
            return Err(format!(
                "\"{what}\" is {} {} node, which is not on a link, so {term} does \
                 not apply to it",
                kind.article(),
                node.node_type
            ));
        }
        // A network node's rule is settled below, once its protocol says
        // whether it is a stream.
        if stream_stated && kind == DHType::Device {
            return Err(format!(
                "\"{what}\" is {} {} node, and where a read ends is a property of a \
                 stream -- a line, or a tcp or unix_stream socket: max_length, \
                 read_timeout, terminators and length_prefix do not apply to it",
                kind.article(),
                node.node_type
            ));
        }
    }

    // What the rules settle about where a read ends, for a stream.
    let mut framing = None;
    let endpoint = match kind {
        // A socket: a host and a port, and which protocol is spoken to it.
        // The MOC link is one of these, and a `moc` node needs no protocol --
        // a ground link is a datagram link, as the one tcspecial has always
        // had is.
        DHType::Network => {
            let protocol = match node.attributes.get("protocol") {
                // A raw socket is a protocol only with its number beside it.
                Some(stated) if stated.as_str() == crate::types::RAW => {
                    NetworkProtocol::Raw(ip_protocol_of(what, node)?)
                }
                Some(stated) => {
                    NetworkProtocol::from_spelling(stated.as_str()).ok_or_else(|| {
                        format!(
                            "\"{what}\": \"{}\" is not a protocol: there are {}",
                            stated.as_str(),
                            NetworkProtocol::spellings()
                        )
                    })?
                }
                None if is_gateway(&node.node_type) => NetworkProtocol::Udp,
                None => {
                    return Err(format!(
                        "\"{what}\" is a network node, so it needs the protocol spoken \
                         to it: one of {}",
                        NetworkProtocol::spellings()
                    ))
                }
            };

            ip_protocol_misplaced(what, node, protocol)?;

            // A stream socket delivers bytes with no packets of their own,
            // as a line does, so it says where one ends by the same rule and
            // must: without one a pass would take whatever bytes happened to
            // be waiting, half a record as often as a whole one. A datagram
            // is a packet already, and a rule on one would be a rule nothing
            // applies.
            match (protocol.is_stream(), terms.stream.take()) {
                (true, Some(stated)) => {
                    framing = Some(
                        crate::endpoint_config::validate_stream(what, stated)
                            .map_err(|why| why.to_string())?,
                    );
                }
                (true, None) => {
                    return Err(format!(
                        "\"{what}\" is reached over {}, which is a stream of bytes, so \
                         it must say where one packet of it ends: give max_length, and \
                         read_timeout, terminators or length_prefix",
                        protocol.spelling()
                    ))
                }
                (false, Some(_)) => {
                    return Err(format!(
                        "\"{what}\" is reached over {}, whose datagrams are packets \
                         already: max_length, read_timeout, terminators and \
                         length_prefix say where a packet of a stream ends, and apply \
                         only to a line or a tcp or unix_stream socket",
                        protocol.spelling()
                    ))
                }
                (false, None) => {}
            }

            // A ground link that states neither address nor port binds
            // nothing: it only sends, to its ground_base, and the ground has
            // nowhere aboard to send to. One that receives -- commands, or a
            // line out of it -- must state both, which `Graph::check` holds it
            // to, knowing the links. Both or neither, whichever: half an
            // address is a mistake on any node.
            let has = |a: &str| node.attributes.contains_key(a);
            if is_gateway(&node.node_type) && !has("address") && !has("port") {
                EndpointConfig::Network(NetworkConfig {
                    protocol,
                    address: UNBOUND.to_string(),
                    port: 0,
                })
            } else {
                EndpointConfig::Network(NetworkConfig {
                    protocol,
                    address: required(
                        what,
                        node,
                        "address",
                        "a network node",
                        "the host it is at",
                    )?
                    .as_str()
                    .to_string(),
                    port: match protocol {
                        // An IP datagram has no port: what tells one
                        // payload's datagrams from another's is the protocol
                        // number and the host.
                        NetworkProtocol::Raw(_) if has("port") => {
                            return Err(format!(
                                "\"{what}\" is reached on a raw socket, which has no \
                                 port: take port out, the ip_protocol and the \
                                 address being what say which datagrams are its"
                            ))
                        }
                        NetworkProtocol::Raw(_) => 0,
                        _ => port_of(what, node)?,
                    },
                })
            }
        }
        // A file, opened: see node::payload::device_file for why that is a type of its
        // own and not every other type as well.
        DHType::Device => EndpointConfig::Device(DeviceConfig {
            path: required(
                what,
                node,
                "path",
                "a device file node",
                "the file it opens",
            )?
            .as_str()
            .to_string(),
        }),
        // Everything else is on a link with terms of its own -- a rate, a
        // bus address, a clock mode -- and the rules for that type settle
        // them.
        kind => {
            let on_a_link = crate::types::kind_of_terms(what, kind, terms)?;
            framing = on_a_link.stream().cloned();
            endpoint_of(what, &location_of(what, node, kind)?, &on_a_link)?
        }
    };

    // A read is delivered as one datum, so one that could hold more than a
    // datum carries would arrive cut short -- and a line read in pieces is a
    // line whose pieces nothing knows how to put back together.
    if let Some(rule) = &framing {
        if rule.max_length as usize > crate::types::DATUM_BYTES {
            return Err(format!(
                "\"{what}\": max_length {} is more than one datum carries: a read \
                 is delivered whole, and the most a datum holds is {} bytes",
                rule.max_length,
                crate::types::DATUM_BYTES
            ));
        }
    }

    // What the interface itself will not do, which is the last thing to ask:
    // both of these read what the kind settled above.
    if trigger.is_some() {
        if let Some(why) = cannot_be_asked(&endpoint) {
            return Err(format!("\"{what}\" states a trigger, and {why}"));
        }
    }
    if init.is_some() {
        if let Some(why) = cannot_be_written(&endpoint) {
            return Err(format!("\"{what}\" states an init, and {why}"));
        }
    }
    if let Some(size) = read_size {
        fits_one_read(what, size, &endpoint)?;
    }

    let ground = ground_of(what, node)?;
    let sends_from = match sends_from {
        Some(written) => Some(sends_from_of(what, written.as_str(), &endpoint)?),
        None => None,
    };

    Ok(Reach {
        endpoint,
        read_size,
        trigger,
        init,
        read_delay,
        ground,
        stream: framing,
        sends_from,
        stand_in,
    })
}

/// Where a node says its payload sends from, or what is wrong with it.
///
/// A datagram socket's, and only one's: a stream is connected, so the
/// connection says who is at the other end, and a raw socket is connected to
/// its payload's host already. A host and a port over UDP, written together
/// as one address, and over a Unix datagram socket a path. Never the node's
/// own address, which is the vehicle's end of the link: the payload sending
/// from where the vehicle waits would be one socket at both ends.
///
/// Whether the payload is one that speaks first, the only kind this is for,
/// is a question of its links, and `Graph::check` asks it.
fn sends_from_of(
    what: &str,
    written: &str,
    endpoint: &EndpointConfig,
) -> Result<crate::SendsFrom, String> {
    let net = match endpoint {
        EndpointConfig::Network(net) => net,
        _ => {
            return Err(format!(
                "\"{what}\" states sends_from, which says where a datagram payload \
                 sends from: only a network node over udp or unix_dgram has one"
            ))
        }
    };
    match net.protocol {
        NetworkProtocol::Udp => {
            let at: std::net::SocketAddr = written.trim().parse().map_err(|_| {
                format!(
                    "\"{what}\": sends_from {written:?} is not a host and a port: \
                     write the payload's IP address and the port it sends from, \
                     as 10.0.0.7:7001"
                )
            })?;
            if at.port() == 0 {
                return Err(format!(
                    "\"{what}\": sends_from {written:?} names port 0, which is any \
                     port at all: write the port the payload sends from"
                ));
            }
            let own = format!("{}:{}", net.address, net.port).parse::<std::net::SocketAddr>();
            if own.is_ok_and(|own| own == at) {
                return Err(format!(
                    "\"{what}\": sends_from {written:?} is the node's own address and \
                     port, which is where the vehicle waits for it: write where the \
                     payload sends from"
                ));
            }
            Ok(crate::SendsFrom::Socket(at))
        }
        NetworkProtocol::UnixDgram => {
            let path = written.trim();
            if path.is_empty() {
                return Err(format!(
                    "\"{what}\": sends_from is empty: write the path the payload's \
                     socket is bound to"
                ));
            }
            if path == net.address {
                return Err(format!(
                    "\"{what}\": sends_from {written:?} is the node's own address, \
                     which is where the vehicle waits for it: write the path the \
                     payload's socket is bound to"
                ));
            }
            Ok(crate::SendsFrom::Path(path.to_string()))
        }
        other => Err(format!(
            "\"{what}\" is reached over {}, and sends_from says where a datagram \
             payload sends from: a stream is connected, and so is a raw socket, so \
             the other end is known already. Take sends_from out",
            other.spelling()
        )),
    }
}

/// What a node's attributes have said so far, as [`take`] sorts them.
struct Said {
    terms: LinkTerms,
    stream: StreamTerms,
    stream_stated: bool,
    read_size: Option<u32>,
    trigger: Option<Vec<u8>>,
    init: Option<Vec<u8>>,
    read_delay: Option<std::time::Duration>,
    sends_from: Option<Scalar>,
    stand_in: Option<std::net::SocketAddr>,
}

impl Default for Said {
    fn default() -> Said {
        Said {
            terms: LinkTerms::default(),
            stream: StreamTerms {
                max_length: None,
                timeout: None,
                terminators: None,
                length_prefix: None,
            },
            stream_stated: false,
            read_size: None,
            trigger: None,
            init: None,
            read_delay: None,
            sends_from: None,
            stand_in: None,
        }
    }
}

/// Put one attribute where it belongs: `Ok(false)` for a word nothing here
/// reads, which the caller refuses.
///
/// Apart from [`reach_of`] so that a refusal can ask whether the word with
/// its hyphens and underscores swapped is one this reads -- by asking this,
/// rather than a second list of the words that would drift from the first.
fn take(
    said: &mut Said,
    node: &Node,
    kind: DHType,
    attribute: &str,
    value: &Scalar,
) -> Result<bool, String> {
    let what = node.name.as_str();
    // The terms of a link go to the rules for the type, whichever type
    // they belong to: one on the wrong type is refused by that type's own
    // rules, in the words it has always used.
    if put_term(&mut said.terms, attribute, value) {
        return Ok(true);
    }

    match attribute {
        // Where it is. Which attributes locate a node depends on its
        // type, and one that locates another type is refused rather than
        // ignored: a port on a node reached through a device file is a
        // file that looks configured and is not.
        other if LOCATORS.contains(&other) => {
            if !locators(kind).contains(&other) {
                return Err(format!(
                    "\"{what}\" is {} {} node, so {other} does not locate it: \
                     it is located by {}",
                    kind.article(),
                    node.node_type,
                    locators(kind).join(" and ")
                ));
            }
        }
        // When it is read, which is the schedule's business and not
        // this one's: the group it is read with, or the rate it states
        // for itself -- `trigger_interval` where it is asked and
        // `poll_interval` where it is simply read.
        "group" | "interval" | "trigger_interval" | "poll_interval" => {}
        // How long to wait between asking and reading, which is this
        // one's: it belongs to the part rather than to the schedule.
        "read_delay" => {
            said.read_delay = Some(
                crate::graph::parse_interval(value.as_str())
                    .map_err(|why| format!("\"{what}\": read_delay: {why}"))?,
            );
        }
        // Where a gateway node's data goes, which only a gateway node has,
        // each kind saying it by the name of what is at its far end: read
        // below.
        other if GatewayKind::ALL.iter().any(|k| k.base_attribute() == other) => {
            match GatewayKind::of_node_type(&node.node_type) {
                Some(of) if of.base_attribute() == other => {}
                Some(of) => {
                    return Err(format!(
                        "\"{what}\" is a {} node, whose data goes to {}: say {} \
                         rather than {other}",
                        node.node_type,
                        of.far_end(),
                        of.base_attribute()
                    ))
                }
                None => {
                    return Err(format!(
                        "\"{what}\" is {} {} node, and {other} says where a ground \
                         link's data goes: only a gateway node has one",
                        kind.article(),
                        node.node_type
                    ))
                }
            }
        }
        // Where the F´ IDs of what goes to an F´ bridge begin, which the
        // bridge's model is made from and nothing aboard reads: read below.
        "fprime_id_base"
            if GatewayKind::of_node_type(&node.node_type) == Some(GatewayKind::Fprime) => {}
        "fprime_id_base" => {
            return Err(format!(
                "\"{what}\" is {} {} node, and fprime_id_base says where the F´ \
                 IDs of what goes to an F´ bridge begin: only a gateway_fprime \
                 node has one",
                kind.article(),
                node.node_type
            ))
        }
        "read_size" => {
            let size = number(what, "read_size", value)?;
            if size == 0 {
                return Err(format!(
                    "\"{what}\": read_size must be more than nought -- a read of no \
                     bytes is not a read, and a node that is not read says nothing \
                     about it"
                ));
            }
            said.read_size = Some(size);
        }
        "trigger" => {
            said.trigger = Some(
                crate::trigger::trigger_bytes(value.as_str())
                    .map_err(|why| format!("\"{what}\": trigger: {why}"))?,
            );
        }
        // Written the way a trigger is, because it is bytes written the
        // way a trigger is: only once, as the node opens.
        "init" if is_gateway(&node.node_type) => {
            return Err(format!(
                "\"{what}\" is a link to the ground, and init is what a payload \
                 is set to as the vehicle opens it: a ground link has no state \
                 to start in"
            ))
        }
        "init" => {
            said.init = Some(
                crate::trigger::bytes_of(&crate::trigger::INIT, value.as_str())
                    .map_err(|why| format!("\"{what}\": {why}"))?,
            );
        }
        // Where a datagram payload that speaks first sends from, which is
        // checked once the protocol is known: see `sends_from_of`.
        "sends_from" => said.sends_from = Some(value.clone()),
        // Where it is reached when simulated, for the types the operating
        // system cannot stand in for faithfully. On this host and nowhere
        // else: a stand-in reached off the host would be one nothing here can
        // tell from the hardware.
        "stand_in" if stands_in_over_udp(kind) => {
            let said_at = value.as_str().trim();
            let at: std::net::SocketAddr = said_at.parse().map_err(|_| {
                format!(
                    "\"{what}\": stand_in is \"{said_at}\", which is not an \
                     address:port: give where tcssim answers for it, such as \
                     127.0.0.1:7950"
                )
            })?;
            if !at.ip().is_loopback() {
                return Err(format!(
                    "\"{what}\": stand_in is {at}, which is not on this host: give a \
                     loopback address, such as 127.0.0.1:{}, so that what answers \
                     there can only be tcssim",
                    at.port()
                ));
            }
            said.stand_in = Some(at);
        }
        "stand_in" => {
            return Err(format!(
                "\"{what}\" is {} {} node, which is simulated at its own endpoint, \
                 so stand_in does not apply to it: only hdlc, spi, rpmsg and iio \
                 nodes, which the operating system has nothing faithful to stand \
                 in for, are reached over UDP when simulated",
                kind.article(),
                node.node_type
            ))
        }
        // Where one read of a stream ends. Four attributes rather than a
        // section of its own, because a node's attributes are flat and
        // there is nowhere to nest them.
        "max_length" => {
            said.stream.max_length = Some(value.clone());
            said.stream_stated = true;
        }
        "read_timeout" => {
            said.stream.timeout = Some(value.clone());
            said.stream_stated = true;
        }
        "terminators" => {
            said.stream.terminators = Some(value.clone());
            said.stream_stated = true;
        }
        "length_prefix" => {
            said.stream.length_prefix = Some(value.clone());
            said.stream_stated = true;
        }
        // One a configuration used to write, told what to say instead:
        // see `retired_term`, and the note there about why it is not a
        // misspelling.
        other if retired_term(other).is_some() => {
            return Err(format!(
                "\"{what}\": \"{other}\" is no longer an attribute: {}",
                retired_term(other).expect("just asked")
            ))
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// Why this interface cannot be asked for a reading, if it cannot.
///
/// A trigger is bytes written to a part to make it answer, so an interface
/// that cannot be written at all cannot carry one. Two cannot, and each for
/// its own reason:
///
/// A set of discrete lines is read or driven and not both. Writing to lines
/// that are watched would be driving them, which is commanding a device
/// rather than reading it, and the kernel's request is for one direction.
///
/// A sensor's sample buffer is read and never written. What an IIO device is
/// configured with -- its channels, its sample rate, its trigger in the
/// kernel's sense -- is in `sysfs` and is written by whoever brought the
/// sensor up, not by a datum on this link.
fn cannot_be_asked(endpoint: &EndpointConfig) -> Option<&'static str> {
    match endpoint {
        EndpointConfig::Gpio(_) => Some(
            "a set of discrete lines is read or driven and not both: writing to \
             lines that are watched would be commanding a device rather than \
             reading it",
        ),
        EndpointConfig::Iio(_) => Some(
            "a sensor's sample buffer is read and not written: what an IIO device \
             is configured with is in sysfs, written by whoever brought the sensor \
             up",
        ),
        _ => None,
    }
}

/// Why nothing can be written to this interface as it opens, if nothing can.
///
/// The two a trigger cannot be written to, less the one a setting can: a set
/// of discrete lines that is driven is the very thing an init is for -- a
/// heater that starts off -- where one that is watched is read and not
/// written, and a sensor's sample buffer is never written at all.
fn cannot_be_written(endpoint: &EndpointConfig) -> Option<&'static str> {
    match endpoint {
        EndpointConfig::Gpio(gpio) if !gpio.direction.is_output() => Some(
            "these lines are watched and not driven: an init is written, and \
             writing to lines that are watched would be driving them. Say \
             direction: output if they are driven",
        ),
        EndpointConfig::Iio(_) => Some(
            "a sensor's sample buffer is read and not written: what an IIO device \
             starts with is in sysfs, written by whoever brought the sensor up",
        ),
        _ => None,
    }
}

/// Whether one read of this size fits what the interface carries at once.
///
/// Two interfaces carry a bounded amount and refuse the rest rather than
/// splitting it, so a `read_size` larger than that is a read that can never
/// come back whole -- which is a configuration fault and not a thing to find
/// out in flight.
///
/// A CAN frame carries eight bytes, or sixty-four where the file says
/// `can_fd: true`; an rpmsg message carries what `max_message_bytes` says,
/// and 496 where it says nothing -- 512-byte buffers less the 16-byte header.
/// A set of discrete lines is the other way round: what it delivers is made
/// rather than carried, as records of
/// [`crate::node::payload::gpio::EVENT_BYTES`], and a read takes whole ones
/// only. A `read_size` that is not a whole number of them is room left
/// unfilled on every read -- and one smaller than a record is a node that
/// can never deliver anything, with nothing to say why.
///
/// Every other interface is a stream or a register and has nothing to say
/// here: where one read ends is the `stream` rule's business.
fn fits_one_read(what: &str, size: u32, endpoint: &EndpointConfig) -> Result<(), String> {
    if let EndpointConfig::Gpio(_) = endpoint {
        let record = crate::node::payload::gpio::EVENT_BYTES as u32;
        if size.is_multiple_of(record) {
            return Ok(());
        }
        let whole = (size / record).max(1) * record;
        return Err(if size < record {
            format!(
                "\"{what}\": read_size is {size}, and a set of lines is read in \
                 records of {record} bytes, whole ones only: a read of {size} could \
                 never hold one, so the node would never deliver anything. Say \
                 {record}, or a multiple of it"
            )
        } else {
            format!(
                "\"{what}\": read_size is {size}, and a set of lines is read in \
                 records of {record} bytes, whole ones only: {} of every read would \
                 never be filled. Say {whole}, or another multiple of {record}",
                size % record
            )
        });
    }
    let (room, carries) = match endpoint {
        EndpointConfig::Can(can) => (
            crate::node::payload::can::frame_bytes(can.fd) as u32,
            if can.fd { "an FD frame" } else { "a frame" },
        ),
        EndpointConfig::Rpmsg(rpmsg) => (
            crate::node::payload::rpmsg::message_bytes(rpmsg.max_message_bytes),
            "a message",
        ),
        _ => return Ok(()),
    };

    if size > room {
        return Err(format!(
            "\"{what}\": read_size is {size} and {carries} carries {room} bytes: \
             the rest is refused rather than split, so a read of {size} could \
             never come back whole"
        ));
    }
    Ok(())
}

/// What a `moc` node that binds nothing is given as its address: see
/// [`binds`].
const UNBOUND: &str = "0.0.0.0";

/// Whether a node's endpoint is bound aboard for the far end to reach.
///
/// False only for a gateway node that states no address and no port, which
/// only sends. Everything else opens what it describes.
pub fn binds(node: &Node) -> bool {
    !is_gateway(&node.node_type)
        || node.attributes.contains_key("address")
        || node.attributes.contains_key("port")
}

/// Where a gateway node's data goes: the `address:port` its kind's base
/// attribute states -- a radio's `ground_base`, an F´ gateway's
/// `bridge_base`.
///
/// Required: a ground link with nowhere written down to send would be one
/// that waits to be spoken to before it can report anything. An F´ gateway's
/// `fprime_id_base` is required here too, and read as a whole number, though
/// nothing aboard uses it: a bridge model made from a configuration that did
/// not say where its IDs begin would be one nobody could check.
fn ground_of(what: &str, node: &Node) -> Result<Option<HostPort>, String> {
    let Some(kind) = GatewayKind::of_node_type(&node.node_type) else {
        return Ok(None);
    };
    let base = kind.base_attribute();
    let stated = required(
        what,
        node,
        base,
        "a ground link",
        &format!(
            "where {} is, as address:port -- each node linked into it sends to \
             that port plus its own device id",
            kind.far_end()
        ),
    )?;
    if kind == GatewayKind::Fprime {
        let ids = required(
            what,
            node,
            "fprime_id_base",
            "a link to an F´ bridge",
            "the first of the F´ IDs the bridge gives what arrives -- each node \
             linked into it is that ID plus its own device id",
        )?;
        number(what, "fprime_id_base", ids)?;
    }
    host_port_of(base, stated.as_str())
        .map(Some)
        .map_err(|why| format!("\"{what}\": {why}"))
}

/// Every attribute that says where some type of node is.
///
/// One list, so that an attribute which locates nothing is an unknown
/// attribute rather than one silently accepted on every type.
const LOCATORS: &[&str] = &[
    "path",
    "address",
    "port",
    "protocol",
    "ip_protocol",
    "interface",
    "can_id",
];

/// What locates a node of this type.
fn locators(kind: DHType) -> &'static [&'static str] {
    match kind {
        // A host and a port, and which protocol is spoken to it -- or, on a
        // raw socket, a host and the IP protocol its datagrams are.
        DHType::Network => &["address", "port", "protocol", "ip_protocol"],
        // A bus and the address of one device on it.
        DHType::I2c => &["path", "address"],
        // An interface and the identifier the frames carry: a CAN controller
        // is a network interface and not a device file.
        DHType::Can => &["interface", "can_id"],
        // An interface and nothing else: a raw HDLC frame carries no address,
        // so one line is one node.
        DHType::Hdlc => &["interface"],
        // Everything else is a device node.
        _ => &["path"],
    }
}

/// Which [`DHType`] this node's type is.
///
/// The type names differ from those in two places and both are deliberate.
/// `device-file` is what a configuration calls a file opened in the
/// filesystem, the word `device` having meant that and a piece of hardware on
/// a bus at once. And `moc` is a network link named for what is at the far end
/// of it rather than for how it is reached, because that is the thing a reader
/// of the configuration cares about.
fn kind_of(node: &Node) -> Result<DHType, String> {
    match node.node_type.as_str() {
        "i2c" => Ok(DHType::I2c),
        "spi" => Ok(DHType::Spi),
        "serial" => Ok(DHType::Serial),
        "can" => Ok(DHType::Can),
        "iio" => Ok(DHType::Iio),
        "rpmsg" => Ok(DHType::Rpmsg),
        "gpio" => Ok(DHType::Gpio),
        "hdlc" => Ok(DHType::Hdlc),
        "device-file" => Ok(DHType::Device),
        "network" => Ok(DHType::Network),
        other if is_gateway(other) => Ok(DHType::Network),
        "compute" => Err(format!(
            "\"{}\" is a compute node: it is a computation and not something \
             that is opened",
            node.name
        )),
        "log" => Err(format!(
            "\"{}\" is a log node: what it writes to is a directory, not an \
             interface",
            node.name
        )),
        // An app node is a program of its own, and what is opened for it is a
        // pair of sockets held by this process rather than an interface
        // reached by address: see `crate::node::app` and `tcsapp`.
        other if crate::node::app::Kind::of(other).is_some() => Err(format!(
            "\"{}\" is an app node: a program tcspecial runs, which is reached \
             over what tcspecial makes for it when it starts it, so it takes no \
             address, port or path -- it names its executable, and the \
             arguments its kind is run with",
            node.name
        )),
        other => Err(format!(
            "\"{}\": there is no node type called \"{other}\". The interfaces are \
             i2c, spi, serial, hdlc, can, iio, rpmsg, gpio, device-file and \
             network; a \
             ground link is moc; and compute, log, null-moc, null-payload, \
             tee-moc, tee-payload, ping and the app nodes are the rest",
            node.name
        )),
    }
}

/// Where this node is, by the rules for its type.
fn location_of(what: &str, node: &Node, kind: DHType) -> Result<EndpointLocation, String> {
    // A CAN controller is a network interface and not a device file, and a
    // payload on it is told apart from the others by the identifier its
    // frames carry -- as a device on a bus is told apart by its address.
    if kind == DHType::Can {
        let interface = required(
            what,
            node,
            "interface",
            "a can node",
            "the interface its bus is reached through",
        )?;
        let can_id = required(
            what,
            node,
            "can_id",
            "a can node",
            "the identifier its frames carry",
        )?;
        let can_id = parse_u32(what, "can_id", can_id)
            .and_then(|id| crate::node::payload::can::check_can_id(what, id))
            .map_err(|e| format!("{e}"))?;
        return Ok(EndpointLocation::Can {
            interface: interface.as_str().to_string(),
            can_id,
        });
    }

    // A synchronous line is a network interface and nothing else.
    if kind == DHType::Hdlc {
        let interface = required(
            what,
            node,
            "interface",
            "an hdlc node",
            "the interface its line is",
        )?;
        return Ok(EndpointLocation::Interface {
            interface: interface.as_str().to_string(),
        });
    }

    let path = required(
        what,
        node,
        "path",
        &format!("{} {} node", kind.article(), kind.spelling()),
        "the device it is reached through",
    )?
    .as_str()
    .to_string();

    if kind != DHType::I2c {
        return Ok(EndpointLocation::Path { path });
    }

    // A bus is shared by design and an address on it is not, so both are
    // needed to say which device this is.
    let address = required(
        what,
        node,
        "address",
        "a node on an I2C bus",
        "the address of the device on it",
    )?;
    let ten_bit = node
        .attributes
        .get("ten_bit")
        .map(|t| t.as_str() == "true")
        .unwrap_or(false);

    Ok(EndpointLocation::I2c {
        bus: path,
        address: crate::node::payload::i2c::parse_i2c_address(what, address.as_str(), ten_bit)
            .map_err(|e| format!("{e}"))?,
    })
}

/// An attribute this type cannot do without.
fn required<'a>(
    what: &str,
    node: &'a Node,
    attribute: &str,
    is: &str,
    wants: &str,
) -> Result<&'a Scalar, String> {
    node.attributes
        .get(attribute)
        .ok_or_else(|| format!("\"{what}\" is {is}, so it needs {wants}: say {attribute}"))
}

/// A whole number an attribute states, in decimal or in hexadecimal.
fn number(what: &str, attribute: &str, value: &Scalar) -> Result<u32, String> {
    parse_u32(what, attribute, value).map_err(|e| format!("{e}"))
}

/// The IP protocol a raw network node's datagrams are, from its
/// `ip_protocol`: a number from 1 to 254, IANA's list being where the numbers
/// are given out.
fn ip_protocol_of(what: &str, node: &Node) -> Result<crate::types::IpProtocol, String> {
    let stated = required(
        what,
        node,
        "ip_protocol",
        "a node reached on a raw socket",
        "the IP protocol number its datagrams carry, 1 to 254",
    )?;
    let number = number(what, "ip_protocol", stated)?;
    u8::try_from(number)
        .ok()
        .and_then(crate::types::IpProtocol::of)
        .ok_or_else(|| {
            format!(
                "\"{what}\": ip_protocol {number} is not one a raw socket can be \
                 read with -- it is 1 to 254 (253 and 254 are the two set aside for \
                 experiments): 0 is no protocol, and 255 can only send"
            )
        })
}

/// An attribute a node of this protocol does not take, said in its terms:
/// `ip_protocol` names a raw socket's datagrams and nothing else's.
fn ip_protocol_misplaced(what: &str, node: &Node, protocol: NetworkProtocol) -> Result<(), String> {
    if node.attributes.contains_key("ip_protocol") && !matches!(protocol, NetworkProtocol::Raw(_)) {
        return Err(format!(
            "\"{what}\" is reached over {}, so ip_protocol does not apply to it: it \
             is for protocol: raw, where it says which IP datagrams are the node's",
            protocol.spelling()
        ));
    }
    Ok(())
}

/// The port a network node is at.
fn port_of(what: &str, node: &Node) -> Result<u16, String> {
    port_named(what, node, "port", "a network node", "the port it is at")
}

/// A port, under whichever attribute states it.
fn port_named(
    what: &str,
    node: &Node,
    attribute: &str,
    is: &str,
    wants: &str,
) -> Result<u16, String> {
    let stated = required(what, node, attribute, is, wants)?;
    let port = number(what, attribute, stated)?;

    // Nought is the one number a socket would accept and a configuration
    // cannot mean: it asks the kernel for whichever port is free, so a
    // configuration that stated it would describe a link nothing could be
    // told where to find.
    if port == 0 || port > u16::MAX as u32 {
        return Err(format!(
            "\"{what}\": {attribute} {port} is not a port -- a port is 1 to 65535, and 0 \
             is \"whichever one is free\", which a configuration cannot mean"
        ));
    }

    Ok(port as u16)
}

/// Put one attribute in the term it names, saying whether it was one.
///
/// Every term of every type, in one place: a term is handed to the rules for
/// the node's own type whichever type it belongs to, and those rules refuse
/// the ones that are not theirs. The alternative -- a list per type here --
/// would be a second copy of what each type accepts, and the two would
/// disagree the first time a type gained a term.
///
/// A term a configuration used to write, and what to write instead.
///
/// The spellings three terms had before each became a length of time.
/// Refused by name rather than as a term nothing knows, because the value
/// changes with the name: a node that says `debounce_us: 50` is not a node
/// with a misspelling on it, it is a node written for the spelling before
/// this one. The files here were renamed with these, so the old spelling is
/// nobody's now.
pub fn retired_term(attribute: &str) -> Option<&'static str> {
    match attribute {
        "debounce_us" => Some("say debounce, as a length of time: 50 us rather than 50"),
        "rs485_delay_before_send_ms" => {
            Some("say rs485_delay_before_send, as a length of time: 2 ms rather than 2")
        }
        "rs485_delay_after_send_ms" => {
            Some("say rs485_delay_after_send, as a length of time: 2 ms rather than 2")
        }
        _ => None,
    }
}

fn put_term(terms: &mut LinkTerms, attribute: &str, value: &Scalar) -> bool {
    let value = Some(value.clone());
    match attribute {
        // A line.
        "datarate" => terms.datarate = value,
        "stop_bits" => terms.stop_bits = value,
        // A synchronous line.
        "standard" => terms.standard = value,
        "clock_rate" => terms.clock_rate = value,
        "frame_check" => terms.frame_check = value,
        "parity" => terms.parity = value,
        "clock_type" => terms.clock_type = value,
        "encoding" => terms.encoding = value,
        "loopback" => terms.loopback = value,
        "byte_length" => terms.byte_length = value,
        // A bus, and a device on one.
        "ten_bit" => terms.ten_bit = value,
        "pec" => terms.pec = value,
        "retries" => terms.retries = value,
        "timeout" => terms.timeout = value,
        // A peripheral on a SPI bus.
        "max_speed" => terms.max_speed = value,
        "spi_mode" => terms.spi_mode = value,
        "bits_per_word" => terms.bits_per_word = value,
        "bit_order" => terms.bit_order = value,
        "cs_active" => terms.cs_active = value,
        // A half-duplex RS-485 line.
        "rs485" => terms.rs485 = value,
        "rs485_rts_on_send" => terms.rs485_rts_on_send = value,
        "rs485_delay_before_send" => terms.rs485_delay_before_send = value,
        "rs485_delay_after_send" => terms.rs485_delay_after_send = value,
        // A CAN bus.
        "can_fd" => terms.can_fd = value,
        // A channel to another core.
        "service" => terms.service = value,
        "max_message_bytes" => terms.max_message_bytes = value,
        // Discrete lines.
        "lines" => terms.lines = value,
        "edge" => terms.edge = value,
        "bias" => terms.bias = value,
        "debounce" => terms.debounce = value,
        "state_interval" => terms.state_interval = value,
        "direction" => terms.direction = value,
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoint_config::LengthPrefix;
    use crate::graph::Graph;
    use crate::node::payload::gpio::Direction;
    use crate::node::payload::spi::SpiMode;
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::time::Duration;

    /// The shipped example, which is what this has to be able to open.
    fn thermostat() -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        Graph::load(root.join("tests/manual/thermostat.yaml")).expect("the example reads")
    }

    /// A node built here rather than read, so a test can make one wrong.
    fn node(name: &str, node_type: &str, attributes: &[(&str, &str)]) -> Node {
        Node {
            name: name.to_string(),
            node_type: node_type.to_string(),
            group: None,
            attributes: attributes
                .iter()
                .map(|(a, v)| (a.to_string(), Scalar::of(*v)))
                .collect::<BTreeMap<_, _>>(),
        }
    }

    /// A ground link says where the ground is, as one address and port.
    #[test]
    fn a_ground_link_says_where_the_ground_is() {
        let moc = node("moc", "moc", &[("ground_base", "127.0.0.1:5100")]);
        let reach = reach_of(&moc).expect("it reads");
        let ground = reach.ground.expect("a ground");
        assert_eq!((ground.address.as_str(), ground.port), ("127.0.0.1", 5100));
        // And with no address of its own, it binds nothing.
        assert!(!binds(&moc));

        let mut without = moc.clone();
        without.attributes.remove("ground_base");
        let said = reach_of(&without).expect_err("a ground link says where");
        assert!(said.contains("ground_base"), "{said}");

        let mut portless = moc.clone();
        portless
            .attributes
            .insert("ground_base".to_string(), Scalar::of("127.0.0.1"));
        let said = reach_of(&portless).expect_err("an address and a port");
        assert!(said.contains("ground_base"), "{said}");
    }

    /// A ground link that receives states where, both halves of it.
    #[test]
    fn a_ground_link_binds_both_halves_or_neither() {
        let both = node(
            "moc",
            "moc",
            &[
                ("address", "127.0.0.1"),
                ("port", "5000"),
                ("ground_base", "127.0.0.1:5100"),
            ],
        );
        assert!(binds(&both));
        assert!(reach_of(&both).is_ok());

        for (missing, said_of) in [("address", "address"), ("port", "port")] {
            let mut half = both.clone();
            half.attributes.remove(missing);
            let said = reach_of(&half).expect_err("half an address");
            assert!(said.contains(said_of), "{said}");
        }
    }

    /// Only a gateway node has a ground_base.
    #[test]
    fn a_ground_base_is_refused_anywhere_else() {
        let network = node(
            "sensor",
            "network",
            &[
                ("protocol", "udp"),
                ("address", "127.0.0.1"),
                ("port", "5001"),
                ("ground_base", "127.0.0.1:5100"),
            ],
        );
        let said = reach_of(&network).expect_err("not a ground link");
        assert!(said.contains("only a gateway node"), "{said}");
    }

    /// Every device of the worked example becomes what the flight software
    /// opens.
    ///
    /// The example and not an arrangement resembling it: these are the four
    /// types the thermostat actually uses, read out of the shipped file.
    #[test]
    fn the_worked_example_opens() {
        let graph = thermostat();
        let reach = |name: &str| {
            reach_of(graph.node(name).expect("a node of the example"))
                .unwrap_or_else(|why| panic!("{name}: {why}"))
        };

        // A device on a bus: the bus, and the address that tells it from the
        // other sensor on the same wires.
        let fore = reach("temp_1");
        match &fore.endpoint {
            EndpointConfig::I2c(i2c) => {
                assert_eq!(i2c.bus, "/dev/i2c-1");
                assert_eq!(
                    i2c.address, 0x48,
                    "0x48 and not the 72 a YAML reader makes of it"
                );
                assert!(!i2c.ten_bit);
                assert!(!i2c.pec);
            }
            other => panic!("the forward sensor should be on a bus: {other:?}"),
        }
        assert_eq!(
            fore.read_size,
            Some(2),
            "two bytes, which is what the part answers in"
        );
        assert_eq!(fore.trigger, None);

        // And the one in the equipment bay, which is a peripheral on a SPI
        // bus and answers in three.
        let bay = reach("temp_3");
        match &bay.endpoint {
            EndpointConfig::Spi(spi) => {
                assert_eq!(spi.path, "/dev/spidev0.0");
                assert_eq!(spi.max_speed, 1_000_000);
                assert_eq!(spi.mode, SpiMode::Mode0);
            }
            other => panic!("the bay sensor should be on a SPI bus: {other:?}"),
        }
        assert_eq!(bay.read_size, Some(3));

        // The heaters: three lines of one chip, driven.
        match &reach("heaters_1").endpoint {
            EndpointConfig::Gpio(gpio) => {
                assert_eq!(gpio.path, "/dev/gpiochip0");
                assert_eq!(gpio.lines, vec![3, 4, 5]);
                assert_eq!(gpio.direction, Direction::Output);
                assert_eq!(gpio.debounce, None);
            }
            other => panic!("the heaters should be discrete lines: {other:?}"),
        }

        // And the ground link, which needs no protocol: a ground link is a
        // datagram link.
        match &reach("moc").endpoint {
            EndpointConfig::Network(net) => {
                assert_eq!(net.protocol, NetworkProtocol::Udp);
                assert_eq!(net.address, "10.0.0.1");
                assert_eq!(net.port, 5000);
            }
            other => panic!("the ground link should be a socket: {other:?}"),
        }
    }

    /// What is not reached over an interface says so.
    #[test]
    fn a_computation_and_a_log_are_not_opened() {
        let graph = thermostat();

        let said = reach_of(graph.node("thermostat").expect("the thermostat"))
            .expect_err("a computation opened");
        assert!(
            said.contains("thermostat") && said.contains("not something that is opened"),
            "{said}"
        );

        let said =
            reach_of(graph.node("housekeeping").expect("the log")).expect_err("a log opened");
        assert!(said.contains("directory"), "{said}");

        let said = reach_of(&node("mystery", "quantum", &[])).expect_err("a type that is not one");
        assert!(
            said.contains("quantum") && said.contains("device-file"),
            "the complaint should list the types there are: {said}"
        );
    }

    /// An attribute nothing reads is refused, not ignored.
    ///
    /// The reason this bridge checks names at all. A term belonging to
    /// another type reaches that type's own rules and is refused in their
    /// words; a term belonging to no type would simply vanish, and a
    /// peripheral clocked at whatever the last program left the bus at looks
    /// exactly like a configured one.
    #[test]
    fn an_attribute_nothing_reads_is_refused() {
        let said = reach_of(&node(
            "temp_3",
            "spi",
            &[("path", "/dev/spidev0.0"), ("max_speeed", "1000000")],
        ))
        .expect_err("a misspelled term");
        assert!(
            said.contains("max_speeed") && said.contains("looks like something that was set"),
            "{said}"
        );
    }

    /// A term of another type of link is refused by that type's own rules.
    #[test]
    fn a_term_of_another_link_is_refused() {
        let said = reach_of(&node(
            "temp_1",
            "i2c",
            &[
                ("path", "/dev/i2c-1"),
                ("address", "0x48"),
                ("datarate", "9600"),
            ],
        ))
        .expect_err("a line's rate on a bus");
        assert!(
            said.contains("datarate"),
            "the complaint should name the term: {said}"
        );
    }

    /// An attribute that locates another type of node is refused.
    #[test]
    fn an_attribute_that_locates_another_type_is_refused() {
        let said = reach_of(&node(
            "heaters",
            "gpio",
            &[("path", "/dev/gpiochip0"), ("lines", "3"), ("port", "5000")],
        ))
        .expect_err("a port on a set of lines");
        assert!(
            said.contains("port does not locate it") && said.contains("path"),
            "the complaint should say what does locate it: {said}"
        );

        // And the other way round: a socket has no device file.
        let said = reach_of(&node(
            "moc",
            "moc",
            &[
                ("address", "10.0.0.1"),
                ("port", "5000"),
                ("path", "/dev/null"),
            ],
        ))
        .expect_err("a path on a socket");
        assert!(said.contains("path does not locate it"), "{said}");
    }

    /// A synchronous line is located by an interface and nothing else.
    #[test]
    fn a_synchronous_line_is_an_interface() {
        let reach = reach_of(&node(
            "link",
            "hdlc",
            &[
                ("interface", "hdlc0"),
                ("standard", "v35"),
                ("clock_type", "internal"),
                ("clock_rate", "2048000"),
                ("frame_check", "crc16_pr1_ccitt"),
            ],
        ))
        .expect("a line");

        match reach.endpoint {
            EndpointConfig::Hdlc(hdlc) => {
                assert_eq!(hdlc.interface, "hdlc0");
                assert_eq!(hdlc.clock_rate, Some(2_048_000));
            }
            other => panic!("expected a synchronous line: {other:?}"),
        }

        // A path does not locate one: the kernel drives such a line as a
        // network interface, so there is no file to open.
        let said = reach_of(&node(
            "link",
            "hdlc",
            &[
                ("path", "/dev/ttyS1"),
                ("standard", "v35"),
                ("clock_type", "external"),
            ],
        ))
        .expect_err("a path on a synchronous line");
        assert!(said.contains("path does not locate it"), "{said}");

        // And a start-stop line's words are not its.
        let said = reach_of(&node(
            "link",
            "hdlc",
            &[
                ("interface", "hdlc0"),
                ("standard", "v35"),
                ("clock_type", "external"),
                ("stop_bits", "1"),
            ],
        ))
        .expect_err("stop bits on a clocked line");
        assert!(said.contains("stop_bits"), "{said}");
    }

    /// A node that does not say where it is cannot be opened.
    #[test]
    fn a_node_that_says_nowhere_is_refused() {
        for (name, node_type, attributes, wanted) in [
            ("temp_1", "i2c", vec![("path", "/dev/i2c-1")], "address"),
            ("temp_1", "i2c", vec![("address", "0x48")], "path"),
            ("link", "moc", vec![("port", "5000")], "address"),
            ("link", "moc", vec![("address", "10.0.0.1")], "port"),
            ("file", "device-file", vec![], "path"),
            ("bus", "can", vec![("can_id", "0x201")], "interface"),
            ("bus", "can", vec![("interface", "vcan0")], "can_id"),
        ] {
            let said = reach_of(&node(name, node_type, &attributes))
                .expect_err("a node that says nowhere");
            assert!(
                said.contains(wanted),
                "the complaint should name what is missing ({wanted}): {said}"
            );
        }
    }

    /// Two kinds of interface cannot be asked, and a trigger on one is
    /// refused.
    ///
    /// A trigger is bytes written to a part to make it answer, and these two
    /// are not written: a set of discrete lines is read or driven and not
    /// both, and a sensor's sample buffer is read and never written -- what an
    /// IIO device is configured with is in `sysfs`. Each says which of the two
    /// reasons it is, because they are different mistakes: one is a direction
    /// and the other is a place.
    #[test]
    fn an_interface_that_cannot_be_asked_refuses_a_trigger() {
        let said = reach_of(&node(
            "asked_lines",
            "gpio",
            &[
                ("path", "/dev/gpiochip0"),
                ("lines", "0"),
                ("trigger", "[55]"),
            ],
        ))
        .expect_err("a set of discrete lines cannot be asked");
        assert!(
            said.contains("asked_lines") && said.contains("driven"),
            "the complaint should say why it cannot be asked: {said}"
        );

        let said = reach_of(&node(
            "asked_sensor",
            "iio",
            &[("path", "/dev/iio:device0"), ("trigger", "[55]")],
        ))
        .expect_err("a sample buffer cannot be asked");
        assert!(
            said.contains("asked_sensor") && said.contains("sysfs"),
            "the complaint should say where it is configured instead: {said}"
        );

        // And the kinds that can be asked still are: a register on a bus is
        // read by asking for it.
        reach_of(&node(
            "temp_1",
            "i2c",
            &[
                ("path", "/dev/i2c-1"),
                ("address", "0x48"),
                ("read_size", "2"),
                ("trigger", "[55]"),
            ],
        ))
        .expect("a register is read by asking for it");
    }

    /// An init is bytes written the way a trigger is, as the node opens, and
    /// is refused where nothing can be written: lines that are watched, a
    /// sensor's sample buffer, and a link to the ground, which has no state to
    /// start in. Driven lines take one: a heater that starts off is what it is
    /// for.
    #[test]
    fn an_init_is_taken_where_the_payload_can_be_written() {
        let heaters = reach_of(&node(
            "heaters",
            "gpio",
            &[
                ("path", "/dev/gpiochip0"),
                ("lines", "4,5"),
                ("direction", "output"),
                ("init", "\"0\""),
            ],
        ))
        .expect("driven lines take an init");
        assert_eq!(heaters.init.as_deref(), Some(&b"0"[..]));

        let sensor = reach_of(&node(
            "temp_1",
            "i2c",
            &[
                ("path", "/dev/i2c-1"),
                ("address", "0x48"),
                ("init", "\"\\x01\\x60\""),
            ],
        ))
        .expect("a register set before the part reads sensibly");
        assert_eq!(sensor.init.as_deref(), Some(&[0x01, 0x60][..]));

        let said = reach_of(&node(
            "switches",
            "gpio",
            &[("path", "/dev/gpiochip0"), ("lines", "0"), ("init", "[01]")],
        ))
        .expect_err("watched lines are not written");
        assert!(
            said.contains("switches") && said.contains("direction: output"),
            "{said}"
        );

        let said = reach_of(&node(
            "accel",
            "iio",
            &[("path", "/dev/iio:device0"), ("init", "[01]")],
        ))
        .expect_err("a sample buffer is not written");
        assert!(said.contains("accel") && said.contains("sysfs"), "{said}");

        let said = reach_of(&node(
            "link",
            "moc",
            &[("address", "10.0.0.1"), ("port", "5000"), ("init", "[01]")],
        ))
        .expect_err("a ground link has no state to start in");
        assert!(said.contains("link") && said.contains("init"), "{said}");

        let said = reach_of(&node(
            "temp_1",
            "i2c",
            &[("path", "/dev/i2c-1"), ("address", "0x48"), ("init", "zz")],
        ))
        .expect_err("bytes that are not bytes");
        assert!(said.contains("init") && !said.contains("trigger"), "{said}");
    }

    /// A set of lines is read in whole records, so a read_size that is not a
    /// whole number of them is refused -- one smaller than a record being a
    /// node that would never deliver anything -- and one that is, is not.
    #[test]
    fn a_set_of_lines_is_read_in_whole_records() {
        let lines = |size: &str| {
            reached(
                "gpio",
                &[
                    ("path", "/dev/gpiochip0"),
                    ("lines", "3"),
                    ("read_size", size),
                ],
            )
        };
        lines("16").expect("one record");
        lines("64").expect("four records");

        let short = lines("10").expect_err("less than a record");
        assert!(short.contains("never deliver anything"), "{short}");
        let ragged = lines("20").expect_err("a record and a piece");
        assert!(ragged.contains("Say 16"), "{ragged}");
    }

    /// A read larger than the interface carries at once is refused.
    ///
    /// Two interfaces carry a bounded amount and refuse the rest rather than
    /// splitting it, so such a read could never come back whole. A CAN frame
    /// carries eight bytes, or sixty-four in an FD frame; an rpmsg message
    /// carries 496 -- 512-byte buffers less the header -- or what
    /// `max_message_bytes` says. Both numbers are in the complaint, because
    /// what a reader has to decide is which of the two to change.
    #[test]
    fn a_read_larger_than_the_interface_carries_is_refused() {
        let frame = |terms: &[(&str, &str)]| {
            let mut all = vec![("interface", "vcan0"), ("can_id", "1")];
            all.extend_from_slice(terms);
            reach_of(&node("big_frame", "can", &all))
        };

        let said = frame(&[("read_size", "12")]).expect_err("twelve bytes in eight");
        assert!(said.contains("12") && said.contains('8'), "{said}");

        // Eight fits, and sixty-four fits an FD frame -- which is what the
        // flag is for.
        frame(&[("read_size", "8")]).expect("eight bytes of frame");
        frame(&[("read_size", "12"), ("can_fd", "true")]).expect("an FD frame has room");
        let said = frame(&[("read_size", "65"), ("can_fd", "true")])
            .expect_err("sixty-five bytes in sixty-four");
        assert!(said.contains("65") && said.contains("64"), "{said}");

        let message = |terms: &[(&str, &str)]| {
            let mut all = vec![("path", "/dev/rpmsg0"), ("service", "payload")];
            all.extend_from_slice(terms);
            reach_of(&node("big_message", "rpmsg", &all))
        };

        let said = message(&[("read_size", "600")]).expect_err("600 bytes in 496");
        assert!(said.contains("600") && said.contains("496"), "{said}");

        // And a platform with larger buffers says so, which makes it fit.
        message(&[("read_size", "600"), ("max_message_bytes", "2048")])
            .expect("a platform with room for it");
        let said = message(&[("read_size", "2049"), ("max_message_bytes", "2048")])
            .expect_err("one byte over what it said");
        assert!(said.contains("2049") && said.contains("2048"), "{said}");
    }

    /// A raw socket is a protocol with its number beside it: an address, an
    /// `ip_protocol` from 1 to 254, and no port. And the number belongs to
    /// a raw socket and to nothing else.
    #[test]
    fn a_raw_node_states_its_ip_protocol_and_no_port() {
        let raw = |extra: &[(&str, &str)]| {
            let mut all = vec![("address", "10.0.0.7"), ("protocol", "raw")];
            all.extend_from_slice(extra);
            reach_of(&node("sounder", "network", &all))
        };

        match raw(&[("ip_protocol", "253")]).expect("a raw node").endpoint {
            EndpointConfig::Network(net) => {
                assert_eq!(
                    net.protocol,
                    NetworkProtocol::Raw(crate::IpProtocol::of(253).expect("a protocol"))
                );
                assert_eq!(net.address, "10.0.0.7");
            }
            other => panic!("expected a network endpoint, got {other:?}"),
        }

        let said = raw(&[]).expect_err("no number");
        assert!(said.contains("ip_protocol"), "{said}");
        for number in ["0", "255", "300"] {
            let said = raw(&[("ip_protocol", number)]).expect_err(number);
            assert!(said.contains(number) && said.contains("1 to 254"), "{said}");
        }
        let said = raw(&[("ip_protocol", "253"), ("port", "5000")]).expect_err("a port");
        assert!(said.contains("no port"), "{said}");

        let said = reach_of(&node(
            "link",
            "network",
            &[
                ("address", "10.0.0.7"),
                ("port", "5000"),
                ("protocol", "udp"),
                ("ip_protocol", "253"),
            ],
        ))
        .expect_err("a number on a udp node");
        assert!(
            said.contains("ip_protocol") && said.contains("udp"),
            "{said}"
        );
    }

    /// A read of no bytes is not a read.
    #[test]
    fn a_read_size_of_nothing_is_refused() {
        let said = reach_of(&node(
            "temp_1",
            "i2c",
            &[
                ("path", "/dev/i2c-1"),
                ("address", "0x48"),
                ("read_size", "0"),
            ],
        ))
        .expect_err("a read of no bytes");
        assert!(said.contains("read_size"), "{said}");
    }

    /// A port is a port, and nought is not one.
    #[test]
    fn a_port_is_a_port() {
        let said = reach_of(&node(
            "link",
            "moc",
            &[("address", "10.0.0.1"), ("port", "70000")],
        ))
        .expect_err("a number that is not a port");
        assert!(said.contains("70000"), "{said}");

        let said = reach_of(&node(
            "link",
            "moc",
            &[("address", "10.0.0.1"), ("port", "0")],
        ))
        .expect_err("port nought");
        assert!(said.contains("whichever one is free"), "{said}");
    }

    /// A network node says which protocol is spoken to it; a MOC link need
    /// not.
    #[test]
    fn a_network_node_says_its_protocol() {
        let said = reach_of(&node(
            "payload",
            "network",
            &[("address", "127.0.0.1"), ("port", "5000")],
        ))
        .expect_err("a socket with no protocol");
        assert!(said.contains("protocol"), "{said}");

        let reach = reach_of(&node(
            "payload",
            "network",
            &[
                ("address", "127.0.0.1"),
                ("port", "5000"),
                ("protocol", "tcp"),
                ("max_length", "64"),
                ("read_timeout", "250 ms"),
            ],
        ))
        .expect("a TCP payload");
        match reach.endpoint {
            EndpointConfig::Network(net) => assert_eq!(net.protocol, NetworkProtocol::Tcp),
            other => panic!("{other:?}"),
        }

        let said = reach_of(&node(
            "payload",
            "network",
            &[
                ("address", "127.0.0.1"),
                ("port", "5000"),
                ("protocol", "smoke"),
            ],
        ))
        .expect_err("a protocol that is not one");
        assert!(said.contains("smoke"), "{said}");
    }

    /// A stream socket is a stream of bytes, as a line is, so it says where
    /// one of its packets ends by the same rule, and must; a datagram is a
    /// packet already, and is refused one.
    #[test]
    fn a_stream_socket_says_where_a_packet_ends() {
        let socket = |protocol: &str, extra: &[(&str, &str)]| {
            let mut attributes = vec![
                ("protocol", protocol),
                ("address", "127.0.0.1"),
                ("port", "5000"),
            ];
            attributes.extend_from_slice(extra);
            reached("network", &attributes)
        };
        for protocol in ["tcp", "unix_stream"] {
            let reach = socket(
                protocol,
                &[("max_length", "128"), ("terminators", "\"\\r\\n\"")],
            )
            .unwrap_or_else(|e| panic!("{protocol}: {e}"));
            let rule = reach.stream.expect("a stream's packets end somewhere");
            assert_eq!(rule.max_length, 128);
            assert_eq!(rule.terminators, vec![b"\r\n".to_vec()]);

            let said = socket(protocol, &[]).expect_err("a stream with no rule");
            assert!(
                said.contains(protocol) && said.contains("stream") && said.contains("max_length"),
                "{protocol}: {said}"
            );
        }
        for protocol in ["udp", "unix_dgram"] {
            assert_eq!(socket(protocol, &[]).expect("a datagram").stream, None);
            let said = socket(protocol, &[("max_length", "128"), ("read_timeout", "none")])
                .expect_err("a datagram is a packet already");
            assert!(said.contains("packets already"), "{protocol}: {said}");
        }
    }

    /// A line says where one of its reads ends, as three attributes.
    ///
    /// A node's attributes are flat, so they are written beside the rest of
    /// the line's terms.
    #[test]
    fn a_line_says_where_a_read_ends() {
        let reach = reach_of(&node(
            "console",
            "serial",
            &[
                ("path", "/dev/ttyS0"),
                ("datarate", "9600"),
                ("stop_bits", "1"),
                ("parity", "none"),
                ("byte_length", "8"),
                ("max_length", "512"),
                ("terminators", "[0A]"),
            ],
        ))
        .expect("a line");
        match reach.endpoint {
            EndpointConfig::Serial(serial) => {
                assert_eq!(serial.path, "/dev/ttyS0");
                assert_eq!(serial.datarate, 9600);
            }
            other => panic!("{other:?}"),
        }

        // A line that says nothing about where a read ends is refused, by the
        // rule that has always refused it.
        let said = reach_of(&node(
            "console",
            "serial",
            &[
                ("path", "/dev/ttyS0"),
                ("datarate", "9600"),
                ("stop_bits", "1"),
                ("parity", "none"),
                ("byte_length", "8"),
            ],
        ))
        .expect_err("a read with no end");
        assert!(said.contains("stream"), "{said}");

        // And those three on something that is not a line.
        let said = reach_of(&node(
            "file",
            "device-file",
            &[("path", "/dev/urandom"), ("max_length", "512")],
        ))
        .expect_err("where a read ends, on a file");
        assert!(
            said.contains("max_length") && said.contains("property of a"),
            "{said}"
        );
    }

    /// A node that must be asked carries what asks it.
    #[test]
    fn a_trigger_is_read_as_the_bytes_it_is() {
        let reach = reach_of(&node(
            "temp_1",
            "i2c",
            &[
                ("path", "/dev/i2c-1"),
                ("address", "0x48"),
                ("read_size", "2"),
                ("trigger", "[00]"),
            ],
        ))
        .expect("a sensor that is asked");
        assert_eq!(reach.trigger, Some(vec![0x00]));

        // A command and a register, which is what an interface document
        // commonly asks for: two values, the second of them two bytes.
        let reach = reach_of(&node(
            "temp_1",
            "i2c",
            &[
                ("path", "/dev/i2c-1"),
                ("address", "0x48"),
                ("trigger", "\"\\x03\\x04\\x00\""),
            ],
        ))
        .expect("a sensor that is asked");
        assert_eq!(reach.trigger, Some(vec![0x03, 0x04, 0x00]));

        // And a trigger that is text, which is a C string: the quotes are
        // part of the value.
        let reach = reach_of(&node(
            "console",
            "device-file",
            &[("path", "/dev/ttyS0"), ("trigger", "\"READ\\r\"")],
        ))
        .expect("a console that is asked");
        assert_eq!(reach.trigger, Some(b"READ\r".to_vec()));

        let said = reach_of(&node(
            "temp_1",
            "i2c",
            &[
                ("path", "/dev/i2c-1"),
                ("address", "0x48"),
                ("trigger", "zz"),
            ],
        ))
        .expect_err("a trigger that is not bytes");
        assert!(said.contains("trigger"), "{said}");
    }

    /// The group a node is read with is the schedule's business, not this
    /// one's.
    #[test]
    fn a_group_is_not_an_attribute_of_the_interface() {
        let reach = reach_of(&node(
            "temp_1",
            "i2c",
            &[
                ("path", "/dev/i2c-1"),
                ("address", "0x48"),
                ("group", "cabin_temps"),
            ],
        ))
        .expect("a node in a group");
        assert!(matches!(reach.endpoint, EndpointConfig::I2c(_)));
    }

    // -- the rules of a link, as a node states them ---------------------------
    //
    // A line, a bus and a peripheral each have rules of their own: which terms
    // the kind takes, which it must be told, and which belong to another kind.
    // The rules are stated once, in `endpoint_config` and its per-kind
    // modules, and tested there. These are the other half: that a node
    // stating them reaches an endpoint carrying them. A term read and dropped
    // on the way is a line set to something nobody asked for, and nothing in
    // the rule's own tests would notice.

    /// What a node of this type and these attributes reaches.
    fn reached(node_type: &str, attributes: &[(&str, &str)]) -> Result<Reach, String> {
        reach_of(&node("p", node_type, attributes))
    }

    /// The endpoint a node of this type and these attributes opens.
    fn endpoint(node_type: &str, attributes: &[(&str, &str)]) -> Result<EndpointConfig, String> {
        reached(node_type, attributes).map(|reach| reach.endpoint)
    }

    /// What the rules for the link settle, for the terms that never reach an
    /// endpoint -- where a read of a line ends, the knobs of a bus and a
    /// peripheral -- and so cannot be read back from `reach_of`.
    ///
    /// The node is read through `reach_of` first, so a refusal is the one a
    /// configuration gets. Then the terms are gathered as `reach_of` gathers
    /// them -- `put_term`, and the three attributes of where a read ends --
    /// and handed to the same `kind_of_terms` it hands them to.
    fn settled(
        node_type: &str,
        attributes: &[(&str, &str)],
    ) -> Result<crate::endpoint_config::GroupKind, String> {
        let node = node("p", node_type, attributes);
        reach_of(&node)?;
        let mut terms = LinkTerms::default();
        let mut stream = StreamTerms {
            max_length: None,
            timeout: None,
            terminators: None,
            length_prefix: None,
        };
        let mut stream_stated = false;
        for (attribute, value) in &node.attributes {
            if put_term(&mut terms, attribute, value) {
                continue;
            }
            match attribute.as_str() {
                "max_length" => stream.max_length = Some(value.clone()),
                "read_timeout" => stream.timeout = Some(value.clone()),
                "terminators" => stream.terminators = Some(value.clone()),
                "length_prefix" => stream.length_prefix = Some(value.clone()),
                _ => continue,
            }
            stream_stated = true;
        }
        if stream_stated {
            terms.stream = Some(stream);
        }
        crate::types::kind_of_terms("p", kind_of(&node)?, terms)
    }

    /// The terms of a line, which is what the stream rule belongs to.
    const A_LINE: &[(&str, &str)] = &[
        ("path", "/dev/ttyS0"),
        ("datarate", "9600"),
        ("stop_bits", "1"),
        ("byte_length", "8"),
    ];

    /// A line's terms with these beside them.
    fn line_with<'a>(extra: &[(&'a str, &'a str)]) -> Vec<(&'a str, &'a str)> {
        A_LINE.iter().chain(extra).copied().collect()
    }

    /// An RS-485 bus reaches its endpoint with its turnaround on it.
    #[test]
    fn an_rs485_line_carries_its_turnaround_to_the_endpoint() {
        let bus = line_with(&[
            ("rs485", "true"),
            ("rs485_rts_on_send", "false"),
            ("rs485_delay_after_send", "2 ms"),
            ("max_length", "64"),
            ("read_timeout", "none"),
        ]);
        match endpoint("serial", &bus).expect("it opens") {
            EndpointConfig::Serial(serial) => {
                let rs485 = serial.rs485.expect("the bus reached the endpoint");
                assert!(!rs485.rts_on_send, "RTS is low while sending on this board");
                assert_eq!(
                    rs485.delay_before_send,
                    Duration::ZERO,
                    "nothing was asked for"
                );
                assert_eq!(rs485.delay_after_send, Duration::from_millis(2));
            }
            other => panic!("expected a serial endpoint, got {other:?}"),
        }

        // And a line that says nothing is not a bus.
        let plain = line_with(&[("max_length", "64"), ("read_timeout", "none")]);
        match endpoint("serial", &plain).expect("it opens") {
            EndpointConfig::Serial(serial) => assert_eq!(serial.rs485, None),
            other => panic!("expected a serial endpoint, got {other:?}"),
        }
    }

    /// The turnaround belongs to a line and to no other kind.
    #[test]
    fn rs485_terms_are_refused_on_a_kind_that_is_not_a_line() {
        let said = endpoint(
            "spi",
            &[
                ("path", "/dev/spidev0.0"),
                ("max_speed", "1000000"),
                ("spi_mode", "0"),
                ("rs485", "true"),
            ],
        )
        .expect_err("a peripheral is not an RS-485 bus");
        assert!(
            said.contains("rs485"),
            "the error should name the term: {said}"
        );
    }

    /// A GPIO node reaches its endpoint with the lines it watches and how.
    #[test]
    fn a_gpio_node_is_a_chip_and_the_lines_it_watches() {
        let opened = endpoint(
            "gpio",
            &[
                ("path", "/dev/gpiochip0"),
                ("lines", "3, 7"),
                ("edge", "rising"),
                ("bias", "pull_up"),
                ("debounce", "5000 us"),
            ],
        )
        .expect("it opens");
        match &opened {
            EndpointConfig::Gpio(gpio) => {
                assert_eq!(gpio.path, "/dev/gpiochip0");
                assert_eq!(gpio.lines, vec![3, 7]);
                assert_eq!(gpio.edge, crate::LineEdge::Rising);
                assert_eq!(gpio.bias, crate::Bias::PullUp);
                assert_eq!(gpio.debounce, Some(Duration::from_micros(5_000)));
            }
            other => panic!("expected a GPIO endpoint, got {other:?}"),
        }
        assert_eq!(opened.kind(), DHType::Gpio);
    }

    /// An rpmsg node reaches its endpoint with its channel and its service.
    /// A type the operating system cannot stand in for may give a stand_in,
    /// and is reached there over UDP only in a run with stand-ins: under the
    /// flight setting the device is what is opened.
    #[test]
    fn a_stand_in_is_reached_over_udp_only_when_the_run_has_stand_ins() {
        use crate::simulate::Mode;
        let rpmsg = node(
            "channel",
            "rpmsg",
            &[
                ("path", "/dev/rpmsg0"),
                ("service", "payload-telemetry"),
                ("stand_in", "127.0.0.1:7950"),
            ],
        );
        let reach = reach_of(&rpmsg).expect("it opens");
        assert_eq!(
            reach.opened(Mode::None),
            reach.endpoint,
            "flight opens the device"
        );
        for mode in [Mode::Some, Mode::All] {
            assert_eq!(
                reach.opened(mode),
                EndpointConfig::Network(NetworkConfig {
                    protocol: NetworkProtocol::Udp,
                    address: "127.0.0.1".to_string(),
                    port: 7950,
                })
            );
        }
        // Still what it is, for everything that asks what it is.
        assert_eq!(reach.endpoint.kind(), DHType::Rpmsg);
    }

    /// Only on this host, and only on the types that need one.
    #[test]
    fn a_stand_in_is_refused_off_the_host_and_on_a_type_with_a_stand_in_of_its_own() {
        let off = node(
            "line",
            "hdlc",
            &[("interface", "hdlc0"), ("stand_in", "10.0.0.5:7950")],
        );
        let said = reach_of(&off).unwrap_err();
        assert!(said.contains("not on this host"), "{said}");

        let serial = node(
            "line",
            "serial",
            &[
                ("path", "/dev/ttyS0"),
                ("datarate", "9600"),
                ("max_length", "64"),
                ("read_timeout", "none"),
                ("stand_in", "127.0.0.1:7950"),
            ],
        );
        let said = reach_of(&serial).unwrap_err();
        assert!(said.contains("stand_in does not apply to it"), "{said}");
    }

    #[test]
    fn an_rpmsg_node_is_a_channel_and_the_service_it_should_be() {
        let opened = endpoint(
            "rpmsg",
            &[("path", "/dev/rpmsg0"), ("service", "payload-telemetry")],
        )
        .expect("it opens");
        match &opened {
            EndpointConfig::Rpmsg(rpmsg) => {
                assert_eq!(rpmsg.path, "/dev/rpmsg0");
                assert_eq!(rpmsg.service, "payload-telemetry");
                assert_eq!(rpmsg.max_message_bytes, None, "the usual transport");
            }
            other => panic!("expected an rpmsg endpoint, got {other:?}"),
        }
        assert_eq!(opened.kind(), DHType::Rpmsg);

        // The service is what identifies a channel, so a node without one is
        // refused: a channel's number is assigned as channels appear.
        let said =
            endpoint("rpmsg", &[("path", "/dev/rpmsg0")]).expect_err("a channel with no service");
        assert!(
            said.contains("service"),
            "the error should name what is missing: {said}"
        );
    }

    /// Where a datagram payload sends from is a host and port over UDP and a
    /// path over a Unix datagram socket, and is refused on anything else and
    /// in any shape that names no one sender.
    #[test]
    fn sends_from_names_one_sender_of_datagrams() {
        let udp = |from: &str| {
            reached(
                "network",
                &[
                    ("protocol", "udp"),
                    ("address", "127.0.0.1"),
                    ("port", "6200"),
                    ("sends_from", from),
                ],
            )
        };
        assert_eq!(
            udp("10.0.0.7:7001").expect("a host and a port").sends_from,
            Some(crate::SendsFrom::Socket("10.0.0.7:7001".parse().unwrap()))
        );
        for (from, says) in [
            ("10.0.0.7", "host and a port"),
            ("sensor.local:7001", "host and a port"),
            ("10.0.0.7:0", "port 0"),
            ("127.0.0.1:6200", "own address"),
        ] {
            let why = udp(from).expect_err(from);
            assert!(why.contains(says), "{from}: {why}");
        }

        let unix = |from: &str| {
            reached(
                "network",
                &[
                    ("protocol", "unix_dgram"),
                    ("address", "/run/vehicle.sock"),
                    ("port", "5200"),
                    ("sends_from", from),
                ],
            )
        };
        assert_eq!(
            unix("/run/sensor.sock").expect("a path").sends_from,
            Some(crate::SendsFrom::Path("/run/sensor.sock".to_string()))
        );
        assert!(unix("/run/vehicle.sock")
            .unwrap_err()
            .contains("own address"));

        let tcp = reached(
            "network",
            &[
                ("protocol", "tcp"),
                ("address", "127.0.0.1"),
                ("port", "6200"),
                ("max_length", "64"),
                ("read_timeout", "none"),
                ("sends_from", "127.0.0.1:6201"),
            ],
        )
        .expect_err("a stream is connected");
        assert!(tcp.contains("Take sends_from out"), "{tcp}");

        let file = reached(
            "device-file",
            &[("path", "/dev/null"), ("sends_from", "/run/x.sock")],
        )
        .expect_err("a file sends from nowhere");
        assert!(file.contains("only a network node"), "{file}");
    }

    /// A network node may be asked, whatever its protocol, datagrams
    /// included.
    ///
    /// A node states where it is, so the vehicle has somewhere to send a
    /// trigger before the node has said anything: the question goes to the
    /// address and port the node states, and the answer comes back on the
    /// same socket.
    #[test]
    fn a_datagram_node_may_be_triggered() {
        for protocol in ["udp", "unix_dgram", "tcp"] {
            let reach = reached(
                "network",
                &[
                    ("protocol", protocol),
                    ("address", "127.0.0.1"),
                    ("port", "5000"),
                    ("trigger", "\"READ\""),
                ]
                .into_iter()
                // A stream says where its packets end; see
                // a_stream_socket_says_where_a_packet_ends.
                .chain(
                    (protocol == "tcp")
                        .then_some([("max_length", "64"), ("read_timeout", "none")])
                        .into_iter()
                        .flatten(),
                )
                .collect::<Vec<_>>(),
            )
            .unwrap_or_else(|e| panic!("{protocol}: {e}"));
            assert_eq!(
                reach.trigger.as_deref(),
                Some(b"READ".as_slice()),
                "{protocol}"
            );
        }
    }

    /// An rpmsg node may be asked: the channel carries both directions.
    #[test]
    fn an_rpmsg_node_may_be_triggered() {
        let asked = endpoint(
            "rpmsg",
            &[
                ("path", "/dev/rpmsg0"),
                ("service", "payload"),
                ("trigger", "\"READ\""),
            ],
        )
        .expect("a channel may be asked");
        assert!(matches!(asked, EndpointConfig::Rpmsg(_)), "{asked:?}");
    }

    /// A CAN node reaches its endpoint with its interface and identifier.
    ///
    /// Which is the whole of where it is: a CAN controller is a network
    /// interface rather than a device file, and the identifier its frames
    /// carry is what tells one node on a shared bus from another.
    #[test]
    fn a_can_node_is_an_interface_and_an_identifier() {
        let opened =
            endpoint("can", &[("interface", "vcan0"), ("can_id", "0x123")]).expect("it opens");
        match &opened {
            EndpointConfig::Can(can) => {
                assert_eq!(can.interface, "vcan0");
                assert_eq!(can.can_id, 0x123);
                assert!(!can.fd, "classic CAN unless the file says otherwise");
                assert!(
                    can.loopback,
                    "and looped back, which is what the kernel does"
                );
            }
            other => panic!("expected a CAN endpoint, got {other:?}"),
        }
        assert_eq!(opened.kind(), DHType::Can);
    }

    /// A CAN node needs both halves of where it is.
    #[test]
    fn a_can_node_without_an_interface_or_an_id_is_refused() {
        for (missing, stated) in [
            ("interface", ("can_id", "1")),
            ("can_id", ("interface", "vcan0")),
        ] {
            let said = endpoint("can", &[stated]).expect_err("half of where it is");
            assert!(
                said.contains(missing),
                "the error should name what is missing: {said}"
            );
        }
    }

    // -- where a read of a line ends ------------------------------------------

    /// The stream rule a line with these attributes settles.
    fn line_stream(
        attributes: &[(&str, &str)],
    ) -> Result<crate::endpoint_config::StreamParams, String> {
        let line = line_with(attributes);
        let terms = settled("serial", &line)?;
        Ok(terms.stream().expect("a line has a stream rule").clone())
    }

    /// A line's reach carries where its reads end, so that the vehicle can
    /// end them there; nothing else's does.
    #[test]
    fn a_line_carries_its_stream_rule_and_nothing_else_does() {
        let line = line_with(&[("max_length", "80"), ("terminators", "[0A]")]);
        let reach = reached("serial", &line).expect("a line");
        let rule = reach.stream.expect("a line's reads end somewhere");
        assert_eq!(rule.max_length, 80);
        assert_eq!(rule.terminators, vec![vec![0x0A]]);

        let bus = reached("i2c", &[("path", "/dev/i2c-1"), ("address", "0x48")]).expect("a bus");
        assert_eq!(bus.stream, None, "a bus transfer is already a frame");
    }

    /// A line whose reads could be longer than a datum is refused: a read is
    /// delivered whole, and one cut short is a line read in pieces nothing
    /// knows how to put back together.
    #[test]
    fn a_read_longer_than_a_datum_is_refused() {
        let most = crate::types::DATUM_BYTES.to_string();
        let one_more = (crate::types::DATUM_BYTES + 1).to_string();
        let fits = line_with(&[("max_length", &most), ("read_timeout", "none")]);
        reached("serial", &fits).expect("a datum's worth fits");
        let over = line_with(&[("max_length", &one_more), ("read_timeout", "none")]);
        let said = reached("serial", &over).expect_err("more than a datum");
        assert!(
            said.contains("max_length") && said.contains(&most),
            "{said}"
        );
    }

    #[test]
    fn a_stream_rule_must_say_how_long_a_read_may_be() {
        let said = line_stream(&[("read_timeout", "1 s")]).expect_err("max_length is required");
        assert!(said.contains("max_length"), "{said}");
    }

    #[test]
    fn a_timeout_alone_ends_a_read() {
        let stream =
            line_stream(&[("max_length", "64"), ("read_timeout", "250 ms")]).expect("accepted");
        assert_eq!(stream.timeout, Some(Duration::from_millis(250)));
        assert!(stream.terminators.is_empty());
        assert!(!stream.is_fixed_length());
    }

    #[test]
    fn terminators_alone_end_a_read() {
        let stream =
            line_stream(&[("max_length", "64"), ("terminators", "[0A]")]).expect("accepted");
        assert_eq!(stream.timeout, None);
        assert_eq!(stream.terminators, vec![vec![10]]);
        assert!(!stream.is_fixed_length());
    }

    #[test]
    fn a_timeout_and_terminators_may_both_end_a_read() {
        let stream = line_stream(&[
            ("max_length", "64"),
            ("read_timeout", "1 s"),
            ("terminators", "[04]"),
        ])
        .expect("accepted");
        assert_eq!(stream.timeout, Some(Duration::from_secs(1)));
        assert_eq!(stream.terminators, vec![vec![0x04]]);
    }

    #[test]
    fn a_stream_rule_that_ends_no_read_is_refused() {
        let said = line_stream(&[("max_length", "64")]).expect_err("nothing ends the read");
        assert!(
            said.contains("timeout") && said.contains("terminator"),
            "{said}"
        );
    }

    #[test]
    fn timeout_none_is_how_a_fixed_length_read_is_asked_for() {
        // The documented way to say "hand me max_length bytes at a time".
        let stream =
            line_stream(&[("max_length", "12"), ("read_timeout", "none")]).expect("accepted");
        assert_eq!(stream.max_length, 12);
        assert_eq!(stream.timeout, None);
        assert!(stream.terminators.is_empty());
        assert!(stream.is_fixed_length());
    }

    /// A length prefix ends a read, beside the timeout it must have: with
    /// one, a frame cut short ends and the next is looked for afresh.
    #[test]
    fn a_length_prefix_ends_a_read() {
        for (word, want) in [
            ("u8", LengthPrefix::U8),
            ("u16be", LengthPrefix::U16Be),
            ("U16LE", LengthPrefix::U16Le),
        ] {
            let stream = line_stream(&[
                ("max_length", "64"),
                ("length_prefix", word),
                ("read_timeout", "100 ms"),
            ])
            .unwrap_or_else(|e| panic!("{word}: {e}"));
            assert_eq!(stream.length_prefix, Some(want), "for {word}");
            assert!(!stream.is_fixed_length());
        }
        let none = line_stream(&[
            ("max_length", "64"),
            ("length_prefix", "u8"),
            ("read_timeout", "none"),
        ])
        .expect("none is a timeout stated");
        assert_eq!(none.timeout, None);
        assert!(!none.is_fixed_length(), "the prefix ends it");
    }

    /// A length prefix with no timeout stated is refused: one lost byte
    /// would leave every later read out of step, and saying none is how a
    /// file says it means that.
    #[test]
    fn a_length_prefix_must_say_its_timeout() {
        let said = line_stream(&[("max_length", "64"), ("length_prefix", "u8")])
            .expect_err("no timeout stated");
        assert!(
            said.contains("length_prefix") && said.contains("read_timeout"),
            "{said}"
        );
    }

    /// Terminators would end a read in the body of a frame whose length was
    /// given, so the two are not stated together.
    #[test]
    fn a_length_prefix_and_terminators_are_refused_together() {
        let said = line_stream(&[
            ("max_length", "64"),
            ("length_prefix", "u8"),
            ("read_timeout", "1 s"),
            ("terminators", "[0A]"),
        ])
        .expect_err("two rules for one end");
        assert!(
            said.contains("length_prefix") && said.contains("terminators"),
            "{said}"
        );
    }

    #[test]
    fn a_length_prefix_is_one_of_three_words() {
        let said = line_stream(&[
            ("max_length", "64"),
            ("length_prefix", "2"),
            ("read_timeout", "1 s"),
        ])
        .expect_err("not a prefix");
        assert!(said.contains("u8") && said.contains("u16be"), "{said}");
    }

    #[test]
    fn max_length_must_hold_a_length_prefix() {
        let said = line_stream(&[
            ("max_length", "1"),
            ("length_prefix", "u16le"),
            ("read_timeout", "1 s"),
        ])
        .expect_err("a frame cannot fit");
        assert!(
            said.contains("max_length") && said.contains("u16le"),
            "{said}"
        );
        line_stream(&[
            ("max_length", "2"),
            ("length_prefix", "u16le"),
            ("read_timeout", "1 s"),
        ])
        .expect("a prefix alone fits");
    }

    /// An attribute written with hyphens for its underscores is refused,
    /// and told its spelling: an attribute has the one.
    #[test]
    fn an_attribute_with_hyphens_is_told_its_spelling() {
        for (written, spelled) in [
            ("max-length", "max_length"),
            ("read-timeout", "read_timeout"),
            ("stop-bits", "stop_bits"),
            ("ip-protocol", "ip_protocol"),
        ] {
            let said = reached("serial", &line_with(&[(written, "1")])).expect_err("one spelling");
            assert!(
                said.contains(&format!("\"{written}\""))
                    && said.contains(&format!("it is spelled {spelled}")),
                "{written}: {said}"
            );
        }
    }

    /// A word that is not an attribute either way is refused as before, with
    /// no spelling to offer -- and so is one whose other spelling is retired.
    #[test]
    fn a_word_with_no_other_spelling_gets_no_hint() {
        let said = reached("serial", &line_with(&[("max-lenght", "1")])).expect_err("nothing");
        assert!(!said.contains("spelled"), "{said}");
        assert!(said.contains("looks like something that was set"), "{said}");

        let said = reached("serial", &line_with(&[("debounce-us", "1")])).expect_err("nothing");
        assert!(!said.contains("spelled"), "{said}");
    }

    #[test]
    fn an_empty_terminator_list_is_refused() {
        let said = line_stream(&[("max_length", "64"), ("terminators", "")])
            .expect_err("an empty list is not a list");
        assert!(said.contains("terminator"), "{said}");
    }

    #[test]
    fn a_timeouts_units_are_understood() {
        for (text, want) in [
            ("900 us", Duration::from_micros(900)),
            ("250 ms", Duration::from_millis(250)),
            ("3 s", Duration::from_secs(3)),
        ] {
            let stream = line_stream(&[("max_length", "8"), ("read_timeout", text)])
                .unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!(stream.timeout, Some(want), "for {text}");
        }
    }

    /// A number with no unit after it is refused, and so is one written
    /// against its unit.
    ///
    /// Two halves of the same rule: a length of time is a number, whitespace
    /// and a unit, so "250" is missing the unit and "250ms" is missing the
    /// whitespace. Each is told which, rather than both being told that the
    /// value is not a length of time.
    #[test]
    fn a_timeout_without_a_unit_is_refused() {
        let said =
            line_stream(&[("max_length", "8"), ("read_timeout", "250")]).expect_err("250 of what");
        assert!(said.contains("a number, a space, and a unit"), "{said}");

        let said = line_stream(&[("max_length", "8"), ("read_timeout", "250ms")])
            .expect_err("written together");
        assert!(said.contains("\"250 ms\""), "{said}");
    }

    #[test]
    fn a_zero_timeout_is_refused_in_favour_of_none() {
        let said = line_stream(&[("max_length", "8"), ("read_timeout", "0 ms")])
            .expect_err("a read that times out at once reads nothing");
        assert!(
            said.contains("no interval at all") && said.contains("none"),
            "{said}"
        );
    }

    /// A timeout takes every unit and every number an interval takes.
    ///
    /// One grammar for a length of time: see `graph::parse_interval`, which
    /// this reads through. What a timeout has of its own is the word `none`.
    #[test]
    fn a_timeout_takes_what_an_interval_takes() {
        for (text, want) in [
            ("1.5 s", Duration::from_millis(1_500)),
            ("5 min", Duration::from_secs(300)),
            ("1 h", Duration::from_secs(3_600)),
            ("2_000 ms", Duration::from_secs(2)),
        ] {
            let stream = line_stream(&[("max_length", "8"), ("read_timeout", text)])
                .unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!(stream.timeout, Some(want), "for {text}");
        }

        // And what neither takes: an ambiguous unit, by the name the one
        // grammar refuses it with.
        for text in ["5m", "1y", "6mon", "0x8ms"] {
            let said = line_stream(&[("max_length", "8"), ("read_timeout", text)])
                .expect_err("not a length of time");
            assert!(said.contains("timeout"), "{text}: {said}");
        }
    }

    /// A node's terminators are its alternatives, each a run of bytes in the
    /// notation a trigger is written in: held as one value with commas
    /// between them, which is how the reader of a file gathers its list.
    #[test]
    fn terminators_are_runs_of_bytes_any_of_which_ends_a_read() {
        let stream = line_stream(&[
            ("max_length", "8"),
            ("terminators", "\"\\r\\n\", [03], \"END\" [00]"),
        ])
        .expect("alternatives");
        assert_eq!(
            stream.terminators,
            vec![b"\r\n".to_vec(), vec![0x03], b"END\0".to_vec()]
        );
    }

    /// The spelling this replaced -- each terminator a byte, in decimal or
    /// 0x -- is refused with the brackets each one wants.
    #[test]
    fn a_terminator_written_bare_is_told_its_brackets() {
        let said = line_stream(&[("max_length", "8"), ("terminators", "13")]).expect_err("bare");
        assert!(
            said.contains("terminators") && said.contains("[0D]"),
            "{said}"
        );
    }

    /// A terminator longer than a read can hold would never end one.
    #[test]
    fn a_terminator_longer_than_max_length_is_refused() {
        let said =
            line_stream(&[("max_length", "2"), ("terminators", "\"END\"")]).expect_err("too long");
        assert!(
            said.contains("max_length 2") && said.contains("\"END\""),
            "{said}"
        );
    }

    #[test]
    fn a_terminator_wider_than_a_byte_is_refused() {
        let said = line_stream(&[("max_length", "8"), ("terminators", "[100]")])
            .expect_err("a line carries bytes");
        assert!(said.contains("more than one byte"), "{said}");
    }

    // -- the framing of a line ------------------------------------------------

    /// What the rules settle for a line with these terms, beside a path, a
    /// rate, a byte length and a fixed-length read.
    fn line_terms(extra: &[(&str, &str)]) -> Result<crate::endpoint_config::GroupKind, String> {
        let mut line: Vec<(&str, &str)> = vec![
            ("path", "/dev/ttyS0"),
            ("datarate", "9600"),
            ("byte_length", "8"),
            ("max_length", "8"),
            ("read_timeout", "none"),
        ];
        line.extend_from_slice(extra);
        settled("serial", &line)
    }

    /// A line says its framing, and the words of a clocked line are not its.
    ///
    /// This type is start-stop framing and nothing else. A line whose bits are
    /// carried on a clock is an `hdlc` node -- a network interface, with the
    /// clock and the coding set on it -- so the words that belong to one are
    /// refused here.
    #[test]
    fn a_line_is_start_stop_and_says_so() {
        let serial = |extra: &[(&str, &str)]| match line_terms(extra) {
            Ok(crate::endpoint_config::GroupKind::Serial(serial)) => serial,
            Ok(other) => panic!("{other:?}"),
            Err(e) => panic!("{extra:?}: {e}"),
        };

        // Stop bits are required: they are what start-stop framing uses in
        // place of a clock, so a line without them is not one of these.
        let start_stop = serial(&[("stop_bits", "2")]);
        assert_eq!(start_stop.stop_bits, crate::StopBits::Two);
        let said = line_terms(&[]).expect_err("stop bits are required");
        assert!(said.contains("stop_bits"), "{said}");

        // Parity defaults to none, which is what nearly every device link
        // uses.
        assert_eq!(start_stop.parity, crate::Parity::None);
        assert_eq!(
            serial(&[("stop_bits", "1"), ("parity", "even")]).parity,
            crate::Parity::Even
        );

        // A clocked line's words belong to the hdlc type and are refused
        // here, each by name.
        for stated in [
            ("clock_type", "external"),
            ("encoding", "nrzi"),
            ("clock_rate", "64000"),
            ("frame_check", "crc16_pr1"),
            ("standard", "v35"),
            ("loopback", "true"),
        ] {
            let said = line_terms(&[("stop_bits", "1"), stated])
                .expect_err("a clocked line's word on a start-stop line");
            assert!(said.contains(stated.0), "{stated:?}: {said}");
        }

        // And a parity that is a frame check is refused with the five a
        // character can have: a CRC is not one of them.
        let said = line_terms(&[("stop_bits", "1"), ("parity", "crc16_pr1")])
            .expect_err("a character's parity is not a frame check");
        assert!(said.contains("none, even, odd, mark, or space"), "{said}");
    }

    #[test]
    fn stop_bits_are_one_one_and_a_half_or_two() {
        for (text, want) in [
            ("1", crate::StopBits::One),
            ("1.5", crate::StopBits::OnePointFive),
            ("2", crate::StopBits::Two),
        ] {
            match line_terms(&[("stop_bits", text)]).unwrap_or_else(|e| panic!("{text}: {e}")) {
                crate::endpoint_config::GroupKind::Serial(serial) => {
                    assert_eq!(serial.stop_bits, want, "for {text}")
                }
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn a_byte_length_no_uart_offers_is_refused() {
        let said = settled(
            "serial",
            &[
                ("path", "/dev/ttyS0"),
                ("datarate", "9600"),
                ("stop_bits", "1"),
                ("byte_length", "9"),
                ("max_length", "8"),
                ("read_timeout", "none"),
            ],
        )
        .expect_err("a UART offers five to eight bits");
        assert!(said.contains("out of range"), "{said}");
    }

    // -- a bus and what sits on it --------------------------------------------

    /// One device on a bus, with these terms beside its bus and address.
    fn bus(extra: &[(&str, &str)]) -> Result<crate::node::payload::i2c::I2cParams, String> {
        let mut device: Vec<(&str, &str)> = vec![("path", "/dev/i2c-1")];
        device.extend_from_slice(extra);
        match settled("i2c", &device)? {
            crate::endpoint_config::GroupKind::I2c(i2c) => Ok(i2c),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_bus_reads_its_terms() {
        let i2c = bus(&[
            ("ten_bit", "false"),
            ("pec", "true"),
            ("retries", "3"),
            ("timeout", "50 ms"),
            ("address", "0x48"),
        ])
        .expect("every term of a bus");
        assert!(!i2c.ten_bit);
        assert!(i2c.pec);
        assert_eq!(i2c.retries, 3);
        assert_eq!(i2c.timeout, Some(Duration::from_millis(50)));
    }

    #[test]
    fn every_term_of_a_bus_has_a_default() {
        // Nothing but the type and where it is is required: the protocol
        // fixes the framing, and every knob here has a quiet setting.
        let i2c = bus(&[("address", "16")]).expect("a bus states little");
        assert!(!i2c.ten_bit);
        assert!(!i2c.pec);
        assert_eq!(i2c.retries, 0);
        assert_eq!(i2c.timeout, None);
    }

    #[test]
    fn a_bus_timeout_is_rounded_up_to_the_drivers_resolution() {
        let i2c = bus(&[("timeout", "25 ms"), ("address", "8")]).expect("a timeout");
        assert_eq!(i2c.timeout, Some(Duration::from_millis(25)));
        assert_eq!(i2c.effective_timeout(), Some(Duration::from_millis(30)));
    }

    #[test]
    fn a_device_on_a_bus_carries_the_bus_and_its_place_on_it() {
        let place =
            endpoint("i2c", &[("path", "/dev/i2c-2"), ("address", "0x49")]).expect("a place");
        assert_eq!(
            place,
            EndpointConfig::I2c(crate::types::I2cConfig {
                bus: "/dev/i2c-2".to_string(),
                address: 0x49,
                ten_bit: false,
                pec: false,
            })
        );
    }

    #[test]
    fn an_address_too_wide_for_the_addressing_is_refused() {
        let said = endpoint("i2c", &[("path", "/dev/i2c-2"), ("address", "0x90")])
            .expect_err("0x90 needs eight bits");
        assert!(said.contains("does not fit in 7 bits"), "{said}");

        // ...and the same address is fine once the node says so.
        match endpoint(
            "i2c",
            &[
                ("path", "/dev/i2c-2"),
                ("address", "0x90"),
                ("ten_bit", "true"),
            ],
        )
        .expect("ten bits hold it")
        {
            EndpointConfig::I2c(i2c) => {
                assert_eq!(i2c.address, 0x90);
                assert!(i2c.ten_bit);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_address_the_specification_reserves_is_refused() {
        // In range, but the specification keeps these, so no device can
        // answer to one.
        for address in ["0x00", "0x01", "0x07", "0x78", "0x7B", "0x7F"] {
            let said = endpoint("i2c", &[("path", "/dev/i2c-1"), ("address", address)])
                .expect_err(address);
            assert!(
                said.contains("reserved by the I2C specification"),
                "for {address}: {said}"
            );
        }
    }

    #[test]
    fn the_addresses_either_side_of_the_reserved_blocks_are_accepted() {
        for (address, want) in [("0x08", 0x08), ("0x77", 0x77)] {
            match endpoint("i2c", &[("path", "/dev/i2c-1"), ("address", address)])
                .unwrap_or_else(|e| panic!("{address} should be usable: {e}"))
            {
                EndpointConfig::I2c(i2c) => assert_eq!(i2c.address, want, "for {address}"),
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn ten_bit_addressing_has_no_reserved_block() {
        // A 10-bit transfer carries its address after the 0x78 prefix, so the
        // whole space is available and the 7-bit reservations do not apply.
        for address in ["0x00", "0x78", "0x3FF"] {
            endpoint(
                "i2c",
                &[
                    ("path", "/dev/i2c-1"),
                    ("ten_bit", "true"),
                    ("address", address),
                ],
            )
            .unwrap_or_else(|e| panic!("{address} should be usable with ten_bit: {e}"));
        }
    }

    #[test]
    fn a_device_on_a_bus_says_where_on_it_it_is() {
        let said = endpoint("i2c", &[("path", "/dev/i2c-1")]).expect_err("which device on the bus");
        assert!(said.contains("address"), "{said}");

        // And which bus, the two together being what tells one device on a
        // bus from another.
        let said = endpoint("i2c", &[("address", "0x48")]).expect_err("which bus");
        assert!(said.contains("path"), "{said}");
    }

    // -- a peripheral ---------------------------------------------------------

    /// One peripheral with these terms beside its device file.
    fn peripheral(extra: &[(&str, &str)]) -> Result<crate::node::payload::spi::SpiParams, String> {
        let mut device: Vec<(&str, &str)> = vec![("path", "/dev/spidev0.0")];
        device.extend_from_slice(extra);
        match settled("spi", &device)? {
            crate::endpoint_config::GroupKind::Spi(spi) => Ok(spi),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_peripheral_reads_its_terms() {
        let spi = peripheral(&[
            ("max_speed", "10000000"),
            ("spi_mode", "3"),
            ("bits_per_word", "16"),
            ("bit_order", "lsb"),
            ("cs_active", "high"),
        ])
        .expect("every term of a peripheral");
        assert_eq!(spi.max_speed, 10_000_000);
        assert_eq!(spi.mode, SpiMode::Mode3);
        assert_eq!(spi.bits_per_word.bits(), 16);
        assert_eq!(spi.bit_order, crate::BitOrder::LsbFirst);
        assert_eq!(spi.cs_active, crate::CsActive::High);
    }

    #[test]
    fn a_peripherals_defaults_are_eight_bit_msb_first_and_active_low() {
        let spi = peripheral(&[("max_speed", "1000000"), ("spi_mode", "0")]).expect("the usual");
        assert_eq!(spi.bits_per_word.bits(), 8);
        assert_eq!(spi.bit_order, crate::BitOrder::MsbFirst);
        assert_eq!(spi.cs_active, crate::CsActive::Low);
    }

    #[test]
    fn a_peripheral_says_which_mode_it_is_clocked_in() {
        let said = peripheral(&[("max_speed", "1000000")]).expect_err("a mode is required");
        assert!(said.contains("mode"), "{said}");

        let said = peripheral(&[("max_speed", "1000000"), ("spi_mode", "4")])
            .expect_err("there are four of them");
        assert!(said.contains("not one of 0, 1, 2, or 3"), "{said}");
    }

    #[test]
    fn a_bus_is_given_no_stream_rule() {
        // A master clocks exactly as many bytes as it asks for, so there is
        // no rule to give for where a read ends.
        for (kind, rest) in [
            ("i2c", vec![("path", "/dev/i2c-1"), ("address", "0x40")]),
            (
                "spi",
                vec![
                    ("path", "/dev/spidev0.0"),
                    ("max_speed", "1"),
                    ("spi_mode", "0"),
                ],
            ),
        ] {
            let mut stated = rest.clone();
            stated.extend_from_slice(&[("max_length", "8"), ("read_timeout", "none")]);
            let said = endpoint(kind, &stated).expect_err(kind);
            assert!(said.contains("does not apply"), "{kind}: {said}");
        }
    }

    #[test]
    fn a_term_of_another_kind_of_link_is_refused_by_that_kind() {
        // The rule that a misplaced term is reported where it was written,
        // checked across the cross-product rather than one way.
        let i2c = vec![("path", "/dev/i2c-1"), ("address", "0x40")];
        let spi = vec![
            ("path", "/dev/spidev0.0"),
            ("max_speed", "1"),
            ("spi_mode", "0"),
        ];
        let serial = vec![
            ("path", "/dev/ttyS0"),
            ("datarate", "9600"),
            ("stop_bits", "1"),
            ("byte_length", "8"),
            ("max_length", "8"),
            ("read_timeout", "none"),
        ];
        for (kind, rest, foreign) in [
            ("i2c", &i2c, ("datarate", "9600")),
            ("i2c", &i2c, ("spi_mode", "0")),
            ("spi", &spi, ("pec", "true")),
            ("serial", &serial, ("cs_active", "low")),
        ] {
            let mut stated = rest.clone();
            stated.push(foreign);
            let said = endpoint(kind, &stated).expect_err(foreign.0);
            assert!(
                said.contains(foreign.0) && said.contains("does not apply"),
                "{foreign:?} on {kind}: {said}"
            );
        }
    }
}
