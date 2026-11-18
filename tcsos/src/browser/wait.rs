//! Waiting for any of several things to be ready, in a page: the page's
//! sockets, and wakeups, which are what a page has to wait on. See
//! [`crate::browser::net`], whose sockets each are a [`Source`].

use std::time::Duration;

/// Something that can be waited on: a socket's number in the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Source(u32);

impl Source {
    /// Nothing: a source a wait passes over.
    pub const NONE: Source = Source(0);

    /// The source a socket of the page is.
    ///
    /// # Arguments
    ///
    /// * `id` - The socket's number.
    ///
    /// # Returns
    ///
    /// The source.
    pub(crate) fn of(id: u32) -> Source {
        Source(id)
    }

    /// The socket's number.
    ///
    /// # Returns
    ///
    /// It.
    pub(crate) fn id(self) -> u32 {
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

/// The most sources one wait takes, as on Linux.
const MOST: usize = 64;

/// Waits until any of a set is ready for what is wanted of it, or the time
/// is up.
///
/// # Arguments
///
/// * `set`     - What to wait on: each one's `got` is set.
/// * `timeout` - How long to wait at most; `None` for ever.
///
/// # Returns
///
/// How many are ready: nought where the time was up first.
///
/// # Errors
///
/// A set larger than the most one wait takes is refused.
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
    let mut asked = [(Source::NONE, Ready::default()); MOST];
    for (asked, watch) in asked.iter_mut().zip(set.iter()) {
        *asked = (watch.source, watch.want);
    }
    let mut got = [Ready::default(); MOST];
    let count = crate::browser::net::wait_on(
        &asked[..set.len()],
        &mut got[..set.len()],
        timeout,
    );
    for (watch, got) in set.iter_mut().zip(got.iter()) {
        watch.got = *got;
    }
    Ok(count)
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
#[derive(Debug)]
pub struct Wakeup {
    /// Its number among the page's sources.
    id: u32,
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
    /// None, in a page.
    pub fn new() -> std::io::Result<Wakeup> {
        Ok(Wakeup {
            id: crate::browser::net::new_wakeup(),
        })
    }

    /// What a wait waits on to be woken by this.
    ///
    /// # Returns
    ///
    /// Its source.
    pub fn source(&self) -> Source {
        Source(self.id)
    }

    /// Wakes whatever is waiting on this.
    pub fn poke(&self) {
        crate::browser::net::set_wakeup(self.id, true);
    }

    /// Takes whatever pokes are waiting.
    pub fn drain(&self) {
        crate::browser::net::set_wakeup(self.id, false);
    }
}

impl Drop for Wakeup {
    fn drop(&mut self) {
        crate::browser::net::release(self.id);
    }
}
