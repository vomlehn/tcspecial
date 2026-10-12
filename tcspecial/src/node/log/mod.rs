//! The log node: what reaches it is written down.
//!
//! `type: log`. Built on `tcslog`, which is the library the telemetry log has
//! always used -- so a log node is a generalisation of something this software
//! already does rather than a new mechanism to trust.
//!
//! # What it is for
//!
//! One log holding what several nodes produced, in the order they happened.
//! That is the whole point of it and it is worth being plain about, because a
//! file per sensor is the obvious alternative and it is the wrong one:
//! reconstructing a control loop afterwards means reading a reading, the
//! average it went into and the decision that came out in the order they
//! occurred, and three files with three clocks cannot be interleaved after the
//! fact with any confidence.
//!
//! # What it writes
//!
//! Whatever arrives, as one record per arrival. The bytes are not inspected:
//! a reading is decimal text by the time it is on a link, a decision is `1` or
//! `0`, and a log that reformatted either would be a log of what it thought
//! rather than of what happened.
//!
//! # What it does not do
//!
//! It never produces. A log is a place data stops, so nothing is read from it
//! and no link may leave it -- which is also why it is the one node type with
//! no fixed number of inputs: there is nothing downstream for an arrival to be
//! ambiguous to.

use tcslibgs::graph::Node;
use tcslibgs::node::log::{params_of, LogFormat, LogParams};
use tcslog::{Format, LogWrite, SEGMENT_FILE_HEADER_LEN};

/// First part of every log segment file's name.
///
/// The node's own name goes after it, so one directory can hold the logs of
/// several nodes and a reader can tell which is which: `log-housekeeping-`.
pub const LOG_PREFIX: &str = "log-";

/// Last part of every log segment file's name, as the telemetry log's is.
pub const LOG_SUFFIX: &str = ".tcslog";

/// A log, open and ready to be written.
pub struct LogNode {
    /// The node's name in the configuration, which every complaint names.
    what: String,
    /// Where the segment files are, for a complaint that has to say where
    /// writing failed.
    directory: String,
    /// The open log.
    log: LogWrite<()>,
    /// What the records look like, which decides what may be written: see
    /// [`format_of`].
    format: Format,
}

impl LogNode {
    /// Open the log this node describes.
    ///
    /// The directory must exist: a log node is not the thing to be creating
    /// directories on a vehicle, where a mistyped path would become a log
    /// nobody is looking in rather than a configuration that was refused.
    pub fn of(node: &Node) -> Result<LogNode, String> {
        let what = node.name.as_str();

        // What the file says, by the rules the ground tool checks it against:
        // see tcslibgs::node::log.
        let LogParams {
            directory,
            format,
            segment_bytes,
        } = params_of(what, &node.attributes)?;
        let format = tcslog_format(format);

        let log = LogWrite::new(
            &directory,
            &format!("{LOG_PREFIX}{what}-"),
            LOG_SUFFIX,
            SEGMENT_FILE_HEADER_LEN.saturating_add(segment_bytes),
            format,
            // No callbacks: a filled segment file stays in the directory
            // until something clears it, there being no path that sends files
            // to the ground yet.
            (),
        )
        .map_err(|e| format!("\"{what}\": cannot open a log in {directory}: {e}"))?;

        Ok(LogNode {
            what: what.to_string(),
            directory,
            log,
            format,
        })
    }

    /// Write one record.
    ///
    /// Reported rather than swallowed. The fabric counts a refused delivery
    /// and carries on to the next destination -- a log that cannot be written
    /// must not stop the ground getting its telemetry -- so what this owes its
    /// caller is the truth about what happened, not an attempt to carry on.
    pub fn record(&mut self, data: &[u8]) -> Result<(), String> {
        self.log.write(data).map(|_| ()).map_err(|e| {
            format!(
                "{}: cannot write a {}-byte record to the log in {}: {e}",
                self.what,
                data.len(),
                self.directory
            )
        })
    }

    /// Push what has been written to the file.
    ///
    /// Not after every record: a log flushed per arrival is a vehicle doing a
    /// write system call per reading, and the records are in the kernel's hands
    /// either way. Called when a pass is over, and whenever something is about
    /// to happen that the log should be on the far side of.
    pub fn flush(&mut self) -> Result<(), String> {
        self.log
            .flush()
            .map_err(|e| format!("{}: cannot flush the log in {}: {e}", self.what, self.directory))
    }

    /// What the records of this log look like.
    pub fn format(&self) -> Format {
        self.format
    }

    /// What this log is called in the configuration.
    pub fn name(&self) -> &str {
        &self.what
    }
}

/// What this log is, in a line: the node, where it writes and in what shape.
///
/// Written by hand because the open log is not something to show -- it holds a
/// file, a position and a clock anchor, none of which says anything useful
/// about the configuration -- and because a complaint about a log wants the
/// three facts that identify it.
impl std::fmt::Debug for LogNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "log \"{}\" in {} as {:?}",
            self.what, self.directory, self.format
        )
    }
}

/// The `tcslog` format these records are written in.
///
/// The one place the two spellings meet. A configuration states a format by
/// the rules in `tcslibgs::node::log`, which the ground tool checks without
/// `tcslog` in it at all; this is where what it settled becomes what the
/// library takes.
fn tcslog_format(format: LogFormat) -> Format {
    match format {
        LogFormat::Fixed(bytes) => Format::Fixed(bytes),
        LogFormat::VariableSimple => Format::VariableSimple,
        LogFormat::VariableTsRc => Format::VariableTsRc,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tcslibgs::graph::Graph;
    use tcslibgs::Scalar;

    /// A node built here rather than read, so a test can make one wrong.
    ///
    /// What a log node may say is checked in `tcslibgs::node::log`, where the
    /// ground tool checks it too. What is tested here is the other half: that
    /// a log opens where the node said, writes what arrives, and reports what
    /// it could not write.
    fn node(name: &str, attributes: &[(&str, &str)]) -> Node {
        Node {
            name: name.to_string(),
            node_type: "log".to_string(),
            group: None,
            attributes: attributes
                .iter()
                .map(|(a, v)| (a.to_string(), Scalar::of(*v)))
                .collect(),
            editor: None,
        }
    }

    /// A log node with nowhere to write is refused, saying where it looked.
    #[test]
    fn a_log_with_nowhere_to_write_is_refused() {
        let said = LogNode::of(&node(
            "housekeeping",
            &[
                ("directory", "/nonesuch/nowhere"),
                ("format", "variable_simple"),
            ],
        ))
        .expect_err("a directory that is not there");
        assert!(
            said.contains("/nonesuch/nowhere") && said.contains("housekeeping"),
            "the complaint should name the log and the directory: {said}"
        );

        // And a log whose attributes are wrong never gets as far as looking:
        // the rules are the ground tool's, and they are applied here too.
        let said = LogNode::of(&node("housekeeping", &[("format", "variable_simple")]))
            .expect_err("a log with no directory");
        assert!(said.contains("directory"), "{said}");
    }

    /// What arrives is written, and the log holds it.
    #[test]
    fn what_arrives_is_written() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut log = LogNode::of(&node(
            "housekeeping",
            &[
                ("directory", where_.path().to_str().expect("a path")),
                ("format", "variable_ts_rc"),
            ],
        ))
        .expect("a log");

        assert_eq!(log.name(), "housekeeping");
        assert_eq!(log.format(), Format::VariableTsRc);

        // Three readings and a decision, as the thermostat produces them.
        for record in [&b"289.15"[..], b"289.15", b"287.3188", b"1"] {
            log.record(record).expect("a record");
        }
        log.flush().expect("the log is flushed");

        // One segment file, named for the node so that a directory holding
        // several logs can be read.
        let written: Vec<String> = std::fs::read_dir(where_.path())
            .expect("the directory")
            .map(|entry| {
                entry
                    .expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        assert_eq!(written.len(), 1, "{written:?}");
        assert!(
            written[0].starts_with("log-housekeeping-") && written[0].ends_with(".tcslog"),
            "the segment file should be named for the node: {written:?}"
        );

        // And the records are in it: the file is at least their bytes plus
        // the headers.
        let bytes = std::fs::metadata(where_.path().join(&written[0]))
            .expect("the segment file")
            .len();
        let records = 6 + 6 + 8 + 1;
        assert!(bytes >= records, "{bytes} bytes is not four records");
    }

    /// A record the format cannot hold is refused, not silently cut.
    #[test]
    fn a_record_the_format_cannot_hold_is_refused() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut log = LogNode::of(&node(
            "fixed_log",
            &[
                ("directory", where_.path().to_str().expect("a path")),
                ("format", "fixed:4"),
            ],
        ))
        .expect("a log");

        log.record(b"abcd").expect("a record of exactly four bytes");

        let said = log
            .record(b"abcdefgh")
            .expect_err("eight bytes in a four-byte log");
        assert!(
            said.contains("fixed_log") && said.contains("8-byte"),
            "the complaint should name the log and the record: {said}"
        );

        // What it says about itself, for a log line that has to identify it.
        assert!(
            format!("{log:?}").contains("fixed_log"),
            "{log:?}"
        );
    }

    /// The shipped example's log opens as the file describes it.
    #[test]
    fn the_examples_log_is_what_it_says() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let graph = Graph::load(root.join("tests/manual/thermostat.yaml")).expect("it reads");
        let housekeeping = graph.node("housekeeping").expect("the log");

        // Opened into a directory of this test's own, the example's being a
        // place on a vehicle: what is checked is that nothing else the node
        // says is refused, and that it dates its records -- which is what a
        // control loop is asked about afterwards.
        let where_ = tempfile::tempdir().expect("a directory");
        let mut moved = housekeeping.clone();
        moved.attributes.insert(
            "directory".to_string(),
            Scalar::of(where_.path().to_str().expect("a path")),
        );

        let log = LogNode::of(&moved).expect("the example's log opens");
        assert_eq!(log.format(), Format::VariableTsRc);
    }
}
