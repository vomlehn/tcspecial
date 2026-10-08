//! Data Handler implementation for TCSpecial
//!
//! Data handlers conduit data between the OC (Operations Center) and payloads.

use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use log::info;
use tcslibgs::{DHConfig, DHId, DHName, Statistics, TcsError, TcsResult};

use crate::endpoint::{connect_endpoint_pair, EndpointReadable, EndpointWritable};
use crate::conduit::{Conduit, ConduitDirection, DHSamples, Polling};

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
            // And the triggers, which nothing else here counts. A handler's
            // sent and received are the ground's data in each direction, so
            // this conduit's writes -- the payload side of what it read --
            // are deliberately left out of them; a trigger is a write of
            // this conduit's own, with no read behind it, and would be
            // invisible without a count of its own.
            stats.triggers_sent += live.triggers_sent;
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
    /// see [`connect_endpoint_pair`].
    pub fn start(&mut self, oc_reader: Box<dyn EndpointReadable + Send>, oc_writer: Box<dyn EndpointWritable + Send>) -> TcsResult<()> {
        if self.state != DHState::Created {
            return Err(TcsError::DataHandler("Invalid state for start".to_string()));
        }

        let pipes = self
            .cmd_pipes
            .ok_or_else(|| TcsError::DataHandler("No command pipe".to_string()))?;

        // Which end of the payload link waits at the configured address
        // depends on the protocol and on which end can speak first; see
        // connect_endpoint_pair. Opened once either way, because a socket
        // cannot be opened twice and the two conduits share it.
        let (payload_reader, payload_writer) = connect_endpoint_pair(&self.config.endpoint)?;

        // Create conduits
        let mut g2p_conduit =
            Conduit::new(ConduitDirection::GroundToPayload, pipes[G2P].1);
        let mut p2g_conduit =
            Conduit::new(ConduitDirection::PayloadToGround, pipes[P2G].1);

        // Start both, each on its own command pipe and both recording into
        // this handler's samples. If the second will not start, the first is
        // stopped again rather than left running in a handler that reports
        // itself as never started.
        // What this handler has to send its payload to get anything back,
        // for a payload of the kind that answers requests rather than sending
        // on its own. It goes to the conduit that writes the payload, which is
        // the one that already has somewhere to put it.
        let polling = self.config.mode.polling().map(|(trigger, interval_ms)| Polling {
            trigger: trigger.to_vec(),
            interval: Duration::from_millis(interval_ms as u64),
        });
        if let Some(polling) = &polling {
            info!(
                "{}: polling every {:?} with {:?}",
                self.config.name.0, polling.interval, polling.trigger
            );
        }

        g2p_conduit.start(
            oc_reader,
            payload_writer,
            pipes[G2P].0,
            self.samples.clone(),
            polling,
        )?;

        if let Err(e) = p2g_conduit.start(
            payload_reader,
            oc_writer,
            pipes[P2G].0,
            self.samples.clone(),
            // Nothing: a trigger goes to the payload, and this is the conduit
            // that writes the ground.
            None,
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

    /// A UDP payload that speaks first, and a handler waiting where it
    /// speaks to, both come up and data moves both ways.
    ///
    /// They used to both bind the payload's address, so whichever started
    /// second got AddrInUse and no data moved through a network handler at
    /// all. Then the handler reached out to a payload that listened, which
    /// works for a stream and not for a datagram: a UDP connect sends
    /// nothing, so a listening payload heard nothing and -- producing data
    /// because it is running rather than because it was asked -- had nowhere
    /// to send it. Both ends sat there with nothing moving. So over UDP the
    /// handler binds the address and the payload reaches out to it.
    ///
    /// This drives the whole loop, because that is the only way to see that
    /// both ends really found each other: the payload sends unprompted and it
    /// reaches the ground, and then the ground sends and it reaches the
    /// payload at the address the handler learnt from that first packet.
    #[test]
    fn a_payload_that_speaks_first_and_a_handler_waiting_for_it_both_come_up() {
        use std::net::UdpSocket;
        use std::time::{Duration, Instant};
        use tcslibgs::{NetworkConfig, NetworkProtocol};

        // A free address for the handler's payload side, which it binds. Port
        // 0 and then let go, so this cannot collide with a real one or
        // another test.
        let probe = UdpSocket::bind("127.0.0.1:0").expect("a free payload port");
        let payload_side = probe.local_addr().expect("its address");
        drop(probe);

        let oc = UdpSocket::bind("127.0.0.1:0").expect("a free OC port");
        let oc_addr = oc.local_addr().expect("its address");
        drop(oc);

        let config = DHConfig {
            dh_id: DHId(1),
            name: DHName::new("Net"),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: payload_side.ip().to_string(),
                port: payload_side.port(),
            }),
            packet_size: 64,
            oc: Some(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: oc_addr.ip().to_string(),
                port: oc_addr.port(),
            }),
                    mode: Default::default(),
        };

        let mut dh = DataHandler::new(config.clone()).unwrap();
        let (oc_reader, oc_writer) = crate::endpoint::bind_endpoint_pair(
            &EndpointConfig::Network(config.oc.clone().unwrap()),
        )
        .unwrap();
        dh.start(oc_reader, oc_writer)
            .expect("the handler starts and waits at its payload's address");

        let ground = UdpSocket::bind("127.0.0.1:0").unwrap();
        ground
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();

        // Standing in for tcssim: the payload has a port of its own and
        // reaches the handler at the address the handler is waiting on.
        let payload = UdpSocket::bind("127.0.0.1:0").expect("the payload binds a local port");
        payload
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        payload.connect(payload_side).expect("it reaches the handler");

        // The handler knows neither address until it is spoken to: no
        // configuration says where the ground is, and the payload's port is
        // its own. So the ground speaks once into the dark -- that datagram
        // has nowhere to go on to and is dropped, which the handler counts as
        // a failed write -- and the handler now knows where the ground is.
        ground.send_to(b"from the ground", oc_addr).unwrap();

        // The payload sends because it is running, not because it was
        // asked, and that is what reaches the ground. Said again until it
        // lands, as a running payload would: the handler drops payload data
        // until it has read the ground's first datagram, and the two conduits
        // are threads of their own, so which of the two gets there first is
        // not fixed.
        ground
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let mut buffer = [0u8; 4096];
        let deadline = Instant::now() + Duration::from_secs(5);
        let arrived = loop {
            payload.send(b"from the payload").expect("it sends unprompted");

            if let Ok(arrived) = ground.recv_from(&mut buffer) {
                break Some(arrived);
            }
            if Instant::now() >= deadline {
                break None;
            }
        };

        let (n, from) = arrived.expect("the payload's data should reach the ground");
        assert_eq!(&buffer[..n], b"from the payload");
        assert_eq!(from, oc_addr, "it came from the handler's OC address");

        // And now back the other way, to the address the handler learnt from
        // that packet.
        ground.send_to(b"from the ground", oc_addr).unwrap();
        let n = payload
            .recv(&mut buffer)
            .expect("the ground's data should reach the payload");
        assert_eq!(&buffer[..n], b"from the ground");

        // At least one message each way. More, where the payload had to say
        // itself again.
        let stats = dh.statistics();
        assert!(
            stats.bytes_received >= b"from the ground".len() as u64,
            "the ground's data was not read: {} bytes",
            stats.bytes_received
        );
        assert!(
            stats.bytes_sent >= b"from the payload".len() as u64,
            "the payload's data did not go to the ground: {} bytes",
            stats.bytes_sent
        );

        dh.stop().unwrap();
    }

    /// A triggered handler asks its payload, at the interval its
    /// configuration gives, and carries the answer to the ground.
    ///
    /// Nothing in tcspecial used to send a payload anything of its own
    /// accord: both conduits only ever copied, one from the OC and one from
    /// the payload, so a payload that answers requests could be described in
    /// a file and would never be asked. This drives the whole loop, because
    /// the asking is only worth anything if the answer gets home.
    #[test]
    fn a_triggered_handler_asks_its_payload_and_carries_the_answer() {
        use std::io::{Read, Write};
        use std::net::{TcpListener, UdpSocket};
        use std::time::{Duration, Instant};
        use tcslibgs::{DHConfig, DHMode, DHName, EndpointConfig, NetworkConfig, NetworkProtocol};

        // A TCP payload, because a triggered payload has to be reachable
        // before it has spoken: the handler connects to it, so it knows where
        // to send from the start.
        let payload = TcpListener::bind("127.0.0.1:0").expect("the payload listens");
        let payload_addr = payload.local_addr().expect("its address");

        let oc = UdpSocket::bind("127.0.0.1:0").expect("a free OC port");
        let oc_addr = oc.local_addr().expect("its address");
        drop(oc);

        let config = DHConfig {
            dh_id: DHId(4),
            name: DHName::new("Polled"),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Tcp,
                address: payload_addr.ip().to_string(),
                port: payload_addr.port(),
            }),
            packet_size: 64,
            oc: Some(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: oc_addr.ip().to_string(),
                port: oc_addr.port(),
            }),
            mode: DHMode::Triggered {
                trigger: b"READ\r".to_vec(),
                interval_ms: 50,
            },
        };

        let mut dh = DataHandler::new(config.clone()).unwrap();
        let (oc_reader, oc_writer) = crate::endpoint::bind_endpoint_pair(
            &EndpointConfig::Network(config.oc.clone().unwrap()),
        )
        .unwrap();
        dh.start(oc_reader, oc_writer)
            .expect("the handler starts and connects to its payload");

        let (mut asked, _) = payload.accept().expect("the handler connects");
        asked
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();

        // The trigger arrives without the ground having said anything.
        let mut buffer = [0u8; 64];
        let n = asked.read(&mut buffer).expect("a trigger");
        assert_eq!(&buffer[..n], b"READ\r");

        // And again, because it is sent at an interval rather than once.
        let since = Instant::now();
        let n = asked.read(&mut buffer).expect("a second trigger");
        assert_eq!(&buffer[..n], b"READ\r");
        assert!(
            since.elapsed() >= Duration::from_millis(40),
            "the second trigger came {:?} after the first, which is no interval \
             at all",
            since.elapsed()
        );

        // The ground has to speak once before an answer can reach it; see
        // the OC's learnt address.
        let ground = UdpSocket::bind("127.0.0.1:0").unwrap();
        ground
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        ground.send_to(b"from the ground", oc_addr).unwrap();

        // Now answer a trigger, as the payload would, until it reaches the
        // ground: the handler may be between polls when the first answer is
        // written.
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut arrived = None;
        while arrived.is_none() && Instant::now() < deadline {
            asked.write_all(b"from the payload").expect("the payload answers");
            if let Ok((n, _)) = ground.recv_from(&mut buffer) {
                arrived = Some(buffer[..n].to_vec());
            }
        }
        assert_eq!(
            arrived.as_deref(),
            Some(&b"from the payload"[..]),
            "the payload's answer did not reach the ground"
        );

        // The triggers are counted apart from the writes they are, so that a
        // handler whose payload is not answering can be told from one the
        // ground is not sending to.
        let stats = dh.statistics();
        assert!(
            stats.triggers_sent >= 2,
            "{} triggers counted",
            stats.triggers_sent
        );
        // And they are counted nowhere else: a handler's bytes_sent is what
        // went to the ground, so the one answer that reached it is all of
        // that, however many triggers went the other way.
        assert_eq!(
            stats.bytes_sent,
            b"from the payload".len() as u64,
            "the triggers were counted as data sent to the ground"
        );

        dh.stop().unwrap();
    }

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
                    mode: Default::default(),
        };

        let mut dh = DataHandler::new(config.clone()).unwrap();
        let (oc_reader, oc_writer) =
            crate::endpoint::bind_endpoint_pair(&EndpointConfig::Network(
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
                    mode: Default::default(),
        };

        let dh = DataHandler::new(config);
        assert!(dh.is_ok());

        let dh = dh.unwrap();
        assert_eq!(dh.state(), DHState::Created);
    }
}
