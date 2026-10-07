//! TCSpecial Payload Simulator (tcssim)
//!
//! A GUI application for simulating payloads that communicate with tcspecial.
//!
//! Two configuration files describe what to simulate. The payload file
//! describes the payloads themselves: which data handlers exist, how tcspecial
//! reaches each one, and how big its packets are. The simulator file adds what
//! simulating one takes and the payload file has no business knowing: how fast
//! a payload produces packets, and how a packet is divided into segments. The
//! two are joined by the data handler's name; see [`sim_config`].

use slint::{Model, ModelRc, SharedString, VecModel};
use std::env;
use std::process;
use std::rc::Rc;
use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex};

use tcslibgs::config::{load_dh_configs, payload_path_from_args, SIM_PAYLOAD_CONFIG_PATH_VAR};
use tcslibgs::{DHConfig, EndpointConfig, NetworkProtocol};

mod grid;
mod payload;
mod sim_config;

use payload::{PayloadConfig, PayloadProtocol, SimulatedPayload};
use sim_config::{ResolvedSim, SimConfigFile};

slint::include_modules!();


/// Which environment variable names the payload simulation configuration, and
/// what is read when that variable is unset.
const SIM_CONFIG_PATH_VAR: &str = "PAYLOAD_SIM_YAML";
const DEFAULT_SIM_CONFIG_PATH: &str = "payload1sim.yaml";

/// Turn a data handler and its simulator settings into the simulator's own
/// configuration.
///
/// The payload file describes what tcspecial expects to talk to, so the
/// simulator takes the other end of it: the handler's endpoint becomes the
/// address the simulated payload uses. Everything about how the payload
/// behaves comes from `sim`, which is what the simulator file settled.
fn payload_config_from(dh: &DHConfig, sim: &ResolvedSim) -> Result<PayloadConfig, String> {
    let (protocol, address, port) = match &dh.endpoint {
        EndpointConfig::Network(net) => {
            let protocol = match net.protocol {
                NetworkProtocol::Tcp => PayloadProtocol::Tcp,
                NetworkProtocol::Udp => PayloadProtocol::Udp,
                // The simulator speaks only TCP, UDP, and devices. A Unix
                // socket handler is a configuration the simulator cannot
                // stand in for, so say so rather than simulating the wrong
                // thing.
                other => {
                    return Err(format!(
                        "{} uses {:?}, which the simulator cannot simulate",
                        dh.name.0, other
                    ))
                }
            };
            (protocol, net.address.clone(), net.port)
        }
        EndpointConfig::Device(dev) => (PayloadProtocol::Device, dev.path.clone(), 0),
    };

    let packet_size = u32::try_from(dh.packet_size)
        .map_err(|_| format!("{} has a packet size too large to simulate", dh.name.0))?;

    Ok(PayloadConfig {
        _id: dh.dh_id.0,
        protocol,
        address,
        port,
        packet_size: Arc::new(AtomicU32::new(packet_size)),
        segment_size: Arc::new(AtomicU32::new(sim.segment_size)),
        packet_interval_ms: Arc::new(AtomicU32::new(sim.packet_interval_ms)),
        segment_interval_ms: Arc::new(AtomicU32::new(sim.segment_interval_ms)),
    })
}

/// How a data handler's endpoint reads in its panel.
fn endpoint_description(endpoint: &EndpointConfig) -> String {
    match endpoint {
        EndpointConfig::Network(net) => {
            let protocol = match net.protocol {
                NetworkProtocol::Tcp => "TCP",
                NetworkProtocol::Udp => "UDP",
                NetworkProtocol::UnixStream => "Unix stream",
                NetworkProtocol::UnixDgram => "Unix datagram",
            };

            match net.protocol {
                // A Unix socket is named by a path; its port means nothing.
                NetworkProtocol::UnixStream | NetworkProtocol::UnixDgram => {
                    format!("{} {}", protocol, net.address)
                }
                _ => format!("{} {}:{}", protocol, net.address, net.port),
            }
        }
        EndpointConfig::Device(dev) => format!("Device {}", dev.path),
    }
}

/// Turn a data handler and its settled simulator settings into the panel the
/// window shows for it.
fn payload_info_from(dh: &DHConfig, sim: &ResolvedSim) -> PayloadInfo {
    PayloadInfo {
        name: SharedString::from(dh.name.0.clone()),
        config: SharedString::from(endpoint_description(&dh.endpoint)),
        packet_size: i32::try_from(dh.packet_size).unwrap_or(i32::MAX),
        segment_size: sim.segment_size as i32,
        packet_interval: sim.packet_interval_ms as i32,
        segment_interval: sim.segment_interval_ms as i32,
        status: SharedString::from("Stopped"),
        last_sent_time: SharedString::from("--:--:--"),
        last_sent_data: SharedString::new(),
        last_recv_time: SharedString::from("--:--:--"),
        last_recv_data: SharedString::new(),
        packets_sent: 0,
        packets_recv: 0,
    }
}

/// Change one panel's contents, leaving the rest of it alone.
///
/// A Slint model row is a value rather than a place, so a field is changed by
/// reading the row, changing it, and putting it back.
fn update_row(model: &Rc<VecModel<PayloadInfo>>, row: usize, f: impl FnOnce(&mut PayloadInfo)) {
    if let Some(mut info) = model.row_data(row) {
        f(&mut info);
        model.set_row_data(row, info);
    }
}

fn main() {
//    env_logger::init();

    // Read both configuration files before the window exists, so a file that
    // cannot be read is reported plainly rather than as an empty window.
    let payload_path = match payload_path_from_args(
        env::args(),
        Some(SIM_PAYLOAD_CONFIG_PATH_VAR),
    ) {
        Ok(payload_path) => payload_path,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };
    let sim_path = env::var(SIM_CONFIG_PATH_VAR)
        .unwrap_or_else(|_| DEFAULT_SIM_CONFIG_PATH.to_string());

    let dh_configs = match load_dh_configs(&payload_path) {
        Ok(dh_configs) => dh_configs,
        Err(e) => {
            eprintln!("Error loading payload configuration from {}: {}", payload_path, e);
            process::exit(1);
        }
    };

    let sim_file = match SimConfigFile::load(&sim_path) {
        Ok(sim_file) => sim_file,
        Err(e) => {
            eprintln!("Error loading simulator configuration from {}: {}", sim_path, e);
            process::exit(1);
        }
    };

    // Join the two by name. Every data handler needs settings and every
    // settings entry needs a handler, so this is where a file naming a payload
    // that does not exist, or leaving one undriven, is caught.
    let sims = match sim_file.resolve(&dh_configs) {
        Ok(sims) => sims,
        Err(e) => {
            eprintln!("Error in {} against {}: {}", sim_path, payload_path, e);
            process::exit(1);
        }
    };

    let configs: Vec<PayloadConfig> = match dh_configs
        .iter()
        .zip(&sims)
        .map(|(dh, sim)| payload_config_from(dh, sim))
        .collect()
    {
        Ok(configs) => configs,
        Err(e) => {
            eprintln!("Error in payload configuration {}: {}", payload_path, e);
            process::exit(1);
        }
    };

    if configs.is_empty() {
        eprintln!("{} describes no payloads to simulate", payload_path);
        process::exit(1);
    }

    eprintln!(
        "Simulating {} payloads from {} and {}",
        configs.len(),
        payload_path,
        sim_path
    );
    if let Some(description) = &sim_file.description {
        eprintln!(
            "Simulator configuration: {} (version {})",
            description,
            sim_file.version.as_deref().unwrap_or("unstated")
        );
    }

    let ui = MainWindow::new().unwrap();
    let ui_weak = ui.as_weak();

    // One panel per payload, in a grid worked out from how many there are.
    let model: Rc<VecModel<PayloadInfo>> = Rc::new(VecModel::from(
        dh_configs
            .iter()
            .zip(&sims)
            .map(|(dh, sim)| payload_info_from(dh, sim))
            .collect::<Vec<_>>(),
    ));
    ui.set_payload_model(ModelRc::from(model.clone()));

    let shape = grid::grid_shape(configs.len());
    ui.set_columns(shape.columns as i32);
    ui.set_cell_width(grid::PANEL_WIDTH);
    ui.set_cell_height(grid::PANEL_HEIGHT);
    ui.window().set_size(slint::LogicalSize::new(shape.width, shape.height));

    // Create simulated payloads
    let payloads: Arc<Mutex<Vec<SimulatedPayload>>> = Arc::new(Mutex::new(
        configs.into_iter().map(|c| SimulatedPayload::new(c)).collect()
    ));

    // Start payload handler
    {
        let payloads = payloads.clone();
        let model = model.clone();
        ui.on_start_payload(move |row| {
            let row = row as usize;
            let mut guard = payloads.lock().unwrap();
            if let Some(payload) = guard.get_mut(row) {
                match payload.start() {
                    Ok(_) => {
                        update_row(&model, row, |info| {
                            info.status = SharedString::from("Running");
                        });
                    }
                    Err(e) => {
                        eprintln!("Failed to start payload {}: {}", row, e);
                    }
                }
            }
        });
    }

    // Stop payload handler
    {
        let payloads = payloads.clone();
        let model = model.clone();
        ui.on_stop_payload(move |row| {
            let row = row as usize;
            let mut guard = payloads.lock().unwrap();
            if let Some(payload) = guard.get_mut(row) {
                payload.stop();
                update_row(&model, row, |info| {
                    info.status = SharedString::from("Stopped");
                });
            }
        });
    }

    // Config change handler
    {
        let payloads = payloads.clone();
        ui.on_config_payload(move |row, packet_size, segment_size, packet_interval, segment_interval| {
            let guard = payloads.lock().unwrap();
            if let Some(payload) = guard.get(row as usize) {
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
        let model = model.clone();
        let timer = slint::Timer::default();
        timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(500), move || {
            let guard = payloads.lock().unwrap();
            for (row, payload) in guard.iter().enumerate() {
                let stats = payload.stats();
                update_row(&model, row, |info| {
                    info.packets_sent = stats.packets_sent as i32;
                    info.packets_recv = stats.packets_recv as i32;
                });
            }
        });

        // Keep timer alive
        std::mem::forget(timer);
    }

    ui.run().unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::Ordering;
    use tcslibgs::{DHId, DHName, DeviceConfig, NetworkConfig};

    fn repo_file(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(name)
    }

    /// Every shipped payload set: a payload file and the simulator file beside
    /// it.
    ///
    /// The sets are discovered rather than listed, so a set added to the
    /// repository is covered by the tests that already exist, instead of only
    /// by whichever one somebody remembers to extend. A payload file with no
    /// simulator file beside it fails here: tcssim needs both, so half a set
    /// is a set that cannot be run.
    fn shipped_sets() -> Vec<(std::path::PathBuf, std::path::PathBuf)> {
        let root = repo_file(".");
        let mut sets = Vec::new();

        for entry in std::fs::read_dir(&root).expect("the repository root is readable") {
            let path = entry.expect("a readable directory entry").path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name,
                None => continue,
            };

            // The simulator files are found through their payload file, not on
            // their own, so that one with no payload file is noticed too.
            let stem = match name
                .strip_prefix("payload")
                .and_then(|rest| rest.strip_suffix(".yaml"))
            {
                Some(stem) if !stem.ends_with("sim") => stem,
                _ => continue,
            };

            let sim_path = repo_file(&format!("payload{}sim.yaml", stem));
            assert!(
                sim_path.exists(),
                "{} has no {} beside it, so the set cannot be simulated",
                path.display(),
                sim_path.display()
            );
            sets.push((path, sim_path));
        }

        sets.sort();
        assert!(
            !sets.is_empty(),
            "no payload sets found in {}",
            root.display()
        );
        sets
    }

    fn sim(packet_interval_ms: u32, segment_interval_ms: u32, segment_size: u32) -> ResolvedSim {
        ResolvedSim {
            packet_interval_ms,
            segment_interval_ms,
            segment_size,
        }
    }

    /// Every shipped set has to describe payloads the simulator can simulate,
    /// and its two files have to agree with each other.
    ///
    /// Nothing else checks that: the GUI only reveals files that disagree when
    /// someone runs it and finds the simulator refusing to start.
    #[test]
    fn the_shipped_files_become_simulator_configs() {
        for (payload_path, sim_path) in shipped_sets() {
            let dh_configs = load_dh_configs(&payload_path)
                .unwrap_or_else(|e| panic!("{} failed to load: {e}", payload_path.display()));
            let sim_file = SimConfigFile::load(&sim_path)
                .unwrap_or_else(|e| panic!("{} failed to load: {e}", sim_path.display()));

            let sims = sim_file.resolve(&dh_configs).unwrap_or_else(|e| {
                panic!(
                    "{} does not fit {}: {e}",
                    sim_path.display(),
                    payload_path.display()
                )
            });

            let configs: Vec<PayloadConfig> = dh_configs
                .iter()
                .zip(&sims)
                .map(|(dh, s)| payload_config_from(dh, s))
                .collect::<Result<_, _>>()
                .unwrap_or_else(|e| panic!("{} is not simulatable: {e}", payload_path.display()));

            assert!(
                !configs.is_empty(),
                "{} has no payloads to simulate",
                payload_path.display()
            );
        }
    }

    /// The payload file is for payloads, so it says nothing about simulating
    /// them. A packet interval that crept back into it would be read by
    /// nothing and silently disagree with the simulator file.
    #[test]
    fn the_shipped_payload_files_state_no_simulator_settings() {
        for (payload_path, sim_path) in shipped_sets() {
            let text = std::fs::read_to_string(&payload_path).unwrap();

            for (number, line) in text.lines().enumerate() {
                let line = line.trim().trim_start_matches("- ");
                if line.starts_with('#') {
                    continue;
                }
                for setting in ["packet_interval_ms", "segment_interval_ms", "segment_size"] {
                    assert!(
                        !line.starts_with(setting),
                        "{}:{} states {}, which belongs in {}",
                        payload_path.display(),
                        number + 1,
                        setting,
                        sim_path.display()
                    );
                }
            }
        }
    }

    /// Every payload of every shipped set gets a panel, and they fill a grid
    /// with room for all of them.
    #[test]
    fn the_shipped_files_give_a_panel_each() {
        for (payload_path, sim_path) in shipped_sets() {
            let dh_configs = load_dh_configs(&payload_path).unwrap();
            let sim_file = SimConfigFile::load(&sim_path).unwrap();
            let sims = sim_file.resolve(&dh_configs).unwrap();

            let panels: Vec<PayloadInfo> = dh_configs
                .iter()
                .zip(&sims)
                .map(|(dh, s)| payload_info_from(dh, s))
                .collect();

            assert_eq!(
                panels.len(),
                dh_configs.len(),
                "{}",
                payload_path.display()
            );

            let shape = grid::grid_shape(panels.len());
            assert!(
                shape.columns * shape.rows >= panels.len(),
                "{} does not fit its grid",
                payload_path.display()
            );
        }
    }

    /// A panel shows the data handler's packet size and the simulator file's
    /// settings, so an edit starts from what the files said.
    #[test]
    fn a_panel_shows_both_files_values() {
        let dh = DHConfig {
            dh_id: DHId(3),
            name: DHName::new("DH3"),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: "localhost".to_string(),
                port: 5003,
            }),
            packet_size: 15,
     oc: None,
 };

        let info = payload_info_from(&dh, &sim(500, 250, 5));
        assert_eq!(info.name, SharedString::from("DH3"));
        assert_eq!(info.config, SharedString::from("UDP localhost:5003"));
        // From the payload file
        assert_eq!(info.packet_size, 15);
        // From the simulator file
        assert_eq!(info.packet_interval, 500);
        assert_eq!(info.segment_interval, 250);
        assert_eq!(info.segment_size, 5);
    }

    #[test]
    fn a_device_handler_simulates_against_its_path() {
        let dh = DHConfig {
            dh_id: DHId(7),
            name: DHName::new("DH7"),
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/urandom".to_string(),
            }),
            packet_size: 4,
     oc: None,
 };

        let config = payload_config_from(&dh, &sim(250, 100, 2)).unwrap();
        assert_eq!(config.address, "/dev/urandom");
        assert!(config.protocol == PayloadProtocol::Device);
        // The packet size is the payload file's; everything else is the
        // simulator file's.
        assert_eq!(config.packet_size.load(Ordering::SeqCst), 4);
        assert_eq!(config.segment_size.load(Ordering::SeqCst), 2);
        assert_eq!(config.packet_interval_ms.load(Ordering::SeqCst), 250);
        assert_eq!(config.segment_interval_ms.load(Ordering::SeqCst), 100);
    }

    #[test]
    fn a_unix_socket_handler_is_refused_rather_than_mis_simulated() {
        let dh = DHConfig {
            dh_id: DHId(8),
            name: DHName::new("DH8"),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::UnixStream,
                address: "/tmp/dh8".to_string(),
                port: 0,
            }),
            packet_size: 4,
     oc: None,
 };

        assert!(payload_config_from(&dh, &sim(250, 250, 4)).is_err());
    }
}
