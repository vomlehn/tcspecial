//! The configuration being edited.
//!
//! This is an *editing* model, and deliberately not the schema a vehicle
//! flies. The difference is the whole reason there are two types: an editor
//! must be able to hold a configuration that is wrong -- a link to a node not
//! placed yet, a mean with two of its three inputs bound, a name typed halfway
//! -- because that is what the middle of an edit looks like, and an editor
//! that refused to hold it would be an editor that could not be used. The
//! flight schema must hold no such thing.
//!
//! So a document may be invalid, says where and why, and is turned into the
//! flight configuration only when asked. What is wrong is what the window
//! draws in red; everything else about a node is drawn without it.
//!
//! The document also holds what the vehicle does not care about -- where on the
//! canvas each node sits -- because a person who arranges a diagram expects the
//! arrangement to survive being closed, and because it is cheap to keep it
//! beside the thing it describes rather than in a second file.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::journal::{Edit, Link};
use crate::pane::{is_payload, size_of};
use crate::style::NodeType;
use crate::subdiagram::{file_for, Home, Qualified};
use tcslibgs::endpoint_config::Scalar;
use tcslibgs::graph::{section_of, Section};
use tcslibgs::ConfigFormat;

/// One node as the editor holds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub name: String,
    /// The type's spelling, not the enum: a document read from a file may name
    /// a type this build does not have, and saying so is better than refusing
    /// to open the file.
    pub node_type: String,
    /// Everything else the type requires, as written. Kept as text rather than
    /// parsed into numbers and enumerations because the editor's job is to let
    /// a person type, and "0x4" on the way to "0x48" is not a number yet.
    pub attributes: BTreeMap<String, String>,
    /// Where on the canvas. Nothing to do with the configuration.
    pub at: (i32, i32),
    /// Which subdiagram this node lives in, or `None` for the top diagram.
    ///
    /// The editor holds one assembled document whatever it was read from, and
    /// this is what says which file each node goes back to. It is also what
    /// lets two subdiagrams each have a `temp_1`: a node is known by its
    /// name within its own home, and by `home/name` from outside.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Home,
    /// What somebody wrote about this node.
    ///
    /// Kept with the node and written out with it. A configuration is read by
    /// people under pressure, and why a sensor is at address 0x49 is not
    /// deducible from the fact that it is -- so there has to be somewhere to
    /// say it, and that somewhere has to survive the editor rewriting the
    /// file, which it does after every edit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// A statement that the ground may set an attribute of a node.
///
/// The fourth thing a configuration is made of, and the one with nothing on
/// the canvas: a binding is not drawn, because what it describes is a value
/// arriving rather than data travelling a path. It is held and written all the
/// same -- a configuration the editor rewrote without it would be a thermostat
/// whose setpoint had quietly stopped being commandable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    /// The gateway node the command arrives on.
    pub from: String,
    /// What it sets, as `node.attribute`.
    pub sets: String,
    /// What somebody wrote about it, kept as the comments on nodes and links
    /// are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// The file it is written in: a subdiagram, or `None` for the top
    /// diagram, where a binding made in the editor goes. Kept so that a
    /// binding read from an included file is written back there rather than
    /// moved into the top file by a save.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Home,
}

/// Which line of a file's structure a comment is written above.
///
/// A comment about a section as a whole, or about every node of one type, is
/// not about any node, so a node cannot carry it -- and an editor that kept
/// only what nodes carry would delete the paragraph under `payloads:` that
/// says why every payload is the far end of a socket. These are the places
/// such a comment is found and written back.
///
/// The section is named as the file names it, `payloads`; the type as it is
/// written on its group's line, `network`. A type belongs to one section --
/// see `tcslibgs::graph::section_of` -- so the type alone says which group.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
pub enum Heading {
    /// At the head of the file, before anything it says, with a blank line
    /// under it: what the file is, which a file shared by two
    /// configurations says for both of them.
    File,
    /// Above the line that opens the section: `payloads:` or `<payloads>`.
    Section(String),
    /// Under that line, above the section's first type group.
    ///
    /// Which is also where the first group's own comment would go, so a file
    /// cannot tell the two apart and the reader takes it as the section's:
    /// it is the paragraph the converter put there, and the one a person
    /// writes. A group comment that comes to be first, its group's
    /// predecessors gone, is written there too and read back as the
    /// section's -- the words kept, their owner changed.
    Opening(String),
    /// Above the line that opens a type group: `- type network:` or
    /// `<type name="network">`.
    Type(String),
}

/// A comment about part of a file's structure rather than about a node in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remark {
    /// Which file: the subdiagram, or `None` for the top diagram.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Home,
    /// Which line it is written above.
    pub heading: Heading,
    /// What it says, a line of text for each of its lines.
    pub text: String,
}

/// Something that might be wrong with the drawing, and might not be.
///
/// Apart from [`Fault`], which is what *is* wrong: a fault is a document the
/// editor will not pretend is sound -- two nodes of one name, a link to a node
/// that is not there -- and a concern is a shape somebody may have meant. A
/// payload nothing reads yet is the ordinary state of a diagram halfway
/// through being drawn, and an editor that called it an error would be an
/// editor crying wolf at its own user.
///
/// See [`Document::concerns`] for which rules apply where, and
/// [`Document::aside`] for how one is set aside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Concern {
    /// A node at the start of a stream whose data reaches nothing: a payload
    /// whose readings go nowhere, a gateway link nothing is listening to.
    GoesNowhere { node: String, terminal: String },
    /// A vehicle node something arrives at and nothing leaves. The stream
    /// stops there, and a log is the one type that is allowed to stop one.
    NothingLeaves { node: String },
    /// A vehicle node something leaves and nothing arrives at: it is sending
    /// what it has not been given.
    NothingArrives { node: String },
}

impl Concern {
    /// Which node this is about.
    ///
    /// # Returns
    ///
    /// The node's absolute name.
    pub fn node(&self) -> &str {
        match self {
            Concern::GoesNowhere { node, .. }
            | Concern::NothingLeaves { node }
            | Concern::NothingArrives { node } => node,
        }
    }

    /// The word for this kind, as the file writes it.
    ///
    /// Part of what a set-aside is remembered by, so it has to be stable: a
    /// word changed here is a decision somebody made and lost.
    ///
    /// # Returns
    ///
    /// `<terminal>-goes-nowhere`, `nothing-leaves` or `nothing-arrives`.
    pub fn kind(&self) -> String {
        match self {
            Concern::GoesNowhere { terminal, .. } => {
                format!("{terminal}-goes-nowhere")
            }
            Concern::NothingLeaves { .. } => "nothing-leaves".to_string(),
            Concern::NothingArrives { .. } => "nothing-arrives".to_string(),
        }
    }

    /// What identifies this concern: the node, and the kind.
    pub fn key(&self) -> (String, String) {
        (self.node().to_string(), self.kind())
    }

    /// What the button's menu says about it, in one line.
    ///
    /// # Returns
    ///
    /// The line, naming the node.
    pub fn describe(&self) -> String {
        match self {
            Concern::GoesNowhere { node, terminal } => {
                format!("\"{node}\": nothing is linked to its {terminal}")
            }
            Concern::NothingLeaves { node } => {
                format!("\"{node}\": something arrives and nothing leaves")
            }
            Concern::NothingArrives { node } => {
                format!("\"{node}\": something leaves and nothing arrives")
            }
        }
    }
}

/// Where one line goes, as a file writes it: the node it reaches and the name
/// it arrives by, `zone_temp.in0`, under the data source it leaves by. Not a
/// type the document holds: the document holds links, and this is one of them
/// on its way into a file.
///
/// See `tcslibgs::graph::Destinations` for how a reader takes it.
struct Bound {
    /// The node the line goes to, written as the file names it: a node on
    /// another page is named from the page this one is written on.
    target: String,
    /// The name it arrives by: see `Link::arrives_by`.
    name: String,
}

impl Bound {
    /// The line as the file writes it.
    fn written(&self) -> String {
        format!("{}.{}", self.target, self.name)
    }
}

/// Whether a node's attribute is written as one of its sinks' lists of names
/// rather than as an attribute: see `Document::sinks`.
///
/// # Arguments
///
/// * `node`      - The node.
/// * `attribute` - The attribute.
///
/// # Returns
///
/// Whether the sink lists write it.
fn in_a_sink(node: &Node, attribute: &str) -> bool {
    attribute == "world-in" || attribute == "moc-in" || votes(node, attribute)
}

/// Whether this is a vote's inputs, which a file writes as its world-in.
///
/// # Arguments
///
/// * `node`      - The node.
/// * `attribute` - The attribute.
///
/// # Returns
///
/// Whether it is.
fn votes(node: &Node, attribute: &str) -> bool {
    attribute == "inputs"
        && node.node_type == "compute"
        && node.attributes.get("function").map(String::as_str) == Some("vote")
}

/// The whole of what is being edited.
/// One file of a configuration being written somewhere else: see
/// [`Document::files_moved`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    /// Where the file is now.
    pub from: std::path::PathBuf,
    /// Where it is going.
    pub to: std::path::PathBuf,
    /// What goes in it there.
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// The concerns somebody has decided not to act on, by node and kind.
    ///
    /// A decision about this configuration rather than about this session, so
    /// it is written in the file's own `tcsgui` section beside where the nodes
    /// were drawn: two people editing one configuration should not each have
    /// to decide again that a payload nobody reads yet is on purpose. See
    /// [`Concern::key`].
    #[serde(
        default,
        skip_serializing_if = "std::collections::BTreeSet::is_empty"
    )]
    pub aside: std::collections::BTreeSet<(String, String)>,
    /// What the configuration says it is, as the file's `description`.
    ///
    /// Held so that saving does not drop it. It is a line of the schema rather
    /// than a comment, so a reader can see it and an editor that wrote the
    /// file back without it would be deleting something somebody wrote.
    #[serde(default)]
    pub description: Option<String>,
    /// The simulator's configuration, where this session was given one.
    ///
    /// A second file and a second thing: how fast a *simulated* payload
    /// produces packets and what it does wrong, none of which is a property of
    /// the payload. See `sim.rs`, which says why it is not on the canvas.
    #[serde(default)]
    pub simulator: Option<crate::sim::Simulator>,
    /// What tcspecial itself is configured with: where commands are taken,
    /// where beacons go, where the telemetry log is written.
    ///
    /// Required of every configuration, and the one part of a configuration
    /// that is not on the canvas: it is about the whole of it rather than
    /// about any node in it, so it is edited in a window of its own and not by
    /// pressing something. See `tcslibgs::graph::parameters`, which holds the
    /// list of what it may say and the rules about it.
    ///
    /// Text, as a node's attributes are, because the editor has to be able to
    /// hold a port somebody is halfway through typing.
    #[serde(default)]
    pub tcspecial: BTreeMap<String, String>,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    /// The groups, by name, each with the interval it reads on.
    ///
    /// A group is one of the four things a configuration is made of and it was
    /// missing here: nodes could say `group: zone_temps` and nothing declared
    /// what that group was, so the editor wrote a configuration the flight
    /// reader refuses. Found by the checker on the shipped example,
    /// which is what a checker is for.
    #[serde(default)]
    pub groups: BTreeMap<String, String>,
    /// The subdiagram each group is declared in, by group name; a group not
    /// named here is the top file's, where a group made in the editor goes.
    ///
    /// Kept apart from `groups` and left alone when a group is removed, so
    /// that undoing the removal puts it back in the file it came from, and
    /// so that a save writes a group read from an included file back there
    /// rather than moving it into the top file.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub group_homes: BTreeMap<String, String>,
    /// What each subdiagram's file exports, by the subdiagram's name: for
    /// each terminal, the nodes of that file, by their own names, a line
    /// from another file may reach there. See
    /// `tcslibgs::graph::exports`.
    ///
    /// Left alone when a node is removed, so that undoing the removal finds
    /// the node exported as it was; a name with no node behind it is not
    /// written. See [`Document::exports_written`].
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub exports: BTreeMap<String, tcslibgs::graph::exports::Exports>,
    /// What the ground may set in flight.
    ///
    /// Not editable on the canvas yet, and written out whatever else happens:
    /// see [`Command`].
    #[serde(default)]
    pub commands: Vec<Command>,
    /// What somebody wrote about a section or a type group: see [`Heading`].
    ///
    /// Each lives as long as what it is about. A section or group the editor
    /// empties is not written, so its comment would have nowhere to go, and
    /// one that came back later would be a new group with somebody else's
    /// words over it: see [`Document::forget_lost_comments`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remarks: Vec<Remark>,
    /// What somebody wrote above a line of the `tcspecial` section, by the
    /// attribute that line states: `state_file`, a subsection such as
    /// `beacon`, or a subsection's attribute such as `beacon.recv`, flat as
    /// [`Document::tcspecial`] is.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tcspecial_comments: BTreeMap<String, String>,
}

/// One attribute a group may set on all of its payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupField {
    pub attribute: String,
    /// What every payload of the group says, where they all say the same.
    ///
    /// `None` is "none of them states one" where `differs` is false, and
    /// "they do not agree" where it is true. The two are different things to
    /// show and the same thing to set: typing a value here gives it to all of
    /// them.
    pub value: Option<String>,
    pub differs: bool,
}

/// What the flight software would make of this node, or what is wrong with
/// what it says.
///
/// The editor's node turned into the schema's and handed to the rules that
/// have been tested against real buses. A conversion and not a second set of
/// rules: a bus address is refused here for exactly what it would be refused
/// for aboard, in the same words, which is the whole reason the editor can say
/// anything useful about a value at all.
///
/// # Arguments
///
/// * `node` - The node, as the editor holds it.
///
/// # Errors
///
/// The rules' own words for what is wrong: what an app node lacks, or why a
/// payload's or the gateway link's way of being reached is refused.
fn reach_of_node(node: &Node) -> Result<(), String> {
    let theirs = tcslibgs::graph::Node {
        name: node.name.clone(),
        node_type: node.node_type.clone(),
        group: node
            .attributes
            .get("group")
            .map(|group| group.trim().to_string())
            .filter(|group| !group.is_empty()),
        attributes: node
            .attributes
            .iter()
            .filter(|(attribute, value)| {
                // The group is a field of the schema rather than one of the
                // attributes, and an empty value is one nobody has answered:
                // neither belongs in what the rules are shown.
                *attribute != "group" && !value.trim().is_empty()
            })
            .map(|(attribute, value)| {
                (attribute.clone(), Scalar::of(value.as_str()))
            })
            .collect(),
    };

    // An app node -- two of the kinds are payloads -- is reached by running
    // it, not over an interface, so what it is held to is what an app node
    // must say: its executable and the arguments its kind is run with. The
    // same rule tcspecial reads it by, so what this says is what is missing
    // rather than that an app has no address, which no app has.
    if tcslibgs::node::app::Kind::of(&theirs.node_type).is_some() {
        return tcslibgs::node::app::params_of(
            &theirs.name,
            &theirs.node_type,
            &theirs.attributes,
        )
        .map(|_| ());
    }

    // Only the nodes that are opened. A computation and a log have rules of
    // their own, and this would tell them they are not interfaces.
    if !theirs.is_payload()
        && !matches!(
            theirs.node_type.as_str(),
            "gateway_radio" | "gateway_yamcs"
        )
    {
        return Ok(());
    }
    tcslibgs::graph::reach::reach_of(&theirs).map(|_| ())
}

/// Whether a node names this group.
fn in_group(node: &Node, group: &str) -> bool {
    node.attributes.get("group").map(|g| g.trim()) == Some(group)
}

/// Something wrong with the document, and what it is attached to.
///
/// The editor's errors, which are the only things drawn in red. Each names one
/// node or one link, so the window can colour that thing rather than the
/// canvas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
    /// Two nodes with one name. Links name nodes, so this makes a link
    /// ambiguous as well as the node.
    DuplicateName { name: String },
    /// A node with no name at all, which nothing can link to.
    NamelessNode { at: (i32, i32) },
    /// A type this build does not know how to place.
    UnknownType { node: String, node_type: String },
    /// A link naming a node that is not in the document.
    DanglingLink { from: String, to: String },
    /// A link from a node to itself.
    SelfLink { node: String },
    /// Two links into one input of one node.
    InputBoundTwice { node: String, input: String },
    /// A node in a group the configuration does not declare.
    ///
    /// The group owns the interval, so a node in one that is not there is a
    /// node with no interval at all.
    NoSuchGroup { node: String, group: String },
    /// A group nothing is in, which reads nothing on its interval.
    EmptyGroup { group: String },
    /// A node whose attributes do not hold together.
    ///
    /// Not a node that is unfinished. The editor is lazy about what has been
    /// entered and strict about what it says: a bus address of `0xZZ` is
    /// wrong whatever else is missing, and an attribute belonging to another
    /// type is wrong on this one. Absence is checked on the ground and aboard,
    /// where the complaint can name the file and the line -- see
    /// `tcslibgs::graph::check::Problem::BadNode`.
    ///
    /// `why` is the reason in the words of the rules that have been tested
    /// against real buses, so the editor says what `tcsverify` will say.
    BadNode { node: String, why: String },
    /// A tcspecial section whose parameters do not hold together.
    ///
    /// Strict in the same way and for the same reason: a beacon address with
    /// no port is wrong as soon as it is typed, and an unanswered parameter is
    /// not wrong at all yet.
    BadTcspecial { why: String },
    /// A link delivering into a payload that is read and states no trigger.
    ///
    /// The same rule `tcsverify` applies on the ground and tcspecial applies
    /// again aboard: what is written to a payload that is read is what asks
    /// it for a reading, and the trigger is what is written. Marked here as
    /// the link is drawn, which is the moment at which saying so costs
    /// nobody anything.
    UntriggeredInput { node: String, from: String },
}

impl Fault {
    /// What the window shows beside the thing it is attached to.
    ///
    /// # Returns
    ///
    /// The text, in the words `tcsverify` would use where the fault is a
    /// rule's.
    pub fn describe(&self) -> String {
        match self {
            Fault::DuplicateName { name } => {
                format!("two nodes are called \"{name}\"")
            }
            Fault::NamelessNode { .. } => "this node has no name".to_string(),
            Fault::UnknownType { node_type, .. } => {
                format!("\"{node_type}\" is not a type this build can place")
            }
            Fault::DanglingLink { from, to } => {
                format!("\"{from}\" to \"{to}\": one of them is not here")
            }
            Fault::SelfLink { node } => {
                format!("\"{node}\" is linked to itself")
            }
            Fault::InputBoundTwice { node, input } => {
                format!("two links arrive at \"{node}.{input}\"")
            }
            // The reason names the node already: reach_of's complaints are
            // written to be read on their own.
            Fault::BadNode { why, .. } => why.clone(),
            Fault::BadTcspecial { why } => format!("tcspecial: {why}"),
            // The payload first, it being the one that has to change: the
            // sender is named only as what is being sent, so that nobody goes
            // looking at the ground link for the trouble.
            Fault::UntriggeredInput { node, from } => {
                format!(
                    "\"{node}\" states no trigger, so what \"{from}\" sends it \
                     has nowhere to go"
                )
            }
            Fault::NoSuchGroup { group, .. } => {
                format!("there is no group \"{group}\"")
            }
            Fault::EmptyGroup { group } => {
                format!("group \"{group}\" has no nodes in it")
            }
        }
    }

    /// The node this fault should be drawn on, where it belongs to one.
    ///
    /// # Returns
    ///
    /// The node's name, or `None` for a fault about a nameless node, an empty
    /// group or the tcspecial section.
    pub fn node(&self) -> Option<&str> {
        match self {
            Fault::DuplicateName { name } => Some(name),
            Fault::NamelessNode { .. } => None,
            Fault::UnknownType { node, .. } => Some(node),
            Fault::DanglingLink { from, .. } => Some(from),
            Fault::SelfLink { node } => Some(node),
            Fault::InputBoundTwice { node, .. } => Some(node),
            // On the payload and not on the source: the source is right about
            // where it sends, and the payload is where a trigger would go.
            Fault::UntriggeredInput { node, .. } => Some(node),
            Fault::NoSuchGroup { node, .. } => Some(node),
            Fault::EmptyGroup { .. } => None,
            Fault::BadNode { node, .. } => Some(node),
            // Not about a node: the section is about the configuration as a
            // whole, so there is nothing on the canvas to draw it on. The
            // parameters window shows it.
            Fault::BadTcspecial { .. } => None,
        }
    }
}

impl Document {
    pub fn new() -> Self {
        Document::default()
    }

    /// The node this absolute name refers to.
    ///
    /// Absolute: `temp_1` for a node of the top diagram and
    /// `zone/temp_1` for one inside a subdiagram. That is how the editor
    /// holds a name everywhere except in a file, where it is written as the
    /// node's own -- see [`Qualified::written`]. A subdiagram's box in the
    /// top diagram is named by its include, which may be a path such as
    /// `../common/zone`: its `/` says no home, so a box of the whole name is
    /// looked for first.
    ///
    /// # Arguments
    ///
    /// * `name` - The node's absolute name.
    ///
    /// # Returns
    ///
    /// The node, or `None` if there is none of that name.
    pub fn node(&self, name: &str) -> Option<&Node> {
        if let Some(node) = self
            .nodes
            .iter()
            .find(|n| n.home.is_none() && n.name == name)
        {
            return Some(node);
        }
        let wanted = Qualified::parse(name);
        self.nodes
            .iter()
            .find(|n| n.name == wanted.node && n.home == wanted.in_)
    }

    /// Finds the node an absolute name refers to, to be changed.
    ///
    /// # Arguments
    ///
    /// * `name` - The node's absolute name, as for [`Document::node`].
    ///
    /// # Returns
    ///
    /// The node, or `None` if there is none of that name.
    fn node_mut(&mut self, name: &str) -> Option<&mut Node> {
        // A subdiagram's box first, whose name, a path, has a `/` in it
        // that does not say a home: see `Document::node`.
        if let Some(at) = self
            .nodes
            .iter()
            .position(|n| n.home.is_none() && n.name == name)
        {
            return self.nodes.get_mut(at);
        }
        let wanted = Qualified::parse(name);
        self.nodes
            .iter_mut()
            .find(|n| n.name == wanted.node && n.home == wanted.in_)
    }

    /// Every subdiagram the document holds, in the order they first appear.
    ///
    /// Taken from the nodes rather than kept in a list beside them: a list
    /// could disagree with the nodes, and there is nothing a subdiagram is
    /// except where some nodes live.
    ///
    /// # Returns
    ///
    /// The subdiagrams' names.
    pub fn subdiagrams(&self) -> Vec<String> {
        let mut found: Vec<String> = Vec::new();
        for node in &self.nodes {
            if let Some(home) = &node.home {
                if !found.contains(home) {
                    found.push(home.clone());
                }
            }
        }
        // And the boxes that stand for one, which may be there before anything
        // has been put inside.
        for node in &self.nodes {
            if node.node_type == "subdiagram" && !found.contains(&node.name) {
                found.push(node.name.clone());
            }
        }
        // And a file that declares a group or a binding and holds no node,
        // which is still a file the top one includes.
        let declared = self
            .groups
            .keys()
            .filter_map(|group| self.group_homes.get(group))
            .chain(self.commands.iter().filter_map(|c| c.home.as_ref()));
        for home in declared {
            if !found.contains(home) {
                found.push(home.clone());
            }
        }
        found
    }

    /// The groups declared in one file, each with its interval.
    ///
    /// # Arguments
    ///
    /// * `home` - The subdiagram, or `None` for the top diagram.
    ///
    /// # Returns
    ///
    /// The groups, by name.
    fn groups_in(&self, home: &Home) -> Vec<(&String, &String)> {
        self.groups
            .iter()
            .filter(|(name, _)| self.group_homes.get(*name) == home.as_ref())
            .collect()
    }

    /// What a subdiagram's file says it exports, as it is written: each
    /// list in order of name, and only the nodes the file has.
    ///
    /// # Arguments
    ///
    /// * `home` - The subdiagram, or `None` for the top diagram, which has
    ///            nothing outside it to export to.
    ///
    /// # Returns
    ///
    /// The exports, or `None` where there are none to write.
    pub fn exports_written(
        &self,
        home: &Home,
    ) -> Option<tcslibgs::graph::exports::Exports> {
        let said = self.exports.get(home.as_ref()?)?;
        let kept = |nodes: &Vec<String>| {
            let mut kept: Vec<String> = nodes
                .iter()
                .filter(|node| {
                    let absolute = Qualified {
                        in_: home.clone(),
                        node: (*node).clone(),
                    }
                    .absolute();
                    self.node(&absolute).is_some_and(|n| &n.home == home)
                })
                .cloned()
                .collect();
            kept.sort();
            kept.dedup();
            kept
        };
        let written = tcslibgs::graph::exports::Exports {
            world_in: kept(&said.world_in),
            world_out: kept(&said.world_out),
            moc_in: kept(&said.moc_in),
            moc_out: kept(&said.moc_out),
        };
        (!written.is_empty()).then_some(written)
    }

    /// The nodes that live in `home`.
    pub fn nodes_in(&self, home: &Home) -> Vec<&Node> {
        self.nodes.iter().filter(|n| &n.home == home).collect()
    }

    /// One home's nodes of a section, grouped by type, as a file writes
    /// them: the types by name, and each type's nodes by name.
    ///
    /// # Arguments
    ///
    /// * `home`    - The subdiagram, or `None` for the top diagram.
    /// * `section` - The section.
    ///
    /// # Returns
    ///
    /// Each type with its nodes; none for a section with no nodes.
    fn grouped(
        &self,
        home: &Home,
        section: Section,
    ) -> Vec<(&str, Vec<&Node>)> {
        let mut grouped: BTreeMap<&str, Vec<&Node>> = BTreeMap::new();
        for node in self.nodes_in(home) {
            // A subdiagram's box is the editor's own and no node: what a
            // file says of it is its include, and where it was drawn.
            if node.node_type == "subdiagram" {
                continue;
            }
            if section_of(&node.node_type) == section {
                grouped.entry(&node.node_type).or_default().push(node);
            }
        }
        grouped
            .into_iter()
            .map(|(node_type, mut nodes)| {
                nodes.sort_by(|a, b| a.name.cmp(&b.name));
                (node_type, nodes)
            })
            .collect()
    }

    /// What one file says above one line of its structure, if anything.
    ///
    /// # Arguments
    ///
    /// * `home`    - The subdiagram, or `None` for the top diagram.
    /// * `heading` - The line.
    ///
    /// # Returns
    ///
    /// The comment's text, or `None` where nothing is written there.
    fn remark(&self, home: &Home, heading: &Heading) -> Option<&str> {
        self.remarks
            .iter()
            .find(|remark| &remark.home == home && &remark.heading == heading)
            .map(|remark| remark.text.as_str())
    }

    /// What is written above one line of the `tcspecial` section.
    ///
    /// # Arguments
    ///
    /// * `parent`    - The subsection the line is in, empty for the top of
    ///                 the section.
    /// * `attribute` - What the line states: an attribute, or a subsection
    ///                 where `parent` is empty.
    ///
    /// # Returns
    ///
    /// The comment's text, or `None` where nothing is written there.
    fn tcspecial_comment(&self, parent: &str, attribute: &str) -> Option<&str> {
        let key = if parent.is_empty() {
            attribute.to_string()
        } else {
            format!("{parent}.{attribute}")
        };
        self.tcspecial_comments.get(&key).map(String::as_str)
    }

    /// Drops the comments whose section, type group or `tcspecial` line is
    /// no longer in the document.
    ///
    /// What a comment is about is written only while it has something in
    /// it -- see the writers, which leave out an empty section and an empty
    /// group -- so a comment on one the editor has emptied would wait unseen
    /// and come back over whatever was put there next: a `- type serial:`
    /// made again for a different line under a paragraph about the old one.
    /// Gone with what it described is what a person deleting the last node
    /// of a type would expect, and undo does not bring it back, as it does
    /// not bring back a removed node's comment.
    pub fn forget_lost_comments(&mut self) {
        // The files that write an exports section, whose comment is kept.
        let exported: Vec<Home> = self
            .exports
            .keys()
            .map(|home| Some(home.clone()))
            .filter(|home| self.exports_written(home).is_some())
            .collect();
        let nodes = &self.nodes;
        self.remarks.retain(|remark| {
            // What a file is said to be is kept while there is the file,
            // which a save writes whatever is in it.
            if remark.heading == Heading::File {
                return true;
            }
            if remark.heading == Heading::Section("exports".to_string()) {
                return exported.contains(&remark.home);
            }
            nodes.iter().any(|node| {
                node.home == remark.home
                    && match &remark.heading {
                        Heading::Section(name) | Heading::Opening(name) => {
                            section_of(&node.node_type).name() == name
                        }
                        Heading::Type(node_type) => {
                            &node.node_type == node_type
                        }
                        Heading::File => true,
                    }
            })
        });
        let stated: Vec<&String> = self
            .tcspecial
            .iter()
            .filter(|(_, value)| !value.trim().is_empty())
            .map(|(attribute, _)| attribute)
            .collect();
        self.tcspecial_comments.retain(|key, _| {
            stated.iter().any(|attribute| {
                *attribute == key
                    || attribute
                        .strip_prefix(key.as_str())
                        .is_some_and(|rest| rest.starts_with('.'))
            })
        });
    }

    /// One home's nodes, by name: the order the editor's section writes
    /// their places in.
    ///
    /// # Arguments
    ///
    /// * `home` - The subdiagram, or `None` for the top diagram.
    ///
    /// # Returns
    ///
    /// The nodes.
    fn placed_in(&self, home: &Home) -> Vec<&Node> {
        let mut placed = self.nodes_in(home);
        placed.sort_by(|a, b| a.name.cmp(&b.name));
        placed
    }

    /// What was set aside about one home's nodes, and only those: a
    /// decision about a node in a subdiagram belongs in that subdiagram's
    /// file, where the node is.
    ///
    /// # Arguments
    ///
    /// * `home` - The subdiagram, or `None` for the top diagram.
    ///
    /// # Returns
    ///
    /// Each node, as the document holds it, and the concern, in order.
    fn aside_in(&self, home: &Home) -> Vec<&(String, String)> {
        let placed = self.nodes_in(home);
        self.aside
            .iter()
            .filter(|(node, _)| {
                placed.iter().any(|drawn| {
                    Qualified::of(&drawn.name, home).absolute() == *node
                })
            })
            .collect()
    }

    /// The parameters of the section that are actually stated, by the
    /// subsection each belongs to.
    ///
    /// The document holds every attribute the section may have, so that the
    /// window has a field for each; what the file holds is the ones somebody
    /// has answered. Both writers ask this, so neither can come to write a
    /// section the other would not.
    ///
    /// Each entry is a subsection's name and what is under it, or an empty
    /// name and the values at the top of the section. The document holds them
    /// flat and dotted -- one name per field and per complaint -- and this is
    /// where the file's shape comes back. The attributes are sorted, so a
    /// subsection's members are already together.
    ///
    /// # Returns
    ///
    /// Each subsection's name, empty for the top of the section, with the
    /// stated attributes under it and their values.
    fn tcspecial_stated(&self) -> Vec<(String, Vec<(String, String)>)> {
        let mut out: Vec<(String, Vec<(String, String)>)> = Vec::new();
        for (attribute, value) in &self.tcspecial {
            if value.trim().is_empty() {
                continue;
            }
            let (parent, name) = match attribute.split_once('.') {
                Some((parent, child)) => {
                    (parent.to_string(), child.to_string())
                }
                None => (String::new(), attribute.clone()),
            };
            match out.last_mut() {
                Some((had, under)) if *had == parent => {
                    under.push((name, value.clone()))
                }
                _ => out.push((parent, vec![(name, value.clone())])),
            }
        }
        out
    }

    /// Puts every node inside the pane its type belongs to.
    ///
    /// Called when a configuration is read and not at any other time. What it
    /// is for is the file somebody wrote before the canvas was divided, or
    /// wrote by hand, or wrote with the divider somewhere else: a payload
    /// drawn in the vehicle working pane is a drawing that contradicts the
    /// rule the editor otherwise enforces, and a reader has to be told which
    /// of the two to believe.
    ///
    /// This is a change to the document, so the honest thing is to say what it
    /// is not. It is not an edit: nothing goes in the history, because undoing
    /// it would put a node back somewhere the editor refuses to put one. And
    /// it writes nothing by itself, nor does an edit of anything else: a save
    /// writes only what was edited -- see `crate::faithful` -- and a node
    /// settled here is written where it now is when it is moved.
    ///
    /// And on the grid: a node whose upper left corner is not on a grid point
    /// is moved to the nearest one, which is the rule a drag obeys as well --
    /// see `pane::confine`, which does both.
    ///
    /// A node that has to move keeps its height and takes the near edge of its
    /// own pane, which is the smallest move that obeys the rule. Where that
    /// lands it on top of another node it steps down the grid until it is
    /// clear: a payload column read out of a file where the payloads were in a
    /// row would otherwise become one node with eleven behind it.
    ///
    /// # Arguments
    ///
    /// * `dividers` - Where the three working panes meet.
    ///
    /// # Returns
    ///
    /// How many nodes were moved, which is what a caller would log.
    pub fn settle(&mut self, dividers: crate::pane::Dividers) -> usize {
        // The rule is tcsdiagram's, which tcspecial's visual window draws by
        // as well: see pane::settle. Walked in the order of the names, which
        // is the one order a configuration has, so that a file saved and
        // read again is laid out as it was whatever order the session added
        // its nodes in.
        let mut order: Vec<usize> = (0..self.nodes.len()).collect();
        order.sort_by(|a, b| self.nodes[*a].name.cmp(&self.nodes[*b].name));
        let mut placing: Vec<crate::pane::Placing> = order
            .iter()
            .map(|at| crate::pane::Placing {
                node_type: &self.nodes[*at].node_type,
                at: self.nodes[*at].at,
            })
            .collect();
        let moved = crate::pane::settle(&mut placing, dividers);
        let settled: Vec<(i32, i32)> =
            placing.iter().map(|placing| placing.at).collect();
        for (at, place) in order.into_iter().zip(settled) {
            self.nodes[at].at = place;
        }
        moved
    }

    /// What might be wrong, which is not the same as what is wrong.
    ///
    /// A payload whose readings reach nothing, a vehicle node something
    /// arrives at and nothing leaves, one something leaves and nothing arrives
    /// at: none of these is a file that cannot be read, and every one of them
    /// is a diagram somebody has not finished -- or has finished, and meant.
    /// So they are raised rather than refused, counted on a button, and set
    /// aside one at a time by whoever decides they are not problems: see
    /// [`Document::aside`].
    ///
    /// Which rules apply to a node depends on which pane it is in, because the
    /// data flows one way through the panes: a payload is where the payload's
    /// own data begins and a gateway link is where it ends, and the other
    /// stream runs the other way. So a payload's `world-out` with nothing on
    /// it is a concern -- those readings reach nobody -- where a computation's
    /// is not, a computation being free to send its result the other way or to
    /// be read by nothing yet. What is asked of a vehicle node is that a stream
    /// passing through it comes out somewhere, which is the shape of the whole
    /// graph: payload, vehicle nodes, gateway link.
    ///
    /// In the document's own order, so the list a person reads is the list in
    /// the file.
    ///
    /// Nothing at all while there is one node or none. A configuration that
    /// does not exist yet opens with the ground station already in it, and
    /// the moment it is placed every rule here has something to say about it:
    /// nothing is linked to it, because there is nothing to link it to. A
    /// count on a button before anybody has drawn anything is a complaint
    /// about the editor's own first act, and it cannot be acted on until
    /// there is a second node -- so it is not raised. The rules come back as
    /// soon as there is a graph to apply them to.
    ///
    /// # Returns
    ///
    /// The concerns, in the document's order; none for fewer than two nodes.
    pub fn concerns(&self) -> Vec<Concern> {
        if self.nodes.len() < 2 {
            return Vec::new();
        }
        self.concerns_in_a_graph()
    }

    /// The rules themselves, once there is a graph to apply them to.
    ///
    /// # Returns
    ///
    /// The concerns, in the document's order.
    fn concerns_in_a_graph(&self) -> Vec<Concern> {
        let mut concerns = Vec::new();

        for node in &self.nodes {
            let name = Qualified::of(&node.name, &node.home).absolute();
            let Some(found) = NodeType::ALL
                .iter()
                .find(|t| t.spelling() == node.node_type)
            else {
                // A type this build does not know: that is a fault, and
                // `faults` is where it is said.
                continue;
            };
            let terminals = found.terminals();
            let leaves = |terminal: &str| {
                self.links.iter().any(|link| {
                    link.from == name
                        && crate::style::terminal_for(
                            &node.node_type,
                            link.origin.as_deref(),
                            false,
                        ) == Some(terminal)
                })
            };
            let anything_leaves =
                self.links.iter().any(|link| link.from == name);
            let anything_arrives =
                self.links.iter().any(|link| link.to == name);

            match crate::pane::pane_of(&node.node_type) {
                // Where a stream begins: what it sends has to reach
                // something, or the node is a sensor nobody reads.
                crate::pane::Pane::Payloads | crate::pane::Pane::Gateway => {
                    for terminal in terminals.sources {
                        if !leaves(terminal) {
                            concerns.push(Concern::GoesNowhere {
                                node: name.clone(),
                                terminal: (*terminal).to_string(),
                            });
                        }
                    }
                }
                // And in between, a stream has to come out somewhere. A log
                // is the exception by having no data source at all: it is
                // where data stops on purpose.
                crate::pane::Pane::Vehicle => {
                    if anything_arrives
                        && !anything_leaves
                        && !terminals.sources.is_empty()
                    {
                        concerns.push(Concern::NothingLeaves {
                            node: name.clone(),
                        });
                    }
                    if anything_leaves
                        && !anything_arrives
                        && !terminals.sinks.is_empty()
                    {
                        concerns.push(Concern::NothingArrives {
                            node: name.clone(),
                        });
                    }
                }
                // No node of a configuration's is a physics component.
                crate::pane::Pane::Physics => {}
            }
        }

        concerns
    }

    /// The concerns somebody has decided not to act on, and the rest.
    ///
    /// Two lists, because the window shows them on two buttons: what might be
    /// wrong, and what somebody has looked at and left. A concern that stops
    /// applying leaves both lists by itself -- the set-aside is remembered by
    /// node and kind, and a kind that no longer holds is not raised -- so
    /// nothing has to be tidied up after a link is drawn.
    pub fn concerns_apart(&self) -> (Vec<Concern>, Vec<Concern>) {
        self.concerns()
            .into_iter()
            .partition(|concern| !self.aside.contains(&concern.key()))
    }

    /// Which nodes share a name with another node, in the order the file holds
    /// them.
    ///
    /// Every one of them, not the second of each pair: a file whose two nodes
    /// are both called `i2c` does not say which of them was meant, so there is
    /// nothing to prefer, and whoever is putting it right should be offered
    /// both names to change.
    ///
    /// By the whole name, which includes the subdiagram a node lives in: two
    /// pages of one configuration may each have a `temp_1`, and that is not
    /// a duplicate -- see [`Node::home`].
    ///
    /// # Returns
    ///
    /// The indices, in the document's list of nodes, of every node whose
    /// absolute name another node also has.
    pub fn sharing_a_name(&self) -> Vec<usize> {
        let mut count: BTreeMap<&str, usize> = BTreeMap::new();
        let whole: Vec<String> = self
            .nodes
            .iter()
            .map(|node| Qualified::of(&node.name, &node.home).absolute())
            .collect();
        for name in &whole {
            *count.entry(name.as_str()).or_default() += 1;
        }
        whole
            .iter()
            .enumerate()
            .filter(|(_, name)| {
                count.get(name.as_str()).copied().unwrap_or(0) > 1
            })
            .map(|(at, _)| at)
            .collect()
    }

    /// Renames the node in this position.
    ///
    /// By position and not by name, which is the whole point: a name two nodes
    /// share cannot say which of them to rename, and
    /// [`Edit::RenameNode`] -- which renames the first node it finds -- would
    /// rename whichever happened to be read first.
    ///
    /// The links follow only if the old name is left with nobody holding it.
    /// A link names a name: while two nodes shared one, every link naming it
    /// was ambiguous, and renaming one of the two does not make the link mean
    /// the other -- it leaves it meaning the node that kept the name. When the
    /// last holder is renamed there is no ambiguity left, and the links go
    /// with it as they do for any other rename.
    ///
    /// Nothing happens for a position that is not there or a name that is
    /// already this node's.
    ///
    /// # Arguments
    ///
    /// * `at`  - The node's index in the document's list of nodes.
    /// * `now` - Its new name, within its own subdiagram.
    pub fn rename_at(&mut self, at: usize, now: &str) {
        let Some(node) = self.nodes.get(at) else {
            return;
        };
        let was = Qualified::of(&node.name, &node.home).absolute();
        let home = node.home.clone();
        let whole = Qualified::of(now, &home).absolute();
        if whole == was {
            return;
        }
        self.nodes[at].name = now.to_string();

        let shared = self.nodes.iter().any(|node| {
            Qualified::of(&node.name, &node.home).absolute() == was
        });
        if shared {
            return;
        }
        for link in &mut self.links {
            if link.from == was {
                link.from = whole.clone();
            }
            if link.to == was {
                link.to = whole.clone();
            }
        }
        // And the simulator's entry for it, where there is one: the two files
        // are joined by name, so a name changed in one of them has to change
        // in the other or the pair stops being a pair. By the node's own
        // name, which is how the simulator's file keys it wherever the node
        // is drawn. See `crate::sim::Simulator::key_of`.
        if let Some(simulator) = &mut self.simulator {
            simulator.rename(&crate::sim::Simulator::key_of(&was), now);
        }
    }

    /// A name no node has, derived from one that is taken.
    ///
    /// The name with a hyphen and the first free number, or the next free
    /// number where it already ends in one: `i2c` becomes `i2c-1`, and `i2c-1`
    /// becomes `i2c-2` rather than `i2c-1-1`. The same shape of name the editor
    /// gives a node it places -- see `tcsgui::fresh_name` -- so a derived name
    /// looks like a name and not like a repair.
    ///
    /// Within the home of the node at `at`, since that is where it has to be
    /// free.
    ///
    /// # Arguments
    ///
    /// * `at` - The index, in the document's list of nodes, of the node whose
    ///          name is taken.
    ///
    /// # Returns
    ///
    /// The name, such as `i2c-2`; empty if there is no node at `at`.
    ///
    /// # Panics
    ///
    /// Never in practice: the search for a free number cannot run out before
    /// the nodes do.
    pub fn spare_name(&self, at: usize) -> String {
        let Some(node) = self.nodes.get(at) else {
            return String::new();
        };
        let home = node.home.clone();
        // Whatever it is called, without a number on the end: a file with
        // `i2c-1` twice should offer `i2c-2`, not `i2c-1-1`.
        let stem = match node.name.rsplit_once('-') {
            Some((stem, number))
                if !number.is_empty()
                    && number.bytes().all(|b| b.is_ascii_digit()) =>
            {
                stem
            }
            _ => node.name.as_str(),
        };
        let stem = if stem.is_empty() {
            node.node_type.as_str()
        } else {
            stem
        };
        let taken = |name: &str| {
            let whole = Qualified::of(name, &home).absolute();
            self.nodes.iter().any(|node| {
                Qualified::of(&node.name, &node.home).absolute() == whole
            })
        };
        (1..)
            .map(|n| format!("{stem}-{n}"))
            .find(|name| !taken(name))
            .expect("a free name exists")
    }

    /// The edit that takes this node away, with everything that goes with it.
    ///
    /// A removal carries the node's own words and every line that touches it,
    /// at either end, because that is what undo has to put back: the links go
    /// when the node does -- a line to a node that is not there would dangle
    /// -- and an undo that restored the node without them would be an undo
    /// that did half of what was undone.
    ///
    /// # Arguments
    ///
    /// * `name` - The node's absolute name.
    ///
    /// # Returns
    ///
    /// The removal, or `None` for a node that is not here, which is a window
    /// asking about one somebody has already deleted.
    pub fn removal_of(&self, name: &str) -> Option<Edit> {
        let node = self.node(name)?;
        Some(Edit::RemoveNode {
            name: node.name.clone(),
            node_type: node.node_type.clone(),
            attributes: node
                .attributes
                .iter()
                .map(|(attribute, value)| (attribute.clone(), value.clone()))
                .collect(),
            links: self
                .links
                .iter()
                .filter(|link| link.from == name || link.to == name)
                .cloned()
                .collect(),
            home: node.home.clone(),
        })
    }

    /// How many lines touch this node, which is what a question about
    /// deleting it has to say.
    ///
    /// # Arguments
    ///
    /// * `name` - The node's absolute name.
    ///
    /// # Returns
    ///
    /// How many links start or end at it.
    pub fn lines_touching(&self, name: &str) -> usize {
        self.links
            .iter()
            .filter(|link| link.from == name || link.to == name)
            .count()
    }

    /// Where every node would go if the diagram were laid out afresh.
    ///
    /// What the **Arrange** button and **Edit -> Arrange the nodes** do. Each
    /// page on its own, each node in its own pane, laid out the way the data
    /// flows: the payloads in a column down the left of theirs, the vehicle's
    /// nodes in columns by how many steps from a payload each is -- what a
    /// conversion feeds standing to the right of the conversion -- and the
    /// ground in a column of its own. Within a column the nodes are in the
    /// order of what feeds them, so the lines between two columns cross as
    /// little as they can, and each node is level with what feeds it where
    /// there is room, so a line between them runs straight. A pane too narrow
    /// for every step of the vehicle's graph holds the later steps in its last
    /// column.
    ///
    /// All of it from the top of the sheet, which is where the view starts,
    /// clear of the pane names and of each other, on the node grid. It is
    /// what a person reaches for when a configuration read out of a file is
    /// spread over more sheet than the window shows, or lies outside the
    /// panes the window has, or when a morning's dragging has left it in a
    /// state nobody can read.
    ///
    /// Only the nodes that would move come back, each with where it would go:
    /// that is what [`Edit::Arrange`] carries, and a node already in the right
    /// place does not belong in an undo.
    ///
    /// # Arguments
    ///
    /// * `dividers` - Where the working panes meet, which says how wide each
    ///                pane is.
    ///
    /// # Returns
    ///
    /// The name of each node that would move, as the editor holds it, with
    /// its new upper left corner.
    pub fn arranged(
        &self,
        dividers: crate::pane::Dividers,
    ) -> Vec<(String, (i32, i32))> {
        let mut homes: Vec<&Home> = Vec::new();
        for node in &self.nodes {
            if !homes.contains(&&node.home) {
                homes.push(&node.home);
            }
        }
        homes
            .into_iter()
            .flat_map(|home| self.arranged_page(home, dividers))
            .collect()
    }

    /// Where every node of one page would go: see [`Document::arranged`].
    ///
    /// # Arguments
    ///
    /// * `home`     - The page.
    /// * `dividers` - Where the working panes meet.
    ///
    /// # Returns
    ///
    /// Each node of the page that would move, by its whole name, with where
    /// it would go.
    fn arranged_page(
        &self,
        home: &Home,
        dividers: crate::pane::Dividers,
    ) -> Vec<(String, (i32, i32))> {
        use crate::pane::{edges_of, node_after, node_ceil, pane_of, Pane};

        // By name, the one order a configuration has, so that the same
        // nodes are arranged alike however the session came to hold them.
        let mut nodes: Vec<&Node> = self.nodes_in(home);
        nodes.sort_by(|a, b| a.name.cmp(&b.name));
        let named: Vec<String> = nodes
            .iter()
            .map(|node| Qualified::of(&node.name, &node.home).absolute())
            .collect();
        let index = |name: &str| named.iter().position(|n| n == name);

        // What feeds what on this page, by index: every line, either
        // stream, and apart from it the readings' lines alone, which are
        // what says how many steps from a payload a vehicle node is. The
        // commands' lines run back the other way, and counting them would
        // make every loop through a thermostat a graph with no first step.
        let mut feeds: Vec<(usize, usize)> = Vec::new();
        let mut readings: Vec<(usize, usize)> = Vec::new();
        for link in &self.links {
            if let (Some(from), Some(to)) = (index(&link.from), index(&link.to))
            {
                if from == to {
                    continue;
                }
                feeds.push((from, to));
                if self.arrives_at(link) == Some("world-in") {
                    readings.push((from, to));
                }
            }
        }

        // The vehicle's steps: nought for a node nothing aboard feeds, and
        // one more than the furthest of what feeds it. As many passes as
        // there are nodes, which is as long as any path can be; a loop of
        // readings, which the rules refuse, stops there rather than forever.
        let panes: Vec<Pane> =
            nodes.iter().map(|node| pane_of(&node.node_type)).collect();
        let mut step = vec![0usize; nodes.len()];
        for _ in 0..nodes.len() {
            let mut changed = false;
            for &(from, to) in &readings {
                if panes[from] == Pane::Vehicle
                    && panes[to] == Pane::Vehicle
                    && step[to] < step[from] + 1
                {
                    step[to] = step[from] + 1;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        // The columns, left to right: the payloads, the vehicle's as many as
        // its pane holds, and the ground. A column is a pane, and where in
        // it the column's nodes stand.
        let width = crate::style::WIDTH as i32;
        let gap = node_after(node_ceil(0)) - node_ceil(0);
        let (left, right) = edges_of(Pane::Vehicle, dividers);
        // A gap before each column, the first's being the pane's margin: so
        // the last column's right edge is the left plus a gap and a width for
        // each, which is what has to be inside the pane.
        let room = ((right - left) / (width + gap)).max(1) as usize;
        let column_of = |at: usize| -> usize {
            match panes[at] {
                Pane::Physics | Pane::Payloads => 0,
                Pane::Vehicle => 1 + step[at].min(room - 1),
                Pane::Gateway => room + 1,
            }
        };
        let columns = room + 2;
        let x_of = |at: usize, column: usize| -> i32 {
            let node_type = &nodes[at].node_type;
            let (left, _) = edges_of(panes[at], dividers);
            let within = match panes[at] {
                Pane::Vehicle => (column - 1) as i32 * (width + gap),
                _ => 0,
            };
            crate::pane::confine(node_type, dividers, (left + gap + within, 0))
                .0
        };

        // The order down each column: what feeds a node, on average, where
        // it stands in the columns to its left; the payloads, which nothing
        // feeds, in the order of what they feed, once that is known. Ties by
        // name, which is the order they came in.
        let mut row = vec![0f32; nodes.len()];
        let mut order: Vec<Vec<usize>> = vec![Vec::new(); columns];
        for at in 0..nodes.len() {
            order[column_of(at)].push(at);
        }
        for (i, &at) in order[0].iter().enumerate() {
            row[at] = i as f32;
        }
        let mean = |of: &[f32]| -> Option<f32> {
            (!of.is_empty()).then(|| of.iter().sum::<f32>() / of.len() as f32)
        };
        for pass in 0..2 {
            for (column, members) in order.iter_mut().enumerate().skip(1) {
                let mut ranked: Vec<(f32, usize)> = members
                    .iter()
                    .map(|&at| {
                        let fed: Vec<f32> = feeds
                            .iter()
                            .filter(|&&(from, to)| {
                                to == at && column_of(from) < column
                            })
                            .map(|&(from, _)| row[from])
                            .collect();
                        (mean(&fed).unwrap_or(row[at]), at)
                    })
                    .collect();
                ranked.sort_by(|a, b| a.0.total_cmp(&b.0));
                *members = ranked.iter().map(|&(_, at)| at).collect();
                for (i, &at) in members.iter().enumerate() {
                    row[at] = i as f32;
                }
            }
            // And the payloads by what they feed, once, so that the second
            // pass orders the rest against them.
            if pass == 0 {
                let mut ranked: Vec<(f32, usize)> = order[0]
                    .iter()
                    .map(|&at| {
                        let fed: Vec<f32> = feeds
                            .iter()
                            .filter(|&&(from, to)| {
                                from == at && column_of(to) > 0
                            })
                            .map(|&(_, to)| row[to])
                            .collect();
                        (mean(&fed).unwrap_or(row[at]), at)
                    })
                    .collect();
                ranked.sort_by(|a, b| a.0.total_cmp(&b.0));
                order[0] = ranked.iter().map(|&(_, at)| at).collect();
                for (i, &at) in order[0].iter().enumerate() {
                    row[at] = i as f32;
                }
            }
        }

        // Down each column, left to right: each node level with what feeds
        // it where there is room, and otherwise under the one above it. The
        // first place is under the pane's name; the next is a step of the
        // grid below the node above.
        let top = node_ceil(crate::pane::NAMES.1);
        let mut at_y: Vec<Option<i32>> = vec![None; nodes.len()];
        let place =
            |order: &[usize],
             at_y: &mut Vec<Option<i32>>,
             wanted: &dyn Fn(usize) -> Option<i32>| {
                let mut next = top;
                for &at in order {
                    let y = wanted(at)
                        .map(|y| crate::pane::on_node_grid(y).max(next))
                        .unwrap_or(next);
                    at_y[at] = Some(y);
                    let (_, height) = size_of(&nodes[at].node_type);
                    next = node_ceil(y + height as i32 + gap);
                }
            };
        for (column, members) in order.iter().enumerate() {
            let placed = at_y.clone();
            let wanted = |at: usize| -> Option<i32> {
                let fed: Vec<f32> = feeds
                    .iter()
                    .filter(|&&(from, to)| to == at && column_of(from) < column)
                    .filter_map(|&(from, _)| placed[from].map(|y| y as f32))
                    .collect();
                mean(&fed).map(|y| y.round() as i32)
            };
            place(members, &mut at_y, &wanted);
        }
        // And the payloads again, level with what they feed now that it has
        // a place: a column of sensors each beside its own conversion.
        let placed = at_y.clone();
        let wanted = |at: usize| -> Option<i32> {
            let fed: Vec<f32> = feeds
                .iter()
                .filter(|&&(from, to)| from == at && column_of(to) > 0)
                .filter_map(|&(_, to)| placed[to].map(|y| y as f32))
                .collect();
            mean(&fed).map(|y| y.round() as i32)
        };
        place(&order[0], &mut at_y, &wanted);

        let mut moved = Vec::new();
        for (at, node) in nodes.iter().enumerate() {
            let now = (x_of(at, column_of(at)), at_y[at].unwrap_or(top));
            if now != node.at {
                moved.push((named[at].clone(), now));
            }
        }
        moved
    }

    /// How many of a page's nodes lie outside their own pane, across.
    ///
    /// Wholly or partly: a node overhanging its pane's edge is a node half
    /// of which may be off the side of the window, and one in another pane
    /// is a node drawn where its type does not belong. What the Arrange
    /// button is emphasised for. Down the sheet nothing is outside -- the
    /// sheet scrolls.
    ///
    /// # Arguments
    ///
    /// * `home`     - The page.
    /// * `dividers` - Where the working panes meet.
    ///
    /// # Returns
    ///
    /// How many.
    pub fn outside_panes(
        &self,
        home: &Home,
        dividers: crate::pane::Dividers,
    ) -> usize {
        self.nodes_in(home)
            .iter()
            .filter(|node| {
                let (left, right) = crate::pane::edges_of(
                    crate::pane::pane_of(&node.node_type),
                    dividers,
                );
                let (width, _) = size_of(&node.node_type);
                node.at.0 < left || node.at.0 + width as i32 > right
            })
            .count()
    }

    /// Where each of these nodes is now, for the undo of an arrangement.
    ///
    /// # Arguments
    ///
    /// * `named` - The nodes, by name, with where they would go; where they
    ///             would go is not used.
    ///
    /// # Returns
    ///
    /// Each of those nodes that is here, with its upper left corner now.
    pub fn where_each_is(
        &self,
        named: &[(String, (i32, i32))],
    ) -> Vec<(String, (i32, i32))> {
        named
            .iter()
            .filter_map(|(name, _)| {
                self.node(name).map(|node| (name.clone(), node.at))
            })
            .collect()
    }

    /// The payloads that are in `group`.
    ///
    /// Payloads and not every node: a group is the schedule a set of payloads
    /// is read on, and it is what the vehicle carries that gets read. A
    /// computation or a log naming a group is an oddity the configuration
    /// checker has words for; it is not something to set a bus address on.
    ///
    /// # Arguments
    ///
    /// * `group` - The group's name.
    ///
    /// # Returns
    ///
    /// The payload nodes whose `group` attribute names it, in the document's
    /// order.
    pub fn payloads_of(&self, group: &str) -> Vec<&Node> {
        self.nodes
            .iter()
            .filter(|node| in_group(node, group) && is_payload(&node.node_type))
            .collect()
    }

    /// The same, to be changed.
    fn payloads_of_mut<'a>(
        &'a mut self,
        group: &'a str,
    ) -> impl Iterator<Item = &'a mut Node> {
        self.nodes.iter_mut().filter(move |node| {
            in_group(node, group) && is_payload(&node.node_type)
        })
    }

    /// What a group may set on its payloads, and what they say now.
    ///
    /// Every attribute that *all* of the group's payloads declare, which is
    /// what makes setting one from the group meaningful: an attribute only
    /// half of them have is a value that would mean nothing on the rest. For a
    /// group of one kind that is most of its type's attributes; for a mixed
    /// group it is the few they share -- the path, the read size, the trigger.
    ///
    /// Two attributes are left out whatever the types say. `group` is the
    /// group itself, and a field that renamed every member's group one node at
    /// a time is a different thing from renaming the group. `interval` belongs
    /// to the group and not to its members -- a member stating one is a fault
    /// the flight reader refuses -- so it is offered as the group's own value
    /// and never propagated.
    ///
    /// # Arguments
    ///
    /// * `group` - The group's name.
    ///
    /// # Returns
    ///
    /// One field per shared attribute, in the first payload's type's order;
    /// none for a group with no payloads.
    pub fn group_fields(&self, group: &str) -> Vec<GroupField> {
        let payloads = self.payloads_of(group);
        if payloads.is_empty() {
            return Vec::new();
        }

        // The attributes the first payload's type declares, in its own order,
        // kept only while every other payload's type declares them too.
        let declared = |node: &Node| -> Vec<&'static str> {
            NodeType::ALL
                .iter()
                .find(|t| t.spelling() == node.node_type)
                .map(|t| t.attributes().iter().map(|(name, _)| *name).collect())
                .unwrap_or_default()
        };

        let mut shared: Vec<&'static str> = declared(payloads[0])
            .into_iter()
            .filter(|name| !matches!(*name, "group" | "interval"))
            .collect();
        for node in &payloads[1..] {
            let theirs = declared(node);
            shared.retain(|name| theirs.contains(name));
        }

        shared
            .into_iter()
            .map(|attribute| {
                let mut values = payloads.iter().map(|node| {
                    node.attributes.get(attribute).map(|v| v.trim().to_string())
                });
                let first = values.next().flatten();
                let agreed = values.all(|value| value == first);
                GroupField {
                    attribute: attribute.to_string(),
                    value: if agreed { first.clone() } else { None },
                    differs: !agreed,
                }
            })
            .collect()
    }

    /// How many links cross into or out of `subdiagram`.
    ///
    /// What a closed box shows instead of its contents: the count of nodes
    /// inside it and the count of connections that cross its edge are the two
    /// things worth knowing about a diagram you are not looking at.
    ///
    /// # Arguments
    ///
    /// * `subdiagram` - The subdiagram's name.
    ///
    /// # Returns
    ///
    /// How many links have one end inside it and the other outside.
    pub fn crossings(&self, subdiagram: &str) -> usize {
        let inside = |name: &str| {
            Qualified::parse(name).in_.as_deref() == Some(subdiagram)
        };
        self.links
            .iter()
            .filter(|l| inside(&l.from) != inside(&l.to))
            .count()
    }

    /// Applies an edit.
    ///
    /// Every edit is applied the same way whether it came from a person, from
    /// an undo, or from replaying a journal after a crash. That is what makes
    /// the three agree: there is one implementation of what an edit means, so
    /// a document rebuilt from its history is the document that was there.
    ///
    /// An edit that cannot be applied -- setting an attribute of a node that
    /// is not here -- is ignored rather than refused. It is reachable only
    /// through a journal that disagrees with its document, which is a damaged
    /// pair of files, and the useful response to that is to recover as much as
    /// can be recovered.
    ///
    /// # Arguments
    ///
    /// * `edit` - The edit to apply.
    pub fn apply(&mut self, edit: &Edit) {
        match edit {
            Edit::AddNode {
                name,
                node_type,
                home,
            } => {
                self.nodes.push(Node {
                    name: name.clone(),
                    node_type: node_type.clone(),
                    attributes: BTreeMap::new(),
                    at: (0, 0),
                    comment: None,
                    home: home.clone(),
                });
            }
            Edit::RemoveNode { name, .. } => {
                self.nodes.retain(|n| &n.name != name);
                // A link to a node that is gone would be a dangling link, so
                // the links go with it. The removal carries them, which is how
                // undo puts them back.
                self.links.retain(|l| &l.from != name && &l.to != name);
            }
            Edit::RestoreNode {
                name,
                node_type,
                attributes,
                links,
                home,
            } => {
                self.nodes.push(Node {
                    name: name.clone(),
                    node_type: node_type.clone(),
                    attributes: attributes.iter().cloned().collect(),
                    at: (0, 0),
                    comment: None,
                    home: home.clone(),
                });
                for link in links {
                    if !self.links.iter().any(|l| l.same_connection(link)) {
                        self.links.push(link.clone());
                    }
                }
            }
            Edit::SetAttribute {
                node,
                attribute,
                now,
                ..
            } => {
                if let Some(node) = self.node_mut(node) {
                    match now {
                        Some(value) => {
                            node.attributes
                                .insert(attribute.clone(), value.clone());
                        }
                        // Absent and empty are different things, so removing
                        // is removing.
                        None => {
                            node.attributes.remove(attribute);
                        }
                    }
                }
            }
            Edit::RenameNode { was, now } => {
                if let Some(node) = self.node_mut(was) {
                    node.name = now.clone();
                }
                // The links refer by name, so they are renamed with it.
                for link in &mut self.links {
                    if &link.from == was {
                        link.from = now.clone();
                    }
                    if &link.to == was {
                        link.to = now.clone();
                    }
                }
                // And so does the simulator's own file, which is joined to
                // the configuration by the payload's name and by nothing
                // else: a rename that left its entry behind would leave a set
                // of settings naming a payload that is not there, which the
                // next session refuses the file for. By the node's own name,
                // as the file keys it: see `crate::sim::Simulator::key_of`.
                if let Some(simulator) = &mut self.simulator {
                    let key_of = crate::sim::Simulator::key_of;
                    simulator.rename(&key_of(was), &key_of(now));
                }
                // And its own file's exports, which list it by its own name.
                let (was, now) = (Qualified::parse(was), Qualified::parse(now));
                if let Some(exports) =
                    was.in_.as_ref().and_then(|home| self.exports.get_mut(home))
                {
                    for list in [
                        &mut exports.world_in,
                        &mut exports.world_out,
                        &mut exports.moc_in,
                        &mut exports.moc_out,
                    ] {
                        for said in list.iter_mut() {
                            if *said == was.node {
                                said.clone_from(&now.node);
                            }
                        }
                    }
                }
            }
            Edit::AddLink { link } => {
                // By connection and not by equality: adding a link that is
                // already there with a comment on it must not make a second,
                // commentless copy of the same connection.
                if !self.links.iter().any(|l| l.same_connection(link)) {
                    self.links.push(link.clone());
                }
            }
            Edit::RemoveLink { link } => {
                self.links.retain(|l| !l.same_connection(link));
            }
            // One end of a line, put somewhere else. The old connection goes
            // and the new one takes its place -- in place, so that a link
            // keeps its position in the file and a save after a re-route is a
            // one-line diff.
            Edit::MoveLink { was, now } => {
                let at = self.links.iter().position(|l| l.same_connection(was));
                match at {
                    Some(at) => self.links[at] = now.clone(),
                    // Not there to move, which an undo of an undo can ask
                    // for: added rather than refused, so the document ends up
                    // saying what the edit says.
                    None => {
                        if !self.links.iter().any(|l| l.same_connection(now)) {
                            self.links.push(now.clone());
                        }
                    }
                }
            }
            Edit::MoveNode { node, now, .. } => {
                if let Some(node) = self.node_mut(node) {
                    node.at = *now;
                }
            }
            // A node shown or hidden at its subdiagram's edge: see
            // `Document::exports`. Showing one shown already, or hiding one
            // hidden, leaves the list as it is.
            Edit::SetExport {
                home,
                terminal,
                node,
                exported,
            } => {
                let exports = self.exports.entry(home.clone()).or_default();
                let list = match terminal.as_str() {
                    "world-in" => &mut exports.world_in,
                    "world-out" => &mut exports.world_out,
                    "moc-in" => &mut exports.moc_in,
                    "moc-out" => &mut exports.moc_out,
                    _ => return,
                };
                list.retain(|said| said != node);
                if *exported {
                    list.push(node.clone());
                    list.sort();
                }
                if exports.is_empty() {
                    self.exports.remove(home);
                }
            }
            // A decision about a concern, remembered by node and kind: see
            // `Document::aside`.
            Edit::SetAside { node, kind, aside } => {
                let key = (node.clone(), kind.clone());
                if *aside {
                    self.aside.insert(key);
                } else {
                    self.aside.remove(&key);
                }
            }
            // One edit for a whole layout: see `Edit::Arrange`. A name that is
            // not here any more is passed over, as a move of a node that is
            // gone would be.
            Edit::Arrange { now, .. } => {
                for (name, at) in now {
                    if let Some(node) = self.node_mut(name) {
                        node.at = *at;
                    }
                }
            }
            // Each in turn, as if each had been made: see `Edit::Group`.
            Edit::Group { edits } => {
                for edit in edits {
                    self.apply(edit);
                }
            }
            Edit::SetTcspecial { attribute, now, .. } => match now {
                Some(value) => {
                    self.tcspecial.insert(attribute.clone(), value.clone());
                }
                // Cleared rather than emptied: absent and empty are the same
                // thing to the checking, and a cleared field should leave the
                // file as somebody would have written it.
                None => {
                    self.tcspecial.remove(attribute);
                }
            },
            Edit::SetSimPayload {
                payload,
                setting,
                now,
                ..
            } => {
                if let Some(simulator) = &mut self.simulator {
                    let said =
                        simulator.payloads.entry(payload.clone()).or_default();
                    match now {
                        Some(value) => {
                            said.insert(setting.clone(), value.clone());
                        }
                        None => {
                            said.remove(setting);
                        }
                    }
                }
            }
            Edit::SetSimGroup {
                group,
                setting,
                now,
                ..
            } => {
                if let Some(simulator) = &mut self.simulator {
                    // Only a group that is there: a setting on a group
                    // somebody has since removed is an edit with nothing to
                    // change, and making the entry here would put the group
                    // back.
                    if let Some(said) = simulator.groups.get_mut(group) {
                        match now {
                            Some(value) => {
                                said.insert(setting.clone(), value.clone());
                            }
                            None => {
                                said.remove(setting);
                            }
                        }
                    }
                }
            }
            Edit::AddSimGroup { name } => {
                if let Some(simulator) = &mut self.simulator {
                    simulator.groups.entry(name.clone()).or_default();
                }
            }
            Edit::RemoveSimGroup { name, .. } => {
                if let Some(simulator) = &mut self.simulator {
                    simulator.groups.remove(name);
                }
            }
            Edit::RestoreSimGroup { name, settings } => {
                if let Some(simulator) = &mut self.simulator {
                    simulator.groups.insert(
                        name.clone(),
                        settings.iter().cloned().collect(),
                    );
                }
            }
            Edit::AddPhysics { name, kind } => {
                if let Some(simulator) = &mut self.simulator {
                    simulator.world.components.insert(
                        name.clone(),
                        crate::sim::PhysicsComponent {
                            kind: kind.clone(),
                            settings: Default::default(),
                        },
                    );
                }
            }
            Edit::RemovePhysics { name, joins, .. } => {
                if let Some(simulator) = &mut self.simulator {
                    let world = &mut simulator.world;
                    for joined in world.connections_of(name) {
                        world.connections.remove(&joined);
                    }
                    world.components.remove(name);
                    // Each stand-in keeps the terminals of other components
                    // it named, in their order, and loses its connects where
                    // that was all it named.
                    let mine = format!("{name}.");
                    for (payload, was) in joins {
                        let kept: Vec<&str> =
                            tcslibgs::sim_config::terminals_of(was)
                                .filter(|said| !said.starts_with(&mine))
                                .collect();
                        let said = simulator
                            .payloads
                            .entry(payload.clone())
                            .or_default();
                        if kept.is_empty() {
                            said.remove("connects");
                        } else {
                            said.insert("connects".into(), kept.join(", "));
                        }
                    }
                }
            }
            Edit::RestorePhysics {
                name,
                kind,
                settings,
                connections,
                joins,
            } => {
                if let Some(simulator) = &mut self.simulator {
                    for (payload, was) in joins {
                        simulator
                            .payloads
                            .entry(payload.clone())
                            .or_default()
                            .insert("connects".into(), was.clone());
                    }
                    let world = &mut simulator.world;
                    world.components.insert(
                        name.clone(),
                        crate::sim::PhysicsComponent {
                            kind: kind.clone(),
                            settings: settings.iter().cloned().collect(),
                        },
                    );
                    world.connections.extend(connections.iter().cloned());
                }
            }
            Edit::SetPhysics {
                component,
                setting,
                now,
                ..
            } => {
                // Only a component that is there, as for a group: making the
                // entry here would put a removed component back.
                if let Some(said) = self
                    .simulator
                    .as_mut()
                    .and_then(|s| s.world.components.get_mut(component))
                {
                    match now {
                        Some(value) => {
                            said.settings
                                .insert(setting.clone(), value.clone());
                        }
                        None => {
                            said.settings.remove(setting);
                        }
                    }
                }
            }
            Edit::MovePhysics { component, now, .. } => {
                if let Some(simulator) = &mut self.simulator {
                    let placed = &mut simulator.world.placed;
                    match now {
                        Some(at) => {
                            placed.insert(component.clone(), *at);
                        }
                        None => {
                            placed.remove(component);
                        }
                    }
                }
            }
            Edit::AddConnection { from, to } => {
                if let Some(simulator) = &mut self.simulator {
                    simulator
                        .world
                        .connections
                        .insert((from.clone(), to.clone()));
                }
            }
            Edit::RemoveConnection { from, to } => {
                if let Some(simulator) = &mut self.simulator {
                    simulator
                        .world
                        .connections
                        .remove(&(from.clone(), to.clone()));
                }
            }
            Edit::AddGroup { name, interval } => {
                self.groups.insert(name.clone(), interval.clone());
            }
            Edit::RemoveGroup { name, .. } => {
                self.groups.remove(name);
            }
            Edit::SetGroupInterval { group, now, .. } => {
                // Only a group that is there: an interval set on a group
                // somebody has since removed is an edit with nothing to
                // change, and inserting one here would put the group back.
                if let Some(interval) = self.groups.get_mut(group) {
                    *interval = now.clone();
                }
            }
            Edit::RenameGroup { was, now } => {
                if let Some(interval) = self.groups.remove(was) {
                    self.groups.insert(now.clone(), interval);
                }
                // In the file it was declared in, under its new name.
                if let Some(home) = self.group_homes.remove(was) {
                    self.group_homes.insert(now.clone(), home);
                }
                // And every node that named it. A node's group is how it gets
                // its interval, so a rename that left them behind would be a
                // rename that silently unscheduled a dozen sensors.
                for node in &mut self.nodes {
                    if node.attributes.get("group").map(|g| g.trim())
                        == Some(was.as_str())
                    {
                        node.attributes
                            .insert("group".to_string(), now.clone());
                    }
                }
            }
            Edit::SetGroupAttribute {
                group,
                attribute,
                now,
                ..
            } => {
                for node in self.payloads_of_mut(group) {
                    match now {
                        Some(value) => {
                            node.attributes
                                .insert(attribute.clone(), value.clone());
                        }
                        // An empty field clears the attribute rather than
                        // setting it to nothing: absent and empty are
                        // different things to a configuration.
                        None => {
                            node.attributes.remove(attribute);
                        }
                    }
                }
            }
            Edit::RestoreGroupAttribute { attribute, now, .. } => {
                // Each node's own value, by name: the group may have changed
                // since, and what is being put back is what these nodes had.
                for (name, value) in now {
                    let Some(node) = self.node_mut(name) else {
                        continue;
                    };
                    match value {
                        Some(value) => {
                            node.attributes
                                .insert(attribute.clone(), value.clone());
                        }
                        None => {
                            node.attributes.remove(attribute);
                        }
                    }
                }
            }
            Edit::SetNodeComment { node, now, .. } => {
                if let Some(node) = self.node_mut(node) {
                    node.comment = now.clone();
                }
            }
            Edit::SetLinkComment { link, now, .. } => {
                if let Some(existing) =
                    self.links.iter_mut().find(|l| l.same_connection(link))
                {
                    existing.comment = now.clone();
                }
            }
        }

        // Whatever the edit emptied takes its comment with it.
        self.forget_lost_comments();
    }

    /// Everything wrong with the document as it stands.
    ///
    /// Empty means it could be flown, as far as the editor can tell: the
    /// checks a graph needs that depend on a node type's own rules -- whether a
    /// log's sources suit its format, whether every input of a mean is bound --
    /// belong to the flight-side validation and are reported by `tcsverify`,
    /// which the editor runs rather than reimplements.
    ///
    /// # Returns
    ///
    /// The faults, each attached to the node or link it is about.
    pub fn faults(&self) -> Vec<Fault> {
        let mut faults = Vec::new();

        // By the name a link would have to use, not by the bare one: two
        // subdiagrams may each hold a temp_1, and they are two nodes
        // because zone/temp_1 and bay/temp_1 are two names. Counting
        // bare names here would refuse the reuse a separate file is for.
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        for node in &self.nodes {
            if node.name.trim().is_empty() {
                faults.push(Fault::NamelessNode { at: node.at });
                continue;
            }
            *seen
                .entry(Qualified::of(&node.name, &node.home).absolute())
                .or_insert(0) += 1;

            match NodeType::ALL
                .iter()
                .find(|t| t.spelling() == node.node_type)
            {
                None => faults.push(Fault::UnknownType {
                    node: node.name.clone(),
                    node_type: node.node_type.clone(),
                }),
                // What is entered has to be right; what is not entered is
                // not wrong yet. The editor is lazy on purpose -- an editor
                // that would not let you leave a field empty is one that
                // fights a person working out what they are describing -- and
                // the rules about what a type must say are applied on the
                // ground and aboard, where the complaint can name the file and
                // the line.
                //
                // So the type's own rules are run only once everything it must
                // have is there. Until then there is nothing to say that would
                // not be "this is unfinished", which the person at the
                // keyboard already knows.
                Some(node_type) => {
                    let stated = |attribute: &str| {
                        node.attributes
                            .get(attribute)
                            .map(|value| value.trim())
                            .filter(|value| !value.is_empty())
                    };
                    let filled = node_type
                        .attributes()
                        .iter()
                        .filter(|(_, required)| *required)
                        .all(|(attribute, _)| stated(attribute).is_some());

                    if filled {
                        if let Err(why) = reach_of_node(node) {
                            faults.push(Fault::BadNode {
                                node: Qualified::of(&node.name, &node.home)
                                    .absolute(),
                                why,
                            });
                        }
                    }
                }
            }
        }
        for (name, count) in seen {
            if count > 1 {
                faults.push(Fault::DuplicateName { name });
            }
        }

        // A node's group must be declared, and a declared group must hold
        // something.
        for node in &self.nodes {
            let Some(group) = node.attributes.get("group") else {
                continue;
            };
            if !self.groups.contains_key(group.trim()) {
                faults.push(Fault::NoSuchGroup {
                    node: Qualified::of(&node.name, &node.home).absolute(),
                    group: group.clone(),
                });
            }
        }
        for group in self.groups.keys() {
            let used = self.nodes.iter().any(|n| {
                n.attributes.get("group").map(|g| g.trim())
                    == Some(group.as_str())
            });
            if !used {
                faults.push(Fault::EmptyGroup {
                    group: group.clone(),
                });
            }
        }

        // What tcspecial is configured with, once all of it is there. Lazy
        // about the unanswered and strict about the answered, as a node is:
        // the section being required is the ground check's business and is
        // where it can be said with a file and a line.
        let answered = |attribute: &str| {
            self.tcspecial
                .get(attribute)
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
        };
        // Which are required depends on what the vehicle answers to, which
        // decides how its links are written.
        let kind = answered("gateway")
            .and_then(|said| tcslibgs::gateway::GatewayKind::of(said).ok());
        let filled = tcslibgs::graph::parameters::ATTRIBUTES
            .iter()
            .filter(|(_, needed)| needed.required(kind))
            .all(|(attribute, _)| answered(attribute).is_some());
        if filled {
            let said = self
                .tcspecial
                .iter()
                .filter(|(_, value)| !value.trim().is_empty())
                .map(|(attribute, value)| {
                    (attribute.clone(), Scalar::of(value.as_str()))
                })
                .collect();
            let outcome = tcslibgs::graph::parameters::ci_config_of(&said)
                .and_then(|section| section.to_ci_config().map(|_| ()));
            if let Err(why) = outcome {
                faults.push(Fault::BadTcspecial { why });
            }
        }

        // What may be written to a payload.
        //
        // A payload that is read is asked for its readings, and the bytes that
        // ask are its trigger -- so a link delivering into a payload that is
        // read and states no trigger has nowhere to go. The same rule
        // tcsverify applies on the ground and tcspecial applies again aboard;
        // see tcslibgs::graph::check, which is where the reasoning is.
        //
        // Only a payload that is read. One that is written and never read -- a
        // heater, a valve, a latch -- is driven rather than asked, and a
        // trigger on one would be bytes written before a read that never
        // happens.
        for node in &self.nodes {
            let payload = NodeType::ALL
                .iter()
                .find(|t| t.spelling() == node.node_type)
                .is_some_and(|t| t.is_payload());
            if !payload || node.attributes.contains_key("trigger") {
                continue;
            }
            let name = Qualified::of(&node.name, &node.home).absolute();
            if !self.links.iter().any(|link| link.from == name) {
                continue;
            }
            for link in self.links.iter().filter(|link| link.to == name) {
                faults.push(Fault::UntriggeredInput {
                    node: name.clone(),
                    from: link.from.clone(),
                });
            }
        }

        let mut bound: BTreeMap<(&str, &str), usize> = BTreeMap::new();
        for link in &self.links {
            if link.from == link.to {
                faults.push(Fault::SelfLink {
                    node: link.from.clone(),
                });
                continue;
            }
            if self.node(&link.from).is_none() || self.node(&link.to).is_none()
            {
                faults.push(Fault::DanglingLink {
                    from: link.from.clone(),
                    to: link.to.clone(),
                });
                continue;
            }
            if let Some(input) = &link.input {
                *bound
                    .entry((link.to.as_str(), input.as_str()))
                    .or_insert(0) += 1;
            }
        }
        for ((node, input), count) in bound {
            if count > 1 {
                faults.push(Fault::InputBoundTwice {
                    node: node.to_string(),
                    input: input.to_string(),
                });
            }
        }

        faults
    }

    /// Every file this document is written as, and what goes in each.
    ///
    /// The top diagram is `path`, and each subdiagram is a file beside it
    /// named after itself, in the same format -- see
    /// [`crate::subdiagram::file_for`]. The top file carries an `include` list
    /// naming them, which is how the flight software is told what to read:
    /// nothing is discovered by scanning a directory.
    ///
    /// A node is written in its own home's file. A link is written in the file
    /// its source lives in, with names qualified only where they have to
    /// travel, so a subdiagram's internal links read as plainly inside it as
    /// they would if it were the whole configuration.
    ///
    /// # Arguments
    ///
    /// * `path` - The top diagram's file, whose extension says which format
    ///            every file but the simulator's is written in.
    ///
    /// # Returns
    ///
    /// Each file's path with its contents: the top diagram, its other spelling
    /// where one exists, each subdiagram, and the simulator configuration where
    /// there is one.
    ///
    /// # Errors
    ///
    /// A message naming the file if its extension is not `.ytcs` or
    /// `.xtcs`.
    pub fn files(
        &self,
        path: &std::path::Path,
    ) -> Result<Vec<(std::path::PathBuf, String)>, String> {
        let files = self.files_moved(path, path, None)?;
        Ok(files
            .into_iter()
            .map(|moved| (moved.to, moved.text))
            .collect())
    }

    /// Every file this document is written as, moved to be written somewhere
    /// else: what Save as writes.
    ///
    /// The same files [`Document::files`] gives for `from`, each placed
    /// relative to `to` as it is to `from` -- an included file a directory
    /// up from the old top file is a directory up from the new one -- so that
    /// the names the top file includes them by still find them, and the copy
    /// is the same configuration with the same digest. The top file and its
    /// other spelling take `to`'s name; every other file keeps its own.
    ///
    /// The other spellings are those `from` has, wherever `to` is: a copy of
    /// a configuration kept as a pair is the pair.
    ///
    /// The text is what the editor writes for each file at its new place,
    /// which differs from the old only where a file says where its schema
    /// is.
    ///
    /// # Arguments
    ///
    /// * `from`      - The top diagram's file now.
    /// * `to`        - Where it is going, whose extension says which format
    ///                 every file but the simulator's is written in.
    /// * `simulator` - Where the simulator's file is going, or `None` to
    ///                 leave it where it is.
    ///
    /// # Returns
    ///
    /// Each file, in the order [`Document::files`] gives them, with where it
    /// was and where it goes.
    ///
    /// # Errors
    ///
    /// A message naming the file if its extension is not `.ytcs` or
    /// `.xtcs`.
    pub fn files_moved(
        &self,
        from: &std::path::Path,
        to: &std::path::Path,
        simulator: Option<&std::path::Path>,
    ) -> Result<Vec<Moved>, String> {
        let mut written = Vec::new();

        // The top diagram, and the includes that say what else to read.
        //
        // Each moved file is said to come from the old one in its own
        // spelling where there is one, so that a copy made in the other
        // spelling of a pair is the copy of that file.
        let paired = self.other_spelling(from).is_some();
        written.push(Moved {
            from: same_spelling(from, to, paired),
            to: to.to_path_buf(),
            text: self.text_for(&None, to)?,
        });

        // And the same configuration in the other spelling, where one is
        // already there beside it. A configuration that exists as both
        // `vehicle.ytcs` and `vehicle.xtcs` is one configuration written twice,
        // and two files that are meant to say the same thing drift the moment
        // only one of them is written: the shipped example is a pair, and
        // keeping it a pair by hand meant a command to remember after every
        // session.
        //
        // Only where the sibling exists. Writing one that does not would be
        // the editor deciding somebody wants their configuration in two
        // languages, which is not a thing to decide for them.
        if let Some(beside_to) = swapped(to).filter(|_| paired) {
            written.push(Moved {
                text: self.text_for(&None, &beside_to)?,
                from: spelled_like(from, &beside_to),
                to: beside_to,
            });
        }

        // Each subdiagram, and its other spelling where that is there too,
        // for the same reason as the top diagram's.
        for name in self.subdiagrams() {
            let home = Some(name.clone());
            let file = file_for(&name, from);
            let file_to = file_for(&name, to);
            let paired = self.other_spelling(&file).is_some();
            written.push(Moved {
                from: same_spelling(&file, &file_to, paired),
                to: file_to.clone(),
                text: self.text_for(&home, to)?,
            });
            if let Some(beside_to) = swapped(&file_to).filter(|_| paired) {
                written.push(Moved {
                    text: self.text_for(&home, &beside_to)?,
                    from: spelled_like(&file, &beside_to),
                    to: beside_to,
                });
            }
        }

        // And the simulator's own file, where the session was given one. Part
        // of the same set on purpose: the two files are edited together,
        // checked together and must agree, so one Save writes both and one
        // Undo takes back whichever of them the last edit touched.
        if let Some(simulator_now) = self.simulator.as_ref() {
            let at = simulator.unwrap_or(&simulator_now.path);
            if let Some((_, text)) = self.simulator_file_at(at) {
                written.push(Moved {
                    from: simulator_now.path.clone(),
                    to: at.to_path_buf(),
                    text,
                });
            }
        }
        Ok(written)
    }

    /// The simulator configuration's file and what goes in it, where the
    /// session was given one.
    ///
    /// As YAML whatever the configuration beside it is written in: a
    /// simulator configuration is a YAML file, and its name has to say so --
    /// see `sim::must_be_yaml` -- so there is one spelling of it and nothing
    /// here to choose between.
    ///
    /// # Returns
    ///
    /// The file's path with its contents, or `None` without a simulator
    /// configuration.
    pub fn simulator_file(&self) -> Option<(std::path::PathBuf, String)> {
        let simulator = self.simulator.as_ref()?;
        self.simulator_file_at(&simulator.path)
    }

    /// The simulator configuration's file and what goes in it, written at
    /// `at` rather than where it was read from.
    ///
    /// Apart from [`Document::simulator_file`] because the file says where
    /// its schema is relative to itself, so the text depends on where it is
    /// going.
    ///
    /// # Arguments
    ///
    /// * `at` - Where the file is going.
    ///
    /// # Returns
    ///
    /// `at` with the file's contents, or `None` without a simulator
    /// configuration.
    pub fn simulator_file_at(
        &self,
        at: &std::path::Path,
    ) -> Option<(std::path::PathBuf, String)> {
        let mut simulator = self.simulator.clone()?;
        simulator.path = at.to_path_buf();
        let payloads = self.simulated_payloads();
        Some((
            simulator.path.clone(),
            crate::sim::to_yaml(&simulator, &payloads),
        ))
    }

    /// The payloads a simulator configuration can have an entry for, each
    /// with its type.
    ///
    /// The configuration's own payloads and their types, because it is what
    /// decides which payloads there are to simulate and what each is. Only
    /// the payloads a stand-in can be made for, of a kind of payload: one
    /// that is a program is the program itself, and has no type a simulated
    /// payload can be written under.
    ///
    /// # Returns
    ///
    /// Each payload's name and type, in the order the document holds them.
    pub fn simulated_payloads(&self) -> Vec<(String, String)> {
        self.nodes
            .iter()
            .filter(|node| crate::pane::is_payload(&node.node_type))
            .filter(|node| {
                tcslibgs::DHType::from_spelling(&node.node_type).is_some()
            })
            .map(|node| (node.name.clone(), node.node_type.clone()))
            .collect()
    }

    /// One file's worth: the nodes at `home` and the links that start there.
    ///
    /// # Arguments
    ///
    /// * `home` - The subdiagram to write, or `None` for the top diagram.
    /// * `path` - Where the text is going, whose extension says the format.
    ///
    /// # Returns
    ///
    /// The file's text.
    ///
    /// # Errors
    ///
    /// A message naming the file if its extension is not `.ytcs` or
    /// `.xtcs`.
    fn text_for(
        &self,
        home: &Home,
        path: &std::path::Path,
    ) -> Result<String, String> {
        // The extension decides, by the rule every program here follows:
        // see tcslibgs::ConfigFormat, which refuses anything but .ytcs and
        // .xtcs rather than guess at it.
        match ConfigFormat::of_file(path).map_err(|e| e.to_string())? {
            ConfigFormat::Yaml => Ok(self.to_yaml_for(home, path)),
            ConfigFormat::Xml => Ok(self.to_xml_for(home, path)),
        }
    }

    /// The top diagram alone, in the format `path` asks for.
    ///
    /// For the tests, and for anything that wants one file's worth without
    /// the subdiagrams beside it. [`Document::files`] is what the editor
    /// writes through.
    #[cfg(test)]
    pub fn to_text(&self, path: &std::path::Path) -> Result<String, String> {
        self.text_for(&None, path)
    }

    /// The document as XML.
    ///
    /// The same conventions the shipped XML sets use: every value is a child
    /// element and never an attribute, and a sequence is repeated sibling
    /// elements named for the field. A comment is an XML comment above the
    /// element it belongs to, which is where it is in the YAML as well.
    #[cfg(test)]
    pub fn to_xml(&self) -> String {
        self.to_xml_for(&None, std::path::Path::new("configuration.xtcs"))
    }

    /// One home's nodes and links, as XML.
    ///
    /// `path` is where the text is going, which the file needs to know only
    /// to say where its schema is relative to itself. See
    /// [`crate::nodes::schema_reference`].
    ///
    /// # Arguments
    ///
    /// * `home` - The subdiagram to write, or `None` for the top diagram.
    /// * `path` - Where the text is going.
    ///
    /// # Returns
    ///
    /// The file's text.
    fn to_xml_for(&self, home: &Home, path: &std::path::Path) -> String {
        let mut out = String::new();
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        // Which schema describes this file, in the one spelling every XML
        // tool reads: an editor opening a configuration then validates it
        // without being set up to.
        let schema =
            crate::nodes::schema_reference(path, crate::nodes::CONFIG_XSD);
        out.push_str("<configuration\n");
        out.push_str(
            "    xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"\n",
        );
        out.push_str(&format!(
            "    xsi:noNamespaceSchemaLocation=\"{schema}\">\n"
        ));
        // What the file is, with the blank line under it that keeps it the
        // file's rather than the first entry's: see `read::comments_of`.
        if let Some(header) = self.remark(home, &Heading::File) {
            xml_comment(&mut out, Some(header), 4);
            out.push('\n');
        }
        // The configuration's description, in the top file only: a file it
        // includes may be included by another configuration too, which says
        // what it is itself.
        if let (None, Some(description)) = (home, &self.description) {
            out.push_str(&format!(
                "    <description>{}</description>\n",
                escape(description)
            ));
        }

        // The same section the YAML writes, in the same place and for the
        // same reason.
        if home.is_none() {
            let said = self.tcspecial_stated();
            if !said.is_empty() {
                out.push_str("    <tcspecial>\n");
                for (parent, under) in said {
                    if parent.is_empty() {
                        for (attribute, value) in under {
                            xml_comment(
                                &mut out,
                                self.tcspecial_comment(&parent, &attribute),
                                8,
                            );
                            out.push_str(&format!(
                                "        <{attribute}>{}</{attribute}>\n",
                                escape(&value)
                            ));
                        }
                        continue;
                    }
                    xml_comment(
                        &mut out,
                        self.tcspecial_comment("", &parent),
                        8,
                    );
                    out.push_str(&format!("        <{parent}>\n"));
                    for (attribute, value) in under {
                        xml_comment(
                            &mut out,
                            self.tcspecial_comment(&parent, &attribute),
                            12,
                        );
                        out.push_str(&format!(
                            "            <{attribute}>{}</{attribute}>\n",
                            escape(&value)
                        ));
                    }
                    out.push_str(&format!("        </{parent}>\n"));
                }
                out.push_str("    </tcspecial>\n");
            }
        }

        if home.is_none() {
            // In order of name, which is the order they are digested in.
            let mut included = self.subdiagrams();
            included.sort();
            for name in included {
                out.push_str(&format!(
                    "    <include>{}</include>\n",
                    escape(&name)
                ));
            }
        }

        // The exports, where the YAML writer has them.
        if let Some(exports) = self.exports_written(home) {
            xml_comment(
                &mut out,
                self.remark(home, &Heading::Section("exports".to_string())),
                4,
            );
            out.push_str("    <exports>\n");
            for (terminal, nodes) in exports.each() {
                for node in nodes {
                    out.push_str(&format!(
                        "        <{terminal}>{}</{terminal}>\n",
                        escape(node)
                    ));
                }
            }
            out.push_str("    </exports>\n");
        }

        {
            for (name, interval) in self.groups_in(home) {
                out.push_str("    <payload_groups>\n");
                out.push_str(&format!(
                    "        <name>{}</name>\n",
                    escape(name)
                ));
                out.push_str(&format!(
                    "        <interval>{}</interval>\n",
                    escape(interval)
                ));
                out.push_str("    </payload_groups>\n");
            }
        }

        // The three sections, as the YAML writer does them: one file shape in
        // two spellings. Several destinations are several elements of one
        // name, which is how XML says a list.
        for section in Section::ALL {
            let grouped = self.grouped(home, section);
            if grouped.is_empty() {
                continue;
            }
            let word = section.node_word();
            let name = section.name().to_string();
            xml_comment(
                &mut out,
                self.remark(home, &Heading::Section(name.clone())),
                4,
            );
            out.push_str(&format!("    <{name}>\n"));
            xml_comment(
                &mut out,
                self.remark(home, &Heading::Opening(name)),
                8,
            );
            for (node_type, nodes) in grouped {
                let heading = Heading::Type(node_type.to_string());
                xml_comment(&mut out, self.remark(home, &heading), 8);
                out.push_str(&format!(
                    "        <type name=\"{}\">\n",
                    escape(node_type)
                ));
                for node in nodes {
                    if let Some(comment) = &node.comment {
                        xml_comment(&mut out, Some(comment), 12);
                    }
                    // What is inside the node, written after its opening
                    // tag; a node with nothing to say is one empty element.
                    let mut inside = String::new();
                    for (attribute, value) in &node.attributes {
                        // Left out where it is empty, as the YAML writer does
                        // it, and the sinks' names, which are written below.
                        if value.trim().is_empty() || in_a_sink(node, attribute)
                        {
                            continue;
                        }
                        // A list is the element once for each entry, which
                        // is how XML writes one: see tcslibgs::graph::LISTED.
                        if tcslibgs::graph::LISTED.contains(&attribute.as_str())
                        {
                            for one in listed(value) {
                                inside.push_str(&format!(
                                    "                <{attribute}>{}\
                                     </{attribute}>\n",
                                    escape(one)
                                ));
                            }
                            continue;
                        }
                        inside.push_str(&format!(
                            "                <{attribute}>{}</{attribute}>\n",
                            escape(value)
                        ));
                    }
                    for (terminal, destinations, comment) in
                        self.leaving(node, home)
                    {
                        if let Some(comment) = &comment {
                            xml_comment(&mut inside, Some(comment), 16);
                        }
                        for bound in destinations {
                            inside.push_str(&format!(
                                "                <{terminal}>{}</{terminal}>\n",
                                escape(&bound.written())
                            ));
                        }
                    }
                    for (sink, names) in self.sinks(node, home) {
                        for name in names {
                            inside.push_str(&format!(
                                "                <{sink}>{}</{sink}>\n",
                                escape(&name)
                            ));
                        }
                    }
                    if inside.is_empty() {
                        out.push_str(&format!(
                            "            <{word} name=\"{}\"/>\n",
                            escape(&node.name)
                        ));
                    } else {
                        out.push_str(&format!(
                            "            <{word} name=\"{}\">\n",
                            escape(&node.name)
                        ));
                        out.push_str(&inside);
                        out.push_str(&format!("            </{word}>\n"));
                    }
                }
                out.push_str("        </type>\n");
            }
            out.push_str(&format!("    </{}>\n", section.name()));
        }

        // The command bindings, each in the file it was read from: what the
        // ground may change is a property of the whole configuration, and the
        // file a binding is written in is whoever wrote it's choice. One made
        // in the editor goes in the top file.
        {
            for command in self.commands.iter().filter(|c| &c.home == home) {
                if let Some(comment) = &command.comment {
                    xml_comment(&mut out, Some(comment), 4);
                }
                out.push_str("    <commands>\n");
                out.push_str(&format!(
                    "        <from>{}</from>\n",
                    escape(&command.from)
                ));
                out.push_str(&format!(
                    "        <sets>{}</sets>\n",
                    escape(&command.sets)
                ));
                out.push_str("    </commands>\n");
            }
        }

        // And last, what the editor arranged: see the YAML writer. Each
        // entry names its node as the sections name theirs, by a name
        // attribute, and in the order of the names.
        let placed = self.placed_in(home);
        let here = self.aside_in(home);
        if !placed.is_empty() || !here.is_empty() {
            out.push_str("    <tcsgui>\n");
        }
        if !placed.is_empty() {
            out.push_str("        <nodes>\n");
            for node in &placed {
                out.push_str(&format!(
                    "            <node name=\"{}\" x=\"{}\" y=\"{}\"/>\n",
                    escape(&node.name),
                    node.at.0,
                    node.at.1
                ));
            }
            out.push_str("        </nodes>\n");
        }
        // And what was set aside about the nodes in this file.
        if !here.is_empty() {
            out.push_str("        <set-aside>\n");
            for (node, concern) in &here {
                out.push_str(&format!(
                    "            <node name=\"{}\" concern=\"{}\"/>\n",
                    escape(&Qualified::parse(node).written()),
                    escape(concern)
                ));
            }
            out.push_str("        </set-aside>\n");
        }
        if !placed.is_empty() || !here.is_empty() {
            out.push_str("    </tcsgui>\n");
        }

        out.push_str("</configuration>\n");
        out
    }

    /// The document as a configuration file.
    ///
    /// Written in the shape `design.rst` specifies: three sections of nodes,
    /// each grouped by type, each node carrying its name, its attributes and
    /// the links that leave it. The canvas positions are written as well, in
    /// a `tcsgui` section at the end, under a key the flight software
    /// ignores, so that reopening the file finds the diagram as it was left.
    #[cfg(test)]
    pub fn to_yaml(&self) -> String {
        self.to_yaml_for(&None, std::path::Path::new("configuration.ytcs"))
    }

    /// One home's nodes and links, as YAML.
    ///
    /// `path` is where the text is going, which the file needs to know only
    /// to say where its schema is relative to itself. See
    /// [`crate::nodes::schema_reference`].
    ///
    /// # Arguments
    ///
    /// * `home` - The subdiagram to write, or `None` for the top diagram.
    /// * `path` - Where the text is going.
    ///
    /// # Returns
    ///
    /// The file's text.
    fn to_yaml_for(&self, home: &Home, path: &std::path::Path) -> String {
        let mut out = String::new();
        // Which schema describes this file, in the spelling a YAML language
        // server reads -- a comment, YAML having no other way to say it. The
        // blank line under it is what keeps it a note about the file rather
        // than about the first entry: see `read::comments_of`.
        out.push_str(&format!(
            "# yaml-language-server: $schema={}\n\n",
            crate::nodes::schema_reference(
                path,
                crate::nodes::CONFIG_JSON_SCHEMA
            )
        ));
        // What the file is, as the XML writer writes it. The blank line that
        // keeps it the file's is the one every section begins with, and one
        // of its own above the description, which begins with none.
        let header = self.remark(home, &Heading::File);
        yaml_comment(&mut out, header, 0);
        // No version: a configuration has no such field. The description in
        // the top file only, as the XML writer has it.
        if let (None, Some(description)) = (home, &self.description) {
            if header.is_some() {
                out.push('\n');
            }
            out.push_str(&format!("description: {}\n", quoted(description)));
        }

        // What tcspecial is configured with, first, because it is about the
        // configuration as a whole rather than about anything in it -- which
        // is where the payload sets put their own copy of this section. Only
        // the top file: a configuration has one command interpreter, and an
        // included file that declared one would be a second answer to one
        // question.
        // An attribute somebody has cleared is left out rather than written
        // empty. Absent and empty mean the same thing to the checking -- see
        // parameters::ci_config_of -- and a file without the line is the one a
        // person would have written. A section with nothing filled in is
        // therefore no section at all: a `tcspecial:` with nothing under it
        // would read back as nothing, which is a file that does not say what
        // it says.
        if home.is_none() {
            let said = self.tcspecial_stated();
            if !said.is_empty() {
                out.push_str("\ntcspecial:\n");
                for (parent, under) in said {
                    if parent.is_empty() {
                        for (attribute, value) in under {
                            yaml_comment(
                                &mut out,
                                self.tcspecial_comment(&parent, &attribute),
                                2,
                            );
                            out.push_str(&format!(
                                "  {attribute}: {}\n",
                                quoted(&value)
                            ));
                        }
                        continue;
                    }
                    yaml_comment(
                        &mut out,
                        self.tcspecial_comment("", &parent),
                        2,
                    );
                    out.push_str(&format!("  {parent}:\n"));
                    for (attribute, value) in under {
                        yaml_comment(
                            &mut out,
                            self.tcspecial_comment(&parent, &attribute),
                            4,
                        );
                        out.push_str(&format!(
                            "    {attribute}: {}\n",
                            quoted(&value)
                        ));
                    }
                }
            }
        }

        // The subdiagrams this file pulls in, named rather than discovered.
        // Only the top diagram has them: a subdiagram that included another
        // would be a nesting this editor does not make.
        if home.is_none() {
            let mut included = self.subdiagrams();
            included.sort();
            if !included.is_empty() {
                out.push_str("\ninclude:\n");
                for name in &included {
                    out.push_str(&format!("  - {name}\n"));
                }
            }
        }

        // What of a subdiagram's file the files around it may join, near the
        // top, because it is what a reader of the file from outside wants
        // first: see tcslibgs::graph::exports.
        if let Some(exports) = self.exports_written(home) {
            out.push('\n');
            yaml_comment(
                &mut out,
                self.remark(home, &Heading::Section("exports".to_string())),
                0,
            );
            out.push_str("exports:\n");
            for (terminal, nodes) in exports.each() {
                if !nodes.is_empty() {
                    out.push_str(&format!(
                        "  {terminal}: [{}]\n",
                        nodes.join(", ")
                    ));
                }
            }
        }

        // The groups, each in the file it was read from, as the command
        // bindings are: see the XML writer. One made in the editor goes in
        // the top file.
        let groups = self.groups_in(home);
        if !groups.is_empty() {
            out.push_str("\npayload_groups:\n");
            for (name, interval) in groups {
                out.push_str(&format!(
                    "  - name: {}\n    interval: {}\n",
                    quoted(name),
                    quoted(interval)
                ));
            }
        }

        // The nodes, in three sections: the ground links, what the vehicle
        // itself runs, and what it carries. The same three working panes the
        // canvas is divided into, and which section a node belongs in is its
        // type's own answer -- see tcslibgs::graph::section_of -- so the file
        // and the drawing cannot come to disagree about what a node is.
        //
        // Within a section the nodes are grouped by type, each group an entry
        // `type <type>:` holding its nodes, and everything is in the one
        // order a configuration has: the types by name, the nodes of a type
        // by name, a node's attributes by name, then its links and their
        // destinations, then its sinks. Nothing in a configuration is in an
        // order that means anything, so the editor writes the order that
        // makes two files saying one thing the same text.
        //
        // A section with nothing in it is left out rather than written empty:
        // a configuration with no payloads yet is a configuration somebody is
        // partway through, and `payloads:` with nothing under it reads back as
        // nothing anyway.
        for section in Section::ALL {
            let grouped = self.grouped(home, section);
            if grouped.is_empty() {
                continue;
            }
            let word = section.node_word();
            let name = section.name().to_string();
            out.push('\n');
            yaml_comment(
                &mut out,
                self.remark(home, &Heading::Section(name.clone())),
                0,
            );
            out.push_str(&format!("{name}:\n"));
            yaml_comment(
                &mut out,
                self.remark(home, &Heading::Opening(name)),
                2,
            );
            for (node_type, nodes) in grouped {
                let heading = Heading::Type(node_type.to_string());
                yaml_comment(&mut out, self.remark(home, &heading), 2);
                out.push_str(&format!("  - type {node_type}:\n"));
                for node in nodes {
                    // Above the entry it belongs to, as a comment and not as a
                    // field: this is what a person hand-editing the file
                    // expects to find, and it is what they would have written
                    // themselves. Broken where the XML writer breaks it, so
                    // the two spellings carry the same comment: see
                    // `comment_lines`.
                    if let Some(comment) = &node.comment {
                        yaml_comment(&mut out, Some(comment), 6);
                    }
                    out.push_str(&format!(
                        "      - {word}: {}\n",
                        plain(&node.name)
                    ));
                    for (attribute, value) in &node.attributes {
                        // Nothing written for an attribute nobody has
                        // answered. Absent and empty mean the same thing to
                        // the checking, and a file without the line is the one
                        // a person would have written -- which is the rule the
                        // tcspecial section follows too. It matters on the way
                        // back as much as on the way out: a hand-written
                        // `address:` with nothing after it would otherwise be
                        // saved as `address: ""`.
                        if value.trim().is_empty() || in_a_sink(node, attribute)
                        {
                            continue;
                        }
                        // Quoted, which is not fussiness. A YAML reader turns
                        // 0x48 into the integer 72 and 2.0 into 2 before
                        // anything sees the text, where an XML reader hands
                        // over what was written -- so the same configuration
                        // in the two formats would read differently, and the
                        // digest of one would not match the other. Quoting
                        // keeps the spelling the person wrote.
                        // A list is a sequence: see tcslibgs::graph::LISTED.
                        if tcslibgs::graph::LISTED.contains(&attribute.as_str())
                        {
                            out.push_str(&format!("        {attribute}:\n"));
                            for one in listed(value) {
                                out.push_str(&format!(
                                    "          - {}\n",
                                    quoted(one)
                                ));
                            }
                            continue;
                        }
                        out.push_str(&format!(
                            "        {attribute}: {}\n",
                            quoted(value)
                        ));
                    }
                    // And the links out of it, each under the data source it
                    // leaves by, one entry for each destination.
                    for (terminal, destinations, comment) in
                        self.leaving(node, home)
                    {
                        if let Some(comment) = &comment {
                            yaml_comment(&mut out, Some(comment), 8);
                        }
                        out.push_str(&format!("        {terminal}:\n"));
                        for bound in destinations {
                            out.push_str(&format!(
                                "          - {}\n",
                                bound.written()
                            ));
                        }
                    }
                    // And which of its data sinks something arrives at, which
                    // is the same lines seen from the other end. See
                    // `arriving`.
                    for (sink, names) in self.sinks(node, home) {
                        out.push_str(&format!(
                            "        {sink}: [{}]\n",
                            names
                                .iter()
                                .map(|name| plain(name))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                }
            }
        }

        // The command bindings, each in its own file: see the XML writer.
        let commands: Vec<&Command> =
            self.commands.iter().filter(|c| &c.home == home).collect();
        if !commands.is_empty() {
            out.push_str("\ncommands:\n");
            for command in commands {
                if let Some(comment) = &command.comment {
                    yaml_comment(&mut out, Some(comment), 2);
                }
                out.push_str(&format!(
                    "  - {{ from: {}, sets: {} }}\n",
                    command.from, command.sets
                ));
            }
        }

        // And last, what the editor arranged: where each node was drawn.
        //
        // Last and in a section of its own because it is a different kind of
        // thing. The sections above say what the vehicle carries and what it
        // is told; this says where somebody put the boxes on a screen, which
        // the flight software ignores -- so a reader meets the vehicle first
        // and can stop reading when the vehicle stops.
        //
        // Each entry names its node as the sections name theirs, by its first
        // key, and in the order of the names.
        let placed = self.placed_in(home);
        let here = self.aside_in(home);
        if !placed.is_empty() || !here.is_empty() {
            out.push_str("\ntcsgui:\n");
        }
        if !placed.is_empty() {
            out.push_str("  nodes:\n");
            for node in placed {
                out.push_str(&format!(
                    "    - {{ node: {}, x: {}, y: {} }}\n",
                    plain(&node.name),
                    node.at.0,
                    node.at.1
                ));
            }
        }
        if !here.is_empty() {
            out.push_str("  set-aside:\n");
            for (node, concern) in here {
                out.push_str(&format!(
                    "    - {{ node: {}, concern: {} }}\n",
                    plain(&Qualified::parse(node).written()),
                    plain(concern)
                ));
            }
        }

        out
    }

    /// The same configuration's other spelling, where there is one.
    ///
    /// `vehicle.ytcs` beside `vehicle.xtcs`, in either direction, and `None`
    /// where the sibling is not there -- which is every ordinary
    /// configuration. See [`Document::files`], which writes it.
    ///
    /// # Arguments
    ///
    /// * `path` - The configuration file being written.
    ///
    /// # Returns
    ///
    /// The other spelling's path, if that file exists.
    fn other_spelling(
        &self,
        path: &std::path::Path,
    ) -> Option<std::path::PathBuf> {
        swapped(path).filter(|beside| beside.is_file())
    }

    /// What leaves a node, by the data source it leaves by.
    ///
    /// A link is an attribute of the node it leaves -- `world-out: k_1` -- so
    /// writing a node means writing the links out of it, named after their
    /// terminal. Which terminal is the editor's own table where the link does
    /// not say: a payload has one data source and the line was drawn from it.
    ///
    /// In the one order a file writes them in: the terminals by name, and
    /// each one's destinations by the node they reach and then the input, so
    /// that one document is one file however the links were drawn.
    ///
    /// Written where the node is, which is what makes every link written
    /// exactly once however far it reaches: a link from a node on one page to
    /// a node on another is an attribute of the first, so the file holding the
    /// second says nothing about it. A link written in both files it touched
    /// would be read as two.
    ///
    /// One entry per line, each the node it goes to and the name it arrives
    /// by. Several lines out of one terminal are several entries, so nothing
    /// in a file pairs by position.
    ///
    /// # Arguments
    ///
    /// * `node` - The node the links leave.
    /// * `home` - The subdiagram the node is in, which the targets' names are
    ///            written from.
    ///
    /// # Returns
    ///
    /// For each data source with something on it, `world-out` then
    /// `moc-out`: the terminal's name, an entry per link, and the first
    /// comment found on any of them.
    fn leaving(
        &self,
        node: &Node,
        home: &Home,
    ) -> Vec<(&'static str, Vec<Bound>, Option<String>)> {
        let mut leaving: Vec<(&'static str, Vec<Bound>, Option<String>)> =
            Vec::new();
        // In the one order: the terminals by name, and each one's
        // destinations by the node they reach and then the input.
        for terminal in ["moc-out", "world-out"] {
            let mut destinations: Vec<Bound> = Vec::new();
            // One line carries one comment, so the attribute's comment is the
            // first thing written about any of the links under it. Nothing in
            // the editor sets one -- a link's comment is read from a file
            // somebody wrote -- so in practice they agree or there is one.
            let mut comment: Option<String> = None;
            // The links this node is the source of. Compared as the document
            // holds them -- `zone/temp_1` from outside the subdiagram it
            // is in -- because a node's own name is the one inside its file.
            let ours = Qualified::of(&node.name, home).absolute();
            for link in self.links.iter().filter(|l| l.from == ours) {
                let by = crate::style::terminal_for(
                    &node.node_type,
                    link.origin.as_deref(),
                    false,
                );
                if by != Some(terminal) {
                    continue;
                }
                destinations.push(Bound {
                    target: Qualified::parse(&link.to).written(),
                    name: link.arrives_by(),
                });
                if comment.is_none() {
                    comment = link.comment.clone();
                }
            }
            if !destinations.is_empty() {
                destinations.sort_by(|a, b| {
                    (&a.target, &a.name).cmp(&(&b.target, &b.name))
                });
                leaving.push((terminal, destinations, comment));
            }
        }
        leaving
    }

    /// The names each of a node's data sinks lists, where it lists any.
    ///
    /// The names the node was given -- its `world-in` and `moc-in`, and a
    /// vote's inputs, which are its world-in -- and after them the name of
    /// every line arriving there that is not among them yet, so that a file
    /// the editor writes never sends a line to a name its far end does not
    /// list. A line drawn in the editor arrives by a name of its own: see
    /// `Link::arrives_by`. In name order.
    ///
    /// # Arguments
    ///
    /// * `node` - The node the links arrive at.
    /// * `home` - The subdiagram the node is in.
    ///
    /// # Returns
    ///
    /// Each sink with a name, `moc-in` then `world-in`, and its names.
    fn sinks(
        &self,
        node: &Node,
        home: &Home,
    ) -> Vec<(&'static str, Vec<String>)> {
        let mut sinks = Vec::new();
        let ours = Qualified::of(&node.name, home).absolute();
        for sink in ["moc-in", "world-in"] {
            let given = node
                .attributes
                .iter()
                .filter(|(attribute, _)| {
                    attribute.as_str() == sink
                        || (sink == "world-in" && votes(node, attribute))
                })
                .flat_map(|(_, value)| {
                    value
                        .split([',', ' '])
                        .map(str::trim)
                        .filter(|name| !name.is_empty())
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                });
            let mut names: Vec<String> = Vec::new();
            for name in given {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            for link in self.links.iter().filter(|l| l.to == ours) {
                if self.arrives_at(link) != Some(sink) {
                    continue;
                }
                let name = link.arrives_by();
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            // In the order of their text, a sink's names being a set whose
            // order means nothing, as a listed attribute's entries are: so one
            // document is one file however its lines were drawn.
            names.sort_unstable();
            if !names.is_empty() {
                sinks.push((sink, names));
            }
        }
        sinks
    }

    /// Which data sink a line arrives at.
    ///
    /// The one at the other end of the stream it leaves by, which needs the
    /// terminal it left by: the two are one stream read from either end. The
    /// node it left is found by its whole name, which is what the link holds
    /// -- `zone/temp_1` for a node in a subdiagram -- and which is what tells
    /// two nodes of one name apart. A line that names the sink, as one to a
    /// type with two must, arrives where it says.
    ///
    /// # Arguments
    ///
    /// * `link` - The line.
    ///
    /// # Returns
    ///
    /// `world-in` or `moc-in`, or `None` where the node it leaves is not in
    /// the document.
    fn arrives_at(&self, link: &Link) -> Option<&'static str> {
        match link.input.as_deref() {
            Some("world-in") => return Some("world-in"),
            Some("moc-in") => return Some("moc-in"),
            _ => {}
        }
        let from = self.nodes.iter().find(|n| {
            Qualified::of(&n.name, &n.home).absolute() == link.from
        })?;
        let left_by = crate::style::terminal_for(
            &from.node_type,
            link.origin.as_deref(),
            false,
        );
        left_by.and_then(tcslibgs::graph::terminals::answers)
    }
}

/// `path` in the other spelling of a configuration, whether or not that
/// file is there.
///
/// # Arguments
///
/// * `path` - A `.ytcs` or `.xtcs` file.
///
/// # Returns
///
/// `path` with the other extension, or `None` for a name that is neither.
fn swapped(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let other = match ConfigFormat::from_path(path)? {
        ConfigFormat::Yaml => ConfigFormat::Xml,
        ConfigFormat::Xml => ConfigFormat::Yaml,
    };
    Some(path.with_extension(other.extension()))
}

/// The file a moved file is the copy of: the old file in the new one's
/// spelling where the old one is kept in both, and the old file otherwise.
///
/// # Arguments
///
/// * `from`   - The old file.
/// * `to`     - Where the copy goes.
/// * `paired` - Whether `from` has its other spelling beside it.
///
/// # Returns
///
/// The old file to copy.
fn same_spelling(
    from: &std::path::Path,
    to: &std::path::Path,
    paired: bool,
) -> std::path::PathBuf {
    if paired {
        spelled_like(from, to)
    } else {
        from.to_path_buf()
    }
}

/// `path` with the extension of `like`: where the other spelling of a
/// file goes.
///
/// # Arguments
///
/// * `path` - The file in one spelling.
/// * `like` - A file in the other.
///
/// # Returns
///
/// `path`, extended as `like` is.
fn spelled_like(
    path: &std::path::Path,
    like: &std::path::Path,
) -> std::path::PathBuf {
    match like.extension() {
        Some(extension) => path.with_extension(extension),
        None => path.to_path_buf(),
    }
}

/// The entries of a listed attribute, as a file writes them: in the order of
/// their text, a list of this kind being a set whose order means nothing. See
/// tcslibgs::graph::LISTED.
///
/// # Arguments
///
/// * `value` - The attribute, its entries joined as the document holds them.
///
/// # Returns
///
/// The entries, sorted.
fn listed(value: &str) -> Vec<&str> {
    let mut entries = tcslibgs::graph::entries(value);
    entries.sort_unstable();
    entries
}

/// A value as YAML, spelled as it was written.
///
/// Every value is quoted rather than only the ones that need it. Deciding
/// which need it means knowing YAML's whole scalar grammar -- 0x48, 2.0, yes,
/// no, null, ~, 08, 1:30 are all values a reader would transform -- and a rule
/// with that many exceptions is one that will be wrong about the next one. A
/// quote on every value is uniform and cannot be wrong.
fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A name or a type, written bare where bare says the same thing.
///
/// Most of them do: `temp_1` and `i2c` are plain scalars in YAML and read
/// back as themselves, and a file where every name is in quotes reads as a
/// file written by a program. The exceptions are the words YAML gives a
/// meaning of its own -- `null` is nothing at all, `yes` is true, `0x48` is a
/// number -- and the `null` node is exactly one of them: written bare, its
/// type reads back as nothing and the file no longer says what it says.
///
/// So: bare where the text is a letter or an underscore followed by letters,
/// digits and the punctuation a name uses, or a relative path -- an
/// included file's name, `../thermostat-common/thermostat` -- and quoted
/// otherwise. That is
/// narrower than YAML's own rule on purpose -- what is safe here is what is
/// safe in both formats and in every YAML reader, and a name this quotes
/// unnecessarily is a name with quotes round it rather than a file that is
/// wrong.
///
/// # Arguments
///
/// * `value` - The name or type.
///
/// # Returns
///
/// The value bare, or quoted as [`quoted`] quotes it.
fn plain(value: &str) -> String {
    const RESERVED: &[&str] = &[
        "null", "Null", "NULL", "~", "true", "True", "TRUE", "false", "False",
        "FALSE", "yes", "Yes", "YES", "no", "No", "NO", "on", "On", "ON",
        "off", "Off", "OFF",
    ];

    let bare = !value.is_empty()
        && !RESERVED.contains(&value)
        && (value
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            || value.starts_with("./")
            || value.starts_with("../"))
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-./".contains(c));

    if bare {
        value.to_string()
    } else {
        quoted(value)
    }
}

/// XML's five, so that a comment or a value containing one of them is still
/// the document it was.
///
/// A name with an ampersand in it is unlikely and a comment with one is not:
/// "sampled at 1Hz & logged" is the sort of thing somebody writes, and an
/// unescaped ampersand makes the file unreadable rather than merely wrong.
///
/// # Arguments
///
/// * `text` - The text to write in an XML file.
///
/// # Returns
///
/// The text with `&`, `<`, `>`, `"` and `'` replaced by their entities.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// A line of a comment, as XML will carry it.
///
/// XML forbids two hyphens in a row inside a comment and forbids a comment
/// ending in one: there is no escape for them, the sequence being how a
/// comment ends. This project writes `--` as an em dash all over its prose, so
/// a note somebody wrote in the editor would otherwise make the file
/// unreadable rather than merely different -- which is the one thing a writer
/// must not do.
///
/// A run of hyphens becomes one, which reads as the dash it was and is the
/// smallest change that makes the file legal. It is a thing the XML spelling
/// does not preserve, and is listed with the others in `docs/design.rst`.
///
/// # Arguments
///
/// * `line` - One line of a comment.
///
/// # Returns
///
/// The line with each run of hyphens made one, and with trailing hyphens and
/// white space removed.
pub(crate) fn comment_safe(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut hyphen = false;
    for c in line.chars() {
        if c == '-' {
            if hyphen {
                continue;
            }
            hyphen = true;
        } else {
            hyphen = false;
        }
        out.push(c);
    }
    // And not ending in one, for the same reason: `-->` would close the
    // comment a character early.
    out.trim_end_matches('-').trim_end().to_string()
}

/// The longest a comment's line may be in the worked example, marks and
/// indent included.
#[cfg(test)]
pub(crate) const COMMENT_LINE: usize = 80;

/// How much of a line the marks around a comment take, in the format that
/// takes more of them: the XML one, `<!-- ` and ` -->` after the indent, where
/// YAML takes only `# ` after the same indent.
#[cfg(test)]
fn comment_room(indent: usize) -> usize {
    COMMENT_LINE.saturating_sub(indent + "<!-- ".len() + " -->".len())
}

/// A comment, as the lines it is written in at this indent: each of its own
/// lines, and any that would run past [`COMMENT_LINE`] broken between words.
///
/// For composing the worked example's comments, which are written here as
/// paragraphs. The writers never break a comment: see [`yaml_comment`].
///
/// One set of lines for both formats, measured against the one that takes
/// more room, so a configuration carries the same comment whichever spelling
/// it is written in -- and measured as XML writes it, escapes and all, so the
/// XML file is held to the limit and not only the text. Broken at spaces and
/// nowhere else, so a word longer than a line is a long line rather than a
/// word cut in two. A line's own indent is carried on to the lines it is
/// broken into, so an indented list in a comment stays one.
///
/// # Arguments
///
/// * `comment` - The comment, which may have several lines.
/// * `indent`  - How many columns the comment's marks are indented.
///
/// # Returns
///
/// The lines to write, without marks or indent.
#[cfg(test)]
pub(crate) fn comment_lines(comment: &str, indent: usize) -> Vec<String> {
    let room = comment_room(indent);
    let width = |text: &str| escape(&comment_safe(text)).chars().count();
    let mut out = Vec::new();
    for line in comment.lines() {
        if width(line) <= room {
            out.push(line.to_string());
            continue;
        }
        let lead: String = line.chars().take_while(|c| *c == ' ').collect();
        let mut now = lead.clone();
        for word in line.split(' ').filter(|word| !word.is_empty()) {
            let joined = if now.trim().is_empty() {
                format!("{lead}{word}")
            } else {
                format!("{now} {word}")
            };
            if width(&joined) <= room || now.trim().is_empty() {
                now = joined;
            } else {
                out.push(now);
                now = format!("{lead}{word}");
            }
        }
        out.push(now);
    }
    out
}

/// Writes a comment as YAML, a `#` line for each of its lines.
///
/// Each line as it is held, which is as it was read: a comment is the
/// author's, broken where they broke it, and a writer that broke it again
/// to a width of its own would change every comment it was not asked to.
///
/// # Arguments
///
/// * `out`     - The text being written.
/// * `comment` - The comment, if there is one; nothing is written if not.
/// * `indent`  - How many columns the `#` is indented.
fn yaml_comment(out: &mut String, comment: Option<&str>, indent: usize) {
    for line in comment.map(str::lines).into_iter().flatten() {
        if line.is_empty() {
            out.push_str(&format!("{:indent$}#\n", ""));
        } else {
            out.push_str(&format!("{:indent$}# {line}\n", ""));
        }
    }
}

/// Writes a comment as XML: one `<!-- -->`, its lines after the first
/// under the first line's text, as a comment written by hand is laid out.
///
/// Each line as it is held, as the YAML writer writes it, except for the one
/// thing XML cannot carry: two hyphens in a row, which end a comment and
/// have no escape -- see [`comment_safe`]. Nothing else is escaped. XML
/// reads no entity inside a comment, so `&apos;` there is six characters
/// and not an apostrophe.
///
/// # Arguments
///
/// * `out`     - The text being written.
/// * `comment` - The comment, if there is one; nothing is written if not.
/// * `indent`  - How many columns the `<!--` is indented.
fn xml_comment(out: &mut String, comment: Option<&str>, indent: usize) {
    let Some(comment) = comment else {
        return;
    };
    let lines: Vec<String> = comment.lines().map(comment_safe).collect();
    let last = lines.len().saturating_sub(1);
    for (at, line) in lines.iter().enumerate() {
        let start = if at == 0 {
            format!("{:indent$}<!--", "")
        } else {
            format!("{:indent$}    ", "")
        };
        let end = if at == last { " -->" } else { "" };
        if line.is_empty() && at != 0 && at != last {
            out.push('\n');
        } else {
            out.push_str(&format!("{start} {line}{end}\n"));
        }
    }
}

/// The worked example from `design.rst`: a zone thermostat.
///
/// Built here rather than written out by hand in two formats, so that the
/// shipped `tests/manual/thermostat.ytcs` and `.xtcs` are what this editor
/// writes and not an approximation of it.
/// `the_shipped_example_is_what_the_editor_writes` is what keeps the files and
/// this agreeing.
///
/// # Returns
///
/// The thermostat's document.
#[cfg(test)]
pub fn thermostat_example() -> Document {
    let mut document = Document::new();

    let place = |document: &mut Document,
                 name: &str,
                 node_type: &str,
                 at: (i32, i32)| {
        document.apply(&Edit::AddNode {
            name: name.to_string(),
            node_type: node_type.to_string(),
            home: None,
        });
        document.apply(&Edit::MoveNode {
            node: name.to_string(),
            was: (0, 0),
            now: at,
        });
    };
    let set =
        |document: &mut Document, node: &str, attribute: &str, value: &str| {
            document.apply(&Edit::SetAttribute {
                node: node.to_string(),
                attribute: attribute.to_string(),
                was: None,
                now: Some(value.to_string()),
            });
        };
    // A note held as the lines it is written in above a node, which is what
    // reading the file back gives.
    let note = |document: &mut Document, node: &str, comment: &str| {
        document.apply(&Edit::SetNodeComment {
            node: node.to_string(),
            was: None,
            now: Some(comment_lines(comment, 12).join("\n")),
        });
    };

    // What tcspecial itself is configured with, which every configuration
    // must say: where commands are taken, where beacons go, where the
    // telemetry log is written, and what it answers to.
    for (attribute, value) in [
        ("gateway", "radio"),
        ("interface", "ad9aac4dd5a6d88dd5e78824ba8a83ac"),
        ("tcs_cmd.recv", "0.0.0.0:4000"),
        ("tcs_cmd.send", "127.0.0.1:4100"),
        ("payload_command.source_base", "127.0.0.1:4300"),
        ("payload_command.dest_base", "0.0.0.0:4200"),
        ("payload_telemetry.source_base", "0.0.0.0:6000"),
        ("payload_telemetry.dest_base", "127.0.0.1:6100"),
        ("beacon.default_interval", "5 s"),
        ("beacon.reset_interval", "1 s"),
        ("beacon.send", "0.0.0.0:5550"),
        ("beacon.recv", "127.0.0.1:5650"),
        ("state_file", "/tmp/tcspecial-thermostat.state"),
        ("log.directory", "/var/log/tcspecial"),
        ("log.segment_size", "65536"),
    ] {
        document.apply(&Edit::SetTcspecial {
            attribute: attribute.to_string(),
            was: None,
            now: Some(value.to_string()),
        });
    }

    // The group the three sensors share, which is the point of the example:
    // readings that will be averaged are taken as close to together as the
    // hardware allows.
    document
        .groups
        .insert("zone_temps".to_string(), "100 ms".to_string());

    place(&mut document, "temp_1", "i2c", (34, 34));
    set(&mut document, "temp_1", "path", "/dev/i2c-1");
    set(&mut document, "temp_1", "address", "0x48");
    set(&mut document, "temp_1", "group", "zone_temps");
    set(&mut document, "temp_1", "read_size", "2");
    note(
        &mut document,
        "temp_1",
        "One of three sensors of the same place, a trio, so that \
         the temperature survives one of them going wrong. Two bytes, \
         signed, big endian.",
    );

    place(&mut document, "temp_2", "i2c", (34, 166));
    set(&mut document, "temp_2", "path", "/dev/i2c-1");
    set(&mut document, "temp_2", "address", "0x49");
    set(&mut document, "temp_2", "group", "zone_temps");
    set(&mut document, "temp_2", "read_size", "2");
    note(
        &mut document,
        "temp_2",
        "The second of the trio; the same part as temp_1, on the same bus.",
    );

    place(&mut document, "temp_3", "spi", (34, 280));
    set(&mut document, "temp_3", "path", "/dev/spidev0.0");
    set(&mut document, "temp_3", "max_speed", "1000000");
    set(&mut document, "temp_3", "spi_mode", "0");
    set(&mut document, "temp_3", "group", "zone_temps");
    set(&mut document, "temp_3", "read_size", "3");
    note(
        &mut document,
        "temp_3",
        "The third. A different part on a different bus: three \
         bytes, unsigned, its own scale and so its own conversion, so that no \
         one bus and no one part number can take the whole trio out.",
    );

    for (name, at, format, scale, offset) in [
        ("temp1", (242, 34), "i16_be", "0.0625", "273.15"),
        ("temp2", (242, 166), "i16_be", "0.0625", "273.15"),
        ("temp3", (242, 280), "u24_be", "0.0078", "255.37"),
    ] {
        place(&mut document, name, "compute", at);
        set(&mut document, name, "function", "to_kelvin");
        set(&mut document, name, "format", format);
        set(&mut document, name, "scale", scale);
        set(&mut document, name, "offset", offset);
    }

    place(&mut document, "ambient", "compute", (469, 166));
    set(&mut document, "ambient", "function", "vote");
    set(&mut document, "ambient", "algorithm", "median");
    set(&mut document, "ambient", "inputs", "in0,in1,in2");
    note(
        &mut document,
        "ambient",
        "One temperature out of the three, by the algorithm named: \
         median takes the middle reading, closest_mean the mean of the two \
         that agree best. Either leaves a sensor that has gone wrong out of \
         the answer, which a mean over all three would not, and it produces \
         a value on every arrival, so one sensor that stops reporting \
         is outvoted rather than stopping the vehicle from deciding anything.",
    );

    place(&mut document, "thermostat", "compute", (601, 242));
    set(&mut document, "thermostat", "function", "thermostat");
    set(&mut document, "thermostat", "setpoint", "293.15");
    set(&mut document, "thermostat", "half_band", "2.0");
    note(
        &mut document,
        "thermostat",
        "setpoint is set from the ground; half_band is how far either side of \
         it the heaters are switched at, the setpoint being in the middle of \
         the band.",
    );

    place(&mut document, "gateway", "gateway_radio", (790, 34));
    set(&mut document, "gateway", "address", "10.0.0.1");
    set(&mut document, "gateway", "port", "5000");
    // Where the ground station is: each node linked into this one sends to
    // this port plus its own device id, from the start.
    set(&mut document, "gateway", "ground_base", "10.0.0.2:5100");

    for (name, at, lines) in [
        ("heaters_1", (34, 393), "3,4,5"),
        ("heaters_2", (34, 470), "6,7,8"),
    ] {
        place(&mut document, name, "gpio", at);
        set(&mut document, name, "path", "/dev/gpiochip0");
        set(&mut document, name, "direction", "output");
        set(&mut document, name, "lines", lines);
    }
    note(
        &mut document,
        "heaters_1",
        "Two heaters, each three circuits driven together, both near \
         the sensors on the same thermal mass. Separate nodes rather than \
         six lines on one, so that one can be stopped from the ground while \
         the other goes on warming the mass.",
    );

    place(&mut document, "housekeeping", "log", (469, 374));
    set(
        &mut document,
        "housekeeping",
        "directory",
        "/var/log/tcspecial",
    );
    set(&mut document, "housekeeping", "format", "variable_ts_rc");

    // Every link names the data source it leaves by, which is the attribute
    // it is written under: the readings go groundward and what drives a
    // payload goes vehicleward.
    let link = |document: &mut Document,
                from: &str,
                by: &str,
                to: &str,
                input: Option<&str>| {
        document.apply(&Edit::AddLink {
            link: Link::joining(from, Some(by), to, input),
        });
    };

    link(&mut document, "temp_1", "world-out", "temp1", None);
    link(&mut document, "temp_2", "world-out", "temp2", None);
    link(&mut document, "temp_3", "world-out", "temp3", None);

    // The "T": each converted reading goes to the ground, into the vote, and
    // into the log.
    for (from, input) in [("temp1", "in0"), ("temp2", "in1"), ("temp3", "in2")]
    {
        link(&mut document, from, "world-out", "gateway", None);
        link(&mut document, from, "world-out", "ambient", Some(input));
        link(&mut document, from, "world-out", "housekeeping", None);
    }

    link(&mut document, "ambient", "world-out", "gateway", None);
    link(&mut document, "ambient", "world-out", "thermostat", None);
    // The heaters are a payload, so what drives them leaves the thermostat by
    // its moc-out: the vehicleward stream, which is what reaches a payload.
    link(&mut document, "thermostat", "moc-out", "heaters_1", None);
    link(&mut document, "thermostat", "moc-out", "heaters_2", None);
    link(
        &mut document,
        "thermostat",
        "world-out",
        "housekeeping",
        None,
    );
    link(&mut document, "thermostat", "world-out", "gateway", None);
    // The comment belongs to the terminal rather than to one link, so both
    // of the heaters carry it: that is what reading the file back gives, a
    // comment above `moc-out` being about the block and not its first line.
    for heater in ["heaters_1", "heaters_2"] {
        document.apply(&Edit::SetLinkComment {
            link: Link::joining("thermostat", Some("moc-out"), heater, None),
            was: None,
            now: Some(
                comment_lines(
                    "asserted below the setpoint, unasserted above it. Two \
                     heaters, because one failing must not leave the mass \
                     with no way to be warmed: either alone holds the band \
                     while the orbit is warm, and both together hold it in \
                     the cold too.",
                    16,
                )
                .join("\n"),
            ),
        });
    }

    // What the ground may change in flight, which the worked example calls for
    // and the diagram cannot show: the setpoint is commanded, and the rest of
    // the thermostat is not.
    document.commands.push(Command {
        from: "gateway".to_string(),
        sets: "thermostat.setpoint".to_string(),
        comment: Some(
            "the zone temperature the heaters are held at, in kelvin"
                .to_string(),
        ),
        home: None,
    });

    document
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Makes the edit that adds a node to the top diagram.
    ///
    /// # Arguments
    ///
    /// * `name`      - The new node's name.
    /// * `node_type` - Its type, as the configuration spells it.
    ///
    /// # Returns
    ///
    /// The edit.
    fn add(name: &str, node_type: &str) -> Edit {
        Edit::AddNode {
            name: name.to_string(),
            node_type: node_type.to_string(),
            home: None,
        }
    }

    /// The editor marks a link into a payload that states no trigger.
    ///
    /// The same rule the ground check and the flight software apply, applied
    /// here as the link is drawn: the moment at which saying so costs nobody
    /// anything. A fault and not a refusal -- the editor has to be able to
    /// hold a configuration that is wrong, because that is what the middle of
    /// an edit looks like.
    #[test]
    fn a_link_into_an_untriggered_payload_is_marked() {
        let mut document = Document::new();
        for edit in [
            add("sensor", "i2c"),
            add("k", "compute"),
            Edit::AddLink {
                link: Link::new("sensor", "k", None),
            },
            Edit::AddLink {
                link: Link::new("k", "sensor", None),
            },
        ] {
            document.apply(&edit);
        }

        let faults = document.faults();
        assert!(
            faults.iter().any(|f| matches!(
                f,
                Fault::UntriggeredInput { node, from }
                    if node == "sensor" && from == "k"
            )),
            "{faults:?}"
        );
        // Drawn on the payload, which is where a trigger would be typed.
        let said = faults
            .iter()
            .find(|f| matches!(f, Fault::UntriggeredInput { .. }))
            .unwrap();
        assert_eq!(said.node(), Some("sensor"));
        // And says so of the payload, which is where the trouble is, and not
        // of the node sending to it.
        assert_eq!(
            said.describe(),
            "\"sensor\" states no trigger, so what \"k\" sends it has nowhere \
             to go"
        );

        // A trigger clears it.
        document.apply(&Edit::SetAttribute {
            node: "sensor".into(),
            attribute: "trigger".into(),
            was: None,
            now: Some("[01]".into()),
        });
        assert!(
            !document
                .faults()
                .iter()
                .any(|f| matches!(f, Fault::UntriggeredInput { .. })),
            "{:?}",
            document.faults()
        );
    }

    /// A payload that is only driven is not asked for a trigger.
    ///
    /// The heaters of the shipped thermostat: written and never read, so there
    /// is nothing to ask them for.
    #[test]
    fn a_driven_payload_is_not_asked_for_a_trigger() {
        let mut document = Document::new();
        for edit in [
            add("thermostat", "compute"),
            add("heaters", "gpio"),
            Edit::AddLink {
                link: Link::new("thermostat", "heaters", None),
            },
        ] {
            document.apply(&edit);
        }
        assert!(
            !document
                .faults()
                .iter()
                .any(|f| matches!(f, Fault::UntriggeredInput { .. })),
            "{:?}",
            document.faults()
        );
    }

    /// The editor and the checker mean the same thing by "payload".
    ///
    /// Two statements of one fact -- `style.rs` has the editor's, so that a
    /// type can be put in the right selection pane, and `tcslibgs` has the
    /// one the ground check and the flight software use -- and two statements
    /// of one fact drift. What that would look like is a node the editor
    /// drew in the payload pane and the checker did not hold to a payload's
    /// rules, or the other way about.
    #[test]
    fn the_editor_and_the_checker_agree_on_what_a_payload_is() {
        for t in NodeType::ALL {
            // The subdiagram is the editor's own: a page of a configuration
            // is an `include` to everything else, and no node at all.
            if t == NodeType::Subdiagram {
                assert!(!t.is_payload());
                continue;
            }
            let theirs = tcslibgs::graph::Node {
                name: "x".to_string(),
                node_type: t.spelling().to_string(),
                group: None,
                attributes: Default::default(),
            };
            assert_eq!(
                t.is_payload(),
                theirs.is_payload(),
                "{} is a payload to one of them and not the other",
                t.spelling()
            );
        }
    }

    /// What tcspecial is configured with is written, read back, and left
    /// alone while it is unfinished.
    ///
    /// The editor is lazy about what has been entered: a section half filled
    /// in is a person partway through, not a configuration that is wrong, and
    /// nothing is marked for it. What is checked is what the words say -- and
    /// only once they are all there, because until then the only thing to say
    /// is "this is unfinished", which the person at the keyboard knows.
    #[test]
    fn the_tcspecial_section_is_written_and_left_alone_until_it_is_finished() {
        let mut document = Document::new();

        // Nothing at all, which is where every configuration starts: not a
        // fault.
        assert!(document.faults().is_empty(), "{:?}", document.faults());

        // Half of it, which is what a person typing has. Still not a fault.
        for (attribute, value) in [("protocol", "udp"), ("port", "4000")] {
            document.apply(&Edit::SetTcspecial {
                attribute: attribute.into(),
                was: None,
                now: Some(value.into()),
            });
        }
        assert!(document.faults().is_empty(), "{:?}", document.faults());

        // Written in both spellings, and in the place the payload sets put it:
        // before anything else, because it is about the configuration as a
        // whole.
        let yaml = document.to_yaml();
        assert!(yaml.contains("tcspecial:\n"), "{yaml}");
        // Quoted, as everything the graph writer writes is: a graph's values
        // are read as `Scalar`, which takes a value however the file spelled
        // it, so quoting them all is what keeps a round trip from changing
        // what a file means. The simulator's own file is different -- see
        // sim.rs -- because its reader wants the types.
        assert!(yaml.contains("  port: \"4000\"\n"), "{yaml}");
        // Before the nodes, which this document has none of: a section about
        // the configuration as a whole comes before the things in it.
        let nodes = yaml.find("payloads:").unwrap_or(yaml.len());
        assert!(
            yaml.find("tcspecial:").expect("the section") < nodes,
            "the section should come first: {yaml}"
        );
        let xml = document.to_xml();
        assert!(xml.contains("<tcspecial>"), "{xml}");
        assert!(xml.contains("<port>4000</port>"), "{xml}");

        // A parameter somebody clears leaves the file without the line rather
        // than with an empty one: absent and empty mean the same thing to the
        // checking, and a file with an empty value is one nobody would have
        // written.
        document.apply(&Edit::SetTcspecial {
            attribute: "port".into(),
            was: Some("4000".into()),
            now: None,
        });
        assert!(
            !document.to_yaml().contains("port"),
            "{}",
            document.to_yaml()
        );
        assert!(document.faults().is_empty(), "clearing one is not a fault");
    }

    /// Once the section is complete, what it says has to hold together.
    ///
    /// The other half of being lazy: the editor requires nothing to be entered
    /// and requires everything entered to be valid. A beacon address with no
    /// port is wrong the moment the last field is filled in, and the words are
    /// the ones the command interpreter itself would refuse it with.
    #[test]
    fn a_finished_tcspecial_section_is_checked_by_its_own_rules() {
        let whole = [
            ("gateway", "radio"),
            ("interface", "ad9aac4dd5a6d88dd5e78824ba8a83ac"),
            ("tcs_cmd.recv", "0.0.0.0:4000"),
            ("tcs_cmd.send", "127.0.0.1:4100"),
            ("payload_command.source_base", "127.0.0.1:4300"),
            ("payload_command.dest_base", "0.0.0.0:4200"),
            ("payload_telemetry.source_base", "0.0.0.0:6000"),
            ("payload_telemetry.dest_base", "127.0.0.1:6100"),
            ("beacon.default_interval", "5 s"),
            ("beacon.reset_interval", "1 s"),
            ("beacon.send", "0.0.0.0:5550"),
            ("beacon.recv", "127.0.0.1:5650"),
            ("state_file", "/var/lib/tcspecial/state"),
        ];

        let filled = |wrong: Option<(&str, &str)>| {
            let mut document = Document::new();
            for (attribute, value) in whole {
                let value = match wrong {
                    Some((name, instead)) if name == attribute => instead,
                    _ => value,
                };
                document.apply(&Edit::SetTcspecial {
                    attribute: attribute.into(),
                    was: None,
                    now: Some(value.into()),
                });
            }
            document
        };

        // Complete and sound: nothing said.
        assert!(
            filled(None).faults().is_empty(),
            "{:?}",
            filled(None).faults()
        );

        // Complete and wrong, in the words of the rules that know why.
        for (attribute, instead, expected) in [
            // A word that is no kind of gateway.
            ("gateway", "ground", "not a kind of gateway"),
            ("beacon.send", "10.0.0.1", "address:port"),
            ("beacon.recv", "127.0.0.1:0", "port 0"),
            // Slower across a restart than otherwise, which defeats it.
            ("beacon.reset_interval", "10 s", "no longer than"),
            // Half a command interface, which is the one way an attribute
            // carrying two things goes wrong.
            ("tcs_cmd.recv", "0.0.0.0", "address:port"),
            ("tcs_cmd.recv", "0.0.0.0:4200", "payload_command.dest_base"),
            (
                "payload_command.dest_base",
                "nought",
                "payload_command.dest_base",
            ),
        ] {
            let document = filled(Some((attribute, instead)));
            let said = document
                .faults()
                .into_iter()
                .find_map(|fault| match fault {
                    Fault::BadTcspecial { why } => Some(why),
                    _ => None,
                })
                .unwrap_or_else(|| {
                    panic!("{attribute} = {instead} should be refused")
                });
            assert!(
                said.contains(expected),
                "{attribute} = {instead} was refused as {said:?}"
            );
        }
    }

    /// A node is left alone while it is unfinished and checked once it is
    /// not.
    ///
    /// The same laziness, for the nodes. An editor that marked every empty
    /// field in red would be calling a node somebody is partway through a
    /// broken one -- and the thing that has to be true of a configuration
    /// before it flies is checked where the complaint can name the file and
    /// the line.
    #[test]
    fn a_node_is_left_alone_until_it_says_enough_to_be_wrong() {
        let mut document = Document::new();
        document.apply(&add("temp_1", "i2c"));

        // Nothing stated: nothing to say.
        assert!(document.faults().is_empty(), "{:?}", document.faults());

        // A bus and no address: still unfinished, still nothing to say.
        document.apply(&Edit::SetAttribute {
            node: "temp_1".into(),
            attribute: "path".into(),
            was: None,
            now: Some("/dev/i2c-1".into()),
        });
        assert!(document.faults().is_empty(), "{:?}", document.faults());

        // And an address that is not one, which is wrong rather than
        // unfinished.
        document.apply(&Edit::SetAttribute {
            node: "temp_1".into(),
            attribute: "address".into(),
            was: None,
            now: Some("0xZZ".into()),
        });
        let said = document
            .faults()
            .into_iter()
            .find_map(|fault| match fault {
                Fault::BadNode { node, why } if node == "temp_1" => Some(why),
                _ => None,
            })
            .expect("an address that is not one should be refused");
        assert!(said.contains("address"), "{said}");

        // Put right, and the editor goes quiet again.
        document.apply(&Edit::SetAttribute {
            node: "temp_1".into(),
            attribute: "address".into(),
            was: Some("0xZZ".into()),
            now: Some("0x48".into()),
        });
        assert!(document.faults().is_empty(), "{:?}", document.faults());
    }

    /// Reading a configuration puts every node in the pane it belongs to.
    ///
    /// A payload the file draws in the vehicle working pane is a drawing that
    /// contradicts the rule the editor enforces everywhere else, and a reader
    /// has to be told which of the two to believe. The commonest source is a
    /// file written before the canvas was divided at all.
    #[test]
    fn reading_a_configuration_settles_its_nodes_into_their_panes() {
        let dividers = crate::pane::default_dividers();
        let divider = dividers.payloads;
        let mut document = Document::new();

        use crate::pane::{node_floor, on_node_grid as on_grid};

        // A payload on the wrong side, a vehicle node on the wrong side, and
        // one of each where it belongs, on the grid.
        for (name, node_type, at) in [
            ("stray_sensor", "i2c", (divider + 40, on_grid(120))),
            ("stray_compute", "compute", (on_grid(40), on_grid(200))),
            ("good_sensor", "i2c", (on_grid(16), on_grid(320))),
            (
                "good_compute",
                "compute",
                (on_grid(divider + 240), on_grid(40)),
            ),
        ] {
            document.apply(&add(name, node_type));
            document.apply(&Edit::MoveNode {
                node: name.into(),
                was: (0, 0),
                now: at,
            });
        }

        let moved = document.settle(dividers);
        assert_eq!(moved, 2, "only the two out of place should have moved");

        let at = |name: &str| document.node(name).unwrap().at;
        let width = crate::style::WIDTH as i32;

        // The payload is in its own pane now, at the near edge of it: the
        // smallest move that obeys the rule, which is its own width short of
        // the divider, taken back to the grid.
        assert_eq!(
            at("stray_sensor"),
            (node_floor(divider - width), on_grid(120))
        );
        // And the computation is out of it, against the divider.
        assert_eq!(
            at("stray_compute"),
            (crate::pane::node_ceil(divider), on_grid(200))
        );
        // The two that were already right are untouched, which matters: a
        // reader who arranged a diagram should find it as they left it.
        assert_eq!(at("good_sensor"), (on_grid(16), on_grid(320)));
        assert_eq!(at("good_compute"), (on_grid(divider + 240), on_grid(40)));

        // Settling twice changes nothing, which is what makes it safe to do
        // on every open.
        assert_eq!(document.settle(dividers), 0);

        // And a node in its own pane but off the grid is brought to the
        // nearest grid point.
        document.apply(&add("off_grid", "i2c"));
        document.apply(&Edit::MoveNode {
            node: "off_grid".into(),
            was: (0, 0),
            now: (17, 500),
        });
        assert_eq!(document.settle(dividers), 1);
        assert_eq!(at_now(&document, "off_grid"), (on_grid(17), on_grid(500)));
    }

    fn at_now(document: &Document, name: &str) -> (i32, i32) {
        document.node(name).unwrap().at
    }

    /// A payload that is a program is held to what an app node must say, and
    /// not asked for an address it has no use for.
    ///
    /// It was: the editor asked every payload how it is reached, and an app
    /// is reached by running it, so every `app_pl` came up as a problem
    /// saying so -- with nothing in the file that could be changed to put it
    /// right. What is said now is what an app node is missing, in the words
    /// tcspecial would refuse it with.
    #[test]
    fn a_payload_that_is_a_program_is_held_to_the_app_rules() {
        let mut document = Document::new();
        document.apply(&add("lorem", "app_pl"));
        for (attribute, value) in [
            ("executable", "examples/simple-app/lorem_pl.py"),
            ("app-payload-arg", "1"),
        ] {
            document.apply(&Edit::SetAttribute {
                node: "lorem".into(),
                attribute: attribute.into(),
                was: None,
                now: Some(value.into()),
            });
        }
        let about_lorem = |document: &Document| -> Vec<String> {
            document
                .faults()
                .iter()
                .map(|fault| fault.describe())
                .filter(|said| said.contains("lorem"))
                .collect()
        };
        assert_eq!(about_lorem(&document), Vec::<String>::new());

        // And one that says something an app does not take -- a device's
        // path, which an app has no use for -- is told which word, and what an
        // app does take, in tcspecial's words.
        document.apply(&Edit::SetAttribute {
            node: "lorem".into(),
            attribute: "path".into(),
            was: None,
            now: Some("/dev/ttyS0".into()),
        });
        let said = about_lorem(&document);
        assert!(
            said.iter().any(|s| s.contains("nothing called \"path\"")
                && s.contains("It takes executable")),
            "{said:?}"
        );
    }

    /// Nodes settled into one pane do not land on top of each other.
    ///
    /// A file with its payloads in a row across the canvas would otherwise
    /// become one node with the rest behind it -- every one of them clamped to
    /// the same edge, at whatever height it already had.
    #[test]
    fn settling_does_not_stack_nodes_on_one_another() {
        let dividers = crate::pane::default_dividers();
        let divider = dividers.payloads;
        let mut document = Document::new();

        // Four sensors in a row at one height, all of them in the wrong pane.
        for (n, name) in ["one", "two", "three", "four"].iter().enumerate() {
            document.apply(&add(name, "i2c"));
            document.apply(&Edit::MoveNode {
                node: (*name).into(),
                was: (0, 0),
                now: (divider + 8 + n as i32 * 160, 80),
            });
        }

        assert_eq!(document.settle(dividers), 4);

        // Every one in its pane, and no two overlapping.
        let boxes: Vec<((i32, i32), (u32, u32))> = document
            .nodes
            .iter()
            .map(|node| (node.at, crate::pane::size_of(&node.node_type)))
            .collect();
        for (at, _) in &boxes {
            assert!(
                at.0 + crate::style::WIDTH as i32 <= divider,
                "{at:?} is not in the payload pane"
            );
        }
        for (i, (one, one_size)) in boxes.iter().enumerate() {
            for (two, two_size) in &boxes[i + 1..] {
                let apart = one.0 + one_size.0 as i32 <= two.0
                    || two.0 + two_size.0 as i32 <= one.0
                    || one.1 + one_size.1 as i32 <= two.1
                    || two.1 + two_size.1 as i32 <= one.1;
                assert!(apart, "{one:?} and {two:?} overlap");
            }
        }
    }

    /// A value set on a group reaches every payload in it.
    ///
    /// What a group is for beyond the schedule: a dozen sensors on one bus are
    /// told the bus once. One edit, so that undo takes back the gesture rather
    /// than the first of a dozen consequences -- which the round-trip test
    /// above proves for this variant along with the others.
    #[test]
    fn a_value_set_on_a_group_reaches_every_payload_in_it() {
        let mut document = Document::new();
        document.apply(&Edit::AddGroup {
            name: "zone_temps".into(),
            interval: "3 s".into(),
        });
        for (name, node_type, group) in [
            ("temp_1", "i2c", Some("zone_temps")),
            ("temp_2", "i2c", Some("zone_temps")),
            // In another group, so it is not touched.
            ("temp_3", "i2c", Some("bay")),
            // Not in a group at all.
            ("loose", "i2c", None),
            // In the group and not a payload, which is an oddity the checker
            // has words for and not something to set a bus address on.
            ("k", "compute", Some("zone_temps")),
        ] {
            document.apply(&add(name, node_type));
            if let Some(group) = group {
                document.apply(&Edit::SetAttribute {
                    node: name.into(),
                    attribute: "group".into(),
                    was: None,
                    now: Some(group.into()),
                });
            }
        }

        let was: Vec<(String, Option<String>)> = document
            .payloads_of("zone_temps")
            .iter()
            .map(|n| (n.name.clone(), n.attributes.get("path").cloned()))
            .collect();
        assert_eq!(was.len(), 2, "the group's payloads: {was:?}");

        document.apply(&Edit::SetGroupAttribute {
            group: "zone_temps".into(),
            attribute: "path".into(),
            was,
            now: Some("/dev/i2c-1".into()),
        });

        let path = |name: &str| {
            document.node(name).unwrap().attributes.get("path").cloned()
        };
        assert_eq!(path("temp_1").as_deref(), Some("/dev/i2c-1"));
        assert_eq!(path("temp_2").as_deref(), Some("/dev/i2c-1"));
        assert_eq!(path("temp_3"), None, "another group was written to");
        assert_eq!(path("loose"), None, "a node in no group was written to");
        assert_eq!(path("k"), None, "a computation was given a bus path");
    }

    /// Renaming a group renames it on every node that is in it.
    ///
    /// A node's group is how it gets its interval, so a rename that left the
    /// members behind would be a rename that silently unscheduled them -- and
    /// it would do it quietly, because the nodes would still say they were in
    /// a group and the group would still exist under its new name.
    #[test]
    fn renaming_a_group_renames_it_on_its_members() {
        let mut document = Document::new();
        document.apply(&Edit::AddGroup {
            name: "zone_temps".into(),
            interval: "3 s".into(),
        });
        document.apply(&add("temp_1", "i2c"));
        document.apply(&Edit::SetAttribute {
            node: "temp_1".into(),
            attribute: "group".into(),
            was: None,
            now: Some("zone_temps".into()),
        });

        document.apply(&Edit::RenameGroup {
            was: "zone_temps".into(),
            now: "zone".into(),
        });

        assert_eq!(
            document.groups.get("zone").map(String::as_str),
            Some("3 s")
        );
        assert!(!document.groups.contains_key("zone_temps"));
        assert_eq!(
            document
                .node("temp_1")
                .unwrap()
                .attributes
                .get("group")
                .map(String::as_str),
            Some("zone")
        );
        // And the group is not left looking empty, which is a fault.
        assert!(
            !document.faults().iter().any(|f| matches!(
                f,
                Fault::EmptyGroup { .. } | Fault::NoSuchGroup { .. }
            )),
            "{:?}",
            document.faults()
        );
    }

    /// A group offers the attributes its payloads all have, and nothing else.
    ///
    /// An attribute only half of them declare is a value that would mean
    /// nothing on the rest. `group` is left out because it *is* the group, and
    /// `interval` because it belongs to the group and a member stating one is
    /// a fault the flight reader refuses.
    #[test]
    fn a_group_offers_what_its_payloads_have_in_common() {
        let mut document = Document::new();
        document.apply(&Edit::AddGroup {
            name: "zone_temps".into(),
            interval: "3 s".into(),
        });
        for (name, node_type) in [("temp_1", "i2c"), ("temp_3", "spi")] {
            document.apply(&add(name, node_type));
            document.apply(&Edit::SetAttribute {
                node: name.into(),
                attribute: "group".into(),
                was: None,
                now: Some("zone_temps".into()),
            });
        }

        let offered: Vec<String> = document
            .group_fields("zone_temps")
            .into_iter()
            .map(|f| f.attribute)
            .collect();

        // What an I2C node and a SPI node both have.
        for shared in ["path", "read_size", "trigger"] {
            assert!(offered.contains(&shared.to_string()), "{offered:?}");
        }
        // And what only one of them has.
        for theirs in ["address", "pec", "max_speed", "spi_mode"] {
            assert!(!offered.contains(&theirs.to_string()), "{offered:?}");
        }
        // Never these two.
        for never in ["group", "interval"] {
            assert!(!offered.contains(&never.to_string()), "{offered:?}");
        }

        // A group with nothing in it offers nothing: there is no payload for a
        // value to mean anything on.
        assert!(document.group_fields("bay").is_empty());
    }

    /// A field whose payloads disagree says so.
    ///
    /// Different from nobody having one, and worth saying: typing into the
    /// field will overwrite several different values with one, and that is
    /// worth knowing before it happens rather than after.
    #[test]
    fn a_group_field_says_when_its_payloads_disagree() {
        let mut document = Document::new();
        document.apply(&Edit::AddGroup {
            name: "zone_temps".into(),
            interval: "3 s".into(),
        });
        for (name, address) in [("temp_1", "0x48"), ("temp_2", "0x49")] {
            document.apply(&add(name, "i2c"));
            for (attribute, value) in [
                ("group", "zone_temps"),
                ("address", address),
                ("path", "/dev/i2c-1"),
            ] {
                document.apply(&Edit::SetAttribute {
                    node: name.into(),
                    attribute: attribute.into(),
                    was: None,
                    now: Some(value.into()),
                });
            }
        }

        let fields = document.group_fields("zone_temps");
        let field = |name: &str| {
            fields
                .iter()
                .find(|f| f.attribute == name)
                .unwrap_or_else(|| panic!("{name} should be offered"))
        };

        // They agree about the bus.
        assert_eq!(field("path").value.as_deref(), Some("/dev/i2c-1"));
        assert!(!field("path").differs);
        // And not about the address, which is what makes them two sensors.
        assert!(field("address").differs);
        assert_eq!(field("address").value, None);
        // Nobody has a trigger, which is not the same as disagreeing.
        assert!(!field("trigger").differs);
        assert_eq!(field("trigger").value, None);
    }

    /// A document rebuilt from its history is the document that was there.
    ///
    /// The claim crash recovery rests on. Checked by building one document by
    /// applying edits and another by replaying the same edits, which is what
    /// happens after a crash, and requiring them to be equal.
    #[test]
    fn replaying_the_edits_rebuilds_the_same_document() {
        let edits = vec![
            add("temp_1", "i2c"),
            add("k_1", "compute"),
            Edit::SetAttribute {
                node: "temp_1".into(),
                attribute: "address".into(),
                was: None,
                now: Some("0x48".into()),
            },
            Edit::AddLink {
                link: Link::new("temp_1", "k_1", None),
            },
            Edit::MoveNode {
                node: "k_1".into(),
                was: (0, 0),
                now: (240, 80),
            },
        ];

        let mut live = Document::new();
        for edit in &edits {
            live.apply(edit);
        }

        let mut recovered = Document::new();
        for edit in &edits {
            recovered.apply(edit);
        }

        assert_eq!(live, recovered);
        assert_eq!(live.node("k_1").unwrap().at, (240, 80));
        assert_eq!(
            live.node("temp_1").unwrap().attributes.get("address"),
            Some(&"0x48".to_string())
        );
    }

    /// An edit and its inverse leave the document where it started.
    ///
    /// Every variant, because a single one that does not round-trip is a one
    /// press of undo that silently loses something.
    #[test]
    fn every_edit_undone_leaves_the_document_as_it_was() {
        let mut document = Document::new();
        document.apply(&add("a", "i2c"));
        document.apply(&add("b", "compute"));
        document.apply(&Edit::SetAttribute {
            node: "a".into(),
            attribute: "address".into(),
            was: None,
            now: Some("0x48".into()),
        });
        document.apply(&Edit::AddLink {
            link: Link::new("a", "b", Some("in0")),
        });

        let edits = [
            add("c", "spi"),
            Edit::SetAttribute {
                node: "a".into(),
                attribute: "address".into(),
                was: Some("0x48".into()),
                now: Some("0x49".into()),
            },
            Edit::SetAttribute {
                node: "a".into(),
                attribute: "pec".into(),
                was: None,
                now: Some("true".into()),
            },
            Edit::RenameNode {
                was: "a".into(),
                now: "sensor".into(),
            },
            Edit::MoveNode {
                node: "b".into(),
                was: (0, 0),
                now: (10, 10),
            },
            Edit::RemoveLink {
                link: Link::new("a", "b", Some("in0")),
            },
        ];

        for edit in edits {
            let before = document.clone();
            document.apply(&edit);
            document.apply(&edit.inverse());
            assert_eq!(
                document,
                before,
                "{} did not undo cleanly",
                edit.describe()
            );
        }
    }

    /// Removing a node takes its links, and undoing it brings them back.
    #[test]
    fn a_removed_node_takes_its_links_and_gets_them_back() {
        let mut document = Document::new();
        document.apply(&add("a", "i2c"));
        document.apply(&add("b", "compute"));
        let link = Link::new("a", "b", None);
        document.apply(&Edit::AddLink { link: link.clone() });

        // As the window would build it: the removal carries what is being
        // removed, which is what makes the inverse complete.
        let node = document.node("a").unwrap().clone();
        let removal = Edit::RemoveNode {
            name: node.name.clone(),
            node_type: node.node_type.clone(),
            attributes: node
                .attributes
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            links: vec![link.clone()],
            home: None,
        };

        document.apply(&removal);
        assert!(document.node("a").is_none());
        assert!(document.links.is_empty(), "a dangling link was left behind");

        document.apply(&removal.inverse());
        assert!(document.node("a").is_some());
        assert_eq!(document.links, vec![link], "the links did not come back");
    }

    /// Renaming a node renames what points at it.
    #[test]
    fn renaming_a_node_follows_through_its_links() {
        let mut document = Document::new();
        document.apply(&add("a", "i2c"));
        document.apply(&add("b", "compute"));
        document.apply(&Edit::AddLink {
            link: Link::new("a", "b", Some("in0")),
        });

        document.apply(&Edit::RenameNode {
            was: "a".into(),
            now: "temp_1".into(),
        });

        assert_eq!(document.links[0].from, "temp_1");
        // Its own fault and not every fault: these nodes have no attributes
        // filled in, which is a fault of its own and nothing to do with
        // renaming.
        assert!(
            !document
                .faults()
                .iter()
                .any(|f| matches!(f, Fault::DanglingLink { .. })),
            "renaming left a link dangling: {:?}",
            document.faults()
        );
    }

    /// The faults the editor finds, each of which is what red marks.
    #[test]
    fn a_document_says_what_is_wrong_with_it() {
        let mut document = Document::new();
        // Nothing is wrong with emptiness: a configuration with nothing in it
        // is where every configuration starts, and what it still has to say
        // before it flies is checked on the ground.
        assert!(document.faults().is_empty(), "{:?}", document.faults());

        document.apply(&add("a", "i2c"));
        document.apply(&add("a", "spi"));
        let faults = document.faults();
        assert!(
            faults.contains(&Fault::DuplicateName { name: "a".into() }),
            "{faults:?}"
        );

        let mut document = Document::new();
        document.apply(&add("a", "i2c"));
        document.apply(&Edit::AddLink {
            link: Link::new("a", "nowhere", None),
        });
        assert!(
            document
                .faults()
                .iter()
                .any(|f| matches!(f, Fault::DanglingLink { .. })),
            "{:?}",
            document.faults()
        );

        let mut document = Document::new();
        document.apply(&add("a", "contraption"));
        match document.faults().as_slice() {
            [Fault::UnknownType { node_type, .. }] => {
                assert_eq!(node_type, "contraption")
            }
            other => panic!("{other:?}"),
        }

        // Two links into one input, which is the fault that would otherwise
        // average a reading with itself.
        let mut document = Document::new();
        document.apply(&add("a", "i2c"));
        document.apply(&add("b", "spi"));
        document.apply(&add("m", "compute"));
        for from in ["a", "b"] {
            document.apply(&Edit::AddLink {
                link: Link::new(from, "m", Some("in0")),
            });
        }
        assert!(
            document
                .faults()
                .iter()
                .any(|f| matches!(f, Fault::InputBoundTwice { .. })),
            "{:?}",
            document.faults()
        );
    }

    /// Every kind of edit undoes itself, and the list cannot go stale.
    ///
    /// The `match` is the point of this test as much as the assertions are: it
    /// has an arm per variant and no wildcard, so a variant added later stops
    /// the build here until somebody puts it in `every_variant` below. A test
    /// that enumerated the variants in a list would simply go quiet instead.
    #[test]
    fn every_kind_of_edit_undoes_itself() {
        /// Names every variant of [`Edit`], so that a variant left out of the
        /// test stops the build.
        ///
        /// # Arguments
        ///
        /// * `edit` - An edit of any kind; nothing is done with it.
        fn exhaustive(edit: &Edit) {
            match edit {
                Edit::AddNode { .. } => (),
                Edit::RemoveNode { .. } => (),
                Edit::RestoreNode { .. } => (),
                Edit::SetAttribute { .. } => (),
                Edit::RenameNode { .. } => (),
                Edit::AddLink { .. } => (),
                Edit::RemoveLink { .. } => (),
                Edit::MoveLink { .. } => (),
                Edit::SetNodeComment { .. } => (),
                Edit::SetLinkComment { .. } => (),
                Edit::MoveNode { .. } => (),
                Edit::Arrange { .. } => (),
                Edit::Group { .. } => (),
                Edit::SetAside { .. } => (),
                Edit::SetExport { .. } => (),
                Edit::SetTcspecial { .. } => (),
                Edit::SetSimPayload { .. } => (),
                Edit::SetSimGroup { .. } => (),
                Edit::AddSimGroup { .. } => (),
                Edit::RemoveSimGroup { .. } => (),
                Edit::RestoreSimGroup { .. } => (),
                Edit::AddPhysics { .. } => (),
                Edit::RemovePhysics { .. } => (),
                Edit::RestorePhysics { .. } => (),
                Edit::SetPhysics { .. } => (),
                Edit::MovePhysics { .. } => (),
                Edit::AddConnection { .. } => (),
                Edit::RemoveConnection { .. } => (),
                Edit::AddGroup { .. } => (),
                Edit::RemoveGroup { .. } => (),
                Edit::SetGroupInterval { .. } => (),
                Edit::RenameGroup { .. } => (),
                Edit::SetGroupAttribute { .. } => (),
                Edit::RestoreGroupAttribute { .. } => (),
            }
        }

        // A document with something in it to edit, so that every variant has
        // a target: an edit applied to nothing is an edit that trivially
        // round-trips and proves nothing.
        let furnished = || {
            let mut document = Document::new();
            document.apply(&add("a", "i2c"));
            document.apply(&add("b", "compute"));
            document.apply(&Edit::SetAttribute {
                node: "a".into(),
                attribute: "address".into(),
                was: None,
                now: Some("0x48".into()),
            });
            document.apply(&Edit::AddLink {
                link: Link::new("a", "b", Some("in0")),
            });
            document.apply(&Edit::SetNodeComment {
                node: "a".into(),
                was: None,
                now: Some("the fore sensor".into()),
            });
            // A simulator configuration, so that the edits which set one of
            // its settings have somewhere to put them.
            document.simulator = Some(crate::sim::Simulator::new(
                std::path::Path::new("vehiclesim.yaml"),
            ));
            document.apply(&Edit::AddSimGroup {
                name: "steady_1hz".into(),
            });
            // A physics connection, so that taking one away has one to take.
            document.apply(&Edit::AddConnection {
                from: "heater_1.heat".into(),
                to: "zone.heat".into(),
            });
            document.apply(&Edit::SetSimGroup {
                group: "steady_1hz".into(),
                setting: "packet_interval".into(),
                was: None,
                now: Some("1 s".into()),
            });
            document.apply(&Edit::SetSimPayload {
                payload: "a".into(),
                setting: "drop_percent".into(),
                was: None,
                now: Some("10".into()),
            });
            // What tcspecial is configured with, so that the edits which set
            // one of its parameters have something to change.
            for (attribute, value) in
                [("port", "4000"), ("log.directory", "/var/log/tcspecial")]
            {
                document.apply(&Edit::SetTcspecial {
                    attribute: attribute.into(),
                    was: None,
                    now: Some(value.into()),
                });
            }
            // A group with a payload in it, so that the edits that act on a
            // group have something to act on: a group edit applied to no
            // members trivially round-trips and proves nothing.
            document.apply(&Edit::AddGroup {
                name: "zone_temps".into(),
                interval: "3 s".into(),
            });
            document.apply(&Edit::SetAttribute {
                node: "a".into(),
                attribute: "group".into(),
                was: None,
                now: Some("zone_temps".into()),
            });
            document
        };

        let every_variant = vec![
            add("c", "spi"),
            Edit::RemoveNode {
                name: "b".into(),
                node_type: "compute".into(),
                attributes: Vec::new(),
                links: vec![Link::new("a", "b", Some("in0"))],
                home: None,
            },
            Edit::RestoreNode {
                name: "d".into(),
                node_type: "log".into(),
                attributes: vec![("format".into(), "variable_ts_rc".into())],
                links: Vec::new(),
                home: None,
            },
            Edit::SetAttribute {
                node: "a".into(),
                attribute: "address".into(),
                was: Some("0x48".into()),
                now: Some("0x49".into()),
            },
            Edit::RenameNode {
                was: "a".into(),
                now: "temp_1".into(),
            },
            Edit::AddLink {
                link: Link::new("b", "a", None),
            },
            Edit::RemoveLink {
                link: Link::new("a", "b", Some("in0")),
            },
            Edit::MoveLink {
                was: Link::new("a", "b", Some("in0")),
                now: Link::new("a", "b", Some("in1")),
            },
            Edit::SetNodeComment {
                node: "a".into(),
                was: Some("the fore sensor".into()),
                now: Some("the fore sensor, 0x49 after the rework".into()),
            },
            Edit::SetLinkComment {
                link: Link::new("a", "b", Some("in0")),
                was: None,
                now: Some("raw counts, not kelvin".into()),
            },
            Edit::MoveNode {
                node: "a".into(),
                was: (0, 0),
                now: (120, 80),
            },
            // A concern set aside, which is a decision about the drawing and
            // is undone like any other.
            Edit::SetAside {
                node: "a".into(),
                kind: "world-out-goes-nowhere".into(),
                aside: true,
            },
            // A node shown at its subdiagram's edge, which its undo hides
            // again and takes the emptied list with it.
            Edit::SetExport {
                home: "zone".into(),
                terminal: "world-out".into(),
                node: "a".into(),
                exported: true,
            },
            // A whole layout in one edit: where each node was, and where the
            // arrangement put it. Both at (0, 0) to begin with, which is
            // where the furnished document above leaves them.
            Edit::Arrange {
                was: vec![("a".into(), (0, 0)), ("b".into(), (0, 0))],
                now: vec![("a".into(), (720, 0)), ("b".into(), (192, 0))],
            },
            Edit::SetTcspecial {
                attribute: "port".into(),
                was: Some("4000".into()),
                now: Some("4100".into()),
            },
            Edit::SetSimPayload {
                payload: "a".into(),
                setting: "drop_percent".into(),
                was: Some("10".into()),
                now: Some("20".into()),
            },
            Edit::SetSimGroup {
                group: "steady_1hz".into(),
                setting: "packet_interval".into(),
                was: Some("1 s".into()),
                now: Some("500".into()),
            },
            Edit::AddSimGroup {
                name: "continuous".into(),
            },
            Edit::RemoveSimGroup {
                name: "steady_1hz".into(),
                settings: vec![("packet_interval".into(), "1 s".into())],
            },
            Edit::RestoreSimGroup {
                name: "bay".into(),
                settings: vec![("packet_interval".into(), "250 ms".into())],
            },
            // A physics component put somewhere, from being laid out by
            // name; and a connection made, and one taken away.
            Edit::MovePhysics {
                component: "zone".into(),
                was: None,
                now: Some((-180, 98)),
            },
            Edit::AddConnection {
                from: "heater_2.heat".into(),
                to: "zone.heat".into(),
            },
            Edit::RemoveConnection {
                from: "heater_1.heat".into(),
                to: "zone.heat".into(),
            },
            // And one that clears a parameter, which takes the line out of
            // the file rather than writing it empty.
            Edit::SetTcspecial {
                attribute: "log.directory".into(),
                was: Some("/var/log/tcspecial".into()),
                now: None,
            },
            Edit::AddGroup {
                name: "bay".into(),
                interval: "5 s".into(),
            },
            Edit::RemoveGroup {
                name: "zone_temps".into(),
                interval: "3 s".into(),
            },
            Edit::SetGroupInterval {
                group: "zone_temps".into(),
                was: "3 s".into(),
                now: "1 s".into(),
            },
            Edit::RenameGroup {
                was: "zone_temps".into(),
                now: "zone".into(),
            },
            // One value for every payload of the group, carrying what each of
            // them had: `a` had no path, so undoing this takes the attribute
            // off again rather than leaving it empty.
            Edit::SetGroupAttribute {
                group: "zone_temps".into(),
                attribute: "path".into(),
                was: vec![("a".into(), None)],
                now: Some("/dev/i2c-1".into()),
            },
            Edit::RestoreGroupAttribute {
                group: "zone_temps".into(),
                attribute: "path".into(),
                was: None,
                now: vec![("a".into(), Some("/dev/i2c-1".into()))],
            },
            // A node placed, as the palette places one: added, given what its
            // entry answers, and moved to where it fits.
            Edit::Group {
                edits: vec![
                    add("e", "i2c"),
                    Edit::SetAttribute {
                        node: "e".into(),
                        attribute: "address".into(),
                        was: None,
                        now: Some("0x4a".into()),
                    },
                    Edit::MoveNode {
                        node: "e".into(),
                        was: (0, 0),
                        now: (40, 120),
                    },
                ],
            },
        ];

        for edit in &every_variant {
            exhaustive(edit);

            let mut document = furnished();
            let before = document.clone();
            document.apply(edit);
            document.apply(&edit.inverse());
            assert_eq!(
                document,
                before,
                "{} did not undo cleanly",
                edit.describe()
            );

            // And an undo followed by a redo lands where the edit did, which
            // is the other half of what undo has to mean.
            let mut document = furnished();
            document.apply(edit);
            let after_edit = document.clone();
            document.apply(&edit.inverse());
            document.apply(edit);
            assert_eq!(
                document,
                after_edit,
                "{} did not redo cleanly",
                edit.describe()
            );
        }
    }

    /// A comment survives being written, and belongs to its own object.
    #[test]
    fn comments_are_kept_on_nodes_and_on_links() {
        let mut document = Document::new();
        document.apply(&add("temp_1", "i2c"));
        document.apply(&add("k_1", "compute"));
        document.apply(&Edit::AddLink {
            link: Link::new("temp_1", "k_1", None),
        });
        document.apply(&Edit::SetNodeComment {
            node: "temp_1".into(),
            was: None,
            now: Some("0x49 after the rework; see ECR-114".into()),
        });
        document.apply(&Edit::SetLinkComment {
            link: Link::new("temp_1", "k_1", None),
            was: None,
            now: Some("raw counts, not kelvin".into()),
        });

        let yaml = document.to_yaml();
        assert!(
            yaml.contains("# 0x49 after the rework; see ECR-114"),
            "the node's comment is not in the file: {yaml}"
        );
        assert!(
            yaml.contains("# raw counts, not kelvin"),
            "the link's comment is not in the file: {yaml}"
        );

        let xml = document.to_xml();
        assert!(
            xml.contains("<!-- 0x49 after the rework; see ECR-114 -->"),
            "{xml}"
        );
        assert!(xml.contains("<!-- raw counts, not kelvin -->"), "{xml}");
    }

    /// A comment on a link is not part of which link it is.
    ///
    /// The trap this guards: if a comment were part of a link's identity, then
    /// commenting on a link and then removing it would leave the original
    /// uncommented link behind, and the diagram would show two.
    #[test]
    fn commenting_on_a_link_does_not_make_a_second_one() {
        let mut document = Document::new();
        document.apply(&add("a", "i2c"));
        document.apply(&add("b", "compute"));
        document.apply(&Edit::AddLink {
            link: Link::new("a", "b", None),
        });
        document.apply(&Edit::SetLinkComment {
            link: Link::new("a", "b", None),
            was: None,
            now: Some("why this goes here".into()),
        });
        assert_eq!(document.links.len(), 1);

        // Adding it again, as a drag that ended where it started might,
        // leaves the comment alone rather than replacing the link with a bare
        // one.
        document.apply(&Edit::AddLink {
            link: Link::new("a", "b", None),
        });
        assert_eq!(document.links.len(), 1);
        assert_eq!(
            document.links[0].comment.as_deref(),
            Some("why this goes here"),
            "adding an existing link dropped what was written about it"
        );

        // And removing it needs no comment to match.
        document.apply(&Edit::RemoveLink {
            link: Link::new("a", "b", None),
        });
        assert!(document.links.is_empty());
    }

    /// A multi-line comment stays several lines, in both formats.
    #[test]
    fn a_comment_of_several_lines_is_written_as_several() {
        let mut document = Document::new();
        document.apply(&add("a", "i2c"));
        document.apply(&Edit::SetNodeComment {
            node: "a".into(),
            was: None,
            now: Some("two things:\nthe second one".into()),
        });

        let yaml = document.to_yaml();
        assert!(yaml.contains("  # two things:"), "{yaml}");
        assert!(yaml.contains("  # the second one"), "{yaml}");
    }

    /// A comment is written as it was given, however long its lines, in
    /// both formats: a writer that broke a comment again to a width of its
    /// own would change every comment it was not asked to.
    ///
    /// In XML as one comment, its lines under the first line's text, with
    /// the one change XML makes: see `comment_safe`.
    #[test]
    fn a_long_comment_is_written_as_it_was_given() {
        let long = "the second of the two sensors on the forward bulkhead, \
                    which was moved there after the rework & is read at its \
                    own address <see ECR-114> now that the first one shares \
                    the bus -- for now";
        let mut document = Document::new();
        document.apply(&add("a", "i2c"));
        document.apply(&Edit::SetNodeComment {
            node: "a".into(),
            was: None,
            now: Some(format!("{long}\n\n  an indented line")),
        });

        let yaml = document.to_yaml();
        assert!(
            yaml.contains(&format!(
                "      # {long}\n      #\n      #   an indented line\n"
            )),
            "{yaml}"
        );
        let xml = document.to_xml();
        assert!(
            xml.contains(&format!(
                "            <!-- {}\n\n                   an indented \
                 line -->\n",
                long.replace("--", "-")
            )),
            "{xml}"
        );
    }

    /// The worked example's comments are composed as paragraphs, and broken
    /// to fit eighty columns where they are written: by words, an indented
    /// line keeping its indent, a word longer than a line left whole.
    #[test]
    fn the_worked_examples_comments_are_broken_by_words() {
        let long = "the second of the two sensors on the forward bulkhead, \
                    which was moved there after the rework & is read at its \
                    own address <see ECR-114> now that the first one shares \
                    the bus";
        let lines = comment_lines(long, 8);
        assert!(lines.len() > 1, "{lines:?}");
        assert_eq!(lines.join(" "), long, "only the line breaks change");
        assert!(lines.iter().all(|l| l.len() + 8 + 9 <= COMMENT_LINE));

        let indented = comment_lines(
            "  an indented line that goes on and on and on well past where a \
             line should stop",
            4,
        );
        assert!(indented.len() > 1, "{indented:?}");
        assert!(
            indented.iter().all(|line| line.starts_with("  ")),
            "{indented:?}"
        );

        let word = "x".repeat(100);
        assert_eq!(comment_lines(&word, 4), vec![word.clone()]);
    }

    /// The format follows the file's name.
    #[test]
    fn the_extension_decides_the_format() {
        use std::path::Path;

        let mut document = Document::new();
        document.apply(&add("a", "i2c"));

        let yaml = document.to_text(Path::new("graph.ytcs")).expect("yaml");
        // A YAML configuration opens by naming its schema, which is a
        // comment, and states no version: the loader would refuse one.
        assert!(
            yaml.starts_with("# yaml-language-server: $schema="),
            "{yaml}"
        );
        assert!(!yaml.contains("version:"), "{yaml}");

        let xml = document.to_text(Path::new("graph.xtcs")).expect("xml");
        assert!(xml.starts_with("<?xml"), "{xml}");

        // And anything else is refused rather than guessed at.
        let said = document
            .to_text(Path::new("graph.json"))
            .expect_err("an unknown extension should be refused");
        assert!(said.contains(".ytcs"), "{said}");
        let said = document
            .to_text(Path::new("graph"))
            .expect_err("no extension should be refused");
        assert!(said.contains(".ytcs"), "{said}");
        // A name of the kind every configuration once had is told the
        // name it should have.
        let said = document
            .to_text(Path::new("graph.yaml"))
            .expect_err(".yaml should be refused");
        assert!(said.contains("rename it graph.ytcs"), "{said}");
    }

    /// A comment with markup in it is written as it is, XML reading no
    /// markup and no entity inside a comment, and reads back as it was.
    #[test]
    fn a_comment_with_markup_in_it_is_still_a_comment() {
        let mut document = Document::new();
        document.apply(&add("a", "i2c"));
        document.apply(&Edit::SetNodeComment {
            node: "a".into(),
            was: None,
            now: Some("sampled at 1Hz & logged <always>".into()),
        });

        let xml = document.to_xml();
        assert!(
            xml.contains("<!-- sampled at 1Hz & logged <always> -->"),
            "{xml}"
        );
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("a.xtcs");
        std::fs::write(&path, &xml).expect("written");
        let read = crate::read::read(&path).expect("it reads");
        assert_eq!(
            read.node("a").and_then(|a| a.comment.as_deref()),
            Some("sampled at 1Hz & logged <always>")
        );
    }

    /// A configuration that exists in two spellings is written in both.
    ///
    /// The shipped example is a pair, and two files meant to say the same
    /// thing drift the moment only one of them is written: keeping them a pair
    /// by hand meant a command to remember after every session. Only where the
    /// sibling is already there, because writing one that is not would be the
    /// editor deciding somebody wants their configuration in two languages.
    #[test]
    fn a_configuration_in_two_spellings_is_written_in_both() {
        let where_ = tempfile::tempdir().expect("a directory");
        let yaml = where_.path().join("vehicle.ytcs");
        let xml = where_.path().join("vehicle.xtcs");

        let mut document = Document::new();
        document.apply(&add("sensor", "i2c"));

        // Only the file it was given, there being no sibling yet.
        let files = document.files(&yaml).expect("the files");
        assert_eq!(files.len(), 1, "{files:?}");
        assert_eq!(files[0].0, yaml);

        // With one beside it, both -- each in its own language.
        std::fs::write(&xml, "<configuration/>\n").expect("a sibling");
        let files = document.files(&yaml).expect("the files");
        assert_eq!(files.len(), 2, "{files:?}");
        let written: Vec<&std::path::PathBuf> =
            files.iter().map(|(path, _)| path).collect();
        assert!(
            written.contains(&&yaml) && written.contains(&&xml),
            "{written:?}"
        );
        for (path, text) in &files {
            if path == &xml {
                assert!(text.starts_with("<?xml"), "{text}");
            } else {
                assert!(
                    text.starts_with("# yaml-language-server: $schema="),
                    "{text}"
                );
                assert!(!text.contains("version:"), "{text}");
            }
            assert!(text.contains("sensor"), "{text}");
        }

        // And the other way about, which is the same rule read backwards.
        std::fs::write(&yaml, "description: a sibling\n").expect("a sibling");
        let files = document.files(&xml).expect("the files");
        assert_eq!(files.len(), 2, "{files:?}");
    }

    /// Terminators are written as a list -- a YAML sequence, an XML element
    /// for each -- and read back as the one value the editor holds them in,
    /// in either format.
    #[test]
    fn terminators_are_written_as_a_list_and_read_back() {
        let mut document = Document::new();
        document.apply(&add("console", "serial"));
        document.apply(&Edit::SetAttribute {
            node: "console".into(),
            attribute: "terminators".into(),
            was: None,
            now: Some("\"\\r\\n\", [03]".into()),
        });

        let yaml = document.to_yaml();
        assert!(
            yaml.contains(
                "        terminators:\n          - \
                           \"\\\"\\\\r\\\\n\\\"\"\n          - \"[03]\"\n"
            ),
            "{yaml}"
        );
        let xml = document.to_xml();
        assert!(
            xml.contains("<terminators>\"\\r\\n\"</terminators>")
                || xml
                    .contains("<terminators>&quot;\\r\\n&quot;</terminators>"),
            "{xml}"
        );
        assert!(xml.contains("<terminators>[03]</terminators>"), "{xml}");

        let where_ = tempfile::tempdir().expect("a directory");
        for (name, text) in [("c.ytcs", &yaml), ("c.xtcs", &xml)] {
            let path = where_.path().join(name);
            std::fs::write(&path, text).expect("written");
            let graph = tcslibgs::graph::Graph::load(&path).expect("read back");
            let console = graph.node("console").expect("the node");
            assert_eq!(
                console.attributes["terminators"].as_str(),
                "\"\\r\\n\", [03]",
                "{name}"
            );
        }
    }

    /// A line is written at both of the nodes it joins.
    ///
    /// On the node it leaves, as the node it reaches and the name it arrives
    /// by; and on the node it arrives at, which lists that name under the sink
    /// it arrives at. Both spellings, because a configuration saved under the
    /// other name must say the same thing.
    #[test]
    fn a_link_is_written_at_both_of_its_ends() {
        use crate::journal::Link;

        let mut document = Document::new();
        document.apply(&add("temp_1", "i2c"));
        document.apply(&add("gateway", "gateway_radio"));
        document.apply(&Edit::AddLink {
            link: Link::joining("temp_1", Some("world-out"), "gateway", None),
        });

        let yaml = document.to_yaml();
        assert!(
            yaml.contains("        world-out:\n          - gateway.temp_1\n"),
            "{yaml}"
        );
        assert!(
            yaml.contains("        world-in: [temp_1]\n"),
            "the arriving end does not list the name: {yaml}"
        );

        let xml = document.to_xml();
        assert!(
            xml.contains("<world-out>gateway.temp_1</world-out>"),
            "{xml}"
        );
        assert!(
            xml.contains("<world-in>temp_1</world-in>"),
            "the arriving end does not list the name: {xml}"
        );

        // And the record is of the stream, not of the terminal somebody drew
        // from: what leaves by world-out arrives at world-in and nowhere
        // else, so the payload's own sink says nothing about this link.
        assert!(!yaml.contains("moc-in"), "{yaml}");
    }

    /// A sink several lines arrive at lists each line's name once, in name
    /// order.
    ///
    /// The ground's case, which is every configuration: each payload aboard
    /// reports to the one gateway node, told apart by the socket its id
    /// names.
    #[test]
    fn a_sink_that_several_links_arrive_at_says_so_once() {
        use crate::journal::Link;

        let mut document = Document::new();
        document.apply(&add("gateway", "gateway_radio"));
        for name in ["temp_2", "temp_1"] {
            document.apply(&add(name, "i2c"));
            document.apply(&Edit::AddLink {
                link: Link::joining(name, Some("world-out"), "gateway", None),
            });
        }

        let yaml = document.to_yaml();
        assert_eq!(
            yaml.matches("world-in: [temp_1, temp_2]\n").count(),
            1,
            "once, for the sink and not for each line: {yaml}"
        );
    }

    /// What might be wrong is raised; what is meant can be set aside.
    ///
    /// A configuration with one node raises nothing.
    ///
    /// The ground station the editor places in a new configuration is one
    /// node with nothing to link it to, so every rule in `concerns` has
    /// something to say about it the moment it exists -- which is a complaint
    /// about the editor's own first act, and nothing a person can act on.
    #[test]
    fn nothing_is_raised_about_a_graph_that_is_one_node() {
        let mut document = Document::new();
        assert!(document.concerns().is_empty(), "nothing at all");

        document.apply(&Edit::AddNode {
            name: "gateway-1".to_string(),
            node_type: "gateway_radio".to_string(),
            home: None,
        });
        assert!(
            document.concerns().is_empty(),
            "the ground station alone: {:?}",
            document.concerns()
        );

        // And the rules come back as soon as there is a graph for them: a
        // payload and a ground station with nothing between them is two
        // nodes, and the payload's readings reach nobody.
        document.apply(&Edit::AddNode {
            name: "i2c-1".to_string(),
            node_type: "i2c".to_string(),
            home: None,
        });
        let raised = document.concerns();
        assert!(
            raised.iter().any(|concern| concern.node() == "i2c-1"),
            "two nodes and no line between them: {raised:?}"
        );
    }

    /// A duplicate name is put right by position, and the links stay with
    /// whoever keeps the name.
    #[test]
    fn one_of_two_nodes_sharing_a_name_can_be_renamed() {
        let mut document = Document::new();
        for name in ["i2c", "i2c", "gateway"] {
            document.apply(&Edit::AddNode {
                name: name.to_string(),
                node_type: if name == "gateway" {
                    "gateway_radio"
                } else {
                    "i2c"
                }
                .to_string(),
                home: None,
            });
        }
        document.apply(&Edit::AddLink {
            link: Link::new("i2c", "gateway", None),
        });

        // Both of them are offered, because the file does not say which was
        // meant. The ground station is not.
        assert_eq!(document.sharing_a_name(), vec![0, 1]);

        // A name derived from the one that is taken, and free.
        assert_eq!(document.spare_name(1), "i2c-1");

        // The second one renamed: the first keeps the name, so the link --
        // which named that name -- stays where it was.
        document.rename_at(1, "i2c-1");
        assert_eq!(document.nodes[0].name, "i2c");
        assert_eq!(document.nodes[1].name, "i2c-1");
        assert_eq!(document.links[0].from, "i2c");
        assert!(document.sharing_a_name().is_empty());

        // And now that nobody shares it, renaming the holder takes the link
        // with it, as any other rename does.
        document.rename_at(0, "i2c-2");
        assert_eq!(document.links[0].from, "i2c-2");

        // A derived name steps the number rather than adding another: a file
        // with two `i2c-2` nodes should offer another number, not `i2c-2-1`.
        document.apply(&Edit::AddNode {
            name: "i2c-2".to_string(),
            node_type: "i2c".to_string(),
            home: None,
        });
        assert_eq!(document.sharing_a_name(), vec![0, 3]);
        assert_eq!(document.spare_name(3), "i2c-3");
    }

    /// A node in a subdiagram renamed by position takes its simulated
    /// settings with it, held under its own name before and after: the
    /// simulator's file names a payload so wherever it is drawn.
    #[test]
    fn a_payload_in_a_subdiagram_renamed_by_position_keeps_its_settings() {
        let mut document = Document::new();
        let home = Some("zone".to_string());
        document.apply(&Edit::AddNode {
            name: "temp_1".to_string(),
            node_type: "i2c".to_string(),
            home: home.clone(),
        });
        document.simulator = Some(crate::sim::Simulator::new(
            std::path::Path::new("vehiclesim.yaml"),
        ));
        document.apply(&Edit::SetSimPayload {
            payload: crate::sim::Simulator::key_of("zone/temp_1"),
            setting: "packet_interval".to_string(),
            was: None,
            now: Some("1 s".to_string()),
        });

        document.rename_at(0, "temp_2");
        let payloads = &document.simulator.as_ref().expect("one").payloads;
        assert_eq!(payloads.keys().collect::<Vec<_>>(), vec!["temp_2"]);
        assert_eq!(payloads["temp_2"]["packet_interval"], "1 s");
    }

    /// Two pages may each hold a node of the same name, which is not a
    /// duplicate.
    #[test]
    fn a_name_is_only_shared_within_one_home() {
        let mut document = Document::new();
        for home in [None, Some("zone".to_string()), Some("bay".to_string())] {
            document.apply(&Edit::AddNode {
                name: "temp_1".to_string(),
                node_type: "i2c".to_string(),
                home,
            });
        }
        assert!(
            document.sharing_a_name().is_empty(),
            "three pages, three nodes, one name apiece"
        );

        // And inside one page it is a duplicate like any other.
        document.apply(&Edit::AddNode {
            name: "temp_1".to_string(),
            node_type: "i2c".to_string(),
            home: Some("zone".to_string()),
        });
        assert_eq!(document.sharing_a_name(), vec![1, 3]);
        assert_eq!(document.spare_name(3), "temp_1-1");
    }

    /// Three shapes, and which rules apply depends on the pane: a payload is
    /// where the payload's own data begins and a gateway link where it ends, so
    /// those two are asked whether what they send reaches anything, and a
    /// vehicle node is asked whether a stream passing through it comes out
    /// somewhere.
    #[test]
    fn what_might_be_wrong_is_raised_and_can_be_set_aside() {
        use crate::journal::Link;

        let mut document = Document::new();
        for (name, node_type) in [
            ("sensor", "i2c"),
            ("ground", "gateway_radio"),
            ("zone_temp", "compute"),
            ("housekeeping", "log"),
        ] {
            document.apply(&add(name, node_type));
        }

        // Nothing drawn: the payload's readings reach nothing and so does
        // what the ground sends. The two vehicle nodes are not accused of
        // anything -- a node nothing is linked to either way is a node
        // somebody has just placed.
        let said: Vec<String> =
            document.concerns().iter().map(|c| c.kind()).collect();
        assert_eq!(
            said,
            vec![
                "world-out-goes-nowhere".to_string(),
                "moc-out-goes-nowhere".to_string()
            ],
            "{:?}",
            document.concerns()
        );

        // The readings reach a computation: the payload is answered, and the
        // computation is now one a stream stops at.
        document.apply(&Edit::AddLink {
            link: Link::joining(
                "sensor",
                Some("world-out"),
                "zone_temp",
                Some("in0"),
            ),
        });
        let said: Vec<String> =
            document.concerns().iter().map(|c| c.kind()).collect();
        assert!(
            !said.contains(&"world-out-goes-nowhere".to_string()),
            "{said:?}"
        );
        assert!(said.contains(&"nothing-leaves".to_string()), "{said:?}");

        // And on to the ground, which is the shape the whole graph is: a
        // payload, vehicle nodes, a gateway link.
        document.apply(&Edit::AddLink {
            link: Link::joining("zone_temp", Some("world-out"), "ground", None),
        });
        let said: Vec<String> =
            document.concerns().iter().map(|c| c.kind()).collect();
        assert_eq!(
            said,
            vec!["moc-out-goes-nowhere".to_string()],
            "{:?}",
            document.concerns()
        );

        // A log is where a stream stops on purpose: something arrives and
        // nothing leaves, and that is what a log is for.
        document.apply(&Edit::AddLink {
            link: Link::joining(
                "zone_temp",
                Some("world-out"),
                "housekeeping",
                None,
            ),
        });
        assert!(
            !document
                .concerns()
                .iter()
                .any(|c| c.node() == "housekeeping"),
            "{:?}",
            document.concerns()
        );

        // What is left is the ground's own: nothing is listening to what it
        // sends. Set aside, it leaves the first list for the second.
        let concern = document.concerns().first().cloned().expect("one left");
        document.apply(&Edit::SetAside {
            node: concern.node().to_string(),
            kind: concern.kind(),
            aside: true,
        });
        let (raised, aside) = document.concerns_apart();
        assert!(raised.is_empty(), "{raised:?}");
        assert_eq!(aside.len(), 1);
        assert_eq!(aside[0].node(), "ground");

        // Taken up again, it comes back.
        document.apply(&Edit::SetAside {
            node: concern.node().to_string(),
            kind: concern.kind(),
            aside: false,
        });
        assert_eq!(document.concerns_apart().0.len(), 1);
    }

    /// A concern that stops applying leaves both lists by itself.
    ///
    /// What keeps the set-aside list from filling up with decisions about
    /// shapes that are no longer there: it is remembered by node and kind, and
    /// a kind that no longer holds is not raised, so nothing has to be tidied
    /// up after a line is drawn.
    #[test]
    fn a_concern_that_stops_applying_is_not_raised_or_remembered() {
        use crate::journal::Link;

        let mut document = Document::new();
        document.apply(&add("sensor", "i2c"));
        document.apply(&add("ground", "gateway_radio"));
        let concern = document
            .concerns()
            .into_iter()
            .find(|c| c.node() == "sensor")
            .expect("the payload's readings reach nothing");
        document.apply(&Edit::SetAside {
            node: concern.node().to_string(),
            kind: concern.kind(),
            aside: true,
        });
        assert_eq!(document.concerns_apart().1.len(), 1);

        // Drawn, and it is gone from both lists -- while the decision is
        // still in the document, which is what makes it come back if the line
        // is deleted.
        document.apply(&Edit::AddLink {
            link: Link::joining("sensor", Some("world-out"), "ground", None),
        });
        let (raised, aside) = document.concerns_apart();
        assert!(
            !raised
                .iter()
                .chain(aside.iter())
                .any(|c| c.node() == "sensor"),
            "{raised:?} {aside:?}"
        );
        // The decision is still in the document, which is what makes it come
        // back if the line is deleted.
        assert!(document.aside.contains(&concern.key()));
    }

    /// A set-aside is written in the editor's own section and read back.
    #[test]
    fn a_set_aside_survives_being_written() {
        let mut document = Document::new();
        document.apply(&add("sensor", "i2c"));
        document.apply(&Edit::SetAside {
            node: "sensor".into(),
            kind: "world-out-goes-nowhere".into(),
            aside: true,
        });

        let yaml = document.to_yaml();
        assert!(
            yaml.contains(
                "  set-aside:\n    - { node: sensor, concern: \
                 world-out-goes-nowhere }"
            ),
            "{yaml}"
        );
        let xml = document.to_xml();
        assert!(
            xml.contains(concat!(
                "<set-aside>\n",
                "            <node name=\"sensor\" ",
                "concern=\"world-out-goes-nowhere\"/>\n",
                "        </set-aside>"
            )),
            "{xml}"
        );
    }

    /// Arranging puts every node at the top of its own pane, clear of the
    /// others.
    ///
    /// The command for a configuration spread over more sheet than a window
    /// shows -- which is how the shipped example reads after a session of
    /// dragging: three payloads 650 pixels apart down a pane 240 wide, so one
    /// of them is in view at a time.
    #[test]
    fn arranging_brings_every_node_to_the_top_of_its_pane() {
        let dividers = crate::pane::default_dividers();
        let mut document = Document::new();
        for (name, node_type, at) in [
            ("gateway", "gateway_radio", (24, 1800)),
            ("mean", "compute", (272, 288)),
            ("payload", "network", (744, 160)),
            ("sensor", "i2c", (720, 432)),
            ("other", "i2c", (720, 1600)),
        ] {
            document.apply(&add(name, node_type));
            document.apply(&Edit::MoveNode {
                node: name.to_string(),
                was: (0, 0),
                now: at,
            });
        }

        let arranged = document.arranged(dividers);
        assert!(!arranged.is_empty(), "there is plenty to tidy");
        document.apply(&Edit::Arrange {
            was: document.where_each_is(&arranged),
            now: arranged,
        });

        // Each in its own pane, and none of them below the first screenful:
        // the point of the command is that everything is in view.
        for node in &document.nodes {
            let (left, right) =
                crate::pane::pane_for(&node.node_type, dividers);
            assert!(
                node.at.0 >= left && node.at.0 <= right.max(left),
                "{} is at {:?}, outside {left}..{right}",
                node.name,
                node.at
            );
            assert!(node.at.1 < 400, "{} is at {:?}", node.name, node.at);
        }

        // And nothing covering anything.
        for (one, node) in document.nodes.iter().enumerate() {
            let (width, height) = size_of(&node.node_type);
            for (two, theirs) in document.nodes.iter().enumerate() {
                if one == two {
                    continue;
                }
                let (their_width, their_height) = size_of(&theirs.node_type);
                let over = node.at.0 < theirs.at.0 + their_width as i32
                    && theirs.at.0 < node.at.0 + width as i32
                    && node.at.1 < theirs.at.1 + their_height as i32
                    && theirs.at.1 < node.at.1 + height as i32;
                assert!(!over, "{} covers {}", node.name, theirs.name);
            }
        }

        // Asking again changes nothing: an arranged diagram is arranged.
        assert!(
            document.arranged(dividers).is_empty(),
            "{:?}",
            document.arranged(dividers)
        );
    }

    /// Arranging lays a configuration out the way its data flows, and a
    /// second arrangement changes nothing.
    ///
    /// The worked thermostat: its three sensors in the payload pane, each
    /// level with the conversion it feeds; the conversions in the vehicle
    /// pane's first column, the vote to their right and the thermostat to
    /// the right of that; the ground in its own pane.
    #[test]
    fn arranging_follows_the_data_and_settles() {
        let dividers = crate::pane::default_dividers();
        let mut document = thermostat_example();
        let now = document.arranged(dividers);
        document.apply(&Edit::Arrange {
            was: document.where_each_is(&now),
            now,
        });
        let at = |name: &str| document.node(name).expect(name).at;

        for (sensor, conversion) in [
            ("temp_1", "temp1"),
            ("temp_2", "temp2"),
            ("temp_3", "temp3"),
        ] {
            assert_eq!(at(sensor).1, at(conversion).1, "{sensor}");
            assert!(at(sensor).0 < at(conversion).0, "{sensor}");
        }
        assert!(at("temp1").0 < at("ambient").0);
        assert!(at("ambient").0 < at("thermostat").0);
        for node in &document.nodes {
            let (left, right) = crate::pane::edges_of(
                crate::pane::pane_of(&node.node_type),
                dividers,
            );
            let (width, _) = size_of(&node.node_type);
            assert!(
                node.at.0 >= left && node.at.0 + width as i32 <= right,
                "{} is at {:?}, outside {left}..{right}",
                node.name,
                node.at
            );
        }
        assert_eq!(document.outside_panes(&None, dividers), 0);

        assert_eq!(document.arranged(dividers), Vec::new());
    }

    /// A configuration of several pages settles too: each page is arranged
    /// on its own, lines to another page left out of where a node goes, and
    /// a second arrangement moves nothing.
    #[test]
    fn a_configuration_of_pages_settles_when_arranged() {
        let dividers = crate::pane::default_dividers();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let path = root.join("examples/thermostat-yamcs/thermostat-yamcs.ytcs");
        let mut document = crate::read::read(&path).expect("it reads");
        let now = document.arranged(dividers);
        assert!(!now.is_empty());
        document.apply(&Edit::Arrange {
            was: document.where_each_is(&now),
            now,
        });
        assert_eq!(document.arranged(dividers), Vec::new());
    }

    /// A node outside its own pane, across, is counted, which is what the
    /// Arrange button is emphasised for; one inside it is not, however far
    /// down the sheet it is.
    #[test]
    fn a_node_outside_its_pane_is_counted() {
        let dividers = crate::pane::default_dividers();
        let mut document = Document::new();
        document.apply(&add("sensor", "i2c"));
        document.apply(&Edit::MoveNode {
            node: "sensor".to_string(),
            was: (0, 0),
            now: (34, 2000),
        });
        assert_eq!(document.outside_panes(&None, dividers), 0);

        // Overhanging the divider into the vehicle's pane.
        document.apply(&Edit::MoveNode {
            node: "sensor".to_string(),
            was: (34, 2000),
            now: (dividers.payloads - 20, 34),
        });
        assert_eq!(document.outside_panes(&None, dividers), 1);
    }

    /// One undo puts a hand-made layout back.
    ///
    /// The thing that makes the command safe to try: somebody who arranged a
    /// diagram by hand and then tidied it by accident gets their arrangement
    /// back with one press, not one press per node.
    #[test]
    fn one_undo_takes_back_a_whole_arrangement() {
        let dividers = crate::pane::default_dividers();
        let mut document = Document::new();
        let before: Vec<(String, (i32, i32))> = [
            ("gateway", "gateway_radio", (24, 1800)),
            ("sensor", "i2c", (720, 900)),
            ("other", "i2c", (800, 1400)),
        ]
        .iter()
        .map(|(name, node_type, at)| {
            document.apply(&add(name, node_type));
            document.apply(&Edit::MoveNode {
                node: name.to_string(),
                was: (0, 0),
                now: *at,
            });
            (name.to_string(), *at)
        })
        .collect();

        let now = document.arranged(dividers);
        let edit = Edit::Arrange {
            was: document.where_each_is(&now),
            now,
        };
        document.apply(&edit);
        assert_ne!(
            document.node("sensor").expect("it").at,
            (720, 900),
            "nothing was arranged"
        );

        document.apply(&edit.inverse());
        for (name, at) in &before {
            assert_eq!(
                document.node(name).expect("it").at,
                *at,
                "{name} was not put back"
            );
        }
    }

    /// The shipped example is what this editor writes.
    ///
    /// Two files, the same configuration in each, as every example in this
    /// repository is kept. Compared rather than described, so an example
    /// cannot quietly stop being what the program produces -- which is the
    /// failure that makes an example worse than none.
    ///
    /// Set TCSGUI_WRITE_EXAMPLE to regenerate them after a deliberate change
    /// to the format.
    #[test]
    fn the_shipped_example_is_what_the_editor_writes() {
        use std::path::Path;

        let document = thermostat_example();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

        for name in [
            "tests/manual/thermostat.ytcs",
            "tests/manual/thermostat.xtcs",
        ] {
            let path = root.join(name);
            // Written for the place it is going, which is what decides how it
            // names its schema: see `nodes::schema_reference`.
            let written =
                document.to_text(&path).expect("a format this writes");

            if std::env::var("TCSGUI_WRITE_EXAMPLE").is_ok() {
                std::fs::write(&path, &written)
                    .expect("the example is writable");
                continue;
            }

            let shipped = std::fs::read_to_string(&path).unwrap_or_else(|e| {
                panic!(
                    "{}: {e}. Run the tests with TCSGUI_WRITE_EXAMPLE=1 to \
                     write the examples out",
                    path.display()
                )
            });
            assert_eq!(
                shipped,
                written,
                "{} is not what the editor writes any more",
                path.display()
            );
        }
    }

    /// The two shipped formats describe the same configuration.
    ///
    /// Not a comparison of the text, which differs, but of what each says:
    /// the same nodes with the same attributes and the same links. The YAML
    /// and the XML of an example drifting apart is the thing this catches.
    #[test]
    fn the_two_shipped_formats_say_the_same_thing() {
        let document = thermostat_example();
        let yaml = document.to_yaml();
        let xml = document.to_xml();

        for node in &document.nodes {
            let word = section_of(&node.node_type).node_word();
            assert!(
                yaml.contains(&format!("- {word}: {}\n", node.name)),
                "{}",
                node.name
            );
            assert!(
                xml.contains(&format!("<{word} name=\"{}\">", node.name)),
                "{} is missing from the XML",
                node.name
            );
            // A vote's inputs are its world-in, which is written as that.
            for (attribute, value) in node
                .attributes
                .iter()
                .filter(|(attribute, _)| !in_a_sink(node, attribute))
            {
                assert!(
                    yaml.contains(&format!("{attribute}: \"{value}\"")),
                    "{}.{attribute} is missing from the YAML",
                    node.name
                );
                assert!(
                    xml.contains(&format!(
                        "<{attribute}>{value}</{attribute}>"
                    )),
                    "{}.{attribute} is missing from the XML",
                    node.name
                );
            }
            if let Some(comment) = &node.comment {
                // Its first line as it is written, broken to fit where the
                // node's comment goes.
                let lines = comment_lines(comment, 12);
                let first = lines.first().expect("a comment has a line");
                assert!(
                    yaml.contains(first),
                    "a comment is missing from the YAML"
                );
                // The XML spelling folds a run of hyphens to one, there being
                // no way to write two of them inside a comment: see
                // `comment_safe`.
                assert!(
                    xml.contains(&comment_safe(first)),
                    "a comment is missing from the XML"
                );
            }
        }

        // Every link, written as an attribute of the node it leaves: the
        // terminal is the attribute's name, and the destination its value --
        // alone where the node has one, inside a list where it has several.
        // Both spellings say it, each in its own way.
        for link in &document.links {
            // The node it reaches and the name it arrives by, in both.
            let line = format!("{}.{}", link.to, link.arrives_by());
            let yaml_said = format!("- {line}\n");
            let xml_said = format!(">{line}</");
            assert!(
                yaml.contains(&yaml_said),
                "{} to {} is missing from the YAML",
                link.from,
                link.to
            );
            assert!(
                xml.contains(&xml_said),
                "{} to {} is missing from the XML",
                link.from,
                link.to
            );
        }
    }

    /// A node placed in a subdiagram belongs to it, and is named through it.
    #[test]
    fn a_subdiagrams_nodes_are_its_own() {
        let mut document = Document::new();
        document.apply(&Edit::AddNode {
            name: "zone".into(),
            node_type: "subdiagram".into(),
            home: None,
        });
        document.apply(&Edit::AddNode {
            name: "temp_1".into(),
            node_type: "i2c".into(),
            home: Some("zone".into()),
        });
        document.apply(&Edit::AddNode {
            name: "gateway".into(),
            node_type: "gateway_radio".into(),
            home: None,
        });

        assert_eq!(document.subdiagrams(), vec!["zone".to_string()]);
        assert_eq!(document.nodes_in(&None).len(), 2, "zone and gateway");
        assert_eq!(document.nodes_in(&Some("zone".into())).len(), 1);

        // Named through its home from outside, plainly from inside.
        assert!(document.node("zone/temp_1").is_some());
        assert!(
            document.node("temp_1").is_none(),
            "a subdiagram's node answered to a bare name from the top diagram"
        );
    }

    /// Two subdiagrams may each hold a node of the same name.
    ///
    /// The reason names are qualified rather than globally unique: a
    /// subdiagram is a separate file so it can be reused, and a uniqueness
    /// rule would make including two copies impossible.
    #[test]
    fn the_same_name_in_two_subdiagrams_is_two_nodes() {
        let mut document = Document::new();
        for home in ["zone", "bay"] {
            document.apply(&Edit::AddNode {
                name: home.into(),
                node_type: "subdiagram".into(),
                home: None,
            });
            document.apply(&Edit::AddNode {
                name: "temp_1".into(),
                node_type: "i2c".into(),
                home: Some(home.into()),
            });
        }

        assert_eq!(document.nodes.len(), 4);
        assert!(
            !document
                .faults()
                .iter()
                .any(|f| matches!(f, Fault::DuplicateName { .. })),
            "two subdiagrams with a like-named node collided: {:?}",
            document.faults()
        );
        assert_ne!(
            document.node("zone/temp_1").unwrap().home,
            document.node("bay/temp_1").unwrap().home
        );
    }

    /// A link may cross a boundary, and is counted where it crosses.
    #[test]
    fn a_crossing_link_is_counted_against_the_subdiagram() {
        let mut document = Document::new();
        document.apply(&Edit::AddNode {
            name: "zone".into(),
            node_type: "subdiagram".into(),
            home: None,
        });
        document.apply(&Edit::AddNode {
            name: "gateway".into(),
            node_type: "gateway_radio".into(),
            home: None,
        });
        for name in ["temp_1", "k_1"] {
            document.apply(&Edit::AddNode {
                name: name.into(),
                node_type: "i2c".into(),
                home: Some("zone".into()),
            });
        }

        // One link inside, one crossing out.
        document.apply(&Edit::AddLink {
            link: Link::new("zone/temp_1", "zone/k_1", None),
        });
        document.apply(&Edit::AddLink {
            link: Link::new("zone/k_1", "gateway", None),
        });

        assert_eq!(
            document.crossings("zone"),
            1,
            "only the link leaving the subdiagram crosses it"
        );
        assert!(
            !document.faults().iter().any(|f| matches!(
                f,
                Fault::DanglingLink { .. } | Fault::SelfLink { .. }
            )),
            "a crossing link was taken for a broken one: {:?}",
            document.faults()
        );
    }

    /// A line from a node in a subdiagram is recorded where it arrives.
    ///
    /// The link holds its source's whole name, `zone/temp_1`, and the node
    /// its own, `temp_1`: compared as they stand they never matched, and the
    /// node the line reached said nothing about it.
    #[test]
    fn a_line_from_a_subdiagram_is_recorded_where_it_arrives() {
        use std::path::Path;

        let mut document = Document::new();
        document.apply(&Edit::AddNode {
            name: "zone".into(),
            node_type: "subdiagram".into(),
            home: None,
        });
        document.apply(&Edit::AddNode {
            name: "gateway".into(),
            node_type: "gateway_radio".into(),
            home: None,
        });
        document.apply(&Edit::AddNode {
            name: "temp_1".into(),
            node_type: "i2c".into(),
            home: Some("zone".into()),
        });
        document.apply(&Edit::AddLink {
            link: Link::new("zone/temp_1", "gateway", None),
        });

        let files = document
            .files(Path::new("/tmp/vehicle.ytcs"))
            .expect("files");
        let (_, top) = &files[0];
        assert!(
            top.contains("world-in: [temp_1]"),
            "the ground does not list the line's name: {top}"
        );
    }

    /// A line is recorded at the sink its own source's stream arrives at,
    /// whatever else shares the source's name.
    ///
    /// A node in a subdiagram may have the name of one in the top diagram.
    /// Found by the bare name, the line from the top diagram's `p` was taken
    /// for one from `zone/p`, of another type with another source, and the
    /// ground was written as fed at the wrong sink -- a file that reading it
    /// back refuses, the record disagreeing with the link.
    #[test]
    fn a_line_is_recorded_at_its_own_sources_sink() {
        use std::path::Path;

        let mut document = Document::new();
        document.apply(&Edit::AddNode {
            name: "zone".into(),
            node_type: "subdiagram".into(),
            home: None,
        });
        // First, so that a search by the bare name finds it before the
        // node the line leaves.
        document.apply(&Edit::AddNode {
            name: "p".into(),
            node_type: "app_payload_veh".into(),
            home: Some("zone".into()),
        });
        document.apply(&Edit::AddNode {
            name: "p".into(),
            node_type: "i2c".into(),
            home: None,
        });
        document.apply(&Edit::AddNode {
            name: "gateway".into(),
            node_type: "gateway_radio".into(),
            home: None,
        });
        document.apply(&Edit::AddLink {
            link: Link::new("p", "gateway", None),
        });

        let files = document
            .files(Path::new("/tmp/vehicle.ytcs"))
            .expect("files");
        let (_, top) = &files[0];
        assert!(top.contains("world-in: [p]"), "{top}");
        assert!(
            !top.contains("moc-in"),
            "the line was recorded at the sink of zone/p's stream: {top}"
        );
    }

    /// A crossing link is written once, in the file its source lives in.
    ///
    /// Written in both files it touches, it would be read back as two links;
    /// written in neither, it would be lost.
    #[test]
    fn a_crossing_link_is_written_exactly_once() {
        use std::path::Path;

        let mut document = Document::new();
        document.apply(&Edit::AddNode {
            name: "zone".into(),
            node_type: "subdiagram".into(),
            home: None,
        });
        document.apply(&Edit::AddNode {
            name: "gateway".into(),
            node_type: "gateway_radio".into(),
            home: None,
        });
        document.apply(&Edit::AddNode {
            name: "temp_1".into(),
            node_type: "i2c".into(),
            home: Some("zone".into()),
        });
        document.apply(&Edit::AddLink {
            link: Link::new("zone/temp_1", "gateway", None),
        });

        let files = document
            .files(Path::new("/tmp/vehicle.ytcs"))
            .expect("files");
        assert_eq!(files.len(), 2, "a top diagram and one subdiagram");

        let written: usize = files
            .iter()
            .map(|(_, text)| text.matches("- gateway.temp_1").count())
            .sum();
        assert_eq!(written, 1, "the crossing link was written {written} times");

        // In the subdiagram's file, where its source lives: a link is an
        // attribute of the node it leaves, so the file holding that node is
        // the only file that says anything about it. The far end is plain
        // because `gateway` is in the top diagram and has no subdiagram to
        // qualify it with.
        let (_, inner) = files
            .iter()
            .find(|(path, _)| path.ends_with("zone.ytcs"))
            .expect("the subdiagram's file");
        assert!(
            inner.contains("world-out:\n          - gateway.temp_1\n"),
            "{inner}"
        );

        // And the top file says what it includes.
        let (_, top) = &files[0];
        assert!(top.contains("include:\n  - zone"), "{top}");
    }

    /// A link into a subdiagram that is not there dangles, and says so.
    #[test]
    fn a_link_into_a_subdiagram_that_is_absent_is_reported() {
        let mut document = Document::new();
        document.apply(&Edit::AddNode {
            name: "gateway".into(),
            node_type: "gateway_radio".into(),
            home: None,
        });
        document.apply(&Edit::AddLink {
            link: Link::new("gateway", "zone/temp_1", None),
        });

        assert!(
            document
                .faults()
                .iter()
                .any(|f| matches!(f, Fault::DanglingLink { .. })),
            "{:?}",
            document.faults()
        );
    }

    /// Every fault can be shown to a person, and most point at a node.
    #[test]
    fn every_fault_describes_itself() {
        let faults = [
            Fault::DuplicateName { name: "a".into() },
            Fault::NamelessNode { at: (0, 0) },
            Fault::UnknownType {
                node: "a".into(),
                node_type: "x".into(),
            },
            Fault::DanglingLink {
                from: "a".into(),
                to: "b".into(),
            },
            Fault::SelfLink { node: "a".into() },
            Fault::InputBoundTwice {
                node: "m".into(),
                input: "in0".into(),
            },
        ];
        for fault in faults {
            assert!(!fault.describe().is_empty(), "{fault:?} says nothing");
        }
    }

    /// What is written is the shape the design document specifies.
    #[test]
    fn the_file_written_is_the_v2_shape() {
        let mut document = Document::new();
        document.apply(&add("temp_1", "i2c"));
        document.apply(&Edit::SetAttribute {
            node: "temp_1".into(),
            attribute: "address".into(),
            was: None,
            now: Some("0x48".into()),
        });
        document.apply(&add("zone_temp", "compute"));
        document.apply(&Edit::AddLink {
            link: Link::new("temp_1", "zone_temp", Some("in0")),
        });

        let yaml = document.to_yaml();
        assert!(!yaml.contains("version:"), "{yaml}");
        // Grouped by type, and each node named by the word for its section.
        assert!(
            yaml.contains(
                "\npayloads:\n  - type i2c:\n      - payload: temp_1\n"
            ),
            "{yaml}"
        );
        assert!(
            yaml.contains(
                "\nvehicle:\n  - type compute:\n      - compute: zone_temp\n"
            ),
            "{yaml}"
        );
        // Quoted: a YAML reader would otherwise turn 0x48 into 72. See
        // `quoted`.
        assert!(yaml.contains("address: \"0x48\""), "{yaml}");
        // The three sections, and the link as an attribute of the node it
        // leaves: a payload's readings leave by its world-out, and the input
        // they arrive at is named after the dot.
        assert!(yaml.contains("\npayloads:\n"), "{yaml}");
        assert!(yaml.contains("\nvehicle:\n"), "{yaml}");
        assert!(
            yaml.contains("        world-out:\n          - zone_temp.in0\n"),
            "a link into a named input is written as its node and the \
             input: {yaml}"
        );
        assert!(
            !yaml.contains("\nlinks:"),
            "there is no links section: {yaml}"
        );
    }
}
