//! TCSpecial Payload Simulator (tcs-sim)
//!
//! A GUI application for simulating payloads that communicate with tcspecial.
//!
//! Two configuration files describe what to simulate. The tcspecial
//! configuration describes the payloads themselves: which data handlers exist,
//! how tcspecial reaches each one, and how big its packets are. The simulator
//! file adds what simulating one takes and the tcspecial configuration has no
//! business knowing: how fast a payload produces packets, and how a packet is
//! divided into segments. The two are joined by the data handler's name; see
//! [`tcsprotocol::sim_config`].
//!
//! `--simulate=<mode>` is the run's simulation mode, as tcspecial and tcs-moc
//! take it: see `tcsprotocol::simulate`. Under `none`, the flight setting
//! and the mode a run without the flag gets, tcs-sim refuses to start, there
//! being nothing for it to do in a run that may simulate nothing. Under `some`
//! and `all` it checks that what it will simulate, with what is a simulated or
//! dummy device already, is what the mode allows.
//!
//! Every simulated payload is made as tcs-sim starts and kept until it stops,
//! so that tcspecial finds them whatever the panels say; a panel's button
//! decides only whether its payload sends. Before making any, tcs-sim binds the
//! readiness socket its simulator file states as `tcs-sim.ready`, and answers
//! there whether they are made: see [`ready`].
//!
//! `--wait[=<seconds>]` is the other end of that: given the same files and
//! mode, it waits for a simulator already started on them to be ready, and
//! exits successfully when it is, or with a failure saying why. Thirty
//! seconds unless told otherwise.
//!
//! SIGHUP, SIGINT and SIGTERM stop tcs-sim as its Quit button does: every
//! payload stops and its simulated device is cleared before tcs-sim exits,
//! with the status a shell gives a process the signal ended, 128 more than
//! its number.
//! A second one, for a stop that is taking too long, ends tcs-sim at once.

use slint::{Model, ModelRc, SharedString, VecModel};
use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use tcsos::env;
use tcsos::process;
use tcsos::sync::atomic::{AtomicUsize, Ordering};
use tcsos::sync::{Arc, Mutex};
use tcsos::time::Duration;

use tcsprotocol::config::{
    payload_path_from_args, SIM_PAYLOAD_CONFIG_PATH_VAR,
};
use tcsprotocol::{
    payload_parameters, DHConfig, DHSample, Edge, NO_TRANSFER_TIME,
};

use std::collections::BTreeMap;

use crate::node::physics;
use crate::node::physics::sensor::Sensor;
use crate::node::physics::Attachment;
use crate::node::vehicle::thermostat::Thermostat;

mod endpoint;
mod grid;
// Public for a ground station that draws the lander as tcs-sim does: see
// examples/lander.
pub mod lander_view;
mod landing_rating;
mod node;
mod payload;
// Linux alone: a page has no terminals.
#[cfg(target_os = "linux")]
mod pty;
mod ready;
#[cfg(test)]
mod shots;

use endpoint::{endpoint_description, payload_config_attached};
use payload::{PayloadConfig, PayloadStats, SimulatedPayload};
use tcs_verify::startup::Startup;
use tcsprotocol::graph::Graph;
use tcsprotocol::node::physics::Resolution;
use tcsprotocol::ready::SetUpFor;
use tcsprotocol::simulate::{
    counted, disagreement, judge, refusals, Host, Judged, Mode, Verdict,
};
use tcsprotocol::{ResolvedSim, SimConfigFile};

slint::include_modules!();

/// What a panel's status says of a payload that is sending, and of one that
/// is not.
///
/// `ui/main.slint` labels the panel's one button from these and colours the
/// status line by them, and `handle_transfer_button` reads the same status
/// back to decide which way a press goes, so the button and what it does
/// cannot disagree. What the button actually says each way is the window's
/// own: the two words appear in Rust only in the test that holds the window
/// to this rule.
const RUNNING_STATUS: &str = "Running";
const STOPPED_STATUS: &str = "Stopped";

/// Which environment variable names the payload simulation configuration, and
/// what is read when that variable is unset.
const SIM_CONFIG_PATH_VAR: &str = "PAYLOAD_SIM_YAML";
const DEFAULT_SIM_CONFIG_PATH: &str = "tests/manual/tcspecial1sim.yaml";

/// The two strings a panel shows for one sample: when, and what.
///
/// The time's formatting is [`DHSample::panel_lines`], which tcs-moc's
/// panels use too: the two show the one transfer from opposite ends of a
/// link and are read side by side. What was sent is shown as what it is --
/// a reading as its value and unit, as tcs-moc shows the value the vehicle
/// makes of it -- where the payload's attachment reads it as one: see
/// [`Attachment::shown`]. Anything else is its bytes.
///
/// # Arguments
///
/// * `sample`     - The sample.
/// * `attachment` - What the payload is to the world, if anything.
/// * `sent`       - Whether the payload sent it, rather than was sent it.
///
/// # Returns
///
/// The time and what was sent.
fn sample_lines(
    sample: &DHSample,
    attachment: Option<&Attachment>,
    sent: bool,
) -> (SharedString, SharedString) {
    let (time, data) = sample.panel_lines();
    let shown = attachment
        .filter(|_| !sample.is_empty() && !sample.was_truncated())
        .and_then(|a| a.shown(sent, sample.data()))
        .unwrap_or(data);
    (SharedString::from(time), SharedString::from(shown))
}

/// What a press of a panel's one button asks of the payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transfer {
    /// Start it, so it sends.
    ///
    /// Named for what the payload does, where the button it comes from is
    /// labelled for what the press does: `Enable`, as the MOC's panels label
    /// the same press. The two windows are read side by side and the label is
    /// the thing both have to say the same way, which the payload's own words
    /// -- `Send` and `Silent` -- did not.
    Send,
    /// Stop it, so it goes quiet. The button offering it says `Disable`.
    Silent,
}

/// Decides which way a press of a panel's one button goes.
///
/// Decided by what the button offered, read back from the status its label
/// comes from -- not by what the payload turns out to be doing. Anything that
/// is not the running status offers to send: a payload that failed to
/// start shows the stopped status, and the useful thing to offer then is the
/// start that failed rather than a stop of something that never began.
///
/// # Arguments
///
/// * `status` - The panel's status, which its button's label comes from.
///
/// # Returns
///
/// `Silent` for a running payload, `Send` for anything else.
fn transfer_wanted(status: &str) -> Transfer {
    if status == RUNNING_STATUS {
        Transfer::Silent
    } else {
        Transfer::Send
    }
}

/// Puts a payload's traffic into its panel.
///
/// The times are what the panel is for: a count tells how much has moved
/// since the payload started, and the time tells whether any of it is moving
/// now. A payload stopped ten minutes ago and one sending every second have
/// the same count a moment after they are looked at. The bytes beside each
/// time are the head of that packet, which is what says the traffic is the
/// traffic that was expected rather than merely traffic.
///
/// # Arguments
///
/// * `info`       - The panel's row, whose counts and last packets are
///                  set.
/// * `stats`      - The payload's statistics; a count too large for the
///                  panel shows as `i32::MAX`.
/// * `attachment` - What the payload is to the world, if anything, which
///                  its packets are read by.
fn show_traffic(
    info: &mut PayloadInfo,
    stats: &PayloadStats,
    attachment: Option<&Attachment>,
) {
    info.packets_sent = i32::try_from(stats.packets_sent).unwrap_or(i32::MAX);
    info.packets_recv = i32::try_from(stats.packets_recv).unwrap_or(i32::MAX);
    (info.last_sent_time, info.last_sent_data) =
        sample_lines(&stats.last_sent, attachment, true);
    (info.last_recv_time, info.last_recv_data) =
        sample_lines(&stats.last_recv, attachment, false);
}

/// Has each panel's one button send its payload or silence it.
///
/// Which way a press goes is read back from the status the button's label
/// came from, so what happens is what the label offered: see
/// [`transfer_wanted`].
///
/// # Arguments
///
/// * `ui`       - The main window, whose presses are handled.
/// * `payloads` - The simulated payloads, in the panels' order.
/// * `model`    - The panels' rows, whose statuses are read and set.
fn handle_transfer_button(
    ui: &MainWindow,
    payloads: Arc<Mutex<Vec<SimulatedPayload>>>,
    model: Rc<VecModel<PayloadInfo>>,
) {
    ui.on_transfer_payload(move |row| {
        let row = row as usize;

        // The status the button's label came from, so what happens is
        // what the label offered.
        let showing = match model.row_data(row) {
            Some(info) => info.status.to_string(),
            None => return,
        };

        let mut guard = payloads.lock().unwrap();
        let payload = match guard.get_mut(row) {
            Some(payload) => payload,
            None => return,
        };

        match transfer_wanted(&showing) {
            Transfer::Send => {
                payload.send(true);
                update_row(&model, row, |info| {
                    info.status = SharedString::from(RUNNING_STATUS);
                });
            }
            Transfer::Silent => {
                payload.send(false);
                update_row(&model, row, |info| {
                    info.status = SharedString::from(STOPPED_STATUS);
                });
            }
        }
        // Said at once rather than at the next refresh: a heater just
        // disabled is off now.
        let driving = SharedString::from(payload.driving_said());
        update_row(&model, row, |info| info.driving = driving);
    });
}

/// Puts what each payload has done since the last look into its panel:
/// its traffic, and what a part the vehicle drives is doing. What the
/// window's timer runs, every half second.
///
/// # Arguments
///
/// * `payloads` - The simulated payloads, in the panels' order.
/// * `model`    - The panels' rows.
fn show_progress(
    payloads: &Mutex<Vec<SimulatedPayload>>,
    model: &Rc<VecModel<PayloadInfo>>,
) {
    let guard = payloads.lock().unwrap();
    for (row, payload) in guard.iter().enumerate() {
        let stats = payload.stats();
        let driving = SharedString::from(payload.driving_said());
        update_row(model, row, |info| {
            show_traffic(info, &stats, payload.attachment());
            info.driving = driving;
        });
    }
}

/// Turns a data handler and its settled simulator settings into the panel
/// the window shows for it.
///
/// # Arguments
///
/// * `dev_id` - The handler's place in the payload configuration, counted
///              from 0.
/// * `dh`     - The data handler.
/// * `sim`    - Its simulator settings.
///
/// # Returns
///
/// The panel, stopped and with nothing moved yet.
fn payload_info_from(
    dev_id: usize,
    dh: &DHConfig,
    sim: &ResolvedSim,
) -> PayloadInfo {
    PayloadInfo {
        name: SharedString::from(dh.name.0.clone()),
        // Its place in the payload configuration, which is the only number a
        // payload has: nothing in the file states one, and tcs-moc counts the
        // same way.
        dev_id: i32::try_from(dev_id).unwrap_or(i32::MAX),
        config: SharedString::from(endpoint_description(&dh.endpoint)),
        status: SharedString::from(STOPPED_STATUS),
        // Said once the payload is made: see `SimulatedPayload::driving_said`.
        driving: SharedString::new(),
        last_sent_time: SharedString::from(NO_TRANSFER_TIME),
        last_sent_data: SharedString::new(),
        last_recv_time: SharedString::from(NO_TRANSFER_TIME),
        last_recv_data: SharedString::new(),
        packets_sent: 0,
        packets_recv: 0,
        // Both files, as they were read. Tcs-sim has both in hand, so its
        // panels can show the whole of what a payload set says about a
        // payload.
        parameters: SharedString::from(payload_parameters(dh, Some(sim))),
        // Whether anything is ever sent to this payload, which decides
        // whether its panel has a received line at all.
        triggered: sim.triggered,
    }
}

/// The order of a set of data handlers by their names: the order their
/// panels are shown in.
///
/// # Arguments
///
/// * `dh_configs` - The data handlers, in their configurations' order.
///
/// # Returns
///
/// Each one's place among them, the first name's first; two of one name,
/// which a configuration refuses, in their own order.
fn name_order(dh_configs: &[DHConfig]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..dh_configs.len()).collect();
    order.sort_by(|&a, &b| dh_configs[a].name.0.cmp(&dh_configs[b].name.0));
    order
}

/// Puts a list in an order.
///
/// # Arguments
///
/// * `items` - The list.
/// * `order` - Each item's place in it, in the order wanted: every place
///             once.
///
/// # Returns
///
/// The items in that order.
fn in_order<T>(items: Vec<T>, order: &[usize]) -> Vec<T> {
    let mut items: Vec<Option<T>> = items.into_iter().map(Some).collect();
    order
        .iter()
        .map(|&at| items[at].take().expect("each place once"))
        .collect()
}

/// Changes one panel's contents, leaving the rest of it alone.
///
/// A Slint model row is a value rather than a place, so a field is changed by
/// reading the row, changing it, and putting it back.
///
/// # Arguments
///
/// * `model` - The panels' rows.
/// * `row`   - Which panel; one that does not exist is left alone.
/// * `f`     - What to change in it.
fn update_row(
    model: &Rc<VecModel<PayloadInfo>>,
    row: usize,
    f: impl FnOnce(&mut PayloadInfo),
) {
    if let Some(mut info) = model.row_data(row) {
        f(&mut info);
        model.set_row_data(row, info);
    }
}

/// What each joined payload is to the world, by payload name, and the
/// thermostats the simulator plays, none or several.
type Attachments = (BTreeMap<String, Attachment>, Vec<Arc<Thermostat>>);

/// Makes the world of the simulator file's physics model, works out
/// what each simulated payload is to it, by payload name, and makes the
/// thermostats the simulator plays, if the file names any.
///
/// One world, shared by every payload joined to it: the sensors read the
/// state the heaters warm, which is what makes a simulated control loop a
/// loop rather than three payloads sending noise.
///
/// A sensor's encoding comes from the conversion its readings go to -- see
/// `tcsprotocol::graph::simulated::reading_of` -- so the bytes it sends are
/// the bytes the vehicle reads back as what its terminal is, and neither file
/// states the scale twice.
///
/// # Arguments
///
/// * `sim_file`   - The simulator file, which names the model.
/// * `graphs`     - The tcspecial configurations' graphs, whose conversions
///                  give the sensors' encodings: each payload's is the one
///                  it is a node of. Empty if none was read.
/// * `sims`       - Each handler's simulator settings, beside `dh_configs`.
/// * `dh_configs` - The data handlers simulated.
///
/// # Returns
///
/// What each joined payload is to the world, by payload name, empty if the
/// file names no model; and the thermostats, one for each the file names.
///
/// # Errors
///
/// A message if the model or the joins to it are wrong, a model is named but
/// no graph was read, a sensor's conversion cannot be found in the graph or
/// is in another unit than its terminal, a thermostat the file names is not
/// a thermostat node or has no sensors or no heaters, or two thermostats
/// switch one heater.
fn joined_to_the_world(
    sim_file: &SimConfigFile,
    graphs: &[&tcsprotocol::graph::Graph],
    sims: &[ResolvedSim],
    dh_configs: &[DHConfig],
) -> Result<Attachments, String> {
    use tcsprotocol::node::physics::terminal::Kind;
    use tcsprotocol::sim_config::Attached;

    let model = sim_file.physics().map_err(|e| e.to_string())?;
    let simulating = sim_file
        .tcs_sim
        .as_ref()
        .map(|tcs_sim| tcs_sim.thermostats.clone())
        .unwrap_or_default();
    let Some(model) = model else {
        if let Some(name) = simulating.first() {
            return Err(format!(
                "thermostat: {name} is to be simulated, and no physics \
                 model is written for it to read or warm"
            ));
        }
        return Ok((BTreeMap::new(), Vec::new()));
    };

    // A model needs a graph to take its sensors' encodings from: without the
    // graph's conversions nothing says which bytes stand for a reading.
    let Some(first) = graphs.first().copied() else {
        return Err(
            "the simulator file has a physics model, and no graph was read: a \
                    simulated sensor sends the bytes a to_kelvin or to_value \
                    node reads back as what it reports, and without a graph \
                    there is no such node"
                .to_string(),
        );
    };

    // What the world's and the sensors' random draws start from: the file's,
    // or one of tcs-sim's own, said so that the run can be had again.
    let seed = match sim_file.tcs_sim.as_ref().and_then(|t| t.seed) {
        Some(seed) => seed,
        None => {
            let seed: u64 = rand::random();
            eprintln!(
                "the physics model's noise starts from seed {seed}: state \
                 seed: {seed} in the tcs-sim section to have it again"
            );
            seed
        }
    };

    // Keeping pace with the wall clock, which the vehicle at the other end
    // of every link runs on, from the first question a running payload
    // asks it: see `physics::clock`.
    let world = Arc::new(physics::world::World::new(
        &model,
        physics::clock::Clock::starting_when_asked(1.0),
        seed,
    ));
    // Each terminal is one the model has: `physics` refused a payload
    // naming another before this was reached.
    let place = |at: &tcsprotocol::node::physics::terminal::At| {
        model.find(at).map(|(place, _)| place)
    };

    // The graph a node is of: its own configuration's, where several are
    // simulated together, since only it has the node's links and conversions.
    // A name no configuration has is looked for in the first, which says so.
    let graph_of = |name: &str| {
        graphs
            .iter()
            .copied()
            .find(|graph| graph.node(name).is_some())
            .unwrap_or(first)
    };

    let mut attached = BTreeMap::new();
    for (dh, sim) in dh_configs.iter().zip(sims) {
        let name = dh.name.0.clone();
        let graph = graph_of(&name);
        let attachment = match &sim.attached {
            None => continue,
            Some(Attached::Reports {
                at,
                unit,
                offset,
                sensor,
            }) => Attachment::Reports {
                world: world.clone(),
                place: place(at)?,
                offset: *offset,
                reading: tcsprotocol::graph::simulated::reading_of(
                    graph, &name, *unit,
                )?,
                sensor: Arc::new(Sensor::new(&name, *sensor, seed)),
            },
            Some(Attached::Streams { at }) => Attachment::Streams {
                world: world.clone(),
                place: place(at)?,
                next: Arc::new(tcsos::sync::atomic::AtomicUsize::new(0)),
            },
            Some(Attached::Pulls { at }) => Attachment::Pulls {
                world: world.clone(),
                places: at.iter().map(place).collect::<Result<_, _>>()?,
            },
            Some(Attached::Signals { at }) => Attachment::Signals {
                world: world.clone(),
                place: place(at)?,
                said: Arc::new(tcsos::sync::atomic::AtomicU64::new(
                    f64::NAN.to_bits(),
                )),
            },
            Some(Attached::Drives { at, kind }) => {
                let unit = match kind {
                    Kind::Value(unit) => {
                        tcsprotocol::graph::simulated::driving_of(
                            graph, &name, *unit,
                        )?;
                        Some(*unit)
                    }
                    Kind::Decision | Kind::Records => None,
                };
                let place = place(at)?;
                // A heater's switch, which its panel says is heating rather
                // than on: a heater's own, or one of a zone's heater_N --
                // a thermal mass's only decisions are its heaters.
                let heater = matches!(
                    (&model.components[place.component].what, unit),
                    (Resolution::Heater(_), _)
                        | (Resolution::ThermalMass(_), None)
                );
                // Disabled, as every payload starts: its panel's button, or
                // --sending, enables it.
                Attachment::Drives {
                    world: world.clone(),
                    place,
                    unit,
                    power: Arc::new(physics::Power::new(false, heater)),
                }
            }
        };
        attached.insert(name, attachment);
    }

    let mut thermostats: Vec<Arc<Thermostat>> = Vec::new();
    for name in &simulating {
        let thermostat = thermostat_of(graph_of(name), name, &attached)?;
        // Two thermostats switching one heater would each undo the other's
        // decision, which no configuration means.
        if let Some((other, heater)) = thermostats.iter().find_map(|other| {
            thermostat
                .heaters()
                .find(|heater| other.switches(heater))
                .map(|heater| (other.name().to_string(), heater.to_string()))
        }) {
            return Err(format!(
                "thermostat: {other} and {name} both switch {heater}, and a \
                 heater is switched by one thermostat"
            ));
        }
        thermostats.push(Arc::new(thermostat));
    }
    Ok((attached, thermostats))
}

/// Makes the simulated thermostat that plays a thermostat node.
///
/// Its sensors are the payloads reporting the world whose data reaches the
/// node, and its heaters the payloads driving it that data from the node
/// reaches. Where several configurations are simulated together, only the
/// node's own configuration's payloads can be either: no link reaches from
/// one configuration into another.
///
/// # Arguments
///
/// * `graph`    - The tcspecial configuration's graph.
/// * `name`     - The thermostat node.
/// * `attached` - What each joined payload is to the world.
///
/// # Returns
///
/// The thermostat.
///
/// # Errors
///
/// A message if the node is not a thermostat node, or no sensor reaches it,
/// or it reaches no heater.
fn thermostat_of(
    graph: &tcsprotocol::graph::Graph,
    name: &str,
    attached: &BTreeMap<String, Attachment>,
) -> Result<Thermostat, String> {
    use tcsprotocol::graph::simulated::{reaches, thermostat_of};

    let params = thermostat_of(graph, name)?;
    let mut sensors = BTreeMap::new();
    let mut heaters = Vec::new();
    for (payload, attachment) in attached {
        match attachment {
            Attachment::Reports { reading, .. }
                if reaches(graph, payload, name) =>
            {
                sensors.insert(payload.clone(), *reading);
            }
            Attachment::Drives { .. } if reaches(graph, name, payload) => {
                heaters.push((payload.clone(), attachment.clone()));
            }
            _ => {}
        }
    }
    if sensors.is_empty() {
        return Err(format!(
            "thermostat: no simulated sensor reporting the world sends data \
             that reaches {name}"
        ));
    }
    if heaters.is_empty() {
        return Err(format!(
            "thermostat: data from {name} reaches no simulated heater driving \
             the world"
        ));
    }
    Ok(Thermostat::new(name, params, sensors, heaters))
}

/// What [`load`] reads out of the pair: the handlers the simulator
/// simulates, the simulator file, its settings resolved against them, and the
/// payloads' own configurations.
type Loaded = (
    Vec<DHConfig>,
    SimConfigFile,
    Vec<ResolvedSim>,
    Vec<PayloadConfig>,
);

/// Reads the pair, once tcs-verify has found nothing wrong with it, and says
/// in `startup` whatever stops it being simulated.
///
/// Every one of these is a refusal tcs-verify has already made, bar the ones
/// that are the simulator's alone -- a payload it cannot simulate, and a
/// set with no payloads to simulate, which leaves it nothing to be. They are
/// asked again rather than assumed: the files can change between the two
/// reads, and a simulator running on a half-read pair would be worse than one
/// that says so.
///
/// # Arguments
///
/// * `payload_paths` - The tcspecial configuration files: one, or several
///                     simulated together in one world, which the simulator
///                     file is then shared by.
/// * `sim_path`      - The simulator file.
/// * `mode`          - The `--simulate` setting.
/// * `startup`       - Where each problem found is said.
///
/// # Returns
///
/// The handlers, the simulator file, their settings and the payloads'
/// configurations, or `None` if anything stops the set being simulated.
fn load(
    payload_paths: &[&str],
    sim_path: &str,
    mode: Mode,
    startup: &mut Startup,
) -> Option<Loaded> {
    // How the configurations are named in what is said: as they always were
    // where there is one.
    let said = payload_paths.join(", ");
    let sim_file = match SimConfigFile::load(sim_path) {
        Ok(sim_file) => sim_file,
        Err(e) => {
            startup.error(format!(
                "Error loading simulator configuration from {sim_path}: {e}"
            ));
            return None;
        }
    };
    // One configuration of several sharing the file simulates its own
    // payloads alone, the others' naming theirs by their `configuration`.
    let sim_file = match payload_paths {
        [one] => sim_file.for_configuration(
            &tcsprotocol::sim_config::configuration_name(Path::new(one)),
        ),
        _ => sim_file,
    };

    // A configuration's payloads are nodes of a graph, and the ones simulated
    // are the ones the simulator file names: see
    // tcsprotocol::graph::simulated, which makes of each a data handler,
    // reached where tcspecial reaches it. Kept rather than dropped once the
    // handlers are made: a sensor joined to the world sends what it reports in
    // the encoding the graph's own conversion reads, and that is where the
    // encoding is.
    //
    // Several configurations share the one file, each taking the payloads
    // it has a node of: see `shared_out`, which holds the file to naming
    // each payload of exactly one of them.
    let mut graphs = Vec::with_capacity(payload_paths.len());
    let mut dh_configs = Vec::new();
    for payload_path in payload_paths {
        let loaded = tcsprotocol::graph::Graph::load(payload_path)
            .map_err(|e| e.to_string())
            .and_then(|read| {
                let handlers = if payload_paths.len() == 1 {
                    tcsprotocol::graph::simulated::handlers(&read, &sim_file)
                } else {
                    tcsprotocol::graph::simulated::handlers_in(&read, &sim_file)
                };
                graphs.push(read);
                handlers
            });
        match loaded {
            Ok(handlers) => dh_configs.extend(handlers),
            Err(e) => {
                startup.error(format!(
                    "Error loading payload configuration from \
                     {payload_path}: {e}"
                ));
                return None;
            }
        }
    }
    if payload_paths.len() > 1 {
        if let Err(e) = shared_out(payload_paths, &graphs, &sim_file) {
            startup.error(format!("Error in {sim_path} against {said}: {e}"));
            return None;
        }
    }

    // Join the two by name. Every data handler needs settings and every
    // settings entry needs a handler, so this is where a file naming a payload
    // that does not exist, or leaving one undriven, is caught.
    let sims = match sim_file.resolve(&dh_configs) {
        Ok(sims) => sims,
        Err(e) => {
            startup.error(format!("Error in {sim_path} against {said}: {e}"));
            return None;
        }
    };

    // Whether each payload's simulate agrees with what is at its endpoint, and
    // whether what this simulator will simulate, with what is a simulated or
    // dummy device already, is what the mode allows. Asked once the file is
    // known to say simulate once for every payload. A configuration pointing at
    // hardware in a file that says it is simulated, or a simulator file that
    // forgot a payload, is caught here, before anything runs; only tcspecial's
    // own check protects the hardware, because only tcspecial opens it.
    let mut refusing = false;
    for (graph, payload_path) in graphs.iter().zip(payload_paths) {
        for refusal in simulation_refused(graph, &sim_file, sim_path, mode) {
            startup.error(format!("{payload_path}: {refusal}"));
            refusing = true;
        }
    }
    if refusing {
        return None;
    }

    // The world, made once and shared: the sensors joined to it read the
    // same state the heaters joined to it warm, which is the whole point of
    // the world being one thing rather than a figure per payload.
    let read: Vec<&tcsprotocol::graph::Graph> = graphs.iter().collect();
    let (joined, thermostats) =
        match joined_to_the_world(&sim_file, &read, &sims, &dh_configs) {
            Ok(found) => found,
            Err(e) => {
                startup.error(format!("Error in {sim_path}: {e}"));
                return None;
            }
        };

    let configs: Vec<PayloadConfig> = match dh_configs
        .iter()
        .zip(&sims)
        .map(|(dh, sim)| {
            let mut config = payload_config_attached(
                dh,
                sim,
                joined.get(&dh.name.0).cloned(),
            )?;
            // A sensor a simulated thermostat reads reports to it.
            config.thermostats = thermostats
                .iter()
                .filter(|t| t.reads(&dh.name.0))
                .cloned()
                .collect();
            Ok(config)
        })
        .collect::<Result<_, String>>()
    {
        Ok(configs) => configs,
        Err(e) => {
            startup
                .error(format!("Error in payload configuration {said}: {e}"));
            return None;
        }
    };

    for (name, attachment) in &joined {
        eprintln!("{}", attachment.describe(name));
    }
    for thermostat in &thermostats {
        eprintln!(
            "tcs-sim simulates the thermostat {}, switching the heaters from \
             the sensors' readings",
            thermostat.name()
        );
    }

    if configs.is_empty() {
        let describe = if payload_paths.len() == 1 {
            "describes"
        } else {
            "describe"
        };
        startup.error(format!("{said} {describe} no payloads to simulate"));
        return None;
    }

    Some((dh_configs, sim_file, sims, configs))
}

/// Holds a simulator file shared by several configurations to naming each
/// payload it simulates as a payload of exactly one of them.
///
/// Everything tcs-sim keeps of a payload -- its panel, its settings, what it
/// is to the world, the thermostat it reports to -- is kept by its name, so
/// a name that is a payload of two configurations would be two payloads
/// with one name, and is refused; the configurations name theirs apart.
///
/// # Arguments
///
/// * `payload_paths` - The configuration files, beside `graphs`.
/// * `graphs`        - Their graphs.
/// * `sim_file`      - The simulator file they share.
///
/// # Errors
///
/// A message for the first name the file simulates that is a payload of
/// none of the configurations, or of more than one.
fn shared_out(
    payload_paths: &[&str],
    graphs: &[tcsprotocol::graph::Graph],
    sim_file: &SimConfigFile,
) -> Result<(), String> {
    for entry in &sim_file.simulated_payloads {
        let mut payload_of = Vec::new();
        let mut node_of = None;
        for (graph, path) in graphs.iter().zip(payload_paths) {
            match graph.node(&entry.name) {
                Some(node) if node.is_payload() => payload_of.push(*path),
                Some(node) => {
                    node_of = Some((*path, node.node_type.clone()));
                }
                None => {}
            }
        }
        match (payload_of.as_slice(), node_of) {
            ([path], _) => {
                let name = tcsprotocol::sim_config::configuration_name(
                    Path::new(path),
                );
                if let Some(said) =
                    entry.configuration.as_deref().filter(|said| **said != name)
                {
                    return Err(format!(
                        "\"{}\" is simulated as a payload of {said}, and it \
                         is a payload of {path}",
                        entry.name
                    ));
                }
            }
            ([], Some((path, node_type))) => {
                return Err(format!(
                    "\"{}\" is simulated, and it is a {node_type} node of \
                     {path}, not a payload",
                    entry.name
                ))
            }
            ([], None) => {
                return Err(format!(
                    "\"{}\" is simulated, and none of {} has a node of that \
                     name",
                    entry.name,
                    payload_paths.join(", ")
                ))
            }
            (several, _) => {
                return Err(format!(
                    "\"{}\" is simulated, and it is a payload of {}: one \
                     simulator file simulates a payload of one configuration, \
                     so the configurations simulated together name their \
                     payloads apart",
                    entry.name,
                    several.join(" and ")
                ))
            }
        }
    }
    Ok(())
}

/// Finds every way the payloads of a graph disagree with what the
/// simulator file says of them, or are not what the mode allows.
///
/// Each payload is compared with what is at its endpoint: see
/// `tcsprotocol::simulate::disagreement`. One the file says is simulated is
/// held to being reached at a simulated or dummy device, one it says is real to
/// not being, and one it does not name is refused where it is a simulated or
/// dummy device all the same, so that every payload simulated is one tcs-sim
/// controls and shows. For the mode, a payload is simulated when its simulate
/// is true, and a dummy device stays one.
///
/// # Arguments
///
/// * `graph`    - The tcspecial configuration's graph.
/// * `sim_file` - The simulator file.
/// * `sim_path` - Its path, for the messages.
/// * `mode`     - The `--simulate` setting.
///
/// # Returns
///
/// A message for each disagreement, or, where there are none, for each
/// refusal under the mode; empty when the pair can be simulated.
fn simulation_refused(
    graph: &Graph,
    sim_file: &SimConfigFile,
    sim_path: &str,
    mode: Mode,
) -> Vec<String> {
    let host = Host::this();
    let mut disagreements = Vec::new();
    let judged: Vec<(&str, Judged)> = graph
        .nodes
        .iter()
        .filter(|node| counted(node))
        .filter_map(|node| {
            let found = judge(&host, node, mode)?;
            let said = sim_file
                .simulated_payloads
                .iter()
                .find(|simulated| simulated.name == node.name)
                .and_then(|simulated| sim_file.simulate_of(simulated).ok());
            disagreements.extend(disagreement(
                &node.name,
                &node.node_type,
                said,
                &found,
            ));
            let judged = match said {
                Some(true) => Judged {
                    verdict: if found.is_dummy() {
                        Verdict::Dummy
                    } else {
                        Verdict::Simulated
                    },
                    found: format!("{sim_path} says simulate: true"),
                    ..found
                },
                _ => found,
            };
            Some((node.name.as_str(), judged))
        })
        .collect();
    if !disagreements.is_empty() {
        return disagreements;
    }
    let pairs: Vec<(&str, &Judged)> =
        judged.iter().map(|(name, j)| (*name, j)).collect();
    refusals(mode, &pairs)
        .into_iter()
        .map(|refusal| match mode {
            Mode::All => format!(
                "{refusal}, or name it in {sim_path} with simulate: true for \
                 tcs-sim to simulate"
            ),
            _ => refusal,
        })
        .collect()
}

/// Reads where this simulator says whether it is ready, and binds it, with
/// what it is set up for: or says what stops either.
///
/// # Arguments
///
/// * `payload_paths` - The tcspecial configuration files.
/// * `sim_path`      - The simulator file.
/// * `sim_file`      - The simulator file as read, which gives the address.
/// * `mode`          - The `--simulate` setting.
///
/// # Returns
///
/// The bound readiness socket, and what this simulator is set up for.
///
/// # Errors
///
/// A message naming the file if the address is wrong or cannot be bound,
/// or either file cannot be digested.
fn ready_socket(
    payload_paths: &[&str],
    sim_path: &str,
    sim_file: &SimConfigFile,
    mode: Mode,
) -> Result<(tcsos::net::UdpSocket, SetUpFor), String> {
    let at = sim_file.ready().map_err(|e| format!("{sim_path}: {e}"))?;
    let set_up_for = set_up_for(payload_paths, sim_path, mode)?;
    let socket = ready::bind(at).map_err(|e| format!("{sim_path}: {e}"))?;
    eprintln!("answering whether it is ready at {at}");
    Ok((socket, set_up_for))
}

/// Works out what a simulator started on these files under a mode is set
/// up for, as its readiness answers say and a harness compares.
///
/// # Arguments
///
/// * `payload_paths` - The tcspecial configuration files.
/// * `sim_path`      - The simulator file.
/// * `mode`          - The `--simulate` setting.
///
/// # Returns
///
/// The configurations' digest, one configuration's own or the digest of
/// several's, the simulator file's, and the mode.
///
/// # Errors
///
/// A message naming the files if any cannot be read for its digest.
fn set_up_for(
    payload_paths: &[&str],
    sim_path: &str,
    mode: Mode,
) -> Result<SetUpFor, String> {
    Ok(SetUpFor {
        configuration: tcsprotocol::config_digest::digest_of_files(
            payload_paths,
        )
        .map_err(|e| format!("{}: {e}", payload_paths.join(", ")))?,
        simulator: tcsprotocol::config_digest::digest_of_simulator_file(
            sim_path,
        )
        .map_err(|e| format!("{sim_path}: {e}"))?,
        mode,
    })
}

/// How long `--wait` waits where it is not told.
const WAIT_DEFAULT: tcsos::time::Duration =
    tcsos::time::Duration::from_secs(30);

/// Takes `--wait` or `--wait=<seconds>` out of the arguments.
///
/// # Arguments
///
/// * `args` - The command line's arguments.
///
/// # Returns
///
/// How long to wait, where it was given, and the arguments left.
///
/// # Errors
///
/// A message if the seconds are not a positive number, or `--wait` is
/// given twice.
fn take_wait(
    args: Vec<String>,
) -> Result<(Option<tcsos::time::Duration>, Vec<String>), String> {
    let mut waiting = None;
    let mut rest = Vec::with_capacity(args.len());
    for arg in args {
        let patience = if arg == "--wait" {
            WAIT_DEFAULT
        } else if let Some(seconds) = arg.strip_prefix("--wait=") {
            match seconds.parse::<f64>() {
                Ok(seconds) if seconds > 0.0 && seconds.is_finite() => {
                    tcsos::time::Duration::from_secs_f64(seconds)
                }
                _ => {
                    return Err(format!(
                        "--wait={seconds} is not a number of seconds: give \
                         --wait=30, or --wait for thirty seconds"
                    ))
                }
            }
        } else {
            rest.push(arg);
            continue;
        };
        if waiting.replace(patience).is_some() {
            return Err("--wait is given twice: give it once".to_string());
        }
    }
    Ok((waiting, rest))
}

/// What has every payload send from the start, as though each panel's
/// button had been pressed.
const SENDING_FLAG: &str = "--sending";

/// Takes `--sending` out of the arguments.
///
/// For a run with nobody at the window: a test harness that starts tcs-sim
/// and holds the ground to what the payloads send. Without it every payload
/// is silent until its panel is pressed, which is the run a person watches.
///
/// # Arguments
///
/// * `args` - The command line's arguments.
///
/// # Returns
///
/// Whether it was given, and the arguments left.
///
/// # Errors
///
/// A message if it is given twice, or given a value.
fn take_sending(args: Vec<String>) -> Result<(bool, Vec<String>), String> {
    let mut sending = false;
    let mut rest = Vec::with_capacity(args.len());
    for arg in args {
        if arg == SENDING_FLAG {
            if sending {
                return Err(format!(
                    "{SENDING_FLAG} is given twice: give it once"
                ));
            }
            sending = true;
        } else if arg.starts_with(&format!("{SENDING_FLAG}=")) {
            return Err(format!(
                "{arg}: {SENDING_FLAG} takes no value -- give it alone to \
                 have every payload send from the start"
            ));
        } else {
            rest.push(arg);
        }
    }
    Ok((sending, rest))
}

/// What names the simulator file on the command line.
const SIMULATOR_FLAG: &str = "--simulator";

/// Takes `--simulator=FILE` out of the arguments.
///
/// The one way to name the simulator file several configurations share: a
/// file beside a configuration and named for it is that configuration's, and
/// there is no one configuration for a shared file to be beside.
///
/// # Arguments
///
/// * `args` - The command line's arguments.
///
/// # Returns
///
/// The file, where it was given, and the arguments left.
///
/// # Errors
///
/// A message if it is given twice, or given without a file.
fn take_simulator(
    args: Vec<String>,
) -> Result<(Option<String>, Vec<String>), String> {
    let mut simulator = None;
    let mut rest = Vec::with_capacity(args.len());
    for arg in args {
        let Some(file) = arg.strip_prefix(&format!("{SIMULATOR_FLAG}=")) else {
            if arg == SIMULATOR_FLAG {
                return Err(format!(
                    "{SIMULATOR_FLAG} takes the simulator file: give \
                     {SIMULATOR_FLAG}=FILE"
                ));
            }
            rest.push(arg);
            continue;
        };
        if file.is_empty() {
            return Err(format!(
                "{SIMULATOR_FLAG}= names no file: give {SIMULATOR_FLAG}=FILE"
            ));
        }
        if simulator.replace(file.to_string()).is_some() {
            return Err(format!(
                "{SIMULATOR_FLAG} is given twice: give it once"
            ));
        }
    }
    Ok((simulator, rest))
}

/// Takes the configuration files from what is left of the command line.
///
/// One, or none, as every program takes it: see
/// `tcsprotocol::config::payload_path_from_args`, which falls back to
/// `SIM_PAYLOAD_CONFIG_PATH` and then to the default. Several, simulated
/// together in one world.
///
/// # Arguments
///
/// * `args` - What is left of the command line, the program's name first.
///
/// # Returns
///
/// The configuration files, in the order given.
///
/// # Errors
///
/// A message if an argument left is an option tcs-sim does not take.
fn payload_paths_from(args: Vec<String>) -> Result<Vec<String>, String> {
    if let Some(unknown) = args.iter().skip(1).find(|a| a.starts_with("--")) {
        return Err(format!(
            "tcs-sim: {unknown} is not an option tcs-sim takes\nusage: \
             tcs-sim [--simulate=MODE] [--sending] [--wait[=SECONDS]] \
             [{SIMULATOR_FLAG}=FILE] [CONFIGURATION...]"
        ));
    }
    if args.len() <= 2 {
        return Ok(vec![payload_path_from_args(
            args.into_iter(),
            Some(SIM_PAYLOAD_CONFIG_PATH_VAR),
        )?]);
    }
    Ok(args.into_iter().skip(1).collect())
}

/// Does `--wait`: waits for a simulator started on these files under a mode
/// to be ready, and says what to exit with. Nothing is simulated and no
/// window opens: this is the harness for a run started by hand.
///
/// # Arguments
///
/// * `payload_paths` - The tcspecial configuration files, in the order the
///                     simulator was given them.
/// * `sim_path`      - The simulator file, which says where to ask.
/// * `mode`          - The `--simulate` setting the simulator must have.
/// * `patience`      - How long to wait.
///
/// # Returns
///
/// The exit status: 0 once it is ready; 1 if it is not ready in time, is
/// set up for something else, or the files cannot be read.
fn wait_for_another(
    payload_paths: &[&str],
    sim_path: &str,
    mode: Mode,
    patience: tcsos::time::Duration,
) -> i32 {
    let wanted = SimConfigFile::load(sim_path)
        .map_err(|e| format!("{sim_path}: {e}"))
        .and_then(|sim_file| {
            sim_file.ready().map_err(|e| format!("{sim_path}: {e}"))
        })
        .and_then(|at| Ok((at, set_up_for(payload_paths, sim_path, mode)?)));
    let (at, ours) = match wanted {
        Ok(wanted) => wanted,
        Err(e) => {
            eprintln!("tcs-sim --wait: {e}");
            return 1;
        }
    };
    match tcsmoc::simulator::wait(at, &ours, patience) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("tcs-sim --wait: {e}");
            1
        }
    }
}

/// Says what was found as tcs-sim started, in a window of its own, and waits
/// for it to be closed.
///
/// Before the main window is shown, so that nobody is looking at panels for a
/// pair with something wrong with it without having been told -- and on
/// stderr as well, for a run from a terminal and for one with no display to
/// open a window on.
///
/// # Arguments
///
/// * `startup` - What was found.
fn present(startup: &Startup) {
    startup.eprint("tcs-sim");

    let heading = if startup.fatal() {
        "Something is wrong with the configuration."
    } else {
        "Something about the configuration is worth knowing."
    };
    let everything = startup.copied(heading);
    let fatal = startup.fatal();
    let lines: Vec<StartupLine> = startup
        .lines()
        .iter()
        .map(|(error, text)| StartupLine {
            text: text.clone().into(),
            error: *error,
        })
        .collect();
    tcs_ui::windows::on_window_thread(move |closed| {
        let window = match StartupWindow::new() {
            Ok(window) => window,
            Err(e) => {
                eprintln!(
                    "tcs-sim: what was found cannot be shown in a window: {e}"
                );
                return;
            }
        };
        window.set_program("tcs-sim".into());
        window.set_heading(heading.into());
        window.set_everything(everything.into());
        window.set_fatal(fatal);
        window.set_lines(ModelRc::from(Rc::new(VecModel::from(lines))));
        {
            let weak = window.as_weak();
            window.on_acknowledged(move || {
                if let Some(window) = weak.upgrade() {
                    let _ = tcs_ui::windows::hide(&window);
                }
            });
        }
        // Runs until it is closed, by its button or by the window manager:
        // either is the person having read it.
        if let Err(e) = tcs_ui::windows::run(window, (), closed) {
            eprintln!(
                "tcs-sim: what was found cannot be shown in a window: {e}"
            );
        }
    });
}

/// Runs tcs-sim: reads and checks the tcspecial configuration and its
/// simulator file, simulates each payload it names, and shows
/// a panel for each until the window is closed. With `--wait` it instead
/// waits for another simulator to be ready.
///
/// Exits with status 1 if the arguments, the mode or the files are wrong,
/// or the readiness socket cannot be bound.
///
/// # Panics
///
/// If the window cannot be made or run.
pub fn main() {
    //    env_logger::init();

    // The mode first, and before anything is read: under the flight setting
    // there is nothing to read anything for.
    let (mode, args) =
        match tcsprotocol::simulate::take_mode(env::args().collect()) {
            Ok(taken) => taken,
            Err(e) => {
                eprintln!("tcs-sim: {e}");
                process::exit(1);
            }
        };
    let (waiting, args) = match take_wait(args) {
        Ok(taken) => taken,
        Err(e) => {
            eprintln!("tcs-sim: {e}");
            process::exit(1);
        }
    };
    let (sending, args) = match take_sending(args) {
        Ok(taken) => taken,
        Err(e) => {
            eprintln!("tcs-sim: {e}");
            process::exit(1);
        }
    };
    let (simulator, args) = match take_simulator(args) {
        Ok(taken) => taken,
        Err(e) => {
            eprintln!("tcs-sim: {e}");
            process::exit(1);
        }
    };
    if mode.is_flight() {
        eprintln!(
            "tcs-sim: --simulate={mode} is the flight setting, which allows \
             only real devices, so there is nothing for a simulator to do or \
             to wait for: give --simulate=some or --simulate=all for a run \
             that simulates"
        );
        process::exit(1);
    }

    // Read both configuration files before the window exists, so a file that
    // cannot be read is reported plainly rather than as an empty window.
    let payload_paths = match payload_paths_from(args) {
        Ok(payload_paths) => payload_paths,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };
    let payload_paths: Vec<&str> =
        payload_paths.iter().map(String::as_str).collect();
    // The one --simulator names, then the one PAYLOAD_SIM_YAML names, and
    // otherwise, for one configuration, the one beside it and named for it,
    // found as tcs-moc finds it. Several configurations share a file, which
    // is beside none of them, so it is named.
    let sim_path =
        match simulator.or_else(|| env::var(SIM_CONFIG_PATH_VAR).ok()) {
            Some(sim_path) => sim_path,
            None => match payload_paths.as_slice() {
                [payload_path] => {
                    tcsprotocol::config::sim_path_beside(payload_path)
                        .unwrap_or_else(|| DEFAULT_SIM_CONFIG_PATH.to_string())
                }
                several => {
                    eprintln!(
                    "tcs-sim: {} configurations are simulated together, and \
                     nothing says which simulator file they share: give \
                     {SIMULATOR_FLAG}=FILE, or set {SIM_CONFIG_PATH_VAR}",
                    several.len()
                );
                    process::exit(1);
                }
            },
        };

    if let Some(patience) = waiting {
        process::exit(wait_for_another(
            &payload_paths,
            &sim_path,
            mode,
            patience,
        ));
    }

    // Everything wrong with the pair, in tcs-verify's words and before
    // anything else is read from either: every problem at once, each with its
    // file and line, which is what tcs-verify, tcs-moc, tcspecial and tcs-cfge
    // say about the same files. Then what only a simulator has to ask. All of
    // it is shown before the main window is, and the window is not shown at
    // all where any of it is an error.
    //
    // Several configurations sharing a simulator file are each verified on
    // their own: verified with the file, each would find the others'
    // payloads in it unknown. The file is then held to the set of them as
    // it is read: see `load` and `shared_out`.
    let mut startup = Startup::default();
    match payload_paths.as_slice() {
        [payload_path] => startup.report(
            &tcsverify::verify(
                Path::new(payload_path),
                Some(Path::new(&sim_path)),
                tcsverify::Limits::EVERYTHING,
            ),
            true,
        ),
        several => {
            for payload_path in several {
                startup.report(
                    &tcsverify::verify(
                        Path::new(payload_path),
                        None,
                        tcsverify::Limits::EVERYTHING,
                    ),
                    true,
                );
            }
        }
    }
    let loaded = if startup.fatal() {
        None
    } else {
        load(&payload_paths, &sim_path, mode, &mut startup)
    };
    if !startup.is_empty() {
        present(&startup);
    }
    let Some((dh_configs, sim_file, sims, configs)) =
        loaded.filter(|_| !startup.fatal())
    else {
        process::exit(1);
    };
    // The panels in their payloads' names' order, left to right and top to
    // bottom: the payloads, their settings and their configurations put in
    // that order together, since a panel's place is what finds its payload.
    // Each keeps its id, its place in the configuration.
    let order = name_order(&dh_configs);
    let dev_ids = order.clone();
    let dh_configs = in_order(dh_configs, &order);
    let sims = in_order(sims, &order);
    let configs = in_order(configs, &order);
    // A world with a lander is shown in a window of its own, beside the
    // panels: see `lander_view`. Found now, while the payloads, which reach
    // the world, are still to hand.
    let lander = lander_view::scene(&sim_file, &configs);

    // Where this simulator says whether it is ready, bound before any
    // payload is simulated: a harness asking while it starts is told so, and
    // an address already held -- by a simulator left running -- stops this
    // one before it fights that one for the payloads' paths.
    let readiness =
        match ready_socket(&payload_paths, &sim_path, &sim_file, mode) {
            Ok(readiness) => readiness,
            Err(e) => {
                let mut refused = Startup::default();
                refused.error(e);
                present(&refused);
                process::exit(1);
            }
        };

    // The cFS end of a configuration that answers to cFS: tcs-sim is the
    // software bus the vehicle joins as an SBN peer, and the ground joins
    // too, as the radio's far end is the ground itself. Made before the
    // readiness socket says ready, so that a vehicle started then finds it;
    // kept until tcs-sim ends. See `tcsmoc::bus`. One for each
    // configuration that answers to cFS.
    let mut _cfs_ends = Vec::new();
    for payload_path in &payload_paths {
        match Graph::load(payload_path)
            .map_err(|e| format!("{payload_path}: {e}"))
            .and_then(|graph| tcsmoc::bus::SimulatedCfs::open(&graph))
        {
            Ok(cfs) => {
                if let Some(cfs) = &cfs {
                    eprintln!("simulating the cFS end at {}", cfs.local_addr());
                }
                _cfs_ends.push(cfs);
            }
            Err(e) => {
                let mut refused = Startup::default();
                refused.error(e);
                present(&refused);
                process::exit(1);
            }
        }
    }

    eprintln!(
        "Simulating {} payloads from {} and {}",
        configs.len(),
        payload_paths.join(", "),
        sim_path
    );
    if let Some(description) = &sim_file.description {
        eprintln!("Simulator configuration: {description}");
    }

    // Before any payload is simulated, so that nothing is left behind by a
    // signal.
    let signalled = match note_quit_signals() {
        Ok(signalled) => signalled,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };

    // One panel per payload, in a grid worked out from how many there are:
    // given before the window is measured, a panel being as wide as its
    // widest payload wants.
    let mut infos: Vec<PayloadInfo> = dh_configs
        .iter()
        .zip(&sims)
        .zip(&dev_ids)
        .map(|((dh, sim), &dev_id)| payload_info_from(dev_id, dh, sim))
        .collect();
    let count = configs.len();

    // Every simulated payload made now, and each silent until its panel says
    // otherwise: what tcspecial finds is then the same whatever anyone has
    // pressed, and the readiness socket can say when all of it is there.
    let (socket, set_up_for) = readiness;
    ready::serve(
        socket,
        Arc::new(ready::Readiness {
            payloads: configs.iter().map(|c| c.standing.clone()).collect(),
            set_up_for,
        }),
    );
    let mut made: Vec<SimulatedPayload> =
        configs.into_iter().map(SimulatedPayload::new).collect();
    for (row, payload) in made.iter_mut().enumerate() {
        // A payload not yet running, so this cannot be refused.
        let _ = payload.start();
        // As though its button had been pressed, where --sending asks, or
        // the simulator file's enable says so of this payload: which for a
        // heater is to power it.
        let asked = sending || sims.get(row).is_some_and(|sim| sim.enabled);
        if asked {
            payload.send(true);
            if let Some(info) = infos.get_mut(row) {
                info.status = SharedString::from(RUNNING_STATUS);
            }
        }
        if let Some(info) = infos.get_mut(row) {
            info.driving = SharedString::from(payload.driving_said());
        }
    }
    let payloads: Arc<Mutex<Vec<SimulatedPayload>>> =
        Arc::new(Mutex::new(made));

    // The window, made where windows are made -- which in a web page is not
    // this thread -- and run until it closes.
    {
        let signalled = signalled.clone();
        tcs_ui::windows::on_window_thread(move |closed| {
            show_panels(infos, count, lander, payloads, signalled, closed);
        });
    }

    let signal = signalled.load(Ordering::SeqCst);
    if signal != 0 {
        eprintln!("tcs-sim: stopped by signal {signal}");
        process::exit(128 + signal as i32);
    }
}

/// Makes the main window, with a panel for each payload, and runs it until
/// it is closed: by its Quit button, by a quit signal, or by whoever is
/// looking at it.
///
/// # Arguments
///
/// * `infos`     - The panels, in order, as they start.
/// * `count`     - How many payloads there are.
/// * `lander`    - The lander's world, where there is one, to show beside
///                 the panels.
/// * `payloads`  - The simulated payloads, in the panels' order.
/// * `signalled` - The quit signal taken, or 0.
/// * `closed`    - What to say the window has closed with: see
///                 [`tcs_ui::windows`].
///
/// # Panics
///
/// If the window cannot be made or run.
fn show_panels(
    infos: Vec<PayloadInfo>,
    count: usize,
    lander: Option<lander_view::Scene>,
    payloads: Arc<Mutex<Vec<SimulatedPayload>>>,
    signalled: Arc<AtomicUsize>,
    closed: tcs_ui::windows::Closed,
) {
    let ui = MainWindow::new().unwrap();
    // Room for the lander's pane, where the world has a lander, before the
    // window is measured: its chrome counts the pane.
    if lander.is_some() {
        lander_view::make_room(&ui);
    }
    let ui_weak = ui.as_weak();

    let model: Rc<VecModel<PayloadInfo>> = Rc::new(VecModel::from(infos));
    ui.set_payload_model(ModelRc::from(model.clone()));
    // No more than this in a row, however wide the window is made.
    ui.set_most_columns(i32::try_from(grid::MAX_COLUMNS).unwrap_or(1));
    let sizes = grid::settle(&ui);

    // What the markup says a panel and the chrome come to, asked for rather
    // than written down here as well: see grid::Sizes.
    let shape = grid::grid_shape(&sizes, count);
    // Taller than the grid asks for where there is a third row to look
    // ahead for, and no taller than the payloads on hand need otherwise. It
    // is also why the window can open taller than it is wide, which the shape
    // itself never is: the shape rule chooses the grid, and the room rule
    // says how much of the window to give it.
    // And at least as tall as the lander's pane, where there is one.
    let size = grid::window_size(&sizes, &shape, count);
    ui.window().set_size(slint::LogicalSize::new(
        size.width,
        size.height.max(ui.get_least_height()),
    ));

    handle_transfer_button(&ui, payloads.clone(), model.clone());

    // What the Quit button does, and a signal: every payload stopped, which
    // clears what it made, and the window closed, which ends the event loop.
    let quit: Rc<dyn Fn()> = {
        let payloads = payloads.clone();
        let ui_weak = ui_weak.clone();
        Rc::new(move || {
            let mut guard = payloads.lock().unwrap();
            for payload in guard.iter_mut() {
                payload.stop();
            }
            drop(guard);

            let ui = ui_weak.unwrap();
            tcs_ui::windows::hide(&ui).unwrap();
        })
    };
    {
        let quit = quit.clone();
        ui.on_quit_clicked(move || quit());
    }
    quit_when_signalled(signalled, quit);

    // Periodic update timer, kept as long as the window is.
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        tcsos::time::Duration::from_millis(500),
        move || show_progress(&payloads, &model),
    );

    // Against the bottom of the display, tcs-moc having asked for the top;
    // or, with a lander's pane, at the top right, leaving the rest of the
    // width to the lander example's MOC window.
    if lander.is_some() {
        ask_for_the_top_right(&ui);
    } else {
        ask_for_the_edge(&ui, Edge::Bottom);
    }

    // Kept while the window is: the timer that keeps the lander's pane up
    // to the world.
    let lander = lander.map(|scene| lander_view::attach(&ui, scene));

    tcs_ui::windows::run(ui, (timer, lander), closed).unwrap();
}

/// How often the event loop looks for a quit signal.
const SIGNAL_POLL: Duration = Duration::from_millis(100);

/// The status tcs-sim exits with on a second quit signal, which does not
/// wait for the payloads to stop.
const SECOND_SIGNAL_STATUS: i32 = 1;

/// Has each quit signal -- SIGHUP, SIGINT and SIGTERM -- be noted rather
/// than end tcs-sim, so that the event loop can stop the payloads and clear
/// what they made first. A second, should that take too long, ends tcs-sim
/// at once.
///
/// # Returns
///
/// The number of the signal that came, which is 0 until one has.
///
/// # Errors
///
/// Why a signal could not be handled, naming it.
fn note_quit_signals() -> Result<Arc<AtomicUsize>, String> {
    tcsos::signals::note_quits(SECOND_SIGNAL_STATUS)
        .map_err(|e| format!("tcs-sim: {e}"))
}

/// Runs `quit` from the event loop once a signal has come, and only once.
///
/// # Arguments
///
/// * `signalled` - The number of the signal that came, or 0; see
///                 `note_quit_signals`.
/// * `quit`      - What the Quit button does.
fn quit_when_signalled(signalled: Arc<AtomicUsize>, quit: Rc<dyn Fn()>) {
    let done = Cell::new(false);
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, SIGNAL_POLL, move || {
        if !done.get() && signalled.load(Ordering::SeqCst) != 0 {
            done.set(true);
            quit();
        }
    });
    // For as long as the event loop runs, as the update timer is kept.
    std::mem::forget(timer);
}

/// The environment variable naming the file tcs-sim writes, with a lander's
/// pane, the width the display has left beside its window into: what the
/// lander example's ground station sizes its window to.
const ROOM_FILE: &str = "TCS_SIM_ROOM_FILE";

/// Asks for this window to be put at the top right of the display, and says
/// how much width that leaves to its left, in the file [`ROOM_FILE`] names,
/// where it names one.
///
/// Asked once the event loop is running, as [`ask_for_the_edge`] asks, for
/// the window to have its size. A request: a Wayland compositor may place
/// the window where it likes, and the lander example's window rule for KDE
/// puts it there in any case. The width left is the display's, in logical
/// pixels -- its physical width over the window's scale -- less the
/// window's, written as a whole number; nothing is written where the
/// display's width is unknown.
///
/// # Arguments
///
/// * `ui` - The main window.
fn ask_for_the_top_right(ui: &MainWindow) {
    let ui_weak = ui.as_weak();
    let timer = slint::Timer::default();
    // A second after the loop starts, not at once: a Wayland compositor
    // tells a window its scale after it is shown, and until then it reads
    // as one, which would make the room left far too wide.
    timer.start(
        slint::TimerMode::SingleShot,
        tcsos::time::Duration::from_secs(1),
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let window = ui.window();
            let Some(screen) = tcsprotocol::display::screen_width() else {
                return;
            };
            let wide = window.size().width;
            let left = screen.saturating_sub(wide);
            window.set_position(slint::PhysicalPosition::new(
                i32::try_from(left).unwrap_or(0),
                0,
            ));
            if let Ok(path) = tcsos::env::var(ROOM_FILE) {
                let room = f64::from(left) / f64::from(window.scale_factor());
                if let Err(e) = tcsos::fs::write(&path, format!("{room:.0}\n"))
                {
                    eprintln!("tcs-sim: {path}: {e}");
                }
            }
        },
    );
    // As ask_for_the_edge's: dropped, it would never fire.
    std::mem::forget(timer);
}

/// Asks for this window to be put against one edge of the display.
///
/// Asked once the event loop is running, because that is when the window has
/// a size: before it is shown, what it reports is what was requested rather
/// than what it was given, and the bottom of the display is the display's
/// height less the window's.
///
/// A request, and not a thing to report on: see `tcsprotocol::display`.
///
/// # Arguments
///
/// * `ui`   - The main window.
/// * `edge` - The edge to put it against.
fn ask_for_the_edge(ui: &MainWindow, edge: Edge) {
    let ui_weak = ui.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::SingleShot,
        tcsos::time::Duration::ZERO,
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let window = ui.window();
            if let Some(y) = tcsprotocol::display::top_of(
                edge,
                tcsprotocol::screen_height(),
                window.size().height,
            ) {
                window.set_position(slint::PhysicalPosition::new(0, y));
            }
        },
    );
    // The timer fires once and is done; dropping it here would cancel it
    // before the loop had a chance to run it.
    std::mem::forget(timer);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A list put in an order has each item once, where the order puts it.
    #[test]
    fn a_list_goes_into_its_order() {
        assert_eq!(in_order(vec!["c", "a", "b"], &[1, 2, 0]), ["a", "b", "c"]);
        assert!(in_order(Vec::<u8>::new(), &[]).is_empty());
    }
    use std::path::Path;
    use tcsos::time::Duration;
    use tcsprotocol::{DHName, EndpointConfig, NetworkConfig, NetworkProtocol};

    /// `--sending` is taken out wherever it is, once, and nothing else is;
    /// twice, or with a value, is refused saying so.
    #[test]
    fn sending_is_a_flag_given_once() {
        let args = |said: &[&str]| -> Vec<String> {
            said.iter().map(|s| s.to_string()).collect()
        };
        assert_eq!(
            take_sending(args(&["tcs-sim", "x.ytcs"])),
            Ok((false, args(&["tcs-sim", "x.ytcs"])))
        );
        assert_eq!(
            take_sending(args(&["tcs-sim", "--sending", "x.ytcs"])),
            Ok((true, args(&["tcs-sim", "x.ytcs"])))
        );
        let twice = take_sending(args(&["--sending", "x", "--sending"]))
            .expect_err("twice");
        assert!(twice.contains("twice"), "{twice}");
        let valued =
            take_sending(args(&["--sending=yes"])).expect_err("a value");
        assert!(valued.contains("takes no value"), "{valued}");
    }

    fn repo_file(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(name)
    }

    /// Writes two configurations into a directory -- the minimal example's,
    /// and the same again with its payload renamed and moved to another
    /// port -- and one simulator file for both, simulating each payload.
    ///
    /// # Arguments
    ///
    /// * `dir`    - The directory.
    /// * `second` - The name the second configuration's payload is given.
    /// * `extra`  - A further name the simulator file simulates, if any.
    ///
    /// # Returns
    ///
    /// The two configurations' paths and the simulator file's.
    fn two_minimals(
        dir: &Path,
        second: &str,
        extra: Option<&str>,
    ) -> (String, String, String) {
        let minimal = tcsos::fs::read_to_string(repo_file(
            "examples/minimal/minimal.ytcs",
        ))
        .expect("the example reads");
        let a = dir.join("a.ytcs");
        let b = dir.join("b.ytcs");
        tcsos::fs::write(&a, &minimal).expect("written");
        let renamed = minimal
            .replace("payload: sensor", &format!("payload: {second}"))
            .replace("node: sensor", &format!("node: {second}"))
            .replace("127.0.0.28:5001", "127.0.0.28:5002");
        tcsos::fs::write(&b, renamed).expect("written");
        let entry = |name: &str| {
            format!(
                "      - payload: {name}\n        packet_interval: 1 s\n\
                 \x20       protocol: udp\n        simulate: true\n"
            )
        };
        let mut sim = String::from(
            "tcs-sim:\n  ready: \"127.0.0.1:7999\"\nsimulated_payloads:\n  \
             - type network:\n",
        );
        sim.push_str(&entry("sensor"));
        if second != "sensor" {
            sim.push_str(&entry(second));
        }
        if let Some(extra) = extra {
            sim.push_str(&entry(extra));
        }
        let sim_path = dir.join("both-sim.yaml");
        tcsos::fs::write(&sim_path, sim).expect("written");
        let path = |p: &Path| p.to_str().expect("a path").to_string();
        (path(&a), path(&b), path(&sim_path))
    }

    /// Two configurations sharing one simulator file are simulated in one
    /// world: each payload with its own configuration's endpoint.
    #[test]
    fn two_configurations_are_simulated_together() {
        let dir = tempfile::tempdir().expect("a directory");
        let (a, b, sim) = two_minimals(dir.path(), "probe", None);
        let mut startup = Startup::default();
        let (dh_configs, _, _, configs) =
            load(&[&a, &b], &sim, Mode::All, &mut startup)
                .unwrap_or_else(|| panic!("refused: {:?}", startup.lines()));
        assert!(startup.is_empty(), "{:?}", startup.lines());
        let names: Vec<&str> =
            dh_configs.iter().map(|dh| dh.name.0.as_str()).collect();
        assert_eq!(names, ["sensor", "probe"]);
        assert_eq!(configs.len(), 2);
        let ports: Vec<String> = dh_configs
            .iter()
            .map(|dh| format!("{:?}", dh.endpoint))
            .collect();
        assert!(ports[0].contains("5001"), "{ports:?}");
        assert!(ports[1].contains("5002"), "{ports:?}");
    }

    /// A payload two configurations both have is refused where they share a
    /// simulator file: tcs-sim keeps each payload by its name.
    #[test]
    fn a_payload_of_two_configurations_is_refused() {
        let dir = tempfile::tempdir().expect("a directory");
        let (a, b, sim) = two_minimals(dir.path(), "sensor", None);
        let mut startup = Startup::default();
        assert!(load(&[&a, &b], &sim, Mode::All, &mut startup).is_none());
        assert!(
            startup.lines().iter().any(|(error, line)| *error
                && line.contains(
                    "\"sensor\" is simulated, and it is a \
                                  payload of"
                )),
            "{:?}",
            startup.lines()
        );
    }

    /// A name the shared file simulates that no configuration has is
    /// refused, naming every configuration it was looked for in.
    #[test]
    fn a_payload_of_no_configuration_is_refused() {
        let dir = tempfile::tempdir().expect("a directory");
        let (a, b, sim) = two_minimals(dir.path(), "probe", Some("ghost"));
        let mut startup = Startup::default();
        assert!(load(&[&a, &b], &sim, Mode::All, &mut startup).is_none());
        assert!(
            startup.lines().iter().any(|(error, line)| *error
                && line.contains("\"ghost\" is simulated, and none of")),
            "{:?}",
            startup.lines()
        );
    }

    /// What a simulator says it is set up for: one configuration's own
    /// digest, as every harness comparing against one configuration
    /// expects, and for several a digest of theirs, in the order given.
    #[test]
    fn a_set_of_configurations_has_one_digest() {
        let dir = tempfile::tempdir().expect("a directory");
        let (a, b, sim) = two_minimals(dir.path(), "probe", None);
        let one = set_up_for(&[&a], &sim, Mode::All).expect("digested");
        assert_eq!(
            one.configuration,
            tcsprotocol::config_digest::digest_of_file(&a).expect("digested")
        );
        let ab = set_up_for(&[&a, &b], &sim, Mode::All).expect("digested");
        let ba = set_up_for(&[&b, &a], &sim, Mode::All).expect("digested");
        assert_ne!(ab.configuration, one.configuration);
        assert_ne!(ab.configuration, ba.configuration);
        assert_eq!(
            ab.configuration,
            set_up_for(&[&a, &b], &sim, Mode::All)
                .expect("digested")
                .configuration
        );
    }

    /// --simulator names the simulator file, once and with a file; the
    /// configurations are what is left.
    #[test]
    fn the_simulator_file_is_named_once() {
        let args = |words: &[&str]| -> Vec<String> {
            words.iter().map(|w| w.to_string()).collect()
        };
        let (simulator, rest) =
            take_simulator(args(&["tcs-sim", "--simulator=s.yaml", "a", "b"]))
                .expect("taken");
        assert_eq!(simulator.as_deref(), Some("s.yaml"));
        assert_eq!(rest, args(&["tcs-sim", "a", "b"]));
        assert_eq!(payload_paths_from(rest).expect("taken"), args(&["a", "b"]));
        assert!(take_simulator(args(&["tcs-sim", "--simulator"])).is_err());
        assert!(take_simulator(args(&["tcs-sim", "--simulator="])).is_err());
        assert!(take_simulator(args(&[
            "tcs-sim",
            "--simulator=a",
            "--simulator=b"
        ]))
        .is_err());
        let bogus = args(&["tcs-sim", "--bogus", "a"]);
        assert!(payload_paths_from(bogus).is_err());
    }

    /// An example is simulated: its simulated payload nodes, reached where
    /// tcspecial reaches them, with the simulator file beside it found as
    /// tcs-moc finds it.
    #[test]
    fn an_example_is_simulated() {
        let payload = repo_file("examples/minimal/minimal.ytcs");
        let payload = payload.to_str().expect("a path");
        let sim = tcsprotocol::config::sim_path_beside(payload)
            .expect("its simulator file");
        assert!(sim.ends_with("minimal-sim.yaml"), "{sim}");

        let mut startup = Startup::default();
        let (dh_configs, _, sims, configs) =
            load(&[payload], &sim, Mode::All, &mut startup).expect("it loads");
        assert!(startup.is_empty(), "{:?}", startup.lines());
        assert_eq!(dh_configs.len(), 1);
        assert_eq!(dh_configs[0].name.0, "sensor");
        assert_eq!(sims.len(), 1);
        assert_eq!(configs.len(), 1);
    }

    /// A payload's simulate is held to what is at its endpoint: true where
    /// it is hardware is refused, and so is false where it is a simulated
    /// device -- here, an rpmsg channel reached at its sim_address.
    #[test]
    fn simulate_is_held_to_what_is_at_the_endpoint() {
        let dir = tempfile::tempdir().unwrap();
        let configuration = |simulated: bool| {
            let path = dir.path().join("set.ytcs");
            let at = if simulated {
                "        sim_address: \"127.0.0.1:9\"\n"
            } else {
                ""
            };
            tcsos::fs::write(
                &path,
                format!(
                    "payloads:
  - type rpmsg:
      - payload: channel
        path: \"/dev/null\"
        service: \"payload-telemetry\"
{at}"
                ),
            )
            .unwrap();
            tcsprotocol::graph::Graph::load(&path).expect("it reads")
        };
        let sim = |simulate: bool| -> SimConfigFile {
            tcsprotocol::format::ConfigFormat::Yaml
                .parse(&format!(
                    "simulated_payloads:\n  - type rpmsg:\n      \
                     - payload: channel\n        simulate: {simulate}\n"
                ))
                .unwrap()
        };

        assert!(simulation_refused(
            &configuration(true),
            &sim(true),
            "s.ytcs",
            Mode::All
        )
        .is_empty());
        let said = simulation_refused(
            &configuration(false),
            &sim(true),
            "s.ytcs",
            Mode::All,
        );
        assert!(said[0].contains("says simulate: true"), "{said:?}");
        let said = simulation_refused(
            &configuration(true),
            &sim(false),
            "s.ytcs",
            Mode::Some,
        );
        assert!(
            said[0].contains(
                "a simulated device, and the simulator file says simulate: \
                 false"
            ),
            "{said:?}"
        );
        assert!(simulation_refused(
            &configuration(false),
            &sim(false),
            "s.ytcs",
            Mode::Some
        )
        .iter()
        .all(|refusal| !refusal.contains("simulate: false")));
    }

    /// One whose simulator file names nothing is refused, saying so: there is
    /// nothing for a simulator to be.
    #[test]
    fn an_example_with_nothing_simulated_is_refused() {
        let payload = repo_file("examples/simple-app/simple-app.ytcs");
        let payload = payload.to_str().expect("a path");
        let sim = tcsprotocol::config::sim_path_beside(payload)
            .expect("its simulator file");

        let mut startup = Startup::default();
        assert!(load(&[payload], &sim, Mode::All, &mut startup).is_none());
        assert!(
            startup.lines().iter().any(|(error, line)| *error
                && line.contains("no payloads to simulate")),
            "{:?}",
            startup.lines()
        );
    }

    /// Makes a sample of a transfer, as the stats hold it.
    ///
    /// # Arguments
    ///
    /// * `seconds` - When it moved, in seconds since the epoch, which is
    ///               what these tests mean; a time is carried in
    ///               microseconds.
    /// * `data`    - The packet.
    ///
    /// # Returns
    ///
    /// The sample.
    fn sample_of(seconds: u64, data: &[u8]) -> DHSample {
        let mut sample = DHSample::new();
        sample.record(data);
        sample.time = Some(tcsprotocol::Timestamp {
            micros: seconds * 1_000_000,
        });
        sample
    }

    /// The window size the design document states is the one the rule gives.
    ///
    /// It said 640x496 for a long time against the 664x722 the program opened
    /// at: the number went stale when the panels gained their three rows of
    /// data and the window its three-row floor, and nothing noticed because
    /// nothing read it. Anyone sizing a screen or a screenshot from the
    /// document was being told the wrong thing.
    #[test]
    fn the_design_document_states_the_size_the_window_opens_at() {
        let doc = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("docs/tcs-sim-design.rst");
        let text = tcsos::fs::read_to_string(&doc).unwrap();

        // The four payloads of tcspecial1.ytcs, which is the set the document
        // describes.
        // The fixture rather than a window: the Slint backend belongs to one
        // thread and one test, and what keeps the fixture equal to the markup
        // is the_markup_reports_the_sizes_the_tests_assume.
        let sizes = grid::Sizes::for_tests();
        let shape = grid::grid_shape(&sizes, 4);
        let size = grid::window_size(&sizes, &shape, 4);
        let said = format!("{}x{}", size.width, size.height);

        assert_eq!(
            (shape.columns, shape.rows),
            (3, 2),
            "the document describes a row of three and a row of one"
        );
        assert!(
            text.contains(&said),
            "{} does not say the window opens at {said}",
            doc.display()
        );
    }

    /// What the window's one panel button says each way.
    ///
    /// Here rather than beside the statuses because the window owns its
    /// labels: Slint builds the string, and these are how the test below
    /// holds it to the same rule Rust decides a press by.
    const DISABLE_LABEL: &str = "Disable";
    const ENABLE_LABEL: &str = "Enable";

    impl Transfer {
        /// Gives the label a button offering this carries.
        ///
        /// # Returns
        ///
        /// `Enable` for `Send`, `Disable` for `Silent`.
        fn label(&self) -> &'static str {
            match self {
                Transfer::Send => ENABLE_LABEL,
                Transfer::Silent => DISABLE_LABEL,
            }
        }
    }

    /// A panel's one button offers what pressing it will do, and Rust does
    /// what the label offered.
    ///
    /// There were two buttons, Start and Stop, and either was pressable
    /// whatever the payload was doing: a Stop on a payload that was not
    /// sending did nothing and read as though it had. One button offers the
    /// one thing worth doing next -- and because Slint decides the label and
    /// Rust decides what a press does, both from the same status, the two
    /// have to be checked against each other or they drift into offering one
    /// thing and doing the other.
    #[test]
    fn the_transfer_button_does_what_its_label_offers() {
        // A payload that is sending offers to be disabled, and one that is
        // not offers to be enabled. Anything that is not the running status
        // offers to start: a payload that failed to start shows the stopped
        // status, and the useful thing to offer then is the start that
        // failed.
        assert_eq!(transfer_wanted(RUNNING_STATUS), Transfer::Silent);
        assert_eq!(transfer_wanted(STOPPED_STATUS), Transfer::Send);
        assert_eq!(transfer_wanted(""), Transfer::Send);

        assert_eq!(Transfer::Silent.label(), DISABLE_LABEL);
        assert_eq!(Transfer::Send.label(), ENABLE_LABEL);

        // And the window labels it by the same rule, which is the half Rust
        // cannot fail on: the label is a Slint expression, so a drift between
        // the two files is a button that offers Enable and stops the
        // payload.
        let window =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/main.slint");
        let source = tcsos::fs::read_to_string(&window).unwrap();
        let offered = format!(
            "payload-status == \"{}\" ? \"{}\" : \"{}\"",
            RUNNING_STATUS,
            transfer_wanted(RUNNING_STATUS).label(),
            transfer_wanted(STOPPED_STATUS).label()
        );
        assert!(
            source.contains(&offered),
            "{} does not label its transfer button {:?}",
            window.display(),
            offered
        );

        // The status line is coloured and weighted by the same status, so it
        // and the button cannot come to disagree either.
        assert!(
            source.contains(&format!(
                "payload-status == \"{}\" ? black : red",
                RUNNING_STATUS
            )),
            "{} does not colour the status line on {:?}",
            window.display(),
            RUNNING_STATUS
        );

        // One button, not two: the pair they replaced took the same room and
        // left a press available that did nothing.
        for gone in ["text: \"Start\"", "text: \"Stop\""] {
            assert!(
                !source.contains(gone),
                "{} still has a panel button saying {gone}",
                window.display()
            );
        }
    }

    /// A panel shows when the last packet went each way and what it was, and
    /// says plainly when there has not been one.
    ///
    /// The time is the point of the two rows: a count says how much has moved
    /// since the payload started, and only a time says whether any of it is
    /// moving now -- a payload stopped ten minutes ago and one sending every
    /// second look identical by their counts. The bytes beside it are what
    /// say the traffic is the traffic that was expected rather than merely
    /// traffic.
    #[test]
    fn a_panel_shows_when_the_last_packet_went_and_what_it_was() {
        let mut info =
            payload_info_from(0, &handler("DH0"), &sim(1000, 1000, 12));
        assert_eq!(info.last_sent_time, NO_TRANSFER_TIME);
        assert_eq!(info.last_recv_time, NO_TRANSFER_TIME);

        // Nothing has moved yet, so neither row shows a time or any bytes: a
        // count of nought with a time beside it would be a transfer that
        // never happened.
        show_traffic(&mut info, &PayloadStats::default(), None);
        assert_eq!(info.last_sent_time, NO_TRANSFER_TIME);
        assert_eq!(info.last_sent_data, "");
        assert_eq!(info.last_recv_time, NO_TRANSFER_TIME);
        assert_eq!(info.last_recv_data, "");

        // One each way, an hour apart, as the panel shows them.
        show_traffic(
            &mut info,
            &PayloadStats {
                packets_sent: 7,
                packets_recv: 2,
                last_sent: sample_of(3661, &[0x01, 0xAB, 0xFF]),
                last_recv: sample_of(7322, b"READ\r"),
                ..PayloadStats::default()
            },
            None,
        );
        assert_eq!(info.last_sent_time, "01:01:01");
        assert_eq!(info.last_sent_data, "01 AB FF");
        assert_eq!(info.last_recv_time, "02:02:02");
        assert_eq!(info.last_recv_data, "52 45 41 44 0D");
        assert_eq!((info.packets_sent, info.packets_recv), (7, 2));

        // A packet longer than the sample keeps shows a head and says so,
        // which is how a panel of eight bytes talks about a packet of twelve.
        show_traffic(
            &mut info,
            &PayloadStats {
                packets_sent: 8,
                last_sent: sample_of(86_399, &[0x11; 12]),
                ..PayloadStats::default()
            },
            None,
        );
        assert_eq!(info.last_sent_time, "23:59:59");
        assert_eq!(info.last_sent_data, "11 11 11 11 11 11 11 11...");

        // And one direction without the other: a payload that sends and is
        // never spoken to is the commonest kind there is.
        assert_eq!(
            (info.last_recv_time.as_str(), info.last_recv_data.as_str()),
            (NO_TRANSFER_TIME, ""),
            "a payload nobody has spoken to has no received line to show"
        );
    }

    /// Every panel, and each of the three rows of data it ends with, is
    /// inside the window tcs-sim opens for each shipped set.
    ///
    /// Where the chrome figures the markup states are checked. The window is
    /// opened here as main() opens it, through Slint's testing backend: no
    /// display and no event loop, but a real layout with real font metrics,
    /// so what this measures is what the window does. A panel scrolled out of
    /// sight is not merely off-screen, it is never built, so the count of
    /// panels found is part of the check.
    /// The markup reports the sizes the grid tests do their arithmetic on.
    ///
    /// Nothing in the program compares the two -- it asks the markup and
    /// believes it -- so this is the only thing standing between
    /// `Sizes::for_tests` and a set of numbers that were true once. A panel
    /// that grows a row is meant to fail here: the fixture is then wrong, and
    /// so is whatever a test concluded from it.
    ///
    /// Not a test of its own: the backend belongs to one thread, so the tests
    /// that need a window run from the one that starts it.
    fn the_markup_reports_the_sizes_the_tests_assume() {
        let ui = MainWindow::new().unwrap();
        let _ = grid::settle(&ui);
        assert_eq!(
            grid::Sizes::of(&ui),
            grid::Sizes::for_tests(),
            "the window reports sizes the tests were written against other \
             numbers for"
        );
    }

    /// The padding the panel's layout keeps inside its border, as
    /// `ui/main.slint` states it. The one figure this test needs from the
    /// markup that it cannot measure: it is the gap that is meant to be
    /// under the last row, as against the slack that is not.
    const PANEL_PADDING: f32 = 4.0;

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
            let sim_file = shipped_sim(&payload_path, &sim_path)
                .unwrap_or_else(|e| {
                    panic!("{} failed to load: {e}", sim_path.display())
                });
            let dh_configs = simulated_payloads_of(&payload_path, &sim_file);
            let sims = sim_file.resolve(&dh_configs).unwrap_or_else(|e| {
                panic!("{} does not fit: {e}", sim_path.display())
            });

            let panels: Vec<PayloadInfo> = dh_configs
                .iter()
                .zip(&sims)
                .enumerate()
                .map(|(row, (dh, sim))| payload_info_from(row, dh, sim))
                .collect();
            // An example that simulates nothing has no window to measure:
            // `simple-app`'s payload is a program, which tcspecial runs
            // rather than reaches, so no simulator plays it.
            if panels.is_empty() {
                continue;
            }
            cases.push((format!("{}", payload_path.display()), panels));
        }
        assert!(!cases.is_empty(), "no example simulates anything");
        let one = cases[0].1[0].clone();
        cases.push((
            "sixteen payloads".to_string(),
            std::iter::repeat_with(|| one.clone()).take(16).collect(),
        ));

        // A case of panels filled with the longest lines they can show was
        // tried here and taken out again: Slint clips a line too long for its
        // panel rather than widening anything, and the testing backend
        // reports a Text's laid-out width rather than the width it wanted, so
        // such a case measures exactly what an empty one does. What the rows
        // hold is checked by the tests of show_traffic instead, and how much
        // room they have by the heights below.

        // A line of the panels' type, as the markup measures it.
        let line_height = PanelProbe::new()
            .expect("a window to measure a line in")
            .get_line_height();

        for (set, rows) in cases {
            let panels = rows.len();

            let ui = MainWindow::new().unwrap();
            let sizes = grid::settle(&ui);
            ui.set_payload_model(ModelRc::from(Rc::new(VecModel::from(rows))));

            // Opened as main() opens it, floor and all.
            let shape = grid::grid_shape(&sizes, panels);
            ui.window()
                .set_size(grid::window_size(&sizes, &shape, panels));
            ui.show().unwrap();

            let window = ui.window().size().to_logical(1.0);

            // The grid fits the area it is given, across and down: it stood
            // wider and taller than that area while the padding it keeps
            // inside the scrolling area was left out of the window size.
            let area =
                ElementHandle::find_by_element_id(&ui, "MainWindow::scroll")
                    .next()
                    .expect("the window has a scrolling area for the panels");
            let grid =
                ElementHandle::find_by_element_id(&ui, "MainWindow::grid")
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
            let wanted =
                grid::height_for_rows(&sizes, rows) - sizes.chrome_height;
            assert!(
                area.size().height >= wanted,
                "{set}: the panels have {} of the {wanted} that {rows} rows \
                 need",
                area.size().height
            );
            assert!(
                area.size().height < wanted + sizes.panel_height,
                "{set}: the panels have {}, room for a row more than the \
                 {rows} wanted",
                area.size().height
            );

            // Every button is inside the window across, which is what
            // keeps a label from running off the side of its panel: a
            // Button reports the width it was given, where a Text that does
            // not fit is clipped and reports the width it had.
            for button in
                ElementHandle::find_by_element_type_name(&ui, "Button")
            {
                let right = button.absolute_position().x + button.size().width;
                assert!(
                    right <= window.width,
                    "{set}: the {:?} button reaches {right}, past the {} the \
                     window is wide",
                    button.accessible_label(),
                    window.width
                );
            }

            // Nothing in a panel stands on anything else, and nothing is
            // left over at the end of one.
            //
            // The three faults this answers were all in one panel at once.
            // The row of buttons was given a height of twenty-four where a
            // Button in this style is thirty-two, and a layout that cannot
            // shrink a child does not clip it: the buttons hung six pixels
            // below their row and drew across the "Sent:" line under it. The
            // panel was given two hundred and ten where its rows take a
            // hundred and seventy-six, and since every row states a height
            // the slack could go nowhere but the bottom, leaving a band of
            // nothing under the packet count. And the grid was centred in a
            // scrolling area taller than itself, so the spare row the window
            // keeps room for showed half above the panels and half below.
            //
            // Measured rather than reasoned about: each is a number this
            // backend reports, and each was found by reading them.
            let rows = [
                "PayloadPanel::title-row",
                "PayloadPanel::config-row",
                "PayloadPanel::controls-row",
                "PayloadPanel::sent-row",
                "PayloadPanel::recv-row",
                "PayloadPanel::packets-row",
            ];
            // The rows that are a line of text, every one of which is at
            // least a line of the window's type tall. They were told
            // fourteen and eighteen pixels in type of twelve points, and cut
            // off every descender and underscore: "temp_1" read "temp 1".
            let text_rows = [
                "PayloadPanel::title-row",
                "PayloadPanel::config-row",
                "PayloadPanel::sent-row",
                "PayloadPanel::recv-row",
                "PayloadPanel::packets-row",
            ];
            let geometry = |id: &str| -> Vec<(f32, f32)> {
                ElementHandle::find_by_element_id(&ui, id)
                    .map(|e| (e.absolute_position().y, e.size().height))
                    .collect()
            };

            // Gathered once and not once per panel: finding elements walks
            // the whole tree, and a set of sixteen panels did so sixteen
            // times over, which cost the suite a minute and a half.
            let placed: Vec<Vec<(f32, f32)>> =
                rows.iter().map(|id| geometry(id)).collect();
            for id in text_rows {
                let tall = geometry(id);
                assert_eq!(tall.len(), panels, "{set}: {id} not built");
                for (n, (_, height)) in tall.into_iter().enumerate() {
                    assert!(
                        height + 0.5 >= line_height,
                        "{set}: panel {n}'s {id} is {height} tall, which \
                         cuts off a line of text {line_height} tall"
                    );
                }
            }
            let sent_rows = geometry("PayloadPanel::sent-row");
            let buttons: Vec<(Option<String>, f32, f32)> =
                ElementHandle::find_by_element_type_name(&ui, "Button")
                    .map(|b| {
                        (
                            b.accessible_label().map(|l| l.to_string()),
                            b.absolute_position().y,
                            b.absolute_position().y + b.size().height,
                        )
                    })
                    .collect();
            let panels_found: Vec<(f32, f32)> =
                ElementHandle::find_by_element_type_name(&ui, "PayloadPanel")
                    .map(|e| (e.absolute_position().y, e.size().height))
                    .collect();

            // No row overlaps the one below it. The buttons are measured
            // rather than their row, because it was the buttons that
            // overflowed while the row reported the height it was told.
            for (n, (top, panel_height)) in
                panels_found.iter().copied().enumerate()
            {
                let bottom = top + panel_height;
                let mut lowest = top;
                for (which, id) in rows.iter().enumerate() {
                    let (y, height) = placed[which][n];
                    assert!(
                        y >= lowest,
                        "{set}: panel {n}'s {id} starts at {y}, above the \
                         {lowest} the row before it reaches"
                    );
                    lowest = y + height;
                }

                let (sent, _) = sent_rows[n];
                for (label, head, foot) in buttons.iter() {
                    if *head >= top && *foot <= bottom {
                        assert!(
                            *foot <= sent,
                            "{set}: panel {n}'s {label:?} button \
                             reaches {foot}, over \
                             the \"Sent:\" row at {sent}"
                        );
                    }
                }

                // And the last row ends where the panel does, but for the
                // padding it keeps: a panel taller than its rows shows the
                // difference as a gap under the packet count.
                let (y, height) = placed[rows.len() - 1][n];
                let spare = bottom - (y + height);
                assert!(
                    spare <= PANEL_PADDING + 1.0,
                    "{set}: panel {n} has {spare} pixels below its packet \
                     count, where {PANEL_PADDING} of padding is all it keeps"
                );
            }

            // The grid reflows when the window is resized.
            //
            // The column count is the window's own, worked out from the width
            // it has rather than handed over once at startup: a window
            // dragged wider is meant to fit another column and use it, up to
            // three in a row. This drags it and counts the columns by how
            // many panels share the top row.
            //
            // Done last in the case, because it leaves the window a different
            // size than it was opened at.
            let columns_now = |ui: &MainWindow| -> usize {
                let tops: Vec<f32> = ElementHandle::find_by_element_type_name(
                    ui,
                    "PayloadPanel",
                )
                .map(|e| e.absolute_position().y)
                .collect();
                let first = tops.iter().copied().fold(f32::MAX, f32::min);
                tops.iter().filter(|y| (**y - first).abs() < 1.0).count()
            };

            let opened = columns_now(&ui);
            assert_eq!(
                opened,
                grid::columns_for(panels),
                "{set}: opened with {opened} columns where {} were asked for",
                grid::columns_for(panels)
            );

            if panels >= 2 {
                // One panel's width narrower than two panels need, so the
                // count has to come down.
                let narrow = sizes.panel_width + sizes.chrome_width;
                ui.window()
                    .set_size(slint::LogicalSize::new(narrow, window.height));
                assert_eq!(
                    columns_now(&ui),
                    1,
                    "{set}: a window one panel wide did not come down to one \
                     column"
                );

                // And wide enough for every panel in a row, so it goes up --
                // to three at most, however wide.
                let wide =
                    panels as f32 * sizes.panel_width + sizes.chrome_width;
                ui.window()
                    .set_size(slint::LogicalSize::new(wide, window.height));
                let most = panels.min(grid::MAX_COLUMNS);
                assert_eq!(
                    columns_now(&ui),
                    most,
                    "{set}: a window {panels} panels wide did not use the \
                     room, up to {most} in a row"
                );
            }

            // The grid starts where the area it scrolls in starts, so the
            // room the window keeps for a row not yet filled is all below the
            // panels rather than half of it above them.
            assert!(
                grid.absolute_position().y <= area.absolute_position().y + 1.0,
                "{set}: the grid starts at {}, {} below the {} the scrolling \
                 area starts at",
                grid.absolute_position().y,
                grid.absolute_position().y - area.absolute_position().y,
                area.absolute_position().y
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
                    ElementHandle::find_by_element_type_name(&ui, what)
                        .collect()
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
                let area_bottom =
                    area.absolute_position().y + area.size().height;
                let area_right = area.absolute_position().x + area.size().width;
                for (n, element) in found.iter().enumerate() {
                    let bottom =
                        element.absolute_position().y + element.size().height;
                    let right =
                        element.absolute_position().x + element.size().width;
                    assert!(
                        bottom <= area_bottom && right <= area_right,
                        "{set}: {what} {n} reaches ({right},{bottom}), past \
                         the ({area_right},{area_bottom}) the panels have in a \
                         window {}x{}",
                        window.width,
                        window.height
                    );
                }
            }
        }

        // The backend is up and this thread owns its windows, so the other
        // checks that need one run from here.
        the_markup_reports_the_sizes_the_tests_assume();
        the_panel_shows_the_time_and_bytes_it_was_given();
        each_panel_says_which_payload_of_the_set_it_is();
        the_params_button_shows_what_both_files_said();
    }

    /// A shipped set's simulator file, read as `load` reads it with that
    /// configuration alone: a file shared by several configurations without
    /// the others' payloads.
    ///
    /// # Arguments
    ///
    /// * `configuration` - The set's tcspecial configuration file.
    /// * `sim_path`      - Its simulator file.
    ///
    /// # Returns
    ///
    /// The file, with the payloads of this configuration alone.
    ///
    /// # Errors
    ///
    /// Why the file cannot be read.
    fn shipped_sim(
        configuration: &std::path::Path,
        sim_path: &std::path::Path,
    ) -> Result<SimConfigFile, String> {
        let file = SimConfigFile::load(sim_path).map_err(|e| e.to_string())?;
        Ok(file.for_configuration(
            &tcsprotocol::sim_config::configuration_name(configuration),
        ))
    }

    /// Makes the payloads of a configuration that its simulator file names,
    /// as `load` makes them.
    ///
    /// Through `tcsprotocol::graph::simulated`, which is what turns a
    /// payload node into what the simulator simulates: where it is reached, how
    /// big a packet is, and whether it answers a trigger.
    ///
    /// # Arguments
    ///
    /// * `configuration` - The tcspecial configuration file.
    /// * `sim_file`      - Its simulator file, as read.
    ///
    /// # Returns
    ///
    /// A data handler for each payload the simulator simulates.
    ///
    /// # Panics
    ///
    /// If the configuration cannot be read or simulated.
    fn simulated_payloads_of(
        configuration: &std::path::Path,
        sim_file: &SimConfigFile,
    ) -> Vec<DHConfig> {
        let graph = tcsprotocol::graph::Graph::load(configuration)
            .unwrap_or_else(|e| {
                panic!("{} failed to load: {e}", configuration.display())
            });
        tcsprotocol::graph::simulated::handlers(&graph, sim_file)
            .unwrap_or_else(|e| {
                panic!("{} cannot be simulated: {e}", configuration.display())
            })
    }

    /// Every shipped set's simulator file names every payload that is a
    /// simulated or dummy device, so that tcs-sim controls and shows each of
    /// them. Only that is
    /// asked here: whether a device path is real hardware depends on the host
    /// the test runs on.
    #[test]
    fn every_shipped_simulated_payload_is_named() {
        for (configuration, sim_path) in shipped_sets() {
            let graph = tcsprotocol::graph::Graph::load(&configuration)
                .expect("it reads");
            let sim_file =
                shipped_sim(&configuration, &sim_path).expect("it reads");
            let refused: Vec<String> = simulation_refused(
                &graph,
                &sim_file,
                &sim_path.display().to_string(),
                Mode::All,
            )
            .into_iter()
            .filter(|refusal| refusal.contains("does not name it"))
            .collect();
            assert!(
                refused.is_empty(),
                "{}: {refused:?}",
                configuration.display()
            );
        }
    }

    /// Finds every shipped configuration and the simulator file beside it.
    ///
    /// The examples, found rather than listed, so an example added to the
    /// repository is covered by these tests without anyone remembering to
    /// name it here.
    ///
    /// # Returns
    ///
    /// Each example's `<name>.ytcs` and `<name>-sim.yaml`, sorted.
    ///
    /// # Panics
    ///
    /// If the examples cannot be read, one has no simulator file, or there
    /// are none.
    fn shipped_sets() -> Vec<(std::path::PathBuf, std::path::PathBuf)> {
        let root = repo_file("examples");
        let mut sets = Vec::new();

        for entry in
            tcsos::fs::read_dir(&root).expect("the examples are readable")
        {
            let path = entry.expect("a readable directory entry").path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !tcsos::fs::is_dir(&path) {
                continue;
            }

            let configuration = path.join(format!("{name}.ytcs"));
            let sim_path = path.join(format!("{name}-sim.yaml"));
            if !tcsos::fs::is_file(&configuration) {
                continue;
            }
            assert!(
                tcsos::fs::path_exists(&sim_path),
                "{} has no {} beside it, so it cannot be simulated",
                configuration.display(),
                sim_path.display()
            );
            sets.push((configuration, sim_path));
        }

        sets.sort();
        assert!(
            !sets.is_empty(),
            "no configurations found in {}",
            root.display()
        );
        sets
    }

    /// Makes a data handler, for the tests that only need one to exist.
    ///
    /// # Arguments
    ///
    /// * `name` - Its name.
    ///
    /// # Returns
    ///
    /// A UDP handler to localhost:5000 with twelve-byte packets.
    fn handler(name: &str) -> DHConfig {
        DHConfig {
            name: DHName::new(name),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: "localhost".to_string(),
                port: 5000,
            }),
            packet_size: 12,
            oc: None,
            ground: None,
            mode: Default::default(),
            stale_interval: None,
            sends_from: None,
            sim_address: None,
            driven: false,
        }
    }

    /// Makes simulator settings attached to nothing, with no faults.
    ///
    /// # Arguments
    ///
    /// * `packet_interval_ms`  - The time between packets, in ms.
    /// * `segment_interval_ms` - The time between segments, in ms.
    /// * `segment_size`        - The bytes in a segment.
    ///
    /// # Returns
    ///
    /// The settings.
    fn sim(
        packet_interval_ms: u32,
        segment_interval_ms: u32,
        segment_size: u32,
    ) -> ResolvedSim {
        ResolvedSim {
            attached: None,
            packet_interval: Duration::from_millis(u64::from(
                packet_interval_ms,
            )),
            segment_interval: Duration::from_millis(u64::from(
                segment_interval_ms,
            )),
            segment_size,
            triggered: false,
            faults: Default::default(),
            payload_address: None,
            payload_port: None,
            enabled: false,
        }
    }

    // Nothing here checks that no shipped simulator file fits another
    // configuration, because some do: `minimal` and `beacon` both have a sensor
    // on a UDP socket, and `minimal`'s simulator file fits either, which is a
    // true thing about two examples rather than a fault. The rule that matters
    // is that a simulated payload states the kind of payload it simulates, and
    // that is tested where the rule is: `tcsprotocol::sim_config`'s
    // `the_two_files_cannot_disagree_about_the_kind`.

    /// Every shipped set has to describe payloads the simulator can simulate,
    /// and its two files have to agree with each other.
    ///
    /// Nothing else checks that: the GUI only reveals files that disagree when
    /// someone runs it and finds the simulator refusing to start.
    #[test]
    fn the_shipped_files_become_simulator_configs() {
        for (payload_path, sim_path) in shipped_sets() {
            let sim_file = shipped_sim(&payload_path, &sim_path)
                .unwrap_or_else(|e| {
                    panic!("{} failed to load: {e}", sim_path.display())
                });
            let dh_configs = simulated_payloads_of(&payload_path, &sim_file);

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
                .map(|(dh, s)| payload_config_attached(dh, s, None))
                .collect::<Result<_, _>>()
                .unwrap_or_else(|e| {
                    panic!("{} is not simulatable: {e}", payload_path.display())
                });

            // An example whose payloads are all programs simulates nothing,
            // which is a pair tcs-sim is right to accept: see `simple-app`,
            // whose payload tcspecial runs rather than reaches.
            if configs.is_empty() {
                continue;
            }
            assert!(
                !configs.is_empty(),
                "{} has no payloads to simulate",
                payload_path.display()
            );
        }
    }

    /// Asked to simulate the thermostat example's thermostat node, tcs-sim
    /// reads the three sensors and switches the two heaters, and only the
    /// sensors report to it.
    #[test]
    fn tcs_sim_simulates_the_examples_thermostat() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let payload_path = root.join("examples/thermostat/thermostat.ytcs");
        let mut sim_file = SimConfigFile::load(
            root.join("examples/thermostat/thermostat-sim.yaml"),
        )
        .expect("the simulator file reads");
        simulate_for(&mut sim_file, &["thermostat"]);
        let graph =
            tcsprotocol::graph::Graph::load(&payload_path).expect("reads");
        let dh_configs = simulated_payloads_of(&payload_path, &sim_file);
        let sims = sim_file.resolve(&dh_configs).expect("it fits");

        let (attached, thermostats) =
            joined_to_the_world(&sim_file, &[&graph], &sims, &dh_configs)
                .expect("the world and the thermostat are made");
        let named: Vec<&str> = thermostats.iter().map(|t| t.name()).collect();
        assert_eq!(named, ["thermostat"]);
        let thermostat = &thermostats[0];
        for sensor in ["temp_1", "temp_2", "temp_3"] {
            assert!(thermostat.reads(sensor), "{sensor}");
        }
        assert!(!thermostat.reads("gpio-out-tmass1"));
        let heaters: Vec<&str> = thermostat.heaters().collect();
        assert_eq!(heaters, ["gpio-out-tmass1", "gpio-out-tmass2"]);
        // The three sensors and the two heaters' switches.
        assert_eq!(attached.len(), 5);

        // Naming a node that is not a thermostat node is refused.
        simulate_for(&mut sim_file, &["thermostat", "temp1"]);
        assert!(
            joined_to_the_world(&sim_file, &[&graph], &sims, &dh_configs)
                .is_err()
        );
    }

    /// The thermostat example's two heater switches each say which heater
    /// they are, and say it in the words the design gives: heating, off, or
    /// disabled, so off.
    ///
    /// Both are joined straight to the zone's two heater inputs, and their
    /// panels named the zone alone -- "zone: on" over "zone: on" -- so that
    /// the two could not be told apart, and said "on" where a heater is
    /// heating.
    #[test]
    fn the_examples_heaters_say_which_they_are_and_what_they_do() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let payload_path = root.join("examples/thermostat/thermostat.ytcs");
        let sim_file = SimConfigFile::load(
            root.join("examples/thermostat/thermostat-sim.yaml"),
        )
        .expect("the simulator file reads");
        let graph =
            tcsprotocol::graph::Graph::load(&payload_path).expect("reads");
        let dh_configs = simulated_payloads_of(&payload_path, &sim_file);
        let sims = sim_file.resolve(&dh_configs).expect("it fits");
        let (attached, _) =
            joined_to_the_world(&sim_file, &[&graph], &sims, &dh_configs)
                .expect("the world is made");

        for (payload, heater) in [
            ("gpio-out-tmass1", "zone.heater_1"),
            ("gpio-out-tmass2", "zone.heater_2"),
        ] {
            let switch = &attached[payload];
            assert_eq!(
                switch.driving_said(),
                format!("{heater}: disabled, so off"),
                "{payload}"
            );
            switch.enable_at(true, 1.0);
            assert_eq!(switch.driving_said(), format!("{heater}: off"));
            switch.sent_at(1.0, 2.0);
            assert_eq!(switch.driving_said(), format!("{heater}: heating"));
        }
    }

    /// Has a simulator file's tcs-sim section name the thermostats tcs-sim
    /// simulates.
    ///
    /// # Arguments
    ///
    /// * `sim_file` - The simulator file, which has a tcs-sim section.
    /// * `names`    - The thermostat nodes, in order.
    fn simulate_for(sim_file: &mut SimConfigFile, names: &[&str]) {
        sim_file
            .tcs_sim
            .as_mut()
            .expect("a tcs-sim section")
            .thermostats = names.iter().map(|n| n.to_string()).collect();
    }

    /// Gives the thermostat example's graph a second thermostat node,
    /// `backstop`, which reads the vote as `thermostat` does and takes
    /// `gpio-out-tmass2` from it: two thermostats, each with a heater of its own.
    ///
    /// # Arguments
    ///
    /// * `graph` - The thermostat example's graph.
    fn with_a_backstop(graph: &mut tcsprotocol::graph::Graph) {
        let mut node = graph
            .nodes
            .iter()
            .find(|node| node.name == "thermostat")
            .cloned()
            .expect("the example has a thermostat");
        node.name = "backstop".to_string();
        graph.nodes.push(node);
        let mut added = Vec::new();
        for link in &mut graph.links {
            if link.to.each().contains(&"thermostat") {
                let mut also = link.clone();
                also.to = tcsprotocol::graph::Destinations::of(vec![
                    "backstop".to_string(),
                ]);
                added.push(also);
            }
            if link.origin().node == "thermostat"
                && link.to.each().contains(&"gpio-out-tmass2")
            {
                let mut taken = link.clone();
                taken.from = link.from.replacen("thermostat", "backstop", 1);
                taken.to = tcsprotocol::graph::Destinations::of(vec![
                    "gpio-out-tmass2".to_string(),
                ]);
                added.push(taken);
                link.to = tcsprotocol::graph::Destinations::of(
                    link.to
                        .each()
                        .into_iter()
                        .filter(|to| *to != "gpio-out-tmass2")
                        .map(str::to_string)
                        .collect(),
                );
            }
        }
        graph.links.retain(|link| !link.to.each().is_empty());
        graph.links.extend(added);
    }

    /// Several thermostats are simulated together, each switching the
    /// heaters it drives and reading the sensors that feed it.
    #[test]
    fn tcs_sim_simulates_several_thermostats() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let payload_path = root.join("examples/thermostat/thermostat.ytcs");
        let mut sim_file = SimConfigFile::load(
            root.join("examples/thermostat/thermostat-sim.yaml"),
        )
        .expect("the simulator file reads");
        simulate_for(&mut sim_file, &["thermostat", "backstop"]);
        let mut graph =
            tcsprotocol::graph::Graph::load(&payload_path).expect("reads");
        with_a_backstop(&mut graph);
        let dh_configs = simulated_payloads_of(&payload_path, &sim_file);
        let sims = sim_file.resolve(&dh_configs).expect("it fits");

        let (_, thermostats) =
            joined_to_the_world(&sim_file, &[&graph], &sims, &dh_configs)
                .expect("the world and the thermostats are made");
        let named: Vec<&str> = thermostats.iter().map(|t| t.name()).collect();
        assert_eq!(named, ["thermostat", "backstop"]);
        for (thermostat, heater) in thermostats
            .iter()
            .zip(["gpio-out-tmass1", "gpio-out-tmass2"])
        {
            for sensor in ["temp_1", "temp_2", "temp_3"] {
                assert!(thermostat.reads(sensor), "{sensor}");
            }
            let heaters: Vec<&str> = thermostat.heaters().collect();
            assert_eq!(heaters, [heater]);
        }
    }

    /// Two thermostats switching one heater are refused, naming both.
    #[test]
    fn two_thermostats_switching_one_heater_are_refused() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let payload_path = root.join("examples/thermostat/thermostat.ytcs");
        let mut sim_file = SimConfigFile::load(
            root.join("examples/thermostat/thermostat-sim.yaml"),
        )
        .expect("the simulator file reads");
        simulate_for(&mut sim_file, &["thermostat", "backstop"]);
        let mut graph =
            tcsprotocol::graph::Graph::load(&payload_path).expect("reads");
        with_a_backstop(&mut graph);
        // The backstop driving the first heater too.
        let mut also = graph
            .links
            .iter()
            .find(|link| {
                link.origin().node == "backstop"
                    && link.to.each().contains(&"gpio-out-tmass2")
            })
            .cloned()
            .expect("the backstop drives gpio-out-tmass2");
        also.to =
            tcsprotocol::graph::Destinations::of(
                vec!["gpio-out-tmass1".into()],
            );
        graph.links.push(also);
        let dh_configs = simulated_payloads_of(&payload_path, &sim_file);
        let sims = sim_file.resolve(&dh_configs).expect("it fits");

        let refused =
            joined_to_the_world(&sim_file, &[&graph], &sims, &dh_configs)
                .expect_err("refused");
        assert!(
            refused.contains("thermostat")
                && refused.contains("backstop")
                && refused.contains("gpio-out-tmass1"),
            "{refused}"
        );
    }

    /// What a run of the thermostat example's physics did: see
    /// [`run_the_thermostat_example`].
    #[derive(Debug, Default)]
    struct ThermostatRun {
        /// Simulated seconds watched, after the first climb to the band.
        watched: f64,
        /// Of those, the seconds the thermostat had the heaters on.
        decided_on: f64,
        /// How many times it switched them on.
        came_on: usize,
        /// The seconds each heater gave out heat.
        heating: [f64; 2],
        /// The seconds one enabled heater was heating and another enabled
        /// one was not: never, if they switch together.
        apart: f64,
        /// The seconds the zone was inside the band, the setpoint plus or
        /// minus a half band.
        inside: f64,
        /// The seconds it was below the setpoint less two half bands.
        below: f64,
        /// The seconds it was above the setpoint plus two half bands.
        above: f64,
        /// The coldest and the warmest the zone was.
        coldest: f64,
        warmest: f64,
    }

    /// Runs the thermostat example's physics with no window and no vehicle:
    /// its world, its three sensors with their errors, the median of their
    /// readings, and its thermostat deciding on it once a second,
    /// as the payload group is read, sending each change to both heaters.
    /// A heater disabled, as its panel would have it, gets the decisions
    /// and stays off, as it does in tcs-sim.
    ///
    /// # Arguments
    ///
    /// * `seed`    - What the world's random draws start from.
    /// * `seconds` - How long to run, in simulated seconds.
    /// * `enabled` - Whether `gpio-out-tmass1` and `gpio-out-tmass2` are enabled.
    ///
    /// # Returns
    ///
    /// What happened, counted from the first time the zone reached the
    /// band, where it does; the climb from cold is no part of holding it.
    /// With no heater enabled it never climbs, and all of it is counted.
    fn run_the_thermostat_example(
        seed: u64,
        seconds: f64,
        enabled: [bool; 2],
    ) -> ThermostatRun {
        use tcsprotocol::graph::simulated::thermostat_of;

        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let payload_path = root.join("examples/thermostat/thermostat.ytcs");
        let mut sim_file = SimConfigFile::load(
            root.join("examples/thermostat/thermostat-sim.yaml"),
        )
        .expect("the simulator file reads");
        sim_file.tcs_sim.as_mut().expect("a tcs-sim section").seed = Some(seed);
        let graph =
            tcsprotocol::graph::Graph::load(&payload_path).expect("reads");
        let dh_configs = simulated_payloads_of(&payload_path, &sim_file);
        let sims = sim_file.resolve(&dh_configs).expect("it fits");
        let (attached, _) =
            joined_to_the_world(&sim_file, &[&graph], &sims, &dh_configs)
                .expect("the world is made");

        let thermostat = thermostat_of(&graph, "thermostat").expect("one");
        let (setpoint, half_band) = (thermostat.setpoint, thermostat.half_band);
        let mut zone = None;
        let mut sensors = Vec::new();
        let mut heaters = Vec::new();
        for (payload, attachment) in &attached {
            match attachment {
                Attachment::Reports {
                    world,
                    place,
                    offset,
                    sensor,
                    ..
                } => {
                    zone = Some((world.clone(), *place));
                    sensors.push((*place, *offset, sensor.clone()));
                }
                Attachment::Drives { place, .. } => {
                    heaters.push((payload.clone(), attachment.clone(), *place));
                }
                Attachment::Signals { .. }
                | Attachment::Streams { .. }
                | Attachment::Pulls { .. } => {}
            }
        }
        let (world, zone) = zone.expect("a sensor of the zone");
        assert_eq!(sensors.len(), 3);
        let named: Vec<&str> = heaters.iter().map(|h| h.0.as_str()).collect();
        assert_eq!(named, ["gpio-out-tmass1", "gpio-out-tmass2"]);
        for (n, (_, heater, place)) in heaters.iter().enumerate() {
            assert_eq!(world.name_of(*place), format!("zone.heater_{}", n + 1));
            heater.enable_at(enabled[n], 0.0);
        }

        let step = 1.0;
        let mut decided = false;
        let mut run = ThermostatRun {
            coldest: f64::INFINITY,
            warmest: f64::NEG_INFINITY,
            ..ThermostatRun::default()
        };
        let mut holding = !enabled.contains(&true);
        let mut now = 0.0;
        while now < seconds {
            now += step;
            let mut readings: Vec<f64> = sensors
                .iter()
                .map(|(place, offset, sensor)| {
                    sensor.read(world.read_at(*place, now) + offset, now)
                })
                .collect();
            readings.sort_by(f64::total_cmp);
            let voted = readings[1];
            let was = decided;
            if voted > setpoint + half_band {
                decided = false;
            } else if voted < setpoint - half_band {
                decided = true;
            }
            // Sent to both heaters, enabled or not, as the vehicle does.
            if decided != was || now <= step {
                for (_, heater, _) in &heaters {
                    heater.sent_at(if decided { 1.0 } else { 0.0 }, now);
                }
            }

            let temperature = world.read_at(zone, now);
            holding |= temperature > setpoint - half_band;
            if !holding {
                continue;
            }
            run.watched += step;
            if decided {
                run.decided_on += step;
                run.came_on += usize::from(!was);
            }
            // A heater of the zone's own heats while its input is held on.
            let heating: Vec<bool> = heaters
                .iter()
                .map(|(_, heater, _)| {
                    matches!(
                        heater.driving(),
                        Some(node::physics::Driving::At(on)) if on > 0.5
                    )
                })
                .collect();
            for (n, on) in heating.iter().enumerate() {
                if *on {
                    run.heating[n] += step;
                }
            }
            let enabled_heating: Vec<bool> = heating
                .iter()
                .zip(enabled)
                .filter(|(_, enabled)| *enabled)
                .map(|(on, _)| *on)
                .collect();
            if enabled_heating.iter().any(|on| *on)
                && !enabled_heating.iter().all(|on| *on)
            {
                run.apart += step;
            }
            let off = temperature - setpoint;
            if off.abs() <= half_band {
                run.inside += step;
            }
            if off < -2.0 * half_band {
                run.below += step;
            }
            if off > 2.0 * half_band {
                run.above += step;
            }
            run.coldest = run.coldest.min(temperature);
            run.warmest = run.warmest.max(temperature);
        }
        run
    }

    /// The thermostat example's zone over three orbits of its surroundings'
    /// swing, from three seeds, with both heaters enabled: the thermostat
    /// cycles them, they always switch together, and the zone stays within
    /// two half bands of the setpoint nearly all the time.
    #[test]
    fn the_thermostat_example_holds_the_zone_with_both_heaters() {
        for seed in [1, 2, 3] {
            let run = run_the_thermostat_example(seed, 3.0 * 5400.0, [true; 2]);
            eprintln!("both, seed {seed}: {run:?}");
            let share = |seconds: f64| seconds / run.watched;
            assert_eq!(run.apart, 0.0, "{run:?}");
            assert!(run.came_on >= 30, "it cycles them: {run:?}");
            assert!(
                (run.heating[0] - run.decided_on).abs() < 1.0
                    && (run.heating[1] - run.decided_on).abs() < 1.0,
                "{run:?}"
            );
            assert!(share(run.below + run.above) < 0.01, "{run:?}");
            assert!(share(run.inside) > 0.7, "{run:?}");
        }
    }

    /// With one heater disabled, the other holds the zone in the band alone,
    /// on for about twice as long, and the disabled heater gets every
    /// decision and never heats. With both disabled, the zone falls from
    /// the band to its surroundings, just inside the plot's lower red.
    #[test]
    fn the_thermostat_example_holds_the_zone_with_a_heater_disabled() {
        for seed in [1, 2, 3] {
            let run =
                run_the_thermostat_example(seed, 3.0 * 5400.0, [true, false]);
            eprintln!("one, seed {seed}: {run:?}");
            let share = |seconds: f64| seconds / run.watched;
            assert_eq!(run.heating[1], 0.0, "disabled, it never heats");
            assert!(run.came_on >= 30, "it cycles the other: {run:?}");
            assert!(share(run.below + run.above) < 0.01, "{run:?}");
            assert!(share(run.inside) > 0.7, "{run:?}");
            assert!(share(run.heating[0]) > 0.4, "{run:?}");
        }

        let run = run_the_thermostat_example(1, 5400.0, [false; 2]);
        eprintln!("neither: {run:?}");
        assert_eq!(run.heating, [0.0; 2]);
        assert!(run.coldest > 286.0 && run.coldest < 289.15, "{run:?}");
        assert!(run.warmest < 296.0, "{run:?}");
    }

    /// A shipped tcspecial configuration states a packet interval only where a
    /// payload is triggered, and no other simulation setting at all.
    ///
    /// The interval a tcspecial configuration may state is the rate a trigger
    /// goes out at, which is tcspecial's doing and so flight behaviour.
    /// Everything else about timing and segmentation is the simulator's, and a
    /// line of it that crept into the tcspecial configuration would be read by
    /// nothing: the loader refuses an interval on a periodic payload, but a
    /// segment size there is an unknown attribute, and this says plainly which
    /// file the settings belong in.
    #[test]
    fn a_shipped_payload_file_states_only_a_triggered_payload_s_interval() {
        for (payload_path, sim_path) in shipped_sets() {
            let text = tcsos::fs::read_to_string(&payload_path).unwrap();
            // Whether the payload whose attributes are being read is one that
            // answers requests. Each payload starts with its name.
            let mut triggered = false;
            // Whether the line being read is in the tcspecial section, which
            // has a segment size of its own -- the telemetry log's, under
            // `log` -- and is not about a payload at all.
            let mut in_tcspecial = false;

            for (number, line) in text.lines().enumerate() {
                if !line.starts_with(char::is_whitespace) && line.contains(':')
                {
                    in_tcspecial = line.starts_with("tcspecial:");
                }
                let line = line.trim().trim_start_matches("- ");
                if in_tcspecial {
                    continue;
                }
                if line.starts_with('#') {
                    continue;
                }
                if line.starts_with("name") {
                    triggered = false;
                }
                if line.starts_with("mode") && line.contains("triggered") {
                    triggered = true;
                }

                for setting in ["segment_interval_ms", "segment_size"] {
                    assert!(
                        !line.starts_with(setting),
                        "{}:{} states {}, which belongs in {}",
                        payload_path.display(),
                        number + 1,
                        setting,
                        sim_path.display()
                    );
                }

                assert!(
                    !line.starts_with("packet_interval_ms") || triggered,
                    "{}:{} states packet_interval_ms for a payload that sends \
                     on its own, whose rate belongs in {}",
                    payload_path.display(),
                    number + 1,
                    sim_path.display()
                );
            }
        }
    }

    /// Every payload of every shipped set gets a panel, and they fill a grid
    /// with room for all of them.
    #[test]
    fn the_shipped_files_give_a_panel_each() {
        for (payload_path, sim_path) in shipped_sets() {
            let sim_file = shipped_sim(&payload_path, &sim_path).unwrap();
            let dh_configs = simulated_payloads_of(&payload_path, &sim_file);
            let sims = sim_file.resolve(&dh_configs).unwrap();

            let panels: Vec<PayloadInfo> = dh_configs
                .iter()
                .zip(&sims)
                .enumerate()
                .map(|(row, (dh, s))| payload_info_from(row, dh, s))
                .collect();

            assert_eq!(
                panels.len(),
                dh_configs.len(),
                "{}",
                payload_path.display()
            );

            // The shape rule and not the sizes, so there is no window to
            // ask: a grid that holds its panels does so whatever a panel
            // comes to.
            let shape =
                grid::grid_shape(&grid::Sizes::for_tests(), panels.len());
            assert!(
                shape.columns * shape.rows >= panels.len(),
                "{} does not fit its grid",
                payload_path.display()
            );
        }
    }

    /// The panel on screen shows the time and bytes Rust put in the model.
    ///
    /// The rest of the chain is checked a piece at a time -- the payload
    /// records a sample, the refresh puts it in the row -- and none of that
    /// shows anything if the window does not read the field it was put in.
    /// A panel declares a property per value and the grid passes each one
    /// across by name, so a field the grid forgets is a panel that shows its
    /// own default for ever, with every test of the Rust side still passing.
    /// This reads the text the window actually laid out.
    ///
    /// Not a test of its own: the testing backend is started once per test
    /// binary and its windows belong to the thread that made them, so the
    /// checks that need a window are all reached from the one test below.
    fn the_panel_shows_the_time_and_bytes_it_was_given() {
        use i_slint_backend_testing::ElementHandle;

        /// Finds every line of a panel that begins with some text, as laid
        /// out.
        ///
        /// # Arguments
        ///
        /// * `ui`    - The window, shown.
        /// * `start` - What the lines begin with.
        ///
        /// # Returns
        ///
        /// The lines' text, in the order they were found.
        fn lines_from(ui: &MainWindow, start: &str) -> Vec<String> {
            ElementHandle::find_by_element_type_name(ui, "Text")
                .filter_map(|e| e.accessible_label())
                .map(|label| label.to_string())
                .filter(|label| label.starts_with(start))
                .collect()
        }

        let shown = |info: PayloadInfo| {
            let ui = MainWindow::new().unwrap();
            let _ = grid::settle(&ui);
            ui.set_payload_model(ModelRc::from(Rc::new(VecModel::from(vec![
                info,
            ]))));
            ui.show().unwrap();
            (
                lines_from(&ui, "Sent: "),
                lines_from(&ui, "Recv: "),
                lines_from(&ui, "Pkts: "),
            )
        };

        // A payload that sends on its own and has moved nothing: it says so
        // in the direction it can move in, and has no received line at all --
        // nothing is ever sent to such a payload, so a line for it would read
        // the same from the first packet to the last.
        let quiet = payload_info_from(0, &handler("DH0"), &sim(1000, 1000, 12));
        let (sent, recv, pkts) = shown(quiet.clone());
        assert_eq!(sent, vec![format!("Sent: {NO_TRANSFER_TIME} ")]);
        assert!(
            recv.is_empty(),
            "a payload that sends on its own has a received line: {recv:?}"
        );
        // And it counts only what it sends: a received count would be a
        // nought that nothing can change.
        assert_eq!(pkts, vec!["Pkts: 0 sent".to_string()]);

        // One that answers requests has both, and says the same thing in
        // each before anything has moved.
        let mut asked = sim(1000, 1000, 12);
        asked.triggered = true;
        asked.packet_interval = Duration::ZERO;
        asked.segment_interval = Duration::ZERO;
        let (sent, recv, pkts) =
            shown(payload_info_from(0, &handler("DH0"), &asked));
        assert_eq!(sent, vec![format!("Sent: {NO_TRANSFER_TIME} ")]);
        assert_eq!(recv, vec![format!("Recv: {NO_TRANSFER_TIME} ")]);
        assert_eq!(pkts, vec!["Pkts: 0 sent / 0 recv".to_string()]);

        // And one that has sent and been spoken to shows when each happened
        // and the head of what moved.
        let mut moving = payload_info_from(0, &handler("DH0"), &asked);
        show_traffic(
            &mut moving,
            &PayloadStats {
                packets_sent: 3,
                packets_recv: 1,
                last_sent: sample_of(3661, &[0xDE, 0xAD]),
                last_recv: sample_of(7322, b"ASK"),
                ..PayloadStats::default()
            },
            None,
        );
        let (sent, recv, pkts) = shown(moving);
        assert_eq!(
            sent,
            vec!["Sent: 01:01:01 DE AD".to_string()],
            "the panel does not show the time and bytes of the last packet sent"
        );
        assert_eq!(
            recv,
            vec!["Recv: 02:02:02 41 53 4B".to_string()],
            "the panel does not show the time and bytes of the last packet \
             received"
        );
        assert_eq!(pkts, vec!["Pkts: 3 sent / 1 recv".to_string()]);

        // A heater sends nothing, and its first row says in words what it
        // is doing instead.
        for doing in ["heating", "off", "disabled, so off"] {
            let mut heater = payload_info_from(0, &handler("DH0"), &asked);
            heater.driving = SharedString::from(format!("heater_1: {doing}"));
            let ui = MainWindow::new().unwrap();
            let _ = grid::settle(&ui);
            ui.set_payload_model(ModelRc::from(Rc::new(VecModel::from(vec![
                heater,
            ]))));
            ui.show().unwrap();
            assert_eq!(
                lines_from(&ui, "heater_1: "),
                vec![format!("heater_1: {doing}")]
            );
            assert!(lines_from(&ui, "Sent: ").is_empty(), "{doing}");
        }
    }

    /// Each panel says which payload of the configuration it is, beside the
    /// name.
    ///
    /// The number is the payload's place in the file, counted from nought,
    /// and nothing in the file states it: the order is what decides it. Both
    /// windows count the same way, so a payload is the same number in each,
    /// which is the whole use of it -- two windows read side by side.
    ///
    /// Not a test of its own: one test per binary may start the backend.
    fn each_panel_says_which_payload_of_the_set_it_is() {
        use i_slint_backend_testing::ElementHandle;

        let first = handler("DH0");
        let mut second = handler("DH0");
        second.name = tcsprotocol::DHName::new("DH1");
        let settings = sim(1000, 1000, 12);

        let ui = MainWindow::new().unwrap();
        let sizes = grid::settle(&ui);
        ui.set_payload_model(ModelRc::from(Rc::new(VecModel::from(vec![
            payload_info_from(0, &first, &settings),
            payload_info_from(1, &second, &settings),
        ]))));
        // Opened as main() opens it for two payloads, so both panels are laid
        // out: a panel scrolled out of sight is never built at all.
        let shape = grid::grid_shape(&sizes, 2);
        ui.window().set_size(grid::window_size(&sizes, &shape, 2));
        ui.show().unwrap();

        let shown: Vec<String> =
            ElementHandle::find_by_element_type_name(&ui, "Text")
                .filter_map(|e| e.accessible_label())
                .map(|label| label.to_string())
                .collect();

        for wanted in ["DH0 (id 0)", "DH1 (id 1)"] {
            assert!(
                shown.iter().any(|line| line == wanted),
                "no panel is labelled {wanted:?}: {shown:?}"
            );
        }
    }

    /// Pressing a panel's Configuration button shows what both files said about
    /// that payload.
    ///
    /// Everything up to the window is checked elsewhere -- the text itself by
    /// `tcsprotocol::payload_parameters`, and that a panel carries it by
    /// the model -- and none of that puts anything on screen if the button is
    /// not wired to the popup or the popup not to the text. This presses the
    /// button and reads what the window then laid out.
    ///
    /// Not a test of its own, for the reason given above: one test per binary
    /// may start the backend.
    fn the_params_button_shows_what_both_files_said() {
        use i_slint_backend_testing::ElementHandle;

        let dh = handler("DH0");
        let sim = sim(1000, 1000, 12);
        let ui = MainWindow::new().unwrap();
        let _ = grid::settle(&ui);
        ui.set_payload_model(ModelRc::from(Rc::new(VecModel::from(vec![
            payload_info_from(0, &dh, &sim),
        ]))));
        ui.show().unwrap();

        // Every line of text the window has laid out.
        let on_screen = |ui: &MainWindow| -> Vec<String> {
            // What the popup says is selectable text, whose words are its
            // accessible value rather than a label: see copy.slint.
            ElementHandle::find_by_element_type_name(ui, "Text")
                .filter_map(|e| e.accessible_label())
                .chain(
                    ElementHandle::find_by_element_type_name(ui, "TextInput")
                        .filter_map(|e| e.accessible_value()),
                )
                .map(|label| label.to_string())
                .collect()
        };

        // Nothing of the parameters is on screen until the button is pressed:
        // a panel is a dozen lines tall and the parameters are thirty.
        assert!(
            !on_screen(&ui)
                .iter()
                .any(|line| line.contains("packet_size")),
            "the parameters are shown before anyone asked for them"
        );

        let button: Vec<_> =
            ElementHandle::find_by_accessible_label(&ui, "Configuration")
                .collect();
        assert_eq!(
            button.len(),
            1,
            "the panel has no Configuration button to press"
        );
        button[0].invoke_accessible_default_action();

        let shown = on_screen(&ui);
        assert!(
            shown.iter().any(|line| line == "DH0 parameters"),
            "the popup does not say which payload it is describing: {shown:?}"
        );

        // And what it shows is what the two files said, which is the one
        // description both programs use.
        let wanted = payload_parameters(&dh, Some(&sim));
        assert!(
            shown.contains(&wanted),
            "the popup shows something other than the parameters of this \
             payload:\n{shown:?}"
        );

        // More lines than the popup has room for, and a wheel turned over
        // them brings up the rest. The text stood in its scrolling area
        // alone, which measures only layouts, so the area took it to be no
        // taller than itself and the lines at the foot -- the faults --
        // could not be reached.
        let find = |id: &str| {
            ElementHandle::find_by_element_id(&ui, id)
                .next()
                .unwrap_or_else(|| panic!("no {id}"))
        };
        let area = find("PayloadPanel::parameters-scroll");
        let text = find("PayloadPanel::parameters-text");
        assert!(
            text.size().height > area.size().height,
            "the parameters ({}) fit the popup ({}): nothing to scroll",
            text.size().height,
            area.size().height
        );
        let before = text.absolute_position().y;
        // The backend places an element of a popup from the popup's corner
        // rather than the window's, and the popup hangs 4 pixels in from
        // the panel's.
        let panel =
            ElementHandle::find_by_element_type_name(&ui, "PayloadPanel")
                .next()
                .expect("a panel")
                .absolute_position();
        let over = area.absolute_position();
        ui.window().dispatch_event(
            slint::platform::WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(
                    panel.x + 4.0 + over.x + area.size().width / 2.0,
                    panel.y + 4.0 + over.y + area.size().height / 2.0,
                ),
                delta_x: 0.0,
                delta_y: -60.0,
            },
        );
        assert!(
            text.absolute_position().y < before,
            "a turn of the wheel did not scroll the parameters"
        );
    }

    /// A panel shows the data handler's packet size and the simulator file's
    /// settings, so an edit starts from what the files said.
    #[test]
    fn a_panel_shows_both_files_values() {
        let dh = DHConfig {
            name: DHName::new("DH3"),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: "localhost".to_string(),
                port: 5003,
            }),
            packet_size: 15,
            oc: None,
            ground: None,
            mode: Default::default(),
            stale_interval: None,
            sends_from: None,
            sim_address: None,
            driven: false,
        };

        let info = payload_info_from(0, &dh, &sim(500, 250, 5));
        assert_eq!(info.name, SharedString::from("DH3"));
        assert_eq!(info.config, SharedString::from("UDP localhost:5003"));
    }
}
