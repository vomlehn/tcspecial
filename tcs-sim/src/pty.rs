//! A pty simulating a device node.
//!
//! Two kinds of endpoint are simulated this way. A serial line, where it is a
//! faithful simulated device: a pty slave is a device file with a line
//! discipline behind it, and a handler opens it, sets it and reads it exactly
//! as it would a port. And a SPI peripheral, where it is not: a pty carries the
//! bytes and has no clock, so what the handler sets a real peripheral to has
//! nowhere to go. See [`crate::node::payload::serial`] and
//! [`crate::node::payload::spi`] for what each makes of that.
//!
//! What they share is here: making the path a handler was told to open lead
//! to the slave for as long as the payload runs. Opening the pty and taking
//! the line discipline out of the way are `tcsos::linux::pty`'s.

use std::path::PathBuf;
use tcsos::fs::File;

use crate::payload::make_the_path_lead_to;

/// A pty whose slave a configured path leads to, for as long as this lives.
pub(crate) struct SimulatedNode {
    /// The master: this end of the line.
    pub(crate) master: File,
    /// The slave's name, as the kernel chose it. Kept for the log line that
    /// says where the configured path leads, which is the one place the name
    /// is of any use to somebody watching.
    #[allow(dead_code)]
    slave: String,
    /// The path a handler was told to open, which is a link to the slave.
    named: PathBuf,
}

impl SimulatedNode {
    /// Opens a pty, sets its master raw and non-blocking, and makes a path
    /// lead to its slave.
    ///
    /// # Arguments
    ///
    /// * `what` - What is simulated, for the log line that says where.
    /// * `path` - The configured path a handler opens.
    ///
    /// # Returns
    ///
    /// The pty, which takes the link away again when dropped.
    ///
    /// # Errors
    ///
    /// A message saying what failed if the pty cannot be opened or set up,
    /// or the path cannot be made to lead to it.
    pub(crate) fn open(what: &str, path: &str) -> Result<Self, String> {
        let named = PathBuf::from(path);
        let (master, slave) = tcsos::linux::pty::open()?;

        // Raw, before anything is written. A pty slave starts in canonical
        // mode with echo on, which would send every byte this end writes
        // straight back to it -- counted as received, and counted twice over
        // when the handler answers. A handler sets the slave raw when it opens
        // it, but it may not have opened it yet.
        tcsos::linux::pty::make_raw(&master)?;
        tcsos::linux::pty::set_nonblocking(&master)?;

        make_the_path_lead_to(&named, &slave)?;

        eprintln!("{} is simulated at {} -> {}", what, named.display(), slave);

        Ok(Self {
            master,
            slave,
            named,
        })
    }

    /// The slave's name, which is what a handler really opens.
    #[cfg(test)]
    pub(crate) fn slave(&self) -> &str {
        &self.slave
    }
}

impl Drop for SimulatedNode {
    /// Take the link away with the line, so the path does not outlive the
    /// pty it leads to. A link left behind points at a slave that is gone,
    /// and a handler opening it would be told the device does not exist --
    /// true, but in a way that reads as a missing port rather than a stopped
    /// simulator.
    fn drop(&mut self) {
        if self.named.is_symlink() {
            let _ = tcsos::fs::remove_file(&self.named);
        }
    }
}
