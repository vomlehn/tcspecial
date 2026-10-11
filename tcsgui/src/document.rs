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
use crate::style::NodeType;

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
}

/// The whole of what is being edited.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
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
        }
    }
}

impl Document {
    pub fn new() -> Self {
        Document::default()
    }

    pub fn node(&self, name: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.name == name)
    }

    fn node_mut(&mut self, name: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.name == name)
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
            Edit::AddNode { name, node_type } => {
                self.nodes.push(Node {
                    name: name.clone(),
                    node_type: node_type.clone(),
                    attributes: BTreeMap::new(),
                    at: (0, 0),
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
            } => {
                self.nodes.push(Node {
                    name: name.clone(),
                    node_type: node_type.clone(),
                    attributes: attributes.iter().cloned().collect(),
                    at: (0, 0),
                });
                for link in links {
                    if !self.links.contains(link) {
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
                if !self.links.contains(link) {
                    self.links.push(link.clone());
                }
            }
            Edit::RemoveLink { link } => {
                self.links.retain(|l| l != link);
            }
            Edit::MoveNode { node, now, .. } => {
                if let Some(node) = self.node_mut(node) {
                    node.at = *now;
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

        let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
        for node in &self.nodes {
            if node.name.trim().is_empty() {
                faults.push(Fault::NamelessNode { at: node.at });
                continue;
            }
            *seen.entry(node.name.as_str()).or_insert(0) += 1;

            if !NodeType::ALL
                .iter()
                .any(|t| t.spelling() == node.node_type)
            {
                faults.push(Fault::UnknownType {
                    node: node.name.clone(),
                    node_type: node.node_type.clone(),
                });
            }
        }
        for (name, count) in seen {
            if count > 1 {
                faults.push(Fault::DuplicateName {
                    name: name.to_string(),
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

    /// The document as a configuration file.
    ///
    /// Written in the shape `design.rst` specifies: a `nodes` list where each
    /// entry carries its `type` and its attributes, and a `links` list. The
    /// canvas positions are written as well, under a key the flight software
    /// ignores, so that reopening the file finds the diagram as it was left.
    pub fn to_yaml(&self) -> String {
        let mut out = String::new();
        out.push_str("version: \"2.0\"\n\nnodes:\n");
        for node in &self.nodes {
            out.push_str(&format!(
                "  - name: {}\n    type: {}\n",
                node.name, node.node_type
            ));
            for (attribute, value) in &node.attributes {
                out.push_str(&format!("    {attribute}: {value}\n"));
            }
            out.push_str(&format!(
                "    editor: {{ at: [{}, {}] }}\n",
                node.at.0, node.at.1
            ));
        }

        out.push_str("\nlinks:\n");
        for link in &self.links {
            match &link.input {
                Some(input) => out.push_str(&format!(
                    "  - {{ from: {}, to: {}.{} }}\n",
                    link.from, link.to, input
                )),
                None => out.push_str(&format!(
                    "  - {{ from: {}, to: {} }}\n",
                    link.from, link.to
                )),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(name: &str, node_type: &str) -> Edit {
        Edit::AddNode {
            name: name.to_string(),
            node_type: node_type.to_string(),
        }
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
                link: Link {
                    from: "temp_fore".into(),
                    to: "k_fore".into(),
                    input: None,
                },
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
            link: Link {
                from: "a".into(),
                to: "b".into(),
                input: Some("in0".into()),
            },
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
                link: Link {
                    from: "a".into(),
                    to: "b".into(),
                    input: Some("in0".into()),
                },
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
        let link = Link {
            from: "a".into(),
            to: "b".into(),
            input: None,
        };
        document.apply(&Edit::AddLink { link: link.clone() });

        // As the window would build it: the removal carries what is being
        // removed, which is what makes the inverse complete.
        let node = document.node("a").unwrap().clone();
        let removal = Edit::RemoveNode {
            name: node.name.clone(),
            node_type: node.node_type.clone(),
            attributes: node.attributes.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            links: vec![link.clone()],
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
            link: Link {
                from: "a".into(),
                to: "b".into(),
                input: Some("in0".into()),
            },
        });

        document.apply(&Edit::RenameNode {
            was: "a".into(),
            now: "temp_fore".into(),
        });

        assert_eq!(document.links[0].from, "temp_fore");
        assert!(
            document.faults().is_empty(),
            "renaming left the document faulty: {:?}",
            document.faults()
        );
    }

    /// The faults the editor finds, each of which is what red marks.
    #[test]
    fn a_document_says_what_is_wrong_with_it() {
        let mut document = Document::new();
        assert!(document.faults().is_empty(), "an empty document is not faulty");

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
            link: Link {
                from: "a".into(),
                to: "nowhere".into(),
                input: None,
            },
        });
        assert!(
            document.faults().iter().any(|f| matches!(f, Fault::DanglingLink { .. })),
            "{:?}",
            document.faults()
        );

        let mut document = Document::new();
        document.apply(&add("a", "contraption"));
        match document.faults().as_slice() {
            [Fault::UnknownType { node_type, .. }] => assert_eq!(node_type, "contraption"),
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
                link: Link {
                    from: from.into(),
                    to: "m".into(),
                    input: Some("in0".into()),
                },
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
            link: Link {
                from: "temp_fore".into(),
                to: "mean_temp".into(),
                input: Some("in0".into()),
            },
        });

        let yaml = document.to_yaml();
        assert!(yaml.contains("version: \"2.0\""), "{yaml}");
        assert!(yaml.contains("type: i2c"), "{yaml}");
        assert!(yaml.contains("address: 0x48"), "{yaml}");
        assert!(
            yaml.contains("{ from: temp_fore, to: mean_temp.in0 }"),
            "a link into a named input is written as one: {yaml}"
        );
    }
}
