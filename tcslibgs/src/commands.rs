//! Command definitions for TCSpecial
//!
//! Commands are sent from ground to space and are idempotent.

use crate::config_digest::{ConfigDigest, ConfigVersion};
use crate::types::{ArmKey, BeaconInterval, DHName, NodeId};

/// Command message header
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandHeader {
    /// Command sequence number for tracking
    pub sequence: u32,
    /// Command type identifier
    pub cmd_type: CommandType,
    /// The generation of tcspecial this command was sent for: the one the
    /// ground last heard, from a beacon or a CONNECT's answer. Nought, which
    /// no start of tcspecial is, where the ground has heard none.
    ///
    /// Stamped by whatever sends the command, as it goes -- see
    /// [`Command::for_generation`] -- because the sender is what knows which
    /// generation it last heard, and a command built before a restart and
    /// sent after one must say the generation it is sent in. The vehicle
    /// refuses a command of any other generation than its own, but for
    /// those [`CommandType::always_taken`] names: see "The
    /// generation number and the state file" in `docs/design.rst`.
    pub generation: u32,
}

/// Command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Config,
    /// Enable a node.
    ///
    /// The five below are what answer for a node, which a command names by
    /// id. They do not reuse the opcodes of the five that named a data handler
    /// before them -- START_DH, STOP_DH, QUERY_DH, QUERY_DH_SAMPLE and
    /// CONFIG_DH -- so a ground station of an older build is told its command
    /// is not one rather than having it acted on as another.
    StartNode,
    /// And disable one.
    StopNode,
    /// What a node has moved.
    QueryNode,
    /// The last datum a node moved each way.
    QueryNodeSample,
    /// Set an attribute of a node.
    ///
    /// A graph's `commands` section says which attribute of which node the
    /// ground may set, so this is the command that section was written for.
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
    /// Names this link in the words a message uses.
    ///
    /// # Returns
    ///
    /// "spacecraft command" or "payload command".
    pub fn spelling(&self) -> &'static str {
        match self {
            CommandLink::Spacecraft => "spacecraft command",
            CommandLink::Payload => "payload command",
        }
    }
}

impl CommandType {
    /// Says which link this command is sent on and taken on.
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
    ///
    /// # Returns
    ///
    /// The link a command of this kind belongs to.
    pub fn link(&self) -> CommandLink {
        match self {
            CommandType::Ping
            | CommandType::Connect
            | CommandType::RestartArm
            | CommandType::Restart
            | CommandType::Config => CommandLink::Spacecraft,

            CommandType::StartNode
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

    /// Tells whether a command of this kind is taken whatever generation it
    /// was sent for, and however old it is.
    ///
    /// PING and CONNECT, and nothing else: they are how a ground station that
    /// has heard no beacon finds out what is flying, and where one whose count
    /// has gone back learns where to start it again, and neither changes
    /// anything aboard, so neither can do harm by reaching a vehicle that is
    /// not the one it was meant for or by arriving late. Every other command
    /// acts on definitions a restart may have changed, and one sent before a
    /// restart and delivered after it -- the usual case on a link
    /// light-seconds long -- is refused rather than read with definitions it
    /// was not written for; and one older than a command already taken is
    /// not acted on, because the later one is what the ground meant last.
    ///
    /// # Returns
    ///
    /// `true` for PING and CONNECT, `false` for every other kind.
    pub fn always_taken(&self) -> bool {
        match self {
            CommandType::Ping | CommandType::Connect => true,

            CommandType::RestartArm
            | CommandType::Restart
            | CommandType::Config
            | CommandType::StartNode
            | CommandType::StopNode
            | CommandType::SetNode
            | CommandType::QueryNode
            | CommandType::QueryNodeSample => false,
        }
    }

    /// Gives the opcode a command of this kind carries. What goes on the link
    /// is stated in `xtce/link.xml`, and a test in [`crate::link`] holds this
    /// to it, so the two cannot come to differ.
    ///
    /// # Returns
    ///
    /// The opcode.
    pub fn to_u8(&self) -> u8 {
        match self {
            CommandType::Ping => 0x01,
            CommandType::Connect => 0x04,
            CommandType::RestartArm => 0x02,
            CommandType::Restart => 0x03,
            CommandType::Config => 0x20,
            CommandType::StartNode => 0x30,
            CommandType::StopNode => 0x31,
            CommandType::SetNode => 0x32,
            CommandType::QueryNode => 0x33,
            CommandType::QueryNodeSample => 0x34,
        }
    }

    /// Reads the kind of command an opcode names: the inverse of
    /// [`CommandType::to_u8`].
    ///
    /// # Arguments
    ///
    /// * `value` - The opcode from the link.
    ///
    /// # Returns
    ///
    /// The kind, or `None` for an opcode no command has.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(CommandType::Ping),
            0x04 => Some(CommandType::Connect),
            0x02 => Some(CommandType::RestartArm),
            0x03 => Some(CommandType::Restart),
            0x20 => Some(CommandType::Config),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PingCommand {
    pub header: CommandHeader,
}

impl PingCommand {
    /// Makes a PING, its generation not yet stamped.
    ///
    /// # Arguments
    ///
    /// * `sequence` - The command's sequence number.
    ///
    /// # Returns
    ///
    /// The command.
    pub fn new(sequence: u32) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::Ping,
                generation: 0,
            },
        }
    }
}

/// How long a RESTART_ARM lasts where the configuration does not say:
/// twenty minutes from when the vehicle takes it. The `tcspecial` section's
/// `restart_arm_valid` says otherwise -- see
/// [`crate::CIConfig::restart_arm_valid`].
///
/// A RESTART is refused unless an arm with its key was taken within this long
/// before it. Long enough for an operator to arm, confirm what the vehicle is
/// doing and decide, over a link with minutes of latency; short enough that an
/// arm left behind by a session that ended does not stand ready to restart the
/// vehicle on the next RESTART anybody sends.
pub const RESTART_ARM_VALID_DEFAULT: std::time::Duration =
    std::time::Duration::from_secs(20 * 60);

/// RESTART_ARM command - enable a restart for the next `restart_arm_valid`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartArmCommand {
    pub header: CommandHeader,
    pub arm_key: ArmKey,
}

impl RestartArmCommand {
    /// Makes a RESTART_ARM, its generation not yet stamped.
    ///
    /// # Arguments
    ///
    /// * `sequence` - The command's sequence number.
    /// * `arm_key`  - The key the RESTART that follows must carry.
    ///
    /// # Returns
    ///
    /// The command.
    pub fn new(sequence: u32, arm_key: ArmKey) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::RestartArm,
                generation: 0,
            },
            arm_key,
        }
    }
}

/// RESTART command - restart TCSpecial if armed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartCommand {
    pub header: CommandHeader,
    pub arm_key: ArmKey,
}

impl RestartCommand {
    /// Makes a RESTART, its generation not yet stamped.
    ///
    /// # Arguments
    ///
    /// * `sequence` - The command's sequence number.
    /// * `arm_key`  - The key the RESTART_ARM before it carried.
    ///
    /// # Returns
    ///
    /// The command.
    pub fn new(sequence: u32, arm_key: ArmKey) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::Restart,
                generation: 0,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectCommand {
    pub header: CommandHeader,
    pub version: ConfigVersion,
    pub digest: ConfigDigest,
}

impl ConnectCommand {
    /// Makes a CONNECT, its generation not yet stamped.
    ///
    /// # Arguments
    ///
    /// * `sequence` - The command's sequence number.
    /// * `version`  - The version of the ground end's build.
    /// * `digest`   - The digest of the configuration the ground end read.
    ///
    /// # Returns
    ///
    /// The command.
    pub fn new(
        sequence: u32,
        version: ConfigVersion,
        digest: ConfigDigest,
    ) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::Connect,
                generation: 0,
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
/// last moved all name a node and ask for one thing. No kind of node rides
/// along as a check that the two ends read the same configuration: the id and
/// the digest make that check beforehand and for every command, so a kind
/// here would be a second answer to a settled question.
///
/// Either of them about a node already in the state asked for answers
/// success. A command that arrives twice, or whose answer was lost, is the
/// ordinary case on a link with latency, and an operator who presses Disable
/// twice has asked for a state and not for an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    /// Makes a node command of any of the four kinds, its generation not yet
    /// stamped.
    ///
    /// # Arguments
    ///
    /// * `sequence` - The command's sequence number.
    /// * `cmd_type` - Which of the node commands it is.
    /// * `node`     - The node it is about.
    ///
    /// # Returns
    ///
    /// The command.
    fn of(sequence: u32, cmd_type: CommandType, node: NodeId) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type,
                generation: 0,
            },
            node,
        }
    }
}

/// SET_NODE - set an attribute of a node
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
/// carry the node with it -- but a command that says `setpoint` is one a log
/// reads back.
///
/// Both are held in room of a fixed size, [`ATTRIBUTE_BYTES`] and
/// [`VALUE_BYTES`], so that the vehicle reads one without allocating. On the
/// link the attribute is a byte of length and its text, and the value two
/// bytes of length and its bytes: see `xtce/link.xml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetNodeCommand {
    pub header: CommandHeader,
    /// Which node, by its node id: see [`NodeId`].
    pub node: NodeId,
    /// Which of its attributes, as the `commands` section spells it.
    pub attribute: Attribute,
    /// What to set it to, as the bytes the node reads it from.
    ///
    /// Bytes and not a number, because what an attribute takes is the node's
    /// business: a setpoint is a temperature, a band is a width, and a node
    /// that wanted a word would want a word. The node refuses what it cannot
    /// use, which is where a setpoint that is not a number is caught.
    pub value: Value,
}

impl SetNodeCommand {
    /// Makes a SET_NODE, its generation not yet stamped.
    ///
    /// # Arguments
    ///
    /// * `sequence`  - The command's sequence number.
    /// * `node`      - The node whose attribute is set.
    /// * `attribute` - The attribute's name.
    /// * `value`     - The value it is set to.
    ///
    /// # Returns
    ///
    /// The command.
    ///
    /// # Errors
    ///
    /// An attribute longer than [`ATTRIBUTE_BYTES`] or a value longer than
    /// [`VALUE_BYTES`] is refused here, on the ground, rather than sent to a
    /// vehicle that would refuse it.
    pub fn new(
        sequence: u32,
        node: NodeId,
        attribute: &str,
        value: &[u8],
    ) -> Result<Self, String> {
        Ok(Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::SetNode,
                generation: 0,
            },
            node,
            attribute: Attribute::of(attribute)?,
            value: Value::of(value)?,
        })
    }
}

/// The longest attribute name a SET_NODE carries, in bytes.
///
/// Longer than any attribute a node has, by some way; a command binding whose
/// attribute is longer is refused where the configuration is read, so every
/// attribute the ground may set is one a SET_NODE can name.
pub const ATTRIBUTE_BYTES: usize = 64;

/// The longest value a SET_NODE carries, in bytes.
///
/// A setpoint, a band, a word: what a node is set to is short. A value longer
/// is refused on the ground, by [`SetNodeCommand::new`], and by the vehicle as
/// it reads one.
pub const VALUE_BYTES: usize = 256;

/// Text of at most `N` bytes, held in place: what a SET_NODE's attribute is.
///
/// A string on the wire, as a `String` would be.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Attribute {
    bytes: [u8; ATTRIBUTE_BYTES],
    len: u8,
}

impl Attribute {
    /// Makes an attribute name of the text given.
    ///
    /// # Arguments
    ///
    /// * `name` - The attribute's name.
    ///
    /// # Returns
    ///
    /// The name.
    ///
    /// # Errors
    ///
    /// A message when it is longer than [`ATTRIBUTE_BYTES`].
    pub fn of(name: &str) -> Result<Attribute, String> {
        if name.len() > ATTRIBUTE_BYTES {
            return Err(format!(
                "the attribute \"{name}\" is {} bytes long, and a command \
                 names one of at most {ATTRIBUTE_BYTES}",
                name.len()
            ));
        }
        let mut bytes = [0; ATTRIBUTE_BYTES];
        bytes[..name.len()].copy_from_slice(name.as_bytes());
        Ok(Attribute {
            bytes,
            len: name.len() as u8,
        })
    }

    pub fn as_str(&self) -> &str {
        // Made only from a `&str`, whole: see `of`.
        std::str::from_utf8(&self.bytes[..usize::from(self.len)])
            .unwrap_or_default()
    }
}

impl std::ops::Deref for Attribute {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq<&str> for Attribute {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<str> for Attribute {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl std::fmt::Debug for Attribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl std::fmt::Display for Attribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Bytes, at most [`VALUE_BYTES`] of them, held in place: what a SET_NODE's
/// value is.
///
/// An array of numbers on the wire, as a `Vec<u8>` would be.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Value {
    bytes: [u8; VALUE_BYTES],
    len: u16,
}

impl Value {
    /// Makes a value of the bytes given.
    ///
    /// # Arguments
    ///
    /// * `value` - The bytes.
    ///
    /// # Returns
    ///
    /// The value.
    ///
    /// # Errors
    ///
    /// A message when there are more than [`VALUE_BYTES`] of them.
    pub fn of(value: &[u8]) -> Result<Value, String> {
        if value.len() > VALUE_BYTES {
            return Err(format!(
                "a value of {} bytes is longer than a command carries, which \
                 is at most {VALUE_BYTES}",
                value.len()
            ));
        }
        let mut bytes = [0; VALUE_BYTES];
        bytes[..value.len()].copy_from_slice(value);
        Ok(Value {
            bytes,
            len: value.len() as u16,
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

impl std::ops::Deref for Value {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_bytes(), f)
    }
}

/// CONFIG command - configure TCSpecial values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigCommand {
    pub header: CommandHeader,
    pub beacon_interval: BeaconInterval,
}

impl ConfigCommand {
    /// Makes a CONFIG, its generation not yet stamped.
    ///
    /// # Arguments
    ///
    /// * `sequence`        - The command's sequence number.
    /// * `beacon_interval` - The beacon interval it sets.
    ///
    /// # Returns
    ///
    /// The command.
    pub fn new(sequence: u32, beacon_interval: BeaconInterval) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::Config,
                generation: 0,
            },
            beacon_interval,
        }
    }
}
/// Union of all command types
///
/// Whether command sequence `a` was sent before `b`.
///
/// The sequence is four bytes and counts on past its largest to nought, so
/// "before" is serial-number arithmetic (RFC 1982): `a` is before `b` when `b`
/// is less than half the counter's range ahead of it. A count that wraps
/// would otherwise make every command after it look older than the last one
/// taken, and be answered `Stale` for ever. The vehicle refuses a command by
/// this and a ground station starts its count again by it, so both ask here.
pub fn sequence_is_before(a: u32, b: u32) -> bool {
    (b.wrapping_sub(a) as i32) > 0
}

/// As large as a SET_NODE, whose attribute and value are held in place: a
/// SET_NODE boxed to make the rest smaller would be an allocation for each
/// one read, which is what holding them in place is for.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Ping(PingCommand),
    Connect(ConnectCommand),
    RestartArm(RestartArmCommand),
    Restart(RestartCommand),
    Config(ConfigCommand),
    StartNode(NodeCommand),
    StopNode(NodeCommand),
    SetNode(SetNodeCommand),
    QueryNode(NodeCommand),
    QueryNodeSample(NodeCommand),
}

impl Command {
    /// The generation this command was sent for: see
    /// [`CommandHeader::generation`].
    pub fn generation(&self) -> u32 {
        self.header().generation
    }

    /// This command, sent for `generation`.
    pub fn for_generation(mut self, generation: u32) -> Self {
        self.header_mut().generation = generation;
        self
    }

    /// Gives this command's header, whichever kind it is.
    ///
    /// # Returns
    ///
    /// The header.
    fn header(&self) -> &CommandHeader {
        match self {
            Command::Ping(cmd) => &cmd.header,
            Command::Connect(cmd) => &cmd.header,
            Command::RestartArm(cmd) => &cmd.header,
            Command::Restart(cmd) => &cmd.header,
            Command::Config(cmd) => &cmd.header,
            Command::StartNode(cmd)
            | Command::StopNode(cmd)
            | Command::QueryNode(cmd)
            | Command::QueryNodeSample(cmd) => &cmd.header,
            Command::SetNode(cmd) => &cmd.header,
        }
    }

    /// Gives this command's header to change, whichever kind it is.
    ///
    /// # Returns
    ///
    /// The header.
    fn header_mut(&mut self) -> &mut CommandHeader {
        match self {
            Command::Ping(cmd) => &mut cmd.header,
            Command::Connect(cmd) => &mut cmd.header,
            Command::RestartArm(cmd) => &mut cmd.header,
            Command::Restart(cmd) => &mut cmd.header,
            Command::Config(cmd) => &mut cmd.header,
            Command::StartNode(cmd)
            | Command::StopNode(cmd)
            | Command::QueryNode(cmd)
            | Command::QueryNodeSample(cmd) => &mut cmd.header,
            Command::SetNode(cmd) => &mut cmd.header,
        }
    }

    /// Gives this command's sequence number.
    ///
    /// # Returns
    ///
    /// The sequence number from its header.
    pub fn sequence(&self) -> u32 {
        match self {
            Command::Ping(cmd) => cmd.header.sequence,
            Command::Connect(cmd) => cmd.header.sequence,
            Command::RestartArm(cmd) => cmd.header.sequence,
            Command::Restart(cmd) => cmd.header.sequence,
            Command::Config(cmd) => cmd.header.sequence,
            Command::StartNode(cmd)
            | Command::StopNode(cmd)
            | Command::QueryNode(cmd)
            | Command::QueryNodeSample(cmd) => cmd.header.sequence,
            Command::SetNode(cmd) => cmd.header.sequence,
        }
    }

    /// Says which node this command is about, where it is about one.
    ///
    /// # Returns
    ///
    /// The node's id, or `None` for every command that names no node, each
    /// of them named in the match, so that a command added later is placed
    /// by someone who had to decide.
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
            | Command::Config(_) => None,
        }
    }

    /// Which link this command belongs on; see [`CommandType::link`].
    pub fn link(&self) -> CommandLink {
        self.cmd_type().link()
    }

    /// Says which payload this command is about, where it is about one.
    ///
    /// Every payload command carries the name of its payload, and nothing
    /// else does: that is what makes a payload command one. Both ends ask
    /// this of the same table -- the ground to choose the socket that payload
    /// is commanded on, the spacecraft to refuse a command that arrived on
    /// another payload's -- so neither can come to disagree about which
    /// payload a command is for.
    ///
    /// # Returns
    ///
    /// The payload's name, or `None` for a spacecraft command, and for
    /// CONNECT, which is about the configuration the far end read rather than
    /// about any payload.
    pub fn payload_name(&self) -> Option<&DHName> {
        match self {
            Command::Ping(_)
            | Command::Connect(_)
            | Command::RestartArm(_)
            | Command::Restart(_)
            | Command::Config(_)
            // A node command names a node, which is not a payload
            // name: see [`Command::node_id`].
            | Command::StartNode(_)
            | Command::StopNode(_)
            | Command::SetNode(_)
            | Command::QueryNode(_)
            | Command::QueryNodeSample(_) => None,
        }
    }

    /// Gives this command's kind.
    ///
    /// # Returns
    ///
    /// The kind from its header.
    pub fn cmd_type(&self) -> CommandType {
        match self {
            Command::Ping(cmd) => cmd.header.cmd_type,
            Command::Connect(cmd) => cmd.header.cmd_type,
            Command::RestartArm(cmd) => cmd.header.cmd_type,
            Command::Restart(cmd) => cmd.header.cmd_type,
            Command::Config(cmd) => cmd.header.cmd_type,
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

    /// One sequence is before another by serial-number arithmetic, so a
    /// count that wraps past its largest is still going forward.
    #[test]
    fn a_sequence_is_before_one_that_follows_it_even_across_the_wrap() {
        assert!(sequence_is_before(1, 2));
        assert!(!sequence_is_before(2, 1));
        assert!(
            !sequence_is_before(5, 5),
            "the same one is not before itself"
        );
        assert!(
            sequence_is_before(u32::MAX, 0),
            "nought follows the largest"
        );
        assert!(sequence_is_before(u32::MAX - 3, 2));
        assert!(!sequence_is_before(2, u32::MAX - 3));
    }

    /// An arm lasts twenty minutes unless the configuration says otherwise,
    /// and the manual says the same: an operator deciding whether there is
    /// time to restart reads it there.
    #[test]
    fn an_arm_lasts_as_long_as_the_manual_says() {
        assert_eq!(
            RESTART_ARM_VALID_DEFAULT,
            std::time::Duration::from_secs(20 * 60)
        );
        let manual = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../docs/manual.rst"),
        )
        .expect("the manual reads");
        let said = manual.split_whitespace().collect::<Vec<_>>().join(" ");
        let minutes = RESTART_ARM_VALID_DEFAULT.as_secs() / 60;
        assert!(
            said.contains(&format!("An arm lasts {minutes} minutes")),
            "the manual should say an arm lasts {minutes} minutes"
        );
    }

    /// A SET_NODE reads back off the link as it was written, and refuses
    /// what will not fit.
    #[test]
    fn a_set_node_reads_back_and_refuses_what_will_not_fit() {
        let command =
            SetNodeCommand::new(3, NodeId::of(2), "setpoint", b"295.0")
                .expect("it fits");
        let Ok(Command::SetNode(back)) =
            Command::read(&Command::SetNode(command).to_bytes())
        else {
            panic!("a SET_NODE");
        };
        assert_eq!(back, command);
        assert_eq!(back.attribute, "setpoint");
        assert_eq!(back.value.as_bytes(), b"295.0");

        // Each limit, exactly, and one past it.
        let longest = "a".repeat(ATTRIBUTE_BYTES);
        assert!(SetNodeCommand::new(1, NodeId::of(0), &longest, b"").is_ok());
        let said =
            SetNodeCommand::new(1, NodeId::of(0), &format!("{longest}a"), b"")
                .expect_err("too long a name");
        assert!(said.contains(&ATTRIBUTE_BYTES.to_string()), "{said}");
        assert!(
            SetNodeCommand::new(1, NodeId::of(0), "x", &[7; VALUE_BYTES])
                .is_ok()
        );
        assert!(SetNodeCommand::new(
            1,
            NodeId::of(0),
            "x",
            &[7; VALUE_BYTES + 1]
        )
        .is_err());

        // And as read off the link: a value too long is refused, not cut
        // short. Built by hand, as no packer will build one: the longest,
        // with its length one more and one more byte.
        let longest = Command::SetNode(
            SetNodeCommand::new(1, NodeId::of(0), "x", &[7; VALUE_BYTES])
                .expect("the longest fits"),
        )
        .to_bytes();
        let length_at = longest.len() - VALUE_BYTES - 2;
        let mut too_long = longest[..length_at].to_vec();
        too_long.extend(((VALUE_BYTES + 1) as u16).to_be_bytes());
        too_long.extend([7; VALUE_BYTES + 1]);
        let packet_length = (too_long.len() - 7) as u16;
        too_long[4..6].copy_from_slice(&packet_length.to_be_bytes());
        assert!(Command::read(&too_long).is_err());
    }

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
            // A node's commands go on this link because what the ground
            // needs in a hurry must not be queued behind what it asked for at
            // leisure.
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
}
