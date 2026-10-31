//! Restarting tcspecial in place: what an armed RESTART does.
//!
//! The process runs itself again. Everything it opened is closed as it
//! `exec`s its own executable with the arguments it was started with, keeping
//! the descriptors it was started with and only those: standard input, output
//! and error, and whatever a supervisor handed it -- a socket it listens on, a
//! pipe it reports to. The process id stays the same, so whatever supervises
//! tcspecial sees one process throughout, and the vehicle that comes back
//! chooses its configuration again -- the one the RESTART named, which the
//! state file now says -- which is how another configuration is flown: see
//! [`crate::runtime::choose`].
//!
//! Which descriptors it was started with is asked as the process starts,
//! before anything opens, because by the time a RESTART arrives they are
//! among dozens tcspecial opened itself and nothing tells the two apart. On a
//! system with no `/proc/self/fd` or `/dev/fd` to ask, or where the `exec`
//! fails, the restart says so on stderr and the process exits: a restart that
//! cannot keep what it was handed is not one, and a process that carried on
//! after being told to restart would be flying something the ground thinks
//! has gone.

use std::ffi::{CString, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use log::info;

/// What a restart needs, noted as the process starts.
struct Started {
    /// The executable, as it was when the process started.
    program: PathBuf,
    /// Every argument, the program's own name first.
    args: Vec<OsString>,
    /// The descriptors the process was started with, or why they could not
    /// be found.
    inherited: Result<Vec<libc::c_int>, String>,
}

static STARTED: OnceLock<Started> = OnceLock::new();

/// Whether an armed RESTART has asked for one.
static ASKED: AtomicBool = AtomicBool::new(false);

/// Notes what a restart will need: the program, its arguments and the
/// descriptors it was started with.
///
/// The first thing `main` does, before anything is opened: a descriptor
/// opened before this is one a restart would keep, and would then have twice.
pub fn note_how_started() {
    let program = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("/proc/self/exe"));
    let _ = STARTED.set(Started {
        program,
        args: std::env::args_os().collect(),
        inherited: open_descriptors(),
    });
}

/// Asks for a restart once the vehicle has stopped.
///
/// For the command interpreter, having answered an armed RESTART. The vehicle
/// stops as it does when it is told to -- the pass finished, the links closed,
/// the log flushed -- and then [`if_asked`] runs it again.
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

/// Runs tcspecial again, if a restart was asked for, and otherwise returns.
///
/// Does not return when one was: the process becomes the new tcspecial, or
/// says why it could not and exits. Unless a stop was asked for as the
/// restart was under way -- a SIGINT or a SIGTERM since the RESTART was
/// taken -- when it returns, and the process stops: a vehicle somebody
/// asked to stop must stop, and the restart is the one to give way. See
/// [`crate::runtime::signals::before_exec`]. The state file already names
/// the RESTART's target for the next start, which is what that start then
/// loads, as a commanded one.
pub fn if_asked() {
    if !asked() {
        return;
    }
    if crate::runtime::signals::before_exec() {
        return;
    }
    let why = match STARTED.get() {
        None => {
            "how tcspecial was started was not noted as it started".to_string()
        }
        Some(started) => match &started.inherited {
            Err(why) => why.clone(),
            Ok(inherited) => {
                info!("restarting: {}", started.program.display());
                run_again(started, inherited)
            }
        },
    };
    eprintln!(
        "tcspecial: cannot restart on this system: {why}; exiting instead"
    );
    std::process::exit(1);
}

/// Closes everything this process opened, keeps `inherited`, and becomes
/// `started.program` again.
///
/// What this process opened is marked close-on-exec rather than closed here,
/// so the kernel closes it all at the moment the exec succeeds. Closed one by
/// one beforehand, a descriptor's number could be reused by a thread still
/// running before the exec -- a write meant for one device landing on another
/// -- and an exec that failed would leave owners of descriptors that were
/// gone.
///
/// # Arguments
///
/// * `started`   - The program and arguments noted as the process started.
/// * `inherited` - The descriptors the process was started with, kept open
///                 across the `exec`.
///
/// # Returns
///
/// Only if the process could not be run again: why not (a nul in the
/// program's name or an argument, the open descriptors unreadable or not
/// marked, or the `exec` failing).
fn run_again(started: &Started, inherited: &[libc::c_int]) -> String {
    let program = match CString::new(started.program.as_os_str().as_bytes()) {
        Ok(program) => program,
        Err(_) => {
            return format!(
                "{} has a nul in its name",
                started.program.display()
            )
        }
    };
    let args: Vec<CString> = match started
        .args
        .iter()
        .map(|arg| CString::new(arg.as_bytes()))
        .collect()
    {
        Ok(args) => args,
        Err(_) => return "an argument has a nul in it".to_string(),
    };
    let mut argv: Vec<*const libc::c_char> =
        args.iter().map(|arg| arg.as_ptr()).collect();
    argv.push(std::ptr::null());

    // Everything open now that the process was not started with. Asked
    // again rather than remembered, because what tcspecial opened is spread
    // through every node, and a list kept as they opened would be one more
    // thing each of them had to remember to keep.
    let open = match open_descriptors() {
        Ok(open) => open,
        Err(why) => return why,
    };
    for fd in open {
        if !inherited.contains(&fd) {
            // SAFETY: setting a descriptor's flags, which leaves it open and
            // owned as it was until the exec; one that has gone since is
            // refused by the kernel and is nothing to close.
            unsafe {
                let flags = libc::fcntl(fd, libc::F_GETFD);
                if flags >= 0 {
                    libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC);
                }
            }
        }
    }
    // Kept across the exec, whatever has been done to them since: they were
    // handed to this process to be kept.
    for &fd in inherited {
        // SAFETY: setting a descriptor's flags; one that has gone since is
        // refused by the kernel and is nothing to pass on.
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFD);
            if flags >= 0 {
                libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC);
            }
        }
    }

    // SAFETY: a nul-terminated program and a null-terminated vector of
    // nul-terminated arguments, all of which outlive the call.
    unsafe { libc::execv(program.as_ptr(), argv.as_ptr()) };
    format!(
        "{} could not be run again: {}",
        started.program.display(),
        std::io::Error::last_os_error()
    )
}

/// The descriptors this process has open, in order.
///
/// Read from `/proc/self/fd`, or `/dev/fd` where that is what the system has.
/// Reading the directory opens one descriptor of its own, which is in the
/// listing and closed by the time it is returned, so each one listed is asked
/// about again once the directory is closed.
///
/// # Returns
///
/// The open descriptors, sorted.
///
/// # Errors
///
/// A message saying so if neither `/proc/self/fd` nor `/dev/fd` can be read.
fn open_descriptors() -> Result<Vec<libc::c_int>, String> {
    let listed: Vec<libc::c_int> = ["/proc/self/fd", "/dev/fd"]
        .iter()
        .find_map(|dir| std::fs::read_dir(dir).ok())
        .ok_or_else(|| {
            "neither /proc/self/fd nor /dev/fd can be read, so which \
             descriptors tcspecial was started with cannot be known"
                .to_string()
        })?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
        .collect();
    let mut open: Vec<libc::c_int> = listed
        .into_iter()
        // SAFETY: asking about a descriptor's flags changes nothing.
        .filter(|&fd| unsafe { libc::fcntl(fd, libc::F_GETFD) } >= 0)
        .collect();
    open.sort_unstable();
    Ok(open)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What is open is listed, a file just opened among it.
    #[test]
    fn the_open_descriptors_are_listed() {
        use std::os::fd::AsRawFd;
        let file = tempfile::tempfile().expect("a file");
        let open = open_descriptors().expect("this system can say");
        assert!(open.contains(&file.as_raw_fd()), "{open:?}");
        assert!(open.contains(&2), "standard error: {open:?}");
    }
}
