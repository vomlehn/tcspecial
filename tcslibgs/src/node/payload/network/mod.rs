//! Network payloads: a socket, of one of the four protocols or a raw one.
//!
//! Only how a network payload's stand-in is told apart is here. What a
//! network node may say is shared with the ground link, and is
//! `crate::graph::reach`'s: see `crate::node`, which says why there is no
//! `moc` directory.

use std::net::IpAddr;

use crate::types::{NetworkConfig, NetworkProtocol};

/// What `--simulate` asks to change on a network node that is not a
/// stand-in.
const TO_STAND_IN: &str =
    "give address as a loopback address, such as 127.0.0.1";

/// Whether the far end of a network payload is on this host. See
/// [`crate::simulate`].
///
/// The weakest of the judgements, and said to be: an address on this host
/// says only that the far end is on this host, so a local program in front
/// of real hardware passes it. A datagram payload is judged by the address
/// the node states, which is the one the vehicle binds where the payload
/// speaks first, so under `all` it must be a loopback address and nothing off
/// the host can be the one that answers.
pub fn judged(at: &NetworkConfig) -> crate::simulate::Judged {
    use crate::simulate::Judged;
    let address = at.address.as_str();
    match at.protocol {
        NetworkProtocol::UnixStream | NetworkProtocol::UnixDgram => {
            Judged::stand_in(
                format!(
                    "{address} is a Unix-domain path, so the far end is on \
                     this host"
                ),
                "address",
                TO_STAND_IN,
            )
        }
        NetworkProtocol::Tcp
        | NetworkProtocol::Udp
        | NetworkProtocol::Raw(_) => {
            match address.parse::<IpAddr>() {
                Ok(ip) if ip.is_loopback() => Judged::stand_in(
                    format!(
                        "{address} is a loopback address, so the far end is on \
                         this host"
                    ),
                    "address",
                    TO_STAND_IN,
                ),
                Ok(ip) if ip.is_unspecified() => Judged::cannot_tell(
                    format!(
                        "{address} is every address this host has, so the far \
                         end may be another host"
                    ),
                    "address",
                    TO_STAND_IN,
                ),
                Ok(_) => Judged::hardware(
                    format!(
                        "{address} is not a loopback address, so the far end \
                         may be another host"
                    ),
                    "address",
                    TO_STAND_IN,
                ),
                Err(_) if address == "localhost" => Judged::stand_in(
                    format!("{address} is this host, so the far end is on it"),
                    "address",
                    TO_STAND_IN,
                ),
                // Not looked up: what a name resolves to is the resolver's
                // answer today, and the judgement is one that has to hold.
                Err(_) => Judged::cannot_tell(
                    format!(
                        "{address} is a host name, which is not looked up to \
                         judge it"
                    ),
                    "address",
                    TO_STAND_IN,
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulate::Verdict;

    fn at(protocol: NetworkProtocol, address: &str) -> Verdict {
        judged(&NetworkConfig {
            protocol,
            address: address.to_string(),
            port: 7000,
        })
        .verdict
    }

    #[test]
    fn only_this_host_is_a_stand_in() {
        assert_eq!(at(NetworkProtocol::Tcp, "127.0.0.1"), Verdict::StandIn);
        assert_eq!(at(NetworkProtocol::Udp, "::1"), Verdict::StandIn);
        assert_eq!(at(NetworkProtocol::Udp, "localhost"), Verdict::StandIn);
        assert_eq!(
            at(NetworkProtocol::UnixStream, "/tmp/x.sock"),
            Verdict::StandIn
        );
        assert_eq!(at(NetworkProtocol::Udp, "0.0.0.0"), Verdict::CannotTell);
        assert_eq!(at(NetworkProtocol::Tcp, "10.0.0.5"), Verdict::Hardware);
        assert_eq!(
            at(NetworkProtocol::Tcp, "payload.local"),
            Verdict::CannotTell
        );
    }
}
