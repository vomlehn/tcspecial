//! Telemetry definitions for TCSpecial
//!
//! Telemetry is sent from space to ground.

use serde::{Deserialize, Serialize};

use crate::config_digest::{ConfigDigest, ConfigVersion};
use crate::types::{CommandStatus, DHName, DHSample, DHState, NodeId, Statistics, Timestamp};

/// Telemetry message header
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelemetryHeader {
    /// Sequence number matching the command that generated this response
    pub sequence: u32,
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
    /// A header for an answer being made now.
    ///
    /// The only way one is built, so that a telemetry type added later cannot
    /// forget the time: there is nowhere to leave it out.
    pub fn new(sequence: u32, tm_type: TelemetryType, status: CommandStatus) -> Self {
        Self {
            sequence,
            tm_type,
            status,
            timestamp: Timestamp::now(),
        }
    }
}

/// Telemetry types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TelemetryType {
    Ping,
    Connect,
    RestartArm,
    Restart,
    StartDH,
    StopDH,
    QueryDH,
    QueryDHSample,
    Config,
    ConfigDH,
    /// The answer to a START_NODE: see [`NodeTelemetry`].
    StartNode,
    /// And to a STOP_NODE.
    StopNode,
    Beacon,
}

impl TelemetryType {
    pub fn to_u8(&self) -> u8 {
        match self {
            TelemetryType::Ping => 0x81,
            TelemetryType::Connect => 0x84,
            TelemetryType::RestartArm => 0x82,
            TelemetryType::Restart => 0x83,
            TelemetryType::StartDH => 0x90,
            TelemetryType::StopDH => 0x91,
            TelemetryType::QueryDH => 0x92,
            TelemetryType::QueryDHSample => 0x93,
            TelemetryType::Config => 0xA0,
            TelemetryType::ConfigDH => 0xA1,
            TelemetryType::StartNode => 0x30,
            TelemetryType::StopNode => 0x31,
            TelemetryType::Beacon => 0xF0,
        }
    }

    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x81 => Some(TelemetryType::Ping),
            0x84 => Some(TelemetryType::Connect),
            0x82 => Some(TelemetryType::RestartArm),
            0x83 => Some(TelemetryType::Restart),
            0x90 => Some(TelemetryType::StartDH),
            0x91 => Some(TelemetryType::StopDH),
            0x92 => Some(TelemetryType::QueryDH),
            0x93 => Some(TelemetryType::QueryDHSample),
            0xA0 => Some(TelemetryType::Config),
            0xA1 => Some(TelemetryType::ConfigDH),
            0x30 => Some(TelemetryType::StartNode),
            0x31 => Some(TelemetryType::StopNode),
            0xF0 => Some(TelemetryType::Beacon),
            _ => None,
        }
    }
}

/// PING telemetry response
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RestartArmTelemetry {
    pub header: TelemetryHeader,
}

impl RestartArmTelemetry {
    pub fn new(sequence: u32, status: CommandStatus) -> Self {
        Self {
            header: TelemetryHeader::new(sequence, TelemetryType::RestartArm, status),
        }
    }
}

/// RESTART telemetry response
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RestartTelemetry {
    pub header: TelemetryHeader,
}

impl RestartTelemetry {
    pub fn new(sequence: u32, status: CommandStatus) -> Self {
        Self {
            header: TelemetryHeader::new(sequence, TelemetryType::Restart, status),
        }
    }
}

/// START_DH telemetry response
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectTelemetry {
    pub header: TelemetryHeader,
    pub version: ConfigVersion,
    /// The version the tcspecial configuration file states, which the beacon
    /// carries too: a ground station reading the answer is told which set
    /// this end is serving and not only which build is serving it.
    pub config_version: ConfigVersion,
    pub digest: ConfigDigest,
}

impl ConnectTelemetry {
    pub fn new(
        sequence: u32,
        version: ConfigVersion,
        config_version: ConfigVersion,
        digest: ConfigDigest,
    ) -> Self {
        Self {
            header: TelemetryHeader::new(
                sequence,
                TelemetryType::Connect,
                CommandStatus::Success,
            ),
            version,
            config_version,
            digest,
        }
    }

    /// The same answer, with a status other than success.
    ///
    /// For a CONNECT that is refused rather than answered -- one that arrived
    /// on a payload's link, which is not what a connection to the vehicle is
    /// asked on. The three facts are this end's all the same, so the ground
    /// is told what it asked as well as that it asked in the wrong place.
    pub fn refused(self, status: CommandStatus) -> Self {
        Self {
            header: TelemetryHeader::new(self.header.sequence, TelemetryType::Connect, status),
            ..self
        }
    }
}

/// START_DH telemetry
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartDHTelemetry {
    pub header: TelemetryHeader,
}

impl StartDHTelemetry {
    pub fn new(sequence: u32, status: CommandStatus) -> Self {
        Self {
            header: TelemetryHeader::new(sequence, TelemetryType::StartDH, status),
        }
    }
}

/// STOP_DH telemetry response
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct StopDHTelemetry {
    pub header: TelemetryHeader,
}

impl StopDHTelemetry {
    pub fn new(sequence: u32, status: CommandStatus) -> Self {
        Self {
            header: TelemetryHeader::new(sequence, TelemetryType::StopDH, status),
        }
    }
}

/// QUERY_DH telemetry response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueryDHTelemetry {
    pub header: TelemetryHeader,
    pub name: DHName,
    /// What the handler is doing, which nothing else on the link says.
    ///
    /// `None` when there is no such handler, which is the NotFound answer: a
    /// state would have to be invented for a handler that does not exist, and
    /// a ground station reading one would show it.
    ///
    /// The ground once had only its own commands to go by, so a panel showed
    /// what the last START_DH or STOP_DH implied and went on showing it --
    /// through a handler that stopped on the spacecraft, and through a MOC
    /// attached to a tcspecial serving another payload set.
    #[serde(default)]
    pub state: Option<DHState>,
    pub statistics: Statistics,
}

impl QueryDHTelemetry {
    pub fn new(
        sequence: u32,
        status: CommandStatus,
        name: DHName,
        state: Option<DHState>,
        statistics: Statistics,
    ) -> Self {
        Self {
            header: TelemetryHeader::new(sequence, TelemetryType::QueryDH, status),
            name,
            state,
            statistics,
        }
    }
}

/// QUERY_DH_SAMPLE telemetry response
///
/// Carries what a data handler last sent and last received, each with the time
/// it happened. Either may be empty, which is what a handler that has moved
/// nothing in that direction looks like -- distinct from a handler that has
/// moved zero bytes, which cannot happen.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueryDHSampleTelemetry {
    pub header: TelemetryHeader,
    pub name: DHName,
    pub sent: DHSample,
    pub received: DHSample,
}

impl QueryDHSampleTelemetry {
    pub fn new(
        sequence: u32,
        status: CommandStatus,
        name: DHName,
        sent: DHSample,
        received: DHSample,
    ) -> Self {
        Self {
            header: TelemetryHeader::new(sequence, TelemetryType::QueryDHSample, status),
            name,
            sent,
            received,
        }
    }
}

/// CONFIG telemetry response
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigTelemetry {
    pub header: TelemetryHeader,
}

impl ConfigTelemetry {
    pub fn new(sequence: u32, status: CommandStatus) -> Self {
        Self {
            header: TelemetryHeader::new(sequence, TelemetryType::Config, status),
        }
    }
}

/// CONFIG_DH telemetry response
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigDHTelemetry {
    pub header: TelemetryHeader,
}

impl ConfigDHTelemetry {
    pub fn new(sequence: u32, status: CommandStatus) -> Self {
        Self {
            header: TelemetryHeader::new(sequence, TelemetryType::ConfigDH, status),
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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

/// BEACON asynchronous telemetry
///
/// It carries what a CONNECT response carries -- the build and the digest of
/// the configuration this process read -- because a beacon arrives whether or
/// not anything has connected. A ground station that is only listening can
/// then see which software is flying and which payload set it is serving,
/// rather than having to ask before it can know.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct BeaconTelemetry {
    pub header: TelemetryHeader,
    /// The version of this build, from its `Cargo.toml`.
    pub version: ConfigVersion,
    /// The version the configuration file states, as its three decimal parts.
    ///
    /// The file's own, not this build's: a payload set is versioned by
    /// whoever writes it, and a ground station reading a beacon wants to know
    /// which set it is hearing from as well as which software.
    pub config_version: ConfigVersion,
    /// The digest of the configuration file this process read.
    pub digest: ConfigDigest,
}

impl BeaconTelemetry {
    /// `sequence` counts the beacons this process has sent, from one.
    ///
    /// Every other telemetry's sequence number is the command's, which is how
    /// a ground station pairs an answer with what it asked. A beacon answers
    /// nothing, so the number is the beacon's own: it says which beacon this
    /// is, and a ground station that sees 7 after 5 knows it missed one
    /// rather than wondering whether the spacecraft went quiet.
    ///
    /// Counted by the sender, so it starts again at one when tcspecial does.
    /// A number going backwards is therefore a restart, which is worth
    /// knowing and is not the same thing as a gap.
    pub fn new(
        sequence: u32,
        version: ConfigVersion,
        config_version: ConfigVersion,
        digest: ConfigDigest,
    ) -> Self {
        Self {
            header: TelemetryHeader::new(
                sequence,
                TelemetryType::Beacon,
                CommandStatus::Success,
            ),
            version,
            config_version,
            digest,
        }
    }
}

/// An answer of this status to a command of that kind.
///
/// The telemetry type has to match the command, because that is how the ground
/// pairs an answer with what it asked: a refusal of the wrong type is an
/// answer the ground cannot place, and the command then looks unanswered --
/// which is the one thing a refusal is for saying it is not.
///
/// For the answers that carry nothing but a status. The ones that carry
/// measurements -- a handler's statistics, a handler's last transfers -- are
/// given empty ones here: a refused command measured nothing.
pub fn answer_to(command: &crate::commands::Command, status: CommandStatus) -> Telemetry {
    use crate::commands::Command;

    let sequence = command.sequence();
    match command {
        Command::Ping(_) => Telemetry::Ping(PingTelemetry::new(sequence, status)),
        // A CONNECT is answered where it belongs -- see the command
        // interpreter -- so the one that comes through here is a refused one,
        // and the status is the whole of what it says. The three facts are
        // this build's, which is all a refusal knows.
        Command::Connect(_) => Telemetry::Connect(
            ConnectTelemetry::new(
                sequence,
                ConfigVersion::of_this_build(),
                ConfigVersion::of_this_build(),
                ConfigDigest([0u8; 16]),
            )
            .refused(status),
        ),
        Command::RestartArm(_) => {
            Telemetry::RestartArm(RestartArmTelemetry::new(sequence, status))
        }
        Command::Restart(_) => Telemetry::Restart(RestartTelemetry::new(sequence, status)),
        Command::StartDH(_) => Telemetry::StartDH(StartDHTelemetry::new(sequence, status)),
        Command::StopDH(_) => Telemetry::StopDH(StopDHTelemetry::new(sequence, status)),
        Command::QueryDH(cmd) => Telemetry::QueryDH(QueryDHTelemetry::new(
            sequence,
            status,
            cmd.name.clone(),
            None,
            Statistics::new(),
        )),
        Command::QueryDHSample(cmd) => Telemetry::QueryDHSample(QueryDHSampleTelemetry::new(
            sequence,
            status,
            cmd.name.clone(),
            DHSample::new(),
            DHSample::new(),
        )),
        Command::Config(_) => Telemetry::Config(ConfigTelemetry::new(sequence, status)),
        Command::ConfigDH(_) => Telemetry::ConfigDH(ConfigDHTelemetry::new(sequence, status)),
        // The id the command carried, refused or not: an answer about a node
        // says which node, and an id out of range is exactly the case where
        // the ground needs to see which one it sent.
        Command::StartNode(cmd) => {
            Telemetry::StartNode(NodeTelemetry::started(sequence, status, cmd.node))
        }
        Command::StopNode(cmd) => {
            Telemetry::StopNode(NodeTelemetry::stopped(sequence, status, cmd.node))
        }
    }
}

/// Union of all telemetry types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Telemetry {
    Ping(PingTelemetry),
    Connect(ConnectTelemetry),
    RestartArm(RestartArmTelemetry),
    Restart(RestartTelemetry),
    StartDH(StartDHTelemetry),
    StopDH(StopDHTelemetry),
    QueryDH(QueryDHTelemetry),
    QueryDHSample(QueryDHSampleTelemetry),
    Config(ConfigTelemetry),
    ConfigDH(ConfigDHTelemetry),
    StartNode(NodeTelemetry),
    StopNode(NodeTelemetry),
    Beacon(BeaconTelemetry),
}

impl Telemetry {
    pub fn sequence(&self) -> u32 {
        match self {
            Telemetry::Ping(tm) => tm.header.sequence,
            Telemetry::Connect(tm) => tm.header.sequence,
            Telemetry::RestartArm(tm) => tm.header.sequence,
            Telemetry::Restart(tm) => tm.header.sequence,
            Telemetry::StartDH(tm) => tm.header.sequence,
            Telemetry::StopDH(tm) => tm.header.sequence,
            Telemetry::QueryDH(tm) => tm.header.sequence,
            Telemetry::QueryDHSample(tm) => tm.header.sequence,
            Telemetry::Config(tm) => tm.header.sequence,
            Telemetry::ConfigDH(tm) => tm.header.sequence,
            Telemetry::StartNode(tm) | Telemetry::StopNode(tm) => tm.header.sequence,
            Telemetry::Beacon(tm) => tm.header.sequence,
        }
    }

    pub fn tm_type(&self) -> TelemetryType {
        match self {
            Telemetry::Ping(tm) => tm.header.tm_type,
            Telemetry::Connect(tm) => tm.header.tm_type,
            Telemetry::RestartArm(tm) => tm.header.tm_type,
            Telemetry::Restart(tm) => tm.header.tm_type,
            Telemetry::StartDH(tm) => tm.header.tm_type,
            Telemetry::StopDH(tm) => tm.header.tm_type,
            Telemetry::QueryDH(tm) => tm.header.tm_type,
            Telemetry::QueryDHSample(tm) => tm.header.tm_type,
            Telemetry::Config(tm) => tm.header.tm_type,
            Telemetry::ConfigDH(tm) => tm.header.tm_type,
            Telemetry::StartNode(tm) | Telemetry::StopNode(tm) => tm.header.tm_type,
            Telemetry::Beacon(tm) => tm.header.tm_type,
        }
    }

    pub fn status(&self) -> CommandStatus {
        match self {
            Telemetry::Ping(tm) => tm.header.status,
            Telemetry::Connect(tm) => tm.header.status,
            Telemetry::RestartArm(tm) => tm.header.status,
            Telemetry::Restart(tm) => tm.header.status,
            Telemetry::StartDH(tm) => tm.header.status,
            Telemetry::StopDH(tm) => tm.header.status,
            Telemetry::QueryDH(tm) => tm.header.status,
            Telemetry::QueryDHSample(tm) => tm.header.status,
            Telemetry::Config(tm) => tm.header.status,
            Telemetry::ConfigDH(tm) => tm.header.status,
            Telemetry::StartNode(tm) | Telemetry::StopNode(tm) => tm.header.status,
            Telemetry::Beacon(tm) => tm.header.status,
        }
    }

    /// When the spacecraft made this answer.
    ///
    /// Every kind carries it, in the header; see [`TelemetryHeader`].
    pub fn timestamp(&self) -> Timestamp {
        match self {
            Telemetry::Ping(tm) => tm.header.timestamp,
            Telemetry::Connect(tm) => tm.header.timestamp,
            Telemetry::RestartArm(tm) => tm.header.timestamp,
            Telemetry::Restart(tm) => tm.header.timestamp,
            Telemetry::StartDH(tm) => tm.header.timestamp,
            Telemetry::StopDH(tm) => tm.header.timestamp,
            Telemetry::QueryDH(tm) => tm.header.timestamp,
            Telemetry::QueryDHSample(tm) => tm.header.timestamp,
            Telemetry::Config(tm) => tm.header.timestamp,
            Telemetry::ConfigDH(tm) => tm.header.timestamp,
            Telemetry::StartNode(tm) | Telemetry::StopNode(tm) => tm.header.timestamp,
            Telemetry::Beacon(tm) => tm.header.timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_beacon_telemetry() {
        let tm = BeaconTelemetry::new(
            7,
            ConfigVersion::of_this_build(),
            ConfigVersion::of_text("1.0").expect("a version"),
            ConfigDigest([0u8; 16]),
        );
        assert_eq!(tm.header.tm_type, TelemetryType::Beacon);
        // The beacon's own number, not a command's: a beacon answers nothing.
        assert_eq!(tm.header.sequence, 7);
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
            Telemetry::Connect(ConnectTelemetry::new(2, version, version, digest)),
            Telemetry::RestartArm(RestartArmTelemetry::new(3, CommandStatus::Success)),
            Telemetry::Restart(RestartTelemetry::new(4, CommandStatus::Success)),
            Telemetry::StartDH(StartDHTelemetry::new(5, CommandStatus::Success)),
            Telemetry::StopDH(StopDHTelemetry::new(6, CommandStatus::Success)),
            Telemetry::QueryDH(QueryDHTelemetry::new(
                7,
                CommandStatus::Success,
                DHName::new("DH0"),
                Some(DHState::Active),
                Statistics::new(),
            )),
            Telemetry::QueryDHSample(QueryDHSampleTelemetry::new(
                8,
                CommandStatus::Success,
                DHName::new("DH0"),
                DHSample::new(),
                DHSample::new(),
            )),
            Telemetry::Config(ConfigTelemetry::new(9, CommandStatus::Success)),
            Telemetry::ConfigDH(ConfigDHTelemetry::new(10, CommandStatus::Success)),
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
            Telemetry::Beacon(BeaconTelemetry::new(13, version, version, digest)),
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

    #[test]
    fn test_telemetry_serialization() {
        let tm = Telemetry::Ping(PingTelemetry::new(42, CommandStatus::Success));
        let json = serde_json::to_string(&tm).unwrap();
        let deserialized: Telemetry = serde_json::from_str(&json).unwrap();
        assert_eq!(tm.sequence(), deserialized.sequence());
    }
}
