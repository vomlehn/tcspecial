//! The ground end puts the line back together and stops at the end of it.
//!
//! This test is the vehicle: it sends the words the payload would have sent to
//! the address the ground station listens on, which is what a `moc` node does
//! with its `ground_address` and `ground_port`. What it reads is what the
//! ground station printed.

use std::io::{BufRead, BufReader, Read};
use std::net::UdpSocket;
use std::process::{Child, Command, Stdio};

/// An address on this host that nothing holds, for the ground to listen on.
fn a_free_address() -> String {
    let held = UdpSocket::bind("127.0.0.1:0").expect("a port of its own");
    held.local_addr().expect("its address").to_string()
}

/// The ground station, listening on `address`, once it says it is.
///
/// Waiting for it to say so is what makes the test sound: a word sent before
/// the ground has bound its socket is a word nobody will ever read.
fn ground(address: &str) -> Child {
    let mut ground = Command::new(env!("CARGO_BIN_EXE_lorem_moc"))
        .arg(address)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the ground station starts");
    let mut said = String::new();
    BufReader::new(ground.stderr.take().expect("its complaints"))
        .read_line(&mut said)
        .expect("what it said");
    assert!(said.contains("listening on"), "{said}");
    ground
}

/// What the ground station printed, all of it.
fn printed(ground: &mut Child) -> String {
    let mut said = String::new();
    ground
        .stdout
        .take()
        .expect("its output")
        .read_to_string(&mut said)
        .expect("what it printed");
    said
}

/// The words of a line arrive as packets and are written out as a line.
#[test]
fn the_words_are_written_out_as_one_line() {
    let address = a_free_address();
    let mut ground = ground(&address);
    let vehicle = UdpSocket::bind("127.0.0.1:0").expect("the vehicle's own socket");

    let first = simple_app::line(1).expect("a paragraph has a first line");
    for word in first.words() {
        vehicle
            .send_to(word.as_bytes(), &address)
            .expect("a word goes to the ground");
    }
    vehicle
        .send_to(simple_app::TERMINATOR.as_bytes(), &address)
        .expect("the end of the line goes to the ground");

    // One space between the words, none before the first, and the line ended:
    // the line as it was before the payload took it apart.
    assert_eq!(
        printed(&mut ground),
        format!(
            "{}{}",
            first.words().collect::<Vec<&str>>().join(" "),
            simple_app::TERMINATOR
        )
    );

    // And it stopped of its own accord when the line was whole.
    assert!(
        ground.wait().expect("it stops").success(),
        "a ground station that has the whole line has nothing left to wait for"
    );
}

/// A message that is only the terminator is printed, and ends it.
///
/// The rule on its own: whatever the ground station has or has not printed, a
/// line's end is a line's end.
#[test]
fn a_message_that_is_only_the_terminator_ends_it() {
    let address = a_free_address();
    let mut ground = ground(&address);
    let vehicle = UdpSocket::bind("127.0.0.1:0").expect("the vehicle's own socket");

    vehicle
        .send_to(simple_app::TERMINATOR.as_bytes(), &address)
        .expect("the end of a line nobody sent");

    assert_eq!(printed(&mut ground), simple_app::TERMINATOR);
    assert!(ground.wait().expect("it stops").success());
}

/// An address it cannot listen on is refused, saying which.
#[test]
fn an_address_it_cannot_listen_on_is_refused() {
    let held = UdpSocket::bind("127.0.0.1:0").expect("a port of its own");
    let address = held.local_addr().expect("its address").to_string();

    let output = Command::new(env!("CARGO_BIN_EXE_lorem_moc"))
        .arg(&address)
        .output()
        .expect("the ground station runs");
    assert!(!output.status.success());
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(said.contains(&address), "{said}");
}
