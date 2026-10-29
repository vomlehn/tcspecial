//! The Python's own tests, run where the Rust's are.
//!
//! Two suites: the libraries' in `python/tests`, which say each Rust test of
//! `tcslib::beacon`, `tcsui` and the ping format again, with the same figures;
//! and this example's, beside `ping_moc.py`. Run under `cargo test` so that a
//! change to the Rust that the Python has not followed fails where the Rust
//! change is being tested.

use std::path::Path;
use std::process::Command;

/// Run one suite of `unittest` tests and fail with what it said.
///
/// # Arguments
///
/// * `directory` - Where the tests are, from the repository's root.
/// * `pattern`   - Which files in it are tests, as a glob.
/// * `top`       - The top of the package tree the tests import from.
///
/// # Panics
///
/// python3 cannot be run, or the suite does not pass.
fn suite(directory: &str, pattern: &str, top: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ran = Command::new("python3")
        .current_dir(&root)
        .args([
            "-m", "unittest", "discover", "-s", directory, "-p", pattern, "-t",
            top,
        ])
        .output()
        .expect("python3 runs: the example's ground station is Python 3");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(ran.status.success(), "{directory}:\n{said}");
    assert!(said.contains("OK"), "{directory}:\n{said}");
}

#[test]
fn the_python_libraries_pass_their_tests() {
    suite("python/tests", "test_*.py", "python");
}

#[test]
fn the_ping_examples_python_passes_its_tests() {
    suite("examples/ping", "test_*.py", "examples/ping");
}
