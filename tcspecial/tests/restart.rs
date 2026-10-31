//! An armed RESTART runs tcspecial again in the same process, keeping the
//! descriptors it was started with and none it opened itself.
//!
//! Through tcspecial itself, as the ground commands it: armed, restarted, and
//! watched by its beacons, which start again at #1 when it comes back, each
//! start with the restart flag set until a CONNECT carrying the
//! configuration's checksum. Started
//! with one end of a pipe beyond the standard three, which is what a
//! supervisor hands a process, and which the process that comes back must
//! still hold.

use std::net::UdpSocket;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tcslibgs::{
    ArmKey, Beacon, CommandStatus, RestartArmCommand, RestartCommand, Telemetry,
};

mod common;

/// Binds a socket on a loopback port nothing else holds, kept so that it
/// stays that way.
///
/// # Returns
///
/// The socket, with a read timeout of five seconds.
///
/// # Panics
///
/// If no port can be bound or the timeout cannot be set.
fn socket() -> UdpSocket {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("a socket");
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("a timeout");
    socket
}

/// A port nothing holds: the configuration check refuses port 0.
fn free() -> u16 {
    socket().local_addr().expect("a port").port()
}

/// tcspecial, stopped however the test ends.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        // SAFETY: a signal to a child this test started.
        unsafe { libc::kill(self.0.id() as libc::pid_t, libc::SIGTERM) };
        let _ = self.0.wait();
    }
}

/// Lists the descriptors a process has open, from `/proc`.
///
/// # Arguments
///
/// * `pid` - The process's ID.
///
/// # Returns
///
/// Its open descriptors, in ascending order.
///
/// # Panics
///
/// If the process's descriptors cannot be read.
fn descriptors(pid: u32) -> Vec<i32> {
    let mut open: Vec<i32> = std::fs::read_dir(format!("/proc/{pid}/fd"))
        .expect("the process's descriptors")
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    open.sort_unstable();
    open
}

/// Waits for the next beacon.
///
/// # Arguments
///
/// * `ground` - The socket the beacons arrive at.
///
/// # Returns
///
/// The beacon.
///
/// # Panics
///
/// If the read fails or times out, or what arrives is not a beacon.
fn beacon(ground: &UdpSocket) -> Beacon {
    let mut buffer = [0u8; 65535];
    loop {
        match ground.recv_from(&mut buffer) {
            Ok((size, _)) => match Beacon::read(&buffer[..size]) {
                Ok(beacon) => return beacon,
                Err(why) => panic!("expected a beacon: {why}"),
            },
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {
                continue
            }
            Err(e) => panic!("no beacon: {e}"),
        }
    }
}

/// Sends a command and reads its answer's status.
///
/// # Arguments
///
/// * `ground`  - The socket to send from, where the answer arrives.
/// * `at`      - The loopback port tcspecial takes commands on.
/// * `command` - The command to send.
///
/// # Returns
///
/// The status in tcspecial's answer.
///
/// # Panics
///
/// If the send or the read fails, or the answer is not telemetry.
fn ask(
    ground: &UdpSocket,
    at: u16,
    command: &tcslibgs::Command,
) -> CommandStatus {
    let asked = command.to_bytes();
    ground
        .send_to(&asked, ("127.0.0.1", at))
        .expect("it is sent");
    let mut buffer = [0u8; 65535];
    loop {
        match ground.recv_from(&mut buffer) {
            Ok((size, _)) => {
                return Telemetry::read(&buffer[..size])
                    .expect("telemetry")
                    .status()
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {
                continue
            }
            Err(e) => panic!("no answer: {e}"),
        }
    }
}

#[test]
fn an_armed_restart_runs_tcspecial_again_keeping_what_it_was_given() {
    let dir = tempfile::tempdir().expect("a directory");
    let beacons = socket();
    let commands = socket();
    let commands_at = commands.local_addr().expect("where").port();
    let take_at = free();

    let configuration = dir.path().join("vehicle.yaml");
    let state = dir.path().join("tcspecial.state").display().to_string();
    let at = || format!("127.0.0.1:{}", free());
    std::fs::write(
        &configuration,
        format!(
            "tcspecial:
  gateway: radio
  interface: \"ad9aac4dd5a6d88dd5e78824ba8a83ac\"
  beacon: {{ default_interval: \"100 ms\", reset_interval: \"100 ms\", \
  recv: \"127.0.0.1:{}\", send: \"{}\" }}
  payload_command: {{ dest_base: \"{}\", source_base: \"{}\" }}
  payload_telemetry: {{ dest_base: \"{}\", source_base: \"{}\" }}
  state_file: \"{state}\"
  tcs_cmd: {{ recv: \"127.0.0.1:{take_at}\", \
  send: \"127.0.0.1:{commands_at}\" }}
",
            beacons.local_addr().expect("where").port(),
            at(),
            at(),
            at(),
            at(),
            at(),
        ),
    )
    .expect("written");

    // A pipe whose write end tcspecial inherits, as a process inherits what
    // its supervisor hands it: made without close-on-exec, which is what
    // makes it inherited.
    let mut ends = [0 as libc::c_int; 2];
    // SAFETY: a pipe into an array of two.
    assert_eq!(unsafe { libc::pipe(ends.as_mut_ptr()) }, 0, "a pipe");
    let handed = ends[1];

    let vehicle = Running(
        Command::new(env!("CARGO_BIN_EXE_tcspecial"))
            .arg(&configuration)
            .arg("--quiet")
            .arg("--simulate=all")
            .env_remove("RUST_LOG")
            .stdout(Stdio::null())
            .spawn()
            .expect("tcspecial runs"),
    );
    let pid = vehicle.0.id();
    // SAFETY: this test's own descriptors, no longer needed here.
    unsafe {
        libc::close(ends[0]);
        libc::close(ends[1]);
    }

    // Running, and settled: a second beacon is sent after everything opened.
    beacon(&beacons);
    let first = beacon(&beacons);
    assert!(first.sequence >= 2, "beacon #{}", first.sequence);
    assert!(first.restart_flag, "a first start begins with the flag set");

    // A CONNECT carrying another configuration's checksum is answered and
    // changes nothing: the flag stays set, and a PING is not acted on.
    let connect = |sequence, digest| {
        tcslibgs::Command::Connect(tcslibgs::ConnectCommand::new(
            sequence, digest,
        ))
    };
    let ours = common::checksum_of(&configuration);
    assert_eq!(
        ask(
            &commands,
            take_at,
            &connect(1, tcslibgs::ConfigDigest([0; 16]))
        ),
        CommandStatus::Success
    );
    let ping = |sequence| {
        tcslibgs::Command::Ping(tcslibgs::PingCommand::new(sequence))
    };
    assert_eq!(ask(&commands, take_at, &ping(2)), CommandStatus::Unmatched);
    beacon(&beacons);
    assert!(
        beacon(&beacons).restart_flag,
        "a mismatched CONNECT clears nothing"
    );

    // One carrying this configuration's clears it: the first beacon after
    // its answer may have been on its way, and the one after does not carry
    // it. And the PING is taken.
    assert_eq!(
        ask(&commands, take_at, &connect(3, ours)),
        CommandStatus::Success
    );
    beacon(&beacons);
    let before = beacon(&beacons);
    assert!(!before.restart_flag, "cleared by the matching CONNECT");
    assert_eq!(ask(&commands, take_at, &ping(4)), CommandStatus::Success);
    let open_before = descriptors(pid);
    assert!(
        open_before.contains(&handed),
        "the handed descriptor, {handed}, is not open in tcspecial: \
         {open_before:?}"
    );

    // A restart onto a configuration the vehicle does not hold is refused
    // NotFound, and nothing restarts; onto the running one, it is a plain
    // restart.
    let key = ArmKey(0x5eed);
    let elsewhere = tcslibgs::ConfigDigest([0x77; 16]);
    assert_eq!(
        ask(
            &commands,
            take_at,
            &tcslibgs::Command::RestartArm(RestartArmCommand::new(
                5, key, elsewhere
            ))
        ),
        CommandStatus::NotFound
    );
    assert_eq!(
        ask(
            &commands,
            take_at,
            &tcslibgs::Command::RestartArm(RestartArmCommand::new(
                6, key, ours
            ))
        ),
        CommandStatus::Success
    );
    assert_eq!(
        ask(
            &commands,
            take_at,
            &tcslibgs::Command::Restart(RestartCommand::new(7, key, ours))
        ),
        CommandStatus::Success
    );

    // The old start's beacons carry the restart flag from the moment the
    // RESTART is taken; the new start's number from one again, and the
    // first of them already carries it too.
    let by = Instant::now() + Duration::from_secs(10);
    let mut last = before.sequence;
    let mut ending = false;
    let back = loop {
        assert!(Instant::now() < by, "tcspecial did not come back");
        let heard = beacon(&beacons);
        if heard.sequence > last {
            // One sent before the RESTART was taken may still be arriving;
            // once one carries the flag, every one after it does.
            assert!(heard.restart_flag || !ending, "the old start, ending");
            ending |= heard.restart_flag;
            last = heard.sequence;
            continue;
        }
        break heard;
    };
    assert!(ending, "the old start's last beacons carried the flag");
    assert_eq!(back.sequence, 1, "the first beacon of the new start");
    assert!(back.restart_flag, "every start begins with the flag set");
    // Said once, and no more: a crash of this start would not be commanded.
    let kept = std::fs::read_to_string(&state).expect("the state file");
    assert!(!kept.contains("generation"), "{kept}");
    assert!(!kept.contains("next_start"), "{kept}");
    // And settled again, as it was when it was counted the first time.
    beacon(&beacons);
    beacon(&beacons);

    // The new start is matched as the first was.
    assert_eq!(
        ask(&commands, take_at, &connect(8, ours)),
        CommandStatus::Success
    );
    beacon(&beacons);
    assert!(
        !beacon(&beacons).restart_flag,
        "cleared by the matching CONNECT"
    );

    assert_eq!(
        vehicle.0.id(),
        pid,
        "the same process, as a supervisor sees it"
    );
    let open_after = descriptors(pid);
    assert!(
        open_after.contains(&handed),
        "the handed descriptor, {handed}, did not survive the restart: \
         {open_after:?}"
    );
    assert_eq!(
        open_after.len(),
        open_before.len(),
        "what was opened before the restart should have been closed: before \
         {open_before:?}, after {open_after:?}"
    );
}

/// Writes a configuration whose links are all on loopback, its command link
/// taking commands at `take_at` from `commands_at` and its beacons going to
/// `beacons_at`, keeping configurations in `kept`; `arm` is its
/// `restart_arm_valid`, which is what tells two of them apart.
///
/// # Arguments
///
/// * `path`        - Where to write it.
/// * `state`       - The state file it names.
/// * `kept`        - The directory it keeps configurations in.
/// * `take_at`     - Where it takes commands.
/// * `commands_at` - Where the ground sends them from.
/// * `beacons_at`  - Where its beacons go.
/// * `arm`         - Its `restart_arm_valid`.
///
/// # Panics
///
/// If it cannot be written.
fn written(
    path: &std::path::Path,
    state: &std::path::Path,
    kept: &std::path::Path,
    take_at: u16,
    commands_at: u16,
    beacons_at: u16,
    arm: &str,
) {
    let at = || format!("127.0.0.1:{}", free());
    std::fs::write(
        path,
        format!(
            "tcspecial:
  gateway: radio
  interface: \"{}\"
  beacon: {{ default_interval: \"100 ms\", reset_interval: \"100 ms\", \
  recv: \"127.0.0.1:{beacons_at}\", send: \"{}\" }}
  payload_command: {{ dest_base: \"{}\", source_base: \"{}\" }}
  payload_telemetry: {{ dest_base: \"{}\", source_base: \"{}\" }}
  state_file: \"{}\"
  configurations: \"{}\"
  restart_arm_valid: \"{arm}\"
  tcs_cmd: {{ recv: \"127.0.0.1:{take_at}\", \
  send: \"127.0.0.1:{commands_at}\" }}
",
            tcslibgs::InterfaceDigest::of_this_build(),
            at(),
            at(),
            at(),
            at(),
            at(),
            state.display(),
            kept.display(),
        ),
    )
    .expect("written");
}

/// Waits for a beacon of a new start, whose number has gone back below
/// the old start's, skipping the old start's last beacons.
///
/// # Arguments
///
/// * `beacons` - Where the beacons arrive.
/// * `last`    - The last beacon number heard of the old start.
///
/// # Returns
///
/// The new start's first beacon heard.
///
/// # Panics
///
/// If no new start is heard within ten seconds.
fn next_start(beacons: &UdpSocket, mut last: u32) -> Beacon {
    let by = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < by, "tcspecial did not come back");
        let heard = beacon(beacons);
        if heard.sequence > last {
            last = heard.sequence;
            continue;
        }
        return heard;
    }
}

/// A RESTART naming a configuration kept on board runs that configuration:
/// its beacons carry its checksum, and a CONNECT carrying it matches it.
/// One naming a configuration the vehicle does not hold is refused
/// `NotFound`, and one whose configuration has gone between the arm and the
/// restart is refused `NotFound` too, and nothing restarts. A matching
/// CONNECT confirms what runs, so the state file names it.
#[test]
fn a_restart_runs_the_configuration_it_names() {
    let dir = tempfile::tempdir().expect("a directory");
    let beacons = socket();
    let commands = socket();
    let commands_at = commands.local_addr().expect("where").port();
    let beacons_at = beacons.local_addr().expect("where").port();
    let take_at = free();
    let kept = dir.path().join("kept");
    std::fs::create_dir(&kept).expect("a store");
    let state = dir.path().join("tcspecial.state");

    let first = dir.path().join("first.yaml");
    let second = dir.path().join("second.yaml");
    for (path, arm) in [(&first, "20 min"), (&second, "30 min")] {
        written(path, &state, &kept, take_at, commands_at, beacons_at, arm);
    }
    let ours = common::checksum_of(&first);
    let theirs = common::checksum_of(&second);
    assert_ne!(ours, theirs, "two configurations");
    let store = tcslibgs::store::Store::at(&kept);
    store.keep(&second).expect("kept");

    let _vehicle = Running(
        Command::new(env!("CARGO_BIN_EXE_tcspecial"))
            .arg(&first)
            .arg("--quiet")
            .arg("--simulate=all")
            .env_remove("RUST_LOG")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("tcspecial runs"),
    );
    let started = beacon(&beacons);
    assert_eq!(started.digest, ours, "the command line's, at a first start");

    let connect = |sequence, digest| {
        tcslibgs::Command::Connect(tcslibgs::ConnectCommand::new(
            sequence, digest,
        ))
    };
    let arm = |sequence, digest| {
        tcslibgs::Command::RestartArm(RestartArmCommand::new(
            sequence,
            ArmKey(0x5eed),
            digest,
        ))
    };
    let restart = |sequence, digest| {
        tcslibgs::Command::Restart(RestartCommand::new(
            sequence,
            ArmKey(0x5eed),
            digest,
        ))
    };
    assert_eq!(
        ask(&commands, take_at, &connect(1, ours)),
        CommandStatus::Success
    );
    let elsewhere = tcslibgs::ConfigDigest([0x77; 16]);
    assert_eq!(
        ask(&commands, take_at, &arm(2, elsewhere)),
        CommandStatus::NotFound,
        "a configuration the vehicle does not hold"
    );

    // Armed onto the second, which then goes from the store: the RESTART
    // checks it again and refuses it, and nothing restarts.
    assert_eq!(
        ask(&commands, take_at, &arm(3, theirs)),
        CommandStatus::Success
    );
    let kept_at = store.find(theirs).expect("it is kept");
    std::fs::remove_file(&kept_at).expect("removed");
    assert_eq!(
        ask(&commands, take_at, &restart(4, theirs)),
        CommandStatus::NotFound,
        "a configuration gone since the arm"
    );
    beacon(&beacons);
    let still = beacon(&beacons);
    assert_eq!(still.digest, ours, "nothing restarted");
    assert!(!still.restart_flag, "and the match stands");

    // Kept again, and the arm standing: the RESTART is taken, and the start
    // that comes back runs the second.
    store.keep(&second).expect("kept again");
    assert_eq!(
        ask(&commands, take_at, &restart(5, theirs)),
        CommandStatus::Success
    );
    let back = next_start(&beacons, still.sequence);
    assert_eq!(back.digest, theirs, "the configuration the RESTART named");
    assert!(back.restart_flag);
    let said = std::fs::read_to_string(&state).expect("the state file");
    assert!(
        said.contains(&format!("next {theirs}\ntrials 1\n")),
        "{said}"
    );
    assert!(said.contains(&format!("confirmed {ours}\n")), "{said}");

    // Matched with its own checksum, which confirms it.
    assert_eq!(
        ask(&commands, take_at, &connect(6, ours)),
        CommandStatus::Success,
        "answered, and matching nothing"
    );
    assert_eq!(
        ask(&commands, take_at, &connect(7, theirs)),
        CommandStatus::Success
    );
    beacon(&beacons);
    assert!(!beacon(&beacons).restart_flag, "matched");
    let said = std::fs::read_to_string(&state).expect("the state file");
    assert!(
        said.contains(&format!("next {theirs}\ntrials 0\n")),
        "{said}"
    );
    assert!(said.contains(&format!("confirmed {theirs}\n")), "{said}");
}

/// A SIGTERM that arrives after a RESTART is taken and before the process
/// runs itself again stops the vehicle rather than being lost: the process
/// ends, with success, and no start comes back. The state file names the
/// RESTART's target for the next start, as a commanded one.
#[test]
fn a_stop_during_a_restart_stops_the_vehicle() {
    let dir = tempfile::tempdir().expect("a directory");
    let beacons = socket();
    let commands = socket();
    let commands_at = commands.local_addr().expect("where").port();
    let beacons_at = beacons.local_addr().expect("where").port();
    let take_at = free();
    let kept = dir.path().join("kept");
    std::fs::create_dir(&kept).expect("a store");
    let state = dir.path().join("tcspecial.state");
    let configuration = dir.path().join("vehicle.yaml");
    written(
        &configuration,
        &state,
        &kept,
        take_at,
        commands_at,
        beacons_at,
        "20 min",
    );
    let ours = common::checksum_of(&configuration);

    let mut vehicle = Command::new(env!("CARGO_BIN_EXE_tcspecial"))
        .arg(&configuration)
        .arg("--quiet")
        .arg("--simulate=all")
        .env_remove("RUST_LOG")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("tcspecial runs");
    let pid = vehicle.id() as libc::pid_t;
    let first = beacon(&beacons);
    assert_eq!(first.digest, ours);

    let key = ArmKey(0x5eed);
    assert_eq!(
        ask(
            &commands,
            take_at,
            &tcslibgs::Command::RestartArm(RestartArmCommand::new(
                1, key, ours
            ))
        ),
        CommandStatus::Success
    );
    assert_eq!(
        ask(
            &commands,
            take_at,
            &tcslibgs::Command::Restart(RestartCommand::new(2, key, ours))
        ),
        CommandStatus::Success
    );
    // At once, as the restart is under way: the answer goes before the
    // vehicle stops to run itself again.
    // SAFETY: a signal to a child this test started.
    unsafe { libc::kill(pid, libc::SIGTERM) };

    let by = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = vehicle.try_wait().expect("it can be asked") {
            break Some(status);
        }
        if Instant::now() > by {
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let Some(status) = status else {
        let _ = vehicle.kill();
        let _ = vehicle.wait();
        panic!("the stop was lost: tcspecial restarted rather than stopping");
    };
    assert!(status.success(), "it stopped as a stop stops it: {status}");

    // No start came back: nothing beacons after it has gone.
    beacons
        .set_read_timeout(Some(Duration::from_millis(500)))
        .expect("a timeout");
    let mut buffer = [0u8; 256];
    while let Ok((size, _)) = beacons.recv_from(&mut buffer) {
        let heard = Beacon::read(&buffer[..size]).expect("a beacon");
        assert!(
            heard.sequence > first.sequence,
            "a start came back: beacon #{}",
            heard.sequence
        );
    }
    let said = std::fs::read_to_string(&state).expect("the state file");
    assert!(said.contains(&format!("next {ours}\ntrials 0\n")), "{said}");
    assert!(said.contains("next_start commanded"), "{said}");
}
