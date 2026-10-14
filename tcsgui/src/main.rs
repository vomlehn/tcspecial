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
mod history;
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
mod codegen;
mod nodes;
mod style;
mod subdiagram;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use slint::{ModelRc, VecModel};

use document::Document;
use editor::Editor;
use pane::{
    confine, dividers_for, edges_of, is_payload, pane_of, percents_for, size_of, Dividers,
    Pane, GRID, WORKING_WIDTH,
};
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

/// How far one divider may be dragged, given what is already on the canvas.
///
/// Two rules, and the narrower of the two wins. No pane may be squeezed below
/// a twentieth of the working pane, which is what was asked for and is in
/// `settings.rs` as the percentages that are allowed -- so a divider also
/// stops a twentieth short of its neighbour. And a divider does not pass a
/// node: it stops against the right edge of the rightmost node of the pane on
/// its left and against the left edge of the leftmost node of the pane on its
/// right.
///
/// The second rule is what keeps the panes honest without moving anything. The
/// alternative was to let a divider go where it liked and shunt the nodes it
/// passed, which is a gesture that quietly rearranges a diagram -- and one
/// that would have to carry every node it moved through the history to be
/// undoable. A line that stops against a node says what it is doing while it
/// does it.
///
/// A node already on the wrong side of a divider holds nothing: it is a node a
/// file put there, it comes home when it is next dragged, and letting it pin
/// the divider would mean one such node froze the control.
fn divider_range(document: &Document, dividers: Dividers, which: Pane) -> (i32, i32) {
    // Which panes this divider is between, and the room its neighbours keep.
    let step = WORKING_WIDTH * settings::LEAST / 100;
    let (left, right, mut low, mut high) = match which {
        // The moc divider: between the moc pane and the vehicle's.
        Pane::Moc => (Pane::Moc, Pane::Vehicle, step, dividers.vehicle - step),
        // And the vehicle divider, between the vehicle's and the payloads'.
        _ => (
            Pane::Vehicle,
            Pane::Payloads,
            dividers.moc + step,
            WORKING_WIDTH - step,
        ),
    };

    let (_, here) = edges_of(left, dividers);
    for node in &document.nodes {
        let (width, _) = size_of(&node.node_type);
        let pane = pane_of(&node.node_type);
        let edge = node.at.0 + width as i32;
        if pane == right {
            if node.at.0 >= here {
                high = high.min(node.at.0);
            }
        } else if pane == left && edge <= here {
            low = low.max(edge);
        }
    }
    (low, high.max(low))
}

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
    /// Where the two dividers between the three working panes sit, in the
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
    divider: Rc<std::cell::Cell<Dividers>>,
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
    /// The end of a link in somebody's hand, if any: see `Drawing::holding`.
    ///
    /// The one piece of a gesture the drawing needs to know about, because a
    /// line being dragged away by its end is drawn nowhere until the hand puts
    /// it down. Shared with the gesture's own handlers, as `chosen` is.
    drawing: Rc<RefCell<Option<Drawing>>>,
    /// The nodes whose attributes are open, by the name a link would use.
    ///
    /// One window per node and several at once, which is what they are for: a
    /// panel showing whichever node was last pressed made comparing two
    /// sensors on one bus a matter of pressing, reading, pressing and
    /// remembering. Each stays until it is closed.
    attributes: BTreeMap<String, NodeWindow>,
    /// The offer to save on the way out, while it is being made.
    ///
    /// Held so that pressing Exit twice does not stack two of them, and so
    /// that answering it can take it away again.
    leaving: Option<LeaveWindow>,
    /// The window that says what went wrong, and what it last said.
    ///
    /// One window, re-pointed: two of them side by side would be two copies of
    /// one sentence. What it last said is kept so that an error is shown once
    /// -- every edit redraws every window, and a window that popped up again
    /// on each of them would be a window nobody could type past.
    alert: Option<AlertWindow>,
    alerted: Rc<RefCell<String>>,
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
    /// What went wrong, said where it cannot be missed.
    ///
    /// Once per thing that goes wrong, and that is what `alerted` is for:
    /// every edit redraws every window, so a window shown on each of them
    /// would be one nobody could type past. A trouble that is still the same
    /// trouble has already been said.
    ///
    /// The red line in the bar stays as well, and the two are not the same
    /// thing: the line is the state -- "saved", or what is wrong with the last
    /// save -- and this is the moment it went wrong. An error nobody sees is
    /// an error that gets worse: a full device stays full while every edit
    /// after it is one more the files do not have.
    fn alert(&self, editor: &Editor) {
        let said = editor.trouble().unwrap_or_default().to_string();
        if said == *self.alerted.borrow() {
            return;
        }
        *self.alerted.borrow_mut() = said.clone();
        if said.is_empty() {
            return;
        }

        let Some(window) = &self.alert else { return };
        window.set_trouble(said.into());
        if let Err(e) = window.show() {
            // Nowhere to say it but the terminal, the thing that says things
            // having failed.
            eprintln!("tcsgui: {e}");
        }
    }

    /// The lines, redrawn, and nothing else.
    ///
    /// For the moment a link's end is taken hold of: the line comes off the
    /// drawing while the hand is moving, and the nodes must not be touched.
    /// Rebuilding them would destroy the element holding the pointer, which
    /// takes the rest of the gesture with it -- the fault this window has made
    /// once already, when the same press that chose a node rebuilt the canvas
    /// under it.
    fn relink(&self, editor: &Editor) {
        let held = self
            .drawing
            .borrow()
            .as_ref()
            .and_then(|drawing| drawing.holding.clone());
        if let Some(top) = self.top.upgrade() {
            let (_, links) = views_of(editor, &None, held.as_ref());
            top.set_links(ModelRc::from(Rc::new(VecModel::from(links))));
        }
        for (name, window) in &self.open {
            let home = Some(name.clone());
            let (_, links) = views_of(editor, &home, held.as_ref());
            window.set_links(ModelRc::from(Rc::new(VecModel::from(links))));
        }
    }

    fn refresh(&self, editor: &Editor) {
        self.alert(editor);
        let chosen = self.chosen.borrow();
        let divider = self.divider.get();
        // The link whose end is in somebody's hand, which is drawn nowhere:
        // a line that stayed put while its end was being dragged away would
        // be a line saying the gesture had not started.
        let held = self
            .drawing
            .borrow()
            .as_ref()
            .and_then(|drawing| drawing.holding.clone());
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
            put(
                &top,
                editor,
                &None,
                chosen.get("").map(|s| s.as_str()),
                divider,
                held.as_ref(),
            );
        }
        for (name, window) in &self.open {
            put_sub(
                window,
                editor,
                name,
                chosen.get(name).map(|s| s.as_str()),
                divider,
                held.as_ref(),
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
    ($window:expr, $described:expr) => {{
        let window = $window;
        let described = $described;
        window.set_moc_palette(palette_of(Pane::Moc, described));
        window.set_vehicle_palette(palette_of(Pane::Vehicle, described));
        window.set_payload_palette(palette_of(Pane::Payloads, described));
        window.set_working_width(WORKING_WIDTH as f32);
        // How long the sheet is: one length for all three panes, which is what
        // makes them scroll together. See pane::WORKING_HEIGHT.
        window.set_working_height(pane::WORKING_HEIGHT as f32);
        // What each pane is painted. Here with the palette rather than in the
        // markup, because which ground belongs to which pane is a rule with
        // tests on it: see style::ground_of.
        window.set_moc_ground(colour(&ground_of(style::Ground::Moc)));
        window.set_vehicle_ground(colour(&ground_of(style::Ground::Vehicle)));
        window.set_payload_ground(colour(&ground_of(style::Ground::Payloads)));
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
             the menu. It is written as YAML, so its name has to end in \".yaml\""
        );
        std::process::exit(2);
    });
    // The simulator's configuration, where one was named. A second argument
    // and not a name derived from the first: the shipped sets call it after
    // the configuration with `sim` on the end, and nothing requires that.
    //
    // Refused here rather than reported in the window when the name is not a
    // YAML one: it is the command line that is wrong, the window would be
    // showing a session with half of what was asked for, and the remedy is to
    // type the name again. See `sim::must_be_yaml`.
    let simulator = std::env::args().nth(2);
    if let Some(simulator) = &simulator {
        if let Err(why) = sim::must_be_yaml(std::path::Path::new(simulator)) {
            eprintln!("tcsgui: {why}");
            std::process::exit(2);
        }
    }

    // How the canvas was divided when the editor was last used, which is a
    // preference and not part of the configuration: see settings.rs. Reading
    // it is also what makes the directory, so it exists from the first run
    // whether or not anything has been saved into it.
    //
    // Before the configuration, because where the dividers are decides which
    // pane each node belongs in, and a node the file draws in the wrong one is
    // brought into its own as the file is read.
    let settings = settings::load();
    let divider = Rc::new(std::cell::Cell::new(dividers_for(
        settings.moc_percent,
        settings.vehicle_percent,
    )));

    // The histories of sessions nobody is coming back to: a week old, or whose
    // configuration is no longer there. Once, at startup, before anything is
    // opened -- see `crate::history::prune`.
    let gone = history::prune(std::time::SystemTime::now());
    if gone > 0 {
        eprintln!("tcsgui: {gone} old session histories were removed");
    }

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
    furnish!(&ui, descriptions());

    let views = Rc::new(RefCell::new(Views {
        top: ui.as_weak(),
        open: BTreeMap::new(),
        chosen: Rc::new(RefCell::new(BTreeMap::new())),
        drawing: Rc::new(RefCell::new(None)),
        divider: divider.clone(),
        group: None,
        parameters: None,
        attributes: BTreeMap::new(),
        leaving: None,
        alert: None,
        alerted: Rc::new(RefCell::new(String::new())),
        sim_groups: None,
        showing_sim_group: Rc::new(RefCell::new(String::new())),
        showing: Rc::new(RefCell::new(String::new())),
    }));
    alerting(&views);
    // Which node the panel is showing, per window.
    //
    // Deliberately not in the document: what somebody is looking at is not
    // something the vehicle flies, and an edit for it would make choosing a
    // node undoable, which is not what undo is for.
    let chosen: Rc<RefCell<BTreeMap<String, String>>> = Rc::new(RefCell::new(BTreeMap::new()));

    // Which node a link is being drawn from, while the gesture is running.
    // Nothing is recorded until it finishes on another node, so a drag that
    // ends nowhere leaves no trace.
    let drawing: Rc<RefCell<Option<Drawing>>> = Rc::new(RefCell::new(None));

    views.borrow().refresh(&editor.borrow());

    views.borrow_mut().chosen = chosen.clone();
    // The same half-drawn gesture the handlers hold, so the canvas can leave
    // the line being dragged off the drawing: see Views::drawing.
    views.borrow_mut().drawing = drawing.clone();
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
            furnish!(&window, descriptions());
            wire_sub(&window, &editor, &views, &drawing, &chosen, &name);
            put_sub(
                &window,
                &editor.borrow(),
                &name,
                None,
                views.borrow().divider.get(),
                None,
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
    let mut kept = false;
    if editor.borrow().unsaved() {
        editor.borrow_mut().save();
        match editor.borrow().trouble() {
            None => eprintln!("tcsgui: saved {path} on the way out"),
            Some(why) => {
                // The one case where the history is the only copy of a
                // session's work, so it is the one case where it is kept:
                // this exit is tidy as a gesture and not as an outcome.
                kept = true;
                eprintln!(
                    "tcsgui: {path} could not be saved: {why}\n\
                     tcsgui: the work of this session is in {}",
                    editor
                        .borrow()
                        .journal_path()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "no history, there being nowhere to \
                                             keep one"
                            .to_string())
                );
            }
        }
    }

    // And the history goes, this being an exit that was asked for.
    //
    // What is kept is the history of a session that ended *badly* -- a crash,
    // a kill, a machine that went down -- because that is the undo somebody
    // will want when they come back. A session that ended tidily has no
    // further use for its own, and a directory that only ever grows is a
    // directory somebody has to tidy. See `crate::history`.
    if !kept {
        editor.borrow().forget_history();
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
            drawing: &Rc<RefCell<Option<Drawing>>>,
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
                // A weak handle to the window the press came from: the
                // placement depends on what that window is showing, and
                // scrolling it afterwards is half of what placing a node
                // means. Weak because the closure outlives nothing and the
                // window owns the closure.
                let window_for_place = window.as_weak();
                window.on_place_node(move |called, node_type, sets| {
                    // What the window is showing, which is what decides where
                    // a new node goes: as high as it fits *in view*. The
                    // window is asked rather than told, because the person
                    // scrolling is the one who moved it.
                    let visible = (
                        (-window_for_place
                            .upgrade()
                            .map(|w| w.get_canvas_offset())
                            .unwrap_or(0.0))
                        .round() as i32,
                        window_for_place
                            .upgrade()
                            .map(|w| w.get_canvas_view_height())
                            .unwrap_or(0.0)
                            .round() as i32,
                    );
                    let at = {
                        let mut editor = editor.borrow_mut();
                        // Named after what was pressed rather than after the
                        // type: a described node is "mean" and not
                        // "compute-3", which is what a reader of the
                        // configuration will be looking for.
                        let name = fresh_name(editor.document(), &called, &home);
                        let at = next_place(
                            editor.document(),
                            &home,
                            &node_type,
                            views.borrow().divider.get(),
                            visible,
                        );
                        // Two edits: a node exists, and then it is somewhere.
                        // Separate so that undoing a drag leaves the node
                        // rather than removing it.
                        editor.edit(Edit::AddNode {
                            name: name.clone(),
                            node_type: node_type.to_string(),
                            home: home.clone(),
                        });
                        // What the description answers for it, before it is
                        // placed: a compute node from the `mean` entry is a
                        // mean from the moment it exists, so the panel asks
                        // for a mean's attributes and the checking holds it to
                        // a mean's rules.
                        for (attribute, value) in stated(&sets) {
                            editor.edit(Edit::SetAttribute {
                                node: Qualified::of(&name, &home).absolute(),
                                attribute,
                                was: None,
                                now: Some(value),
                            });
                        }
                        editor.edit(Edit::MoveNode {
                            node: Qualified::of(&name, &home).absolute(),
                            was: (0, 0),
                            now: at,
                        });
                        at
                    };
                    views.borrow().refresh(&editor.borrow());
                    // And the sheet is scrolled so the new node is at the top
                    // of the view. It was placed where it fits, which may be
                    // above or below what the window was showing; a node
                    // placed out of sight is a press that did nothing as far
                    // as the person who made it can tell.
                    if let Some(window) = window_for_place.upgrade() {
                        window.set_canvas_offset(-(at.1 as f32));
                    }
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
                        // The *gesture* and not the position it works out to.
                        // Those are different questions, and asking the second
                        // one lost a node's first click: a node whose position
                        // is not on the grid, or is outside the pane its type
                        // belongs to -- a file somebody hand-edited, a
                        // configuration from a build that drew the panes
                        // differently -- resolves to somewhere other than
                        // where it is, so `now != was` and a click on it was
                        // read as a drag. It moved instead of opening, and the
                        // second click opened it.
                        if dx.round() as i32 == 0 && dy.round() as i32 == 0 {
                            drop(editor);
                            show_node(&editor_for_click, &views, &absolute);
                            return;
                        }
                        // A drag that ends where it started: no edit, and no
                        // window either. Dragging a node against a divider and
                        // back is a gesture that changed nothing, and an entry
                        // in the history for it would be an undo that does
                        // nothing visible.
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
                // The configuration as Rust, written where the section says.
                // Asked for rather than done on every save: see
                // `Editor::write_rust_code`.
                let editor = editor.clone();
                let views = views.clone();
                window.on_generate_rust(move || {
                    editor.borrow_mut().write_rust_code();
                    views.borrow().refresh(&editor.borrow());
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
                // The groups, from the File menu. Which group is chosen is
                // the window's own business now: a list of what a
                // configuration happens to contain is data, and a control
                // holding data belongs with the form that edits it.
                let editor = editor.clone();
                let views = views.clone();
                window.on_edit_groups(move || {
                    show_group(&editor, &views);
                });
            }
            {
                // The way out. Where something has been changed that the
                // files do not have, the offer to write it comes first: see
                // leave.
                let editor = editor.clone();
                let views = views.clone();
                window.on_exit_requested(move || {
                    leave(&editor, &views);
                });
            }
            {
                // Moving either divider between the three working panes.
                //
                // Told once, when the gesture finishes, as a node's drag is:
                // the line follows the pointer in the window and this is what
                // it settles on. Not an edit -- there is nothing to undo,
                // because the configuration has not changed -- so it is
                // written to the settings instead of to the history.
                {
                    let editor = editor.clone();
                    let views = views.clone();
                    window.on_moc_divider_moved(move |dx: f32| {
                        drag_divider(&editor, &views, Pane::Moc, dx);
                    });
                }
                {
                    let editor = editor.clone();
                    let views = views.clone();
                    window.on_vehicle_divider_moved(move |dx: f32| {
                        drag_divider(&editor, &views, Pane::Vehicle, dx);
                    });
                }
            }
            {
                // Drawing a link: the terminal pressed is remembered, and the
                // terminal released over is the other end.
                let drawing = drawing.clone();
                let home = home.clone();
                let editor_for_press = editor.clone();
                let views_for_press = views.clone();
                window.on_link_started(move |name, terminal| {
                    let at = Drawing {
                        node: Qualified::of(&name, &home).absolute(),
                        terminal: terminal.to_string(),
                        holding: None,
                    };
                    // A terminal with one link on it is a line being picked up
                    // by its end; one with none, or with several, is a line
                    // about to be drawn. See Drawing::holding.
                    let sink = tcslibgs::graph::terminals::stream_of(&at.terminal)
                        .map(|(sink, _)| sink)
                        .unwrap_or(false);
                    let held = {
                        let editor = editor_for_press.borrow();
                        let theirs = attached(editor.document(), &at, sink);
                        match theirs.as_slice() {
                            [one] => Some(one.clone()),
                            _ => None,
                        }
                    };
                    *drawing.borrow_mut() = Some(Drawing { holding: held, ..at });
                    // Taken off the drawing while the gesture runs, so the
                    // line follows the hand rather than staying where it was.
                    // The lines only: rebuilding the nodes would destroy the
                    // element holding the pointer and the rest of the gesture
                    // with it. See Views::relink.
                    views_for_press
                        .borrow()
                        .relink(&editor_for_press.borrow());
                });
            }
            {
                let editor = editor.clone();
                let views = views.clone();
                let drawing = drawing.clone();
                let home = home.clone();
                window.on_link_finished(move |x, y| {
                    let from = match drawing.borrow_mut().take() {
                        Some(from) => from,
                        None => return,
                    };
                    {
                        let mut editor = editor.borrow_mut();
                        // A line that was picked up by its end, or one being
                        // drawn: two gestures that look the same and do
                        // different things.
                        let what = match &from.holding {
                            Some(held) => {
                                dropped(editor.document(), &home, held, &from, (x, y))
                            }
                            None => joined(editor.document(), &home, &from, (x, y))
                                .map(|link| Edit::AddLink { link }),
                        };
                        if let Some(edit) = what {
                            editor.edit(edit);
                        }
                    }
                    // Always, now: a gesture that changed nothing still has a
                    // line to put back, the held one having been taken off the
                    // drawing while the hand was moving.
                    views.borrow().refresh(&editor.borrow());
                });
            }
            {
                // Choosing a node: the outline follows, and nothing is
                // recorded. What the node says about itself is a window of
                // its own, opened by a click -- see on_node_moved -- so a
                // press has only the outline to change.
                let views = views.clone();
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
    drawing: &Rc<RefCell<Option<Drawing>>>,
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
    dividers: Dividers,
    held: Option<&Link>,
) {
    ui.set_moc_divider(dividers.moc as f32);
    ui.set_vehicle_divider(dividers.vehicle as f32);
    ui.set_has_simulator(editor.document().simulator.is_some());
    let (nodes, links) = views_of(editor, home, held);
    ui.set_selected(chosen.unwrap_or_default().into());
    ui.set_nodes(ModelRc::from(Rc::new(VecModel::from(nodes))));
    ui.set_links(ModelRc::from(Rc::new(VecModel::from(links))));
    ui.set_undo_depth(editor.undo_depth() as i32);
    ui.set_redo_depth(editor.journal().redoable() as i32);
    ui.set_undo_label(label(editor.journal().next_undo()));
    ui.set_redo_label(label(editor.journal().next_redo()));
    ui.set_trouble(editor.trouble().unwrap_or_default().into());
    ui.set_said(editor.said().unwrap_or_default().into());
}

/// The same, for one open subdiagram.
fn put_sub(
    window: &SubWindow,
    editor: &Editor,
    home: &str,
    chosen: Option<&str>,
    dividers: Dividers,
    held: Option<&Link>,
) {
    window.set_moc_divider(dividers.moc as f32);
    window.set_vehicle_divider(dividers.vehicle as f32);
    window.set_has_simulator(editor.document().simulator.is_some());
    let home = Some(home.to_string());
    let (nodes, links) = views_of(editor, &home, held);
    window.set_selected(chosen.unwrap_or_default().into());
    window.set_nodes(ModelRc::from(Rc::new(VecModel::from(nodes))));
    window.set_links(ModelRc::from(Rc::new(VecModel::from(links))));
    window.set_undo_depth(editor.undo_depth() as i32);
    window.set_redo_depth(editor.journal().redoable() as i32);
    window.set_undo_label(label(editor.journal().next_undo()));
    window.set_redo_label(label(editor.journal().next_redo()));
    window.set_trouble(editor.trouble().unwrap_or_default().into());
    window.set_said(editor.said().unwrap_or_default().into());
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

/// Leave, offering first to write anything the files do not have.
///
/// Normally there is nothing to offer: every edit is written as it is made, so
/// the files are already the configuration in the window, and this quits. What
/// it catches is the session where a write failed -- a full device, a
/// directory made read-only -- because leaving a session's work in a process
/// that is ending is the one way this editor could lose anything.
///
/// Settle one divider where a gesture left it.
///
/// `which` is the pane on the divider's left -- `Pane::Moc` for the line
/// between the moc pane and the vehicle's, `Pane::Vehicle` for the one between
/// the vehicle's and the payloads'. Both gestures are the same work, and
/// writing it twice is two places for the clamping to be got wrong in.
fn drag_divider(
    editor: &Rc<RefCell<Editor>>,
    views: &Rc<RefCell<Views>>,
    which: Pane,
    dx: f32,
) {
    let dividers = views.borrow().divider.clone();
    {
        let editor = editor.borrow();
        let was = dividers.get();
        let (low, high) = divider_range(editor.document(), was, which);
        let from = if which == Pane::Moc { was.moc } else { was.vehicle };
        let now = snap((from + dx.round() as i32, 0)).0.clamp(low, high);
        if now == from {
            // It did not move, or it was already against what stops it.
            // Nothing to redraw and nothing to write: a settings file
            // rewritten on every press of a handle is a file written for
            // nothing.
            return;
        }
        dividers.set(if which == Pane::Moc {
            Dividers { moc: now, ..was }
        } else {
            Dividers { vehicle: now, ..was }
        });
    }

    let (moc_percent, vehicle_percent) = percents_for(dividers.get());
    settings::save(settings::Settings {
        moc_percent,
        vehicle_percent,
    });
    views.borrow().refresh(&editor.borrow());
}

/// The window that says what went wrong, made ready before anything does.
///
/// Made once and kept hidden, rather than made when it is needed: the moment
/// it is needed is the moment something has already failed, and a window that
/// could not be made then would leave nothing to say it with. Hidden and shown
/// again, so acknowledging one thing does not throw away the means of saying
/// the next.
///
/// Without a window there is still the red line in the bar and the error
/// output, so this failing is a quieter editor and not a broken one.
fn alerting(views: &Rc<RefCell<Views>>) {
    let window = match AlertWindow::new() {
        Ok(window) => window,
        Err(e) => {
            eprintln!("tcsgui: errors cannot be shown in a window: {e}");
            return;
        }
    };
    {
        let views = views.clone();
        window.on_acknowledged(move || {
            // Hidden and kept, not dropped: see above.
            let window = views.borrow().alert.as_ref().map(|window| window.as_weak());
            if let Some(window) = window.and_then(|window| window.upgrade()) {
                let _ = window.hide();
            }
        });
    }
    views.borrow_mut().alert = Some(window);
}

/// The offer and not the saving: somebody who has been told the device is full
/// may want to leave and deal with it, and a program that insisted on writing
/// first would be a program they cannot get out of.
fn leave(editor: &Rc<RefCell<Editor>>, views: &Rc<RefCell<Views>>) {
    if !editor.borrow().unsaved() {
        slint::quit_event_loop().ok();
        return;
    }

    if views.borrow().leaving.is_none() {
        let window = match LeaveWindow::new() {
            Ok(window) => window,
            Err(e) => {
                // Nothing to ask with. The work is still on the disk as of
                // the last write that succeeded, and refusing to quit would
                // trap somebody in an editor they asked to leave.
                eprintln!("tcsgui: the offer to save cannot be shown: {e}");
                slint::quit_event_loop().ok();
                return;
            }
        };
        {
            let editor = editor.clone();
            let views = views.clone();
            window.on_saved(move || {
                editor.borrow_mut().save();
                match editor.borrow().trouble() {
                    None => {
                        slint::quit_event_loop().ok();
                    }
                    // Still not written. Said again, and the way out left
                    // open: the one thing not to do is quit quietly.
                    Some(why) => {
                        eprintln!("tcsgui: still not saved: {why}");
                        views.borrow().refresh(&editor.borrow());
                    }
                }
            });
        }
        {
            let views = views.clone();
            window.on_left(move || {
                let window = views.borrow_mut().leaving.take();
                if let Some(window) = window {
                    let _ = window.hide();
                }
                slint::quit_event_loop().ok();
            });
        }
        {
            let views = views.clone();
            window.on_stayed(move || {
                let window = views.borrow_mut().leaving.take();
                if let Some(window) = window {
                    let _ = window.hide();
                }
            });
        }
        views.borrow_mut().leaving = Some(window);
    }

    let window = views.borrow().leaving.as_ref().map(|window| window.as_weak());
    if let Some(window) = window.and_then(|window| window.upgrade()) {
        window.set_trouble(editor.borrow().trouble().unwrap_or_default().into());
        if let Err(e) = window.show() {
            eprintln!("tcsgui: the offer to save cannot be shown: {e}");
            slint::quit_event_loop().ok();
        }
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

    // The choices before the choice: a ComboBox given a value that is not in
    // its model clears the selection, and says so on the error output.
    window.set_choices(ModelRc::from(Rc::new(VecModel::from(
        document
            .groups
            .keys()
            .map(|name| slint::SharedString::from(name.clone()))
            .collect::<Vec<_>>(),
    ))));
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

/// Show `group` in the group window, making the window if there is not one.
///
/// Made once and re-pointed afterwards, so choosing a second group does not
/// leave a window per group behind. The handlers are wired when the window is
/// made and read the group out of `Views::showing` as they run, which is what
/// lets the same handlers go on working after the window is re-pointed -- or
/// after the group it is showing is renamed from inside it.
fn show_group(editor: &Rc<RefCell<Editor>>, views: &Rc<RefCell<Views>>) {
    // Whichever group was last looked at, or the first the configuration
    // declares: a window that opened on an invitation to choose would make
    // every visit two presses when there is only one group, which is what
    // most configurations have.
    {
        let views = views.borrow();
        let mut showing = views.showing.borrow_mut();
        let known = editor.borrow().document().groups.contains_key(&*showing);
        if !known {
            *showing = editor
                .borrow()
                .document()
                .groups
                .keys()
                .next()
                .cloned()
                .unwrap_or_default();
        }
    }

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
        let group = views.borrow().showing.borrow().clone();
        put_group(&window, &editor.borrow(), &group);
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
        // Choosing which group the window is showing. Nothing is recorded:
        // what somebody is looking at is not something the vehicle flies.
        let editor = editor.clone();
        let views = views.clone();
        let showing = views.borrow().showing.clone();
        window.on_choose(move |chosen| {
            *showing.borrow_mut() = chosen.to_string();
            views.borrow().refresh(&editor.borrow());
        });
    }
    {
        // A new group, with an interval, because a group without one is a
        // group that reads nothing: a value to change is a better start than
        // a blank to fill in.
        let editor = editor.clone();
        let views = views.clone();
        let showing = views.borrow().showing.clone();
        window.on_add(move || {
            let name = {
                let mut editor = editor.borrow_mut();
                let name = fresh_group(editor.document());
                editor.edit(Edit::AddGroup {
                    name: name.clone(),
                    interval: FIRST_INTERVAL.to_string(),
                });
                name
            };
            *showing.borrow_mut() = name;
            views.borrow().refresh(&editor.borrow());
        });
    }
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

    // What this node is asked for. The description tree first, where it
    // describes this node: a compute node whose function is `mean` is a mean,
    // and a mean is asked for a mean's attributes rather than for every
    // attribute any computation has. See nodes::matching.
    //
    // Otherwise the type's own list, which is what every node was asked for
    // before there was a tree and what a payload is asked for still.
    let declared: Vec<(String, bool)> =
        match nodes::matching(descriptions(), &node.node_type, &node.attributes) {
            Some(described) => described.asked(),
            None => NodeType::ALL
                .iter()
                .find(|t| t.spelling() == node.node_type)
                .map(|t| {
                    t.attributes()
                        .iter()
                        .map(|(name, required)| ((*name).to_string(), *required))
                        .collect()
                })
                .unwrap_or_default(),
        };

    let mut fields: Vec<FieldView> = declared
        .iter()
        .map(|(name, required)| {
            let value = node.attributes.get(name).cloned().unwrap_or_default();
            FieldView {
                name: name.clone().into(),
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
fn views_of(
    editor: &Editor,
    home: &Home,
    held: Option<&Link>,
) -> (Vec<NodeView>, Vec<LinkView>) {
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
        // Not the one whose end is in somebody's hand: a line that stayed put
        // while its end was being dragged away would say the gesture had not
        // started. It comes back where the hand put it down, or where it was.
        .filter(|link| !held.is_some_and(|held| held.same_connection(link)))
        .map(|link| {
            let ends = ends_of(document, link, home);

            // The label says which input it arrives at, and names the far
            // node where that node is on another page.
            let crossing = !here(&link.from) || !here(&link.to);
            //
            // A terminal is not one of those, and is not labelled: it is drawn
            // as the corner the line is attached to and as the arrow in it,
            // which says the same thing in the place a reader is already
            // looking. A label reading "from_payload" beside every line would
            // be the name of the thing the picture is showing.
            let named_input = link
                .input
                .as_deref()
                .filter(|input| tcslibgs::graph::terminals::stream_of(input).is_none());
            let label = match (named_input, crossing) {
                (Some(input), false) => input.to_string(),
                (Some(input), true) => format!("{} . {input}", far_end(link, home)),
                (None, true) => far_end(link, home),
                (None, false) => String::new(),
            };

            // The label goes at the middle of the route's longest run, which
            // is on the line wherever the line had to go.
            let (label_x, label_y) = route::label_at(ends, &obstacles);

            LinkView {
                // The route and the arrow, worked out where they can be
                // tested: see route.rs.
                route: route::route(ends, &obstacles).into(),
                head: route::arrow(ends).into(),
                marker: route::marker(ends, &obstacles).into(),
                label_x,
                label_y,
                wide: named_input.is_some(),
                label: label.into(),
                comment: link.comment.clone().unwrap_or_default().into(),
            }
        })
        .collect();

    (nodes, links)
}

/// Where a link's line starts and finishes, and which way it points at each
/// end.
///
/// Each end at the terminal it is attached to, which is where the line has to
/// start and finish now that a link is drawn from a terminal: a line that left
/// the middle of a node's right edge while its arrow sat in the lower left
/// corner would be two statements about one link. See [`style::attachment`].
///
/// The direction comes with the point, and that is the other half of it. A
/// groundward link leaves a node's left side and arrives at the next one's
/// right side, so "out of the source, across, into the target" is no longer
/// always left to right -- see [`route::Ends`].
///
/// One function because two callers must agree: the canvas draws this, and the
/// test that no line runs through a node asks about the same geometry. Two
/// copies of it would be a test of a drawing nobody sees.
fn ends_of(document: &Document, link: &Link, home: &Home) -> route::Ends {
    let here = |name: &str| &Qualified::parse(name).in_ == home;
    let from = document.node(&link.from);
    let to = document.node(&link.to);

    let joined = |node: &document::Node, named: Option<&str>, sink: bool| {
        let terminal = style::terminal_for(&node.node_type, named, sink)?;
        style::attachment(&node.node_type, node.at, terminal)
    };
    // Which data sink the line arrives at, where the link does not say. The
    // one that pairs with the source it left: a link leaving `to_payload`
    // arrives at a `from_moc`, that being the same stream named at its other
    // end -- see tcslibgs::graph::terminals::answers. Without this the drawing
    // fell back to the groundward sink whatever the link carried, so a command
    // from the ground was drawn arriving at the corner the readings leave by.
    let pairing = |from: Option<&document::Node>, origin: Option<&str>| {
        let from = from?;
        let leaves = style::terminal_for(&from.node_type, origin, false)?;
        tcslibgs::graph::terminals::answers(leaves)
    };

    let ((x1, y1), leaves_right) = match from {
        Some(node) if here(&link.from) => joined(node, link.origin.as_deref(), false)
            .unwrap_or_else(|| {
                // A type this build does not know, drawn as a node with no
                // terminals: the middle of its right edge, where every link
                // used to leave.
                let (w, h) = size_of(&node.node_type);
                (
                    (
                        (node.at.0 + w as i32) as f32,
                        (node.at.1 + h as i32 / 2) as f32,
                    ),
                    true,
                )
            }),
        // Off this page: a stub at the left edge.
        _ => (
            (0.0, to.map(|n| n.at.1 as f32 + 24.0).unwrap_or(24.0)),
            true,
        ),
    };
    let ((x2, y2), arrives_right) = match to {
        Some(node) if here(&link.to) => {
            // What the link wrote, where that is a terminal this type has; the
            // terminal that pairs with the source otherwise. A computation's
            // own input -- `in0` -- is neither, and falls through to the
            // pairing one: a mean's three lines arrive where its readings do.
            let wanted = link
                .input
                .as_deref()
                .filter(|input| {
                    NodeType::ALL
                        .iter()
                        .find(|t| t.spelling() == node.node_type)
                        .is_some_and(|t| t.terminals().has(input))
                })
                .or_else(|| pairing(from, link.origin.as_deref()));
            joined(node, wanted, true).unwrap_or_else(|| {
                let (_, h) = size_of(&node.node_type);
                ((node.at.0 as f32, (node.at.1 + h as i32 / 2) as f32), true)
            })
        }
        // Off this page: a stub to the right of its source.
        _ => ((x1 + 72.0, y1), true),
    };

    route::Ends::new((x1, y1), (x2, y2), leaves_right, arrives_right)
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
/// Half a link gesture: the node a line was dragged from, and the terminal.
///
/// The terminal is what the gesture is about. A node has two of them per
/// stream and the one pressed says which stream the link carries and which way
/// -- so a gesture that remembered only the node would have thrown away the
/// thing it was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Drawing {
    node: String,
    terminal: String,
    /// The link this gesture took hold of, where the terminal had one.
    ///
    /// A terminal with a link on it is a line somebody can pick up by its end:
    /// dragging it away takes that end off, and where it is let go of decides
    /// what happens to the link -- see [`dropped`]. A terminal with none is a
    /// line about to be drawn, and one with several is as well, because
    /// nothing says which of them was meant.
    ///
    /// Which is the common case for a data source, a source being allowed to
    /// reach several sinks: so a second line out of one is drawn from the
    /// *sink* end, the gesture working either way about.
    holding: Option<Link>,
}

/// What becomes of a link whose end was dragged somewhere.
///
/// `held` is the link and `pressed` is the end that was taken hold of; `(x, y)`
/// is where the pointer was let go of. Four answers, which are the four things
/// a person can do with the end of a line:
///
/// put it back where it was, or anywhere that is not a terminal at all but is
/// still on a node: nothing changes. A gesture that has thought better of
/// itself leaves no trace, and neither does one that landed on a node's label;
///
/// drop it on another terminal that suits that end: the link is moved, in one
/// edit, so one undo puts it back;
///
/// drop it on a terminal that does not suit the end -- the wrong kind, the
/// wrong stream, its own far node, or one the same link already reaches:
/// nothing changes, as a line that cannot be drawn is not drawn;
///
/// and let go of it over no node at all: the link is deleted. Which is the
/// only way to delete one, and it is the gesture a person reaches for -- pull
/// the wire out and drop it on the floor.
fn dropped(
    document: &Document,
    home: &Home,
    held: &Link,
    pressed: &Drawing,
    (x, y): (f32, f32),
) -> Option<Edit> {
    let (sink_pressed, _) = tcslibgs::graph::terminals::stream_of(&pressed.terminal)?;

    // Over nothing: the link goes. A node's own box and not its terminals,
    // because letting go over a node is a gesture that missed and letting go
    // over the canvas is one that meant it.
    let Some(over) = terminal_under(document, home, (x, y), sink_pressed) else {
        return if node_under(document, home, (x, y)).is_none() {
            Some(Edit::RemoveLink { link: held.clone() })
        } else {
            None
        };
    };

    // Back where it started.
    if over.node == pressed.node && over.terminal == pressed.terminal {
        return None;
    }

    // The far end stays where it is and the pressed end moves.
    let mut now = held.clone();
    if sink_pressed {
        now.to = over.node.clone();
        now.input = arriving_at(document, &over.node, &over.terminal);
    } else {
        now.from = over.node.clone();
        now.origin = leaving_by(&over);
    }

    let source = Drawing {
        node: now.from.clone(),
        terminal: now.origin.clone().unwrap_or_default(),
        holding: None,
    };
    let sink = Drawing {
        node: now.to.clone(),
        terminal: now
            .input
            .clone()
            .filter(|input| tcslibgs::graph::terminals::stream_of(input).is_some())
            .or_else(|| {
                style::terminal_for(
                    &document.node(&now.to)?.node_type.clone(),
                    now.input.as_deref(),
                    true,
                )
                .map(str::to_string)
            })
            .unwrap_or_default(),
        holding: None,
    };
    if source.node == sink.node {
        return None;
    }
    if !tcslibgs::graph::terminals::pairs_with(&source.terminal, &sink.terminal) {
        return None;
    }
    if document
        .links
        .iter()
        .any(|theirs| theirs.same_connection(&now) && !theirs.same_connection(held))
    {
        return None;
    }

    Some(Edit::MoveLink {
        was: held.clone(),
        now,
    })
}

/// Which node is under this point, where one is.
///
/// For telling "let go of over a node" from "let go of over the canvas", which
/// are two different things: the first is a gesture that missed a terminal and
/// the second is one that meant to put the line down.
fn node_under(document: &Document, home: &Home, (x, y): (f32, f32)) -> Option<String> {
    document
        .nodes_in(home)
        .into_iter()
        .find(|node| {
            let (width, height) = size_of(&node.node_type);
            x >= node.at.0 as f32
                && x <= (node.at.0 + width as i32) as f32
                && y >= node.at.1 as f32
                && y <= (node.at.1 + height as i32) as f32
        })
        .map(|node| node.name.clone())
}

/// The links attached to this terminal of this node.
///
/// Both ends are asked: a line may be picked up by either of them.
fn attached(document: &Document, at: &Drawing, sink: bool) -> Vec<Link> {
    document
        .links
        .iter()
        .filter(|link| {
            if sink {
                link.to == at.node
                    && style::terminal_for(
                        &document
                            .node(&link.to)
                            .map(|node| node.node_type.clone())
                            .unwrap_or_default(),
                        link.input.as_deref(),
                        true,
                    ) == Some(at.terminal.as_str())
            } else {
                link.from == at.node
                    && style::terminal_for(
                        &document
                            .node(&link.from)
                            .map(|node| node.node_type.clone())
                            .unwrap_or_default(),
                        link.origin.as_deref(),
                        false,
                    ) == Some(at.terminal.as_str())
            }
        })
        .cloned()
        .collect()
}

/// The link a gesture made, where it made one.
///
/// `pressed` is the terminal the line was drawn from and `(x, y)` is where the
/// pointer was let go of, in the canvas's own coordinates. `None` for every
/// gesture that is not a link, and there are four of those -- each refused
/// here, as the gesture, rather than drawn and then reported. An editor that
/// cannot make a bad link is worth more than one that explains it afterwards:
///
/// let go of over nothing, or over a terminal of the same kind as the one the
/// drag started at, which is how a gesture is abandoned;
///
/// let go of over the node it started on, which is a loop and not a link --
/// and releasing on the terminal you pressed is a gesture that changed its
/// mind;
///
/// let go of over a terminal that does not pair with the one it started at. A
/// `to_moc` may only be linked to a `from_payload` and a `to_payload` only to
/// a `from_moc`: that is one stream, named at each of its ends, and anything
/// else would carry one stream into the other. The rule is
/// `tcslibgs::graph::terminals::pairs_with`, which is also what the checking
/// refuses a written-out configuration for;
///
/// and a link that is already there, which would otherwise be a second line
/// on top of the first and two deliveries of every datum.
fn joined(
    document: &Document,
    home: &Home,
    pressed: &Drawing,
    (x, y): (f32, f32),
) -> Option<Link> {
    // Either way about. A link may be drawn from the data source that feeds
    // it or from the data sink that wants feeding -- the same link, and a
    // person drawing a diagram works in whichever direction they are reading
    // it. So which end was pressed decides what is being looked for under the
    // pointer, and the link is put back in its own order afterwards: a link
    // leaves a source and arrives at a sink however it was drawn.
    let (sink_pressed, _) = tcslibgs::graph::terminals::stream_of(&pressed.terminal)?;
    let dropped = terminal_under(document, home, (x, y), !sink_pressed)?;
    let (source, sink) = if sink_pressed {
        (&dropped, pressed)
    } else {
        (pressed, &dropped)
    };

    if source.node == sink.node {
        return None;
    }
    if !tcslibgs::graph::terminals::pairs_with(&source.terminal, &sink.terminal) {
        return None;
    }

    let link = Link::joining(
        &source.node,
        leaving_by(source).as_deref(),
        &sink.node,
        arriving_at(document, &sink.node, &sink.terminal).as_deref(),
    );
    if document.links.iter().any(|theirs| theirs.same_connection(&link)) {
        return None;
    }
    Some(link)
}

/// Which terminal of which node is at this point, where one is.
///
/// What the release of a link gesture lands on. `sink` says which kind is
/// being looked for: a link is let go of over a data sink, a data source being
/// where it was drawn from.
///
/// Only the nodes of this page, because only they are drawn on it: a point on
/// this canvas is not a point on another one, and a link to a node on another
/// page is drawn as a stub rather than as a box to aim at.
fn terminal_under(
    document: &Document,
    home: &Home,
    (x, y): (f32, f32),
    sink: bool,
) -> Option<Drawing> {
    for node in document.nodes_in(home) {
        let terminals = NodeType::ALL
            .iter()
            .find(|t| t.spelling() == node.node_type)?
            .terminals();
        let named = if sink {
            terminals.sinks
        } else {
            terminals.sources
        };
        for terminal in named {
            let Some((bx, by, width, height)) =
                style::terminal_box(&node.node_type, node.at, terminal)
            else {
                continue;
            };
            if x >= bx && x <= bx + width && y >= by && y <= by + height {
                return Some(Drawing {
                    node: node.name.clone(),
                    terminal: (*terminal).to_string(),
                    holding: None,
                });
            }
        }
    }
    None
}

/// What to record about the end a line arrives at.
///
/// Three answers, and the order is the order of authority:
///
/// a computation that declares its inputs gets the next free one. That is
/// what a mean is: `in0`, `in1`, `in2`, filled as the lines are drawn, and
/// the terminal the line was dropped on is not what distinguishes them;
///
/// a type with more than one data sink gets the terminal, because the file
/// has to say which -- the `null` node and nothing else today;
///
/// and everything else gets nothing, because there is nothing to say: a
/// payload has one way in, and the schema reads an unnamed end as that one.
/// Which is also what keeps the editor from writing a terminal into every
/// link of every file it saves.
fn arriving_at(document: &Document, node: &str, terminal: &str) -> Option<String> {
    if let Some(input) = free_input(document, node) {
        return Some(input);
    }
    must_name(document, node, true).then(|| terminal.to_string())
}

/// And about the end it leaves by, which is always the terminal pressed.
///
/// Always, because a link *is* an attribute of the node it leaves, named after
/// that terminal: `to_moc: k_fore` is where the link is written, so there is no
/// such thing as a link whose source terminal the file does not say.
fn leaving_by(from: &Drawing) -> Option<String> {
    Some(from.terminal.clone())
}

/// Whether a link at this end of this node has to name its terminal.
///
/// The type's own answer -- see [`NodeType::must_name`] -- and `false` for a
/// node that is not there or a type this build does not know: a name written
/// on a type nothing can describe would be a guess recorded in a file.
fn must_name(document: &Document, node: &str, sink: bool) -> bool {
    document
        .node(node)
        .and_then(|node| {
            NodeType::ALL
                .iter()
                .find(|t| t.spelling() == node.node_type)
        })
        .is_some_and(|t| t.must_name(sink))
}

/// Whether this node is a computation that declares its inputs.
///
/// Which is the one kind of destination whose arriving end is named by
/// something that is not a terminal: a mean's `in0`. See `arriving_at`.
#[cfg(test)]
fn declares_inputs(document: &Document, node: &str) -> bool {
    document
        .node(node)
        .is_some_and(|node| node.attributes.contains_key("inputs"))
}

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
        // Empty, with one exception the schema itself names: the beacon's
        // interval has an answer that is right until somebody has a reason,
        // where a socket has none. See `parameters::default_of`.
        let now = tcslibgs::graph::parameters::default_of(attribute)
            .unwrap_or_default()
            .to_string();
        editor.edit(Edit::SetTcspecial {
            attribute: (*attribute).to_string(),
            was: None,
            now: Some(now),
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
    dividers: Dividers,
    visible: (i32, i32),
) -> (i32, i32) {
    // Every box already on this page, whichever pane it is in: a node drawn
    // over another is unreadable wherever the two came from.
    let taken: Vec<(i32, i32, u32, u32)> = document
        .nodes_in(home)
        .iter()
        .map(|node| {
            let (width, height) = size_of(&node.node_type);
            (node.at.0, node.at.1, width, height)
        })
        .collect();

    snap(pane::place_in(&taken, node_type, dividers, visible))
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
/// -- and a flat list of fifteen shapes hides the pattern that would make
/// them learnable.
///
/// Three lists rather than one, because there are three working panes and a
/// shape's pane follows from the list it came from. A single list would leave
/// the rule to be found out by placing something and watching where it went.
fn palette_of(pane: Pane, described: &[nodes::Described]) -> ModelRc<PaletteEntry> {
    let mut entries: Vec<PaletteEntry> = Vec::new();
    let mut family = "";

    for t in NodeType::ALL
        .iter()
        .copied()
        .filter(|t| pane_of(t.spelling()) == pane)
    {
        // What the description tree says about this type, where it says
        // anything. A type the tree describes is offered as its descriptions
        // and not as itself: `compute` is one node type, and the tree turns it
        // into `mean`, `hysteresis`, `to_kelvin` and `process` -- four things
        // to place, each a compute node with its function already answered. A
        // bare `compute` would be a fifth entry that is refused until somebody
        // types the function in by hand.
        //
        // And a type the tree says nothing about is offered as itself, which
        // is what every type was before there was a tree and what they all are
        // again if the tree cannot be found.
        let theirs: Vec<&nodes::Described> = described
            .iter()
            .filter(|one| one.node_type == t.spelling())
            .collect();

        if t.family() != family {
            family = t.family();
            entries.push(PaletteEntry {
                heading: family.into(),
                node: view_of("", "", (0, 0), None, false, None),
                is_heading: true,
                sets: "".into(),
            });
        }

        if theirs.is_empty() {
            entries.push(PaletteEntry {
                heading: "".into(),
                node: view_of(t.spelling(), t.spelling(), (8, 2), None, false, None),
                is_heading: false,
                sets: "".into(),
            });
            continue;
        }
        for one in theirs {
            entries.push(PaletteEntry {
                heading: "".into(),
                node: view_of(&one.node, t.spelling(), (8, 2), None, false, None),
                is_heading: false,
                sets: written(&one.sets).into(),
            });
        }
    }
    ModelRc::from(Rc::new(VecModel::from(entries)))
}

/// Every description the editor found, read once.
///
/// Once because the tree is data on the disk and the same for every window:
/// re-reading it per window would mean two windows of one session could offer
/// different things. What is wrong with the tree goes to the error output, as a
/// preferences file that cannot be read does -- an editor that refused to start
/// because one description has a typo in it would be an editor a typo turns
/// off.
fn descriptions() -> &'static [nodes::Described] {
    static FOUND: std::sync::OnceLock<Vec<nodes::Described>> = std::sync::OnceLock::new();
    FOUND.get_or_init(|| {
        let (described, trouble) = nodes::described();
        for said in trouble {
            eprintln!("tcsgui: {said}");
        }
        described
    })
}

/// What a palette entry fills in, as the one string a callback can carry.
///
/// `name=value`, several of them separated by semicolons. Neither character
/// can appear in an attribute's name, and a value that held one would be a
/// value this could not carry -- so the pairs are the ones a *description*
/// states, which are the ones that pick a variant of a type: a function's
/// name, not a path or a program's arguments.
fn written(sets: &BTreeMap<String, String>) -> String {
    sets.iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<String>>()
        .join(";")
}

/// And back: what a placed node is given.
fn stated(sets: &str) -> Vec<(String, String)> {
    sets.split(';')
        .filter(|pair| !pair.trim().is_empty())
        .filter_map(|pair| pair.split_once('='))
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .collect()
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
    let found = NodeType::ALL.iter().find(|t| t.spelling() == node_type);
    let appearance = found.map(|t| t.appearance());
    let terminals = found.map(|t| t.terminals()).unwrap_or(style::Terminals {
        sinks: &[],
        sources: &[],
    });

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
        // The terminals this type has, each drawn in its own corner. A type
        // this build does not know has none: the shape already says it cannot
        // be placed, and four arrows on it would be four claims about a type
        // nothing here can describe.
        from_moc: terminal_view("from_moc", terminals, size),
        to_payload: terminal_view("to_payload", terminals, size),
        from_payload: terminal_view("from_payload", terminals, size),
        to_moc: terminal_view("to_moc", terminals, size),
        terminal_column: style::TERMINAL_COLUMN as f32,
    }
}

/// One terminal of a node, as the window wants it.
///
/// Whether this type has it and where it goes. The corner and the arrow are
/// `style`'s to say -- see [`style::corner_of`] -- so that the rule "the
/// vehicleward stream along the top going right, the groundward stream along
/// the bottom going left" is stated once and tested where it is stated.
fn terminal_view(
    terminal: &str,
    terminals: style::Terminals,
    size: (u32, u32),
) -> TerminalView {
    let corner = style::corner_of(terminal);
    let (x, y) = corner
        .map(|corner| style::terminal_at(corner, size))
        .unwrap_or_default();
    TerminalView {
        shown: terminals.has(terminal),
        x: x as f32,
        y: y as f32,
        rightward: corner.is_some_and(|corner| corner.rightward),
    }
}

/// The palette's colours.
///
/// Held here rather than in `style.rs` on purpose: that module holds what a
/// type *means* and the rules the looks obey, and this holds what the meanings
/// are painted. Keeping them apart is what lets the accessibility tests check
/// the rules without knowing a single hex value.
fn tint_of(tint: Tint) -> String {
    // Searched for rather than chosen by eye, and three things were asked of
    // the search at once: that every tint stay in the hue family of the pane
    // its nodes are drawn on, that it be dark enough against that pane to read
    // as a shape -- three to one, which is the threshold for something that is
    // not text -- and that, subject to those, the smallest perceived
    // difference between any two of the fifteen be as large as it can be,
    // taken in CIE L*a*b* under normal vision and under simulated protanopia,
    // deuteranopia and tritanopia at once.
    //
    // It is 14 CIE units, where two or three is "the same colour in two
    // printings" and ten is "clearly two colours". So a pair anybody can tell
    // apart is a pair everybody can tell apart. The test below checks it
    // against these very values, which is the point of its existing: tuning
    // one of these means re-running the search.
    //
    // Three units less than the fourteen-colour palette that came before, and
    // the three are the price of the families: a palette free to put its
    // colours anywhere spreads them further than one whose reds must stay red.
    // What the families buy is that a node's colour says which pane it belongs
    // in, which no amount of separation would have told anybody.
    match tint {
        // The ground link, on its pale blue pane.
        Tint::Azure => "#0080e8",
        // The vehicle's own graph, on pale green.
        Tint::Pine => "#206038",
        Tint::Olive => "#686808",
        Tint::Gold => "#a08800",
        Tint::Moss => "#609850",
        // What the vehicle carries, on pale blush: ten of them, which is why
        // this is the family the search had the least room in.
        Tint::Aubergine => "#481040",
        Tint::Oxblood => "#481820",
        Tint::Crimson => "#780008",
        Tint::Magenta => "#780870",
        Tint::Mulberry => "#683850",
        Tint::Rose => "#a82038",
        Tint::Plum => "#884078",
        Tint::Orchid => "#a848a0",
        Tint::Heather => "#906078",
        Tint::Clay => "#a86060",
    }
    .to_string()
}

/// What each working pane is painted.
///
/// Light and neutral, as asked: 15 CIE units apart from each other under every
/// one of the four ways of seeing, and none with a chroma above 14, so a pane
/// reads as a tinted sheet of paper rather than as a coloured panel. Each is
/// the light end of the hue family its nodes are drawn in -- see
/// `style::Tint` -- which is what makes a node's colour say where it belongs.
fn ground_of(ground: style::Ground) -> String {
    match ground {
        style::Ground::Moc => "#d4e6fe",
        style::Ground::Vehicle => "#eff5da",
        style::Ground::Payloads => "#ddced1",
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

    /// Where the dividers sit until somebody moves them.
    ///
    /// Most of what follows is about the panes rather than about moving the
    /// lines between them, so it is worked in the division the editor opens
    /// with.
    fn dividers() -> Dividers {
        dividers_for(settings::DEFAULT_MOC, settings::DEFAULT_VEHICLE)
    }

    /// Every row of one selection pane.
    /// A screenful at the top of the sheet, which is what a window shows when
    /// a configuration is opened: the top of the view and how much of it
    /// there is.
    const IN_VIEW: (i32, i32) = (0, 700);

    fn rows_of(pane: Pane) -> Vec<PaletteEntry> {
        rows_from(pane, descriptions())
    }

    /// The same, from a tree given rather than found.
    fn rows_from(pane: Pane, described: &[nodes::Described]) -> Vec<PaletteEntry> {
        let palette = palette_of(pane, described);
        (0..palette.row_count())
            .map(|i| palette.row_data(i).unwrap())
            .collect()
    }

    /// The three selection panes offer every type the editor can place, and
    /// each type in exactly one of them.
    ///
    /// A type missing from all three is a type nobody can place, which is a
    /// failure a person would find only by going looking for it. A type in two
    /// of them would be a type whose pane depends on which list it was pressed
    /// in, and the pane is what says what the node is allowed to be.
    #[test]
    fn the_selection_panes_offer_every_node_type_once() {
        let mut offered: Vec<String> = Vec::new();
        for pane in [Pane::Moc, Pane::Vehicle, Pane::Payloads] {
            let rows = rows_of(pane);

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
                    pane_of(t.spelling()),
                    pane,
                    "{spelling} is offered in the wrong selection pane"
                );
                assert!(
                    headings.contains(&t.family().to_string()),
                    "{spelling} has no heading above it"
                );
                offered.push(spelling);
            }
        }

        // Every type is offered somewhere, and a type the description tree
        // says nothing about is offered exactly once -- as itself. A type the
        // tree describes is offered once per description instead, which is
        // what the test below is about.
        for t in NodeType::ALL {
            let how_many = offered.iter().filter(|said| *said == t.spelling()).count();
            let described = descriptions()
                .iter()
                .filter(|one| one.node_type == t.spelling())
                .count();
            assert_eq!(
                how_many,
                described.max(1),
                "{} is offered {how_many} times and has {described} descriptions",
                t.spelling()
            );
        }
    }

    /// The description tree is what the vehicle pane offers.
    ///
    /// A type the tree describes is offered as its descriptions and not as
    /// itself: `compute` is one node type, and the tree turns it into `mean`,
    /// `hysteresis`, `to_kelvin` and `process`. Each entry is named after the
    /// description and carries what the description answers for it, so placing
    /// one puts down a compute node that is already a mean.
    #[test]
    fn the_vehicle_pane_offers_what_the_tree_describes() {
        let tree = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("share/nodes");
        let (described, trouble) = nodes::read_tree(&tree);
        assert_eq!(trouble, Vec::<String>::new());

        let rows = rows_from(Pane::Vehicle, &described);
        let named: Vec<String> = rows
            .iter()
            .filter(|row| !row.is_heading)
            .map(|row| row.node.name.to_string())
            .collect();
        for one in &described {
            assert!(named.contains(&one.node), "{} is not offered: {named:?}", one.node);
        }
        // And the bare type is not offered beside them: a compute node with no
        // function is refused until somebody types one in.
        assert!(!named.contains(&"compute".to_string()), "{named:?}");
        // A page of its own is in no description -- what a page holds is what
        // has terminals -- so it is still offered as itself.
        assert!(named.contains(&"subdiagram".to_string()), "{named:?}");

        // What a placed node is given, as the one string a callback carries.
        let mean = rows
            .iter()
            .find(|row| row.node.name == "mean")
            .expect("the mean entry");
        assert_eq!(mean.node.node_type, "compute");
        assert_eq!(mean.sets, "function=mean");
        assert_eq!(stated(&mean.sets), vec![("function".to_string(), "mean".to_string())]);

        // With no tree at all the pane is what it was before there was one:
        // every type offered as itself. An editor whose descriptions are not
        // installed is an editor, not a blank palette.
        let bare: Vec<String> = rows_from(Pane::Vehicle, &[])
            .iter()
            .filter(|row| !row.is_heading)
            .map(|row| row.node.name.to_string())
            .collect();
        assert!(bare.contains(&"compute".to_string()), "{bare:?}");
        assert!(bare.contains(&"null".to_string()), "{bare:?}");
        assert!(!bare.contains(&"mean".to_string()), "{bare:?}");
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
        for pane in [Pane::Moc, Pane::Vehicle, Pane::Payloads] {
            for row in rows_of(pane) {
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

    /// A node is drawn with the terminals its type has and no others.
    ///
    /// What the canvas says about what may be linked to what, which is the one
    /// thing a reader of a diagram cannot work out from the shapes: a payload
    /// is the right-hand end of both streams, so both of its terminals are on
    /// its left, and the ground is the left-hand end, so both of its are on
    /// its right. Checked at the view, which is what the markup draws from.
    #[test]
    fn a_node_is_drawn_with_the_terminals_its_type_has() {
        let view = |node_type: &str| view_of("x", node_type, (0, 0), None, false, None);

        // A payload: what the ground sends arrives, and its readings leave.
        let sensor = view("i2c");
        assert!(sensor.from_moc.shown && sensor.to_moc.shown);
        assert!(!sensor.to_payload.shown && !sensor.from_payload.shown);
        // Both on its left, and the arrows point the way the data travels:
        // in from the ground along the top, back out along the bottom.
        assert!(sensor.from_moc.x < sensor.width / 2.0);
        assert!(sensor.to_moc.x < sensor.width / 2.0);
        assert!(sensor.from_moc.y < sensor.to_moc.y);
        assert!(sensor.from_moc.rightward && !sensor.to_moc.rightward);

        // The ground: the other end of both, and both on its right.
        let moc = view("moc");
        assert!(moc.to_payload.shown && moc.from_payload.shown);
        assert!(!moc.from_moc.shown && !moc.to_moc.shown);
        assert!(moc.to_payload.x > moc.width / 2.0);
        assert!(moc.from_payload.x > moc.width / 2.0);
        assert!(moc.to_payload.y < moc.from_payload.y);

        // A page has all four: what is inside it may have any of them, and
        // nothing out here knows which -- so a line can be drawn to a page
        // either way on either stream.
        let page = view("subdiagram");
        for terminal in [
            &page.from_moc,
            &page.to_payload,
            &page.from_payload,
            &page.to_moc,
        ] {
            assert!(terminal.shown);
        }

        // A type this build does not know has none. The shape already says it
        // cannot be placed; four arrows on it would be four claims about a
        // type nothing here can describe -- and there is nothing to be gained
        // by letting a line be drawn to one.
        let unknown = view("contraption");
        assert!(!unknown.from_moc.shown && !unknown.to_moc.shown);

        // Every terminal is drawn inside its node, which is what makes the box
        // a person presses and the box a link arrives at the same box.
        for (node, terminal) in [
            (&sensor, &sensor.from_moc),
            (&sensor, &sensor.to_moc),
            (&moc, &moc.to_payload),
            (&moc, &moc.from_payload),
        ] {
            assert!(terminal.x >= 0.0 && terminal.y >= 0.0);
            assert!(terminal.x + style::TERMINAL.0 as f32 <= node.width);
            assert!(terminal.y + style::TERMINAL.1 as f32 <= node.height);
        }
    }

    /// Every pair of tints is told apart by a colour blind reader.
    ///
    /// The rule the palette was searched for, checked against the colours the
    /// window is actually given rather than against the intention. Fifteen
    /// colours in three hue families is well past what care at the keyboard
    /// would keep true: a tint nudged by hand would be a near-copy of another
    /// under one of the three dichromacies, and nobody with normal vision
    /// would see it.
    ///
    /// The floor is fourteen CIE units, where two or three is "the same colour
    /// in two printings" and ten is "clearly two colours". So a pair anybody
    /// can tell apart is a pair everybody can tell apart, which is what was
    /// asked for.
    ///
    /// Fourteen and not the seventeen the family-free palette managed, and the
    /// three units are the price of the families: a palette free to put its
    /// colours anywhere spreads them further than one whose reds must stay
    /// red. What the families buy is a colour that says which pane a node
    /// belongs in. A failure here means the search has to be run again rather
    /// than a value nudged.
    #[test]
    fn no_two_tints_are_the_same_colour_to_a_colour_blind_reader() {
        use sight::{difference, seen, Sight};

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
            apart >= 14.0,
            "{a} and {b} are only {apart:.1} apart to a reader with \
             {sight:?}, and the floor is 14"
        );
    }

    /// Every node is legible on its own pane, and is in that pane's family.
    ///
    /// Which is the other half of what a colour is for here. A node is drawn
    /// on a tinted ground rather than on white, so "is this colour usable" is
    /// a question about the pair: three to one is the threshold for something
    /// that is not text, and it is asked of the node against its own pane.
    ///
    /// And the hue says where the node belongs. A payload is a plum or a
    /// maroon on pale blush, a vehicle node a green on pale green, the ground
    /// link an azure on pale blue -- so somebody who has learned one pane's
    /// colour can tell at a glance when a node is drawn in the wrong one.
    #[test]
    fn every_node_is_legible_on_its_own_pane_and_in_its_family() {
        use sight::{chroma, contrast, hue, hue_apart, lightness};

        let painted = |hex: &str| {
            let c = colour(hex);
            (c.red(), c.green(), c.blue())
        };

        for t in NodeType::ALL {
            let node = painted(&tint_of(t.appearance().tint));
            let pane = pane_of(t.spelling());
            let ground = painted(&ground_of(style::ground_of(pane)));

            let against = contrast(node, ground);
            assert!(
                against >= 3.0,
                "{} has {against:.2} contrast against the {} pane",
                t.spelling(),
                pane.label()
            );
            // Darker than its ground, and by enough to read as something
            // drawn on it rather than as a hole in it.
            assert!(
                lightness(ground) - lightness(node) >= 25.0,
                "{} is {:.0} and its pane is {:.0}",
                t.spelling(),
                lightness(node),
                lightness(ground)
            );
            // And in the pane's own hue family.
            let apart = hue_apart(hue(node), hue(ground));
            assert!(
                apart <= 40.0,
                "{}'s hue is {apart:.0} degrees from the {} pane's",
                t.spelling(),
                pane.label()
            );
        }

        // The three grounds: light, neutral, and far enough apart that
        // somebody with any of the three dichromacies can tell which pane
        // they are looking at.
        use sight::{difference, seen, Sight};
        let grounds: Vec<(Pane, (u8, u8, u8))> = [Pane::Moc, Pane::Vehicle, Pane::Payloads]
            .into_iter()
            .map(|pane| (pane, painted(&ground_of(style::ground_of(pane)))))
            .collect();
        for (pane, ground) in &grounds {
            assert!(
                lightness(*ground) >= 84.0,
                "the {} pane is {:.0} light, and a working pane is a light \
                 ground",
                pane.label(),
                lightness(*ground)
            );
            assert!(
                chroma(*ground) <= 14.0,
                "the {} pane has a chroma of {:.0}, and a ground is neutral",
                pane.label(),
                chroma(*ground)
            );
        }
        for (i, (one, a)) in grounds.iter().enumerate() {
            for (two, b) in &grounds[i + 1..] {
                for sight in Sight::ALL {
                    let apart = difference(seen(*a, sight), seen(*b, sight));
                    assert!(
                        apart >= 15.0,
                        "the {} and {} panes are {apart:.1} apart to a reader \
                         with {sight:?}",
                        one.label(),
                        two.label()
                    );
                }
            }
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

        let (top, _) = views_of(&editor, &None, None);
        let names: Vec<String> = top.iter().map(|n| n.name.to_string()).collect();
        assert_eq!(names, vec!["cabin".to_string(), "moc".to_string()]);

        let (inside, _) = views_of(&editor, &Some("cabin".into()), None);
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
        let (_, inside) = views_of(&editor, &Some("cabin".into()), None);
        assert_eq!(inside.len(), 1);
        assert_eq!(inside[0].label, "moc");

        // And seen from the top diagram, where its source is off the page.
        let (_, top) = views_of(&editor, &None, None);
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

    /// A link's ends are placed on the terminals it is attached to.
    ///
    /// Which is the drawing the canvas is for: the readings leave a payload by
    /// its `to_moc`, in its lower left corner, and arrive at the computation
    /// by its `from_payload`, in that node's lower right -- so the line runs
    /// leftward along the bottom of both, the way the data goes and the way
    /// the panes are arranged. It used to leave the middle of the source's
    /// right edge and arrive at the middle of the target's left edge, which
    /// was the only thing a line could do before terminals.
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
            // The payload to the right of the computation, which is where
            // the panes put it: payloads are the rightmost working pane and
            // the vehicle's own graph is the middle one.
            Edit::MoveNode {
                node: "a".into(),
                was: (0, 0),
                now: (504, 24),
            },
            Edit::MoveNode {
                node: "b".into(),
                was: (0, 0),
                now: (200, 40),
            },
            Edit::AddLink {
                link: Link::new("a", "b", Some("in0")),
            },
        ] {
            editor.edit(edit);
        }

        let (_, links) = views_of(&editor, &None, None);
        assert_eq!(links.len(), 1);
        let view = &links[0];

        // It starts on the payload's own `to_moc` terminal, which is the
        // left edge of its lower left corner, and finishes on the
        // computation's `from_payload`, the right edge of its lower right
        // corner -- so the line touches both boxes it joins and crosses
        // neither shape.
        let ((start_x, start_y), leaves_right) =
            style::attachment("i2c", (504, 24), "to_moc").expect("a payload's source");
        assert!(!leaves_right, "the readings leave towards the ground");
        assert!(
            view.route
                .starts_with(&format!("M {start_x:.1} {start_y:.1}")),
            "{}",
            view.route
        );

        let ((end_x, end_y), _) =
            style::attachment("compute", (200, 40), "from_payload").expect("a sink");
        assert!(
            view.route.ends_with(&format!("{end_x:.1} {end_y:.1}")),
            "{}",
            view.route
        );
        assert!(view.head.ends_with('Z'), "there should be an arrowhead");
        assert!(view.wide, "a link into a named input is drawn wide");

        // And the whole of it is on the canvas. A line that leaves a node by
        // its left side has its stub to the left of that node, which runs off
        // the edge if the node is against it -- and a line that disappears
        // reads as a line that stops. No minus sign anywhere in the path is
        // the whole of the check, the canvas starting at nought.
        assert!(
            !view.route.contains('-'),
            "part of the route is off the canvas: {}",
            view.route
        );
    }

    /// A link is drawn from one terminal to the one it pairs with, and
    /// nothing else is drawn at all.
    ///
    /// The gesture, without the window: the terminal pressed and the point the
    /// pointer was let go of are the whole of what it says, and `joined` is
    /// what turns those into a link or into nothing. Each refusal here is a
    /// link the editor cannot make -- which is worth more than one it makes
    /// and then marks red.
    #[test]
    fn a_link_is_drawn_between_terminals_that_pair() {
        use journal::Edit;

        let mut document = Document::new();
        for (name, node_type, at) in [
            ("sensor", "i2c", (560, 120)),
            ("through", "null", (300, 120)),
            ("ground", "moc", (16, 120)),
        ] {
            document.apply(&Edit::AddNode {
                name: name.into(),
                node_type: node_type.into(),
                home: None,
            });
            document.apply(&Edit::MoveNode {
                node: name.into(),
                was: (0, 0),
                now: at,
            });
        }

        // The middle of a terminal's box, which is where a person lets go.
        // Taken from a copy of where the nodes are, so that a link can be
        // added to the document afterwards without the borrow outliving it.
        let placed: Vec<(String, String, (i32, i32))> = document
            .nodes_in(&None)
            .iter()
            .map(|node| (node.name.clone(), node.node_type.clone(), node.at))
            .collect();
        let over = |node: &str, terminal: &str| {
            let (_, node_type, at) = placed
                .iter()
                .find(|(name, _, _)| name == node)
                .expect("the node");
            let (x, y, width, height) =
                style::terminal_box(node_type, *at, terminal).expect("a terminal");
            (x + width / 2.0, y + height / 2.0)
        };
        let drawn = |node: &str, terminal: &str| Drawing {
            node: node.to_string(),
            terminal: terminal.to_string(),
            holding: None,
        };

        // The readings, from the payload to the ground: a to_moc let go of
        // over a from_payload. Neither end names its terminal, both types
        // having one of each -- which is what keeps a saved file saying
        // `{ from: sensor, to: ground }`.
        let link = joined(
            &document,
            &None,
            &drawn("sensor", "to_moc"),
            over("ground", "from_payload"),
        )
        .expect("a payload's readings reach the ground");
        assert_eq!(link.from, "sensor");
        // The terminal it leaves by, always: a link is written as an
        // attribute of its own node, named after that terminal.
        assert_eq!(link.origin.as_deref(), Some("to_moc"));
        assert_eq!(link.to, "ground");
        // And nothing at the arriving end, the ground link having one data
        // sink: the schema reads an unnamed end as that one.
        assert_eq!(link.input, None);

        // And back the other way, which is the other stream: a to_payload let
        // go of over a from_moc.
        let back = joined(
            &document,
            &None,
            &drawn("ground", "to_payload"),
            over("sensor", "from_moc"),
        )
        .expect("what the ground sends reaches the payload");
        assert_eq!((back.from.as_str(), back.to.as_str()), ("ground", "sensor"));

        // A null node has two of each, so its links say which: that is the
        // whole reason a terminal is written into a file.
        let through = joined(
            &document,
            &None,
            &drawn("through", "to_moc"),
            over("ground", "from_payload"),
        )
        .expect("a null node reaches the ground");
        assert_eq!(through.origin.as_deref(), Some("to_moc"));

        // Now the four gestures that are not links.
        //
        // Let go of over a terminal that does not pair: a to_payload carries
        // what the ground sends, and a from_payload is where readings arrive
        // -- a line between them would carry one stream into the other.
        assert_eq!(
            joined(
                &document,
                &None,
                &drawn("through", "to_payload"),
                over("ground", "from_payload"),
            ),
            None,
            "a to_payload may only be linked to a from_moc"
        );

        // Let go of over a data source, which is where a link is drawn from
        // and not a place to let go of one.
        assert_eq!(
            joined(
                &document,
                &None,
                &drawn("sensor", "to_moc"),
                over("ground", "to_payload"),
            ),
            None
        );

        // Let go of over nothing.
        assert_eq!(
            joined(&document, &None, &drawn("sensor", "to_moc"), (900.0, 400.0)),
            None
        );

        // Let go of over the node it started on, which is a gesture that
        // changed its mind rather than a request for a loop.
        assert_eq!(
            joined(
                &document,
                &None,
                &drawn("through", "to_moc"),
                over("through", "from_moc"),
            ),
            None
        );

        // Drawn the other way about, which is the same link: pressed on the
        // ground link's data sink and let go of over the payload's data
        // source. A person drawing a diagram works in whichever direction they
        // are reading it.
        let backwards = joined(
            &document,
            &None,
            &drawn("ground", "from_payload"),
            over("sensor", "to_moc"),
        )
        .expect("a link drawn from the sink end");
        assert_eq!(backwards, link, "the same link, whichever end it was drawn from");

        // But not between two ends of the same kind: a source feeds a sink,
        // and two sources have nothing to say to each other.
        assert_eq!(
            joined(
                &document,
                &None,
                &drawn("sensor", "to_moc"),
                over("through", "to_moc"),
            ),
            None
        );

        // And a link that is already there: a second line on top of the first
        // would be two deliveries of every datum.
        document.apply(&Edit::AddLink { link: link.clone() });
        assert_eq!(
            joined(
                &document,
                &None,
                &drawn("sensor", "to_moc"),
                over("ground", "from_payload"),
            ),
            None,
            "the same link twice"
        );
    }

    /// A node's comment may be several lines, and they reach the file.
    ///
    /// The comment field is at the bottom of a node's window and scrolls on
    /// its own, because a note is the field most likely to be long: "0x49
    /// after the rework" and the reason for it. The file writes it as a run of
    /// comment lines above the node and reads it back the same way, so the
    /// shape it is typed in is the shape it keeps.
    #[test]
    fn a_comment_of_several_lines_is_kept() {
        use journal::Edit;

        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.yaml");
        let mut editor = Editor::open(&path, dividers());
        editor.edit(Edit::AddNode {
            name: "sensor".to_string(),
            node_type: "i2c".to_string(),
            home: None,
        });
        editor.edit(Edit::SetNodeComment {
            node: "sensor".to_string(),
            was: None,
            now: Some("0x49 after the rework.\nSee ECR-114.".to_string()),
        });

        let written = std::fs::read_to_string(&path).expect("the file");
        assert!(written.contains("  # 0x49 after the rework.\n"), "{written}");
        assert!(written.contains("  # See ECR-114.\n"), "{written}");

        // And read back as the one note it is.
        let read = crate::read::read(&path).expect("it reads");
        assert_eq!(
            read.node("sensor").expect("the node").comment.as_deref(),
            Some("0x49 after the rework.\nSee ECR-114.")
        );
    }

    /// The end of a line can be taken hold of, moved, or dropped on the floor.
    ///
    /// The four things a person can do with it, which is the whole of the
    /// gesture: put it back, move it to another terminal that suits, try one
    /// that does not, or let go of it over nothing. The last is also the only
    /// way to delete a link, and it is the one somebody reaches for -- pull
    /// the wire out and drop it.
    #[test]
    fn the_end_of_a_link_can_be_moved_or_dropped() {
        use journal::{Edit, Link};

        let mut document = Document::new();
        for (name, node_type, at) in [
            ("ground", "moc", (16, 120)),
            ("through", "null", (300, 120)),
            ("sensor", "i2c", (600, 120)),
            ("other", "i2c", (600, 400)),
        ] {
            document.apply(&Edit::AddNode {
                name: name.into(),
                node_type: node_type.into(),
                home: None,
            });
            document.apply(&Edit::MoveNode {
                node: name.into(),
                was: (0, 0),
                now: at,
            });
        }
        // The readings of one sensor, reaching the ground.
        let held = Link::joining("sensor", Some("to_moc"), "ground", None);
        document.apply(&Edit::AddLink { link: held.clone() });

        // Where each node is and what it is, copied out so that a link can be
        // added to the document afterwards without the borrow outliving it.
        let placed: Vec<(String, String, (i32, i32))> = document
            .nodes_in(&None)
            .iter()
            .map(|node| (node.name.clone(), node.node_type.clone(), node.at))
            .collect();
        let found = |node: &str| {
            placed
                .iter()
                .find(|(name, _, _)| name == node)
                .expect("the node")
                .clone()
        };
        let at = |node: &str| found(node).2;
        let kind = |node: &str| found(node).1;
        let over = |node: &str, terminal: &str| {
            let (x, y, width, height) =
                style::terminal_box(&kind(node), at(node), terminal).expect("a terminal");
            (x + width / 2.0, y + height / 2.0)
        };
        let middle = |node: &str| {
            let (width, height) = size_of(&kind(node));
            (
                at(node).0 as f32 + width as f32 / 2.0,
                at(node).1 as f32 + height as f32 / 2.0,
            )
        };
        // The end that was taken hold of: the sink, which is where this link
        // arrives.
        let pressed = Drawing {
            node: "ground".to_string(),
            terminal: "from_payload".to_string(),
            holding: Some(held.clone()),
        };

        // Put back where it came from: nothing happens. A gesture that has
        // thought better of itself leaves no trace.
        assert_eq!(
            dropped(&document, &None, &held, &pressed, over("ground", "from_payload")),
            None
        );

        // Let go of over a node but not over a terminal: also nothing. That
        // is a gesture that missed.
        assert_eq!(dropped(&document, &None, &held, &pressed, middle("ground")), None);

        // Moved to a terminal that suits the end: one edit, so one undo puts
        // it back. The far end -- the sensor -- stays where it is.
        let moved = dropped(
            &document,
            &None,
            &held,
            &pressed,
            over("through", "from_payload"),
        )
        .expect("a sink that pairs with the source");
        match moved {
            Edit::MoveLink { was, now } => {
                assert_eq!(was, held);
                assert_eq!(now.from, "sensor", "the far end stays put");
                assert_eq!(now.origin.as_deref(), Some("to_moc"));
                assert_eq!(now.to, "through");
                assert_eq!(now.input.as_deref(), Some("from_payload"));
            }
            other => panic!("{other:?}"),
        }

        // A terminal that does not suit it: nothing. A `from_moc` takes what
        // the ground sends, and this link carries readings.
        assert_eq!(
            dropped(
                &document,
                &None,
                &held,
                &pressed,
                over("through", "from_moc")
            ),
            None
        );

        // Nor onto a data source, which is the other end's kind.
        assert_eq!(
            dropped(&document, &None, &held, &pressed, over("through", "to_moc")),
            None
        );

        // Nor onto the far node itself, which would be a link from a node to
        // itself.
        assert_eq!(
            dropped(&document, &None, &held, &pressed, over("sensor", "from_moc")),
            None
        );

        // Over nothing at all: the link goes.
        assert_eq!(
            dropped(&document, &None, &held, &pressed, (40.0, 900.0)),
            Some(Edit::RemoveLink { link: held.clone() })
        );

        // And the source end moves the same way: taken hold of at the
        // sensor's `to_moc` and put on another sensor's.
        let pressed = Drawing {
            node: "sensor".to_string(),
            terminal: "to_moc".to_string(),
            holding: Some(held.clone()),
        };
        let moved = dropped(&document, &None, &held, &pressed, over("other", "to_moc"))
            .expect("another sensor's readings may reach the same ground");
        match moved {
            Edit::MoveLink { now, .. } => {
                assert_eq!(now.from, "other");
                assert_eq!(now.origin.as_deref(), Some("to_moc"));
                assert_eq!(now.to, "ground", "the far end stays put");
            }
            other => panic!("{other:?}"),
        }

        // And a move that would make a link the document already has does
        // nothing: two lines between one pair of terminals lie on top of each
        // other and deliver twice.
        document.apply(&Edit::AddLink {
            link: Link::joining("other", Some("to_moc"), "ground", None),
        });
        assert_eq!(
            dropped(&document, &None, &held, &pressed, over("other", "to_moc")),
            None
        );
    }

    /// A terminal with one link on it hands it over; one with several does not.
    ///
    /// Which is the rule that lets one gesture mean two things. A data sink
    /// takes one source, so dragging from a sink is always "move this line".
    /// A data source may reach several sinks, and when it already has more
    /// than one nothing says which of them was meant -- so that gesture draws
    /// a new line instead, and a second line out of a source is drawn from the
    /// sink end.
    #[test]
    fn a_terminal_with_one_link_hands_it_over() {
        use journal::{Edit, Link};

        let mut document = Document::new();
        for (name, node_type) in [
            ("ground", "moc"),
            ("sensor", "i2c"),
            ("other", "i2c"),
            ("housekeeping", "log"),
        ] {
            document.apply(&Edit::AddNode {
                name: name.into(),
                node_type: node_type.into(),
                home: None,
            });
        }

        let at = |node: &str, terminal: &str| Drawing {
            node: node.to_string(),
            terminal: terminal.to_string(),
            holding: None,
        };

        // Nothing linked yet: nothing to hand over.
        assert!(attached(&document, &at("sensor", "to_moc"), false).is_empty());

        document.apply(&Edit::AddLink {
            link: Link::joining("sensor", Some("to_moc"), "ground", None),
        });
        assert_eq!(attached(&document, &at("sensor", "to_moc"), false).len(), 1);
        // And the sink end of the same link, from the other side.
        assert_eq!(
            attached(&document, &at("ground", "from_payload"), true).len(),
            1
        );

        // A second destination from the same source: now the source has two,
        // and nothing says which a drag meant.
        document.apply(&Edit::AddLink {
            link: Link::joining("sensor", Some("to_moc"), "housekeeping", None),
        });
        assert_eq!(attached(&document, &at("sensor", "to_moc"), false).len(), 2);

        // The ground's sink takes many, and the same is true of it: every
        // payload aboard reports to the one ground link.
        document.apply(&Edit::AddLink {
            link: Link::joining("other", Some("to_moc"), "ground", None),
        });
        assert_eq!(
            attached(&document, &at("ground", "from_payload"), true).len(),
            2
        );
    }

    /// A link to a terminal is recorded as the terminal that was pressed.
    ///
    /// The fault this is here for: a line dragged from the ground link's
    /// `to_payload` to the upper left corner of a computation -- its `from_moc`
    /// -- was recorded as a link naming no terminal at all, and then drawn
    /// arriving at the computation's lower right. Twice wrong: what somebody
    /// had just said was thrown away, and the drawing then guessed the other
    /// stream.
    ///
    /// Thrown away because the question had been put to the schema's table,
    /// which calls a computation's sinks its own business, rather than to the
    /// table the canvas drew from. The gesture is about the boxes on the
    /// screen.
    #[test]
    fn a_link_into_a_computation_remembers_which_terminal() {
        use journal::Edit;

        let mut document = Document::new();
        for (name, node_type, at) in [
            ("ground", "moc", (16, 120)),
            ("to_kelvin", "compute", (300, 120)),
        ] {
            document.apply(&Edit::AddNode {
                name: name.into(),
                node_type: node_type.into(),
                home: None,
            });
            document.apply(&Edit::MoveNode {
                node: name.into(),
                was: (0, 0),
                now: at,
            });
        }

        let over = |node: &str, node_type: &str, at: (i32, i32), terminal: &str| {
            let (x, y, width, height) =
                style::terminal_box(node_type, at, terminal).expect("a terminal");
            let _ = node;
            (x + width / 2.0, y + height / 2.0)
        };

        let link = joined(
            &document,
            &None,
            &Drawing {
                node: "ground".to_string(),
                terminal: "to_payload".to_string(),
                holding: None,
            },
            over("to_kelvin", "compute", (300, 120), "from_moc"),
        )
        .expect("the ground reaches a computation");

        assert_eq!(link.from, "ground");
        assert_eq!(link.origin.as_deref(), Some("to_payload"));
        assert_eq!(link.to, "to_kelvin");
        // The terminal that was pressed, which is the whole of what the
        // gesture said: a computation has two data sinks, so the file has to
        // say which.
        assert_eq!(link.input.as_deref(), Some("from_moc"));

        // And the line is drawn to that corner: the upper left, where what
        // the ground sends arrives.
        document.apply(&Edit::AddLink { link });
        let ends = ends_of(&document, &document.links[0], &None);
        let ((x, y), _) =
            style::attachment("compute", (300, 120), "from_moc").expect("a terminal");
        assert_eq!(ends.to, (x, y), "the line arrives at the terminal it was drawn to");

        // A mean is the one destination whose arriving end is named by
        // something that is not a terminal, and it still fills its inputs.
        document.apply(&Edit::AddNode {
            name: "mean_temp".into(),
            node_type: "compute".into(),
            home: None,
        });
        document.apply(&Edit::SetAttribute {
            node: "mean_temp".into(),
            attribute: "inputs".into(),
            was: None,
            now: Some("in0,in1".into()),
        });
        assert!(declares_inputs(&document, "mean_temp"));
        assert_eq!(
            arriving_at(&document, "mean_temp", "from_payload").as_deref(),
            Some("in0"),
            "a mean is filled by name, not by terminal"
        );
    }

    /// A link the file does not name a sink for is drawn at the pairing one.
    ///
    /// A link leaving `to_payload` carries what the ground sends, so it
    /// arrives at a `from_moc`: the same stream, named at its other end. The
    /// drawing used to fall back to the groundward sink whatever the link
    /// carried, which drew a command from the ground arriving at the corner
    /// the readings leave by.
    #[test]
    fn an_unnamed_sink_is_drawn_where_its_stream_arrives() {
        use journal::{Edit, Link};

        let mut document = Document::new();
        for (name, node_type, at) in [
            ("ground", "moc", (16, 120)),
            ("through", "null", (300, 120)),
        ] {
            document.apply(&Edit::AddNode {
                name: name.into(),
                node_type: node_type.into(),
                home: None,
            });
            document.apply(&Edit::MoveNode {
                node: name.into(),
                was: (0, 0),
                now: at,
            });
        }
        // Written by hand, naming the source and not the sink.
        document.apply(&Edit::AddLink {
            link: Link::joining("ground", Some("to_payload"), "through", None),
        });

        let ends = ends_of(&document, &document.links[0], &None);
        let ((x, y), _) =
            style::attachment("null", (300, 120), "from_moc").expect("the sink");
        assert_eq!(
            ends.to,
            (x, y),
            "what the ground sends arrives where that stream arrives"
        );
    }

    /// Every working pane has room for a node.
    ///
    /// Each pane is a share of the working area and every node is one width:
    /// a pane narrower than that width is a pane nothing fits in, and every
    /// node placed there would land in the same place because the clamp had
    /// nowhere to put it.
    #[test]
    fn every_pane_has_room_for_a_node() {
        let width = style::WIDTH as i32;
        let dividers = dividers();
        for pane in [Pane::Moc, Pane::Vehicle, Pane::Payloads] {
            let (left, right) = edges_of(pane, dividers);
            assert!(
                right - left >= width + GRID * 2,
                "the {} working pane is {}px wide and a node is {width}px",
                pane.label(),
                right - left
            );
        }

        // And the vehicle pane has room for several across, which is what it
        // is the largest of the three for.
        let (left, right) = edges_of(Pane::Vehicle, dividers);
        assert!(
            right - left >= width * 3,
            "the vehicle working pane is {}px wide and a node is {width}px",
            right - left
        );

        // On the grid, so that a node held against a divider is on it too.
        assert_eq!(dividers.moc % GRID, 0);
        assert_eq!(dividers.vehicle % GRID, 0);
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
        let d = dividers();
        let (moc, vehicle) = (d.moc, d.vehicle);

        // A payload: left to the vehicle divider, right to the far edge.
        assert_eq!(confine("i2c", d, (0, 100)), (vehicle, 100));
        assert_eq!(confine("i2c", d, (vehicle - 1, 100)), (vehicle, 100));
        assert_eq!(confine("i2c", d, (vehicle + 8, 100)), (vehicle + 8, 100));
        assert_eq!(
            confine("i2c", d, (WORKING_WIDTH, 100)),
            (WORKING_WIDTH - width, 100)
        );

        // The ground link: the leftmost pane, and nowhere right of its own
        // divider.
        assert_eq!(confine("moc", d, (0, 100)), (0, 100));
        assert_eq!(confine("moc", d, (-40, 100)), (0, 100));
        assert_eq!(confine("moc", d, (WORKING_WIDTH, 100)), (moc - width, 100));

        // And the vehicle's own graph, between the two.
        assert_eq!(confine("compute", d, (0, 100)), (moc, 100));
        assert_eq!(confine("compute", d, (-40, 100)), (moc, 100));
        assert_eq!(
            confine("compute", d, (WORKING_WIDTH, 100)),
            (vehicle - width, 100)
        );

        // A node is never dragged off the top, whichever pane it is in: a
        // negative y is a node nobody can reach to drag back.
        for node_type in ["i2c", "compute", "moc"] {
            assert_eq!(confine(node_type, d, (vehicle, -80)).1, 0, "{node_type}");
        }

        // A type this build does not know goes in the vehicle pane, which is
        // where something that needs looking at should be.
        assert!(!is_payload("contraption"));
        let (left, right) = pane::pane_for("contraption", d);
        assert_eq!(left, moc);
        assert!(right < vehicle);
    }

    /// Every type is placed in the pane its list is in.
    ///
    /// The rule that was asked for: a payload can only be created in the
    /// payload working pane, the ground link only in the moc working pane,
    /// and the rest of the graph only in the vehicle's. Checked for every
    /// type rather than for one of each, because the thing that would break
    /// it is a new type whose family says one thing and whose placement says
    /// another.
    #[test]
    fn every_type_is_placed_in_its_own_pane() {
        let document = Document::new();
        let d = dividers();
        for t in NodeType::ALL {
            let (x, y) = next_place(&document, &None, t.spelling(), d, IN_VIEW);
            let (width, _) = size_of(t.spelling());
            let right = x + width as i32;
            let pane = pane_of(t.spelling());
            let (left, edge) = edges_of(pane, d);
            assert!(
                x >= left && right <= edge,
                "{} is placed at {x} and reaches {right}, outside the {} \
                 working pane ({left}..{edge})",
                t.spelling(),
                pane.label()
            );
            assert!(y >= 0, "{} is placed off the top", t.spelling());
        }
    }

    /// Nodes placed in one pane do not move where the others go.
    ///
    /// A new node goes as high as it fits in its *own* pane, and what is in
    /// another pane cannot be in the way: three computations in the middle
    /// column leave the top of the payload column empty, and a payload that
    /// appeared three steps down one it had never been in would look like
    /// three nodes gone missing.
    #[test]
    fn placing_in_one_pane_does_not_move_the_other_along() {
        let mut document = Document::new();
        let d = dividers();
        let first_payload = next_place(&document, &None, "i2c", d, IN_VIEW);

        for (name, at) in [("a", (0, 0)), ("b", (0, 60)), ("c", (0, 120))] {
            document.apply(&Edit::AddNode {
                name: name.into(),
                node_type: "compute".into(),
                home: None,
            });
            document.apply(&Edit::MoveNode {
                node: name.into(),
                was: (0, 0),
                now: (d.moc + 24, at.1),
            });
        }
        assert_eq!(
            next_place(&document, &None, "i2c", d, IN_VIEW),
            first_payload,
            "three computations moved where the first payload goes"
        );

        // And a payload already placed is in the way, so the next one goes
        // beside it or below it rather than on top of it.
        document.apply(&Edit::AddNode {
            name: "temp_fore".into(),
            node_type: "i2c".into(),
            home: None,
        });
        document.apply(&Edit::MoveNode {
            node: "temp_fore".into(),
            was: (0, 0),
            now: first_payload,
        });
        let second = next_place(&document, &None, "i2c", d, IN_VIEW);
        assert_ne!(second, first_payload);
        let (width, height) = size_of("i2c");
        let clear = second.0 >= first_payload.0 + width as i32
            || second.1 >= first_payload.1 + height as i32;
        assert!(clear, "{second:?} covers {first_payload:?}");
    }

    /// Neither line can squeeze a pane below a twentieth of the working area.
    ///
    /// Both lines and all three panes, which is three rules in one place: the
    /// moc line keeps the moc pane and the vehicle's open, and the vehicle
    /// line keeps the vehicle's and the payloads' open. A pane with nothing
    /// visible in it is a pane whose nodes cannot be reached, and the drag
    /// that does it is one press away.
    #[test]
    fn neither_divider_can_close_a_pane() {
        let document = Document::new();
        let least = WORKING_WIDTH * settings::LEAST / 100;

        for (which, dividers) in [
            (Pane::Moc, dividers()),
            (Pane::Vehicle, dividers()),
            // And from the ends of their travel, where an off-by-one would
            // show: a line already against what stops it.
            (
                Pane::Moc,
                Dividers {
                    moc: least,
                    vehicle: least * 2,
                },
            ),
            (
                Pane::Vehicle,
                Dividers {
                    moc: least,
                    vehicle: WORKING_WIDTH - least,
                },
            ),
        ] {
            let (low, high) = divider_range(&document, dividers, which);
            for at in [low, high] {
                let settled = if which == Pane::Moc {
                    Dividers { moc: at, ..dividers }
                } else {
                    Dividers {
                        vehicle: at,
                        ..dividers
                    }
                };
                for pane in [Pane::Moc, Pane::Vehicle, Pane::Payloads] {
                    let (left, right) = edges_of(pane, settled);
                    assert!(
                        right - left >= least,
                        "{which:?} at {at} leaves the {} pane {} wide, and a \
                         twentieth is {least}",
                        pane.label(),
                        right - left
                    );
                }
            }
        }
    }

    /// A new node goes as high as it fits, in view, covering nothing.
    ///
    /// The three places it may go, in the order somebody would look: the
    /// highest in view, then the lowest above the view, then the highest below
    /// it. A sheet is longer than a window, so all three happen.
    #[test]
    fn a_new_node_goes_as_high_as_it_fits_in_view() {
        let d = dividers();
        let (width, height) = size_of("i2c");
        let (left, _) = pane::pane_for("i2c", d);

        // An empty pane: the top of the view, wherever the view is.
        let scrolled = (600, 400);
        assert_eq!(
            pane::place_in(&[], "i2c", d, scrolled),
            (left, 600),
            "a new node goes at the top of what is on the screen"
        );

        // A row of them across the pane, and the next goes under the row
        // rather than on top of one.
        let mut taken: Vec<(i32, i32, u32, u32)> = Vec::new();
        let mut x = left;
        while x <= pane::pane_for("i2c", d).1 {
            taken.push((x, 600, width, height));
            x += width as i32;
        }
        let under = pane::place_in(&taken, "i2c", d, scrolled);
        assert!(under.1 >= 600 + height as i32, "{under:?}");
        assert!(under.1 < 600 + 400, "and still in view: {under:?}");

        // A view packed solid: the lowest place *above* it, which is the
        // nearest and so the shortest scroll.
        let mut packed: Vec<(i32, i32, u32, u32)> = Vec::new();
        let mut y = 600 - 200;
        while y < 600 + 400 {
            let mut x = left;
            while x <= pane::pane_for("i2c", d).1 {
                packed.push((x, y, width, height));
                x += GRID;
            }
            y += GRID;
        }
        let above = pane::place_in(&packed, "i2c", d, scrolled);
        assert!(
            above.1 + height as i32 <= 600 - 200,
            "a node should go above the view when the view is full: {above:?}"
        );

        // And everything above as well: below the view, highest first.
        let mut walled = packed.clone();
        let mut y = 0;
        while y < 600 - 200 {
            let mut x = left;
            while x <= pane::pane_for("i2c", d).1 {
                walled.push((x, y, width, height));
                x += GRID;
            }
            y += GRID;
        }
        let below = pane::place_in(&walled, "i2c", d, scrolled);
        assert!(
            below.1 >= 600 + 400,
            "a node should go below the view when everything above is taken: \
             {below:?}"
        );
        assert!(below.1 + height as i32 <= pane::WORKING_HEIGHT, "{below:?}");
    }

    /// The divider comes back where it was left.
    ///
    /// What is written down is a percentage and what the canvas works in is
    /// pixels, so the two conversions have to agree or the panes would creep
    /// by a step every time the editor started. Every position the control can
    /// settle on, round tripped.
    #[test]
    fn the_dividers_survive_being_written_down() {
        for moc in settings::LEAST..=settings::MOST {
            for vehicle in [settings::LEAST, 25, settings::DEFAULT_VEHICLE] {
                let (moc, vehicle) = settings::shares(moc, vehicle);
                let dividers = dividers_for(moc, vehicle);
                assert_eq!(dividers.moc % GRID, 0, "{moc}% is off the grid");
                assert_eq!(dividers.vehicle % GRID, 0, "{vehicle}% is off the grid");
                // And a percentage means what it says: that much of the
                // working pane, within the grid step it is snapped to.
                assert!(
                    (dividers.moc - WORKING_WIDTH * moc / 100).abs() <= GRID / 2,
                    "a moc pane of {moc}% was placed at {} of {WORKING_WIDTH}",
                    dividers.moc
                );
                assert!(
                    (dividers.vehicle - WORKING_WIDTH * (moc + vehicle) / 100).abs()
                        <= GRID / 2,
                    "a vehicle pane of {vehicle}% left the divider at {} of \
                     {WORKING_WIDTH}",
                    dividers.vehicle
                );
                let (moc_again, vehicle_again) = percents_for(dividers);
                assert_eq!(
                    dividers_for(moc_again, vehicle_again),
                    dividers,
                    "{moc}% and {vehicle}% come back as {moc_again}% and \
                     {vehicle_again}%"
                );
            }
        }

        // And percentages outside the rule are brought inside it rather than
        // obeyed: a hand-edited settings file is still a file the editor has
        // to open.
        assert_eq!(
            dividers_for(0, 200),
            dividers_for(settings::LEAST, settings::MOST)
        );
        assert_eq!(
            dividers_for(-5, -5),
            dividers_for(settings::LEAST, settings::LEAST)
        );
    }

    /// No working pane can be squeezed away.
    ///
    /// The rule that was asked for: each pane keeps at least a twentieth of
    /// the working pane, so none can be dragged shut -- and with three of
    /// them, a divider stops a twentieth short of its neighbour as well as of
    /// the edge. A pane with nothing visible in it is a pane whose nodes
    /// cannot be reached, and the drag that does it is one press away.
    #[test]
    fn no_pane_can_be_squeezed_away() {
        let document = Document::new();
        let d = dividers();
        let step = WORKING_WIDTH * settings::LEAST / 100;

        let (low, high) = divider_range(&document, d, Pane::Moc);
        assert_eq!(low, step, "the moc pane keeps a twentieth");
        assert_eq!(high, d.vehicle - step, "and so does the vehicle's");

        let (low, high) = divider_range(&document, d, Pane::Vehicle);
        assert_eq!(low, d.moc + step);
        assert_eq!(
            high,
            WORKING_WIDTH - step,
            "the payload pane keeps a twentieth"
        );
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
        let d = dividers();

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
            now: (d.vehicle + 48, 40),
        });

        let (low, high) = divider_range(&document, d, Pane::Vehicle);
        assert_eq!(low, 400 + width, "the computation's right edge stops it");
        assert_eq!(high, d.vehicle + 48, "the payload's left edge stops it");
        assert!(low <= d.vehicle && d.vehicle <= high, "{low}..{high}");

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
            divider_range(&document, d, Pane::Vehicle),
            (low, high),
            "a payload left of the divider should not pin it"
        );
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
        let mut editor = Editor::open(&path, dividers());
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
                Some(tcslibgs::graph::parameters::default_of(attribute).unwrap_or("")),
                "{attribute} should be offered with whatever it defaults to"
            );
        }
        // Which is nothing, for everything but the beacon's interval: a socket
        // has no value that is right until somebody says so, and "how often
        // does the vehicle say it is alive" has one.
        assert_eq!(
            said.get("beacon.interval_ms").map(String::as_str),
            Some("1000")
        );
        // And none of it is a fault: the editor is lazy, and a configuration
        // nobody has filled in yet is one nobody has filled in yet. What it
        // must say before it flies is the ground check's to insist on, with a
        // file and a line.
        assert!(editor.faults().is_empty(), "{:?}", editor.faults());

        // And what reaches the file is what is stated, which is the beacon's
        // interval and nothing else: an unanswered attribute is written out of
        // the file rather than written empty, and a `tcspecial:` with nothing
        // under it would read back as no section at all.
        let written = std::fs::read_to_string(&path).expect("the file");
        assert!(written.contains("interval_ms: \"1000\""), "{written}");
        for unanswered in ["tcs_cmd", "payload_telemetry", "log", "rust-code-path"] {
            assert!(
                !written.contains(unanswered),
                "{unanswered} is not stated and should not be written: {written}"
            );
        }

        let moc = editor.document().node("moc").expect("a ground station");
        assert_eq!(moc.node_type, "moc");
        assert_eq!(moc.at, MOC_PLACE);
        // The upper left of the moc working pane, which is where the ground
        // link belongs.
        let (left, right) = pane::pane_for("moc", dividers());
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
        let mut editor = Editor::open(&where_.path().join("a.yaml"), dividers());
        editor.edit(Edit::AddNode {
            name: "to_kelvin".into(),
            node_type: "compute".into(),
            home: None,
        });
        seed(&mut editor);
        assert!(editor.document().node("moc").is_none());

        // One somebody has emptied, which has a history behind it.
        let mut editor = Editor::open(&where_.path().join("b.yaml"), dividers());
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
        let mut editor = Editor::open(&damaged, dividers());
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
            let editor = Editor::open(&root.join(format!("example.{extension}")), dividers());
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
                // The same ends the canvas draws, which is the point of
                // asking here: see ends_of.
                let through = route::runs_through(ends_of(document, link, &None), &obstacles);
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
        the_group_window_offers_every_group();
        leaving_offers_to_write_what_the_files_do_not_have();
        the_menu_opens_the_tcspecial_parameters();
        a_payloads_panel_carries_its_simulation_settings();
        the_menu_opens_the_simulators_groups();
        a_group_window_sets_every_payload_in_its_group();
        a_payload_stays_in_the_payload_working_pane();
        the_divider_can_be_dragged();
        a_node_follows_the_pointer_and_reports_where_it_went();
        an_existing_configuration_is_on_the_canvas();
        a_link_is_drawn_by_dragging_from_a_terminal();
        a_link_is_deleted_by_dragging_its_end_onto_the_canvas();
        placing_a_described_node_answers_what_it_describes();
        an_error_is_put_where_it_cannot_be_missed();
    }

    /// A failed write puts a window in front of the person, once.
    ///
    /// The red line in the bar is easy to work past without reading, and the
    /// thing it says -- the file does not have your last edit -- is the one
    /// thing in this editor that silence makes worse. So it is also said in a
    /// window. Once per thing, though: with a directory that cannot be written
    /// every edit fails, and a window that came back on each of them would be
    /// a window nobody could type past.
    fn an_error_is_put_where_it_cannot_be_missed() {
        use std::os::unix::fs::PermissionsExt;
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let locked = where_.path().join("locked");
        std::fs::create_dir(&locked).unwrap();
        let path = locked.join("vehicle.yaml");

        let editor = Rc::new(RefCell::new(Editor::open(&path, dividers())));
        let ui = MainWindow::new().expect("a window");
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            drawing: Rc::new(RefCell::new(None)),
            divider: Rc::new(std::cell::Cell::new(dividers())),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));
        alerting(&views);

        let shown = || {
            let views = views.borrow();
            let window = views.alert.as_ref().expect("a window to say it with");
            (
                window.window().is_visible(),
                window.get_trouble().to_string(),
            )
        };

        // A write that worked says nothing.
        editor.borrow_mut().edit(Edit::AddNode {
            name: "to_kelvin".into(),
            node_type: "compute".into(),
            home: None,
        });
        views.borrow().refresh(&editor.borrow());
        assert!(!editor.borrow().unsaved());
        assert!(
            !shown().0,
            "a session with nothing wrong should be left alone"
        );

        // One that failed is put in front of them, and says why.
        let shut = |mode: u32| {
            let mut permissions = std::fs::metadata(&locked).unwrap().permissions();
            permissions.set_mode(mode);
            std::fs::set_permissions(&locked, permissions).unwrap();
        };
        shut(0o500);
        editor.borrow_mut().edit(Edit::AddNode {
            name: "mean_temp".into(),
            node_type: "compute".into(),
            home: None,
        });
        views.borrow().refresh(&editor.borrow());
        assert!(editor.borrow().unsaved(), "the write should have failed");
        let (visible, said) = shown();
        assert!(visible, "a failed write should be said in a window");
        assert!(!said.is_empty(), "and should say what failed");

        // Acknowledged, it goes; and the same trouble on the next edit does
        // not bring it back.
        views.borrow().alert.as_ref().unwrap().invoke_acknowledged();
        assert!(!shown().0);
        editor.borrow_mut().edit(Edit::AddNode {
            name: "to_fahrenheit".into(),
            node_type: "compute".into(),
            home: None,
        });
        views.borrow().refresh(&editor.borrow());
        assert!(
            !shown().0,
            "the same trouble, already read, should not come back"
        );
        // The bar still says it, which is the state and not the moment.
        assert!(editor.borrow().trouble().is_some());

        // Put right and then wrong again: that is a new thing to say.
        shut(0o700);
        editor.borrow_mut().edit(Edit::AddNode {
            name: "to_rankine".into(),
            node_type: "compute".into(),
            home: None,
        });
        views.borrow().refresh(&editor.borrow());
        assert!(
            !editor.borrow().unsaved(),
            "{:?}",
            editor.borrow().trouble()
        );
        shut(0o500);
        editor.borrow_mut().edit(Edit::AddNode {
            name: "to_celsius".into(),
            node_type: "compute".into(),
            home: None,
        });
        views.borrow().refresh(&editor.borrow());
        assert!(shown().0, "a fresh failure should be said again");

        // Left writable, or the directory cannot be removed.
        shut(0o700);
    }


    /// Dragging a link's end onto empty canvas deletes it.
    ///
    /// Through the window, because that is the half the unit tests cannot
    /// reach: a data sink has to answer a press at all -- it did not, links
    /// being drawn only from sources -- and the line has to come off the
    /// drawing while the hand is moving. It is also the only way to delete a
    /// link, which is why it is worth a test of its own.
    fn a_link_is_deleted_by_dragging_its_end_onto_the_canvas() {
        use i_slint_backend_testing as testing;
        use slint::platform::{PointerEventButton, WindowEvent};
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Rc::new(RefCell::new(Editor::open(
            &where_.path().join("pulled.yaml"),
            dividers(),
        )));
        for (name, node_type, at) in [
            ("ground", "moc", (16, 120)),
            ("sensor", "i2c", (dividers().vehicle + 24, 120)),
        ] {
            editor.borrow_mut().edit(Edit::AddNode {
                name: name.to_string(),
                node_type: node_type.to_string(),
                home: None,
            });
            editor.borrow_mut().edit(Edit::MoveNode {
                node: name.to_string(),
                was: (0, 0),
                now: at,
            });
        }
        editor.borrow_mut().edit(Edit::AddLink {
            link: journal::Link::joining("sensor", Some("to_moc"), "ground", None),
        });

        let ui = MainWindow::new().expect("a window");
        furnish!(&ui, descriptions());
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            drawing: Rc::new(RefCell::new(None)),
            divider: Rc::new(std::cell::Cell::new(dividers())),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));
        let chosen: Rc<RefCell<BTreeMap<String, String>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let drawing: Rc<RefCell<Option<Drawing>>> = Rc::new(RefCell::new(None));
        views.borrow_mut().chosen = chosen.clone();
        views.borrow_mut().drawing = drawing.clone();
        wire(&ui, &editor, &views, &drawing, &chosen, &None);
        views.borrow().refresh(&editor.borrow());
        ui.show().expect("the window shows");

        let canvas = testing::ElementHandle::find_by_element_id(&ui, "Surface::canvas")
            .next()
            .expect("the window has a canvas");
        let origin = canvas.absolute_position();
        let middle = |node: &str, node_type: &str, at: (i32, i32), terminal: &str| {
            let (x, y, width, height) =
                style::terminal_box(node_type, at, terminal).expect("a terminal");
            let _ = node;
            slint::LogicalPosition::new(
                origin.x + x + width / 2.0,
                origin.y + y + height / 2.0,
            )
        };

        // Taken hold of at the ground link's own sink, which is where this
        // link arrives.
        let from = middle("ground", "moc", (16, 120), "from_payload");
        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position: from });
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position: from,
            button: PointerEventButton::Left,
        });

        // While the hand is moving, the line is off the drawing: a line that
        // stayed put would say the gesture had not started.
        assert_eq!(ui.get_links().row_count(), 0, "the held line is not drawn");

        // And let go of over empty canvas: in the middle of the vehicle pane,
        // where neither node is, and inside the window so the toolkit
        // delivers the event.
        let onto = slint::LogicalPosition::new(
            origin.x + dividers().moc as f32 + 100.0,
            origin.y + 380.0,
        );
        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position: onto });
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position: onto,
            button: PointerEventButton::Left,
        });

        assert!(
            drawing.borrow().is_none(),
            "the release should have reached the handler"
        );
        assert!(
            editor.borrow().document().links.is_empty(),
            "dragging a link's end onto the canvas should delete it: {:?}",
            editor.borrow().document().links
        );
        // Undoable, like every other edit.
        editor.borrow_mut().undo();
        assert_eq!(editor.borrow().document().links.len(), 1);
    }

    /// Placing a described node puts down a node that already is one.
    ///
    /// The whole point of the description tree at the palette: pressing `mean`
    /// should give a compute node whose function is mean, named `mean`, with a
    /// panel asking for a mean's attributes. Before the tree it gave a node
    /// called `compute` with nothing answered, and a person had to know that
    /// `function` was a word and that `mean` was one of its values.
    fn placing_a_described_node_answers_what_it_describes() {
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Rc::new(RefCell::new(Editor::open(
            &where_.path().join("placed.yaml"),
            dividers(),
        )));

        let ui = MainWindow::new().expect("a window");
        furnish!(&ui, descriptions());
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            drawing: Rc::new(RefCell::new(None)),
            divider: Rc::new(std::cell::Cell::new(dividers())),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));
        let chosen: Rc<RefCell<BTreeMap<String, String>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let drawing: Rc<RefCell<Option<Drawing>>> = Rc::new(RefCell::new(None));
        views.borrow_mut().chosen = chosen.clone();
        views.borrow_mut().drawing = drawing.clone();
        wire(&ui, &editor, &views, &drawing, &chosen, &None);
        views.borrow().refresh(&editor.borrow());

        // The entry the palette offers, pressed.
        let mean = rows_of(Pane::Vehicle)
            .into_iter()
            .find(|row| row.node.name == "mean")
            .expect("the palette offers a mean");
        ui.invoke_place_node(mean.node.name.clone(), mean.node.node_type.clone(), mean.sets);

        let placed = editor
            .borrow()
            .document()
            .node("mean")
            .cloned()
            .expect("a node called mean");
        assert_eq!(placed.node_type, "compute");
        assert_eq!(
            placed.attributes.get("function").map(String::as_str),
            Some("mean"),
            "a mean placed from the palette is a mean"
        );

        // And the panel asks it for a mean's attributes rather than for every
        // attribute any computation has.
        chosen.borrow_mut().insert(String::new(), "mean".to_string());
        let (_, _, _, fields) = panel_of(&editor.borrow(), &None, Some("mean"));
        let asked: Vec<String> = fields
            .iter()
            .filter(|field| !field.is_heading)
            .map(|field| field.name.to_string())
            .collect();
        assert_eq!(asked, vec!["function".to_string(), "inputs".to_string()]);

        // Undoable in one step per edit, as everything is: the attribute the
        // description answered is an edit of its own, so taking it back leaves
        // a compute node with nothing answered rather than no node at all.
        editor.borrow_mut().undo();
        assert_eq!(
            editor
                .borrow()
                .document()
                .node("mean")
                .expect("still there")
                .attributes
                .get("function"),
            Some(&"mean".to_string()),
            "the first undo takes back the placing and not the function"
        );
    }

    /// Dragging from one terminal to another makes the link.
    ///
    /// The gesture through the window, which is the only way to find out
    /// whether the terminals can be reached at all: they are drawn inside the
    /// node, over the area a drag is taken from, and a press that went to the
    /// grip instead would move the node and draw nothing. What `joined`
    /// decides is tested on its own above; this is the wiring around it.
    fn a_link_is_drawn_by_dragging_from_a_terminal() {
        use i_slint_backend_testing as testing;
        use slint::platform::{PointerEventButton, WindowEvent};
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Rc::new(RefCell::new(Editor::open(
            &where_.path().join("drawn.yaml"),
            dividers(),
        )));
        // A payload in the payload working pane and the ground link in its
        // own, which is where the two of them belong -- and the arrangement
        // the terminals are drawn for: the readings leave the payload's lower
        // left corner and arrive at the ground link's lower right.
        for (name, node_type, at) in [
            ("sensor", "i2c", (dividers().vehicle + 24, 120)),
            ("ground", "moc", (16, 120)),
        ] {
            editor.borrow_mut().edit(Edit::AddNode {
                name: name.to_string(),
                node_type: node_type.to_string(),
                home: None,
            });
            editor.borrow_mut().edit(Edit::MoveNode {
                node: name.to_string(),
                was: (0, 0),
                now: at,
            });
        }

        let ui = MainWindow::new().expect("a window");
        furnish!(&ui, descriptions());
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            drawing: Rc::new(RefCell::new(None)),
            divider: Rc::new(std::cell::Cell::new(dividers())),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));
        let chosen: Rc<RefCell<BTreeMap<String, String>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let drawing: Rc<RefCell<Option<Drawing>>> = Rc::new(RefCell::new(None));
        views.borrow_mut().chosen = chosen.clone();
        views.borrow_mut().drawing = drawing.clone();
        wire(&ui, &editor, &views, &drawing, &chosen, &None);
        views.borrow().refresh(&editor.borrow());
        ui.show().expect("the window shows");

        let canvas = testing::ElementHandle::find_by_element_id(&ui, "Surface::canvas")
            .next()
            .expect("the window has a canvas");
        let origin = canvas.absolute_position();

        // The middle of a terminal's box, where the pointer goes.
        let middle = |node: &str, terminal: &str| {
            let at = editor.borrow().document().node(node).expect("the node").at;
            let node_type = editor
                .borrow()
                .document()
                .node(node)
                .expect("the node")
                .node_type
                .clone();
            let (x, y, width, height) =
                style::terminal_box(&node_type, at, terminal).expect("a terminal");
            slint::LogicalPosition::new(
                origin.x + x + width / 2.0,
                origin.y + y + height / 2.0,
            )
        };

        let from = middle("sensor", "to_moc");
        let to = middle("ground", "from_payload");

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

        let links = editor.borrow().document().links.clone();
        assert_eq!(
            links.len(),
            1,
            "dragging from a terminal to one that pairs with it should make one \
             link: {links:?}"
        );
        assert_eq!((links[0].from.as_str(), links[0].to.as_str()), ("sensor", "ground"));
        // The end it leaves by is named, that being the attribute the link is
        // written under; the end it arrives at is not, the ground link having
        // one data sink.
        assert_eq!(links[0].origin.as_deref(), Some("to_moc"));
        assert_eq!(links[0].input, None);

        // And the node did not move: a press on a terminal draws a link, and
        // the drag the same press would otherwise have started is what the
        // terminals sit on top of.
        assert_eq!(
            editor.borrow().document().node("sensor").expect("it").at,
            (dividers().vehicle + 24, 120)
        );

        // A pairing that is not allowed makes nothing: a to_moc let go of
        // over a to_payload, which is a data source and not a place for a
        // link to arrive.
        let to = middle("ground", "to_payload");
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
        assert_eq!(
            editor.borrow().document().links.len(),
            1,
            "a gesture between terminals that do not pair should draw nothing"
        );
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
            let editor = Rc::new(RefCell::new(Editor::open(&path, dividers())));
            assert_eq!(
                editor.borrow().trouble(),
                None,
                "example.{extension} should open without complaint"
            );

            let ui = MainWindow::new().expect("a window");
            furnish!(&ui, descriptions());
            put(&ui, &editor.borrow(), &None, None, dividers(), None);

            let names: Vec<String> = (0..ui.get_nodes().row_count())
                .filter_map(|row| ui.get_nodes().row_data(row))
                .map(|node| node.name.to_string())
                .collect();
            // In the order the file's sections are read: the ground link,
            // then the vehicle's own graph, then what it carries -- which is
            // the three working panes left to right.
            assert_eq!(
                names,
                vec!["moc".to_string(), "payload".to_string()],
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
            dividers(),
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
        // Somewhere inside its own pane rather than in a corner: a node at
        // the origin is a node whose own position cannot be told from the
        // canvas's, and one left of its pane would be brought into it by the
        // drag rather than moved by it.
        editor.borrow_mut().edit(Edit::MoveNode {
            node: "to_kelvin".to_string(),
            was: (0, 0),
            now: (dividers().moc + 120, 88),
        });
        let was = editor
            .borrow()
            .document()
            .node("to_kelvin")
            .expect("the node")
            .at;

        let ui = MainWindow::new().expect("a window");
        furnish!(&ui, descriptions());
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            drawing: Rc::new(RefCell::new(None)),
            divider: Rc::new(std::cell::Cell::new(dividers())),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));
        let chosen: Rc<RefCell<BTreeMap<String, String>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let drawing: Rc<RefCell<Option<Drawing>>> = Rc::new(RefCell::new(None));
        views.borrow_mut().chosen = chosen.clone();
        views.borrow_mut().drawing = drawing.clone();
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
            dividers(),
        )));
        editor.borrow_mut().edit(Edit::AddNode {
            name: "temp_fore".to_string(),
            node_type: "i2c".to_string(),
            home: None,
        });
        // In its own pane, a little in from the divider.
        let was = (dividers().vehicle + 24, 88);
        editor.borrow_mut().edit(Edit::MoveNode {
            node: "temp_fore".to_string(),
            was: (0, 0),
            now: was,
        });

        let ui = MainWindow::new().expect("a window");
        furnish!(&ui, descriptions());
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            drawing: Rc::new(RefCell::new(None)),
            divider: Rc::new(std::cell::Cell::new(dividers())),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));
        let chosen: Rc<RefCell<BTreeMap<String, String>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let drawing: Rc<RefCell<Option<Drawing>>> = Rc::new(RefCell::new(None));
        views.borrow_mut().chosen = chosen.clone();
        views.borrow_mut().drawing = drawing.clone();
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
            (dividers().vehicle, was.1 + by.1 as i32),
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
            dividers(),
        )));

        let ui = MainWindow::new().expect("a window");
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            drawing: Rc::new(RefCell::new(None)),
            divider: Rc::new(std::cell::Cell::new(dividers())),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));
        let chosen: Rc<RefCell<BTreeMap<String, String>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let drawing: Rc<RefCell<Option<Drawing>>> = Rc::new(RefCell::new(None));
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
        window.invoke_parameter_set("tcs_cmd.recv".into(), "0.0.0.0:4000".into());
        assert_eq!(
            editor
                .borrow()
                .document()
                .tcspecial
                .get("tcs_cmd.recv")
                .map(String::as_str),
            Some("0.0.0.0:4000")
        );
        assert_eq!(editor.borrow().undo_depth(), depth + 1);

        // And the window says so the next time it is drawn: the field is
        // filled and no longer marked.
        let interface = (0..window.get_fields().row_count())
            .filter_map(|row| window.get_fields().row_data(row))
            .find(|f| f.name == "tcs_cmd.recv")
            .expect("the command interface field");
        assert_eq!(interface.value, "0.0.0.0:4000");

        assert!(editor.borrow_mut().undo());
        assert!(!editor
            .borrow()
            .document()
            .tcspecial
            .contains_key("tcs_cmd.recv"));
    }

    /// The group window offers every group there is.
    ///
    /// Which group is being edited is the window's own business. It was a
    /// menu in the bar, with the menu's own name as its first row so the
    /// control did not read as a setting that said "every_second"; a window
    /// that holds the form can hold the chooser, and the File menu now says
    /// only "Edit payload groups".
    fn the_group_window_offers_every_group() {
        use slint::Model;
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Rc::new(RefCell::new(Editor::open(
            &where_.path().join("vehicle.yaml"),
            dividers(),
        )));
        for (name, interval) in [("cabin_temps", "3s"), ("bay", "5s")] {
            editor.borrow_mut().edit(Edit::AddGroup {
                name: name.into(),
                interval: interval.into(),
            });
        }

        let ui = MainWindow::new().expect("a window");
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            drawing: Rc::new(RefCell::new(None)),
            divider: Rc::new(std::cell::Cell::new(dividers())),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));

        show_group(&editor, &views);
        let window = views
            .borrow()
            .group
            .as_ref()
            .map(|window| window.as_weak())
            .expect("the menu should have opened the groups")
            .upgrade()
            .expect("and left them open");

        let offered: Vec<String> = (0..window.get_choices().row_count())
            .filter_map(|row| window.get_choices().row_data(row))
            .map(|name| name.to_string())
            .collect();
        assert_eq!(offered, vec!["bay".to_string(), "cabin_temps".to_string()]);
        // Opened on one of them rather than on an invitation to choose: most
        // configurations have one group, and two presses to reach it would be
        // one too many.
        assert_eq!(window.get_group(), "bay");

        // Choosing another shows that one.
        window.invoke_choose("cabin_temps".into());
        assert_eq!(window.get_group(), "cabin_temps");
        assert_eq!(window.get_interval(), "3s");

        // And a new one is made with an interval, because a group without one
        // reads nothing.
        window.invoke_add();
        assert_eq!(window.get_group(), "group");
        assert_eq!(window.get_interval(), FIRST_INTERVAL);
    }

    /// Leaving offers to write what the files do not have, and only then.
    ///
    /// Normally there is nothing to offer -- every edit is written as it is
    /// made -- so Exit exits. What this covers is the session where a write
    /// failed: leaving a session's work in a process that is ending is the one
    /// way this editor could lose anything.
    fn leaving_offers_to_write_what_the_files_do_not_have() {
        use std::os::unix::fs::PermissionsExt;
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let locked = where_.path().join("locked");
        std::fs::create_dir(&locked).unwrap();
        let path = locked.join("vehicle.yaml");

        let editor = Rc::new(RefCell::new(Editor::open(&path, dividers())));
        let ui = MainWindow::new().expect("a window");
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            drawing: Rc::new(RefCell::new(None)),
            divider: Rc::new(std::cell::Cell::new(dividers())),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));

        // Nothing owed: leaving asks nothing. (The loop is not running in a
        // test, so quitting it is a no-op and the absence of the window is
        // what there is to check.)
        editor.borrow_mut().edit(Edit::AddNode {
            name: "to_kelvin".into(),
            node_type: "compute".into(),
            home: None,
        });
        assert!(!editor.borrow().unsaved());
        leave(&editor, &views);
        assert!(
            views.borrow().leaving.is_none(),
            "a session that owes nothing should not be asked"
        );

        // A write that failed, so there is something to offer.
        let shut = |mode: u32| {
            let mut permissions = std::fs::metadata(&locked).unwrap().permissions();
            permissions.set_mode(mode);
            std::fs::set_permissions(&locked, permissions).unwrap();
        };
        shut(0o500);
        editor.borrow_mut().edit(Edit::AddNode {
            name: "mean_temp".into(),
            node_type: "compute".into(),
            home: None,
        });
        assert!(editor.borrow().unsaved(), "the write should have failed");

        leave(&editor, &views);
        let window = views
            .borrow()
            .leaving
            .as_ref()
            .map(|window| window.as_weak())
            .expect("the offer should be made")
            .upgrade()
            .expect("and still be there");
        // It says why, which is the thing a person needs in order to answer.
        assert!(!window.get_trouble().is_empty());

        // Staying takes the offer away and leaves the work where it is.
        window.invoke_stayed();
        assert!(views.borrow().leaving.is_none());
        assert!(editor.borrow().unsaved());
        assert_eq!(editor.borrow().document().nodes.len(), 2);

        // Put right, and saving on the way out writes it.
        shut(0o700);
        leave(&editor, &views);
        let window = views
            .borrow()
            .leaving
            .as_ref()
            .map(|window| window.as_weak())
            .expect("asked again")
            .upgrade()
            .unwrap();
        window.invoke_saved();
        assert!(!editor.borrow().unsaved(), "{:?}", editor.borrow().trouble());
        let written = std::fs::read_to_string(&path).expect("the file");
        assert!(written.contains("mean_temp"), "{written}");
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
            dividers(),
        )));
        {
            let mut editor = editor.borrow_mut();
            // Inside the vehicle working pane, which is where a computation
            // belongs: a click lands on a node where it is drawn.
            let left = dividers().moc;
            for (name, node_type, at) in [
                ("to_kelvin", "compute", (left + 120, 88)),
                ("mean_temp", "compute", (left + 120, 240)),
                // And a payload, in its own pane. Of a different type on
                // purpose: a click has to open the window for whatever was
                // clicked, and a payload is the one a person reaches for
                // first.
                ("sensor", "i2c", (dividers().vehicle + 24, 88)),
            ] {
                editor.edit(Edit::AddNode {
                    name: name.to_string(),
                    node_type: node_type.to_string(),
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
        furnish!(&ui, descriptions());
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            drawing: Rc::new(RefCell::new(None)),
            divider: Rc::new(std::cell::Cell::new(dividers())),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));
        let chosen: Rc<RefCell<BTreeMap<String, String>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let drawing: Rc<RefCell<Option<Drawing>>> = Rc::new(RefCell::new(None));
        views.borrow_mut().chosen = chosen.clone();
        views.borrow_mut().drawing = drawing.clone();
        wire(&ui, &editor, &views, &drawing, &chosen, &None);
        views.borrow().refresh(&editor.borrow());
        ui.show().expect("the window shows");

        let canvas = testing::ElementHandle::find_by_element_id(&ui, "Surface::canvas")
            .next()
            .expect("the window has a canvas");
        let at = canvas.absolute_position();
        // Pressed and released at one place, which is a click.
        let click = |node: (i32, i32), node_type: &str| {
            let (width, height) = size_of(node_type);
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

        click((dividers().moc + 120, 88), "compute");
        assert!(
            views.borrow().attributes.contains_key("to_kelvin"),
            "a click should open what the node says: {:?}",
            views.borrow().attributes.keys().collect::<Vec<_>>()
        );
        // And it moved nothing, so nothing is in the history.
        // Three nodes placed and moved, which is six edits: a click adds
        // none.
        assert_eq!(editor.borrow().undo_depth(), 6, "a click is not an edit");

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
        click((dividers().moc + 120, 240), "compute");
        assert_eq!(
            views.borrow().attributes.len(),
            2,
            "a second node should open a second window: {:?}",
            views.borrow().attributes.keys().collect::<Vec<_>>()
        );
        assert!(views.borrow().attributes.contains_key("to_kelvin"));

        // Clicking the same node again raises its window rather than making a
        // second one saying the same thing.
        click((dividers().moc + 120, 88), "compute");
        assert_eq!(views.borrow().attributes.len(), 2);

        // A node whose position is not on the grid opens on the first click
        // too, which it did not: a click used to be told from a drag by
        // whether the position it *resolves to* was where the node already
        // was, and a node off the grid -- a file somebody hand-edited, a
        // configuration from a build that drew the panes differently --
        // resolves to somewhere else. So the click moved it and the second
        // click opened it. A click is a gesture that moved nothing.
        {
            let mut editor = editor.borrow_mut();
            editor.edit(Edit::AddNode {
                name: "askew".to_string(),
                node_type: "i2c".to_string(),
                home: None,
            });
            // Three pixels off the eight-pixel grid, and in its own pane.
            editor.edit(Edit::MoveNode {
                node: "askew".to_string(),
                was: (0, 0),
                now: (dividers().vehicle + 27, 203),
            });
        }
        views.borrow().refresh(&editor.borrow());
        let before = editor.borrow().undo_depth();

        click((dividers().vehicle + 27, 203), "i2c");
        assert!(
            views.borrow().attributes.contains_key("askew"),
            "a node off the grid should open on the first click: {:?}",
            views.borrow().attributes.keys().collect::<Vec<_>>()
        );
        assert_eq!(
            editor.borrow().undo_depth(),
            before,
            "and the click should not have moved it"
        );
        assert_eq!(
            editor.borrow().document().node("askew").expect("it").at,
            (dividers().vehicle + 27, 203),
            "nor snapped it"
        );

        // And a payload opens its own, which is the one a person reaches for
        // first: a sensor is where an address and a bus are typed.
        click((dividers().vehicle + 24, 88), "i2c");
        assert!(
            views.borrow().attributes.contains_key("sensor"),
            "clicking a payload should open what it says: {:?}",
            views.borrow().attributes.keys().collect::<Vec<_>>()
        );
        let payload = views
            .borrow()
            .attributes
            .get("sensor")
            .map(|window| window.as_weak())
            .unwrap()
            .upgrade()
            .expect("still open");
        assert_eq!(payload.get_node_type(), "i2c");
        // With the fields an i2c node is asked for, so somebody can type a
        // bus and an address into it.
        let asked: Vec<String> = (0..payload.get_fields().row_count())
            .filter_map(|row| payload.get_fields().row_data(row))
            .map(|field| field.name.to_string())
            .collect();
        assert!(asked.contains(&"path".to_string()), "{asked:?}");
        assert!(asked.contains(&"address".to_string()), "{asked:?}");

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

        // And closing takes a press of its own, and takes only that one: the
        // other two windows are still open.
        window.invoke_closed();
        assert_eq!(
            views.borrow().attributes.keys().collect::<Vec<_>>(),
            vec!["askew", "mean_temp", "sensor"],
            "Close should shut one window and leave the others"
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
            dividers(),
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
            drawing: Rc::new(RefCell::new(None)),
            divider: Rc::new(std::cell::Cell::new(dividers())),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
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
        assert!(!editor.borrow().document().simulator.as_ref().unwrap().payloads
            ["temp_fore"]
            .contains_key("drop_percent"));
    }

    /// The menu opens the simulator's groups, and typing in them reaches its
    /// file.
    fn the_menu_opens_the_simulators_groups() {
        use slint::Model;
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Rc::new(RefCell::new(Editor::open(
            &where_.path().join("vehicle.yaml"),
            dividers(),
        )));
        editor
            .borrow_mut()
            .open_simulator(&where_.path().join("vehiclesim.yaml"));

        let ui = MainWindow::new().expect("a window");
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            drawing: Rc::new(RefCell::new(None)),
            divider: Rc::new(std::cell::Cell::new(dividers())),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));
        let chosen: Rc<RefCell<BTreeMap<String, String>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let drawing: Rc<RefCell<Option<Drawing>>> = Rc::new(RefCell::new(None));
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
            dividers(),
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
            drawing: Rc::new(RefCell::new(None)),
            divider: Rc::new(std::cell::Cell::new(dividers())),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));

        // The only group there is, so the window opens on it.
        show_group(&editor, &views);

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
        furnish!(&window, descriptions());
        window.set_moc_divider(dividers().moc as f32);
        window.set_vehicle_divider(dividers().vehicle as f32);

        // Both of them, because there are two and each should report its own
        // gesture: a window that wired one callback to both lines would pass
        // this test for the wrong reason.
        let moved: Rc<Cell<Option<f32>>> = Rc::new(Cell::new(None));
        let moc_moved: Rc<Cell<Option<f32>>> = Rc::new(Cell::new(None));
        {
            let moved = moved.clone();
            window.on_vehicle_divider_moved(move |dx| moved.set(Some(dx)));
        }
        {
            let moc_moved = moc_moved.clone();
            window.on_moc_divider_moved(move |dx| moc_moved.set(Some(dx)));
        }
        window.show().expect("the window shows");

        let canvas = testing::ElementHandle::find_by_element_id(&window, "Surface::canvas")
            .next()
            .expect("the window has a canvas");
        let at = canvas.absolute_position();

        // On the line, halfway down it, which is where the handle is.
        let from =
            slint::LogicalPosition::new(at.x + dividers().vehicle as f32, at.y + 200.0);
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
        assert_eq!(
            moc_moved.get(),
            None,
            "dragging the vehicle divider moved the moc divider too"
        );

        // And the other line, which is its own control.
        let from =
            slint::LogicalPosition::new(at.x + dividers().moc as f32, at.y + 200.0);
        let by = 40.0;
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
        let said = moc_moved
            .get()
            .expect("dragging the moc divider should report how far it went");
        assert!(
            (said - by).abs() < 1.0,
            "the moc divider dragged by {by} reported {said}"
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
