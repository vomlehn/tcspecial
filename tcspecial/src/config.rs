//! Configuration loading for TCSpecial

use std::path::Path;

use tcslibgs::endpoint_config::{self, EndpointConfigDoc};
use tcslibgs::config::command_address_parts;
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

/// Where commands are taken: what the command line said, or failing that what
/// the configuration file said.
///
/// The command line wins, and that is the whole point of it. Tcsmoc decides
/// the address it will send commands to and starts tcspecial with it, so the
/// one that matters is the one the ground chose; a tcspecial that preferred
/// its own file could be listening somewhere nobody was talking to, and the
/// only sign of it was every command timing out.
///
/// With no argument nothing changes: a tcspecial run on its own -- `make run`,
/// or under a debugger -- is still placed by its configuration file.
pub fn command_address(config: &CIConfig, given: Option<&str>) -> Result<(String, u16), String> {
    match given {
        Some(address) => command_address_parts(address),
        None => Ok((config.address.clone(), config.port)),
    }
}

/// Load an endpoint configuration file in JSON, YAML, or XML.
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

    // FIXME: use getaddrinfo()
    pub const BEACON_NETADDR: &str = "0.0.0.0:5550";

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
            // Absent in every actual input, so the serde defaults must apply.
            assert_eq!(config.log_dir, None, "{ext}");
            assert_eq!(config.log_segment_bytes, 65_536, "{ext}");
        }
    }

    /// The command line says where commands are taken, and the configuration
    /// file says it only when the command line did not.
    ///
    /// Tcsmoc starts tcspecial with the address it is about to send to, so the
    /// argument has to win. Before it existed, the MOC sent to its own default
    /// and tcspecial bound what its file said; the two agreed because someone
    /// kept them equal by hand, and when they stopped agreeing every command
    /// timed out with nothing on either end to say why.
    #[test]
    fn the_command_line_places_the_command_interpreter() {
        let config = load_from(".yaml", YAML).expect("the configuration loads");
        assert_eq!((config.address.clone(), config.port), ("0.0.0.0".to_string(), 4000));

        // Nothing given: the file is still what places it.
        assert_eq!(
            command_address(&config, None).unwrap(),
            ("0.0.0.0".to_string(), 4000)
        );

        // Given: the argument, down to the port, which is the half most
        // likely to differ and the half a bind cannot do without.
        assert_eq!(
            command_address(&config, Some("127.0.0.1:4100")).unwrap(),
            ("127.0.0.1".to_string(), 4100)
        );

        // And an argument that is not an address does not quietly leave the
        // file's one in place: a tcspecial listening somewhere other than
        // where it was told is the fault this exists to prevent.
        let e = command_address(&config, Some("127.0.0.1")).expect_err("no port");
        assert!(e.contains("127.0.0.1"), "{e}");
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

    const ENDPOINTS_JSON: &str = r#"{
        "endpoint_groups": [
            { "name": "bus", "type": "i2c", "pec": true },
            { "name": "chip", "type": "spi", "max_speed": 1000000, "mode": 0 }
        ],
        "endpoints": [
            { "name": "thermal", "group": "bus",
              "device": "/dev/i2c-1", "address": "0x48" },
            { "name": "imu", "group": "chip", "device": "/dev/spidev0.0" }
        ]
    }"#;

    #[test]
    fn test_load_endpoint_config_every_format() {
        for (ext, text) in [
            (".yaml", ENDPOINTS_YAML),
            (".xml", ENDPOINTS_XML),
            (".json", ENDPOINTS_JSON),
        ] {
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
        let json = load_endpoints_from(".json", ENDPOINTS_JSON).unwrap();
        assert_eq!(yaml, xml);
        assert_eq!(yaml, json);
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
