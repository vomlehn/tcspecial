//! It carries each stream through to the other interface, and does not cross
//! them.
//!
//! Which is the whole of what an app with both interfaces can do that the
//! other two cannot, so it is what these tests are about. Driven through
//! `tcsapp::parent`, which is how tcspecial runs an app.

use std::path::PathBuf;
use std::time::Duration;

use tcsapp::parent::{self, Supports};

/// The program this crate builds.
fn app() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_app_all_veh"))
}

/// The arguments an `app_all_veh` is run with: its name, then the payload
/// argument, then the MOC one, this kind handling both streams.
fn arguments() -> Vec<String> {
    vec![
        "through".to_string(),
        "--readings".to_string(),
        "--commands".to_string(),
    ]
}

/// Each stream comes out of the interface it was going to.
#[test]
fn each_stream_is_carried_to_the_other_interface() {
    let mut running = parent::start(&app(), &arguments(), Supports::ALL).expect("it starts");

    // Groundward: payload data written at the first interface, where the app
    // reads payload data, comes out at the second, where it writes it.
    running.write_payload(b"293.15").expect("it takes a packet");
    assert_eq!(
        running.read_payload().expect("it is carried"),
        Some(b"293.15".to_vec())
    );

    // And vehicleward, the other way about.
    running.write_moc(b"SET 300").expect("it takes a packet");
    assert_eq!(
        running.read_moc().expect("it is carried"),
        Some(b"SET 300".to_vec())
    );

    // An empty packet on each, which is a packet.
    running
        .write_payload(b"")
        .expect("it takes an empty packet");
    assert_eq!(
        running.read_payload().expect("it is carried"),
        Some(Vec::new())
    );
    running.write_moc(b"").expect("it takes an empty packet");
    assert_eq!(running.read_moc().expect("it is carried"), Some(Vec::new()));

    running.no_more().expect("the interfaces end");
    assert!(
        running.waited_for().expect("it stops").success(),
        "an app whose interfaces ended has finished, not failed"
    );
}

/// And neither stream comes out of the interface it came in at.
///
/// An app that crossed the two would pass every test above: the same bytes
/// arrive either way, and only which interface they arrive on tells the two
/// apart. So this one waits on the wrong interface and wants nothing.
#[test]
fn the_streams_are_not_crossed() {
    let mut running = parent::start(&app(), &arguments(), Supports::ALL).expect("it starts");
    // Short, because what this test wants is for nothing to arrive: the
    // default would have it wait five seconds to find out.
    running.waits(Some(Duration::from_millis(500)));

    // Payload data in. Nothing comes back as MOC data, which is what the
    // first interface carries the other way.
    running.write_payload(b"293.15").expect("it takes a packet");
    let said = running.read_moc().expect_err("nothing should come back");
    assert!(
        said.contains("the payload interface"),
        "the wait should have been on the interface it was asked of: {said}"
    );

    // And it is where it should be, which proves the packet was carried and
    // not merely lost.
    assert_eq!(
        running.read_payload().expect("it is carried"),
        Some(b"293.15".to_vec())
    );

    running.no_more().expect("the interfaces end");
    assert!(running.waited_for().expect("it stops").success());
}

/// Run with the wrong number of arguments, it refuses and says so.
///
/// The list is fixed by the kind of app it is, so a program given a short one
/// would read the wrong word as its first argument. Refused where the
/// interfaces are refused, and for the same reason: something that started it
/// has misunderstood what it is.
#[test]
fn run_with_the_wrong_arguments_it_refuses_and_says_why() {
    let mut running =
        parent::start(&app(), &["through".to_string()], Supports::ALL).expect("it starts");

    let said = running
        .read_complaint()
        .expect("it complains")
        .expect("a message");
    assert!(said.contains("app_all_veh"), "{said}");
    assert!(said.contains("argument"), "{said}");

    assert!(!running.waited_for().expect("it stops").success());
}
