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

// The two-wire variant, which is one of a line's own terms rather than
// an interface of its own -- so it nests here rather than sitting beside
// this in node/.
pub mod rs485;

use std::io::{Read, Write};
use tcsos::fs::File;
use tcsos::linux::serial;
use tcsos::wait::{Ready, Source, Waitable};

use tcsprotocol::{Parity, SerialConfig, StopBits, TcsError, TcsResult};

use crate::config::constants::ENDPOINT_BUFFER_SIZE;
use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable,
    WaitResult,
};

/// The parity a line is asked for, in the system's terms.
///
/// # Arguments
///
/// * `parity` - The parity, as the configuration gives it.
///
/// # Returns
///
/// The same parity, as `tcsos::linux::serial` takes it.
fn parity_asked(parity: Parity) -> serial::Parity {
    match parity {
        Parity::None => serial::Parity::None,
        Parity::Even => serial::Parity::Even,
        Parity::Odd => serial::Parity::Odd,
        Parity::Mark => serial::Parity::Mark,
        Parity::Space => serial::Parity::Space,
    }
}

/// The parity a line has, in the configuration's terms.
///
/// # Arguments
///
/// * `parity` - The parity, as `tcsos::linux::serial` reports it.
///
/// # Returns
///
/// The same parity, as the configuration says it.
fn parity_said(parity: serial::Parity) -> Parity {
    match parity {
        serial::Parity::None => Parity::None,
        serial::Parity::Even => Parity::Even,
        serial::Parity::Odd => Parity::Odd,
        serial::Parity::Mark => Parity::Mark,
        serial::Parity::Space => Parity::Space,
    }
}

/// A serial line, and the terms it was set to.
pub struct SerialEndpoint {
    file: File,
    _buffer: Vec<u8>,
}

impl SerialEndpoint {
    /// Opens the line and sets it to what the configuration says.
    ///
    /// # Arguments
    ///
    /// * `what`   - The handler's name, for what a complaint says.
    /// * `config` - The line's path and the terms to set it to.
    ///
    /// # Returns
    ///
    /// The endpoint, its line set to those terms.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if the line cannot be opened; otherwise as
    /// `apply_line_terms`.
    pub fn new(what: &str, config: &SerialConfig) -> TcsResult<Self> {
        // Not this process's terminal: see tcsos::linux::fs::open_device.
        let file = serial_open(&config.path).map_err(TcsError::Io)?;
        crate::exclusive::device(what, &config.path, &file)?;

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
    ///
    /// # Returns
    ///
    /// The second endpoint, on a duplicate of this one's descriptor.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if the descriptor cannot be duplicated.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            file: self.file.try_clone()?,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }
}

/// Sets the line's data rate, byte length, stop bits and parity, and its RS-485
/// mode where the configuration gives one and the line is not a simulated
/// device.
///
/// Raw rather than canonical: a device link carries bytes, and the line
/// discipline's editing, echo and newline translation would corrupt them. The
/// parity is read back once set, because a port may take the call and keep
/// less than it was given.
///
/// A simulated line -- a pseudo-terminal, which tcs-sim makes -- has no
/// transceiver and no RTS to drive, so the RS-485 terms are left unapplied
/// there and the bytes go both ways as on any line: the bus is simulated by
/// what is at the far end, and only its electrical turnaround is not.
///
/// A rate the platform cannot express is reported with the rate in hand,
/// because the alternative is a line running at a rate nobody chose.
///
/// # Arguments
///
/// * `what`   - The handler's name, for what a complaint says.
/// * `file`   - The open line.
/// * `config` - The line's path, for what a complaint says, and its terms.
///
/// # Errors
///
/// `TcsError::Config` if the rate or byte length is not one a port
/// offers, or the line will not take its RS-485 mode;
/// `TcsError::Endpoint` if the line is not a serial port, will not take
/// the terms, keeps a parity other than the one asked for, or is asked
/// for one and a half stop bits.
fn apply_line_terms(
    what: &str,
    file: &File,
    config: &SerialConfig,
) -> TcsResult<()> {
    let terms = serial::Terms {
        rate: rate_of(config.datarate)?,
        byte_length: byte_length_of(config.byte_length)?,
        stop_bits: match config.stop_bits {
            StopBits::One => serial::StopBits::One,
            StopBits::Two => serial::StopBits::Two,
            StopBits::OnePointFive => {
                return Err(stop_bits_unsupported(what, &config.path))
            }
        },
        // The parity, which termios computes per character.
        parity: parity_asked(config.parity),
    };
    match serial::set_terms(file, &terms) {
        Ok(()) => {}
        Err(serial::Refused::Rate(_) | serial::Refused::ByteLength(_)) => {
            unreachable!("the rate and byte length are checked above")
        }
        Err(serial::Refused::NotALine) => {
            return Err(not_a_line(
                what,
                &config.path,
                "cannot be read as a serial port",
            ))
        }
        Err(serial::Refused::Speed) => {
            return Err(not_a_line(
                what,
                &config.path,
                &format!(
                    "cannot be set to {} bits per second",
                    config.datarate
                ),
            ))
        }
        // What was asked for, because which of the terms the port would not
        // take is the whole of what the reader needs. A pty, which is what a
        // simulated line is, takes no parity but none -- so a line set to
        // even against the simulator fails here, and the message has to say
        // that rather than leave a configuration to be bisected.
        Err(serial::Refused::Terms) => {
            return Err(not_a_line(
                what,
                &config.path,
                &format!(
                    "cannot be set to those terms: {} bits per second, {} \
                     data bits, {} stop bits, {} parity",
                    config.datarate,
                    config.byte_length,
                    config.stop_bits,
                    config.parity
                ),
            ))
        }
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
    let wanted = config.parity;
    let Some(got) = serial::parity_now(file).map(parity_said) else {
        return Err(not_a_line(
            what,
            &config.path,
            "cannot be read back after setting",
        ));
    };
    if got != wanted {
        return Err(not_a_line(
            what,
            &config.path,
            &format!(
                "will not take {wanted} parity: it reports {} after being set \
                 to it, and a handler cannot check what the port is not \
                 checking. A simulated line is a pseudo-terminal, which takes \
                 no parity but none",
                got
            ),
        ));
    }

    // And last, the electrical layer under all of that, for a line that is a
    // two-wire bus. After the termios terms, because the mode is a property of
    // the port rather than of the line discipline and the driver wants the line
    // already set up; and refused rather than reported, for the reason
    // serial::rs485::apply gives. Not on a simulated device: under the flight
    // setting none is allowed to start, so a pseudo-terminal here is a run
    // against tcs-sim, which carries the bus's bytes and has no turnaround.
    if let Some(terms) = &config.rs485 {
        if tcsprotocol::node::payload::serial::judged(&config.path)
            .is_simulated()
        {
            log::info!(
                "{what}: {} is a simulated device, so its RS-485 terms are \
                 not applied",
                config.path
            );
        } else {
            crate::node::payload::serial::rs485::apply(file, what, terms)?;
        }
    }

    Ok(())
}

/// Says that one and a half stop bits is the one term a termios line cannot
/// hold.
///
/// A UART that offers it does so for five-bit bytes, and `termios` has one
/// bit for stop bits: one, or two. A configuration asking for it is told so
/// rather than quietly given two.
///
/// # Arguments
///
/// * `what` - The handler's name.
/// * `path` - The line's path.
///
/// # Returns
///
/// A `TcsError::Endpoint` saying so.
fn stop_bits_unsupported(what: &str, path: &str) -> TcsError {
    not_a_line(
        what,
        path,
        "cannot be set to one and a half stop bits: a serial port offers one \
         or two",
    )
}

/// Checks that a data rate is one a port can be set to.
///
/// # Arguments
///
/// * `datarate` - The rate in bits per second.
///
/// # Returns
///
/// The rate.
///
/// # Errors
///
/// `TcsError::Config` naming the rate, where it is not one `termios` has a
/// constant for: the alternative is a line running at a rate nobody chose.
fn rate_of(datarate: u32) -> TcsResult<u32> {
    if serial::is_a_rate(datarate) {
        return Ok(datarate);
    }
    Err(TcsError::Config(format!(
        "{datarate} bits per second is not a rate a serial port can be set to"
    )))
}

/// Checks that a byte length is one a port offers.
///
/// # Arguments
///
/// * `bits` - The byte length in bits.
///
/// # Returns
///
/// The length.
///
/// # Errors
///
/// `TcsError::Config` if the length is not 5 to 8.
fn byte_length_of(bits: u8) -> TcsResult<u8> {
    if serial::is_a_byte_length(bits) {
        return Ok(bits);
    }
    Err(TcsError::Config(format!(
        "a byte of {bits} bits is not a length a serial port offers"
    )))
}

/// Opens a line, not as this process's terminal.
///
/// # Arguments
///
/// * `path` - The line.
///
/// # Returns
///
/// The line, open for reading and writing.
///
/// # Errors
///
/// What the system said.
fn serial_open(path: &str) -> std::io::Result<File> {
    tcsos::linux::fs::open_device(std::path::Path::new(path), true, true)
}

fn not_a_line(what: &str, path: &str, trouble: &str) -> TcsError {
    TcsError::Endpoint(format!("{what}: {path} {trouble}"))
}

impl EndpointWaitable for SerialEndpoint {
    fn io_source(&self) -> Source {
        self.file.source()
    }

    fn wait_for_event(
        &self,
        cmd: Source,
        timeout_ms: i32,
    ) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_source(), cmd, Ready::READ, timeout_ms)
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

    /// A line is opened with the parity its configuration asks for, and a
    /// line that will not take one says so.
    ///
    /// Termios computes a parity bit per character, which is the whole of what
    /// a start-stop line does about checking. The five values are what a UART
    /// offers; a pty is the one line that will take none of them but `none`,
    /// which is a property of the simulated line rather than of the
    /// configuration -- and since a simulated line is a pty, the handler has to
    /// say which term was refused rather than leave a file to be bisected.
    #[test]
    fn a_line_is_set_to_the_parity_it_was_given() {
        let (_master, slave) =
            tcsos::linux::pty::open().expect("a pseudo-terminal");

        let line = |parity: Parity| SerialConfig {
            path: slave.clone(),
            datarate: 9600,
            parity,
            stop_bits: StopBits::One,
            byte_length: 8,
            // Not an RS-485 bus: a pty takes no turnaround, as it takes no
            // parity.
            rs485: None,
        };

        // No parity, which is what a device link commonly runs and what a
        // pty will take: the port comes back with the bit disabled.
        let endpoint = SerialEndpoint::new("telemetry", &line(Parity::None))
            .expect("a pty takes none");
        assert_eq!(
            serial::parity_now(&endpoint.file),
            Some(serial::Parity::None),
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
                    && (said.contains("cannot be set")
                        || said.contains("will not take")),
                "{parity}: {said}"
            );
        }
    }

    /// A rate no port offers is named rather than rounded to one that is.
    #[test]
    fn a_rate_no_port_offers_is_reported() {
        assert!(rate_of(9600).is_ok());
        let e = rate_of(9601).unwrap_err();
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
    /// Which is the whole of what tcs-sim simulates a serial line with. The
    /// terms are stored and ignored by the kernel -- a pty has no cable to run
    /// at 9600 -- so what this shows is that a handler configured for a line
    /// opens one, sets it, and reads what the far end sends, with no hardware
    /// Makes a pseudo-terminal, as tcs-sim makes one for a serial payload.
    ///
    /// # Returns
    ///
    /// The master, which is the device's end, and the slave's path, which is
    /// the handler's.
    ///
    /// # Panics
    ///
    /// If no pseudo-terminal can be made.
    fn a_pty() -> (File, String) {
        tcsos::linux::pty::open().expect("a pseudo-terminal")
    }

    /// A pty's slave is a port as far as this endpoint is concerned: it takes
    /// the terms and carries the bytes.
    ///
    /// Which is the whole of what tcs-sim simulates a serial line with. The
    /// terms are stored and ignored by the kernel -- a pty has no cable to run
    /// at 9600 -- so what this shows is that a handler configured for a line
    /// opens one, sets it, and reads what the far end sends, with no hardware
    /// in it anywhere.
    #[test]
    fn a_pty_slave_is_a_line_this_endpoint_can_open() {
        use std::io::Write;

        let (mut master, slave) = a_pty();
        let config = SerialConfig {
            path: slave.clone(),
            datarate: 9600,
            parity: Parity::None,
            stop_bits: StopBits::One,
            byte_length: 8,
            rs485: None,
        };
        let mut endpoint = SerialEndpoint::new("telemetry", &config)
            .unwrap_or_else(|e| {
                panic!("{slave} would not open as a line: {e}")
            });

        // What the device's end writes, the handler's end reads. Raw terms
        // were applied when the endpoint was opened, so these bytes are not
        // edited, echoed or translated on the way.
        master
            .write_all(b"from the device")
            .expect("the far end writes");
        master.flush().ok();

        let mut buffer = [0u8; 64];
        let deadline =
            tcsos::time::Instant::now() + tcsos::time::Duration::from_secs(5);
        let mut got = Vec::new();
        while got.len() < b"from the device".len()
            && tcsos::time::Instant::now() < deadline
        {
            match endpoint.read(&mut buffer) {
                Ok(n) if n > 0 => got.extend_from_slice(&buffer[..n]),
                _ => {
                    tcsos::thread::sleep(tcsos::time::Duration::from_millis(10))
                }
            }
        }

        assert_eq!(
            got,
            b"from the device",
            "what came up the line was {:?}",
            String::from_utf8_lossy(&got)
        );
    }

    /// A line that states RS-485 is simulated by a pty as any line is: the
    /// terms are left unapplied there, rather than refused, and the bytes go
    /// both ways.
    ///
    /// A pty has no transceiver and no RTS to drive, and refuses the mode
    /// (see rs485's own test), so a handler that applied it would never open
    /// against tcs-sim; the far end, not the kernel, is what simulates the bus.
    #[test]
    fn a_pty_stands_in_for_an_rs485_line() {
        use std::io::{Read, Write};
        use tcsos::time::{Duration, Instant};

        let (mut master, slave) = a_pty();
        let config = SerialConfig {
            path: slave.clone(),
            datarate: 9600,
            parity: Parity::None,
            stop_bits: StopBits::One,
            byte_length: 8,
            rs485: Some(tcsprotocol::Rs485Params {
                rts_on_send: true,
                delay_before_send: Duration::from_millis(2),
                delay_after_send: Duration::from_millis(2),
            }),
        };
        let mut endpoint =
            SerialEndpoint::new("bus", &config).unwrap_or_else(|e| {
                panic!("{slave} would not simulate a bus: {e}")
            });

        master.write_all(b"up").expect("the far end writes");
        let mut buffer = [0u8; 16];
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut got = Vec::new();
        while got.len() < 2 && Instant::now() < deadline {
            match endpoint.read(&mut buffer) {
                Ok(n) if n > 0 => got.extend_from_slice(&buffer[..n]),
                _ => tcsos::thread::sleep(Duration::from_millis(10)),
            }
        }
        assert_eq!(got, b"up", "what came up the bus");

        endpoint.write(b"down").expect("the handler writes");
        let mut down = [0u8; 4];
        master.read_exact(&mut down).expect("the far end reads");
        assert_eq!(&down, b"down", "what went down the bus");
    }

    /// A file that is not a serial port is reported as one that cannot be set,
    /// rather than opened and read as though the terms had been applied.
    #[test]
    fn a_file_that_is_not_a_port_is_reported() {
        let file = tempfile::NamedTempFile::new().expect("a temporary file");
        let config = SerialConfig {
            path: file.path().display().to_string(),
            datarate: 9600,
            parity: Parity::None,
            stop_bits: StopBits::One,
            byte_length: 8,
            // Not an RS-485 bus: a pty takes no turnaround, as it takes no
            // parity.
            rs485: None,
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
