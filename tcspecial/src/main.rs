//! TCSpecial main entry point
//!
//! Starts the command interpreter and data handlers based on configuration.

use std::env;
use std::process;
use tcspecial::{config::load_config, CommandInterpreter};

fn main() {
    // Initialize logging
    env_logger::init();

    // Get config file path from command line or use default
    let config_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "tcspayload.json".to_string());

    println!("TCSpecial starting...");
    println!("Loading configuration from: {}", config_path);

    // Load configuration
    let (ci_config, dh_configs) = match load_config(&config_path) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            process::exit(1);
        }
    };

    println!("CI config: {}:{}", ci_config.address, ci_config.port);
    println!("Loaded {} data handler configurations", dh_configs.len());

    // Create command interpreter
    let mut ci = match CommandInterpreter::new(ci_config, dh_configs) {
        Ok(ci) => ci,
        Err(e) => {
            eprintln!("Error creating command interpreter: {}", e);
            process::exit(1);
        }
    };

    // Initialize data handlers
    if let Err(e) = ci.initialize_handlers() {
        eprintln!("Error initializing data handlers: {}", e);
        process::exit(1);
    }

    println!("TCSpecial initialized, entering main loop...");

    // Run main loop
    if let Err(e) = ci.run() {
        eprintln!("Error in main loop: {}", e);
        ci.shutdown().ok();
        process::exit(1);
    }

    // Shutdown
    if let Err(e) = ci.shutdown() {
        eprintln!("Error during shutdown: {}", e);
        process::exit(1);
    }

    println!("TCSpecial shutdown complete");
}
