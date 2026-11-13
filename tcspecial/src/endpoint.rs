//! Endpoints: what a data handler does its I/O on.
//!
//! An endpoint is one end of a path a data handler's data travels. A handler
//! has two of them -- the OC side and the payload side -- and what an
//! endpoint is made of depends on what is at the far end: a datagram socket
//! for a UDP address, a stream for a TCP one, an open file for a device.
//!
//! This module is what the rest of tcspecial sees of all of them: the three
//! traits every endpoint satisfies, the waiting they share, and the factories
//! that open whichever kind a configuration asks for. Each kind itself lives
//! in a file of its own, since they have nothing in common but these traits:
//!
//! * [`crate::node::payload::network::udp`] -- datagram sockets
//! * [`crate::node::payload::network::tcp`] -- stream sockets
//! * [`crate::node::payload::network::raw`] -- raw IP sockets
//! * [`crate::node::payload::device_file`] -- device files
//! * [`crate::node::payload::network`] -- the address family, socket type and
//!   protocol tables the two socket kinds draw on

use std::io;
use std::os::fd::BorrowedFd;
use std::os::unix::io::RawFd;

use nix::poll::{poll, PollFd, PollFlags};
use tcslib_protocol::graph::terminals::Stream;
use tcslib_protocol::{
    EndpointConfig, NetworkConfig, NetworkProtocol, SendsFrom, TcsError,
    TcsResult,
};

#[cfg(feature = "can")]
use crate::node::payload::can::CanEndpoint;
#[cfg(feature = "device-file")]
use crate::node::payload::device_file::DeviceEndpoint;
#[cfg(any(
    feature = "gpio_in",
    feature = "gpio_out",
    feature = "gpio_inout"
))]
use crate::node::payload::gpio::GpioEndpoint;
#[cfg(feature = "hdlc")]
use crate::node::payload::hdlc::HdlcEndpoint;
#[cfg(feature = "i2c")]
use crate::node::payload::i2c::I2cEndpoint;
#[cfg(feature = "iio")]
use crate::node::payload::iio::IioEndpoint;
#[cfg(feature = "raw")]
use crate::node::payload::network::raw::RawEndpoint;
use crate::node::payload::network::tcp::TcpEndpoint;
use crate::node::payload::network::udp::UdpEndpoint;
use crate::node::payload::network::unix::{
    UnixDatagramEndpoint, UnixStreamEndpoint,
};
#[cfg(feature = "rpmsg")]
use crate::node::payload::rpmsg::RpmsgEndpoint;
#[cfg(feature = "serial")]
use crate::node::payload::serial::SerialEndpoint;
#[cfg(feature = "spi")]
use crate::node::payload::spi::SpiEndpoint;

/// An open refused because this build does not contain the type.
///
/// In the words the load uses: see
/// [`tcslib_protocol::graph::built::not_built_message`]. Only a build that
/// leaves a type out can reach it.
#[allow(dead_code)]
fn not_built(what: &str, node_type: &str) -> TcsError {
    TcsError::Config(tcslib_protocol::graph::built::not_built_message(
        what, node_type,
    ))
}

/// Says what could not be bound, and what that usually means.
///
/// A bare io::Error carries the kind and nothing else, so the whole of what a
/// reader got was "Address already in use (os error 98)" -- not which address,
/// and no hint that the usual cause is a second copy of the program. Both ends
/// of a data handler bind an address too, so this is used for all of them.
///
/// # Arguments
///
/// * `what` - What the address was to be listened on for, as it is named to
///            the reader.
/// * `addr` - The address that could not be bound.
/// * `e`    - What binding it failed with.
///
/// # Returns
///
/// A `TcsError::Config` naming `what` and `addr`, which for an address in
/// use says what most often holds it.
pub fn bind_failed(what: &str, addr: &str, e: std::io::Error) -> TcsError {
    if e.kind() == std::io::ErrorKind::AddrInUse {
        TcsError::Config(format!(
            "cannot listen on {addr} for the {what}: address already in use. \
             Something else holds it -- most often another tcspecial, or a \
             tcsmoc that starts one of its own"
        ))
    } else {
        TcsError::Config(format!("cannot listen on {addr} for the {what}: {e}"))
    }
}

/// Trait for endpoints that can wait for events
pub trait EndpointWaitable {
    /// Gets the I/O file descriptor.
    ///
    /// # Returns
    ///
    /// The descriptor the endpoint's I/O is done on, which it still owns.
    fn io_fd(&self) -> RawFd;

    /// Waits for an event on this endpoint or on the command pipe.
    ///
    /// # Arguments
    ///
    /// * `cmd_fd`     - The command pipe's read end.
    /// * `timeout_ms` - How long to wait, in milliseconds; negative for ever.
    ///
    /// # Returns
    ///
    /// Which of the two is ready, or that the wait timed out.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` where the wait itself fails.
    fn wait_for_event(
        &self,
        cmd_fd: RawFd,
        timeout_ms: i32,
    ) -> TcsResult<WaitResult>;
}

/// Result of waiting for an event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitResult {
    /// I/O is ready
    IoReady,
    /// Command is pending
    CommandPending,
    /// Both I/O and command are ready
    Both,
    /// Timeout occurred
    Timeout,
    /// Error occurred
    Error,
}

/// Trait for readable endpoints
pub trait EndpointReadable: EndpointWaitable {
    /// Reads data from the endpoint.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Where the data is put.
    ///
    /// # Returns
    ///
    /// How many bytes were put in `buffer`.
    ///
    /// # Errors
    ///
    /// What the endpoint failed with.
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize>;

    /// How many datagrams were turned away as a stranger's since this was
    /// last asked, and start counting again.
    ///
    /// A datagram socket that knows where its payload is reads only what
    /// comes from there: anything else that can reach the port would
    /// otherwise be handed on as the payload's reading, which nothing
    /// downstream could tell from the real one. Turned away and counted
    /// rather than reported as a failed read, because each failure is a line
    /// in the log, and a stranger sending a thousand datagrams would be a
    /// thousand lines and a thousand messages made in the loop. Nought for
    /// every endpoint
    /// that has nobody to turn away, which is what the default is.
    fn strangers(&mut self) -> u64 {
        0
    }

    /// Whether a read now would find something whatever its descriptor
    /// says: an endpoint that makes what it reports by the clock rather than
    /// being sent it -- a set of lines reported for their levels every
    /// interval -- has a report due that no descriptor announces. False for
    /// every endpoint whose data arrives, which is what the default is.
    fn due(&self) -> bool {
        false
    }

    /// Which stream the last read's data is on, where the endpoint says.
    ///
    /// An endpoint with one source terminal leaves it to the link, which is
    /// every one of them but a set of lines both watched and driven: what it
    /// watched goes groundward and what it was told to drive is passed on
    /// vehicleward, and a datum sent down the other's links would be a
    /// reading sent out as a command. `None`, the default, is no opinion.
    fn stream(&self) -> Option<Stream> {
        None
    }
}

/// The most datagrams one read turns away before it gives up for the pass.
///
/// A bound on the work a flood of strangers can make a read do: the rest
/// wait for the next pass, and the payload's own datagrams with them, which
/// is a payload late by a pass rather than a vehicle that stops passing.
pub(crate) const STRANGERS_PER_READ: usize = 16;

/// Trait for writable endpoints
pub trait EndpointWritable: EndpointWaitable {
    /// Writes data to the endpoint.
    ///
    /// # Arguments
    ///
    /// * `data` - The data to write.
    ///
    /// # Returns
    ///
    /// How many bytes of `data` were written.
    ///
    /// # Errors
    ///
    /// What the endpoint failed with.
    fn write(&mut self, data: &[u8]) -> TcsResult<usize>;
}

/// Waits for either the endpoint or the command pipe to have something.
///
/// Shared by every kind of endpoint: what differs between them is what
/// the descriptor refers to, not how it is waited on.
///
/// # Arguments
///
/// * `io_fd`      - The endpoint's descriptor, which must stay open for the
///                  call.
/// * `cmd_fd`     - The command pipe's read end, waited on for input; it too
///                  must stay open.
/// * `io_events`  - What to wait for on `io_fd`.
/// * `timeout_ms` - How long to wait, in milliseconds; negative for ever.
///
/// # Returns
///
/// Which of the two is ready; `WaitResult::Timeout` where neither became
/// ready in time, and `WaitResult::Error` where `poll` woke for something
/// that was neither, such as a hang-up or an error on `io_fd`.
///
/// # Errors
///
/// `TcsError::Io` where `poll` fails.
pub(crate) fn wait_for_fds(
    io_fd: RawFd,
    cmd_fd: RawFd,
    io_events: PollFlags,
    timeout_ms: i32,
) -> TcsResult<WaitResult> {
    // SAFETY: borrow_raw needs each descriptor to stay open for as long as
    // the borrow lasts, which is this call. Every caller passes its own
    // endpoint's descriptor and its command pipe's, both owned by the
    // handler that is calling and so open until it returns; nothing here
    // closes either, and the borrows end when poll does.
    let io_borrowed = unsafe { BorrowedFd::borrow_raw(io_fd) };
    // SAFETY: as for `io_fd` above.
    let cmd_borrowed = unsafe { BorrowedFd::borrow_raw(cmd_fd) };

    let mut poll_fds = [
        PollFd::new(&io_borrowed, io_events),
        PollFd::new(&cmd_borrowed, PollFlags::POLLIN),
    ];

    match poll(&mut poll_fds, timeout_ms) {
        Ok(0) => Ok(WaitResult::Timeout),
        Ok(_) => {
            let io_ready = poll_fds[0]
                .revents()
                .is_some_and(|r| r.intersects(io_events));
            let cmd_ready = poll_fds[1]
                .revents()
                .is_some_and(|r| r.contains(PollFlags::POLLIN));

            match (io_ready, cmd_ready) {
                (true, true) => Ok(WaitResult::Both),
                (true, false) => Ok(WaitResult::IoReady),
                (false, true) => Ok(WaitResult::CommandPending),
                (false, false) => Ok(WaitResult::Error),
            }
        }
        Err(e) => Err(TcsError::Io(io::Error::from_raw_os_error(e as i32))),
    }
}

/// Opens a payload's ground link: bound here, sending where the file says.
///
/// The one link in tcspecial whose far end is written down -- `ground` is
/// `payload_telemetry.dest_base` counted on by the payload's id, which is what
/// the MOC binds -- so this sends there rather than waiting to be spoken to.
///
/// Always UDP: that is what the link between a handler and the ground is. A
/// configuration that said otherwise would be refused where it is read.
///
/// `ground` absent is a payload converted on its own, with no `tcspecial`
/// section to count from; such a handler cannot be started, and this is here
/// rather than refusing so that the one place that says so is
/// `start_handler`.
///
/// # Arguments
///
/// * `what`   - What the link is for, as errors name it.
/// * `oc`     - The address to bind, this end of the link.
/// * `ground` - Where the ground is, which is sent to; `None` for a payload
///              with no `tcspecial` section to count it from.
///
/// # Returns
///
/// The reader and the writer, the writer a duplicate of the reader.
///
/// # Errors
///
/// What opening or duplicating the socket failed with: see
/// [`bind_failed`] for an address that cannot be bound.
pub fn bind_oc_pair(
    what: &str,
    oc: &NetworkConfig,
    ground: Option<&NetworkConfig>,
) -> TcsResult<(
    Box<dyn EndpointReadable + Send>,
    Box<dyn EndpointWritable + Send>,
)> {
    let reader = match ground {
        Some(ground) => UdpEndpoint::sending_to(what, oc, ground)?,
        None => UdpEndpoint::new(what, oc)?,
    };
    let writer = reader.try_clone()?;
    Ok((Box::new(reader), Box::new(writer)))
}

/// Opens a reader and a writer that wait at one endpoint, opened once.
///
/// For the OC side of a data handler, which the ground sends to at an address
/// it was told, so this end binds. See [`connect_endpoint_pair`] for the
/// payload side, which reaches out instead.
///
/// A conduit pair needs both ends of the same endpoint: one conduit reads it
/// and the other writes it. Calling the two single factories for one
/// configuration opens it twice, which a network address does not allow --
/// `UdpSocket::bind` and `TcpListener::bind` both fail with `AddrInUse` the
/// second time -- so the endpoint is opened once here and the second handle is
/// a duplicate of the first.
///
/// # Arguments
///
/// * `what`   - What the endpoint is for, as errors name it.
/// * `config` - The endpoint to open.
///
/// # Returns
///
/// The reader and the writer, the writer a duplicate of the reader.
///
/// # Errors
///
/// What opening or duplicating the endpoint failed with, and
/// `TcsError::Config` for a network protocol this cannot bind or a type
/// this build does not contain.
pub fn bind_endpoint_pair(
    what: &str,
    config: &EndpointConfig,
) -> TcsResult<(
    Box<dyn EndpointReadable + Send>,
    Box<dyn EndpointWritable + Send>,
)> {
    match config {
        EndpointConfig::Network(net_config) => match net_config.protocol {
            NetworkProtocol::Udp => {
                let reader = UdpEndpoint::new(what, net_config)?;
                let writer = reader.try_clone()?;
                Ok((Box::new(reader), Box::new(writer)))
            }
            NetworkProtocol::Tcp => {
                let reader = TcpEndpoint::new_server(what, net_config)?;
                let writer = reader.try_clone()?;
                Ok((Box::new(reader), Box::new(writer)))
            }
            #[cfg(feature = "raw")]
            NetworkProtocol::Raw(_) => {
                let reader = RawEndpoint::connect(what, net_config)?;
                let writer = reader.try_clone()?;
                Ok((Box::new(reader), Box::new(writer)))
            }
            #[cfg(not(feature = "raw"))]
            NetworkProtocol::Raw(_) => {
                Err(not_built(what, tcslib_protocol::RAW))
            }
            _ => Err(TcsError::Config(
                "Unsupported network protocol".to_string(),
            )),
        },
        #[cfg(feature = "device-file")]
        EndpointConfig::Device(dev_config) => {
            let reader = DeviceEndpoint::new(what, dev_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // Each of these is a device file with terms of its own, applied when
        // it is opened: see the file for the kind. The duplicate carries what
        // was set rather than setting it again.
        #[cfg(feature = "serial")]
        EndpointConfig::Serial(serial_config) => {
            let reader = SerialEndpoint::new(what, serial_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        #[cfg(feature = "i2c")]
        EndpointConfig::I2c(i2c_config) => {
            let reader = I2cEndpoint::new(what, i2c_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        #[cfg(feature = "spi")]
        EndpointConfig::Spi(spi_config) => {
            let reader = SpiEndpoint::new(what, spi_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // A sensor on a thermal mass is opened as the device on its bus is:
        // see node::payload::i2c_tmass.
        #[cfg(feature = "i2c_tmass")]
        EndpointConfig::I2cTmass(i2c_config) => {
            let reader = I2cEndpoint::new(what, i2c_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        #[cfg(feature = "spi_tmass")]
        EndpointConfig::SpiTmass(spi_config) => {
            let reader = SpiEndpoint::new(what, spi_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // A CAN controller is an interface rather than a file, so this is a
        // socket -- but one socket, shared by the pair, as a device file is:
        // a second socket on the bus would be a second member of it with a
        // filter of its own. Neither bound to an address nor connected to
        // one, so both factories do the same thing with it.
        #[cfg(feature = "can")]
        EndpointConfig::Can(can_config) => {
            let reader = CanEndpoint::new(what, can_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // A sensor's buffer: opened and read, and the writer it is paired
        // with has nothing to write. See endpoint_iio.
        #[cfg(feature = "iio")]
        EndpointConfig::Iio(iio_config) => {
            let reader = IioEndpoint::new(what, iio_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // A channel to another core: a character device carrying whole
        // messages both ways. See endpoint_rpmsg.
        #[cfg(feature = "rpmsg")]
        EndpointConfig::Rpmsg(rpmsg_config) => {
            let reader = RpmsgEndpoint::new(what, rpmsg_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // Discrete lines, watched for changes or driven: the request is made
        // once and the pair shares its descriptor, the kernel giving a line
        // to one request at a time, and what one half is told the other
        // reports -- a report asked for, a value passed on. See node/gpio.
        #[cfg(any(
            feature = "gpio_in",
            feature = "gpio_out",
            feature = "gpio_inout"
        ))]
        EndpointConfig::Gpio(gpio_config) => {
            let reader = GpioEndpoint::new(what, gpio_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // A synchronous line, which the kernel drives as a network interface:
        // the line is set up once and the pair shares the socket its frames
        // come and go on. See node/hdlc.
        #[cfg(feature = "hdlc")]
        EndpointConfig::Hdlc(hdlc_config) => {
            let reader = HdlcEndpoint::new(what, hdlc_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // A type this build does not contain. A configuration with one is
        // refused as it is loaded, so this is reached only by an open nothing
        // checked first; it says what the load would have. Unreachable in a
        // build with every type, which is what the allow is for.
        #[allow(unreachable_patterns)]
        other => Err(not_built(what, other.kind().spelling())),
    }
}

/// Opens a reader and a writer for a payload that is spoken to first, once.
///
/// For what the arrangement below cannot serve. A UDP payload that speaks first
/// is reached by binding its address and waiting to hear from it, and a payload
/// that is spoken to first has nothing to say until it is -- so the wait would
/// never end and every write would be refused for want of a peer. A heater is
/// such a payload, and so is a valve: the vehicle decides and the device obeys.
/// So is a sensor that is asked: the trigger is the first word.
/// [`tcslib_protocol::graph::interface::speaks_first`] says which payloads
/// these are.
///
/// So this sends where the node says the device is, and binds a port of its
/// own that nothing has to find, which is where an answer comes back to. See
/// [`UdpEndpoint::reaching`].
///
/// A Unix datagram payload has the same shape and is reached the same way:
/// see [`UnixDatagramEndpoint::reaching`]. Its socket is bound to a name the
/// kernel chooses, outside the filesystem, so that an answer has somewhere to
/// come back to and nothing is left behind when the node goes.
///
/// Nothing else differs. A TCP or Unix stream payload is connected to
/// whichever way it is used, a device is opened, and both work as well
/// written as read, so for anything else this is [`connect_endpoint_pair`].
///
/// # Arguments
///
/// * `what`   - What the endpoint is for, as errors name it.
/// * `config` - The endpoint to open.
///
/// # Returns
///
/// The reader and the writer, the writer a duplicate of the reader.
///
/// # Errors
///
/// What opening or duplicating the endpoint failed with, as for
/// [`connect_endpoint_pair`].
pub fn reaching_endpoint_pair(
    what: &str,
    config: &EndpointConfig,
) -> TcsResult<(
    Box<dyn EndpointReadable + Send>,
    Box<dyn EndpointWritable + Send>,
)> {
    match config {
        EndpointConfig::Network(net_config)
            if net_config.protocol == NetworkProtocol::Udp =>
        {
            let reader = UdpEndpoint::reaching(what, net_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        EndpointConfig::Network(net_config)
            if net_config.protocol == NetworkProtocol::UnixDgram =>
        {
            let reader = UnixDatagramEndpoint::reaching(what, net_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        _ => connect_endpoint_pair(what, config),
    }
}

/// Opens a reader and a writer for the payload side of a data handler, once.
///
/// Which end waits at the configured address depends on the protocol, and it
/// follows from which end can speak first.
///
/// A TCP payload listens and the handler connects to it: the payload is what
/// exists at an address, and a stream has to be accepted before anything can
/// be sent either way. A UDP payload cannot be reached that way, because a
/// UDP connect sends nothing: a payload bound at that address would hear
/// nothing and -- producing data because it is running rather than because it
/// was asked -- would have nowhere to send it. So for UDP the handler binds
/// the address and the payload reaches out to it, and the handler learns
/// where its payload is from the first packet that arrives -- for a payload
/// that speaks first; one that is spoken to first is reached by
/// [`reaching_endpoint_pair`] instead. Both ends binding it is what they used
/// to do, and an address can be bound once, so whichever started second got
/// `AddrInUse` and no data moved at all.
///
/// A device is neither bound nor connected -- it is opened -- so for one of
/// those this is [`bind_endpoint_pair`] by another name. Two opens of a device
/// are fine where two binds of a socket are not, which is why the device
/// handler was the only one that ever worked.
///
/// # Arguments
///
/// * `what`   - What the endpoint is for, as errors name it.
/// * `config` - The endpoint to open.
///
/// # Returns
///
/// The reader and the writer, the writer a duplicate of the reader.
///
/// # Errors
///
/// As for [`hearing_endpoint_pair`].
pub fn connect_endpoint_pair(
    what: &str,
    config: &EndpointConfig,
) -> TcsResult<(
    Box<dyn EndpointReadable + Send>,
    Box<dyn EndpointWritable + Send>,
)> {
    hearing_endpoint_pair(what, config, None)
}

/// Opens what [`connect_endpoint_pair`] does, for a payload whose node may
/// say where it sends from.
///
/// A datagram payload that speaks first is learnt from what arrives, so the
/// first sender heard would otherwise be taken for it, whoever that was.
/// Where its node states `sends_from`, its datagrams are read from there and
/// nowhere else, as a payload reached at a stated address is read: see
/// [`EndpointReadable::strangers`]. Nothing changes for anything else.
///
/// # Arguments
///
/// * `what`       - What the endpoint is for, as errors name it.
/// * `config`     - The endpoint to open.
/// * `sends_from` - Where a datagram payload's datagrams are read from, and
///                  nowhere else; `None` to learn it from the first to
///                  arrive. Ignored for anything but a datagram payload.
///
/// # Returns
///
/// The reader and the writer, the writer a duplicate of the reader.
///
/// # Errors
///
/// What opening, restricting or duplicating the endpoint failed with, and
/// `TcsError::Config` for a type this build does not contain.
pub fn hearing_endpoint_pair(
    what: &str,
    config: &EndpointConfig,
    sends_from: Option<&SendsFrom>,
) -> TcsResult<(
    Box<dyn EndpointReadable + Send>,
    Box<dyn EndpointWritable + Send>,
)> {
    match config {
        EndpointConfig::Network(net_config) => match net_config.protocol {
            // Bound, not connected: the payload is the end that speaks first
            // over UDP, so this is the end that has to be findable.
            NetworkProtocol::Udp => {
                let mut reader = UdpEndpoint::new(what, net_config)?;
                if let Some(from) = sends_from {
                    reader.hear_only(what, from)?;
                }
                let writer = reader.try_clone()?;
                Ok((Box::new(reader), Box::new(writer)))
            }
            NetworkProtocol::Tcp => {
                let reader = TcpEndpoint::connect_retrying(what, net_config)?;
                let writer = reader.try_clone()?;
                Ok((Box::new(reader), Box::new(writer)))
            }
            // A Unix socket divides as TCP and UDP divide, for the same
            // reason: a stream is accepted and a datagram is not. The
            // difference is only that both are named by a path.
            NetworkProtocol::UnixStream => {
                let reader =
                    UnixStreamEndpoint::connect_retrying(what, net_config)?;
                let writer = reader.try_clone()?;
                Ok((Box::new(reader), Box::new(writer)))
            }
            NetworkProtocol::UnixDgram => {
                let mut reader = UnixDatagramEndpoint::bind(what, net_config)?;
                if let Some(from) = sends_from {
                    reader.hear_only(what, from)?;
                }
                let writer = reader.try_clone()?;
                Ok((Box::new(reader), Box::new(writer)))
            }
            // Connected, whichever way the payload is used: a raw socket has
            // no port to wait at, and a connected one hears only its payload.
            // See node/payload/network/raw.
            #[cfg(feature = "raw")]
            NetworkProtocol::Raw(_) => {
                let reader = RawEndpoint::connect(what, net_config)?;
                let writer = reader.try_clone()?;
                Ok((Box::new(reader), Box::new(writer)))
            }
            #[cfg(not(feature = "raw"))]
            NetworkProtocol::Raw(_) => {
                Err(not_built(what, tcslib_protocol::RAW))
            }
        },
        #[cfg(feature = "device-file")]
        EndpointConfig::Device(dev_config) => {
            let reader = DeviceEndpoint::new(what, dev_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // Each of these is a device file with terms of its own, applied when
        // it is opened: see the file for the kind. The duplicate carries what
        // was set rather than setting it again.
        #[cfg(feature = "serial")]
        EndpointConfig::Serial(serial_config) => {
            let reader = SerialEndpoint::new(what, serial_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        #[cfg(feature = "i2c")]
        EndpointConfig::I2c(i2c_config) => {
            let reader = I2cEndpoint::new(what, i2c_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        #[cfg(feature = "spi")]
        EndpointConfig::Spi(spi_config) => {
            let reader = SpiEndpoint::new(what, spi_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // A sensor on a thermal mass is opened as the device on its bus is:
        // see node::payload::i2c_tmass.
        #[cfg(feature = "i2c_tmass")]
        EndpointConfig::I2cTmass(i2c_config) => {
            let reader = I2cEndpoint::new(what, i2c_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        #[cfg(feature = "spi_tmass")]
        EndpointConfig::SpiTmass(spi_config) => {
            let reader = SpiEndpoint::new(what, spi_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // A CAN controller is an interface rather than a file, so this is a
        // socket -- but one socket, shared by the pair, as a device file is:
        // a second socket on the bus would be a second member of it with a
        // filter of its own. Neither bound to an address nor connected to
        // one, so both factories do the same thing with it.
        #[cfg(feature = "can")]
        EndpointConfig::Can(can_config) => {
            let reader = CanEndpoint::new(what, can_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // A sensor's buffer: opened and read, and the writer it is paired
        // with has nothing to write. See endpoint_iio.
        #[cfg(feature = "iio")]
        EndpointConfig::Iio(iio_config) => {
            let reader = IioEndpoint::new(what, iio_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // A channel to another core: a character device carrying whole
        // messages both ways. See endpoint_rpmsg.
        #[cfg(feature = "rpmsg")]
        EndpointConfig::Rpmsg(rpmsg_config) => {
            let reader = RpmsgEndpoint::new(what, rpmsg_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // Discrete lines, watched for changes or driven: the request is made
        // once and the pair shares its descriptor, the kernel giving a line
        // to one request at a time, and what one half is told the other
        // reports -- a report asked for, a value passed on. See node/gpio.
        #[cfg(any(
            feature = "gpio_in",
            feature = "gpio_out",
            feature = "gpio_inout"
        ))]
        EndpointConfig::Gpio(gpio_config) => {
            let reader = GpioEndpoint::new(what, gpio_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // A synchronous line, which the kernel drives as a network interface:
        // the line is set up once and the pair shares the socket its frames
        // come and go on. See node/hdlc.
        #[cfg(feature = "hdlc")]
        EndpointConfig::Hdlc(hdlc_config) => {
            let reader = HdlcEndpoint::new(what, hdlc_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // A type this build does not contain. A configuration with one is
        // refused as it is loaded, so this is reached only by an open nothing
        // checked first; it says what the load would have. Unreachable in a
        // build with every type, which is what the allow is for.
        #[allow(unreachable_patterns)]
        other => Err(not_built(what, other.kind().spelling())),
    }
}

/// A reader whose datagrams are CCSDS telecommands under one APID: each
/// header is checked and taken off, and what is under it is what is read.
///
/// For the uplink of a gateway whose packets are CCSDS Space Packets: see
/// [`tcslib_protocol::ccsds`]. A datagram whose header is not one -- not a
/// telecommand, another APID, a length it does not say -- is passed over and
/// counted with the strangers, as a datagram from the wrong sender is:
/// nothing in it is acted on, and a flood of them is a count rather than a
/// line in the log apiece. A few at a time, as strangers are, so that a
/// flood costs a read a bounded amount of work.
pub struct CcsdsReader {
    /// What the datagrams are read from.
    inner: Box<dyn EndpointReadable + Send>,
    /// The APID they must carry.
    apid: u16,
    /// Datagrams passed over since [`EndpointReadable::strangers`] last
    /// asked.
    refused: u64,
}

impl CcsdsReader {
    /// Reads telecommands under `apid` from `inner`.
    ///
    /// # Arguments
    ///
    /// * `inner` - What the datagrams are read from.
    /// * `apid`  - The APID they must carry.
    ///
    /// # Returns
    ///
    /// The reader, with nothing passed over yet.
    pub fn of(inner: Box<dyn EndpointReadable + Send>, apid: u16) -> Self {
        CcsdsReader {
            inner,
            apid,
            refused: 0,
        }
    }
}

impl EndpointWaitable for CcsdsReader {
    fn io_fd(&self) -> RawFd {
        self.inner.io_fd()
    }

    fn wait_for_event(
        &self,
        cmd_fd: RawFd,
        timeout_ms: i32,
    ) -> TcsResult<WaitResult> {
        self.inner.wait_for_event(cmd_fd, timeout_ms)
    }
}

impl EndpointReadable for CcsdsReader {
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        for _ in 0..STRANGERS_PER_READ {
            let got = self.inner.read(buffer)?;
            if got == 0 {
                return Ok(0);
            }
            match tcslib_protocol::ccsds::read_header(
                &buffer[..got],
                tcslib_protocol::ccsds::PacketType::Telecommand,
                self.apid,
            ) {
                Ok(data) => {
                    // Moved down over the header rather than copied out, so
                    // that reading allocates nothing.
                    let size = data.len();
                    buffer.copy_within(
                        tcslib_protocol::ccsds::HEADER_BYTES..got,
                        0,
                    );
                    return Ok(size);
                }
                Err(_) => self.refused += 1,
            }
        }
        Ok(0)
    }

    fn strangers(&mut self) -> u64 {
        self.inner.strangers() + std::mem::take(&mut self.refused)
    }

    fn due(&self) -> bool {
        self.inner.due()
    }
}

/// A writer whose every datagram is a CCSDS telemetry packet under one
/// APID: a header goes on each, counted by the writer's own sequence count.
///
/// For a source's downlink through a gateway whose packets are CCSDS Space
/// Packets: see [`tcslib_protocol::ccsds`]. The packet is made in room the
/// writer keeps, which grows to the largest datum and no further, so that
/// writing allocates nothing once that has been seen.
pub struct CcsdsWriter {
    /// What the packets are written to.
    inner: Box<dyn EndpointWritable + Send>,
    /// The APID, the type and the count.
    sender: tcslib_protocol::ccsds::Sender,
    /// Where each packet is made.
    room: Vec<u8>,
}

impl CcsdsWriter {
    /// Writes telemetry under `apid` to `inner`.
    ///
    /// # Arguments
    ///
    /// * `inner` - What the packets are written to.
    /// * `apid`  - The APID they carry.
    ///
    /// # Returns
    ///
    /// The writer, its count at nought.
    ///
    /// # Errors
    ///
    /// `TcsError::Config` for an APID past eleven bits.
    pub fn of(
        inner: Box<dyn EndpointWritable + Send>,
        apid: u16,
    ) -> TcsResult<Self> {
        let sender = tcslib_protocol::ccsds::Sender::new(
            tcslib_protocol::ccsds::PacketType::Telemetry,
            apid,
        )
        .map_err(|fault| TcsError::Config(fault.to_string()))?;
        Ok(CcsdsWriter {
            inner,
            sender,
            room: Vec::new(),
        })
    }
}

impl EndpointWaitable for CcsdsWriter {
    fn io_fd(&self) -> RawFd {
        self.inner.io_fd()
    }

    fn wait_for_event(
        &self,
        cmd_fd: RawFd,
        timeout_ms: i32,
    ) -> TcsResult<WaitResult> {
        self.inner.wait_for_event(cmd_fd, timeout_ms)
    }
}

impl EndpointWritable for CcsdsWriter {
    /// Writes `data` as one packet, and says how much of `data` went: all of
    /// it, none, or what went after the header of a packet only partly
    /// written.
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        let total = tcslib_protocol::ccsds::HEADER_BYTES + data.len();
        if self.room.len() < total {
            self.room.resize(total, 0);
        }
        let packet = self
            .sender
            .frame(&mut self.room, data)
            .map_err(|fault| TcsError::Endpoint(fault.to_string()))?;
        let wrote = self.inner.write(packet)?;
        Ok(match wrote == packet.len() {
            true => data.len(),
            false => wrote.saturating_sub(tcslib_protocol::ccsds::HEADER_BYTES),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A framed writer's packets read back through a framed reader, under
    /// the one APID, and nothing else is read: an unframed datagram, one
    /// under another APID and telemetry where a telecommand belongs are
    /// each passed over and counted.
    #[test]
    fn framed_datagrams_round_trip_and_the_rest_are_counted() {
        use std::net::UdpSocket;
        let at = NetworkConfig {
            protocol: NetworkProtocol::Udp,
            address: "127.0.0.1".to_string(),
            port: 0,
        };
        let inner = UdpEndpoint::new("uplink", &at).expect("bound");
        // SAFETY: the endpoint's own descriptor, borrowed while it is open
        // and not kept.
        let borrowed = unsafe { BorrowedFd::borrow_raw(inner.io_fd()) };
        let here = socket2::SockRef::from(&borrowed)
            .local_addr()
            .ok()
            .and_then(|at| at.as_socket())
            .expect("where it is bound");
        let mut reader = CcsdsReader::of(Box::new(inner), 0x123);

        let ground = UdpSocket::bind("127.0.0.1:0").expect("a ground");
        let mut sender = tcslib_protocol::ccsds::Sender::new(
            tcslib_protocol::ccsds::PacketType::Telecommand,
            0x123,
        )
        .expect("an APID");
        let mut room = [0u8; 64];
        let mut wrong = tcslib_protocol::ccsds::Sender::new(
            tcslib_protocol::ccsds::PacketType::Telecommand,
            0x124,
        )
        .expect("an APID");
        let mut telemetry = tcslib_protocol::ccsds::Sender::new(
            tcslib_protocol::ccsds::PacketType::Telemetry,
            0x123,
        )
        .expect("an APID");
        ground.send_to(b"unframed", here).expect("sent");
        let packet = wrong.frame(&mut room, b"wrong").expect("framed");
        ground.send_to(packet, here).expect("sent");
        let packet = telemetry.frame(&mut room, b"tm").expect("framed");
        ground.send_to(packet, here).expect("sent");
        let packet = sender.frame(&mut room, b"set 21").expect("framed");
        ground.send_to(packet, here).expect("sent");

        let mut buffer = [0u8; 64];
        let mut got = 0;
        for _ in 0..200 {
            got = reader.read(&mut buffer).expect("a read");
            if got > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(&buffer[..got], b"set 21");
        assert_eq!(reader.strangers(), 3, "three passed over");
        assert_eq!(reader.strangers(), 0, "and counted once");

        // The other way: telemetry under the downlink's APID.
        let to = ground.local_addr().expect("where the ground is");
        let ground_at = NetworkConfig {
            protocol: NetworkProtocol::Udp,
            address: to.ip().to_string(),
            port: to.port(),
        };
        let (_, inner) =
            bind_oc_pair("downlink", &at, Some(&ground_at)).expect("bound");
        let mut writer = CcsdsWriter::of(inner, 0x201).expect("an APID");
        assert_eq!(writer.write(&[1, 2, 3]).expect("written"), 3);
        assert_eq!(writer.write(&[4]).expect("written"), 1);
        assert!(writer.write(&[]).is_err(), "an empty packet is refused");
        ground
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .expect("a timeout");
        let mut arrived = [0u8; 64];
        let (size, _) = ground.recv_from(&mut arrived).expect("a packet");
        assert_eq!(
            &arrived[..size],
            [0x02, 0x01, 0xC0, 0x00, 0x00, 0x02, 1, 2, 3]
        );
        let (size, _) = ground.recv_from(&mut arrived).expect("a packet");
        assert_eq!(&arrived[..size], [0x02, 0x01, 0xC0, 0x01, 0x00, 0x00, 4]);
    }

    #[test]
    fn test_wait_result() {
        assert_eq!(WaitResult::IoReady, WaitResult::IoReady);
        assert_ne!(WaitResult::IoReady, WaitResult::Timeout);
    }
}
