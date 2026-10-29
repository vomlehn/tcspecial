//! An armed RESTART runs tcspecial again in the same process, keeping the
//! descriptors it was started with and none it opened itself.
//!
//! Through tcspecial itself, as the ground commands it: armed, restarted, and
//! watched by its beacons, which start again at #1 when it comes back. Started
//! with one end of a pipe beyond the standard three, which is what a
//! supervisor hands a process, and which the process that comes back must
//! still hold.

use std::net::UdpSocket;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tcslibgs::{
    ArmKey, Beacon, CommandStatus, RestartArmCommand, RestartCommand, Telemetry,
};

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
    let before = beacon(&beacons);
    assert!(before.sequence >= 2, "beacon #{}", before.sequence);
    assert!(!before.commanded_restart, "nobody commanded a first start");
    let open_before = descriptors(pid);
    assert!(
        open_before.contains(&handed),
        "the handed descriptor, {handed}, is not open in tcspecial: \
         {open_before:?}"
    );

    let key = ArmKey(0x5eed);
    assert_eq!(
        ask(
            &commands,
            take_at,
            &tcslibgs::Command::RestartArm(RestartArmCommand::new(1, key))
        ),
        CommandStatus::Success
    );
    assert_eq!(
        ask(
            &commands,
            take_at,
            &tcslibgs::Command::Restart(RestartCommand::new(2, key))
        ),
        CommandStatus::Success
    );

    // The old start's beacons carry the commanded-restart bit from the
    // moment the RESTART is taken; the new start's number from one again,
    // and the first of them already carries it too.
    let by = Instant::now() + Duration::from_secs(10);
    let mut last = before.sequence;
    let mut ending = false;
    let back = loop {
        assert!(Instant::now() < by, "tcspecial did not come back");
        let heard = beacon(&beacons);
        if heard.sequence > last {
            // One sent before the RESTART was taken may still be arriving;
            // once one carries the bit, every one after it does.
            assert!(
                heard.commanded_restart || !ending,
                "the old start, ending"
            );
            ending |= heard.commanded_restart;
            last = heard.sequence;
            continue;
        }
        break heard;
    };
    assert!(ending, "the old start's last beacons carried the bit");
    assert_eq!(back.sequence, 1, "the first beacon of the new start");
    assert!(back.commanded_restart, "the start the RESTART asked for");
    // Said once, and no more: a crash of this start would not be commanded.
    let kept = std::fs::read_to_string(&state).expect("the state file");
    assert!(kept.contains("\ngeneration 2\n"), "{kept}");
    assert!(!kept.contains("next_start"), "{kept}");
    // And settled again, as it was when it was counted the first time.
    beacon(&beacons);
    beacon(&beacons);

    // A CONNECT clears the bit: the first beacon after its answer may have
    // been on its way, and the one after does not carry it. Whatever the
    // digests say, the ground has connected.
    let connect = tcslibgs::Command::Connect(tcslibgs::ConnectCommand::new(
        3,
        tcslibgs::ConfigVersion::of_this_build(),
        tcslibgs::ConfigDigest([0; 16]),
    ));
    assert_eq!(ask(&commands, take_at, &connect), CommandStatus::Success);
    beacon(&beacons);
    assert!(
        !beacon(&beacons).commanded_restart,
        "cleared by the CONNECT"
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
