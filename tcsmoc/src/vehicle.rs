//! The MOC's view of the vehicle: running a configuration.
//!
//! A vehicle is a graph of nodes, and what the window shows is that graph:
//! each node drawn into a `moc` node sends to the `moc` node's `ground_base`
//! plus its device id, and each payload nothing touches sends
//! over `payload_telemetry` -- see `tcslibgs::graph::telemetry`. So there is
//! one panel per source, filled through `tcslib::moc`, the same function call
//! any ground software uses.
//!
//! # What the buttons send
//!
//! The vehicle takes commands on two links, and binds the second once for
//! every node, the node's id in the message saying which -- see
//! `tcslibgs::NodeId`. So Enable and Disable on a panel are START_NODE
//! and STOP_NODE about the node that panel is for, by the device id the panel
//! already shows.
//!
//! The unit is a node and not a payload, which is what makes this worth
//! having: a computation, a log and a sensor are each something the operator
//! can stop. A STOP_NODE is answered once the node has stopped *and* given up
//! what it held, so a success means its device, bus address or socket is free.
//!
//! # Why the digests have to agree first
//!
//! A node id means a position in the configuration the two ends read. What
//! makes them mean the same node by one id is the digest, which is computed
//! over the configuration including the order the nodes appear in -- so two
//! ends whose digests agree cannot disagree about which node an id names, and
//! two whose digests differ almost certainly do. A command sent under that
//! misunderstanding would stop something nobody chose.
//!
//! CONNECT is where the two digests are compared, and it is sent as soon as
//! the link comes up. Until it has agreed, the panels' buttons refuse: see
//! [`Agreed`].
//!
//! What stays the same is everything else the window does: the panels, the
//! grid they sit in, tcspecial and tcssim started on the same file, and Quit.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use tcslib::moc::Moc;
use tcslib::moc::DATUM_BYTES;
use tcslib::ping::{Pinger, Uplink};
use tcslib::simulator;
use tcslibgs::graph::telemetry::{self, Telemetered, Via};
use tcslibgs::graph::Graph;
use tcslibgs::ready::SetUpFor;
use tcslibgs::sim_config::SimConfigFile;
use tcslibgs::simulate::Mode;
use tcslibgs::{DHSample, Timestamp};
use tcsui::PingShown;
use tcsverify::startup::Startup;

use crate::beacon_receive::BeaconReceive;
use crate::ci_link::{CiLink, Commanded};
use crate::oc_link::{Arrival, Arrivals};
use crate::{
    ask_for_the_edge, grid_shape, handle_close, handle_quit, present,
    show_arrivals, stop, window_size, DHInfo, Edge, MainWindow, MenuAction,
    PingWindow, ProcessManager, Sizes, CHILDREN, CONNECTED_STATUS,
    DISCONNECTED_STATUS, ERROR_STATUS, NO_TRANSFER_TIME,
    PANEL_REFRESH_INTERVAL,
};

/// What a panel's status says of a node the spacecraft says is running.
pub const ACTIVE_STATUS: &str = "Active";

/// What a panel's status says of a node the spacecraft says is stopped.
pub const STOPPED_STATUS: &str = "Stopped";

/// What a panel's status says where nobody can speak for its node.
///
/// Where every panel starts: a MOC that has just come up has been told
/// nothing, and the tcspecial it attached to may have been running nodes for
/// hours. And what a panel says when the spacecraft answers that it has no
/// such node, which is a MOC attached to a tcspecial flying another
/// configuration.
pub const UNKNOWN_STATUS: &str = "Unknown";

/// What a panel's status says once contact with the spacecraft is lost.
///
/// Whatever it said before was what the spacecraft answered while it was
/// being heard from: a node since stopped, or gone with a process that
/// restarted, would go on looking as it last did. Stale says that the status
/// is the last one known and is not known to be true now.
pub const STALE_STATUS: &str = "Stale";

/// What a node does when contact with the spacecraft is lost: its status is
/// stale.
///
/// One node's, so that the response to a loss is said once for a node and
/// [`every_node_lost_contact`] is only the asking of each.
pub fn node_lost_contact(info: &mut DHInfo) {
    info.status = SharedString::from(STALE_STATUS);
}

/// Tell every node in the window that contact with the spacecraft is lost.
pub fn every_node_lost_contact(model: &ModelRc<DHInfo>) {
    for row in 0..model.row_count() {
        if let Some(mut info) = model.row_data(row) {
            node_lost_contact(&mut info);
            model.set_row_data(row, info);
        }
    }
}

/// Which panels' nodes the operator has stopped, one flag per panel: see
/// [`crate::Declined`]. Set by Disable and cleared by Enable, from the
/// spacecraft's answer, so that it says what the operator's last word about
/// each node was.
fn declined_for(rows: usize) -> crate::Declined {
    Arc::new(Mutex::new(vec![false; rows]))
}

/// Whether the operator's last word about the node of panel `row` was to
/// stop it.
fn was_declined(declined: &crate::Declined, row: usize) -> bool {
    declined
        .lock()
        .ok()
        .and_then(|flags| flags.get(row).copied())
        .unwrap_or(false)
}

/// What a node command says when the two ends have not agreed what they read.
pub const NOT_AGREED: &str = "The vehicle has not agreed which configuration \
                              it is flying, so      a node id means nothing \
                              yet: press Connect, which sends CONNECT and      \
                              compares the digests.";

/// Whether the vehicle has agreed that it is flying the configuration this
/// window read.
///
/// `None` until a CONNECT has been answered, `Some(false)` where the answer
/// disagreed. Shared between the Connect button, which sets it, and the
/// panels' buttons, which will not send a node id without it: a node id is a
/// position in the configuration, so an id sent to a vehicle flying another
/// one names whatever happens to be in that position.
pub type Agreed = Arc<Mutex<Option<bool>>>;

/// Whether a node command may be sent.
fn may_command_nodes(agreed: &Agreed) -> bool {
    matches!(agreed.lock().as_deref(), Ok(Some(true)))
}

/// Which attributes of each node the configuration lets the ground set.
///
/// From the `commands` section, in the order it is written: `{ from: moc,
/// sets: thermostat.setpoint }` puts `setpoint` under `thermostat`. A node no
/// binding names is absent, which is most of them.
///
/// Read here rather than asked of the spacecraft: what the ground may set is
/// written in the file both ends read, which is the whole point of writing it
/// there. A binding that names no attribute is left out -- the checking
/// refuses a configuration for one, and this is not the place to guess.
pub fn settable_of(graph: &Graph) -> BTreeMap<String, Vec<String>> {
    let mut settable: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for binding in &graph.commands {
        let target = binding.target();
        if let Some(attribute) = target.input {
            let theirs = settable.entry(target.node).or_default();
            // Once each: two bindings of one attribute, from two moc nodes,
            // are one thing the ground may set.
            if !theirs.contains(&attribute) {
                theirs.push(attribute);
            }
        }
    }
    settable
}

/// One panel per link to the ground, in the order the configuration declares
/// the sources.
///
/// `settable` is what [`settable_of`] found, so that each panel offers what
/// the configuration says may be set on its node and nothing else.
pub fn panels(
    links: &[Telemetered],
    settable: &BTreeMap<String, Vec<String>>,
) -> Vec<DHInfo> {
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
                status: SharedString::from(UNKNOWN_STATUS),
                oc_time: SharedString::from(NO_TRANSFER_TIME),
                oc_data: SharedString::new(),
                packets_recv: 0,
                parameters: SharedString::from(format!(
                    "{} (device id {})\nsends {via}\nthe ground receives at \
                     {}:{}",
                    link.source, link.id, link.ground.address, link.ground.port
                )),
                // What the ground may set on this node: all of them for the
                // line that says so, and the first for the field to start
                // with.
                settable: SharedString::from(
                    settable
                        .get(&link.source)
                        .map(|theirs| theirs.join(", "))
                        .unwrap_or_default(),
                ),
                setting: SharedString::from(
                    settable
                        .get(&link.source)
                        .and_then(|theirs| theirs.first().cloned())
                        .unwrap_or_default(),
                ),
            }
        })
        .collect()
}

/// Whether the simulator file names any payload to simulate, so that tcssim
/// is started only when it has something to be.
///
/// `None` is a payload set with no simulator file beside it, which simulates
/// nothing: there is no file to read, and no simulator to start.
fn anything_simulated(graph: &Graph, sim_path: Option<&str>) -> bool {
    sim_path
        .and_then(|path| SimConfigFile::load(path).ok())
        .and_then(|sim| tcslibgs::graph::simulated::handlers(graph, &sim).ok())
        .is_some_and(|handlers| !handlers.is_empty())
}

/// How long a simulator already running is given to answer before one is
/// started: a simulator that is there answers at once.
const ASK_FIRST: std::time::Duration = std::time::Duration::from_secs(1);

/// How long a simulator this starts is given to make its stand-ins. Built
/// by `cargo run` first, which is most of it.
const WAIT_FOR_SIMULATOR: std::time::Duration =
    std::time::Duration::from_secs(120);

/// Have a simulator ready for this run, where it has anything to simulate.
///
/// Under the flight setting, none: a run that is like a flight then looks
/// like one, with no simulator present to answer for a device that is
/// missing. Otherwise the simulator file's readiness address is asked first.
/// One that answers set up for this run is used and left running when this
/// exits, since this did not start it; one set up for another is refused,
/// saying what differs; and where nothing answers, tcssim is started and
/// waited for. See "Waiting for the Simulator" in docs/design.rst.
fn simulator_for(
    graph: &Graph,
    payload_path: &str,
    sim_path: Option<&str>,
    mode: Mode,
    pm_tcssim: &ProcessManager,
) -> Result<(), String> {
    if mode.is_flight() {
        eprintln!(
            "--simulate={mode}: tcssim is not started, a run like a flight \
             having no stand-ins"
        );
        return Ok(());
    }
    let Some(sim_path) =
        sim_path.filter(|_| anything_simulated(graph, sim_path))
    else {
        match sim_path {
            Some(path) => eprintln!(
                "{path}: nothing to simulate, so tcssim is not started"
            ),
            None => {
                eprintln!(
                    "{payload_path}: no simulator file beside it, so \
                           tcssim is not started"
                )
            }
        }
        return Ok(());
    };
    let sim_file = SimConfigFile::load(sim_path)
        .map_err(|e| format!("{sim_path}: {e}"))?;
    let at = sim_file.ready().map_err(|e| format!("{sim_path}: {e}"))?;
    let ours = SetUpFor {
        configuration: tcslibgs::config_digest::digest_of_graph(graph)
            .map_err(|e| format!("{payload_path}: {e}"))?,
        simulator: tcslibgs::config_digest::digest_of_simulator_file(sim_path)
            .map_err(|e| format!("{sim_path}: {e}"))?,
        mode,
    };
    let [_, tcssim] = CHILDREN;
    match simulator::ask(at, ASK_FIRST).map_err(|e| e.to_string())? {
        Some(answer) => {
            let differences = ours.differences(&answer.set_up_for);
            if !differences.is_empty() {
                return Err(simulator::NotReady::Differs { at, differences }
                    .to_string());
            }
            eprintln!(
                "a simulator is already answering at {at} for this run: \
                       using it, and leaving it running"
            );
            return simulator::wait(at, &ours, WAIT_FOR_SIMULATOR)
                .map_err(|e| e.to_string());
        }
        None => {
            pm_tcssim.start_child(tcssim, payload_path, mode);
            eprintln!("waiting for tcssim to say at {at} that it is ready");
        }
    }

    // Waited for a little at a time, so that a tcssim that refused to start
    // -- and said why, in its own window -- is noticed as that, rather than
    // waited for until the time runs out.
    let started = std::time::Instant::now();
    loop {
        let waited = started.elapsed();
        match simulator::wait(at, &ours, ASK_FIRST) {
            Ok(()) => return Ok(()),
            Err(simulator::NotReady::Silent { .. })
            | Err(simulator::NotReady::StillStarting { .. })
                if pm_tcssim.has_exited() =>
            {
                return Err(format!(
                    "tcssim stopped before it said at {at} that it was ready: \
                     what it said is why"
                ))
            }
            Err(simulator::NotReady::Silent { .. })
                if waited < WAIT_FOR_SIMULATOR => {}
            Err(simulator::NotReady::StillStarting { .. })
                if waited < WAIT_FOR_SIMULATOR => {}
            Err(simulator::NotReady::Silent { at, .. }) => {
                return Err(
                    simulator::NotReady::Silent { at, waited }.to_string()
                )
            }
            Err(simulator::NotReady::StillStarting { at, .. }) => {
                return Err(simulator::NotReady::StillStarting { at, waited }
                    .to_string())
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

/// The two command links the configuration's `tcspecial` section states.
///
/// The spacecraft's -- where this end sends from, `tcs_cmd.send`, and where
/// the vehicle takes them, `tcs_cmd.recv` -- and the one every node is
/// commanded on. Both ends of each from the file, which is what lets the
/// vehicle refuse a command from anywhere else.
///
/// `None` for a configuration with no section, which takes no commands, as it
/// sends no beacons: the section is where the ports are written down.
fn command_links_of(
    graph: &Graph,
) -> Result<Option<(String, String, Commanded)>, String> {
    let Some(links) = tcslib::command_links_of(graph)? else {
        return Ok(None);
    };
    Ok(Some((
        links.from,
        links.interpreter,
        Commanded {
            from: links.nodes_from,
            to: links.nodes_to,
        },
    )))
}

/// Say what the vehicle answered a CONNECT with, and whether it agrees.
///
/// What it read, said the way a beacon says it, so the two can be read
/// against each other, and the verdict kept: a node id means nothing until
/// the two ends have agreed which configuration they are talking about.
fn hear_what_the_vehicle_read(
    ui: &MainWindow,
    link: &Mutex<CiLink>,
    agreed: &Agreed,
    version: tcslibgs::ConfigVersion,
    digest: tcslibgs::ConfigDigest,
) {
    let answer = {
        let mut guard = link.lock().unwrap();
        match guard.client() {
            Some(client) => client.connect(version, digest),
            None => {
                ui.set_last_response(SharedString::from(
                    crate::ci_link::NOT_CONNECTED,
                ));
                return;
            }
        }
    };

    match answer {
        Ok((theirs, their_digest)) => {
            eprintln!(
                "CONNECT answer: {}",
                tcslib::beacon::what_it_read(theirs, their_digest)
            );
            let same = their_digest == digest;
            if let Ok(mut held) = agreed.lock() {
                *held = Some(same);
            }
            let said = if !same {
                format!(
                    "tcspecial is flying a different configuration: md5 \
                     {their_digest}, not {digest} -- so a node id would name a \
                     node of a graph this window has not read, and Enable and \
                     Disable will not send one"
                )
            } else if theirs != version {
                format!(
                    "tcspecial is version {theirs}, this is {version}; it is \
                     flying the configuration this window read, md5 {digest}"
                )
            } else {
                format!("tcspecial agrees: v{version} md5 {digest}")
            };
            if !same || theirs != version {
                eprintln!("{said}");
            }
            ui.set_last_response(SharedString::from(said));
        }
        Err(e) => {
            eprintln!("CONNECT did not get through: {e}");
            if let Ok(mut held) = agreed.lock() {
                *held = None;
            }
            ui.set_last_response(SharedString::from(format!(
                "CONNECT failed: {e}"
            )));
        }
    }
}

/// Ask every panel's node what it has moved, and show it.
///
/// One command per panel rather than one for all of them: a query names a
/// node, and a vehicle that answered for every node at once would be sending
/// the ground counts for nodes nobody asked about. The whole set is asked
/// here because that is what the button means, and what an operator does with
/// it is compare them.
///
/// Each panel takes its node's status from the answer -- running or stopped,
/// which is what the query is for and what the panels cannot otherwise know
/// -- and the counts go in the panel's Configuration popup, beside the rest
/// of what is known about that node.
fn query_every_node(
    ui: &MainWindow,
    link: &Arc<Mutex<CiLink>>,
    agreed: &Agreed,
    model: &impl Model<Data = DHInfo>,
) {
    ask_nodes(ui, link, agreed, model, None);
}

/// Ask the spacecraft what each node is doing, because contact has just been
/// made: on the first beacon, and at the end of an outage.
///
/// Not a poll: nothing is asked while contact simply holds. A node whose last
/// press was Disable is left out, because the operator said to stop it and
/// there is nothing to ask. And where nothing may be asked -- the link is
/// down, or the vehicle has not agreed which configuration it is flying --
/// nothing is, and nothing is said: the panels keep what they show until
/// contact is next made or somebody presses Query all.
fn contact_made(
    ui: &MainWindow,
    link: &Arc<Mutex<CiLink>>,
    agreed: &Agreed,
    model: &impl Model<Data = DHInfo>,
    declined: &crate::Declined,
) {
    let connected = link
        .lock()
        .map(|mut guard| guard.client().is_some())
        .unwrap_or(false);
    if connected && may_command_nodes(agreed) {
        ask_nodes(ui, link, agreed, model, Some(declined));
    }
}

/// After a Connect, ask what [`contact_made`] would have asked, where contact
/// was made before the link was up.
///
/// The first beacon usually arrives before anybody has pressed Connect, and
/// then nothing could be asked: the panels would say Unknown until an outage
/// ended or somebody pressed Query all. A Connect the vehicle agrees to, with
/// the spacecraft being heard from, is the moment the asking was owed. What
/// the Connect said stays in Last Response, with what the asking found after
/// it.
fn connected_in_contact(
    ui: &MainWindow,
    link: &Arc<Mutex<CiLink>>,
    agreed: &Agreed,
    model: &impl Model<Data = DHInfo>,
    declined: &crate::Declined,
    contact: &Mutex<tcslib::beacon::Contact>,
) {
    let heard =
        matches!(contact.lock().as_deref(), Ok(tcslib::beacon::Contact::Made));
    if !heard || !may_command_nodes(agreed) {
        return;
    }
    let connect_said = ui.get_last_response();
    contact_made(ui, link, agreed, model, declined);
    let asked = ui.get_last_response();
    if asked != connect_said {
        ui.set_last_response(SharedString::from(format!(
            "{connect_said}; {asked}"
        )));
    }
}

/// Ask each panel's node what it is doing and has moved, and show it: every
/// node, or with `declined`, every node the operator has not stopped.
fn ask_nodes(
    ui: &MainWindow,
    link: &Arc<Mutex<CiLink>>,
    agreed: &Agreed,
    model: &impl Model<Data = DHInfo>,
    declined: Option<&crate::Declined>,
) {
    if !may_command_nodes(agreed) {
        ui.set_last_response(SharedString::from(NOT_AGREED));
        return;
    }

    let mut said = Vec::new();
    for row in 0..model.row_count() {
        if declined.is_some_and(|declined| was_declined(declined, row)) {
            continue;
        }
        let Some(mut info) = model.row_data(row) else {
            continue;
        };
        let node =
            tcslibgs::NodeId(u32::try_from(info.dev_id).unwrap_or(u32::MAX));
        let answer = {
            let mut guard = link.lock().unwrap();
            match guard.client() {
                Some(client) => client.query_node(node),
                None => {
                    ui.set_last_response(SharedString::from(
                        crate::ci_link::NOT_CONNECTED,
                    ));
                    return;
                }
            }
        };

        match answer {
            // No such node aboard: a tcspecial flying another configuration,
            // which nobody here can speak for.
            Ok((tcslibgs::CommandStatus::NotFound, _, _)) => {
                info.status = SharedString::from(UNKNOWN_STATUS);
                said.push(format!("{}: {}", info.name, info.status));
                model.set_row_data(row, info);
            }
            Ok((status, enabled, counts)) if status.is_success() => {
                info.status = SharedString::from(if enabled {
                    ACTIVE_STATUS
                } else {
                    STOPPED_STATUS
                });
                info.parameters = SharedString::from(format!(
                    "{}\n\nwhat it has moved since the vehicle started\n{}",
                    describes(&info),
                    counted(&counts)
                ));
                said.push(format!("{}: {}", info.name, info.status));
                model.set_row_data(row, info);
            }
            Ok((status, _, _)) => {
                said.push(format!("{}: {status:?}", info.name))
            }
            Err(e) => said.push(format!("{}: no answer ({e})", info.name)),
        }
    }

    ui.set_last_response(SharedString::from(said.join("; ")));
}

/// What a panel says about its node before any query: where its data goes.
///
/// Kept apart from the counts so that a query adds to what the popup said
/// rather than replacing it: both are about the same node, and an operator
/// reading one wants the other beside it.
fn describes(info: &DHInfo) -> String {
    format!("{} (device id {})\n{}", info.name, info.dev_id, info.config)
}

/// What a node has moved, as the popup shows it.
///
/// A column of name and value, in a font of one width, as every other
/// parameter line in this popup is: the lines only line up that way, and what
/// they are read for is the comparison between them.
fn counted(counts: &tcslibgs::NodeCounts) -> String {
    let moved = match counts.last_moved {
        Some(at) => at.time_of_day(),
        // Never, which is not the same as a long time ago: the first has not
        // spoken and the second has gone quiet.
        None => NO_TRANSFER_TIME.to_string(),
    };
    format!(
        "taken          {} bytes in {} reads\n\
         reads failed   {}\n\
         given          {} bytes in {} deliveries\n\
         refused        {}\n\
         dropped        {} (it was stopped)\n\
         asked          {} triggers, {} refused\n\
         strangers      {} (datagrams from elsewhere, turned away)\n\
         last moved     {moved}",
        counts.taken_bytes,
        counts.reads,
        counts.reads_failed,
        counts.given_bytes,
        counts.writes,
        counts.writes_failed,
        counts.dropped,
        counts.asks,
        counts.asks_failed,
        counts.strangers,
    )
}

/// Set an attribute of the node a panel is for, and say how it went.
///
/// `attribute` and `value` are as they were typed. Neither is checked here:
/// the vehicle refuses an attribute no binding mentions and a value the node
/// will not take, and it is the end that has the configuration it is flying.
/// What is checked here is that there is something to send -- an empty field
/// is a press nobody meant.
fn set_a_nodes_attribute(
    ui: &MainWindow,
    link: &Arc<Mutex<CiLink>>,
    agreed: &Agreed,
    model: &Rc<VecModel<DHInfo>>,
    row: usize,
    attribute: &str,
    value: &str,
) {
    let Some(info) = model.row_data(row) else {
        return;
    };
    let node = tcslibgs::NodeId(u32::try_from(info.dev_id).unwrap_or(u32::MAX));
    let name = info.name.to_string();

    if attribute.trim().is_empty() || value.is_empty() {
        ui.set_last_response(SharedString::from(format!(
            "SET_NODE {node} ({name}) needs an attribute and a value"
        )));
        return;
    }

    if !may_command_nodes(agreed) {
        ui.set_last_response(SharedString::from(NOT_AGREED));
        return;
    }

    let answer = {
        let mut guard = link.lock().unwrap();
        match guard.client() {
            Some(client) => {
                client.set_node(node, attribute.trim(), value.as_bytes())
            }
            None => {
                ui.set_last_response(SharedString::from(
                    crate::ci_link::NOT_CONNECTED,
                ));
                return;
            }
        }
    };

    let said = match answer {
        Ok((status, answered, _)) if answered != node => format!(
            "SET_NODE {node} ({name}) was answered about {answered}: the two \
             ends are not reading the same configuration ({status:?})"
        ),
        Ok((status, _, _)) if status.is_success() => {
            format!(
                "SET_NODE {node} ({name}).{} = {value}: {status:?}",
                attribute.trim()
            )
        }
        // An attribute no binding mentions and a value the node will not take
        // are both InvalidParameter, so the status says that it was refused
        // and the reason the vehicle sends says why: which word to change.
        Ok((status, _, reason)) if !reason.is_empty() => format!(
            "SET_NODE {node} ({name}).{} = {value} was refused: {status:?} -- \
             {reason}",
            attribute.trim()
        ),
        // A vehicle that sends no reason: said as the two possibilities,
        // because an operator with the window and no log should still know
        // where to look.
        Ok((status, _, _)) => format!(
            "SET_NODE {node} ({name}).{} = {value} was refused: {status:?} -- \
             no binding names that attribute, or the node will not take that \
             value",
            attribute.trim()
        ),
        Err(e) => format!("SET_NODE {node} ({name}) failed: {e}"),
    };
    eprintln!("{said}");
    ui.set_last_response(SharedString::from(said));
}

/// Send one node command about the node a panel is for, and say how it went.
///
/// `row` is the panel, and the id is the `dev_id` the panel shows -- the
/// node's position in the configuration, which is the number a command
/// carries and the number its telemetry port is counted on. One function for
/// Enable and Disable, because everything but the verb is the same.
///
/// An answer of success is the spacecraft saying what the node now is, so the
/// panel's status takes it, and `declined` keeps the operator's word: Disable
/// leaves the node out of the asking done when contact is made, and Enable
/// puts it back.
fn command_a_node(
    ui: &MainWindow,
    link: &Mutex<CiLink>,
    agreed: &Agreed,
    model: &Rc<VecModel<DHInfo>>,
    declined: &crate::Declined,
    row: usize,
    enable: bool,
) {
    let Some(info) = model.row_data(row) else {
        return;
    };
    let node = tcslibgs::NodeId(u32::try_from(info.dev_id).unwrap_or(u32::MAX));
    let name = info.name.to_string();
    let verb = if enable { "START_NODE" } else { "STOP_NODE" };

    if !may_command_nodes(agreed) {
        ui.set_last_response(SharedString::from(NOT_AGREED));
        return;
    }

    let answer = {
        let mut guard = link.lock().unwrap();
        match guard.client() {
            Some(client) => {
                if enable {
                    client.start_node(node)
                } else {
                    client.stop_node(node)
                }
            }
            None => {
                ui.set_last_response(SharedString::from(
                    crate::ci_link::NOT_CONNECTED,
                ));
                return;
            }
        }
    };

    let said = match answer {
        // The id in the answer as well as the name: the answer carries the id
        // it was asked about, and an id that came back as another is the one
        // thing this could not otherwise notice.
        Ok((status, answered)) if answered != node => format!(
            "{verb} {node} ({name}) was answered about {answered}: the two \
             ends are not reading the same configuration ({status:?})"
        ),
        Ok((status, _)) if status.is_success() => {
            let mut info = info;
            info.status = SharedString::from(if enable {
                ACTIVE_STATUS
            } else {
                STOPPED_STATUS
            });
            model.set_row_data(row, info);
            if let Ok(mut flags) = declined.lock() {
                if let Some(flag) = flags.get_mut(row) {
                    *flag = !enable;
                }
            }
            format!("{verb} {node} ({name}): {status:?}")
        }
        Ok((status, _)) => format!(
            "{verb} {node} ({name}) was refused: \
                                    {status:?}"
        ),
        Err(e) => format!("{verb} {node} ({name}) failed: {e}"),
    };
    eprintln!("{said}");
    ui.set_last_response(SharedString::from(said));
}

/// Wire every control that sends a command.
///
/// Apart from `run` so that it can be driven in a test: a button is pressed
/// through Slint's testing backend, and what is being checked is that the
/// press reaches the vehicle and the window says what came back. Everything
/// here asks the link for a client as it goes rather than holding one, so
/// that a disconnect leaves nothing talking to a socket that is closed: see
/// [`crate::ci_link`].
///
/// `digest` is this end's, or `None` for a configuration that could not be
/// digested -- which leaves nothing to check the vehicle's answer against,
/// and so nothing that may be commanded by id.
///
/// `contact` is the beacon thread's reading of whether the spacecraft is
/// being heard from, which a Connect reads: see [`connected_in_contact`].
pub(crate) fn command_buttons(
    ui: &MainWindow,
    link: &Arc<Mutex<CiLink>>,
    agreed: &Agreed,
    digest: Option<tcslibgs::ConfigDigest>,
    model: &Rc<VecModel<DHInfo>>,
    declined: &crate::Declined,
    contact: &Arc<Mutex<tcslib::beacon::Contact>>,
) {
    // Connect and Disconnect, the one button whose meaning follows the
    // status. Connecting sends a CONNECT at once, because that is what makes
    // a node id mean anything -- see `Agreed`.
    {
        let weak = ui.as_weak();
        let link = link.clone();
        let agreed = agreed.clone();
        let model = model.clone();
        let declined = declined.clone();
        let contact = contact.clone();
        let ours = tcslibgs::config_digest::ConfigVersion::of_this_build();
        ui.on_link_clicked(move || {
            let ui = weak.unwrap();

            if ui.get_ci_status() == CONNECTED_STATUS {
                // Said in terms of where the link was, not of what the
                // address box says, which may have been edited since.
                let address = {
                    let mut guard = link.lock().unwrap();
                    let address = guard.address().to_string();
                    guard.disconnect();
                    address
                };
                // What was agreed was agreed with that vehicle, over that
                // link: a reconnect asks again.
                if let Ok(mut held) = agreed.lock() {
                    *held = None;
                }
                eprintln!("Disconnected from {address}");
                ui.set_ci_status(SharedString::from(DISCONNECTED_STATUS));
                ui.set_last_response(SharedString::from(format!(
                    "Disconnected \
                                                                 from \
                                                                 {address}"
                )));
                return;
            }

            // Where the address box points now, rather than where the link
            // last went: typing an address and pressing Connect is how this
            // window is told to talk to a different vehicle.
            let address = ui.get_ci_address().to_string();
            if let Err(e) = link.lock().unwrap().connect(&address) {
                eprintln!("Failed to connect to {address}: {e}");
                ui.set_ci_status(SharedString::from(ERROR_STATUS));
                ui.set_last_response(SharedString::from(format!(
                    "Connection to {address} failed: {e}"
                )));
                return;
            }
            eprintln!("Connected to {address}");
            ui.set_ci_status(SharedString::from(CONNECTED_STATUS));

            match digest {
                Some(digest) => {
                    hear_what_the_vehicle_read(
                        &ui, &link, &agreed, ours, digest,
                    );
                    connected_in_contact(
                        &ui,
                        &link,
                        &agreed,
                        model.as_ref(),
                        &declined,
                        &contact,
                    );
                }
                // Nothing to compare with, so nothing may be commanded by id:
                // the warning at startup said why.
                None => ui.set_last_response(SharedString::from(
                    "Connected, and this configuration could not be digested, \
                     so there is nothing to check the vehicle's answer \
                     against: Enable and Disable will not send a node id",
                )),
            }
        });
    }

    // Ping, which says this link reaches the vehicle -- a different fact from
    // the beacon's, which says the vehicle is alive.
    {
        let weak = ui.as_weak();
        let link = link.clone();
        ui.on_ping_clicked(move || {
            let ui = weak.unwrap();
            let answer =
                link.lock().unwrap().client().map(|client| client.ping());
            let said = match answer {
                Some(Ok(tm)) => {
                    format!("PING OK at {}", tm.header.timestamp.time_of_day())
                }
                Some(Err(e)) => format!("PING failed: {e}"),
                None => crate::ci_link::NOT_CONNECTED.to_string(),
            };
            ui.set_last_response(SharedString::from(said));
        });
    }

    // Query, which asks every node what it has moved. Both the callback the
    // window declares for it and the menu item that is the only way to reach
    // it: `query-all-clicked` is declared and nothing in the window invokes
    // it, so the menu is where an operator presses this.
    {
        let weak = ui.as_weak();
        let link = link.clone();
        let agreed = agreed.clone();
        let model = model.clone();
        ui.on_query_all_clicked(move || {
            if let Some(ui) = weak.upgrade() {
                query_every_node(&ui, &link, &agreed, &model);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let link = link.clone();
        let agreed = agreed.clone();
        let model = model.clone();
        ui.on_menu_action(move |action| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            match action {
                // The two the menu can do something about. Each is the same
                // command the button beside it sends, by the same function:
                // one place decides what a press means.
                MenuAction::Query => {
                    query_every_node(&ui, &link, &agreed, &model)
                }
                MenuAction::Ping => {
                    let answer = link
                        .lock()
                        .unwrap()
                        .client()
                        .map(|client| client.ping());
                    ui.set_last_response(SharedString::from(match answer {
                        Some(Ok(tm)) => {
                            format!(
                                "PING OK at {}",
                                tm.header.timestamp.time_of_day()
                            )
                        }
                        Some(Err(e)) => format!("PING failed: {e}"),
                        None => crate::ci_link::NOT_CONNECTED.to_string(),
                    }));
                }
                // The rest are not answered: the vehicle does not answer a
                // restart yet, and the three DH items name a data
                // handler where a graph has nodes -- which the panels'
                // buttons command.
                other => ui.set_last_response(SharedString::from(format!(
                    "{other:?} is not answered by the vehicle: a graph has \
                     nodes, which each panel's buttons start, stop and set"
                ))),
            }
        });
    }

    // And each panel's two, about the node that panel is for.
    {
        let weak = ui.as_weak();
        let link = link.clone();
        let agreed = agreed.clone();
        let model = model.clone();
        let declined = declined.clone();
        ui.on_enable_dh(move |row| {
            if let Some(ui) = weak.upgrade() {
                command_a_node(
                    &ui,
                    &link,
                    &agreed,
                    &model,
                    &declined,
                    row as usize,
                    true,
                );
            }
        });
    }
    {
        let weak = ui.as_weak();
        let link = link.clone();
        let agreed = agreed.clone();
        let model = model.clone();
        let declined = declined.clone();
        ui.on_disable_dh(move |row| {
            if let Some(ui) = weak.upgrade() {
                command_a_node(
                    &ui,
                    &link,
                    &agreed,
                    &model,
                    &declined,
                    row as usize,
                    false,
                );
            }
        });
    }
    {
        let weak = ui.as_weak();
        let link = link.clone();
        let agreed = agreed.clone();
        let model = model.clone();
        ui.on_set_dh(move |row, attribute, value| {
            if let Some(ui) = weak.upgrade() {
                set_a_nodes_attribute(
                    &ui,
                    &link,
                    &agreed,
                    &model,
                    row as usize,
                    attribute.as_str(),
                    value.as_str(),
                );
            }
        });
    }
}

/// Say, from every control, that this configuration names no command link.
///
/// A configuration with no `tcspecial` section states no ports, so there is
/// nowhere to send a command: the controls say that rather than doing
/// nothing, which an operator reads as a window that has stopped working.
fn no_command_link(ui: &MainWindow) {
    const NO_LINK: &str =
        "This configuration states no tcspecial section, so it says where \
         no command link is: nothing can be started or stopped from here. The \
         panels show what each source sends to the ground.";

    ui.set_ci_status(SharedString::from("None"));
    ui.set_last_response(SharedString::from(NO_LINK));
    for wire in [0, 1, 2] {
        let weak = ui.as_weak();
        match wire {
            0 => ui.on_link_clicked(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.set_last_response(SharedString::from(NO_LINK));
                }
            }),
            1 => ui.on_ping_clicked(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.set_last_response(SharedString::from(NO_LINK));
                }
            }),
            _ => ui.on_query_all_clicked(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.set_last_response(SharedString::from(NO_LINK));
                }
            }),
        }
    }
    let weak = ui.as_weak();
    ui.on_enable_dh(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.set_last_response(SharedString::from(NO_LINK));
        }
    });
    let weak = ui.as_weak();
    ui.on_disable_dh(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.set_last_response(SharedString::from(NO_LINK));
        }
    });
    let weak = ui.as_weak();
    ui.on_set_dh(move |_, _, _| {
        if let Some(ui) = weak.upgrade() {
            ui.set_last_response(SharedString::from(NO_LINK));
        }
    });
    let weak = ui.as_weak();
    ui.on_menu_action(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.set_last_response(SharedString::from(NO_LINK));
        }
    });
}

/// The ping node's answers, on their way to the pinger and its window.
///
/// A ping node is a source like any other, so its answers arrive at its `moc`
/// node with the rest and are shown in its panel. They are also the answers
/// to the pings sent from the ping window, so the receiver hands each one to
/// the pinger as well, which matches it to its ping and times it.
///
/// The receiver does no more than that and say so: the window is shown what
/// the pinger keeps from the window's own thread, on its own timer -- see
/// `show_answers`.
#[derive(Clone)]
struct PingFeed {
    node: String,
    pinger: Arc<Mutex<Pinger>>,
    window: slint::Weak<PingWindow>,
    /// Whether something has arrived from the ping node that the window has
    /// not yet been shown.
    fresh: Arc<AtomicBool>,
}

/// Hand what the ping node sent down to the pinger, and say the window has
/// something to show. Allocates nothing: see `tcslib::ping::Pinger`.
fn take_answer(feed: &PingFeed, data: &[u8], at: std::time::Duration) {
    // A poisoned lock is a pinger nobody can use, and this answer goes with
    // it.
    if let Ok(mut pinger) = feed.pinger.lock() {
        pinger.answered(data, at);
        feed.fresh.store(true, Ordering::Release);
    }
}

/// How often the ping window looks for an answer to show. Short, because the
/// answer to a ping somebody has just sent is what they are watching for;
/// cheap, because most looks find nothing and do nothing.
const PING_REFRESH_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(100);

/// Show the ping window what the pinger keeps, whenever something new has
/// arrived, from a timer on the window's thread.
///
/// Allocates, for each answer shown: a window shows text, and what it is
/// shown is made on its thread. The receiver, which is the loop that runs for
/// as long as the MOC does, makes none of it.
fn show_answers(feed: PingFeed) -> slint::Timer {
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        PING_REFRESH_INTERVAL,
        move || {
            if !feed.fresh.swap(false, Ordering::Acquire) {
                return;
            }
            let Some(window) = feed.window.upgrade() else {
                return;
            };
            if let Ok(pinger) = feed.pinger.lock() {
                show_ping(&window, &PingShown::of(pinger.exchange()));
            }
        },
    );
    timer
}

/// Put what the pinger keeps into the ping window.
fn show_ping(window: &PingWindow, shown: &PingShown) {
    window.set_sent(shown.sent.clone().into());
    window.set_sent_time(shown.sent_time.clone().into());
    window.set_received(shown.received.clone().into());
    window.set_received_time(shown.received_time.clone().into());
    window.set_round_trip(shown.round_trip.clone().into());
    window.set_up(shown.up.clone().into());
    window.set_down(shown.down.clone().into());
}

/// Keep what arrives at one `moc` node, in the panel of the source it came
/// from -- and hand the ping node's answers to the pinger, where this is the
/// `moc` node they come down through.
fn serve(
    mut moc: Moc,
    rows: Arc<BTreeMap<String, usize>>,
    sends_values: Arc<BTreeSet<String>>,
    arrivals: Arrivals,
    pings: Option<PingFeed>,
) {
    // What each source is to this receiver, by its place in the moc node's
    // sources, worked out before the loop: a datum says which source sent it
    // by that place, so the loop looks nothing up by name.
    struct Source {
        /// The panel it is shown in, if it has one.
        row: Option<usize>,
        sends_values: bool,
        /// Whether it is the ping node whose answers the pinger times.
        pings: bool,
        /// How many have arrived from it since the MOC started.
        packets: u64,
    }
    let mut sources: Vec<Source> = moc
        .sources()
        .iter()
        .map(|&source| Source {
            row: rows.get(source).copied(),
            sends_values: sends_values.contains(source),
            pings: pings.as_ref().is_some_and(|feed| feed.node == source),
            packets: 0,
        })
        .collect();
    let mut datum = vec![0u8; DATUM_BYTES];
    loop {
        let arrival = match moc.receive_into(None, &mut datum) {
            Ok(Some(arrival)) => arrival,
            Ok(None) => continue,
            Err(e) => {
                eprintln!("{e}");
                continue;
            }
        };
        // Held: which node sent it, and in what format, is not known, so
        // no panel is given it and no ping answer is timed by it. The window
        // says data is held, beside the beacon.
        if arrival.held {
            continue;
        }
        let data = &datum[..arrival.length];
        let Some(source) = sources.get_mut(arrival.source) else {
            continue;
        };
        if let Some(feed) = pings.as_ref().filter(|_| source.pings) {
            // The ground's clock, now: a datum is timed as it is taken.
            if let Ok(at) = tcslibgs::node::vehicle::ping::now() {
                take_answer(feed, data, at);
            }
        }
        let Some(row) = source.row else {
            continue;
        };
        source.packets += 1;
        let mut sample = DHSample::new();
        sample.record(data);
        if let Ok(mut slots) = arrivals.lock() {
            if let Some(slot) = slots.get_mut(row) {
                *slot = Some(Arrival {
                    at: Timestamp::now(),
                    data: sample,
                    packets: source.packets,
                    sends_values: source.sends_values,
                });
            }
        }
    }
}

/// Set the ping window up, where the configuration has a ping node the
/// ground can reach both ways: the Ping node button that opens it, and its
/// Send ping, which pings the node through its `moc` node's uplink.
///
/// The `moc` node the answers come down through, and what its receiver hands
/// them on to; `None` where there is nothing to ping, and an error where there
/// is a ping node and no way to send to it.
fn ping_node(
    ui: &MainWindow,
    ping_window: &PingWindow,
    graph: &Graph,
) -> Result<Option<(String, PingFeed)>, String> {
    let Some(route) = tcslib::ping::route_of(graph) else {
        return Ok(None);
    };
    let uplink = Uplink::open(graph, &route.moc)
        .map_err(|e| format!("{} cannot be pinged: {e}", route.node))?;
    eprintln!(
        "pinging {} through {} at {}",
        route.node,
        route.moc,
        uplink.to()
    );
    let pinger = Arc::new(Mutex::new(Pinger::new()));
    ping_window.set_node(SharedString::from(route.node.as_str()));
    ui.set_can_ping_node(true);
    {
        let window = ping_window.as_weak();
        ui.on_ping_node_clicked(move || {
            if let Some(window) = window.upgrade() {
                let _ = window.show();
            }
        });
    }
    {
        let pinger = pinger.clone();
        let window = ping_window.as_weak();
        ping_window.on_send_clicked(move || {
            let Some(window) = window.upgrade() else {
                return;
            };
            let sent = tcslibgs::node::vehicle::ping::now().and_then(|at| {
                let mut pinger =
                    pinger.lock().map_err(|_| "the pinger".to_string())?;
                let message = pinger.ping(at)?;
                uplink.send(message.as_bytes())?;
                Ok(PingShown::of(pinger.exchange()))
            });
            match sent {
                Ok(shown) => show_ping(&window, &shown),
                Err(why) => eprintln!("ping: {why}"),
            }
        });
    }
    Ok(Some((
        route.moc,
        PingFeed {
            node: route.node,
            pinger,
            window: ping_window.as_weak(),
            fresh: Arc::new(AtomicBool::new(false)),
        },
    )))
}

/// Run the MOC on a configuration. Does not return.
pub fn run(
    payload_path: &str,
    sim_path: Option<&str>,
    mode: Mode,
    mut startup: Startup,
) -> ! {
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
            "{payload_path}: nothing in it sends to the ground, so there is \
             nothing for the panels to show"
        ));
    }

    let ui = MainWindow::new().unwrap();
    crate::remember_main_window(&ui);
    let _ = Sizes::settle(&ui);

    let rows_in_order = panels(&links, &settable_of(&graph));
    let rows: Arc<BTreeMap<String, usize>> = Arc::new(
        rows_in_order
            .iter()
            .enumerate()
            .map(|(row, info)| (info.name.to_string(), row))
            .collect(),
    );
    // Which sources send values, asked of the configuration once: a panel
    // then shows a reading as a temperature and everything else as bytes.
    // See `tcslibgs::node::vehicle::compute::node_sends_a_value` for why this
    // is the configuration's answer rather than a guess from the data.
    let sends_values: Arc<BTreeSet<String>> = Arc::new(
        graph
            .nodes
            .iter()
            .filter(|node| {
                tcslibgs::node::vehicle::compute::node_sends_a_value(
                    &node.node_type,
                    &node.attributes,
                )
            })
            .map(|node| node.name.clone())
            .collect(),
    );

    let count = rows_in_order.len();
    let model: Rc<VecModel<DHInfo>> = Rc::new(VecModel::from(rows_in_order));
    ui.set_dh_model(ModelRc::from(model.clone()));

    let sizes = Sizes::of(&ui);
    let shape = grid_shape(&sizes, count);
    ui.window().set_size(window_size(&sizes, &shape, count));
    eprintln!("{payload_path}: {count} source(s) sending to the ground");

    // The two command links, both ends of each from the file. A configuration
    // with no `tcspecial` section states no ports, so there is nothing to
    // connect to and the buttons say so.
    let (link, vehicle_at) = match command_links_of(&graph) {
        Ok(Some((from, to, commanded))) => {
            eprintln!(
                "Commands from {from} to {to}, nodes on {}",
                commanded.to
            );
            (
                Some(Arc::new(Mutex::new(CiLink::down_commanding(
                    &from, commanded,
                )))),
                to,
            )
        }
        Ok(None) => {
            startup.warning(format!(
                "{payload_path} states no tcspecial section, so it says where \
                 no command link is: nothing can be started or stopped from \
                 here"
            ));
            (None, String::new())
        }
        Err(e) => {
            startup.warning(format!("No command link: {e}"));
            (None, String::new())
        }
    };

    ui.set_ci_status(SharedString::from(DISCONNECTED_STATUS));
    ui.set_ci_address(SharedString::from(vehicle_at));
    ui.set_last_response(SharedString::from(
        "Each panel is a node. Press Connect to command them",
    ));

    // Whether the vehicle has agreed which configuration it is flying, which
    // is what licenses a node id: see `Agreed`.
    let agreed: Agreed = Arc::new(Mutex::new(None));
    // Which nodes the operator has stopped, which the asking done when
    // contact is made leaves out.
    let declined = declined_for(rows.len());

    // The simulator, where there is anything to simulate, ready before the
    // vehicle starts: tcspecial judges each payload by what is at its
    // endpoint before it opens anything, and a stand-in not made yet is one
    // it does not find. Then the vehicle, which has no command interpreter to
    // ask whether it is already running, so one is started.
    let [tcspecial, _] = CHILDREN;
    let pm_tcssim = Arc::new(ProcessManager::new());
    if let Err(e) =
        simulator_for(&graph, payload_path, sim_path, mode, &pm_tcssim)
    {
        // A simulator this started and that never became ready is not left
        // holding the payloads' paths.
        pm_tcssim.kill();
        startup.error(e);
        stop(&startup);
    }
    let pm_tcspecial = Arc::new(ProcessManager::new());
    pm_tcspecial.start_child(tcspecial, payload_path, mode);

    // Whether the vehicle is serving the configuration this MOC read. This
    // MOC digests the configuration files it read, with the function
    // tcspecial digests the files it read with; tcspecial sends its digest in
    // every beacon, and a beacon whose digest differs asks the operator
    // whether to continue anyway -- the panels say where each source's data
    // arrives, and a different configuration may send it somewhere else.
    let digest = match tcslibgs::config_digest::digest_of_graph(&graph) {
        Ok(digest) => {
            eprintln!(
                "tcsmoc v{} md5 {digest}",
                tcslibgs::config_digest::ConfigVersion::of_this_build()
            );
            Some(digest)
        }
        Err(e) => {
            startup.warning(format!(
                "{payload_path} cannot be digested, so tcspecial's beacons \
                 cannot be checked against it: {e}"
            ));
            None
        }
    };
    let contact = Arc::new(Mutex::new(tcslib::beacon::Contact::Lost));
    // The interval the configuration sends beacons at, which is what says how
    // late one is; a configuration whose interval cannot be read gets the
    // standard schedule, and says so.
    let interval = match tcslibgs::graph::parameters::beacon_interval_of(&graph)
    {
        Ok(interval) => interval.unwrap_or(tcslib::beacon::STANDARD_INTERVAL),
        Err(e) => {
            startup.warning(format!(
                "The beacon interval cannot be read, so the beacon indicator \
                 ages as though it were {:?}: {e}",
                tcslib::beacon::STANDARD_INTERVAL
            ));
            tcslib::beacon::STANDARD_INTERVAL
        }
    };
    // Whether data from the vehicle is held, which the beacons decide and
    // every receiver asks: one for the window, shared by them all. Path
    // purged is the one thing that ends it, and it says why where it cannot.
    let data_hold = tcslib::moc::DataHold::new();
    {
        let hold = data_hold.clone();
        let weak = ui.as_weak();
        ui.on_path_purged(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            match hold.purged() {
                Ok(()) => {
                    eprintln!("the path is purged: data is attributed again");
                    crate::beacon_receive::show_hold(&ui, None);
                }
                Err(why) => {
                    eprintln!("{why}");
                    ui.set_data_held_why(SharedString::from(why));
                }
            }
        });
    }
    let _beacon_receive = match tcslibgs::graph::parameters::beacon_of(&graph) {
        Ok(Some(beacon)) => {
            eprintln!(
                "Listening for beacons at {}, which {} sends from",
                beacon.recv, beacon.send
            );
            BeaconReceive::new(
                ui.as_weak(),
                beacon,
                crate::config::constants::beacon_schedule(interval),
                crate::beacon_receive::Decided {
                    contact: contact.clone(),
                    hold: data_hold.clone(),
                },
                digest,
                Some(crate::ask_whether_to_continue(
                    pm_tcssim.clone(),
                    Some(pm_tcspecial.clone()),
                )),
                // What the nodes do as contact changes, each on the window's
                // thread, which is the only one that may touch the panels.
                crate::beacon_receive::Contacts {
                    // Contact lost: every node's status is stale.
                    lost: Some({
                        let weak = ui.as_weak();
                        Arc::new(move || {
                            let weak = weak.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = weak.upgrade() {
                                    every_node_lost_contact(&ui.get_dh_model());
                                }
                            });
                        })
                    }),
                    // Contact made: the spacecraft is asked about every node
                    // the operator has not stopped, where it can be asked.
                    made: link.clone().map(|link| {
                        let weak = ui.as_weak();
                        let agreed = agreed.clone();
                        let declined = declined.clone();
                        Arc::new(move || {
                            let weak = weak.clone();
                            let link = link.clone();
                            let agreed = agreed.clone();
                            let declined = declined.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = weak.upgrade() {
                                    contact_made(
                                        &ui,
                                        &link,
                                        &agreed,
                                        &ui.get_dh_model(),
                                        &declined,
                                    );
                                }
                            });
                        })
                            as crate::beacon_receive::OnContact
                    }),
                },
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
    // The ping node, where there is one the ground can reach both ways: a
    // window of its own, opened by Ping node, that pings it and times the
    // answer. See `tcslib::ping`.
    let ping_window = PingWindow::new().unwrap();
    let pings = match ping_node(&ui, &ping_window, &graph) {
        Ok(pings) => pings,
        Err(e) => {
            startup.warning(e);
            None
        }
    };

    // Kept for as long as the window runs: a timer dropped is a timer
    // stopped.
    let _answers = pings.as_ref().map(|(_, feed)| show_answers(feed.clone()));

    let arrivals: Arrivals = Arc::new(Mutex::new(vec![None; count]));
    let mocs: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|n| tcslibgs::gateway::is_gateway(&n.node_type))
        .map(|n| n.name.as_str())
        .filter(|name| {
            links.iter().any(|l| l.via == Via::Moc(name.to_string()))
        })
        .collect();
    let mut receivers = Vec::new();
    for name in mocs {
        match Moc::open(&graph, name) {
            Ok(mut moc) => {
                moc.share(&data_hold);
                receivers.push(moc);
            }
            Err(e) => startup.warning(e),
        }
    }
    match Moc::open_payload_telemetry(&graph) {
        Ok(Some(mut moc)) => {
            moc.share(&data_hold);
            receivers.push(moc);
        }
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
        let sends_values = sends_values.clone();
        let arrivals = arrivals.clone();
        let feed = pings
            .as_ref()
            .filter(|(through, _)| through == moc.name())
            .map(|(_, feed)| feed.clone());
        thread::spawn(move || serve(moc, rows, sends_values, arrivals, feed));
    }

    // The panels, kept current -- and the one thing that keeps running on the
    // event loop whatever else the window is doing, which is where a signal
    // asking the MOC to stop is noticed: see `crate::stop_if_asked`.
    crate::listen_for_stop();
    let timer = slint::Timer::default();
    {
        let model = model.clone();
        let arrivals = arrivals.clone();
        let stopping = (pm_tcssim.clone(), Some(pm_tcspecial.clone()));
        let times = link.clone();
        let ui_weak = ui.as_weak();
        timer.start(
            slint::TimerMode::Repeated,
            PANEL_REFRESH_INTERVAL,
            move || {
                crate::stop_if_asked(&stopping.0, stopping.1.as_ref());
                show_arrivals(&model, &arrivals);

                // When the last command went out and when its answer came back,
                // which the window shows under the link's own status.
                //
                // try_lock rather than lock: the link belongs to the buttons,
                // and a window that waited for it here would stop repainting
                // exactly while a command was in flight -- which is when an
                // operator is watching it. A pass that cannot have it leaves
                // the times as they are, and the next one is a second away.
                let Some(times) = times.as_ref() else { return };
                if let (Some(ui), Ok(link)) =
                    (ui_weak.upgrade(), times.try_lock())
                {
                    let (sent, received) =
                        link.command_times().unwrap_or((None, None));
                    ui.set_cmd_sent_time(SharedString::from(
                        crate::shown_time(sent),
                    ));
                    ui.set_cmd_recv_time(SharedString::from(
                        crate::shown_time(received),
                    ));
                }
            },
        );
    }

    // The buttons. A configuration with no command link gets handlers that
    // say so, rather than buttons that do nothing when pressed.
    let Some(link) = link else {
        no_command_link(&ui);
        handle_quit(&ui, pm_tcssim.clone(), Some(pm_tcspecial.clone()));
        handle_close(&ui, pm_tcssim, Some(pm_tcspecial));
        if !startup.is_empty() {
            present(&startup);
        }
        ask_for_the_edge(&ui, Edge::Top);
        ui.run().unwrap();
        std::process::exit(0);
    };

    command_buttons(&ui, &link, &agreed, digest, &model, &declined, &contact);

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
pub(crate) mod tests {
    use super::*;
    use std::path::Path;

    /// An answer from the ping node reaches the pinger, timed, and the
    /// window is told there is something to show -- by a flag, the receiver
    /// making nothing for the window itself.
    #[test]
    fn a_ping_answer_reaches_the_pinger_and_the_window_is_told() {
        use tcslibgs::node::vehicle::ping;

        let port = std::net::UdpSocket::bind("127.0.0.1:0")
            .and_then(|socket| socket.local_addr())
            .expect("a free port")
            .port();
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("vehicle.yaml");
        // The ping node is the second node written, device id 1, so its
        // answers arrive at ground_base plus one: `port`.
        std::fs::write(
            &path,
            format!(
                "moc:
  - {{ name: ground, type: moc, ground_base: \"127.0.0.1:{}\" }}
vehicle:
  - {{ name: pinger, type: ping, payload-out: ground }}
",
                port - 1
            ),
        )
        .expect("written");
        let graph = Graph::load(&path).expect("it reads");
        let Ok(moc) = Moc::open(&graph, "ground") else {
            // The port was taken between being chosen and being bound.
            return;
        };

        let pinger = Arc::new(Mutex::new(Pinger::new()));
        let at = std::time::Duration::new(1_760_900_000, 0);
        let sent = pinger
            .lock()
            .expect("the pinger")
            .ping(at)
            .expect("a ping")
            .to_string();
        let feed = PingFeed {
            node: "pinger".to_string(),
            pinger: Arc::clone(&pinger),
            window: slint::Weak::default(),
            fresh: Arc::new(AtomicBool::new(false)),
        };
        let fresh = Arc::clone(&feed.fresh);
        let rows = Arc::new(BTreeMap::from([("pinger".to_string(), 0)]));
        let arrivals: Arrivals = Arc::new(Mutex::new(vec![None]));
        thread::spawn(move || {
            serve(moc, rows, Arc::new(BTreeSet::new()), arrivals, Some(feed))
        });

        let answer = ping::reply(sent.as_bytes(), at).expect("answered");
        let vehicle = std::net::UdpSocket::bind("127.0.0.1:0")
            .expect("the vehicle's socket");
        vehicle
            .send_to(answer.as_bytes(), ("127.0.0.1", port))
            .expect("sent");

        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !fresh.load(Ordering::Acquire) {
            assert!(
                std::time::Instant::now() < deadline,
                "the window was never told of the answer"
            );
            thread::sleep(std::time::Duration::from_millis(10));
        }
        let pinger = pinger.lock().expect("the pinger");
        assert!(
            pinger.exchange().received_at.is_some(),
            "the answer should be timed: {:?}",
            pinger.exchange()
        );
    }

    /// What arrives at a moc node is put in the panel of the source that
    /// sent it, counted, and marked as values where that source sends them;
    /// a source with no panel is passed over.
    #[test]
    fn each_datum_goes_to_its_sources_panel() {
        let port = std::net::UdpSocket::bind("127.0.0.1:0")
            .and_then(|socket| socket.local_addr())
            .expect("a free port")
            .port();
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("vehicle.yaml");
        std::fs::write(
            &path,
            format!(
                "moc:
  - {{ name: ground, type: moc, ground_base: \"127.0.0.1:{}\" }}
payloads:
  - {{ name: a, type: network, protocol: udp, address: \"127.0.0.1\", \
    port: \"7001\", payload-out: ground }}
  - {{ name: b, type: network, protocol: udp, address: \"127.0.0.1\", \
    port: \"7002\", payload-out: ground }}
",
                port - 1
            ),
        )
        .expect("written");
        let graph = Graph::load(&path).expect("it reads");
        let Ok(moc) = Moc::open(&graph, "ground") else {
            // The port was taken between being chosen and being bound.
            return;
        };

        // b has the second panel and sends values; a has no panel.
        let rows = Arc::new(BTreeMap::from([("b".to_string(), 1)]));
        let sends_values = Arc::new(BTreeSet::from(["b".to_string()]));
        let arrivals: Arrivals = Arc::new(Mutex::new(vec![None, None]));
        {
            let arrivals = arrivals.clone();
            thread::spawn(move || {
                serve(moc, rows, sends_values, arrivals, None)
            });
        }

        let vehicle = std::net::UdpSocket::bind("127.0.0.1:0")
            .expect("the vehicle's socket");
        vehicle
            .send_to(b"from a", ("127.0.0.1", port))
            .expect("sent");
        for _ in 0..2 {
            vehicle
                .send_to(b"293.15", ("127.0.0.1", port + 1))
                .expect("sent");
        }

        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let slots = arrivals.lock().expect("the slots");
            if let Some(arrival) = &slots[1] {
                if arrival.packets == 2 {
                    assert_eq!(
                        &arrival.data.bytes[..arrival.data.len as usize],
                        b"293.15"
                    );
                    assert!(arrival.sends_values);
                    assert!(
                        slots[0].is_none(),
                        "a has no panel, and filled one"
                    );
                    break;
                }
            }
            drop(slots);
            assert!(
                std::time::Instant::now() < deadline,
                "b's two data did not reach its panel"
            );
            thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// When contact is lost every node's status is stale, whatever each said
    /// before: what the spacecraft answered while it was heard from is not
    /// known to be true now.
    #[test]
    fn losing_contact_makes_every_status_stale() {
        let rows: Vec<DHInfo> = [ACTIVE_STATUS, STOPPED_STATUS, UNKNOWN_STATUS]
            .into_iter()
            .map(|status| DHInfo {
                status: SharedString::from(status),
                ..Default::default()
            })
            .collect();
        let model = ModelRc::from(Rc::new(VecModel::from(rows)));

        every_node_lost_contact(&model);

        let statuses: Vec<String> =
            model.iter().map(|info| info.status.to_string()).collect();
        assert_eq!(statuses, [STALE_STATUS; 3]);
    }

    fn example(name: &str) -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        Graph::load(root.join(format!("examples/{name}/{name}.yaml")))
            .expect("the example reads")
    }

    /// One panel per source, in device id order, saying where its data
    /// arrives.
    #[test]
    fn each_source_has_a_panel_in_device_id_order() {
        let graph = example("thermostat");
        let links = telemetry::of(&graph).expect("its links");
        let panels = panels(&links, &settable_of(&graph));
        let named: Vec<(String, i32)> = panels
            .iter()
            .map(|p| (p.name.to_string(), p.dev_id))
            .collect();
        assert_eq!(
            named,
            vec![
                ("k_1".to_string(), 1),
                ("k_2".to_string(), 2),
                ("k_3".to_string(), 3),
                ("cabin_temp".to_string(), 4),
                // The decision too, which the ground sees as well as the
                // heaters.
                ("thermostat".to_string(), 5),
            ]
        );
        assert!(panels[3].config.contains("to moc"), "{}", panels[3].config);
        assert!(panels[3].config.contains("5104"), "{}", panels[3].config);
    }

    /// The minimal example's sensor reports over payload_telemetry.
    #[test]
    fn an_untouched_payload_has_a_panel_too() {
        let graph = example("minimal");
        let links = telemetry::of(&graph).expect("its links");
        let panels = panels(&links, &settable_of(&graph));
        assert_eq!(panels.len(), 1);
        assert_eq!(panels[0].name.as_str(), "sensor");
        assert!(
            panels[0].config.contains("payload_telemetry"),
            "{}",
            panels[0].config
        );
    }

    /// The two command links come from the file, both ends of each.
    ///
    /// Both ends because the vehicle checks the one it receives on: a command
    /// from anywhere but where the configuration says the ground sends from
    /// is ignored, so a window that chose its own source port would be
    /// ignored in silence.
    #[test]
    fn the_command_links_are_the_ones_the_file_states() {
        let (from, to, commanded) = command_links_of(&example("simple-app"))
            .expect("the section reads")
            .expect("it has one");

        // tcs_cmd.send and tcs_cmd.recv: this end sends from the first and
        // the vehicle takes them on the second.
        assert_eq!(from, "127.0.0.1:4100");
        assert_eq!(to, "0.0.0.0:4000");

        // And one link for every node, not one per node: the base itself,
        // with nothing added to it, because the id is in the message.
        let Commanded { from, to } = commanded;
        assert_eq!(from.to_string(), "127.0.0.1:4300");
        assert_eq!(to.to_string(), "0.0.0.0:4200");
    }

    /// A panel offers what the configuration says may be set on its node, and
    /// nothing else.
    ///
    /// From the `commands` section and not from the spacecraft: what the
    /// ground may set is written in the file both ends read. The thermostat
    /// example binds one attribute of one node, so one of its four panels
    /// offers a field and the other three say they offer none.
    #[test]
    fn a_panel_offers_what_its_node_binds() {
        let graph = example("thermostat");
        let settable = settable_of(&graph);
        assert_eq!(
            settable.get("thermostat").map(Vec::as_slice),
            Some(&["setpoint".to_string()][..]),
            "the example binds the thermostat's setpoint and nothing else"
        );

        let shown =
            panels(&telemetry::of(&graph).expect("its links"), &settable);
        // The thermostat is one of the panels, its decision reaching the
        // ground as well as the heaters, and it is the one that binds
        // something: its panel offers the setpoint and the others offer
        // nothing.
        for panel in &shown {
            let offered = match panel.name.as_str() {
                "thermostat" => ("setpoint", "setpoint"),
                _ => ("", ""),
            };
            assert_eq!(
                (panel.settable.as_str(), panel.setting.as_str()),
                offered,
                "what {} offers",
                panel.name
            );
        }

        // And a source that does bind something offers it, with the first of
        // them in the field: one binding is the ordinary case, and then
        // nothing has to be typed but the value.
        let mut both = settable.clone();
        both.insert(
            shown[0].name.to_string(),
            vec!["setpoint".to_string(), "band".to_string()],
        );
        let offered = panels(&telemetry::of(&graph).expect("its links"), &both);
        assert_eq!(offered[0].settable.as_str(), "setpoint, band");
        assert_eq!(offered[0].setting.as_str(), "setpoint");
    }

    /// A node command waits for the two ends to agree what they read.
    ///
    /// A node id is a position in a configuration, so an id sent to a vehicle
    /// flying a different one names whatever is in that position there --
    /// which is a node the operator did not choose. Nothing is sent until a
    /// CONNECT has agreed, and a connect that failed or disagreed leaves it
    /// refused.
    #[test]
    fn a_node_is_not_commanded_until_the_two_ends_agree() {
        let agreed: Agreed = Arc::new(Mutex::new(None));
        assert!(
            !may_command_nodes(&agreed),
            "nothing has agreed anything yet"
        );

        *agreed.lock().unwrap() = Some(false);
        assert!(
            !may_command_nodes(&agreed),
            "the vehicle is flying another configuration"
        );

        *agreed.lock().unwrap() = Some(true);
        assert!(may_command_nodes(&agreed));
    }
    /// Send ping, pressed, pings the ping node through its moc node, and the
    /// node's answer, taken as the receiver takes it, is timed in the window.
    ///
    /// Run from `main.rs`'s window test, which owns the testing backend. The
    /// vehicle is a socket this test holds where the moc node binds, so what
    /// is checked is the press and the answer and not tcspecial.
    pub(crate) fn pressing_send_ping_pings_the_node_and_times_its_answer() {
        use std::net::UdpSocket;
        use tcslibgs::node::vehicle::ping;

        let vehicle = UdpSocket::bind("127.0.0.1:0").expect("a stub socket");
        vehicle
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .expect("a timeout");
        let port = vehicle.local_addr().expect("where it is").port();
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("ping.yaml");
        std::fs::write(
            &path,
            format!(
                "moc:\n  - name: moc\n    type: moc\n    address: \
                 \"127.0.0.1\"\n    port: \"{port}\"\n    ground_base: \
                 \"127.0.0.1:9100\"\n    payload-in: true\n    moc-out:\n      \
                 - target: ping\nvehicle:\n  - name: ping\n    type: ping\n    \
                 moc-in: true\n    payload-out:\n      - target: moc\n"
            ),
        )
        .expect("written");
        let graph = Graph::load(&path).expect("it reads");

        let ui = MainWindow::new().expect("a window");
        let ping_window = PingWindow::new().expect("the ping window");
        let (through, feed) = ping_node(&ui, &ping_window, &graph)
            .expect("it can be pinged")
            .expect("there is a ping node");
        assert_eq!(through, "moc");
        assert!(ui.get_can_ping_node(), "the Ping node button is there");
        ui.invoke_ping_node_clicked();

        // The press -- Enter, which the window takes from the moment it
        // opens, as it does a click on Send ping: a ping goes up, numbered
        // from one, and is shown.
        for event in [
            slint::platform::WindowEvent::KeyPressed {
                text: slint::platform::Key::Return.into(),
            },
            slint::platform::WindowEvent::KeyReleased {
                text: slint::platform::Key::Return.into(),
            },
        ] {
            ping_window.window().dispatch_event(event);
        }
        let mut buffer = [0u8; ping::REPLY_BYTES];
        let (got, _) =
            vehicle.recv_from(&mut buffer).expect("the ping arrived");
        let sent = std::str::from_utf8(&buffer[..got])
            .expect("text")
            .to_string();
        assert_eq!(
            ping::read_message(sent.as_bytes()).expect("a ping").count,
            1
        );
        assert_eq!(ping_window.get_sent().as_str(), sent);
        assert_eq!(
            ping_window.get_received_time().as_str(),
            tcslib::ping::NO_PRECISE_TIME,
            "nothing has answered it"
        );

        // The answer, as the receiver hands it on, with the times filled in:
        // the window is told there is one, and shown it as its timer is.
        let had = ping::now().expect("a clock");
        let answer = ping::reply(sent.as_bytes(), had).expect("answered");
        take_answer(
            &feed,
            answer.as_bytes(),
            had + std::time::Duration::from_millis(1),
        );
        assert!(feed.fresh.load(Ordering::Acquire), "the window is told");
        show_ping(
            &ping_window,
            &PingShown::of(feed.pinger.lock().expect("the pinger").exchange()),
        );
        assert_eq!(
            ping_window.get_received().as_str(),
            String::from_utf8_lossy(answer.as_bytes())
        );
        assert!(!ping_window.get_received_time().is_empty());
        assert!(
            ping_window.get_round_trip().ends_with(" ms"),
            "{}",
            ping_window.get_round_trip()
        );
        assert!(
            ping_window.get_down().starts_with("1.000000"),
            "{}",
            ping_window.get_down()
        );

        // And a configuration with nothing to ping has no button.
        let other = MainWindow::new().expect("a window");
        let other_ping = PingWindow::new().expect("a ping window");
        assert!(ping_node(&other, &other_ping, &example("beacon"))
            .expect("nothing wrong")
            .is_none());
        assert!(!other.get_can_ping_node());
    }

    /// Connect, and then a panel's Enable, reach the vehicle and say what came
    /// back.
    ///
    /// Through the window, by pressing the buttons: everything below the
    /// handlers is tested on its own, and what this adds is the wiring -- that
    /// the press reaches the handler, the handler reaches the vehicle, and
    /// what the vehicle answered lands in Last Response. That was the one part
    /// of the command half nothing could check.
    ///
    /// The vehicle is a stand-in on a loopback port, answering a CONNECT with
    /// the digest this window read and a STOP_NODE with success. It answers
    /// the real messages, by the same serialisation tcspecial uses, so the
    /// client is exercised rather than mocked.
    ///
    /// Not a test of its own: one test per binary may start the testing
    /// backend, and its windows belong to the thread that made them. Called
    /// from `crate::tests::every_panels_data_is_inside_the_window`.
    pub(crate) fn pressing_connect_and_enable_commands_the_vehicle() {
        use i_slint_backend_testing::ElementHandle;
        use std::net::UdpSocket;
        use tcslibgs::{
            Command, ConfigDigest, ConfigVersion, ConnectTelemetry, HostPort,
            NodeTelemetry, Telemetry,
        };

        let ours = ConfigDigest([0x5Au8; 16]);

        // A stand-in vehicle: one socket for both links, since what is being
        // tested is that a press gets an answer back and not which socket it
        // went out on. Two commands, then it stops.
        let vehicle = UdpSocket::bind("127.0.0.1:0").expect("a stub socket");
        let at = vehicle.local_addr().expect("where it is").to_string();
        // What the stub was asked to set, so the test can say the typed
        // attribute and value are what went out.
        type Setting = (String, Vec<u8>);
        let was_set: Arc<Mutex<Option<Setting>>> = Arc::new(Mutex::new(None));
        let setting = was_set.clone();
        let answering = std::thread::spawn(move || {
            let mut buffer = [0u8; 65535];
            let mut seen = Vec::new();
            for _ in 0..7 {
                let (size, from) = match vehicle.recv_from(&mut buffer) {
                    Ok(got) => got,
                    Err(_) => return seen,
                };
                let command =
                    Command::read(&buffer[..size]).expect("a command");
                let seq = command.sequence();
                seen.push(command.cmd_type());
                let answer = match &command {
                    Command::Connect(_) => {
                        Telemetry::Connect(ConnectTelemetry::new(
                            seq,
                            ConfigVersion::of_this_build(),
                            // The same digest the window read: a vehicle flying
                            // the configuration this window is showing.
                            ConfigDigest([0x5Au8; 16]),
                            1,
                        ))
                    }
                    Command::StopNode(cmd) => {
                        Telemetry::StopNode(NodeTelemetry::stopped(
                            seq,
                            tcslibgs::CommandStatus::Success,
                            cmd.node,
                        ))
                    }
                    Command::StartNode(cmd) => {
                        Telemetry::StartNode(NodeTelemetry::started(
                            seq,
                            tcslibgs::CommandStatus::Success,
                            cmd.node,
                        ))
                    }
                    Command::SetNode(cmd) => {
                        *setting.lock().expect("the record") = Some((
                            cmd.attribute.to_string(),
                            cmd.value.to_vec(),
                        ));
                        Telemetry::SetNode(tcslibgs::SetNodeTelemetry::new(
                            seq,
                            tcslibgs::CommandStatus::Success,
                            cmd.node,
                        ))
                    }
                    Command::QueryNode(cmd) => {
                        Telemetry::QueryNode(tcslibgs::QueryNodeTelemetry::new(
                            seq,
                            tcslibgs::CommandStatus::Success,
                            cmd.node,
                            // Stopped, so the panel has something to show that
                            // it could not have known without asking.
                            false,
                            tcslibgs::NodeCounts {
                                taken_bytes: 480,
                                reads: 40,
                                asks: 40,
                                ..Default::default()
                            },
                        ))
                    }
                    other => panic!("the window sent {:?}", other.cmd_type()),
                };
                let data = answer.to_bytes();
                vehicle.send_to(&data, from).expect("an answer");
            }
            seen
        });

        let ui = MainWindow::new().expect("a window");
        let _ = Sizes::settle(&ui);

        // One panel, for a node whose id is 1, as the example's is -- and a
        // binding on it, so the popup has a field to press Set in. The
        // example states none, so the test states one: what is being checked
        // is the press and not the file.
        let graph = example("simple-app");
        let links = telemetry::of(&graph).expect("its links");
        let mut settable = settable_of(&graph);
        settable.insert(links[0].source.clone(), vec!["setpoint".to_string()]);
        let model: Rc<VecModel<DHInfo>> =
            Rc::new(VecModel::from(panels(&links, &settable)));
        ui.set_dh_model(ModelRc::from(model.clone()));
        ui.set_ci_status(SharedString::from(DISCONNECTED_STATUS));
        ui.set_ci_address(SharedString::from(at.clone()));

        let link = Arc::new(Mutex::new(CiLink::down_commanding(
            "127.0.0.1:0",
            Commanded {
                from: HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 0,
                },
                to: at
                    .parse::<std::net::SocketAddr>()
                    .map(|a| HostPort {
                        address: a.ip().to_string(),
                        port: a.port(),
                    })
                    .expect("an address"),
            },
        )));
        let agreed: Agreed = Arc::new(Mutex::new(None));
        let declined = declined_for(model.row_count());
        // The spacecraft being heard from, as the beacon thread would say: so
        // a Connect is the moment to ask what each node is doing.
        let contact = Arc::new(Mutex::new(tcslib::beacon::Contact::Made));
        command_buttons(
            &ui,
            &link,
            &agreed,
            Some(ours),
            &model,
            &declined,
            &contact,
        );

        // A panel starts at Unknown: nothing has been asked.
        assert_eq!(
            model.row_data(0).expect("the panel").status.as_str(),
            UNKNOWN_STATUS
        );

        let press = |label: &str| {
            let found: Vec<ElementHandle> =
                ElementHandle::find_by_accessible_label(&ui, label).collect();
            assert_eq!(
                found.len(),
                1,
                "one {label} button, found {}",
                found.len()
            );
            found[0].invoke_accessible_default_action();
        };

        // Before anything is pressed, a node command is refused: the two ends
        // have not agreed what they read.
        press("Enable");
        assert_eq!(
            ui.get_last_response().as_str(),
            NOT_AGREED,
            "a node id must not go out before CONNECT has agreed"
        );

        // Connect, which brings the link up and sends a CONNECT.
        press("Connect");
        assert_eq!(ui.get_ci_status().as_str(), CONNECTED_STATUS);
        assert!(
            ui.get_last_response().as_str().contains("agrees"),
            "the window should say the vehicle agreed: {}",
            ui.get_last_response()
        );
        assert!(may_command_nodes(&agreed), "and remember that it did");
        // And, contact having been made before the link was up, the Connect
        // asked what the node is doing: stopped, which only asking could tell.
        assert_eq!(
            model.row_data(0).expect("the panel").status.as_str(),
            STOPPED_STATUS,
            "a Connect in contact should have asked the spacecraft"
        );
        assert!(
            ui.get_last_response().as_str().contains("agrees"),
            "what the Connect said should still be there: {}",
            ui.get_last_response()
        );
        // Out of contact, a Connect asks nothing: were it to, the stand-in's
        // list below would say so.
        let out_of_contact = Mutex::new(tcslib::beacon::Contact::Lost);
        connected_in_contact(
            &ui,
            &link,
            &agreed,
            model.as_ref(),
            &declined,
            &out_of_contact,
        );

        // And now a panel's Disable reaches the vehicle, which answers about
        // the node the panel is for.
        press("Disable");
        let said = ui.get_last_response().to_string();
        assert!(
            said.contains("STOP_NODE") && said.contains("Success"),
            "the answer should be shown: {said}"
        );
        assert!(
            said.contains("lorem"),
            "and say which node it was about: {said}"
        );
        // The panel takes the answer, and the operator's word is kept.
        assert_eq!(
            model.row_data(0).expect("the panel").status.as_str(),
            STOPPED_STATUS
        );
        assert!(was_declined(&declined, 0), "Disable is the operator's word");

        // Contact made now asks nothing: the one node is one the operator
        // stopped. Were it asked, the stand-in's list below would say so.
        contact_made(&ui, &link, &agreed, model.as_ref(), &declined);

        // And the Set button in the panel's popup, which has to be opened
        // first: a popup's contents are not built until it is shown.
        press("Configuration");

        // The popup's two fields, found by what is in them rather than by a
        // label: a LineEdit carries no accessible label, and these two are
        // told apart by the thing the test is checking -- the attribute comes
        // filled in from the binding and the value does not.
        let fields: Vec<ElementHandle> =
            ElementHandle::find_by_element_type_name(&ui, "LineEdit")
                .filter(|field| {
                    field.accessible_value().is_some_and(|held| {
                        held == "setpoint" || held.is_empty()
                    })
                })
                .collect();
        assert_eq!(
            fields.len(),
            2,
            "the popup should have the attribute and the value: {:?}",
            fields
                .iter()
                .map(|f| f.accessible_value().map(|v| v.to_string()))
                .collect::<Vec<_>>()
        );
        assert!(
            fields.iter().any(|field| field
                .accessible_value()
                .is_some_and(|held| held == "setpoint")),
            "one field should start with the attribute the binding names, so \
             that only the value has to be typed"
        );
        let value = fields
            .iter()
            .find(|field| {
                field.accessible_value().is_some_and(|held| held.is_empty())
            })
            .expect("the value field, which starts empty");
        value.set_accessible_value("300.0");
        press("Set");

        let said = ui.get_last_response().to_string();
        assert!(
            said.contains("SET_NODE")
                && said.contains("setpoint")
                && said.contains("300.0"),
            "the answer should be shown: {said}"
        );
        assert_eq!(
            was_set.lock().expect("the record").clone(),
            Some(("setpoint".to_string(), b"300.0".to_vec())),
            "what was typed is what went out"
        );

        // And Query All, which is the one control whose answer the panels
        // take something from: a node that is running and one the ground
        // stopped cannot be told apart from what arrives, which is why the
        // query exists.
        // Through the menu, which is where the Query control is: the window
        // declares a query-all callback and nothing in it invokes one.
        press("Command");
        press("Query");
        assert_eq!(
            model.row_data(0).expect("the panel").status.as_str(),
            STOPPED_STATUS,
            "the panel should say what the query answered"
        );
        let shown =
            model.row_data(0).expect("the panel").parameters.to_string();
        assert!(
            shown.contains("480 bytes in 40 reads")
                && shown.contains("40 triggers"),
            "and the counts should be in the popup: {shown}"
        );
        assert!(
            shown.contains("device id"),
            "beside what the popup said already: {shown}"
        );

        // Enable puts the node back: Active, and no longer declined.
        press("Enable");
        assert_eq!(
            model.row_data(0).expect("the panel").status.as_str(),
            ACTIVE_STATUS
        );
        assert!(
            !was_declined(&declined, 0),
            "Enable is the operator's word too"
        );

        // So contact made asks about it, and the panel says what the
        // spacecraft answered -- stopped, which only asking could tell.
        contact_made(&ui, &link, &agreed, model.as_ref(), &declined);
        assert_eq!(
            model.row_data(0).expect("the panel").status.as_str(),
            STOPPED_STATUS,
            "contact made should have asked the spacecraft"
        );

        let sent = answering.join().expect("the stand-in answered");
        assert_eq!(
            sent,
            vec![
                tcslibgs::CommandType::Connect,
                tcslibgs::CommandType::QueryNode,
                tcslibgs::CommandType::StopNode,
                tcslibgs::CommandType::SetNode,
                tcslibgs::CommandType::QueryNode,
                tcslibgs::CommandType::StartNode,
                tcslibgs::CommandType::QueryNode
            ],
            "the commands the presses and contact made should have sent, and \
             no query of a node the operator had stopped"
        );
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
        assert!(anything_simulated(
            &example("minimal"),
            Some(&sim("minimal"))
        ));
        assert!(!anything_simulated(
            &example("simple-app"),
            Some(&sim("simple-app"))
        ));

        // And a set with no simulator file beside it simulates nothing: it
        // used to be read against another set's file, which complained about
        // handlers it had never heard of.
        assert!(!anything_simulated(&example("minimal"), None));
    }
}
