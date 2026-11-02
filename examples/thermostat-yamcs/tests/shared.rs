//! The thermostat example and this one fly the same vehicle, and say so.
//!
//! What the two share that a file can refer to is shared by reference: the
//! physics model, `examples/thermostat-common/thermostat-physics.yaml`,
//! which both simulator files name. The rest cannot be. A simulator file
//! includes no other, and the configurations are read whole by the Python
//! ground station, which reads no includes -- and a kept configuration's
//! includes are names beside it, not paths into another example. So the
//! vehicle and its payloads are written in both examples, and the simulator
//! files' stand-ins in both, and this holds each copy to the other: the
//! same nodes, links, groups and commands, but the gateway node, which is a
//! radio there and Yamcs here, the `tcspecial` section, and the ports the
//! payloads are at, which differ so that the two can run at once; and the
//! same stand-ins and physics, but where tcssim answers.

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
        thermostat["nodes"].as_array().expect("nodes").len() == 11,
        "{thermostat}"
    );
}

/// A simulator file as its digest reads it, with the physics model it names
/// read in, less where tcssim answers.
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
    let model = file.physics_path(path).expect("it names its physics");
    assert_eq!(
        model.canonicalize().expect("the model is there"),
        examples()
            .join("thermostat-common/thermostat-physics.yaml")
            .canonicalize()
            .expect("the shared model is there"),
        "{} names the shared physics",
        path.display()
    );
    let mut tree = interpretation_of_simulator(&file);
    let section = tree["tcssim"].as_object_mut().expect("a tcssim section");
    assert!(
        section.remove("ready").is_some(),
        "it says where it answers"
    );
    tree
}

/// The two simulator files stand in for the same payloads, on the same
/// physics -- the one file both name.
#[test]
fn the_simulators_are_the_same() {
    let thermostat =
        simulated_by(&examples().join("thermostat/thermostat-sim.yaml"));
    let yamcs = simulated_by(
        &examples().join("thermostat-yamcs/thermostat-yamcs-sim.yaml"),
    );
    assert_eq!(thermostat, yamcs);
    assert!(thermostat["physics"].is_object(), "{thermostat}");
}
