//! Telemetry definitions for TCSpecial
//!
//! Telemetry is sent from space to ground.
//!
//! These are the answers to commands, which go on the link as the packets
//! `xtce/link.xml` states: see [`crate::link`]. The beacon, which answers
//! nothing and has a layout of its own, is in [`crate::beacon`].

use crate::config_digest::{ConfigDigest, ConfigVersion};
use crate::types::{CommandStatus, DHSample, NodeCounts, NodeId, Timestamp};

/// Telemetry message header
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetryHeader {
    /// Sequence number matching the command that generated this response
    pub sequence: u32,
    /// The generation of tcspecial that made this answer: how many times it
    /// has started, from its state file, as the beacon says it.
    ///
    /// In every answer, refusals among them, so that a ground station hears
    /// of a restart from whatever answer reaches it first, and can tell
    /// which start of tcspecial took the command it is answering. Stamped as
    /// the answer goes -- see [`Telemetry::for_generation`] -- by the one
    /// place that sends answers, which is what knows it; nought until then,
    /// which no start is.
    pub generation: u32,
    /// Telemetry type identifier
    pub tm_type: TelemetryType,
    /// Command status
    pub status: CommandStatus,
    /// When the spacecraft made this answer.
    ///
    /// Not when the command was received, and not when whatever the answer is
    /// about happened: a QUERY_DH_SAMPLE carries the times its samples moved
    /// and this is the time it was asked. The two are worth telling apart --
    /// a sample from an hour ago in an answer made a second ago is a handler
    /// that has gone quiet, and without a time on the answer there is nothing
    /// to read that against.
    ///
    /// In the header, so that every response has one. Two of them used to
    /// carry a time of their own and the other eight carried none, which is
    /// the sort of thing a reader of the eight finds out by needing it.
    pub timestamp: Timestamp,
}

impl TelemetryHeader {
    /// Makes a header for an answer being made now.
    ///
    /// The only way one is built, so that a telemetry type added later cannot
    /// forget the time: there is nowhere to leave it out.
    ///
    /// # Arguments
    ///
    /// * `sequence` - The sequence number of the command it answers.
    /// * `tm_type`  - What kind of answer it is.
    /// * `status`   - How the command went.
    ///
    /// # Returns
    ///
    /// The header, stamped with the time now and with generation nought.
    pub fn new(
        sequence: u32,
        tm_type: TelemetryType,
        status: CommandStatus,
    ) -> Self {
        Self {
            sequence,
            generation: 0,
            tm_type,
            status,
            timestamp: Timestamp::now(),
        }
    }
}

/// Telemetry types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryType {
    Ping,
    Connect,
    RestartArm,
    Restart,
    /// The answer to a START_NODE: see [`NodeTelemetry`].
    StartNode,
    /// And to a STOP_NODE.
    StopNode,
    /// And to a SET_NODE.
    SetNode,
    /// The answer to a QUERY_NODE: see [`QueryNodeTelemetry`].
    QueryNode,
    /// And to a QUERY_NODE_SAMPLE: see [`QueryNodeSampleTelemetry`].
    QueryNodeSample,
}

impl TelemetryType {
    /// Gives the kind an answer of this type carries. What goes on the link
    /// is stated in `xtce/link.xml`, and a test in [`crate::link`] holds this
    /// to it, so the two cannot come to differ.
    ///
    /// # Returns
    ///
    /// The kind's byte on the link.
    pub fn to_u8(&self) -> u8 {
        match self {
            TelemetryType::Ping => 0x81,
            TelemetryType::Connect => 0x84,
            TelemetryType::RestartArm => 0x82,
            TelemetryType::Restart => 0x83,
            // 0xA0 is not used: it was CONFIG's answer.
            TelemetryType::StartNode => 0x30,
            TelemetryType::StopNode => 0x31,
            TelemetryType::SetNode => 0x32,
            TelemetryType::QueryNode => 0x33,
            TelemetryType::QueryNodeSample => 0x34,
        }
    }

    /// Reads the type an answer's kind byte names: [`TelemetryType::to_u8`]
    /// the other way.
    ///
    /// # Arguments
    ///
    /// * `value` - The kind's byte on the link.
    ///
    /// # Returns
    ///
    /// The type, or `None` for a byte that names none.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x81 => Some(TelemetryType::Ping),
            0x84 => Some(TelemetryType::Connect),
            0x82 => Some(TelemetryType::RestartArm),
            0x83 => Some(TelemetryType::Restart),
            0x30 => Some(TelemetryType::StartNode),
            0x31 => Some(TelemetryType::StopNode),
            0x32 => Some(TelemetryType::SetNode),
            0x33 => Some(TelemetryType::QueryNode),
            0x34 => Some(TelemetryType::QueryNodeSample),
            _ => None,
        }
    }
}

/// PING telemetry response
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PingTelemetry {
    pub header: TelemetryHeader,
}

impl PingTelemetry {
    pub fn new(sequence: u32, status: CommandStatus) -> Self {
        Self {
            header: TelemetryHeader::new(sequence, TelemetryType::Ping, status),
        }
    }
}

/// RESTART_ARM telemetry response
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartArmTelemetry {
    pub header: TelemetryHeader,
}

impl RestartArmTelemetry {
    /// Makes the answer to a RESTART_ARM, made now.
    ///
    /// # Arguments
    ///
    /// * `sequence` - The sequence number of the command it answers.
    /// * `status`   - How the command went.
    ///
    /// # Returns
    ///
    /// The answer.
    pub fn new(sequence: u32, status: CommandStatus) -> Self {
        Self {
            header: TelemetryHeader::new(
                sequence,
                TelemetryType::RestartArm,
                status,
            ),
        }
    }
}

/// RESTART telemetry response
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartTelemetry {
    pub header: TelemetryHeader,
}

impl RestartTelemetry {
    /// Makes the answer to a RESTART, made now.
    ///
    /// # Arguments
    ///
    /// * `sequence` - The sequence number of the command it answers.
    /// * `status`   - How the command went.
    ///
    /// # Returns
    ///
    /// The answer.
    pub fn new(sequence: u32, status: CommandStatus) -> Self {
        Self {
            header: TelemetryHeader::new(
                sequence,
                TelemetryType::Restart,
                status,
            ),
        }
    }
}

/// START_DH telemetry response
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectTelemetry {
    pub header: TelemetryHeader,
    pub version: ConfigVersion,
    /// The digest of the configuration this end read, which says which set
    /// it is serving: see `crate::config_digest`.
    pub digest: ConfigDigest,
    /// The newest command sequence this start of tcspecial has taken, or
    /// the CONNECT's own where it has taken none. A ground station whose
    /// count has gone back -- a station started again, or a second one --
    /// would otherwise have every command answered `Stale`; this is where
    /// it starts its count again. Nought in a refusal, which knows none.
    pub newest_taken: u32,
}

impl ConnectTelemetry {
    /// Makes a successful answer to a CONNECT, made now.
    ///
    /// # Arguments
    ///
    /// * `sequence`     - The sequence number of the CONNECT.
    /// * `version`      - The version of this end's build.
    /// * `digest`       - The digest of the configuration this end read.
    /// * `newest_taken` - The newest command sequence this start has taken.
    ///
    /// # Returns
    ///
    /// The answer.
    pub fn new(
        sequence: u32,
        version: ConfigVersion,
        digest: ConfigDigest,
        newest_taken: u32,
    ) -> Self {
        Self {
            header: TelemetryHeader::new(
                sequence,
                TelemetryType::Connect,
                CommandStatus::Success,
            ),
            version,
            digest,
            newest_taken,
        }
    }

    /// Makes the same answer with a status other than success.
    ///
    /// For a CONNECT that is refused rather than answered -- one that arrived
    /// on a payload's link, which is not what a connection to the vehicle is
    /// asked on. The facts are this end's all the same, so the ground
    /// is told what it asked as well as that it asked in the wrong place.
    ///
    /// # Arguments
    ///
    /// * `status` - Why it is refused.
    ///
    /// # Returns
    ///
    /// The answer, with a fresh header carrying `status`.
    pub fn refused(self, status: CommandStatus) -> Self {
        Self {
            header: TelemetryHeader::new(
                self.header.sequence,
                TelemetryType::Connect,
                status,
            ),
            ..self
        }
    }
}
/// The answer to a START_NODE or a STOP_NODE
///
/// The status, and the id the command carried. The id is here so that an
/// answer can be matched to its command without the name -- answers may
/// overtake one another on a link, and the sequence number pairs them while
/// the id says which node the pair was about. The ground shows the name,
/// which it has from the configuration it read: the same file the id is
/// counted from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeTelemetry {
    pub header: TelemetryHeader,
    /// The node the command named, whether or not the configuration has it:
    /// an id out of range is answered with the id that was asked about.
    pub node: NodeId,
}

impl NodeTelemetry {
    /// The answer to an enabling.
    pub fn started(sequence: u32, status: CommandStatus, node: NodeId) -> Self {
        Self::of(sequence, TelemetryType::StartNode, status, node)
    }

    /// And to a disabling.
    pub fn stopped(sequence: u32, status: CommandStatus, node: NodeId) -> Self {
        Self::of(sequence, TelemetryType::StopNode, status, node)
    }

    /// Makes an answer about one node, made now.
    ///
    /// # Arguments
    ///
    /// * `sequence` - The sequence number of the command it answers.
    /// * `tm_type`  - Which command it answers: START_NODE or STOP_NODE.
    /// * `status`   - How the command went.
    /// * `node`     - The node the command named.
    ///
    /// # Returns
    ///
    /// The answer.
    fn of(
        sequence: u32,
        tm_type: TelemetryType,
        status: CommandStatus,
        node: NodeId,
    ) -> Self {
        Self {
            header: TelemetryHeader::new(sequence, tm_type, status),
            node,
        }
    }
}

/// The answer to a SET_NODE: the status, the node, and why it was refused.
///
/// The node and not the attribute: the sequence number pairs the answer with
/// the command, which carries both, and what the ground shows is the name it
/// has from the configuration it read.
///
/// The reason is what the vehicle's log says of a refusal -- an attribute no
/// binding names, with its spelling where it was written with hyphens for
/// underscores; a value the node will not take -- because the status alone
/// is `InvalidParameter` for every one of them, and an operator who is told
/// only that has to go and read a log on the vehicle to find out which word
/// to change. Empty for a SET_NODE that was done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetNodeTelemetry {
    pub header: TelemetryHeader,
    /// The node the command named, as [`NodeTelemetry::node`].
    pub node: NodeId,
    /// Why it was refused, or nothing.
    pub reason: Reason,
}

impl SetNodeTelemetry {
    /// Makes the answer to a SET_NODE, made now, with no reason: see
    /// [`SetNodeTelemetry::because`].
    ///
    /// # Arguments
    ///
    /// * `sequence` - The sequence number of the command it answers.
    /// * `status`   - How the command went.
    /// * `node`     - The node the command named.
    ///
    /// # Returns
    ///
    /// The answer.
    pub fn new(sequence: u32, status: CommandStatus, node: NodeId) -> Self {
        Self {
            header: TelemetryHeader::new(
                sequence,
                TelemetryType::SetNode,
                status,
            ),
            node,
            reason: Reason::default(),
        }
    }

    /// And why it was refused.
    pub fn because(self, reason: Reason) -> Self {
        Self { reason, ..self }
    }
}

/// The longest reason a refusal carries, in bytes.
///
/// Room for what the vehicle says of a refused SET_NODE, which is a sentence
/// naming the node and the attribute and what to do: one longer is cut, at a
/// character, and ends `...`. The whole of it is in the vehicle's log.
pub const REASON_BYTES: usize = 160;

/// Text of at most [`REASON_BYTES`], held in place: why a command was refused.
///
/// Held rather than a `String` so that the vehicle makes an answer without
/// allocating, as it reads a command: a reason is written into one with
/// `write!`, and what does not fit is cut. A string on the wire.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Reason {
    bytes: [u8; REASON_BYTES],
    len: u8,
    /// Whether it ends in the mark, after which nothing more is written.
    cut: bool,
}

impl Default for Reason {
    fn default() -> Reason {
        Reason {
            bytes: [0; REASON_BYTES],
            len: 0,
            cut: false,
        }
    }
}

impl Reason {
    /// This text, cut to fit where it does not: see [`REASON_BYTES`].
    pub fn of(text: &str) -> Reason {
        let mut reason = Reason::default();
        let _ = std::fmt::Write::write_str(&mut reason, text);
        reason
    }

    pub fn as_str(&self) -> &str {
        // Written only whole characters at a time: see `write_str`.
        std::str::from_utf8(&self.bytes[..usize::from(self.len)])
            .unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Written into with `write!`, which is how the vehicle makes one: whatever
/// does not fit is cut at a character, and the reason ends `...` so that a
/// reader knows it was. Never an error, so a reason being long is never what
/// keeps an answer from going.
impl std::fmt::Write for Reason {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        const MARK: &str = "...";
        if self.cut {
            return Ok(());
        }
        let len = usize::from(self.len);
        if len + text.len() <= REASON_BYTES {
            self.bytes[len..len + text.len()].copy_from_slice(text.as_bytes());
            self.len = (len + text.len()) as u8;
            return Ok(());
        }

        // What is held and as much of this as leaves room for the mark,
        // ending on a character either way: where what is held already
        // reaches past that, it is what gives way.
        let keep = REASON_BYTES - MARK.len();
        let mut end = len.min(keep);
        while !self.as_str().is_char_boundary(end) {
            end -= 1;
        }
        let mut take = (keep - end).min(text.len());
        while !text.is_char_boundary(take) {
            take -= 1;
        }
        self.bytes[end..end + take].copy_from_slice(&text.as_bytes()[..take]);
        self.bytes[end + take..end + take + MARK.len()]
            .copy_from_slice(MARK.as_bytes());
        self.len = (end + take + MARK.len()) as u8;
        self.cut = true;
        Ok(())
    }
}

impl std::fmt::Debug for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The answer to a QUERY_NODE: what that node has moved, and whether it is
/// running
///
/// Both, because they are read together: counts that have stopped going up
/// mean one thing for a node the ground disabled and another for one that is
/// running, and an operator who had to ask twice to tell them apart would be
/// asking twice every time.
///
/// A command about a disabled node is answered rather than ignored, so this
/// is what a disabled node answers: its counters, which survive a disable,
/// and `false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryNodeTelemetry {
    pub header: TelemetryHeader,
    /// The node the command named.
    pub node: NodeId,
    /// Whether the pass may touch it.
    pub enabled: bool,
    /// What it has moved since the vehicle started.
    pub counts: NodeCounts,
}

impl QueryNodeTelemetry {
    /// Makes the answer to a QUERY_NODE, made now.
    ///
    /// # Arguments
    ///
    /// * `sequence` - The sequence number of the command it answers.
    /// * `status`   - How the command went.
    /// * `node`     - The node the command named.
    /// * `enabled`  - Whether the pass may touch it.
    /// * `counts`   - What it has moved since the vehicle started.
    ///
    /// # Returns
    ///
    /// The answer.
    pub fn new(
        sequence: u32,
        status: CommandStatus,
        node: NodeId,
        enabled: bool,
        counts: NodeCounts,
    ) -> Self {
        Self {
            header: TelemetryHeader::new(
                sequence,
                TelemetryType::QueryNode,
                status,
            ),
            node,
            enabled,
            counts,
        }
    }

    /// The answer to a query about a node this configuration has not got.
    ///
    /// The id that was asked about, no counts, and not enabled: a node that
    /// does not exist is not running, and an answer that said it was would be
    /// worse than the refusal beside it.
    pub fn refused(sequence: u32, status: CommandStatus, node: NodeId) -> Self {
        Self::new(sequence, status, node, false, NodeCounts::default())
    }
}

/// The answer to a QUERY_NODE_SAMPLE: the last datum that node moved each way
///
/// For an operator who wants to see the bytes rather than a count of them. A
/// direction that has carried nothing has no time, which is not the same as
/// one that carried a transfer of no bytes: see [`DHSample`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryNodeSampleTelemetry {
    pub header: TelemetryHeader,
    /// The node the command named.
    pub node: NodeId,
    /// The last datum the vehicle took from it.
    pub taken: DHSample,
    /// And the last it gave it.
    pub given: DHSample,
}

impl QueryNodeSampleTelemetry {
    /// Makes the answer to a QUERY_NODE_SAMPLE, made now.
    ///
    /// # Arguments
    ///
    /// * `sequence` - The sequence number of the command it answers.
    /// * `status`   - How the command went.
    /// * `node`     - The node the command named.
    /// * `taken`    - The last datum the vehicle took from it.
    /// * `given`    - The last datum the vehicle gave it.
    ///
    /// # Returns
    ///
    /// The answer.
    pub fn new(
        sequence: u32,
        status: CommandStatus,
        node: NodeId,
        taken: DHSample,
        given: DHSample,
    ) -> Self {
        Self {
            header: TelemetryHeader::new(
                sequence,
                TelemetryType::QueryNodeSample,
                status,
            ),
            node,
            taken,
            given,
        }
    }
}

/// Makes an answer of this status to a command of that kind.
///
/// The telemetry type has to match the command, because that is how the ground
/// pairs an answer with what it asked: a refusal of the wrong type is an
/// answer the ground cannot place, and the command then looks unanswered --
/// which is the one thing a refusal is for saying it is not.
///
/// For the answers that carry nothing but a status. The ones that carry
/// measurements -- a handler's statistics, a handler's last transfers -- are
/// given empty ones here: a refused command measured nothing.
///
/// # Arguments
///
/// * `command` - The command answered.
/// * `status`  - How it went.
///
/// # Returns
///
/// The answer, of the type that pairs with the command.
pub fn answer_to(
    command: &crate::commands::Command,
    status: CommandStatus,
) -> Telemetry {
    use crate::commands::Command;

    let sequence = command.sequence();
    match command {
        Command::Ping(_) => {
            Telemetry::Ping(PingTelemetry::new(sequence, status))
        }
        // A CONNECT is answered where it belongs -- see the command
        // interpreter -- so the one that comes through here is a refused one,
        // and the status is the whole of what it says. The build is this
        // one's, which is all a refusal knows.
        Command::Connect(_) => Telemetry::Connect(
            ConnectTelemetry::new(
                sequence,
                ConfigVersion::of_this_build(),
                ConfigDigest([0u8; 16]),
                0,
            )
            .refused(status),
        ),
        Command::RestartArm(_) => {
            Telemetry::RestartArm(RestartArmTelemetry::new(sequence, status))
        }
        Command::Restart(_) => {
            Telemetry::Restart(RestartTelemetry::new(sequence, status))
        }
        // The id the command carried, refused or not: an answer about a node
        // says which node, and an id out of range is exactly the case where
        // the ground needs to see which one it sent.
        Command::StartNode(cmd) => Telemetry::StartNode(
            NodeTelemetry::started(sequence, status, cmd.node),
        ),
        Command::StopNode(cmd) => Telemetry::StopNode(NodeTelemetry::stopped(
            sequence, status, cmd.node,
        )),
        Command::SetNode(cmd) => Telemetry::SetNode(SetNodeTelemetry::new(
            sequence, status, cmd.node,
        )),
        // A refused query measured nothing, so it carries no counts and no
        // samples: the status is the whole of what it says.
        Command::QueryNode(cmd) => Telemetry::QueryNode(
            QueryNodeTelemetry::refused(sequence, status, cmd.node),
        ),
        Command::QueryNodeSample(cmd) => {
            Telemetry::QueryNodeSample(QueryNodeSampleTelemetry::new(
                sequence,
                status,
                cmd.node,
                DHSample::new(),
                DHSample::new(),
            ))
        }
    }
}

/// Union of all telemetry types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Telemetry {
    Ping(PingTelemetry),
    Connect(ConnectTelemetry),
    RestartArm(RestartArmTelemetry),
    Restart(RestartTelemetry),
    StartNode(NodeTelemetry),
    StopNode(NodeTelemetry),
    SetNode(SetNodeTelemetry),
    QueryNode(QueryNodeTelemetry),
    QueryNodeSample(QueryNodeSampleTelemetry),
}

impl Telemetry {
    /// Gives the header every answer starts with.
    ///
    /// # Returns
    ///
    /// This answer's header.
    pub fn header(&self) -> &TelemetryHeader {
        match self {
            Telemetry::Ping(tm) => &tm.header,
            Telemetry::Connect(tm) => &tm.header,
            Telemetry::RestartArm(tm) => &tm.header,
            Telemetry::Restart(tm) => &tm.header,
            Telemetry::StartNode(tm) | Telemetry::StopNode(tm) => &tm.header,
            Telemetry::SetNode(tm) => &tm.header,
            Telemetry::QueryNode(tm) => &tm.header,
            Telemetry::QueryNodeSample(tm) => &tm.header,
        }
    }

    /// Gives the header every answer starts with, to change.
    ///
    /// # Returns
    ///
    /// This answer's header.
    fn header_mut(&mut self) -> &mut TelemetryHeader {
        match self {
            Telemetry::Ping(tm) => &mut tm.header,
            Telemetry::Connect(tm) => &mut tm.header,
            Telemetry::RestartArm(tm) => &mut tm.header,
            Telemetry::Restart(tm) => &mut tm.header,
            Telemetry::StartNode(tm) | Telemetry::StopNode(tm) => {
                &mut tm.header
            }
            Telemetry::SetNode(tm) => &mut tm.header,
            Telemetry::QueryNode(tm) => &mut tm.header,
            Telemetry::QueryNodeSample(tm) => &mut tm.header,
        }
    }

    pub fn sequence(&self) -> u32 {
        self.header().sequence
    }

    /// The generation that made this answer: see
    /// [`TelemetryHeader::generation`].
    pub fn generation(&self) -> u32 {
        self.header().generation
    }

    /// This answer, made by `generation`.
    pub fn for_generation(mut self, generation: u32) -> Self {
        self.header_mut().generation = generation;
        self
    }

    /// Gives this answer as the answer to the command numbered `sequence`.
    ///
    /// For a stand-in vehicle, which answers whatever it is asked: the
    /// ground takes an answer only for the command whose sequence it
    /// carries.
    ///
    /// # Arguments
    ///
    /// * `sequence` - The sequence number of the command it answers.
    ///
    /// # Returns
    ///
    /// The answer, carrying that sequence number.
    pub fn for_sequence(mut self, sequence: u32) -> Self {
        self.header_mut().sequence = sequence;
        self
    }

    pub fn tm_type(&self) -> TelemetryType {
        self.header().tm_type
    }

    pub fn status(&self) -> CommandStatus {
        self.header().status
    }

    /// When the spacecraft made this answer.
    ///
    /// Every kind carries it, in the header; see [`TelemetryHeader`].
    pub fn timestamp(&self) -> Timestamp {
        self.header().timestamp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reason is kept whole where it fits, and cut at a character where it
    /// does not, ending in a mark that says so -- whether it arrives in one
    /// piece or in several, as `write!` hands it over.
    #[test]
    fn a_reason_is_cut_to_fit_at_a_character_and_says_so() {
        use std::fmt::Write;

        assert_eq!(
            Reason::of("no binding names it").as_str(),
            "no binding names it"
        );
        assert!(Reason::default().is_empty());

        let long = "x".repeat(REASON_BYTES + 40);
        let cut = Reason::of(&long);
        assert_eq!(cut.as_str().len(), REASON_BYTES);
        assert!(cut.as_str().ends_with("..."), "{cut}");

        // A character of more than one byte where the cut falls is left out
        // whole rather than split.
        let wide = format!("{}é and more", "x".repeat(REASON_BYTES - 4));
        let cut = Reason::of(&wide);
        assert!(cut.as_str().len() <= REASON_BYTES);
        assert!(cut.as_str().ends_with("..."), "{cut}");

        // In pieces: what was held gives way to the mark, and nothing is
        // written after it.
        let mut pieces = Reason::default();
        write!(pieces, "{}", "y".repeat(REASON_BYTES - 1)).unwrap();
        write!(pieces, "zz").unwrap();
        write!(pieces, "more").unwrap();
        assert_eq!(pieces.as_str().len(), REASON_BYTES);
        assert!(pieces.as_str().ends_with("y..."), "{pieces}");
    }

    /// A SET_NODE's answer carries its reason as text, and one with none
    /// reads as one with none.
    #[test]
    fn a_set_node_answer_carries_its_reason() {
        let refused = Telemetry::SetNode(
            SetNodeTelemetry::new(
                7,
                CommandStatus::InvalidParameter,
                NodeId::of(2),
            )
            .because(Reason::of("it is spelled half_band")),
        );
        let packed = refused.to_bytes();
        assert!(
            packed
                .windows(b"it is spelled half_band".len())
                .any(|w| w == b"it is spelled half_band"),
            "the reason is on the link as its text"
        );
        assert_eq!(Telemetry::read(&packed), Ok(refused));

        let without = Telemetry::SetNode(SetNodeTelemetry::new(
            7,
            CommandStatus::Success,
            NodeId::of(2),
        ));
        match Telemetry::read(&without.to_bytes()) {
            Ok(Telemetry::SetNode(tm)) => assert!(tm.reason.is_empty()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn test_telemetry_type_conversion() {
        let tm_type = TelemetryType::Ping;
        assert_eq!(tm_type.to_u8(), 0x81);
        assert_eq!(TelemetryType::from_u8(0x81), Some(TelemetryType::Ping));
    }

    #[test]
    fn test_ping_telemetry() {
        let tm = PingTelemetry::new(1, CommandStatus::Success);
        assert_eq!(tm.header.sequence, 1);
        assert_eq!(tm.header.status, CommandStatus::Success);
    }

    /// Every kind of response says when it was made.
    ///
    /// Two of them used to carry a time of their own and the other eight
    /// carried none, which is the sort of thing a reader of the eight finds
    /// out by needing it. The time is in the header now, and the header is
    /// built one way, so a kind added later cannot be the ninth.
    #[test]
    fn every_response_says_when_it_was_made() {
        let version = ConfigVersion::of_this_build();
        let digest = ConfigDigest([0u8; 16]);
        let before = Timestamp::now();

        let answers = [
            Telemetry::Ping(PingTelemetry::new(1, CommandStatus::Success)),
            Telemetry::Connect(ConnectTelemetry::new(2, version, digest, 1)),
            Telemetry::RestartArm(RestartArmTelemetry::new(
                3,
                CommandStatus::Success,
            )),
            Telemetry::Restart(RestartTelemetry::new(
                4,
                CommandStatus::Success,
            )),
            Telemetry::StartNode(NodeTelemetry::started(
                11,
                CommandStatus::Success,
                NodeId::of(0),
            )),
            Telemetry::StopNode(NodeTelemetry::stopped(
                12,
                CommandStatus::NotFound,
                NodeId::of(99),
            )),
            Telemetry::SetNode(SetNodeTelemetry::new(
                13,
                CommandStatus::Success,
                NodeId::of(4),
            )),
            Telemetry::QueryNode(QueryNodeTelemetry::new(
                14,
                CommandStatus::Success,
                NodeId::of(1),
                true,
                NodeCounts::default(),
            )),
            Telemetry::QueryNodeSample(QueryNodeSampleTelemetry::new(
                15,
                CommandStatus::Success,
                NodeId::of(1),
                DHSample::new(),
                DHSample::new(),
            )),
        ];

        // Every kind there is, which is what makes this a statement about all
        // of them rather than about the ones that were remembered.
        assert_eq!(
            answers.len(),
            (0u8..=255)
                .filter(|byte| TelemetryType::from_u8(*byte).is_some())
                .count(),
            "a kind of telemetry was left out of this test"
        );

        let after = Timestamp::now();
        for answer in answers {
            let at = answer.timestamp();
            assert!(
                at >= before && at <= after,
                "{:?} was not stamped as this test ran: {at:?}",
                answer.tm_type()
            );
        }
    }
}
