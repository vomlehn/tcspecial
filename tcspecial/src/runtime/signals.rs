//! The two signals that stop the vehicle, SIGINT and SIGTERM, taken by a
//! thread of their own rather than by a handler, in the child that runs the
//! vehicle.
//!
//! A handler runs in whichever thread the kernel picks and can only set a
//! flag. A stop that arrives between a RESTART being taken and the child's
//! exit for it must stop the vehicle rather than be turned into a restart,
//! so it has to be seen however late it comes. See "Restarting" in
//! `docs/tcspecial-design.rst`.
//!
//! So the two are blocked in every thread from the first moment of `main`,
//! before any thread is made -- the supervisor starts the child with them
//! blocked already -- and every thread inherits the block. A signal that is
//! blocked everywhere stays pending in the process until something takes
//! it. One thread, made here, takes them with `sigtimedwait` and turns each
//! into a stop. A child about to exit for a restart first ends that thread
//! ([`before_exec`]) and looks at what is pending: a stop taken or pending
//! then makes the exit a good one, and the supervisor stops rather than
//! starting another. Having looked, it gives the two back their default
//! action and unblocks them, so that one arriving after the look and before
//! the exit ends the child by that signal, which the supervisor takes as a
//! stop too: nothing is left between the look and the exit for a stop to be
//! lost in. A stop sent to the supervisor itself, which is where a service
//! manager sends one, is passed on to the child and stops the supervisor
//! whatever the child exits with.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::Duration;

use log::info;

/// How long the thread waits for a signal before it looks whether it is to
/// end: what [`before_exec`] waits at most for it to go.
const WAIT: Duration = Duration::from_millis(50);

/// Whether a stop signal has been taken, by this image.
static SIGNALLED: AtomicBool = AtomicBool::new(false);

/// Whether the vehicle's loop is there to be stopped by a signal, which
/// [`listening`] says: before it is, a stop ends the process at once, as a
/// signal nobody handles does.
static LISTENING: AtomicBool = AtomicBool::new(false);

/// Whether the thread is to end.
static ENDING: AtomicBool = AtomicBool::new(false);

/// The thread, until [`before_exec`] joins it.
static THREAD: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

/// The two signals, as a set.
///
/// # Returns
///
/// SIGINT and SIGTERM.
fn stop_signals() -> libc::sigset_t {
    // SAFETY: a set made empty and then given two signals, on the stack.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::sigaddset(&mut set, libc::SIGTERM);
        set
    }
}

/// Blocks the two signals in this thread, and so in every thread made after,
/// and starts the thread that takes them.
///
/// The first thing `main` does after noting how it was started, before any
/// thread is made: a thread made before would not inherit the block, and a
/// signal the kernel gave it would be handled the old way, or not at all.
///
/// # Arguments
///
/// * `stop` - What a signal taken once the loop is listening calls: sets the
///            loop's stop flag.
pub fn start(stop: fn()) {
    let set = stop_signals();
    // SAFETY: blocking signals in the calling thread, with a set that lives
    // across the call.
    unsafe {
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut())
    };
    let thread = crate::runtime::thread::spawn(
        tcsprotocol::ThreadRole::Signals,
        tcsprotocol::NO_NODE,
        "signals",
        move || take(stop),
    )
    .expect("the signal thread starts");
    if let Ok(mut held) = THREAD.lock() {
        *held = Some(thread);
    }
}

/// Takes the two signals until told to end.
///
/// One taken before the loop is listening ends the process at once, with
/// the status a shell gives a process a signal ended, as the signal would
/// have done unhandled: nothing is running yet that a stop has to finish.
/// One taken after asks the loop to stop.
///
/// # Arguments
///
/// * `stop` - What asks the loop to stop.
fn take(stop: fn()) {
    let set = stop_signals();
    let wait = libc::timespec {
        tv_sec: 0,
        tv_nsec: WAIT.as_nanos() as libc::c_long,
    };
    while !ENDING.load(Ordering::SeqCst) {
        // SAFETY: waiting on a set and a timeout that live across the call;
        // no information about the signal is asked for.
        let taken =
            unsafe { libc::sigtimedwait(&set, std::ptr::null_mut(), &wait) };
        if taken <= 0 {
            continue;
        }
        SIGNALLED.store(true, Ordering::SeqCst);
        if !LISTENING.load(Ordering::SeqCst) {
            std::process::exit(128 + taken);
        }
        stop();
    }
}

/// Says that the vehicle's loop is there to be stopped: from now a signal
/// asks it to stop, finishing its pass, rather than ending the process.
///
/// # Returns
///
/// Whether the signals are being taken here at all: `false` where
/// [`start`] was never called, as in a test, for the caller to handle them
/// another way.
pub fn listening() -> bool {
    LISTENING.store(true, Ordering::SeqCst);
    THREAD.lock().map(|held| held.is_some()).unwrap_or(false)
}

/// Ends the thread that takes the signals, so that nothing takes one from
/// here to the exit, and says whether a stop has been asked for.
///
/// For a restart, just before the child exits for one: a stop asked for is
/// one the restart is turned into, since a vehicle somebody asked to stop
/// must stop. Where none has been, the two signals are given back their
/// default action and unblocked in this thread, so that one arriving from
/// here to the exit ends the process by that signal, which the supervisor
/// takes as a stop, rather than staying pending and being lost with it.
///
/// # Returns
///
/// Whether a stop signal has been taken by this image or is pending now.
pub fn before_exec() -> bool {
    ENDING.store(true, Ordering::SeqCst);
    let thread = THREAD.lock().ok().and_then(|mut held| held.take());
    let took = thread.is_some();
    if let Some(thread) = thread {
        let _ = thread.join();
    }
    // SAFETY: asking which signals are pending, into a set on the stack.
    let pending = unsafe {
        let mut pending: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut pending);
        libc::sigpending(&mut pending);
        libc::sigismember(&pending, libc::SIGINT) == 1
            || libc::sigismember(&pending, libc::SIGTERM) == 1
    };
    let asked = SIGNALLED.load(Ordering::SeqCst) || pending;
    if asked {
        info!("a stop was asked for as the restart was under way: stopping");
    } else if took {
        // Only where this process took the signals: a test that never did
        // keeps whatever its harness set.
        unblock_default();
    }
    asked
}

/// Gives the two signals their default action, which ends the process, and
/// unblocks them in this thread, the one that is to exit.
///
/// The other threads keep their block, so a stop sent to the process is
/// handed to this one, and ends it however little of the exit is left. One
/// that arrived since [`before_exec`] looked is pending, and is handed over
/// as the block is lifted.
fn unblock_default() {
    let set = stop_signals();
    // SAFETY: setting two signals to their default action, which installs no
    // handler, and unblocking them in the calling thread, with a set that
    // lives across the call.
    unsafe {
        libc::signal(libc::SIGINT, libc::SIG_DFL);
        libc::signal(libc::SIGTERM, libc::SIG_DFL);
        libc::pthread_sigmask(libc::SIG_UNBLOCK, &set, std::ptr::null_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Command;

    /// Set in the environment of the copy of this test that is to be
    /// stopped, as the restart's child would be.
    const CHILD: &str = "TCS_SIGNALS_TEST_CHILD";

    /// What the copy exits with if a stop is lost.
    const LOST: i32 = 3;

    /// Asks a loop to stop; there is none here.
    fn nothing() {}

    /// A stop that arrives after the restart has looked, and before the
    /// child exits, ends the child by that signal rather than being lost
    /// with it.
    ///
    /// The test runs itself again as a child, since what it checks ends a
    /// process. The copy takes the signals as a vehicle does, looks as a
    /// restart does, and then is sent SIGTERM -- to its own thread, so that
    /// none of the test harness's threads, which have no block, takes it --
    /// and exits as for a restart. It must die of the SIGTERM.
    #[test]
    fn a_stop_after_the_look_is_not_lost() {
        if std::env::var_os(CHILD).is_some() {
            start(nothing);
            assert!(!before_exec(), "no stop has been asked for yet");
            // SAFETY: a signal sent to this thread, and an exit that runs
            // nothing more of this process.
            unsafe {
                libc::pthread_kill(libc::pthread_self(), libc::SIGTERM);
                libc::_exit(LOST);
            }
        }
        let name =
            concat!(module_path!(), "::a_stop_after_the_look_is_not_lost");
        let name = name.split_once("::").map(|(_, rest)| rest).unwrap_or(name);
        let status = Command::new(std::env::current_exe().expect("this test"))
            .args(["--exact", name, "--test-threads=1", "--nocapture"])
            .env(CHILD, "1")
            .status()
            .expect("the copy runs");
        assert_eq!(
            status.signal(),
            Some(libc::SIGTERM),
            "the stop was lost: the copy {status}"
        );
    }
}
