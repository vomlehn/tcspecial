//! Whether the ground has matched this start's configuration.
//!
//! A start of tcspecial -- the first, a commanded restart, a crash the
//! supervisor restarted, a power cycle -- begins unmatched, and stays so until
//! it takes a CONNECT whose configuration checksum is the one it runs. Until
//! then its beacons carry the restart flag, it sends the ground nothing but
//! beacons and answers, and it acts on no command but CONNECT, RESTART_ARM and
//! RESTART. Taking a RESTART ends the match for good: the process ends, and
//! the start that comes back begins unmatched again. See "Configurations
//! named by their checksum" in `docs/design.rst`.
//!
//! One fact, read in three places -- the beacon sender, the command
//! interpreter and every link to the ground -- so it is held once, here, and
//! each of them holds a clone. Two atomics and no lock: a link to the ground
//! asks it for every datum it is handed, in the main loop, which must neither
//! allocate nor wait.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Whether this start has been matched, shared by everything that asks.
#[derive(Debug, Clone, Default)]
pub struct Matched {
    /// Set by a matching CONNECT, and cleared by a RESTART being taken.
    matched: Arc<AtomicBool>,
    /// Set once a RESTART has been taken, after which nothing matches again
    /// in this process.
    ending: Arc<AtomicBool>,
}

impl Matched {
    /// The state every start begins in: not matched, and not ending.
    ///
    /// # Returns
    ///
    /// The shared state, of which each holder takes a clone.
    pub fn at_a_start() -> Matched {
        Matched::default()
    }

    /// Whether the ground has matched this start: a CONNECT carrying the
    /// running configuration's checksum has been taken, and no RESTART
    /// since.
    pub fn is_matched(&self) -> bool {
        self.matched.load(Ordering::SeqCst)
    }

    /// Whether a RESTART has been taken, so that the process is ending.
    pub fn is_ending(&self) -> bool {
        self.ending.load(Ordering::SeqCst)
    }

    /// A CONNECT carrying the running configuration's checksum has been
    /// taken: matched from now, unless a RESTART has already been taken, in
    /// which case the CONNECT changes nothing.
    ///
    /// # Returns
    ///
    /// `true` where this began the match: it was not matched before, and is
    /// now. A repeated CONNECT, and one after a RESTART, is `false`.
    pub fn connected(&self) -> bool {
        if self.is_ending() {
            return false;
        }
        !self.matched.swap(true, Ordering::SeqCst)
    }

    /// A RESTART has been taken: not matched from this moment, and never
    /// again in this process.
    ///
    /// The flag goes down before the RESTART is answered, so that nothing
    /// the old configuration makes is sent after it.
    pub fn restart_taken(&self) {
        self.ending.store(true, Ordering::SeqCst);
        self.matched.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A start begins unmatched; a matching CONNECT matches it once; a
    /// RESTART ends the match, and no CONNECT after it matches again.
    #[test]
    fn a_start_is_matched_by_a_connect_until_a_restart_is_taken() {
        let matched = Matched::at_a_start();
        let beacons = matched.clone();
        assert!(!beacons.is_matched(), "every start begins unmatched");

        assert!(matched.connected(), "the CONNECT that began the match");
        assert!(beacons.is_matched(), "every clone sees it");
        assert!(!matched.connected(), "a repeat begins nothing");

        matched.restart_taken();
        assert!(!beacons.is_matched(), "unmatched from the RESTART");
        assert!(!matched.connected(), "nothing matches after a RESTART");
        assert!(!beacons.is_matched());
        assert!(beacons.is_ending());
    }
}
