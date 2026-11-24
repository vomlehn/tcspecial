//! The station's links to the two vehicles: what it starts, the CONNECT it
//! sends each, the data it takes from their gateways, and what it sends up
//! through them -- each a light time late, as from the Earth.
//!
//! `lander_yamcs.py` puts a relay that holds every datagram on each of the
//! links between Yamcs and the vehicles. This station holds them itself,
//! which comes to the same: what it sends goes up [`Station::delay`] after
//! it is asked for, and what arrives is shown [`Station::delay`] after it
//! arrives. The vehicles' own links are as their configurations state them.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};

use tcsmoc::gateway::GatewayReceiver;
use tcsmoc::listening::{Heard, Listening};
use tcsmoc::ping::Uplink;
use tcsos::process::{Child, Command};
use tcsos::sync::{mpsc, Arc, Mutex};
use tcsos::thread;
use tcsos::time::{Duration, Instant};
use tcsprotocol::ccsds::{PacketType, Sender};
use tcsprotocol::graph::reach::apid_base_of;
use tcsprotocol::graph::telemetry::{self, Via};
use tcsprotocol::graph::units::unit_of;
use tcsprotocol::graph::Graph;
use tcsprotocol::unit::Unit;

/// How long each CONNECT waits for its answer, besides the light time both
/// ways.
const ANSWER: Duration = Duration::from_secs(1);

/// How long a gateway's thread waits for a datum before looking whether
/// the station is stopping.
const LISTEN: Duration = Duration::from_millis(200);

/// One datum from a vehicle, held until it is due to be seen.
#[derive(Debug)]
pub struct Arrival {
    /// When the station shows it: a light time after it arrived.
    pub due: Instant,
    /// The node that sent it.
    pub source: String,
    /// What it sent.
    pub data: Vec<u8>,
}

/// One datum to send up, held until it is due to leave.
#[derive(Debug)]
struct Going {
    /// When it leaves.
    due: Instant,
    /// The payload it is for, which names the gateway that passes it on.
    payload: String,
    /// What it carries.
    data: Vec<u8>,
}

/// A gateway's way up, and the CCSDS packets it is sent in.
struct Up {
    /// The way up.
    uplink: Uplink,
    /// Frames each datum as a telecommand under the gateway's APID, where
    /// the gateway is Yamcs's; `None` where it takes datagrams bare.
    framing: Option<Sender>,
}

/// One of the vehicles, as the station reads it.
pub struct Craft {
    /// Its configuration's name: `lander-a` or `lander-b`.
    pub name: String,
    /// Its configuration file.
    pub path: PathBuf,
    /// Its configuration.
    pub graph: Graph,
}

impl Craft {
    /// Reads a vehicle's configuration.
    ///
    /// # Arguments
    ///
    /// * `path` - Its configuration file.
    ///
    /// # Returns
    ///
    /// The vehicle.
    ///
    /// # Errors
    ///
    /// Where the file does not read, naming it.
    pub fn read(path: &Path) -> Result<Craft, String> {
        let graph = Graph::load(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Craft {
            name,
            path: path.to_path_buf(),
            graph,
        })
    }

    /// The vehicle's gateway nodes.
    ///
    /// # Returns
    ///
    /// Their names.
    fn gateways(&self) -> Vec<&str> {
        self.graph
            .nodes
            .iter()
            .filter(|node| tcsprotocol::gateway::is_gateway(&node.node_type))
            .map(|node| node.name.as_str())
            .collect()
    }

    /// Whether data is drawn into a gateway node, so that the ground has
    /// something to take from it.
    ///
    /// # Arguments
    ///
    /// * `gateway` - The node's name.
    ///
    /// # Returns
    ///
    /// True where the vehicle sends the ground anything through it.
    fn fed(&self, gateway: &str) -> bool {
        let through = Via::Gateway(gateway.to_string());
        telemetry::of(&self.graph)
            .map(|links| links.iter().any(|link| link.via == through))
            .unwrap_or(false)
    }

    /// The payloads each gateway node passes what the ground sends it on
    /// to, by its `moc-out`.
    ///
    /// # Returns
    ///
    /// Each payload, and the gateway that reaches it.
    fn uplinked(&self) -> Vec<(String, String)> {
        let mut found = Vec::new();
        for gateway in self.gateways() {
            let from = format!("{gateway}.moc-out");
            for link in self.graph.links.iter().filter(|l| l.from == from) {
                for to in link.to.each() {
                    let payload = to.split('.').next().unwrap_or(to);
                    found.push((payload.to_string(), gateway.to_string()));
                }
            }
        }
        found
    }

    /// The nodes the window's box of this vehicle's panels shows, in their
    /// names' order: each the station hears from through the vehicle's
    /// gateways, and each it commands through them.
    ///
    /// # Returns
    ///
    /// Each node's name, and whether the station commands it.
    pub fn panel_nodes(&self) -> Vec<(String, bool)> {
        let gateways = self.gateways();
        let mut nodes: BTreeMap<String, bool> = telemetry::of(&self.graph)
            .map(|links| {
                links
                    .into_iter()
                    .filter(|link| {
                        gateways
                            .iter()
                            .any(|g| link.via == Via::Gateway(g.to_string()))
                    })
                    .map(|link| (link.source, false))
                    .collect()
            })
            .unwrap_or_default();
        for (payload, _) in self.uplinked() {
            nodes.insert(payload, true);
        }
        nodes.into_iter().collect()
    }

    /// The unit of each node drawn into a gateway that sends a number.
    ///
    /// # Returns
    ///
    /// Each such node's unit, by its name.
    pub fn units(&self) -> HashMap<String, Unit> {
        self.graph
            .nodes
            .iter()
            .filter_map(|node| {
                unit_of(&self.graph, &node.name)
                    .map(|unit| (node.name.clone(), unit))
            })
            .collect()
    }
}

/// What the station tells the window of each vehicle's link.
#[derive(Debug, Clone, Default)]
pub struct Linked {
    /// Each vehicle's last word on its CONNECT, by its name.
    pub said: HashMap<String, String>,
}

/// The station's links to the vehicles.
pub struct Station {
    /// The light time each way.
    pub delay: Duration,
    /// What has arrived and is not yet due.
    arrivals: Arc<Mutex<VecDeque<Arrival>>>,
    /// What each vehicle's CONNECT last heard.
    linked: Arc<Mutex<Linked>>,
    /// Where what is to go up is handed to the thread that sends it.
    going: mpsc::Sender<Going>,
}

impl Station {
    /// Opens the links to the vehicles: binds each gateway's ports, opens
    /// each way up, and starts sending each vehicle CONNECT until it
    /// answers with its configuration's checksum.
    ///
    /// # Arguments
    ///
    /// * `crafts` - The vehicles.
    /// * `delay`  - The light time each way.
    ///
    /// # Returns
    ///
    /// The station.
    ///
    /// # Errors
    ///
    /// Where a gateway's ports or a way up cannot be opened, or a command
    /// link cannot be bound, naming the vehicle.
    pub fn open(crafts: &[Craft], delay: Duration) -> Result<Station, String> {
        let arrivals = Arc::new(Mutex::new(VecDeque::new()));
        let linked = Arc::new(Mutex::new(Linked::default()));
        let mut ups = HashMap::new();
        for craft in crafts {
            let at = |e: String| format!("{}: {e}", craft.name);
            for gateway in craft.gateways() {
                if craft.fed(gateway) {
                    let receiver = GatewayReceiver::open(&craft.graph, gateway)
                        .map_err(at)?;
                    listen(receiver, delay, Arc::clone(&arrivals));
                }
            }
            for (payload, gateway) in craft.uplinked() {
                let uplink =
                    Uplink::open(&craft.graph, &gateway).map_err(at)?;
                let node = craft.graph.node(&gateway).expect("the gateway");
                let framing = match apid_base_of(node).map_err(at)? {
                    Some(apid) => Some(
                        Sender::new(PacketType::Telecommand, apid)
                            .map_err(|e| at(e.to_string()))?,
                    ),
                    None => None,
                };
                ups.insert(payload, Up { uplink, framing });
            }
            let listening = Listening::open(&craft.graph)
                .map_err(at)?
                .ok_or_else(|| at("it states no command link".to_string()))?;
            connect(craft.name.clone(), listening, delay, Arc::clone(&linked));
        }
        let (going, leaving) = mpsc::channel();
        send_up(ups, leaving);
        Ok(Station {
            delay,
            arrivals,
            linked,
            going,
        })
    }

    /// Takes what has arrived and is now due to be seen.
    ///
    /// # Arguments
    ///
    /// * `now` - The moment.
    ///
    /// # Returns
    ///
    /// The data due, in the order they arrived.
    pub fn due(&self, now: Instant) -> Vec<Arrival> {
        let mut due = Vec::new();
        if let Ok(mut arrivals) = self.arrivals.lock() {
            while arrivals.front().is_some_and(|a| a.due <= now) {
                due.extend(arrivals.pop_front());
            }
        }
        due
    }

    /// What each vehicle's CONNECT last heard.
    ///
    /// # Returns
    ///
    /// The words, by vehicle.
    pub fn linked(&self) -> Linked {
        self.linked.lock().map(|l| l.clone()).unwrap_or_default()
    }

    /// Sends a datum up to a payload, through the gateway that passes it
    /// on, a light time from now.
    ///
    /// # Arguments
    ///
    /// * `payload` - The payload: `restart` or `sky_quench`.
    /// * `data`    - What it carries.
    pub fn send(&self, payload: &str, data: &[u8]) {
        let _ = self.going.send(Going {
            due: Instant::now() + self.delay,
            payload: payload.to_string(),
            data: data.to_vec(),
        });
    }
}

/// Takes what arrives at a gateway's ports, on a thread of its own, and
/// holds each datum for a light time.
///
/// # Arguments
///
/// * `receiver` - The gateway, open.
/// * `delay`    - The light time.
/// * `arrivals` - Where each datum is held.
fn listen(
    mut receiver: GatewayReceiver,
    delay: Duration,
    arrivals: Arc<Mutex<VecDeque<Arrival>>>,
) {
    thread::spawn(move || loop {
        match receiver.receive(Some(LISTEN)) {
            Ok(Some(datum)) => {
                if let Ok(mut held) = arrivals.lock() {
                    held.push_back(Arrival {
                        due: Instant::now() + delay,
                        source: datum.source,
                        data: datum.data,
                    });
                }
            }
            Ok(None) => {}
            Err(e) => eprintln!("lander-station: {}: {e}", receiver.name()),
        }
    });
}

/// Sends a vehicle CONNECT, on a thread of its own, until it answers with
/// its configuration's checksum: each a light time after the last answer,
/// and waiting for its own a light time each way longer.
///
/// # Arguments
///
/// * `name`      - The vehicle's name, for what is said.
/// * `listening` - Its command link.
/// * `delay`     - The light time.
/// * `linked`    - Where what it heard is said.
fn connect(
    name: String,
    mut listening: Listening,
    delay: Duration,
    linked: Arc<Mutex<Linked>>,
) {
    let ours = listening.ours();
    thread::spawn(move || loop {
        thread::sleep(delay);
        let heard = listening.connect(ANSWER + delay * 2);
        let said = heard.said(ours);
        eprintln!("lander-station: {name}: {said}");
        let matched = matches!(heard, Heard::Matched(_));
        let word = if matched {
            "connected".to_string()
        } else {
            "connecting".to_string()
        };
        if let Ok(mut linked) = linked.lock() {
            linked.said.insert(name.clone(), word);
        }
        if matched {
            return;
        }
    });
}

/// Sends each datum up when it is due, on a thread of its own.
///
/// # Arguments
///
/// * `ups`     - Each payload's way up.
/// * `leaving` - Where the data to send come from.
fn send_up(mut ups: HashMap<String, Up>, leaving: mpsc::Receiver<Going>) {
    thread::spawn(move || {
        for going in leaving.iter() {
            let wait = going.due.saturating_duration_since(Instant::now());
            if !wait.is_zero() {
                thread::sleep(wait);
            }
            let Some(up) = ups.get_mut(&going.payload) else {
                eprintln!(
                    "lander-station: no gateway passes anything on to {}",
                    going.payload
                );
                continue;
            };
            let mut room = vec![0u8; tcsprotocol::ccsds::HEADER_BYTES + 64];
            let packet = match up.framing.as_mut() {
                Some(sender) => match sender.frame(&mut room, &going.data) {
                    Ok(packet) => packet,
                    Err(e) => {
                        eprintln!("lander-station: {}: {e}", going.payload);
                        continue;
                    }
                },
                None => &going.data[..],
            };
            if let Err(e) = up.uplink.send(packet) {
                eprintln!("lander-station: {}: {e}", going.payload);
            }
        }
    });
}

/// Where a program the station starts is: beside the station where it is
/// installed, as tcs-moc finds its own; `None` where the station is run out
/// of a source tree, whose programs are run with `cargo run`.
///
/// # Arguments
///
/// * `name` - The program.
///
/// # Returns
///
/// Its path, or `None`.
fn installed(name: &str) -> Option<PathBuf> {
    let program =
        tcsos::fs::canonicalize(tcsos::env::current_exe().ok()?).ok()?;
    let dir = program.parent()?;
    if tcsos::fs::is_dir(dir.join("deps")) {
        return None;
    }
    Some(dir.join(name))
}

/// Starts a program, which ends when the station does, on the run's
/// simulator file: named by `PAYLOAD_SIM_YAML`, which is where tcspecial
/// finds it, and the run's simulation mode in it.
///
/// # Arguments
///
/// * `name` - The program: `tcs-sim` or `tcspecial`.
/// * `args` - Its arguments.
/// * `sim`  - The run's simulator file.
///
/// # Returns
///
/// The program, started.
///
/// # Errors
///
/// Where it cannot be started, naming it.
pub fn start(name: &str, args: &[String], sim: &Path) -> Result<Child, String> {
    let mut command = match installed(name) {
        Some(program) => Command::new(program),
        None => {
            let mut cargo = Command::new("cargo");
            cargo.args(["run", "--quiet", "--bin", name, "--"]);
            cargo
        }
    };
    command
        .args(args)
        .env(tcsprotocol::simulate::SIM_PATH_VAR, sim);
    tcsos::child::dies_with_parent(
        &mut command,
        tcsos::child::Ending::Terminate,
        "the lander station that started it has already ended",
    );
    command
        .spawn()
        .map_err(|e| format!("{name} cannot be started: {e}"))
}
