//! Configuration file format detection and parsing.
//!
//! Every configuration file format is parsed into the *same* Rust types. The
//! structs in [`crate::types`] are the single description of the data; the
//! formats below are just different spellings of it. Adding a format means
//! adding a variant here, never a second set of structs.
//!
//! The format-level structs (`CIConfigJson`, `DHConfigJson`) are deliberately
//! flat -- scalars and `Option`s only, no enums. XML has no native sequence or
//! variant type, so a flat shape is the subset all three formats agree on.

use std::fs;
use std::path::Path;

use serde::de::DeserializeOwned;

use crate::{TcsError, TcsResult};

/// A supported configuration file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Json,
    Yaml,
    Xml,
}

impl ConfigFormat {
    /// Guess the format from a path's extension, defaulting to JSON so that
    /// extensionless and legacy configuration files keep working.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Self {
        match path
            .as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("yaml") | Some("yml") => ConfigFormat::Yaml,
            Some("xml") => ConfigFormat::Xml,
            _ => ConfigFormat::Json,
        }
    }

    /// Parse `text` in this format into `T`.
    pub fn parse<T: DeserializeOwned>(self, text: &str) -> TcsResult<T> {
        match self {
            ConfigFormat::Json => serde_json::from_str(text).map_err(TcsError::from),
            ConfigFormat::Yaml => serde_norway::from_str(text).map_err(TcsError::from),
            ConfigFormat::Xml => quick_xml::de::from_str(text).map_err(TcsError::from),
        }
    }
}

/// Read and parse a configuration file, choosing the parser from its extension.
///
/// This is the single entry point every configuration loader should use.
pub fn load_config_file<T: DeserializeOwned, P: AsRef<Path>>(path: P) -> TcsResult<T> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)?;
    ConfigFormat::from_path(path).parse(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_from_extension() {
        assert_eq!(ConfigFormat::from_path("a.json"), ConfigFormat::Json);
        assert_eq!(ConfigFormat::from_path("a.yaml"), ConfigFormat::Yaml);
        assert_eq!(ConfigFormat::from_path("a.yml"), ConfigFormat::Yaml);
        assert_eq!(ConfigFormat::from_path("a.YML"), ConfigFormat::Yaml);
        assert_eq!(ConfigFormat::from_path("a.xml"), ConfigFormat::Xml);
        // Unknown and missing extensions fall back to JSON.
        assert_eq!(ConfigFormat::from_path("a.conf"), ConfigFormat::Json);
        assert_eq!(ConfigFormat::from_path("tcspecial"), ConfigFormat::Json);
    }
}
