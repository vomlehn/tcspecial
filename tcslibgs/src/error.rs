//! Error definitions for TCSpecial

use thiserror::Error;

/// TCSpecial error types
#[derive(Error, Debug)]
pub enum TcsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A configuration's canonical form, which its digest is taken over, is
    /// built as a JSON value tree. Nothing on the link and no configuration
    /// file is JSON: see [`crate::link`] and [`crate::format`].
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_norway::Error),

    /// Where the reader had got to when it failed, as YAML's error says
    /// itself: quick-xml's carries no position, so `ConfigFormat::parse`
    /// asks the reader and puts it here. Written "at line N column M" as
    /// YAML's is, so what reads a line back out of a message reads either.
    #[error("XML error at line {line} column {column}: {error}")]
    Xml {
        error: quick_xml::DeError,
        line: usize,
        column: usize,
    },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    /// A datagram on a command link that is not a packet of the link's
    /// layout, or whose CRC does not match.
    #[error("{0}")]
    Link(#[from] crate::link::LinkError),

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

    /// A command refused by the ground's own client, unsent: a RESTART was
    /// sent, and nothing more may be until a beacon says it is done.
    #[error("{0}")]
    AwaitingRestart(String),

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
