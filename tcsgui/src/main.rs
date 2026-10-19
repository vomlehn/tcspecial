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
//!   every other in at least two of outline, the line around it, border width
//!   and size;
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
mod nodes;
mod style;
mod subdiagram;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use slint::{ModelRc, VecModel};

use document::{Document, Fault};
use tcslibgs::graph::check::Problem;
use editor::Editor;
use pane::{
    confine, dividers_for, edges_of, is_payload, pane_of, percents_for, size_of, Dividers,
    Pane, WORKING_WIDTH,
};
use journal::{Edit, Link};
use subdiagram::{Home, Qualified};
use style::{Edge, Face, NodeType, Outline, Weight, ERROR_TINT};

slint::include_modules!();

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
        // The payload divider: between the payload pane and the vehicle's.
        Pane::Payloads => (
            Pane::Payloads,
            Pane::Vehicle,
            step,
            dividers.vehicle - step,
        ),
        // And the vehicle divider, between the vehicle's and the moc pane.
        _ => (
            Pane::Vehicle,
            Pane::Moc,
            dividers.payloads + step,
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
/// The upper right of the moc working pane, clear of the pane's own label: the
/// moc pane being the rightmost, the node is held off its right edge. Where a
/// node may be, at 209 millimetres across and 9 down -- see
/// `pane::NODE_STEP_MM` and the test below, which holds it there.
const MOC_PLACE: (i32, i32) = (790, 34);
// Clear of the strip the panes' names are written in, as everything the
// editor places is: see `pane::NAMES`, which the test below holds this to.
const _: () = assert!(MOC_PLACE.1 >= pane::NAMES.0 + pane::NAMES.1);

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
    /// And how the selection column is divided, as two lists' percentages of
    /// it: the moc list's and the vehicle list's, the payload list taking what
    /// is left. By list rather than by position, as the settings keep them.
    ///
    /// Percentages rather than pixels, because the column is as tall as the
    /// window and a share is what somebody chose -- the same distinction
    /// `settings.rs` draws for the working panes, kept in the same file.
    /// Shared between windows for the reason the dividers above are.
    lists: Rc<std::cell::Cell<(i32, i32)>>,
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
    /// The list of what might be wrong, while it is open.
    ///
    /// One window for both lists -- what has not been looked at and what was
    /// set aside -- pointed at either by its own `aside` flag. Held so that
    /// pressing the button twice does not stack two of them, and so that the
    /// list can be refreshed under somebody's eyes as they act on it.
    concerns: Option<ConcernsWindow>,
    /// The question a node's Delete asks, while it is being asked.
    ///
    /// Held so that pressing Delete twice does not stack two of them, and so
    /// that answering one takes it away. One at a time: it names the node it
    /// is about, and a second question about another node would leave a person
    /// answering whichever window happened to be on top.
    deleting: Option<DeleteWindow>,
    /// The question about two nodes that share a name, while it is being
    /// asked. See `settle_names`.
    names: Option<ProblemsWindow>,
    /// The window that says what went wrong, and what it last said.
    ///
    /// One window, re-pointed: two of them side by side would be two copies of
    /// one sentence. What it last said is kept so that an error is shown once
    /// -- every edit redraws every window, and a window that popped up again
    /// on each of them would be a window nobody could type past.
    alert: Option<AlertWindow>,
    alerted: Rc<RefCell<String>>,
    /// Which faults have been put in front of somebody already.
    ///
    /// By the sentence, because that is what was shown. A fault that is no
    /// longer there is forgotten, so one put right and made again is said
    /// again -- see `alert_faults`.
    faulted: Rc<RefCell<std::collections::BTreeSet<String>>>,
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
        if said != *self.alerted.borrow() {
            *self.alerted.borrow_mut() = said.clone();
            if !said.is_empty() {
                self.say(
                    "Something did not work.",
                    &said,
                    "The session is still open. Put the cause right and press \
                     Save, or carry on editing -- nothing has been lost.",
                );
            }
        }
        self.alert_faults(editor);
    }

    /// And what is wrong with the configuration, the first time it is wrong.
    ///
    /// A fault is an error: two nodes of one name, two lines into one input, a
    /// value a rule refuses. The configuration will not fly with one in it, so
    /// `tcsverify` fails and the flight software would refuse the file -- and
    /// until now the editor said it in red on the node and nowhere else, which
    /// is a sentence inside a box forty-eight pixels tall.
    ///
    /// Once per fault, and the set of what has been said is kept so that a
    /// fault put right and made again is said again: every edit refreshes
    /// every window, and a window that came back on each of them would be one
    /// nobody could type past. The red on the node stays, and so does the
    /// sentence in the node's own window; this is the moment it appeared.
    fn alert_faults(&self, editor: &Editor) {
        let faults: Vec<String> = editor
            .faults()
            .iter()
            .map(|fault| fault.describe())
            .collect();

        // What is new, and what has gone: a fault that is no longer there is
        // forgotten, which is what lets it be said again if it comes back.
        let mut alerted = self.faulted.borrow_mut();
        alerted.retain(|said| faults.contains(said));
        let fresh: Vec<String> = faults
            .iter()
            .filter(|said| !alerted.contains(*said))
            .cloned()
            .collect();
        if fresh.is_empty() {
            return;
        }
        for said in &fresh {
            alerted.insert(said.clone());
        }
        drop(alerted);

        // Every one, in the order the checking found them.
        let said = fresh.join("; ");

        self.say(
            "Something is wrong with the configuration.",
            &said,
            "It is drawn in red on the node it is about, and said again in \
             that node's own window. Nothing has been lost, and the \
             configuration will not fly until it is put right.",
        );
    }

    /// Put one of those in front of somebody.
    fn say(&self, heading: &str, said: &str, advice: &str) {
        let Some(window) = &self.alert else { return };
        window.set_heading(heading.into());
        window.set_trouble(said.into());
        window.set_advice(advice.into());
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
        let lists = self.lists.get();
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
                lists,
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
                lists,
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
/// Whether this session was started with `--read-only`: see `main`.
///
/// Once, at startup, and then only read: a session is one or the other for
/// as long as it runs. Unset is a session that may edit, which is what every
/// test and every window made outside `main` is.
static READ_ONLY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Whether this session may change nothing.
fn read_only() -> bool {
    READ_ONLY.get().copied().unwrap_or(false)
}

/// Tell a window whether this session may change anything, which is what
/// greys the controls that would: see `Session` in the markup. Every window
/// is told as it is made, a global being one per window rather than one per
/// program.
macro_rules! session {
    ($window:expr) => {
        $window.global::<Session>().set_read_only(read_only())
    };
}

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

/// The files named on the command line, in order, and whether the session is
/// to be read-only.
///
/// The flag wherever it is, in any of its three spellings. Any other word
/// starting with a dash is refused rather than taken for a file name: a
/// mistyped flag would otherwise be a configuration created under that name.
/// More than two files is refused too, there being nothing for a third to be.
fn command_line(words: impl Iterator<Item = String>) -> Result<(Vec<String>, bool), String> {
    let mut files = Vec::new();
    let mut read_only = false;
    for word in words {
        match word.as_str() {
            "-r" | "--read-only" | "--readonly" => read_only = true,
            flag if flag.starts_with('-') && flag != "-" => {
                return Err(format!("there is no option {flag:?}"));
            }
            _ => files.push(word),
        }
    }
    if files.len() > 2 {
        return Err(format!(
            "{} files were named, and tcsgui takes a configuration and a simulator file",
            files.len()
        ));
    }
    Ok((files, read_only))
}

/// What is said when the command line is not one tcsgui takes.
const USAGE: &str = "usage: tcsgui [-r | --read-only] <configuration file> [<simulator file>]";

fn main() -> Result<(), slint::PlatformError> {
    // The flags, wherever they are, and the files in the order given. One
    // flag: `-r`, `--read-only` or `--readonly`, a session that shows a
    // configuration and changes nothing -- see `Editor::set_read_only` and
    // `command_line`.
    let (files, read_only_asked) = match command_line(std::env::args().skip(1)) {
        Ok(said) => said,
        Err(why) => {
            eprintln!("tcsgui: {why}");
            eprintln!("tcsgui: {USAGE}");
            std::process::exit(2);
        }
    };
    let _ = READ_ONLY.set(read_only_asked);
    let mut files = files.into_iter();

    let path = files.next().unwrap_or_else(|| {
        eprintln!("tcsgui: {USAGE}");
        eprintln!(
            "tcsgui: the file need not exist yet; it is written as edits are made, \
             and its history is kept beside it"
        );
        eprintln!(
            "tcsgui: a second file is the simulator's own configuration -- how fast \
             a simulated payload sends and what it does wrong -- and is edited from \
             the menu. It is written as YAML, so its name has to end in \".yaml\""
        );
        eprintln!(
            "tcsgui: -r, --read-only or --readonly opens the files to look at: \
             nothing in them can be changed and nothing is written"
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
    let simulator = files.next();
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
    // And how the selection column was divided, which is the same preference
    // the other way: see `Views::lists`.
    let lists = Rc::new(std::cell::Cell::new((
        settings.list_moc_percent,
        settings.list_vehicle_percent,
    )));

    // The histories of sessions nobody is coming back to: a week old, or whose
    // configuration is no longer there. Once, at startup, before anything is
    // opened -- see `crate::history::prune`.
    //
    // Not in a read-only session, which deletes nothing: the histories are
    // other sessions' business.
    if !read_only() {
        let gone = history::prune(std::time::SystemTime::now());
        if gone > 0 {
            eprintln!("tcsgui: {gone} old session histories were removed");
        }
    }

    // The configuration: see Editor::open.
    let editor = Rc::new(RefCell::new(Editor::open(
        std::path::Path::new(&path),
        divider.get(),
    )));
    // Before anything can be edited -- the seeding below included, which is
    // an edit like any other and so is not made.
    editor.borrow_mut().set_read_only(read_only());
    if let Some(simulator) = &simulator {
        editor
            .borrow_mut()
            .open_simulator(std::path::Path::new(simulator));
    }
    seed(&mut editor.borrow_mut());

    let ui = MainWindow::new()?;
    session!(&ui);
    furnish!(&ui, descriptions());

    let views = Rc::new(RefCell::new(Views {
        top: ui.as_weak(),
        open: BTreeMap::new(),
        chosen: Rc::new(RefCell::new(BTreeMap::new())),
        drawing: Rc::new(RefCell::new(None)),
        divider: divider.clone(),
        lists: lists.clone(),
        group: None,
        parameters: None,
        attributes: BTreeMap::new(),
        leaving: None,
        concerns: None,
        deleting: None,
        names: None,
        alert: None,
        alerted: Rc::new(RefCell::new(String::new())),
        faulted: Rc::new(RefCell::new(Default::default())),
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

    // What the file arrived with is counted as said before anything is drawn,
    // so that the alert leaves it to the window that asks about it: see
    // `settle_problems`, below. Drawing is what raises the alert, and the
    // first drawing is the next line.
    closing(&ui, &views);

    // The main window first, so that whatever came up as the editor started
    // is read over a drawing rather than instead of one -- and so that those
    // windows are in front of it: a window shown later is a window on top.
    // Nothing is painted until the event loop runs, which is after all of
    // this, so it is shown before it is filled and is never seen empty.
    //
    // It takes no presses while any of them is up. See `inert` in the markup,
    // set below once what came up is known.
    ui.show()?;

    settled(&editor, &views);
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
                Ok(window) => {
                    session!(&window);
                    window
                },
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
                views.borrow().lists.get(),
                None,
            );
            if let Err(e) = window.show() {
                eprintln!("tcsgui: {name} cannot be shown: {e}");
                return;
            }
            views.borrow_mut().open.insert(name, window);
        });
    }

    // And the question about whatever is wrong with the file as it arrived,
    // in front of the window that is already up: see `settle_problems`.
    //
    // The alert is told to keep quiet about those first, so that a file with
    // problems does not open with two windows about them -- one that offers
    // something and one that offers nothing. What the alert is for after this
    // is what goes wrong *while somebody works*: a fault they introduced,
    // which is news, where the state the file arrived in is the question this
    // window asks.
    settle_problems(&editor, &views);

    // And the main window takes no presses while any of that is still being
    // read: the question about the file's problems, the alert about a file
    // that could not be read. The drawing is there to be looked at -- it is
    // what the question is about -- and nothing in it can be edited behind a
    // window asking about the state the file arrived in. Each of those
    // windows gives the presses back as it goes: see `listen_when_read`.
    let waiting = startup_waiting(&views, None);
    ui.set_inert(waiting);
    // And those windows in front of it for as long as it waits for them,
    // whatever the window manager would have done with a window shown after
    // another: see `on_top`.
    on_top(&views, waiting);
    let outcome = slint::run_event_loop();
    let _ = ui.hide();

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
                        // A node exists, and then it is somewhere: two edits,
                        // recorded as one group so that one Undo takes the
                        // node away and one Redo puts it back where it was
                        // placed. A drag afterwards is a move of its own, so
                        // undoing that still leaves the node.
                        let mut edits = vec![Edit::AddNode {
                            name: name.clone(),
                            node_type: node_type.to_string(),
                            home: home.clone(),
                        }];
                        // What the description answers for it, before it is
                        // placed: a compute node from the `mean` entry is a
                        // mean from the moment it exists, so the panel asks
                        // for a mean's attributes and the checking holds it to
                        // a mean's rules.
                        for (attribute, value) in stated(&sets) {
                            edits.push(Edit::SetAttribute {
                                node: Qualified::of(&name, &home).absolute(),
                                attribute,
                                was: None,
                                now: Some(value),
                            });
                        }
                        edits.push(Edit::MoveNode {
                            node: Qualified::of(&name, &home).absolute(),
                            was: (0, 0),
                            now: at,
                        });
                        editor.edit(Edit::Group { edits });
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
                // Every node to the top of its own pane, and the view back at
                // the top of the sheet: see `Editor::arrange`.
                let editor = editor.clone();
                let views = views.clone();
                let window_for_scroll = window.as_weak();
                window.on_arrange(move || {
                    let dividers = views.borrow().divider.get();
                    editor.borrow_mut().arrange(dividers);
                    // And the view back at the top of the sheet, because that
                    // is where the nodes have gone. A view left halfway down
                    // one would make the command look as though it had emptied
                    // the canvas.
                    if let Some(window) = window_for_scroll.upgrade() {
                        window.set_canvas_offset(0.0);
                    }
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
                    window.on_payload_divider_moved(move |dx: f32| {
                        drag_divider(&editor, &views, Pane::Payloads, dx);
                    });
                }
                {
                    let editor = editor.clone();
                    let views = views.clone();
                    window.on_vehicle_divider_moved(move |dx: f32| {
                        drag_divider(&editor, &views, Pane::Vehicle, dx);
                    });
                }
                // And either line between the three selection lists, which is
                // the same gesture down the column: see drag_list_divider.
                {
                    let editor = editor.clone();
                    let views = views.clone();
                    window.on_payload_list_divider_moved(move |dy: f32, tall: f32| {
                        drag_list_divider(&editor, &views, Pane::Payloads, dy, tall);
                    });
                }
                {
                    let editor = editor.clone();
                    let views = views.clone();
                    window.on_vehicle_list_divider_moved(move |dy: f32, tall: f32| {
                        drag_list_divider(&editor, &views, Pane::Vehicle, dy, tall);
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
                let window_for_drawing = window.as_weak();
                let drawing_for_motion = drawing.clone();
                let editor_for_motion = editor.clone();
                let home_for_motion = home.clone();
                // Where the pointer is, while a line is being drawn from a
                // terminal. The line follows it: a gesture whose line stayed
                // at the terminal until it was let go of is a gesture with
                // nothing to aim.
                window.on_link_moved(move |x, y| {
                    let Some(window) = window_for_drawing.upgrade() else {
                        return;
                    };
                    let said = drawing_for_motion
                        .borrow()
                        .as_ref()
                        .and_then(|from| {
                            let editor = editor_for_motion.borrow();
                            let anchor = anchored(editor.document(), &home_for_motion, from)?;
                            Some(route::drawing(anchor, (x, y)))
                        })
                        .unwrap_or_default();
                    window.set_drawing_route(said.into());
                });
            }
            {
                let editor = editor.clone();
                let views = views.clone();
                let drawing = drawing.clone();
                let home = home.clone();
                let window_for_finish = window.as_weak();
                window.on_link_finished(move |x, y| {
                    // The line in hand goes first, whatever the gesture turns
                    // out to have done: it was never a link, and leaving it
                    // drawn would be a second line nobody made.
                    if let Some(window) = window_for_finish.upgrade() {
                        window.set_drawing_route(Default::default());
                    }
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
                let editor_for_menu = editor.clone();
                let views_for_menu = views.clone();
                let home_for_menu = home.clone();
                // Edit, from the menu a click on a node opens: the window
                // that shows what the node says, which is what a click used
                // to open on its own.
                window.on_node_edit(move |name| {
                    let absolute = Qualified::of(&name, &home_for_menu).absolute();
                    show_node(&editor_for_menu, &views_for_menu, &absolute);
                });
            }
            {
                let editor_for_concerns = editor.clone();
                let views_for_concerns = views.clone();
                // What might be wrong, and what was set aside: one window,
                // pointed at either list. See `show_concerns`.
                window.on_show_concerns(move || {
                    show_concerns(&editor_for_concerns, &views_for_concerns, false);
                });
            }
            {
                let editor_for_aside = editor.clone();
                let views_for_aside = views.clone();
                window.on_show_aside(move || {
                    show_concerns(&editor_for_aside, &views_for_aside, true);
                });
            }
            {
                let editor_for_delete = editor.clone();
                let views_for_delete = views.clone();
                let home_for_delete = home.clone();
                // And Delete, which asks before it does anything: see
                // `confirm_delete`.
                window.on_node_delete(move |name| {
                    let absolute = Qualified::of(&name, &home_for_delete).absolute();
                    confirm_delete(&editor_for_delete, &views_for_delete, &absolute);
                });
            }
            {
                // Choosing a node: the outline follows, and nothing is
                // recorded. What the node says about itself is a window of
                // its own, reached through the node's menu, so a press has
                // only the outline to change.
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
    lists: (i32, i32),
    held: Option<&Link>,
) {
    ui.set_payload_divider(dividers.payloads as f32);
    ui.set_vehicle_divider(dividers.vehicle as f32);
    ui.set_moc_list_percent(lists.0);
    ui.set_vehicle_list_percent(lists.1);
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
    // What might be wrong, counted on the two buttons: the list itself is a
    // window. See `Document::concerns`.
    let (raised, aside) = editor.document().concerns_apart();
    ui.set_concern_count(raised.len() as i32);
    ui.set_aside_count(aside.len() as i32);
}

/// The same, for one open subdiagram.
fn put_sub(
    window: &SubWindow,
    editor: &Editor,
    home: &str,
    chosen: Option<&str>,
    dividers: Dividers,
    lists: (i32, i32),
    held: Option<&Link>,
) {
    window.set_payload_divider(dividers.payloads as f32);
    window.set_vehicle_divider(dividers.vehicle as f32);
    window.set_moc_list_percent(lists.0);
    window.set_vehicle_list_percent(lists.1);
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
    if !refilled(&window.get_fields(), &fields) {
        window.set_fields(ModelRc::from(Rc::new(VecModel::from(fields))));
    }
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
            Ok(window) => {
                session!(&window);
                window
            },
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
/// `which` is the pane on the divider's left -- `Pane::Payloads` for the line
/// between the payload pane and the vehicle's, `Pane::Vehicle` for the one
/// between the vehicle's and the moc pane. Both gestures are the same work, and
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
        let from = if which == Pane::Payloads {
            was.payloads
        } else {
            was.vehicle
        };
        // On the grid, and inside what stops it: a node's edge is not always
        // a grid point, so the bounds are taken in to the nearest ones.
        let (low, high) = (pane::grid_ceil(low), pane::grid_floor(high));
        let now = snap((from + dx.round() as i32, 0)).0.clamp(low, high.max(low));
        if now == from {
            // It did not move, or it was already against what stops it.
            // Nothing to redraw and nothing to write: a settings file
            // rewritten on every press of a handle is a file written for
            // nothing.
            return;
        }
        dividers.set(if which == Pane::Payloads {
            Dividers {
                payloads: now,
                ..was
            }
        } else {
            Dividers { vehicle: now, ..was }
        });
    }

    let (moc_percent, vehicle_percent) = percents_for(dividers.get());
    let (list_moc_percent, list_vehicle_percent) = views.borrow().lists.get();
    settings::save(settings::Settings {
        moc_percent,
        vehicle_percent,
        list_moc_percent,
        list_vehicle_percent,
    });
    views.borrow().refresh(&editor.borrow());
}

/// One of the two lines between the selection lists, dragged.
///
/// `dy` is how far it moved and `tall` how tall the whole column is, both in
/// the window's own pixels: the share that changed is the one, over the other,
/// and the arithmetic is here rather than in the markup for the reason the
/// link routes are -- it can be read and tested here.
///
/// Each line moves the boundary between its own two neighbours and leaves the
/// third alone, which is what a person dragging it expects: the list above
/// grows by what the list below gives up. Nothing may be squeezed below
/// [`settings::LEAST`] per cent of the column, so a line dragged to the top or
/// the bottom stops a twentieth short of it -- a list with nothing showing is
/// a list whose entries cannot be reached, and a line dragged to the edge is
/// easy to do by accident and hard to understand afterwards.
///
/// The arithmetic is [`list_shares_after`], which is the part worth testing.
fn drag_list_divider(
    editor: &Rc<RefCell<Editor>>,
    views: &Rc<RefCell<Views>>,
    which: Pane,
    dy: f32,
    tall: f32,
) {
    let lists = views.borrow().lists.clone();
    let was = lists.get();
    let now = list_shares_after(was, which, dy, tall);
    if now == was {
        // It did not move, or it was already against what stops it: nothing
        // to redraw, and nothing to write.
        return;
    }
    lists.set(now);

    let (moc_percent, vehicle_percent) = percents_for(views.borrow().divider.get());
    settings::save(settings::Settings {
        moc_percent,
        vehicle_percent,
        list_moc_percent: now.0,
        list_vehicle_percent: now.1,
    });
    views.borrow().refresh(&editor.borrow());
}

/// What the two shares become when a line is dragged this far.
///
/// `was` is the moc list's share and the vehicle list's, the payload list
/// taking what is left; `which` is the list above the line that moved -- the
/// payload list for the upper line, the vehicle list for the lower one, the
/// lists running payloads, vehicle, moc down the column; `dy` is how far it
/// went and `tall` how tall the column is, both in pixels.
///
/// A share of the column, worked out here rather than in the markup: a
/// percentage is what somebody chose and what the settings file keeps, where a
/// pixel count is how this window happens to be sized today. And every share
/// comes back inside [`settings::LEAST`] however far the line was dragged,
/// which is the rule a test can hold this to and a markup cannot.
fn list_shares_after(
    (moc, vehicle): (i32, i32),
    which: Pane,
    dy: f32,
    tall: f32,
) -> (i32, i32) {
    if tall < 1.0 {
        return (moc, vehicle);
    }
    let step = (dy / tall * 100.0).round() as i32;
    if step == 0 {
        return (moc, vehicle);
    }
    // Each line moves only the boundary between its own two neighbours, so
    // the step is held to what both of them can give: the third list is not
    // touched however far the line goes.
    let payload = 100 - moc - vehicle;
    let least = settings::LEAST;
    let asked = match which {
        // The upper line takes from the vehicle list and gives to the payload
        // one, which is whatever is left.
        Pane::Payloads => {
            let step = step.clamp(least - payload, vehicle - least);
            (moc, vehicle - step)
        }
        // The lower line takes from the moc list and gives to the vehicle's.
        _ => {
            let step = step.clamp(least - vehicle, moc - least);
            (moc - step, vehicle + step)
        }
    };
    settings::shares(asked.0, asked.1)
}

/// Show what might be wrong, with somewhere to go and something to decide.
///
/// `aside` says which of the two lists: what has not been looked at, or what
/// somebody looked at and left. One window, pointed at either -- two windows
/// saying almost the same thing would be two windows to keep in step -- so
/// pressing the other button points this one at the other list.
///
/// Rows are held by their place in the list, which is why the window is
/// refreshed after every decision: a row acted on changes the list under it,
/// and an index into a list that has moved is a decision about the wrong node.
fn show_concerns(editor: &Rc<RefCell<Editor>>, views: &Rc<RefCell<Views>>, aside: bool) {
    if views.borrow().concerns.is_none() {
        let window = match ConcernsWindow::new() {
            Ok(window) => {
                session!(&window);
                window
            },
            Err(e) => {
                eprintln!("tcsgui: the list cannot be shown: {e}");
                return;
            }
        };
        {
            let editor = editor.clone();
            let views = views.clone();
            window.on_go_to(move |which| {
                let node = {
                    let shown = views.borrow();
                    let Some(window) = shown.concerns.as_ref() else {
                        return;
                    };
                    let (raised, set_aside) = editor.borrow().document().concerns_apart();
                    let list = if window.get_aside() { set_aside } else { raised };
                    match list.get(which as usize) {
                        Some(concern) => concern.node().to_string(),
                        None => return,
                    }
                };
                go_to(editor.clone(), views.clone(), &node);
            });
        }
        {
            let editor = editor.clone();
            let views = views.clone();
            window.on_decided(move |which| {
                let decision = {
                    let shown = views.borrow();
                    let Some(window) = shown.concerns.as_ref() else {
                        return;
                    };
                    let was_aside = window.get_aside();
                    let (raised, set_aside) = editor.borrow().document().concerns_apart();
                    let list = if was_aside { set_aside } else { raised };
                    list.get(which as usize).map(|concern| Edit::SetAside {
                        node: concern.node().to_string(),
                        kind: concern.kind(),
                        aside: !was_aside,
                    })
                };
                if let Some(edit) = decision {
                    editor.borrow_mut().edit(edit);
                    // The list has changed under the row that was pressed, so
                    // it is written again before anybody presses another.
                    let aside = views
                        .borrow()
                        .concerns
                        .as_ref()
                        .is_some_and(|window| window.get_aside());
                    list_concerns(&editor.borrow(), &views, aside);
                    views.borrow().refresh(&editor.borrow());
                }
            });
        }
        {
            let views = views.clone();
            window.on_closed(move || {
                let window = views.borrow_mut().concerns.take();
                if let Some(window) = window {
                    let _ = window.hide();
                }
            });
        }
        views.borrow_mut().concerns = Some(window);
    }

    list_concerns(&editor.borrow(), views, aside);
    let window = views.borrow().concerns.as_ref().map(|window| window.as_weak());
    if let Some(window) = window.and_then(|window| window.upgrade()) {
        if let Err(e) = window.show() {
            eprintln!("tcsgui: the list cannot be shown: {e}");
        }
    }
}

/// Write one of the two lists into the window.
fn list_concerns(editor: &Editor, views: &Rc<RefCell<Views>>, aside: bool) {
    let (raised, set_aside) = editor.document().concerns_apart();
    let shown = if aside { set_aside } else { raised };
    let rows: Vec<ConcernRow> = shown
        .iter()
        .map(|concern| ConcernRow {
            said: concern.describe().into(),
            node: concern.node().into(),
        })
        .collect();

    // What the Copy button copies: every row, one to a line.
    let everything: Vec<String> = rows.iter().map(|row| row.said.to_string()).collect();

    if let Some(window) = views.borrow().concerns.as_ref() {
        window.set_aside(aside);
        window.set_everything(everything.join("\n").into());
        window.set_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
    }
}

/// Show a node: chosen, in view, and with its window open.
///
/// What "go to it" means. The outline says which node the list was talking
/// about, the scroll puts it where it can be seen -- a node named in a list
/// and drawn eight hundred pixels below the fold is a list that points at
/// nothing -- and the window is what somebody went there to look at.
fn go_to(editor: Rc<RefCell<Editor>>, views: Rc<RefCell<Views>>, node: &str) {
    let at = editor
        .borrow()
        .document()
        .node(node)
        .map(|node| node.at)
        .unwrap_or((0, 0));
    let home = Qualified::parse(node).in_;

    views
        .borrow()
        .chosen
        .borrow_mut()
        .insert(home.clone().unwrap_or_default(), node.to_string());
    if home.is_none() {
        if let Some(top) = views.borrow().top.upgrade() {
            top.set_canvas_offset(-(at.1 as f32));
        }
    }
    views.borrow().refresh(&editor.borrow());
    show_node(&editor, &views, node);
}

/// Ask whether a node should go, and take it away if the answer is yes.
///
/// A node's Delete asks first. It is next to Edit in a menu a click opens, so
/// a press meant for Edit is a press away from it -- and a node is not one
/// word: it is its attributes and every line that touches it, which go with
/// it. Undo would put all of that back, a removal being one edit, but an undo
/// somebody has to discover is no answer to a node that has already gone.
///
/// One window at a time, re-pointed at whichever node was asked about: it
/// names the node, and two of them would leave somebody answering whichever
/// was on top. Deleting is what the window's Delete does and nothing else
/// here does it, so a window that cannot be made is a node that cannot be
/// deleted -- said on the error output, rather than deleting it unasked.
fn confirm_delete(editor: &Rc<RefCell<Editor>>, views: &Rc<RefCell<Views>>, node: &str) {
    // Nothing to ask about: a node somebody deleted while the menu was open.
    if editor.borrow().document().node(node).is_none() {
        return;
    }

    if views.borrow().deleting.is_none() {
        let window = match DeleteWindow::new() {
            Ok(window) => {
                session!(&window);
                window
            },
            Err(e) => {
                eprintln!("tcsgui: {node} cannot be asked about: {e}");
                return;
            }
        };
        {
            let editor = editor.clone();
            let views = views.clone();
            window.on_confirmed(move || {
                let asked = views
                    .borrow()
                    .deleting
                    .as_ref()
                    .map(|window| window.get_node().to_string())
                    .unwrap_or_default();
                let removal = editor.borrow().document().removal_of(&asked);
                if let Some(removal) = removal {
                    editor.borrow_mut().edit(removal);
                }
                hide_deleting(&views);
                views.borrow().refresh(&editor.borrow());
            });
        }
        {
            let views = views.clone();
            window.on_cancelled(move || hide_deleting(&views));
        }
        views.borrow_mut().deleting = Some(window);
    }

    let lines = editor.borrow().document().lines_touching(node);
    let carries = match lines {
        0 => String::new(),
        1 => "One line is attached to it and goes with it.".to_string(),
        many => format!("{many} lines are attached to it and go with it."),
    };

    let window = views.borrow().deleting.as_ref().map(|window| window.as_weak());
    if let Some(window) = window.and_then(|window| window.upgrade()) {
        window.set_node(node.into());
        window.set_carries(carries.into());
        if let Err(e) = window.show() {
            eprintln!("tcsgui: {node} cannot be asked about: {e}");
        }
    }
}

/// What a row of the startup window offers to do, besides nothing.
///
/// One of these per problem, chosen by what kind of problem it is: there is no
/// use offering to delete a group for a node whose address is malformed, and
/// no use offering to open a node for a line that goes nowhere.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Fix {
    /// A name mends it: two nodes sharing one, or a node without one. The row
    /// carries a field and a suggestion, this being the one kind of problem
    /// the editor can mend by itself.
    Name,
    /// The node has to be looked at or taken out, which is somebody's
    /// decision: an address that is not an address, a group that is not
    /// declared, a payload with no trigger, a type this build has not got.
    Node,
    /// A group nothing is in. Deleting it is the whole of the fix, so the row
    /// offers that -- the alternative is putting a node in it, which is
    /// drawing and not repair.
    Group(String),
    /// A line: one that goes nowhere, one that goes back where it came from,
    /// one of two into a single input. Deleting it is the fix.
    Line(Link),
    /// The `tcspecial` section, which is edited in its own window.
    Parameters,
    /// Nothing the editor can offer, beyond leaving it: a problem `tcsverify`
    /// finds in the files that is about no node on the canvas -- a group's
    /// interval, the simulator file -- and has no window of its own here.
    Nothing,
}

/// One problem a configuration opened with, and what can be done about it.
///
/// Built from every fault the document reports -- see `Document::faults` --
/// so that nothing arrives in the alert with no offer beside it. What differs
/// between them is the offer: see [`Fix`].
#[derive(Debug, Clone, PartialEq, Eq)]
struct Trouble {
    /// Where the node is in the document, which is how a rename reaches the
    /// right one of two nodes that share a name: see
    /// `Document::rename_at`. Nothing for a problem that is not about a node.
    at: Option<usize>,
    /// The node it is about, or nothing where there is none.
    node: String,
    /// What is wrong, in the words the fault itself uses.
    said: String,
    /// And what is offered for it.
    fix: Fix,
}

impl Trouble {
    /// Whether a name is the fix.
    fn nameable(&self) -> bool {
        self.fix == Fix::Name && self.at.is_some()
    }

    /// The label on the one button this kind of problem offers, over and above
    /// opening or deleting the node. Empty where it offers none.
    fn offer(&self) -> String {
        match &self.fix {
            Fix::Group(name) => format!("Delete the group \"{name}\""),
            Fix::Line(_) => "Delete the line".to_string(),
            Fix::Parameters => "Edit the parameters".to_string(),
            Fix::Name | Fix::Node | Fix::Nothing => String::new(),
        }
    }
}

/// Every problem a configuration opened with, and what is offered for each.
///
/// In the order the document holds the nodes, so the list a person reads is
/// the order the file is in rather than the order the checks happen to run.
///
/// Every fault gets a row. That is the change from the first version of this:
/// a problem with no row went to the alert on its own, which says what is
/// wrong and offers nothing -- and an alert with nothing to press is a
/// complaint rather than a question. What differs between them is the offer:
/// a name for the ones a name mends, the node for the ones somebody has to
/// look at, the group, the line, or the parameters window for the rest.
fn troubles_of(document: &Document) -> Vec<Trouble> {
    let sharing = document.sharing_a_name();
    let mut out: Vec<Trouble> = Vec::new();

    // The duplicates first, by position, because a name two nodes share
    // cannot say which of them a row is about.
    for at in sharing {
        let Some(node) = document.nodes.get(at) else {
            continue;
        };
        out.push(Trouble {
            at: Some(at),
            node: node.name.clone(),
            said: format!(
                "\"{}\" is the name of two nodes, so a link to it does not say \
                 which one it means",
                node.name
            ),
            fix: Fix::Name,
        });
    }

    let at_of = |name: &str| {
        document
            .nodes
            .iter()
            .position(|node| Qualified::of(&node.name, &node.home).absolute() == name)
    };

    for fault in document.faults() {
        // The duplicates are above, one row per node rather than one per
        // name.
        if matches!(fault, Fault::DuplicateName { .. }) {
            continue;
        }
        let said = fault.describe();
        let (at, node, fix) = match &fault {
            Fault::DuplicateName { .. } => continue,
            // A node with no name at all: found by where it sits, because
            // there is no name to find it by.
            Fault::NamelessNode { at } => (
                document.nodes.iter().position(|node| node.at == *at),
                String::new(),
                Fix::Name,
            ),
            // The group ones. A node in a group nobody declared is the node's
            // to answer -- the group may be a misspelling -- and a group with
            // nothing in it is the group's.
            Fault::NoSuchGroup { node, .. } => (at_of(node), node.clone(), Fix::Node),
            Fault::EmptyGroup { group } => (None, String::new(), Fix::Group(group.clone())),
            // The lines.
            // The line the document actually holds, not one built from the
            // two names: a removal is matched by what a link joins, origin
            // and input included -- see `Link::same_connection`.
            Fault::DanglingLink { from, to } => {
                let line = document
                    .links
                    .iter()
                    .find(|link| link.from == *from && link.to == *to)
                    .cloned();
                match line {
                    Some(line) => (None, String::new(), Fix::Line(line)),
                    None => continue,
                }
            }
            Fault::SelfLink { node } => {
                let line = document
                    .links
                    .iter()
                    .find(|link| link.from == *node && link.to == *node)
                    .cloned();
                match line {
                    Some(line) => (None, String::new(), Fix::Line(line)),
                    None => continue,
                }
            }
            Fault::InputBoundTwice { node, input } => {
                // Either of the two lines into that input would do; the one
                // the document holds second is the one that made it a fault,
                // so that is the one offered.
                let line = document
                    .links
                    .iter()
                    .rfind(|link| {
                        link.to == *node && link.input.as_deref() == Some(input.as_str())
                    })
                    .cloned();
                match line {
                    Some(line) => (None, String::new(), Fix::Line(line)),
                    None => (at_of(node), node.clone(), Fix::Node),
                }
            }
            // The configuration's own parameters.
            Fault::BadTcspecial { .. } => (None, String::new(), Fix::Parameters),
            // And everything else is a node somebody has to look at.
            Fault::UnknownType { node, .. }
            | Fault::BadNode { node, .. }
            | Fault::UntriggeredInput { node, .. } => (at_of(node), node.clone(), Fix::Node),
        };
        // A node fault naming a node the document does not hold has nothing
        // to offer and nothing to point at: it cannot happen from a file the
        // editor read, and if it did, the alert still says it.
        if matches!(fix, Fix::Node) && at.is_none() {
            continue;
        }
        out.push(Trouble {
            at,
            node,
            said,
            fix,
        });
    }
    out
}

/// Every problem the configuration opened with: the editor's own, with what
/// it can offer for each, and then whatever else `tcsverify` finds in the
/// files.
///
/// The second half is what makes the editor say what every other program
/// says. The editor's faults are its own checks of the drawing, made as it
/// changes, and deliberately lazy about what has not been typed yet; tcsverify
/// is the rules tcspecial, tcsmoc and tcssim refuse a file by, asked of the
/// files as they are on the disk. A file the editor opened quietly and the
/// vehicle would refuse is the disagreement this is here to end, so what the
/// rules find is listed too -- warnings included, the unfinished being worth
/// knowing about as a session starts -- in tcsverify's own words.
///
/// Of the files on the disk rather than the document, because the files are
/// what those programs read. They are the same thing here: every edit is
/// written as it is made.
fn startup_troubles(editor: &Editor) -> Vec<Trouble> {
    let mut troubles = troubles_of(editor.document());
    troubles.extend(verified(editor));
    troubles
}

/// What `tcsverify` finds in the files that the editor's own faults do not
/// already say.
///
/// Nothing for a file that is not there yet, which is how a new configuration
/// is begun: an empty session is not one with a problem.
fn verified(editor: &Editor) -> Vec<Trouble> {
    let (config, simulator) = editor.files();
    if !config.exists() {
        return Vec::new();
    }
    let simulator = simulator.filter(|path| path.exists());
    let report = tcsverify::verify(config, simulator, tcsverify::Limits::EVERYTHING);

    let document = editor.document();
    let faults = document.faults();
    let at_of = |name: &str| {
        document
            .nodes
            .iter()
            .position(|node| Qualified::of(&node.name, &node.home).absolute() == name)
    };

    // In the order found, as tcsverify prints them.
    report
        .findings
        .iter()
        .filter(|finding| {
            finding
                .problem
                .as_ref()
                .is_none_or(|problem| !said_already(problem, &faults))
        })
        .map(|finding| {
            let node = match finding.problem.as_ref().map(|problem| problem.about()) {
                Some(tcslibgs::About::Node(name)) => name,
                _ => String::new(),
            };
            let at = (!node.is_empty()).then(|| at_of(&node)).flatten();
            let fix = match &finding.problem {
                _ if at.is_some() => Fix::Node,
                Some(Problem::NoTcspecial) | Some(Problem::BadTcspecial { .. }) => {
                    Fix::Parameters
                }
                Some(Problem::EmptyGroup { group }) => Fix::Group(group.clone()),
                _ => Fix::Nothing,
            };
            Trouble {
                at,
                node: if at.is_some() { node } else { String::new() },
                said: finding.to_string(),
                fix,
            }
        })
        .collect()
}

/// Whether one of the editor's own faults already says what this problem
/// says.
///
/// By what each is about rather than by its words: the editor's sentences
/// were written for a node's own window and the rules' for a file and a line,
/// and the same mistake is said both ways. Saying it twice in one list would
/// read as two mistakes.
fn said_already(problem: &Problem, faults: &[Fault]) -> bool {
    faults.iter().any(|fault| match (problem, fault) {
        (Problem::DuplicateNode { name }, Fault::DuplicateName { name: said }) => name == said,
        (Problem::NamelessNode, Fault::NamelessNode { .. }) => true,
        (Problem::NoSuchNode { from, missing, .. }, Fault::DanglingLink { from: f, to }) => {
            (from == f || missing == f) && (missing == to || from == to)
        }
        (Problem::SelfLink { node }, Fault::SelfLink { node: said }) => node == said,
        (Problem::NoSuchGroup { node, .. }, Fault::NoSuchGroup { node: said, .. }) => {
            node == said
        }
        (Problem::EmptyGroup { group }, Fault::EmptyGroup { group: said }) => group == said,
        (
            Problem::InputBoundTwice { node, input },
            Fault::InputBoundTwice {
                node: said,
                input: into,
            },
        ) => node == said && input == into,
        (Problem::BadNode { node, .. }, Fault::BadNode { node: said, .. })
        | (Problem::BadNode { node, .. }, Fault::UnknownType { node: said, .. }) => node == said,
        (Problem::BadTcspecial { .. }, Fault::BadTcspecial { .. }) => true,
        (
            Problem::UntriggeredInput { node, from },
            Fault::UntriggeredInput {
                node: said,
                from: by,
            },
        ) => node == said && from == by,
        _ => false,
    })
}

/// Whether a set of proposed names settles the duplicate, and what is wrong
/// with it if not.
///
/// `proposed` is what the fields hold, by the position of the node each is
/// for, and `document` is what else is in the configuration. Three things are
/// asked: that no name is blank, that no two of the proposed names are the
/// same, and that none of them is a name some *other* node already has --
/// renaming one duplicate into a third node's name is a duplicate again, and
/// pressing a button that made the same question come back would be no answer
/// at all.
///
/// Here rather than in the markup for the usual reason: the markup can compare
/// two strings, this is a rule about a document, and a rule is worth a test.
fn names_settle(
    document: &Document,
    proposed: &BTreeMap<usize, String>,
) -> Result<(), String> {
    let whole = |at: usize, name: &str| {
        document
            .nodes
            .get(at)
            .map(|node| Qualified::of(name, &node.home).absolute())
            .unwrap_or_else(|| name.to_string())
    };

    let mut taken: BTreeMap<String, usize> = BTreeMap::new();
    for (at, name) in proposed {
        let said = name.trim();
        if said.is_empty() {
            return Err("A node has to have a name.".to_string());
        }
        if let Some(before) = taken.insert(whole(*at, said), *at) {
            let _ = before;
            return Err(format!("Two of these are called \"{said}\"."));
        }
    }

    // And against the nodes that are not being renamed, which is every node
    // whose position is not in the list.
    for (at, node) in document.nodes.iter().enumerate() {
        if proposed.contains_key(&at) {
            continue;
        }
        let held = Qualified::of(&node.name, &node.home).absolute();
        if let Some(asked) = taken.get(&held) {
            let _ = asked;
            return Err(format!(
                "Another node is already called \"{}\".",
                node.name
            ));
        }
    }
    Ok(())
}

/// Ask about the problems a configuration opened with, and offer what can be
/// done about each.
///
/// Shown once, at startup, when the editor has read a file the vehicle could
/// not fly: a node named twice, a node with no name, a type this build does
/// not know, attributes that do not hold together. See [`troubles_of`] for
/// which faults reach this window and which are left to the alert.
///
/// Doing nothing is one of the offers, and the window closes with it. A
/// problem left alone stays a problem -- red on its own node, counted in the
/// alert, refused by `tcsverify` -- but the node is on the canvas in its own
/// working pane whatever is decided here, which is what lets somebody look at
/// the drawing before choosing. An editor that would not show a configuration
/// until it was correct could not be used to correct one.
///
/// Where a name is the fix, the row carries a field and a suggestion: that is
/// the one kind of problem the editor can mend by itself, and the renaming it
/// does is not an edit -- see `Editor::settle_name`, which says why a repair
/// is written at once and kept out of the history. Everything else is
/// somebody's decision, so the row offers to open the node or to take it out.
fn settle_problems(editor: &Rc<RefCell<Editor>>, views: &Rc<RefCell<Views>>) {
    let troubles = startup_troubles(&editor.borrow());
    if troubles.is_empty() {
        return;
    }

    let window = match ProblemsWindow::new() {
        Ok(window) => {
            session!(&window);
            window
        },
        Err(e) => {
            // Every one of these is still reported as a fault, so this
            // failing is a quieter editor rather than a broken one.
            eprintln!("tcsgui: the problems cannot be asked about: {e}");
            return;
        }
    };

    // What the fields hold, by the position of the node each is for: its own
    // name to begin with, so a window opened and closed again changes
    // nothing.
    let proposed: Rc<RefCell<BTreeMap<usize, String>>> = Rc::new(RefCell::new(
        troubles
            .iter()
            .filter(|trouble| trouble.nameable())
            .filter_map(|trouble| trouble.at.map(|at| (at, trouble.node.clone())))
            .collect(),
    ));
    // And the rows the window is showing, so a press can say which one it was
    // about after the document has changed under it.
    let showing: Rc<RefCell<Vec<Trouble>>> = Rc::new(RefCell::new(troubles));

    {
        let editor = editor.clone();
        let proposed = proposed.clone();
        let showing = showing.clone();
        let window = window.as_weak();
        window
            .upgrade()
            .expect("the window exists")
            .on_typed(move |row, said| {
                let Some(at) = showing.borrow().get(row as usize).and_then(|t| t.at) else {
                    return;
                };
                proposed.borrow_mut().insert(at, said.to_string());
                if let Some(window) = window.upgrade() {
                    // The rows are not rebuilt: a model replaced under a
                    // LineEdit takes the focus with it, and somebody typing a
                    // name would have to press the field again for every
                    // letter. Only the answer changes.
                    say_problems(
                        &window,
                        editor.borrow().document(),
                        &showing.borrow(),
                        &proposed.borrow(),
                        false,
                    );
                }
            });
    }
    {
        let editor = editor.clone();
        let proposed = proposed.clone();
        let showing = showing.clone();
        let window = window.as_weak();
        window
            .upgrade()
            .expect("the window exists")
            .on_derive(move |row| {
                let Some(at) = showing.borrow().get(row as usize).and_then(|t| t.at) else {
                    return;
                };
                let spare = editor.borrow().document().spare_name(at);
                proposed.borrow_mut().insert(at, spare);
                if let Some(window) = window.upgrade() {
                    // Rebuilt here, because the field's own text is what
                    // changed: a suggestion nobody can see is no suggestion.
                    say_problems(
                        &window,
                        editor.borrow().document(),
                        &showing.borrow(),
                        &proposed.borrow(),
                        true,
                    );
                }
            });
    }
    {
        let editor = editor.clone();
        let views = views.clone();
        let proposed = proposed.clone();
        let showing = showing.clone();
        window.on_confirmed(move || {
            let asked = proposed.borrow().clone();
            if names_settle(editor.borrow().document(), &asked).is_err() {
                return;
            }
            {
                let mut editor = editor.borrow_mut();
                for (at, name) in &asked {
                    editor.settle_name(*at, name.trim());
                }
            }
            // What is left after that, which is the rest of the problems: the
            // window stays on them rather than closing and leaving somebody
            // to find them in the alert.
            again(&editor, &views, &showing, &proposed);
        });
    }
    {
        let editor = editor.clone();
        let views = views.clone();
        let showing = showing.clone();
        window.on_edit(move |row| {
            let node = showing
                .borrow()
                .get(row as usize)
                .map(|trouble| trouble.node.clone());
            if let Some(node) = node.filter(|node| !node.is_empty()) {
                show_node(&editor, &views, &node);
            }
        });
    }
    {
        let editor = editor.clone();
        let views = views.clone();
        let showing = showing.clone();
        let proposed = proposed.clone();
        window.on_remove(move |row| {
            let node = showing
                .borrow()
                .get(row as usize)
                .map(|trouble| trouble.node.clone());
            let Some(node) = node.filter(|node| !node.is_empty()) else {
                return;
            };
            // Through the same question any other deletion goes through: a
            // node and its lines are not taken out on one press, and the
            // wording that says what goes with it is already written.
            confirm_delete(&editor, &views, &node);
            again(&editor, &views, &showing, &proposed);
        });
    }
    {
        let editor = editor.clone();
        let views = views.clone();
        let showing = showing.clone();
        let proposed = proposed.clone();
        window.on_acted(move |row| {
            let fix = showing.borrow().get(row as usize).map(|t| t.fix.clone());
            match fix {
                // A group nothing is in: taking it out is the whole of the
                // fix, and it is an edit like any other -- somebody who meant
                // to put a node in it can undo this.
                Some(Fix::Group(name)) => {
                    let interval = editor
                        .borrow()
                        .document()
                        .groups
                        .get(&name)
                        .cloned()
                        .unwrap_or_default();
                    editor
                        .borrow_mut()
                        .edit(Edit::RemoveGroup { name, interval });
                }
                // A line that goes nowhere, goes back where it came from, or
                // is the second into one input.
                Some(Fix::Line(link)) => {
                    editor.borrow_mut().edit(Edit::RemoveLink { link });
                }
                // And the configuration's own parameters, in their own
                // window: what is wrong there is a value somebody has to
                // choose, not something to delete.
                Some(Fix::Parameters) => {
                    show_parameters(&editor, &views);
                }
                Some(Fix::Name) | Some(Fix::Node) | Some(Fix::Nothing) | None => return,
            }
            again(&editor, &views, &showing, &proposed);
        });
    }
    {
        let views = views.clone();
        // Doing nothing about this one. The problem stays, drawn in red on
        // its own node and counted in the alert, and every node is on the
        // canvas.
        window.on_left(move || hide_problems(&views));
    }
    {
        let editor = editor.clone();
        let views = views.clone();
        // And doing nothing about all of them, which is what somebody opening
        // a file to look at it wants: every node is added as it stands, and
        // nothing says any of this again for the rest of the session. The
        // faults are still faults -- red on their nodes, refused by
        // tcsverify -- but the editor stops asking.
        window.on_ignored(move || {
            ignore_faults(&editor, &views);
            hide_problems(&views);
        });
    }
    {
        let editor = editor.clone();
        let views = views.clone();
        // A way out of the editor, which is the one answer here that is not
        // about the configuration. Through the same path the menu's Exit
        // takes: nothing has been edited yet, so it ends there and then, and
        // a session that somehow has something unwritten is offered the same
        // chance to write it.
        window.on_quit(move || leave(&editor, &views));
    }
    {
        let editor = editor.clone();
        let views = views.clone();
        // Closed by the window manager, which is ignoring the rest of them:
        // the same as the button that says so, the faults included. Taking
        // the window away without that would be a close box that asks the
        // question again at the next drawing -- so somebody who answers a
        // window by closing it would be answering it over and over, which is
        // not what closing a window means anywhere else.
        window.window().on_close_requested(move || {
            ignore_faults(&editor, &views);
            hide_problems(&views);
            slint::CloseRequestResponse::HideWindow
        });
    }

    say_problems(
        &window,
        editor.borrow().document(),
        &showing.borrow(),
        &proposed.borrow(),
        true,
    );
    if let Err(e) = window.show() {
        eprintln!("tcsgui: the problems cannot be asked about: {e}");
        return;
    }
    views.borrow_mut().names = Some(window);
}

/// What is left to ask about after something was done about one of them.
///
/// The window stays open on the rest rather than closing: a file with four
/// problems answered one at a time should not need opening four times, and a
/// problem that has gone should leave the list rather than sit there answered.
fn again(
    editor: &Rc<RefCell<Editor>>,
    views: &Rc<RefCell<Views>>,
    showing: &Rc<RefCell<Vec<Trouble>>>,
    proposed: &Rc<RefCell<BTreeMap<usize, String>>>,
) {
    let left = startup_troubles(&editor.borrow());
    *showing.borrow_mut() = left.clone();
    *proposed.borrow_mut() = left
        .iter()
        .filter(|trouble| trouble.nameable())
        .filter_map(|trouble| trouble.at.map(|at| (at, trouble.node.clone())))
        .collect();

    let window = views.borrow().names.as_ref().map(|window| window.as_weak());
    if left.is_empty() {
        hide_problems(views);
    } else if let Some(window) = window.and_then(|window| window.upgrade()) {
        say_problems(
            &window,
            editor.borrow().document(),
            &showing.borrow(),
            &proposed.borrow(),
            true,
        );
    }
    views.borrow().refresh(&editor.borrow());
}

/// Put the problems into the window, and say whether the names settle what
/// they are offered for.
///
/// `rows` says whether the list itself is rebuilt. It is not, for a letter
/// somebody typed: replacing the model under a LineEdit takes the focus with
/// it, and a field that had to be pressed again for every letter is the bug
/// this editor has already had once. See `refilled`.
fn say_problems(
    window: &ProblemsWindow,
    document: &Document,
    showing: &[Trouble],
    proposed: &BTreeMap<usize, String>,
    rows: bool,
) {
    if rows {
        let rows: Vec<ProblemRow> = showing
            .iter()
            .map(|trouble| ProblemRow {
                said: trouble.said.clone().into(),
                node: trouble.node.clone().into(),
                now: trouble
                    .at
                    .and_then(|at| proposed.get(&at))
                    .cloned()
                    .unwrap_or_else(|| trouble.node.clone())
                    .into(),
                nameable: trouble.nameable(),
                there: trouble.at.is_some() && !trouble.node.is_empty(),
                action: trouble.offer().into(),
            })
            .collect();
        window.set_any_nameable(rows.iter().any(|row| row.nameable));
        // What the Copy button copies: the window's heading, then every
        // problem, one to a line.
        let mut everything =
            vec!["This configuration has problems the vehicle would not fly with".to_string()];
        everything.extend(rows.iter().map(|row| row.said.to_string()));
        window.set_everything(everything.join("\n").into());
        window.set_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
    }

    match names_settle(document, proposed) {
        Ok(()) => {
            window.set_ready(true);
            window.set_trouble(Default::default());
        }
        Err(why) => {
            window.set_ready(false);
            window.set_trouble(why.into());
        }
    }
}

/// Count what the file arrived with as already said, so the alert leaves it
/// to the window that asks about it.
///
/// The same thing [`ignore_faults`] does, done before the first window is
/// shown rather than on a press: two windows about one set of problems, one
/// of them with nothing to press, is what this is for.
fn settled(editor: &Rc<RefCell<Editor>>, views: &Rc<RefCell<Views>>) {
    ignore_faults(editor, views);
}

/// Say nothing further about what is wrong with the configuration as it
/// stands.
///
/// Every fault the document has now is counted as said, so the alert does not
/// raise it again -- and so the window that asked about them does not come
/// back. The configuration is not changed: the nodes stay as they are, on the
/// canvas in their own panes, drawn in red, and `tcsverify` still refuses the
/// file. What stops is the asking.
///
/// It is for the session rather than for the file, which is the distinction
/// worth stating. A decision about a *concern* is written into the file, that
/// being a judgement about the drawing somebody else should see -- see
/// `Document::aside`. A fault is an error; nobody should inherit somebody
/// else's decision to ignore one, so this is forgotten when the editor
/// closes.
///
/// A fault that goes away and comes back is said again: the set is kept by
/// what each one says, and `Views::alert_faults` drops what is no longer
/// there.
fn ignore_faults(editor: &Rc<RefCell<Editor>>, views: &Rc<RefCell<Views>>) {
    let said: Vec<String> = editor
        .borrow()
        .faults()
        .iter()
        .map(|fault| fault.describe())
        .collect();
    let views = views.borrow();
    let mut faulted = views.faulted.borrow_mut();
    for fault in said {
        faulted.insert(fault);
    }
}

/// Take the question away, however it was answered.
fn hide_problems(views: &Rc<RefCell<Views>>) {
    listen_when_read(views, Some(Startup::Problems));
    let window = views.borrow_mut().names.take();
    if let Some(window) = window {
        let _ = window.hide();
    }
}

/// What the main window's close box does.
///
/// Closed by the window manager, which is the end of the session: the windows
/// that say what is wrong with the drawing go with it. Through the close box
/// rather than through `leave`, which is the menu's Exit: there is nothing to
/// ask about, every edit having been written as it was made, and a window
/// that asked would be a window somebody closed and then had to answer.
///
/// Its own function so that the tests close a window the way the window
/// manager closes it, rather than calling what the handler would have called.
fn closing(ui: &MainWindow, views: &Rc<RefCell<Views>>) {
    let views = views.clone();
    ui.window().on_close_requested(move || {
        hide_what_is_wrong(&views);
        slint::CloseRequestResponse::HideWindow
    });
}

/// Take down the windows that say what is wrong, whichever are up.
///
/// What closing the main window comes to: the question about the state the
/// file arrived in, the alert about something that failed, and the list of
/// concerns are all *about* the drawing, so none of them has anything to say
/// once the drawing has gone. A window left behind by a closed main window is
/// worse than untidy -- the toolkit stops when its last window closes, so one
/// still up is an editor still running with nothing to edit in, and the only
/// way out of it is to close that window as well.
///
/// Taken out of `Views` as each window's own answer does, so nothing holds a
/// window that is not there.
fn hide_what_is_wrong(views: &Rc<RefCell<Views>>) {
    let (names, alert, concerns) = {
        let mut views = views.borrow_mut();
        (
            views.names.take(),
            views.alert.take(),
            views.concerns.take(),
        )
    };
    // Each held only long enough to be hidden: the three are three types, so
    // they are hidden one at a time rather than walked over as one list.
    if let Some(window) = names {
        let _ = window.hide();
    }
    if let Some(window) = alert {
        let _ = window.hide();
    }
    if let Some(window) = concerns {
        let _ = window.hide();
    }
}

/// The windows that can come up as the editor starts, in front of the main
/// one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Startup {
    /// The question about the problems the file opened with.
    Problems,
    /// The alert, about a file that could not be read or a simulator file
    /// that could not be used.
    Alert,
}

/// Whether any window that came up as the editor started is still showing,
/// leaving out `going`, which is on its way.
fn startup_waiting(views: &Rc<RefCell<Views>>, going: Option<Startup>) -> bool {
    let views = views.borrow();
    let problems = going != Some(Startup::Problems)
        && views
            .names
            .as_ref()
            .is_some_and(|window| window.window().is_visible());
    let alert = going != Some(Startup::Alert)
        && views
            .alert
            .as_ref()
            .is_some_and(|window| window.window().is_visible());
    problems || alert
}

/// Give the main window its presses back, once what came up as the editor
/// started has been read and `going` was the last of it.
///
/// Called as each of those windows goes, and before it goes, `going` being
/// the one on its way: a window that is still up is still a question, and the
/// drawing behind it stays inert until the last of them has been answered.
///
/// The main window is up throughout -- it is shown before any of them, so
/// they are in front of it -- so this is about presses and not about showing.
/// After the start it does nothing: nothing sets `inert` again, and the
/// windows that come up while somebody works are their own business.
fn listen_when_read(views: &Rc<RefCell<Views>>, going: Option<Startup>) {
    if startup_waiting(views, going) {
        return;
    }
    // Through a weak handle, so nothing of `views` is borrowed while the
    // window is touched: a handler that borrowed it is what runs next.
    let top = views.borrow().top.clone();
    if let Some(top) = top.upgrade() {
        top.set_inert(false);
    }
    on_top(views, false);
}

/// Keep the windows that came up as the editor started in front of the main
/// window, or let them go.
///
/// Shown after the main window, which puts them on top by the usual rule --
/// but the rule is the window manager's, and one that gives the focus to the
/// larger window or the one that finished drawing last puts a question behind
/// the drawing that is waiting for it, where nobody can find it and nothing
/// in front of them takes a press. So for as long as the main window waits,
/// they are asked to stay on top; once it takes presses again they are
/// ordinary windows, the alert coming back later as one among the others.
fn on_top(views: &Rc<RefCell<Views>>, wanted: bool) {
    let views = views.borrow();
    if let Some(window) = views.names.as_ref() {
        window.set_on_top(wanted);
    }
    if let Some(window) = views.alert.as_ref() {
        window.set_on_top(wanted);
    }
}

/// Take the question away, whichever way it was answered.
fn hide_deleting(views: &Rc<RefCell<Views>>) {
    let window = views.borrow_mut().deleting.take();
    if let Some(window) = window {
        let _ = window.hide();
    }
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
        Ok(window) => {
            session!(&window);
            window
        },
        Err(e) => {
            eprintln!("tcsgui: errors cannot be shown in a window: {e}");
            return;
        }
    };
    {
        let views = views.clone();
        window.on_acknowledged(move || {
            listen_when_read(&views, Some(Startup::Alert));
            // Hidden and kept, not dropped: see above.
            let window = views.borrow().alert.as_ref().map(|window| window.as_weak());
            if let Some(window) = window.and_then(|window| window.upgrade()) {
                let _ = window.hide();
            }
        });
    }
    {
        // Closed by the window manager rather than by OK, which is the same
        // answer -- and as the editor starts, the main window behind it is
        // taking no presses until it is answered.
        let views = views.clone();
        window.window().on_close_requested(move || {
            listen_when_read(&views, Some(Startup::Alert));
            slint::CloseRequestResponse::HideWindow
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
            Ok(window) => {
                session!(&window);
                window
            },
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
    if !refilled(&window.get_fields(), &fields) {
        window.set_fields(ModelRc::from(Rc::new(VecModel::from(fields))));
    }
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
/// What tells one row of a window's fields from another.
///
/// The name and whether it is a heading, which is what says two lists are the
/// same list: a list with the same names in the same order is the one a
/// repeater has already built elements for, and those elements are what the
/// cursor lives in. See [`refilled`].
trait Named {
    fn named(&self) -> (String, bool);
}

impl Named for FieldView {
    fn named(&self) -> (String, bool) {
        (self.name.to_string(), self.is_heading)
    }
}

impl Named for GroupFieldView {
    fn named(&self) -> (String, bool) {
        (self.name.to_string(), false)
    }
}

/// Put these rows into a window's fields without replacing the model.
///
/// Replacing it destroys every element the repeater built from it -- which
/// includes the field somebody is typing in, and the cursor goes with it. That
/// was the bug: every character was an edit, every edit a refresh, and every
/// refresh a new text box, so a value had to be typed one character and one
/// click at a time.
///
/// So a model of the same shape is updated row by row, and only where a row
/// has actually changed: setting a row that already says what it says would
/// still move the cursor to the end of it. A list that is a different list --
/// a different length, or different names in a different order -- is replaced,
/// because then the elements are not the same elements anyway.
///
/// Answers whether it managed it; the caller replaces the model if not.
fn refilled<Row>(model: &ModelRc<Row>, rows: &[Row]) -> bool
where
    Row: Clone + PartialEq + Named + 'static,
{
    use slint::Model;
    let Some(held) = model.as_any().downcast_ref::<VecModel<Row>>() else {
        return false;
    };
    if held.row_count() != rows.len() {
        return false;
    }
    for (at, row) in rows.iter().enumerate() {
        let Some(had) = held.row_data(at) else {
            return false;
        };
        if had.named() != row.named() {
            return false;
        }
    }
    for (at, row) in rows.iter().enumerate() {
        if held.row_data(at).as_ref() != Some(row) {
            held.set_row_data(at, row.clone());
        }
    }
    true
}

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
                // The section's own parameters are not described in the tree,
                // which is about node types: each is a box to write in until
                // something says otherwise.
                kind: nodes::TEXT.into(),
            }
        })
        .collect();

    if !refilled(&window.get_fields(), &fields) {
        window.set_fields(ModelRc::from(Rc::new(VecModel::from(fields))));
    }
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
            // The simulator's settings are described by `tcslibgs::sim_config`
            // rather than by the tree, so they are boxes to write in until
            // that says what each takes.
            kind: nodes::TEXT.into(),
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
            kind: nodes::TEXT.into(),
        });
    }
    shown
}

/// Show the tcspecial parameters, making the window if there is not one.
fn show_parameters(editor: &Rc<RefCell<Editor>>, views: &Rc<RefCell<Views>>) {
    if views.borrow().parameters.is_none() {
        let window = match ParametersWindow::new() {
            Ok(window) => {
                session!(&window);
                window
            },
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
            Ok(window) => {
                session!(&window);
                window
            },
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
            Ok(window) => {
                session!(&window);
                window
            },
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
    // Each with what it takes, which decides the control it is offered with:
    // see `nodes::KINDS`. A type the tree does not describe falls back to its
    // own list, which says what is asked for and not what kind it is, so
    // those are text -- and every type has a description, which is what
    // `nodes::tests::every_description_has_its_rust` is for.
    let declared: Vec<(String, bool, String)> =
        match nodes::matching(descriptions(), &node.node_type, &node.attributes) {
            Some(described) => described.fields(),
            None => NodeType::ALL
                .iter()
                .find(|t| t.spelling() == node.node_type)
                .map(|t| {
                    t.attributes()
                        .iter()
                        .map(|(name, required)| {
                            ((*name).to_string(), *required, nodes::TEXT.to_string())
                        })
                        .collect()
                })
                .unwrap_or_default(),
        };

    let mut fields: Vec<FieldView> = declared
        .iter()
        .map(|(name, required, kind)| {
            let value = node.attributes.get(name).cloned().unwrap_or_default();
            FieldView {
                name: name.clone().into(),
                value: value.clone().into(),
                required: *required,
                is_heading: false,
                simulated: false,
                kind: kind.clone().into(),
            }
        })
        .collect();

    // Anything the node holds that its type does not declare. Shown rather
    // than dropped: a file from a later build is readable, and an attribute
    // nobody can see is one that gets written back out without anybody
    // knowing it is there.
    for (name, value) in &node.attributes {
        if declared.iter().any(|(declared, _, _)| declared == name) {
            continue;
        }
        fields.push(FieldView {
            name: name.clone().into(),
            value: value.clone().into(),
            required: false,
            is_heading: false,
            simulated: false,
            kind: nodes::TEXT.into(),
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
                // A rule across the panel rather than a field: it takes
                // nothing, so what it would be typed with is not a question.
                kind: nodes::TEXT.into(),
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
                summary.as_deref(),
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
    // Every route laid so far, in the order the file holds the links, so that
    // each line is laid clear of the ones already on the canvas: two lines
    // that run along one track are one line on the screen, and the one drawn
    // first is not on it at all. See `route::lay`.
    //
    // In the file's order rather than by any rule of its own, so that the same
    // configuration is drawn the same way every time it is opened -- a layout
    // that depended on which link happened to be routed first would change
    // under somebody who only reordered two lines in a text editor.
    let mut taken: Vec<Vec<(f32, f32)>> = Vec::new();
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
            // looking. A label reading "payload-in" beside every line would
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

            // The line itself, laid once and clear of the lines already laid.
            // Everything else about its drawing comes from these points, so
            // the path, the marker and the label cannot disagree about where
            // the line went.
            let points = route::lay(ends, &obstacles, &taken);
            let drawn = route::drawn_route(&points);
            let marker = route::marker_on(&points);
            // The label goes at the middle of the route's longest run, which
            // is on the line wherever the line had to go.
            let (label_x, label_y) = route::label_on(ends, &points);
            taken.push(points);

            LinkView {
                // The route and the arrow, worked out where they can be
                // tested: see route.rs.
                route: drawn.into(),
                head: route::arrow(ends).into(),
                marker: marker.into(),
                label_x,
                label_y,
                wide: named_input.is_some(),
                // Which stream it carries, as a colour: see `line_ink`.
                ink: colour(&line_ink(
                    document
                        .node(&link.from)
                        .and_then(|node| {
                            style::terminal_for(
                                &node.node_type,
                                link.origin.as_deref(),
                                false,
                            )
                        }),
                )),
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
/// vehicleward link leaves a node's left side and arrives at the next one's
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
    // one that pairs with the source it left: a link leaving `moc-out`
    // arrives at a `moc-in`, that being the same stream named at its other
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

    // The end being carried keeps its own terminal: a `payload-in` taken
    // off one node goes on another node's `payload-in` and nowhere else.
    let Some(over) = landed(document, home, (x, y), &pressed.terminal) else {
        // Over nothing at all. From the end a line *leaves* by, that is how a
        // line is deleted: pull the wire out of the thing that is sending and
        // drop it on the floor. From the end it arrives at, it is ignored --
        // a sink takes one source, so dragging its end away and letting go
        // over nothing is a slip far more often than it is a decision, and
        // the line is still written on the node it leaves whatever the
        // arriving end was dragged over.
        //
        // So nothing becomes undeletable: a line into a sink that takes
        // several -- the ground's -- is deleted from its source end, which has
        // the one link on it.
        if node_under(document, home, (x, y)).is_none() {
            return (!sink_pressed).then(|| Edit::RemoveLink { link: held.clone() });
        }
        // Over a node that cannot be this end, but could be the other one:
        // then the gesture was drawing a new line and not moving this one.
        //
        // Which is the only way to draw a second line out of a data source. A
        // source may feed several sinks; a terminal with one line on it hands
        // that line over, so pressing a source that already has one and
        // dropping it on a payload used to do nothing at all -- the payload
        // cannot be a source, so there was nowhere for the end to go. It is
        // what somebody tried first, and what they meant was obvious: this
        // node, fed from here.
        //
        // Tried second, so nothing that worked before changes. A node that
        // *can* take this end still takes it -- that is the re-routing
        // gesture, and it is the reading a person has when they drag an end
        // off one vehicle node onto another.
        return joined(document, home, pressed, (x, y)).map(|link| Edit::AddLink { link });
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
/// `payload-out` may only be linked to a `payload-in` and a `moc-out` only to
/// a `moc-in`: that is one stream, named at each of its ends, and anything
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
    // The far end of the line, which the stream names: see `landed`.
    let wanted = tcslibgs::graph::terminals::answers(&pressed.terminal)?;
    let dropped = landed(document, home, (x, y), wanted)?;
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

/// Where the line a gesture is drawing is anchored: the end that stays put.
///
/// The terminal the gesture started at, where a line is being drawn from
/// nothing. The *far* end of the line, where one is being carried by its end:
/// that end is in somebody's hand and the other one is what it still hangs
/// from, so a line anchored at the pressed terminal would be drawn from the
/// end that is moving.
///
/// `None` for a node or a terminal this page does not draw, which is what a
/// gesture on another page looks like from here: no line, rather than a line
/// from a guess.
fn anchored(document: &Document, home: &Home, from: &Drawing) -> Option<(f32, f32)> {
    if let Some(held) = &from.holding {
        let ends = ends_of(document, held, home);
        let (sink_pressed, _) = tcslibgs::graph::terminals::stream_of(&from.terminal)?;
        // The end that is not the one in hand.
        return Some(if sink_pressed { ends.from } else { ends.to });
    }

    let node = document.node(&from.node)?;
    let (at, _) = style::attachment(&node.node_type, node.at, &from.terminal)?;
    Some(at)
}

/// Where a line being dragged has landed: the node under the pointer, and the
/// terminal the line attaches to there.
///
/// Anywhere on the node will do -- its name, its middle, a corner box -- and
/// `wanted` is the terminal it attaches to, which the gesture already knows.
/// A stream has one name at each of its ends, so a line drawn from a `payload-out`
/// can only arrive at a `payload-in`, and an end carried off a
/// `payload-in` can only be put down on another `payload-in`: there is
/// nothing else the gesture could have meant. The terminals are still drawn,
/// because they are what says which streams a type carries and which way --
/// but hitting a box in a corner is work the editor can do instead of the
/// person.
///
/// `None` where the pointer is over no node, and where the node has no such
/// terminal: the ground receives readings and is sent nothing, so a line
/// carrying what the ground sends has nowhere to land on it. A gesture that
/// cannot make a link still makes nothing; what has changed is how hard it is
/// to aim, not what may be joined to what.
fn landed(
    document: &Document,
    home: &Home,
    (x, y): (f32, f32),
    wanted: &str,
) -> Option<Drawing> {
    let (sink, _) = tcslibgs::graph::terminals::stream_of(wanted)?;
    let node = node_under(document, home, (x, y))?;
    let found = document.node(&node)?;
    let terminals = NodeType::ALL
        .iter()
        .find(|t| t.spelling() == found.node_type)?
        .terminals();
    let theirs = if sink {
        terminals.sinks
    } else {
        terminals.sources
    };
    if !theirs.contains(&wanted) {
        return None;
    }
    Some(Drawing {
        node,
        terminal: wanted.to_string(),
        holding: None,
    })
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
/// that terminal: `payload-out: k_1` is where the link is written, so there is no
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
    //
    // One group with the ground link below, so that the new configuration is
    // one Undo deep rather than one per parameter: what was seeded was one
    // thing, and nobody asked for it a field at a time.
    let mut edits = Vec::new();
    for (attribute, _) in tcslibgs::graph::parameters::ATTRIBUTES {
        // Empty, with one exception the schema itself names: the beacon's
        // interval has an answer that is right until somebody has a reason,
        // where a socket has none. See `parameters::default_of`.
        let now = tcslibgs::graph::parameters::default_of(attribute)
            .unwrap_or_default()
            .to_string();
        edits.push(Edit::SetTcspecial {
            attribute: (*attribute).to_string(),
            was: None,
            now: Some(now),
        });
    }

    // Two edits, as placing one by hand is: a node exists, and then it is
    // somewhere. And named as one placed by hand is -- `moc-1`, from
    // `fresh_name` -- because a name the editor chose should look the same
    // whichever of its hands chose it. First in the group, so that the
    // group is described by it: see `Edit::describe`.
    let name = fresh_name(editor.document(), "moc", &None);
    let placed = [
        Edit::AddNode {
            name: name.clone(),
            node_type: "moc".to_string(),
            home: None,
        },
        Edit::MoveNode {
            node: name,
            was: (0, 0),
            now: MOC_PLACE,
        },
    ];
    edits.splice(0..0, placed);
    editor.edit(Edit::Group { edits });
}

/// A name no node in the document has yet.
///
/// The type's own spelling, a hyphen, and the first number that is free: a
/// person who places three i2c nodes gets i2c-1, i2c-2 and i2c-3 and renames
/// them, rather than being made to name each one before it can exist.
///
/// Numbered from the first one rather than from the second. The first used to
/// be the bare type -- i2c, then i2c-2 -- which reads as two kinds of name for
/// one kind of thing: a person with one of each ends up with a diagram of
/// types rather than of nodes, and the moment a second one is placed the first
/// one's name is the odd one out. A number on every one of them also says
/// plainly that the name is the editor's suggestion and not a description.
fn fresh_name(document: &Document, node_type: &str, home: &Home) -> String {
    let taken = |name: &str| {
        document
            .node(&Qualified::of(name, home).absolute())
            .is_some()
    };
    (1..)
        .map(|n| format!("{node_type}-{n}"))
        .find(|name| !taken(name))
        .expect("a free name exists")
}

/// How often a group reads until somebody says otherwise.
///
/// A second: slow enough for anything, fast enough to be obviously a value
/// somebody should look at rather than a default that happens to work.
const FIRST_INTERVAL: &str = "1 s";

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
/// A node's upper left corner sits on the millimetre grid, in steps of five
/// millimetres so that its terminals are on lines five millimetres apart --
/// see `pane::PX_PER_MM` and `pane::NODE_STEP_MM`. Three things come of it and none is merely decorative: a
/// diagram stops looking approximate, nodes line up without anybody nudging
/// them, and the positions written to the file are points a person can name,
/// so a file edited by hand and a file written by the editor differ in what
/// was meant rather than in a pixel.
fn snap((x, y): (i32, i32)) -> (i32, i32) {
    (pane::on_node_grid(x), pane::on_node_grid(y))
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

    alphabetical(&mut entries);
    ModelRc::from(Rc::new(VecModel::from(entries)))
}

/// Put each heading's entries in alphabetical order.
///
/// Within a heading and not across the panel, because a heading is a statement
/// about the entries under it -- "Devices", "Ends of the graph" -- and a list
/// sorted past its headings would file a device under whichever heading its
/// first letter landed in.
///
/// Alphabetical because a palette is a thing people look *in*, not a thing
/// they read: the order it had was the order the table happens to declare the
/// types in, which is no order at all to somebody hunting for `rpmsg`. The
/// headings themselves keep the order they are declared in, which is the one
/// place the table's own order says something -- devices before the things
/// that are driven, and the ends of the graph last.
fn alphabetical(entries: &mut [PaletteEntry]) {
    let mut at = 0;
    while at < entries.len() {
        if entries[at].is_heading {
            at += 1;
            continue;
        }
        let from = at;
        while at < entries.len() && !entries[at].is_heading {
            at += 1;
        }
        entries[from..at].sort_by(|one, two| {
            one.node.name.to_lowercase().cmp(&two.node.name.to_lowercase())
        });
    }
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
    // What a closed subdiagram holds, and nothing for any other type: see
    // `NodeView::summary`. A node's own comment is in its window.
    summary: Option<&str>,
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
    let (outline, edge, weight, size, tint, face, label_size, label_weight, label_italic) =
        match appearance {
            Some(a) => (
                a.outline,
                a.edge,
                a.weight,
                a.size,
                tint_of(node_type),
                a.face,
                a.label_size,
                a.label_weight,
                a.label_italic,
            ),
            None => (
                Outline::Square,
                Edge::Single,
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
            Outline::Parallelogram => "parallelogram",
        }
        .into(),
        // The shape to fill, the line to stroke and the second line of a
        // doubled edge, all as paths in the node's own coordinates, for every
        // outline. See style::polygon_of, which all three come from.
        shape: style::shape_of(outline, size).into(),
        edge: style::edge_of(outline, edge, size).into(),
        inner: style::inner_of(outline, edge, size).into(),
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
        // What is drawn *on* the body, now that the body is painted in the
        // tint: see NodeView::mark.
        mark: colour(MARK),
        faulty: fault.is_some(),
        selected,
        summary: summary.unwrap_or_default().into(),
        // A diagram of its own can be opened; nothing else can.
        openable: node_type == "subdiagram",
        // The terminals this type has, each drawn in its own corner. A type
        // this build does not know has none: the shape already says it cannot
        // be placed, and four arrows on it would be four claims about a type
        // nothing here can describe.
        moc_in: terminal_view("moc-in", terminals, outline, size, at),
        moc_out: terminal_view("moc-out", terminals, outline, size, at),
        payload_in: terminal_view("payload-in", terminals, outline, size, at),
        payload_out: terminal_view("payload-out", terminals, outline, size, at),
        // The column the name is padded away from, plus whatever the outline
        // is missing at its widest: a slanted shape moves its terminals in,
        // and a name padded by the column alone would be padded into them.
        terminal_column: (style::TERMINAL_COLUMN as i32
            + style::room_of(outline, size)) as f32,
        // The room anything drawn inside the shape has to leave at the sides,
        // which is nothing unless the shape leans: see style::room_of.
        inset: style::room_of(outline, size) as f32,
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
    outline: Outline,
    size: (u32, u32),
    // Where the node is, which is what puts each terminal on the grid: see
    // `style::terminal_at`.
    at: (i32, i32),
) -> TerminalView {
    let corner = style::corner_of(terminal);
    let (x, y) = corner
        .map(|corner| style::terminal_at(outline, corner, size, at))
        .unwrap_or_default();
    TerminalView {
        shown: terminals.has(terminal),
        x: x as f32,
        y: y as f32,
        rightward: corner.is_some_and(|corner| corner.rightward),

    }
}

/// What a line is drawn in, by the stream it carries.
///
/// A line used to be grey, whichever way its data went, so a reader tracing a
/// control loop had to tell the two streams apart by which corner each line
/// left -- which is reading a diagram by its arithmetic. Now the colour says
/// it: a line carrying the payload's own data is of the payload family, and
/// one carrying what the ground sends is of the ground's.
///
/// The middle of the three tiers the palette is built as, and a shade of the
/// pane its data comes from: the payload stream leaves the payload pane and
/// is drawn in a darker teal, the ground's stream leaves the moc pane and is
/// drawn in a darker blue. A drawing then reads as one picture -- a pale
/// sheet, lines on the sheet, solid blocks on the lines -- a line can never be
/// mistaken for a node's own edge, and the colour of a line says which end of
/// the canvas its data started at.
///
/// Written down rather than computed, because the palette is written down --
/// the maths lives in `sight.rs`, which is test-only. What holds these to the
/// tiers is `the_palette_runs_from_the_pane_through_the_line_to_the_bodies`,
/// and what holds them apart from everything else is
/// `no_two_colours_of_the_scheme_look_alike`.
///
/// Grey for a line whose stream cannot be told, which is a line this build
/// does not understand: it is drawn, because a connection missing from a
/// drawing is worse than one drawn in the wrong colour.
fn line_ink(leaves_by: Option<&str>) -> String {
    match leaves_by {
        Some("payload-out") => PAYLOAD_LINE,
        Some("moc-out") => MOC_LINE,
        _ => "#6b7280",
    }
    .to_string()
}

/// What is written on a node: its outline, its name, its terminals.
///
/// White. A body is painted in its own tint now, and the tints are the darkest
/// of the three tiers the palette is built as -- a working pane is the
/// lightest shade of its family, a link line is darker, and the bodies are
/// darker still -- so what is written on a body has to be legible on the
/// lightest body and the darkest alike. White clears four and a half to one
/// against every one of them, which is the threshold for text, and a test says
/// so.
///
/// Not the pane's own ground, which was the first thing tried: the payload
/// pane is the darkest of the three at 84 of lightness, and its own shade on
/// its own lightest body is 3.4 to one -- under the floor for a name somebody
/// has to read.
pub const MARK: &str = "#ffffff";

/// The payload stream's line: a darker shade of the payload pane, at 58.2 of
/// lightness.
///
/// A line is drawn in the colour of the pane its data comes *from*, darker.
/// The payload's own readings start in the payload pane and generally end at
/// the ground, so the line is a teal taken from the teal pane they leave --
/// which means a reader can see where a line's data came from without
/// following it back. What it is darker *than* is also what it crosses, so the
/// shade is where it is for the reason below.
///
/// Twenty-four points darker, not "a bit". That is what the two other rules
/// leave: a line has to stay eight points above the lightest node body, which
/// puts the floor at 57.4, and it has to be seen on a pane at 90 of lightness,
/// which puts the lightest usable shade at 58.2 -- the two meet, and there is
/// nothing in between. Anything lighter reaches 1.3 to 1 against its own pane
/// and disappears; nothing in the gamut reaches 2.5 to one at all while
/// staying clear of the bodies. See
/// `tests::the_palette_runs_from_the_pane_through_the_line_to_the_bodies`.
pub const PAYLOAD_LINE: &str = "#289aa2";

/// And what the ground sends: a darker shade of the moc pane, at 58.2.
///
/// Thirty-two points darker, for the same reasons, and still a blue -- the
/// ground's own hue, which is the one thing about the lines that did not have
/// to change.
pub const MOC_LINE: &str = "#0292e4";

/// What a node of this type is painted.
///
/// A rung of its own panel's ramp. Each panel starts twelve CIE units below
/// the line that carries its data -- the moc panel below the moc line, the
/// payload panel below the payload line, and the vehicle panel below both,
/// having no line of its own -- and steps three and a half units darker for
/// each node, taking the panel's nodes in alphabetical order. A reader can
/// then find a node in a panel by how dark it is, and the panel reads as one
/// family rather than as a set of unrelated colours.
///
/// Twelve for the first step and not the eight the tiers ask, because the moc
/// family's line and its bodies are the same hue: at the same hue a step in
/// lightness *is* the difference, and twelve is the floor two colours of this
/// scheme are held apart by.
///
/// The ramp stops at thirteen of lightness, which is where the three families
/// stop being told apart: below it a dark blue, a dark green and a dark teal
/// converge under deuteranopia. A panel
/// with more nodes than its ramp has rungs starts again at the top, so two
/// nodes of one panel may share a colour -- which is allowed here because
/// colour is the redundant channel: every pair of types differs in two of
/// outline, edge, border width, size and label, and
/// `style::NodeType::appearance` is held to that by its own test.
///
/// What this gave up is the twelve units between *neighbours* in one panel.
/// A ramp of ten in one hue family cannot keep it -- see `design.rst` -- so
/// within a panel the steps are three to five units, which is "a shade
/// darker", and across panels the closest pair is 30.9. The ramp is the
/// information: a node's place in its panel is its colour's place in the
/// ramp.
///
/// Searched offline and written down, as the palette before it was: the
/// maths lives in `sight.rs`, which is test-only, and
/// `tests::every_panel_is_a_ramp_from_its_own_line` holds these to the rule.
fn tint_of(node_type: &str) -> String {
    let pane = pane_of(node_type);
    let rungs = match pane {
        Pane::Moc => MOC_RUNGS,
        Pane::Vehicle => VEHICLE_RUNGS,
        Pane::Payloads => PAYLOAD_RUNGS,
    };
    let order = ramp_order(pane);
    // Where this type comes in its own panel, alphabetically. A type this
    // build does not know is not in any panel and is drawn as a fault, so the
    // first rung is as good an answer as any.
    let at = order.iter().position(|name| *name == node_type).unwrap_or(0);
    rungs[at % rungs.len()].to_string()
}

/// The ground link's panel: one node, so one rung.
const MOC_RUNGS: &[&str] = &["#0073b5"];

/// The vehicle's own graph, in the order the panel lists it: compute, log,
/// null, subdiagram.
const VEHICLE_RUNGS: &[&str] = &["#547800", "#4b7000", "#446600", "#3d5d00"];

/// And what it carries, which is the longest ramp: ten rungs from 46.2 of
/// lightness down to 15.0, three and a half units apart, which is as far as
/// the floor allows.
const PAYLOAD_RUNGS: &[&str] = &[
    "#007a7d", "#007173", "#00676a", "#005e61", "#005557", "#004c4e", "#004446",
    "#003b3d", "#003335", "#002b2c",
];

/// Each panel's own node types in the order the ramp is walked: the types the
/// rungs were chosen for first, alphabetically, and then the apps,
/// alphabetically.
///
/// The apps came later than the rungs, and a type that comes later takes the
/// colours from the top of the ramp again rather than its place in the
/// alphabet: sorted in with the rest, app_all_pl and app_pl took the first two
/// payload rungs and moved every device a rung or two down, so that a can
/// node was no longer the colour a can node had been. Wrapping is allowed --
/// see `tint_of` -- and moving a colour that is already in use is not.
fn ramp_order(pane: Pane) -> Vec<&'static str> {
    let mut types: Vec<NodeType> = NodeType::ALL
        .iter()
        .copied()
        .filter(|t| pane_of(t.spelling()) == pane)
        .collect();
    types.sort_by_key(|t| (is_app(*t), t.spelling()));
    types.iter().map(|t| t.spelling()).collect()
}

/// Whether a type is one of the five apps, which take their colours after the
/// rest of their panel's: see `ramp_order`.
fn is_app(node_type: NodeType) -> bool {
    matches!(
        node_type,
        NodeType::AppPayloadVeh
            | NodeType::AppMocVeh
            | NodeType::AppAllVeh
            | NodeType::AppPl
            | NodeType::AppAllPl
    )
}

/// The hue each family of colours is built around, in degrees of the CIE
/// wheel.
///
/// Declared rather than read off the pane, which is the change the lilac-grey
/// payload pane forced and an improvement on its own: the rule is that a
/// node's colour says which pane it belongs in, and a pane with a chroma of
/// 1.8 has no hue to be said with -- the number its hue comes out as is
/// arithmetic on a rounding error. The family is the thing that has a hue, and
/// the bodies and the line of a family are held to it.
///
/// The blue the ground station is drawn in, the green of the vehicle's own
/// graph, and the teal of what it carries. Kept far apart from each other --
/// the test below says 60 degrees at least -- because two families a reader
/// cannot tell apart are two families that say nothing about where a node
/// belongs.
///
/// For the tests, which hold the palette to these hues: the colours
/// themselves are written down, in `tint_of` and `ground_of`.
#[cfg(test)]
fn family_hue(pane: Pane) -> f64 {
    match pane {
        Pane::Moc => 265.0,
        Pane::Vehicle => 118.0,
        Pane::Payloads => 200.0,
    }
}

fn ground_of(ground: style::Ground) -> String {
    match ground {
        style::Ground::Moc => "#dee2fc",
        style::Ground::Vehicle => "#e8e4ca",
        style::Ground::Payloads => "#bad2d0",
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

    /// And how the selection column is divided, until somebody drags a line.
    fn lists() -> (i32, i32) {
        (settings::DEFAULT_LIST_MOC, settings::DEFAULT_LIST_VEHICLE)
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

    /// The palette runs from the pane, through the line, to the bodies.
    ///
    /// Three tiers of one hue family, lightest first: the working pane is the
    /// palest shade of it, a link line carrying that family's data is darker,
    /// and the node bodies are darker still. It is what makes a diagram read
    /// as one picture -- a pale sheet with lines drawn on it and solid blocks
    /// sitting on top -- rather than as three unrelated colour choices, and it
    /// is why a body is painted in its own tint and written on in white.
    ///
    /// The step between tiers is at least eight CIE units, which is "clearly
    /// lighter" rather than "possibly lighter"; what the whole scheme is held
    /// to as a floor is a separate question, asked by
    /// `no_two_colours_of_the_scheme_look_alike`.
    #[test]
    fn the_palette_runs_from_the_pane_through_the_line_to_the_bodies() {
        use sight::{contrast, hue, hue_apart, lightness};

        let painted = |hex: &str| {
            let c = colour(hex);
            (c.red(), c.green(), c.blue())
        };

        let panes = [Pane::Moc, Pane::Vehicle, Pane::Payloads];
        let lines = [PAYLOAD_LINE, MOC_LINE];
        let mark = painted(MARK);

        // Every pane is lighter than every line. Every line crosses every
        // pane -- a payload's data is drawn from the payload pane to the
        // ground one -- so this is asked of all six pairs rather than of each
        // line against the pane it belongs to.
        for pane in panes {
            let ground = painted(&ground_of(style::ground_of(pane)));
            for line in lines {
                let step = lightness(ground) - lightness(painted(line));
                assert!(
                    step >= 8.0,
                    "the {} pane is only {step:.1} lighter than a line on it",
                    pane.label()
                );
            }
        }

        // And each line is a shade of the pane its data comes from. A
        // payload's readings start in the payload pane and generally end at
        // the ground; what the ground sends starts in the moc pane and ends
        // at a payload. So the line's colour says which end of the canvas its
        // data started at, without a reader following it back.
        //
        // Twenty degrees, which is "the same colour, darker" rather than "a
        // related colour": a pane and its line have to be recognisably one
        // family, and the hue of a wash is a little unsteady -- at a chroma of
        // 9 a step of one in a channel moves it several degrees.
        for (what, line, pane) in [
            ("payload", PAYLOAD_LINE, Pane::Payloads),
            ("moc", MOC_LINE, Pane::Moc),
        ] {
            let ground = painted(&ground_of(style::ground_of(pane)));
            let apart = hue_apart(hue(painted(line)), hue(ground));
            assert!(
                apart <= 20.0,
                "the {what} line is {apart:.0} degrees from the {} pane it is \
                 a shade of",
                pane.label()
            );
            // Darker than it, and by more than the tier asks: these two are
            // as light as a line can be and still be seen on a pane, which is
            // further down than "a bit" -- see PAYLOAD_LINE.
            let step = lightness(ground) - lightness(painted(line));
            assert!(
                step >= 8.0,
                "the {what} line is only {step:.1} darker than its own pane"
            );
        }

        // And every line is lighter than every body, for the same reason: a
        // line arrives at nodes of both other panes.
        for t in NodeType::ALL {
            let body = painted(&tint_of(t.spelling()));
            for line in lines {
                let step = lightness(painted(line)) - lightness(body);
                assert!(
                    step >= 8.0,
                    "{} is only {step:.1} darker than a line that reaches it",
                    t.spelling()
                );
            }

            // What is written on a body has to be readable on it: a name is
            // text, and four and a half to one is the threshold for text.
            let against = contrast(mark, body);
            assert!(
                against >= 4.5,
                "{}'s name is {against:.2} against its own body",
                t.spelling()
            );
        }

        // And the bodies are a range of shades rather than one shade: the
        // tier is "darker still", and a tier of fifteen colours all at one
        // darkness would not read as the bottom of a scale.
        //
        // The span, and not a ladder of one body per rung. A rung per body
        // was tried: ten payload bodies, each a step darker than the last and
        // each far enough from the rest to be told apart by a reader with a
        // dichromacy, does not exist. Under protanopia and deuteranopia a
        // single hue family is very nearly a lightness scale, so ten bodies
        // at the floor below would want ten steps of fourteen -- a hundred and
        // forty units of lightness out of the eighty-seven there are. What a
        // ladder costs is measured: forcing one drops the floor from 12 to
        // about 10, and re-fitting the family to get it back leaves each
        // colour's name describing a colour it no longer is.
        let bodies: Vec<f64> = NodeType::ALL
            .iter()
            .map(|t| lightness(painted(&tint_of(t.spelling()))))
            .collect();
        let span = bodies.iter().cloned().fold(f64::MIN, f64::max)
            - bodies.iter().cloned().fold(f64::MAX, f64::min);
        assert!(
            span >= 25.0,
            "the bodies span {span:.1} of lightness, which is not a range of \
             shades"
        );

        // And the two lines are told apart by anybody: the whole point of
        // colouring them is that a reader can see which stream a line
        // carries without tracing it to its corner.
        use sight::{difference, seen, Sight};
        let payload = painted(PAYLOAD_LINE);
        let moc = painted(MOC_LINE);
        for sight in Sight::ALL {
            let apart = difference(seen(payload, sight), seen(moc, sight));
            assert!(
                apart >= 14.0,
                "the two lines are {apart:.1} apart under {sight:?}, which is \
                 the floor the palette is held to"
            );
        }
    }

    /// How much a selection list spends above its first node.
    ///
    /// Its own heading and the padding and spacing around it, and the family
    /// heading the first entry sits under where there is one -- the numbers
    /// the markup lays a `SelectionPane` out with: eight of padding, an
    /// eighteen-pixel heading, eight of spacing, a twenty-two-pixel family
    /// heading, and eight of row padding.
    ///
    /// Here so that the rule below can be a test rather than a hope. It is a
    /// claim about the markup, and a markup changed without this is a list
    /// whose first node is a pixel off the bottom of it -- which is why the
    /// test reads the numbers out of `main.slint` and holds them against this.
    const ABOVE_THE_FIRST: f32 = 8.0 + 18.0 + 8.0 + 22.0 + 8.0;

    /// Every selection list shows a node when the window opens.
    ///
    /// Asked for, and the thing a share of a column cannot be trusted to do
    /// by itself: a list given a twentieth of an 820-pixel window has
    /// forty-one pixels, which is its own heading and nothing else -- a list
    /// somebody has to discover is there by dragging a line they cannot see
    /// the point of.
    ///
    /// So the three default shares are checked against what the three lists
    /// actually have to draw, at the height the window opens at. The window's
    /// own preferred height is read out of the markup rather than written
    /// here twice, and so is the padding above the first row: a rule about
    /// what fits is a rule about numbers in two files, and a test that
    /// restated them would pass while the window showed nothing.
    #[test]
    fn every_selection_list_shows_a_node_when_the_window_opens() {
        let markup = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/main.slint"),
        )
        .expect("the markup this binary was built from");

        // The main window's preferred height, which is what it opens at.
        let tall: f32 = markup
            .split("export component MainWindow")
            .nth(1)
            .and_then(|from| from.split("preferred-height:").nth(1))
            .and_then(|said| said.split(';').next())
            .and_then(|said| said.trim().strip_suffix("px"))
            .and_then(|said| said.trim().parse().ok())
            .expect("the markup says what height the window opens at");

        // And what it spends above the first row, which this test's own
        // constant claims: the padding, the headings and the spacing.
        for said in ["out property <length> step: 8px;", "height: 18px;"] {
            assert!(
                markup.contains(said),
                "the markup no longer says {said:?}, so ABOVE_THE_FIRST is \
                 out of date"
            );
        }

        for (pane, share) in [
            (Pane::Moc, settings::DEFAULT_LIST_MOC),
            (Pane::Vehicle, settings::DEFAULT_LIST_VEHICLE),
            (
                Pane::Payloads,
                100 - settings::DEFAULT_LIST_MOC - settings::DEFAULT_LIST_VEHICLE,
            ),
        ] {
            let rows = rows_of(pane);
            let first = rows
                .iter()
                .find(|row| !row.is_heading)
                .expect("every list offers something");
            // At the size the list draws it, which the markup says.
            let shrink: f32 = markup
                .split("property <float> shrink:")
                .nth(1)
                .and_then(|rest| rest.split(';').next())
                .and_then(|said| said.trim().parse().ok())
                .expect("the markup says how small a list draws its shapes");
            let wanted = ABOVE_THE_FIRST + first.node.height * shrink;
            let got = tall * share as f32 / 100.0;
            assert!(
                got >= wanted,
                "the {} list opens {got:.0} pixels tall and its first node \
                 wants {wanted:.0}",
                pane.label()
            );
        }
    }

    /// The read-only flag is taken in all three spellings and anywhere on the
    /// line, and nothing else that looks like a flag is taken at all.
    #[test]
    fn the_command_line_takes_the_read_only_flag() {
        let said = |words: &[&str]| command_line(words.iter().map(|w| w.to_string()));
        assert_eq!(said(&["a.yaml"]), Ok((vec!["a.yaml".to_string()], false)));
        for flag in ["-r", "--read-only", "--readonly"] {
            assert_eq!(said(&[flag, "a.yaml"]), Ok((vec!["a.yaml".to_string()], true)));
            assert_eq!(
                said(&["a.yaml", "a-sim.yaml", flag]),
                Ok((vec!["a.yaml".to_string(), "a-sim.yaml".to_string()], true))
            );
        }
        assert!(said(&["--readonley", "a.yaml"]).is_err(), "a misspelled flag");
        assert!(said(&["-x", "a.yaml"]).is_err());
        assert!(said(&["a.yaml", "b.yaml", "c.yaml"]).is_err());
    }

    /// In a read-only session the problems window's Edit and Delete are
    /// greyed, and what would rename or act on a node is greyed with them;
    /// the ways out of the window are not.
    #[test]
    fn a_read_only_session_greys_what_would_edit() {
        use i_slint_backend_testing::ElementHandle;

        i_slint_backend_testing::init_no_event_loop();
        let window = ProblemsWindow::new().expect("a window");
        window.global::<Session>().set_read_only(true);
        window.set_rows(ModelRc::from(Rc::new(VecModel::from(vec![ProblemRow {
            said: "\"twice\" is the name of two nodes".into(),
            node: "twice".into(),
            now: "twice".into(),
            nameable: true,
            there: true,
            action: "".into(),
        }]))));
        window.set_any_nameable(true);
        window.set_ready(true);
        window.show().expect("the window shows");

        let enabled = |label: &str| {
            ElementHandle::find_by_accessible_label(&window, label)
                .next()
                .unwrap_or_else(|| panic!("there is no {label} button"))
                .accessible_enabled()
        };
        for label in ["Edit", "Delete", "Suggest", "Rename"] {
            assert_eq!(enabled(label), Some(false), "{label} is not greyed");
        }
        for label in ["Quit", "Leave this one", "Ignore them all"] {
            assert_eq!(enabled(label), Some(true), "{label} is greyed");
        }

        // And the same window in a session that may edit has them all.
        window.global::<Session>().set_read_only(false);
        for label in ["Edit", "Delete", "Suggest", "Rename"] {
            assert_eq!(enabled(label), Some(true), "{label} is greyed");
        }
    }

    /// Every popup says what it says in text that can be copied, and has a
    /// button that copies all of it.
    ///
    /// Asked of the markup, as the other rules about the window are: a
    /// `Text` in one of these is a message somebody would have to type out
    /// again to pass it on. And the two that can come up as the editor starts
    /// can be held in front of the main window while it waits for them.
    #[test]
    fn every_popup_can_be_copied_from() {
        let markup = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/main.slint"),
        )
        .expect("the markup");
        let window = |name: &str| -> &str {
            let start = markup
                .find(&format!("export component {name} inherits Window {{"))
                .unwrap_or_else(|| panic!("there is no {name}"));
            let rest = &markup[start..];
            &rest[..rest.find("\n}\n").expect("the end of it")]
        };
        for name in [
            "AlertWindow",
            "ConcernsWindow",
            "ProblemsWindow",
            "DeleteWindow",
            "LeaveWindow",
        ] {
            let said = window(name);
            assert!(!said.contains("Text {"), "{name} says something that cannot be copied");
            assert!(said.contains("Copyable {"), "{name} says nothing");
            assert!(said.contains("CopyButton {"), "{name} has no Copy button");
        }
        for name in ["AlertWindow", "ProblemsWindow"] {
            assert!(
                window(name).contains("always-on-top: root.on-top;"),
                "{name} cannot be held in front of the main window"
            );
        }
    }

    /// A line between two lists cannot squeeze either of them away.
    ///
    /// The floor asked for: a twentieth of the column, whichever line is
    /// dragged and however far. A list squeezed to nothing is a list whose
    /// entries cannot be reached, and a line dragged to the end of its travel
    /// is easy to do by accident and hard to understand afterwards -- so the
    /// drag stops rather than the list disappearing.
    #[test]
    fn a_line_between_the_lists_cannot_squeeze_one_away() {
        // A column 800 pixels tall, and each line dragged a long way past
        // either end of its travel.
        for (which, dy) in [
            (Pane::Payloads, -4000.0),
            (Pane::Payloads, 4000.0),
            (Pane::Vehicle, -4000.0),
            (Pane::Vehicle, 4000.0),
        ] {
            let (moc, vehicle) = list_shares_after(lists(), which, dy, 800.0);
            let payload = 100 - moc - vehicle;
            for (name, share) in [("moc", moc), ("vehicle", vehicle), ("payload", payload)] {
                assert!(
                    share >= settings::LEAST,
                    "dragging the {which:?} line by {dy} would leave the \
                     {name} list {share}% of the column"
                );
            }
        }

        // An ordinary drag moves the two it is between and leaves the third
        // alone: the list above grows by what the list below gives up. The
        // upper line gives the payload list, which has no share of its own,
        // what it takes from the vehicle list.
        let (was_moc, was_vehicle) = lists();
        let (moc, vehicle) = list_shares_after(lists(), Pane::Payloads, 80.0, 800.0);
        assert_eq!((moc, vehicle), (was_moc, was_vehicle - 10));

        // And the lower line takes from the moc list and gives to the
        // vehicle's, leaving the payload list alone.
        let (moc, vehicle) = list_shares_after(lists(), Pane::Vehicle, 80.0, 800.0);
        assert_eq!((moc, vehicle), (was_moc - 10, was_vehicle + 10));
        assert_eq!(
            100 - moc - vehicle,
            100 - was_moc - was_vehicle,
            "the third list is left alone"
        );

        // A gesture that worked out to nothing changes nothing, so nothing is
        // redrawn and no settings file is written: a line nudged by a pixel on
        // a tall column is not a preference.
        assert_eq!(list_shares_after(lists(), Pane::Payloads, 2.0, 800.0), lists());
    }

    /// Every problem is offered, with what can be done about that kind of it.
    ///
    /// The list the startup window is built from. Every fault gets a row --
    /// a row that offers nothing is what the alert used to be, and an alert
    /// with nothing to press is a complaint rather than a question -- and
    /// what differs is the offer: a name, the node, the group, the line, or
    /// the parameters window.
    #[test]
    fn every_problem_is_offered_the_fix_its_kind_has() {
        let mut document = Document::new();
        // A node named twice, a node of a type this build does not know, a
        // line to a node that is not there, and a group nothing is in.
        for (name, node_type) in [("i2c", "i2c"), ("i2c", "i2c"), ("odd", "contraption")] {
            document.apply(&Edit::AddNode {
                name: name.to_string(),
                node_type: node_type.to_string(),
                home: None,
            });
        }
        document.apply(&Edit::AddLink {
            link: Link::new("i2c", "nowhere", None),
        });
        document.apply(&Edit::AddGroup {
            name: "every_second".to_string(),
            interval: "1 s".to_string(),
        });

        let troubles = troubles_of(&document);
        let fix_for = |which: &str| {
            troubles
                .iter()
                .find(|t| t.said.contains(which))
                .map(|t| t.fix.clone())
        };

        // Both nodes of the shared name, because the file does not say which
        // of them was meant, and each by its own position so a rename can
        // reach it.
        let shared: Vec<&Trouble> = troubles.iter().filter(|t| t.node == "i2c").collect();
        assert_eq!(shared.len(), 2, "{troubles:?}");
        assert_eq!((shared[0].at, shared[1].at), (Some(0), Some(1)));
        assert!(shared.iter().all(|t| t.nameable()), "a name is the fix");

        // The unknown type: the node has to be looked at or taken out.
        assert_eq!(fix_for("contraption"), Some(Fix::Node));

        // The line that goes nowhere: deleting it is the fix, and the line
        // offered is the one the document holds -- a removal is matched by
        // what a link joins.
        match fix_for("one of them is not here") {
            Some(Fix::Line(line)) => {
                assert_eq!((line.from.as_str(), line.to.as_str()), ("i2c", "nowhere"));
            }
            other => panic!("the dangling line should offer its own removal: {other:?}"),
        }

        // And the group nothing is in, which offers its own deletion.
        assert_eq!(
            fix_for("has no nodes in it"),
            Some(Fix::Group("every_second".to_string()))
        );

        // Every fault has a row, which is the rule: nothing reaches the alert
        // with no offer beside it.
        assert_eq!(
            troubles.len(),
            document.faults().len() + 1,
            "one row per fault, and one extra because a shared name is two \
             rows: {troubles:?} against {:?}",
            document.faults()
        );
        // And every row says something and offers something.
        for trouble in &troubles {
            assert!(!trouble.said.is_empty(), "{trouble:?}");
            assert!(
                trouble.nameable() || trouble.at.is_some() || !trouble.offer().is_empty(),
                "a row with nothing to press: {trouble:?}"
            );
        }
    }

    /// Doing nothing leaves the node on the canvas, in its own working pane.
    ///
    /// The offer the window closes with, and the reason it can be offered: a
    /// node the editor cannot make sense of is still drawn, still in the pane
    /// its type belongs to, and still marked. An editor that would not show a
    /// configuration until it was correct could not be used to correct one.
    #[test]
    fn a_node_nobody_fixed_is_still_drawn_in_its_pane() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.yaml");
        std::fs::write(
            &path,
            "vehicle:\n  - name: odd\n    type: contraption\n",
        )
        .expect("a configuration naming a type this build has not got");

        let editor = Editor::open(&path, dividers());
        assert!(
            !troubles_of(editor.document()).is_empty(),
            "the unknown type should be a problem"
        );

        // On the canvas, in the vehicle pane -- which is where a type nothing
        // knows goes, because the two other panes mean something about what a
        // node is and this says nothing.
        let (nodes, _) = views_of(&editor, &None, None);
        let drawn = nodes
            .iter()
            .find(|view| view.name == "odd")
            .expect("the node should be drawn");
        assert!(drawn.faulty, "and marked as the problem it is");
        assert_eq!(pane_of("contraption"), Pane::Vehicle);
        let (left, right) = pane::pane_for("contraption", dividers());
        let at = editor.document().node("odd").expect("the node").at;
        assert!(
            at.0 >= left && at.0 <= right,
            "a node nobody fixed should sit in its own pane: {at:?} against \
             {left}..{right}"
        );
    }

    /// What tcsverify finds is listed as the editor starts, and what the
    /// editor already marks is not listed twice.
    ///
    /// The editor is lazy about a node that is not finished; the rules are
    /// not, and a vehicle would refuse the file. So the startup list says what
    /// tcsverify says about it -- in its words, with the file and the line --
    /// beside the editor's own rows.
    #[test]
    fn the_startup_list_says_what_tcsverify_says() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.yaml");
        std::fs::write(
            &path,
            "payload_groups:\n  - name: idle\n    interval: 3 s\n\
             payloads:\n  - name: a\n    type: i2c\n    payload-out: moc\n\
             moc:\n  - name: moc\n    type: moc\n",
        )
        .expect("a configuration with an empty group and an unfinished node");

        let editor = Editor::open(&path, dividers());
        let troubles = startup_troubles(&editor);
        let said: Vec<&str> = troubles.iter().map(|t| t.said.as_str()).collect();

        // The empty group is the editor's own fault and the rules' too: one
        // row, the editor's, which offers to delete it.
        assert_eq!(
            said.iter().filter(|said| said.contains("\"idle\"")).count(),
            1,
            "{said:#?}"
        );

        // The unfinished node is the rules' alone, and it is there, said as
        // tcsverify says it and offering the node.
        let report = tcsverify::verify(&path, None, tcsverify::Limits::EVERYTHING);
        let unfinished = report
            .findings
            .iter()
            .find(|f| f.severity == tcslibgs::graph::check::Severity::Warning)
            .expect("tcsverify should warn about the unfinished node");
        let row = troubles
            .iter()
            .find(|t| t.said == unfinished.to_string())
            .unwrap_or_else(|| panic!("{unfinished} should be listed: {said:#?}"));
        assert_eq!(row.fix, Fix::Node);
        assert!(row.at.is_some());

        // And nothing tcsverify reports is missing from the list.
        for finding in &report.findings {
            let listed = troubles.iter().any(|t| t.said == finding.to_string())
                || finding
                    .problem
                    .as_ref()
                    .is_some_and(|p| said_already(p, &editor.faults()));
            assert!(listed, "{finding} is not listed: {said:#?}");
        }
    }

    /// A configuration that is not there yet has nothing wrong with it.
    #[test]
    fn a_new_configuration_starts_with_nothing_to_say() {
        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Editor::open(&where_.path().join("new.yaml"), dividers());
        assert!(verified(&editor).is_empty());
    }

    /// A set of names settles a duplicate only if all of them are different,
    /// none is empty, and none belongs to a node that is not being renamed.
    #[test]
    fn the_names_offered_have_to_settle_the_duplicate() {
        let mut document = Document::new();
        for (name, node_type) in [("i2c", "i2c"), ("i2c", "i2c"), ("moc", "moc")] {
            document.apply(&Edit::AddNode {
                name: name.to_string(),
                node_type: node_type.to_string(),
                home: None,
            });
        }

        let asked = |pairs: &[(usize, &str)]| -> BTreeMap<usize, String> {
            pairs
                .iter()
                .map(|(at, name)| (*at, (*name).to_string()))
                .collect()
        };

        // As the file has them: still the same name, which is what the window
        // opens on and what the button has to wait for.
        assert!(names_settle(&document, &asked(&[(0, "i2c"), (1, "i2c")])).is_err());

        // One of them changed is enough.
        assert!(names_settle(&document, &asked(&[(0, "i2c"), (1, "i2c-1")])).is_ok());
        // Both, to something else again.
        assert!(names_settle(&document, &asked(&[(0, "fore"), (1, "aft")])).is_ok());

        // A blank is not a name.
        assert!(names_settle(&document, &asked(&[(0, "i2c"), (1, "  ")])).is_err());

        // And a third node's name is a duplicate again: a button that made
        // the same question come back would be no answer at all.
        let said = names_settle(&document, &asked(&[(0, "i2c"), (1, "moc")]))
            .expect_err("renaming one of them to the ground station's name");
        assert!(said.contains("moc"), "{said}");

        // What is said is about what is wrong, because it is said instead of
        // the button being pressable.
        let said = names_settle(&document, &asked(&[(0, "same"), (1, "same")]))
            .expect_err("two the same");
        assert!(said.contains("same"), "{said}");
    }

    /// Each heading of each selection panel is in alphabetical order.
    ///
    /// A palette is a thing people look in rather than read, and the order it
    /// had was the order the type table happens to declare things in -- which
    /// is no order at all to somebody hunting for `rpmsg`. Within a heading
    /// and not across the panel: a heading is a statement about the entries
    /// under it, and a list sorted past its headings would file a device under
    /// whichever heading its first letter landed in.
    #[test]
    fn every_panel_is_in_alphabetical_order_under_each_heading() {
        for pane in [Pane::Moc, Pane::Vehicle, Pane::Payloads] {
            let rows = rows_from(pane, descriptions());
            let mut heading = String::new();
            let mut under: Vec<String> = Vec::new();
            let check = |heading: &str, under: &mut Vec<String>| {
                let mut sorted = under.clone();
                sorted.sort_by_key(|name| name.to_lowercase());
                assert_eq!(
                    *under,
                    sorted,
                    "{}, under \"{heading}\", is not in alphabetical order",
                    pane.label()
                );
                under.clear();
            };

            for row in &rows {
                if row.is_heading {
                    check(&heading, &mut under);
                    heading = row.heading.to_string();
                    continue;
                }
                under.push(row.node.name.to_string());
            }
            check(&heading, &mut under);
            assert!(!rows.is_empty(), "{} offers nothing", pane.label());
        }
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

    /// The description tree is what each pane offers.
    ///
    /// A type the tree describes is offered as its descriptions and not as
    /// itself: `compute` is one node type, and the tree turns it into `mean`,
    /// `hysteresis`, `to_kelvin` and `process`. Each entry is named after the
    /// description and carries what the description answers for it, so placing
    /// one puts down a compute node that is already a mean.
    ///
    /// Every pane, now that every type is described: a payload kind and the
    /// ground link have a description apiece, which is where their terminals
    /// are declared, and each is offered in its own pane under its own name.
    #[test]
    fn the_vehicle_pane_offers_what_the_tree_describes() {
        let tree = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("share/nodes");
        let (described, trouble) = nodes::read_tree(&tree);
        assert_eq!(trouble, Vec::<String>::new());

        // Every description is offered, in whichever pane its type belongs to.
        let everywhere: Vec<String> = [Pane::Moc, Pane::Vehicle, Pane::Payloads]
            .into_iter()
            .flat_map(|pane| {
                rows_from(pane, &described)
                    .into_iter()
                    .filter(|row| !row.is_heading)
                    .map(|row| row.node.name.to_string())
                    .collect::<Vec<String>>()
            })
            .collect();
        for one in &described {
            assert!(
                everywhere.contains(&one.node),
                "{} is not offered: {everywhere:?}",
                one.node
            );
        }

        let rows = rows_from(Pane::Vehicle, &described);
        let named: Vec<String> = rows
            .iter()
            .filter(|row| !row.is_heading)
            .map(|row| row.node.name.to_string())
            .collect();
        // And the bare type is not offered beside them: a compute node with no
        // function is refused until somebody types one in.
        assert!(!named.contains(&"compute".to_string()), "{named:?}");
        // A page of its own is in no description -- what a page holds is what
        // has terminals -- so it is still offered as itself.
        assert!(named.contains(&"subdiagram".to_string()), "{named:?}");

        // What a placed node is given, as the one string a callback carries.
        let voter = rows
            .iter()
            .find(|row| row.node.name == "vote")
            .expect("the vote entry");
        assert_eq!(voter.node.node_type, "compute");
        assert_eq!(voter.sets, "function=vote");
        assert_eq!(
            stated(&voter.sets),
            vec![("function".to_string(), "vote".to_string())]
        );

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

    /// Every outline the views carry is one the markup draws.
    ///
    /// The word is the whole of the agreement between `style` and the markup:
    /// an outline spelled one way here and another way there is drawn as a
    /// plain square, silently, because the markup's last branch is the one
    /// with no test on it. A path for the shapes a corner radius cannot draw
    /// is the same seam twice over -- the word and the data have to arrive
    /// together -- so both are checked here, against the markup itself.
    #[test]
    fn every_outline_is_one_the_markup_knows() {
        let markup = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/main.slint"),
        )
        .expect("the markup this binary was built from");

        for kind in style::NodeType::ALL {
            let view = view_of("n", kind.spelling(), (0, 0), None, false, None);
            let outline = view.outline.to_string();
            // One of three means, and exactly one. A path is how the markup
            // asks about a shape a corner radius cannot draw, so a type
            // carrying one is drawn whatever its word says. Square is the
            // branch with nothing to test: the markup asks about every other
            // radius outline and draws a square when none of them answers, so
            // it is the one word agreed to by being absent. Everything else
            // has to be asked about by name.
            let shaped = !view.shape.is_empty()
                || outline == "square"
                || markup.contains(&format!("node.outline == \"{outline}\""));
            assert!(
                shaped,
                "{} is drawn as {outline:?} with no path, and the markup never \
                 asks about that word",
                kind.spelling()
            );
            // And the path the view carries is the one the table drew. Which
            // outlines need a path is style's own question, asked by its own
            // test; this is the handover, which is the part that can go wrong
            // here -- a shape left out of the view is an outline drawn as a
            // plain square with nothing to say so.
            let drawn = kind.appearance();
            assert_eq!(
                view.shape.to_string(),
                style::shape_of(drawn.outline, drawn.size),
                "{}'s path did not reach its view",
                kind.spelling()
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

    /// The apps take the payload colours again from the top, and the devices
    /// keep the colours they had before there were apps.
    #[test]
    fn the_apps_reuse_the_payload_colours_in_order() {
        let devices = [
            "can", "device-file", "gpio", "hdlc", "i2c", "iio", "network", "rpmsg",
            "serial", "spi",
        ];
        for (rung, name) in devices.iter().enumerate() {
            assert_eq!(tint_of(name), PAYLOAD_RUNGS[rung], "{name}");
        }
        assert_eq!(tint_of("app_all_pl"), PAYLOAD_RUNGS[0]);
        assert_eq!(tint_of("app_pl"), PAYLOAD_RUNGS[1]);
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
            let hex = tint_of(t.spelling());
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
    /// is the left-hand end of both streams, so both of its terminals are on
    /// its right, and the ground is the right-hand end, so both of its are on
    /// its left. Checked at the view, which is what the markup draws from.
    #[test]
    fn a_node_is_drawn_with_the_terminals_its_type_has() {
        // Where a node may be, which is where the terminals' places are
        // worked out from: see `style::terminal_at`.
        let at = (pane::node_ceil(0), pane::node_ceil(0));
        let view = |node_type: &str| view_of("x", node_type, at, None, false, None);

        // A payload: what the ground sends arrives, and its readings leave.
        let sensor = view("i2c");
        assert!(sensor.moc_in.shown && sensor.payload_out.shown);
        assert!(!sensor.moc_out.shown && !sensor.payload_in.shown);
        // Both on its right, and the arrows point the way the data travels:
        // its own readings leave along the top going right, and what asks for
        // them arrives along the bottom going left.
        assert!(sensor.moc_in.x > sensor.width / 2.0);
        assert!(sensor.payload_out.x > sensor.width / 2.0);
        assert!(sensor.payload_out.y < sensor.moc_in.y);
        assert!(!sensor.moc_in.rightward && sensor.payload_out.rightward);

        // The ground: the other end of both, and both on its left.
        let moc = view("moc");
        assert!(moc.moc_out.shown && moc.payload_in.shown);
        assert!(!moc.moc_in.shown && !moc.payload_out.shown);
        assert!(moc.moc_out.x < moc.width / 2.0);
        assert!(moc.payload_in.x < moc.width / 2.0);
        assert!(moc.payload_in.y < moc.moc_out.y);

        // A page has all four: what is inside it may have any of them, and
        // nothing out here knows which -- so a line can be drawn to a page
        // either way on either stream.
        let page = view("subdiagram");
        for terminal in [
            &page.moc_in,
            &page.moc_out,
            &page.payload_in,
            &page.payload_out,
        ] {
            assert!(terminal.shown);
        }

        // A type this build does not know has none. The shape already says it
        // cannot be placed; four arrows on it would be four claims about a
        // type nothing here can describe -- and there is nothing to be gained
        // by letting a line be drawn to one.
        let unknown = view("contraption");
        assert!(!unknown.moc_in.shown && !unknown.payload_out.shown);

        // Every terminal is drawn inside its node, which is what makes the box
        // a person presses and the box a link arrives at the same box.
        for (node, terminal) in [
            (&sensor, &sensor.moc_in),
            (&sensor, &sensor.payload_out),
            (&moc, &moc.moc_out),
            (&moc, &moc.payload_in),
        ] {
            assert!(terminal.x >= 0.0 && terminal.y >= 0.0);
            assert!(terminal.x + style::TERMINAL.0 as f32 <= node.width);
            assert!(terminal.y + style::TERMINAL.1 as f32 <= node.height);
        }
    }

    /// No line can be mistaken for something being wrong.
    ///
    /// Red is reserved for faults, and a line is the one thing on the canvas
    /// drawn in a hue family's own colour at full strength across the whole
    /// sheet: a line in the fault's own corner of the colour space reads as a
    /// warning about the connection it draws. The payload family *is* the
    /// reddish one, so the line cannot leave the family -- it would stop
    /// saying which stream it carries -- and goes to the far side of it
    /// instead: the mauves, 53 units from the fault red where it used to be
    /// 27.
    ///
    /// Thirty-five is the floor, where ten is "clearly two colours": a line
    /// has to be unmistakable rather than merely distinguishable, because
    /// nobody compares it with a fault side by side. Asked under each of the
    /// four ways of seeing, since a dichromacy is exactly what collapses a red
    /// and a green into one another.
    ///
    /// The node bodies are *not* held to this. A faulty node is drawn in the
    /// fault's colour and in the one shape no real type uses, so what says it
    /// is wrong is the shape as much as the colour -- which is the rule that
    /// lets a dark green body exist at all. A line has no second channel.
    #[test]
    fn no_line_is_mistakable_for_a_fault() {
        use sight::{difference, seen, Sight};

        let painted = |hex: &str| {
            let c = colour(hex);
            (c.red(), c.green(), c.blue())
        };
        let fault = painted(ERROR_TINT);

        for (what, line) in [("payload", PAYLOAD_LINE), ("moc", MOC_LINE)] {
            for sight in Sight::ALL {
                let apart = difference(seen(painted(line), sight), seen(fault, sight));
                assert!(
                    apart >= 35.0,
                    "the {what} line is {apart:.1} from the fault red under \
                     {sight:?}, which is close enough to read as one"
                );
            }
        }
    }

    /// Each panel's nodes are a ramp from its own line, darkening down the
    /// panel in alphabetical order.
    ///
    /// The rule the body colours are derived by rather than chosen: a panel
    /// starts twelve CIE units below the line that carries its data -- the
    /// vehicle panel below both lines, having none of its own -- and steps
    /// darker for each node in turn. A reader finds a node in a panel by how
    /// dark it is, and a panel reads as one family.
    ///
    /// Twelve for the first step and not the eight the tiers ask, because the
    /// ground station's line and its bodies are the same hue: at one hue a
    /// step in lightness *is* the difference between two colours, and twelve
    /// is the floor this scheme holds.
    ///
    /// A panel with more nodes than rungs starts again at its first colour,
    /// so two nodes of one panel may share one -- which is allowed because
    /// colour is the redundant channel here. Nothing wraps today: the longest
    /// panel has ten nodes and ten rungs, and the test says so, so that the
    /// day an eleventh payload kind is added somebody sees the wrap happen
    /// rather than discovering two identical swatches.
    #[test]
    fn every_panel_is_a_ramp_from_its_own_line() {
        use sight::{contrast, difference, lightness, seen, Sight};

        let painted = |hex: &str| {
            let c = colour(hex);
            (c.red(), c.green(), c.blue())
        };
        let mark = painted(MARK);

        for (pane, rungs, from) in [
            (Pane::Moc, MOC_RUNGS, MOC_LINE),
            (Pane::Payloads, PAYLOAD_RUNGS, PAYLOAD_LINE),
            // The vehicle panel has no line of its own, so it starts below
            // the darker of the two that cross it.
            (Pane::Vehicle, VEHICLE_RUNGS, PAYLOAD_LINE),
        ] {
            let order = ramp_order(pane);
            assert!(
                !order.is_empty(),
                "the {} panel offers nothing",
                pane.label()
            );

            // Alphabetical, the rest and then the apps, which is the order
            // the ramp is walked in: see ramp_order.
            let mut sorted = order.clone();
            sorted.sort_by_key(|name| (name.starts_with("app_"), *name));
            assert_eq!(order, sorted, "the {} panel is out of order", pane.label());

            // The first rung is darker than the line, by the eight the tiers
            // ask -- and twelve *apart* from it, which is the rule that
            // actually binds and the reason the first step is bigger than the
            // ones after it. Lightness carries the whole of that difference
            // for the ground station, whose line and body are one hue; for
            // the others the hue helps, so their first rung needs less
            // lightness to make the twelve.
            let line = lightness(painted(from));
            let first = lightness(painted(rungs[0]));
            assert!(
                line - first >= 8.0,
                "the {} panel starts {:.1} below its line, and the tier is 8",
                pane.label(),
                line - first
            );
            for sight in Sight::ALL {
                let apart = difference(seen(painted(rungs[0]), sight), seen(painted(from), sight));
                assert!(
                    apart >= 12.0,
                    "the {} panel's first node is {apart:.1} from the line it \
                     comes below, under {sight:?}",
                    pane.label()
                );
            }

            // And each rung is darker than the one before, by enough to be
            // seen beside it: two and a half units, where two is "the same
            // colour in two printings".
            for pair in rungs.windows(2) {
                let (above, below) = (lightness(painted(pair[0])), lightness(painted(pair[1])));
                assert!(
                    above - below >= 2.5,
                    "two rungs of the {} panel are {:.1} apart in lightness",
                    pane.label(),
                    above - below
                );
            }

            // Every rung stays above the floor where the three families stop
            // being told apart, and every rung can be written on.
            for rung in rungs {
                let body = painted(rung);
                assert!(
                    lightness(body) >= 13.0,
                    "a rung of the {} panel is at {:.1} of lightness, below the \
                     floor where the families converge",
                    pane.label(),
                    lightness(body)
                );
                let legible = contrast(mark, body);
                assert!(
                    legible >= 4.5,
                    "a rung of the {} panel reads {legible:.2} against its own \
                     label",
                    pane.label()
                );
            }

            // The ramp is walked in order, and wraps where it runs out.
            for (at, name) in order.iter().enumerate() {
                assert_eq!(
                    tint_of(name),
                    rungs[at % rungs.len()],
                    "{name} should take rung {} of the {} panel",
                    at % rungs.len(),
                    pane.label()
                );
            }
            // Two of the three panels wrap, and that is said here
            // deliberately, which is what this assertion is for. The vehicle
            // has seven types since the three apps joined the computation, the
            // pass-through and the log, and four rungs; the payloads have
            // twelve since the two payloads that are programs joined the ten
            // devices, and ten rungs. The apps are the ones that wrap, taking
            // the top of the ramp again, so the types that were there first
            // keep the colours they had. Allowed because colour is the fourth
            // channel in this scheme and not the first -- every pair of types
            // differs in two things that are not colour, which
            // `style::tests::every_pair_of_types_differs_in_two_things_that_are_not_colour`
            // is the statement of -- so two nodes sharing a tint are still two
            // shapes nobody can confuse. The ramp is walked in order either
            // way, which the loop above is what checks.
            if !matches!(pane, Pane::Vehicle | Pane::Payloads) {
                assert!(
                    order.len() <= rungs.len(),
                    "the {} panel has {} nodes and {} rungs, so it wraps -- \
                     which is allowed, and this is where to say so \
                     deliberately",
                    pane.label(),
                    order.len(),
                    rungs.len()
                );
            }
        }
    }

    /// No two colours of the whole scheme look alike to anybody.
    ///
    /// Every pair of it: the three working panes, the two link lines and the
    /// fifteen node bodies, under normal vision and under each of the three
    /// dichromacies. Twenty colours rather than the fifteen this used to ask
    /// about, because a line is a colour a reader has to tell from the bodies
    /// at either end of it and from the sheet it is drawn on -- and two of
    /// them did not: the ground station's body was nine and a half from the
    /// line arriving at it.
    ///
    /// Checked against the colours the window is actually given rather than
    /// against the intention. Twenty colours in three hue families is well
    /// past what care at the keyboard would keep true: a tint nudged by hand
    /// would be a near-copy of another under one of the three dichromacies,
    /// and nobody with normal vision would see it.
    ///
    /// The floor is twelve CIE units, where two or three is "the same colour
    /// in two printings" and ten is "clearly two colours". So a pair anybody
    /// can tell apart is a pair everybody can tell apart, with something in
    /// hand.
    ///
    /// Twelve, where the bodies alone managed fourteen, and the two units are
    /// what the three tiers cost. The bodies now have to be darker than the
    /// lines as well as apart from each other -- see
    /// `the_palette_runs_from_the_pane_through_the_line_to_the_bodies` -- which
    /// takes the top off the range they had: six of them moved down to make
    /// room, and the closest pair that is left is an olive and a gold at 12.2.
    /// A failure here means the search has to be run again rather than a value
    /// nudged.
    #[test]
    fn no_two_colours_of_the_scheme_look_alike() {
        use sight::{difference, seen, Sight};

        let painted = |hex: &str| {
            let c = colour(hex);
            (c.red(), c.green(), c.blue())
        };

        // Everything on the canvas, each tagged with the panel whose ramp it
        // belongs to -- or none, for the panes and the lines, which belong to
        // no ramp.
        let mut scheme: Vec<(String, Option<Pane>, (u8, u8, u8))> = Vec::new();
        for pane in [Pane::Moc, Pane::Vehicle, Pane::Payloads] {
            scheme.push((
                format!("the {} pane", pane.label()),
                None,
                painted(&ground_of(style::ground_of(pane))),
            ));
        }
        scheme.push(("the payload line".to_string(), None, painted(PAYLOAD_LINE)));
        scheme.push(("the moc line".to_string(), None, painted(MOC_LINE)));
        for t in NodeType::ALL {
            scheme.push((
                t.spelling().to_string(),
                Some(pane_of(t.spelling())),
                painted(&tint_of(t.spelling())),
            ));
        }

        let mut closest = (f64::MAX, Sight::Normal, String::new(), String::new());
        for (i, (one, mine, a)) in scheme.iter().enumerate() {
            for (two, theirs, b) in &scheme[i + 1..] {
                // Two nodes of one panel are the exemption, and it is
                // arithmetic rather than a relaxation somebody chose: a
                // panel's bodies are a ramp down from its own line -- see
                // `tint_of` -- so neighbours in it are three or four units
                // apart by construction. Ten steps of twelve would want a
                // hundred and twenty units of lightness out of the
                // eighty-seven there are. What tells two nodes of one panel
                // apart is the shape, the edge, the border width, the size
                // and the label, two of which differ for every pair; what
                // their colour says is which panel they are in and how far
                // down it they sit.
                if mine.is_some() && mine == theirs {
                    continue;
                }
                for sight in Sight::ALL {
                    let apart = difference(seen(*a, sight), seen(*b, sight));
                    if apart < closest.0 {
                        closest = (apart, sight, one.clone(), two.clone());
                    }
                }
            }
        }

        let (apart, sight, a, b) = closest;
        assert!(
            apart >= 12.0,
            "{a} and {b} are only {apart:.1} apart to a reader with \
             {sight:?}, and the floor is 12"
        );
    }

    /// Every node is legible on its own pane, and is in that pane's family.
    ///
    /// Which is the other half of what a colour is for here. A node is drawn
    /// on a tinted ground rather than on white, so "is this colour usable" is
    /// a question about the pair: three to one is the threshold for something
    /// that is not text, and it is asked of the node against its own pane.
    ///
    /// And the hue says where the node belongs. A payload is a dark teal on
    /// pale teal, a vehicle node a green on pale green, the ground
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
            let node = painted(&tint_of(t.spelling()));
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
            // And in its own family's hue, which is declared rather than
            // read off the pane: a pane may be near enough neutral to have no
            // hue worth measuring against. See `family_hue`.
            let apart = hue_apart(hue(node), family_hue(pane));
            assert!(
                apart <= 40.0,
                "{}'s hue is {apart:.0} degrees from the {} family's",
                t.spelling(),
                pane.label()
            );
        }

        // The three families, far enough apart that a reader can tell which
        // one a colour belongs to. Sixty degrees: a hue family spans forty on
        // either side of its own centre, so anything less would let two
        // families overlap and a node's colour would stop saying which pane
        // it belongs in -- which is the whole of what the rule above is for.
        for (one, two) in [
            (Pane::Moc, Pane::Vehicle),
            (Pane::Moc, Pane::Payloads),
            (Pane::Vehicle, Pane::Payloads),
        ] {
            let apart = hue_apart(family_hue(one), family_hue(two));
            assert!(
                apart >= 60.0,
                "the {} and {} families are {apart:.0} degrees apart",
                one.label(),
                two.label()
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
                lightness(*ground) >= 82.0,
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
                        apart >= 14.0,
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
        // A square polygon, one whole line, and no second line: the shape
        // nothing real wears, drawn as plainly as it can be.
        assert!(!view.shape.is_empty() && !view.edge.is_empty());
        assert!(view.inner.is_empty());
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
            name: "temp_1".into(),
            node_type: "i2c".into(),
            home: None,
        });

        let (name, node_type, _, fields) = panel_of(&editor, &None, Some("temp_1"));
        assert_eq!(name, "temp_1");
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
            name: "temp_1".into(),
            node_type: "i2c".into(),
            home: None,
        });
        editor.edit(Edit::SetAttribute {
            node: "temp_1".into(),
            attribute: "path".into(),
            was: None,
            now: Some("/dev/i2c-1".into()),
        });

        let (_, _, _, fields) = panel_of(&editor, &None, Some("temp_1"));
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
            name: "temp_1".into(),
            node_type: "i2c".into(),
            home: None,
        });
        editor.edit(Edit::SetAttribute {
            node: "temp_1".into(),
            attribute: "future_thing".into(),
            was: None,
            now: Some("7".into()),
        });

        let (_, _, _, fields) = panel_of(&editor, &None, Some("temp_1"));
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
    ///
    /// A millimetre grid: 96/25.4 pixels a step, each point rounded to the
    /// nearest pixel -- 0, 4, 8, 11, 15, 19, 23, 26 and on -- and a node on
    /// every fifth of them, a millimetre short of each line its terminals are
    /// on: 4 mm, 9 mm, 14 mm, which are 15, 34 and 53 pixels.
    #[test]
    fn a_node_lands_on_the_grid() {
        for (dragged, expected) in [
            ((15, 15), (15, 15)),
            ((20, 30), (15, 34)),
            ((30, 40), (34, 34)),
            ((44, 50), (53, 53)),
        ] {
            assert_eq!(snap(dragged), expected, "{dragged:?}");
        }
        // And everything snapped is a place a node may be, and the nearest:
        // never more than half a step away.
        let half = (pane::NODE_STEP_MM as f64 * pane::PX_PER_MM / 2.0).ceil() as i32;
        for x in -40..400 {
            let at = snap((x, x)).0;
            assert!(pane::is_on_node_grid(at), "{x} snapped to {at}");
            assert!((at - x).abs() <= half, "{x} snapped to {at}");
        }
        // Every point is its own count of millimetres from the edge, to
        // within the half pixel it is rounded by, however far across the
        // sheet: a grid built by adding a rounded step would drift.
        for mm in 0..=635 {
            let at = pane::grid_point(mm);
            assert!(
                (at as f64 - mm as f64 * pane::PX_PER_MM).abs() <= 0.5,
                "{mm} mm is at {at}"
            );
            assert_eq!(pane::on_grid(at), at, "{mm} mm");
            assert_eq!(pane::grid_after(at), pane::grid_point(mm + 1), "{mm} mm");
            assert_eq!(pane::grid_before(at), pane::grid_point(mm - 1), "{mm} mm");
        }
        // The ground station a new configuration starts with is on it too.
        assert!(pane::is_on_node_grid(MOC_PLACE.0) && pane::is_on_node_grid(MOC_PLACE.1));
        assert!(MOC_PLACE.0 + style::WIDTH as i32 <= WORKING_WIDTH);
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
            name: "temp_1".into(),
            node_type: "i2c".into(),
            home: Some("cabin".into()),
        });

        let (top, _) = views_of(&editor, &None, None);
        let names: Vec<String> = top.iter().map(|n| n.name.to_string()).collect();
        assert_eq!(names, vec!["cabin".to_string(), "moc".to_string()]);

        let (inside, _) = views_of(&editor, &Some("cabin".into()), None);
        let names: Vec<String> = inside.iter().map(|n| n.name.to_string()).collect();
        assert_eq!(names, vec!["temp_1".to_string()]);

        // The closed box says what is in it, rather than drawing it.
        let box_ = top.iter().find(|n| n.name == "cabin").unwrap();
        assert!(box_.openable, "a subdiagram should be openable");
        assert!(
            box_.summary.contains("1 node"),
            "the closed box should say what is inside: {}",
            box_.summary
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
            name: "temp_1".into(),
            node_type: "i2c".into(),
            home: Some("cabin".into()),
        });
        editor.edit(Edit::AddLink {
            link: Link::new("cabin/temp_1", "moc", None),
        });

        // Seen from inside the subdiagram: one link, named for where it goes.
        let (_, inside) = views_of(&editor, &Some("cabin".into()), None);
        assert_eq!(inside.len(), 1);
        assert_eq!(inside[0].label, "moc");

        // And seen from the top diagram, where its source is off the page.
        let (_, top) = views_of(&editor, &None, None);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].label, "cabin/temp_1");
    }

    /// A name is the type, a hyphen and the first free number, and it is
    /// fresh within its own home.
    ///
    /// Two subdiagrams may each hold an i2c, so placing one in each gives
    /// both the first number rather than numbering the second one twice.
    #[test]
    fn a_fresh_name_is_numbered_from_one_and_fresh_where_it_is_placed() {
        let mut document = Document::new();
        assert_eq!(fresh_name(&document, "i2c", &None), "i2c-1");

        document.apply(&Edit::AddNode {
            name: "i2c-1".into(),
            node_type: "i2c".into(),
            home: None,
        });
        assert_eq!(fresh_name(&document, "i2c", &None), "i2c-2");

        // A gap is filled rather than stepped over: the number says which one
        // this is among the ones there are, not how many have ever been
        // placed.
        document.apply(&Edit::AddNode {
            name: "i2c-2".into(),
            node_type: "i2c".into(),
            home: None,
        });
        document.apply(&Edit::RemoveNode {
            name: "i2c-1".into(),
            node_type: "i2c".into(),
            home: None,
            attributes: Vec::new(),
            links: Vec::new(),
        });
        assert_eq!(fresh_name(&document, "i2c", &None), "i2c-1");

        // In a subdiagram, the first number is free again.
        let cabin = Some("cabin".to_string());
        assert_eq!(fresh_name(&document, "i2c", &cabin), "i2c-1");
    }

    /// A link's ends are placed on the terminals it is attached to.
    ///
    /// Which is the drawing the canvas is for: the readings leave a payload by
    /// its `payload-out`, in its upper right corner, and arrive at the
    /// computation by its `payload-in`, in that node's upper left -- so the
    /// line runs rightward along the top of both, the way the data goes and
    /// the way the panes are arranged. It used to leave the middle of the source's
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
            // The payload to the left of the computation, which is where
            // the panes put it: payloads are the leftmost working pane and
            // the vehicle's own graph is the middle one.
            Edit::MoveNode {
                node: "a".into(),
                was: (0, 0),
                now: (24, 24),
            },
            Edit::MoveNode {
                node: "b".into(),
                was: (0, 0),
                now: (400, 40),
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

        // It starts on the payload's own `payload-out` terminal, which is the
        // right edge of its upper right corner, and finishes on the
        // computation's `payload-in`, the left edge of its upper left
        // corner -- so the line touches both boxes it joins and crosses
        // neither shape.
        let ((start_x, start_y), leaves_right) =
            style::attachment("i2c", (24, 24), "payload-out").expect("a payload's source");
        assert!(leaves_right, "the readings leave towards the ground");
        assert!(
            view.route
                .starts_with(&format!("M {start_x:.1} {start_y:.1}")),
            "{}",
            view.route
        );

        let ((end_x, end_y), _) =
            style::attachment("compute", (400, 40), "payload-in").expect("a sink");
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
        // And the middle of a node, which is the other place a person lets go:
        // a line may be dropped anywhere on the node it is being joined to.
        let middle = |node: &str| {
            let (_, node_type, at) = placed
                .iter()
                .find(|(name, _, _)| name == node)
                .expect("the node");
            let (width, height) = size_of(node_type);
            (
                at.0 as f32 + width as f32 / 2.0,
                at.1 as f32 + height as f32 / 2.0,
            )
        };

        // The readings, from the payload to the ground: a payload-out let go of
        // over a payload-in. Neither end names its terminal, both types
        // having one of each -- which is what keeps a saved file saying
        // `{ from: sensor, to: ground }`.
        let link = joined(
            &document,
            &None,
            &drawn("sensor", "payload-out"),
            over("ground", "payload-in"),
        )
        .expect("a payload's readings reach the ground");
        assert_eq!(link.from, "sensor");
        // The terminal it leaves by, always: a link is written as an
        // attribute of its own node, named after that terminal.
        assert_eq!(link.origin.as_deref(), Some("payload-out"));
        assert_eq!(link.to, "ground");
        // And nothing at the arriving end, the ground link having one data
        // sink: the schema reads an unnamed end as that one.
        assert_eq!(link.input, None);

        // And back the other way, which is the other stream: a moc-out let
        // go of over a moc-in.
        let back = joined(
            &document,
            &None,
            &drawn("ground", "moc-out"),
            over("sensor", "moc-in"),
        )
        .expect("what the ground sends reaches the payload");
        assert_eq!((back.from.as_str(), back.to.as_str()), ("ground", "sensor"));

        // A null node has two of each, so its links say which: that is the
        // whole reason a terminal is written into a file.
        let through = joined(
            &document,
            &None,
            &drawn("through", "payload-out"),
            over("ground", "payload-in"),
        )
        .expect("a null node reaches the ground");
        assert_eq!(through.origin.as_deref(), Some("payload-out"));

        // Now the gestures that are not links.

        // Let go of over the far node's own data source, which is still a
        // release over that node: the line lands on the terminal the stream
        // names -- the ground's payload-in -- as it would anywhere else on
        // the box. What may be joined to what has not changed; what has gone
        // is having to hit a small box to say it.
        assert_eq!(
            joined(
                &document,
                &None,
                &drawn("sensor", "payload-out"),
                over("ground", "moc-out"),
            ),
            Some(link.clone()),
            "a release anywhere on the node makes the link the stream allows"
        );

        // And the middle of it, which is where a person actually aims.
        assert_eq!(
            joined(&document, &None, &drawn("sensor", "payload-out"), middle("ground")),
            Some(link.clone())
        );

        // A node with no terminal of that stream takes nothing: the ground
        // receives readings and is sent nothing, so what the ground sends has
        // nowhere to land on the ground.
        assert_eq!(
            joined(
                &document,
                &None,
                &drawn("through", "moc-out"),
                middle("ground"),
            ),
            None,
            "a moc node has no moc-in to arrive at"
        );

        // Let go of over nothing.
        assert_eq!(
            joined(&document, &None, &drawn("sensor", "payload-out"), (900.0, 400.0)),
            None
        );

        // Let go of over the node it started on, which is a gesture that
        // changed its mind rather than a request for a loop.
        assert_eq!(
            joined(
                &document,
                &None,
                &drawn("through", "payload-out"),
                over("through", "moc-in"),
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
            &drawn("ground", "payload-in"),
            over("sensor", "payload-out"),
        )
        .expect("a link drawn from the sink end");
        assert_eq!(backwards, link, "the same link, whichever end it was drawn from");

        // And from a data sink onto the *node*, which is the same gesture with
        // nothing to aim at: a sink is fed by one stream, so the terminal the
        // line leaves by is the payload's payload-out whether or not the hand
        // found the box.
        assert_eq!(
            joined(
                &document,
                &None,
                &drawn("ground", "payload-in"),
                middle("sensor"),
            ),
            Some(link.clone()),
            "a line from a data sink should reach a node dropped anywhere on it"
        );

        // Released over the null node's own payload-out box, which is a release
        // over the null node: the readings land on its payload-in, that
        // being where a groundward stream arrives.
        let through_sink = joined(
            &document,
            &None,
            &drawn("sensor", "payload-out"),
            over("through", "payload-out"),
        )
        .expect("the readings reach the node in the way");
        assert_eq!(
            (
                through_sink.to.as_str(),
                through_sink.input.as_deref(),
            ),
            ("through", Some("payload-in")),
            "a null node has two data sinks, so the file says which"
        );

        // And a link that is already there: a second line on top of the first
        // would be two deliveries of every datum.
        document.apply(&Edit::AddLink { link: link.clone() });
        assert_eq!(
            joined(
                &document,
                &None,
                &drawn("sensor", "payload-out"),
                over("ground", "payload-in"),
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
        let held = Link::joining("sensor", Some("payload-out"), "ground", None);
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
            terminal: "payload-in".to_string(),
            holding: Some(held.clone()),
        };

        // Put back where it came from: nothing happens. A gesture that has
        // thought better of itself leaves no trace.
        assert_eq!(
            dropped(&document, &None, &held, &pressed, over("ground", "payload-in")),
            None
        );

        // Or anywhere else on the node it is already attached to, which is
        // the same answer for the same reason: that is where it is.
        assert_eq!(dropped(&document, &None, &held, &pressed, middle("ground")), None);

        // Moved to a terminal that suits the end: one edit, so one undo puts
        // it back. The far end -- the sensor -- stays where it is.
        let moved = dropped(
            &document,
            &None,
            &held,
            &pressed,
            over("through", "payload-in"),
        )
        .expect("a sink that pairs with the source");
        match moved {
            Edit::MoveLink { was, now } => {
                assert_eq!(was, held);
                assert_eq!(now.from, "sensor", "the far end stays put");
                assert_eq!(now.origin.as_deref(), Some("payload-out"));
                assert_eq!(now.to, "through");
                assert_eq!(now.input.as_deref(), Some("payload-in"));
            }
            other => panic!("{other:?}"),
        }

        // Anywhere on that node is the same answer, which is what makes this
        // worth aiming loosely: the corner box of its other stream, the box
        // of one of its data sources, and the middle of it all put the end on
        // its payload-in, that being where a groundward stream arrives.
        for landing in [
            over("through", "moc-in"),
            over("through", "payload-out"),
            middle("through"),
        ] {
            assert_eq!(
                dropped(&document, &None, &held, &pressed, landing),
                Some(Edit::MoveLink {
                    was: held.clone(),
                    now: Link::joining("sensor", Some("payload-out"), "through", Some("payload-in")),
                }),
                "a release anywhere on the node moves the end to its payload-in"
            );
        }

        // Not onto the far node itself, which would be a link from a node to
        // itself.
        assert_eq!(
            dropped(&document, &None, &held, &pressed, over("sensor", "moc-in")),
            None
        );

        // And onto a node that cannot be this end but could be the other one,
        // the gesture is read as drawing a line rather than moving one: a
        // payload has no payload-in, so dropping the ground's arriving end
        // on one means "this payload reports here too". The ground's sink
        // takes as many as the configuration gives it, which is what makes
        // that a sentence and not a contradiction.
        assert_eq!(
            dropped(&document, &None, &held, &pressed, middle("other")),
            Some(Edit::AddLink {
                link: Link::joining("other", Some("payload-out"), "ground", None)
            }),
            "a second line into the ground should be drawn, not refused"
        );

        // Over nothing at all, from the end the line arrives at: ignored. A
        // sink takes one source, so letting go of its end over nothing is a
        // slip more often than a decision.
        assert_eq!(
            dropped(&document, &None, &held, &pressed, (40.0, 900.0)),
            None,
            "an arriving end dropped on the canvas should change nothing"
        );

        // And the source end moves the same way: taken hold of at the
        // sensor's `payload-out` and put on another sensor's.
        let pressed = Drawing {
            node: "sensor".to_string(),
            terminal: "payload-out".to_string(),
            holding: Some(held.clone()),
        };
        let moved = dropped(&document, &None, &held, &pressed, over("other", "payload-out"))
            .expect("another sensor's readings may reach the same ground");
        match moved {
            Edit::MoveLink { now, .. } => {
                assert_eq!(now.from, "other");
                assert_eq!(now.origin.as_deref(), Some("payload-out"));
                assert_eq!(now.to, "ground", "the far end stays put");
            }
            other => panic!("{other:?}"),
        }

        // And a move that would make a link the document already has does
        // nothing: two lines between one pair of terminals lie on top of each
        // other and deliver twice.
        document.apply(&Edit::AddLink {
            link: Link::joining("other", Some("payload-out"), "ground", None),
        });
        assert_eq!(
            dropped(&document, &None, &held, &pressed, over("other", "payload-out")),
            None
        );
    }

    /// A second line out of a data source is drawn from that source.
    ///
    /// The gesture somebody tried first and found dead: a moc node already
    /// feeding one payload, pressed on its `moc-out` and let go of over a
    /// second payload. That terminal has one line on it, so the press means
    /// "pick this line up" -- and a payload cannot be a data source, so there
    /// was nowhere for the end to land and nothing happened at all. What they
    /// meant was plain: that node, fed from here.
    ///
    /// So a landing that cannot be this end and could be the other one is read
    /// as a new line. Which is the only gesture there is for a second line out
    /// of a source: a source may feed several sinks, and the one it already
    /// feeds is what makes the press ambiguous in the first place.
    #[test]
    fn a_source_that_already_feeds_one_node_can_feed_another() {
        use journal::{Edit, Link};

        let mut document = Document::new();
        for (name, node_type, at) in [
            ("ground", "moc", (0, 0)),
            ("i2c", "i2c", (dividers().vehicle + 8, 0)),
            ("spi", "spi", (dividers().vehicle + 8, 152)),
        ] {
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
        }
        // The line that is already there, which is what made the gesture
        // ambiguous: the ground feeds one payload.
        let held = Link::joining("ground", Some("moc-out"), "i2c", None);
        document.apply(&Edit::AddLink { link: held.clone() });

        // Pressed on the ground's moc-out, which hands over the line it
        // has, and let go of in the middle of the other payload.
        let pressed = Drawing {
            node: "ground".to_string(),
            terminal: "moc-out".to_string(),
            holding: Some(held.clone()),
        };
        let (width, height) = size_of("spi");
        let at = document.node("spi").expect("the node").at;
        let over = (
            at.0 as f32 + width as f32 / 2.0,
            at.1 as f32 + height as f32 / 2.0,
        );

        assert_eq!(
            dropped(&document, &None, &held, &pressed, over),
            Some(Edit::AddLink {
                link: Link::joining("ground", Some("moc-out"), "spi", None)
            }),
            "the second line was not drawn"
        );

        // And the line that was picked up is still there: a gesture that drew
        // a line did not move one.
        assert!(document.links.contains(&held));
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
        assert!(attached(&document, &at("sensor", "payload-out"), false).is_empty());

        document.apply(&Edit::AddLink {
            link: Link::joining("sensor", Some("payload-out"), "ground", None),
        });
        assert_eq!(attached(&document, &at("sensor", "payload-out"), false).len(), 1);
        // And the sink end of the same link, from the other side.
        assert_eq!(
            attached(&document, &at("ground", "payload-in"), true).len(),
            1
        );

        // A second destination from the same source: now the source has two,
        // and nothing says which a drag meant.
        document.apply(&Edit::AddLink {
            link: Link::joining("sensor", Some("payload-out"), "housekeeping", None),
        });
        assert_eq!(attached(&document, &at("sensor", "payload-out"), false).len(), 2);

        // The ground's sink takes many, and the same is true of it: every
        // payload aboard reports to the one ground link.
        document.apply(&Edit::AddLink {
            link: Link::joining("other", Some("payload-out"), "ground", None),
        });
        assert_eq!(
            attached(&document, &at("ground", "payload-in"), true).len(),
            2
        );
    }

    /// A link to a terminal is recorded as the terminal that was pressed.
    ///
    /// The fault this is here for: a line dragged from the ground link's
    /// `moc-out` to the upper left corner of a computation -- its `moc-in`
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
                terminal: "moc-out".to_string(),
                holding: None,
            },
            over("to_kelvin", "compute", (300, 120), "moc-in"),
        )
        .expect("the ground reaches a computation");

        assert_eq!(link.from, "ground");
        assert_eq!(link.origin.as_deref(), Some("moc-out"));
        assert_eq!(link.to, "to_kelvin");
        // The terminal that was pressed, which is the whole of what the
        // gesture said: a computation has two data sinks, so the file has to
        // say which.
        assert_eq!(link.input.as_deref(), Some("moc-in"));

        // And the line is drawn to that corner: the upper left, where what
        // the ground sends arrives.
        document.apply(&Edit::AddLink { link });
        let ends = ends_of(&document, &document.links[0], &None);
        let ((x, y), _) =
            style::attachment("compute", (300, 120), "moc-in").expect("a terminal");
        assert_eq!(ends.to, (x, y), "the line arrives at the terminal it was drawn to");

        // A mean is the one destination whose arriving end is named by
        // something that is not a terminal, and it still fills its inputs.
        document.apply(&Edit::AddNode {
            name: "cabin_temp".into(),
            node_type: "compute".into(),
            home: None,
        });
        document.apply(&Edit::SetAttribute {
            node: "cabin_temp".into(),
            attribute: "inputs".into(),
            was: None,
            now: Some("in0,in1".into()),
        });
        assert!(declares_inputs(&document, "cabin_temp"));
        assert_eq!(
            arriving_at(&document, "cabin_temp", "payload-in").as_deref(),
            Some("in0"),
            "a mean is filled by name, not by terminal"
        );
    }

    /// A link the file does not name a sink for is drawn at the pairing one.
    ///
    /// A link leaving `moc-out` carries what the ground sends, so it
    /// arrives at a `moc-in`: the same stream, named at its other end. The
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
            link: Link::joining("ground", Some("moc-out"), "through", None),
        });

        let ends = ends_of(&document, &document.links[0], &None);
        let ((x, y), _) =
            style::attachment("null", (300, 120), "moc-in").expect("the sink");
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
                right - left >= width + 16,
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
        assert!(pane::is_on_grid(dividers.payloads));
        assert!(pane::is_on_grid(dividers.vehicle));
    }

    /// A node is held in the pane its type belongs to.
    ///
    /// Both directions, because a clamp written one-sided is a clamp that
    /// looks right until somebody drags the other way: a payload must not
    /// cross the divider rightwards and must not leave the working area
    /// leftwards.
    #[test]
    fn a_node_is_held_in_its_own_pane() {
        let width = style::WIDTH as i32;
        let d = dividers();
        let (payloads, vehicle) = (d.payloads, d.vehicle);
        // Every answer is a place a node may be as well as in the pane: the
        // edges are the places just inside them.
        let y = pane::on_node_grid(100);
        let payload_left = pane::node_ceil(0);
        let payload_right = pane::node_floor(payloads - width);

        // A payload: the leftmost pane, and nowhere right of its own divider.
        assert_eq!(confine("i2c", d, (0, 100)), (payload_left, y));
        assert_eq!(confine("i2c", d, (-40, 100)), (payload_left, y));
        assert_eq!(confine("i2c", d, (40, 100)), (pane::on_node_grid(40), y));
        assert_eq!(
            confine("i2c", d, (payload_right + 1, 100)),
            (payload_right, y)
        );
        assert_eq!(confine("i2c", d, (WORKING_WIDTH, 100)), (payload_right, y));
        assert!(payload_right + width <= payloads);

        // The ground link: left to the vehicle divider, right to the far edge.
        assert_eq!(confine("moc", d, (0, 100)), (pane::node_ceil(vehicle), y));
        assert_eq!(
            confine("moc", d, (vehicle - 1, 100)),
            (pane::node_ceil(vehicle), y)
        );
        assert_eq!(
            confine("moc", d, (WORKING_WIDTH, 100)),
            (pane::node_floor(WORKING_WIDTH - width), y)
        );

        // And the vehicle's own graph, between the two.
        assert_eq!(confine("compute", d, (0, 100)), (pane::node_ceil(payloads), y));
        assert_eq!(confine("compute", d, (-40, 100)), (pane::node_ceil(payloads), y));
        assert_eq!(
            confine("compute", d, (WORKING_WIDTH, 100)),
            (pane::node_floor(vehicle - width), y)
        );

        // A node is never dragged off the top, whichever pane it is in: a
        // negative y is a node nobody can reach to drag back.
        for node_type in ["i2c", "compute", "moc"] {
            assert_eq!(
                confine(node_type, d, (vehicle, -80)).1,
                pane::node_ceil(0),
                "{node_type}"
            );
        }

        // A type this build does not know goes in the vehicle pane, which is
        // where something that needs looking at should be.
        assert!(!is_payload("contraption"));
        let (left, right) = pane::pane_for("contraption", d);
        assert_eq!(left, pane::node_ceil(payloads));
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
                now: (d.payloads + 24, at.1),
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
            name: "temp_1".into(),
            node_type: "i2c".into(),
            home: None,
        });
        document.apply(&Edit::MoveNode {
            node: "temp_1".into(),
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
    /// payload line keeps the payload pane and the vehicle's open, and the
    /// vehicle line keeps the vehicle's and the moc pane open. A pane with nothing
    /// visible in it is a pane whose nodes cannot be reached, and the drag
    /// that does it is one press away.
    #[test]
    fn neither_divider_can_close_a_pane() {
        let document = Document::new();
        let least = WORKING_WIDTH * settings::LEAST / 100;

        for (which, dividers) in [
            (Pane::Payloads, dividers()),
            (Pane::Vehicle, dividers()),
            // And from the ends of their travel, where an off-by-one would
            // show: a line already against what stops it.
            (
                Pane::Payloads,
                Dividers {
                    payloads: least,
                    vehicle: least * 2,
                },
            ),
            (
                Pane::Vehicle,
                Dividers {
                    payloads: least,
                    vehicle: WORKING_WIDTH - least,
                },
            ),
        ] {
            let (low, high) = divider_range(&document, dividers, which);
            for at in [low, high] {
                let settled = if which == Pane::Payloads {
                    Dividers {
                        payloads: at,
                        ..dividers
                    }
                } else {
                    Dividers {
                        vehicle: at,
                        ..dividers
                    }
                };
                for pane in [Pane::Payloads, Pane::Vehicle, Pane::Moc] {
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

    /// A node's window offers each attribute the control its kind takes.
    ///
    /// The kind comes from the description tree and reaches the window on the
    /// field, which is what decides whether the row is a box to write in or a
    /// control with two states: see `FieldView::kind` in the markup. So this
    /// asks what the window is given -- an app node's `restart` is a true or
    /// false, its `executable` a path, and its argument text -- and that the
    /// markup offers a two-state control for the one of those that is a true
    /// or false.
    #[test]
    fn a_fields_kind_reaches_the_window() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.yaml");
        std::fs::write(
            &path,
            "payloads:\n  - name: bay\n    type: app_pl\n    \
             executable: /bin/true\n    app-payload-arg: \"--bay\"\n",
        )
        .expect("a payload that is a program");
        let editor = Editor::open(&path, dividers());

        let (_, _, _, fields) = panel_of(&editor, &None, Some("bay"));
        let kind = |name: &str| {
            fields
                .iter()
                .find(|field| field.name == name)
                .map(|field| field.kind.to_string())
                .unwrap_or_else(|| panic!("the window offers {name}"))
        };
        assert_eq!(kind("restart"), "boolean");
        assert_eq!(kind("executable"), "path");
        assert_eq!(kind("app-payload-arg"), "text");

        // And what a boolean is offered with. The markup is read because the
        // control is in it: a kind that reached the window and chose the same
        // box as everything else would be a kind that did nothing.
        let markup = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/main.slint"),
        )
        .expect("the markup this binary was built from");
        let tightened: String = markup
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ");
        assert!(
            tightened.contains("if !f.is-heading && f.kind == \"boolean\": CheckBox {"),
            "a boolean field is not offered with a control of two states"
        );
        assert!(
            tightened.contains("if !f.is-heading && f.kind != \"boolean\": LineEdit {"),
            "the other kinds are not offered with a box to write in"
        );
    }

    /// An inert main window has something over it that takes the presses.
    ///
    /// `inert` is set from Rust and what it does is in the markup, so this
    /// reads the markup: an area over the whole window, and the last thing in
    /// it, which is what puts it over the surface and over the menus. A menu
    /// is a way of editing too, and a window that stopped presses reaching
    /// the canvas but not the menus would be inert in the half nobody would
    /// test by hand.
    #[test]
    fn an_inert_main_window_has_an_area_over_it() {
        let markup = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/main.slint"),
        )
        .expect("the markup this binary was built from");

        let window = markup
            .split("export component MainWindow inherits Window {")
            .nth(1)
            .expect("the markup has a main window");
        let body = window
            .split("\n}\n")
            .next()
            .expect("the main window has a body");

        let overlay = body
            .find("if root.inert: TouchArea {")
            .expect("an inert main window has an area over it");
        let surface = body
            .find("surface := Surface {")
            .expect("the main window has its surface");
        assert!(
            overlay > surface,
            "the area that takes the presses is written before the surface, \
             so the surface is over it and takes them instead"
        );
        for said in ["width: 100%;", "height: 100%;"] {
            assert!(
                body[overlay..].contains(said),
                "the area over an inert window does not say {said:?}, so it \
                 covers only part of it"
            );
        }
    }

    /// Nothing the editor places itself covers a pane's own name.
    ///
    /// Each pane has a band at its top with its name in it -- "MOC",
    /// "Vehicle", "Payloads" -- painted in that pane's own tint, and the band
    /// is what a pane has to say when it is empty or when somebody has lost
    /// their bearings. A node somebody *drags* there is their own business;
    /// one the editor put there would be the editor covering a name nobody
    /// asked it to cover.
    ///
    /// The bands are written down in `pane::NAMES` and drawn in `main.slint`,
    /// so this reads the markup and holds the two against each other: a band
    /// made deeper in the window without this would leave the row reserved
    /// shallower than the row that is drawn, and a node would be placed over
    /// a name.
    #[test]
    fn nothing_placed_by_the_editor_covers_a_panes_name() {
        let markup = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/main.slint"),
        )
        .expect("the markup this binary was built from");
        // The same markup with its line breaks and indentation taken out, so
        // that what is asked of it is what it says rather than how it is laid
        // out.
        let tightened: String = markup
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ");

        // The three names, each at the same height inside its own band.
        for name in ["\"MOC\"", "\"Vehicle\"", "\"Payloads\""] {
            let at = markup
                .split(&format!("text: {name};"))
                .next()
                .expect("the markup names the panes");
            let said = at.rsplit("y:").next().expect("a y for the name");
            assert!(
                said.trim_start().starts_with("Space.tight"),
                "the {name} label is no longer at Space.tight, so pane::NAMES \
                 is out of date"
            );
        }

        // One band per pane, each starting at the top of its pane, all of one
        // depth, and each painted in its own pane's tint. Three of them: a
        // band belongs to the pane whose name is in it.
        for (ground, width) in [
            ("payload-ground", "root.payload-divider;"),
            ("vehicle-ground", "root.vehicle-divider - root.payload-divider;"),
            ("moc-ground", "root.working-width - root.vehicle-divider;"),
        ] {
            let band = format!(
                "y: 0; width: {width} height: canvas.heading-tall; \
                 background: root.{ground};"
            );
            assert!(
                tightened.contains(&band),
                "the band painted root.{ground} is not a pane-wide row at the \
                 top of its pane, so pane::NAMES is out of date"
            );
        }

        // And its depth is asked of the labels rather than chosen, which is
        // what `NAMES` is the generous answer to.
        assert!(
            tightened.contains(
                "property <length> heading-tall: max(moc-heading.preferred-height, \
                 max(vehicle-heading.preferred-height, \
                 payload-heading.preferred-height)) + 2 * Space.tight;"
            ),
            "the bands' depth is no longer the tallest label plus Space.tight \
             above and below, so pane::NAMES is out of date"
        );
        for said in [
            "out property <length> tight: 4px;",
            "out property <length> small: 10px;",
        ] {
            assert!(
                markup.contains(said),
                "the markup no longer says {said:?}, so pane::NAMES is out of \
                 date"
            );
        }

        let (top, height) = pane::NAMES;
        assert_eq!(top, 0, "a band is drawn, so it starts at the pane's top");
        assert!(
            height >= 13 + 2 * 4,
            "a ten-pixel line wants thirteen pixels of box, with four above it \
             and four below, and the band is {height}"
        );

        // On an empty canvas, scrolled to the top of the sheet, every type
        // goes below its pane's band rather than over it.
        for kind in NodeType::ALL {
            let at = pane::place_in(&[], kind.spelling(), dividers(), (0, 700));
            assert!(
                at.1 >= top + height,
                "{} was placed at {at:?}, which covers the pane names at \
                 {top}..{}",
                kind.spelling(),
                top + height
            );
        }

        // And so does every node of an arrangement, which lays the whole
        // canvas out at once.
        let where_ = tempfile::tempdir().expect("a directory");
        let mut editor = Editor::open(&where_.path().join("vehicle.yaml"), dividers());
        for (name, kind) in [("a", "i2c"), ("b", "i2c"), ("c", "compute"), ("d", "moc")] {
            editor.edit(Edit::AddNode {
                name: name.to_string(),
                node_type: kind.to_string(),
                home: None,
            });
        }
        editor.arrange(dividers());
        for node in &editor.document().nodes {
            assert!(
                node.at.1 >= top + height,
                "{} was arranged at {:?}, which covers the pane names",
                node.name,
                node.at
            );
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
            (left, pane::grid_ceil(600)),
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
                x = pane::grid_after(x);
            }
            y = pane::grid_after(y);
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
                x = pane::grid_after(x);
            }
            y = pane::grid_after(y);
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
                assert!(pane::is_on_grid(dividers.payloads), "{vehicle}% is off the grid");
                assert!(pane::is_on_grid(dividers.vehicle), "{moc}% is off the grid");
                // And a percentage means what it says: that much of the
                // working pane, within the grid step it is snapped to, counted
                // from the right where the moc pane is.
                assert!(
                    (dividers.vehicle - WORKING_WIDTH * (100 - moc) / 100).abs() <= 2,
                    "a moc pane of {moc}% left the divider at {} of {WORKING_WIDTH}",
                    dividers.vehicle
                );
                assert!(
                    (dividers.payloads - WORKING_WIDTH * (100 - moc - vehicle) / 100).abs()
                        <= 2,
                    "a vehicle pane of {vehicle}% left the divider at {} of \
                     {WORKING_WIDTH}",
                    dividers.payloads
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

        let (low, high) = divider_range(&document, d, Pane::Payloads);
        assert_eq!(low, step, "the payload pane keeps a twentieth");
        assert_eq!(high, d.vehicle - step, "and so does the vehicle's");

        let (low, high) = divider_range(&document, d, Pane::Vehicle);
        assert_eq!(low, d.payloads + step);
        assert_eq!(
            high,
            WORKING_WIDTH - step,
            "the moc pane keeps a twentieth"
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

        // A computation in the vehicle pane and the ground link in the moc
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
            name: "ground".into(),
            node_type: "moc".into(),
            home: None,
        });
        document.apply(&Edit::MoveNode {
            node: "ground".into(),
            was: (0, 0),
            now: (d.vehicle + 48, 40),
        });

        let (low, high) = divider_range(&document, d, Pane::Vehicle);
        assert_eq!(low, 400 + width, "the computation's right edge stops it");
        assert_eq!(high, d.vehicle + 48, "the ground link's left edge stops it");
        assert!(low <= d.vehicle && d.vehicle <= high, "{low}..{high}");

        // A node on the wrong side of the divider holds nothing. Otherwise one
        // such node -- read from a file that put it there -- would freeze the
        // control, and the way to free it would be to drag the node, which is
        // the thing somebody would be using the divider to make room for.
        document.apply(&Edit::AddNode {
            name: "stray".into(),
            node_type: "moc".into(),
            home: None,
        });
        document.apply(&Edit::MoveNode {
            node: "stray".into(),
            was: (0, 0),
            now: (d.payloads + 8, 200),
        });
        assert_eq!(
            divider_range(&document, d, Pane::Vehicle),
            (low, high),
            "a ground link left of the divider should not pin it"
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
        assert_eq!(said.get("beacon.interval").map(String::as_str), Some("1 s"));
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
        assert!(written.contains("interval: \"1 s\""), "{written}");
        for unanswered in ["tcs_cmd", "payload_telemetry", "log"] {
            assert!(
                !written.contains(unanswered),
                "{unanswered} is not stated and should not be written: {written}"
            );
        }

        let moc = editor.document().node("moc-1").expect("a ground station");
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
            editor.document().node("moc-1").is_none(),
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
        assert!(editor.document().node("moc-1").is_none());

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
            editor.document().node("moc-1").is_none(),
            "a configuration somebody emptied was refilled"
        );

        // And one that could not be read, which is a file to repair rather
        // than a file to write over.
        let damaged = where_.path().join("c.yaml");
        std::fs::write(&damaged, "nodes: [[[\n").expect("a file");
        let mut editor = Editor::open(&damaged, dividers());
        assert!(editor.trouble().is_some(), "a damaged file should be reported");
        seed(&mut editor);
        assert!(editor.document().node("moc-1").is_none());
        assert_eq!(
            std::fs::read_to_string(&damaged).unwrap(),
            "nodes: [[[\n",
            "a damaged configuration was written over"
        );
    }

    /// No line in the worked example is drawn through a node.
    ///
    /// The rules are checked in `route.rs` against boxes and numbers; this
    /// asks the same question of a real configuration, which is where a
    /// routing fault was actually found. The return link of the example used
    /// to run its long leg straight through the box of the node it was
    /// arriving at.
    ///
    /// The thermostat, because `example.*` ships with no lines at all: it is
    /// the file the editor opens by default and the one somebody draws their
    /// own graph in, and the worked example is where the shipped lines live.
    #[test]
    fn no_line_of_the_worked_example_is_drawn_through_a_node_or_along_another() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

        // The worked example, which is the shipped configuration with lines in
        // it: `example.*` is the one somebody opens the editor on and draws
        // their own, and it ships with none.
        for extension in ["yaml", "xml"] {
            let editor = Editor::open(
                &root.join(format!("tests/manual/thermostat.{extension}")),
                dividers(),
            );
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
            assert!(!obstacles.is_empty(), "thermostat.{extension} has no nodes");
            assert!(!document.links.is_empty(), "and no links");

            // Every route, laid in the file's order as the canvas lays them,
            // so that each is checked against the ones already on it.
            let mut taken: Vec<Vec<(f32, f32)>> = Vec::new();
            // How much of the drawing is one line lying along another, and
            // how much it would be if each route were laid in ignorance of
            // the rest -- which is what the router did before the lanes.
            let (mut along, mut blind) = (0u32, 0u32);
            for link in &document.links {
                // The same ends the canvas draws, which is the point of
                // asking here: see ends_of.
                let ends = ends_of(document, link, &None);
                let through = route::runs_through(ends, &obstacles);
                assert_eq!(
                    through, 0,
                    "thermostat.{extension}: the line from {} to {} runs \
                     through {through} node(s)",
                    link.from, link.to
                );

                // And no two lines run along one track. Crossing is fine --
                // a crossing is a point, and a reader follows each line
                // through it -- but a shared run is one line hiding another,
                // and the hidden one reads as a link that is not there.
                //
                // Except where the two are joined to the same node, which is
                // the one case nothing can route around: two lines arriving
                // at one terminal arrive at one point, so they approach it
                // together however they got there -- and that reads as lines
                // meeting rather than as a line gone missing.
                let points = route::lay(ends, &obstacles, &taken);
                // Joined is asked of the lines rather than of the names: two
                // routes that start or end at the same point are two lines
                // meeting at one terminal, whatever their links are called.
                let meets = |one: &[(f32, f32)], two: &[(f32, f32)]| {
                    let ends = |p: &[(f32, f32)]| {
                        [p.first().copied(), p.last().copied()]
                    };
                    ends(one).iter().flatten().any(|a| {
                        ends(two)
                            .iter()
                            .flatten()
                            .any(|b| (a.0 - b.0).abs() < 1.0 && (a.1 - b.1).abs() < 1.0)
                    })
                };
                let apart: Vec<Vec<(f32, f32)>> = taken
                    .iter()
                    .filter(|theirs| !meets(&points, theirs))
                    .cloned()
                    .collect();
                along += route::shared(&points, &apart);
                // What the same link would have cost before the lanes: the
                // route the router picks when it is told about nothing.
                blind += route::shared(
                    &route::lay(ends, &obstacles, &[]),
                    &apart,
                );
                taken.push(points);
            }

            // A budget rather than nothing, and the budget is what the
            // router actually reaches on this drawing: twenty-two
            // lane-widths, against the hundred and seven the same routes
            // share when each is laid in ignorance of the others. Four fifths
            // of it gone, and what is left is the cases with no lane free --
            // a column with nodes either side of it, where every lane the
            // shapes offer is taken or blocked.
            //
            // Stated as a number for two reasons: a change that brings
            // wholesale overlap back fails here, and the day somebody teaches
            // the router to clear the rest, this number comes down rather
            // than the rule being quietly satisfied by a drawing that got
            // simpler.
            assert!(
                along <= 22,
                "thermostat.{extension}: {along} lane-widths of line lie along \
                 another line, and the budget is 22"
            );
            // And laying each route against the others has to beat laying
            // each in ignorance of them, which is the whole claim.
            assert!(
                blind > along * 2,
                "thermostat.{extension}: laying the routes against each other \
                 should more than halve the overlap, and it was {along} \
                 against {blind}"
            );
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
        a_fault_is_put_where_it_cannot_be_missed();
        the_menu_arranges_the_nodes_and_shows_them();
        the_line_being_drawn_follows_the_pointer();
        deleting_a_node_asks_first();
        the_main_window_is_inert_until_what_came_up_is_read();
        closing_the_question_ignores_the_rest();
        the_question_offers_a_way_out_of_the_editor();
        closing_the_main_window_closes_what_says_what_is_wrong();
        an_emptied_field_takes_the_attribute_off();
    }

    /// A field somebody clears takes the attribute off, and the file loses
    /// the line.
    ///
    /// Absent and empty are not the same thing to a configuration: an
    /// `app-payload-arg: ""` is a required argument stated as nothing, which
    /// the program is then run with -- and `lorem_pl.py`, run with an empty
    /// one, refuses it. An absent one is the file a person would have written.
    /// So an empty box removes rather than empties, and the writer leaves out
    /// what it is given empty.
    ///
    /// Written because a shipped example turned up with one stated as nothing
    /// and this window was the suspect. It is not: the path a cleared box
    /// takes is the one below.
    fn an_emptied_field_takes_the_attribute_off() {
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.yaml");
        std::fs::write(
            &path,
            "payloads:\n  - name: bay\n    type: app_pl\n    \
             executable: /bin/true\n    app-payload-arg: \"1\"\n",
        )
        .expect("a payload that is a program");
        let editor = Rc::new(RefCell::new(Editor::open(&path, dividers())));
        let ui = MainWindow::new().expect("a window");
        let views = views_for(&ui);

        show_node(&editor, &views, "bay");
        let window = views
            .borrow()
            .attributes
            .get("bay")
            .map(|window| window.as_weak())
            .expect("the node's window")
            .upgrade()
            .expect("and still there");

        // What it says now, so that clearing it is a change and not a no-op.
        assert_eq!(
            editor
                .borrow()
                .document()
                .node("bay")
                .expect("the node")
                .attributes
                .get("app-payload-arg")
                .map(String::as_str),
            Some("1")
        );

        window.invoke_attribute_set("app-payload-arg".into(), "".into());

        assert_eq!(
            editor
                .borrow()
                .document()
                .node("bay")
                .expect("the node")
                .attributes
                .get("app-payload-arg"),
            None,
            "a cleared field left the attribute behind as nothing"
        );
        let written = std::fs::read_to_string(&path).expect("the file");
        assert!(
            !written.contains("app-payload-arg"),
            "the file kept a line for an attribute nobody answered: {written}"
        );

        // And one written empty by hand goes the same way, rather than being
        // kept as a value: this is the other road to the same file.
        std::fs::write(
            &path,
            "payloads:\n  - name: bay\n    type: app_pl\n    \
             executable: /bin/true\n    app-payload-arg: \"\"\n",
        )
        .expect("a payload whose argument is stated as nothing");
        let mut reopened = Editor::open(&path, dividers());
        reopened.edit(Edit::MoveNode {
            node: "bay".to_string(),
            was: (0, 0),
            now: (40, 40),
        });
        let written = std::fs::read_to_string(&path).expect("the file");
        assert!(
            !written.contains("app-payload-arg"),
            "an attribute stated as nothing survived a save: {written}"
        );
    }

    /// What the editor holds about its windows, for a test that drives one.
    ///
    /// The same structure `main` builds, with nothing open: the two tests
    /// about what comes up at startup both want it, and a second copy of
    /// twenty fields is a second place to add the twenty-first.
    fn views_for(ui: &MainWindow) -> Rc<RefCell<Views>> {
        Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            drawing: Rc::new(RefCell::new(None)),
            divider: Rc::new(std::cell::Cell::new(dividers())),
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }))
    }

    /// The main window is there from the start and takes no presses until the
    /// problems the file arrived with have been read.
    ///
    /// Shown beside the question rather than held back, because the drawing is
    /// what the question is about -- and inert while it is up, so nothing is
    /// edited behind a window asking about the state the file arrived in.
    /// Answering it, however it is answered, gives the presses back.
    fn the_main_window_is_inert_until_what_came_up_is_read() {
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.yaml");
        std::fs::write(
            &path,
            "payload_groups:\n  - name: idle\n    interval: 3 s\n",
        )
        .expect("a configuration with a group nothing is in");
        let editor = Rc::new(RefCell::new(Editor::open(&path, dividers())));

        let ui = MainWindow::new().expect("a window");
        let views = views_for(&ui);
        // The order main() goes in: the window first, then what came up over
        // it.
        ui.show().expect("the window shows");
        alerting(&views);
        settled(&editor, &views);
        views.borrow().refresh(&editor.borrow());
        settle_problems(&editor, &views);
        ui.set_inert(startup_waiting(&views, None));

        assert!(
            startup_waiting(&views, None),
            "the question about the empty group should be up"
        );
        assert!(ui.window().is_visible(), "the main window is up beside it");
        assert!(ui.get_inert(), "and takes no presses while it is");

        let question = views
            .borrow()
            .names
            .as_ref()
            .map(|window| window.as_weak())
            .and_then(|window| window.upgrade())
            .expect("the question");
        question.invoke_left();

        assert!(views.borrow().names.is_none(), "the question has gone");
        assert!(ui.window().is_visible(), "the main window is still up");
        assert!(!ui.get_inert(), "and takes presses again");
        let _ = ui.hide();
    }

    /// Closing the question with its own close box ignores the rest of them.
    ///
    /// What the window manager's close box means everywhere else: the question
    /// has been answered and will not be asked again. The button that says so
    /// is "Ignore them all", and this is that button -- so the two are held
    /// against each other rather than against a list of what each does, and a
    /// change to what the button does is a change to the close box.
    fn closing_the_question_ignores_the_rest() {
        use std::rc::Rc;

        // What the editor is left holding after the question is answered this
        // way: which faults are being ignored, and whether the window has
        // gone and the main window taken its presses back.
        let answered = |how: &dyn Fn(&ProblemsWindow)| {
            let where_ = tempfile::tempdir().expect("a directory");
            let path = where_.path().join("vehicle.yaml");
            std::fs::write(
                &path,
                "payload_groups:\n  - name: idle\n    interval: 3 s\n",
            )
            .expect("a configuration with a group nothing is in");
            let editor = Rc::new(RefCell::new(Editor::open(&path, dividers())));

            let ui = MainWindow::new().expect("a window");
            let views = views_for(&ui);
            ui.show().expect("the window shows");
            alerting(&views);
            settled(&editor, &views);
            views.borrow().refresh(&editor.borrow());
            settle_problems(&editor, &views);
            ui.set_inert(startup_waiting(&views, None));

            let question = views
                .borrow()
                .names
                .as_ref()
                .map(|window| window.as_weak())
                .and_then(|window| window.upgrade())
                .expect("the question");
            how(&question);

            let left = (
                views.borrow().faulted.borrow().clone(),
                views.borrow().names.is_none(),
                ui.get_inert(),
            );
            let _ = ui.hide();
            left
        };

        let by_button = answered(&|question| question.invoke_ignored());
        let by_close_box = answered(&|question| {
            question
                .window()
                .dispatch_event(slint::platform::WindowEvent::CloseRequested)
        });

        assert_eq!(
            by_button, by_close_box,
            "closing the question should leave what the button that ignores \
             them all leaves"
        );
        // And what that is, so this says something whichever way they agree:
        // the question gone, the faults ignored, and the drawing taking
        // presses again.
        let (ignored, gone, inert) = by_close_box;
        assert!(!ignored.is_empty(), "the faults are being ignored");
        assert!(gone, "the question has gone");
        assert!(!inert, "and the main window takes presses again");
    }

    /// The question offers a way out of the editor.
    ///
    /// A file that arrived in a state somebody would rather put right
    /// elsewhere is a reason to leave, and a question about it should not be
    /// a reason to go looking for how. The same path the menu's Exit takes:
    /// with nothing unwritten it goes straight out, which is what this says
    /// by there being no offer to save.
    fn the_question_offers_a_way_out_of_the_editor() {
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.yaml");
        std::fs::write(
            &path,
            "payload_groups:\n  - name: idle\n    interval: 3 s\n",
        )
        .expect("a configuration with a group nothing is in");
        let editor = Rc::new(RefCell::new(Editor::open(&path, dividers())));

        let ui = MainWindow::new().expect("a window");
        let views = views_for(&ui);
        ui.show().expect("the window shows");
        alerting(&views);
        settled(&editor, &views);
        views.borrow().refresh(&editor.borrow());
        settle_problems(&editor, &views);

        let question = views
            .borrow()
            .names
            .as_ref()
            .map(|window| window.as_weak())
            .and_then(|window| window.upgrade())
            .expect("the question");
        assert!(
            !editor.borrow().unsaved(),
            "every edit is written as it is made, so a file just opened has \
             nothing unwritten"
        );

        question.invoke_quit();

        assert!(
            views.borrow().leaving.is_none(),
            "with nothing to write it leaves rather than offering to save"
        );
        let _ = ui.hide();
    }

    /// Closing the main window closes the windows that say what is wrong.
    ///
    /// All three of them: the question about the state the file arrived in,
    /// the alert about something that failed, and the list of concerns. Each
    /// is about the drawing, so none of them has anything to say once the
    /// drawing has gone -- and the toolkit stops when its last window closes,
    /// so one left up is an editor still running with nothing to edit in.
    fn closing_the_main_window_closes_what_says_what_is_wrong() {
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.yaml");
        std::fs::write(
            &path,
            "payload_groups:\n  - name: idle\n    interval: 3 s\n",
        )
        .expect("a configuration with a group nothing is in");
        let editor = Rc::new(RefCell::new(Editor::open(&path, dividers())));

        let ui = MainWindow::new().expect("a window");
        let views = views_for(&ui);
        closing(&ui, &views);
        ui.show().expect("the window shows");
        alerting(&views);
        settled(&editor, &views);
        views.borrow().refresh(&editor.borrow());
        settle_problems(&editor, &views);

        // All three up: the question came up with the file, the alert is told
        // to say something of its own, and the list is opened as the bar's
        // count opens it.
        views
            .borrow()
            .say("Something did not work.", "a device that is full", "");
        show_concerns(&editor, &views, false);
        assert!(views.borrow().names.is_some(), "the question is up");
        assert!(views.borrow().alert.is_some(), "the alert is up");
        assert!(views.borrow().concerns.is_some(), "the list is up");

        ui.window()
            .dispatch_event(slint::platform::WindowEvent::CloseRequested);

        assert!(views.borrow().names.is_none(), "the question has gone");
        assert!(views.borrow().alert.is_none(), "the alert has gone");
        assert!(views.borrow().concerns.is_none(), "the list has gone");
        let _ = ui.hide();
    }

    /// Delete asks, and only takes the node away if it is answered.
    ///
    /// The menu's second item, and the one a press meant for Edit is next to.
    /// A node is not one word -- it is its attributes and every line that
    /// touches it -- so the question names it and says what goes with it, and
    /// Cancel leaves the document exactly as it was, history included.
    fn deleting_a_node_asks_first() {
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Rc::new(RefCell::new(Editor::open(
            &where_.path().join("vehicle.yaml"),
            dividers(),
        )));
        for (name, node_type) in [("ground", "moc"), ("sensor", "i2c")] {
            editor.borrow_mut().edit(Edit::AddNode {
                name: name.to_string(),
                node_type: node_type.to_string(),
                home: None,
            });
        }
        editor.borrow_mut().edit(Edit::AddLink {
            link: journal::Link::joining("sensor", Some("payload-out"), "ground", None),
        });

        let ui = MainWindow::new().expect("a window");
        furnish!(&ui, descriptions());
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            drawing: Rc::new(RefCell::new(None)),
            divider: Rc::new(std::cell::Cell::new(dividers())),
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));
        let chosen: Rc<RefCell<BTreeMap<String, String>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let drawing: Rc<RefCell<Option<Drawing>>> = Rc::new(RefCell::new(None));
        wire(&ui, &editor, &views, &drawing, &chosen, &None);
        views.borrow().refresh(&editor.borrow());

        assert!(views.borrow().deleting.is_none(), "nothing asked yet");
        let depth = editor.borrow().undo_depth();

        // Asked about, and the question says which node and what goes with
        // it: one line is attached to this one.
        ui.invoke_node_delete("sensor".into());
        let question = views
            .borrow()
            .deleting
            .as_ref()
            .map(|window| window.as_weak())
            .expect("the question should be asked")
            .upgrade()
            .expect("and left open");
        assert_eq!(question.get_node(), "sensor");
        assert!(question.get_carries().contains("One line"), "{}", question.get_carries());

        // Cancelled: the node is still there, the line with it, and nothing
        // is in the history.
        question.invoke_cancelled();
        assert!(views.borrow().deleting.is_none(), "the question should have gone");
        assert!(editor.borrow().document().node("sensor").is_some());
        assert_eq!(editor.borrow().document().links.len(), 1);
        assert_eq!(editor.borrow().undo_depth(), depth, "cancelling is not an edit");

        // Asked again and confirmed: the node goes, and the line that touched
        // it goes with it -- a line to a node that is not there would dangle.
        ui.invoke_node_delete("sensor".into());
        let question = views
            .borrow()
            .deleting
            .as_ref()
            .map(|window| window.as_weak())
            .expect("asked again")
            .upgrade()
            .unwrap();
        question.invoke_confirmed();
        assert!(views.borrow().deleting.is_none());
        assert!(editor.borrow().document().node("sensor").is_none(), "the node stayed");
        assert!(editor.borrow().document().links.is_empty(), "a link was left dangling");

        // One edit, so one undo puts the node and its line back.
        assert_eq!(editor.borrow().undo_depth(), depth + 1);
        editor.borrow_mut().undo();
        assert!(editor.borrow().document().node("sensor").is_some());
        assert_eq!(editor.borrow().document().links.len(), 1);

        // And a node that is not there is not asked about: a menu left open
        // over a node somebody has already deleted.
        ui.invoke_node_delete("nothing-here".into());
        assert!(views.borrow().deleting.is_none());
    }

    /// While a line is being drawn it follows the pointer, and it goes when
    /// the gesture ends.
    ///
    /// Through the window, because every part of this is the window's: the
    /// terminal reports where the pointer is while it holds it, the canvas
    /// draws a dashed line there, and the line is erased and drawn again on
    /// every movement. Without it a person aims at nothing -- the line
    /// appeared only once the gesture was over, which is to say once it was
    /// too late to aim.
    fn the_line_being_drawn_follows_the_pointer() {
        use i_slint_backend_testing as testing;
        use slint::platform::{PointerEventButton, WindowEvent};
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Rc::new(RefCell::new(Editor::open(
            &where_.path().join("drawn.yaml"),
            dividers(),
        )));
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
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
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
        let at = |x: f32, y: f32| slint::LogicalPosition::new(origin.x + x, origin.y + y);

        // Pressed on the payload's payload-out, which is where its readings leave.
        let node = editor.borrow().document().node("sensor").expect("it").at;
        let (x, y, width, height) =
            style::terminal_box("i2c", node, "payload-out").expect("a terminal");
        let from = at(x + width / 2.0, y + height / 2.0);

        // Nothing drawn before the gesture.
        assert_eq!(ui.get_drawing_route(), "");

        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position: from });
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position: from,
            button: PointerEventButton::Left,
        });

        // Moved partway: a line from the terminal to the pointer, in pieces
        // because it is dashed.
        let halfway = at(400.0, 200.0);
        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position: halfway });
        let said = ui.get_drawing_route().to_string();
        assert!(said.contains('M'), "no line is being drawn: {said:?}");
        assert!(
            said.matches('M').count() > 1,
            "the line should be dashed: {said}"
        );
        assert!(said.ends_with("L 400.0 200.0"), "{said}");

        // Moved again: erased and drawn again, which is the point of it.
        let further = at(300.0, 320.0);
        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position: further });
        let moved = ui.get_drawing_route().to_string();
        assert_ne!(moved, said, "the line did not follow the pointer");
        assert!(moved.ends_with("L 300.0 320.0"), "{moved}");

        // And let go of over the ground, which makes the link and takes the
        // line being drawn away: what is on the canvas now is a link.
        let over = at(16.0 + 40.0, 120.0 + 40.0);
        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position: over });
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position: over,
            button: PointerEventButton::Left,
        });
        assert_eq!(
            ui.get_drawing_route(),
            "",
            "the line in hand should have gone"
        );
        assert_eq!(editor.borrow().document().links.len(), 1);
    }

    /// The Arrange item lays the diagram out and brings the view to it.
    ///
    /// Through the window, because the half worth testing is the half the unit
    /// tests cannot reach: the menu item has to reach the editor, and the view
    /// has to come back to the top of the sheet. A configuration spread down
    /// 1800 pixels of a sheet is what this is for -- the shipped example reads
    /// that way after a session of dragging, and a window shows a few hundred
    /// pixels of it at a time.
    fn the_menu_arranges_the_nodes_and_shows_them() {
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Rc::new(RefCell::new(Editor::open(
            &where_.path().join("vehicle.yaml"),
            dividers(),
        )));
        for (name, node_type, at) in [
            ("ground", "moc", (16, 1600)),
            ("sensor", "i2c", (dividers().vehicle + 24, 1200)),
            ("other", "i2c", (dividers().vehicle + 24, 1800)),
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
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));
        let chosen: Rc<RefCell<BTreeMap<String, String>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let drawing: Rc<RefCell<Option<Drawing>>> = Rc::new(RefCell::new(None));
        wire(&ui, &editor, &views, &drawing, &chosen, &None);
        views.borrow().refresh(&editor.borrow());

        // Scrolled away from where the nodes are, which is the state this
        // command is reached in.
        ui.set_canvas_offset(-1400.0);
        let depth = editor.borrow().undo_depth();

        ui.invoke_arrange();

        // Everything in the first screenful of its own pane.
        for node in &editor.borrow().document().nodes {
            let (left, right) = pane::pane_for(&node.node_type, dividers());
            assert!(
                node.at.0 >= left && node.at.0 <= right.max(left),
                "{} is at {:?}",
                node.name,
                node.at
            );
            assert!(node.at.1 < 400, "{} is at {:?}", node.name, node.at);
        }

        // One edit, so one undo puts the layout back.
        assert_eq!(editor.borrow().undo_depth(), depth + 1);

        // And the view is where the nodes are.
        assert_eq!(ui.get_canvas_offset(), 0.0);

        // Asked again with nothing to do, it does nothing -- including to the
        // history, because an undo that changes nothing is an undo that looks
        // broken.
        ui.invoke_arrange();
        assert_eq!(editor.borrow().undo_depth(), depth + 1);
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
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
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
            name: "cabin_temp".into(),
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

    /// Something wrong with the configuration is put in front of them too.
    ///
    /// A fault is an error: two lines into one input, two nodes of one name, a
    /// value a rule refuses. The configuration will not fly with one in it, and
    /// until this the editor said so in red inside a node forty-eight pixels
    /// tall and nowhere else. Said once, and said again if it is put right and
    /// made again.
    fn a_fault_is_put_where_it_cannot_be_missed() {
        use std::rc::Rc;

        let where_ = tempfile::tempdir().expect("a directory");
        let editor = Rc::new(RefCell::new(Editor::open(
            &where_.path().join("vehicle.yaml"),
            dividers(),
        )));
        for (name, node_type) in [
            ("temp_1", "i2c"),
            ("temp_2", "i2c"),
            ("cabin_temp", "compute"),
        ] {
            editor.borrow_mut().edit(Edit::AddNode {
                name: name.to_string(),
                node_type: node_type.to_string(),
                home: None,
            });
        }

        let ui = MainWindow::new().expect("a window");
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            drawing: Rc::new(RefCell::new(None)),
            divider: Rc::new(std::cell::Cell::new(dividers())),
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
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
                window.get_heading().to_string(),
                window.get_trouble().to_string(),
            )
        };

        // Nothing wrong yet: three nodes and no lines is a diagram somebody
        // has started, which is a concern at most.
        views.borrow().refresh(&editor.borrow());
        assert!(!shown().0, "{:?}", editor.borrow().faults());

        // Two lines into one input, which is the fault: a mean over one
        // reading and itself.
        let twice = |to: &str| journal::Link::joining(to, Some("payload-out"), "cabin_temp", Some("in0"));
        editor.borrow_mut().edit(Edit::AddLink { link: twice("temp_1") });
        views.borrow().refresh(&editor.borrow());
        assert!(!shown().0, "one line into an input is no fault");

        editor.borrow_mut().edit(Edit::AddLink { link: twice("temp_2") });
        views.borrow().refresh(&editor.borrow());
        let (visible, heading, said) = shown();
        assert!(visible, "the fault was not shown");
        assert!(heading.contains("configuration"), "{heading}");
        assert!(said.contains("two links arrive"), "{said}");
        assert!(said.contains("cabin_temp.in0"), "{said}");

        // Acknowledged, and the next edit does not bring it back: the red on
        // the node and the sentence in its window are still saying it.
        views.borrow().alert.as_ref().unwrap().invoke_acknowledged();
        editor.borrow_mut().edit(Edit::AddNode {
            name: "temp_3".to_string(),
            node_type: "i2c".to_string(),
            home: None,
        });
        views.borrow().refresh(&editor.borrow());
        assert!(!shown().0, "the same fault should not come back");

        // Put right and made again, and it is said again: a fault that is no
        // longer there is forgotten, which is what lets it be said twice.
        editor.borrow_mut().edit(Edit::RemoveLink { link: twice("temp_2") });
        views.borrow().refresh(&editor.borrow());
        assert!(editor.borrow().faults().is_empty(), "{:?}", editor.borrow().faults());
        editor.borrow_mut().edit(Edit::AddLink { link: twice("temp_2") });
        views.borrow().refresh(&editor.borrow());
        assert!(shown().0, "a fault made again should be said again");
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
            link: journal::Link::joining("sensor", Some("payload-out"), "ground", None),
        });

        let ui = MainWindow::new().expect("a window");
        furnish!(&ui, descriptions());
        let views = Rc::new(RefCell::new(Views {
            top: ui.as_weak(),
            open: BTreeMap::new(),
            chosen: Rc::new(RefCell::new(BTreeMap::new())),
            drawing: Rc::new(RefCell::new(None)),
            divider: Rc::new(std::cell::Cell::new(dividers())),
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
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

        // Taken hold of at the payload's own source, which is where this line
        // leaves: a line is deleted from the end that sends it. The end it
        // arrives at is ignored when it is dropped on nothing -- a sink takes
        // one source, so letting go of its end over empty canvas is a slip
        // more often than a decision -- which `the_end_of_a_link_can_be_moved
        // _or_dropped` is where that half is checked.
        let from = middle("sensor", "i2c", (dividers().vehicle + 24, 120), "payload-out");
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
            origin.x + dividers().payloads as f32 + 100.0,
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
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
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
        let voter = rows_of(Pane::Vehicle)
            .into_iter()
            .find(|row| row.node.name == "vote")
            .expect("the palette offers a vote");
        ui.invoke_place_node(voter.node.name.clone(), voter.node.node_type.clone(), voter.sets);

        // Named after the type, numbered from one: see `fresh_name`. The
        // palette entry is called "vote" and the node it places is "vote-1",
        // which is the editor saying "this is the first of them" rather than
        // "this is the vote".
        let placed = editor
            .borrow()
            .document()
            .node("vote-1")
            .cloned()
            .expect("a node called vote-1");
        assert_eq!(placed.node_type, "compute");
        assert_eq!(
            placed.attributes.get("function").map(String::as_str),
            Some("vote"),
            "a vote placed from the palette is a vote"
        );

        // And the panel asks it for a vote's attributes rather than for every
        // attribute any computation has.
        chosen.borrow_mut().insert(String::new(), "vote-1".to_string());
        let (_, _, _, fields) = panel_of(&editor.borrow(), &None, Some("vote-1"));
        let asked: Vec<String> = fields
            .iter()
            .filter(|field| !field.is_heading)
            .map(|field| field.name.to_string())
            .collect();
        assert_eq!(
            asked,
            vec![
                "function".to_string(),
                "inputs".to_string(),
                "algorithm".to_string()
            ]
        );

        // Placed in one step, so undone in one: the node, the function its
        // entry answered and the place it was put all go with one Undo, and
        // all come back with one Redo.
        let at = placed.at;
        editor.borrow_mut().undo();
        assert!(
            editor.borrow().document().node("vote-1").is_none(),
            "one undo takes the placed node away"
        );
        editor.borrow_mut().redo();
        let back = editor
            .borrow()
            .document()
            .node("vote-1")
            .cloned()
            .expect("one redo puts it back");
        assert_eq!(back.at, at, "where it was placed, not the corner");
        assert_eq!(
            back.attributes.get("function").map(String::as_str),
            Some("vote"),
            "and still a vote"
        );

        // And so for every entry the palette offers, in every pane: whatever
        // a type's entry answers, it is one Undo away from not being there
        // and one Redo from being back where it was put.
        for pane in [Pane::Payloads, Pane::Vehicle, Pane::Moc] {
            for row in rows_of(pane) {
                let before: Vec<String> = editor
                    .borrow()
                    .document()
                    .nodes
                    .iter()
                    .map(|node| node.name.clone())
                    .collect();
                ui.invoke_place_node(
                    row.node.name.clone(),
                    row.node.node_type.clone(),
                    row.sets.clone(),
                );
                let placed = editor
                    .borrow()
                    .document()
                    .nodes
                    .iter()
                    .find(|node| !before.contains(&node.name))
                    .cloned()
                    .unwrap_or_else(|| panic!("{} placed nothing", row.node.name));

                editor.borrow_mut().undo();
                assert!(
                    editor.borrow().document().node(&placed.name).is_none(),
                    "one undo should take {} away",
                    placed.name
                );
                editor.borrow_mut().redo();
                let back = editor
                    .borrow()
                    .document()
                    .node(&placed.name)
                    .cloned()
                    .unwrap_or_else(|| panic!("one redo should put {} back", placed.name));
                assert_eq!(back, placed, "{} came back other than it was", placed.name);
            }
        }
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
            ("other", "i2c", (dividers().vehicle + 24, 320)),
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
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
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

        // And the middle of a node, which is the other place a line may be
        // let go of: anywhere on the node it is being joined to will do.
        let centre = |node: &str| {
            let found = editor.borrow().document().node(node).expect("the node").clone();
            let (width, height) = size_of(&found.node_type);
            slint::LogicalPosition::new(
                origin.x + found.at.0 as f32 + width as f32 / 2.0,
                origin.y + found.at.1 as f32 + height as f32 / 2.0,
            )
        };

        let from = middle("sensor", "payload-out");
        let to = middle("ground", "payload-in");

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
        assert_eq!(links[0].origin.as_deref(), Some("payload-out"));
        assert_eq!(links[0].input, None);

        // And the node did not move: a press on a terminal draws a link, and
        // the drag the same press would otherwise have started is what the
        // terminals sit on top of.
        assert_eq!(
            editor.borrow().document().node("sensor").expect("it").at,
            (dividers().vehicle + 24, 120)
        );

        // The same gesture let go of anywhere on the node, which is the way
        // a person actually draws one: the other sensor's readings dropped in
        // the middle of the ground link's box. The terminal is the stream's
        // own answer -- a payload-out arrives at a payload-in -- so there is
        // nothing to aim at.
        let gesture = |from: slint::LogicalPosition, to: slint::LogicalPosition| {
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
        };

        gesture(middle("other", "payload-out"), centre("ground"));
        let links = editor.borrow().document().links.clone();
        assert_eq!(
            links.len(),
            2,
            "a line let go of on a node should be joined to it: {links:?}"
        );
        let made = links.iter().find(|link| link.from == "other").expect("the second line");
        assert_eq!(made.to, "ground");
        assert_eq!(made.origin.as_deref(), Some("payload-out"));

        // And a node with no terminal of that stream takes nothing: a payload
        // receives what asks it for a reading, so what arrives at its
        // moc-in leaves a moc-out -- and a payload has none to offer.
        gesture(middle("other", "moc-in"), centre("sensor"));
        assert_eq!(
            editor.borrow().document().links.len(),
            2,
            "a payload has no moc-out for a line to leave by"
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
            put(&ui, &editor.borrow(), &None, None, dividers(), lists(), None);

            let names: Vec<String> = (0..ui.get_nodes().row_count())
                .filter_map(|row| ui.get_nodes().row_data(row))
                .map(|node| node.name.to_string())
                .collect();
            // Every node the file holds, in the order its sections are read:
            // the ground link, then the vehicle's own graph, then what it
            // carries -- which is the three working panes right to left.
            //
            // What the file holds rather than a list written out here: this is
            // the file the editor opens by default, so what is in it is
            // whatever was last drawn.
            let held: Vec<String> = editor
                .borrow()
                .document()
                .nodes
                .iter()
                .map(|node| node.name.clone())
                .collect();
            // Whatever it holds, including nothing: an emptied configuration
            // is somebody's work in progress and a canvas with nothing on it
            // is the right drawing of it.
            assert_eq!(
                names, held,
                "example.{extension} should put its nodes on the canvas"
            );
            assert_eq!(
                ui.get_links().row_count(),
                editor.borrow().document().links.len(),
                "example.{extension}: the canvas and the file disagree about its lines"
            );
        }

        // The lines are the worked example's, and each is drawn with a head
        // and a marker: two of its lines run between one pair of nodes, one
        // each way, so the direction is the only thing that tells them apart.
        for extension in ["yaml", "xml"] {
            let editor = Editor::open(
                &root.join(format!("tests/manual/thermostat.{extension}")),
                dividers(),
            );
            let ui = MainWindow::new().expect("a window");
            furnish!(&ui, descriptions());
            put(&ui, &editor, &None, None, dividers(), lists(), None);

            assert_eq!(
                ui.get_links().row_count(),
                18,
                "thermostat.{extension} should draw every line"
            );
            for row in 0..ui.get_links().row_count() {
                let link = ui.get_links().row_data(row).expect("a link");
                assert!(
                    link.head.ends_with('Z'),
                    "thermostat.{extension}: line {row} has no arrowhead: {:?}",
                    link.head
                );
                assert!(
                    link.marker.ends_with('Z'),
                    "thermostat.{extension}: line {row} has no flow marker: {:?}",
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
            now: (dividers().payloads + 120, 88),
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
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
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
            snap((was.0 + by.0 as i32, was.1 + by.1 as i32)),
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
            name: "temp_1".to_string(),
            node_type: "i2c".to_string(),
            home: None,
        });
        // In its own pane, a little in from the divider.
        let was = (24, 88);
        editor.borrow_mut().edit(Edit::MoveNode {
            node: "temp_1".to_string(),
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
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
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
        // Far enough right to be in the vehicle pane if nothing stopped it.
        let by = (240.0, 24.0);
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
            .node("temp_1")
            .expect("the node")
            .at;
        assert_eq!(
            now,
            (
                pane::node_floor(dividers().payloads - style::WIDTH as i32),
                pane::on_node_grid(was.1 + by.1 as i32)
            ),
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
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
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

        // And the fields are the same fields afterwards, which is what lets
        // somebody type into one.
        //
        // Every character typed is an edit, every edit a refresh, and a
        // refresh used to replace this model -- which destroys the elements a
        // repeater built from it, the text box among them, and takes the
        // cursor with it. The symptom was a value that had to be typed one
        // character and one click at a time. So the rows are updated in place
        // and the model is the model it was: see `refilled`.
        let before = window.get_fields();
        let before = before
            .as_any()
            .downcast_ref::<VecModel<FieldView>>()
            .expect("a list of fields") as *const VecModel<FieldView>;
        window.invoke_parameter_set("tcs_cmd.send".into(), "127.0.0.1:4100".into());
        views.borrow().refresh(&editor.borrow());
        let after = window.get_fields();
        let after = after
            .as_any()
            .downcast_ref::<VecModel<FieldView>>()
            .expect("a list of fields") as *const VecModel<FieldView>;
        assert!(
            std::ptr::eq(before, after),
            "the fields were replaced, which takes the cursor with them"
        );
        // And the row says what was typed, so updating in place is not the
        // same as not updating.
        let sent = (0..window.get_fields().row_count())
            .filter_map(|row| window.get_fields().row_data(row))
            .find(|f| f.name == "tcs_cmd.send")
            .expect("the field");
        assert_eq!(sent.value, "127.0.0.1:4100");
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
        for (name, interval) in [("cabin_temps", "3 s"), ("bay", "5 s")] {
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
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
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
        assert_eq!(window.get_interval(), "3 s");

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
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
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
            name: "cabin_temp".into(),
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
        assert!(written.contains("cabin_temp"), "{written}");
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
    /// Reached by a click and not by a press, because a press is how a drag
    /// starts: a gesture that moved the node is a move, and one that moved
    /// nothing asks what there is to do with it. Told apart by the distance,
    /// not by the time.
    ///
    /// The click opens the node's menu, whose Edit opens this window. The menu
    /// itself is the toolkit's and is not on this canvas to be found by a
    /// test, so what is checked here is the two halves either side of it: that
    /// a click records no edit, and that Edit opens the window for the node
    /// that was asked about.
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
            let left = dividers().payloads;
            for (name, node_type, at) in [
                ("to_kelvin", "compute", (left + 120, 88)),
                ("cabin_temp", "compute", (left + 120, 240)),
                // And a payload, in its own pane. Of a different type on
                // purpose: a click has to open the window for whatever was
                // clicked, and a payload is the one a person reaches for
                // first.
                ("sensor", "i2c", (24, 88)),
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
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
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

        click((dividers().payloads + 120, 88), "compute");
        assert!(
            views.borrow().attributes.is_empty(),
            "a click opens the menu and nothing else: {:?}",
            views.borrow().attributes.keys().collect::<Vec<_>>()
        );
        ui.invoke_node_edit("to_kelvin".into());
        assert!(
            views.borrow().attributes.contains_key("to_kelvin"),
            "Edit should open what the node says: {:?}",
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
        click((dividers().payloads + 120, 240), "compute");
        ui.invoke_node_edit("cabin_temp".into());
        assert_eq!(
            views.borrow().attributes.len(),
            2,
            "a second node should open a second window: {:?}",
            views.borrow().attributes.keys().collect::<Vec<_>>()
        );
        assert!(views.borrow().attributes.contains_key("to_kelvin"));

        // Clicking the same node again raises its window rather than making a
        // second one saying the same thing.
        click((dividers().payloads + 120, 88), "compute");
        ui.invoke_node_edit("to_kelvin".into());
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
        ui.invoke_node_edit("askew".into());
        assert!(
            views.borrow().attributes.contains_key("askew"),
            "a node off the grid should answer the first click: {:?}",
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
        ui.invoke_node_edit("sensor".into());
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
            vec!["askew", "cabin_temp", "sensor"],
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
            for (name, node_type) in [("temp_1", "i2c"), ("k", "compute")] {
                editor.edit(Edit::AddNode {
                    name: name.to_string(),
                    node_type: node_type.to_string(),
                    home: None,
                });
            }
        }

        // With no simulator file, a payload's panel is its own attributes and
        // nothing else: there is no second file for anything to go in.
        let (_, _, _, fields) = panel_of(&editor.borrow(), &None, Some("temp_1"));
        assert!(
            fields.iter().all(|f| !f.is_heading && !f.simulated),
            "the panel should carry nothing of a file that was not given"
        );

        editor
            .borrow_mut()
            .open_simulator(&where_.path().join("vehiclesim.yaml"));

        // Now the payload's panel is its attributes, a rule, and the
        // simulator's settings for it.
        let (_, _, _, fields) = panel_of(&editor.borrow(), &None, Some("temp_1"));
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
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
            sim_groups: None,
            showing_sim_group: Rc::new(RefCell::new(String::new())),
            showing: Rc::new(RefCell::new(String::new())),
        }));

        show_node(&editor, &views, "temp_1");
        let window = views
            .borrow()
            .attributes
            .get("temp_1")
            .map(|window| window.as_weak())
            .expect("the node's window")
            .upgrade()
            .expect("still open");

        let depth = editor.borrow().undo_depth();
        window.invoke_sim_attribute_set("drop_percent".into(), "10".into());
        assert_eq!(
            editor.borrow().document().simulator.as_ref().unwrap().payloads
                ["temp_1"]["drop_percent"],
            "10"
        );
        // One edit, and the node's own attributes are untouched: two files,
        // and the panel knows which row goes where.
        assert_eq!(editor.borrow().undo_depth(), depth + 1);
        assert!(!editor
            .borrow()
            .document()
            .node("temp_1")
            .unwrap()
            .attributes
            .contains_key("drop_percent"));
        assert!(editor.borrow_mut().undo());
        assert!(!editor.borrow().document().simulator.as_ref().unwrap().payloads
            ["temp_1"]
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
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
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
        groups.invoke_setting_set("packet_interval".into(), "500 ms".into());
        assert_eq!(
            editor.borrow().document().simulator.as_ref().unwrap().groups["group"]
                ["packet_interval"],
            "500 ms"
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
                ["packet_interval"],
            "500 ms",
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
                interval: "3 s".to_string(),
            });
            for name in ["temp_1", "temp_2"] {
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
            lists: Rc::new(std::cell::Cell::new((
                settings::DEFAULT_LIST_MOC,
                settings::DEFAULT_LIST_VEHICLE,
            ))),
            group: None,
            parameters: None,
            attributes: BTreeMap::new(),
            leaving: None,
            concerns: None,
            deleting: None,
            names: None,
            alert: None,
            alerted: Rc::new(RefCell::new(String::new())),
            faulted: Rc::new(RefCell::new(Default::default())),
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
        assert_eq!(window.get_interval(), "3 s");
        assert_eq!(window.get_members_text(), "temp_1, temp_2");

        // One of the fields they both have, set once.
        let depth = editor.borrow().undo_depth();
        window.invoke_attribute_set("path".into(), "/dev/i2c-1".into());

        for name in ["temp_1", "temp_2"] {
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
        for name in ["temp_1", "temp_2"] {
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
                .node("temp_1")
                .unwrap()
                .attributes
                .get("group")
                .map(String::as_str),
            Some("cabin")
        );
        window.invoke_interval_set("500 ms".into());
        assert_eq!(
            editor.borrow().document().groups.get("cabin").map(String::as_str),
            Some("500 ms")
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
        window.set_payload_divider(dividers().payloads as f32);
        window.set_vehicle_divider(dividers().vehicle as f32);

        // Both of them, because there are two and each should report its own
        // gesture: a window that wired one callback to both lines would pass
        // this test for the wrong reason.
        let moved: Rc<Cell<Option<f32>>> = Rc::new(Cell::new(None));
        let payload_moved: Rc<Cell<Option<f32>>> = Rc::new(Cell::new(None));
        {
            let moved = moved.clone();
            window.on_vehicle_divider_moved(move |dx| moved.set(Some(dx)));
        }
        {
            let payload_moved = payload_moved.clone();
            window.on_payload_divider_moved(move |dx| payload_moved.set(Some(dx)));
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
            payload_moved.get(),
            None,
            "dragging the vehicle divider moved the payload divider too"
        );

        // And the other line, which is its own control.
        let from =
            slint::LogicalPosition::new(at.x + dividers().payloads as f32, at.y + 200.0);
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
        let said = payload_moved
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
        let node = view_of("temp_1", "i2c", at, None, false, None);
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
