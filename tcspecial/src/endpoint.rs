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
use tcslibgs::{
    EndpointConfig, NetworkConfig, NetworkProtocol, SendsFrom, TcsError,
    TcsResult,
};

#[cfg(feature = "can")]
use crate::node::payload::can::CanEndpoint;
#[cfg(feature = "device-file")]
use crate::node::payload::device_file::DeviceEndpoint;
#[cfg(feature = "gpio")]
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
/// [`tcslibgs::graph::built::not_built_message`]. Only a build that leaves a
/// type out can reach it.
#[allow(dead_code)]
fn not_built(what: &str, node_type: &str) -> TcsError {
    TcsError::Config(tcslibgs::graph::built::not_built_message(what, node_type))
}

/// Say what could not be bound, and what that usually means.
///
/// A bare io::Error carries the kind and nothing else, so the whole of what a
/// reader got was "Address already in use (os error 98)" -- not which address,
/// and no hint that the usual cause is a second copy of the program. Both ends
/// of a data handler bind an address too, so this is used for all of them.
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
    /// Get the I/O file descriptor
    fn io_fd(&self) -> RawFd;

    /// Wait for an event on this endpoint
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
    /// Read data from the endpoint
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
}

/// The most datagrams one read turns away before it gives up for the pass.
///
/// A bound on the work a flood of strangers can make a read do: the rest
/// wait for the next pass, and the payload's own datagrams with them, which
/// is a payload late by a pass rather than a vehicle that stops passing.
pub(crate) const STRANGERS_PER_READ: usize = 16;

/// Trait for writable endpoints
pub trait EndpointWritable: EndpointWaitable {
    /// Write data to the endpoint
    fn write(&mut self, data: &[u8]) -> TcsResult<usize>;
}

/// Wait for either the endpoint or the command pipe to have something.
///
/// Shared by every kind of endpoint: what differs between them is what
/// the descriptor refers to, not how it is waited on.
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

/// A reader and a writer that wait at one endpoint, opened once.
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
/// Open a payload's ground link: bound here, sending where the file says.
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
            NetworkProtocol::Raw(_) => Err(not_built(what, tcslibgs::RAW)),
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
        // to one request at a time. Which half of the pair does anything
        // follows from the configured direction, and the other half says so.
        // See node/gpio.
        #[cfg(feature = "gpio")]
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

/// A reader and a writer for a payload that is spoken to first, opened once.
///
/// For what the arrangement below cannot serve. A UDP payload that speaks
/// first is reached by binding its address and waiting to hear from it, and
/// a payload that is spoken to first has nothing to say until it is -- so the
/// wait would never end and every write would be refused for want of a peer.
/// A heater is such a payload, and so is a valve: the vehicle decides and the
/// device obeys. So is a sensor that is asked: the trigger is the first word.
/// [`tcslibgs::graph::interface::speaks_first`] says which payloads these are.
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

/// A reader and a writer for the payload side of a data handler, opened once.
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
pub fn connect_endpoint_pair(
    what: &str,
    config: &EndpointConfig,
) -> TcsResult<(
    Box<dyn EndpointReadable + Send>,
    Box<dyn EndpointWritable + Send>,
)> {
    hearing_endpoint_pair(what, config, None)
}

/// [`connect_endpoint_pair`], for a payload whose node may say where it
/// sends from.
///
/// A datagram payload that speaks first is learnt from what arrives, so the
/// first sender heard would otherwise be taken for it, whoever that was.
/// Where its node states `sends_from`, its datagrams are read from there and
/// nowhere else, as a payload reached at a stated address is read: see
/// [`EndpointReadable::strangers`]. Nothing changes for anything else.
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
            NetworkProtocol::Raw(_) => Err(not_built(what, tcslibgs::RAW)),
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
        // to one request at a time. Which half of the pair does anything
        // follows from the configured direction, and the other half says so.
        // See node/gpio.
        #[cfg(feature = "gpio")]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wait_result() {
        assert_eq!(WaitResult::IoReady, WaitResult::IoReady);
        assert_ne!(WaitResult::IoReady, WaitResult::Timeout);
    }
}
