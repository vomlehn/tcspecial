//! TCSpecial main entry point
//!
//! Starts the command interpreter and data handlers based on configuration.

use std::env;
use std::process;

use log::{error, info, trace};
use tcspecial::config::{beacon_address, load_endpoint_config, load_tcspecial_config};
use tcspecial::CommandInterpreter;
use tcslibgs::config::{
    load_dh_configs, load_tcspecial_section, payload_path_from_args, PAYLOAD_CONFIG_PATH_VAR,
};
use tcslibgs::config_digest::{digest_of_file, ConfigVersion};

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
        unwrap_or_else(|_| "tcspecial/src/tcspecial.yaml".to_string());
    info!("Loading tcspecial configuration from: {}", config_path);

    // Load configuration
    let mut tcspecial_config = match load_tcspecial_config(&config_path) {
        Ok(config) => config,
        Err(e) => {
            error!("Error loading tcspecial configuration: {}", e);
            process::exit(1);
        }
    };

    // Named on the command line, or by PAYLOAD_CONFIG_PATH, or the default.
    // The file may be a payload configuration or an endpoint configuration;
    // load_dh_configs reads either. It is the one thing the command line
    // says: where commands are taken and where beacons go are both the
    // configuration file's, so each address is written in one place.
    let payload_path = match payload_path_from_args(env::args(), Some(PAYLOAD_CONFIG_PATH_VAR)) {
        Ok(payload_path) => payload_path,
        Err(e) => {
            error!("{}", e);
            process::exit(1);
        }
    };
    info!("Loading payload configuration from: {}", payload_path);

    // Where beacons go belongs to the payload set: it is the set's ground
    // station that listens for them, so the set's own file is where it is
    // said, and this file is the fallback for a set that does not say. A set
    // written in the endpoint language has no such section and so never does.
    let section = match load_tcspecial_section(&payload_path) {
        Ok(section) => section,
        Err(e) => {
            error!("{}: cannot read its tcspecial section: {}", payload_path, e);
            process::exit(1);
        }
    };
    match beacon_address(&tcspecial_config, section.as_ref()) {
        Ok(address) => tcspecial_config.beacon_address = address,
        Err(e) => {
            error!("{}: {}", payload_path, e);
            process::exit(1);
        }
    }

    // On stderr rather than through the log, and before the bind: these are
    // the addresses the ground has to be pointed at, so they are said whether
    // or not anyone set RUST_LOG, and said above whatever a failure to bind
    // one then says.
    eprintln!(
        "Commands are taken on {}:{} as {} asked, and beacons go to {}",
        tcspecial_config.address,
        tcspecial_config.port,
        config_path,
        tcspecial_config.beacon_address
    );

    // Load configuration
    let payload_config = match load_dh_configs(&payload_path) {
        Ok(payload_config) => payload_config,
        Err(e) => {
            error!("Error loading payload configuration: {}", e);
            process::exit(1);
        }
    };

    info!("Loaded {} data handler configurations", payload_config.len());

    // What this process read, said as it read it and before the command
    // interpreter's socket is bound below. On stderr, as the command address
    // is and for the same reason: it is one of the two facts a ground station
    // checks against its own, so it has to appear whether or not anyone set
    // RUST_LOG -- and before the bind, so that an address already in use is
    // reported under the configuration it was going to serve rather than
    // instead of it.
    //
    // Of the file rather than of the handlers it produced: what the two ends
    // compare is the configuration, and a digest of the file is the thing a
    // person can recompute.
    let digest = match digest_of_file(&payload_path) {
        Ok(digest) => digest,
        Err(e) => {
            error!("cannot digest {}: {}", payload_path, e);
            process::exit(1);
        }
    };
    eprintln!(
        "Version {}, configuration {} md5 {}",
        ConfigVersion::of_this_build(),
        payload_path,
        digest
    );

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
    // Every kind of endpoint converts now, I2C included: see
    // EndpointConfigDoc::to_dh_configs.
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
    let mut ci = match CommandInterpreter::new(tcspecial_config, payload_config, digest) {
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
