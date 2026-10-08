//! A pty standing in for a device node.
//!
//! Two kinds of endpoint are simulated this way. A serial line, where it is a
//! faithful stand-in: a pty slave is a device file with a line discipline
//! behind it, and a handler opens it, sets it and reads it exactly as it would
//! a port. And a SPI peripheral, where it is not: a pty carries the bytes and
//! has no clock, so what the handler sets a real peripheral to has nowhere to
//! go. See [`crate::payload_serial`] and [`crate::payload_spi`] for what each
//! makes of that.
//!
//! What they share is here: opening the pty, taking the line discipline out of
//! the way, and making the path a handler was told to open lead to the slave
//! for as long as the payload runs.

use std::fs::File;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::path::PathBuf;

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
    /// Open a pty and make `path` lead to its slave.
    pub(crate) fn open(what: &str, path: &str) -> Result<Self, String> {
        let named = PathBuf::from(path);
        let (master, slave) = open_pty()?;

        // Raw, before anything is written. A pty slave starts in canonical
        // mode with echo on, which would send every byte this end writes
        // straight back to it -- counted as received, and counted twice over
        // when the handler answers. A handler sets the slave raw when it opens
        // it, but it may not have opened it yet.
        make_raw(master.as_raw_fd())?;
        set_nonblocking(master.as_raw_fd())?;

        make_the_path_lead_to(&named, &slave)?;

        eprintln!("{} stands in at {} -> {}", what, named.display(), slave);

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
            let _ = std::fs::remove_file(&self.named);
        }
    }
}

/// Open a pty, returning the master and the slave's name.
fn open_pty() -> Result<(File, String), String> {
    // SAFETY: each call is given what it documents and its result is checked
    // before anything is done with it.
    let fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
    if fd < 0 {
        return Err(format!(
            "no pseudo-terminal could be opened: {}",
            std::io::Error::last_os_error()
        ));
    }

    // From here the descriptor is owned, so that an early return closes it.
    let master = unsafe { File::from_raw_fd(fd) };

    if unsafe { libc::grantpt(fd) } != 0 {
        return Err(format!(
            "the pseudo-terminal's slave could not be granted: {}",
            std::io::Error::last_os_error()
        ));
    }
    if unsafe { libc::unlockpt(fd) } != 0 {
        return Err(format!(
            "the pseudo-terminal's slave could not be unlocked: {}",
            std::io::Error::last_os_error()
        ));
    }

    let mut name = [0 as libc::c_char; 128];
    if unsafe { libc::ptsname_r(fd, name.as_mut_ptr(), name.len()) } != 0 {
        return Err(format!(
            "the pseudo-terminal's slave has no name to be found: {}",
            std::io::Error::last_os_error()
        ));
    }
    let slave = unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) }
        .to_string_lossy()
        .into_owned();

    Ok((master, slave))
}

/// Take the line discipline out of the way: no echo, no editing, no newline
/// translation. A payload link carries bytes, not lines of text.
pub(crate) fn make_raw(fd: RawFd) -> Result<(), String> {
    let mut terms: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut terms) } != 0 {
        return Err(format!(
            "the pseudo-terminal's terms cannot be read: {}",
            std::io::Error::last_os_error()
        ));
    }
    unsafe { libc::cfmakeraw(&mut terms) };
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &terms) } != 0 {
        return Err(format!(
            "the pseudo-terminal cannot be set raw: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

/// Reads must not wait: one thread does the receiving and the sending both.
fn set_nonblocking(fd: RawFd) -> Result<(), String> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(format!(
            "the pseudo-terminal cannot be made non-blocking: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

