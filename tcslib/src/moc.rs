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
use std::time::Duration;

use tcslibgs::graph::telemetry::{self, Telemetered, Via};
use tcslibgs::graph::Graph;

/// What the ground's end of the `payload_telemetry` links is called.
pub const PAYLOAD_TELEMETRY: &str = "payload_telemetry";

/// The largest datum one receive returns: room for a whole datagram.
const DATUM_BYTES: usize = 65_536;

/// One datum from the vehicle, and the node that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Received {
    /// The node drawn into the `moc` node that produced this.
    pub source: String,
    /// What it produced, exactly as it was sent.
    pub data: Vec<u8>,
}

/// A `moc` node, open on the ground: a port for each node drawn into it.
#[derive(Debug)]
pub struct Moc {
    name: String,
    /// Each source and the socket its data arrives at, in the order the
    /// configuration draws them.
    ports: Vec<(String, UdpSocket)>,
    /// Which port the next receive looks at first, so that one busy source
    /// cannot keep the others waiting.
    next: usize,
}

impl Moc {
    /// Open the `moc` node of this name in the configuration at `path`.
    pub fn open_file(path: impl AsRef<Path>, moc: &str) -> Result<Moc, String> {
        let path = path.as_ref();
        let graph = Graph::load(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Moc::open(&graph, moc)
    }

    /// Open the `moc` node of this name: bind the port each source's data
    /// arrives at.
    ///
    /// Refused for a name that is not a `moc` node, for one nothing is drawn
    /// into -- it would receive nothing, and a caller waiting on it would wait
    /// for ever -- and for a port that cannot be bound, which says which.
    pub fn open(graph: &Graph, moc: &str) -> Result<Moc, String> {
        match graph.node(moc) {
            Some(node) if node.node_type == "moc" => {}
            Some(node) => {
                return Err(format!(
                    "\"{moc}\" is a {} node, not a moc node",
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
                "\"{moc}\" has nothing drawn into it, so nothing would ever arrive \
                 at it"
            ));
        }
        Moc::binding(moc, ours)
    }

    /// Open the ground's end of every payload that reports over the
    /// `tcspecial` section's `payload_telemetry`: the payloads nothing in the
    /// drawing touches, each at `dest_base` plus its device id.
    ///
    /// Gathered as one, named for the section, because it is the same thing to
    /// a caller: data, and which node it came from. `None` where no payload
    /// reports that way.
    pub fn open_payload_telemetry(graph: &Graph) -> Result<Option<Moc>, String> {
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

    /// Bind each link's ground end.
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
        Ok(Moc {
            name: name.to_string(),
            ports,
            next: 0,
        })
    }

    /// The `moc` node's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The nodes whose data arrives here, in the order they are drawn.
    pub fn sources(&self) -> Vec<&str> {
        self.ports.iter().map(|(source, _)| source.as_str()).collect()
    }

    /// The next datum to arrive from any source, and which source it is from.
    ///
    /// Waits up to `wait` for one, or for ever where `wait` is `None`; `None`
    /// back is the wait running out with nothing arrived.
    pub fn receive(&mut self, wait: Option<Duration>) -> Result<Option<Received>, String> {
        let mut fds: Vec<libc::pollfd> = self
            .ports
            .iter()
            .map(|(_, socket)| libc::pollfd {
                fd: socket.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            })
            .collect();
        let timeout = match wait {
            None => -1,
            Some(wait) => i32::try_from(wait.as_millis()).unwrap_or(i32::MAX),
        };

        let ready = loop {
            let ready =
                unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) };
            if ready >= 0 {
                break ready;
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(format!("\"{}\": waiting for data failed: {error}", self.name));
            }
        };
        if ready == 0 {
            return Ok(None);
        }

        // From the port after the last one that answered, round to it.
        let count = self.ports.len();
        for step in 0..count {
            let at = (self.next + step) % count;
            if fds[at].revents & libc::POLLIN == 0 {
                continue;
            }
            let (source, socket) = &self.ports[at];
            let mut datum = vec![0u8; DATUM_BYTES];
            let got = socket.recv(&mut datum).map_err(|e| {
                format!("\"{}\": reading what \"{source}\" sent failed: {e}", self.name)
            })?;
            datum.truncate(got);
            self.next = (at + 1) % count;
            return Ok(Some(Received {
                source: source.clone(),
                data: datum,
            }));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A port nothing holds, for a base that puts a source's port on it.
    fn a_free_port() -> u16 {
        UdpSocket::bind("127.0.0.1:0")
            .expect("a port")
            .local_addr()
            .expect("its address")
            .port()
    }

    /// Two sources drawn into one moc node, whose ground_base puts the first
    /// source -- device id 1 -- on `port` and the second on the one after.
    fn graph(port: u16) -> Graph {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("vehicle.yaml");
        std::fs::write(
            &path,
            format!(
                "version: \"2.0\"
moc:
  - {{ name: ground, type: moc, ground_base: \"127.0.0.1:{}\" }}
payloads:
  - {{ name: a, type: network, protocol: udp, address: \"127.0.0.1\", port: \"7001\", payload-out: ground }}
  - {{ name: b, type: network, protocol: udp, address: \"127.0.0.1\", port: \"7002\", payload-out: ground }}
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

        let vehicle = UdpSocket::bind("127.0.0.1:0").expect("the vehicle's socket");
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

    /// A wait that runs out says nothing arrived.
    #[test]
    fn a_wait_that_runs_out_is_nothing() {
        let mut moc = Moc::open(&graph(a_free_port()), "ground").expect("it opens");
        assert_eq!(moc.receive(Some(Duration::from_millis(50))), Ok(None));
    }

    /// The payloads nothing touches are received too, named for the section.
    #[test]
    fn untouched_payloads_are_received_over_payload_telemetry() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut graph = Graph::load(root.join("examples/minimal/minimal.yaml")).expect("it reads");
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

        let vehicle = UdpSocket::bind("127.0.0.1:0").expect("the vehicle's socket");
        vehicle.send_to(b"reading", ("127.0.0.1", port)).expect("sent");
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
        assert!(said.contains("not a moc node"), "{said}");
        let said = Moc::open(&graph, "nowhere").expect_err("no such node");
        assert!(said.contains("no node called"), "{said}");
    }
}
