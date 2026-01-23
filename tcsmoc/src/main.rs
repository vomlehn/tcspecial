//! TCSpecial Mission Operations Center (tcsmoc)
//!
//! A GUI application for testing and visualizing tcspecial operation.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use slint::SharedString;
use tcslib::{TcsClient, UdpConnection};
use tcslibgs::{Command, CommandStatus, DHId, DHName, DHType, PingCommand, QueryDHCommand, StartDHCommand, StopDHCommand};

slint::include_modules!();

mod app;

fn main() {
    env_logger::init();

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

    // Quit handler
    {
        let ui_weak = ui_weak.clone();
        ui.on_quit_clicked(move || {
            let ui = ui_weak.unwrap();
            ui.hide().unwrap();
        });
    }

    ui.run().unwrap();
}
