//! tcssim refuses the flight setting, and `tcssim --wait` says what it heard.
//!
//! Neither opens a window: the first is refused before anything is read, and
//! the second waits for a simulator rather than being one.

use std::process::Command;

fn tcssim(args: &[&str]) -> (bool, String) {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
    let ran = Command::new(env!("CARGO_BIN_EXE_tcssim"))
        .args(args)
        .current_dir(root)
        .env_remove("PAYLOAD_SIM_YAML")
        .output()
        .expect("tcssim runs");
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
        &["examples/minimal/minimal.yaml"][..],
        &["--simulate=none", "examples/minimal/minimal.yaml"],
        &["--wait", "examples/minimal/minimal.yaml"],
    ] {
        let (ran, said) = tcssim(args);
        assert!(!ran, "{args:?}");
        assert!(said.contains("the flight setting"), "{said}");
        assert!(said.contains("--simulate=some or --simulate=all"), "{said}");
    }
}

/// With no simulator running, `--wait` says it heard nothing, and where it
/// listened -- the address in the simulator file.
#[test]
fn waiting_for_a_simulator_nobody_started_says_nothing_answered() {
    let (ran, said) = tcssim(&[
        "--simulate=all",
        "--wait=0.5",
        "examples/minimal/minimal.yaml",
    ]);
    assert!(!ran, "{said}");
    assert!(
        said.contains("nothing answered at 127.0.0.1:7903"),
        "{said}"
    );
}
