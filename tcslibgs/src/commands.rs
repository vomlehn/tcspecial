//! Command definitions for TCSpecial
//!
//! Commands are sent from ground to space and are idempotent.

use serde::{Deserialize, Serialize};
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
    RestartArm,
    Restart,
    StartDH,
    StopDH,
    QueryDH,
    Config,
    ConfigDH,
}

impl CommandType {
    pub fn to_u8(&self) -> u8 {
        match self {
            CommandType::Ping => 0x01,
            CommandType::RestartArm => 0x02,
            CommandType::Restart => 0x03,
            CommandType::StartDH => 0x10,
            CommandType::StopDH => 0x11,
            CommandType::QueryDH => 0x12,
            CommandType::Config => 0x20,
            CommandType::ConfigDH => 0x21,
        }
    }

    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(CommandType::Ping),
            0x02 => Some(CommandType::RestartArm),
            0x03 => Some(CommandType::Restart),
            0x10 => Some(CommandType::StartDH),
            0x11 => Some(CommandType::StopDH),
            0x12 => Some(CommandType::QueryDH),
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
    RestartArm(RestartArmCommand),
    Restart(RestartCommand),
    StartDH(StartDHCommand),
    StopDH(StopDHCommand),
    QueryDH(QueryDHCommand),
    Config(ConfigCommand),
    ConfigDH(ConfigDHCommand),
}

impl Command {
    pub fn sequence(&self) -> u32 {
        match self {
            Command::Ping(cmd) => cmd.header.sequence,
            Command::RestartArm(cmd) => cmd.header.sequence,
            Command::Restart(cmd) => cmd.header.sequence,
            Command::StartDH(cmd) => cmd.header.sequence,
            Command::StopDH(cmd) => cmd.header.sequence,
            Command::QueryDH(cmd) => cmd.header.sequence,
            Command::Config(cmd) => cmd.header.sequence,
            Command::ConfigDH(cmd) => cmd.header.sequence,
        }
    }

    pub fn cmd_type(&self) -> CommandType {
        match self {
            Command::Ping(cmd) => cmd.header.cmd_type,
            Command::RestartArm(cmd) => cmd.header.cmd_type,
            Command::Restart(cmd) => cmd.header.cmd_type,
            Command::StartDH(cmd) => cmd.header.cmd_type,
            Command::StopDH(cmd) => cmd.header.cmd_type,
            Command::QueryDH(cmd) => cmd.header.cmd_type,
            Command::Config(cmd) => cmd.header.cmd_type,
            Command::ConfigDH(cmd) => cmd.header.cmd_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
