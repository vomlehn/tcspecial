//! The thermostat example and this one fly the same vehicle, and say so.
//!
//! What the two share is shared by reference. The vehicle and its payloads
//! are `examples/thermostat-common/thermostat.ytcs` and its XML, which both
//! configurations include. The simulator file, physics model and all, is
//! `examples/thermostat-common/thermostat-sim.yaml`: each example's is a link
//! to it, so that each is found beside its configuration by name, as every
//! simulator file is. This holds the links to that file.
//!
//! The manual's fixture, `tests/manual/thermostat-sim.yaml`, is the same
//! vehicle with its payloads on buses, so its stand-ins differ; its physics
//! is a copy of the shared file's, and is held to it.

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
