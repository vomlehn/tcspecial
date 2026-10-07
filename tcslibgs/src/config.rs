//! Configuration loading for payloads

use std::fs;
use std::path::Path;

use serde::de::IgnoredAny;
use serde::Deserialize;

use crate::{
    endpoint_config, load_config_file, ConfigFormat, DHConfig, PayloadConfig, TcsError, TcsResult,
};

/// Which environment variable names each program's payload configuration, and
/// what every one of them reads when its variable is unset.
///
/// Each program has its own variable because tcsmoc starts the other two as
/// subprocesses, which inherit its environment: one shared name could not
/// point tcsmoc at one file and its children at another. Tcsmoc itself takes
/// its file as a command line argument, for the same reason, and then sets
/// each child's variable on that child's own command.
///
/// They live here, beside [`load_dh_configs`], rather than in the
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

/// Which kind of configuration a file holds.
///
/// A file's extension says how it is spelled -- YAML, JSON or XML -- and not
/// what it describes. Both kinds of configuration can be written in any of the
/// three, so which one a file holds is read from the sections it has rather
/// than from its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerSource {
    /// A payload configuration, which names data handlers directly.
    Payload,
    /// An endpoint configuration, whose endpoints become data handlers.
    Endpoints,
}

/// Just enough of a file to tell what kind it is.
///
/// Every field is ignored once seen: this asks which sections exist and
/// nothing about their contents, so a file with an error inside a section is
/// still recognised and then reported by the real parser, which can say what
/// is wrong with it.
#[derive(Deserialize)]
struct Sections {
    #[serde(default)]
    data_handlers: Option<IgnoredAny>,
    #[serde(default, alias = "endpoint-groups")]
    endpoint_groups: Option<IgnoredAny>,
    #[serde(default)]
    endpoints: Option<IgnoredAny>,
}

/// Which kind of configuration `text` holds.
fn handler_source_of(text: &str, format: ConfigFormat) -> TcsResult<HandlerSource> {
    let sections: Sections = format.parse(text)?;

    // A payload file is recognised by its own section, so a file carrying both
    // -- which neither format describes -- is read as a payload file rather
    // than rejected. There is nothing a caller could do about it either way.
    if sections.data_handlers.is_some() {
        Ok(HandlerSource::Payload)
    } else if sections.endpoints.is_some() || sections.endpoint_groups.is_some() {
        Ok(HandlerSource::Endpoints)
    } else {
        Err(TcsError::Config(
            "names neither data_handlers nor endpoints, so it describes no data \
             handlers"
                .to_string(),
        ))
    }
}

/// Which kind of configuration the file at `path` holds.
pub fn handler_source<P: AsRef<Path>>(path: P) -> TcsResult<HandlerSource> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)?;
    handler_source_of(&text, ConfigFormat::from_path(path))
}

/// Load data handlers from a payload or an endpoint configuration file.
///
/// Every program that needs data handlers reads them through this, so that any
/// of them can be pointed at either kind of file and all of them agree about
/// what a given file describes. That matters more than it sounds: tcsmoc's
/// panels, tcssim's payloads and tcspecial's handlers have to be the same
/// handlers, and one program reading a file a different way is the drift the
/// payload set mechanism exists to prevent.
pub fn load_dh_configs<P: AsRef<Path>>(path: P) -> TcsResult<Vec<DHConfig>> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)?;
    let format = ConfigFormat::from_path(path);

    match handler_source_of(&text, format)? {
        HandlerSource::Payload => {
            let config: PayloadConfig = format.parse(&text)?;
            config.to_dh_configs().map_err(TcsError::Config)
        }
        HandlerSource::Endpoints => {
            let doc = endpoint_config::from_str(&text, format)?;
            doc.to_dh_configs().map_err(TcsError::from)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAYLOAD: &str = "
version: \"1.0\"
description: a payload file
data_handlers:
  - dh_id: 0
    name: DH0
    oc_address: 127.0.0.1
    oc_port: 6000
    type: network
    protocol: udp
    address: localhost
    port: 5000
    packet_size: 12
";

    const ENDPOINTS: &str = "
endpoint_groups:
  - name: payload_udp
    type: network
    protocol: udp
    packet_size: 12
endpoints:
  - name: DH0
    group: payload_udp
    dh_id: 0
    oc_address: 127.0.0.1
    oc_port: 6000
    address: localhost
    port: 5000
";

    #[test]
    fn each_kind_of_file_is_recognised_by_its_sections() {
        assert_eq!(
            handler_source_of(PAYLOAD, ConfigFormat::Yaml).unwrap(),
            HandlerSource::Payload
        );
        assert_eq!(
            handler_source_of(ENDPOINTS, ConfigFormat::Yaml).unwrap(),
            HandlerSource::Endpoints
        );
    }

    #[test]
    fn a_file_naming_neither_section_is_rejected() {
        // Not read as an endpoint configuration with no endpoints, which is
        // what an empty document would otherwise look like.
        let message = handler_source_of("version: \"1.0\"\n", ConfigFormat::Yaml)
            .expect_err("must be rejected")
            .to_string();
        assert!(
            message.contains("data_handlers") && message.contains("endpoints"),
            "the error should name both sections, but said: {message}"
        );
    }

    #[test]
    fn both_kinds_of_file_give_the_same_handler() {
        // The same data handler written each way, so this is what says the two
        // formats are interchangeable where they overlap.
        let from_payload = {
            let config: PayloadConfig = ConfigFormat::Yaml.parse(PAYLOAD).unwrap();
            config.to_dh_configs().unwrap()
        };
        let from_endpoints = endpoint_config::from_yaml_str(ENDPOINTS)
            .unwrap()
            .to_dh_configs()
            .unwrap();

        assert_eq!(from_payload.len(), 1);
        assert_eq!(from_endpoints.len(), 1);

        let a = &from_payload[0];
        let b = &from_endpoints[0];
        assert_eq!(a.dh_id, b.dh_id);
        assert_eq!(a.name, b.name);
        assert_eq!(a.endpoint, b.endpoint);
        assert_eq!(a.packet_size, b.packet_size);
        assert_eq!(a.oc, b.oc);
    }
}
