//! TCSpecial Mission Operations Center (tcsmoc)
//!
//! A GUI application for testing and visualizing tcspecial operation.

pub mod client;

use slint::{LogicalSize, Model, ModelRc, SharedString, VecModel};
use std::env;
use std::process::{Child, Command, exit};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub use crate::client::TcsClient;
use tcslib::UdpConnection;
use tcslibgs::config::{
    load_payload_config, DEFAULT_PAYLOAD_CONFIG_PATH, PAYLOAD_CONFIG_PATH_VAR,
    SIM_PAYLOAD_CONFIG_PATH_VAR,
};
use tcslibgs::{
    ArmKey, CommandStatus, DHConfig, DHSample, DHType, EndpointConfig, NetworkProtocol,
    DH_SAMPLE_BYTES,
};
use tcspecial::config::constants::BEACON_NETADDR;

use crate::beacon_receive::BeaconReceive;
use crate::config::constants::BEACON_INDICATOR;

slint::include_modules!();

mod app;
mod beacon_receive;
mod config;

/// Default CI address
const DEFAULT_CI_ADDRESS: &str = "127.0.0.1:4000";

/// Shown for the time of a transfer that has not happened.
///
/// The same text `ui/main.slint` defaults its two time lines to, so a panel
/// reads alike before any query and after one that found nothing moved.
const NO_TRANSFER_TIME: &str = "--:--:--";

/// The MOC's children, and the variable each one names its payload
/// configuration with.
///
/// Both names come from tcslibgs, beside the loader that reads the file, so
/// the name set here is the one the child reads. Tcssim's simulator
/// configuration is not among these: the MOC never reads it and so has
/// nothing to say about which one is right, and tcssim takes it from
/// `PAYLOAD_SIM_YAML`, inherited from the MOC's environment like any other
/// variable.
const CHILDREN: [(&str, &str); 2] = [
    ("tcspecial", PAYLOAD_CONFIG_PATH_VAR),
    ("tcssim", SIM_PAYLOAD_CONFIG_PATH_VAR),
];

/// The payload configuration file named on the command line.
///
/// The MOC takes this as an argument rather than from the environment because
/// it starts tcspecial and tcssim as subprocesses, which inherit its
/// environment: a variable naming the MOC's file would name theirs too, and
/// could not point the MOC at one file and its children at another. An
/// argument belongs to the MOC alone.
///
/// One argument is expected, and more than one is refused rather than ignored,
/// because a second path is more likely a mistake about which file is being
/// read than something meant to have no effect.
fn payload_path_from_args<I: Iterator<Item = String>>(mut args: I) -> Result<String, String> {
    let program = args.next().unwrap_or_else(|| "tcsmoc".to_string());
    let path = match args.next() {
        Some(path) => path,
        None => return Ok(DEFAULT_PAYLOAD_CONFIG_PATH.to_string()),
    };

    match args.next() {
        Some(extra) => Err(format!(
            "unexpected argument \"{}\"\nusage: {} [payload configuration file]",
            extra, program
        )),
        None => Ok(path),
    }
}

/// A panel's nominal size, and the height of everything above and below the
/// grid of them.
///
/// These are used only to choose how many columns the grid has and how large
/// the window opens; the layout itself stretches panels to fit. They have to
/// agree with `ui/main.slint`, which sizes the grid with the same panel
/// height.
const PANEL_WIDTH: f32 = 320.0;
const PANEL_HEIGHT: f32 = 210.0;
const CHROME_HEIGHT: f32 = 210.0;

/// The narrowest the window may open, matching `min-width` in the window
/// itself. Below this the command interpreter's controls no longer fit side by
/// side.
const MIN_WINDOW_WIDTH: f32 = 640.0;

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
            height: rows as f32 * PANEL_HEIGHT + CHROME_HEIGHT,
        }
    };

    // One column per panel is always at least square -- a single row is only
    // CHROME_HEIGHT + PANEL_HEIGHT tall and at least MIN_WINDOW_WIDTH wide --
    // so there is always a candidate, and the fallback is unreachable unless
    // those constants change.
    (1..=panels)
        .map(shape_for)
        .filter(|shape| shape.width >= shape.height)
        .min_by(|a, b| a.aspect().total_cmp(&b.aspect()))
        .unwrap_or_else(|| shape_for(panels))
}

/// How a data handler's endpoint reads in its panel.
fn endpoint_description(endpoint: &EndpointConfig) -> String {
    match endpoint {
        EndpointConfig::Network(net) => {
            let protocol = match net.protocol {
                NetworkProtocol::Tcp => "TCP",
                NetworkProtocol::Udp => "UDP",
                NetworkProtocol::UnixStream => "Unix stream",
                NetworkProtocol::UnixDgram => "Unix datagram",
            };

            match net.protocol {
                // A Unix socket is named by a path; its port means nothing.
                NetworkProtocol::UnixStream | NetworkProtocol::UnixDgram => {
                    format!("{} {}", protocol, net.address)
                }
                _ => format!("{} {}:{}", protocol, net.address, net.port),
            }
        }
        EndpointConfig::Device(dev) => format!("Device {}", dev.path),
    }
}

/// Which kind of data handler tcspecial is being asked to start.
///
/// START_DH carries the type, and the configuration file is what knows it: a
/// device handler started as a network handler is a command tcspecial cannot
/// carry out.
fn dh_type_of(endpoint: &EndpointConfig) -> DHType {
    match endpoint {
        EndpointConfig::Network(_) => DHType::Network,
        EndpointConfig::Device(_) => DHType::Device,
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
/// The child is told which payload file to read, through the variable that
/// child names its payload configuration with. Tcsmoc builds its panels from
/// that file, so a child reading a different one would serve or simulate
/// payloads the panels do not describe -- handlers that never connect, with
/// nothing on screen to say why. Setting the variable on the child's own
/// command rather than in the MOC's environment is what lets each child be
/// told separately -- the thing one inherited variable could not do -- and it
/// overrides any value inherited from the shell: for the run of a payload set,
/// the MOC's own file is the one that counts.
///
/// The command is built rather than run so that a test can read what a child
/// would be started with, without starting it. The children open windows and
/// bind fixed ports, so a test that spawned them would not be a test anyone
/// could run twice at once, or alongside a real session.
fn child_command(name: &str, payload_var: &str, payload_path: &str) -> Command {
    let mut command = Command::new("cargo");
    command
        .args(["run", "--bin", name])
        .env(payload_var, payload_path);
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
    fn start_child(&self, name: &str, payload_var: &str, payload_path: &str) {
        let child = child_command(name, payload_var, payload_path)
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
    let payload_path = match payload_path_from_args(env::args()) {
        Ok(payload_path) => payload_path,
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    };
    eprintln!("Loading payload configuration from: {}", payload_path);

    let dh_configs: Arc<Vec<DHConfig>> = match load_payload_config(&payload_path) {
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
    ui.window()
        .set_size(LogicalSize::new(shape.width, shape.height));

    // Start tcspecial and tcssim subprocesses first, each reading the payload
    // file tcsmoc read, so all three describe the same payloads.
    let [(tcspecial, tcspecial_var), (tcssim, tcssim_var)] = CHILDREN;
    let process_manager_tcspecial = Arc::new(ProcessManager::new());
    process_manager_tcspecial.start_child(tcspecial, tcspecial_var, &payload_path);
    let process_manager_tcssim = Arc::new(ProcessManager::new());
    process_manager_tcssim.start_child(tcssim, tcssim_var, &payload_path);

    eprintln!("started tcspecial and tcssim, sleeping to let them initialize");
    thread::sleep(Duration::new(2, 0));

    // Create connection and client on startup
    let client: Arc<Mutex<TcsClient>> = match UdpConnection::new("0.0.0.0:0", DEFAULT_CI_ADDRESS) {
        Ok(conn) => {
            eprintln!("Connected to {}", DEFAULT_CI_ADDRESS);
            ui.set_ci_status(SharedString::from("Connected"));
            ui.set_ci_address(SharedString::from(DEFAULT_CI_ADDRESS));
            Arc::new(Mutex::new(TcsClient::new(Box::new(conn))))
        }
        Err(e) => {
            eprintln!("Failed to connect to {}: {}", DEFAULT_CI_ADDRESS, e);
            ui.set_ci_status(SharedString::from("Error"));
            ui.set_last_response(SharedString::from(format!("Connection failed: {}", e)));
            // Exit since we can't operate without a connection
            exit(1);
        }
    };

    // Start receiving beacon data
    let beacon_addr: std::net::SocketAddr = BEACON_NETADDR.parse().unwrap();
    let beacon_ui_weak = ui_weak.clone();
    let _beacon_receive = BeaconReceive::new(beacon_ui_weak, beacon_addr, BEACON_INDICATOR.clone());

    handle_main_menu(&ui, ui_weak.clone(), client.clone());
    query_dh_buttons(&ui, ui_weak.clone(), client.clone(), dh_configs.clone(), dh_model.clone());
    start_dh_handler(&ui, ui_weak.clone(), client.clone(), dh_configs.clone(), dh_model.clone());
    stop_dh_handler(&ui, ui_weak.clone(), client.clone(), dh_configs.clone(), dh_model.clone());
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

// Menu action handler
fn handle_main_menu (ui: &MainWindow, ui_weak: slint::Weak<MainWindow>, client: Arc<Mutex<TcsClient>>) {
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

// Query all DHs button handler
//
// Which data handlers to ask about comes from the configuration file: the
// panels are its rows, so a row and its handler share an index.
fn query_dh_buttons(
    ui: &MainWindow,
    ui_weak: slint::Weak<MainWindow>,
    client: Arc<Mutex<TcsClient>>,
    dh_configs: Arc<Vec<DHConfig>>,
    dh_model: Rc<VecModel<DHInfo>>,
) {
    ui.on_query_all_clicked(move || {
        let ui = ui_weak.unwrap();
        let mut guard = client.lock().unwrap();
        let mut results = Vec::new();

        for (row, dh) in dh_configs.iter().enumerate() {
            match guard.query_dh(dh.dh_id) {
                Ok((status, stats)) => {
                    results.push(format!(
                        "{}: {:?} sent={} recv={}",
                        dh.name.0, status, stats.bytes_sent, stats.bytes_received
                    ));

                    update_row(&dh_model, row, |info| {
                        info.bytes_sent = stats.bytes_sent as i32;
                        info.bytes_recv = stats.bytes_received as i32;
                    });
                }
                Err(e) => {
                    results.push(format!("{}: Error - {}", dh.name.0, e));
                }
            }

            // The samples are a second command, because the statistics every
            // poll asks for do not carry payload bytes. A handler that cannot
            // answer leaves its two lines as they were rather than blanking
            // them: the last thing seen is better than nothing seen.
            if let Ok((_, sent, received)) = guard.query_dh_sample(dh.dh_id) {
                update_row(&dh_model, row, |info| {
                    let (time, data) = sample_lines(&sent);
                    info.last_sent_time = time;
                    info.last_sent = data;

                    let (time, data) = sample_lines(&received);
                    info.last_recv_time = time;
                    info.last_recv = data;
                });
            }
        }

        ui.set_last_response(SharedString::from(results.join("; ")));
    });
}

// Start DH handler
//
// The panel passes its own row; the identity, name, and type of the handler
// that row stands for come from the configuration file rather than from the
// row number.
fn start_dh_handler(
    ui: &MainWindow,
    ui_weak: slint::Weak<MainWindow>,
    client: Arc<Mutex<TcsClient>>,
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

        let mut guard = client.lock().unwrap();
        match guard.start_dh(dh.dh_id, dh_type_of(&dh.endpoint), dh.name.clone()) {
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
    client: Arc<Mutex<TcsClient>>,
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

        let mut guard = client.lock().unwrap();
        match guard.stop_dh(dh.dh_id) {
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
fn handle_quit(ui: &MainWindow, pm_tcssim: Arc<ProcessManager>, pm_tcspecial: Arc<ProcessManager>) {
    ui.on_quit_clicked(move || {
        kill_and_exit_all(&pm_tcssim, &pm_tcspecial);
    });
}

    // Window close handler (close box)
fn handle_close(ui: &MainWindow, pm_tcssim: Arc<ProcessManager>, pm_tcspecial: Arc<ProcessManager>) {
    ui.window().on_close_requested(move || {
        kill_and_exit_all(&pm_tcssim, &pm_tcspecial);
        slint::CloseRequestResponse::HideWindow
    });
}

fn kill_and_exit_all(pm_tcssim: &Arc<ProcessManager>, pm_tcspecial: &Arc<ProcessManager>) {
    pm_tcssim.kill();
    pm_tcspecial.kill();
    exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::path::Path;
    use tcslibgs::{DHId, DHName, DeviceConfig, NetworkConfig, Timestamp};

    /// Arguments as the program really receives them, the program's own name
    /// first.
    fn args(rest: &[&str]) -> std::vec::IntoIter<String> {
        let mut all = vec!["tcsmoc".to_string()];
        all.extend(rest.iter().map(|s| s.to_string()));
        all.into_iter()
    }

    /// A panel's two lines, from the sample telemetry that feeds them.
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

    #[test]
    fn the_payload_file_comes_from_the_command_line() {
        assert_eq!(
            payload_path_from_args(args(&["payload2.yaml"])).unwrap(),
            "payload2.yaml"
        );
    }

    #[test]
    fn no_argument_reads_the_default_payload_file() {
        assert_eq!(
            payload_path_from_args(args(&[])).unwrap(),
            DEFAULT_PAYLOAD_CONFIG_PATH
        );
    }

    /// Each child is started on the payload file the MOC read, through that
    /// child's own variable and nothing else.
    ///
    /// This is what was previously only ever confirmed by running the MOC and
    /// reading what its children printed.
    #[test]
    fn each_child_is_started_on_the_mocs_payload_file() {
        for (name, var) in CHILDREN {
            let command = child_command(name, var, "payload2.yaml");

            assert_eq!(command.get_program(), "cargo", "{name}");
            let args: Vec<&OsStr> = command.get_args().collect();
            assert_eq!(args, ["run", "--bin", name], "{name}");

            // Exactly one variable, so neither child is handed the other's
            // and none of the MOC's own environment is overridden besides.
            let envs: Vec<(&OsStr, Option<&OsStr>)> = command.get_envs().collect();
            assert_eq!(
                envs,
                [(OsStr::new(var), Some(OsStr::new("payload2.yaml")))],
                "{name}"
            );
        }
    }

    /// The file a child is started on is the one named on the command line.
    #[test]
    fn the_children_get_the_file_the_command_line_named() {
        let payload_path = payload_path_from_args(args(&["payload2.yaml"])).unwrap();

        for (name, var) in CHILDREN {
            let command = child_command(name, var, &payload_path);
            let envs: Vec<(&OsStr, Option<&OsStr>)> = command.get_envs().collect();
            assert_eq!(
                envs,
                [(OsStr::new(var), Some(OsStr::new("payload2.yaml")))],
                "{name} was not given the file the command line named"
            );
        }
    }

    /// With no argument, the children are started on the same default the MOC
    /// itself reads, so an unconfigured run has all three on one payload set.
    #[test]
    fn with_no_argument_every_program_is_on_the_same_default() {
        let payload_path = payload_path_from_args(args(&[])).unwrap();
        assert_eq!(payload_path, DEFAULT_PAYLOAD_CONFIG_PATH);

        for (name, var) in CHILDREN {
            let command = child_command(name, var, &payload_path);
            let envs: Vec<(&OsStr, Option<&OsStr>)> = command.get_envs().collect();
            assert_eq!(
                envs,
                [(
                    OsStr::new(var),
                    Some(OsStr::new(DEFAULT_PAYLOAD_CONFIG_PATH))
                )],
                "{name}"
            );
        }
    }

    /// Each child is paired with the variable that child reads.
    ///
    /// The pairing is named here rather than read back out of `CHILDREN`,
    /// because every other test in this file iterates that table and so would
    /// agree with it however it were wrong -- two entries swapped included.
    /// The final authority is each child's own reader, which lives in that
    /// child's binary and so cannot be imported; this is what can be checked
    /// from here.
    ///
    /// Two different variables is the point of them: the children inherit the
    /// MOC's environment, so one shared name would name the MOC's file and
    /// theirs at once.
    #[test]
    fn each_child_is_paired_with_the_variable_it_reads() {
        assert_eq!(
            CHILDREN,
            [
                ("tcspecial", PAYLOAD_CONFIG_PATH_VAR),
                ("tcssim", SIM_PAYLOAD_CONFIG_PATH_VAR),
            ]
        );

        let [(_, tcspecial_var), (_, tcssim_var)] = CHILDREN;
        assert_ne!(tcspecial_var, tcssim_var);
    }

    #[test]
    fn a_second_payload_file_is_refused_rather_than_ignored() {
        // Silently reading the first would leave the second looking as though
        // it had been read.
        let message = payload_path_from_args(args(&["payload1.yaml", "payload2.yaml"]))
            .expect_err("two paths must be refused");
        assert!(
            message.contains("payload2.yaml"),
            "the error should name the extra argument, but said: {message}"
        );
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
        let dh_configs = load_payload_config(&path)
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

    /// A device handler is started as a device handler, not as whatever the
    /// first panel happens to be.
    #[test]
    fn a_handler_is_started_as_the_type_its_endpoint_makes_it() {
        let device = EndpointConfig::Device(DeviceConfig {
            path: "/dev/urandom".to_string(),
        });
        assert!(dh_type_of(&device) == DHType::Device);
        assert_eq!(endpoint_description(&device), "Device /dev/urandom");

        let network = EndpointConfig::Network(NetworkConfig {
            protocol: NetworkProtocol::Tcp,
            address: "localhost".to_string(),
            port: 5000,
        });
        assert!(dh_type_of(&network) == DHType::Network);
        assert_eq!(endpoint_description(&network), "TCP localhost:5000");
    }

    /// A Unix socket is named by a path, so its panel does not append a port.
    #[test]
    fn a_unix_socket_panel_shows_no_port() {
        let unix = EndpointConfig::Network(NetworkConfig {
            protocol: NetworkProtocol::UnixStream,
            address: "/tmp/dh8".to_string(),
            port: 0,
        });
        assert_eq!(endpoint_description(&unix), "Unix stream /tmp/dh8");
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
                let height = rows as f32 * PANEL_HEIGHT + CHROME_HEIGHT;

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
        let dh_configs = load_payload_config(&path)
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
