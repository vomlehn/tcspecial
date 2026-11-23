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
//! Python 3. A second test is the ground through tcsmoc-c instead, with the
//! C ground station `tcsmoc-c/examples/consolidator-station.c` built as C
//! and as C++.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tcsos::net::SocketAddr;
use tcsos::process::{Command, Stdio};
use tcsos::time::{Duration, Instant};

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
    sensors: &tcsos::net::UdpSocket,
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

/// The consolidator example, moved so that it runs beside anything else.
struct Example {
    /// The moved configuration, in the test's directory.
    configuration: PathBuf,
    /// The port the sun's sensor is read at.
    sun: u16,
    /// The port the wheel's sensor is read at.
    wheel: u16,
    /// The port the battery's sensor is read at.
    battery: u16,
    /// The port tcspecial takes commands at.
    take_at: u16,
}

/// Writes the consolidator example into `dir`, each of its sockets moved to
/// a port nothing holds, and its log and state into `dir`.
///
/// # Arguments
///
/// * `dir`      - The test's directory.
/// * `commands` - The port the ground sends its commands from.
///
/// # Returns
///
/// Where the configuration is, and the ports a test needs.
fn example(dir: &Path, commands: u16) -> Example {
    let example = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../examples/consolidator/consolidator.ytcs");
    let mut text = common::on_one_host(
        &tcsos::fs::read_to_string(&example).expect("the example reads"),
    );
    // tcspecial writes its log into a directory that is there, and does not
    // make one.
    tcsos::fs::create_dir(dir.join("telemetry")).expect("the log's directory");

    let (sun, wheel, battery, base) = (free(), free(), free(), free());
    // Where tcs_cmd.recv says tcspecial takes the ground's commands.
    let take_at = free();
    let moves = [
        (":5001\"", format!(":{sun}\"")),
        (":5002\"", format!(":{wheel}\"")),
        (":5003\"", format!(":{battery}\"")),
        ("127.0.0.1:5100", format!("127.0.0.1:{base}")),
        ("127.0.0.1:5650", format!("127.0.0.1:{}", free())),
        ("127.0.0.1:5550", format!("127.0.0.1:{}", free())),
        ("127.0.0.1:4200", format!("127.0.0.1:{}", free())),
        ("127.0.0.1:4300", format!("127.0.0.1:{}", free())),
        ("127.0.0.1:6100", format!("127.0.0.1:{}", free())),
        ("127.0.0.1:6000", format!("127.0.0.1:{}", free())),
        ("127.0.0.1:4000", format!("127.0.0.1:{take_at}")),
        ("127.0.0.1:4100", format!("127.0.0.1:{commands}")),
        (
            "/tmp/telemetry/consolidator",
            dir.join("telemetry").display().to_string(),
        ),
        (
            "/tmp/tcspecial-consolidator.state",
            dir.join("state").display().to_string(),
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
    ];
    text = moved(&text, &moves);
    let configuration = dir.join("consolidator.ytcs");
    tcsos::fs::write(&configuration, &text).expect("written");
    Example {
        configuration,
        sun,
        wheel,
        battery,
        take_at,
    }
}

/// Starts tcspecial on a configuration, quietly.
///
/// # Arguments
///
/// * `configuration` - The configuration it flies.
///
/// # Returns
///
/// tcspecial, stopped when this is dropped.
fn flying(configuration: &Path) -> common::Flying {
    common::Flying::quiet(
        Command::new(env!("CARGO_BIN_EXE_tcspecial"))
            .arg(configuration)
            .arg("--quiet")
            .arg("--simulate=all")
            .arg("--no-memlock")
            .env_remove("RUST_LOG")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("tcspecial starts"),
    )
}

/// A port found free that is one the example names is moved once, not
/// twice: 4300, written for 5650, is not taken for the example's own 4300,
/// which `example` once did, failing now and then.
#[test]
fn a_free_port_the_example_names_is_not_moved_again() {
    let text = "a: \"127.0.0.1:5650\"\nb: \"127.0.0.1:4300\"\n";
    assert_eq!(
        moved(
            text,
            &[
                ("127.0.0.1:5650", "127.0.0.1:4300"),
                ("127.0.0.1:4300", "127.0.0.1:4301"),
            ],
        ),
        "a: \"127.0.0.1:4300\"\nb: \"127.0.0.1:4301\"\n"
    );
}

#[test]
fn the_datum_carries_defaults_then_readings() {
    let dir = tempfile::tempdir().expect("a directory");
    // The ground's command link: where tcs_cmd.send says it sends from.
    let commands = bound();
    let Example {
        configuration,
        sun,
        wheel,
        battery,
        take_at,
    } = example(dir.path(), port(&commands));

    // The ground, as the example's is: the gateway node's one port, for
    // housekeeping's datum, handed out whole.
    let graph = Graph::load(&configuration).expect("it reads");
    let mut ground =
        GatewayReceiver::open(&graph, "gateway").expect("the ground opens");
    assert_eq!(ground.sources(), ["housekeeping"]);

    let _flying = flying(&configuration);

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

/// The path cargo says it built a file at, from what `cargo build
/// --message-format=json` wrote: the quoted path among its artifacts that
/// ends in the file's name.
///
/// Asked of cargo rather than worked out from where this test is, which is
/// a guess about cargo's layout: cargo 1.100 moved the tests out of the
/// directory the static library is in, and the guess named a directory the
/// library was never in.
///
/// # Arguments
///
/// * `json` - What cargo wrote.
/// * `file` - The file's name, such as `libtcsmoc_c.a`.
///
/// # Returns
///
/// Its path; none where cargo reported no such file.
fn reported(json: &str, file: &str) -> Option<PathBuf> {
    let end = format!("/{file}\"");
    json.match_indices(&end).find_map(|(at, _)| {
        let start = json[..at].rfind('"')? + 1;
        Some(PathBuf::from(&json[start..at + end.len() - 1]))
    })
}

/// Builds `libtcsmoc_c.a` from the source as it is now, as tcsmoc-c's own
/// tests do (`tcsmoc-c/tests/c_and_cpp.rs`), and says where it is: where
/// cargo says it put it. Release where this test was built without debug
/// assertions, which only the release profile leaves out.
///
/// # Returns
///
/// The library's path.
///
/// # Panics
///
/// If cargo cannot be run, the library does not build, or cargo reports
/// no library.
fn tcsmoc_c_library() -> PathBuf {
    let mut build = Command::new(env!("CARGO"));
    build.args([
        "build",
        "--quiet",
        "-p",
        "tcsmoc-c",
        "--lib",
        "--message-format=json",
    ]);
    if !cfg!(debug_assertions) {
        build.arg("--release");
    }
    let built = build.output().expect("cargo runs");
    assert!(
        built.status.success(),
        "tcsmoc-c does not build:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );
    reported(&String::from_utf8_lossy(&built.stdout), "libtcsmoc_c.a")
        .expect("cargo says where it put libtcsmoc_c.a")
}

/// Compiles the C ground station against `tcsmoc-c.h` and the library.
///
/// # Arguments
///
/// * `compiler` - "gcc" or "g++".
/// * `standard` - The language standard's flag, such as "-std=c11".
/// * `library`  - The `libtcsmoc_c.a` to link with.
/// * `out`      - Where the program is written.
///
/// # Returns
///
/// The program's path, or `None` where there is no such compiler.
///
/// # Panics
///
/// If the compiler cannot be run for another reason, or fails.
fn station(
    compiler: &str,
    standard: &str,
    library: &Path,
    out: &Path,
) -> Option<PathBuf> {
    let tcsmoc_c = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tcsmoc-c");
    let ran = Command::new(compiler)
        .args([standard, "-Wall", "-Wextra", "-Werror", "-pedantic"])
        .arg(tcsmoc_c.join("examples/consolidator-station.c"))
        .arg("-I")
        .arg(tcsmoc_c.join("include"))
        .arg(library)
        .args(["-lpthread", "-ldl", "-lm", "-o"])
        .arg(out)
        .output();
    let ran = match ran {
        Ok(ran) => ran,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("there is no {compiler}, so the station is not built");
            return None;
        }
        Err(e) => panic!("{compiler} could not be run: {e}"),
    };
    assert!(
        ran.status.success(),
        "{compiler}:\n{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    Some(out.to_path_buf())
}

/// Runs a program to its end, killing it if it has not ended in `most`.
///
/// # Arguments
///
/// * `command` - The program, its arguments given.
/// * `most`    - How long it may take.
///
/// # Returns
///
/// What it printed, and how it ended.
fn ran_within(command: &mut Command, most: Duration) -> tcsos::process::Output {
    let child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("it starts");
    let pid = child.id();
    let ended = Arc::new(AtomicBool::new(false));
    let watch = {
        let ended = ended.clone();
        tcsos::thread::spawn(move || {
            let deadline = Instant::now() + most;
            while Instant::now() < deadline {
                if ended.load(Ordering::SeqCst) {
                    return;
                }
                tcsos::thread::sleep(Duration::from_millis(100));
            }
            // The child this test started, which has not been waited for,
            // so its id is still its own.
            let _ = tcsos::process::kill(pid);
        })
    };
    let output = child.wait_with_output().expect("it is waited for");
    ended.store(true, Ordering::SeqCst);
    let _ = watch.join();
    output
}

/// The C ground station, built as C and as C++, takes the consolidator's
/// data as tcspecial sends it: it hears the beacon, connects, is matched,
/// and takes each datum apart through tcsmoc-c.h into the three sensors'
/// readings, in order, and the battery's age.
#[test]
fn a_c_ground_station_takes_the_datum_apart() {
    let library = tcsmoc_c_library();
    let dir = tempfile::tempdir().expect("a directory");
    let Example {
        configuration,
        sun,
        wheel,
        battery,
        ..
    } = example(dir.path(), free());
    let _flying = flying(&configuration);

    // The three sensors, from a thread until the stations are done: the sun
    // 0x0304, the wheel -7 and the battery 0x55.
    let done = Arc::new(AtomicBool::new(false));
    let sensors = {
        let done = done.clone();
        tcsos::thread::spawn(move || {
            let sensors = bound();
            let at = |port: u16| -> SocketAddr {
                format!("127.0.0.1:{port}").parse().expect("an address")
            };
            let readings: [(SocketAddr, &[u8]); 3] = [
                (at(sun), &[0x03, 0x04]),
                (at(wheel), &[0xff, 0xff, 0xff, 0xf9]),
                (at(battery), &[0x55]),
            ];
            while !done.load(Ordering::SeqCst) {
                for (at, reading) in &readings {
                    sensors.send_to(reading, at).expect("sent");
                }
                tcsos::thread::sleep(Duration::from_millis(200));
            }
        })
    };

    for (compiler, standard) in [("gcc", "-std=c11"), ("g++", "-std=c++17")] {
        let Some(program) = station(
            compiler,
            standard,
            &library,
            &dir.path().join(format!("station_{compiler}")),
        ) else {
            continue;
        };
        let ran = ran_within(
            Command::new(&program).arg(&configuration).args([
                "gateway",
                "housekeeping",
                "6",
            ]),
            Duration::from_secs(60),
        );
        let said = String::from_utf8_lossy(&ran.stdout);
        assert!(
            ran.status.success(),
            "{compiler}: {}\n{said}{}",
            ran.status,
            String::from_utf8_lossy(&ran.stderr)
        );
        let lines: Vec<&str> = said.lines().collect();
        assert!(lines[0].starts_with("beacon: tcspecial v"), "{said}");
        assert_eq!(lines[1..3], ["connect: agrees", "matched"], "{said}");
        let datums = &lines[3..];
        assert_eq!(datums.len(), 6, "{compiler}: {said}");
        assert!(
            datums.iter().all(|line| line.starts_with("datum: sun=")),
            "{compiler}: every datum takes apart: {said}"
        );
        // Once the battery's age has come, every member is a reading.
        assert!(
            datums.iter().any(|line| {
                line.strip_prefix(
                    "datum: sun=772 wheel=-7 battery=85 battery_age=",
                )
                .is_some_and(|age| age != "65535")
            }),
            "{compiler}: {said}"
        );
    }
    done.store(true, Ordering::SeqCst);
    sensors.join().expect("the sensors end");
}
