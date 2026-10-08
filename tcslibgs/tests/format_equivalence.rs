//! Cross-format equivalence tests.
//!
//! The Rust structs in `tcslibgs::types` are the single description of the
//! configuration data; JSON, YAML, and XML are three spellings of it. Nothing
//! in the code guarantees the three spellings stay interchangeable, so these
//! tests do: for each file in `tests/actual`, every format must deserialize to
//! an *identical* value.
//!
//! When you add a field, add it to all three actual files. These tests fail
//! if you forget one.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use tcslibgs::{
    load_config_file, CIConfigJson, ConfigFormat, EndpointConfig, NetworkProtocol, PayloadConfig,
};

fn actual(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/actual")
        .join(name)
}

/// Load the same logical configuration from all three formats and require
/// that every format produced the same value as JSON.
fn assert_all_formats_agree<T>(stem: &str)
where
    T: DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json: T = load_config_file(actual(&format!("{stem}.json")))
        .unwrap_or_else(|e| panic!("{stem}.json failed to parse: {e}"));

    for ext in ["yaml", "xml"] {
        let other: T = load_config_file(actual(&format!("{stem}.{ext}")))
            .unwrap_or_else(|e| panic!("{stem}.{ext} failed to parse: {e}"));
        assert_eq!(
            json, other,
            "{stem}.{ext} disagrees with {stem}.json -- the actual files have drifted"
        );
    }
}

#[test]
fn payload_config_is_format_independent() {
    assert_all_formats_agree::<PayloadConfig>("payload");
}

#[test]
fn ci_config_is_format_independent() {
    assert_all_formats_agree::<CIConfigJson>("tcspecial");
}

#[test]
fn payload_actual_file_has_expected_contents() {
    // Guards against the equivalence test passing because all three formats
    // are identically wrong (for instance, every field silently defaulting).
    let config: PayloadConfig = load_config_file(actual("payload.yaml")).unwrap();

    assert_eq!(config.version, "1.0");
    assert_eq!(config.len(), 2);
    assert!(
        config.ci_config.is_none(),
        "the payload actual file has no ci_config"
    );

    assert_eq!(config.payload_groups.len(), 1);
    let group = config.group("udp_localhost").expect("the group is defined");
    assert_eq!(group.dh_type.as_deref(), Some("network"));
    assert_eq!(group.protocol.as_deref(), Some("udp"));
    assert_eq!(group.packet_size, Some(12));
    assert_eq!(group.port, None, "a port tells one handler of a group from another");

    // A handler in the group states only what the group does not carry.
    let network = &config.payloads[0];
    assert_eq!(network.dh_id, 0);
    assert_eq!(network.group.as_deref(), Some("udp_localhost"));
    assert_eq!(network.dh_type, None);
    assert_eq!(network.packet_size, None);
    assert_eq!(network.port, Some(5000));
    assert_eq!(network.path, None);

    // A handler in no group states everything itself.
    let device = &config.payloads[1];
    assert_eq!(device.group, None);
    assert_eq!(device.dh_type.as_deref(), Some("device"));
    assert_eq!(device.path.as_deref(), Some("/dev/ttyS0"));
    assert_eq!(device.port, None);
}

#[test]
fn a_grouped_handler_resolves_the_same_from_every_format() {
    // The group is only useful if what a handler inherits from it survives
    // every format, so resolve the whole file rather than inspecting fields.
    for ext in ["json", "yaml", "xml"] {
        let config: PayloadConfig = load_config_file(actual(&format!("payload.{ext}"))).unwrap();
        let handlers = config
            .to_dh_configs()
            .unwrap_or_else(|e| panic!("payload.{ext} failed to resolve: {e}"));

        match &handlers[0].endpoint {
            EndpointConfig::Network(net) => {
                // All three from the group.
                assert_eq!(net.protocol, NetworkProtocol::Udp, "payload.{ext}");
                assert_eq!(net.address, "localhost", "payload.{ext}");
                // The handler's own.
                assert_eq!(net.port, 5000, "payload.{ext}");
            }
            other => panic!("payload.{ext}: DH0 resolved to {other:?}"),
        }
        // Stated by the group alone, so this is what proves an attribute no
        // handler mentions still reaches it.
        assert_eq!(handlers[0].packet_size, 12, "payload.{ext}");
    }
}

#[test]
fn a_handler_naming_an_undefined_group_is_rejected() {
    // Without this, a misspelled group name would leave a handler with no
    // endpoint attributes at all, which is a different and more confusing
    // error than the one that is really there.
    let config: PayloadConfig = ConfigFormat::Yaml
        .parse(
            "version: \"1.0\"
description: one handler naming a group that is not there
payloads:
  - dh_id: 0
    name: DH0
    group: nonesuch
    port: 5000
    packet_size: 1
",
        )
        .expect("parses: an undefined group is not a syntax error");

    let message = config.to_dh_configs().expect_err("must be rejected");
    assert!(
        message.contains("nonesuch"),
        "the error should name the missing group, but said: {message}"
    );
}

#[test]
fn a_group_defined_twice_is_rejected() {
    let config: PayloadConfig = ConfigFormat::Yaml
        .parse(
            "version: \"1.0\"
description: two groups of one name
payload_groups:
  - name: dup
    type: network
    protocol: udp
    address: localhost
  - name: dup
    type: device
    path: /dev/null
payloads:
  - dh_id: 0
    name: DH0
    group: dup
    port: 5000
    packet_size: 1
",
        )
        .expect("parses");

    let message = config.to_dh_configs().expect_err("must be rejected");
    assert!(
        message.contains("dup"),
        "the error should name the repeated group, but said: {message}"
    );
}

#[test]
fn every_format_converts_to_runtime_types() {
    // Equivalence at the file layer is only useful if the conversion into the
    // real runtime types also succeeds from every format.
    for ext in ["json", "yaml", "xml"] {
        let config: PayloadConfig = load_config_file(actual(&format!("payload.{ext}"))).unwrap();
        config
            .to_dh_configs()
            .unwrap_or_else(|e| panic!("payload.{ext} failed conversion: {e}"));

        let ci: CIConfigJson = load_config_file(actual(&format!("tcspecial.{ext}"))).unwrap();
        ci.to_ci_config()
            .unwrap_or_else(|e| panic!("tcspecial.{ext} failed conversion: {e}"));
    }
}

#[test]
fn malformed_input_is_rejected_in_every_format() {
    // Negative cases: without these, a parser that accepts anything would
    // still pass the tests above.
    assert!(ConfigFormat::Json.parse::<PayloadConfig>("{ not json").is_err());
    assert!(ConfigFormat::Yaml.parse::<PayloadConfig>("version: [").is_err());
    assert!(ConfigFormat::Xml.parse::<PayloadConfig>("<payload><version>").is_err());

    // A required field is missing in each format, so each must refuse it.
    assert!(ConfigFormat::Json
        .parse::<CIConfigJson>(r#"{"address":"0.0.0.0","port":1}"#)
        .is_err());
    assert!(ConfigFormat::Yaml
        .parse::<CIConfigJson>("address: 0.0.0.0\nport: 1\n")
        .is_err());
    assert!(ConfigFormat::Xml
        .parse::<CIConfigJson>("<ci><address>0.0.0.0</address><port>1</port></ci>")
        .is_err());
}
