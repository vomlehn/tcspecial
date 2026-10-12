//! The version 2 configuration: a graph of nodes.
//!
//! What a vehicle does, as `design.rst` specifies it. Four things and no more:
//! nodes, groups of nodes that share a sampling interval, links between nodes,
//! and bindings that say which commands from the ground may set which
//! attribute of which node.
//!
//! One logical configuration, which may be written as several files. The top
//! file may name others in an `include` list and their contents are merged
//! into it; nothing is discovered by scanning a directory, so the set of files
//! a configuration is made of is itself written down. Either format: the
//! extension decides, through [`crate::format::load_config_file`], which is
//! the one entry point every loader here uses.
//!
//! This reads version 2 and nothing else. Version 1 -- the `payloads` and
//! `payload_groups` of the shipped sets -- describes a different thing, and a
//! reader that took both would be two sets of rules to keep correct for as
//! long as the software lives. The two are told apart by the `version` field
//! and a file of the wrong one is refused saying so.
//!
//! Reading a file is not accepting it. [`Graph::check`] is what says whether a
//! configuration can be flown, and `tcsverify` runs it on the ground against
//! the same code that runs again aboard.

pub mod check;
pub mod parameters;
pub mod reach;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use crate::endpoint_config::Scalar;
use crate::error::{TcsError, TcsResult};
use crate::format::ConfigFormat;

/// The version this module reads.
pub const VERSION: &str = "2.0";

/// One file of a configuration, as it is written.
///
/// Every list defaults to empty, so a file that says only what it has is
/// legal: a subdiagram holding nodes and links need not mention groups, and a
/// top file that is nothing but an include list need not mention nodes.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GraphFile {
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Other files this one is made of, by name without an extension.
    ///
    /// Named rather than discovered. Each is read in the same format as the
    /// file that named it: a set of files half in YAML and half in XML is
    /// something nobody meant and everybody would have to remember.
    #[serde(default)]
    pub include: Vec<String>,
    /// What tcspecial itself is configured with: where commands are taken,
    /// where beacons go, where the telemetry log is written.
    ///
    /// The same section a payload set's file holds, read by the same struct --
    /// see [`CIConfigJson`]. One statement of what the command interpreter
    /// needs, so that a graph and a payload set cannot come to describe it
    /// differently.
    ///
    /// Optional here and required by [`Graph::check`], which is the difference
    /// between a file that cannot be read and a configuration that is
    /// incomplete: the second is worth a complaint naming the file and saying
    /// what to add, and the first is a serde error nobody can act on.
    ///
    /// Held as the words the file holds, and turned into the struct by
    /// [`parameters::ci_config_of`]. The editor has to be able to write a
    /// section somebody is halfway through filling in, and a file it cannot
    /// read back is a file it has destroyed -- see `parameters.rs`, which has
    /// the argument.
    #[serde(default)]
    pub tcspecial: Option<BTreeMap<String, Scalar>>,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub links: Vec<Link>,
    #[serde(default)]
    pub commands: Vec<CommandBinding>,
}

/// A set of nodes read on one interval.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub name: String,
    /// How often the group is read: a whole number with a unit, `500ms` or
    /// `3s`. The same grammar a timeout uses in version 1, so that a reader
    /// who has learned one has learned the other.
    pub interval: String,
}

impl Group {
    /// The interval as a duration, or what is wrong with it.
    pub fn interval(&self) -> Result<Duration, String> {
        parse_interval(&self.interval)
    }
}

/// One node: a device, a MOC link, a compute element or a sink.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Node {
    pub name: String,
    /// What it is. `type` in the file, which is the word the configuration has
    /// always used and the word the protocol uses -- see `DHType`.
    #[serde(rename = "type")]
    pub node_type: String,
    /// The group it is read with, where it is in one.
    #[serde(default)]
    pub group: Option<String>,
    /// Everything else the type requires, as written.
    ///
    /// Collected rather than declared field by field, because what a node must
    /// say depends on its type and a struct with every type's fields on it
    /// would let an I2C node carry a baud rate. What each type requires is
    /// checked in [`check`], against the one list of it.
    ///
    /// Text and not numbers: what a value means is the type's business, and a
    /// reader that parsed `0x48` into an integer here would have to know which
    /// fields are addresses before it knew what the node was.
    ///
    /// [`Scalar`] rather than `String`, which is not a detail. A YAML reader
    /// turns `0x48` into the integer 72 and `yes` into a boolean before
    /// anything here sees it, and an XML value arrives as an element with text
    /// in it rather than as a string -- so a map of strings cannot hold what
    /// either format actually writes. Scalar is version 1's answer to exactly
    /// that and it is not version 1's concern: a value as a file spelled it is
    /// a question about formats. It should move to `format` when version 1
    /// goes.
    #[serde(flatten)]
    pub attributes: BTreeMap<String, Scalar>,
    /// Where the editor drew this node, which the flight software ignores.
    ///
    /// Declared rather than swept up with the attributes, for two reasons. A
    /// position is two numbers and the attributes are text, so flattening it
    /// in would make the attribute map hold a thing that is not an attribute;
    /// and a node's attributes are checked against what its type requires, so
    /// an undeclared `editor` would be reported as one the type does not know.
    #[serde(default)]
    pub editor: Option<Editor>,
}

impl Node {
    /// Whether this node is a payload: something the vehicle carries.
    ///
    /// A device on a bus, a line it drives, a link to something aboard. The
    /// rest of a graph is not -- a computation, a log and the ground link are
    /// parts of the telemetry and control system itself rather than things
    /// bolted to the vehicle for a flight.
    ///
    /// Listed rather than taken as "everything else", so that a type nothing
    /// recognises is not a payload: a misspelled type is already refused by
    /// name, and counting it as a payload would pile a second complaint on
    /// top of the one that says what is actually wrong.
    pub fn is_payload(&self) -> bool {
        matches!(
            self.node_type.as_str(),
            "i2c"
                | "spi"
                | "serial"
                | "hdlc"
                | "can"
                | "iio"
                | "rpmsg"
                | "network"
                | "device-file"
                | "gpio"
        )
    }
}

/// What an editor keeps about a node that a vehicle does not need.
///
/// Only a position so far. It is in the configuration rather than in a file
/// beside it because a person who arranges a diagram expects the arrangement
/// to survive being closed, and because a second file to keep in step is a
/// second file to lose.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Editor {
    pub x: i32,
    pub y: i32,
}

/// A connection from one node to one or more destinations.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub from: String,
    /// Where it goes. One destination or several: several is how one reading
    /// reaches the ground and an on-board computation at once.
    pub to: Destinations,
    #[serde(default)]
    pub comment: Option<String>,
}

/// One or several destinations, however the file wrote them.
///
/// `to: moc` and `to: [moc, mean.in0]` are both legal, because a link with one
/// destination is the ordinary case and making it write a list of one would be
/// ceremony. In XML the same thing is repeated `<to>` elements.
///
/// Read by hand rather than as an untagged enum, for the reason [`Scalar`]
/// exists: a value written as an XML child element does not arrive as a
/// string. quick-xml hands it over as a map of one entry whose value is the
/// text, so a `String` variant never matches and the whole enum fails -- which
/// is what it did, on the shipped example, until this was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destinations(Vec<String>);

impl Destinations {
    /// Each destination, as written.
    pub fn each(&self) -> Vec<&str> {
        self.0.iter().map(String::as_str).collect()
    }

    /// These destinations, for the tests and for anything building a graph
    /// rather than reading one.
    pub fn of(destinations: Vec<String>) -> Destinations {
        Destinations(destinations)
    }
}

impl<'de> Deserialize<'de> for Destinations {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::{self, MapAccess, SeqAccess, Visitor};
        use std::fmt;

        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = Destinations;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a destination, or a list of them")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Destinations, E> {
                Ok(Destinations(vec![v.to_string()]))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Destinations, A::Error> {
                let mut all = Vec::new();
                // Each element may itself be a string or an XML element with
                // text in it, so each is read as a Scalar.
                while let Some(one) = seq.next_element::<Scalar>()? {
                    all.push(one.as_str().to_string());
                }
                Ok(Destinations(all))
            }

            /// One XML element with text in it: `<to>moc</to>`.
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Destinations, A::Error> {
                let mut text: Option<String> = None;
                while let Some((_, value)) = map.next_entry::<String, String>()? {
                    text = Some(value);
                }
                text.map(|t| Destinations(vec![t]))
                    .ok_or_else(|| de::Error::custom("a destination with nothing in it"))
            }
        }

        d.deserialize_any(V)
    }
}

/// Which node and which input a destination names.
///
/// `mean_temp.in0` is the `in0` input of `mean_temp`; `moc` is a node that
/// takes one input and does not name it. The separator is a dot because a node
/// name cannot contain one and an input name cannot either, which is checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destination {
    pub node: String,
    pub input: Option<String>,
}

impl Destination {
    /// Read a destination as a file wrote it.
    pub fn parse(text: &str) -> Destination {
        match text.split_once('.') {
            Some((node, input)) if !node.is_empty() && !input.is_empty() => Destination {
                node: node.to_string(),
                input: Some(input.to_string()),
            },
            // Total on purpose: saying what is wrong with a destination
            // belongs to the checking, which has a link to attach the
            // complaint to.
            _ => Destination {
                node: text.to_string(),
                input: None,
            },
        }
    }
}

/// A command from the ground that sets an attribute of a node.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CommandBinding {
    /// The MOC node the command arrives on.
    pub from: String,
    /// What it sets, as `node.attribute`.
    pub sets: String,
}

impl CommandBinding {
    /// The node and attribute this binding names.
    pub fn target(&self) -> Destination {
        Destination::parse(&self.sets)
    }
}

/// A whole configuration, assembled from its files.
#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    pub description: Option<String>,
    /// What tcspecial itself is configured with, from the top file.
    ///
    /// The top file's and no other's: a configuration has one command
    /// interpreter, so one of its files says what that is. An included file
    /// that declares a section of its own is refused when the configuration is
    /// read rather than quietly ignored -- two sections would be two answers
    /// to one question, and whichever won would be an accident of the order
    /// the files happened to be read in.
    pub tcspecial: Option<BTreeMap<String, Scalar>>,
    pub groups: Vec<Group>,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    pub commands: Vec<CommandBinding>,
    /// Every file the configuration was read from, the top one first.
    ///
    /// Kept so that the digest the beacon carries can be taken over the whole
    /// set rather than over the top file, and so that a complaint can say
    /// which file a node came from.
    pub files: Vec<PathBuf>,
    /// Which file each node was read from, by node name.
    pub homes: BTreeMap<String, PathBuf>,
}

impl Graph {
    /// Read a configuration, following its includes.
    ///
    /// The top file first, then each file it names, each in the top file's
    /// format and beside it. An include that names a file twice, or a cycle of
    /// includes, is refused rather than read twice.
    pub fn load<P: AsRef<Path>>(path: P) -> TcsResult<Graph> {
        let path = path.as_ref();
        let mut graph = Graph {
            description: None,
            tcspecial: None,
            groups: Vec::new(),
            nodes: Vec::new(),
            links: Vec::new(),
            commands: Vec::new(),
            files: Vec::new(),
            homes: BTreeMap::new(),
        };

        let top: GraphFile = read_one(path)?;
        graph.description = top.description.clone();
        graph.tcspecial = top.tcspecial.clone();
        graph.absorb(path, &top);
        graph.files.push(path.to_path_buf());

        for name in &top.include {
            let included = beside(path, name);
            if graph.files.contains(&included) {
                return Err(TcsError::Config(format!(
                    "{}: {} is included twice",
                    path.display(),
                    included.display()
                )));
            }
            let file: GraphFile = read_one(&included)?;
            if file.tcspecial.is_some() {
                return Err(TcsError::Config(format!(
                    "{}: an included file may not have a tcspecial section -- a \
                     configuration has one command interpreter, and the file \
                     that was asked for says what it is",
                    included.display()
                )));
            }
            if !file.include.is_empty() {
                // One level, and the refusal says why rather than silently
                // ignoring it: a subdiagram that included another would be a
                // nesting this has no need for and the editor does not make.
                return Err(TcsError::Config(format!(
                    "{}: an included file may not include others",
                    included.display()
                )));
            }
            graph.absorb(&included, &file);
            graph.files.push(included);
        }

        Ok(graph)
    }

    /// Add one file's contents to the whole.
    fn absorb(&mut self, from: &Path, file: &GraphFile) {
        for node in &file.nodes {
            self.homes.insert(node.name.clone(), from.to_path_buf());
        }
        self.groups.extend(file.groups.iter().cloned());
        self.nodes.extend(file.nodes.iter().cloned());
        self.links.extend(file.links.iter().cloned());
        self.commands.extend(file.commands.iter().cloned());
    }

    /// The node of this name, if the configuration has one.
    pub fn node(&self, name: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.name == name)
    }

    /// The group of this name, if the configuration has one.
    pub fn group(&self, name: &str) -> Option<&Group> {
        self.groups.iter().find(|g| g.name == name)
    }
}

/// Just the version, from a file that may be of any version.
///
/// Read first and on its own, which matters. The full structure refuses a
/// field it does not know -- that is what catches a misspelling -- so reading
/// a version 1 file as a graph complains about `payloads` being an unknown
/// field. True, and the wrong thing to say: the useful complaint is that the
/// file is version 1, and it cannot be made until the version has been read
/// without the rest getting in the way.
#[derive(Deserialize)]
struct JustTheVersion {
    #[serde(default)]
    version: Option<String>,
}

/// What version a file says it is, or nothing if it cannot be asked.
///
/// Nothing covers every way the question can fail to have an answer: a name
/// that is not a configuration file, a file that cannot be read, one that does
/// not parse, one that says no version at all. A caller choosing which checks
/// to run wants one answer to all of those -- "not version 2" -- and the
/// better complaint about an unreadable file belongs to whatever tries to read
/// it properly.
pub fn version_of(path: &Path) -> Option<String> {
    let format = ConfigFormat::of_file(path).ok()?;
    let text = std::fs::read_to_string(path).ok()?;
    let said: JustTheVersion = format.parse(&text).ok()?;
    said.version
}

/// Whether this file says it is the version this module reads.
pub fn is_version_2(path: &Path) -> bool {
    version_of(path).as_deref() == Some(VERSION)
}

/// Read one file and require it to be version 2.
///
/// The version is checked here rather than in the checking, because a version 1
/// file is not a graph with problems in it -- it is a different language, and
/// every complaint the checking made about it would be noise.
fn read_one(path: &Path) -> TcsResult<GraphFile> {
    let format = ConfigFormat::of_file(path)?;
    let text = std::fs::read_to_string(path)
        .map_err(|e| TcsError::Config(format!("{}: {e}", path.display())))?;

    let version: JustTheVersion = format
        .parse(&text)
        .map_err(|e| TcsError::Config(format!("{}: {e}", path.display())))?;
    match version.version.as_deref() {
        Some(VERSION) => {}
        Some(other) => {
            return Err(TcsError::Config(format!(
                "{}: this reads version {VERSION} configurations and that one \
                 says version {other}. A version 1 file -- one with payloads \
                 and payload_groups in it -- describes a different thing and \
                 is not read here",
                path.display()
            )))
        }
        None => {
            return Err(TcsError::Config(format!(
                "{}: a configuration says which version it is, and this one \
                 says nothing. Version {VERSION} is what this reads",
                path.display()
            )))
        }
    }

    format
        .parse(&text)
        .map_err(|e| TcsError::Config(format!("{}: {e}", path.display())))
}

/// Where an included file sits: beside the file that named it, in the same
/// format.
fn beside(top: &Path, name: &str) -> PathBuf {
    let extension = top
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("yaml")
        .to_string();
    let mut path = top.to_path_buf();
    path.set_file_name(format!("{name}.{extension}"));
    path
}

/// A whole number with a unit: `us`, `ms` or `s`.
///
/// The same grammar version 1 uses for a timeout, deliberately: a reader who
/// has learned one has learned the other. Nought is refused -- a group read
/// every nought seconds is a group read as fast as the bus will go, which is
/// a thing to ask for in words rather than by writing zero.
pub fn parse_interval(text: &str) -> Result<Duration, String> {
    let lower = text.trim().to_ascii_lowercase();

    let (digits, unit) = match lower.strip_suffix("us") {
        Some(d) => (d, "us"),
        None => match lower.strip_suffix("ms") {
            Some(d) => (d, "ms"),
            None => match lower.strip_suffix('s') {
                Some(d) => (d, "s"),
                None => {
                    return Err(format!(
                        "\"{text}\" has no unit: a number followed by us, ms or s"
                    ))
                }
            },
        },
    };

    let n: u64 = digits
        .trim()
        .parse()
        .map_err(|_| format!("\"{text}\" is not a whole number of {unit}"))?;

    let interval = match unit {
        "us" => Duration::from_micros(n),
        "ms" => Duration::from_millis(n),
        _ => Duration::from_secs(n),
    };

    if interval.is_zero() {
        return Err(format!("\"{text}\" is no interval at all"));
    }
    Ok(interval)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// The two spellings of a shipped example say the same thing.
    ///
    /// Not a comparison of the text, which differs, but of what each
    /// describes: the same groups, the same nodes with the same attributes,
    /// and the same links. Every example in this repository is kept in both
    /// formats, and the two drifting apart is what this catches -- the
    /// thermostat's pair is written by the editor and compared there, where
    /// these are written by hand and have nothing else to hold them together.
    #[test]
    fn the_two_spellings_of_an_example_agree() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

        for name in ["example", "tests/manual/thermostat"] {
            let yaml = Graph::load(root.join(format!("{name}.yaml")))
                .unwrap_or_else(|e| panic!("{name}.yaml: {e}"));
            let xml = Graph::load(root.join(format!("{name}.xml")))
                .unwrap_or_else(|e| panic!("{name}.xml: {e}"));

            assert_eq!(yaml.description, xml.description, "{name}: description");
            assert_eq!(yaml.groups, xml.groups, "{name}: groups");
            assert_eq!(yaml.nodes, xml.nodes, "{name}: nodes");
            assert_eq!(yaml.links, xml.links, "{name}: links");
            assert_eq!(yaml.commands, xml.commands, "{name}: commands");
        }
    }

    /// An interval is a number and a unit, and nought is not one.
    #[test]
    fn an_interval_is_a_number_and_a_unit() {
        assert_eq!(parse_interval("3s").unwrap(), Duration::from_secs(3));
        assert_eq!(parse_interval("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_interval("250us").unwrap(), Duration::from_micros(250));
        assert_eq!(parse_interval(" 3s ").unwrap(), Duration::from_secs(3));

        for bad in ["3", "fast", "", "s", "3 seconds", "-1s"] {
            assert!(parse_interval(bad).is_err(), "{bad:?} was accepted");
        }
        // Nought is refused rather than taken for "as fast as possible".
        for zero in ["0s", "0ms", "0us"] {
            let said = parse_interval(zero).expect_err("nought should be refused");
            assert!(said.contains("no interval at all"), "{said}");
        }
    }

    /// A destination names a node, and an input where it says one.
    #[test]
    fn a_destination_may_name_an_input() {
        let plain = Destination::parse("moc");
        assert_eq!(plain.node, "moc");
        assert_eq!(plain.input, None);

        let named = Destination::parse("mean_temp.in0");
        assert_eq!(named.node, "mean_temp");
        assert_eq!(named.input.as_deref(), Some("in0"));

        // Total: what is wrong with a destination is the checking's to say,
        // which has a link to attach the complaint to.
        for odd in [".in0", "mean.", ".", ""] {
            let read = Destination::parse(odd);
            assert_eq!(read.input, None, "{odd:?}");
            assert_eq!(read.node, odd, "{odd:?}");
        }
    }

    /// The shipped example reads, in both formats, and says the same thing.
    ///
    /// The real test of the reader: tests/manual/thermostat.yaml and .xml are
    /// written by tcsgui, not by hand here, so this is the editor and the
    /// flight-side reader agreeing about a file rather than two pieces of this
    /// module agreeing with each other.
    #[test]
    fn the_shipped_example_reads_the_same_in_both_formats() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

        let yaml = Graph::load(root.join("tests/manual/thermostat.yaml"))
            .expect("the YAML example reads");
        let xml = Graph::load(root.join("tests/manual/thermostat.xml"))
            .expect("the XML example reads");

        assert_eq!(yaml.nodes, xml.nodes, "the two formats describe different nodes");
        assert_eq!(yaml.links, xml.links, "the two formats describe different links");
        assert_eq!(yaml.commands, xml.commands);
        assert_eq!(yaml.groups, xml.groups);

        // And it is the thermostat: three sensors, a mean, a comparison, the
        // ground, the heaters and a log.
        assert_eq!(
            yaml.nodes.len(),
            11,
            "{:?}",
            yaml.nodes.iter().map(|n| &n.name).collect::<Vec<_>>()
        );
        assert_eq!(yaml.groups.len(), 1, "the three sensors share one group");
        assert_eq!(
            yaml.group("cabin_temps").map(|g| g.interval()),
            Some(Ok(std::time::Duration::from_secs(3)))
        );
        let sensor = yaml.node("temp_fore").expect("the fore sensor");
        assert_eq!(sensor.node_type, "i2c");
        assert_eq!(
            sensor.attributes.get("address").map(Scalar::as_str),
            Some("0x48")
        );
        assert_eq!(sensor.editor.as_ref().map(|e| (e.x, e.y)), Some((40, 40)));
    }

    /// The example is a configuration that could be flown.
    ///
    /// Reading it is one thing and accepting it another, and this is the half
    /// that matters: every link names nodes that exist, every input is bound
    /// once, every group is real.
    #[test]
    fn the_shipped_example_passes_its_own_checks() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let graph = Graph::load(root.join("tests/manual/thermostat.yaml")).expect("it reads");

        let problems = graph.check();
        assert!(
            problems.is_empty(),
            "the shipped example is refused:\n{}",
            problems
                .iter()
                .map(|p| format!("  {}", p.describe()))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// A configuration made of several files is read as one.
    #[test]
    fn an_included_file_is_merged_into_the_one_that_named_it() {
        let where_ = tempfile::tempdir().expect("a directory");
        let top = where_.path().join("vehicle.yaml");

        // With a tcspecial section, which every configuration must have and
        // only the top file may: see Graph::tcspecial.
        std::fs::write(
            &top,
            "version: \"2.0\"\ninclude:\n  - cabin\n\
             tcspecial:\n  address: 0.0.0.0\n  port: 4000\n  payload_port: 4001\n  \
             protocol: udp\n  beacon_interval_ms: 5000\n  \
             beacon_address: \"239.255.0.1:5550\"\n  beacon_interface: \"127.0.0.1\"\n\
             nodes:\n  \
             - { name: moc, type: moc }\nlinks:\n  - { from: cabin_sensor, to: moc }\n",
        )
        .unwrap();
        std::fs::write(
            where_.path().join("cabin.yaml"),
            "version: \"2.0\"\nnodes:\n  - { name: cabin_sensor, type: i2c }\n",
        )
        .unwrap();

        let graph = Graph::load(&top).expect("the pair reads as one");
        assert_eq!(graph.nodes.len(), 2);
        assert!(graph.node("cabin_sensor").is_some(), "the include was not read");
        assert_eq!(graph.files.len(), 2, "both files should be recorded");

        // Which file each node came from, so the digest can cover the set and
        // a complaint can say where to look.
        assert!(graph.homes["cabin_sensor"].ends_with("cabin.yaml"));
        assert!(graph.homes["moc"].ends_with("vehicle.yaml"));

        // And a link across the boundary is sound, which is what including a
        // file is for.
        assert_eq!(graph.check(), Vec::new(), "{:?}", graph.check());
    }

    /// A version 1 file is refused, saying what it is.
    ///
    /// Not read and complained about field by field: version 1 is a different
    /// language, and every complaint the checking made about it would be noise
    /// hiding the one thing worth saying.
    #[test]
    fn a_version_1_file_is_refused_as_one() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("old.yaml");
        std::fs::write(
            &path,
            "version: \"1.0\"\ndescription: an old set\npayloads: []\n",
        )
        .unwrap();

        let said = format!("{}", Graph::load(&path).expect_err("version 1 is refused"));
        assert!(said.contains("version 2"), "{said}");
        assert!(said.contains("payloads"), "it should say what version 1 is: {said}");
    }

    /// An include naming the same file twice is refused.
    #[test]
    fn a_file_is_not_included_twice() {
        let where_ = tempfile::tempdir().expect("a directory");
        let top = where_.path().join("vehicle.yaml");
        std::fs::write(
            &top,
            "version: \"2.0\"\ninclude:\n  - cabin\n  - cabin\n",
        )
        .unwrap();
        std::fs::write(
            where_.path().join("cabin.yaml"),
            "version: \"2.0\"\nnodes: []\n",
        )
        .unwrap();

        let said = format!("{}", Graph::load(&top).expect_err("twice is refused"));
        assert!(said.contains("included twice"), "{said}");
    }

    /// An included file may not include others.
    #[test]
    fn an_include_does_not_nest() {
        let where_ = tempfile::tempdir().expect("a directory");
        let top = where_.path().join("vehicle.yaml");
        std::fs::write(&top, "version: \"2.0\"\ninclude:\n  - cabin\n").unwrap();
        std::fs::write(
            where_.path().join("cabin.yaml"),
            "version: \"2.0\"\ninclude:\n  - deeper\n",
        )
        .unwrap();

        let said = format!("{}", Graph::load(&top).expect_err("nesting is refused"));
        assert!(said.contains("may not include others"), "{said}");
    }

    /// A name that is not a configuration file is refused before it is read.
    #[test]
    fn a_file_that_says_nothing_about_its_format_is_refused() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.json");
        std::fs::write(&path, "{}").unwrap();

        let said = format!("{}", Graph::load(&path).expect_err("json is refused"));
        assert!(said.contains(".yaml"), "{said}");
    }

    /// One destination or several, however the file wrote it.
    #[test]
    fn a_link_may_go_to_one_place_or_many() {
        let one: Link = serde_norway::from_str("from: a\nto: b\n").expect("one destination");
        assert_eq!(one.to.each(), vec!["b"]);

        let many: Link =
            serde_norway::from_str("from: a\nto: [b, c.in0]\n").expect("several destinations");
        assert_eq!(many.to.each(), vec!["b", "c.in0"]);
    }
}
