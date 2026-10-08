//! The SPI endpoint: a peripheral, and the terms it is clocked on.
//!
//! The device node names the bus and the chip select together, so unlike I2C
//! there is no address beside it. What there is instead is a set of terms the
//! controller must be set to before a transfer: which edges of the clock
//! shift and sample, how fast, how wide a word is, and which end of it goes
//! first. None of it is negotiated, and a peripheral clocked on the wrong
//! terms returns data rather than an error.
//!
//! That is why a SPI endpoint is not a plain device endpoint: opening the node
//! and reading it leaves the controller on whatever terms it was last used
//! with, which belong to whatever used it last.
//!
//! See [`crate::endpoint`] for the traits.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};

use nix::poll::PollFlags;
use tcslibgs::{BitOrder, CsActive, SpiConfig, SpiMode, TcsError, TcsResult};

use crate::config::constants::ENDPOINT_BUFFER_SIZE;
use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable, WaitResult,
};

/// Requests of an open spidev node, from `linux/spi/spidev.h`.
///
/// Each is an `_IOW('k', n, T)`: the direction and the size of the argument
/// are encoded in the number, which is why the mode requests differ by the
/// width of what they take rather than only by their index.
const SPI_IOC_WR_MODE: libc::c_ulong = 0x4001_6B01;
const SPI_IOC_WR_LSB_FIRST: libc::c_ulong = 0x4001_6B02;
const SPI_IOC_WR_BITS_PER_WORD: libc::c_ulong = 0x4001_6B03;
const SPI_IOC_WR_MAX_SPEED_HZ: libc::c_ulong = 0x4004_6B04;

/// Mode bits, from the same header. The two low bits are the mode proper --
/// clock polarity and clock phase -- and the chip select polarity is a bit
/// beside them.
const SPI_CPHA: u8 = 0x01;
const SPI_CPOL: u8 = 0x02;
const SPI_CS_HIGH: u8 = 0x04;

/// One SPI peripheral, on the terms its configuration gives.
pub struct SpiEndpoint {
    device: File,
    _buffer: Vec<u8>,
}

impl SpiEndpoint {
    /// Open the device and set the controller to the configured terms.
    pub fn new(config: &SpiConfig) -> TcsResult<Self> {
        let device = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&config.path)?;

        let fd = device.as_raw_fd();

        set_u8(fd, SPI_IOC_WR_MODE, mode_bits(config), &config.path, "its mode")?;
        set_u8(
            fd,
            SPI_IOC_WR_BITS_PER_WORD,
            config.bits_per_word,
            &config.path,
            &format!("{} bits per word", config.bits_per_word),
        )?;
        set_u8(
            fd,
            SPI_IOC_WR_LSB_FIRST,
            u8::from(matches!(config.bit_order, BitOrder::LsbFirst)),
            &config.path,
            "its bit order",
        )?;
        set_u32(
            fd,
            SPI_IOC_WR_MAX_SPEED_HZ,
            config.max_speed,
            &config.path,
            &format!("{} Hz", config.max_speed),
        )?;

        Ok(Self {
            device,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }

    /// A second endpoint on the same open device.
    ///
    /// The terms belong to the description, as an I2C address does, so the
    /// duplicate carries what was set rather than setting it again.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            device: self.device.try_clone()?,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }
}

/// The mode byte: the clock terms, and the chip select polarity beside them.
fn mode_bits(config: &SpiConfig) -> u8 {
    let mut bits = match config.mode {
        SpiMode::Mode0 => 0,
        SpiMode::Mode1 => SPI_CPHA,
        SpiMode::Mode2 => SPI_CPOL,
        SpiMode::Mode3 => SPI_CPOL | SPI_CPHA,
    };
    if let CsActive::High = config.cs_active {
        bits |= SPI_CS_HIGH;
    }
    bits
}

fn set_u8(fd: RawFd, request: libc::c_ulong, value: u8, path: &str, what: &str) -> TcsResult<()> {
    set(fd, request, &value as *const u8 as *const libc::c_void, path, what)
}

fn set_u32(fd: RawFd, request: libc::c_ulong, value: u32, path: &str, what: &str) -> TcsResult<()> {
    set(fd, request, &value as *const u32 as *const libc::c_void, path, what)
}

/// Make one request of the open device, saying what was being set if it fails.
///
/// A node that is not a spidev answers `ENOTTY`, which says nothing on its
/// own about what was attempted.
fn set(
    fd: RawFd,
    request: libc::c_ulong,
    value: *const libc::c_void,
    path: &str,
    what: &str,
) -> TcsResult<()> {
    // SAFETY: fd is open for the lifetime of the borrow, and the pointer is
    // to a live value of the width the request's number encodes.
    if unsafe { libc::ioctl(fd, request, value) } < 0 {
        let why = std::io::Error::last_os_error();
        return Err(TcsError::Endpoint(format!(
            "{path} would not take {what}: {why}"
        )));
    }
    Ok(())
}

impl EndpointWaitable for SpiEndpoint {
    fn io_fd(&self) -> RawFd {
        self.device.as_raw_fd()
    }

    /// As for an I2C bus: a transfer is one the controller starts, so there is
    /// nothing to wait for on the device itself, and the wait is really on the
    /// command pipe.
    fn wait_for_event(&self, cmd_fd: RawFd, timeout_ms: i32) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_fd(), cmd_fd, PollFlags::POLLIN, timeout_ms)
    }
}

impl EndpointReadable for SpiEndpoint {
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        self.device.read(buffer).map_err(TcsError::Io)
    }
}

impl EndpointWritable for SpiEndpoint {
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        self.device.write(data).map_err(TcsError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four modes are the two clock bits, and the chip select polarity is
    /// a bit beside them rather than part of the mode.
    #[test]
    fn the_mode_byte_is_the_clock_terms_and_the_chip_select() {
        let of = |mode, cs| {
            mode_bits(&SpiConfig {
                path: "/dev/spidev0.0".to_string(),
                max_speed: 1_000_000,
                mode,
                bits_per_word: 8,
                bit_order: BitOrder::MsbFirst,
                cs_active: cs,
            })
        };

        assert_eq!(of(SpiMode::Mode0, CsActive::Low), 0);
        assert_eq!(of(SpiMode::Mode1, CsActive::Low), SPI_CPHA);
        assert_eq!(of(SpiMode::Mode2, CsActive::Low), SPI_CPOL);
        assert_eq!(of(SpiMode::Mode3, CsActive::Low), SPI_CPOL | SPI_CPHA);
        assert_eq!(
            of(SpiMode::Mode0, CsActive::High),
            SPI_CS_HIGH,
            "an active-high chip select is not a mode of its own"
        );
    }

    /// A file that is not a spidev node is reported, naming what was being set.
    #[test]
    fn a_file_that_is_not_a_spi_device_is_reported() {
        let file = tempfile::NamedTempFile::new().expect("a temporary file");
        let config = SpiConfig {
            path: file.path().display().to_string(),
            max_speed: 1_000_000,
            mode: SpiMode::Mode0,
            bits_per_word: 8,
            bit_order: BitOrder::MsbFirst,
            cs_active: CsActive::Low,
        };

        let e = match SpiEndpoint::new(&config) {
            Ok(_) => panic!("a plain file was taken for a SPI device"),
            Err(e) => e,
        };
        let said = format!("{e}");
        assert!(
            said.contains(&config.path) && said.contains("mode"),
            "the complaint names neither the device nor what was being set: {said}"
        );
    }
}
