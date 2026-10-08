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
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::{AsRawFd, RawFd};

use log::info;
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

/// Whether the node at the end of the path is a peripheral or a stand-in.
///
/// A spidev node takes the requests above and clocks a peripheral on the
/// terms they set. A terminal takes none of them -- it has no clock to set --
/// and is what tcssim stands in for a peripheral with, there being no module
/// that emulates one. The bytes of such a link are real and its clocking does
/// not exist, so a handler on one is told what it has, once, rather than left
/// to infer it from transfers that work while the terms did not take.
///
/// Nothing else is accepted. A regular file, or a character device that is
/// neither of these, is a path naming something other than what the
/// configuration describes, and opening it would be a handler talking
/// confidently to the wrong thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Clocking {
    /// A real spidev node, set to the configured terms.
    Peripheral,
    /// A terminal standing in for one: the bytes without the clock.
    StandIn,
}

/// Take a terminal's line discipline out of the way, so that what it carries
/// is bytes: no editing, no echo, no translation of what looks like a line.
///
/// The same thing the serial endpoint does to a port, and for the same reason,
/// but not for the same purpose: there it is part of setting the line to the
/// terms the configuration gives, and here there are no terms -- it is only
/// the discipline that would otherwise sit between the two ends of a link
/// carrying payload data.
fn make_raw(fd: RawFd, path: &str) -> TcsResult<()> {
    let mut terms: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut terms) } != 0 {
        return Err(TcsError::Endpoint(format!(
            "{path} is a terminal whose terms cannot be read: {}",
            std::io::Error::last_os_error()
        )));
    }

    unsafe { libc::cfmakeraw(&mut terms) };

    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &terms) } != 0 {
        return Err(TcsError::Endpoint(format!(
            "{path} is a terminal that cannot be set raw: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

/// Whether this descriptor is a terminal, which is what a stand-in for a
/// peripheral is.
fn is_a_terminal(fd: RawFd) -> bool {
    // SAFETY: isatty reads the descriptor's kind and nothing else.
    unsafe { libc::isatty(fd) == 1 }
}

/// One SPI peripheral, on the terms its configuration gives.
pub struct SpiEndpoint {
    device: File,
    /// What this turned out to be, settled when it was opened.
    #[allow(dead_code)]
    clocking: Clocking,
    _buffer: Vec<u8>,
}

impl SpiEndpoint {
    /// Open the device and set the controller to the configured terms.
    pub fn new(config: &SpiConfig) -> TcsResult<Self> {
        let device = OpenOptions::new()
            .read(true)
            .write(true)
            // Not this process's terminal. A device that is a tty -- a real
            // serial port, or a pty standing in for one -- becomes the
            // controlling terminal of a process that opens it without this,
            // and then a read from a background process group raises SIGTTIN
            // and a hangup raises SIGHUP: a handler stopped or killed for
            // reasons that have nothing to do with its payload.
            .custom_flags(libc::O_NOCTTY)
            .open(&config.path)?;

        let fd = device.as_raw_fd();

        // The mode first, and what happens to it decides the rest. A node
        // that refuses it is not a spidev: a terminal is a stand-in to carry
        // bytes on, and anything else is a configuration naming the wrong
        // path.
        match set_u8(fd, SPI_IOC_WR_MODE, mode_bits(config), &config.path, "its mode") {
            Ok(()) => {}
            Err(_) if is_a_terminal(fd) => {
                // The one term that means anything on a terminal: take the
                // line discipline out of the way. Left canonical, the
                // discipline holds bytes back until a newline arrives, and
                // payload data has no lines in it -- so a handler would sit on
                // a read that never returned while the bytes it wanted were
                // being edited on its behalf.
                make_raw(fd, &config.path)?;

                info!(
                    "{} is a terminal standing in for a SPI peripheral: it will carry \
                     bytes raw, and the mode, clock rate and word width in the \
                     configuration are not applied to anything",
                    config.path
                );
                return Ok(Self {
                    device,
                    clocking: Clocking::StandIn,
                    _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
                });
            }
            Err(e) => return Err(e),
        }
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
            clocking: Clocking::Peripheral,
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
            clocking: self.clocking,
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

    /// A terminal is taken as a stand-in for a peripheral, and carries bytes.
    ///
    /// Which is what tcssim offers: nothing emulates a SPI peripheral, so what
    /// it puts at the end of the path is a pty. The terms do not take -- a pty
    /// has no clock to set -- and the handler says so and carries the bytes
    /// anyway, because the bytes are the part a stand-in can be honest about.
    #[test]
    fn a_terminal_is_taken_as_a_stand_in_and_carries_bytes() {
        use std::io::Write;
        use std::os::unix::io::FromRawFd;

        let master_fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
        assert!(master_fd >= 0, "no pty: {}", std::io::Error::last_os_error());
        let mut master = unsafe { File::from_raw_fd(master_fd) };
        assert_eq!(unsafe { libc::grantpt(master_fd) }, 0);
        assert_eq!(unsafe { libc::unlockpt(master_fd) }, 0);

        let mut name = [0 as libc::c_char; 128];
        assert_eq!(
            unsafe { libc::ptsname_r(master_fd, name.as_mut_ptr(), name.len()) },
            0
        );
        let slave = unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) }
            .to_string_lossy()
            .into_owned();

        let config = SpiConfig {
            path: slave.clone(),
            max_speed: 1_000_000,
            mode: SpiMode::Mode0,
            bits_per_word: 8,
            bit_order: BitOrder::MsbFirst,
            cs_active: CsActive::Low,
        };
        let mut endpoint = SpiEndpoint::new(&config)
            .unwrap_or_else(|e| panic!("{slave} was not taken as a stand-in: {e}"));
        assert_eq!(
            endpoint.clocking,
            Clocking::StandIn,
            "a pty was mistaken for a peripheral"
        );

        master
            .write_all(b"from the peripheral")
            .expect("the far end writes");
        master.flush().ok();
        let mut buffer = [0u8; 64];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut got = Vec::new();
        while got.len() < b"from the peripheral".len()
            && std::time::Instant::now() < deadline
        {
            match endpoint.read(&mut buffer) {
                Ok(n) if n > 0 => got.extend_from_slice(&buffer[..n]),
                _ => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        }

        assert_eq!(got, b"from the peripheral");
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
        // Narrowly: a terminal is a stand-in and a plain file is not. A
        // handler that accepted anything would open the wrong path
        // confidently.
    }
}
