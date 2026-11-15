//! The consolidator example, flown: three sensors' readings go down as one
//! datum, and the ground's receive hands each new one out as its sensor's,
//! and a held one not at all.
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

#[test]
fn each_new_reading_reaches_the_ground_as_its_sensors() {
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
    // housekeeping's datum, and every sensor among what it hands out.
    let graph = Graph::load(&configuration).expect("it reads");
    let mut ground =
        GatewayReceiver::open(&graph, "gateway").expect("the ground opens");
    let mut sources = ground.sources();
    sources.sort();
    assert_eq!(sources, ["battery", "housekeeping", "sun", "wheel"]);

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
    // again until every one has come down, which is how long tcspecial
    // takes to start.
    let sensors = bound();
    let at = |port: u16| -> SocketAddr {
        format!("127.0.0.1:{port}").parse().expect("an address")
    };
    let readings: [(&str, u16, &[u8]); 3] = [
        ("sun", sun, &[0x01, 0x02]),
        ("wheel", wheel, &[0xff, 0xff, 0xff, 0xf9]),
        ("battery", battery, &[0x55]),
    ];
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut heard: Vec<(String, Vec<u8>)> = Vec::new();
    while !readings.iter().all(|(name, _, reading)| {
        heard
            .iter()
            .any(|(source, data)| source == name && data == reading)
    }) {
        assert!(Instant::now() < deadline, "only {heard:?} came down");
        for (_, port, reading) in &readings {
            sensors.send_to(reading, at(*port)).expect("sent");
        }
        heard.extend(received_for(&mut ground, Duration::from_millis(300)));
    }
    // The consolidator's datum itself is never handed out: only its inputs.
    assert!(
        heard.iter().all(|(source, _)| source != "housekeeping"),
        "{heard:?}"
    );

    // Quiet until whatever was in flight has come down and the interval
    // has found nothing new.
    while !received_for(&mut ground, Duration::from_secs(3)).is_empty() {}

    // The sun alone, new: housekeeping sends it at its interval, the wheel
    // and the battery held, and only the sun's reading comes down.
    sensors.send_to(&[0x03, 0x04], at(sun)).expect("sent");
    let after = received_for(&mut ground, Duration::from_secs(4));
    assert_eq!(after, [("sun".to_string(), vec![0x03, 0x04])]);
}
