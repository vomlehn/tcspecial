//! The consolidator example, flown: three sensors' readings go down as one
//! datum, a structure of them, with each one's default until it has
//! reported, and its last reading, held, after.
//!
//! What is run is `examples/consolidator/consolidator.ytcs` itself, with
//! each of its sockets moved to a port nothing holds -- so that this runs
//! beside anything else, the example included -- and its log and state moved
//! to a directory of the test's. The test plays the three sensors, and is
//! the ground through tcsmoc's `GatewayReceiver`, as tcs-moc is.

use std::net::SocketAddr;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use tcsmoc::gateway::GatewayReceiver;
use tcsprotocol::consolidation::Layout;
use tcsprotocol::graph::Graph;

mod common;

use common::{bound, free, moved, port};

/// Receives for `wait`, keeping what arrives.
///
/// # Arguments
///
/// * `ground` - The gateway node, open on the ground.
/// * `wait`   - How long to listen.
///
/// # Returns
///
/// Each datum that arrived, with the node it is named by, in order.
fn received_for(
    ground: &mut GatewayReceiver,
    wait: Duration,
) -> Vec<(String, Vec<u8>)> {
    let deadline = Instant::now() + wait;
    let mut got = Vec::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match ground.receive(Some(left)).expect("it reads") {
            Some(received) => got.push((received.source, received.data)),
            None => break,
        }
    }
    got
}

/// Sends readings until a datum that is `wanted` comes down.
///
/// # Arguments
///
/// * `ground`   - The gateway node, open on the ground.
/// * `sensors`  - The socket the sensors send from.
/// * `readings` - Each sensor's address and reading, sent again each round.
/// * `wanted`   - The datum waited for.
fn until_sent(
    ground: &mut GatewayReceiver,
    sensors: &std::net::UdpSocket,
    readings: &[(SocketAddr, &[u8])],
    wanted: &[u8],
) {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut heard: Vec<(String, Vec<u8>)> = Vec::new();
    while !heard
        .iter()
        .any(|(source, data)| source == "housekeeping" && data == wanted)
    {
        assert!(Instant::now() < deadline, "only {heard:?} came down");
        for (at, reading) in readings {
            sensors.send_to(reading, at).expect("sent");
        }
        heard.extend(received_for(ground, Duration::from_millis(300)));
    }
}

#[test]
fn the_datum_carries_defaults_then_readings() {
    let example = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../examples/consolidator/consolidator.ytcs");
    let text = std::fs::read_to_string(&example).expect("the example reads");
    let dir = tempfile::tempdir().expect("a directory");
    // tcspecial writes its log into a directory that is there, and does not
    // make one.
    std::fs::create_dir(dir.path().join("telemetry"))
        .expect("the log's directory");

    let (sun, wheel, battery, base) = (free(), free(), free(), free());
    // The ground's command link: where tcs_cmd.send says it sends from,
    // and where tcs_cmd.recv says tcspecial takes them.
    let (commands, take_at) = (bound(), free());
    let mut text = text;
    for (was, now) in [
        (":5001\"", format!(":{sun}\"")),
        (":5002\"", format!(":{wheel}\"")),
        (":5003\"", format!(":{battery}\"")),
        ("127.0.0.1:5100", format!("127.0.0.1:{base}")),
        ("127.0.0.1:5650", format!("127.0.0.1:{}", free())),
        ("0.0.0.0:5550", format!("127.0.0.1:{}", free())),
        ("0.0.0.0:4200", format!("127.0.0.1:{}", free())),
        ("127.0.0.1:4300", format!("127.0.0.1:{}", free())),
        ("127.0.0.1:6100", format!("127.0.0.1:{}", free())),
        ("0.0.0.0:6000", format!("127.0.0.1:{}", free())),
        ("0.0.0.0:4000", format!("127.0.0.1:{take_at}")),
        ("127.0.0.1:4100", format!("127.0.0.1:{}", port(&commands))),
        (
            "/tmp/telemetry",
            dir.path().join("telemetry").display().to_string(),
        ),
        (
            "/tmp/tcspecial-consolidator.state",
            dir.path().join("state").display().to_string(),
        ),
    ] {
        text = moved(&text, was, &now);
    }
    let configuration = dir.path().join("consolidator.ytcs");
    std::fs::write(&configuration, &text).expect("written");

    // The ground, as the example's is: the gateway node's one port, for
    // housekeeping's datum, handed out whole.
    let graph = Graph::load(&configuration).expect("it reads");
    let mut ground =
        GatewayReceiver::open(&graph, "gateway").expect("the ground opens");
    assert_eq!(ground.sources(), ["housekeeping"]);

    let _flying = common::Flying::quiet(
        Command::new(env!("CARGO_BIN_EXE_tcspecial"))
            .arg(&configuration)
            .arg("--quiet")
            .arg("--simulate=all")
            .arg("--no-memlock")
            .env_remove("RUST_LOG")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("tcspecial starts"),
    );

    // Nothing comes down until the ground has matched the start, with a
    // CONNECT carrying the configuration's checksum.
    let take: SocketAddr =
        format!("127.0.0.1:{take_at}").parse().expect("an address");
    common::connect(&commands, take, 1, common::checksum_of(&configuration));

    // The three sensors, each sending its own type: a u16, an i32 and a u8,
    // in the datum in that order, the order of housekeeping's world-in.
    let sensors = bound();
    let at = |port: u16| -> SocketAddr {
        format!("127.0.0.1:{port}").parse().expect("an address")
    };

    // The sun alone, again until the interval sends it: the wheel and the
    // battery have never reported, and go down as their defaults, 0 and
    // 255.
    until_sent(
        &mut ground,
        &sensors,
        &[(at(sun), &[0x01, 0x02])],
        &[0x01, 0x02, 0, 0, 0, 0, 0xff],
    );

    // All three, until a datum with every one of them comes down.
    until_sent(
        &mut ground,
        &sensors,
        &[
            (at(sun), &[0x03, 0x04]),
            (at(wheel), &[0xff, 0xff, 0xff, 0xf9]),
            (at(battery), &[0x55]),
        ],
        &[0x03, 0x04, 0xff, 0xff, 0xff, 0xf9, 0x55],
    );

    // Quiet until whatever was in flight has come down and the interval
    // has found nothing new.
    while !received_for(&mut ground, Duration::from_secs(3)).is_empty() {}

    // The sun alone, new: housekeeping sends it at its interval, the wheel
    // and the battery held at their last readings, and the datum takes
    // apart by the layout the configuration gives.
    sensors.send_to(&[0x05, 0x06], at(sun)).expect("sent");
    let after = received_for(&mut ground, Duration::from_secs(4));
    let datum = [0x05, 0x06, 0xff, 0xff, 0xff, 0xf9, 0x55];
    assert_eq!(after, [("housekeeping".to_string(), datum.to_vec())]);
    let node = graph.node("housekeeping").expect("the consolidator");
    let (_, layout) = Layout::of_node(&graph, node)
        .expect("a consolidator")
        .expect("it lays out");
    assert_eq!(
        layout.unpack(&datum),
        Ok(vec![
            vec![0x05, 0x06],
            vec![0xff, 0xff, 0xff, 0xf9],
            vec![0x55]
        ])
    );
}
