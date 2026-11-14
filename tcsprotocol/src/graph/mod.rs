//! A configuration: a graph of nodes.
//!
//! What a vehicle does, as `tcspecial-design.rst` specifies it. Four things and
//! no more: nodes, groups of nodes that share a sampling interval, links
//! between nodes, and bindings that say which commands from the ground may set
//! which attribute of which node.
//!
//! Three sections of nodes, and they are the three working panes of the
//! editor: `gateway` holds the ground links, `vehicle` holds what the vehicle
//! itself runs, and `payloads` holds what it carries. A node's type says which
//! section it belongs in -- see [`Section`] -- and a node written in the wrong
//! one is refused where the file is read, because a file and a drawing that
//! disagree about what a node is are two statements of one fact.
//!
//! A link is an attribute of the node it leaves, named after the data source
//! it leaves by, with one entry per line, each the node it reaches and the
//! name it arrives there by: `world-out: [k_1.reading]` is a link from this
//! node's `world-out` terminal to the input `k_1` lists as `reading`. A node
//! lists the names a line may arrive at it by under the sink each belongs
//! to -- `world-in: [reading]` -- so that a node's own entry says where it
//! may be joined, and a link to a name nobody listed is refused. See
//! [`mirror`], which matches the two.
//!
//! So there is no links section, and a node and the links out of it are read,
//! written and deleted together.
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
//! There is one configuration language, and a file does not say which
//! version of it it is written in: the software reading it is what says that,
//! in [`VERSION`]. A file has no `version` field, so one that writes it is
//! refused as it would be for any field a configuration does not have.
//!
//! Reading a file is not accepting it. [`Graph::check`] is what says whether a
//! configuration can be flown, and `tcs-verify` runs it on the ground against
//! the same code that runs again aboard.

pub mod built;
pub mod check;
pub mod exports;
pub mod fprime;
pub mod grouped;
pub mod interface;
pub mod link_types;
pub mod mirror;
pub mod own_xtce;
pub mod parameters;
pub mod rates;
pub mod reach;
pub mod renamed;
pub mod reshaped;
pub mod settings;
pub mod simulated;
pub mod sockets;
pub mod telemetry;
pub mod terminals;
pub mod units;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use crate::endpoint_config::Scalar;
use crate::error::{TcsError, TcsResult};
use crate::format::ConfigFormat;

/// The version of the configuration language this software reads.
///
/// Said by the software and never by a file. It goes nowhere on the link of
/// its own: the digest of a configuration is taken with it, so two builds
/// reading the language differently announce different digests, and the
/// digest is what a beacon and an answer to CONNECT carry. A file states
/// nothing about it.
pub const VERSION: &str = "3.0";

/// One file a configuration's top file includes: a subdiagram, by its name
/// and its path.
///
/// Written as a node is, the name as the entry and the path under it:
///
/// ```yaml
/// include:
///   - subdiagram: zone
///     path: ../thermostat-common/thermostat
/// ```
///
/// and in XML as an element with the two as attributes,
/// `<include name="zone" path="../thermostat-common/thermostat"/>`.
///
/// A name of its own because a subdiagram is drawn as a node of the top page
/// and joined like one, and a path is no name: it says where the file is,
/// which changes when the file moves, and it has slashes and dots in it,
/// which a name may not. The name is one no node of the configuration has.
/// An include written as a bare path, as they once were, is refused, saying
/// how to write it now.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(try_from = "IncludeWritten")]
pub struct Include {
    /// The subdiagram's name.
    pub name: String,
    /// The file's path from the including file's directory, without an
    /// extension: see [`include_path`].
    pub path: String,
}

/// An include as a file may write it: as it is written now, or as a bare
/// path, which is read only to be refused by name.
#[derive(Deserialize)]
#[serde(untagged)]
enum IncludeWritten {
    Named(IncludeNamed),
    Bare(String),
}

/// An include as it is written now: see [`Include`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IncludeNamed {
    #[serde(default, rename = "subdiagram", alias = "@name")]
    name: Option<String>,
    #[serde(default, alias = "@path")]
    path: Option<String>,
}

impl TryFrom<IncludeWritten> for Include {
    type Error = String;

    /// Takes an include as written, refusing one written as a bare path.
    ///
    /// # Arguments
    ///
    /// * `written` - The include as the file wrote it.
    ///
    /// # Returns
    ///
    /// The include.
    ///
    /// # Errors
    ///
    /// What to write instead, for a bare path.
    fn try_from(written: IncludeWritten) -> Result<Include, String> {
        match written {
            IncludeWritten::Named(IncludeNamed {
                name: Some(name),
                path: Some(path),
            }) => Ok(Include { name, path }),
            IncludeWritten::Named(IncludeNamed { name, path }) => {
                let what = match (name, path) {
                    (Some(name), None) => {
                        format!("the subdiagram \"{name}\" has no path")
                    }
                    (None, Some(path)) => {
                        format!("the include of \"{path}\" has no name")
                    }
                    _ => "an include has neither name nor path".to_string(),
                };
                Err(format!(
                    "{what}: an include names its subdiagram and gives its \
                     path"
                ))
            }
            IncludeWritten::Bare(path) => Err(format!(
                "the include \"{path}\" is a bare path: an include names its \
                 subdiagram and gives its path, `- subdiagram: <name>` with \
                 `path: {path}` under it, or in XML \
                 <include name=\"<name>\" path=\"{path}\"/>"
            )),
        }
    }
}

/// One file of a configuration, as it is written.
///
/// Every list defaults to empty, so a file that says only what it has is
/// legal: a subdiagram holding nodes and links need not mention groups, and a
/// top file that is nothing but an include list need not mention nodes.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GraphFile {
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
    /// Other files this one is made of, each a subdiagram with a name of its
    /// own and the file's path from this file's directory without an
    /// extension: `zone` at `../common/zone`. See [`Include`].
    ///
    /// Named rather than discovered. Each is read in the same format as the
    /// file that named it: a set of files half in YAML and half in XML is
    /// something nobody meant and everybody would have to remember.
    #[serde(default)]
    pub include: Vec<Include>,
    /// What of this file may be joined from outside it, where it is an
    /// included file: see [`exports`]. Refused in a top file, which has
    /// nothing outside it.
    #[serde(default)]
    pub exports: Option<exports::Exports>,
    /// What tcspecial itself is configured with: where commands are taken,
    /// where beacons go, where the telemetry log is written.
    ///
    /// Read by the struct the command interpreter's configuration is -- see
    /// [`crate::CIConfigJson`]. One statement of what the command interpreter
    /// needs, so that nothing can come to describe it differently.
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
    /// The ground links: what the editor's gateway working pane holds, by
    /// type. See [`grouped`] for how a section is written.
    #[serde(default, deserialize_with = "grouped::gateways")]
    pub gateways: Vec<grouped::Typed>,
    /// What the vehicle itself runs: computations, logs, pass-throughs,
    /// pages.
    #[serde(default, deserialize_with = "grouped::vehicle")]
    pub vehicle: Vec<grouped::Typed>,
    /// What the vehicle carries: a device on a bus, a line it drives, a link
    /// to something aboard.
    #[serde(default, deserialize_with = "grouped::payloads")]
    pub payloads: Vec<grouped::Typed>,
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
    #[serde(default, rename = "tcs-gui")]
    pub tcs_gui: Option<TcsGui>,
    /// The sections a configuration file used to have, kept to be refused.
    ///
    /// Declared so that a file written against the earlier shape is refused by
    /// name, saying where its contents have moved. Without them the complaint
    /// is serde's -- "unknown field `nodes`" -- which says what is wrong and
    /// not what to do about it, and a reader with a file full of them would
    /// have to be told three times.
    #[serde(default)]
    pub nodes: Option<Gone>,
    #[serde(default)]
    pub links: Option<Gone>,
    #[serde(default)]
    pub groups: Option<Gone>,
}

/// A section a file used to have, read only so that it can be refused by
/// name: whatever it holds is passed over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gone;

impl<'de> Deserialize<'de> for Gone {
    fn deserialize<D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<Self, D::Error> {
        serde::de::IgnoredAny::deserialize(d).map(|_| Gone)
    }
}

/// What the editor keeps about a configuration that a vehicle does not need.
///
/// Only the positions so far, which is what `tcs-gui` means: the editor's own
/// section of the file. Anything else it comes to keep about a drawing belongs
/// here too, so that the question "does the flight software read this" has one
/// answer for the whole section.
///
/// Written as the nodes are, each entry naming the node it is about: `- {
/// node: temp_1, x: 34, y: 34 }` in YAML, `<node name="temp_1" x="34"
/// y="34"/>` in XML. See [`grouped::layout`].
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct TcsGui {
    /// Where each node was drawn, by name.
    #[serde(default, deserialize_with = "grouped::layout")]
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
    ///
    /// Written as the positions are: `- { node: sensor, concern: ... }` in
    /// YAML, `<node name="sensor" concern="..."/>` in XML.
    #[serde(
        default,
        rename = "set-aside",
        deserialize_with = "grouped::set_aside"
    )]
    pub set_aside: Vec<Aside>,
}

/// One concern somebody has set aside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aside {
    /// The node it is about, as the nodes section named it.
    pub node: String,
    /// Which kind of concern: the editor's own word for it, so that a concern
    /// of another kind about the same node is still raised.
    pub concern: String,
}

/// One node's place on the canvas.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    Gateway,
    Vehicle,
    Payloads,
}

impl Section {
    /// Names the section as a file does.
    ///
    /// # Returns
    ///
    /// `gateways`, `vehicle` or `payloads`.
    pub fn name(self) -> &'static str {
        match self {
            Section::Gateway => "gateways",
            Section::Vehicle => "vehicle",
            Section::Payloads => "payloads",
        }
    }

    /// Names a node of the section as a file writes it: the key that names
    /// a node in YAML, and the element a node is in XML.
    ///
    /// # Returns
    ///
    /// `gateway`, `compute` -- every node of the vehicle's, whatever its
    /// type -- or `payload`.
    pub fn node_word(self) -> &'static str {
        match self {
            Section::Gateway => "gateway",
            Section::Vehicle => "compute",
            Section::Payloads => "payload",
        }
    }

    /// Every section, in the order a file is written in.
    pub const ALL: [Section; 3] =
        [Section::Gateway, Section::Vehicle, Section::Payloads];
}

/// Says which section a node of this type belongs in.
///
/// The one statement of the rule, read by the file reader and by the editor's
/// panes: a payload is something the vehicle carries, `gateway` is the ground
/// link, and everything else -- a computation, a log, a pass-through, a page of
/// its own -- is the vehicle's own. A type this build does not know is the
/// vehicle's: it is refused by name elsewhere, and the vehicle pane is the one
/// a reader looks at first.
///
/// # Arguments
///
/// * `node_type` - The node's type, as a file spells it.
///
/// # Returns
///
/// The section.
pub fn section_of(node_type: &str) -> Section {
    if Node::is_payload_type(node_type) {
        Section::Payloads
    } else if crate::gateway::is_gateway(node_type) {
        Section::Gateway
    } else {
        Section::Vehicle
    }
}

/// The vehicle's own node types that are not programs.
///
/// The app kinds are the rest of the vehicle's, and which of those are the
/// vehicle's and which are payloads is `crate::node::app`'s to say: see
/// [`types_in`], which puts the two together.
const VEHICLE_TYPES: &[&str] = &[
    "compute",
    "log",
    "null-gateway",
    "null-payload",
    "tee-gateway",
    "tee-payload",
    "ping",
];

/// Lists every node type a section may hold, as a file spells them.
///
/// The one statement of which types there are. Each section's list is made
/// from where its types are defined -- the interfaces from
/// [`crate::types::DHType`], the programs from [`crate::node::app::Kind`] --
/// so a type added there is a type here. The checking refuses a node whose
/// type is in none of them, and the schemas beside tcs-gui are tested against
/// them, so a file an editor accepts is one this reader knows.
///
/// # Arguments
///
/// * `section` - The section.
///
/// # Returns
///
/// The types, as a file spells them; for `gateway`, only the gateway kinds this
/// build has.
pub fn types_in(section: Section) -> Vec<&'static str> {
    use crate::node::app::Kind;
    let apps = |payload: bool| {
        Kind::ALL
            .into_iter()
            .filter(move |kind| kind.is_payload() == payload)
            .map(|kind| kind.node_type())
    };
    match section {
        Section::Gateway => crate::gateway::GatewayKind::ALL
            .into_iter()
            .filter(|kind| kind.is_built())
            .filter_map(|kind| kind.node_type())
            .collect(),
        Section::Vehicle => {
            VEHICLE_TYPES.iter().copied().chain(apps(false)).collect()
        }
        Section::Payloads => crate::types::DHType::ALL
            .iter()
            .map(|kind| kind.spelling())
            .chain(apps(true))
            .collect(),
    }
}

/// Whether there is a node type of this name.
pub fn is_node_type(node_type: &str) -> bool {
    [Section::Gateway, Section::Vehicle, Section::Payloads]
        .into_iter()
        .any(|section| types_in(section).contains(&node_type))
}

/// The attributes a node writes as a list: a YAML sequence, or one XML
/// element for each entry.
///
/// A node's attributes are one value each, so a listed one is held with its
/// entries joined by commas -- which is what the editor shows, and what
/// [`entries`] takes apart again for a writer. Its entries are runs of bytes,
/// whose commas are inside quotes where there are any, so the join is exact.
/// The one list: the reader and both of the editor's writers ask it.
pub const LISTED: &[&str] = &["terminators"];

/// Splits a listed attribute, as the model holds it, into its entries: see
/// [`LISTED`].
///
/// # Arguments
///
/// * `joined` - The attribute's value, its entries joined.
///
/// # Returns
///
/// Each entry, trimmed.
pub fn entries(joined: &str) -> Vec<&str> {
    crate::trigger::split_sequences(joined)
        .into_iter()
        .map(str::trim)
        .collect()
}

/// Says what to tell a file that wrote an attribute with hyphens where it has
/// underscores, or the other way round, if that is what it did.
///
/// An attribute has the one spelling. Two would be twice the words for the
/// editor, the XML and the YAML to agree about, and a file that said both
/// would say one thing twice with nothing to choose between them. So
/// `max-length` is refused like any word nothing reads -- but a reader who
/// wrote it meant something real, and is told its spelling rather than left
/// to find it.
///
/// The one statement of the hint, for every node type's refusal of a word it
/// does not know.
///
/// # Arguments
///
/// * `said`  - The attribute as the file wrote it.
/// * `known` - Whether a name is an attribute where `said` was written.
///
/// # Returns
///
/// The hint, such as `it is spelled max_length`, or `None` when the other
/// spelling is not an attribute either.
pub fn spelled(said: &str, known: impl Fn(&str) -> bool) -> Option<String> {
    [said.replace('-', "_"), said.replace('_', "-")]
        .into_iter()
        .find(|other| other != said && known(other))
        .map(|other| format!("it is spelled {other}"))
}

/// Whether the ground may start and stop a node of this type.
///
/// Every type but `ping`. A ping node is how the ground learns that the link
/// and the vehicle are alive, and a check that can be switched off is one
/// that can be silenced by mistake -- after which a vehicle that is well and
/// a vehicle that is not look the same from the ground. So START_NODE and
/// STOP_NODE about one are answered with a bad status and change nothing.
///
/// The one statement of the rule: the vehicle reads it to answer the
/// commands, and a ground station may read it to know not to send them.
pub fn starts_and_stops(node_type: &str) -> bool {
    node_type != "ping"
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
    /// Where this node's groundward data goes: each line its `world-out`
    /// sends, as the node it reaches and the name it arrives by,
    /// `k_1.reading`.
    pub world_out: Vec<String>,
    /// And its vehicleward data: each line its `moc-out` sends.
    pub moc_out: Vec<String>,
    /// The names a groundward line may arrive at this node by: its
    /// `world-in`.
    ///
    /// Where the node may be joined, said on the node, so that a link is
    /// written to a name the node at its other end gave and not to whatever
    /// the writer guessed. A vote's are its inputs. See [`mirror`].
    pub world_in: Vec<String>,
    /// And the names a vehicleward line may arrive by: its `moc-in`.
    pub moc_in: Vec<String>,
}

impl NodeFile {
    /// Makes the links this node's sources make.
    ///
    /// One per destination, each `from` naming the node and the data source it
    /// leaves by -- `k_1.world-out` -- which is what every rule about a link
    /// already reads: see [`Link::origin`].
    ///
    /// # Returns
    ///
    /// One link for each data source with destinations, `world-out` first.
    pub fn links(&self) -> Vec<Link> {
        let mut links = Vec::new();
        for (terminal, destinations) in
            [("world-out", &self.world_out), ("moc-out", &self.moc_out)]
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

    /// The names this entry gave each of its sinks.
    ///
    /// The other end of [`NodeFile::links`], and deliberately not links:
    /// where a node may be joined, which [`mirror`] matches each link to.
    ///
    /// # Returns
    ///
    /// Each sink's terminal and the names listed under it.
    pub fn arriving(&self) -> [(&'static str, &[String]); 2] {
        [("world-in", &self.world_in), ("moc-in", &self.moc_in)]
    }
}

/// A set of nodes read on one interval.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub name: String,
    /// How often the group is read: a whole number with a unit, `500 ms` or
    /// `3 s`. The same grammar a timeout uses, so that a reader who has
    /// learned one has learned the other.
    pub interval: String,
}

impl Group {
    /// The interval as a duration, or what is wrong with it.
    pub fn interval(&self) -> Result<Duration, String> {
        parse_interval(&self.interval)
    }
}

/// One node: a device, a gateway link, a compute element or a sink.
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
    /// either format actually writes. Scalar is the answer to exactly that,
    /// and a value as a file spelled it is a question about formats: it should
    /// move to `format`.
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
        crate::types::DHType::from_spelling(node_type).is_some()
            || crate::node::app::Kind::of(node_type)
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
    /// Written the way a destination is -- `temp_1.world-out` -- because the
    /// two ends of a link are the same kind of thing: a node and one of its
    /// terminals. A node with one source needs no name after the dot, so every
    /// file written before there were names for these still says what it said.
    pub fn origin(&self) -> Destination {
        Destination::parse(&self.from)
    }
}

/// One or several destinations, however the file wrote them.
///
/// Each the node a line reaches and the name it arrives by, separated by a
/// dot, one per line:
///
/// ```yaml
/// world-out:
///   - gateway.readings
///   - zone_temp.in0
/// ```
///
/// One entry per line, so several destinations are several entries and
/// nothing pairs by position. In XML the same thing is a repeated
/// `<world-out>` with the destination as its text. One destination needs no
/// list: `world-out: gateway.readings`.
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
    fn deserialize<D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<Self, D::Error> {
        use serde::de::{self, MapAccess, SeqAccess, Visitor};
        use std::fmt;

        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = Destinations;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a destination, or a list of them")
            }

            fn visit_str<E: de::Error>(
                self,
                v: &str,
            ) -> Result<Destinations, E> {
                Ok(Destinations(vec![v.to_string()]))
            }

            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Destinations, A::Error> {
                let mut all = Vec::new();
                // Each element is an entry, a string, or an XML element with
                // text in it: see Bound.
                while let Some(one) = seq.next_element::<Bound>()? {
                    all.push(one.0);
                }
                Ok(Destinations(all))
            }

            /// One entry -- `{ target: gateway }` -- or one XML element with
            /// text in it: `<world-out>gateway</world-out>`.
            fn visit_map<A: MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<Destinations, A::Error> {
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
    fn deserialize<D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<Self, D::Error> {
        use serde::de::{MapAccess, Visitor};
        use std::fmt;

        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = Bound;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a destination, or an entry naming its target")
            }

            fn visit_str<E: serde::de::Error>(
                self,
                v: &str,
            ) -> Result<Bound, E> {
                Ok(Bound(v.to_string()))
            }

            fn visit_map<A: MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<Bound, A::Error> {
                Ok(Bound(written_as(map)?))
            }
        }

        d.deserialize_any(V)
    }
}

/// Values written as a list: a YAML sequence, or one value, which is one
/// element of a list an XML file writes by repeating the element.
///
/// Each element is read as a [`Scalar`], for the reason that type exists.
struct Listed(Vec<Scalar>);

impl<'de> Deserialize<'de> for Listed {
    fn deserialize<D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<Self, D::Error> {
        use serde::de::value::MapAccessDeserializer;
        use serde::de::{MapAccess, SeqAccess, Visitor};
        use std::fmt;

        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = Listed;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a value, or a list of them")
            }

            fn visit_str<E: serde::de::Error>(
                self,
                v: &str,
            ) -> Result<Listed, E> {
                Ok(Listed(vec![Scalar::of(v)]))
            }

            fn visit_u64<E: serde::de::Error>(
                self,
                v: u64,
            ) -> Result<Listed, E> {
                Ok(Listed(vec![Scalar::of(v.to_string())]))
            }

            fn visit_i64<E: serde::de::Error>(
                self,
                v: i64,
            ) -> Result<Listed, E> {
                Ok(Listed(vec![Scalar::of(v.to_string())]))
            }

            /// A YAML boolean, kept as its word so that whoever asked for
            /// the list can say why it is not one.
            fn visit_bool<E: serde::de::Error>(
                self,
                v: bool,
            ) -> Result<Listed, E> {
                Ok(Listed(vec![Scalar::of(v.to_string())]))
            }

            /// Nothing after the key, which says an empty list.
            fn visit_unit<E: serde::de::Error>(self) -> Result<Listed, E> {
                Ok(Listed(Vec::new()))
            }

            fn visit_none<E: serde::de::Error>(self) -> Result<Listed, E> {
                Ok(Listed(Vec::new()))
            }

            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Listed, A::Error> {
                let mut all = Vec::new();
                while let Some(one) = seq.next_element::<Scalar>()? {
                    all.push(one);
                }
                Ok(Listed(all))
            }

            /// An XML element with text in it, read as [`Scalar`] reads one.
            fn visit_map<A: MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<Listed, A::Error> {
                Ok(Listed(vec![Scalar::deserialize(
                    MapAccessDeserializer::new(map),
                )?]))
            }
        }

        d.deserialize_any(V)
    }
}

/// Reads the names a node's entry lists under one of its sinks.
///
/// `world-in: [in0, in1]` in YAML, and a repeated `<world-in>` in XML, each
/// name an entry of its own; one name needs no list. A name is a word: no dot,
/// which is what parts it from its node in a destination, and no comma or
/// space, which would make one name read as several.
///
/// `true` and `false` are refused, saying what the field holds now. A sink
/// said whether a line arrived there once, and a file written then would
/// otherwise list a name nobody meant.
///
/// # Arguments
///
/// * `sink` - The sink's name, for the message.
/// * `map`  - The entry being read, its next value the sink's.
///
/// # Returns
///
/// The names, in the order written.
///
/// # Errors
///
/// What the map's next value fails with, and a custom error for a flag, an
/// empty name, and a name with a dot, a comma or a space in it.
fn names_of<'de, A: serde::de::MapAccess<'de>>(
    sink: &str,
    map: &mut A,
) -> Result<Vec<String>, A::Error> {
    use serde::de::Error;

    let listed = map.next_value::<Listed>()?;
    let mut names = Vec::new();
    for one in &listed.0 {
        let name = one.as_str().trim();
        if name == "true" || name == "false" {
            return Err(A::Error::custom(format!(
                "{sink}: \"{name}\" is a name for where a line may arrive \
                 here, not whether one does: list the names, as \
                 `{sink}: [readings]`, and write each link to one of them, \
                 as `<node>.readings`"
            )));
        }
        if name.is_empty() || name.contains(['.', ',', ' ']) {
            return Err(A::Error::custom(format!(
                "{sink}: \"{name}\" is not a name. A name is one word, with \
                 no dot, comma or space in it: a link is written to it as \
                 `<node>.<name>`, and the dot is what parts the two"
            )));
        }
        names.push(name.to_string());
    }
    Ok(names)
}

/// Reads the destination a mapping names, however the mapping got there.
///
/// An XML element with text in it, `<world-out>gateway.readings</world-out>`,
/// which quick-xml hands over as a map of one entry whose value is the text:
/// the destination, in the language where it does not arrive as a string.
///
/// Anything else is refused, saying how a destination is written. An entry
/// naming a `target`, and an `input` beside it, is how a destination was once
/// written, and is told the new shape rather than read: a file that says
/// `target: gateway` names no name to arrive by.
///
/// # Arguments
///
/// * `map` - The mapping.
///
/// # Returns
///
/// The destination as written, `node.name`.
///
/// # Errors
///
/// What the map fails with, and a custom error for a mapping with anything
/// but text in it.
fn written_as<'de, A: serde::de::MapAccess<'de>>(
    mut map: A,
) -> Result<String, A::Error> {
    use serde::de::Error;

    let mut text: Option<String> = None;
    let mut said: Option<String> = None;

    while let Some(key) = map.next_key::<String>()? {
        match key.as_str() {
            "$text" => {
                text = Some(map.next_value::<Scalar>()?.as_str().to_string())
            }
            other => {
                said.get_or_insert_with(|| other.to_string());
                map.next_value::<serde::de::IgnoredAny>()?;
            }
        }
    }

    match (text, said) {
        (Some(text), None) => Ok(text),
        (_, Some(said)) => Err(A::Error::custom(format!(
            "a link is written as the node it reaches and the name it arrives \
             there by, separated by a dot -- `zone_temp.in0` -- and not as an \
             entry saying `{said}`: write the entry as that one line"
        ))),
        (None, None) => Err(A::Error::custom(
            "a destination is the node a line reaches and the name it \
             arrives by, `node.name`",
        )),
    }
}

/// Which node and which input a destination names.
///
/// `zone_temp.in0` is the `in0` input of `zone_temp`; `gateway` is a node
/// that takes one input and does not name it. The separator is a dot because a
/// node name cannot contain one and an input name cannot either, which is
/// checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destination {
    pub node: String,
    /// The terminal named after the dot, where one was: a data sink at the end
    /// a link arrives at, a data source at the end it leaves. Named `input`
    /// from when the only terminals were a computation's inputs.
    pub input: Option<String>,
}

impl Destination {
    /// Reads a destination as a file wrote it.
    ///
    /// Total: what is wrong with a destination is the checking's to say.
    ///
    /// # Arguments
    ///
    /// * `text` - The destination: `node` or `node.input`.
    ///
    /// # Returns
    ///
    /// The node and the input named after the dot, where there is one with
    /// something either side of it; otherwise the whole text as the node.
    pub fn parse(text: &str) -> Destination {
        match text.split_once('.') {
            Some((node, input)) if !node.is_empty() && !input.is_empty() => {
                Destination {
                    node: node.to_string(),
                    input: Some(input.to_string()),
                }
            }
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
    /// The gateway node the command arrives on.
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
    /// Where each node was drawn, by node name: the `tcs-gui` sections of the
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
    /// and the one reader is tcs-gui. See [`Tcs-gui::set_aside`].
    pub set_aside: std::collections::BTreeSet<(String, String)>,
    /// What the files call the arriving ends of their links: each node's
    /// names, and the name each link was written to.
    ///
    /// Apart from [`Graph::links`], which say what each link means, because
    /// nothing aboard needs the names: they say where a node may be joined,
    /// which the file is held to as it is read -- see [`mirror`] -- and which
    /// the editor writes back.
    pub named: Named,
    /// What each included file exports, by the name the include list gives
    /// it -- which is the same whichever spelling the files are read in --
    /// see [`exports`]. A file with no `exports` section exports nothing,
    /// and has no entry.
    pub exports: BTreeMap<String, exports::Exports>,
    /// The top file's includes, as it wrote them and in its order: the
    /// files after the first in [`Graph::files`], one for one.
    pub includes: Vec<Include>,
}

/// The names the files give the arriving ends of their links.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Named {
    /// By node, the names listed under its `world-in` and its `moc-in`.
    pub sinks: BTreeMap<String, Sinks>,
    /// The destination each link was written to, `node.name`: by the
    /// link's `from` and the destination in [`Graph::links`] it means.
    pub written: BTreeMap<(String, String), String>,
}

/// One node's names for where a line may arrive at it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sinks {
    /// Its `world-in`: the names a groundward line may arrive by.
    pub world_in: Vec<String>,
    /// Its `moc-in`: the names a vehicleward line may arrive by.
    pub moc_in: Vec<String>,
}

impl Graph {
    /// Reads a configuration, following its includes.
    ///
    /// The top file first, then each file it names, each in the top file's
    /// format and at its include name read from the top file's directory --
    /// see [`include_path`]. An include that names a file twice, or names the
    /// top file, is refused rather than read twice.
    ///
    /// # Arguments
    ///
    /// * `path` - The top file.
    ///
    /// # Returns
    ///
    /// The configuration, every file's nodes, links, groups and commands in
    /// one.
    ///
    /// # Errors
    ///
    /// [`TcsError::Config`], naming the file, for a file that does not read,
    /// an include name of a shape [`include_path`] refuses, an include named
    /// twice, an included file with a `tcspecial` section or
    /// includes of its own, a node in the wrong section, and a link to a
    /// name its far end does not list.
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
            named: Named::default(),
            exports: BTreeMap::new(),
            includes: Vec::new(),
        };

        // What the files record at the arriving ends of their links, gathered
        // as they are read and checked once they all have been: a link may
        // cross from an included file into the top one, so the two halves of
        // one record are not necessarily in the same file. See `mirror`.
        let mut mirror = mirror::Mirror::default();

        let top: GraphFile = read_one(path)?;
        if top.exports.is_some() {
            return Err(TcsError::Config(format!(
                "{}: only an included file may have an exports section -- \
                 it says what of that file the files around it may join, \
                 and this file has none around it. Take the section out, or \
                 move the nodes it names into a file this one includes",
                path.display()
            )));
        }
        graph.description = top.description.clone();
        graph.tcspecial = top.tcspecial.clone();
        graph.absorb(path, &top, &mut mirror)?;
        graph.files.push(path.to_path_buf());

        // Each file once, by what it is rather than by how it was named:
        // `zone` and `../vehicle/zone` from a top file in `vehicle` are one
        // file, and so is a name that comes back to the top file itself.
        let mut seen = vec![canonical(path)?];
        for include in &top.include {
            let name = &include.name;
            include_name_ok(path, include, &top.include)?;
            let included = include_path(path, &include.path)?;
            let file: GraphFile = read_one(&included)?;
            let real = canonical(&included)?;
            if seen.contains(&real) {
                return Err(TcsError::Config(format!(
                    "{}: {} is included twice, or is this file",
                    path.display(),
                    included.display()
                )));
            }
            seen.push(real);
            if file.tcspecial.is_some() {
                return Err(TcsError::Config(format!(
                    "{}: an included file may not have a tcspecial section -- \
                     a configuration has one command interpreter, and the file \
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
            if let Some(exports) = &file.exports {
                graph.exports.insert(name.clone(), exports.clone());
            }
            graph.files.push(included);
            graph.includes.push(include.clone());
        }

        // A subdiagram is drawn as a node of the top page and joined like
        // one, so its name is no node's.
        for include in &graph.includes {
            if graph.homes.contains_key(&include.name) {
                return Err(TcsError::Config(format!(
                    "{}: the subdiagram \"{}\" has the name of a node; a \
                     subdiagram is drawn and joined as a node is, so its name \
                     has to be its own",
                    path.display(),
                    include.name
                )));
            }
        }

        // In the one order there is: by name. Nothing a file writes is in an
        // order that means anything -- not its types, not its nodes, not the
        // files it includes -- so a node's device id, its place in this
        // list, is its place among the names (see `telemetry::ids`), and the
        // links are held by the node they leave and then where they go.
        // Each link matched to a name its far end listed, and turned into
        // what it means, before anything is ordered by what a link means.
        mirror.resolve(&mut graph)?;

        graph.nodes.sort_by(|a, b| a.name.cmp(&b.name));
        for link in &mut graph.links {
            link.to.0.sort();
        }
        graph.links.sort_by(|a, b| {
            (&a.from, a.to.each()).cmp(&(&b.from, b.to.each()))
        });

        Ok(graph)
    }

    /// Says the top file's includes: each subdiagram's name, and its file's
    /// path from the top file's directory without the extension
    /// [`Graph::load`] gave it, as the top file wrote them. The same set of
    /// files is named the same from wherever it is read, which is what lets
    /// the digest take these in.
    ///
    /// # Returns
    ///
    /// The includes, in the order the top file gave them; none for a
    /// configuration of one file, or one not read from files.
    pub fn included(&self) -> Vec<Include> {
        self.includes.clone()
    }

    /// Says the name of the subdiagram one of the other files is, as the
    /// top file's include list gives it.
    ///
    /// # Arguments
    ///
    /// * `file` - One of [`Graph::files`].
    ///
    /// # Returns
    ///
    /// The subdiagram's name, or `None` for the top file and for a file that
    /// is not one of this configuration's.
    pub fn include_name(&self, file: &Path) -> Option<String> {
        let at = self.files.iter().position(|f| f == file)?;
        let include = self.includes.get(at.checked_sub(1)?)?;
        Some(include.name.clone())
    }

    /// Takes in one file: its nodes, the links they carry, and the rest.
    ///
    /// The three sections are one list of nodes once they are read, because
    /// every rule about a node is about nodes and not about sections; the
    /// sections are for a reader of the file, who can see at a glance what
    /// the vehicle carries this flight. A node cannot be in the wrong one:
    /// its type is the group it is written under, and the reader refuses a
    /// group of another section's type.
    ///
    /// # Arguments
    ///
    /// * `from`   - The file's path, for the complaints.
    /// * `file`   - The file as read.
    /// * `mirror` - Where what its data sinks say is recorded, to be checked
    ///              once every file is in.
    ///
    /// # Returns
    ///
    /// `Ok(())` once the file is taken in.
    ///
    /// # Errors
    ///
    /// [`TcsError::Config`] for a node named as a node already taken in is,
    /// in this file or another.
    fn absorb(
        &mut self,
        from: &Path,
        file: &GraphFile,
        mirror: &mut mirror::Mirror,
    ) -> TcsResult<()> {
        for typed in [&file.gateways, &file.vehicle, &file.payloads]
            .into_iter()
            .flatten()
        {
            for said in &typed.nodes {
                let name = &said.node.name;
                if let Some(home) = self.homes.get(name) {
                    let (where_, nth) = if home == from {
                        ("twice in this file".to_string(), 1)
                    } else {
                        (format!("here and in {}", home.display()), 0)
                    };
                    let at = grouped::line_of_node(from, name, nth)
                        .map(|line| format!("at line {line}, "))
                        .unwrap_or_default();
                    return Err(TcsError::Config(format!(
                        "{}: {at}there is a node called \"{name}\" \
                         {where_}: a node's name is the one thing every other \
                         part of a configuration knows it by, so no two nodes \
                         share one",
                        from.display(),
                    )));
                }
                self.homes.insert(name.clone(), from.to_path_buf());
                self.nodes.push(said.node.clone());
                self.links.extend(said.links());
                mirror.record(from, said);
            }
        }

        self.groups.extend(file.payload_groups.iter().cloned());
        self.commands.extend(file.commands.iter().cloned());

        // What the editor arranged, by node name. A node named here and
        // nowhere else is a position left behind by a node somebody deleted:
        // kept rather than refused, because it says nothing about the vehicle
        // and the editor writes the section out again from the nodes it has.
        if let Some(said) = &file.tcs_gui {
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

/// Reads one file of a configuration, without following its includes.
///
/// [`Graph::load`] is what reads a configuration; this is for a reader that
/// needs to know which of its files says what, as the editor does to write
/// each group and command binding back to the file it came from.
///
/// The sections of an older spelling are named before the file is read as a
/// graph: a file still written in them is not a graph with problems in it,
/// and every complaint the graph reader made about it would be noise.
///
/// # Arguments
///
/// * `path` - The file.
///
/// # Returns
///
/// The file as read.
///
/// # Errors
///
/// [`TcsError::Config`], naming the file, for an extension that is no
/// format, a file that cannot be read or parsed, and a section of an older
/// spelling.
pub fn read_one(path: &Path) -> TcsResult<GraphFile> {
    let format = ConfigFormat::of_file(path)?;
    let text = std::fs::read_to_string(path)
        .map_err(|e| TcsError::Config(format!("{}: {e}", path.display())))?;

    if let Some(TcsError::Config(gone)) =
        crate::config::names_what_is_gone(&text, format)
    {
        return Err(TcsError::Config(format!("{}: {gone}", path.display())));
    }

    // A file whose nodes are written in the shape they had before they were
    // grouped by type, refused by its line and shown the shape now.
    if let Some(why) = reshaped::refused(&text, format) {
        return Err(TcsError::Config(format!("{}: {why}", path.display())));
    }

    // A word that was renamed, refused by its line and told the word it is
    // now, before serde can call it a field nobody has heard of.
    if let Some(why) = renamed::refused(&text, format) {
        return Err(TcsError::Config(format!("{}: {why}", path.display())));
    }

    let file: GraphFile = format
        .parse(&text)
        .map_err(|e| TcsError::Config(format!("{}: {e}", path.display())))?;

    // The sections a configuration file used to have, refused by name and told
    // where their contents have gone. A file full of them would otherwise be
    // read as far as the first one and complained about in serde's words --
    // "unknown field `nodes`" -- which says what is wrong and not what to do.
    let moved: &[(&str, bool, &str)] = &[
        (
            "nodes",
            file.nodes.is_some(),
            "a node is written under gateway, vehicle or payloads, according \
             to what it is",
        ),
        (
            "links",
            file.links.is_some(),
            "a link is an attribute of the node it leaves, named after the \
             data source it leaves by: `world_out: k_1`",
        ),
        (
            "groups",
            file.groups.is_some(),
            "groups are now payload_groups",
        ),
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

/// Holds a subdiagram's name to what a name may be: something, with no dot
/// or slash in it, and no other include's.
///
/// A dot because a line reads what follows one as an input's name, and a
/// slash because the editor reads what precedes one as a page's: a name with
/// either in it would be two names to whatever reads a line to it.
///
/// # Arguments
///
/// * `top`      - The file that includes it, for the complaint.
/// * `include`  - The include.
/// * `includes` - Every include of that file.
///
/// # Errors
///
/// [`TcsError::Config`], naming the file and the subdiagram.
fn include_name_ok(
    top: &Path,
    include: &Include,
    includes: &[Include],
) -> TcsResult<()> {
    let name = &include.name;
    let refused = |why: &str| {
        Err(TcsError::Config(format!(
            "{}: the subdiagram \"{name}\" {why}",
            top.display()
        )))
    };
    if name.trim().is_empty() {
        return refused("has no name");
    }
    if name.contains('.') || name.contains('/') {
        return refused(
            "has a dot or a slash in its name, which a line reads as \
             separating a node's name from an input's or a page's",
        );
    }
    if includes.iter().filter(|other| &other.name == name).count() > 1 {
        return refused(
            "is named twice; each subdiagram has a name of its own",
        );
    }
    Ok(())
}

/// Says where an included file is: its include name, read from the
/// directory of the file that named it, in that file's format.
///
/// A name is a relative path without an extension, its parts separated by
/// `/`: `zone` beside the top file, `../thermostat-common/thermostat` in a
/// directory beside the top file's. Each part is a name or `..`. Refused are
/// an empty name, an absolute one, and a part that is empty or `.`: those
/// last say nothing a name without them does not, and a name is written into
/// the digest as it is written in the file, so two spellings of one path
/// would be two digests of one configuration. An absolute name would make
/// the configuration mean something different on every machine it is
/// copied to, and so would a name that began with `~` or named an
/// environment variable with `$`, were either expanded. Neither is: both
/// are refused rather than read as a file's name, because somebody who
/// wrote one meant the expansion, and a digest that depended on a home
/// directory or a variable could not be worked out from the files alone.
///
/// # Arguments
///
/// * `top`  - The file that named it.
/// * `name` - The name it was included by, without an extension.
///
/// # Returns
///
/// The included file's path: the top file's directory joined with the name,
/// the top file's extension added.
///
/// # Errors
///
/// [`TcsError::Config`], naming the top file and the include, for a name of
/// a shape refused above.
fn include_path(top: &Path, name: &str) -> TcsResult<PathBuf> {
    let refused = |why: &str| {
        Err(TcsError::Config(format!(
            "{}: the include \"{name}\" {why}: an include is relative to \
             this file's directory, naming a file by its path from there \
             without an extension, such as zone or ../common/zone",
            top.display()
        )))
    };
    if name.is_empty() {
        return refused("is empty");
    }
    if name.starts_with('/') {
        return refused("is an absolute path");
    }
    if name.split('/').any(|part| part.is_empty() || part == ".") {
        return refused("has a part that is empty or \".\"");
    }
    if name.starts_with('~') {
        return refused("begins with ~, which is not expanded");
    }
    if name.contains('$') {
        return refused("has a $ in it, and no variable is expanded");
    }
    let extension = top
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("ytcs")
        .to_string();
    let dir = top.parent().unwrap_or(Path::new(""));
    Ok(dir.join(format!("{name}.{extension}")))
}

/// Says what a file is, whatever path it was reached by.
///
/// # Arguments
///
/// * `path` - The file.
///
/// # Returns
///
/// Its canonical path, every link and `..` resolved.
///
/// # Errors
///
/// [`TcsError::Config`], naming the file, for one that cannot be found.
fn canonical(path: &Path) -> TcsResult<PathBuf> {
    std::fs::canonicalize(path)
        .map_err(|e| TcsError::Config(format!("{}: {e}", path.display())))
}

/// Reads a length of time: a number, a space, and a unit.
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
///
/// # Arguments
///
/// * `text` - The length of time as the file wrote it.
///
/// # Returns
///
/// The length of time, to the nearest microsecond.
///
/// # Errors
///
/// A message quoting `text` for anything the grammar above does not take,
/// naming what to write instead where that is known, and for nought or a
/// length too long to hold.
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
        (
            "mon",
            "a month is not a length of time: say 30 d, or a number of days",
        ),
        (
            "mo",
            "a month is not a length of time: say 30 d, or a number of days",
        ),
        (
            "month",
            "a month is not a length of time: say 30 d, or a number of days",
        ),
        (
            "y",
            "a year is not a length of time: say 365 d, or a number of days",
        ),
        (
            "yr",
            "a year is not a length of time: say 365 d, or a number of days",
        ),
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
                "\"{text}\": \"{unit}\" is not a unit of time: us, ms, s, min, \
                 h, d or w"
            )
        })?;

    let total = whole_micros(text, digits, unit, micros)?;
    let interval = Duration::from_micros(total);
    if interval.is_zero() {
        return Err(format!("\"{text}\" is no interval at all"));
    }
    Ok(interval)
}

/// Says what to tell a value whose number and unit are written together.
///
/// The commonest thing a file will have wrong while the space is new, and the
/// one worth naming exactly: a message about a unit that is not a unit would
/// send somebody looking at the unit, which is not what is wrong.
///
/// # Arguments
///
/// * `text` - The value as the file wrote it.
/// * `said` - The value, trimmed.
///
/// # Returns
///
/// The message: the value with the space put in, or what a length of time
/// is where it cannot be split into a number and a unit.
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
    format!(
        "\"{text}\": write the number and the unit with a space between them, \
         as \"{number} {unit}\""
    )
}

/// Reads `digits` of `unit` as whole microseconds.
///
/// A whole number or a decimal fraction, underscores ignored, read exactly and
/// rounded to the nearest microsecond. In microseconds throughout, so that a
/// week and a microsecond are the same arithmetic -- and checked, because
/// seven hundred thousand weeks overflows the count rather than being an
/// interval anybody meant.
///
/// # Arguments
///
/// * `text`   - The value as the file wrote it, for the messages.
/// * `digits` - The number.
/// * `unit`   - The unit's name, for the messages.
/// * `micros` - How many microseconds one of the unit is.
///
/// # Returns
///
/// The length of time in microseconds, rounded to the nearest.
///
/// # Errors
///
/// A message for a number that is not digits with at most one point, and
/// for a length too long to keep track of.
fn whole_micros(
    text: &str,
    digits: &str,
    unit: &str,
    micros: f64,
) -> Result<u64, String> {
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

    /// Reads one node, written in YAML under a type of the section it
    /// belongs in.
    ///
    /// # Arguments
    ///
    /// * `node_type` - The type it is written under.
    /// * `body`      - The node's mapping, its first key naming it, as lines
    ///                 not yet indented.
    ///
    /// # Returns
    ///
    /// The node as read.
    ///
    /// # Errors
    ///
    /// What the reader says is wrong with it.
    fn yaml_node(node_type: &str, body: &str) -> Result<NodeFile, String> {
        let section = section_of(node_type);
        let indented = body.trim_end().replace('\n', "\n        ");
        let text = format!(
            "{}:\n  - type {node_type}:\n      - {indented}\n",
            section.name()
        );
        let file: GraphFile =
            ConfigFormat::Yaml.parse(&text).map_err(|e| e.to_string())?;
        first_node(file)
    }

    /// Reads one node, written in XML under a type of the section it belongs
    /// in.
    ///
    /// # Arguments
    ///
    /// * `node_type` - The type it is written under.
    /// * `element`   - The node's element.
    ///
    /// # Returns
    ///
    /// The node as read.
    ///
    /// # Errors
    ///
    /// What the reader says is wrong with it.
    fn xml_node(node_type: &str, element: &str) -> Result<NodeFile, String> {
        let section = section_of(node_type).name();
        let text = format!(
            "<configuration><{section}><type name=\"{node_type}\">\
             {element}</type></{section}></configuration>"
        );
        let file: GraphFile =
            ConfigFormat::Xml.parse(&text).map_err(|e| e.to_string())?;
        first_node(file)
    }

    /// The one node a file read for a test holds.
    ///
    /// # Arguments
    ///
    /// * `file` - The file.
    ///
    /// # Returns
    ///
    /// Its first node.
    ///
    /// # Errors
    ///
    /// A message for a file with no node.
    fn first_node(file: GraphFile) -> Result<NodeFile, String> {
        [file.gateways, file.vehicle, file.payloads]
            .into_iter()
            .flatten()
            .flat_map(|typed| typed.nodes)
            .next()
            .ok_or_else(|| "no node".to_string())
    }

    /// The hint is the other spelling, either way round, and only when that
    /// spelling is a word where it was written.
    #[test]
    fn spelled_offers_the_other_spelling_only_where_it_is_real() {
        let known =
            |name: &str| ["max_length", "app-gateway-arg"].contains(&name);
        assert_eq!(
            spelled("max-length", known).as_deref(),
            Some("it is spelled max_length")
        );
        assert_eq!(
            spelled("app_gateway_arg", known).as_deref(),
            Some("it is spelled app-gateway-arg")
        );
        assert_eq!(spelled("max-lenght", known), None, "a misspelling");
        assert_eq!(spelled("max_length", known), None, "itself is no hint");
        assert_eq!(spelled("datarate", known), None, "nothing to swap");
    }

    /// Each type is in one section, the one [`section_of`] says, and the
    /// payloads' list is [`Node::is_payload_type`]'s.
    #[test]
    fn each_type_is_in_the_section_it_belongs_in() {
        let mut seen = std::collections::BTreeSet::new();
        for section in [Section::Gateway, Section::Vehicle, Section::Payloads] {
            for node_type in types_in(section) {
                assert!(seen.insert(node_type), "{node_type} is listed twice");
                assert_eq!(section_of(node_type), section, "{node_type}");
                assert_eq!(
                    Node::is_payload_type(node_type),
                    section == Section::Payloads,
                    "{node_type}"
                );
                assert!(is_node_type(node_type));
            }
        }
        assert!(seen.contains("ping") && seen.contains("app_pl"));
        assert!(!is_node_type("bogus") && !is_node_type(""));
    }

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
            let yaml = Graph::load(root.join(format!("{name}.ytcs")))
                .unwrap_or_else(|e| panic!("{name}.ytcs: {e}"));
            let xml = Graph::load(root.join(format!("{name}.xtcs")))
                .unwrap_or_else(|e| panic!("{name}.xtcs: {e}"));

            assert_eq!(
                yaml.description, xml.description,
                "{name}: description"
            );
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
        assert_eq!(
            parse_interval("500 ms").unwrap(),
            Duration::from_millis(500)
        );
        assert_eq!(
            parse_interval("250 us").unwrap(),
            Duration::from_micros(250)
        );
        assert_eq!(parse_interval(" 3 s ").unwrap(), Duration::from_secs(3));

        // The seven units, the long ones in the terms somebody would check
        // them in: a minute of seconds, an hour of minutes, a day of hours
        // and a week of days.
        assert_eq!(parse_interval("1 min").unwrap(), Duration::from_secs(60));
        assert_eq!(parse_interval("15 min").unwrap(), Duration::from_secs(900));
        assert_eq!(parse_interval("1 h").unwrap(), Duration::from_secs(3_600));
        assert_eq!(parse_interval("6 h").unwrap(), Duration::from_secs(21_600));
        assert_eq!(parse_interval("1 d").unwrap(), Duration::from_secs(86_400));
        assert_eq!(
            parse_interval("2 w").unwrap(),
            Duration::from_secs(14 * 86_400)
        );
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
        assert_eq!(
            parse_interval("1.5 s").unwrap(),
            Duration::from_millis(1_500)
        );
        assert_eq!(
            parse_interval("0.5 d").unwrap(),
            Duration::from_secs(12 * 3_600)
        );
        assert_eq!(
            parse_interval("1.5 ms").unwrap(),
            Duration::from_micros(1_500)
        );
        assert_eq!(
            parse_interval("1.0000004 s").unwrap(),
            Duration::from_micros(1_000_000),
            "a fraction of a microsecond is rounded to the nearest"
        );
        assert_eq!(parse_interval("1.6 us").unwrap(), Duration::from_micros(2));

        // Underscores group the digits and mean nothing, as everywhere else
        // in these files.
        for said in ["1000 ms", "1_000 ms", "1_0_0_0 ms", "1.000 s", "1.0_00 s"]
        {
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
            "3",
            "fast",
            "",
            "s",
            "3 seconds",
            "-1 s",
            "0b11 s",
            "0x3e8 ms",
            "1e3 s",
            "1.2.3 s",
            ". s",
            "1..5 s",
            "+1 s",
            "1,5 s",
            "1 s please",
        ] {
            assert!(parse_interval(bad).is_err(), "{bad:?} was accepted");
        }

        // The whitespace is required, and what a value written together is
        // told is where the space goes -- not that its unit is unknown,
        // which is a message about the wrong half of the value.
        for together in ["500ms", "3s", "15min", "1.5s", "1 w2 d"] {
            let why = parse_interval(together).expect_err("written together");
            assert!(
                why.contains("space between them")
                    || why.contains("nothing else"),
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
        for zero in [
            "0 s", "0 ms", "0 us", "0 min", "0 h", "0 d", "0 w", "0.0 s",
            "0.4 us",
        ] {
            let said =
                parse_interval(zero).expect_err("nought should be refused");
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
        let plain = Destination::parse("gateway");
        assert_eq!(plain.node, "gateway");
        assert_eq!(plain.input, None);

        let named = Destination::parse("zone_temp.in0");
        assert_eq!(named.node, "zone_temp");
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
    /// The real test of the reader: tests/manual/thermostat.ytcs and .xtcs are
    /// written by tcs-gui, not by hand here, so this is the editor and the
    /// flight-side reader agreeing about a file rather than two pieces of this
    /// module agreeing with each other.
    #[test]
    fn the_shipped_example_reads_the_same_in_both_formats() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

        let yaml = Graph::load(root.join("tests/manual/thermostat.ytcs"))
            .expect("the YAML example reads");
        let xml = Graph::load(root.join("tests/manual/thermostat.xtcs"))
            .expect("the XML example reads");

        assert_eq!(
            yaml.nodes, xml.nodes,
            "the two formats describe different nodes"
        );
        assert_eq!(
            yaml.links, xml.links,
            "the two formats describe different links"
        );
        assert_eq!(yaml.commands, xml.commands);
        assert_eq!(yaml.groups, xml.groups);

        // And it is the thermostat: three sensors, a vote over them, a
        // comparison, the ground, two heaters and a log.
        assert_eq!(
            yaml.nodes.len(),
            12,
            "{:?}",
            yaml.nodes.iter().map(|n| &n.name).collect::<Vec<_>>()
        );
        assert_eq!(yaml.groups.len(), 1, "the three sensors share one group");
        assert_eq!(
            yaml.group("zone_temps").map(|g| g.interval()),
            // Ten times a second, which is what holds the band: the heaters
            // move the mass several kelvin a second, so a loop that acted
            // once a second would overshoot the band every time.
            Some(Ok(std::time::Duration::from_millis(100)))
        );
        let sensor = yaml.node("temp_1").expect("the first of the trio");
        assert_eq!(sensor.node_type, "i2c");
        assert_eq!(
            sensor.attributes.get("address").map(Scalar::as_str),
            Some("0x48")
        );
        // And where it was drawn, which is the editor's section at the end
        // of the file rather than a field on the node: a node says what the
        // vehicle carries, and this says where somebody put the box.
        assert_eq!(
            yaml.placed.get("temp_1").map(|at| (at.x, at.y)),
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
        let graph = Graph::load(root.join("tests/manual/thermostat.ytcs"))
            .expect("it reads");

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
        let top = where_.path().join("vehicle.ytcs");

        // With a tcspecial section, which every configuration must have and
        // only the top file may: see Graph::tcspecial.
        std::fs::write(
            &top,
            "include:
  - subdiagram: zone
    path: zone
tcspecial:
  gateway: radio
  interface: \"c7820f9bfe585feab72247ece0a2261d\"
  tcs_cmd:
    \
             recv: 0.0.0.0:4000
    send: 127.0.0.1:4100
  \
             payload_command:
    source_base: 127.0.0.1:4300
    dest_base: \
             0.0.0.0:4200
  payload_telemetry:
    source_base: \
             0.0.0.0:6000
    dest_base: 127.0.0.1:6100
  state_file: /var/lib/tcspecial/state
  beacon:
    \
             default_interval: 5 s
    reset_interval: 1 s
    send: \"0.0.0.0:5550\"
    recv: \"\
             127.0.0.1:5650\"
gateways:
  - type gateway_radio:
      - { gateway: gateway, address: \"10.0.0.1\", port: \"5000\", \
             ground_base: \"10.0.0.2:5100\", world-in: [zone] }
",
        )
        .unwrap();
        // And the included file's own payload, with the link that joins the
        // two: a link is an attribute of the node it leaves, so a link across
        // the boundary between two files is written in the file the node is
        // in.
        std::fs::write(
            where_.path().join("zone.ytcs"),
            "exports:\n  world-out: [zone_sensor]\n\
             payloads:\n  \
             - type i2c:\n      \
             - { payload: zone_sensor, path: \"/dev/i2c-1\", \
             address: \"0x48\", world-out: gateway.zone }\n",
        )
        .unwrap();

        let graph = Graph::load(&top).expect("the pair reads as one");
        assert_eq!(graph.nodes.len(), 2);
        assert!(
            graph.node("zone_sensor").is_some(),
            "the include was not read"
        );
        assert_eq!(graph.files.len(), 2, "both files should be recorded");

        // Which file each node came from, so the digest can cover the set and
        // a complaint can say where to look.
        assert!(graph.homes["zone_sensor"].ends_with("zone.ytcs"));
        assert!(graph.homes["gateway"].ends_with("vehicle.ytcs"));

        // And a link across the boundary is sound, which is what including a
        // file is for, at a node the file exports.
        assert_eq!(graph.check(), Vec::new(), "{:?}", graph.check());
    }

    /// A configuration has no version field, so a file that writes one is
    /// refused as it would be for any field it does not have.
    ///
    /// Nothing reads a version out of a file: the software is what says which
    /// version of the language it reads. Refused rather than ignored, as every
    /// unknown field is, because a field that is read and never acted on is
    /// a field somebody will one day believe in.
    #[test]
    fn a_version_is_a_field_a_configuration_does_not_have() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("old.ytcs");

        for version in ["1.0", "2.0"] {
            std::fs::write(
                &path,
                format!("version: \"{version}\"\npayloads: []\n"),
            )
            .unwrap();
            let said = format!(
                "{}",
                Graph::load(&path).expect_err("a version is refused")
            );
            assert!(said.contains("unknown field `version`"), "{said}");
        }

        // And the same file without the line reads.
        std::fs::write(&path, "payloads: []\n").unwrap();
        Graph::load(&path).expect("no version, nothing wrong");
    }

    /// A subdiagram is named, its path under its name, in both spellings;
    /// and its name is one no node and no other subdiagram has, with no dot
    /// or slash in it. A bare path, as includes were once written, is
    /// refused, saying how to write it now.
    #[test]
    fn a_subdiagram_is_named_and_its_name_is_its_own() {
        let where_ = tempfile::tempdir().expect("a directory");
        let dir = where_.path();
        std::fs::write(dir.join("zone.ytcs"), "payloads: []\n").unwrap();
        std::fs::write(
            dir.join("zone.xtcs"),
            "<configuration><payloads/></configuration>\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("room.ytcs"),
            "payloads:\n  - type i2c:\n      - payload: probe\n",
        )
        .unwrap();

        let top = dir.join("vehicle.xtcs");
        std::fs::write(
            &top,
            "<configuration>\n    \
             <include name=\"inside\" path=\"zone\"/>\n\
             </configuration>\n",
        )
        .unwrap();
        let graph = Graph::load(&top).expect("the XML spelling reads");
        assert_eq!(
            graph.included(),
            [Include {
                name: "inside".into(),
                path: "zone".into(),
            }]
        );
        assert_eq!(
            graph.include_name(&dir.join("zone.xtcs")).as_deref(),
            Some("inside")
        );

        let top = dir.join("vehicle.ytcs");
        let refused = |includes: &str| {
            std::fs::write(&top, format!("include:\n{includes}")).unwrap();
            format!("{}", Graph::load(&top).expect_err("it is refused"))
        };
        assert!(refused("  - zone\n").contains("subdiagram: <name>"));
        assert!(refused("  - subdiagram: probe\n    path: room\n")
            .contains("the name of a node"));
        assert!(refused(
            "  - subdiagram: a\n    path: zone\n  \
             - subdiagram: a\n    path: room\n"
        )
        .contains("named twice"));
        assert!(refused("  - subdiagram: a.b\n    path: zone\n")
            .contains("a dot or a slash"));
        assert!(refused("  - subdiagram: zone\n").contains("no path"));
    }

    /// An include naming the same file twice is refused.
    #[test]
    fn a_file_is_not_included_twice() {
        let where_ = tempfile::tempdir().expect("a directory");
        let top = where_.path().join("vehicle.ytcs");
        std::fs::write(&top, "include:\n  - subdiagram: zone\n    path: zone\n  - subdiagram: zone2\n    path: zone\n").unwrap();
        std::fs::write(where_.path().join("zone.ytcs"), "payloads: []\n")
            .unwrap();

        let said =
            format!("{}", Graph::load(&top).expect_err("twice is refused"));
        assert!(said.contains("included twice"), "{said}");
    }

    /// A file reached by two include names is still included twice, and so
    /// is the top file named as an include.
    #[test]
    fn a_file_named_two_ways_is_still_included_twice() {
        let where_ = tempfile::tempdir().expect("a directory");
        let vehicle = where_.path().join("vehicle");
        std::fs::create_dir(&vehicle).unwrap();
        let top = vehicle.join("top.ytcs");
        std::fs::write(vehicle.join("zone.ytcs"), "payloads: []\n").unwrap();

        for includes in [
            "  - subdiagram: one\n    path: zone\n  \
             - subdiagram: two\n    path: ../vehicle/zone\n",
            "  - subdiagram: one\n    path: ../vehicle/top\n",
        ] {
            std::fs::write(&top, format!("include:\n{includes}")).unwrap();
            let said =
                format!("{}", Graph::load(&top).expect_err("twice is refused"));
            assert!(said.contains("included twice"), "{said}");
        }
    }

    /// An include may name a file in another directory, by its path from
    /// the top file's, and it is known by that name as it was written.
    #[test]
    fn an_include_may_be_in_another_directory() {
        let where_ = tempfile::tempdir().expect("a directory");
        for dir in ["vehicle", "common"] {
            std::fs::create_dir(where_.path().join(dir)).unwrap();
        }
        let top = where_.path().join("vehicle/top.ytcs");
        std::fs::write(
            &top,
            "include:\n  - subdiagram: zone\n    path: ../common/zone\n",
        )
        .unwrap();
        std::fs::write(
            where_.path().join("common/zone.ytcs"),
            "vehicle:\n  - type ping:\n      - { compute: p }\n",
        )
        .unwrap();

        let graph = Graph::load(&top).expect("it reads");
        assert!(graph.node("p").is_some(), "the include was not read");
        let paths: Vec<String> =
            graph.included().into_iter().map(|i| i.path).collect();
        assert_eq!(paths, vec!["../common/zone".to_string()]);

        // And from another working directory, by another path to the same
        // top file, it is named the same, which is what the digest takes.
        let again =
            Graph::load(where_.path().join("common/../vehicle/top.ytcs"))
                .expect("it reads");
        assert_eq!(again.included(), graph.included());
    }

    /// An include name that is absolute, empty, or has a part that is empty
    /// or `.` is refused, saying which and what a name is.
    #[test]
    fn an_include_name_of_the_wrong_shape_is_refused() {
        let where_ = tempfile::tempdir().expect("a directory");
        let top = where_.path().join("vehicle.ytcs");
        std::fs::write(where_.path().join("zone.ytcs"), "payloads: []\n")
            .unwrap();
        let absolute = where_.path().join("zone");
        for (name, why) in [
            ("\"\"", "is empty"),
            (absolute.to_str().unwrap(), "is an absolute path"),
            ("./zone", "has a part that is empty"),
            ("sub//zone", "has a part that is empty"),
            ("zone/", "has a part that is empty"),
            ("~/zone", "begins with ~"),
            ("~zone", "begins with ~"),
            ("$HOME/zone", "has a $ in it"),
            ("${COMMON}/zone", "has a $ in it"),
        ] {
            std::fs::write(
                &top,
                format!("include:\n  - subdiagram: sub\n    path: {name}\n"),
            )
            .unwrap();
            let said =
                format!("{}", Graph::load(&top).expect_err("it is refused"));
            assert!(said.contains(why), "{name}: {said}");
            assert!(said.contains("../common/zone"), "{name}: {said}");
            assert!(
                said.contains("relative to this file's directory"),
                "{name}: {said}"
            );
        }
    }

    /// An included file may not include others.
    #[test]
    fn an_include_does_not_nest() {
        let where_ = tempfile::tempdir().expect("a directory");
        let top = where_.path().join("vehicle.ytcs");
        std::fs::write(
            &top,
            "include:\n  - subdiagram: zone\n    path: zone\n",
        )
        .unwrap();
        std::fs::write(
            where_.path().join("zone.ytcs"),
            "include:\n  - subdiagram: deeper\n    path: deeper\n",
        )
        .unwrap();

        let said =
            format!("{}", Graph::load(&top).expect_err("nesting is refused"));
        assert!(said.contains("may not include others"), "{said}");
    }

    /// A line into or out of an included file reaches only what it exports.
    ///
    /// A gateway node in the top file, a sensor in it feeding a conversion
    /// in the included file, and the conversion's reading going back to the
    /// gateway node: two lines crossing the file's edge, one in and one out.
    /// Each is refused until the file exports the end it reaches, and the
    /// refusal says which list to add it to.
    #[test]
    fn a_line_crosses_into_an_included_file_only_where_it_exports() {
        use crate::graph::check::Problem;

        let where_ = tempfile::tempdir().expect("a directory");
        let top = where_.path().join("vehicle.ytcs");
        std::fs::write(
            &top,
            "include:\n  - subdiagram: zone\n    path: zone\n\
             gateways:\n  - type gateway_radio:\n      \
             - gateway: ground\n        world-in: [temp]\n\
             payloads:\n  - type i2c:\n      - payload: sensor\n        \
             world-out:\n          - temp.sensor\n",
        )
        .unwrap();
        let zone = |exports: &str| {
            std::fs::write(
                where_.path().join("zone.ytcs"),
                format!(
                    "{exports}vehicle:\n  - type compute:\n      \
                     - compute: temp\n        function: \"to_kelvin\"\n        \
                     world-in: [sensor]\n        world-out:\n          \
                     - ground.temp\n"
                ),
            )
            .unwrap();
            Graph::load(&top)
                .expect("it loads")
                .check()
                .into_iter()
                .filter(|p| {
                    matches!(
                        p,
                        Problem::NotExported { .. } | Problem::BadExport { .. }
                    )
                })
                .collect::<Vec<_>>()
        };

        let refused = zone("");
        assert_eq!(refused.len(), 2, "{refused:?}");
        let said: Vec<String> = refused.iter().map(Problem::describe).collect();
        assert!(
            said.iter().any(|s| s.contains("\"temp\" is in zone")
                && s.contains("add \"temp\" to world-in under exports")),
            "{said:?}"
        );
        assert!(
            said.iter()
                .any(|s| s.contains("to world-out under exports")),
            "{said:?}"
        );

        // Half exported, half refused.
        assert_eq!(
            zone("exports:\n  world-in: [temp]\n").len(),
            1,
            "the reading going out is not exported yet"
        );
        assert!(zone("exports:\n  world-in: [temp]\n  world-out: [temp]\n")
            .is_empty());

        // And a list naming what is not there, or a terminal the node has
        // not got.
        let bad = zone(
            "exports:\n  world-in: [temp, sensor, nobody]\n  \
             world-out: [temp]\n",
        );
        let why: Vec<String> = bad
            .iter()
            .map(|p| match p {
                Problem::BadExport { node, why, .. } => format!("{node} {why}"),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(why, ["sensor is in another file", "nobody is not a node"]);
    }

    /// A line to one of a vote's named inputs arrives at its `world-in`.
    ///
    /// `vote.in0` names an input of the vote, not a terminal, so the
    /// export it needs is the vote's `world-in`: a file exporting that is
    /// joined, and one that does not is told to add the vote to
    /// `world-in`, never to an `in0` no exports section can have.
    #[test]
    fn a_line_to_a_named_input_crosses_at_world_in() {
        use crate::graph::check::Problem;

        let where_ = tempfile::tempdir().expect("a directory");
        let top = where_.path().join("vehicle.ytcs");
        std::fs::write(
            &top,
            "include:\n  - subdiagram: zone\n    path: zone\n\
             gateways:\n  - type gateway_radio:\n      \
             - gateway: ground\n        world-in: [vote]\n\
             payloads:\n  - type i2c:\n      - payload: sensor\n        \
             world-out:\n          - vote.in0\n",
        )
        .unwrap();
        let zone = |exports: &str| {
            std::fs::write(
                where_.path().join("zone.ytcs"),
                format!(
                    "{exports}vehicle:\n  - type compute:\n      \
                     - compute: vote\n        function: \"vote\"\n        \
                     algorithm: \"median\"\n        world-in: [in0]\n        \
                     world-out:\n          - ground.vote\n"
                ),
            )
            .unwrap();
            Graph::load(&top)
                .expect("it loads")
                .check()
                .into_iter()
                .filter(|p| matches!(p, Problem::NotExported { .. }))
                .map(|p| p.describe())
                .collect::<Vec<_>>()
        };

        let said = zone("exports:\n  world-out: [vote]\n");
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(
            said[0].contains("add \"vote\" to world-in under exports"),
            "{said:?}"
        );
        assert!(zone("exports:\n  world-in: [vote]\n  world-out: [vote]\n")
            .is_empty());
    }

    /// A top file has nothing around it, so nothing to export to.
    #[test]
    fn a_top_file_may_not_export() {
        let where_ = tempfile::tempdir().expect("a directory");
        let top = where_.path().join("vehicle.ytcs");
        std::fs::write(&top, "exports:\n  world-in: [temp]\n").unwrap();

        let said = format!("{}", Graph::load(&top).expect_err("it is refused"));
        assert!(
            said.contains("only an included file may have an exports section"),
            "{said}"
        );
    }

    /// A name that is not a configuration file is refused before it is read.
    #[test]
    fn a_file_that_says_nothing_about_its_format_is_refused() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.json");
        std::fs::write(&path, "{}").unwrap();

        let said =
            format!("{}", Graph::load(&path).expect_err("json is refused"));
        assert!(said.contains(".ytcs"), "{said}");
    }

    /// One destination or several, however the file wrote it.
    #[test]
    fn a_link_may_go_to_one_place_or_many() {
        let one: Link = serde_norway::from_str("from: a\nto: b\n")
            .expect("one destination");
        assert_eq!(one.to.each(), vec!["b"]);

        let many: Link = serde_norway::from_str("from: a\nto: [b, c.in0]\n")
            .expect("several destinations");
        assert_eq!(many.to.each(), vec!["b", "c.in0"]);
    }

    /// A link is written as the node it reaches and the name it arrives
    /// by, one line each: a list, or one destination on its own.
    #[test]
    fn a_link_is_written_as_its_node_and_a_name() {
        let said = yaml_node(
            "compute",
            "compute: k_1
world-out:
  - gateway.readings
  - zone_temp.in0
",
        )
        .expect("a list");
        assert_eq!(said.world_out, vec!["gateway.readings", "zone_temp.in0"]);

        let one = yaml_node("compute", "compute: a\nworld-out: gateway.a\n")
            .expect("one");
        assert_eq!(one.world_out, vec!["gateway.a"]);

        let xml = xml_node(
            "compute",
            "<compute name=\"k_1\"><world-out>gateway.readings</world-out>\
             <world-out>zone_temp.in0</world-out></compute>",
        )
        .expect("repeated elements");
        assert_eq!(xml.world_out, said.world_out);
    }

    /// The entry a link was once written as is refused, told the line to
    /// write instead: `target: gateway` names no name to arrive by.
    #[test]
    fn a_link_written_as_an_entry_is_refused() {
        for entry in [
            "compute: a\nworld-out:\n  - target: gateway\n",
            "compute: a\nworld-out:\n  - { target: z, input: in0 }\n",
            "compute: a\nworld-out: { target: gateway }\n",
        ] {
            let said = yaml_node("compute", entry).expect_err("an entry");
            assert!(said.contains("`zone_temp.in0`"), "{said}");
            assert!(said.contains("`target`"), "{said}");
        }
    }

    /// A node's terminators are a list: a YAML sequence, or a repeated XML
    /// element, and the two say the same thing. A lone value is a list of
    /// one.
    #[test]
    fn terminators_are_read_as_a_list_in_either_format() {
        let yaml = yaml_node(
            "serial",
            "payload: a\nterminators:\n  - '\"\\r\\n\"'\n  - '[03]'\n",
        )
        .expect("a list");
        let xml = xml_node(
            "serial",
            "<payload name=\"a\"><terminators>\"\\r\\n\"\
             </terminators><terminators>[03]</terminators></payload>",
        )
        .expect("repeated elements");
        let joined = |file: &NodeFile| {
            file.node.attributes["terminators"].as_str().to_string()
        };
        assert_eq!(joined(&yaml), "\"\\r\\n\", [03]");
        assert_eq!(joined(&xml), joined(&yaml));

        let one = yaml_node("serial", "payload: a\nterminators: '[0A]'\n")
            .expect("one");
        assert_eq!(joined(&one), "[0A]");
    }

    /// Several terminators written as one value with commas between them
    /// are refused: a terminator is an entry of its own, which is the one
    /// shape -- and a comma inside a string is a character of it.
    #[test]
    fn terminators_written_as_one_value_are_refused() {
        let said =
            yaml_node("serial", "payload: a\nterminators: '[0D], [0A]'\n")
                .expect_err("one value, two terminators");
        assert!(said.contains("entry of its own"), "{said}");

        let comma = yaml_node("serial", "payload: a\nterminators: '\",\"'\n")
            .expect("a comma in a string is a character");
        assert_eq!(comma.node.attributes["terminators"].as_str(), "\",\"");
    }
}
