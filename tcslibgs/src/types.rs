//! Common types used throughout TCSpecial

use serde::{Deserialize, Serialize};
use std::fmt;

/// Timestamp representing spacecraft time
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timestamp {
    /// Seconds since epoch
    pub seconds: u64,
    /// Nanoseconds within the second
    pub nanoseconds: u32,
}

impl Timestamp {
    pub fn new(seconds: u64, nanoseconds: u32) -> Self {
        Self { seconds, nanoseconds }
    }

    pub fn now() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Self {
            seconds: duration.as_secs(),
            nanoseconds: duration.subsec_nanos(),
        }
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:09}", self.seconds, self.nanoseconds)
    }
}

/// Key used for arming restart operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArmKey(pub u32);

/// Data Handler identifier - must implement Ord trait
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DHId(pub u32);

impl fmt::Display for DHId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DH{}", self.0)
    }
}

/// Data Handler type - network or device
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DHType {
    /// Network-based data handler (TCP/IP, UDP/IP, etc.)
    Network,
    /// Device-based data handler (serial ports, I2C, SPI, etc.)
    Device,
}

/// Data Handler name/configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DHName(pub String);

impl DHName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// Beacon interval time in milliseconds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeaconTime(pub u64);

impl Default for BeaconTime {
    fn default() -> Self {
        Self(10000) // 10 seconds default
    }
}

/// I/O Statistics for a data handler
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Statistics {
    /// Spacecraft time when statistics were collected
    pub timestamp: Option<Timestamp>,
    /// Number of bytes received
    pub bytes_received: u64,
    /// Number of read operations completed successfully
    pub reads_completed: u64,
    /// Number of read operations that returned an error
    pub reads_failed: u64,
    /// Number of bytes sent
    pub bytes_sent: u64,
    /// Number of write operations completed successfully
    pub writes_completed: u64,
    /// Number of write operations that returned an error
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

/// Canonical address family values (OS-independent)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum CanonicalAddressFamily {
    Unix = 1,
    Inet = 2,
    Inet6 = 3,
    Ax25 = 4,
    Ipx = 5,
    Appletalk = 6,
    X25 = 7,
    Decnet = 8,
    Key = 9,
    Netlink = 10,
    Packet = 11,
    Rds = 12,
    Pppox = 13,
    Llc = 14,
    Ib = 15,
    Mpls = 16,
    Can = 17,
    Tipc = 18,
    Bluetooth = 19,
    Alg = 20,
    Vsock = 21,
    Xdp = 22,
}

impl CanonicalAddressFamily {
    /// Convert from OS-specific address family to canonical
    #[cfg(target_os = "linux")]
    pub fn from_os(af: i32) -> Option<Self> {
        match af {
            libc::AF_UNIX => Some(Self::Unix),
            libc::AF_INET => Some(Self::Inet),
            libc::AF_INET6 => Some(Self::Inet6),
            libc::AF_AX25 => Some(Self::Ax25),
            libc::AF_IPX => Some(Self::Ipx),
            libc::AF_APPLETALK => Some(Self::Appletalk),
            libc::AF_X25 => Some(Self::X25),
            libc::AF_KEY => Some(Self::Key),
            libc::AF_NETLINK => Some(Self::Netlink),
            libc::AF_PACKET => Some(Self::Packet),
            libc::AF_PPPOX => Some(Self::Pppox),
            libc::AF_LLC => Some(Self::Llc),
            libc::AF_CAN => Some(Self::Can),
            libc::AF_TIPC => Some(Self::Tipc),
            libc::AF_BLUETOOTH => Some(Self::Bluetooth),
            libc::AF_ALG => Some(Self::Alg),
            libc::AF_VSOCK => Some(Self::Vsock),
            libc::AF_XDP => Some(Self::Xdp),
            _ => None,
        }
    }

    /// Convert from canonical to OS-specific address family
    #[cfg(target_os = "linux")]
    pub fn to_os(self) -> i32 {
        match self {
            Self::Unix => libc::AF_UNIX,
            Self::Inet => libc::AF_INET,
            Self::Inet6 => libc::AF_INET6,
            Self::Ax25 => libc::AF_AX25,
            Self::Ipx => libc::AF_IPX,
            Self::Appletalk => libc::AF_APPLETALK,
            Self::X25 => libc::AF_X25,
            Self::Decnet => libc::AF_DECnet,
            Self::Key => libc::AF_KEY,
            Self::Netlink => libc::AF_NETLINK,
            Self::Packet => libc::AF_PACKET,
            Self::Rds => libc::AF_RDS,
            Self::Pppox => libc::AF_PPPOX,
            Self::Llc => libc::AF_LLC,
            Self::Ib => libc::AF_IB,
            Self::Mpls => libc::AF_MPLS,
            Self::Can => libc::AF_CAN,
            Self::Tipc => libc::AF_TIPC,
            Self::Bluetooth => libc::AF_BLUETOOTH,
            Self::Alg => libc::AF_ALG,
            Self::Vsock => libc::AF_VSOCK,
            Self::Xdp => libc::AF_XDP,
        }
    }
}

/// Canonical socket type values (OS-independent)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CanonicalSocketType {
    Stream = 1,
    Dgram = 2,
    Raw = 3,
    SeqPacket = 4,
}

impl CanonicalSocketType {
    #[cfg(target_os = "linux")]
    pub fn from_os(st: i32) -> Option<Self> {
        match st {
            libc::SOCK_STREAM => Some(Self::Stream),
            libc::SOCK_DGRAM => Some(Self::Dgram),
            libc::SOCK_RAW => Some(Self::Raw),
            libc::SOCK_SEQPACKET => Some(Self::SeqPacket),
            _ => None,
        }
    }

    #[cfg(target_os = "linux")]
    pub fn to_os(self) -> i32 {
        match self {
            Self::Stream => libc::SOCK_STREAM,
            Self::Dgram => libc::SOCK_DGRAM,
            Self::Raw => libc::SOCK_RAW,
            Self::SeqPacket => libc::SOCK_SEQPACKET,
        }
    }

    /// Returns true if this socket type has stream semantics
    pub fn is_stream(&self) -> bool {
        matches!(self, Self::Stream)
    }

    /// Returns true if this socket type has datagram semantics
    pub fn is_datagram(&self) -> bool {
        matches!(self, Self::Dgram | Self::Raw | Self::SeqPacket)
    }
}

/// Configuration for endpoint delays
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EndpointDelayConfig {
    /// Initial delay value in milliseconds
    pub init_ms: u64,
    /// Maximum delay value in milliseconds
    pub max_ms: u64,
}

impl Default for EndpointDelayConfig {
    fn default() -> Self {
        Self {
            init_ms: 100,
            max_ms: 5000,
        }
    }
}

/// Stream endpoint delay configuration
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StreamEPDelay(pub u64);

impl Default for StreamEPDelay {
    fn default() -> Self {
        Self(10) // 10ms default delay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp() {
        let ts = Timestamp::new(1234567890, 123456789);
        assert_eq!(ts.seconds, 1234567890);
        assert_eq!(ts.nanoseconds, 123456789);
    }

    #[test]
    fn test_dh_id_ordering() {
        let id1 = DHId(1);
        let id2 = DHId(2);
        assert!(id1 < id2);
    }

    #[test]
    fn test_statistics_default() {
        let stats = Statistics::new();
        assert_eq!(stats.bytes_received, 0);
        assert_eq!(stats.reads_completed, 0);
    }

    #[test]
    fn test_socket_type_semantics() {
        assert!(CanonicalSocketType::Stream.is_stream());
        assert!(CanonicalSocketType::Dgram.is_datagram());
        assert!(CanonicalSocketType::SeqPacket.is_datagram());
    }
}
