//! One module per node type, and nothing else.
//!
//! A node type is three pieces of code in three crates: what its attributes
//! are and how they are checked, which is here; the flight-side node, which is
//! `tcspecial::node`; and the stand-in the simulator puts at the far end of
//! it, which is `tcssim::node`. The three trees carry the same names, so a
//! type is `node/i2c` wherever you are standing and `grep -r node/gpio` finds
//! everything about discrete lines.
//!
//! Three parallel trees rather than one directory per type holding all three
//! pieces. A single crate per type would either link the simulator's
//! stand-ins into the flight image or hide them behind cargo features, and
//! feature combinations are configurations that can be wrong -- which trades
//! reliability for convenience, the wrong way round for this project.
//!
//! Where a type needs more than one file it gets a directory of its own:
//! `network` holds the three transports it covers, and `serial` holds the
//! two-wire variant that is one of its own terms.

pub mod i2c;
