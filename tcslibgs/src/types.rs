//! Type definitions shared between ground and space software

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeSet;
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

/// Bytes of one transfer kept for display.
///
/// A data handler's panel shows what it last sent and last received. Keeping
/// the whole of a packet to show eight bytes of it would cost a buffer per
/// direction per handler, so only the head is kept, and only this much of it.
pub const DH_SAMPLE_BYTES: usize = 8;

/// The time and the first few bytes of one transfer.
///
/// Fixed size and `Copy`, so that recording one costs no allocation on the
/// path that moves payload data, and so that telemetry carrying one has a
/// layout that does not depend on what the payload sent.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DHSample {
    /// When the transfer happened, or `None` if none has.
    ///
    /// This is the time the data moved, not the time it was asked about: a
    /// panel showing the latter would count every query as activity.
    pub time: Option<Timestamp>,
    /// How many of `bytes` are data.
    pub len: u8,
    /// Bytes in the whole transfer, of which `bytes` holds the head.
    ///
    /// Kept so that a display can say a packet was longer than what is shown.
    /// Without it, a transfer of exactly [`DH_SAMPLE_BYTES`] and one of a
    /// thousand look identical.
    pub total: u32,
    /// The first [`DH_SAMPLE_BYTES`] bytes of the transfer, or fewer.
    pub bytes: [u8; DH_SAMPLE_BYTES],
}

impl Default for DHSample {
    fn default() -> Self {
        Self {
            time: None,
            len: 0,
            total: 0,
            bytes: [0; DH_SAMPLE_BYTES],
        }
    }
}

impl DHSample {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a transfer that has just happened.
    ///
    /// Longer data is truncated to what fits; the panel shows a head and says
    /// so, and the rest is of no use to it.
    pub fn record(&mut self, data: &[u8]) {
        let len = data.len().min(DH_SAMPLE_BYTES);
        // Cleared rather than overwritten in place, or a short transfer after
        // a long one would leave the tail of the long one behind it.
        self.bytes = [0; DH_SAMPLE_BYTES];
        self.bytes[..len].copy_from_slice(&data[..len]);
        self.len = len as u8;
        self.total = u32::try_from(data.len()).unwrap_or(u32::MAX);
        self.time = Some(Timestamp::now());
    }

    /// The bytes recorded, which is at most [`DH_SAMPLE_BYTES`] of them.
    pub fn data(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len).min(DH_SAMPLE_BYTES)]
    }

    /// Whether any transfer has been recorded.
    pub fn is_empty(&self) -> bool {
        self.time.is_none()
    }

    /// Whether the transfer was longer than the bytes kept from it.
    pub fn was_truncated(&self) -> bool {
        self.total as usize > self.data().len()
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
}

/// Payload configuration file structure
///
/// A payload file describes payloads and nothing else. It carries no packet
/// interval: how fast a payload produces packets is a property of a
/// simulation rather than of a payload, so it belongs to the simulator's own
/// configuration file. A file that still states one parses, with the interval
/// ignored, the same way one still carrying a `ci_config` section does.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PayloadConfig {
    pub version: String,
    pub description: String,
    /// Named groups of attributes that several data handlers share.
    ///
    /// Optional: a file whose handlers have nothing in common, or that prefers
    /// to spell every one of them out, has no groups.
    #[serde(default)]
    pub data_handler_groups: Vec<DHGroupJson>,
    pub data_handlers: Vec<DHConfigJson>,
    /// Retained so a payload file that still carries a CI section parses,
    /// but unused: the CI reads its own configuration from tcspecial.json.
    #[serde(default)]
    pub ci_config: Option<CIConfigJson>,
}

impl PayloadConfig {
    pub fn len(&self) -> usize {
        self.data_handlers.len()
    }

    /// Look up a data handler group by name.
    pub fn group(&self, name: &str) -> Option<&DHGroupJson> {
        self.data_handler_groups.iter().find(|g| g.name == name)
    }

    /// Settle every data handler into its runtime configuration.
    ///
    /// This is where a handler is laid over the group it names, so it is the
    /// only way to convert a file that uses groups. A group defined twice,
    /// named by a handler and defined nowhere, or defined and named by no
    /// handler is an error here rather than a handler quietly taking the wrong
    /// attributes or none at all.
    pub fn to_dh_configs(&self) -> Result<Vec<DHConfig>, String> {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for group in &self.data_handler_groups {
            if !seen.insert(group.name.as_str()) {
                return Err(format!(
                    "data handler group \"{}\" is defined more than once",
                    group.name
                ));
            }
        }

        let configs: Vec<DHConfig> = self
            .data_handlers
            .iter()
            .map(|dh| {
                let group = match &dh.group {
                    Some(name) => Some(self.group(name).ok_or_else(|| {
                        format!(
                            "data handler \"{}\" names group \"{}\", which is not defined",
                            dh.name, name
                        )
                    })?),
                    None => None,
                };
                dh.to_dh_config_in(group)
            })
            .collect::<Result<_, String>>()?;

        // A group no handler names has no effect on the configuration, which
        // is exactly what a group whose name a handler misspelled looks like.
        // Checked after the handlers, so that the misspelling is reported from
        // the handler's end, where the name actually is.
        let named: BTreeSet<&str> = self
            .data_handlers
            .iter()
            .filter_map(|dh| dh.group.as_deref())
            .collect();
        if let Some(unused) = self
            .data_handler_groups
            .iter()
            .find(|group| !named.contains(group.name.as_str()))
        {
            // Worded exactly as the endpoint and simulator configuration
            // formats word the same rule, so that one rule reads as one rule
            // wherever it is met.
            return Err(format!(
                "data handler group \"{}\" is named by no data handler: name it \
                 from one, or remove the group",
                unused.name
            ));
        }

        Ok(configs)
    }
}

/// A named group of attributes that several data handlers share.
///
/// Every attribute is optional, so a group carries exactly what its handlers
/// have in common and no more. What a group deliberately cannot carry is a
/// `dh_id` or a `name`: those are what tell one handler of a group from
/// another, and so belong to the handler.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DHGroupJson {
    /// Name data handlers use to refer to this group.
    pub name: String,
    #[serde(rename = "type", default)]
    pub dh_type: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub packet_size: Option<usize>,
}

/// JSON representation of DH config
///
/// Every attribute but `dh_id` and `name` is optional, because a handler
/// naming a group need only state what it does not take from that group.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DHConfigJson {
    pub dh_id: u32,
    pub name: String,
    /// Name of the group supplying the attributes this handler does not state.
    #[serde(default)]
    pub group: Option<String>,
    #[serde(rename = "type", default)]
    pub dh_type: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub packet_size: Option<usize>,
}

impl DHConfigJson {
    /// Convert a handler that states every attribute for itself.
    ///
    /// A handler naming a group needs that group's attributes, which only the
    /// file as a whole knows; use [`PayloadConfig::to_dh_configs`] for those.
    pub fn to_dh_config(&self) -> Result<DHConfig, String> {
        self.to_dh_config_in(None)
    }

    /// Convert a handler, taking from `group` whatever the handler does not
    /// state itself.
    pub fn to_dh_config_in(&self, group: Option<&DHGroupJson>) -> Result<DHConfig, String> {
        // The handler wins wherever it says anything, so a group holds what
        // its handlers share without preventing one of them from differing.
        let dh_type = self
            .dh_type
            .as_deref()
            .or_else(|| group.and_then(|g| g.dh_type.as_deref()));
        let protocol = self
            .protocol
            .as_deref()
            .or_else(|| group.and_then(|g| g.protocol.as_deref()));
        let address = self
            .address
            .as_deref()
            .or_else(|| group.and_then(|g| g.address.as_deref()));
        let port = self.port.or_else(|| group.and_then(|g| g.port));
        let path = self
            .path
            .as_deref()
            .or_else(|| group.and_then(|g| g.path.as_deref()));
        let packet_size = self
            .packet_size
            .or_else(|| group.and_then(|g| g.packet_size));

        let endpoint = match dh_type {
            Some("network") => {
                let protocol = match protocol {
                    Some("tcp") => NetworkProtocol::Tcp,
                    Some("udp") => NetworkProtocol::Udp,
                    Some("unix_stream") => NetworkProtocol::UnixStream,
                    Some("unix_dgram") => NetworkProtocol::UnixDgram,
                    _ => return Err("Invalid or missing protocol".to_string()),
                };
                EndpointConfig::Network(NetworkConfig {
                    protocol,
                    address: address.ok_or("Missing address")?.to_string(),
                    port: port.ok_or("Missing port")?,
                })
            }
            Some("device") => EndpointConfig::Device(DeviceConfig {
                path: path.ok_or("Missing path")?.to_string(),
            }),
            Some(other) => return Err(format!("Invalid DH type: {}", other)),
            None => return Err("Missing type".to_string()),
        };

        Ok(DHConfig {
            dh_id: DHId(self.dh_id),
            name: DHName::new(&self.name),
            endpoint,
            packet_size: packet_size.ok_or("Missing packet_size")?,
        })
    }
}

/// Payload bytes per telemetry log segment file, when the configuration
/// does not say. The segment file header is added on top of this.
fn default_log_segment_bytes() -> u32 {
    65_536
}

/// JSON representation of CI config
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CIConfigJson {
    pub address: String,
    pub port: u16,
    pub protocol: String,
    pub beacon_interval_ms: u32,
    /// Directory holding the telemetry log's segment files. The directory
    /// must already exist. Telemetry logging is disabled when this is absent.
    #[serde(default)]
    pub log_dir: Option<String>,
    /// Payload bytes per segment file. The segment file header is added to
    /// this, so it is the space available for telemetry records.
    #[serde(default = "default_log_segment_bytes")]
    pub log_segment_bytes: u32,
}

/// Command interpreter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CIConfig {
    pub address: String,
    pub port: u16,
    pub protocol: NetworkProtocol,
    pub beacon_interval: BeaconTime,
    /// Directory holding the telemetry log's segment files, or `None` to
    /// run without a telemetry log.
    pub log_dir: Option<String>,
    /// Payload bytes per segment file, not counting the segment file header.
    pub log_segment_bytes: u32,
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
            log_dir: self.log_dir.clone(),
            log_segment_bytes: self.log_segment_bytes,
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

    /// A payload file as it is really read, so these tests go through the
    /// same deserialization a file on disk does.
    fn payload(text: &str) -> PayloadConfig {
        crate::ConfigFormat::Yaml.parse(text).expect("parses")
    }

    #[test]
    fn a_sample_records_the_head_of_a_transfer() {
        let mut sample = DHSample::new();
        assert!(sample.is_empty(), "nothing has moved yet");
        assert_eq!(sample.data(), &[] as &[u8]);

        sample.record(&[1, 2, 3]);
        assert!(!sample.is_empty());
        assert_eq!(sample.data(), &[1, 2, 3]);
        assert_eq!(sample.total, 3);
        assert!(!sample.was_truncated());
        assert!(sample.time.is_some(), "a recorded transfer has a time");
    }

    #[test]
    fn a_sample_keeps_only_what_fits() {
        // A packet may be any size; a sample is one fixed-size value, so the
        // head is kept and the rest dropped.
        let long: Vec<u8> = (0..64).collect();
        let mut sample = DHSample::new();
        sample.record(&long);

        assert_eq!(sample.data().len(), DH_SAMPLE_BYTES);
        assert_eq!(sample.data(), &long[..DH_SAMPLE_BYTES]);
        // The whole length survives, so a display can say there was more.
        assert_eq!(sample.total, 64);
        assert!(sample.was_truncated());
    }

    #[test]
    fn recording_again_replaces_the_previous_sample() {
        // Including the bytes the shorter transfer does not reach, or a long
        // transfer followed by a short one would show the tail of the first.
        let mut sample = DHSample::new();
        sample.record(&[9; DH_SAMPLE_BYTES]);
        sample.record(&[1, 2]);

        assert_eq!(sample.data(), &[1, 2]);
        assert_eq!(sample.bytes[2..], [0; DH_SAMPLE_BYTES - 2]);
        assert_eq!(sample.total, 2);
        assert!(!sample.was_truncated());
    }

    #[test]
    fn a_zero_length_transfer_is_still_a_transfer() {
        // It has a time, so a panel shows when rather than the placeholder.
        let mut sample = DHSample::new();
        sample.record(&[]);

        assert!(!sample.is_empty());
        assert_eq!(sample.data(), &[] as &[u8]);
    }

    #[test]
    fn a_handler_takes_its_groups_attributes() {
        let config = payload(
            "
version: \"1.0\"
description: one group, two handlers in it
data_handler_groups:
  - name: udp_localhost
    type: network
    protocol: udp
    address: localhost
data_handlers:
  - dh_id: 1
    name: DH1
    group: udp_localhost
    port: 5001
    packet_size: 11
  - dh_id: 3
    name: DH3
    group: udp_localhost
    port: 5003
    packet_size: 15
",
        );

        let handlers = config.to_dh_configs().unwrap();

        // Everything shared comes from the group; the port and packet size,
        // which are what tell the two apart, come from each handler.
        assert_eq!(
            handlers[0].endpoint,
            EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: "localhost".to_string(),
                port: 5001,
            })
        );
        assert_eq!(handlers[0].packet_size, 11);
        assert_eq!(
            handlers[1].endpoint,
            EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: "localhost".to_string(),
                port: 5003,
            })
        );
        assert_eq!(handlers[1].packet_size, 15);
    }

    #[test]
    fn a_handler_overrides_its_group() {
        let config = payload(
            "
version: \"1.0\"
description: a handler differing from the group it is in
data_handler_groups:
  - name: udp_localhost
    type: network
    protocol: udp
    address: localhost
    packet_size: 11
data_handlers:
  - dh_id: 0
    name: DH0
    group: udp_localhost
    protocol: tcp
    port: 5000
",
        );

        let handlers = config.to_dh_configs().unwrap();

        assert_eq!(
            handlers[0].endpoint,
            EndpointConfig::Network(NetworkConfig {
                // The handler's own, not the group's udp.
                protocol: NetworkProtocol::Tcp,
                address: "localhost".to_string(),
                port: 5000,
            })
        );
        // Not overridden, so still the group's.
        assert_eq!(handlers[0].packet_size, 11);
    }

    #[test]
    fn a_handler_needs_no_group() {
        let config = payload(
            "
version: \"1.0\"
description: a handler stating everything for itself
data_handlers:
  - dh_id: 2
    name: DH2
    type: device
    path: /dev/urandom
    packet_size: 1
",
        );

        let handlers = config.to_dh_configs().unwrap();
        assert_eq!(
            handlers[0].endpoint,
            EndpointConfig::Device(DeviceConfig {
                path: "/dev/urandom".to_string()
            })
        );
    }

    #[test]
    fn a_group_no_handler_names_is_an_error() {
        // A group with no members has no effect on the configuration, so a
        // file carrying one is more likely wrong than deliberate.
        let config = payload(
            "
version: \"1.0\"
description: a group nothing is in
data_handler_groups:
  - name: udp_localhost
    type: network
    protocol: udp
    address: localhost
data_handlers:
  - dh_id: 0
    name: DH0
    type: network
    protocol: tcp
    address: localhost
    port: 5000
    packet_size: 12
",
        );

        let message = config.to_dh_configs().expect_err("must be rejected");
        assert!(
            message.contains("udp_localhost"),
            "the error should name the group nothing is in, but said: {message}"
        );
    }

    #[test]
    fn a_misspelled_group_is_reported_from_the_handler_not_the_group() {
        // A typo leaves the group unnamed and the name undefined at once. The
        // handler's end is where the misspelling actually is, so that is the
        // error worth giving.
        let config = payload(
            "
version: \"1.0\"
description: a handler misspelling the one group
data_handler_groups:
  - name: udp_localhost
    type: network
    protocol: udp
    address: localhost
data_handlers:
  - dh_id: 0
    name: DH0
    group: udp_localhst
    port: 5000
    packet_size: 12
",
        );

        let message = config.to_dh_configs().expect_err("must be rejected");
        assert!(
            message.contains("udp_localhst") && message.contains("DH0"),
            "the error should name the handler and its misspelling, but said: {message}"
        );
    }

    #[test]
    fn a_handler_with_no_packet_size_anywhere_is_an_error() {
        // A missing packet size would otherwise resolve to zero, and a handler
        // whose packets are zero bytes long reads nothing forever.
        let config = payload(
            "
version: \"1.0\"
description: a handler nothing gives a packet size
data_handler_groups:
  - name: udp_localhost
    type: network
    protocol: udp
    address: localhost
data_handlers:
  - dh_id: 0
    name: DH0
    group: udp_localhost
    port: 5000
",
        );

        assert!(config.to_dh_configs().is_err());
    }
}
