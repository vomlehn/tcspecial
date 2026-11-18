//! Whether two configurations can be run at once: tcs-conflict's check.
//!
//! Each configuration is verified first, with its simulator file where one
//! is given, as [`crate::verify`] verifies it, because what a configuration
//! holds is listed from what it says and a list made from a file with
//! errors in it would be a list of half-read things. Then what both hold is
//! found by `tcsprotocol::graph::conflict`, which lists what each holds with
//! the same functions its own checks use, and reported against the second
//! configuration, at the line that says it, naming the first's file and
//! line too.

use std::path::{Path, PathBuf};

use tcsprotocol::graph::conflict::{between, holdings, Holding, Written};
use tcsprotocol::graph::Graph;
use tcsprotocol::{ConfigFormat, SimConfigFile};

use crate::locate::{Where, THE_FILE_ITSELF};
use crate::{verify, Limits, Report};

/// One configuration of the two, as it was given.
#[derive(Debug, Clone, Copy)]
pub struct Given<'a> {
    /// The configuration.
    pub configuration: &'a Path,
    /// Its simulator file, if one was given.
    pub simulator: Option<&'a Path>,
}

/// One configuration, read, and where its names are written.
struct Read {
    /// The configuration.
    graph: Graph,
    /// Its simulator file, where one was given.
    sim: Option<(PathBuf, SimConfigFile)>,
    /// Where each name is written, file by file: the configuration's own,
    /// those it includes, and the simulator file.
    located: Vec<(PathBuf, Where)>,
}

impl Read {
    /// Reads a configuration that verified.
    ///
    /// # Arguments
    ///
    /// * `given` - The configuration and its simulator file.
    ///
    /// # Returns
    ///
    /// It, read; `None` where it will not read, which having verified it
    /// does.
    fn of(given: Given) -> Option<Read> {
        let graph = Graph::load(given.configuration).ok()?;
        let mut located = Vec::new();
        let mut note = |file: &Path| {
            let format = ConfigFormat::of_file(file).ok()?;
            let text = tcsos::fs::read_to_string(file).ok()?;
            located.push((file.to_path_buf(), Where::of(&text, format)));
            Some(())
        };
        for file in &graph.files {
            note(file);
        }
        let sim = match given.simulator {
            Some(path) => {
                note(path);
                Some((path.to_path_buf(), SimConfigFile::load(path).ok()?))
            }
            None => None,
        };
        Some(Read {
            graph,
            sim,
            located,
        })
    }

    /// Lists what this configuration holds.
    ///
    /// # Returns
    ///
    /// What it holds, as `tcsprotocol::graph::conflict::holdings` lists it.
    fn holdings(&self) -> Vec<Holding> {
        holdings(&self.graph, self.sim.as_ref().map(|(_, sim)| sim))
    }

    /// Finds the file and line a holding is written at.
    ///
    /// # Arguments
    ///
    /// * `held`  - What is held.
    /// * `top`   - The configuration's own file, as it was given.
    ///
    /// # Returns
    ///
    /// The file and the line in it.
    fn place(&self, held: &Holding, top: &Path) -> (PathBuf, usize) {
        let file = match (held.written, &held.about) {
            (Written::Simulator, _) => self
                .sim
                .as_ref()
                .map_or_else(|| top.to_path_buf(), |(path, _)| path.clone()),
            (Written::Configuration, tcsprotocol::About::Node(name)) => self
                .graph
                .homes
                .get(name)
                .cloned()
                .unwrap_or_else(|| top.to_path_buf()),
            (Written::Configuration, _) => top.to_path_buf(),
        };
        let line = self
            .located
            .iter()
            .find(|(path, _)| *path == file)
            .map_or(THE_FILE_ITSELF, |(_, where_)| where_.line_of(&held.about));
        (file, line)
    }
}

/// Checks two configurations, each with its simulator file where one is
/// given, and whether they can be run at once.
///
/// # Arguments
///
/// * `one`    - The first configuration.
/// * `two`    - The second, which each conflict is reported against.
/// * `limits` - The run's limits, as for [`verify`], over both files'
///              findings and the conflicts together.
///
/// # Returns
///
/// What is wrong with either configuration, if anything is, and otherwise
/// everything both hold.
pub fn conflicts(one: Given, two: Given, limits: Limits) -> Report {
    let mut report = verify(one.configuration, one.simulator, limits);
    let then = verify(two.configuration, two.simulator, limits);
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
    // Nothing is compared once either has an error: see the module's words.
    if report.halted || report.found_errors() {
        return report;
    }
    let (Some(first), Some(second)) = (Read::of(one), Read::of(two)) else {
        return report;
    };

    let (ours, theirs) = (first.holdings(), second.holdings());
    for conflict in between(&ours, &theirs) {
        if !report.room() {
            break;
        }
        let (there, there_line) =
            first.place(&ours[conflict.first], one.configuration);
        let (here, line) =
            second.place(&theirs[conflict.second], two.configuration);
        report.found(
            &here,
            line,
            format!(
                "{} (the other is at {}:{there_line})",
                conflict.why,
                there.display()
            ),
            conflict.severity,
            None,
        );
    }
    report
}
