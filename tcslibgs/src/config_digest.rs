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

use crate::{TcsError, TcsResult};

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
    /// Says what version this build is.
    ///
    /// From the crate version of this library rather than of the program, so
    /// that both ends name the same thing: what has to match is the shared
    /// understanding of commands and configuration, which is what lives here.
    ///
    /// # Returns
    ///
    /// The version, any part that does not read as a byte being nought.
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

    /// Reads the version a configuration file states, if it states one of
    /// these.
    ///
    /// One, two or three decimal parts: a file saying `1.0` means 1.0.0, the
    /// part it leaves off being nought. Nothing else -- a part above 255 does
    /// not fit the byte it goes on the link in, and a word is not a version
    /// at all.
    ///
    /// # Arguments
    ///
    /// * `text` - The version as the file wrote it.
    ///
    /// # Returns
    ///
    /// The version, or `None` rather than a guess, because this goes on the
    /// wire: a file whose version cannot be read as three bytes would
    /// otherwise be announced as 0.0.0 to every ground station listening,
    /// which is a version some other file might really have.
    pub fn of_text(text: &str) -> Option<Self> {
        let mut parts = [0u8; 3];
        let mut count = 0;

        for part in text.trim().split('.') {
            if count == parts.len() {
                return None;
            }
            parts[count] = part.trim().parse().ok()?;
            count += 1;
        }

        if count == 0 {
            return None;
        }

        Some(Self {
            major: parts[0],
            minor: parts[1],
            patch: parts[2],
        })
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
/// The file may be written in either format; what is digested is the document
/// it parses into, so the format cannot show through.
pub fn digest_of_file<P: AsRef<Path>>(path: P) -> TcsResult<ConfigDigest> {
    // A configuration is a graph, possibly across several files: see
    // digest_of_graph.
    digest_of_graph(&crate::graph::Graph::load(path.as_ref())?)
}

/// The digest of a configuration: everything the vehicle and the
/// ground act on, across every file it was read from.
///
/// Taken over the configuration as loaded rather than over a file's text, so
/// that its included files are in it and so that the YAML and the XML
/// spellings of one configuration digest alike. In it: the `tcspecial`
/// section, the payload groups, the nodes in the order the files declare them
/// -- which is what a node's device id is, and so which port its data reaches
/// the ground on -- with their attributes, the links, and the command
/// bindings. Not in it: the editor's own section, where the nodes are drawn,
/// and the description, neither of which changes what anything does.
pub fn digest_of_graph(graph: &crate::graph::Graph) -> TcsResult<ConfigDigest> {
    digest_of_value(&interpretation_of_graph(graph))
}

/// The digest of the simulator file at `path`: of its bytes.
///
/// Of the text rather than of what it parses into, as a configuration's is:
/// a simulator file is read by tcssim alone and in one format, so the two
/// spellings that make the configuration's digest an interpretation's do not
/// arise. What it is for is a harness telling whether a simulator already
/// running read the file the harness did.
pub fn digest_of_simulator_file<P: AsRef<Path>>(
    path: P,
) -> TcsResult<ConfigDigest> {
    let text = std::fs::read(path.as_ref())?;
    Ok(ConfigDigest(md5::compute(text).0))
}

/// What a configuration file is read as, as a value tree: the thing its
/// digest is taken over.
///
/// Two files with the same interpretation are one configuration, whichever
/// format each is written in -- their digests are the same, and every program
/// that reads them acts on the same thing. Two whose interpretations differ
/// differ where the trees do, which is what a program comparing two spellings
/// of one configuration needs to say: see `tcsverify`.
pub fn interpretation_of_file<P: AsRef<Path>>(
    path: P,
) -> TcsResult<serde_json::Value> {
    Ok(interpretation_of_graph(&crate::graph::Graph::load(
        path.as_ref(),
    )?))
}

/// Writes out what a configuration is read as: see [`digest_of_graph`],
/// which says what is in it and what is not.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// Its `tcspecial` section, groups, nodes, links and command bindings as a
/// value tree, with every scalar as its text.
pub fn interpretation_of_graph(
    graph: &crate::graph::Graph,
) -> serde_json::Value {
    use serde_json::{json, Map, Value};

    let text =
        |scalar: &crate::Scalar| Value::String(scalar.as_str().to_string());
    let tcspecial: Map<String, Value> = graph
        .tcspecial
        .as_ref()
        .map(|section| {
            crate::graph::parameters::flatten(section)
                .iter()
                .map(|(name, value)| (name.clone(), text(value)))
                .collect()
        })
        .unwrap_or_default();
    let nodes: Vec<Value> = graph
        .nodes
        .iter()
        .map(|node| {
            let attributes: Map<String, Value> = node
                .attributes
                .iter()
                .map(|(name, value)| (name.clone(), text(value)))
                .collect();
            json!({
                "name": node.name,
                "type": node.node_type,
                "group": node.group,
                "attributes": attributes,
            })
        })
        .collect();
    let links: Vec<Value> = graph
        .links
        .iter()
        .map(|link| json!({ "from": link.from, "to": link.to.each() }))
        .collect();
    let groups: Vec<Value> = graph
        .groups
        .iter()
        .map(|group| json!({ "name": group.name, "interval": group.interval }))
        .collect();
    let commands: Vec<Value> = graph
        .commands
        .iter()
        .map(|binding| json!({ "from": binding.from, "sets": binding.sets }))
        .collect();

    json!({
        "version": crate::graph::VERSION,
        "tcspecial": tcspecial,
        "payload_groups": groups,
        "nodes": nodes,
        "links": links,
        "commands": commands,
    })
}

/// Digests any document that can be written out as a value tree.
///
/// # Arguments
///
/// * `document` - The document.
///
/// # Returns
///
/// The MD5 digest of its canonical bytes.
///
/// # Errors
///
/// [`TcsError::Config`] if it cannot be written out as a value tree.
fn digest_of_value<T: Serialize>(document: &T) -> TcsResult<ConfigDigest> {
    let value = serde_json::to_value(document).map_err(|e| {
        TcsError::Config(format!("cannot digest the configuration: {e}"))
    })?;

    let mut canonical = Vec::new();
    write_canonical(&value, &mut canonical);
    Ok(ConfigDigest(md5::compute(&canonical).0))
}

/// Writes the bytes a value contributes to the digest.
///
/// One rule, which is three rules to read:
///
/// * A map is written with its keys in order, so the sections of a file may
///   be given in any order, and so may the attributes of one payload.
/// * A list is written in the order it was given, because that order means
///   something: it is the order the file was written in. The payloads reach
///   here sorted by the sequence number each recorded as it was read, so "the
///   order it was given" is the order the file gave.
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
///
/// # Arguments
///
/// * `value` - The value.
/// * `out`   - Where its bytes are appended.
fn write_canonical(value: &serde_json::Value, out: &mut Vec<u8>) {
    match value {
        serde_json::Value::Object(map) => {
            // By name and then by value: a name appears once in a map, so the
            // value settles nothing here -- it is in the ordering because a
            // reader of the digest should not have to know that.
            let mut pairs: Vec<(&String, &serde_json::Value)> =
                map.iter().collect();
            pairs.sort_by(|a, b| {
                a.0.cmp(b.0)
                    .then_with(|| a.1.to_string().cmp(&b.1.to_string()))
            });

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
        serde_json::Value::Bool(yes) => {
            out.extend_from_slice(if *yes { b"true" } else { b"false" })
        }
        serde_json::Value::Number(n) => {
            out.extend_from_slice(n.to_string().as_bytes())
        }
        serde_json::Value::String(s) => out.extend_from_slice(s.as_bytes()),
    }
}

#[cfg(test)]
mod tests {
    /// A configuration file's version is read as the three bytes it goes on
    /// the link as, and a version that is not that is refused rather than
    /// rounded.
    ///
    /// It is announced in every beacon, so a file whose version cannot be
    /// read would otherwise be announced as 0.0.0 -- a version some other
    /// file might really have.
    #[test]
    fn a_configuration_states_its_version_in_decimal_parts() {
        for (text, want) in [
            ("1.0", (1, 0, 0)),
            ("1", (1, 0, 0)),
            ("2.3.4", (2, 3, 4)),
            (" 2.3.4 ", (2, 3, 4)),
            ("0.0.0", (0, 0, 0)),
            ("255.255.255", (255, 255, 255)),
        ] {
            let version = ConfigVersion::of_text(text)
                .unwrap_or_else(|| panic!("{text}"));
            assert_eq!(
                (version.major, version.minor, version.patch),
                want,
                "for {text}"
            );
        }

        for text in [
            "",
            "one",
            "1.0.0.0",
            "1.x",
            "-1",
            "256",
            "1.0-draft",
            "v1.0",
        ] {
            assert!(
                ConfigVersion::of_text(text).is_none(),
                "{text:?} is not a version"
            );
        }
    }

    use super::*;

    /// Loads one of the examples.
    ///
    /// # Arguments
    ///
    /// * `name` - The example's directory under `examples/`, which is also
    ///            its file's stem.
    /// * `ext`  - Which spelling to load: `yaml` or `xml`.
    ///
    /// # Returns
    ///
    /// The configuration.
    ///
    /// # Panics
    ///
    /// If it does not read.
    fn example_graph(name: &str, ext: &str) -> crate::graph::Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        crate::graph::Graph::load(
            root.join(format!("examples/{name}/{name}.{ext}")),
        )
        .expect("the example reads")
    }

    /// A configuration digests alike in either spelling, and
    /// through the file as through the graph.
    #[test]
    fn a_graph_digests_alike_in_both_spellings() {
        for name in ["minimal", "thermostat", "simple-app"] {
            let yaml = digest_of_graph(&example_graph(name, "yaml"))
                .expect("it digests");
            let xml = digest_of_graph(&example_graph(name, "xml"))
                .expect("it digests");
            assert_eq!(yaml, xml, "{name}");
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
            let file = digest_of_file(
                root.join(format!("examples/{name}/{name}.yaml")),
            )
            .expect("the file digests");
            assert_eq!(file, yaml, "{name}");
        }
    }

    /// Where the editor draws a node is not the configuration; a node's
    /// attributes and its place in the order are.
    #[test]
    fn a_graph_digest_follows_what_the_vehicle_acts_on() {
        let graph = example_graph("thermostat", "yaml");
        let digest = digest_of_graph(&graph).expect("it digests");

        let mut moved = graph.clone();
        moved.placed.clear();
        moved.description = Some("another description".to_string());
        assert_eq!(digest_of_graph(&moved).expect("it digests"), digest);

        let mut retuned = graph.clone();
        let node = retuned
            .nodes
            .iter_mut()
            .find(|n| n.name == "thermostat")
            .expect("the thermostat");
        node.attributes
            .insert("setpoint".to_string(), crate::Scalar::of("300.0"));
        assert_ne!(digest_of_graph(&retuned).expect("it digests"), digest);

        // Two nodes swapped are two device ids swapped, and two ports.
        let mut reordered = graph.clone();
        reordered.nodes.swap(1, 2);
        assert_ne!(digest_of_graph(&reordered).expect("it digests"), digest);
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
        let said = format!(
            "{}",
            digest_of_graph(&example_graph("minimal", "yaml"))
                .expect("it digests")
        );
        assert_eq!(said.len(), 32, "{said}");
        assert!(said.chars().all(|c| c.is_ascii_hexdigit()), "{said}");
    }
}
