//! The stand-ins for what the vehicle carries.
//!
//! One per payload kind, each putting at the far end of a node the thing the
//! node expects to find: a pseudo-terminal where a serial line would be, a
//! socket where a network payload would be, a file where a device would be.

pub mod i2c;
pub mod spi;
pub mod can;
pub mod gpio;
pub mod iio;
pub mod rpmsg;
pub mod serial;
pub mod network;
pub mod device_file;
