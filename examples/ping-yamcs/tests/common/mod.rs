//! What the tests read out of the example's configuration through
//! `tcslibgs`: the Rust's answers, which the Python's and the Yamcs
//! wrapper's are held to. How the wrapper is generated and read is
//! `yamcs_common`'s, which thermostat-yamcs's tests share.

use std::path::{Path, PathBuf};

use tcslibgs::config_digest::digest_of_file;
use tcslibgs::graph::parameters::{beacon_of, ci_config_of, flatten};
use tcslibgs::graph::reach::{apid_base_of, reach_of};
use tcslibgs::graph::telemetry::{self, Via};
use tcslibgs::graph::Graph;
use tcslibgs::EndpointConfig;

/// Where the example is.
///
/// # Returns
///
/// Its directory.
pub fn here() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// The address to reach a socket at from this host, as `ping_yamcs.py`'s
/// `local` gives it: Yamcs, on this host, reaches a socket bound on every
/// interface at this host's own address.
///
/// # Arguments
///
/// * `address` - The address the socket is bound at.
///
/// # Returns
///
/// The address to send to.
fn local(address: &str) -> String {
    match address {
        "0.0.0.0" | "::" | "" => "127.0.0.1".to_string(),
        other => other.to_string(),
    }
}

/// The Rust's answers to the same questions, from the YAML spelling.
///
/// # Returns
///
/// The ports, the APIDs and the digest, as `ping_yamcs.py --describe` names
/// them. Not the settings that are Yamcs's own -- its web port and its
/// directories -- which the configuration does not say.
///
/// # Panics
///
/// The example's configuration does not read or lacks any of these.
pub fn worked_out() -> Vec<(String, String)> {
    let path = here().join("ping-yamcs.ytcs");
    let graph = Graph::load(&path).expect("the example reads");

    let section = graph.tcspecial.as_ref().expect("a tcspecial section");
    let ci = ci_config_of(&flatten(section))
        .expect("its section reads")
        .to_ci_config()
        .expect("its section settles");
    let sockets = ci.sockets().expect("its links are sockets");
    let framing = sockets.framing.expect("its packets are framed");

    let beacon = beacon_of(&graph)
        .expect("its section reads")
        .expect("it has one");

    // The one gateway_yamcs node and the one ping node, as the Python finds
    // them: by their types.
    let one_of = |kind: &str| {
        let mut found =
            graph.nodes.iter().filter(|node| node.node_type == kind);
        let node = found.next().unwrap_or_else(|| panic!("a {kind} node"));
        assert!(found.next().is_none(), "one {kind} node");
        node
    };
    let gateway = one_of("gateway_yamcs");
    let ping = one_of("ping");

    let uplink = match reach_of(gateway).expect("it is reached").endpoint {
        EndpointConfig::Network(network) => network,
        other => panic!("a gateway node is a socket: {other:?}"),
    };
    let uplink_apid = apid_base_of(gateway)
        .expect("its apid_base reads")
        .expect("it has one");

    let answer = telemetry::of(&graph)
        .expect("its links to the ground work out")
        .into_iter()
        .find(|link| {
            link.source == ping.name
                && link.via == Via::Gateway(gateway.name.clone())
        })
        .expect("ping sends to the gateway node");
    let downlink = answer.ground().expect("a link over UDP").clone();
    let reply_apid = answer.apid.expect("its packets carry an APID");

    [
        ("cmd_host", local(&sockets.address)),
        ("cmd_port", sockets.port.to_string()),
        ("uplink_host", local(&uplink.address)),
        ("uplink_port", uplink.port.to_string()),
        ("downlink_port", downlink.port.to_string()),
        ("beacon_port", beacon.recv.port.to_string()),
        ("beacon_apid", framing.beacon_apid.to_string()),
        ("cmd_apid", framing.cmd_apid.to_string()),
        ("uplink_apid", uplink_apid.to_string()),
        ("reply_apid", reply_apid.to_string()),
        (
            "digest",
            digest_of_file(&path).expect("it digests").to_string(),
        ),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_string(), value))
    .collect()
}

/// One of the Rust's answers, by the name `ping_yamcs.py --describe`
/// gives it.
///
/// # Arguments
///
/// * `name` - Its name.
///
/// # Returns
///
/// The answer.
///
/// # Panics
///
/// There is no answer of that name.
// Each test file is a crate of its own that takes the share of this module
// it needs, so what one does not use is not dead.
#[allow(dead_code)]
pub fn worked_out_of(name: &str) -> String {
    worked_out()
        .into_iter()
        .find(|(said, _)| said == name)
        .map(|(_, value)| value)
        .unwrap_or_else(|| panic!("no {name} is worked out"))
}

/// A copy of the example's configuration with a payload more drawn into
/// the gateway node, "pong", and two payloads nothing touches, "sensor" and
/// "tank": `test_ping_yamcs.with_more_nodes`, the one place it is written.
///
/// # Arguments
///
/// * `dir` - Where to write it.
///
/// # Returns
///
/// The copy's path.
///
/// # Panics
///
/// The Python fails.
#[allow(dead_code)]
pub fn with_more_nodes(dir: &Path) -> PathBuf {
    let said = yamcs_common::python(
        "from pathlib import Path\n\
         import test_ping_yamcs\n\
         print(test_ping_yamcs.with_more_nodes(Path(sys.argv[1])))\n",
        &[&here()],
        &[dir],
    );
    PathBuf::from(said.trim())
}
