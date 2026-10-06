//! Configuration loading for payloads

use std::path::Path;

use crate::{load_config_file, DHConfig, PayloadConfig, TcsError, TcsResult};

/// Load payload configuration from a JSON, YAML, or XML file.
///
/// The format is chosen from the file extension; see
/// [`crate::format::ConfigFormat`]. All formats deserialize into the same
/// [`PayloadConfig`].
pub fn load_payload_config<P: AsRef<Path>>(path: P) -> TcsResult<Vec<DHConfig>> {
    let payload_config: PayloadConfig = load_config_file(path)?;

    let dh_configs: Result<Vec<DHConfig>, String> = payload_config
        .data_handlers
        .iter()
        .map(|dh| dh.to_dh_config())
        .collect();

    dh_configs.map_err(TcsError::Config)
}
