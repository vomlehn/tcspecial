//! The UDP endpoint: one datagram socket, read and written by two conduits.
//!
//! Used for both sides of a handler. The OC side binds the address the ground
//! was told to send to; the device side binds the address its device sends
//! to, since a datagram device is the end that speaks first. Either way the
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
    /// The handler this endpoint belongs to, as the device configuration
    /// names it, so that what this endpoint says of itself says which handler
    /// it is for.
    what: String,
    socket: UdpSocket,
    /// Where the other end of this link last spoke from.
    ///
    /// Learnt from what arrives, which is how the command interpreter answers
    /// the ground: `recv_from` then `send_to`. It is what a payload's own
    /// socket has to go on -- the payload is the end that speaks first over
    /// UDP and no configuration says where it will speak from.
    ///
    /// Shared with every duplicate of this endpoint, so that the conduit
    /// reading the socket teaches the conduit writing it where to send.
    peer: Arc<Mutex<Option<SocketAddr>>>,
    /// Or where the configuration says the other end is, for a link that is
    /// written down at both ends.
    ///
    /// A payload's ground link is: `payload_telemetry.dest_base` counted on by
    /// the payload's id, which is what the MOC binds. Stated beats learnt, so
    /// this sends there from the first datagram -- a ground station does not
    /// have to say hello to be sent to, and one that said hello once and was
    /// then cut off by a START_DH rebinding this socket is a failure mode
    /// that has gone with it.
    told: Option<SocketAddr>,
    _buffer: Vec<u8>,
}

impl UdpEndpoint {
    pub fn new(what: &str, config: &NetworkConfig) -> TcsResult<Self> {
        Self::bound(what, config, None)
    }

    /// An endpoint that sends where the configuration says, not where it was
    /// last spoken to from.
    ///
    /// For a link whose far end is written down: a payload's ground link, both
    /// ends of which come out of `payload_telemetry`. `dest` is that far end.
    pub fn sending_to(
        what: &str,
        config: &NetworkConfig,
        dest: &NetworkConfig,
    ) -> TcsResult<Self> {
        let told = format!("{}:{}", dest.address, dest.port)
            .parse::<SocketAddr>()
            .map_err(|e| {
                TcsError::Config(format!(
                    "{what}: \"{}:{}\" is where the configuration says the ground \
                     is, and it is not an address: {e}",
                    dest.address, dest.port
                ))
            })?;
        Self::bound(what, config, Some(told))
    }

    fn bound(
        what: &str,
        config: &NetworkConfig,
        told: Option<SocketAddr>,
    ) -> TcsResult<Self> {
        let addr = format!("{}:{}", config.address, config.port);
        let socket = UdpSocket::bind(&addr)
            .map_err(|e| bind_failed(&format!("{what} UDP endpoint"), &addr, e))?;
        socket.set_nonblocking(true)?;

        Ok(Self {
            what: what.to_string(),
            socket,
            peer: Arc::new(Mutex::new(None)),
            told,
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
            what: self.what.clone(),
            socket: self.socket.try_clone()?,
            // Shared, not copied: the point of the duplicate is that one
            // conduit reads this socket while another writes it, and only the
            // reader learns where the far end is.
            peer: self.peer.clone(),
            told: self.told,
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
        // Where the configuration says, or where the far end last spoke
        // from. Nowhere to send until one of the two is known: reported
        // rather than counted as a write of no bytes, because the data is
        // dropped and a write that moved nothing is not a write that
        // succeeded.
        let peer = self.told.or_else(|| self.peer()).ok_or_else(|| {
            TcsError::Endpoint(format!(
                "{}: nothing says where the far end is -- the configuration \
                 states none and nothing has been received on this socket yet",
                self.what
            ))
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

    /// An endpoint told where the far end is sends there, unprompted.
    ///
    /// Which is what a payload's ground link is: both ends are written in the
    /// configuration, so the first datagram of telemetry goes to the ground
    /// without the ground having said anything. It used to have to say hello
    /// once a second, because this socket is rebound by every START_DH and
    /// the peer it had learnt went with it.
    #[test]
    fn an_endpoint_told_where_the_ground_is_sends_there() {
        use std::net::UdpSocket;
        use std::time::Duration;
        use tcslibgs::NetworkProtocol;

        // The ground's end, bound and listening, and the vehicle's, which is
        // a port the system chooses.
        let ground = UdpSocket::bind("127.0.0.1:0").unwrap();
        ground
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let at = ground.local_addr().unwrap();

        let mine = NetworkConfig {
            protocol: NetworkProtocol::Udp,
            address: "127.0.0.1".to_string(),
            port: 0,
        };
        let theirs = NetworkConfig {
            protocol: NetworkProtocol::Udp,
            address: at.ip().to_string(),
            port: at.port(),
        };

        let reader = UdpEndpoint::sending_to("auto-send", &mine, &theirs).unwrap();
        let mut writer = reader.try_clone().unwrap();

        // Nothing has arrived on it, and it sends all the same.
        assert_eq!(
            writer.write(b"telemetry").expect("it knows where to send"),
            b"telemetry".len()
        );

        let mut buffer = [0u8; 64];
        let (size, _) = ground
            .recv_from(&mut buffer)
            .expect("the ground should be sent to without asking");
        assert_eq!(&buffer[..size], b"telemetry");

        // And what the configuration says beats what arrives: a datagram from
        // somewhere else does not move the link.
        let stranger = UdpSocket::bind("127.0.0.1:0").unwrap();
        let mut reading = reader;
        stranger
            .send_to(b"from the ground", mine_address(&writer))
            .expect("it is sent");
        let mut into = [0u8; 64];
        // Read so that the peer is learnt, which is what would have taken the
        // link over before.
        let _ = reading.read(&mut into);
        writer.write(b"again").expect("it still knows where to send");
        let (size, _) = ground
            .recv_from(&mut buffer)
            .expect("the stranger should not have taken the link");
        assert_eq!(&buffer[..size], b"again");
    }

    /// Where an endpoint's own socket is bound, for a test that has to send
    /// to it.
    fn mine_address(endpoint: &UdpEndpoint) -> std::net::SocketAddr {
        endpoint.socket.local_addr().expect("where it is bound")
    }

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

        let mut reader = UdpEndpoint::new("beacon", &config).unwrap();
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
