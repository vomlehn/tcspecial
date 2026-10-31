//! Checks a tcspecial configuration file, and the simulator file beside it.
//!
//! Every program that reads these files refuses the first thing wrong with
//! one and stops, which is the right thing for a program that cannot run
//! without it and the wrong thing for a person fixing the file: a set with
//! four mistakes in it is four runs of tcsmoc, each reporting one of them.
//!
//! This asks the same rules for all of their answers at once, and says for
//! each one which file it is in, which line of it, and what is wrong. The
//! rules are not restated here -- they are `tcslibgs`'s, where the programs
//! that run the files get them -- so a file this accepts is a file they
//! accept, which is the only thing that makes a verifier worth running.
//!
//! A configuration is a graph of nodes, and this checks:
//!
//! * that it reads, following the files it includes;
//! * and that `tcslibgs::graph::Graph::check` finds nothing wrong with it,
//!   which is the same code tcspecial runs aboard. This program adds the line
//!   numbers and nothing else: a problem about a node is reported in the file
//!   that node is written in, which is not always the file named on the
//!   command line.
//! * The simulator file, if one is given: that it parses, and that every name
//!   in it is one the graph and the file itself agree on -- a simulated
//!   payload the graph has, a group the file declares and uses, no setting
//!   that is not one; see `tcslibgs::SimConfigFile::problems_by_name`. And,
//!   where those are all right, that the simulator would stand in for what it
//!   names -- not for a payload that is only driven, nor for one that is a
//!   program -- which is `tcslibgs::graph::simulated::handlers`, the question
//!   tcssim and tcsmoc ask before they start one.
//!
//! The simulator file is checked against the configuration only when the
//! configuration reads. One that does not read has no payloads to join to,
//! and a join against payloads nobody could name would report every simulated
//! payload as unknown: a page of problems caused by this program rather than
//! by the files.

use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tcslibgs::graph::check::Severity;
use tcslibgs::{ConfigFormat, SimConfigFile, TcsError};

pub mod locate;
pub mod startup;

use locate::{Where, THE_FILE_ITSELF};

/// What is done with warnings: tcsverify's `--warnings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Warnings {
    /// Recorded and printed, and not counted against the limit.
    #[default]
    On,
    /// Not recorded at all.
    Off,
    /// Recorded, printed, and counted against the limit as an error is --
    /// for the limit only: a run whose only findings are warnings still
    /// passes.
    Errors,
}

/// How much a run looks for: tcsverify's `--error-limit` and `--warnings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// How many counted findings are recorded before the run stops, or
    /// `None` to look at everything. Errors are counted, and warnings too
    /// under [`Warnings::Errors`].
    pub error_limit: Option<usize>,
    pub warnings: Warnings,
}

impl Limits {
    /// Everything, warnings included: what the programs that check a file
    /// before running it ask for. A configuration is small, and each decides
    /// for itself how much of what was found to show.
    pub const EVERYTHING: Limits = Limits {
        error_limit: None,
        warnings: Warnings::On,
    };
}

impl Default for Limits {
    fn default() -> Self {
        Limits::EVERYTHING
    }
}

/// One thing wrong with one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The file it is in, named as the command line named it.
    pub file: PathBuf,
    /// The line of that file to look at.
    pub line: usize,
    /// What is wrong, in the words the library uses for it.
    pub message: String,
    /// Whether the configuration says something wrong or has not finished
    /// saying it. See [`Severity`], which is `tcslibgs`'s: the split belongs
    /// with the rules and not with the program that prints them.
    pub severity: Severity,
    /// The rule's own answer, where it came from a configuration's
    /// [`tcslibgs::graph::Graph::check`].
    ///
    /// Kept for a program that has rules of its own about the same thing and
    /// has to tell which of these it has already said: tcsgui marks faults on
    /// the nodes as somebody draws, and lists beside them what this finds that
    /// it does not. Not printed: the message is what it says.
    pub problem: Option<tcslibgs::graph::check::Problem>,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Labelled, both kinds. A report whose warnings said so and whose
        // errors said nothing would leave a reader working out which an
        // unlabelled line was.
        write!(
            f,
            "{}:{}: {}: {}",
            self.file.display(),
            self.line,
            self.severity.label(),
            self.message
        )
    }
}

/// What a run found.
#[derive(Debug, Default)]
pub struct Report {
    /// The problems, in the order the rules are asked. No more counted
    /// findings than [`Report::limits`] allows.
    pub findings: Vec<Finding>,
    /// Whether the run reached its limit and stopped there: nothing was
    /// looked for after the last counted finding was recorded.
    pub halted: bool,
    /// How much the run looks for. See [`Limits`].
    pub limits: Limits,
    /// Whether the simulator file was checked against the tcspecial
    /// configuration.
    ///
    /// False when no simulator file was given, and false when the payload
    /// file had problems of its own -- in which case the simulator file was
    /// still read and reported on for what it says by itself.
    pub joined: bool,
}

impl Report {
    /// Whether anything is wrong, as against unfinished.
    ///
    /// Warnings do not count. A configuration with a node whose attributes
    /// are not all in yet is one somebody is in the middle of writing, and a
    /// verifier that failed on it would fail on every save: what the status
    /// is for is telling `make` that the files say something wrong.
    ///
    /// The errors recorded, which are the errors printed: a run stopped by
    /// its limit before it found one passes, as a limit of nought always
    /// does. Asking for none of the reasons is asking for no verdict.
    pub fn found_errors(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.severity == Severity::Error)
    }

    /// Whether anything at all was found, wrong or unfinished.
    pub fn found_anything(&self) -> bool {
        !self.findings.is_empty()
    }

    /// Gives the status a program exits with after this.
    ///
    /// # Returns
    ///
    /// Nought when no error was found, so that `make` and a shell `&&` can
    /// be told by it, and 1 when one was.
    pub fn status(&self) -> i32 {
        if self.found_errors() {
            1
        } else {
            0
        }
    }

    /// Writes every finding, in the order it was found, one per line.
    ///
    /// All of them: how many is the run's limit, applied while it looked --
    /// see [`Report::limits`] -- so there is nothing left over to count. And
    /// errors and warnings as they came rather than errors first, so that a
    /// run stopped by its limit ends on the finding that stopped it.
    ///
    /// # Arguments
    ///
    /// * `out` - Where the lines are written.
    ///
    /// # Errors
    ///
    /// Whatever writing to `out` fails with.
    pub fn write(&self, out: &mut impl Write) -> io::Result<()> {
        for line in self.said() {
            writeln!(out, "{line}")?;
        }
        Ok(())
    }

    /// The lines [`Report::write`] prints, one per element.
    ///
    /// For a program that says them somewhere other than a stream -- a log, a
    /// window -- so that every program reading these files says what this one
    /// says about them, in the same words and the same order.
    pub fn said(&self) -> Vec<String> {
        self.lines().into_iter().map(|(_, line)| line).collect()
    }

    /// Gives the lines [`Report::write`] prints, each with whether it is
    /// about an error.
    ///
    /// # Returns
    ///
    /// One `(is an error, line)` pair per finding, in the order found.
    pub fn lines(&self) -> Vec<(bool, String)> {
        self.findings
            .iter()
            .map(|finding| {
                (finding.severity == Severity::Error, finding.to_string())
            })
            .collect()
    }

    /// Whether any of what was found is about this file.
    ///
    /// For a program that reads one of the two files loosely: tcsmoc shows
    /// what a simulator file says and runs without it, so a problem there is
    /// worth saying and not worth stopping for.
    pub fn found_errors_in(&self, file: &Path) -> bool {
        self.findings.iter().any(|finding| {
            finding.severity == Severity::Error && finding.file == file
        })
    }

    fn add(&mut self, file: &Path, line: usize, message: impl Into<String>) {
        self.found(file, line, message, Severity::Error, None)
    }

    /// Records a finding, unless the run has stopped or it is a warning and
    /// warnings are off, and stops the run if it reaches the limit.
    ///
    /// # Arguments
    ///
    /// * `file`     - The file the finding is about.
    /// * `line`     - The line in it, from 1.
    /// * `message`  - What is wrong, as it is printed.
    /// * `severity` - Whether it is an error or a warning.
    /// * `problem`  - The graph's problem it came from, where it came from one.
    fn found(
        &mut self,
        file: &Path,
        line: usize,
        message: impl Into<String>,
        severity: Severity,
        problem: Option<tcslibgs::graph::check::Problem>,
    ) {
        if self.halted {
            return;
        }
        if severity == Severity::Warning
            && self.limits.warnings == Warnings::Off
        {
            return;
        }
        self.findings.push(Finding {
            file: file.to_path_buf(),
            line,
            message: message.into(),
            severity,
            problem,
        });
        // Stopped at once on the last that counts, rather than on the one
        // after it: nothing is looked for that will not be printed.
        if self
            .limits
            .error_limit
            .is_some_and(|limit| self.counted() >= limit)
        {
            self.halted = true;
        }
    }

    /// Counts the findings that count against the limit.
    ///
    /// # Returns
    ///
    /// How many errors were found, and warnings too under
    /// [`Warnings::Errors`].
    fn counted(&self) -> usize {
        let errors_only = self.limits.warnings != Warnings::Errors;
        self.findings
            .iter()
            .filter(|finding| {
                !errors_only || finding.severity == Severity::Error
            })
            .count()
    }

    /// Makes an empty report for a run under these limits. A limit of nought
    /// is reached before anything is looked at.
    ///
    /// # Arguments
    ///
    /// * `limits` - The run's limits.
    ///
    /// # Returns
    ///
    /// The report, with nothing found.
    fn under(limits: Limits) -> Report {
        Report {
            halted: limits.error_limit == Some(0),
            limits,
            ..Report::default()
        }
    }

    fn room(&self) -> bool {
        !self.halted
    }
}

/// Checks a tcspecial configuration file, and a simulator file if there is
/// one.
///
/// A configuration is a graph of nodes, and is checked as one -- with the
/// rules tcspecial runs again aboard -- and the simulator file against the
/// graph's payloads by name.
///
/// # Arguments
///
/// * `payload_path` - The configuration.
/// * `sim_path`     - The simulator file, if there is one.
/// * `limits`       - tcsverify's `--error-limit` and `--warnings`, and the
///                    report passes or fails -- [`Report::status`] -- as
///                    tcsverify would under them. [`Limits::EVERYTHING`]
///                    looks at everything.
///
/// # Returns
///
/// Everything found, with its file and line.
pub fn verify(
    payload_path: &Path,
    sim_path: Option<&Path>,
    limits: Limits,
) -> Report {
    verify_built(
        payload_path,
        sim_path,
        limits,
        &tcslibgs::graph::built::OPTIONAL_TYPES,
    )
}

/// Checks a configuration as [`verify`] does, for a tcspecial containing only
/// the optional payload types `built` names.
///
/// Each node of a type `built` leaves out is reported as that tcspecial would
/// refuse it, in the same words: see [`tcslibgs::graph::built`]. Tcspecial
/// asks this with the types it was built with, and tcsverify with the types
/// `--types` names, so a configuration can be held to a vehicle's build on
/// the ground. [`verify`] is this with every type.
///
/// # Arguments
///
/// * `payload_path` - The configuration.
/// * `sim_path`     - The simulator file, if there is one.
/// * `limits`       - The run's limits, as for [`verify`].
/// * `built`        - The optional payload types the tcspecial has.
///
/// # Returns
///
/// Everything found, with its file and line.
pub fn verify_built(
    payload_path: &Path,
    sim_path: Option<&Path>,
    limits: Limits,
    built: &[&str],
) -> Report {
    let mut report = Report::under(limits);

    let graph = check_graph(payload_path, built, &mut report);
    if let Some(sim_path) = sim_path {
        // Against the graph's payloads, and only when the graph read: see the
        // module comment.
        report.joined = graph.is_some();
        check_graph_simulator(sim_path, graph.as_ref(), &mut report);
    }

    report
}

/// Checks two spellings of one configuration -- one XML, one YAML -- each
/// against the same simulator file, and then that they say the same thing.
///
/// Each is verified as [`verify`] verifies one, the first named first. Then
/// what each is read as is compared: every section, every attribute and every
/// value, in the form the programs act on -- see
/// `tcslibgs::config_digest::interpretation_of_file`, which is what a
/// configuration's digest is taken over. So two files this finds no
/// difference between are two files every program reads alike, and two whose
/// digests tcspecial and tcsmoc would agree on.
///
/// A difference is reported against the second file, at the line the thing
/// that differs is written on where it can be found, saying what each file
/// says. A file that cannot be read at all has nothing to compare, which is
/// said once and is already among its own findings.
///
/// # Arguments
///
/// * `first`    - The first spelling, verified first.
/// * `second`   - The second, which differences are reported against.
/// * `sim_path` - The simulator file both are checked with.
/// * `limits`   - The whole run's, both files and the comparison together:
///                a run that reaches its limit on the first file looks no
///                further.
/// * `built`    - The optional payload types of the tcspecial the two are
///                checked for, as in [`verify_built`].
///
/// # Returns
///
/// Everything found in either file and every difference between them.
pub fn verify_pair(
    first: &Path,
    second: &Path,
    sim_path: &Path,
    limits: Limits,
    built: &[&str],
) -> Report {
    let mut report = verify_built(first, Some(sim_path), limits, built);
    if report.halted {
        return report;
    }
    let then = verify_built(second, Some(sim_path), limits, built);
    report.joined = report.joined && then.joined;
    // Added under this run's limit, which is what stops it once the two
    // files together have reached it.
    for finding in then.findings {
        if report.halted {
            break;
        }
        report.found(
            &finding.file,
            finding.line,
            finding.message,
            finding.severity,
            finding.problem,
        );
    }
    // Nothing more is looked for once the run has stopped, the comparison
    // included.
    if !report.halted {
        compare(first, second, &mut report);
    }
    report
}

/// Checks whether two files are read as one configuration, every difference
/// found added to `report`. See [`verify_pair`].
///
/// # Arguments
///
/// * `first`  - One file.
/// * `second` - The other, which each difference is reported against, at
///              the line the thing that differs is written on.
/// * `report` - Where the differences, or that either file cannot be read,
///              are added.
pub fn compare(first: &Path, second: &Path, report: &mut Report) {
    use tcslibgs::config_digest::interpretation_of_file;

    let (one, two) = match (
        interpretation_of_file(first),
        interpretation_of_file(second),
    ) {
        (Ok(one), Ok(two)) => (one, two),
        (one, two) => {
            let unread: Vec<String> =
                [(first, one.is_err()), (second, two.is_err())]
                    .iter()
                    .filter(|(_, failed)| *failed)
                    .map(|(path, _)| path.display().to_string())
                    .collect();
            report.add(
                second,
                THE_FILE_ITSELF,
                format!(
                    "the two files are not compared, because {} cannot be read",
                    unread.join(" and ")
                ),
            );
            return;
        }
    };

    let mut differences = Vec::new();
    differences_between(&one, &two, &mut Vec::new(), &mut differences);
    let text = std::fs::read_to_string(second).unwrap_or_default();
    let (first_name, second_name) = (first.display(), second.display());
    for (path, said) in differences {
        let line = line_of_path(&text, &path);
        let message = match said {
            Said::Both(a, b) => format!(
                "{}: {first_name} says {a} and {second_name} says {b}",
                path.join(" / ")
            ),
            Said::OnlyFirst(Some(a)) => format!(
                "{}: {first_name} says {a} and {second_name} says nothing",
                path.join(" / ")
            ),
            Said::OnlyFirst(None) => format!(
                "{}: {first_name} has this and {second_name} does not",
                path.join(" / ")
            ),
            Said::OnlySecond(Some(b)) => format!(
                "{}: {first_name} says nothing and {second_name} says {b}",
                path.join(" / ")
            ),
            Said::OnlySecond(None) => format!(
                "{}: {first_name} does not have this and {second_name} does",
                path.join(" / ")
            ),
            Said::Order(a, b) => format!(
                "{}: {first_name} gives them in the order {a} and \
                 {second_name} in the order {b}, and the order is part of \
                 what they mean",
                path.join(" / ")
            ),
        };
        report.add(second, line, message);
    }
}

/// What two files say about one thing they differ on.
enum Said {
    Both(String, String),
    /// Said by one file only: what it says, where that is a value, and
    /// `None` where it is a whole entry -- a node, a section -- which is
    /// named by the path and would be a page if it were printed.
    OnlyFirst(Option<String>),
    OnlySecond(Option<String>),
    /// The same entries, in a different order.
    Order(String, String),
}

/// Shows a value as a reader would write it in a message.
///
/// # Arguments
///
/// * `value` - The value.
///
/// # Returns
///
/// A string in double quotes, anything else as JSON.
fn shown(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => format!("\"{text}\""),
        other => other.to_string(),
    }
}

/// Shows a value said by one file only: itself where it is one, nothing
/// where it is a whole entry.
///
/// # Arguments
///
/// * `value` - The value.
///
/// # Returns
///
/// The value as [`shown`] gives it, or `None` for an object or a list.
fn alone(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => None,
        other => Some(shown(other)),
    }
}

/// Finds what tells the entries of a list apart, where something does: a
/// node by its name, a link by where it leaves from.
///
/// # Arguments
///
/// * `list` - The list's entries.
///
/// # Returns
///
/// Each entry's `name`, or failing that its `from`, in order; `None` for a
/// list whose entries have no such key, or whose keys repeat, which is then
/// compared place by place.
fn keys_of(list: &[serde_json::Value]) -> Option<Vec<String>> {
    for key in ["name", "from"] {
        let keys: Option<Vec<String>> = list
            .iter()
            .map(|entry| {
                entry.get(key).and_then(|k| k.as_str()).map(str::to_string)
            })
            .collect();
        if let Some(keys) = keys {
            let mut unique = keys.clone();
            unique.sort();
            unique.dedup();
            if unique.len() == keys.len() {
                return Some(keys);
            }
        }
    }
    None
}

/// Finds every place two value trees differ, each with the path to it.
///
/// Objects are compared key by key, lists of named entries name by name and
/// then for order, other lists place by place. A key that is null on one
/// side and missing on the other is no difference.
///
/// # Arguments
///
/// * `one`  - What the first file is read as.
/// * `two`  - What the second is read as.
/// * `path` - The path to `one` and `two`, as it is built up; left as it was
///            found.
/// * `out`  - Where each difference is added, with its path.
fn differences_between(
    one: &serde_json::Value,
    two: &serde_json::Value,
    path: &mut Vec<String>,
    out: &mut Vec<(Vec<String>, Said)>,
) {
    use serde_json::Value;

    match (one, two) {
        (Value::Object(a), Value::Object(b)) => {
            let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                path.push(key.clone());
                match (a.get(key), b.get(key)) {
                    (Some(x), Some(y)) => differences_between(x, y, path, out),
                    (Some(x), None) if !x.is_null() => {
                        out.push((path.clone(), Said::OnlyFirst(alone(x))))
                    }
                    (None, Some(y)) if !y.is_null() => {
                        out.push((path.clone(), Said::OnlySecond(alone(y))))
                    }
                    _ => {}
                }
                path.pop();
            }
        }
        (Value::Array(a), Value::Array(b)) => match (keys_of(a), keys_of(b)) {
            // Entries with names: matched by name, so one entry missing is
            // one difference rather than every entry after it.
            (Some(ka), Some(kb)) => {
                for (key, x) in ka.iter().zip(a) {
                    path.push(key.clone());
                    match kb.iter().position(|k| k == key) {
                        Some(at) => differences_between(x, &b[at], path, out),
                        None => {
                            out.push((path.clone(), Said::OnlyFirst(alone(x))))
                        }
                    }
                    path.pop();
                }
                for (key, y) in kb.iter().zip(b) {
                    if !ka.contains(key) {
                        path.push(key.clone());
                        out.push((path.clone(), Said::OnlySecond(alone(y))));
                        path.pop();
                    }
                }
                // And the order, which is part of what they mean: a node's
                // place is its device id.
                let common = |keys: &[String],
                              other: &[String]|
                 -> Vec<String> {
                    keys.iter().filter(|k| other.contains(k)).cloned().collect()
                };
                let (oa, ob) = (common(&ka, &kb), common(&kb, &ka));
                if oa != ob {
                    out.push((
                        path.clone(),
                        Said::Order(oa.join(", "), ob.join(", ")),
                    ));
                }
            }
            _ => {
                for at in 0..a.len().max(b.len()) {
                    path.push(at.to_string());
                    match (a.get(at), b.get(at)) {
                        (Some(x), Some(y)) => {
                            differences_between(x, y, path, out)
                        }
                        (Some(x), None) => {
                            out.push((path.clone(), Said::OnlyFirst(alone(x))))
                        }
                        (None, Some(y)) => {
                            out.push((path.clone(), Said::OnlySecond(alone(y))))
                        }
                        (None, None) => {}
                    }
                    path.pop();
                }
            }
        },
        (x, y) if x != y => {
            out.push((path.clone(), Said::Both(shown(x), shown(y))))
        }
        _ => {}
    }
}

/// Finds the line a path's last named part is written on, searching from
/// where each part before it was found -- the node, then the attribute under
/// it.
///
/// Only the parts a file writes are searched for: a node's name, an
/// attribute's, a section's. The comparison's own words for where it is --
/// `nodes`, `attributes`, a place in a list -- are not in a file, or are in it
/// somewhere else (the editor's own section has `nodes`), and finding one
/// would send a reader to the wrong line. A link's `node.terminal` is the node
/// and then the terminal under it, which is how both formats write it.
///
/// # Arguments
///
/// * `text` - The file's text.
/// * `path` - The path to the thing, as [`differences_between`] gives it.
///
/// # Returns
///
/// The line, from 1; the file's first line where nothing is found.
fn line_of_path(text: &str, path: &[String]) -> usize {
    const NOT_WRITTEN: [&str; 4] = ["nodes", "attributes", "links", "to"];
    let lines: Vec<&str> = text.lines().collect();
    let mut from = 0;
    let mut found = None;
    let parts = path
        .iter()
        .filter(|part| {
            part.parse::<usize>().is_err()
                && !NOT_WRITTEN.contains(&part.as_str())
        })
        .flat_map(|part| {
            part.split('.').map(str::to_string).collect::<Vec<_>>()
        });
    for part in parts {
        // A whole name, outside a comment: `heater` is not found in
        // `heaters`, and a comment saying what an attribute is for is not
        // where it is set.
        let names = |line: &&str| {
            let said = line.trim_start();
            if said.starts_with('#') || said.starts_with("<!--") {
                return false;
            }
            let in_a_name =
                |c: char| c.is_alphanumeric() || c == '_' || c == '-';
            line.match_indices(part.as_str()).any(|(at, _)| {
                let before = line[..at].chars().next_back();
                let after = line[at + part.len()..].chars().next();
                !before.is_some_and(in_a_name) && !after.is_some_and(in_a_name)
            })
        };
        if let Some(at) = lines[from..].iter().position(names) {
            from += at;
            found = Some(from + 1);
        }
    }
    found.unwrap_or(THE_FILE_ITSELF)
}

/// Checks a configuration: reads it, then asks what is wrong with it.
///
/// The checking is `tcslibgs::graph::Graph::check`, which is the same code
/// tcspecial runs aboard. This adds only the line numbers, which it gets by
/// scanning the text for the name the problem is about.
///
/// # Arguments
///
/// * `path`   - The configuration's top file.
/// * `built`  - The optional payload types of the tcspecial it is checked for.
/// * `report` - Where each problem is added, at its own file and line.
///
/// # Returns
///
/// The graph, for a simulator file to be joined to, when it read at all.
fn check_graph(
    path: &Path,
    built: &[&str],
    report: &mut Report,
) -> Option<tcslibgs::graph::Graph> {
    let graph = match tcslibgs::graph::Graph::load(path) {
        Ok(graph) => graph,
        Err(e) => {
            // A file that would not load at all: reported from where the
            // reader stopped if it said, and from the first line otherwise.
            report.add(path, line_in_message(&e.to_string()), e.to_string());
            return None;
        }
    };

    // Where each name is written, per file: a configuration made of several
    // files has a node in one of them, and a problem about it belongs at that
    // file's line and not at the top file's.
    let mut located: Vec<(PathBuf, crate::locate::Where)> = Vec::new();
    for file in &graph.files {
        let Ok(format) = ConfigFormat::of_file(file) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        located.push((file.clone(), crate::locate::Where::of(&text, format)));
    }

    // The build's types after the graph's own rules, being asked of a graph
    // that is otherwise right: a node of a misspelled type is reported as
    // unknown and needs no build.
    let problems = graph
        .check()
        .into_iter()
        .chain(tcslibgs::graph::built::not_built(&graph.nodes, built));
    for problem in problems {
        if !report.room() {
            break;
        }
        let about = problem.about();

        // The file the thing is in, where the problem is about a node; the
        // top file otherwise, which is where the links and commands are.
        let home = match &about {
            tcslibgs::About::Node(name) => graph.homes.get(name).cloned(),
            _ => None,
        }
        .unwrap_or_else(|| path.to_path_buf());

        let line = located
            .iter()
            .find(|(file, _)| *file == home)
            .map(|(_, where_)| where_.line_of(&about))
            .unwrap_or(crate::locate::THE_FILE_ITSELF);

        report.found(
            &home,
            line,
            problem.describe(),
            problem.severity(),
            Some(problem.clone()),
        );
    }

    Some(graph)
}

/// Checks a configuration's simulator file, against the configuration's
/// payloads if the configuration read.
///
/// By name first: see `tcslibgs::SimConfigFile::problems_by_name`. Then, where
/// the names are all right, what tcssim and tcsmoc ask before starting a
/// simulator, `tcslibgs::graph::simulated::handlers`, and what tcssim asks of
/// each payload's settings against its handler,
/// `tcslibgs::SimConfigFile::problems`: a file that passed here and was refused
/// there would be a verifier saying yes to a run that fails.
///
/// # Arguments
///
/// * `path`   - The simulator file.
/// * `graph`  - The configuration, or `None` if it did not read, when the
///              file is only parsed.
/// * `report` - Where each problem is added.
fn check_graph_simulator(
    path: &Path,
    graph: Option<&tcslibgs::graph::Graph>,
    report: &mut Report,
) {
    let Some((text, format)) = read(path, report) else {
        return;
    };

    let sim: SimConfigFile = match format.parse(&text) {
        Ok(sim) => sim,
        Err(parse_error) => {
            report.add(path, line_of(&parse_error), parse_error.to_string());
            return;
        }
    };

    let Some(graph) = graph else { return };
    let names: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.is_payload())
        .map(|node| node.name.as_str())
        .collect();

    let found = Where::of(&text, format);
    let by_name = sim.problems_by_name(&names);
    let named_well = by_name.is_empty();
    for problem in by_name {
        if !report.room() {
            break;
        }
        report.add(path, found.line_of(&problem.about()), problem.to_string());
    }

    // One answer and not all of them: the simulator stops at the first
    // payload it will not stand in for, and says which, as the start would.
    if named_well && report.room() {
        match tcslibgs::graph::simulated::handlers(graph, &sim) {
            Err(why) => {
                // The complaint names the payload first, in quotes, which is
                // the entry to point at.
                let line = why
                    .strip_prefix('"')
                    .and_then(|rest| rest.split_once('"'))
                    .map_or(THE_FILE_ITSELF, |(payload, _)| {
                        found.line_of(&tcslibgs::About::SimPayload(
                            payload.to_string(),
                        ))
                    });
                report.add(path, line, why);
            }
            Ok(handlers) => {
                for problem in sim.problems(&handlers) {
                    if !report.room() {
                        break;
                    }
                    report.add(
                        path,
                        found.line_of(&problem.about()),
                        problem.to_string(),
                    );
                }
            }
        }
    }
}

/// Reads a file, and the format its name says it is written in.
///
/// # Arguments
///
/// * `path`   - The file.
/// * `report` - Where it is reported if either cannot be had.
///
/// # Returns
///
/// The file's text and its format, or `None` (with the reason in `report`).
fn read(path: &Path, report: &mut Report) -> Option<(String, ConfigFormat)> {
    let format = match ConfigFormat::of_file(path) {
        Ok(format) => format,
        Err(e) => {
            report.add(path, THE_FILE_ITSELF, e.to_string());
            return None;
        }
    };

    match std::fs::read_to_string(path) {
        Ok(text) => Some((text, format)),
        Err(e) => {
            report.add(path, THE_FILE_ITSELF, e.to_string());
            None
        }
    }
}

/// Finds the line a message mentions, if it mentions one.
///
/// A parse error carries its location in a field, and `line_of` reads it
/// there. A graph that would not load reports through `TcsError::Config`,
/// which is a sentence -- the path has been put in front of the parser's own
/// words, which is worth more than the field was -- so the line is read back
/// out of the text. "at line 7 column 5" is what both parsers write.
///
/// # Arguments
///
/// * `message` - The message.
///
/// # Returns
///
/// The number after the first "at line ", or the file's first line where
/// there is none.
fn line_in_message(message: &str) -> usize {
    message
        .split("at line ")
        .nth(1)
        .and_then(|rest| {
            rest.split(|c: char| !c.is_ascii_digit())
                .find(|piece| !piece.is_empty())
        })
        .and_then(|digits| digits.parse().ok())
        .unwrap_or(THE_FILE_ITSELF)
}

/// Finds where a parser stopped.
///
/// That is the line a reader has to look at: the file does not parse, so
/// nothing in it has a name yet to be found by. Both parsers say -- YAML in
/// its error, XML by the reader's position, which `ConfigFormat::parse` puts
/// in its error -- and anything else is reported from the first line.
///
/// # Arguments
///
/// * `e` - The parser's error.
///
/// # Returns
///
/// The line, from 1.
fn line_of(e: &TcsError) -> usize {
    match e {
        TcsError::Yaml(e) => {
            e.location().map_or(THE_FILE_ITSELF, |at| at.line())
        }
        TcsError::Xml { line, .. } => *line,
        _ => THE_FILE_ITSELF,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslibgs::graph::built::OPTIONAL_TYPES as ALL;

    /// Copies the thermostat example's two spellings and its simulator file
    /// to a directory of their own, with `change` made to the XML.
    ///
    /// # Arguments
    ///
    /// * `change` - Takes the XML's text and gives what is written instead.
    ///
    /// # Returns
    ///
    /// The directory, which is removed when dropped, and the paths of the
    /// XML, the YAML and the simulator file in it.
    fn thermostat_pair(
        change: impl Fn(String) -> String,
    ) -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../examples/thermostat");
        let dir = tempfile::tempdir().expect("a directory");
        let at = |name: &str| dir.path().join(name);
        for name in ["thermostat.yaml", "thermostat-sim.yaml"] {
            std::fs::copy(root.join(name), at(name)).expect("copied");
        }
        let xml = std::fs::read_to_string(root.join("thermostat.xml"))
            .expect("the XML");
        std::fs::write(at("thermostat.xml"), change(xml)).expect("written");
        let (xml, yaml, sim) = (
            at("thermostat.xml"),
            at("thermostat.yaml"),
            at("thermostat-sim.yaml"),
        );
        (dir, xml, yaml, sim)
    }

    /// A configuration checked for a build without one of its types reports
    /// each node of that type, at the line the node is written on, in the
    /// words tcspecial uses to refuse it -- and nothing else.
    #[test]
    fn a_node_of_a_type_not_built_is_reported_where_it_is_written() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../examples/thermostat");
        let yaml = root.join("thermostat.yaml");
        let report =
            verify_built(&yaml, None, Limits::EVERYTHING, &["i2c", "spi"]);
        let found: Vec<(usize, &str)> = report
            .findings
            .iter()
            .map(|finding| (finding.line, finding.message.as_str()))
            .collect();
        let text = std::fs::read_to_string(&yaml).expect("the example");
        let line_of = |name: &str| {
            1 + text
                .lines()
                .position(|line| line.trim() == format!("- name: {name}"))
                .expect("the node is in the example")
        };
        assert_eq!(
            found,
            vec![
                (
                    line_of("heaters_1"),
                    tcslibgs::graph::built::not_built_message(
                        "heaters_1",
                        "gpio"
                    )
                    .as_str()
                ),
                (
                    line_of("heaters_2"),
                    tcslibgs::graph::built::not_built_message(
                        "heaters_2",
                        "gpio"
                    )
                    .as_str()
                ),
            ]
        );
        assert!(report.found_errors(), "a build that cannot fly it fails");
        // Every type built is what verify asks for, and finds nothing.
        assert!(verify(&yaml, None, Limits::EVERYTHING).findings.is_empty());
    }

    /// Two spellings of one configuration: each checked, nothing different.
    #[test]
    fn two_spellings_of_one_configuration_compare_alike() {
        let (_dir, xml, yaml, sim) = thermostat_pair(|xml| xml);
        let report = verify_pair(&xml, &yaml, &sim, Limits::EVERYTHING, &ALL);
        assert!(report.findings.is_empty(), "{:?}", report.said());
        assert!(report.joined, "both were checked with the simulator file");
        // Either order.
        assert!(verify_pair(&yaml, &xml, &sim, Limits::EVERYTHING, &ALL)
            .findings
            .is_empty());
    }

    /// A value changed in one spelling is reported: where, and what each
    /// file says, at the line the second file sets it on.
    #[test]
    fn a_value_one_spelling_changes_is_reported() {
        let (_dir, xml, yaml, sim) = thermostat_pair(|xml| {
            xml.replacen("<setpoint>293.15", "<setpoint>290", 1)
        });
        let report = verify_pair(&yaml, &xml, &sim, Limits::EVERYTHING, &ALL);
        let said = report.said();
        assert_eq!(report.findings.len(), 1, "{said:?}");
        let finding = &report.findings[0];
        assert_eq!(finding.file, xml);
        assert!(
            finding
                .message
                .contains("nodes / thermostat / attributes / setpoint")
                && finding.message.contains("says \"293.15\"")
                && finding.message.contains("says \"290\""),
            "{said:?}"
        );
        let text = std::fs::read_to_string(&xml).expect("the XML");
        let line = text.lines().nth(finding.line - 1).expect("a line");
        assert!(
            line.contains("<setpoint>290"),
            "line {}: {line}",
            finding.line
        );
        assert_eq!(report.status(), 1);
    }

    /// A node renamed in one spelling is one node missing from each, and a
    /// line to it that names it differently -- not a page of differences for
    /// every node after it.
    #[test]
    fn a_node_one_spelling_renames_is_missing_from_the_other() {
        // Compared and not verified, so the simulator file is not wanted.
        let (_dir, xml, yaml, _) = thermostat_pair(|xml| {
            xml.replace("<name>heaters_1</name>", "<name>heater</name>")
                .replace(
                    "<target>heaters_1</target>",
                    "<target>heater</target>",
                )
        });
        let mut report = Report::default();
        compare(&yaml, &xml, &mut report);
        let said: Vec<&str> =
            report.findings.iter().map(|f| f.message.as_str()).collect();
        assert!(
            said.iter()
                .any(|m| m.contains("nodes / heaters_1:")
                    && m.contains("does not")),
            "{said:?}"
        );
        assert!(
            said.iter().any(|m| m.contains("nodes / heater:")),
            "{said:?}"
        );
        assert!(
            said.iter().any(
                |m| m.contains("\"heaters_1\"") && m.contains("\"heater\"")
            ),
            "{said:?}"
        );
        assert!(said.len() <= 4, "{said:?}");
    }

    /// The order nodes are written in is part of what they mean -- a node's
    /// place is its device id -- so two spellings that give them in two
    /// orders differ.
    #[test]
    fn nodes_in_another_order_differ() {
        let one =
            serde_json::json!({ "nodes": [{ "name": "a" }, { "name": "b" }] });
        let two =
            serde_json::json!({ "nodes": [{ "name": "b" }, { "name": "a" }] });
        let mut out = Vec::new();
        differences_between(&one, &two, &mut Vec::new(), &mut out);
        assert_eq!(out.len(), 1);
        assert!(
            matches!(&out[0].1, Said::Order(a, b) if a == "a, b" && b == "b, a")
        );
    }

    /// A file in a temporary directory, so the findings name a real path.
    struct Set {
        _dir: tempfile::TempDir,
        payload: PathBuf,
        sim: PathBuf,
    }

    /// Writes a configuration and a simulator file to a temporary directory.
    ///
    /// # Arguments
    ///
    /// * `payload_ext` - The configuration's extension, with its dot, which
    ///                   says its format.
    /// * `payload`     - The configuration's text.
    /// * `sim`         - The simulator file's text, written as YAML.
    ///
    /// # Returns
    ///
    /// The two paths, and the directory they are in.
    fn written(payload_ext: &str, payload: &str, sim: &str) -> Set {
        let dir = tempfile::tempdir().expect("a directory");
        let payload_path = dir.path().join(format!("set{payload_ext}"));
        let sim_path = dir.path().join("setsim.yaml");
        std::fs::File::create(&payload_path)
            .expect("the tcspecial configuration")
            .write_all(payload.as_bytes())
            .expect("written");
        std::fs::File::create(&sim_path)
            .expect("the simulator file")
            .write_all(sim.as_bytes())
            .expect("written");
        Set {
            _dir: dir,
            payload: payload_path,
            sim: sim_path,
        }
    }

    /// Writes a configuration to a temporary directory.
    ///
    /// # Arguments
    ///
    /// * `extension` - The file's extension, with its dot, which says its
    ///                 format.
    /// * `text`      - What it says.
    ///
    /// # Returns
    ///
    /// The directory, which is removed when dropped, and the file's path.
    fn graph_written(
        extension: &str,
        text: &str,
    ) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join(format!("vehicle{extension}"));
        std::fs::write(&path, text).expect("written");
        (dir, path)
    }

    const SOUND_GRAPH: &str = "\
tcspecial:
  gateway: radio
  state_file: /tmp/tcspecial-test.state
  tcs_cmd:
    recv: 0.0.0.0:4000
    send: 127.0.0.1:4100
  payload_command:
    source_base: 127.0.0.1:4300
    dest_base: 0.0.0.0:4200
  payload_telemetry:
    source_base: 0.0.0.0:6000
    dest_base: 127.0.0.1:6100
  beacon:
    default_interval: 5 s
    reset_interval: 5 s
    send: \"0.0.0.0:5550\"
    recv: \"127.0.0.1:5650\"
payload_groups:
  - name: temps
    interval: \"3 s\"
payloads:
  - { name: sensor, type: i2c, group: temps, path: \"/dev/i2c-1\", \
address: \"0x48\", payload-out: gateway }
gateway:
  - { name: gateway, type: gateway_radio, address: \"10.0.0.1\", \
port: \"5000\", ground_base: \"10.0.0.2:5100\" }
";

    /// A simulator file that names a payload the simulator will not stand in
    /// for is refused here, as tcssim and tcsmoc refuse it, on its line.
    ///
    /// A heater is driven and never read, so its stand-in sends nothing, and a
    /// packet interval given it governs nothing: the simulator refuses it, and
    /// a verifier that said nothing would pass a run that fails.
    #[test]
    fn a_simulated_payload_the_simulator_refuses_is_an_error() {
        // The sound graph with a heater the ground drives through a tee.
        let configuration = format!(
            "{SOUND_GRAPH}vehicle:
  - {{ name: echo, type: tee-gateway, gateway-out: heater }}
"
        )
        .replace(
            "payload-out: gateway }\n",
            "payload-out: gateway }\n  - { name: heater, type: network, \
             protocol: udp, address: \"127.0.0.1\", port: \"8001\" }\n",
        )
        .replace(
            "ground_base: \"10.0.0.2:5100\" }",
            "ground_base: \"10.0.0.2:5100\", gateway-out: echo }",
        );
        let sim = "\
simulated_payloads:
  - name: sensor
    simulate: true
    type: i2c
    packet_interval: 1 s
  - name: heater
    simulate: true
    type: network
    protocol: udp
    packet_interval: 1 s
";
        let set = written(".yaml", &configuration, sim);
        let report = verify(&set.payload, Some(&set.sim), Limits::EVERYTHING);
        let said: Vec<&Finding> = report
            .findings
            .iter()
            .filter(|f| f.file == set.sim)
            .collect();
        assert_eq!(said.len(), 1, "{:?}", report.findings);
        assert!(
            said[0].message.contains("\"heater\" is only driven"),
            "{}",
            said[0].message
        );
        assert_eq!(said[0].line, 6, "the heater's entry");

        // And without the interval, nothing: a driven payload is simulated,
        // its stand-in counting what the vehicle sends it.
        let set = written(
            ".yaml",
            &configuration,
            &sim.replace(
                "    protocol: udp\n    packet_interval: 1 s\n",
                "    protocol: udp\n",
            ),
        );
        let report = verify(&set.payload, Some(&set.sim), Limits::EVERYTHING);
        assert!(
            !report.findings.iter().any(|f| f.file == set.sim),
            "{:?}",
            report.findings
        );
    }

    /// A sound configuration is accepted in silence.
    #[test]
    fn a_sound_graph_is_accepted() {
        let (_dir, path) = graph_written(".yaml", SOUND_GRAPH);
        let report = verify(&path, None, Limits::EVERYTHING);
        assert!(
            !report.found_errors(),
            "{:?}",
            report
                .findings
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(report.status(), 0);
    }

    /// A sound configuration is accepted on its own, and with a simulator
    /// file beside it.
    #[test]
    fn a_sound_configuration_is_accepted_alone_and_with_its_simulator_file() {
        let (_dir, path) = graph_written(".yaml", SOUND_GRAPH);
        let report = verify(&path, None, Limits::EVERYTHING);
        assert!(!report.found_errors());

        let set = written(".yaml", GOOD, GOOD_SIM);
        let report = verify(&set.payload, Some(&set.sim), Limits::EVERYTHING);
        assert!(!report.found_errors(), "{:?}", report.findings);
    }

    /// Each problem a graph has is reported, with the line to look at.
    #[test]
    fn a_graphs_problems_are_reported_with_their_lines() {
        let (_dir, path) = graph_written(
            ".yaml",
            "\
payload_groups:
  - name: temps
    interval: soon
payloads:
  - { name: a, type: i2c, group: temps, path: \"/dev/i2c-1\", \
payload-out: nowhere }
  - { name: a, type: spi, path: \"/dev/spidev0.0\" }
vehicle:
  - { name: m, type: compute, function: mean, inputs: \"in0\" }
",
        );

        let report = verify(&path, None, Limits::EVERYTHING);
        let said: Vec<String> =
            report.findings.iter().map(|f| f.to_string()).collect();
        let all = said.join("\n");

        assert!(all.contains("two nodes are called \"a\""), "{all}");
        assert!(all.contains("is not a length of time"), "{all}");
        assert!(all.contains("there is no node called \"nowhere\""), "{all}");
        assert_eq!(report.status(), 1);

        // Every finding points at a line of the file rather than at line
        // nought, which is what a reader is given to act on.
        for finding in &report.findings {
            assert!(finding.line >= 1, "{finding}");
            assert_eq!(finding.file, path, "{finding}");
        }
    }

    /// A file still written in a word that was renamed is reported at the
    /// line the word is on, with the word to write instead, rather than at
    /// the top of the file in the parser's words.
    #[test]
    fn an_old_word_is_reported_at_its_line_with_the_new_one() {
        let dir = tempfile::tempdir().expect("a directory");
        let top = dir.path().join("vehicle.yaml");
        std::fs::write(
            &top,
            "description: an old file\n\
             moc:\n  \
             - { name: down, type: gateway_radio }\n",
        )
        .unwrap();

        let report = verify(&top, None, Limits::EVERYTHING);
        let old = report
            .findings
            .iter()
            .find(|f| f.message.contains("rename it"))
            .expect("the old section should be reported");
        assert_eq!(old.line, 2, "{old}");
        assert!(old.message.contains("is called gateway now"), "{old}");
    }

    /// A problem about a node is reported in the file the node is in.
    ///
    /// A configuration made of several files has its nodes spread across them,
    /// and a complaint pointing at the top file's line would send a reader to
    /// the wrong page.
    #[test]
    fn a_problem_is_reported_in_the_file_it_is_in() {
        let dir = tempfile::tempdir().expect("a directory");
        let top = dir.path().join("vehicle.yaml");
        std::fs::write(
            &top,
            "include:\n  - cabin\ngateway:\n  \
             - { name: gateway, type: gateway_radio, gateway-out: gateway }\n",
        )
        .unwrap();
        // The included file holds a node nothing links to.
        std::fs::write(
            dir.path().join("cabin.yaml"),
            "payloads:\n  - { name: lonely, type: i2c }\n",
        )
        .unwrap();

        let report = verify(&top, None, Limits::EVERYTHING);
        let lonely = report
            .findings
            .iter()
            .find(|f| f.message.contains("lonely"))
            .expect("the unconnected node should be reported");
        assert!(
            lonely.file.ends_with("cabin.yaml"),
            "it should be reported in the file it is in, not {}",
            lonely.file.display()
        );
        assert_eq!(lonely.line, 2, "{lonely}");
    }

    /// A configuration's simulator file is joined to it by name.
    ///
    /// The rule the editor holds a simulator file to as it opens one, asked
    /// here of the same pair: an entry for a payload the graph does not have
    /// is somebody's misspelling, and drives nothing.
    #[test]
    fn a_graphs_simulator_file_names_its_payloads() {
        let (dir, path) = graph_written(".yaml", SOUND_GRAPH);
        let sim = dir.path().join("vehiclesim.yaml");

        std::fs::write(
            &sim,
            "simulated_payloads:\n  - name: sensor\n    simulate: true\n    \
             type: i2c\n    packet_interval: 1 s\n",
        )
        .unwrap();
        let report = verify(&path, Some(&sim), Limits::EVERYTHING);
        assert!(!report.found_anything(), "{:?}", report.findings);
        assert!(report.joined);

        std::fs::write(
            &sim,
            "simulated_payloads:\n  - name: sensor\n    simulate: true\n  \
             - name: sensr\n    group: none\n",
        )
        .unwrap();
        let report = verify(&path, Some(&sim), Limits::EVERYTHING);
        let said: Vec<String> =
            report.findings.iter().map(|f| f.to_string()).collect();
        let all = said.join("\n");
        assert!(all.contains("\"sensr\" is simulated"), "{all}");
        assert!(all.contains("names group \"none\""), "{all}");
        for finding in &report.findings {
            assert_eq!(finding.file, sim, "{finding}");
            assert_eq!(finding.line, 4, "{finding}");
        }
    }

    /// The example's simulator file names only payloads the example has.
    ///
    /// The example itself is asked nothing here: it is a drawing somebody is
    /// working on, and what is unfinished in it is not this test's business.
    #[test]
    fn the_shipped_examples_simulator_file_fits_it() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let sim = root.join("example-sim.yaml");
        let report =
            verify(&root.join("example.yaml"), Some(&sim), Limits::EVERYTHING);
        assert!(report.joined);
        let about_sim: Vec<String> = report
            .findings
            .iter()
            .filter(|f| f.file == sim)
            .map(|f| f.to_string())
            .collect();
        assert!(about_sim.is_empty(), "{about_sim:?}");
    }

    /// The same configuration in XML is checked the same way.
    #[test]
    fn a_graph_in_xml_is_checked_too() {
        let (_dir, path) = graph_written(
            ".xml",
            "\
<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<configuration>
    <payloads>
        <name>a</name>
        <type>i2c</type>
        <path>/dev/i2c-1</path>
        <payload-out>nowhere</payload-out>
    </payloads>
</configuration>
",
        );

        let report = verify(&path, None, Limits::EVERYTHING);
        let said = report
            .findings
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(said.contains("nowhere"), "{said}");
    }

    /// A line mentioned in a message is the line reported.
    #[test]
    fn a_line_is_read_out_of_a_parsers_words() {
        assert_eq!(line_in_message("x: YAML error: bad at line 7 column 5"), 7);
        assert_eq!(line_in_message("nothing about a line"), THE_FILE_ITSELF);
        assert_eq!(line_in_message(""), THE_FILE_ITSELF);
    }

    /// A configuration with nothing wrong with it, and nothing in it but what
    /// every configuration must have: the `tcspecial` section and a payload.
    ///
    /// Two payloads, neither touched by a link, which report to the ground
    /// over `payload_telemetry` -- the shape of `examples/minimal`. The tests
    /// below are about the reporting rather than about any one kind of node,
    /// so what they are given is the simplest thing that reads.
    const GOOD: &str = "\
description: two payloads
tcspecial:
  gateway: radio
  state_file: /tmp/tcspecial-test.state
  beacon:
    default_interval: \"5 s\"
    reset_interval: \"5 s\"
    recv: \"127.0.0.1:5650\"
    send: \"0.0.0.0:5550\"
  log:
    directory: \"/tmp/telemetry\"
    segment_size: \"65536\"
  payload_command:
    dest_base: \"0.0.0.0:4200\"
    source_base: \"127.0.0.1:4300\"
  payload_telemetry:
    dest_base: \"127.0.0.1:6100\"
    source_base: \"0.0.0.0:6000\"
  tcs_cmd:
    recv: \"0.0.0.0:4000\"
    send: \"127.0.0.1:4100\"
payloads:
  - name: DH0
    type: device-file
    path: /dev/zero
    read_size: \"4\"
  - name: DH1
    type: network
    protocol: udp
    address: \"127.0.0.1\"
    port: \"5000\"
";

    const GOOD_SIM: &str = "\
description: settings
simulated_payloads:
  - name: DH0
    simulate: true
    type: device-file
    packet_interval: 1 s
  - name: DH1
    simulate: true
    type: network
    protocol: udp
    packet_interval: 1 s
";

    #[test]
    fn a_good_set_has_nothing_to_say() {
        let set = written(".yaml", GOOD, GOOD_SIM);
        let report = verify(&set.payload, Some(&set.sim), Limits::EVERYTHING);
        assert!(
            report.findings.is_empty(),
            "a good set was faulted: {:?}",
            report.findings
        );
        assert_eq!(report.status(), 0);
        assert!(
            report.joined,
            "the two files were not checked against each other"
        );
    }

    /// Every payload is reported, not only the first.
    ///
    /// This is the whole point of the program: a program that loads the file
    /// stops at the first of these, so a set with three mistakes took three
    /// runs to find them.
    #[test]
    fn every_payload_with_something_wrong_is_reported() {
        // Three links to nodes that are not there, which is an error apiece.
        // It was three payloads each missing something its kind needs, which
        // in a graph is a warning rather than an error: an unfinished node is
        // a drawing somebody is working on. What the file is wrong about has
        // changed; that every one of them is reported has not.
        let text = GOOD.replace(
            "payloads:",
            "\
payloads:
  - name: first
    type: device-file
    path: /dev/urandom
    payload-out:
      - target: nowhere
  - name: second
    type: device-file
    path: /dev/full
    payload-out:
      - target: elsewhere
  - name: third
    type: device-file
    path: /dev/random
    payload-out:
      - target: anywhere
# what GOOD's own payloads were:",
        );
        let set = written(".yaml", &text, GOOD_SIM);
        let report = verify(&set.payload, None, Limits::EVERYTHING);

        assert_eq!(report.findings.len(), 3, "{:?}", report.findings);
        assert_eq!(report.status(), 1);

        // Each beside the node it is about, in the order the file wrote them.
        let lines: Vec<usize> =
            report.findings.iter().map(|f| f.line).collect();
        assert_eq!(lines, vec![23, 28, 33], "{:?}", report.findings);
        for (finding, missing) in
            report
                .findings
                .iter()
                .zip(["nowhere", "elsewhere", "anywhere"])
        {
            assert!(
                finding.message.contains(missing),
                "a finding should name what the link could not find: \
                 {finding:?}"
            );
        }
    }

    /// A finding says the file, the line, which kind it is and what is wrong.
    ///
    /// The shape an editor and a build log both read: `file:line: error:
    /// message`.
    #[test]
    fn a_finding_names_the_file_and_the_line() {
        let finding = Finding {
            file: PathBuf::from("tests/manual/tcspecial2.yaml"),
            line: 7,
            message: "payload \"DH0\" has no packet_size".to_string(),
            severity: Severity::Error,
            problem: None,
        };
        assert_eq!(
            finding.to_string(),
            "tests/manual/tcspecial2.yaml:7: error: payload \"DH0\" has no \
             packet_size"
        );

        // And a warning says which it is in the same place, so that the two
        // are read without counting colons.
        let unfinished = Finding {
            severity: Severity::Warning,
            ..finding.clone()
        };
        assert!(
            unfinished.to_string().contains(":7: warning: "),
            "{unfinished}"
        );

        // And a real one is written the same way, by the same Display.
        let set = written(
            ".yaml",
            &GOOD.replace("    path: /dev/zero\n", ""),
            GOOD_SIM,
        );
        let report = verify(&set.payload, None, Limits::EVERYTHING);
        let first =
            report.findings.first().expect("something is wrong with it");
        assert!(
            first.to_string().starts_with(&format!(
                "{}:{}: {}: ",
                set.payload.display(),
                first.line,
                first.severity.label()
            )),
            "a finding must lead with its file and line: {first}"
        );
        assert!(
            first.line > 1,
            "it must name the payload's own line: {first}"
        );
    }

    /// A file that does not parse is reported from where the parser stopped.
    #[test]
    fn a_file_that_does_not_parse_says_where_it_stopped() {
        let text = GOOD
            .replace("payloads:", "payloads:\n  - name: bad\n   type: oops");
        let set = written(".yaml", &text, GOOD_SIM);
        let report = verify(&set.payload, None, Limits::EVERYTHING);
        assert_eq!(report.findings.len(), 1, "{:?}", report.findings);
        assert!(
            report.findings[0].line > 1,
            "the YAML parser says which line: {:?}",
            report.findings[0]
        );
    }

    /// An XML file that does not parse is reported from where the reader
    /// stopped, as a YAML file is.
    ///
    /// Refused by its reader, and told at the line that is wrong, not at the
    /// file's first.
    #[test]
    fn an_xml_file_that_does_not_parse_says_where_it_stopped() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/minimal");
        let good = std::fs::read_to_string(root.join("minimal.xml"))
            .expect("the example");
        let line_holding = |text: &str, what: &str| {
            text.lines()
                .position(|line| line.contains(what))
                .map(|at| at + 1)
                .expect("the edit is in the file")
        };
        for (from, to) in [
            // An element the editor's section has no field for.
            ("<x>34</x>", "<z>34</z>"),
            // A closing tag that is not its element's.
            ("5 s</default_interval>", "5 s</default_intervl>"),
        ] {
            let text = good.replacen(from, to, 1);
            let (_dir, path) = graph_written(".xml", &text);
            let report = verify(&path, None, Limits::EVERYTHING);
            assert_eq!(report.findings.len(), 1, "{to}: {:?}", report.findings);
            assert_eq!(
                report.findings[0].line,
                line_holding(&text, to),
                "{to}: {:?}",
                report.findings[0]
            );
        }
    }

    /// A file written in the language that is gone is told what became of it.
    #[test]
    fn a_file_in_the_old_language_is_told_so() {
        let text = "endpoint_groups:\n  - name: g\n    type: network\n    \
                    protocol: udp\n\
                    endpoints:\n  - name: e\n    group: g\n    \
                    address: localhost\n    \
                    port: 5000\n";
        let set = written(".yaml", text, GOOD_SIM);
        let report = verify(&set.payload, None, Limits::EVERYTHING);
        assert_eq!(report.findings.len(), 1);
        assert!(
            report.findings[0].message.contains("endpoints"),
            "{:?}",
            report.findings[0]
        );
    }

    /// A file that writes a version is refused for a field a configuration
    /// does not have, beside the line that writes it, in either format.
    #[test]
    fn a_version_is_refused_as_a_field_there_is_not() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/minimal");
        // Where the line goes: below the YAML schema comment, and below the
        // XML root's start tag, which runs over the file's second to fourth.
        for (file, line, at) in [
            ("minimal.yaml", "version: \"2.0\"\n", 3),
            ("minimal.xml", "    <version>2.0</version>\n", 5),
        ] {
            let good =
                std::fs::read_to_string(root.join(file)).expect("the example");
            let extension = if file.ends_with(".xml") {
                ".xml"
            } else {
                ".yaml"
            };
            let mut lines: Vec<&str> = good.split_inclusive('\n').collect();
            lines.insert(at - 1, line);
            let (_dir, path) = graph_written(extension, &lines.concat());

            let report = verify(&path, None, Limits::EVERYTHING);
            assert_eq!(report.findings.len(), 1, "{file}: {:?}", report.said());
            assert_eq!(
                report.findings[0].line,
                at,
                "{file}: {:?}",
                report.said()
            );
            let said = &report.findings[0].message;
            assert!(said.contains("unknown field `version`"), "{file}: {said}");
            assert_ne!(report.status(), 0, "{file}");
        }
    }

    /// An extension this does not read is refused, naming the file.
    #[test]
    fn a_file_that_is_not_a_configuration_file_is_refused() {
        let set = written(".conf", GOOD, GOOD_SIM);
        let report = verify(&set.payload, None, Limits::EVERYTHING);
        assert_eq!(report.findings.len(), 1);
        assert!(
            report.findings[0].message.contains(".yaml"),
            "{:?}",
            report.findings[0]
        );
    }

    /// The simulator file's problems are its own, reported from its lines.
    #[test]
    fn the_simulator_file_is_checked_against_the_payload_file() {
        let sim = "\
description: settings
simulated_payloads:
  - name: DH0
    simulate: true
    type: device-file
    packet_interval: 1 s
  - name: nobody
    simulate: true
    type: device-file
    packet_interval: 1 s
";
        let set = written(".yaml", GOOD, sim);
        let report = verify(&set.payload, Some(&set.sim), Limits::EVERYTHING);

        // "nobody" is no payload of the other file, which is the end of the
        // join this asks about. A payload simulated by nothing is the other
        // end, and in a graph it is no fault at all: a configuration may
        // leave a payload unsimulated on purpose -- one that is a program, or
        // one on real hardware -- so only the names that match nothing are
        // reported.
        assert_eq!(report.findings.len(), 1, "{:?}", report.findings);
        for finding in &report.findings {
            assert_eq!(finding.file, set.sim, "a problem in the wrong file");
        }
        assert!(
            report.findings.iter().any(|f| f.line == 7),
            "the entry that names nobody is on line 7: {:?}",
            report.findings
        );
    }

    /// A simulator file is not joined to a tcspecial configuration that did not
    /// settle.
    ///
    /// Every payload of the one is named in the other, so a join against the
    /// payloads that did settle reports the rest as unsimulated: a page of
    /// problems this program caused rather than found.
    #[test]
    fn a_broken_payload_file_is_not_joined_to() {
        // A configuration that does not parse, which is the case there is
        // nothing to join to: its payloads are not known, and a simulator
        // file is joined to them by name.
        //
        // Not merely a configuration with an error in it. A graph that reads
        // and fails a check still says which payloads it has, so the two
        // files are still read against each other.
        let text = GOOD.replace("payloads:", "payloads: [oh dear");
        let set = written(".yaml", &text, GOOD_SIM);
        let report = verify(&set.payload, Some(&set.sim), Limits::EVERYTHING);

        assert!(!report.joined);
        assert!(
            report.findings.iter().all(|f| f.file == set.payload),
            "the simulator file was faulted for the tcspecial \
             configuration's problems: {:?}",
            report.findings
        );
    }

    /// A simulator file that does not parse is reported whatever the payload
    /// file is like.
    #[test]
    fn a_simulator_file_is_read_even_when_the_other_one_is_broken() {
        // A link to a node that is not there: an error, so the two files
        // are not joined.
        let text = GOOD.replace(
            "    read_size: \"4\"\n",
            "    payload-out:\n      - target: nowhere\n",
        );
        let set = written(
            ".yaml",
            &text,
            "simulated_payloads:\n  - name: DH0\n   simulate: true\n   \
             type: device-file\n",
        );
        let report = verify(&set.payload, Some(&set.sim), Limits::EVERYTHING);
        assert!(
            report.findings.iter().any(|f| f.file == set.sim),
            "the simulator file was not read: {:?}",
            report.findings
        );
    }

    /// Writes a configuration with this many errors in it, one per payload:
    /// each links to a node that is not there.
    ///
    /// # Arguments
    ///
    /// * `count` - How many errors.
    ///
    /// # Returns
    ///
    /// The configuration and a good simulator file, in a directory of their
    /// own.
    fn with_errors(count: usize) -> Set {
        let mut payloads = "payloads:\n".to_string();
        for id in 0..count {
            // A link to a node that is not there, which is one error each.
            payloads.push_str(&format!(
                "  - name: DH{id}\n    type: device-file\n    \
                 path: /dev/null{id}\n    \
                 payload-out:\n      - target: nowhere{id}\n"
            ));
        }
        let head = GOOD
            .split("payloads:")
            .next()
            .expect("GOOD states its payloads");
        written(".yaml", &format!("{head}{payloads}"), GOOD_SIM)
    }

    /// Writes a sound graph with unfinished payloads added: warnings, and no
    /// errors.
    ///
    /// # Arguments
    ///
    /// * `spares` - How many unfinished payloads, one warning each.
    ///
    /// # Returns
    ///
    /// The directory, which is removed when dropped, and the file's path.
    fn with_warnings(spares: usize) -> (tempfile::TempDir, PathBuf) {
        let mut extra = String::new();
        for id in 0..spares {
            // Placed and not yet filled in or linked, as the editor writes a
            // node the moment it is dropped: see
            // an_unfinished_graph_warns_and_passes.
            extra.push_str(&format!("  - {{ name: spare{id}, type: i2c }}\n"));
        }
        graph_written(
            ".yaml",
            &SOUND_GRAPH.replace("gateway:\n", &format!("{extra}gateway:\n")),
        )
    }

    /// Makes the limits for a run.
    ///
    /// # Arguments
    ///
    /// * `error_limit` - How many counted findings stop the run; `None` for
    ///                   no limit.
    /// * `warnings`    - What is done with warnings.
    ///
    /// # Returns
    ///
    /// The limits.
    fn limited(error_limit: Option<usize>, warnings: Warnings) -> Limits {
        Limits {
            error_limit,
            warnings,
        }
    }

    fn printed(report: &Report) -> String {
        let mut printed = Vec::new();
        report.write(&mut printed).expect("written");
        String::from_utf8(printed).expect("text")
    }

    /// The limit stops the run at once on the last finding it allows:
    /// that many are printed, and nothing after them.
    #[test]
    fn the_error_limit_stops_the_run_on_the_last_it_allows() {
        let set = with_errors(12);
        for limit in [1, 3, 12] {
            let report =
                verify(&set.payload, None, limited(Some(limit), Warnings::On));
            assert!(report.halted, "the run did not stop at {limit}");
            assert_eq!(report.findings.len(), limit, "{:?}", report.findings);
            assert_eq!(printed(&report).lines().count(), limit);
            assert_eq!(report.status(), 1);
        }
    }

    /// A limit of nought looks at nothing and prints nothing, so the run
    /// passes: no error was printed, and the status is about what was.
    #[test]
    fn a_limit_of_nought_prints_nothing_and_passes() {
        let set = with_errors(12);
        let report = verify(&set.payload, None, limited(Some(0), Warnings::On));
        assert!(report.findings.is_empty(), "{:?}", report.findings);
        assert_eq!(printed(&report), "");
        assert_eq!(report.status(), 0);
    }

    /// No limit looks at everything.
    #[test]
    fn no_limit_looks_at_everything() {
        let set = with_errors(150);
        let report = verify(&set.payload, None, Limits::EVERYTHING);
        assert!(!report.halted);
        assert_eq!(report.findings.len(), 150);
        assert_eq!(report.status(), 1);
    }

    /// What is printed is what was found, in the order found, and a run
    /// stopped by its limit ends on the finding that stopped it: the first
    /// lines of the run with no limit, and no more.
    #[test]
    fn findings_are_printed_in_the_order_found() {
        let (_dir, path) = with_warnings(3);
        let text = std::fs::read_to_string(&path).expect("written");
        std::fs::write(&path, text.replace("    recv: 0.0.0.0:4000\n", ""))
            .expect("written");

        let every = printed(&verify(&path, None, Limits::EVERYTHING));
        let every: Vec<&str> = every.lines().collect();
        let errors = every
            .iter()
            .filter(|line| line.contains(": error: "))
            .count();
        assert!(errors >= 1, "{every:?}");
        assert!(
            every.iter().any(|line| line.contains(": warning: ")),
            "{every:?}"
        );

        for limit in 1..=errors {
            let report =
                verify(&path, None, limited(Some(limit), Warnings::On));
            let some = printed(&report);
            let some: Vec<&str> = some.lines().collect();
            assert_eq!(some, every[..some.len()], "under {limit}");
            assert!(
                some.last().unwrap().contains(": error: "),
                "under {limit}: {some:?}"
            );
        }
        for limit in 1..=every.len() {
            let report =
                verify(&path, None, limited(Some(limit), Warnings::Errors));
            let some = printed(&report);
            assert_eq!(
                some.lines().collect::<Vec<_>>(),
                every[..limit],
                "under {limit}"
            );
        }
    }

    /// Warnings are printed and not counted, dropped, or counted as errors
    /// are -- and in none of the three do they fail a run.
    #[test]
    fn warnings_are_printed_dropped_or_counted() {
        let (_dir, path) = with_warnings(4);
        let every = verify(&path, None, Limits::EVERYTHING);
        let warned = every.findings.len();
        assert!(
            warned >= 4,
            "too few warnings to tell apart: {:?}",
            every.findings
        );
        assert!(
            every
                .findings
                .iter()
                .all(|f| f.severity == Severity::Warning),
            "{:?}",
            every.findings
        );
        assert_eq!(every.status(), 0);

        // On: a limit counts errors, and there are none to count.
        let on = verify(&path, None, limited(Some(2), Warnings::On));
        assert_eq!(on.findings.len(), warned);
        assert!(!on.halted);
        assert_eq!(on.status(), 0);

        // Off: not one is said.
        let off = verify(&path, None, limited(None, Warnings::Off));
        assert!(off.findings.is_empty(), "{:?}", off.findings);
        assert_eq!(off.status(), 0);

        // Errors: counted, so the limit stops the run on the second, and the
        // run still passes, warnings being counted for the limit only.
        let counted = verify(&path, None, limited(Some(2), Warnings::Errors));
        assert_eq!(counted.findings.len(), 2, "{:?}", counted.findings);
        assert!(counted.halted);
        assert_eq!(counted.status(), 0);

        // And errors are not touched by what is done with warnings.
        let set = with_errors(3);
        for warnings in [Warnings::On, Warnings::Off, Warnings::Errors] {
            let report = verify(&set.payload, None, limited(None, warnings));
            assert_eq!(report.findings.len(), 3, "{warnings:?}");
            assert_eq!(report.status(), 1, "{warnings:?}");
        }
    }

    /// Every shipped configuration verifies, in both formats.
    ///
    /// The examples somebody is handed first. A graph has no simulator file
    /// beside it, so each is checked on its own -- and both spellings of each
    /// are checked, because a configuration may be written in either and the
    /// two are transcribed by hand.
    ///
    /// Errors, which is what "verifies" means: a shipped example may hold a
    /// node somebody has not finished -- that is what a warning is -- and the
    /// thermostat holds none. What must never be in one of these is something
    /// the rules say is wrong.
    #[test]
    fn every_shipped_graph_verifies() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        // The worked example, which is the shipped configuration nobody edits
        // by hand. `example.*` is what tcsgui opens by default, so it holds
        // whatever was last drawn in it -- including, while somebody is
        // drawing, a line they have not finished moving. What is asked of that
        // one is that it can be read at all: an error in it is somebody's work
        // in progress, and saying so is the editor's business rather than a
        // test's.
        for name in ["tests/manual/thermostat"] {
            for extension in ["yaml", "xml"] {
                let graph = root.join(format!("{name}.{extension}"));
                let report = verify(&graph, None, Limits::EVERYTHING);
                assert!(
                    !report.found_errors(),
                    "{name}.{extension} was faulted: {:?}",
                    report.findings
                );
            }
        }

        for extension in ["yaml", "xml"] {
            let graph = root.join(format!("example.{extension}"));
            let report = verify(&graph, None, Limits::EVERYTHING);
            assert!(
                !report
                    .findings
                    .iter()
                    .any(|finding| finding.message.contains("cannot be read")),
                "example.{extension} does not read: {:?}",
                report.findings
            );
        }

        // And the worked example is finished, warnings and all: it is the one
        // a reader is meant to learn the shape from.
        for extension in ["yaml", "xml"] {
            let graph =
                root.join(format!("tests/manual/thermostat.{extension}"));
            let report = verify(&graph, None, Limits::EVERYTHING);
            assert!(
                !report.found_anything(),
                "thermostat.{extension}: {:?}",
                report.findings
            );
        }
    }

    /// A configuration whose only trouble is unfinished nodes warns and
    /// passes.
    ///
    /// The state every configuration is in while it is being drawn: a node
    /// placed and not yet filled in, a node not yet linked to anything. A
    /// verifier that failed on that would fail on every save, so it says so
    /// and exits nought -- and says `warning` on the line, so that a reader
    /// can tell it from the kind that means the file is wrong.
    #[test]
    fn an_unfinished_graph_warns_and_passes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("half_drawn.yaml");

        // The shipped example with one more payload on it, placed and not yet
        // told which device it is on or linked to anything -- which is what
        // the editor writes the moment somebody drops a node on the canvas.
        let said =
            std::fs::read_to_string(root.join("tests/manual/thermostat.yaml"))
                .expect("the worked example");
        std::fs::write(
            &path,
            said.replace(
                "payloads:\n",
                "payloads:\n  - name: temp_new\n    type: i2c\n",
            ),
        )
        .unwrap();

        let report = verify(&path, None, Limits::EVERYTHING);
        assert!(
            report.found_anything(),
            "the unfinished node was not noticed"
        );
        assert!(
            !report.found_errors(),
            "an unfinished node is not an error: {:?}",
            report.findings
        );
        assert_eq!(report.status(), 0, "and must not fail a build");

        let mut printed = Vec::new();
        report.write(&mut printed).expect("written");
        let printed = String::from_utf8(printed).expect("text");
        assert!(printed.contains(": warning: "), "{printed}");
        assert!(printed.contains("temp_new"), "{printed}");
    }
}
