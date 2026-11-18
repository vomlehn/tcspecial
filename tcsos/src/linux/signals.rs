//! The two signals that ask a program to stop, SIGINT and SIGTERM, taken by
//! a thread that waits for them rather than by a handler.
//!
//! What a program does with a stop is its own: this is only the four things
//! it needs of the system to take one reliably. They are blocked in every
//! thread, so that one sent stays pending in the process until something
//! takes it; one thread takes them; a program can ask whether one is
//! pending; and the thread that is to exit can give them back their default
//! action, so that one arriving then ends it rather than being lost. See
//! tcspecial's `runtime::signals`, which is what they are for.

use std::time::Duration;

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

/// Blocks the two stop signals in the calling thread, and so in every
/// thread it makes after.
///
/// To be called before any thread is made: one made before does not
/// inherit the block.
pub fn block() {
    let set = stop_signals();
    // SAFETY: blocking signals in the calling thread, with a set that lives
    // across the call.
    unsafe {
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut())
    };
}

/// Takes one stop signal that is pending, or waits for one.
///
/// # Arguments
///
/// * `wait` - How long to wait for one, at most.
///
/// # Returns
///
/// The signal's number, which a shell adds 128 to for the status of a
/// process a signal ended; `None` where none came in time.
pub fn take(wait: Duration) -> Option<i32> {
    let set = stop_signals();
    let wait = libc::timespec {
        tv_sec: wait.as_secs() as libc::time_t,
        tv_nsec: wait.subsec_nanos() as libc::c_long,
    };
    // SAFETY: waiting on a set and a timeout that live across the call; no
    // information about the signal is asked for.
    let taken =
        unsafe { libc::sigtimedwait(&set, std::ptr::null_mut(), &wait) };
    (taken > 0).then_some(taken)
}

/// Whether a stop signal is pending: sent, and not yet taken.
///
/// # Returns
///
/// `true` where SIGINT or SIGTERM is pending.
pub fn pending() -> bool {
    // SAFETY: asking which signals are pending, into a set on the stack.
    unsafe {
        let mut pending: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut pending);
        libc::sigpending(&mut pending);
        libc::sigismember(&pending, libc::SIGINT) == 1
            || libc::sigismember(&pending, libc::SIGTERM) == 1
    }
}

/// Gives the two stop signals their default action, which ends the
/// process, and unblocks them in the calling thread.
///
/// The other threads keep their block, so a stop sent to the process is
/// handed to this one; one pending is handed over as the block is lifted.
pub fn default_and_unblock() {
    let set = stop_signals();
    // SAFETY: setting two signals to their default action, which installs
    // no handler, and unblocking them in the calling thread, with a set that
    // lives across the call.
    unsafe {
        libc::signal(libc::SIGINT, libc::SIG_DFL);
        libc::signal(libc::SIGTERM, libc::SIG_DFL);
        libc::pthread_sigmask(libc::SIG_UNBLOCK, &set, std::ptr::null_mut());
    }
}

/// Sends SIGTERM to the calling thread, as a stop sent to the process is
/// handed to the one thread without the block. For a test of what a stop
/// does.
pub fn terminate_this_thread() {
    // SAFETY: a signal sent to the calling thread.
    unsafe {
        libc::pthread_kill(libc::pthread_self(), libc::SIGTERM);
    }
}

/// SIGTERM's number, for a test that a process was ended by it.
pub const TERMINATE: i32 = libc::SIGTERM;

/// Runs something with the two stop signals blocked in the calling thread,
/// and puts back what was blocked before: for a test of what a program
/// started from such a thread inherits.
///
/// # Arguments
///
/// * `run` - What to run.
///
/// # Returns
///
/// What it returned.
pub fn with_stops_blocked<T>(run: impl FnOnce() -> T) -> T {
    let set = stop_signals();
    // SAFETY: sets this thread's own mask, with sets on this stack, and puts
    // back what was there before.
    let before = unsafe {
        let mut before: libc::sigset_t = std::mem::zeroed();
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, &mut before);
        before
    };
    let out = run();
    // SAFETY: as above.
    unsafe {
        libc::pthread_sigmask(libc::SIG_SETMASK, &before, std::ptr::null_mut())
    };
    out
}

/// The flag a stop signal sets, once [`set_flag_on_stop`] has named one.
static FLAG: std::sync::OnceLock<&'static std::sync::atomic::AtomicBool> =
    std::sync::OnceLock::new();

/// What a stop signal does once [`set_flag_on_stop`] has named a flag:
/// sets it and returns, which is all a handler may safely do.
///
/// # Arguments
///
/// * `_signal` - Which signal it was.
extern "C" fn set_flag(_signal: libc::c_int) {
    if let Some(flag) = FLAG.get() {
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Has either stop signal set a flag, by a handler, where no thread takes
/// them: for a program, or a test, that has not called [`block`].
///
/// # Arguments
///
/// * `flag` - What a stop sets. The first flag named is the one set.
pub fn set_flag_on_stop(flag: &'static std::sync::atomic::AtomicBool) {
    let _ = FLAG.set(flag);
    // SAFETY: the handler does one thing -- an atomic store -- which is
    // async-signal-safe. `signal` is used rather than `sigaction` because
    // that is all this needs.
    unsafe {
        let handler = set_flag as *const () as libc::sighandler_t;
        libc::signal(libc::SIGINT, handler);
        libc::signal(libc::SIGTERM, handler);
    }
}

/// Has the signals that ask a program with a window to quit -- SIGHUP,
/// SIGINT and SIGTERM -- be noted rather than end it, so that its event
/// loop can stop what it runs and clear what it made first; and a second,
/// should that take too long, end it at once.
///
/// # Arguments
///
/// * `second` - What a second ends the program with.
///
/// # Returns
///
/// The number of the signal that came, which is nought until one has.
///
/// # Errors
///
/// Why a signal could not be handled, naming it.
pub fn note_quits(
    second: i32,
) -> Result<std::sync::Arc<std::sync::atomic::AtomicUsize>, String> {
    use signal_hook::flag;
    use std::sync::atomic::{AtomicBool, AtomicUsize};
    use std::sync::Arc;

    let came = Arc::new(AtomicBool::new(false));
    let signalled = Arc::new(AtomicUsize::new(0));
    for signal in [libc::SIGHUP, libc::SIGINT, libc::SIGTERM] {
        // The exit first: it looks at whether a signal had already come,
        // and the first one is to find that none had.
        flag::register_conditional_shutdown(signal, second, came.clone())
            .and_then(|_| flag::register(signal, came.clone()))
            .and_then(|_| {
                flag::register_usize(signal, signalled.clone(), signal as usize)
            })
            .map_err(|e| format!("cannot handle signal {signal}: {e}"))?;
    }
    Ok(signalled)
}
