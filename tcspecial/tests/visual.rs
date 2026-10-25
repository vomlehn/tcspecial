//! `--visual` is refused where there is no window to open, and where a window
//! would be one opened in flight, before anything is read or opened.
//!
//! The build every test makes is the default one, without the window, because
//! a build with it must not fly; `make check` runs these with
//! `--features visual` as well.

use std::process::Command;

fn tcspecial(args: &[&str]) -> (bool, String) {
    let ran = Command::new(env!("CARGO_BIN_EXE_tcspecial"))
        .args(args)
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
    let (ran, said) = tcspecial(&[
        "--visual",
        "--simulate=all",
        "/nonexistent/tcspecial-visual-test.yaml",
    ]);
    assert!(!ran, "refused");
    assert!(said.contains("--features visual"), "{said}");
    // Before the file is looked at: nothing about it is said.
    assert!(!said.contains("payload configuration:"), "{said}");
}

/// In the flight setting, whether it is asked for or is what a run without
/// `--simulate` gets. Said whether or not the window is built in, so that the
/// refusal a flight launch would meet does not depend on the build.
#[test]
fn a_window_asked_of_the_flight_setting_is_refused_saying_why() {
    for simulate in [&["--simulate=none"][..], &[]] {
        let mut args = vec!["--visual"];
        args.extend_from_slice(simulate);
        args.push("/nonexistent/tcspecial-visual-test.yaml");
        let (ran, said) = tcspecial(&args);
        assert!(!ran, "refused");
        assert!(
            said.contains("--simulate=none, the flight setting"),
            "{said}"
        );
        assert!(said.contains("--simulate=some or --simulate=all"), "{said}");
        assert!(!said.contains("payload configuration:"), "{said}");
    }
}

#[test]
fn a_mistaken_mode_is_refused_naming_the_three() {
    let (ran, said) = tcspecial(&[
        "--simulate=most",
        "/nonexistent/tcspecial-visual-test.yaml",
    ]);
    assert!(!ran, "refused");
    assert!(said.contains("none, some, all"), "{said}");
    assert!(!said.contains("payload configuration:"), "{said}");
}
