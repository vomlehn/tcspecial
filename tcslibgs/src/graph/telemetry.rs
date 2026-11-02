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
//! Every node has one: its position among the configuration's nodes sorted
//! by name, counting from nought, across every file the configuration is
//! made of. The order the files write their nodes in, and the order the top
//! file includes the others in, mean nothing and change no id. So adding or
//! removing a node moves the ids of the nodes whose names sort after it, and
//! a ground station working from the same file counts the same way. See
//! [`ids`].
//!
//! # Two kinds of link
//!
//! * A link drawn into a gateway node. What the source node produces is sent to
//!   the gateway node's `ground_base` host, at its port plus the source's id.
//!   The vehicle's end is whatever port the system gives it: what the ground
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
//! # On a software bus
//!
//! A configuration that answers to cFS has message IDs where these have
//! ports, counted the same way: a gateway node's `tlm_base`, or the
//! section's `payload_telemetry.tlm_base`, plus the source's id. [`msgids`]
//! is every message ID such a configuration uses, telemetry and commands
//! alike, and holds each to the mission's allotment.
//!
//! # Under Yamcs
//!
//! A configuration that answers to Yamcs has ports as the radio's does, and
//! every packet on them is a CCSDS Space Packet, its APID counted as its port
//! is: what goes through a `gateway_yamcs` node carries the node's
//! `apid_base` plus the source's device id, and the telemetry of a payload
//! nothing touches the section's `payload_telemetry.apid_base` plus the
//! payload's device id. [`apids`] is every APID such a configuration uses,
//! and holds each to being one, at most 0x7FF, and used once.
//!
//! This is the one place that says all of it. The flight software routes by
//! it, the checking judges a configuration by it, and a ground station or
//! simulator working from the same file finds the same ports by it.

use std::collections::{BTreeMap, BTreeSet};

use super::parameters::{ci_config_of, flatten};
use super::{Destination, Graph};
use crate::types::{
    host_port_of, msgid_of, Links, MsgIdRange, NetworkConfig, NetworkProtocol,
};

/// Which kind of link to the ground this is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Via {
    /// Drawn into this gateway node, and sent to its `ground_base`.
    Gateway(String),
    /// A payload nothing touches, over the section's `payload_telemetry`.
    PayloadTelemetry,
}

/// How a link to the ground is reached: over UDP, or on a software bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reaches {
    /// Over UDP, from one socket to another.
    Socket {
        /// tcspecial's end, which it binds and sends from. Port nought for a
        /// link into a gateway node, whose vehicle end is whatever the
        /// system gives.
        vehicle: NetworkConfig,
        /// The ground's end, which the ground binds and tcspecial sends to.
        ground: NetworkConfig,
    },
    /// Published on a cFS software bus, under this message ID.
    Bus(u16),
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
    /// How it is reached.
    pub reaches: Reaches,
    /// The APID its packets carry as CCSDS telemetry, where they are CCSDS
    /// Space Packets -- a link through a `gateway_yamcs` node, or over
    /// `payload_telemetry` in a configuration that answers to Yamcs -- or
    /// `None` for a link whose packets carry no header.
    pub apid: Option<u16>,
}

impl Telemetered {
    /// tcspecial's end of a link over UDP.
    ///
    /// # Returns
    ///
    /// The socket, or `None` for a link on a software bus.
    pub fn vehicle(&self) -> Option<&NetworkConfig> {
        match &self.reaches {
            Reaches::Socket { vehicle, .. } => Some(vehicle),
            Reaches::Bus(_) => None,
        }
    }

    /// The ground's end of a link over UDP.
    ///
    /// # Returns
    ///
    /// The socket, or `None` for a link on a software bus.
    pub fn ground(&self) -> Option<&NetworkConfig> {
        match &self.reaches {
            Reaches::Socket { ground, .. } => Some(ground),
            Reaches::Bus(_) => None,
        }
    }

    /// The message ID a link on a software bus is published under.
    ///
    /// # Returns
    ///
    /// The message ID, or `None` for a link over UDP.
    pub fn msgid(&self) -> Option<u16> {
        match self.reaches {
            Reaches::Bus(msgid) => Some(msgid),
            Reaches::Socket { .. } => None,
        }
    }
}

/// What is wrong with a configuration's links to the ground.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// The node it is about, or none for the `tcspecial` section.
    pub node: Option<String>,
    pub why: String,
}

/// Gives every node's device id, which is its place among the
/// configuration's nodes sorted by name.
///
/// By name, because nothing else about a node is in an order: the order a
/// file writes its types, its nodes and the files it includes means nothing,
/// and two files that differ only in it are one configuration with one set of
/// ids. A node's name is unique across the configuration, so the order is a
/// whole one, and a node added or renamed moves only the ids of the nodes
/// whose names sort after it.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// Each node's device id, by name.
pub fn ids(graph: &Graph) -> BTreeMap<String, usize> {
    let names: std::collections::BTreeSet<&str> =
        graph.nodes.iter().map(|node| node.name.as_str()).collect();
    names
        .into_iter()
        .enumerate()
        .map(|(id, name)| (name.to_string(), id))
        .collect()
}

/// A message ID base, counted on by a node's device id.
///
/// The one place the sum is taken for message IDs, as [`HostPort::at`] is
/// for ports, so that both ends of a software bus work out the same ID from
/// the same file.
///
/// # Arguments
///
/// * `base` - The base.
/// * `id`   - The node's device id.
///
/// # Returns
///
/// The message ID.
///
/// # Errors
///
/// A message where the sum is more than a message ID's sixteen bits.
///
/// [`HostPort::at`]: crate::types::HostPort::at
pub fn counted(base: u16, id: usize) -> Result<u16, String> {
    usize::from(base)
        .checked_add(id)
        .and_then(|sum| u16::try_from(sum).ok())
        .ok_or_else(|| {
            format!(
                "{base:#06x} plus {id} is more than a message ID's sixteen \
                 bits: a base this high leaves no room for the nodes counted \
                 from it"
            )
        })
}

/// An APID base, counted on by a node's device id: [`counted`] for APIDs.
///
/// # Arguments
///
/// * `base` - The base.
/// * `id`   - The node's device id.
///
/// # Returns
///
/// The APID.
///
/// # Errors
///
/// A message where the sum is past an APID's eleven bits.
pub fn counted_apid(base: u16, id: usize) -> Result<u16, String> {
    usize::from(base)
        .checked_add(id)
        .and_then(|sum| u16::try_from(sum).ok())
        .filter(|sum| *sum <= crate::ccsds::MAX_APID)
        .ok_or_else(|| {
            format!(
                "{base:#x} plus the device id {id} is past an APID's eleven \
                 bits, {:#x}: lower the base, or give the node a name that \
                 sorts earlier",
                crate::ccsds::MAX_APID
            )
        })
}

/// What a source's link into a gateway node is called.
pub fn gateway_link_name(source: &str, gateway: &str) -> String {
    format!("{source} to {gateway}")
}

/// What an untouched payload's link over `payload_telemetry` is called.
pub fn payload_link_name(payload: &str) -> String {
    format!("{payload} telemetry")
}

/// Finds every link to the ground this configuration has.
///
/// A gateway node with no `ground_base`, or one that does not read, contributes
/// nothing here: [`crate::graph::reach::reach_of`] says what is wrong with it,
/// and says it once.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// The links: those drawn into a gateway node, by source and then by
/// gateway, then one for each payload nothing touches.
///
/// # Errors
///
/// [`Refused`] when a link's port or message ID cannot be counted on to from
/// its base, the `tcspecial` section does not read, or two links land on one
/// port of one host or on one message ID.
pub fn of(graph: &Graph) -> Result<Vec<Telemetered>, Refused> {
    let ids = ids(graph);
    let mut links = Vec::new();

    // Links drawn into a gateway node, in the order the configuration holds
    // them: by the node they leave, and then by where they go.
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    for link in &graph.links {
        let source = link.origin().node;
        for destination in link.to.each() {
            let to = Destination::parse(destination).node;
            let Some(gateway) = graph
                .node(&to)
                .filter(|n| crate::gateway::is_gateway(&n.node_type))
            else {
                continue;
            };
            // Each kind names its far end by what is there: a radio's
            // ground_base, an F´ gateway's bridge_base, cFS's tlm_base. One
            // rule counts on from each.
            let kind =
                crate::gateway::GatewayKind::of_node_type(&gateway.node_type)
                    .expect("a gateway node");
            let attribute = kind.base_attribute();
            let Some(said) = gateway.attributes.get(attribute) else {
                continue;
            };
            if !seen.insert((source.clone(), to.clone())) {
                continue;
            }
            let Some(&id) = ids.get(&source) else {
                continue;
            };
            let refused = |why: String| Refused {
                node: Some(to.clone()),
                why: format!("\"{to}\": {attribute} {why}"),
            };
            // Under Yamcs, the node's apid_base counted on by the source's
            // id, as the port is. A node whose apid_base does not read
            // contributes nothing, as one whose base does not: reach_of
            // says what is wrong with it.
            let apid = match crate::graph::reach::apid_base_of(gateway) {
                Err(_) => continue,
                Ok(None) => None,
                Ok(Some(base)) => {
                    Some(counted_apid(base, id).map_err(|why| Refused {
                        node: Some(to.clone()),
                        why: format!("\"{to}\": apid_base {why}"),
                    })?)
                }
            };
            let reaches = if kind.on_bus() {
                let Ok(base) = msgid_of(attribute, said.as_str()) else {
                    continue;
                };
                Reaches::Bus(counted(base, id).map_err(refused)?)
            } else {
                let Ok(base) = host_port_of(attribute, said.as_str()) else {
                    continue;
                };
                Reaches::Socket {
                    vehicle: NetworkConfig {
                        protocol: NetworkProtocol::Udp,
                        address: "0.0.0.0".to_string(),
                        port: 0,
                    },
                    ground: base.at(id).map_err(refused)?,
                }
            };
            links.push(Telemetered {
                name: gateway_link_name(&source, &to),
                source: source.clone(),
                id,
                via: Via::Gateway(to.clone()),
                reaches,
                apid,
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
            // Under Yamcs, payload_telemetry.apid_base counted on by the
            // payload's id, as its port is.
            let apid = match ci.sockets().and_then(|s| s.framing) {
                None => None,
                Some(framing) => {
                    Some(counted_apid(framing.payload_apid_base, id).map_err(
                        |why| Refused {
                            node: Some(node.name.clone()),
                            why: format!(
                                "\"{}\"'s telemetry: \
                                 payload_telemetry.apid_base {why}",
                                node.name
                            ),
                        },
                    )?)
                }
            };
            let reaches = match &ci.links {
                Links::Sockets(sockets) => Reaches::Socket {
                    vehicle: sockets
                        .payload_source_base
                        .at(id)
                        .map_err(refused)?,
                    ground: sockets
                        .payload_dest_base
                        .at(id)
                        .map_err(refused)?,
                },
                Links::Bus(bus) => Reaches::Bus(
                    counted(bus.tlm_base, id)
                        .map_err(|why| {
                            format!("payload_telemetry.tlm_base {why}")
                        })
                        .map_err(refused)?,
                ),
            };
            links.push(Telemetered {
                name: payload_link_name(&node.name),
                source: node.name.clone(),
                id,
                via: Via::PayloadTelemetry,
                reaches,
                apid,
            });
        }
    }

    // No two sources on one port of one host, nor on one message ID.
    let mut landed: BTreeMap<(String, u16), &Telemetered> = BTreeMap::new();
    for link in &links {
        let (at, said) = match &link.reaches {
            Reaches::Socket { ground, .. } => (
                (ground.address.clone(), ground.port),
                format!(
                    "send to the ground at {}:{}",
                    ground.address, ground.port
                ),
            ),
            Reaches::Bus(msgid) => (
                (String::new(), *msgid),
                format!("be published as message ID {msgid:#06x}"),
            ),
        };
        if let Some(first) = landed.insert(at, link) {
            return Err(Refused {
                node: Some(link.source.clone()),
                why: format!(
                    "\"{}\" and \"{}\" would both {said}, and their data \
                     would arrive interleaved with nothing to tell it apart: \
                     move a base so they do not overlap",
                    first.source, link.source,
                ),
            });
        }
    }

    Ok(links)
}

/// One message ID a configuration that answers over a software bus uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsgIdUse {
    /// The message ID.
    pub msgid: u16,
    /// Whether it is one tcspecial takes commands on (`true`), or one it
    /// publishes telemetry on.
    pub command: bool,
    /// What it is for, as a complaint names it.
    pub what: String,
    /// Which node it is about, where it is a node's.
    pub node: Option<String>,
    /// What it carries.
    pub carries: Carries,
}

/// What a message ID carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Carries {
    /// Commands about the spacecraft.
    SpacecraftCommands,
    /// Their answers.
    SpacecraftAnswers,
    /// Commands about the node of this device id.
    NodeCommands(usize),
    /// Their answers.
    NodeAnswers(usize),
    /// The beacon.
    Beacon,
    /// What a gateway node sends on along its lines and command bindings:
    /// the gateway node's own `cmd_msgid`.
    Gateway(String),
    /// A source's data: one of [`of`]'s links.
    Telemetry(String),
}

/// Every message ID a configuration that answers over a software bus uses,
/// each checked: inside `msgid_range`, a command's where tcspecial takes it
/// and telemetry's where it publishes, and used once.
///
/// The one statement of what tcspecial subscribes to and publishes, which
/// the vehicle opens its subscriptions from and the ground check judges the
/// allotment by: the spacecraft's command ID and its answers', each node's
/// command and answer IDs counted from `payload_command`'s bases, each
/// `gateway_cfs` node's `cmd_msgid`, the beacon's, and every link [`of`]
/// finds.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// Every message ID used, or nothing for a configuration whose links are
/// sockets or that has no section.
///
/// # Errors
///
/// [`Refused`] naming what the message ID is for, the ID and the range, or
/// the two uses an ID is given.
pub fn msgids(graph: &Graph) -> Result<Vec<MsgIdUse>, Refused> {
    let Some(section) = graph.tcspecial.as_ref() else {
        return Ok(Vec::new());
    };
    let refused = |why: String| Refused { node: None, why };
    let ci = ci_config_of(&flatten(section))
        .map_err(refused)?
        .to_ci_config()
        .map_err(refused)?;
    let Some(bus) = ci.bus() else {
        return Ok(Vec::new());
    };
    let range = MsgIdRange {
        first: bus.first,
        last: bus.last,
    };

    let mut used = vec![
        MsgIdUse {
            msgid: bus.cmd_msgid,
            command: true,
            what: "tcs_cmd.cmd_msgid".to_string(),
            node: None,
            carries: Carries::SpacecraftCommands,
        },
        MsgIdUse {
            msgid: bus.response_msgid,
            command: false,
            what: "tcs_cmd.response_msgid".to_string(),
            node: None,
            carries: Carries::SpacecraftAnswers,
        },
        MsgIdUse {
            msgid: bus.beacon_msgid,
            command: false,
            what: "beacon.tlm_msgid".to_string(),
            node: None,
            carries: Carries::Beacon,
        },
    ];
    let ids = ids(graph);
    for node in &graph.nodes {
        let id = ids[&node.name];
        let counted_for = |name: &str, base: u16| {
            counted(base, id).map_err(|why| Refused {
                node: Some(node.name.clone()),
                why: format!(
                    "\"{}\"'s message ID, {name} plus its device id {id}: {why}",
                    node.name
                ),
            })
        };
        used.push(MsgIdUse {
            msgid: counted_for("payload_command.cmd_base", bus.cmd_base)?,
            command: true,
            what: format!(
                "the command message ID of \"{}\", payload_command.cmd_base \
                 plus its device id {id}",
                node.name
            ),
            node: Some(node.name.clone()),
            carries: Carries::NodeCommands(id),
        });
        used.push(MsgIdUse {
            msgid: counted_for(
                "payload_command.response_base",
                bus.response_base,
            )?,
            command: false,
            what: format!(
                "the answer message ID of \"{}\", \
                 payload_command.response_base plus its device id {id}",
                node.name
            ),
            node: Some(node.name.clone()),
            carries: Carries::NodeAnswers(id),
        });
        if crate::gateway::GatewayKind::of_node_type(&node.node_type)
            .is_some_and(|kind| kind.on_bus())
        {
            if let Some(said) = node.attributes.get("cmd_msgid") {
                let msgid =
                    msgid_of("cmd_msgid", said.as_str()).map_err(|why| {
                        Refused {
                            node: Some(node.name.clone()),
                            why: format!("\"{}\": {why}", node.name),
                        }
                    })?;
                used.push(MsgIdUse {
                    msgid,
                    command: true,
                    what: format!("the cmd_msgid of \"{}\"", node.name),
                    node: Some(node.name.clone()),
                    carries: Carries::Gateway(node.name.clone()),
                });
            }
        }
    }
    for link in of(graph)? {
        if let Some(msgid) = link.msgid() {
            let what = match &link.via {
                Via::Gateway(gateway) => format!(
                    "the message ID of what \"{}\" sends to \"{gateway}\", \
                     its tlm_base plus the device id {}",
                    link.source, link.id
                ),
                Via::PayloadTelemetry => format!(
                    "the message ID of \"{}\"'s telemetry, \
                     payload_telemetry.tlm_base plus its device id {}",
                    link.source, link.id
                ),
            };
            used.push(MsgIdUse {
                msgid,
                command: false,
                what,
                node: Some(link.source.clone()),
                carries: Carries::Telemetry(link.name.clone()),
            });
        }
    }

    let mut first_use: BTreeMap<u16, &MsgIdUse> = BTreeMap::new();
    for one in &used {
        range
            .holds(&one.what, one.msgid, one.command)
            .map_err(|why| Refused {
                node: one.node.clone(),
                why,
            })?;
        if let Some(before) = first_use.insert(one.msgid, one) {
            return Err(Refused {
                node: one.node.clone(),
                why: format!(
                    "{} and {} are both message ID {:#06x}: two kinds of \
                     packet under one ID would each be read as the other, so \
                     move a base or an ID",
                    before.what, one.what, one.msgid
                ),
            });
        }
    }
    Ok(used)
}

/// What an APID a configuration whose packets are CCSDS Space Packets uses
/// carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApidCarries {
    /// The beacon, as telemetry: `beacon.apid`.
    Beacon,
    /// The commands on both command links, as telecommands, and their
    /// answers, as telemetry: `tcs_cmd.apid`.
    Commands,
    /// What the ground sends this gateway node, as telecommands: its
    /// `apid_base`.
    Uplink(String),
    /// A source's data through this gateway node, as telemetry: one of
    /// [`of`]'s links.
    Downlink {
        /// The gateway node it goes through.
        gateway: String,
        /// The link's name.
        link: String,
    },
    /// The telemetry of a payload nothing touches, over the section's
    /// `payload_telemetry`: one of [`of`]'s links, by its name.
    PayloadTelemetry(String),
}

/// One APID a configuration whose packets are CCSDS Space Packets uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApidUse {
    /// The APID.
    pub apid: u16,
    /// What it is for, as a complaint names it.
    pub what: String,
    /// What to change to move it, as a complaint names it.
    pub change: String,
    /// Which node it is about, where it is a node's.
    pub node: Option<String>,
    /// What it carries.
    pub carries: ApidCarries,
}

impl ApidUse {
    /// Whether two uses may share an APID: only a gateway node's commands
    /// and what goes to the ground through that same node, which are told
    /// apart by their packet type.
    ///
    /// # Arguments
    ///
    /// * `other` - The other use.
    ///
    /// # Returns
    ///
    /// `true` where the two may be one APID.
    fn may_share(&self, other: &ApidUse) -> bool {
        match (&self.carries, &other.carries) {
            (
                ApidCarries::Uplink(one),
                ApidCarries::Downlink { gateway, .. },
            )
            | (
                ApidCarries::Downlink { gateway, .. },
                ApidCarries::Uplink(one),
            ) => one == gateway,
            _ => false,
        }
    }
}

/// Every APID a configuration whose packets are CCSDS Space Packets uses,
/// each checked: an APID, and used once.
///
/// The beacon's and the command links', each `gateway_yamcs` node's
/// `apid_base`, every link [`of`] finds through one, and every payload's
/// link over `payload_telemetry`. A Yamcs ground
/// station sorts what arrives into streams by APID, so two kinds of packet
/// under one would each be read as the other. The one exception is a
/// gateway node's own `apid_base` and the first APID counted from it, the
/// device id 0's: one carries telecommands and the other telemetry, and the
/// packet type tells them apart.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// Every APID used, or nothing for a configuration whose packets carry no
/// header or that has no section.
///
/// # Errors
///
/// [`Refused`] naming the two uses an APID is given, the APID and what to
/// change, or an APID counted from a base that is past eleven bits.
pub fn apids(graph: &Graph) -> Result<Vec<ApidUse>, Refused> {
    let Some(section) = graph.tcspecial.as_ref() else {
        return Ok(Vec::new());
    };
    let refused = |why: String| Refused { node: None, why };
    let ci = ci_config_of(&flatten(section))
        .map_err(refused)?
        .to_ci_config()
        .map_err(refused)?;
    let Some(framing) = ci.sockets().and_then(|sockets| sockets.framing) else {
        return Ok(Vec::new());
    };

    let mut used = vec![
        ApidUse {
            apid: framing.beacon_apid,
            what: "beacon.apid".to_string(),
            change: "beacon.apid".to_string(),
            node: None,
            carries: ApidCarries::Beacon,
        },
        ApidUse {
            apid: framing.cmd_apid,
            what: "tcs_cmd.apid".to_string(),
            change: "tcs_cmd.apid".to_string(),
            node: None,
            carries: ApidCarries::Commands,
        },
    ];
    for node in &graph.nodes {
        // One whose apid_base does not read is complained about by
        // reach_of, and once.
        let Ok(Some(apid)) = crate::graph::reach::apid_base_of(node) else {
            continue;
        };
        used.push(ApidUse {
            apid,
            what: format!("the apid_base of \"{}\"", node.name),
            change: format!("the apid_base of \"{}\"", node.name),
            node: Some(node.name.clone()),
            carries: ApidCarries::Uplink(node.name.clone()),
        });
    }
    for link in of(graph)? {
        let Some(apid) = link.apid else {
            continue;
        };
        used.push(match &link.via {
            Via::Gateway(gateway) => ApidUse {
                apid,
                what: format!(
                    "the APID of what \"{}\" sends to \"{gateway}\", its \
                     apid_base plus the device id {}",
                    link.source, link.id
                ),
                change: format!("the apid_base of \"{gateway}\""),
                node: Some(gateway.clone()),
                carries: ApidCarries::Downlink {
                    gateway: gateway.clone(),
                    link: link.name.clone(),
                },
            },
            Via::PayloadTelemetry => ApidUse {
                apid,
                what: format!(
                    "the APID of \"{}\"'s telemetry, \
                     payload_telemetry.apid_base plus its device id {}",
                    link.source, link.id
                ),
                change: "payload_telemetry.apid_base".to_string(),
                node: Some(link.source.clone()),
                carries: ApidCarries::PayloadTelemetry(link.name.clone()),
            },
        });
    }

    for (at, one) in used.iter().enumerate() {
        let Some(before) = used[..at]
            .iter()
            .find(|before| before.apid == one.apid && !before.may_share(one))
        else {
            continue;
        };
        let change = match before.change == one.change {
            true => one.change.clone(),
            false => format!("{} or {}", before.change, one.change),
        };
        return Err(Refused {
            node: one.node.clone(),
            why: format!(
                "{} and {} are both APID {:#x}: Yamcs sorts packets by APID, \
                 so each would be read as the other -- change {change} so \
                 they do not overlap",
                before.what, one.what, one.apid
            ),
        });
    }
    Ok(used)
}

/// Finds every node the drawing touches: an end of a link, or of a command
/// binding.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// The nodes' names.
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
        Graph::load(root.join(format!("examples/{name}/{name}.ytcs")))
            .expect("the example reads")
    }

    /// Names that differ only in case are two names, and ids follow the
    /// names' code points, as tests/ids/case.ids says and the Python's test
    /// of the same file holds it to.
    #[test]
    fn ids_tell_case_apart_and_sort_by_code_point() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/ids");
        let graph = Graph::load(dir.join("case.ytcs")).expect("the file reads");
        let table = std::fs::read_to_string(dir.join("case.ids"))
            .expect("the table reads");
        let expected: BTreeMap<String, usize> = table
            .lines()
            .filter(|line| !line.starts_with('#'))
            .map(|line| {
                let (name, id) =
                    line.split_once(' ').expect("a name and an id");
                (name.to_string(), id.parse().expect("an id"))
            })
            .collect();
        assert_eq!(expected.len(), 8);
        assert_eq!(ids(&graph), expected);
    }

    /// Sums up links to the ground for comparison.
    ///
    /// # Arguments
    ///
    /// * `links` - The links.
    ///
    /// # Returns
    ///
    /// Each link's source, device id, and ground address and port.
    fn summary(links: &[Telemetered]) -> Vec<(&str, usize, &str, u16)> {
        links
            .iter()
            .map(|l| {
                let ground = l.ground().expect("a link over UDP");
                (
                    l.source.as_str(),
                    l.id,
                    ground.address.as_str(),
                    ground.port,
                )
            })
            .collect()
    }

    /// Every node is counted, whatever its kind, in the order of the
    /// names.
    #[test]
    fn every_node_has_a_device_id() {
        let ids = ids(&example("thermostat"));
        assert_eq!(ids["ambient"], 0);
        assert_eq!(ids["gateway"], 1);
        assert_eq!(ids["heaters_1"], 2);
        assert_eq!(ids["heaters_2"], 3);
        assert_eq!(ids["temp1"], 5);
        assert_eq!(ids["temp_1"], 8);
        assert_eq!(ids["thermostat_1"], 11);
        assert_eq!(ids["thermostat_2"], 12);
    }

    /// The order the nodes are in means nothing: shuffled, they have the
    /// ids they had.
    #[test]
    fn the_order_of_the_nodes_changes_no_id() {
        let graph = example("thermostat");
        let mut shuffled = graph.clone();
        shuffled.nodes.reverse();
        shuffled.nodes.swap(0, 5);
        assert_eq!(ids(&shuffled), ids(&graph));
        assert_eq!(of(&shuffled), of(&graph));
    }

    /// What is drawn into a gateway node goes to its ground_base plus the
    /// source's id, one port per source.
    #[test]
    fn a_link_into_a_gateway_node_goes_to_its_ground_base_plus_the_id() {
        let links = of(&example("thermostat")).expect("it reads");
        assert_eq!(
            summary(&links),
            vec![
                ("ambient", 0, "127.0.0.1", 5100),
                ("temp1", 5, "127.0.0.1", 5105),
                ("temp2", 6, "127.0.0.1", 5106),
                ("temp3", 7, "127.0.0.1", 5107),
                // The decisions themselves, which the ground sees as well as
                // the heaters: a reader of the telemetry can then say why the
                // heaters are on without inferring it from the temperature.
                ("thermostat_1", 11, "127.0.0.1", 5111),
                ("thermostat_2", 12, "127.0.0.1", 5112),
            ]
        );
        assert!(links
            .iter()
            .all(|l| l.via == Via::Gateway("gateway".to_string())));

        let links = of(&example("simple-app")).expect("it reads");
        assert_eq!(summary(&links), vec![("lorem", 1, "127.0.0.1", 5101)]);
    }

    /// A payload nothing touches reports over the section's payload_telemetry.
    #[test]
    fn an_untouched_payload_reports_over_payload_telemetry() {
        let links = of(&example("minimal")).expect("it reads");
        assert_eq!(summary(&links), vec![("sensor", 0, "127.0.0.1", 6100)]);
        assert_eq!(links[0].via, Via::PayloadTelemetry);
        let vehicle = links[0].vehicle().expect("a link over UDP");
        assert_eq!((vehicle.address.as_str(), vehicle.port), ("0.0.0.0", 6000));
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
        // temp_2, id 9, unlinked: 6100 + 9 = 6109; a gateway base of 6104
        // puts temp1, id 5, on the same port.
        graph.links.retain(|link| link.origin().node != "temp_2");
        let gateway = graph
            .nodes
            .iter_mut()
            .find(|n| n.name == "gateway")
            .expect("gateway");
        gateway.attributes.insert(
            "ground_base".to_string(),
            crate::Scalar::of("127.0.0.1:6104"),
        );
        let refused = of(&graph).expect_err("two sources on 6109");
        assert!(refused.why.contains("temp1"), "{}", refused.why);
        assert!(refused.why.contains("temp_2"), "{}", refused.why);
        assert!(refused.why.contains("6109"), "{}", refused.why);
    }

    /// A base with no room for the ids counted from it is refused.
    #[test]
    fn a_base_with_no_room_is_refused() {
        let mut graph = example("thermostat");
        let gateway = graph
            .nodes
            .iter_mut()
            .find(|n| n.name == "gateway")
            .expect("gateway");
        gateway.attributes.insert(
            "ground_base".to_string(),
            crate::Scalar::of("127.0.0.1:65535"),
        );
        let refused = of(&graph).expect_err("port 65536 is not a port");
        assert_eq!(refused.node.as_deref(), Some("gateway"));
        assert!(refused.why.contains("65535"), "{}", refused.why);
    }

    /// No section: the gateway links stand, and no payload reports on its own.
    #[test]
    fn with_no_section_only_the_gateway_links_remain() {
        let mut graph = example("minimal");
        graph.tcspecial = None;
        assert_eq!(of(&graph), Ok(Vec::new()));
    }
}
