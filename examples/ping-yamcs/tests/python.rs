//! The Python's own tests, run where the Rust's are.
//!
//! The suite beside `ping_yamcs.py`: what it reads, the Yamcs configuration it
//! writes, its ping and beacon links through a Yamcs played there, and that
//! Yamcs dies with it. None needs Yamcs or opens a window. Run under
//! `cargo test` so that a change to the Rust that the Python has not followed
//! fails where the Rust change is being tested.

use std::path::Path;
use std::process::Command;

#[test]
fn the_ping_yamcs_examples_python_passes_its_tests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ran = Command::new("python3")
        .current_dir(&root)
        .args([
            "-m",
            "unittest",
            "discover",
            "-s",
            "examples/ping-yamcs",
            "-p",
            "test_*.py",
            "-t",
            "examples/ping-yamcs",
        ])
        .output()
        .expect("python3 runs: the example's ground station is Python 3");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(ran.status.success(), "examples/ping-yamcs:\n{said}");
    assert!(said.contains("OK"), "examples/ping-yamcs:\n{said}");
}
