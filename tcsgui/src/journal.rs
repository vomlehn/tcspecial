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

/// One change to the document, carrying what is needed to undo it.
///
/// Deliberately coarse: a node moved on the canvas is one edit, not a hundred
/// as the pointer travels. The window decides when a gesture has finished and
/// records one edit for it, which is what makes undo land where a person
/// expects rather than one pixel back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Edit {
    /// A node was added. Undone by removing it.
    AddNode { name: String, node_type: String },
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
    },
    /// A node was moved on the canvas. Not a change to the configuration --
    /// the vehicle does not care where the box is -- but it is in the history
    /// because a person who drags a node expects undo to put it back.
    MoveNode {
        node: String,
        was: (i32, i32),
        now: (i32, i32),
    },
}

/// One connection, from a node to a named input of another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub from: String,
    pub to: String,
    /// Which input of the destination, where the destination has more than
    /// one. `None` is a destination that takes a single input.
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
        self.from == other.from && self.to == other.to && self.input == other.input
    }

    /// A link joining these, with nothing written about it.
    ///
    /// Used by the tests today, and by drawing a link on the canvas when that
    /// lands -- which is why it is here rather than gated to the tests: the
    /// one caller it is waiting for is the obvious one.
    #[allow(dead_code)]
    pub fn new(from: &str, to: &str, input: Option<&str>) -> Link {
        Link {
            from: from.to_string(),
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
            Edit::AddNode { name, node_type } => Edit::RemoveNode {
                name: name.clone(),
                node_type: node_type.clone(),
                attributes: Vec::new(),
                links: Vec::new(),
            },
            Edit::RemoveNode {
                name,
                node_type,
                attributes,
                links,
            } => {
                // Undoing a removal is more than an addition: the attributes
                // and the links come back as well. The caller applies this as
                // a group, which `Journal::undo` does.
                Edit::RestoreNode {
                    name: name.clone(),
                    node_type: node_type.clone(),
                    attributes: attributes.clone(),
                    links: links.clone(),
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
            Edit::MoveNode { node, was, now } => Edit::MoveNode {
                node: node.clone(),
                was: *now,
                now: *was,
            },
            Edit::SetNodeComment { node, was, now } => Edit::SetNodeComment {
                node: node.clone(),
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
            } => Edit::RemoveNode {
                name: name.clone(),
                node_type: node_type.clone(),
                attributes: attributes.clone(),
                links: links.clone(),
            },
        }
    }

    /// A short phrase for the undo menu, so that what a press will undo is
    /// readable before it is pressed.
    pub fn describe(&self) -> String {
        match self {
            Edit::AddNode { name, node_type } => format!("add {node_type} \"{name}\""),
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
            Edit::MoveNode { node, .. } => format!("move \"{node}\""),
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

    /// Every edit the document reflects, oldest first.
    ///
    /// What a fresh document is rebuilt from after a crash.
    pub fn applied(&self) -> &[Edit] {
        &self.done[..self.cursor]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(name: &str) -> Edit {
        Edit::AddNode {
            name: name.to_string(),
            node_type: "i2c".to_string(),
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
            node: "temp_fore".into(),
            attribute: "address".into(),
            was: Some("0x48".into()),
            now: Some("0x49".into()),
        };
        assert_eq!(
            set.inverse(),
            Edit::SetAttribute {
                node: "temp_fore".into(),
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
            name: "k_fore".into(),
            node_type: "compute".into(),
            attributes: vec![("function".into(), "to_kelvin".into())],
            links: vec![
                Link::new("temp_fore", "k_fore", None),
                Link::new("k_fore", "mean_temp", Some("in0")),
            ],
        };

        match removal.inverse() {
            Edit::RestoreNode {
                name,
                attributes,
                links,
                ..
            } => {
                assert_eq!(name, "k_fore");
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

        journal.record(add("temp_fore"));
        let said = journal.next_undo().expect("something to undo").describe();
        assert!(
            said.contains("temp_fore") && said.contains("i2c"),
            "the description should name what will be undone: {said}"
        );

        journal.undo();
        assert!(journal.next_undo().is_none());
        assert!(journal.next_redo().is_some());
    }
}
