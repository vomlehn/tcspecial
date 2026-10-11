//! One module per node type: the simulator's stand-in.
//!
//! What goes at the far end of a node so that a configuration can be run with
//! no hardware present. The attributes come from `tcslibgs::node` and the
//! flight side is `tcspecial::node`; the three trees carry the same names, so
//! a type is `node/i2c` wherever you are standing.
//!
//! See `tcslibgs::node` for why there are three trees rather than one.
//!
//! Not every type has one. A sensor's buffer is filled by the sensor and an
//! IIO node cannot be triggered, so what stands in for one is the kernel's
//! dummy rather than anything here.

pub mod i2c;
