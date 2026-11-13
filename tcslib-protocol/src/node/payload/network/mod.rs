//! Network payloads: a socket, of one of the four protocols or a raw one.
//!
//! Only how a simulated device reached over the network is told apart is here:
//! the UDP socket at a node's `sim_address`, where tcssim answers. A `network`
//! node itself is a real device, whatever answers at its far end, and the mode
//! does not count it: see [`crate::simulate::counted`]. What a network node may
//! say is shared with the ground link, and is `crate::graph::reach`'s: see
//! `crate::node`, which says why there is no `gateway` directory.

use std::net::IpAddr;

use crate::types::{NetworkConfig, NetworkProtocol};

/// What `--simulate` asks to change where the far end may be off this host.
/// [`crate::simulate::judge`] says what to change of `sim_address` instead,
/// which is the one network endpoint it asks about.
const TO_SIMULATE: &str =
    "give address as a loopback address, such as 127.0.0.1";

/// Judges whether the far end of a network endpoint is on this host: for a node
/// reached at its `sim_address`, whether it is tcssim's simulated device. See
/// [`crate::simulate`].
///
/// The weakest of the judgements, and said to be: an address on this host says
/// only that the far end is on this host, so a local program in front of real
/// hardware passes it. A `sim_address` is always a UDP address on this host, as
/// `crate::graph::reach` holds it to be; the other arms judge what that rule
/// never lets through, and are kept so that the judgement says something of any
/// endpoint it is handed.
///
/// # Arguments
///
/// * `at` - The endpoint's protocol and address.
///
/// # Returns
///
/// The judgement: a simulated device for a Unix-domain path, a loopback address
/// or `localhost`; cannot-tell for the unspecified address or a host name;
/// hardware otherwise.
pub fn judged(at: &NetworkConfig) -> crate::simulate::Judged {
    use crate::simulate::Judged;
    let address = at.address.as_str();
    match at.protocol {
        NetworkProtocol::UnixStream | NetworkProtocol::UnixDgram => {
            Judged::simulated(
                format!(
                    "{address} is a Unix-domain path, so the far end is on \
                     this host"
                ),
                "address",
                TO_SIMULATE,
            )
        }
        NetworkProtocol::Tcp
        | NetworkProtocol::Udp
        | NetworkProtocol::Raw(_) => {
            match address.parse::<IpAddr>() {
                Ok(ip) if ip.is_loopback() => Judged::simulated(
                    format!(
                        "{address} is a loopback address, so the far end is on \
                         this host"
                    ),
                    "address",
                    TO_SIMULATE,
                ),
                Ok(ip) if ip.is_unspecified() => Judged::cannot_tell(
                    format!(
                        "{address} is every address this host has, so the far \
                         end may be another host"
                    ),
                    "address",
                    TO_SIMULATE,
                ),
                Ok(_) => Judged::hardware(
                    format!(
                        "{address} is not a loopback address, so the far end \
                         may be another host"
                    ),
                    "address",
                    TO_SIMULATE,
                ),
                Err(_) if address == "localhost" => Judged::simulated(
                    format!("{address} is this host, so the far end is on it"),
                    "address",
                    TO_SIMULATE,
                ),
                // Not looked up: what a name resolves to is the resolver's
                // answer today, and the judgement is one that has to hold.
                Err(_) => Judged::cannot_tell(
                    format!(
                        "{address} is a host name, which is not looked up to \
                         judge it"
                    ),
                    "address",
                    TO_SIMULATE,
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulate::Verdict;

    /// Judges a network node on port 7000.
    ///
    /// # Arguments
    ///
    /// * `protocol` - The node's protocol.
    /// * `address`  - The node's address.
    ///
    /// # Returns
    ///
    /// The verdict [`judged`] gives it.
    fn at(protocol: NetworkProtocol, address: &str) -> Verdict {
        judged(&NetworkConfig {
            protocol,
            address: address.to_string(),
            port: 7000,
        })
        .verdict
    }

    #[test]
    fn only_this_host_is_simulated() {
        assert_eq!(at(NetworkProtocol::Tcp, "127.0.0.1"), Verdict::Simulated);
        assert_eq!(at(NetworkProtocol::Udp, "::1"), Verdict::Simulated);
        assert_eq!(at(NetworkProtocol::Udp, "localhost"), Verdict::Simulated);
        assert_eq!(
            at(NetworkProtocol::UnixStream, "/tmp/x.sock"),
            Verdict::Simulated
        );
        assert_eq!(at(NetworkProtocol::Udp, "0.0.0.0"), Verdict::CannotTell);
        assert_eq!(at(NetworkProtocol::Tcp, "10.0.0.5"), Verdict::Hardware);
        assert_eq!(
            at(NetworkProtocol::Tcp, "payload.local"),
            Verdict::CannotTell
        );
    }
}
