//! The I2C endpoint: a bus, and one device addressed on it.
//!
//! The kind of endpoint a path does not identify. Several devices sit on one
//! bus and are told apart by the address the master sends to, so opening
//! `/dev/i2c-1` says which bus and nothing about which device: the address has
//! to be given to the open bus before any transfer, which is what
//! `I2C_SLAVE` does.
//!
//! That is why an I2C endpoint could not be a plain device endpoint, and why
//! for a long time it could not be a data handler at all -- there was nowhere
//! in a handler's configuration to put the address, so a bus described in a
//! configuration file could never be run.
//!
//! See [`crate::endpoint`] for the traits.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};

use nix::poll::PollFlags;
use tcslibgs::{I2cConfig, TcsError, TcsResult};

use crate::config::constants::ENDPOINT_BUFFER_SIZE;
use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable, WaitResult,
};

/// Requests of an open I2C bus, from `linux/i2c-dev.h`.
///
/// The addressing ones are set once, when the bus is opened for a particular
/// device, and hold for every transfer on that description afterwards -- which
/// is why each handler opens the bus for itself rather than sharing one
/// description between devices.
const I2C_SLAVE: libc::c_ulong = 0x0703;
const I2C_TENBIT: libc::c_ulong = 0x0704;
const I2C_PEC: libc::c_ulong = 0x0708;

/// One device on an I2C bus.
pub struct I2cEndpoint {
    bus: File,
    _buffer: Vec<u8>,
}

impl I2cEndpoint {
    /// Open the bus and point it at the device.
    ///
    /// The order matters: ten-bit addressing is set before the address, since
    /// whether an address is in range at all depends on it. The address the
    /// configuration carries has already been checked against what the
    /// specification reserves -- see `endpoint_config_i2c` -- so what can
    /// still go wrong here is the bus, not the number.
    pub fn new(config: &I2cConfig) -> TcsResult<Self> {
        let bus = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&config.bus)?;

        let fd = bus.as_raw_fd();

        if config.ten_bit {
            request(fd, I2C_TENBIT, 1, &config.bus, "ten-bit addressing")?;
        }
        request(
            fd,
            I2C_SLAVE,
            config.address as libc::c_int,
            &config.bus,
            &format!("the address {:#04X}", config.address),
        )?;
        if config.pec {
            request(fd, I2C_PEC, 1, &config.bus, "the packet error check")?;
        }

        Ok(Self {
            bus,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }

    /// A second endpoint on the same open bus.
    ///
    /// The addressing is a property of this description, so the duplicate
    /// carries it: a conduit pair reading and writing one device must both be
    /// pointed at that device, and a second open would have to be addressed
    /// again.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            bus: self.bus.try_clone()?,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }
}

/// Make one request of the open bus, saying what was being asked for if it
/// fails.
///
/// A file that is not an I2C bus answers `ENOTTY`, which on its own says
/// nothing about what was attempted; a handler that opened the wrong node is
/// better told that it is not a bus than left talking to it.
fn request(
    fd: RawFd,
    request: libc::c_ulong,
    value: libc::c_int,
    bus: &str,
    what: &str,
) -> TcsResult<()> {
    // SAFETY: fd is open for the lifetime of the borrow, and every request
    // here takes an integer by value.
    if unsafe { libc::ioctl(fd, request, value) } < 0 {
        let why = std::io::Error::last_os_error();
        return Err(TcsError::Endpoint(format!(
            "{bus} would not take {what}: {why}"
        )));
    }
    Ok(())
}

impl EndpointWaitable for I2cEndpoint {
    fn io_fd(&self) -> RawFd {
        self.bus.as_raw_fd()
    }

    /// A bus is not pollable the way a socket is -- a read of one is a
    /// transfer the master starts -- so the wait is on the command pipe and
    /// the bus together, as for a device file, and a bus that is always ready
    /// simply makes every wait return at once.
    fn wait_for_event(&self, cmd_fd: RawFd, timeout_ms: i32) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_fd(), cmd_fd, PollFlags::POLLIN, timeout_ms)
    }
}

impl EndpointReadable for I2cEndpoint {
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        self.bus.read(buffer).map_err(TcsError::Io)
    }
}

impl EndpointWritable for I2cEndpoint {
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        self.bus.write(data).map_err(TcsError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file that is not a bus is reported, and reported as the bus it was
    /// asked to be rather than as a number nobody can place.
    #[test]
    fn a_file_that_is_not_a_bus_is_reported() {
        let file = tempfile::NamedTempFile::new().expect("a temporary file");
        let config = I2cConfig {
            bus: file.path().display().to_string(),
            address: 0x48,
            ten_bit: false,
            pec: false,
        };

        let e = match I2cEndpoint::new(&config) {
            Ok(_) => panic!("a plain file was taken for a bus"),
            Err(e) => e,
        };
        let said = format!("{e}");
        assert!(
            said.contains(&config.bus) && said.contains("0x48"),
            "the complaint names neither the bus nor the address: {said}"
        );
    }
}
