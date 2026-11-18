//! The simulations of what the vehicle carries.
//!
//! One per payload kind, each putting at the far end of a node the thing the
//! node expects to find: a pseudo-terminal where a serial line would be, a
//! socket where a network payload would be, a file where a device would be.

#[cfg(target_os = "linux")]
pub mod can;
#[cfg(target_os = "linux")]
pub mod device_file;
pub mod gpio;
#[cfg(target_os = "linux")]
pub mod i2c;
pub mod i2c_tmass;
#[cfg(target_os = "linux")]
pub mod iio;
pub mod network;
#[cfg(target_os = "linux")]
pub mod rpmsg;
#[cfg(target_os = "linux")]
pub mod serial;
#[cfg(target_os = "linux")]
pub mod spi;
pub mod spi_tmass;
