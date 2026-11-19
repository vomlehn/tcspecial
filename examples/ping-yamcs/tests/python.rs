//! The Python's own tests, run where the Rust's are.
//!
//! The suite beside `ping_yamcs.py`: what it reads, the Yamcs configuration it
//! writes, its ping and beacon links through a Yamcs played there, and that
//! Yamcs dies with it. None needs Yamcs or opens a window. Run under
//! `cargo test` so that a change to the Rust that the Python has not followed
//! fails where the Rust change is being tested.

#[path = "../../python-common/suite.rs"]
mod suite;

#[test]
fn the_ping_yamcs_examples_python_passes_its_tests() {
    suite::suite("examples/ping-yamcs", "test_*.py", "examples/ping-yamcs");
}
