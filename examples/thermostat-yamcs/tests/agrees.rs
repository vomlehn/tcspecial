//! What the Python reads out of the configuration is what the Rust reads.
//!
//! The ground station is Python and reads the XML spelling; the vehicle reads
//! the configuration through `tcslibgs`. `thermostat_yamcs.py` writes Yamcs's
//! configuration from what it reads -- where Yamcs sends commands, where it
//! listens for each source's data and the beacon, the APIDs that tell them
//! apart -- and its mission database gives each source's parameter the type
//! the configuration says its data are, by `tcslibgs/form.py`. A port, an
//! APID or a form the two sides work out differently is a Yamcs that listens
//! where nothing arrives, or reads a temperature as something else. So this
//! holds the Python's answers for `thermostat-yamcs.xtcs` to the Rust's for
//! `thermostat-yamcs.ytcs`, and the Python's form of every node of every
//! example to `tcslibgs::graph::fprime::Form::of`, which tcsmoc's plot
//! window and the F´ bridge go by, so that the two cannot drift.

use std::path::{Path, PathBuf};
use std::process::Command;

use tcslibgs::config_digest::digest_of_file;
use tcslibgs::graph::fprime::Form;
use tcslibgs::graph::parameters::{beacon_of, ci_config_of, flatten};
use tcslibgs::graph::reach::apid_base_of;
use tcslibgs::graph::telemetry::{self, Via};
use tcslibgs::graph::Graph;
use tcslibgs::unit::Unit;

/// Where the example is.
///
/// # Returns
///
/// Its directory.
fn here() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// What `thermostat_yamcs.py --describe` says, as `name value` pairs.
///
/// # Returns
///
/// Each line's name and the rest of it, in the order printed.
///
/// # Panics
///
/// python3 cannot be run, the program fails, or a line is not a name and a
/// value.
fn described() -> Vec<(String, String)> {
    let ran = Command::new("python3")
        .arg(here().join("thermostat_yamcs.py"))
        .arg("--describe")
        .arg(here().join("thermostat-yamcs.xtcs"))
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

/// The address to reach a socket at from this host, as the Python's
/// `tcslibgs.yamcs.local` gives it.
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
/// The settings, each source drawn into the gateway node as `source` with
/// its name, APID, port and form, and the digest, as `thermostat_yamcs.py
/// --describe` writes them. Not the settings that are Yamcs's own -- its
/// web port and its directory -- which the configuration does not say.
///
/// # Panics
///
/// The example's configuration does not read or lacks any of these.
fn worked_out() -> Vec<(String, String)> {
    let path = here().join("thermostat-yamcs.ytcs");
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
    let gateway = graph
        .nodes
        .iter()
        .find(|node| node.node_type == "gateway_yamcs")
        .expect("a gateway_yamcs node");
    let uplink_apid = apid_base_of(gateway)
        .expect("its apid_base reads")
        .expect("it has one");

    let mut said = vec![
        ("cmd_host".to_string(), local(&sockets.address)),
        ("cmd_port".to_string(), sockets.port.to_string()),
        ("beacon_port".to_string(), beacon.recv.port.to_string()),
        ("beacon_apid".to_string(), framing.beacon_apid.to_string()),
        ("cmd_apid".to_string(), framing.cmd_apid.to_string()),
        ("uplink_apid".to_string(), uplink_apid.to_string()),
    ];
    for link in telemetry::of(&graph).expect("its links work out") {
        if link.via != Via::Gateway(gateway.name.clone()) {
            continue;
        }
        said.push((
            "source".to_string(),
            format!(
                "{} {} {} {:?}",
                link.source,
                link.apid.expect("framed"),
                link.ground().expect("over UDP").port,
                Form::of(&graph, &link.source)
            ),
        ));
    }
    said.push((
        "digest".to_string(),
        digest_of_file(&path).expect("it digests").to_string(),
    ));
    said
}

#[test]
fn the_python_reads_the_configuration_as_the_rust_does() {
    let python = described();
    let rust = worked_out();
    let ours: Vec<&(String, String)> = python
        .iter()
        .filter(|(name, _)| name != "http_port" && name != "run_dir")
        .collect();
    assert_eq!(ours.len(), rust.len(), "{python:?}\n{rust:?}");
    for ((name, mine), (said, theirs)) in ours.into_iter().zip(&rust) {
        assert_eq!(name, said, "the Python and the Rust say different things");
        // The digest is tcsverify's, which the Python asks for and does not
        // work out: a build that has not made tcsverify yet leaves it to say
        // so, and there is then nothing of the Python's to compare.
        if name == "digest" && mine == "None" {
            eprintln!(
                "tcsverify is not built, so the Python had no digest to \
                 compare"
            );
            continue;
        }
        assert_eq!(mine, theirs, "{name}: the Python and the Rust disagree");
    }
    // What the example is for: temperatures, and the levels of the lines
    // sensing its two heaters.
    let forms: Vec<&str> = rust
        .iter()
        .filter(|(name, _)| name == "source")
        .map(|(_, said)| said.rsplit(' ').next().expect("a form"))
        .collect();
    assert_eq!(
        forms,
        [
            "Value(Kelvin)",
            "Lines(2)",
            "Value(Kelvin)",
            "Value(Kelvin)",
            "Value(Kelvin)",
        ]
    );
}

/// The examples' configurations, both spellings.
///
/// # Returns
///
/// Each example's `.xtcs` and `.ytcs`, sorted: every directory of
/// `examples/` that has them, the common ones having none.
fn every_example() -> Vec<(PathBuf, PathBuf)> {
    let examples = here().join("..");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&examples).expect("the examples read") {
        let dir = entry.expect("an entry").path();
        let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let xml = dir.join(format!("{name}.xtcs"));
        let yaml = dir.join(format!("{name}.ytcs"));
        if xml.is_file() && yaml.is_file() {
            found.push((xml, yaml));
        }
    }
    found.sort();
    assert!(found.len() > 10, "the examples were not found: {found:?}");
    found
}

/// The Python's form of every node of a configuration.
///
/// # Arguments
///
/// * `path` - The configuration, in XML.
///
/// # Returns
///
/// One line per node: its name and its form, as `tcslibgs/form.py` writes
/// it, sorted.
fn python_forms(path: &Path) -> Vec<String> {
    let said = yamcs_common::python(
        "from tcslibgs import configuration, form\n\
         said = configuration.load(sys.argv[1])\n\
         for node in said.nodes:\n\
         \x20   print(node.name, form.form_of(said, node.name))\n",
        &[],
        &[path],
    );
    let mut lines: Vec<String> = said.lines().map(str::to_string).collect();
    lines.sort();
    lines
}

/// The form of every node of every example is what `Form::of` says: the
/// readings and votes in their units -- kelvin, volts, amperes, metres and
/// the rest -- the thermostats' and the battery's decisions, and every other
/// node's bytes. And a node nobody has is bytes too.
#[test]
fn every_nodes_form_is_the_rusts() {
    let mut values = 0;
    for (xml, yaml) in every_example() {
        let graph = Graph::load(&yaml).expect("the example reads");
        let mut theirs: Vec<String> = graph
            .nodes
            .iter()
            .map(|node| {
                format!("{} {:?}", node.name, Form::of(&graph, &node.name))
            })
            .collect();
        theirs.sort();
        values += theirs.iter().filter(|line| line.contains("Value")).count();
        assert_eq!(python_forms(&xml), theirs, "{}", xml.display());
    }
    assert!(values > 10, "too few values to say anything: {values}");
    let said = yamcs_common::python(
        "from tcslibgs import configuration, form\n\
         said = configuration.load(sys.argv[1])\n\
         print(form.form_of(said, 'nobody'))\n",
        &[],
        &[&here().join("thermostat-yamcs.xtcs")],
    );
    assert_eq!(said.trim(), format!("{:?}", Form::Datum));
}

/// The Python's table of units is `tcslibgs::unit`'s: each unit, in order,
/// with the same name, symbol, words and steps.
#[test]
fn the_units_are_the_rusts() {
    let said = yamcs_common::python(
        "from tcslibgs import form\n\
         for unit in form.UNITS:\n\
         \x20   print(unit.variant, unit.symbol, unit.per_unit, unit.name,\n\
         \x20         sep='|')\n",
        &[],
        &[],
    );
    let theirs: Vec<String> = Unit::ALL
        .iter()
        .map(|unit| {
            format!(
                "{unit:?}|{}|{:?}|{}",
                unit.symbol(),
                unit.per_unit(),
                unit.name()
            )
        })
        .collect();
    assert_eq!(said.lines().collect::<Vec<_>>(), theirs);
}
