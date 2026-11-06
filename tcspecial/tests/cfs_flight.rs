//! The cfs example, flown against cFS's own Linux build with SBN: the test
//! that the wire format tcspecial speaks is SBN's and not only ours.
//!
//! `tests/cfs.rs` plays the cFS end with a socket speaking the format as
//! `tcslibgs::sbn` states it, so it can only show that the vehicle keeps to
//! that statement. Here the far end is cFS itself, built from a cFS checkout
//! with SBN v7.0.1 and its UDP module, and this test is its ground: it
//! commands cFS through CI_LAB, which publishes what arrives on UDP on the
//! software bus, and reads what TO_LAB sends down from it. Everything
//! tcspecial takes or sends therefore crosses SBN in both directions.
//!
//! Ignored by `cargo test`, because it needs that build running: `make
//! cfs-test` builds cFS, starts it and tcspecial, and runs this with
//! `--ignored`, saying where each is through the environment. See
//! `cfs/cfs-test`.

use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tcslibgs::config_digest::digest_of_graph;
use tcslibgs::graph::Graph;
use tcslibgs::{
    cfs, Beacon, Command, CommandStatus, ConnectCommand, NodeCommand, NodeId,
    Telemetry,
};

/// The example's spacecraft.
const SPACECRAFT: u32 = 0x42;

/// The example's message IDs.
const SPACECRAFT_COMMANDS: u16 = 0x4280;
const SPACECRAFT_ANSWERS: u16 = 0x4200;
const BEACON: u16 = 0x4201;
const FSW: u16 = 0x4281;
const THERMOMETER: u16 = 0x4242;
const NODE_COMMANDS: u16 = 0x4290;
const NODE_ANSWERS: u16 = 0x4210;

/// Where the example's payloads are: the heater, which tcspecial sends to
/// and the test listens as, and the thermometer, which tcspecial listens on
/// and the test sends readings to.
const HEATER: &str = "127.0.0.1:8601";
const THERMOMETER_AT: &str = "127.0.0.1:8602";

/// CI_LAB's uplink, processor 1's port, and where TO_LAB sends: their
/// defaults (`CI_LAB_BASE_UDP_PORT`, `TO_LAB_TLM_PORT`).
const CI_LAB: &str = "127.0.0.1:1234";
const TO_LAB: &str = "127.0.0.1:1235";

/// TO_LAB's command message ID, as cfs/defs maps its topic, 0x80, under
/// the version 2 mapping: see cfs/defs/sample_defs/cfe_core_api_base_msgids.h.
const TO_LAB_COMMANDS: u16 = 0x1980;

/// TO_LAB's function code that starts it sending to an address
/// (`TO_LAB_OUTPUT_ENABLE_CC`), and the room that address has
/// (`TO_LAB_EnableOutput_Payload_t`).
const TO_LAB_OUTPUT_ENABLE: u8 = 6;
const TO_LAB_ADDRESS_BYTES: usize = 16;

/// Reads a path the harness gives in the environment.
///
/// # Arguments
///
/// * `name` - The variable.
///
/// # Returns
///
/// The path.
///
/// # Panics
///
/// Where the variable is not set, which is this being run by hand without
/// the harness.
fn given(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_else(|| {
        panic!("{name} is not set: run this through make cfs-test")
    }))
}

/// Waits until a log says something.
///
/// # Arguments
///
/// * `log`    - The log.
/// * `said`   - What it is to say.
/// * `within` - How long to wait before the test fails.
///
/// # Returns
///
/// The line that says it.
fn logged(log: &Path, said: &str, within: Duration) -> String {
    let deadline = Instant::now() + within;
    loop {
        let text = std::fs::read_to_string(log).unwrap_or_default();
        if let Some(line) = text.lines().find(|line| line.contains(said)) {
            return line.to_string();
        }
        assert!(
            Instant::now() < deadline,
            "{} never said \"{said}\"",
            log.display()
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// The ground of the cFS: CI_LAB's uplink, and TO_LAB's downlink.
struct Ground {
    socket: UdpSocket,
    sequence: u16,
}

impl Ground {
    /// Binds where TO_LAB sends.
    ///
    /// # Returns
    ///
    /// The ground, whose reads give up after a fifth of a second.
    fn new() -> Ground {
        let socket = UdpSocket::bind(TO_LAB).expect("TO_LAB's port");
        socket
            .set_read_timeout(Some(Duration::from_millis(200)))
            .expect("a timeout");
        Ground {
            socket,
            sequence: 0,
        }
    }

    /// Publishes a command on cFS's bus, through CI_LAB.
    ///
    /// # Arguments
    ///
    /// * `msgid` - Its message ID.
    /// * `code`  - Its function code.
    /// * `data`  - What it carries.
    fn command(&mut self, msgid: u16, code: u8, data: &[u8]) {
        let mut room = [0u8; 1024];
        let size = cfs::write_command(
            &mut room,
            msgid,
            self.sequence,
            SPACECRAFT,
            code,
            data,
        )
        .expect("it fits");
        self.sequence = self.sequence.wrapping_add(1);
        self.socket.send_to(&room[..size], CI_LAB).expect("sent");
    }

    /// Publishes a tcspecial command under `msgid`, as its function code
    /// says.
    ///
    /// # Arguments
    ///
    /// * `msgid`   - The message ID it is published under.
    /// * `command` - The command.
    fn send(&mut self, msgid: u16, command: &Command) {
        let code = command.cmd_type().to_u8();
        self.command(msgid, code, &command.to_bytes());
    }

    /// Waits for a telemetry packet under `msgid` from TO_LAB, passing over
    /// the rest -- cFS's own housekeeping and events among them.
    ///
    /// # Arguments
    ///
    /// * `msgid`  - The message ID waited for.
    /// * `within` - How long to wait.
    ///
    /// # Returns
    ///
    /// What it carries after its headers, or `None` where none came.
    fn published(&self, msgid: u16, within: Duration) -> Option<Vec<u8>> {
        let deadline = Instant::now() + within;
        let mut room = [0u8; 65536];
        while Instant::now() < deadline {
            let Ok(size) = self.socket.recv(&mut room) else {
                continue;
            };
            let Ok(frame) = cfs::read_telemetry(&room[..size]) else {
                continue;
            };
            if frame.msgid == msgid {
                assert_ne!(frame.time, [0; 6], "stamped with cFE time");
                return Some(frame.data.to_vec());
            }
        }
        None
    }

    /// Starts TO_LAB sending to this ground, again until something it
    /// sends arrives: CI_LAB may not yet be reading its uplink.
    fn downlink(&mut self) {
        let mut address = [0u8; TO_LAB_ADDRESS_BYTES];
        address[..9].copy_from_slice(b"127.0.0.1");
        for _ in 0..30 {
            self.command(TO_LAB_COMMANDS, TO_LAB_OUTPUT_ENABLE, &address);
            let mut room = [0u8; 65536];
            if self.socket.recv(&mut room).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        panic!("TO_LAB sent nothing after being told where to");
    }
}

/// tcspecial joins cFS's own software bus as an SBN peer, and the beacon, a
/// CONNECT and its answer, a reading, a command for fsw and a node command all
/// cross SBN between the two, as `tests/cfs.rs` has them cross the test's
/// simulated cFS end.
#[test]
#[ignore = "needs cFS's Linux build running: make cfs-test"]
fn a_vehicle_answering_to_cfs_is_a_peer_of_cfs_itself() {
    let configuration = given("TCS_CFS_CONFIGURATION");
    let cfs_log = given("TCS_CFS_LOG");
    let tcspecial_log = given("TCS_TCSPECIAL_LOG");
    let heater = UdpSocket::bind(HEATER).expect("the heater's port");
    heater
        .set_read_timeout(Some(Duration::from_millis(200)))
        .expect("a timeout");
    let mut ground = Ground::new();

    // SBN on cFS took tcspecial for the peer its table lists, and
    // tcspecial took cFS for SBN v7.0.1, protocol version 11, built with
    // the ident the configuration states: a peer refused for either is
    // disconnected, which would never get as far as SBN's "connected".
    let up = Duration::from_secs(30);
    println!("{}", logged(&cfs_log, "Peer 66:2 connected", up));
    println!("{}", logged(&tcspecial_log, "is connected", up));
    println!("{}", logged(&tcspecial_log, "protocol version 11", up));
    let said = std::fs::read_to_string(&tcspecial_log).unwrap_or_default();
    assert!(
        !said.contains("refused"),
        "tcspecial refused the peer:\n{said}"
    );

    ground.downlink();

    // The beacon, every reset_interval until the start is matched, as the
    // radio carries it, published to TO_LAB's subscription through SBN.
    let beacon = ground
        .published(BEACON, Duration::from_secs(10))
        .expect("a beacon through cFS");
    Beacon::read(&beacon).expect("the beacon as the radio carries it");

    // CONNECT, carrying the configuration's checksum, published by CI_LAB
    // and carried to tcspecial by SBN, and its answer back the same way.
    let graph = Graph::load(&configuration).expect("the configuration");
    let digest = digest_of_graph(&graph).expect("a digest");
    let connect = Command::Connect(ConnectCommand::new(1, digest));
    let mut answer = None;
    for _ in 0..20 {
        ground.send(SPACECRAFT_COMMANDS, &connect);
        answer =
            ground.published(SPACECRAFT_ANSWERS, Duration::from_millis(700));
        if answer.is_some() {
            break;
        }
    }
    let answer =
        Telemetry::read(&answer.expect("CONNECT answered")).expect("an answer");
    assert_eq!(answer.status(), CommandStatus::Success, "{answer:?}");

    // A reading, from the thermometer to fsw, published as its tlm_base
    // plus its device id.
    let sensor = UdpSocket::bind("127.0.0.1:0").expect("a socket");
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut reading = None;
    while reading.is_none() && Instant::now() < deadline {
        sensor.send_to(b"293.15", THERMOMETER_AT).expect("sent");
        reading = ground.published(THERMOMETER, Duration::from_millis(500));
    }
    assert_eq!(reading.as_deref(), Some(&b"293.15"[..]));

    // A command for fsw, published as its cmd_msgid, goes on to the heater.
    let mut buffer = [0u8; 64];
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut heard = None;
    while heard.is_none() && Instant::now() < deadline {
        ground.command(FSW, 0, b"on");
        heard = heater.recv(&mut buffer).ok();
    }
    assert_eq!(heard.map(|got| &buffer[..got]), Some(&b"on"[..]));

    // A node command under its node's message ID, answered under its
    // node's answer ID.
    let query = Command::QueryNode(NodeCommand::query(2, NodeId(1)));
    let mut answer = None;
    for _ in 0..10 {
        ground.send(NODE_COMMANDS + 1, &query);
        answer = ground.published(NODE_ANSWERS + 1, Duration::from_millis(700));
        if answer.is_some() {
            break;
        }
    }
    let answer = Telemetry::read(&answer.expect("the query answered"))
        .expect("an answer");
    assert_eq!(answer.status(), CommandStatus::Success, "{answer:?}");
}
