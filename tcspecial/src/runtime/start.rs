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

use log::{error, info, warn};
use tcslibgs::graph::Graph;

use crate::runtime::fabric::Moved;
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
    eprintln!("{path}: {} nodes", graph.nodes.len());
    for (name, what) in described(&graph) {
        eprintln!("  {name}: {what}");
    }

    let mut system = System::of(&graph, Instant::now())?;
    info!("{path}: {system:?}");

    listen_for_stop();

    system.run(&asked_to_stop, report, pause);

    info!("{path}: stopped");
    Ok(())
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
             to_moc: nobody }\n\
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
