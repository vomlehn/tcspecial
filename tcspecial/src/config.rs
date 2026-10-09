//! Configuration loading for TCSpecial

use std::path::Path;

use tcslibgs::endpoint_config::{self, EndpointConfigDoc};
use tcslibgs::{load_config_file, CIConfig, TcsError, TcsResult};
use tcslibgs::CIConfigJson;

/// Load tcspecial configuration from a YAML or XML file.
///
/// The format is chosen from the file extension; see
/// `tcslibgs::format::ConfigFormat`.
pub fn load_tcspecial_config<P: AsRef<Path>>(path: P) -> TcsResult<CIConfig> {
    let tcspecial_config_file: CIConfigJson = load_config_file(path)?;

    let tcspecial_config = tcspecial_config_file.to_ci_config()
        .map_err(|e| TcsError::Config(e))?;

    Ok(tcspecial_config)
}

/// Load an endpoint configuration file in YAML or XML.
///
/// The format is chosen from the file extension, as it is for every other
/// configuration file here. What comes back is the groups and the endpoints
/// drawing on them, already checked against the rules in "Endpoint
/// Configuration Files" in `docs/design.rst`: a group carries what its
/// endpoints share, and each endpoint carries what locates it.
pub fn load_endpoint_config<P: AsRef<Path>>(path: P) -> TcsResult<EndpointConfigDoc> {
    endpoint_config::load(path).map_err(TcsError::from)
}

/// Configuration constants
pub mod constants {
    use std::time::Duration;

    pub const BEACON_DEFAULT_MS: Duration = Duration::new(20, 0);

    /// Initial delay for endpoint retry
    pub const ENDPOINT_DELAY_INIT: Duration = Duration::from_millis(100);

    /// Maximum delay for endpoint retry
    pub const ENDPOINT_DELAY_MAX: Duration = Duration::from_secs(10);

    /// Maximum number of endpoint retries
    pub const ENDPOINT_MAX_RETRIES: u32 = 10;

    /// How long a data handler keeps trying to reach a payload that refuses
    /// the connection.
    ///
    /// Bounded, and well under the ground's command timeout, because StartDH
    /// is answered on the command interpreter's own thread: retrying for
    /// ENDPOINT_MAX_RETRIES with ENDPOINT_DELAY_MAX between attempts would
    /// take some forty seconds, holding up every other command and leaving
    /// the ground to time out rather than be told anything.
    ///
    /// Two seconds is enough for the case this exists for -- a simulated
    /// payload started a moment after its handler -- and short enough that a
    /// payload which is simply not there is reported promptly.
    pub const ENDPOINT_CONNECT_BUDGET: Duration = Duration::from_secs(2);

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

    const YAML: &str = "address: 0.0.0.0\nport: 4000\nprotocol: udp\nbeacon_interval_ms: 5000\n";

    const XML: &str = "<tcspecial>\
        <address>0.0.0.0</address>\
        <port>4000</port>\
        <protocol>udp</protocol>\
        <beacon_interval_ms>5000</beacon_interval_ms>\
        </tcspecial>";

    #[test]
    fn test_load_tcspecial_config_every_format() {
        for (ext, text) in [(".yaml", YAML), (".xml", XML)] {
            let config = load_from(ext, text)
                .unwrap_or_else(|e| panic!("{ext} failed to load: {e}"));

            assert_eq!(config.address, "0.0.0.0", "{ext}");
            assert_eq!(config.port, 4000, "{ext}");
            assert_eq!(config.protocol, NetworkProtocol::Udp, "{ext}");
            assert_eq!(config.beacon_interval.0, 5000, "{ext}");
            // Absent in every actual input, so the serde defaults must apply.
            assert_eq!(config.log_dir, None, "{ext}");
            assert_eq!(config.log_segment_bytes, 65_536, "{ext}");
        }
    }

    /// The beacon address is read from the file, and defaults when absent.
    ///
    /// It was a constant in tcspecial that tcsmoc imported, so a mission that
    /// wanted beacons anywhere else had to rebuild both programs. The default
    /// is that same constant, so a file written before the attribute existed
    /// still sends them where it always did.
    #[test]
    fn the_beacon_address_comes_from_the_file_or_the_default() {
        let config = load_from(".yaml", YAML).expect("the configuration loads");
        assert_eq!(
            config.beacon_address.to_string(),
            tcslibgs::DEFAULT_BEACON_ADDRESS,
            "a file that says nothing gets the default"
        );

        let stated = YAML.replace(
            "beacon_interval_ms: 5000",
            "beacon_interval_ms: 5000\nbeacon_address: 127.0.0.1:6550",
        );
        let config = load_from(".yaml", &stated).expect("the configuration loads");
        assert_eq!(config.beacon_address.to_string(), "127.0.0.1:6550");
    }

    /// A beacon address that is not one is refused where the file is read.
    ///
    /// Not where the beacon is sent: a beacon goes out on a timer with nobody
    /// to report to, so an address that cannot be parsed has to be caught
    /// while someone is still reading errors. It used to be a constant parsed
    /// with `unwrap`, which could only ever have panicked.
    #[test]
    fn a_beacon_address_that_is_not_one_is_refused() {
        for bad in ["0.0.0.0", "nowhere:5550", "0.0.0.0:not-a-port", ""] {
            let text = YAML.replace(
                "beacon_interval_ms: 5000",
                &format!("beacon_interval_ms: 5000\nbeacon_address: \"{bad}\""),
            );
            let said = load_from(".yaml", &text)
                .expect_err(bad)
                .to_string();
            assert!(
                said.contains("beacon_address"),
                "{bad}: the refusal does not name the attribute: {said}"
            );
        }
    }

    /// An extension this does not read is refused rather than guessed at.
    ///
    /// It used to be read as JSON, which is how a file whose extension was
    /// misspelled became a file that would not parse for reasons that said
    /// nothing about its name.
    #[test]
    fn an_unknown_extension_is_refused() {
        let said = load_from(".conf", YAML)
            .expect_err("a .conf file is not a configuration file")
            .to_string();
        assert!(
            said.contains(".yaml") && said.contains(".xml"),
            "the refusal does not say what is read: {said}"
        );
    }

    #[test]
    fn test_bad_protocol_is_rejected() {
        let bad = YAML.replace("protocol: udp", "protocol: carrier-pigeon");
        assert!(load_from(".yaml", &bad).is_err());
    }

    // -- endpoint configuration ---------------------------------------------

    /// As `load_from`, for an endpoint configuration file.
    fn load_endpoints_from(ext: &str, text: &str) -> TcsResult<EndpointConfigDoc> {
        let mut file = Builder::new().suffix(ext).tempfile().unwrap();
        file.write_all(text.as_bytes()).unwrap();
        file.flush().unwrap();
        load_endpoint_config(file.path())
    }

    const ENDPOINTS_YAML: &str = "\
endpoint_groups:
  - name: bus
    type: i2c
    pec: true
  - name: chip
    type: spi
    max_speed: 1000000
    mode: 0
endpoints:
  - name: thermal
    group: bus
    device: /dev/i2c-1
    address: 0x48
  - name: imu
    group: chip
    device: /dev/spidev0.0
";

    const ENDPOINTS_XML: &str = r#"<endpoint-configuration>
  <endpoint-groups>
    <group name="bus" type="i2c" pec="true"/>
    <group name="chip" type="spi" max_speed="1000000" mode="0"/>
  </endpoint-groups>
  <endpoints>
    <endpoint name="thermal" group="bus" device="/dev/i2c-1" address="0x48"/>
    <endpoint name="imu" group="chip" device="/dev/spidev0.0"/>
  </endpoints>
</endpoint-configuration>"#;

    #[test]
    fn test_load_endpoint_config_every_format() {
        for (ext, text) in [(".yaml", ENDPOINTS_YAML), (".xml", ENDPOINTS_XML)] {
            let doc = load_endpoints_from(ext, text)
                .unwrap_or_else(|e| panic!("{ext} failed to load: {e}"));

            assert_eq!(doc.groups.len(), 2, "{ext}");
            assert_eq!(doc.endpoints.len(), 2, "{ext}");
            assert_eq!(doc.group("bus").unwrap().kind.type_name(), "i2c", "{ext}");
            assert_eq!(doc.group("chip").unwrap().kind.type_name(), "spi", "{ext}");
        }
    }

    #[test]
    fn test_endpoint_formats_agree() {
        let yaml = load_endpoints_from(".yaml", ENDPOINTS_YAML).unwrap();
        let xml = load_endpoints_from(".xml", ENDPOINTS_XML).unwrap();
        assert_eq!(yaml, xml);
    }

    #[test]
    fn test_endpoint_rule_violations_reach_the_caller() {
        // The rules live in the parser; what this checks is that a violation
        // arrives here as an error rather than as a surprising default.
        let reserved = ENDPOINTS_YAML.replace("address: 0x48", "address: 0x00");
        let e = load_endpoints_from(".yaml", &reserved).unwrap_err();
        assert!(format!("{e}").contains("reserved"), "got {e}");

        let no_mode = ENDPOINTS_YAML.replace("    mode: 0\n", "");
        assert!(load_endpoints_from(".yaml", &no_mode).is_err());
    }
}
