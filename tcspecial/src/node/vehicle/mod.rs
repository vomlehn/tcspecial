//! What the vehicle itself runs.
//!
//! Each of these is a node type like any other: it is named in the
//! configuration, it takes what arrives and it hands something on. What tells
//! them from a payload is that there is nothing bolted to the vehicle for
//! them -- they are parts of the telemetry and control system itself, which is
//! why a mission that changes what it carries does not change these.
//!
//! What a file may say about one, and what is wrong with what it says, is
//! `tcslibgs::node::vehicle`.

// Computation aboard: the arithmetic, the control loops, and the program a
// configuration may run.
pub mod compute;
// Somewhere data stops rather than being passed on.
pub mod log;
// A node that carries both streams through and does nothing to what passes.
pub mod null;
