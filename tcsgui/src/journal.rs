//! The edit history: deep undo, and surviving a crash, from one mechanism.
//!
//! The editor saves after every edit and offers undo back to the beginning of
//! the session. Those sound like two features and are better built as one: a
//! list of the edits made, in order, with a cursor saying how far along it the
//! document currently stands.
//!
//! * **Undo** moves the cursor back and applies each edit's inverse.
//! * **Redo** moves it forward and applies the edit again.
//! * **Depth** is the whole list. Configurations are small -- hundreds of
//!   nodes at the most -- so there is no reason to forget anything, and a
//!   bounded history is a promise to lose work that nobody asked for.
//! * **Crash recovery** is the same list written to a file beside the
//!   document. An editor that died after edit 40 comes back with 40 edits to
//!   replay and a cursor to put back where it was, which is a stronger promise
//!   than "the document was saved": the *history* was saved too, so undo still
//!   reaches back past the crash.
//!
//! An edit is stored with everything needed to undo it, not merely to redo it.
//! Setting an attribute records what it was as well as what it became; removing
//! a node records the node. That makes an inverse a matter of reading rather
//! than of recomputing, and recomputing is where undo implementations go wrong
//! -- an inverse derived from the current document is an inverse of whatever
//! the document happens to be now, not of what the edit did.

use serde::{Deserialize, Serialize};

use crate::subdiagram::Home;

/// One change to the document, carrying what is needed to undo it.
///
/// Deliberately coarse: a node moved on the canvas is one edit, not a hundred
/// as the pointer travels. The window decides when a gesture has finished and
/// records one edit for it, which is what makes undo land where a person
/// expects rather than one pixel back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Edit {
    /// A node was added. Undone by removing it.
    ///
    /// `home` is the subdiagram it was added to, or `None` for the top
    /// diagram: a node placed in a subdiagram's window belongs to that
    /// subdiagram's file, and undo has to put it back in the same one.
    AddNode {
        name: String,
        node_type: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        home: Home,
    },
    /// A node was removed, with everything it had, so that undo puts the same
    /// node back and not a fresh one of the same type.
    RemoveNode {
        name: String,
        node_type: String,
        attributes: Vec<(String, String)>,
        /// The links that went with it. Removing a node removes the links that
        /// mention it -- leaving them would be a document that fails its own
        /// validation -- so undo has to restore those too.
        links: Vec<Link>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        home: Home,
    },
    /// An attribute was set. Both values, so the inverse is a lookup.
    ///
    /// `was: None` means the attribute was not set before, and undoing the
    /// edit removes it rather than setting it to an empty string: an absent
    /// attribute and an empty one are different things to a configuration.
    SetAttribute {
        node: String,
        attribute: String,
        was: Option<String>,
        now: Option<String>,
    },
    /// A node was renamed. Links refer to nodes by name, so this is a document
    /// change rather than a cosmetic one, and the inverse is the other way
    /// round.
    RenameNode { was: String, now: String },
    /// A link was added.
    AddLink { link: Link },
    /// One end of a link was dragged to another terminal.
    ///
    /// One edit and not a removal followed by an addition, because it is one
    /// gesture: somebody took hold of the end of a line and put it somewhere
    /// else, and an undo that put the line back in two steps would be an undo
    /// that does half of what was done.
    MoveLink { was: Link, now: Link },
    /// A link was removed, carried whole for the same reason a node is.
    RemoveLink { link: Link },
    /// What somebody wrote about a node was changed.
    ///
    /// A comment is part of the document and so part of the history: written
    /// out with the node, undone like anything else, and never quietly
    /// dropped. Both values, so the inverse is a lookup.
    SetNodeComment {
        node: String,
        was: Option<String>,
        now: Option<String>,
    },
    /// What somebody wrote about a connection was changed.
    SetLinkComment {
        link: Link,
        was: Option<String>,
        now: Option<String>,
    },
    /// A node, its attributes and its links put back as they were.
    ///
    /// The inverse of a removal, and a variant of its own rather than a
    /// sequence of additions: undoing one removal must be one redo away, and a
    /// group of edits in the history would be several.
    RestoreNode {
        name: String,
        node_type: String,
        attributes: Vec<(String, String)>,
        links: Vec<Link>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        home: Home,
    },
    /// A group was declared, with the interval it reads on.
    ///
    /// A group is one of the four things a configuration is made of, and the
    /// one the editor could not make: a node could say `group: cabin_temps`
    /// and nothing declared what that was.
    AddGroup { name: String, interval: String },
    /// A group was taken away, with the interval it had.
    ///
    /// The members are left saying they are in it, which is a fault the editor
    /// marks: the alternative is to take the group off every node that named
    /// it, which is a removal that quietly edits a dozen other things.
    RemoveGroup { name: String, interval: String },
    /// How often a group is read was changed. Both values, so the inverse is
    /// a lookup.
    SetGroupInterval {
        group: String,
        was: String,
        now: String,
    },
    /// A group was renamed, and every node in it with it.
    ///
    /// Nodes name their group, so this is a change to them as well -- and one
    /// edit rather than a dozen, because a rename is one thing a person did
    /// and one thing they will expect undo to take back.
    RenameGroup { was: String, now: String },
    /// An attribute was set on every payload of a group at once.
    ///
    /// What a group is for, beyond the schedule: a dozen sensors on one bus
    /// are told the bus once. `was` carries what each node had, so undo puts
    /// back exactly what was there -- including the nodes that had nothing,
    /// which get the attribute removed again rather than emptied.
    SetGroupAttribute {
        group: String,
        attribute: String,
        was: Vec<(String, Option<String>)>,
        now: Option<String>,
    },
    /// Every payload of a group given back the value it had.
    ///
    /// The inverse of [`Edit::SetGroupAttribute`], and a variant of its own
    /// for the same reason [`Edit::RestoreNode`] is: undoing one gesture must
    /// be one redo away, and a dozen separate edits in the history would be a
    /// dozen presses.
    RestoreGroupAttribute {
        group: String,
        attribute: String,
        was: Option<String>,
        now: Vec<(String, Option<String>)>,
    },
    /// One of tcspecial's own parameters was set. Both values, so the
    /// inverse is a lookup.
    ///
    /// `was: None` means the attribute was not stated before, and undoing
    /// takes the line out again rather than writing it empty.
    SetTcspecial {
        attribute: String,
        was: Option<String>,
        now: Option<String>,
    },
    /// A setting of one simulated payload was set. Both values, so the
    /// inverse is a lookup.
    ///
    /// `payload` is the payload of the configuration this one stands in for:
    /// the two files are joined by name, and the editor never invents a name
    /// the configuration does not have.
    SetSimPayload {
        payload: String,
        setting: String,
        was: Option<String>,
        now: Option<String>,
    },
    /// A setting of one simulator group was set.
    SetSimGroup {
        group: String,
        setting: String,
        was: Option<String>,
        now: Option<String>,
    },
    /// A simulator group was declared, or taken away again.
    ///
    /// A group is a set of settings several simulated payloads share, so it
    /// exists before anything is in it -- which is why making one is an edit
    /// of its own rather than a consequence of typing.
    AddSimGroup { name: String },
    /// A simulator group was removed, with what it held, so that undo puts
    /// back the group that was there and not an empty one of the same name.
    RemoveSimGroup {
        name: String,
        settings: Vec<(String, String)>,
    },
    /// A simulator group put back as it was, with everything it held.
    RestoreSimGroup {
        name: String,
        settings: Vec<(String, String)>,
    },
    /// A node was moved on the canvas. Not a change to the configuration --
    /// the vehicle does not care where the box is -- but it is in the history
    /// because a person who drags a node expects undo to put it back.
    MoveNode {
        node: String,
        was: (i32, i32),
        now: (i32, i32),
    },
    /// A concern set aside, or taken up again.
    ///
    /// An edit because it is a decision about the configuration and it is
    /// written in the file: somebody who sets one aside by accident expects
    /// undo to bring it back, as they would for any other press.
    SetAside {
        node: String,
        kind: String,
        /// Whether it is being set aside, as against taken up again.
        aside: bool,
    },
    /// Every node brought to the top of its own pane, in one edit.
    ///
    /// What **Edit -> Arrange the nodes** does. One edit and not one per node,
    /// so that one undo puts the whole layout back: somebody who tidies a
    /// diagram they had arranged by hand must be able to get it back, and
    /// eleven undos would be eleven presses and a chance to stop halfway.
    ///
    /// Both lists hold only the nodes that moved, by name, so the inverse is
    /// the same edit with the pairs the other way about.
    Arrange {
        was: Vec<(String, (i32, i32))>,
        now: Vec<(String, (i32, i32))>,
    },
    /// Several edits that one gesture made, undone and redone as one.
    ///
    /// Placing a node is a node added, the attributes its palette entry
    /// answers, and a move to where it fits. Each is its own edit, so that
    /// each is applied the one way edits are applied; recorded singly they
    /// were three presses of Undo for one press of the palette, the first of
    /// them leaving the node in the corner it was added at. Applied in order,
    /// and undone in the reverse order.
    Group { edits: Vec<Edit> },
}

/// One connection, from a node to a named input of another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub from: String,
    /// Which data source of the origin the line leaves by, where the type has
    /// more than one. `None` is a node with one, which needs no naming -- see
    /// `tcslibgs::graph::terminals`.
    ///
    /// Written into the file as part of the `from`: `from: through.payload-out`.
    /// Kept apart from the name here because everything in the editor that
    /// looks a link's source up looks up a node, and a field that sometimes
    /// held `node.terminal` would be a name that sometimes is not one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    pub to: String,
    /// Which input of the destination, where the destination has more than
    /// one. `None` is a destination that takes a single input.
    ///
    /// Two things are written here, because the file writes both in the same
    /// place: a computation's named input -- `in0` -- and a data sink of a
    /// type that has more than one, which is a terminal. Which it is, is a
    /// question for the name: `tcslibgs::graph::terminals::stream_of` knows
    /// the four terminals and nothing else does.
    pub input: Option<String>,
    /// What somebody wrote about this connection.
    ///
    /// Not part of what makes one link the same connection as another -- see
    /// [`Link::same_connection`] -- because a comment edited must not turn a
    /// link into a different link that the old one is still beside.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

impl Link {
    /// Whether these name the same connection.
    ///
    /// The two ends and the input, and deliberately not the comment: a link is
    /// identified by what it joins. Equality, which does include the comment,
    /// is what the history compares -- an edit that changed a comment and an
    /// edit that did not are different edits.
    pub fn same_connection(&self, other: &Link) -> bool {
        self.from == other.from
            && self.origin == other.origin
            && self.to == other.to
            && self.input == other.input
    }

    /// A link joining these, with nothing written about it.
    ///
    /// Neither end naming a terminal. For the tests: a link the editor makes
    /// always names the data source it leaves by, that being the attribute it
    /// is written under -- see [`Link::joining`].
    #[cfg(test)]
    pub fn new(from: &str, to: &str, input: Option<&str>) -> Link {
        Link::joining(from, None, to, input)
    }

    /// A link leaving a named data source, with nothing written about it.
    ///
    /// What the canvas makes when the line was dragged from the terminal of a
    /// node that has more than one -- a `null` node, whose links have to say
    /// which stream they carry.
    pub fn joining(
        from: &str,
        origin: Option<&str>,
        to: &str,
        input: Option<&str>,
    ) -> Link {
        Link {
            from: from.to_string(),
            origin: origin.map(|t| t.to_string()),
            to: to.to_string(),
            input: input.map(|i| i.to_string()),
            comment: None,
        }
    }
}

impl Edit {
    /// The edit that undoes this one.
    ///
    /// A total function with no reference to the document, which is the point:
    /// an inverse that consulted the document would be an inverse of the
    /// document's present state rather than of this edit.
    pub fn inverse(&self) -> Edit {
        match self {
            Edit::AddNode {
                name,
                node_type,
                home,
            } => Edit::RemoveNode {
                name: name.clone(),
                node_type: node_type.clone(),
                attributes: Vec::new(),
                links: Vec::new(),
                home: home.clone(),
            },
            Edit::RemoveNode {
                name,
                node_type,
                attributes,
                links,
                home,
            } => {
                // Undoing a removal is more than an addition: the attributes
                // and the links come back as well. The caller applies this as
                // a group, which `Journal::undo` does.
                Edit::RestoreNode {
                    name: name.clone(),
                    node_type: node_type.clone(),
                    attributes: attributes.clone(),
                    links: links.clone(),
                    home: home.clone(),
                }
            }
            Edit::SetAttribute {
                node,
                attribute,
                was,
                now,
            } => Edit::SetAttribute {
                node: node.clone(),
                attribute: attribute.clone(),
                was: now.clone(),
                now: was.clone(),
            },
            Edit::RenameNode { was, now } => Edit::RenameNode {
                was: now.clone(),
                now: was.clone(),
            },
            Edit::AddLink { link } => Edit::RemoveLink { link: link.clone() },
            Edit::RemoveLink { link } => Edit::AddLink { link: link.clone() },
            Edit::MoveLink { was, now } => Edit::MoveLink {
                was: now.clone(),
                now: was.clone(),
            },
            Edit::MoveNode { node, was, now } => Edit::MoveNode {
                node: node.clone(),
                was: *now,
                now: *was,
            },
            Edit::SetAside { node, kind, aside } => Edit::SetAside {
                node: node.clone(),
                kind: kind.clone(),
                aside: !aside,
            },
            Edit::Arrange { was, now } => Edit::Arrange {
                was: now.clone(),
                now: was.clone(),
            },
            Edit::Group { edits } => Edit::Group {
                edits: edits.iter().rev().map(Edit::inverse).collect(),
            },
            Edit::SetNodeComment { node, was, now } => Edit::SetNodeComment {
                node: node.clone(),
                was: now.clone(),
                now: was.clone(),
            },
            Edit::SetTcspecial {
                attribute,
                was,
                now,
            } => Edit::SetTcspecial {
                attribute: attribute.clone(),
                was: now.clone(),
                now: was.clone(),
            },
            Edit::SetSimPayload {
                payload,
                setting,
                was,
                now,
            } => Edit::SetSimPayload {
                payload: payload.clone(),
                setting: setting.clone(),
                was: now.clone(),
                now: was.clone(),
            },
            Edit::SetSimGroup {
                group,
                setting,
                was,
                now,
            } => Edit::SetSimGroup {
                group: group.clone(),
                setting: setting.clone(),
                was: now.clone(),
                now: was.clone(),
            },
            Edit::AddSimGroup { name } => Edit::RemoveSimGroup {
                name: name.clone(),
                settings: Vec::new(),
            },
            Edit::RemoveSimGroup { name, settings } => {
                // A group comes back with what it held, which an addition
                // followed by a dozen settings would not: undoing one removal
                // must be one redo away.
                Edit::RestoreSimGroup {
                    name: name.clone(),
                    settings: settings.clone(),
                }
            }
            Edit::RestoreSimGroup { name, settings } => Edit::RemoveSimGroup {
                name: name.clone(),
                settings: settings.clone(),
            },
            Edit::AddGroup { name, interval } => Edit::RemoveGroup {
                name: name.clone(),
                interval: interval.clone(),
            },
            Edit::RemoveGroup { name, interval } => Edit::AddGroup {
                name: name.clone(),
                interval: interval.clone(),
            },
            Edit::SetGroupInterval { group, was, now } => Edit::SetGroupInterval {
                group: group.clone(),
                was: now.clone(),
                now: was.clone(),
            },
            Edit::RenameGroup { was, now } => Edit::RenameGroup {
                was: now.clone(),
                now: was.clone(),
            },
            // The inverse of one value for everybody is each node's own value
            // back again, which is why the edit carries them. `now` is not a
            // single value here and there is nothing to put in it: see
            // Document::apply, which uses `was` when `now` is absent and the
            // list is there.
            Edit::SetGroupAttribute {
                group,
                attribute,
                was,
                now,
            } => Edit::RestoreGroupAttribute {
                group: group.clone(),
                attribute: attribute.clone(),
                was: now.clone(),
                now: was.clone(),
            },
            Edit::RestoreGroupAttribute {
                group,
                attribute,
                was,
                now,
            } => Edit::SetGroupAttribute {
                group: group.clone(),
                attribute: attribute.clone(),
                was: now.clone(),
                now: was.clone(),
            },
            Edit::SetLinkComment { link, was, now } => Edit::SetLinkComment {
                link: link.clone(),
                was: now.clone(),
                now: was.clone(),
            },
            Edit::RestoreNode {
                name,
                node_type,
                attributes,
                links,
                home,
            } => Edit::RemoveNode {
                name: name.clone(),
                node_type: node_type.clone(),
                attributes: attributes.clone(),
                links: links.clone(),
                home: home.clone(),
            },
        }
    }

    /// A short phrase for the undo menu, so that what a press will undo is
    /// readable before it is pressed.
    pub fn describe(&self) -> String {
        match self {
            Edit::AddNode {
                name,
                node_type,
                home,
            } => match home {
                Some(home) => format!("add {node_type} \"{name}\" to \"{home}\""),
                None => format!("add {node_type} \"{name}\""),
            },
            Edit::RemoveNode { name, .. } => format!("remove \"{name}\""),
            Edit::RestoreNode { name, .. } => format!("restore \"{name}\""),
            Edit::SetAttribute { node, attribute, .. } => {
                format!("set {attribute} of \"{node}\"")
            }
            Edit::RenameNode { was, now } => format!("rename \"{was}\" to \"{now}\""),
            Edit::AddLink { link } => format!("link \"{}\" to \"{}\"", link.from, link.to),
            Edit::RemoveLink { link } => {
                format!("unlink \"{}\" from \"{}\"", link.from, link.to)
            }
            Edit::MoveLink { was, now } => format!(
                "move the link from \"{}\" to \"{}\"",
                was.to, now.to
            ),
            Edit::MoveNode { node, .. } => format!("move \"{node}\""),
            Edit::SetAside { node, aside, .. } => {
                if *aside {
                    format!("set aside what \"{node}\" says")
                } else {
                    format!("take up what \"{node}\" says again")
                }
            }
            Edit::Arrange { now, .. } => match now.len() {
                1 => "arrange 1 node".to_string(),
                many => format!("arrange {many} nodes"),
            },
            // Named by what it began with, which is what the gesture was: a
            // node placed is "add i2c "i2c-1"" however many edits followed.
            // The inverse of a group begins with the inverse of its last edit,
            // so this is asked only of a group as it was made.
            Edit::Group { edits } => edits
                .first()
                .map(Edit::describe)
                .unwrap_or_else(|| "nothing".to_string()),
            Edit::SetTcspecial { attribute, .. } => {
                format!("set tcspecial {attribute}")
            }
            Edit::SetSimPayload {
                payload, setting, ..
            } => format!("set {setting} of simulated \"{payload}\""),
            Edit::SetSimGroup { group, setting, .. } => {
                format!("set {setting} of simulator group \"{group}\"")
            }
            Edit::AddSimGroup { name } => {
                format!("add simulator group \"{name}\"")
            }
            Edit::RemoveSimGroup { name, .. } => {
                format!("remove simulator group \"{name}\"")
            }
            Edit::RestoreSimGroup { name, .. } => {
                format!("put back simulator group \"{name}\"")
            }
            Edit::AddGroup { name, .. } => format!("add group \"{name}\""),
            Edit::RemoveGroup { name, .. } => format!("remove group \"{name}\""),
            Edit::SetGroupInterval { group, .. } => {
                format!("set how often \"{group}\" is read")
            }
            Edit::RenameGroup { was, now } => {
                format!("rename group \"{was}\" to \"{now}\"")
            }
            Edit::SetGroupAttribute {
                group, attribute, ..
            } => format!("set {attribute} of every payload in \"{group}\""),
            Edit::RestoreGroupAttribute {
                group, attribute, ..
            } => format!("put back {attribute} on every payload in \"{group}\""),
            Edit::SetNodeComment { node, now, .. } => match now {
                Some(_) => format!("comment on \"{node}\""),
                None => format!("remove the comment on \"{node}\""),
            },
            Edit::SetLinkComment { link, now, .. } => match now {
                Some(_) => format!("comment on \"{}\" to \"{}\"", link.from, link.to),
                None => format!(
                    "remove the comment on \"{}\" to \"{}\"",
                    link.from, link.to
                ),
            },
        }
    }
}

/// The edit history and where in it the document stands.
///
/// `done` is every edit ever made this session, oldest first. `cursor` is how
/// many of them the document currently reflects: everything before it has been
/// applied, everything from it onwards has been undone and can be redone.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Journal {
    done: Vec<Edit>,
    cursor: usize,
}

impl Journal {
    pub fn new() -> Self {
        Journal {
            done: Vec::new(),
            cursor: 0,
        }
    }

    /// Record an edit that has just been applied.
    ///
    /// Anything that had been undone is discarded first. That is the ordinary
    /// behaviour of an editor -- undo three times, then type, and the three
    /// are gone -- and it is why the history is not simply append-only: a
    /// branch nobody can reach is a branch to maintain for nothing.
    pub fn record(&mut self, edit: Edit) {
        self.done.truncate(self.cursor);
        self.done.push(edit);
        self.cursor = self.done.len();
    }

    /// The edit to undo next, without moving the cursor.
    ///
    /// For the menu: what an undo will do is worth showing before it is done.
    pub fn next_undo(&self) -> Option<&Edit> {
        self.cursor.checked_sub(1).and_then(|i| self.done.get(i))
    }

    /// The edit to redo next, without moving the cursor.
    pub fn next_redo(&self) -> Option<&Edit> {
        self.done.get(self.cursor)
    }

    /// Step back one edit, handing back the inverse to apply.
    pub fn undo(&mut self) -> Option<Edit> {
        let i = self.cursor.checked_sub(1)?;
        let inverse = self.done.get(i)?.inverse();
        self.cursor = i;
        Some(inverse)
    }

    /// Step forward one edit, handing back the edit to apply again.
    pub fn redo(&mut self) -> Option<Edit> {
        let edit = self.done.get(self.cursor)?.clone();
        self.cursor += 1;
        Some(edit)
    }

    /// How many edits can still be undone. The depth of the undo, which the
    /// window shows: a person about to abandon an afternoon's work deserves to
    /// know how much of it undo still holds.
    pub fn depth(&self) -> usize {
        self.cursor
    }

    /// How many can be redone.
    pub fn redoable(&self) -> usize {
        self.done.len() - self.cursor
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(name: &str) -> Edit {
        Edit::AddNode {
            name: name.to_string(),
            node_type: "i2c".to_string(),
            home: None,
        }
    }

    /// Undo reaches back to the beginning, however long the session was.
    ///
    /// The depth requirement, stated as a number large enough that a bounded
    /// history would fail it: a thousand edits is an afternoon's work and all
    /// of it must be reachable.
    #[test]
    fn undo_reaches_the_start_of_a_long_session() {
        let mut journal = Journal::new();
        for n in 0..1000 {
            journal.record(add(&format!("node{n}")));
        }
        assert_eq!(journal.depth(), 1000);

        for _ in 0..1000 {
            assert!(journal.undo().is_some());
        }
        assert_eq!(journal.depth(), 0);
        assert!(journal.undo().is_none(), "undo went past the beginning");
        assert_eq!(journal.redoable(), 1000, "redo lost the history");
    }

    /// Every edit's inverse undoes exactly it.
    ///
    /// Checked as a property of each variant rather than through the document,
    /// because an inverse that needed the document to be right would be the
    /// bug this guards against.
    #[test]
    fn an_inverse_puts_back_what_the_edit_changed() {
        let set = Edit::SetAttribute {
            node: "temp_1".into(),
            attribute: "address".into(),
            was: Some("0x48".into()),
            now: Some("0x49".into()),
        };
        assert_eq!(
            set.inverse(),
            Edit::SetAttribute {
                node: "temp_1".into(),
                attribute: "address".into(),
                was: Some("0x49".into()),
                now: Some("0x48".into()),
            }
        );

        // An attribute that was not set before comes back unset, not empty.
        let first = Edit::SetAttribute {
            node: "n".into(),
            attribute: "a".into(),
            was: None,
            now: Some("1".into()),
        };
        match first.inverse() {
            Edit::SetAttribute { now, .. } => assert_eq!(
                now, None,
                "undoing a first setting left the attribute present and empty"
            ),
            other => panic!("{other:?}"),
        }

        let renamed = Edit::RenameNode {
            was: "a".into(),
            now: "b".into(),
        };
        assert_eq!(
            renamed.inverse(),
            Edit::RenameNode {
                was: "b".into(),
                now: "a".into()
            }
        );

        let moved = Edit::MoveNode {
            node: "n".into(),
            was: (10, 20),
            now: (30, 40),
        };
        assert_eq!(
            moved.inverse(),
            Edit::MoveNode {
                node: "n".into(),
                was: (30, 40),
                now: (10, 20)
            }
        );

        // And an inverse of an inverse is the edit again, for every variant
        // above: an undo followed by a redo must land where it started.
        for edit in [set, renamed, moved] {
            assert_eq!(edit.inverse().inverse(), edit);
        }
    }

    /// Removing a node carries its links, so undo restores the whole thing.
    ///
    /// The case a shallow undo gets wrong: a node removed takes its links with
    /// it, and an undo that puts back only the node leaves a graph missing the
    /// connections it had.
    #[test]
    fn undoing_a_removal_restores_the_links_as_well() {
        let removal = Edit::RemoveNode {
            name: "k_1".into(),
            node_type: "compute".into(),
            attributes: vec![("function".into(), "to_kelvin".into())],
            links: vec![
                Link::new("temp_1", "k_1", None),
                Link::new("k_1", "cabin_temp", Some("in0")),
            ],
            home: None,
        };

        match removal.inverse() {
            Edit::RestoreNode {
                name,
                attributes,
                links,
                ..
            } => {
                assert_eq!(name, "k_1");
                assert_eq!(attributes.len(), 1);
                assert_eq!(links.len(), 2, "the node came back without its links");
            }
            other => panic!("{other:?}"),
        }
    }

    /// An edit after an undo discards what had been undone.
    #[test]
    fn editing_after_an_undo_drops_the_redo_history() {
        let mut journal = Journal::new();
        journal.record(add("a"));
        journal.record(add("b"));
        journal.record(add("c"));

        journal.undo();
        journal.undo();
        assert_eq!(journal.redoable(), 2);

        journal.record(add("d"));
        assert_eq!(journal.redoable(), 0, "a branch nobody can reach was kept");
        assert_eq!(journal.depth(), 2);
    }

    /// The history survives being written out and read back.
    ///
    /// Which is what makes undo reach past a crash: the document alone would
    /// come back with no history at all.
    #[test]
    fn a_journal_written_and_read_is_the_same_journal() {
        let mut journal = Journal::new();
        journal.record(add("a"));
        journal.record(Edit::AddLink {
            link: Link::new("a", "b", Some("in0")),
        });
        journal.undo();

        let written = serde_yaml::to_string(&journal).expect("a journal writes");
        let read: Journal = serde_yaml::from_str(&written).expect("and reads back");

        assert_eq!(read, journal);
        assert_eq!(read.depth(), 1);
        assert_eq!(read.redoable(), 1, "redo did not survive the round trip");
    }

    /// What the menu says an undo will do.
    #[test]
    fn the_next_undo_can_be_named_before_it_happens() {
        let mut journal = Journal::new();
        assert!(journal.next_undo().is_none());

        journal.record(add("temp_1"));
        let said = journal.next_undo().expect("something to undo").describe();
        assert!(
            said.contains("temp_1") && said.contains("i2c"),
            "the description should name what will be undone: {said}"
        );

        journal.undo();
        assert!(journal.next_undo().is_none());
        assert!(journal.next_redo().is_some());
    }
}
