//! Connection management for ground-to-space communication

use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};
use tcslibgs::{
    came_from, Command, CommandPacker, HostPort, TcsError, TcsResult, Telemetry,
};

/// Connection to the spacecraft
pub trait Connection: Send {
    /// Sends a command to the spacecraft.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to pack and send.
    ///
    /// # Returns
    ///
    /// Nothing, once the command has gone.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if the datagram cannot be sent.
    fn send(&mut self, command: &Command) -> TcsResult<()>;

    /// Waits, without limit, for the spacecraft's next answer.
    ///
    /// # Returns
    ///
    /// The telemetry the spacecraft sent.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if the socket fails, or the error from reading an
    /// answer that is not one of the link's packets.
    fn receive(&mut self) -> TcsResult<Telemetry>;

    /// Waits for the spacecraft's next answer, for at most `timeout`.
    ///
    /// # Arguments
    ///
    /// * `timeout` - How long to wait for the answer in all.
    ///
    /// # Returns
    ///
    /// The telemetry the spacecraft sent.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if the socket fails or nothing answers in time, or
    /// the error from reading an answer that is not one of the link's
    /// packets.
    fn receive_timeout(&mut self, timeout: Duration) -> TcsResult<Telemetry>;

    /// Tells whether a datagram is waiting to be read, without reading it.
    ///
    /// # Returns
    ///
    /// `true` if a datagram is waiting.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if the socket cannot be looked at.
    fn has_data(&self) -> TcsResult<bool>;

    /// Closes the connection.
    ///
    /// # Returns
    ///
    /// Nothing, once it is closed.
    ///
    /// # Errors
    ///
    /// Whatever the connection's kind may fail with in closing; a UDP
    /// connection never fails.
    fn close(&mut self) -> TcsResult<()>;
}

/// UDP-based connection to the spacecraft
pub struct UdpConnection {
    socket: UdpSocket,
    remote_addr: SocketAddr,
    /// `remote_addr` as [`came_from`] reads it, worked out once.
    expected: HostPort,
    recv_buffer: Vec<u8>,
    /// What a command is packed into to be sent, kept from one to the next.
    packer: CommandPacker,
}

impl UdpConnection {
    /// Creates a UDP connection to the spacecraft.
    ///
    /// # Arguments
    ///
    /// * `local_addr`  - The `host:port` this end binds.
    /// * `remote_addr` - The `host:port` of the spacecraft's command
    ///                   socket, which commands go to and answers must
    ///                   come from.
    ///
    /// # Returns
    ///
    /// The connection, blocking on reads.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if `local_addr` cannot be bound, and
    /// `TcsError::Config` if `remote_addr` is not a socket address.
    pub fn new(local_addr: &str, remote_addr: &str) -> TcsResult<Self> {
        let socket = UdpSocket::bind(local_addr)?;
        let remote: SocketAddr = remote_addr.parse().map_err(|e| {
            TcsError::Config(format!("Invalid remote address: {}", e))
        })?;

        socket.set_nonblocking(false)?;

        Ok(Self {
            socket,
            remote_addr: remote,
            expected: HostPort {
                address: remote.ip().to_string(),
                port: remote.port(),
            },
            recv_buffer: vec![0u8; 65535],
            packer: CommandPacker::new(),
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
    pub fn set_write_timeout(
        &self,
        timeout: Option<Duration>,
    ) -> TcsResult<()> {
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
    ///
    /// # Arguments
    ///
    /// * `deadline` - When to give up waiting, or `None` to wait for ever.
    ///
    /// # Returns
    ///
    /// The answer.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if the socket fails or the deadline passes. An
    /// answer that is not one of this link's packets is an error saying
    /// so: it came from where the command went, so it is the answer, and
    /// it cannot be read.
    fn answer(&mut self, deadline: Option<Instant>) -> TcsResult<Telemetry> {
        loop {
            let (size, from) = self.socket.recv_from(&mut self.recv_buffer)?;
            if came_from(&self.expected, &from) {
                return Ok(Telemetry::read(&self.recv_buffer[..size])?);
            }

            eprintln!(
                "a datagram came from {from} and not from {}, where the \
                 command went -- ignored",
                self.remote_addr
            );

            match deadline {
                None => continue,
                Some(deadline) => {
                    let left =
                        deadline.saturating_duration_since(Instant::now());
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
        let packed = self.packer.pack(command);
        self.socket.send_to(packed, self.remote_addr)?;
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
        // This test requires network access, so we just verify the types
        // compile
        let _: fn() -> TcsResult<UdpConnection> =
            || UdpConnection::new("127.0.0.1:0", "127.0.0.1:4000");
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
        let mut link = UdpConnection::new("127.0.0.1:0", &at.to_string())
            .expect("a connection");
        let mine = link.socket.local_addr().expect("where this end is");

        let answer = |sequence: u32| {
            Telemetry::Ping(PingTelemetry::new(
                sequence,
                CommandStatus::Success,
            ))
            .to_bytes()
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
