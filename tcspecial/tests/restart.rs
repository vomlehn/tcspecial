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

/// A socket on a port nothing else holds, kept so that it stays that way.
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

/// The descriptors process `pid` has open.
fn descriptors(pid: u32) -> Vec<i32> {
    let mut open: Vec<i32> = std::fs::read_dir(format!("/proc/{pid}/fd"))
        .expect("the process's descriptors")
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    open.sort_unstable();
    open
}

/// The next beacon's number.
fn beacon(ground: &UdpSocket) -> u32 {
    let mut buffer = [0u8; 65535];
    loop {
        match ground.recv_from(&mut buffer) {
            Ok((size, _)) => match Beacon::read(&buffer[..size]) {
                Ok(beacon) => return beacon.sequence,
                Err(why) => panic!("expected a beacon: {why}"),
            },
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {
                continue
            }
            Err(e) => panic!("no beacon: {e}"),
        }
    }
}

/// Send `command` from `ground` to `at` and read its answer's status.
fn ask(
    ground: &UdpSocket,
    at: u16,
    command: &tcslibgs::Command,
) -> CommandStatus {
    let asked = serde_json::to_vec(command).expect("a command");
    ground
        .send_to(&asked, ("127.0.0.1", at))
        .expect("it is sent");
    let mut buffer = [0u8; 65535];
    loop {
        match ground.recv_from(&mut buffer) {
            Ok((size, _)) => {
                return serde_json::from_slice::<Telemetry>(&buffer[..size])
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
    let at = || format!("127.0.0.1:{}", free());
    std::fs::write(
        &configuration,
        format!(
            "tcspecial:
  gateway: radio
  beacon: {{ interval: \"100 ms\", recv: \"127.0.0.1:{}\", send: \"{}\" }}
  payload_command: {{ dest_base: \"{}\", source_base: \"{}\" }}
  payload_telemetry: {{ dest_base: \"{}\", source_base: \"{}\" }}
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
    assert!(before >= 2, "beacon #{before}");
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

    // Back, numbering its beacons from one again.
    let by = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < by, "tcspecial did not come back");
        if beacon(&beacons) == 1 {
            break;
        }
    }
    // And settled again, as it was when it was counted the first time.
    beacon(&beacons);
    beacon(&beacons);

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
