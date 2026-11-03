//! The thermostat example and this one fly the same vehicle, and say so.
//!
//! What the two share that a file can refer to is shared by reference: the
//! vehicle and its payloads, `examples/thermostat-common/thermostat.ytcs`
//! and its XML, which both configurations include, and the physics model,
//! `examples/thermostat-common/thermostat-physics.yaml`, which both
//! simulator files name. The rest cannot be. A simulator file includes no
//! other, so the simulator files' stand-ins are written in both, and this
//! holds each copy to the other: the same stand-ins and physics, but where
//! tcssim answers.

use std::path::{Path, PathBuf};

use serde_json::Value;

use tcslibgs::config_digest::interpretation_of_simulator;
use tcslibgs::sim_config::SimConfigFile;

/// Where the examples are.
///
/// # Returns
///
/// The `examples` directory.
fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
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
