//! One module per node type: the flight side.
//!
//! What opens the real thing and moves bytes through it. The attributes come
//! from `tcslibgs::node`, which checks them, and the stand-in the simulator
//! puts at the far end is `tcssim::node`; the three trees carry the same names
//! so that a type is `node/i2c` wherever you are standing.
//!
//! See `tcslibgs::node` for why there are three trees rather than one.

pub mod i2c;
