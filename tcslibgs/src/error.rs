//! Error types for TCSpecial

use thiserror::Error;
use crate::telemetry::ErrorCode;

/// Main error type for TCSpecial operations
#[derive(Error, Debug)]
pub enum TcsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Data handler error: {0}")]
    DataHandler(String),

    #[error("Command error: {code:?} - {message}")]
    Command { code: ErrorCode, message: String },

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Resource allocation failed: {0}")]
    ResourceAllocation(String),

    #[error("Timeout")]
    Timeout,

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Invalid state: {0}")]
    InvalidState(String),
}

impl TcsError {
    pub fn protocol(msg: impl Into<String>) -> Self {
        Self::Protocol(msg.into())
    }

    pub fn data_handler(msg: impl Into<String>) -> Self {
        Self::DataHandler(msg.into())
    }

    pub fn command(code: ErrorCode, msg: impl Into<String>) -> Self {
        Self::Command {
            code,
            message: msg.into(),
        }
    }

    pub fn configuration(msg: impl Into<String>) -> Self {
        Self::Configuration(msg.into())
    }

    pub fn resource_allocation(msg: impl Into<String>) -> Self {
        Self::ResourceAllocation(msg.into())
    }

    pub fn invalid_state(msg: impl Into<String>) -> Self {
        Self::InvalidState(msg.into())
    }

    /// Convert to an error code for telemetry responses
    pub fn to_error_code(&self) -> ErrorCode {
        match self {
            Self::Io(_) => ErrorCode::IoError,
            Self::Serialization(_) => ErrorCode::InvalidCommand,
            Self::Protocol(_) => ErrorCode::InvalidCommand,
            Self::DataHandler(_) => ErrorCode::DHNotFound,
            Self::Command { code, .. } => *code,
            Self::Configuration(_) => ErrorCode::InvalidConfiguration,
            Self::ResourceAllocation(_) => ErrorCode::ResourceAllocationFailed,
            Self::Timeout => ErrorCode::IoError,
            Self::ConnectionClosed => ErrorCode::IoError,
            Self::InvalidState(_) => ErrorCode::Unknown,
        }
    }
}

/// Result type alias for TCSpecial operations
pub type TcsResult<T> = Result<T, TcsError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = TcsError::protocol("invalid header");
        assert!(err.to_string().contains("Protocol error"));
    }

    #[test]
    fn test_error_code_conversion() {
        let err = TcsError::Timeout;
        assert_eq!(err.to_error_code(), ErrorCode::IoError);
    }
}
