//! The thermostat example and this one fly the same vehicle, and say so.
//!
//! The simulator file is shared by reference: each example's is a link to
//! `examples/thermostat-common/thermostat-sim.yaml`, so that each is found
//! beside its configuration by name, as every simulator file is. The
//! configurations cannot be. They are read whole by the Python ground
//! station, which reads no includes -- and a kept configuration's includes
//! are names beside it, not paths into another example. So the vehicle and
//! its payloads are written in both examples, and this holds each copy to
//! the other: the same nodes, links, groups and commands, but the gateway
//! node, which is a radio there and Yamcs here, the `tcspecial` section,
//! and the ports the payloads are at.
//!
//! The manual's fixture, `tests/manual/thermostat-sim.yaml`, is the same
//! vehicle with its payloads on buses, so its stand-ins differ; its physics
//! is a copy of the shared file's, and is held to it.

use std::path::{Path, PathBuf};

use serde_json::Value;

use tcslibgs::config_digest::{
    interpretation_of_file, interpretation_of_simulator,
};
use tcslibgs::sim_config::SimConfigFile;

/// Where the examples are.
///
/// # Returns
///
/// The `examples` directory.
fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// A configuration as its digest reads it, less what the two examples are
/// meant to differ in.
///
/// # Arguments
///
/// * `path` - The configuration.
///
/// # Returns
///
/// Its value tree, without its `tcspecial` section, its gateway node, or
/// any payload's port.
fn vehicle_of(path: &Path) -> Value {
    let mut tree = interpretation_of_file(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let said = tree.as_object_mut().expect("a tree");
    said.remove("tcspecial");
    let nodes = said["nodes"].as_array_mut().expect("nodes");
    nodes.retain(|node| node["name"] != "gateway");
    for node in nodes {
        if node["type"] == "network" {
            let attributes = node["attributes"].as_object_mut().expect("some");
            assert!(attributes.remove("port").is_some(), "{node}");
        }
    }
    tree
}

/// The two configurations are one vehicle, with one gateway node differing.
#[test]
fn the_vehicles_are_the_same() {
    let thermostat = vehicle_of(&examples().join("thermostat/thermostat.ytcs"));
    let yamcs =
        vehicle_of(&examples().join("thermostat-yamcs/thermostat-yamcs.ytcs"));
    assert_eq!(thermostat, yamcs);
    assert!(
        thermostat["nodes"].as_array().expect("nodes").len() == 12,
        "{thermostat}"
    );
}

/// The two examples' simulator files are links to one file, the shared
/// one in thermostat-common.
#[test]
fn the_simulator_files_are_one_file() {
    let shared = examples()
        .join("thermostat-common/thermostat-sim.yaml")
        .canonicalize()
        .expect("the shared simulator file is there");
    for name in [
        "thermostat/thermostat-sim.yaml",
        "thermostat-yamcs/thermostat-yamcs-sim.yaml",
    ] {
        let path = examples().join(name);
        let link = std::fs::symlink_metadata(&path)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(link.file_type().is_symlink(), "{name} is a link");
        assert_eq!(
            path.canonicalize().expect("it resolves"),
            shared,
            "{name} is the shared file"
        );
    }
}

/// A simulator file as its digest reads it.
///
/// # Arguments
///
/// * `path` - The simulator file.
///
/// # Returns
///
/// Its value tree.
fn simulated_by(path: &Path) -> Value {
    let file = SimConfigFile::load(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    interpretation_of_simulator(&file)
}

/// The manual's fixture has the same world as the shared simulator file:
/// the same ambient temperature, components, figures and connections,
/// however each file writes them.
#[test]
fn the_manuals_fixture_has_the_same_physics() {
    let shared =
        simulated_by(&examples().join("thermostat-common/thermostat-sim.yaml"));
    let manual =
        simulated_by(&examples().join("../tests/manual/thermostat-sim.yaml"));
    assert!(shared["world"].is_object(), "{shared}");
    assert_eq!(shared["world"], manual["world"]);
}
