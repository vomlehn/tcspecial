//! What a program found as it started, gathered to be said before its window.
//!
//! Every program that reads a configuration asks [`crate::verify`] about it
//! first, so that each says what `tcsverify` says about the same files, in the
//! same words. What a program then finds for itself -- a socket it cannot
//! make, a file it can read but not run -- is added beside those, so that a
//! person starting it is told all of it at once rather than one thing per
//! attempt.
//!
//! Gathered here and shown by the program, because showing is the program's
//! own: tcsmoc and tcssim put it in a window that has to be closed before
//! their main window opens, and tcspecial, which has no window, logs it. See
//! `ui/startup.slint` beside this crate, which is that window.

use std::path::Path;

use crate::Report;

/// Everything a program has to say about how it is starting.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Startup {
    /// Each line, with whether it is an error.
    lines: Vec<(bool, String)>,
    /// Whether the program cannot go on with what was found.
    fatal: bool,
}

impl Startup {
    /// What the verifier found, said in its words.
    ///
    /// `fatal` says whether its errors stop this program, which is a question
    /// about the program and not about the files: tcsmoc reads a simulator
    /// file only to show it, and goes on without one that is wrong.
    pub fn report(&mut self, report: &Report, fatal: bool) {
        self.lines.extend(report.lines());
        if fatal && report.found_errors() {
            self.fatal = true;
        }
    }

    /// The same, with one file read loosely: errors in `loose` are said and
    /// do not stop the program, and errors anywhere else do.
    pub fn report_loosely(&mut self, report: &Report, loose: &Path) {
        self.lines.extend(report.lines());
        if report
            .findings
            .iter()
            .any(|finding| finding.severity == crate::Severity::Error && finding.file != loose)
        {
            self.fatal = true;
        }
    }

    /// Something that stops the program.
    pub fn error(&mut self, said: impl Into<String>) {
        self.lines.push((true, said.into()));
        self.fatal = true;
    }

    /// Something worth saying that does not stop it.
    pub fn warning(&mut self, said: impl Into<String>) {
        self.lines.push((false, said.into()));
    }

    /// Whether there is nothing to say.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Whether the program has to stop.
    pub fn fatal(&self) -> bool {
        self.fatal
    }

    /// The lines, each with whether it is an error.
    pub fn lines(&self) -> &[(bool, String)] {
        &self.lines
    }

    /// What the window's Copy button copies: the heading it is shown under,
    /// then every line, one to a line -- what a person passing it on would
    /// otherwise type out again.
    pub fn copied(&self, heading: &str) -> String {
        let mut said = vec![heading.to_string()];
        said.extend(self.lines.iter().map(|(_, line)| line.clone()));
        said.join("\n")
    }

    /// Say it all on stderr, which is said whether or not there is a window
    /// to say it in: a program started from a terminal is read there, and one
    /// that cannot open a window has nowhere else.
    pub fn eprint(&self, program: &str) {
        for (_, line) in &self.lines {
            eprintln!("{program}: {line}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Finding;
    use std::path::PathBuf;
    use tcslibgs::graph::check::Severity;

    fn finding(file: &str, severity: Severity) -> Finding {
        Finding {
            file: PathBuf::from(file),
            line: 1,
            message: "wrong".to_string(),
            severity,
            problem: None,
        }
    }

    /// The verifier's errors stop a program that reads the files strictly,
    /// and its warnings do not.
    #[test]
    fn errors_stop_a_program_and_warnings_do_not() {
        let mut warned = Report::default();
        warned.findings.push(finding("a.yaml", Severity::Warning));
        let mut startup = Startup::default();
        startup.report(&warned, true);
        assert!(!startup.fatal());
        assert_eq!(startup.lines(), &[(false, "a.yaml:1: warning: wrong".to_string())]);

        let mut wrong = Report::default();
        wrong.findings.push(finding("a.yaml", Severity::Error));
        startup.report(&wrong, true);
        assert!(startup.fatal());
    }

    /// What is copied is the heading and every line, in order.
    #[test]
    fn what_is_copied_is_the_heading_and_every_line() {
        let mut startup = Startup::default();
        startup.warning("a.yaml:1: warning: unfinished");
        startup.error("cannot listen on 10.0.0.1:5000");
        assert_eq!(
            startup.copied("Something is wrong."),
            "Something is wrong.\na.yaml:1: warning: unfinished\ncannot listen on 10.0.0.1:5000"
        );
    }

    /// An error in the file read loosely is said and does not stop it.
    #[test]
    fn an_error_in_the_loose_file_is_said_and_does_not_stop() {
        let mut report = Report::default();
        report.findings.push(finding("sim.yaml", Severity::Error));
        let mut startup = Startup::default();
        startup.report_loosely(&report, Path::new("sim.yaml"));
        assert!(!startup.fatal());
        assert!(!startup.is_empty());

        report.findings.push(finding("set.yaml", Severity::Error));
        let mut startup = Startup::default();
        startup.report_loosely(&report, Path::new("sim.yaml"));
        assert!(startup.fatal());
    }

    /// What a program adds of its own is said beside the verifier's.
    #[test]
    fn a_programs_own_lines_are_kept() {
        let mut startup = Startup::default();
        startup.warning("no beacons");
        assert!(!startup.fatal());
        startup.error("no link");
        assert!(startup.fatal());
        assert_eq!(startup.lines().len(), 2);
    }
}
