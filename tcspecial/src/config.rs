//! Configuration loading for TCSpecial

use std::path::Path;

use tcslibgs::{load_config_file, CIConfig, TcsError, TcsResult};
use tcslibgs::CIConfigJson;

/// Load tcspecial configuration from a JSON, YAML, or XML file.
///
/// The format is chosen from the file extension; see
/// `tcslibgs::format::ConfigFormat`.
pub fn load_tcspecial_config<P: AsRef<Path>>(path: P) -> TcsResult<CIConfig> {
    let tcspecial_config_file: CIConfigJson = load_config_file(path)?;

    let tcspecial_config = tcspecial_config_file.to_ci_config()
        .map_err(|e| TcsError::Config(e))?;

    Ok(tcspecial_config)
}

/// Configuration constants
pub mod constants {
    use std::time::Duration;

    pub const BEACON_DEFAULT_MS: Duration = Duration::new(20, 0);

    // FIXME: use getaddrinfo()
    pub const BEACON_NETADDR: &str = "0.0.0.0:5550";

    /// Initial delay for endpoint retry
    pub const ENDPOINT_DELAY_INIT: Duration = Duration::from_millis(100);

    /// Maximum delay for endpoint retry
    pub const ENDPOINT_DELAY_MAX: Duration = Duration::from_secs(10);

    /// Maximum number of endpoint retries
    pub const ENDPOINT_MAX_RETRIES: u32 = 10;

    /// Stream endpoint delay for collecting bytes
    pub const STREAM_EP_DELAY: Duration = Duration::from_millis(50);

    /// Default buffer size for endpoints
    pub const ENDPOINT_BUFFER_SIZE: usize = 4096;

    /// Restart arm timeout
    pub const RESTART_ARM_TIMEOUT: Duration = Duration::from_secs(60);

    /// First part of every telemetry log segment file name
    pub const TELEMETRY_LOG_PREFIX: &str = "telem-";

    /// Last part of every telemetry log segment file name
    pub const TELEMETRY_LOG_SUFFIX: &str = ".tcslog";
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tcslibgs::NetworkProtocol;
    use tempfile::Builder;

    /// Write `text` to a temporary file with the given extension, so that
    /// `load_tcspecial_config` picks the matching parser.
    fn load_from(ext: &str, text: &str) -> TcsResult<CIConfig> {
        let mut file = Builder::new().suffix(ext).tempfile().unwrap();
        file.write_all(text.as_bytes()).unwrap();
        file.flush().unwrap();
        load_tcspecial_config(file.path())
    }

    const JSON: &str = r#"{
        "address": "0.0.0.0",
        "port": 4000,
        "protocol": "udp",
        "beacon_interval_ms": 5000
    }"#;

    const YAML: &str = "address: 0.0.0.0\nport: 4000\nprotocol: udp\nbeacon_interval_ms: 5000\n";

    const XML: &str = "<tcspecial>\
        <address>0.0.0.0</address>\
        <port>4000</port>\
        <protocol>udp</protocol>\
        <beacon_interval_ms>5000</beacon_interval_ms>\
        </tcspecial>";

    #[test]
    fn test_load_tcspecial_config_every_format() {
        for (ext, text) in [(".json", JSON), (".yaml", YAML), (".xml", XML)] {
            let config = load_from(ext, text)
                .unwrap_or_else(|e| panic!("{ext} failed to load: {e}"));

            assert_eq!(config.address, "0.0.0.0", "{ext}");
            assert_eq!(config.port, 4000, "{ext}");
            assert_eq!(config.protocol, NetworkProtocol::Udp, "{ext}");
            assert_eq!(config.beacon_interval.0, 5000, "{ext}");
            // Absent in every fixture, so the serde defaults must apply.
            assert_eq!(config.log_dir, None, "{ext}");
            assert_eq!(config.log_segment_bytes, 65_536, "{ext}");
        }
    }

    #[test]
    fn test_unknown_extension_is_parsed_as_json() {
        let config = load_from(".conf", JSON).unwrap();
        assert_eq!(config.port, 4000);
    }

    #[test]
    fn test_bad_protocol_is_rejected() {
        let bad = YAML.replace("protocol: udp", "protocol: carrier-pigeon");
        assert!(load_from(".yaml", &bad).is_err());
    }
}
