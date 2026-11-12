//! The stage configurations of "Building a Vehicle" are checked as the
//! examples are.
//!
//! Each stage of the guide is a directory of `docs/building-a-vehicle/`,
//! `stage-NN`, holding `deck.ytcs`, the same in XML, `deck.xtcs`, and the
//! simulator file beside them, `deck-sim.yaml`. They are discovered rather
//! than listed, so a stage added to the guide is checked without anyone
//! having to name it here. The files the later stages include, in
//! `common/`, are not stages: each is checked as part of every stage that
//! includes it.
//!
//! The guide is taught from these files, so a rule that changes and leaves
//! one behind fails here rather than in a reader's hands.

use std::path::{Path, PathBuf};

use tcslibgs::graph::Graph;
use tcsverify::verify;

/// The directory the guide's stages are in.
///
/// # Returns
///
/// `docs/building-a-vehicle`, from this crate's directory.
fn guide_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/building-a-vehicle")
}

/// Finds every stage's configuration in YAML.
///
/// # Returns
///
/// Each `stage-*/deck.ytcs`, sorted.
///
/// # Panics
///
/// If the guide's directory cannot be read, or holds no stage, which would
/// otherwise pass every test here by checking nothing.
fn stages() -> Vec<PathBuf> {
    let dir = guide_dir();
    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("stage-"))
        })
        .map(|stage| stage.join("deck.ytcs"))
        .collect();
    found.sort();
    assert!(!found.is_empty(), "no stages in {}", dir.display());
    found
}

/// Every stage is a configuration and its simulator file, and nothing is
/// found wrong or unfinished in either. Warnings count, as they do for the
/// examples: a reader copies a stage.
#[test]
fn every_stage_verifies() {
    for config in stages() {
        let sim = config.with_file_name("deck-sim.yaml");
        assert!(
            sim.is_file(),
            "{} has no {}",
            config.display(),
            sim.display()
        );

        let report = verify(&config, Some(&sim), tcsverify::Limits::EVERYTHING);
        let findings: Vec<String> =
            report.findings.iter().map(|f| f.to_string()).collect();
        assert!(
            findings.is_empty(),
            "{}:\n{}",
            config.display(),
            findings.join("\n")
        );
        assert!(
            report.joined,
            "{} was not checked against {}",
            sim.display(),
            config.display()
        );
    }
}

/// Every stage is written in both spellings, and the two read as one
/// configuration, the files each includes included.
#[test]
fn every_stage_reads_the_same_in_both_formats() {
    for yaml in stages() {
        let xml = yaml.with_extension("xtcs");
        assert!(
            xml.is_file(),
            "{} has no {} beside it: a stage is written in both formats",
            yaml.display(),
            xml.display()
        );
        assert_eq!(
            load(&xml),
            load(&yaml),
            "{} and {} describe different configurations",
            xml.display(),
            yaml.display()
        );
    }
}

/// Loads a configuration, less where it was read from, which is the one
/// thing its two spellings differ in by rights.
///
/// # Arguments
///
/// * `path` - The configuration's top file.
///
/// # Returns
///
/// The configuration, with no files and no homes.
///
/// # Panics
///
/// If it does not load.
fn load(path: &Path) -> Graph {
    let mut graph =
        Graph::load(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    graph.files.clear();
    graph.homes.clear();
    graph
}

/// Every stage, in both spellings, names this build's interface: a change
/// to `tcslibgs/xtce/link.xtce` that left one behind fails here, saying the
/// one command that brings them all up to date.
#[test]
fn every_stage_names_this_builds_interface() {
    let ours = tcslibgs::InterfaceDigest::of_this_build().to_string();
    let mut stale = Vec::new();
    for yaml in stages() {
        for path in [yaml.clone(), yaml.with_extension("xtcs")] {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let xml = path.extension().is_some_and(|e| e == "xtcs");
            let (_, set) =
                tcsverify::set_interface::set_in_text(&text, xml, &ours)
                    .unwrap_or_else(|why| panic!("{}: {why}", path.display()));
            if set != tcsverify::set_interface::Set::Already {
                stale.push(path.display().to_string());
            }
        }
    }
    assert!(
        stale.is_empty(),
        "these do not name this build's interface, {ours}: run `make \
         set-interfaces` (tcsverify --set-interface) and commit them: \
         {stale:?}"
    );
}
