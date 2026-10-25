//! rpmsg payloads: a channel to a co-processor.
//!
//! **Experimental, and not yet supported.** The terms parse and reach a
//! handler, and the handler reads and writes the device; nothing has been run
//! against a remote processor.
//!
//! A modern flight computer is commonly one package with more than one kind of
//! core in it -- a Linux core and one or two real-time cores -- and the
//! payload is software on the other core rather than anything on a wire. Linux
//! talks to it over rpmsg: the remote announces a named channel, Linux makes a
//! character device for it, and the two ends exchange whole messages through
//! that device.
//!
//! So an rpmsg payload is a device file read and written in both directions,
//! with two things to settle that no other kind has.
//!
//! The first is which channel it is. The device node's number is assigned as
//! channels appear, so `/dev/rpmsg0` is whichever channel announced itself
//! first -- the same hazard an I2C stub adapter has, where the bus number
//! depends on what else is loaded. What identifies a channel is the service
//! name the remote announced, so a file states it, and a handler can hold the
//! node it was given against it.
//!
//! The second is how much one message carries. rpmsg messages go in buffers
//! the transport fixes the size of: the usual virtio configuration has 512-byte
//! buffers with a 16-byte header, leaving 496 bytes, and a write longer than
//! that is refused by the driver. A payload whose packets will not fit is
//! refused here instead.

use serde::{Deserialize, Serialize};

use crate::endpoint_config::{
    bad, parse_u32, reject_foreign_fields, reject_stream, require,
    EndpointConfigResult, GroupKind, LinkTerms,
};

/// What an rpmsg payload's channel is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpmsgParams {
    /// The service name the remote announced the channel with.
    ///
    /// Required, because the node's number is not an identity: it is assigned
    /// as channels appear. A handler holds the node it was given against this
    /// where the kernel will say -- see `tcspecial::endpoint_rpmsg`.
    pub service: String,
    /// The greatest number of bytes one message may carry, where the file
    /// says something other than the usual.
    ///
    /// `None` is [`USUAL_MESSAGE_BYTES`], which is what a virtio transport
    /// with 512-byte buffers leaves. A platform with larger buffers says so
    /// here, and one with smaller ones must.
    pub max_message_bytes: Option<u32>,
}

/// What one message carries where the transport is the usual one.
///
/// 512-byte buffers less the 16-byte rpmsg header. A write longer than this
/// is refused by the driver rather than split, rpmsg having no
/// fragmentation.
pub const USUAL_MESSAGE_BYTES: u32 = 496;

/// How much a payload's messages may carry, by what its file said.
pub fn message_bytes(max_message_bytes: Option<u32>) -> u32 {
    max_message_bytes.unwrap_or(USUAL_MESSAGE_BYTES)
}

/// What an rpmsg payload says about its channel, and what it must say.
pub(crate) fn group_kind_of(
    name: &str,
    g: LinkTerms,
) -> EndpointConfigResult<GroupKind> {
    reject_foreign_fields(
        name,
        "rpmsg",
        &g,
        &["service", "max_message_bytes"],
    )?;

    // A read of an rpmsg device hands over one whole message, so there is
    // nothing for a rule about where a read ends to say.
    reject_stream(name, "rpmsg", g.stream.is_some())?;

    let service = require(name, "rpmsg", "service", g.service.as_ref())?;
    let service = service.as_str().to_string();
    if service.is_empty() {
        return Err(bad(
            name,
            "service must not be empty: it is the name the remote announced \
             the channel with, and the only thing that identifies it",
        ));
    }

    let max_message_bytes = match g.max_message_bytes.as_ref() {
        Some(stated) => {
            let bytes = parse_u32(name, "max_message_bytes", stated)?;
            if bytes == 0 {
                return Err(bad(
                    name,
                    "max_message_bytes must be greater than zero: a message of \
                     no bytes is not a message",
                ));
            }
            Some(bytes)
        }
        None => None,
    };

    Ok(GroupKind::Rpmsg(RpmsgParams {
        service,
        max_message_bytes,
    }))
}

/// Whether the device at `path` is a stand-in: a pseudo-terminal, which is
/// what tcssim makes for a rpmsg payload. See [`crate::simulate`].
pub fn judged(path: &str) -> crate::simulate::Judged {
    crate::simulate::pseudo_terminal(
        path,
        "path",
        "start tcssim first, which makes a pseudo-terminal at path",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoint_config::Scalar;

    fn terms(stated: &[(&str, &str)]) -> LinkTerms {
        let mut g = LinkTerms::default();
        for (field, value) in stated {
            let value = Some(Scalar::of(*value));
            match *field {
                "service" => g.service = value,
                "max_message_bytes" => g.max_message_bytes = value,
                "datarate" => g.datarate = value,
                other => panic!("{other} is not a term this test states"),
            }
        }
        g
    }

    fn params(stated: &[(&str, &str)]) -> RpmsgParams {
        match group_kind_of("remote", terms(stated)).expect("a channel") {
            GroupKind::Rpmsg(rpmsg) => rpmsg,
            other => panic!("expected an rpmsg channel, got {other:?}"),
        }
    }

    /// A channel states the service name it was announced with.
    ///
    /// Required, because a device node's number is not an identity: it is
    /// assigned as channels appear, so the node a file names may be another
    /// channel's on the next boot.
    #[test]
    fn a_channel_states_the_service_it_is() {
        assert_eq!(
            params(&[("service", "payload-telemetry")]),
            RpmsgParams {
                service: "payload-telemetry".to_string(),
                max_message_bytes: None,
            }
        );

        let said = group_kind_of("remote", terms(&[]))
            .expect_err("a channel with no name");
        assert!(
            format!("{said}").contains("service"),
            "the error should name what is missing: {said}"
        );

        let said = group_kind_of("remote", terms(&[("service", "")]))
            .expect_err("a channel named nothing");
        assert!(
            format!("{said}").contains("announced"),
            "the error should say what the name is for: {said}"
        );
    }

    /// What a message carries: the usual size, or what the platform says.
    #[test]
    fn a_message_is_the_usual_size_or_what_the_file_says() {
        assert_eq!(message_bytes(None), USUAL_MESSAGE_BYTES);
        assert_eq!(message_bytes(Some(2048)), 2048);

        assert_eq!(
            params(&[("service", "x"), ("max_message_bytes", "2048")])
                .max_message_bytes,
            Some(2048)
        );

        let said = group_kind_of(
            "remote",
            terms(&[("service", "x"), ("max_message_bytes", "0")]),
        )
        .expect_err("a message of no bytes");
        assert!(
            format!("{said}").contains("max_message_bytes"),
            "the error should name the term: {said}"
        );
    }

    /// A term of another kind of link is refused.
    #[test]
    fn a_term_of_another_link_is_refused() {
        let said = group_kind_of(
            "remote",
            terms(&[("service", "x"), ("datarate", "9600")]),
        )
        .expect_err("a line's rate on a channel");
        assert!(
            format!("{said}").contains("datarate"),
            "the error should name the term: {said}"
        );
    }
}
