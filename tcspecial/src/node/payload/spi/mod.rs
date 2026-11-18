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

use std::io::IsTerminal;
use std::io::{Read, Write};
use std::path::Path;
use tcsos::fs::File;
use tcsos::wait::{Ready, Source, Waitable};

use log::info;
use tcsprotocol::{
    BitOrder, CsActive, SpiConfig, SpiMode, TcsError, TcsResult,
};

use crate::config::constants::ENDPOINT_BUFFER_SIZE;
use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable,
    WaitResult,
};

use tcsos::linux::spi::{CPHA, CPOL, CS_HIGH};

/// Whether the node at the end of the path is a peripheral or a simulated
/// device.
///
/// A spidev node takes the requests above and clocks a peripheral on the terms
/// they set. A terminal takes none of them -- it has no clock to set -- and is
/// what tcs-sim simulates a peripheral with, there being no module that
/// emulates one. The bytes of such a link are real and its clocking does not
/// exist, so a handler on one is told what it has, once, rather than left to
/// infer it from transfers that work while the terms did not take.
///
/// Nothing else is accepted. A regular file, or a character device that is
/// neither of these, is a path naming something other than what the
/// configuration describes, and opening it would be a handler talking
/// confidently to the wrong thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Clocking {
    /// A real spidev node, set to the configured terms.
    Peripheral,
    /// A terminal simulating one: the bytes without the clock.
    Simulated,
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
    /// Opens the device and sets the controller to the configured terms.
    ///
    /// A terminal, which is what simulates a peripheral, is made raw instead
    /// and said so in the log; nothing else that refuses the mode is accepted.
    ///
    /// # Arguments
    ///
    /// * `what`   - The handler's name, for what a complaint says.
    /// * `config` - The node's path and the terms to clock it on.
    ///
    /// # Returns
    ///
    /// The endpoint, a peripheral set to those terms or a raw simulated one.
    ///
    /// # Errors
    ///
    /// `TcsError::Endpoint` if the node cannot be opened, refuses a term
    /// (the mode, unless it is a terminal), or is a terminal that cannot be
    /// made raw.
    pub fn new(what: &str, config: &SpiConfig) -> TcsResult<Self> {
        // The handler's name and its node together, because every complaint
        // below is about both: which device could not be reached, and the
        // node it was to be reached over. One label, built once, so that the
        // name cannot be left off one message and put on the next.
        let on = format!("{what}: {}", config.path);

        // Not this process's terminal: see tcsos::linux::fs::open_device.
        let device =
            tcsos::linux::fs::open_device(Path::new(&config.path), true, true)
                .map_err(|e| {
                    TcsError::Endpoint(format!("{on} cannot be opened: {e}"))
                })?;
        crate::exclusive::device(what, &config.path, &device)?;

        // A node that is not a spidev answers `ENOTTY`, which says nothing on
        // its own about what was attempted, so each refusal says what was
        // being set.
        let refused = |wanted: &str, why: std::io::Error| {
            TcsError::Endpoint(format!("{on} would not take {wanted}: {why}"))
        };

        // The mode first, and what happens to it decides the rest. A node that
        // refuses it is not a spidev: a terminal is a simulated device to carry
        // bytes on, and anything else is a configuration naming the wrong path.
        match tcsos::linux::spi::set_mode(&device, mode_bits(config)) {
            Ok(()) => {}
            Err(_) if device.is_terminal() => {
                // The one term that means anything on a terminal: take the
                // line discipline out of the way. Left canonical, the
                // discipline holds bytes back until a newline arrives, and
                // device data has no lines in it -- so a handler would sit on
                // a read that never returned while the bytes it wanted were
                // being edited on its behalf.
                tcsos::linux::pty::make_raw(&device).map_err(|e| {
                    TcsError::Endpoint(format!(
                        "{on} is a terminal that cannot be set raw: {e}"
                    ))
                })?;

                info!(
                    "{} is a terminal simulating a SPI peripheral: it \
                     will carry bytes raw, and the mode, clock rate and word \
                     width in the configuration are not applied to anything",
                    on
                );
                return Ok(Self {
                    device,
                    clocking: Clocking::Simulated,
                    _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
                });
            }
            Err(e) => return Err(refused("its mode", e)),
        }
        tcsos::linux::spi::set_bits_per_word(&device, config.bits_per_word)
            .map_err(|e| {
                refused(&format!("{} bits per word", config.bits_per_word), e)
            })?;
        tcsos::linux::spi::set_lsb_first(
            &device,
            matches!(config.bit_order, BitOrder::LsbFirst),
        )
        .map_err(|e| refused("its bit order", e))?;
        tcsos::linux::spi::set_max_speed(&device, config.max_speed)
            .map_err(|e| refused(&format!("{} Hz", config.max_speed), e))?;

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
            device: self.device.try_clone()?,
            clocking: self.clocking,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }
}

/// Builds the mode byte: the clock terms, and the chip select polarity
/// beside them.
///
/// # Arguments
///
/// * `config` - The SPI mode and chip select polarity.
///
/// # Returns
///
/// The byte `tcsos::linux::spi::set_mode` takes.
fn mode_bits(config: &SpiConfig) -> u8 {
    let mut bits = match config.mode {
        SpiMode::Mode0 => 0,
        SpiMode::Mode1 => CPHA,
        SpiMode::Mode2 => CPOL,
        SpiMode::Mode3 => CPOL | CPHA,
    };
    if let CsActive::High = config.cs_active {
        bits |= CS_HIGH;
    }
    bits
}

impl EndpointWaitable for SpiEndpoint {
    fn io_source(&self) -> Source {
        self.device.source()
    }

    /// As for an I2C bus: a transfer is one the controller starts, so there is
    /// nothing to wait for on the device itself, and the wait is really on the
    /// command pipe.
    fn wait_for_event(
        &self,
        cmd: Source,
        timeout_ms: i32,
    ) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_source(), cmd, Ready::READ, timeout_ms)
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
        assert_eq!(of(SpiMode::Mode1, CsActive::Low), CPHA);
        assert_eq!(of(SpiMode::Mode2, CsActive::Low), CPOL);
        assert_eq!(of(SpiMode::Mode3, CsActive::Low), CPOL | CPHA);
        assert_eq!(
            of(SpiMode::Mode0, CsActive::High),
            CS_HIGH,
            "an active-high chip select is not a mode of its own"
        );
    }

    /// A terminal is taken as a simulated peripheral, and carries bytes.
    ///
    /// Which is what tcs-sim offers: nothing emulates a SPI peripheral, so what
    /// it puts at the end of the path is a pty. The terms do not take -- a pty
    /// has no clock to set -- and the handler says so and carries the bytes
    /// anyway, because the bytes are the part a simulation can be honest about.
    #[test]
    fn a_terminal_is_taken_as_simulated_and_carries_bytes() {
        use std::io::Write;

        let (mut master, slave) =
            tcsos::linux::pty::open().expect("a pseudo-terminal");

        let config = SpiConfig {
            path: slave.clone(),
            max_speed: 1_000_000,
            mode: SpiMode::Mode0,
            bits_per_word: 8,
            bit_order: BitOrder::MsbFirst,
            cs_active: CsActive::Low,
        };
        let mut endpoint = SpiEndpoint::new("instrument", &config)
            .unwrap_or_else(|e| {
                panic!("{slave} was not taken as simulated: {e}")
            });
        assert_eq!(
            endpoint.clocking,
            Clocking::Simulated,
            "a pty was mistaken for a peripheral"
        );

        master
            .write_all(b"from the peripheral")
            .expect("the far end writes");
        master.flush().ok();
        let mut buffer = [0u8; 64];
        let deadline =
            tcsos::time::Instant::now() + tcsos::time::Duration::from_secs(5);
        let mut got = Vec::new();
        while got.len() < b"from the peripheral".len()
            && tcsos::time::Instant::now() < deadline
        {
            match endpoint.read(&mut buffer) {
                Ok(n) if n > 0 => got.extend_from_slice(&buffer[..n]),
                _ => {
                    tcsos::thread::sleep(tcsos::time::Duration::from_millis(10))
                }
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

        let e = match SpiEndpoint::new("instrument", &config) {
            Ok(_) => panic!("a plain file was taken for a SPI device"),
            Err(e) => e,
        };
        let said = format!("{e}");
        assert!(
            said.contains(&config.path) && said.contains("mode"),
            "the complaint names neither the device nor what was being set: \
             {said}"
        );
        // And which handler it was for, as the device configuration names it.
        assert!(
            said.contains("instrument"),
            "the complaint does not say which handler: {said}"
        );
        // Narrowly: a terminal is a simulated device and a plain file is not. A
        // handler that accepted anything would open the wrong path confidently.
    }
}
