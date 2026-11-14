//! What the two ends say to each other about the configuration they read.
//!
//! Both ends of a link read a payload set: tcs-moc builds its panels from one
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
//! set and must agree. Comments, indentation and order are all spelling too:
//! the order of the sections, of the types in a section, of the nodes of a
//! type, of a node's attributes, of its links and their destinations, of the
//! groups and the command bindings, and of the entries of an attribute that
//! is a set. Nothing in a configuration is in an order that means anything,
//! so the digest is taken over it in one order, by name.

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

impl ConfigDigest {
    /// Reads a checksum as tcs-verify prints it and a beacon carries it: 32
    /// lower-case hex digits.
    ///
    /// # Arguments
    ///
    /// * `said` - The digits.
    ///
    /// # Returns
    ///
    /// The checksum, or `None` for anything that is not 32 lower-case hex
    /// digits.
    pub fn of_hex(said: &str) -> Option<ConfigDigest> {
        crate::interface::InterfaceDigest::of_hex(said)
            .map(|digest| ConfigDigest(digest.0))
    }
}

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
/// section, the payload groups, the nodes with their attributes, the links,
/// the names each node lists under its `world-in` and its `moc-in`, the
/// command bindings, and the names its include list gives its other files. Not in it: the editor's own section, where the nodes are drawn,
/// and the description, neither of which changes what anything does.
///
/// Each list in one order, whatever order the files wrote it in: the nodes
/// by name -- which is also the order their device ids are counted in, so
/// the digest says which port each node's data reaches the ground on -- the
/// links by the node they leave, the groups by name, the bindings by where
/// they come from and what they set, and the included files by name. The
/// exceptions are a node's four terminal lists: what it lists under its
/// `world-in` and `moc-in`, and the destinations under its `world-out` and
/// `moc-out`, each digested in the order the files list it. A node that
/// takes or sends several lines may make something of their order -- the
/// order a consolidated datum's fields are in, say -- and the vehicle and
/// the ground must then agree about it. See [`interpretation_of_graph`].
pub fn digest_of_graph(graph: &crate::graph::Graph) -> TcsResult<ConfigDigest> {
    digest_of_value(&interpretation_of_graph(graph))
}

/// The digest of the simulator file at `path`: of what it says, as a
/// configuration's is.
///
/// What it is for is a harness telling whether a simulator already running
/// read the file the harness did, and the file, its physics model with it,
/// is what that simulator acts on. So the digest is of the file as read, in
/// one order, rather than of its bytes: a comment added, or the payloads or
/// the components written in another order, is the same simulator file,
/// and a figure of the model changed is not. See
/// [`interpretation_of_simulator`].
///
/// # Arguments
///
/// * `path` - The simulator file.
///
/// # Returns
///
/// Its digest.
///
/// # Errors
///
/// [`TcsError::Config`] where the file cannot be read.
pub fn digest_of_simulator_file<P: AsRef<Path>>(
    path: P,
) -> TcsResult<ConfigDigest> {
    let file = crate::sim_config::SimConfigFile::load(path.as_ref())?;
    digest_of_value(&interpretation_of_simulator(&file))
}

/// Writes out what a simulator file is read as: the thing its digest is
/// taken over.
///
/// The `tcs-sim` section; the groups of settings, by name, each with every
/// setting it states; the simulated payloads, by name, each with its type,
/// the group it names and every setting it states; and the world, as read:
/// its ambient temperature, and its physics model's components by name,
/// each with its type and figures, and its connections in the order of what
/// they join. Not in it: the description,
/// which changes nothing tcs-sim does. Every list is in one order whatever order the file
/// wrote it in, nothing in a simulator file or a model being in an order
/// that means anything, and the keys of a map are put in order as the
/// digest is taken.
///
/// # Arguments
///
/// * `file` - The simulator file.
///
/// # Returns
///
/// It as a value tree.
pub fn interpretation_of_simulator(
    file: &crate::sim_config::SimConfigFile,
) -> serde_json::Value {
    use serde_json::{json, Value};

    let settings = |settings: &crate::sim_config::SimSettings| -> Value {
        serde_json::to_value(settings).unwrap_or(Value::Null)
    };
    let tcs_sim = file.tcs_sim.as_ref().map(|section| {
        // The names as one text, as a file writes them, and nothing where
        // there are none: one thermostat interprets as it did before a file
        // could name several.
        let thermostat = (!section.thermostats.is_empty())
            .then(|| section.thermostats.join(","));
        json!({
            "ready": section.ready,
            "thermostat": thermostat,
            "seed": section.seed,
        })
    });
    let mut groups: Vec<&crate::sim_config::SimGroup> =
        file.simulated_payload_groups.iter().collect();
    groups.sort_by(|a, b| a.name.cmp(&b.name));
    let groups: Vec<Value> = groups
        .into_iter()
        .map(|group| {
            json!({ "name": group.name, "settings": settings(&group.settings) })
        })
        .collect();
    let mut payloads: Vec<&crate::sim_config::SimPayload> =
        file.simulated_payloads.iter().collect();
    payloads.sort_by(|a, b| a.name.cmp(&b.name));
    let payloads: Vec<Value> = payloads
        .into_iter()
        .map(|payload| {
            json!({
                "name": payload.name,
                "type": payload.settings.dh_type,
                "group": payload.group,
                "settings": settings(&payload.settings),
            })
        })
        .collect();
    let world = file.world.as_ref().map(|world| {
        let mut components: Vec<&crate::node::physics::Component> =
            world.physics.components.iter().collect();
        components.sort_by(|a, b| a.name().cmp(b.name()));
        let mut connections: Vec<(&str, &str)> = world
            .physics
            .connections
            .iter()
            .map(|c| (c.from.as_str(), c.to.as_str()))
            .collect();
        connections.sort_unstable();
        json!({
            "ambient_temp": world.ambient_temp,
            "physics": {
                "components": components,
                "connections": connections
                    .into_iter()
                    .map(|(from, to)| json!({ "from": from, "to": to }))
                    .collect::<Vec<Value>>(),
            },
        })
    });
    json!({
        "tcs-sim": tcs_sim,
        "simulated_payload_groups": groups,
        "simulated_payloads": payloads,
        "world": world,
    })
}

/// What a configuration file is read as, as a value tree: the thing its
/// digest is taken over.
///
/// Two files with the same interpretation are one configuration, whichever
/// format each is written in -- their digests are the same, and every program
/// that reads them acts on the same thing. Two whose interpretations differ
/// differ where the trees do, which is what a program comparing two spellings
/// of one configuration needs to say: see `tcs-verify`.
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
/// In canonical order, so that two configurations that differ only in the
/// order they are written in are read as the same tree: every list is
/// sorted, and an attribute whose value is a set of entries -- see
/// [`unordered`] -- is written with its entries sorted, in one spelling.
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
    let mut sorted: Vec<&crate::graph::Node> = graph.nodes.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    let nodes: Vec<Value> = sorted
        .into_iter()
        .map(|node| {
            let attributes: Map<String, Value> = node
                .attributes
                .iter()
                .map(|(name, value)| {
                    let said = unordered(&node.node_type, name, value.as_str())
                        .unwrap_or_else(|| value.as_str().to_string());
                    (name.clone(), Value::String(said))
                })
                .collect();
            json!({
                "name": node.name,
                "type": node.node_type,
                "group": node.group,
                "attributes": attributes,
            })
        })
        .collect();
    // Each link's destinations in the order its source lists them, under its
    // world-out or moc-out, as a node's sinks are: see digest_of_graph.
    let mut links: Vec<(&str, Vec<&str>)> = graph
        .links
        .iter()
        .map(|link| (link.from.as_str(), link.to.each()))
        .collect();
    links.sort();
    let links: Vec<Value> = links
        .into_iter()
        .map(|(from, to)| json!({ "from": from, "to": to }))
        .collect();
    let mut groups: Vec<&crate::graph::Group> = graph.groups.iter().collect();
    groups.sort_by(|a, b| (&a.name, &a.interval).cmp(&(&b.name, &b.interval)));
    let groups: Vec<Value> = groups
        .into_iter()
        .map(|group| json!({ "name": group.name, "interval": group.interval }))
        .collect();
    let mut commands: Vec<(&str, &str)> = graph
        .commands
        .iter()
        .map(|binding| (binding.from.as_str(), binding.sets.as_str()))
        .collect();
    commands.sort_unstable();
    let commands: Vec<Value> = commands
        .into_iter()
        .map(|(from, sets)| json!({ "from": from, "sets": sets }))
        .collect();

    // Each node's sinks, in the order its entry lists them, by node, as its
    // sources' destinations are above. Only the nodes that list some.
    let sinks: Vec<Value> = graph
        .named
        .sinks
        .iter()
        .filter(|(_, sinks)| {
            !sinks.world_in.is_empty() || !sinks.moc_in.is_empty()
        })
        .map(|(node, sinks)| {
            json!({
                "node": node,
                "world-in": sinks.world_in,
                "moc-in": sinks.moc_in,
            })
        })
        .collect();

    let mut interpretation = json!({
        "version": crate::graph::VERSION,
        "tcspecial": tcspecial,
        "payload_groups": groups,
        "nodes": nodes,
        "links": links,
        "sinks": sinks,
        "commands": commands,
    });
    // The files it includes, each by its subdiagram's name and its path, so
    // that the checksum a kept configuration is named by says the shape of
    // its tree too: see `crate::store`. Only where there are some, so that
    // a configuration of one file keeps the checksum it always had. By name,
    // the order meaning nothing.
    let mut included = graph.included();
    included.sort();
    if !included.is_empty() {
        let included: Vec<Value> = included
            .iter()
            .map(|i| json!({ "subdiagram": i.name, "path": i.path }))
            .collect();
        interpretation["include"] = json!(included);
    }
    interpretation
}

/// Writes an attribute whose value is a set of entries in its one spelling.
///
/// Some attributes list things whose order means nothing: a vote's `inputs`,
/// the `lines` of a GPIO chip a payload watches or drives, and the
/// `terminators` any of which ends a read. Written in another order, or
/// with other spaces or commas between the entries, they say the same thing,
/// and are digested as the same thing: the entries split as the attribute's
/// reader splits them, trimmed, sorted, and joined by a comma and a space.
///
/// An attribute whose order is its meaning is left as it is written: a
/// `trigger`'s bytes and an `init` sequence are runs of bytes sent in order.
///
/// # Arguments
///
/// * `node_type` - The type of the node the attribute is on.
/// * `attribute` - The attribute's name.
/// * `value`     - Its value, as read.
///
/// # Returns
///
/// The value in its one spelling, or `None` for an attribute that is not a
/// set.
pub fn unordered(
    node_type: &str,
    attribute: &str,
    value: &str,
) -> Option<String> {
    let mut entries: Vec<&str> = match (node_type, attribute) {
        // As `node::vehicle::compute` reads a vote's inputs.
        ("compute", "inputs") => value.split([',', ' ']).collect(),
        // As `node::payload::gpio::lines_of` reads them.
        ("gpio_in" | "gpio_out" | "gpio_inout", "lines") => {
            value.split([',', ' ', '\t']).collect()
        }
        // A listed attribute, held with its entries joined: see
        // `graph::LISTED`.
        (_, listed) if crate::graph::LISTED.contains(&listed) => {
            crate::graph::entries(value)
        }
        _ => return None,
    };
    entries = entries
        .into_iter()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect();
    entries.sort_unstable();
    Some(entries.join(", "))
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
/// * A list is written in the order it was given. Nothing in a configuration
///   is in an order that means anything, and [`interpretation_of_graph`]
///   has already put every list in one order, by name, so "the order it was
///   given" is that order and never the file's.
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
            let yaml = digest_of_graph(&example_graph(name, "ytcs"))
                .expect("it digests");
            let xml = digest_of_graph(&example_graph(name, "xtcs"))
                .expect("it digests");
            assert_eq!(yaml, xml, "{name}");
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
            let file = digest_of_file(
                root.join(format!("examples/{name}/{name}.ytcs")),
            )
            .expect("the file digests");
            assert_eq!(file, yaml, "{name}");
        }
    }

    /// Where the editor draws a node is not the configuration; a node's
    /// attributes and its place in the order are.
    #[test]
    fn a_graph_digest_follows_what_the_vehicle_acts_on() {
        let graph = example_graph("thermostat", "ytcs");
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

        // The interface the configuration names is part of it, so two
        // configurations whose checksums match name the same interface.
        let mut other_interface = graph.clone();
        if let Some(section) = other_interface.tcspecial.as_mut() {
            section.insert(
                "interface".to_string(),
                crate::graph::parameters::Parameter::Value(crate::Scalar::of(
                    "0123456789abcdef0123456789abcdef",
                )),
            );
        }
        assert_ne!(
            digest_of_graph(&other_interface).expect("it digests"),
            digest
        );

        // Two nodes swapped are the same configuration: a node's device id
        // is its place among the names, not its place in a file.
        let mut reordered = graph.clone();
        reordered.nodes.swap(1, 2);
        assert_eq!(digest_of_graph(&reordered).expect("it digests"), digest);
    }

    /// The order of the nodes, the links, the groups and the bindings means
    /// nothing: the same graph with those lists in another order digests
    /// alike. A node's terminal lists are another matter: see
    /// `a_sink_list_in_another_order_is_another_configuration`.
    #[test]
    fn a_graph_in_another_order_digests_alike() {
        let graph = example_graph("thermostat", "ytcs");
        let digest = digest_of_graph(&graph).expect("it digests");

        let mut shuffled = graph.clone();
        shuffled.nodes.reverse();
        shuffled.links.reverse();
        shuffled.groups.reverse();
        shuffled.commands.reverse();
        assert_eq!(digest_of_graph(&shuffled).expect("it digests"), digest);
    }

    /// Writes a file into a directory.
    ///
    /// # Arguments
    ///
    /// * `dir`  - The directory.
    /// * `name` - The file's name.
    /// * `text` - What it holds.
    ///
    /// # Returns
    ///
    /// The file's path.
    ///
    /// # Panics
    ///
    /// If it cannot be written.
    fn written(dir: &Path, name: &str, text: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, text).expect("written");
        path
    }

    /// Two files that differ only in the order they write things in are one
    /// configuration: the types of a section, the nodes of a type, a node's
    /// attributes, the groups, the command bindings, the includes, and the
    /// entries of an attribute that is a set. Not a node's terminal lists --
    /// its `world-in`, `moc-in`, `world-out` and `moc-out` -- whose order is
    /// their own: see
    /// `a_sink_list_in_another_order_is_another_configuration`.
    #[test]
    fn a_file_in_another_order_digests_alike() {
        let dir = tempfile::tempdir().expect("a directory");
        let one = written(
            dir.path(),
            "one.ytcs",
            "include:\n  - subdiagram: a\n    path: a\n  - subdiagram: b\n    path: b\n\
             payload_groups:\n  - { name: g1, interval: 1 s }\n  \
             - { name: g2, interval: 2 s }\n\
             vehicle:\n  - type compute:\n      - compute: v\n        \
             function: vote\n        world-in: [in0, in1, in2]\n        \
             world-out:\n          - x.v\n          - y.v\n  \
             - type log:\n      - { compute: x, world-in: [v] }\n      \
             - { compute: y, world-in: [v] }\n\
             payloads:\n  - type gpio_in:\n      - payload: p\n        \
             lines: \"0, 1\"\n        chip: c\n  \
             - type serial:\n      - payload: s\n        \
             terminators:\n          - '[0A]'\n          - '[0D]'\n\
             commands:\n  - { from: x, sets: v.a }\n  \
             - { from: y, sets: v.b }\n",
        );
        let two = written(
            dir.path(),
            "two.ytcs",
            "commands:\n  - { from: y, sets: v.b }\n  \
             - { from: x, sets: v.a }\n\
             payloads:\n  - type serial:\n      - payload: s\n        \
             terminators:\n          - '[0D]'\n          - '[0A]'\n  \
             - type gpio_in:\n      - payload: p\n        chip: c\n        \
             lines: \"1 0\"\n\
             vehicle:\n  - type log:\n      \
             - { compute: y, world-in: [v] }\n      \
             - { compute: x, world-in: [v] }\n  - type compute:\n      \
             - compute: v\n        \
             world-out:\n          - x.v\n          \
             - y.v\n        world-in: [in0, in1, in2]\n        \
             function: vote\n\
             payload_groups:\n  - { name: g2, interval: 2 s }\n  \
             - { name: g1, interval: 1 s }\n\
             include:\n  - subdiagram: b\n    path: b\n  - subdiagram: a\n    path: a\n",
        );
        written(
            dir.path(),
            "a.ytcs",
            "payloads:\n  - type i2c:\n      - payload: pa\n",
        );
        written(
            dir.path(),
            "b.ytcs",
            "payloads:\n  - type i2c:\n      - payload: pb\n",
        );
        let first = digest_of_file(&one).expect("one digests");
        assert_eq!(digest_of_file(&two).expect("two digests"), first);

        // And the node ids are the same, the include order and all.
        let ids = |path: &Path| {
            crate::graph::telemetry::ids(
                &crate::graph::Graph::load(path).expect("it reads"),
            )
        };
        assert_eq!(ids(&one), ids(&two));
        assert_eq!(ids(&one)["pa"], 1);
        assert_eq!(ids(&one)["y"], 6);

        // A set's entries are a set, and a trigger's bytes are not.
        assert_eq!(
            unordered("compute", "inputs", "in2, in1 in0").as_deref(),
            Some("in0, in1, in2")
        );
        assert_eq!(unordered("serial", "trigger", "[01 02]"), None);
        assert_eq!(unordered("serial", "init", "[01 02]"), None);
    }

    /// A node's terminal lists -- the names under its `world-in` and
    /// `moc-in`, and the destinations under its `world-out` and `moc-out` --
    /// are digested in the order its entry lists them, so that a node that
    /// makes something of their order is held to one order on the vehicle
    /// and the ground alike. The same names in another order are another
    /// configuration.
    #[test]
    fn a_sink_list_in_another_order_is_another_configuration() {
        let graph = example_graph("thermostat", "ytcs");
        let digest = digest_of_graph(&graph).expect("it digests");

        let mut reordered = graph.clone();
        let (node, sinks) = reordered
            .named
            .sinks
            .iter_mut()
            .find(|(_, sinks)| sinks.world_in.len() > 1)
            .expect("a node with several world-in names");
        let node = node.clone();
        sinks.world_in.reverse();
        assert_ne!(
            digest_of_graph(&reordered).expect("it digests"),
            digest,
            "{node}'s world-in reversed digests as it was"
        );

        let mut reordered = graph.clone();
        if let Some(sinks) = reordered
            .named
            .sinks
            .values_mut()
            .find(|sinks| sinks.moc_in.len() > 1)
        {
            sinks.moc_in.reverse();
            assert_ne!(digest_of_graph(&reordered).expect("digests"), digest);
        }

        let mut reordered = graph.clone();
        let link = reordered
            .links
            .iter_mut()
            .find(|link| link.to.each().len() > 1)
            .expect("a source with several destinations");
        let from = link.from.clone();
        let mut to: Vec<String> =
            link.to.each().into_iter().map(str::to_string).collect();
        to.reverse();
        link.to = crate::graph::Destinations::of(to);
        assert_ne!(
            digest_of_graph(&reordered).expect("it digests"),
            digest,
            "{from}'s destinations reversed digest as they were"
        );
    }

    /// A simulator file's digest is of what it and its model say: two
    /// written in other orders, with other comments and descriptions, digest
    /// alike, and a setting or a figure of the model changed does not.
    #[test]
    fn a_simulator_file_in_another_order_digests_alike() {
        let dir = tempfile::tempdir().expect("a directory");
        let model = "world:\n  ambient_temp: \"1 K\"\n  physics:\n    \
                     components:\n      - type heater:\n          \
                     - component: h1\n            power: \"1 W\"\n            \
                     voltage: \"1 V\"\n          - component: h2\n            \
                     voltage: \"1 V\"\n            power: \"1 W\"\n      \
                     - type thermal_mass:\n          - component: z\n            \
                     time_constant: 1 s\n            capacity: \"1 J/K\"\n    \
                     connections:\n      - { from: h1.heat, to: z.heat }\n      \
                     - { from: h2.heat, to: z.heat }\n";
        let reordered = model.replace(
            "      - { from: h1.heat, to: z.heat }\n      \
             - { from: h2.heat, to: z.heat }\n",
            "      - { from: h2.heat, to: z.heat }\n      \
             - { from: h1.heat, to: z.heat }\n",
        );
        assert_ne!(model, reordered);
        let one = written(
            dir.path(),
            "one-sim.yaml",
            &format!(
                "description: one\ntcs-sim:\n  ready: 127.0.0.1:7999\n\
                 {model}\
                 simulated_payload_groups:\n  - group: g\n    \
                 simulate: true\n  - group: f\n    packet_interval: 1 s\n    \
                 simulate: true\n\
                 simulated_payloads:\n  - type network:\n      \
                 - payload: a\n        protocol: udp\n        group: f\n  \
                 - type spi:\n      - payload: b\n        group: g\n        \
                 packet_interval: 2 s\n"
            ),
        );
        let text = format!(
            "# The same, written otherwise.\n\
             simulated_payloads:\n  - type spi:\n      \
             - payload: b\n        packet_interval: 2 s\n        \
             group: g\n  - type network:\n      - payload: a\n        \
             group: f\n        protocol: udp\n\
             simulated_payload_groups:\n  - group: f\n    simulate: true\n    \
             packet_interval: 1 s\n  - group: g\n    simulate: true\n\
             {reordered}\
             tcs-sim:\n  ready: 127.0.0.1:7999\ndescription: two\n"
        );
        let two = written(dir.path(), "two-sim.yaml", &text);
        let first = digest_of_simulator_file(&one).expect("one digests");
        assert_eq!(digest_of_simulator_file(&two).expect("digests"), first);

        // A setting changed is another file.
        std::fs::write(&two, text.replace("2 s", "3 s")).expect("written");
        assert_ne!(digest_of_simulator_file(&two).expect("digests"), first);

        // And so is the world's ambient temperature, and a figure of its
        // model.
        std::fs::write(&two, text.replace("\"1 K\"", "\"2 K\""))
            .expect("written");
        assert_ne!(digest_of_simulator_file(&two).expect("digests"), first);
        std::fs::write(&two, text.replace("\"1 J/K\"", "\"2 J/K\""))
            .expect("written");
        assert_ne!(digest_of_simulator_file(&two).expect("digests"), first);
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
            digest_of_graph(&example_graph("minimal", "ytcs"))
                .expect("it digests")
        );
        assert_eq!(said.len(), 32, "{said}");
        assert!(said.chars().all(|c| c.is_ascii_hexdigit()), "{said}");
    }
}
