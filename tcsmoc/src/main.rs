//! TCSpecial Mission Operations Center (tcsmoc)
//!
//! A GUI program used to control simulated payloads interacting with tcspecial
//! using the tcslib library over a datagram connection.
//! Uses Slint for the GUI framework with black on white theme.

use std::net::SocketAddr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::rc::Rc;
use std::cell::RefCell;
use std::time::Duration;
use log::{info, error};
use tcslib::{TcsClient, DHId, DHName, DHType, ArmKey, BeaconTime, Statistics};
use tcslibgs::telemetry::ResponseStatus;

slint::include_modules!();

/// Message from background thread to UI
#[derive(Debug)]
pub enum UiMessage {
    Connected,
    Disconnected(String),
    PingResponse(String),
    DHStarted(u32, bool, String),
    DHStopped(u32, bool, String),
    DHStats(u32, Statistics),
    ConfigResponse(bool, String),
    Error(String),
}

/// Message from UI to background thread
#[derive(Debug)]
pub enum CmdMessage {
    Connect(SocketAddr, SocketAddr),
    Disconnect,
    Ping,
    StartDH(u32, DHType, String),
    StopDH(u32),
    QueryDH(u32),
    Configure(Option<u64>),
    RestartArm(u32),
    Restart(u32),
}

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    info!("TCSmoc starting up");

    let main_window = MainWindow::new().unwrap();

    // Create channels for communication
    let (cmd_tx, cmd_rx) = mpsc::channel::<CmdMessage>();
    let (ui_tx, ui_rx) = mpsc::channel::<UiMessage>();

    // Store command sender in a RefCell for use in callbacks
    let cmd_sender = Rc::new(RefCell::new(Some(cmd_tx)));

    // Clone main_window handle for the timer callback
    let window_weak = main_window.as_weak();

    // Set up callbacks
    let cmd_sender_clone = cmd_sender.clone();
    main_window.on_connect_clicked(move || {
        if let Some(sender) = cmd_sender_clone.borrow().as_ref() {
            let window = window_weak.unwrap();
            let remote_str = window.get_remote_addr().to_string();
            let local_str = window.get_local_addr().to_string();

            if let (Ok(remote), Ok(local)) = (remote_str.parse::<SocketAddr>(), local_str.parse::<SocketAddr>()) {
                let _ = sender.send(CmdMessage::Connect(remote, local));
                window.set_status_message("Connecting...".into());
            } else {
                window.set_status_message("Invalid address format".into());
            }
        }
    });

    let window_weak = main_window.as_weak();
    let cmd_sender_clone = cmd_sender.clone();
    main_window.on_disconnect_clicked(move || {
        if let Some(sender) = cmd_sender_clone.borrow().as_ref() {
            let _ = sender.send(CmdMessage::Disconnect);
        }
        let window = window_weak.unwrap();
        window.set_connected(false);
        window.set_status_message("Disconnected".into());
    });

    let window_weak = main_window.as_weak();
    let cmd_sender_clone = cmd_sender.clone();
    main_window.on_ping_clicked(move || {
        if let Some(sender) = cmd_sender_clone.borrow().as_ref() {
            let _ = sender.send(CmdMessage::Ping);
        }
    });

    let window_weak = main_window.as_weak();
    let cmd_sender_clone = cmd_sender.clone();
    main_window.on_configure_clicked(move || {
        if let Some(sender) = cmd_sender_clone.borrow().as_ref() {
            let window = window_weak.unwrap();
            let interval: Option<u64> = window.get_beacon_interval().to_string().parse().ok();
            let _ = sender.send(CmdMessage::Configure(interval));
        }
    });

    let window_weak = main_window.as_weak();
    let cmd_sender_clone = cmd_sender.clone();
    main_window.on_arm_clicked(move || {
        if let Some(sender) = cmd_sender_clone.borrow().as_ref() {
            let window = window_weak.unwrap();
            if let Ok(key) = window.get_arm_key().to_string().parse::<u32>() {
                let _ = sender.send(CmdMessage::RestartArm(key));
            }
        }
    });

    let window_weak = main_window.as_weak();
    let cmd_sender_clone = cmd_sender.clone();
    main_window.on_restart_clicked(move || {
        if let Some(sender) = cmd_sender_clone.borrow().as_ref() {
            let window = window_weak.unwrap();
            if let Ok(key) = window.get_arm_key().to_string().parse::<u32>() {
                let _ = sender.send(CmdMessage::Restart(key));
            }
        }
    });

    let cmd_sender_clone = cmd_sender.clone();
    main_window.on_dh_start(move |id, dh_type, config| {
        if let Some(sender) = cmd_sender_clone.borrow().as_ref() {
            let rust_dh_type = match dh_type {
                DHTypeEnum::Network => DHType::Network,
                DHTypeEnum::Device => DHType::Device,
            };
            let _ = sender.send(CmdMessage::StartDH(id as u32, rust_dh_type, config.to_string()));
        }
    });

    let cmd_sender_clone = cmd_sender.clone();
    main_window.on_dh_stop(move |id| {
        if let Some(sender) = cmd_sender_clone.borrow().as_ref() {
            let _ = sender.send(CmdMessage::StopDH(id as u32));
        }
    });

    let cmd_sender_clone = cmd_sender.clone();
    main_window.on_dh_query(move |id| {
        if let Some(sender) = cmd_sender_clone.borrow().as_ref() {
            let _ = sender.send(CmdMessage::QueryDH(id as u32));
        }
    });

    // Start background thread
    thread::spawn(move || {
        background_thread(cmd_rx, ui_tx);
    });

    // Set up timer for processing messages from background thread
    let window_weak = main_window.as_weak();
    let ui_rx = Rc::new(RefCell::new(ui_rx));
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            let window = window_weak.unwrap();
            while let Ok(msg) = ui_rx.borrow_mut().try_recv() {
                process_ui_message(&window, msg);
            }
        },
    );

    main_window.run().unwrap();

    info!("TCSmoc shutdown");
}

fn process_ui_message(window: &MainWindow, msg: UiMessage) {
    match msg {
        UiMessage::Connected => {
            window.set_connected(true);
            window.set_status_message("Connected".into());
        }
        UiMessage::Disconnected(reason) => {
            window.set_connected(false);
            window.set_status_message(format!("Disconnected: {}", reason).into());
        }
        UiMessage::PingResponse(msg) => {
            window.set_status_message(msg.into());
        }
        UiMessage::DHStarted(dh_id, success, msg) => {
            let state = if success { DHState::Active } else { DHState::Idle };
            set_dh_state(window, dh_id, state);
            set_dh_status(window, dh_id, &msg);
        }
        UiMessage::DHStopped(dh_id, success, msg) => {
            if success {
                set_dh_state(window, dh_id, DHState::Stopped);
            }
            set_dh_status(window, dh_id, &msg);
        }
        UiMessage::DHStats(dh_id, stats) => {
            let stats_str = format!(
                "RX: {} bytes, {} ops | TX: {} bytes, {} ops",
                stats.bytes_received, stats.reads_completed,
                stats.bytes_sent, stats.writes_completed
            );
            set_dh_stats(window, dh_id, &stats_str);
        }
        UiMessage::ConfigResponse(success, msg) => {
            let status = if success {
                format!("Config: {}", msg)
            } else {
                format!("Config failed: {}", msg)
            };
            window.set_status_message(status.into());
        }
        UiMessage::Error(err) => {
            window.set_status_message(format!("Error: {}", err).into());
        }
    }
}

fn set_dh_state(window: &MainWindow, dh_id: u32, state: DHState) {
    match dh_id {
        0 => window.set_dh0_state(state),
        1 => window.set_dh1_state(state),
        2 => window.set_dh2_state(state),
        3 => window.set_dh3_state(state),
        4 => window.set_dh4_state(state),
        5 => window.set_dh5_state(state),
        6 => window.set_dh6_state(state),
        7 => window.set_dh7_state(state),
        _ => {}
    }
}

fn set_dh_status(window: &MainWindow, dh_id: u32, status: &str) {
    let status: slint::SharedString = status.into();
    match dh_id {
        0 => window.set_dh0_status(status),
        1 => window.set_dh1_status(status),
        2 => window.set_dh2_status(status),
        3 => window.set_dh3_status(status),
        4 => window.set_dh4_status(status),
        5 => window.set_dh5_status(status),
        6 => window.set_dh6_status(status),
        7 => window.set_dh7_status(status),
        _ => {}
    }
}

fn set_dh_stats(window: &MainWindow, dh_id: u32, stats: &str) {
    let stats: slint::SharedString = stats.into();
    match dh_id {
        0 => window.set_dh0_stats(stats),
        1 => window.set_dh1_stats(stats),
        2 => window.set_dh2_stats(stats),
        3 => window.set_dh3_stats(stats),
        4 => window.set_dh4_stats(stats),
        5 => window.set_dh5_stats(stats),
        6 => window.set_dh6_stats(stats),
        7 => window.set_dh7_stats(stats),
        _ => {}
    }
}

/// Background thread for network communication
fn background_thread(
    cmd_rx: Receiver<CmdMessage>,
    ui_tx: Sender<UiMessage>,
) {
    let mut client: Option<TcsClient> = None;

    loop {
        match cmd_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(CmdMessage::Connect(remote, local)) => {
                match TcsClient::connect(remote, local) {
                    Ok(c) => {
                        client = Some(c);
                        let _ = ui_tx.send(UiMessage::Connected);
                    }
                    Err(e) => {
                        let _ = ui_tx.send(UiMessage::Disconnected(e.to_string()));
                    }
                }
            }
            Ok(CmdMessage::Disconnect) => {
                client = None;
                let _ = ui_tx.send(UiMessage::Disconnected("User requested".to_string()));
            }
            Ok(CmdMessage::Ping) => {
                if let Some(ref mut c) = client {
                    match c.ping() {
                        Ok(resp) => {
                            let _ = ui_tx.send(UiMessage::PingResponse(
                                format!("Pong! Timestamp: {}", resp.timestamp)
                            ));
                        }
                        Err(e) => {
                            let _ = ui_tx.send(UiMessage::Error(e.to_string()));
                        }
                    }
                }
            }
            Ok(CmdMessage::StartDH(id, dh_type, name)) => {
                if let Some(ref mut c) = client {
                    match c.start_dh(DHId(id), dh_type, DHName::new(name)) {
                        Ok(resp) => {
                            let success = matches!(resp.base.status, ResponseStatus::Success);
                            let _ = ui_tx.send(UiMessage::DHStarted(id, success,
                                if success { "Started".to_string() } else { "Failed".to_string() }));
                        }
                        Err(e) => {
                            let _ = ui_tx.send(UiMessage::DHStarted(id, false, e.to_string()));
                        }
                    }
                }
            }
            Ok(CmdMessage::StopDH(id)) => {
                if let Some(ref mut c) = client {
                    match c.stop_dh(DHId(id)) {
                        Ok(resp) => {
                            let success = matches!(resp.base.status, ResponseStatus::Success);
                            let _ = ui_tx.send(UiMessage::DHStopped(id, success,
                                if success { "Stopped".to_string() } else { "Failed".to_string() }));
                        }
                        Err(e) => {
                            let _ = ui_tx.send(UiMessage::DHStopped(id, false, e.to_string()));
                        }
                    }
                }
            }
            Ok(CmdMessage::QueryDH(id)) => {
                if let Some(ref mut c) = client {
                    match c.query_dh(DHId(id)) {
                        Ok(stats) => {
                            let _ = ui_tx.send(UiMessage::DHStats(id, stats));
                        }
                        Err(e) => {
                            let _ = ui_tx.send(UiMessage::Error(e.to_string()));
                        }
                    }
                }
            }
            Ok(CmdMessage::Configure(beacon_interval)) => {
                if let Some(ref mut c) = client {
                    match c.configure(beacon_interval.map(BeaconTime)) {
                        Ok(_) => {
                            let _ = ui_tx.send(UiMessage::ConfigResponse(true, "OK".to_string()));
                        }
                        Err(e) => {
                            let _ = ui_tx.send(UiMessage::ConfigResponse(false, e.to_string()));
                        }
                    }
                }
            }
            Ok(CmdMessage::RestartArm(key)) => {
                if let Some(ref mut c) = client {
                    match c.restart_arm(ArmKey(key)) {
                        Ok(_) => {
                            let _ = ui_tx.send(UiMessage::PingResponse("Restart armed".to_string()));
                        }
                        Err(e) => {
                            let _ = ui_tx.send(UiMessage::Error(e.to_string()));
                        }
                    }
                }
            }
            Ok(CmdMessage::Restart(key)) => {
                if let Some(ref mut c) = client {
                    match c.restart(ArmKey(key)) {
                        Ok(_) => {
                            let _ = ui_tx.send(UiMessage::PingResponse("Restart initiated".to_string()));
                        }
                        Err(e) => {
                            let _ = ui_tx.send(UiMessage::Error(e.to_string()));
                        }
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Continue loop
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }
}
