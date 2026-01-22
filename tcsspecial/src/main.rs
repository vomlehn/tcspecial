//! TCSpecial Spacecraft Process
//!
//! This is the main entry point for the tcspecial process that runs on the spacecraft.
//! It initializes the Command Interpreter and enters the main loop.

use std::process;
use std::sync::atomic::Ordering;
use log::{info, error};
use tcspecial_lib::{CommandInterpreter, TcspecialConfig};

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    info!("TCSpecial starting up");

    // Load configuration (use defaults for now)
    let config = TcspecialConfig::default();
    info!("Configuration: listen_addr={}, beacon_interval={}ms",
        config.listen_addr, config.beacon_interval.0);

    // Create and run the command interpreter
    let mut ci = match CommandInterpreter::new(config) {
        Ok(ci) => ci,
        Err(e) => {
            error!("Failed to create Command Interpreter: {}", e);
            process::exit(1);
        }
    };

    // Set up signal handler for graceful shutdown
    let running = ci.running_flag();
    ctrlc_handler(running.clone());

    // Run the main loop
    if let Err(e) = ci.run() {
        error!("Command Interpreter error: {}", e);
        process::exit(1);
    }

    info!("TCSpecial shutdown complete");
}

/// Set up Ctrl+C handler for graceful shutdown
fn ctrlc_handler(running: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    if let Err(e) = ctrlc::set_handler(move || {
        info!("Received shutdown signal");
        running.store(false, Ordering::SeqCst);
    }) {
        error!("Failed to set Ctrl+C handler: {}", e);
    }
}
