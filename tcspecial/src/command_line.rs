//! The TCSpecial Flight Executive's entry point: the flight software, as a
//! program.
//!
//! Reads the configuration it is given, says what it read, and runs the graph
//! that configuration describes: see [`tcspecial::runtime::start`].
//!
//! `-q` or `--quiet` leaves the startup summary unsaid and logs warnings and
//! worse rather than information, for a program that starts tcspecial to
//! show something else. What stops a start is said either way.
//!
//! `--visual` opens a window showing the graph as tcs-cfge draws it, each link
//! lit as data crosses it, in a tcspecial built with the `visual` feature;
//! one built without it says so and exits. Never in flight: see
//! `tcspecial::visual`. It is refused under `--simulate=none`, the flight
//! setting and the one a run without `--simulate` gets.
//!
//! `--simulate=<mode>` is the simulation mode, `none`, `some` or `all`: see
//! `tcsprotocol::simulate`. Each payload is judged a real, a dummy or a
//! simulated device before anything is opened, and a run whose payloads the
//! mode does not allow is refused: see `tcspecial::runtime::start`.
//!
//! Bit errors on the link to the ground are asked for in the simulator
//! file's `gateway` section, not on the command line, and applied by a
//! tcspecial built with the `bit-errors` feature. Never in flight: see
//! `tcspecial::runtime::bit_errors`. Refused under `--simulate=none`, as
//! `--visual` is.
//!
//! Every check that no test code is flying is made in
//! `tcspecial::flight_checks`, before anything is read, and what it found
//! goes in every beacon.
//!
//! `--no-memlock` runs the vehicle without locking its memory into RAM,
//! under any `--simulate`. Without it the vehicle's memory is always locked,
//! and a lock that fails is said and run without in flight, and stops
//! tcspecial under `--simulate=some` or `all`: see
//! `tcspecial::runtime::memory`.
//!
//! `--types` prints the payload types this build contains, separated by
//! commas, and exits: what `tcs-verify --types=` takes, so that a
//! configuration can be checked on the ground against the binary that will
//! fly, with `tcs-verify --types=$(tcspecial --types) <configuration>`.
//!
//! The binary is no more than a call of [`main`], so that a web page, which
//! starts its programs as functions, can start tcspecial too.

use tcsos::env;
use tcsos::process;

use crate::runtime;
use log::{debug, error, trace, warn};
use tcsprotocol::config::{payload_path_from_args, PAYLOAD_CONFIG_PATH_VAR};

/// What `--help` prints.
const USAGE: &str = "\
usage: tcspecial [options] [configuration]

  configuration      the .ytcs or .xtcs file to fly (default: the
                     PAYLOAD_CONFIG_PATH environment variable, or the
                     build's default)
  --simulate=MODE    none (the default, the flight setting), some or all
  --no-memlock       run without locking memory into RAM
  -q, --quiet        no startup summary; warnings and worse only
  --visual           show the graph in a window (a build with the visual
                     feature, under --simulate=some or all)
  --types            print the payload types this build contains, and exit
  -h, --help         print this, and exit
";

/// Runs tcspecial as the command line asks, as the module documentation
/// says, and exits with the run's status; 1 for a command line,
/// configuration or build that refuses it, with the reason said.
pub fn main() {
    // The process that was started is the supervisor, which starts this
    // same program as its child to run the vehicle: see
    // `runtime::supervisor`. A child ends when its supervisor does, and
    // says so first, before anything is made.
    let child = runtime::supervisor::is_child();
    if child {
        // First, so that as little of the main thread's stack is in use as
        // can be when its depth starts to be measured.
        runtime::thread::watch_main();
        runtime::supervisor::child_follows_supervisor();

        // Before any thread is made, so that each allocates from the main
        // thread's malloc arena rather than reserving its own. Not with the
        // window, whose memory has no counterpart in flight, so that there
        // is nothing to make like flight: its threads have arenas of their
        // own, as glibc gives them.
        if !env::args().skip(1).any(|arg| arg == "--visual") {
            runtime::thread::one_arena();
        }

        // And before any thread is made, so that every thread inherits the
        // block on SIGINT and SIGTERM and one thread of their own takes
        // them: a stop that arrives as a RESTART is under way is then not
        // lost. See `runtime::signals`. The supervisor blocks them itself
        // and takes them in its own loop.
        runtime::signals::start(runtime::start::stop_as_signalled);
    }

    // How it is run, on stdout and nothing else done, before anything is
    // read: a flag it does not know would otherwise be taken for the name
    // of a configuration.
    if env::args()
        .skip(1)
        .any(|arg| arg == "-h" || arg == "--help")
    {
        print!("{USAGE}");
        return;
    }

    // What this build contains, asked of the binary rather than recalled
    // from how it was built: on stdout, because it is the answer, and nothing
    // else is done. Before anything is read, so that it can be asked of a
    // tcspecial with no configuration to hand.
    if env::args().skip(1).any(|arg| arg == "--types") {
        println!("{}", crate::node::payload::BUILT.join(","));
        return;
    }

    // The flags that change how it runs, taken out before the configuration
    // is read from what is left, so that they may come before or after the
    // file.
    let (quiet, args): (Vec<String>, Vec<String>) =
        env::args().partition(|arg| arg == "-q" || arg == "--quiet");
    let (visual, args): (Vec<String>, Vec<String>) =
        args.into_iter().partition(|arg| arg == "--visual");
    let visual = !visual.is_empty();
    let (no_memlock, args): (Vec<String>, Vec<String>) =
        args.into_iter().partition(|arg| arg == "--no-memlock");
    let memlock = no_memlock.is_empty();
    let (simulate, args) = match tcsprotocol::simulate::take_mode(args) {
        Ok(taken) => taken,
        Err(e) => {
            eprintln!("tcspecial: {e}");
            process::exit(1);
        }
    };
    let quiet = !quiet.is_empty();
    if quiet {
        runtime::start::be_quiet();
    }

    // Default to info so that the startup messages below, which used to
    // be printed unconditionally, still appear; warn where quiet. RUST_LOG
    // overrides either; RUST_LOG=trace adds the per-datagram tracing in the
    // main loop.
    let level = if quiet { "warn" } else { "info" };
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(level),
    )
    // Where a logger is set already -- a web page's, which shows every
    // program's log in its console -- that one is used.
    .try_init()
    .ok();

    trace!("main: entered");

    // A build with test code in it is never one to fly, and says so every
    // time it starts, whether or not the test code is asked for: a warning,
    // so that quiet says it too. Its beacons say `test`.
    let never = crate::flight_checks::never_flies();
    if !never.is_empty() {
        warn!(
            "this tcspecial was built with {} and must not fly: build the \
             flight software without {}",
            never.join(" and "),
            if never.len() == 1 { "it" } else { "them" }
        );
    }

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
    let payload_path = match payload_path_from_args(
        args.into_iter(),
        Some(PAYLOAD_CONFIG_PATH_VAR),
    ) {
        Ok(payload_path) => payload_path,
        Err(e) => {
            error!("{}", e);
            process::exit(1);
        }
    };

    // Every check that no test code is flying, before the configuration is
    // read or anything opened, and so before the first beacon, which says
    // what they found: test code asked of a run in the flight setting, or
    // of a build without it, is refused. The flight setting is what a run
    // that leaves --simulate out gets, so test code there would be test
    // code in flight. See crate::flight_checks.
    let bit_errors =
        crate::flight_checks::bit_errors_for(&payload_path, simulate)
            .unwrap_or_else(|why| {
                eprintln!("tcspecial: {why}");
                process::exit(1);
            });
    let asked = crate::flight_checks::Asked { visual, bit_errors };
    let checked = match crate::flight_checks::check(simulate, &asked) {
        Ok(checked) => checked,
        Err(refused) => {
            for why in refused {
                eprintln!("tcspecial: {why}");
            }
            process::exit(1);
        }
    };
    // The supervisor refuses a configuration with problems once, as the
    // child would, rather than starting a child on it that fails, and then
    // supervises. The startup summary is the child's.
    //
    // Which file was named is the summary's first line, said once, by the
    // supervisor, and before the file is read.
    let built = crate::node::payload::BUILT;
    if !child {
        runtime::start::summary(format!(
            "payload configuration: {payload_path}"
        ));

        // Before the configuration is checked, so that a node refused because
        // this build lacks its type is read beside the list of what it has.
        runtime::start::summary(format!(
            "payload types built: {}",
            if built.is_empty() {
                "none but the network type".to_string()
            } else {
                built.join(", ")
            }
        ));
        if refused(&payload_path, built) {
            process::exit(runtime::supervisor::BAD);
        }
        process::exit(runtime::supervisor::run(&payload_path, simulate));
    }

    // The test code armed, said by the child alone: the supervisor made the
    // same checks, but sends nothing, and under `seed: random` drew a seed
    // of its own that no packet is lost by.
    if let Some(armed) = crate::flight_checks::armed() {
        runtime::start::summary(armed);
    }

    // Locked before the configuration is read, so that everything a load
    // makes is locked as it is made: see `runtime::memory`.
    match runtime::memory::lock(memlock, simulate) {
        Ok(runtime::memory::Locked::Locked) => {
            runtime::start::summary("memory: locked");
        }
        Ok(runtime::memory::Locked::NotAsked) => {
            runtime::start::summary("memory: not locked, as --no-memlock asks");
        }
        Ok(runtime::memory::Locked::Unlocked(why)) => {
            error!("running unlocked: {why}");
            runtime::start::summary(format!("memory: not locked: {why}"));
        }
        Err(why) => {
            eprintln!(
                "tcspecial: {why}. Under --simulate={simulate} a lock that \
                 fails stops tcspecial, since aboard it would mean a \
                 vehicle whose memory is not held"
            );
            process::exit(runtime::supervisor::FATAL);
        }
    }

    if refused(&payload_path, built) {
        process::exit(runtime::supervisor::BAD);
    }

    // The configuration is a graph, which is the only thing it can be: see
    // `runtime::start`. A file that is not one is refused above, by the same
    // checks tcs-verify runs, so by here there is nothing to choose between.
    //
    // With the window, where one was asked for: the vehicle on a thread of
    // its own beside it, and each stopping the other. See tcspecial::visual.
    #[cfg(feature = "visual")]
    if visual {
        process::exit(crate::visual::run(&payload_path, simulate, checked));
    }
    process::exit(runtime::start::run_or_report(
        &payload_path,
        simulate,
        checked,
    ));
}

/// Says what tcs-verify says about the configuration at `payload_path`, and
/// whether that refuses it.
///
/// Asked before anything else is read from it and said in tcs-verify's own
/// words: every problem at once, each with its file and line, rather than
/// whichever of them a loader happened to meet first. The same lines
/// tcs-moc, tcs-sim and tcs-cfge show as they start.
///
/// Refused on a warning as well: a node whose attributes are not all in is
/// a node that cannot be opened, so aboard it is not a question of degree
/// -- see `tcsprotocol::graph::check::Severity`.
///
/// Against the types this build contains, so that a configuration with a
/// node of another type is refused here, before anything is opened, in the
/// words tcs-verify's `--types` uses on the ground.
///
/// # Arguments
///
/// * `payload_path` - The configuration on the command line.
/// * `built`        - The payload types this build contains.
///
/// # Returns
///
/// Whether it is refused, each problem having been logged and said on
/// stderr.
fn refused(payload_path: &str, built: &[&str]) -> bool {
    let report = tcsverify::verify_built(
        std::path::Path::new(payload_path),
        None,
        tcsverify::Limits::EVERYTHING,
        built,
    );
    for (_, line) in report.lines() {
        error!("{}", line);
        // On stderr too, because a vehicle that will not start is read by
        // whoever is standing there and not through RUST_LOG.
        eprintln!("{}", line);
    }
    if report.found_anything() {
        error!("{}: not started", payload_path);
        return true;
    }
    false
}
