//! The fprime example, flown: a vehicle that answers to an F´ flight computer
//! sends each reading to the bridge component at its `bridge_base`, and takes
//! the bridge's commands on to the heater.
//!
//! What is run is `examples/fprime/fprime.yaml` itself, with each of its
//! sockets moved to a port nothing holds -- so that this runs beside anything
//! else, the example included -- and its log moved to a directory of the
//! test's. The test is the bridge and both payloads.

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
/// tcspecial takes to start; what arrived is returned.
fn until_heard(from: &UdpSocket, to: SocketAddr, datum: &[u8], at: &UdpSocket) -> Vec<u8> {
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

#[test]
fn a_vehicle_answering_to_fprime_reports_to_the_bridge_and_takes_its_commands() {
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/fprime/fprime.yaml");
    let text = std::fs::read_to_string(&example).expect("the example reads");

    // Where the example's own bridge hears the thermometer, which its
    // comments and the manual say: bridge_base, 9700, plus the thermometer's
    // device id, 2.
    let shipped = telemetry::of(&Graph::load(&example).expect("the example reads"))
        .expect("its links to the bridge work out");
    let reading = shipped
        .iter()
        .find(|link| link.source == "thermometer")
        .expect("the thermometer reports");
    assert_eq!(reading.ground.port, 9702);

    let dir = tempfile::tempdir().expect("a directory");
    // tcspecial writes its log into a directory that is there, and does not
    // make one.
    std::fs::create_dir(dir.path().join("telemetry")).expect("the log's directory");

    // The heater is driven and never heard from, so tcspecial sends to it:
    // the test is the heater, bound where the configuration says it is.
    let heater = bound();
    let (thermometer, uplink, base) = (free(), free(), free());
    let mut text = text;
    for (was, now) in [
        ("\"9001\"", format!("\"{}\"", port(&heater))),
        ("\"9002\"", format!("\"{thermometer}\"")),
        ("\"9600\"", format!("\"{uplink}\"")),
        ("127.0.0.1:9700", format!("127.0.0.1:{base}")),
        ("127.0.0.1:9510", format!("127.0.0.1:{}", free())),
        ("0.0.0.0:9500", format!("127.0.0.1:{}", free())),
        ("0.0.0.0:9300", format!("127.0.0.1:{}", free())),
        ("127.0.0.1:9310", format!("127.0.0.1:{}", free())),
        ("127.0.0.1:9410", format!("127.0.0.1:{}", free())),
        ("0.0.0.0:9400", format!("127.0.0.1:{}", free())),
        ("0.0.0.0:9200", format!("127.0.0.1:{}", free())),
        ("127.0.0.1:9210", format!("127.0.0.1:{}", free())),
        (
            "/tmp/telemetry",
            dir.path().join("telemetry").display().to_string(),
        ),
    ] {
        text = moved(&text, was, &now);
    }
    let configuration = dir.path().join("fprime.yaml");
    std::fs::write(&configuration, &text).expect("written");

    // Where the bridge hears the thermometer, as the vehicle works it out:
    // the gateway node's bridge_base plus the thermometer's device id.
    let graph = Graph::load(&configuration).expect("it reads");
    let links = telemetry::of(&graph).expect("its links to the bridge work out");
    let link = links
        .iter()
        .find(|link| link.source == "thermometer")
        .expect("the thermometer reports");
    assert_eq!(link.ground.port, base + 2);
    let bridge = UdpSocket::bind(format!("{}:{}", link.ground.address, link.ground.port))
        .expect("the bridge's port");
    bridge
        .set_read_timeout(Some(Duration::from_millis(100)))
        .expect("a timeout");

    let _flying = Flying(
        Command::new(env!("CARGO_BIN_EXE_tcspecial"))
            .arg(&configuration)
            .arg("--quiet")
            .env_remove("RUST_LOG")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("tcspecial starts"),
    );

    // A reading: from the thermometer to the bridge.
    let sensor = bound();
    let to: SocketAddr = format!("127.0.0.1:{thermometer}")
        .parse()
        .expect("an address");
    assert_eq!(until_heard(&sensor, to, b"293.15", &bridge), b"293.15");

    // A command: from the bridge, up to the gateway node, and on to the
    // heater.
    let commander = bound();
    let up: SocketAddr = format!("127.0.0.1:{uplink}").parse().expect("an address");
    assert_eq!(until_heard(&commander, up, b"on", &heater), b"on");
}
