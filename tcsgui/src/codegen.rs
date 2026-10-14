//! The configuration, as Rust.
//!
//! What tcspecial does today is read a configuration file when it starts. What
//! this is for is the other way of building it: the configuration becomes a
//! Rust source file, compiled into the executable along with the node types it
//! uses and the infrastructure around them. A vehicle built that way carries no
//! configuration file at all, and nothing aboard has to parse one.
//!
//! The file is written where the configuration's own `tcspecial` section says,
//! under `rust-code-path` -- a path, relative to the configuration's own directory
//! or absolute -- and it is written when somebody asks for it from the Edit
//! menu, not on every save: generating it is a second output of the editor and
//! not part of what editing means.
//!
//! # What is generated, and what is not
//!
//! This generates the part that varies between configurations: the nodes, what
//! each of them says about itself, the links between them, the payload groups,
//! the command bindings, and what tcspecial itself is configured with. The part
//! that does not vary -- what an i2c node does, what a mean computes, what the
//! command interpreter answers -- is the code that is already in `tcspecial`
//! and `tcslibgs`, and the generated file names those types rather than
//! repeating them.
//!
//! It builds a [`tcslibgs::graph::Graph`], which is what the flight software
//! already works in: `Graph::check` is what says a configuration can be flown
//! and `runtime::start` is what opens it, so a graph that was built in Rust
//! rather than read from a file arrives at the same two functions. That is the
//! whole point of generating this rather than something cleverer -- a second
//! representation of a configuration would be a second thing to keep correct,
//! and the one that is checked is this one.
//!
//! What is still to come is the build that uses it: a tcspecial whose
//! `main` calls `configuration()` instead of reading a path. **Needs
//! examination**, and it is a change to how the flight software is built
//! rather than to what the editor writes, which is why it is not here.
//!
//! # Why it is generated rather than hand-written
//!
//! Because it is derived. A person who edits the diagram and then edits this
//! file by hand has two configurations, and the one that flies is whichever
//! was compiled last. So the file says, at the top, that it is generated and
//! from what -- and every generation overwrites it whole.

use std::path::{Path, PathBuf};

use crate::document::Document;

/// Where a configuration's generated Rust goes, if it says.
///
/// `rust-code-path` in the `tcspecial` section: a path as somebody typed it, which
/// may be relative. Relative to the configuration's own directory and not to
/// wherever the editor happened to be started from -- a configuration and the
/// code generated from it belong together, and a path that meant different
/// files depending on the working directory would be a path nobody could
/// check.
pub fn where_to(document: &Document, configuration: &Path) -> Option<PathBuf> {
    let said = document
        .tcspecial
        .get(tcslibgs::graph::parameters::RUST_CODE)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())?;

    let said = PathBuf::from(said);
    if said.is_absolute() {
        return Some(said);
    }
    Some(
        configuration
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(said),
    )
}

/// What is wrong with generating the code, where something is.
///
/// Said as the editor says everything else that cannot be done: in words, in a
/// window, with nothing written. A configuration with no `rust-code-path` is
/// not a fault -- it is a configuration nobody has asked this of -- but asking
/// for the code without having said where it goes is worth a sentence.
///
/// And the configuration itself has to hold together. The editor is lazy on
/// purpose while a diagram is being drawn -- a node placed and not yet filled
/// in is how anybody works -- and generating code is the one command where
/// that laziness has to end: what comes out of it is compiled into the
/// program the vehicle runs, so a sensor whose device nobody named would be a
/// payload the binary cannot open. Both kinds stop it, the unfinished as well
/// as the wrong: see `tcslibgs::graph::check::Severity`.
pub fn trouble(document: &Document, configuration: &Path) -> Option<String> {
    if where_to(document, configuration).is_none() {
        return Some(format!(
            "this configuration does not say where its generated code goes: set \
             {} in the tcspecial parameters, under Edit",
            tcslibgs::graph::parameters::RUST_CODE
        ));
    }
    unfinished(configuration)
}

/// What the configuration check says, as one sentence, where it says anything.
///
/// Asked of the files rather than of the document in hand, because the rules
/// are `tcslibgs`'s and they are written against a `Graph`: every edit is
/// written as it is made, so the files say what the document says -- and where
/// a write failed, the editor is already saying so.
///
/// The first few and then a count of the rest, which is the shape `tcsverify`
/// prints: `check::LISTED` and `check::AND_OTHERS` are shared so that somebody
/// who has run the verifier meets a report they recognise. Errors before
/// warnings, for the same reason the verifier puts them there.
fn unfinished(configuration: &Path) -> Option<String> {
    use tcslibgs::graph::check::{Severity, AND_OTHERS, LISTED};

    let graph = match tcslibgs::graph::Graph::load(configuration) {
        Ok(graph) => graph,
        // The file the editor has been writing cannot be read back, which is
        // a worse thing than this command: said plainly, with the name of the
        // file to look at.
        Err(e) => {
            return Some(format!(
                "{} cannot be read back, so there is nothing to generate from: {e}",
                configuration.display()
            ))
        }
    };

    let problems = graph.check();
    if problems.is_empty() {
        return None;
    }

    let mut ordered: Vec<&tcslibgs::graph::check::Problem> = problems
        .iter()
        .filter(|problem| problem.severity() == Severity::Error)
        .collect();
    ordered.extend(
        problems
            .iter()
            .filter(|problem| problem.severity() == Severity::Warning),
    );

    let listed: Vec<String> = ordered
        .iter()
        .take(LISTED)
        .map(|problem| problem.describe())
        .collect();
    let rest = ordered.len().saturating_sub(LISTED);
    let others = if rest > 0 {
        format!("; {AND_OTHERS} ({rest} more)")
    } else {
        String::new()
    };

    Some(format!(
        "the code cannot be generated from a configuration that is not \
         finished. {}{others}. Put these right and ask again -- tcsverify \
         lists all of them, with the line of the file each is on",
        listed.join("; ")
    ))
}

/// Run the generator on this configuration, writing where the section says.
///
/// A program of its own -- `tcsrustgen` -- rather than a corner of the editor:
/// generating the flight software's configuration should not require a window,
/// so a Makefile and this menu item run the same thing the same way. What the
/// editor adds is the question of whether it is worth running, which is
/// `trouble` above, and somebody to tell when it is not.
///
/// Found beside this executable first and on the `PATH` after. A workspace
/// built with cargo puts the two binaries in one directory, which is where a
/// person running the editor out of a checkout has it; an installed editor has
/// an installed generator, which is on the path. Said plainly where it is
/// neither, because "no such file or directory" about a program nobody asked
/// for by name is a sentence that explains nothing.
pub fn run(configuration: &Path, output: &Path) -> Result<String, String> {
    let program = program();
    let said = std::process::Command::new(&program)
        .arg(configuration)
        .arg(output)
        .output();

    match said {
        Ok(said) if said.status.success() => Ok(format!("wrote {}", output.display())),
        Ok(said) => {
            // What the generator said about it, which is a sentence about a
            // file: its own output is better than anything this could add.
            let why = String::from_utf8_lossy(&said.stderr).trim().to_string();
            Err(if why.is_empty() {
                format!(
                    "{} could not generate the code, and said nothing about why",
                    program.display()
                )
            } else {
                why
            })
        }
        Err(e) => Err(format!(
            "{GENERATOR} could not be run: {e}. It is the program that writes \
             the configuration as Rust, and it is built and installed beside \
             this editor"
        )),
    }
}

/// The name of the program that does the generating.
const GENERATOR: &str = "tcsrustgen";

/// Which program to run, in the order the places are tried.
///
/// `TCSRUSTGEN` first, for somebody who keeps it somewhere of their own.
/// Then beside this executable, which is what a workspace built with cargo and
/// an installed pair both look like. Then one directory above that, which is
/// where the pair sits when this is a test binary under `target/debug/deps`.
/// And failing all of those, the bare name, which leaves the `PATH` to answer.
fn program() -> PathBuf {
    if let Some(said) = std::env::var_os("TCSRUSTGEN") {
        return PathBuf::from(said);
    }
    if let Ok(ours) = std::env::current_exe() {
        for directory in [ours.parent(), ours.parent().and_then(|at| at.parent())] {
            if let Some(at) = directory {
                let beside = at.join(GENERATOR);
                if beside.is_file() {
                    return beside;
                }
            }
        }
    }
    PathBuf::from(GENERATOR)
}

/// Whether the generator can be run at all.
///
/// For the tests, which check what the editor does with what it says: a test
/// that failed because the generator has not been built would be a test that
/// says nothing about the editor. See [`program`] for where it is looked for.
pub fn available() -> bool {
    std::process::Command::new(program()).output().is_ok()
}

/// A string as Rust spells it.
fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// An `Option<String>` as Rust spells it.
fn option_of(value: Option<&str>) -> String {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => format!("Some({}.to_string())", quoted(value)),
        None => "None".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::Edit;
    use std::path::Path;

    /// Where the code goes, as the section says it.
    #[test]
    fn the_section_says_where_the_code_goes() {
        let mut document = Document::new();
        let configuration = Path::new("/vehicles/aurora/vehicle.yaml");

        // Nothing said, which is not a fault: a configuration nobody has asked
        // this of. Asking for it anyway is told what to do about it.
        assert_eq!(where_to(&document, configuration), None);
        let said = trouble(&document, configuration).expect("a sentence");
        assert!(said.contains("rust-code-path"), "{said}");

        // Relative to the configuration's own directory, and not to wherever
        // the editor was started from.
        document.apply(&Edit::SetTcspecial {
            attribute: "rust-code-path".to_string(),
            was: None,
            now: Some("generated/aurora.rs".to_string()),
        });
        assert_eq!(
            where_to(&document, configuration),
            Some(PathBuf::from("/vehicles/aurora/generated/aurora.rs"))
        );
        // An absolute path is where it says.
        document.apply(&Edit::SetTcspecial {
            attribute: "rust-code-path".to_string(),
            was: Some("generated/aurora.rs".to_string()),
            now: Some("/tmp/aurora.rs".to_string()),
        });
        assert_eq!(
            where_to(&document, configuration),
            Some(PathBuf::from("/tmp/aurora.rs"))
        );
    }

    /// A configuration that is not finished is refused, and told why.
    ///
    /// The command this guards is the one whose output is compiled into the
    /// program the vehicle runs. Everywhere else the editor is lazy on
    /// purpose; here a node nobody has finished is a payload the binary could
    /// not open, so the answer is the list of what is missing and nothing
    /// written.
    #[test]
    fn an_unfinished_configuration_cannot_be_generated_from() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.yaml");

        // The worked example, which is finished, with the parameter set: the
        // code is generated from it and this says nothing.
        let mut document = crate::document::thermostat_example();
        document.apply(&Edit::SetTcspecial {
            attribute: "rust-code-path".to_string(),
            was: None,
            now: Some("generated/vehicle.rs".to_string()),
        });
        std::fs::write(&path, document.to_yaml()).expect("the configuration");
        assert_eq!(trouble(&document, &path), None, "{:?}", trouble(&document, &path));

        // One node more, placed and not yet told anything: refused, and the
        // sentence names the node and what it is short of.
        document.apply(&Edit::AddNode {
            name: "temp_new".to_string(),
            node_type: "i2c".to_string(),
            home: None,
        });
        std::fs::write(&path, document.to_yaml()).expect("the configuration");
        let said = trouble(&document, &path).expect("a sentence");
        assert!(said.contains("not finished"), "{said}");
        assert!(said.contains("temp_new"), "{said}");
        assert!(said.contains("path"), "{said}");

        // And where there are more than a few, the rest are counted rather
        // than listed: the same shape tcsverify prints.
        for id in 0..6 {
            document.apply(&Edit::AddNode {
                name: format!("temp_{id}"),
                node_type: "i2c".to_string(),
                home: None,
            });
        }
        std::fs::write(&path, document.to_yaml()).expect("the configuration");
        let said = trouble(&document, &path).expect("a sentence");
        assert!(
            said.contains(tcslibgs::graph::check::AND_OTHERS),
            "the rest should be counted: {said}"
        );
        assert_eq!(
            said.matches("; ").count(),
            tcslibgs::graph::check::LISTED,
            "the first few and then the count: {said}"
        );
    }


}
