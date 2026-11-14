//! How every thread tcspecial runs on is made: named, and with a small
//! stack.
//!
//! The standard library gives a thread two mebibytes of stack, which a node
//! never comes near: a node's buffers are made at startup, not on its stack.
//! A stack that size is memory that is reserved, and once the process locks
//! its memory, locked, for nothing, once for every node. So each thread is
//! given the configuration's `thread_stack` instead, 256 KiB where it says
//! nothing, and every thread the flight software runs on is made here, so
//! that none is made with the standard library's default by being made
//! somewhere else.
//!
//! The signal thread is made before any configuration is read, and has the
//! default; every other thread is made after [`set_stack`]. The main
//! thread's stack is the kernel's, sized by `ulimit -s` or a systemd unit's
//! `LimitSTACK` when the process starts.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::Builder;

/// The stack each thread made from here on is given, in bytes.
///
/// Set once a configuration is chosen, from its `thread_stack`, which
/// `tcsprotocol` holds to at least `THREAD_STACK_LEAST`: the least the
/// tests pass with in a debug build, which uses more than a release build.
static STACK_BYTES: AtomicUsize =
    AtomicUsize::new(tcsprotocol::THREAD_STACK_DEFAULT as usize);

/// Sets the stack each thread made from here on is given.
///
/// # Arguments
///
/// * `bytes` - The configuration's `thread_stack`.
pub fn set_stack(bytes: u32) {
    STACK_BYTES.store(bytes as usize, Ordering::Relaxed);
}

/// The stack each thread made now is given, in bytes.
///
/// # Returns
///
/// What [`set_stack`] last set, or the default.
pub fn stack() -> usize {
    STACK_BYTES.load(Ordering::Relaxed)
}

/// A thread to be started, named `name` and with a stack of [`stack`]
/// bytes.
///
/// # Arguments
///
/// * `name` - What the thread is called in a log, a debugger and `ps -T`.
///
/// # Returns
///
/// The builder, to be spawned.
pub fn named(name: impl Into<String>) -> Builder {
    Builder::new().name(name.into()).stack_size(stack())
}
