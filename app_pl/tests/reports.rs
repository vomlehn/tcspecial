//! It reports without being asked, and cannot be asked.
//!
//! The payload-side mirror of `app_moc_veh`'s tests: nothing goes in, readings
//! come out. Driven through `tcsapp::parent`, which is how tcspecial runs an
//! app.

use std::path::PathBuf;

use tcsapp::parent::{self, Supports};

/// The program this crate builds.
fn app() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_app_pl"))
}

/// Readings arrive with nothing having been asked for them.
///
/// Which is what this kind of payload is: it reports on a clock of its own, so
/// a second packet comes along after the first without this end doing anything
/// at all.
#[test]
fn readings_arrive_without_being_asked() {
    let mut running = parent::start(&app(), Supports::MOC).expect("it starts");

    assert_eq!(
        running.read_payload().expect("a reading"),
        Some(b"1".to_vec())
    );
    assert_eq!(
        running.read_payload().expect("another reading"),
        Some(b"2".to_vec()),
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
    let mut running = parent::start(&app(), Supports::MOC).expect("it starts");

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
    let mut running = parent::start(&app(), Supports::ALL).expect("it starts");

    let said = running
        .read_complaint()
        .expect("it complains")
        .expect("a message");
    assert!(said.contains("app_pl"), "{said}");
    assert!(said.contains("the payload interface"), "{said}");

    assert!(!running.waited_for().expect("it stops").success());
}
