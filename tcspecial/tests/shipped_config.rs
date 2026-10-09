//! The configuration files tcspecial ships with must load.
//!
//! Every other test of the loaders builds its input from an inline string, so
//! a loader change that the real files do not follow goes unnoticed. That has
//! happened: when CI configuration moved to a flat `CIConfigJson`,
//! `tcspecial/src/tcspecial.yaml` still carried the `tcspecial_config`
//! wrapper the loader had stopped expecting, and every test stayed green
//! while the program could not read its own configuration.
//!
//! These tests read the files themselves. They deliberately assert little
//! about the values, because the point is to catch a file that no longer
//! matches its loader, not to freeze a port number -- but they do assert
//! enough that a file parsing into nothing but defaults would fail.

use std::path::{Path, PathBuf};

use tcslibgs::config::load_dh_configs;
use tcslibgs::config_digest::digest_of_file;
use tcslibgs::{EndpointConfig, NetworkProtocol};
use tcspecial::config::load_tcspecial_config;

/// Where the shipped payload sets live.
///
/// They are there rather than at the repository root because they are run by
/// hand: every one of them opens sockets or device nodes that only a person
/// at a terminal should be opening.
const MANUAL_SETS: &str = "tests/manual";

/// A path relative to the repository root.
fn repo_file(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

/// Every shipped payload file.
///
/// Discovered rather than listed, so a payload set added to the repository is
/// covered by these tests without anyone having to remember to name it here.
/// The simulator files beside them are tcssim's business, not tcspecial's.
fn shipped_payload_files() -> Vec<PathBuf> {
    let root = repo_file(MANUAL_SETS);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("the repository root is readable")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .and_then(|name| name.strip_prefix("tcspecial"))
                .and_then(|rest| rest.strip_suffix(".yaml"))
                .is_some_and(|stem| !stem.ends_with("sim"))
        })
        .collect();

    files.sort();
    assert!(
        !files.is_empty(),
        "no payload files found in {}",
        root.display()
    );
    files
}

#[test]
fn the_shipped_tcspecial_config_loads() {
    let path = repo_file("tcspecial/src/tcspecial.yaml");
    let config = load_tcspecial_config(&path)
        .unwrap_or_else(|e| panic!("{} failed to load: {e}", path.display()));

    // Nothing here pins a value. Each check fails for a file that parsed but
    // produced a configuration the program could not act on.
    assert!(!config.address.is_empty(), "no address");
    assert_ne!(config.port, 0, "no port");
    assert!(
        matches!(
            config.protocol,
            NetworkProtocol::Tcp
                | NetworkProtocol::Udp
                | NetworkProtocol::UnixStream
                | NetworkProtocol::UnixDgram
        ),
        "protocol did not parse into a known transport"
    );
    assert_ne!(config.beacon_interval.0, 0, "no beacon interval");
    assert_ne!(config.log_segment_bytes, 0, "no log segment size");
}

/// Every shipped payload set is written in all three formats, and the three
/// describe the same data handlers.
///
/// The formats are three spellings of one configuration, which is a claim
/// nothing in the code enforces: each is parsed by its own parser, and a set
/// transcribed into another format by hand can differ in a port or lose an
/// attribute without either file becoming invalid. `make runmocx` and
/// `make runmocj` run the XML and the JSON of a set, so a difference between
/// them is a difference in what the programs are given.
#[test]
fn every_shipped_set_reads_the_same_in_all_three_formats() {
    for yaml in shipped_payload_files() {
        let from_yaml = load_dh_configs(&yaml)
            .unwrap_or_else(|e| panic!("{} failed to load: {e}", yaml.display()));

        for ext in ["xml", "json"] {
            let other = yaml.with_extension(ext);
            assert!(
                other.exists(),
                "{} has no {} beside it: a set is written in all three formats",
                yaml.display(),
                other.display()
            );

            let from_other = load_dh_configs(&other)
                .unwrap_or_else(|e| panic!("{} failed to load: {e}", other.display()));

            assert_eq!(
                from_other,
                from_yaml,
                "{} and {} describe different data handlers",
                other.display(),
                yaml.display()
            );

            // And they digest the same, which is the claim the two ends of a
            // link make to each other: a tcsmoc reading the JSON of a set and
            // a tcspecial reading its XML have read the same set, and a digest
            // that said otherwise would make the check worse than none.
            assert_eq!(
                digest_of_file(&other).unwrap_or_else(|e| panic!("{}: {e}", other.display())),
                digest_of_file(&yaml).unwrap_or_else(|e| panic!("{}: {e}", yaml.display())),
                "{} and {} digest differently",
                other.display(),
                yaml.display()
            );
        }
    }
}

#[test]
fn the_shipped_payload_configs_load() {
    for path in shipped_payload_files() {
        let handlers = load_dh_configs(&path)
            .unwrap_or_else(|e| panic!("{} failed to load: {e}", path.display()));

        let file = path.display();
        assert!(!handlers.is_empty(), "{file} has no data handlers");

        for dh in &handlers {
            assert!(
                !dh.name.0.is_empty(),
                "{file}: data handler {:?} has no name",
                dh.dh_id
            );
            assert_ne!(
                dh.packet_size, 0,
                "{file}: {:?} has a zero packet size",
                dh.dh_id
            );
            match &dh.endpoint {
                EndpointConfig::Network(net) => {
                    assert!(
                        !net.address.is_empty(),
                        "{file}: {:?} has no address",
                        dh.dh_id
                    );
                    assert_ne!(net.port, 0, "{file}: {:?} has no port", dh.dh_id);
                }
                EndpointConfig::Device(dev) => {
                    assert!(
                        !dev.path.is_empty(),
                        "{file}: {:?} has no device path",
                        dh.dh_id
                    );
                }
                // The kinds that are device files with terms of their own:
                // what is checked here is only that each is located, since
                // what the terms should be is the file's business.
                EndpointConfig::Serial(serial) => {
                    assert!(
                        !serial.path.is_empty(),
                        "{file}: {:?} has no serial device",
                        dh.dh_id
                    );
                    assert_ne!(
                        serial.datarate, 0,
                        "{file}: {:?} has no data rate",
                        dh.dh_id
                    );
                }
                EndpointConfig::I2c(i2c) => {
                    assert!(!i2c.bus.is_empty(), "{file}: {:?} has no bus", dh.dh_id);
                }
                EndpointConfig::Spi(spi) => {
                    assert!(
                        !spi.path.is_empty(),
                        "{file}: {:?} has no SPI device",
                        dh.dh_id
                    );
                    assert_ne!(
                        spi.max_speed, 0,
                        "{file}: {:?} has no clock rate",
                        dh.dh_id
                    );
                }
            }
        }
    }
}

#[test]
fn the_shipped_files_have_distinct_data_handler_ids() {
    // The loader refuses a duplicate id now, so this says the shipped files
    // get past that rule rather than being the only thing that enforces it,
    // which is what it was before the rule existed.
    for path in shipped_payload_files() {
        let handlers = load_dh_configs(&path).unwrap();
        let mut ids: Vec<_> = handlers.iter().map(|dh| dh.dh_id).collect();
        let before = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(
            before,
            ids.len(),
            "{}: two data handlers share a dh_id",
            path.display()
        );
    }
}

/// Every shipped handler can be started.
///
/// A handler needs an OC address to run: without one it has nowhere to send
/// what it reads from its payload. The payload format leaves it optional,
/// because a file describing payloads is complete without it, so nothing but a
/// test of the real files catches a shipped handler that StartDH would refuse.
#[test]
fn every_shipped_handler_has_an_oc_address() {
    for path in shipped_payload_files() {
        for dh in load_dh_configs(&path).unwrap() {
            let oc = dh.oc.unwrap_or_else(|| {
                panic!(
                    "{}: {} has no OC address, so it cannot be started",
                    path.display(),
                    dh.name.0
                )
            });
            assert!(!oc.address.is_empty(), "{}: empty OC address", dh.name.0);
            assert_ne!(oc.port, 0, "{}: no OC port", dh.name.0);
        }
    }
}

/// No two shipped handlers share an OC port, and none collides with a payload
/// port on the same host.
///
/// Two handlers binding one address is a start that fails at the second, and
/// an OC port equal to a payload port on the same host is the same collision
/// one step further away.
#[test]
fn shipped_handlers_do_not_share_a_port() {
    for path in shipped_payload_files() {
        let handlers = load_dh_configs(&path).unwrap();
        let mut bound: Vec<(String, u16, String)> = Vec::new();

        for dh in &handlers {
            if let Some(oc) = &dh.oc {
                bound.push((oc.address.clone(), oc.port, format!("{} OC", dh.name.0)));
            }
            if let EndpointConfig::Network(net) = &dh.endpoint {
                bound.push((
                    net.address.clone(),
                    net.port,
                    format!("{} payload", dh.name.0),
                ));
            }
        }

        for (i, (address, port, what)) in bound.iter().enumerate() {
            for (other_address, other_port, other) in &bound[i + 1..] {
                // localhost and 127.0.0.1 are the same host under two names,
                // so compare the ports and say so when they match. The loader
                // refuses a collision now, so this says the shipped files get
                // past that rule rather than being what enforces it.
                let same_host = address == other_address
                    || ["localhost", "127.0.0.1"].contains(&address.as_str())
                        && ["localhost", "127.0.0.1"].contains(&other_address.as_str());
                assert!(
                    !(same_host && port == other_port),
                    "{}: {what} and {other} both want {address}:{port}",
                    path.display()
                );
            }
        }
    }
}
