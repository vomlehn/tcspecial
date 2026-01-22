//! Telemetry definitions for TCSpecial
//!
//! Both command and telemetry messages are subject to loss between the sender
//! and the receiver.

use serde::{Deserialize, Serialize};
use crate::types::{Statistics, Timestamp};

/// Telemetry message identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum TelemetryId {
    Ping = 1,
    RestartArm = 2,
    Restart = 3,
    StartDH = 4,
    StopDH = 5,
    QueryDH = 6,
    Config = 7,
    ConfigDH = 8,
    Beacon = 100,
}

/// Response status for telemetry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResponseStatus {
    Success,
    Failure(ErrorCode),
}

/// Error codes for failed commands
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum ErrorCode {
    /// Unknown error
    Unknown = 0,
    /// Invalid command
    InvalidCommand = 1,
    /// DH not found
    DHNotFound = 2,
    /// DH already exists
    DHAlreadyExists = 3,
    /// Invalid arm key
    InvalidArmKey = 4,
    /// Restart not armed
    RestartNotArmed = 5,
    /// Arm window expired
    ArmWindowExpired = 6,
    /// Resource allocation failed
    ResourceAllocationFailed = 7,
    /// Invalid configuration
    InvalidConfiguration = 8,
    /// I/O error
    IoError = 9,
}

/// Base telemetry response with common fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryBase {
    /// Sequence number from the corresponding command
    pub sequence: u32,
    /// Response status
    pub status: ResponseStatus,
}

impl TelemetryBase {
    pub fn success(sequence: u32) -> Self {
        Self {
            sequence,
            status: ResponseStatus::Success,
        }
    }

    pub fn failure(sequence: u32, code: ErrorCode) -> Self {
        Self {
            sequence,
            status: ResponseStatus::Failure(code),
        }
    }
}

/// PING telemetry response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingTelemetry {
    pub base: TelemetryBase,
    /// Spacecraft time when response was sent
    pub timestamp: Timestamp,
}

impl PingTelemetry {
    pub fn new(sequence: u32, timestamp: Timestamp) -> Self {
        Self {
            base: TelemetryBase::success(sequence),
            timestamp,
        }
    }
}

/// RESTART_ARM telemetry response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartArmTelemetry {
    pub base: TelemetryBase,
}

impl RestartArmTelemetry {
    pub fn success(sequence: u32) -> Self {
        Self {
            base: TelemetryBase::success(sequence),
        }
    }

    pub fn failure(sequence: u32, code: ErrorCode) -> Self {
        Self {
            base: TelemetryBase::failure(sequence, code),
        }
    }
}

/// RESTART telemetry response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartTelemetry {
    pub base: TelemetryBase,
}

impl RestartTelemetry {
    pub fn success(sequence: u32) -> Self {
        Self {
            base: TelemetryBase::success(sequence),
        }
    }

    pub fn failure(sequence: u32, code: ErrorCode) -> Self {
        Self {
            base: TelemetryBase::failure(sequence, code),
        }
    }
}

/// START_DH telemetry response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartDHTelemetry {
    pub base: TelemetryBase,
}

impl StartDHTelemetry {
    pub fn success(sequence: u32) -> Self {
        Self {
            base: TelemetryBase::success(sequence),
        }
    }

    pub fn failure(sequence: u32, code: ErrorCode) -> Self {
        Self {
            base: TelemetryBase::failure(sequence, code),
        }
    }
}

/// STOP_DH telemetry response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopDHTelemetry {
    pub base: TelemetryBase,
}

impl StopDHTelemetry {
    pub fn success(sequence: u32) -> Self {
        Self {
            base: TelemetryBase::success(sequence),
        }
    }

    pub fn failure(sequence: u32, code: ErrorCode) -> Self {
        Self {
            base: TelemetryBase::failure(sequence, code),
        }
    }
}

/// QUERY_DH telemetry response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryDHTelemetry {
    pub base: TelemetryBase,
    /// Statistics from the data handler
    pub statistics: Option<Statistics>,
}

impl QueryDHTelemetry {
    pub fn success(sequence: u32, statistics: Statistics) -> Self {
        Self {
            base: TelemetryBase::success(sequence),
            statistics: Some(statistics),
        }
    }

    pub fn failure(sequence: u32, code: ErrorCode) -> Self {
        Self {
            base: TelemetryBase::failure(sequence, code),
            statistics: None,
        }
    }
}

/// CONFIG telemetry response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigTelemetry {
    pub base: TelemetryBase,
}

impl ConfigTelemetry {
    pub fn success(sequence: u32) -> Self {
        Self {
            base: TelemetryBase::success(sequence),
        }
    }

    pub fn failure(sequence: u32, code: ErrorCode) -> Self {
        Self {
            base: TelemetryBase::failure(sequence, code),
        }
    }
}

/// CONFIG_DH telemetry response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDHTelemetry {
    pub base: TelemetryBase,
}

impl ConfigDHTelemetry {
    pub fn success(sequence: u32) -> Self {
        Self {
            base: TelemetryBase::success(sequence),
        }
    }

    pub fn failure(sequence: u32, code: ErrorCode) -> Self {
        Self {
            base: TelemetryBase::failure(sequence, code),
        }
    }
}

/// BEACON telemetry - sent asynchronously at configured intervals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeaconTelemetry {
    /// Spacecraft time at which the beacon message was sent
    pub timestamp: Timestamp,
}

impl BeaconTelemetry {
    pub fn new(timestamp: Timestamp) -> Self {
        Self { timestamp }
    }

    pub fn now() -> Self {
        Self {
            timestamp: Timestamp::now(),
        }
    }
}

/// Enumeration of all possible telemetry messages
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub fn id(&self) -> TelemetryId {
        match self {
            Telemetry::Ping(_) => TelemetryId::Ping,
            Telemetry::RestartArm(_) => TelemetryId::RestartArm,
            Telemetry::Restart(_) => TelemetryId::Restart,
            Telemetry::StartDH(_) => TelemetryId::StartDH,
            Telemetry::StopDH(_) => TelemetryId::StopDH,
            Telemetry::QueryDH(_) => TelemetryId::QueryDH,
            Telemetry::Config(_) => TelemetryId::Config,
            Telemetry::ConfigDH(_) => TelemetryId::ConfigDH,
            Telemetry::Beacon(_) => TelemetryId::Beacon,
        }
    }

    /// Serialize telemetry to JSON bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize telemetry from JSON bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_telemetry() {
        let tlm = PingTelemetry::new(1, Timestamp::now());
        assert_eq!(tlm.base.sequence, 1);
        assert_eq!(tlm.base.status, ResponseStatus::Success);
    }

    #[test]
    fn test_beacon_telemetry() {
        let beacon = BeaconTelemetry::now();
        let tlm = Telemetry::Beacon(beacon);
        assert_eq!(tlm.id(), TelemetryId::Beacon);
    }

    #[test]
    fn test_telemetry_serialization() {
        let tlm = Telemetry::Ping(PingTelemetry::new(42, Timestamp::new(1000, 0)));
        let bytes = tlm.to_bytes().unwrap();
        let decoded = Telemetry::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.id(), TelemetryId::Ping);
    }
}
