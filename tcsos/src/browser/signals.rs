//! A stop asked of a program, in place of the signals that ask a program
//! to stop on Linux.
//!
//! A page has no signals. What stands in for SIGINT and SIGTERM is a
//! program's stop flag -- see [`crate::browser::context`] -- set by the
//! page's Stop, or by the program that started it, through
//! [`crate::browser::process::Child::kill`]. The thread that takes stops
//! sees it as it would a signal: [`take`] returns it once, and [`pending`]
//! says whether one is waiting. There is nothing to block or unblock, so
//! those do nothing.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::browser::context;

/// How often [`take`] looks whether a stop has been asked for, while it
/// waits.
const LOOK_EVERY: Duration = Duration::from_millis(10);

/// The number a stop is taken as: SIGTERM's on Linux, so that a status
/// worked out from it reads the same.
pub const TERMINATE: i32 = 15;

/// Asks every program in the page to stop, as the page's Stop does.
pub fn request() {
    context::ask_everything_to_stop();
}

/// Does nothing: there are no signals to block.
pub fn block() {}

/// Takes the calling program's stop, or waits for one.
///
/// # Arguments
///
/// * `wait` - How long to wait for one, at most.
///
/// # Returns
///
/// [`TERMINATE`] where one was asked for; `None` where none was in time.
pub fn take(wait: Duration) -> Option<i32> {
    let program = context::current();
    let mut waited = Duration::ZERO;
    loop {
        if program.stop.swap(false, Ordering::SeqCst) {
            return Some(TERMINATE);
        }
        if waited >= wait {
            return None;
        }
        let step = LOOK_EVERY.min(wait - waited);
        crate::browser::thread::sleep(step);
        waited += step;
    }
}

/// Whether the calling program has been asked to stop.
///
/// # Returns
///
/// `true` where it has.
pub fn pending() -> bool {
    context::current().stop.load(Ordering::SeqCst)
}

/// Does nothing: there is no default action to give back, and nothing
/// blocked.
pub fn default_and_unblock() {}

/// Asks the calling program to stop, as a test of what a stop does sends
/// SIGTERM on Linux.
pub fn terminate_this_thread() {
    context::ask_to_stop(context::current().id);
}

/// Runs something: there is nothing to block.
///
/// # Arguments
///
/// * `run` - What to run.
///
/// # Returns
///
/// What it returned.
pub fn with_stops_blocked<T>(run: impl FnOnce() -> T) -> T {
    run()
}

/// Has a stop asked of the calling program set a flag, as a handler would
/// on Linux: a thread of its own looks for the stop.
///
/// # Arguments
///
/// * `flag` - What a stop sets.
pub fn set_flag_on_stop(flag: &'static AtomicBool) {
    let program = context::current();
    crate::browser::thread::spawn(move || loop {
        if program.stop.load(Ordering::SeqCst) {
            flag.store(true, Ordering::Relaxed);
            return;
        }
        crate::browser::thread::sleep(LOOK_EVERY);
    });
}

/// Has a stop asked of the calling program be noted, for a program with a
/// window to quit by from its event loop. A second cannot end it at once,
/// as on Linux, nothing in a page being able to end a program but itself.
///
/// # Arguments
///
/// * `_second` - What a second would end the program with.
///
/// # Returns
///
/// [`TERMINATE`] once a stop has been asked for, nought until then.
///
/// # Errors
///
/// None.
pub fn note_quits(_second: i32) -> Result<Arc<AtomicUsize>, String> {
    let program = context::current();
    let noted = Arc::new(AtomicUsize::new(0));
    let set = noted.clone();
    crate::browser::thread::spawn(move || loop {
        if program.stop.load(Ordering::SeqCst) {
            set.store(TERMINATE as usize, Ordering::SeqCst);
            return;
        }
        crate::browser::thread::sleep(LOOK_EVERY);
    });
    Ok(noted)
}
