//! Cross-format equivalence tests.
//!
//! The Rust structs in `tcslibgs::types` are the single description of the
//! configuration data; JSON, YAML, and XML are three spellings of it. Nothing
//! in the code guarantees the three spellings stay interchangeable, so these
//! tests do: for each fixture, every format must deserialize to an *identical*
//! value.
//!
//! When you add a field, add it to all three fixtures. These tests fail if you
//! forget one.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use tcslibgs::{load_config_file, CIConfigJson, ConfigFormat, PayloadConfig};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Load the same logical fixture from all three formats and require that every
/// format produced the same value as JSON.
fn assert_all_formats_agree<T>(stem: &str)
where
    T: DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json: T = load_config_file(fixture(&format!("{stem}.json")))
        .unwrap_or_else(|e| panic!("{stem}.json failed to parse: {e}"));

    for ext in ["yaml", "xml"] {
        let other: T = load_config_file(fixture(&format!("{stem}.{ext}")))
            .unwrap_or_else(|e| panic!("{stem}.{ext} failed to parse: {e}"));
        assert_eq!(
            json, other,
            "{stem}.{ext} disagrees with {stem}.json -- the fixtures have drifted"
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
fn payload_fixture_has_expected_contents() {
    // Guards against the equivalence test passing because all three formats
    // are identically wrong (for instance, every field silently defaulting).
    let config: PayloadConfig = load_config_file(fixture("payload.yaml")).unwrap();

    assert_eq!(config.version, "1.0");
    assert_eq!(config.len(), 2);
    assert!(config.ci_config.is_none(), "payload fixture has no ci_config");

    let network = &config.data_handlers[0];
    assert_eq!(network.dh_id, 0);
    assert_eq!(network.dh_type, "network");
    assert_eq!(network.port, Some(5000));
    assert_eq!(network.path, None);

    let device = &config.data_handlers[1];
    assert_eq!(device.dh_type, "device");
    assert_eq!(device.path.as_deref(), Some("/dev/ttyS0"));
    assert_eq!(device.port, None);
}

#[test]
fn every_format_converts_to_runtime_types() {
    // Equivalence at the file layer is only useful if the conversion into the
    // real runtime types also succeeds from every format.
    for ext in ["json", "yaml", "xml"] {
        let config: PayloadConfig = load_config_file(fixture(&format!("payload.{ext}"))).unwrap();
        for dh in &config.data_handlers {
            dh.to_dh_config()
                .unwrap_or_else(|e| panic!("payload.{ext}: {} failed conversion: {e}", dh.name));
        }

        let ci: CIConfigJson = load_config_file(fixture(&format!("tcspecial.{ext}"))).unwrap();
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
