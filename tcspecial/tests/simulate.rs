//! tcspecial judges each payload a stand-in or not before it opens anything,
//! and refuses a run whose payloads its `--simulate` mode does not allow.
//!
//! The configuration's one payload is a socket on this host, which is a
//! stand-in. Every address it states is a port below 1024, which a test
//! cannot bind: a run the mode refuses says so before it tries, and a run it
//! allows goes on to say its digest and then fails to bind, so nothing here
//! holds a port another test might want.

use std::process::Command;

/// Writes the test's configuration: one payload that is a stand-in, and
/// every address a port below 1024.
///
/// # Arguments
///
/// * `dir` - The directory to write it in; its log and state file are
///           named there too.
///
/// # Returns
///
/// The configuration file's path.
///
/// # Panics
///
/// If the file cannot be written.
fn configuration(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("vehicle.yaml");
    let missing = dir.join("telemetry");
    let state = dir.join("tcspecial.state").display().to_string();
    std::fs::write(
        &path,
        format!(
            "tcspecial:
  gateway: radio
  interface: \"ad9aac4dd5a6d88dd5e78824ba8a83ac\"
  beacon: {{ default_interval: \"5 s\", reset_interval: \"1 s\", \
  recv: \"127.0.0.1:1\", send: \"127.0.0.1:2\" }}
  log: {{ directory: \"{}\", segment_size: \"65536\" }}
  payload_command: {{ dest_base: \"127.0.0.1:3\", \
  source_base: \"127.0.0.1:4\" }}
  payload_telemetry: {{ dest_base: \"127.0.0.1:5\", \
  source_base: \"127.0.0.1:6\" }}
  state_file: \"{state}\"
  tcs_cmd: {{ recv: \"127.0.0.1:7\", send: \"127.0.0.1:8\" }}
payloads:
  - name: sensor
    type: network
    protocol: \"udp\"
    address: \"127.0.0.1\"
    port: \"9\"
",
            missing.display()
        ),
    )
    .expect("written");
    path
}

/// Runs tcspecial, which must fail, on a fresh copy of the configuration.
///
/// # Arguments
///
/// * `args` - The arguments to give before the configuration's path.
///
/// # Returns
///
/// What tcspecial wrote on stderr.
///
/// # Panics
///
/// If tcspecial cannot be run or exits successfully.
fn run(args: &[&str]) -> String {
    let dir = tempfile::tempdir().expect("a directory");
    let path = configuration(dir.path());
    let ran = Command::new(env!("CARGO_BIN_EXE_tcspecial"))
        .args(args)
        .arg(&path)
        .env_remove("RUST_LOG")
        .output()
        .expect("tcspecial runs");
    assert!(!ran.status.success(), "no run binds a port below 1024");
    String::from_utf8_lossy(&ran.stderr).into_owned()
}

/// Left out, the mode is the flight setting, and a stand-in is refused
/// saying which flag a test run wants -- before the log is looked at.
#[test]
fn a_stand_in_is_refused_in_the_flight_setting() {
    for args in [&[][..], &["--simulate=none"]] {
        let said = run(args);
        assert!(said.contains("simulation mode: none"), "{said}");
        assert!(
            said.contains(
                "sensor: a stand-in: 127.0.0.1 is a loopback address"
            ),
            "{said}"
        );
        assert!(
            said.contains("--simulate=none, the flight setting, allows none"),
            "{said}"
        );
        assert!(!said.contains("tcspecial v"), "{said}");
    }
}

/// Allowed, the run goes on past the judgement to say its digest.
#[test]
fn a_stand_in_is_allowed_where_the_mode_allows_it() {
    let said = run(&["--simulate=all"]);
    assert!(said.contains("simulation mode: all"), "{said}");
    assert!(!said.contains("allows"), "{said}");
    assert!(said.contains("tcspecial v"), "{said}");

    // One stand-in and nothing else is not `some`.
    let said = run(&["--simulate=some"]);
    assert!(said.contains("give --simulate=all"), "{said}");
}

/// A payload of a type the operating system cannot stand in for, with a
/// stand_in, is judged where it will be reached: at its stand_in in a run
/// with stand-ins, and as the device it is under the flight setting.
#[test]
fn a_stand_in_is_where_a_simulated_channel_is_reached() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("vehicle.yaml");
    let state = dir.path().join("tcspecial.state").display().to_string();
    std::fs::write(
        &path,
        format!(
            "tcspecial:
  gateway: radio
  interface: \"ad9aac4dd5a6d88dd5e78824ba8a83ac\"
  beacon: {{ default_interval: \"5 s\", reset_interval: \"1 s\", \
  recv: \"127.0.0.1:1\", send: \"127.0.0.1:2\" }}
  payload_command: {{ dest_base: \"127.0.0.1:3\", \
  source_base: \"127.0.0.1:4\" }}
  payload_telemetry: {{ dest_base: \"127.0.0.1:5\", \
  source_base: \"127.0.0.1:6\" }}
  state_file: \"{state}\"
  tcs_cmd: {{ recv: \"127.0.0.1:7\", send: \"127.0.0.1:8\" }}
payloads:
  - name: channel
    type: rpmsg
    path: \"/nonexistent/rpmsg0\"
    service: \"payload-telemetry\"
    stand_in: \"127.0.0.1:9\"
"
        ),
    )
    .expect("written");
    let said = |mode: &str| {
        let ran = Command::new(env!("CARGO_BIN_EXE_tcspecial"))
            .arg(mode)
            .arg(&path)
            .env_remove("RUST_LOG")
            .output()
            .expect("tcspecial runs");
        String::from_utf8_lossy(&ran.stderr).into_owned()
    };

    let all = said("--simulate=all");
    assert!(
        all.contains("channel: a stand-in: 127.0.0.1 is a loopback address"),
        "{all}"
    );
    let none = said("--simulate=none");
    assert!(
        none.contains(
            "channel: taken for hardware, as what could not be told: \
                       nothing is at /nonexistent/rpmsg0"
        ),
        "{none}"
    );
}
