//! Running a version 2 configuration.
//!
//! A version 2 vehicle is a graph of nodes, and what the window shows is that
//! graph: each node drawn into a `moc` node sends to the `moc` node's
//! `ground_base` plus its device id, and each payload nothing touches sends
//! over `payload_telemetry` -- see `tcslibgs::graph::telemetry`. So there is
//! one panel per source, filled through `tcslib::moc`, the same function call
//! any ground software uses.
//!
//! # What the buttons send
//!
//! The same two links a version 1 vehicle takes commands on, and the second
//! is where they differ: version 1 binds a socket per payload, and a version 2
//! vehicle binds one for every node, the node's id in the message saying which
//! -- see `tcslibgs::NodeId`. So Enable and Disable on a panel are START_NODE
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

use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use tcslib::moc::Moc;
use tcslibgs::graph::telemetry::{self, Telemetered, Via};
use tcslibgs::graph::Graph;
use tcslibgs::sim_config::SimConfigFile;
use tcslibgs::{DHSample, Timestamp};
use tcsverify::startup::Startup;

use crate::beacon_receive::BeaconReceive;
use crate::ci_link::{CiLink, Commanded};
use crate::oc_link::{Arrival, Arrivals};
use crate::{
    ask_for_the_edge, grid_shape, handle_close, handle_quit, present, show_arrivals, stop,
    window_size, DHInfo, Edge, MainWindow, MenuAction, ProcessManager, Sizes, CHILDREN,
    CONNECTED_STATUS, DISCONNECTED_STATUS, ERROR_STATUS, NO_TRANSFER_TIME,
    PANEL_REFRESH_INTERVAL,
};

/// What a panel's status says: a source the ground receives from.
pub const RECEIVING_STATUS: &str = "Receiving";

/// What a panel's status says of a node the ground has stopped.
///
/// Beside [`RECEIVING_STATUS`], which is what a panel says before anything
/// has been asked: a panel is a source the ground receives from, and a query
/// is what turns that into a statement about the node itself.
pub const STOPPED_STATUS: &str = "Stopped";

/// What a node command says when the two ends have not agreed what they read.
pub const NOT_AGREED: &str = "The vehicle has not agreed which configuration it is flying, so      a node id means nothing yet: press Connect, which sends CONNECT and      compares the digests.";

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
pub fn panels(links: &[Telemetered], settable: &BTreeMap<String, Vec<String>>) -> Vec<DHInfo> {
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

/// The two command links the configuration's `tcspecial` section states.
///
/// The spacecraft's -- where this end sends from, `tcs_cmd.send`, and where
/// the vehicle takes them, `tcs_cmd.recv` -- and the one every node is
/// commanded on. Both ends of each from the file, which is what lets the
/// vehicle refuse a command from anywhere else.
///
/// `None` for a configuration with no section, which takes no commands, as it
/// sends no beacons: the section is where the ports are written down.
fn command_links_of(graph: &Graph) -> Result<Option<(String, String, Commanded)>, String> {
    let Some(section) = graph.tcspecial.as_ref() else {
        return Ok(None);
    };
    let ci = tcslibgs::graph::parameters::ci_config_of(&tcslibgs::graph::parameters::flatten(
        section,
    ))?
    .to_ci_config()?;
    Ok(Some((
        ci.moc_cmd_send.to_string(),
        format!("{}:{}", ci.address, ci.port),
        Commanded::Nodes {
            from: ci.payload_command_source_base,
            to: ci.payload_command_dest_base,
        },
    )))
}

/// Say what the vehicle answered a CONNECT with, and whether it agrees.
///
/// What version 1 does, by the same words: the three facts said the way a
/// beacon says them, so the two can be read against each other. What version
/// 2 adds is that the verdict is kept -- a node id means nothing until the
/// two ends have agreed which configuration they are talking about.
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
                ui.set_last_response(SharedString::from(crate::ci_link::NOT_CONNECTED));
                return;
            }
        }
    };

    match answer {
        Ok((theirs, their_config, their_digest)) => {
            eprintln!(
                "CONNECT answer: {}",
                crate::beacon_receive::what_it_read(theirs, their_config, their_digest)
            );
            let same = their_digest == digest;
            if let Ok(mut held) = agreed.lock() {
                *held = Some(same);
            }
            let said = if !same {
                format!(
                    "tcspecial is flying a different configuration: config \
                     v{their_config}, md5 {their_digest}, not {digest} -- so a node \
                     id would name a node of a graph this window has not read, and \
                     Enable and Disable will not send one"
                )
            } else if theirs != version {
                format!(
                    "tcspecial is version {theirs}, this is {version}; it is flying \
                     the configuration this window read, config v{their_config}, \
                     md5 {digest}"
                )
            } else {
                format!("tcspecial agrees: v{version}, config v{their_config}, md5 {digest}")
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
            ui.set_last_response(SharedString::from(format!("CONNECT failed: {e}")));
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
    model: &Rc<VecModel<DHInfo>>,
) {
    if !may_command_nodes(agreed) {
        ui.set_last_response(SharedString::from(NOT_AGREED));
        return;
    }

    let mut said = Vec::new();
    for row in 0..model.row_count() {
        let Some(mut info) = model.row_data(row) else {
            continue;
        };
        let node = tcslibgs::NodeId(u32::try_from(info.dev_id).unwrap_or(u32::MAX));
        let answer = {
            let mut guard = link.lock().unwrap();
            match guard.client() {
                Some(client) => client.query_node(node),
                None => {
                    ui.set_last_response(SharedString::from(crate::ci_link::NOT_CONNECTED));
                    return;
                }
            }
        };

        match answer {
            Ok((status, enabled, counts)) if status.is_success() => {
                info.status = SharedString::from(if enabled {
                    RECEIVING_STATUS
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
            Ok((status, _, _)) => said.push(format!("{}: {status:?}", info.name)),
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
            Some(client) => client.set_node(node, attribute.trim(), value.as_bytes()),
            None => {
                ui.set_last_response(SharedString::from(crate::ci_link::NOT_CONNECTED));
                return;
            }
        }
    };

    let said = match answer {
        Ok((status, answered)) if answered != node => format!(
            "SET_NODE {node} ({name}) was answered about {answered}: the two ends \
             are not reading the same configuration ({status:?})"
        ),
        Ok((status, _)) if status.is_success() => {
            format!("SET_NODE {node} ({name}).{} = {value}: {status:?}", attribute.trim())
        }
        // What was refused and why are two things, and only the first is on
        // the wire: an attribute no binding mentions and a value the node
        // will not take are both InvalidParameter, and the vehicle's log says
        // which. Said here as the two possibilities, because an operator with
        // the window and no log should still know where to look.
        Ok((status, _)) => format!(
            "SET_NODE {node} ({name}).{} = {value} was refused: {status:?} -- no \
             binding names that attribute, or the node will not take that value",
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
fn command_a_node(
    ui: &MainWindow,
    link: &Mutex<CiLink>,
    agreed: &Agreed,
    model: &Rc<VecModel<DHInfo>>,
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
                ui.set_last_response(SharedString::from(crate::ci_link::NOT_CONNECTED));
                return;
            }
        }
    };

    let said = match answer {
        // The id in the answer as well as the name: the answer carries the id
        // it was asked about, and an id that came back as another is the one
        // thing this could not otherwise notice.
        Ok((status, answered)) if answered != node => format!(
            "{verb} {node} ({name}) was answered about {answered}: the two ends are \
             not reading the same configuration ({status:?})"
        ),
        Ok((status, _)) if status.is_success() => {
            format!("{verb} {node} ({name}): {status:?}")
        }
        Ok((status, _)) => format!("{verb} {node} ({name}) was refused: {status:?}"),
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
pub(crate) fn command_buttons(
    ui: &MainWindow,
    link: &Arc<Mutex<CiLink>>,
    agreed: &Agreed,
    digest: Option<tcslibgs::ConfigDigest>,
    model: &Rc<VecModel<DHInfo>>,
) {
    // Connect and Disconnect, the one button whose meaning follows the
    // status. Connecting sends a CONNECT at once, because that is what makes
    // a node id mean anything -- see `Agreed`.
    {
        let weak = ui.as_weak();
        let link = link.clone();
        let agreed = agreed.clone();
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
                    "Disconnected from {address}"
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
                    hear_what_the_vehicle_read(&ui, &link, &agreed, ours, digest)
                }
                // Nothing to compare with, so nothing may be commanded by id:
                // the warning at startup said why.
                None => ui.set_last_response(SharedString::from(
                    "Connected, and this configuration could not be digested, so \
                     there is nothing to check the vehicle's answer against: \
                     Enable and Disable will not send a node id",
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
            let answer = link.lock().unwrap().client().map(|client| client.ping());
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
                MenuAction::Query => query_every_node(&ui, &link, &agreed, &model),
                MenuAction::Ping => {
                    let answer = link.lock().unwrap().client().map(|client| client.ping());
                    ui.set_last_response(SharedString::from(match answer {
                        Some(Ok(tm)) => {
                            format!("PING OK at {}", tm.header.timestamp.time_of_day())
                        }
                        Some(Err(e)) => format!("PING failed: {e}"),
                        None => crate::ci_link::NOT_CONNECTED.to_string(),
                    }));
                }
                // The rest are version 1's. A restart is not answered by a
                // version 2 vehicle yet, and the three DH items name a data
                // handler where a graph has nodes -- which the panels'
                // buttons command.
                other => ui.set_last_response(SharedString::from(format!(
                    "{other:?} is not answered by a version 2 vehicle: a graph has \
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
        ui.on_enable_dh(move |row| {
            if let Some(ui) = weak.upgrade() {
                command_a_node(&ui, &link, &agreed, &model, row as usize, true);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let link = link.clone();
        let agreed = agreed.clone();
        let model = model.clone();
        ui.on_disable_dh(move |row| {
            if let Some(ui) = weak.upgrade() {
                command_a_node(&ui, &link, &agreed, &model, row as usize, false);
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
    const NO_LINK: &str = "This configuration states no tcspecial section, so it says where \
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

    let rows_in_order = panels(&links, &settable_of(&graph));
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

    // The two command links, both ends of each from the file. A configuration
    // with no `tcspecial` section states no ports, so there is nothing to
    // connect to and the buttons say so.
    let (link, vehicle_at) = match command_links_of(&graph) {
        Ok(Some((from, to, commanded))) => {
            eprintln!("Commands from {from} to {to}, nodes on {}", match &commanded {
                Commanded::Nodes { to, .. } => to.to_string(),
                Commanded::Payloads(_) => "payload links".to_string(),
            });
            (
                Some(Arc::new(Mutex::new(CiLink::down_commanding(&from, commanded)))),
                to,
            )
        }
        Ok(None) => {
            startup.warning(format!(
                "{payload_path} states no tcspecial section, so it says where no \
                 command link is: nothing can be started or stopped from here"
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
        "Version 2: each panel is a node. Press Connect to command them",
    ));

    // Whether the vehicle has agreed which configuration it is flying, which
    // is what licenses a node id: see `Agreed`.
    let agreed: Agreed = Arc::new(Mutex::new(None));

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

    command_buttons(&ui, &link, &agreed, digest, &model);

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

    fn example(name: &str) -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        Graph::load(root.join(format!("examples/{name}/{name}.yaml"))).expect("the example reads")
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
        let graph = example("minimal");
        let links = telemetry::of(&graph).expect("its links");
        let panels = panels(&links, &settable_of(&graph));
        assert_eq!(panels.len(), 1);
        assert_eq!(panels[0].name.as_str(), "sensor");
        assert!(panels[0].config.contains("payload_telemetry"), "{}", panels[0].config);
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
        match commanded {
            Commanded::Nodes { from, to } => {
                assert_eq!(from.to_string(), "127.0.0.1:4300");
                assert_eq!(to.to_string(), "0.0.0.0:4200");
            }
            Commanded::Payloads(_) => panic!("a graph's nodes are not payloads"),
        }
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

        let shown = panels(&telemetry::of(&graph).expect("its links"), &settable);
        // The thermostat is not drawn into the moc node in this example -- its
        // decision goes to the heaters -- so the panels are the four sources,
        // none of which binds anything.
        for panel in &shown {
            assert_eq!(
                (panel.settable.as_str(), panel.setting.as_str()),
                ("", ""),
                "{} binds nothing, so its panel offers nothing",
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
            Command, ConfigDigest, ConfigVersion, ConnectTelemetry, HostPort, NodeTelemetry,
            Telemetry,
        };

        let ours = ConfigDigest([0x5Au8; 16]);

        // A stand-in vehicle: one socket for both links, since what is being
        // tested is that a press gets an answer back and not which socket it
        // went out on. Two commands, then it stops.
        let vehicle = UdpSocket::bind("127.0.0.1:0").expect("a stub socket");
        let at = vehicle.local_addr().expect("where it is").to_string();
        // What the stub was asked to set, so the test can say the typed
        // attribute and value are what went out.
        let was_set: Arc<Mutex<Option<(String, Vec<u8>)>>> = Arc::new(Mutex::new(None));
        let setting = was_set.clone();
        let answering = std::thread::spawn(move || {
            let mut buffer = [0u8; 65535];
            let mut seen = Vec::new();
            for _ in 0..4 {
                let (size, from) = match vehicle.recv_from(&mut buffer) {
                    Ok(got) => got,
                    Err(_) => return seen,
                };
                let command: Command =
                    serde_json::from_slice(&buffer[..size]).expect("a command");
                let seq = command.sequence();
                seen.push(command.cmd_type());
                let answer = match &command {
                    Command::Connect(_) => Telemetry::Connect(ConnectTelemetry::new(
                        seq,
                        ConfigVersion::of_this_build(),
                        ConfigVersion::of_text("2.0").expect("a version"),
                        // The same digest the window read: a vehicle flying
                        // the configuration this window is showing.
                        ConfigDigest([0x5Au8; 16]),
                    )),
                    Command::StopNode(cmd) => Telemetry::StopNode(NodeTelemetry::stopped(
                        seq,
                        tcslibgs::CommandStatus::Success,
                        cmd.node,
                    )),
                    Command::SetNode(cmd) => {
                        *setting.lock().expect("the record") =
                            Some((cmd.attribute.clone(), cmd.value.clone()));
                        Telemetry::SetNode(tcslibgs::NodeTelemetry::set(
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
                let data = serde_json::to_vec(&answer).expect("telemetry");
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
            Commanded::Nodes {
                from: HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 0,
                },
                to: at.parse::<std::net::SocketAddr>().map(|a| HostPort {
                    address: a.ip().to_string(),
                    port: a.port(),
                }).expect("an address"),
            },
        )));
        let agreed: Agreed = Arc::new(Mutex::new(None));
        command_buttons(&ui, &link, &agreed, Some(ours), &model);

        let press = |label: &str| {
            let found: Vec<ElementHandle> =
                ElementHandle::find_by_accessible_label(&ui, label).collect();
            assert_eq!(found.len(), 1, "one {label} button, found {}", found.len());
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
                    field
                        .accessible_value()
                        .is_some_and(|held| held == "setpoint" || held.is_empty())
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
            fields
                .iter()
                .any(|field| field.accessible_value().is_some_and(|held| held == "setpoint")),
            "one field should start with the attribute the binding names, so that \
             only the value has to be typed"
        );
        let value = fields
            .iter()
            .find(|field| field.accessible_value().is_some_and(|held| held.is_empty()))
            .expect("the value field, which starts empty");
        value.set_accessible_value("300.0");
        press("Set");

        let said = ui.get_last_response().to_string();
        assert!(
            said.contains("SET_NODE") && said.contains("setpoint") && said.contains("300.0"),
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
        let shown = model.row_data(0).expect("the panel").parameters.to_string();
        assert!(
            shown.contains("480 bytes in 40 reads") && shown.contains("40 triggers"),
            "and the counts should be in the popup: {shown}"
        );
        assert!(
            shown.contains("device id"),
            "beside what the popup said already: {shown}"
        );

        let sent = answering.join().expect("the stand-in answered");
        assert_eq!(
            sent,
            vec![
                tcslibgs::CommandType::Connect,
                tcslibgs::CommandType::StopNode,
                tcslibgs::CommandType::SetNode,
                tcslibgs::CommandType::QueryNode
            ],
            "the four commands the presses should have sent"
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
        assert!(anything_simulated(&example("minimal"), &sim("minimal")));
        assert!(!anything_simulated(&example("simple-app"), &sim("simple-app")));
    }
}
