//! Writing the F´ model of a configuration's bridge, for `--fprime-model`.
//!
//! What the model is, and every rule about it, is
//! [`tcslib_protocol::graph::fprime`]'s. This only writes it: the FPP the
//! `TcsBridge` component includes and the C++ built beside it, into the
//! directory the component's build reads them from. Here rather than in the
//! component's build because tcsverify is where a configuration is checked,
//! and a model is made only from one that has been: the bridge is then built
//! from exactly what the vehicle would load.

use std::path::{Path, PathBuf};

use tcslib_protocol::graph::{fprime, Graph};

/// What the FPP is called, which `TcsBridge.fpp` includes by this name.
pub const FPPI: &str = "TcsBridgeModel.fppi";

/// What the C++ is called, which the component's CMakeLists.txt builds.
pub const CPP: &str = "TcsBridgeModel.cpp";

/// Writes the model of the one bridge a configuration has into a directory.
///
/// A file that already says what would be written is left alone, so that
/// writing the model again does not make the bridge build again.
///
/// # Arguments
///
/// * `configuration` - The configuration, which has been checked and found
///                     to have no errors.
/// * `dir`           - Where to write it, made if it is not there.
///
/// # Returns
///
/// The gateway node the model is of, and each file written.
///
/// # Errors
///
/// A message, in the user's terms, when the configuration does not read,
/// has no `gateway_fprime` node or more than one, its model cannot be made,
/// or a file cannot be written.
pub fn write(
    configuration: &Path,
    dir: &Path,
) -> Result<(String, Vec<PathBuf>), String> {
    let shown = configuration.display().to_string();
    let graph = Graph::load(configuration).map_err(|e| format!("{e}"))?;
    let digest = tcslib_protocol::config_digest::digest_of_graph(&graph)
        .map_err(|e| format!("{shown}: no digest: {e}"))?
        .to_string();
    let models = fprime::every(&graph)
        .map_err(|r| format!("{shown}: \"{}\": {}", r.node, r.why))?;
    let model = match models.as_slice() {
        [one] => one,
        [] => {
            return Err(format!(
                "{shown} has no gateway_fprime node, so there is no F´ \
                 bridge to write the model of"
            ))
        }
        several => {
            return Err(format!(
                "{shown} has {} gateway_fprime nodes -- {} -- and a model \
                 is of one bridge: give each bridge a configuration of its \
                 own",
                several.len(),
                several
                    .iter()
                    .map(|m| m.gateway.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }
    };
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("{}: cannot be made: {e}", dir.display()))?;
    let mut written = Vec::new();
    for (name, text) in [
        (FPPI, model.fppi(&shown, &digest)),
        (CPP, model.cpp(&shown, &digest)),
    ] {
        let path = dir.join(name);
        if std::fs::read_to_string(&path).is_ok_and(|was| was == text) {
            continue;
        }
        std::fs::write(&path, text).map_err(|e| {
            format!("{}: cannot be written: {e}", path.display())
        })?;
        written.push(path);
    }
    Ok((model.gateway.clone(), written))
}
