//! Data Handler implementation for TCSpecial
//!
//! Data handlers conduit data between the OC (Operations Center) and payloads.

use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tcslibgs::{DHConfig, DHId, DHName, Statistics, TcsError, TcsResult};

use crate::endpoint::{create_endpoint_pair, EndpointReadable, EndpointWritable};
use crate::conduit::{Conduit, ConduitDirection, DHSamples};

/// Data handler state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DHState {
    /// Created but not activated
    Created,
    /// Active and conduiting data
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
    ground_to_payload: Option<Conduit>,
    payload_to_ground: Option<Conduit>,
    stats: Statistics,
    /// What this handler last sent and received, written by its conduits as
    /// data moves and read by the command interpreter when asked.
    samples: Arc<Mutex<DHSamples>>,
    running: Arc<AtomicBool>,
    /// One command pipe per conduit: the ground-to-payload conduit's, then
    /// the payload-to-ground conduit's.
    ///
    /// Not one pipe shared. `Conduit::stop` writes a single byte to wake its
    /// thread out of the poll it is sitting in, and two threads polling one
    /// read end would race for that byte: whichever read first would take it
    /// and the other would wait out its timeout instead.
    cmd_pipes: Option<[(RawFd, RawFd); 2]>,
}

/// Indices into a handler's command pipes.
const G2P: usize = 0;
const P2G: usize = 1;

/// Create a pipe, returning its read and write ends.
fn command_pipe() -> TcsResult<(RawFd, RawFd)> {
    let mut fds = [0i32; 2];
    unsafe {
        if libc::pipe(fds.as_mut_ptr()) != 0 {
            return Err(TcsError::Io(std::io::Error::last_os_error()));
        }
    }
    Ok((fds[0], fds[1]))
}

impl DataHandler {
    /// Create a new data handler
    pub fn new(config: DHConfig) -> TcsResult<Self> {
        // One per conduit; see cmd_pipes. The first is closed if the second
        // cannot be made, so a failure here leaks no descriptors.
        let g2p = command_pipe()?;
        let p2g = match command_pipe() {
            Ok(pipe) => pipe,
            Err(e) => {
                unsafe {
                    libc::close(g2p.0);
                    libc::close(g2p.1);
                }
                return Err(e);
            }
        };

        Ok(Self {
            id: config.dh_id,
            name: config.name.clone(),
            config,
            state: DHState::Created,
            ground_to_payload: None,
            payload_to_ground: None,
            stats: Statistics::new(),
            samples: Arc::new(Mutex::new(DHSamples::default())),
            running: Arc::new(AtomicBool::new(false)),
            cmd_pipes: Some([g2p, p2g]),
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
    ///
    /// What its conduits have counted, whether they are still running or have
    /// stopped. `stats` holds what stopped conduits left behind, and a running
    /// conduit is asked directly, so a handler reports what has moved rather
    /// than nothing until it is stopped.
    ///
    /// Which conduit contributes which half is the handler's own sense, the
    /// same one its samples use: received is data from the ground, which the
    /// ground-to-payload conduit reads, and sent is data to the ground, which
    /// the payload-to-ground conduit writes. The other half of each conduit is
    /// the payload side of the same bytes and is not counted twice.
    pub fn statistics(&self) -> Statistics {
        let mut stats = self.stats;

        if let Some(conduit) = &self.ground_to_payload {
            let live = conduit.statistics();
            stats.bytes_received += live.bytes_received;
            stats.reads_completed += live.reads_completed;
            stats.reads_failed += live.reads_failed;
        }

        if let Some(conduit) = &self.payload_to_ground {
            let live = conduit.statistics();
            stats.bytes_sent += live.bytes_sent;
            stats.writes_completed += live.writes_completed;
            stats.writes_failed += live.writes_failed;
        }

        stats.with_timestamp()
    }

    /// What this handler last sent and received.
    ///
    /// Both are empty until the handler is started and its conduits have
    /// moved something. A poisoned lock gives empty samples rather than an
    /// error, because a display is not worth failing a command over.
    pub fn samples(&self) -> DHSamples {
        self.samples
            .lock()
            .map(|guard| *guard)
            .unwrap_or_default()
    }

    /// The samples themselves, for a conduit to record into.
    pub fn samples_handle(&self) -> Arc<Mutex<DHSamples>> {
        self.samples.clone()
    }

    /// Start the data handler
    ///
    /// The OC endpoints are given rather than opened here: the command
    /// interpreter knows where the OC is, from `oc` in the handler's
    /// configuration, and opening them there keeps a handler that cannot be
    /// reached from being created at all.
    ///
    /// The payload endpoint is opened here, once, and both conduits share it:
    /// see [`create_endpoint_pair`].
    pub fn start(&mut self, oc_reader: Box<dyn EndpointReadable + Send>, oc_writer: Box<dyn EndpointWritable + Send>) -> TcsResult<()> {
        if self.state != DHState::Created {
            return Err(TcsError::DataHandler("Invalid state for start".to_string()));
        }

        let pipes = self
            .cmd_pipes
            .ok_or_else(|| TcsError::DataHandler("No command pipe".to_string()))?;

        // Create payload endpoint. Opened once: a network address cannot be
        // bound twice, so the two conduits share one socket.
        let (payload_reader, payload_writer) = create_endpoint_pair(&self.config.endpoint)?;

        // Create conduits
        let mut g2p_conduit =
            Conduit::new(ConduitDirection::GroundToPayload, pipes[G2P].1);
        let mut p2g_conduit =
            Conduit::new(ConduitDirection::PayloadToGround, pipes[P2G].1);

        // Start both, each on its own command pipe and both recording into
        // this handler's samples. If the second will not start, the first is
        // stopped again rather than left running in a handler that reports
        // itself as never started.
        g2p_conduit.start(
            oc_reader,
            payload_writer,
            pipes[G2P].0,
            self.samples.clone(),
        )?;

        if let Err(e) = p2g_conduit.start(
            payload_reader,
            oc_writer,
            pipes[P2G].0,
            self.samples.clone(),
        ) {
            let _ = g2p_conduit.stop();
            return Err(e);
        }

        self.state = DHState::Active;
        self.running.store(true, Ordering::SeqCst);

        self.ground_to_payload = Some(g2p_conduit);
        self.payload_to_ground = Some(p2g_conduit);

        Ok(())
    }

    /// Stop the data handler
    pub fn stop(&mut self) -> TcsResult<()> {
        if self.state != DHState::Active {
            // Idempotent - already stopped
            return Ok(());
        }

        self.running.store(false, Ordering::SeqCst);

        // Stop conduits and collect statistics
        if let Some(mut conduit) = self.ground_to_payload.take() {
            if let Ok(stats) = conduit.stop() {
                self.stats.bytes_received += stats.bytes_received;
                self.stats.reads_completed += stats.reads_completed;
                self.stats.reads_failed += stats.reads_failed;
            }
        }

        if let Some(mut conduit) = self.payload_to_ground.take() {
            if let Ok(stats) = conduit.stop() {
                self.stats.bytes_sent += stats.bytes_sent;
                self.stats.writes_completed += stats.writes_completed;
                self.stats.writes_failed += stats.writes_failed;
            }
        }

        self.state = DHState::Stopped;

        // Close command pipes
        if let Some(pipes) = self.cmd_pipes.take() {
            for (read_fd, write_fd) in pipes {
                unsafe {
                    libc::close(read_fd);
                    libc::close(write_fd);
                }
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

    /// A running handler reports what has moved, not nothing.
    ///
    /// The counts used to reach a handler only when its conduits were joined,
    /// so every one read zero however much had gone through. This drives a
    /// handler over the loopback and reads the counts while it runs.
    #[test]
    fn a_running_handler_reports_what_has_moved() {
        use std::net::UdpSocket;
        use std::time::Duration;
        use tcslibgs::{NetworkConfig, NetworkProtocol};

        // Port 0 for both sides, so this cannot collide with another test or
        // with a running tcspecial.
        let probe = UdpSocket::bind("127.0.0.1:0").unwrap();
        let oc_addr = probe.local_addr().unwrap();
        drop(probe);

        let config = DHConfig {
            dh_id: DHId(0),
            name: DHName::new("Live"),
            // /dev/urandom is always readable, so the payload-to-ground
            // conduit has something to move as soon as it starts.
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/urandom".to_string(),
            }),
            packet_size: 64,
            oc: Some(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: oc_addr.ip().to_string(),
                port: oc_addr.port(),
            }),
        };

        let mut dh = DataHandler::new(config.clone()).unwrap();
        let (oc_reader, oc_writer) =
            crate::endpoint::create_endpoint_pair(&EndpointConfig::Network(
                config.oc.clone().unwrap(),
            ))
            .unwrap();
        dh.start(oc_reader, oc_writer).unwrap();
        assert_eq!(dh.state(), DHState::Active);

        // Speak first: until the ground does, the handler has nowhere to send
        // and every write towards it fails.
        let ground = UdpSocket::bind("127.0.0.1:0").unwrap();
        ground
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        ground.send_to(b"hello", oc_addr).unwrap();

        let mut buffer = [0u8; 8192];
        ground
            .recv_from(&mut buffer)
            .expect("the handler should send payload data once spoken to");

        let running = dh.statistics();
        assert_ne!(
            running.bytes_sent, 0,
            "a running handler reported no bytes sent"
        );
        assert_eq!(
            running.bytes_received, 5,
            "the five bytes the ground sent should be counted"
        );

        // Stopping folds the conduits' counts into the handler's own, so what
        // it reports cannot go backwards.
        dh.stop().unwrap();
        let stopped = dh.statistics();
        assert!(
            stopped.bytes_sent >= running.bytes_sent,
            "stopping lost counts: {} then {}",
            running.bytes_sent,
            stopped.bytes_sent
        );
        assert_eq!(stopped.bytes_received, running.bytes_received);
    }

    #[test]
    fn test_dh_creation() {
        let config = DHConfig {
            dh_id: DHId(0),
            name: DHName::new("Test"),
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/null".to_string(),
            }),
            packet_size: 64,
            oc: None,
        };

        let dh = DataHandler::new(config);
        assert!(dh.is_ok());

        let dh = dh.unwrap();
        assert_eq!(dh.state(), DHState::Created);
    }
}
