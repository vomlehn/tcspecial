//! The simulations of what the vehicle carries.
//!
//! One per payload kind, each putting at the far end of a node the thing the
//! node expects to find: a pseudo-terminal where a serial line would be, a
//! socket where a network payload would be, a file where a device would be.

pub mod can;
pub mod device_file;
pub mod gpio;
pub mod i2c;
pub mod i2c_tmass;
pub mod iio;
pub mod network;
pub mod rpmsg;
pub mod serial;
pub mod spi;
pub mod spi_tmass;
