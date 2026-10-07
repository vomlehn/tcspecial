//! Conduit implementation for TCSpecial
//!
//! Conduits move data between endpoints in one direction.

use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use tcslibgs::{DHSample, Statistics, TcsError, TcsResult};

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

/// What a data handler last moved, in each direction.
///
/// Shared between the conduit threads that write it and the command
/// interpreter that reads it, because a conduit's statistics reach the handler
/// only when the conduit stops, and a panel asks while data is still flowing.
///
/// Both senses are the handler's, as its statistics are: `received` is data
/// that came from the ground and `sent` is data that went to it. So one
/// conduit fills each -- the ground-to-payload conduit on its read, the
/// payload-to-ground conduit on its write -- rather than both filling both,
/// which would leave each field holding whichever direction moved last.
#[derive(Debug, Clone, Copy, Default)]
pub struct DHSamples {
    /// The last data sent to the ground.
    pub sent: DHSample,
    /// The last data received from the ground.
    pub received: DHSample,
}

/// Conduit thread state
pub struct Conduit {
    direction: ConduitDirection,
    running: Arc<AtomicBool>,
    /// Counted as data moves, and shared so it can be read while it moves.
    ///
    /// A conduit's thread used to keep these to itself and hand them back when
    /// it was joined, which meant a running handler reported nothing: every
    /// counter read zero until it was stopped, however much had gone through.
    ///
    /// Locked rather than tried, unlike the samples beside it: a sample missed
    /// under contention costs one stale line on a display, where a count
    /// missed is a count wrong from then on. The lock is held for a few
    /// additions and by a reader only long enough to copy.
    stats: Arc<Mutex<Statistics>>,
    thread_handle: Option<JoinHandle<()>>,
    cmd_pipe_write: RawFd,
}

/// Add to the counts.
///
/// Unlike [`record`], this waits for the lock: a count that went missing would
/// stay missing, and the hold is a few additions long.
fn count(stats: &Mutex<Statistics>, f: impl FnOnce(&mut Statistics)) {
    if let Ok(mut guard) = stats.lock() {
        f(&mut guard);
    }
}

/// Record a sample, unless someone is reading them.
///
/// Deliberately best effort: the samples exist for a display, and payload data
/// must not wait on a panel that happens to be asking. A lost sample costs one
/// stale line on a screen, where a blocked conduit costs data.
fn record(samples: &Mutex<DHSamples>, f: impl FnOnce(&mut DHSamples)) {
    if let Ok(mut guard) = samples.try_lock() {
        f(&mut guard);
    }
}

impl Conduit {
    /// Create a new conduit
    ///
    /// The endpoints belong to [`Conduit::start`], not here: a conduit that
    /// has not started owns nothing to read or write, and taking them twice
    /// once meant the caller had to hand over two sets.
    pub fn new(direction: ConduitDirection, cmd_pipe_write: RawFd) -> Self {
        let running = Arc::new(AtomicBool::new(false));

        Self {
            direction,
            running,
            stats: Arc::new(Mutex::new(Statistics::new())),
            thread_handle: None,
            cmd_pipe_write,
        }
    }

    /// Start the conduit thread
    pub fn start(
        &mut self,
        mut reader: Box<dyn EndpointReadable + Send>,
        mut writer: Box<dyn EndpointWritable + Send>,
        cmd_fd: RawFd,
        samples: Arc<Mutex<DHSamples>>,
    ) -> TcsResult<()> {
        if self.running.load(Ordering::SeqCst) {
            return Err(TcsError::DataHandler("Conduit already running".to_string()));
        }

        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);

        let direction = self.direction;
        let stats = self.stats.clone();

        let handle = thread::spawn(move || {
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
                                count(&stats, |s| {
                                    s.bytes_received += n as u64;
                                    s.reads_completed += 1;
                                });
                                // Only the ground-facing half of each conduit
                                // is a sample, so that the two lines a panel
                                // shows mean what the two byte counters beside
                                // them mean: received is data from the ground,
                                // sent is data to it. Recording both halves of
                                // both conduits leaves each field holding
                                // whichever direction moved last.
                                if direction == ConduitDirection::GroundToPayload {
                                    record(&samples, |s| s.received.record(&buffer[..n]));
                                }

                                // Write to destination
                                match writer.write(&buffer[..n]) {
                                    Ok(written) => {
                                        count(&stats, |s| {
                                            s.bytes_sent += written as u64;
                                            s.writes_completed += 1;
                                        });
                                        if direction == ConduitDirection::PayloadToGround {
                                            record(&samples, |s| {
                                                s.sent.record(&buffer[..written])
                                            });
                                        }
                                    }
                                    Err(_) => {
                                        count(&stats, |s| s.writes_failed += 1);
                                    }
                                }
                            }
                            Err(_) => {
                                count(&stats, |s| s.reads_failed += 1);
                            }
                        }
                    }
                    Ok(WaitResult::Timeout) => continue,
                    Ok(WaitResult::Error) | Err(_) => {
                        break;
                    }
                }
            }

        });

        self.thread_handle = Some(handle);
        Ok(())
    }

    /// What this conduit has counted so far.
    ///
    /// Readable while the conduit runs, which is the point of it: a handler is
    /// asked for its statistics far more often than it is stopped.
    pub fn statistics(&self) -> Statistics {
        self.stats.lock().map(|guard| *guard).unwrap_or_default()
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
            handle
                .join()
                .map_err(|_| TcsError::DataHandler("Thread join failed".to_string()))?;
        }

        // Read after the join, so nothing the thread was in the middle of
        // counting is left out of what the handler accumulates.
        Ok(self.statistics())
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
