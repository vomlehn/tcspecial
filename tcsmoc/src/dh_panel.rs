//! Data Handler panel for tcsmoc GUI

use std::sync::mpsc::Sender;
use eframe::egui;
use tcslib::{DHType, Statistics};
use crate::app::CmdMessage;

/// State of a DH panel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DHPanelState {
    /// Not started
    Idle,
    /// Started and active
    Active,
    /// Stopped
    Stopped,
}

/// Data Handler panel
pub struct DHPanel {
    /// DH ID (0-7)
    id: u32,
    /// Current state
    state: DHPanelState,
    /// DH type selection
    dh_type: DHType,
    /// Configuration name
    config_name: String,
    /// Status message
    status: String,
    /// Last statistics
    stats: Option<Statistics>,
    /// Last send time
    last_send_time: Option<String>,
    /// Last send data
    last_send_data: Option<String>,
    /// Last receive time
    last_recv_time: Option<String>,
    /// Last receive data
    last_recv_data: Option<String>,
}

impl DHPanel {
    pub fn new(id: u32) -> Self {
        let default_config = match id {
            0 => "localhost:5000:tcp".to_string(),
            1 => "localhost:5001:udp".to_string(),
            2 => "/dev/urandom".to_string(),
            3 => "localhost:5003:udp".to_string(),
            _ => format!("localhost:{}", 5000 + id),
        };

        let default_type = if id == 2 { DHType::Device } else { DHType::Network };

        Self {
            id,
            state: DHPanelState::Idle,
            dh_type: default_type,
            config_name: default_config,
            status: "Idle".to_string(),
            stats: None,
            last_send_time: None,
            last_send_data: None,
            last_recv_time: None,
            last_recv_data: None,
        }
    }

    pub fn set_started(&mut self) {
        self.state = DHPanelState::Active;
    }

    pub fn set_stopped(&mut self) {
        self.state = DHPanelState::Stopped;
    }

    pub fn set_status(&mut self, status: String) {
        self.status = status;
    }

    pub fn update_stats(&mut self, stats: Statistics) {
        if let Some(ts) = &stats.timestamp {
            self.last_recv_time = Some(ts.to_string());
        }
        self.last_recv_data = Some(format!(
            "RX: {} bytes, {} ops | TX: {} bytes, {} ops",
            stats.bytes_received, stats.reads_completed,
            stats.bytes_sent, stats.writes_completed
        ));
        self.stats = Some(stats);
    }

    pub fn show(&mut self, ui: &mut egui::Ui, connected: bool, cmd_sender: Option<&Sender<CmdMessage>>) {
        let frame = egui::Frame::default()
            .fill(match self.state {
                DHPanelState::Idle => egui::Color32::from_gray(40),
                DHPanelState::Active => egui::Color32::from_rgb(20, 60, 20),
                DHPanelState::Stopped => egui::Color32::from_rgb(60, 40, 20),
            })
            .stroke(egui::Stroke::new(1.0, egui::Color32::GRAY))
            .inner_margin(8.0)
            .rounding(4.0);

        frame.show(ui, |ui| {
            ui.set_min_size(egui::vec2(220.0, 180.0));
            ui.set_max_width(240.0);

            // Header
            ui.horizontal(|ui| {
                ui.heading(format!("DH{}", self.id));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let state_text = match self.state {
                        DHPanelState::Idle => "⬜",
                        DHPanelState::Active => "🟢",
                        DHPanelState::Stopped => "🔴",
                    };
                    ui.label(state_text);
                });
            });

            ui.separator();

            // Configuration
            ui.horizontal(|ui| {
                ui.label("Type:");
                egui::ComboBox::from_id_salt(format!("dh_type_{}", self.id))
                    .selected_text(match self.dh_type {
                        DHType::Network => "Network",
                        DHType::Device => "Device",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.dh_type, DHType::Network, "Network");
                        ui.selectable_value(&mut self.dh_type, DHType::Device, "Device");
                    });
            });

            ui.horizontal(|ui| {
                ui.label("Config:");
                ui.add(egui::TextEdit::singleline(&mut self.config_name).desired_width(140.0));
            });

            // Control buttons
            ui.horizontal(|ui| {
                ui.set_enabled(connected);

                if self.state == DHPanelState::Idle || self.state == DHPanelState::Stopped {
                    if ui.button("Start").clicked() {
                        if let Some(sender) = cmd_sender {
                            let _ = sender.send(CmdMessage::StartDH(
                                self.id,
                                self.dh_type,
                                self.config_name.clone(),
                            ));
                        }
                    }
                }

                if self.state == DHPanelState::Active {
                    if ui.button("Stop").clicked() {
                        if let Some(sender) = cmd_sender {
                            let _ = sender.send(CmdMessage::StopDH(self.id));
                        }
                    }
                }

                if self.state == DHPanelState::Active {
                    if ui.button("Query").clicked() {
                        if let Some(sender) = cmd_sender {
                            let _ = sender.send(CmdMessage::QueryDH(self.id));
                        }
                    }
                }
            });

            ui.separator();

            // Status
            ui.label(format!("Status: {}", self.status));

            // Last data
            if let Some(ref data) = self.last_recv_data {
                ui.label(egui::RichText::new(data).small());
            }
        });
    }
}
