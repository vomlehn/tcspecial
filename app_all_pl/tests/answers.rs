//! It answers what asks it, and cannot reach the interface it has not got.
//!
//! What a payload does, as a program: asked for a reading, it writes one.
//! Driven through `tcsapp::parent`, which is how tcspecial runs an app.

use std::path::PathBuf;

use tcsapp::parent::{self, Supports};

/// The program this crate builds.
fn app() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_app_all_pl"))
}

/// The arguments an `app_all_pl` is run with: its name, then the payload
/// argument, then the MOC one, this kind handling both streams.
///
/// # Returns
///
/// The three arguments, in order.
fn arguments() -> Vec<String> {
    vec![
        "bay".to_string(),
        "--bay-2".to_string(),
        "--bay-2-commands".to_string(),
    ]
}

/// What asks for a reading comes back as a reading.
#[test]
fn what_asks_for_a_reading_is_answered_with_one() {
    let mut running =
        parent::start(&app(), &arguments(), Supports::MOC).expect("it starts");

    running.write_moc(b"READ").expect("it takes a packet");
    assert_eq!(
        running.read_payload().expect("an answer"),
        Some(b"READ".to_vec())
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
    assert!(said.contains("app_all_pl"), "{said}");
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
        parent::start(&app(), &["bay".to_string()], Supports::MOC)
            .expect("it starts");

    let said = running
        .read_complaint()
        .expect("it complains")
        .expect("a message");
    assert!(said.contains("app_all_pl"), "{said}");
    assert!(said.contains("argument"), "{said}");

    assert!(!running.waited_for().expect("it stops").success());
}
