//! An app's standard streams are not its interfaces.
//!
//! Driven through `tcsapp::parent`, as tcspecial runs an app, with
//! `stray_app`, which prints, reads stdin and panics.

use std::path::PathBuf;
use std::time::Duration;

use tcsapp::parent::{self, Supports};

/// Starts `stray_app`, as the `app_payload_veh` it is.
///
/// # Returns
///
/// The running app.
fn started() -> parent::Running {
    let app = PathBuf::from(env!("CARGO_BIN_EXE_stray_app"));
    let arguments = vec!["stray".to_string(), "--readings".to_string()];
    let mut running =
        parent::start(&app, &arguments, Supports::PAYLOAD).expect("it starts");
    running.waits(Some(Duration::from_secs(5)));
    running
}

/// What it prints reaches neither interface, and stdin is not the payload
/// interface: it reads as ended, and the packets on the interface arrive
/// whole.
#[test]
fn printing_and_reading_stdin_leave_the_interfaces_alone() {
    let mut running = started();
    assert_eq!(
        running.read_gateway().expect("its report"),
        Some(b"stdin gave Some(0)".to_vec())
    );
    running.write_payload(b"293.15").expect("it takes a packet");
    assert_eq!(
        running.read_gateway().expect("an answer"),
        Some(b"293.15".to_vec())
    );
    running.no_more().expect("the interfaces end");
    assert!(running.waited_for().expect("it stops").success());
    assert_eq!(running.read_complaint().expect("no complaint"), None);
}

/// A panic is said on the error interface, as one packet with the node's
/// name.
#[test]
fn a_panic_is_said_as_a_complaint() {
    let mut running = started();
    running.read_gateway().expect("its report");
    running.write_payload(b"panic").expect("it takes a packet");
    let said = running
        .read_complaint()
        .expect("it complains")
        .expect("a message");
    assert!(said.starts_with("stray: "), "{said}");
    assert!(said.contains("asked to panic"), "{said}");
    assert!(!running.waited_for().expect("it stops").success());
}
