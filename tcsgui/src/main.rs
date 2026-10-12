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
//! * **Every edit is written as it is made**, which is `editor.rs`, and
//!   written in a way that cannot damage the file that was there, which is
//!   `save.rs`. Saving is therefore never something a person has to remember.
//!   There is a Save button all the same -- it writes every file now, which is
//!   what retries a write that failed -- and the way out of the program writes
//!   anything the files do not have.
//! * **Undo goes back to the start of the session**, and past a crash, because
//!   the history is written down beside the document; see `journal.rs`.
//! * **It is lazy.** Every value entered has to be valid and none of them has
//!   to be entered: an editor that insisted on a complete node before it would
//!   let you move on is one that fights a person working out what they are
//!   describing. Requiring that it all be entered belongs to `tcsverify` and
//!   the flight software, where the complaint can name the file and the line.
//!   See `document.rs`.
//!
//! A session may be given a second file, the simulator's own configuration.
//! Nothing of it is on the canvas -- it introduces no nodes and no connections
//! -- so it is edited from the menu; see `sim.rs`.

mod document;
mod editor;
mod read;
mod journal;
mod pane;
mod route;
mod save;
mod settings;
mod sim;
/// What the colours look like to a reader who cannot see all of them. Used by
/// the palette's tests and by nothing at run time.
#[cfg(test)]
mod sight;
mod style;
mod subdiagram;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use slint::{ModelRc, VecModel};

use document::Document;
use editor::Editor;
use pane::{confine, divider_for, is_payload, percent_for, size_of, GRID, WORKING_WIDTH};
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

/// How far the divider may be dragged, given what is already on the canvas.
///
/// Two rules, and the narrower of the two wins. Neither pane may be squeezed
/// below a twentieth of the working pane, which is what was asked for and is
/// in `settings.rs` as the percentages that are allowed. And the divider does
/// not pass a node: it stops against the right edge of the rightmost vehicle
/// node and against the left edge of the leftmost payload.
///
/// The second rule is what keeps the panes honest without moving anything. The
/// alternative was to let the divider go where it liked and shunt the nodes it
/// passed, which is a gesture that quietly rearranges a diagram -- and one
/// that would have to carry every node it moved through the history to be
/// undoable. A line that stops against a node says what it is doing while it
/// does it.
///
/// A node already on the wrong side of the divider holds nothing: it is a node
/// a file put there, it comes home when it is next dragged, and letting it
/// pin the divider would mean one such node froze the control.
fn divider_range(document: &Document, divider: i32) -> (i32, i32) {
    let mut low = WORKING_WIDTH * settings::LEAST / 100;
    let mut high = WORKING_WIDTH * settings::MOST / 100;

    for node in &document.nodes {
        let (width, _) = size_of(&node.node_type);
        let right = node.at.0 + width as i32;
        if is_payload(&node.node_type) {
            if node.at.0 >= divider {
                high = high.min(node.at.0);
            }
        } else if right <= divider {
            low = low.max(right);
        }
    }
    (low, high)
}

/// The groups menu's own name, which is what it reads as when it is closed.
///
/// A row of the menu rather than a label beside it, because a Slint ComboBox
/// shows whatever is selected: with no such row the menu would read as a
/// setting that says "every_second", and a person would reasonably think they
/// had put the diagram into that group.
const MENU_HEADING: &str = "Payload groups\u{2026}";

/// And the row that makes one.
const MENU_NEW: &str = "New group\u{2026}";

/// Where the ground station goes in a configuration that is new.
///
/// The upper left of the vehicle working pane, clear of the pane's own label.
const MOC_PLACE: (i32, i32) = (24, 40);

/// Every window currently showing part of the document.
///
/// The top diagram, and one per open subdiagram. Held so that an edit made in
/// any of them can redraw all of them -- which is the whole of keeping them in
/// step: there is one document, and after every edit every window is drawn
/// from it again. Nothing is copied between windows, so nothing can disagree.
struct Views {
    top: slint::Weak<MainWindow>,
    open: BTreeMap<String, SubWindow>,
    /// Where the divider between the two working panes sits, in the
    /// coordinates the file holds.
    ///
    /// One for every window, because it is one canvas divided one way: a
    /// subdiagram is a page of the same configuration, and a payload is on
    /// the same side of every page of it. Shared rather than copied so that
    /// moving it in one window moves it in all of them, which is the same
    /// arrangement the document itself has.
    ///
    /// Not in the document: it is a working preference and not part of the
    /// diagram. See `settings.rs`.
    divider: Rc<std::cell::Cell<i32>>,
    /// Which node each window's panel is showing, by the window's home.
    ///
    /// Per window, because two windows looking at two subdiagrams are looking
    /// at two different nodes and a single selection would make one of them
    /// follow the other.
    chosen: Rc<RefCell<BTreeMap<String, String>>>,
    /// The group window, if one is open.
    ///
    /// One window and not one per group, as subdiagrams have. A subdiagram is
    /// a page, and two of them are two drawings worth seeing at once; a group
    /// is a set of attributes, and a second window would be a second copy of
    /// the same form. Choosing another group re-points this one, which is also
    /// what makes the menu behave like a menu.
    group: Option<GroupWindow>,
    /// The nodes whose attributes are open, by the name a link would use.
    ///
    /// One window per node and several at once, which is what they are for: a
    /// panel showing whichever node was last pressed made comparing two
    /// sensors on one bus a matter of pressing, reading, pressing and
    /// remembering. Each stays until it is closed.
    attributes: BTreeMap<String, NodeWindow>,
    /// The simulator's groups window, if it is open, and which group it is
    /// showing.
    ///
    /// The groups and not the simulated payloads: a payload's settings belong
    /// to a payload that is already on the canvas, so they are shown in its
    /// panel under a rule. A window of its own would be a second place to look
    /// for what one node says about itself.
    sim_groups: Option<SimWindow>,
    showing_sim_group: Rc<RefCell<String>>,
    /// The tcspecial parameters window, if one is open.
    ///
    /// One, like the group window and for the same reason: it is a form about
    /// the configuration rather than a page of it, and a second copy would be
    /// two sets of fields writing to one set of values.
    parameters: Option<ParametersWindow>,
    /// Which group that window is showing.
    ///
    /// Shared with the window's own handlers, because a group can be renamed
    /// from inside it: a handler that had captured the name the window was
    /// made with would go on editing a group that no longer exists.
    showing: Rc<RefCell<String>>,
}

impl Views {
    /// Say which node is drawn as chosen, and nothing else.
    ///
    /// Apart from [`Views::refresh`] because choosing a node must not rebuild
    /// the canvas. Rebuilding it replaces every element on it, including the
    /// one the pointer is pressing -- and a press is how a node is chosen, so
    /// a full redraw there destroyed the element mid-gesture and the drag that
    /// had just begun went with it. A dragged node sat exactly where it was
    /// and nothing said why.
    ///
    /// Nothing on the canvas needs rebuilding for a selection anyway: a node
    /// is drawn as chosen by comparing its name with the window's `selected`,
    /// so setting that one property is the whole of it. What a node says about
    /// itself is in a window of its own -- see [`show_node`] -- which is not
    /// touched by a press.
    fn chose(&self, home: &Home) {
        let chosen = self.chosen.borrow();
        let said = |home: &str| {
            chosen.get(home).cloned().unwrap_or_default()
        };
        match home {
            None => {
                if let Some(top) = self.top.upgrade() {
                    top.set_selected(said("").into());
                }
            }
            Some(home) => {
                if let Some(window) = self.open.get(home) {
                    window.set_selected(said(home).into());
                }
            }
        }
    }

    /// Draw every window from the document as it now is.
    fn refresh(&self, editor: &Editor) {
        let chosen = self.chosen.borrow();
        let divider = self.divider.get();
        if let Some(window) = &self.group {
            put_group(window, editor, &self.showing.borrow());
        }
        if let Some(window) = &self.parameters {
            put_parameters(window, editor);
        }
        if let Some(window) = &self.sim_groups {
            put_sim_groups(window, editor, &self.showing_sim_group.borrow());
        }
        // Every node whose attributes are open. A node that has since been
        // removed shows itself as empty rather than vanishing: a window that
        // disappeared would leave somebody wondering what they had pressed.
        for (node, window) in &self.attributes {
            put_node(window, editor, node);
        }
        if let Some(top) = self.top.upgrade() {
            put(&top, editor, &None, chosen.get("").map(|s| s.as_str()), divider);
        }
        for (name, window) in &self.open {
            put_sub(
                window,
                editor,
                name,
                chosen.get(name).map(|s| s.as_str()),
                divider,
            );
        }
    }
}

/// Put into a window the things that never change while it is open.
///
/// The two selection panes, and how wide the working pane is. Apart from
/// [`put`] because nothing here depends on the document or on what anybody has
/// moved: a refresh after every edit would rebuild two models that cannot have
/// changed. Where the divider is does change, so that is in `put`.
macro_rules! furnish {
    ($window:expr) => {{
        let window = $window;
        window.set_vehicle_palette(palette_of(false));
        window.set_payload_palette(palette_of(true));
        window.set_working_width(WORKING_WIDTH as f32);
    }};
}

fn main() -> Result<(), slint::PlatformError> {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("tcsgui: usage: tcsgui <configuration file> [<simulator file>]");
        eprintln!(
            "tcsgui: the file need not exist yet; it is written as edits are made, \
             and its history is kept beside it"
        );
        eprintln!(
            "tcsgui: a second file is the simulator's own configuration -- how fast \
             a simulated payload sends and what it does wrong -- and is edited from \
             the menu"
        );
        std::process::exit(2);
    });
    // The simulator's configuration, where one was named. A second argument
    // and not a name derived from the first: the shipped sets call it after
    // the configuration with `sim` on the end, and nothing requires that.
    let simulator = std::env::args().nth(2);

    // How the canvas was divided when the editor was last used, which is a
    // preference and not part of the configuration: see settings.rs. Reading
    // it is also what makes the directory, so it exists from the first run
    // whether or not anything has been saved into it.
    //
    // Before the configuration, because where the divider is decides which
    // pane each node belongs in, and a node the file draws in the wrong one is
    // brought into its own as the file is read.
    let divider = Rc::new(std::cell::Cell::new(divider_for(
        settings::load().vehicle_percent,
    )));

    // The configuration: see Editor::open.
    let editor = Rc::new(RefCell::new(Editor::open(
        std::path::Path::new(&path),
        divider.get(),
    )));
    if let Some(simulator) = &simulator {
        editor
            .borrow_mut()
            .open_simulator(std::path::Path::new(simulator));
    }
    seed(&mut editor.borrow_mut());

    let ui = MainWindow::new()?;
    furnish!(&ui);

    let views = Rc::new(RefCell::new(Views {
        top: ui.as_weak(),
        open: BTreeMap::new(),
        chosen: Rc::new(RefCell::new(BTreeMap::new())),
        divider: divider.clone(),
        group: None,
        parameters: None,
        attributes: BTreeMap::new(),
        sim_groups: None,
        showing_sim_group: Rc::new(RefCell::new(String::new())),
        showing: Rc::new(RefCell::new(String::new())),
    }));
    // Which node the panel is showing, per window.
    //
    // Deliberately not in the document: what somebody is looking at is not
    // something the vehicle flies, and an edit for it would make choosing a
    // node undoable, which is not what undo is for.
    let chosen: Rc<RefCell<BTreeMap<String, String>>> = Rc::new(RefCell::new(BTreeMap::new()));

    // Which node a link is being drawn from, while the gesture is running.
    // Nothing is recorded until it finishes on another node, so a drag that
    // ends nowhere leaves no trace.
    let drawing: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    views.borrow().refresh(&editor.borrow());

    views.borrow_mut().chosen = chosen.clone();
    wire(&ui, &editor, &views, &drawing, &chosen, &None);

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
            furnish!(&window);
            wire_sub(&window, &editor, &views, &drawing, &chosen, &name);
            put_sub(
                &window,
                &editor.borrow(),
                &name,
                None,
                views.borrow().divider.get(),
            );
            if let Err(e) = window.show() {
                eprintln!("tcsgui: {name} cannot be shown: {e}");
                return;
            }
            views.borrow_mut().open.insert(name, window);
        });
    }

    let outcome = ui.run();

    // On the way out, anything the files do not have.
    //
    // Normally there is nothing: every edit is written as it is made, so the
    // configuration on disk is already the configuration in the window. What
    // this catches is the session where a write failed -- a full device, a
    // directory made read-only -- and was then put right without another edit
    // being made. Leaving a session's work in a process that is ending is the
    // one way this editor could lose anything, so it is worth one more try
    // and one plain sentence either way.
    if editor.borrow().unsaved() {
        editor.borrow_mut().save();
        match editor.borrow().trouble() {
            None => eprintln!("tcsgui: saved {path} on the way out"),
            Some(why) => eprintln!(
                "tcsgui: {path} could not be saved: {why}\n\
                 tcsgui: the work of this session is in {}",
                editor.borrow().journal_path().display()
            ),
        }
    }

    outcome
}

/// Wire a window's callbacks to the one editor.
///
/// `home` is the subdiagram the window is editing, so a node placed in it goes
/// into that subdiagram's file. The two window types have the same callbacks
/// and so want the same wiring; what differs is only the home they pass.
macro_rules! wiring {
    ($name:ident, $window:ty, $put:ident) => {
        #[allow(clippy::too_many_arguments)]
        fn $name(
            window: &$window,
            editor: &Rc<RefCell<Editor>>,
            views: &Rc<RefCell<Views>>,
            drawing: &Rc<RefCell<Option<String>>>,
            chosen: &Rc<RefCell<BTreeMap<String, String>>>,
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
                        let at = next_place(
                            editor.document(),
                            &home,
                            &node_type,
                            views.borrow().divider.get(),
                        );
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
                // A second handle, for the click that opens a window: the
                // borrow taken to read the document has to be dropped before
                // the window's own handlers can run.
                let editor_for_click = editor.clone();
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
                        let (was, node_type) = match editor.document().node(&absolute) {
                            Some(node) => (node.at, node.node_type.clone()),
                            None => return,
                        };
                        // Brought back inside its own pane: a payload dragged
                        // left stops at the divider rather than crossing it.
                        // The drag is clamped rather than refused, so pushing
                        // a node at the edge moves it along the edge instead
                        // of doing nothing.
                        let now = confine(
                            &node_type,
                            views.borrow().divider.get(),
                            snap((was.0 + dx.round() as i32, was.1 + dy.round() as i32)),
                        );
                        // A press that moved nothing is a click, and a click
                        // on a node is a request to see what it says. Told
                        // apart by the distance and not by the time: a gesture
                        // that means one thing or another depending on how
                        // long it took is a gesture that does the wrong one
                        // under pressure.
                        //
                        // It is also what keeps a drag out of the history:
                        // without this, every press on a node would put an
                        // entry in it.
                        if now == was {
                            drop(editor);
                            show_node(&editor_for_click, &views, &absolute);
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
                // Writing every file of the configuration now. Nothing to do
                // in the ordinary case -- every edit is written as it is made
                // -- and the way to retry a write that failed. See
                // Editor::save.
                let editor = editor.clone();
                let views = views.clone();
                window.on_save_clicked(move || {
                    editor.borrow_mut().save();
                    views.borrow().refresh(&editor.borrow());
                });
            }
            {
                // The menu item that opens what tcspecial is configured with.
                let editor = editor.clone();
                let views = views.clone();
                window.on_edit_parameters(move || {
                    show_parameters(&editor, &views);
                });
            }
            {
                // And the simulator's groups of settings. A simulated
                // payload's own settings are in its node's panel, under a
                // rule: see panel_of.
                let editor = editor.clone();
                let views = views.clone();
                window.on_edit_simulator_groups(move || {
                    show_sim_groups(&editor, &views);
                });
            }
            {
                // The groups menu. Its first row is its own name and does
                // nothing; its last makes a group; anything else is a group
                // to open.
                let editor = editor.clone();
                let views = views.clone();
                window.on_group_chosen(move |chosen| {
                    let chosen = chosen.to_string();
                    if chosen == MENU_HEADING {
                        return;
                    }

                    let group = if chosen == MENU_NEW {
                        let name = {
                            let mut editor = editor.borrow_mut();
                            let name = fresh_group(editor.document());
                            // With an interval, because a group without one
                            // is a group that reads nothing: a value to
                            // change is a better start than a blank to fill
                            // in.
                            editor.edit(Edit::AddGroup {
                                name: name.clone(),
                                interval: FIRST_INTERVAL.to_string(),
                            });
                            name
                        };
                        views.borrow().refresh(&editor.borrow());
                        name
                    } else {
                        chosen
                    };

                    show_group(&editor, &views, &group);
                });
            }
            {
                // Moving the divider between the two working panes.
                //
                // Told once, when the gesture finishes, as a node's drag is:
                // the line follows the pointer in the window and this is what
                // it settles on. Not an edit -- there is nothing to undo,
                // because the configuration has not changed -- so it is
                // written to the settings instead of to the history.
                let editor = editor.clone();
                let views = views.clone();
                window.on_divider_moved(move |dx: f32| {
                    let divider = views.borrow().divider.clone();
                    {
                        let editor = editor.borrow();
                        let (low, high) = divider_range(editor.document(), divider.get());
                        let wanted = snap((divider.get() + dx.round() as i32, 0)).0;
                        let now = wanted.clamp(low, high);
                        if now == divider.get() {
                            // It did not move, or it was already against what
                            // stops it. Nothing to redraw and nothing to
                            // write: a settings file rewritten on every press
                            // of the handle is a file written for nothing.
                            return;
                        }
                        divider.set(now);
                    }
                    settings::save(settings::Settings {
                        vehicle_percent: percent_for(divider.get()),
                    });
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
            {
                // Choosing a node: the panel follows, and nothing is recorded.
                let views = views.clone();
                let editor = editor.clone();
                let chosen = chosen.clone();
                let home = home.clone();
                window.on_node_selected(move |name| {
                    chosen.borrow_mut().insert(
                        home.clone().unwrap_or_default(),
                        name.to_string(),
                    );
                    // The outline only: choosing a node is the same press
                    // that starts dragging it, and rebuilding the canvas here
                    // would replace the element the pointer is holding. See
                    // Views::chose.
                    views.borrow().chose(&home);
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
    chosen: &Rc<RefCell<BTreeMap<String, String>>>,
    home: &str,
) {
    wire_sub_inner(
        window,
        editor,
        views,
        drawing,
        chosen,
        &Some(home.to_string()),
    );
}

/// Put the editor's state into the top window.
///
/// One function rather than a set of setters called from each place that
/// changes something: a window refreshed in one place cannot be refreshed
/// partly.
fn put(
    ui: &MainWindow,
    editor: &Editor,
    home: &Home,
    chosen: Option<&str>,
    divider: i32,
) {
    ui.set_divider(divider as f32);
    ui.set_has_simulator(editor.document().simulator.is_some());
    ui.set_group_menu(group_menu(editor.document()));
    let (nodes, links) = views_of(editor, home);
    ui.set_selected(chosen.unwrap_or_default().into());
    ui.set_nodes(ModelRc::from(Rc::new(VecModel::from(nodes))));
    ui.set_links(ModelRc::from(Rc::new(VecModel::from(links))));
    ui.set_undo_depth(editor.undo_depth() as i32);
    ui.set_redo_depth(editor.journal().redoable() as i32);
    ui.set_undo_label(label(editor.journal().next_undo()));
    ui.set_redo_label(label(editor.journal().next_redo()));
    ui.set_trouble(editor.trouble().unwrap_or_default().into());
}

/// The same, for one open subdiagram.
fn put_sub(
    window: &SubWindow,
    editor: &Editor,
    home: &str,
    chosen: Option<&str>,
    divider: i32,
) {
    window.set_divider(divider as f32);
    window.set_has_simulator(editor.document().simulator.is_some());
    window.set_group_menu(group_menu(editor.document()));
    let home = Some(home.to_string());
    let (nodes, links) = views_of(editor, &home);
    window.set_selected(chosen.unwrap_or_default().into());
    window.set_nodes(ModelRc::from(Rc::new(VecModel::from(nodes))));
    window.set_links(ModelRc::from(Rc::new(VecModel::from(links))));
    window.set_undo_depth(editor.undo_depth() as i32);
    window.set_redo_depth(editor.journal().redoable() as i32);
    window.set_undo_label(label(editor.journal().next_undo()));
    window.set_redo_label(label(editor.journal().next_redo()));
    window.set_trouble(editor.trouble().unwrap_or_default().into());
}

/// Put what a node says about itself into its own window.
///
/// Everything its type declares, whatever else it holds, and -- below a rule
/// -- the settings the simulator file gives it. See [`panel_of`], which builds
/// the rows; this is what hands them to a window.
fn put_node(window: &NodeWindow, editor: &Editor, node: &str) {
    let (name, node_type, comment, fields) =
        panel_of(editor, &Qualified::parse(node).in_, Some(&Qualified::parse(node).node));

    // What is wrong with what it says, if anything: the same red the canvas
    // draws, where somebody typing is looking.
    let fault = editor
        .faults()
        .iter()
        .find(|fault| fault.node() == Some(node))
        .map(|fault| fault.describe())
        .unwrap_or_default();

    window.set_node(name.into());
    window.set_node_type(node_type.into());
    window.set_comment(comment.into());
    window.set_fields(ModelRc::from(Rc::new(VecModel::from(fields))));
    window.set_fault(fault.into());
    window.set_trouble(editor.trouble().unwrap_or_default().into());
}

/// Show what a node says about itself, making the window if there is not one.
///
/// Made once per node and raised afterwards: pressing a node twice should
/// bring its window forward, not make a second one saying the same thing.
fn show_node(editor: &Rc<RefCell<Editor>>, views: &Rc<RefCell<Views>>, node: &str) {
    if !views.borrow().attributes.contains_key(node) {
        let window = match NodeWindow::new() {
            Ok(window) => window,
            Err(e) => {
                eprintln!("tcsgui: {node} cannot be opened: {e}");
                return;
            }
        };
        wire_node(&window, editor, views, node);
        views
            .borrow_mut()
            .attributes
            .insert(node.to_string(), window);
    }

    // Through a weak handle, so nothing of `views` is borrowed while the
    // window is shown: showing one runs the toolkit, and the toolkit runs
    // handlers that borrow it.
    let window = views
        .borrow()
        .attributes
        .get(node)
        .map(|window| window.as_weak());
    if let Some(window) = window.and_then(|window| window.upgrade()) {
        put_node(&window, &editor.borrow(), node);
        if let Err(e) = window.show() {
            eprintln!("tcsgui: {node} cannot be shown: {e}");
        }
    }
}

/// Wire one node's window to the one editor.
///
/// `node` is the name a link would use, captured once: a window belongs to the
/// node it was opened for, and goes on belonging to it however many others are
/// opened afterwards.
fn wire_node(
    window: &NodeWindow,
    editor: &Rc<RefCell<Editor>>,
    views: &Rc<RefCell<Views>>,
    node: &str,
) {
    {
        // Typing into a field. The old value comes from the document, so the
        // edit carries what it replaced and undo is a lookup.
        let editor = editor.clone();
        let views = views.clone();
        let node = node.to_string();
        window.on_attribute_set(move |attribute, value| {
            {
                let mut editor = editor.borrow_mut();
                let was = editor
                    .document()
                    .node(&node)
                    .and_then(|n| n.attributes.get(attribute.as_str()))
                    .cloned();
                // An empty field clears the attribute rather than setting it
                // to nothing: absent and empty are different things to a
                // configuration.
                let now = (!value.trim().is_empty()).then(|| value.to_string());
                if was == now {
                    return;
                }
                editor.edit(Edit::SetAttribute {
                    node: node.clone(),
                    attribute: attribute.to_string(),
                    was,
                    now,
                });
            }
            views.borrow().refresh(&editor.borrow());
        });
    }
    {
        // And into one of the simulator's settings for it, which is a
        // different file and so a different edit.
        let editor = editor.clone();
        let views = views.clone();
        let node = node.to_string();
        window.on_sim_attribute_set(move |setting, value| {
            {
                let mut editor = editor.borrow_mut();
                let setting = setting.to_string();
                let was = editor
                    .document()
                    .simulator
                    .as_ref()
                    .and_then(|simulator| simulator.payloads.get(&node))
                    .and_then(|said| said.get(&setting).cloned());
                let now = (!value.trim().is_empty()).then(|| value.to_string());
                if was == now {
                    return;
                }
                editor.edit(Edit::SetSimPayload {
                    payload: node.clone(),
                    setting,
                    was,
                    now,
                });
            }
            views.borrow().refresh(&editor.borrow());
        });
    }
    {
        let editor = editor.clone();
        let views = views.clone();
        let node = node.to_string();
        window.on_comment_set(move |text| {
            {
                let mut editor = editor.borrow_mut();
                let was = editor
                    .document()
                    .node(&node)
                    .and_then(|n| n.comment.clone());
                let now = (!text.trim().is_empty()).then(|| text.to_string());
                if was == now {
                    return;
                }
                editor.edit(Edit::SetNodeComment {
                    node: node.clone(),
                    was,
                    now,
                });
            }
            views.borrow().refresh(&editor.borrow());
        });
    }
    {
        // Closing takes a press of its own, which is the point of a window
        // per node: one that closed itself when another node was pressed
        // would be the panel this replaced.
        let views = views.clone();
        let node = node.to_string();
        window.on_closed(move || {
            let window = views.borrow_mut().attributes.remove(&node);
            if let Some(window) = window {
                let _ = window.hide();
            }
        });
    }
}

/// Put a group's state into its window.
///
/// What a group is: a name, how often it is read, and the payloads that are in
/// it. The fields below those are the attributes every one of those payloads
/// declares -- see [`Document::group_fields`] -- which are the ones a value
/// set here can mean anything on.
fn put_group(window: &GroupWindow, editor: &Editor, group: &str) {
    let document = editor.document();

    let members: Vec<slint::SharedString> = document
        .payloads_of(group)
        .iter()
        .map(|node| Qualified::of(&node.name, &node.home).absolute().into())
        .collect();
    let said: Vec<String> = members.iter().map(|m| m.to_string()).collect();

    let fields: Vec<GroupFieldView> = document
        .group_fields(group)
        .into_iter()
        .map(|field| GroupFieldView {
            name: field.attribute.into(),
            value: field.value.unwrap_or_default().into(),
            differs: field.differs,
        })
        .collect();

    window.set_group(group.into());
    window.set_interval(
        document
            .groups
            .get(group)
            .cloned()
            .unwrap_or_default()
            .into(),
    );
    window.set_members_text(said.join(", ").into());
    window.set_members(ModelRc::from(Rc::new(VecModel::from(members))));
    window.set_fields(ModelRc::from(Rc::new(VecModel::from(fields))));
    // The same red the main window shows, because somebody typing in here
    // should not have to look at the other one to find out that nothing is
    // reaching the disk.
    window.set_trouble(editor.trouble().unwrap_or_default().into());
}

/// Put the tcspecial parameters into their window.
///
/// Every attribute the section may have, in the order `tcslibgs` lists them --
/// the ones that must be stated first -- with the empty required ones marked.
/// Built from that list rather than from what the document happens to hold, so
/// that a section somebody has never filled in shows nine fields to fill
/// rather than nothing at all.
fn put_parameters(window: &ParametersWindow, editor: &Editor) {
    let said = &editor.document().tcspecial;
    let fields: Vec<FieldView> = tcslibgs::graph::parameters::ATTRIBUTES
        .iter()
        .map(|(attribute, required)| {
            let value = said.get(*attribute).cloned().unwrap_or_default();
            FieldView {
                name: (*attribute).into(),
                value: value.clone().into(),
                required: *required,
                is_heading: false,
                simulated: false,
            }
        })
        .collect();

    window.set_fields(ModelRc::from(Rc::new(VecModel::from(fields))));
    window.set_trouble(editor.trouble().unwrap_or_default().into());
    // What is wrong with what has been entered, which belongs to no node and
    // so has nowhere else to be shown.
    window.set_complaint(
        editor
            .faults()
            .iter()
            .find_map(|fault| match fault {
                document::Fault::BadTcspecial { why } => Some(why.clone()),
                _ => None,
            })
            .unwrap_or_default()
            .into(),
    );
}

/// Put one of the simulator's groups of settings into its window.
fn put_sim_groups(window: &SimWindow, editor: &Editor, group: &str) {
    let document = editor.document();
    let choices: Vec<slint::SharedString> = document
        .simulator
        .as_ref()
        .map(|simulator| simulator.groups.keys().map(|name| name.clone().into()).collect())
        .unwrap_or_default();

    let empty = BTreeMap::new();
    let said = document
        .simulator
        .as_ref()
        .and_then(|simulator| simulator.groups.get(group))
        .unwrap_or(&empty);

    window.set_editing("Simulator payload groups".into());
    window.set_about(
        "Settings several simulated payloads share. A payload names a group \
         and overrides whatever it wants for itself, so what is here is what \
         they have in common -- a rate, a segmentation, a fault to inject. A \
         payload's own settings are in its panel on the canvas."
            .into(),
    );
    window.set_can_add(true);
    window.set_choices(ModelRc::from(Rc::new(VecModel::from(choices))));
    window.set_chosen(group.into());
    window.set_fields(settings_shown(&crate::sim::Simulator::fields(false), said));
    window.set_trouble(editor.trouble().unwrap_or_default().into());
}

/// The fields a simulator window shows: every setting there is, filled in or
/// not.
///
/// Built from the list of settings rather than from what the file happens to
/// hold, so that an entry nobody has touched shows what it could be asked
/// rather than nothing at all. Nothing is marked: every one of these is
/// optional -- a payload that says nothing takes everything from its group --
/// so an empty field is a question not yet answered and not a fault.
fn settings_shown(
    fields: &[&'static str],
    said: &BTreeMap<String, String>,
) -> ModelRc<FieldView> {
    ModelRc::from(Rc::new(VecModel::from(settings_of(fields, said))))
}

/// The same, as rows rather than as a model, for the panel to append to a
/// node's own.
fn settings_of(
    fields: &[&'static str],
    said: &BTreeMap<String, String>,
) -> Vec<FieldView> {
    let mut shown: Vec<FieldView> = fields
        .iter()
        .map(|setting| FieldView {
            name: (*setting).into(),
            value: said.get(*setting).cloned().unwrap_or_default().into(),
            required: false,
            is_heading: false,
            simulated: true,
        })
        .collect();

    // And anything the file holds that this build does not know about, after
    // them: a setting nobody can see is one that gets written back out
    // without anybody knowing it is there.
    for (setting, value) in said {
        if fields.contains(&setting.as_str()) {
            continue;
        }
        shown.push(FieldView {
            name: setting.clone().into(),
            value: value.clone().into(),
            required: false,
            is_heading: false,
            simulated: true,
        });
    }
    shown
}

/// Show the tcspecial parameters, making the window if there is not one.
fn show_parameters(editor: &Rc<RefCell<Editor>>, views: &Rc<RefCell<Views>>) {
    if views.borrow().parameters.is_none() {
        let window = match ParametersWindow::new() {
            Ok(window) => window,
            Err(e) => {
                eprintln!("tcsgui: the tcspecial parameters cannot be opened: {e}");
                return;
            }
        };
        {
            let editor = editor.clone();
            let views = views.clone();
            window.on_parameter_set(move |attribute, value| {
                {
                    let mut editor = editor.borrow_mut();
                    let attribute = attribute.to_string();
                    let was = editor.document().tcspecial.get(&attribute).cloned();
                    // An empty field clears the parameter rather than setting
                    // it to nothing: the line comes out of the file, which is
                    // what somebody would have written.
                    let now = (!value.trim().is_empty()).then(|| value.to_string());
                    if was == now {
                        return;
                    }
                    editor.edit(Edit::SetTcspecial {
                        attribute,
                        was,
                        now,
                    });
                }
                views.borrow().refresh(&editor.borrow());
            });
        }
        views.borrow_mut().parameters = Some(window);
    }

    // Through a weak handle, so nothing of `views` is borrowed while the
    // window is shown.
    let window = views
        .borrow()
        .parameters
        .as_ref()
        .map(|window| window.as_weak());
    if let Some(window) = window.and_then(|window| window.upgrade()) {
        put_parameters(&window, &editor.borrow());
        if let Err(e) = window.show() {
            eprintln!("tcsgui: the tcspecial parameters cannot be shown: {e}");
        }
    }
}

/// Show the simulator's groups of settings, making the window if there is not
/// one.
fn show_sim_groups(editor: &Rc<RefCell<Editor>>, views: &Rc<RefCell<Views>>) {
    if views.borrow().sim_groups.is_none() {
        let window = match SimWindow::new() {
            Ok(window) => window,
            Err(e) => {
                eprintln!("tcsgui: a simulator window cannot be opened: {e}");
                return;
            }
        };
        wire_sim_groups(&window, editor, views);
        views.borrow_mut().sim_groups = Some(window);

        // Something chosen to begin with, so the window opens on a form
        // rather than on an invitation to choose.
        let first = editor
            .borrow()
            .document()
            .simulator
            .as_ref()
            .and_then(|simulator| simulator.groups.keys().next().cloned());
        if let Some(first) = first {
            *views.borrow().showing_sim_group.borrow_mut() = first;
        }
    }

    // Through a weak handle, so nothing of `views` is borrowed while the
    // window is shown.
    let window = views
        .borrow()
        .sim_groups
        .as_ref()
        .map(|window| window.as_weak());
    if let Some(window) = window.and_then(|window| window.upgrade()) {
        {
            let editor = editor.borrow();
            let views = views.borrow();
            put_sim_groups(&window, &editor, &views.showing_sim_group.borrow());
        }
        if let Err(e) = window.show() {
            eprintln!("tcsgui: a simulator window cannot be shown: {e}");
        }
    }
}

/// Wire the groups window's four callbacks to the one editor.
fn wire_sim_groups(
    window: &SimWindow,
    editor: &Rc<RefCell<Editor>>,
    views: &Rc<RefCell<Views>>,
) {
    let showing = views.borrow().showing_sim_group.clone();

    {
        // Choosing which group the window is showing. Nothing is recorded:
        // what somebody is looking at is not something the vehicle flies.
        let editor = editor.clone();
        let views = views.clone();
        let showing = showing.clone();
        window.on_choose(move |chosen| {
            *showing.borrow_mut() = chosen.to_string();
            views.borrow().refresh(&editor.borrow());
        });
    }
    {
        // A new group, which is this file's to make -- unlike a simulated
        // payload, whose name is the configuration's.
        let editor = editor.clone();
        let views = views.clone();
        let showing = showing.clone();
        window.on_add(move || {
            let name = {
                let mut editor = editor.borrow_mut();
                let taken = |name: &str| {
                    editor
                        .document()
                        .simulator
                        .as_ref()
                        .is_some_and(|simulator| simulator.groups.contains_key(name))
                };
                let name = if !taken("group") {
                    "group".to_string()
                } else {
                    (2..)
                        .map(|n| format!("group-{n}"))
                        .find(|name| !taken(name))
                        .expect("a free name exists")
                };
                editor.edit(Edit::AddSimGroup { name: name.clone() });
                name
            };
            *showing.borrow_mut() = name;
            views.borrow().refresh(&editor.borrow());
        });
    }
    {
        let editor = editor.clone();
        let views = views.clone();
        let showing = showing.clone();
        window.on_remove(move || {
            let name = showing.borrow().clone();
            {
                let mut editor = editor.borrow_mut();
                let settings: Vec<(String, String)> = match editor
                    .document()
                    .simulator
                    .as_ref()
                    .and_then(|simulator| simulator.groups.get(&name))
                {
                    // Carried whole, so that undoing the removal puts back
                    // the group that was there and not an empty one of the
                    // same name.
                    Some(said) => said.iter().map(|(a, b)| (a.clone(), b.clone())).collect(),
                    None => return,
                };
                editor.edit(Edit::RemoveSimGroup { name, settings });
            }
            // Nothing chosen, rather than a group that is gone.
            showing.borrow_mut().clear();
            views.borrow().refresh(&editor.borrow());
        });
    }
    {
        let editor = editor.clone();
        let views = views.clone();
        window.on_setting_set(move |setting, value| {
            let group = showing.borrow().clone();
            if group.is_empty() {
                return;
            }
            {
                let mut editor = editor.borrow_mut();
                let setting = setting.to_string();
                let was = editor
                    .document()
                    .simulator
                    .as_ref()
                    .and_then(|simulator| simulator.groups.get(&group))
                    .and_then(|said| said.get(&setting).cloned());
                // An empty field clears the setting rather than setting it to
                // nothing: absent is what the simulator's own reader means by
                // "not stated".
                let now = (!value.trim().is_empty()).then(|| value.to_string());
                if was == now {
                    return;
                }
                editor.edit(Edit::SetSimGroup {
                    group,
                    setting,
                    was,
                    now,
                });
            }
            views.borrow().refresh(&editor.borrow());
        });
    }
}

/// What the groups menu offers.
///
/// Its own name first, which does nothing and is what the menu reads as when
/// it is closed; then every group the configuration declares; then the way to
/// make one. A menu of nothing at all would be a control with no way in, which
/// is what a configuration that declares no groups yet would have.
fn group_menu(document: &Document) -> ModelRc<slint::SharedString> {
    let mut rows: Vec<slint::SharedString> = vec![MENU_HEADING.into()];
    rows.extend(document.groups.keys().map(|name| name.clone().into()));
    rows.push(MENU_NEW.into());
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

/// Show `group` in the group window, making the window if there is not one.
///
/// Made once and re-pointed afterwards, so choosing a second group does not
/// leave a window per group behind. The handlers are wired when the window is
/// made and read the group out of `Views::showing` as they run, which is what
/// lets the same handlers go on working after the window is re-pointed -- or
/// after the group it is showing is renamed from inside it.
fn show_group(editor: &Rc<RefCell<Editor>>, views: &Rc<RefCell<Views>>, group: &str) {
    *views.borrow().showing.borrow_mut() = group.to_string();

    if views.borrow().group.is_none() {
        let window = match GroupWindow::new() {
            Ok(window) => window,
            Err(e) => {
                eprintln!("tcsgui: a group window cannot be opened: {e}");
                return;
            }
        };
        wire_group(&window, editor, views);
        views.borrow_mut().group = Some(window);
    }

    // Through a weak handle, so nothing of `views` is borrowed while the
    // window is shown: showing one runs the toolkit, and the toolkit runs
    // handlers that borrow it.
    let window = views.borrow().group.as_ref().map(|window| window.as_weak());
    if let Some(window) = window.and_then(|window| window.upgrade()) {
        put_group(&window, &editor.borrow(), group);
        if let Err(e) = window.show() {
            eprintln!("tcsgui: a group window cannot be shown: {e}");
        }
    }
}

/// Wire the group window's four callbacks to the one editor.
///
/// Every one of them is an ordinary edit: recorded in the history, written as
/// it is made, and taken back by Undo in the main window. The last is the one
/// worth reading twice -- it sets one value on every payload of the group as a
/// single edit, so that undo takes back the gesture rather than the first of a
/// dozen consequences.
fn wire_group(window: &GroupWindow, editor: &Rc<RefCell<Editor>>, views: &Rc<RefCell<Views>>) {
    {
        let editor = editor.clone();
        let views = views.clone();
        let showing = views.borrow().showing.clone();
        window.on_renamed(move |now| {
            let was = showing.borrow().clone();
            let now = now.trim().to_string();

            // A group with no name is a group nothing can be in, and a name
            // another group already has would make membership ambiguous.
            // Refused quietly rather than reported: the refresh below puts
            // the field back to the name the group has, which is an answer a
            // person can see without reading anything.
            let allowed = {
                let editor = editor.borrow();
                let groups = &editor.document().groups;
                !now.is_empty()
                    && now != was
                    && groups.contains_key(&was)
                    && !groups.contains_key(&now)
            };
            if allowed {
                editor.borrow_mut().edit(Edit::RenameGroup {
                    was,
                    now: now.clone(),
                });
                *showing.borrow_mut() = now;
            }
            views.borrow().refresh(&editor.borrow());
        });
    }
    {
        let editor = editor.clone();
        let views = views.clone();
        let showing = views.borrow().showing.clone();
        window.on_interval_set(move |value| {
            let group = showing.borrow().clone();
            {
                let mut editor = editor.borrow_mut();
                let was = match editor.document().groups.get(&group) {
                    Some(was) => was.clone(),
                    None => return,
                };
                let now = value.trim().to_string();
                if now == was {
                    return;
                }
                editor.edit(Edit::SetGroupInterval {
                    group,
                    was,
                    now,
                });
            }
            views.borrow().refresh(&editor.borrow());
        });
    }
    {
        let editor = editor.clone();
        let views = views.clone();
        let showing = views.borrow().showing.clone();
        window.on_attribute_set(move |attribute, value| {
            let group = showing.borrow().clone();
            {
                let mut editor = editor.borrow_mut();
                let attribute = attribute.to_string();
                // What each payload has now, by name, so that undo puts back
                // exactly what was there -- including the ones that had
                // nothing, which get the attribute taken off again rather
                // than emptied.
                let was: Vec<(String, Option<String>)> = editor
                    .document()
                    .payloads_of(&group)
                    .iter()
                    .map(|node| {
                        (
                            Qualified::of(&node.name, &node.home).absolute(),
                            node.attributes.get(&attribute).cloned(),
                        )
                    })
                    .collect();
                if was.is_empty() {
                    return;
                }
                let now = (!value.trim().is_empty()).then(|| value.to_string());
                // Nothing to do where they all have it already.
                if was.iter().all(|(_, had)| *had == now) {
                    return;
                }
                editor.edit(Edit::SetGroupAttribute {
                    group,
                    attribute,
                    was,
                    now,
                });
            }
            views.borrow().refresh(&editor.borrow());
        });
    }
    {
        let editor = editor.clone();
        let views = views.clone();
        let showing = views.borrow().showing.clone();
        window.on_removed(move || {
            let group = showing.borrow().clone();
            {
                let mut editor = editor.borrow_mut();
                let Some(interval) = editor.document().groups.get(&group).cloned() else {
                    return;
                };
                editor.edit(Edit::RemoveGroup {
                    name: group,
                    interval,
                });
            }
            // The window goes with it. The payloads that named it are left
            // saying so, which the canvas marks: taking the group off a dozen
            // nodes is a different edit from removing the group, and not one
            // somebody asked for.
            if let Some(window) = &views.borrow().group {
                let _ = window.hide();
            }
            views.borrow().refresh(&editor.borrow());
        });
    }
}

/// What the panel shows: the chosen node, its type, its comment, its fields.
///
/// The fields are the ones the type declares rather than the ones the node
/// happens to hold, so a node just placed asks for what it needs instead of
/// showing nothing -- and an attribute the configuration does not know about,
/// which a file written by a later build may carry, is shown after them rather
/// than hidden.
fn panel_of(
    editor: &Editor,
    home: &Home,
    chosen: Option<&str>,
) -> (String, String, String, Vec<FieldView>) {
    let empty = (String::new(), String::new(), String::new(), Vec::new());
    let chosen = match chosen {
        Some(chosen) if !chosen.is_empty() => chosen,
        _ => return empty,
    };
    let absolute = Qualified::of(chosen, home).absolute();
    let node = match editor.document().node(&absolute) {
        Some(node) => node,
        // Chosen and then removed: the panel empties rather than showing a
        // node that is not there.
        None => return empty,
    };

    let declared = NodeType::ALL
        .iter()
        .find(|t| t.spelling() == node.node_type)
        .map(|t| t.attributes())
        .unwrap_or(&[]);

    let mut fields: Vec<FieldView> = declared
        .iter()
        .map(|(name, required)| {
            let value = node.attributes.get(*name).cloned().unwrap_or_default();
            FieldView {
                name: (*name).into(),
                value: value.clone().into(),
                required: *required,
                is_heading: false,
                simulated: false,
            }
        })
        .collect();

    // Anything the node holds that its type does not declare. Shown rather
    // than dropped: a file from a later build is readable, and an attribute
    // nobody can see is one that gets written back out without anybody
    // knowing it is there.
    for (name, value) in &node.attributes {
        if declared.iter().any(|(declared, _)| declared == name) {
            continue;
        }
        fields.push(FieldView {
            name: name.clone().into(),
            value: value.clone().into(),
            required: false,
            is_heading: false,
            simulated: false,
        });
    }

    // And, below a rule, what the simulator's file says about this payload.
    //
    // Here rather than in a window of its own, which is what it had: a
    // payload's simulation settings are about a payload that is already on the
    // canvas, and a second window would be a second place to look for what one
    // node says about itself. Two files, one panel, and a line to say where
    // one ends and the other begins.
    //
    // Only for a payload, and only where this session was given the
    // simulator's file. A computation, a log and the ground link are not
    // simulated -- there is nothing standing in for them -- so there is
    // nothing to show.
    if let Some(simulator) = &editor.document().simulator {
        if is_payload(&node.node_type) {
            let empty = BTreeMap::new();
            let said = simulator.payloads.get(&absolute).unwrap_or(&empty);
            fields.push(FieldView {
                name: "simulated payload".into(),
                value: String::new().into(),
                required: false,
                is_heading: true,
                simulated: true,
            });
            fields.extend(
                settings_of(&crate::sim::Simulator::fields(true), said)
                    .into_iter()
                    .map(|mut field| {
                        field.simulated = true;
                        field
                    }),
            );
        }
    }

    (
        node.name.clone(),
        node.node_type.clone(),
        node.comment.clone().unwrap_or_default(),
        fields,
    )
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

    // What a line may not run through: every node on this page, where it is
    // and how big it is. Worked out once for the page rather than once per
    // link, and handed to the router, which knows about geometry and nothing
    // about documents.
    let obstacles: Vec<route::Obstacle> = document
        .nodes_in(home)
        .iter()
        .map(|node| {
            let (width, height) = size_of(&node.node_type);
            route::Obstacle::new(
                node.at.0 as f32,
                node.at.1 as f32,
                width as f32,
                height as f32,
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

            // The label goes at the middle of the route's longest run, which
            // is on the line wherever the line had to go.
            let (label_x, label_y) = route::label_at(x1, y1, x2, y2, &obstacles);

            LinkView {
                // The route and the arrow, worked out where they can be
                // tested: see route.rs.
                route: route::route(x1, y1, x2, y2, &obstacles).into(),
                head: route::arrow(x2, y2).into(),
                marker: route::marker(x1, y1, x2, y2, &obstacles).into(),
                label_x,
                label_y,
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

/// Put the ground station into a configuration that has nothing in it.
///
/// Every graph has one: a vehicle whose telemetry reaches nobody is a vehicle
/// nobody is operating, so the ground link is the one node a configuration
/// cannot be without. Placing it is work the editor can do rather than work it
/// can wait for, and a canvas that opens with the thing everything else
/// connects to is a canvas that says where to start.
///
/// Through the journal like any other edit, so it can be undone by somebody
/// who is drawing a page of a bigger configuration and does not want a second
/// ground station on it.
///
/// Only for a configuration that is genuinely new. A document with nodes in it
/// has its own; one with a history behind it is a session being recovered, and
/// an empty one there means somebody deleted what was in it; and a file that
/// could not be read is a file to be repaired, not written over.
fn seed(editor: &mut Editor) {
    let fresh = editor.document().nodes.is_empty()
        && editor.undo_depth() == 0
        && editor.journal().redoable() == 0
        && editor.trouble().is_none();
    if !fresh {
        return;
    }

    // What tcspecial is configured with, which every configuration must say.
    //
    // The attributes and no values: a new configuration is given the questions
    // and answers none of them. It was given three -- a protocol, a beacon
    // interval, a segment size -- on the grounds that they are the same on
    // every vehicle until somebody has a reason, and that is the argument the
    // schema itself refuses: a configuration that was silently given an answer
    // looks as answered as one somebody thought about. So the fields are there
    // to be filled, empty, and marked until they are.
    for (attribute, _) in tcslibgs::graph::parameters::ATTRIBUTES {
        editor.edit(Edit::SetTcspecial {
            attribute: (*attribute).to_string(),
            was: None,
            now: Some(String::new()),
        });
    }

    // Two edits, as placing one by hand is: a node exists, and then it is
    // somewhere.
    editor.edit(Edit::AddNode {
        name: "moc".to_string(),
        node_type: "moc".to_string(),
        home: None,
    });
    editor.edit(Edit::MoveNode {
        node: "moc".to_string(),
        was: (0, 0),
        now: MOC_PLACE,
    });
}

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

/// How often a group reads until somebody says otherwise.
///
/// A second: slow enough for anything, fast enough to be obviously a value
/// somebody should look at rather than a default that happens to work.
const FIRST_INTERVAL: &str = "1s";

/// A name no group in the document has yet.
///
/// The word itself, numbered if that is taken, as a node's fresh name is: a
/// person who makes two groups gets `group` and `group-2` and renames them,
/// rather than being made to name one before it can exist.
fn fresh_group(document: &Document) -> String {
    if !document.groups.contains_key("group") {
        return "group".to_string();
    }
    (2..)
        .map(|n| format!("group-{n}"))
        .find(|name| !document.groups.contains_key(name))
        .expect("a free name exists")
}

/// Where the next node placed should go.
///
/// Counted within the home being edited and within the pane it is going into,
/// so a subdiagram's first node lands where the top diagram's first one does
/// rather than wherever the whole document's count happens to have reached --
/// and a payload placed after three computations lands at the top of its own
/// pane rather than three steps down a pane it was never in.
fn next_place(
    document: &Document,
    home: &Home,
    node_type: &str,
    divider: i32,
) -> (i32, i32) {
    let payload = is_payload(node_type);
    let placed = document
        .nodes_in(home)
        .iter()
        .filter(|node| is_payload(&node.node_type) == payload)
        .count() as i32;

    // Down the pane for a payload, and diagonally for everything else. The
    // payload working pane is a quarter as wide as the other, with room for
    // one node and little more: a diagonal there would walk two nodes into the
    // right-hand edge and then stack the rest on top of each other.
    let at = if payload {
        (divider + GRID * 3, FIRST_PLACE.1 + placed * PLACE_OFFSET)
    } else {
        (
            FIRST_PLACE.0 + placed * PLACE_OFFSET,
            FIRST_PLACE.1 + placed * PLACE_OFFSET,
        )
    };
    confine(node_type, divider, snap(at))
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

/// One of every node type on one side of the canvas, under its family's
/// heading.
///
/// Built from the same table the canvas uses, so a shape in the list is the
/// shape on the canvas rather than a picture of one that may drift away from
/// it. Grouped because the families *are* the shape grammar -- square is
/// hardware, rounded is software aboard, a rounded end is an end of the graph
/// -- and a flat list of fourteen shapes hides the pattern that would make
/// them learnable.
///
/// Two lists rather than one, because there are two working panes and a
/// shape's pane follows from the list it came from. A single list would leave
/// the rule to be found out by placing something and watching where it went.
fn palette_of(payloads: bool) -> ModelRc<PaletteEntry> {
    let mut entries: Vec<PaletteEntry> = Vec::new();
    let mut family = "";

    for t in NodeType::ALL.iter().copied().filter(|t| t.is_payload() == payloads) {
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

/// The palette's colours.
///
/// Held here rather than in `style.rs` on purpose: that module holds what a
/// type *means* and the rules the looks obey, and this holds what the meanings
/// are painted. Keeping them apart is what lets the accessibility test check
/// the rules without knowing a single hex value.
fn tint_of(tint: Tint) -> String {
    // Searched for rather than chosen by eye, and the thing searched for was
    // not prettiness: these are the fourteen sRGB colours that maximise the
    // smallest perceived difference between any two of them, taken under
    // normal vision and under simulated protanopia, deuteranopia and
    // tritanopia at once. The worst pair of the fourteen is 17 CIE units apart
    // under every one of the four, which the test below checks against these
    // very values.
    //
    // Two constraints beside that, in this order. Each has a contrast against
    // white of at least three to one, so that a one-pixel border of it is
    // legible rather than nearly there. And the chroma is kept moderate and
    // the lightness spread over five levels -- which is as much aesthetics as
    // the first priority leaves room for, and is also useful: lightness is the
    // one channel every kind of colour blindness keeps, the total ones
    // included.
    //
    // Tuning any of these means re-running that test, which is the point of
    // its existing.
    match tint {
        Tint::Indigo => "#243a91",
        Tint::Forest => "#254a2a",
        Tint::Bronze => "#504100",
        Tint::Plum => "#5f354c",
        Tint::Purple => "#6b357b",
        Tint::Magenta => "#9d4b82",
        Tint::Blue => "#3768b0",
        Tint::Rose => "#9b535c",
        Tint::Olive => "#6b722e",
        Tint::Taupe => "#9c9068",
        Tint::Periwinkle => "#8387ea",
        Tint::Gold => "#a3911b",
        Tint::Blush => "#bb8095",
        Tint::Lavender => "#9b87c3",
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

    /// Where the divider sits until somebody moves it.
    ///
    /// Most of what follows is about the two panes rather than about moving
    /// the line between them, so it is worked in the division the editor
    /// opens with.
    const DIVIDER: i32 = WORKING_WIDTH * settings::DEFAULT / 100;

    /// Every row of one selection pane.
    fn rows_of(payloads: bool) -> Vec<PaletteEntry> {
        let palette = palette_of(payloads);
        (0..palette.row_count())
            .map(|i| palette.row_data(i).unwrap())
            .collect()
    }

    /// The two selection panes offer every type the editor can place, and
    /// each type in exactly one of them.
    ///
    /// A type missing from both is a type nobody can place, which is a failure
    /// a person would find only by going looking for it. A type in both would
    /// be a type whose pane depends on which list it was pressed in, and the
    /// pane is what says whether it is a payload.
    #[test]
    fn the_selection_panes_offer_every_node_type_once() {
        let mut offered: Vec<String> = Vec::new();
        for payloads in [false, true] {
            let rows = rows_of(payloads);

            // Grouped, and every shape sits under a heading rather than above
            // the first one.
            assert!(
                rows[0].is_heading,
                "a selection pane does not start with a family"
            );
            let headings: Vec<String> = rows
                .iter()
                .filter(|r| r.is_heading)
                .map(|r| r.heading.to_string())
                .collect();
            // No family twice, which would mean the table had gone out of
            // family order and the pane was showing one group as two.
            let mut once = headings.clone();
            once.sort();
            once.dedup();
            assert_eq!(once.len(), headings.len(), "a family is listed twice");

            for row in rows.iter().filter(|r| !r.is_heading) {
                let spelling = row.node.node_type.to_string();
                let t = NodeType::ALL
                    .iter()
                    .find(|t| t.spelling() == spelling)
                    .copied()
                    .unwrap_or_else(|| panic!("{spelling} is not a type"));
                assert_eq!(
                    t.is_payload(),
                    payloads,
                    "{spelling} is offered in the wrong selection pane"
                );
                assert!(
                    headings.contains(&t.family().to_string()),
                    "{spelling} has no heading above it"
                );
                offered.push(spelling);
            }
        }

        offered.sort();
        let mut expected: Vec<String> = NodeType::ALL
            .iter()
            .map(|t| t.spelling().to_string())
            .collect();
        expected.sort();
        assert_eq!(offered, expected, "the two panes do not offer every type");
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
        for payloads in [false, true] {
            for row in rows_of(payloads) {
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

    /// Every pair of tints is told apart by a colour blind reader.
    ///
    /// The rule the palette was chosen for, checked against the colours the
    /// window is actually given rather than against the intention. Fourteen
    /// colours is enough that no amount of care at the keyboard would keep
    /// this true: the fourteenth tint added by hand would be a near-copy of
    /// one of the other thirteen under one of the three dichromacies, and
    /// nobody with normal vision would see it.
    ///
    /// The floor is fifteen CIE units, where two or three is "the same colour
    /// in two printings" and ten is "clearly two colours". The palette in
    /// `tint_of` manages seventeen, so there is room to adjust one -- and a
    /// failure here says the adjustment has to be searched for again rather
    /// than nudged.
    #[test]
    fn no_two_tints_are_the_same_colour_to_a_colour_blind_reader() {
        use sight::{contrast_on_white, difference, seen, Sight};

        let painted = |t: NodeType| {
            let c = colour(&tint_of(t.appearance().tint));
            (c.red(), c.green(), c.blue())
        };

        let mut closest = (f64::MAX, Sight::Normal, "", "");
        for (i, a) in NodeType::ALL.iter().enumerate() {
            for b in &NodeType::ALL[i + 1..] {
                for sight in Sight::ALL {
                    let apart =
                        difference(seen(painted(*a), sight), seen(painted(*b), sight));
                    if apart < closest.0 {
                        closest = (apart, sight, a.spelling(), b.spelling());
                    }
                }
            }
        }

        let (apart, sight, a, b) = closest;
        assert!(
            apart >= 15.0,
            "{a} and {b} are only {apart:.1} apart to a reader with \
             {sight:?}, and the floor is 15"
        );

        // And each is legible as a border on the white canvas, which is the
        // other half of a colour being usable: three to one is the threshold
        // for something that is not text.
        for t in NodeType::ALL {
            let contrast = contrast_on_white(painted(t));
            assert!(
                contrast >= 3.0,
                "{}'s tint has {contrast:.2} contrast against the canvas",
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

    /// The panel asks for what a type needs, filled in or not.
    ///
    /// A node just placed holds no attributes at all, and a panel showing
    /// only what it holds would show nothing -- leaving the person who placed
    /// it to know that an I2C device wants a bus and an address.
    #[test]
    fn the_panel_asks_for_what_the_type_needs() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut editor = Editor::new(&where_.path().join("v.yaml"));
        editor.edit(Edit::AddNode {
            name: "temp_fore".into(),
            node_type: "i2c".into(),
            home: None,
        });

        let (name, node_type, _, fields) = panel_of(&editor, &None, Some("temp_fore"));
        assert_eq!(name, "temp_fore");
        assert_eq!(node_type, "i2c");

        let shown: Vec<String> = fields.iter().map(|f| f.name.to_string()).collect();
        assert!(shown.contains(&"path".to_string()), "{shown:?}");
        assert!(shown.contains(&"address".to_string()), "{shown:?}");

        // The two it must have say so, and say it whether or not they are
        // filled in: the mark is what the configuration will need, not a
        // complaint about an empty field. The editor is lazy -- see
        // Document::faults -- so an empty one is a question not yet answered.
        for wanted in ["path", "address"] {
            let field = fields.iter().find(|f| f.name == wanted).unwrap();
            assert!(field.required, "{wanted} should be marked");
        }
        let optional = fields.iter().find(|f| f.name == "pec").unwrap();
        assert!(!optional.required);

        // And nothing is wrong with the node, which has nothing in it: the
        // one place that insists is the ground check, where the complaint can
        // name the file and the line.
        assert!(editor.faults().is_empty(), "{:?}", editor.faults());
    }

    /// A filled field shows what it was filled with.
    #[test]
    fn a_filled_field_shows_its_value() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut editor = Editor::new(&where_.path().join("v.yaml"));
        editor.edit(Edit::AddNode {
            name: "temp_fore".into(),
            node_type: "i2c".into(),
            home: None,
        });
        editor.edit(Edit::SetAttribute {
            node: "temp_fore".into(),
            attribute: "path".into(),
            was: None,
            now: Some("/dev/i2c-1".into()),
        });

        let (_, _, _, fields) = panel_of(&editor, &None, Some("temp_fore"));
        let path = fields.iter().find(|f| f.name == "path").unwrap();
        assert_eq!(path.value, "/dev/i2c-1");
        assert!(path.required, "the bus is one the configuration must state");

        // Half filled in is still not a fault, and the address it has not been
        // given yet is the ground check's to insist on.
        assert!(editor.faults().is_empty(), "{:?}", editor.faults());
    }

    /// An attribute the build does not know about is shown, not dropped.
    ///
    /// A file written by a later build may carry one, and an attribute nobody
    /// can see is one that gets written back out without anybody knowing it is
    /// there.
    #[test]
    fn an_unrecognised_attribute_is_still_shown() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut editor = Editor::new(&where_.path().join("v.yaml"));
        editor.edit(Edit::AddNode {
            name: "temp_fore".into(),
            node_type: "i2c".into(),
            home: None,
        });
        editor.edit(Edit::SetAttribute {
            node: "temp_fore".into(),
            attribute: "future_thing".into(),
            was: None,
            now: Some("7".into()),
        });

        let (_, _, _, fields) = panel_of(&editor, &None, Some("temp_fore"));
        let extra = fields
            .iter()
            .find(|f| f.name == "future_thing")
            .expect("the unknown attribute should be shown");
        assert_eq!(extra.value, "7");
        assert!(!extra.required);

        // After the declared ones, so the panel asks for what it needs first.
        let at = fields.iter().position(|f| f.name == "future_thing").unwrap();
        assert_eq!(at, fields.len() - 1, "it should come last");
    }

    /// A node chosen and then removed leaves the panel empty.
    #[test]
    fn the_panel_empties_when_its_node_goes() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut editor = Editor::new(&where_.path().join("v.yaml"));
        editor.edit(Edit::AddNode {
            name: "a".into(),
            node_type: "i2c".into(),
            home: None,
        });
        assert_eq!(panel_of(&editor, &None, Some("a")).0, "a");

        editor.edit(Edit::RemoveNode {
            name: "a".into(),
            node_type: "i2c".into(),
            attributes: Vec::new(),
            links: Vec::new(),
            home: None,
        });
        let (name, _, _, fields) = panel_of(&editor, &None, Some("a"));
        assert!(name.is_empty(), "the panel showed a node that is not there");
        assert!(fields.is_empty());
    }

    /// Nothing chosen, nothing shown.
    #[test]
    fn the_panel_is_empty_until_something_is_chosen() {
        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Editor::new(&where_.path().join("v.yaml"));
        for chosen in [None, Some(""), Some("nothing-here")] {
            let (name, _, _, fields) = panel_of(&editor, &None, chosen);
            assert!(name.is_empty(), "{chosen:?}");
            assert!(fields.is_empty(), "{chosen:?}");
        }
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

    /// Both working panes have room for a node.
    ///
    /// The payload working pane is a quarter of the working area, and every
    /// node is one width: a quarter narrower than that width would be a pane
    /// nothing fits in, and every payload placed would land in the same
    /// place because the clamp had nowhere to put it.
    #[test]
    fn both_panes_have_room_for_a_node() {
        let width = style::WIDTH as i32;
        assert!(
            WORKING_WIDTH - DIVIDER >= width + GRID * 2,
            "the payload working pane is {}px wide and a node is {width}px",
            WORKING_WIDTH - DIVIDER
        );
        // And the vehicle pane has room for several across, which is what it
        // is the larger of the two for.
        assert!(
            DIVIDER >= width * 3,
            "the vehicle working pane is {DIVIDER}px wide and a node is {width}px"
        );
        // Three quarters and one quarter to begin with, as asked for, and
        // on the grid so that a node held against the divider is on it too.
        assert_eq!(DIVIDER, divider_for(settings::DEFAULT));
        assert_eq!(DIVIDER, WORKING_WIDTH * 3 / 4);
        assert_eq!(DIVIDER % GRID, 0);
    }

    /// A node is held in the pane its type belongs to.
    ///
    /// Both directions, because a clamp written one-sided is a clamp that
    /// looks right until somebody drags the other way: a payload must not
    /// cross the divider leftwards and must not leave the working area
    /// rightwards.
    #[test]
    fn a_node_is_held_in_its_own_pane() {
        let width = style::WIDTH as i32;

        // A payload: left to the divider, right to the far edge.
        assert_eq!(confine("i2c", DIVIDER, (0, 100)), (DIVIDER, 100));
        assert_eq!(confine("i2c", DIVIDER, (DIVIDER - 1, 100)), (DIVIDER, 100));
        assert_eq!(confine("i2c", DIVIDER, (DIVIDER + 8, 100)), (DIVIDER + 8, 100));
        assert_eq!(
            confine("i2c", DIVIDER, (WORKING_WIDTH, 100)),
            (WORKING_WIDTH - width, 100)
        );

        // And everything else: nowhere past the divider, and nowhere left of
        // the canvas.
        assert_eq!(confine("compute", DIVIDER, (0, 100)), (0, 100));
        assert_eq!(confine("compute", DIVIDER, (-40, 100)), (0, 100));
        assert_eq!(
            confine("compute", DIVIDER, (DIVIDER, 100)),
            (DIVIDER - width, 100)
        );
        assert_eq!(
            confine("moc", DIVIDER, (WORKING_WIDTH, 100)),
            (DIVIDER - width, 100)
        );

        // A node is never dragged off the top, whichever pane it is in: a
        // negative y is a node nobody can reach to drag back.
        for node_type in ["i2c", "compute"] {
            assert_eq!(confine(node_type, DIVIDER, (DIVIDER, -80)).1, 0, "{node_type}");
        }

        // A type this build does not know goes in the vehicle pane, which is
        // where something that needs looking at should be.
        assert!(!is_payload("contraption"));
        let (left, right) = pane::pane_for("contraption", DIVIDER);
        assert_eq!(left, 0);
        assert!(right < DIVIDER);
    }

    /// Every type is placed in the pane its list is in.
    ///
    /// The rule that was asked for: a payload can only be created in the
    /// payload working pane and everything else only in the vehicle working
    /// pane. Checked for every type rather than for one of each, because the
    /// thing that would break it is a new type whose family says one thing and
    /// whose placement says another.
    #[test]
    fn every_type_is_placed_in_its_own_pane() {
        let document = Document::new();
        for t in NodeType::ALL {
            let (x, y) = next_place(&document, &None, t.spelling(), DIVIDER);
            let (width, _) = size_of(t.spelling());
            let right = x + width as i32;
            if t.is_payload() {
                assert!(
                    x >= DIVIDER && right <= WORKING_WIDTH,
                    "{} is placed at {x} and reaches {right}, outside the \
                     payload working pane",
                    t.spelling()
                );
            } else {
                assert!(
                    x >= 0 && right <= DIVIDER,
                    "{} is placed at {x} and reaches {right}, outside the \
                     vehicle working pane",
                    t.spelling()
                );
            }
            assert!(y > 0, "{} is placed off the top", t.spelling());
        }
    }

    /// Nodes placed in one pane do not step down the other.
    ///
    /// The offset is counted per pane, so a payload placed after three
    /// computations starts at the top of its own pane: counted over the whole
    /// page, the first payload would appear four steps down a column it had
    /// never been in, which looks like three nodes went missing.
    #[test]
    fn placing_in_one_pane_does_not_move_the_other_along() {
        let mut document = Document::new();
        let first_payload = next_place(&document, &None, "i2c", DIVIDER);

        for name in ["a", "b", "c"] {
            document.apply(&Edit::AddNode {
                name: name.into(),
                node_type: "compute".into(),
                home: None,
            });
        }
        assert_eq!(
            next_place(&document, &None, "i2c", DIVIDER),
            first_payload,
            "three computations moved where the first payload goes"
        );

        // And a payload placed does step the next one along, so two of them
        // are not drawn on top of each other.
        document.apply(&Edit::AddNode {
            name: "temp_fore".into(),
            node_type: "i2c".into(),
            home: None,
        });
        let second = next_place(&document, &None, "i2c", DIVIDER);
        assert_ne!(second, first_payload);
        assert_eq!(second.0, first_payload.0, "payloads go down their pane");
        assert!(second.1 > first_payload.1);
    }

    /// The divider comes back where it was left.
    ///
    /// What is written down is a percentage and what the canvas works in is
    /// pixels, so the two conversions have to agree or the panes would creep
    /// by a step every time the editor started. Every position the control can
    /// settle on, round tripped.
    #[test]
    fn the_divider_survives_being_written_down() {
        for percent in settings::LEAST..=settings::MOST {
            let divider = divider_for(percent);
            assert_eq!(divider % GRID, 0, "{percent}% is off the grid");
            // And a percentage means what it says: that much of the working
            // pane, to the left-hand pane, within the grid step it is
            // snapped to.
            assert!(
                (divider - WORKING_WIDTH * percent / 100).abs() <= GRID / 2,
                "a vehicle pane of {percent}% was placed at {divider} of \
                 {WORKING_WIDTH}"
            );
            let again = divider_for(percent_for(divider));
            assert_eq!(
                again, divider,
                "a vehicle pane of {percent}% comes back as {}%",
                percent_for(divider)
            );
        }

        // And a percentage outside the rule is brought inside it rather than
        // obeyed: a hand-edited settings file is still a file the editor has
        // to open.
        assert_eq!(divider_for(0), divider_for(settings::LEAST));
        assert_eq!(divider_for(100), divider_for(settings::MOST));
        assert_eq!(divider_for(-5), divider_for(settings::LEAST));
    }

    /// Neither working pane can be squeezed away.
    ///
    /// The rule that was asked for: each pane keeps at least a twentieth of
    /// the working pane, so neither can be dragged shut. A pane with nothing
    /// visible in it is a pane whose nodes cannot be reached, and the drag
    /// that does it is one press away.
    #[test]
    fn neither_pane_can_be_squeezed_away() {
        let document = Document::new();
        let (low, high) = divider_range(&document, DIVIDER);

        assert_eq!(low, WORKING_WIDTH * settings::LEAST / 100);
        assert_eq!(high, WORKING_WIDTH * settings::MOST / 100);
        // Said the way it was asked: neither pane below five per cent, so
        // neither above ninety-five.
        assert_eq!(percent_for(low), settings::LEAST);
        assert_eq!(percent_for(high), settings::MOST);
        assert!(100 - percent_for(high) >= settings::LEAST);
        assert!(low > 0 && high < WORKING_WIDTH);
    }

    /// The divider stops against a node rather than passing over it.
    ///
    /// Which is what keeps every node in the pane its type belongs to without
    /// the divider quietly rearranging the diagram: a line that stops where it
    /// would orphan something says what it is doing while it does it.
    #[test]
    fn the_divider_stops_against_a_node() {
        let mut document = Document::new();
        let width = style::WIDTH as i32;

        // A computation in the vehicle pane and a payload in the payload
        // pane, both where they belong.
        document.apply(&Edit::AddNode {
            name: "to_kelvin".into(),
            node_type: "compute".into(),
            home: None,
        });
        document.apply(&Edit::MoveNode {
            node: "to_kelvin".into(),
            was: (0, 0),
            now: (400, 40),
        });
        document.apply(&Edit::AddNode {
            name: "temp_fore".into(),
            node_type: "i2c".into(),
            home: None,
        });
        document.apply(&Edit::MoveNode {
            node: "temp_fore".into(),
            was: (0, 0),
            now: (DIVIDER + 48, 40),
        });

        let (low, high) = divider_range(&document, DIVIDER);
        assert_eq!(low, 400 + width, "the computation's right edge stops it");
        assert_eq!(high, DIVIDER + 48, "the payload's left edge stops it");
        assert!(low <= DIVIDER && DIVIDER <= high, "{low}..{high}");

        // A node on the wrong side of the divider holds nothing. Otherwise one
        // such node -- read from a file that put it there -- would freeze the
        // control, and the way to free it would be to drag the node, which is
        // the thing somebody would be using the divider to make room for.
        document.apply(&Edit::AddNode {
            name: "stray".into(),
            node_type: "spi".into(),
            home: None,
        });
        document.apply(&Edit::MoveNode {
            node: "stray".into(),
            was: (0, 0),
            now: (40, 200),
        });
        assert_eq!(
            divider_range(&document, DIVIDER),
            (low, high),
            "a payload left of the divider should not pin it"
        );
    }

    /// The groups menu names itself, then the groups, then the way to make
    /// one.
    ///
    /// The heading is a row of the menu because a Slint ComboBox shows
    /// whatever is selected: without it the control would read as a setting
    /// that says "cabin_temps", and a person would reasonably think they had
    /// just put something into that group. The last row matters for the
    /// opposite reason -- a configuration that declares no groups yet would
    /// otherwise have a menu with no way in.
    #[test]
    fn the_groups_menu_names_itself_and_offers_a_way_in() {
        use slint::Model;

        let mut document = Document::new();
        let rows = |document: &Document| -> Vec<String> {
            let menu = group_menu(document);
            (0..menu.row_count())
                .map(|i| menu.row_data(i).unwrap().to_string())
                .collect()
        };

        // With no groups at all: the heading and the way to make one.
        assert_eq!(rows(&document), vec![MENU_HEADING, MENU_NEW]);

        for (name, interval) in [("cabin_temps", "3s"), ("bay", "5s")] {
            document.apply(&Edit::AddGroup {
                name: name.into(),
                interval: interval.into(),
            });
        }
        assert_eq!(
            rows(&document),
            vec![MENU_HEADING, "bay", "cabin_temps", MENU_NEW],
            "the groups go between, in their own order"
        );

        // The heading and the new row are not names a group could have, or
        // choosing one would be ambiguous.
        assert!(MENU_HEADING.ends_with('\u{2026}'));
        assert!(MENU_NEW.ends_with('\u{2026}'));
    }

    /// A group made from the menu gets a name nothing else has.
    #[test]
    fn a_fresh_group_name_is_free() {
        let mut document = Document::new();
        assert_eq!(fresh_group(&document), "group");

        document.apply(&Edit::AddGroup {
            name: "group".into(),
            interval: FIRST_INTERVAL.into(),
        });
        assert_eq!(fresh_group(&document), "group-2");

        document.apply(&Edit::AddGroup {
            name: "group-2".into(),
            interval: FIRST_INTERVAL.into(),
        });
        assert_eq!(fresh_group(&document), "group-3");

        // And it comes with an interval, because a group without one reads
        // nothing.
        assert!(!FIRST_INTERVAL.is_empty());
    }

    /// A new configuration opens with the ground station in it.
    ///
    /// Every graph has one, so the editor places it rather than waiting to be
    /// asked -- and a canvas that opens with the thing everything else
    /// connects to is a canvas that says where to start.
    #[test]
    fn a_new_configuration_starts_with_the_ground_station() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.yaml");
        let mut editor = Editor::open(&path, DIVIDER);
        seed(&mut editor);

        // And what tcspecial is configured with: every attribute of it, and
        // not one answer. A new configuration is given the questions -- so
        // the window has a field apiece and the required ones are marked --
        // and answers none of them, because a value nobody chose looks as
        // chosen as one somebody did.
        let said = &editor.document().tcspecial;
        for (attribute, _) in tcslibgs::graph::parameters::ATTRIBUTES {
            assert_eq!(
                said.get(*attribute).map(String::as_str),
                Some(""),
                "{attribute} should be offered and empty"
            );
        }
        // And none of it is a fault: the editor is lazy, and a configuration
        // nobody has filled in yet is one nobody has filled in yet. What it
        // must say before it flies is the ground check's to insist on, with a
        // file and a line.
        assert!(editor.faults().is_empty(), "{:?}", editor.faults());

        // And nothing of it reaches the file, because nothing of it is
        // stated: a section with nothing under it would read back as no
        // section at all.
        let written = std::fs::read_to_string(&path).expect("the file");
        assert!(!written.contains("tcspecial"), "{written}");

        let moc = editor.document().node("moc").expect("a ground station");
        assert_eq!(moc.node_type, "moc");
        assert_eq!(moc.at, MOC_PLACE);
        // The upper left of the vehicle working pane.
        let (left, right) = pane::pane_for("moc", DIVIDER);
        assert!(moc.at.0 >= left && moc.at.0 <= right);

        // Undoable, like any other edit: somebody drawing a page of a bigger
        // configuration does not want a second ground station on it.
        assert!(editor.undo_depth() >= 1);
        editor.undo();
        editor.undo();
        assert!(
            editor.document().node("moc").is_none(),
            "the ground station cannot be taken back"
        );

        // And written, because every edit is: the file is a configuration
        // from the moment it is opened.
        assert!(path.exists(), "the new configuration was not written");
    }

    /// Nothing is seeded into a configuration that is not new.
    ///
    /// Three ways a document can be empty and not new, and all three would be
    /// the editor putting back something somebody had got rid of.
    #[test]
    fn a_configuration_that_is_not_new_is_left_alone() {
        let where_ = tempfile::tempdir().expect("a directory");

        // One that has something in it.
        let mut editor = Editor::open(&where_.path().join("a.yaml"), DIVIDER);
        editor.edit(Edit::AddNode {
            name: "to_kelvin".into(),
            node_type: "compute".into(),
            home: None,
        });
        seed(&mut editor);
        assert!(editor.document().node("moc").is_none());

        // One somebody has emptied, which has a history behind it.
        let mut editor = Editor::open(&where_.path().join("b.yaml"), DIVIDER);
        editor.edit(Edit::AddNode {
            name: "to_kelvin".into(),
            node_type: "compute".into(),
            home: None,
        });
        editor.undo();
        assert!(editor.document().nodes.is_empty());
        seed(&mut editor);
        assert!(
            editor.document().node("moc").is_none(),
            "a configuration somebody emptied was refilled"
        );

        // And one that could not be read, which is a file to repair rather
        // than a file to write over.
        let damaged = where_.path().join("c.yaml");
        std::fs::write(&damaged, "version: \"2.0\"\nnodes: [[[\n").expect("a file");
        let mut editor = Editor::open(&damaged, DIVIDER);
        assert!(editor.trouble().is_some(), "a damaged file should be reported");
        seed(&mut editor);
        assert!(editor.document().node("moc").is_none());
        assert_eq!(
            std::fs::read_to_string(&damaged).unwrap(),
            "version: \"2.0\"\nnodes: [[[\n",
            "a damaged configuration was written over"
        );
    }

    /// The place a first node is put is on the canvas.
    #[test]
    fn a_new_node_is_placed_where_it_can_be_seen() {
        assert!(FIRST_PLACE.0 > 0 && FIRST_PLACE.1 > 0);
    }

    /// No line in the shipped example is drawn through a node.
    ///
    /// The rules are checked in `route.rs` against boxes and numbers; this
    /// asks the same question of a real configuration, which is where a
    /// routing fault was actually found. The return link of this example used
    /// to run its long leg straight through the box of the node it was
    /// arriving at.
    #[test]
    fn the_shipped_example_draws_no_line_through_a_node() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

        for extension in ["yaml", "xml"] {
            let editor = Editor::open(&root.join(format!("example.{extension}")), DIVIDER);
            let document = editor.document();

            // The same boxes and the same ends views_of works out, because
            // what is being checked is what gets drawn.
            let obstacles: Vec<route::Obstacle> = document
                .nodes_in(&None)
                .iter()
                .map(|node| {
                    let (width, height) = size_of(&node.node_type);
                    route::Obstacle::new(
                        node.at.0 as f32,
                        node.at.1 as f32,
                        width as f32,
                        height as f32,
                    )
                })
                .collect();
            assert!(!obstacles.is_empty(), "example.{extension} has no nodes");
            assert!(!document.links.is_empty(), "and no links");

            for link in &document.links {
                let from = document.node(&link.from).expect("the source");
                let to = document.node(&link.to).expect("the target");
                let (width, height) = size_of(&from.node_type);
                let (_, arrives) = size_of(&to.node_type);
                let through = route::runs_through(
                    (from.at.0 + width as i32) as f32,
                    (from.at.1 + height as i32 / 2) as f32,
                    to.at.0 as f32,
                    (to.at.1 + arrives as i32 / 2) as f32,
                    &obstacles,
                );
                assert_eq!(
                    through, 0,
                    "example.{extension}: the line from {} to {} runs through \
                     {through} node(s)",
                    link.from, link.to
                );
            }
        }
    }

    /// What needs a window, which is one test because only one may have one.
    ///
    /// One test per binary may start the testing backend, and its windows
    /// belong to the thread that made them -- so these are functions called in
    /// order rather than tests of their own, as tcsmoc's window tests are.
    #[test]
    fn what_a_window_shows_and_what_a_gesture_does() {
        i_slint_backend_testing::init_integration_test_with_system_time();

        dragging_a_node_moves_it();
        a_click_opens_a_window_per_node();
        the_menu_opens_the_tcspecial_parameters();
        a_payloads_panel_carries_its_simulation_settings();
        the_menu_opens_the_simulators_groups();
        a_group_window_sets_every_payload_in_its_group();
        a_payload_stays_in_the_payload_working_pane();
        the_divider_can_be_dragged();
        a_node_follows_the_pointer_and_reports_where_it_went();
        an_existing_configuration_is_on_the_canvas();
    }

    /// Opening a configuration puts it on the canvas.
    ///
    /// Which it did not: tcsgui had no reader, so a file that already existed
    /// opened as an empty canvas -- and the first edit wrote that emptiness
    /// over the file. Both spellings of the shipped example, because the
    /// reader has a path for each.
    fn an_existing_configuration_is_on_the_canvas() {
        use slint::Model;
        use std::rc::Rc;

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

        for extension in ["yaml", "xml"] {
            let path = root.join(format!("example.{extension}"));
            let editor = Rc::new(RefCell::new(Editor::open(&path, DIVIDER)));
            assert_eq!(
                editor.borrow().trouble(),
                None,
                "example.{extension} should open without complaint"
            );

            let ui = MainWindow::new().expect("a window");
            furnish!(&ui);
            put(&ui, &editor.borrow(), &None, None, DIVIDER);

            let names: Vec<String> = (0..ui.get_nodes().row_count())
                .filter_map(|row| ui.get_nodes().row_data(row))
                .map(|node| node.name.to_string())
                .collect();
            assert_eq!(
                names,
                vec!["payload".to_string(), "moc".to_string()],
                "example.{extension} should put its nodes on the canvas"
            );
            assert_eq!(
                ui.get_links().row_count(),
                2,
                "example.{extension} should draw both links"
            );

            // Each with a head and a marker: the two links of this example
            // run between the same pair of nodes, one each way, so the
            // direction is the only thing that tells them apart.
            for row in 0..ui.get_links().row_count() {
                let link = ui.get_links().row_data(row).expect("a link");
                assert!(
                    link.head.ends_with('Z'),
                    "example.{extension}: link {row} has no arrowhead: {:?}",
                    link.head
                );
                assert!(
                    link.marker.ends_with('Z'),
                    "example.{extension}: link {row} has no flow marker: {:?}",
                    link.marker
                );
            }
        }
    }

    /// Dragging a node moves it, all the way into the document.
    ///
    /// Pressed, moved and released through the window, and then the document
    /// is read: a drag is markup wired to a handler, so nothing short of the
    /// whole path says whether dragging works. It is the one gesture that
    /// cannot be tried any other way, and it was broken twice over -- once in
    /// the arithmetic that places a node while the pointer is down, and once
    /// by the canvas being rebuilt under the pointer when the same press
    /// chose the node.
    fn dragging_a_node_moves_it() {
        use i_slint_backend_testing as testing;
        use slint::platform::{PointerEventButton, WindowEvent};
        use std::rc::Rc;

        // An editor on a file of this test's own: tcsgui saves as it edits,
        // and a test that wrote into the repository would be a test that
        // rewrote whatever it was pointed at.
        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Rc::new(RefCell::new(Editor::open(
            &where_.path().join("graph.yaml"),
            DIVIDER,
        )));
        // A compute rather than a device: a device is a payload and is held
        // in the payload working pane, so a drag of one measures the pane's
        // edge as much as the gesture. That is worth a test of its own, and
        // has one below.
        editor.borrow_mut().edit(Edit::AddNode {
            name: "to_kelvin".to_string(),
            node_type: "compute".to_string(),
            home: None,
        });
        // Somewhere on the canvas rather than in its corner: a node at the
        // origin is a node whose own position cannot be told from the
        // canvas's.
        editor.borrow_mut().edit(Edit::MoveNode {
            node: "to_kelvin".to_string(),
            was: (0, 0),
            now: (120, 88),
        });
        let was = editor
            .borrow()
            .document()
            .node("to_kelvin")
            .expect("the node")
            .at;

        let ui = MainWindow::new().expect("a window");
        furnish!(&ui);
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            divider: Rc::new(std::cell::Cell::new(DIVIDER)),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));
        let chosen: Rc<RefCell<BTreeMap<String, String>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let drawing: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        views.borrow_mut().chosen = chosen.clone();
        wire(&ui, &editor, &views, &drawing, &chosen, &None);
        views.borrow().refresh(&editor.borrow());

        ui.show().expect("the window shows");

        let canvas = testing::ElementHandle::find_by_element_id(&ui, "Surface::canvas")
            .next()
            .expect("the window has a canvas");
        let at = canvas.absolute_position();
        let (width, height) = size_of("compute");

        // The middle of the node, which is where a person presses.
        let from = slint::LogicalPosition::new(
            at.x + was.0 as f32 + width as f32 / 2.0,
            at.y + was.1 as f32 + height as f32 / 2.0,
        );
        let by = (64.0, 40.0);

        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position: from });
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position: from,
            button: PointerEventButton::Left,
        });

        // A pointer sends a stream of small movements, not one jump, and a
        // drag has to add up to the same thing either way: an offset measured
        // in a frame the offset itself moves loses a little on every step, so
        // a single jump and a hundred steps are two different tests.
        let steps = 8;
        for step in 1..=steps {
            let part = step as f32 / steps as f32;
            ui.window().dispatch_event(WindowEvent::PointerMoved {
                position: slint::LogicalPosition::new(
                    from.x + by.0 * part,
                    from.y + by.1 * part,
                ),
            });
        }

        // It follows the pointer while the gesture is running, which is the
        // half a person sees: a drag that only moved the node on release
        // would look like a drag that was doing nothing.
        // The one on the canvas, the others being the palette's.
        let drawn = testing::ElementHandle::find_by_element_id(&ui, "NodeShape::moving")
            .find(|handle| handle.absolute_position().x >= at.x)
            .expect("a node draws its parts in a container that the drag moves");
        let followed = drawn.absolute_position();
        assert!(
            (followed.x - (at.x + was.0 as f32 + by.0)).abs() < 1.0
                && (followed.y - (at.y + was.1 as f32 + by.1)).abs() < 1.0,
            "mid-drag the node is drawn at ({}, {}), not ({}, {})",
            followed.x,
            followed.y,
            at.x + was.0 as f32 + by.0,
            at.y + was.1 as f32 + by.1
        );

        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position: slint::LogicalPosition::new(from.x + by.0, from.y + by.1),
            button: PointerEventButton::Left,
        });

        // And the document says so, which is what is saved and what undo
        // takes back.
        let now = editor
            .borrow()
            .document()
            .node("to_kelvin")
            .expect("the node")
            .at;
        assert_eq!(
            now,
            (was.0 + by.0 as i32, was.1 + by.1 as i32),
            "a node dragged by {by:?} from {was:?} is at {now:?}"
        );
    }

    /// A payload dragged towards the vehicle stops at the divider.
    ///
    /// The confinement, through the window rather than through [`confine`]:
    /// the arithmetic is tested below, and this says the arithmetic is on the
    /// path a drag actually takes. What it would catch is the clamp being
    /// applied to the wrong number -- the offset rather than the position, or
    /// the position before the snap -- which arithmetic on its own cannot.
    fn a_payload_stays_in_the_payload_working_pane() {
        use i_slint_backend_testing as testing;
        use slint::platform::{PointerEventButton, WindowEvent};
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Rc::new(RefCell::new(Editor::open(
            &where_.path().join("graph.yaml"),
            DIVIDER,
        )));
        editor.borrow_mut().edit(Edit::AddNode {
            name: "temp_fore".to_string(),
            node_type: "i2c".to_string(),
            home: None,
        });
        // In its own pane, a little in from the divider.
        let was = (DIVIDER + 24, 88);
        editor.borrow_mut().edit(Edit::MoveNode {
            node: "temp_fore".to_string(),
            was: (0, 0),
            now: was,
        });

        let ui = MainWindow::new().expect("a window");
        furnish!(&ui);
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            divider: Rc::new(std::cell::Cell::new(DIVIDER)),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));
        let chosen: Rc<RefCell<BTreeMap<String, String>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let drawing: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        views.borrow_mut().chosen = chosen.clone();
        wire(&ui, &editor, &views, &drawing, &chosen, &None);
        views.borrow().refresh(&editor.borrow());
        ui.show().expect("the window shows");

        let canvas = testing::ElementHandle::find_by_element_id(&ui, "Surface::canvas")
            .next()
            .expect("the window has a canvas");
        let at = canvas.absolute_position();
        let (width, height) = size_of("i2c");
        let from = slint::LogicalPosition::new(
            at.x + was.0 as f32 + width as f32 / 2.0,
            at.y + was.1 as f32 + height as f32 / 2.0,
        );
        // Far enough left to be in the vehicle pane if nothing stopped it.
        let by = (-240.0, 24.0);
        let to = slint::LogicalPosition::new(from.x + by.0, from.y + by.1);

        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position: from });
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position: from,
            button: PointerEventButton::Left,
        });
        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position: to });
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position: to,
            button: PointerEventButton::Left,
        });

        let now = editor
            .borrow()
            .document()
            .node("temp_fore")
            .expect("the node")
            .at;
        assert_eq!(
            now,
            (DIVIDER, was.1 + by.1 as i32),
            "a payload dragged {by:?} from {was:?} left its pane and is at {now:?}"
        );
    }

    /// The menu item opens the parameters, and typing in them reaches the
    /// configuration.
    ///
    /// The whole path bar the pointer: the menu item's callback, the window it
    /// makes, the fields it shows, and the edit one of them records. What it
    /// would catch is the window being built from what the document happens to
    /// hold rather than from the list of what a section may say -- which would
    /// show nothing at all for a configuration that has never had one, and
    /// leave no way to give it one.
    fn the_menu_opens_the_tcspecial_parameters() {
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Rc::new(RefCell::new(Editor::open(
            &where_.path().join("vehicle.yaml"),
            DIVIDER,
        )));

        let ui = MainWindow::new().expect("a window");
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            divider: Rc::new(std::cell::Cell::new(DIVIDER)),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));
        let chosen: Rc<RefCell<BTreeMap<String, String>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let drawing: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        wire(&ui, &editor, &views, &drawing, &chosen, &None);

        // Nothing is open until the menu item is chosen.
        assert!(views.borrow().parameters.is_none());
        ui.invoke_edit_parameters();

        let window = views
            .borrow()
            .parameters
            .as_ref()
            .map(|window| window.as_weak())
            .expect("the menu should have opened the parameters")
            .upgrade()
            .expect("and left them open");

        // A field for every attribute the section may have, the required ones
        // first and marked, and this configuration has none of them.
        use slint::Model;
        let fields: Vec<FieldView> = (0..window.get_fields().row_count())
            .filter_map(|row| window.get_fields().row_data(row))
            .collect();
        let offered: Vec<String> = fields.iter().map(|f| f.name.to_string()).collect();
        let expected: Vec<String> = tcslibgs::graph::parameters::ATTRIBUTES
            .iter()
            .map(|(name, _)| (*name).to_string())
            .collect();
        assert_eq!(offered, expected, "the fields are not the attributes");
        // The seven that must be stated say so. Said and not insisted on:
        // an empty one is a question not yet answered, which is why none of
        // this is a fault.
        assert_eq!(
            fields.iter().filter(|f| f.required).count(),
            tcslibgs::graph::parameters::ATTRIBUTES
                .iter()
                .filter(|(_, required)| *required)
                .count()
        );
        assert_eq!(window.get_complaint(), "", "nothing is wrong yet");

        // Typing in one records an edit, and one Undo takes it back.
        let depth = editor.borrow().undo_depth();
        window.invoke_parameter_set("port".into(), "4000".into());
        assert_eq!(
            editor
                .borrow()
                .document()
                .tcspecial
                .get("port")
                .map(String::as_str),
            Some("4000")
        );
        assert_eq!(editor.borrow().undo_depth(), depth + 1);

        // And the window says so the next time it is drawn: the field is
        // filled and no longer marked.
        let port = (0..window.get_fields().row_count())
            .filter_map(|row| window.get_fields().row_data(row))
            .find(|f| f.name == "port")
            .expect("the port field");
        assert_eq!(port.value, "4000");

        assert!(editor.borrow_mut().undo());
        assert!(!editor.borrow().document().tcspecial.contains_key("port"));
    }

    /// A click on a node opens a window for it, and another node opens
    /// another.
    ///
    /// What replaced the panel down the right-hand side. The panel showed
    /// whichever node was pressed last, so comparing two sensors on one bus
    /// was press, read, press, remember; now each node's attributes are a
    /// window and they stay until they are closed. Closing takes a press of
    /// its own, or the windows would be the panel again.
    ///
    /// Opened by a click and not by a press, because a press is how a drag
    /// starts: a gesture that moved the node is a move, and one that moved
    /// nothing is a request to see what the node says. Told apart by the
    /// distance, not by the time.
    fn a_click_opens_a_window_per_node() {
        use i_slint_backend_testing as testing;
        use slint::platform::{PointerEventButton, WindowEvent};
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Rc::new(RefCell::new(Editor::open(
            &where_.path().join("vehicle.yaml"),
            DIVIDER,
        )));
        {
            let mut editor = editor.borrow_mut();
            for (name, at) in [("to_kelvin", (120, 88)), ("mean_temp", (120, 240))] {
                editor.edit(Edit::AddNode {
                    name: name.to_string(),
                    node_type: "compute".to_string(),
                    home: None,
                });
                editor.edit(Edit::MoveNode {
                    node: name.to_string(),
                    was: (0, 0),
                    now: at,
                });
            }
        }

        let ui = MainWindow::new().expect("a window");
        furnish!(&ui);
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            divider: Rc::new(std::cell::Cell::new(DIVIDER)),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));
        let chosen: Rc<RefCell<BTreeMap<String, String>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let drawing: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        views.borrow_mut().chosen = chosen.clone();
        wire(&ui, &editor, &views, &drawing, &chosen, &None);
        views.borrow().refresh(&editor.borrow());
        ui.show().expect("the window shows");

        let canvas = testing::ElementHandle::find_by_element_id(&ui, "Surface::canvas")
            .next()
            .expect("the window has a canvas");
        let at = canvas.absolute_position();
        let (width, height) = size_of("compute");

        // Pressed and released at one place, which is a click.
        let click = |node: (i32, i32)| {
            let on = slint::LogicalPosition::new(
                at.x + node.0 as f32 + width as f32 / 2.0,
                at.y + node.1 as f32 + height as f32 / 2.0,
            );
            ui.window()
                .dispatch_event(WindowEvent::PointerMoved { position: on });
            ui.window().dispatch_event(WindowEvent::PointerPressed {
                position: on,
                button: PointerEventButton::Left,
            });
            ui.window().dispatch_event(WindowEvent::PointerReleased {
                position: on,
                button: PointerEventButton::Left,
            });
        };

        assert!(views.borrow().attributes.is_empty(), "nothing open yet");

        click((120, 88));
        assert!(
            views.borrow().attributes.contains_key("to_kelvin"),
            "a click should open what the node says: {:?}",
            views.borrow().attributes.keys().collect::<Vec<_>>()
        );
        // And it moved nothing, so nothing is in the history.
        assert_eq!(editor.borrow().undo_depth(), 4, "a click is not an edit");

        // The window shows that node and no other, and offers what the type
        // declares.
        let window = views
            .borrow()
            .attributes
            .get("to_kelvin")
            .map(|window| window.as_weak())
            .unwrap()
            .upgrade()
            .expect("still open");
        assert_eq!(window.get_node(), "to_kelvin");
        assert_eq!(window.get_node_type(), "compute");

        // A second node opens a second window, and the first stays: which is
        // the whole point of this not being a panel.
        click((120, 240));
        assert_eq!(
            views.borrow().attributes.len(),
            2,
            "a second node should open a second window: {:?}",
            views.borrow().attributes.keys().collect::<Vec<_>>()
        );
        assert!(views.borrow().attributes.contains_key("to_kelvin"));

        // Clicking the same node again raises its window rather than making a
        // second one saying the same thing.
        click((120, 88));
        assert_eq!(views.borrow().attributes.len(), 2);

        // Typing in it reaches the document, and the other window is
        // untouched.
        window.invoke_attribute_set("function".into(), "to_kelvin".into());
        assert_eq!(
            editor
                .borrow()
                .document()
                .node("to_kelvin")
                .unwrap()
                .attributes
                .get("function")
                .map(String::as_str),
            Some("to_kelvin")
        );

        // And closing takes a press of its own, and takes only that one.
        window.invoke_closed();
        assert_eq!(
            views.borrow().attributes.keys().collect::<Vec<_>>(),
            vec!["mean_temp"],
            "Close should shut one window and leave the other"
        );
    }

    /// A payload's panel carries the simulator's settings for it, and typing
    /// in them reaches the simulator's file.
    ///
    /// The settings were in a window of their own and are in the panel now: a
    /// payload's simulation settings are about a payload that is already on
    /// the canvas, and a second window was a second place to look for what one
    /// node says about itself. Two files, one panel, and a rule between them.
    fn a_payloads_panel_carries_its_simulation_settings() {
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Rc::new(RefCell::new(Editor::open(
            &where_.path().join("vehicle.yaml"),
            DIVIDER,
        )));
        {
            let mut editor = editor.borrow_mut();
            for (name, node_type) in [("temp_fore", "i2c"), ("k", "compute")] {
                editor.edit(Edit::AddNode {
                    name: name.to_string(),
                    node_type: node_type.to_string(),
                    home: None,
                });
            }
        }

        // With no simulator file, a payload's panel is its own attributes and
        // nothing else: there is no second file for anything to go in.
        let (_, _, _, fields) = panel_of(&editor.borrow(), &None, Some("temp_fore"));
        assert!(
            fields.iter().all(|f| !f.is_heading && !f.simulated),
            "the panel should carry nothing of a file that was not given"
        );

        editor
            .borrow_mut()
            .open_simulator(&where_.path().join("vehiclesim.yaml"));

        // Now the payload's panel is its attributes, a rule, and the
        // simulator's settings for it.
        let (_, _, _, fields) = panel_of(&editor.borrow(), &None, Some("temp_fore"));
        let rule = fields
            .iter()
            .position(|f| f.is_heading)
            .expect("a rule between the two files");
        assert!(
            fields[..rule].iter().all(|f| !f.simulated),
            "the node's own attributes come first"
        );
        assert!(
            fields[rule + 1..].iter().all(|f| f.simulated),
            "and the simulator's settings after"
        );
        let after: Vec<String> = fields[rule + 1..]
            .iter()
            .map(|f| f.name.to_string())
            .collect();
        assert_eq!(after, crate::sim::Simulator::fields(true));

        // A computation is not simulated -- nothing stands in for it -- so
        // its panel carries none of this.
        let (_, _, _, fields) = panel_of(&editor.borrow(), &None, Some("k"));
        assert!(
            fields.iter().all(|f| !f.is_heading && !f.simulated),
            "a computation has no simulated payload"
        );

        // And typing in one reaches the simulator's file, as its own edit --
        // through the node's own window, which is where the fields are.
        let ui = MainWindow::new().expect("a window");
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            divider: Rc::new(std::cell::Cell::new(DIVIDER)),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));

        show_node(&editor, &views, "temp_fore");
        let window = views
            .borrow()
            .attributes
            .get("temp_fore")
            .map(|window| window.as_weak())
            .expect("the node's window")
            .upgrade()
            .expect("still open");

        let depth = editor.borrow().undo_depth();
        window.invoke_sim_attribute_set("drop_percent".into(), "10".into());
        assert_eq!(
            editor.borrow().document().simulator.as_ref().unwrap().payloads
                ["temp_fore"]["drop_percent"],
            "10"
        );
        // One edit, and the node's own attributes are untouched: two files,
        // and the panel knows which row goes where.
        assert_eq!(editor.borrow().undo_depth(), depth + 1);
        assert!(!editor
            .borrow()
            .document()
            .node("temp_fore")
            .unwrap()
            .attributes
            .contains_key("drop_percent"));
        assert!(editor.borrow_mut().undo());
        assert!(editor.borrow().document().simulator.as_ref().unwrap().payloads
            ["temp_fore"]
            .get("drop_percent")
            .is_none());
    }

    /// The menu opens the simulator's groups, and typing in them reaches its
    /// file.
    fn the_menu_opens_the_simulators_groups() {
        use slint::Model;
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Rc::new(RefCell::new(Editor::open(
            &where_.path().join("vehicle.yaml"),
            DIVIDER,
        )));
        editor
            .borrow_mut()
            .open_simulator(&where_.path().join("vehiclesim.yaml"));

        let ui = MainWindow::new().expect("a window");
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            divider: Rc::new(std::cell::Cell::new(DIVIDER)),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));
        let chosen: Rc<RefCell<BTreeMap<String, String>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let drawing: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        wire(&ui, &editor, &views, &drawing, &chosen, &None);
        views.borrow().refresh(&editor.borrow());

        // The entry is live, this session having been given a simulator file.
        assert!(ui.get_has_simulator());

        ui.invoke_edit_simulator_groups();
        let groups = views
            .borrow()
            .sim_groups
            .as_ref()
            .map(|window| window.as_weak())
            .expect("the menu should have opened the groups")
            .upgrade()
            .expect("and left them open");
        assert!(groups.get_can_add(), "a group is this file's to make");
        assert_eq!(groups.get_choices().row_count(), 0, "none yet");

        groups.invoke_add();
        assert_eq!(groups.get_chosen(), "group");
        assert_eq!(groups.get_choices().row_count(), 1);
        groups.invoke_setting_set("packet_interval_ms".into(), "500".into());
        assert_eq!(
            editor.borrow().document().simulator.as_ref().unwrap().groups["group"]
                ["packet_interval_ms"],
            "500"
        );
        // And it has no field for a group of its own, a group having none.
        let offered: Vec<String> = (0..groups.get_fields().row_count())
            .filter_map(|row| groups.get_fields().row_data(row))
            .map(|f| f.name.to_string())
            .collect();
        assert!(!offered.contains(&"group".to_string()), "{offered:?}");

        // Removing one takes what it held with it, so that undo puts back the
        // group that was there.
        groups.invoke_remove();
        assert!(editor
            .borrow()
            .document()
            .simulator
            .as_ref()
            .unwrap()
            .groups
            .is_empty());
        assert!(editor.borrow_mut().undo());
        assert_eq!(
            editor.borrow().document().simulator.as_ref().unwrap().groups["group"]
                ["packet_interval_ms"],
            "500",
            "the group came back empty"
        );
    }

    /// Typing in a group's window reaches every payload in the group.
    ///
    /// Through the window's own callbacks and the handlers wired to them,
    /// which is the whole path bar the pointer: what a person types in the
    /// field arrives as one edit, every payload of the group has the value,
    /// and one Undo takes the lot back. The last part is what was asked for
    /// and is the part a dozen separate edits would quietly fail.
    fn a_group_window_sets_every_payload_in_its_group() {
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Rc::new(RefCell::new(Editor::open(
            &where_.path().join("vehicle.yaml"),
            DIVIDER,
        )));
        {
            let mut editor = editor.borrow_mut();
            editor.edit(Edit::AddGroup {
                name: "cabin_temps".to_string(),
                interval: "3s".to_string(),
            });
            for name in ["temp_fore", "temp_aft"] {
                editor.edit(Edit::AddNode {
                    name: name.to_string(),
                    node_type: "i2c".to_string(),
                    home: None,
                });
                editor.edit(Edit::SetAttribute {
                    node: name.to_string(),
                    attribute: "group".to_string(),
                    was: None,
                    now: Some("cabin_temps".to_string()),
                });
            }
        }

        let ui = MainWindow::new().expect("a window");
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            divider: Rc::new(std::cell::Cell::new(DIVIDER)),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));

        show_group(&editor, &views, "cabin_temps");

        // The window shows what the group is: its interval, and the payloads
        // in it.
        let window = views
            .borrow()
            .group
            .as_ref()
            .map(|window| window.as_weak())
            .expect("a group window should have been made")
            .upgrade()
            .expect("and still be there");
        assert_eq!(window.get_group(), "cabin_temps");
        assert_eq!(window.get_interval(), "3s");
        assert_eq!(window.get_members_text(), "temp_fore, temp_aft");

        // One of the fields they both have, set once.
        let depth = editor.borrow().undo_depth();
        window.invoke_attribute_set("path".into(), "/dev/i2c-1".into());

        for name in ["temp_fore", "temp_aft"] {
            assert_eq!(
                editor
                    .borrow()
                    .document()
                    .node(name)
                    .unwrap()
                    .attributes
                    .get("path")
                    .map(String::as_str),
                Some("/dev/i2c-1"),
                "{name} did not get the group's value"
            );
        }

        // One edit for one gesture, and one Undo takes it back.
        assert_eq!(
            editor.borrow().undo_depth(),
            depth + 1,
            "setting one value on a group should be one edit"
        );
        assert!(editor.borrow_mut().undo());
        for name in ["temp_fore", "temp_aft"] {
            assert_eq!(
                editor.borrow().document().node(name).unwrap().attributes.get("path"),
                None,
                "{name} kept the value after one undo"
            );
        }

        // Renaming the group from its own window renames it on its members,
        // and the window goes on working afterwards -- its handlers read the
        // group as they run rather than remembering the name they were made
        // with.
        window.invoke_renamed("cabin".into());
        assert_eq!(window.get_group(), "cabin");
        assert_eq!(
            editor
                .borrow()
                .document()
                .node("temp_fore")
                .unwrap()
                .attributes
                .get("group")
                .map(String::as_str),
            Some("cabin")
        );
        window.invoke_interval_set("500ms".into());
        assert_eq!(
            editor.borrow().document().groups.get("cabin").map(String::as_str),
            Some("500ms")
        );
    }

    /// The divider's handle can be found, pressed and dragged.
    ///
    /// The markup alone, with no editor behind it: what the window reports is
    /// what the handler would act on, and the handler's arithmetic is tested
    /// above. Deliberately not the whole path, because moving the divider
    /// writes the settings file -- which lives in the home directory of
    /// whoever is running the tests, and a test that wrote there would be a
    /// test that changed how somebody's editor opens.
    fn the_divider_can_be_dragged() {
        use i_slint_backend_testing as testing;
        use slint::platform::{PointerEventButton, WindowEvent};
        use std::cell::Cell;
        use std::rc::Rc;

        let window = MainWindow::new().expect("a window");
        furnish!(&window);
        window.set_divider(DIVIDER as f32);

        let moved: Rc<Cell<Option<f32>>> = Rc::new(Cell::new(None));
        {
            let moved = moved.clone();
            window.on_divider_moved(move |dx| moved.set(Some(dx)));
        }
        window.show().expect("the window shows");

        let canvas = testing::ElementHandle::find_by_element_id(&window, "Surface::canvas")
            .next()
            .expect("the window has a canvas");
        let at = canvas.absolute_position();

        // On the line, halfway down it, which is where the handle is.
        let from = slint::LogicalPosition::new(at.x + DIVIDER as f32, at.y + 200.0);
        let by = -120.0;
        let to = slint::LogicalPosition::new(from.x + by, from.y);

        window
            .window()
            .dispatch_event(WindowEvent::PointerMoved { position: from });
        window.window().dispatch_event(WindowEvent::PointerPressed {
            position: from,
            button: PointerEventButton::Left,
        });
        window
            .window()
            .dispatch_event(WindowEvent::PointerMoved { position: to });
        window.window().dispatch_event(WindowEvent::PointerReleased {
            position: to,
            button: PointerEventButton::Left,
        });

        let said = moved
            .get()
            .expect("dragging the divider should report how far it went");
        assert!(
            (said - by).abs() < 1.0,
            "the divider dragged by {by} reported {said}"
        );
    }

    /// The markup alone reports the move it was given.
    ///
    /// Apart from the one above because it fails differently: this says the
    /// drag arithmetic is wrong, where that says the gesture does not reach
    /// the document. The offset used to be measured against the node's own
    /// `TouchArea`, which moves with the node, so the delta cancelled itself
    /// -- and this reported (0, 0) for every drag.
    fn a_node_follows_the_pointer_and_reports_where_it_went() {
        use i_slint_backend_testing as testing;
        use slint::platform::{PointerEventButton, WindowEvent};
        use std::cell::Cell;
        use std::rc::Rc;

        let window = MainWindow::new().expect("a window");
        let at = (120, 90);
        let node = view_of("temp_fore", "i2c", at, None, false, None);
        let (width, height) = (node.width, node.height);
        window
            .set_nodes(ModelRc::from(Rc::new(VecModel::from(vec![node]))));

        // What the window says it moved, which is what the editor would
        // record.
        let moved: Rc<Cell<Option<(f32, f32)>>> = Rc::new(Cell::new(None));
        {
            let moved = moved.clone();
            window.on_node_moved(move |_name, dx, dy| moved.set(Some((dx, dy))));
        }

        // The canvas is laid out before anything can be pressed on it.
        window.show().expect("the window shows");
        let canvas = testing::ElementHandle::find_by_element_id(&window, "Surface::canvas")
            .next();
        let (origin_x, origin_y) = match canvas {
            Some(canvas) => {
                let at = canvas.absolute_position();
                (at.x, at.y)
            }
            // The canvas has no id to find: the node's own position is still
            // relative to it, so the press would land somewhere else
            // entirely. Said rather than guessed at.
            None => panic!("the canvas should be findable by its element id"),
        };

        // The middle of the node, which is where a person presses.
        let from = slint::LogicalPosition::new(
            origin_x + at.0 as f32 + width / 2.0,
            origin_y + at.1 as f32 + height / 2.0,
        );
        let by = (60.0, 40.0);
        let to = slint::LogicalPosition::new(from.x + by.0, from.y + by.1);

        window.window().dispatch_event(WindowEvent::PointerMoved { position: from });
        window.window().dispatch_event(WindowEvent::PointerPressed {
            position: from,
            button: PointerEventButton::Left,
        });
        window.window().dispatch_event(WindowEvent::PointerMoved { position: to });

        // It follows the pointer while the gesture is running, which is the
        // half a person sees: a drag that only moved the node on release
        // would look like a drag that was doing nothing.
        let drawn = testing::ElementHandle::find_by_element_id(&window, "NodeShape::moving")
            .next()
            .expect("a node draws its parts in a container that the drag moves");
        let followed = drawn.absolute_position();
        assert!(
            (followed.x - (origin_x + at.0 as f32 + by.0)).abs() < 1.0
                && (followed.y - (origin_y + at.1 as f32 + by.1)).abs() < 1.0,
            "mid-drag the node is drawn at {followed:?}, not {:?}",
            (origin_x + at.0 as f32 + by.0, origin_y + at.1 as f32 + by.1)
        );

        window.window().dispatch_event(WindowEvent::PointerReleased {
            position: to,
            button: PointerEventButton::Left,
        });

        let said = moved.get().expect("a drag should report where the node went");
        assert!(
            (said.0 - by.0).abs() < 1.0 && (said.1 - by.1).abs() < 1.0,
            "a node dragged by {by:?} reported {said:?}"
        );
    }
}
