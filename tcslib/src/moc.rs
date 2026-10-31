//! Getting the vehicle's data from a `moc` node, on the ground.
//!
//! A `moc` node is the ground's end of the vehicle's data. tcspecial sends
//! what each node drawn into it produces as UDP datagrams to the node's
//! `ground_base` address, at its port plus the source node's device id -- see
//! `tcslibgs::graph::telemetry`, which is where both ends work that out. So a
//! `moc` node is a set of ports, one per source, and the port a datagram
//! arrives at says which node sent it.
//!
//! Ground software does not open those ports itself. It names the `moc` node
//! in the configuration, [`Moc::open`]s it, and calls [`Moc::receive`] for
//! each datum, which comes back with the name of the node that produced it:
//! what to do with it next -- process it, display it, log it -- is the
//! caller's. One function between the configuration and the data means one
//! place that counts ports, and it is the same counting the vehicle does.

use std::net::UdpSocket;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::time::{Duration, Instant};

use tcslibgs::graph::telemetry::{self, Telemetered, Via};
use tcslibgs::graph::Graph;

use crate::matched::Matched;

/// What the ground's end of the `payload_telemetry` links is called.
pub const PAYLOAD_TELEMETRY: &str = "payload_telemetry";

/// The largest datum one receive returns: room for a whole datagram, and
/// the size of a buffer that [`Moc::receive_into`] can always fill.
pub const DATUM_BYTES: usize = 65_536;

/// One datum from the vehicle, and the node that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Received {
    /// The node drawn into the `moc` node that produced this.
    pub source: String,
    /// What it produced, exactly as it was sent.
    pub data: Vec<u8>,
}

/// Where [`Moc::receive_into`] put a datum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrival {
    /// Which source sent it, as its place in [`Moc::sources`]: see
    /// [`Moc::source`].
    pub source: usize,
    /// How many bytes of the buffer it filled.
    pub length: usize,
}

/// A `moc` node, open on the ground: a port for each node drawn into it.
#[derive(Debug)]
pub struct Moc {
    name: String,
    /// Each source and the socket its data arrives at, in the order the
    /// configuration draws them.
    ports: Vec<(String, UdpSocket)>,
    /// What a receive waits on, one for each port and in the same order:
    /// made once, when the node is opened.
    fds: Vec<libc::pollfd>,
    /// Which port the next receive looks at first, so that one busy source
    /// cannot keep the others waiting.
    next: usize,
    /// Whether the station is matched with the vehicle, where it watches
    /// the beacons: see [`Moc::gated_by`]. `None` reads everything.
    gate: Option<Matched>,
}

impl Moc {
    /// Opens a `moc` node of the configuration in a file, as [`Moc::open`]
    /// does.
    ///
    /// # Arguments
    ///
    /// * `path` - The configuration file.
    /// * `moc`  - The `moc` node's name.
    ///
    /// # Returns
    ///
    /// The open node.
    ///
    /// # Errors
    ///
    /// A message naming the file where it cannot be loaded, and whatever
    /// [`Moc::open`] refuses.
    pub fn open_file(path: impl AsRef<Path>, moc: &str) -> Result<Moc, String> {
        let path = path.as_ref();
        let graph = Graph::load(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Moc::open(&graph, moc)
    }

    /// Opens a `moc` node: binds the port each source's data arrives at.
    ///
    /// # Arguments
    ///
    /// * `graph` - The configuration's graph.
    /// * `moc`   - The `moc` node's name.
    ///
    /// # Returns
    ///
    /// The open node, with a hold of its own.
    ///
    /// # Errors
    ///
    /// Refused for a name that is not a `moc` node, for one nothing is drawn
    /// into -- it would receive nothing, and a caller waiting on it would wait
    /// for ever -- and for a port that cannot be bound, which says which.
    pub fn open(graph: &Graph, moc: &str) -> Result<Moc, String> {
        match graph.node(moc) {
            Some(node) if tcslibgs::gateway::is_gateway(&node.node_type) => {}
            Some(node) => {
                return Err(format!(
                    "\"{moc}\" is a {} node, not a gateway node",
                    node.node_type
                ))
            }
            None => return Err(format!("there is no node called \"{moc}\"")),
        }

        let links = telemetry::of(graph).map_err(|refused| refused.why)?;
        let ours: Vec<Telemetered> = links
            .into_iter()
            .filter(|l| l.via == Via::Moc(moc.to_string()))
            .collect();
        if ours.is_empty() {
            return Err(format!(
                "\"{moc}\" has nothing drawn into it, so nothing would ever \
                 arrive at it"
            ));
        }
        Moc::binding(moc, ours)
    }

    /// Open the ground's end of every payload that reports over the
    /// `tcspecial` section's `payload_telemetry`: the payloads nothing in the
    /// drawing touches, each at `dest_base` plus its device id.
    ///
    /// Gathered as one, named for the section, because it is the same thing to
    /// a caller: data, and which node it came from.
    ///
    /// # Arguments
    ///
    /// * `graph` - The configuration's graph.
    ///
    /// # Returns
    ///
    /// The ports gathered as one node, or `None` where no payload reports
    /// that way.
    ///
    /// # Errors
    ///
    /// A message saying what is wrong with the telemetry links, or which
    /// port cannot be bound.
    pub fn open_payload_telemetry(
        graph: &Graph,
    ) -> Result<Option<Moc>, String> {
        let links = telemetry::of(graph).map_err(|refused| refused.why)?;
        let ours: Vec<Telemetered> = links
            .into_iter()
            .filter(|l| l.via == Via::PayloadTelemetry)
            .collect();
        if ours.is_empty() {
            return Ok(None);
        }
        Moc::binding(PAYLOAD_TELEMETRY, ours).map(Some)
    }

    /// Binds each link's ground end, as one node.
    ///
    /// # Arguments
    ///
    /// * `name`  - What the node is called.
    /// * `links` - The links whose ground ends it binds.
    ///
    /// # Returns
    ///
    /// The open node.
    ///
    /// # Errors
    ///
    /// A message saying which source's port cannot be bound.
    fn binding(name: &str, links: Vec<Telemetered>) -> Result<Moc, String> {
        let mut ports = Vec::new();
        for link in links {
            let at = format!("{}:{}", link.ground.address, link.ground.port);
            let socket = UdpSocket::bind(&at).map_err(|e| {
                format!(
                    "\"{name}\": cannot listen for \"{}\" on {at}: {e}",
                    link.source
                )
            })?;
            ports.push((link.source, socket));
        }
        let fds = ports
            .iter()
            .map(|(_, socket)| libc::pollfd {
                fd: socket.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            })
            .collect();
        Ok(Moc {
            name: name.to_string(),
            ports,
            fds,
            next: 0,
            gate: None,
        })
    }

    /// Reads data only while the station is matched with the vehicle for
    /// listening, and counts every datum in `matched`.
    ///
    /// While the station is not matched every datum that arrives is dropped
    /// and counted, rather than read with a configuration that may not be
    /// the one that made it; each time a match begins, everything already
    /// queued on this node's sockets is dropped and counted first, since all
    /// of it was made before the match. A station that watches the vehicle's
    /// beacons gates every node it opens so; one that does not has nothing
    /// to gate by, and reads everything.
    ///
    /// # Arguments
    ///
    /// * `matched` - Whether the station is matched, which its beacon watch
    ///               keeps, and where what arrives is counted.
    pub fn gated_by(&mut self, matched: Matched) {
        // Allocates, once, as the node is gated: the match's list of
        // sockets to empty.
        for (_, socket) in &self.ports {
            matched.queued_on(socket.as_raw_fd());
        }
        self.gate = Some(matched);
    }

    /// The `moc` node's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Lists the nodes whose data arrives here, in the order they are drawn.
    ///
    /// # Returns
    ///
    /// Their names.
    pub fn sources(&self) -> Vec<&str> {
        self.ports
            .iter()
            .map(|(source, _)| source.as_str())
            .collect()
    }

    /// The source at this place in [`Moc::sources`], which is what an
    /// [`Arrival`] names.
    pub fn source(&self, index: usize) -> Option<&str> {
        self.ports.get(index).map(|(source, _)| source.as_str())
    }

    /// The next datum to arrive from any source, and which source it is from.
    ///
    /// Waits up to `wait` for one, or for ever where `wait` is `None`; `None`
    /// back is the wait running out with nothing arrived.
    ///
    /// Allocates the datum and its source's name, each time: for a caller
    /// that keeps what it receives. A loop that does not, [`Moc::receive_into`]
    /// a buffer it made before it started.
    ///
    /// # Arguments
    ///
    /// * `wait` - How long to wait for a datum, or `None` for ever.
    ///
    /// # Returns
    ///
    /// The datum, its source and whether it is held, or `None` where the
    /// wait ran out with nothing arrived.
    ///
    /// # Errors
    ///
    /// What [`Moc::receive_into`] refuses.
    pub fn receive(
        &mut self,
        wait: Option<Duration>,
    ) -> Result<Option<Received>, String> {
        let mut datum = vec![0u8; DATUM_BYTES];
        let Some(arrival) = self.receive_into(wait, &mut datum)? else {
            return Ok(None);
        };
        datum.truncate(arrival.length);
        Ok(Some(Received {
            source: self.ports[arrival.source].0.clone(),
            data: datum,
        }))
    }

    /// [`Moc::receive`], into `out`, allocating nothing: where the datum was
    /// put, and which source sent it.
    ///
    /// # Arguments
    ///
    /// * `wait` - How long to wait for a datum, or `None` for ever.
    /// * `out`  - Where the datum is put, from its start.
    ///
    /// # Returns
    ///
    /// Which source sent the datum, how long it is and whether it is held,
    /// or `None` where the wait ran out with nothing arrived.
    ///
    /// # Errors
    ///
    /// A message where waiting or reading fails. A datum longer than `out`
    /// is refused, saying how long it was, rather than cut short: a buffer
    /// of [`DATUM_BYTES`] holds any.
    pub fn receive_into(
        &mut self,
        wait: Option<Duration>,
        out: &mut [u8],
    ) -> Result<Option<Arrival>, String> {
        let deadline = wait.map(|wait| Instant::now() + wait);
        loop {
            let left = deadline.map(|deadline| {
                deadline.saturating_duration_since(Instant::now())
            });
            let Some(arrival) = self.next_arrival(left, out)? else {
                return Ok(None);
            };
            match self.gate.as_ref() {
                // Not matched: dropped and counted, and the wait goes on for
                // as long as it was asked to.
                Some(matched) if !matched.for_listening() => {
                    matched.mismatched(1);
                    continue;
                }
                Some(matched) if arrival.length > out.len() => {
                    matched.malformed();
                }
                Some(matched) => matched.processed(),
                None => {}
            }
            if arrival.length > out.len() {
                let (source, _) = &self.ports[arrival.source];
                return Err(format!(
                    "\"{}\": \"{source}\" sent {} bytes, which is more \
                     than the {} there was room for: receive into a buffer of \
                     DATUM_BYTES",
                    self.name,
                    arrival.length,
                    out.len()
                ));
            }
            return Ok(Some(arrival));
        }
    }

    /// The next datagram to arrive at any of this node's sockets, waiting up
    /// to `wait` for one, or for ever where it is `None`.
    ///
    /// # Arguments
    ///
    /// * `wait` - How long to wait, or `None` for ever.
    /// * `out`  - Where the datagram is put, from its start.
    ///
    /// # Returns
    ///
    /// Which source sent it and how long it was, which is more than `out`
    /// holds where it was cut short; `None` where the wait ran out.
    ///
    /// # Errors
    ///
    /// A message where waiting or reading fails.
    fn next_arrival(
        &mut self,
        wait: Option<Duration>,
        out: &mut [u8],
    ) -> Result<Option<Arrival>, String> {
        for fd in &mut self.fds {
            fd.revents = 0;
        }
        let timeout = match wait {
            None => -1,
            Some(wait) => i32::try_from(wait.as_millis()).unwrap_or(i32::MAX),
        };

        let ready = loop {
            // SAFETY: poll needs an array of as many pollfds as it is told,
            // writable for the call: the count is the Vec's own length. Each
            // descriptor is one of `self.ports`' sockets, which `fds` was
            // built from in `binding` and which nothing replaces or closes
            // while `self` lives.
            let ready = unsafe {
                libc::poll(
                    self.fds.as_mut_ptr(),
                    self.fds.len() as libc::nfds_t,
                    timeout,
                )
            };
            if ready >= 0 {
                break ready;
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(format!(
                    "\"{}\": waiting for data failed: {error}",
                    self.name
                ));
            }
        };
        if ready == 0 {
            return Ok(None);
        }

        // From the port after the last one that answered, round to it.
        let count = self.ports.len();
        for step in 0..count {
            let at = (self.next + step) % count;
            if self.fds[at].revents & libc::POLLIN == 0 {
                continue;
            }
            let (source, socket) = &self.ports[at];
            // MSG_TRUNC, so that what comes back is the datagram's length
            // and not what fitted: one cut short is told from one that
            // happened to fill the buffer.
            //
            // SAFETY: recv needs an open socket and a buffer writable for the
            // length it is given: the socket is borrowed from `self.ports`,
            // and the buffer and its length are both `out`'s. MSG_TRUNC
            // changes what is returned, not how much is written.
            let got = unsafe {
                libc::recv(
                    socket.as_raw_fd(),
                    out.as_mut_ptr().cast(),
                    out.len(),
                    libc::MSG_TRUNC,
                )
            };
            self.next = (at + 1) % count;
            let Ok(length) = usize::try_from(got) else {
                return Err(format!(
                    "\"{}\": reading what \"{source}\" sent failed: {}",
                    self.name,
                    std::io::Error::last_os_error()
                ));
            };
            return Ok(Some(Arrival { source: at, length }));
        }
        Ok(None)
    }
}

impl Drop for Moc {
    /// Takes this node's sockets off its match's list before they close, so
    /// that the descriptors' numbers, which the system may give to anything
    /// opened later, are never read from by the match.
    fn drop(&mut self) {
        if let Some(matched) = self.gate.as_ref() {
            for (_, socket) in &self.ports {
                matched.no_longer_queued_on(socket.as_raw_fd());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Finds a port nothing holds, for a base that puts a source's port on
    /// it.
    ///
    /// # Returns
    ///
    /// The port, free when it was looked at.
    fn a_free_port() -> u16 {
        UdpSocket::bind("127.0.0.1:0")
            .expect("a port")
            .local_addr()
            .expect("its address")
            .port()
    }

    /// Makes a configuration of two sources drawn into one moc node.
    ///
    /// # Arguments
    ///
    /// * `port` - Where the `ground_base` puts the first source -- device
    ///            id 1; the second goes on the one after.
    ///
    /// # Returns
    ///
    /// The configuration's graph.
    fn graph(port: u16) -> Graph {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("vehicle.yaml");
        std::fs::write(
            &path,
            format!(
                "moc:
  - {{ name: ground, type: moc, ground_base: \"127.0.0.1:{}\" }}
payloads:
  - {{ name: a, type: network, protocol: udp, address: \"127.0.0.1\", \
    port: \"7001\", payload-out: ground }}
  - {{ name: b, type: network, protocol: udp, address: \"127.0.0.1\", \
    port: \"7002\", payload-out: ground }}
",
                port - 1
            ),
        )
        .expect("written");
        Graph::load(&path).expect("it reads")
    }

    /// Each datum comes back with the node that produced it, told apart by
    /// the port it arrived at.
    #[test]
    fn each_datum_says_which_node_sent_it() {
        let port = a_free_port();
        let mut moc = Moc::open(&graph(port), "ground").expect("it opens");
        assert_eq!(moc.sources(), vec!["a", "b"]);

        let vehicle =
            UdpSocket::bind("127.0.0.1:0").expect("the vehicle's socket");
        vehicle
            .send_to(b"from a", ("127.0.0.1", port))
            .expect("sent");
        vehicle
            .send_to(b"from b", ("127.0.0.1", port + 1))
            .expect("sent");

        let mut got = Vec::new();
        for _ in 0..2 {
            let received = moc
                .receive(Some(Duration::from_secs(5)))
                .expect("it reads")
                .expect("something arrived");
            got.push((received.source, received.data));
        }
        got.sort();
        assert_eq!(
            got,
            vec![
                ("a".to_string(), b"from a".to_vec()),
                ("b".to_string(), b"from b".to_vec())
            ]
        );
    }

    /// An F´ gateway is opened as a ground link is, and what is drawn into
    /// it arrives at its bridge_base plus the source's id: what the F´
    /// bridge component, built on this through tcslib-c, receives.
    #[test]
    fn an_fprime_gateway_receives_at_its_bridge_base() {
        let port = a_free_port();
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("vehicle.yaml");
        std::fs::write(
            &path,
            format!(
                "moc:
  - {{ name: fsw, type: gateway_fprime, bridge_base: \"127.0.0.1:{}\", \
    fprime_id_base: \"0x3000\" }}
payloads:
  - {{ name: a, type: network, protocol: udp, address: \"127.0.0.1\", \
    port: \"7001\", payload-out: fsw }}
",
                port - 1
            ),
        )
        .expect("written");
        let graph = Graph::load(&path).expect("it reads");
        let mut moc = Moc::open(&graph, "fsw").expect("it opens");
        assert_eq!(moc.sources(), vec!["a"]);

        UdpSocket::bind("127.0.0.1:0")
            .expect("the vehicle's socket")
            .send_to(b"from a", ("127.0.0.1", port))
            .expect("sent");
        let received = moc
            .receive(Some(Duration::from_secs(5)))
            .expect("it reads")
            .expect("something arrived");
        assert_eq!(
            (received.source, received.data),
            ("a".to_string(), b"from a".to_vec())
        );
    }

    /// A gated node reads nothing while the station is not matched, and
    /// drops and counts it; at a match it drops and counts what was queued
    /// before reading on; and every datum is counted once.
    #[test]
    fn a_gated_node_reads_only_while_matched() {
        use crate::beacon::Stage;
        use tcslibgs::ConfigDigest;

        let ours = ConfigDigest([0x5A; 16]);
        let port = a_free_port();
        let mut moc = Moc::open(&graph(port), "ground").expect("it opens");
        let matched = crate::matched::Matched::for_configuration(ours);
        moc.gated_by(matched.clone());
        let vehicle =
            UdpSocket::bind("127.0.0.1:0").expect("the vehicle's socket");
        let send = |what: &[u8]| {
            vehicle.send_to(what, ("127.0.0.1", port)).expect("sent");
        };
        let mut datum = vec![0u8; DATUM_BYTES];
        let wait = Some(Duration::from_millis(200));

        // Not matched: what arrives is dropped and counted.
        send(b"before");
        assert_eq!(moc.receive_into(wait, &mut datum), Ok(None));
        assert_eq!(matched.counts().mismatched, 1);

        // Queued while not matched, and dropped as the match begins, before
        // anything is read; what comes after it is read.
        send(b"queued");
        send(b"queued too");
        matched.beacon(false, Some(ours), Stage::Heard, false);
        assert_eq!(matched.counts().mismatched, 3, "dropped at the match");
        assert_eq!(moc.receive_into(wait, &mut datum), Ok(None));
        send(b"matched");
        let arrival = moc
            .receive_into(Some(Duration::from_secs(5)), &mut datum)
            .expect("it reads")
            .expect("read while matched");
        assert_eq!(&datum[..arrival.length], b"matched");

        // The match ends with a flagged beacon: nothing is read again.
        matched.beacon(true, Some(ours), Stage::Heard, false);
        send(b"flagged");
        assert_eq!(moc.receive_into(wait, &mut datum), Ok(None));

        let counts = matched.counts();
        assert_eq!(counts.processed, 1);
        assert_eq!(counts.mismatched, 4);
        assert_eq!(counts.received(), 5, "every datum counted once");
        assert_eq!(counts.malformed, 0);
    }

    /// A wait that runs out says nothing arrived.
    #[test]
    fn a_wait_that_runs_out_is_nothing() {
        let mut moc =
            Moc::open(&graph(a_free_port()), "ground").expect("it opens");
        assert_eq!(moc.receive(Some(Duration::from_millis(50))), Ok(None));
    }

    /// The payloads nothing touches are received too, named for the section.
    #[test]
    fn untouched_payloads_are_received_over_payload_telemetry() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut graph = Graph::load(root.join("examples/minimal/minimal.yaml"))
            .expect("it reads");
        // Somewhere free instead of the example's 6100.
        let port = a_free_port();
        let section = graph.tcspecial.as_mut().expect("a section");
        let mut flat = tcslibgs::graph::parameters::flatten(section);
        flat.insert(
            "payload_telemetry.dest_base".to_string(),
            tcslibgs::Scalar::of(format!("127.0.0.1:{port}")),
        );
        *section = tcslibgs::graph::parameters::nest(&flat);

        let mut ground = Moc::open_payload_telemetry(&graph)
            .expect("it opens")
            .expect("the sensor reports that way");
        assert_eq!(ground.name(), PAYLOAD_TELEMETRY);
        assert_eq!(ground.sources(), vec!["sensor"]);

        let vehicle =
            UdpSocket::bind("127.0.0.1:0").expect("the vehicle's socket");
        vehicle
            .send_to(b"reading", ("127.0.0.1", port))
            .expect("sent");
        let received = ground
            .receive(Some(Duration::from_secs(5)))
            .expect("it reads")
            .expect("something arrived");
        assert_eq!(received.source, "sensor");
        assert_eq!(received.data, b"reading".to_vec());
    }

    /// Only a moc node can be opened, and only one something is drawn into.
    #[test]
    fn what_cannot_be_opened_says_why() {
        let graph = graph(a_free_port());
        let said = Moc::open(&graph, "a").expect_err("a payload");
        assert!(said.contains("not a gateway node"), "{said}");
        let said = Moc::open(&graph, "nowhere").expect_err("no such node");
        assert!(said.contains("no node called"), "{said}");
    }
}
