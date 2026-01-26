//! Conduit implementation for TCSpecial
//!
//! Conduits move data between endpoints in one direction.

use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use tcslibgs::{Statistics, TcsError, TcsResult};

use crate::config::constants::ENDPOINT_BUFFER_SIZE;
use crate::endpoint::{EndpointReadable, EndpointWritable, WaitResult};

/// Direction of data flow in a conduit
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConduitDirection {
    /// Ground to payload
    GroundToPayload,
    /// Payload to ground
    PayloadToGround,
}

/// Command for conduit control
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConduitCommand {
    /// Stop the conduit
    Stop,
    /// Get statistics
    GetStats,
}

/// Conduit thread state
pub struct Conduit {
    direction: ConduitDirection,
    running: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<TcsResult<Statistics>>>,
    cmd_pipe_write: RawFd,
}

impl Conduit {
    /// Create a new conduit
    pub fn new(
        direction: ConduitDirection,
        _reader: Box<dyn EndpointReadable + Send>,
        _writer: Box<dyn EndpointWritable + Send>,
        _cmd_pipe_read: RawFd,
        cmd_pipe_write: RawFd,
    ) -> Self {
        let running = Arc::new(AtomicBool::new(false));

        Self {
            direction,
            running,
            thread_handle: None,
            cmd_pipe_write,
        }
    }

    /// Start the conduit thread
    pub fn start(&mut self, mut reader: Box<dyn EndpointReadable + Send>, mut writer: Box<dyn EndpointWritable + Send>, cmd_fd: RawFd) -> TcsResult<()> {
        if self.running.load(Ordering::SeqCst) {
            return Err(TcsError::DataHandler("Conduit already running".to_string()));
        }

        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);

        let handle = thread::spawn(move || {
            let mut stats = Statistics::new();
            let mut buffer = vec![0u8; ENDPOINT_BUFFER_SIZE];

            while running.load(Ordering::SeqCst) {
                // Wait for I/O or command
                match reader.wait_for_event(cmd_fd, 1000) {
                    Ok(WaitResult::CommandPending) | Ok(WaitResult::Both) => {
                        // Read command byte from pipe
                        let mut cmd_buf = [0u8; 1];
                        unsafe {
                            libc::read(cmd_fd, cmd_buf.as_mut_ptr() as *mut libc::c_void, 1);
                        }
                        // Check if we should stop
                        if cmd_buf[0] == 0 {
                            break;
                        }
                    }
                    Ok(WaitResult::IoReady) => {
                        // Read from source
                        match reader.read(&mut buffer) {
                            Ok(0) => continue,
                            Ok(n) => {
                                stats.bytes_received += n as u64;
                                stats.reads_completed += 1;

                                // Write to destination
                                match writer.write(&buffer[..n]) {
                                    Ok(written) => {
                                        stats.bytes_sent += written as u64;
                                        stats.writes_completed += 1;
                                    }
                                    Err(_) => {
                                        stats.writes_failed += 1;
                                    }
                                }
                            }
                            Err(_) => {
                                stats.reads_failed += 1;
                            }
                        }
                    }
                    Ok(WaitResult::Timeout) => continue,
                    Ok(WaitResult::Error) | Err(_) => {
                        break;
                    }
                }
            }

            Ok(stats.with_timestamp())
        });

        self.thread_handle = Some(handle);
        Ok(())
    }

    /// Stop the conduit thread
    pub fn stop(&mut self) -> TcsResult<Statistics> {
        self.running.store(false, Ordering::SeqCst);

        // Send stop command through pipe
        let cmd = [0u8; 1];
        unsafe {
            libc::write(self.cmd_pipe_write, cmd.as_ptr() as *const libc::c_void, 1);
        }

        if let Some(handle) = self.thread_handle.take() {
            handle.join().map_err(|_| TcsError::DataHandler("Thread join failed".to_string()))?
        } else {
            Ok(Statistics::new())
        }
    }

    /// Check if the conduit is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get the conduit direction
    pub fn direction(&self) -> ConduitDirection {
        self.direction
    }
}

impl Drop for Conduit {
    fn drop(&mut self) {
        if self.is_running() {
            let _ = self.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conduit_direction() {
        assert_ne!(ConduitDirection::GroundToPayload, ConduitDirection::PayloadToGround);
    }
}
