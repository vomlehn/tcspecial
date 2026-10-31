//! The cfs example, flown: a vehicle that answers to a cFS flight computer
//! joins its software bus as an SBN peer, publishes the beacon, its answers
//! and each reading under their message IDs, and takes commands published
//! for it.
//!
//! What is run is `examples/cfs/cfs.ytcs` itself, with each of its sockets
//! moved to a port nothing holds and its log and state file moved to a
//! directory of the test's. The test is the cFS end -- a UDP socket speaking
//! SBN's wire format, as `tcslibgs::sbn` and `tcslibgs::cfs` state it -- and
//! both payloads. A run against cFS's own Linux build, with SBN, is the test
//! that the format is SBN's and not only ours; this is the test that the
//! vehicle keeps to it.

use std::collections::BTreeSet;
use std::net::{SocketAddr, UdpSocket};
use std::path::Path;
use std::process::{Child, Command as Process, Stdio};
use std::time::{Duration, Instant};

use tcslibgs::sbn::{self, kind, Header};
use tcslibgs::{
    cfs, Command, CommandStatus, ConfigDigest, ConnectCommand, NodeCommand,
    NodeId, Telemetry,
};

mod common;

/// The example's spacecraft, and the two processors.
const SPACECRAFT: u32 = 0x42;
const OURS: u32 = 2;
const THEIRS: u32 = 1;

/// The example's message IDs.
const SPACECRAFT_COMMANDS: u16 = 0x4280;
const SPACECRAFT_ANSWERS: u16 = 0x4200;
const BEACON: u16 = 0x4201;
const FSW: u16 = 0x4281;
const THERMOMETER: u16 = 0x4242;
const NODE_COMMANDS: u16 = 0x4290;
const NODE_ANSWERS: u16 = 0x4210;

/// Binds a socket on a loopback port nothing holds.
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

/// tcspecial, stopped however the test ends.
struct Flying(Child);

impl Drop for Flying {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Writes `was` as `now` in `text`, where it appears exactly once.
fn moved(text: &str, was: &str, now: &str) -> String {
    assert_eq!(
        text.matches(was).count(),
        1,
        "the example should say {was} once"
    );
    text.replace(was, now)
}

/// The cFS end of the bus, played by the test.
struct Peer {
    socket: UdpSocket,
    /// Where tcspecial is, once it has been heard from.
    vehicle: Option<SocketAddr>,
    ident: [u8; sbn::IDENT_BYTES],
    sequence: u16,
}

impl Peer {
    /// Sends a message of `what` type to tcspecial, as the peer.
    fn say(&self, what: u8, message: &[u8]) {
        let mut room = vec![0u8; sbn::HEADER_BYTES + message.len()];
        let size = Header {
            kind: what,
            processor_id: THEIRS,
            spacecraft_id: SPACECRAFT,
        }
        .write(&mut room, message)
        .expect("it fits");
        let to = self.vehicle.expect("tcspecial has been heard from");
        self.socket.send_to(&room[..size], to).expect("sent");
    }

    /// Publishes a command for tcspecial, its checksum made, or broken
    /// where `broken`.
    fn command(&mut self, msgid: u16, code: u8, data: &[u8], broken: bool) {
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
        if broken {
            room[size - 1] ^= 0x01;
        }
        self.say(kind::APPLICATION, &room[..size]);
    }

    /// Publishes a tcspecial command under `msgid`, as its function code
    /// says.
    fn send(&mut self, msgid: u16, command: &Command) {
        let code = command.cmd_type().to_u8();
        self.command(msgid, code, &command.to_bytes(), false);
    }

    /// What tcspecial sends next, or `None` where nothing comes in the
    /// socket's timeout.
    fn heard(&mut self) -> Option<(Header, Vec<u8>)> {
        let mut room = [0u8; 65536];
        let (size, from) = self.socket.recv_from(&mut room).ok()?;
        self.vehicle = Some(from);
        let (header, message) =
            Header::read(&room[..size]).expect("an SBN datagram");
        assert_eq!(
            (header.processor_id, header.spacecraft_id),
            (OURS, SPACECRAFT),
            "tcspecial says who it is in every header"
        );
        Some((header, message.to_vec()))
    }

    /// Waits for tcspecial to send a message of `what` type, passing over
    /// any other.
    fn until(&mut self, what: u8, within: Duration) -> Vec<u8> {
        let deadline = Instant::now() + within;
        while Instant::now() < deadline {
            if let Some((header, message)) = self.heard() {
                if header.kind == what {
                    return message;
                }
            }
        }
        panic!("tcspecial sent no message of type {what:#04x}");
    }

    /// Waits for a telemetry packet under `msgid`, passing over the rest.
    fn published(&mut self, msgid: u16, within: Duration) -> Option<Vec<u8>> {
        let deadline = Instant::now() + within;
        while Instant::now() < deadline {
            if let Some((header, message)) = self.heard() {
                if header.kind != kind::APPLICATION {
                    continue;
                }
                let frame = cfs::read_telemetry(&message)
                    .expect("tcspecial publishes telemetry");
                if frame.msgid == msgid {
                    assert_ne!(frame.time, [0; 6], "stamped with cFE time");
                    return Some(frame.data.to_vec());
                }
            }
        }
        None
    }

    /// Sends a CONNECT carrying `digest` until it is answered, as a ground
    /// station does once tcspecial is up.
    fn connect(&mut self, digest: ConfigDigest) -> Telemetry {
        let connect = Command::Connect(ConnectCommand::new(1, digest));
        for _ in 0..20 {
            self.send(SPACECRAFT_COMMANDS, &connect);
            if let Some(answer) =
                self.published(SPACECRAFT_ANSWERS, Duration::from_millis(500))
            {
                return Telemetry::read(&answer).expect("an answer");
            }
        }
        panic!("nothing answered a CONNECT on the bus");
    }
}

/// The example, moved to ports of the test's, with the peer bound.
///
/// # Returns
///
/// The directory it is written in, the configuration's path, the peer, the
/// heater, and the thermometer's port.
fn example() -> (tempfile::TempDir, std::path::PathBuf, Peer, UdpSocket, u16) {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/cfs/cfs.ytcs");
    let text = std::fs::read_to_string(&path).expect("the example reads");
    let dir = tempfile::tempdir().expect("a directory");
    std::fs::create_dir(dir.path().join("telemetry"))
        .expect("the log's directory");
    let socket = bound();
    let heater = bound();
    let thermometer = free();
    let mut text = text;
    for (was, now) in [
        ("0.0.0.0:8634", format!("127.0.0.1:{}", free())),
        ("127.0.0.1:2234", format!("127.0.0.1:{}", port(&socket))),
        ("\"8601\"", format!("\"{}\"", port(&heater))),
        ("\"8602\"", format!("\"{thermometer}\"")),
        (
            "/tmp/telemetry",
            dir.path().join("telemetry").display().to_string(),
        ),
        (
            "/tmp/tcspecial-cfs.state",
            dir.path().join("state").display().to_string(),
        ),
    ] {
        text = moved(&text, was, &now);
    }
    let configuration = dir.path().join("cfs.ytcs");
    std::fs::write(&configuration, &text).expect("written");
    let ident = sbn::ident_of("0653995939d714ac94906f867b4397e9953ca66a")
        .expect("the example's ident");
    let peer = Peer {
        socket,
        vehicle: None,
        ident,
        sequence: 0,
    };
    (dir, configuration, peer, heater, thermometer)
}

/// Starts tcspecial on `configuration`.
fn fly(configuration: &Path) -> Flying {
    Flying(
        Process::new(env!("CARGO_BIN_EXE_tcspecial"))
            .arg(configuration)
            .arg("--quiet")
            .arg("--simulate=all")
            .env_remove("RUST_LOG")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("tcspecial starts"),
    )
}

/// Connects the peer: waits for tcspecial's announcement, answers it with
/// the release's protocol version, and checks what tcspecial says back.
///
/// # Returns
///
/// The message IDs tcspecial subscribed to.
fn joined(peer: &mut Peer) -> BTreeSet<u16> {
    let announced = peer.until(kind::ANNOUNCE, Duration::from_secs(20));
    assert!(announced.is_empty(), "an announcement carries nothing");
    peer.say(kind::PROTOCOL, &[11]);
    assert_eq!(
        peer.until(kind::PROTOCOL, Duration::from_secs(5)),
        vec![11],
        "SBN v7.0.1's protocol version"
    );
    let subscribed = peer.until(kind::SUBSCRIBE, Duration::from_secs(5));
    assert_eq!(&subscribed[..sbn::IDENT_BYTES], &peer.ident[..]);
    sbn::read_subscriptions(&subscribed, &peer.ident)
        .expect("the far end's ident")
        .flatten()
        .collect()
}

#[test]
fn a_vehicle_answering_to_cfs_is_a_peer_of_its_software_bus() {
    let (_dir, configuration, mut peer, heater, thermometer) = example();
    let _flying = fly(&configuration);

    // tcspecial speaks first, as it must: SBN on cFS announces itself only
    // to a peer on another spacecraft.
    let subscribed = joined(&mut peer);
    assert_eq!(
        subscribed,
        BTreeSet::from([
            SPACECRAFT_COMMANDS,
            FSW,
            NODE_COMMANDS,
            NODE_COMMANDS + 1,
            NODE_COMMANDS + 2
        ]),
        "its command message IDs and nothing else"
    );

    // What the cFS side's applications want.
    let mut room = [0u8; 256];
    let wanted = [
        SPACECRAFT_ANSWERS,
        BEACON,
        THERMOMETER,
        NODE_ANSWERS,
        NODE_ANSWERS + 1,
        NODE_ANSWERS + 2,
    ];
    let size = sbn::write_subscriptions(
        &mut room,
        &peer.ident,
        wanted.iter().copied(),
    )
    .unwrap();
    peer.say(kind::SUBSCRIBE, &room[..size]);

    // The beacon, every reset_interval until the start is matched.
    let beacon = peer
        .published(BEACON, Duration::from_secs(5))
        .expect("a beacon on the bus");
    assert!(
        tcslibgs::Beacon::read(&beacon).is_ok(),
        "the beacon as the radio carries it"
    );

    // A command whose checksum is wrong is not answered.
    let connect = Command::Connect(ConnectCommand::new(
        1,
        common::checksum_of(&configuration),
    ));
    let bytes = connect.to_bytes();
    peer.command(SPACECRAFT_COMMANDS, 0x04, &bytes, true);
    assert_eq!(
        peer.published(SPACECRAFT_ANSWERS, Duration::from_millis(700)),
        None,
        "a command whose checksum is wrong is not answered"
    );
    // Nor is one whose function code is not its command's code.
    peer.command(SPACECRAFT_COMMANDS, 0x05, &bytes, false);
    assert_eq!(
        peer.published(SPACECRAFT_ANSWERS, Duration::from_millis(700)),
        None,
        "a function code that is not the command's code is not answered"
    );

    // CONNECT, carrying the configuration's checksum, on tcs_cmd.
    let answer = peer.connect(common::checksum_of(&configuration));
    assert_eq!(answer.status(), CommandStatus::Success, "{answer:?}");

    // A reading: from the thermometer to fsw, published as its tlm_base
    // plus its device id.
    let sensor = bound();
    let to: SocketAddr = format!("127.0.0.1:{thermometer}")
        .parse()
        .expect("an address");
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut reading = None;
    while reading.is_none() && Instant::now() < deadline {
        sensor.send_to(b"293.15", to).expect("sent");
        reading = peer.published(THERMOMETER, Duration::from_millis(300));
    }
    assert_eq!(reading.as_deref(), Some(&b"293.15"[..]));

    // A command for fsw, published as its cmd_msgid, goes on to the heater.
    let mut buffer = [0u8; 64];
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut heard = None;
    while heard.is_none() && Instant::now() < deadline {
        peer.command(FSW, 0, b"on", false);
        heard = heater.recv(&mut buffer).ok();
    }
    assert_eq!(heard.map(|got| &buffer[..got]), Some(&b"on"[..]));

    // A node command, under its node's message ID, answered under its
    // node's answer ID.
    let query = Command::QueryNode(NodeCommand::query(2, NodeId(1)));
    peer.send(NODE_COMMANDS + 1, &query);
    let answer = peer
        .published(NODE_ANSWERS + 1, Duration::from_secs(5))
        .expect("an answer under node 1's answer ID");
    let answer = Telemetry::read(&answer).expect("an answer");
    assert_eq!(answer.status(), CommandStatus::Success, "{answer:?}");

    // And one about node 2 published under node 1's: refused, as a command
    // on the wrong UDP link is.
    let query = Command::QueryNode(NodeCommand::query(3, NodeId(2)));
    peer.send(NODE_COMMANDS + 1, &query);
    let answer = peer
        .published(NODE_ANSWERS + 1, Duration::from_secs(5))
        .expect("an answer under node 1's answer ID");
    let answer = Telemetry::read(&answer).expect("an answer");
    assert_eq!(answer.status(), CommandStatus::InvalidCommand);
}

/// A peer whose subscriptions carry another ident is refused and
/// disconnected, every time it tries again.
#[test]
fn a_peer_with_another_ident_is_refused_each_time() {
    let (_dir, configuration, mut peer, _heater, _thermometer) = example();
    let _flying = fly(&configuration);
    let other = sbn::ident_of(&"1".repeat(40)).unwrap();
    for _ in 0..2 {
        joined(&mut peer);
        let mut room = [0u8; 64];
        let size = sbn::write_subscriptions(
            &mut room,
            &other,
            [SPACECRAFT_ANSWERS].into_iter(),
        )
        .unwrap();
        peer.say(kind::SUBSCRIBE, &room[..size]);
        peer.until(kind::DISCONNECT, Duration::from_secs(5));
    }
}

/// A peer whose protocol version is not the release's is refused and
/// disconnected.
#[test]
fn a_peer_of_another_protocol_version_is_refused() {
    let (_dir, configuration, mut peer, _heater, _thermometer) = example();
    let _flying = fly(&configuration);
    peer.until(kind::ANNOUNCE, Duration::from_secs(20));
    peer.say(kind::PROTOCOL, &[10]);
    peer.until(kind::DISCONNECT, Duration::from_secs(5));
}
