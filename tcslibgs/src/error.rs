//! Error definitions for TCSpecial

use thiserror::Error;

/// TCSpecial error types
#[derive(Error, Debug)]
pub enum TcsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_norway::Error),

    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::DeError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Command error: {0}")]
    Command(String),

    #[error("Data handler error: {0}")]
    DataHandler(String),

    #[error("Endpoint error: {0}")]
    Endpoint(String),

    #[error("Timeout")]
    Timeout,

    #[error("Not armed for restart")]
    NotArmed,

    #[error("Invalid arm key")]
    InvalidArmKey,

    #[error("Data handler not found: {0}")]
    DHNotFound(u32),

    #[error("Data handler already exists: {0}")]
    DHExists(u32),

    #[error("Channel error: {0}")]
    Channel(String),

    #[error("Telemetry log error: {0}")]
    Log(String),
}

/// Result type alias for TCSpecial operations
pub type TcsResult<T> = Result<T, TcsError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = TcsError::Config("test".to_string());
        assert_eq!(format!("{}", err), "Configuration error: test");
    }
}
