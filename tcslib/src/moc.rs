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
//!
//! # Holding data whose format is not known
//!
//! A datum is the node's bytes and nothing else: no header, no generation.
//! Which node sent it is the port it arrived at, and how to read it is that
//! node's configuration. When the vehicle changes configuration -- a commanded
//! switch, or a fallback to the golden build -- a datum the old start sent can
//! still be on its way, held by a relay or a recorder, and would arrive at a
//! port the new configuration may give to another node, in another format.
//! Nothing in it says so.
//!
//! So a [`DataHold`], fed every beacon, stops attributing data the moment the
//! beacons say the configuration or the interface has changed, or that it is
//! not the one this station reads with, and keeps on not attributing it until
//! the station says the path has been purged of what the old configuration
//! sent: a datum that arrives meanwhile comes back marked
//! [`Arrival::held`], its bytes and its port, with nothing claimed about who
//! sent it. One hold for a station, shared by every `Moc` it opens, so that
//! opening the new configuration's `moc` nodes does not end it. See "What is
//! held at the MOC" in `docs/design.rst`.

use std::net::UdpSocket;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tcslibgs::config_digest::{digest_of_graph, ConfigDigest};
use tcslibgs::graph::telemetry::{self, Telemetered, Via};
use tcslibgs::graph::Graph;
use tcslibgs::InterfaceDigest;

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
    /// Whether it arrived while the station's [`DataHold`] was holding, so
    /// that `source` is only the name this configuration gives the port it
    /// arrived at, and the bytes may be in another configuration's format.
    pub held: bool,
}

/// Where [`Moc::receive_into`] put a datum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrival {
    /// Which source sent it, as its place in [`Moc::sources`]: see
    /// [`Moc::source`].
    pub source: usize,
    /// How many bytes of the buffer it filled.
    pub length: usize,
    /// Whether it arrived while the station's [`DataHold`] was holding: see
    /// [`Received::held`].
    pub held: bool,
}

/// Why a station's data is being held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hold {
    /// The last beacon says the vehicle is serving a configuration or an
    /// interface other than the one this station reads with.
    Differs,
    /// The configuration or the interface the beacons say changed between
    /// one beacon and the next: a switch, or a fallback to the golden build.
    Changed,
    /// The first beacon this station heard is of a start the ground
    /// commanded, which may have taken up definitions this station did not
    /// see the vehicle change to.
    CommandedStart,
}

impl Hold {
    /// Gives the number the hold is kept as in a [`DataHold`]'s atomic.
    ///
    /// # Returns
    ///
    /// 1, 2 or 3: never 0, which is no hold.
    fn number(self) -> u8 {
        match self {
            Hold::Differs => 1,
            Hold::Changed => 2,
            Hold::CommandedStart => 3,
        }
    }

    /// Reads back what [`Hold::number`] gives.
    ///
    /// # Arguments
    ///
    /// * `number` - The number a [`DataHold`] keeps.
    ///
    /// # Returns
    ///
    /// The hold, or `None` for 0, which is no hold, or any number that is
    /// not one.
    fn of(number: u8) -> Option<Hold> {
        match number {
            1 => Some(Hold::Differs),
            2 => Some(Hold::Changed),
            3 => Some(Hold::CommandedStart),
            _ => None,
        }
    }

    /// Says why data is held, in words an operator reads beside the button
    /// that ends it.
    ///
    /// # Returns
    ///
    /// The words.
    pub fn why(self) -> &'static str {
        match self {
            Hold::Differs => {
                "the vehicle is serving another configuration than this \
                 station read, so its data cannot be read with this one"
            }
            Hold::Changed => {
                "the vehicle's configuration changed, and data the old one \
                 sent may still be on its way: purge the path, then say so"
            }
            Hold::CommandedStart => {
                "the vehicle was restarted by command before this station \
                 heard it, and data an earlier configuration sent may still \
                 be on its way: purge the path, then say so"
            }
        }
    }
}

/// What the beacons say the vehicle serves: its configuration's digest and
/// its interface's.
type Serving = (ConfigDigest, InterfaceDigest);

#[derive(Debug, Default)]
struct HoldState {
    /// What this station reads with: the digest of the configuration its
    /// `Moc`s were opened from, and this build's interface. `None` until one
    /// has been opened.
    ours: Option<Serving>,
    /// What the last beacon said, if one has been heard.
    last: Option<Serving>,
}

/// Whether a station's data is being held, shared by every [`Moc`] it opens
/// and fed every beacon it hears. Cheap to clone: the clones are one hold.
///
/// Asked once for every datum, without a lock or an allocation; told of a
/// beacon, which takes a lock, on the beacon's thread.
#[derive(Debug, Clone, Default)]
pub struct DataHold {
    held: Arc<AtomicU8>,
    state: Arc<Mutex<HoldState>>,
}

impl DataHold {
    /// A hold that is not holding, and has heard no beacon.
    pub fn new() -> DataHold {
        DataHold::default()
    }

    /// Why data is being held, or `None` where it is not.
    pub fn held(&self) -> Option<Hold> {
        Hold::of(self.held.load(Ordering::SeqCst))
    }

    /// Begins holding, unless a hold has already begun; either way the
    /// reason kept is `why`.
    ///
    /// # Arguments
    ///
    /// * `why` - Why data is to be held.
    ///
    /// # Returns
    ///
    /// `why` where this began the hold, and `None` where data was already
    /// held.
    fn hold(&self, why: Hold) -> Option<Hold> {
        match self.held.swap(why.number(), Ordering::SeqCst) {
            0 => Some(why),
            _ => None,
        }
    }

    /// Takes note that the station reads with a configuration, and this
    /// build's interface: said by [`Moc::open`]. Holds where the last beacon
    /// served something else.
    ///
    /// # Arguments
    ///
    /// * `digest` - The digest of the configuration the station reads with.
    fn reading_with(&self, digest: ConfigDigest) {
        let ours = (digest, InterfaceDigest::of_this_build());
        if let Ok(mut state) = self.state.lock() {
            state.ours = Some(ours);
            if state.last.is_some_and(|last| last != ours) {
                self.hold(Hold::Differs);
            }
        }
    }

    /// Takes note that a beacon has arrived.
    ///
    /// Holds where what it serves is not what this station reads with,
    /// where it differs from what the beacon before it served, and where
    /// it is the first beacon heard and its start was commanded. Nothing
    /// here ends a hold: only [`DataHold::purged`] does.
    ///
    /// # Arguments
    ///
    /// * `config`    - The digest of the configuration the beacon serves.
    /// * `interface` - The digest of the interface it serves.
    /// * `commanded` - Whether its start was commanded.
    ///
    /// # Returns
    ///
    /// Why the hold began, where this began it, for the station to say so;
    /// otherwise `None`.
    pub fn beacon_heard(
        &self,
        config: ConfigDigest,
        interface: InterfaceDigest,
        commanded: bool,
    ) -> Option<Hold> {
        let now = (config, interface);
        let Ok(mut state) = self.state.lock() else {
            return None;
        };
        let why = match (state.last, state.ours) {
            (Some(last), _) if last != now => Some(Hold::Changed),
            (_, Some(ours)) if ours != now => Some(Hold::Differs),
            (None, _) if commanded => Some(Hold::CommandedStart),
            _ => None,
        };
        state.last = Some(now);
        why.and_then(|why| self.hold(why))
    }

    /// What a pass of the station's [`crate::beacon::Beacons`] heard, handed
    /// on: [`DataHold::beacon_heard`] for a pass on which a beacon arrived,
    /// and nothing for one on which none did.
    ///
    /// # Arguments
    ///
    /// * `pass` - What the pass heard.
    ///
    /// # Returns
    ///
    /// Why the hold began, where this began it; otherwise `None`.
    pub fn pass_heard(&self, pass: &crate::beacon::Pass) -> Option<Hold> {
        if pass.arrived != crate::beacon::Arrived::Beacon {
            return None;
        }
        let (config, interface) = pass.serving?;
        self.beacon_heard(config, interface, pass.commanded_restart)
    }

    /// The station says the path from the vehicle has been purged of what
    /// an earlier configuration sent: the hold ends, and data is attributed
    /// again.
    ///
    /// Refused while the last beacon serves another configuration or
    /// interface than this station reads with, saying both: purging the
    /// path does not make the station read the right configuration, and
    /// data attributed by the wrong one would be worse than data held.
    ///
    /// # Errors
    ///
    /// A message saying both configurations and both interfaces, where the
    /// last beacon serves another than this station reads with, or that
    /// the hold could not be read.
    pub fn purged(&self) -> Result<(), String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "the hold could not be read".to_string())?;
        if let (Some((config, interface)), Some((ours, ours_interface))) =
            (state.last, state.ours)
        {
            if config != ours || interface != ours_interface {
                return Err(format!(
                    "data is still held: the vehicle is serving configuration \
                     md5 {config} and interface {interface}, and this station \
                     read md5 {ours} and interface {ours_interface}. Open the \
                     configuration the vehicle is serving, then say the path \
                     is purged"
                ));
            }
        }
        self.held.store(0, Ordering::SeqCst);
        Ok(())
    }
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
    /// Whether the station's data is being held: its own until
    /// [`Moc::share`] gives it the station's.
    hold: DataHold,
    /// The digest of the configuration this was opened from, which is what
    /// it reads data with.
    digest: ConfigDigest,
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
        Moc::binding(moc, ours, graph)
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
        Moc::binding(PAYLOAD_TELEMETRY, ours, graph).map(Some)
    }

    /// Binds each link's ground end, as one node.
    ///
    /// # Arguments
    ///
    /// * `name`  - What the node is called.
    /// * `links` - The links whose ground ends it binds.
    /// * `graph` - The configuration's graph, whose digest the node reads
    ///             data with.
    ///
    /// # Returns
    ///
    /// The open node, with a hold of its own.
    ///
    /// # Errors
    ///
    /// A message where the configuration cannot be digested, or saying
    /// which source's port cannot be bound.
    fn binding(
        name: &str,
        links: Vec<Telemetered>,
        graph: &Graph,
    ) -> Result<Moc, String> {
        let digest = digest_of_graph(graph).map_err(|e| {
            format!("\"{name}\": the configuration cannot be digested: {e}")
        })?;
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
        let hold = DataHold::new();
        hold.reading_with(digest);
        Ok(Moc {
            name: name.to_string(),
            ports,
            fds,
            next: 0,
            hold,
            digest,
        })
    }

    /// Hold this one's data whenever `hold` -- the station's, which every
    /// `Moc` it opens shares -- is holding, in place of the hold it was
    /// opened with.
    pub fn share(&mut self, hold: &DataHold) {
        hold.reading_with(self.digest);
        self.hold = hold.clone();
    }

    /// The hold this asks before it attributes a datum.
    pub fn hold(&self) -> &DataHold {
        &self.hold
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
            held: arrival.held,
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
        for fd in &mut self.fds {
            fd.revents = 0;
        }
        let timeout = match wait {
            None => -1,
            Some(wait) => i32::try_from(wait.as_millis()).unwrap_or(i32::MAX),
        };

        let ready = loop {
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
            if length > out.len() {
                return Err(format!(
                    "\"{}\": \"{source}\" sent {length} bytes, which is more \
                     than the {} there was room for: receive into a buffer of \
                     DATUM_BYTES",
                    self.name,
                    out.len()
                ));
            }
            // Asked as each datum is taken, so that one that arrived
            // before a beacon began a hold is attributed and one after it
            // is not. One atomic load, which allocates nothing.
            return Ok(Some(Arrival {
                source: at,
                length,
                held: self.hold.held().is_some(),
            }));
        }
        Ok(None)
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

    /// A hold begins when the beacons say the configuration or interface
    /// changed, or differs from the station's, or on a first beacon of a
    /// commanded start; it ends only when the station says the path is
    /// purged, and not while the vehicle serves another configuration.
    #[test]
    fn data_is_held_from_a_change_until_the_path_is_purged() {
        let ours = ConfigDigest([1; 16]);
        let theirs = ConfigDigest([2; 16]);
        let interface = InterfaceDigest::of_this_build();
        let hold = DataHold::new();
        hold.reading_with(ours);

        assert_eq!(hold.beacon_heard(ours, interface, false), None);
        assert_eq!(hold.held(), None, "the configuration this station read");

        // A switch: the beacons change, and the station is not yet reading
        // the new configuration.
        assert_eq!(
            hold.beacon_heard(theirs, interface, true),
            Some(Hold::Changed)
        );
        assert_eq!(hold.beacon_heard(theirs, interface, true), None, "once");
        let refused = hold.purged().expect_err("still the wrong one");
        assert!(refused.contains(&theirs.to_string()), "{refused}");
        assert_eq!(hold.held(), Some(Hold::Differs));

        // The station opens the new configuration: still held, because what
        // the old one sent may still be on its way.
        hold.reading_with(theirs);
        assert!(hold.held().is_some());
        hold.purged().expect("the path is purged");
        assert_eq!(hold.held(), None);
        assert_eq!(hold.beacon_heard(theirs, interface, true), None);

        // A station whose first beacon is of a commanded start did not see
        // what changed before it.
        let fresh = DataHold::new();
        fresh.reading_with(ours);
        assert_eq!(
            fresh.beacon_heard(ours, interface, true),
            Some(Hold::CommandedStart)
        );
        let quiet = DataHold::new();
        quiet.reading_with(ours);
        assert_eq!(quiet.beacon_heard(ours, interface, false), None);
    }

    /// A datum that arrives while the station's hold is holding is marked
    /// held, and one after the hold ends is not; every `Moc` sharing the hold
    /// holds together.
    #[test]
    fn a_datum_that_arrives_while_held_is_marked() {
        let port = a_free_port();
        let graph = graph(port);
        let hold = DataHold::new();
        let mut moc = Moc::open(&graph, "ground").expect("it opens");
        moc.share(&hold);
        let digest = digest_of_graph(&graph).expect("a digest");
        let interface = InterfaceDigest::of_this_build();
        let vehicle =
            UdpSocket::bind("127.0.0.1:0").expect("the vehicle's socket");
        let mut next = |what: &[u8]| {
            vehicle.send_to(what, ("127.0.0.1", port)).expect("sent");
            moc.receive(Some(Duration::from_secs(5)))
                .expect("it reads")
                .expect("something arrived")
        };

        hold.beacon_heard(digest, interface, false);
        assert!(!next(b"before").held);
        hold.beacon_heard(ConfigDigest([9; 16]), interface, true);
        let held = next(b"during");
        assert!(held.held);
        assert_eq!(held.data, b"during", "the bytes, as they came");
        hold.beacon_heard(digest, interface, true);
        hold.purged().expect("purged");
        assert!(!next(b"after").held);
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
