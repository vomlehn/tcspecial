//! What the vehicle carries, and the ground link with it.
//!
//! A device on a bus, a line the vehicle drives, a link to something aboard.
//! These are the types the configuration's `payloads` section holds -- the
//! part that changes between missions, which is why the tree is divided this
//! way.
//!
//! Two of these are payloads that are not devices: `app_pl` and `app_all_pl`
//! are programs the mission wrote, run by tcspecial and read the way a sensor
//! is read. What a file may say about either is `tcslibgs::node::app`, that
//! rule being one rule for all five app node types, and the interfaces their
//! programs are run with are `tcsapp`.
//!
//! `network` is here and serves two sections: a network payload and the ground
//! link are the same endpoint code, a socket being a socket whichever end of
//! the mission is at the other end of it. That is why there is no `moc`
//! directory -- see `tcslibgs::node`.

pub mod can;
pub mod device_file;
pub mod gpio;
pub mod hdlc;
pub mod i2c;
pub mod iio;
pub mod network;
pub mod rpmsg;
pub mod serial;
pub mod spi;
// A payload that is a program rather than a device: one that is asked for a
// reading, and one that is never asked.
pub mod app_all_pl;
pub mod app_pl;
