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
//! * [`crate::endpoint_udp`] -- datagram sockets
//! * [`crate::endpoint_tcp`] -- stream sockets
//! * [`crate::endpoint_device`] -- device files
//! * [`crate::endpoint_network`] -- the address family, socket type and
//!   protocol tables the two socket kinds draw on

use std::io;
use std::os::fd::BorrowedFd;
use std::os::unix::io::RawFd;

use nix::poll::{poll, PollFd, PollFlags};
use tcslibgs::{EndpointConfig, NetworkProtocol, TcsError, TcsResult};

use crate::endpoint_device::DeviceEndpoint;
use crate::endpoint_i2c::I2cEndpoint;
use crate::endpoint_serial::SerialEndpoint;
use crate::endpoint_spi::SpiEndpoint;
use crate::endpoint_tcp::TcpEndpoint;
use crate::endpoint_udp::UdpEndpoint;
use crate::endpoint_unix::{UnixDatagramEndpoint, UnixStreamEndpoint};

/// Trait for endpoints that can wait for events
pub trait EndpointWaitable {
    /// Get the I/O file descriptor
    fn io_fd(&self) -> RawFd;

    /// Wait for an event on this endpoint
    fn wait_for_event(&self, cmd_fd: RawFd, timeout_ms: i32) -> TcsResult<WaitResult>;
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
}

/// Trait for writable endpoints
pub trait EndpointWritable: EndpointWaitable {
    /// Write data to the endpoint
    fn write(&mut self, data: &[u8]) -> TcsResult<usize>;
}

/// Wait for either the endpoint or the command pipe to have something.
///
/// Shared by every kind of endpoint: what differs between them is what
/// the descriptor refers to, not how it is waited on.
pub(crate) fn wait_for_fds(io_fd: RawFd, cmd_fd: RawFd, io_events: PollFlags, timeout_ms: i32) -> TcsResult<WaitResult> {
    let io_borrowed = unsafe { BorrowedFd::borrow_raw(io_fd) };
    let cmd_borrowed = unsafe { BorrowedFd::borrow_raw(cmd_fd) };

    let mut poll_fds = [
        PollFd::new(&io_borrowed, io_events),
        PollFd::new(&cmd_borrowed, PollFlags::POLLIN),
    ];

    match poll(&mut poll_fds, timeout_ms) {
        Ok(0) => Ok(WaitResult::Timeout),
        Ok(_) => {
            let io_ready = poll_fds[0].revents().map_or(false, |r| r.intersects(io_events));
            let cmd_ready = poll_fds[1].revents().map_or(false, |r| r.contains(PollFlags::POLLIN));

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
pub fn bind_endpoint_pair(
    what: &str,
    config: &EndpointConfig,
) -> TcsResult<(Box<dyn EndpointReadable + Send>, Box<dyn EndpointWritable + Send>)> {
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
            _ => Err(TcsError::Config("Unsupported network protocol".to_string())),
        },
        EndpointConfig::Device(dev_config) => {
            let reader = DeviceEndpoint::new(what, dev_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // Each of these is a device file with terms of its own, applied when
        // it is opened: see the file for the kind. The duplicate carries what
        // was set rather than setting it again.
        EndpointConfig::Serial(serial_config) => {
            let reader = SerialEndpoint::new(what, serial_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        EndpointConfig::I2c(i2c_config) => {
            let reader = I2cEndpoint::new(what, i2c_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        EndpointConfig::Spi(spi_config) => {
            let reader = SpiEndpoint::new(what, spi_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
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
/// where its payload is from the first packet that arrives. Both ends binding
/// it is what they used to do, and an address can be bound once, so whichever
/// started second got `AddrInUse` and no data moved at all.
///
/// A device is neither bound nor connected -- it is opened -- so for one of
/// those this is [`bind_endpoint_pair`] by another name. Two opens of a device
/// are fine where two binds of a socket are not, which is why the device
/// handler was the only one that ever worked.
pub fn connect_endpoint_pair(
    what: &str,
    config: &EndpointConfig,
) -> TcsResult<(Box<dyn EndpointReadable + Send>, Box<dyn EndpointWritable + Send>)> {
    match config {
        EndpointConfig::Network(net_config) => match net_config.protocol {
            // Bound, not connected: the payload is the end that speaks first
            // over UDP, so this is the end that has to be findable.
            NetworkProtocol::Udp => {
                let reader = UdpEndpoint::new(what, net_config)?;
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
                let reader = UnixStreamEndpoint::connect_retrying(what, net_config)?;
                let writer = reader.try_clone()?;
                Ok((Box::new(reader), Box::new(writer)))
            }
            NetworkProtocol::UnixDgram => {
                let reader = UnixDatagramEndpoint::bind(what, net_config)?;
                let writer = reader.try_clone()?;
                Ok((Box::new(reader), Box::new(writer)))
            }
        },
        EndpointConfig::Device(dev_config) => {
            let reader = DeviceEndpoint::new(what, dev_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        // Each of these is a device file with terms of its own, applied when
        // it is opened: see the file for the kind. The duplicate carries what
        // was set rather than setting it again.
        EndpointConfig::Serial(serial_config) => {
            let reader = SerialEndpoint::new(what, serial_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        EndpointConfig::I2c(i2c_config) => {
            let reader = I2cEndpoint::new(what, i2c_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
        EndpointConfig::Spi(spi_config) => {
            let reader = SpiEndpoint::new(what, spi_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
        }
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
