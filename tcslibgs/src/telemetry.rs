//! Telemetry definitions for TCSpecial
//!
//! Telemetry is sent from space to ground.

use serde::{Deserialize, Serialize};

use crate::config_digest::{ConfigDigest, ConfigVersion};
use crate::types::{CommandStatus, DHId, DHSample, Statistics, Timestamp};

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
    Connect,
    RestartArm,
    Restart,
    StartDH,
    StopDH,
    QueryDH,
    QueryDHSample,
    Config,
    ConfigDH,
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
pub struct ConnectTelemetry {
    pub header: TelemetryHeader,
    pub version: ConfigVersion,
    /// The version the payload configuration file states, which the beacon
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
            header: TelemetryHeader {
                sequence,
                tm_type: TelemetryType::Connect,
                status: CommandStatus::Success,
            },
            version,
            config_version,
            digest,
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

/// QUERY_DH_SAMPLE telemetry response
///
/// Carries what a data handler last sent and last received, each with the time
/// it happened. Either may be empty, which is what a handler that has moved
/// nothing in that direction looks like -- distinct from a handler that has
/// moved zero bytes, which cannot happen.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueryDHSampleTelemetry {
    pub header: TelemetryHeader,
    pub dh_id: DHId,
    pub sent: DHSample,
    pub received: DHSample,
}

impl QueryDHSampleTelemetry {
    pub fn new(
        sequence: u32,
        status: CommandStatus,
        dh_id: DHId,
        sent: DHSample,
        received: DHSample,
    ) -> Self {
        Self {
            header: TelemetryHeader {
                sequence,
                tm_type: TelemetryType::QueryDHSample,
                status,
            },
            dh_id,
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
///
/// It carries what a CONNECT response carries -- the build and the digest of
/// the configuration this process read -- because a beacon arrives whether or
/// not anything has connected. A ground station that is only listening can
/// then see which software is flying and which payload set it is serving,
/// rather than having to ask before it can know.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct BeaconTelemetry {
    pub header: TelemetryHeader,
    pub timestamp: Timestamp,
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
    pub fn new(
        version: ConfigVersion,
        config_version: ConfigVersion,
        digest: ConfigDigest,
    ) -> Self {
        Self {
            header: TelemetryHeader {
                sequence: 0,
                tm_type: TelemetryType::Beacon,
                status: CommandStatus::Success,
            },
            timestamp: Timestamp::now(),
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
        Command::Connect(_) => Telemetry::Connect(ConnectTelemetry::new(
            sequence,
            ConfigVersion::of_this_build(),
            ConfigVersion::of_this_build(),
            ConfigDigest([0u8; 16]),
        )),
        Command::RestartArm(_) => {
            Telemetry::RestartArm(RestartArmTelemetry::new(sequence, status))
        }
        Command::Restart(_) => Telemetry::Restart(RestartTelemetry::new(sequence, status)),
        Command::StartDH(_) => Telemetry::StartDH(StartDHTelemetry::new(sequence, status)),
        Command::StopDH(_) => Telemetry::StopDH(StopDHTelemetry::new(sequence, status)),
        Command::QueryDH(cmd) => Telemetry::QueryDH(QueryDHTelemetry::new(
            sequence,
            status,
            cmd.dh_id,
            Statistics::new(),
        )),
        Command::QueryDHSample(cmd) => Telemetry::QueryDHSample(QueryDHSampleTelemetry::new(
            sequence,
            status,
            cmd.dh_id,
            DHSample::new(),
            DHSample::new(),
        )),
        Command::Config(_) => Telemetry::Config(ConfigTelemetry::new(sequence, status)),
        Command::ConfigDH(_) => Telemetry::ConfigDH(ConfigDHTelemetry::new(sequence, status)),
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
        let tm = BeaconTelemetry::new(
            ConfigVersion::of_this_build(),
            ConfigVersion::of_text("1.0").expect("a version"),
            ConfigDigest([0u8; 16]),
        );
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
