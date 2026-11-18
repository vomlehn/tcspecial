//! Locking a process's memory into RAM, what it has, and ending it when
//! memory runs out.

use std::io::Read;

/// What a process has of memory, in bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Figures {
    /// What it has mapped.
    pub mapped: u64,
    /// What of that is in RAM.
    pub resident: u64,
    /// What of that is locked into RAM.
    pub locked: u64,
}

/// Locks everything the process has, and everything it will have, into
/// RAM.
///
/// # Errors
///
/// Why the system refused: commonly, the limit on what the process may
/// lock, which [`lock_limit`] gives.
pub fn lock_all() -> std::io::Result<()> {
    // SAFETY: mlockall takes flags and changes only how the kernel keeps
    // this process's pages.
    if unsafe { libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// What to change for a lock refused for want of room, in words for the
/// person who starts the program.
pub const RAISE_THE_LIMIT: &str = "Raise the limit -- LimitMEMLOCK=infinity \
     in its systemd unit, ulimit -l unlimited in the shell that starts it";

/// The limit on what this process may lock, in bytes, `u64::MAX` where
/// there is none.
///
/// # Returns
///
/// The soft limit, or `None` where it cannot be read.
pub fn lock_limit() -> Option<u64> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: getrlimit writes the limit it is asked for into `limit`.
    if unsafe { libc::getrlimit(libc::RLIMIT_MEMLOCK, &mut limit) } != 0 {
        return None;
    }
    Some(if limit.rlim_cur == libc::RLIM_INFINITY {
        u64::MAX
    } else {
        limit.rlim_cur
    })
}

/// What the process has of memory now, without allocating, since it is
/// asked from a loop that must not.
///
/// `/proc/self/status` is read into a buffer on the stack: the standard
/// library's `File::open` makes the path's C string on the stack too, for
/// a path as short as this.
///
/// # Returns
///
/// The figures; a figure that cannot be read is nought.
pub fn figures() -> Figures {
    let mut buffer = [0u8; 4096];
    let mut filled = 0;
    if let Ok(mut file) = std::fs::File::open("/proc/self/status") {
        while filled < buffer.len() {
            match file.read(&mut buffer[filled..]) {
                Ok(0) | Err(_) => break,
                Ok(got) => filled += got,
            }
        }
    }
    let status = &buffer[..filled];
    Figures {
        mapped: field_of(status, b"VmSize:"),
        resident: field_of(status, b"VmRSS:"),
        locked: field_of(status, b"VmLck:"),
    }
}

/// One of `/proc/self/status`'s sizes, in bytes.
///
/// # Arguments
///
/// * `status` - The file's text.
/// * `label`  - The field's label, its colon included: `VmSize:`.
///
/// # Returns
///
/// The size, which the file gives in KiB; nought where there is none.
fn field_of(status: &[u8], label: &[u8]) -> u64 {
    for line in status.split(|&byte| byte == b'\n') {
        if let Some(rest) = line.strip_prefix(label) {
            let kib = rest
                .iter()
                .skip_while(|byte| byte.is_ascii_whitespace())
                .take_while(|byte| byte.is_ascii_digit())
                .fold(0u64, |sum, digit| {
                    sum.saturating_mul(10)
                        .saturating_add(u64::from(digit - b'0'))
                });
            return kib.saturating_mul(1024);
        }
    }
    0
}

/// Writes a message to the error output and ends the process at once,
/// without allocating and without running what an orderly exit would,
/// which might allocate: for when memory has run out.
///
/// # Arguments
///
/// * `said`   - The message.
/// * `status` - What the process exits with.
pub fn say_and_end(said: &[u8], status: i32) -> ! {
    // SAFETY: writes bytes from the caller's buffer to the error output, and
    // ends the process.
    unsafe {
        libc::write(libc::STDERR_FILENO, said.as_ptr().cast(), said.len());
        libc::_exit(status);
    }
}

/// Has the allocator keep one arena for every thread, rather than one for
/// each, which is memory locking would count against its limit. To be
/// called while the process has one thread.
///
/// # Returns
///
/// Whether the allocator took the setting.
pub fn one_arena() -> bool {
    // SAFETY: mallopt takes two integers and changes only malloc's own
    // settings; called while the process has one thread.
    unsafe { libc::mallopt(libc::M_ARENA_MAX, 1) == 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The process's figures are read from its status, the mapped at least
    /// as large as the resident, and a field the status lacks is nought.
    #[test]
    fn the_process_memory_is_read() {
        let memory = figures();
        assert!(memory.mapped > 0, "{memory:?}");
        assert!(memory.resident > 0, "{memory:?}");
        assert!(memory.mapped >= memory.resident, "{memory:?}");
        assert!(lock_limit().is_some_and(|limit| limit > 0));
        assert_eq!(field_of(b"VmSize:\t  1234 kB\n", b"VmSize:"), 1234 * 1024);
        assert_eq!(field_of(b"VmRSS:\t5 kB\n", b"VmSize:"), 0);
    }
}
