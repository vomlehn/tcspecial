//! Starting a version 2 configuration, and stopping it.
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
use std::time::{Duration, Instant};

use log::{debug, error, info, warn};
use tcslibgs::config_digest::{digest_of_graph, ConfigDigest, ConfigVersion};
use tcslibgs::graph::Graph;

use crate::beacon_send::BeaconSend;
use crate::runtime::fabric::Moved;
use crate::telemetry_log::TelemetryLog;
use crate::runtime::system::{described, System};

/// Whether something has asked this process to stop.
static STOPPING: AtomicBool = AtomicBool::new(false);

/// Read, check and run the configuration in `path`, until asked to stop.
///
/// Every reason it could not start, rather than the first: a file with three
/// problems in it is one trip to a desk, where one problem at a time is three.
pub fn run_graph(path: &str) -> Result<(), Vec<String>> {
    let graph = Graph::load(path).map_err(|why| vec![format!("{path}: {why}")])?;

    // The same checks the ground tool runs, against the same code. A
    // configuration that arrived here unchecked is exactly the configuration
    // that should not open a bus.
    let problems = graph.check();
    if !problems.is_empty() {
        return Err(problems
            .iter()
            .map(|problem| format!("{path}: {}", problem.describe()))
            .collect());
    }

    // What is about to run, before anything is opened: on stderr as the
    // command addresses are, because it is what somebody watching a vehicle
    // start compares against the file they think is flying.
    eprintln!("{path}: {}", shape_of(&graph));
    for (name, what) in described(&graph) {
        eprintln!("  {name}: {what}");
    }

    // What this vehicle is serving, said as version 1 says it: the build,
    // the configuration's version and its digest. The ground compares the
    // digest in every beacon with its own, which is how it knows the vehicle
    // is running the configuration it is reading -- the same ports, the same
    // device ids, the same nodes.
    let digest = digest_of_graph(&graph).map_err(|why| vec![format!("{path}: {why}")])?;
    let config_version = ConfigVersion::of_text(tcslibgs::graph::VERSION)
        .expect("the language's own version reads");
    eprintln!(
        "tcspecial v{}, config v{config_version}, md5 {digest}",
        ConfigVersion::of_this_build()
    );

    let mut system = System::of(&graph, Instant::now())?;
    // What was opened, which the line above already says the shape of: for
    // somebody looking at how the running system is put together.
    debug!("{path}: {system:?}");

    // What the vehicle sends to the ground, kept where the configuration
    // says: opened before anything that records into it, so that a log
    // directory that is not there stops the start rather than losing the
    // first beacons to it.
    let telemetry_log = telemetry_log(&graph).map_err(|why| vec![format!("{path}: {why}")])?;

    // Beacons, once everything has opened: a vehicle that will not start has
    // nothing to announce. Held until the vehicle stops.
    let _beacon = beacons(&graph, config_version, digest, telemetry_log.clone())
        .map_err(|why| vec![format!("{path}: {why}")])?;

    listen_for_stop();

    system.run(&asked_to_stop, report, pause);

    // What was recorded and not yet written, before the process goes: the
    // log is the account of what the vehicle sent, and an account missing
    // its last page is one nobody can rely on.
    telemetry_log.flush();

    info!("{path}: stopped");
    Ok(())
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
fn telemetry_log(graph: &Graph) -> Result<TelemetryLog, String> {
    let Some(section) = graph.tcspecial.as_ref() else {
        return Ok(TelemetryLog::disabled());
    };
    let ci = tcslibgs::graph::parameters::ci_config_of(&tcslibgs::graph::parameters::flatten(
        section,
    ))?
    .to_ci_config()?;
    let log = TelemetryLog::open(&ci).map_err(|e| e.to_string())?;
    if log.is_enabled() {
        eprintln!(
            "telemetry log: {}, {}-byte segments",
            ci.log_dir.as_deref().unwrap_or("?"),
            ci.log_segment_bytes
        );
    }
    Ok(log)
}

/// Start sending beacons where the configuration's `tcspecial` section says.
///
/// From `beacon.send` to `beacon.recv`, every `beacon.interval`, carrying
/// this build's version, the configuration's and the configuration's digest:
/// the same beacon a version 1 vehicle sends, by the same code. An interval of
/// nought sends none, as it does in version 1. Recorded in the telemetry
/// log, which is the vehicle's own account of what it sent.
fn beacons(
    graph: &Graph,
    config_version: ConfigVersion,
    digest: ConfigDigest,
    telemetry_log: TelemetryLog,
) -> Result<Option<BeaconSend>, String> {
    let Some(section) = graph.tcspecial.as_ref() else {
        return Ok(None);
    };
    let ci = tcslibgs::graph::parameters::ci_config_of(&tcslibgs::graph::parameters::flatten(
        section,
    ))?
    .to_ci_config()?;
    let sending = BeaconSend::new(
        Duration::from_millis(u64::from(ci.beacon_interval.0)),
        ci.beacon_send.clone(),
        ci.beacon_recv.clone(),
        telemetry_log,
        ConfigVersion::of_this_build(),
        config_version,
        digest,
    );
    if sending.is_some() {
        eprintln!(
            "{} ms beacon: {} to {}",
            ci.beacon_interval.0, ci.beacon_send, ci.beacon_recv
        );
    }
    Ok(sending)
}

/// Whether the loop should stop.
pub fn asked_to_stop() -> bool {
    STOPPING.load(Ordering::Relaxed)
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

/// Wait, but not past the next pass.
fn pause(waiting: Duration) {
    // A nap and not a poll on the descriptors. What that costs is the
    // staleness of a reading from a node that is polled rather than
    // scheduled, which `POLL_PAUSE` bounds; what it buys is a loop with no
    // missed-wakeup failure mode at all, which for the first version of a
    // flight loop is the right side of that trade. A wait on the descriptors
    // belongs here when something needs it, and the ports hand out their
    // descriptors for exactly that.
    if !waiting.is_zero() {
        std::thread::sleep(waiting);
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

/// Whether this file is a version 2 configuration.
///
/// What decides which of the two programs runs: the file says which it is, as
/// it does for the ground tool. A file that is neither is reported by whichever
/// reader was asked for it, which can say what is wrong with it.
pub fn is_a_graph(path: &str) -> bool {
    tcslibgs::graph::is_version_2(std::path::Path::new(path))
}

/// Run the configuration in `path` and say what to exit with.
///
/// For `main`, which has one more thing to do than report: a vehicle that
/// could not start must not look as though it is running.
pub fn run_or_report(path: &str) -> i32 {
    match run_graph(path) {
        Ok(()) => 0,
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
            "version: \"2.0\"\n\
             payloads:\n\
             \x20 - { name: a, type: i2c, path: /dev/i2c-1, address: \"0x48\", \
             payload-out: nobody }\n\
             vehicle:\n\
             \x20 - { name: a, type: compute, function: mean }\n",
        )
        .expect("the file is written");

        let problems =
            run_graph(path.to_str().expect("a path")).expect_err("a configuration with faults");
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

    /// A file that is not a graph is not run as one.
    #[test]
    fn a_file_that_is_not_a_graph_is_not_one() {
        let where_ = tempfile::tempdir().expect("a directory");

        let v1 = where_.path().join("payloads.yaml");
        std::fs::write(&v1, "version: \"1.0\"\ndescription: a set\npayloads: []\n")
            .expect("the file is written");
        assert!(!is_a_graph(v1.to_str().expect("a path")));

        let v2 = where_.path().join("graph.yaml");
        std::fs::write(&v2, "version: \"2.0\"\nnodes: []\nlinks: []\n")
            .expect("the file is written");
        assert!(is_a_graph(v2.to_str().expect("a path")));

        // And a file that is not there, which is not a graph either: the
        // reader that is asked for it says what is wrong with it.
        assert!(!is_a_graph("/nonesuch/nowhere.yaml"));
    }

    /// The shipped example is a graph, and is checked without complaint.
    #[test]
    fn the_shipped_example_is_a_checked_graph() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let path = root.join("tests/manual/thermostat.yaml");
        let path = path.to_str().expect("a path");

        assert!(is_a_graph(path));
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
