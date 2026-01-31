//! TCSpecial Mission Operations Center (tcsmoc)
//!
//! A GUI application for testing and visualizing tcspecial operation.

use std::process::{Child, Command, exit};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use slint::{SharedString, Weak};
use tcslib::{TcsClient, UdpConnection};
use tcslibgs::{ArmKey, CommandStatus, DHId, DHName, DHType};
use tcspecial::config::constants::BEACON_NETADDR;

use crate::beacon_receive::BeaconReceive;
use crate::config::constants::BEACON_INDICATOR;

slint::include_modules!();

mod app;
mod beacon_receive;
mod config;

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

    /// Starts tcssim in a background thread and exits when it completes
    fn start_child(&self, name: &str) {
        let child = Command::new("cargo")
            .args(["run", "--bin", name])
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
    // env_logger::init();
    let ui = MainWindow::new().unwrap();
    let ui_weak = ui.as_weak();

    // Shared client state
    let client: Arc<Mutex<Option<TcsClient>>> = Arc::new(Mutex::new(None));

    // Start collecting beacon data
    let beacon_addr: std::net::SocketAddr = BEACON_NETADDR.parse().unwrap();
    let beacon_ui_weak = ui_weak.clone();
    let _beacon_receive = BeaconReceive::new(beacon_ui_weak, beacon_addr, BEACON_INDICATOR.clone());

    // Start tcssim subprocess
    let process_manager_tcspecial = Arc::new(ProcessManager::new());
    process_manager_tcspecial.start_child("tcspecial");
    let process_manager_tcssim = Arc::new(ProcessManager::new());
    process_manager_tcssim.start_child("tcssim");
    eprintln!("started tcspecial, sleeping, then starting tcssim");
    thread::sleep(Duration::new(2, 0));

    // Connect button handler
    {
        let client = client.clone();
        let ui_weak = ui_weak.clone();
        ui.on_connect_clicked(move || {
            let ui = ui_weak.unwrap();
            let address = ui.get_ci_address().to_string();

            // Parse address
            let remote_addr = if address.contains(':') {
                address.clone()
            } else {
                format!("{}:4000", address)
            };

            // Create connection
            match UdpConnection::new("0.0.0.0:0", &remote_addr) {
                Ok(conn) => {
                    let mut guard = client.lock().unwrap();
                    *guard = Some(TcsClient::new(Box::new(conn)));
                    ui.set_ci_status(SharedString::from("Connected"));
                    ui.set_last_response(SharedString::from("Connected successfully"));
                }
                Err(e) => {
                    ui.set_ci_status(SharedString::from("Error"));
                    ui.set_last_response(SharedString::from(format!("Connection failed: {}", e)));
                }
            }
        });
    }

    // Disconnect button handler
    {
        let client = client.clone();
        let ui_weak = ui_weak.clone();
        ui.on_disconnect_clicked(move || {
            let ui = ui_weak.unwrap();
            let mut guard = client.lock().unwrap();
            if let Some(ref mut c) = *guard {
                let _ = c.close();
            }
            *guard = None;
            ui.set_ci_status(SharedString::from("Disconnected"));
            ui.set_last_response(SharedString::from("Disconnected"));
        });
    }

    // Ping button handler
    {
        let client = client.clone();
        let ui_weak = ui_weak.clone();
        ui.on_ping_clicked(move || {
            let ui = ui_weak.unwrap();
            let mut guard = client.lock().unwrap();
            if let Some(ref mut c) = *guard {
                match c.ping() {
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
            } else {
                ui.set_last_response(SharedString::from("Not connected"));
            }
        });
    }

    // Menu action handler
    {
        let client = client.clone();
        let ui_weak = ui_weak.clone();
        ui.on_menu_action(move |action| {
            let ui = ui_weak.unwrap();
            let mut guard = client.lock().unwrap();

            match action {
                MenuAction::Ping => {
                    if let Some(ref mut c) = *guard {
                        match c.ping() {
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
                    } else {
                        ui.set_last_response(SharedString::from("Not connected"));
                    }
                }
                MenuAction::ArmRestart => {
                    if let Some(ref mut c) = *guard {
                        match c.restart_arm(ArmKey(0xf001adad)) {
                            Ok(status) => {
                                ui.set_last_response(SharedString::from(format!("ARM_RESTART: {:?}", status)));
                            }
                            Err(e) => {
                                ui.set_last_response(SharedString::from(format!("ARM_RESTART failed: {}", e)));
                            }
                        }
                    } else {
                        ui.set_last_response(SharedString::from("Not connected"));
                    }
                }
                MenuAction::Restart => {
                    if let Some(ref mut c) = *guard {
                        match c.restart(ArmKey(0xf001adad)) {
                            Ok(status) => {
                                ui.set_last_response(SharedString::from(format!("RESTART: {:?}", status)));
                            }
                            Err(e) => {
                                ui.set_last_response(SharedString::from(format!("RESTART failed: {}", e)));
                            }
                        }
                    } else {
                        ui.set_last_response(SharedString::from("Not connected"));
                    }
                }
                MenuAction::Query => {
                    ui.set_last_response(SharedString::from("Query not yet implemented"));
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
    {
        let client = client.clone();
        let ui_weak = ui_weak.clone();
        ui.on_query_all_clicked(move || {
            let ui = ui_weak.unwrap();
            let mut guard = client.lock().unwrap();
            if let Some(ref mut c) = *guard {
                let mut results = Vec::new();
                for dh_id in 0..4 {
                    match c.query_dh(DHId(dh_id)) {
                        Ok((status, stats)) => {
                            results.push(format!("DH{}: {:?} sent={} recv={}", dh_id, status, stats.bytes_sent, stats.bytes_received));

                            // Update UI for each DH
                            match dh_id {
                                0 => {
                                    ui.set_dh0_bytes_sent(stats.bytes_sent as i32);
                                    ui.set_dh0_bytes_recv(stats.bytes_received as i32);
                                }
                                1 => {
                                    ui.set_dh1_bytes_sent(stats.bytes_sent as i32);
                                    ui.set_dh1_bytes_recv(stats.bytes_received as i32);
                                }
                                2 => {
                                    ui.set_dh2_bytes_sent(stats.bytes_sent as i32);
                                    ui.set_dh2_bytes_recv(stats.bytes_received as i32);
                                }
                                3 => {
                                    ui.set_dh3_bytes_sent(stats.bytes_sent as i32);
                                    ui.set_dh3_bytes_recv(stats.bytes_received as i32);
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            results.push(format!("DH{}: Error - {}", dh_id, e));
                        }
                    }
                }
                ui.set_last_response(SharedString::from(results.join("; ")));
            } else {
                ui.set_last_response(SharedString::from("Not connected"));
            }
        });
    }

    // Start DH handler
    {
        let client = client.clone();
        let ui_weak = ui_weak.clone();
        ui.on_start_dh(move |dh_id| {
            let ui = ui_weak.unwrap();
            let mut guard = client.lock().unwrap();
            if let Some(ref mut c) = *guard {
                let id = DHId(dh_id as u32);
                let name = DHName::new(format!("DH{}", dh_id));
                match c.start_dh(id, DHType::Network, name) {
                    Ok(status) => {
                        let status_str = if status == CommandStatus::Success { "Active" } else { "Error" };
                        match dh_id {
                            0 => ui.set_dh0_status(SharedString::from(status_str)),
                            1 => ui.set_dh1_status(SharedString::from(status_str)),
                            2 => ui.set_dh2_status(SharedString::from(status_str)),
                            3 => ui.set_dh3_status(SharedString::from(status_str)),
                            _ => {}
                        }
                        ui.set_last_response(SharedString::from(format!("START_DH {} - {:?}", dh_id, status)));
                    }
                    Err(e) => {
                        ui.set_last_response(SharedString::from(format!("START_DH {} failed: {}", dh_id, e)));
                    }
                }
            } else {
                ui.set_last_response(SharedString::from("Not connected"));
            }
        });
    }

    // Stop DH handler
    {
        let client = client.clone();
        let ui_weak = ui_weak.clone();
        ui.on_stop_dh(move |dh_id| {
            let ui = ui_weak.unwrap();
            let mut guard = client.lock().unwrap();
            if let Some(ref mut c) = *guard {
                let id = DHId(dh_id as u32);
                match c.stop_dh(id) {
                    Ok(status) => {
                        match dh_id {
                            0 => ui.set_dh0_status(SharedString::from("Stopped")),
                            1 => ui.set_dh1_status(SharedString::from("Stopped")),
                            2 => ui.set_dh2_status(SharedString::from("Stopped")),
                            3 => ui.set_dh3_status(SharedString::from("Stopped")),
                            _ => {}
                        }
                        ui.set_last_response(SharedString::from(format!("STOP_DH {} - {:?}", dh_id, status)));
                    }
                    Err(e) => {
                        ui.set_last_response(SharedString::from(format!("STOP_DH {} failed: {}", dh_id, e)));
                    }
                }
            } else {
                ui.set_last_response(SharedString::from("Not connected"));
            }
        });
    }

    // Quit button handler
    {
        let pm_tcssim = process_manager_tcssim.clone();
        let pm_tcspecial = process_manager_tcspecial.clone();
        ui.on_quit_clicked(move || {
            kill_and_exit_all(&pm_tcssim, &pm_tcspecial);
        });
    }

    // Window close handler (close box)
    {
        let pm_tcssim = process_manager_tcssim.clone();
        let pm_tcspecial = process_manager_tcspecial.clone();
        ui.window().on_close_requested(move || {
            kill_and_exit_all(&pm_tcssim, &pm_tcspecial);
            slint::CloseRequestResponse::HideWindow
        });
    }

    ui.run().unwrap();
}

fn kill_and_exit_all(pm_tcssim: &Arc<ProcessManager>, pm_tcspecial: &Arc<ProcessManager>) {
    pm_tcssim.kill();
    pm_tcspecial.kill();
    exit(0);
}
