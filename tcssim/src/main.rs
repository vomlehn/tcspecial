//! TCSpecial Payload Simulator (tcssim)
//!
//! A GUI simulating payloads for testing tcspecial. The payloads are:
//!
//! | DH # | Type    | Configuration          | Packet Size | Packet Interval  |
//! |------|---------|------------------------|-------------|------------------|
//! | 0    | Network | TCP/IP localhost:5000  | 12 bytes    | 1 packet/second  |
//! | 1    | Network | UDP/IP localhost:5001  | 11 bytes    | 1 packet/second  |
//! | 2    | Device  | /dev/urandom           | 1 byte      | continuous       |
//! | 3    | Network | UDP/IP localhost:5003  | 15 bytes    | 2 packets/second |
//!
//! Uses Slint for the GUI framework with black on white theme.

mod payload;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::rc::Rc;
use std::cell::RefCell;
use log::{info, error};
use payload::{PayloadConfig, PayloadType, Payload, SharedStats};

slint::include_modules!();

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    info!("TCSSim payload simulator starting up");

    let running = Arc::new(AtomicBool::new(true));

    // Create shared stats for each payload
    let stats: Vec<Arc<Mutex<SharedStats>>> = (0..4)
        .map(|_| Arc::new(Mutex::new(SharedStats {
            sent: 0,
            received: 0,
            last_sent: String::new(),
            last_received: String::new(),
            status: "Listening".to_string(),
            connected: false,
        })))
        .collect();

    // Define payload configurations
    let configs = vec![
        PayloadConfig {
            id: 0,
            payload_type: PayloadType::TcpServer,
            address: "127.0.0.1:5000".to_string(),
            packet_size: 12,
            interval: Duration::from_secs(1),
        },
        PayloadConfig {
            id: 1,
            payload_type: PayloadType::UdpServer,
            address: "127.0.0.1:5001".to_string(),
            packet_size: 11,
            interval: Duration::from_secs(1),
        },
        // DH 2 is /dev/urandom which doesn't need simulation (passive device)
        PayloadConfig {
            id: 3,
            payload_type: PayloadType::UdpServer,
            address: "127.0.0.1:5003".to_string(),
            packet_size: 15,
            interval: Duration::from_millis(500),
        },
    ];

    // Start payload threads
    let mut handles = Vec::new();

    for config in configs {
        let r = running.clone();
        let stats_idx = if config.id == 3 { 3 } else { config.id as usize };
        let stats_clone = stats[stats_idx].clone();

        let handle = thread::spawn(move || {
            let mut payload = Payload::new(config, stats_clone);
            if let Err(e) = payload.run(r) {
                error!("Payload error: {}", e);
            }
        });
        handles.push(handle);
    }

    info!("All payloads started. Starting GUI...");

    // Create and run the GUI
    let main_window = MainWindow::new().unwrap();

    // Set up Ctrl+C handler
    let running_clone = running.clone();
    let window_weak = main_window.as_weak();
    ctrlc::set_handler(move || {
        info!("Received shutdown signal");
        running_clone.store(false, Ordering::SeqCst);
        // Close the window
        if let Some(window) = window_weak.upgrade() {
            window.hide().ok();
        }
    }).expect("Error setting Ctrl+C handler");

    // Set up timer to update GUI from stats
    let stats_clone: Vec<Arc<Mutex<SharedStats>>> = stats.iter().map(|s| s.clone()).collect();
    let window_weak = main_window.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            if let Some(window) = window_weak.upgrade() {
                update_gui(&window, &stats_clone);
            }
        },
    );

    // Run the GUI event loop
    main_window.run().unwrap();

    // Signal threads to stop
    running.store(false, Ordering::SeqCst);

    info!("Waiting for payload threads to finish...");

    for handle in handles {
        let _ = handle.join();
    }

    info!("TCSSim shutdown complete");
}

fn update_gui(window: &MainWindow, stats: &[Arc<Mutex<SharedStats>>]) {
    // Update payload 0
    if let Ok(s) = stats[0].lock() {
        window.set_payload0_sent(s.sent as i32);
        window.set_payload0_received(s.received as i32);
        window.set_payload0_last_sent(s.last_sent.clone().into());
        window.set_payload0_last_received(s.last_received.clone().into());
        window.set_payload0_status(s.status.clone().into());
        window.set_payload0_state(if s.connected { PayloadState::Connected } else { PayloadState::Running });
    }

    // Update payload 1
    if let Ok(s) = stats[1].lock() {
        window.set_payload1_sent(s.sent as i32);
        window.set_payload1_received(s.received as i32);
        window.set_payload1_last_sent(s.last_sent.clone().into());
        window.set_payload1_last_received(s.last_received.clone().into());
        window.set_payload1_status(s.status.clone().into());
        window.set_payload1_state(if s.connected { PayloadState::Connected } else { PayloadState::Running });
    }

    // Payload 2 is passive (device)

    // Update payload 3
    if let Ok(s) = stats[3].lock() {
        window.set_payload3_sent(s.sent as i32);
        window.set_payload3_received(s.received as i32);
        window.set_payload3_last_sent(s.last_sent.clone().into());
        window.set_payload3_last_received(s.last_received.clone().into());
        window.set_payload3_status(s.status.clone().into());
        window.set_payload3_state(if s.connected { PayloadState::Connected } else { PayloadState::Running });
    }
}
