//! The Python's own tests, run where the Rust's are.
//!
//! The suite beside `memusage.py`: what it reads, the Yamcs server it writes
//! for one tcspecial and for two, and what it prints of an answer. None
//! needs Yamcs. Run under `cargo test` so that a change to the Rust that the
//! Python has not followed fails where the Rust change is being tested.

use std::path::Path;
use std::process::Command;

#[test]
fn the_memusage_examples_python_passes_its_tests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ran = Command::new("python3")
        .current_dir(&root)
        .args([
            "-m",
            "unittest",
            "discover",
            "-s",
            "examples/memusage",
            "-p",
            "test_*.py",
            "-t",
            "examples/memusage",
        ])
        .output()
        .expect("python3 runs: the example's ground station is Python 3");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(ran.status.success(), "examples/memusage:\n{said}");
    assert!(said.contains("OK"), "examples/memusage:\n{said}");
}
