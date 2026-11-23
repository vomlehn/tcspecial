//! tcs-conflict run as a program: the shapes of its command line, and what
//! it says of two configurations that share everything and of two that
//! share nothing.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Runs tcs-conflict with `args`.
///
/// # Arguments
///
/// * `args` - Its command line.
///
/// # Returns
///
/// What it printed and its status.
fn run(args: &[&Path]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tcs-conflict"))
        .args(args)
        .output()
        .expect("tcs-conflict runs")
}

/// The shipped minimal example.
fn minimal() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../examples/minimal/minimal.ytcs")
}

/// The minimal example with everything it holds moved: every port up by
/// ten thousand, its own state file, log directory and payload address.
///
/// # Arguments
///
/// * `dir` - Where to write it.
///
/// # Returns
///
/// Its path.
fn moved(dir: &Path) -> PathBuf {
    let text = std::fs::read_to_string(minimal()).expect("the example");
    let mut text = text
        .replace("/tmp/tcspecial-minimal.state", "/tmp/other.state")
        .replace("/tmp/telemetry", "/tmp/other-telemetry")
        .replace("../../tcsprotocol", "../tcsprotocol");
    for port in [
        "5650", "5550", "4200", "4300", "6100", "6000", "4000", "4100", "5001",
        "5000", "5100",
    ] {
        text = text.replace(&format!(":{port}\""), &format!(":1{port}\""));
    }
    let path = dir.join("moved.ytcs");
    std::fs::write(&path, text).expect("written");
    path
}

#[test]
fn a_configuration_conflicts_with_itself() {
    let said = run(&[&minimal(), &minimal()]);
    assert_eq!(said.status.code(), Some(1));
    let out = String::from_utf8_lossy(&said.stdout);
    for wanted in ["state_file", "log.directory", ":4000", "\"sensor\""] {
        assert!(out.contains(wanted), "{wanted} in {out}");
    }
}

#[test]
fn two_configurations_that_share_nothing_pass() {
    let dir = tempfile::tempdir().expect("a directory");
    let said = run(&[&minimal(), &moved(dir.path())]);
    let out = String::from_utf8_lossy(&said.stdout);
    assert_eq!(said.status.code(), Some(0), "{out}");
    assert!(out.is_empty(), "{out}");
}

#[test]
fn a_command_line_of_another_shape_is_refused() {
    let sim = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../examples/minimal/minimal-sim.yaml");
    for args in [
        vec![minimal()],
        vec![minimal(), minimal(), minimal()],
        vec![sim.clone(), minimal()],
    ] {
        let args: Vec<&Path> = args.iter().map(PathBuf::as_path).collect();
        let said = run(&args);
        assert_eq!(said.status.code(), Some(2), "{args:?}");
    }
}
