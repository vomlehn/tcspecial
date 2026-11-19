//! The Python's own tests, run where the Rust's are.
//!
//! The suite beside `thermostat_yamcs.py`: what it reads, the mission
//! database it has generated, and what it shows of a Yamcs played there.
//! None needs Yamcs, tcs-sim or a vehicle, or opens a window. Run under
//! `cargo test` so that a change to the Rust that the Python has not
//! followed fails where the Rust change is being tested.

#[path = "../../python-common/suite.rs"]
mod suite;

#[test]
fn the_thermostat_yamcs_examples_python_passes_its_tests() {
    suite::suite(
        "examples/thermostat-yamcs",
        "test_*.py",
        "examples/thermostat-yamcs",
    );
}
