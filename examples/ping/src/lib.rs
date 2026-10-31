//! The ping example's tests, and nothing else.
//!
//! The example's ground station is a Python 3 program, `ping_gateway.py`,
//! beside this crate, and the libraries it is built on are Python spellings of
//! Rust ones: `python/tcslib/beacon.py` of `tcslib::beacon`, `python/tcsui` of
//! `tcsui`, and `python/tcslibgs` of the parts of `tcslibgs` a ground station
//! reads. A rule written twice is two rules on the day one of them changes, so
//! the tests here hold each Python spelling to its Rust one:
//!
//! * `tests/python.rs` runs the Python's own tests, which use the Rust tests'
//!   examples;
//! * `tests/agrees.rs` checks that what the Python reads out of `ping.xml` is
//!   what the Rust reads out of `ping.yaml`;
//! * `tests/pings.rs` plays the vehicle with the Rust -- the ping answered by
//!   `tcslibgs::node::vehicle::ping::reply`, the beacon serialized as
//!   tcspecial serializes it -- and runs the ground station against it.
//!
//! A crate rather than a directory of scripts so that `cargo test` runs them
//! with everything else, which is where a change to the Rust is tested.
