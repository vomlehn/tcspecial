//! What the Python reads out of the configuration is what the Rust reads.
//!
//! The ground station is Python and reads the XML spelling; the vehicle reads
//! the configuration through `tcslibgs`. `ping_yamcs.py` writes Yamcs's
//! configuration from what it reads -- where Yamcs sends commands and pings,
//! where it listens for the answers and the beacon, and the APIDs that tell
//! them apart -- so a port or an APID the two sides work out differently is a
//! Yamcs that sends where the vehicle is not listening, or reads an answer as
//! something else. This holds the Python's answers for `ping-yamcs.xtcs` to
//! the Rust's for `ping-yamcs.ytcs`, which is also the two spellings being
//! held to one another.

use std::path::Path;
use std::process::Command;

use std::collections::BTreeSet;

use tcslibgs::graph::telemetry::{self, ApidCarries, Via};
use tcslibgs::graph::{Destination, Graph};

mod common;

use common::{
    generated_mdb, here, read_xml, with_more_nodes, worked_out, Element,
};

/// What `ping_yamcs.py --describe` says, as `name value` pairs.
///
/// # Returns
///
/// Each line's name and value, in the order printed.
///
/// # Panics
///
/// python3 cannot be run, the program fails, or a line is not a name and a
/// value.
fn described() -> Vec<(String, String)> {
    let ran = Command::new("python3")
        .arg(here().join("ping_yamcs.py"))
        .arg("--describe")
        .arg(here().join("ping-yamcs.xtcs"))
        .output()
        .expect("python3 runs");
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    String::from_utf8(ran.stdout)
        .expect("text")
        .lines()
        .map(|line| {
            let (name, value) =
                line.split_once(' ').expect("a name and a value");
            (name.to_string(), value.to_string())
        })
        .collect()
}

#[test]
fn the_python_reads_the_configuration_as_the_rust_does() {
    let python = described();
    let rust = worked_out();

    for (name, theirs) in &rust {
        let ours = python
            .iter()
            .find(|(said, _)| said == name)
            .map(|(_, value)| value)
            .unwrap_or_else(|| {
                panic!("the Python says nothing of {name}: {python:?}")
            });
        // The digest is tcsverify's, which the Python asks for and does not
        // work out: a build that has not made tcsverify yet leaves it to say
        // so, and there is then nothing of the Python's to compare.
        if name == "digest" && ours == "None" {
            eprintln!(
                "tcsverify is not built, so the Python had no digest to \
                 compare"
            );
            continue;
        }
        assert_eq!(ours, theirs, "{name}: the Python and the Rust disagree");
    }
}

/// The payloads nothing touches in `path`, each with the APID its telemetry
/// carries and where it leaves and arrives, as the Python reads them.
///
/// # Arguments
///
/// * `path` - The configuration, in XML.
///
/// # Returns
///
/// One line per payload: its name, its APID, and the ground's and the
/// vehicle's address and port.
///
/// # Panics
///
/// python3 cannot be run, or the Python fails.
fn python_payload_telemetry(path: &Path) -> Vec<String> {
    let python = here().join("../../python");
    let script = "import sys\n\
        sys.path.insert(0, sys.argv[1])\n\
        from tcslibgs import configuration\n\
        said = configuration.load(sys.argv[2])\n\
        for name in said.untouched_payloads():\n\
        \x20   vehicle, ground = said.payload_telemetry_link(name)\n\
        \x20   print(name, said.payload_telemetry_apid(name), ground.address,\n\
        \x20         ground.port, vehicle.address, vehicle.port)\n";
    let ran = Command::new("python3")
        .arg("-c")
        .arg(script)
        .arg(&python)
        .arg(path)
        .output()
        .expect("python3 runs");
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    String::from_utf8(ran.stdout)
        .expect("text")
        .lines()
        .map(str::to_string)
        .collect()
}

/// A payload nothing touches, under Yamcs, reports framed under
/// `payload_telemetry.apid_base` plus its device id, and the Python works
/// out the APID, and the ports, as the Rust does. The example has no such
/// payload, so two are added to a copy of it: "sensor" and "tank", whose
/// names sort after "gateway" and "ping", device ids 2 and 3.
#[test]
fn the_python_counts_payload_telemetry_apids_as_the_rust_does() {
    let text = std::fs::read_to_string(here().join("ping-yamcs.xtcs"))
        .expect("the example");
    let payloads = "    <payloads>\n\
        \x20       <type name=\"network\">\n\
        \x20           <payload name=\"sensor\">\n\
        \x20               <address>127.0.0.1</address>\n\
        \x20               <port>16801</port>\n\
        \x20               <protocol>udp</protocol>\n\
        \x20           </payload>\n\
        \x20           <payload name=\"tank\">\n\
        \x20               <address>127.0.0.1</address>\n\
        \x20               <port>16802</port>\n\
        \x20               <protocol>udp</protocol>\n\
        \x20           </payload>\n\
        \x20       </type>\n\
        \x20   </payloads>\n\
        \x20   <tcsgui>";
    assert_eq!(text.matches("    <tcsgui>").count(), 1, "one tcsgui");
    let text = text.replace("    <tcsgui>", payloads);
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("with-payloads.xtcs");
    std::fs::write(&path, text).expect("written");

    let graph = Graph::load(&path).expect("the copy reads");
    assert_eq!(graph.check(), Vec::new(), "the copy is sound");
    let rust: Vec<String> = telemetry::of(&graph)
        .expect("its links to the ground work out")
        .into_iter()
        .filter(|link| link.via == Via::PayloadTelemetry)
        .map(|link| {
            let ground = link.ground().expect("a link over UDP");
            let vehicle = link.vehicle().expect("a link over UDP");
            format!(
                "{} {} {} {} {} {}",
                link.source,
                link.apid.expect("framed"),
                ground.address,
                ground.port,
                vehicle.address,
                vehicle.port
            )
        })
        .collect();
    assert_eq!(
        rust,
        [
            "sensor 770 127.0.0.1 16412 0.0.0.0 16402",
            "tank 771 127.0.0.1 16413 0.0.0.0 16403",
        ],
        "0x300 plus 2 and 3"
    );
    assert_eq!(python_payload_telemetry(&path), rust);
}

/// Every APID the generated wrapper restricts a container to or assigns a
/// command, with the container's or the command's name.
///
/// # Arguments
///
/// * `xtce` - The wrapper.
///
/// # Returns
///
/// Each `(name, apid)`.
fn mdb_apids(xtce: &Element) -> BTreeSet<(String, u16)> {
    let number = |said: &str| said.parse::<u16>().expect("an APID");
    let mut found = BTreeSet::new();
    let mut containers = Vec::new();
    xtce.within("SequenceContainer", &mut containers);
    for container in containers {
        let mut comparisons = Vec::new();
        if let Some(base) = container.child("BaseContainer") {
            base.within("Comparison", &mut comparisons);
        }
        for comparison in comparisons {
            if comparison.attribute("parameterRef") == Some("CcsdsApid") {
                found.insert((
                    container.attribute("name").expect("a name").to_string(),
                    number(comparison.attribute("value").expect("a value")),
                ));
            }
        }
    }
    let mut commands = Vec::new();
    xtce.within("MetaCommand", &mut commands);
    for command in commands {
        let mut assignments = Vec::new();
        command.within("ArgumentAssignment", &mut assignments);
        for assigned in assignments {
            if assigned.attribute("argumentName") == Some("CcsdsApid") {
                found.insert((
                    command.attribute("name").expect("a name").to_string(),
                    number(
                        assigned.attribute("argumentValue").expect("a value"),
                    ),
                ));
            }
        }
    }
    found
}

/// The command the ping node's interface document, `tcslibgs/xtce/ping.xtce`,
/// states: what a gateway node whose data goes only to ping nodes takes.
///
/// # Returns
///
/// Its name.
///
/// # Panics
///
/// The document does not read, or states no one command.
fn ping_command() -> String {
    let path = here().join("../../tcslibgs/xtce/ping.xtce");
    let document = read_xml(&std::fs::read_to_string(path).expect("it reads"));
    let mut commands = Vec::new();
    document.within("MetaCommand", &mut commands);
    let [command] = commands.as_slice() else {
        panic!("ping.xtce states one command");
    };
    command.attribute("name").expect("a name").to_string()
}

/// The command the ground sends a gateway node by: the ping node's
/// document's, where every node its data goes to is a ping node, and else
/// one named after the node, which carries its data whole.
///
/// # Arguments
///
/// * `graph`   - The configuration's graph.
/// * `gateway` - The gateway node's name.
///
/// # Returns
///
/// The command's name.
fn uplink_command(graph: &Graph, gateway: &str) -> String {
    let mut types = BTreeSet::new();
    for link in &graph.links {
        if link.origin().node != gateway {
            continue;
        }
        for destination in link.to.each() {
            let node = Destination::parse(destination).node;
            let node = graph.node(&node).expect("a node it reaches");
            types.insert(node.node_type.clone());
        }
    }
    if types.len() == 1 && types.contains("ping") {
        ping_command()
    } else {
        gateway.to_string()
    }
}

/// What the Rust says the wrapper for `path` must have: for every APID
/// `telemetry::apids` counts, the container or the command named for what
/// it carries -- the beacon's and the answers' containers and CONNECT, a
/// command for each gateway node (see [`uplink_command`]), and a container
/// named after each node whose data reaches the ground.
///
/// # Arguments
///
/// * `path` - The configuration.
///
/// # Returns
///
/// Each `(name, apid)`.
///
/// # Panics
///
/// The configuration does not read, or its APIDs do not work out.
fn rust_apids(path: &Path) -> BTreeSet<(String, u16)> {
    let graph = Graph::load(path).expect("the configuration reads");
    assert_eq!(graph.check(), Vec::new(), "{} is sound", path.display());
    let links = telemetry::of(&graph).expect("its links work out");
    let source = |name: &str| {
        let link = links
            .iter()
            .find(|link| link.name == name)
            .unwrap_or_else(|| panic!("no link {name}"));
        format!("{}Packet", link.source)
    };
    let mut expected = BTreeSet::new();
    for used in telemetry::apids(&graph).expect("its APIDs work out") {
        let names = match &used.carries {
            ApidCarries::Beacon => vec!["BeaconPacket".to_string()],
            ApidCarries::Commands => {
                vec!["AnswerPacket".to_string(), "Connect".to_string()]
            }
            ApidCarries::Uplink(gateway) => {
                vec![uplink_command(&graph, gateway)]
            }
            ApidCarries::Downlink { link, .. } => vec![source(link)],
            ApidCarries::PayloadTelemetry(link) => vec![source(link)],
        };
        expected.extend(names.into_iter().map(|name| (name, used.apid)));
    }
    expected
}

/// The wrapper `tcslibgs/yamcs.py` generates holds exactly the APIDs the
/// Rust counts, each in the container or command named for it: for the
/// example, and for a copy with a payload more drawn into the gateway node
/// and two payloads nothing touches, whose containers no hand wrote.
#[test]
fn the_generated_wrapper_has_every_apid_the_rust_counts() {
    let example = here().join("ping-yamcs.xtcs");
    assert_eq!(
        mdb_apids(&generated_mdb(&example)),
        rust_apids(&here().join("ping-yamcs.ytcs"))
    );

    let dir = tempfile::tempdir().expect("a directory");
    let more = with_more_nodes(dir.path());
    let theirs = rust_apids(&more);
    assert_eq!(mdb_apids(&generated_mdb(&more)), theirs);
    for added in ["pongPacket", "sensorPacket", "tankPacket"] {
        assert!(
            theirs.iter().any(|(name, _)| name == added),
            "{added} is counted: {theirs:?}"
        );
    }
}
