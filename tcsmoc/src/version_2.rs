//! Running a version 2 configuration.
//!
//! A version 2 vehicle is a graph of nodes, and what it offers the ground is
//! not what a version 1 vehicle offers. There is no command interpreter, so
//! nothing to connect to, no payloads to enable or query, and no beacons. What
//! there is, is data: each node drawn into a `moc` node sends to the `moc`
//! node's `ground_base` plus its device id, and each payload nothing touches
//! sends over `payload_telemetry` -- see `tcslibgs::graph::telemetry`. So the
//! MOC here is a ground station for that data: one panel per source, filled
//! through `tcslib::moc`, the same function call any ground software uses.
//!
//! What stays the same is everything else the window does: the panels, the
//! grid they sit in, tcspecial and tcssim started on the same file, and Quit.

use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use tcslib::moc::Moc;
use tcslibgs::graph::telemetry::{self, Telemetered, Via};
use tcslibgs::graph::Graph;
use tcslibgs::sim_config::SimConfigFile;
use tcslibgs::{DHSample, Timestamp};
use tcsverify::startup::Startup;

use crate::beacon_receive::BeaconReceive;
use crate::oc_link::{Arrival, Arrivals};
use crate::{
    ask_for_the_edge, grid_shape, handle_close, handle_quit, present, show_arrivals, stop,
    window_size, DHInfo, Edge, MainWindow, ProcessManager, Sizes, CHILDREN, NO_TRANSFER_TIME,
    PANEL_REFRESH_INTERVAL,
};

/// What the command-link half of the window says for a version 2 vehicle.
pub const NO_COMMAND_LINK: &str = "None (version 2)";

/// What a panel's status says: a source the ground receives from, and does
/// not command.
pub const RECEIVING_STATUS: &str = "Receiving";

/// What the command buttons say when pressed.
pub const NOT_COMMANDED: &str = "A version 2 vehicle has no command interpreter: its payloads \
     are not started, stopped or queried from here. The panels show what each \
     source sends to the ground.";

/// One panel per link to the ground, in the order the configuration declares
/// the sources.
pub fn panels(links: &[Telemetered]) -> Vec<DHInfo> {
    let mut links: Vec<&Telemetered> = links.iter().collect();
    links.sort_by_key(|link| link.id);
    links
        .into_iter()
        .map(|link| {
            let via = match &link.via {
                Via::Moc(moc) => format!("to {moc}"),
                Via::PayloadTelemetry => "over payload_telemetry".to_string(),
            };
            DHInfo {
                name: SharedString::from(link.source.clone()),
                // The node's device id, which is what its port is counted on.
                dev_id: i32::try_from(link.id).unwrap_or(i32::MAX),
                config: SharedString::from(format!(
                    "{via}, at {}:{}",
                    link.ground.address, link.ground.port
                )),
                packet_size: 0,
                status: SharedString::from(RECEIVING_STATUS),
                oc_time: SharedString::from(NO_TRANSFER_TIME),
                oc_data: SharedString::new(),
                packets_recv: 0,
                data_color: crate::config::constants::freshness_color(
                    crate::freshness::Freshness::Unjudged,
                ),
                parameters: SharedString::from(format!(
                    "{} (device id {})\nsends {via}\nthe ground receives at {}:{}",
                    link.source, link.id, link.ground.address, link.ground.port
                )),
            }
        })
        .collect()
}

/// Whether the simulator file names any payload to simulate, so that tcssim
/// is started only when it has something to be.
fn anything_simulated(graph: &Graph, sim_path: &str) -> bool {
    SimConfigFile::load(sim_path)
        .ok()
        .and_then(|sim| tcslibgs::graph::simulated::handlers(graph, &sim).ok())
        .is_some_and(|handlers| !handlers.is_empty())
}

/// Where the beacons come from and go to: the `tcspecial` section's
/// `beacon.send` and `beacon.recv`, which tcspecial sends them by. `None` for
/// a configuration with no section, which has no beacons.
fn beacon_of(graph: &Graph) -> Result<Option<tcspecial::config::Beacon>, String> {
    let Some(section) = graph.tcspecial.as_ref() else {
        return Ok(None);
    };
    let ci = tcslibgs::graph::parameters::ci_config_of(&tcslibgs::graph::parameters::flatten(
        section,
    ))?
    .to_ci_config()?;
    Ok(Some(tcspecial::config::Beacon {
        send: ci.beacon_send,
        recv: ci.beacon_recv,
    }))
}

/// Keep what arrives at one `moc` node, in the panel of the source it came
/// from.
fn serve(mut moc: Moc, rows: Arc<BTreeMap<String, usize>>, arrivals: Arrivals) {
    let mut packets: BTreeMap<String, u64> = BTreeMap::new();
    loop {
        let received = match moc.receive(None) {
            Ok(Some(received)) => received,
            Ok(None) => continue,
            Err(e) => {
                eprintln!("{e}");
                continue;
            }
        };
        let Some(&row) = rows.get(&received.source) else {
            continue;
        };
        let count = packets.entry(received.source.clone()).or_insert(0);
        *count += 1;
        let mut data = DHSample::new();
        data.record(&received.data);
        if let Ok(mut slots) = arrivals.lock() {
            if let Some(slot) = slots.get_mut(row) {
                *slot = Some(Arrival {
                    at: Timestamp::now(),
                    data,
                    packets: *count,
                });
            }
        }
    }
}

/// Run the MOC on a version 2 configuration. Does not return.
pub fn run(payload_path: &str, sim_path: &str, mut startup: Startup) -> ! {
    let graph = match Graph::load(payload_path) {
        Ok(graph) => graph,
        Err(e) => {
            startup.error(format!("{payload_path}: {e}"));
            stop(&startup);
        }
    };
    let links = match telemetry::of(&graph) {
        Ok(links) => links,
        Err(refused) => {
            startup.error(format!("{payload_path}: {}", refused.why));
            stop(&startup);
        }
    };
    if links.is_empty() {
        startup.warning(format!(
            "{payload_path}: nothing in it sends to the ground, so there is nothing \
             for the panels to show"
        ));
    }

    let ui = MainWindow::new().unwrap();
    crate::remember_main_window(&ui);
    let _ = Sizes::settle(&ui);

    let rows_in_order = panels(&links);
    let rows: Arc<BTreeMap<String, usize>> = Arc::new(
        rows_in_order
            .iter()
            .enumerate()
            .map(|(row, info)| (info.name.to_string(), row))
            .collect(),
    );
    let count = rows_in_order.len();
    let model: Rc<VecModel<DHInfo>> = Rc::new(VecModel::from(rows_in_order));
    ui.set_dh_model(ModelRc::from(model.clone()));

    let sizes = Sizes::of(&ui);
    let shape = grid_shape(&sizes, count);
    ui.window().set_size(window_size(&sizes, &shape, count));
    eprintln!(
        "{payload_path}: a version 2 configuration, {count} source(s) sending to the ground"
    );

    ui.set_ci_status(SharedString::from(NO_COMMAND_LINK));
    ui.set_ci_address(SharedString::new());
    ui.set_last_response(SharedString::from(
        "Version 2: each panel is a node whose data reaches the ground",
    ));

    // The vehicle, and the simulator where there is anything to simulate. A
    // version 2 tcspecial has no command interpreter to ask whether it is
    // already running, so one is started; and tcssim with nothing to be would
    // exit at once, taking the MOC with it.
    let [tcspecial, tcssim] = CHILDREN;
    let pm_tcspecial = Arc::new(ProcessManager::new());
    pm_tcspecial.start_child(tcspecial, payload_path);
    let pm_tcssim = Arc::new(ProcessManager::new());
    if anything_simulated(&graph, sim_path) {
        pm_tcssim.start_child(tcssim, payload_path);
    } else {
        eprintln!("{sim_path}: nothing to simulate, so tcssim is not started");
    }

    // Whether the vehicle is serving the configuration this MOC read. This
    // MOC digests the configuration files it read, with the function
    // tcspecial digests the files it read with; tcspecial sends its digest in
    // every beacon, and a beacon whose digest differs asks the operator
    // whether to continue anyway -- the panels say where each source's data
    // arrives, and a different configuration may send it somewhere else.
    let digest = match tcslibgs::config_digest::digest_of_graph(&graph) {
        Ok(digest) => {
            eprintln!(
                "tcsmoc v{}, config v{}, md5 {digest}",
                tcslibgs::config_digest::ConfigVersion::of_this_build(),
                tcslibgs::graph::VERSION
            );
            Some(digest)
        }
        Err(e) => {
            startup.warning(format!(
                "{payload_path} cannot be digested, so tcspecial's beacons cannot be \
                 checked against it: {e}"
            ));
            None
        }
    };
    let contact = Arc::new(Mutex::new(crate::Contact::Lost));
    let _beacon_receive = match beacon_of(&graph) {
        Ok(Some(beacon)) => {
            eprintln!(
                "Listening for beacons at {}, which {} sends from",
                beacon.recv, beacon.send
            );
            BeaconReceive::new(
                ui.as_weak(),
                beacon,
                crate::BEACON_INDICATOR.clone(),
                contact,
                digest,
                Some(crate::ask_whether_to_continue(
                    pm_tcssim.clone(),
                    Some(pm_tcspecial.clone()),
                )),
            )
        }
        Ok(None) => None,
        Err(e) => {
            startup.warning(format!("Not listening for beacons: {e}"));
            None
        }
    };

    // The ground's end of every link, through tcslib::moc: one receiver per
    // moc node and one for the payloads that report over payload_telemetry,
    // each on a thread of its own. A port that cannot be bound is said with
    // the rest of what was found, and costs its panels their data.
    let arrivals: Arrivals = Arc::new(Mutex::new(vec![None; count]));
    let mocs: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|n| n.node_type == "moc")
        .map(|n| n.name.as_str())
        .filter(|name| links.iter().any(|l| l.via == Via::Moc(name.to_string())))
        .collect();
    let mut receivers = Vec::new();
    for name in mocs {
        match Moc::open(&graph, name) {
            Ok(moc) => receivers.push(moc),
            Err(e) => startup.warning(e),
        }
    }
    match Moc::open_payload_telemetry(&graph) {
        Ok(Some(moc)) => receivers.push(moc),
        Ok(None) => {}
        Err(e) => startup.warning(e),
    }
    for moc in receivers {
        eprintln!(
            "receiving at {} from {}",
            moc.name(),
            moc.sources().join(", ")
        );
        let rows = rows.clone();
        let arrivals = arrivals.clone();
        thread::spawn(move || serve(moc, rows, arrivals));
    }

    // The panels, kept current.
    let timer = slint::Timer::default();
    {
        let model = model.clone();
        let arrivals = arrivals.clone();
        timer.start(slint::TimerMode::Repeated, PANEL_REFRESH_INTERVAL, move || {
            show_arrivals(&model, &arrivals);
        });
    }

    // The buttons that command a version 1 vehicle say why they do nothing.
    {
        let weak = ui.as_weak();
        ui.on_link_clicked(move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_last_response(SharedString::from(NOT_COMMANDED));
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_ping_clicked(move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_last_response(SharedString::from(NOT_COMMANDED));
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_query_all_clicked(move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_last_response(SharedString::from(NOT_COMMANDED));
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_enable_dh(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.set_last_response(SharedString::from(NOT_COMMANDED));
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_disable_dh(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.set_last_response(SharedString::from(NOT_COMMANDED));
            }
        });
    }

    handle_quit(&ui, pm_tcssim.clone(), Some(pm_tcspecial.clone()));
    handle_close(&ui, pm_tcssim, Some(pm_tcspecial));

    if !startup.is_empty() {
        present(&startup);
    }
    ask_for_the_edge(&ui, Edge::Top);
    ui.run().unwrap();
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn example(name: &str) -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        Graph::load(root.join(format!("examples/{name}/{name}.yaml"))).expect("the example reads")
    }

    /// One panel per source, in device id order, saying where its data
    /// arrives.
    #[test]
    fn each_source_has_a_panel_in_device_id_order() {
        let links = telemetry::of(&example("thermostat")).expect("its links");
        let panels = panels(&links);
        let named: Vec<(String, i32)> = panels
            .iter()
            .map(|p| (p.name.to_string(), p.dev_id))
            .collect();
        assert_eq!(
            named,
            vec![
                ("k_fore".to_string(), 1),
                ("k_aft".to_string(), 2),
                ("k_bay".to_string(), 3),
                ("mean_temp".to_string(), 4),
            ]
        );
        assert!(panels[3].config.contains("to moc"), "{}", panels[3].config);
        assert!(panels[3].config.contains("5104"), "{}", panels[3].config);
    }

    /// The minimal example's sensor reports over payload_telemetry.
    #[test]
    fn an_untouched_payload_has_a_panel_too() {
        let links = telemetry::of(&example("minimal")).expect("its links");
        let panels = panels(&links);
        assert_eq!(panels.len(), 1);
        assert_eq!(panels[0].name.as_str(), "sensor");
        assert!(panels[0].config.contains("payload_telemetry"), "{}", panels[0].config);
    }

    /// tcssim is started only for a configuration it has something to simulate.
    #[test]
    fn the_simulator_is_started_only_with_something_to_simulate() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let sim = |name: &str| {
            root.join(format!("examples/{name}/{name}-sim.yaml"))
                .to_str()
                .expect("a path")
                .to_string()
        };
        assert!(anything_simulated(&example("minimal"), &sim("minimal")));
        assert!(!anything_simulated(&example("simple-app"), &sim("simple-app")));
    }
}
