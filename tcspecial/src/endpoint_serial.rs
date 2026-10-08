//! The serial endpoint: a line, opened and then set to the terms of the link.
//!
//! A device node like any other, which is how it used to be treated -- and
//! that was the fault. A serial line negotiates nothing: both ends must be
//! told the same data rate, the same byte length and the same stop bits, and a
//! line read at the wrong rate delivers bytes that are wrong rather than
//! bytes that are missing. Opening the node without setting those leaves the
//! link at whatever the port was last used for, which may be anything.
//!
//! So the terms come from the configuration and are applied here, to the port,
//! before any data moves. What cannot be applied is reported rather than
//! passed over: a handler that could not set its line is a handler that would
//! have read nonsense.
//!
//! See [`crate::endpoint`] for the traits.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};

use nix::poll::PollFlags;
use tcslibgs::{SerialConfig, StopBits, TcsError, TcsResult};

use crate::config::constants::ENDPOINT_BUFFER_SIZE;
use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable, WaitResult,
};

/// A serial line, and the terms it was set to.
pub struct SerialEndpoint {
    file: File,
    _buffer: Vec<u8>,
}

impl SerialEndpoint {
    /// Open the line and set it to what the configuration says.
    pub fn new(config: &SerialConfig) -> TcsResult<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&config.path)?;

        apply_line_terms(&file, config)?;

        Ok(Self {
            file,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }

    /// A second endpoint on the same open line.
    ///
    /// One description, shared, so that the conduit reading the line and the
    /// conduit writing it see one set of terms: the port's settings belong to
    /// the port, and setting them twice from two descriptions would be two
    /// chances to disagree.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            file: self.file.try_clone()?,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }
}

/// Set the line's data rate, byte length and stop bits.
///
/// Raw rather than canonical: a payload link carries bytes, and the line
/// discipline's editing, echo and newline translation would corrupt them. No
/// parity, because the configuration language has none to give -- a serial
/// group states the rate, the stop bits and the byte length, and nothing about
/// parity -- so the line is set to none rather than to whatever it had.
///
/// A rate the platform cannot express is reported with the rate in hand,
/// because the alternative is a line running at a rate nobody chose.
fn apply_line_terms(file: &File, config: &SerialConfig) -> TcsResult<()> {
    let fd = file.as_raw_fd();
    let speed = baud_of(config.datarate)?;

    // Read what the port is set to now, so that what is not named here is
    // left as it was rather than zeroed.
    let mut terms: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut terms) } != 0 {
        return Err(not_a_line(&config.path, "cannot be read as a serial port"));
    }

    unsafe { libc::cfmakeraw(&mut terms) };

    terms.c_cflag &= !(libc::CSIZE | libc::CSTOPB | libc::PARENB | libc::PARODD);
    terms.c_cflag |= byte_length_of(config.byte_length)?;
    match config.stop_bits {
        StopBits::One => {}
        StopBits::Two => terms.c_cflag |= libc::CSTOPB,
        StopBits::OnePointFive => return Err(stop_bits_unsupported(&config.path)),
    }
    // Read and write both: a payload link is two-way, and a port that is not
    // CLOCAL waits on carrier a payload link does not have.
    terms.c_cflag |= libc::CREAD | libc::CLOCAL;

    if unsafe { libc::cfsetispeed(&mut terms, speed) } != 0
        || unsafe { libc::cfsetospeed(&mut terms, speed) } != 0
    {
        return Err(not_a_line(
            &config.path,
            &format!("cannot be set to {} bits per second", config.datarate),
        ));
    }

    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &terms) } != 0 {
        return Err(not_a_line(&config.path, "cannot be set to those terms"));
    }

    Ok(())
}

/// One and a half stop bits is the one term a termios line cannot hold.
///
/// A UART that offers it does so for five-bit bytes, and `termios` has one
/// bit for stop bits: one, or two. A configuration asking for it is told so
/// rather than quietly given two.
fn stop_bits_unsupported(path: &str) -> TcsError {
    not_a_line(
        path,
        "cannot be set to one and a half stop bits: a serial port offers one or two",
    )
}

/// The `termios` constant for a data rate, or an error naming the rate.
fn baud_of(datarate: u32) -> TcsResult<libc::speed_t> {
    let speed = match datarate {
        50 => libc::B50,
        75 => libc::B75,
        110 => libc::B110,
        134 => libc::B134,
        150 => libc::B150,
        200 => libc::B200,
        300 => libc::B300,
        600 => libc::B600,
        1200 => libc::B1200,
        1800 => libc::B1800,
        2400 => libc::B2400,
        4800 => libc::B4800,
        9600 => libc::B9600,
        19200 => libc::B19200,
        38400 => libc::B38400,
        57600 => libc::B57600,
        115_200 => libc::B115200,
        230_400 => libc::B230400,
        460_800 => libc::B460800,
        500_000 => libc::B500000,
        576_000 => libc::B576000,
        921_600 => libc::B921600,
        1_000_000 => libc::B1000000,
        1_152_000 => libc::B1152000,
        1_500_000 => libc::B1500000,
        2_000_000 => libc::B2000000,
        2_500_000 => libc::B2500000,
        3_000_000 => libc::B3000000,
        3_500_000 => libc::B3500000,
        4_000_000 => libc::B4000000,
        other => {
            return Err(TcsError::Config(format!(
                "{other} bits per second is not a rate a serial port can be set to"
            )))
        }
    };
    Ok(speed)
}

/// The `termios` flag for a byte length.
fn byte_length_of(bits: u8) -> TcsResult<libc::tcflag_t> {
    match bits {
        5 => Ok(libc::CS5),
        6 => Ok(libc::CS6),
        7 => Ok(libc::CS7),
        8 => Ok(libc::CS8),
        other => Err(TcsError::Config(format!(
            "a byte of {other} bits is not a length a serial port offers"
        ))),
    }
}

fn not_a_line(path: &str, trouble: &str) -> TcsError {
    TcsError::Endpoint(format!("{path} {trouble}"))
}

impl EndpointWaitable for SerialEndpoint {
    fn io_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    fn wait_for_event(&self, cmd_fd: RawFd, timeout_ms: i32) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_fd(), cmd_fd, PollFlags::POLLIN, timeout_ms)
    }
}

impl EndpointReadable for SerialEndpoint {
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        self.file.read(buffer).map_err(TcsError::Io)
    }
}

impl EndpointWritable for SerialEndpoint {
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        self.file.write(data).map_err(TcsError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rate no port offers is named rather than rounded to one that is.
    #[test]
    fn a_rate_no_port_offers_is_reported() {
        assert!(baud_of(9600).is_ok());
        let e = baud_of(9601).unwrap_err();
        assert!(
            format!("{e}").contains("9601"),
            "the rate asked for is not in the complaint: {e}"
        );
    }

    /// Likewise a byte length: five to eight bits is what a UART has.
    #[test]
    fn a_byte_length_no_port_offers_is_reported() {
        for bits in 5..=8u8 {
            assert!(byte_length_of(bits).is_ok(), "{bits} bits");
        }
        let e = byte_length_of(9).unwrap_err();
        assert!(format!("{e}").contains('9'), "{e}");
    }

    /// A file that is not a serial port is reported as one that cannot be set,
    /// rather than opened and read as though the terms had been applied.
    #[test]
    fn a_file_that_is_not_a_port_is_reported() {
        let file = tempfile::NamedTempFile::new().expect("a temporary file");
        let config = SerialConfig {
            path: file.path().display().to_string(),
            datarate: 9600,
            stop_bits: StopBits::One,
            byte_length: 8,
        };

        let e = match SerialEndpoint::new(&config) {
            Ok(_) => panic!("a plain file was taken for a serial port"),
            Err(e) => e,
        };
        let said = format!("{e}");
        assert!(
            said.contains("serial port") || said.contains("those terms"),
            "a plain file was taken for a serial line: {said}"
        );
    }
}
