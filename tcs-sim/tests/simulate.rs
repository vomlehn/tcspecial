//! tcs-sim refuses the flight setting, and `tcs-sim --wait` says what it heard.
//!
//! Neither opens a window: the first is refused before anything is read, and
//! the second waits for a simulator rather than being one.

use std::process::Command;

/// Runs tcs-sim from the top of the tree, without `PAYLOAD_SIM_YAML`.
///
/// # Arguments
///
/// * `args` - Its command-line arguments.
///
/// # Returns
///
/// Whether it succeeded, and what it wrote to standard error.
///
/// # Panics
///
/// If tcs-sim cannot be run.
fn tcs_sim(args: &[&str]) -> (bool, String) {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
    let ran = Command::new(env!("CARGO_BIN_EXE_tcs-sim"))
        .args(args)
        .current_dir(root)
        .env_remove("PAYLOAD_SIM_YAML")
        .output()
        .expect("tcs-sim runs");
    (
        ran.status.success(),
        String::from_utf8_lossy(&ran.stderr).into_owned(),
    )
}

/// Left out, the mode is the flight setting, which has nothing for a
/// simulator to do.
#[test]
fn the_flight_setting_is_refused_before_anything_is_read() {
    for args in [
        &["examples/minimal/minimal.ytcs"][..],
        &["--simulate=none", "examples/minimal/minimal.ytcs"],
        &["--wait", "examples/minimal/minimal.ytcs"],
    ] {
        let (ran, said) = tcs_sim(args);
        assert!(!ran, "{args:?}");
        assert!(said.contains("the flight setting"), "{said}");
        assert!(said.contains("--simulate=some or --simulate=all"), "{said}");
    }
}

/// With no simulator running, `--wait` says it heard nothing, and where it
/// listened -- the address in the simulator file.
#[test]
fn waiting_for_a_simulator_nobody_started_says_nothing_answered() {
    let (ran, said) = tcs_sim(&[
        "--simulate=all",
        "--wait=0.5",
        "examples/minimal/minimal.ytcs",
    ]);
    assert!(!ran, "{said}");
    assert!(
        said.contains("nothing answered at 127.0.0.1:7903"),
        "{said}"
    );
}
