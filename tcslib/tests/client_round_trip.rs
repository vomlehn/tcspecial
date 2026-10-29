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

use tcslib::{TcsClient, UdpConnection};
use tcslibgs::{
    ArmKey, BeaconInterval, Command, CommandStatus, ConfigDigest,
    ConfigTelemetry, ConfigVersion, ConnectTelemetry, DHSample, NodeCounts,
    NodeId, NodeTelemetry, PingTelemetry, QueryNodeSampleTelemetry,
    QueryNodeTelemetry, Reason, RestartArmTelemetry, RestartTelemetry,
    SetNodeTelemetry, TcsError, Telemetry,
};

fn stub_sample(first: u8) -> DHSample {
    let mut sample = DHSample::new();
    sample.record(&[first, first + 1, first + 2]);
    sample
}

/// Starts a stand-in spacecraft that answers commands as tcspecial would,
/// on a thread of its own, then stops.
///
/// Every answer carries the command's own sequence number, which is what lets
/// a caller match one to the other.
///
/// # Arguments
///
/// * `count` - How many commands it answers before it stops. It stops
///             sooner if nothing arrives for ten seconds.
///
/// # Returns
///
/// The `host:port` it answers at, and its thread, to be joined.
///
/// # Panics
///
/// If its socket cannot be made, or the thread does if what arrives is not
/// a valid command or the answer cannot be sent.
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
            let command = Command::read(&buffer[..size])
                .expect("the client sent a valid command");
            let seq = command.sequence();
            // Answered as generation 1, the one start this stand-in is.
            let generation = 1;

            let answer = match command {
                Command::Ping(_) => Telemetry::Ping(PingTelemetry::new(
                    seq,
                    CommandStatus::Success,
                )),
                // The stub answers with the ground's own two, which is a
                // spacecraft reading the same configuration.
                Command::Connect(cmd) => Telemetry::Connect(
                    ConnectTelemetry::new(seq, cmd.version, cmd.digest, seq),
                ),
                Command::RestartArm(_) => Telemetry::RestartArm(
                    RestartArmTelemetry::new(seq, CommandStatus::Success),
                ),
                Command::Restart(_) => Telemetry::Restart(
                    RestartTelemetry::new(seq, CommandStatus::Success),
                ),
                Command::Config(_) => Telemetry::Config(ConfigTelemetry::new(
                    seq,
                    CommandStatus::Success,
                )),
                // A node's commands, each answered for the node it named.
                // Every command has an answer here because the stub reads
                // whatever arrives, and a command it could not answer would
                // look like a link that had gone quiet.
                Command::StartNode(cmd) => {
                    Telemetry::StartNode(NodeTelemetry::started(
                        seq,
                        CommandStatus::Success,
                        cmd.node,
                    ))
                }
                Command::StopNode(cmd) => {
                    Telemetry::StopNode(NodeTelemetry::stopped(
                        seq,
                        CommandStatus::Success,
                        cmd.node,
                    ))
                }
                // A setpoint is taken, and anything else refused as the
                // vehicle refuses it: with a reason.
                Command::SetNode(cmd) if cmd.attribute == "setpoint" => {
                    Telemetry::SetNode(SetNodeTelemetry::new(
                        seq,
                        CommandStatus::Success,
                        cmd.node,
                    ))
                }
                Command::SetNode(cmd) => Telemetry::SetNode(
                    SetNodeTelemetry::new(
                        seq,
                        CommandStatus::InvalidParameter,
                        cmd.node,
                    )
                    .because(Reason::of("it is spelled half_band")),
                ),
                Command::QueryNode(cmd) => {
                    Telemetry::QueryNode(QueryNodeTelemetry::new(
                        seq,
                        CommandStatus::Success,
                        cmd.node,
                        true,
                        stub_counts(),
                    ))
                }
                Command::QueryNodeSample(cmd) => {
                    Telemetry::QueryNodeSample(QueryNodeSampleTelemetry::new(
                        seq,
                        CommandStatus::Success,
                        cmd.node,
                        stub_sample(0x30),
                        stub_sample(0x40),
                    ))
                }
            };

            let data = answer.for_generation(generation).to_bytes();
            socket.send_to(&data, from).expect("answer");
        }
    });

    (addr, handle)
}

/// Makes a client whose links all go to the one stub: the spacecraft's link
/// and the one link every node is commanded on.
///
/// The stub answers whatever arrives, so one socket stands in for every link
/// here. What the links are for is tested where they are served; this is
/// about every operation reaching the far end and coming back. The one
/// payload is DH2, which is the payload the operations below ask about.
///
/// # Arguments
///
/// * `addr` - The `host:port` the stub answers at.
///
/// # Returns
///
/// The client, with a ten-second timeout and a beacon of generation 1
/// already heard.
///
/// # Panics
///
/// If a connection cannot be made.
fn client_to(addr: &str) -> TcsClient {
    let spacecraft =
        UdpConnection::new("127.0.0.1:0", addr).expect("connection");
    let nodes = UdpConnection::new("127.0.0.1:0", addr).expect("the node link");
    let mut client =
        TcsClient::of_a_graph(Box::new(spacecraft), Box::new(nodes));
    client.set_timeout(Duration::from_secs(10));
    // A station that has connected, so that it knows which generation is
    // flying and its node commands are sent rather than refused unsent.
    client
        .connect(ConfigVersion::of_this_build(), ConfigDigest([0x5A; 16]))
        .expect("connected");
    client
}

/// Makes the counts a stand-in vehicle says a node has moved.
///
/// # Returns
///
/// The same counts each time, none of them zero but one.
fn stub_counts() -> NodeCounts {
    NodeCounts {
        taken_bytes: 480,
        reads: 40,
        reads_failed: 1,
        given_bytes: 96,
        writes: 8,
        writes_failed: 0,
        dropped: 2,
        asks: 40,
        asks_failed: 3,
        strangers: 4,
        last_moved: Some(tcslibgs::Timestamp {
            micros: 1_700_000_000_000_000,
        }),
    }
}

/// What a node has moved comes back whole, and so does what it last moved.
#[test]
fn what_a_node_has_moved_comes_back() {
    let (addr, stub) = stub_spacecraft(3);
    let mut client = client_to(&addr);

    let (status, enabled, counts) =
        client.query_node(NodeId::of(2)).expect("an answer");
    assert!(status.is_success());
    assert!(
        enabled,
        "and whether it is running, which is read with them"
    );
    assert_eq!(counts, stub_counts(), "every count, not some of them");

    // And the bytes, which are a separate command: asking for them every
    // poll would spend downlink on data nobody reads.
    let (status, taken, given) =
        client.query_node_sample(NodeId::of(2)).expect("an answer");
    assert!(status.is_success());
    assert_eq!(taken.data(), stub_sample(0x30).data());
    assert_eq!(given.data(), stub_sample(0x40).data());

    stub.join().expect("the stand-in answered both");
}

/// A node's attribute is set by the node's id and the attribute's name.
#[test]
fn a_nodes_attribute_is_set_by_id_and_name() {
    let (addr, stub) = stub_spacecraft(3);
    let mut client = client_to(&addr);

    let (status, node, reason) = client
        .set_node(NodeId::of(4), "setpoint", b"295.0")
        .expect("an answer");
    assert!(status.is_success());
    assert_eq!(node, NodeId::of(4));
    assert!(
        reason.is_empty(),
        "a SET_NODE done says nothing of why: {reason}"
    );

    // And one refused says why, so that the operator is told which word to
    // change.
    let (status, _, reason) = client
        .set_node(NodeId::of(4), "half-band", b"1.0")
        .expect("an answer");
    assert_eq!(status, CommandStatus::InvalidParameter);
    assert_eq!(reason.as_str(), "it is spelled half_band");

    stub.join().expect("the stand-in answered");
}

/// A node is started and stopped by its id, and the answer says which node.
///
/// The id as well as the status, because that is what an answer is placed by
/// when two overtake one another on a link: the sequence pairs the answer
/// with the command and the id says which node the pair was about.
#[test]
fn a_node_is_commanded_by_its_id_and_the_answer_says_which() {
    let (addr, stub) = stub_spacecraft(3);
    let mut client = client_to(&addr);

    let (status, node) = client.start_node(NodeId::of(3)).expect("an answer");
    assert!(status.is_success());
    assert_eq!(node, NodeId::of(3));

    let (status, node) = client.stop_node(NodeId::of(3)).expect("an answer");
    assert!(status.is_success());
    assert_eq!(node, NodeId::of(3));

    stub.join().expect("the stand-in answered both");
}

/// A client with no node link refuses a node command rather than sending it.
///
/// The link is what a node command goes on, and a client may be built with
/// the spacecraft's alone -- a program that only pings, or watches beacons.
/// Refused before anything is sent: a command on the wrong socket means the
/// two ends are not reading the same configuration, and the spacecraft is
/// right to refuse one.
#[test]
fn a_client_with_no_node_link_refuses_a_node_command() {
    // Nothing is listening: the refusal happens before anything is sent, so
    // there is nothing for a far end to do.
    let spacecraft =
        UdpConnection::new("127.0.0.1:0", "127.0.0.1:1").expect("connection");
    let mut client = TcsClient::spacecraft_only(Box::new(spacecraft));
    let said =
        format!("{}", client.start_node(NodeId::of(0)).expect_err("refused"));
    assert!(
        said.contains("node") && said.contains("link"),
        "the refusal should say why: {said}"
    );
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
    assert_eq!(client.connect(version, digest).unwrap(), (version, digest));
    assert_eq!(
        client.restart_arm(ArmKey(0x1234)).unwrap(),
        CommandStatus::Success
    );
    assert_eq!(
        client.restart(ArmKey(0x1234)).unwrap(),
        CommandStatus::Success
    );

    // Nothing but a CONNECT, an arm and a restart may follow a RESTART
    // until a beacon says it is done: the rest are refused here, unsent, so
    // the stand-in is not asked them. A PING among them, which goes
    // regardless of the generation but not during the wait.
    let configure = |client: &mut tcslib::TcsClient| {
        client.configure(BeaconInterval::of(Duration::from_secs(1)))
    };
    let refused = configure(&mut client).expect_err("refused while waiting");
    assert!(
        matches!(refused, TcsError::AwaitingRestart(_)),
        "{refused:?}"
    );
    assert!(refused.to_string().contains("starts again"));
    assert!(matches!(client.ping(), Err(TcsError::AwaitingRestart(_))));
    // The three that go, none of them ending the wait: the CONNECT is
    // answered by the start the RESTART went to, and the second RESTART
    // says nothing of whether the first was taken.
    assert_eq!(client.connect(version, digest).unwrap(), (version, digest));
    assert_eq!(
        client.restart_arm(ArmKey(0x5678)).unwrap(),
        CommandStatus::Success
    );
    assert_eq!(
        client.restart(ArmKey(0x5678)).unwrap(),
        CommandStatus::Success
    );
    assert!(client.restart_wait().is_some());
    // Not by the old start's beacons, though they carry the bit.
    assert_eq!(client.beacon_heard(false, true), None);
    assert!(client.restart_wait().is_some());
    // By the next start, which says it was the one commanded.
    let ended = client.beacon_heard(true, true).expect("it ends the wait");
    assert_eq!(ended.commanded, Some(true));
    assert!(client.restart_wait().is_none());
    // And then a CONNECT, before anything but a PING.
    assert!(matches!(
        configure(&mut client),
        Err(TcsError::GenerationUnknown(_))
    ));
    client.connect(version, digest).unwrap();
    assert_eq!(configure(&mut client).unwrap(), CommandStatus::Success);

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
    let nodes = UdpConnection::new("127.0.0.1:0", &addr).unwrap();
    let mut client =
        TcsClient::of_a_graph(Box::new(connection), Box::new(nodes));
    client.set_timeout(Duration::from_millis(250));

    assert!(client.ping().is_err(), "a silent link should be an error");
}
