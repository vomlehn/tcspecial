//! How every thread tcspecial runs on is made: named, and with a small
//! stack.
//!
//! The standard library gives a thread two mebibytes of stack, which a node
//! never comes near: a node's buffers are made at startup, not on its stack.
//! A stack that size is memory that is reserved, and once the process locks
//! its memory, locked, for nothing, once for every node. So each thread is
//! given [`STACK_BYTES`] instead, and every thread the flight software runs on is made here, so that none
//! is made with the standard library's default by being made somewhere else.

use std::thread::Builder;

/// The stack each of tcspecial's threads is given, in bytes.
///
/// Twice what the deepest of them needs in a debug build, which uses more
/// than a release build does: tcspecial's tests overflow the command
/// thread's stack at 64 KiB and pass at 128 KiB. Measure again, the same
/// way, before making it smaller.
pub const STACK_BYTES: usize = 256 * 1024;

/// A thread to be started, named `name` and with a stack of
/// [`STACK_BYTES`].
///
/// # Arguments
///
/// * `name` - What the thread is called in a log, a debugger and `ps -T`.
///
/// # Returns
///
/// The builder, to be spawned.
pub fn named(name: impl Into<String>) -> Builder {
    Builder::new().name(name.into()).stack_size(STACK_BYTES)
}
