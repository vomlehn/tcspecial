//! The ground end puts the line back together and stops at the end of it.
//!
//! This test is the vehicle: it sends the words the payload would have sent to
//! the port a gateway node's data arrives at -- its `ground_base` plus the
//! sending node's device id -- which is what tcspecial does. The ground
//! station gets them from the gateway node with `tcslib::gateway`, so it is
//! given a configuration of its own, whose gateway node is somewhere free. What
//! this reads is what the ground station printed.

use std::io::{BufRead, BufReader, Read};
use std::net::UdpSocket;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// A configuration whose gateway node takes `lorem`'s words on a port nothing
/// holds, and that port: `lorem` is the second node read, device id 1, so it
/// is the base plus one.
///
/// # Returns
///
/// The directory the configuration is in, which removes it when dropped; the
/// configuration's path; and the port `lorem`'s words arrive at.
///
/// # Panics
///
/// No port can be had, or the file cannot be written.
fn configuration() -> (tempfile::TempDir, PathBuf, u16) {
    let port = UdpSocket::bind("127.0.0.1:0")
        .expect("a port of its own")
        .local_addr()
        .expect("its address")
        .port();
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("simple-app.yaml");
    std::fs::write(
        &path,
        format!(
            "gateway:
  - {{ name: gateway, type: gateway_radio, ground_base: \"127.0.0.1:{}\" }}
payloads:
  - {{ name: lorem, type: network, protocol: udp, address: \"127.0.0.1\", \
port: \"7001\", payload-out: gateway }}
",
            port - 1
        ),
    )
    .expect("written");
    (dir, path, port)
}

/// The ground station, getting its data from that configuration's gateway node,
/// once it says it is.
///
/// Waiting for it to say so is what makes the test sound: a word sent before
/// the ground has its port is a word nobody will ever read.
///
/// # Arguments
///
/// * `configuration` - The configuration file to give it.
///
/// # Returns
///
/// The ground station's process, its standard output still to be read.
///
/// # Panics
///
/// It cannot be started, or what it first says is not that it is listening.
fn ground(configuration: &PathBuf) -> Child {
    let mut ground = Command::new(env!("CARGO_BIN_EXE_lorem_gateway"))
        .arg(configuration)
        .arg("gateway")
        // To be told when it is listening, which it says only when verbose.
        .arg("--verbose")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the ground station starts");
    let mut said = String::new();
    BufReader::new(ground.stderr.take().expect("its complaints"))
        .read_line(&mut said)
        .expect("what it said");
    assert!(said.contains("listening on gateway for lorem"), "{said}");
    ground
}

/// What the ground station printed, all of it.
///
/// # Arguments
///
/// * `ground` - The ground station's process, whose standard output is taken
///              and read to its end.
///
/// # Returns
///
/// Everything it printed on standard output.
///
/// # Panics
///
/// Its output was taken already, or is not UTF-8.
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

/// The words of a line arrive as datagrams and are written out as a line.
#[test]
fn the_words_are_written_out_as_one_line() {
    let (_dir, path, port) = configuration();
    let mut ground = ground(&path);
    let vehicle =
        UdpSocket::bind("127.0.0.1:0").expect("the vehicle's own socket");

    let first = simple_app::line(1).expect("a paragraph has a first line");
    for word in first.words() {
        vehicle
            .send_to(word.as_bytes(), ("127.0.0.1", port))
            .expect("a word goes to the ground");
    }
    vehicle
        .send_to(simple_app::TERMINATOR.as_bytes(), ("127.0.0.1", port))
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

/// A datum that is only the terminator is printed, and ends it.
///
/// The rule on its own: whatever the ground station has or has not printed, a
/// line's end is a line's end.
#[test]
fn a_datum_that_is_only_the_terminator_ends_it() {
    let (_dir, path, port) = configuration();
    let mut ground = ground(&path);
    let vehicle =
        UdpSocket::bind("127.0.0.1:0").expect("the vehicle's own socket");

    vehicle
        .send_to(simple_app::TERMINATOR.as_bytes(), ("127.0.0.1", port))
        .expect("the end of a line nobody sent");

    assert_eq!(printed(&mut ground), simple_app::TERMINATOR);
    assert!(ground.wait().expect("it stops").success());
}

/// A gateway node it cannot open is refused, saying why.
#[test]
fn a_gateway_node_it_cannot_open_is_refused() {
    let (_dir, path, _port) = configuration();
    let output = Command::new(env!("CARGO_BIN_EXE_lorem_gateway"))
        .arg(&path)
        .arg("lorem")
        .output()
        .expect("the ground station runs");
    assert!(!output.status.success());
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(said.contains("not a gateway node"), "{said}");
}
