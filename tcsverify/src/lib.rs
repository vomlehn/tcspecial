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
//! What is checked:
//!
//! * The payload configuration file on its own: that it parses, that it
//!   describes payloads, that every payload and group is settled by the rules
//!   for its kind, and that no two payloads want one address or one device.
//! * The simulator file, if one is given: that it parses, and that it and the
//!   payload file agree -- every payload simulated, every simulated payload a
//!   payload, and every setting one that payload can have.
//!
//! The second is checked against the first only when the first has no
//! problems of its own. A payload file that did not settle has no handlers to
//! join to, and a join against half of them would report every missing
//! payload as unsimulated: a page of problems caused by this program rather
//! than by the files.

use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tcslibgs::{
    describes_payloads_text, ConfigFormat, DHConfig, PayloadConfig, SimConfigFile, TcsError,
};

pub mod locate;

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
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}: {}", self.file.display(), self.line, self.message)
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
    /// Whether the simulator file was checked against the payload file.
    ///
    /// False when no simulator file was given, and false when the payload
    /// file had problems of its own -- in which case the simulator file was
    /// still read and reported on for what it says by itself.
    pub joined: bool,
}

impl Report {
    /// Whether anything is wrong.
    pub fn found_errors(&self) -> bool {
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

    /// Write the findings, and say so if there were too many.
    pub fn write(&self, out: &mut impl Write) -> io::Result<()> {
        for finding in self.findings.iter().take(ERROR_LIMIT) {
            writeln!(out, "{finding}")?;
        }
        if self.halted {
            writeln!(out, "{TOO_MANY}")?;
        }
        Ok(())
    }

    fn add(&mut self, file: &Path, line: usize, message: impl Into<String>) {
        if self.halted {
            return;
        }
        self.findings.push(Finding {
            file: file.to_path_buf(),
            line,
            message: message.into(),
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

/// Check a payload configuration file, and a simulator file if there is one.
pub fn verify(payload_path: &Path, sim_path: Option<&Path>) -> Report {
    let mut report = Report::default();

    let handlers = check_payloads(payload_path, &mut report);

    if let Some(sim_path) = sim_path {
        // Only against a payload file that settled; see the module comment.
        let clean = handlers.as_ref().filter(|_| !report.found_errors());
        report.joined = clean.is_some();
        check_simulator(sim_path, clean.map(Vec::as_slice), &mut report);
    }

    report
}

/// Check the payload configuration file, and hand back what it describes.
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
fn line_of(e: &TcsError) -> usize {
    match e {
        TcsError::Yaml(e) => e.location().map_or(THE_FILE_ITSELF, |at| at.line()),
        _ => THE_FILE_ITSELF,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            .expect("the payload file")
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

    const GOOD: &str = "\
version: \"1.0\"
description: two payloads
payloads:
  - dh_id: 0
    name: DH0
    type: device
    path: /dev/zero
    packet_size: 4
  - dh_id: 1
    name: DH1
    type: network
    protocol: udp
    address: localhost
    port: 5000
    packet_size: 4
";

    const GOOD_SIM: &str = "\
version: \"1.0\"
description: settings
simulated_payloads:
  - name: DH0
    type: device
    packet_interval_ms: 1000
  - name: DH1
    type: network
    protocol: udp
    packet_interval_ms: 1000
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
  - dh_id: 0
    name: no_size
    type: device
    path: /dev/zero
  - dh_id: 1
    name: no_path
    type: device
    packet_size: 4
  - dh_id: 2
    name: bad_kind
    type: device
    path: /dev/null
    port: 5000
    packet_size: 4
";
        let set = written(".yaml", text, GOOD_SIM);
        let report = verify(&set.payload, None);

        assert_eq!(report.findings.len(), 3, "{:?}", report.findings);
        assert_eq!(report.status(), 1);

        // Each beside the payload it is about, in the order the file wrote
        // them.
        let lines: Vec<usize> = report.findings.iter().map(|f| f.line).collect();
        assert_eq!(lines, vec![5, 9, 13]);
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

    /// A finding says the file, the line and what is wrong, in that order.
    ///
    /// The shape an editor and a build log both read: `file:line: message`.
    #[test]
    fn a_finding_names_the_file_and_the_line() {
        let finding = Finding {
            file: PathBuf::from("tests/manual/tcspecial2.yaml"),
            line: 7,
            message: "payload \"DH0\" has no packet_size".to_string(),
        };
        assert_eq!(
            finding.to_string(),
            "tests/manual/tcspecial2.yaml:7: payload \"DH0\" has no packet_size"
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
            first
                .to_string()
                .starts_with(&format!("{}:{}: ", set.payload.display(), first.line)),
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
        let text = "version: \"1.0\"\ndescription: x\npayloads:\n  - dh_id: 0\n   name: bad\n";
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
version: \"1.0\"
description: settings
simulated_payloads:
  - name: DH0
    type: device
    packet_interval_ms: 1000
  - name: nobody
    type: device
    packet_interval_ms: 1000
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
            report.findings.iter().any(|f| f.line == 7),
            "the entry that names nobody is on line 7: {:?}",
            report.findings
        );
    }

    /// A simulator file is not joined to a payload file that did not settle.
    ///
    /// Every payload of the one is named in the other, so a join against the
    /// payloads that did settle reports the rest as unsimulated: a page of
    /// problems this program caused rather than found.
    #[test]
    fn a_broken_payload_file_is_not_joined_to() {
        let text = GOOD.replace("    packet_size: 4\n  - dh_id: 1", "  - dh_id: 1");
        let set = written(".yaml", &text, GOOD_SIM);
        let report = verify(&set.payload, Some(&set.sim));

        assert!(!report.joined);
        assert!(
            report.findings.iter().all(|f| f.file == set.payload),
            "the simulator file was faulted for the payload file's problems: {:?}",
            report.findings
        );
    }

    /// A simulator file that does not parse is reported whatever the payload
    /// file is like.
    #[test]
    fn a_simulator_file_is_read_even_when_the_other_one_is_broken() {
        let text = GOOD.replace("    packet_size: 4\n  - dh_id: 1", "  - dh_id: 1");
        let set = written(
            ".yaml",
            &text,
            "simulated_payloads:\n  - name: DH0\n   type: device\n",
        );
        let report = verify(&set.payload, Some(&set.sim));
        assert!(
            report.findings.iter().any(|f| f.file == set.sim),
            "the simulator file was not read: {:?}",
            report.findings
        );
    }

    /// A hundred and one problems stop the run, and the hundred and first is
    /// not printed: it is the one that says there are more.
    #[test]
    fn too_many_problems_stop_the_run() {
        // Each payload states a port it cannot have, so each is one problem.
        let mut text = "version: \"1.0\"\ndescription: many\npayloads:\n".to_string();
        for id in 0..150 {
            text.push_str(&format!(
                "  - dh_id: {id}\n    name: DH{id}\n    type: device\n    \
                 path: /dev/null{id}\n    port: 1\n    packet_size: 4\n"
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
        assert_eq!(lines.len(), ERROR_LIMIT + 1, "a hundred and the message");
        assert_eq!(lines[ERROR_LIMIT], TOO_MANY);
        assert_eq!(report.status(), 1);
    }

    /// Exactly a hundred is not too many: the message says there are more
    /// than were printed, so it must not appear when there are not.
    #[test]
    fn a_hundred_problems_are_all_printed_and_nothing_more_is_said() {
        let mut text = "version: \"1.0\"\ndescription: many\npayloads:\n".to_string();
        for id in 0..ERROR_LIMIT {
            text.push_str(&format!(
                "  - dh_id: {id}\n    name: DH{id}\n    type: device\n    \
                 path: /dev/null{id}\n    port: 1\n    packet_size: 4\n"
            ));
        }
        let set = written(".yaml", &text, GOOD_SIM);
        let report = verify(&set.payload, None);

        assert_eq!(report.findings.len(), ERROR_LIMIT);
        assert!(!report.halted);

        let mut printed = Vec::new();
        report.write(&mut printed).expect("written");
        let printed = String::from_utf8(printed).expect("text");
        assert_eq!(printed.lines().count(), ERROR_LIMIT);
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
}
