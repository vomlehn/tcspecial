//! Connection management for ground-to-space communication

use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};
use tcslibgs::{came_from, Command, HostPort, TcsError, TcsResult, Telemetry};

/// Connection to the spacecraft
pub trait Connection: Send {
    /// Send a command to the spacecraft
    fn send(&mut self, command: &Command) -> TcsResult<()>;

    /// Receive telemetry from the spacecraft
    fn receive(&mut self) -> TcsResult<Telemetry>;

    /// Receive telemetry with a timeout
    fn receive_timeout(&mut self, timeout: Duration) -> TcsResult<Telemetry>;

    /// Check if there is data available to read
    fn has_data(&self) -> TcsResult<bool>;

    /// Close the connection
    fn close(&mut self) -> TcsResult<()>;
}

/// UDP-based connection to the spacecraft
pub struct UdpConnection {
    socket: UdpSocket,
    remote_addr: SocketAddr,
    recv_buffer: Vec<u8>,
}

impl UdpConnection {
    /// Create a new UDP connection
    pub fn new(local_addr: &str, remote_addr: &str) -> TcsResult<Self> {
        let socket = UdpSocket::bind(local_addr)?;
        let remote: SocketAddr = remote_addr
            .parse()
            .map_err(|e| TcsError::Config(format!("Invalid remote address: {}", e)))?;

        socket.set_nonblocking(false)?;

        Ok(Self {
            socket,
            remote_addr: remote,
            recv_buffer: vec![0u8; 65535],
        })
    }

    /// Connect to the remote address
    pub fn connect(&mut self) -> TcsResult<()> {
        self.socket.connect(self.remote_addr)?;
        Ok(())
    }

    /// Set the read timeout
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> TcsResult<()> {
        self.socket.set_read_timeout(timeout)?;
        Ok(())
    }

    /// Set the write timeout
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> TcsResult<()> {
        self.socket.set_write_timeout(timeout)?;
        Ok(())
    }

    /// The next answer from the spacecraft, and nothing from anywhere else.
    ///
    /// A response is only a response if it came from the socket the command
    /// went to -- `tcs_cmd.recv` for the spacecraft's own link, which is the
    /// address this connection sends to. A datagram from elsewhere is
    /// somebody else's traffic arriving at a bound port, and taking it for an
    /// answer would be letting whoever sent it say what the spacecraft is
    /// doing.
    ///
    /// Such a datagram is dropped and the wait goes on, up to `deadline` where
    /// there is one: a flood of them must not buy more time than the caller
    /// asked for, or a command would hang on traffic that has nothing to do
    /// with it.
    fn answer(&mut self, deadline: Option<Instant>) -> TcsResult<Telemetry> {
        loop {
            let (size, from) = self.socket.recv_from(&mut self.recv_buffer)?;
            let expected = HostPort {
                address: self.remote_addr.ip().to_string(),
                port: self.remote_addr.port(),
            };
            if came_from(&expected, &from) {
                let telemetry: Telemetry =
                    serde_json::from_slice(&self.recv_buffer[..size])?;
                return Ok(telemetry);
            }

            eprintln!(
                "a datagram came from {from} and not from {}, where the command \
                 went -- ignored",
                self.remote_addr
            );

            match deadline {
                None => continue,
                Some(deadline) => {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Err(TcsError::Io(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "nothing answered from where the command went",
                        )));
                    }
                    self.socket.set_read_timeout(Some(left))?;
                }
            }
        }
    }
}

impl Connection for UdpConnection {
    fn send(&mut self, command: &Command) -> TcsResult<()> {
        let data = serde_json::to_vec(command)?;
        self.socket.send_to(&data, self.remote_addr)?;
        Ok(())
    }

    fn receive(&mut self) -> TcsResult<Telemetry> {
        self.answer(None)
    }

    fn receive_timeout(&mut self, timeout: Duration) -> TcsResult<Telemetry> {
        self.socket.set_read_timeout(Some(timeout))?;
        let result = self.answer(Some(Instant::now() + timeout));
        self.socket.set_read_timeout(None)?;
        result
    }

    fn has_data(&self) -> TcsResult<bool> {
        self.socket.set_nonblocking(true)?;
        let mut buf = [0u8; 1];
        let result = match self.socket.peek(&mut buf) {
            Ok(_) => Ok(true),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(false),
            Err(e) => Err(TcsError::Io(e)),
        };
        self.socket.set_nonblocking(false)?;
        result
    }

    fn close(&mut self) -> TcsResult<()> {
        // UDP sockets don't need explicit closing
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_udp_connection_creation() {
        // This test requires network access, so we just verify the types compile
        let _: fn() -> TcsResult<UdpConnection> = || UdpConnection::new("127.0.0.1:0", "127.0.0.1:4000");
    }

    /// Only what comes from where the command went is an answer.
    ///
    /// A stranger's datagram at a bound port is not a response, however well
    /// formed: taking it for one would let whoever sent it say what the
    /// spacecraft is doing. The real answer follows it here, so this is about
    /// which datagram was taken rather than about the receive having failed.
    #[test]
    fn a_datagram_from_elsewhere_is_not_an_answer() {
        use tcslibgs::{CommandStatus, PingTelemetry, Telemetry};

        let spacecraft = UdpSocket::bind("127.0.0.1:0").expect("the far end");
        let at = spacecraft.local_addr().expect("where it is");
        let mut link =
            UdpConnection::new("127.0.0.1:0", &at.to_string()).expect("a connection");
        let mine = link.socket.local_addr().expect("where this end is");

        let answer = |sequence: u32| {
            serde_json::to_vec(&Telemetry::Ping(PingTelemetry::new(
                sequence,
                CommandStatus::Success,
            )))
            .expect("it serializes")
        };

        // A stranger answers first, then the spacecraft.
        let stranger = UdpSocket::bind("127.0.0.1:0").expect("another socket");
        stranger.send_to(&answer(1), mine).expect("it is sent");
        spacecraft.send_to(&answer(2), mine).expect("it is sent");

        match link
            .receive_timeout(Duration::from_secs(5))
            .expect("the spacecraft's answer")
        {
            Telemetry::Ping(tm) => assert_eq!(
                tm.header.sequence, 2,
                "the stranger's datagram was taken for an answer"
            ),
            other => panic!("expected PING telemetry, got {other:?}"),
        }
    }
}
