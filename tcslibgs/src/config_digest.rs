//! What the two ends say to each other about the configuration they read.
//!
//! Both ends of a link read a payload set: tcsmoc builds its panels from one
//! and tcspecial serves the handlers of one. Nothing made them prove it was
//! the same one, and when it was not, the only sign was a command answered
//! `NotFound` for a payload the operator could see on the screen -- a
//! tcspecial left running from an earlier set, attached to rather than
//! replaced. So a connection says which version of the software is speaking
//! and carries a digest of the configuration behind it, and the answer carries
//! the same two from the other end.
//!
//! The digest is of what a file *says*, not of its bytes. A payload set is
//! shipped in YAML and XML, and the two are one configuration written two
//! ways: two ends reading different spellings of one set have read the same
//! set and must agree. Comments, indentation, the order of the sections
//! and the order of a payload's attributes are all spelling too.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::{handler_source_of_text, HandlerSource};
use crate::{endpoint_config, ConfigFormat, PayloadConfig, TcsError, TcsResult};

/// The version of the software at one end of a link.
///
/// Three bytes, each a part of the crate version: a ground station and a
/// spacecraft that disagree about the protocol disagree in one of these, and
/// a byte each is what a spacecraft link can afford to say it with.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl ConfigVersion {
    /// What this build is.
    ///
    /// From the crate version of this library rather than of the program, so
    /// that both ends name the same thing: what has to match is the shared
    /// understanding of commands and configuration, which is what lives here.
    pub fn of_this_build() -> Self {
        fn part(text: &str) -> u8 {
            text.parse().unwrap_or(0)
        }

        Self {
            major: part(env!("CARGO_PKG_VERSION_MAJOR")),
            minor: part(env!("CARGO_PKG_VERSION_MINOR")),
            patch: part(env!("CARGO_PKG_VERSION_PATCH")),
        }
    }
}

impl std::fmt::Display for ConfigVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// An MD5 digest of a configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigDigest(pub [u8; 16]);

impl std::fmt::Display for ConfigDigest {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Separators for the canonical form, chosen from the characters reserved for
/// exactly this: a file's own text cannot contain them, so no value can be
/// written to look like a boundary.
const END_OF_VALUE: u8 = 0x1f;
const END_OF_ATTRIBUTE: u8 = 0x1e;
const END_OF_ENTRY: u8 = 0x1d;

/// The digest of the configuration file at `path`.
///
/// The file may be a payload configuration or an endpoint configuration, and
/// may be written in any of the three formats; what is digested is the
/// document each parses into, so the format cannot show through.
pub fn digest_of_file<P: AsRef<Path>>(path: P) -> TcsResult<ConfigDigest> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)?;
    digest_of_text(&text, ConfigFormat::of_file(path)?)
}

/// The digest of a configuration already in hand.
pub fn digest_of_text(text: &str, format: ConfigFormat) -> TcsResult<ConfigDigest> {
    // Parsed into its real document and then written back out as a value
    // tree, which is what makes the three formats agree: a number is a
    // number by then rather than the text a file spelled it with, a list is
    // a list rather than XML's repeated siblings, and an attribute nobody
    // stated is absent rather than missing.
    match handler_source_of_text(text, format)? {
        HandlerSource::Payload => digest_of_payload_config(&format.parse(text)?),
        HandlerSource::Endpoints => {
            digest_of_endpoint_config(&endpoint_config::from_str(text, format)?)
        }
    }
}

/// The digest of a payload configuration already parsed.
///
/// Here rather than only behind [`digest_of_text`] because this is where the
/// payloads are put in order, and a caller with a configuration in hand --
/// both programs have one -- should get the same answer as one with the file.
pub fn digest_of_payload_config(config: &PayloadConfig) -> TcsResult<ConfigDigest> {
    // By sequence number, which is the order the file gave them. A list
    // straight from a parser is in that order already; ordering it here says
    // so rather than relying on it, so that two ends agree about which
    // payload came first whatever either has since done with its own list --
    // sorted it for a lookup, say. The order means something, because ids
    // will be assigned in it.
    let mut ordered = config.clone();
    ordered.payloads.sort_by_key(|payload| payload.sequence);
    digest_of_value(&ordered)
}

/// The digest of an endpoint configuration already parsed.
///
/// The endpoints are put in the order the file gave them, by the sequence
/// number each was given as the document was built, for the reason
/// [`digest_of_payload_config`] puts the payloads in theirs: the order is
/// configuration, since ids will be assigned in it, and two ends must agree
/// about it whatever either has since done with its own list.
pub fn digest_of_endpoint_config(
    doc: &endpoint_config::EndpointConfigDoc,
) -> TcsResult<ConfigDigest> {
    let mut ordered = doc.clone();
    ordered.endpoints.sort_by_key(|endpoint| endpoint.sequence);
    digest_of_value(&ordered)
}

/// The digest of any document that can be written out as a value tree.
fn digest_of_value<T: Serialize>(document: &T) -> TcsResult<ConfigDigest> {
    let value = serde_json::to_value(document)
        .map_err(|e| TcsError::Config(format!("cannot digest the configuration: {e}")))?;

    let mut canonical = Vec::new();
    write_canonical(&value, &mut canonical);
    Ok(ConfigDigest(md5::compute(&canonical).0))
}

/// The bytes a value contributes to the digest.
///
/// One rule, which is three rules to read:
///
/// * A map is written with its keys in order, so the sections of a file may
///   be given in any order, and so may the attributes of one payload.
/// * A list is written in the order it was given, because that order means
///   something: ids will be assigned in the order the payloads appear. The
///   payloads reach here sorted by the sequence number each recorded as it
///   was read, so "the order it was given" is the order the file gave.
/// * A scalar is written as its text, which is the same text whichever format
///   it was read from.
///
/// Order is settled twice over, deliberately. The document these values come
/// from is a set of Rust structs, and a struct writes its fields in the order
/// it declares them however the file gave them -- so the order a file used is
/// already gone before this is reached. The sort below is what keeps the rule
/// true of anything that is a map rather than a struct, and of a `serde_json`
/// built to preserve insertion order rather than to sort. Removing it does not
/// fail a test today; it would fail one the day a configuration grows a map.
fn write_canonical(value: &serde_json::Value, out: &mut Vec<u8>) {
    match value {
        serde_json::Value::Object(map) => {
            // By name and then by value: a name appears once in a map, so the
            // value settles nothing here -- it is in the ordering because a
            // reader of the digest should not have to know that.
            let mut pairs: Vec<(&String, &serde_json::Value)> = map.iter().collect();
            pairs.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.to_string().cmp(&b.1.to_string())));

            for (name, value) in pairs {
                out.extend_from_slice(name.as_bytes());
                out.push(END_OF_VALUE);
                write_canonical(value, out);
                out.push(END_OF_ATTRIBUTE);
            }
        }
        serde_json::Value::Array(entries) => {
            for entry in entries {
                write_canonical(entry, out);
                out.push(END_OF_ENTRY);
            }
        }
        serde_json::Value::Null => {}
        serde_json::Value::Bool(yes) => out.extend_from_slice(if *yes { b"true" } else { b"false" }),
        serde_json::Value::Number(n) => out.extend_from_slice(n.to_string().as_bytes()),
        serde_json::Value::String(s) => out.extend_from_slice(s.as_bytes()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "\
version: \"1.0\"
description: a set
tcspecial:
  address: 0.0.0.0
  port: 4000
  protocol: udp
  beacon_interval_ms: 5000
  beacon_address: 239.255.0.1:5550
  beacon_interface: 127.0.0.1
payloads:
  - dh_id: 0
    name: DH0
    type: device
    path: /dev/null
    packet_size: 1
  - dh_id: 1
    name: DH1
    type: device
    path: /dev/zero
    packet_size: 2
";

    fn digest(text: &str) -> ConfigDigest {
        digest_of_text(text, ConfigFormat::Yaml).expect("it digests")
    }

    /// The version is three bytes, and says what this build is.
    #[test]
    fn the_version_is_three_bytes_of_this_build() {
        let version = ConfigVersion::of_this_build();
        assert_eq!(
            format!("{version}"),
            format!(
                "{}.{}.{}",
                env!("CARGO_PKG_VERSION_MAJOR"),
                env!("CARGO_PKG_VERSION_MINOR"),
                env!("CARGO_PKG_VERSION_PATCH")
            )
        );
    }

    /// A digest is sixteen bytes, said as hex.
    #[test]
    fn a_digest_is_sixteen_bytes_of_hex() {
        let said = format!("{}", digest(YAML));
        assert_eq!(said.len(), 32, "{said}");
        assert!(said.chars().all(|c| c.is_ascii_hexdigit()), "{said}");
    }

    /// The order of the sections does not change the digest.
    ///
    /// Which is the point of ordering them: a file is free to put its
    /// tcspecial section before or after its payloads, and two ends that
    /// chose differently have still read the same configuration.
    #[test]
    fn the_order_of_the_sections_does_not_matter() {
        let moved = "\
payloads:
  - dh_id: 0
    name: DH0
    type: device
    path: /dev/null
    packet_size: 1
  - dh_id: 1
    name: DH1
    type: device
    path: /dev/zero
    packet_size: 2
description: a set
tcspecial:
  address: 0.0.0.0
  port: 4000
  protocol: udp
  beacon_interval_ms: 5000
  beacon_address: 239.255.0.1:5550
  beacon_interface: 127.0.0.1
version: \"1.0\"
";
        assert_eq!(digest(moved), digest(YAML));
    }

    /// Nor does the order of one payload's attributes.
    #[test]
    fn the_order_of_attributes_does_not_matter() {
        let shuffled = YAML.replace(
            "  - dh_id: 0\n    name: DH0\n    type: device\n    path: /dev/null\n    packet_size: 1\n",
            "  - packet_size: 1\n    path: /dev/null\n    name: DH0\n    type: device\n    dh_id: 0\n",
        );
        assert_ne!(shuffled, YAML, "the test did not shuffle anything");
        assert_eq!(digest(&shuffled), digest(YAML));
    }

    /// The order of the payloads does, because that order means something:
    /// ids will be assigned in the order the payloads appear.
    #[test]
    fn the_order_of_the_payloads_does_matter() {
        let swapped = "\
version: \"1.0\"
description: a set
tcspecial:
  address: 0.0.0.0
  port: 4000
  protocol: udp
  beacon_interval_ms: 5000
  beacon_address: 239.255.0.1:5550
  beacon_interface: 127.0.0.1
payloads:
  - dh_id: 1
    name: DH1
    type: device
    path: /dev/zero
    packet_size: 2
  - dh_id: 0
    name: DH0
    type: device
    path: /dev/null
    packet_size: 1
";
        assert_ne!(digest(swapped), digest(YAML));
    }

    /// The digest is of the payloads in file order, whatever order the list
    /// is in.
    ///
    /// This is what the sequence number is for. A digest that walked the list
    /// as it found it would say two ends disagreed as soon as one of them
    /// sorted its payloads for a lookup; walking by sequence number means the
    /// two agree when they read the same file and differ when they did not,
    /// which is the only thing the comparison is asked to tell.
    #[test]
    fn the_digest_walks_the_payloads_by_sequence_number() {
        let mut config: PayloadConfig = ConfigFormat::Yaml.parse(YAML).expect("it parses");
        let in_file_order = digest(YAML);

        // As code might: the payloads in some order of its own.
        config.payloads.reverse();
        assert_eq!(config.payloads[0].name, "DH1", "the reverse did nothing");

        assert_eq!(
            digest_of_payload_config(&config).expect("it digests"),
            in_file_order,
            "a list in another order digested differently"
        );
    }

    /// Two files whose payloads are in different orders digest differently,
    /// which is the other half: the sequence numbers are part of what is
    /// hashed, so the orders are compared and not merely carried.
    #[test]
    fn a_file_in_another_order_digests_differently() {
        let swapped = "\
version: \"1.0\"
description: a set
tcspecial:
  address: 0.0.0.0
  port: 4000
  protocol: udp
  beacon_interval_ms: 5000
  beacon_address: 239.255.0.1:5550
  beacon_interface: 127.0.0.1
payloads:
  - dh_id: 1
    name: DH1
    type: device
    path: /dev/zero
    packet_size: 2
  - dh_id: 0
    name: DH0
    type: device
    path: /dev/null
    packet_size: 1
";
        assert_ne!(digest(swapped), digest(YAML));
    }

    /// An endpoint configuration digests by sequence number too.
    ///
    /// The endpoint language is the other one a payload set may be written in
    /// -- set 4 is -- so the rule has to hold there as well: a list in some
    /// other order digests the same, and a file in some other order does not.
    #[test]
    fn an_endpoint_configuration_digests_by_sequence_number() {
        let yaml = "general:\n  version: \"1.0\"\n\
                    endpoint_groups:\n  - name: g\n    type: network\n    protocol: udp\n\
                    endpoints:\n  \
                    - name: first\n    group: g\n    address: localhost\n    port: 5000\n  \
                    - name: second\n    group: g\n    address: localhost\n    port: 5001\n";
        let swapped = "general:\n  version: \"1.0\"\n\
                       endpoint_groups:\n  - name: g\n    type: network\n    protocol: udp\n\
                       endpoints:\n  \
                       - name: second\n    group: g\n    address: localhost\n    port: 5001\n  \
                       - name: first\n    group: g\n    address: localhost\n    port: 5000\n";

        let in_file_order = digest_of_text(yaml, ConfigFormat::Yaml).expect("it digests");

        // A list in another order, as code might leave it.
        let mut doc = endpoint_config::from_str(yaml, ConfigFormat::Yaml).expect("it parses");
        doc.endpoints.reverse();
        assert_eq!(
            digest_of_endpoint_config(&doc).expect("it digests"),
            in_file_order,
            "a list in another order digested differently"
        );

        // And a file in another order, which is a different configuration.
        assert_ne!(
            digest_of_text(swapped, ConfigFormat::Yaml).expect("it digests"),
            in_file_order
        );
    }

    /// A value that changed changes the digest, which is the whole job.
    #[test]
    fn a_changed_value_changes_the_digest() {
        for changed in [
            YAML.replace("packet_size: 1", "packet_size: 3"),
            YAML.replace("/dev/null", "/dev/urandom"),
            YAML.replace("port: 4000", "port: 4001"),
            YAML.replace("description: a set", "description: another set"),
            YAML.replace("name: DH0", "name: DH9"),
        ] {
            assert_ne!(
                digest(&changed),
                digest(YAML),
                "a change left the digest alone: {changed}"
            );
        }
    }

    /// Comments and whitespace are spelling, not configuration.
    #[test]
    fn comments_and_whitespace_do_not_matter() {
        let commented = format!("# what this set is for\n{YAML}\n\n");
        assert_eq!(digest(&commented), digest(YAML));
    }
}
