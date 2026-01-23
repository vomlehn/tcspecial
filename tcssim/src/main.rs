//! TCSpecial Payload Simulator (tcssim)
//!
//! A GUI application for simulating payloads that communicate with tcspecial.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use slint::SharedString;

mod payload;

use payload::{PayloadConfig, PayloadProtocol, SimulatedPayload};

slint::include_modules!();

fn main() {
    env_logger::init();

    let ui = MainWindow::new().unwrap();
    let ui_weak = ui.as_weak();

    // Create payload configurations from tcspayload.json spec
    let configs = vec![
        PayloadConfig {
            id: 0,
            protocol: PayloadProtocol::Tcp,
            address: "127.0.0.1".to_string(),
            port: 5000,
            packet_size: Arc::new(AtomicU32::new(12)),
            segment_size: Arc::new(AtomicU32::new(12)),
            packet_interval_ms: Arc::new(AtomicU32::new(1000)),
            segment_interval_ms: Arc::new(AtomicU32::new(1000)),
        },
        PayloadConfig {
            id: 1,
            protocol: PayloadProtocol::Udp,
            address: "127.0.0.1".to_string(),
            port: 5001,
            packet_size: Arc::new(AtomicU32::new(11)),
            segment_size: Arc::new(AtomicU32::new(11)),
            packet_interval_ms: Arc::new(AtomicU32::new(1000)),
            segment_interval_ms: Arc::new(AtomicU32::new(1000)),
        },
        PayloadConfig {
            id: 2,
            protocol: PayloadProtocol::Device,
            address: "/dev/urandom".to_string(),
            port: 0,
            packet_size: Arc::new(AtomicU32::new(1)),
            segment_size: Arc::new(AtomicU32::new(1)),
            packet_interval_ms: Arc::new(AtomicU32::new(0)),
            segment_interval_ms: Arc::new(AtomicU32::new(0)),
        },
        PayloadConfig {
            id: 3,
            protocol: PayloadProtocol::Udp,
            address: "127.0.0.1".to_string(),
            port: 5003,
            packet_size: Arc::new(AtomicU32::new(15)),
            segment_size: Arc::new(AtomicU32::new(15)),
            packet_interval_ms: Arc::new(AtomicU32::new(500)),
            segment_interval_ms: Arc::new(AtomicU32::new(500)),
        },
    ];

    // Create simulated payloads
    let payloads: Arc<Mutex<Vec<SimulatedPayload>>> = Arc::new(Mutex::new(
        configs.into_iter().map(|c| SimulatedPayload::new(c)).collect()
    ));

    // Start payload handler
    {
        let payloads = payloads.clone();
        let ui_weak = ui_weak.clone();
        ui.on_start_payload(move |id| {
            let ui = ui_weak.unwrap();
            let mut guard = payloads.lock().unwrap();
            if let Some(payload) = guard.get_mut(id as usize) {
                match payload.start() {
                    Ok(_) => {
                        let status = SharedString::from("Running");
                        match id {
                            0 => ui.set_p0_status(status),
                            1 => ui.set_p1_status(status),
                            2 => ui.set_p2_status(status),
                            3 => ui.set_p3_status(status),
                            _ => {}
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to start payload {}: {}", id, e);
                    }
                }
            }
        });
    }

    // Stop payload handler
    {
        let payloads = payloads.clone();
        let ui_weak = ui_weak.clone();
        ui.on_stop_payload(move |id| {
            let ui = ui_weak.unwrap();
            let mut guard = payloads.lock().unwrap();
            if let Some(payload) = guard.get_mut(id as usize) {
                payload.stop();
                let status = SharedString::from("Stopped");
                match id {
                    0 => ui.set_p0_status(status),
                    1 => ui.set_p1_status(status),
                    2 => ui.set_p2_status(status),
                    3 => ui.set_p3_status(status),
                    _ => {}
                }
            }
        });
    }

    // Config change handler
    {
        let payloads = payloads.clone();
        ui.on_config_payload(move |id, packet_size, segment_size, packet_interval, segment_interval| {
            let guard = payloads.lock().unwrap();
            if let Some(payload) = guard.get(id as usize) {
                payload.set_packet_size(packet_size as u32);
                payload.set_segment_size(segment_size as u32);
                payload.set_packet_interval(packet_interval as u32);
                payload.set_segment_interval(segment_interval as u32);
            }
        });
    }

    // Quit handler
    {
        let payloads = payloads.clone();
        let ui_weak = ui_weak.clone();
        ui.on_quit_clicked(move || {
            // Stop all payloads
            let mut guard = payloads.lock().unwrap();
            for payload in guard.iter_mut() {
                payload.stop();
            }
            drop(guard);

            let ui = ui_weak.unwrap();
            ui.hide().unwrap();
        });
    }

    // Periodic update timer
    {
        let payloads = payloads.clone();
        let ui_weak = ui_weak.clone();
        let timer = slint::Timer::default();
        timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(500), move || {
            let ui = match ui_weak.upgrade() {
                Some(ui) => ui,
                None => return,
            };

            let guard = payloads.lock().unwrap();
            for (i, payload) in guard.iter().enumerate() {
                let stats = payload.stats();
                match i {
                    0 => {
                        ui.set_p0_packets_sent(stats.packets_sent as i32);
                        ui.set_p0_packets_recv(stats.packets_recv as i32);
                    }
                    1 => {
                        ui.set_p1_packets_sent(stats.packets_sent as i32);
                        ui.set_p1_packets_recv(stats.packets_recv as i32);
                    }
                    2 => {
                        ui.set_p2_packets_sent(stats.packets_sent as i32);
                        ui.set_p2_packets_recv(stats.packets_recv as i32);
                    }
                    3 => {
                        ui.set_p3_packets_sent(stats.packets_sent as i32);
                        ui.set_p3_packets_recv(stats.packets_recv as i32);
                    }
                    _ => {}
                }
            }
        });

        // Keep timer alive
        std::mem::forget(timer);
    }

    ui.run().unwrap();
}
