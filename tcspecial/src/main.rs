//! TCSpecial main entry point: the flight software, as a program.
//!
//! Reads the configuration it is given, says what it read, and runs the graph
//! that configuration describes: see [`tcspecial::runtime::start`].
//!
//! `-q` or `--quiet` leaves the startup summary unsaid and logs warnings and
//! worse rather than information, for a program that starts tcspecial to
//! show something else. What stops a start is said either way.

use std::env;
use std::process;

use log::{debug, error, trace};
use tcslibgs::config::{payload_path_from_args, PAYLOAD_CONFIG_PATH_VAR};
use tcspecial::runtime;

fn main() {
    // Before anything is opened, so that what a RESTART keeps is what this
    // process was started with and nothing it opened itself.
    runtime::restart::note_how_started();

    // The one flag, taken out before the configuration is read from what is
    // left, so that it may come before or after the file.
    let (quiet, args): (Vec<String>, Vec<String>) =
        env::args().partition(|arg| arg == "-q" || arg == "--quiet");
    let quiet = !quiet.is_empty();
    if quiet {
        runtime::start::be_quiet();
    }

    // Default to info so that the startup messages below, which used to
    // be printed unconditionally, still appear; warn where quiet. RUST_LOG
    // overrides either; RUST_LOG=trace adds the per-datagram tracing in the
    // main loop.
    let level = if quiet { "warn" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level)).init();

    trace!("main: entered");

    debug!("starting");

    // One file. Where commands are taken, where beacons go and where the log
    // is written are all in the configuration's `tcspecial` section, which it
    // must have, so there is nothing left for a second file to answer, and a
    // second file could only disagree with the one being flown.
    //
    // The startup summary is said on stderr, plainly and one fact a line --
    // `label: value` -- whether or not anyone set RUST_LOG: it is what
    // somebody watching a vehicle start compares against the file they think
    // it is reading. The log is for what happens after.

    // Named on the command line, or by PAYLOAD_CONFIG_PATH, or the default.
    // It is the one thing the command line
    // says: where commands are taken and where beacons go are both the
    // configuration file's, so each address is written in one place.
    let payload_path = match payload_path_from_args(args.into_iter(), Some(PAYLOAD_CONFIG_PATH_VAR))
    {
        Ok(payload_path) => payload_path,
        Err(e) => {
            error!("{}", e);
            process::exit(1);
        }
    };
    runtime::start::summary(format!("payload configuration: {payload_path}"));

    // What tcsverify says about this file, asked before anything else is read
    // from it and said in tcsverify's own words: every problem at once, each
    // with its file and line, rather than whichever of them a loader below
    // happened to meet first. The same lines tcsmoc, tcssim and tcsgui show
    // as they start.
    //
    // Refused on a warning as well: a node whose attributes are not all in is
    // a node that cannot be opened, so aboard it is not a question of degree
    // -- see `tcslibgs::graph::check::Severity`.
    let report = tcsverify::verify(
        std::path::Path::new(&payload_path),
        None,
        tcsverify::Limits::EVERYTHING,
    );
    for (_, line) in report.lines() {
        error!("{}", line);
        // On stderr too, because a vehicle that will not start is read by
        // whoever is standing there and not through RUST_LOG.
        eprintln!("{}", line);
    }
    if report.found_anything() {
        error!("{}: not started", payload_path);
        process::exit(1);
    }

    // The configuration is a graph, which is the only thing it can be: see
    // `runtime::start`. A file that is not one is refused above, by the same
    // checks tcsverify runs, so by here there is nothing to choose between.
    process::exit(runtime::start::run_or_report(&payload_path));
}
