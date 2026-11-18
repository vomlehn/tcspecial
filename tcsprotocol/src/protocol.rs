//! Protocol definitions for TCSpecial
//!
//! Provides canonical values for operating system dependent values like
//! address families, socket types, and protocols, and, on Linux, their
//! conversion to the system's own numbers.

use serde::{Deserialize, Serialize};

/// Canonical address family values
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u16)]
pub enum AddressFamily {
    Unix = 1,
    Inet = 2,
    Inet6 = 10,
    Ax25 = 3,
    Ipx = 4,
    Appletalk = 5,
    X25 = 9,
    Decnet = 12,
    Key = 15,
    Netlink = 16,
    Packet = 17,
    Rds = 21,
    Pppox = 24,
    Llc = 26,
    Ib = 27,
    Mpls = 28,
    Can = 29,
    Tipc = 30,
    Bluetooth = 31,
    Alg = 38,
    Vsock = 40,
    Xdp = 44,
}

impl AddressFamily {
    /// Converts from the OS's address family number to the canonical value.
    ///
    /// # Arguments
    ///
    /// * `value` - The OS's `AF_*` number.
    ///
    /// # Returns
    ///
    /// The address family, or `None` for a number this does not know.
    #[cfg(target_os = "linux")]
    pub fn from_os(value: i32) -> Option<Self> {
        use tcsos::linux::net::numbers as n;

        match value {
            n::AF_UNIX => Some(AddressFamily::Unix),
            n::AF_INET => Some(AddressFamily::Inet),
            n::AF_INET6 => Some(AddressFamily::Inet6),
            n::AF_AX25 => Some(AddressFamily::Ax25),
            n::AF_IPX => Some(AddressFamily::Ipx),
            n::AF_APPLETALK => Some(AddressFamily::Appletalk),
            n::AF_X25 => Some(AddressFamily::X25),
            n::AF_DECnet => Some(AddressFamily::Decnet),
            n::AF_KEY => Some(AddressFamily::Key),
            n::AF_NETLINK => Some(AddressFamily::Netlink),
            n::AF_PACKET => Some(AddressFamily::Packet),
            n::AF_RDS => Some(AddressFamily::Rds),
            n::AF_PPPOX => Some(AddressFamily::Pppox),
            n::AF_LLC => Some(AddressFamily::Llc),
            n::AF_IB => Some(AddressFamily::Ib),
            n::AF_MPLS => Some(AddressFamily::Mpls),
            n::AF_CAN => Some(AddressFamily::Can),
            n::AF_TIPC => Some(AddressFamily::Tipc),
            n::AF_BLUETOOTH => Some(AddressFamily::Bluetooth),
            n::AF_ALG => Some(AddressFamily::Alg),
            n::AF_VSOCK => Some(AddressFamily::Vsock),
            n::AF_XDP => Some(AddressFamily::Xdp),
            _ => None,
        }
    }

    /// Converts from the canonical value to the OS's address family number.
    ///
    /// # Returns
    ///
    /// The OS's `AF_*` number.
    #[cfg(target_os = "linux")]
    pub fn to_os(&self) -> i32 {
        use tcsos::linux::net::numbers as n;

        match self {
            AddressFamily::Unix => n::AF_UNIX,
            AddressFamily::Inet => n::AF_INET,
            AddressFamily::Inet6 => n::AF_INET6,
            AddressFamily::Ax25 => n::AF_AX25,
            AddressFamily::Ipx => n::AF_IPX,
            AddressFamily::Appletalk => n::AF_APPLETALK,
            AddressFamily::X25 => n::AF_X25,
            AddressFamily::Decnet => n::AF_DECnet,
            AddressFamily::Key => n::AF_KEY,
            AddressFamily::Netlink => n::AF_NETLINK,
            AddressFamily::Packet => n::AF_PACKET,
            AddressFamily::Rds => n::AF_RDS,
            AddressFamily::Pppox => n::AF_PPPOX,
            AddressFamily::Llc => n::AF_LLC,
            AddressFamily::Ib => n::AF_IB,
            AddressFamily::Mpls => n::AF_MPLS,
            AddressFamily::Can => n::AF_CAN,
            AddressFamily::Tipc => n::AF_TIPC,
            AddressFamily::Bluetooth => n::AF_BLUETOOTH,
            AddressFamily::Alg => n::AF_ALG,
            AddressFamily::Vsock => n::AF_VSOCK,
            AddressFamily::Xdp => n::AF_XDP,
        }
    }
}

/// Canonical socket type values
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u16)]
pub enum SocketType {
    Stream = 1,
    Dgram = 2,
    Raw = 3,
    Seqpacket = 5,
}

impl SocketType {
    /// Converts from the OS's socket type number to the canonical value.
    ///
    /// # Arguments
    ///
    /// * `value` - The OS's `SOCK_*` number.
    ///
    /// # Returns
    ///
    /// The socket type, or `None` for a number this does not know.
    #[cfg(target_os = "linux")]
    pub fn from_os(value: i32) -> Option<Self> {
        use tcsos::linux::net::numbers as n;

        match value {
            n::SOCK_STREAM => Some(SocketType::Stream),
            n::SOCK_DGRAM => Some(SocketType::Dgram),
            n::SOCK_RAW => Some(SocketType::Raw),
            n::SOCK_SEQPACKET => Some(SocketType::Seqpacket),
            _ => None,
        }
    }

    /// Converts from the canonical value to the OS's socket type number.
    ///
    /// # Returns
    ///
    /// The OS's `SOCK_*` number.
    #[cfg(target_os = "linux")]
    pub fn to_os(&self) -> i32 {
        use tcsos::linux::net::numbers as n;

        match self {
            SocketType::Stream => n::SOCK_STREAM,
            SocketType::Dgram => n::SOCK_DGRAM,
            SocketType::Raw => n::SOCK_RAW,
            SocketType::Seqpacket => n::SOCK_SEQPACKET,
        }
    }

    /// Check if this socket type has stream semantics
    pub fn is_stream(&self) -> bool {
        matches!(self, SocketType::Stream)
    }

    /// Check if this socket type has datagram semantics
    pub fn is_datagram(&self) -> bool {
        !self.is_stream()
    }
}

/// Protocol configuration for socket creation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SocketConfig {
    pub family: AddressFamily,
    pub socket_type: SocketType,
    pub protocol: i32,
}

impl SocketConfig {
    /// Gives the socket configuration for TCP over IPv4.
    ///
    /// # Returns
    ///
    /// The configuration, with the family's default protocol.
    pub fn tcp_v4() -> Self {
        Self {
            family: AddressFamily::Inet,
            socket_type: SocketType::Stream,
            protocol: 0,
        }
    }

    /// Gives the socket configuration for UDP over IPv4.
    ///
    /// # Returns
    ///
    /// The configuration, with the family's default protocol.
    pub fn udp_v4() -> Self {
        Self {
            family: AddressFamily::Inet,
            socket_type: SocketType::Dgram,
            protocol: 0,
        }
    }

    /// Gives the socket configuration for TCP over IPv6.
    ///
    /// # Returns
    ///
    /// The configuration, with the family's default protocol.
    pub fn tcp_v6() -> Self {
        Self {
            family: AddressFamily::Inet6,
            socket_type: SocketType::Stream,
            protocol: 0,
        }
    }

    /// Gives the socket configuration for UDP over IPv6.
    ///
    /// # Returns
    ///
    /// The configuration, with the family's default protocol.
    pub fn udp_v6() -> Self {
        Self {
            family: AddressFamily::Inet6,
            socket_type: SocketType::Dgram,
            protocol: 0,
        }
    }

    /// Gives the socket configuration for a Unix-domain stream socket.
    ///
    /// # Returns
    ///
    /// The configuration, with the family's default protocol.
    pub fn unix_stream() -> Self {
        Self {
            family: AddressFamily::Unix,
            socket_type: SocketType::Stream,
            protocol: 0,
        }
    }

    /// Gives the socket configuration for a Unix-domain datagram socket.
    ///
    /// # Returns
    ///
    /// The configuration, with the family's default protocol.
    pub fn unix_dgram() -> Self {
        Self {
            family: AddressFamily::Unix,
            socket_type: SocketType::Dgram,
            protocol: 0,
        }
    }
}

/// Message framing for stream protocols
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageFrame {
    /// Length of the message (not including the length field itself)
    pub length: u32,
    /// Message data
    pub data: Vec<u8>,
}

impl MessageFrame {
    /// Frames a message.
    ///
    /// # Arguments
    ///
    /// * `data` - The message.
    ///
    /// # Returns
    ///
    /// The frame, its length that of `data`.
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            length: data.len() as u32,
            data,
        }
    }

    /// Serializes the frame to bytes.
    ///
    /// # Returns
    ///
    /// The length, four bytes most significant first, then the data.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(4 + self.data.len());
        bytes.extend_from_slice(&self.length.to_be_bytes());
        bytes.extend_from_slice(&self.data);
        bytes
    }

    /// Deserializes a frame from bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - A length prefix and at least that many bytes after it;
    ///             any more are ignored.
    ///
    /// # Returns
    ///
    /// The frame, or `None` if the bytes are shorter than the frame.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 {
            return None;
        }
        let length =
            u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if bytes.len() < 4 + length as usize {
            return None;
        }
        Some(Self {
            length,
            data: bytes[4..4 + length as usize].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    use tcsos::linux::net::numbers as n;

    #[test]
    #[cfg(target_os = "linux")]
    fn test_address_family_conversion() {
        let af = AddressFamily::Inet;
        assert_eq!(af.to_os(), n::AF_INET);
        assert_eq!(
            AddressFamily::from_os(n::AF_INET),
            Some(AddressFamily::Inet)
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_socket_type_conversion() {
        let st = SocketType::Stream;
        assert_eq!(st.to_os(), n::SOCK_STREAM);
        assert!(st.is_stream());
        assert!(!st.is_datagram());
    }

    #[test]
    fn test_message_frame() {
        let data = vec![1, 2, 3, 4, 5];
        let frame = MessageFrame::new(data.clone());
        let bytes = frame.to_bytes();
        let parsed = MessageFrame::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.data, data);
    }
}
