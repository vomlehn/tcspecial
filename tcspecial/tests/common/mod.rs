//! What more than one of these tests does as the ground.

use std::net::{SocketAddr, UdpSocket};
use std::path::Path;
use std::time::{Duration, Instant};

use tcslibgs::config_digest::{digest_of_graph, ConfigDigest, ConfigVersion};
use tcslibgs::graph::Graph;
use tcslibgs::{Command, ConnectCommand, Telemetry};

/// The checksum of the configuration in `path`, as tcspecial computes it.
///
/// # Arguments
///
/// * `path` - The configuration.
///
/// # Returns
///
/// Its checksum.
///
/// # Panics
///
/// If it does not load or cannot be digested.
#[allow(dead_code)]
pub fn checksum_of(path: &Path) -> ConfigDigest {
    digest_of_graph(&Graph::load(path).expect("it reads")).expect("a digest")
}

/// Sends CONNECT with `digest` from `from` to `to` until tcspecial answers,
/// as a ground station does once tcspecial is up: until then nothing goes
/// down but beacons, and a CONNECT carrying the configuration's checksum is
/// what opens the downlink.
///
/// # Arguments
///
/// * `from`     - Where the configuration's `tcs_cmd.send` says the ground
///                sends from, with a short read timeout.
/// * `to`       - Where it says tcspecial takes them, `tcs_cmd.recv`.
/// * `sequence` - The CONNECT's sequence number.
/// * `digest`   - The checksum it carries.
///
/// # Returns
///
/// The answer.
///
/// # Panics
///
/// If nothing answers within 20 seconds.
#[allow(dead_code)]
pub fn connect(
    from: &UdpSocket,
    to: SocketAddr,
    sequence: u32,
    digest: ConfigDigest,
) -> Telemetry {
    let command = Command::Connect(ConnectCommand::new(
        sequence,
        ConfigVersion::of_this_build(),
        digest,
    ))
    .to_bytes();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut buffer = [0u8; 65535];
    while Instant::now() < deadline {
        from.send_to(&command, to).expect("sent");
        let wait = Instant::now() + Duration::from_millis(500);
        while Instant::now() < wait {
            if let Ok(got) = from.recv(&mut buffer) {
                if let Ok(answer) = Telemetry::read(&buffer[..got]) {
                    return answer;
                }
            }
        }
    }
    panic!("nothing answered a CONNECT at {to}");
}
