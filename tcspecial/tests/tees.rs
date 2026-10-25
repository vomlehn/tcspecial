//! The tees example, flown: each command the ground sends reaches the heater
//! and comes back to the ground, and each reading reaches the ground.
//!
//! What is run is `examples/tees/tees.yaml` itself, with each of its sockets
//! moved to a port nothing holds -- so that this runs beside anything else,
//! the example included -- and its log moved to a directory of the test's.
//! Everything else is the example's, so a change to it that stops a tee
//! sending both ways fails here.

use std::net::{SocketAddr, UdpSocket};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tcslibgs::graph::telemetry;
use tcslibgs::graph::Graph;

/// A socket on a port nothing holds.
fn bound() -> UdpSocket {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("a port of its own");
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .expect("a timeout");
    socket
}

fn port(socket: &UdpSocket) -> u16 {
    socket.local_addr().expect("its address").port()
}

/// A port nothing holds, for tcspecial to bind.
fn free() -> u16 {
    port(&bound())
}

/// tcspecial, stopped however the test ends: a vehicle left running would
/// hold its ports for the next test to trip over.
struct Flying(Child);

impl Drop for Flying {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// `text` with `was` written as `now`, where it appears exactly once: a
/// socket the example stops naming is a test that no longer moves it, and
/// would bind the example's own port.
fn moved(text: &str, was: &str, now: &str) -> String {
    assert_eq!(
        text.matches(was).count(),
        1,
        "the example should say {was} once"
    );
    text.replace(was, now)
}

/// Send `datum` to `to` until something arrives at `at`, which is how long
/// tcspecial takes to start; what arrived, and nothing but it, is returned.
fn until_heard(
    from: &UdpSocket,
    to: SocketAddr,
    datum: &[u8],
    at: &UdpSocket,
) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut buffer = [0u8; 256];
    while Instant::now() < deadline {
        from.send_to(datum, to).expect("sent");
        if let Ok(got) = at.recv(&mut buffer) {
            return buffer[..got].to_vec();
        }
    }
    panic!("nothing arrived at {:?}", at.local_addr());
}

/// What arrives at `at` within a few seconds, or `None`.
fn heard(at: &UdpSocket) -> Option<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut buffer = [0u8; 256];
    while Instant::now() < deadline {
        if let Ok(got) = at.recv(&mut buffer) {
            return Some(buffer[..got].to_vec());
        }
    }
    None
}

#[test]
fn a_tee_sends_each_datum_both_ways() {
    let example = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../examples/tees/tees.yaml");
    let text = std::fs::read_to_string(&example).expect("the example reads");

    // Where the example's own ground hears each tee, which its diagram and
    // the manual say: ground_base, 8700, plus each tee's device id.
    let shipped =
        telemetry::of(&Graph::load(&example).expect("the example reads"))
            .expect("its links to the ground work out");
    for (source, at) in [("echo", 8704), ("readings", 8705)] {
        let link = shipped
            .iter()
            .find(|link| link.source == source)
            .unwrap_or_else(|| panic!("{source} sends to the ground"));
        assert_eq!(link.ground.port, at, "{source}");
    }
    let dir = tempfile::tempdir().expect("a directory");
    // tcspecial writes its log into a directory that is there, and does not
    // make one.
    std::fs::create_dir(dir.path().join("telemetry"))
        .expect("the log's directory");

    // The heater is driven and never heard from, so tcspecial sends to it:
    // the test is the heater, bound where the configuration says it is.
    let heater = bound();
    let (sensor, uplink, base) = (free(), free(), free());
    let mut text = text;
    for (was, now) in [
        ("\"8001\"", format!("\"{}\"", port(&heater))),
        ("\"8002\"", format!("\"{sensor}\"")),
        ("\"8003\"", format!("\"{}\"", free())),
        ("\"8600\"", format!("\"{uplink}\"")),
        ("127.0.0.1:8700", format!("127.0.0.1:{base}")),
        ("127.0.0.1:8510", format!("127.0.0.1:{}", free())),
        ("0.0.0.0:8500", format!("127.0.0.1:{}", free())),
        ("0.0.0.0:8300", format!("127.0.0.1:{}", free())),
        ("127.0.0.1:8310", format!("127.0.0.1:{}", free())),
        ("127.0.0.1:8410", format!("127.0.0.1:{}", free())),
        ("0.0.0.0:8400", format!("127.0.0.1:{}", free())),
        ("0.0.0.0:8200", format!("127.0.0.1:{}", free())),
        ("127.0.0.1:8210", format!("127.0.0.1:{}", free())),
        (
            "/tmp/telemetry",
            dir.path().join("telemetry").display().to_string(),
        ),
    ] {
        text = moved(&text, was, &now);
    }
    let configuration = dir.path().join("tees.yaml");
    std::fs::write(&configuration, &text).expect("written");

    // Where the ground hears each tee, as the vehicle works it out: the moc
    // node's ground_base plus the tee's device id.
    let graph = Graph::load(&configuration).expect("it reads");
    let links =
        telemetry::of(&graph).expect("its links to the ground work out");
    let ground_of = |source: &str| {
        let link = links
            .iter()
            .find(|link| link.source == source)
            .unwrap_or_else(|| panic!("{source} sends to the ground"));
        let socket = UdpSocket::bind(format!(
            "{}:{}",
            link.ground.address, link.ground.port
        ))
        .expect("the ground's port");
        socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .expect("a timeout");
        socket
    };
    let echoed = ground_of("echo");
    let read = ground_of("readings");

    let _flying = Flying(
        Command::new(env!("CARGO_BIN_EXE_tcspecial"))
            .arg(&configuration)
            .arg("--quiet")
            .arg("--simulate=all")
            .env_remove("RUST_LOG")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("tcspecial starts"),
    );

    // A command: to the heater, as it was going, and back to the ground.
    let ground = bound();
    let up: SocketAddr =
        format!("127.0.0.1:{uplink}").parse().expect("an address");
    assert_eq!(until_heard(&ground, up, b"on", &heater), b"on");
    assert_eq!(
        heard(&echoed).as_deref(),
        Some(&b"on"[..]),
        "the command should come back to the ground as payload data"
    );

    // A reading: to the ground, as it was going. Its copy vehicleward goes
    // to a node that drops it, which is nothing anybody can hear.
    let thermometer = bound();
    let to: SocketAddr =
        format!("127.0.0.1:{sensor}").parse().expect("an address");
    assert_eq!(until_heard(&thermometer, to, b"293.15", &read), b"293.15");
}
