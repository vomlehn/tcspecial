//! What the vehicle carries, and the ground link with it.
//!
//! A device on a bus, a line the vehicle drives, a link to something aboard.
//! These are the types the configuration's `payloads` section holds -- the
//! part that changes between missions, which is why the tree is divided this
//! way.
//!
//! `network` is here and serves two sections: a network payload and the ground
//! link are the same endpoint code, a socket being a socket whichever end of
//! the mission is at the other end of it. That is why there is no `moc`
//! directory -- see `tcslibgs::node`.

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
