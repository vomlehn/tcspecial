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
//! Which checks are run depends on the version the file says it is. The two
//! versions describe different things, so there is no sense in running one's
//! checks over the other's file: a version 2 configuration reported as a
//! version 1 one would be a page of complaints about missing payloads, which
//! hides the only thing worth saying about it.
//!
//! A version 2 configuration -- a graph of nodes:
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
//!   that is not one. A graph's payloads are nodes rather than data handlers,
//!   so the rules about what a handler is settled into are not asked of them;
//!   see `tcslibgs::SimConfigFile::problems_by_name`.
//!
//! A version 1 configuration -- a set of payloads, with a simulator file:
//!
//! * The tcspecial configuration file on its own: that it parses, that it
//!   describes payloads, that every payload and group is settled by the rules
//!   for its type, and that no two payloads want one address or one device.
//! * The simulator file, if one is given: that it parses, and that it and the
//!   tcspecial configuration agree -- every payload simulated, every simulated
//!   payload a payload, and every setting one that payload can have.
//!
//! The second is checked against the first only when the first has no problems
//! of its own. A tcspecial configuration that did not settle has no handlers to
//! join to, and a join against half of them would report every missing payload
//! as unsimulated: a page of problems caused by this program rather than by the
//! files.

use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tcslibgs::graph::check::{Severity, AND_OTHERS, LISTED};
use tcslibgs::{
    describes_payloads_text, ConfigFormat, DHConfig, PayloadConfig, SimConfigFile, TcsError,
};

pub mod locate;
pub mod startup;

use locate::{Where, THE_FILE_ITSELF};

/// How many errors are printed before this stops looking.
///
/// A file with more wrong with it than this is a file whose first hundred
/// problems are what its author should read; the hundred and first is the one
/// that says there are more, and the rest would be a page nobody scrolls to
/// the end of. Many of them would also be consequences of the first few.
pub const ERROR_LIMIT: usize = 100;

/// What is printed when the limit is reached.
pub const TOO_MANY: &str = "Too many errors, halting";

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
    /// The rule's own answer, where it came from a version 2 configuration's
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
    /// The problems, in the order the rules are asked. One more than
    /// [`ERROR_LIMIT`] at most: the last of them is the one that stopped the
    /// run, and is counted rather than printed.
    pub findings: Vec<Finding>,
    /// Whether the run stopped because there were too many.
    pub halted: bool,
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
    pub fn found_errors(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.severity == Severity::Error)
    }

    /// Whether anything at all was found, wrong or unfinished.
    pub fn found_anything(&self) -> bool {
        !self.findings.is_empty()
    }

    /// The status a program exits with after this.
    ///
    /// Nought when the files are good, so that `make` and a shell `&&` can be
    /// told by it.
    pub fn status(&self) -> i32 {
        if self.found_errors() {
            1
        } else {
            0
        }
    }

    /// Write the first few findings, then say how many more there were.
    ///
    /// Errors first and warnings after, so that the lines a person reads are
    /// the ones saying the configuration is wrong rather than the ones saying
    /// it is unfinished. [`LISTED`] of them, and then [`AND_OTHERS`] with the
    /// count: a report twenty lines long is a report scrolled past, and in a
    /// half-built graph most of those lines are one unfinished node said
    /// twice.
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

    /// The same lines, each with whether it is about an error.
    ///
    /// The count of the rest, and the line saying the run halted, are counted
    /// with the errors when there are any: they stand for more of what was
    /// listed above them, and errors are listed first.
    pub fn lines(&self) -> Vec<(bool, String)> {
        let ordered = self.ordered();
        let mut said: Vec<(bool, String)> = ordered
            .iter()
            .take(LISTED)
            .map(|finding| (finding.severity == Severity::Error, finding.to_string()))
            .collect();
        let errors = self.found_errors();
        let rest = ordered.len().saturating_sub(LISTED);
        if rest > 0 {
            said.push((errors, format!("{AND_OTHERS} ({rest} more)")));
        }
        // And whether there were more than were even looked for, which is a
        // different thing from more than were printed.
        if self.halted {
            said.push((errors, TOO_MANY.to_string()));
        }
        said
    }

    /// Whether any of what was found is about this file.
    ///
    /// For a program that reads one of the two files loosely: tcsmoc shows
    /// what a simulator file says and runs without it, so a problem there is
    /// worth saying and not worth stopping for.
    pub fn found_errors_in(&self, file: &Path) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.severity == Severity::Error && finding.file == file)
    }

    /// The findings, errors first. See [`Report::write`].
    pub fn ordered(&self) -> Vec<&Finding> {
        let mut ordered: Vec<&Finding> = self
            .findings
            .iter()
            .filter(|finding| finding.severity == Severity::Error)
            .collect();
        ordered.extend(
            self.findings
                .iter()
                .filter(|finding| finding.severity == Severity::Warning),
        );
        ordered
    }

    fn add(&mut self, file: &Path, line: usize, message: impl Into<String>) {
        self.found(file, line, message, Severity::Error, None)
    }

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
        self.findings.push(Finding {
            file: file.to_path_buf(),
            line,
            message: message.into(),
            severity,
            problem,
        });
        // The one past the limit is kept so that the count is honest, and is
        // what says there were too many.
        if self.findings.len() > ERROR_LIMIT {
            self.halted = true;
        }
    }

    fn room(&self) -> bool {
        !self.halted
    }
}

/// Check a tcspecial configuration file, and a simulator file if there is one.
///
/// Which checks are run depends on the version the file says it is. The two
/// versions describe different things -- version 1 a set of payloads with a
/// simulator file beside it, version 2 a graph of nodes -- so there is nothing
/// to share between the two paths but this function, which reads the version
/// and takes one.
pub fn verify(payload_path: &Path, sim_path: Option<&Path>) -> Report {
    let mut report = Report::default();

    // Asked of the library rather than worked out here: one place knows how
    // to read a version. A file that cannot be read at all answers "not
    // version 2", and version 1's path reports it -- it has the better
    // messages for a file that does not parse, having had them longer.
    if tcslibgs::graph::is_version_2(payload_path) {
        let payloads = check_graph(payload_path, &mut report);
        if let Some(sim_path) = sim_path {
            // Against the graph's payloads by name, which is all a graph's
            // simulator file is joined to it by -- and only when the graph
            // read, as version 1's is only when its handlers settled: see
            // the module comment.
            report.joined = payloads.is_some();
            check_graph_simulator(sim_path, payloads.as_deref(), &mut report);
        }
        return report;
    }

    let handlers = check_payloads(payload_path, &mut report);

    if let Some(sim_path) = sim_path {
        // Only against a tcspecial configuration that settled; see the module
        // comment.
        let clean = handlers.as_ref().filter(|_| !report.found_errors());
        report.joined = clean.is_some();
        check_simulator(sim_path, clean.map(Vec::as_slice), &mut report);
    }

    report
}

/// Check two spellings of one configuration -- one XML, one YAML -- each
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
pub fn verify_pair(first: &Path, second: &Path, sim_path: &Path) -> Report {
    let mut report = verify(first, Some(sim_path));
    let then = verify(second, Some(sim_path));
    report.joined = report.joined && then.joined;
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
    compare(first, second, &mut report);
    report
}

/// Whether two files are read as one configuration, every difference found
/// added to `report`. See [`verify_pair`].
pub fn compare(first: &Path, second: &Path, report: &mut Report) {
    use tcslibgs::config_digest::interpretation_of_file;

    let (one, two) = match (
        interpretation_of_file(first),
        interpretation_of_file(second),
    ) {
        (Ok(one), Ok(two)) => (one, two),
        (one, two) => {
            let unread: Vec<String> = [(first, one.is_err()), (second, two.is_err())]
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
                "{}: {first_name} gives them in the order {a} and {second_name} \
                 in the order {b}, and the order is part of what they mean",
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

/// A value as a reader would write it in a message.
fn shown(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => format!("\"{text}\""),
        other => other.to_string(),
    }
}

/// A value said by one file only: itself where it is one, nothing where it is
/// a whole entry.
fn alone(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => None,
        other => Some(shown(other)),
    }
}

/// What tells the entries of a list apart, where something does: a node by
/// its name, a link by where it leaves from. `None` for a list whose entries
/// have no such key, or whose keys repeat, which is then compared place by
/// place.
fn keys_of(list: &[serde_json::Value]) -> Option<Vec<String>> {
    for key in ["name", "from"] {
        let keys: Option<Vec<String>> = list
            .iter()
            .map(|entry| entry.get(key).and_then(|k| k.as_str()).map(str::to_string))
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

/// Every place two value trees differ, each with the path to it.
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
                        None => out.push((path.clone(), Said::OnlyFirst(alone(x)))),
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
                let common = |keys: &[String], other: &[String]| -> Vec<String> {
                    keys.iter().filter(|k| other.contains(k)).cloned().collect()
                };
                let (oa, ob) = (common(&ka, &kb), common(&kb, &ka));
                if oa != ob {
                    out.push((path.clone(), Said::Order(oa.join(", "), ob.join(", "))));
                }
            }
            _ => {
                for at in 0..a.len().max(b.len()) {
                    path.push(at.to_string());
                    match (a.get(at), b.get(at)) {
                        (Some(x), Some(y)) => differences_between(x, y, path, out),
                        (Some(x), None) => out.push((path.clone(), Said::OnlyFirst(alone(x)))),
                        (None, Some(y)) => out.push((path.clone(), Said::OnlySecond(alone(y)))),
                        (None, None) => {}
                    }
                    path.pop();
                }
            }
        },
        (x, y) if x != y => out.push((path.clone(), Said::Both(shown(x), shown(y)))),
        _ => {}
    }
}

/// The line a path's last named part is written on, searching from where each
/// part before it was found -- the node, then the attribute under it. The
/// file's first line where nothing is found.
///
/// Only the parts a file writes are searched for: a node's name, an
/// attribute's, a section's. The comparison's own words for where it is --
/// `nodes`, `attributes`, a place in a list -- are not in a file, or are in it
/// somewhere else (the editor's own section has `nodes`), and finding one
/// would send a reader to the wrong line. A link's `node.terminal` is the node
/// and then the terminal under it, which is how both formats write it.
fn line_of_path(text: &str, path: &[String]) -> usize {
    const NOT_WRITTEN: [&str; 4] = ["nodes", "attributes", "links", "to"];
    let lines: Vec<&str> = text.lines().collect();
    let mut from = 0;
    let mut found = None;
    let parts = path
        .iter()
        .filter(|part| part.parse::<usize>().is_err() && !NOT_WRITTEN.contains(&part.as_str()))
        .flat_map(|part| part.split('.').map(str::to_string).collect::<Vec<_>>());
    for part in parts {
        // A whole name, outside a comment: `heater` is not found in
        // `heaters`, and a comment saying what an attribute is for is not
        // where it is set.
        let names = |line: &&str| {
            let said = line.trim_start();
            if said.starts_with('#') || said.starts_with("<!--") {
                return false;
            }
            let in_a_name = |c: char| c.is_alphanumeric() || c == '_' || c == '-';
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

/// Check a version 2 configuration: read it, then ask what is wrong with it.
///
/// The checking is `tcslibgs::graph::Graph::check`, which is the same code
/// tcspecial runs aboard. This adds only the line numbers, which it gets the
/// way version 1's problems get theirs: by scanning the text for the name the
/// problem is about.
///
/// Hands back the names of the graph's payloads, for a simulator file to be
/// joined to, when the graph read at all.
fn check_graph(path: &Path, report: &mut Report) -> Option<Vec<String>> {
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

    for problem in graph.check() {
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

    Some(
        graph
            .nodes
            .iter()
            .filter(|node| node.is_payload())
            .map(|node| node.name.clone())
            .collect(),
    )
}

/// Check a version 2 configuration's simulator file, against the names of its
/// payloads if the configuration read.
///
/// By name only: see `tcslibgs::SimConfigFile::problems_by_name`, which says
/// which of the simulator's rules a graph has anything to be asked of.
fn check_graph_simulator(path: &Path, payloads: Option<&[String]>, report: &mut Report) {
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

    let Some(payloads) = payloads else { return };
    let names: Vec<&str> = payloads.iter().map(String::as_str).collect();

    let found = Where::of(&text, format);
    for problem in sim.problems_by_name(&names) {
        if !report.room() {
            break;
        }
        report.add(path, found.line_of(&problem.about()), problem.to_string());
    }
}

/// Check the tcspecial configuration file, and hand back what it describes.
///
/// The handlers come back even when something is wrong, because what came
/// back is what the file does describe; whether that is all of it is what the
/// report says.
fn check_payloads(path: &Path, report: &mut Report) -> Option<Vec<DHConfig>> {
    let (text, format) = read(path, report)?;

    let config: PayloadConfig = match format.parse(&text) {
        Ok(config) => config,
        Err(parse_error) => {
            // A file written to the old section names, or in the language
            // that is gone, is told so: "names endpoints, which was a second
            // way to describe payloads" says what to do about it, where
            // "missing field `payloads`" says what the parser wanted.
            let message = match describes_payloads_text(&text, format) {
                Err(e) => e.to_string(),
                Ok(()) => parse_error.to_string(),
            };
            report.add(path, line_of(&parse_error), message);
            return None;
        }
    };

    let found = Where::of(&text, format);
    let (handlers, problems) = config.check();
    for problem in problems {
        if !report.room() {
            break;
        }
        report.add(path, found.line_of(&problem.about), problem.message);
    }

    Some(handlers)
}

/// Check the simulator file, against those handlers if there are any.
fn check_simulator(path: &Path, handlers: Option<&[DHConfig]>, report: &mut Report) {
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

    // Everything else this file says is said about a payload of the other
    // one, so without those there is nothing more to ask.
    let Some(handlers) = handlers else { return };

    let found = Where::of(&text, format);
    for problem in sim.problems(handlers) {
        if !report.room() {
            break;
        }
        report.add(path, found.line_of(&problem.about()), problem.to_string());
    }
}

/// The text of a file and the format its name says it is written in.
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

/// Where a parser stopped, when it says.
///
/// YAML says, and that is the line a reader has to look at: the file does not
/// parse, so nothing in it has a name yet to be found by. XML does not say,
/// and a deserialization error -- a missing element, say -- has no position at
/// all even in principle, so such a file is reported from its first line.
/// The line a message mentions, if it mentions one.
///
/// A parse error carries its location in a field, and `line_of` reads it
/// there. A graph that would not load reports through `TcsError::Config`,
/// which is a sentence -- the path has been put in front of the parser's own
/// words, which is worth more than the field was -- so the line is read back
/// out of the text. "at line 7 column 5" is what both parsers write.
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

fn line_of(e: &TcsError) -> usize {
    match e {
        TcsError::Yaml(e) => e.location().map_or(THE_FILE_ITSELF, |at| at.line()),
        _ => THE_FILE_ITSELF,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The thermostat example's two spellings and its simulator file, copied
    /// to a directory of their own, with `change` made to the XML.
    fn thermostat_pair(change: impl Fn(String) -> String) -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/thermostat");
        let dir = tempfile::tempdir().expect("a directory");
        let at = |name: &str| dir.path().join(name);
        for name in ["thermostat.yaml", "thermostat-sim.yaml"] {
            std::fs::copy(root.join(name), at(name)).expect("copied");
        }
        let xml = std::fs::read_to_string(root.join("thermostat.xml")).expect("the XML");
        std::fs::write(at("thermostat.xml"), change(xml)).expect("written");
        let (xml, yaml, sim) = (at("thermostat.xml"), at("thermostat.yaml"), at("thermostat-sim.yaml"));
        (dir, xml, yaml, sim)
    }

    /// Two spellings of one configuration: each checked, nothing different.
    #[test]
    fn two_spellings_of_one_configuration_compare_alike() {
        let (_dir, xml, yaml, sim) = thermostat_pair(|xml| xml);
        let report = verify_pair(&xml, &yaml, &sim);
        assert!(report.findings.is_empty(), "{:?}", report.said());
        assert!(report.joined, "both were checked with the simulator file");
        // Either order.
        assert!(verify_pair(&yaml, &xml, &sim).findings.is_empty());
    }

    /// A value changed in one spelling is reported: where, and what each
    /// file says, at the line the second file sets it on.
    #[test]
    fn a_value_one_spelling_changes_is_reported() {
        let (_dir, xml, yaml, sim) =
            thermostat_pair(|xml| xml.replacen("<setpoint>293.15", "<setpoint>290", 1));
        let report = verify_pair(&yaml, &xml, &sim);
        let said = report.said();
        assert_eq!(report.findings.len(), 1, "{said:?}");
        let finding = &report.findings[0];
        assert_eq!(finding.file, xml);
        assert!(
            finding.message.contains("nodes / thermostat / attributes / setpoint")
                && finding.message.contains("says \"293.15\"")
                && finding.message.contains("says \"290\""),
            "{said:?}"
        );
        let text = std::fs::read_to_string(&xml).expect("the XML");
        let line = text.lines().nth(finding.line - 1).expect("a line");
        assert!(line.contains("<setpoint>290"), "line {}: {line}", finding.line);
        assert_eq!(report.status(), 1);
    }

    /// A node renamed in one spelling is one node missing from each, and a
    /// line to it that names it differently -- not a page of differences for
    /// every node after it.
    #[test]
    fn a_node_one_spelling_renames_is_missing_from_the_other() {
        // Compared and not verified, so the simulator file is not wanted.
        let (_dir, xml, yaml, _) = thermostat_pair(|xml| {
            xml.replace("<name>heaters</name>", "<name>heater</name>")
                .replace("<target>heaters</target>", "<target>heater</target>")
        });
        let mut report = Report::default();
        compare(&yaml, &xml, &mut report);
        let said: Vec<&str> = report.findings.iter().map(|f| f.message.as_str()).collect();
        assert!(said.iter().any(|m| m.contains("nodes / heaters:") && m.contains("does not")), "{said:?}");
        assert!(said.iter().any(|m| m.contains("nodes / heater:")), "{said:?}");
        assert!(said.iter().any(|m| m.contains("\"heaters\"") && m.contains("\"heater\"")), "{said:?}");
        assert!(said.len() <= 4, "{said:?}");
    }

    /// The order nodes are written in is part of what they mean -- a node's
    /// place is its device id -- so two spellings that give them in two
    /// orders differ.
    #[test]
    fn nodes_in_another_order_differ() {
        let one = serde_json::json!({ "nodes": [{ "name": "a" }, { "name": "b" }] });
        let two = serde_json::json!({ "nodes": [{ "name": "b" }, { "name": "a" }] });
        let mut out = Vec::new();
        differences_between(&one, &two, &mut Vec::new(), &mut out);
        assert_eq!(out.len(), 1);
        assert!(matches!(&out[0].1, Said::Order(a, b) if a == "a, b" && b == "b, a"));
    }

    /// A file in a temporary directory, so the findings name a real path.
    struct Set {
        _dir: tempfile::TempDir,
        payload: PathBuf,
        sim: PathBuf,
    }

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

    /// A version 2 configuration in a temporary directory.
    fn graph_written(extension: &str, text: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join(format!("vehicle{extension}"));
        std::fs::write(&path, text).expect("written");
        (dir, path)
    }

    const SOUND_GRAPH: &str = "\
version: \"2.0\"
tcspecial:
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
    interval: 5 s
    send: \"0.0.0.0:5550\"
    recv: \"127.0.0.1:5650\"
payload_groups:
  - name: temps
    interval: \"3 s\"
payloads:
  - { name: sensor, type: i2c, group: temps, path: \"/dev/i2c-1\", address: \"0x48\", payload-out: moc }
moc:
  - { name: moc, type: moc, address: \"10.0.0.1\", port: \"5000\", ground_base: \"10.0.0.2:5100\" }
";

    /// A sound version 2 configuration is accepted in silence.
    #[test]
    fn a_sound_graph_is_accepted() {
        let (_dir, path) = graph_written(".yaml", SOUND_GRAPH);
        let report = verify(&path, None);
        assert!(
            !report.found_errors(),
            "{:?}",
            report.findings.iter().map(|f| f.to_string()).collect::<Vec<_>>()
        );
        assert_eq!(report.status(), 0);
    }

    /// The version decides which checks are run.
    ///
    /// A version 2 file reported as a version 1 one would be a list of
    /// complaints about missing payloads, which is noise hiding the fact that
    /// it is a different language.
    #[test]
    fn the_version_decides_which_checks_run() {
        let (_dir, path) = graph_written(".yaml", SOUND_GRAPH);
        let report = verify(&path, None);
        assert!(!report.found_errors());

        // And a version 1 file still goes down version 1's path.
        let set = written(".yaml", GOOD, GOOD_SIM);
        let report = verify(&set.payload, Some(&set.sim));
        assert!(!report.found_errors(), "{:?}", report.findings);
    }

    /// Each problem a graph has is reported, with the line to look at.
    #[test]
    fn a_graphs_problems_are_reported_with_their_lines() {
        let (_dir, path) = graph_written(
            ".yaml",
            "\
version: \"2.0\"
payload_groups:
  - name: temps
    interval: soon
payloads:
  - { name: a, type: i2c, group: temps, path: \"/dev/i2c-1\", payload-out: nowhere }
  - { name: a, type: spi, path: \"/dev/spidev0.0\" }
vehicle:
  - { name: m, type: compute, function: mean, inputs: \"in0\" }
",
        );

        let report = verify(&path, None);
        let said: Vec<String> = report.findings.iter().map(|f| f.to_string()).collect();
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
            "version: \"2.0\"\ninclude:\n  - cabin\nmoc:\n  \
             - { name: moc, type: moc, moc-out: moc }\n",
        )
        .unwrap();
        // The included file holds a node nothing links to.
        std::fs::write(
            dir.path().join("cabin.yaml"),
            "version: \"2.0\"\npayloads:\n  - { name: lonely, type: i2c }\n",
        )
        .unwrap();

        let report = verify(&top, None);
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
        assert_eq!(lonely.line, 3, "{lonely}");
    }

    /// A version 2 configuration's simulator file is joined to it by name.
    ///
    /// The rule the editor holds a simulator file to as it opens one, asked
    /// here of the same pair: an entry for a payload the graph does not have
    /// is somebody's misspelling, and drives nothing.
    #[test]
    fn a_graphs_simulator_file_names_its_payloads() {
        let (dir, path) = graph_written(".yaml", SOUND_GRAPH);
        let sim = dir.path().join("vehiclesim.yaml");

        std::fs::write(&sim, "simulated_payloads:\n  - name: sensor\n").unwrap();
        let report = verify(&path, Some(&sim));
        assert!(!report.found_anything(), "{:?}", report.findings);
        assert!(report.joined);

        std::fs::write(
            &sim,
            "simulated_payloads:\n  - name: sensor\n  - name: sensr\n    group: none\n",
        )
        .unwrap();
        let report = verify(&path, Some(&sim));
        let said: Vec<String> = report.findings.iter().map(|f| f.to_string()).collect();
        let all = said.join("\n");
        assert!(all.contains("\"sensr\" is simulated"), "{all}");
        assert!(all.contains("names group \"none\""), "{all}");
        for finding in &report.findings {
            assert_eq!(finding.file, sim, "{finding}");
            assert_eq!(finding.line, 3, "{finding}");
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
        let report = verify(&root.join("example.yaml"), Some(&sim));
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
    <version>2.0</version>
    <payloads>
        <name>a</name>
        <type>i2c</type>
        <path>/dev/i2c-1</path>
        <payload-out>nowhere</payload-out>
    </payloads>
</configuration>
",
        );

        let report = verify(&path, None);
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

    const GOOD: &str = "\
version: \"1.0\"
description: two payloads
payloads:
  - name: DH0
    type: device-file
    path: /dev/zero
    packet_size: 4
  - name: DH1
    type: network
    protocol: udp
    payload_interface: localhost:5000
    packet_size: 4
";

    const GOOD_SIM: &str = "\
description: settings
simulated_payloads:
  - name: DH0
    type: device-file
    packet_interval: 1 s
  - name: DH1
    type: network
    protocol: udp
    packet_interval: 1 s
";

    #[test]
    fn a_good_set_has_nothing_to_say() {
        let set = written(".yaml", GOOD, GOOD_SIM);
        let report = verify(&set.payload, Some(&set.sim));
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
        let text = "\
version: \"1.0\"
description: three bad payloads
payloads:
  - name: no_size
    type: device-file
    path: /dev/zero
  - name: no_path
    type: device-file
    packet_size: 4
  - name: bad_kind
    type: device-file
    path: /dev/null
    payload_port: 5000
    packet_size: 4
";
        let set = written(".yaml", text, GOOD_SIM);
        let report = verify(&set.payload, None);

        assert_eq!(report.findings.len(), 3, "{:?}", report.findings);
        assert_eq!(report.status(), 1);

        // Each beside the payload it is about, in the order the file wrote
        // them.
        let lines: Vec<usize> = report.findings.iter().map(|f| f.line).collect();
        assert_eq!(lines, vec![4, 7, 10]);
        assert!(
            report.findings[0].message.contains("packet_size"),
            "{:?}",
            report.findings[0]
        );
        assert!(
            report.findings[1].message.contains("path"),
            "{:?}",
            report.findings[1]
        );
        assert!(
            report.findings[2].message.contains("port"),
            "{:?}",
            report.findings[2]
        );
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
        let report = verify(&set.payload, None);
        let first = report.findings.first().expect("something is wrong with it");
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
        let text = "version: \"1.0\"\ndescription: x\npayloads:\n  - name: bad\n";
        let set = written(".yaml", text, GOOD_SIM);
        let report = verify(&set.payload, None);
        assert_eq!(report.findings.len(), 1, "{:?}", report.findings);
        assert!(
            report.findings[0].line > 1,
            "the YAML parser says which line: {:?}",
            report.findings[0]
        );
    }

    /// A file written in the language that is gone is told what became of it.
    #[test]
    fn a_file_in_the_old_language_is_told_so() {
        let text = "endpoint_groups:\n  - name: g\n    type: network\n    protocol: udp\n\
                    endpoints:\n  - name: e\n    group: g\n    address: localhost\n    \
                    port: 5000\n";
        let set = written(".yaml", text, GOOD_SIM);
        let report = verify(&set.payload, None);
        assert_eq!(report.findings.len(), 1);
        assert!(
            report.findings[0].message.contains("endpoints"),
            "{:?}",
            report.findings[0]
        );
    }

    /// A version a beacon cannot carry is reported from the version line.
    ///
    /// Which is the one problem reported from a line that is not a payload's:
    /// the set's version is the set's, and the line that states it is the one
    /// to change.
    #[test]
    fn a_version_a_beacon_cannot_carry_is_reported_from_its_own_line() {
        let set = written(
            ".yaml",
            &GOOD.replace("version: \"1.0\"", "version: \"draft\""),
            GOOD_SIM,
        );
        let report = verify(&set.payload, None);

        assert_eq!(report.findings.len(), 1, "{:?}", report.findings);
        assert_eq!(report.findings[0].line, 1, "the version is the first line");
        assert!(
            report.findings[0].message.contains("draft"),
            "{:?}",
            report.findings[0]
        );
    }

    /// An extension this does not read is refused, naming the file.
    #[test]
    fn a_file_that_is_not_a_configuration_file_is_refused() {
        let set = written(".conf", GOOD, GOOD_SIM);
        let report = verify(&set.payload, None);
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
    type: device-file
    packet_interval: 1 s
  - name: nobody
    type: device-file
    packet_interval: 1 s
";
        let set = written(".yaml", GOOD, sim);
        let report = verify(&set.payload, Some(&set.sim));

        // DH1 is simulated by nothing, and "nobody" is no payload of the
        // other file: both ends of the join are asked.
        assert_eq!(report.findings.len(), 2, "{:?}", report.findings);
        for finding in &report.findings {
            assert_eq!(finding.file, set.sim, "a problem in the wrong file");
        }
        assert!(
            report.findings.iter().any(|f| f.line == 6),
            "the entry that names nobody is on line 6: {:?}",
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
        let text = GOOD.replace("    packet_size: 4\n  - name: DH1", "  - name: DH1");
        let set = written(".yaml", &text, GOOD_SIM);
        let report = verify(&set.payload, Some(&set.sim));

        assert!(!report.joined);
        assert!(
            report.findings.iter().all(|f| f.file == set.payload),
            "the simulator file was faulted for the tcspecial configuration's problems: {:?}",
            report.findings
        );
    }

    /// A simulator file that does not parse is reported whatever the payload
    /// file is like.
    #[test]
    fn a_simulator_file_is_read_even_when_the_other_one_is_broken() {
        let text = GOOD.replace("    packet_size: 4\n  - name: DH1", "  - name: DH1");
        let set = written(
            ".yaml",
            &text,
            "simulated_payloads:\n  - name: DH0\n   type: device-file\n",
        );
        let report = verify(&set.payload, Some(&set.sim));
        assert!(
            report.findings.iter().any(|f| f.file == set.sim),
            "the simulator file was not read: {:?}",
            report.findings
        );
    }

    /// A hundred and one problems stop the run, and the printed report is the
    /// first few of them and a count of the rest.
    #[test]
    fn too_many_problems_stop_the_run() {
        // Each payload states a port it cannot have, so each is one problem.
        let mut text = "version: \"1.0\"\ndescription: many\npayloads:\n".to_string();
        for id in 0..150 {
            text.push_str(&format!(
                "  - name: DH{id}\n    type: device-file\n    \
                 path: /dev/null{id}\n    payload_port: 1\n    packet_size: 4\n"
            ));
        }
        let set = written(".yaml", &text, GOOD_SIM);
        let report = verify(&set.payload, None);

        assert!(report.halted, "the run did not stop");
        assert_eq!(
            report.findings.len(),
            ERROR_LIMIT + 1,
            "the run kept looking after it had too many"
        );

        let mut printed = Vec::new();
        report.write(&mut printed).expect("written");
        let printed = String::from_utf8(printed).expect("text");
        let lines: Vec<&str> = printed.lines().collect();
        // The first few, then that there were others, then that the run gave
        // up looking -- which is a different thing from there being more than
        // were printed.
        assert_eq!(lines.len(), LISTED + 2, "{printed}");
        assert!(lines[LISTED].starts_with(AND_OTHERS), "{printed}");
        assert!(
            lines[LISTED].contains(&format!("{} more", report.findings.len() - LISTED)),
            "{printed}"
        );
        assert_eq!(lines[LISTED + 1], TOO_MANY);
        assert_eq!(report.status(), 1);
    }

    /// Exactly a hundred is not too many: the run did not give up, so the
    /// message that says it did must not appear.
    #[test]
    fn a_hundred_problems_are_all_printed_and_nothing_more_is_said() {
        let mut text = "version: \"1.0\"\ndescription: many\npayloads:\n".to_string();
        for id in 0..ERROR_LIMIT {
            text.push_str(&format!(
                "  - name: DH{id}\n    type: device-file\n    \
                 path: /dev/null{id}\n    payload_port: 1\n    packet_size: 4\n"
            ));
        }
        let set = written(".yaml", &text, GOOD_SIM);
        let report = verify(&set.payload, None);

        assert_eq!(report.findings.len(), ERROR_LIMIT);
        assert!(!report.halted);

        let mut printed = Vec::new();
        report.write(&mut printed).expect("written");
        let printed = String::from_utf8(printed).expect("text");
        assert_eq!(printed.lines().count(), LISTED + 1, "{printed}");
        assert!(printed.contains(AND_OTHERS), "{printed}");
        assert!(!printed.contains(TOO_MANY), "{printed}");
    }

    /// The shipped sets verify, in both formats, with their simulator files.
    ///
    /// The one test that says this program agrees with the programs that run
    /// the files: every set under `tests/manual` is a set someone runs, so a
    /// set this faulted would be this program's mistake.
    #[test]
    fn every_shipped_set_verifies() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        for set in 1..=4 {
            let sim = root.join(format!("tests/manual/tcspecial{set}sim.yaml"));
            for extension in ["yaml", "xml"] {
                let payload = root.join(format!("tests/manual/tcspecial{set}.{extension}"));
                let report = verify(&payload, Some(&sim));
                assert!(
                    !report.found_errors(),
                    "set {set}.{extension} was faulted: {:?}",
                    report.findings
                );
                assert!(report.joined, "set {set}.{extension} was not joined");
            }
        }
    }

    /// Every shipped version 2 configuration verifies, in both formats.
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
                let report = verify(&graph, None);
                assert!(
                    !report.found_errors(),
                    "{name}.{extension} was faulted: {:?}",
                    report.ordered()
                );
            }
        }

        for extension in ["yaml", "xml"] {
            let graph = root.join(format!("example.{extension}"));
            let report = verify(&graph, None);
            assert!(
                !report
                    .findings
                    .iter()
                    .any(|finding| finding.message.contains("cannot be read")),
                "example.{extension} does not read: {:?}",
                report.ordered()
            );
        }

        // And the worked example is finished, warnings and all: it is the one
        // a reader is meant to learn the shape from.
        for extension in ["yaml", "xml"] {
            let graph = root.join(format!("tests/manual/thermostat.{extension}"));
            let report = verify(&graph, None);
            assert!(
                !report.found_anything(),
                "thermostat.{extension}: {:?}",
                report.ordered()
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
        let said = std::fs::read_to_string(root.join("tests/manual/thermostat.yaml"))
            .expect("the worked example");
        std::fs::write(
            &path,
            said.replace(
                "payloads:\n",
                "payloads:\n  - name: temp_new\n    type: i2c\n",
            ),
        )
        .unwrap();

        let report = verify(&path, None);
        assert!(report.found_anything(), "the unfinished node was not noticed");
        assert!(
            !report.found_errors(),
            "an unfinished node is not an error: {:?}",
            report.ordered()
        );
        assert_eq!(report.status(), 0, "and must not fail a build");

        let mut printed = Vec::new();
        report.write(&mut printed).expect("written");
        let printed = String::from_utf8(printed).expect("text");
        assert!(printed.contains(": warning: "), "{printed}");
        assert!(printed.contains("temp_new"), "{printed}");
    }
}
