//! Command definitions for TCSpecial
//!
//! Commands are sent from ground to space and are idempotent.

use serde::{Deserialize, Serialize};

use crate::config_digest::{ConfigDigest, ConfigVersion};
use crate::types::{ArmKey, BeaconInterval, DHName, DHType, NodeId};

/// Command message header
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandHeader {
    /// Command sequence number for tracking
    pub sequence: u32,
    /// Command type identifier
    pub cmd_type: CommandType,
}

/// Command types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CommandType {
    Ping,
    /// What each end reads, asked of the spacecraft as a link comes up.
    ///
    /// About the link to the vehicle and not about anything the vehicle
    /// carries, which is why it belongs on the spacecraft's own link: a
    /// ground station with no connection has nothing to ask a payload.
    Connect,
    RestartArm,
    Restart,
    StartDH,
    StopDH,
    QueryDH,
    QueryDHSample,
    Config,
    ConfigDH,
    /// Enable a node of a version 2 graph: version 1's START_DH, with a node
    /// in place of a data handler.
    ///
    /// A kind of its own rather than a START_DH carrying a different body.
    /// The two name their subject differently -- a data handler by name, a
    /// node by id -- and a vehicle running one version has no use for the
    /// other's, so a single kind would be one message meaning two things and
    /// each end guessing which from what else it knows.
    StartNode,
    /// And disable one: version 1's STOP_DH, the same way.
    StopNode,
    /// What a node has moved: version 1's QUERY_DH, about a node.
    QueryNode,
    /// The last datum a node moved each way: version 1's QUERY_DH_SAMPLE.
    QueryNodeSample,
    /// Set an attribute of a node: version 1's CONFIG_DH, with something for
    /// it to do.
    ///
    /// Version 1's carried a name and nothing to configure with, and was
    /// refused as unimplemented. A graph's `commands` section already says
    /// which attribute of which node the ground may set, so this is the
    /// command that section was written for.
    SetNode,
}

/// Which of the command interpreter's two links a command belongs on.
///
/// There are two because what the ground needs in a hurry must never be
/// queued behind what it asked for at leisure: a RESTART waiting behind a
/// payload's statistics is a spacecraft that cannot be rescued while it is
/// busy. On a space link the two become two virtual channels, which is where
/// the priority between them is really decided; over IP they are two ports --
/// `port` and `payload_port`; see [`crate::CIConfigJson`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandLink {
    /// The spacecraft's own link: ping it, arm it, restart it, configure it.
    Spacecraft,
    /// A payload's link: start one, stop one, ask one what it has moved.
    Payload,
}

impl CommandLink {
    /// The word a message uses for this link.
    pub fn spelling(&self) -> &'static str {
        match self {
            CommandLink::Spacecraft => "spacecraft command",
            CommandLink::Payload => "payload command",
        }
    }
}

impl CommandType {
    /// Which link this command is sent on and taken on.
    ///
    /// One table, read by the ground when it sends and by the spacecraft when
    /// it answers, so that the two cannot come to disagree about where a
    /// command belongs. A disagreement there is the worst kind: nothing can
    /// report it except as a command that went unanswered.
    ///
    /// A match over every kind rather than a rule with an exception, so that
    /// a command added later is placed deliberately -- by someone who has to
    /// decide, rather than by whichever side of a default it fell on.
    ///
    /// CONNECT is the spacecraft's. It is about having a connection to the
    /// vehicle at all -- what the end that answers read, asked as a link comes
    /// up -- and a ground station with no connection has nothing to ask a
    /// payload. A CONNECT that arrives on a payload's link is refused like any
    /// other command on the wrong one.
    pub fn link(&self) -> CommandLink {
        match self {
            CommandType::Ping
            | CommandType::Connect
            | CommandType::RestartArm
            | CommandType::Restart
            | CommandType::Config => CommandLink::Spacecraft,

            CommandType::StartDH
            | CommandType::StopDH
            | CommandType::QueryDH
            | CommandType::QueryDHSample
            | CommandType::ConfigDH
            | CommandType::StartNode
            | CommandType::StopNode
            | CommandType::SetNode
            | CommandType::QueryNode
            | CommandType::QueryNodeSample => CommandLink::Payload,
        }
    }

    /// Whether a command of this kind may be taken on `link`.
    ///
    /// A command that arrives on the other one is refused rather than served:
    /// it reached the spacecraft, so something can be said about it, and what
    /// has gone wrong is that one end is not reading the configuration the
    /// other is. Serving it would hide that, and hide it in the one place the
    /// split is supposed to be reliable.
    pub fn may_arrive_on(&self, link: CommandLink) -> bool {
        self.link() == link
    }

    pub fn to_u8(&self) -> u8 {
        match self {
            CommandType::Ping => 0x01,
            CommandType::Connect => 0x04,
            CommandType::RestartArm => 0x02,
            CommandType::Restart => 0x03,
            CommandType::StartDH => 0x10,
            CommandType::StopDH => 0x11,
            CommandType::QueryDH => 0x12,
            CommandType::QueryDHSample => 0x13,
            CommandType::Config => 0x20,
            CommandType::ConfigDH => 0x21,
            CommandType::StartNode => 0x30,
            CommandType::StopNode => 0x31,
            CommandType::SetNode => 0x32,
            CommandType::QueryNode => 0x33,
            CommandType::QueryNodeSample => 0x34,
        }
    }

    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(CommandType::Ping),
            0x04 => Some(CommandType::Connect),
            0x02 => Some(CommandType::RestartArm),
            0x03 => Some(CommandType::Restart),
            0x10 => Some(CommandType::StartDH),
            0x11 => Some(CommandType::StopDH),
            0x12 => Some(CommandType::QueryDH),
            0x13 => Some(CommandType::QueryDHSample),
            0x20 => Some(CommandType::Config),
            0x21 => Some(CommandType::ConfigDH),
            0x30 => Some(CommandType::StartNode),
            0x31 => Some(CommandType::StopNode),
            0x32 => Some(CommandType::SetNode),
            0x33 => Some(CommandType::QueryNode),
            0x34 => Some(CommandType::QueryNodeSample),
            _ => None,
        }
    }
}

/// PING command - verify TCSpecial is able to process commands
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PingCommand {
    pub header: CommandHeader,
}

impl PingCommand {
    pub fn new(sequence: u32) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::Ping,
            },
        }
    }
}

/// RESTART_ARM command - enable restart for next interval
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RestartArmCommand {
    pub header: CommandHeader,
    pub arm_key: ArmKey,
}

impl RestartArmCommand {
    pub fn new(sequence: u32, arm_key: ArmKey) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::RestartArm,
            },
            arm_key,
        }
    }
}

/// RESTART command - restart TCSpecial if armed
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RestartCommand {
    pub header: CommandHeader,
    pub arm_key: ArmKey,
}

impl RestartCommand {
    pub fn new(sequence: u32, arm_key: ArmKey) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::Restart,
            },
            arm_key,
        }
    }
}

/// CONNECT command - say what the ground read before anything else
///
/// Sent when a link is opened. It carries the version of the software at the
/// ground end and a digest of the configuration it read, and the answer
/// carries the same two from the spacecraft: nothing else in the protocol
/// makes the two ends prove they are talking about the same payload set, and
/// when they were not, the only sign was a command answered `NotFound` for a
/// payload the operator could see on the screen.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectCommand {
    pub header: CommandHeader,
    pub version: ConfigVersion,
    pub digest: ConfigDigest,
}

impl ConnectCommand {
    pub fn new(sequence: u32, version: ConfigVersion, digest: ConfigDigest) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::Connect,
            },
            version,
            digest,
        }
    }
}

/// The commands that carry a node id and nothing else
///
/// START_NODE, STOP_NODE, QUERY_NODE and QUERY_NODE_SAMPLE. One body for the
/// four, because there is nothing to say about a node beyond which node it
/// is: enabling it, disabling it, asking what it has moved and asking what it
/// last moved all name a node and ask for one thing. Version 1's START_DH
/// carried the kind of handler it expected as a check that the two ends had
/// read the same configuration; the id and the digest make that check
/// beforehand and for every command, so a kind here would be a second answer
/// to a settled question.
///
/// Either of them about a node already in the state asked for answers
/// success. A command that arrives twice, or whose answer was lost, is the
/// ordinary case on a link with latency, and an operator who presses Disable
/// twice has asked for a state and not for an action.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeCommand {
    pub header: CommandHeader,
    /// Which node, by its node id: see [`NodeId`].
    pub node: NodeId,
}

impl NodeCommand {
    /// Enable that node.
    pub fn start(sequence: u32, node: NodeId) -> Self {
        Self::of(sequence, CommandType::StartNode, node)
    }

    /// Disable it.
    pub fn stop(sequence: u32, node: NodeId) -> Self {
        Self::of(sequence, CommandType::StopNode, node)
    }

    /// Ask what it has moved.
    pub fn query(sequence: u32, node: NodeId) -> Self {
        Self::of(sequence, CommandType::QueryNode, node)
    }

    /// Ask for the last datum it moved each way.
    pub fn query_sample(sequence: u32, node: NodeId) -> Self {
        Self::of(sequence, CommandType::QueryNodeSample, node)
    }

    fn of(sequence: u32, cmd_type: CommandType, node: NodeId) -> Self {
        Self {
            header: CommandHeader { sequence, cmd_type },
            node,
        }
    }
}

/// SET_NODE - set an attribute of a node of a version 2 graph
///
/// Which node, which of its attributes, and what to set it to. The
/// configuration's `commands` section says which attribute of which node the
/// ground may set -- `{ from: moc, sets: thermostat.setpoint }` -- and a
/// command naming an attribute no binding mentions is refused. That is what
/// makes what the ground may change in flight something written down and
/// checkable rather than whatever a computation happens to accept: a bus
/// address or a set of line numbers is what the file said.
///
/// The attribute is text, where the node is a number. A node id can be a
/// fixed-length field because a node has a position in the configuration and
/// the digest covers that order; an attribute has no such number of its own.
/// A binding's position in the `commands` section would be one -- and would
/// carry the node with it -- but the value here is text of no fixed length
/// anyway, so a message of fixed length was never on offer, and a command
/// that says `setpoint` is one a log reads back.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetNodeCommand {
    pub header: CommandHeader,
    /// Which node, by its node id: see [`NodeId`].
    pub node: NodeId,
    /// Which of its attributes, as the `commands` section spells it.
    pub attribute: String,
    /// What to set it to, as the bytes the node reads it from.
    ///
    /// Bytes and not a number, because what an attribute takes is the node's
    /// business: a setpoint is a temperature, a band is a width, and a node
    /// that wanted a word would want a word. The node refuses what it cannot
    /// use, which is where a setpoint that is not a number is caught.
    pub value: Vec<u8>,
}

impl SetNodeCommand {
    pub fn new(sequence: u32, node: NodeId, attribute: &str, value: &[u8]) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::SetNode,
            },
            node,
            attribute: attribute.to_string(),
            value: value.to_vec(),
        }
    }
}

/// START_DH command - start a data handler
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartDHCommand {
    pub header: CommandHeader,
    pub dh_type: DHType,
    pub name: DHName,
}

impl StartDHCommand {
    pub fn new(sequence: u32, dh_type: DHType, name: DHName) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::StartDH,
            },
            dh_type,
            name,
        }
    }
}

/// STOP_DH command - stop a data handler
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StopDHCommand {
    pub header: CommandHeader,
    pub name: DHName,
}

impl StopDHCommand {
    pub fn new(sequence: u32, name: DHName) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::StopDH,
            },
            name,
        }
    }
}

/// QUERY_DH command - query statistics from a data handler
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueryDHCommand {
    pub header: CommandHeader,
    pub name: DHName,
}

impl QueryDHCommand {
    pub fn new(sequence: u32, name: DHName) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::QueryDH,
            },
            name,
        }
    }
}

/// QUERY_DH_SAMPLE command - ask what a data handler last sent and received
///
/// Separate from QUERY_DH so that the statistics every poll asks for stay the
/// size they are. A sample is wanted only while someone is looking at a
/// panel, and carrying payload bytes in routine telemetry would spend downlink
/// on data nobody reads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueryDHSampleCommand {
    pub header: CommandHeader,
    pub name: DHName,
}

impl QueryDHSampleCommand {
    pub fn new(sequence: u32, name: DHName) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::QueryDHSample,
            },
            name,
        }
    }
}

/// CONFIG command - configure TCSpecial values
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigCommand {
    pub header: CommandHeader,
    pub beacon_interval: BeaconInterval,
}

impl ConfigCommand {
    pub fn new(sequence: u32, beacon_interval: BeaconInterval) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::Config,
            },
            beacon_interval,
        }
    }
}

/// CONFIG_DH command - configure data handler values
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigDHCommand {
    pub header: CommandHeader,
    pub name: DHName,
    // Additional configuration fields can be added here
}

impl ConfigDHCommand {
    pub fn new(sequence: u32, name: DHName) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::ConfigDH,
            },
            name,
        }
    }
}

/// Union of all command types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Command {
    Ping(PingCommand),
    Connect(ConnectCommand),
    RestartArm(RestartArmCommand),
    Restart(RestartCommand),
    StartDH(StartDHCommand),
    StopDH(StopDHCommand),
    QueryDH(QueryDHCommand),
    QueryDHSample(QueryDHSampleCommand),
    Config(ConfigCommand),
    ConfigDH(ConfigDHCommand),
    StartNode(NodeCommand),
    StopNode(NodeCommand),
    SetNode(SetNodeCommand),
    QueryNode(NodeCommand),
    QueryNodeSample(NodeCommand),
}

impl Command {
    pub fn sequence(&self) -> u32 {
        match self {
            Command::Ping(cmd) => cmd.header.sequence,
            Command::Connect(cmd) => cmd.header.sequence,
            Command::RestartArm(cmd) => cmd.header.sequence,
            Command::Restart(cmd) => cmd.header.sequence,
            Command::StartDH(cmd) => cmd.header.sequence,
            Command::StopDH(cmd) => cmd.header.sequence,
            Command::QueryDH(cmd) => cmd.header.sequence,
            Command::QueryDHSample(cmd) => cmd.header.sequence,
            Command::Config(cmd) => cmd.header.sequence,
            Command::ConfigDH(cmd) => cmd.header.sequence,
            Command::StartNode(cmd)
            | Command::StopNode(cmd)
            | Command::QueryNode(cmd)
            | Command::QueryNodeSample(cmd) => cmd.header.sequence,
            Command::SetNode(cmd) => cmd.header.sequence,
        }
    }

    /// Which node this command is about, where it is about one.
    ///
    /// What [`Command::payload_name`] is for a version 1 payload set, for a
    /// version 2 graph: `None` for every command that names no node, so that
    /// a command added later is placed by someone who had to decide.
    pub fn node_id(&self) -> Option<NodeId> {
        match self {
            Command::StartNode(cmd)
            | Command::StopNode(cmd)
            | Command::QueryNode(cmd)
            | Command::QueryNodeSample(cmd) => Some(cmd.node),
            Command::SetNode(cmd) => Some(cmd.node),
            Command::Ping(_)
            | Command::Connect(_)
            | Command::RestartArm(_)
            | Command::Restart(_)
            | Command::Config(_)
            | Command::StartDH(_)
            | Command::StopDH(_)
            | Command::QueryDH(_)
            | Command::QueryDHSample(_)
            | Command::ConfigDH(_) => None,
        }
    }

    /// Which link this command belongs on; see [`CommandType::link`].
    pub fn link(&self) -> CommandLink {
        self.cmd_type().link()
    }

    /// Which payload this command is about, where it is about one.
    ///
    /// Every payload command carries the name of its payload, and nothing
    /// else does: that is what makes a payload command one. Both ends ask
    /// this of the same table -- the ground to choose the socket that payload
    /// is commanded on, the spacecraft to refuse a command that arrived on
    /// another payload's -- so neither can come to disagree about which
    /// payload a command is for.
    ///
    /// `None` for a spacecraft command, and for CONNECT, which is about the
    /// configuration the far end read rather than about any payload.
    pub fn payload_name(&self) -> Option<&DHName> {
        match self {
            Command::StartDH(cmd) => Some(&cmd.name),
            Command::StopDH(cmd) => Some(&cmd.name),
            Command::QueryDH(cmd) => Some(&cmd.name),
            Command::QueryDHSample(cmd) => Some(&cmd.name),
            Command::ConfigDH(cmd) => Some(&cmd.name),
            Command::Ping(_)
            | Command::Connect(_)
            | Command::RestartArm(_)
            | Command::Restart(_)
            | Command::Config(_)
            // A version 2 node command names a node, which is not a payload
            // name: see [`Command::node_id`].
            | Command::StartNode(_)
            | Command::StopNode(_)
            | Command::SetNode(_)
            | Command::QueryNode(_)
            | Command::QueryNodeSample(_) => None,
        }
    }

    pub fn cmd_type(&self) -> CommandType {
        match self {
            Command::Ping(cmd) => cmd.header.cmd_type,
            Command::Connect(cmd) => cmd.header.cmd_type,
            Command::RestartArm(cmd) => cmd.header.cmd_type,
            Command::Restart(cmd) => cmd.header.cmd_type,
            Command::StartDH(cmd) => cmd.header.cmd_type,
            Command::StopDH(cmd) => cmd.header.cmd_type,
            Command::QueryDH(cmd) => cmd.header.cmd_type,
            Command::QueryDHSample(cmd) => cmd.header.cmd_type,
            Command::Config(cmd) => cmd.header.cmd_type,
            Command::ConfigDH(cmd) => cmd.header.cmd_type,
            Command::StartNode(cmd)
            | Command::StopNode(cmd)
            | Command::QueryNode(cmd)
            | Command::QueryNodeSample(cmd) => cmd.header.cmd_type,
            Command::SetNode(cmd) => cmd.header.cmd_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every command belongs to one link, and the two classes between them
    /// hold every command there is.
    ///
    /// The table is what the two ends agree by, so what matters is that it is
    /// total: a command with no link would be a command the ground could not
    /// send and the spacecraft could not refuse.
    #[test]
    fn every_command_says_which_link_it_belongs_on() {
        let spacecraft = [
            CommandType::Ping,
            CommandType::Connect,
            CommandType::RestartArm,
            CommandType::Restart,
            CommandType::Config,
        ];
        let payload = [
            CommandType::StartDH,
            CommandType::StopDH,
            CommandType::QueryDH,
            CommandType::QueryDHSample,
            CommandType::ConfigDH,
            // A version 2 graph's nodes are commanded on this link too, and
            // for the same reason its payloads were: what the ground needs in
            // a hurry must not be queued behind what it asked for at leisure.
            CommandType::StartNode,
            CommandType::StopNode,
            CommandType::SetNode,
            CommandType::QueryNode,
            CommandType::QueryNodeSample,
        ];

        for kind in spacecraft {
            assert_eq!(kind.link(), CommandLink::Spacecraft, "{kind:?}");
            assert!(kind.may_arrive_on(CommandLink::Spacecraft), "{kind:?}");
            assert!(
                !kind.may_arrive_on(CommandLink::Payload),
                "{kind:?} is not a payload's business"
            );
        }

        for kind in payload {
            assert_eq!(kind.link(), CommandLink::Payload, "{kind:?}");
            assert!(kind.may_arrive_on(CommandLink::Payload), "{kind:?}");
            assert!(
                !kind.may_arrive_on(CommandLink::Spacecraft),
                "{kind:?} is a payload's, and the spacecraft link is for what \
                 cannot wait behind one"
            );
        }

        // Every command, and no command twice: the two lists above are the
        // whole of the enumeration, which is what makes the table total.
        let mut all: Vec<u8> = spacecraft
            .iter()
            .chain(payload.iter())
            .map(|kind| kind.to_u8())
            .collect();
        all.sort_unstable();
        let mut known: Vec<u8> = (0u8..=255)
            .filter(|byte| CommandType::from_u8(*byte).is_some())
            .collect();
        known.sort_unstable();
        assert_eq!(all, known, "a command was left out of both lists");
    }

    /// A CONNECT is the spacecraft's link's, and no payload's.
    ///
    /// It is about having a connection to the vehicle, which is what the
    /// spacecraft's link is; a ground station with none has nothing to ask a
    /// payload.
    #[test]
    fn what_each_end_read_is_asked_of_the_spacecraft() {
        assert_eq!(CommandType::Connect.link(), CommandLink::Spacecraft);
        assert!(CommandType::Connect.may_arrive_on(CommandLink::Spacecraft));
        assert!(!CommandType::Connect.may_arrive_on(CommandLink::Payload));
    }

    #[test]
    fn test_command_type_conversion() {
        let cmd_type = CommandType::Ping;
        assert_eq!(cmd_type.to_u8(), 0x01);
        assert_eq!(CommandType::from_u8(0x01), Some(CommandType::Ping));
    }

    #[test]
    fn test_ping_command() {
        let cmd = PingCommand::new(1);
        assert_eq!(cmd.header.sequence, 1);
        assert_eq!(cmd.header.cmd_type, CommandType::Ping);
    }

    #[test]
    fn test_command_serialization() {
        let cmd = Command::Ping(PingCommand::new(42));
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, deserialized);
    }
}
