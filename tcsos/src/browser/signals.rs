//! A stop asked for by the page, in place of the two signals that ask a
//! program to stop on Linux.
//!
//! A web page has no signals. What stands in for SIGINT and SIGTERM is a
//! request the page makes -- its Stop button, or the page closing -- with
//! [`request`], which the thread that takes stops sees as it would a signal:
//! [`take`] returns it once, and [`pending`] says whether one is waiting.
//! There is nothing to block or unblock, so those do nothing.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Whether the page has asked for a stop that nothing has taken yet.
static ASKED: AtomicBool = AtomicBool::new(false);

/// How often [`take`] looks whether a stop has been asked for, while it
/// waits.
const LOOK_EVERY: Duration = Duration::from_millis(10);

/// Asks the programs of the page to stop, as SIGTERM does on Linux.
pub fn request() {
    ASKED.store(true, Ordering::SeqCst);
}

/// Does nothing: there are no signals to block.
pub fn block() {}

/// Takes the page's request to stop, or waits for one.
///
/// # Arguments
///
/// * `wait` - How long to wait for one, at most.
///
/// # Returns
///
/// [`TERMINATE`] where the page asked for a stop; `None` where it did not
/// in time.
pub fn take(wait: Duration) -> Option<i32> {
    let mut waited = Duration::ZERO;
    loop {
        if ASKED.swap(false, Ordering::SeqCst) {
            return Some(TERMINATE);
        }
        if waited >= wait {
            return None;
        }
        let step = LOOK_EVERY.min(wait - waited);
        std::thread::sleep(step);
        waited += step;
    }
}

/// Whether the page has asked for a stop that nothing has taken.
///
/// # Returns
///
/// `true` where one is waiting.
pub fn pending() -> bool {
    ASKED.load(Ordering::SeqCst)
}

/// Does nothing: there is no default action to give back, and nothing
/// blocked.
pub fn default_and_unblock() {}

/// Asks for a stop, as a test of what a stop does sends SIGTERM on Linux.
pub fn terminate_this_thread() {
    request();
}

/// The number a stop from the page is taken as: SIGTERM's on Linux, so
/// that a status worked out from it reads the same.
pub const TERMINATE: i32 = 15;
