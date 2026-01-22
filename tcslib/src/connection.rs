//! Connection management for tcslib
//!
//! Handles the datagram connection between ground software and tcspecial.

use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;
use tcslibgs::{
    Command, Telemetry, TcsError, TcsResult,
    protocol::{ProtocolMessage, MAX_MESSAGE_SIZE},
};

/// Connection configuration
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Address of tcspecial
    pub remote_addr: SocketAddr,
    /// Local bind address
    pub local_addr: SocketAddr,
    /// Receive timeout
    pub recv_timeout: Option<Duration>,
    /// Send timeout
    pub send_timeout: Option<Duration>,
}

impl ConnectionConfig {
    pub fn new(remote_addr: SocketAddr, local_addr: SocketAddr) -> Self {
        Self {
            remote_addr,
            local_addr,
            recv_timeout: Some(Duration::from_secs(5)),
            send_timeout: Some(Duration::from_secs(5)),
        }
    }
}

/// Connection to tcspecial using UDP datagrams
pub struct Connection {
    socket: UdpSocket,
    remote_addr: SocketAddr,
    recv_buffer: Vec<u8>,
}

impl Connection {
    /// Create a new connection with the given configuration
    pub fn new(config: ConnectionConfig) -> TcsResult<Self> {
        let socket = UdpSocket::bind(config.local_addr)?;
        socket.connect(config.remote_addr)?;

        if let Some(timeout) = config.recv_timeout {
            socket.set_read_timeout(Some(timeout))?;
        }
        if let Some(timeout) = config.send_timeout {
            socket.set_write_timeout(Some(timeout))?;
        }

        Ok(Self {
            socket,
            remote_addr: config.remote_addr,
            recv_buffer: vec![0u8; MAX_MESSAGE_SIZE],
        })
    }

    /// Get the remote address
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }

    /// Send a command to tcspecial
    pub fn send_command(&self, cmd: Command) -> TcsResult<()> {
        let msg = ProtocolMessage::from_command(cmd)?;
        let bytes = msg.to_bytes()?;
        self.socket.send(&bytes)?;
        Ok(())
    }

    /// Receive a telemetry message from tcspecial
    pub fn recv_telemetry(&mut self) -> TcsResult<Telemetry> {
        let n = self.socket.recv(&mut self.recv_buffer)?;
        if n == 0 {
            return Err(TcsError::ConnectionClosed);
        }

        let msg = ProtocolMessage::from_bytes(&self.recv_buffer[..n])?;
        match msg.payload {
            tcslibgs::protocol::MessagePayload::Telemetry(tlm) => Ok(tlm),
            _ => Err(TcsError::protocol("Expected telemetry, got command")),
        }
    }

    /// Send a command and wait for a response
    pub fn send_and_recv(&mut self, cmd: Command) -> TcsResult<Telemetry> {
        self.send_command(cmd)?;
        self.recv_telemetry()
    }

    /// Set receive timeout
    pub fn set_recv_timeout(&self, timeout: Option<Duration>) -> TcsResult<()> {
        self.socket.set_read_timeout(timeout)?;
        Ok(())
    }

    /// Set send timeout
    pub fn set_send_timeout(&self, timeout: Option<Duration>) -> TcsResult<()> {
        self.socket.set_write_timeout(timeout)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};

    #[test]
    fn test_connection_config() {
        let remote = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 5000));
        let local = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 5001));
        let config = ConnectionConfig::new(remote, local);
        assert_eq!(config.remote_addr, remote);
        assert_eq!(config.local_addr, local);
    }
}
