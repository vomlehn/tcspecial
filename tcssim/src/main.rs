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
use std::sync::{Arc, Mutex};

use tcslibgs::config::{load_dh_configs, payload_path_from_args, SIM_PAYLOAD_CONFIG_PATH_VAR};
use tcslibgs::DHConfig;

mod endpoint;
mod grid;
mod payload;
mod payload_device;
mod payload_i2c;
mod payload_serial;
mod payload_spi;
mod payload_tcp;
mod payload_unix;
mod pty;
mod payload_udp;
mod sim_config;

use endpoint::{endpoint_description, payload_config_from};
use payload::{PayloadConfig, SimulatedPayload};
use sim_config::{ResolvedSim, SimConfigFile};

slint::include_modules!();


/// Which environment variable names the payload simulation configuration, and
/// what is read when that variable is unset.
const SIM_CONFIG_PATH_VAR: &str = "PAYLOAD_SIM_YAML";
const DEFAULT_SIM_CONFIG_PATH: &str = "payload1sim.yaml";

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
    // Taller than the grid asks for where there is a third row to look
    // ahead for, and no taller than the payloads on hand need otherwise. It
    // is also why the window can open taller than it is wide, which the shape
    // itself never is: the shape rule chooses the grid, and the room rule
    // says how much of the window to give it.
    ui.window()
        .set_size(grid::window_size(&shape, configs.len()));

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
    use tcslibgs::{DHId, DHName, EndpointConfig, NetworkConfig, NetworkProtocol};

    fn repo_file(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(name)
    }

    /// Every panel, and each of the three rows of data it ends with, is
    /// inside the window tcssim opens for each shipped set.
    ///
    /// Where `CHROME_HEIGHT` and `CHROME_WIDTH` come from. The window is
    /// opened here as main() opens it, through Slint's testing backend: no
    /// display and no event loop, but a real layout with real font metrics,
    /// so what this measures is what the window does. A panel scrolled out of
    /// sight is not merely off-screen, it is never built, so the count of
    /// panels found is part of the check.
    #[test]
    fn every_panels_data_is_inside_the_window() {
        use i_slint_backend_testing as testing;
        use testing::ElementHandle;

        testing::init_integration_test_with_system_time();

        // Every shipped set, and then more payloads than the three-row floor
        // covers. Both are needed: the floor leaves the shipped sets rows to
        // spare, so only a grid taller than the floor can show whether the
        // panel and chrome sizes are right.
        let mut cases: Vec<(String, Vec<PayloadInfo>)> = Vec::new();
        for (payload_path, sim_path) in shipped_sets() {
            let dh_configs = load_dh_configs(&payload_path)
                .unwrap_or_else(|e| panic!("{} failed to load: {e}", payload_path.display()));
            let sim_file = SimConfigFile::load(&sim_path)
                .unwrap_or_else(|e| panic!("{} failed to load: {e}", sim_path.display()));
            let sims = sim_file
                .resolve(&dh_configs)
                .unwrap_or_else(|e| panic!("{} does not fit: {e}", sim_path.display()));

            cases.push((
                format!("{}", payload_path.display()),
                dh_configs
                    .iter()
                    .zip(&sims)
                    .map(|(dh, sim)| payload_info_from(dh, sim))
                    .collect(),
            ));
        }
        let one = cases[0].1[0].clone();
        cases.push((
            "sixteen payloads".to_string(),
            std::iter::repeat_with(|| one.clone()).take(16).collect(),
        ));

        for (set, rows) in cases {
            let panels = rows.len();

            let ui = MainWindow::new().unwrap();
            ui.set_payload_model(ModelRc::from(Rc::new(VecModel::from(rows))));

            // Opened as main() opens it, floor and all.
            let shape = grid::grid_shape(panels);
            ui.set_columns(shape.columns as i32);
            ui.set_cell_width(grid::PANEL_WIDTH);
            ui.set_cell_height(grid::PANEL_HEIGHT);
            ui.window().set_size(grid::window_size(&shape, panels));
            ui.show().unwrap();

            let window = ui.window().size().to_logical(1.0);

            // The grid fits the area it is given, across and down: it stood
            // wider and taller than that area while the padding it keeps
            // inside the scrolling area was left out of the window size.
            let area = ElementHandle::find_by_element_id(&ui, "MainWindow::scroll")
                .next()
                .expect("the window has a scrolling area for the panels");
            let grid = ElementHandle::find_by_element_id(&ui, "MainWindow::grid")
                .next()
                .expect("the window has a grid of panels");
            assert!(
                grid.size().width <= area.size().width
                    && grid.size().height <= area.size().height,
                "{set}: the grid is {}x{} in an area {}x{}",
                grid.size().width,
                grid.size().height,
                area.size().width,
                area.size().height
            );

            // Room for as many rows as the rule says, and no more than
            // that: a set of fewer payloads than the window looks ahead for
            // opens only as tall as those payloads need.
            let rows = grid::rows_of_room(shape.rows, panels);
            let wanted = grid::height_for_rows(rows) - grid::CHROME_HEIGHT;
            assert!(
                area.size().height >= wanted,
                "{set}: the panels have {} of the {wanted} that {rows} rows need",
                area.size().height
            );
            assert!(
                area.size().height < wanted + grid::PANEL_HEIGHT,
                "{set}: the panels have {}, room for a row more than the {rows} wanted",
                area.size().height
            );

            for what in [
                "PayloadPanel",
                "PayloadPanel::sent-row",
                "PayloadPanel::recv-row",
                "PayloadPanel::packets-row",
            ] {
                let found: Vec<_> = if what.contains("::") {
                    ElementHandle::find_by_element_id(&ui, what).collect()
                } else {
                    ElementHandle::find_by_element_type_name(&ui, what).collect()
                };

                assert_eq!(
                    found.len(),
                    panels,
                    "{set}: {} of {panels} {what} were built, so the rest are \
                     out of sight",
                    found.len()
                );

                // Against the edges of the area the panels live in, which
                // is where they are cut off, rather than the edges of the
                // window: below the area is the row with Quit All in it.
                let area_bottom = area.absolute_position().y + area.size().height;
                let area_right = area.absolute_position().x + area.size().width;
                for (n, element) in found.iter().enumerate() {
                    let bottom = element.absolute_position().y + element.size().height;
                    let right = element.absolute_position().x + element.size().width;
                    assert!(
                        bottom <= area_bottom && right <= area_right,
                        "{set}: {what} {n} reaches ({right},{bottom}), past the \
                         ({area_right},{area_bottom}) the panels have in a \
                         window {}x{}",
                        window.width,
                        window.height
                    );
                }
            }
        }
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
            triggered: false,
            faults: Default::default(),
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
      mode: Default::default(),
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
}
