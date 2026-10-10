//! The client's operations, driven against a stand-in for tcspecial.
//!
//! tcslib is what a control application is built on, so what matters is that
//! each operation sends the command it says and hands back what the answering
//! telemetry carried. Until this existed the client had one test, of a
//! builder's timeout field, and nothing exercised a command at all.
//!
//! The stand-in is a UDP socket that reads a command and answers it, which is
//! the whole of what the client needs on the other end. Nothing starts
//! tcspecial: a test that needed the flight software running could not be run
//! twice at once, and would be testing that as much as this.

use std::net::UdpSocket;
use std::thread;
use std::time::Duration;

use tcslibgs::{
    ArmKey, BeaconTime, Command, CommandStatus, ConfigDHTelemetry, ConfigTelemetry,
    ConfigDigest, ConfigVersion, ConnectTelemetry, DHId, DHName,
    DHSample, DHType, PingTelemetry, QueryDHSampleTelemetry, QueryDHTelemetry, RestartArmTelemetry,
    RestartTelemetry, StartDHTelemetry, Statistics, StopDHTelemetry, Telemetry,
};
use tcslib::{TcsClient, UdpConnection};

/// Statistics distinct enough that passing the wrong field through would show.
fn stub_statistics() -> Statistics {
    Statistics {
        timestamp: None,
        bytes_received: 11,
        reads_completed: 22,
        reads_failed: 33,
        bytes_sent: 44,
        writes_completed: 55,
        writes_failed: 66,
        triggers_sent: 77,
    }
}

fn stub_sample(first: u8) -> DHSample {
    let mut sample = DHSample::new();
    sample.record(&[first, first + 1, first + 2]);
    sample
}

/// The version a payload set states, as the stub spacecraft answers it.
///
/// Not this build's: the two are different things, and a test that used one
/// value for both would pass whichever of them the answer carried.
const A_SETS_VERSION: ConfigVersion = ConfigVersion {
    major: 2,
    minor: 3,
    patch: 4,
};

/// Answer `count` commands as tcspecial would, then stop.
///
/// Every answer carries the command's own sequence number, which is what lets
/// a caller match one to the other.
fn stub_spacecraft(count: usize) -> (String, thread::JoinHandle<()>) {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let addr = socket.local_addr().expect("addr").to_string();
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");

    let handle = thread::spawn(move || {
        let mut buffer = vec![0u8; 65535];
        for _ in 0..count {
            let (size, from) = match socket.recv_from(&mut buffer) {
                Ok(received) => received,
                Err(_) => return,
            };
            let command: Command =
                serde_json::from_slice(&buffer[..size]).expect("the client sent a valid command");
            let seq = command.sequence();

            let answer = match command {
                Command::Ping(_) => {
                    Telemetry::Ping(PingTelemetry::new(seq, CommandStatus::Success))
                }
                // The stub answers with the ground's own two, which is a
                // spacecraft reading the same configuration.
                Command::Connect(cmd) => Telemetry::Connect(ConnectTelemetry::new(
                    seq,
                    cmd.version,
                    // The ground does not say which set it read, so the stub
                    // answers with a version of its own: what matters here is
                    // that all three come back.
                    A_SETS_VERSION,
                    cmd.digest,
                )),
                Command::RestartArm(_) => {
                    Telemetry::RestartArm(RestartArmTelemetry::new(seq, CommandStatus::Success))
                }
                Command::Restart(_) => {
                    Telemetry::Restart(RestartTelemetry::new(seq, CommandStatus::Success))
                }
                Command::StartDH(_) => {
                    Telemetry::StartDH(StartDHTelemetry::new(seq, CommandStatus::Success))
                }
                Command::StopDH(_) => {
                    Telemetry::StopDH(StopDHTelemetry::new(seq, CommandStatus::Success))
                }
                Command::QueryDH(cmd) => Telemetry::QueryDH(QueryDHTelemetry::new(
                    seq,
                    CommandStatus::Success,
                    cmd.dh_id,
                    stub_statistics(),
                )),
                Command::QueryDHSample(cmd) => {
                    Telemetry::QueryDHSample(QueryDHSampleTelemetry::new(
                        seq,
                        CommandStatus::Success,
                        cmd.dh_id,
                        stub_sample(0x10),
                        stub_sample(0x20),
                    ))
                }
                Command::Config(_) => {
                    Telemetry::Config(ConfigTelemetry::new(seq, CommandStatus::Success))
                }
                Command::ConfigDH(_) => {
                    Telemetry::ConfigDH(ConfigDHTelemetry::new(seq, CommandStatus::Success))
                }
            };

            let data = serde_json::to_vec(&answer).expect("serialise");
            socket.send_to(&data, from).expect("answer");
        }
    });

    (addr, handle)
}

/// A client whose two links both go to the one stub.
///
/// The stub answers whatever arrives, so one socket stands in for both links
/// here. What the two links are for is tested where they are served; this is
/// about every operation reaching the far end and coming back.
fn client_to(addr: &str) -> TcsClient {
    let spacecraft = UdpConnection::new("127.0.0.1:0", addr).expect("connection");
    let payload = UdpConnection::new("127.0.0.1:0", addr).expect("the payload link");
    let mut client = TcsClient::new(Box::new(spacecraft), Box::new(payload));
    client.set_timeout(Duration::from_secs(10));
    client
}

#[test]
fn every_operation_reaches_the_spacecraft_and_comes_back() {
    let (addr, stub) = stub_spacecraft(10);
    let mut client = client_to(&addr);

    assert!(client.ping().unwrap().header.status.is_success());

    // What each end read, which is the first thing a link says. Both come
    // back, so that the caller can tell a version difference from a
    // configuration difference rather than being handed a verdict.
    let version = ConfigVersion::of_this_build();
    let digest = ConfigDigest([0x5A; 16]);
    assert_eq!(
        client.connect(version, digest).unwrap(),
        (version, A_SETS_VERSION, digest)
    );
    assert_eq!(
        client.restart_arm(ArmKey(0x1234)).unwrap(),
        CommandStatus::Success
    );
    assert_eq!(
        client.restart(ArmKey(0x1234)).unwrap(),
        CommandStatus::Success
    );
    assert_eq!(
        client
            .start_dh(DHId(2), DHType::Device, DHName::new("DH2"))
            .unwrap(),
        CommandStatus::Success
    );
    assert_eq!(client.stop_dh(DHId(2)).unwrap(), CommandStatus::Success);

    // The statistics must arrive field for field, not merely arrive.
    let (status, stats) = client.query_dh(DHId(2)).unwrap();
    assert_eq!(status, CommandStatus::Success);
    assert_eq!(stats, stub_statistics());

    // Sent first, received second, which is the order the client documents.
    let (status, sent, received) = client.query_dh_sample(DHId(2)).unwrap();
    assert_eq!(status, CommandStatus::Success);
    assert_eq!(sent.data(), &[0x10, 0x11, 0x12]);
    assert_eq!(received.data(), &[0x20, 0x21, 0x22]);

    assert_eq!(
        client.configure(BeaconTime(1000)).unwrap(),
        CommandStatus::Success
    );

    stub.join().expect("the stand-in answered every command");
}

#[test]
fn a_spacecraft_that_does_not_answer_times_out() {
    // Nothing is listening, so this is what a control application sees when
    // the link is down: an error rather than a wait with no end.
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = socket.local_addr().unwrap().to_string();
    drop(socket);

    let connection = UdpConnection::new("127.0.0.1:0", &addr).unwrap();
    let payload = UdpConnection::new("127.0.0.1:0", &addr).unwrap();
    let mut client = TcsClient::new(Box::new(connection), Box::new(payload));
    client.set_timeout(Duration::from_millis(250));

    assert!(client.ping().is_err(), "a silent link should be an error");
}
