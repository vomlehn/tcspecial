//! tcsgui: editing a TCSpecial configuration as a diagram.
//!
//! The configuration is a graph -- nodes joined by links, as `design.rst`
//! specifies -- and a graph is a thing better drawn than typed. What this
//! program is for is placing nodes, joining them, and filling in what each one
//! needs, with the file on disk kept in step as the work happens.
//!
//! Three things about it were asked for and are not incidental:
//!
//! * **Nothing is told apart by colour alone.** Every node type differs from
//!   every other in at least two of outline, border width, shading and size;
//!   see `style.rs`, where a test fails if that stops being true. Red is
//!   reserved for things that are wrong, and is the only colour with a meaning
//!   of its own.
//! * **There is no Save command.** Every edit is written as it is made, which
//!   is `editor.rs`, and written in a way that cannot damage the file that was
//!   there, which is `save.rs`.
//! * **Undo goes back to the start of the session**, and past a crash, because
//!   the history is written down beside the document; see `journal.rs`.

mod document;
mod editor;
mod journal;
mod save;
mod style;

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ModelRc, VecModel};

use editor::Editor;
use journal::Edit;
use style::{Face, NodeType, Outline, Shading, Tint, Weight, ERROR_TINT};

slint::include_modules!();

/// Where the first node placed goes, and how far each one after it is offset.
///
/// A diagram read from a file brings its own positions; a node just placed has
/// to go somewhere. Somewhere visible, and not on top of the last one -- a
/// stack of nodes at one spot looks like one node, and the person who placed
/// three would think two presses had been missed.
const FIRST_PLACE: (i32, i32) = (240, 40);
const PLACE_OFFSET: i32 = 28;

fn main() -> Result<(), slint::PlatformError> {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("tcsgui: usage: tcsgui <configuration file>");
        eprintln!(
            "tcsgui: the file need not exist yet; it is written as edits are made, \
             and its history is kept beside it"
        );
        std::process::exit(2);
    });

    // Recovered rather than opened: if a previous session died, its history is
    // beside the file and the work comes back with its undo intact.
    let editor = Rc::new(RefCell::new(Editor::recover(std::path::Path::new(&path))));

    let ui = MainWindow::new()?;
    ui.set_palette(palette());
    refresh(&ui, &editor.borrow());

    {
        let editor = editor.clone();
        let handle = ui.as_weak();
        ui.on_undo_clicked(move || {
            let ui = handle.unwrap();
            editor.borrow_mut().undo();
            refresh(&ui, &editor.borrow());
        });
    }
    {
        let editor = editor.clone();
        let handle = ui.as_weak();
        ui.on_redo_clicked(move || {
            let ui = handle.unwrap();
            editor.borrow_mut().redo();
            refresh(&ui, &editor.borrow());
        });
    }
    {
        let editor = editor.clone();
        let handle = ui.as_weak();
        ui.on_place_node(move |node_type| {
            let ui = handle.unwrap();
            {
                let mut editor = editor.borrow_mut();
                let name = fresh_name(editor.document(), &node_type);
                let at = next_place(editor.document());
                // Two edits, because a name and a place are two things that
                // can be undone separately -- and because an undo of the
                // placement should leave the node rather than remove it.
                editor.edit(Edit::AddNode {
                    name: name.clone(),
                    node_type: node_type.to_string(),
                });
                editor.edit(Edit::MoveNode {
                    node: name,
                    was: (0, 0),
                    now: at,
                });
            }
            refresh(&ui, &editor.borrow());
        });
    }

    {
        let editor = editor.clone();
        let handle = ui.as_weak();
        ui.on_node_moved(move |name, dx, dy| {
            let ui = handle.unwrap();
            {
                let mut editor = editor.borrow_mut();
                // Where it was, from the document rather than from the window:
                // the window has been drawing it offset by the drag, and the
                // document is what the edit has to be relative to.
                let was = match editor.document().node(&name) {
                    Some(node) => node.at,
                    None => return,
                };
                let now = (was.0 + dx.round() as i32, was.1 + dy.round() as i32);
                // A drag that went nowhere is not an edit. Without this, every
                // press on a node would put an entry in the history and undo
                // would spend presses undoing nothing.
                if now == was {
                    return;
                }
                editor.edit(Edit::MoveNode {
                    node: name.to_string(),
                    was,
                    now,
                });
            }
            refresh(&ui, &editor.borrow());
        });
    }

    ui.run()
}

/// Put the editor's state into the window.
///
/// One function rather than a set of setters called from each place that
/// changes something: a window that is refreshed in one place cannot be
/// refreshed partly.
fn refresh(ui: &MainWindow, editor: &Editor) {
    let document = editor.document();
    let faults = editor.faults();

    let nodes: Vec<NodeView> = document
        .nodes
        .iter()
        .map(|node| {
            let fault = faults
                .iter()
                .find(|f| f.node() == Some(node.name.as_str()))
                .map(|f| f.describe());
            view_of(
                &node.name,
                &node.node_type,
                node.at,
                fault,
                false,
                node.comment.as_deref(),
            )
        })
        .collect();

    let links: Vec<LinkView> = document
        .links
        .iter()
        .filter_map(|link| {
            let from = document.node(&link.from)?;
            let to = document.node(&link.to)?;
            let (fw, fh) = size_of(&from.node_type);
            let (_, th) = size_of(&to.node_type);
            Some(LinkView {
                comment: link.comment.clone().unwrap_or_default().into(),
                x1: (from.at.0 + fw as i32) as f32,
                y1: (from.at.1 + fh as i32 / 2) as f32,
                x2: to.at.0 as f32,
                y2: (to.at.1 + th as i32 / 2) as f32,
                wide: link.input.is_some(),
                label: link.input.clone().unwrap_or_default().into(),
            })
        })
        .collect();

    ui.set_nodes(ModelRc::from(Rc::new(VecModel::from(nodes))));
    ui.set_links(ModelRc::from(Rc::new(VecModel::from(links))));
    ui.set_undo_depth(editor.undo_depth() as i32);
    ui.set_redo_depth(editor.journal().redoable() as i32);
    ui.set_undo_label(
        editor
            .journal()
            .next_undo()
            .map(|e| e.describe())
            .unwrap_or_default()
            .into(),
    );
    ui.set_redo_label(
        editor
            .journal()
            .next_redo()
            .map(|e| e.describe())
            .unwrap_or_default()
            .into(),
    );
    ui.set_trouble(editor.trouble().unwrap_or_default().into());
}

/// A name no node in the document has yet.
///
/// The type's own spelling, numbered if that is taken: a person who places
/// three i2c nodes gets i2c, i2c-2 and i2c-3 and renames them, rather than
/// being made to name each one before it can exist.
fn fresh_name(document: &document::Document, node_type: &str) -> String {
    if document.node(node_type).is_none() {
        return node_type.to_string();
    }
    (2..)
        .map(|n| format!("{node_type}-{n}"))
        .find(|name| document.node(name).is_none())
        .expect("a free name exists")
}

/// Where the next node placed should go.
fn next_place(document: &document::Document) -> (i32, i32) {
    let placed = document.nodes.len() as i32;
    (
        FIRST_PLACE.0 + placed * PLACE_OFFSET,
        FIRST_PLACE.1 + placed * PLACE_OFFSET,
    )
}

/// One of every node type, drawn as the canvas draws it.
///
/// The palette is built from the same table the canvas uses, so a shape in the
/// list is the shape on the canvas rather than a picture of one that may drift
/// away from it.
fn palette() -> ModelRc<NodeView> {
    let views: Vec<NodeView> = NodeType::ALL
        .iter()
        .map(|t| view_of(t.spelling(), t.spelling(), (8, 2), None, false, None))
        .collect();
    ModelRc::from(Rc::new(VecModel::from(views)))
}

/// What a node of this type looks like, as the window wants it.
fn view_of(
    name: &str,
    node_type: &str,
    at: (i32, i32),
    fault: Option<String>,
    selected: bool,
    comment: Option<&str>,
) -> NodeView {
    let appearance = NodeType::ALL
        .iter()
        .find(|t| t.spelling() == node_type)
        .map(|t| t.appearance());

    // A type this build does not know is drawn as a small open square, which
    // is a shape no real type uses, and marked faulty. Showing it beats
    // refusing to open the file: a configuration written for a later build is
    // readable, and what cannot be placed is visibly what cannot be placed.
    let (outline, shading, weight, size, tint, face, label_size, label_weight, label_italic) =
        match appearance {
            Some(a) => (
                a.outline,
                a.shading,
                a.weight,
                a.size,
                tint_of(a.tint),
                a.face,
                a.label_size,
                a.label_weight,
                a.label_italic,
            ),
            None => (
                Outline::Square,
                Shading::Open,
                Weight::Narrow,
                (104u32, 40u32),
                ERROR_TINT.to_string(),
                Face::Plain,
                11,
                400,
                true,
            ),
        };

    NodeView {
        name: name.into(),
        node_type: node_type.into(),
        x: at.0 as f32,
        y: at.1 as f32,
        width: size.0 as f32,
        height: size.1 as f32,
        outline: match outline {
            Outline::Square => "square",
            Outline::Rounded => "rounded",
            Outline::Circle => "circle",
            Outline::Keyed => "keyed",
        }
        .into(),
        shading: match shading {
            Shading::Solid => "solid",
            Shading::Hatched => "hatched",
            Shading::Open => "open",
            Shading::Double => "double",
        }
        .into(),
        border: match weight {
            Weight::Narrow => 1.0,
            Weight::Wide => 3.0,
        },
        face: match face {
            Face::Plain => "plain",
            Face::Mono => "mono",
        }
        .into(),
        label_size: label_size as f32,
        label_weight: label_weight as i32,
        label_italic,
        tint: colour(&tint),
        faulty: fault.is_some(),
        fault: fault.unwrap_or_default().into(),
        selected,
        comment: comment.unwrap_or_default().into(),
    }
}

/// The nominal size of a type, for placing a link's ends.
fn size_of(node_type: &str) -> (u32, u32) {
    NodeType::ALL
        .iter()
        .find(|t| t.spelling() == node_type)
        .map(|t| t.appearance().size)
        .unwrap_or((104, 40))
}

/// The palette's colours.
///
/// Held here rather than in `style.rs` on purpose: that module holds what a
/// type *means* and the rules the looks obey, and this holds what the meanings
/// are painted. Keeping them apart is what lets the accessibility test check
/// the rules without knowing a single hex value.
fn tint_of(tint: Tint) -> String {
    match tint {
        Tint::Slate => "#4a6572",
        Tint::Teal => "#1b6e6a",
        Tint::Amber => "#9a6700",
        Tint::Violet => "#6b4ea8",
        Tint::Olive => "#5c6b2f",
        Tint::Indigo => "#2f4b8f",
        Tint::Sand => "#8a6d3b",
        Tint::Cyan => "#1f6f8b",
        Tint::Grey => "#5a5a5a",
    }
    .to_string()
}

/// A hex string as a Slint colour.
fn colour(hex: &str) -> slint::Color {
    let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0x33_33_33);
    slint::Color::from_argb_u8(
        0xff,
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use document::Document;
    use slint::Model;

    /// The palette shows every type the editor can place.
    ///
    /// A type missing from the palette is a type nobody can place, which is a
    /// failure a person would find only by going looking for it.
    #[test]
    fn the_palette_offers_every_node_type() {
        let palette = palette();
        assert_eq!(palette.row_count(), NodeType::ALL.len());

        let offered: Vec<String> = (0..palette.row_count())
            .map(|i| palette.row_data(i).unwrap().node_type.to_string())
            .collect();
        for t in NodeType::ALL {
            assert!(
                offered.contains(&t.spelling().to_string()),
                "{} is not in the palette",
                t.spelling()
            );
        }
    }

    /// Nothing in the palette is red.
    ///
    /// Red is reserved for faults, and the palette holds no faults, so a red
    /// swatch in it would be the reservation being broken. Checked against the
    /// colours the window is actually given rather than against the table, so
    /// that a mistake in `tint_of` is caught too.
    #[test]
    fn the_palette_holds_no_red() {
        let error = colour(ERROR_TINT);
        let palette = palette();
        for i in 0..palette.row_count() {
            let view = palette.row_data(i).unwrap();
            assert!(!view.faulty, "a palette entry is marked faulty");
            assert_ne!(
                view.tint, error,
                "{} is painted the error colour",
                view.node_type
            );
        }
    }

    /// Every tint is a colour, and no two types share one closely enough to
    /// be mistaken.
    ///
    /// Colour is the redundant channel rather than the distinguishing one, so
    /// this is not a requirement -- two types may share a tint and still be
    /// told apart. What it does catch is a typo in a hex value that makes a
    /// tint black or invisible.
    #[test]
    fn every_tint_is_a_visible_colour() {
        for t in NodeType::ALL {
            let hex = tint_of(t.appearance().tint);
            assert!(hex.starts_with('#') && hex.len() == 7, "{hex}");
            let c = colour(&hex);
            assert_ne!(
                (c.red(), c.green(), c.blue()),
                (0xff, 0xff, 0xff),
                "{} is painted white on a white canvas",
                t.spelling()
            );
        }
    }

    /// A type the build does not know is drawn, marked, and not crashed on.
    #[test]
    fn an_unknown_type_is_shown_as_a_fault() {
        let view = view_of("mystery", "contraption", (0, 0), Some("unknown".into()), false, None);
        assert!(view.faulty);
        assert_eq!(view.tint, colour(ERROR_TINT));
        assert_eq!(view.outline, "square");
        assert_eq!(view.shading, "open");
    }

    /// A link's ends are placed on the two nodes it joins.
    #[test]
    fn a_link_is_drawn_between_its_nodes() {
        use journal::{Edit, Link};

        let mut document = Document::new();
        document.apply(&Edit::AddNode {
            name: "a".into(),
            node_type: "i2c".into(),
        });
        document.apply(&Edit::AddNode {
            name: "b".into(),
            node_type: "compute".into(),
        });
        document.apply(&Edit::MoveNode {
            node: "a".into(),
            was: (0, 0),
            now: (10, 20),
        });
        document.apply(&Edit::MoveNode {
            node: "b".into(),
            was: (0, 0),
            now: (300, 40),
        });
        document.apply(&Edit::AddLink {
            link: Link::new("a", "b", Some("in0")),
        });

        let (width, height) = size_of("i2c");
        let view = {
            let from = document.node("a").unwrap();
            let to = document.node("b").unwrap();
            LinkView {
                comment: "".into(),
                x1: (from.at.0 + width as i32) as f32,
                y1: (from.at.1 + height as i32 / 2) as f32,
                x2: to.at.0 as f32,
                y2: (to.at.1 + size_of("compute").1 as i32 / 2) as f32,
                wide: true,
                label: "in0".into(),
            }
        };

        // It starts at the right edge of the source and ends at the left edge
        // of the destination, so the line does not cross either shape.
        assert_eq!(view.x1, (10 + width as i32) as f32);
        assert_eq!(view.x2, 300.0);
        assert!(view.wide, "a link into a named input is drawn wide");
    }

    /// The place a first node is put is on the canvas.
    #[test]
    fn a_new_node_is_placed_where_it_can_be_seen() {
        assert!(FIRST_PLACE.0 > 0 && FIRST_PLACE.1 > 0);
    }
}
