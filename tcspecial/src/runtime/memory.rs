//! Locking tcspecial's memory into RAM, and what a shortage of memory does.
//!
//! The child that runs the nodes locks everything it has and everything it
//! will have with `mlockall`, before it loads a configuration, so that every
//! buffer is real once it is made: a shortage is then a failure to load,
//! not a process killed mid-pass when a buffer is first written. The
//! supervisor locks nothing. `--no-memlock` asks for no lock, under any
//! `--simulate`.
//!
//! A lock can fail at two moments. The call itself fails, or not, the same
//! for every configuration: it locks what the process has mapped before any
//! is loaded. In flight that is said in the log and the startup summary,
//! and the configuration runs unlocked, since a vehicle that would not start
//! for want of a setting would be lost for want of it. Under
//! `--simulate=some` or `all` it is fatal, since a desk is set up to lock as
//! a vehicle is, and a lock that fails there may be the first sign of one
//! that would fail aboard.
//!
//! Once the lock has succeeded, what is made later is locked as it is made,
//! and memory that cannot be had -- a buffer, a node's thread -- is the
//! second moment. [`Allocator`] is tcspecial's allocator, which ends the
//! child at once when an allocation fails, saying how many bytes it was,
//! with [`supervisor::SHORT`] in flight -- so that the supervisor gives the
//! configuration up and starts a fresh child on the one beneath, or, for the
//! command line's, which has none, starts it again unlocked -- and the
//! fatal status under `--simulate`. A fresh process, because what a failed
//! load took is not reliably handed back and stays locked.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicI32, Ordering};

use tcsprotocol::simulate::Mode;

use crate::runtime::supervisor;

/// The status the child ends with when memory runs out:
/// [`supervisor::SHORT`] in flight, and [`supervisor::FATAL`] once [`lock`]
/// has been told the mode is `--simulate=some` or `all`.
static SHORT_STATUS: AtomicI32 = AtomicI32::new(supervisor::SHORT);

/// What became of the lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Locked {
    /// Everything the process has, and will have, is locked.
    Locked,
    /// `--no-memlock` asked for no lock.
    NotAsked,
    /// The lock failed, in flight, and the vehicle runs unlocked: why.
    Unlocked(String),
}

/// Locks the process's memory, unless `--no-memlock` asked it not to.
///
/// # Arguments
///
/// * `asked` - False where `--no-memlock` was given.
/// * `mode`  - The simulation mode, which decides what a failure is.
///
/// # Returns
///
/// What became of the lock.
///
/// # Errors
///
/// Why the lock failed, under `--simulate=some` or `all`, where that is
/// fatal: the child is to end with [`supervisor::FATAL`].
pub fn lock(asked: bool, mode: Mode) -> Result<Locked, String> {
    if !mode.is_flight() {
        SHORT_STATUS.store(supervisor::FATAL, Ordering::Relaxed);
    }
    if !asked {
        return Ok(Locked::NotAsked);
    }
    // SAFETY: mlockall takes flags and changes only how the kernel keeps
    // this process's pages.
    if unsafe { libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE) } == 0 {
        return Ok(Locked::Locked);
    }
    let why = refusal(std::io::Error::last_os_error());
    if mode.is_flight() {
        Ok(Locked::Unlocked(why))
    } else {
        Err(why)
    }
}

/// Says why a lock failed, and what to do about it.
///
/// # Arguments
///
/// * `error` - What `mlockall` failed with.
///
/// # Returns
///
/// A sentence naming the limit, how much is mapped and what to change.
fn refusal(error: std::io::Error) -> String {
    let limit = limit();
    let mapped = mapped();
    let sizes = match (limit, mapped) {
        (Some(limit), Some(mapped)) => format!(
            ": the limit on what it may lock is {} and it has {} mapped",
            kib(limit),
            kib(mapped)
        ),
        (Some(limit), None) => {
            format!(": the limit on what it may lock is {}", kib(limit))
        }
        _ => String::new(),
    };
    format!(
        "its memory cannot be locked into RAM ({error}){sizes}. Raise the \
         limit -- LimitMEMLOCK=infinity in its systemd unit, ulimit -l \
         unlimited in the shell that starts it -- or give --no-memlock to \
         run without the lock"
    )
}

/// A size in bytes, as KiB.
fn kib(bytes: u64) -> String {
    if bytes == u64::MAX {
        "unlimited".to_string()
    } else {
        format!("{} KiB", bytes / 1024)
    }
}

/// The limit on what this process may lock, in bytes, `u64::MAX` where
/// there is none.
///
/// # Returns
///
/// The soft limit, or `None` where it cannot be read.
fn limit() -> Option<u64> {
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

/// How much this process has mapped, in bytes, which is what a lock of
/// everything it has is held to the limit by.
///
/// # Returns
///
/// `VmSize` from `/proc/self/status`, or `None` where it cannot be read.
fn mapped() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmSize:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

/// tcspecial's allocator: the system's, ending the process when it fails.
///
/// The standard library aborts when an allocation fails, which a
/// supervisor cannot tell from a crash. This ends the child with the
/// status that says what happened, and says how many bytes were asked for,
/// without allocating to say it.
pub struct Allocator;

// SAFETY: every call is the system allocator's, with what it returns,
// except that a null it returns ends the process rather than being returned.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let at = System.alloc(layout);
        if at.is_null() {
            ran_out(layout.size());
        }
        at
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let at = System.alloc_zeroed(layout);
        if at.is_null() {
            ran_out(layout.size());
        }
        at
    }

    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize,
    ) -> *mut u8 {
        let at = System.realloc(ptr, layout, new_size);
        if at.is_null() {
            ran_out(new_size);
        }
        at
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }
}

/// Ends the process because memory could not be had, saying so on its
/// error output.
///
/// Allocates nothing, since there may be nothing to allocate: the message
/// is written from the stack, and the process ends without running what an
/// orderly exit would, which might allocate.
///
/// # Arguments
///
/// * `bytes` - How many bytes were asked for.
pub fn ran_out(bytes: usize) -> ! {
    let mut said = [0u8; 256];
    let mut at = 0;
    let mut put = |text: &[u8]| {
        let room = said.len() - at;
        let take = text.len().min(room);
        said[at..at + take].copy_from_slice(&text[..take]);
        at += take;
    };
    put(b"tcspecial: memory ran out: ");
    put(digits(bytes).as_slice());
    put(
        b" bytes could not be had, so this configuration cannot run here \
          as it is\n",
    );
    // SAFETY: writes bytes from a buffer on the stack to the error output,
    // and ends the process.
    unsafe {
        libc::write(libc::STDERR_FILENO, said.as_ptr().cast(), at);
        libc::_exit(SHORT_STATUS.load(Ordering::Relaxed));
    }
}

/// A number in decimal, without allocating.
///
/// # Arguments
///
/// * `number` - The number.
///
/// # Returns
///
/// Its digits.
fn digits(mut number: usize) -> Digits {
    let mut out = Digits {
        bytes: [0; 20],
        len: 0,
    };
    let mut backwards = [0u8; 20];
    let mut n = 0;
    loop {
        backwards[n] = b'0' + (number % 10) as u8;
        n += 1;
        number /= 10;
        if number == 0 {
            break;
        }
    }
    for at in 0..n {
        out.bytes[at] = backwards[n - 1 - at];
    }
    out.len = n;
    out
}

/// A number's decimal digits, held on the stack.
struct Digits {
    /// The digits, most significant first.
    bytes: [u8; 20],
    /// How many of `bytes` are digits.
    len: usize,
}

impl Digits {
    /// The digits.
    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A number is written in decimal, from nought to the largest.
    #[test]
    fn a_number_is_written_in_decimal() {
        for number in [0, 7, 10, 4096, 123_456_789, usize::MAX] {
            assert_eq!(
                digits(number).as_slice(),
                number.to_string().as_bytes()
            );
        }
    }

    /// No lock asked for is none taken, in any mode.
    #[test]
    fn no_lock_asked_for_is_none_taken() {
        for mode in [Mode::None, Mode::Some, Mode::All] {
            assert_eq!(lock(false, mode), Ok(Locked::NotAsked), "{mode}");
        }
    }

    /// A lock refused says what limit it was held to and what to change.
    #[test]
    fn a_refused_lock_says_what_to_change() {
        let said = refusal(std::io::Error::from_raw_os_error(libc::ENOMEM));
        assert!(said.contains("LimitMEMLOCK"), "{said}");
        assert!(said.contains("ulimit -l"), "{said}");
        assert!(said.contains("--no-memlock"), "{said}");
        assert!(said.contains("the limit on what it may lock"), "{said}");
    }
}
