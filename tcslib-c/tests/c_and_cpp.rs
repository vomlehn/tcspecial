//! The C/C++ ground station, built against `tcs.h` as C and as C++, run
//! against a vehicle the Rust plays.
//!
//! What keeps the header honest. It is written by hand, so the only proof
//! that it says what the library does is a program that compiles against it
//! and works: `examples/ground.c` with gcc and `examples/ground.cpp` with
//! g++, each linked with the `libtcs.a` this crate builds, each run against a
//! stand-in vehicle that sends beacons as tcspecial serializes them, answers
//! CONNECT, PING and QUERY_NODE, and sends one datum to the moc node. The two
//! must print the same lines, and the right ones.
//!
//! The ports are free ones, found as the test starts. Where there is no gcc
//! or g++ this says so and passes: it tests the header, and a machine with no
//! C compiler has no C program to break.

use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tcslibgs::config_digest::{digest_of_file, ConfigVersion};
use tcslibgs::node::vehicle::ping;
use tcslibgs::{
    Beacon, CommandStatus, ConfigTelemetry, ConnectTelemetry, DHSample,
    NodeCounts, PingTelemetry, QueryNodeSampleTelemetry, QueryNodeTelemetry,
    Reason, RestartArmTelemetry, RestartTelemetry, SetNodeTelemetry, Telemetry,
    Timestamp,
};

fn here() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Where cargo puts `libtcs.a`: the directory above this test's own.
fn library() -> PathBuf {
    let test = std::env::current_exe().expect("where this test is");
    let target = test
        .parent()
        .and_then(Path::parent)
        .expect("the target directory");
    target.join("libtcs.a")
}

/// Build `libtcs.a` from the source as it is now, and say where it is.
///
/// `cargo test` builds this crate for its tests as an rlib and leaves the
/// static library as the last `cargo build` left it, so a test that linked
/// whatever was there could pass against a library older than the change
/// it was meant to test. Built here, with the cargo and the profile running
/// this test, it is this source's.
fn fresh_library() -> PathBuf {
    let library = library();
    let profile = library
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap_or("debug");
    let mut build = Command::new(env!("CARGO"));
    build.args(["build", "--quiet", "-p", "tcslib-c", "--lib"]);
    if profile == "release" {
        build.arg("--release");
    }
    let built = build.output().expect("cargo runs");
    assert!(
        built.status.success(),
        "the library does not build:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert!(
        library.is_file(),
        "{} is not where cargo put it",
        library.display()
    );
    library
}

/// A socket on a port nothing holds.
fn bound() -> UdpSocket {
    UdpSocket::bind("127.0.0.1:0").expect("a port of its own")
}

fn port(socket: &UdpSocket) -> u16 {
    socket.local_addr().expect("its address").port()
}

/// A port nothing holds, for the ground station to bind.
fn free() -> u16 {
    port(&bound())
}

/// Compile `source` with `compiler` against `library`, or `None` where there
/// is no compiler.
fn compiled(
    compiler: &str,
    standard: &str,
    source: &str,
    library: &Path,
    out: &Path,
) -> Option<PathBuf> {
    let ran = Command::new(compiler)
        .args([standard, "-Wall", "-Wextra", "-Werror", "-pedantic"])
        .arg(here().join("examples").join(source))
        .arg("-I")
        .arg(here().join("include"))
        .arg(library)
        .args(["-lpthread", "-ldl", "-lm", "-o"])
        .arg(out)
        .output();
    let ran = match ran {
        Ok(ran) => ran,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("there is no {compiler}, so {source} is not built");
            return None;
        }
        Err(e) => panic!("{compiler} could not be run: {e}"),
    };
    assert!(
        ran.status.success(),
        "{compiler} {source}:\n{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    Some(out.to_path_buf())
}

/// Run `program` against a stand-in vehicle, and say what it printed.
fn against_a_vehicle(program: &Path) -> String {
    // The vehicle's sockets: where beacons come from, where commands are
    // taken, where node commands are taken, where the moc node takes a ping,
    // and where its data is sent from.
    let beacon = bound();
    let interpreter = bound();
    let nodes = bound();
    let uplink = bound();
    let sender = bound();
    for socket in [&interpreter, &nodes, &uplink] {
        socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .expect("a timeout");
    }
    let (beacons, ground_data, cmd_from, nodes_from) =
        (free(), free(), free(), free());

    let dir = tempfile::tempdir().expect("a directory");
    let configuration = dir.path().join("ground.yaml");
    let state = dir.path().join("tcspecial.state").display().to_string();
    std::fs::write(
        &configuration,
        format!(
            "tcspecial:
  gateway: radio
  beacon:
    default_interval: \"5 s\"
    recv: \"127.0.0.1:{beacons}\"
    reset_interval: \"1 s\"
    send: \"127.0.0.1:{}\"
  log:
    directory: \"{}\"
    segment_size: \"65536\"
  payload_command:
    dest_base: \"127.0.0.1:{}\"
    source_base: \"127.0.0.1:{nodes_from}\"
  payload_telemetry:
    dest_base: \"127.0.0.1:{}\"
    source_base: \"127.0.0.1:{}\"
  state_file: \"{state}\"
  tcs_cmd:
    recv: \"127.0.0.1:{}\"
    send: \"127.0.0.1:{cmd_from}\"
moc:
  - name: moc
    type: moc
    address: \"127.0.0.1\"
    port: \"{}\"
    ground_base: \"127.0.0.1:{}\"
    payload-in: true
    moc-out:
      - target: ping
vehicle:
  - name: ping
    type: ping
    moc-in: true
    payload-out:
      - target: moc
",
            port(&beacon),
            dir.path().display(),
            port(&nodes),
            free(),
            free(),
            port(&interpreter),
            port(&uplink),
            // ping is the second node, device id 1, so its data arrives one
            // above the base.
            ground_data - 1,
        ),
    )
    .expect("written");
    let digest = digest_of_file(&configuration).expect("it digests");
    let version = ConfigVersion::of_this_build();

    let stop = Arc::new(AtomicBool::new(false));
    let vehicle = {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut buffer = [0u8; 65535];
            let mut sequence = 0;
            let mut answered = Vec::new();
            while !stop.load(Ordering::Relaxed) {
                sequence += 1;
                let datagram =
                    Beacon::new(sequence, version, digest, 1).to_bytes();
                let _ = beacon.send_to(&datagram, ("127.0.0.1", beacons));

                // A ping, answered as the ping node answers it, down to where
                // the ground takes the ping node's data.
                if let Ok((size, _)) = uplink.recv_from(&mut buffer) {
                    let answer = ping::reply(
                        &buffer[..size],
                        ping::now().expect("a clock"),
                    )
                    .expect("it is a ping");
                    sender
                        .send_to(answer.as_bytes(), ("127.0.0.1", ground_data))
                        .expect("the answer");
                }

                for link in [&interpreter, &nodes] {
                    let Ok((size, from)) = link.recv_from(&mut buffer) else {
                        continue;
                    };
                    let command: tcslibgs::Command =
                        serde_json::from_slice(&buffer[..size])
                            .expect("a command");
                    let seq = command.sequence();
                    answered.push(command.cmd_type());
                    let answer = match &command {
                        tcslibgs::Command::Connect(_) => Telemetry::Connect(
                            ConnectTelemetry::new(seq, version, digest),
                        ),
                        tcslibgs::Command::Ping(_) => Telemetry::Ping(
                            PingTelemetry::new(seq, CommandStatus::Success),
                        ),
                        tcslibgs::Command::QueryNode(cmd) => {
                            Telemetry::QueryNode(QueryNodeTelemetry::new(
                                seq,
                                CommandStatus::Success,
                                cmd.node,
                                true,
                                NodeCounts::default(),
                            ))
                        }
                        // Refused as the vehicle refuses one, with its
                        // reason: which the ground station prints.
                        tcslibgs::Command::SetNode(cmd) => Telemetry::SetNode(
                            SetNodeTelemetry::new(
                                seq,
                                CommandStatus::InvalidParameter,
                                cmd.node,
                            )
                            .because(Reason::of(
                                "no command binding says the ground may set \
                                \"ping.count\"",
                            )),
                        ),
                        tcslibgs::Command::QueryNodeSample(cmd) => {
                            // What it took: "ping", four bytes of a transfer
                            // of six; and nothing given.
                            let mut taken = DHSample::new();
                            taken.time = Some(Timestamp { micros: 1_000_000 });
                            taken.len = 4;
                            taken.total = 6;
                            taken.bytes[..4].copy_from_slice(b"ping");
                            Telemetry::QueryNodeSample(
                                QueryNodeSampleTelemetry::new(
                                    seq,
                                    CommandStatus::Success,
                                    cmd.node,
                                    taken,
                                    DHSample::new(),
                                ),
                            )
                        }
                        tcslibgs::Command::Config(_) => Telemetry::Config(
                            ConfigTelemetry::new(seq, CommandStatus::Success),
                        ),
                        tcslibgs::Command::RestartArm(_) => {
                            Telemetry::RestartArm(RestartArmTelemetry::new(
                                seq,
                                CommandStatus::Success,
                            ))
                        }
                        tcslibgs::Command::Restart(_) => Telemetry::Restart(
                            RestartTelemetry::new(seq, CommandStatus::Success),
                        ),
                        other => panic!(
                            "the ground station sent {:?}",
                            other.cmd_type()
                        ),
                    };
                    let data = serde_json::to_vec(&answer).expect("telemetry");
                    link.send_to(&data, from).expect("an answer");

                    // The query answered, the node's data follows, as it
                    // would from a node that is running.
                    if matches!(command, tcslibgs::Command::QueryNode(_)) {
                        sender
                            .send_to(
                                b"hello from ping",
                                ("127.0.0.1", ground_data),
                            )
                            .expect("the datum");
                    }
                }
            }
            answered
        })
    };

    let ran = Command::new(program)
        .arg("--restart")
        .arg(&configuration)
        .output()
        .expect("the ground station runs");
    stop.store(true, Ordering::Relaxed);
    let answered = vehicle.join().expect("the vehicle stops");

    let printed = String::from_utf8_lossy(&ran.stdout).into_owned();
    assert!(
        ran.status.success(),
        "{}: {printed}\n{}",
        program.display(),
        String::from_utf8_lossy(&ran.stderr)
    );
    assert_eq!(
        answered,
        [
            tcslibgs::CommandType::Connect,
            tcslibgs::CommandType::Ping,
            tcslibgs::CommandType::QueryNode,
            tcslibgs::CommandType::SetNode,
            tcslibgs::CommandType::QueryNodeSample,
            tcslibgs::CommandType::Config,
            tcslibgs::CommandType::RestartArm,
            tcslibgs::CommandType::Restart,
        ],
        "{}: what the vehicle was asked",
        program.display()
    );

    // What it printed, with the digest taken out: it is the temporary
    // configuration's, which differs between the two runs.
    let expected = [
        format!("configuration: md5 {digest}"),
        format!(
            "beacon: Heard, tcspecial v{version} md5 {digest} generation 1"
        ),
        "connect: agrees".to_string(),
        "ping: answered".to_string(),
        "query ping (node 1): Success, running".to_string(),
        "set ping count: refused -- no command binding says the ground \
        may set \"ping.count\""
            .to_string(),
        "sample ping: took \"ping\" of 6 bytes, gave nothing".to_string(),
        "configure: beacon every 5 s, done".to_string(),
        "restart: armed done, restarted done".to_string(),
        "restart: awaiting a beacon of the next generation: yes".to_string(),
        "ping ping through moc: sent".to_string(),
        "data from ping: hello from ping".to_string(),
        "ping ping: answered, round trip timed, up and down summing to it \
        yes"
        .to_string(),
    ];
    let lines: Vec<&str> = printed.lines().collect();
    assert_eq!(lines, expected, "{}", program.display());
    printed.replace(&digest.to_string(), "<digest>")
}

#[test]
fn the_ground_station_works_as_c_and_as_cpp_and_says_the_same() {
    let library = fresh_library();
    let dir = tempfile::tempdir().expect("a directory");
    let c = compiled(
        "gcc",
        "-std=c11",
        "ground.c",
        &library,
        &dir.path().join("ground_c"),
    );
    let cpp = compiled(
        "g++",
        "-std=c++17",
        "ground.cpp",
        &library,
        &dir.path().join("ground_cpp"),
    );

    let said: Vec<String> = [c, cpp]
        .into_iter()
        .flatten()
        .map(|program| against_a_vehicle(&program))
        .collect();
    if let [c, cpp] = said.as_slice() {
        assert_eq!(c, cpp, "the C and the C++ should say the same");
    }
}

/// The header alone, included first, compiles as old C and old C++: nothing
/// in it needs more than C99 or C++11.
#[test]
fn the_header_stands_alone_in_c_and_in_cpp() {
    let dir = tempfile::tempdir().expect("a directory");
    let source = dir.path().join("only.c");
    std::fs::write(
        &source,
        "#include \"tcs.h\"\nint main(void) { return 0; }\n",
    )
    .expect("written");
    for (compiler, language, standard) in
        [("gcc", "c", "-std=c99"), ("g++", "c++", "-std=c++11")]
    {
        let ran = Command::new(compiler)
            .args([
                "-x",
                language,
                standard,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-pedantic",
                "-c",
            ])
            .arg("-I")
            .arg(here().join("include"))
            .arg(&source)
            .arg("-o")
            .arg(dir.path().join("only.o"))
            .output();
        match ran {
            Ok(ran) => assert!(
                ran.status.success(),
                "{compiler} {standard}:\n{}",
                String::from_utf8_lossy(&ran.stderr)
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!(
                    "there is no {compiler}, so the header is not compiled \
                    as {language}"
                )
            }
            Err(e) => panic!("{compiler} could not be run: {e}"),
        }
    }
}
