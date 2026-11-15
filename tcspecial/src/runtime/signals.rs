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
//! starting another. A stop sent to the supervisor itself, which is where a
//! service manager sends one, is passed on to the child and stops the
//! supervisor whatever the child exits with.

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
/// must stop.
///
/// # Returns
///
/// Whether a stop signal has been taken by this image or is pending now.
pub fn before_exec() -> bool {
    ENDING.store(true, Ordering::SeqCst);
    let thread = THREAD.lock().ok().and_then(|mut held| held.take());
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
    }
    asked
}
