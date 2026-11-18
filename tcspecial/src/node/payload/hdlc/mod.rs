//! The HDLC endpoint: a synchronous line, which Linux drives as a network
//! interface.
//!
//! **Experimental, and not yet supported.** Run against no card, because there
//! is no card and no dummy device: every other experimental type here has
//! something in the kernel to run it against -- `vcan`, `i2c-stub`, `gpio-sim`,
//! `iio_dummy` -- and generic HDLC has nothing. What is written here is held to
//! the kernel's structures and constants by tests, which is how the GPIO
//! endpoint was written before `gpio-sim` existed.
//!
//! # What this does
//!
//! Two ioctls to set the line up, two more to read back what it took, and a
//! packet socket for the frames.
//!
//! `SIOCWANDEV` is the one ioctl generic HDLC has, and it carries whichever
//! structure the `type` field names:
//!
//! * `IF_IFACE_V35` and its fellows carry `sync_serial_settings` -- the clock,
//!   its rate, and the loopback. This is the port's wiring.
//! * `IF_PROTO_HDLC` carries `raw_hdlc_proto` -- the encoding and the frame
//!   check. This is what the bits mean once the port is clocked.
//! * `IF_GET_IFACE` and `IF_GET_PROTO` read the two back.
//!
//! Read back and compared, which is the rule a serial line already follows: a
//! port may take the call and keep less than it was given, and a line whose
//! frame check is not the one the configuration asked for is a line checking
//! nothing while the file says it checks every frame.
//!
//! Then the frames. Once the line is set up and the interface is up, a raw
//! HDLC netdevice carries frames with no address in them, so this binds an
//! `AF_PACKET` socket of type `SOCK_DGRAM` to the interface: a read hands over
//! one frame's payload and a write puts one payload into one frame, as the CAN
//! endpoint does with its own. `SOCK_DGRAM` rather than `SOCK_RAW` because
//! there is no link header worth seeing -- a raw HDLC frame has none.
//!
//! The interface is not brought up here. Bringing an interface up is the
//! platform's business, as it is for a CAN controller: `ip link set dev hdlc0
//! up`, from whatever starts the vehicle.

use tcsos::linux::hdlc::{self, Line, Refused};
use tcsos::wait::{Ready, Source, Waitable};

use tcsprotocol::{
    ClockType, Encoding, FrameCheck, HdlcConfig, Standard, TcsError, TcsResult,
};

use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable,
    WaitResult,
};

/// A synchronous line: the frames, and the settings that were put on it.
pub struct HdlcEndpoint {
    /// The handler this endpoint belongs to, as the configuration names it.
    what: String,
    /// The line, set up, and the socket its frames come and go on.
    line: Line,
    /// Which interface, for what a complaint says.
    interface: String,
}

impl HdlcEndpoint {
    /// Sets the line up and binds a socket for its frames.
    ///
    /// # Arguments
    ///
    /// * `what`   - The handler this endpoint belongs to, as the
    ///              configuration names it, for what a complaint says.
    /// * `config` - The interface and the wiring and framing to put on it.
    ///
    /// # Returns
    ///
    /// The endpoint, its socket bound to the interface.
    ///
    /// # Errors
    ///
    /// `TcsError::Config` if there is no such interface or its name is
    /// not one an interface can have; `TcsError::Endpoint` if a socket
    /// cannot be opened or bound, if the card refuses the wiring or the
    /// framing, or if what it reads back is not what it was given.
    pub fn new(what: &str, config: &HdlcConfig) -> TcsResult<Self> {
        let line = Line::open(
            &config.interface,
            &wiring_of(config),
            &framing_of(config),
        )
        .map_err(|refused| refused_for(what, config, refused))?;
        Ok(HdlcEndpoint {
            what: what.to_string(),
            line,
            interface: config.interface.clone(),
        })
    }

    /// A second endpoint on the same socket.
    ///
    /// The descriptor is duplicated rather than the socket opened again: the
    /// line is set up once, and setting it up twice would be two programs
    /// configuring one port.
    ///
    /// # Returns
    ///
    /// The second endpoint, on a duplicate of this one's descriptor.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if the descriptor cannot be duplicated.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(HdlcEndpoint {
            what: self.what.clone(),
            line: self.line.try_clone()?,
            interface: self.interface.clone(),
        })
    }
}

/// The port's wiring, from the configuration.
///
/// # Arguments
///
/// * `config` - The line's configuration.
///
/// # Returns
///
/// Its standard, clock, the clock's rate -- nought where the far end drives
/// it, which is the kernel's "not this end's to set": the configuration
/// refuses a rate in that case -- and its loopback.
fn wiring_of(config: &HdlcConfig) -> hdlc::Wiring {
    hdlc::Wiring {
        standard: match config.standard {
            Standard::V35 => hdlc::Standard::V35,
            Standard::V24 => hdlc::Standard::V24,
            Standard::X21 => hdlc::Standard::X21,
            Standard::X21D => hdlc::Standard::X21D,
        },
        clock: match config.clock_type {
            ClockType::External => hdlc::Clock::External,
            ClockType::Internal => hdlc::Clock::Internal,
            ClockType::TxInternal => hdlc::Clock::TxInternal,
            ClockType::TxFromRx => hdlc::Clock::TxFromRx,
        },
        clock_rate: config.clock_rate.unwrap_or(0),
        loopback: config.loopback,
    }
}

/// What the bits mean, from the configuration.
///
/// # Arguments
///
/// * `config` - The line's configuration.
///
/// # Returns
///
/// The encoding and the frame check. The kernel calls a frame check a
/// line's parity, which is the one place this project's word and the
/// kernel's differ: see tcsprotocol::node::payload::hdlc.
fn framing_of(config: &HdlcConfig) -> hdlc::Framing {
    hdlc::Framing {
        encoding: match config.encoding {
            Encoding::Nrz => hdlc::Encoding::Nrz,
            Encoding::Nrzi => hdlc::Encoding::Nrzi,
            Encoding::FmMark => hdlc::Encoding::FmMark,
            Encoding::FmSpace => hdlc::Encoding::FmSpace,
            Encoding::Manchester => hdlc::Encoding::Manchester,
        },
        frame_check: match config.frame_check {
            FrameCheck::None => hdlc::FrameCheck::None,
            FrameCheck::Crc16Pr0 => hdlc::FrameCheck::Crc16Pr0,
            FrameCheck::Crc16Pr1 => hdlc::FrameCheck::Crc16Pr1,
            FrameCheck::Crc16Pr0Ccitt => hdlc::FrameCheck::Crc16Pr0Ccitt,
            FrameCheck::Crc16Pr1Ccitt => hdlc::FrameCheck::Crc16Pr1Ccitt,
            FrameCheck::Crc32Pr0Ccitt => hdlc::FrameCheck::Crc32Pr0Ccitt,
            FrameCheck::Crc32Pr1Ccitt => hdlc::FrameCheck::Crc32Pr1Ccitt,
        },
    }
}

/// What a line that could not be opened is said to be.
///
/// Read back and compared, which is the rule a serial line already
/// follows: a line whose frame check is not the one the configuration
/// asked for is a line checking nothing while the file says it checks
/// every frame.
///
/// # Arguments
///
/// * `what`    - The handler, for what a complaint says.
/// * `config`  - The line's configuration.
/// * `refused` - What could not be done.
///
/// # Returns
///
/// `TcsError::Config` for an interface that is not there or a name that
/// cannot be one, `TcsError::Endpoint` for the rest.
fn refused_for(what: &str, config: &HdlcConfig, refused: Refused) -> TcsError {
    let interface = &config.interface;
    match refused {
        Refused::Name => TcsError::Config(format!(
            "{what}: \"{interface}\" is not a name an interface can have"
        )),
        Refused::TooLong => TcsError::Config(format!(
            "{what}: \"{interface}\" is longer than an interface name may be",
        )),
        Refused::NoInterface(e) => TcsError::Config(format!(
            "{what}: this host has no interface called \"{interface}\": {e}. \
             A synchronous line is a card -- a wanxl, a pci200syn, a pc300too \
             or a FarSync -- and unlike a CAN bus there is no dummy one to \
             run against"
        )),
        Refused::Asking(e) => TcsError::Endpoint(format!(
            "{what}: cannot open a socket to set up {interface}: {e}"
        )),
        Refused::Wiring(e) => TcsError::Endpoint(format!(
            "{what}: {interface} will not be wired as {}: {e}. A card that \
             does not offer that standard refuses it here",
            config.standard
        )),
        Refused::Framing(e) => TcsError::Endpoint(format!(
            "{what}: {interface} will not take {} framing with {}: {e}",
            config.encoding, config.frame_check
        )),
        Refused::ReadWiring(e) => TcsError::Endpoint(format!(
            "{what}: {interface} cannot be read back after being wired: {e}"
        )),
        Refused::WiringKept(got) => TcsError::Endpoint(format!(
            "{what}: {interface} reports {got}, and a handler cannot run a \
             line the card is not running"
        )),
        Refused::ReadFraming(e) => TcsError::Endpoint(format!(
            "{what}: {interface} cannot be read back after its framing was \
             set: {e}"
        )),
        Refused::FramingKept(got) => TcsError::Endpoint(format!(
            "{what}: {interface} reports {got}: a frame check the card is not \
             applying is one the configuration only says it applies"
        )),
        Refused::PacketSocket(e) => TcsError::Endpoint(format!(
            "{what}: cannot open a packet socket for {interface}: {e}. A \
             kernel without packet sockets has none to give"
        )),
        Refused::Bind(e) => TcsError::Endpoint(format!(
            "{what}: cannot bind a packet socket to {interface}: {e}"
        )),
    }
}

impl EndpointWaitable for HdlcEndpoint {
    fn io_source(&self) -> Source {
        self.line.source()
    }

    fn wait_for_event(
        &self,
        cmd: Source,
        timeout_ms: i32,
    ) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_source(), cmd, Ready::READ, timeout_ms)
    }
}

impl EndpointReadable for HdlcEndpoint {
    /// One frame's payload, or nothing.
    ///
    /// A frame at a time, because that is what the socket delivers: a read
    /// gets one whole frame or none of one, so there is no partial frame to
    /// keep and no stream rule to say where a read ends.
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        self.line.receive(buffer).map_err(TcsError::Io)
    }
}

impl EndpointWritable for HdlcEndpoint {
    /// One payload into one frame, addressed to the interface: a raw HDLC
    /// frame carries no address, there being one far end and no bus.
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        self.line.send(data).map_err(|e| {
            TcsError::Endpoint(format!(
                "{}: cannot send a frame on {}: {e}",
                self.what, self.interface
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gives a configuration for `hdlc0` with every setting at its
    /// simplest.
    ///
    /// # Returns
    ///
    /// A V.35 line, clocked from the far end, NRZ, with no frame check.
    fn config() -> HdlcConfig {
        HdlcConfig {
            interface: "hdlc0".to_string(),
            standard: Standard::V35,
            clock_type: ClockType::External,
            clock_rate: None,
            loopback: false,
            encoding: Encoding::Nrz,
            frame_check: FrameCheck::None,
        }
    }

    /// What the configuration says becomes what the kernel is told: the
    /// far end's clock with no rate, and this end's at the rate the file
    /// gave.
    #[test]
    fn the_settings_are_what_the_file_said() {
        let wiring = wiring_of(&config());
        assert_eq!(wiring.clock, hdlc::Clock::External);
        assert_eq!(wiring.clock_rate, 0);
        assert!(!wiring.loopback);

        let mut mine = config();
        mine.clock_type = ClockType::Internal;
        mine.clock_rate = Some(2_048_000);
        mine.loopback = true;
        let wiring = wiring_of(&mine);
        assert_eq!(wiring.clock, hdlc::Clock::Internal);
        assert_eq!(wiring.clock_rate, 2_048_000);
        assert!(wiring.loopback);

        let framing = framing_of(&config());
        assert_eq!(framing.encoding, hdlc::Encoding::Nrz);
        assert_eq!(framing.frame_check, hdlc::FrameCheck::None);
    }

    /// An interface that is not there is reported, and says what one is.
    #[test]
    fn an_interface_that_is_not_there_is_reported() {
        let mut config = config();
        config.interface = "hdlc-nonesuch".to_string();

        let said = match HdlcEndpoint::new("link", &config) {
            Err(e) => format!("{e}"),
            Ok(_) => panic!("this host has an interface called hdlc-nonesuch"),
        };
        assert!(
            said.contains("hdlc-nonesuch") && said.contains("no dummy one"),
            "the complaint should name the interface and say what one is: \
             {said}"
        );
    }
}
