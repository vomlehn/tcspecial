//! One module per node type: the flight side, in three families.
//!
//! What opens the real thing and moves bytes through it. The attributes come
//! from `tcslibgs::node`, which checks them, and the stand-in the simulator
//! puts at the far end is `tcssim::node`; the three trees carry the same names
//! so that a type is `node/payload/i2c` wherever you are standing.
//!
//! The families are the configuration's own sections and the editor's working
//! panes: `vehicle` is what the vehicle itself runs and `payload` is what it
//! carries. See `tcslibgs::graph::section_of`, which is the one statement of
//! which family a type belongs to.
//!
//! There is no `moc` directory, and that is not an omission: a ground link is
//! a network endpoint, so what opens one is `payload/network`. See
//! `tcslibgs::node`, which has the argument.
//!
//! See `tcslibgs::node` for why there are three trees rather than one.

/// What the vehicle itself runs: a computation, a log, a pass-through.
pub mod vehicle;
/// What the vehicle carries, and the ground link with it: a device on a bus, a
/// line the vehicle drives, a socket to something aboard or on the ground.
pub mod payload;
