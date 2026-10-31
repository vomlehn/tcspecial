//! Which configurations a RESTART may name: those the vehicle holds.
//!
//! A RESTART_ARM and a RESTART carry the checksum of the configuration the
//! vehicle is to run after the restart, and one that names a configuration
//! the vehicle does not hold is refused `NotFound`, so that nothing
//! restarts onto nothing. See "The three commands that always go" in
//! `docs/design.rst`.

use tcslibgs::ConfigDigest;

/// The configurations this start of tcspecial could restart onto.
#[derive(Debug, Clone)]
pub struct Held {
    /// The configuration running now, which a RESTART may always name: one
    /// that does is a plain restart.
    running: ConfigDigest,
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
        Held { running }
    }

    /// Says whether the vehicle holds the configuration `target` names, as
    /// a RESTART_ARM asks.
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
        if self.holds(target) {
            Ok(())
        } else {
            Err(format!(
                "configuration {target} is not held: this start runs {}",
                self.running
            ))
        }
    }
}
