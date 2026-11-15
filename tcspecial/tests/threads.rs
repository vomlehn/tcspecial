//! QUERY_THREADS, asked of tcspecial itself: every thread the vehicle runs
//! on is told of, with what it is for, the node it belongs to, the stack it
//! was given and how deep it has been into it.
//!
//! The figures themselves are tested where they are measured, in
//! `runtime::thread`; this is about each thread being there, with the role
//! and the stack its role is given.

use std::net::UdpSocket;
use std::time::{Duration, Instant};

use tcsprotocol::{
    Command, CommandStatus, NodeId, Telemetry, ThreadRole, ThreadStack,
    ThreadsCommand, NO_NODE,
};

mod common;
use common::{bound, connect, free, port, Flying};

/// The stack the configuration gives each node's thread: not the default,
/// so that the test sees the configuration's.
const NODE_STACK: u32 = 48 * 1024;

/// Asks for the threads from the `first`, until an answer comes.
///
/// # Arguments
///
/// * `from`  - The ground's end of the node command link.
/// * `to`    - tcspecial's.
/// * `first` - The position of the first thread to tell of.
///
/// # Returns
///
/// The answer.
///
/// # Panics
///
/// If nothing answers within ten seconds, or what answers is not an answer
/// to QUERY_THREADS.
fn threads(
    from: &UdpSocket,
    to: std::net::SocketAddr,
    first: u32,
) -> tcsprotocol::QueryThreadsTelemetry {
    let asked = Command::QueryThreads(ThreadsCommand::new(100 + first, first))
        .to_bytes();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut buffer = [0u8; 65535];
    while Instant::now() < deadline {
        from.send_to(&asked, to).expect("sent");
        if let Ok(got) = from.recv(&mut buffer) {
            match Telemetry::read(&buffer[..got]) {
                Ok(Telemetry::QueryThreads(answer)) => return answer,
                other => panic!("not an answer to QUERY_THREADS: {other:?}"),
            }
        }
    }
    panic!("nothing answered QUERY_THREADS");
}

#[test]
fn every_thread_is_told_of_with_its_role_and_stack() {
    let dir = tempfile::tempdir().expect("a directory");
    let beacons = bound();
    let commands = bound();
    let nodes = bound();
    let take_at = free();
    let nodes_at = free();
    let at = || format!("127.0.0.1:{}", free());

    let configuration = dir.path().join("vehicle.ytcs");
    let state = dir.path().join("tcspecial.state").display().to_string();
    std::fs::write(
        &configuration,
        format!(
            "tcspecial:
  gateway: radio
  interface: \"{}\"
  beacon: {{ default_interval: \"100 ms\", reset_interval: \"100 ms\", \
  recv: \"127.0.0.1:{}\", send: \"{}\" }}
  payload_command: {{ dest_base: \"127.0.0.1:{nodes_at}\", \
  source_base: \"127.0.0.1:{}\" }}
  payload_telemetry: {{ dest_base: \"{}\", source_base: \"{}\" }}
  state_file: \"{state}\"
  thread_stack: \"{NODE_STACK}\"
  tcs_cmd: {{ recv: \"127.0.0.1:{take_at}\", \
  send: \"127.0.0.1:{}\" }}
gateways:
  - type gateway_radio:
      - gateway: ground
        address: \"{}\"
        ground_base: \"127.0.0.1:{}\"
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
            port(&nodes),
            at(),
            at(),
            port(&commands),
            at(),
            free(),
        ),
    )
    .expect("written");
    let _vehicle =
        Flying::logged(&configuration, &dir.path().join("tcspecial.log"));

    // Matched, so that the command is acted on.
    let digest = common::checksum_of(&configuration);
    let answer = connect(
        &commands,
        format!("127.0.0.1:{take_at}").parse().expect("an address"),
        1,
        digest,
    );
    assert_eq!(answer.header().status, CommandStatus::Success);

    let to = format!("127.0.0.1:{nodes_at}").parse().expect("an address");
    let answer = threads(&nodes, to, 0);
    assert_eq!(answer.header.status, CommandStatus::Success);
    assert_eq!(answer.first, 0);
    let told: Vec<ThreadStack> = answer.told().to_vec();
    assert_eq!(told.len(), answer.total as usize, "all in one answer");

    // tcspecial's own, once each and belonging to no node, with the
    // service stack; the main thread's is the kernel's.
    let service = tcspecial::runtime::thread::SERVICE_STACK as u32;
    for role in [
        ThreadRole::Signals,
        ThreadRole::Commands,
        ThreadRole::Beacons,
    ] {
        let of: Vec<_> = told.iter().filter(|t| t.role == role).collect();
        assert_eq!(of.len(), 1, "{role:?}: {told:?}");
        assert_eq!(of[0].node, NO_NODE, "{role:?}");
        assert_eq!(of[0].stack, service, "{role:?}");
    }
    let main: Vec<_> =
        told.iter().filter(|t| t.role == ThreadRole::Main).collect();
    assert_eq!(main.len(), 1, "{told:?}");
    assert!(main[0].stack >= service, "{main:?}");

    // A thread for each node, by its id -- ground is 0 and pinger 1, in the
    // order of their names -- with the configuration's stack.
    for node in [NodeId(0), NodeId(1)] {
        assert!(
            told.iter().any(|t| t.role == ThreadRole::Node
                && t.node == node
                && t.stack == NODE_STACK),
            "{node:?}: {told:?}"
        );
    }

    // Every one has used some of its stack, and none more than it has.
    for thread in &told {
        assert!(thread.deepest > 0, "{thread:?}");
        assert!(thread.deepest <= thread.stack, "{thread:?}");
    }

    // Asked from past the last, none, and still how many there are.
    let past = threads(&nodes, to, answer.total);
    assert_eq!(past.header.status, CommandStatus::Success);
    assert!(past.told().is_empty());
    assert_eq!(past.total, answer.total);
}
