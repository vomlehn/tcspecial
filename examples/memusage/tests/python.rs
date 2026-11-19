//! The Python's own tests, run where the Rust's are.
//!
//! The suite beside `memusage.py`: what it reads, the Yamcs server it writes
//! for one tcspecial and for two, and what it prints of an answer. None
//! needs Yamcs. Run under `cargo test` so that a change to the Rust that the
//! Python has not followed fails where the Rust change is being tested.

#[path = "../../python-common/suite.rs"]
mod suite;

#[test]
fn the_memusage_examples_python_passes_its_tests() {
    suite::suite("examples/memusage", "test_*.py", "examples/memusage");
}
