//! Cross-format equivalence tests.
//!
//! The Rust structs in `tcslibgs::types` are the single description of the
//! configuration data; YAML and XML are two spellings of it. Nothing in the
//! code guarantees the two spellings stay interchangeable, so these tests do:
//! for each file in `tests/actual`, both formats must deserialize to an
//! *identical* value. The command interpreter's own configuration is part of
//! that file, as the `tcspecial` section, rather than an actual file of its
//! own.
//!
//! There were three spellings. JSON is no longer read -- see
//! `tcslibgs::format` -- so there is one fewer file and one fewer parser to
//! keep in step.
//!
//! When you add a field, add it to both actual files. These tests fail if you
//! forget one.

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

/// Load the same logical configuration from both formats and require that
/// they produced the same value.
fn assert_all_formats_agree<T>(stem: &str)
where
    T: DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let yaml: T = load_config_file(actual(&format!("{stem}.yaml")))
        .unwrap_or_else(|e| panic!("{stem}.yaml failed to parse: {e}"));

    let xml: T = load_config_file(actual(&format!("{stem}.xml")))
        .unwrap_or_else(|e| panic!("{stem}.xml failed to parse: {e}"));

    assert_eq!(
        yaml, xml,
        "{stem}.xml disagrees with {stem}.yaml -- the actual files have drifted"
    );
}

#[test]
fn the_actual_file_is_format_independent() {
    assert_all_formats_agree::<PayloadConfig>("tcspecial");
}

#[test]
fn the_actual_file_has_expected_contents() {
    // Guards against the equivalence test passing because all three formats
    // are identically wrong (for instance, every field silently defaulting).
    let config: PayloadConfig = load_config_file(actual("tcspecial.yaml")).unwrap();

    assert_eq!(config.version, "1.0");
    assert_eq!(config.len(), 2);
    // The section that says what tcspecial itself is configured with for this
    // set. Checked field by field rather than only for being present, because
    // every format has to carry a nested section and not merely accept the
    // name of one.
    let ci = config
        .tcspecial
        .as_ref()
        .expect("the payload actual file carries a tcspecial section");
    assert_eq!(ci.address, "0.0.0.0");
    assert_eq!(ci.port, 4000);
    assert_eq!(ci.protocol, "udp");
    assert_eq!(ci.beacon_interval_ms, 5000);
    assert_eq!(ci.log_dir.as_deref(), Some("/var/log/tcspecial"));
    assert_eq!(ci.log_segment_bytes, 65_536);

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
    for ext in ["yaml", "xml"] {
        let config: PayloadConfig = load_config_file(actual(&format!("tcspecial.{ext}"))).unwrap();
        let handlers = config
            .to_dh_configs()
            .unwrap_or_else(|e| panic!("tcspecial.{ext} failed to resolve: {e}"));

        match &handlers[0].endpoint {
            EndpointConfig::Network(net) => {
                // All three from the group.
                assert_eq!(net.protocol, NetworkProtocol::Udp, "tcspecial.{ext}");
                assert_eq!(net.address, "localhost", "tcspecial.{ext}");
                // The handler's own.
                assert_eq!(net.port, 5000, "tcspecial.{ext}");
            }
            other => panic!("tcspecial.{ext}: DH0 resolved to {other:?}"),
        }
        // Stated by the group alone, so this is what proves an attribute no
        // handler mentions still reaches it.
        assert_eq!(handlers[0].packet_size, 12, "tcspecial.{ext}");
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
    for ext in ["yaml", "xml"] {
        let config: PayloadConfig = load_config_file(actual(&format!("tcspecial.{ext}"))).unwrap();
        config
            .to_dh_configs()
            .unwrap_or_else(|e| panic!("tcspecial.{ext} failed conversion: {e}"));

    }
}

#[test]
fn the_tcspecial_section_is_a_command_interpreter_configuration() {
    // The section is what a tcspecial.yaml holds, so it has to convert into
    // the runtime configuration a command interpreter is placed by --
    // otherwise it is a look-alike, and a file carrying it would be
    // describing something no program could ever be placed by.
    //
    // It is also the one place the three spellings of a command interpreter
    // configuration are still compared: the_actual_file_is_format_independent
    // holds the whole file identical across the formats, and this section is
    // part of that file.
    for ext in ["yaml", "xml"] {
        let config: PayloadConfig = load_config_file(actual(&format!("tcspecial.{ext}"))).unwrap();
        let carried = config
            .tcspecial
            .as_ref()
            .unwrap_or_else(|| panic!("tcspecial.{ext} carries no tcspecial section"));

        let ci = carried
            .to_ci_config()
            .unwrap_or_else(|e| panic!("tcspecial.{ext}'s section does not convert: {e}"));

        // Enough of the result to show the conversion carried the values
        // rather than succeeding on defaults.
        assert_eq!(ci.address, "0.0.0.0", "tcspecial.{ext}");
        assert_eq!(ci.port, 4000, "tcspecial.{ext}");
        assert_eq!(ci.protocol, NetworkProtocol::Udp, "tcspecial.{ext}");
    }
}

#[test]
fn a_tcspecial_section_that_could_never_be_used_is_refused() {
    // Read by nothing yet, and still checked: a file states the section
    // because its author meant it, so a section that could not place a
    // command interpreter is an error where the file is loaded rather than a
    // surprise for the first program to look at it.
    let config: PayloadConfig = ConfigFormat::Yaml
        .parse(
            "version: \"1.0\"
description: a set whose tcspecial section is wrong
payloads:
  - dh_id: 0
    name: DH0
    type: device
    path: /dev/null
    packet_size: 1
tcspecial:
  address: 0.0.0.0
  port: 4000
  protocol: carrier-pigeon
  beacon_interval_ms: 5000
  beacon_address: 239.255.0.1:5550
  beacon_interface: 127.0.0.1
",
        )
        .expect("parses: a protocol that is not one is not a syntax error");

    let message = config.to_dh_configs().expect_err("must be rejected");
    assert!(
        message.contains("tcspecial") && message.contains("carrier-pigeon"),
        "the error should name the section and the value, but said: {message}"
    );
}

#[test]
fn malformed_input_is_rejected_in_every_format() {
    // Negative cases: without these, a parser that accepts anything would
    // still pass the tests above.
    assert!(ConfigFormat::Yaml.parse::<PayloadConfig>("version: [").is_err());
    assert!(ConfigFormat::Xml.parse::<PayloadConfig>("<payload><version>").is_err());

    // A required field is missing in each format, so each must refuse it.
    assert!(ConfigFormat::Yaml
        .parse::<CIConfigJson>("address: 0.0.0.0\nport: 1\n")
        .is_err());
    assert!(ConfigFormat::Xml
        .parse::<CIConfigJson>("<ci><address>0.0.0.0</address><port>1</port></ci>")
        .is_err());
}
