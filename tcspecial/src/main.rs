//! TCSpecial main entry point
//!
//! Starts the command interpreter and data handlers based on configuration.

use std::env;
use std::process;

use log::{error, info, trace};
use tcspecial::config::{load_endpoint_config, load_tcspecial_config};
use tcspecial::CommandInterpreter;
use tcslibgs::config::{
    load_payload_config, DEFAULT_PAYLOAD_CONFIG_PATH, PAYLOAD_CONFIG_PATH_VAR,
};

fn main() {
    // Default to info so that the startup messages below, which used to
    // be printed unconditionally, still appear. RUST_LOG overrides it;
    // RUST_LOG=trace adds the per-datagram tracing in the main loop.
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .init();

    trace!("main: entered");

    info!("TCSpecial starting...");

    let config_path = env::var("TCSPECIAL_CONFIG_PATH").
        unwrap_or_else(|_| "tcspecial/src/tcspecial.json".to_string());
    info!("Loading tcspecial configuration from: {}", config_path);

    // Load configuration
    let tcspecial_config = match load_tcspecial_config(&config_path) {
        Ok(config) => config,
        Err(e) => {
            error!("Error loading tcspecial configuration: {}", e);
            process::exit(1);
        }
    };

    let payload_path = env::var(PAYLOAD_CONFIG_PATH_VAR)
        .unwrap_or_else(|_| DEFAULT_PAYLOAD_CONFIG_PATH.to_string());
    info!("Loading payload configuration from: {}", payload_path);

    // Load configuration
    let payload_config = match load_payload_config(&payload_path) {
        Ok(payload_config) => payload_config,
        Err(e) => {
            error!("Error loading payload configuration: {}", e);
            process::exit(1);
        }
    };

    info!("CI config: {}:{}", tcspecial_config.address, tcspecial_config.port);
    info!("Loaded {} data handler configurations", payload_config.len());

    // An endpoint configuration file is read when one is named. Its endpoints
    // can become data handlers -- EndpointConfigDoc::to_dh_configs does it --
    // but nothing here calls that yet, and the file is read only so that a
    // malformed one is reported at startup rather than whenever the first
    // reader of it appears.
    //
    // What is left is not the conversion but the choice of which file each
    // program reads. It has to be made for all three at once: tcsmoc's panels,
    // tcssim's payloads and the handlers served here must describe the same
    // handlers, so a program reading an endpoint file while the others read a
    // payload file is the drift the payload set mechanism exists to prevent.
    // I2C endpoints are the one thing the conversion cannot do, for want of
    // anywhere to put a bus and a slave address.
    if let Ok(endpoint_path) = env::var("ENDPOINT_CONFIG_PATH") {
        info!("Loading endpoint configuration from: {}", endpoint_path);

        match load_endpoint_config(&endpoint_path) {
            Ok(endpoints) => info!(
                "Loaded {} endpoint groups and {} endpoints",
                endpoints.groups.len(),
                endpoints.endpoints.len()
            ),
            Err(e) => {
                error!("Error loading endpoint configuration: {}", e);
                process::exit(1);
            }
        }
    }

    // Create command interpreter
    let mut ci = match CommandInterpreter::new(tcspecial_config, payload_config) {
        Ok(ci) => ci,
        Err(e) => {
            error!("Error creating command interpreter: {}", e);
            process::exit(1);
        }
    };

    // Initialize data handlers
    if let Err(e) = ci.initialize_handlers() {
        error!("Error initializing data handlers: {}", e);
        process::exit(1);
    }

    info!("TCSpecial initialized, entering main loop...");

    // Run main loop
    if let Err(e) = ci.run() {
        error!("Error in main loop: {}", e);
        ci.shutdown().ok();
        process::exit(1);
    }

    // Shutdown
    if let Err(e) = ci.shutdown() {
        error!("Error during shutdown: {}", e);
        process::exit(1);
    }

    info!("TCSpecial shutdown complete");
}
