//! `--visual` is refused where there is no window to open, and where a window
//! would be one opened in flight, before anything is read or opened.
//!
//! The build every test makes is the default one, without the window, because
//! a build with it must not fly; `make check` runs these with
//! `--features visual` as well.

use tcsos::process::Command;

/// The fixture simulator file that states only a mode.
///
/// # Arguments
///
/// * `mode` - The mode: `none`, `some` or `all`.
///
/// # Returns
///
/// Its path.
fn sim(mode: &str) -> String {
    format!(
        "{}/tests/simulate/{mode}-sim.yaml",
        env!("CARGO_MANIFEST_DIR")
    )
}

/// Runs tcspecial.
///
/// # Arguments
///
/// * `mode` - The simulation mode its simulator file states; `None` for no
///            simulator file.
/// * `args` - Its command-line arguments.
///
/// # Returns
///
/// Whether it exited successfully, and what it wrote on stderr.
///
/// # Panics
///
/// If tcspecial cannot be run.
fn tcspecial(mode: Option<&str>, args: &[&str]) -> (bool, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tcspecial"));
    command.env_remove("PAYLOAD_SIM_YAML");
    if let Some(mode) = mode {
        command.env("PAYLOAD_SIM_YAML", sim(mode));
    }
    let ran = command
        .args(args)
        .arg("--no-memlock")
        .env_remove("RUST_LOG")
        .output()
        .expect("tcspecial runs");
    (
        ran.status.success(),
        String::from_utf8_lossy(&ran.stderr).into_owned(),
    )
}

#[cfg(not(feature = "visual"))]
#[test]
fn a_window_asked_of_a_build_without_one_is_refused_saying_why() {
    let (ran, said) = tcspecial(
        Some("all"),
        &["--visual", "/nonexistent/tcspecial-visual-test.ytcs"],
    );
    assert!(!ran, "refused");
    assert!(said.contains("--features visual"), "{said}");
    // Before the file is looked at: nothing about it is said.
    assert!(!said.contains("payload configuration:"), "{said}");
}

/// In the flight setting, whether a simulator file states it or it is what
/// a run without one gets. Said whether or not the window is built in, so that the
/// refusal a flight launch would meet does not depend on the build.
#[test]
fn a_window_asked_of_the_flight_setting_is_refused_saying_why() {
    for mode in [Some("none"), None] {
        let (ran, said) = tcspecial(
            mode,
            &["--visual", "/nonexistent/tcspecial-visual-test.ytcs"],
        );
        assert!(!ran, "refused");
        assert!(
            said.contains("simulate: none, the flight setting"),
            "{said}"
        );
        assert!(said.contains("simulate: some or simulate: all"), "{said}");
        assert!(!said.contains("payload configuration:"), "{said}");
    }
}

/// The flag the mode was once given by is refused, saying where the mode is
/// stated now, before the file is looked at.
#[test]
fn the_old_flag_is_refused_saying_where_the_mode_is() {
    let (ran, said) = tcspecial(
        None,
        &["--simulate=all", "/nonexistent/tcspecial-visual-test.ytcs"],
    );
    assert!(!ran, "refused");
    assert!(said.contains("tcspecial section"), "{said}");
    assert!(said.contains("none, some, all"), "{said}");
    assert!(!said.contains("payload configuration:"), "{said}");
}
