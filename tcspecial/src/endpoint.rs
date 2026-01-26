//! Endpoint implementations for TCSpecial
//!
//! Endpoints handle the low-level I/O operations for data handlers.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::os::unix::io::{AsRawFd, RawFd};
//use std::time::Duration;
use nix::poll::{poll, PollFd, PollFlags};
use std::os::fd::BorrowedFd;
use tcslibgs::{DeviceConfig, EndpointConfig, NetworkConfig, NetworkProtocol, TcsError, TcsResult};

use crate::config::constants::{ENDPOINT_BUFFER_SIZE, /*ENDPOINT_DELAY_INIT, ENDPOINT_DELAY_MAX, ENDPOINT_MAX_RETRIES*/};

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

/// Helper function to wait for events on file descriptors
fn wait_for_fds(io_fd: RawFd, cmd_fd: RawFd, io_events: PollFlags, timeout_ms: i32) -> TcsResult<WaitResult> {
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

/// UDP endpoint for network communication
pub struct UdpEndpoint {
    socket: UdpSocket,
    _buffer: Vec<u8>,
}

impl UdpEndpoint {
    pub fn new(config: &NetworkConfig) -> TcsResult<Self> {
        let addr = format!("{}:{}", config.address, config.port);
        let socket = UdpSocket::bind(&addr)?;
        socket.set_nonblocking(true)?;

        Ok(Self {
            socket,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }

    pub fn connect(&self, addr: &str) -> TcsResult<()> {
        self.socket.connect(addr)?;
        Ok(())
    }
}

impl EndpointWaitable for UdpEndpoint {
    fn io_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }

    fn wait_for_event(&self, cmd_fd: RawFd, timeout_ms: i32) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_fd(), cmd_fd, PollFlags::POLLIN, timeout_ms)
    }
}

impl EndpointReadable for UdpEndpoint {
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        match self.socket.recv(buffer) {
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(TcsError::Io(e)),
        }
    }
}

impl EndpointWritable for UdpEndpoint {
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        match self.socket.send(data) {
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(TcsError::Io(e)),
        }
    }
}

/// TCP endpoint for stream communication
pub struct TcpEndpoint {
    stream: Option<TcpStream>,
    listener: Option<TcpListener>,
    _buffer: Vec<u8>,
    _is_server: bool,
}

impl TcpEndpoint {
    pub fn new_server(config: &NetworkConfig) -> TcsResult<Self> {
        let addr = format!("{}:{}", config.address, config.port);
        let listener = TcpListener::bind(&addr)?;
        listener.set_nonblocking(true)?;

        Ok(Self {
            stream: None,
            listener: Some(listener),
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
            _is_server: true,
        })
    }

    pub fn new_client(config: &NetworkConfig) -> TcsResult<Self> {
        let addr = format!("{}:{}", config.address, config.port);
        let stream = TcpStream::connect(&addr)?;
        stream.set_nonblocking(true)?;

        Ok(Self {
            stream: Some(stream),
            listener: None,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
            _is_server: false,
        })
    }

    pub fn accept(&mut self) -> TcsResult<bool> {
        if let Some(ref listener) = self.listener {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(true)?;
                    self.stream = Some(stream);
                    Ok(true)
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(false),
                Err(e) => Err(TcsError::Io(e)),
            }
        } else {
            Ok(false)
        }
    }

    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }
}

impl EndpointWaitable for TcpEndpoint {
    fn io_fd(&self) -> RawFd {
        if let Some(ref stream) = self.stream {
            stream.as_raw_fd()
        } else if let Some(ref listener) = self.listener {
            listener.as_raw_fd()
        } else {
            -1
        }
    }

    fn wait_for_event(&self, cmd_fd: RawFd, timeout_ms: i32) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_fd(), cmd_fd, PollFlags::POLLIN, timeout_ms)
    }
}

impl EndpointReadable for TcpEndpoint {
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        if let Some(ref mut stream) = self.stream {
            match stream.read(buffer) {
                Ok(n) => Ok(n),
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
                Err(e) => Err(TcsError::Io(e)),
            }
        } else {
            Ok(0)
        }
    }
}

impl EndpointWritable for TcpEndpoint {
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        if let Some(ref mut stream) = self.stream {
            match stream.write(data) {
                Ok(n) => Ok(n),
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
                Err(e) => Err(TcsError::Io(e)),
            }
        } else {
            Ok(0)
        }
    }
}

/// Device endpoint for device file I/O
pub struct DeviceEndpoint {
    file: File,
    _buffer: Vec<u8>,
}

impl DeviceEndpoint {
    pub fn new(config: &DeviceConfig) -> TcsResult<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&config.path)?;

        Ok(Self {
            file,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }
}

impl EndpointWaitable for DeviceEndpoint {
    fn io_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    fn wait_for_event(&self, cmd_fd: RawFd, timeout_ms: i32) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_fd(), cmd_fd, PollFlags::POLLIN, timeout_ms)
    }
}

impl EndpointReadable for DeviceEndpoint {
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        match self.file.read(buffer) {
            Ok(n) => Ok(n),
            Err(e) => Err(TcsError::Io(e)),
        }
    }
}

impl EndpointWritable for DeviceEndpoint {
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        match self.file.write(data) {
            Ok(n) => Ok(n),
            Err(e) => Err(TcsError::Io(e)),
        }
    }
}

/// Factory for creating endpoints from configuration
pub fn create_reader_endpoint(config: &EndpointConfig) -> TcsResult<Box<dyn EndpointReadable + Send>> {
    match config {
        EndpointConfig::Network(net_config) => {
            match net_config.protocol {
                NetworkProtocol::Udp => {
                    Ok(Box::new(UdpEndpoint::new(net_config)?))
                }
                NetworkProtocol::Tcp => {
                    Ok(Box::new(TcpEndpoint::new_server(net_config)?))
                }
                _ => Err(TcsError::Config("Unsupported network protocol".to_string())),
            }
        }
        EndpointConfig::Device(dev_config) => {
            Ok(Box::new(DeviceEndpoint::new(dev_config)?))
        }
    }
}

/// Factory for creating endpoints from configuration
pub fn create_writer_endpoint(config: &EndpointConfig) -> TcsResult<Box<dyn EndpointWritable + Send>> {
    match config {
        EndpointConfig::Network(net_config) => {
            match net_config.protocol {
                NetworkProtocol::Udp => {
                    Ok(Box::new(UdpEndpoint::new(net_config)?))
                }
                NetworkProtocol::Tcp => {
                    Ok(Box::new(TcpEndpoint::new_server(net_config)?))
                }
                _ => Err(TcsError::Config("Unsupported network protocol".to_string())),
            }
        }
        EndpointConfig::Device(dev_config) => {
            Ok(Box::new(DeviceEndpoint::new(dev_config)?))
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
