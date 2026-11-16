//! The consolidator example, flown: three sensors' readings and the
//! battery's age go down as one datum, a structure of them, with each one's
//! default until it has reported, and its last reading, held, after.
//!
//! What is run is `examples/consolidator/consolidator.ytcs` itself, with
//! each of its sockets moved to a port nothing holds -- so that this runs
//! beside anything else, the example included -- and its log and state moved
//! to a directory of the test's. The test plays the three sensors, and is
//! the ground through tcsmoc's `GatewayReceiver`, as tcs-moc is; the
//! battery's age is the example's own program, `battery_age.py`, which needs
//! Python 3.

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

/// Sends readings until a datum that `wanted` accepts comes down.
///
/// # Arguments
///
/// * `ground`   - The gateway node, open on the ground.
/// * `sensors`  - The socket the sensors send from.
/// * `readings` - Each sensor's address and reading, sent again each round.
/// * `wanted`   - Whether a datum is the one waited for.
///
/// # Returns
///
/// The datum.
fn until_sent(
    ground: &mut GatewayReceiver,
    sensors: &std::net::UdpSocket,
    readings: &[(SocketAddr, &[u8])],
    wanted: impl Fn(&[u8]) -> bool,
) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut heard: Vec<(String, Vec<u8>)> = Vec::new();
    loop {
        if let Some((_, datum)) = heard
            .iter()
            .find(|(source, data)| source == "housekeeping" && wanted(data))
        {
            return datum.clone();
        }
        assert!(Instant::now() < deadline, "only {heard:?} came down");
        for (at, reading) in readings {
            sensors.send_to(reading, at).expect("sent");
        }
        heard.extend(received_for(ground, Duration::from_millis(300)));
    }
}

/// The battery's age in a datum: its last two bytes.
///
/// # Arguments
///
/// * `datum` - The datum.
///
/// # Returns
///
/// The age, in seconds.
fn age_of(datum: &[u8]) -> u16 {
    u16::from_be_bytes([datum[7], datum[8]])
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
        (
            "\"examples/consolidator/battery_age.py\"",
            format!(
                "\"{}\"",
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../examples/consolidator/battery_age.py")
                    .display()
            ),
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
    // in the datum in that order, the order of housekeeping's world-in, and
    // then the battery's age, a u16.
    let sensors = bound();
    let at = |port: u16| -> SocketAddr {
        format!("127.0.0.1:{port}").parse().expect("an address")
    };

    // The sun alone, again until the interval sends it: the wheel and the
    // battery have never reported, and go down as their defaults, 0 and
    // 255, and the battery's age as never, 65535.
    until_sent(&mut ground, &sensors, &[(at(sun), &[0x01, 0x02])], |d| {
        d == [0x01, 0x02, 0, 0, 0, 0, 0xff, 0xff, 0xff]
    });

    // All three, until a datum with every one of them comes down and the
    // battery's age a second or two: the age is sent once a second, so the
    // first datum after the battery's reading may still say never.
    until_sent(
        &mut ground,
        &sensors,
        &[
            (at(sun), &[0x03, 0x04]),
            (at(wheel), &[0xff, 0xff, 0xff, 0xf9]),
            (at(battery), &[0x55]),
        ],
        |d| {
            d.starts_with(&[0x03, 0x04, 0xff, 0xff, 0xff, 0xf9, 0x55])
                && age_of(d) <= 2
        },
    );

    // The sun alone, new, the battery quiet: the wheel and the battery go
    // down held at their last readings, and the battery's age grows.
    let quiet =
        until_sent(&mut ground, &sensors, &[(at(sun), &[0x05, 0x06])], |d| {
            d.starts_with(&[0x05, 0x06, 0xff, 0xff, 0xff, 0xf9, 0x55])
                && age_of(d) >= 3
        });
    assert!(age_of(&quiet) < 0xffff, "{quiet:?}");

    // And the datum takes apart by the layout the configuration gives.
    let datum = quiet;
    let node = graph.node("housekeeping").expect("the consolidator");
    let (_, layout) = Layout::of_node(&graph, node)
        .expect("a consolidator")
        .expect("it lays out");
    assert_eq!(
        layout.unpack(&datum),
        Ok(vec![
            vec![0x05, 0x06],
            vec![0xff, 0xff, 0xff, 0xf9],
            vec![0x55],
            datum[7..].to_vec(),
        ])
    );
}
