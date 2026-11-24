//! The supervisor: the process that is started, which starts a child to run
//! the vehicle, waits for it, and decides from how it ended what to start
//! next.
//!
//! tcspecial is two processes. The supervisor chooses nothing and opens no
//! node: it holds the instance lock on the state file, starts a child -- this
//! same program, told by [`CHILD_VAR`] that it is one -- with the arguments it
//! was given, passes SIGINT and SIGTERM on to it, and waits. The child is
//! the tcspecial of everything else: it runs the top of the configuration
//! stack and exits with one of the statuses below. Every attempt is a fresh
//! child, so that nothing a failed configuration did -- memory it took and
//! did not give back, a descriptor it left open, a thread it left stuck --
//! is there for the next.
//!
//! What the supervisor does next, by how the child ended:
//!
//! * [`GOOD`], having been told to stop, or SIGINT or SIGTERM, which ends
//!   only a child about to exit for a restart, a stop that arrived as it
//!   went: it stops too.
//! * [`RESTART`], having taken a RESTART, which pushed what it named: it
//!   starts a child on the new top.
//! * [`FATAL`]: it stops, saying so, the stack unchanged.
//! * [`BAD`], any other status, or any other signal: it pops the top,
//!   unless it is the bottom, and starts a child on what is then the top.
//!   The bottom, which nothing pops, is started again after a delay that
//!   doubles at each failure; on a desk, in a simulated run, a bottom that
//!   fails stops the supervisor instead, since nothing on a desk is helped
//!   by trying for ever.
//!
//! Before each child the supervisor counts an attempt on the top, in the
//! state file, and pops a top whose attempts have reached `load_trials`:
//! a power loss or a hung machine ends the supervisor too, and only a count
//! kept on storage gives such a configuration up. A matching CONNECT sets
//! the count back to nought. See "The supervisor and the configuration
//! stack" in `docs/tcspecial-design.rst`.
//!
//! Single-threaded, and allocating nothing while it waits: what it starts
//! children with is made before the first. Between children it reads and
//! writes the state file, which allocates, as every change to that file
//! does, once for each child.

use std::path::Path;
use tcsos::time::{Duration, Instant};

use log::{error, info, warn};
use tcsos::child::{Ended, Spawner};
use tcsprotocol::simulate::Mode;

use crate::runtime::choose::Boot;
use crate::state::{self, State};

/// The child ran and was told to stop.
pub const GOOD: i32 = 0;
/// The child failed: a configuration that would not load, check or open,
/// or a vehicle that ended without being told to. Any status not named here
/// is taken as this.
pub const BAD: i32 = 1;
/// The child cannot run, and no other configuration would help: a memory
/// lock that failed in a simulated run, a command line that is not one.
pub const FATAL: i32 = 2;
/// The child took a RESTART, and the state file's top is what it named.
pub const RESTART: i32 = 3;
/// The child ran out of memory once its memory was locked, in flight: as
/// [`BAD`] for every configuration but the command line's, whose next child
/// is started unlocked, there being none beneath it to need less. See
/// `runtime::memory`.
pub const SHORT: i32 = 4;

/// The environment variable that tells tcspecial it is a supervisor's child,
/// holding the supervisor's process id.
pub const CHILD_VAR: &str = "TCSPECIAL_SUPERVISOR";

/// The least a bottom that fails waits before it is started again.
const FIRST_DELAY: Duration = Duration::from_secs(1);
/// The most it waits, however often it has failed.
const MOST_DELAY: Duration = Duration::from_secs(60);
/// How long a bottom must stay up for the delay to go back to the least.
const STAYED_UP: Duration = Duration::from_secs(60);
/// How long the wait for a child or a signal lasts before it looks again.
const LOOK: Duration = Duration::from_millis(200);

/// Whether this process is a supervisor's child, and so the tcspecial that
/// runs the vehicle.
///
/// # Returns
///
/// Whether [`CHILD_VAR`] is set.
pub fn is_child() -> bool {
    tcsos::env::var_os(CHILD_VAR).is_some()
}

/// Makes this child end when its supervisor does, so that no child runs
/// without one: a SIGTERM, which the child takes as a stop. And if the
/// supervisor has ended already, between starting it and now, ends at once.
///
/// The first thing a child does.
pub fn child_follows_supervisor() {
    let supervisor = tcsos::env::var(CHILD_VAR)
        .ok()
        .and_then(|said| said.parse::<u32>().ok());
    if !tcsos::child::follow_parent(supervisor) {
        eprintln!("tcspecial: its supervisor has ended, so it does not start");
        tcsos::process::exit(BAD);
    }
}

/// What the supervisor does before a child starts: counts an attempt on the
/// top, or pops a top whose attempts have run out and counts one on the one
/// beneath.
///
/// # Arguments
///
/// * `state`       - The state file's stack, changed.
/// * `load_trials` - How many attempts a stacked configuration is given.
///
/// # Returns
///
/// The top given up, where one was.
fn before_child(state: &mut State, load_trials: u32) -> Option<String> {
    let mut given_up = None;
    if let Some(top) = state.top() {
        if state.trials >= load_trials {
            state.stack.pop();
            state.trials = 0;
            given_up = Some(format!(
                "configuration {top} is given up: {load_trials} children \
                 were started on it without a matching CONNECT, which is \
                 load_trials, so it is popped from the configuration stack"
            ));
        }
    }
    if state.top().is_some() {
        state.trials = state.trials.saturating_add(1);
    }
    given_up
}

/// What the supervisor does after a child that failed: pops the top, unless
/// it is the bottom.
///
/// # Arguments
///
/// * `state` - The state file's stack, changed.
///
/// # Returns
///
/// The configuration popped, or `None` where the top is the bottom.
fn after_bad(state: &mut State) -> Option<tcsprotocol::ConfigDigest> {
    let popped = state.stack.pop();
    if popped.is_some() {
        state.trials = 0;
    }
    popped
}

/// Says whether a child that ended so was stopped.
///
/// SIGINT and SIGTERM are taken by a thread of the child's own, and end it
/// only once it is about to exit for a restart, when a stop that arrives
/// must be turned into one rather than lost: see
/// [`crate::runtime::signals::before_exec`].
///
/// # Arguments
///
/// * `ended` - How it ended.
///
/// # Returns
///
/// Whether it exited with [`GOOD`] or was ended by SIGINT or SIGTERM.
fn stopped(ended: Ended) -> bool {
    ended == Ended::Exited(GOOD) || ended.by_a_stop()
}

/// Changes the state file at `path`, saying in the log where it cannot be.
///
/// # Arguments
///
/// * `path`   - The state file.
/// * `change` - What to do to what it holds.
///
/// # Returns
///
/// What `change` returned, where the file was changed.
fn changed<T>(path: &Path, change: impl FnOnce(&mut State) -> T) -> Option<T> {
    let mut out = None;
    match state::update(path, |state| out = Some(change(state))) {
        Ok(_) => out,
        Err(why) => {
            error!("supervisor: {why}");
            None
        }
    }
}

/// Supervises tcspecial on the configuration `path`, under `mode`, until a
/// child stops for good.
///
/// # Arguments
///
/// * `path` - The configuration on the command line.
/// * `mode` - The run's simulation mode.
///
/// # Returns
///
/// The exit status: 0 where the last child was told to stop and stopped, 1
/// otherwise.
pub fn run(path: &str, mode: Mode) -> i32 {
    // Blocked before anything else, so that a stop sent from now on waits
    // to be taken here, or by the child, which starts with them blocked.
    tcsos::child::block_stops_and_children();

    let boot = match Boot::of_command_line(Path::new(path)) {
        Ok((boot, _)) => boot,
        Err(problems) => {
            for problem in &problems {
                error!("{problem}");
                eprintln!("{problem}");
            }
            return BAD;
        }
    };
    if let Some(state_file) = &boot.state_file {
        if let Err(why) = crate::exclusive::state_file(state_file) {
            error!("{path}: {why}");
            eprintln!("{path}: {why}");
            return BAD;
        }
    }
    let spawning = match Spawner::new(CHILD_VAR, "--no-memlock") {
        Ok(spawning) => spawning,
        Err(why) => {
            error!("supervisor: no child can be started: {why}");
            eprintln!("tcspecial: no child can be started: {why}");
            return BAD;
        }
    };

    let mut delay = FIRST_DELAY;
    // Whether the next child is to run unlocked: once the command line's
    // configuration has run out of memory with it locked, until a RESTART
    // brings another.
    let mut unlocked = false;
    loop {
        if let Some(state_file) = &boot.state_file {
            if let Some(Some(why)) = changed(state_file, |state| {
                before_child(state, boot.load_trials)
            }) {
                warn!("supervisor: {why}");
            }
        }

        let started = Instant::now();
        let pid = match spawning.start(unlocked) {
            Ok(pid) => pid,
            Err(e) => {
                error!("supervisor: no child can be started: {e}");
                if !mode.is_flight() || tcsos::child::pause(delay) {
                    return BAD;
                }
                delay = (delay * 2).min(MOST_DELAY);
                continue;
            }
        };
        info!("supervisor: child {pid} started");
        let mut stopping = false;
        let ended = tcsos::child::wait_for(&pid, LOOK, &mut stopping);
        if stopping {
            info!("supervisor: stopped, child {pid} {ended:?}");
            return if stopped(ended) { GOOD } else { BAD };
        }
        if stopped(ended) {
            if ended != Ended::Exited(GOOD) {
                info!(
                    "supervisor: child {pid} {ended:?} as it went to restart: \
                     stopping"
                );
            }
            return GOOD;
        }
        match ended {
            Ended::Exited(RESTART) => {
                info!("supervisor: child {pid} took a RESTART");
                delay = FIRST_DELAY;
                unlocked = false;
                continue;
            }
            Ended::Exited(FATAL) => {
                error!(
                    "supervisor: child {pid} cannot run, and no other \
                     configuration would help: stopping"
                );
                return BAD;
            }
            ended => {
                let popped = boot
                    .state_file
                    .as_deref()
                    .and_then(|state_file| changed(state_file, after_bad))
                    .flatten();
                match popped {
                    // The bottom has nothing beneath it that needs less, so
                    // it is run unlocked rather than not at all.
                    None if ended == Ended::Exited(SHORT) && !unlocked => {
                        warn!(
                            "supervisor: child {pid} ran out of memory on the \
                             command line's configuration, which has nothing \
                             beneath it: starting it again unlocked"
                        );
                        unlocked = true;
                        delay = FIRST_DELAY;
                    }
                    Some(popped) => {
                        warn!(
                            "supervisor: child {pid} failed ({ended:?}), so \
                             configuration {popped} is popped from the \
                             configuration stack"
                        );
                        delay = FIRST_DELAY;
                    }
                    None if !mode.is_flight() => {
                        error!(
                            "supervisor: child {pid} failed ({ended:?}) on \
                             the command line's configuration, which has \
                             nothing beneath it: stopping, as a desk does"
                        );
                        return BAD;
                    }
                    None => {
                        if started.elapsed() >= STAYED_UP {
                            delay = FIRST_DELAY;
                        }
                        warn!(
                            "supervisor: child {pid} failed ({ended:?}) on \
                             the command line's configuration, which has \
                             nothing beneath it: starting it again in {} s",
                            delay.as_secs()
                        );
                        if tcsos::child::pause(delay) {
                            return GOOD;
                        }
                        delay = (delay * 2).min(MOST_DELAY);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcsprotocol::ConfigDigest;

    const ONE: ConfigDigest = ConfigDigest([0x11; 16]);
    const TWO: ConfigDigest = ConfigDigest([0x22; 16]);

    /// Each child on a stacked top is counted, and a top whose attempts
    /// have reached load_trials is popped, the one beneath counted in its
    /// place; the bottom is never counted.
    #[test]
    fn a_top_that_keeps_failing_unheard_is_given_up() {
        let mut state = State {
            stack: vec![ONE, TWO],
            trials: 0,
            commanded: false,
        };
        assert_eq!(before_child(&mut state, 2), None);
        assert_eq!(before_child(&mut state, 2), None);
        assert_eq!((state.top(), state.trials), (Some(TWO), 2));
        let why = before_child(&mut state, 2).expect("given up");
        assert!(why.contains(&TWO.to_string()), "{why}");
        assert_eq!((state.top(), state.trials), (Some(ONE), 1));

        let mut bottom = State::default();
        assert_eq!(before_child(&mut bottom, 1), None);
        assert_eq!(bottom.trials, 0, "the bottom is never counted");
    }

    /// A child that fails pops the top, and the bottom is never popped.
    #[test]
    fn a_failure_pops_the_top_and_never_the_bottom() {
        let mut state = State {
            stack: vec![ONE, TWO],
            trials: 3,
            commanded: false,
        };
        assert_eq!(after_bad(&mut state), Some(TWO));
        assert_eq!((state.top(), state.trials), (Some(ONE), 0));
        assert_eq!(after_bad(&mut state), Some(ONE));
        assert_eq!(after_bad(&mut state), None);
        assert!(state.stack.is_empty());
    }

    /// A good exit, SIGINT and SIGTERM are stops; any other signal, and
    /// any other status, are not.
    #[test]
    fn a_stop_signal_is_a_stop() {
        assert!(stopped(Ended::Exited(GOOD)));
        use tcsos::child::signal;

        assert!(stopped(Ended::Signalled(signal::TERMINATE)));
        assert!(stopped(Ended::Signalled(signal::INTERRUPT)));
        assert!(!stopped(Ended::Signalled(signal::SEGMENTATION)));
        assert!(!stopped(Ended::Signalled(signal::KILL)));
        assert!(!stopped(Ended::Exited(RESTART)));
        assert!(!stopped(Ended::Exited(BAD)));
    }

    /// The four statuses are four numbers.
    #[test]
    fn the_statuses_are_distinct() {
        let all = [GOOD, BAD, FATAL, RESTART];
        for (at, one) in all.iter().enumerate() {
            assert!(!all[at + 1..].contains(one), "{one} twice");
        }
    }
}
