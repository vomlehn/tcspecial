//! tcs-sim refuses the flight setting and the flag the mode was once given
//! by, and `tcs-sim --wait` says what it heard.
//!
//! None opens a window: the first two are refused before any configuration
//! is read, and the third waits for a simulator rather than being one.

use tcsos::process::Command;

/// Runs tcs-sim from the top of the tree.
///
/// # Arguments
///
/// * `sim`  - The simulator file `PAYLOAD_SIM_YAML` names, from the top of
///            the tree; `None` for none named.
/// * `args` - Its command-line arguments.
///
/// # Returns
///
/// Whether it succeeded, and what it wrote to standard error.
///
/// # Panics
///
/// If tcs-sim cannot be run.
fn tcs_sim(sim: Option<&str>, args: &[&str]) -> (bool, String) {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
    let mut command = Command::new(env!("CARGO_BIN_EXE_tcs-sim"));
    command.env_remove("PAYLOAD_SIM_YAML");
    if let Some(sim) = sim {
        command.env("PAYLOAD_SIM_YAML", sim);
    }
    let ran = command
        .args(args)
        .current_dir(root)
        .output()
        .expect("tcs-sim runs");
    (
        ran.status.success(),
        String::from_utf8_lossy(&ran.stderr).into_owned(),
    )
}

/// A simulator file stating the flight setting, `none`, leaves a simulator
/// nothing to do.
#[test]
fn the_flight_setting_is_refused() {
    let none = "tcs-sim/tests/simulate/none-sim.yaml";
    for args in [
        &["examples/minimal/minimal.ytcs"][..],
        &["--wait", "examples/minimal/minimal.ytcs"],
    ] {
        let (ran, said) = tcs_sim(Some(none), args);
        assert!(!ran, "{args:?}");
        assert!(said.contains("the flight setting"), "{said}");
        assert!(said.contains("simulate: some or simulate: all"), "{said}");
    }
}

/// The flag the mode was once given by is refused, saying where the mode is
/// stated now.
#[test]
fn the_old_flag_is_refused() {
    let (ran, said) =
        tcs_sim(None, &["--simulate=all", "examples/minimal/minimal.ytcs"]);
    assert!(!ran, "{said}");
    assert!(said.contains("tcspecial section"), "{said}");
}

/// With no simulator running, `--wait` says it heard nothing, and where it
/// listened -- the address in the simulator file.
#[test]
fn waiting_for_a_simulator_nobody_started_says_nothing_answered() {
    let (ran, said) =
        tcs_sim(None, &["--wait=0.5", "examples/minimal/minimal.ytcs"]);
    assert!(!ran, "{said}");
    assert!(
        said.contains("nothing answered at 127.0.0.28:7903"),
        "{said}"
    );
}
