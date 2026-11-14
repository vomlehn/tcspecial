//! A vehicle that answers to Yamcs, flown: every packet to and from the
//! ground is a CCSDS Space Packet.
//!
//! Through tcspecial itself, as a Yamcs ground station would see it: the
//! beacon arrives as telemetry under `beacon.apid`; a CONNECT sent as a
//! telecommand under `tcs_cmd.apid` is answered as telemetry under the same
//! APID, and one sent bare or under another APID is not answered at all;
//! and once matched, a ping sent to the `gateway_yamcs` node under its
//! `apid_base` comes back from the ping node under `apid_base` plus that
//! node's device id. `tcs_cmd.send` is written with port 0, so commands
//! from any port of the ground's host are taken, as Yamcs sends them. A
//! payload nothing in the drawing touches reports over `payload_telemetry`,
//! framed too: as telemetry under `payload_telemetry.apid_base` plus its
//! device id, at `dest_base` plus the same. The test is the ground station.

use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use tcsprotocol::ccsds::{read_header, PacketType, Sender};
use tcsprotocol::link_type::{uncounted, Count};
use tcsprotocol::{Beacon, Command, ConnectCommand, Telemetry};

mod common;

use common::{bound, checksum_of, free, port, Flying};

/// What arrives at `at` within `wait`, if anything does.
///
/// # Arguments
///
/// * `at`   - The socket to listen on, with a short read timeout.
/// * `wait` - How long to keep listening.
///
/// # Returns
///
/// The first datagram to arrive, or `None`.
fn heard(at: &UdpSocket, wait: Duration) -> Option<Vec<u8>> {
    let deadline = Instant::now() + wait;
    let mut buffer = [0u8; 65535];
    while Instant::now() < deadline {
        if let Ok(got) = at.recv(&mut buffer) {
            return Some(buffer[..got].to_vec());
        }
    }
    None
}

/// Sends `data` framed by `sender` from `from` to `to`.
///
/// # Arguments
///
/// * `sender` - What frames it, and counts it.
/// * `from`   - The socket to send from.
/// * `to`     - Where to send it.
/// * `data`   - What the packet carries.
///
/// # Panics
///
/// If it cannot be framed or sent.
fn send_framed(
    sender: &mut Sender,
    from: &UdpSocket,
    to: SocketAddr,
    data: &[u8],
) {
    let mut room = vec![0u8; tcsprotocol::ccsds::HEADER_BYTES + data.len()];
    let packet = sender.frame(&mut room, data).expect("framed");
    from.send_to(packet, to).expect("sent");
}

#[test]
fn a_vehicle_answering_to_yamcs_frames_every_packet() {
    let dir = tempfile::tempdir().expect("a directory");
    let beacons = bound();
    let commands = bound();
    // The ping node's name sorts second of the two, device id 1, so what it
    // sends goes to ground_base plus one: the ground socket's port.
    let downlink = bound();
    let take_at = free();
    let gateway_at = free();
    let at = || format!("127.0.0.1:{}", free());

    let configuration = dir.path().join("vehicle.ytcs");
    let state = dir.path().join("tcspecial.state").display().to_string();
    std::fs::write(
        &configuration,
        format!(
            "tcspecial:
  gateway: yamcs
  interface: \"{}\"
  beacon: {{ default_interval: \"100 ms\", reset_interval: \"100 ms\", recv: \
            \"127.0.0.1:{}\", send: \"{}\", apid: \"0x100\" }}
  payload_command: {{ dest_base: \"{}\", source_base: \"{}\" }}
  payload_telemetry: {{ dest_base: \"{}\", source_base: \"{}\", \
            apid_base: \"0x300\" }}
  state_file: \"{state}\"
  tcs_cmd: {{ recv: \"127.0.0.1:{take_at}\", send: \"127.0.0.1:0\", apid: \
            \"0x101\" }}
gateways:
  - type gateway_yamcs:
      - gateway: ground
        address: 127.0.0.1
        apid_base: \"0x200\"
        ground_base: \"127.0.0.1:{}\"
        port: \"{gateway_at}\"
        moc-out: [pinger.ground]
        world-in: [pinger]
vehicle:
  - type ping:
      - compute: pinger
        world-out: [ground.pinger]
        moc-in: [ground]
",
            tcsprotocol::InterfaceDigest::of_this_build(),
            port(&beacons),
            at(),
            at(),
            at(),
            at(),
            at(),
            port(&downlink) - 1,
        ),
    )
    .expect("written");
    let digest = checksum_of(&configuration);
    let _vehicle =
        Flying::logged(&configuration, &dir.path().join("tcspecial.log"));

    // The beacon, as telemetry under beacon.apid.
    let first = heard(&beacons, Duration::from_secs(20)).expect("a beacon");
    let data = read_header(&first, PacketType::Telemetry, 0x100)
        .expect("a framed beacon");
    let beacon = Beacon::read(data).expect("a beacon under the header");
    assert_eq!(beacon.digest, digest);

    // A CONNECT bare, or under another APID, or as telemetry, is not
    // answered.
    let to: SocketAddr = format!("127.0.0.1:{take_at}").parse().unwrap();
    let connect = Command::Connect(ConnectCommand::new(1, digest)).to_bytes();
    commands.send_to(&connect, to).expect("sent");
    assert_eq!(heard(&commands, Duration::from_secs(1)), None, "bare");
    let mut other = Sender::new(PacketType::Telecommand, 0x102).unwrap();
    send_framed(&mut other, &commands, to, &connect);
    assert_eq!(heard(&commands, Duration::from_secs(1)), None, "0x102");
    let mut telemetry = Sender::new(PacketType::Telemetry, 0x101).unwrap();
    send_framed(&mut telemetry, &commands, to, &connect);
    assert_eq!(heard(&commands, Duration::from_secs(1)), None, "telemetry");

    // Framed, it is answered framed.
    let mut tc = Sender::new(PacketType::Telecommand, 0x101).unwrap();
    send_framed(&mut tc, &commands, to, &connect);
    let answer = heard(&commands, Duration::from_secs(5)).expect("an answer");
    let data = read_header(&answer, PacketType::Telemetry, 0x101)
        .expect("a framed answer");
    match Telemetry::read(data).expect("an answer under the header") {
        Telemetry::Connect(connected) => {
            assert_eq!(connected.digest, digest)
        }
        other => panic!("expected a CONNECT's answer: {other:?}"),
    }

    // tcs_cmd.send is port 0, so a command from any port of the host is
    // taken, as Yamcs sends each from whatever port the system gives it,
    // and answered back to that port.
    let another = bound();
    let ping = Command::Ping(tcsprotocol::PingCommand::new(2)).to_bytes();
    send_framed(&mut tc, &another, to, &ping);
    let answer = heard(&another, Duration::from_secs(5))
        .expect("an answer, to the port it came from");
    let data = read_header(&answer, PacketType::Telemetry, 0x101)
        .expect("a framed answer");
    assert!(matches!(Telemetry::read(data), Ok(Telemetry::Ping(_))));

    // Matched: a ping up through the gateway comes back down from the ping
    // node, under apid_base plus its device id.
    let gateway: SocketAddr =
        format!("127.0.0.1:{gateway_at}").parse().unwrap();
    let mut up = Sender::new(PacketType::Telecommand, 0x200).unwrap();
    send_framed(&mut up, &downlink, gateway, b"ping #1 1760900000.000000001");
    let pong = heard(&downlink, Duration::from_secs(5)).expect("a pong");
    let data = read_header(&pong, PacketType::Telemetry, 0x201)
        .expect("a framed pong");
    // A string: after the 32-bit count of its bytes, as XTCE encodes it.
    let start = uncounted(data, Count::Xtce).expect("counted");
    assert!(
        data[start..].starts_with(b"ping #1 1760900000.000000001 "),
        "{data:?}"
    );

    // And one sent up bare is not acted on.
    downlink
        .send_to(b"ping #2 1760900000.000000001", gateway)
        .expect("sent");
    assert_eq!(heard(&downlink, Duration::from_secs(1)), None, "bare");
}

/// Sends CONNECT with `digest`, framed under `apid`, from `from` to `to`
/// until tcspecial answers, as a Yamcs ground station does once tcspecial
/// is up: until then nothing goes down but beacons.
///
/// # Arguments
///
/// * `from`   - The ground's socket, with a short read timeout.
/// * `to`     - Where tcspecial takes commands, `tcs_cmd.recv`.
/// * `apid`   - `tcs_cmd.apid`.
/// * `digest` - The configuration's checksum.
///
/// # Panics
///
/// If nothing answers, framed, within 20 seconds.
fn connect_framed(
    from: &UdpSocket,
    to: SocketAddr,
    apid: u16,
    digest: tcsprotocol::config_digest::ConfigDigest,
) {
    let connect = Command::Connect(ConnectCommand::new(1, digest)).to_bytes();
    let mut tc = Sender::new(PacketType::Telecommand, apid).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        send_framed(&mut tc, from, to, &connect);
        let Some(answer) = heard(from, Duration::from_millis(500)) else {
            continue;
        };
        let data = read_header(&answer, PacketType::Telemetry, apid)
            .expect("a framed answer");
        match Telemetry::read(data).expect("an answer under the header") {
            Telemetry::Connect(connected) => {
                assert_eq!(connected.digest, digest);
                return;
            }
            other => panic!("expected a CONNECT's answer: {other:?}"),
        }
    }
    panic!("nothing answered a CONNECT at {to}");
}

/// A payload nothing touches reports over payload_telemetry, and under
/// Yamcs each datagram goes as CCSDS telemetry under
/// payload_telemetry.apid_base plus the payload's device id, to dest_base
/// plus the same.
#[test]
fn an_untouched_payload_reports_framed_under_its_counted_apid() {
    let dir = tempfile::tempdir().expect("a directory");
    let beacons = bound();
    let commands = bound();
    // "first" and "second" are the only nodes, so "second" is device id
    // 1: its telemetry goes to dest_base plus one, the ground socket's
    // port, under apid_base plus one.
    let telemetry = bound();
    let take_at = free();
    let first_at = free();
    let second_at = free();
    let at = || format!("127.0.0.1:{}", free());

    let configuration = dir.path().join("vehicle.ytcs");
    let state = dir.path().join("tcspecial.state").display().to_string();
    std::fs::write(
        &configuration,
        format!(
            "tcspecial:
  gateway: yamcs
  interface: \"{}\"
  beacon: {{ default_interval: \"100 ms\", reset_interval: \"100 ms\", recv: \
            \"127.0.0.1:{}\", send: \"{}\", apid: \"0x100\" }}
  payload_command: {{ dest_base: \"{}\", source_base: \"{}\" }}
  payload_telemetry: {{ dest_base: \"127.0.0.1:{}\", source_base: \"{}\", \
            apid_base: \"0x300\" }}
  state_file: \"{state}\"
  tcs_cmd: {{ recv: \"127.0.0.1:{take_at}\", send: \"127.0.0.1:0\", apid: \
            \"0x101\" }}
payloads:
  - type network:
      - payload: first
        address: \"127.0.0.1\"
        port: \"{first_at}\"
        protocol: \"udp\"
      - payload: second
        address: \"127.0.0.1\"
        port: \"{second_at}\"
        protocol: \"udp\"
",
            tcsprotocol::InterfaceDigest::of_this_build(),
            port(&beacons),
            at(),
            at(),
            at(),
            port(&telemetry) - 1,
            at(),
        ),
    )
    .expect("written");
    let digest = checksum_of(&configuration);
    let _vehicle =
        Flying::logged(&configuration, &dir.path().join("tcspecial.log"));

    // Nothing goes down but beacons until the ground has matched.
    let to: SocketAddr = format!("127.0.0.1:{take_at}").parse().unwrap();
    connect_framed(&commands, to, 0x101, digest);

    // The payload speaks first, from a socket of its own, to the address its
    // node states, which the vehicle binds. Sent until something arrives:
    // the vehicle may not have bound it the moment it answered.
    let payload = bound();
    let second: SocketAddr = format!("127.0.0.1:{second_at}").parse().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let packet = loop {
        assert!(Instant::now() < deadline, "no telemetry from \"second\"");
        payload.send_to(b"reading 42", second).expect("sent");
        if let Some(packet) = heard(&telemetry, Duration::from_millis(500)) {
            break packet;
        }
    };
    let data = read_header(&packet, PacketType::Telemetry, 0x301)
        .expect("framed under payload_telemetry.apid_base plus 1");
    // Bytes, after the 32-bit count of them, as XTCE encodes it.
    assert_eq!(data, b"\0\0\0\x0areading 42");
}
