//! The UDP endpoint: one datagram socket, read and written by two conduits.
//!
//! Used for both sides of a handler. The OC side binds the address the ground
//! was told to send to; the payload side binds the address its payload sends
//! to, since a datagram payload is the end that speaks first. Either way the
//! far end is learnt rather than configured -- a datagram's sender is the only
//! statement of where it came from -- which is why a write before anything has
//! been read is an error rather than a write of no bytes.
//!
//! See [`crate::endpoint`] for the traits, and
//! [`crate::endpoint::connect_endpoint_pair`] for which end binds what.

use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::{Arc, Mutex};

use nix::poll::PollFlags;
use tcslibgs::{NetworkConfig, TcsError, TcsResult};

use crate::ci::bind_failed;
use crate::config::constants::ENDPOINT_BUFFER_SIZE;
use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable, WaitResult,
};

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
}
