//! What an armed RESTART does: the child stops, and tells its supervisor
//! to start another.
//!
//! The RESTART has pushed the configuration it names onto the configuration
//! stack (see [`crate::state::commanded_restart`]). The vehicle then stops the
//! way it always does -- the pass finished, the links closed, the log
//! flushed -- and the child exits with [`crate::runtime::supervisor::RESTART`],
//! on which its supervisor starts a fresh child on the new top. A fresh
//! process, rather than this one run again, so that nothing this
//! configuration did is there for the next: see
//! [`crate::runtime::supervisor`]. The child is handed what the supervisor
//! was handed -- standard input, output and error, and whatever started
//! tcspecial gave it -- and nothing this child opened.

use std::sync::atomic::{AtomicBool, Ordering};

/// Whether an armed RESTART has asked for one.
static ASKED: AtomicBool = AtomicBool::new(false);

/// Asks for a restart once the vehicle has stopped.
///
/// For the command interpreter, having answered an armed RESTART. The vehicle
/// stops as it does when it is told to, and then [`status`] says to exit for
/// a restart.
pub fn ask() {
    ASKED.store(true, Ordering::SeqCst);
}

/// Says whether a restart has been asked for.
///
/// # Returns
///
/// True once [`ask`] has been called.
pub fn asked() -> bool {
    ASKED.load(Ordering::SeqCst)
}

/// What a child that ran and stopped exits with.
///
/// The restart status where a RESTART asked for one, unless a stop was
/// asked for as the restart was under way -- a SIGINT or a SIGTERM since the
/// RESTART was taken -- when it is the good one, and the supervisor stops
/// too: a vehicle somebody asked to stop must stop, and the restart is the
/// one to give way. See [`crate::runtime::signals::before_exec`]. The state
/// file already has the RESTART's target on top, which is what the next
/// start runs, whenever it comes, as a commanded one.
///
/// # Returns
///
/// [`crate::runtime::supervisor::RESTART`] or
/// [`crate::runtime::supervisor::GOOD`].
pub fn status() -> i32 {
    use crate::runtime::supervisor::{GOOD, RESTART};

    if !asked() || crate::runtime::signals::before_exec() {
        return GOOD;
    }
    log::info!("restarting: the supervisor starts a child on the new top");
    RESTART
}
