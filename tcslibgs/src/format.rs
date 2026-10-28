//! Configuration file format detection and parsing.
//!
//! Every configuration file format is parsed into the *same* Rust types. The
//! structs in [`crate::types`] are the single description of the data; the
//! formats below are just different spellings of it. Adding a format means
//! adding a variant here, never a second set of structs.
//!
//! The format-level structs (`CIConfigJson`, and a node's map of attributes)
//! are deliberately flat -- scalars and `Option`s only, no enums. XML has no
//! native sequence or variant type, so a flat shape is the subset both formats
//! agree on.
//!
//! There were three. JSON was read as well, and was the format an
//! extensionless or unrecognised file was assumed to be, which meant a
//! misspelled extension was not an error but a file parsed as the wrong
//! language. Nothing was written in it that was not also written in YAML, so
//! it is gone, and a file this cannot place by its extension is refused.
//! (Nothing here is about the command link, whose packets are stated in
//! XTCE and are no part of configuration: see [`crate::link`].)

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
                "{}: a configuration file is named .yaml, .yml or .xml, and \
                 this is none of those",
                path.display()
            ))
        })
    }

    /// Parse `text` in this format into `T`.
    pub fn parse<T: DeserializeOwned>(self, text: &str) -> TcsResult<T> {
        match self {
            ConfigFormat::Yaml => {
                serde_norway::from_str(text).map_err(TcsError::from)
            }
            ConfigFormat::Xml => {
                let mut reader = quick_xml::de::Deserializer::from_str(text);
                T::deserialize(&mut reader).map_err(|error| {
                    let (line, column) =
                        xml_position(text, &error, reader.get_ref().get_ref());
                    TcsError::Xml {
                        error,
                        line,
                        column,
                    }
                })
            }
        }
    }
}

/// The line and column, counted from one, an XML error is to be reported at.
///
/// Markup that does not parse is where its `<` is, which the reader keeps for
/// just that. Anything else -- an element the type has no field for, a value
/// that is not one -- is found by the deserializer after it has read the
/// event, so it is where the reader stopped: just past the tag or the text in
/// question, and on its line unless the element runs over several.
fn xml_position(
    text: &str,
    error: &quick_xml::DeError,
    reader: &quick_xml::Reader<&[u8]>,
) -> (usize, usize) {
    let offset = match error {
        quick_xml::DeError::InvalidXml(_) => reader.error_position(),
        _ => reader.buffer_position(),
    };
    let bytes = text.as_bytes();
    let before = &bytes[..usize::try_from(offset)
        .map_or(bytes.len(), |at| at.min(bytes.len()))];
    let line_start = before
        .iter()
        .rposition(|b| *b == b'\n')
        .map_or(0, |at| at + 1);
    let line = before.iter().filter(|b| **b == b'\n').count() + 1;
    let column = String::from_utf8_lossy(&before[line_start..])
        .chars()
        .count()
        + 1;
    (line, column)
}

/// Which spelling of a configuration a program was told to read: `--yaml`,
/// `--xml`, or neither.
///
/// Every example program takes the two, and one of them at most: they say
/// which of the example's two files -- `<name>.yaml` or `<name>.xml`, which
/// say the same thing -- is read, and a command line naming both says two
/// things at once. One reading of them, here, so that every program refuses
/// the same mistakes in the same words.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Spelling(Option<ConfigFormat>);

impl Spelling {
    /// Take `arg` if it is `--yaml` or `--xml`: `Ok(true)` if it was one,
    /// `Ok(false)` if it is something else for the caller to read, and what
    /// is wrong where it contradicts one already taken. The same flag twice
    /// says one thing twice and is taken.
    pub fn take(&mut self, arg: &str) -> Result<bool, String> {
        let said = match arg {
            "--yaml" => ConfigFormat::Yaml,
            "--xml" => ConfigFormat::Xml,
            _ => return Ok(false),
        };
        match self.0 {
            Some(already) if already != said => Err(
                "--yaml and --xml each say which configuration to read: give \
                 one of them, or neither"
                    .to_string(),
            ),
            _ => {
                self.0 = Some(said);
                Ok(true)
            }
        }
    }

    /// The configuration to read.
    ///
    /// The one `named`, where one was; a flag beside it has to agree with its
    /// extension, because a file named `.xml` read as asked for in YAML is a
    /// command line that cannot both be right. Otherwise `stem` -- the
    /// example's path without an extension -- in the spelling asked for, or
    /// in `default` where neither was.
    pub fn configuration(
        &self,
        named: Option<String>,
        stem: &str,
        default: ConfigFormat,
    ) -> Result<String, String> {
        match named {
            Some(path) => match (self.0, ConfigFormat::from_path(&path)) {
                (Some(asked), Some(is)) if asked != is => Err(format!(
                    "{path} is {} and {} asks for {}: name the other file, or \
                     leave the flag off",
                    is.name(),
                    asked.flag(),
                    asked.name()
                )),
                _ => Ok(path),
            },
            None => {
                Ok(format!("{stem}.{}", self.0.unwrap_or(default).extension()))
            }
        }
    }
}

impl ConfigFormat {
    /// The extension a file in this format is written with.
    pub fn extension(self) -> &'static str {
        match self {
            ConfigFormat::Yaml => "yaml",
            ConfigFormat::Xml => "xml",
        }
    }

    /// This format's name, as a person says it.
    pub fn name(self) -> &'static str {
        match self {
            ConfigFormat::Yaml => "YAML",
            ConfigFormat::Xml => "XML",
        }
    }

    /// The command-line flag that asks for this format.
    pub fn flag(self) -> &'static str {
        match self {
            ConfigFormat::Yaml => "--yaml",
            ConfigFormat::Xml => "--xml",
        }
    }
}

/// Read and parse a configuration file, choosing the parser from its extension.
///
/// This is the single entry point every configuration loader should use.
pub fn load_config_file<T: DeserializeOwned, P: AsRef<Path>>(
    path: P,
) -> TcsResult<T> {
    let path = path.as_ref();
    let format = ConfigFormat::of_file(path)?;
    let text = fs::read_to_string(path)?;
    format.parse(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// --yaml and --xml pick the example's file, one or neither but not
    /// both, and a file named beside a flag has to be the spelling it asks
    /// for.
    #[test]
    fn a_spelling_flag_picks_the_file_and_the_two_are_not_both_given() {
        let stem = "examples/beacon/beacon";
        let chosen = |args: &[&str]| -> Result<Spelling, String> {
            let mut spelling = Spelling::default();
            for arg in args {
                assert!(spelling.take(arg)?, "{arg} is a spelling flag");
            }
            Ok(spelling)
        };

        let neither = chosen(&[]).unwrap();
        assert_eq!(
            neither.configuration(None, stem, ConfigFormat::Yaml),
            Ok(format!("{stem}.yaml"))
        );
        assert_eq!(
            neither.configuration(None, stem, ConfigFormat::Xml),
            Ok(format!("{stem}.xml"))
        );
        assert_eq!(
            chosen(&["--xml"]).unwrap().configuration(
                None,
                stem,
                ConfigFormat::Yaml
            ),
            Ok(format!("{stem}.xml"))
        );
        assert_eq!(
            chosen(&["--yaml", "--yaml"]).unwrap().configuration(
                None,
                stem,
                ConfigFormat::Xml
            ),
            Ok(format!("{stem}.yaml"))
        );

        // Both is two answers to one question.
        assert!(chosen(&["--yaml", "--xml"]).is_err());
        assert!(chosen(&["--xml", "--yaml"]).is_err());

        // Anything else is the caller's to read.
        assert_eq!(Spelling::default().take("-v"), Ok(false));

        // A file named is read, and a flag beside it must agree with it.
        let xml = chosen(&["--xml"]).unwrap();
        assert_eq!(
            xml.configuration(Some("a.xml".into()), stem, ConfigFormat::Yaml),
            Ok("a.xml".to_string())
        );
        assert!(xml
            .configuration(Some("a.yaml".into()), stem, ConfigFormat::Yaml)
            .is_err());
        assert_eq!(
            neither.configuration(
                Some("a.yaml".into()),
                stem,
                ConfigFormat::Xml
            ),
            Ok("a.yaml".to_string())
        );
    }

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

            let said = ConfigFormat::of_file(name).expect_err(name).to_string();
            assert!(said.contains(name), "{name}: {said}");
            assert!(
                said.contains(".yaml") && said.contains(".xml"),
                "{name}: the refusal does not say what is read: {said}"
            );
        }
    }

    /// An XML file that does not read says where, as a YAML file does.
    ///
    /// quick-xml's own error carries no position, and a file refused for a
    /// misspelled element was reported on its first line, which is the one
    /// line it was certainly not wrong on.
    #[test]
    fn an_xml_error_says_where_the_reader_stopped() {
        #[derive(serde::Deserialize, Debug)]
        #[serde(deny_unknown_fields)]
        #[allow(dead_code)]
        struct Doc {
            #[serde(rename = "@x")]
            x: Option<String>,
            a: String,
            b: Option<String>,
        }
        let at = |text: &str| match ConfigFormat::Xml.parse::<Doc>(text) {
            Err(TcsError::Xml { line, column, .. }) => (line, column),
            other => panic!("{text:?} was read as {other:?}"),
        };

        // An element the type has no field for: on its line, the column being
        // wherever the deserializer had read to before it refused it.
        assert_eq!(at("<doc>\n  <a>x</a>\n  <c>y</c>\n</doc>\n").0, 3);
        // A closing tag that is not the open element's: where it starts.
        assert_eq!(at("<doc>\n  <a>x</b>\n</doc>\n"), (2, 7));
        // A root over several lines does not throw the count off.
        assert_eq!(at("<doc\n    x=\"1\">\n  <a>x</a>\n  <c/>\n</doc>\n").0, 4);

        // And the message says so, in the words YAML's does.
        let said = ConfigFormat::Xml
            .parse::<Doc>("<doc>\n  <c/>\n</doc>\n")
            .expect_err("c is no field")
            .to_string();
        assert!(said.contains("at line 2 column"), "{said}");
    }
}
