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
//! * [`GOOD`], having been told to stop: it stops too.
//! * [`RESTART`], having taken a RESTART, which pushed what it named: it
//!   starts a child on the new top.
//! * [`FATAL`]: it stops, saying so, the stack unchanged.
//! * [`BAD`], any other status, or a signal: it pops the top, unless it is
//!   the bottom, and starts a child on what is then the top. The bottom,
//!   which nothing pops, is started again after a delay that doubles at
//!   each failure; on a desk, under `--simulate`, a bottom that fails stops
//!   the supervisor instead, since nothing on a desk is helped by trying
//!   for ever.
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

use std::ffi::{CString, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use log::{error, info, warn};
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
/// lock that failed under `--simulate`, a command line that is not one.
pub const FATAL: i32 = 2;
/// The child took a RESTART, and the state file's top is what it named.
pub const RESTART: i32 = 3;

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
    std::env::var_os(CHILD_VAR).is_some()
}

/// Makes this child end when its supervisor does, so that no child runs
/// without one: a SIGTERM, which the child takes as a stop. And if the
/// supervisor has ended already, between starting it and now, ends at once.
///
/// The first thing a child does.
pub fn child_follows_supervisor() {
    // SAFETY: prctl with a signal number, and getppid, take and return
    // integers only.
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM as libc::c_ulong);
    }
    let supervisor = std::env::var(CHILD_VAR)
        .ok()
        .and_then(|said| said.parse::<libc::pid_t>().ok());
    // SAFETY: as above.
    if supervisor.is_some_and(|pid| unsafe { libc::getppid() } != pid) {
        eprintln!("tcspecial: its supervisor has ended, so it does not start");
        std::process::exit(BAD);
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

/// The two stop signals and SIGCHLD, as a set.
///
/// # Arguments
///
/// * `child` - Whether SIGCHLD is in it.
///
/// # Returns
///
/// The set.
fn signals(child: bool) -> libc::sigset_t {
    // SAFETY: a set made empty and then given signals, on the stack.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::sigaddset(&mut set, libc::SIGTERM);
        if child {
            libc::sigaddset(&mut set, libc::SIGCHLD);
        }
        set
    }
}

/// What every child is started with, made once.
struct Spawning {
    /// The program, this one.
    program: CString,
    /// Its arguments, its own name first, as this process was given them:
    /// held for `argv` to point into.
    _args: Vec<CString>,
    /// This process's environment, with [`CHILD_VAR`] added: held for
    /// `envp` to point into.
    _env: Vec<CString>,
    /// The arguments, as `posix_spawn` takes them, ending in a null.
    argv: Vec<*mut libc::c_char>,
    /// The environment, as `posix_spawn` takes it, ending in a null.
    envp: Vec<*mut libc::c_char>,
}

impl Spawning {
    /// Makes what every child is started with.
    ///
    /// # Returns
    ///
    /// It.
    ///
    /// # Errors
    ///
    /// Why not: a nul in the program's name, an argument or the
    /// environment.
    fn new() -> Result<Spawning, String> {
        let program = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("/proc/self/exe"));
        let c = |said: &[u8]| {
            CString::new(said).map_err(|_| {
                "a nul in the program's name, an argument or the environment"
                    .to_string()
            })
        };
        let args = std::env::args_os()
            .map(|arg: OsString| c(arg.as_bytes()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut env = std::env::vars_os()
            .filter(|(name, _)| name != CHILD_VAR)
            .map(|(name, value)| {
                let mut pair = name.as_bytes().to_vec();
                pair.push(b'=');
                pair.extend_from_slice(value.as_bytes());
                c(&pair)
            })
            .collect::<Result<Vec<_>, _>>()?;
        env.push(c(format!("{CHILD_VAR}={}", std::process::id()).as_bytes())?);
        // Pointers into the strings, whose bytes stay where they are when
        // the vectors holding them move.
        let pointers = |all: &[CString]| {
            all.iter()
                .map(|one| one.as_ptr() as *mut libc::c_char)
                .chain(std::iter::once(std::ptr::null_mut()))
                .collect::<Vec<_>>()
        };
        Ok(Spawning {
            program: c(program.as_os_str().as_bytes())?,
            argv: pointers(&args),
            envp: pointers(&env),
            _args: args,
            _env: env,
        })
    }

    /// Starts a child, with SIGINT and SIGTERM blocked in it as they are
    /// here, so that one sent before its signal thread runs is taken by
    /// that thread rather than ending it, and SIGCHLD not.
    ///
    /// # Returns
    ///
    /// The child's process id.
    ///
    /// # Errors
    ///
    /// What `posix_spawn` failed with.
    fn start(&self) -> std::io::Result<libc::pid_t> {
        let mask = signals(false);
        let mut pid: libc::pid_t = 0;
        // SAFETY: the attributes are made, used and destroyed here; the
        // program, the arguments, the environment and the vectors of them
        // are nul-terminated and outlive the call, which copies them. Every
        // descriptor tcspecial opens is closed on exec, so the child is
        // handed what this process was handed and nothing else.
        let started = unsafe {
            let mut attr: libc::posix_spawnattr_t = std::mem::zeroed();
            libc::posix_spawnattr_init(&mut attr);
            libc::posix_spawnattr_setsigmask(&mut attr, &mask);
            libc::posix_spawnattr_setflags(
                &mut attr,
                libc::POSIX_SPAWN_SETSIGMASK as libc::c_short,
            );
            let started = libc::posix_spawn(
                &mut pid,
                self.program.as_ptr(),
                std::ptr::null(),
                &attr,
                self.argv.as_ptr(),
                self.envp.as_ptr(),
            );
            libc::posix_spawnattr_destroy(&mut attr);
            started
        };
        if started != 0 {
            return Err(std::io::Error::from_raw_os_error(started));
        }
        Ok(pid)
    }
}

/// How a child ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ended {
    /// It exited with this status.
    Exited(i32),
    /// A signal ended it.
    Signalled(i32),
}

/// Waits for `pid` to end, passing SIGINT and SIGTERM on to it.
///
/// # Arguments
///
/// * `pid`      - The child.
/// * `stopping` - Set where a stop signal was taken, and passed on.
///
/// # Returns
///
/// How it ended.
fn wait_for(pid: libc::pid_t, stopping: &mut bool) -> Ended {
    let set = signals(true);
    let look = libc::timespec {
        tv_sec: 0,
        tv_nsec: LOOK.as_nanos() as libc::c_long,
    };
    loop {
        let mut status: libc::c_int = 0;
        // SAFETY: `status` is this frame's own, and `pid` is the child this
        // process started and nothing else waits for.
        let waited = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
        if waited == pid {
            return if libc::WIFEXITED(status) {
                Ended::Exited(libc::WEXITSTATUS(status))
            } else {
                Ended::Signalled(libc::WTERMSIG(status))
            };
        }
        // SAFETY: waiting on a set and a timeout that live across the call.
        let taken =
            unsafe { libc::sigtimedwait(&set, std::ptr::null_mut(), &look) };
        if taken == libc::SIGINT || taken == libc::SIGTERM {
            *stopping = true;
            // SAFETY: a signal sent to the child this process started.
            unsafe { libc::kill(pid, taken) };
        }
    }
}

/// Waits for `delay`, unless a stop signal comes first.
///
/// # Arguments
///
/// * `delay` - How long.
///
/// # Returns
///
/// Whether a stop signal came.
fn pause(delay: Duration) -> bool {
    let set = signals(false);
    let wait = libc::timespec {
        tv_sec: delay.as_secs() as libc::time_t,
        tv_nsec: delay.subsec_nanos() as libc::c_long,
    };
    // SAFETY: waiting on a set and a timeout that live across the call.
    let taken =
        unsafe { libc::sigtimedwait(&set, std::ptr::null_mut(), &wait) };
    taken == libc::SIGINT || taken == libc::SIGTERM
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
    let all = signals(true);
    // SAFETY: blocking signals in this, the only thread, with a set that
    // lives across the call.
    unsafe { libc::sigprocmask(libc::SIG_BLOCK, &all, std::ptr::null_mut()) };

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
    let spawning = match Spawning::new() {
        Ok(spawning) => spawning,
        Err(why) => {
            error!("supervisor: no child can be started: {why}");
            eprintln!("tcspecial: no child can be started: {why}");
            return BAD;
        }
    };

    let mut delay = FIRST_DELAY;
    loop {
        if let Some(state_file) = &boot.state_file {
            if let Some(Some(why)) = changed(state_file, |state| {
                before_child(state, boot.load_trials)
            }) {
                warn!("supervisor: {why}");
            }
        }

        let started = Instant::now();
        let pid = match spawning.start() {
            Ok(pid) => pid,
            Err(e) => {
                error!("supervisor: no child can be started: {e}");
                if !mode.is_flight() || pause(delay) {
                    return BAD;
                }
                delay = (delay * 2).min(MOST_DELAY);
                continue;
            }
        };
        info!("supervisor: child {pid} started");
        let mut stopping = false;
        let ended = wait_for(pid, &mut stopping);
        if stopping {
            info!("supervisor: stopped, child {pid} {ended:?}");
            return if ended == Ended::Exited(GOOD) {
                GOOD
            } else {
                BAD
            };
        }
        match ended {
            Ended::Exited(GOOD) => return GOOD,
            Ended::Exited(RESTART) => {
                info!("supervisor: child {pid} took a RESTART");
                delay = FIRST_DELAY;
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
                        if pause(delay) {
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

    /// The four statuses are four numbers.
    #[test]
    fn the_statuses_are_distinct() {
        let all = [GOOD, BAD, FATAL, RESTART];
        for (at, one) in all.iter().enumerate() {
            assert!(!all[at + 1..].contains(one), "{one} twice");
        }
    }
}
