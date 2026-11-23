//! Whether two configurations can run at once, on one machine or on one
//! network: what each holds, and which of those the other holds too.
//!
//! One configuration is held to wanting nothing twice by its own checks --
//! two nodes that want one thing (`Problem::TwoNodesWantOneThing`) and two
//! sockets that are one ([`crate::graph::sockets`]). Two configurations run
//! by two tcspecials are held to the same rules across the pair, with what
//! each holds listed by the same functions, so that the two checks cannot
//! disagree about what a node claims. What a configuration holds that one
//! configuration cannot hold twice is added: the files and directories its
//! `tcspecial` section names, each log node's directory, and, with a
//! simulator file, where tcs-sim answers.
//!
//! Some of what one configuration may not want twice, two processes may
//! share, and a few rules differ so:
//!
//! * A path that is a named pipe, a socket or a plain file is a channel two
//!   processes may share by design, one writing and one reading, so it is a
//!   warning, as is a path that is not there to look at. A device -- a
//!   character or block device -- is an error, as tcspecial's own lock on
//!   it would refuse the second at its start.
//! * A CAN identifier or a raw IP protocol on one interface is a warning:
//!   every socket on one is handed every frame, so each tcspecial sees all
//!   of it rather than part, but both may send.
//!
//! Everything else held by both is an error.

use std::path::{Path, PathBuf};

use super::check::{claim_text, claims_of, is_opened, Claim, Severity};
use super::sockets::{self, BoundBy, Kind, Socket};
use super::Graph;
use crate::sim_config::SimConfigFile;
use crate::types::{EndpointConfig, NetworkProtocol};
use crate::About;

/// Which of the two files given for a configuration something is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    /// The configuration, or the file it includes that the node is in.
    Configuration,
    /// The simulator file.
    Simulator,
}

/// What is held.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Thing {
    /// A socket: see [`crate::graph::sockets`].
    Socket(Socket),
    /// What a node claims that is not a port: a device, a place on a bus, a
    /// line, an identifier. A port is a socket, and compared as one.
    Claim(Claim),
    /// A file or directory a configuration writes, as it really is where it
    /// can be found, and as written, with `.` and doubled `/` taken out,
    /// where it cannot.
    File(PathBuf),
}

/// One thing a configuration holds, and where it says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holding {
    /// What is held.
    thing: Thing,
    /// What it is, in the configuration's words.
    pub what: String,
    /// Which file says so.
    pub written: Written,
    /// What in that file says so, for finding its line.
    pub about: About,
}

/// A thing two configurations both hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// Which of the first configuration's holdings.
    pub first: usize,
    /// Which of the second's, which the conflict is reported against.
    pub second: usize,
    /// An error where the second could not start beside the first, or
    /// would take what the first is given; a warning where both can run
    /// and each sees all of what the other does.
    pub severity: Severity,
    /// What is wrong, naming both and what to do.
    pub why: String,
}

/// A path as it really is, links followed, where it is there; and as
/// written otherwise, with `.` and doubled separators taken out.
///
/// # Arguments
///
/// * `written` - The path as a configuration writes it.
///
/// # Returns
///
/// The path to compare.
fn real(written: &str) -> PathBuf {
    tcsos::fs::canonicalize(written)
        .unwrap_or_else(|_| Path::new(written).components().collect())
}

/// Lists what a configuration holds, with its simulator file where it is
/// given: every socket, every claim of every node that is opened, and each
/// file and directory it writes.
///
/// What does not read is left out, as [`sockets::of`] leaves it out: what is
/// wrong with it is said where the configuration is checked.
///
/// # Arguments
///
/// * `graph` - The configuration.
/// * `sim`   - Its simulator file, if one is given.
///
/// # Returns
///
/// What it holds.
pub fn holdings(graph: &Graph, sim: Option<&SimConfigFile>) -> Vec<Holding> {
    let mut held = Vec::new();
    let in_section = About::Section("tcspecial");

    for socket in sockets::of(graph) {
        let about = socket.node.clone().map_or(in_section.clone(), About::Node);
        held.push(Holding {
            what: format!(
                "{} at {}:{}",
                socket.what, socket.address, socket.port
            ),
            thing: Thing::Socket(socket),
            written: Written::Configuration,
            about,
        });
    }

    for node in graph.nodes.iter().filter(|node| is_opened(node)) {
        let Ok(reach) = super::reach::reach_of(node) else {
            continue;
        };
        for (claim, _) in claims_of(&node.name, &reach.endpoint) {
            // A port is among the sockets above.
            if matches!(claim, Claim::Port { .. }) {
                continue;
            }
            held.push(Holding {
                what: format!("\"{}\" on {}", node.name, claim_text(&claim)),
                thing: Thing::Claim(claim),
                written: Written::Configuration,
                about: About::Node(node.name.clone()),
            });
        }
    }

    let ci = graph.tcspecial.as_ref().and_then(|section| {
        super::parameters::ci_config_of(&super::parameters::flatten(section))
            .and_then(|json| json.to_ci_config())
            .ok()
    });
    if let Some(ci) = ci {
        let mut file = |attribute: &str, path: &str| {
            held.push(Holding {
                thing: Thing::File(real(path)),
                what: format!("{attribute} {path}"),
                written: Written::Configuration,
                about: in_section.clone(),
            });
        };
        file("state_file", &ci.state_file);
        if let Some(store) = &ci.configurations {
            file("configurations", store);
        }
        if let Some(log) = &ci.log_dir {
            file("log.directory", log);
        }
    }
    for node in graph.nodes.iter().filter(|node| node.node_type == "log") {
        if let Some(directory) = node.attributes.get("directory") {
            let directory = directory.as_str();
            held.push(Holding {
                thing: Thing::File(real(directory)),
                what: format!("log \"{}\"'s directory {directory}", node.name),
                written: Written::Configuration,
                about: About::Node(node.name.clone()),
            });
        }
    }

    if let Some(sim) = sim {
        simulated(graph, sim, &mut held);
    }
    held
}

/// Adds where tcs-sim answers, as a simulator file says: its `ready`
/// address and each simulated payload's own `payload_address`.
///
/// # Arguments
///
/// * `graph` - The configuration the simulator file is for.
/// * `sim`   - The simulator file.
/// * `held`  - Where each is added.
fn simulated(graph: &Graph, sim: &SimConfigFile, held: &mut Vec<Holding>) {
    if let Ok(ready) = sim.ready() {
        let socket = Socket {
            address: ready.ip().to_string(),
            port: ready.port(),
            protocol: NetworkProtocol::Udp,
            bound_by: BoundBy::Simulator,
            kind: Kind::SimAddress,
            what: "tcs_sim.ready".to_string(),
            node: None,
        };
        held.push(Holding {
            what: format!("tcs_sim.ready at {ready}"),
            thing: Thing::Socket(socket),
            written: Written::Simulator,
            about: About::Section("tcs_sim"),
        });
    }

    for payload in &sim.simulated_payloads {
        if !sim.simulates(&payload.name) {
            continue;
        }
        let Some(said) = sim.settings_of(payload).payload_address else {
            continue;
        };
        let protocol = graph
            .nodes
            .iter()
            .find(|node| node.name == payload.name)
            .and_then(|node| super::reach::reach_of(node).ok())
            .and_then(|reach| match reach.endpoint {
                EndpointConfig::Network(net) => Some(net.protocol),
                _ => None,
            });
        let about = About::SimPayload(payload.name.clone());
        let what = format!("\"{}\"'s payload_address {said}", payload.name);
        let thing = match protocol {
            Some(NetworkProtocol::UnixDgram) => {
                Thing::Claim(Claim::Path(said.clone()))
            }
            Some(protocol) => {
                // A host alone, which binds any port, claims none.
                let Ok(at) =
                    crate::types::host_port_of("payload_address", &said)
                else {
                    continue;
                };
                Thing::Socket(Socket {
                    address: at.address,
                    port: at.port,
                    protocol,
                    bound_by: BoundBy::Simulator,
                    kind: Kind::SimAddress,
                    what: format!("\"{}\"'s payload_address", payload.name),
                    node: Some(payload.name.clone()),
                })
            }
            None => continue,
        };
        held.push(Holding {
            thing,
            what,
            written: Written::Simulator,
            about,
        });
    }
}

/// How bad it is for two processes to hold one claim, and why.
///
/// # Arguments
///
/// * `claim` - The claim both hold.
///
/// # Returns
///
/// The severity, and what follows from sharing it.
fn shared(claim: &Claim) -> (Severity, &'static str) {
    match claim {
        Claim::Path(path) => match tcsos::fs::metadata(path) {
            Ok(meta) => {
                if tcsos::fs::is_device(&meta) {
                    (
                        Severity::Error,
                        "it is a device, which the second tcspecial to open \
                         it is refused, its lock held by the first",
                    )
                } else {
                    (
                        Severity::Warning,
                        "it is not a device, so both may open it: right for \
                         a pipe one writes and the other reads, and wrong \
                         for anything else",
                    )
                }
            }
            Err(_) => (
                Severity::Warning,
                "it is not on this machine to look at: a device would refuse \
                 the second tcspecial to open it, and a pipe one writes and \
                 the other reads would be right",
            ),
        },
        Claim::OnCanBus { .. } | Claim::Raw { .. } => (
            Severity::Warning,
            "each is handed every frame, so neither loses any, but both may \
             send under it",
        ),
        Claim::OnBus { .. } | Claim::OnChip { .. } | Claim::Port { .. } => (
            Severity::Error,
            "the second to open it is refused, the first holding it",
        ),
    }
}

/// Why two configurations cannot both have one socket.
///
/// # Arguments
///
/// * `one` - The first configuration's.
/// * `two` - The second's, which is the same socket.
///
/// # Returns
///
/// An error, and what follows: a payload's own address is one payload
/// both would command, and any other is one the second to bind is refused.
fn bound_twice(one: &Socket, two: &Socket) -> (Severity, &'static str) {
    if one.bound_by == BoundBy::Payload && two.bound_by == BoundBy::Payload {
        (Severity::Error, "both would command one payload")
    } else {
        (
            Severity::Error,
            "whichever binds it second fails as it starts",
        )
    }
}

/// Finds everything two configurations both hold.
///
/// # Arguments
///
/// * `one` - What the first holds, as [`holdings`] lists it.
/// * `two` - What the second holds, which each conflict is reported against.
///
/// # Returns
///
/// Each conflict, in the second's order.
pub fn between(one: &[Holding], two: &[Holding]) -> Vec<Conflict> {
    let mut found = Vec::new();
    for (second, theirs) in two.iter().enumerate() {
        for (first, ours) in one.iter().enumerate() {
            let judged = match (&ours.thing, &theirs.thing) {
                (Thing::Socket(a), Thing::Socket(b)) if a.is(b) => {
                    Some(bound_twice(a, b))
                }
                (Thing::Claim(a), Thing::Claim(b)) if a == b => Some(shared(a)),
                (Thing::File(a), Thing::File(b)) if a == b => Some((
                    Severity::Error,
                    "each would write over what the other keeps there",
                )),
                _ => None,
            };
            if let Some((severity, because)) = judged {
                found.push(Conflict {
                    first,
                    second,
                    severity,
                    why: if theirs.what == ours.what {
                        format!(
                            "both configurations have {}: {because}. Give \
                             one of them another",
                            theirs.what
                        )
                    } else {
                        format!(
                            "{} is {} in the other configuration: \
                             {because}. Give one of them another",
                            theirs.what, ours.what
                        )
                    },
                });
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shipped example, loaded.
    fn example(name: &str) -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
        Graph::load(root.join(name)).expect("the example loads")
    }

    #[test]
    fn a_configuration_conflicts_with_itself_everywhere() {
        let graph = example("minimal/minimal.ytcs");
        let held = holdings(&graph, None);
        assert!(
            held.iter().any(|one| one.what.starts_with("state_file")),
            "{held:#?}"
        );
        let found = between(&held, &held);
        // Everything it binds or writes is held twice, so every socket
        // that claims a port and every file is found.
        assert!(
            found.iter().any(|c| c.why.contains("state_file")),
            "{found:#?}"
        );
        assert!(found.iter().any(|c| c.why.contains(":4000")), "{found:#?}");
        assert!(found.iter().all(|c| c.why.contains("Give one of them")));
    }

    #[test]
    fn two_configurations_that_share_nothing_do_not_conflict() {
        let one = holdings(&example("minimal/minimal.ytcs"), None);
        let two: Vec<Holding> = one
            .iter()
            .cloned()
            .map(|mut held| {
                held.thing = match held.thing {
                    // Well clear of every port the example has: a step of
                    // one would land one port on the next, as the gateway's
                    // 5000 on the sensor's 5001.
                    Thing::Socket(mut socket) => {
                        socket.port = socket.port.wrapping_add(10_000);
                        Thing::Socket(socket)
                    }
                    Thing::File(path) => Thing::File(path.join("other")),
                    claim => claim,
                };
                held
            })
            .collect();
        let found = between(&one, &two);
        assert!(
            found
                .iter()
                .all(|c| matches!(two[c.second].thing, Thing::Claim(_))),
            "{found:#?}"
        );
    }

    #[test]
    fn a_shared_device_is_an_error_and_a_shared_file_a_warning() {
        let file = tempfile::NamedTempFile::new().expect("a file");
        let path = file.path().display().to_string();
        assert_eq!(shared(&Claim::Path(path)).0, Severity::Warning);
        assert_eq!(
            shared(&Claim::Path("/dev/null".to_string())).0,
            Severity::Error
        );
        assert_eq!(
            shared(&Claim::OnBus {
                bus: "/dev/i2c-1".to_string(),
                address: 0x48
            })
            .0,
            Severity::Error
        );
        assert_eq!(
            shared(&Claim::OnCanBus {
                interface: "can0".to_string(),
                can_id: 0x100
            })
            .0,
            Severity::Warning
        );
    }
}
