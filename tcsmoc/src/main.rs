//! TCSpecial Mission Operations Center (tcsmoc)
//!
//! A GUI application for testing and visualizing tcspecial operation.

use slint::{LogicalSize, Model, ModelRc, SharedString, VecModel};
use std::env;
use std::path::Path;
use std::process::{exit, Child, Command};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tcslibgs::config::payload_path_from_args;
use tcslibgs::{Edge, NO_TRANSFER_TIME};
use tcsverify::startup::Startup;

use crate::oc_link::Arrivals;

slint::include_modules!();

mod beacon_receive;
mod ci_link;
mod config;
mod oc_link;
mod vehicle;

/// What the window's status line says about the link to the command
/// interpreter.
///
/// `ui/main.slint` compares the status against "Connected" to choose the
/// colour it is shown in, and defaults it to "Disconnected"; these are the
/// same strings, named here so the two files can be checked against each
/// other.
pub(crate) const CONNECTED_STATUS: &str = "Connected";
pub(crate) const DISCONNECTED_STATUS: &str = "Disconnected";
pub(crate) const ERROR_STATUS: &str = "Error";

/// The programs the MOC starts.
///
/// Each takes its tcspecial configuration file as an argument, as the MOC does,
/// so the MOC passes on the file it was given. It used to set each child's own
/// environment variable instead; an argument says the same thing without a
/// name that reaches further than intended, and all three programs now read
/// their file the same way.
///
/// Tcssim's simulator configuration is not passed on: tcssim takes it from
/// `PAYLOAD_SIM_YAML`, inherited from the MOC's environment like any other
/// variable, and so is already looking at the file the MOC was pointed at.
/// The MOC reads that file itself, but only to show it -- see
/// `simulator_settings` -- and acts on none of it, so it has nothing to say
/// about which one is right.
const CHILDREN: [&str; 2] = ["tcspecial", "tcssim"];

/// Which environment variable names the simulator configuration.
///
/// The same variable tcssim reads, which is the point: the MOC shows what the
/// tcssim it starts is simulating, so the two have to be looking at one file.
///
/// There was a default beside this, read when the variable was unset and no
/// file was beside the tcspecial configuration -- and it named another
/// payload set's simulator file, so a set with no simulator of its own was
/// read against one describing payloads it had never heard of. A set with no
/// simulator file simulates nothing: see [`sim_path_for`].
const SIM_CONFIG_PATH_VAR: &str = "PAYLOAD_SIM_YAML";

/// What one column of the grid takes, and the height of everything above and
/// below the grid.
///
/// The sizes the window is built from, as the markup reports them.
///
/// Asked for rather than declared here as well. `ui/main.slint` lays out a
/// panel of its own and hands over what it came to, and states the gap between
/// panels and what the window spends around the grid. This module does
/// arithmetic on those four and keeps no copy of them.
///
/// It kept copies until recently, and every one of them was wrong: a panel was
/// called 180 tall where its rows want 162 and 256 wide where they take 255,
/// and the margin round the grid was called 30 where it is 36 -- and the gaps
/// between columns were left out of the window's width altogether, so a
/// three-column grid stood twenty-six pixels wider than the area it scrolled
/// in and the rightmost column was cut off.
///
/// What is measured and what is merely stated differs between them, and the
/// markup says which: a panel is measured, the chrome cannot be.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Sizes {
    panel_width: f32,
    panel_height: f32,
    grid_spacing: f32,
    chrome_width: f32,
    chrome_height: f32,
}

impl Sizes {
    /// What the window says about itself.
    ///
    /// Read before the window is sized, which works because a preferred size
    /// is a layout question: what a panel wants is known without laying the
    /// window out, where what it got is not.
    fn of(ui: &MainWindow) -> Self {
        // A panel's size comes from a window of its own -- see PanelProbe in
        // ui/main.slint -- built, read and dropped here and never shown. The
        // rest comes from the real window, which knows its own padding.
        let probe = PanelProbe::new().expect("a window to measure a panel in");
        let sizes = Self {
            panel_width: probe.get_panel_width(),
            panel_height: probe.get_panel_height(),
            grid_spacing: ui.get_grid_spacing(),
            chrome_width: ui.get_chrome_width(),
            chrome_height: ui.get_chrome_height(),
        };

        // What the window lays the grid out by, so the cells are the size the
        // measurement says.
        ui.set_panel_width(sizes.panel_width);
        ui.set_panel_height(sizes.panel_height);
        sizes
    }

    /// Measure a panel and tell the window what it came to.
    ///
    /// Every window needs this before it lays anything out: the grid's cells
    /// are the panel size, so one that was never told comes up with cells of
    /// nought and builds no panels at all.
    fn settle(ui: &MainWindow) -> Self {
        Self::of(ui)
    }

    /// The sizes the shipped window reports, for tests with no window to ask.
    ///
    /// The shape rule does not depend on what a panel comes to, so a test of
    /// that rule needs numbers rather than the numbers.
    /// `the_markup_reports_the_sizes_the_tests_assume` keeps these from
    /// drifting away from the markup they were read off.
    #[cfg(test)]
    fn for_tests() -> Self {
        Self {
            panel_width: 255.0,
            panel_height: 165.0,
            grid_spacing: 10.0,
            chrome_width: 36.0,
            chrome_height: 369.0,
        }
    }

    /// How wide a window holding `columns` columns of panels is, gaps and all.
    fn width_for_columns(&self, columns: usize) -> f32 {
        let columns = columns.max(1);
        columns as f32 * self.panel_width
            + (columns - 1) as f32 * self.grid_spacing
            + self.chrome_width
    }
}

/// How many rows of panels the window looks ahead for.
///
/// Asked for rather than worked out: a window with room for only the rows a
/// particular tcspecial configuration happens to have is one that has to be
/// resized before a larger file can be watched, and three rows is the room that
/// was wanted. It is not a floor, though -- see `rows_of_room`.
const MIN_PANEL_ROWS: usize = 3;

/// How many rows of panels the window opens with room for.
///
/// Never fewer than the grid has, so that nothing is cut off; never more than
/// [`MIN_PANEL_ROWS`], which is as far ahead as the window looks; and never
/// more rows than there are panels to put in them, so a file of one or two
/// handlers opens only as tall as those need rather than keeping room it
/// could not fill.
fn rows_of_room(rows: usize, panels: usize) -> usize {
    rows.max(MIN_PANEL_ROWS.min(panels.max(1)))
}

/// How tall a window with room for `rows` rows of panels is.
///
/// One row of them is the window's `min-height`, the least it is ever opened
/// at; `rows_of_room` says how many rows a particular tcspecial configuration
/// opens with.
fn height_for_rows(sizes: &Sizes, rows: usize) -> f32 {
    let rows = rows.max(1);
    rows as f32 * sizes.panel_height + (rows - 1) as f32 * sizes.grid_spacing + sizes.chrome_height
}

/// The size the window opens at for a grid of `shape` holding `panels`.
fn window_size(sizes: &Sizes, shape: &GridShape, panels: usize) -> LogicalSize {
    LogicalSize::new(
        shape.width,
        height_for_rows(sizes, rows_of_room(shape.rows, panels)),
    )
}

/// The narrowest the window may open, matching `min-width` in the window
/// itself.
///
/// What this is for has changed. The command interpreter's controls used to
/// be a row that stopped fitting below 640; stacked in their box they want
/// far less than that, and what the floor holds up instead is the shape of
/// the grid, which cannot be separated from it: four handlers in two columns
/// stand 680 tall, and a window that may be narrower than that is taller
/// than wide, which puts the shape out of the running altogether --
/// `grid_shape` does not consider such shapes at all, so four panels would
/// open in a 3x2 grid with two cells empty. At 700 every panel count keeps the
/// shape it has. (Two panels used to be the other half of this argument: they
/// stacked, and a narrower window would have put them side by side. They are
/// side by side at any width now -- a grid is never taller in panels than it is
/// wide -- so the four-panel case is what the floor holds up.) Anything that
/// changes the panel or chrome heights has to be weighed the same way, against
/// `the_shipped_payload_config_opens_a_nearly_square_window`.
const CONTROLS_WIDTH: f32 = 700.0;

/// The least the window is ever opened at, across.
///
/// Two halves of one floor. [`CONTROLS_WIDTH`] is what the command
/// interpreter's own controls need; the other half is the shape of the grid,
/// which must be wider than it is tall while it fits two rows -- panels read
/// across the window, and a window taller than wide invites a column of them.
///
/// Written as that rule rather than as the number it comes to, so that a
/// panel which grows widens the window instead of breaking the rule. The line
/// saying what the ground itself received took two rows from 680 to 716, and
/// a floor fixed at 700 would have left the shipped four-panel set taller
/// than wide.
fn min_window_width(sizes: &Sizes) -> f32 {
    CONTROLS_WIDTH.max(height_for_rows(sizes, 2))
}

/// How the grid of data handler panels is shaped, and the window size that
/// shape asks for.
#[derive(Debug, Clone, Copy, PartialEq)]
struct GridShape {
    columns: usize,
    rows: usize,
    width: f32,
    height: f32,
}

/// The most panels tcsmoc puts in a row.
///
/// Three of them is 960 pixels of panel, which is a window most screens have
/// the width for and a row an eye can take in at once. A fourth column would
/// widen the window past 1280 before the grid had earned it.
const MAX_COLUMNS: usize = 3;

/// How many columns `panels` handlers are laid out in.
///
/// A row is filled before another is started under it, up to [`MAX_COLUMNS`]
/// across. The window works the same count back out of the width it was
/// given, so that resizing reflows the grid; see `columns` in
/// `ui/main.slint`.
fn columns_for(panels: usize) -> usize {
    panels.clamp(1, MAX_COLUMNS)
}

/// Choose the grid shape for `panels` data handlers.
///
/// A row is filled before another is started under it, up to [`MAX_COLUMNS`]
/// across. So the shape is settled by the count alone: three panels go in one
/// row of three, four in a row of three and a row of one.
///
/// This replaced a rule that chose the shape nearest square among those at
/// least as wide as they were tall. That rule read well and laid the panels
/// out badly: two panels came out as one column of two, because a 700x680
/// window is squarer than a 700x508 one, so the set the Makefile runs went
/// down the window instead of across it. Squareness was never the thing
/// wanted -- it was a proxy for a window that fits a screen, and a column
/// count says that directly.
///
/// The window is no longer always wider than it is tall: ten panels or more
/// are four rows of a three-wide grid, which is taller than 960. They scroll,
/// as too many panels always have.
fn grid_shape(sizes: &Sizes, panels: usize) -> GridShape {
    // An empty configuration still has to produce a window.
    let panels = panels.max(1);
    let columns = columns_for(panels);
    let rows = panels.div_ceil(columns);

    GridShape {
        columns,
        rows,
        // The floor is a floor rather than the width of one column:
        // what the command interpreter's controls need is wider than a single
        // panel, so one and two panels both open at it.
        //
        // The chrome and the column gaps are both in it: the grid sits in a
        // scrolling area with padding of its own and keeps a gap between one
        // panel and the next, and a window as wide as its panels alone put the
        // last column's buttons over the edge -- thirty pixels for the padding
        // that was allowed for, and twenty more for two gaps that were not.
        width: sizes
            .width_for_columns(columns)
            .max(min_window_width(sizes)),
        height: height_for_rows(sizes, rows),
    }
}

/// Which simulator file goes with this payload set, if it has one.
///
/// The one `PAYLOAD_SIM_YAML` names, and otherwise the one beside the
/// tcspecial configuration and named for it -- see
/// `tcslibgs::config::sim_path_beside`, which tcssim asks as well -- which is
/// the convention the whole payload set mechanism rests on.
///
/// `None` for a set with no simulator file beside it, which is a set with
/// nothing simulated. It used to fall back to one named here, which was
/// another payload set's: a configuration of its own was then read against
/// payloads it had never heard of, and the window opened complaining about
/// four handlers nobody had mentioned. A set that simulates nothing says so
/// and starts no simulator.
fn sim_path_for(payload_path: &str) -> Option<String> {
    match env::var(SIM_CONFIG_PATH_VAR) {
        Ok(named) => Some(named),
        Err(_) => tcslibgs::config::sim_path_beside(payload_path),
    }
}

/// Say what was found as tcsmoc started, in a window of its own, and wait for
/// it to be closed.
///
/// Before the main window is shown, so that nobody is commanding payloads
/// from panels for a set with something wrong with it without having been
/// told -- and on stderr as well, for a run from a terminal and for one with
/// no display to open a window on.
fn present(startup: &Startup) {
    startup.eprint("tcsmoc");

    let window = match StartupWindow::new() {
        Ok(window) => window,
        Err(e) => {
            eprintln!("tcsmoc: what was found cannot be shown in a window: {e}");
            return;
        }
    };
    window.set_program("tcsmoc".into());
    let heading = if startup.fatal() {
        "Something is wrong with the configuration."
    } else {
        "Something about the configuration is worth knowing."
    };
    window.set_heading(heading.into());
    window.set_everything(startup.copied(heading).into());
    window.set_fatal(startup.fatal());
    let lines: Vec<StartupLine> = startup
        .lines()
        .iter()
        .map(|(error, text)| StartupLine {
            text: text.clone().into(),
            error: *error,
        })
        .collect();
    window.set_lines(ModelRc::from(Rc::new(VecModel::from(lines))));
    {
        let weak = window.as_weak();
        window.on_acknowledged(move || {
            if let Some(window) = weak.upgrade() {
                let _ = window.hide();
            }
        });
    }
    // Runs until it is closed, by its button or by the window manager: either
    // is the person having read it.
    if let Err(e) = window.run() {
        eprintln!("tcsmoc: what was found cannot be shown in a window: {e}");
    }
}

/// Say what was found and stop, where any of it stops tcsmoc.
fn present_and_stop_if_fatal(startup: &Startup) {
    if startup.fatal() {
        stop(startup);
    }
}

/// Say what was found, and stop.
fn stop(startup: &Startup) -> ! {
    present(startup);
    exit(1);
}

/// Put what the ground received into the panels.
///
/// What no command asked for: the listeners in [`oc_link`] put it where this
/// can pick it up, as the poller does. Read every pass rather than taken,
/// because it is the last thing that arrived rather than a queue of events --
/// a panel shows the latest, as it does for the times beside it.
///
/// All three of a panel's readings of its payload come from here, and that is
/// the point of them coming from one place: when the data arrived, what it
/// was, and how much of it there has been are one account, kept by the end
/// that received it.
/// A time as the window shows it, or the placeholder if there is none.
///
/// The same placeholder every other time in the window uses, so a reading
/// that has not happened looks the same wherever it is.
pub(crate) fn shown_time(at: Option<tcslibgs::Timestamp>) -> String {
    match at {
        Some(at) => at.time_of_day(),
        None => NO_TRANSFER_TIME.to_string(),
    }
}

fn show_arrivals(dh_model: &Rc<VecModel<DHInfo>>, arrivals: &Arrivals) {
    let Ok(slots) = arrivals.lock() else { return };

    for (row, arrival) in slots.iter().enumerate() {
        let Some(arrival) = arrival else { continue };

        let (time, data) = arrival.panel_lines();
        let packets = i32::try_from(arrival.packets).unwrap_or(i32::MAX);
        update_row(dh_model, row, |info| {
            info.oc_time = SharedString::from(time.clone());
            info.oc_data = SharedString::from(data.clone());
            info.packets_recv = packets;
        });
    }
}

/// Change one panel's contents, leaving the rest of it alone.
///
/// A Slint model row is a value rather than a place, so a field is changed by
/// reading the row, changing it, and putting it back.
fn update_row(model: &Rc<VecModel<DHInfo>>, row: usize, f: impl FnOnce(&mut DHInfo)) {
    if let Some(mut info) = model.row_data(row) {
        f(&mut info);
        model.set_row_data(row, info);
    }
}

/// The command that starts one of the MOC's children.
///
/// The child is told which tcspecial configuration to read, as an argument.
/// Tcsmoc builds its panels from that file, so a child reading a different one
/// would serve or simulate payloads the panels do not describe -- handlers that
/// never connect, with nothing on screen to say why. An argument also beats
/// anything the child would have taken from the environment, so for the run of
/// a payload set the MOC's own file is the one that counts.
///
/// The tcspecial configuration is the whole of what a child is told. Where
/// tcspecial takes commands and where it sends beacons are both in its own
/// configuration file, so neither is the MOC's to hand over -- which means the
/// MOC and a tcspecial it starts agree about the command address only by
/// reading the same default, and a tcspecial whose file moves it is one this
/// MOC will not reach. The exchange on connecting is what reports a mismatch of
/// configuration; an address is not part of it.
///
/// The command is built rather than run so that a test can read what a child
/// would be started with, without starting it. The children open windows and
/// bind fixed ports, so a test that spawned them would not be a test anyone
/// could run twice at once, or alongside a real session.
fn child_command(name: &str, payload_path: &str) -> Command {
    let mut command = Command::new("cargo");
    command.args(["run", "--bin", name, "--", payload_path]);
    command
}

/// Manages the tcssim subprocess
struct ProcessManager {
    child: Arc<Mutex<Option<Child>>>,
    name: Mutex<String>,
}

impl ProcessManager {
    fn new() -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
            name: Mutex::new(String::new()),
        }
    }

    /// Starts a child in a background thread and exits when it completes.
    fn start_child(&self, name: &str, payload_path: &str) {
        let child = child_command(name, payload_path)
            .spawn()
            .unwrap_or_else(|_| panic!("Failed to start {}", name));

        *self.child.lock().unwrap() = Some(child);
        *self.name.lock().unwrap() = name.to_string();

        let child_handle = self.child.clone();
        let name_clone = name.to_string();

        thread::spawn(move || {
            eprintln!("start_child: started {}", name_clone);
            loop {
                thread::sleep(Duration::from_millis(100));
                let mut guard = child_handle.lock().unwrap();
                if let Some(ref mut child) = *guard {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            println!("{} exited with status: {}", name_clone, status);
                            drop(guard);
                            exit(0);
                        }
                        Ok(None) => {
                            // Still running
                        }
                        Err(e) => {
                            println!("Error waiting for {}: {}", name_clone, e);
                            drop(guard);
                            exit(1);
                        }
                    }
                } else {
                    // Process handle was taken (killed), exit thread
                    break;
                }
            }
        });
    }

    fn kill(&self) {
        let name = self.name.lock().unwrap();
        eprintln!("Kill child {}", *name);
        drop(name);

        let mut guard = self.child.lock().unwrap();
        if let Some(ref mut child) = *guard {
            let _ = child.kill();
            let _ = child.wait();
            println!("Process killed");
        }
        *guard = None;
    }
}

fn main() {
    eprintln!("TcsMoc running");

    // Read the payload configuration before anything else, so a file the MOC
    // cannot read is reported before subprocesses are started and a window
    // appears. The window is built from what this file says: the number of
    // panels, the grid they sit in, and the size the window opens at all
    // follow from how many data handlers it describes.
    // No variable of its own: the MOC starts the other two and they inherit
    // its environment, so a variable here would name their file as well.
    let payload_path = match payload_path_from_args(env::args(), None) {
        Ok(payload_path) => payload_path,
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    };
    eprintln!("payload configuration: {}", payload_path);

    // Everything wrong with the payload set, in tcsverify's words: every
    // problem at once, each with its file and line, which is what tcsverify,
    // tcspecial, tcssim and tcsgui say about the same files. The simulator
    // file is read loosely, as it is below: what is wrong with it is said,
    // and costs the panels the simulator half of what they show.
    //
    // All of what this finds, and what the MOC finds for itself below, is
    // shown before the main window is -- and the window is not shown at all
    // where any of it stops the MOC.
    let sim_path = sim_path_for(&payload_path);
    let mut startup = Startup::default();
    startup.report_loosely(
        &tcsverify::verify(
            Path::new(&payload_path),
            sim_path.as_deref().map(Path::new),
            tcsverify::Limits::EVERYTHING,
        ),
        // Where a set has no simulator file, the configuration itself is what
        // every problem is reported against: there is no second file for one
        // to belong to.
        Path::new(sim_path.as_deref().unwrap_or(&payload_path)),
    );
    present_and_stop_if_fatal(&startup);

    vehicle::run(&payload_path, sim_path.as_deref(), startup);
}

/// Ask for this window to be put against one edge of the display.
///
/// Asked once the event loop is running, because that is when the window has
/// a size: before it is shown, what it reports is what was requested rather
/// than what it was given, and the bottom of the display is the display's
/// height less the window's.
///
/// A request, and not a thing to report on: see `tcslibgs::display`.
fn ask_for_the_edge(ui: &MainWindow, edge: Edge) {
    let ui_weak = ui.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::SingleShot,
        std::time::Duration::ZERO,
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let window = ui.window();
            if let Some(y) =
                tcslibgs::display::top_of(edge, tcslibgs::screen_height(), window.size().height)
            {
                window.set_position(slint::PhysicalPosition::new(0, y));
            }
        },
    );
    // The timer fires once and is done; dropping it here would cancel it
    // before the loop had a chance to run it.
    std::mem::forget(timer);
}

/// How often the panels are repainted from what has arrived.
///
/// Nothing is sent to the spacecraft on this timer: what a panel shows is
/// what has arrived, which the ground has without asking anybody.
const PANEL_REFRESH_INTERVAL: Duration = Duration::from_millis(1000);

/// Which payloads the operator has turned off, one flag per panel.
///
/// Set when Disable is pressed and cleared when Enable is, so it says
/// whether the operator's last word about that payload was to stop it. What
/// reads it is the asking done when contact returns: a payload somebody
/// stopped is not a payload to go asking the spacecraft about.
pub type Declined = Arc<Mutex<Vec<bool>>>;

// Quit button handler
fn handle_quit(
    ui: &MainWindow,
    pm_tcssim: Arc<ProcessManager>,
    pm_tcspecial: Option<Arc<ProcessManager>>,
) {
    ui.on_quit_clicked(move || {
        kill_and_exit_all(&pm_tcssim, pm_tcspecial.as_ref());
    });
}

// Window close handler (close box)
fn handle_close(
    ui: &MainWindow,
    pm_tcssim: Arc<ProcessManager>,
    pm_tcspecial: Option<Arc<ProcessManager>>,
) {
    ui.window().on_close_requested(move || {
        kill_and_exit_all(&pm_tcssim, pm_tcspecial.as_ref());
        slint::CloseRequestResponse::HideWindow
    });
}

/// Stop what the MOC started, and exit.
///
/// `pm_tcspecial` is `None` when a tcspecial was already running and the MOC
/// attached to it instead of starting one. The MOC does not kill that one:
/// something else started it, something else may still be using it, and
/// shutting down a spacecraft's command interpreter because a ground display
/// was closed is not the MOC's decision to make.
/// What to do when tcspecial's beacons announce a configuration other than
/// the one this MOC read: ask the operator whether to continue anyway.
///
/// Continue closes the question and leaves everything running; Quit stops
/// tcspecial and tcssim, where this MOC started them, and exits, as the Quit
/// button does. Asked from the beacon thread, so the window is made on the
/// event loop.
fn ask_whether_to_continue(
    pm_tcssim: Arc<ProcessManager>,
    pm_tcspecial: Option<Arc<ProcessManager>>,
) -> beacon_receive::Ask {
    Arc::new(move |why: String| {
        let pm_tcssim = pm_tcssim.clone();
        let pm_tcspecial = pm_tcspecial.clone();
        let _ = slint::invoke_from_event_loop(move || {
            let Ok(window) = DiffersWindow::new() else {
                return;
            };
            window.set_message(SharedString::from(why));
            // The panels take no presses until it is answered: nothing is
            // commanded behind a question about whether to go on at all.
            main_window_inert(true);
            // The window holds a handle on itself through this callback, so
            // that it stays up until it is answered rather than until this
            // function returns.
            let held = window.clone_strong();
            window.on_continue_anyway(move || {
                main_window_inert(false);
                let _ = held.hide();
            });
            window.on_quit(move || {
                kill_and_exit_all(&pm_tcssim, pm_tcspecial.as_ref());
            });
            let _ = window.show();
        });
    })
}

thread_local! {
    /// The main window, for a question asked in a window of its own to make
    /// it inert while it waits: see `ask_whether_to_continue`. Weak, so the
    /// window is not kept alive by being remembered. On the event loop's
    /// thread, which is the only one a window may be touched from.
    static MAIN_WINDOW: std::cell::RefCell<Option<slint::Weak<MainWindow>>> =
        const { std::cell::RefCell::new(None) };
}

/// Remember the main window, so a question can make it inert. Called where
/// each run makes its window.
fn remember_main_window(ui: &MainWindow) {
    MAIN_WINDOW.with(|main| *main.borrow_mut() = Some(ui.as_weak()));
}

/// Make the main window take no presses, or take them again.
fn main_window_inert(inert: bool) {
    MAIN_WINDOW.with(|main| {
        if let Some(ui) = main.borrow().as_ref().and_then(|weak| weak.upgrade()) {
            ui.set_inert(inert);
        }
    });
}

fn kill_and_exit_all(pm_tcssim: &Arc<ProcessManager>, pm_tcspecial: Option<&Arc<ProcessManager>>) {
    pm_tcssim.kill();
    if let Some(pm_tcspecial) = pm_tcspecial {
        pm_tcspecial.kill();
    }
    exit(0);
}

/// Whether something has asked this MOC to stop.
static STOPPING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Ask the kernel to tell us about an interrupt and a termination.
///
/// Quit and the close box stop what the MOC started; a signal did not, so a
/// MOC killed from a terminal or by a service manager left its tcspecial and
/// its tcssim running -- holding the command ports, the beacon socket and
/// every payload's telemetry port, which is what the next MOC on that
/// configuration then fails to bind.
///
/// Both signals, because the two ways a program is stopped are a person at a
/// console and a service manager: one that answered only one of them would be
/// killed outright by the other.
pub(crate) fn listen_for_stop() {
    // SAFETY: the handler does one thing -- an atomic store -- which is
    // async-signal-safe. `libc::signal` is used rather than `sigaction`
    // because that is all this needs and it is what tcspecial uses.
    unsafe {
        let handler = note_the_stop as *const () as libc::sighandler_t;
        libc::signal(libc::SIGINT, handler);
        libc::signal(libc::SIGTERM, handler);
    }
}

/// What a signal does: sets a flag and returns.
///
/// Nothing else, which is the rule for a signal handler. Killing the children
/// and exiting are not things to do here -- `exit` runs whatever the process
/// registered to run at exit, and a window's event loop is not a thing to
/// unwind from a handler -- so the timer that repaints the panels notices
/// within a second and does it from the event loop.
extern "C" fn note_the_stop(_signal: libc::c_int) {
    STOPPING.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Stop what the MOC started, if a signal has asked for it.
///
/// Called from the timer that repaints the panels, which is the one thing
/// that keeps running on the event loop whatever else the window is doing. A
/// second is late enough to be unnoticed by a person and soon enough that
/// nothing else is waiting on the ports.
pub(crate) fn stop_if_asked(
    pm_tcssim: &Arc<ProcessManager>,
    pm_tcspecial: Option<&Arc<ProcessManager>>,
) {
    if STOPPING.load(std::sync::atomic::Ordering::Relaxed) {
        eprintln!("Stopping, and stopping what was started with it");
        kill_and_exit_all(pm_tcssim, pm_tcspecial);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::ffi::OsStr;
    use std::path::Path;
    use tcslibgs::config::DEFAULT_PAYLOAD_CONFIG_PATH;

    /// A panel for a source called `name`, in row `row`.
    ///
    /// Made here rather than from a configuration, because these tests are
    /// about the window: where a panel lands, how wide it is, what text it
    /// shows. What a panel is made *of* is `vehicle::panels`' business and
    /// is tested there, against the shipped examples.
    fn a_panel(row: usize, name: &str) -> DHInfo {
        DHInfo {
            name: SharedString::from(name),
            dev_id: i32::try_from(row).unwrap_or(i32::MAX),
            config: SharedString::from(format!("to moc, at 127.0.0.1:{}", 5100 + row)),
            packet_size: 0,
            status: SharedString::from(crate::vehicle::RECEIVING_STATUS),
            oc_time: SharedString::from(NO_TRANSFER_TIME),
            oc_data: SharedString::new(),
            packets_recv: 0,
            parameters: SharedString::from(format!("{name} (device id {row})")),
            settable: SharedString::new(),
            setting: SharedString::new(),
        }
    }

    /// Arguments as the program really receives them, the program's own name
    /// first.
    fn args(rest: &[&str]) -> std::vec::IntoIter<String> {
        let mut all = vec!["tcsmoc".to_string()];
        all.extend(rest.iter().map(|s| s.to_string()));
        all.into_iter()
    }

    /// The room the window opens with, which is a rule rather than a floor.
    ///
    /// Three rows are looked ahead for, so a file that grows a row can be
    /// watched without resizing the window; but a file with fewer panels than
    /// that opens only as tall as the panels it has, rather than with room
    /// below that nothing could fill. A grid taller than three rows is given
    /// all of them.
    #[test]
    fn the_window_keeps_room_for_the_rows_there_are_to_fill() {
        // One and two handlers: as tall as they need and no taller.
        assert_eq!(rows_of_room(1, 1), 1);
        assert_eq!(rows_of_room(2, 2), 2);

        // Three or more: three rows of room, whatever the grid came to.
        assert_eq!(rows_of_room(1, 3), 3);
        assert_eq!(rows_of_room(2, 4), 3);
        assert_eq!(rows_of_room(3, 9), 3);

        // And never fewer rows than the grid actually has.
        assert_eq!(rows_of_room(4, 16), 4);
        assert_eq!(rows_of_room(8, 64), 8);

        // An empty tcspecial configuration still opens a window.
        assert_eq!(rows_of_room(1, 0), 1);
    }

    /// Every panel, and each of the three rows of data it ends with, is
    /// inside the window the MOC opens for the shipped configuration.
    ///
    /// This is the test for the panels running off the bottom of the window,
    /// and it is also where `PANEL_HEIGHT` and `CHROME_HEIGHT` come from.
    /// They used to be estimates, and were 49 and 74 pixels short: each panel
    /// was given 210 of the 260 its contents take, the window was opened for
    /// a chrome of 272 that really wants 346, and what fell off the bottom
    /// was the last rows of the lowest panels -- the two sample lines and the
    /// byte counters, which is to say the data.
    ///
    /// The window is opened here as main() opens it, through Slint's testing
    /// backend: no display and no event loop, but a real layout with real
    /// font metrics, so what this measures is what the window does. A panel
    /// that is scrolled out of sight is not merely off-screen, it is never
    /// built, so the count of panels found is itself part of the check.
    /// Every panel of a row is the same width, whatever its payload is.
    ///
    /// Measured, because it was not: the three texts of the sent line used to
    /// be behind `if triggered`, a layout whose children come and go is
    /// measured differently from one whose children are always there, and the
    /// two payloads of set 2 came out 196 and 458 wide -- the triggered one
    /// taking every pixel of slack in the row. The widths are the one thing a
    /// panel's kind must not change.
    ///
    /// Not a test of its own: one test per binary may start the testing
    /// backend, and its windows belong to the thread that made them.
    fn every_panel_in_a_row_is_the_same_width() {
        use i_slint_backend_testing::ElementHandle;

        // A row of two and a row of three, which are the rows a window has:
        // the rule is about the width every panel of a row is given, so it
        // has to hold whatever the row holds.
        //
        // The panels need not differ in kind: a panel has one shape, there
        // being nothing in a node that adds a line to it.
        for (what, names) in [
            ("a row of two", vec!["k_1", "k_2"]),
            ("a row of three", vec!["k_1", "k_2", "cabin_temp"]),
        ] {
            let rows: Vec<DHInfo> = names
                .iter()
                .enumerate()
                .map(|(row, name)| a_panel(row, name))
                .collect();
            let panels = rows.len();

            let ui = MainWindow::new().unwrap();

            let _ = Sizes::settle(&ui);
            ui.set_dh_model(ModelRc::from(Rc::new(VecModel::from(rows))));
            let sizes = Sizes::of(&ui);
            let shape = grid_shape(&sizes, panels);
            ui.window().set_size(window_size(&sizes, &shape, panels));
            ui.show().unwrap();

            let widths: Vec<f32> = ElementHandle::find_by_element_type_name(&ui, "DHPanel")
                .map(|panel| panel.size().width)
                .collect();
            assert_eq!(
                widths.len(),
                panels,
                "{what}: {} panels found",
                widths.len()
            );
            assert!(
                widths.iter().all(|w| *w == widths[0]),
                "{what}: the panels are {widths:?} wide"
            );
        }
    }

    /// The panels are laid out across the window before they are laid out
    /// down it.
    ///
    /// Measured in a real layout rather than read off the grid arithmetic,
    /// because the arithmetic was never the part that was wrong: the row and
    /// column each panel is given have always been row-major, and what sent
    /// two panels down the window instead of across it was the shape chosen
    /// for them -- one column of two, because that is the squarer window.
    /// Only a laid-out window shows the two together.
    ///
    /// Not a test of its own: one test per binary may start the testing
    /// backend, and its windows belong to the thread that made them.
    fn panels_fill_left_to_right() {
        use i_slint_backend_testing::ElementHandle;

        for panels in [2usize, 3, 4, 5] {
            let rows: Vec<DHInfo> = (0..panels).map(|i| a_panel(i, &format!("DH{i}"))).collect();

            let ui = MainWindow::new().unwrap();

            let _ = Sizes::settle(&ui);
            ui.set_dh_model(ModelRc::from(Rc::new(VecModel::from(rows))));
            let sizes = Sizes::of(&ui);
            let shape = grid_shape(&sizes, panels);
            ui.window().set_size(window_size(&sizes, &shape, panels));
            ui.show().unwrap();

            // One per panel, in the model's order: the name is the first text
            // of each panel and nothing else shows it.
            let mut placed: Vec<(String, f32, f32)> = Vec::new();
            for i in 0..panels {
                // As the panel writes it: the handler's name and its number,
                // which is its place in the configuration.
                let name = format!("DH{i} (dev id {i})");
                let handle = ElementHandle::find_by_accessible_label(&ui, &name)
                    .next()
                    .unwrap_or_else(|| panic!("{panels} panels: no panel labelled {name}"));
                let at = handle.absolute_position();
                placed.push((name, at.x, at.y));
            }

            // The second panel is beside the first and never under it,
            // which is the complaint this was written for. True of every
            // count: a grid with no more rows than columns and room for two
            // panels has at least two columns.
            assert!(
                placed[1].1 > placed[0].1 && placed[1].2 == placed[0].2,
                "{panels} panels in a {}x{} grid: {} at ({}, {}) is not beside \
                 {} at ({}, {})",
                shape.columns,
                shape.rows,
                placed[1].0,
                placed[1].1,
                placed[1].2,
                placed[0].0,
                placed[0].1,
                placed[0].2
            );

            // Across first: each panel but the first of a row is to the
            // right of the one before it and level with it, and each panel
            // that starts a row is back at the first column and below.
            for i in 1..panels {
                let (name, x, y) = &placed[i];
                let (before, before_x, before_y) = &placed[i - 1];
                let placement = format!(
                    "{panels} panels in a {}x{} grid: {name} at ({x}, {y}) \
                     after {before} at ({before_x}, {before_y})",
                    shape.columns, shape.rows
                );

                if i % shape.columns == 0 {
                    assert_eq!(*x, placed[0].1, "{placement}: a new row starts over");
                    assert!(y > before_y, "{placement}: a new row is below");
                } else {
                    assert!(x > before_x, "{placement}: the next panel is to the right");
                    assert_eq!(y, before_y, "{placement}: the same row is level");
                }
            }

            // And the first row is filled before there is a second at all.
            assert_eq!(
                shape.columns.min(panels),
                placed.iter().filter(|(_, _, y)| *y == placed[0].2).count(),
                "{panels} panels in a {}x{} grid: the first row is not full",
                shape.columns,
                shape.rows
            );
        }
    }

    #[test]
    fn every_panels_data_is_inside_the_window() {
        use i_slint_backend_testing as testing;
        use testing::ElementHandle;

        testing::init_integration_test_with_system_time();

        // The shipped fixture, through the builder the window really uses:
        // see `vehicle::panels`. A panel's height is what this measures, so
        // the panels have to be real ones.
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(DEFAULT_PAYLOAD_CONFIG_PATH);
        let graph = tcslibgs::graph::Graph::load(&path)
            .unwrap_or_else(|e| panic!("{} failed to load: {e}", path.display()));
        let links = tcslibgs::graph::telemetry::of(&graph).expect("its links to the ground");
        let shipped = crate::vehicle::panels(&links, &crate::vehicle::settable_of(&graph));
        assert!(!shipped.is_empty(), "{} has no panels", path.display());

        // More handlers than the three-row floor covers, as well as the
        // shipped file. Both are needed: the floor leaves the shipped four
        // rows to spare, so only a grid taller than the floor can show
        // whether the panel and chrome heights are right.
        let crowded: Vec<DHInfo> = std::iter::repeat_with(|| shipped[0].clone())
            .take(16)
            .collect();

        for (what, rows) in [
            (format!("{}", path.display()), shipped),
            ("sixteen handlers".to_string(), crowded),
        ] {
            let panels = rows.len();

            let ui = MainWindow::new().unwrap();

            let _ = Sizes::settle(&ui);
            ui.set_dh_model(ModelRc::from(Rc::new(VecModel::from(rows))));

            // Opened as main() opens it, room rule and all.
            let sizes = Sizes::of(&ui);
            let shape = grid_shape(&sizes, panels);
            ui.window().set_size(window_size(&sizes, &shape, panels));
            ui.show().unwrap();

            let window = ui.window().size().to_logical(1.0);
            let window_height = window.height;

            // Every button is inside the window across, which is what keeps a
            // label from running off the side of its panel: a Button reports
            // the width it was given, where a Text that does not fit is
            // clipped and reports the width it had.
            for button in ElementHandle::find_by_element_type_name(&ui, "Button") {
                let right = button.absolute_position().x + button.size().width;
                assert!(
                    right <= window.width,
                    "{what}: the {:?} button reaches {right}, past the {} the \
                     window is wide",
                    button.accessible_label(),
                    window.width
                );
            }

            // And inside the panel it belongs to, which is the stricter half
            // of the same thing: a row of buttons wider than PANEL_WIDTH
            // keeps its buttons at their own width and hangs them over the
            // panel's edge, where they land on the panel beside it. The
            // window is wide enough for that to happen with nothing running
            // off its own edge, so this is what notices -- spelling Config
            // out as Configuration wanted 41 pixels the panel had not got.
            // Across and down both, which it did not used to be. A button
            // was matched to a panel by its left edge alone, and the Quit
            // button -- which is in no panel, below the grid -- sat in the
            // column of the last one: it passed while the window happened to
            // be narrow enough for its right edge to fall inside that column,
            // and failed the moment the window got wider.
            let panel_rects: Vec<(f32, f32, f32, f32)> =
                ElementHandle::find_by_element_type_name(&ui, "DHPanel")
                    .map(|panel| {
                        let left = panel.absolute_position().x;
                        let top = panel.absolute_position().y;
                        (
                            left,
                            left + panel.size().width,
                            top,
                            top + panel.size().height,
                        )
                    })
                    .collect();
            for button in ElementHandle::find_by_element_type_name(&ui, "Button") {
                let left = button.absolute_position().x;
                let right = left + button.size().width;
                let top = button.absolute_position().y;
                // The command interpreter's own buttons, and Quit, are in no
                // panel.
                let Some(&(panel_left, panel_right, _, _)) =
                    panel_rects
                        .iter()
                        .find(|(left_edge, right_edge, top_edge, bottom_edge)| {
                            left >= *left_edge
                                && left < *right_edge
                                && top >= *top_edge
                                && top < *bottom_edge
                        })
                else {
                    continue;
                };
                assert!(
                    right <= panel_right,
                    "{what}: the {:?} button runs from {left} to {right}, past the \
                     panel that holds it, which ends at {panel_right} having \
                     started at {panel_left}",
                    button.accessible_label(),
                );
            }

            // The grid sits at the top of the area it is given and fits
            // inside it. Both halves matter: a grid shorter than its area is
            // centred in it, which is where the gap under the Data Handlers
            // heading came from, and a grid taller than its area is one whose
            // lowest panels are below the window.
            let area = ElementHandle::find_by_element_id(&ui, "MainWindow::scroll")
                .next()
                .expect("the window has a scrolling area for the panels");
            let grid = ElementHandle::find_by_element_id(&ui, "MainWindow::grid")
                .next()
                .expect("the window has a grid of panels");
            assert_eq!(
                grid.absolute_position().y,
                area.absolute_position().y,
                "{what}: the grid does not start at the top of its area"
            );
            assert!(
                grid.size().height <= area.size().height,
                "{what}: the grid is {} tall in an area {} tall",
                grid.size().height,
                area.size().height
            );
            // And across, which was never checked and was wrong: the window's
            // width allowed for the padding round the grid but not for the
            // gaps between its columns, so a three-column grid stood
            // twenty-six pixels wider than the area it scrolled in and the
            // rightmost column was cut off along its edge.
            assert!(
                grid.size().width <= area.size().width,
                "{what}: the grid is {} wide in an area {} wide",
                grid.size().width,
                area.size().width
            );

            // The fixture the shape tests reason about is the window's own.
            the_markup_reports_the_sizes_the_tests_assume(&ui);

            // A panel is the size it asked for, and its rows are the sizes
            // they asked for.
            //
            // This is the fault the sizes being written down twice produced
            // here. Every row of a panel states a floor rather than a height,
            // and the grid took the whole of the scrolling area and stretched
            // its rows to fill it: a panel came out 275 tall where it wants
            // 162, and the rows shared out the surplus until a Button stood
            // fifty pixels tall. tcssim had the same disease the other way
            // round -- its rows state heights, so the surplus could only
            // gather at the bottom, which showed as a band under the packet
            // count.
            for (n, panel) in ElementHandle::find_by_element_type_name(&ui, "DHPanel").enumerate() {
                assert!(
                    (panel.size().height - sizes.panel_height).abs() <= 1.0,
                    "{what}: panel {n} is {} tall where a panel wants {}",
                    panel.size().height,
                    sizes.panel_height
                );
            }
            for button in ElementHandle::find_by_element_type_name(&ui, "Button") {
                assert!(
                    button.size().height <= 40.0,
                    "{what}: the {:?} button is {} tall, which is a stretched \
                     row rather than a button",
                    button.accessible_label(),
                    button.size().height
                );
            }

            // The grid opens with the columns asked for, and reflows when
            // the window is resized.
            //
            // The count is the window's own, worked out from the width it has
            // rather than handed over once at startup, so a window dragged
            // wider fits another column and uses it. Counted by how many
            // panels share the top row.
            //
            // Last in the case, because it leaves the window a different size
            // than it was opened at.
            let columns_now = |ui: &MainWindow| -> usize {
                let tops: Vec<f32> = ElementHandle::find_by_element_type_name(ui, "DHPanel")
                    .map(|e| e.absolute_position().y)
                    .collect();
                let first = tops.iter().copied().fold(f32::MAX, f32::min);
                tops.iter().filter(|y| (**y - first).abs() < 1.0).count()
            };

            let opened = columns_now(&ui);
            assert_eq!(
                opened,
                columns_for(panels),
                "{what}: opened with {opened} columns where {} were asked for",
                columns_for(panels)
            );

            if panels >= 2 {
                let narrow = sizes.width_for_columns(1);
                ui.window()
                    .set_size(LogicalSize::new(narrow, window_height));
                assert_eq!(
                    columns_now(&ui),
                    1,
                    "{what}: a window one panel wide did not come down to one column"
                );

                let wide = sizes.width_for_columns(panels);
                ui.window().set_size(LogicalSize::new(wide, window_height));
                assert_eq!(
                    columns_now(&ui),
                    panels,
                    "{what}: a window {panels} panels wide did not use the room"
                );
            }

            // Room for as many rows as the rule says, and no more than
            // that: a configuration of fewer panels than the window looks
            // ahead for opens only as tall as those panels need.
            let rows = rows_of_room(shape.rows, panels);
            let wanted = height_for_rows(&sizes, rows) - sizes.chrome_height;
            assert!(
                area.size().height >= wanted,
                "{what}: the panels have {} of the {wanted} that {rows} rows \
                 need",
                area.size().height
            );
            assert!(
                area.size().height < wanted + sizes.panel_height,
                "{what}: the panels have {}, room for a row more than the \
                 {rows} wanted",
                area.size().height
            );

            // Every panel, then its buttons and the rows of data it ends
            // with: those are what go off the bottom of a panel given less
            // height than it needs.
            for found_what in [
                "DHPanel",
                "DHPanel::commands-row",
                "DHPanel::last-recv-row",
                "DHPanel::from-payload-row",
                "DHPanel::counters-row",
            ] {
                let found: Vec<_> = if found_what.contains("::") {
                    ElementHandle::find_by_element_id(&ui, found_what).collect()
                } else {
                    ElementHandle::find_by_element_type_name(&ui, found_what).collect()
                };

                assert_eq!(
                    found.len(),
                    panels,
                    "{what}: {} of {panels} {found_what} were built, so the \
                     rest are out of sight",
                    found.len()
                );

                // Against the bottom of the area the panels live in,
                // which is where they are cut off, rather than the bottom of
                // the window: below the area is the row with Quit in it.
                let area_bottom = area.absolute_position().y + area.size().height;
                for (n, element) in found.iter().enumerate() {
                    let bottom = element.absolute_position().y + element.size().height;
                    assert!(
                        bottom <= area_bottom && bottom <= window_height,
                        "{what}: {found_what} {n} ends {bottom} down, past the \
                         {area_bottom} the panels have in a window \
                         {window_height} tall"
                    );
                }
            }
        }

        // The backend is up and this thread owns its windows, so the other
        // checks that need one run from here.
        the_window_shows_when_the_last_beacon_arrived();
        every_panel_shows_what_arrived_from_its_payload();
        panels_fill_left_to_right();
        every_panel_in_a_row_is_the_same_width();
        crate::vehicle::tests::pressing_connect_and_enable_commands_the_vehicle();
    }

    /// The MOC listens where tcspecial sends, by reading the same files.
    ///
    /// Checked against the shipped configurations rather than a contrived
    /// pair, because what matters is that the two programs resolve one pair of
    /// addresses out of the files as shipped: the MOC binds the one beacons
    /// are received on and tcspecial sends to it. A listener that assumed an
    /// address would go deaf the moment a mission moved the beacon, and the
    /// only symptom would be an indicator saying nothing had arrived.
    #[test]
    fn the_moc_listens_where_tcspecial_sends() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let payload_path = root.join(DEFAULT_PAYLOAD_CONFIG_PATH);

        // One file says it, and one function reads it, which is what both
        // ends call: tcspecial resolves the pair the same way.
        let graph = tcslibgs::graph::Graph::load(&payload_path)
            .unwrap_or_else(|e| panic!("{} failed to load: {e}", payload_path.display()));
        let mine = tcslibgs::graph::parameters::beacon_of(&graph)
            .expect("its tcspecial section reads")
            .expect("the shipped fixture states a beacon");

        // What the file itself says, in its own words.
        let section = graph.tcspecial.as_ref().expect("a tcspecial section");
        let said = tcslibgs::graph::parameters::ci_config_of(
            &tcslibgs::graph::parameters::flatten(section),
        )
        .expect("the section reads");
        assert_eq!(mine.send.to_string(), said.beacon.send);
        assert_eq!(mine.recv.to_string(), said.beacon.recv);

        // And the two ends are two sockets. One address for both would be a
        // sender bound where the listener is, which is the listener's port
        // taken from it at startup.
        assert_ne!(
            mine.send, mine.recv,
            "the two ends of the beacon link are one"
        );
    }

    /// Every panel shows what arrived from its payload, and nothing the
    /// spacecraft merely claims.
    ///
    /// Two lines, read for two things: whether anything is still coming, and
    /// what it was. The spacecraft's own account of what it last sent and
    /// last received was shown above them and is gone -- a panel is watched
    /// for the data that reached the ground.
    ///
    /// Not a test of its own: one test per binary may start the testing
    /// backend, and its windows belong to the thread that made them.
    fn every_panel_shows_what_arrived_from_its_payload() {
        use i_slint_backend_testing::ElementHandle;

        let shown = |info: DHInfo| {
            let ui = MainWindow::new().unwrap();
            let _ = Sizes::settle(&ui);
            ui.set_dh_model(ModelRc::from(Rc::new(VecModel::from(vec![info]))));
            ui.show().unwrap();
            ElementHandle::find_by_element_type_name(&ui, "Text")
                .filter_map(|e| e.accessible_label())
                .map(|label| label.to_string())
                .collect::<Vec<_>>()
        };

        // One panel, which is every panel: a panel has one shape, there
        // being nothing in a graph that adds a line to a panel.
        {
            let kind = "sends to the ground";
            let lines = shown(a_panel(0, "DH0"));
            let count = |label: &str| lines.iter().filter(|shown| *shown == label).count();

            // The one panel's two readings of what arrived.
            for line in ["Last recv:", "From payload:"] {
                assert_eq!(
                    count(line),
                    1,
                    "a payload that {kind} has no {line} line: {lines:?}"
                );
            }

            // And the spacecraft's own account of what it last sent and
            // received, which the panels showed and do not any more. The
            // window has a line of its own saying when the last command went
            // out and when its answer came back, so these two labels appear
            // once each in the window and never in a panel: with one panel
            // on screen, one of each is the window's.
            for elsewhere in ["Last sent:", "Last rcvd:"] {
                assert_eq!(
                    count(elsewhere),
                    1,
                    "a payload that {kind} shows {elsewhere} in its panel, or \
                     the window has lost its own: {lines:?}"
                );
            }

            // The count beside them, which is how many packets arrived
            // rather than how many bytes the spacecraft read from the ground:
            // that was the other direction, and read nought for every
            // payload nobody sends to.
            let mut counted = a_panel(0, "DH0");
            counted.packets_recv = 3;
            let shown_counted = shown(counted);
            assert!(
                shown_counted.iter().any(|line| line == "Recv: 3 recv"),
                "a payload that {kind} does not show what arrived: {shown_counted:?}"
            );
            assert!(
                !shown_counted
                    .iter()
                    .any(|line| line.starts_with("Recv: ") && line.contains("bytes")),
                "a panel still counts bytes where packets are counted: {shown_counted:?}"
            );

            // And a panel that has heard nothing says nought rather than
            // nothing: the count is a count from the moment the MOC started.
            assert!(
                lines.iter().any(|line| line == "Recv: 0 recv"),
                "a payload that {kind} has no count at all: {lines:?}"
            );
        }
    }

    /// The window shows the beacon's last-received time where it says it
    /// does, and says the same thing about nothing received as the panels do.
    ///
    /// The line existed and nothing ever set it. Checking the formatting
    /// alone would not have caught that, and would not catch the next way of
    /// losing it either: the property is set by a background thread through
    /// the event loop, so nothing in Rust fails if the window stops showing
    /// it.
    ///
    /// Not a test of its own: one test per binary may start the testing
    /// backend, and its windows belong to the thread that made them.
    fn the_window_shows_when_the_last_beacon_arrived() {
        use i_slint_backend_testing::ElementHandle;

        let ui = MainWindow::new().unwrap();

        let _ = Sizes::settle(&ui);
        ui.show().unwrap();

        let on_screen = |ui: &MainWindow| -> Vec<String> {
            ElementHandle::find_by_element_type_name(ui, "Text")
                .filter_map(|e| e.accessible_label())
                .map(|label| label.to_string())
                .collect()
        };

        // Before a beacon: the placeholder every other time line uses, so the
        // window says one thing about having received nothing, and the
        // message line saying nothing has been said.
        let shown = on_screen(&ui);
        assert!(
            shown.iter().any(|line| line == NO_TRANSFER_TIME),
            "the beacon box does not say that nothing has arrived: {shown:?}"
        );
        assert!(
            shown
                .iter()
                .any(|line| line == tcslib::beacon::NO_BEACON_MESSAGE),
            "the beacon box does not say that nothing has been said: {shown:?}"
        );

        // And after one, the time it arrived and what it said. What to show
        // is tcsui's to work out, from what the beacon loop reports; what is
        // checked here is that this window shows it.
        beacon_receive::show_beacon(
            &ui,
            &tcsui::Shown::at(
                &tcslib::beacon::Schedule::standard().at(Duration::ZERO),
                Some(std::time::UNIX_EPOCH + Duration::from_secs(7322)),
                "version 0.1.0, md5 0123456789abcdef",
            ),
        );
        let shown = on_screen(&ui);
        assert!(
            shown.iter().any(|line| line == "02:02:02"),
            "the beacon's arrival time is not on screen: {shown:?}"
        );
        assert!(
            shown
                .iter()
                .any(|line| line == "version 0.1.0, md5 0123456789abcdef"),
            "what the beacon said is not on screen: {shown:?}"
        );
    }

    /// The window size the design document states is the one the rule gives.
    ///
    /// It once said 640x630 while the program opened at 700x852: the number
    /// went stale when the window gained its three-row floor and the width
    /// floor the command interpreter's controls need, and nothing noticed
    /// because nothing read it. What the program opens at now is whatever the
    /// rule gives, and this is what makes the document say the same.
    #[test]
    fn the_design_document_states_the_size_the_window_opens_at() {
        let doc = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("docs/design.rst");
        let text = std::fs::read_to_string(&doc).unwrap();

        // The four handlers of tcspecial1.yaml, which is the set the document
        // describes.
        let shape = grid_shape(&Sizes::for_tests(), 4);
        let size = window_size(&Sizes::for_tests(), &shape, 4);
        let said = format!("{}x{}", size.width, size.height);

        assert_eq!(
            (shape.columns, shape.rows),
            (3, 2),
            "the document describes a three-by-two grid"
        );
        assert!(
            text.contains(&said),
            "{} does not say the window opens at {said}",
            doc.display()
        );
    }

    /// The markup reports the sizes the tests do their arithmetic on.
    ///
    /// There used to be a test here comparing three numbers in
    /// `ui/main.slint` against three constants in this file, by reading the
    /// markup as text and looking for "panel-height: 180px;". It was the right
    /// idea and it could only ever catch the copies going out of step, not
    /// their being wrong together -- which they were: a panel is 162 tall and
    /// 255 wide, and both files said otherwise in the same words.
    ///
    /// There is nothing to compare now. The markup measures a panel and states
    /// the chrome, Rust reads both, and the only number written twice is the
    /// fixture the tests below do arithmetic on. This is what keeps that
    /// honest. A panel that gains a row is meant to fail here.
    ///
    /// Not a test of its own: the Slint backend belongs to one thread, so the
    /// checks that need a window run from the one that starts it.
    fn the_markup_reports_the_sizes_the_tests_assume(ui: &MainWindow) {
        assert_eq!(
            Sizes::of(ui),
            Sizes::for_tests(),
            "the window reports sizes the tests were written against other \
             numbers for"
        );
    }

    /// The status line in the window and the strings Rust sets it to.
    ///
    /// Slint does the comparison that colours the line, so nothing in Rust
    /// fails if the two files drift apart: the line would quietly stop
    /// turning green on a connection, or come up saying something other than
    /// what no connection means. Checked against the window's own source
    /// rather than left to a comment.
    #[test]
    fn the_window_and_rust_agree_on_the_status_strings() {
        let window = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/main.slint");
        let source = std::fs::read_to_string(&window).unwrap();

        assert!(
            source.contains(&format!("ci-status: \"{}\"", DISCONNECTED_STATUS)),
            "{} does not come up saying {:?}",
            window.display(),
            DISCONNECTED_STATUS
        );
        assert!(
            source.contains(&format!("ci-status == \"{}\"", CONNECTED_STATUS)),
            "{} does not colour the line on {:?}",
            window.display(),
            CONNECTED_STATUS
        );

        // The one button offers the opposite of the status it reads, and a
        // button offering Connect on a link that is up would be read as a
        // link that is down.
        let offered = format!(
            "ci-status == \"{}\" ? \"Disconnect\" : \"Connect\"",
            CONNECTED_STATUS
        );
        assert!(
            source.contains(&offered),
            "{} does not label its link button {:?}",
            window.display(),
            offered
        );
    }

    /// What a panel's two buttons say, and what each sends.
    ///
    /// Here rather than beside the statuses because the window owns its
    /// labels: Slint builds the strings, and these are what the test below
    /// holds them to.
    const ENABLE_LABEL: &str = "Enable";
    const DISABLE_LABEL: &str = "Disable";

    /// Each panel has a button per command, each saying what it sends.
    ///
    /// There was one button whose label followed the status -- the one thing
    /// worth doing next -- so a press had to be decided by reading that
    /// status back, and Slint's label and Rust's choice had to be held to one
    /// rule or they would drift into offering one thing and doing the other.
    /// Two buttons have nothing to drift: a press is the command on the
    /// label. What this holds is that the window still has both, with Enable
    /// first, and that each is wired to its own callback.
    #[test]
    fn a_panel_has_a_button_for_each_command() {
        let window = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/main.slint");
        let source = std::fs::read_to_string(&window).unwrap();

        for (label, callback) in [
            (ENABLE_LABEL, "enable-clicked()"),
            (DISABLE_LABEL, "disable-clicked()"),
        ] {
            let button = format!("text: \"{label}\";");
            assert!(
                source.contains(&button),
                "{} has no button saying {label}",
                window.display()
            );
            assert!(
                source.contains(callback),
                "{} does not wire {label} to {callback}",
                window.display()
            );
        }

        // Enable before Disable, which is the order a panel is read in: the
        // press a panel is opened to make comes first.
        let enable = source.find(&format!("text: \"{ENABLE_LABEL}\";")).unwrap();
        let disable = source.find(&format!("text: \"{DISABLE_LABEL}\";")).unwrap();
        assert!(enable < disable, "Disable is offered before Enable");

        // And the one button they replaced is gone, label and callback both.
        for gone in ["\"Receive\"", "\"Discard\"", "transfer-clicked"] {
            assert!(
                !source.contains(gone),
                "{} still has the one-button wiring: {gone}",
                window.display()
            );
        }
    }

    /// shown as having brought nothing.
    #[test]
    fn a_panel_counts_what_arrived_at_the_ground() {
        use crate::oc_link::Arrival;
        use tcslibgs::{DHSample, Timestamp};
        let model: Rc<VecModel<DHInfo>> =
            Rc::new(VecModel::from(vec![a_panel(0, "DH0"), a_panel(1, "DH1")]));

        let mut data = DHSample::new();
        data.record(&[0xDE, 0xAD]);
        let arrival = Arrival {
            at: Timestamp::at(std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(3661)),
            data,
            packets: 5,
            sends_values: false,
        };
        let arrivals: Arrivals = Arc::new(Mutex::new(vec![None, Some(arrival)]));

        show_arrivals(&model, &arrivals);

        let shown = model.row_data(1).unwrap();
        assert_eq!(shown.packets_recv, 5, "the count the ground kept");
        assert_eq!(shown.oc_time, SharedString::from("01:01:01"));
        assert!(
            shown.oc_data.contains("DE") && shown.oc_data.contains("AD"),
            "what arrived: {}",
            shown.oc_data
        );

        // The payload nothing has arrived from keeps its placeholders and its
        // nought: a slot with nothing in it is a payload that has sent
        // nothing, not a reading to be guessed at.
        let quiet = model.row_data(0).unwrap();
        assert_eq!(quiet.packets_recv, 0);
        assert_eq!(quiet.oc_time, SharedString::from(NO_TRANSFER_TIME));
    }

    /// A fresh panel says nothing has arrived from its payload yet.
    ///
    /// The placeholder every other time in the window uses, so a panel that
    /// has heard nothing and one whose clock read zero do not look alike.
    #[test]
    fn a_fresh_panel_has_heard_nothing_from_its_payload() {
        let info = a_panel(0, "DH0");
        assert_eq!(info.oc_time, SharedString::from(NO_TRANSFER_TIME));
        assert!(info.oc_data.is_empty());
    }

    /// Each child is started on the tcspecial configuration the MOC read, and
    /// told so by argument.
    ///
    /// This is what was previously only ever confirmed by running the MOC and
    /// reading what its children printed.
    #[test]
    fn each_child_is_started_on_the_mocs_payload_file() {
        for name in CHILDREN {
            let command = child_command(name, "tcspecial2.yaml");

            assert_eq!(command.get_program(), "cargo", "{name}");
            let args: Vec<&OsStr> = command.get_args().collect();
            assert_eq!(
                args,
                ["run", "--bin", name, "--", "tcspecial2.yaml"],
                "{name}"
            );

            // Nothing is put in the child's environment: the argument says it
            // all, and a variable set here would reach whatever that child
            // starts in turn.
            let envs: Vec<(&OsStr, Option<&OsStr>)> = command.get_envs().collect();
            assert!(envs.is_empty(), "{name} was given {envs:?}");
        }
    }

    /// The file a child is started on is the one named on the MOC's own
    /// command line.
    #[test]
    fn the_children_get_the_file_the_command_line_named() {
        let payload_path = payload_path_from_args(args(&["tcspecial2.yaml"]), None).unwrap();

        for name in CHILDREN {
            let command = child_command(name, &payload_path);
            let args: Vec<&OsStr> = command.get_args().collect();
            assert_eq!(
                args.last(),
                Some(&OsStr::new("tcspecial2.yaml")),
                "{name} was not given the file the command line named"
            );
        }
    }

    /// With no argument, the children are started on the same default the MOC
    /// itself reads, so an unconfigured run has all three on one payload set.
    #[test]
    fn with_no_argument_every_program_is_on_the_same_default() {
        let payload_path = payload_path_from_args(args(&[]), None).unwrap();
        assert_eq!(payload_path, DEFAULT_PAYLOAD_CONFIG_PATH);

        for name in CHILDREN {
            let command = child_command(name, &payload_path);
            let args: Vec<&OsStr> = command.get_args().collect();
            assert_eq!(
                args.last(),
                Some(&OsStr::new(DEFAULT_PAYLOAD_CONFIG_PATH)),
                "{name}"
            );
        }
    }

    /// The MOC starts the two programs it is meant to.
    ///
    /// Named here rather than read back out of `CHILDREN`, because every other
    /// test in this file iterates that list and so would agree with it however
    /// it were wrong.
    #[test]
    fn the_moc_starts_tcspecial_and_tcssim() {
        assert_eq!(CHILDREN, ["tcspecial", "tcssim"]);
    }

    /// The panels and the sources line up by index, which is what lets a
    /// panel's row number name the node it is about.
    #[test]
    fn a_panel_row_names_the_source_at_that_index() {
        // The shipped fixture, through the builder the window uses: the
        // panels are its sources in device id order, which is what the rows
        // are counted by.
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(DEFAULT_PAYLOAD_CONFIG_PATH);
        let graph = tcslibgs::graph::Graph::load(&path)
            .unwrap_or_else(|e| panic!("{} failed to load: {e}", path.display()));
        let links = tcslibgs::graph::telemetry::of(&graph).expect("its links to the ground");
        let model = crate::vehicle::panels(&links, &crate::vehicle::settable_of(&graph));

        assert!(model.len() > 1, "the fixture has {} panels", model.len());

        // Row 1 is the second source, whatever it is called, and the number
        // beside it is the node's own device id rather than the row: a
        // command names the id, and a panel that showed the row would name
        // the wrong node the moment a source was left out of the window.
        let second = links
            .iter()
            .min_by_key(|link| link.id)
            .map(|first| first.id)
            .expect("at least one link");
        assert_eq!(model[1].dev_id, i32::try_from(second + 1).unwrap());
        assert_eq!(
            model[1].name,
            SharedString::from(
                links
                    .iter()
                    .find(|link| link.id == second + 1)
                    .expect("the second source")
                    .source
                    .clone()
            )
        );
    }

    /// How many panel counts to check the grid rules against. Well past any
    /// plausible payload configuration, so a rule that holds only for small
    /// counts does not pass.
    const COUNTS: usize = 64;

    /// A row holds up to three panels, and the next row starts under it.
    ///
    /// Checked for every panel count rather than for the shipped one: a
    /// handler added to the configuration file must not change the shape of
    /// the rows above it.
    #[test]
    fn a_row_holds_up_to_three_panels() {
        for panels in 1..=COUNTS {
            // The shape rule and not the sizes, so there is no window
            // to ask: how many columns a count gets does not depend on
            // what a panel comes to.
            let shape = grid_shape(&Sizes::for_tests(), panels);

            assert_eq!(
                shape.columns,
                panels.min(MAX_COLUMNS),
                "{panels} panels went into {} columns",
                shape.columns
            );
            assert_eq!(
                shape.rows,
                panels.div_ceil(shape.columns),
                "{panels} panels in {} columns went into {} rows",
                shape.columns,
                shape.rows
            );
        }
    }

    /// The grid is wider than it is tall while it fits two rows.
    ///
    /// Six panels, which is every shipped set and then some. A third row is
    /// 852 against a 700-wide window, so past six the grid is taller than wide
    /// and the panels scroll -- as too many panels always have. The window the
    /// MOC *opens* has been taller than wide since it gained its three-row
    /// floor, which is a different thing: the floor is room to grow into.
    #[test]
    fn the_grid_is_wider_than_tall_while_it_fits_two_rows() {
        for panels in 1..=(MAX_COLUMNS * 2) {
            // The shape rule and not the sizes, so there is no window
            // to ask: how many columns a count gets does not depend on
            // what a panel comes to.
            let shape = grid_shape(&Sizes::for_tests(), panels);
            assert!(
                shape.width >= shape.height,
                "{panels} panels gives {}x{}, taller than wide",
                shape.width,
                shape.height
            );
        }
    }

    /// Every panel has a cell, and the grid has no row to spare.
    #[test]
    fn the_grid_holds_every_panel_and_no_empty_row() {
        for panels in 1..=COUNTS {
            // The shape rule and not the sizes, so there is no window
            // to ask: how many columns a count gets does not depend on
            // what a panel comes to.
            let shape = grid_shape(&Sizes::for_tests(), panels);
            assert!(
                shape.columns * shape.rows >= panels,
                "{panels} panels do not fit in {}x{}",
                shape.columns,
                shape.rows
            );
            assert!(
                shape.columns * (shape.rows - 1) < panels,
                "{panels} panels in {}x{} leaves an empty row",
                shape.columns,
                shape.rows
            );
        }
    }

    /// An unreadable or empty configuration still produces a window.
    #[test]
    fn no_panels_still_has_a_shape() {
        let shape = grid_shape(&Sizes::for_tests(), 0);
        assert_eq!(shape.columns, 1);
        assert_eq!(shape.rows, 1);
        assert!(shape.width >= shape.height);
    }
}
