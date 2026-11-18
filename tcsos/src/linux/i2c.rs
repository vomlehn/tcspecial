//! An I2C bus through the kernel's `i2c-dev`: pointing an open bus at a
//! device, asking what its adapter can do, and SMBus block transfers. Linux
//! alone: in a browser, a device on a bus is only ever simulated.

use std::fs::File;
use std::os::fd::AsRawFd;

/// `I2C_SLAVE`: the address later transfers are to.
const I2C_SLAVE: libc::c_ulong = 0x0703;
/// `I2C_TENBIT`: ten-bit addressing.
const I2C_TENBIT: libc::c_ulong = 0x0704;
/// `I2C_PEC`: the packet error check.
const I2C_PEC: libc::c_ulong = 0x0708;
/// `I2C_FUNCS`: what the adapter can do.
const I2C_FUNCS: libc::c_ulong = 0x0705;
/// `I2C_SMBUS`: one SMBus transfer.
const I2C_SMBUS: libc::c_ulong = 0x0720;

/// What an adapter can do, from `I2C_FUNCS`: raw I2C transfers.
pub const FUNC_I2C: u64 = 0x0000_0001;
/// SMBus block reads.
pub const FUNC_SMBUS_READ_I2C_BLOCK: u64 = 0x0400_0000;
/// SMBus block writes.
pub const FUNC_SMBUS_WRITE_I2C_BLOCK: u64 = 0x0800_0000;

/// A transfer's direction: a write.
const SMBUS_WRITE: u8 = 0;
/// A read.
const SMBUS_READ: u8 = 1;
/// A transfer's size: an I2C block.
const SMBUS_I2C_BLOCK_DATA: u32 = 8;

/// The most bytes one SMBus block carries.
pub const BLOCK_MAX: usize = 32;

/// One SMBus transfer, as `linux/i2c-dev.h` describes it.
#[repr(C)]
struct SmbusTransfer {
    /// [`SMBUS_READ`] or [`SMBUS_WRITE`].
    read_write: u8,
    /// The register.
    command: u8,
    /// What kind of transfer.
    size: u32,
    /// The data.
    data: *mut SmbusData,
}

/// The data of one SMBus transfer. A block carries its length in the first
/// byte, which is why this is one longer than the block maximum plus that.
#[repr(C)]
struct SmbusData {
    /// The length, then the bytes.
    block: [u8; BLOCK_MAX + 2],
}

/// Makes one request of an open bus that takes an integer.
///
/// # Arguments
///
/// * `bus`     - The open bus.
/// * `request` - The request.
/// * `value`   - The integer it takes.
///
/// # Errors
///
/// What the bus said: `ENOTTY` from a file that is not a bus.
fn request(
    bus: &File,
    request: libc::c_ulong,
    value: libc::c_int,
) -> std::io::Result<()> {
    // SAFETY: the bus is open for the call, and every request here takes an
    // integer by value.
    if unsafe { libc::ioctl(bus.as_raw_fd(), request, value) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Has a bus take ten-bit addresses, which must come before the address.
///
/// # Arguments
///
/// * `bus` - The open bus.
///
/// # Errors
///
/// What the bus said.
pub fn set_ten_bit(bus: &File) -> std::io::Result<()> {
    request(bus, I2C_TENBIT, 1)
}

/// Points a bus at a device: later transfers are to it.
///
/// # Arguments
///
/// * `bus`     - The open bus.
/// * `address` - The device's address.
///
/// # Errors
///
/// What the bus said.
pub fn set_address(bus: &File, address: u16) -> std::io::Result<()> {
    request(bus, I2C_SLAVE, libc::c_int::from(address))
}

/// Has a bus add the packet error check to transfers.
///
/// # Arguments
///
/// * `bus` - The open bus.
///
/// # Errors
///
/// What the bus said.
pub fn set_pec(bus: &File) -> std::io::Result<()> {
    request(bus, I2C_PEC, 1)
}

/// What a bus's adapter can do.
///
/// # Arguments
///
/// * `bus` - The open bus.
///
/// # Returns
///
/// The adapter's functions, as bits: [`FUNC_I2C`] and the rest.
///
/// # Errors
///
/// What the bus said, where it will not say.
pub fn functions(bus: &File) -> std::io::Result<u64> {
    let mut funcs: libc::c_ulong = 0;
    // SAFETY: I2C_FUNCS needs an open i2c-dev descriptor and a pointer to an
    // unsigned long it can write; `funcs` is a local one.
    if unsafe { libc::ioctl(bus.as_raw_fd(), I2C_FUNCS, &mut funcs) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // An unsigned long, which is 32 bits on a 32-bit system and 64 here.
    #[allow(clippy::useless_conversion)]
    Ok(u64::from(funcs))
}

/// Makes one SMBus block transfer to or from a register.
///
/// # Arguments
///
/// * `bus`        - The open bus, pointed at the device.
/// * `read_write` - [`SMBUS_READ`] or [`SMBUS_WRITE`].
/// * `register`   - The register.
/// * `bytes`      - The block: its first byte the length. Filled by a
///                  read, taken from by a write.
///
/// # Errors
///
/// What the bus said.
fn block(
    bus: &File,
    read_write: u8,
    register: u8,
    bytes: &mut SmbusData,
) -> std::io::Result<()> {
    let mut transfer = SmbusTransfer {
        read_write,
        command: register,
        size: SMBUS_I2C_BLOCK_DATA,
        data: bytes as *mut SmbusData,
    };
    // SAFETY: the bus is open, and the transfer points at a live block for
    // the duration of the call.
    if unsafe { libc::ioctl(bus.as_raw_fd(), I2C_SMBUS, &mut transfer) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Reads one SMBus block from a register.
///
/// # Arguments
///
/// * `bus`      - The open bus, pointed at the device.
/// * `register` - The register.
/// * `into`     - Where the bytes go: as many as fit, [`BLOCK_MAX`] at most.
///
/// # Returns
///
/// How many bytes were read.
///
/// # Errors
///
/// What the bus said.
pub fn read_block(
    bus: &File,
    register: u8,
    into: &mut [u8],
) -> std::io::Result<usize> {
    let want = into.len().min(BLOCK_MAX);
    if want == 0 {
        return Ok(0);
    }
    let mut bytes = SmbusData {
        block: [0u8; BLOCK_MAX + 2],
    };
    bytes.block[0] = want as u8;
    block(bus, SMBUS_READ, register, &mut bytes)?;
    let got = (bytes.block[0] as usize).min(want);
    into[..got].copy_from_slice(&bytes.block[1..=got]);
    Ok(got)
}

/// Writes one SMBus block to a register.
///
/// # Arguments
///
/// * `bus`      - The open bus, pointed at the device.
/// * `register` - The register.
/// * `data`     - The bytes: as many as one block carries, [`BLOCK_MAX`] at
///                most.
///
/// # Returns
///
/// How many bytes were written.
///
/// # Errors
///
/// What the bus said.
pub fn write_block(
    bus: &File,
    register: u8,
    data: &[u8],
) -> std::io::Result<usize> {
    let going = data.len().min(BLOCK_MAX);
    if going == 0 {
        return Ok(0);
    }
    let mut bytes = SmbusData {
        block: [0u8; BLOCK_MAX + 2],
    };
    bytes.block[0] = going as u8;
    bytes.block[1..=going].copy_from_slice(&data[..going]);
    block(bus, SMBUS_WRITE, register, &mut bytes)?;
    Ok(going)
}

/// Makes one SMBus block transfer on a raw block, as the kernel holds it:
/// its first byte the length, then the data. For the far end of a bus,
/// which reads and writes the whole bank a register is a window onto.
///
/// # Arguments
///
/// * `bus`      - The open bus, pointed at the device.
/// * `write`    - Whether it is a write; a read otherwise.
/// * `register` - The register.
/// * `raw`      - The block: filled by a read, taken from by a write.
///
/// # Errors
///
/// What the bus said.
pub fn transfer_block(
    bus: &File,
    write: bool,
    register: u8,
    raw: &mut [u8; BLOCK_MAX + 2],
) -> std::io::Result<()> {
    let mut bytes = SmbusData { block: *raw };
    let read_write = if write { SMBUS_WRITE } else { SMBUS_READ };
    block(bus, read_write, register, &mut bytes)?;
    *raw = bytes.block;
    Ok(())
}
