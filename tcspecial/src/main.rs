//! TCSpecial main entry point
//!
//! Starts the command interpreter and data handlers based on configuration.

use std::env;
use std::process;
use tcspecial::config::{load_endpoint_config, load_tcspecial_config};
use tcspecial::CommandInterpreter;
use tcslibgs::config::load_payload_config;

fn main() {
eprintln!("TCSspecial::main: entered");
    // Initialize logging
//    env_logger::init();

    println!("TCSpecial starting...");

    let config_path = env::var("TCSPECIAL_CONFIG_PATH").
        unwrap_or_else(|_| "tcspecial/src/tcspecial.json".to_string());
    println!("Loading tcspecial configuration from: {}", config_path);

    // Load configuration
    let tcspecial_config = match load_tcspecial_config(&config_path) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error loading payload_configuration: {}", e);
            process::exit(1);
        }
    };

    let payload_path = env::var("PAYLOAD_CONFIG_PATH").
        unwrap_or_else(|_| "tcspayload.json".to_string());
    println!("Loading payload configuration from: {}", payload_path);

    // Load configuration
    let payload_config = match load_payload_config(&payload_path) {
        Ok(payload_config) => payload_config,
        Err(e) => {
            eprintln!("Error loading payload configuration: {}", e);
            process::exit(1);
        }
    };

    println!("CI config: {}:{}", tcspecial_config.address, tcspecial_config.port);
    println!("Loaded {} data handler configurations", payload_config.len());

    // An endpoint configuration file is read when one is named. It is not yet
    // a second source of data handlers: a data handler also needs a packet
    // size and interval, which the endpoint format does not carry, and an I2C
    // endpoint has no EndpointConfig to become. Reading it here means a
    // malformed file is reported at startup rather than whenever the first
    // reader of it appears.
    if let Ok(endpoint_path) = env::var("ENDPOINT_CONFIG_PATH") {
        println!("Loading endpoint configuration from: {}", endpoint_path);

        match load_endpoint_config(&endpoint_path) {
            Ok(endpoints) => println!(
                "Loaded {} endpoint groups and {} endpoints",
                endpoints.groups.len(),
                endpoints.endpoints.len()
            ),
            Err(e) => {
                eprintln!("Error loading endpoint configuration: {}", e);
                process::exit(1);
            }
        }
    }

    // Create command interpreter
    let mut ci = match CommandInterpreter::new(tcspecial_config, payload_config) {
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
