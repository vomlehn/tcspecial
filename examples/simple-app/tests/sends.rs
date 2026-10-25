//! The payload sends the line its node names, a word at a time.
//!
//! Driven through `tcsapp::parent`, which is how tcspecial runs an app: so
//! what this test starts is what the vehicle starts, with the arguments the
//! configuration gives it. No vehicle and no ground station are needed to say
//! whether the words go out in the right order.
//!
//! The payload is `lorem_pl.py`, a Python 3 program with its own copy of the
//! text. What it sends is compared here with this crate's copy, which is the
//! one the ground reads, so the two cannot drift apart unnoticed.
//!
//! Skipped, loudly, where `python3` is not installed: a test that fails
//! because a tool is absent teaches a reader to ignore it.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use tcsapp::parent::{self, Supports};

/// The payload program, which tcspecial runs as the configuration names it.
fn app() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lorem_pl.py")
}

/// Whether there is a Python 3 to run it with.
fn python3() -> bool {
    let there = Command::new("python3").arg("--version").output().is_ok();
    if !there {
        eprintln!("python3 is not installed: lorem_pl.py was not run");
    }
    there
}

/// What an `app_pl` is run with: the node's name, then the argument for the
/// one stream it handles, which for this payload is the line to send.
fn arguments(line: &str) -> Vec<String> {
    vec!["lorem".to_string(), line.to_string()]
}

/// The first line arrives as one packet per word, then the terminator.
#[test]
fn the_line_arrives_a_word_at_a_time() {
    if !python3() {
        return;
    }
    let mut running = parent::start(&app(), &arguments("1"), Supports::MOC)
        .expect("it starts");
    // The payload waits for a ground station to have said hello before its
    // first word, and no ground station is coming: see `lorem_pl.py`.
    running.waits(Some(Duration::from_secs(20)));

    let first = simple_app::line(1).expect("a paragraph has a first line");
    for word in first.words() {
        assert_eq!(
            running.read_payload().expect("a word"),
            Some(word.as_bytes().to_vec()),
            "the words go out in the order they are written in"
        );
    }

    // And then what ends the line, which is how the ground knows the line is
    // whole without being told how many words to expect.
    assert_eq!(
        running.read_payload().expect("the end of the line"),
        Some(simple_app::TERMINATOR.as_bytes().to_vec())
    );

    // It stays where it is afterwards rather than ending its interface: a
    // program that had finished would have the vehicle reporting every pass
    // that nothing more will come from it.
    assert!(
        running.stopped().expect("it can be asked").is_none(),
        "a payload that has said everything is still a payload"
    );
    assert!(running.stop().is_ok());
}

/// A node naming a line the paragraph has not got is refused, and says so.
#[test]
fn a_line_that_is_not_there_is_refused() {
    if !python3() {
        return;
    }
    let mut running = parent::start(&app(), &arguments("99"), Supports::MOC)
        .expect("it starts");

    let said = running
        .read_complaint()
        .expect("it complains")
        .expect("a message");
    assert!(said.contains("line 99"), "{said}");
    assert!(said.contains("lorem"), "{said}");
    assert!(!running.waited_for().expect("it stops").success());
}

/// And an argument that is not a line number at all.
#[test]
fn an_argument_that_is_not_a_line_number_is_refused() {
    if !python3() {
        return;
    }
    let mut running = parent::start(&app(), &arguments("two"), Supports::MOC)
        .expect("it starts");

    let said = running
        .read_complaint()
        .expect("it complains")
        .expect("a message");
    assert!(said.contains("app-payload-arg"), "{said}");
    assert!(said.contains("two"), "{said}");
    assert!(!running.waited_for().expect("it stops").success());
}
