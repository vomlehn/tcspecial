//! Data Handler implementation for tcspecial
//!
//! Data Handlers package two Relays, one in each direction. File descriptors
//! are shared between the relays for bidirectional communication.

use std::collections::BTreeMap;
use std::net::{TcpStream, UdpSocket, SocketAddr};
use std::os::fd::{AsRawFd, RawFd};
use std::sync::{Arc, Mutex};
use tcslibgs::{DHId, DHName, DHType, Statistics, TcsError, TcsResult};
use crate::relay::{Relay, RelayConfig, RelayDirection};
use crate::config::DHConfig;

/// State of a data handler
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DHState {
    /// Created but not yet activated
    Created,
    /// Active and relaying data
    Active,
    /// Stopped
    Stopped,
}

/// Data Handler - manages bidirectional communication between OC and payload
pub struct DataHandler {
    /// Data handler ID
    id: DHId,
    /// Data handler type
    dh_type: DHType,
    /// Name/configuration
    name: DHName,
    /// Current state
    state: DHState,
    /// Configuration
    config: DHConfig,
    /// Relay from ground to payload
    ground_to_payload: Option<Relay>,
    /// Relay from payload to ground
    payload_to_ground: Option<Relay>,
    /// OC socket (for network DHs)
    oc_socket: Option<UdpSocket>,
    /// Payload connection (could be various types)
    payload_fd: Option<RawFd>,
    /// Command pipe for notifying relays
    cmd_pipe: Option<(RawFd, RawFd)>, // (read, write)
    /// Statistics
    stats: Statistics,
}

impl DataHandler {
    /// Create a new data handler
    pub fn new(id: DHId, dh_type: DHType, name: DHName, config: DHConfig) -> Self {
        Self {
            id,
            dh_type,
            name,
            state: DHState::Created,
            config,
            ground_to_payload: None,
            payload_to_ground: None,
            oc_socket: None,
            payload_fd: None,
            cmd_pipe: None,
            stats: Statistics::new(),
        }
    }

    /// Get the DH ID
    pub fn id(&self) -> DHId {
        self.id
    }

    /// Get the DH type
    pub fn dh_type(&self) -> DHType {
        self.dh_type
    }

    /// Get the DH name
    pub fn name(&self) -> &DHName {
        &self.name
    }

    /// Get the current state
    pub fn state(&self) -> DHState {
        self.state
    }

    /// Initialize the data handler (allocate resources)
    pub fn initialize(&mut self) -> TcsResult<()> {
        // Create command pipe
        let mut pipe_fds = [0i32; 2];
        if unsafe { libc::pipe(pipe_fds.as_mut_ptr()) } < 0 {
            return Err(TcsError::resource_allocation("Failed to create command pipe"));
        }
        self.cmd_pipe = Some((pipe_fds[0], pipe_fds[1]));

        Ok(())
    }

    /// Activate the data handler (start relaying)
    pub fn activate(&mut self, oc_addr: SocketAddr) -> TcsResult<()> {
        if self.state != DHState::Created {
            return Err(TcsError::invalid_state("DH must be in Created state to activate"));
        }

        // Set up OC connection (UDP to OC)
        let oc_socket = UdpSocket::bind("0.0.0.0:0")?;
        oc_socket.connect(oc_addr)?;
        oc_socket.set_nonblocking(true)?;
        let oc_fd = oc_socket.as_raw_fd();
        self.oc_socket = Some(oc_socket);

        // Set up payload connection based on type
        let payload_fd = match self.dh_type {
            DHType::Network => self.connect_network_payload()?,
            DHType::Device => self.open_device_payload()?,
        };
        self.payload_fd = Some(payload_fd);

        // Create relays
        let cmd_write_fd = self.cmd_pipe.as_ref().map(|(_, w)| *w).unwrap_or(-1);
        let cmd_read_fd = self.cmd_pipe.as_ref().map(|(r, _)| *r).unwrap_or(-1);

        // Ground to Payload relay
        let g2p_config = RelayConfig {
            direction: RelayDirection::GroundToPayload,
            buffer_size: self.config.read_buffer_size,
            is_stream: self.config.is_stream,
        };
        let mut g2p_relay = Relay::new(g2p_config, oc_fd, payload_fd, cmd_write_fd);

        // Payload to Ground relay
        let p2g_config = RelayConfig {
            direction: RelayDirection::PayloadToGround,
            buffer_size: self.config.write_buffer_size,
            is_stream: self.config.is_stream,
        };
        let mut p2g_relay = Relay::new(p2g_config, payload_fd, oc_fd, cmd_write_fd);

        // Start relays
        g2p_relay.start()?;
        p2g_relay.start()?;

        self.ground_to_payload = Some(g2p_relay);
        self.payload_to_ground = Some(p2g_relay);
        self.state = DHState::Active;

        // Drop the read end of the pipe (we only write to it)
        if cmd_read_fd >= 0 {
            unsafe { libc::close(cmd_read_fd); }
        }

        Ok(())
    }

    /// Connect to a network payload
    fn connect_network_payload(&self) -> TcsResult<RawFd> {
        let addr_str = &self.name.0;

        // Parse address (format: host:port or host:port:protocol)
        let parts: Vec<&str> = addr_str.split(':').collect();
        if parts.len() < 2 {
            return Err(TcsError::configuration("Invalid network address format"));
        }

        let host = parts[0];
        let port: u16 = parts[1].parse()
            .map_err(|_| TcsError::configuration("Invalid port number"))?;

        let addr: SocketAddr = format!("{}:{}", host, port)
            .parse()
            .map_err(|_| TcsError::configuration("Invalid socket address"))?;

        // Determine protocol (default to UDP for datagrams, TCP for streams)
        let protocol = if parts.len() > 2 { parts[2] } else if self.config.is_stream { "tcp" } else { "udp" };

        match protocol.to_lowercase().as_str() {
            "tcp" => {
                let stream = TcpStream::connect(addr)?;
                stream.set_nonblocking(true)?;
                Ok(stream.as_raw_fd())
            }
            "udp" => {
                let socket = UdpSocket::bind("0.0.0.0:0")?;
                socket.connect(addr)?;
                socket.set_nonblocking(true)?;
                Ok(socket.as_raw_fd())
            }
            _ => Err(TcsError::configuration(format!("Unknown protocol: {}", protocol))),
        }
    }

    /// Open a device payload
    fn open_device_payload(&self) -> TcsResult<RawFd> {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let path = std::path::Path::new(&self.name.0);
        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| TcsError::configuration("Invalid device path"))?;

        let fd = unsafe {
            libc::open(c_path.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK)
        };

        if fd < 0 {
            return Err(TcsError::Io(std::io::Error::last_os_error()));
        }

        Ok(fd)
    }

    /// Stop the data handler
    pub fn stop(&mut self) -> TcsResult<()> {
        if self.state == DHState::Stopped {
            return Ok(());
        }

        // Stop relays
        if let Some(mut relay) = self.ground_to_payload.take() {
            relay.stop()?;
        }
        if let Some(mut relay) = self.payload_to_ground.take() {
            relay.stop()?;
        }

        // Close payload fd
        if let Some(fd) = self.payload_fd.take() {
            unsafe { libc::close(fd); }
        }

        // Close command pipe
        if let Some((_, write_fd)) = self.cmd_pipe.take() {
            unsafe { libc::close(write_fd); }
        }

        self.state = DHState::Stopped;
        Ok(())
    }

    /// Get statistics
    pub fn get_stats(&self) -> Statistics {
        let mut stats = self.stats.clone();

        // Merge stats from relays
        if let Some(relay) = &self.ground_to_payload {
            if let Ok(relay_stats) = relay.get_stats() {
                stats.bytes_sent += relay_stats.bytes_sent;
                stats.writes_completed += relay_stats.writes_completed;
                stats.writes_failed += relay_stats.writes_failed;
            }
        }
        if let Some(relay) = &self.payload_to_ground {
            if let Ok(relay_stats) = relay.get_stats() {
                stats.bytes_received += relay_stats.bytes_received;
                stats.reads_completed += relay_stats.reads_completed;
                stats.reads_failed += relay_stats.reads_failed;
            }
        }

        stats.with_timestamp()
    }
}

impl Drop for DataHandler {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Manager for multiple data handlers
pub struct DHManager {
    /// Map of DH ID to data handler
    handlers: Arc<Mutex<BTreeMap<DHId, DataHandler>>>,
    /// Maximum number of handlers
    max_handlers: usize,
}

impl DHManager {
    /// Create a new DH manager
    pub fn new(max_handlers: usize) -> Self {
        Self {
            handlers: Arc::new(Mutex::new(BTreeMap::new())),
            max_handlers,
        }
    }

    /// Create and register a new data handler
    pub fn create_dh(
        &self,
        id: DHId,
        dh_type: DHType,
        name: DHName,
        config: DHConfig,
    ) -> TcsResult<()> {
        let mut handlers = self.handlers.lock()
            .map_err(|_| TcsError::invalid_state("Lock poisoned"))?;

        if handlers.contains_key(&id) {
            return Err(TcsError::command(
                tcslibgs::telemetry::ErrorCode::DHAlreadyExists,
                format!("DH {} already exists", id),
            ));
        }

        if handlers.len() >= self.max_handlers {
            return Err(TcsError::resource_allocation("Maximum number of DHs reached"));
        }

        let mut dh = DataHandler::new(id, dh_type, name, config);
        dh.initialize()?;
        handlers.insert(id, dh);

        Ok(())
    }

    /// Activate a data handler
    pub fn activate_dh(&self, id: DHId, oc_addr: SocketAddr) -> TcsResult<()> {
        let mut handlers = self.handlers.lock()
            .map_err(|_| TcsError::invalid_state("Lock poisoned"))?;

        let dh = handlers.get_mut(&id)
            .ok_or_else(|| TcsError::command(
                tcslibgs::telemetry::ErrorCode::DHNotFound,
                format!("DH {} not found", id),
            ))?;

        dh.activate(oc_addr)
    }

    /// Stop a data handler
    pub fn stop_dh(&self, id: DHId) -> TcsResult<()> {
        let mut handlers = self.handlers.lock()
            .map_err(|_| TcsError::invalid_state("Lock poisoned"))?;

        let dh = handlers.get_mut(&id)
            .ok_or_else(|| TcsError::command(
                tcslibgs::telemetry::ErrorCode::DHNotFound,
                format!("DH {} not found", id),
            ))?;

        dh.stop()
    }

    /// Get statistics for a data handler
    pub fn get_dh_stats(&self, id: DHId) -> TcsResult<Statistics> {
        let handlers = self.handlers.lock()
            .map_err(|_| TcsError::invalid_state("Lock poisoned"))?;

        let dh = handlers.get(&id)
            .ok_or_else(|| TcsError::command(
                tcslibgs::telemetry::ErrorCode::DHNotFound,
                format!("DH {} not found", id),
            ))?;

        Ok(dh.get_stats())
    }

    /// Stop all data handlers
    pub fn stop_all(&self) -> TcsResult<()> {
        let mut handlers = self.handlers.lock()
            .map_err(|_| TcsError::invalid_state("Lock poisoned"))?;

        for (_, dh) in handlers.iter_mut() {
            let _ = dh.stop();
        }

        Ok(())
    }

    /// Get a clone of the handlers map for atomic status determination
    pub fn get_handlers(&self) -> Arc<Mutex<BTreeMap<DHId, DataHandler>>> {
        Arc::clone(&self.handlers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dh_state() {
        let dh = DataHandler::new(
            DHId(0),
            DHType::Network,
            DHName::new("localhost:5000"),
            DHConfig::default(),
        );
        assert_eq!(dh.state(), DHState::Created);
    }

    #[test]
    fn test_dh_manager_create() {
        let manager = DHManager::new(8);
        let result = manager.create_dh(
            DHId(0),
            DHType::Network,
            DHName::new("localhost:5000"),
            DHConfig::default(),
        );
        assert!(result.is_ok());

        // Creating duplicate should fail
        let result2 = manager.create_dh(
            DHId(0),
            DHType::Network,
            DHName::new("localhost:5001"),
            DHConfig::default(),
        );
        assert!(result2.is_err());
    }
}
