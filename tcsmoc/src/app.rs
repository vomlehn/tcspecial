//! Main application for tcsmoc

use std::net::SocketAddr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use eframe::egui;
use log::{info, error, warn};
use tcslib::{TcsClient, DHId, DHName, DHType, ArmKey, BeaconTime, Statistics, Telemetry};
use tcslibgs::telemetry::ResponseStatus;
use crate::dh_panel::DHPanel;

/// Maximum number of data handlers
const MAX_DHS: usize = 8;

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

/// Main application state
pub struct TcsmocApp {
    /// Remote address input
    remote_addr_input: String,
    /// Local address input
    local_addr_input: String,
    /// Connection status
    connected: bool,
    /// Status message
    status_message: String,
    /// Data handler panels
    dh_panels: Vec<DHPanel>,
    /// Command sender to background thread
    cmd_sender: Option<Sender<CmdMessage>>,
    /// Message receiver from background thread
    ui_receiver: Option<Receiver<UiMessage>>,
    /// Beacon interval input
    beacon_interval_input: String,
    /// Arm key input
    arm_key_input: String,
}

impl TcsmocApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let dh_panels = (0..MAX_DHS)
            .map(|i| DHPanel::new(i as u32))
            .collect();

        Self {
            remote_addr_input: "127.0.0.1:5000".to_string(),
            local_addr_input: "127.0.0.1:5100".to_string(),
            connected: false,
            status_message: "Not connected".to_string(),
            dh_panels,
            cmd_sender: None,
            ui_receiver: None,
            beacon_interval_input: "10000".to_string(),
            arm_key_input: "12345".to_string(),
        }
    }

    fn connect(&mut self) {
eprintln!("remote_addr_input {:?}", self.remote_addr_input);
        let remote_addr: SocketAddr = match self.remote_addr_input.parse() {
            Ok(addr) => addr,
            Err(e) => {
                self.status_message = format!("Invalid remote address: {}", e);
                return;
            }
        };

        let local_addr: SocketAddr = match self.local_addr_input.parse() {
            Ok(addr) => addr,
            Err(e) => {
                self.status_message = format!("Invalid local address: {}", e);
                return;
            }
        };

        // Create channels for communication
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (ui_tx, ui_rx) = mpsc::channel();

        self.cmd_sender = Some(cmd_tx.clone());
        self.ui_receiver = Some(ui_rx);

        // Start background thread
        thread::spawn(move || {
            background_thread(cmd_rx, ui_tx, remote_addr, local_addr);
        });

        // Send connect command
        if let Some(sender) = &self.cmd_sender {
            let _ = sender.send(CmdMessage::Connect(remote_addr, local_addr));
        }

        self.status_message = "Connecting...".to_string();
    }

    fn disconnect(&mut self) {
        if let Some(sender) = &self.cmd_sender {
            let _ = sender.send(CmdMessage::Disconnect);
        }
        self.connected = false;
        self.status_message = "Disconnected".to_string();
        self.cmd_sender = None;
        self.ui_receiver = None;
    }

    fn send_ping(&self) {
        if let Some(sender) = &self.cmd_sender {
            let _ = sender.send(CmdMessage::Ping);
        }
    }

    fn send_configure(&self) {
        if let Some(sender) = &self.cmd_sender {
            let interval = self.beacon_interval_input.parse().ok();
            let _ = sender.send(CmdMessage::Configure(interval));
        }
    }

    fn send_restart_arm(&self) {
        if let Some(sender) = &self.cmd_sender {
            if let Ok(key) = self.arm_key_input.parse() {
                let _ = sender.send(CmdMessage::RestartArm(key));
            }
        }
    }

    fn send_restart(&self) {
        if let Some(sender) = &self.cmd_sender {
            if let Ok(key) = self.arm_key_input.parse() {
                let _ = sender.send(CmdMessage::Restart(key));
            }
        }
    }

    fn process_messages(&mut self) {
        if let Some(receiver) = &self.ui_receiver {
            while let Ok(msg) = receiver.try_recv() {
                match msg {
                    UiMessage::Connected => {
                        self.connected = true;
                        self.status_message = "Connected".to_string();
                    }
                    UiMessage::Disconnected(reason) => {
                        self.connected = false;
                        self.status_message = format!("Disconnected: {}", reason);
                    }
                    UiMessage::PingResponse(msg) => {
                        self.status_message = msg;
                    }
                    UiMessage::DHStarted(dh_id, success, msg) => {
                        if let Some(panel) = self.dh_panels.get_mut(dh_id as usize) {
                            if success {
                                panel.set_started();
                            }
                            panel.set_status(msg);
                        }
                    }
                    UiMessage::DHStopped(dh_id, success, msg) => {
                        if let Some(panel) = self.dh_panels.get_mut(dh_id as usize) {
                            if success {
                                panel.set_stopped();
                            }
                            panel.set_status(msg);
                        }
                    }
                    UiMessage::DHStats(dh_id, stats) => {
                        if let Some(panel) = self.dh_panels.get_mut(dh_id as usize) {
                            panel.update_stats(stats);
                        }
                    }
                    UiMessage::ConfigResponse(success, msg) => {
                        self.status_message = if success {
                            format!("Config: {}", msg)
                        } else {
                            format!("Config failed: {}", msg)
                        };
                    }
                    UiMessage::Error(err) => {
                        self.status_message = format!("Error: {}", err);
                    }
                }
            }
        }
    }
}

impl eframe::App for TcsmocApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process any pending messages
        self.process_messages();

        // Top panel for CI controls
        egui::TopBottomPanel::top("ci_panel").show(ctx, |ui| {
            ui.heading("Command Interpreter");

            ui.horizontal(|ui| {
                ui.label("Remote:");
                ui.text_edit_singleline(&mut self.remote_addr_input);
                ui.label("Local:");
                ui.text_edit_singleline(&mut self.local_addr_input);

                if !self.connected {
                    if ui.button("Connect").clicked() {
                        self.connect();
                    }
                } else {
                    if ui.button("Disconnect").clicked() {
                        self.disconnect();
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.set_enabled(self.connected);

                if ui.button("Ping").clicked() {
                    self.send_ping();
                }

                ui.separator();

                ui.label("Beacon (ms):");
                ui.add(egui::TextEdit::singleline(&mut self.beacon_interval_input).desired_width(60.0));
                if ui.button("Configure").clicked() {
                    self.send_configure();
                }

                ui.separator();

                ui.label("Arm Key:");
                ui.add(egui::TextEdit::singleline(&mut self.arm_key_input).desired_width(60.0));
                if ui.button("Arm").clicked() {
                    self.send_restart_arm();
                }
                if ui.button("Restart").clicked() {
                    self.send_restart();
                }
            });

            ui.horizontal(|ui| {
                ui.label("Status:");
                ui.label(&self.status_message);
            });
        });

        // Central panel for DH rectangles
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Data Handlers");

            // Grid of DH panels (4x2)
            egui::Grid::new("dh_grid")
                .num_columns(4)
                .spacing([10.0, 10.0])
                .show(ui, |ui| {
                    for (i, panel) in self.dh_panels.iter_mut().enumerate() {
                        panel.show(ui, self.connected, self.cmd_sender.as_ref());
                        if (i + 1) % 4 == 0 {
                            ui.end_row();
                        }
                    }
                });
        });

        // Request repaint for continuous updates
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// Background thread for network communication
fn background_thread(
    cmd_rx: Receiver<CmdMessage>,
    ui_tx: Sender<UiMessage>,
    _default_remote: SocketAddr,
    _default_local: SocketAddr,
) {
    let mut client: Option<TcsClient> = None;

    loop {
        match cmd_rx.recv_timeout(std::time::Duration::from_millis(100)) {
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
                break;
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
