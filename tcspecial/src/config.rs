//! Configuration loading for TCSpecial

use std::path::Path;

use tcslibgs::endpoint_config::{self, EndpointConfigDoc};
use std::net::SocketAddr;

use tcslibgs::{beacon_address_of, load_config_file, CIConfig, CIConfigJson, TcsError, TcsResult};

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

/// Where beacons go: what the payload set said, or what the command
/// interpreter's own file said for a set that said nothing.
///
/// The set wins because the set's own ground station is what listens for its
/// beacons. `section` is the set's `tcspecial` section, and a set either has
/// one -- in which case it states an address, the attribute being required --
/// or has none at all: a set written in the endpoint language has no place to
/// put one, and that is the case this file's own address is for.
pub fn beacon_address(
    config: &CIConfig,
    section: Option<&CIConfigJson>,
) -> Result<SocketAddr, String> {
    match section {
        Some(section) => beacon_address_of(&section.beacon_address),
        None => Ok(config.beacon_address),
    }
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

    const YAML: &str = "address: 0.0.0.0\nport: 4000\nprotocol: udp\n\
                        beacon_interval_ms: 5000\nbeacon_address: 0.0.0.0:5550\n";

    const XML: &str = "<tcspecial>\
        <address>0.0.0.0</address>\
        <port>4000</port>\
        <protocol>udp</protocol>\
        <beacon_interval_ms>5000</beacon_interval_ms>\
        <beacon_address>0.0.0.0:5550</beacon_address>\
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

    /// Where beacons go is read from the file, and a file that omits it is
    /// refused.
    ///
    /// Required, with no default, because beacons are how the ground knows
    /// the spacecraft is alive: a configuration that has not been asked where
    /// to send them has not answered. The interval beside it is required the
    /// same way.
    #[test]
    fn the_beacon_address_and_interval_are_required() {
        let config = load_from(".yaml", YAML).expect("the configuration loads");
        assert_eq!(config.beacon_address.to_string(), "0.0.0.0:5550");
        assert_eq!(config.beacon_interval.0, 5000);

        for missing in ["beacon_address: 0.0.0.0:5550\n", "beacon_interval_ms: 5000\n"] {
            let without = YAML.replace(missing, "");
            assert_ne!(without, YAML, "the test removed nothing");
            assert!(
                load_from(".yaml", &without).is_err(),
                "a file with no {missing:?} was accepted"
            );
        }
    }

    /// The payload set says where its beacons go; this file is the fallback.
    ///
    /// Four cases, because each is a different answer and the rule is which
    /// of two configurations is consulted: a set that states an address, a
    /// set that states none, no set section at all, and a set that states
    /// something that is not an address.
    #[test]
    fn a_payload_set_says_where_its_beacons_go() {
        let from_the_file = load_from(
            ".yaml",
            &YAML.replace("beacon_address: 0.0.0.0:5550", "beacon_address: 10.0.0.1:5550"),
        )
        .expect("the configuration loads");
        assert_eq!(from_the_file.beacon_address.to_string(), "10.0.0.1:5550");

        let section = |beacon: &str| CIConfigJson {
            address: "0.0.0.0".to_string(),
            port: 4000,
            protocol: "udp".to_string(),
            beacon_interval_ms: 5000,
            beacon_address: beacon.to_string(),
            log_dir: None,
            log_segment_bytes: 65_536,
        };

        // A set with a section: the set wins. It always states an address,
        // the attribute being required of a section as of a file.
        assert_eq!(
            beacon_address(&from_the_file, Some(&section("127.0.0.1:7550")))
                .expect("it resolves")
                .to_string(),
            "127.0.0.1:7550"
        );

        // No section at all -- a set written in the endpoint language, which
        // has no place to state one. This file's own address is for that set.
        assert_eq!(
            beacon_address(&from_the_file, None)
                .expect("it resolves")
                .to_string(),
            "10.0.0.1:5550"
        );

        // And a set stating something that is not an address is refused
        // rather than quietly leaving the file's in place.
        let said = beacon_address(&from_the_file, Some(&section("nowhere")))
            .expect_err("not an address")
            .to_string();
        assert!(said.contains("beacon_address"), "{said}");
        assert!(said.contains("nowhere"), "{said}");
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
                "beacon_address: 0.0.0.0:5550",
                &format!("beacon_address: \"{bad}\""),
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
