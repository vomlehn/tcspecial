//! Threads, in a page: each a Web Worker sharing the page's memory, and
//! each belonging to the program that started it.
//!
//! The standard library's own `spawn` does nothing in a page; `wasm_thread`
//! starts a worker in its place. This wraps it so that a thread knows which
//! program it is part of -- see [`crate::browser::context`] -- and that it
//! may wait. Everything else is the standard library's, which works in a
//! worker: parking, the current thread; and sleeping, but for on the page's
//! own thread, which the browser does not let wait: see [`sleep`].

pub use std::thread::{
    current, panicking, park, park_timeout, yield_now, AccessError, LocalKey,
    Result, Thread, ThreadId,
};
pub use wasm_thread::JoinHandle;

use crate::browser::context;

/// Sleeps for a time: on a worker as the standard library sleeps; on the
/// page's own thread, whose waiting the browser refuses and ends the page
/// for, by looking at the page's clock until the time is up. The page draws
/// nothing meanwhile, so what calls this there -- a window's button that
/// waits for a program to end -- keeps the time short.
///
/// # Arguments
///
/// * `time` - How long.
pub fn sleep(time: std::time::Duration) {
    if context::may_wait() {
        std::thread::sleep(time);
        return;
    }
    let until = crate::browser::time::Instant::now() + time;
    while crate::browser::time::Instant::now() < until {
        std::hint::spin_loop();
    }
}

/// How a thread is to be made: its name and its stack.
#[derive(Debug)]
pub struct Builder {
    /// What it is made with.
    inner: wasm_thread::Builder,
}

impl Builder {
    /// A thread with the defaults.
    ///
    /// # Returns
    ///
    /// The builder.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Builder {
        Builder {
            inner: wasm_thread::Builder::new(),
        }
    }

    /// Names the thread.
    ///
    /// # Arguments
    ///
    /// * `name` - Its name.
    ///
    /// # Returns
    ///
    /// The builder.
    pub fn name(self, name: String) -> Builder {
        Builder {
            inner: self.inner.name(name),
        }
    }

    /// Sizes the thread's stack.
    ///
    /// # Arguments
    ///
    /// * `size` - Its size in bytes.
    ///
    /// # Returns
    ///
    /// The builder.
    pub fn stack_size(self, size: usize) -> Builder {
        Builder {
            inner: self.inner.stack_size(size),
        }
    }

    /// Starts the thread, as part of the calling thread's program.
    ///
    /// # Arguments
    ///
    /// * `run` - What it runs.
    ///
    /// # Returns
    ///
    /// Its handle.
    ///
    /// # Errors
    ///
    /// Why no worker could be started.
    pub fn spawn<F, T>(self, run: F) -> std::io::Result<JoinHandle<T>>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let program = context::current();
        self.inner.spawn(move || {
            context::enter(program, true);
            run()
        })
    }
}

/// Starts a thread, as part of the calling thread's program.
///
/// # Arguments
///
/// * `run` - What it runs.
///
/// # Returns
///
/// Its handle.
///
/// # Panics
///
/// If no worker can be started, as the standard library's does.
pub fn spawn<F, T>(run: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    Builder::new().spawn(run).expect("a thread can be started")
}

/// Where the calling thread's stack is: a page does not say.
///
/// # Returns
///
/// `None`.
pub fn stack_bounds() -> Option<(usize, usize, usize)> {
    None
}

/// Whether windows must be made on a thread other than the calling one:
/// on a Web Worker, which is where every program runs, since a page draws
/// its windows on its own thread alone.
///
/// # Returns
///
/// `true` on a worker.
pub fn windows_elsewhere() -> bool {
    crate::browser::context::may_wait()
}
