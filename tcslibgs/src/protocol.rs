//! Protocol definitions for TCSpecial
//!
//! The formats of command and telemetry messages follow the CCSDS 732.1-B-3
//! Unified Space Data Link Protocol (Blue Book, June 2024).

use serde::{Deserialize, Serialize};
use crate::commands::Command;
use crate::telemetry::Telemetry;

/// Protocol version
pub const PROTOCOL_VERSION: u8 = 1;

/// Maximum message size in bytes
pub const MAX_MESSAGE_SIZE: usize = 65535;

/// Protocol message header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHeader {
    /// Protocol version
    pub version: u8,
    /// Message type (0 = command, 1 = telemetry)
    pub msg_type: u8,
    /// Total message length including header
    pub length: u16,
}

impl MessageHeader {
    pub fn for_command(payload_len: usize) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            msg_type: 0,
            length: (payload_len + 4) as u16, // 4 bytes for header
        }
    }

    pub fn for_telemetry(payload_len: usize) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            msg_type: 1,
            length: (payload_len + 4) as u16,
        }
    }
}

/// A framed protocol message containing either a command or telemetry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMessage {
    pub header: MessageHeader,
    pub payload: MessagePayload,
}

/// Payload of a protocol message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    Command(Command),
    Telemetry(Telemetry),
}

impl ProtocolMessage {
    /// Create a protocol message from a command
    pub fn from_command(cmd: Command) -> Result<Self, serde_json::Error> {
        let payload_bytes = cmd.to_bytes()?;
        Ok(Self {
            header: MessageHeader::for_command(payload_bytes.len()),
            payload: MessagePayload::Command(cmd),
        })
    }

    /// Create a protocol message from telemetry
    pub fn from_telemetry(tlm: Telemetry) -> Result<Self, serde_json::Error> {
        let payload_bytes = tlm.to_bytes()?;
        Ok(Self {
            header: MessageHeader::for_telemetry(payload_bytes.len()),
            payload: MessagePayload::Telemetry(tlm),
        })
    }

    /// Serialize the entire message to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize a message from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(data)
    }

    /// Check if this is a command message
    pub fn is_command(&self) -> bool {
        matches!(self.payload, MessagePayload::Command(_))
    }

    /// Check if this is a telemetry message
    pub fn is_telemetry(&self) -> bool {
        matches!(self.payload, MessagePayload::Telemetry(_))
    }

    /// Get the command if this is a command message
    pub fn as_command(&self) -> Option<&Command> {
        match &self.payload {
            MessagePayload::Command(cmd) => Some(cmd),
            _ => None,
        }
    }

    /// Get the telemetry if this is a telemetry message
    pub fn as_telemetry(&self) -> Option<&Telemetry> {
        match &self.payload {
            MessagePayload::Telemetry(tlm) => Some(tlm),
            _ => None,
        }
    }
}

/// Data handler protocol configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DHProtocolConfig {
    /// Whether the protocol uses stream or datagram semantics
    pub is_stream: bool,
    /// Buffer size for reads
    pub read_buffer_size: usize,
    /// Buffer size for writes
    pub write_buffer_size: usize,
    /// Delay for stream endpoints in milliseconds
    pub stream_delay_ms: Option<u64>,
}

impl Default for DHProtocolConfig {
    fn default() -> Self {
        Self {
            is_stream: false, // Default to datagram
            read_buffer_size: 4096,
            write_buffer_size: 4096,
            stream_delay_ms: None,
        }
    }
}

impl DHProtocolConfig {
    pub fn stream() -> Self {
        Self {
            is_stream: true,
            stream_delay_ms: Some(10),
            ..Default::default()
        }
    }

    pub fn datagram() -> Self {
        Self {
            is_stream: false,
            stream_delay_ms: None,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::PingCommand;

    #[test]
    fn test_protocol_message_command() {
        let cmd = Command::Ping(PingCommand::new(1));
        let msg = ProtocolMessage::from_command(cmd).unwrap();
        assert!(msg.is_command());
        assert!(!msg.is_telemetry());
    }

    #[test]
    fn test_protocol_message_roundtrip() {
        let cmd = Command::Ping(PingCommand::new(42));
        let msg = ProtocolMessage::from_command(cmd).unwrap();
        let bytes = msg.to_bytes().unwrap();
        let decoded = ProtocolMessage::from_bytes(&bytes).unwrap();
        assert!(decoded.is_command());
        if let Some(cmd) = decoded.as_command() {
            assert_eq!(cmd.sequence(), 42);
        }
    }
}
