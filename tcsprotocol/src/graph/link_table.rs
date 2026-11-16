//! Everything about the links between the vehicle and the ground that is
//! worked out of a configuration rather than written in it, as one table,
//! and its **link digest**.
//!
//! The configuration digest says that two ends read the same file; it
//! cannot say that they worked the same things out of it. A port or an APID
//! counted on from a base by a device id, the count before a string, and the
//! bit layout of a consolidation are each derived by tcsprotocol's code, and
//! a vehicle and a ground station built from different versions of that code
//! would read one file and disagree about all of them without a word: packets
//! under one APID read as another's, a consolidation's members read at the
//! wrong offsets. So both ends work the table out, and the vehicle puts the
//! digest of its table in every beacon for the ground to compare with its own
//! (see `beacon`).
//!
//! The table is every node's device id; every link to the ground, with
//! where it arrives, its APID and how it is counted;
//! every APID and every message ID used, and what each carries; and every
//! consolidation's layout. It is written as YAML, in one order and one
//! spelling, by [`LinkTable::body`], and the digest is the MD5 of that text:
//! the same table is the same text and the same digest, on any machine. What
//! a complaint would say of a use -- its wording -- is left out, so that
//! rewording a message changes no digest.
//!
//! `tcs-verify --links` prints the table, with the digest and comments, for
//! a ground station that is not built on this crate to read rather than work
//! out again: the Python spelling reads its ports, APIDs and message IDs from
//! there, so the rule that derives them is stated once.

use std::collections::BTreeMap;
use std::fmt::Write;

use super::telemetry::{
    self, ApidCarries, ApidUse, Carries, MsgIdUse, Refused, Telemetered, Via,
};
use super::Graph;
use crate::consolidation::{Layout, Shape};
use crate::link_type::Count;

/// The MD5 of a configuration's [`LinkTable`], as a beacon carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkDigest(pub [u8; 16]);

impl LinkDigest {
    /// Reads a link digest as a beacon and `tcs-verify --links` give it: 32
    /// lower-case hex digits.
    ///
    /// # Arguments
    ///
    /// * `said` - The digits.
    ///
    /// # Returns
    ///
    /// The digest, or `None` for anything that is not 32 lower-case hex
    /// digits.
    pub fn of_hex(said: &str) -> Option<LinkDigest> {
        crate::interface::InterfaceDigest::of_hex(said)
            .map(|digest| LinkDigest(digest.0))
    }
}

impl std::fmt::Display for LinkDigest {
    /// Writes it as 32 lower-case hex digits.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Everything about the links between a vehicle and the ground that is
/// worked out of its configuration.
#[derive(Debug, Clone)]
pub struct LinkTable {
    /// Every node's device id, by name.
    pub ids: BTreeMap<String, usize>,
    /// Every link to the ground: those drawn into a gateway node, then the
    /// untouched payloads'.
    pub links: Vec<Telemetered>,
    /// Every APID used, where the configuration answers to Yamcs.
    pub apids: Vec<ApidUse>,
    /// Every message ID used, where it answers over a software bus.
    pub msgids: Vec<MsgIdUse>,
    /// Every consolidator's layout, by its name.
    pub consolidations: BTreeMap<String, Layout>,
}

impl LinkTable {
    /// Works the table out of a configuration.
    ///
    /// # Arguments
    ///
    /// * `graph` - The configuration.
    ///
    /// # Returns
    ///
    /// The table.
    ///
    /// # Errors
    ///
    /// What [`telemetry::of`], [`telemetry::apids`] and
    /// [`telemetry::msgids`] refuse, or a consolidator whose layout does not
    /// work out, which the check refuses too.
    pub fn of(graph: &Graph) -> Result<LinkTable, Refused> {
        let mut consolidations = BTreeMap::new();
        for node in &graph.nodes {
            let Some(laid) = Layout::of_node(graph, node) else {
                continue;
            };
            let (_, layout) = laid.map_err(|why| Refused {
                node: Some(node.name.clone()),
                why: format!("\"{}\" (consolidator): {why}", node.name),
            })?;
            consolidations.insert(node.name.clone(), layout);
        }
        Ok(LinkTable {
            ids: telemetry::ids(graph),
            links: telemetry::of(graph)?,
            apids: telemetry::apids(graph)?,
            msgids: telemetry::msgids(graph)?,
            consolidations,
        })
    }

    /// The table as YAML, in one order and one spelling, with no comments:
    /// what the digest is taken over.
    ///
    /// # Returns
    ///
    /// The text.
    pub fn body(&self) -> String {
        let mut out = String::new();
        out.push_str(heading("ids", self.ids.is_empty(), "{}").as_str());
        for (name, id) in &self.ids {
            let _ = writeln!(out, "  {}: {id}", quoted(name));
        }

        out.push_str(heading("links", self.links.is_empty(), "[]").as_str());
        for link in &self.links {
            let _ = writeln!(out, "  - name: {}", quoted(&link.name));
            let _ = writeln!(out, "    source: {}", quoted(&link.source));
            let _ = writeln!(out, "    id: {}", link.id);
            match &link.via {
                Via::Gateway(gateway) => {
                    let _ = writeln!(out, "    gateway: {}", quoted(gateway));
                }
                Via::PayloadTelemetry => {
                    out.push_str("    gateway: null\n");
                }
            }
            match (link.vehicle(), link.ground(), link.msgid()) {
                (Some(vehicle), Some(ground), _) => {
                    let _ = writeln!(
                        out,
                        "    vehicle: \"{}:{}\"\n    ground: \"{}:{}\"\n    \
                         msgid: null",
                        vehicle.address,
                        vehicle.port,
                        ground.address,
                        ground.port
                    );
                }
                (_, _, msgid) => {
                    let _ = writeln!(
                        out,
                        "    vehicle: null\n    ground: null\n    msgid: {}",
                        number(msgid.map(u32::from))
                    );
                }
            }
            let _ =
                writeln!(out, "    apid: {}", number(link.apid.map(u32::from)));
            let _ = writeln!(
                out,
                "    type: {}",
                quoted(&link.carries.to_string())
            );
            let count = match link.count() {
                None => "null",
                Some(Count::Xtce) => "xtce",
                Some(Count::Groups) => "groups",
            };
            let _ = writeln!(out, "    count: {count}");
            let _ = writeln!(out, "    largest: {}", link.largest);
        }

        out.push_str(heading("apids", self.apids.is_empty(), "[]").as_str());
        for used in &self.apids {
            // The node is the one whose data or commands the APID carries:
            // a link's source, found by the link's name, or the gateway node
            // an uplink is to.
            let source = |link: &String| {
                self.links
                    .iter()
                    .find(|each| each.name == *link)
                    .map(|each| &each.source)
            };
            let (carries, node, gateway) = match &used.carries {
                ApidCarries::Beacon => ("beacon", None, None),
                ApidCarries::Commands => ("commands", None, None),
                ApidCarries::Uplink(gateway) => {
                    ("uplink", Some(gateway), Some(gateway))
                }
                ApidCarries::Downlink { gateway, link } => {
                    ("downlink", source(link), Some(gateway))
                }
                ApidCarries::PayloadTelemetry(link) => {
                    ("payload_telemetry", source(link), None)
                }
            };
            let _ = writeln!(out, "  - apid: {}", used.apid);
            let _ = writeln!(out, "    carries: {carries}");
            let _ = writeln!(out, "    node: {}", maybe(node));
            let _ = writeln!(out, "    gateway: {}", maybe(gateway));
        }

        out.push_str(heading("msgids", self.msgids.is_empty(), "[]").as_str());
        for used in &self.msgids {
            let (carries, about) = match &used.carries {
                Carries::SpacecraftCommands => ("spacecraft_commands", None),
                Carries::SpacecraftAnswers => ("spacecraft_answers", None),
                Carries::NodeCommands(_) => ("node_commands", None),
                Carries::NodeAnswers(_) => ("node_answers", None),
                Carries::Beacon => ("beacon", None),
                Carries::Gateway(gateway) => ("gateway", Some(gateway)),
                Carries::Telemetry(link) => ("telemetry", Some(link)),
            };
            let _ = writeln!(out, "  - msgid: {}", used.msgid);
            let _ = writeln!(out, "    command: {}", used.command);
            let _ = writeln!(out, "    carries: {carries}");
            let _ = writeln!(out, "    about: {}", maybe(about));
            let _ = writeln!(out, "    node: {}", maybe(used.node.as_ref()));
        }

        out.push_str(
            heading("consolidations", self.consolidations.is_empty(), "{}")
                .as_str(),
        );
        for (name, layout) in &self.consolidations {
            let _ = writeln!(out, "  {}:", quoted(name));
            for (at, input) in layout.inputs.iter().enumerate() {
                let _ = writeln!(out, "    - input: {}", quoted(&input.name));
                let _ =
                    writeln!(out, "      source: {}", quoted(&input.source));
                let _ = writeln!(
                    out,
                    "      type: {}",
                    quoted(&input.kind.to_string())
                );
                match input.shape {
                    Shape::Fixed { bytes } => {
                        let _ = writeln!(
                            out,
                            "      offset: {}\n      bytes: {bytes}",
                            number(
                                layout
                                    .offset(at)
                                    .and_then(|o| u32::try_from(o).ok())
                            )
                        );
                    }
                    // At the datum's end, in the order of the inputs that
                    // vary: a count and its bytes.
                    Shape::Varies { most, count_bytes } => {
                        let _ = writeln!(
                            out,
                            "      offset: null\n      most: {most}\n      \
                             count_bytes: {count_bytes}"
                        );
                    }
                }
                let hex: String =
                    input.default.iter().map(|b| format!("{b:02x}")).collect();
                let _ = writeln!(out, "      default: \"{hex}\"");
            }
        }
        out
    }

    /// The table's digest: the MD5 of [`LinkTable::body`].
    pub fn digest(&self) -> LinkDigest {
        LinkDigest(md5::compute(self.body()).0)
    }

    /// The table as `tcs-verify --links` prints it: comments saying what
    /// it is, the digest, then [`LinkTable::body`].
    ///
    /// # Arguments
    ///
    /// * `file` - The configuration's file, as the comment names it.
    ///
    /// # Returns
    ///
    /// The text.
    pub fn yaml(&self, file: &str) -> String {
        let body = self.body();
        format!(
            "# The links between the vehicle and the ground that tcsprotocol\n\
             # works out of the configuration\n\
             #\n\
             #     {file}\n\
             #\n\
             # each node's device id; every link to the ground, with where\n\
             # it arrives, its APID and how a string or bytes on it is\n\
             # counted;\n\
             # every APID and message ID used; and each consolidation's\n\
             # layout. Written by tcs-verify --links, to be read and not\n\
             # worked out again.\n\
             #\n\
             # link_digest is the MD5 of everything after its line, which\n\
             # tcspecial puts in every beacon: a ground station whose own\n\
             # differs was built to work these out otherwise than the vehicle.\n\
             link_digest: \"{}\"\n{body}",
            LinkDigest(md5::compute(&body).0)
        )
    }
}

/// The link digest of a configuration: see [`LinkTable`].
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// The digest.
///
/// # Errors
///
/// As [`LinkTable::of`].
pub fn digest_of(graph: &Graph) -> Result<LinkDigest, Refused> {
    Ok(LinkTable::of(graph)?.digest())
}

/// Writes a section's key, and where it is empty, its empty value, so that
/// it reads as an empty list or mapping and not as nothing.
///
/// # Arguments
///
/// * `key`   - The section's key.
/// * `empty` - Whether it has nothing in it.
/// * `none`  - What it is when it has nothing: `[]` or `{}`.
///
/// # Returns
///
/// The line.
fn heading(key: &str, empty: bool, none: &str) -> String {
    match empty {
        true => format!("{key}: {none}\n"),
        false => format!("{key}:\n"),
    }
}

/// Writes a string as a YAML string in double quotes, escaping what would
/// end or change it.
///
/// # Arguments
///
/// * `text` - The string.
///
/// # Returns
///
/// The quoted string.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\x{:02x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Writes a name that may be absent: quoted, or `null`.
///
/// # Arguments
///
/// * `name` - The name, or `None`.
///
/// # Returns
///
/// The YAML.
fn maybe(name: Option<&String>) -> String {
    name.map_or("null".to_string(), |name| quoted(name))
}

/// Writes a number that may be absent: in decimal, or `null`.
///
/// # Arguments
///
/// * `value` - The number, or `None`.
///
/// # Returns
///
/// The YAML.
fn number(value: Option<u32>) -> String {
    value.map_or("null".to_string(), |value| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Loads a configuration of this tree.
    ///
    /// # Arguments
    ///
    /// * `path` - The file, from the repository's root.
    ///
    /// # Returns
    ///
    /// Its graph.
    fn load(path: &str) -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        Graph::load(root.join(path)).expect("the file reads")
    }

    /// The table of a consolidator to Yamcs says its link, its APID and the
    /// consolidation's layout; the two spellings of one
    /// configuration have one digest; and the YAML reads back as YAML,
    /// its digest that of what follows it.
    #[test]
    fn the_table_says_what_is_worked_out_and_digests_it() {
        let yaml = LinkTable::of(&load("tests/consolidator/yamcs.ytcs"))
            .expect("it works out");
        let xml = LinkTable::of(&load("tests/consolidator/yamcs.xtcs"))
            .expect("it works out");
        assert_eq!(yaml.digest(), xml.digest());

        let body = yaml.body();
        assert!(body.contains("  - apid: 514\n"), "{body}");
        assert!(!body.contains("  - apid: 515\n"), "no input's own: {body}");
        // The valve, a boolean, a byte at offset 2 after the u16; the name,
        // a string of at most 8, at the end after a count of one byte; each
        // with its default.
        assert!(
            body.contains(
                "    - input: \"valve\"\n      source: \"valve\"\n      \
                 type: \"boolean\"\n      offset: 2\n      bytes: 1\n      \
                 default: \"00\"\n"
            ),
            "{body}"
        );
        assert!(
            body.contains(
                "      offset: null\n      most: 8\n      count_bytes: 1\n      \
                 default: \"\"\n"
            ),
            "{body}"
        );

        let printed = yaml.yaml("yamcs.ytcs");
        let read: serde_norway::Value =
            serde_norway::from_str(&printed).expect("it is YAML");
        assert_eq!(
            read["link_digest"].as_str(),
            Some(yaml.digest().to_string().as_str())
        );
        let (_, after) = printed
            .split_once("link_digest: ")
            .and_then(|(_, rest)| rest.split_once('\n'))
            .expect("a digest line");
        assert_eq!(after, body);
        assert_eq!(
            LinkDigest::of_hex(&yaml.digest().to_string()),
            Some(yaml.digest())
        );
    }

    /// Two configurations whose links differ have different digests: one
    /// whose ground_base moved, the same file otherwise.
    #[test]
    fn a_moved_port_changes_the_digest() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let text =
            std::fs::read_to_string(root.join("tests/consolidator/yamcs.ytcs"))
                .expect("it reads");
        let dir = tempfile::tempdir().expect("a directory");
        let moved = dir.path().join("moved.ytcs");
        std::fs::write(
            &moved,
            text.replace("127.0.0.1:26700", "127.0.0.1:26800"),
        )
        .expect("written");
        let before = LinkTable::of(&load("tests/consolidator/yamcs.ytcs"))
            .expect("it works out")
            .digest();
        let after = LinkTable::of(&Graph::load(&moved).expect("it reads"))
            .expect("it works out")
            .digest();
        assert_ne!(before, after);
    }
}
