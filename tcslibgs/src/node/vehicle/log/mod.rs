//! What a log node says about itself, and what a source must be to feed it.
//!
//! `type: log` is somewhere data stops: what reaches it is written down, and
//! nothing is read from it. The attributes are three -- where the segment
//! files go, what shape the records are, and how large a segment file grows --
//! and the rules about them are here so that the ground tool and the flight
//! software apply the same ones.
//!
//! The format is the interesting part, because it is what decides whether a
//! source may feed this log at all. A fixed-length log is the most compact
//! there is and costs nothing per record, and in exchange every record in it
//! is the same length for the life of the log -- so a source whose records
//! vary cannot write to one. That is settled when the configuration is loaded
//! rather than when a record is written: a log that would have to refuse a
//! record in flight loses telemetry at the moment telemetry is worth most.
//!
//! No `tcslog` here. The library that writes the files is the flight side's
//! business; what this crate holds is what a file may say, which is the same
//! question for every node type -- see [`crate::node`].
//! `tcspecial::node::vehicle::log` turns this into a `tcslog::Format`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::Scalar;

/// Payload bytes per segment file when a node does not say.
///
/// The telemetry log's own default. A log node whose segment size differed
/// from it for no stated reason would be two answers to one question.
pub const DEFAULT_SEGMENT_BYTES: u32 = 65_536;

/// What the records of a log look like.
///
/// The three `tcslog` offers, named as a configuration spells them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogFormat {
    /// Every record is exactly this many bytes and carries no header at all.
    Fixed(u32),
    /// Records of any length, each with a four-byte length before it. For data
    /// that already carries its own time.
    VariableSimple,
    /// As [`LogFormat::VariableSimple`], and the header also carries the time
    /// the record was written and its place in the session. The one to choose
    /// when the question asked of the log later will be *when* -- which for a
    /// control loop it usually is.
    VariableTsRc,
}

impl LogFormat {
    /// The format this word asks for.
    ///
    /// Three spellings, and `fixed` carries the length: `fixed:16`, or
    /// `fixed: 16` with as much space as somebody feels like. The length is
    /// part of the format because it is fixed for the life of the log -- a log
    /// whose record length changed would be a file nothing could read back --
    /// so it belongs where the format is stated rather than in an attribute
    /// beside it that could disagree with it.
    pub fn of(text: &str) -> Result<LogFormat, String> {
        if let Some(rest) = text.strip_prefix("fixed") {
            let length = rest.trim_start().strip_prefix(':').ok_or_else(|| {
                "format: a fixed-length log says how long: fixed:<n>, as in \
                 fixed:16".to_string()
            })?;
            let length: u32 = length.trim().parse().map_err(|_| {
                format!(
                    "format: \"{}\" is not a whole number of bytes",
                    length.trim()
                )
            })?;
            if length == 0 {
                return Err(
                    "format: fixed:0 is a log of records with nothing in them"
                        .to_string(),
                );
            }
            return Ok(LogFormat::Fixed(length));
        }

        match text {
            "variable_simple" => Ok(LogFormat::VariableSimple),
            "variable_ts_rc" => Ok(LogFormat::VariableTsRc),
            other => Err(format!(
                "format: \"{other}\" is not a format a log knows. There are \
                 fixed:<n>, variable_simple and variable_ts_rc"
            )),
        }
    }

    /// How long every record must be, for a format that insists.
    ///
    /// `None` for the two variable formats, which take a record of any length
    /// -- so a source needs nothing said about its records to feed one.
    pub fn exactly(self) -> Option<u32> {
        match self {
            LogFormat::Fixed(bytes) => Some(bytes),
            LogFormat::VariableSimple | LogFormat::VariableTsRc => None,
        }
    }
}

/// What a log node says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogParams {
    /// Where the segment files are written. The directory must exist.
    pub directory: String,
    /// What the records look like.
    pub format: LogFormat,
    /// Payload bytes per segment file.
    pub segment_bytes: u32,
}

/// What this log node says, or what is wrong with what it says.
///
/// Every attribute is checked, including the ones that are not a log's: one
/// nothing reads is refused rather than ignored, because a log whose `fromat`
/// was misspelled would be opened in whichever format the default happens to
/// be and would look exactly like a configured log.
pub fn params_of(
    what: &str,
    attributes: &BTreeMap<String, Scalar>,
) -> Result<LogParams, String> {
    const OWN: [&str; 3] = ["directory", "format", "segment_bytes"];
    for attribute in attributes.keys() {
        if OWN.contains(&attribute.as_str()) {
            continue;
        }
        return Err(
            match crate::graph::spelled(attribute, |other| OWN.contains(&other))
            {
                Some(hint) => {
                    format!(
                        "\"{what}\": there is nothing called \"{attribute}\" \
                         on a log node: {hint}"
                    )
                }
                None => format!(
                    "\"{what}\": there is nothing called \"{attribute}\" on a \
                     log node. It takes directory, format and segment_bytes"
                ),
            },
        );
    }

    let directory = attributes
        .get("directory")
        .ok_or_else(|| {
            format!(
                "\"{what}\" is a log node, so it needs the directory its \
                 segment files are written in: say directory"
            )
        })?
        .as_str()
        .to_string();

    let format = attributes
        .get("format")
        .ok_or_else(|| {
            format!(
                "\"{what}\" is a log node, so it needs the format of its \
                 records: one of fixed:<n>, variable_simple or variable_ts_rc"
            )
        })
        .and_then(|stated| {
            LogFormat::of(stated.as_str())
                .map_err(|why| format!("\"{what}\": {why}"))
        })?;

    let segment_bytes = match attributes.get("segment_bytes") {
        Some(stated) => {
            let bytes: u32 = stated.as_str().parse().map_err(|_| {
                format!(
                    "\"{what}\": segment_bytes: \"{}\" is not a whole number \
                     of bytes",
                    stated.as_str()
                )
            })?;
            // A segment file that cannot hold a record is a log that fills a
            // file per record and then fails on the one that will not fit.
            let least = format.exactly().unwrap_or(1);
            if bytes < least {
                return Err(format!(
                    "\"{what}\": segment_bytes: {bytes} is less than one \
                     record, which is {least} bytes here"
                ));
            }
            bytes
        }
        None => DEFAULT_SEGMENT_BYTES,
    };

    Ok(LogParams {
        directory,
        format,
        segment_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attributes(stated: &[(&str, &str)]) -> BTreeMap<String, Scalar> {
        stated
            .iter()
            .map(|(a, v)| (a.to_string(), Scalar::of(*v)))
            .collect()
    }

    /// The three formats, as a file spells them.
    #[test]
    fn the_formats_are_read_as_a_file_writes_them() {
        assert_eq!(
            LogFormat::of("variable_simple"),
            Ok(LogFormat::VariableSimple)
        );
        assert_eq!(
            LogFormat::of("variable_ts_rc"),
            Ok(LogFormat::VariableTsRc)
        );
        assert_eq!(LogFormat::of("fixed:16"), Ok(LogFormat::Fixed(16)));
        // However much space somebody left around the colon.
        assert_eq!(LogFormat::of("fixed: 16"), Ok(LogFormat::Fixed(16)));
        assert_eq!(LogFormat::of("fixed :  16 "), Ok(LogFormat::Fixed(16)));

        // And what each insists on of a record.
        assert_eq!(LogFormat::Fixed(16).exactly(), Some(16));
        assert_eq!(LogFormat::VariableSimple.exactly(), None);
        assert_eq!(LogFormat::VariableTsRc.exactly(), None);
    }

    /// A format that is not one of the three is refused, listing them.
    #[test]
    fn a_format_that_is_not_one_is_refused() {
        for (text, wanted) in [
            ("compressed", "variable_ts_rc"),
            ("fixed", "fixed:<n>"),
            ("fixed:wide", "not a whole number"),
            ("fixed:0", "nothing in them"),
        ] {
            let said =
                LogFormat::of(text).expect_err("a format that is not one");
            assert!(
                said.contains(wanted),
                "{text}: the complaint should say {wanted}: {said}"
            );
        }
    }

    /// What a log must say, and what it may.
    #[test]
    fn a_log_says_where_and_in_what_shape() {
        assert_eq!(
            params_of(
                "housekeeping",
                &attributes(&[
                    ("directory", "/var/log/tcspecial"),
                    ("format", "variable_ts_rc"),
                ])
            ),
            Ok(LogParams {
                directory: "/var/log/tcspecial".to_string(),
                format: LogFormat::VariableTsRc,
                segment_bytes: DEFAULT_SEGMENT_BYTES,
            })
        );

        // And the one thing it may say beyond those two.
        assert_eq!(
            params_of(
                "housekeeping",
                &attributes(&[
                    ("directory", "/var/log/tcspecial"),
                    ("format", "fixed:16"),
                    ("segment_bytes", "4096"),
                ])
            )
            .expect("a log")
            .segment_bytes,
            4096
        );
    }

    /// A log's attribute written with hyphens is told its spelling.
    #[test]
    fn segment_bytes_with_a_hyphen_is_told_its_spelling() {
        let said = params_of(
            "log",
            &[
                ("directory", "/tmp"),
                ("format", "variable_simple"),
                ("segment-bytes", "4096"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), Scalar::of(v)))
            .collect(),
        )
        .expect_err("one spelling");
        assert!(said.contains("it is spelled segment_bytes"), "{said}");
    }

    /// What a log cannot do without, and what it cannot have.
    #[test]
    fn a_log_that_says_too_little_or_too_much_is_refused() {
        for (stated, wanted) in [
            (vec![("format", "variable_simple")], "directory"),
            (vec![("directory", "/tmp")], "format"),
            (
                vec![
                    ("directory", "/tmp"),
                    ("format", "variable_simple"),
                    ("fromat", "x"),
                ],
                "fromat",
            ),
            (
                vec![
                    ("directory", "/tmp"),
                    ("format", "fixed:64"),
                    ("segment_bytes", "16"),
                ],
                "less than one record",
            ),
            (
                vec![
                    ("directory", "/tmp"),
                    ("format", "variable_simple"),
                    ("segment_bytes", "lots"),
                ],
                "segment_bytes",
            ),
        ] {
            let said = params_of("housekeeping", &attributes(&stated))
                .expect_err("a log that is wrong");
            assert!(
                said.contains(wanted),
                "the complaint should say {wanted}: {said}"
            );
        }
    }
}
