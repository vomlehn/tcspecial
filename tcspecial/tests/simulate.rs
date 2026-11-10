//! tcspecial judges each payload a real, a dummy or a simulated device before
//! it opens anything, and refuses a run whose payloads its `--simulate` mode
//! does not allow.
//!
//! A simulated device here is a pseudo-terminal, as tcssim makes for a serial
//! payload; a network payload is a real device whatever answers it. Every
//! address a configuration states is a port below 1024, which a test cannot
//! bind: a run the mode refuses says so before it tries, and a run it allows
//! goes on to say its digest and then fails to bind, so nothing here holds a
//! port another test might want.

use std::fs::File;
use std::process::Command;

/// Writes a test's configuration: the given payloads, and every address a port
/// below 1024.
///
/// # Arguments
///
/// * `dir`      - The directory to write it in; its log and state file are
///                named there too.
/// * `payloads` - The configuration's `payloads` section, without its key.
///
/// # Returns
///
/// The configuration file's path.
///
/// # Panics
///
/// If the file cannot be written.
fn configuration(dir: &std::path::Path, payloads: &str) -> std::path::PathBuf {
    let path = dir.join("vehicle.ytcs");
    let missing = dir.join("telemetry");
    let state = dir.join("tcspecial.state").display().to_string();
    std::fs::write(
        &path,
        format!(
            "tcspecial:
  gateway: radio
  interface: \"55ad11486347bc8efc163e1ba051e577\"
  beacon: {{ default_interval: \"5 s\", reset_interval: \"1 s\", recv: \
            \"127.0.0.1:1\", send: \"127.0.0.1:2\" }}
  log: {{ directory: \"{}\", segment_size: \"65536\" }}
  payload_command: {{ dest_base: \"127.0.0.1:3\", source_base: \
            \"127.0.0.1:4\" }}
  payload_telemetry: {{ dest_base: \"127.0.0.1:5\", source_base: \
            \"127.0.0.1:6\" }}
  state_file: \"{state}\"
  tcs_cmd: {{ recv: \"127.0.0.1:7\", send: \"127.0.0.1:8\" }}
payloads:
{payloads}",
            missing.display()
        ),
    )
    .expect("written");
    path
}

/// Opens a pseudo-terminal, which is what tcssim makes for a serial payload.
///
/// # Returns
///
/// The master, which keeps the terminal open while it is held, and the slave's
/// path.
///
/// # Panics
///
/// If no pseudo-terminal can be had.
fn pseudo_terminal() -> (File, String) {
    use std::os::unix::io::FromRawFd;

    // SAFETY: posix_openpt takes only flags and has no preconditions; its
    // result is checked before it is used.
    let master_fd =
        unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
    assert!(
        master_fd >= 0,
        "no pty: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: from_raw_fd needs sole ownership of an open descriptor.
    // `master_fd` was just opened, is checked above, and nothing else takes
    // ownership of it; the uses below only borrow it while the File is alive.
    let master = unsafe { File::from_raw_fd(master_fd) };
    // SAFETY: grantpt needs an open pty master, which `master_fd` is, owned by
    // the File above for the rest of the test.
    assert_eq!(unsafe { libc::grantpt(master_fd) }, 0);
    // SAFETY: as for grantpt.
    assert_eq!(unsafe { libc::unlockpt(master_fd) }, 0);
    let mut name = [0 as libc::c_char; 128];
    assert_eq!(
        // SAFETY: ptsname_r needs an open pty master and a buffer it can write
        // of the length it is given: `name` is a local array, and its own
        // length is what is passed.
        unsafe { libc::ptsname_r(master_fd, name.as_mut_ptr(), name.len()) },
        0
    );
    // SAFETY: from_ptr needs a NUL-terminated string that outlives the borrow.
    // ptsname_r returned 0, so it wrote a terminated name into `name`, and
    // `name` outlives the CStr, which is copied out at once.
    let slave = unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    (master, slave)
}

/// The payloads section of a serial payload on a pseudo-terminal.
///
/// # Arguments
///
/// * `slave` - The pseudo-terminal's slave.
///
/// # Returns
///
/// The section.
fn on_a_terminal(slave: &str) -> String {
    format!(
        "  - type serial:
      - payload: sensor
        path: \"{slave}\"
        byte_length: \"8\"
        datarate: \"9600\"
        max_length: \"64\"
        read_timeout: \"none\"
        stop_bits: \"1\"
"
    )
}

/// A network payload on this host.
const ON_THE_NETWORK: &str = "  - type network:
      - payload: sensor
        address: \"127.0.0.1\"
        port: \"9\"
        protocol: \"udp\"
";

/// Runs tcspecial, which must fail, on a fresh configuration.
///
/// # Arguments
///
/// * `args`     - The arguments to give before the configuration's path.
/// * `payloads` - The configuration's `payloads` section.
///
/// # Returns
///
/// What tcspecial wrote on stderr.
///
/// # Panics
///
/// If tcspecial cannot be run or exits successfully.
fn run(args: &[&str], payloads: &str) -> String {
    let dir = tempfile::tempdir().expect("a directory");
    let path = configuration(dir.path(), payloads);
    let ran = Command::new(env!("CARGO_BIN_EXE_tcspecial"))
        .args(args)
        .arg(&path)
        .env_remove("RUST_LOG")
        .output()
        .expect("tcspecial runs");
    assert!(!ran.status.success(), "no run binds a port below 1024");
    String::from_utf8_lossy(&ran.stderr).into_owned()
}

/// Left out, the mode is the flight setting, and a simulated device is refused
/// saying which flag a test run wants -- before the log is looked at.
#[test]
fn a_simulated_device_is_refused_in_the_flight_setting() {
    let (_master, slave) = pseudo_terminal();
    for args in [&[][..], &["--simulate=none"]] {
        let said = run(args, &on_a_terminal(&slave));
        assert!(said.contains("simulation mode: none"), "{said}");
        assert!(
            said.contains(&format!(
                "sensor: a simulated device: {slave} resolves to a \
                 pseudo-terminal"
            )),
            "{said}"
        );
        assert!(
            said.contains(
                "a simulated device, and --simulate=none, the flight \
                 setting, allows only real devices"
            ),
            "{said}"
        );
        assert!(!said.contains("tcspecial v"), "{said}");
    }
}

/// Allowed, the run goes on past the judgement to say its digest.
#[test]
fn a_simulated_device_is_allowed_where_the_mode_allows_it() {
    let (_master, slave) = pseudo_terminal();
    let said = run(&["--simulate=all"], &on_a_terminal(&slave));
    assert!(said.contains("simulation mode: all"), "{said}");
    assert!(!said.contains("allows"), "{said}");
    assert!(said.contains("tcspecial v"), "{said}");

    // One simulated device and nothing else is not `some`.
    let said = run(&["--simulate=some"], &on_a_terminal(&slave));
    assert!(said.contains("give --simulate=all"), "{said}");
}

/// A network payload is a real device, whatever answers at its far end, so the
/// mode does not count it: neither the flight setting nor a run with simulated
/// devices refuses it.
#[test]
fn a_network_payload_is_a_real_device_under_every_mode() {
    for mode in ["--simulate=none", "--simulate=all"] {
        let said = run(&[mode], ON_THE_NETWORK);
        assert!(!said.contains("allows"), "{said}");
        assert!(!said.contains("simulated device"), "{said}");
        assert!(said.contains("tcspecial v"), "{said}");
    }

    // With nothing counted there is no payload to simulate, which `some` wants
    // one of.
    let said = run(&["--simulate=some"], ON_THE_NETWORK);
    assert!(said.contains("has no payloads"), "{said}");
}

/// A payload of a type the operating system has no dummy device for, with a
/// sim_address, is judged where it will be reached: at its sim_address in a run
/// that simulates, and as the device it is under the flight setting.
#[test]
fn a_sim_address_is_where_a_simulated_channel_is_reached() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("vehicle.ytcs");
    let state = dir.path().join("tcspecial.state").display().to_string();
    std::fs::write(
        &path,
        format!(
            "tcspecial:
  gateway: radio
  interface: \"55ad11486347bc8efc163e1ba051e577\"
  beacon: {{ default_interval: \"5 s\", reset_interval: \"1 s\", recv: \
            \"127.0.0.1:1\", send: \"127.0.0.1:2\" }}
  payload_command: {{ dest_base: \"127.0.0.1:3\", source_base: \
            \"127.0.0.1:4\" }}
  payload_telemetry: {{ dest_base: \"127.0.0.1:5\", source_base: \
            \"127.0.0.1:6\" }}
  state_file: \"{state}\"
  tcs_cmd: {{ recv: \"127.0.0.1:7\", send: \"127.0.0.1:8\" }}
payloads:
  - type rpmsg:
      - payload: channel
        path: \"/nonexistent/rpmsg0\"
        service: \"payload-telemetry\"
        sim_address: \"127.0.0.1:9\"
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
        all.contains(
            "channel: a simulated device: 127.0.0.1 is a loopback address"
        ),
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
