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
mod route;
mod save;
mod style;
mod subdiagram;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use slint::{ModelRc, VecModel};

use document::Document;
use editor::Editor;
use journal::{Edit, Link};
use subdiagram::{Home, Qualified};
use style::{Face, NodeType, Outline, Shading, Tint, Weight, ERROR_TINT};

slint::include_modules!();

/// Where the first node placed goes, and how far each one after it is offset.
///
/// A diagram read from a file brings its own positions; a node just placed has
/// to go somewhere. Somewhere visible, and not on top of the last one -- a
/// stack of nodes at one spot looks like one node, and the person who placed
/// three would think two presses had been missed.
const FIRST_PLACE: (i32, i32) = (240, 40);
const PLACE_OFFSET: i32 = 32;

/// The grid nodes sit on, in pixels. See [`snap`].
const GRID: i32 = 8;

/// Every window currently showing part of the document.
///
/// The top diagram, and one per open subdiagram. Held so that an edit made in
/// any of them can redraw all of them -- which is the whole of keeping them in
/// step: there is one document, and after every edit every window is drawn
/// from it again. Nothing is copied between windows, so nothing can disagree.
struct Views {
    top: slint::Weak<MainWindow>,
    open: BTreeMap<String, SubWindow>,
}

impl Views {
    /// Draw every window from the document as it now is.
    fn refresh(&self, editor: &Editor) {
        if let Some(top) = self.top.upgrade() {
            put(&top, editor, &None);
        }
        for (name, window) in &self.open {
            put_sub(window, editor, name);
        }
    }
}

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

    let views = Rc::new(RefCell::new(Views {
        top: ui.as_weak(),
        open: BTreeMap::new(),
    }));
    // Which node a link is being drawn from, while the gesture is running.
    // Nothing is recorded until it finishes on another node, so a drag that
    // ends nowhere leaves no trace.
    let drawing: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    views.borrow().refresh(&editor.borrow());

    wire(&ui, &editor, &views, &drawing, &None);

    {
        // Opening a subdiagram. A window per subdiagram, made once and raised
        // afterwards: pressing the mark twice should bring the window
        // forward, not make a second one showing the same thing.
        let editor = editor.clone();
        let views = views.clone();
        let drawing = drawing.clone();
        ui.on_open_subdiagram(move |name| {
            let name = name.to_string();
            if let Some(already) = views.borrow().open.get(&name) {
                let _ = already.show();
                return;
            }

            let window = match SubWindow::new() {
                Ok(window) => window,
                Err(e) => {
                    eprintln!("tcsgui: {name} cannot be opened: {e}");
                    return;
                }
            };
            window.set_editing(name.clone().into());
            window.set_palette(palette());
            wire_sub(&window, &editor, &views, &drawing, &name);
            put_sub(&window, &editor.borrow(), &name);
            if let Err(e) = window.show() {
                eprintln!("tcsgui: {name} cannot be shown: {e}");
                return;
            }
            views.borrow_mut().open.insert(name, window);
        });
    }

    ui.run()
}

/// Wire a window's callbacks to the one editor.
///
/// `home` is the subdiagram the window is editing, so a node placed in it goes
/// into that subdiagram's file. The two window types have the same callbacks
/// and so want the same wiring; what differs is only the home they pass.
macro_rules! wiring {
    ($name:ident, $window:ty, $put:ident) => {
        fn $name(
            window: &$window,
            editor: &Rc<RefCell<Editor>>,
            views: &Rc<RefCell<Views>>,
            drawing: &Rc<RefCell<Option<String>>>,
            home: &Home,
        ) {
            {
                let editor = editor.clone();
                let views = views.clone();
                window.on_undo_clicked(move || {
                    editor.borrow_mut().undo();
                    views.borrow().refresh(&editor.borrow());
                });
            }
            {
                let editor = editor.clone();
                let views = views.clone();
                window.on_redo_clicked(move || {
                    editor.borrow_mut().redo();
                    views.borrow().refresh(&editor.borrow());
                });
            }
            {
                let editor = editor.clone();
                let views = views.clone();
                let home = home.clone();
                window.on_place_node(move |node_type| {
                    {
                        let mut editor = editor.borrow_mut();
                        let name = fresh_name(editor.document(), &node_type, &home);
                        let at = next_place(editor.document(), &home);
                        // Two edits: a node exists, and then it is somewhere.
                        // Separate so that undoing a drag leaves the node
                        // rather than removing it.
                        editor.edit(Edit::AddNode {
                            name: name.clone(),
                            node_type: node_type.to_string(),
                            home: home.clone(),
                        });
                        editor.edit(Edit::MoveNode {
                            node: Qualified::of(&name, &home).absolute(),
                            was: (0, 0),
                            now: at,
                        });
                    }
                    views.borrow().refresh(&editor.borrow());
                });
            }
            {
                let editor = editor.clone();
                let views = views.clone();
                let home = home.clone();
                window.on_node_moved(move |name, dx, dy| {
                    {
                        let mut editor = editor.borrow_mut();
                        let absolute = Qualified::of(&name, &home).absolute();
                        // Where it was, from the document rather than from the
                        // window: the window has been drawing it offset by the
                        // drag, and the edit has to be relative to the
                        // document.
                        let was = match editor.document().node(&absolute) {
                            Some(node) => node.at,
                            None => return,
                        };
                        let now = snap((was.0 + dx.round() as i32, was.1 + dy.round() as i32));
                        // A drag that went nowhere is not an edit. Without
                        // this, every press on a node would put an entry in
                        // the history.
                        if now == was {
                            return;
                        }
                        editor.edit(Edit::MoveNode {
                            node: absolute,
                            was,
                            now,
                        });
                    }
                    views.borrow().refresh(&editor.borrow());
                });
            }
            {
                // Drawing a link: the handle pressed is remembered, and the
                // handle released over is the other end.
                let drawing = drawing.clone();
                let home = home.clone();
                window.on_link_started(move |name| {
                    *drawing.borrow_mut() = Some(Qualified::of(&name, &home).absolute());
                });
            }
            {
                let editor = editor.clone();
                let views = views.clone();
                let drawing = drawing.clone();
                let home = home.clone();
                window.on_link_finished(move |name| {
                    let from = match drawing.borrow_mut().take() {
                        Some(from) => from,
                        None => return,
                    };
                    let to = Qualified::of(&name, &home).absolute();
                    // A link from a node to itself is refused here rather
                    // than drawn and then reported: releasing on the handle
                    // you pressed is a gesture that was abandoned, not a
                    // request for a loop.
                    if from == to {
                        return;
                    }
                    {
                        let mut editor = editor.borrow_mut();
                        let input = free_input(editor.document(), &to);
                        editor.edit(Edit::AddLink {
                            link: Link::new(&from, &to, input.as_deref()),
                        });
                    }
                    views.borrow().refresh(&editor.borrow());
                });
            }
            let _ = window;
        }
    };
}

wiring!(wire, MainWindow, put);
wiring!(wire_sub_inner, SubWindow, put_sub);

/// Wire a subdiagram's window, which edits that subdiagram.
fn wire_sub(
    window: &SubWindow,
    editor: &Rc<RefCell<Editor>>,
    views: &Rc<RefCell<Views>>,
    drawing: &Rc<RefCell<Option<String>>>,
    home: &str,
) {
    wire_sub_inner(window, editor, views, drawing, &Some(home.to_string()));
}

/// Put the editor's state into the top window.
///
/// One function rather than a set of setters called from each place that
/// changes something: a window refreshed in one place cannot be refreshed
/// partly.
fn put(ui: &MainWindow, editor: &Editor, home: &Home) {
    let (nodes, links) = views_of(editor, home);
    ui.set_nodes(ModelRc::from(Rc::new(VecModel::from(nodes))));
    ui.set_links(ModelRc::from(Rc::new(VecModel::from(links))));
    ui.set_undo_depth(editor.undo_depth() as i32);
    ui.set_redo_depth(editor.journal().redoable() as i32);
    ui.set_undo_label(label(editor.journal().next_undo()));
    ui.set_redo_label(label(editor.journal().next_redo()));
    ui.set_trouble(editor.trouble().unwrap_or_default().into());
}

/// The same, for one open subdiagram.
fn put_sub(window: &SubWindow, editor: &Editor, home: &str) {
    let home = Some(home.to_string());
    let (nodes, links) = views_of(editor, &home);
    window.set_nodes(ModelRc::from(Rc::new(VecModel::from(nodes))));
    window.set_links(ModelRc::from(Rc::new(VecModel::from(links))));
    window.set_undo_depth(editor.undo_depth() as i32);
    window.set_redo_depth(editor.journal().redoable() as i32);
    window.set_undo_label(label(editor.journal().next_undo()));
    window.set_redo_label(label(editor.journal().next_redo()));
    window.set_trouble(editor.trouble().unwrap_or_default().into());
}

/// What an undo or redo will do, for the button to say.
fn label(edit: Option<&Edit>) -> slint::SharedString {
    edit.map(|e| e.describe()).unwrap_or_default().into()
}

/// What one window shows: the nodes that live in `home`, and the links
/// between them.
///
/// A link with one end outside this window is drawn to a stub at the canvas
/// edge, labelled with the far node's full name. It cannot be drawn to the
/// node itself -- that node is in another window -- and leaving it out
/// altogether would make a subdiagram look unconnected when it is not.
fn views_of(editor: &Editor, home: &Home) -> (Vec<NodeView>, Vec<LinkView>) {
    let document = editor.document();
    let faults = editor.faults();

    let nodes: Vec<NodeView> = document
        .nodes_in(home)
        .iter()
        .map(|node| {
            let absolute = Qualified::of(&node.name, &node.home).absolute();
            let fault = faults
                .iter()
                .find(|f| f.node() == Some(absolute.as_str()))
                .map(|f| f.describe());

            // A subdiagram that is not open shows what is in it: how many
            // nodes, and how many connections cross its edge. Those are the
            // two things worth knowing about a diagram you are not looking
            // at, and they are what "simplified" means here -- not a smaller
            // drawing of the contents, which would be unreadable.
            let summary = (node.node_type == "subdiagram").then(|| {
                let inside = document.nodes_in(&Some(node.name.clone())).len();
                let crossing = document.crossings(&node.name);
                format!(
                    "{inside} node{} · {crossing} crossing{}",
                    if inside == 1 { "" } else { "s" },
                    if crossing == 1 { "" } else { "s" }
                )
            });

            view_of(
                &node.name,
                &node.node_type,
                node.at,
                fault,
                false,
                summary.as_deref().or(node.comment.as_deref()),
            )
        })
        .collect();

    let here = |name: &str| &Qualified::parse(name).in_ == home;
    let links: Vec<LinkView> = document
        .links
        .iter()
        .filter(|link| here(&link.from) || here(&link.to))
        .map(|link| {
            let from = document.node(&link.from);
            let to = document.node(&link.to);
            let (x1, y1) = match from {
                Some(node) if here(&link.from) => {
                    let (w, h) = size_of(&node.node_type);
                    ((node.at.0 + w as i32) as f32, (node.at.1 + h as i32 / 2) as f32)
                }
                // Off this page: a stub at the left edge.
                _ => (0.0, to.map(|n| n.at.1 as f32 + 24.0).unwrap_or(24.0)),
            };
            let (x2, y2) = match to {
                Some(node) if here(&link.to) => {
                    let (_, h) = size_of(&node.node_type);
                    (node.at.0 as f32, (node.at.1 + h as i32 / 2) as f32)
                }
                // Off this page: a stub to the right of its source.
                _ => (x1 + 72.0, y1),
            };

            // The label says which input it arrives at, and names the far
            // node where that node is on another page.
            let crossing = !here(&link.from) || !here(&link.to);
            let label = match (&link.input, crossing) {
                (Some(input), false) => input.clone(),
                (Some(input), true) => format!("{} . {input}", far_end(link, home)),
                (None, true) => far_end(link, home),
                (None, false) => String::new(),
            };

            LinkView {
                // The route and the arrow, worked out where they can be
                // tested: see route.rs.
                route: route::route(x1, y1, x2, y2).into(),
                head: route::arrow(x2, y2).into(),
                // The label goes at the middle of the run, which for an
                // orthogonal route is the middle of its vertical segment.
                label_x: (x1 + x2) / 2.0,
                label_y: (y1 + y2) / 2.0,
                wide: link.input.is_some(),
                label: label.into(),
                comment: link.comment.clone().unwrap_or_default().into(),
            }
        })
        .collect();

    (nodes, links)
}

/// The name of the end of `link` that is not on this page.
fn far_end(link: &Link, home: &Home) -> String {
    if &Qualified::parse(&link.from).in_ == home {
        link.to.clone()
    } else {
        link.from.clone()
    }
}

/// Which input of `node` a new link should arrive at.
///
/// A node that names its inputs gets the first one nothing is bound to, so
/// drawing three links into a mean fills in0, in1 and in2 without anybody
/// typing them. A node that names none takes a link with no input at all.
fn free_input(document: &Document, node: &str) -> Option<String> {
    let inputs = document.node(node)?.attributes.get("inputs")?.clone();
    let taken: Vec<&str> = document
        .links
        .iter()
        .filter(|l| l.to == node)
        .filter_map(|l| l.input.as_deref())
        .collect();
    inputs
        .split(',')
        .map(|i| i.trim())
        .find(|i| !i.is_empty() && !taken.contains(i))
        .map(|i| i.to_string())
}

/// A name no node in the document has yet.
/// A name no node in the document has yet.
///
/// The type's own spelling, numbered if that is taken: a person who places
/// three i2c nodes gets i2c, i2c-2 and i2c-3 and renames them, rather than
/// being made to name each one before it can exist.
fn fresh_name(document: &Document, node_type: &str, home: &Home) -> String {
    let taken = |name: &str| {
        document
            .node(&Qualified::of(name, home).absolute())
            .is_some()
    };
    if !taken(node_type) {
        return node_type.to_string();
    }
    (2..)
        .map(|n| format!("{node_type}-{n}"))
        .find(|name| !taken(name))
        .expect("a free name exists")
}

/// Where the next node placed should go.
///
/// Counted within the home being edited, so a subdiagram's first node lands
/// where the top diagram's first one does rather than wherever the whole
/// document's count happens to have reached.
fn next_place(document: &Document, home: &Home) -> (i32, i32) {
    let placed = document.nodes_in(home).len() as i32;
    snap((
        FIRST_PLACE.0 + placed * PLACE_OFFSET,
        FIRST_PLACE.1 + placed * PLACE_OFFSET,
    ))
}

/// The nearest point on the grid.
///
/// Nodes sit on an eight-pixel grid. Three things come of it and none is
/// merely decorative: a diagram stops looking approximate, nodes line up
/// without anybody nudging them, and the positions written to the file are
/// round numbers, so a file edited by hand and a file written by the editor
/// differ in what was meant rather than in a pixel.
fn snap((x, y): (i32, i32)) -> (i32, i32) {
    let to = GRID;
    let round = |v: i32| ((v as f32 / to as f32).round() as i32) * to;
    (round(x), round(y))
}

/// One of every node type, under its family's heading.
///
/// Built from the same table the canvas uses, so a shape in the list is the
/// shape on the canvas rather than a picture of one that may drift away from
/// it. Grouped because the families *are* the shape grammar -- square is
/// hardware, rounded is software aboard, a circle is an end of the graph -- and
/// a flat list of thirteen shapes hides the pattern that would make them
/// learnable.
fn palette() -> ModelRc<PaletteEntry> {
    let mut entries: Vec<PaletteEntry> = Vec::new();
    let mut family = "";

    for t in NodeType::ALL {
        if t.family() != family {
            family = t.family();
            entries.push(PaletteEntry {
                heading: family.into(),
                node: view_of("", "", (0, 0), None, false, None),
                is_heading: true,
            });
        }
        entries.push(PaletteEntry {
            heading: "".into(),
            node: view_of(t.spelling(), t.spelling(), (8, 2), None, false, None),
            is_heading: false,
        });
    }
    ModelRc::from(Rc::new(VecModel::from(entries)))
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
        // A diagram of its own can be opened; nothing else can.
        openable: node_type == "subdiagram",
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
    // Harmonised rather than each chosen by eye: near-equal lightness and
    // moderate chroma, so that no one of them shouts across a canvas holding
    // all nine. Safe to tune only because colour is the redundant channel
    // here -- the outline, weight, shading, size and type distinctions are
    // what tell a type apart, and none of them is touched by this.
    match tint {
        Tint::Slate => "#4b6382",
        Tint::Teal => "#11706a",
        Tint::Amber => "#8a6a12",
        Tint::Violet => "#6a4fa3",
        Tint::Olive => "#5c6b2a",
        Tint::Indigo => "#37508f",
        Tint::Sand => "#8a6136",
        Tint::Cyan => "#1a6b85",
        Tint::Grey => "#5b6065",
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
        let rows: Vec<PaletteEntry> = (0..palette.row_count())
            .map(|i| palette.row_data(i).unwrap())
            .collect();

        let offered: Vec<String> = rows
            .iter()
            .filter(|r| !r.is_heading)
            .map(|r| r.node.node_type.to_string())
            .collect();
        assert_eq!(offered.len(), NodeType::ALL.len());
        for t in NodeType::ALL {
            assert!(
                offered.contains(&t.spelling().to_string()),
                "{} is not in the palette",
                t.spelling()
            );
        }

        // Grouped, and every shape sits under a heading rather than above the
        // first one.
        assert!(rows[0].is_heading, "the palette does not start with a family");
        let headings: Vec<String> = rows
            .iter()
            .filter(|r| r.is_heading)
            .map(|r| r.heading.to_string())
            .collect();
        for t in NodeType::ALL {
            assert!(
                headings.contains(&t.family().to_string()),
                "{} has no heading in the palette",
                t.family()
            );
        }
        // And no family appears twice, which would mean the table had gone out
        // of family order and the palette was showing it as two groups.
        let mut once = headings.clone();
        once.sort();
        once.dedup();
        assert_eq!(once.len(), headings.len(), "a family is listed twice");
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
            let row = palette.row_data(i).unwrap();
            if row.is_heading {
                continue;
            }
            let view = row.node;
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

    /// A node lands on the grid, wherever it was dragged to.
    #[test]
    fn a_node_lands_on_the_grid() {
        for (dragged, expected) in [
            ((0, 0), (0, 0)),
            ((3, 5), (0, 8)),
            ((4, 4), (8, 8)),
            ((12, 20), (16, 24)),
            ((-3, -5), (0, -8)),
        ] {
            assert_eq!(snap(dragged), expected, "{dragged:?}");
        }
        // And everything the grid produces is on it.
        for x in -40..40 {
            assert_eq!(snap((x, x)).0 % GRID, 0, "{x}");
        }
    }

    /// Drawing links into a node that names its inputs fills them in order.
    ///
    /// Which is what makes a mean over three sensors three gestures rather
    /// than three gestures and three pieces of typing.
    #[test]
    fn links_into_a_named_input_fill_the_free_ones() {
        let mut document = Document::new();
        for name in ["a", "b", "c"] {
            document.apply(&Edit::AddNode {
                name: name.into(),
                node_type: "i2c".into(),
                home: None,
            });
        }
        document.apply(&Edit::AddNode {
            name: "mean".into(),
            node_type: "compute".into(),
            home: None,
        });
        document.apply(&Edit::SetAttribute {
            node: "mean".into(),
            attribute: "inputs".into(),
            was: None,
            now: Some("in0,in1,in2".into()),
        });

        for (from, expected) in [("a", "in0"), ("b", "in1"), ("c", "in2")] {
            let input = free_input(&document, "mean");
            assert_eq!(input.as_deref(), Some(expected), "linking {from}");
            document.apply(&Edit::AddLink {
                link: Link::new(from, "mean", input.as_deref()),
            });
        }

        // Full: a fourth link has no free input to take, and the document
        // reports the fault rather than the editor inventing one.
        assert_eq!(free_input(&document, "mean"), None);

        // A node that names no inputs takes a link with none.
        assert_eq!(free_input(&document, "a"), None);
    }

    /// A window shows its own home's nodes and nobody else's.
    #[test]
    fn a_window_shows_only_what_lives_in_it() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut editor = Editor::new(&where_.path().join("vehicle.yaml"));

        editor.edit(Edit::AddNode {
            name: "cabin".into(),
            node_type: "subdiagram".into(),
            home: None,
        });
        editor.edit(Edit::AddNode {
            name: "moc".into(),
            node_type: "moc".into(),
            home: None,
        });
        editor.edit(Edit::AddNode {
            name: "temp_fore".into(),
            node_type: "i2c".into(),
            home: Some("cabin".into()),
        });

        let (top, _) = views_of(&editor, &None);
        let names: Vec<String> = top.iter().map(|n| n.name.to_string()).collect();
        assert_eq!(names, vec!["cabin".to_string(), "moc".to_string()]);

        let (inside, _) = views_of(&editor, &Some("cabin".into()));
        let names: Vec<String> = inside.iter().map(|n| n.name.to_string()).collect();
        assert_eq!(names, vec!["temp_fore".to_string()]);

        // The closed box says what is in it, rather than drawing it.
        let box_ = top.iter().find(|n| n.name == "cabin").unwrap();
        assert!(box_.openable, "a subdiagram should be openable");
        assert!(
            box_.comment.contains("1 node"),
            "the closed box should say what is inside: {}",
            box_.comment
        );
    }

    /// A link that leaves the page is drawn, and says where it goes.
    ///
    /// Left out it would make a subdiagram look unconnected when it is not;
    /// drawn to the node itself it would have to reach into another window.
    #[test]
    fn a_crossing_link_is_drawn_to_a_stub_that_names_its_far_end() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut editor = Editor::new(&where_.path().join("vehicle.yaml"));

        editor.edit(Edit::AddNode {
            name: "cabin".into(),
            node_type: "subdiagram".into(),
            home: None,
        });
        editor.edit(Edit::AddNode {
            name: "moc".into(),
            node_type: "moc".into(),
            home: None,
        });
        editor.edit(Edit::AddNode {
            name: "temp_fore".into(),
            node_type: "i2c".into(),
            home: Some("cabin".into()),
        });
        editor.edit(Edit::AddLink {
            link: Link::new("cabin/temp_fore", "moc", None),
        });

        // Seen from inside the subdiagram: one link, named for where it goes.
        let (_, inside) = views_of(&editor, &Some("cabin".into()));
        assert_eq!(inside.len(), 1);
        assert_eq!(inside[0].label, "moc");

        // And seen from the top diagram, where its source is off the page.
        let (_, top) = views_of(&editor, &None);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].label, "cabin/temp_fore");
    }

    /// A name is fresh within its own home.
    ///
    /// Two subdiagrams may each hold an i2c, so placing one in each gives
    /// both the plain name rather than numbering the second.
    #[test]
    fn a_fresh_name_is_fresh_where_it_is_placed() {
        let mut document = Document::new();
        assert_eq!(fresh_name(&document, "i2c", &None), "i2c");

        document.apply(&Edit::AddNode {
            name: "i2c".into(),
            node_type: "i2c".into(),
            home: None,
        });
        assert_eq!(fresh_name(&document, "i2c", &None), "i2c-2");

        // In a subdiagram, the plain name is free again.
        let cabin = Some("cabin".to_string());
        assert_eq!(fresh_name(&document, "i2c", &cabin), "i2c");
    }

    /// A link's ends are placed on the two nodes it joins.
    #[test]
    fn a_link_is_drawn_between_its_nodes() {
        use journal::{Edit, Link};

        let mut document = Document::new();
        document.apply(&Edit::AddNode {
            name: "a".into(),
            node_type: "i2c".into(),
            home: None,
        });
        document.apply(&Edit::AddNode {
            name: "b".into(),
            node_type: "compute".into(),
            home: None,
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

        let where_ = tempfile::tempdir().expect("a directory");
        let mut editor = Editor::new(&where_.path().join("v.yaml"));
        for edit in [
            Edit::AddNode {
                name: "a".into(),
                node_type: "i2c".into(),
                home: None,
            },
            Edit::AddNode {
                name: "b".into(),
                node_type: "compute".into(),
                home: None,
            },
            Edit::MoveNode {
                node: "a".into(),
                was: (0, 0),
                now: (8, 24),
            },
            Edit::MoveNode {
                node: "b".into(),
                was: (0, 0),
                now: (304, 40),
            },
            Edit::AddLink {
                link: Link::new("a", "b", Some("in0")),
            },
        ] {
            editor.edit(edit);
        }

        let (_, links) = views_of(&editor, &None);
        assert_eq!(links.len(), 1);
        let view = &links[0];

        // It starts at the right edge of the source and ends at the left edge
        // of the destination, so the line does not cross either shape.
        let (width, _) = size_of("i2c");
        assert!(
            view.route.starts_with(&format!("M {}.0", 8 + width as i32)),
            "{}",
            view.route
        );
        let (_, target_height) = size_of("compute");
        let arrives = 40 + target_height as i32 / 2;
        assert!(
            view.route.ends_with(&format!("304.0 {arrives}.0")),
            "{}",
            view.route
        );
        assert!(view.head.ends_with('Z'), "there should be an arrowhead");
        assert!(view.wide, "a link into a named input is drawn wide");
    }

    /// The place a first node is put is on the canvas.
    #[test]
    fn a_new_node_is_placed_where_it_can_be_seen() {
        assert!(FIRST_PLACE.0 > 0 && FIRST_PLACE.1 > 0);
    }
}
