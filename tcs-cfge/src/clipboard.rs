//! Copy, Cut and Paste: nodes taken off one page and put on another, or on
//! the same one again.
//!
//! What is copied is the nodes as the editor holds them -- type, attributes,
//! comment, place -- and the lines that run between two of them. A line with
//! only one end among them is left behind: pasted, its other end would be a
//! node that is not there, or the original of a copy, and a line nobody drew
//! from the copy is not one to make for them. A subdiagram's box is not
//! copied, its contents being a file of their own; nor is what the
//! simulator's file says about a payload, nor a command binding, nor what a
//! page exports, which are about the configuration the node was in rather
//! than about the node.
//!
//! The clipboard is the editor's own and not the desktop's. It lives as long
//! as the editor does, so nodes copied from one configuration can be pasted
//! into the next one opened; and it is never read back from text somebody
//! else put there, which is what a desktop clipboard would mean.
//!
//! A paste is one edit, a group, so one Undo takes the whole of it back --
//! and Cut likewise.

use std::collections::{BTreeMap, BTreeSet};

use crate::document::{Document, Node};
use crate::journal::{Edit, Link};
use crate::subdiagram::{Home, Qualified};
use tcs_diagram::pane::{self, Dividers};

/// What Copy or Cut took: nodes of one page, and the lines between them.
///
/// Names here are each node's own, within its page, so that the clipping
/// can be put on any page: a pasted node is given the page it lands on.
#[derive(Debug, Clone, PartialEq)]
pub struct Clipping {
    /// The nodes, in the order the document holds them, each with its
    /// attributes, comment and place, and its page left out.
    pub nodes: Vec<Node>,
    /// The lines with both ends among `nodes`, each end named by the node's
    /// own name.
    pub links: Vec<Link>,
}

/// Takes a copy of some of a page's nodes and the lines between them.
///
/// # Arguments
///
/// * `document` - The configuration.
/// * `home`     - The page the nodes are on: `None` for the top.
/// * `names`    - The nodes, each by its own name within `home`. A name no
///                node has is passed over, as a node deleted while it was
///                selected.
///
/// # Returns
///
/// The clipping.
///
/// # Errors
///
/// A sentence saying why nothing was copied: nothing selected is on the
/// page, or a subdiagram's box is among what is.
pub fn copy(
    document: &Document,
    home: &Home,
    names: &[String],
) -> Result<Clipping, String> {
    let wanted: BTreeSet<&str> = names.iter().map(String::as_str).collect();
    let nodes: Vec<Node> = document
        .nodes
        .iter()
        .filter(|node| {
            &node.home == home && wanted.contains(node.name.as_str())
        })
        .cloned()
        .map(|node| Node { home: None, ..node })
        .collect();
    if nodes.is_empty() {
        return Err("Nothing is selected to copy: select a node first, \
		            or several with Ctrl or Shift held."
            .to_string());
    }
    if let Some(boxed) =
        nodes.iter().find(|node| node.node_type == "subdiagram")
    {
        return Err(format!(
            "\"{}\" is a subdiagram, which is not copied: its nodes are \
			 a file of their own. Select the nodes inside it, on its \
			 own page, instead.",
            boxed.name
        ));
    }
    let ours: BTreeSet<&str> =
        nodes.iter().map(|node| node.name.as_str()).collect();
    // A line's ends as their own names, where both are on this page and
    // among the nodes taken.
    let own = |name: &str| {
        let said = Qualified::parse(name);
        (said.in_ == *home && ours.contains(said.node.as_str()))
            .then_some(said.node)
    };
    let links = document
        .links
        .iter()
        .filter_map(|link| {
            let from = own(&link.from)?;
            let to = own(&link.to)?;
            Some(Link {
                from,
                to,
                ..link.clone()
            })
        })
        .collect();
    Ok(Clipping { nodes, links })
}

/// The edit that puts a clipping on a page.
///
/// Each node keeps its own name where no node has it -- what a Cut and a
/// Paste moving nodes from one page to another wants -- and is otherwise
/// given the first free one in the editor's way of numbering, `temp-2` for
/// `temp`. Each keeps its place, moved down the sheet a step at a time
/// until none of them lands on the corner of a node already on the page, so
/// a copy pasted beside its original is seen beside it rather than hiding
/// under it; each is then held inside its own pane, as a dragged node is.
/// The lines between them are drawn again between the copies.
///
/// # Arguments
///
/// * `document` - The configuration pasted into.
/// * `clipping` - What was copied.
/// * `home`     - The page pasted on: `None` for the top.
/// * `dividers` - Where the working panes meet.
///
/// # Returns
///
/// The edit, and each pasted node's own name on the page, in the order of
/// the clipping; `None` for a clipping with no nodes.
pub fn paste(
    document: &Document,
    clipping: &Clipping,
    home: &Home,
    dividers: Dividers,
) -> Option<(Edit, Vec<String>)> {
    if clipping.nodes.is_empty() {
        return None;
    }
    let mut given = BTreeSet::new();
    let mut renamed = BTreeMap::new();
    for node in &clipping.nodes {
        let name = free_name(document, &node.name, &given);
        given.insert(name.clone());
        renamed.insert(node.name.clone(), name);
    }

    let step = (pane::NODE_STEP_MM as f64 * pane::PX_PER_MM).round() as i32;
    let corners: BTreeSet<(i32, i32)> = document
        .nodes
        .iter()
        .filter(|node| &node.home == home)
        .map(|node| node.at)
        .collect();
    let placed = |down: i32| -> Vec<(i32, i32)> {
        clipping
            .nodes
            .iter()
            .map(|node| {
                pane::confine(
                    &node.node_type,
                    dividers,
                    (node.at.0, node.at.1 + down),
                )
            })
            .collect()
    };
    // As many steps as there are nodes on the page and one more is always
    // enough: each step can be blocked by a different one of them at most
    // once for each pasted node, and the sheet goes on down.
    let tries = (corners.len() + 1) * clipping.nodes.len() + 1;
    let places = (0..tries as i32)
        .map(|n| placed(n * step))
        .find(|places| places.iter().all(|at| !corners.contains(at)))
        .unwrap_or_else(|| placed(0));

    let mut edits = Vec::new();
    let mut names = Vec::new();
    for (node, at) in clipping.nodes.iter().zip(places) {
        let name = renamed[&node.name].clone();
        let whole = Qualified::of(&name, home).absolute();
        edits.push(Edit::AddNode {
            name: name.clone(),
            node_type: node.node_type.clone(),
            home: home.clone(),
        });
        for (attribute, value) in &node.attributes {
            edits.push(Edit::SetAttribute {
                node: whole.clone(),
                attribute: attribute.clone(),
                was: None,
                now: Some(value.clone()),
            });
        }
        if node.comment.is_some() {
            edits.push(Edit::SetNodeComment {
                node: whole.clone(),
                was: None,
                now: node.comment.clone(),
            });
        }
        edits.push(Edit::MoveNode {
            node: whole,
            was: (0, 0),
            now: at,
        });
        names.push(name);
    }
    let whole = |name: &str| Qualified::of(&renamed[name], home).absolute();
    for link in &clipping.links {
        edits.push(Edit::AddLink {
            link: Link {
                from: whole(&link.from),
                to: whole(&link.to),
                ..link.clone()
            },
        });
    }
    Some((Edit::Group { edits }, names))
}

/// The edit that takes some of a page's nodes away, with the lines that
/// touch them: what Cut does, and Delete.
///
/// One removal per node, each worked out on the document as the ones before
/// it left it, so that a line between two of them is carried once and comes
/// back once. Each node is first moved to where a node put back is put, and
/// its comment taken off, so that the undo -- which puts a removed node back
/// there without one -- then moves it home and gives it its words again.
///
/// # Arguments
///
/// * `document` - The configuration.
/// * `home`     - The page: `None` for the top.
/// * `names`    - The nodes, each by its own name within `home`; a name no
///                node has is passed over.
///
/// # Returns
///
/// The edit; `None` where none of the nodes is there.
pub fn removal(
    document: &Document,
    home: &Home,
    names: &[String],
) -> Option<Edit> {
    let mut left = document.clone();
    let mut edits = Vec::new();
    for name in names {
        let whole = Qualified::of(name, home).absolute();
        let Some(node) = left.node(&whole).cloned() else {
            continue;
        };
        let Some(removed) = left.removal_of(&whole) else {
            continue;
        };
        let mut these = Vec::new();
        if node.comment.is_some() {
            these.push(Edit::SetNodeComment {
                node: whole.clone(),
                was: node.comment.clone(),
                now: None,
            });
        }
        if node.at != (0, 0) {
            these.push(Edit::MoveNode {
                node: whole.clone(),
                was: node.at,
                now: (0, 0),
            });
        }
        these.push(removed);
        for edit in &these {
            left.apply(edit);
        }
        edits.extend(these);
    }
    (!edits.is_empty()).then_some(Edit::Group { edits })
}

/// A name for a pasted node that no node and no physics component has, nor
/// any node pasted with it.
///
/// The name itself where it is free; otherwise its stem -- the name without
/// a number on the end -- with the first free number: `i2c` and `i2c-1` both
/// become `i2c-2` where those two are taken. Free across every file of the
/// configuration, which is what the reader asks of a node's name.
///
/// # Arguments
///
/// * `document` - The configuration pasted into.
/// * `name`     - The copied node's name.
/// * `given`    - The names given to the nodes pasted before it.
///
/// # Returns
///
/// The name.
///
/// # Panics
///
/// Never in practice: the search for a free number cannot run out before
/// the nodes do.
fn free_name(
    document: &Document,
    name: &str,
    given: &BTreeSet<String>,
) -> String {
    let taken = |name: &str| {
        given.contains(name)
            || document.nodes.iter().any(|node| node.name == name)
            || document.simulator.as_ref().is_some_and(|simulator| {
                simulator.world.components.contains_key(name)
            })
    };
    if !taken(name) {
        return name.to_string();
    }
    let stem = match name.rsplit_once('-') {
        Some((stem, number))
            if !stem.is_empty()
                && !number.is_empty()
                && number.bytes().all(|b| b.is_ascii_digit()) =>
        {
            stem
        }
        _ => name,
    };
    (1..)
        .map(|n| format!("{stem}-{n}"))
        .find(|name| !taken(name))
        .expect("a free name exists")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::Editor;

    /// Where the dividers sit until somebody moves them.
    ///
    /// # Returns
    ///
    /// The dividers the editor opens with.
    fn dividers() -> Dividers {
        pane::dividers_for(
            crate::settings::DEFAULT_GATEWAY,
            crate::settings::DEFAULT_VEHICLE,
        )
    }

    /// A page of four nodes in a chain, `sensor` to `scale` to `mean` to
    /// `ground`, on the page `home`, each where a person might have put it,
    /// `scale` with a comment.
    ///
    /// # Arguments
    ///
    /// * `home` - The page: `None` for the top.
    ///
    /// # Returns
    ///
    /// The configuration.
    fn chain(home: &Home) -> Document {
        let mut document = Document::new();
        let vehicle = dividers().vehicle;
        let payloads = dividers().payloads;
        for (name, node_type, at) in [
            ("ground", "gateway_radio", (16, 120)),
            ("mean", "compute", (vehicle - 200, 120)),
            ("scale", "compute", (payloads + 40, 120)),
            ("sensor", "i2c", (vehicle + 24, 120)),
        ] {
            let whole = Qualified::of(name, home).absolute();
            for edit in [
                Edit::AddNode {
                    name: name.to_string(),
                    node_type: node_type.to_string(),
                    home: home.clone(),
                },
                Edit::MoveNode {
                    node: whole.clone(),
                    was: (0, 0),
                    now: pane::confine(node_type, dividers(), at),
                },
            ] {
                document.apply(&edit);
            }
        }
        let whole = |name: &str| Qualified::of(name, home).absolute();
        document.apply(&Edit::SetAttribute {
            node: whole("scale"),
            attribute: "function".to_string(),
            was: None,
            now: Some("scale".to_string()),
        });
        document.apply(&Edit::SetNodeComment {
            node: whole("scale"),
            was: None,
            now: Some("Volts to kelvin.".to_string()),
        });
        for (from, to) in
            [("sensor", "scale"), ("scale", "mean"), ("mean", "ground")]
        {
            document.apply(&Edit::AddLink {
                link: Link::joining(
                    &whole(from),
                    Some("world-out"),
                    &whole(to),
                    None,
                ),
            });
        }
        document
    }

    /// The names of the links that run between two of `names`.
    ///
    /// # Arguments
    ///
    /// * `document` - The configuration.
    /// * `names`    - The nodes, by their absolute names.
    ///
    /// # Returns
    ///
    /// Each such link's ends, by name, in the document's order.
    fn between(document: &Document, names: &[&str]) -> Vec<(String, String)> {
        document
            .links
            .iter()
            .filter(|link| {
                names.contains(&link.from.as_str())
                    && names.contains(&link.to.as_str())
            })
            .map(|link| (link.from.clone(), link.to.clone()))
            .collect()
    }

    /// A copy takes the nodes and the lines between them, and leaves
    /// behind a line with an end outside them.
    #[test]
    fn a_copy_takes_the_lines_between_the_nodes_and_no_others() {
        let document = chain(&None);
        let clipping =
            copy(&document, &None, &["scale".into(), "sensor".into()])
                .expect("it copies");
        let names: Vec<&str> =
            clipping.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["scale", "sensor"], "in the document's order");
        let lines: Vec<(&str, &str)> = clipping
            .links
            .iter()
            .map(|l| (l.from.as_str(), l.to.as_str()))
            .collect();
        assert_eq!(
            lines,
            [("sensor", "scale")],
            "scale to mean has an end outside what was copied"
        );
        let scale = &clipping.nodes[0];
        assert_eq!(scale.comment.as_deref(), Some("Volts to kelvin."));
        assert_eq!(scale.attributes["function"], "scale");
    }

    /// Pasted beside the originals, the copies are given free names, are
    /// moved off the originals' corners, and are joined to each other and
    /// to nothing else; one Undo takes the whole paste back.
    #[test]
    fn a_paste_is_renamed_moved_off_its_originals_and_undone_at_once() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut editor =
            Editor::open(&where_.path().join("pasted.ytcs"), dividers())
                .expect("it opens");
        for edit in chain(&None).nodes.iter().flat_map(|node| {
            let mut edits = vec![
                Edit::AddNode {
                    name: node.name.clone(),
                    node_type: node.node_type.clone(),
                    home: None,
                },
                Edit::MoveNode {
                    node: node.name.clone(),
                    was: (0, 0),
                    now: node.at,
                },
            ];
            edits.extend(node.comment.iter().map(|comment| {
                Edit::SetNodeComment {
                    node: node.name.clone(),
                    was: None,
                    now: Some(comment.clone()),
                }
            }));
            edits
        }) {
            editor.edit(edit);
        }
        for link in chain(&None).links {
            editor.edit(Edit::AddLink { link });
        }
        let before = editor.document().clone();
        let depth = editor.undo_depth();

        let clipping = copy(
            editor.document(),
            &None,
            &["scale".into(), "sensor".into(), "mean".into()],
        )
        .expect("it copies");
        let (edit, names) =
            paste(editor.document(), &clipping, &None, dividers())
                .expect("there is something to paste");
        assert_eq!(names, ["mean-1", "scale-1", "sensor-1"]);
        editor.edit(edit);

        let document = editor.document();
        assert_eq!(document.nodes.len(), 7);
        for name in ["mean", "scale", "sensor"] {
            let original = document.node(name).expect("still there");
            let copied = document.node(&format!("{name}-1")).expect("pasted");
            assert_eq!(copied.node_type, original.node_type);
            assert_eq!(copied.comment, original.comment);
            assert_eq!(copied.attributes, original.attributes);
            assert_ne!(copied.at, original.at, "{name} pasted over itself");
            assert_eq!(copied.at.0, original.at.0, "moved down, not across");
        }
        let mut copied = between(document, &["mean-1", "scale-1", "sensor-1"]);
        copied.sort();
        assert_eq!(
            copied,
            [
                ("scale-1".to_string(), "mean-1".to_string()),
                ("sensor-1".to_string(), "scale-1".to_string()),
            ],
            "the lines between the copies, and none to the originals"
        );
        assert_eq!(document.links.len(), 5);

        assert_eq!(editor.undo_depth(), depth + 1, "one edit");
        editor.undo();
        assert_eq!(*editor.document(), before, "one Undo takes it all back");
    }

    /// Cut and then pasted somewhere the names are free, nodes keep their
    /// names and their places; and pasted onto another page, they are of
    /// that page, and so are the lines between them.
    #[test]
    fn a_cut_pasted_keeps_its_names_and_lands_on_its_page() {
        let document = chain(&None);
        let names: Vec<String> = vec!["scale".into(), "mean".into()];
        let clipping = copy(&document, &None, &names).expect("it copies");
        let mut cut = document.clone();
        cut.apply(&removal(&cut, &None, &names).expect("a removal"));
        assert!(cut.node("scale").is_none() && cut.node("mean").is_none());

        let (edit, given) =
            paste(&cut, &clipping, &None, dividers()).expect("a paste");
        assert_eq!(given, ["mean", "scale"]);
        let mut back = cut.clone();
        back.apply(&edit);
        for name in ["mean", "scale"] {
            assert_eq!(
                back.node(name).map(|node| node.at),
                document.node(name).map(|node| node.at),
                "{name} put back where it was cut from"
            );
        }

        let page = Some("zone".to_string());
        let (edit, given) =
            paste(&cut, &clipping, &page, dividers()).expect("a paste");
        assert_eq!(given, ["mean", "scale"]);
        let mut there = cut.clone();
        there.apply(&edit);
        assert_eq!(
            there.node("zone/scale").map(|n| n.home.clone()),
            Some(page)
        );
        assert_eq!(
            between(&there, &["zone/scale", "zone/mean"]),
            [("zone/scale".to_string(), "zone/mean".to_string())]
        );
    }

    /// Several nodes taken away together carry the line between them once,
    /// and the undo puts every one back where it was, with its words and
    /// its lines, and no line twice.
    #[test]
    fn a_removal_of_several_is_undone_to_where_they_were() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut editor =
            Editor::open(&where_.path().join("removed.ytcs"), dividers())
                .expect("it opens");
        let page = Some("zone".to_string());
        let built = chain(&page);
        for node in &built.nodes {
            let whole = Qualified::of(&node.name, &page).absolute();
            editor.edit(Edit::AddNode {
                name: node.name.clone(),
                node_type: node.node_type.clone(),
                home: page.clone(),
            });
            editor.edit(Edit::MoveNode {
                node: whole.clone(),
                was: (0, 0),
                now: node.at,
            });
            if let Some(comment) = &node.comment {
                editor.edit(Edit::SetNodeComment {
                    node: whole,
                    was: None,
                    now: Some(comment.clone()),
                });
            }
        }
        for link in built.links {
            editor.edit(Edit::AddLink { link });
        }
        let before = editor.document().clone();

        let edit = removal(
            editor.document(),
            &page,
            &["scale".into(), "mean".into(), "gone".into()],
        )
        .expect("a removal");
        editor.edit(edit);
        assert_eq!(editor.document().nodes.len(), 2);
        assert_eq!(
            editor.document().links.len(),
            0,
            "every line touched one of them"
        );
        editor.undo();
        let mut now = editor.document().clone();
        let mut was = before.clone();
        // The order nodes are listed in is not what was asked about.
        now.nodes.sort_by(|a, b| a.name.cmp(&b.name));
        was.nodes.sort_by(|a, b| a.name.cmp(&b.name));
        now.links
            .sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));
        was.links
            .sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));
        assert_eq!(now.nodes, was.nodes, "each where it was, with its words");
        assert_eq!(now.links, was.links, "each line back, and once");

        assert!(removal(editor.document(), &page, &["gone".into()]).is_none());
    }

    /// A subdiagram's box is not copied, and nothing selected is nothing
    /// to copy; both say why.
    #[test]
    fn a_box_and_nothing_are_not_copied() {
        let mut document = chain(&None);
        document.apply(&Edit::AddNode {
            name: "zone".to_string(),
            node_type: "subdiagram".to_string(),
            home: None,
        });
        let why = copy(&document, &None, &["zone".into(), "mean".into()])
            .expect_err("a box is refused");
        assert!(why.contains("\"zone\" is a subdiagram"), "{why}");
        let why = copy(&document, &None, &[]).expect_err("nothing");
        assert!(why.contains("Nothing is selected"), "{why}");
        // A name on another page is not on this one.
        assert!(
            copy(&document, &Some("zone".into()), &["mean".into()]).is_err()
        );
    }

    /// A pasted node's name is free of every node in every file and of the
    /// physics components, and of the nodes pasted with it.
    #[test]
    fn a_pasted_name_is_free_everywhere() {
        let mut document = chain(&None);
        document.apply(&Edit::AddNode {
            name: "mean-1".to_string(),
            node_type: "compute".to_string(),
            home: Some("zone".to_string()),
        });
        let given = BTreeSet::from(["mean-2".to_string()]);
        assert_eq!(free_name(&document, "mean", &given), "mean-3");
        assert_eq!(free_name(&document, "mean-1", &BTreeSet::new()), "mean-2");
        assert_eq!(free_name(&document, "fresh", &BTreeSet::new()), "fresh");
    }
}
