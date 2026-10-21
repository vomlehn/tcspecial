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
description: many
tcspecial:
  gateway: radio
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
    for args in [["--error-limit=2", set], [set, "--error-limit=2"]] {
        let (lines, status) = run(&args);
        assert_eq!(lines.len(), 2, "{args:?}: {lines:?}");
        assert!(
            lines.iter().all(|line| line.contains(": error: ")),
            "{lines:?}"
        );
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

/// The digest is printed for the configurations, and for them only.
///
/// The first pathname of two and the first two of three: what is left is the
/// payload simulator file, which has no digest of this kind -- it is not a
/// graph, and nothing aboard reads it, so there is nothing on the other end
/// to compare one with.
///
/// What the digest *is* is checked in `tcslibgs`, which takes it. Checked
/// here: that the flag is taken as a flag, which file it is taken of, and
/// that the two spellings of one configuration come out alike -- that last
/// being the whole point of digesting what a file says rather than its bytes,
/// and the one thing a reader of this output relies on.
#[test]
fn the_digest_is_printed_for_the_configurations() {
    let example = |name: &str| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../examples/thermostat")
            .join(name)
            .to_str()
            .expect("a path")
            .to_string()
    };
    let (yaml, xml, sim) = (
        example("thermostat.yaml"),
        example("thermostat.xml"),
        example("thermostat-sim.yaml"),
    );
    let digest = |line: &str| line.split("md5 ").nth(1).map(str::to_string);

    // One configuration: its digest, and the flag was not taken for a file.
    let (lines, status) = run(&["--digest", &yaml]);
    assert_eq!(status, 0, "{lines:?}");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].starts_with(&yaml), "{lines:?}");
    let said = digest(&lines[0]).unwrap_or_else(|| panic!("{lines:?}"));
    assert_eq!(said.len(), 32, "a digest is sixteen bytes of hex: {said}");
    assert!(said.chars().all(|c| c.is_ascii_hexdigit()), "{said}");

    // A configuration and its simulator file: the first of the two.
    let (lines, status) = run(&["--digest", &yaml, &sim]);
    assert_eq!(status, 0, "{lines:?}");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(digest(&lines[0]), Some(said.clone()));

    // Two spellings and their simulator file: both configurations, and the
    // two digests are one digest.
    let (lines, status) = run(&["--digest", &xml, &yaml, &sim]);
    assert_eq!(status, 0, "{lines:?}");
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[0].starts_with(&xml) && lines[1].starts_with(&yaml),
        "{lines:?}"
    );
    assert_eq!(
        digest(&lines[0]),
        digest(&lines[1]),
        "the two spellings of one configuration are one configuration"
    );
    assert_eq!(digest(&lines[0]), Some(said));
}

/// A configuration with problems still has a digest; one that cannot be read
/// has none, and saying so is a failed run.
///
/// The second half is why the status is checked under `--error-limit=0`: that
/// limit prints no errors and passes, so without this a run that was asked
/// for a digest and could not take one would end silently and successfully
/// having answered nothing.
#[test]
fn a_digest_that_cannot_be_taken_is_a_failed_run() {
    let dir = tempfile::tempdir().expect("a directory");

    // Twelve errors in it, and it still parses: a digest is of what a file
    // says, and this file says something.
    let set = with_errors(dir.path(), 12);
    let set = set.to_str().expect("a path");
    let (lines, status) = run(&["--digest", set]);
    assert_eq!(status, 1, "{lines:?}");
    assert_eq!(lines.len(), 13, "the digest and the twelve: {lines:?}");
    assert!(
        lines[0].contains(": md5 "),
        "the digest comes first: {lines:?}"
    );

    // And one that does not parse has nothing to digest.
    let unreadable = dir.path().join("unreadable.yaml");
    std::fs::write(&unreadable, "description: nonsense\npayloads: oops\n").expect("written");
    let unreadable = unreadable.to_str().expect("a path");

    let (lines, status) = run(&["--digest", unreadable]);
    assert_eq!(status, 1, "{lines:?}");
    assert!(
        !lines.iter().any(|line| line.contains("md5")),
        "there is no digest to print: {lines:?}"
    );

    // Even where nothing was counted against it.
    let (lines, status) = run(&["--digest", "--error-limit=0", unreadable]);
    assert_eq!(status, 1, "a digest was asked for and not given: {lines:?}");
    assert!(lines.is_empty(), "{lines:?}");
}

#[test]
fn a_bad_option_is_a_command_line_of_none_of_the_shapes() {
    let dir = tempfile::tempdir().expect("a directory");
    let set = with_errors(dir.path(), 1);
    let set = set.to_str().expect("a path");
    for bad in [
        "--error-limit=five",
        "--error-limit=",
        "--warnings=maybe",
        "--warnings=",
    ] {
        let (lines, status) = run(&[bad, set]);
        assert_eq!(status, 2, "{bad}");
        assert!(lines.is_empty(), "{bad}: nothing is checked: {lines:?}");
    }
}

/// A configuration whose one vehicle node is written as `node`, beside a
/// ground link it is joined to both ways.
fn with_vehicle_node(dir: &Path, node: &str) -> PathBuf {
    let text = format!(
        "\
description: one vehicle node
tcspecial:
  gateway: radio
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
moc:
  - name: ground
    type: moc
    address: \"127.0.0.1\"
    port: \"5000\"
    ground_base: \"127.0.0.1:5100\"
    moc-out:
      - target: pinger
vehicle:
  - name: pinger
{node}
    payload-out:
      - target: ground
"
    );
    let path = dir.join("one.yaml");
    std::fs::write(&path, text).expect("written");
    path
}

/// Warnings and no errors is a run that passes: the warnings are printed,
/// and the exit status is success.
#[test]
fn warnings_alone_are_a_run_that_passes() {
    let dir = tempfile::tempdir().expect("a directory");
    let file = with_vehicle_node(dir.path(), "    type: ping\n    interval: \"1 s\"");

    let (lines, status) = run(&[file.to_str().expect("a path")]);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains(": warning: "), "{lines:?}");
    assert_eq!(status, 0);
}

/// A node of a type that does not exist is an error, and fails the run.
#[test]
fn a_type_that_does_not_exist_fails_the_run() {
    let dir = tempfile::tempdir().expect("a directory");
    let file = with_vehicle_node(dir.path(), "    type: bogus");

    let (lines, status) = run(&[file.to_str().expect("a path")]);
    assert!(
        lines
            .iter()
            .any(|line| line.contains(": error: ") && line.contains("\"bogus\"")),
        "{lines:?}"
    );
    assert_eq!(status, 1);
}
