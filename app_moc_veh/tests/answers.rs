//! It answers what arrives, and cannot reach the interface it has not got.
//!
//! The mirror of `app_payload_veh`'s tests: MOC data in, payload data out.
//! Driven through `tcsapp::parent`, which is how tcspecial runs an app.

use std::path::PathBuf;

use tcsapp::parent::{self, Supports};

/// The program this crate builds.
fn app() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_app_moc_veh"))
}

/// The arguments an `app_moc_veh` is run with: its name, then the argument
/// for the one stream its kind handles.
fn arguments() -> Vec<String> {
    vec!["back".to_string(), "--commands".to_string()]
}

/// MOC data in, the same bytes out as payload data.
#[test]
fn a_packet_of_moc_data_comes_back_as_payload_data() {
    let mut running =
        parent::start(&app(), &arguments(), Supports::MOC).expect("it starts");

    running.write_moc(b"SET 300").expect("it takes a packet");
    assert_eq!(
        running.read_payload().expect("an answer"),
        Some(b"SET 300".to_vec())
    );

    running.write_moc(b"").expect("an empty packet is a packet");
    assert_eq!(running.read_payload().expect("an answer"), Some(Vec::new()));

    running.no_more().expect("the interfaces end");
    assert!(running.waited_for().expect("it stops").success());
}

/// The payload interface is not this kind of app's.
#[test]
fn the_interface_it_has_not_got_cannot_be_reached() {
    let mut running =
        parent::start(&app(), &arguments(), Supports::MOC).expect("it starts");

    let said = running
        .write_payload(b"293.15")
        .expect_err("an interface it has not got");
    assert!(said.contains("the payload interface"), "{said}");
    assert!(said.contains("descriptor 0"), "{said}");

    running.no_more().expect("the interfaces end");
    assert!(running.waited_for().expect("it stops").success());
}

/// Run as a kind of app it is not, it refuses and says why.
#[test]
fn run_as_a_kind_it_is_not_it_refuses_and_says_why() {
    let mut running =
        parent::start(&app(), &arguments(), Supports::ALL).expect("it starts");

    let said = running
        .read_complaint()
        .expect("it complains")
        .expect("a message");
    assert!(said.contains("app_moc_veh"), "{said}");
    assert!(said.contains("the payload interface"), "{said}");

    assert!(!running.waited_for().expect("it stops").success());
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
        parent::start(&app(), &["back".to_string()], Supports::MOC)
            .expect("it starts");

    let said = running
        .read_complaint()
        .expect("it complains")
        .expect("a message");
    assert!(said.contains("app_moc_veh"), "{said}");
    assert!(said.contains("argument"), "{said}");

    assert!(!running.waited_for().expect("it stops").success());
}
