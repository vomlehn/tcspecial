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
use tcslibgs::config::{load_dh_configs, payload_path_from_args};
use tcslibgs::{ArmKey, CommandStatus, DHConfig, DHSample, DH_SAMPLE_BYTES};
use tcspecial::config::constants::BEACON_NETADDR;

use crate::beacon_receive::BeaconReceive;
use crate::ci_link::{CiLink, NOT_CONNECTED};
use crate::endpoint::{dh_type_of, endpoint_description};
use crate::config::constants::BEACON_INDICATOR;

slint::include_modules!();

mod app;
mod beacon_receive;
mod ci_link;
mod config;
mod endpoint;

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

/// Shown for the time of a transfer that has not happened.
///
/// The same text `ui/main.slint` defaults its two time lines to, so a panel
/// reads alike before any query and after one that found nothing moved.
const NO_TRANSFER_TIME: &str = "--:--:--";

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

    let mut client = TcsClient::new(Box::new(connection));
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
/// Tcssim's simulator configuration is not here: the MOC never reads it and so
/// has nothing to say about which one is right, and tcssim takes it from
/// `PAYLOAD_SIM_YAML`, inherited from the MOC's environment like any other
/// variable.
const CHILDREN: [&str; 2] = ["tcspecial", "tcssim"];

/// A panel's nominal size, and the height of everything above and below the
/// grid of them.
///
/// These are used only to choose how many columns the grid has and how large
/// the window opens; the layout itself stretches panels to fit. They have to
/// agree with `ui/main.slint`, which sizes the grid with the same panel
/// height.
const PANEL_WIDTH: f32 = 320.0;
/// What a panel's contents come to: the sum of the floors its rows declare,
/// measured through Slint's testing backend rather than guessed, by
/// `every_panels_data_is_inside_the_window`. This was 210 against the 259 the
/// panel took when its two readings each sat in a group box of their own, so
/// the window opened too small for every row and the three rows of data at
/// the bottom of each panel went over the edge. Those readings are labelled
/// lines now, which is most of the difference.
const PANEL_HEIGHT: f32 = 162.0;
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
/// open in a 3x2 grid with two cells empty and two panels would go side by
/// side instead of stacked. At 700 every panel count keeps the shape it has
/// always had. Anything that changes the panel or chrome heights has to be
/// weighed the same way, against
/// `the_shipped_payload_config_opens_a_nearly_square_window`.
const MIN_WINDOW_WIDTH: f32 = 700.0;

/// How the grid of data handler panels is shaped, and the window size that
/// shape asks for.
#[derive(Debug, Clone, Copy, PartialEq)]
struct GridShape {
    columns: usize,
    rows: usize,
    width: f32,
    height: f32,
}

impl GridShape {
    /// Width over height. 1.0 is square, above it is wider than tall.
    fn aspect(&self) -> f32 {
        self.width / self.height
    }
}

/// Choose the grid shape for `panels` data handlers.
///
/// The window should be wider than tall but no wider than it has to be, so of
/// the shapes that are at least square the squarest one wins. Taller-than-wide
/// shapes are not candidates at all, which is what makes the bias a rule
/// rather than a tendency.
fn grid_shape(panels: usize) -> GridShape {
    // An empty configuration still has to produce a window.
    let panels = panels.max(1);

    let shape_for = |columns: usize| {
        let rows = panels.div_ceil(columns);
        GridShape {
            columns,
            rows,
            width: (columns as f32 * PANEL_WIDTH).max(MIN_WINDOW_WIDTH),
            height: height_for_rows(rows),
        }
    };

    // One column per panel is always at least square -- a single row is only
    // CHROME_HEIGHT + PANEL_HEIGHT tall, with no gap between rows to allow
    // for, and at least MIN_WINDOW_WIDTH wide --
    // so there is always a candidate, and the fallback is unreachable unless
    // those constants change.
    (1..=panels)
        .map(shape_for)
        .filter(|shape| shape.width >= shape.height)
        .min_by(|a, b| a.aspect().total_cmp(&b.aspect()))
        .unwrap_or_else(|| shape_for(panels))
}

/// Turn a data handler from the payload configuration file into the panel the
/// window shows for it.
///
/// The configured values fill in what the file knows; the run-time fields
/// start empty and are filled in as tcspecial reports on the handler. There is
/// no packet interval among them: how often a payload produces a packet is a
/// property of a simulation rather than of a payload, so it is stated in the
/// simulator's own file and tcssim is what shows it.
fn dh_info_from(dh: &DHConfig) -> DHInfo {
    DHInfo {
        name: SharedString::from(dh.name.0.clone()),
        config: SharedString::from(endpoint_description(&dh.endpoint)),
        packet_size: i32::try_from(dh.packet_size).unwrap_or(i32::MAX),
        status: SharedString::from("Stopped"),
        last_sent_time: SharedString::from(NO_TRANSFER_TIME),
        last_sent: SharedString::new(),
        last_recv_time: SharedString::from(NO_TRANSFER_TIME),
        last_recv: SharedString::new(),
        bytes_sent: 0,
        bytes_recv: 0,
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
fn panel_update_for(
    client: &mut TcsClient,
    row: usize,
    dh: &DHConfig,
    previous: Option<&PanelUpdate>,
) -> Option<PanelUpdate> {
    let (_, stats) = client.query_dh(dh.dh_id).ok()?;

    // A second command, because the statistics every poll asks for do not
    // carry payload bytes.
    let samples = client.query_dh_sample(dh.dh_id).ok();

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
    match sample.time {
        Some(time) => {
            // The sample truncated the transfer, so bytes_to_hex is never the
            // one doing it here; the ellipsis comes from the length the sample
            // kept of the whole.
            let mut data = app::bytes_to_hex(sample.data(), DH_SAMPLE_BYTES);
            if sample.was_truncated() {
                data.push_str("...");
            }
            (
                SharedString::from(app::format_timestamp(time.seconds, time.nanoseconds)),
                SharedString::from(data),
            )
        }
        None => (
            SharedString::from(NO_TRANSFER_TIME),
            SharedString::new(),
        ),
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
/// The child is told which payload file to read, as an argument. Tcsmoc builds
/// its panels from that file, so a child reading a different one would serve
/// or simulate payloads the panels do not describe -- handlers that never
/// connect, with nothing on screen to say why. An argument also beats anything
/// the child would have taken from the environment, so for the run of a
/// payload set the MOC's own file is the one that counts.
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

    // One panel per configured data handler.
    let dh_model: Rc<VecModel<DHInfo>> =
        Rc::new(VecModel::from(dh_configs.iter().map(dh_info_from).collect::<Vec<_>>()));
    ui.set_dh_model(ModelRc::from(dh_model.clone()));

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

    // Open the link to the command interpreter on startup, by the same call
    // the Connect button makes, so the link the window comes up with is the
    // one that button would have given it.
    let mut link = CiLink::down();
    if let Err(e) = link.connect(DEFAULT_CI_ADDRESS) {
        eprintln!("Failed to connect to {}: {}", DEFAULT_CI_ADDRESS, e);
        ui.set_ci_status(SharedString::from(ERROR_STATUS));
        ui.set_last_response(SharedString::from(format!("Connection failed: {}", e)));
        // Exit since we can't operate without a connection
        exit(1);
    }
    eprintln!("Connected to {}", DEFAULT_CI_ADDRESS);
    ui.set_ci_status(SharedString::from(CONNECTED_STATUS));
    ui.set_ci_address(SharedString::from(DEFAULT_CI_ADDRESS));
    let link: Arc<Mutex<CiLink>> = Arc::new(Mutex::new(link));

    // Start receiving beacon data
    let beacon_addr: std::net::SocketAddr = BEACON_NETADDR.parse().unwrap();
    let beacon_ui_weak = ui_weak.clone();
    let _beacon_receive = BeaconReceive::new(beacon_ui_weak, beacon_addr, BEACON_INDICATOR.clone());

    handle_link_button(&ui, ui_weak.clone(), link.clone());
    handle_main_menu(&ui, ui_weak.clone(), link.clone());
    query_dh_buttons(&ui, ui_weak.clone(), link.clone(), dh_configs.clone(), dh_model.clone());
    poll_panels(link.clone(), dh_configs.clone(), dh_model.clone());
    start_dh_handler(&ui, ui_weak.clone(), link.clone(), dh_configs.clone(), dh_model.clone());
    stop_dh_handler(&ui, ui_weak.clone(), link.clone(), dh_configs.clone(), dh_model.clone());
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
            match panel_update_for(client, row, dh, None) {
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
) {
    let pending: Arc<Mutex<Vec<PanelUpdate>>> = Arc::new(Mutex::new(Vec::new()));

    {
        let pending = pending.clone();
        thread::spawn(move || {
            // What each panel last showed, so a handler that answers the
            // statistics but not the samples keeps its sample lines.
            let mut last: Vec<Option<PanelUpdate>> = vec![None; dh_configs.len()];

            // The poller's own link, kept wherever the window's link is.
            let mut poll_link = CiLink::down();

            loop {
                thread::sleep(PANEL_POLL_INTERVAL);

                let gathered = poll_pass(&link, &mut poll_link, &dh_configs, &mut last);

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
fn poll_pass(
    link: &Mutex<CiLink>,
    poll_link: &mut CiLink,
    dh_configs: &[DHConfig],
    last: &mut [Option<PanelUpdate>],
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
        if let Some(update) = panel_update_for(client, row, dh, last[row].as_ref()) {
            last[row] = Some(update.clone());
            gathered.push(update);
        }
    }
    gathered
}

// Start DH handler
//
// The panel passes its own row; the identity, name, and type of the handler
// that row stands for come from the configuration file rather than from the
// row number.
fn start_dh_handler(
    ui: &MainWindow,
    ui_weak: slint::Weak<MainWindow>,
    link: Arc<Mutex<CiLink>>,
    dh_configs: Arc<Vec<DHConfig>>,
    dh_model: Rc<VecModel<DHInfo>>,
) {
    ui.on_start_dh(move |row| {
        let ui = ui_weak.unwrap();
        let row = row as usize;
        let dh = match dh_configs.get(row) {
            Some(dh) => dh,
            None => return,
        };

        let mut guard = link.lock().unwrap();
        let sent = match guard.client() {
            Some(client) => client.start_dh(dh.dh_id, dh_type_of(&dh.endpoint), dh.name.clone()),
            None => {
                ui.set_last_response(SharedString::from(NOT_CONNECTED));
                return;
            }
        };
        match sent {
            Ok(status) => {
                let status_str = if status == CommandStatus::Success {
                    "Active"
                } else {
                    "Error"
                };
                update_row(&dh_model, row, |info| {
                    info.status = SharedString::from(status_str);
                });
                ui.set_last_response(SharedString::from(format!(
                    "START_DH {} - {:?}",
                    dh.name.0, status
                )));
            }
            Err(e) => {
                ui.set_last_response(SharedString::from(format!(
                    "START_DH {} failed: {}",
                    dh.name.0, e
                )));
            }
        }
    });
}

// Stop DH handler
fn stop_dh_handler(
    ui: &MainWindow,
    ui_weak: slint::Weak<MainWindow>,
    link: Arc<Mutex<CiLink>>,
    dh_configs: Arc<Vec<DHConfig>>,
    dh_model: Rc<VecModel<DHInfo>>,
) {
    ui.on_stop_dh(move |row| {
        let ui = ui_weak.unwrap();
        let row = row as usize;
        let dh = match dh_configs.get(row) {
            Some(dh) => dh,
            None => return,
        };

        let mut guard = link.lock().unwrap();
        let sent = match guard.client() {
            Some(client) => client.stop_dh(dh.dh_id),
            None => {
                ui.set_last_response(SharedString::from(NOT_CONNECTED));
                return;
            }
        };
        match sent {
            Ok(status) => {
                update_row(&dh_model, row, |info| {
                    info.status = SharedString::from("Stopped");
                });
                ui.set_last_response(SharedString::from(format!(
                    "STOP_DH {} - {:?}",
                    dh.name.0, status
                )));
            }
            Err(e) => {
                ui.set_last_response(SharedString::from(format!(
                    "STOP_DH {} failed: {}",
                    dh.name.0, e
                )));
            }
        }
    });
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
    use std::ffi::OsStr;
    use std::path::Path;
    use tcslibgs::{EndpointConfig, NetworkProtocol};
    use std::time::Instant;
    use tcslibgs::config::DEFAULT_PAYLOAD_CONFIG_PATH;
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
        let link = Arc::new(Mutex::new(CiLink::down()));
        link.lock().unwrap().connect(UNANSWERED_ADDRESS).unwrap();

        let pass = {
            let link = link.clone();
            thread::spawn(move || {
                let dh_configs = vec![a_dh()];
                let mut poll_link = CiLink::down();
                let mut last = vec![None];

                let started = Instant::now();
                let gathered = poll_pass(&link, &mut poll_link, &dh_configs, &mut last);
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
        let shipped: Vec<DHInfo> = dh_configs.iter().map(dh_info_from).collect();

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

            let window_height = ui.window().size().to_logical(1.0).height;

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
            ("min-width", MIN_WINDOW_WIDTH),
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
        };

        let model: Rc<VecModel<DHInfo>> = Rc::new(VecModel::from(vec![
            dh_info_from(&dh),
            dh_info_from(&dh),
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
        };

        let model: Rc<VecModel<DHInfo>> = Rc::new(VecModel::from(vec![dh_info_from(&dh)]));
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
        };

        let info = dh_info_from(&dh);
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
            let command = child_command(name, "payload2.yaml");

            assert_eq!(command.get_program(), "cargo", "{name}");
            let args: Vec<&OsStr> = command.get_args().collect();
            assert_eq!(
                args,
                ["run", "--bin", name, "--", "payload2.yaml"],
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
        let payload_path = payload_path_from_args(args(&["payload2.yaml"]), None).unwrap();

        for name in CHILDREN {
            let command = child_command(name, &payload_path);
            let args: Vec<&OsStr> = command.get_args().collect();
            assert_eq!(
                args.last(),
                Some(&OsStr::new("payload2.yaml")),
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

        for (dh, info) in dh_configs.iter().zip(dh_configs.iter().map(dh_info_from)) {
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
            },
        ];

        let model: Vec<DHInfo> = dh_configs.iter().map(dh_info_from).collect();

        assert_eq!(model.len(), dh_configs.len());
        // Row 1 is DH9, whose id is neither 1 nor its row number.
        assert_eq!(model[1].name, SharedString::from("DH9"));
        assert_eq!(dh_configs[1].dh_id, DHId(9));
    }

    /// How many panel counts to check the grid rules against. Well past any
    /// plausible payload configuration, so a rule that holds only for small
    /// counts does not pass.
    const COUNTS: usize = 64;

    /// The window is never taller than it is wide.
    ///
    /// This is the bias, and it has to hold for every panel count rather than
    /// for the shipped one: a handler added to the configuration file must not
    /// turn the window into a column.
    #[test]
    fn the_window_is_always_at_least_square() {
        for panels in 1..=COUNTS {
            let shape = grid_shape(panels);
            assert!(
                shape.width >= shape.height,
                "{panels} panels gives {}x{}, taller than wide",
                shape.width,
                shape.height
            );
        }
    }

    /// Of the shapes that are at least square, the chosen one is the squarest.
    ///
    /// Checked against every column count rather than against a remembered
    /// answer, so the rule is what is tested, not the arithmetic of one case.
    #[test]
    fn no_other_column_count_is_nearer_square() {
        for panels in 1..=COUNTS {
            let chosen = grid_shape(panels);

            for columns in 1..=panels {
                let rows = panels.div_ceil(columns);
                let width = (columns as f32 * PANEL_WIDTH).max(MIN_WINDOW_WIDTH);
                let height = height_for_rows(rows);

                // A taller-than-wide shape is not a candidate, however square.
                if width < height {
                    continue;
                }

                assert!(
                    chosen.aspect() <= width / height,
                    "{panels} panels chose {} columns (aspect {}), but {} columns \
                     is nearer square (aspect {})",
                    chosen.columns,
                    chosen.aspect(),
                    columns,
                    width / height
                );
            }
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

    /// The shipped configuration opens a window that is nearly square.
    ///
    /// The other tests fix the rules; this one records what the rules actually
    /// produce for the file the MOC ships with, so a change to the panel
    /// constants that ruins the shipped case cannot pass quietly.
    #[test]
    fn the_shipped_payload_config_opens_a_nearly_square_window() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(DEFAULT_PAYLOAD_CONFIG_PATH);
        let dh_configs = load_dh_configs(&path)
            .unwrap_or_else(|e| panic!("{} failed to load: {e}", path.display()));

        let shape = grid_shape(dh_configs.len());
        assert!(
            shape.aspect() < 1.1,
            "{} opens a {}x{} window, aspect {}, further from square than it \
             should be",
            path.display(),
            shape.width,
            shape.height,
            shape.aspect()
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
