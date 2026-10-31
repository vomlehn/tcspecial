//! Which configurations a RESTART may name: those the vehicle holds.
//!
//! A RESTART_ARM and a RESTART carry the checksum of the configuration the
//! vehicle is to run after the restart, and one that names a configuration
//! the vehicle does not hold is refused `NotFound`, so that nothing
//! restarts onto nothing. What it holds is the configuration running, the
//! one on its command line, and every one kept in the directory the
//! command line's `configurations` names. See "The three commands that
//! always go" in `docs/design.rst`.

use tcslibgs::ConfigDigest;

use crate::runtime::choose::Boot;

/// The configurations this start of tcspecial could restart onto.
#[derive(Debug, Clone)]
pub struct Held {
    /// The configuration running now, which a RESTART may always name: one
    /// that does is a plain restart.
    running: ConfigDigest,
    /// What the start chose from, which a restart chooses from again; `None`
    /// for one that holds only what it runs, as a test's does.
    boot: Option<Boot>,
}

impl Held {
    /// What a start holds when it holds only what it is running.
    ///
    /// # Arguments
    ///
    /// * `running` - The checksum of the configuration running.
    ///
    /// # Returns
    ///
    /// The configurations held.
    pub fn running_only(running: ConfigDigest) -> Held {
        Held {
            running,
            boot: None,
        }
    }

    /// What a start holds: what it runs, and what it chose that from.
    ///
    /// # Arguments
    ///
    /// * `running` - The checksum of the configuration running.
    /// * `boot`    - What the start chose from.
    ///
    /// # Returns
    ///
    /// The configurations held.
    pub fn of(running: ConfigDigest, boot: Boot) -> Held {
        Held {
            running,
            boot: Some(boot),
        }
    }

    /// Says whether the vehicle holds the configuration `target` names, as
    /// a RESTART_ARM asks: whether a file of it is there, which
    /// [`Held::check`] then reads.
    ///
    /// Allocates, in the command loop, where the store is asked: a path is
    /// made for each name a configuration may be kept under. Once for each
    /// RESTART_ARM, which an operator sends by hand.
    ///
    /// # Arguments
    ///
    /// * `target` - The checksum a RESTART_ARM or a RESTART carried.
    ///
    /// # Returns
    ///
    /// Whether a restart onto it could be made.
    pub fn holds(&self, target: ConfigDigest) -> bool {
        target == self.running
            || self.boot.as_ref().is_some_and(|boot| boot.holds(target))
    }

    /// Checks the configuration `target` names as it would be loaded, as a
    /// RESTART asks just before it is taken: storage can change between the
    /// arm and the restart.
    ///
    /// # Arguments
    ///
    /// * `target` - The checksum the RESTART carried.
    ///
    /// # Returns
    ///
    /// Nothing, where a start could load it.
    ///
    /// # Errors
    ///
    /// Why it could not, in words for the log.
    pub fn check(&self, target: ConfigDigest) -> Result<(), String> {
        match &self.boot {
            Some(boot) => boot.load(target).map(|_| ()),
            None if target == self.running => Ok(()),
            None => Err(format!(
                "configuration {target} is not held: this start runs {}",
                self.running
            )),
        }
    }
}
