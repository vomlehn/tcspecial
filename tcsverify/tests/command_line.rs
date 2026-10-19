//! The program itself, run as a person or a Makefile runs it.
//!
//! The library's tests say what a run finds under each limit; these say that
//! the command line reaches them -- that an option is taken as an option and
//! not as a file to check, which nothing short of running the program shows.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A configuration with this many errors in it, one per payload: each sends
/// to a node that is not there.
fn with_errors(dir: &Path, count: usize) -> PathBuf {
    let mut text = "\
version: \"2.0\"
description: many
tcspecial:
  beacon:
    interval: \"5 s\"
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
"
    .to_string();
    for id in 0..count {
        text.push_str(&format!(
            "  - name: DH{id}\n    type: device-file\n    path: /dev/null{id}\n    \
             payload-out:\n      - target: nowhere{id}\n"
        ));
    }
    let path = dir.join("set.yaml");
    std::fs::write(&path, text).expect("written");
    path
}

/// The lines printed and the exit status, for these arguments.
fn run(args: &[&str]) -> (Vec<String>, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_tcsverify"))
        .args(args)
        .output()
        .expect("tcsverify runs");
    let printed = String::from_utf8(out.stdout).expect("text");
    (
        printed.lines().map(str::to_string).collect(),
        out.status.code().expect("an exit status"),
    )
}

#[test]
fn the_options_are_taken_as_options() {
    let dir = tempfile::tempdir().expect("a directory");
    let set = with_errors(dir.path(), 12);
    let set = set.to_str().expect("a path");

    // Every one unless asked.
    let (lines, status) = run(&[set]);
    assert_eq!(lines.len(), 12, "{lines:?}");
    assert_eq!(status, 1);

    // Before the file or after it, either way.
    for args in [
        ["--error-limit=2", set],
        [set, "--error-limit=2"],
    ] {
        let (lines, status) = run(&args);
        assert_eq!(lines.len(), 2, "{args:?}: {lines:?}");
        assert!(lines.iter().all(|line| line.contains(": error: ")), "{lines:?}");
        assert_eq!(status, 1);
    }

    // Nought prints nothing and passes.
    let (lines, status) = run(&["--error-limit=0", set]);
    assert!(lines.is_empty(), "{lines:?}");
    assert_eq!(status, 0);

    // Every one, and what is done with warnings does not touch the errors.
    let (lines, status) = run(&["--error-limit=unlimited", "--warnings=off", set]);
    assert_eq!(lines.len(), 12, "{lines:?}");
    assert_eq!(status, 1);
}

#[test]
fn a_bad_option_is_a_command_line_of_none_of_the_shapes() {
    let dir = tempfile::tempdir().expect("a directory");
    let set = with_errors(dir.path(), 1);
    let set = set.to_str().expect("a path");
    for bad in ["--error-limit=five", "--error-limit=", "--warnings=maybe", "--warnings="] {
        let (lines, status) = run(&[bad, set]);
        assert_eq!(status, 2, "{bad}");
        assert!(lines.is_empty(), "{bad}: nothing is checked: {lines:?}");
    }
}
