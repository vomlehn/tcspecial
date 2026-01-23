//! Type definitions shared between ground and space software

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

/// Timestamp type for spacecraft time
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Timestamp {
    /// Seconds since UNIX epoch
    pub seconds: u64,
    /// Nanoseconds within the current second
    pub nanoseconds: u32,
}

impl Timestamp {
    /// Create a new timestamp from the current system time
    pub fn now() -> Self {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Self {
            seconds: duration.as_secs(),
            nanoseconds: duration.subsec_nanos(),
        }
    }
}

/// Data handler identifier
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DHId(pub u32);

impl Ord for DHId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for DHId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Data handler type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DHType {
    /// Network-based data handler (TCP, UDP, etc.)
    Network,
    /// Device-based data handler (/dev/*)
    Device,
}

/// Data handler name
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DHName(pub String);

impl DHName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// Arm key for restart commands
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArmKey(pub u64);

/// Beacon interval time in milliseconds
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct BeaconTime(pub u32);

impl Default for BeaconTime {
    fn default() -> Self {
        Self(5000) // 5 seconds default
    }
}

/// Statistics for data handler operations
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Statistics {
    /// Timestamp when statistics were collected
    pub timestamp: Option<Timestamp>,
    /// Number of bytes received
    pub bytes_received: u64,
    /// Number of successful read operations
    pub reads_completed: u64,
    /// Number of failed read operations
    pub reads_failed: u64,
    /// Number of bytes sent
    pub bytes_sent: u64,
    /// Number of successful write operations
    pub writes_completed: u64,
    /// Number of failed write operations
    pub writes_failed: u64,
}

impl Statistics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_timestamp(mut self) -> Self {
        self.timestamp = Some(Timestamp::now());
        self
    }
}

/// Network protocol type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum NetworkProtocol {
    Tcp,
    Udp,
    UnixStream,
    UnixDgram,
}

/// Configuration for a network endpoint
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkConfig {
    pub protocol: NetworkProtocol,
    pub address: String,
    pub port: u16,
}

/// Configuration for a device endpoint
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceConfig {
    pub path: String,
}

/// Endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EndpointConfig {
    Network(NetworkConfig),
    Device(DeviceConfig),
}

/// Data handler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DHConfig {
    pub dh_id: DHId,
    pub name: DHName,
    pub endpoint: EndpointConfig,
    pub packet_size: usize,
    pub packet_interval_ms: u32,
}

/// Payload configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadConfig {
    pub version: String,
    pub description: String,
    pub data_handlers: Vec<DHConfigJson>,
    pub ci_config: CIConfigJson,
}

/// JSON representation of DH config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DHConfigJson {
    pub dh_id: u32,
    pub name: String,
    #[serde(rename = "type")]
    pub dh_type: String,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub path: Option<String>,
    pub packet_size: usize,
    pub packet_interval_ms: u32,
}

impl DHConfigJson {
    pub fn to_dh_config(&self) -> Result<DHConfig, String> {
        let endpoint = match self.dh_type.as_str() {
            "network" => {
                let protocol = match self.protocol.as_deref() {
                    Some("tcp") => NetworkProtocol::Tcp,
                    Some("udp") => NetworkProtocol::Udp,
                    Some("unix_stream") => NetworkProtocol::UnixStream,
                    Some("unix_dgram") => NetworkProtocol::UnixDgram,
                    _ => return Err("Invalid or missing protocol".to_string()),
                };
                EndpointConfig::Network(NetworkConfig {
                    protocol,
                    address: self.address.clone().ok_or("Missing address")?,
                    port: self.port.ok_or("Missing port")?,
                })
            }
            "device" => EndpointConfig::Device(DeviceConfig {
                path: self.path.clone().ok_or("Missing path")?,
            }),
            _ => return Err(format!("Invalid DH type: {}", self.dh_type)),
        };

        Ok(DHConfig {
            dh_id: DHId(self.dh_id),
            name: DHName::new(&self.name),
            endpoint,
            packet_size: self.packet_size,
            packet_interval_ms: self.packet_interval_ms,
        })
    }
}

/// JSON representation of CI config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CIConfigJson {
    pub address: String,
    pub port: u16,
    pub protocol: String,
    pub beacon_interval_ms: u32,
}

/// Command interpreter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CIConfig {
    pub address: String,
    pub port: u16,
    pub protocol: NetworkProtocol,
    pub beacon_interval: BeaconTime,
}

impl CIConfigJson {
    pub fn to_ci_config(&self) -> Result<CIConfig, String> {
        let protocol = match self.protocol.as_str() {
            "tcp" => NetworkProtocol::Tcp,
            "udp" => NetworkProtocol::Udp,
            _ => return Err(format!("Invalid protocol: {}", self.protocol)),
        };

        Ok(CIConfig {
            address: self.address.clone(),
            port: self.port,
            protocol,
            beacon_interval: BeaconTime(self.beacon_interval_ms),
        })
    }
}

/// Result status for command responses
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CommandStatus {
    Success,
    Failure,
    InvalidCommand,
    InvalidParameter,
    NotArmed,
    NotFound,
    AlreadyExists,
    Timeout,
}

impl CommandStatus {
    pub fn is_success(&self) -> bool {
        matches!(self, CommandStatus::Success)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dhid_ordering() {
        let id1 = DHId(1);
        let id2 = DHId(2);
        assert!(id1 < id2);
    }

    #[test]
    fn test_timestamp_now() {
        let ts = Timestamp::now();
        assert!(ts.seconds > 0);
    }

    #[test]
    fn test_statistics_with_timestamp() {
        let stats = Statistics::new().with_timestamp();
        assert!(stats.timestamp.is_some());
    }
}
