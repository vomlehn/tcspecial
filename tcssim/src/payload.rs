//! Payload simulation for tcssim

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use log::{info, warn, debug, error};
use rand::Rng;
use tcslibgs::{TcsError, TcsResult};

/// Type of payload
#[derive(Debug, Clone, Copy)]
pub enum PayloadType {
    /// TCP server
    TcpServer,
    /// UDP server
    UdpServer,
}

/// Payload configuration
#[derive(Debug, Clone)]
pub struct PayloadConfig {
    /// Payload ID (matches DH #)
    pub id: u32,
    /// Type of payload
    pub payload_type: PayloadType,
    /// Address to listen on
    pub address: String,
    /// Packet size in bytes
    pub packet_size: usize,
    /// Interval between packets
    pub interval: Duration,
}

/// Payload statistics for GUI display
pub struct PayloadStats {
    pub sent: u64,
    pub received: u64,
    pub last_sent: String,
    pub last_received: String,
    pub status: String,
    pub connected: bool,
}

/// Shared stats structure used in main.rs
pub struct SharedStats {
    pub sent: u64,
    pub received: u64,
    pub last_sent: String,
    pub last_received: String,
    pub status: String,
    pub connected: bool,
}

/// Simulated payload
pub struct Payload {
    config: PayloadConfig,
    sequence: u32,
    stats: Arc<Mutex<SharedStats>>,
}

impl Payload {
    pub fn new(config: PayloadConfig, stats: Arc<Mutex<SharedStats>>) -> Self {
        Self {
            config,
            sequence: 0,
            stats,
        }
    }

    /// Run the payload
    pub fn run(&mut self, running: Arc<AtomicBool>) -> TcsResult<()> {
        info!("Starting payload {} on {}", self.config.id, self.config.address);

        match self.config.payload_type {
            PayloadType::TcpServer => self.run_tcp_server(running),
            PayloadType::UdpServer => self.run_udp_server(running),
        }
    }

    fn update_status(&self, status: &str) {
        if let Ok(mut s) = self.stats.lock() {
            s.status = status.to_string();
        }
    }

    fn update_connected(&self, connected: bool) {
        if let Ok(mut s) = self.stats.lock() {
            s.connected = connected;
        }
    }

    fn record_sent(&self, data: &[u8]) {
        if let Ok(mut s) = self.stats.lock() {
            s.sent += 1;
            s.last_sent = format!("{} bytes: {:02X?}", data.len(), &data[..data.len().min(8)]);
        }
    }

    fn record_received(&self, data: &[u8], n: usize) {
        if let Ok(mut s) = self.stats.lock() {
            s.received += 1;
            s.last_received = format!("{} bytes: {:02X?}", n, &data[..n.min(8)]);
        }
    }

    /// Run as TCP server
    fn run_tcp_server(&mut self, running: Arc<AtomicBool>) -> TcsResult<()> {
        let addr: SocketAddr = self.config.address.parse()
            .map_err(|_| TcsError::configuration("Invalid address"))?;

        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;

        info!("Payload {} listening on TCP {}", self.config.id, addr);
        self.update_status("Listening");

        while running.load(Ordering::SeqCst) {
            // Accept new connections
            match listener.accept() {
                Ok((stream, peer_addr)) => {
                    info!("Payload {} accepted connection from {}", self.config.id, peer_addr);
                    self.update_status(&format!("Connected: {}", peer_addr));
                    self.update_connected(true);
                    self.handle_tcp_connection(stream, running.clone())?;
                    self.update_connected(false);
                    self.update_status("Listening");
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    warn!("Payload {} accept error: {}", self.config.id, e);
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }

        Ok(())
    }

    /// Handle a TCP connection
    fn handle_tcp_connection(&mut self, mut stream: TcpStream, running: Arc<AtomicBool>) -> TcsResult<()> {
        stream.set_nonblocking(true)?;
        stream.set_nodelay(true)?;

        let mut recv_buf = vec![0u8; 1024];
        let mut last_send = Instant::now();

        while running.load(Ordering::SeqCst) {
            // Try to receive data
            match stream.read(&mut recv_buf) {
                Ok(0) => {
                    info!("Payload {} connection closed", self.config.id);
                    break;
                }
                Ok(n) => {
                    debug!("Payload {} received {} bytes", self.config.id, n);
                    self.record_received(&recv_buf, n);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => {
                    warn!("Payload {} read error: {}", self.config.id, e);
                    break;
                }
            }

            // Send data at configured interval
            if last_send.elapsed() >= self.config.interval {
                let packet = self.generate_packet();
                match stream.write_all(&packet) {
                    Ok(()) => {
                        debug!("Payload {} sent {} bytes", self.config.id, packet.len());
                        self.record_sent(&packet);
                        self.sequence += 1;
                    }
                    Err(e) => {
                        warn!("Payload {} write error: {}", self.config.id, e);
                        break;
                    }
                }
                last_send = Instant::now();
            }

            std::thread::sleep(Duration::from_millis(10));
        }

        Ok(())
    }

    /// Run as UDP server
    fn run_udp_server(&mut self, running: Arc<AtomicBool>) -> TcsResult<()> {
        let addr: SocketAddr = self.config.address.parse()
            .map_err(|_| TcsError::configuration("Invalid address"))?;

        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;

        info!("Payload {} listening on UDP {}", self.config.id, addr);
        self.update_status("Listening");

        let mut recv_buf = vec![0u8; 1024];
        let mut last_send = Instant::now();
        let mut peer_addr: Option<SocketAddr> = None;

        while running.load(Ordering::SeqCst) {
            // Try to receive data
            match socket.recv_from(&mut recv_buf) {
                Ok((n, from_addr)) => {
                    debug!("Payload {} received {} bytes from {}", self.config.id, n, from_addr);
                    self.record_received(&recv_buf, n);
                    if peer_addr.is_none() {
                        self.update_status(&format!("Connected: {}", from_addr));
                        self.update_connected(true);
                    }
                    peer_addr = Some(from_addr);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => {
                    warn!("Payload {} recv error: {}", self.config.id, e);
                }
            }

            // Send data at configured interval if we have a peer
            if let Some(addr) = peer_addr {
                if last_send.elapsed() >= self.config.interval {
                    let packet = self.generate_packet();
                    match socket.send_to(&packet, addr) {
                        Ok(n) => {
                            debug!("Payload {} sent {} bytes to {}", self.config.id, n, addr);
                            self.record_sent(&packet);
                            self.sequence += 1;
                        }
                        Err(e) => {
                            warn!("Payload {} send error: {}", self.config.id, e);
                        }
                    }
                    last_send = Instant::now();
                }
            }

            std::thread::sleep(Duration::from_millis(10));
        }

        Ok(())
    }

    /// Generate a packet with random data
    fn generate_packet(&self) -> Vec<u8> {
        let mut rng = rand::thread_rng();
        let mut packet = Vec::with_capacity(self.config.packet_size);

        // First 4 bytes are sequence number (big-endian)
        packet.extend_from_slice(&self.sequence.to_be_bytes());

        // Remaining bytes are random data
        for _ in 4..self.config.packet_size {
            packet.push(rng.gen());
        }

        // Ensure packet is exactly the configured size
        packet.resize(self.config.packet_size, 0);

        packet
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dummy_stats() -> Arc<Mutex<SharedStats>> {
        Arc::new(Mutex::new(SharedStats {
            sent: 0,
            received: 0,
            last_sent: String::new(),
            last_received: String::new(),
            status: String::new(),
            connected: false,
        }))
    }

    #[test]
    fn test_payload_config() {
        let config = PayloadConfig {
            id: 0,
            payload_type: PayloadType::TcpServer,
            address: "127.0.0.1:5000".to_string(),
            packet_size: 12,
            interval: Duration::from_secs(1),
        };
        assert_eq!(config.id, 0);
        assert_eq!(config.packet_size, 12);
    }

    #[test]
    fn test_packet_generation() {
        let config = PayloadConfig {
            id: 0,
            payload_type: PayloadType::UdpServer,
            address: "127.0.0.1:5000".to_string(),
            packet_size: 12,
            interval: Duration::from_secs(1),
        };
        let payload = Payload::new(config, make_dummy_stats());
        let packet = payload.generate_packet();
        assert_eq!(packet.len(), 12);
    }
}
