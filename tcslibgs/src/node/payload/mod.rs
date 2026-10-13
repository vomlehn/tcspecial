//! What the vehicle carries: the attributes, and how they are checked.
//!
//! A device on a bus, a line it drives, a link to something aboard. These are
//! the types the configuration's `payloads` section holds, and the ones that
//! change between missions -- which is the reason the tree is divided this
//! way: what the vehicle carries this flight is read, reviewed and replaced as
//! a set.
//!
//! What opens one is `tcspecial::node::payload`, and what stands in for one
//! when there is no hardware is `tcssim::node::payload`.

pub mod i2c;
pub mod spi;
pub mod can;
pub mod gpio;
pub mod hdlc;
pub mod iio;
pub mod rpmsg;
pub mod serial;
