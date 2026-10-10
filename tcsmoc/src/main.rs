//! TCSpecial Mission Operations Center (tcsmoc)
//!
//! A GUI application for testing and visualizing tcspecial operation.


use slint::{LogicalSize, Model, ModelRc, SharedString, VecModel};
use std::env;
use std::process::{Child, Command, exit};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tcslib::{TcsClient, UdpConnection};
use tcslibgs::config::{
    load_config_version, load_dh_configs, load_tcspecial_section, payload_path_from_args,
};
use tcslibgs::config_digest::{digest_of_file, ConfigDigest, ConfigVersion};
use tcslibgs::{
    payload_parameters, trigger_as_written, ArmKey, CommandStatus, DHConfig, DHSample,
    ResolvedSim, SimConfigFile, NO_TRANSFER_TIME,
};
use tcspecial::config::{beacon, load_tcspecial_config, tcspecial_config_path, Beacon};

use crate::beacon_receive::BeaconReceive;
use crate::oc_link::Arrivals;
use crate::ci_link::{CiLink, NOT_CONNECTED};
use crate::endpoint::endpoint_description;
use crate::config::constants::BEACON_INDICATOR;

slint::include_modules!();

mod beacon_receive;
mod ci_link;
mod config;
mod endpoint;
mod oc_link;

/// Default CI address
const DEFAULT_CI_ADDRESS: &str = "127.0.0.1:4000";

/// What the window's status line says about the link to the command
/// interpreter.
///
/// `ui/main.slint` compares the status against "Connected" to choose the
/// colour it is shown in, and defaults it to "Disconnected"; these are the
/// same strings, named here so the two files can be checked against each
/// other.
const CONNECTED_STATUS: &str = "Connected";
const DISCONNECTED_STATUS: &str = "Disconnected";
const ERROR_STATUS: &str = "Error";

/// What a panel's status says of a handler that is moving data, and of one
/// that is not.
///
/// `ui/main.slint` labels the panel's one button from these, and
/// `handle_transfer_button` reads the same status back to decide which
/// command a press sends, so the label and the command cannot disagree.
///
/// What the button actually says each way is in `ui/main.slint`: the window
/// owns its own labels, and the two words appear in Rust only in the test
/// that holds the window to the rule a press is decided by.
const ACTIVE_STATUS: &str = "Active";
const STOPPED_STATUS: &str = "Stopped";

/// How long the MOC waits for an answer when asking whether a tcspecial is
/// already there.
///
/// Short, because this is a question about something on the same machine and
/// the answer is wanted before a window opens. A tcspecial that cannot answer
/// a ping this quickly is one the MOC would rather replace than talk to.
const ALREADY_RUNNING_TIMEOUT: Duration = Duration::from_millis(500);

/// Whether something is already answering as tcspecial at `address`.
///
/// Asked by PING rather than by looking for a process or a bound port,
/// because what matters is whether there is a command interpreter that
/// answers: a bound port might be anything, and a process by that name might
/// be wedged. This also finds a tcspecial that is not a local process at all,
/// which a process search never would.
///
/// A link that cannot even be opened counts as nothing running, not as an
/// error: the MOC's job then is to start one.
fn tcspecial_already_running(address: &str) -> bool {
    let connection = match UdpConnection::new("0.0.0.0:0", address) {
        Ok(connection) => connection,
        Err(_) => return false,
    };
    // A PING is a spacecraft command, so the payload link is never sent
    // anything here; it is given the same address because a client has two
    // links and this question needs one.
    let payload = match UdpConnection::new("0.0.0.0:0", address) {
        Ok(connection) => connection,
        Err(_) => return false,
    };

    let mut client = TcsClient::new(Box::new(connection), Box::new(payload));
    client.set_timeout(ALREADY_RUNNING_TIMEOUT);
    client.ping().is_ok()
}

/// The programs the MOC starts.
///
/// Each takes its payload configuration file as an argument, as the MOC does,
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

/// Which environment variable names the simulator configuration, and what is
/// read when the variable is unset and no file is beside the payload file.
///
/// The same variable tcssim reads, which is the point: the MOC shows what the
/// tcssim it starts is simulating, so the two have to be looking at one file.
const SIM_CONFIG_PATH_VAR: &str = "PAYLOAD_SIM_YAML";
const DEFAULT_SIM_CONFIG_PATH: &str = "tests/manual/tcspecial1sim.yaml";

/// What one column of the grid takes, and the height of everything above and
/// below the grid.
///
/// These are used only to choose how large the window opens; the grid itself
/// lays the panels out at the width they ask for. They have to agree with
/// `ui/main.slint`, which sizes the grid with the same panel height.
///
/// A panel measures 196 wide with `GRID_SPACING` beside it, which is what the
/// testing backend reports in `every_panels_data_is_inside_the_window`. This
/// was 320 -- a nominal figure, with a comment claiming the panels stretched
/// to fill whatever it gave them, which they do not. Three columns of the real
/// width fit the window floor the command interpreter's controls already set,
/// where three of 320 would have asked for 960 and left a third of the window
/// empty to the right of the panels.
const PANEL_WIDTH: f32 = 206.0;
/// What a panel's contents come to: the sum of the floors its rows declare,
/// measured through Slint's testing backend rather than guessed, by
/// `every_panels_data_is_inside_the_window`. This was 210 against the 259 the
/// panel took when its two readings each sat in a group box of their own, so
/// the window opened too small for every row and the three rows of data at
/// the bottom of each panel went over the edge. It went 162 to 180 when the
/// panels gained the line saying what the ground itself received, which is
/// the row and the spacing above it. Those readings are labelled
/// lines now, which is most of the difference.
const PANEL_HEIGHT: f32 = 180.0;
/// Everything above and below the grid: both group box headings, the command
/// interpreter's own controls, the row holding Quit, and the padding around
/// them all, and the grid's own padding inside the scrolling area. Measured
/// the same way, and the estimate it replaces was 272 against the 346 it
/// really takes.
const CHROME_HEIGHT: f32 = 346.0;

/// The gap between one panel and the next, matching `spacing` on the grid in
/// the window. The window has to open tall enough for the gaps as well as the
/// panels.
const GRID_SPACING: f32 = 10.0;

/// How many rows of panels the window looks ahead for.
///
/// Asked for rather than worked out: a window with room for only the rows a
/// particular payload file happens to have is one that has to be resized
/// before a larger file can be watched, and three rows is the room that was
/// wanted. It is not a floor, though -- see `rows_of_room`.
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
/// at; `rows_of_room` says how many rows a particular payload file opens
/// with.
fn height_for_rows(rows: usize) -> f32 {
    let rows = rows.max(1);
    rows as f32 * PANEL_HEIGHT + (rows - 1) as f32 * GRID_SPACING + CHROME_HEIGHT
}

/// The size the window opens at for a grid of `shape` holding `panels`.
fn window_size(shape: &GridShape, panels: usize) -> LogicalSize {
    LogicalSize::new(
        shape.width,
        height_for_rows(rows_of_room(shape.rows, panels)),
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
/// side by side at any width now -- a grid is never taller in panels than it
/// is wide -- so the four-panel case is what the floor holds up.) Anything that changes the panel or chrome heights has to be
/// weighed the same way, against
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
fn min_window_width() -> f32 {
    CONTROLS_WIDTH.max(height_for_rows(2))
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
fn grid_shape(panels: usize) -> GridShape {
    // An empty configuration still has to produce a window.
    let panels = panels.max(1);
    let columns = panels.min(MAX_COLUMNS);
    let rows = panels.div_ceil(columns);

    GridShape {
        columns,
        rows,
        // The floor is a floor rather than the width of one column:
        // what the command interpreter's controls need is wider than a single
        // panel, so one and two panels both open at it.
        width: (columns as f32 * PANEL_WIDTH).max(min_window_width()),
        height: height_for_rows(rows),
    }
}

/// Turn a data handler from the payload configuration file into the panel the
/// window shows for it.
///
/// The configured values fill in what the file knows; the run-time fields
/// start empty and are filled in as tcspecial reports on the handler. There is
/// no packet interval among them: how often a payload produces a packet is a
/// property of a simulation rather than of a payload, so it is stated in the
/// simulator's own file and tcssim is what shows it.
fn dh_info_from(dh: &DHConfig, sim: Option<&ResolvedSim>) -> DHInfo {
    DHInfo {
        name: SharedString::from(dh.name.0.clone()),
        config: SharedString::from(endpoint_description(&dh.endpoint)),
        packet_size: i32::try_from(dh.packet_size).unwrap_or(i32::MAX),
        status: SharedString::from("Stopped"),
        last_sent_time: SharedString::from(NO_TRANSFER_TIME),
        last_sent: SharedString::new(),
        last_recv_time: SharedString::from(NO_TRANSFER_TIME),
        last_recv: SharedString::new(),
        // What the ground has received from this payload, which is nothing
        // until it has.
        oc_time: SharedString::from(NO_TRANSFER_TIME),
        oc_data: SharedString::new(),
        bytes_sent: 0,
        bytes_recv: 0,
        // What both files said about this payload, ready for the panel's
        // Configuration button. `sim` is absent when the simulator file could not be
        // read, which is not an error here: the MOC controls payloads and
        // does not simulate them.
        parameters: SharedString::from(payload_parameters(dh, sim)),
    }
}

/// What the simulator configuration settles for each handler, if it can be
/// read.
///
/// Read so that a panel can show the whole of what a payload set says, and
/// read loosely for the same reason it is read at all: the MOC does not
/// simulate anything, so a simulator file that is missing, unreadable, or
/// written for a different payload file must not stop the MOC from
/// controlling payloads. Each of those is said once on the way past and the
/// panels then say, where the parameters are shown, that this half was not
/// read.
fn simulator_settings(payload_path: &str, dh_configs: &[DHConfig]) -> Option<Vec<ResolvedSim>> {
    let sim_path = env::var(SIM_CONFIG_PATH_VAR).unwrap_or_else(|_| {
        // Beside the payload file and named for it, which is the convention
        // the whole payload set mechanism rests on.
        payload_path
            .strip_suffix(".yaml")
            .map(|stem| format!("{stem}sim.yaml"))
            .unwrap_or_else(|| DEFAULT_SIM_CONFIG_PATH.to_string())
    });

    let file = match SimConfigFile::load(&sim_path) {
        Ok(file) => file,
        Err(e) => {
            eprintln!(
                "Not showing simulator settings: {} could not be read: {}",
                sim_path, e
            );
            return None;
        }
    };

    match file.resolve(dh_configs) {
        Ok(sims) => {
            eprintln!("Loaded simulator settings from: {}", sim_path);
            Some(sims)
        }
        Err(e) => {
            eprintln!(
                "Not showing simulator settings: {} does not fit {}: {}",
                sim_path, payload_path, e
            );
            None
        }
    }
}

/// What one panel shows, gathered from a handler.
///
/// Plain values rather than a model row, because this crosses a thread: the
/// asking happens away from the event loop, where blocking is allowed, and
/// only the applying happens on it. A Slint model is not `Send`, and the
/// client's commands wait on the spacecraft for as long as its timeout
/// allows, so doing both in one place would mean a window that stops
/// repainting whenever tcspecial is slow to answer.
#[derive(Clone)]
struct PanelUpdate {
    row: usize,
    bytes_sent: i32,
    bytes_recv: i32,
    last_sent_time: SharedString,
    last_sent: SharedString,
    last_recv_time: SharedString,
    last_recv: SharedString,
}

/// Ask one handler what its panel should show.
///
/// `None` when it did not answer at all. A handler that answers the statistics
/// but not the samples keeps the sample lines it had: the last thing seen is
/// better than nothing seen, and a blank line reads as no data rather than no
/// answer.
/// One line about a payload that has to be asked for its data, or nothing at
/// all about one that does not.
///
/// A triggered payload is the one worth watching this closely. It sends
/// nothing until tcspecial asks it to, so when a panel shows no data the
/// question is always what the MOC sent and what came back -- and neither of
/// those appears anywhere: the window shows a payload's state and not the
/// traffic that settled it.
///
/// Said on stderr, as the startup messages in `main` are, so it appears on the
/// console of whoever started the MOC whether or not they set `RUST_LOG`.
fn triggered_note(dh: &DHConfig, what: &str) -> Option<String> {
    dh.mode.polling()?;
    Some(format!("triggered payload {}: {}", dh.name.0, what))
}

/// Say it, if this is a payload there is anything to say about.
///
/// For the things that happen once: a button pressed, an answer to it. Two
/// presses are two lines even when they say the same thing, because they are
/// two presses.
fn note_triggered(dh: &DHConfig, what: String) {
    if let Some(line) = triggered_note(dh, &what) {
        eprintln!("{line}");
    }
}

/// Say it only if it is not what was last said about this payload.
///
/// For the things that are said over and over: the panels refresh about once a
/// second whether or not anything is happening, so a note on every pass buried
/// what moved under a hundred identical lines. `said` is what was last said
/// about this payload, kept by the caller -- one per panel -- so a line appears
/// when something behind it changed and not again until it changes once more.
///
/// Returns whether anything was said, which is what makes the rule testable: a
/// console is not somewhere a test can read.
fn note_triggered_if_new(dh: &DHConfig, what: String, said: &mut Option<String>) -> bool {
    let line = match triggered_note(dh, &what) {
        Some(line) => line,
        // Nothing to say, and nothing to remember either: `said` belongs to
        // the payload, and a payload nothing is said about has no last line.
        None => return false,
    };

    if said.as_deref() == Some(line.as_str()) {
        return false;
    }

    eprintln!("{line}");
    *said = Some(line);
    true
}

/// What the MOC starts out believing about a triggered payload.
///
/// The whole of what it will address the payload by and ask it with: the id
/// every command carries, where the payload and the OC side are, and the
/// trigger and interval that are tcspecial's to send. A panel shows the first
/// of those and none of the rest, so a session where the MOC and tcspecial
/// disagreed about a payload had nothing on the console to compare.
fn triggered_startup(dh: &DHConfig, status: &str) -> Option<String> {
    let (trigger, interval_ms) = dh.mode.polling()?;
    triggered_note(
        dh,
        &format!(
            "dh_id {}, {}, trigger {} every {} ms, OC {}, status {}",
            dh.dh_id.0,
            endpoint_description(&dh.endpoint),
            trigger_as_written(trigger),
            interval_ms,
            oc_of(dh),
            status
        ),
    )
}

/// Where a handler's OC side is, or that it has not got one.
fn oc_of(dh: &DHConfig) -> String {
    match &dh.oc {
        Some(oc) => format!("{}:{}", oc.address, oc.port),
        None => "none".to_string(),
    }
}

fn panel_update_for(
    client: &mut TcsClient,
    row: usize,
    dh: &DHConfig,
    previous: Option<&PanelUpdate>,
    said: &mut Option<String>,
) -> Option<PanelUpdate> {
    let (status, stats) = match client.query_dh(dh.dh_id) {
        Ok(answer) => answer,
        Err(e) => {
            note_triggered_if_new(dh, format!("QUERY_DH got no answer: {e}"), said);
            return None;
        }
    };

    // A second command, because the statistics every poll asks for do not
    // carry payload bytes.
    let samples = client.query_dh_sample(dh.dh_id).ok();

    // The status each answer carried, which nothing else looks at: a handler
    // tcspecial has not got answers NotFound, and a panel showing zeroes
    // looks exactly like one whose payload is quiet. Triggers are said too,
    // since they are what a triggered handler is for and appear in no other
    // counter.
    note_triggered_if_new(
        dh,
        format!(
            "QUERY_DH {:?}: {} triggers sent, {} bytes to the ground, {} from it; \
             QUERY_DH_SAMPLE {}",
            status,
            stats.triggers_sent,
            stats.bytes_sent,
            stats.bytes_received,
            match &samples {
                Some((status, _, _)) => format!("{status:?}"),
                None => "no answer".to_string(),
            }
        ),
        said,
    );

    let (last_sent_time, last_sent, last_recv_time, last_recv) = match samples {
        Some((_, sent, received)) => {
            let (sent_time, sent_data) = sample_lines(&sent);
            let (recv_time, recv_data) = sample_lines(&received);
            (sent_time, sent_data, recv_time, recv_data)
        }
        None => match previous {
            Some(p) => (
                p.last_sent_time.clone(),
                p.last_sent.clone(),
                p.last_recv_time.clone(),
                p.last_recv.clone(),
            ),
            None => (
                SharedString::from(NO_TRANSFER_TIME),
                SharedString::new(),
                SharedString::from(NO_TRANSFER_TIME),
                SharedString::new(),
            ),
        },
    };

    Some(PanelUpdate {
        row,
        bytes_sent: stats.bytes_sent as i32,
        bytes_recv: stats.bytes_received as i32,
        last_sent_time,
        last_sent,
        last_recv_time,
        last_recv,
    })
}

/// Put a gathered update into its panel.
fn apply_panel_update(dh_model: &Rc<VecModel<DHInfo>>, update: &PanelUpdate) {
    update_row(dh_model, update.row, |info| {
        info.bytes_sent = update.bytes_sent;
        info.bytes_recv = update.bytes_recv;
        info.last_sent_time = update.last_sent_time.clone();
        info.last_sent = update.last_sent.clone();
        info.last_recv_time = update.last_recv_time.clone();
        info.last_recv = update.last_recv.clone();
    });
}

/// The two strings a panel shows for one sample: when, and what.
///
/// A sample with no time is one whose direction has carried nothing, which is
/// not the same as having carried no bytes; it shows the placeholder rather
/// than a time, and no data.
fn sample_lines(sample: &DHSample) -> (SharedString, SharedString) {
    let (time, data) = sample.panel_lines();
    (SharedString::from(time), SharedString::from(data))
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
/// The child is told which payload file to read, as an argument. Tcsmoc builds
/// its panels from that file, so a child reading a different one would serve
/// or simulate payloads the panels do not describe -- handlers that never
/// connect, with nothing on screen to say why. An argument also beats anything
/// the child would have taken from the environment, so for the run of a
/// payload set the MOC's own file is the one that counts.
///
/// The payload file is the whole of what a child is told. Where tcspecial
/// takes commands and where it sends beacons are both in its own
/// configuration file, so neither is the MOC's to hand over -- which means the
/// MOC and a tcspecial it starts agree about the command address only by
/// reading the same default, and a tcspecial whose file moves it is one this
/// MOC will not reach. The exchange on connecting is what reports a mismatch
/// of configuration; an address is not part of it.
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
            .expect(&format!("Failed to start {}", name));

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
    eprintln!("Loading payload configuration from: {}", payload_path);

    let dh_configs: Arc<Vec<DHConfig>> = match load_dh_configs(&payload_path) {
        Ok(dh_configs) => Arc::new(dh_configs),
        Err(e) => {
            eprintln!(
                "Error loading payload configuration from {}: {}",
                payload_path, e
            );
            exit(1);
        }
    };
    eprintln!("Loaded {} data handler configurations", dh_configs.len());

    let ui = MainWindow::new().unwrap();
    let ui_weak = ui.as_weak();

    // And the simulator file beside it, for the panels to show. Loosely: the
    // MOC simulates nothing, so a file it cannot read costs it the simulator
    // half of what a panel can show and nothing else.
    let sims = simulator_settings(&payload_path, &dh_configs);

    // One panel per configured data handler.
    let dh_model: Rc<VecModel<DHInfo>> = Rc::new(VecModel::from(
        dh_configs
            .iter()
            .enumerate()
            .map(|(row, dh)| dh_info_from(dh, sims.as_ref().and_then(|sims| sims.get(row))))
            .collect::<Vec<_>>(),
    ));
    ui.set_dh_model(ModelRc::from(dh_model.clone()));

    // And on the console, what the MOC believes about each payload that has
    // to be asked for its data: see triggered_startup. Said before tcspecial
    // is started or attached to, so that what the MOC read is on the console
    // above whatever the spacecraft then says about the same payload.
    for (row, dh) in dh_configs.iter().enumerate() {
        let status = dh_model
            .row_data(row)
            .map(|info| info.status.to_string())
            .unwrap_or_default();
        if let Some(line) = triggered_startup(dh, &status) {
            eprintln!("{line}");
        }
    }

    // Shape the grid, and open the window at the size that shape wants. The
    // window cannot work this out for itself: it would need the panel count
    // before the model is set.
    let shape = grid_shape(dh_configs.len());
    eprintln!(
        "{} data handlers in a {}x{} grid, window {}x{}",
        dh_configs.len(),
        shape.columns,
        shape.rows,
        shape.width,
        shape.height
    );
    ui.set_columns(i32::try_from(shape.columns).unwrap_or(1));
    // Taller than the grid asks for where there is a third row to look ahead
    // for, and no taller than the panels on hand need otherwise. It is also
    // why the window can open taller than it is wide, which the shape itself
    // never is: the shape rule chooses the grid, and the room rule says how
    // much of the window to give it.
    ui.window().set_size(window_size(&shape, dh_configs.len()));

    // Start tcspecial and tcssim subprocesses first, each reading the payload
    // file tcsmoc read, so all three describe the same payloads.
    //
    // Unless a tcspecial is already there. Starting a second one does not
    // work and never did: the command interpreter's address can be bound
    // once, so the second exits and the MOC comes up with a window, no
    // spacecraft behind it, and the reason only on a terminal nobody is
    // reading. Talking to the one that is already running is what was wanted
    // in every case where this happened.
    let [tcspecial, tcssim] = CHILDREN;

    let mut process_manager_tcspecial = None;
    if tcspecial_already_running(DEFAULT_CI_ADDRESS) {
        eprintln!(
            "a tcspecial is already answering at {}, so not starting another",
            DEFAULT_CI_ADDRESS
        );
        ui.set_last_response(SharedString::from(format!(
            "Using the tcspecial already running at {}",
            DEFAULT_CI_ADDRESS
        )));
    } else {
        let manager = Arc::new(ProcessManager::new());
        manager.start_child(tcspecial, &payload_path);
        process_manager_tcspecial = Some(manager);
    }

    let process_manager_tcssim = Arc::new(ProcessManager::new());
    process_manager_tcssim.start_child(tcssim, &payload_path);

    eprintln!("sleeping to let the subprocesses initialize");
    thread::sleep(Duration::new(2, 0));

    // What this MOC read, said before there is any link to say it over. A
    // socket that cannot be made, or a tcspecial that is not answering, takes
    // the exchange below with it -- and that is exactly when an operator wants
    // to know which configuration each end was holding.
    let version = ConfigVersion::of_this_build();

    // The set's version, said the way tcspecial says its own and the way a
    // beacon says it: this line and tcspecial's are read against each other,
    // so they state the same three facts in the same words.
    //
    // The file has already been loaded, and its version is one of the rules
    // that loading applies, so a failure here is a file that changed between
    // the two reads. Said rather than left out: a line missing one of its
    // three facts reads as a set that has no version.
    let config_version = load_config_version(&payload_path)
        .map(|stated| format!("config v{stated}"))
        .unwrap_or_else(|e| format!("config version unreadable ({e})"));

    let digest = match digest_of_file(&payload_path) {
        Ok(digest) => {
            eprintln!("v{version}, configuration {payload_path} {config_version} md5: {digest}");
            Some(digest)
        }
        Err(e) => {
            eprintln!(
                "v{version}, configuration {payload_path} {config_version} cannot be \
                 digested: {e}"
            );
            None
        }
    };

    // Open the link to the command interpreter on startup, by the same call
    // the Connect button makes, so the link the window comes up with is the
    // one that button would have given it.
    // Where payload commands go. A MOC that cannot work this out cannot
    // command a payload at all, so it says so and stops rather than coming up
    // with panels whose buttons would be refused.
    let payload_port = match payload_command_port() {
        Ok(port) => port,
        Err(e) => {
            eprintln!("Cannot tell where payload commands go: {e}");
            exit(1);
        }
    };

    let mut link = CiLink::down(payload_port);
    if let Err(e) = link.connect(DEFAULT_CI_ADDRESS) {
        eprintln!("Failed to connect to {}: {}", DEFAULT_CI_ADDRESS, e);
        ui.set_ci_status(SharedString::from(ERROR_STATUS));
        ui.set_last_response(SharedString::from(format!("Connection failed: {}", e)));
        // Exit since we can't operate without a connection
        exit(1);
    }
    match link.payload_address() {
        Ok(payload) => eprintln!(
            "Connected to {}, payload commands to {}",
            DEFAULT_CI_ADDRESS, payload
        ),
        // The link is up, so the address parsed when it was opened; said
        // rather than hidden all the same.
        Err(e) => eprintln!(
            "Connected to {}, and cannot say where payload commands go: {}",
            DEFAULT_CI_ADDRESS, e
        ),
    }
    let link: Arc<Mutex<CiLink>> = Arc::new(Mutex::new(link));
    if let Some(digest) = digest {
        hear_what_tcspecial_read(&ui, &link, version, digest);
    }
    ui.set_ci_status(SharedString::from(CONNECTED_STATUS));
    ui.set_ci_address(SharedString::from(DEFAULT_CI_ADDRESS));

    // Start receiving beacon data
    // Where the MOC listens for beacons: the group and interface tcspecial
    // sends them to, read out of the same two files tcspecial reads -- the
    // command interpreter's own configuration, and the payload set's
    // tcspecial section, which wins. Read rather than compiled in, because a
    // listener that assumed an address would hear nothing the moment a
    // mission moved the beacon, and say nothing about why.
    //
    // Loosely, as the simulator file is read: a MOC that cannot work out
    // where the beacons are says so and runs without them, the indicator
    // staying at "none arrived". Controlling payloads does not depend on it.
    let beacon_ui_weak = ui_weak.clone();
    let _beacon_receive = match beacon_to_listen_for(&payload_path) {
        Ok(beacon) => {
            eprintln!(
                "Listening for beacons on the group {} on interface {}",
                beacon.group, beacon.interface
            );
            BeaconReceive::new(beacon_ui_weak, beacon, BEACON_INDICATOR.clone())
        }
        Err(e) => {
            eprintln!("Not listening for beacons: {e}");
            ui.set_last_response(SharedString::from(format!(
                "Not listening for beacons: {e}"
            )));
            None
        }
    };

    handle_link_button(&ui, ui_weak.clone(), link.clone());
    handle_main_menu(&ui, ui_weak.clone(), link.clone());
    query_dh_buttons(&ui, ui_weak.clone(), link.clone(), dh_configs.clone(), dh_model.clone());
    // The payload data path, one listener per handler: see oc_link. Opened
    // before the poller, so a handler that is already running is heard from
    // on the first pass.
    let arrivals = oc_link::listen(&dh_configs);
    poll_panels(
        link.clone(),
        dh_configs.clone(),
        dh_model.clone(),
        payload_port,
        arrivals,
    );
    handle_transfer_button(&ui, ui_weak.clone(), link.clone(), dh_configs.clone(), dh_model.clone());
/*
    // Menu action handler
    {
        let client = client.clone();
        let ui_weak = ui_weak.clone();
        ui.on_menu_action(move |action| {
            let ui = ui_weak.unwrap();
            let mut guard = client.lock().unwrap();

            match action {
                MenuAction::Ping => {
                    eprintln!("Ping from menu");
                    match guard.ping() {
                        Ok(tm) => {
                            ui.set_last_response(SharedString::from(format!(
                                "PING OK - timestamp: {}.{}",
                                tm.timestamp.seconds, tm.timestamp.nanoseconds
                            )));
                        }
                        Err(e) => {
                            ui.set_last_response(SharedString::from(format!("PING failed: {}", e)));
                        }
                    }
                }
                MenuAction::ArmRestart => {
                    match guard.restart_arm(ArmKey(0xf001adad)) {
                        Ok(status) => {
                            ui.set_last_response(SharedString::from(format!("ARM_RESTART: {:?}", status)));
                        }
                        Err(e) => {
                            ui.set_last_response(SharedString::from(format!("ARM_RESTART failed: {}", e)));
                        }
                    }
                }
                MenuAction::Restart => {
                    match guard.restart(ArmKey(0xf001adad)) {
                        Ok(status) => {
                            ui.set_last_response(SharedString::from(format!("RESTART: {:?}", status)));
                        }
                        Err(e) => {
                            ui.set_last_response(SharedString::from(format!("RESTART failed: {}", e)));
                        }
                    }
                }
                MenuAction::Query => {
                    ui.set_last_response(SharedString::from("Query - not yet implemented"));
                }
                MenuAction::QueryDh => {
                    ui.set_last_response(SharedString::from("Query DH - select DH first"));
                }
                MenuAction::StartDh => {
                    ui.set_last_response(SharedString::from("Start DH - select DH first"));
                }
                MenuAction::StopDh => {
                    ui.set_last_response(SharedString::from("Stop DH - select DH first"));
                }
            }
        });
    }
*/

    handle_quit(&ui, process_manager_tcssim.clone(), process_manager_tcspecial.clone());
    // Quit button handler
/*
    {
        let pm_tcssim = process_manager_tcssim.clone();
        let pm_tcspecial = process_manager_tcspecial.clone();
        ui.on_quit_clicked(move || {
            kill_and_exit_all(&pm_tcssim, &pm_tcspecial);
        });
    }
*/

    handle_close(&ui, process_manager_tcssim.clone(), process_manager_tcspecial.clone());
    // Window close handler (close box)
/*
    {
        let pm_tcssim = process_manager_tcssim.clone();
        let pm_tcspecial = process_manager_tcspecial.clone();
        ui.window().on_close_requested(move || {
            kill_and_exit_all(&pm_tcssim, &pm_tcspecial);
            slint::CloseRequestResponse::HideWindow
        });
    }
*/

    ui.run().unwrap();
}

/// The one button that takes the link up and down.
///
/// There were two, and neither did anything: the window declared a callback
/// for each and nothing in Rust answered them, and a Slint callback with no
/// handler is silently nothing. Disconnect left the link up and the status
/// line alone, which is how it was found; Connect looked as though it worked
/// only because the status line already said Connected, the MOC having
/// opened the link at startup before the window appeared.
///
/// Which way a press goes is decided by what the button offered, read back
/// from the status the window derives its label from -- not by what the link
/// turns out to be. The two cannot disagree by any path through this
/// function, and if they ever did, doing what the label said is the honest
/// answer and a safe one: `connect` replaces whatever the link had, and
/// `disconnect` is harmless on a link that is already down.
fn handle_link_button(
    ui: &MainWindow,
    ui_weak: slint::Weak<MainWindow>,
    link: Arc<Mutex<CiLink>>,
) {
    ui.on_link_clicked(move || {
        let ui = ui_weak.unwrap();
        let mut guard = link.lock().unwrap();

        if ui.get_ci_status() == CONNECTED_STATUS {
            // Said in terms of where the link was, not of what the address
            // box says, which may have been edited since.
            let address = guard.address().to_string();
            let was_up = guard.is_connected();
            guard.disconnect();

            eprintln!("Disconnected from {}", address);
            ui.set_ci_status(SharedString::from(DISCONNECTED_STATUS));
            ui.set_last_response(SharedString::from(if was_up {
                format!("Disconnected from {}", address)
            } else {
                format!("Already disconnected from {}", address)
            }));
            return;
        }

        // Where the address box points now, rather than where the link last
        // went: typing an address and pressing Connect is how the MOC is told
        // to talk to a different command interpreter.
        let address = ui.get_ci_address().to_string();
        match guard.connect(&address) {
            Ok(()) => {
                eprintln!("Connected to {}", address);
                ui.set_ci_status(SharedString::from(CONNECTED_STATUS));
                ui.set_last_response(SharedString::from(format!("Connected to {}", address)));
            }
            Err(e) => {
                eprintln!("Failed to connect to {}: {}", address, e);
                ui.set_ci_status(SharedString::from(ERROR_STATUS));
                ui.set_last_response(SharedString::from(format!(
                    "Connection to {} failed: {}",
                    address, e
                )));
            }
        }
    });
}

// Menu action handler
fn handle_main_menu (ui: &MainWindow, ui_weak: slint::Weak<MainWindow>, link: Arc<Mutex<CiLink>>) {
    ui.on_menu_action(move |action| {
        let ui = ui_weak.unwrap();
        let mut guard = link.lock().unwrap();

        match action {
            // The three actions that send a command ask the link for a
            // client as they go: `None` is the link being down, which is a
            // thing to say rather than a command that failed.
            MenuAction::Ping => {
                eprintln!("Ping from menu");
                match guard.client().map(|client| client.ping()) {
                    Some(Ok(tm)) => {
                        ui.set_last_response(SharedString::from(format!(
                            "PING OK - timestamp: {}.{}",
                            tm.timestamp.seconds, tm.timestamp.nanoseconds
                        )));
                    }
                    Some(Err(e)) => {
                        ui.set_last_response(SharedString::from(format!("PING failed: {}", e)));
                    }
                    None => ui.set_last_response(SharedString::from(NOT_CONNECTED)),
                }
            }
            MenuAction::ArmRestart => {
                match guard.client().map(|client| client.restart_arm(ArmKey(0xf001adad))) {
                    Some(Ok(status)) => {
                        ui.set_last_response(SharedString::from(format!("ARM_RESTART: {:?}", status)));
                    }
                    Some(Err(e)) => {
                        ui.set_last_response(SharedString::from(format!("ARM_RESTART failed: {}", e)));
                    }
                    None => ui.set_last_response(SharedString::from(NOT_CONNECTED)),
                }
            }
            MenuAction::Restart => {
                match guard.client().map(|client| client.restart(ArmKey(0xf001adad))) {
                    Some(Ok(status)) => {
                        ui.set_last_response(SharedString::from(format!("RESTART: {:?}", status)));
                    }
                    Some(Err(e)) => {
                        ui.set_last_response(SharedString::from(format!("RESTART failed: {}", e)));
                    }
                    None => ui.set_last_response(SharedString::from(NOT_CONNECTED)),
                }
            }
            MenuAction::Query => {
                ui.set_last_response(SharedString::from("Query - not yet implemented"));
            }
            MenuAction::QueryDh => {
                ui.set_last_response(SharedString::from("Query DH - select DH first"));
            }
            MenuAction::StartDh => {
                ui.set_last_response(SharedString::from("Start DH - select DH first"));
            }
            MenuAction::StopDh => {
                ui.set_last_response(SharedString::from("Stop DH - select DH first"));
            }
        }
    });
}

// Query all DHs button handler
//
// Which data handlers to ask about comes from the configuration file: the
// panels are its rows, so a row and its handler share an index.
fn query_dh_buttons(
    ui: &MainWindow,
    ui_weak: slint::Weak<MainWindow>,
    link: Arc<Mutex<CiLink>>,
    dh_configs: Arc<Vec<DHConfig>>,
    dh_model: Rc<VecModel<DHInfo>>,
) {
    ui.on_query_all_clicked(move || {
        let ui = ui_weak.unwrap();
        let mut guard = link.lock().unwrap();
        let client = match guard.client() {
            Some(client) => client,
            None => {
                ui.set_last_response(SharedString::from(NOT_CONNECTED));
                return;
            }
        };
        let mut results = Vec::new();

        for (row, dh) in dh_configs.iter().enumerate() {
            match panel_update_for(client, row, dh, None, &mut None) {
                Some(update) => {
                    results.push(format!(
                        "{}: sent={} recv={}",
                        dh.name.0, update.bytes_sent, update.bytes_recv
                    ));
                    apply_panel_update(&dh_model, &update);
                }
                None => results.push(format!("{}: no answer", dh.name.0)),
            }
        }

        ui.set_last_response(SharedString::from(results.join("; ")));
    });
}

/// How often the panels are refreshed without being asked.
const PANEL_POLL_INTERVAL: Duration = Duration::from_millis(1000);

/// How long a poll waits for a handler's answer.
///
/// Shorter than the client's own default, because the poller asks again in a
/// second: an answer that arrives after the next question was already due is
/// of no use to a panel, and waiting the default five seconds for one only
/// makes a pass against a wedged tcspecial take tens of seconds. A panel
/// whose handler did not answer keeps what it last showed either way.
const POLL_TIMEOUT: Duration = Duration::from_millis(1000);

/// Keep the panels current.
///
/// Until this existed a panel only changed when someone pressed Query All, so
/// a handler moving data looked exactly like one doing nothing: the byte
/// counters and the two sample lines sat at whatever the last click had left.
///
/// The asking happens on a thread of its own and the applying on the event
/// loop. That split is the point: each command waits on the spacecraft for up
/// to the client's timeout, and four handlers' worth of that on the event loop
/// would be a window that stops repainting whenever tcspecial is slow. The
/// thread leaves what it gathered where the timer can pick it up.
///
/// The thread asks over a connection of its own, following the link the
/// buttons control rather than sharing it; see [`poll_pass`] for why.
fn poll_panels(
    link: Arc<Mutex<CiLink>>,
    dh_configs: Arc<Vec<DHConfig>>,
    dh_model: Rc<VecModel<DHInfo>>,
    payload_port: u16,
    arrivals: Arrivals,
) {
    let pending: Arc<Mutex<Vec<PanelUpdate>>> = Arc::new(Mutex::new(Vec::new()));

    {
        let pending = pending.clone();
        thread::spawn(move || {
            // What each panel last showed, so a handler that answers the
            // statistics but not the samples keeps its sample lines.
            let mut last: Vec<Option<PanelUpdate>> = vec![None; dh_configs.len()];

            // And what was last said about each on the console, so a pass
            // that found nothing new says nothing.
            let mut said: Vec<Option<String>> = vec![None; dh_configs.len()];

            // The poller's own link, kept wherever the window's link is.
            let mut poll_link = CiLink::down(payload_port);

            loop {
                thread::sleep(PANEL_POLL_INTERVAL);

                let gathered =
                    poll_pass(&link, &mut poll_link, &dh_configs, &mut last, &mut said);

                if let Ok(mut queue) = pending.lock() {
                    *queue = gathered;
                }
            }
        });
    }

    // On the event loop, where the model may be touched. This does no I/O, so
    // it cannot hold the window up.
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        PANEL_POLL_INTERVAL,
        move || {
            let updates = match pending.lock() {
                Ok(mut queue) => std::mem::take(&mut *queue),
                Err(_) => return,
            };
            for update in &updates {
                apply_panel_update(&dh_model, update);
            }

            // And what the ground itself received, which no command asked
            // for: the listeners put it where this can pick it up, as the
            // poller does. Read every pass rather than taken, because it is
            // the last thing that arrived rather than a queue of events --
            // a panel shows the latest, as it does for the samples above.
            if let Ok(slots) = arrivals.lock() {
                for (row, arrival) in slots.iter().enumerate() {
                    if let Some(arrival) = arrival {
                        let (time, data) = arrival.panel_lines();
                        update_row(&dh_model, row, |info| {
                            info.oc_time = SharedString::from(time.clone());
                            info.oc_data = SharedString::from(data.clone());
                        });
                    }
                }
            }
        },
    );

    // The timer stops when it is dropped, and this function is returning.
    std::mem::forget(timer);
}

/// One pass of the poller: ask every handler what it has moved.
///
/// `link` is the link the window's buttons control and `poll_link` the
/// poller's own, which follows it. The two are separate so that a pass never
/// holds the lock the window needs: the shared link is locked only long
/// enough to read where it went, never across a command.
///
/// That matters most when the spacecraft has stopped answering. A pass then
/// waits out its timeout on every handler, and when the poller shared the
/// window's link, every click waited behind it -- Disconnect included, which
/// is the one button wanted just then.
///
/// Asking over a second connection is also what makes a pass and a click
/// safe to overlap, which is what the shared lock used to be held for: each
/// socket gets its own answers, since tcspecial replies to whoever asked, so
/// no panel can end up showing one handler's statistics beside another's
/// samples.
///
/// `last` is what each panel last showed, carried in and out so a handler
/// that answers the statistics but not the samples keeps its sample lines.
/// It is left alone for a handler that did not answer at all.
///
/// `said` is what was last said on the console about each payload, carried the
/// same way and for the same kind of reason: a pass happens every second
/// whether or not anything moved, and what is worth saying is what changed.
fn poll_pass(
    link: &Mutex<CiLink>,
    poll_link: &mut CiLink,
    dh_configs: &[DHConfig],
    last: &mut [Option<PanelUpdate>],
    said: &mut [Option<String>],
) -> Vec<PanelUpdate> {
    // Where the buttons have the link now. A poisoned lock leaves the poller
    // where it was rather than taking the panels down with it.
    let wanted = match link.lock() {
        Ok(guard) => guard.connected_to().map(str::to_string),
        Err(_) => return Vec::new(),
    };

    poll_link.follow(wanted.as_deref());
    // Said every pass, since `follow` may have just opened a fresh client
    // and a fresh client starts from the default timeout.
    poll_link.set_timeout(POLL_TIMEOUT);

    // Nothing to ask while the link is down, and the panels keep what they
    // last showed rather than being blanked: a disconnect says nothing about
    // what the handlers did, and polling resumes on its own once Connect
    // brings the link back up.
    let client = match poll_link.client() {
        Some(client) => client,
        None => return Vec::new(),
    };

    let mut gathered = Vec::with_capacity(dh_configs.len());
    for (row, dh) in dh_configs.iter().enumerate() {
        if let Some(update) = panel_update_for(client, row, dh, last[row].as_ref(), &mut said[row])
        {
            last[row] = Some(update.clone());
            gathered.push(update);
        }
    }
    gathered
}

/// Where this MOC should listen for beacons.
///
/// The same two configurations tcspecial resolves it from, in the same order:
/// the payload set's `tcspecial` section if it has one, and the command
/// interpreter's own file otherwise. Reading the same files is what makes the
/// two ends name one group and one interface -- a listener with an address of
/// its own would go deaf the moment a mission moved the beacon, and the only
/// symptom would be an indicator saying nothing had arrived.
fn beacon_to_listen_for(payload_path: &str) -> Result<Beacon, String> {
    let config_path = tcspecial_config_path();
    let config = load_tcspecial_config(&config_path)
        .map_err(|e| format!("{config_path} could not be read: {e}"))?;

    let section = load_tcspecial_section(payload_path)
        .map_err(|e| format!("{payload_path} could not be read: {e}"))?;

    beacon(&config, section.as_ref())
}

/// Which port the spacecraft takes payload commands on.
///
/// From the command interpreter's own configuration, which is where tcspecial
/// takes it from: one file, read by both ends, so they cannot name different
/// ports. The payload set's section states one too and is not read for it,
/// for the reason tcspecial does not read it there -- where commands are
/// taken is the interpreter's own business, not the set's.
fn payload_command_port() -> Result<u16, String> {
    let config_path = tcspecial_config_path();
    let config = load_tcspecial_config(&config_path)
        .map_err(|e| format!("{config_path} could not be read: {e}"))?;

    Ok(config.payload_port)
}

/// Send what this MOC read, and hear what the spacecraft read.
///
/// The two ends each read a payload set -- the MOC to build its panels,
/// tcspecial to serve the handlers -- and nothing made them prove it was the
/// same set. When it was not, the only sign was a command answered `NotFound`
/// for a payload the operator could see: a tcspecial left running from an
/// earlier set, attached to rather than replaced, since the MOC attaches to
/// whatever answers a ping.
///
/// `version` and `digest` are this end's, said on the console before the
/// socket was made; what this adds is the other end's, so the two can be read
/// against each other whether or not anything is wrong, with `Last Response`
/// carrying the verdict for the operator.
fn hear_what_tcspecial_read(
    ui: &MainWindow,
    link: &Mutex<CiLink>,
    version: ConfigVersion,
    digest: ConfigDigest,
) {
    let mut guard = link.lock().unwrap();
    let answer = match guard.client() {
        Some(client) => client.connect(version, digest),
        None => {
            ui.set_last_response(SharedString::from(NOT_CONNECTED));
            return;
        }
    };

    match answer {
        Ok((their_version, their_config_version, their_digest)) => {
            // Said the way a beacon says the same three facts, by the one
            // function that writes them: the two are read against each other.
            eprintln!(
                "tcspecial answers {}",
                beacon_receive::what_it_read(
                    their_version,
                    their_config_version,
                    their_digest
                )
            );

            // What the answer said, in the words a beacon and tcspecial's own
            // console line use: the set's version labelled config v, and the
            // digest after md5:. The set's version is the one tcspecial sent,
            // which is the version of the file it read -- this end does not
            // send its own, so there is nothing here to compare it with.
            let said = if their_version != version {
                format!("tcspecial is version {their_version}, this is {version}")
            } else if their_digest != digest {
                format!(
                    "tcspecial is serving a different configuration: \
                     config v{their_config_version} md5: {their_digest}, not {digest}"
                )
            } else {
                format!(
                    "tcspecial agrees: v{version}, \
                     config v{their_config_version} md5: {digest}"
                )
            };
            if their_version != version || their_digest != digest {
                eprintln!("{said}");
            }
            ui.set_last_response(SharedString::from(said));
        }
        Err(e) => {
            eprintln!("CONNECT did not get through: {e}");
            ui.set_last_response(SharedString::from(format!("CONNECT failed: {e}")));
        }
    }
}

/// Which way a press of a panel's one button goes.
///
/// Decided by what the button offered, read back from the status its label
/// comes from -- not by what the handler turns out to be doing. The two
/// cannot disagree by any path through the window, and if they ever did,
/// doing what the label said is the honest answer: a handler is told to start
/// by a button that offered to start it.
///
/// Anything that is not the running status offers to transmit. A handler
/// whose last command failed shows `Error`, and the useful thing to offer
/// then is the start that failed, not a stop of something that never began.
fn transfer_wanted(status: &str) -> Transfer {
    if status == ACTIVE_STATUS {
        Transfer::Discard
    } else {
        Transfer::Transmit
    }
}

/// What a press of a panel's button asks tcspecial for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transfer {
    /// Start the handler, so its payload's data reaches the MOC.
    Transmit,
    /// Stop it, so what the payload sends is dropped on the spacecraft.
    Discard,
}

/// The one button that starts and stops a handler.
///
/// There were two, Start and Stop, and either was pressable whatever the
/// handler was already doing: a Stop on a stopped handler asks tcspecial to
/// stop something that is not running and reads as though it had done
/// something. One button offers the one thing worth doing next.
fn handle_transfer_button(
    ui: &MainWindow,
    ui_weak: slint::Weak<MainWindow>,
    link: Arc<Mutex<CiLink>>,
    dh_configs: Arc<Vec<DHConfig>>,
    dh_model: Rc<VecModel<DHInfo>>,
) {
    ui.on_transfer_dh(move |row| {
        let ui = ui_weak.unwrap();
        let row = row as usize;
        let dh = match dh_configs.get(row) {
            Some(dh) => dh,
            None => return,
        };

        // The status the button's label came from, so the command sent is the
        // one the label offered.
        let showing = match dh_model.row_data(row) {
            Some(info) => info.status.to_string(),
            None => return,
        };

        match transfer_wanted(&showing) {
            Transfer::Transmit => transmit(&ui, &link, dh, &dh_model, row),
            Transfer::Discard => discard(&ui, &link, dh, &dh_model, row),
        }
    });
}

/// Start the handler: what the payload sends begins reaching the MOC.
///
/// The identity, name, and kind of the handler come from the configuration
/// file rather than from the row number; the panel passes only its own row.
fn transmit(
    ui: &MainWindow,
    link: &Mutex<CiLink>,
    dh: &DHConfig,
    dh_model: &Rc<VecModel<DHInfo>>,
    row: usize,
) {
    note_triggered(
        dh,
        format!(
            "Transmit pressed: sending START_DH for dh_id {} as a {:?} handler",
            dh.dh_id.0,
            dh.endpoint.kind()
        ),
    );

    let mut guard = link.lock().unwrap();
    let sent = match guard.client() {
        Some(client) => client.start_dh(dh.dh_id, dh.endpoint.kind(), dh.name.clone()),
        None => {
            note_triggered(dh, format!("START_DH not sent: {NOT_CONNECTED}"));
            ui.set_last_response(SharedString::from(NOT_CONNECTED));
            return;
        }
    };
    match sent {
        Ok(status) => {
            let showing = if status == CommandStatus::Success {
                ACTIVE_STATUS
            } else {
                ERROR_STATUS
            };
            note_triggered(
                dh,
                format!("START_DH answered {status:?}, so the panel now says {showing}"),
            );
            update_row(dh_model, row, |info| {
                info.status = SharedString::from(showing);
            });
            ui.set_last_response(SharedString::from(format!(
                "START_DH {} - {:?}",
                dh.name.0, status
            )));
        }
        Err(e) => {
            note_triggered(dh, format!("START_DH did not get through: {e}"));
            ui.set_last_response(SharedString::from(format!(
                "START_DH {} failed: {}",
                dh.name.0, e
            )));
        }
    }
}

/// Stop it: what the payload sends is dropped on the spacecraft instead.
fn discard(
    ui: &MainWindow,
    link: &Mutex<CiLink>,
    dh: &DHConfig,
    dh_model: &Rc<VecModel<DHInfo>>,
    row: usize,
) {
    note_triggered(
        dh,
        format!("Discard pressed: sending STOP_DH for dh_id {}", dh.dh_id.0),
    );

    let mut guard = link.lock().unwrap();
    let sent = match guard.client() {
        Some(client) => client.stop_dh(dh.dh_id),
        None => {
            note_triggered(dh, format!("STOP_DH not sent: {NOT_CONNECTED}"));
            ui.set_last_response(SharedString::from(NOT_CONNECTED));
            return;
        }
    };
    match sent {
        Ok(status) => {
            note_triggered(
                dh,
                format!("STOP_DH answered {status:?}, so the panel now says {STOPPED_STATUS}"),
            );
            update_row(dh_model, row, |info| {
                info.status = SharedString::from(STOPPED_STATUS);
            });
            ui.set_last_response(SharedString::from(format!(
                "STOP_DH {} - {:?}",
                dh.name.0, status
            )));
        }
        Err(e) => {
            note_triggered(dh, format!("STOP_DH did not get through: {e}"));
            ui.set_last_response(SharedString::from(format!(
                "STOP_DH {} failed: {}",
                dh.name.0, e
            )));
        }
    }
}

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
fn kill_and_exit_all(pm_tcssim: &Arc<ProcessManager>, pm_tcspecial: Option<&Arc<ProcessManager>>) {
    pm_tcssim.kill();
    if let Some(pm_tcspecial) = pm_tcspecial {
        pm_tcspecial.kill();
    }
    exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The port a test's payload commands would go to; nothing answers on it.
    const A_PAYLOAD_PORT: u16 = 4001;
    use std::ffi::OsStr;
    use std::path::Path;
    use tcslibgs::{EndpointConfig, NetworkProtocol};
    use std::time::Instant;
    use tcslibgs::config::DEFAULT_PAYLOAD_CONFIG_PATH;
    use tcspecial::config::DEFAULT_TCSPECIAL_CONFIG_PATH;
    use tcslibgs::{DHId, DHName, DeviceConfig, NetworkConfig, Timestamp};

    /// Arguments as the program really receives them, the program's own name
    /// first.
    fn args(rest: &[&str]) -> std::vec::IntoIter<String> {
        let mut all = vec!["tcsmoc".to_string()];
        all.extend(rest.iter().map(|s| s.to_string()));
        all.into_iter()
    }

    /// A port nothing in this project uses, so a command sent there is never
    /// answered and a poll has to wait out its timeout.
    const UNANSWERED_ADDRESS: &str = "127.0.0.1:65123";

    /// One data handler, for the tests that need something to ask about.
    fn a_dh() -> DHConfig {
        DHConfig {
            dh_id: DHId(1),
            name: DHName::new("DH1"),
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/null".to_string(),
            }),
            packet_size: 1,
            oc: None,
            mode: Default::default(),
        }
    }

    /// A poll that is waiting on the spacecraft leaves the window's buttons
    /// their link.
    ///
    /// This is what made Disconnect unusable as soon as it was worth using.
    /// The poller held the shared link across its commands, so with nothing
    /// answering, a pass sat on it for a timeout per handler and every click
    /// waited behind -- a window that does nothing, at the moment the
    /// operator is trying to take the link down. The poller now asks over a
    /// link of its own and locks the shared one only to read where it went.
    #[test]
    fn a_poll_leaves_the_window_its_link() {
        let link = Arc::new(Mutex::new(CiLink::down(A_PAYLOAD_PORT)));
        link.lock().unwrap().connect(UNANSWERED_ADDRESS).unwrap();

        let pass = {
            let link = link.clone();
            thread::spawn(move || {
                let dh_configs = vec![a_dh()];
                let mut poll_link = CiLink::down(A_PAYLOAD_PORT);
                let mut last = vec![None];
                let mut said = vec![None];

                let started = Instant::now();
                let gathered =
                    poll_pass(&link, &mut poll_link, &dh_configs, &mut last, &mut said);
                (gathered, started.elapsed())
            })
        };

        // Far enough in for the pass to be waiting on its first command.
        thread::sleep(POLL_TIMEOUT / 4);

        let waited = Instant::now();
        drop(link.lock().unwrap());
        let waited = waited.elapsed();

        let (gathered, pass_took) = pass.join().unwrap();
        assert!(
            gathered.is_empty(),
            "nothing was answering, so there was nothing to show"
        );
        assert!(
            pass_took >= POLL_TIMEOUT / 2,
            "the pass answered in {:?}, so it was not waiting and this proves nothing",
            pass_took
        );
        assert!(
            waited < POLL_TIMEOUT / 2,
            "a click waited {:?} behind a poll",
            waited
        );
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

        // An empty payload file still opens a window.
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

        // One of each kind, which is the case that showed it, and then both
        // kinds alone: a rule about widths has to hold for a row of one kind
        // as much as for a mixed one.
        let mut periodic = a_dh();
        periodic.name = DHName::new("auto-send");
        let mut triggered = a_dh();
        triggered.name = DHName::new("triggered-send");
        triggered.mode = tcslibgs::DHMode::Triggered {
            trigger: b"go".to_vec(),
            interval_ms: 1000,
        };

        for (what, dhs) in [
            ("one of each", vec![&periodic, &triggered]),
            ("both periodic", vec![&periodic, &periodic]),
            ("both triggered", vec![&triggered, &triggered]),
        ] {
            let rows: Vec<DHInfo> = dhs.iter().map(|dh| dh_info_from(dh, None)).collect();
            let panels = rows.len();

            let ui = MainWindow::new().unwrap();
            ui.set_dh_model(ModelRc::from(Rc::new(VecModel::from(rows))));
            let shape = grid_shape(panels);
            ui.set_columns(i32::try_from(shape.columns).unwrap());
            ui.window().set_size(window_size(&shape, panels));
            ui.show().unwrap();

            let widths: Vec<f32> = ElementHandle::find_by_element_type_name(&ui, "DHPanel")
                .map(|panel| panel.size().width)
                .collect();
            assert_eq!(widths.len(), panels, "{what}: {} panels found", widths.len());
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
            let rows: Vec<DHInfo> = (0..panels)
                .map(|i| {
                    let mut dh = a_dh();
                    dh.name = DHName::new(format!("DH{i}"));
                    dh_info_from(&dh, None)
                })
                .collect();

            let ui = MainWindow::new().unwrap();
            ui.set_dh_model(ModelRc::from(Rc::new(VecModel::from(rows))));
            let shape = grid_shape(panels);
            ui.set_columns(i32::try_from(shape.columns).unwrap());
            ui.window().set_size(window_size(&shape, panels));
            ui.show().unwrap();

            // One per panel, in the model's order: the name is the first text
            // of each panel and nothing else shows it.
            let mut placed: Vec<(String, f32, f32)> = Vec::new();
            for i in 0..panels {
                let name = format!("DH{i}");
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
                placed
                    .iter()
                    .filter(|(_, _, y)| *y == placed[0].2)
                    .count(),
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

        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(DEFAULT_PAYLOAD_CONFIG_PATH);
        let dh_configs = load_dh_configs(&path)
            .unwrap_or_else(|e| panic!("{} failed to load: {e}", path.display()));
        let shipped: Vec<DHInfo> = dh_configs.iter().map(|dh| dh_info_from(dh, None)).collect();

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
            ui.set_dh_model(ModelRc::from(Rc::new(VecModel::from(rows))));

            // Opened as main() opens it, room rule and all.
            let shape = grid_shape(panels);
            ui.set_columns(i32::try_from(shape.columns).unwrap());
            ui.window().set_size(window_size(&shape, panels));
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

            // Room for as many rows as the rule says, and no more than
            // that: a configuration of fewer panels than the window looks
            // ahead for opens only as tall as those panels need.
            let rows = rows_of_room(shape.rows, panels);
            let wanted = height_for_rows(rows) - CHROME_HEIGHT;
            assert!(
                area.size().height >= wanted,
                "{what}: the panels have {} of the {wanted} that {rows} rows \
                 need",
                area.size().height
            );
            assert!(
                area.size().height < wanted + PANEL_HEIGHT,
                "{what}: the panels have {}, room for a row more than the \
                 {rows} wanted",
                area.size().height
            );

            // Every panel, and then the three rows of data each one ends
            // with.
            for found_what in [
                "DHPanel",
                "DHPanel::last-sent-row",
                "DHPanel::last-recv-row",
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
        the_params_button_shows_what_both_files_said();
        the_window_shows_when_the_last_beacon_arrived();
        every_panel_shows_both_sample_lines();
        panels_fill_left_to_right();
        every_panel_in_a_row_is_the_same_width();
    }

    /// Only a payload that has to be asked is talked about on the console.
    ///
    /// The console carries the traffic of one payload per line per poll, so a
    /// payload that sends on its own would double the lines and say nothing a
    /// panel does not already show. The one being watched is the one whose
    /// data depends on what the MOC and tcspecial said to each other.
    #[test]
    fn only_a_triggered_payload_is_talked_about() {
        let mut dh = a_dh();

        assert_eq!(
            triggered_note(&dh, "START_DH answered Success"),
            None,
            "a payload that sends on its own is talked about"
        );

        dh.mode = tcslibgs::DHMode::Triggered {
            trigger: b"go".to_vec(),
            interval_ms: 1000,
        };
        let said = triggered_note(&dh, "START_DH answered Success")
            .expect("a triggered payload is talked about");
        assert!(said.contains("DH1"), "the payload is not named: {said}");
        assert!(
            said.contains("START_DH answered Success"),
            "what happened is not said: {said}"
        );
    }

    /// A refreshed line is said when it changes and not again until it
    /// changes once more.
    ///
    /// The panels poll every handler once a second from the moment the link
    /// is up, started or not, so saying the poll's answer every pass meant a
    /// line a second about a payload sitting still -- and the lines that
    /// mattered, a trigger count moving or a status turning into NotFound,
    /// went past in the middle of a hundred identical ones.
    #[test]
    fn a_refreshed_line_is_said_only_when_it_changes() {
        let mut dh = a_dh();
        dh.mode = tcslibgs::DHMode::Triggered {
            trigger: b"go".to_vec(),
            interval_ms: 1000,
        };

        let mut said = None;
        let quiet = "QUERY_DH Success: 0 triggers sent".to_string();

        // The first pass has something to say, and the next pass with the
        // same answer has not.
        assert!(
            note_triggered_if_new(&dh, quiet.clone(), &mut said),
            "the first line was not said"
        );
        assert!(
            !note_triggered_if_new(&dh, quiet, &mut said),
            "the same line was said twice"
        );

        // And the pass where something moved says so.
        assert!(
            note_triggered_if_new(
                &dh,
                "QUERY_DH Success: 5 triggers sent".to_string(),
                &mut said
            ),
            "a line that changed was not said"
        );

        // A payload that sends on its own is still said nothing about, and
        // leaves no mark on what was said about one that does not: the memory
        // belongs to the payload it is kept for.
        let was = said.clone();
        assert!(!note_triggered_if_new(
            &a_dh(),
            "anything".to_string(),
            &mut said
        ));
        assert_eq!(
            said, was,
            "a payload nothing is said about changed the memory"
        );
    }

    /// The startup line says the whole of what the payload will be addressed
    /// by and asked with.
    ///
    /// Every one of these is a thing the two ends can disagree about, and a
    /// panel shows only the first: the id addresses the handler, the OC
    /// address is where its data comes back to, and the trigger and interval
    /// are what tcspecial will be sending while the panel just says Active.
    #[test]
    fn the_startup_line_says_what_the_payload_will_be_asked_with() {
        let dh = DHConfig {
            dh_id: DHId(1),
            name: DHName::new("triggered-send"),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Tcp,
                address: "localhost".to_string(),
                port: 5003,
            }),
            packet_size: 12,
            oc: Some(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: "127.0.0.1".to_string(),
                port: 6003,
            }),
            mode: tcslibgs::DHMode::Triggered {
                trigger: b"go".to_vec(),
                interval_ms: 1000,
            },
        };

        let said = triggered_startup(&dh, STOPPED_STATUS).expect("a triggered payload");
        for wanted in [
            "triggered-send",
            "dh_id 1",
            // As the payload file writes it, so that the console and the file
            // can be read against each other.
            &trigger_as_written(b"go"),
            "1000 ms",
            "127.0.0.1:6003",
            STOPPED_STATUS,
        ] {
            assert!(said.contains(wanted), "{wanted:?} is not said: {said}");
        }

        // And a handler with no OC address says so rather than saying nothing:
        // it is the one fault that stops a start, and it is in this file.
        let mut without = dh.clone();
        without.oc = None;
        let said = triggered_startup(&without, STOPPED_STATUS).expect("a triggered payload");
        assert!(said.contains("OC none"), "a missing OC is not said: {said}");
    }

    /// The MOC listens where tcspecial sends, by reading the same files.
    ///
    /// Checked against the shipped configurations rather than a contrived
    /// pair, because what matters is that the two programs resolve one group
    /// and one interface out of the files as shipped. A listener that assumed
    /// an address would go deaf the moment a mission moved the beacon, and
    /// the only symptom would be an indicator saying nothing had arrived.
    #[test]
    fn the_moc_listens_where_tcspecial_sends() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let payload_path = root.join(DEFAULT_PAYLOAD_CONFIG_PATH);
        let payload_path = payload_path.to_str().expect("a path");

        // The MOC's answer, from the payload set and the command
        // interpreter's own file.
        let config_path = root.join(DEFAULT_TCSPECIAL_CONFIG_PATH);
        let config = load_tcspecial_config(&config_path)
            .unwrap_or_else(|e| panic!("{} failed to load: {e}", config_path.display()));
        let section = load_tcspecial_section(payload_path)
            .unwrap_or_else(|e| panic!("{payload_path} failed to load: {e}"));
        let mine = beacon(&config, section.as_ref()).expect("it resolves");

        // What the set itself says, which is what tcspecial will resolve.
        let said = section.expect("the shipped set has a tcspecial section");
        assert_eq!(mine.group.to_string(), said.beacon_address);
        assert_eq!(mine.interface.to_string(), said.beacon_interface);

        // And it is a group, which is what makes more than one ground station
        // possible at all.
        assert!(mine.group.ip().is_multicast(), "{} is not a group", mine.group);
    }

    /// The simulator file beside a payload file is found and read.
    ///
    /// The MOC shows what the tcssim it starts is simulating, so the two have
    /// to be looking at one file: the convention is that it sits beside the
    /// payload file and is named for it.
    #[test]
    fn the_simulator_file_beside_a_payload_file_is_read() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let payload_path = root.join(DEFAULT_PAYLOAD_CONFIG_PATH);
        let dh_configs = load_dh_configs(&payload_path).expect("the shipped payload file loads");

        // Named from the payload file, since nothing in this test's
        // environment says otherwise.
        let sims = simulator_settings(payload_path.to_str().unwrap(), &dh_configs)
            .expect("the simulator file beside the shipped payload file");
        assert_eq!(
            sims.len(),
            dh_configs.len(),
            "one settings entry per handler"
        );

        // And a payload file with no simulator file beside it costs the MOC
        // that half and nothing else: it controls payloads, it does not
        // simulate them.
        assert!(
            simulator_settings("no_such_payload_set.yaml", &dh_configs).is_none(),
            "a missing simulator file must not stop the MOC"
        );
    }

    /// Every panel shows both lines, whichever kind of payload it is.
    ///
    /// The sent line is what the spacecraft last sent to the ground, which
    /// for a payload that sends on its own is the payload's own data going
    /// up -- the thing such a panel is watched for. It was shown only for a
    /// payload that answers requests, so the panels of the payloads that send
    /// most showed least.
    ///
    /// Not a test of its own: one test per binary may start the testing
    /// backend, and its windows belong to the thread that made them.
    fn every_panel_shows_both_sample_lines() {
        use i_slint_backend_testing::ElementHandle;

        let shown = |info: DHInfo| {
            let ui = MainWindow::new().unwrap();
            ui.set_dh_model(ModelRc::from(Rc::new(VecModel::from(vec![info]))));
            ui.set_columns(1);
            ui.show().unwrap();
            ElementHandle::find_by_element_type_name(&ui, "Text")
                .filter_map(|e| e.accessible_label())
                .map(|label| label.to_string())
                .collect::<Vec<_>>()
        };

        let mut dh = DHConfig {
            dh_id: DHId(0),
            name: DHName::new("DH0"),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Tcp,
                address: "localhost".to_string(),
                port: 5000,
            }),
            packet_size: 12,
            oc: None,
            mode: tcslibgs::DHMode::Periodic,
        };

        for kind in ["sends on its own", "answers requests"] {
            let lines = shown(dh_info_from(&dh, None));
            for line in ["Last sent:", "Last rcvd:", "From payload:"] {
                assert!(
                    lines.iter().any(|shown| shown == line),
                    "a payload that {kind} has no {line} line: {lines:?}"
                );
            }

            dh.mode = tcslibgs::DHMode::Triggered {
                trigger: b"READ\r".to_vec(),
                interval_ms: 500,
            };
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
                .any(|line| line == beacon_receive::NO_BEACON_MESSAGE),
            "the beacon box does not say that nothing has been said: {shown:?}"
        );

        // And after one, the time it arrived and what it said.
        beacon_receive::show_beacon(
            &ui,
            Some(std::time::UNIX_EPOCH + Duration::from_secs(7322)),
            slint::Color::from_rgb_u8(0, 255, 0),
            "version 0.1.0, md5 0123456789abcdef",
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

    /// Pressing a panel's Configuration button shows what both configuration files
    /// said about that handler.
    ///
    /// Everything up to the window is checked elsewhere -- the text itself by
    /// `tcslibgs::payload_parameters` -- and none of it puts anything on
    /// screen if the button is not wired to the popup or the popup not to the
    /// text. This presses the button and reads what the window then laid out.
    ///
    /// Not a test of its own: one test per binary may start the testing
    /// backend, and its windows belong to the thread that made them.
    fn the_params_button_shows_what_both_files_said() {
        use i_slint_backend_testing::ElementHandle;

        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(DEFAULT_PAYLOAD_CONFIG_PATH);
        let dh_configs = load_dh_configs(&path).expect("the shipped payload file loads");
        let dh = &dh_configs[0];

        // Without the simulator file, which is the case this program has to
        // handle: it controls payloads and does not simulate them.
        let ui = MainWindow::new().unwrap();
        ui.set_dh_model(ModelRc::from(Rc::new(VecModel::from(vec![dh_info_from(dh, None)]))));
        ui.set_columns(1);
        ui.show().unwrap();

        let on_screen = |ui: &MainWindow| -> Vec<String> {
            ElementHandle::find_by_element_type_name(ui, "Text")
                .filter_map(|e| e.accessible_label())
                .map(|label| label.to_string())
                .collect()
        };

        // Nothing of it is on screen until the button is pressed.
        assert!(
            !on_screen(&ui).iter().any(|line| line.contains("dh_id")),
            "the parameters are shown before anyone asked for them"
        );

        let button: Vec<_> = ElementHandle::find_by_accessible_label(&ui, "Configuration").collect();
        assert_eq!(button.len(), 1, "the panel has no Configuration button to press");
        button[0].invoke_accessible_default_action();

        let shown = on_screen(&ui);
        assert!(
            shown.iter().any(|line| *line == format!("{} parameters", dh.name.0)),
            "the popup does not say which handler it is describing: {shown:?}"
        );
        assert!(
            shown.iter().any(|line| *line == payload_parameters(dh, None)),
            "the popup shows something other than this handler's parameters:\n{shown:?}"
        );
        assert!(
            shown
                .iter()
                .any(|line| line.contains("not read by this program")),
            "a program without the simulator file must say so rather than \
             leaving that half unexplained:\n{shown:?}"
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
        let shape = grid_shape(4);
        let size = window_size(&shape, 4);
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

    /// The sizes in the window and the ones Rust works them out from.
    ///
    /// Three numbers in `ui/main.slint` are the same numbers as here: the two
    /// floors the window may not go below, and the height the grid claims per
    /// row. Nothing but the comments on them has ever said so, and a window
    /// that disagrees goes wrong quietly -- it opens at a size its own layout
    /// will not keep, or claims room per row that the opening size did not
    /// allow for.
    #[test]
    fn the_window_and_rust_agree_on_the_sizes() {
        let window = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/main.slint");
        let source = std::fs::read_to_string(&window).unwrap();

        for (what, expected) in [
            ("min-width", min_window_width()),
            // The least the window is ever opened at: one row of panels.
            ("min-height", height_for_rows(1)),
            ("panel-height", PANEL_HEIGHT),
            ("grid-spacing", GRID_SPACING),
        ] {
            let said = format!("{}: {}px;", what, expected);
            assert!(
                source.contains(&said),
                "{} does not say {:?}",
                window.display(),
                said
            );
        }
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

    /// What the window's one panel button says each way.
    ///
    /// Here rather than beside the statuses because the window owns its
    /// labels: Slint builds the string, and these are how the test below
    /// holds it to the same rule Rust decides a press by.
    const DISCARD_LABEL: &str = "Discard";
    const TRANSMIT_LABEL: &str = "Transmit";

    impl Transfer {
        /// The label a button offering this carries.
        fn label(&self) -> &'static str {
            match self {
                Transfer::Transmit => TRANSMIT_LABEL,
                Transfer::Discard => DISCARD_LABEL,
            }
        }
    }

    /// A panel's one button offers what pressing it will do, and Rust sends
    /// what the label offered.
    ///
    /// There were two buttons, Start and Stop, and either was pressable
    /// whatever the handler was doing: a Stop on a stopped handler asks
    /// tcspecial to stop something that is not running, and reads as though
    /// it had done something. One button offers the one thing worth doing
    /// next -- and because Slint decides the label and Rust decides the
    /// command, both from the same status, the two have to be checked against
    /// each other or they drift into offering one thing and doing the other.
    #[test]
    fn the_transfer_button_sends_what_its_label_offers() {
        // A handler that is moving data offers to stop it, and one that is
        // not offers to start it. Anything that is not the running status
        // offers to transmit: a handler whose last command failed shows
        // Error, and the useful thing to offer then is the start that failed.
        assert_eq!(transfer_wanted(ACTIVE_STATUS), Transfer::Discard);
        assert_eq!(transfer_wanted(STOPPED_STATUS), Transfer::Transmit);
        assert_eq!(transfer_wanted(ERROR_STATUS), Transfer::Transmit);
        assert_eq!(transfer_wanted(""), Transfer::Transmit);

        assert_eq!(Transfer::Discard.label(), DISCARD_LABEL);
        assert_eq!(Transfer::Transmit.label(), TRANSMIT_LABEL);

        // And the window labels it by the same rule, which is the half Rust
        // cannot fail on: the label is a Slint expression, so a drift between
        // the two files is a button that offers Transmit and stops the
        // handler.
        let window = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/main.slint");
        let source = std::fs::read_to_string(&window).unwrap();
        let offered = format!(
            "dh-status == \"{}\" ? \"{}\" : \"{}\"",
            ACTIVE_STATUS,
            transfer_wanted(ACTIVE_STATUS).label(),
            transfer_wanted(STOPPED_STATUS).label()
        );
        assert!(
            source.contains(&offered),
            "{} does not label its transfer button {:?}",
            window.display(),
            offered
        );

        // One button, not two: the pair they replaced took the same room and
        // left a press available that said nothing.
        for gone in ["text: \"Start\"", "text: \"Stop\""] {
            assert!(
                !source.contains(gone),
                "{} still has a panel button saying {gone}",
                window.display()
            );
        }
    }

    /// A panel's two lines, from the sample telemetry that feeds them.
    /// A gathered update reaches the panel it names, and only that one.
    #[test]
    fn an_update_goes_to_its_own_panel() {
        let dh = DHConfig {
            dh_id: DHId(1),
            name: DHName::new("DH1"),
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/null".to_string(),
            }),
            packet_size: 1,
            oc: None,
                    mode: Default::default(),
        };

        let model: Rc<VecModel<DHInfo>> = Rc::new(VecModel::from(vec![
            dh_info_from(&dh, None),
            dh_info_from(&dh, None),
        ]));

        apply_panel_update(
            &model,
            &PanelUpdate {
                row: 1,
                bytes_sent: 7,
                bytes_recv: 9,
                last_sent_time: SharedString::from("01:02:03"),
                last_sent: SharedString::from("AA BB"),
                last_recv_time: SharedString::from("04:05:06"),
                last_recv: SharedString::from("CC"),
            },
        );

        let touched = model.row_data(1).unwrap();
        assert_eq!(touched.bytes_sent, 7);
        assert_eq!(touched.bytes_recv, 9);
        assert_eq!(touched.last_sent_time, SharedString::from("01:02:03"));
        assert_eq!(touched.last_recv, SharedString::from("CC"));

        // Row 0 was not named, so it is untouched.
        let other = model.row_data(0).unwrap();
        assert_eq!(other.bytes_sent, 0);
        assert_eq!(other.last_sent_time, SharedString::from(NO_TRANSFER_TIME));
    }

    /// A handler that answers the statistics but not the samples keeps the
    /// sample lines it had, rather than having them blanked.
    ///
    /// A blank line reads as a handler that has moved nothing, which is a
    /// different thing from one that did not answer the question.
    #[test]
    fn a_missing_sample_leaves_the_previous_one_showing() {
        let previous = PanelUpdate {
            row: 0,
            bytes_sent: 1,
            bytes_recv: 2,
            last_sent_time: SharedString::from("11:22:33"),
            last_sent: SharedString::from("DE AD"),
            last_recv_time: SharedString::from("44:55:66"),
            last_recv: SharedString::from("BE EF"),
        };

        let dh = DHConfig {
            dh_id: DHId(0),
            name: DHName::new("DH0"),
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/null".to_string(),
            }),
            packet_size: 1,
            oc: None,
                    mode: Default::default(),
        };

        let model: Rc<VecModel<DHInfo>> = Rc::new(VecModel::from(vec![dh_info_from(&dh, None)]));
        apply_panel_update(&model, &previous);

        // What panel_update_for does when the sample query fails: carry the
        // previous strings through onto the new statistics.
        let carried = PanelUpdate {
            bytes_sent: 10,
            bytes_recv: 20,
            ..previous.clone()
        };
        apply_panel_update(&model, &carried);

        let shown = model.row_data(0).unwrap();
        assert_eq!(shown.bytes_sent, 10, "the new statistics are shown");
        assert_eq!(
            shown.last_sent, previous.last_sent,
            "the sample lines are the ones last seen"
        );
        assert_eq!(shown.last_recv_time, previous.last_recv_time);
    }

    #[test]
    fn a_sample_becomes_a_time_and_a_row_of_bytes() {
        let mut sample = DHSample::new();
        sample.record(&[0x01, 0xAB, 0xFF]);
        // The time is the telemetry's, so pin it rather than reading a clock.
        sample.time = Some(Timestamp {
            seconds: 3661,
            nanoseconds: 0,
        });

        let (time, data) = sample_lines(&sample);
        assert_eq!(time, SharedString::from("01:01:01"));
        assert_eq!(data, SharedString::from("01 AB FF"));
    }

    /// A direction that has carried nothing shows the placeholder, not a time.
    ///
    /// A handler that has never moved data and one whose clock read zero must
    /// not look alike, which is why the sample's time is an Option rather than
    /// a zero.
    #[test]
    fn a_direction_that_has_carried_nothing_shows_no_time() {
        let (time, data) = sample_lines(&DHSample::new());
        assert_eq!(time, SharedString::from(NO_TRANSFER_TIME));
        assert!(data.is_empty());
    }

    /// The placeholder a fresh panel shows is the one an empty sample gives,
    /// so a panel does not change appearance on its first query.
    #[test]
    fn a_fresh_panel_and_an_empty_sample_agree() {
        let dh = DHConfig {
            dh_id: DHId(0),
            name: DHName::new("DH0"),
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/null".to_string(),
            }),
            packet_size: 1,
            oc: None,
                    mode: Default::default(),
        };

        let info = dh_info_from(&dh, None);
        let (time, data) = sample_lines(&DHSample::new());
        assert_eq!(info.last_sent_time, time);
        assert_eq!(info.last_recv_time, time);
        assert_eq!(info.last_sent, data);
        assert_eq!(info.last_recv, data);
    }

    /// Longer data is shown as a head with an ellipsis rather than being cut
    /// off silently.
    #[test]
    fn a_truncated_sample_says_so() {
        let mut sample = DHSample::new();
        sample.record(&(0..64).collect::<Vec<u8>>());
        sample.time = Some(Timestamp {
            seconds: 0,
            nanoseconds: 0,
        });

        let (_, data) = sample_lines(&sample);
        assert!(
            data.ends_with("..."),
            "a sample filling the buffer should show as truncated: {data}"
        );
    }

    /// Something answering as tcspecial is found, and nothing is not.
    ///
    /// The check is a PING over the loopback, so it can be tested without a
    /// tcspecial: a socket that answers one is indistinguishable from the real
    /// thing as far as this question goes, which is the point of asking by
    /// command rather than by looking for a process.
    #[test]
    fn an_answering_tcspecial_is_found_and_silence_is_not() {
        use std::net::UdpSocket;
        use tcslibgs::{Command, PingTelemetry, Telemetry};

        // Nothing listening: the MOC's job is to start one.
        let quiet = UdpSocket::bind("127.0.0.1:0").unwrap();
        let quiet_addr = quiet.local_addr().unwrap().to_string();
        drop(quiet);
        assert!(
            !tcspecial_already_running(&quiet_addr),
            "an address nothing answers on should read as nothing running"
        );

        // A socket that answers a PING, which is all the question asks.
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = socket.local_addr().unwrap().to_string();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let stub = thread::spawn(move || {
            let mut buffer = vec![0u8; 65535];
            let (size, from) = match socket.recv_from(&mut buffer) {
                Ok(received) => received,
                Err(_) => return,
            };
            let command: Command = serde_json::from_slice(&buffer[..size]).expect("a command");
            let answer = Telemetry::Ping(PingTelemetry::new(
                command.sequence(),
                CommandStatus::Success,
            ));
            let _ = socket.send_to(&serde_json::to_vec(&answer).unwrap(), from);
        });

        assert!(
            tcspecial_already_running(&addr),
            "an address that answers a ping should read as a tcspecial running"
        );
        stub.join().unwrap();
    }

    /// Each child is started on the payload file the MOC read, and told so by
    /// argument.
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

    /// The shipped payload file has to be one the MOC can show.
    ///
    /// Nothing else checks that: until this test, the window's data handlers
    /// were transcribed from the file by hand, and a file that had moved on
    /// showed up only as a panel saying the wrong thing.
    #[test]
    fn the_shipped_payload_config_becomes_dh_panels() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(DEFAULT_PAYLOAD_CONFIG_PATH);
        let dh_configs = load_dh_configs(&path)
            .unwrap_or_else(|e| panic!("{} failed to load: {e}", path.display()));

        assert!(!dh_configs.is_empty(), "no data handlers to show");

        for (dh, info) in dh_configs.iter().zip(dh_configs.iter().map(|dh| dh_info_from(dh, None))) {
            assert_eq!(info.name, SharedString::from(dh.name.0.clone()));
            assert!(!info.config.is_empty(), "{} has no endpoint to show", dh.name.0);
            // A panel showing a saturated size would be showing a number the
            // file does not contain.
            assert_eq!(info.packet_size as usize, dh.packet_size);
        }
    }

    /// The panels and the configured data handlers line up by index, which is
    /// what lets a panel's row number name a data handler.
    #[test]
    fn a_panel_row_names_the_handler_at_that_index() {
        let dh_configs = vec![
            DHConfig {
                dh_id: DHId(7),
                name: DHName::new("DH7"),
                endpoint: EndpointConfig::Device(DeviceConfig {
                    path: "/dev/urandom".to_string(),
                }),
                packet_size: 4,
                oc: None,
                            mode: Default::default(),
            },
            DHConfig {
                dh_id: DHId(9),
                name: DHName::new("DH9"),
                endpoint: EndpointConfig::Network(NetworkConfig {
                    protocol: NetworkProtocol::Udp,
                    address: "localhost".to_string(),
                    port: 5009,
                }),
                packet_size: 8,
                oc: None,
                            mode: Default::default(),
            },
        ];

        let model: Vec<DHInfo> = dh_configs.iter().map(|dh| dh_info_from(dh, None)).collect();

        assert_eq!(model.len(), dh_configs.len());
        // Row 1 is DH9, whose id is neither 1 nor its row number.
        assert_eq!(model[1].name, SharedString::from("DH9"));
        assert_eq!(dh_configs[1].dh_id, DHId(9));
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
            let shape = grid_shape(panels);

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
            let shape = grid_shape(panels);
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
            let shape = grid_shape(panels);
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

    /// The shipped configuration opens a row of three and a row of one.
    ///
    /// The other tests fix the rules; this one records what the rules actually
    /// produce for the file the MOC ships with, so a change to the panel
    /// constants or the column count that ruins the shipped case cannot pass
    /// quietly.
    #[test]
    fn the_shipped_payload_config_fills_a_row_of_three() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(DEFAULT_PAYLOAD_CONFIG_PATH);
        let dh_configs = load_dh_configs(&path)
            .unwrap_or_else(|e| panic!("{} failed to load: {e}", path.display()));
        assert_eq!(dh_configs.len(), 4, "{} has moved on", path.display());

        let shape = grid_shape(dh_configs.len());
        assert_eq!(
            (shape.columns, shape.rows),
            (3, 2),
            "{} opens a {}x{} grid",
            path.display(),
            shape.columns,
            shape.rows
        );
        assert!(
            shape.width >= shape.height,
            "{} opens a {}x{} window, taller than wide",
            path.display(),
            shape.width,
            shape.height
        );
    }

    /// An unreadable or empty configuration still produces a window.
    #[test]
    fn no_panels_still_has_a_shape() {
        let shape = grid_shape(0);
        assert_eq!(shape.columns, 1);
        assert_eq!(shape.rows, 1);
        assert!(shape.width >= shape.height);
    }
}
