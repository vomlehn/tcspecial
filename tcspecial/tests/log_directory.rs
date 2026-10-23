//! A telemetry log directory that is not there stops tcspecial, saying which
//! directory and what to do about it.
//!
//! What it said was tcslog's "invalid path name", which named neither the
//! directory nor the setting.

use std::net::UdpSocket;
use std::process::Command;

/// A port nothing holds: the configuration check refuses port 0, a link
/// nobody can predict the port of.
fn free() -> u16 {
    UdpSocket::bind("127.0.0.1:0")
        .and_then(|socket| socket.local_addr())
        .expect("a port")
        .port()
}

#[test]
fn a_missing_log_directory_is_named_with_what_to_do() {
    let dir = tempfile::tempdir().expect("a directory");
    let missing = dir.path().join("telemetry");
    let configuration = dir.path().join("vehicle.yaml");
    // Nothing to open but the log, which is opened before anything else.
    let at = || format!("127.0.0.1:{}", free());
    std::fs::write(
        &configuration,
        format!(
            "tcspecial:
  gateway: radio
  beacon: {{ interval: \"5 s\", recv: \"{}\", send: \"{}\" }}
  log: {{ directory: \"{}\", segment_size: \"65536\" }}
  payload_command: {{ dest_base: \"{}\", source_base: \"{}\" }}
  payload_telemetry: {{ dest_base: \"{}\", source_base: \"{}\" }}
  tcs_cmd: {{ recv: \"{}\", send: \"{}\" }}
",
            at(),
            at(),
            missing.display(),
            at(),
            at(),
            at(),
            at(),
            at(),
            at()
        ),
    )
    .expect("written");

    let ran = Command::new(env!("CARGO_BIN_EXE_tcspecial"))
        .arg(&configuration)
        .arg("--quiet")
        .arg("--simulate=all")
        .env_remove("RUST_LOG")
        .output()
        .expect("tcspecial runs");
    assert!(
        !ran.status.success(),
        "a vehicle with nowhere to log is not flown"
    );
    let said = String::from_utf8_lossy(&ran.stderr);
    assert!(said.contains(&missing.display().to_string()), "{said}");
    assert!(said.contains("does not exist"), "{said}");
    assert!(said.contains("log.directory"), "{said}");
    assert!(!said.contains("invalid path name"), "{said}");
}
