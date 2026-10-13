//! One module per node type: the flight side.
//!
//! What opens the real thing and moves bytes through it. The attributes come
//! from `tcslibgs::node`, which checks them, and the stand-in the simulator
//! puts at the far end is `tcssim::node`; the three trees carry the same names
//! so that a type is `node/i2c` wherever you are standing.
//!
//! See `tcslibgs::node` for why there are three trees rather than one.

// Computation aboard, which is a node type like any other: it is named in
// the configuration, it takes what arrives and it hands something on.
pub mod compute;
// Somewhere data stops, which is a node type like any other for the same
// reasons: see node/log.
pub mod log;
// A node that carries both streams through and does nothing to what passes.
pub mod null;
pub mod i2c;
pub mod spi;
pub mod can;
pub mod gpio;
pub mod hdlc;
pub mod iio;
pub mod rpmsg;
pub mod serial;
pub mod network;
pub mod device_file;
