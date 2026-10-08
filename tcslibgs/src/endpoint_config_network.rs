//! Network groups: the transport a payload link runs over.
//!
//! The one kind of group whose endpoints are named by an address rather than
//! by a device, and the only one where what a file must say depends on what
//! it has already said: a stream protocol needs a rule for where a read ends,
//! and a datagram protocol must not give one, the datagram being the frame
//! already.

use serde::Serialize;

use crate::endpoint_config::{validate_stream, StreamParams};
use crate::types::NetworkProtocol;
use crate::endpoint_config::{
    bad, reject_foreign_fields, require, EndpointConfigError, EndpointConfigResult,
    GroupKind, GroupWire,
};

/// Attributes of a network group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NetworkParams {
    /// Transport carrying the payload data.
    pub protocol: NetworkProtocol,
    /// Stream payload protocol attributes. Present for the stream protocols
    /// and absent for the datagram protocols, where a datagram is the frame.
    pub stream: Option<StreamParams>,
}


/// Which transport a file's spelling names.
///
/// The spellings themselves are [`NetworkProtocol`]'s, so this file and a
/// payload file cannot come to name the same transport differently. Hyphens
/// are accepted here, where every attribute name accepts them too.
fn parse_protocol(group: &str, text: &str) -> EndpointConfigResult<NetworkProtocol> {
    let written = text.trim().to_ascii_lowercase().replace('-', "_");
    NetworkProtocol::from_spelling(&written).ok_or_else(|| {
        bad(
            group,
            &format!(
                "protocol: \"{}\" is not one of {}",
                text.trim(),
                NetworkProtocol::spellings()
            ),
        )
    })
}

/// Whether one read of this protocol needs a rule for where it ends.
///
/// A stream has no frames of its own, so something must say where a payload
/// stops; a datagram is the frame already.
fn is_stream_protocol(p: NetworkProtocol) -> bool {
    matches!(
        p,
        NetworkProtocol::Tcp | NetworkProtocol::UnixStream
    )
}

/// What a network group says, and what it must say.
///
/// Whether a stream section is required follows from the protocol: a stream
/// protocol needs a rule for where a read ends, and for a datagram protocol
/// the datagram is already the frame, so one given there is an error rather
/// than a section with no effect.
pub(crate) fn group_kind_of(
    name: &str,
    g: GroupWire,
) -> EndpointConfigResult<GroupKind> {

    reject_foreign_fields(name, "network", &g, &["protocol"])?;

    let protocol = require(name, "network", "protocol", g.protocol.as_ref())?;
    let protocol = parse_protocol(name, protocol)?;

    // The stream protocols need a rule for where a read ends; for the
    // datagram protocols the datagram is already the frame.
    let stream = match (is_stream_protocol(protocol), g.stream) {
        (true, Some(s)) => Some(validate_stream(name, s)?),
        (true, None) => {
            return Err(EndpointConfigError::MissingGroupField {
                group: name.to_string(),
                kind: "network",
                field: "a stream section",
            })
        }
        (false, Some(_)) => {
            return Err(EndpointConfigError::UnusedGroupField {
                group: name.to_string(),
                kind: "datagram network",
                field: "a stream section",
            })
        }
        (false, None) => None,
    };

    Ok(GroupKind::Network(NetworkParams { protocol, stream }))
}
