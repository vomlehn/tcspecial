//! The cfs example, commanded from the ground: tcslib-moc's peer of the bus,
//! through the simulated cFS end that tcssim runs, against the vehicle itself.
//!
//! What `tests/cfs.rs` checks byte by byte, as the cFS end, this checks as a
//! ground station sees it: through `tcslib_moc::bus`, with the station's own
//! beacon watch, command client and gateway receiver, none of which is told
//! that the vehicle is on a bus rather than a radio.

use std::net::{SocketAddr, UdpSocket};
use std::path::Path;
use std::process::{Child, Command as Process, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tcslib_moc::beacon::{self, Arrived, Schedule};
use tcslib_moc::bus::SimulatedCfs;
use tcslib_moc::gateway::GatewayReceiver;
use tcslib_moc::matched::Matched;
use tcslib_moc::{command_links_of, CommandStatus, NodeId, TcsClient};
use tcslibgs::graph::Graph;

mod common;

/// Finds a port nothing holds, for something to bind.
///
/// # Returns
///
/// The port, free when it was looked at.
fn free() -> u16 {
    UdpSocket::bind("127.0.0.1:0")
        .expect("a port")
        .local_addr()
        .expect("its address")
        .port()
}

/// tcspecial, stopped however the test ends.
struct Flying(Child);

impl Drop for Flying {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The example, moved to ports of the test's and written into `dir`.
///
/// # Arguments
///
/// * `dir` - Where it is written, with its log and state file.
///
/// # Returns
///
/// The configuration's path, and the thermometer's port.
fn example(dir: &Path) -> (std::path::PathBuf, u16) {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/cfs/cfs.ytcs");
    let mut text = std::fs::read_to_string(&path).expect("the example reads");
    std::fs::create_dir(dir.join("telemetry")).expect("the log's directory");
    let thermometer = free();
    for (was, now) in [
        ("0.0.0.0:8634", format!("127.0.0.1:{}", free())),
        ("127.0.0.1:2234", format!("127.0.0.1:{}", free())),
        ("\"8601\"", format!("\"{}\"", free())),
        ("\"8602\"", format!("\"{thermometer}\"")),
        (
            "/tmp/telemetry",
            dir.join("telemetry").display().to_string(),
        ),
        (
            "/tmp/tcspecial-cfs.state",
            dir.join("state").display().to_string(),
        ),
    ] {
        assert_eq!(text.matches(was).count(), 1, "the example says {was}");
        text = text.replace(was, &now);
    }
    let configuration = dir.join("cfs.ytcs");
    std::fs::write(&configuration, &text).expect("written");
    (configuration, thermometer)
}

/// A vehicle that answers to cFS is commanded, heard and read from the
/// ground through tcslib-moc, as one that answers to the radio is.
#[test]
fn a_ground_station_commands_a_vehicle_on_a_cfs_bus() {
    let dir = tempfile::tempdir().expect("a directory");
    let (configuration, thermometer) = example(dir.path());
    let graph = Graph::load(&configuration).expect("it reads");
    let digest = common::checksum_of(&configuration);

    let _cfs = SimulatedCfs::open(&graph)
        .expect("the cFS end is simulated")
        .expect("the example answers over a bus");
    let _flying = Flying(
        Process::new(env!("CARGO_BIN_EXE_tcspecial"))
            .arg(&configuration)
            .arg("--quiet")
            .arg("--simulate=all")
            .env_remove("RUST_LOG")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("tcspecial starts"),
    );

    // The beacon watch, on a thread of its own as a station runs it,
    // keeping the match its commands wait for.
    let matched = Matched::for_configuration(digest);
    let mut beacons = beacon::open(&graph, Schedule::standard(), Some(digest))
        .expect("beacons can be received")
        .expect("the example sends beacons");
    let heard = Arc::new(AtomicBool::new(false));
    {
        let matched = matched.clone();
        let heard = heard.clone();
        std::thread::spawn(move || loop {
            let pass = beacons.pass();
            matched.heard(pass);
            if pass.arrived == Arrived::Beacon {
                heard.store(true, Ordering::SeqCst);
            }
        });
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    while !heard.load(Ordering::SeqCst) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        heard.load(Ordering::SeqCst),
        "a beacon arrives over the bus"
    );

    // CONNECT, and then a node command, which only a matched station sends.
    let links = command_links_of(&graph)
        .expect("the links read")
        .expect("the example states them");
    let mut client =
        TcsClient::open(&links, matched.clone()).expect("the client opens");
    client.set_timeout(Duration::from_millis(500));
    let mut answered = None;
    for _ in 0..20 {
        if let Ok(read) = client.connect(digest) {
            answered = Some(read);
            break;
        }
    }
    let (_, theirs) = answered.expect("the CONNECT is answered over the bus");
    assert_eq!(theirs, digest, "the vehicle flies this configuration");
    let deadline = Instant::now() + Duration::from_secs(20);
    while !matched.for_commanding() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    let (status, _, _) =
        client.query_node(NodeId(1)).expect("the node is asked");
    assert_eq!(status, CommandStatus::Success);

    // A reading, through the gateway node it is drawn into.
    let mut fsw = GatewayReceiver::open(&graph, "fsw").expect("fsw opens");
    let sensor = UdpSocket::bind("127.0.0.1:0").expect("the sensor");
    let to: SocketAddr = format!("127.0.0.1:{thermometer}").parse().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut reading = None;
    while reading.is_none() && Instant::now() < deadline {
        sensor.send_to(b"293.15", to).expect("sent");
        reading = fsw
            .receive(Some(Duration::from_millis(300)))
            .expect("it reads");
    }
    let reading = reading.expect("the reading arrives over the bus");
    assert_eq!(reading.source, "thermometer");
    assert_eq!(reading.data, b"293.15".to_vec());
}
