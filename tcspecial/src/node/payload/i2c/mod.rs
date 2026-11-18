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

use std::io::{Read, Write};
use tcsos::fs::{File, OpenOptions};
use tcsos::wait::{Ready, Source, Waitable};

use tcsprotocol::{I2cConfig, TcsError, TcsResult};

use crate::config::constants::ENDPOINT_BUFFER_SIZE;
use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable,
    WaitResult,
};

use tcsos::linux::i2c::{
    FUNC_I2C, FUNC_SMBUS_READ_I2C_BLOCK, FUNC_SMBUS_WRITE_I2C_BLOCK,
};

/// SMBus transfers are addressed to a register within the device, and nothing
/// in a device configuration says which: a device link is a stream of
/// bytes rather than a set of named values, so one register serves as the
/// window onto it. Zero, by convention, and the simulator writes the same
/// one.
const DATA_REGISTER: u8 = 0;

/// How transfers to this device are made.
///
/// Asked of the adapter rather than assumed, because the two kinds of adapter
/// a device link meets cannot both be addressed the same way. A controller on
/// real hardware does raw I2C transfers, where a read is a read of the device
/// and the bytes are whatever it sends. An SMBus-only adapter -- which is what
/// the kernel's `i2c-stub` is, and so what a simulated bus is -- has no raw
/// transfer at all: everything is addressed to a register, and a plain read of
/// the bus fails with `EOPNOTSUPP`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transfers {
    /// Raw I2C: a read is a read, and the device decides what it sends.
    Raw,
    /// SMBus block transfers to [`DATA_REGISTER`], at most
    /// `tcsos::linux::i2c::BLOCK_MAX` bytes at a time.
    BlockRegister,
}

impl Transfers {
    /// Says what an adapter advertising `funcs` can do, or why it is of no
    /// use.
    ///
    /// Raw is preferred where it is offered: it is what a real device expects,
    /// and it carries a packet of any length in one transfer.
    ///
    /// # Arguments
    ///
    /// * `funcs` - What the adapter answered to `I2C_FUNCS`.
    /// * `on`    - The handler and its bus, as [`I2cEndpoint::new`] builds
    ///             it, only ever said back in the complaint.
    ///
    /// # Returns
    ///
    /// How transfers to the device are to be made.
    ///
    /// # Errors
    ///
    /// `TcsError::Endpoint` if the adapter can do neither raw I2C nor SMBus
    /// block transfers.
    fn of(funcs: u64, on: &str) -> TcsResult<Self> {
        if funcs & FUNC_I2C != 0 {
            return Ok(Transfers::Raw);
        }

        let block = FUNC_SMBUS_READ_I2C_BLOCK | FUNC_SMBUS_WRITE_I2C_BLOCK;
        if funcs & block == block {
            return Ok(Transfers::BlockRegister);
        }

        Err(TcsError::Endpoint(format!(
            "{on} can do neither raw I2C transfers nor SMBus block transfers \
             (its adapter offers {funcs:#010X}), so there is no way to carry \
             device data over it"
        )))
    }
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
    /// Opens the bus and points it at the device.
    ///
    /// The order matters: ten-bit addressing is set before the address, since
    /// whether an address is in range at all depends on it. The address the
    /// configuration carries has already been checked against what the
    /// specification reserves -- see `node::payload::i2c` -- so what can
    /// still go wrong here is the bus, not the number.
    ///
    /// # Arguments
    ///
    /// * `what`   - The node's name, for the complaints.
    /// * `config` - The node's settings: the bus, the device's address, and
    ///              whether it takes ten-bit addressing and the packet error
    ///              check.
    ///
    /// # Returns
    ///
    /// The endpoint, the bus open and addressed to the device.
    ///
    /// # Errors
    ///
    /// `TcsError::Endpoint`, naming the node and the bus, if the bus cannot
    /// be opened, will not take one of the settings, will not say what it
    /// can do, or can do no transfer a device link can use.
    pub fn new(what: &str, config: &I2cConfig) -> TcsResult<Self> {
        // The handler's name and its bus together, because every complaint
        // below is about both: which device could not be reached, and the
        // node it was to be reached over. One label, built once, so that the
        // name cannot be left off one message and put on the next.
        let on = format!("{what}: {}", config.bus);

        let bus = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&config.bus)
            .map_err(|e| {
                TcsError::Endpoint(format!("{on} cannot be opened: {e}"))
            })?;
        crate::exclusive::on_bus(what, &config.bus, config.address, &bus)?;

        // A file that is not an I2C bus answers `ENOTTY`, which on its own
        // says nothing about what was attempted; a handler that opened the
        // wrong node is better told that it is not a bus than left talking
        // to it, so each refusal says what was being asked for.
        let refused = |wanted: &str, why: std::io::Error| {
            TcsError::Endpoint(format!("{on} would not take {wanted}: {why}"))
        };
        if config.ten_bit {
            tcsos::linux::i2c::set_ten_bit(&bus)
                .map_err(|e| refused("ten-bit addressing", e))?;
        }
        tcsos::linux::i2c::set_address(&bus, config.address).map_err(|e| {
            refused(&format!("the address {:#04X}", config.address), e)
        })?;
        if config.pec {
            tcsos::linux::i2c::set_pec(&bus)
                .map_err(|e| refused("the packet error check", e))?;
        }

        // What this adapter can do decides how every transfer below is
        // made, so it is asked once, here, rather than guessed at each one.
        let funcs = tcsos::linux::i2c::functions(&bus).map_err(|e| {
            TcsError::Endpoint(format!("{on} will not say what it can do: {e}"))
        })?;
        let transfers = Transfers::of(funcs, &on)?;

        Ok(Self {
            bus,
            transfers,
            at: format!("{on} at {:#04X}", config.address),
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }

    /// Makes a second endpoint on the same open bus.
    ///
    /// The addressing is a property of this description, so the duplicate
    /// carries it: a conduit pair reading and writing one device must both be
    /// pointed at that device, and a second open would have to be addressed
    /// again.
    ///
    /// # Returns
    ///
    /// The second endpoint.
    ///
    /// # Errors
    ///
    /// The I/O error if the descriptor cannot be duplicated.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            bus: self.bus.try_clone()?,
            transfers: self.transfers,
            at: self.at.clone(),
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }
}

impl EndpointWaitable for I2cEndpoint {
    fn io_source(&self) -> Source {
        self.bus.source()
    }

    /// A bus is not pollable the way a socket is -- a read of one is a
    /// transfer the master starts -- so the wait is on the command pipe and
    /// the bus together, as for a device file, and a bus that is always ready
    /// simply makes every wait return at once.
    fn wait_for_event(
        &self,
        cmd: Source,
        timeout_ms: i32,
    ) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_source(), cmd, Ready::READ, timeout_ms)
    }
}

impl EndpointReadable for I2cEndpoint {
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        match self.transfers {
            Transfers::Raw => self.bus.read(buffer).map_err(TcsError::Io),
            Transfers::BlockRegister => {
                tcsos::linux::i2c::read_block(&self.bus, DATA_REGISTER, buffer)
                    .map_err(TcsError::Io)
            }
        }
    }
}

impl EndpointWritable for I2cEndpoint {
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        match self.transfers {
            Transfers::Raw => self.bus.write(data).map_err(TcsError::Io),
            // As much as one block carries. The caller is a conduit, which
            // writes what it was given and counts what went, so a short write
            // is a count rather than a failure.
            Transfers::BlockRegister => {
                tcsos::linux::i2c::write_block(&self.bus, DATA_REGISTER, data)
                    .map_err(TcsError::Io)
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
    /// transfer at all. An adapter with neither is of no use for device data
    /// and says so with its bitfield in hand, since that is the one fact that
    /// explains it.
    #[test]
    fn how_a_bus_is_talked_to_follows_from_what_it_can_do() {
        let raw = FUNC_I2C;
        let block = FUNC_SMBUS_READ_I2C_BLOCK | FUNC_SMBUS_WRITE_I2C_BLOCK;

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
        // carries device data in one direction only, which a conduit pair
        // cannot use.
        for half in [FUNC_SMBUS_READ_I2C_BLOCK, FUNC_SMBUS_WRITE_I2C_BLOCK] {
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
        // only the node leaves the operator to work out whose device it was.
        assert!(
            said.contains("instrument"),
            "the complaint does not say which handler: {said}"
        );
    }
}
