//! Simulated payload implementation

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use rand::Rng;

/// Payload configuration
#[derive(Clone)]
pub struct PayloadConfig {
    pub id: u32,
    pub protocol: PayloadProtocol,
    pub address: String,
    pub port: u16,
    pub packet_size: Arc<AtomicU32>,
    pub packet_interval_ms: Arc<AtomicU32>,
}

/// Payload protocol type
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PayloadProtocol {
    Tcp,
    Udp,
    Device,
}

/// Statistics for a payload
#[derive(Default)]
pub struct PayloadStats {
    pub packets_sent: u64,
    pub packets_recv: u64,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
}

/// Simulated payload
pub struct SimulatedPayload {
    config: PayloadConfig,
    running: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
    stats: Arc<std::sync::Mutex<PayloadStats>>,
}

impl SimulatedPayload {
    /// Create a new simulated payload
    pub fn new(config: PayloadConfig) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
            stats: Arc::new(std::sync::Mutex::new(PayloadStats::default())),
        }
    }

    /// Start the payload simulation
    pub fn start(&mut self) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            return Err("Already running".to_string());
        }

        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();
        let config = self.config.clone();
        let stats = self.stats.clone();

        let handle = thread::spawn(move || {
            match config.protocol {
                PayloadProtocol::Tcp => run_tcp_payload(config, running, stats),
                PayloadProtocol::Udp => run_udp_payload(config, running, stats),
                PayloadProtocol::Device => run_device_payload(config, running, stats),
            }
        });

        self.thread_handle = Some(handle);
        Ok(())
    }

    /// Stop the payload simulation
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }

    /// Check if the payload is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get the current statistics
    pub fn stats(&self) -> PayloadStats {
        let guard = self.stats.lock().unwrap();
        PayloadStats {
            packets_sent: guard.packets_sent,
            packets_recv: guard.packets_recv,
            bytes_sent: guard.bytes_sent,
            bytes_recv: guard.bytes_recv,
        }
    }

    /// Update packet size
    pub fn set_packet_size(&self, size: u32) {
        self.config.packet_size.store(size, Ordering::SeqCst);
    }

    /// Update packet interval
    pub fn set_packet_interval(&self, interval_ms: u32) {
        self.config.packet_interval_ms.store(interval_ms, Ordering::SeqCst);
    }
}

impl Drop for SimulatedPayload {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Run TCP payload simulation
fn run_tcp_payload(config: PayloadConfig, running: Arc<AtomicBool>, stats: Arc<std::sync::Mutex<PayloadStats>>) {
    let addr = format!("{}:{}", config.address, config.port);

    // Try to bind as server
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind TCP listener: {}", e);
            return;
        }
    };

    listener.set_nonblocking(true).ok();

    let mut connection: Option<TcpStream> = None;
    let mut rng = rand::thread_rng();

    while running.load(Ordering::SeqCst) {
        // Accept new connections
        if connection.is_none() {
            if let Ok((stream, _)) = listener.accept() {
                stream.set_nonblocking(true).ok();
                connection = Some(stream);
            }
        }

        if let Some(ref mut stream) = connection {
            // Generate and send data
            let packet_size = config.packet_size.load(Ordering::SeqCst) as usize;
            let interval = config.packet_interval_ms.load(Ordering::SeqCst);

            if interval > 0 {
                let data: Vec<u8> = (0..packet_size).map(|_| rng.gen()).collect();
                if let Ok(n) = stream.write(&data) {
                    let mut guard = stats.lock().unwrap();
                    guard.packets_sent += 1;
                    guard.bytes_sent += n as u64;
                }
            }

            // Try to receive data
            let mut buf = vec![0u8; 4096];
            if let Ok(n) = stream.read(&mut buf) {
                if n > 0 {
                    let mut guard = stats.lock().unwrap();
                    guard.packets_recv += 1;
                    guard.bytes_recv += n as u64;
                }
            }
        }

        let interval = config.packet_interval_ms.load(Ordering::SeqCst);
        if interval > 0 {
            thread::sleep(Duration::from_millis(interval as u64));
        } else {
            thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Run UDP payload simulation
fn run_udp_payload(config: PayloadConfig, running: Arc<AtomicBool>, stats: Arc<std::sync::Mutex<PayloadStats>>) {
    let addr = format!("{}:{}", config.address, config.port);

    let socket = match UdpSocket::bind(&addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to bind UDP socket: {}", e);
            return;
        }
    };

    socket.set_nonblocking(true).ok();

    let mut rng = rand::thread_rng();
    let mut last_peer: Option<std::net::SocketAddr> = None;

    while running.load(Ordering::SeqCst) {
        // Try to receive data
        let mut buf = vec![0u8; 4096];
        if let Ok((n, peer)) = socket.recv_from(&mut buf) {
            last_peer = Some(peer);
            let mut guard = stats.lock().unwrap();
            guard.packets_recv += 1;
            guard.bytes_recv += n as u64;
        }

        // Generate and send data if we have a peer
        if let Some(peer) = last_peer {
            let packet_size = config.packet_size.load(Ordering::SeqCst) as usize;
            let interval = config.packet_interval_ms.load(Ordering::SeqCst);

            if interval > 0 {
                let data: Vec<u8> = (0..packet_size).map(|_| rng.gen()).collect();
                if let Ok(n) = socket.send_to(&data, peer) {
                    let mut guard = stats.lock().unwrap();
                    guard.packets_sent += 1;
                    guard.bytes_sent += n as u64;
                }
            }
        }

        let interval = config.packet_interval_ms.load(Ordering::SeqCst);
        if interval > 0 {
            thread::sleep(Duration::from_millis(interval as u64));
        } else {
            thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Run device payload simulation (simulates /dev/urandom-like behavior)
fn run_device_payload(config: PayloadConfig, running: Arc<AtomicBool>, stats: Arc<std::sync::Mutex<PayloadStats>>) {
    let mut rng = rand::thread_rng();

    while running.load(Ordering::SeqCst) {
        let packet_size = config.packet_size.load(Ordering::SeqCst) as usize;
        let interval = config.packet_interval_ms.load(Ordering::SeqCst);

        // Generate random data (simulating /dev/urandom)
        let _data: Vec<u8> = (0..packet_size).map(|_| rng.gen()).collect();
        {
            let mut guard = stats.lock().unwrap();
            guard.packets_sent += 1;
            guard.bytes_sent += packet_size as u64;
        }

        if interval > 0 {
            thread::sleep(Duration::from_millis(interval as u64));
        } else {
            thread::sleep(Duration::from_millis(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payload_config() {
        let config = PayloadConfig {
            id: 0,
            protocol: PayloadProtocol::Udp,
            address: "127.0.0.1".to_string(),
            port: 5000,
            packet_size: Arc::new(AtomicU32::new(12)),
            packet_interval_ms: Arc::new(AtomicU32::new(1000)),
        };
        assert_eq!(config.id, 0);
    }
}
