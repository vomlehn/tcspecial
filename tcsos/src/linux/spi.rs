//! A SPI peripheral through the kernel's `spidev`: setting the terms it is
//! clocked on. Linux alone: in a browser, a peripheral is only ever
//! simulated.

use std::fs::File;
use std::os::fd::AsRawFd;

/// Requests of an open spidev node, from `linux/spi/spidev.h`.
///
/// Each is an `_IOW('k', n, T)`: the direction and the size of the argument
/// are encoded in the number, which is why the mode requests differ by the
/// width of what they take rather than only by their index.
const WR_MODE: libc::c_ulong = 0x4001_6B01;
const WR_LSB_FIRST: libc::c_ulong = 0x4001_6B02;
const WR_BITS_PER_WORD: libc::c_ulong = 0x4001_6B03;
const WR_MAX_SPEED_HZ: libc::c_ulong = 0x4004_6B04;

/// Mode bits, from the same header: clock phase.
pub const CPHA: u8 = 0x01;
/// Clock polarity.
pub const CPOL: u8 = 0x02;
/// Chip select active high.
pub const CS_HIGH: u8 = 0x04;

/// Makes one request of an open spidev node.
///
/// # Arguments
///
/// * `device`  - The open node.
/// * `request` - The request.
/// * `value`   - What to set, as wide as the request encodes: each caller
///               here passes the type its request names.
///
/// # Errors
///
/// What the node said: `ENOTTY` from a node that is not a spidev.
fn set<T>(
    device: &File,
    request: libc::c_ulong,
    value: &T,
) -> std::io::Result<()> {
    debug_assert_eq!(argument_bytes(request), std::mem::size_of::<T>());
    // SAFETY: the pointer is to `value`, alive and as wide as the request
    // says; the kernel only reads it, for a write request.
    let value = value as *const T as *const libc::c_void;
    if unsafe { libc::ioctl(device.as_raw_fd(), request, value) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// How many bytes an ioctl request says its argument is: the size field of
/// the request number, as `_IOC_SIZE` reads it.
///
/// # Arguments
///
/// * `request` - The request.
///
/// # Returns
///
/// The size.
fn argument_bytes(request: libc::c_ulong) -> usize {
    ((request >> 16) & 0x1fff) as usize
}

/// Sets the mode byte: [`CPHA`], [`CPOL`] and [`CS_HIGH`].
///
/// # Arguments
///
/// * `device` - The open node.
/// * `bits`   - The byte.
///
/// # Errors
///
/// What the node said.
pub fn set_mode(device: &File, bits: u8) -> std::io::Result<()> {
    set(device, WR_MODE, &bits)
}

/// Sets whether the least significant bit goes first.
///
/// # Arguments
///
/// * `device` - The open node.
/// * `first`  - Whether it does.
///
/// # Errors
///
/// What the node said.
pub fn set_lsb_first(device: &File, first: bool) -> std::io::Result<()> {
    set(device, WR_LSB_FIRST, &u8::from(first))
}

/// Sets the bits in a word.
///
/// # Arguments
///
/// * `device` - The open node.
/// * `bits`   - How many.
///
/// # Errors
///
/// What the node said.
pub fn set_bits_per_word(device: &File, bits: u8) -> std::io::Result<()> {
    set(device, WR_BITS_PER_WORD, &bits)
}

/// Sets the fastest the peripheral is clocked.
///
/// # Arguments
///
/// * `device` - The open node.
/// * `hz`     - The rate, in Hz.
///
/// # Errors
///
/// What the node said.
pub fn set_max_speed(device: &File, hz: u32) -> std::io::Result<()> {
    set(device, WR_MAX_SPEED_HZ, &hz)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each request is as wide as the value its function passes: one byte
    /// for the mode, the bit order and the word size, four for the speed.
    #[test]
    fn each_request_is_as_wide_as_its_value() {
        for request in [WR_MODE, WR_LSB_FIRST, WR_BITS_PER_WORD] {
            assert_eq!(argument_bytes(request), 1, "{request:#x}");
        }
        assert_eq!(argument_bytes(WR_MAX_SPEED_HZ), 4);
    }
}
