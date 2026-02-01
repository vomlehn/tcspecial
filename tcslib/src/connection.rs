//! Connection management for ground-to-space communication

use std::io::{self, Read, Write};
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;
use tcslibgs::{Command, TcsError, TcsResult, Telemetry};

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
}

impl Connection for UdpConnection {
    fn send(&mut self, command: &Command) -> TcsResult<()> {
eprintln!("UdpConnection::sendto {:?}", self.remote_addr);
        let data = serde_json::to_vec(command)?;
        self.socket.send_to(&data, self.remote_addr)?;
        Ok(())
    }

    fn receive(&mut self) -> TcsResult<Telemetry> {
        let (size, addr) = self.socket.recv_from(&mut self.recv_buffer)?;
eprintln!("UdpConnection: recv_from {:?}", addr);
        let telemetry: Telemetry = serde_json::from_slice(&self.recv_buffer[..size])?;
        Ok(telemetry)
    }

    fn receive_timeout(&mut self, timeout: Duration) -> TcsResult<Telemetry> {
        self.socket.set_read_timeout(Some(timeout))?;
        let result = self.receive();
eprintln!("UcpConnection: receive");
eprintln!("{}", std::backtrace::Backtrace::force_capture());
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

/// TCP-based connection to the spacecraft (for LEO/MEO or indirect links)
pub struct TcpConnection {
    stream: std::net::TcpStream,
    recv_buffer: Vec<u8>,
}

impl TcpConnection {
    /// Create a new TCP connection
    pub fn new(remote_addr: &str) -> TcsResult<Self> {
        let stream = std::net::TcpStream::connect(remote_addr)?;
        stream.set_nonblocking(false)?;

        Ok(Self {
            stream,
            recv_buffer: vec![0u8; 65535],
        })
    }

    /// Set the read timeout
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> TcsResult<()> {
        self.stream.set_read_timeout(timeout)?;
        Ok(())
    }

    /// Set the write timeout
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> TcsResult<()> {
        self.stream.set_write_timeout(timeout)?;
        Ok(())
    }
}

impl Connection for TcpConnection {
    fn send(&mut self, command: &Command) -> TcsResult<()> {
eprintln!("TcpConnection::send");
        let data = serde_json::to_vec(command)?;
        // Send length prefix (4 bytes big endian)
        let len_bytes = (data.len() as u32).to_be_bytes();
        self.stream.write_all(&len_bytes)?;
        self.stream.write_all(&data)?;
        self.stream.flush()?;
        Ok(())
    }

    fn receive(&mut self) -> TcsResult<Telemetry> {
        // Read length prefix
        let mut len_bytes = [0u8; 4];
        self.stream.read_exact(&mut len_bytes)?;
        let len = u32::from_be_bytes(len_bytes) as usize;

        if len > self.recv_buffer.len() {
            self.recv_buffer.resize(len, 0);
        }

        self.stream.read_exact(&mut self.recv_buffer[..len])?;
eprintln!("TcpConnection: receive");
eprintln!("{}", std::backtrace::Backtrace::force_capture());
        let telemetry: Telemetry = serde_json::from_slice(&self.recv_buffer[..len])?;
        Ok(telemetry)
    }

    fn receive_timeout(&mut self, timeout: Duration) -> TcsResult<Telemetry> {
        self.stream.set_read_timeout(Some(timeout))?;
        let result = self.receive();
eprintln!("TcpConnection: receive_timeout");
        self.stream.set_read_timeout(None)?;
        result
    }

    fn has_data(&self) -> TcsResult<bool> {
        // For TCP we'd need to use peek or platform-specific calls
        // This is a simplified implementation
        Ok(true) // Assume data might be available
    }

    fn close(&mut self) -> TcsResult<()> {
        self.stream.shutdown(std::net::Shutdown::Both)?;
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
}
