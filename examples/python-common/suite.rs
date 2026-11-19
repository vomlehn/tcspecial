//! Running a Python test suite under `cargo test`, shared by the examples
//! whose ground stations are Python: each example's `tests/python.rs`
//! takes this file in with `#[path]`.
//!
//! The Python ground packages read a configuration's links through
//! libtcsmoc-c, loaded from this tree's build. `cargo test` does not build
//! a cdylib for a test, so a fresh checkout would have none, and a checkout
//! built before could have one older than the source: each suite builds it
//! first, as it is now, as `tcspecial/tests/consolidator.rs` builds its
//! static library.

use std::path::Path;
use std::process::Command;

/// Builds libtcsmoc-c from the source as it is now, in the profile this
/// test was built in, where the Python looks for it.
///
/// # Panics
///
/// If cargo cannot be run or the library does not build.
fn build_tcsmoc_c() {
    let test = std::env::current_exe().expect("where this test is");
    let profile = test
        .parent()
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .expect("the target directory's profile");
    let mut build = Command::new(env!("CARGO"));
    build.args(["build", "--quiet", "-p", "tcsmoc-c", "--lib"]);
    if profile == "release" {
        build.arg("--release");
    }
    let built = build.output().expect("cargo runs");
    assert!(
        built.status.success(),
        "tcsmoc-c does not build:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );
}

/// Runs one suite of `unittest` tests, libtcsmoc-c built first, and fails
/// with what it said.
///
/// # Arguments
///
/// * `directory` - Where the tests are, from the repository's root.
/// * `pattern`   - Which files in it are tests, as a glob.
/// * `top`       - The top of the package tree the tests import from.
///
/// # Panics
///
/// If libtcsmoc-c does not build, python3 cannot be run, or the suite does
/// not pass.
pub fn suite(directory: &str, pattern: &str, top: &str) {
    build_tcsmoc_c();
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
