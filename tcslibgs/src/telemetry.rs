//! Telemetry definitions for TCSpecial
//!
//! Telemetry is sent from space to ground.

use serde::{Deserialize, Serialize};
use crate::types::{CommandStatus, DHId, Statistics, Timestamp};

/// Telemetry message header
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelemetryHeader {
    /// Sequence number matching the command that generated this response
    pub sequence: u32,
    /// Telemetry type identifier
    pub tm_type: TelemetryType,
    /// Command status
    pub status: CommandStatus,
}

/// Telemetry types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TelemetryType {
    Ping,
    RestartArm,
    Restart,
    StartDH,
    StopDH,
    QueryDH,
    Config,
    ConfigDH,
    Beacon,
}

impl TelemetryType {
    pub fn to_u8(&self) -> u8 {
        match self {
            TelemetryType::Ping => 0x81,
            TelemetryType::RestartArm => 0x82,
            TelemetryType::Restart => 0x83,
            TelemetryType::StartDH => 0x90,
            TelemetryType::StopDH => 0x91,
            TelemetryType::QueryDH => 0x92,
            TelemetryType::Config => 0xA0,
            TelemetryType::ConfigDH => 0xA1,
            TelemetryType::Beacon => 0xF0,
        }
    }

    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x81 => Some(TelemetryType::Ping),
            0x82 => Some(TelemetryType::RestartArm),
            0x83 => Some(TelemetryType::Restart),
            0x90 => Some(TelemetryType::StartDH),
            0x91 => Some(TelemetryType::StopDH),
            0x92 => Some(TelemetryType::QueryDH),
            0xA0 => Some(TelemetryType::Config),
            0xA1 => Some(TelemetryType::ConfigDH),
            0xF0 => Some(TelemetryType::Beacon),
            _ => None,
        }
    }
}

/// PING telemetry response
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PingTelemetry {
    pub header: TelemetryHeader,
    pub timestamp: Timestamp,
}

impl PingTelemetry {
    pub fn new(sequence: u32, status: CommandStatus) -> Self {
        Self {
            header: TelemetryHeader {
                sequence,
                tm_type: TelemetryType::Ping,
                status,
            },
            timestamp: Timestamp::now(),
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
            header: TelemetryHeader {
                sequence,
                tm_type: TelemetryType::RestartArm,
                status,
            },
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
            header: TelemetryHeader {
                sequence,
                tm_type: TelemetryType::Restart,
                status,
            },
        }
    }
}

/// START_DH telemetry response
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartDHTelemetry {
    pub header: TelemetryHeader,
}

impl StartDHTelemetry {
    pub fn new(sequence: u32, status: CommandStatus) -> Self {
        Self {
            header: TelemetryHeader {
                sequence,
                tm_type: TelemetryType::StartDH,
                status,
            },
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
            header: TelemetryHeader {
                sequence,
                tm_type: TelemetryType::StopDH,
                status,
            },
        }
    }
}

/// QUERY_DH telemetry response
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueryDHTelemetry {
    pub header: TelemetryHeader,
    pub dh_id: DHId,
    pub statistics: Statistics,
}

impl QueryDHTelemetry {
    pub fn new(sequence: u32, status: CommandStatus, dh_id: DHId, statistics: Statistics) -> Self {
        Self {
            header: TelemetryHeader {
                sequence,
                tm_type: TelemetryType::QueryDH,
                status,
            },
            dh_id,
            statistics,
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
            header: TelemetryHeader {
                sequence,
                tm_type: TelemetryType::Config,
                status,
            },
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
            header: TelemetryHeader {
                sequence,
                tm_type: TelemetryType::ConfigDH,
                status,
            },
        }
    }
}

/// BEACON asynchronous telemetry
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct BeaconTelemetry {
    pub header: TelemetryHeader,
    pub timestamp: Timestamp,
}

impl BeaconTelemetry {
    pub fn new() -> Self {
        Self {
            header: TelemetryHeader {
                sequence: 0,
                tm_type: TelemetryType::Beacon,
                status: CommandStatus::Success,
            },
            timestamp: Timestamp::now(),
        }
    }
}

impl Default for BeaconTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

/// Union of all telemetry types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Telemetry {
    Ping(PingTelemetry),
    RestartArm(RestartArmTelemetry),
    Restart(RestartTelemetry),
    StartDH(StartDHTelemetry),
    StopDH(StopDHTelemetry),
    QueryDH(QueryDHTelemetry),
    Config(ConfigTelemetry),
    ConfigDH(ConfigDHTelemetry),
    Beacon(BeaconTelemetry),
}

impl Telemetry {
    pub fn sequence(&self) -> u32 {
        match self {
            Telemetry::Ping(tm) => tm.header.sequence,
            Telemetry::RestartArm(tm) => tm.header.sequence,
            Telemetry::Restart(tm) => tm.header.sequence,
            Telemetry::StartDH(tm) => tm.header.sequence,
            Telemetry::StopDH(tm) => tm.header.sequence,
            Telemetry::QueryDH(tm) => tm.header.sequence,
            Telemetry::Config(tm) => tm.header.sequence,
            Telemetry::ConfigDH(tm) => tm.header.sequence,
            Telemetry::Beacon(tm) => tm.header.sequence,
        }
    }

    pub fn tm_type(&self) -> TelemetryType {
        match self {
            Telemetry::Ping(tm) => tm.header.tm_type,
            Telemetry::RestartArm(tm) => tm.header.tm_type,
            Telemetry::Restart(tm) => tm.header.tm_type,
            Telemetry::StartDH(tm) => tm.header.tm_type,
            Telemetry::StopDH(tm) => tm.header.tm_type,
            Telemetry::QueryDH(tm) => tm.header.tm_type,
            Telemetry::Config(tm) => tm.header.tm_type,
            Telemetry::ConfigDH(tm) => tm.header.tm_type,
            Telemetry::Beacon(tm) => tm.header.tm_type,
        }
    }

    pub fn status(&self) -> CommandStatus {
        match self {
            Telemetry::Ping(tm) => tm.header.status,
            Telemetry::RestartArm(tm) => tm.header.status,
            Telemetry::Restart(tm) => tm.header.status,
            Telemetry::StartDH(tm) => tm.header.status,
            Telemetry::StopDH(tm) => tm.header.status,
            Telemetry::QueryDH(tm) => tm.header.status,
            Telemetry::Config(tm) => tm.header.status,
            Telemetry::ConfigDH(tm) => tm.header.status,
            Telemetry::Beacon(tm) => tm.header.status,
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
        let tm = BeaconTelemetry::new();
        assert_eq!(tm.header.tm_type, TelemetryType::Beacon);
    }

    #[test]
    fn test_telemetry_serialization() {
        let tm = Telemetry::Ping(PingTelemetry::new(42, CommandStatus::Success));
        let json = serde_json::to_string(&tm).unwrap();
        let deserialized: Telemetry = serde_json::from_str(&json).unwrap();
        assert_eq!(tm.sequence(), deserialized.sequence());
    }
}
