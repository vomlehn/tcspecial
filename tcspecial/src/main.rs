//! TCSpecial main entry point
//!
//! Starts the command interpreter and data handlers based on configuration.

use std::env;
use std::process;

use log::{error, info, trace};
use tcslibgs::config::{
    load_config_version, load_dh_configs, load_tcspecial_section, payload_path_from_args,
    PAYLOAD_CONFIG_PATH_VAR,
};
use tcslibgs::config_digest::{digest_of_file, ConfigVersion};
use tcspecial::config::{beacon, load_tcspecial_config, tcspecial_config_path};
use tcspecial::CommandInterpreter;

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

    // The same call tcsmoc makes, so the two read one file.
    let config_path = tcspecial_config_path();
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
    // It is the one thing the command line
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
    // said, and this file is the fallback for a set that does not say.
    let section = match load_tcspecial_section(&payload_path) {
        Ok(section) => section,
        Err(e) => {
            error!("{}: cannot read its tcspecial section: {}", payload_path, e);
            process::exit(1);
        }
    };
    let beacon = match beacon(&tcspecial_config, section.as_ref()) {
        Ok(beacon) => beacon,
        Err(e) => {
            error!("{}: {}", payload_path, e);
            process::exit(1);
        }
    };
    tcspecial_config.beacon_address = beacon.group;
    tcspecial_config.beacon_interface = beacon.interface;

    // On stderr rather than through the log, and before the bind: these are
    // the addresses the ground has to be pointed at, so they are said whether
    // or not anyone set RUST_LOG, and said above whatever a failure to bind
    // one then says.
    eprintln!(
        "Commands are taken on {}:{} as {} asked, and beacons go to the group {} \
         on interface {}",
        tcspecial_config.address,
        tcspecial_config.port,
        config_path,
        beacon.group,
        beacon.interface
    );

    // Load configuration
    let payload_config = match load_dh_configs(&payload_path) {
        Ok(payload_config) => payload_config,
        Err(e) => {
            error!("Error loading payload configuration: {}", e);
            process::exit(1);
        }
    };

    info!(
        "Loaded {} data handler configurations",
        payload_config.len()
    );

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
    // The version the set states, which goes out in every beacon beside this
    // build's: a ground station hearing a beacon is told which software is
    // flying and which payload set it is serving.
    let config_version = match load_config_version(&payload_path) {
        Ok(version) => version,
        Err(e) => {
            error!("{}", e);
            process::exit(1);
        }
    };

    eprintln!(
        "Version {}, configuration {} version {} md5 {}",
        ConfigVersion::of_this_build(),
        payload_path,
        config_version,
        digest
    );

    // Create command interpreter
    let mut ci = match CommandInterpreter::new(
        tcspecial_config,
        payload_config,
        config_version,
        digest,
    ) {
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
