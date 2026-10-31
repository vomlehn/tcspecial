//! It reports without being asked, and cannot be asked.
//!
//! The payload-side mirror of `app_gateway_veh`'s tests: nothing goes in,
//! readings come out. Driven through `tcsapp::parent`, which is how tcspecial
//! runs an app.

use std::path::PathBuf;

use tcsapp::parent::{self, Supports};

/// The program this crate builds.
fn app() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_app_pl"))
}

/// The arguments an `app_pl` is run with: its name, then the argument for the
/// one stream it handles, which it labels its readings with.
fn arguments() -> Vec<String> {
    vec!["bay".to_string(), "--bay-2".to_string()]
}

/// Readings arrive with nothing having been asked for them.
///
/// Which is what this kind of payload is: it reports on a clock of its own, so
/// a second packet comes along after the first without this end doing anything
/// at all. Each reading carries the argument the node was given, which is how
/// this test knows the configuration's word reached the program: a payload
/// that labelled its readings with somebody else's argument would be a node
/// whose telemetry arrived under the wrong name.
#[test]
fn readings_arrive_without_being_asked() {
    let mut running = parent::start(&app(), &arguments(), Supports::GATEWAY)
        .expect("it starts");

    assert_eq!(
        running.read_payload().expect("a reading"),
        Some(b"--bay-2 1".to_vec())
    );
    assert_eq!(
        running.read_payload().expect("another reading"),
        Some(b"--bay-2 2".to_vec()),
        "it reports again without being asked"
    );

    assert!(
        running.stop().is_ok(),
        "a payload that is read until it is not"
    );
}

/// The payload interface is not this kind of app's.
///
/// Descriptor 0 is closed in it: a payload writes its readings, and the
/// interface payload data is written on is the other one.
#[test]
fn the_interface_it_has_not_got_cannot_be_reached() {
    let mut running = parent::start(&app(), &arguments(), Supports::GATEWAY)
        .expect("it starts");

    let said = running
        .write_payload(b"293.15")
        .expect_err("an interface it has not got");
    assert!(said.contains("the payload interface"), "{said}");
    assert!(said.contains("descriptor 0"), "{said}");

    assert!(running.stop().is_ok());
}

/// Run as a kind of app it is not, it refuses and says why.
///
/// Started with both data interfaces, which is how an `app_all_veh` is
/// started: it finds descriptor 0 open, will not take interfaces it does not
/// support, and says so on the error interface before stopping. A program that
/// carried on would be writing the vehicle's data wherever that descriptor
/// goes.
#[test]
fn run_as_a_kind_it_is_not_it_refuses_and_says_why() {
    let mut running =
        parent::start(&app(), &arguments(), Supports::ALL).expect("it starts");

    let said = running
        .read_complaint()
        .expect("it complains")
        .expect("a message");
    assert!(said.contains("app_pl"), "{said}");
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
        parent::start(&app(), &["bay".to_string()], Supports::GATEWAY)
            .expect("it starts");

    let said = running
        .read_complaint()
        .expect("it complains")
        .expect("a message");
    assert!(said.contains("app_pl"), "{said}");
    assert!(said.contains("argument"), "{said}");

    assert!(!running.waited_for().expect("it stops").success());
}
