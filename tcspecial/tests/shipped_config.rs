//! The configuration files tcspecial ships with must load.
//!
//! Every other test of the loaders builds its input from an inline string, so
//! a loader change that the real files do not follow goes unnoticed. That has
//! happened: when CI configuration moved to a flat `CIConfigJson`,
//! `tcspecial/src/tcspecial.json` still carried the `tcspecial_config`
//! wrapper the loader had stopped expecting, and every test stayed green
//! while the program could not read its own configuration.
//!
//! These tests read the files themselves. They deliberately assert little
//! about the values, because the point is to catch a file that no longer
//! matches its loader, not to freeze a port number -- but they do assert
//! enough that a file parsing into nothing but defaults would fail.

use std::path::{Path, PathBuf};

use tcslibgs::config::load_payload_config;
use tcslibgs::{EndpointConfig, NetworkProtocol};
use tcspecial::config::load_tcspecial_config;

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
    let root = repo_file(".");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("the repository root is readable")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .and_then(|name| name.strip_prefix("payload"))
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
    let path = repo_file("tcspecial/src/tcspecial.json");
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

#[test]
fn the_shipped_payload_configs_load() {
    for path in shipped_payload_files() {
        let handlers = load_payload_config(&path)
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
            }
        }
    }
}

#[test]
fn the_shipped_files_have_distinct_data_handler_ids() {
    // A duplicate id parses cleanly and then has one handler shadow another,
    // which is the kind of thing only a test of the real file catches.
    for path in shipped_payload_files() {
        let handlers = load_payload_config(&path).unwrap();
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
