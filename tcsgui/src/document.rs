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
use crate::pane::{confine, is_payload, size_of};
use tcslibgs::endpoint_config::Scalar;
use crate::style::NodeType;
use crate::subdiagram::{file_for, Home, Qualified};

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
    /// lets two subdiagrams each have a `temp_fore`: a node is known by its
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
    /// The MOC node the command arrives on.
    pub from: String,
    /// What it sets, as `node.attribute`.
    pub sets: String,
    /// What somebody wrote about it, kept as the comments on nodes and links
    /// are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// The whole of what is being edited.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Document {
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
    /// missing here: nodes could say `group: cabin_temps` and nothing declared
    /// what that group was, so the editor wrote a configuration the flight
    /// reader refuses. Found by the version 2 checker on the shipped example,
    /// which is what a checker is for.
    #[serde(default)]
    pub groups: BTreeMap<String, String>,
    /// What the ground may set in flight.
    ///
    /// Not editable on the canvas yet, and written out whatever else happens:
    /// see [`Command`].
    #[serde(default)]
    pub commands: Vec<Command>,
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
            .map(|(attribute, value)| (attribute.clone(), Scalar::of(value.as_str())))
            .collect(),
        editor: None,
    };

    // Only the nodes that are opened. A computation and a log have rules of
    // their own, and this would tell them they are not interfaces.
    if !theirs.is_payload() && theirs.node_type != "moc" {
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
    /// Strict in the same way and for the same reason: a beacon address that
    /// is not a multicast group is wrong as soon as it is typed, and an
    /// unanswered parameter is not wrong at all yet.
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
            Fault::SelfLink { node } => format!("\"{node}\" is linked to itself"),
            Fault::InputBoundTwice { node, input } => {
                format!("two links arrive at \"{node}.{input}\"")
            }
            // The reason names the node already: reach_of's complaints are
            // written to be read on their own.
            Fault::BadNode { why, .. } => why.clone(),
            Fault::BadTcspecial { why } => format!("tcspecial: {why}"),
            Fault::UntriggeredInput { from, .. } => {
                format!("nothing is written to this: \"{from}\" has nowhere to go")
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
    /// Absolute: `temp_fore` for a node of the top diagram and
    /// `cabin/temp_fore` for one inside a subdiagram. That is how the editor
    /// holds a name everywhere except in a file, where it is written relative
    /// to the file it is in -- see [`Qualified::written_from`].
    pub fn node(&self, name: &str) -> Option<&Node> {
        let wanted = Qualified::parse(name);
        self.nodes
            .iter()
            .find(|n| n.name == wanted.node && n.home == wanted.in_)
    }

    fn node_mut(&mut self, name: &str) -> Option<&mut Node> {
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
        found
    }

    /// The nodes that live in `home`.
    pub fn nodes_in(&self, home: &Home) -> Vec<&Node> {
        self.nodes.iter().filter(|n| &n.home == home).collect()
    }

    /// The parameters of the section that are actually stated.
    ///
    /// The document holds every attribute the section may have, so that the
    /// window has a field for each; what the file holds is the ones somebody
    /// has answered. Both writers ask this, so neither can come to write a
    /// section the other would not.
    fn tcspecial_stated(&self) -> Vec<(&String, &String)> {
        self.tcspecial
            .iter()
            .filter(|(_, value)| !value.trim().is_empty())
            .collect()
    }

    /// Put every node inside the pane its type belongs to.
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
    /// it writes nothing by itself -- the file is untouched until something is
    /// actually edited, at which point these positions are written with
    /// everything else, because every edit rewrites the whole file.
    ///
    /// A node that has to move keeps its height and takes the near edge of its
    /// own pane, which is the smallest move that obeys the rule. Where that
    /// lands it on top of another node it steps down the grid until it is
    /// clear: a payload column read out of a file where the payloads were in a
    /// row would otherwise become one node with eleven behind it.
    ///
    /// Returns how many were moved, which is what a caller would log.
    pub fn settle(&mut self, divider: i32) -> usize {
        let mut moved = 0;

        for at in 0..self.nodes.len() {
            let node = &self.nodes[at];
            let want = confine(&node.node_type, divider, node.at);
            if want == node.at {
                continue;
            }

            // Clear of the nodes that are already settled, and of the ones
            // that were in the right place to begin with. Checked against the
            // whole document rather than against the pane, because a node of
            // the other pane overlapping this one is just as unreadable.
            let (width, height) = size_of(&self.nodes[at].node_type);
            let mut want = want;
            for _ in 0..200 {
                let clashes = self.nodes.iter().enumerate().any(|(other, node)| {
                    if other == at {
                        return false;
                    }
                    let (theirs, tall) = size_of(&node.node_type);
                    want.0 < node.at.0 + theirs as i32
                        && node.at.0 < want.0 + width as i32
                        && want.1 < node.at.1 + tall as i32
                        && node.at.1 < want.1 + height as i32
                });
                if !clashes {
                    break;
                }
                want.1 += crate::pane::GRID * 4;
            }

            self.nodes[at].at = want;
            moved += 1;
        }

        moved
    }

    /// The payloads that are in `group`.
    ///
    /// Payloads and not every node: a group is the schedule a set of payloads
    /// is read on, and it is what the vehicle carries that gets read. A
    /// computation or a log naming a group is an oddity the configuration
    /// checker has words for; it is not something to set a bus address on.
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
        self.nodes
            .iter_mut()
            .filter(move |node| in_group(node, group) && is_payload(&node.node_type))
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
                let mut values = payloads
                    .iter()
                    .map(|node| node.attributes.get(attribute).map(|v| v.trim().to_string()));
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
    pub fn crossings(&self, subdiagram: &str) -> usize {
        let inside = |name: &str| Qualified::parse(name).in_.as_deref() == Some(subdiagram);
        self.links
            .iter()
            .filter(|l| inside(&l.from) != inside(&l.to))
            .count()
    }

    /// Apply an edit.
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
                            node.attributes.insert(attribute.clone(), value.clone());
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
            Edit::MoveNode { node, now, .. } => {
                if let Some(node) = self.node_mut(node) {
                    node.at = *now;
                }
            }
            Edit::SetTcspecial {
                attribute, now, ..
            } => match now {
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
                    let said = simulator.payloads.entry(payload.clone()).or_default();
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
                    simulator
                        .groups
                        .insert(name.clone(), settings.iter().cloned().collect());
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
                // And every node that named it. A node's group is how it gets
                // its interval, so a rename that left them behind would be a
                // rename that silently unscheduled a dozen sensors.
                for node in &mut self.nodes {
                    if node.attributes.get("group").map(|g| g.trim()) == Some(was.as_str()) {
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
                            node.attributes.insert(attribute.clone(), value.clone());
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
            Edit::RestoreGroupAttribute {
                attribute, now, ..
            } => {
                // Each node's own value, by name: the group may have changed
                // since, and what is being put back is what these nodes had.
                for (name, value) in now {
                    let Some(node) = self.node_mut(name) else {
                        continue;
                    };
                    match value {
                        Some(value) => {
                            node.attributes.insert(attribute.clone(), value.clone());
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
    }

    /// Everything wrong with the document as it stands.
    ///
    /// Empty means it could be flown, as far as the editor can tell: the
    /// checks a graph needs that depend on a node type's own rules -- whether a
    /// log's sources suit its format, whether every input of a mean is bound --
    /// belong to the flight-side validation and are reported by `tcsverify`,
    /// which the editor runs rather than reimplements.
    pub fn faults(&self) -> Vec<Fault> {
        let mut faults = Vec::new();

        // By the name a link would have to use, not by the bare one: two
        // subdiagrams may each hold a temp_fore, and they are two nodes
        // because cabin/temp_fore and bay/temp_fore are two names. Counting
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
                                node: Qualified::of(&node.name, &node.home).absolute(),
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
            let used = self
                .nodes
                .iter()
                .any(|n| n.attributes.get("group").map(|g| g.trim()) == Some(group.as_str()));
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
        let filled = tcslibgs::graph::parameters::ATTRIBUTES
            .iter()
            .filter(|(_, required)| *required)
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
            if self.node(&link.from).is_none() || self.node(&link.to).is_none() {
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
    pub fn files(
        &self,
        path: &std::path::Path,
    ) -> Result<Vec<(std::path::PathBuf, String)>, String> {
        let mut written = Vec::new();

        // The top diagram, and the includes that say what else to read.
        written.push((path.to_path_buf(), self.text_for(&None, path)?));

        for name in self.subdiagrams() {
            let home = Some(name.clone());
            written.push((file_for(&name, path), self.text_for(&home, path)?));
        }

        // And the simulator's own file, where the session was given one. Part
        // of the same set on purpose: the two files are edited together,
        // checked together and must agree, so one Save writes both and one
        // Undo takes back whichever of them the last edit touched.
        if let Some(simulator) = &self.simulator {
            let text = match format_of(&simulator.path)? {
                Format::Yaml => crate::sim::to_yaml(simulator),
                Format::Xml => crate::sim::to_xml(simulator),
            };
            written.push((simulator.path.clone(), text));
        }
        Ok(written)
    }

    /// One file's worth: the nodes at `home` and the links that start there.
    fn text_for(&self, home: &Home, path: &std::path::Path) -> Result<String, String> {
        match format_of(path)? {
            Format::Yaml => Ok(self.to_yaml_for(home)),
            Format::Xml => Ok(self.to_xml_for(home)),
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
        self.to_xml_for(&None)
    }

    /// One home's nodes and links, as XML.
    fn to_xml_for(&self, home: &Home) -> String {
        let mut out = String::new();
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        out.push_str("<configuration>\n    <version>2.0</version>\n");
        if let Some(description) = &self.description {
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
                for (attribute, value) in said {
                    out.push_str(&format!(
                        "        <{attribute}>{}</{attribute}>\n",
                        escape(value)
                    ));
                }
                out.push_str("    </tcspecial>\n");
            }
        }

        if home.is_none() {
            for name in self.subdiagrams() {
                out.push_str(&format!("    <include>{}</include>\n", escape(&name)));
            }
        }

        if home.is_none() {
            for (name, interval) in &self.groups {
                out.push_str("    <groups>\n");
                out.push_str(&format!("        <name>{}</name>\n", escape(name)));
                out.push_str(&format!(
                    "        <interval>{}</interval>\n",
                    escape(interval)
                ));
                out.push_str("    </groups>\n");
            }
        }

        for node in self.nodes_in(home) {
            if let Some(comment) = &node.comment {
                for line in comment.lines() {
                    out.push_str(&format!("    <!-- {} -->\n", escape(line)));
                }
            }
            out.push_str("    <nodes>\n");
            out.push_str(&format!("        <name>{}</name>\n", escape(&node.name)));
            out.push_str(&format!(
                "        <type>{}</type>\n",
                escape(&node.node_type)
            ));
            for (attribute, value) in &node.attributes {
                out.push_str(&format!(
                    "        <{attribute}>{}</{attribute}>\n",
                    escape(value)
                ));
            }
            out.push_str(&format!(
                "        <editor><x>{}</x><y>{}</y></editor>\n",
                node.at.0, node.at.1
            ));
            out.push_str("    </nodes>\n");
        }

        for link in self.links_from(home) {
            if let Some(comment) = &link.comment {
                for line in comment.lines() {
                    out.push_str(&format!("    <!-- {} -->\n", escape(line)));
                }
            }
            let from = Qualified::parse(&link.from).written_from(home);
            let to = Qualified::parse(&link.to).written_from(home);
            out.push_str("    <links>\n");
            out.push_str(&format!("        <from>{}</from>\n", escape(&from)));
            match &link.input {
                Some(input) => out.push_str(&format!(
                    "        <to>{}.{}</to>\n",
                    escape(&to),
                    escape(input)
                )),
                None => out.push_str(&format!("        <to>{}</to>\n", escape(&to))),
            }
            out.push_str("    </links>\n");
        }

        // The command bindings, which only the top file declares: what the
        // ground may change is a property of the whole configuration rather
        // than of one page of it.
        if home.is_none() {
            for command in &self.commands {
                if let Some(comment) = &command.comment {
                    for line in comment.lines() {
                        out.push_str(&format!("    <!-- {} -->\n", escape(line)));
                    }
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

        out.push_str("</configuration>\n");
        out
    }

    /// The document as a configuration file.
    ///
    /// Written in the shape `design.rst` specifies: a `nodes` list where each
    /// entry carries its `type` and its attributes, and a `links` list. The
    /// canvas positions are written as well, under a key the flight software
    /// ignores, so that reopening the file finds the diagram as it was left.
    #[cfg(test)]
    pub fn to_yaml(&self) -> String {
        self.to_yaml_for(&None)
    }

    /// One home's nodes and links, as YAML.
    fn to_yaml_for(&self, home: &Home) -> String {
        let mut out = String::new();
        out.push_str("version: \"2.0\"\n");
        if let Some(description) = &self.description {
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
                for (attribute, value) in said {
                    out.push_str(&format!("  {attribute}: {}\n", quoted(value)));
                }
            }
        }

        // The subdiagrams this file pulls in, named rather than discovered.
        // Only the top diagram has them: a subdiagram that included another
        // would be a nesting this editor does not make.
        if home.is_none() {
            let included = self.subdiagrams();
            if !included.is_empty() {
                out.push_str("\ninclude:\n");
                for name in &included {
                    out.push_str(&format!("  - {name}\n"));
                }
            }
        }

        // The groups, which only the top file declares: a group is a property
        // of the whole configuration rather than of one page of it.
        if home.is_none() && !self.groups.is_empty() {
            out.push_str("\ngroups:\n");
            for (name, interval) in &self.groups {
                out.push_str(&format!(
                    "  - name: {}\n    interval: {}\n",
                    quoted(name),
                    quoted(interval)
                ));
            }
        }

        out.push_str("\nnodes:\n");
        for node in self.nodes_in(home) {
            // Above the entry it belongs to, as a comment and not as a field:
            // this is what a person hand-editing the file expects to find, and
            // it is what they would have written themselves.
            if let Some(comment) = &node.comment {
                for line in comment.lines() {
                    out.push_str(&format!("  # {line}\n"));
                }
            }
            out.push_str(&format!(
                "  - name: {}\n    type: {}\n",
                node.name, node.node_type
            ));
            for (attribute, value) in &node.attributes {
                // Quoted, which is not fussiness. A YAML reader turns 0x48
                // into the integer 72 and 2.0 into 2 before anything sees the
                // text, where an XML reader hands over what was written -- so
                // the same configuration in the two formats would read
                // differently, and the digest of one would not match the
                // other. Quoting keeps the spelling the person wrote.
                out.push_str(&format!("    {attribute}: {}\n", quoted(value)));
            }
            // Two named numbers rather than a list, and the same shape in
            // XML: a reader of either format wants one struct, and `at: [x,
            // y]` in YAML against `<at>x,y</at>` in XML would be two.
            out.push_str(&format!(
                "    editor: {{ x: {}, y: {} }}\n",
                node.at.0, node.at.1
            ));
        }

        out.push_str("\nlinks:\n");
        for link in self.links_from(home) {
            if let Some(comment) = &link.comment {
                for line in comment.lines() {
                    out.push_str(&format!("  # {line}\n"));
                }
            }
            let from = Qualified::parse(&link.from).written_from(home);
            let to = Qualified::parse(&link.to).written_from(home);
            match &link.input {
                Some(input) => out.push_str(&format!(
                    "  - {{ from: {from}, to: {to}.{input} }}\n"
                )),
                None => out.push_str(&format!("  - {{ from: {from}, to: {to} }}\n")),
            }
        }

        // The command bindings, top file only: see the XML writer.
        if home.is_none() && !self.commands.is_empty() {
            out.push_str("\ncommands:\n");
            for command in &self.commands {
                if let Some(comment) = &command.comment {
                    for line in comment.lines() {
                        out.push_str(&format!("  # {line}\n"));
                    }
                }
                out.push_str(&format!(
                    "  - {{ from: {}, sets: {} }}\n",
                    command.from, command.sets
                ));
            }
        }

        out
    }

    /// The links written in `home`'s file: the ones whose source lives there.
    ///
    /// By the source and not by both ends, so that every link is written
    /// exactly once however far it reaches. A link written in both files it
    /// touches would be read as two.
    fn links_from(&self, home: &Home) -> Vec<&Link> {
        self.links
            .iter()
            .filter(|l| &Qualified::parse(&l.from).in_ == home)
            .collect()
    }
}

/// Which language a file's name asks for.
enum Format {
    Yaml,
    Xml,
}

/// The format `path` asks for, or why it asks for nothing.
///
/// The extension decides, which is the rule every configuration file in this
/// project follows: anything that is neither .yaml, .yml nor .xml is refused
/// rather than guessed at, because a guess is how a misspelled extension
/// becomes a file parsed as the wrong language.
fn format_of(path: &std::path::Path) -> Result<Format, String> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("yaml") | Some("yml") => Ok(Format::Yaml),
        Some("xml") => Ok(Format::Xml),
        Some(other) => Err(format!(
            "{}: \".{other}\" is not a configuration file -- a name ending in \
             .yaml, .yml or .xml says how to read it",
            path.display()
        )),
        None => Err(format!(
            "{}: a configuration file's name says how to read it, and this one \
             has no .yaml, .yml or .xml on it",
            path.display()
        )),
    }
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

/// XML's five, so that a comment or a value containing one of them is still
/// the document it was.
///
/// A name with an ampersand in it is unlikely and a comment with one is not:
/// "sampled at 1Hz & logged" is the sort of thing somebody writes, and an
/// unescaped ampersand makes the file unreadable rather than merely wrong.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// The worked example from `design.rst`: a cabin thermostat.
///
/// Built here rather than written out by hand in two formats, so that the
/// shipped `tests/manual/thermostat.yaml` and `.xml` are what this editor
/// writes and not an approximation of it. `the_shipped_example_is_what_the_editor_writes`
/// is what keeps the files and this agreeing.
#[cfg(test)]
pub fn thermostat_example() -> Document {
    let mut document = Document::new();

    let place = |document: &mut Document, name: &str, node_type: &str, at: (i32, i32)| {
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
    let set = |document: &mut Document, node: &str, attribute: &str, value: &str| {
        document.apply(&Edit::SetAttribute {
            node: node.to_string(),
            attribute: attribute.to_string(),
            was: None,
            now: Some(value.to_string()),
        });
    };
    let note = |document: &mut Document, node: &str, comment: &str| {
        document.apply(&Edit::SetNodeComment {
            node: node.to_string(),
            was: None,
            now: Some(comment.to_string()),
        });
    };

    // What tcspecial itself is configured with, which every configuration
    // must say: where commands are taken, where beacons go, where the
    // telemetry log is written.
    for (attribute, value) in [
        ("tcspecial_command", "0.0.0.0:4000"),
        ("moc_command", "127.0.0.1:4100"),
        ("payload_port", "4001"),
        ("tcspecial_tc_base", "0.0.0.0:6000"),
        ("moc_tc_base", "127.0.0.1:6100"),
        ("beacon_interval_ms", "5000"),
        ("beacon_address", "239.255.0.1:5550"),
        ("beacon_interface", "127.0.0.1"),
        ("log_dir", "/var/log/tcspecial"),
        ("log_segment_bytes", "65536"),
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
    document.groups.insert("cabin_temps".to_string(), "3s".to_string());

    place(&mut document, "temp_fore", "i2c", (40, 40));
    set(&mut document, "temp_fore", "path", "/dev/i2c-1");
    set(&mut document, "temp_fore", "address", "0x48");
    set(&mut document, "temp_fore", "group", "cabin_temps");
    set(&mut document, "temp_fore", "read_size", "2");
    note(
        &mut document,
        "temp_fore",
        "Forward bulkhead. Two bytes, signed, big endian.",
    );

    place(&mut document, "temp_aft", "i2c", (40, 160));
    set(&mut document, "temp_aft", "path", "/dev/i2c-1");
    set(&mut document, "temp_aft", "address", "0x49");
    set(&mut document, "temp_aft", "group", "cabin_temps");
    set(&mut document, "temp_aft", "read_size", "2");
    note(&mut document, "temp_aft", "Aft bulkhead; same part as temp_fore.");

    place(&mut document, "temp_bay", "spi", (40, 280));
    set(&mut document, "temp_bay", "path", "/dev/spidev0.0");
    set(&mut document, "temp_bay", "max_speed", "1000000");
    set(&mut document, "temp_bay", "spi_mode", "0");
    set(&mut document, "temp_bay", "group", "cabin_temps");
    set(&mut document, "temp_bay", "read_size", "3");
    note(
        &mut document,
        "temp_bay",
        "Equipment bay. Three bytes, unsigned -- a different part, hence its\nown conversion.",
    );

    for (name, at, format, scale, offset) in [
        ("k_fore", (240, 40), "i16_be", "0.0625", "273.15"),
        ("k_aft", (240, 160), "i16_be", "0.0625", "273.15"),
        ("k_bay", (240, 280), "u24_be", "0.0078", "255.37"),
    ] {
        place(&mut document, name, "compute", at);
        set(&mut document, name, "function", "to_kelvin");
        set(&mut document, name, "format", format);
        set(&mut document, name, "scale", scale);
        set(&mut document, name, "offset", offset);
    }

    place(&mut document, "mean_temp", "compute", (460, 160));
    set(&mut document, "mean_temp", "function", "mean");
    set(&mut document, "mean_temp", "inputs", "in0,in1,in2");

    place(&mut document, "thermostat", "compute", (660, 240));
    set(&mut document, "thermostat", "function", "hysteresis");
    set(&mut document, "thermostat", "setpoint", "293.15");
    set(&mut document, "thermostat", "above", "2.0");
    set(&mut document, "thermostat", "below", "2.0");
    note(
        &mut document,
        "thermostat",
        "setpoint is set from the ground; above and below are the two degrees\neither side that the heaters are switched at.",
    );

    place(&mut document, "moc", "moc", (660, 40));
    set(&mut document, "moc", "address", "10.0.0.1");
    set(&mut document, "moc", "port", "5000");

    place(&mut document, "heaters", "gpio", (860, 240));
    set(&mut document, "heaters", "path", "/dev/gpiochip0");
    set(&mut document, "heaters", "direction", "output");
    set(&mut document, "heaters", "lines", "3,4,5");
    note(&mut document, "heaters", "Three circuits, driven together.");

    place(&mut document, "housekeeping", "log", (460, 380));
    set(&mut document, "housekeeping", "directory", "/var/log/tcspecial");
    set(&mut document, "housekeeping", "format", "variable_ts_rc");

    let link = |document: &mut Document, from: &str, to: &str, input: Option<&str>| {
        document.apply(&Edit::AddLink {
            link: Link::new(from, to, input),
        });
    };

    link(&mut document, "temp_fore", "k_fore", None);
    link(&mut document, "temp_aft", "k_aft", None);
    link(&mut document, "temp_bay", "k_bay", None);

    // The "T": each converted reading goes to the ground, into the mean, and
    // into the log.
    for (from, input) in [("k_fore", "in0"), ("k_aft", "in1"), ("k_bay", "in2")] {
        link(&mut document, from, "moc", None);
        link(&mut document, from, "mean_temp", Some(input));
        link(&mut document, from, "housekeeping", None);
    }

    link(&mut document, "mean_temp", "moc", None);
    link(&mut document, "mean_temp", "thermostat", None);
    link(&mut document, "thermostat", "heaters", None);
    link(&mut document, "thermostat", "housekeeping", None);
    document.apply(&Edit::SetLinkComment {
        link: Link::new("thermostat", "heaters", None),
        was: None,
        now: Some("asserted below the setpoint, unasserted above it".to_string()),
    });

    // What the ground may change in flight, which the worked example calls for
    // and the diagram cannot show: the setpoint is commanded, and the rest of
    // the thermostat is not.
    document.commands.push(Command {
        from: "moc".to_string(),
        sets: "thermostat.setpoint".to_string(),
        comment: Some(
            "the cabin temperature the heaters are held at, in kelvin".to_string(),
        ),
    });

    document
}

#[cfg(test)]
mod tests {
    use super::*;

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
                Fault::UntriggeredInput { node, from } if node == "sensor" && from == "k"
            )),
            "{faults:?}"
        );
        // Drawn on the payload, which is where a trigger would be typed.
        let said = faults
            .iter()
            .find(|f| matches!(f, Fault::UntriggeredInput { .. }))
            .unwrap();
        assert_eq!(said.node(), Some("sensor"));

        // A trigger clears it.
        document.apply(&Edit::SetAttribute {
            node: "sensor".into(),
            attribute: "trigger".into(),
            was: None,
            now: Some("01".into()),
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
                editor: None,
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
        assert!(
            yaml.find("tcspecial:") < yaml.find("nodes:"),
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
        assert!(!document.to_yaml().contains("port"), "{}", document.to_yaml());
        assert!(document.faults().is_empty(), "clearing one is not a fault");
    }

    /// Once the section is complete, what it says has to hold together.
    ///
    /// The other half of being lazy: the editor requires nothing to be entered
    /// and requires everything entered to be valid. A beacon address that is
    /// not a multicast group is wrong the moment the last field is filled in,
    /// and the words are the ones the command interpreter itself would refuse
    /// it with.
    #[test]
    fn a_finished_tcspecial_section_is_checked_by_its_own_rules() {
        let whole = [
            ("tcspecial_command", "0.0.0.0:4000"),
            ("moc_command", "127.0.0.1:4100"),
            ("payload_port", "4001"),
            ("tcspecial_tc_base", "0.0.0.0:6000"),
            ("moc_tc_base", "127.0.0.1:6100"),
            ("beacon_interval_ms", "5000"),
            ("beacon_address", "239.255.0.1:5550"),
            ("beacon_interface", "127.0.0.1"),
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
        assert!(filled(None).faults().is_empty(), "{:?}", filled(None).faults());

        // Complete and wrong, in the words of the rules that know why.
        for (attribute, instead, expected) in [
            ("beacon_address", "10.0.0.1:5550", "multicast"),
            // Half a command interface, which is the one way an attribute
            // carrying two things goes wrong.
            ("tcspecial_command", "0.0.0.0", "address:port"),
            ("tcspecial_command", "0.0.0.0:4001", "payload_port"),
            ("payload_port", "nought", "payload_port"),
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
        document.apply(&add("temp_fore", "i2c"));

        // Nothing stated: nothing to say.
        assert!(document.faults().is_empty(), "{:?}", document.faults());

        // A bus and no address: still unfinished, still nothing to say.
        document.apply(&Edit::SetAttribute {
            node: "temp_fore".into(),
            attribute: "path".into(),
            was: None,
            now: Some("/dev/i2c-1".into()),
        });
        assert!(document.faults().is_empty(), "{:?}", document.faults());

        // And an address that is not one, which is wrong rather than
        // unfinished.
        document.apply(&Edit::SetAttribute {
            node: "temp_fore".into(),
            attribute: "address".into(),
            was: None,
            now: Some("0xZZ".into()),
        });
        let said = document
            .faults()
            .into_iter()
            .find_map(|fault| match fault {
                Fault::BadNode { node, why } if node == "temp_fore" => Some(why),
                _ => None,
            })
            .expect("an address that is not one should be refused");
        assert!(said.contains("address"), "{said}");

        // Put right, and the editor goes quiet again.
        document.apply(&Edit::SetAttribute {
            node: "temp_fore".into(),
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
        let divider = crate::pane::divider_for(crate::settings::DEFAULT);
        let mut document = Document::new();

        // A payload on the wrong side, a vehicle node on the wrong side, and
        // one of each where it belongs.
        for (name, node_type, at) in [
            ("stray_sensor", "i2c", (40, 120)),
            ("stray_compute", "compute", (divider + 40, 200)),
            ("good_sensor", "i2c", (divider + 16, 320)),
            ("good_compute", "compute", (240, 40)),
        ] {
            document.apply(&add(name, node_type));
            document.apply(&Edit::MoveNode {
                node: name.into(),
                was: (0, 0),
                now: at,
            });
        }

        let moved = document.settle(divider);
        assert_eq!(moved, 2, "only the two out of place should have moved");

        let at = |name: &str| document.node(name).unwrap().at;
        let width = crate::style::WIDTH as i32;

        // The payload is in its own pane now, at the near edge of it: the
        // smallest move that obeys the rule.
        assert_eq!(at("stray_sensor"), (divider, 120));
        // And the computation is out of it, by its own width.
        assert_eq!(at("stray_compute"), (divider - width, 200));
        // The two that were already right are untouched, which matters: a
        // reader who arranged a diagram should find it as they left it.
        assert_eq!(at("good_sensor"), (divider + 16, 320));
        assert_eq!(at("good_compute"), (240, 40));

        // Settling twice changes nothing, which is what makes it safe to do
        // on every open.
        assert_eq!(document.settle(divider), 0);
    }

    /// Nodes settled into one pane do not land on top of each other.
    ///
    /// A file with its payloads in a row across the canvas would otherwise
    /// become one node with the rest behind it -- every one of them clamped to
    /// the same edge, at whatever height it already had.
    #[test]
    fn settling_does_not_stack_nodes_on_one_another() {
        let divider = crate::pane::divider_for(crate::settings::DEFAULT);
        let mut document = Document::new();

        // Four sensors in a row at one height, all of them in the wrong pane.
        for (n, name) in ["one", "two", "three", "four"].iter().enumerate() {
            document.apply(&add(name, "i2c"));
            document.apply(&Edit::MoveNode {
                node: (*name).into(),
                was: (0, 0),
                now: (40 + n as i32 * 160, 80),
            });
        }

        assert_eq!(document.settle(divider), 4);

        // Every one in its pane, and no two overlapping.
        let boxes: Vec<((i32, i32), (u32, u32))> = document
            .nodes
            .iter()
            .map(|node| (node.at, crate::pane::size_of(&node.node_type)))
            .collect();
        for (at, _) in &boxes {
            assert!(at.0 >= divider, "{at:?} is not in the payload pane");
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
            name: "cabin_temps".into(),
            interval: "3s".into(),
        });
        for (name, node_type, group) in [
            ("temp_fore", "i2c", Some("cabin_temps")),
            ("temp_aft", "i2c", Some("cabin_temps")),
            // In another group, so it is not touched.
            ("temp_bay", "i2c", Some("bay")),
            // Not in a group at all.
            ("loose", "i2c", None),
            // In the group and not a payload, which is an oddity the checker
            // has words for and not something to set a bus address on.
            ("k", "compute", Some("cabin_temps")),
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
            .payloads_of("cabin_temps")
            .iter()
            .map(|n| (n.name.clone(), n.attributes.get("path").cloned()))
            .collect();
        assert_eq!(was.len(), 2, "the group's payloads: {was:?}");

        document.apply(&Edit::SetGroupAttribute {
            group: "cabin_temps".into(),
            attribute: "path".into(),
            was,
            now: Some("/dev/i2c-1".into()),
        });

        let path = |name: &str| {
            document
                .node(name)
                .unwrap()
                .attributes
                .get("path")
                .cloned()
        };
        assert_eq!(path("temp_fore").as_deref(), Some("/dev/i2c-1"));
        assert_eq!(path("temp_aft").as_deref(), Some("/dev/i2c-1"));
        assert_eq!(path("temp_bay"), None, "another group was written to");
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
            name: "cabin_temps".into(),
            interval: "3s".into(),
        });
        document.apply(&add("temp_fore", "i2c"));
        document.apply(&Edit::SetAttribute {
            node: "temp_fore".into(),
            attribute: "group".into(),
            was: None,
            now: Some("cabin_temps".into()),
        });

        document.apply(&Edit::RenameGroup {
            was: "cabin_temps".into(),
            now: "cabin".into(),
        });

        assert_eq!(document.groups.get("cabin").map(String::as_str), Some("3s"));
        assert!(!document.groups.contains_key("cabin_temps"));
        assert_eq!(
            document
                .node("temp_fore")
                .unwrap()
                .attributes
                .get("group")
                .map(String::as_str),
            Some("cabin")
        );
        // And the group is not left looking empty, which is a fault.
        assert!(
            !document
                .faults()
                .iter()
                .any(|f| matches!(f, Fault::EmptyGroup { .. } | Fault::NoSuchGroup { .. })),
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
            name: "cabin_temps".into(),
            interval: "3s".into(),
        });
        for (name, node_type) in [("temp_fore", "i2c"), ("temp_bay", "spi")] {
            document.apply(&add(name, node_type));
            document.apply(&Edit::SetAttribute {
                node: name.into(),
                attribute: "group".into(),
                was: None,
                now: Some("cabin_temps".into()),
            });
        }

        let offered: Vec<String> = document
            .group_fields("cabin_temps")
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
            name: "cabin_temps".into(),
            interval: "3s".into(),
        });
        for (name, address) in [("temp_fore", "0x48"), ("temp_aft", "0x49")] {
            document.apply(&add(name, "i2c"));
            for (attribute, value) in
                [("group", "cabin_temps"), ("address", address), ("path", "/dev/i2c-1")]
            {
                document.apply(&Edit::SetAttribute {
                    node: name.into(),
                    attribute: attribute.into(),
                    was: None,
                    now: Some(value.into()),
                });
            }
        }

        let fields = document.group_fields("cabin_temps");
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
            add("temp_fore", "i2c"),
            add("k_fore", "compute"),
            Edit::SetAttribute {
                node: "temp_fore".into(),
                attribute: "address".into(),
                was: None,
                now: Some("0x48".into()),
            },
            Edit::AddLink {
                link: Link::new("temp_fore", "k_fore", None),
            },
            Edit::MoveNode {
                node: "k_fore".into(),
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
        assert_eq!(live.node("k_fore").unwrap().at, (240, 80));
        assert_eq!(
            live.node("temp_fore").unwrap().attributes.get("address"),
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
            attributes: node.attributes.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
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
            now: "temp_fore".into(),
        });

        assert_eq!(document.links[0].from, "temp_fore");
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
            document.faults().iter().any(|f| matches!(f, Fault::DanglingLink { .. })),
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
        fn exhaustive(edit: &Edit) {
            match edit {
                Edit::AddNode { .. } => (),
                Edit::RemoveNode { .. } => (),
                Edit::RestoreNode { .. } => (),
                Edit::SetAttribute { .. } => (),
                Edit::RenameNode { .. } => (),
                Edit::AddLink { .. } => (),
                Edit::RemoveLink { .. } => (),
                Edit::SetNodeComment { .. } => (),
                Edit::SetLinkComment { .. } => (),
                Edit::MoveNode { .. } => (),
                Edit::SetTcspecial { .. } => (),
                Edit::SetSimPayload { .. } => (),
                Edit::SetSimGroup { .. } => (),
                Edit::AddSimGroup { .. } => (),
                Edit::RemoveSimGroup { .. } => (),
                Edit::RestoreSimGroup { .. } => (),
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
            document.apply(&Edit::SetSimGroup {
                group: "steady_1hz".into(),
                setting: "packet_interval_ms".into(),
                was: None,
                now: Some("1000".into()),
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
                [("port", "4000"), ("log_dir", "/var/log/tcspecial")]
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
                name: "cabin_temps".into(),
                interval: "3s".into(),
            });
            document.apply(&Edit::SetAttribute {
                node: "a".into(),
                attribute: "group".into(),
                was: None,
                now: Some("cabin_temps".into()),
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
                now: "temp_fore".into(),
            },
            Edit::AddLink {
                link: Link::new("b", "a", None),
            },
            Edit::RemoveLink {
                link: Link::new("a", "b", Some("in0")),
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
                setting: "packet_interval_ms".into(),
                was: Some("1000".into()),
                now: Some("500".into()),
            },
            Edit::AddSimGroup {
                name: "continuous".into(),
            },
            Edit::RemoveSimGroup {
                name: "steady_1hz".into(),
                settings: vec![("packet_interval_ms".into(), "1000".into())],
            },
            Edit::RestoreSimGroup {
                name: "bay".into(),
                settings: vec![("packet_interval_ms".into(), "250".into())],
            },
            // And one that clears a parameter, which takes the line out of
            // the file rather than writing it empty.
            Edit::SetTcspecial {
                attribute: "log_dir".into(),
                was: Some("/var/log/tcspecial".into()),
                now: None,
            },
            Edit::AddGroup {
                name: "bay".into(),
                interval: "5s".into(),
            },
            Edit::RemoveGroup {
                name: "cabin_temps".into(),
                interval: "3s".into(),
            },
            Edit::SetGroupInterval {
                group: "cabin_temps".into(),
                was: "3s".into(),
                now: "1s".into(),
            },
            Edit::RenameGroup {
                was: "cabin_temps".into(),
                now: "cabin".into(),
            },
            // One value for every payload of the group, carrying what each of
            // them had: `a` had no path, so undoing this takes the attribute
            // off again rather than leaving it empty.
            Edit::SetGroupAttribute {
                group: "cabin_temps".into(),
                attribute: "path".into(),
                was: vec![("a".into(), None)],
                now: Some("/dev/i2c-1".into()),
            },
            Edit::RestoreGroupAttribute {
                group: "cabin_temps".into(),
                attribute: "path".into(),
                was: None,
                now: vec![("a".into(), Some("/dev/i2c-1".into()))],
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
        document.apply(&add("temp_fore", "i2c"));
        document.apply(&add("k_fore", "compute"));
        document.apply(&Edit::AddLink {
            link: Link::new("temp_fore", "k_fore", None),
        });
        document.apply(&Edit::SetNodeComment {
            node: "temp_fore".into(),
            was: None,
            now: Some("0x49 after the rework; see ECR-114".into()),
        });
        document.apply(&Edit::SetLinkComment {
            link: Link::new("temp_fore", "k_fore", None),
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
        assert!(xml.contains("<!-- 0x49 after the rework; see ECR-114 -->"), "{xml}");
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

    /// The format follows the file's name.
    #[test]
    fn the_extension_decides_the_format() {
        use std::path::Path;

        let mut document = Document::new();
        document.apply(&add("a", "i2c"));

        let yaml = document.to_text(Path::new("graph.yaml")).expect("yaml");
        assert!(yaml.starts_with("version:"), "{yaml}");
        assert_eq!(
            document.to_text(Path::new("graph.yml")).expect("yml"),
            yaml,
            "the two YAML extensions wrote different files"
        );

        let xml = document.to_text(Path::new("graph.xml")).expect("xml");
        assert!(xml.starts_with("<?xml"), "{xml}");

        // And anything else is refused rather than guessed at.
        let said = document
            .to_text(Path::new("graph.json"))
            .expect_err("an unknown extension should be refused");
        assert!(said.contains(".yaml"), "{said}");
        let said = document
            .to_text(Path::new("graph"))
            .expect_err("no extension should be refused");
        assert!(said.contains("no .yaml"), "{said}");
    }

    /// The five characters XML cannot carry raw.
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
        assert!(xml.contains("&amp;"), "an ampersand went in raw: {xml}");
        assert!(xml.contains("&lt;always&gt;"), "{xml}");
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

        for (name, written) in [
            ("tests/manual/thermostat.yaml", document.to_yaml()),
            ("tests/manual/thermostat.xml", document.to_xml()),
        ] {
            let path = root.join(name);

            if std::env::var("TCSGUI_WRITE_EXAMPLE").is_ok() {
                std::fs::write(&path, &written).expect("the example is writable");
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
            assert!(yaml.contains(&format!("name: {}", node.name)), "{}", node.name);
            assert!(
                xml.contains(&format!("<name>{}</name>", node.name)),
                "{} is missing from the XML",
                node.name
            );
            for (attribute, value) in &node.attributes {
                assert!(
                    yaml.contains(&format!("{attribute}: \"{value}\"")),
                    "{}.{attribute} is missing from the YAML",
                    node.name
                );
                assert!(
                    xml.contains(&format!("<{attribute}>{value}</{attribute}>")),
                    "{}.{attribute} is missing from the XML",
                    node.name
                );
            }
            if let Some(comment) = &node.comment {
                let first = comment.lines().next().unwrap();
                assert!(yaml.contains(first), "a comment is missing from the YAML");
                assert!(xml.contains(first), "a comment is missing from the XML");
            }
        }

        for link in &document.links {
            let to = match &link.input {
                Some(input) => format!("{}.{}", link.to, input),
                None => link.to.clone(),
            };
            assert!(
                yaml.contains(&format!("from: {}, to: {to}", link.from)),
                "{} to {to} is missing from the YAML",
                link.from
            );
            assert!(
                xml.contains(&format!("<to>{to}</to>")),
                "{} to {to} is missing from the XML",
                link.from
            );
        }
    }

    /// A node placed in a subdiagram belongs to it, and is named through it.
    #[test]
    fn a_subdiagrams_nodes_are_its_own() {
        let mut document = Document::new();
        document.apply(&Edit::AddNode {
            name: "cabin".into(),
            node_type: "subdiagram".into(),
            home: None,
        });
        document.apply(&Edit::AddNode {
            name: "temp_fore".into(),
            node_type: "i2c".into(),
            home: Some("cabin".into()),
        });
        document.apply(&Edit::AddNode {
            name: "moc".into(),
            node_type: "moc".into(),
            home: None,
        });

        assert_eq!(document.subdiagrams(), vec!["cabin".to_string()]);
        assert_eq!(document.nodes_in(&None).len(), 2, "cabin and moc");
        assert_eq!(document.nodes_in(&Some("cabin".into())).len(), 1);

        // Named through its home from outside, plainly from inside.
        assert!(document.node("cabin/temp_fore").is_some());
        assert!(
            document.node("temp_fore").is_none(),
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
        for home in ["cabin", "bay"] {
            document.apply(&Edit::AddNode {
                name: home.into(),
                node_type: "subdiagram".into(),
                home: None,
            });
            document.apply(&Edit::AddNode {
                name: "temp_fore".into(),
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
            document.node("cabin/temp_fore").unwrap().home,
            document.node("bay/temp_fore").unwrap().home
        );
    }

    /// A link may cross a boundary, and is counted where it crosses.
    #[test]
    fn a_crossing_link_is_counted_against_the_subdiagram() {
        let mut document = Document::new();
        document.apply(&Edit::AddNode {
            name: "cabin".into(),
            node_type: "subdiagram".into(),
            home: None,
        });
        document.apply(&Edit::AddNode {
            name: "moc".into(),
            node_type: "moc".into(),
            home: None,
        });
        for name in ["temp_fore", "k_fore"] {
            document.apply(&Edit::AddNode {
                name: name.into(),
                node_type: "i2c".into(),
                home: Some("cabin".into()),
            });
        }

        // One link inside, one crossing out.
        document.apply(&Edit::AddLink {
            link: Link::new("cabin/temp_fore", "cabin/k_fore", None),
        });
        document.apply(&Edit::AddLink {
            link: Link::new("cabin/k_fore", "moc", None),
        });

        assert_eq!(
            document.crossings("cabin"),
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

    /// A crossing link is written once, in the file its source lives in.
    ///
    /// Written in both files it touches, it would be read back as two links;
    /// written in neither, it would be lost.
    #[test]
    fn a_crossing_link_is_written_exactly_once() {
        use std::path::Path;

        let mut document = Document::new();
        document.apply(&Edit::AddNode {
            name: "cabin".into(),
            node_type: "subdiagram".into(),
            home: None,
        });
        document.apply(&Edit::AddNode {
            name: "moc".into(),
            node_type: "moc".into(),
            home: None,
        });
        document.apply(&Edit::AddNode {
            name: "temp_fore".into(),
            node_type: "i2c".into(),
            home: Some("cabin".into()),
        });
        document.apply(&Edit::AddLink {
            link: Link::new("cabin/temp_fore", "moc", None),
        });

        let files = document.files(Path::new("/tmp/vehicle.yaml")).expect("files");
        assert_eq!(files.len(), 2, "a top diagram and one subdiagram");

        let written: usize = files
            .iter()
            .map(|(_, text)| text.matches("to: moc").count())
            .sum();
        assert_eq!(written, 1, "the crossing link was written {written} times");

        // In the subdiagram's file, where its source lives, and plainly at
        // this end because that is where it is at home.
        let (_, inner) = files
            .iter()
            .find(|(path, _)| path.ends_with("cabin.yaml"))
            .expect("the subdiagram's file");
        assert!(inner.contains("{ from: temp_fore, to: moc }"), "{inner}");

        // And the top file says what it includes.
        let (_, top) = &files[0];
        assert!(top.contains("include:\n  - cabin"), "{top}");
    }

    /// A link into a subdiagram that is not there dangles, and says so.
    #[test]
    fn a_link_into_a_subdiagram_that_is_absent_is_reported() {
        let mut document = Document::new();
        document.apply(&Edit::AddNode {
            name: "moc".into(),
            node_type: "moc".into(),
            home: None,
        });
        document.apply(&Edit::AddLink {
            link: Link::new("moc", "cabin/temp_fore", None),
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
        document.apply(&add("temp_fore", "i2c"));
        document.apply(&Edit::SetAttribute {
            node: "temp_fore".into(),
            attribute: "address".into(),
            was: None,
            now: Some("0x48".into()),
        });
        document.apply(&add("mean_temp", "compute"));
        document.apply(&Edit::AddLink {
            link: Link::new("temp_fore", "mean_temp", Some("in0")),
        });

        let yaml = document.to_yaml();
        assert!(yaml.contains("version: \"2.0\""), "{yaml}");
        assert!(yaml.contains("type: i2c"), "{yaml}");
        // Quoted: a YAML reader would otherwise turn 0x48 into 72. See
        // `quoted`.
        assert!(yaml.contains("address: \"0x48\""), "{yaml}");
        assert!(
            yaml.contains("{ from: temp_fore, to: mean_temp.in0 }"),
            "a link into a named input is written as one: {yaml}"
        );
    }
}
