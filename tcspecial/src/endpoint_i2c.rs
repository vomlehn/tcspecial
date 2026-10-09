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
/// What the adapter behind this bus can actually do, as a bitfield.
const I2C_FUNCS: libc::c_ulong = 0x0705;
/// One SMBus transfer, described by `SmbusTransfer` below.
const I2C_SMBUS: libc::c_ulong = 0x0720;

/// Functionality bits, from the same header. Only the three that decide how a
/// transfer is made are named.
const I2C_FUNC_I2C: libc::c_ulong = 0x0000_0001;
const I2C_FUNC_SMBUS_READ_I2C_BLOCK: libc::c_ulong = 0x0400_0000;
const I2C_FUNC_SMBUS_WRITE_I2C_BLOCK: libc::c_ulong = 0x0800_0000;

/// SMBus transfer directions and the one transfer size that carries a block.
const I2C_SMBUS_WRITE: u8 = 0;
const I2C_SMBUS_READ: u8 = 1;
const I2C_SMBUS_I2C_BLOCK_DATA: u32 = 8;

/// The most bytes one SMBus block transfer carries. A packet longer than this
/// takes several, which is what the simulator's segment size is for.
const I2C_SMBUS_BLOCK_MAX: usize = 32;

/// The register a payload's data is read from and written to.
///
/// SMBus transfers are addressed to a register within the device, and nothing
/// in an endpoint configuration says which: a payload link is a stream of
/// bytes rather than a set of named values, so one register serves as the
/// window onto it. Zero, by convention, and the simulator writes the same
/// one.
const PAYLOAD_REGISTER: u8 = 0;

/// How transfers to this device are made.
///
/// Asked of the adapter rather than assumed, because the two kinds of adapter
/// a payload link meets cannot both be addressed the same way. A controller on
/// real hardware does raw I2C transfers, where a read is a read of the device
/// and the bytes are whatever it sends. An SMBus-only adapter -- which is what
/// the kernel's `i2c-stub` is, and so what a simulated bus is -- has no raw
/// transfer at all: everything is addressed to a register, and a plain read of
/// the bus fails with `EOPNOTSUPP`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transfers {
    /// Raw I2C: a read is a read, and the device decides what it sends.
    Raw,
    /// SMBus block transfers to [`PAYLOAD_REGISTER`], at most
    /// [`I2C_SMBUS_BLOCK_MAX`] bytes at a time.
    BlockRegister,
}

impl Transfers {
    /// What an adapter advertising `funcs` can do, or why it is of no use.
    ///
    /// `on` is the handler and its bus, as [`I2cEndpoint::new`] builds it, and
    /// is only ever said back in the complaint.
    ///
    /// Raw is preferred where it is offered: it is what a real device expects,
    /// and it carries a packet of any length in one transfer.
    fn of(funcs: libc::c_ulong, on: &str) -> TcsResult<Self> {
        if funcs & I2C_FUNC_I2C != 0 {
            return Ok(Transfers::Raw);
        }

        let block = I2C_FUNC_SMBUS_READ_I2C_BLOCK | I2C_FUNC_SMBUS_WRITE_I2C_BLOCK;
        if funcs & block == block {
            return Ok(Transfers::BlockRegister);
        }

        Err(TcsError::Endpoint(format!(
            "{on} can do neither raw I2C transfers nor SMBus block transfers \
             (its adapter offers {funcs:#010X}), so there is no way to carry \
             payload data over it"
        )))
    }
}

/// One SMBus transfer, as `linux/i2c-dev.h` describes it.
#[repr(C)]
struct SmbusTransfer {
    read_write: u8,
    command: u8,
    size: u32,
    data: *mut SmbusData,
}

/// The payload of one SMBus transfer. A block carries its length in the first
/// byte, which is why this is one longer than the block maximum plus that.
#[repr(C)]
struct SmbusData {
    block: [u8; I2C_SMBUS_BLOCK_MAX + 2],
}

/// One device on an I2C bus.
pub struct I2cEndpoint {
    bus: File,
    /// How transfers are made, settled when the bus was opened.
    transfers: Transfers,
    /// Named for the complaints: an error from a transfer says which bus and
    /// device it was to.
    at: String,
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
    pub fn new(what: &str, config: &I2cConfig) -> TcsResult<Self> {
        // The handler's name and its bus together, because every complaint
        // below is about both: which payload could not be reached, and the
        // node it was to be reached over. One label, built once, so that the
        // name cannot be left off one message and put on the next.
        let on = format!("{what}: {}", config.bus);

        let bus = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&config.bus)
            .map_err(|e| TcsError::Endpoint(format!("{on} cannot be opened: {e}")))?;

        let fd = bus.as_raw_fd();

        if config.ten_bit {
            request(fd, I2C_TENBIT, 1, &on, "ten-bit addressing")?;
        }
        request(
            fd,
            I2C_SLAVE,
            config.address as libc::c_int,
            &on,
            &format!("the address {:#04X}", config.address),
        )?;
        if config.pec {
            request(fd, I2C_PEC, 1, &on, "the packet error check")?;
        }

        // What this adapter can do decides how every transfer below is
        // made, so it is asked once, here, rather than guessed at each one.
        let mut funcs: libc::c_ulong = 0;
        if unsafe { libc::ioctl(fd, I2C_FUNCS, &mut funcs) } < 0 {
            return Err(TcsError::Endpoint(format!(
                "{} will not say what it can do: {}",
                on,
                std::io::Error::last_os_error()
            )));
        }
        let transfers = Transfers::of(funcs, &on)?;

        Ok(Self {
            bus,
            transfers,
            at: format!("{on} at {:#04X}", config.address),
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }

    /// One SMBus block transfer to or from [`PAYLOAD_REGISTER`].
    ///
    /// Returns the bytes moved. A block carries its own length, so a read says
    /// how much there was and a write says how much to take.
    fn block(&mut self, read_write: u8, bytes: &mut SmbusData) -> TcsResult<usize> {
        let mut transfer = SmbusTransfer {
            read_write,
            command: PAYLOAD_REGISTER,
            size: I2C_SMBUS_I2C_BLOCK_DATA,
            data: bytes as *mut SmbusData,
        };

        // SAFETY: the bus is open, and the transfer points at a live block for
        // the duration of the call.
        if unsafe { libc::ioctl(self.bus.as_raw_fd(), I2C_SMBUS, &mut transfer) } < 0 {
            return Err(TcsError::Io(std::io::Error::last_os_error()));
        }

        Ok(bytes.block[0] as usize)
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
            transfers: self.transfers,
            at: self.at.clone(),
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
    on: &str,
    wanted: &str,
) -> TcsResult<()> {
    // SAFETY: fd is open for the lifetime of the borrow, and every request
    // here takes an integer by value.
    if unsafe { libc::ioctl(fd, request, value) } < 0 {
        let why = std::io::Error::last_os_error();
        return Err(TcsError::Endpoint(format!(
            "{on} would not take {wanted}: {why}"
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
        match self.transfers {
            Transfers::Raw => self.bus.read(buffer).map_err(TcsError::Io),
            Transfers::BlockRegister => {
                let want = buffer.len().min(I2C_SMBUS_BLOCK_MAX);
                if want == 0 {
                    return Ok(0);
                }

                let mut bytes = SmbusData {
                    block: [0u8; I2C_SMBUS_BLOCK_MAX + 2],
                };
                bytes.block[0] = want as u8;

                let got = self.block(I2C_SMBUS_READ, &mut bytes)?;
                let got = got.min(want);
                buffer[..got].copy_from_slice(&bytes.block[1..=got]);
                Ok(got)
            }
        }
    }
}

impl EndpointWritable for I2cEndpoint {
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        match self.transfers {
            Transfers::Raw => self.bus.write(data).map_err(TcsError::Io),
            Transfers::BlockRegister => {
                // As much as one block carries. The caller is a conduit, which
                // writes what it was given and counts what went, so a short
                // write is a count rather than a failure.
                let going = data.len().min(I2C_SMBUS_BLOCK_MAX);
                if going == 0 {
                    return Ok(0);
                }

                let mut bytes = SmbusData {
                    block: [0u8; I2C_SMBUS_BLOCK_MAX + 2],
                };
                bytes.block[0] = going as u8;
                bytes.block[1..=going].copy_from_slice(&data[..going]);

                self.block(I2C_SMBUS_WRITE, &mut bytes)?;
                Ok(going)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How a bus is talked to follows from what its adapter says it can do.
    ///
    /// Raw where it is offered, because that is what a real device expects and
    /// it carries a packet of any length in one transfer. SMBus block
    /// transfers where raw is not -- which is the case for the kernel's
    /// `i2c-stub`, and so for a simulated bus, whose adapter has no raw
    /// transfer at all. An adapter with neither is of no use for payload data
    /// and says so with its bitfield in hand, since that is the one fact that
    /// explains it.
    #[test]
    fn how_a_bus_is_talked_to_follows_from_what_it_can_do() {
        let raw = I2C_FUNC_I2C;
        let block = I2C_FUNC_SMBUS_READ_I2C_BLOCK | I2C_FUNC_SMBUS_WRITE_I2C_BLOCK;

        assert_eq!(Transfers::of(raw, "/dev/i2c-1").unwrap(), Transfers::Raw);
        assert_eq!(
            Transfers::of(raw | block, "/dev/i2c-1").unwrap(),
            Transfers::Raw,
            "raw is preferred where both are offered"
        );
        assert_eq!(
            Transfers::of(block, "/dev/i2c-1").unwrap(),
            Transfers::BlockRegister,
            "an SMBus-only adapter, which is what a simulated bus is"
        );

        // Half of block is not block: a bus that can be read and not written
        // carries payload data in one direction only, which a conduit pair
        // cannot use.
        for half in [I2C_FUNC_SMBUS_READ_I2C_BLOCK, I2C_FUNC_SMBUS_WRITE_I2C_BLOCK] {
            let e = match Transfers::of(half, "/dev/i2c-1") {
                Ok(t) => panic!("half a block transfer became {t:?}"),
                Err(e) => format!("{e}"),
            };
            assert!(e.contains("/dev/i2c-1"), "{e}");
        }

        let e = match Transfers::of(0, "/dev/i2c-9") {
            Ok(t) => panic!("an adapter that can do nothing became {t:?}"),
            Err(e) => format!("{e}"),
        };
        assert!(
            e.contains("/dev/i2c-9") && e.contains("0x00000000"),
            "the complaint says neither which bus nor what it offered: {e}"
        );
    }

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

        let e = match I2cEndpoint::new("instrument", &config) {
            Ok(_) => panic!("a plain file was taken for a bus"),
            Err(e) => e,
        };
        let said = format!("{e}");
        assert!(
            said.contains(&config.bus) && said.contains("0x48"),
            "the complaint names neither the bus nor the address: {said}"
        );
        // And which handler it was for: a bus is shared, and an error naming
        // only the node leaves the operator to work out whose payload it was.
        assert!(
            said.contains("instrument"),
            "the complaint does not say which handler: {said}"
        );
    }
}
