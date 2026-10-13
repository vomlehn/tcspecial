//! Configuration loading for TCSpecial

use std::path::Path;

use tcslibgs::{
    host_port_of, load_config_file, CIConfig, CIConfigJson, HostPort, TcsError, TcsResult,
};

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

/// The two ends of the beacon link.
///
/// Both ends resolve this the same way, out of the same two files, which is
/// the whole point of its being a function: tcspecial makes its sending socket
/// out of `send` and sends to `recv`, tcsmoc makes its receiving socket out of
/// `recv`, and a pair that disagreed would lose every beacon with nothing to
/// show for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Beacon {
    /// Where beacons originate: the address and port tcspecial binds the
    /// socket it sends them on to.
    pub send: HostPort,
    /// Where they are received: the address and port tcsmoc binds the socket
    /// it receives them on to.
    pub recv: HostPort,
}

/// Where beacons go: what the payload set said, or what the command
/// interpreter's own file said for a set that said nothing.
///
/// The set wins because the set's own ground station is what listens for its
/// beacons. `section` is the set's `tcspecial` section, and a set either has
/// one -- in which case it states both attributes, each being required -- or
/// has none at all, which is the case this file's own values are for.
pub fn beacon(config: &CIConfig, section: Option<&CIConfigJson>) -> Result<Beacon, String> {
    match section {
        Some(section) => Ok(Beacon {
            send: host_port_of("beacon.send", &section.beacon.send)?,
            recv: host_port_of("beacon.recv", &section.beacon.recv)?,
        }),
        None => Ok(Beacon {
            send: config.beacon_send.clone(),
            recv: config.beacon_recv.clone(),
        }),
    }
}

/// Which file the command interpreter's own configuration is in.
///
/// The path `TCSPECIAL_CONFIG_PATH` names, or the shipped one. Here rather than
/// in tcspecial's own startup because tcsmoc reads the same file for the same
/// reason it reads the tcspecial configuration: to know where the beacons it is
/// listening for are being sent.
pub fn tcspecial_config_path() -> String {
    std::env::var(TCSPECIAL_CONFIG_PATH_VAR)
        .unwrap_or_else(|_| DEFAULT_TCSPECIAL_CONFIG_PATH.to_string())
}

/// Which environment variable names the command interpreter's configuration,
/// and what is read when it is unset.
pub const TCSPECIAL_CONFIG_PATH_VAR: &str = "TCSPECIAL_CONFIG_PATH";
pub const DEFAULT_TCSPECIAL_CONFIG_PATH: &str = "tcspecial/src/tcspecial.yaml";

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
    use tempfile::Builder;

    /// Write `text` to a temporary file with the given extension, so that
    /// `load_tcspecial_config` picks the matching parser.
    fn load_from(ext: &str, text: &str) -> TcsResult<CIConfig> {
        let mut file = Builder::new().suffix(ext).tempfile().unwrap();
        file.write_all(text.as_bytes()).unwrap();
        file.flush().unwrap();
        load_tcspecial_config(file.path())
    }

    const YAML: &str = "tcs_cmd:\n  recv: 0.0.0.0:4000\n  send: 127.0.0.1:4100\n\
                        payload_interface: 0.0.0.0:4001\n\
                        payload_telemetry:\n  source_base: 0.0.0.0:6000\n  \
                        dest_base: 127.0.0.1:6100\n\
                        beacon:\n  interval_ms: 5000\n  send: 0.0.0.0:5550\n  \
                        recv: 127.0.0.1:5650\n";

    const XML: &str = "<tcspecial>\
        <tcs_cmd><recv>0.0.0.0:4000</recv><send>127.0.0.1:4100</send></tcs_cmd>\
        <payload_interface>0.0.0.0:4001</payload_interface>\
        <payload_telemetry>\
          <source_base>0.0.0.0:6000</source_base>\
          <dest_base>127.0.0.1:6100</dest_base>\
        </payload_telemetry>\
        <beacon>\
          <interval_ms>5000</interval_ms>\
          <send>0.0.0.0:5550</send>\
          <recv>127.0.0.1:5650</recv>\
        </beacon>\
        </tcspecial>";

    #[test]
    fn test_load_tcspecial_config_every_format() {
        for (ext, text) in [(".yaml", YAML), (".xml", XML)] {
            let config = load_from(ext, text)
                .unwrap_or_else(|e| panic!("{ext} failed to load: {e}"));

            // The one attribute becomes an address and a port.
            assert_eq!(config.address, "0.0.0.0", "{ext}");
            assert_eq!(config.port, 4000, "{ext}");
            assert_eq!(config.payload_interface.to_string(), "0.0.0.0:4001", "{ext}");
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
    fn the_beacon_send_and_interval_are_required() {
        let config = load_from(".yaml", YAML).expect("the configuration loads");
        assert_eq!(config.beacon_send.to_string(), "0.0.0.0:5550");
        assert_eq!(config.beacon_recv.to_string(), "127.0.0.1:5650");
        assert_eq!(config.beacon_interval.0, 5000);

        for missing in [
            "  send: 0.0.0.0:5550\n  recv: 127.0.0.1:5650\n",
            "  interval_ms: 5000\n",
        ] {
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
            &YAML.replace(
                "send: 0.0.0.0:5550",
                "send: 0.0.0.0:5560",
            ),
        )
        .expect("the configuration loads");
        assert_eq!(from_the_file.beacon_send.to_string(), "0.0.0.0:5560");

        let section = |beacon: &str| CIConfigJson {
            tcs_cmd: tcslibgs::TcsCmdJson {
                recv: "0.0.0.0:4000".to_string(),
                send: "127.0.0.1:4100".to_string(),
            },
            payload_interface: "0.0.0.0:4001".to_string(),
            payload_telemetry: tcslibgs::PayloadTelemetryJson {
                source_base: "0.0.0.0:6000".to_string(),
                dest_base: "127.0.0.1:6100".to_string(),
            },
            beacon: tcslibgs::BeaconJson {
                interval_ms: 5000,
                send: beacon.to_string(),
                recv: "127.0.0.1:5650".to_string(),
            },
            log: tcslibgs::LogJson::default(),
        };

        // A set with a section: the set wins. It always states an address,
        // the attribute being required of a section as of a file.
        assert_eq!(
            beacon(&from_the_file, Some(&section("0.0.0.0:7550")))
                .expect("it resolves")
                .send
                .to_string(),
            "0.0.0.0:7550"
        );

        // No section at all, which a tcspecial configuration need not have.
        // This file's own address is for that set.
        assert_eq!(
            beacon(&from_the_file, None)
                .expect("it resolves")
                .send
                .to_string(),
            "0.0.0.0:5560"
        );

        // And a set stating something that is not an address is refused
        // rather than quietly leaving the file's in place.
        let said = beacon(&from_the_file, Some(&section("nowhere")))
            .expect_err("not an address")
            .to_string();
        assert!(said.contains("beacon.send"), "{said}");
        assert!(said.contains("nowhere"), "{said}");
    }

    /// A beacon address that is not one is refused where the file is read.
    ///
    /// Not where the beacon is sent: a beacon goes out on a timer with nobody
    /// to report to, so an address that cannot be parsed has to be caught
    /// while someone is still reading errors. It used to be a constant parsed
    /// with `unwrap`, which could only ever have panicked.
    #[test]
    fn a_beacon_send_that_is_not_one_is_refused() {
        for bad in [
            // No port, which every beacon address states now that beacons go
            // from one socket to another rather than to a multicast group.
            "0.0.0.0",
            "0.0.0.0:not-a-port",
            // Port 0 is whatever is free, which the other end cannot find.
            "0.0.0.0:0",
            ":5550",
            "",
        ] {
            let text = YAML.replace(
                "send: 0.0.0.0:5550",
                &format!("send: \"{bad}\""),
            );
            let said = load_from(".yaml", &text).expect_err(bad).to_string();
            assert!(
                said.contains("beacon.send"),
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
    fn a_command_interface_that_is_not_one_is_rejected() {
        // There is no protocol to get wrong any more -- the link between
        // tcspecial and the ground is UDP, so nothing states it -- and the
        // command interface took its place as the value that can be written
        // wrongly: half of it, or a port that is not one.
        for instead in [
            "recv: 0.0.0.0",
            "recv: 0.0.0.0:nought",
            "recv: :4000",
        ] {
            let bad = YAML.replace("recv: 0.0.0.0:4000", instead);
            assert_ne!(bad, YAML, "the test changed nothing");
            assert!(
                load_from(".yaml", &bad).is_err(),
                "{instead} was accepted"
            );
        }
    }

    // -- tcspecial configurations ------------------------------------------------------

    /// A tcspecial configuration in each format, written to a temporary file
    /// and loaded the way tcspecial loads one.
    fn load_payloads_from(ext: &str, text: &str) -> TcsResult<Vec<tcslibgs::DHConfig>> {
        let mut file = Builder::new().suffix(ext).tempfile().unwrap();
        file.write_all(text.as_bytes()).unwrap();
        file.flush().unwrap();
        tcslibgs::config::load_dh_configs(file.path())
    }

    const PAYLOADS_YAML: &str = "\
version: \"1.0\"
description: a bus and a peripheral
payloads:
  - name: thermal
    type: i2c
    path: /dev/i2c-1
    address: 0x48
    pec: true
    packet_size: 8
  - name: imu
    type: spi
    path: /dev/spidev0.0
    max_speed: 1000000
    spi_mode: 0
    packet_size: 8
";

    const PAYLOADS_XML: &str = r#"<payload-configuration>
  <version>1.0</version>
  <description>a bus and a peripheral</description>
  <payloads>
    <name>thermal</name>
    <type>i2c</type>
    <path>/dev/i2c-1</path>
    <address>0x48</address>
    <pec>true</pec>
    <packet_size>8</packet_size>
  </payloads>
  <payloads>
    <name>imu</name>
    <type>spi</type>
    <path>/dev/spidev0.0</path>
    <max_speed>1000000</max_speed>
    <spi_mode>0</spi_mode>
    <packet_size>8</packet_size>
  </payloads>
</payload-configuration>"#;

    #[test]
    fn a_payload_file_loads_in_either_format() {
        for (ext, text) in [(".yaml", PAYLOADS_YAML), (".xml", PAYLOADS_XML)] {
            let handlers =
                load_payloads_from(ext, text).unwrap_or_else(|e| panic!("{ext} failed: {e}"));
            assert_eq!(handlers.len(), 2, "{ext}");
            assert_eq!(handlers[0].name, tcslibgs::DHName::new("thermal"), "{ext}");
            assert_eq!(handlers[1].name, tcslibgs::DHName::new("imu"), "{ext}");
        }
    }

    #[test]
    fn the_formats_agree_about_a_payload_file() {
        assert_eq!(
            load_payloads_from(".yaml", PAYLOADS_YAML).unwrap(),
            load_payloads_from(".xml", PAYLOADS_XML).unwrap()
        );
    }

    #[test]
    fn a_rule_violation_reaches_the_caller() {
        // The rules live in the library; what this checks is that a violation
        // arrives here as an error rather than as a surprising default.
        let reserved = PAYLOADS_YAML.replace("address: 0x48", "address: 0x00");
        let e = load_payloads_from(".yaml", &reserved).unwrap_err();
        assert!(format!("{e}").contains("reserved"), "got {e}");

        let no_mode = PAYLOADS_YAML.replace("    spi_mode: 0\n", "");
        assert!(load_payloads_from(".yaml", &no_mode).is_err());
    }
}
