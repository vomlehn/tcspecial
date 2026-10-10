//! Command definitions for TCSpecial
//!
//! Commands are sent from ground to space and are idempotent.

use serde::{Deserialize, Serialize};

use crate::config_digest::{ConfigDigest, ConfigVersion};
use crate::types::{ArmKey, BeaconTime, DHId, DHName, DHType};

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
    /// What each end reads, said at the start of a link.
    Connect,
    RestartArm,
    Restart,
    StartDH,
    StopDH,
    QueryDH,
    QueryDHSample,
    Config,
    ConfigDH,
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
    /// Either of them.
    Either,
}

impl CommandLink {
    /// The word a message uses for this link.
    pub fn spelling(&self) -> &'static str {
        match self {
            CommandLink::Spacecraft => "spacecraft command",
            CommandLink::Payload => "payload command",
            CommandLink::Either => "either command",
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
    /// CONNECT is on neither side of the question. It says what the end that
    /// answers read, and the holder of either link wants that of the link it
    /// holds, so both answer it.
    pub fn link(&self) -> CommandLink {
        match self {
            CommandType::Ping
            | CommandType::RestartArm
            | CommandType::Restart
            | CommandType::Config => CommandLink::Spacecraft,

            CommandType::StartDH
            | CommandType::StopDH
            | CommandType::QueryDH
            | CommandType::QueryDHSample
            | CommandType::ConfigDH => CommandLink::Payload,

            CommandType::Connect => CommandLink::Either,
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
        match (self.link(), link) {
            (CommandLink::Either, _) | (_, CommandLink::Either) => true,
            (wanted, arrived) => wanted == arrived,
        }
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

/// START_DH command - start a data handler
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartDHCommand {
    pub header: CommandHeader,
    pub dh_id: DHId,
    pub dh_type: DHType,
    pub name: DHName,
}

impl StartDHCommand {
    pub fn new(sequence: u32, dh_id: DHId, dh_type: DHType, name: DHName) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::StartDH,
            },
            dh_id,
            dh_type,
            name,
        }
    }
}

/// STOP_DH command - stop a data handler
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct StopDHCommand {
    pub header: CommandHeader,
    pub dh_id: DHId,
}

impl StopDHCommand {
    pub fn new(sequence: u32, dh_id: DHId) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::StopDH,
            },
            dh_id,
        }
    }
}

/// QUERY_DH command - query statistics from a data handler
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueryDHCommand {
    pub header: CommandHeader,
    pub dh_id: DHId,
}

impl QueryDHCommand {
    pub fn new(sequence: u32, dh_id: DHId) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::QueryDH,
            },
            dh_id,
        }
    }
}

/// QUERY_DH_SAMPLE command - ask what a data handler last sent and received
///
/// Separate from QUERY_DH so that the statistics every poll asks for stay the
/// size they are. A sample is wanted only while someone is looking at a
/// panel, and carrying payload bytes in routine telemetry would spend downlink
/// on data nobody reads.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueryDHSampleCommand {
    pub header: CommandHeader,
    pub dh_id: DHId,
}

impl QueryDHSampleCommand {
    pub fn new(sequence: u32, dh_id: DHId) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::QueryDHSample,
            },
            dh_id,
        }
    }
}

/// CONFIG command - configure TCSpecial values
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigCommand {
    pub header: CommandHeader,
    pub beacon_interval: BeaconTime,
}

impl ConfigCommand {
    pub fn new(sequence: u32, beacon_interval: BeaconTime) -> Self {
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigDHCommand {
    pub header: CommandHeader,
    pub dh_id: DHId,
    // Additional configuration fields can be added here
}

impl ConfigDHCommand {
    pub fn new(sequence: u32, dh_id: DHId) -> Self {
        Self {
            header: CommandHeader {
                sequence,
                cmd_type: CommandType::ConfigDH,
            },
            dh_id,
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
        }
    }

    /// Which link this command belongs on; see [`CommandType::link`].
    pub fn link(&self) -> CommandLink {
        self.cmd_type().link()
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
            .chain([CommandType::Connect].iter())
            .map(|kind| kind.to_u8())
            .collect();
        all.sort_unstable();
        let mut known: Vec<u8> = (0u8..=255)
            .filter(|byte| CommandType::from_u8(*byte).is_some())
            .collect();
        known.sort_unstable();
        assert_eq!(all, known, "a command was left out of both lists");
    }

    /// A CONNECT is answered on either link.
    ///
    /// It says what the end that answers read, and the holder of either link
    /// wants that of the link it holds.
    #[test]
    fn what_each_end_read_is_asked_on_either_link() {
        assert_eq!(CommandType::Connect.link(), CommandLink::Either);
        assert!(CommandType::Connect.may_arrive_on(CommandLink::Spacecraft));
        assert!(CommandType::Connect.may_arrive_on(CommandLink::Payload));
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
