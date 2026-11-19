//! The Python's own tests, run where the Rust's are.
//!
//! Three suites: the libraries' in `python/tests`, which say each Rust test of
//! `tcsmoc::beacon`, `tcs-ui` and the ping format again, with the same
//! figures; this example's, beside `ping_gateway.py`; and what it shares with
//! ping-yamcs, the ping link and the window, in `examples/ping-common`. Run
//! under `cargo test` so that a change to the Rust that the Python has not
//! followed fails where the Rust change is being tested.

#[path = "../../python-common/suite.rs"]
mod suite;

use suite::suite;

#[test]
fn the_python_libraries_pass_their_tests() {
    suite("python/tests", "test_*.py", "python");
}

#[test]
fn tcs_uis_python_passes_its_tests() {
    suite("tcs-ui/python/tests", "test_*.py", "tcs-ui/python");
}

#[test]
fn the_ping_examples_python_passes_its_tests() {
    suite("examples/ping", "test_*.py", "examples/ping");
}

#[test]
fn what_the_ping_examples_share_passes_its_tests() {
    suite("examples/ping-common", "test_*.py", "examples/ping-common");
}
