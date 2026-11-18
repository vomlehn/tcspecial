//! What a process is told by the system about the processes it starts.

/// Everything the standard library's `process` has, which on Linux is the
/// interface: what is below is what it does not have.
pub use std::process::*;

/// The signal a process was ended by, where a signal ended it.
///
/// # Arguments
///
/// * `status` - How the process ended.
///
/// # Returns
///
/// The signal's number; `None` where the process exited.
pub fn ended_by(status: &ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;

    status.signal()
}

/// Sends a process SIGTERM, which asks it to stop, as a service manager
/// does. The standard library's `Child::kill` sends SIGKILL, which does not
/// ask.
///
/// # Arguments
///
/// * `pid` - The process.
///
/// # Errors
///
/// What the system said: commonly, that there is no such process.
pub fn terminate(pid: u32) -> std::io::Result<()> {
    signal(pid, libc::SIGTERM)
}

/// Sends a process SIGKILL, for a process known only by its id: one a
/// program started started.
///
/// # Arguments
///
/// * `pid` - The process.
///
/// # Errors
///
/// What the system said.
pub fn kill(pid: u32) -> std::io::Result<()> {
    signal(pid, libc::SIGKILL)
}

/// Sends a process a signal.
///
/// # Arguments
///
/// * `pid`    - The process.
/// * `signal` - The signal.
///
/// # Errors
///
/// What the system said.
fn signal(pid: u32, signal: libc::c_int) -> std::io::Result<()> {
    let pid = libc::pid_t::try_from(pid)
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    // SAFETY: kill takes a process id and a signal number by value.
    if unsafe { libc::kill(pid, signal) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Has a program be started with a limit on what it may lock into memory,
/// soft and hard, for a test of a lock refused.
///
/// # Arguments
///
/// * `command` - How the program is to be started.
/// * `bytes`   - The limit.
pub fn limit_locked_memory(command: &mut std::process::Command, bytes: u64) {
    use std::os::unix::process::CommandExt;

    // SAFETY: setrlimit lowers a limit of the child's, between fork and
    // exec, and allocates nothing.
    unsafe {
        command.pre_exec(move || {
            let limit = libc::rlimit {
                rlim_cur: bytes as libc::rlim_t,
                rlim_max: bytes as libc::rlim_t,
            };
            if libc::setrlimit(libc::RLIMIT_MEMLOCK, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

/// A pipe whose ends every program this process starts inherits, as a
/// process inherits what its supervisor hands it: made without
/// close-on-exec, which is what makes it inherited. For a test that the
/// descriptors a program was handed survive what it does.
///
/// # Returns
///
/// The read end and the write end, and the write end's descriptor number,
/// as the program it is handed to sees it.
///
/// # Errors
///
/// What the system said.
pub fn inherited_pipe() -> std::io::Result<(std::fs::File, std::fs::File, i32)>
{
    use std::os::fd::FromRawFd;

    let mut ends = [0 as libc::c_int; 2];
    // SAFETY: a pipe into an array of two.
    if unsafe { libc::pipe(ends.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: two descriptors pipe has just made, which nothing else owns.
    unsafe {
        Ok((
            std::fs::File::from_raw_fd(ends[0]),
            std::fs::File::from_raw_fd(ends[1]),
            ends[1],
        ))
    }
}

/// Has a function be run as the process exits normally: for a test
/// process's tidying, which the standard library has no hook for.
///
/// # Arguments
///
/// * `run` - What to run: it takes nothing, returns nothing and must not
///           unwind.
pub fn at_exit(run: extern "C" fn()) {
    // SAFETY: atexit is given a function that takes nothing, returns
    // nothing and never unwinds, which is all it asks.
    unsafe {
        libc::atexit(run);
    }
}
