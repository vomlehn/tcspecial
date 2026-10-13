//! What the vehicle itself runs: the attributes, and how they are checked.
//!
//! A computation and a log are node types like any other -- each is named in a
//! configuration, each takes what arrives and does something with it -- and
//! what tells them from a payload is that nothing is bolted to the vehicle for
//! them. They are parts of the telemetry and control system itself, and they
//! are what the configuration's `vehicle` section holds.
//!
//! The arithmetic and the writing are `tcspecial::node::vehicle`; what is here
//! is what a file may say and what is wrong with what it says.

// Computation aboard: what a compute node may say, the arithmetic being
// tcspecial's.
pub mod compute;
// Somewhere data stops rather than being passed on.
pub mod log;
