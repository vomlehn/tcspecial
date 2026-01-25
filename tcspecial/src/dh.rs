//! Data Handler implementation for TCSpecial
//!
//! Data handlers relay data between the OC (Operations Center) and payloads.

use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tcslibgs::{DHConfig, DHId, DHName, Statistics, TcsError, TcsResult};

use crate::endpoint::{create_reader_endpoint, create_writer_endpoint, EndpointReadable, EndpointWritable};
use crate::relay::{Relay, RelayDirection};

/// Data handler state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DHState {
    /// Created but not activated
    Created,
    /// Active and relaying data
    Active,
    /// Stopped
    Stopped,
}

/// Data handler
pub struct DataHandler {
    id: DHId,
    name: DHName,
    config: DHConfig,
    state: DHState,
    ground_to_payload: Option<Relay>,
    payload_to_ground: Option<Relay>,
    stats: Statistics,
    running: Arc<AtomicBool>,
    cmd_pipe: Option<(RawFd, RawFd)>,
}

impl DataHandler {
    /// Create a new data handler
    pub fn new(config: DHConfig) -> TcsResult<Self> {
        // Create command pipe
        let mut pipe_fds = [0i32; 2];
        unsafe {
            if libc::pipe(pipe_fds.as_mut_ptr()) != 0 {
                return Err(TcsError::Io(std::io::Error::last_os_error()));
            }
        }

        Ok(Self {
            id: config.dh_id,
            name: config.name.clone(),
            config,
            state: DHState::Created,
            ground_to_payload: None,
            payload_to_ground: None,
            stats: Statistics::new(),
            running: Arc::new(AtomicBool::new(false)),
            cmd_pipe: Some((pipe_fds[0], pipe_fds[1])),
        })
    }

    /// Get the data handler ID
    pub fn id(&self) -> DHId {
        self.id
    }

    /// Get the data handler name
    pub fn name(&self) -> &DHName {
        &self.name
    }

    /// Get the current state
    pub fn state(&self) -> DHState {
        self.state
    }

    /// Get the statistics
    pub fn statistics(&self) -> Statistics {
        self.stats.clone().with_timestamp()
    }

    /// Start the data handler
    pub fn start(&mut self, oc_reader: Box<dyn EndpointReadable + Send>, oc_writer: Box<dyn EndpointWritable + Send>) -> TcsResult<()> {
        if self.state != DHState::Created {
            return Err(TcsError::DataHandler("Invalid state for start".to_string()));
        }

        let (cmd_read, cmd_write) = self.cmd_pipe.ok_or_else(|| TcsError::DataHandler("No command pipe".to_string()))?;

        // Create payload endpoint
        let payload_reader = create_reader_endpoint(&self.config.endpoint)?;
        let payload_writer = create_writer_endpoint(&self.config.endpoint)?;

        // Create relays
        let g2p_relay = Relay::new(
            RelayDirection::GroundToPayload,
            oc_reader,
            payload_writer,
            cmd_read,
            cmd_write,
        );

        let p2g_relay = Relay::new(
            RelayDirection::PayloadToGround,
            payload_reader,
            oc_writer,
            cmd_read,
            cmd_write,
        );

        // Note: In a full implementation, we would start the relays here
        // For now, we just update state
        self.state = DHState::Active;
        self.running.store(true, Ordering::SeqCst);

        self.ground_to_payload = Some(g2p_relay);
        self.payload_to_ground = Some(p2g_relay);

        Ok(())
    }

    /// Stop the data handler
    pub fn stop(&mut self) -> TcsResult<()> {
        if self.state != DHState::Active {
            // Idempotent - already stopped
            return Ok(());
        }

        self.running.store(false, Ordering::SeqCst);

        // Stop relays and collect statistics
        if let Some(mut relay) = self.ground_to_payload.take() {
            if let Ok(stats) = relay.stop() {
                self.stats.bytes_received += stats.bytes_received;
                self.stats.reads_completed += stats.reads_completed;
                self.stats.reads_failed += stats.reads_failed;
            }
        }

        if let Some(mut relay) = self.payload_to_ground.take() {
            if let Ok(stats) = relay.stop() {
                self.stats.bytes_sent += stats.bytes_sent;
                self.stats.writes_completed += stats.writes_completed;
                self.stats.writes_failed += stats.writes_failed;
            }
        }

        self.state = DHState::Stopped;

        // Close command pipe
        if let Some((read_fd, write_fd)) = self.cmd_pipe.take() {
            unsafe {
                libc::close(read_fd);
                libc::close(write_fd);
            }
        }

        Ok(())
    }

    /// Check if the data handler is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Drop for DataHandler {
    fn drop(&mut self) {
        if self.state == DHState::Active {
            let _ = self.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslibgs::{DeviceConfig, EndpointConfig, DHName};

    #[test]
    fn test_dh_creation() {
        let config = DHConfig {
            dh_id: DHId(0),
            name: DHName::new("Test"),
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/null".to_string(),
            }),
            packet_size: 64,
            packet_interval_ms: 100,
        };

        let dh = DataHandler::new(config);
        assert!(dh.is_ok());

        let dh = dh.unwrap();
        assert_eq!(dh.state(), DHState::Created);
    }
}
