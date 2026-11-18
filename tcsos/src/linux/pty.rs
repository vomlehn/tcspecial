//! Pseudo-terminals: a pair of ends that behave as a serial line, which is
//! what a simulated serial payload is. Linux alone: a browser has no
//! terminal, and simulates a serial payload over a channel instead.

use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd};

/// Opens a pty and unlocks its slave.
///
/// # Returns
///
/// The master and the slave's name.
///
/// # Errors
///
/// A message saying which step failed, and the system's reason.
pub fn open() -> Result<(File, String), String> {
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
    //
    // SAFETY: from_raw_fd needs sole ownership of an open descriptor.
    // posix_openpt has just made `fd`, it is checked above, and nothing else
    // is given it; the calls below only borrow it while `master` lives.
    let master = unsafe { File::from_raw_fd(fd) };

    // SAFETY: grantpt needs an open pty master, which `fd` is, held open by
    // `master`.
    if unsafe { libc::grantpt(fd) } != 0 {
        return Err(format!(
            "the pseudo-terminal's slave could not be granted: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: as for grantpt.
    if unsafe { libc::unlockpt(fd) } != 0 {
        return Err(format!(
            "the pseudo-terminal's slave could not be unlocked: {}",
            std::io::Error::last_os_error()
        ));
    }

    let mut name = [0 as libc::c_char; 128];
    // SAFETY: ptsname_r needs an open pty master and a buffer it can write of
    // the length it is given: `name` is a local array, and its own length is
    // what is passed.
    if unsafe { libc::ptsname_r(fd, name.as_mut_ptr(), name.len()) } != 0 {
        return Err(format!(
            "the pseudo-terminal's slave has no name to be found: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: from_ptr needs a NUL-terminated string that outlives the
    // borrow. ptsname_r returned 0, so it wrote a terminated name into `name`
    // (it fails with ERANGE rather than truncate), and `name` outlives the
    // CStr, which is copied out at once.
    let slave = unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) }
        .to_string_lossy()
        .into_owned();

    Ok((master, slave))
}

/// Takes the line discipline out of the way: no echo, no editing, no
/// newline translation. A payload link carries bytes, not lines of text.
///
/// # Arguments
///
/// * `terminal` - The terminal to set raw.
///
/// # Returns
///
/// Nothing, once the terminal is raw.
///
/// # Errors
///
/// A message if its settings cannot be read or written.
pub fn make_raw(terminal: &impl AsRawFd) -> Result<(), String> {
    let fd = terminal.as_raw_fd();
    // SAFETY: termios is a C struct of integers and arrays of integers, for
    // which all zeroes is a valid value; tcgetattr overwrites it anyway.
    let mut terms: libc::termios = unsafe { std::mem::zeroed() };
    // SAFETY: tcgetattr needs an open descriptor and a termios it can write.
    // Every caller passes the descriptor of a pty end it holds for the whole
    // call, and `terms` is local.
    if unsafe { libc::tcgetattr(fd, &mut terms) } != 0 {
        return Err(format!(
            "the pseudo-terminal's terms cannot be read: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: cfmakeraw needs a termios it can write, which `terms` is,
    // filled in by tcgetattr above.
    unsafe { libc::cfmakeraw(&mut terms) };
    // SAFETY: tcsetattr needs an open descriptor, as for tcgetattr, and a
    // termios it can read, which `terms` is.
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &terms) } != 0 {
        return Err(format!(
            "the pseudo-terminal cannot be set raw: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

/// Makes reads on a descriptor not wait: one thread does the receiving and
/// the sending both.
///
/// # Arguments
///
/// * `file` - The pty's master.
///
/// # Returns
///
/// Nothing, once the descriptor is non-blocking.
///
/// # Errors
///
/// A message if its flags cannot be read or written.
pub fn set_nonblocking(file: &impl AsRawFd) -> Result<(), String> {
    let fd = file.as_raw_fd();
    // SAFETY: F_GETFL and F_SETFL take the descriptor by value and an int,
    // and only read or set its flags. The caller holds the descriptor across
    // the call.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0
        // SAFETY: as for F_GETFL.
        || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) }
            < 0
    {
        return Err(format!(
            "the pseudo-terminal cannot be made non-blocking: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}
