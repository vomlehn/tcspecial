//! Endpoint implementations for TCSpecial
//!
//! Endpoints handle the low-level I/O operations for data handlers.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::{Arc, Mutex};
//use std::time::Duration;
use nix::poll::{poll, PollFd, PollFlags};
use std::os::fd::BorrowedFd;
use tcslibgs::{DeviceConfig, EndpointConfig, NetworkConfig, NetworkProtocol, TcsError, TcsResult};

use crate::ci::bind_failed;
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
    /// Where the other end of this link last spoke from.
    ///
    /// A handler's OC socket binds an address the ground sends to; sending
    /// back needs the ground's own address, which no configuration states.
    /// It is learnt from what arrives, which is how the command interpreter
    /// already answers the ground: `recv_from` then `send_to`.
    ///
    /// Shared with every duplicate of this endpoint, so that the conduit
    /// reading the socket teaches the conduit writing it where to send.
    peer: Arc<Mutex<Option<SocketAddr>>>,
    _buffer: Vec<u8>,
}

impl UdpEndpoint {
    pub fn new(config: &NetworkConfig) -> TcsResult<Self> {
        let addr = format!("{}:{}", config.address, config.port);
        let socket =
            UdpSocket::bind(&addr).map_err(|e| bind_failed("UDP endpoint", &addr, e))?;
        socket.set_nonblocking(true)?;

        Ok(Self {
            socket,
            peer: Arc::new(Mutex::new(None)),
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }

    pub fn connect(&self, addr: &str) -> TcsResult<()> {
        self.socket.connect(addr)?;
        Ok(())
    }

    /// A second endpoint on the same socket.
    ///
    /// An address can be bound once, so a conduit pair reading and writing one
    /// endpoint shares the socket rather than binding it twice.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            socket: self.socket.try_clone()?,
            // Shared, not copied: the point of the duplicate is that one
            // conduit reads this socket while another writes it, and only the
            // reader learns where the far end is.
            peer: self.peer.clone(),
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }

    /// Where the far end last spoke from, if it has.
    pub fn peer(&self) -> Option<SocketAddr> {
        self.peer.lock().ok().and_then(|guard| *guard)
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
        // recv_from rather than recv, so that answering is possible: a
        // datagram's sender is the only statement of where the far end is.
        match self.socket.recv_from(buffer) {
            Ok((n, from)) => {
                if let Ok(mut peer) = self.peer.lock() {
                    *peer = Some(from);
                }
                Ok(n)
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(TcsError::Io(e)),
        }
    }
}

impl EndpointWritable for UdpEndpoint {
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        // Nowhere to send until the far end has spoken. Reported rather than
        // counted as a write of no bytes, because the data is dropped and a
        // write that moved nothing is not a write that succeeded.
        let peer = self.peer().ok_or_else(|| {
            TcsError::Endpoint(
                "nothing has been received on this socket yet, so there is no \
                 address to send to"
                    .to_string(),
            )
        })?;

        match self.socket.send_to(data, peer) {
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
        let listener =
            TcpListener::bind(&addr).map_err(|e| bind_failed("TCP endpoint", &addr, e))?;
        listener.set_nonblocking(true)?;

        Ok(Self {
            stream: None,
            listener: Some(listener),
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
            _is_server: true,
        })
    }

    /// A second endpoint on the same listener or stream.
    ///
    /// See [`UdpEndpoint::try_clone`]: one bind, two handles.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            stream: match &self.stream {
                Some(stream) => Some(stream.try_clone()?),
                None => None,
            },
            listener: match &self.listener {
                Some(listener) => Some(listener.try_clone()?),
                None => None,
            },
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
            _is_server: self._is_server,
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

    /// A second endpoint on the same open file.
    ///
    /// A device could be opened twice where a socket could not, but one
    /// description is shared so that both ends of a conduit pair see one file
    /// position and one set of open flags.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            file: self.file.try_clone()?,
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

/// A reader and a writer for one endpoint, opened once.
///
/// A conduit pair needs both ends of the same endpoint: one conduit reads it
/// and the other writes it. Calling the two factories above for one
/// configuration opens it twice, which a network address does not allow --
/// `UdpSocket::bind` and `TcpListener::bind` both fail with `AddrInUse` the
/// second time -- so the endpoint is opened once here and the second handle is
/// a duplicate of the first.
pub fn create_endpoint_pair(
    config: &EndpointConfig,
) -> TcsResult<(Box<dyn EndpointReadable + Send>, Box<dyn EndpointWritable + Send>)> {
    match config {
        EndpointConfig::Network(net_config) => match net_config.protocol {
            NetworkProtocol::Udp => {
                let reader = UdpEndpoint::new(net_config)?;
                let writer = reader.try_clone()?;
                Ok((Box::new(reader), Box::new(writer)))
            }
            NetworkProtocol::Tcp => {
                let reader = TcpEndpoint::new_server(net_config)?;
                let writer = reader.try_clone()?;
                Ok((Box::new(reader), Box::new(writer)))
            }
            _ => Err(TcsError::Config("Unsupported network protocol".to_string())),
        },
        EndpointConfig::Device(dev_config) => {
            let reader = DeviceEndpoint::new(dev_config)?;
            let writer = reader.try_clone()?;
            Ok((Box::new(reader), Box::new(writer)))
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

    /// A handler's OC socket cannot send until the ground has spoken, and can
    /// afterwards.
    ///
    /// The writing conduit holds a duplicate of the socket the reading conduit
    /// holds, so this also checks that the address one learns is the address
    /// the other sends to.
    #[test]
    fn a_udp_endpoint_learns_where_to_answer() {
        use std::net::UdpSocket;
        use tcslibgs::NetworkProtocol;

        // Port 0, so the test takes whatever is free and cannot collide with
        // another test or a running tcspecial.
        let bound = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = bound.local_addr().unwrap();
        drop(bound);

        let config = NetworkConfig {
            protocol: NetworkProtocol::Udp,
            address: addr.ip().to_string(),
            port: addr.port(),
        };

        let mut reader = UdpEndpoint::new(&config).unwrap();
        let mut writer = reader.try_clone().unwrap();

        // Nothing has arrived, so there is nowhere to answer.
        assert!(
            writer.write(b"telemetry").is_err(),
            "sending with no known peer should be reported, not silently dropped"
        );

        let ground = UdpSocket::bind("127.0.0.1:0").unwrap();
        let ground_addr = ground.local_addr().unwrap();
        ground.send_to(b"command", addr).unwrap();

        // The socket is non-blocking, so a read may find nothing yet.
        let mut buffer = [0u8; 64];
        let mut got = 0;
        for _ in 0..100 {
            got = reader.read(&mut buffer).unwrap();
            if got > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(&buffer[..got], b"command");
        assert_eq!(reader.peer(), Some(ground_addr));
        // The duplicate learnt it too, which is the point of sharing.
        assert_eq!(writer.peer(), Some(ground_addr));

        let sent = writer.write(b"telemetry").unwrap();
        assert_eq!(sent, b"telemetry".len());

        ground
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let (n, from) = ground.recv_from(&mut buffer).unwrap();
        assert_eq!(&buffer[..n], b"telemetry");
        assert_eq!(from, addr, "the answer came from the address the ground sent to");
    }

    #[test]
    fn test_wait_result() {
        assert_eq!(WaitResult::IoReady, WaitResult::IoReady);
        assert_ne!(WaitResult::IoReady, WaitResult::Timeout);
    }
}

/*
 * use socket2::{Socket, Domain, Type, Protocol, SockAddr};
use std::net::SocketAddr;

fn main() -> std::io::Result<()> {
    // Equivalent to: socket(AF_INET, SOCK_STREAM, IPPROTO_TCP)
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;

    // Set options before connecting
    socket.set_reuse_address(true)?;
    socket.set_nodelay(true)?;

    // Connect
    let addr: SocketAddr = "127.0.0.1:7878".parse().unwrap();
    socket.connect(&SockAddr::from(addr))?;

    // Send/receive
    socket.send(b"Hello!")?;

    let mut buf = [0u8; 1024];
    let n = socket.recv(&mut buf)?;
    println!("Received: {}", String::from_utf8_lossy(&buf[..n]));

    Ok(())
}

use socket2::{Socket, Domain, Type, Protocol, SockAddr};
use std::net::SocketAddr;

fn main() -> std::io::Result<()> {
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;

    socket.set_reuse_address(true)?;

    let addr: SocketAddr = "127.0.0.1:7878".parse().unwrap();
    socket.bind(&SockAddr::from(addr))?;
    socket.listen(128)?; // backlog of 128

    let (client, client_addr) = socket.accept()?;
    println!("Connection from: {:?}", client_addr.as_socket());

    Ok(())
}

let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
socket.bind(&SockAddr::from(addr))?;
socket.send_to(b"ping", &SockAddr::from(remote_addr))?;

Key mappings to C
C                                           socket2
socket(AF_INET, SOCK_STREAM, 0) Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))

socket(AF_INET6, SOCK_DGRAM, 0) Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP))

socket(AF_UNIX, SOCK_STREAM, 0) Socket::new(Domain::UNIX, Type::STREAM, None)

setsockopt(...)                 socket.set_reuse_address(true), etc.

bind(), listen(), accept(), connect()   Same method names on Socket

Converting to std types
You can convert a socket2::Socket into a std::net::TcpStream (or TcpListener, UdpSocket) when you're done with low-level setup:

let std_stream: std::net::TcpStream = socket.into();

This is a common pattern: use socket2 for fine-grained control during setup, then convert to std types for ergonomic I/O.
 */
