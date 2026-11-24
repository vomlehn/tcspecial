//! `--help` says how tcspecial is run, rather than being taken for the name
//! of a configuration.

use tcsos::process::Command;

#[test]
fn help_says_how_it_is_run() {
    for flag in ["--help", "-h"] {
        let ran = Command::new(env!("CARGO_BIN_EXE_tcspecial"))
            .arg(flag)
            .output()
            .expect("tcspecial runs");
        assert!(ran.status.success(), "{flag}");
        let said = String::from_utf8_lossy(&ran.stdout);
        for option in [
            "simulate: none, some or all",
            "--no-memlock",
            "--quiet",
            "--types",
        ] {
            assert!(said.contains(option), "{flag}: {option}: {said}");
        }
    }
}
