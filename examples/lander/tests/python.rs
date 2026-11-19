//! The Python's own tests, run where the Rust's are.
//!
//! The suite beside `lander_yamcs.py`: the landscapes it makes, held to the
//! lander's physics by tcs-verify; the relays and their delay; and what its
//! window shows. None needs Yamcs, tcs-sim or a vehicle, or opens a window.
//! Run under `cargo test` so that a change to the Rust that the Python has
//! not followed fails where the Rust change is being tested.

#[path = "../../python-common/suite.rs"]
mod suite;

#[test]
fn the_lander_examples_python_passes_its_tests() {
    suite::suite("examples/lander", "test_*.py", "examples/lander");
}
