//! Configuration file format detection and parsing.
//!
//! Every configuration file format is parsed into the *same* Rust types. The
//! structs in [`crate::types`] are the single description of the data; the
//! formats below are just different spellings of it. Adding a format means
//! adding a variant here, never a second set of structs.
//!
//! The format-level structs (`CIConfigJson`, `DHConfigJson`) are deliberately
//! flat -- scalars and `Option`s only, no enums. XML has no native sequence or
//! variant type, so a flat shape is the subset both formats agree on.
//!
//! There were three. JSON was read as well, and was the format an
//! extensionless or unrecognised file was assumed to be, which meant a
//! misspelled extension was not an error but a file parsed as the wrong
//! language. Nothing was written in it that was not also written in YAML, so
//! it is gone, and a file this cannot place by its extension is refused.
//! (Nothing here is about the command link, which carries JSON between the
//! ground and the spacecraft and is no part of configuration.)

use std::fs;
use std::path::Path;

use serde::de::DeserializeOwned;

use crate::{TcsError, TcsResult};

/// A supported configuration file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Yaml,
    Xml,
}

impl ConfigFormat {
    /// The format a path's extension names, or nothing for one this does not
    /// recognise.
    ///
    /// Nothing rather than a default: a file whose extension is not one of
    /// these is more likely a mistake about which file is being read than a
    /// file meaning to be read some other way, and the one default there ever
    /// was -- JSON -- turned a misspelled extension into a file parsed as the
    /// wrong language.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
        match path
            .as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("yaml") | Some("yml") => Some(ConfigFormat::Yaml),
            Some("xml") => Some(ConfigFormat::Xml),
            _ => None,
        }
    }

    /// The format of the file at `path`, or what is wrong with asking.
    pub fn of_file<P: AsRef<Path>>(path: P) -> TcsResult<Self> {
        let path = path.as_ref();
        Self::from_path(path).ok_or_else(|| {
            TcsError::Config(format!(
                "{}: a configuration file is named .yaml, .yml or .xml, and this \
                 is none of those",
                path.display()
            ))
        })
    }

    /// Parse `text` in this format into `T`.
    pub fn parse<T: DeserializeOwned>(self, text: &str) -> TcsResult<T> {
        match self {
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
    let format = ConfigFormat::of_file(path)?;
    let text = fs::read_to_string(path)?;
    format.parse(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_from_extension() {
        assert_eq!(ConfigFormat::from_path("a.yaml"), Some(ConfigFormat::Yaml));
        assert_eq!(ConfigFormat::from_path("a.yml"), Some(ConfigFormat::Yaml));
        assert_eq!(ConfigFormat::from_path("a.YML"), Some(ConfigFormat::Yaml));
        assert_eq!(ConfigFormat::from_path("a.xml"), Some(ConfigFormat::Xml));
        assert_eq!(ConfigFormat::from_path("a.XML"), Some(ConfigFormat::Xml));
    }

    /// An extension this does not know is refused, and the refusal says what
    /// a configuration file is called.
    ///
    /// It used to be read as JSON, which is how a file named `.yam` became a
    /// file that "describes no payloads" rather than one whose name was
    /// misspelled.
    #[test]
    fn an_extension_that_is_not_one_of_these_is_refused() {
        for name in ["a.json", "a.conf", "a.yam", "tcspecial", "a.xm"] {
            assert_eq!(ConfigFormat::from_path(name), None, "{name}");

            let said = ConfigFormat::of_file(name)
                .expect_err(name)
                .to_string();
            assert!(said.contains(name), "{name}: {said}");
            assert!(
                said.contains(".yaml") && said.contains(".xml"),
                "{name}: the refusal does not say what is read: {said}"
            );
        }
    }
}
