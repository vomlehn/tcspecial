//! Locks, in a page: the standard library's, except that the page's own
//! thread, which draws the windows, never waits for one.
//!
//! The browser refuses `Atomics.wait` on the page's thread, and the
//! standard library's `Mutex` and `RwLock` wait with it when another
//! thread holds them, which there ends the whole page. So [`Mutex`] and
//! [`RwLock`] here are the standard ones, taken on the page's thread by
//! trying again until they are free -- which a lock held for a moment, as
//! every lock the windows share is, soon is -- and on a worker as the
//! standard library takes them. What they hand back is the standard
//! library's guard, so a [`Condvar`] works with them as it would.
//!
//! Everything else is the standard library's own.

pub use std::sync::*;

use std::fmt;

use crate::browser::context;

/// A mutual-exclusion lock that the page's thread takes without waiting:
/// see the module's documentation.
pub struct Mutex<T: ?Sized>(
    /// The standard library's lock.
    std::sync::Mutex<T>,
);

impl<T> Mutex<T> {
    /// Makes a lock, free.
    ///
    /// # Arguments
    ///
    /// * `value` - What it guards.
    ///
    /// # Returns
    ///
    /// The lock.
    pub const fn new(value: T) -> Mutex<T> {
        Mutex(std::sync::Mutex::new(value))
    }

    /// Gives up the lock for what it guards.
    ///
    /// # Returns
    ///
    /// What it guarded.
    ///
    /// # Errors
    ///
    /// Where a thread panicked holding it, as the standard library's does.
    pub fn into_inner(self) -> LockResult<T> {
        self.0.into_inner()
    }
}

impl<T: ?Sized> Mutex<T> {
    /// Takes the lock: on a worker by waiting, on the page's thread by
    /// trying until it is free.
    ///
    /// # Returns
    ///
    /// Its guard.
    ///
    /// # Errors
    ///
    /// Where a thread panicked holding it, as the standard library's does.
    pub fn lock(&self) -> LockResult<MutexGuard<'_, T>> {
        if context::may_wait() {
            return self.0.lock();
        }
        loop {
            match self.0.try_lock() {
                Ok(held) => return Ok(held),
                Err(TryLockError::Poisoned(poisoned)) => return Err(poisoned),
                Err(TryLockError::WouldBlock) => std::hint::spin_loop(),
            }
        }
    }

    /// Takes the lock if it is free.
    ///
    /// # Returns
    ///
    /// Its guard.
    ///
    /// # Errors
    ///
    /// As the standard library's.
    pub fn try_lock(&self) -> TryLockResult<MutexGuard<'_, T>> {
        self.0.try_lock()
    }

    /// Whether a thread panicked holding it.
    ///
    /// # Returns
    ///
    /// `true` if one did.
    pub fn is_poisoned(&self) -> bool {
        self.0.is_poisoned()
    }

    /// What it guards, through a lock nobody else can reach.
    ///
    /// # Returns
    ///
    /// What it guards.
    ///
    /// # Errors
    ///
    /// Where a thread panicked holding it.
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        self.0.get_mut()
    }
}

impl<T: Default> Default for Mutex<T> {
    /// Makes a lock guarding the default value.
    fn default() -> Mutex<T> {
        Mutex::new(T::default())
    }
}

impl<T> From<T> for Mutex<T> {
    /// Makes a lock guarding the value.
    fn from(value: T) -> Mutex<T> {
        Mutex::new(value)
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for Mutex<T> {
    /// Shows it as the standard library's would be shown.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A reader-writer lock that the page's thread takes without waiting:
/// see the module's documentation.
pub struct RwLock<T: ?Sized>(
    /// The standard library's lock.
    std::sync::RwLock<T>,
);

impl<T> RwLock<T> {
    /// Makes a lock, free.
    ///
    /// # Arguments
    ///
    /// * `value` - What it guards.
    ///
    /// # Returns
    ///
    /// The lock.
    pub const fn new(value: T) -> RwLock<T> {
        RwLock(std::sync::RwLock::new(value))
    }

    /// Gives up the lock for what it guards.
    ///
    /// # Returns
    ///
    /// What it guarded.
    ///
    /// # Errors
    ///
    /// Where a thread panicked holding it.
    pub fn into_inner(self) -> LockResult<T> {
        self.0.into_inner()
    }
}

impl<T: ?Sized> RwLock<T> {
    /// Takes the lock to read: on a worker by waiting, on the page's
    /// thread by trying until it is free.
    ///
    /// # Returns
    ///
    /// Its guard.
    ///
    /// # Errors
    ///
    /// Where a thread panicked holding it.
    pub fn read(&self) -> LockResult<RwLockReadGuard<'_, T>> {
        if context::may_wait() {
            return self.0.read();
        }
        loop {
            match self.0.try_read() {
                Ok(held) => return Ok(held),
                Err(TryLockError::Poisoned(poisoned)) => return Err(poisoned),
                Err(TryLockError::WouldBlock) => std::hint::spin_loop(),
            }
        }
    }

    /// Takes the lock to write: on a worker by waiting, on the page's
    /// thread by trying until it is free.
    ///
    /// # Returns
    ///
    /// Its guard.
    ///
    /// # Errors
    ///
    /// Where a thread panicked holding it.
    pub fn write(&self) -> LockResult<RwLockWriteGuard<'_, T>> {
        if context::may_wait() {
            return self.0.write();
        }
        loop {
            match self.0.try_write() {
                Ok(held) => return Ok(held),
                Err(TryLockError::Poisoned(poisoned)) => return Err(poisoned),
                Err(TryLockError::WouldBlock) => std::hint::spin_loop(),
            }
        }
    }

    /// Takes the lock to read if it is free.
    ///
    /// # Returns
    ///
    /// Its guard.
    ///
    /// # Errors
    ///
    /// As the standard library's.
    pub fn try_read(&self) -> TryLockResult<RwLockReadGuard<'_, T>> {
        self.0.try_read()
    }

    /// Takes the lock to write if it is free.
    ///
    /// # Returns
    ///
    /// Its guard.
    ///
    /// # Errors
    ///
    /// As the standard library's.
    pub fn try_write(&self) -> TryLockResult<RwLockWriteGuard<'_, T>> {
        self.0.try_write()
    }

    /// What it guards, through a lock nobody else can reach.
    ///
    /// # Returns
    ///
    /// What it guards.
    ///
    /// # Errors
    ///
    /// Where a thread panicked holding it.
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        self.0.get_mut()
    }
}

impl<T: Default> Default for RwLock<T> {
    /// Makes a lock guarding the default value.
    fn default() -> RwLock<T> {
        RwLock::new(T::default())
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RwLock<T> {
    /// Shows it as the standard library's would be shown.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Channels: the standard library's, but for a [`mpsc::Receiver`] that
/// waits with the page's clock -- the standard library's `recv_timeout`
/// reads its own, which a page has not got -- and that the page's thread
/// waits on by trying until something comes.
pub mod mpsc {
    pub use std::sync::mpsc::{
        RecvError, RecvTimeoutError, SendError, Sender, SyncSender,
        TryRecvError, TrySendError,
    };

    use crate::browser::context;
    use crate::browser::time::{Duration, Instant};

    /// How long a wait for a message sleeps between looks.
    const LOOK: Duration = Duration::from_millis(1);

    /// The receiving end of a channel.
    #[derive(Debug)]
    pub struct Receiver<T>(
        /// The standard library's.
        std::sync::mpsc::Receiver<T>,
    );

    /// Makes a channel that holds any number of messages.
    ///
    /// # Returns
    ///
    /// Its two ends.
    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let (sender, receiver) = std::sync::mpsc::channel();
        (sender, Receiver(receiver))
    }

    /// Makes a channel that holds at most `bound` messages.
    ///
    /// # Arguments
    ///
    /// * `bound` - How many.
    ///
    /// # Returns
    ///
    /// Its two ends.
    pub fn sync_channel<T>(bound: usize) -> (SyncSender<T>, Receiver<T>) {
        let (sender, receiver) = std::sync::mpsc::sync_channel(bound);
        (sender, Receiver(receiver))
    }

    impl<T> Receiver<T> {
        /// Takes a message, waiting for one.
        ///
        /// # Returns
        ///
        /// The message.
        ///
        /// # Errors
        ///
        /// Where every sender has gone.
        pub fn recv(&self) -> Result<T, RecvError> {
            if context::may_wait() {
                return self.0.recv();
            }
            loop {
                match self.0.try_recv() {
                    Ok(message) => return Ok(message),
                    Err(TryRecvError::Disconnected) => return Err(RecvError),
                    Err(TryRecvError::Empty) => std::hint::spin_loop(),
                }
            }
        }

        /// Takes a message if one is there.
        ///
        /// # Returns
        ///
        /// The message.
        ///
        /// # Errors
        ///
        /// Where none is there, or every sender has gone.
        pub fn try_recv(&self) -> Result<T, TryRecvError> {
            self.0.try_recv()
        }

        /// Takes a message, waiting for one no longer than a time, looking
        /// for one every millisecond.
        ///
        /// # Arguments
        ///
        /// * `timeout` - How long.
        ///
        /// # Returns
        ///
        /// The message.
        ///
        /// # Errors
        ///
        /// Where none came in time, or every sender has gone.
        pub fn recv_timeout(
            &self,
            timeout: Duration,
        ) -> Result<T, RecvTimeoutError> {
            let until = Instant::now() + timeout;
            loop {
                match self.0.try_recv() {
                    Ok(message) => return Ok(message),
                    Err(TryRecvError::Disconnected) => {
                        return Err(RecvTimeoutError::Disconnected)
                    }
                    Err(TryRecvError::Empty) => {}
                }
                let left = until.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err(RecvTimeoutError::Timeout);
                }
                if context::may_wait() {
                    std::thread::sleep(left.min(LOOK));
                } else {
                    std::hint::spin_loop();
                }
            }
        }

        /// Every message, waiting for each, until every sender has gone.
        ///
        /// # Returns
        ///
        /// The messages.
        pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
            std::iter::from_fn(move || self.recv().ok())
        }

        /// Every message there now.
        ///
        /// # Returns
        ///
        /// The messages.
        pub fn try_iter(&self) -> impl Iterator<Item = T> + '_ {
            std::iter::from_fn(move || self.try_recv().ok())
        }
    }
}
