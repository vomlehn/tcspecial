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
use tcslibgs::{NetworkConfig, SendsFrom, TcsError, TcsResult};

use crate::config::constants::ENDPOINT_BUFFER_SIZE;
use crate::endpoint::bind_failed;
use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable, WaitResult,
    STRANGERS_PER_READ,
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
    /// The one sender whose datagrams are read, for a payload that is
    /// reached where its node says it is, or that speaks first from where
    /// its node says.
    ///
    /// Everything else that arrives is a stranger's: turned away and
    /// counted, rather than handed on as a reading from the payload. Only
    /// where the far end is known before it speaks -- see
    /// [`UdpEndpoint::reaching`] and [`UdpEndpoint::hear_only`]; a payload
    /// that speaks first and says nothing of where from is learnt from what
    /// arrives.
    only_from: Option<SocketAddr>,
    /// Datagrams turned away since [`EndpointReadable::strangers`] last asked.
    strangers: u64,
    _buffer: Vec<u8>,
}

impl UdpEndpoint {
    pub fn new(what: &str, config: &NetworkConfig) -> TcsResult<Self> {
        Self::bound(what, config, None)
    }

    /// An endpoint for a payload that is spoken to first: a heater, a valve,
    /// or a sensor that is asked.
    ///
    /// Such a payload says nothing until it is spoken to, so there is nothing
    /// to learn from it first, and the arrangement for a payload that speaks
    /// first cannot work: this end would bind the configured address and wait
    /// to be told where its device is by a device waiting to be told
    /// something. It would wait for ever, and every write would be refused for
    /// want of a peer -- a heater the vehicle decided to switch on and could
    /// not reach, a trigger with nowhere to go.
    ///
    /// So the two ends swap roles. What the node says is where the vehicle
    /// reaches the payload, which is what the attribute means everywhere else:
    /// the device waits there and this sends to it. The local port is
    /// whatever is free, because nothing has to find this end before it has
    /// spoken, and an answer comes back to it.
    pub fn reaching(what: &str, config: &NetworkConfig) -> TcsResult<Self> {
        let told = format!("{}:{}", config.address, config.port)
            .parse::<SocketAddr>()
            .map_err(|e| {
                TcsError::Config(format!(
                    "{what}: \"{}:{}\" is where the configuration says the \
                     device is, and it is not an address: {e}",
                    config.address, config.port
                ))
            })?;
        // Any interface, any free port. The family follows the device's, so
        // that a socket bound here can reach it at all.
        let mine = NetworkConfig {
            protocol: config.protocol,
            address: if told.is_ipv6() { "::" } else { "0.0.0.0" }.to_string(),
            port: 0,
        };
        let mut reaching = Self::bound(what, &mine, Some(told))?;
        reaching.only_from = Some(told);
        Ok(reaching)
    }

    /// An endpoint that sends where the configuration says, not where it was
    /// last spoken to from.
    ///
    /// For a link whose far end is written down: a payload's ground link, both
    /// ends of which come out of `payload_telemetry`. `dest` is that far end.
    pub fn sending_to(what: &str, config: &NetworkConfig, dest: &NetworkConfig) -> TcsResult<Self> {
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

    fn bound(what: &str, config: &NetworkConfig, told: Option<SocketAddr>) -> TcsResult<Self> {
        let addr = format!("{}:{}", config.address, config.port);
        let socket = UdpSocket::bind(&addr)
            .map_err(|e| bind_failed(&format!("{what} UDP endpoint"), &addr, e))?;
        socket.set_nonblocking(true)?;

        Ok(Self {
            what: what.to_string(),
            socket,
            peer: Arc::new(Mutex::new(None)),
            told,
            only_from: None,
            strangers: 0,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }

    pub fn connect(&self, addr: &str) -> TcsResult<()> {
        self.socket.connect(addr)?;
        Ok(())
    }

    /// Read datagrams from this sender and turn every other away, for a
    /// payload that speaks first and whose node says where from.
    pub fn hear_only(&mut self, what: &str, from: &SendsFrom) -> TcsResult<()> {
        match from {
            SendsFrom::Socket(at) => {
                self.only_from = Some(*at);
                Ok(())
            }
            SendsFrom::Path(path) => Err(TcsError::Config(format!(
                "{what}: sends_from {path:?} is a path, and this is a UDP socket, \
                 whose payload sends from a host and a port"
            ))),
        }
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
            only_from: self.only_from,
            // Counted where they are read, which is one of the pair.
            strangers: 0,
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
        //
        // A stranger's datagram is passed over and the next one looked at,
        // a few at a time: a flood of them costs this read a bounded amount
        // of work rather than the pass, and what is left is looked at on the
        // next.
        for _ in 0..STRANGERS_PER_READ {
            match self.socket.recv_from(buffer) {
                Ok((_, from)) if self.only_from.is_some_and(|only| from != only) => {
                    self.strangers += 1;
                }
                Ok((n, from)) => {
                    if let Ok(mut peer) = self.peer.lock() {
                        *peer = Some(from);
                    }
                    return Ok(n);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(0),
                Err(e) => return Err(TcsError::Io(e)),
            }
        }
        Ok(0)
    }

    fn strangers(&mut self) -> u64 {
        std::mem::take(&mut self.strangers)
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
        writer
            .write(b"again")
            .expect("it still knows where to send");
        let (size, _) = ground
            .recv_from(&mut buffer)
            .expect("the stranger should not have taken the link");
        assert_eq!(&buffer[..size], b"again");
    }

    /// A payload that is only driven is sent to where the node says it is.
    ///
    /// The case the ordinary arrangement cannot serve. A heater never speaks,
    /// so an endpoint that bound its address and waited to learn where it was
    /// would wait for ever and refuse every write for want of a peer -- the
    /// vehicle deciding to switch the heaters on and having nowhere to send
    /// it. Here the device is the end that waits and the vehicle sends to the
    /// configured address, which is what that address means.
    #[test]
    fn a_driven_payload_is_sent_to_where_the_node_says_it_is() {
        use std::net::UdpSocket;
        use std::time::Duration;
        use tcslibgs::NetworkProtocol;

        // The heater, waiting where the configuration says it is.
        let heater = UdpSocket::bind("127.0.0.1:0").unwrap();
        heater
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let at = heater.local_addr().unwrap();

        let device = NetworkConfig {
            protocol: NetworkProtocol::Udp,
            address: at.ip().to_string(),
            port: at.port(),
        };

        let reader = UdpEndpoint::reaching("heaters_1", &device).unwrap();
        let mut writer = reader.try_clone().unwrap();

        // Not bound to the device's address: that is the device's, and two
        // binds of one address is what the old arrangement would have come
        // to here.
        assert_ne!(
            mine_address(&writer),
            at,
            "the vehicle bound the address its device is waiting at"
        );

        // The heater has said nothing, and the decision reaches it anyway.
        assert_eq!(writer.write(b"1").expect("it knows where to send"), 1);
        let mut buffer = [0u8; 8];
        let (size, from) = heater
            .recv_from(&mut buffer)
            .expect("a heater should be driven without asking to be");
        assert_eq!(&buffer[..size], b"1");
        // By port: this end is bound to any interface, so where it says it is
        // and where a datagram from it appears to come from differ in the
        // address and agree in the port.
        assert_eq!(from.port(), mine_address(&writer).port());

        // And what the node says goes on beating what arrives: a stranger
        // that speaks to this end does not become the heater.
        let stranger = UdpSocket::bind("127.0.0.1:0").unwrap();
        stranger
            .send_to(b"hello", ("127.0.0.1", mine_address(&writer).port()))
            .unwrap();
        let mut reading = reader;
        let mut into = [0u8; 64];
        let _ = reading.read(&mut into);
        writer.write(b"0").expect("it still knows where to send");
        let (size, _) = heater
            .recv_from(&mut buffer)
            .expect("the stranger should not have taken the link");
        assert_eq!(&buffer[..size], b"0");
    }

    /// A payload that is asked is sent its trigger where the node says it
    /// is, before it has said anything, and its answer comes back.
    ///
    /// The arrangement for a payload that speaks first would bind that
    /// address and wait, and the trigger would have nowhere to go until the
    /// payload, waiting to be asked, had spoken -- which it never would.
    #[test]
    fn an_asked_payload_is_sent_its_trigger_where_the_node_says_it_is() {
        use std::net::UdpSocket;
        use std::time::{Duration, Instant};
        use tcslibgs::NetworkProtocol;

        // The sensor, waiting to be asked where the configuration says.
        let sensor = UdpSocket::bind("127.0.0.1:0").unwrap();
        sensor
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let at = sensor.local_addr().unwrap();
        let device = NetworkConfig {
            protocol: NetworkProtocol::Udp,
            address: at.ip().to_string(),
            port: at.port(),
        };

        let mut reader = UdpEndpoint::reaching("temp_1", &device).unwrap();
        let mut writer = reader.try_clone().unwrap();

        assert_eq!(writer.write(b"READ\r").expect("somewhere to send"), 5);
        let mut asked = [0u8; 8];
        let (size, from) = sensor
            .recv_from(&mut asked)
            .expect("the trigger should reach the sensor unprompted");
        assert_eq!(&asked[..size], b"READ\r");

        sensor.send_to(b"21.5", from).expect("the answer goes");
        let mut into = [0u8; 64];
        let deadline = Instant::now() + Duration::from_secs(2);
        let size = loop {
            match reader.read(&mut into).expect("a read") {
                0 if Instant::now() < deadline => std::thread::yield_now(),
                size => break size,
            }
        };
        assert_eq!(&into[..size], b"21.5");
    }

    /// A payload reached where it is reads only what comes from there.
    ///
    /// Anything else that can reach the vehicle's port would otherwise be
    /// handed on as the payload's reading -- a temperature nobody measured,
    /// voted on and acted on as though it were real. Turned away and
    /// counted, and the payload's own datagram behind it still read.
    #[test]
    fn a_stranger_is_turned_away_and_counted() {
        use std::net::UdpSocket;
        use tcslibgs::NetworkProtocol;

        let sensor = UdpSocket::bind("127.0.0.1:0").unwrap();
        let at = sensor.local_addr().unwrap();
        let device = NetworkConfig {
            protocol: NetworkProtocol::Udp,
            address: at.ip().to_string(),
            port: at.port(),
        };
        let mut reader = UdpEndpoint::reaching("temp_1", &device).unwrap();
        let mine = ("127.0.0.1", mine_address(&reader).port());

        let stranger = UdpSocket::bind("127.0.0.1:0").unwrap();
        stranger.send_to(b"99.9", mine).unwrap();
        stranger.send_to(b"99.9", mine).unwrap();
        sensor.send_to(b"21.5", mine).unwrap();

        let mut into = [0u8; 64];
        let size = read_soon(&mut reader, &mut into);
        assert_eq!(
            &into[..size],
            b"21.5",
            "the stranger's was read as the payload's"
        );
        assert_eq!(reader.strangers(), 2);
        assert_eq!(reader.strangers(), 0, "counted once, and asked once");
    }

    /// What arrives within two seconds: a datagram sent over the loopback is
    /// readable at once, nearly always, and the test is not about when.
    fn read_soon(reader: &mut UdpEndpoint, into: &mut [u8]) -> usize {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            match reader.read(into).expect("a read") {
                0 if std::time::Instant::now() < deadline => std::thread::yield_now(),
                size => return size,
            }
        }
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

        // Port 0, bound by the endpoint itself, so the test takes whatever is
        // free and cannot collide with another test or a running tcspecial.
        // The port is read back from the endpoint's own socket. It used to be
        // found by binding a socket of the test's, reading its port and
        // dropping it, and then bound again here -- and between the drop and
        // the bind another test could take it.
        let config = NetworkConfig {
            protocol: NetworkProtocol::Udp,
            address: "127.0.0.1".to_string(),
            port: 0,
        };

        let mut reader = UdpEndpoint::new("beacon", &config).unwrap();
        let addr = reader.socket.local_addr().unwrap();
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
        assert_eq!(
            from, addr,
            "the answer came from the address the ground sent to"
        );
    }
}
