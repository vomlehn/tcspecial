//! Command definitions for TCSpecial
//!
//! Commands are sent from the ground to the spacecraft and must be idempotent,
//! generating the same resulting state and the same telemetry response.

use serde::{Deserialize, Serialize};
use crate::types::{ArmKey, BeaconTime, DHId, DHName, DHType};

/// Command message identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum CommandId {
    Ping = 1,
    RestartArm = 2,
    Restart = 3,
    StartDH = 4,
    StopDH = 5,
    QueryDH = 6,
    Config = 7,
    ConfigDH = 8,
}

/// PING command - verify that TCSpecial is able to process commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingCommand {
    pub sequence: u32,
}

impl PingCommand {
    pub fn new(sequence: u32) -> Self {
        Self { sequence }
    }
}

/// RESTART_ARM command - enable a restart for the next interval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartArmCommand {
    pub sequence: u32,
    /// Value that must match the RESTART command key
    pub arm_key: ArmKey,
}

impl RestartArmCommand {
    pub fn new(sequence: u32, arm_key: ArmKey) -> Self {
        Self { sequence, arm_key }
    }
}

/// RESTART command - restart TCSpecial if armed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartCommand {
    pub sequence: u32,
    /// Value that must match the RESTART_ARM command key
    pub arm_key: ArmKey,
}

impl RestartCommand {
    pub fn new(sequence: u32, arm_key: ArmKey) -> Self {
        Self { sequence, arm_key }
    }
}

/// START_DH command - start a data handler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartDHCommand {
    pub sequence: u32,
    /// Identifier to assign to the new DH
    pub dh_id: DHId,
    /// Type of DH (network or device)
    pub dh_type: DHType,
    /// Configuration name (server:port[:protocol] for network, device path for device)
    pub name: DHName,
}

impl StartDHCommand {
    pub fn new(sequence: u32, dh_id: DHId, dh_type: DHType, name: DHName) -> Self {
        Self { sequence, dh_id, dh_type, name }
    }
}

/// STOP_DH command - stop a data handler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopDHCommand {
    pub sequence: u32,
    /// ID of the DH to stop
    pub dh_id: DHId,
}

impl StopDHCommand {
    pub fn new(sequence: u32, dh_id: DHId) -> Self {
        Self { sequence, dh_id }
    }
}

/// QUERY_DH command - return statistics from a data handler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryDHCommand {
    pub sequence: u32,
    /// ID of the DH to query
    pub dh_id: DHId,
}

impl QueryDHCommand {
    pub fn new(sequence: u32, dh_id: DHId) -> Self {
        Self { sequence, dh_id }
    }
}

/// CONFIG command - configure various TCSpecial values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigCommand {
    pub sequence: u32,
    /// Interval at which BEACON telemetry is sent
    pub beacon_interval: Option<BeaconTime>,
}

impl ConfigCommand {
    pub fn new(sequence: u32) -> Self {
        Self { sequence, beacon_interval: None }
    }

    pub fn with_beacon_interval(mut self, interval: BeaconTime) -> Self {
        self.beacon_interval = Some(interval);
        self
    }
}

/// CONFIG_DH command - configure data handler values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDHCommand {
    pub sequence: u32,
    /// ID of the DH to configure
    pub dh_id: DHId,
    // Additional configuration fields can be added here
}

impl ConfigDHCommand {
    pub fn new(sequence: u32, dh_id: DHId) -> Self {
        Self { sequence, dh_id }
    }
}

/// Enumeration of all possible commands
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub fn id(&self) -> CommandId {
        match self {
            Command::Ping(_) => CommandId::Ping,
            Command::RestartArm(_) => CommandId::RestartArm,
            Command::Restart(_) => CommandId::Restart,
            Command::StartDH(_) => CommandId::StartDH,
            Command::StopDH(_) => CommandId::StopDH,
            Command::QueryDH(_) => CommandId::QueryDH,
            Command::Config(_) => CommandId::Config,
            Command::ConfigDH(_) => CommandId::ConfigDH,
        }
    }

    pub fn sequence(&self) -> u32 {
        match self {
            Command::Ping(c) => c.sequence,
            Command::RestartArm(c) => c.sequence,
            Command::Restart(c) => c.sequence,
            Command::StartDH(c) => c.sequence,
            Command::StopDH(c) => c.sequence,
            Command::QueryDH(c) => c.sequence,
            Command::Config(c) => c.sequence,
            Command::ConfigDH(c) => c.sequence,
        }
    }

    /// Serialize command to JSON bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize command from JSON bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_command_serialization() {
        let cmd = Command::Ping(PingCommand::new(1));
        let bytes = cmd.to_bytes().unwrap();
        let decoded = Command::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.sequence(), 1);
    }

    #[test]
    fn test_start_dh_command() {
        let cmd = StartDHCommand::new(
            1,
            DHId(0),
            DHType::Network,
            DHName::new("localhost:5000"),
        );
        assert_eq!(cmd.dh_id, DHId(0));
        assert_eq!(cmd.dh_type, DHType::Network);
    }
}
