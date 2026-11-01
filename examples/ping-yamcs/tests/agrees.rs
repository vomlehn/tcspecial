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

use std::path::{Path, PathBuf};
use std::process::Command;

use tcslibgs::config_digest::digest_of_file;
use tcslibgs::graph::parameters::{beacon_of, ci_config_of, flatten};
use tcslibgs::graph::reach::{apid_base_of, reach_of};
use tcslibgs::graph::telemetry::{self, Via};
use tcslibgs::graph::Graph;
use tcslibgs::EndpointConfig;

fn here() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

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

/// The address to reach a socket at from this host, as `ping_yamcs.py`'s
/// `local` gives it: Yamcs, on this host, reaches a socket bound on every
/// interface at this host's own address.
///
/// # Arguments
///
/// * `address` - The address the socket is bound at.
///
/// # Returns
///
/// The address to send to.
fn local(address: &str) -> String {
    match address {
        "0.0.0.0" | "::" | "" => "127.0.0.1".to_string(),
        other => other.to_string(),
    }
}

/// The Rust's answers to the same questions, from the YAML spelling.
///
/// # Returns
///
/// The ports, the APIDs and the digest, as `ping_yamcs.py --describe` names
/// them. Not the settings that are Yamcs's own -- its web port and its
/// directories -- which the configuration does not say.
///
/// # Panics
///
/// The example's configuration does not read or lacks any of these.
fn worked_out() -> Vec<(String, String)> {
    let path = here().join("ping-yamcs.ytcs");
    let graph = Graph::load(&path).expect("the example reads");

    let section = graph.tcspecial.as_ref().expect("a tcspecial section");
    let ci = ci_config_of(&flatten(section))
        .expect("its section reads")
        .to_ci_config()
        .expect("its section settles");
    let sockets = ci.sockets().expect("its links are sockets");
    let framing = sockets.framing.expect("its packets are framed");

    let beacon = beacon_of(&graph)
        .expect("its section reads")
        .expect("it has one");

    // The one gateway_yamcs node and the one ping node, as the Python finds
    // them: by their types.
    let one_of = |kind: &str| {
        let mut found =
            graph.nodes.iter().filter(|node| node.node_type == kind);
        let node = found.next().unwrap_or_else(|| panic!("a {kind} node"));
        assert!(found.next().is_none(), "one {kind} node");
        node
    };
    let gateway = one_of("gateway_yamcs");
    let ping = one_of("ping");

    let uplink = match reach_of(gateway).expect("it is reached").endpoint {
        EndpointConfig::Network(network) => network,
        other => panic!("a gateway node is a socket: {other:?}"),
    };
    let uplink_apid = apid_base_of(gateway)
        .expect("its apid_base reads")
        .expect("it has one");

    let answer = telemetry::of(&graph)
        .expect("its links to the ground work out")
        .into_iter()
        .find(|link| {
            link.source == ping.name
                && link.via == Via::Gateway(gateway.name.clone())
        })
        .expect("ping sends to the gateway node");
    let downlink = answer.ground().expect("a link over UDP").clone();
    let reply_apid = answer.apid.expect("its packets carry an APID");

    [
        ("cmd_host", local(&sockets.address)),
        ("cmd_port", sockets.port.to_string()),
        ("uplink_host", local(&uplink.address)),
        ("uplink_port", uplink.port.to_string()),
        ("downlink_port", downlink.port.to_string()),
        ("beacon_port", beacon.recv.port.to_string()),
        ("beacon_apid", framing.beacon_apid.to_string()),
        ("cmd_apid", framing.cmd_apid.to_string()),
        ("uplink_apid", uplink_apid.to_string()),
        ("reply_apid", reply_apid.to_string()),
        (
            "digest",
            digest_of_file(&path).expect("it digests").to_string(),
        ),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_string(), value))
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
