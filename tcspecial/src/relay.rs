//! Relay implementation for tcspecial
//!
//! Relays contain two Endpoints, an EndpointReadable and an EndpointWritable.
//! Data flows in just one direction, from the EndpointReadable to the EndpointWriteable.

use std::os::fd::RawFd;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::{self, JoinHandle};
use tcslibgs::{TcsError, TcsResult, Statistics};

/// Commands that can be sent to a relay
#[derive(Debug, Clone)]
pub enum RelayCommand {
    /// Stop the relay
    Stop,
    /// Get statistics
    GetStats,
}

/// Responses from a relay
#[derive(Debug)]
pub enum RelayResponse {
    /// Relay stopped
    Stopped,
    /// Statistics response
    Stats(Statistics),
    /// Error occurred
    Error(String),
}

/// Direction of data flow
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayDirection {
    /// Data flows from ground to payload
    GroundToPayload,
    /// Data flows from payload to ground
    PayloadToGround,
}

/// Relay configuration
#[derive(Debug, Clone)]
pub struct RelayConfig {
    /// Direction of data flow
    pub direction: RelayDirection,
    /// Buffer size
    pub buffer_size: usize,
    /// Whether the source is a stream
    pub is_stream: bool,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            direction: RelayDirection::GroundToPayload,
            buffer_size: 4096,
            is_stream: false,
        }
    }
}

/// A relay that transfers data in one direction
pub struct Relay {
    /// Configuration
    config: RelayConfig,
    /// Read file descriptor
    read_fd: RawFd,
    /// Write file descriptor
    write_fd: RawFd,
    /// Command pipe write end (to notify the relay thread)
    cmd_write_fd: RawFd,
    /// Statistics
    stats: Statistics,
    /// Thread handle
    thread_handle: Option<JoinHandle<()>>,
    /// Command sender
    cmd_sender: Option<Sender<RelayCommand>>,
    /// Response receiver
    resp_receiver: Option<Receiver<RelayResponse>>,
}

impl Relay {
    /// Create a new relay
    pub fn new(
        config: RelayConfig,
        read_fd: RawFd,
        write_fd: RawFd,
        cmd_write_fd: RawFd,
    ) -> Self {
        Self {
            config,
            read_fd,
            write_fd,
            cmd_write_fd,
            stats: Statistics::new(),
            thread_handle: None,
            cmd_sender: None,
            resp_receiver: None,
        }
    }

    /// Start the relay thread
    pub fn start(&mut self) -> TcsResult<()> {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        let (resp_tx, resp_rx) = std::sync::mpsc::channel();

        self.cmd_sender = Some(cmd_tx);
        self.resp_receiver = Some(resp_rx);

        let read_fd = self.read_fd;
        let write_fd = self.write_fd;
        let buffer_size = self.config.buffer_size;
        let _cmd_write_fd = self.cmd_write_fd;

        let handle = thread::spawn(move || {
            Self::relay_loop(read_fd, write_fd, buffer_size, cmd_rx, resp_tx);
        });

        self.thread_handle = Some(handle);
        Ok(())
    }

    /// Main relay loop (runs in separate thread)
    fn relay_loop(
        _read_fd: RawFd,
        _write_fd: RawFd,
        _buffer_size: usize,
        cmd_rx: Receiver<RelayCommand>,
        resp_tx: Sender<RelayResponse>,
    ) {
        let mut stats = Statistics::new();

        loop {
            // Check for commands (non-blocking)
            match cmd_rx.try_recv() {
                Ok(RelayCommand::Stop) => {
                    let _ = resp_tx.send(RelayResponse::Stopped);
                    return;
                }
                Ok(RelayCommand::GetStats) => {
                    let _ = resp_tx.send(RelayResponse::Stats(stats.clone()));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }

            // TODO: Implement actual I/O relay using poll/epoll
            // For now, just sleep to avoid busy loop
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Update stats (placeholder)
            stats.reads_completed += 0;
            stats.writes_completed += 0;
        }
    }

    /// Stop the relay
    pub fn stop(&mut self) -> TcsResult<()> {
        if let Some(sender) = &self.cmd_sender {
            let _ = sender.send(RelayCommand::Stop);
        }

        // Write to cmd pipe to wake up the thread
        if self.cmd_write_fd >= 0 {
            let buf = [0u8; 1];
            unsafe {
                libc::write(self.cmd_write_fd, buf.as_ptr() as *const libc::c_void, 1);
            }
        }

        if let Some(handle) = self.thread_handle.take() {
            handle.join().map_err(|_| TcsError::invalid_state("Thread join failed"))?;
        }

        Ok(())
    }

    /// Get statistics
    pub fn get_stats(&self) -> TcsResult<Statistics> {
        if let Some(sender) = &self.cmd_sender {
            sender.send(RelayCommand::GetStats)
                .map_err(|_| TcsError::invalid_state("Failed to send command"))?;
        }

        if let Some(receiver) = &self.resp_receiver {
            match receiver.recv_timeout(std::time::Duration::from_secs(1)) {
                Ok(RelayResponse::Stats(stats)) => Ok(stats),
                Ok(_) => Err(TcsError::protocol("Unexpected response")),
                Err(_) => Ok(self.stats.clone()), // Return cached stats on timeout
            }
        } else {
            Ok(self.stats.clone())
        }
    }

    /// Get the direction
    pub fn direction(&self) -> RelayDirection {
        self.config.direction
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_config_default() {
        let config = RelayConfig::default();
        assert_eq!(config.direction, RelayDirection::GroundToPayload);
        assert_eq!(config.buffer_size, 4096);
    }

    #[test]
    fn test_relay_direction() {
        assert_ne!(RelayDirection::GroundToPayload, RelayDirection::PayloadToGround);
    }
}
