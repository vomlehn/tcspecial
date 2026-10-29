//! Starting a configuration, and stopping it.
//!
//! The program-level part of running a graph: read the file, check it, say
//! what is about to run, run it, and stop when the vehicle is asked to. It is
//! apart from [`crate::runtime::system`] on purpose -- that module is the
//! assembly and the loop and has no signals, no logging and no exit codes in
//! it, which is what lets a whole configuration be run in a test.
//!
//! # Checked before anything opens
//!
//! `Graph::check` is run here even though `tcsverify` runs it on the ground.
//! The same code, on the same file, in both places: a configuration that
//! reached a vehicle unchecked is the one case where the ground tool cannot
//! help, and a bus opened on the strength of a file nobody checked is worse
//! than a vehicle that will not start.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use log::{debug, error, info, warn};
use tcslibgs::config_digest::{digest_of_graph, ConfigDigest, ConfigVersion};
use tcslibgs::graph::Graph;
use tcslibgs::simulate::{judge, refusals, Host, Judged, Mode};

use crate::beacon_send::BeaconSend;
use crate::runtime::command::{Commands, Flying};
use crate::runtime::fabric::Moved;
use crate::runtime::restart;
use crate::runtime::system::{described, System};
use crate::segsend::{self, SegSend};
use crate::telemetry_log::TelemetryLog;

/// Whether something has asked this process to stop.
static STOPPING: AtomicBool = AtomicBool::new(false);

/// The run's simulation mode, `--simulate`, once `run_graph` has been given
/// it: none until then, the flight setting.
///
/// Held for the process, as the quiet flag is, because it is one fact about
/// the whole run and is asked where a node is opened -- at the start and again
/// on every START_NODE -- far below anything that could be handed it: see
/// `EndpointPort::open`, where a node with a `stand_in` is reached over UDP
/// when the run has stand-ins.
static MODE: std::sync::OnceLock<Mode> = std::sync::OnceLock::new();

/// The run's simulation mode: see [`MODE`].
pub fn mode() -> Mode {
    MODE.get().copied().unwrap_or(Mode::None)
}

/// Whether the startup summary is left unsaid: `-q` or `--quiet`.
static QUIET: AtomicBool = AtomicBool::new(false);

/// Leave the startup summary unsaid, for the rest of this process.
///
/// For `main`, once, before anything starts: a program that starts tcspecial
/// to show something else -- the examples' ground stations -- has its own
/// console to keep, and the summary is for somebody watching the vehicle
/// itself start. What stops a start is said whatever this says: a vehicle
/// that will not run must say why to whoever started it.
pub fn be_quiet() {
    QUIET.store(true, Ordering::Relaxed);
}

/// Say one line of the startup summary on stderr, unless told to be quiet.
///
/// The summary is what somebody watching a vehicle start compares against
/// the file they think is flying: the shape of the graph, the build and the
/// digest, the log, the beacon and the command addresses. One place that
/// says them, so that quiet means all of them.
pub fn summary(line: impl std::fmt::Display) {
    if !QUIET.load(Ordering::Relaxed) {
        eprintln!("{line}");
    }
}

/// Read, check and run the configuration in `path` under the simulation mode
/// `mode`, until asked to stop.
///
/// Every reason it could not start, rather than the first: a file with three
/// problems in it is one trip to a desk, where one problem at a time is three.
pub fn run_graph(path: &str, mode: Mode) -> Result<(), Vec<String>> {
    // Set once: a RESTART runs this again in the same process, with the mode
    // it was started with, and the first is the one that holds.
    let _ = MODE.set(mode);
    let graph =
        Graph::load(path).map_err(|why| vec![format!("{path}: {why}")])?;

    // The same checks the ground tool runs, against the same code. A
    // configuration that arrived here unchecked is exactly the configuration
    // that should not open a bus. With them, whether this build contains every
    // type the configuration has, which is tcsverify's `--types` with the
    // types this was built with.
    let mut problems = graph.check();
    problems.extend(tcslibgs::graph::built::not_built(
        &graph.nodes,
        crate::node::payload::BUILT,
    ));
    if !problems.is_empty() {
        return Err(problems
            .iter()
            .map(|problem| format!("{path}: {}", problem.describe()))
            .collect());
    }

    // What is about to run, before anything is opened: on stderr as the
    // command addresses are, because it is what somebody watching a vehicle
    // start compares against the file they think is flying.
    summary(format!("{path}: {}", shape_of(&graph)));
    for (name, what) in described(&graph) {
        summary(format!("  {name}: {what}"));
    }

    // Whether each payload is a stand-in, judged from what is at the end of
    // its endpoint and before anything is opened, and the run refused here if
    // the mode is not met: opening is not always harmless, and a check made
    // after opening would already have done what --simulate=all exists to
    // prevent. Said in the summary whatever the answer, because what kind of
    // run this is is what somebody watching it start needs to know first.
    let refused = judged_against(&graph, mode);
    if !refused.is_empty() {
        return Err(refused
            .iter()
            .map(|refusal| format!("{path}: {refusal}"))
            .collect());
    }

    // What this vehicle is serving: the build and the configuration's
    // digest. The ground compares the digest in every beacon with its own,
    // which is how it knows the vehicle is running the configuration it is
    // reading -- the same ports, the same device ids, the same nodes.
    let digest = digest_of_graph(&graph)
        .map_err(|why| vec![format!("{path}: {why}")])?;
    summary(format!(
        "tcspecial v{} md5 {digest}",
        ConfigVersion::of_this_build()
    ));

    // What every log hands its filled segments to, found before any log is
    // opened: opening one hands over what an earlier run left in it.
    let segsend =
        segsend_for(&graph).map_err(|why| vec![format!("{path}: {why}")])?;

    let mut system = System::of(&graph, Instant::now(), segsend.clone())?;
    // What was opened, which the line above already says the shape of: for
    // somebody looking at how the running system is put together.
    debug!("{path}: {system:?}");

    // What the vehicle sends to the ground, kept where the configuration
    // says: opened before anything that records into it, so that a log
    // directory that is not there stops the start rather than losing the
    // first beacons to it.
    let telemetry_log = telemetry_log(&graph, &segsend)
        .map_err(|why| vec![format!("{path}: {why}")])?;

    // Beacons, once everything has opened: a vehicle that will not start has
    // nothing to announce. Held until the vehicle stops.
    let (beacon, generation) = beacons(&graph, digest, telemetry_log.clone())
        .map_err(|why| vec![format!("{path}: {why}")])?;

    // And the ground's commands, on a thread of their own. After the beacons
    // because a CONFIG retimes them, and after the ports because the first
    // thing the ground may do is stop a node: a command link that opened
    // before the nodes did could be asked to stop one that does not exist
    // yet.
    let flying = Flying {
        version: ConfigVersion::of_this_build(),
        digest,
        generation,
    };
    let mut commands = command_links(
        &graph,
        system.states(),
        system.workbench(),
        flying,
        beacon.clone(),
        telemetry_log.clone(),
    )
    .map_err(|why| vec![format!("{path}: {why}")])?;

    listen_for_stop();

    // The loop's own wakeup, taken before the loop borrows the system: a pass
    // waiting for the next group to come due is woken by work the command
    // interpreter has queued, so a command is acted on between stages rather
    // than when the group is read.
    let nudge = system.nudge();
    system.run(&asked_to_stop, report, move |waiting| nudge.wait(waiting));

    // The links closed before the log is flushed, so that nothing records an
    // answer into a log whose last page has already been written.
    if let Some(commands) = commands.as_mut() {
        commands.stop();
    }

    // What was recorded and not yet written, before the process goes: the
    // log is the account of what the vehicle sent, and an account missing
    // its last page is one nobody can rely on.
    telemetry_log.flush();

    info!("{path}: stopped");
    Ok(())
}

/// Judge each payload of `graph`, say what each was found to be, and give
/// whatever `mode` forbids of them: see `tcslibgs::simulate`.
fn judged_against(graph: &Graph, mode: Mode) -> Vec<String> {
    let host = Host::this();
    let judged: Vec<(&str, Judged)> = graph
        .nodes
        .iter()
        .filter_map(|node| {
            judge(&host, node, mode).map(|judged| (node.name.as_str(), judged))
        })
        .collect();
    summary(format!("simulation mode: {mode}"));
    for (name, judged) in &judged {
        summary(format!("  {name}: {}", judged.said()));
    }
    let pairs: Vec<(&str, &Judged)> =
        judged.iter().map(|(name, j)| (*name, j)).collect();
    refusals(mode, &pairs)
}

/// What a configuration is, in a line: how many nodes, groups and logs, each
/// counted in words -- `1 node, no groups, no logs`, `11 nodes, 1 group, 1 log
/// (housekeeping)` -- and the logs named, a log being where to look for what
/// the vehicle kept.
fn shape_of(graph: &Graph) -> String {
    let counted = |n: usize, one: &str, many: &str| match n {
        0 => format!("no {many}"),
        1 => format!("1 {one}"),
        n => format!("{n} {many}"),
    };
    let logs: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.node_type == "log")
        .map(|node| node.name.as_str())
        .collect();
    let mut said = format!(
        "{}, {}, {}",
        counted(graph.nodes.len(), "node", "nodes"),
        counted(graph.groups.len(), "group", "groups"),
        counted(logs.len(), "log", "logs")
    );
    if !logs.is_empty() {
        said.push_str(&format!(" ({})", logs.join(", ")));
    }
    said
}

/// The telemetry log the configuration's `tcspecial` section describes.
///
/// One log for everything the vehicle sends to the ground, which is what
/// makes the downlink reconstructable afterwards: the ground has only what
/// arrived, and this is what was sent. Beacons record into it here, and the
/// command interpreter's answers will when there is one.
///
/// Not the same thing as a `log` node, and not a substitute for one. A log
/// node records what is delivered to it -- the configuration draws the links
/// and decides what is kept -- where this records what leaves the vehicle on
/// a link no graph describes.
///
/// A configuration with no `tcspecial` section, or one naming no log
/// directory, records nothing: see `TelemetryLog::open`.
fn telemetry_log(
    graph: &Graph,
    segsend: &SegSend,
) -> Result<TelemetryLog, String> {
    let Some(section) = graph.tcspecial.as_ref() else {
        return Ok(TelemetryLog::disabled());
    };
    let ci = tcslibgs::graph::parameters::ci_config_of(
        &tcslibgs::graph::parameters::flatten(section),
    )?
    .to_ci_config()?;
    let log = TelemetryLog::open(&ci, segsend).map_err(|e| e.to_string())?;
    if log.is_enabled() {
        summary(format!(
            "telemetry log: {}, {}-byte segments",
            ci.log_dir.as_deref().unwrap_or("?"),
            ci.log_segment_bytes
        ));
    }
    Ok(log)
}

/// Start the thread that hands every log's filled segments to `segsend`, if
/// this configuration keeps a log.
///
/// A configuration that keeps one -- a `log.directory` in its `tcspecial`
/// section, or a `log` node -- does not start without a `segsend` beside
/// tcspecial: a vehicle that found out at its first full segment that it had
/// nothing to hand it to would fill its log directory without a word. One
/// that keeps none has nothing to hand over, and does not need one.
fn segsend_for(graph: &Graph) -> Result<SegSend, String> {
    let telemetry = match graph.tcspecial.as_ref() {
        Some(section) => tcslibgs::graph::parameters::ci_config_of(
            &tcslibgs::graph::parameters::flatten(section),
        )?
        .to_ci_config()?
        .log_dir
        .is_some(),
        None => false,
    };
    let nodes = graph.nodes.iter().any(|node| node.node_type == "log");
    if !telemetry && !nodes {
        return Ok(SegSend::none());
    }
    let program = segsend::beside_this_program()?;
    let started = SegSend::start(&program)?;
    summary(format!("filled log segments: {}", program.display()));
    Ok(started)
}

/// Start sending beacons where the configuration's `tcspecial` section says.
///
/// From `beacon.send` to `beacon.recv`, every `beacon.default_interval`, carrying
/// this build's version, the configuration's digest and this start's
/// generation -- counted in `state_file` here, before the first beacon goes,
/// so that the first beacon already says it. Every
/// `beacon.reset_interval` until the MOC is heard from, as every start
/// is: see [`BeaconSend::heard_from_moc`].
/// An interval of nought sends none. Recorded in the telemetry log, which is
/// the vehicle's own account of what it sent.
///
/// Returns the generation as well, which the command links refuse a command
/// of any other generation by. Nought, which no start is, where there is no
/// `tcspecial` section, which takes no commands either.
///
/// A `beacon.send` that cannot be bound, or a `beacon.recv` that does not
/// resolve, stops the start: a vehicle flying without beacons is one the
/// ground cannot tell from a dead one.
fn beacons(
    graph: &Graph,
    digest: ConfigDigest,
    telemetry_log: TelemetryLog,
) -> Result<(Option<BeaconSend>, u32), String> {
    let Some(section) = graph.tcspecial.as_ref() else {
        return Ok((None, 0));
    };
    let ci = tcslibgs::graph::parameters::ci_config_of(
        &tcslibgs::graph::parameters::flatten(section),
    )?
    .to_ci_config()?;
    let started = crate::state::start(std::path::Path::new(&ci.state_file))?;
    if let Some(why) = &started.damaged {
        log::warn!(
            "the state file was not one tcspecial can read ({why}), so the \
             generation starts again from 1"
        );
    }
    // Which kind of start this is goes in the summary, because it is the
    // first thing somebody reading a log after an unexpected restart asks.
    summary(format!(
        "generation {}, {}: {}",
        started.generation,
        if started.commanded {
            "a commanded restart"
        } else {
            "not commanded"
        },
        ci.state_file
    ));
    let sending = BeaconSend::new(
        crate::beacon_send::BeaconTiming {
            default_interval: ci.beacon_interval,
            reset_interval: ci.reset_interval,
        },
        ci.beacon_send.clone(),
        ci.beacon_recv.clone(),
        telemetry_log,
        ConfigVersion::of_this_build(),
        digest,
        &started,
    )
    .map_err(|e| e.to_string())?;
    if sending.is_some() {
        summary(format!(
            "{} ms beacon: {} to {}",
            ci.beacon_interval.as_millis(),
            ci.beacon_send,
            ci.beacon_recv
        ));
    }
    Ok((sending, started.generation))
}

/// Take the ground's commands where the configuration's `tcspecial` section
/// says.
///
/// Two links: the spacecraft's at `tcs_cmd.recv`, and the one commands about
/// nodes arrive on at `payload_command.dest_base`. See
/// [`crate::runtime::command`], which says why there are two and why the
/// second is one socket.
///
/// A configuration with no `tcspecial` section takes no commands, as it sends
/// no beacons: the section is where the ports are written down, and a vehicle
/// that chose its own would be a vehicle no ground station could find.
fn command_links(
    graph: &Graph,
    states: std::sync::Arc<crate::runtime::node_state::NodeStates>,
    bench: crate::runtime::system::Workbench,
    flying: Flying,
    beacon: Option<BeaconSend>,
    telemetry_log: TelemetryLog,
) -> Result<Option<Commands>, String> {
    let Some(section) = graph.tcspecial.as_ref() else {
        return Ok(None);
    };
    let ci = tcslibgs::graph::parameters::ci_config_of(
        &tcslibgs::graph::parameters::flatten(section),
    )?
    .to_ci_config()?;

    let commands = Commands::start(
        &ci,
        states,
        bench,
        flying,
        beacon,
        telemetry_log,
        restart_when_stopped,
    )
    .map_err(|e| e.to_string())?;
    // Where they are taken, as the sockets were bound: a configuration may ask
    // for port 0, and then the socket is the only statement of which port that
    // is. On stderr as the beacon line is, because it is what somebody
    // standing at a console reads to find out where to send.
    summary(format!(
        "commands: spacecraft on {}, nodes on {}",
        commands.spacecraft_address(),
        commands.node_address()
    ));
    Ok(Some(commands))
}

/// Whether the loop should stop.
pub fn asked_to_stop() -> bool {
    STOPPING.load(Ordering::Relaxed)
}

/// Stop, as a signal stops the vehicle: the pass finished, the links closed,
/// the log flushed.
///
/// For the visual window, whose close box stops the tcspecial it shows. The
/// same flag an interrupt sets, so a window closed is a vehicle stopped the
/// way a person at the console would have stopped it.
pub fn stop_as_signalled() {
    STOPPING.store(true, Ordering::Relaxed);
}

/// Stop, as a signal stops the vehicle, and then run again.
///
/// What an armed RESTART asks for. The vehicle stops the way it always does
/// -- the pass finished, the links closed, the log flushed -- because a
/// restart that left a log part way through a record would lose the record
/// the restart was meant to keep going; [`run_or_report`] then runs it again.
fn restart_when_stopped() {
    restart::ask();
    STOPPING.store(true, Ordering::Relaxed);
}

/// What a pass did, where anything went wrong with it.
///
/// Every refusal, named. A pass that delivered everything says nothing: a
/// vehicle reading three sensors every three seconds would otherwise fill a
/// log with the news that it is working.
fn report(moved: &Moved) {
    for (node, why) in &moved.refused {
        // The node once. A refusal from a port already opens with the node's
        // name -- it is the one fact every complaint in this program carries
        // -- and a line that said it twice reads as though two things went
        // wrong.
        if why.starts_with(&format!("{node}: ")) {
            warn!("{why}");
        } else {
            warn!("{node}: {why}");
        }
    }
}

/// Ask the kernel to tell us about an interrupt and a termination.
///
/// Both of them, because the two ways a vehicle is stopped are a person at a
/// console and a service manager: a loop that answered only one of them would
/// be killed outright by the other, with a log part way through a record.
fn listen_for_stop() {
    // SAFETY: the handler does one thing -- an atomic store -- which is
    // async-signal-safe. `libc::signal` is used rather than `sigaction`
    // because that is all this needs and it is what is available here.
    unsafe {
        let handler = stop as *const () as libc::sighandler_t;
        libc::signal(libc::SIGINT, handler);
        libc::signal(libc::SIGTERM, handler);
    }
}

/// What a signal does: sets a flag and returns.
///
/// Nothing else, which is the rule for a signal handler: the loop notices
/// within one pause, finishes the pass it is making and flushes its logs.
/// Stopping inside a pass would leave a log part way through a record and a
/// heater in a state nothing decided.
extern "C" fn stop(_signal: libc::c_int) {
    STOPPING.store(true, Ordering::Relaxed);
}

/// Run the configuration in `path` under `mode` and say what to exit with.
///
/// For `main`, which has one more thing to do than report: a vehicle that
/// could not start must not look as though it is running.
pub fn run_or_report(path: &str, mode: Mode) -> i32 {
    match run_graph(path, mode) {
        Ok(()) => {
            // Does not return where a RESTART asked for one.
            restart::if_asked();
            0
        }
        Err(problems) => {
            for problem in &problems {
                error!("{problem}");
            }
            // On stderr too, because a vehicle that will not start is read by
            // whoever is standing there and not through RUST_LOG.
            for problem in &problems {
                eprintln!("{problem}");
            }
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A configuration with a problem in it does not open anything.
    ///
    /// Every problem, and before any port: the point of checking here is that
    /// a file nobody checked does not get as far as a bus.
    #[test]
    fn a_configuration_with_problems_is_refused_before_it_opens() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("wrong.yaml");
        std::fs::write(
            &path,
            "payloads:\n\
             \x20 - { name: a, type: i2c, path: /dev/i2c-1, address: \"0x48\", \
             payload-out: nobody }\n\
             vehicle:\n\
             \x20 - { name: a, type: compute, function: mean }\n",
        )
        .expect("the file is written");

        let problems = run_graph(path.to_str().expect("a path"), Mode::All)
            .expect_err("a configuration with faults");
        assert!(
            problems.len() >= 2,
            "every problem should be given: {problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("two nodes are called")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("nobody")),
            "{problems:?}"
        );
    }

    /// The shipped example is a graph, and is checked without complaint.
    #[test]
    fn the_shipped_example_is_a_checked_graph() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let path = root.join("tests/manual/thermostat.yaml");
        let path = path.to_str().expect("a path");

        let graph = Graph::load(path).expect("it reads");
        assert!(graph.check().is_empty(), "{:?}", graph.check());
    }

    /// A stop is noticed, and is not noticed before it happens.
    #[test]
    fn a_stop_is_noticed() {
        assert!(!asked_to_stop(), "nothing has asked yet");
        stop(libc::SIGTERM);
        assert!(asked_to_stop());
        // Left as it was found, this being a flag the whole process shares.
        STOPPING.store(false, Ordering::Relaxed);
    }
}
