//! Waiting for any of several things to be ready -- a socket to read, a
//! device to write, a wakeup to be poked -- for a time or for ever.
//!
//! On Linux, a thing waited on is a descriptor and the wait is `poll`. What
//! the rest of the crates see is a [`Source`], which anything with a
//! descriptor has through [`Waitable`], and [`wait`] on a set of
//! [`Watch`]es. An interrupted wait is returned as
//! [`std::io::ErrorKind::Interrupted`], as `poll` returns `EINTR`, for the
//! caller to wait again.

use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::time::Duration;

/// Something that can be waited on: a descriptor, which whatever it is of
/// still owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Source(RawFd);

impl Source {
    /// Nothing: a source a wait passes over, for something that has
    /// nothing to wait on yet.
    pub const NONE: Source = Source(-1);

    /// The source a descriptor is, for code that holds a descriptor and
    /// nothing that owns it: a device opened by hand.
    ///
    /// # Arguments
    ///
    /// * `fd` - The descriptor, which must stay open while it is waited on.
    ///
    /// # Returns
    ///
    /// The source.
    pub fn from_raw_fd(fd: RawFd) -> Source {
        Source(fd)
    }

    /// The descriptor this source is.
    ///
    /// # Returns
    ///
    /// The descriptor, which whatever it is of still owns.
    pub fn raw_fd(self) -> RawFd {
        self.0
    }
}

/// Anything that can be waited on.
pub trait Waitable {
    /// What a wait waits on for it.
    ///
    /// # Returns
    ///
    /// Its source, valid while it lives.
    fn source(&self) -> Source;
}

impl<T: AsRawFd> Waitable for T {
    fn source(&self) -> Source {
        Source(self.as_raw_fd())
    }
}

/// What a source is ready for, or is wanted ready for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Ready {
    /// It can be read without waiting.
    pub read: bool,
    /// It can be written without waiting.
    pub write: bool,
    /// It has hung up or failed: reported whether asked for or not.
    pub trouble: bool,
}

impl Ready {
    /// Ready to read.
    pub const READ: Ready = Ready {
        read: true,
        write: false,
        trouble: false,
    };
    /// Ready to write.
    pub const WRITE: Ready = Ready {
        read: false,
        write: true,
        trouble: false,
    };
    /// Ready to read or to write.
    pub const READ_WRITE: Ready = Ready {
        read: true,
        write: true,
        trouble: false,
    };

    /// Whether it is ready for anything at all.
    ///
    /// # Returns
    ///
    /// `true` where any of the three is.
    pub fn any(self) -> bool {
        self.read || self.write || self.trouble
    }
}

/// One source in a set waited on, what is wanted of it, and what it was
/// found ready for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Watch {
    /// What is waited on.
    pub source: Source,
    /// What it is wanted ready for.
    pub want: Ready,
    /// What it was ready for when the wait ended: set by [`wait`].
    pub got: Ready,
}

impl Watch {
    /// A watch on a source for what is wanted of it.
    ///
    /// # Arguments
    ///
    /// * `source` - What is waited on.
    /// * `want`   - What it is wanted ready for.
    ///
    /// # Returns
    ///
    /// The watch, ready for nothing yet.
    pub fn new(source: Source, want: Ready) -> Watch {
        Watch {
            source,
            want,
            got: Ready::default(),
        }
    }
}

/// The most sources one wait takes: more than any wait of the TCS programs
/// has, and room on the stack, so that waiting allocates nothing.
const MOST: usize = 64;

/// Waits until any of a set is ready for what is wanted of it, or the time
/// is up.
///
/// Allocates nothing, since it is called from loops that must not.
///
/// # Arguments
///
/// * `set`     - What to wait on: each one's `got` is set to what it was
///               found ready for.
/// * `timeout` - How long to wait at most; `None` for ever.
///
/// # Returns
///
/// How many are ready: nought where the time was up first.
///
/// # Errors
///
/// [`std::io::ErrorKind::Interrupted`] where a signal ended the wait, and
/// what else the system said where it failed. A set larger than the most
/// one wait takes is refused.
pub fn wait(
    set: &mut [Watch],
    timeout: Option<Duration>,
) -> std::io::Result<usize> {
    if set.len() > MOST {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("a wait on {} sources, more than {MOST}", set.len()),
        ));
    }
    let mut fds = [libc::pollfd {
        fd: -1,
        events: 0,
        revents: 0,
    }; MOST];
    for (fd, watch) in fds.iter_mut().zip(set.iter()) {
        let mut events = 0;
        if watch.want.read {
            events |= libc::POLLIN;
        }
        if watch.want.write {
            events |= libc::POLLOUT;
        }
        *fd = libc::pollfd {
            fd: watch.source.0,
            events,
            revents: 0,
        };
    }
    let timeout = match timeout {
        None => -1,
        Some(wait) => i32::try_from(wait.as_millis()).unwrap_or(i32::MAX),
    };
    // SAFETY: poll needs as many pollfds as it is told, writable for the
    // call: the count is the set's length, no more than the array holds.
    // Each descriptor is the caller's, open for the call.
    let ready = unsafe {
        libc::poll(fds.as_mut_ptr(), set.len() as libc::nfds_t, timeout)
    };
    if ready < 0 {
        return Err(std::io::Error::last_os_error());
    }
    for (fd, watch) in fds.iter().zip(set.iter_mut()) {
        watch.got = Ready {
            read: fd.revents & libc::POLLIN != 0,
            write: fd.revents & libc::POLLOUT != 0,
            trouble: fd.revents
                & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL)
                != 0,
        };
    }
    Ok(ready as usize)
}

/// Waits on a set with a timeout in milliseconds, as `poll` takes it:
/// negative is for ever.
///
/// # Arguments
///
/// * `set`        - What to wait on, as for [`wait`].
/// * `timeout_ms` - How long to wait, in milliseconds; negative for ever.
///
/// # Returns
///
/// How many are ready, as for [`wait`].
///
/// # Errors
///
/// As for [`wait`].
pub fn wait_ms(set: &mut [Watch], timeout_ms: i32) -> std::io::Result<usize> {
    let timeout = u64::try_from(timeout_ms).ok().map(Duration::from_millis);
    wait(set, timeout)
}

/// Something a wait can be woken through, from another thread.
///
/// A pipe, both ends non-blocking: a poke is a byte written, and the read
/// end is what goes in a wait's set. Both ends are closed when this is
/// dropped.
#[derive(Debug)]
pub struct Wakeup {
    /// The end that goes in a wait's set.
    read: OwnedFd,
    /// The end a poke writes to.
    write: OwnedFd,
}

impl Wakeup {
    /// Makes a new wakeup.
    ///
    /// # Returns
    ///
    /// The wakeup.
    ///
    /// # Errors
    ///
    /// What the system said where the pipe cannot be made, such as running
    /// out of descriptors.
    pub fn new() -> std::io::Result<Wakeup> {
        use std::os::fd::FromRawFd;

        let mut fds = [0i32; 2];
        // SAFETY: `fds` is two ints, which is what pipe2 writes, and the
        // flags are the ones documented for it.
        let made = unsafe {
            libc::pipe2(fds.as_mut_ptr(), libc::O_NONBLOCK | libc::O_CLOEXEC)
        };
        if made != 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: two descriptors pipe2 has just made, which nothing else
        // owns.
        unsafe {
            Ok(Wakeup {
                read: OwnedFd::from_raw_fd(fds[0]),
                write: OwnedFd::from_raw_fd(fds[1]),
            })
        }
    }

    /// What a wait waits on to be woken by this.
    ///
    /// # Returns
    ///
    /// The read end, as a source.
    pub fn source(&self) -> Source {
        Source(self.read.as_raw_fd())
    }

    /// Wakes whatever is waiting on this.
    ///
    /// One byte, and a failure is ignored: a full pipe already holds a byte
    /// nobody has read, so the wait is already woken.
    pub fn poke(&self) {
        let byte = [0u8; 1];
        // SAFETY: a one-byte write from a buffer that lives across the call,
        // to a descriptor this owns.
        unsafe {
            libc::write(self.write.as_raw_fd(), byte.as_ptr().cast(), 1);
        }
    }

    /// Takes whatever pokes are waiting, so that a wait woken once does not
    /// return at once for ever after. An empty pipe is the ordinary answer.
    pub fn drain(&self) {
        let mut buffer = [0u8; 64];
        loop {
            // SAFETY: a read into a buffer that lives across the call, from
            // a descriptor this owns.
            let got = unsafe {
                libc::read(
                    self.read.as_raw_fd(),
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                )
            };
            if got < buffer.len() as isize {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wakeup poked is ready, and drained is not; a wait on nothing ready
    /// times out.
    #[test]
    fn a_wakeup_wakes_a_wait_until_drained() {
        let wakeup = Wakeup::new().expect("a wakeup");
        let mut set = [Watch::new(wakeup.source(), Ready::READ)];
        assert_eq!(wait_ms(&mut set, 0).unwrap(), 0);
        wakeup.poke();
        assert_eq!(wait_ms(&mut set, 0).unwrap(), 1);
        assert!(set[0].got.read);
        wakeup.drain();
        assert_eq!(wait(&mut set, Some(Duration::ZERO)).unwrap(), 0);
        assert!(!set[0].got.any());
    }
}
