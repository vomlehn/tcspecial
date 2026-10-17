//! The version 2 configuration: a graph of nodes.
//!
//! What a vehicle does, as `design.rst` specifies it. Four things and no more:
//! nodes, groups of nodes that share a sampling interval, links between nodes,
//! and bindings that say which commands from the ground may set which
//! attribute of which node.
//!
//! Three sections of nodes, and they are the three working panes of the
//! editor: `moc` holds the ground links, `vehicle` holds what the vehicle
//! itself runs, and `payloads` holds what it carries. A node's type says which
//! section it belongs in -- see [`Section`] -- and a node written in the wrong
//! one is refused where the file is read, because a file and a drawing that
//! disagree about what a node is are two statements of one fact.
//!
//! A link is an attribute of the node it leaves, named after the data source
//! it leaves by, with one entry per line: `payload-out: [{ target: k_fore }]` is a
//! link from this node's `payload-out` terminal to `k_fore`, and an entry naming an
//! `input` as well feeds a named input of the node it reaches. The short
//! spelling is read too -- `payload-out: k_fore`, `payload-out: [moc, mean_temp.in0]` --
//! and means the same thing; see [`Destinations`].
//!
//! So there is no links section, and a node and the links out of it are read,
//! written and deleted together. The node it arrives at says so as well --
//! `payload-in: true`, under the sink it arrives at -- so that a node's
//! entry says it is connected at both ends; that flag is checked against these
//! links and otherwise never read. See [`mirror`].
//!
//! In memory they are still a list -- see [`Link`] -- because every rule about
//! a link is about both of its ends, and a list of them is what a rule can be
//! written against.
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
pub mod mirror;
pub mod order;
pub mod parameters;
pub mod reach;
pub mod simulated;
pub mod telemetry;
pub mod terminals;

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
    /// Which schema describes this file, where the XML spelling says.
    ///
    /// Read and ignored. A configuration names its own schema -- that is what
    /// makes an editor validate one without being set up to -- and in XML the
    /// way to say so is two attributes on the root element. They are not
    /// configuration: nothing aboard reads them, and the file means the same
    /// without them.
    ///
    /// Named here because `deny_unknown_fields` is worth keeping: a misspelled
    /// section is a section the software silently does nothing about, so every
    /// word a configuration may contain is a word named in this struct. These
    /// two are words it may contain.
    /// The namespace declaration, which keeps its prefix.
    #[serde(default, rename = "@xmlns:xsi")]
    pub xml_schema_namespace: Option<String>,
    /// And the schema itself, which does not: the reader resolves a declared
    /// prefix away, so the attribute arrives under its bare name whatever
    /// prefix the file bound the namespace to. The prefixed spelling is
    /// accepted as well, for a file that declares no namespace at all.
    #[serde(
        default,
        rename = "@noNamespaceSchemaLocation",
        alias = "@xsi:noNamespaceSchemaLocation"
    )]
    pub xml_schema_location: Option<String>,
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
    pub tcspecial: Option<BTreeMap<String, parameters::Parameter>>,
    /// Sets of payloads read on one interval.
    #[serde(default)]
    pub payload_groups: Vec<Group>,
    /// The ground links: what the editor's moc working pane holds.
    #[serde(default)]
    pub moc: Vec<NodeFile>,
    /// What the vehicle itself runs: computations, logs, pass-throughs, pages.
    #[serde(default)]
    pub vehicle: Vec<NodeFile>,
    /// What the vehicle carries: a device on a bus, a line it drives, a link
    /// to something aboard.
    #[serde(default)]
    pub payloads: Vec<NodeFile>,
    #[serde(default)]
    pub commands: Vec<CommandBinding>,
    /// What the editor arranged: where each node was drawn.
    ///
    /// Last in the file, and apart from the nodes, because it is a different
    /// kind of thing. A node entry says what the vehicle carries and what it
    /// is told; this says where somebody put the box on a screen, which the
    /// flight software ignores entirely -- so a reader of a configuration
    /// meets the vehicle first and the drawing at the end, and can stop
    /// reading when the vehicle stops.
    ///
    /// In the configuration all the same, and not in a file beside it: a
    /// person who arranges a diagram expects the arrangement to survive being
    /// closed, two people reading one configuration should see one drawing,
    /// and a second file to keep in step is a second file to lose.
    #[serde(default)]
    pub tcsgui: Option<Tcsgui>,
    /// The sections a version 2 file used to have, kept to be refused.
    ///
    /// Declared so that a file written against the earlier shape is refused by
    /// name, saying where its contents have moved. Without them the complaint
    /// is serde's -- "unknown field `nodes`" -- which says what is wrong and
    /// not what to do about it, and a reader with a file full of them would
    /// have to be told three times.
    #[serde(default)]
    pub nodes: Option<Vec<NodeFile>>,
    #[serde(default)]
    pub links: Option<Vec<Link>>,
    #[serde(default)]
    pub groups: Option<Vec<Group>>,
}

/// What the editor keeps about a configuration that a vehicle does not need.
///
/// Only the positions so far, which is what `tcsgui` means: the editor's own
/// section of the file. Anything else it comes to keep about a drawing belongs
/// here too, so that the question "does the flight software read this" has one
/// answer for the whole section.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct Tcsgui {
    /// Where each node was drawn, by name.
    #[serde(default)]
    pub nodes: Vec<Placed>,
    /// What the editor was told not to go on raising: a node, and the kind of
    /// concern somebody looked at and left.
    ///
    /// A concern is a shape that might be a mistake and might be meant -- a
    /// payload nothing reads yet, a vehicle node a stream stops at -- and the
    /// editor counts them on a button rather than refusing the file. This is
    /// the answer "that one is on purpose", written down so that two people
    /// editing one configuration do not each have to give it again.
    ///
    /// Here rather than in a file of somebody's own for the reason the
    /// positions are: it is a statement about this configuration, and a second
    /// file to keep in step is a second file to lose.
    #[serde(default, rename = "set-aside")]
    pub set_aside: Vec<Aside>,
}

/// One concern somebody has set aside.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Aside {
    /// The node it is about, as the nodes section named it.
    pub node: String,
    /// Which kind of concern: the editor's own word for it, so that a concern
    /// of another kind about the same node is still raised.
    pub concern: String,
}

/// One node's place on the canvas.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Placed {
    /// The node this is about, as the nodes section named it.
    pub name: String,
    pub x: i32,
    pub y: i32,
}

/// Which section of a file a node belongs in.
///
/// The three working panes of the editor, and the same three families the
/// drawing is arranged in: the ground is the other end of the link and not the
/// vehicle at all, the vehicle's own graph persists between missions, and what
/// it carries changes every flight. A file whose sections are those three can
/// be read as "here is the vehicle, and here is what it is carrying".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Moc,
    Vehicle,
    Payloads,
}

impl Section {
    /// What the section is called in a file.
    pub fn name(self) -> &'static str {
        match self {
            Section::Moc => "moc",
            Section::Vehicle => "vehicle",
            Section::Payloads => "payloads",
        }
    }
}

/// Which section a node of this type belongs in.
///
/// The one statement of the rule, read by the file reader and by the editor's
/// panes: a payload is something the vehicle carries, `moc` is the ground link,
/// and everything else -- a computation, a log, a pass-through, a page of its
/// own -- is the vehicle's own. A type this build does not know is the
/// vehicle's: it is refused by name elsewhere, and the vehicle pane is the one
/// a reader looks at first.
pub fn section_of(node_type: &str) -> Section {
    if Node::is_payload_type(node_type) {
        Section::Payloads
    } else if node_type == "moc" {
        Section::Moc
    } else {
        Section::Vehicle
    }
}

/// One node as a file writes it: the node, and the links that leave it.
///
/// Apart from [`Node`] because the two are different things. A node is what the
/// vehicle opens and what the rules are about; the links out of it are a
/// property of the graph, written here because this is where a reader of the
/// file wants them -- beside the node they leave, not in a list at the bottom
/// that has to be read against the nodes to make sense of either.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeFile {
    pub node: Node,
    /// Where this node's groundward data goes: the sinks its `payload-out` reaches.
    pub payload_out: Vec<String>,
    /// And its vehicleward data: the sinks its `moc-out` reaches.
    pub moc_out: Vec<String>,
    /// Whether a line arrives at this node's `payload-in` sink, as the entry
    /// says so.
    ///
    /// A record of the links written at the other end rather than a link, and
    /// the only thing in this structure that nothing downstream reads: see
    /// [`mirror`]. `None` is a file that said nothing about it, which is
    /// allowed; `Some(true)` is a node saying a groundward line arrives here
    /// and `Some(false)` one saying none does, and either is refused where the
    /// links say otherwise.
    pub payload_in: Option<bool>,
    /// And whether one arrives at its `moc-in` sink.
    pub moc_in: Option<bool>,
}

impl NodeFile {
    /// The links this node's sources make.
    ///
    /// One per destination, each `from` naming the node and the data source it
    /// leaves by -- `k_fore.payload-out` -- which is what every rule about a link
    /// already reads: see [`Link::origin`].
    pub fn links(&self) -> Vec<Link> {
        let mut links = Vec::new();
        for (terminal, destinations) in
            [("payload-out", &self.payload_out), ("moc-out", &self.moc_out)]
        {
            if destinations.is_empty() {
                continue;
            }
            links.push(Link {
                from: format!("{}.{terminal}", self.node.name),
                to: Destinations::of(destinations.clone()),
                comment: None,
            });
        }
        links
    }

    /// What this entry said about each of its sinks.
    ///
    /// The mirror of [`NodeFile::links`], and deliberately not links: this is
    /// a record of links written elsewhere, and a record read as a link would
    /// be the same connection in the graph twice.
    pub fn arriving(&self) -> Vec<(&'static str, Option<bool>)> {
        vec![
            ("payload-in", self.payload_in),
            ("moc-in", self.moc_in),
        ]
    }
}

impl<'de> Deserialize<'de> for NodeFile {
    /// Read by hand, for the reason [`Destinations`] is.
    ///
    /// A node's attributes are whatever its type takes, so they are collected
    /// rather than declared -- and `serde(flatten)` cannot be used to collect
    /// them beside a declared `payload-out`, because an XML file writes several
    /// destinations as several `<payload-out>` elements and a flattened map refuses
    /// the second as a duplicate key. Read as a map here, the repeats are what
    /// they look like: more destinations.
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = NodeFile;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a node")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<NodeFile, A::Error> {
                let mut name: Option<String> = None;
                let mut node_type: Option<String> = None;
                let mut group: Option<String> = None;

                let mut attributes: BTreeMap<String, Scalar> = BTreeMap::new();
                let mut payload_out: Vec<String> = Vec::new();
                let mut moc_out: Vec<String> = Vec::new();
                let mut payload_in: Option<bool> = None;
                let mut moc_in: Option<bool> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "name" => name = Some(map.next_value::<Scalar>()?.as_str().to_string()),
                        "type" => {
                            node_type = Some(map.next_value::<Scalar>()?.as_str().to_string())
                        }
                        "group" => {
                            group = Some(map.next_value::<Scalar>()?.as_str().to_string())
                        }
                        // Where this used to be, and said so rather than
                        // read as an attribute nothing knows: a position is
                        // the editor's own and lives in the `tcsgui` section
                        // at the end of the file now.
                        "editor" => {
                            return Err(de::Error::custom(
                                "a node's position is not written on the node \
                                 any more: it is in the tcsgui section at the \
                                 end of the file, as `- { name: <node>, x: \
                                 <x>, y: <y> }`",
                            ))
                        }
                        // A data source, and the one place a file says what is
                        // linked to what. Repeats add to it rather than
                        // replacing it: that is what several XML elements of
                        // one name mean.
                        "payload-out" => payload_out.extend(
                            map.next_value::<Destinations>()?
                                .each()
                                .into_iter()
                                .map(str::to_string),
                        ),
                        "moc-out" => moc_out.extend(
                            map.next_value::<Destinations>()?
                                .each()
                                .into_iter()
                                .map(str::to_string),
                        ),
                        // A data sink, which says whether a line arrives
                        // there -- a record of links written on the nodes they
                        // leave, and not a link. Checked against those links
                        // as the configuration is read: see `mirror`.
                        "payload-in" => {
                            payload_in = Some(said_so("payload-in", &mut map)?)
                        }
                        "moc-in" => moc_in = Some(said_so("moc-in", &mut map)?),
                        // Everything else is an attribute of the node, which
                        // is where a word nothing reads ends up -- and is
                        // refused by name against what the type takes. See
                        // `reach::reach_of`.
                        _ => {
                            let value = map.next_value::<Scalar>()?;
                            if attributes.insert(key.clone(), value).is_some() {
                                return Err(de::Error::custom(format!(
                                    "\"{}\" states {key} twice",
                                    name.as_deref().unwrap_or("a node")
                                )));
                            }
                        }
                    }
                }

                Ok(NodeFile {
                    node: Node {
                        name: name.ok_or_else(|| de::Error::missing_field("name"))?,
                        node_type: node_type
                            .ok_or_else(|| de::Error::missing_field("type"))?,
                        group,
                        attributes,
                    },
                    payload_out,
                    moc_out,
                    payload_in,
                    moc_in,
                })
            }
        }

        d.deserialize_map(V)
    }
}

/// A set of nodes read on one interval.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub name: String,
    /// How often the group is read: a whole number with a unit, `500 ms` or
    /// `3 s`. The same grammar a timeout uses in version 1, so that a reader
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
    ///
    /// A payload need not be a device. Two of the app node types are payloads
    /// -- a payload that is a program rather than something on a bus -- and
    /// which app kinds those are is `crate::node::app`'s to say rather than a
    /// second list here: see [`crate::node::app::Kind::is_payload`].
    pub fn is_payload(&self) -> bool {
        Node::is_payload_type(&self.node_type)
    }

    /// The same question of a type rather than of a node.
    ///
    /// For [`section_of`], which is asked before there is a node to ask about.
    pub fn is_payload_type(node_type: &str) -> bool {
        matches!(
            node_type,
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
        ) || crate::node::app::Kind::of(node_type)
            .is_some_and(|kind| kind.is_payload())
    }
}

/// A connection from one node to one or more destinations.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Link {
    /// Where the data leaves, as `node` or `node.terminal`: see
    /// [`Link::origin`].
    pub from: String,
    /// Where it goes. One destination or several: several is how one reading
    /// reaches the ground and an on-board computation at once.
    pub to: Destinations,
    #[serde(default)]
    pub comment: Option<String>,
}

impl Link {
    /// Which node this link leaves, and by which of its data sources.
    ///
    /// Written the way a destination is -- `temp_fore.payload-out` -- because the
    /// two ends of a link are the same kind of thing: a node and one of its
    /// terminals. A node with one source needs no name after the dot, so every
    /// file written before there were names for these still says what it said.
    pub fn origin(&self) -> Destination {
        Destination::parse(&self.from)
    }
}

/// One or several destinations, however the file wrote them.
///
/// The spelling the editor writes is one entry per line, the node it reaches
/// named by the entry's `target`:
///
/// ```yaml
/// payload-out:
///   - target: moc
///   - target: mean_temp
///     input: in0
/// ```
///
/// A line is then one entry rather than one element of two lists, which is
/// what makes several destinations safe to write: nothing pairs by position,
/// so a list that slips by one cannot turn a reading into somebody else's
/// input. In XML the same thing is a repeated `<payload-out>` with `<target>` in
/// it.
///
/// The short spellings are still read: `payload-out: moc` and `payload-out: [moc,
/// mean_temp.in0]`, the node and its input separated by a dot. Every
/// configuration written before entries existed says what it said, and a link
/// to one place is a line somebody can still write in a line. All of them read
/// into the same thing -- a destination as `node` or `node.input` -- because
/// every rule about a link is about where it goes and not about how the file
/// said so.
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
                // Each element is an entry, a string, or an XML element with
                // text in it: see Bound.
                while let Some(one) = seq.next_element::<Bound>()? {
                    all.push(one.0);
                }
                Ok(Destinations(all))
            }

            /// One entry -- `{ target: moc }` -- or one XML element with text
            /// in it: `<payload-out>moc</payload-out>`.
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Destinations, A::Error> {
                Ok(Destinations(vec![written_as(map)?]))
            }
        }

        d.deserialize_any(V)
    }
}

/// One destination, read from whichever spelling the file used.
///
/// The entry form's own type, so that a list can be a list of entries and a
/// list of names at once: each element is read as one of these and comes out
/// as a destination -- `node`, or `node.input` where the entry named an input.
/// See [`Destinations`] for why there is more than one spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Bound(String);

impl<'de> Deserialize<'de> for Bound {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::{MapAccess, Visitor};
        use std::fmt;

        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = Bound;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a destination, or an entry naming its target")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Bound, E> {
                Ok(Bound(v.to_string()))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Bound, A::Error> {
                Ok(Bound(written_as(map)?))
            }
        }

        d.deserialize_any(V)
    }
}

/// Whether a node's entry says a line arrives at one of its sinks.
///
/// `payload-in: true`, and `false` for a node saying none does. Read through
/// [`Scalar`] rather than as a `bool` because XML has no booleans: the text of
/// `<payload-in>true</payload-in>` is the word, and the word is what both
/// languages have in common.
///
/// Anything else is refused saying what the field means. It named the node the
/// data came from for a while, which read as "and nothing else" the moment a
/// second line arrived -- so a file written against that is refused rather
/// than half understood.
fn said_so<'de, A: serde::de::MapAccess<'de>>(
    sink: &str,
    map: &mut A,
) -> Result<bool, A::Error> {
    use serde::de::Error;

    let said = map.next_value::<Scalar>()?;
    match said.as_str().trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(A::Error::custom(format!(
            "{sink}: \"{other}\" is not true or false. A data sink says \
             whether a line arrives there and nothing more -- which node it \
             comes from is written on that node, as the link itself"
        ))),
    }
}

/// The destination a mapping names, however the mapping got there.
///
/// Two shapes arrive here. An entry -- `{ target: mean_temp, input: in0 }` --
/// which is what the editor writes and what a person writing a link by hand
/// now has to write for a link to a named input. And an XML element with text
/// in it, `<payload-out>moc</payload-out>`, which quick-xml hands over as a map of one
/// entry whose value is the text: the short spelling, in the language where it
/// does not arrive as a string.
///
/// Told apart by `target`, which the entry form always has. A mapping with
/// neither `target` nor exactly one entry is refused rather than guessed at,
/// and so is a word in an entry that is not `target` or `input`: a misspelled
/// `targte` silently ignored is a link that quietly goes nowhere.
fn written_as<'de, A: serde::de::MapAccess<'de>>(mut map: A) -> Result<String, A::Error> {
    use serde::de::Error;

    let mut target: Option<String> = None;
    let mut input: Option<String> = None;
    let mut text: Option<String> = None;
    let mut unknown: Option<String> = None;

    while let Some(key) = map.next_key::<String>()? {
        match key.as_str() {
            "target" => target = Some(map.next_value::<Scalar>()?.as_str().to_string()),
            "input" => input = Some(map.next_value::<Scalar>()?.as_str().to_string()),
            // What XML calls the text of an element, which is how the short
            // spelling arrives in that language: `<payload-out>moc</payload-out>`.
            "$text" => text = Some(map.next_value::<Scalar>()?.as_str().to_string()),
            // And nothing else. A word nobody knows is a misspelling, and a
            // misspelled `target` read as a name would be a link that quietly
            // goes nowhere.
            other => {
                unknown = Some(other.to_string());
                map.next_value::<serde::de::IgnoredAny>()?;
            }
        }
    }

    if let Some(unknown) = unknown {
        return Err(A::Error::custom(format!(
            "\"{unknown}\" is not a word a link entry knows: an entry says \
             `target`, the node the line goes to, and may say `input`, which \
             of that node's inputs it feeds"
        )));
    }

    match (target, input, text) {
        // An entry, which may name the input it feeds.
        (Some(target), input, _) => Ok(match input {
            Some(input) => format!("{target}.{input}"),
            None => target,
        }),
        (None, Some(input), _) => Err(A::Error::custom(format!(
            "a link entry that says `input: {input}` has to say which node \
             that input belongs to, as `target`"
        ))),
        // The short spelling, in XML.
        (None, None, Some(text)) => Ok(text),
        (None, None, None) => Err(A::Error::custom(
            "a destination is a node, or an entry naming its target",
        )),
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
    /// The terminal named after the dot, where one was: a data sink at the end
    /// a link arrives at, a data source at the end it leaves. Named `input`
    /// from when the only terminals were a computation's inputs.
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
    pub tcspecial: Option<BTreeMap<String, parameters::Parameter>>,
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
    /// Where each node was drawn, by node name: the `tcsgui` sections of the
    /// files, merged.
    ///
    /// Apart from the nodes here as it is in the file, and for the same
    /// reason: a node is what the vehicle carries and this is what somebody
    /// arranged. The flight software never looks at it; the editor is the only
    /// reader there is.
    pub placed: BTreeMap<String, Placed>,
    /// What the editor was told not to raise, by node and kind of concern.
    ///
    /// The editor's own, as `placed` is: the flight software never looks at it
    /// -- a concern is a question about a drawing and not about a vehicle --
    /// and the one reader is tcsgui. See [`Tcsgui::set_aside`].
    pub set_aside: std::collections::BTreeSet<(String, String)>,
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
            placed: BTreeMap::new(),
            set_aside: std::collections::BTreeSet::new(),
        };

        // What the files record at the arriving ends of their links, gathered
        // as they are read and checked once they all have been: a link may
        // cross from an included file into the top one, so the two halves of
        // one record are not necessarily in the same file. See `mirror`.
        let mut mirror = mirror::Mirror::default();

        let top: GraphFile = read_one(path)?;
        graph.description = top.description.clone();
        graph.tcspecial = top.tcspecial.clone();
        graph.absorb(path, &top, &mut mirror)?;
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
            graph.absorb(&included, &file, &mut mirror)?;
            graph.files.push(included);
        }

        mirror.check(&graph)?;

        Ok(graph)
    }

    /// Take in one file: its nodes, the links they carry, and the rest.
    ///
    /// The three sections are one list of nodes once they are read, because
    /// every rule about a node is about nodes and not about sections. What the
    /// sections are for is that a reader of the file can see at a glance what
    /// the vehicle carries this flight -- so a node in the wrong one is
    /// refused here, where the complaint can name the file.
    fn absorb(
        &mut self,
        from: &Path,
        file: &GraphFile,
        mirror: &mut mirror::Mirror,
    ) -> TcsResult<()> {
        let first = self.nodes.len();
        for (section, nodes) in [
            (Section::Moc, &file.moc),
            (Section::Vehicle, &file.vehicle),
            (Section::Payloads, &file.payloads),
        ] {
            for said in nodes {
                let belongs = section_of(&said.node.node_type);
                if belongs != section {
                    return Err(TcsError::Config(format!(
                        "{}: \"{}\" is a {} node and is written under {}: it \
                         belongs under {}",
                        from.display(),
                        said.node.name,
                        said.node.node_type,
                        section.name(),
                        belongs.name()
                    )));
                }
                self.homes
                    .insert(said.node.name.clone(), from.to_path_buf());
                self.nodes.push(said.node.clone());
                self.links.extend(said.links());
                mirror.record(from, said);
            }
        }

        // In the order the file declares them, which the sections above do
        // not keep: a node's device id is its position, and that is the order
        // somebody reading the file sees. See `order`.
        let declared = order::declared(from)?;
        let at = |name: &str| declared.iter().position(|n| n == name).unwrap_or(usize::MAX);
        self.nodes[first..].sort_by_key(|node| at(&node.name));

        self.groups.extend(file.payload_groups.iter().cloned());
        self.commands.extend(file.commands.iter().cloned());

        // What the editor arranged, by node name. A node named here and
        // nowhere else is a position left behind by a node somebody deleted:
        // kept rather than refused, because it says nothing about the vehicle
        // and the editor writes the section out again from the nodes it has.
        if let Some(said) = &file.tcsgui {
            for placed in &said.nodes {
                self.placed.insert(placed.name.clone(), placed.clone());
            }
            for aside in &said.set_aside {
                self.set_aside
                    .insert((aside.node.clone(), aside.concern.clone()));
            }
        }
        Ok(())
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

    let file: GraphFile = format
        .parse(&text)
        .map_err(|e| TcsError::Config(format!("{}: {e}", path.display())))?;

    // The sections a version 2 file used to have, refused by name and told
    // where their contents have gone. A file full of them would otherwise be
    // read as far as the first one and complained about in serde's words --
    // "unknown field `nodes`" -- which says what is wrong and not what to do.
    let moved: &[(&str, bool, &str)] = &[
        (
            "nodes",
            file.nodes.is_some(),
            "a node is written under moc, vehicle or payloads, according to what \
             it is",
        ),
        (
            "links",
            file.links.is_some(),
            "a link is an attribute of the node it leaves, named after the data \
             source it leaves by: `payload_out: k_fore`",
        ),
        ("groups", file.groups.is_some(), "groups are now payload_groups"),
    ];
    for (section, there, where_) in moved {
        if *there {
            return Err(TcsError::Config(format!(
                "{}: there is no {section} section any more -- {where_}",
                path.display()
            )));
        }
    }

    Ok(file)
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

/// A length of time: a number, a space, and a unit.
///
/// `500 ms`, `1.5 s`, `15 min`, `6 h`, `1 w`. The space is required, and it is
/// what makes the two halves two things rather than one string a reader has to
/// take apart: a value is read by splitting it, not by knowing every unit
/// there is and looking for one at the end. It also leaves the number free to
/// grow a notation of its own -- hexadecimal, which nothing supports today and
/// nothing is planned to -- without the unit having to be told apart from the
/// digits of it.
///
/// The units are the whole table, and nothing else is one:
///
/// | unit  | what it is   |
/// |-------|--------------|
/// | `us`  | microseconds |
/// | `ms`  | milliseconds |
/// | `s`   | seconds      |
/// | `min` | minutes      |
/// | `h`   | hours        |
/// | `d`   | days         |
/// | `w`   | weeks        |
///
/// Every unit from a microsecond to a week, because a housekeeping reading
/// taken once a week is written `1 w` and not `604800 s`, and a number nobody
/// can read at a glance is a number nobody can check. The same argument
/// covers the middle of the range: `15 min` and `6 h` are how a schedule is
/// discussed, and making somebody write `900 s` and `21600 s` is making them do
/// the arithmetic that the mistake hides in.
///
/// # Nothing ambiguous
///
/// Minutes are `min` and never `m`. A single `m` is one letter from `ms` and a
/// thousand-fold apart from it, so a slip of a finger would have made a
/// reading taken sixty thousand times less often with nothing in the file to
/// say so. It is refused by name, as is anything else that could be read two
/// ways:
///
/// * `m` -- say `min` for minutes or `ms` for milliseconds;
/// * a month or a year -- `mon`, `mo`, `month`, `y`, `yr`, `year` -- which are
///   not lengths of time at all: a month is a calendar's business, needing a
///   reference date and a timezone that nothing aboard a vehicle carries. A
///   span of about that size is written exactly: `30 d`, or a number of days or
///   seconds;
/// * `µs` -- say `us`, this being a grammar of ASCII.
///
/// The units are lowercase and the number is decimal: there is no hexadecimal
/// interval, and none is planned. Underscores among the digits mean nothing,
/// as everywhere else in these files, so `1_000 ms` and `1000 ms` are one
/// interval written twice.
///
/// # The number
///
/// A whole number or a decimal fraction: `1500 ms` and `1.5 s` are the same
/// interval, and `0.5 d` is twelve hours. A fraction is for the times when the
/// unit a person thinks in is not the unit the value divides into -- half a
/// day, a second and a half -- and it is read exactly: the value is kept in
/// whole microseconds, rounded to the nearest, so `1.0000004 s` is a second and
/// nought point four of a microsecond rounded away rather than silently
/// truncated.
///
/// No sign and no exponent. An interval is a length and not a direction, and
/// `1e3s` is a number written for a machine to read rather than a person.
///
/// Nought is refused -- a group read every nought seconds is a group read as
/// fast as the bus will go, which is a thing to ask for in words rather than
/// by writing zero -- and so is an interval too long to hold, which is the
/// other end of the same mistake. A fraction that rounds to nothing is nought
/// by another spelling and is refused with it.
///
/// This is the one reader of a length of time in a configuration, a payload's
/// `timeout` included: see `crate::endpoint_config::parse_timeout`, which is
/// this with the units it may use narrowed.
pub fn parse_interval(text: &str) -> Result<Duration, String> {
    const SECOND: f64 = 1_000_000.0;
    // In order of size rather than of spelling: the unit is a word of its
    // own, so nothing here depends on which spelling ends with which.
    const UNITS: [(&str, f64); 7] = [
        ("us", 1.0),
        ("ms", 1_000.0),
        ("s", SECOND),
        ("min", 60.0 * SECOND),
        ("h", 3_600.0 * SECOND),
        ("d", 24.0 * 3_600.0 * SECOND),
        ("w", 7.0 * 24.0 * 3_600.0 * SECOND),
    ];

    // What a reader might write and this will not take, each with the thing
    // to write instead: said by name, because a unit refused as "no unit at
    // all" sends somebody looking for a typing mistake they did not make.
    const REFUSED: [(&str, &str); 8] = [
        ("m", "say min for minutes or ms for milliseconds"),
        ("mon", "a month is not a length of time: say 30 d, or a number of days"),
        ("mo", "a month is not a length of time: say 30 d, or a number of days"),
        (
            "month",
            "a month is not a length of time: say 30 d, or a number of days",
        ),
        ("y", "a year is not a length of time: say 365 d, or a number of days"),
        ("yr", "a year is not a length of time: say 365 d, or a number of days"),
        (
            "year",
            "a year is not a length of time: say 365 d, or a number of days",
        ),
        ("\u{b5}s", "say us"),
    ];

    let said = text.trim();

    // A space between the number and the unit, which is the whole of the
    // shape: `500 ms`. Written together the two run into one another -- and a
    // reader deciding where the number ends is a reader that has to know
    // every unit before it can find one, which is how `0x1d` comes to be a
    // number that is also a day.
    let Some((digits, unit)) = said.split_once(char::is_whitespace) else {
        return Err(no_space(text, said));
    };
    let unit = unit.trim_start();
    if unit.is_empty() || unit.contains(char::is_whitespace) {
        return Err(format!(
            "\"{text}\" is a number and a unit with a space between them, and \
             nothing else: 500 ms, 1.5 s, 15 min"
        ));
    }

    // Refused by name, so that `15 m` is told what to say rather than being
    // read as fifteen of something else.
    for (refused, instead) in REFUSED {
        if unit == refused {
            return Err(format!("\"{text}\": {instead}"));
        }
    }

    let (unit, micros) = UNITS
        .iter()
        .find(|(known, _)| *known == unit)
        .map(|(known, micros)| (*known, *micros))
        .ok_or_else(|| {
            format!(
                "\"{text}\": \"{unit}\" is not a unit of time: us, ms, s, min, h, \
                 d or w"
            )
        })?;

    let total = whole_micros(text, digits, unit, micros)?;
    let interval = Duration::from_micros(total);
    if interval.is_zero() {
        return Err(format!("\"{text}\" is no interval at all"));
    }
    Ok(interval)
}

/// What to say to a value whose number and unit are written together.
///
/// The commonest thing a file will have wrong while the space is new, and the
/// one worth naming exactly: a message about a unit that is not a unit would
/// send somebody looking at the unit, which is not what is wrong.
fn no_space(text: &str, said: &str) -> String {
    let number: String = said
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '_')
        .collect();
    let unit = &said[number.len()..];
    if number.is_empty() || unit.is_empty() {
        return format!(
            "\"{text}\" is not a length of time: a number, a space, and a unit \
             -- 500 ms, 1.5 s, 15 min"
        );
    }
    format!("\"{text}\": write the number and the unit with a space between them, as \"{number} {unit}\"")
}

/// `digits` of `unit`, in whole microseconds.
///
/// A whole number or a decimal fraction, underscores ignored, read exactly and
/// rounded to the nearest microsecond. In microseconds throughout, so that a
/// week and a microsecond are the same arithmetic -- and checked, because
/// seven hundred thousand weeks overflows the count rather than being an
/// interval anybody meant.
fn whole_micros(text: &str, digits: &str, unit: &str, micros: f64) -> Result<u64, String> {
    let bare: String = digits.chars().filter(|c| *c != '_').collect();
    let bare = bare.trim();
    let bad = || format!("\"{text}\" is not a number of {unit}");

    // Decimal only, and no sign, exponent or anything else a float parser
    // would otherwise take: the grammar is digits, at most one point, digits.
    let mut parts = bare.split('.');
    let before = parts.next().unwrap_or("");
    let after = parts.next().unwrap_or("");
    if parts.next().is_some()
        || bare.is_empty()
        || (before.is_empty() && after.is_empty())
        || !before.chars().all(|c| c.is_ascii_digit())
        || !after.chars().all(|c| c.is_ascii_digit())
    {
        return Err(bad());
    }

    let n: f64 = bare.parse().map_err(|_| bad())?;
    let total = (n * micros).round();
    if !total.is_finite() || total < 0.0 || total > u64::MAX as f64 {
        return Err(format!(
            "\"{text}\" is longer than anything this can keep track of"
        ));
    }
    Ok(total as u64)
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
        assert_eq!(parse_interval("3 s").unwrap(), Duration::from_secs(3));
        assert_eq!(parse_interval("500 ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_interval("250 us").unwrap(), Duration::from_micros(250));
        assert_eq!(parse_interval(" 3 s ").unwrap(), Duration::from_secs(3));

        // The seven units, the long ones in the terms somebody would check
        // them in: a minute of seconds, an hour of minutes, a day of hours
        // and a week of days.
        assert_eq!(parse_interval("1 min").unwrap(), Duration::from_secs(60));
        assert_eq!(parse_interval("15 min").unwrap(), Duration::from_secs(900));
        assert_eq!(parse_interval("1 h").unwrap(), Duration::from_secs(3_600));
        assert_eq!(parse_interval("6 h").unwrap(), Duration::from_secs(21_600));
        assert_eq!(parse_interval("1 d").unwrap(), Duration::from_secs(86_400));
        assert_eq!(parse_interval("2 w").unwrap(), Duration::from_secs(14 * 86_400));
        for (long, short) in [
            ("1 h", "60 min"),
            ("1 d", "24 h"),
            ("1 w", "7 d"),
            ("1 min", "60 s"),
            ("1 min", "60000 ms"),
        ] {
            assert_eq!(
                parse_interval(long).unwrap(),
                parse_interval(short).unwrap(),
                "{long} is {short}"
            );
        }

        // A number or a decimal fraction, and the two ways of writing one
        // interval are one interval. Rounded to the nearest microsecond,
        // which is the smallest thing this counts in.
        assert_eq!(parse_interval("1.5 s").unwrap(), Duration::from_millis(1_500));
        assert_eq!(parse_interval("0.5 d").unwrap(), Duration::from_secs(12 * 3_600));
        assert_eq!(parse_interval("1.5 ms").unwrap(), Duration::from_micros(1_500));
        assert_eq!(
            parse_interval("1.0000004 s").unwrap(),
            Duration::from_micros(1_000_000),
            "a fraction of a microsecond is rounded to the nearest"
        );
        assert_eq!(parse_interval("1.6 us").unwrap(), Duration::from_micros(2));

        // Underscores group the digits and mean nothing, as everywhere else
        // in these files.
        for said in ["1000 ms", "1_000 ms", "1_0_0_0 ms", "1.000 s", "1.0_00 s"] {
            assert_eq!(
                parse_interval(said).unwrap(),
                Duration::from_millis(1000),
                "{said}"
            );
        }

        // Nothing ambiguous. `m` is refused by name rather than read as
        // either of the two things it could be, and a month or a year is
        // refused for not being a length of time at all.
        for (said, instead) in [
            ("15 m", "say min"),
            ("500 m", "say min"),
            ("6 mon", "not a length of time"),
            ("6 mo", "not a length of time"),
            ("6 month", "not a length of time"),
            ("1 y", "not a length of time"),
            ("1 yr", "not a length of time"),
            ("1 year", "not a length of time"),
            ("250 \u{b5}s", "say us"),
        ] {
            let why = parse_interval(said).expect_err("an ambiguous unit");
            assert!(why.contains(instead), "{said}: {why}");
        }

        // No hexadecimal and none planned, no sign, no exponent, and
        // nothing after the unit.
        for bad in [
            "3", "fast", "", "s", "3 seconds", "-1 s", "0b11 s", "0x3e8 ms",
            "1e3 s", "1.2.3 s", ". s", "1..5 s", "+1 s", "1,5 s", "1 s please",
        ] {
            assert!(parse_interval(bad).is_err(), "{bad:?} was accepted");
        }

        // The whitespace is required, and what a value written together is
        // told is where the space goes -- not that its unit is unknown,
        // which is a message about the wrong half of the value.
        for together in ["500ms", "3s", "15min", "1.5s", "1 w2 d"] {
            let why = parse_interval(together).expect_err("written together");
            assert!(
                why.contains("space between them") || why.contains("nothing else"),
                "{together}: {why}"
            );
        }
        assert!(
            parse_interval("500ms")
                .expect_err("written together")
                .contains("\"500 ms\""),
            "the complaint should show what to write"
        );

        // One or more characters of it, and a tab is one: what separates the
        // two halves is whitespace rather than a particular character of it.
        for spaced in ["500 ms", "500  ms", "500\tms", "500 \t ms"] {
            assert_eq!(
                parse_interval(spaced).unwrap(),
                Duration::from_millis(500),
                "{spaced:?}"
            );
        }

        // Nought is refused rather than taken for "as fast as possible", and
        // so is a fraction that rounds to it.
        for zero in ["0 s", "0 ms", "0 us", "0 min", "0 h", "0 d", "0 w", "0.0 s", "0.4 us"] {
            let said = parse_interval(zero).expect_err("nought should be refused");
            assert!(said.contains("no interval at all"), "{zero}: {said}");
        }
        // And so is an interval too long to count, which is the same mistake
        // from the other end: a number that overflowed rather than an
        // interval anybody meant.
        let said = parse_interval("100000000000000000 w")
            .expect_err("a number of weeks past counting should be refused");
        assert!(said.contains("keep track"), "{said}");
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
        // And where it was drawn, which is the editor's section at the end
        // of the file rather than a field on the node: a node says what the
        // vehicle carries, and this says where somebody put the box.
        assert_eq!(
            yaml.placed.get("temp_fore").map(|at| (at.x, at.y)),
            Some((34, 34))
        );
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
            "version: \"2.0\"
include:
  - cabin
tcspecial:
  tcs_cmd:
    recv: 0.0.0.0:4000
    send: 127.0.0.1:4100
  payload_command:
    source_base: 127.0.0.1:4300
    dest_base: 0.0.0.0:4200
  payload_telemetry:
    source_base: 0.0.0.0:6000
    dest_base: 127.0.0.1:6100
  beacon:
    interval: 5 s
    send: \"0.0.0.0:5550\"
    recv: \"127.0.0.1:5650\"
moc:
  - { name: moc, type: moc, address: \"10.0.0.1\", port: \"5000\", ground_base: \"10.0.0.2:5100\" }
",
        )
        .unwrap();
        // And the included file's own payload, with the link that joins the
        // two: a link is an attribute of the node it leaves, so a link across
        // the boundary between two files is written in the file the node is
        // in.
        std::fs::write(
            where_.path().join("cabin.yaml"),
            "version: \"2.0\"\npayloads:\n  \
             - { name: cabin_sensor, type: i2c, path: \"/dev/i2c-1\", \
             address: \"0x48\", payload-out: moc }\n",
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
            "version: \"2.0\"\npayloads: []\n",
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

    /// The entry spelling, which is the one the editor writes.
    ///
    /// One entry per line, each naming its target, so that several
    /// destinations are several entries rather than two lists read against
    /// each other. It means the same thing as the short spelling above, which
    /// is what the assertions say: both come out as a destination.
    #[test]
    fn a_link_may_be_written_as_an_entry_naming_its_target() {
        let said: NodeFile = serde_norway::from_str(
            "name: k_fore
type: compute
payload-out:
  - target: moc
  - target: mean_temp
    input: in0
",
        )
        .expect("entries");
        assert_eq!(said.payload_out, vec!["moc", "mean_temp.in0"]);

        // One entry on its own needs no list, as one destination needs no
        // list.
        let one: NodeFile =
            serde_norway::from_str("name: a\ntype: compute\npayload-out: { target: moc }\n")
                .expect("one entry");
        assert_eq!(one.payload_out, vec!["moc"]);
    }

    /// A misspelled word in an entry is refused, not ignored.
    ///
    /// `targte: moc` read as an entry naming nothing is a link that quietly
    /// goes nowhere, which is the kind of fault a configuration file exists to
    /// make impossible.
    #[test]
    fn a_word_an_entry_does_not_know_is_refused() {
        let said = format!(
            "{}",
            serde_norway::from_str::<NodeFile>(
                "name: a\ntype: compute\npayload-out:\n  - targte: moc\n"
            )
            .expect_err("a misspelling is refused")
        );
        assert!(said.contains("target"), "{said}");

        // And an input with no node to belong to.
        let said = format!(
            "{}",
            serde_norway::from_str::<NodeFile>(
                "name: a\ntype: compute\npayload-out:\n  - input: in0\n"
            )
            .expect_err("an input names no node")
        );
        assert!(said.contains("which node"), "{said}");
    }
}

