//! It answers what arrives, and cannot reach the interface it has not got.
//!
//! Driven through `tcsapp::parent`, which is how tcspecial runs an app: a test
//! that started the program its own way would be a test of an arrangement
//! nothing else uses.

use std::path::PathBuf;
use std::time::Duration;

use tcsapp::parent::{self, Supports};

/// The program this crate builds.
fn app() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_app_payload_veh"))
}

/// Payload data in, the same bytes out as MOC data.
#[test]
fn a_packet_of_payload_data_comes_back_as_moc_data() {
    let mut running = parent::start(&app(), Supports::PAYLOAD).expect("it starts");

    running.write_payload(b"293.15").expect("it takes a packet");
    assert_eq!(
        running.read_moc().expect("an answer"),
        Some(b"293.15".to_vec())
    );

    // And again, because an app that answered once and stopped reading would
    // pass a test that sent one packet.
    running.write_payload(b"294.05").expect("it takes another");
    assert_eq!(
        running.read_moc().expect("another answer"),
        Some(b"294.05".to_vec())
    );

    // Told that nothing more is coming, it finishes of its own accord rather
    // than being killed.
    running.no_more().expect("the interfaces end");
    assert!(
        running.waited_for().expect("it stops").success(),
        "an app whose interfaces ended has finished, not failed"
    );
}

/// A packet of no bytes is a packet, and comes back as one.
///
/// The case an interface of its own kind would lose: an app with nothing to
/// say and an app that has said something empty are different things, and this
/// is the one that proves the second survives the round trip.
#[test]
fn a_zero_length_packet_comes_back() {
    let mut running = parent::start(&app(), Supports::PAYLOAD).expect("it starts");

    running.write_payload(b"").expect("it takes an empty packet");
    assert_eq!(running.read_moc().expect("an answer"), Some(Vec::new()));

    running.no_more().expect("the interfaces end");
    assert!(running.waited_for().expect("it stops").success());
}

/// The interface this kind of app has not got cannot be reached.
///
/// Refused at this end by there being nothing to write on, and in the app by
/// there being no method to call and no descriptor open. Three answers to one
/// mistake, which is what the strictness of this arrangement is for.
#[test]
fn the_interface_it_has_not_got_cannot_be_reached() {
    let mut running = parent::start(&app(), Supports::PAYLOAD).expect("it starts");

    let said = running
        .write_moc(b"SET 300")
        .expect_err("an interface it has not got");
    assert!(said.contains("the MOC interface"), "{said}");
    assert!(said.contains("descriptor 1"), "{said}");

    let said = running
        .read_payload()
        .expect_err("an interface it has not got");
    assert!(said.contains("the MOC interface"), "{said}");

    running.no_more().expect("the interfaces end");
    assert!(running.waited_for().expect("it stops").success());
}

/// Run as a kind of app it is not, it refuses and says why.
///
/// Started with both data interfaces, which is how an `app_all_veh` is
/// started: it finds descriptor 1 open, will not take interfaces it does not
/// support, and says so on the error interface before stopping. A program that
/// carried on would be writing the vehicle's data wherever that descriptor
/// goes.
#[test]
fn run_as_a_kind_it_is_not_it_refuses_and_says_why() {
    let mut running = parent::start(&app(), Supports::ALL).expect("it starts");
    running.waits(Some(Duration::from_secs(5)));

    let said = running
        .read_complaint()
        .expect("it complains")
        .expect("a message");
    assert!(said.contains("app_payload_veh"), "{said}");
    assert!(said.contains("does not support"), "{said}");
    assert!(said.contains("the MOC interface"), "{said}");

    assert!(
        !running.waited_for().expect("it stops").success(),
        "an app that would not start has not succeeded"
    );
}
