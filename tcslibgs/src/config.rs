//! Configuration loading for payloads

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use tcslibgs::{CIConfig, DHConfig, TcsError, TcsResult};


/// Load payload configuration from a JSON file
pub fn load_payload_config<P: AsRef<Path>>(path: P) -> TcsResult<Vec<DHConfig>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let payload_payload_config: PayloadConfig = serde_json::from_reader(reader)?;

    let payload_config = payload_payload_config.payload_config.to_payload_config()
        .map_err(|e| TcsError::Config(e))?;

    let payload_config: Result<Vec<DHConfig>, String> = payload_payload_config
        .data_handlers
        .iter()
        .map(|dh| dh.to_dh_config())
        .collect();

    let payload_config = payload_config.map_err(|e| TcsError::Config(e))?;

    Ok(payload_config)
}
