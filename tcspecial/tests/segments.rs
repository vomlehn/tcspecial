//! Every filled telemetry log segment is handed to `segsend`, so the log
//! directory does not fill as the vehicle runs.
//!
//! Through tcspecial itself and the `segsend` built beside it, which deletes
//! what it is given: beacons a few milliseconds apart into segments a few
//! hundred bytes long fill a segment every handful of beacons, and the
//! directory should hold no more than the segment being written and the
//! one, perhaps, that `segsend` is dealing with.

use std::net::UdpSocket;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Finds a loopback port nothing holds: the configuration check refuses
/// port 0, a link nobody can predict the port of.
///
/// # Returns
///
/// The port.
///
/// # Panics
///
/// If no port can be bound.
fn free() -> u16 {
    UdpSocket::bind("127.0.0.1:0")
        .and_then(|socket| socket.local_addr())
        .expect("a port")
        .port()
}

/// tcspecial, stopped however the test ends: a vehicle left running would
/// hold its ports into the next test.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        // SAFETY: a signal to a child this test started.
        unsafe { libc::kill(self.0.id() as libc::pid_t, libc::SIGTERM) };
        let _ = self.0.wait();
    }
}

#[test]
fn filled_segments_are_handed_to_segsend_and_leave_the_directory() {
    let dir = tempfile::tempdir().expect("a directory");
    let logs = dir.path().join("telemetry");
    std::fs::create_dir(&logs).expect("the log directory");
    let configuration = dir.path().join("vehicle.yaml");
    let state = dir.path().join("tcspecial.state").display().to_string();
    let at = || format!("127.0.0.1:{}", free());
    std::fs::write(
        &configuration,
        format!(
            "tcspecial:
  gateway: radio
  beacon: {{ default_interval: \"5 ms\", reset_interval: \"5 ms\", \
  recv: \"{}\", send: \"{}\" }}
  log: {{ directory: \"{}\", segment_size: \"512\" }}
  payload_command: {{ dest_base: \"{}\", source_base: \"{}\" }}
  payload_telemetry: {{ dest_base: \"{}\", source_base: \"{}\" }}
  state_file: \"{state}\"
  tcs_cmd: {{ recv: \"{}\", send: \"{}\" }}
",
            at(),
            at(),
            logs.display(),
            at(),
            at(),
            at(),
            at(),
            at(),
            at()
        ),
    )
    .expect("written");

    let _vehicle = Running(
        Command::new(env!("CARGO_BIN_EXE_tcspecial"))
            .arg(&configuration)
            .arg("--quiet")
            .arg("--simulate=all")
            .env_remove("RUST_LOG")
            .stdout(Stdio::null())
            .spawn()
            .expect("tcspecial runs"),
    );

    // Long enough for a few hundred beacons, a few dozen segments: a
    // directory that kept them would hold that many.
    let by = Instant::now() + Duration::from_secs(2);
    let mut most = 0;
    while Instant::now() < by {
        let here = std::fs::read_dir(&logs).expect("the log directory").count();
        most = most.max(here);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(most > 0, "nothing was ever written to {}", logs.display());
    let here: Vec<String> = std::fs::read_dir(&logs)
        .expect("the log directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(
        here.len() <= 3,
        "filled segments are being kept rather than handed over: {here:?}"
    );
}
