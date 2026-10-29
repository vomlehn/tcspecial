//! `-q` and `--quiet` leave the startup summary unsaid, and nothing else.
//!
//! A file that is not there stops tcspecial before it opens anything, which
//! makes it the one start a test can run with no ports and no devices: the
//! summary's first line is said before the file is read, and what stops the
//! start is said after. Quiet, only the second.

use std::process::Command;

/// Runs tcspecial, which must fail, and collects what it says.
///
/// # Arguments
///
/// * `args` - Its command-line arguments.
///
/// # Returns
///
/// What tcspecial wrote on stderr.
///
/// # Panics
///
/// If tcspecial cannot be run or exits successfully.
fn said(args: &[&str]) -> String {
    let ran = Command::new(env!("CARGO_BIN_EXE_tcspecial"))
        .args(args)
        // The default, so that what is tested is the flag and not whatever
        // the environment this runs in asks for.
        .env_remove("RUST_LOG")
        .output()
        .expect("tcspecial runs");
    assert!(
        !ran.status.success(),
        "a file that is not there is not flown"
    );
    String::from_utf8_lossy(&ran.stderr).into_owned()
}

#[test]
fn quiet_leaves_the_summary_unsaid_and_still_says_why_it_stopped() {
    let missing = "/nonexistent/tcspecial-quiet-test.yaml";

    let talking = said(&[missing]);
    assert!(talking.contains("payload configuration:"), "{talking}");
    assert!(talking.contains("No such file"), "{talking}");

    for flag in ["-q", "--quiet"] {
        // Before the file or after it: the flag is taken out wherever it is.
        for args in [[flag, missing], [missing, flag]] {
            let quiet = said(&args);
            assert!(
                !quiet.contains("payload configuration:"),
                "{args:?}: {quiet}"
            );
            assert!(quiet.contains("No such file"), "{args:?}: {quiet}");
        }
    }
}
