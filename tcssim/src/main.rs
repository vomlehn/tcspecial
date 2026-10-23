//! TCSpecial Payload Simulator (tcssim)
//!
//! A GUI application for simulating payloads that communicate with tcspecial.
//!
//! Two configuration files describe what to simulate. The tcspecial
//! configuration describes the payloads themselves: which data handlers exist,
//! how tcspecial reaches each one, and how big its packets are. The simulator
//! file adds what simulating one takes and the tcspecial configuration has no
//! business knowing: how fast a payload produces packets, and how a packet is
//! divided into segments. The two are joined by the data handler's name; see
//! [`tcslibgs::sim_config`].
//!
//! `--simulate=<mode>` is the run's simulation mode, as tcspecial and tcsmoc
//! take it: see `tcslibgs::simulate`. Under `none`, the flight setting and
//! the mode a run without the flag gets, tcssim refuses to start, there being
//! nothing for it to do in a run that may have no stand-ins. Under `some` and
//! `all` it checks that what it will stand in for, with what is a stand-in
//! already, is what the mode allows.
//!
//! Every stand-in is made as tcssim starts and kept until it stops, so that
//! tcspecial finds them whatever the panels say; a panel's button decides
//! only whether its payload sends. Before making any, tcssim binds the
//! readiness socket its simulator file states as `tcssim.ready`, and answers
//! there whether they are made: see [`ready`].
//!
//! `--wait[=<seconds>]` is the other end of that: given the same files and
//! mode, it waits for a simulator already started on them to be ready, and
//! exits successfully when it is, or with a failure saying why. Thirty
//! seconds unless told otherwise.

use slint::{Model, ModelRc, SharedString, VecModel};
use std::env;
use std::path::Path;
use std::process;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use tcslibgs::config::{payload_path_from_args, SIM_PAYLOAD_CONFIG_PATH_VAR};
use tcslibgs::{payload_parameters, DHConfig, DHSample, Edge, NO_TRANSFER_TIME};

use std::collections::BTreeMap;

mod endpoint;
mod grid;
mod node;
mod payload;
mod pty;
mod ready;
mod thermal;

use endpoint::{endpoint_description, payload_config_attached};
use payload::{PayloadConfig, PayloadStats, SimulatedPayload};
use tcslibgs::graph::Graph;
use tcslibgs::ready::SetUpFor;
use tcslibgs::simulate::{counted, judge, refusals, Host, Judged, Mode};
use tcslibgs::{ResolvedSim, SimConfigFile};
use tcsverify::startup::Startup;

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
/// The formatting is [`DHSample::panel_lines`], which tcsmoc's panels use
/// too: the two show the one transfer from opposite ends of a link and are
/// read side by side, so a byte or a time that read differently in the two
/// would look like a difference in the data.
fn sample_lines(sample: &DHSample) -> (SharedString, SharedString) {
    let (time, data) = sample.panel_lines();
    (SharedString::from(time), SharedString::from(data))
}

/// Take an edited setting to the payload it belongs to, and say whether the
/// two sizes still describe something that can be sent.
///
/// A panel holds no state of its own: what a spin box shows is what the model
/// says, so an edit has to come through here to be kept. The answer about the
/// sizes is decided here too, so there is one rule -- the window disables the
/// button while this says something and shows what it says, rather than
/// comparing the two numbers itself.
fn handle_config_edits(
    ui: &MainWindow,
    payloads: Arc<Mutex<Vec<SimulatedPayload>>>,
    model: Rc<VecModel<PayloadInfo>>,
) {
    ui.on_config_payload(
        move |row, packet_size, segment_size, packet_interval, segment_interval| {
            let guard = payloads.lock().unwrap();
            if let Some(payload) = guard.get(row as usize) {
                payload.set_packet_size(packet_size as u32);
                payload.set_segment_size(segment_size as u32);
                payload.set_packet_interval(packet_interval as u32);
                payload.set_segment_interval(segment_interval as u32);
            }
            drop(guard);

            let problem = sizes_problem(packet_size, segment_size);
            update_row(&model, row as usize, |info| {
                info.sizes_problem = SharedString::from(problem);
            });
        },
    );
}

/// What is wrong with a packet size and a segment size together, and nothing
/// when they agree.
///
/// A segment is a piece of a packet, so a segment larger than the packet
/// describes a piece larger than the whole it is a piece of. Equal is the
/// ordinary case -- a packet sent in one segment -- and is what a simulator
/// file that states no segment size asks for, so only a segment strictly
/// larger than the packet is wrong.
///
/// Returns the sentence the window shows, which says both numbers and either
/// way out of it: the two spin boxes are side by side and either can be the
/// one that was meant to change.
fn sizes_problem(packet_size: i32, segment_size: i32) -> String {
    if segment_size <= packet_size {
        return String::new();
    }

    format!(
        "A segment of {segment_size} bytes cannot be part of a packet of \
         {packet_size}: a segment is a piece of a packet. Raise the packet size \
         to {segment_size} or more, or lower the segment size to {packet_size} \
         or less."
    )
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

/// Which way a press of a panel's one button goes.
///
/// Decided by what the button offered, read back from the status its label
/// comes from -- not by what the payload turns out to be doing. Anything that
/// is not the running status offers to send: a payload that failed to
/// start shows the stopped status, and the useful thing to offer then is the
/// start that failed rather than a stop of something that never began.
fn transfer_wanted(status: &str) -> Transfer {
    if status == RUNNING_STATUS {
        Transfer::Silent
    } else {
        Transfer::Send
    }
}

/// Put a payload's traffic into its panel.
///
/// The times are what the panel is for: a count tells how much has moved
/// since the payload started, and the time tells whether any of it is moving
/// now. A payload stopped ten minutes ago and one sending every second have
/// the same count a moment after they are looked at. The bytes beside each
/// time are the head of that packet, which is what says the traffic is the
/// traffic that was expected rather than merely traffic.
fn show_traffic(info: &mut PayloadInfo, stats: &PayloadStats) {
    info.packets_sent = i32::try_from(stats.packets_sent).unwrap_or(i32::MAX);
    info.packets_recv = i32::try_from(stats.packets_recv).unwrap_or(i32::MAX);
    (info.last_sent_time, info.last_sent_data) = sample_lines(&stats.last_sent);
    (info.last_recv_time, info.last_recv_data) = sample_lines(&stats.last_recv);
}

/// Turn a data handler and its settled simulator settings into the panel the
/// window shows for it.
fn payload_info_from(dev_id: usize, dh: &DHConfig, sim: &ResolvedSim) -> PayloadInfo {
    PayloadInfo {
        name: SharedString::from(dh.name.0.clone()),
        // Its place in the payload configuration, which is the only number a
        // payload has: nothing in the file states one, and tcsmoc counts the
        // same way.
        dev_id: i32::try_from(dev_id).unwrap_or(i32::MAX),
        config: SharedString::from(endpoint_description(&dh.endpoint)),
        packet_size: i32::try_from(dh.packet_size).unwrap_or(i32::MAX),
        segment_size: sim.segment_size as i32,
        packet_interval: sim.packet_interval.as_millis() as i32,
        segment_interval: sim.segment_interval.as_millis() as i32,
        status: SharedString::from(STOPPED_STATUS),
        last_sent_time: SharedString::from(NO_TRANSFER_TIME),
        last_sent_data: SharedString::new(),
        last_recv_time: SharedString::from(NO_TRANSFER_TIME),
        last_recv_data: SharedString::new(),
        packets_sent: 0,
        packets_recv: 0,
        // Both files, as they were read. Tcssim has both in hand, so its
        // panels can show the whole of what a payload set says about a
        // payload.
        parameters: SharedString::from(payload_parameters(dh, Some(sim))),
        // Checked from the start, not only after an edit: a simulator file may
        // state a segment larger than the tcspecial configuration's packet, and
        // a panel that said nothing about it until a spin box was touched would
        // have the payload refuse to start for no stated reason. Whether
        // anything is ever sent to this payload, which decides whether its
        // panel has a received line at all.
        triggered: sim.triggered,
        sizes_problem: SharedString::from(sizes_problem(
            i32::try_from(dh.packet_size).unwrap_or(i32::MAX),
            sim.segment_size as i32,
        )),
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

/// What each simulated payload is to a thermal mass, by payload name.
///
/// One `ThermalMass` per declared mass, shared by every payload attached to
/// it: the sensors read the state the heaters warm, which is what makes a
/// simulated control loop a loop rather than three payloads sending noise.
///
/// A sensor's encoding comes from the conversion its readings go to -- see
/// `tcslibgs::graph::simulated::reading_of` -- so the bytes it sends are the
/// bytes the vehicle reads back as the mass's temperature, and neither file
/// states the scale twice.
fn thermal_masses(
    sim_file: &SimConfigFile,
    graph: Option<&tcslibgs::graph::Graph>,
    sims: &[ResolvedSim],
    dh_configs: &[DHConfig],
) -> Result<BTreeMap<String, crate::thermal::Thermal>, String> {
    use tcslibgs::sim_config::Attached;

    let declared = sim_file.masses().map_err(|e| e.to_string())?;
    if declared.is_empty() {
        return Ok(BTreeMap::new());
    }

    // A mass needs a graph to take its sensors' encodings from: without the
    // graph's conversions nothing says which bytes stand for a temperature.
    let Some(graph) = graph else {
        return Err(format!(
            "a thermal_mass is declared ({}), and no graph was read: a \
             simulated sensor sends the bytes a to_kelvin node reads back as \
             the temperature, and without a graph there is no such node",
            declared
                .iter()
                .map(|m| m.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    };

    let now = std::time::Instant::now();
    let built: BTreeMap<String, Arc<crate::thermal::ThermalMass>> = declared
        .iter()
        .map(|mass| {
            (
                mass.name.clone(),
                Arc::new(crate::thermal::ThermalMass::new(mass, now)),
            )
        })
        .collect();

    let mut attached = BTreeMap::new();
    for (dh, sim) in dh_configs.iter().zip(sims) {
        let name = dh.name.0.clone();
        match &sim.attached {
            None => {}
            Some(Attached::Reads(mass)) => {
                let reading = tcslibgs::graph::simulated::reading_of(graph, &name)?;
                attached.insert(
                    name,
                    crate::thermal::Thermal::Reads {
                        // The mass is known: `masses` refused one that is not.
                        mass: built[mass].clone(),
                        reading,
                    },
                );
            }
            Some(Attached::Heats { mass, rate }) => {
                attached.insert(
                    name,
                    crate::thermal::Thermal::Heats {
                        mass: built[mass].clone(),
                        rate: *rate,
                    },
                );
            }
        }
    }
    Ok(attached)
}

/// What [`load`] reads out of the pair: the handlers the simulator stands in
/// for, the simulator file, its settings resolved against them, and the
/// payloads' own configurations.
type Loaded = (
    Vec<DHConfig>,
    SimConfigFile,
    Vec<ResolvedSim>,
    Vec<PayloadConfig>,
);

/// Read the pair, once tcsverify has found nothing wrong with it, and say in
/// `startup` whatever stops it being simulated.
///
/// Every one of these is a refusal tcsverify has already made, bar the ones
/// that are the simulator's alone -- a payload it cannot stand in for, and a
/// set with no payloads to simulate, which leaves it nothing to be. They are
/// asked again rather than assumed: the files can change between the two
/// reads, and a simulator running on a half-read pair would be worse than one
/// that says so.
fn load(payload_path: &str, sim_path: &str, mode: Mode, startup: &mut Startup) -> Option<Loaded> {
    let sim_file = match SimConfigFile::load(sim_path) {
        Ok(sim_file) => sim_file,
        Err(e) => {
            startup.error(format!(
                "Error loading simulator configuration from {sim_path}: {e}"
            ));
            return None;
        }
    };

    // A configuration's payloads are nodes of a graph, and the ones simulated
    // are the ones the simulator file names: see tcslibgs::graph::simulated,
    // which makes of each a data handler, reached where tcspecial reaches it.
    // Kept rather than dropped once the handlers are made: a sensor attached
    // to a thermal mass sends its temperature in the encoding the graph's own
    // conversion reads, and that is where the encoding is.
    let mut graph = None;
    let loaded = tcslibgs::graph::Graph::load(payload_path)
        .map_err(|e| e.to_string())
        .and_then(|read| {
            let handlers = tcslibgs::graph::simulated::handlers(&read, &sim_file);
            graph = Some(read);
            handlers
        });
    let dh_configs = match loaded {
        Ok(dh_configs) => dh_configs,
        Err(e) => {
            startup.error(format!(
                "Error loading payload configuration from {payload_path}: {e}"
            ));
            return None;
        }
    };

    // Whether what this simulator will stand in for, with what is a stand-in
    // already, is what the mode allows. A simulator file that forgot a
    // payload is caught here, before anything runs; only tcspecial's own
    // check protects the hardware, because only tcspecial opens it.
    if let Some(graph) = graph.as_ref() {
        let refused = stand_ins_refused(graph, &sim_file, sim_path, mode);
        if !refused.is_empty() {
            for refusal in refused {
                startup.error(format!("{payload_path}: {refusal}"));
            }
            return None;
        }
    }

    // Join the two by name. Every data handler needs settings and every
    // settings entry needs a handler, so this is where a file naming a payload
    // that does not exist, or leaving one undriven, is caught.
    let sims = match sim_file.resolve(&dh_configs) {
        Ok(sims) => sims,
        Err(e) => {
            startup.error(format!("Error in {sim_path} against {payload_path}: {e}"));
            return None;
        }
    };

    // The thermal masses, built once and shared: the sensors attached to one
    // read the same state the heaters attached to it warm, which is the whole
    // point of a mass being a thing rather than a figure per payload.
    let masses = match thermal_masses(&sim_file, graph.as_ref(), &sims, &dh_configs) {
        Ok(masses) => masses,
        Err(e) => {
            startup.error(format!("Error in {sim_path}: {e}"));
            return None;
        }
    };

    let configs: Vec<PayloadConfig> = match dh_configs
        .iter()
        .zip(&sims)
        .map(|(dh, sim)| payload_config_attached(dh, sim, masses.get(&dh.name.0).cloned()))
        .collect()
    {
        Ok(configs) => configs,
        Err(e) => {
            startup.error(format!(
                "Error in payload configuration {payload_path}: {e}"
            ));
            return None;
        }
    };

    for (name, thermal) in &masses {
        match thermal {
            crate::thermal::Thermal::Reads { mass, .. } => eprintln!(
                "{name} reports the thermal mass {}, which starts at {:.2} K",
                mass.name(),
                mass.temperature()
            ),
            crate::thermal::Thermal::Heats { mass, rate } => eprintln!(
                "{name} warms the thermal mass {} by {rate} K/s while it is on",
                mass.name()
            ),
        }
    }

    if configs.is_empty() {
        startup.error(format!("{payload_path} describes no payloads to simulate"));
        return None;
    }

    Some((dh_configs, sim_file, sims, configs))
}

/// Every way the payloads of `graph` are not what `mode` allows, counting as
/// a stand-in each payload the simulator file names, which tcssim will stand
/// in for, and each that already is one by `tcslibgs::simulate`'s rules -- a
/// heater on a gpio-sim chip, say, which bin/mkgpio made and the simulator
/// file leaves out.
fn stand_ins_refused(
    graph: &Graph,
    sim_file: &SimConfigFile,
    sim_path: &str,
    mode: Mode,
) -> Vec<String> {
    let host = Host::this();
    let judged: Vec<(&str, Judged)> = graph
        .nodes
        .iter()
        .filter(|node| counted(node))
        .filter_map(|node| {
            let named = sim_file
                .simulated_payloads
                .iter()
                .any(|simulated| simulated.name == node.name);
            let judged = if named {
                Judged::stand_in(
                    format!("{sim_path} names it, for tcssim to stand in for"),
                    "path",
                    "",
                )
            } else {
                judge(&host, node)?
            };
            Some((node.name.as_str(), judged))
        })
        .collect();
    let pairs: Vec<(&str, &Judged)> = judged.iter().map(|(name, j)| (*name, j)).collect();
    refusals(mode, &pairs)
        .into_iter()
        .map(|refusal| match mode {
            Mode::All => format!("{refusal}, or name it in {sim_path} for tcssim to stand in for"),
            _ => refusal,
        })
        .collect()
}

/// Read where this simulator says whether it is ready, and bind it, with
/// what it is set up for: or say what stops either.
fn ready_socket(
    payload_path: &str,
    sim_path: &str,
    sim_file: &SimConfigFile,
    mode: Mode,
) -> Result<(std::net::UdpSocket, SetUpFor), String> {
    let at = sim_file.ready().map_err(|e| format!("{sim_path}: {e}"))?;
    let set_up_for = set_up_for(payload_path, sim_path, mode)?;
    let socket = ready::bind(at).map_err(|e| format!("{sim_path}: {e}"))?;
    eprintln!("answering whether it is ready at {at}");
    Ok((socket, set_up_for))
}

/// What a simulator started on these files under `mode` is set up for, as
/// its readiness answers say and a harness compares.
fn set_up_for(payload_path: &str, sim_path: &str, mode: Mode) -> Result<SetUpFor, String> {
    Ok(SetUpFor {
        configuration: tcslibgs::config_digest::digest_of_file(payload_path)
            .map_err(|e| format!("{payload_path}: {e}"))?,
        simulator: tcslibgs::config_digest::digest_of_simulator_file(sim_path)
            .map_err(|e| format!("{sim_path}: {e}"))?,
        mode,
    })
}

/// How long `--wait` waits where it is not told.
const WAIT_DEFAULT: std::time::Duration = std::time::Duration::from_secs(30);

/// Take `--wait` or `--wait=<seconds>` out of the arguments: how long to
/// wait, where it was given, and what is left.
fn take_wait(args: Vec<String>) -> Result<(Option<std::time::Duration>, Vec<String>), String> {
    let mut waiting = None;
    let mut rest = Vec::with_capacity(args.len());
    for arg in args {
        let patience = if arg == "--wait" {
            WAIT_DEFAULT
        } else if let Some(seconds) = arg.strip_prefix("--wait=") {
            match seconds.parse::<f64>() {
                Ok(seconds) if seconds > 0.0 && seconds.is_finite() => {
                    std::time::Duration::from_secs_f64(seconds)
                }
                _ => {
                    return Err(format!(
                        "--wait={seconds} is not a number of seconds: give --wait=30, or \
                         --wait for thirty seconds"
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

/// `--wait`: wait for a simulator started on these files under `mode` to be
/// ready, and say what to exit with. Nothing is stood in for and no window
/// opens: this is the harness for a run started by hand.
fn wait_for_another(
    payload_path: &str,
    sim_path: &str,
    mode: Mode,
    patience: std::time::Duration,
) -> i32 {
    let wanted = SimConfigFile::load(sim_path)
        .map_err(|e| format!("{sim_path}: {e}"))
        .and_then(|sim_file| sim_file.ready().map_err(|e| format!("{sim_path}: {e}")))
        .and_then(|at| Ok((at, set_up_for(payload_path, sim_path, mode)?)));
    let (at, ours) = match wanted {
        Ok(wanted) => wanted,
        Err(e) => {
            eprintln!("tcssim --wait: {e}");
            return 1;
        }
    };
    match tcslib::simulator::wait(at, &ours, patience) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("tcssim --wait: {e}");
            1
        }
    }
}

/// Say what was found as tcssim started, in a window of its own, and wait for
/// it to be closed.
///
/// Before the main window is shown, so that nobody is looking at panels for a
/// pair with something wrong with it without having been told -- and on
/// stderr as well, for a run from a terminal and for one with no display to
/// open a window on.
fn present(startup: &Startup) {
    startup.eprint("tcssim");

    let window = match StartupWindow::new() {
        Ok(window) => window,
        Err(e) => {
            eprintln!("tcssim: what was found cannot be shown in a window: {e}");
            return;
        }
    };
    window.set_program("tcssim".into());
    let heading = if startup.fatal() {
        "Something is wrong with the configuration."
    } else {
        "Something about the configuration is worth knowing."
    };
    window.set_heading(heading.into());
    window.set_everything(startup.copied(heading).into());
    window.set_fatal(startup.fatal());
    let lines: Vec<StartupLine> = startup
        .lines()
        .iter()
        .map(|(error, text)| StartupLine {
            text: text.clone().into(),
            error: *error,
        })
        .collect();
    window.set_lines(ModelRc::from(Rc::new(VecModel::from(lines))));
    {
        let weak = window.as_weak();
        window.on_acknowledged(move || {
            if let Some(window) = weak.upgrade() {
                let _ = window.hide();
            }
        });
    }
    // Runs until it is closed, by its button or by the window manager: either
    // is the person having read it.
    if let Err(e) = window.run() {
        eprintln!("tcssim: what was found cannot be shown in a window: {e}");
    }
}

fn main() {
    //    env_logger::init();

    // The mode first, and before anything is read: under the flight setting
    // there is nothing to read anything for.
    let (mode, args) = match tcslibgs::simulate::take_mode(env::args().collect()) {
        Ok(taken) => taken,
        Err(e) => {
            eprintln!("tcssim: {e}");
            process::exit(1);
        }
    };
    let (waiting, args) = match take_wait(args) {
        Ok(taken) => taken,
        Err(e) => {
            eprintln!("tcssim: {e}");
            process::exit(1);
        }
    };
    if mode.is_flight() {
        eprintln!(
            "tcssim: --simulate={mode} is the flight setting, which allows no stand-ins, \
             so there is nothing for a simulator to do or to wait for: give \
             --simulate=some or --simulate=all for a run with stand-ins"
        );
        process::exit(1);
    }

    // Read both configuration files before the window exists, so a file that
    // cannot be read is reported plainly rather than as an empty window.
    let payload_path =
        match payload_path_from_args(args.into_iter(), Some(SIM_PAYLOAD_CONFIG_PATH_VAR)) {
            Ok(payload_path) => payload_path,
            Err(e) => {
                eprintln!("{}", e);
                process::exit(1);
            }
        };
    // The one PAYLOAD_SIM_YAML names, and otherwise the one beside the
    // configuration and named for it, found as tcsmoc finds it.
    let sim_path = env::var(SIM_CONFIG_PATH_VAR).unwrap_or_else(|_| {
        tcslibgs::config::sim_path_beside(&payload_path)
            .unwrap_or_else(|| DEFAULT_SIM_CONFIG_PATH.to_string())
    });

    if let Some(patience) = waiting {
        process::exit(wait_for_another(&payload_path, &sim_path, mode, patience));
    }

    // Everything wrong with the pair, in tcsverify's words and before
    // anything else is read from either: every problem at once, each with its
    // file and line, which is what tcsverify, tcsmoc, tcspecial and tcsgui
    // say about the same files. Then what only a simulator has to ask. All of
    // it is shown before the main window is, and the window is not shown at
    // all where any of it is an error.
    let mut startup = Startup::default();
    startup.report(
        &tcsverify::verify(
            Path::new(&payload_path),
            Some(Path::new(&sim_path)),
            tcsverify::Limits::EVERYTHING,
        ),
        true,
    );
    let loaded = if startup.fatal() {
        None
    } else {
        load(&payload_path, &sim_path, mode, &mut startup)
    };
    if !startup.is_empty() {
        present(&startup);
    }
    let Some((dh_configs, sim_file, sims, configs)) = loaded.filter(|_| !startup.fatal()) else {
        process::exit(1);
    };

    // Where this simulator says whether it is ready, bound before any
    // stand-in is made: a harness asking while it starts is told so, and an
    // address already held -- by a simulator left running -- stops this one
    // before it fights that one for the payloads' paths.
    let readiness = match ready_socket(&payload_path, &sim_path, &sim_file, mode) {
        Ok(readiness) => readiness,
        Err(e) => {
            let mut refused = Startup::default();
            refused.error(e);
            present(&refused);
            process::exit(1);
        }
    };

    eprintln!(
        "Simulating {} payloads from {} and {}",
        configs.len(),
        payload_path,
        sim_path
    );
    if let Some(description) = &sim_file.description {
        eprintln!("Simulator configuration: {description}");
    }

    let ui = MainWindow::new().unwrap();
    let sizes = grid::settle(&ui);
    let ui_weak = ui.as_weak();

    // One panel per payload, in a grid worked out from how many there are.
    let model: Rc<VecModel<PayloadInfo>> = Rc::new(VecModel::from(
        dh_configs
            .iter()
            .zip(&sims)
            .enumerate()
            .map(|(row, (dh, sim))| payload_info_from(row, dh, sim))
            .collect::<Vec<_>>(),
    ));
    ui.set_payload_model(ModelRc::from(model.clone()));

    // What the markup says a panel and the chrome come to, asked for rather
    // than written down here as well: see grid::Sizes.
    let shape = grid::grid_shape(&sizes, configs.len());
    // Taller than the grid asks for where there is a third row to look
    // ahead for, and no taller than the payloads on hand need otherwise. It
    // is also why the window can open taller than it is wide, which the shape
    // itself never is: the shape rule chooses the grid, and the room rule
    // says how much of the window to give it.
    ui.window()
        .set_size(grid::window_size(&sizes, &shape, configs.len()));

    // Every stand-in made now, and each payload silent until its panel says
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
    let mut made: Vec<SimulatedPayload> = configs.into_iter().map(SimulatedPayload::new).collect();
    for payload in &mut made {
        // A payload not yet running, so this cannot be refused.
        let _ = payload.start();
    }
    let payloads: Arc<Mutex<Vec<SimulatedPayload>>> = Arc::new(Mutex::new(made));

    // The one button each panel has, which has its payload send or not.
    {
        let payloads = payloads.clone();
        let model = model.clone();
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
        });
    }

    handle_config_edits(&ui, payloads.clone(), model.clone());

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
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(500),
            move || {
                let guard = payloads.lock().unwrap();
                for (row, payload) in guard.iter().enumerate() {
                    let stats = payload.stats();
                    update_row(&model, row, |info| show_traffic(info, &stats));
                }
            },
        );

        // Keep timer alive
        std::mem::forget(timer);
    }

    // Against the bottom of the display, tcsmoc having asked for the top.
    ask_for_the_edge(&ui, Edge::Bottom);

    ui.run().unwrap();
}

/// Ask for this window to be put against one edge of the display.
///
/// Asked once the event loop is running, because that is when the window has
/// a size: before it is shown, what it reports is what was requested rather
/// than what it was given, and the bottom of the display is the display's
/// height less the window's.
///
/// A request, and not a thing to report on: see `tcslibgs::display`.
fn ask_for_the_edge(ui: &MainWindow, edge: Edge) {
    let ui_weak = ui.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::SingleShot,
        std::time::Duration::ZERO,
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let window = ui.window();
            if let Some(y) =
                tcslibgs::display::top_of(edge, tcslibgs::screen_height(), window.size().height)
            {
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
    use std::path::Path;
    use std::time::Duration;
    use tcslibgs::{DHName, EndpointConfig, NetworkConfig, NetworkProtocol};

    fn repo_file(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(name)
    }

    /// An example is simulated: its simulated payload nodes, reached where
    /// tcspecial reaches them, with the simulator file beside it found as
    /// tcsmoc finds it.
    #[test]
    fn an_example_is_simulated() {
        let payload = repo_file("examples/minimal/minimal.yaml");
        let payload = payload.to_str().expect("a path");
        let sim = tcslibgs::config::sim_path_beside(payload).expect("its simulator file");
        assert!(sim.ends_with("minimal-sim.yaml"), "{sim}");

        let mut startup = Startup::default();
        let (dh_configs, _, sims, configs) =
            load(payload, &sim, Mode::All, &mut startup).expect("it loads");
        assert!(startup.is_empty(), "{:?}", startup.lines());
        assert_eq!(dh_configs.len(), 1);
        assert_eq!(dh_configs[0].name.0, "sensor");
        assert_eq!(sims.len(), 1);
        assert_eq!(configs.len(), 1);
    }

    /// One whose simulator file names nothing is refused, saying so: there is
    /// nothing for a simulator to be.
    #[test]
    fn an_example_with_nothing_simulated_is_refused() {
        let payload = repo_file("examples/simple-app/simple-app.yaml");
        let payload = payload.to_str().expect("a path");
        let sim = tcslibgs::config::sim_path_beside(payload).expect("its simulator file");

        let mut startup = Startup::default();
        assert!(load(payload, &sim, Mode::All, &mut startup).is_none());
        assert!(
            startup
                .lines()
                .iter()
                .any(|(error, line)| *error && line.contains("no payloads to simulate")),
            "{:?}",
            startup.lines()
        );
    }

    /// A sample of a transfer, as the stats hold it.
    ///
    /// Taking the time in seconds, which is what these tests mean; a time is
    /// carried in microseconds since the epoch.
    fn sample_of(seconds: u64, data: &[u8]) -> DHSample {
        let mut sample = DHSample::new();
        sample.record(data);
        sample.time = Some(tcslibgs::Timestamp {
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
            .join("docs/design.rst");
        let text = std::fs::read_to_string(&doc).unwrap();

        // The four payloads of tcspecial1.yaml, which is the set the document
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

    /// An edit that makes the two sizes disagree says so and takes the
    /// button away, and an edit that fixes them gives it back.
    ///
    /// The whole path, from the spin box to the button: the window asks Rust,
    /// Rust answers in the row, and the panel obeys the answer. Checking the
    /// rule alone would say nothing about any of that, and a panel that
    /// quietly let an unsendable pair be started is what this is for.
    ///
    /// Not a test of its own: one test per binary may start the testing
    /// backend, and its windows belong to the thread that made them.
    fn sizes_that_cannot_be_sent_stop_the_button_and_say_so() {
        use i_slint_backend_testing::ElementHandle;

        let dh = handler("DH0");
        let settings = sim(1000, 1000, 12);
        let model: Rc<VecModel<PayloadInfo>> =
            Rc::new(VecModel::from(vec![payload_info_from(0, &dh, &settings)]));

        let ui = MainWindow::new().unwrap();
        let _ = grid::settle(&ui);
        ui.set_payload_model(ModelRc::from(model.clone()));

        // The handler the running program registers, over the payload the
        // files describe.
        let payloads: Arc<Mutex<Vec<SimulatedPayload>>> =
            Arc::new(Mutex::new(vec![SimulatedPayload::new(
                payload_config_attached(&dh, &settings, None).expect("a payload"),
            )]));
        handle_config_edits(&ui, payloads, model.clone());
        ui.show().unwrap();

        let button = |ui: &MainWindow| {
            ElementHandle::find_by_accessible_label(ui, ENABLE_LABEL)
                .next()
                .and_then(|b| b.accessible_enabled())
        };
        // What the popup says is selectable text, whose words are its
        // accessible value rather than a label: see copy.slint.
        let said = |ui: &MainWindow| -> Vec<String> {
            ElementHandle::find_by_element_type_name(ui, "Text")
                .filter_map(|e| e.accessible_label())
                .chain(
                    ElementHandle::find_by_element_type_name(ui, "TextInput")
                        .filter_map(|e| e.accessible_value()),
                )
                .map(|label| label.to_string())
                .filter(|line| line.contains("cannot be"))
                .collect()
        };

        // A packet of twelve bytes in segments of twelve: the ordinary case,
        // and nothing to say about it.
        assert_eq!(button(&ui), Some(true), "the button is not there to press");
        assert!(
            said(&ui).is_empty(),
            "something is said before anything is wrong"
        );

        // The packet size, down one, which leaves the segment larger than the
        // packet it is a piece of. The first spin box of the panel is the
        // packet size; Slint's testing backend does not report the last child
        // of a layout, which is why the segment's is not reached here.
        let packet_size = ElementHandle::find_by_element_type_name(&ui, "SpinBox")
            .next()
            .expect("the panel has a packet size to edit");
        packet_size.invoke_accessible_decrement_action();
        assert_eq!(packet_size.accessible_value().as_deref(), Some("11"));

        assert_eq!(
            button(&ui),
            Some(false),
            "a pair that cannot be sent left the button there to press"
        );
        let shown = said(&ui);
        assert!(
            shown
                .iter()
                .any(|line| line.contains("segment of 12") && line.contains("packet of 11")),
            "nothing on screen says what is wrong: {shown:?}"
        );

        // And back up again: the pair can be sent, so the button returns and
        // the popup stops saying otherwise. An open popup still saying these
        // sizes cannot be sent, after the edit that made them sendable, would
        // be the wrong answer left on the screen.
        packet_size.invoke_accessible_increment_action();
        assert_eq!(packet_size.accessible_value().as_deref(), Some("12"));
        assert_eq!(button(&ui), Some(true), "the button did not come back");
        assert!(
            said(&ui).is_empty(),
            "the popup is still saying it: {:?}",
            said(&ui)
        );
    }

    /// A payload whose files disagree about the two sizes cannot be started,
    /// and says why before anything is touched.
    ///
    /// A simulator file may state a segment larger than the tcspecial
    /// configuration's packet. A panel that said nothing about it until a spin
    /// box was touched would have the payload refuse to start for no stated
    /// reason.
    #[test]
    fn a_payload_whose_files_disagree_starts_out_unsendable() {
        let dh = handler("DH0");
        let info = payload_info_from(0, &dh, &sim(1000, 1000, 20));
        assert_eq!(
            dh.packet_size, 12,
            "the tcspecial configuration says twelve bytes"
        );
        assert!(
            info.sizes_problem.contains("segment of 20")
                && info.sizes_problem.contains("packet of 12"),
            "{}",
            info.sizes_problem
        );

        // And one whose files agree says nothing.
        let info = payload_info_from(0, &dh, &sim(1000, 1000, 12));
        assert_eq!(info.sizes_problem, "");
    }

    /// A segment larger than the packet it is a piece of is refused, and
    /// equal sizes are the ordinary case.
    #[test]
    fn a_segment_cannot_be_larger_than_its_packet() {
        // The ordinary case, which is what a simulator file stating no
        // segment size asks for: one segment, the size of the packet.
        assert_eq!(sizes_problem(12, 12), "");
        assert_eq!(sizes_problem(12, 5), "");
        assert_eq!(sizes_problem(1, 1), "");

        // And the one that cannot be sent. The sentence says both numbers and
        // either way out of it: the two spin boxes are side by side and
        // either can be the one that was meant to change.
        let said = sizes_problem(11, 12);
        assert!(said.contains("12") && said.contains("11"), "{said}");
        assert!(
            said.contains("Raise the packet size") && said.contains("lower the segment size"),
            "the error should say either way out of it: {said}"
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
        /// The label a button offering this carries.
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
        let window = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/main.slint");
        let source = std::fs::read_to_string(&window).unwrap();
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
        let mut info = payload_info_from(0, &handler("DH0"), &sim(1000, 1000, 12));
        assert_eq!(info.last_sent_time, NO_TRANSFER_TIME);
        assert_eq!(info.last_recv_time, NO_TRANSFER_TIME);

        // Nothing has moved yet, so neither row shows a time or any bytes: a
        // count of nought with a time beside it would be a transfer that
        // never happened.
        show_traffic(&mut info, &PayloadStats::default());
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
    /// inside the window tcssim opens for each shipped set.
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
            let sim_file = SimConfigFile::load(&sim_path)
                .unwrap_or_else(|e| panic!("{} failed to load: {e}", sim_path.display()));
            let dh_configs = simulated_payloads_of(&payload_path, &sim_file);
            let sims = sim_file
                .resolve(&dh_configs)
                .unwrap_or_else(|e| panic!("{} does not fit: {e}", sim_path.display()));

            let panels: Vec<PayloadInfo> = dh_configs
                .iter()
                .zip(&sims)
                .enumerate()
                .map(|(row, (dh, sim))| payload_info_from(row, dh, sim))
                .collect();
            // An example that simulates nothing has no window to measure:
            // `simple-app`'s payload is a program, which tcspecial runs
            // rather than reaches, so no simulator stands in for it.
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
            let area = ElementHandle::find_by_element_id(&ui, "MainWindow::scroll")
                .next()
                .expect("the window has a scrolling area for the panels");
            let grid = ElementHandle::find_by_element_id(&ui, "MainWindow::grid")
                .next()
                .expect("the window has a grid of panels");
            assert!(
                grid.size().width <= area.size().width && grid.size().height <= area.size().height,
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
            let wanted = grid::height_for_rows(&sizes, rows) - sizes.chrome_height;
            assert!(
                area.size().height >= wanted,
                "{set}: the panels have {} of the {wanted} that {rows} rows need",
                area.size().height
            );
            assert!(
                area.size().height < wanted + sizes.panel_height,
                "{set}: the panels have {}, room for a row more than the {rows} wanted",
                area.size().height
            );

            // Every button is inside the window across, which is what
            // keeps a label from running off the side of its panel: a
            // Button reports the width it was given, where a Text that does
            // not fit is clipped and reports the width it had.
            for button in
                i_slint_backend_testing::ElementHandle::find_by_element_type_name(&ui, "Button")
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
                "PayloadPanel::controls-row",
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
            let placed: Vec<Vec<(f32, f32)>> = rows.iter().map(|id| geometry(id)).collect();
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
            for (n, (top, panel_height)) in panels_found.iter().copied().enumerate() {
                let bottom = top + panel_height;
                let mut lowest = top;
                for (which, id) in rows.iter().enumerate() {
                    let (y, height) = placed[which][n];
                    assert!(
                        y >= lowest,
                        "{set}: panel {n}'s {id} starts at {y}, above the {lowest} \
                         the row before it reaches"
                    );
                    lowest = y + height;
                }

                let (sent, _) = sent_rows[n];
                for (label, head, foot) in buttons.iter() {
                    if *head >= top && *foot <= bottom {
                        assert!(
                            *foot <= sent,
                            "{set}: panel {n}'s {label:?} button reaches {foot}, over \
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
                    "{set}: panel {n} has {spare} pixels below its packet count, \
                     where {PANEL_PADDING} of padding is all it keeps"
                );
            }

            // The grid reflows when the window is resized.
            //
            // The column count is the window's own, worked out from the width
            // it has rather than handed over once at startup: a window
            // dragged wider is meant to fit another column and use it. This
            // drags it and counts the columns by how many panels share the
            // top row.
            //
            // Done last in the case, because it leaves the window a different
            // size than it was opened at.
            let columns_now = |ui: &MainWindow| -> usize {
                let tops: Vec<f32> = ElementHandle::find_by_element_type_name(ui, "PayloadPanel")
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
                    "{set}: a window one panel wide did not come down to one column"
                );

                // And wide enough for every panel in a row, so it goes up.
                let wide = panels as f32 * sizes.panel_width + sizes.chrome_width;
                ui.window()
                    .set_size(slint::LogicalSize::new(wide, window.height));
                assert_eq!(
                    columns_now(&ui),
                    panels,
                    "{set}: a window {panels} panels wide did not use the room"
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

        // The backend is up and this thread owns its windows, so the other
        // checks that need one run from here.
        the_markup_reports_the_sizes_the_tests_assume();
        the_panel_shows_the_time_and_bytes_it_was_given();
        each_panel_says_which_payload_of_the_set_it_is();
        the_params_button_shows_what_both_files_said();
        sizes_that_cannot_be_sent_stop_the_button_and_say_so();
    }

    /// The payloads of a configuration that its simulator file names, as
    /// `load` makes them.
    ///
    /// Through `tcslibgs::graph::simulated`, which is what turns a payload
    /// node into what the simulator stands in for: where it is reached, how
    /// big a packet is, and whether it answers a trigger.
    fn simulated_payloads_of(
        configuration: &std::path::Path,
        sim_file: &SimConfigFile,
    ) -> Vec<DHConfig> {
        let graph = tcslibgs::graph::Graph::load(configuration)
            .unwrap_or_else(|e| panic!("{} failed to load: {e}", configuration.display()));
        tcslibgs::graph::simulated::handlers(&graph, sim_file)
            .unwrap_or_else(|e| panic!("{} cannot be simulated: {e}", configuration.display()))
    }

    /// Every shipped configuration and the simulator file beside it.
    ///
    /// The examples, found rather than listed, so an example added to the
    /// repository is covered by these tests without anyone remembering to
    /// name it here.
    fn shipped_sets() -> Vec<(std::path::PathBuf, std::path::PathBuf)> {
        let root = repo_file("examples");
        let mut sets = Vec::new();

        for entry in std::fs::read_dir(&root).expect("the examples are readable") {
            let path = entry.expect("a readable directory entry").path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !path.is_dir() {
                continue;
            }

            let configuration = path.join(format!("{name}.yaml"));
            let sim_path = path.join(format!("{name}-sim.yaml"));
            if !configuration.is_file() {
                continue;
            }
            assert!(
                sim_path.exists(),
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

    /// A data handler, for the tests that only need one to exist.
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
        }
    }

    fn sim(packet_interval_ms: u32, segment_interval_ms: u32, segment_size: u32) -> ResolvedSim {
        ResolvedSim {
            attached: None,
            packet_interval: Duration::from_millis(u64::from(packet_interval_ms)),
            segment_interval: Duration::from_millis(u64::from(segment_interval_ms)),
            segment_size,
            triggered: false,
            faults: Default::default(),
            payload_address: None,
            payload_port: None,
        }
    }

    // Nothing here checks that no shipped simulator file fits another
    // configuration, because some do: `minimal` and `beacon` both have a
    // sensor on a UDP socket, and `minimal`'s simulator file fits either,
    // which is a true thing about two examples rather than a fault. The rule
    // that matters is that a simulated payload states the kind it stands in
    // for, and that is tested where the rule is:
    // `tcslibgs::sim_config`'s `the_two_files_cannot_disagree_about_the_kind`.

    /// Every shipped set has to describe payloads the simulator can simulate,
    /// and its two files have to agree with each other.
    ///
    /// Nothing else checks that: the GUI only reveals files that disagree when
    /// someone runs it and finds the simulator refusing to start.
    #[test]
    fn the_shipped_files_become_simulator_configs() {
        for (payload_path, sim_path) in shipped_sets() {
            let sim_file = SimConfigFile::load(&sim_path)
                .unwrap_or_else(|e| panic!("{} failed to load: {e}", sim_path.display()));
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
                .unwrap_or_else(|e| panic!("{} is not simulatable: {e}", payload_path.display()));

            // An example whose payloads are all programs simulates nothing,
            // which is a pair tcssim is right to accept: see `simple-app`,
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
            let text = std::fs::read_to_string(&payload_path).unwrap();
            // Whether the payload whose attributes are being read is one that
            // answers requests. Each payload starts with its name.
            let mut triggered = false;
            // Whether the line being read is in the tcspecial section, which
            // has a segment size of its own -- the telemetry log's, under
            // `log` -- and is not about a payload at all.
            let mut in_tcspecial = false;

            for (number, line) in text.lines().enumerate() {
                if !line.starts_with(char::is_whitespace) && line.contains(':') {
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
                    "{}:{} states packet_interval_ms for a payload that sends on \
                     its own, whose rate belongs in {}",
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
            let sim_file = SimConfigFile::load(&sim_path).unwrap();
            let dh_configs = simulated_payloads_of(&payload_path, &sim_file);
            let sims = sim_file.resolve(&dh_configs).unwrap();

            let panels: Vec<PayloadInfo> = dh_configs
                .iter()
                .zip(&sims)
                .enumerate()
                .map(|(row, (dh, s))| payload_info_from(row, dh, s))
                .collect();

            assert_eq!(panels.len(), dh_configs.len(), "{}", payload_path.display());

            // The shape rule and not the sizes, so there is no window to
            // ask: a grid that holds its panels does so whatever a panel
            // comes to.
            let shape = grid::grid_shape(&grid::Sizes::for_tests(), panels.len());
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

        /// Every line of a panel that begins with `start`, as laid out.
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
            ui.set_payload_model(ModelRc::from(Rc::new(VecModel::from(vec![info]))));
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
        let (sent, recv, pkts) = shown(payload_info_from(0, &handler("DH0"), &asked));
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
            "the panel does not show the time and bytes of the last packet received"
        );
        assert_eq!(pkts, vec!["Pkts: 3 sent / 1 recv".to_string()]);
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
        second.name = tcslibgs::DHName::new("DH1");
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

        let shown: Vec<String> = ElementHandle::find_by_element_type_name(&ui, "Text")
            .filter_map(|e| e.accessible_label())
            .map(|label| label.to_string())
            .collect();

        for wanted in ["DH0 (dev id 0)", "DH1 (dev id 1)"] {
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
    /// `tcslibgs::payload_parameters`, and that a panel carries it by the
    /// model -- and none of that puts anything on screen if the button is not
    /// wired to the popup or the popup not to the text. This presses the
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
            ElementHandle::find_by_accessible_label(&ui, "Configuration").collect();
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
        };

        let info = payload_info_from(0, &dh, &sim(500, 250, 5));
        assert_eq!(info.name, SharedString::from("DH3"));
        assert_eq!(info.config, SharedString::from("UDP localhost:5003"));
        // From the tcspecial configuration
        assert_eq!(info.packet_size, 15);
        // From the simulator file
        assert_eq!(info.packet_interval, 500);
        assert_eq!(info.segment_interval, 250);
        assert_eq!(info.segment_size, 5);
    }
}
