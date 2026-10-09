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
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::{AsRawFd, RawFd};

use nix::poll::PollFlags;
use tcslibgs::{Parity, SerialConfig, StopBits, TcsError, TcsResult};

use crate::config::constants::ENDPOINT_BUFFER_SIZE;
use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable, WaitResult,
};

/// The flags that ask a port for this parity.
///
/// Mark and space are the two that need CMSPAR: they send a constant bit
/// rather than one computed from the data, which is a protocol marking a
/// frame rather than checking one, and the flag that asks for it is Linux's
/// own. The two differ only in which constant, which is PARODD.
///
/// Paired with [`parity_of`], which reads the same flags back: no port
/// available to a test will take a parity at all -- a pseudo-terminal is the
/// only line a test has -- so the two being inverses is what can be checked.
fn parity_flags(parity: Parity) -> libc::tcflag_t {
    match parity {
        Parity::None => 0,
        Parity::Even => libc::PARENB,
        Parity::Odd => libc::PARENB | libc::PARODD,
        Parity::Mark => libc::PARENB | libc::CMSPAR | libc::PARODD,
        Parity::Space => libc::PARENB | libc::CMSPAR,
    }
}

/// Which parity a port's flags say it has.
fn parity_of(flags: libc::tcflag_t) -> Parity {
    if flags & libc::PARENB == 0 {
        return Parity::None;
    }
    match (flags & libc::CMSPAR != 0, flags & libc::PARODD != 0) {
        (true, true) => Parity::Mark,
        (true, false) => Parity::Space,
        (false, true) => Parity::Odd,
        (false, false) => Parity::Even,
    }
}

/// A serial line, and the terms it was set to.
pub struct SerialEndpoint {
    file: File,
    _buffer: Vec<u8>,
}

impl SerialEndpoint {
    /// Open the line and set it to what the configuration says.
    pub fn new(what: &str, config: &SerialConfig) -> TcsResult<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            // Not this process's terminal. A line that is a tty -- a real
            // serial port, or a pty standing in for one -- becomes the
            // controlling terminal of a process that opens it without this,
            // and then a read from a background process group raises SIGTTIN
            // and a hangup raises SIGHUP: a handler stopped or killed for
            // reasons that have nothing to do with its payload.
            .custom_flags(libc::O_NOCTTY)
            .open(&config.path)?;

        apply_line_terms(what, &file, config)?;

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
fn apply_line_terms(what: &str, file: &File, config: &SerialConfig) -> TcsResult<()> {
    let fd = file.as_raw_fd();
    let speed = baud_of(config.datarate)?;

    // Read what the port is set to now, so that what is not named here is
    // left as it was rather than zeroed.
    let mut terms: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut terms) } != 0 {
        return Err(not_a_line(
            what,
            &config.path, "cannot be read as a serial port"));
    }

    unsafe { libc::cfmakeraw(&mut terms) };

    terms.c_cflag &= !(libc::CSIZE | libc::CSTOPB | libc::PARENB | libc::PARODD | libc::CMSPAR);
    terms.c_cflag |= byte_length_of(config.byte_length)?;
    // Stop bits, where the line has any: a synchronous line carries its bits
    // on a clock and has none, so there is nothing to set. Nothing else here
    // changes with it -- termios has no synchronous framing to ask for, and a
    // line that needs one is driven by a driver of its own rather than by
    // these terms.
    match config.stop_bits {
        None => {}
        Some(StopBits::One) => {}
        Some(StopBits::Two) => terms.c_cflag |= libc::CSTOPB,
        Some(StopBits::OnePointFive) => return Err(stop_bits_unsupported(what, &config.path)),
    }

    // And the parity, which termios computes per character. Mark and space
    // are the two that need CMSPAR: they send a constant bit rather than one
    // computed from the data, which is a protocol marking a frame rather than
    // checking one, and the flag that asks for it is Linux's own.
    terms.c_cflag |= parity_flags(config.parity.unwrap_or(Parity::None));
    // Read and write both: a payload link is two-way, and a port that is not
    // CLOCAL waits on carrier a payload link does not have.
    terms.c_cflag |= libc::CREAD | libc::CLOCAL;

    if unsafe { libc::cfsetispeed(&mut terms, speed) } != 0
        || unsafe { libc::cfsetospeed(&mut terms, speed) } != 0
    {
        return Err(not_a_line(
            what,
            &config.path,
            &format!("cannot be set to {} bits per second", config.datarate),
        ));
    }

    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &terms) } != 0 {
        // What was asked for, because which of the terms the port would not
        // take is the whole of what the reader needs. A pty, which is what a
        // simulated line is, takes no parity but none -- so a line set to
        // even against the simulator fails here, and the message has to say
        // that rather than leave a configuration to be bisected.
        return Err(not_a_line(
            what,
            &config.path,
            &format!(
                "cannot be set to those terms: {} bits per second, {} data bits, {}, \
                 {} parity",
                config.datarate,
                config.byte_length,
                match config.stop_bits {
                    Some(stop_bits) => format!("{stop_bits} stop bits"),
                    None => "synchronous".to_string(),
                },
                match config.parity {
                    Some(parity) => parity.to_string(),
                    None => "no".to_string(),
                }
            ),
        ));
    }

    // And read back what the port really has, because a port may take the
    // call and keep less than it was given. A pty does exactly that with
    // parity: it refuses even outright and quietly drops the rest, so a
    // handler that did not look would be checking nothing while its
    // configuration said it was checking every character.
    //
    // Only the parity is read back. It is the term whose absence is silent --
    // a rate or a byte length the port would not take shows up as data that
    // makes no sense, where a missing parity bit shows up as nothing at all
    // until a corrupted byte is believed.
    let wanted = config.parity.unwrap_or(Parity::None);
    let mut got: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut got) } != 0 {
        return Err(not_a_line(
            what,
            &config.path, "cannot be read back after setting"));
    }
    if parity_of(got.c_cflag) != wanted {
        return Err(not_a_line(
            what,
            &config.path,
            &format!(
                "will not take {wanted} parity: it reports {} after being set to it, \
                 and a handler cannot check what the port is not checking. A \
                 simulated line is a pseudo-terminal, which takes no parity but none",
                parity_of(got.c_cflag)
            ),
        ));
    }

    Ok(())
}

/// One and a half stop bits is the one term a termios line cannot hold.
///
/// A UART that offers it does so for five-bit bytes, and `termios` has one
/// bit for stop bits: one, or two. A configuration asking for it is told so
/// rather than quietly given two.
fn stop_bits_unsupported(what: &str, path: &str) -> TcsError {
    not_a_line(
        what,
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

fn not_a_line(what: &str, path: &str, trouble: &str) -> TcsError {
    TcsError::Endpoint(format!("{what}: {path} {trouble}"))
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

    /// The flags that ask for a parity and the flags that report one are
    /// inverses, and each is the combination termios documents.
    ///
    /// The only check there can be of the mapping itself: no line a test can
    /// open will take a parity -- a pseudo-terminal is all there is, and it
    /// takes none -- so a wrong flag would otherwise reach a real port
    /// unnoticed, where mark instead of space is a line checking the opposite
    /// of what the file asked for.
    #[test]
    fn asking_for_a_parity_and_reading_one_back_are_inverses() {
        for parity in [
            Parity::None,
            Parity::Even,
            Parity::Odd,
            Parity::Mark,
            Parity::Space,
        ] {
            assert_eq!(parity_of(parity_flags(parity)), parity, "{parity}");
        }

        // Anchored, so that the two being inverses is not two matching
        // mistakes: these are the combinations termios defines.
        assert_eq!(parity_flags(Parity::None), 0);
        assert_eq!(parity_flags(Parity::Even), libc::PARENB);
        assert_eq!(parity_flags(Parity::Odd) & libc::PARODD, libc::PARODD);
        assert_eq!(parity_flags(Parity::Odd) & libc::CMSPAR, 0);
        assert_eq!(parity_flags(Parity::Mark) & libc::CMSPAR, libc::CMSPAR);
        assert_eq!(
            parity_flags(Parity::Mark) & libc::PARODD,
            libc::PARODD,
            "mark sends a one, which is PARODD with CMSPAR"
        );
        assert_eq!(parity_flags(Parity::Space) & libc::CMSPAR, libc::CMSPAR);
        assert_eq!(
            parity_flags(Parity::Space) & libc::PARODD,
            0,
            "space sends a zero, which is CMSPAR without PARODD"
        );
    }

    /// A line is opened with the parity its configuration asks for, and a
    /// line that will not take one says so.
    ///
    /// Termios computes a parity bit per character, which is the whole of
    /// what a start-stop line does about checking. The five values are what a
    /// UART offers; a pty is the one line that will take none of them but
    /// `none`, which is a property of the stand-in rather than of the
    /// configuration -- and since a simulated line is a pty, the handler has
    /// to say which term was refused rather than leave a file to be bisected.
    #[test]
    fn a_line_is_set_to_the_parity_it_was_given() {
        use std::os::unix::io::FromRawFd;

        let master_fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
        assert!(master_fd >= 0, "no pty: {}", std::io::Error::last_os_error());
        let _master = unsafe { File::from_raw_fd(master_fd) };
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

        let line = |parity: Parity| SerialConfig {
            path: slave.clone(),
            datarate: 9600,
            asynchronous: true,
            parity: Some(parity),
            clock_type: None,
            encoding: None,
            frame_check: None,
            loopback: None,
            stop_bits: Some(StopBits::One),
            byte_length: 8,
        };

        // No parity, which is what a payload link commonly runs and what a
        // pty will take: the port comes back with the bit disabled.
        let endpoint =
            SerialEndpoint::new("telemetry", &line(Parity::None)).expect("a pty takes none");
        let mut terms: libc::termios = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe { libc::tcgetattr(endpoint.file.as_raw_fd(), &mut terms) },
            0
        );
        assert_eq!(
            terms.c_cflag & libc::PARENB,
            0,
            "no parity was asked for and one was set"
        );
        drop(endpoint);

        // And a parity a pty will not take is reported, with the terms that
        // were asked for: a handler that quietly ran without the parity its
        // configuration gave would be checking nothing and saying nothing.
        for parity in [Parity::Even, Parity::Odd, Parity::Mark, Parity::Space] {
            let said = match SerialEndpoint::new("telemetry", &line(parity)) {
                Err(e) => format!("{e}"),
                Ok(_) => panic!(
                    "{parity}: a pseudo-terminal cannot check parity, and the \
                     handler said it had set it"
                ),
            };
            // Named either way: a pty refuses even outright and quietly
            // drops the other three, so one of the two messages says so and
            // both name the parity that was asked for.
            assert!(
                said.contains(parity.as_str())
                    && (said.contains("cannot be set") || said.contains("will not take")),
                "{parity}: {said}"
            );
        }
    }

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

    /// A pty's slave is a port as far as this endpoint is concerned: it takes
    /// the terms and carries the bytes.
    ///
    /// Which is the whole of what tcssim stands in for a serial line with. The
    /// terms are stored and ignored by the kernel -- a pty has no cable to run
    /// at 9600 -- so what this shows is that a handler configured for a line
    /// opens one, sets it, and reads what the far end sends, with no hardware
    /// in it anywhere.
    #[test]
    fn a_pty_slave_is_a_line_this_endpoint_can_open() {
        use std::io::Write;
        use std::os::unix::io::FromRawFd;

        // A pty, as tcssim makes one.
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

        let config = SerialConfig {
            path: slave.clone(),
            datarate: 9600,
            asynchronous: true,
            parity: Some(Parity::None),
            clock_type: None,
            encoding: None,
            frame_check: None,
            loopback: None,
            stop_bits: Some(StopBits::One),
            byte_length: 8,
        };
        let mut endpoint = SerialEndpoint::new("telemetry", &config)
            .unwrap_or_else(|e| panic!("{slave} would not open as a line: {e}"));

        // What the payload's end writes, the handler's end reads. Raw terms
        // were applied when the endpoint was opened, so these bytes are not
        // edited, echoed or translated on the way.
        master.write_all(b"from the payload").expect("the far end writes");
        master.flush().ok();

        let mut buffer = [0u8; 64];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut got = Vec::new();
        while got.len() < b"from the payload".len() && std::time::Instant::now() < deadline {
            match endpoint.read(&mut buffer) {
                Ok(n) if n > 0 => got.extend_from_slice(&buffer[..n]),
                _ => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        }

        assert_eq!(
            got,
            b"from the payload",
            "what came up the line was {:?}",
            String::from_utf8_lossy(&got)
        );
    }

    /// A file that is not a serial port is reported as one that cannot be set,
    /// rather than opened and read as though the terms had been applied.
    #[test]
    fn a_file_that_is_not_a_port_is_reported() {
        let file = tempfile::NamedTempFile::new().expect("a temporary file");
        let config = SerialConfig {
            path: file.path().display().to_string(),
            datarate: 9600,
            asynchronous: true,
            parity: Some(Parity::None),
            clock_type: None,
            encoding: None,
            frame_check: None,
            loopback: None,
            stop_bits: Some(StopBits::One),
            byte_length: 8,
        };

        let e = match SerialEndpoint::new("telemetry", &config) {
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
