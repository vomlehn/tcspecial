//! Configuration loading for payloads

use std::path::Path;

use crate::{load_config_file, DHConfig, PayloadConfig, TcsError, TcsResult};

/// Which environment variable names each program's payload configuration, and
/// what every one of them reads when its variable is unset.
///
/// Each program has its own variable because tcsmoc starts the other two as
/// subprocesses, which inherit its environment: one shared name could not
/// point tcsmoc at one file and its children at another. Tcsmoc itself takes
/// its file as a command line argument, for the same reason, and then sets
/// each child's variable on that child's own command.
///
/// They live here, beside [`load_payload_config`], rather than in the
/// programs: the name tcsmoc sets for a child is then the same constant the
/// child reads, checked by the compiler rather than by two string literals
/// being kept equal by hand.
pub const PAYLOAD_CONFIG_PATH_VAR: &str = "PAYLOAD_CONFIG_PATH";

/// Which environment variable names tcssim's payload configuration.
///
/// See [`PAYLOAD_CONFIG_PATH_VAR`]. Tcssim's simulator configuration is a
/// separate file, named by a variable of its own that only tcssim knows:
/// tcsmoc never reads it, so there is nothing here to keep in step.
pub const SIM_PAYLOAD_CONFIG_PATH_VAR: &str = "SIM_PAYLOAD_CONFIG_PATH";

/// The payload configuration read when nothing names one.
pub const DEFAULT_PAYLOAD_CONFIG_PATH: &str = "payload1.yaml";

/// Load payload configuration from a JSON, YAML, or XML file.
///
/// The format is chosen from the file extension; see
/// [`crate::format::ConfigFormat`]. All formats deserialize into the same
/// [`PayloadConfig`], whose data handler groups are resolved here so a caller
/// gets handlers with every attribute settled.
pub fn load_payload_config<P: AsRef<Path>>(path: P) -> TcsResult<Vec<DHConfig>> {
    let payload_config: PayloadConfig = load_config_file(path)?;

    payload_config.to_dh_configs().map_err(TcsError::Config)
}
