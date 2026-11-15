//! The memory lock, as tcspecial meets it: a lock the limit refuses stops a
//! desk run and is run without in flight, and `--no-memlock` asks for none.
//!
//! Each run is given a limit on locked memory of 1 MiB, far less than
//! tcspecial maps, so that the lock fails here whatever limit the machine
//! running the tests has. A lock that succeeds is not tested here: it takes
//! a limit larger than this desk's.

use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

mod common;
use common::{bound, free, port};

/// The limit on locked memory each run is given, in bytes.
const LIMIT: libc::rlim_t = 1024 * 1024;

/// Writes a configuration with no nodes into `dir`.
///
/// # Arguments
///
/// * `dir` - Where it goes, with its state file.
///
/// # Returns
///
/// Its path.
///
/// # Panics
///
/// If it cannot be written.
fn configuration(dir: &std::path::Path) -> std::path::PathBuf {
    let beacons = bound();
    let at = || format!("127.0.0.1:{}", free());
    let path = dir.join("vehicle.ytcs");
    let state = dir.join("tcspecial.state").display().to_string();
    std::fs::write(
        &path,
        format!(
            "tcspecial:
  gateway: radio
  interface: \"{}\"
  beacon: {{ default_interval: \"100 ms\", reset_interval: \"100 ms\", \
  recv: \"127.0.0.1:{}\", send: \"{}\" }}
  payload_command: {{ dest_base: \"{}\", source_base: \"{}\" }}
  payload_telemetry: {{ dest_base: \"{}\", source_base: \"{}\" }}
  state_file: \"{state}\"
  max_consolidator_inputs: \"32\"
  tcs_cmd: {{ recv: \"{}\", send: \"{}\" }}
",
            tcsprotocol::InterfaceDigest::of_this_build(),
            port(&beacons),
            at(),
            at(),
            at(),
            at(),
            at(),
            at(),
            at(),
        ),
    )
    .expect("written");
    path
}

/// Runs tcspecial with a limit on locked memory of [`LIMIT`] until it
/// ends or `seconds` pass, when it is stopped.
///
/// # Arguments
///
/// * `args`    - Its arguments, the configuration last.
/// * `seconds` - How long it may run.
///
/// # Returns
///
/// Whether it ended by itself, its status, and what it said on its error
/// output.
///
/// # Panics
///
/// If it cannot be started.
fn run(args: &[&str], seconds: u64) -> (bool, Option<i32>, String) {
    let dir = tempfile::tempdir().expect("a directory");
    let log = dir.path().join("tcspecial.log");
    let path = configuration(dir.path());
    let mut command = Command::new(env!("CARGO_BIN_EXE_tcspecial"));
    command
        .args(args)
        .arg(&path)
        .env_remove("RUST_LOG")
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&log).expect("a log"));
    // SAFETY: setrlimit lowers a limit of the child's, between fork and
    // exec, and allocates nothing.
    unsafe {
        command.pre_exec(|| {
            let limit = libc::rlimit {
                rlim_cur: LIMIT,
                rlim_max: LIMIT,
            };
            if libc::setrlimit(libc::RLIMIT_MEMLOCK, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().expect("tcspecial starts");
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut ended = None;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().expect("it can be waited on") {
            ended = Some(status);
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let by_itself = ended.is_some();
    let status = match ended {
        Some(status) => status.code(),
        None => {
            // SAFETY: a signal to a child this test started.
            unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTERM) };
            child.wait().expect("it ends").code()
        }
    };
    (
        by_itself,
        status,
        std::fs::read_to_string(&log).unwrap_or_default(),
    )
}

/// Under `--simulate`, a lock the limit refuses stops tcspecial, saying
/// the limit, and what to change.
#[test]
fn a_refused_lock_stops_a_desk_run() {
    for mode in ["--simulate=all", "--simulate=some"] {
        let (by_itself, status, said) = run(&[mode], 20);
        assert!(by_itself, "{mode}: it stops by itself: {said}");
        assert_ne!(status, Some(0), "{mode}: {said}");
        assert!(said.contains("cannot be locked"), "{mode}: {said}");
        assert!(said.contains("1024 KiB"), "{mode}: the limit: {said}");
        assert!(said.contains("--no-memlock"), "{mode}: {said}");
        assert!(said.contains("LimitMEMLOCK"), "{mode}: {said}");
    }
}

/// In flight, a lock the limit refuses is said, and the vehicle runs
/// unlocked.
#[test]
fn a_refused_lock_is_run_without_in_flight() {
    let (by_itself, _, said) = run(&[], 3);
    assert!(!by_itself, "it runs on: {said}");
    assert!(said.contains("memory: not locked: "), "{said}");
    assert!(said.contains("1024 KiB"), "{said}");
}

/// `--no-memlock` asks for no lock, under any mode, and the summary says
/// so.
#[test]
fn no_memlock_asks_for_none() {
    for mode in ["--simulate=all", "--simulate=none"] {
        let (by_itself, _, said) = run(&[mode, "--no-memlock"], 3);
        assert!(!by_itself, "{mode}: it runs on: {said}");
        assert!(
            said.contains("memory: not locked, as --no-memlock asks"),
            "{mode}: {said}"
        );
        assert!(!said.contains("cannot be locked"), "{mode}: {said}");
    }
}
