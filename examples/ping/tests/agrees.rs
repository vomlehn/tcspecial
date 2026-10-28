//! What the Python reads out of the configuration is what the Rust reads.
//!
//! The ground station is Python and reads the XML spelling, the standard
//! library having no YAML; the vehicle and every Rust ground program read
//! through `tcslibgs`. Where the beacons are and how often they come, where a
//! ping goes up and where its answer comes down are worked out on both sides,
//! so this holds the Python's answers for `ping.xml` to the Rust's for
//! `ping.yaml` -- which is also the two spellings being held to one another.

use std::path::{Path, PathBuf};
use std::process::Command;

use tcslibgs::config_digest::digest_of_file;
use tcslibgs::graph::parameters::{beacon_interval_of, beacon_of};
use tcslibgs::graph::reach::reach_of;
use tcslibgs::graph::telemetry::{self, Via};
use tcslibgs::graph::Graph;
use tcslibgs::EndpointConfig;

fn here() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// What `ping_moc.py --describe` says, as `name value` pairs.
fn described() -> Vec<(String, String)> {
    let ran = Command::new("python3")
        .arg(here().join("ping_moc.py"))
        .arg("--describe")
        .arg(here().join("ping.xml"))
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

/// The Rust's answers to the same questions, from the YAML spelling.
fn worked_out() -> Vec<(String, String)> {
    let path = here().join("ping.yaml");
    let graph = Graph::load(&path).expect("the example reads");

    let link = beacon_of(&graph)
        .expect("its section reads")
        .expect("it has one");

    // In nanoseconds, as the Python keeps it.
    let interval = beacon_interval_of(&graph)
        .expect("its section reads")
        .expect("it has one")
        .as_nanos();

    let moc = graph
        .nodes
        .iter()
        .find(|node| node.name == "moc")
        .expect("a moc node");
    let uplink = match reach_of(moc).expect("it is reached").endpoint {
        EndpointConfig::Network(network) => {
            format!("{}:{}", network.address, network.port)
        }
        other => panic!("a moc node is a socket: {other:?}"),
    };

    let downlink = telemetry::of(&graph)
        .expect("its links to the ground work out")
        .into_iter()
        .find(|link| {
            link.source == "ping" && link.via == Via::Moc("moc".to_string())
        })
        .map(|link| format!("{}:{}", link.ground.address, link.ground.port))
        .expect("ping sends to the moc node");

    vec![
        ("beacon.send".to_string(), link.send.to_string()),
        ("beacon.recv".to_string(), link.recv.to_string()),
        ("beacon.default_interval".to_string(), interval.to_string()),
        ("uplink".to_string(), uplink),
        ("downlink".to_string(), downlink),
        (
            "digest".to_string(),
            digest_of_file(&path).expect("it digests").to_string(),
        ),
    ]
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
