//! TCSpecial Mission Operations Center (tcsmoc)
//!
//! A GUI application for testing and visualizing tcspecial operation.

use std::process::{Child, Command, exit};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use slint::SharedString;
use tcslib::{TcsClient, UdpConnection};
use tcslibgs::{Command as TcsCommand, CommandStatus, DHId, DHName, DHType, PingCommand, QueryDHCommand, StartDHCommand, StopDHCommand};

slint::include_modules!();

mod app;

/// Manages the tcssim subprocess
struct ProcessManager {
    child: Arc<Mutex<Option<Child>>>,
}

impl ProcessManager {
    fn new() -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
        }
    }

    /// Starts tcssim in a background thread and exits when it completes
    fn start_tcssim(&self) {
        let child = Command::new("cargo")
            .args(["run", "--bin", "tcssim"])
            .spawn()
            .expect("Failed to start tcssim");

        *self.child.lock().unwrap() = Some(child);

        let child_handle = self.child.clone();

        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(100));
                let mut guard = child_handle.lock().unwrap();
                if let Some(ref mut child) = *guard {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            println!("tcssim exited with status: {}", status);
                            drop(guard);
                            exit(0);
                        }
                        Ok(None) => {
                            // Still running
                        }
                        Err(e) => {
                            println!("Error waiting for tcssim: {}", e);
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

    /// Kills tcssim and exits the current program
    fn kill_and_exit(&self) {
        let mut guard = self.child.lock().unwrap();
        if let Some(ref mut child) = *guard {
            let _ = child.kill();
            let _ = child.wait();
            println!("tcssim killed");
        }
        *guard = None;
        drop(guard);
        exit(0);
    }
}

fn main() {
    env_logger::init();

    // Start tcssim subprocess
    let process_manager = Arc::new(ProcessManager::new());
    process_manager.start_tcssim();

    let ui = MainWindow::new().unwrap();
    let ui_weak = ui.as_weak();

    // Shared client state
    let client: Arc<Mutex<Option<TcsClient>>> = Arc::new(Mutex::new(None));

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
        let pm = process_manager.clone();
        ui.on_quit_clicked(move || {
            pm.kill_and_exit();
        });
    }

    // Window close handler (close box)
    {
        let pm = process_manager.clone();
        ui.window().on_close_requested(move || {
            pm.kill_and_exit();
            slint::CloseRequestResponse::HideWindow
        });
    }

    ui.run().unwrap();
}
