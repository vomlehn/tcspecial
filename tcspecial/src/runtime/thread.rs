//! How every thread tcspecial runs on is made: named, with a small stack,
//! and measured.
//!
//! The standard library gives a thread two mebibytes of stack, which a node
//! never comes near: a node's buffers are made at startup, not on its stack.
//! A stack that size is memory that is reserved, and once the process locks
//! its memory, locked, for nothing, once for every node. So each node's
//! thread is given the configuration's `thread_stack` instead, 64 KiB where
//! it says nothing, and tcspecial's own threads -- its signals, commands,
//! beacons and segsend -- a fixed [`SERVICE_STACK`], more than a node's
//! because the commands thread reads configurations. Every thread the
//! flight software runs on is made here, so that none is made with the
//! standard library's default by being made somewhere else.
//!
//! Every thread is measured: as it starts, it fills the part of its stack
//! it has not used with a pattern, and how deep it has been since is how
//! much of the pattern is gone, which QUERY_THREADS tells the ground (see
//! [`stacks`]). The figure is a little more than the thread used, never
//! less: the few hundred bytes nearest where the filling started are left
//! as they were, and a thread that writes the pattern itself, which only a
//! value of exactly those eight bytes does, is counted as not having been
//! there. Filling costs a thread its whole stack in memory, which locking
//! the process's memory costs it anyway.
//!
//! Every thread allocates from one malloc arena, the main thread's: see
//! [`one_arena`].
//!
//! The signal thread is made before any configuration is read; every node's
//! thread is made after [`set_stack`]. The main thread's stack is the
//! kernel's, sized by `ulimit -s` or a systemd unit's `LimitSTACK` when the
//! process starts, and is measured from [`watch_main`] on.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use tcsos::thread::{Builder, JoinHandle};
use tcsprotocol::{NodeId, ThreadRole, ThreadStack, NO_NODE};

/// The stack each of tcspecial's own threads is given, in bytes: two and a
/// half times the deepest the tests take the commands thread to in a debug
/// build, about 96 KiB, which is the deepest of them.
pub const SERVICE_STACK: usize = 256 * 1024;

/// How much of the main thread's stack below where [`watch_main`] is called
/// is filled, in bytes: as much as a service thread is given. The kernel
/// grows that stack as it is used, up to `ulimit -s`, so filling all it may
/// grow to would make all of it real.
const MAIN_FILLED: usize = SERVICE_STACK;

/// What a stack is filled with: eight bytes no pointer or count a thread
/// keeps is likely to be, "tcsstack" in ASCII.
const PATTERN: u64 = u64::from_be_bytes(*b"tcsstack");

/// How much of a stack nearest the frame that fills it is left unfilled,
/// in bytes: more than the 128 bytes below the stack pointer a function
/// that calls nothing may keep its variables in, which filling must not
/// overwrite.
const UNFILLED: usize = 1024;

/// The stack each node's thread made from here on is given, in bytes.
///
/// Set once a configuration is chosen, from its `thread_stack`, which
/// `tcsprotocol` holds to at least `THREAD_STACK_LEAST`.
static STACK_BYTES: tcsos::local::Local<AtomicUsize> = tcsos::local::Local::new(
    AtomicUsize::new(tcsprotocol::THREAD_STACK_DEFAULT as usize),
    || AtomicUsize::new(tcsprotocol::THREAD_STACK_DEFAULT as usize),
);

/// A measured thread.
struct Watched {
    /// Which thread, so that it can take itself out when it ends.
    key: u64,
    /// What the thread is for, and its node, as the ground is told.
    role: ThreadRole,
    /// The node it belongs to, or [`NO_NODE`].
    node: NodeId,
    /// The stack it was given, in bytes, as the ground is told.
    given: usize,
    /// The lowest address of what was filled.
    low: usize,
    /// The highest address of its stack, one past the last byte.
    top: usize,
}

/// Every thread being measured, in the order they started.
///
/// A thread takes itself out as it ends, under this lock, so that a stack
/// is never read once the thread whose stack it is has gone and it is
/// unmapped: a reader holds the lock for the whole of a read.
static WATCHED: tcsos::local::Local<Mutex<Vec<Watched>>> =
    tcsos::local::Local::new(Mutex::new(Vec::new()), || Mutex::new(Vec::new()));

/// The key the next measured thread is given.
static NEXT_KEY: AtomicU64 = AtomicU64::new(0);

/// Sets the stack each node's thread made from here on is given.
///
/// # Arguments
///
/// * `bytes` - The configuration's `thread_stack`.
pub fn set_stack(bytes: u32) {
    STACK_BYTES.store(bytes as usize, Ordering::Relaxed);
}

/// The stack each node's thread made now is given, in bytes.
///
/// # Returns
///
/// What [`set_stack`] last set, or the default.
pub fn stack() -> usize {
    STACK_BYTES.load(Ordering::Relaxed)
}

/// Makes room to measure this many more threads.
///
/// Called as a configuration is loaded, so that a thread that starts later,
/// for a node the ground starts, takes its place without allocating.
///
/// # Arguments
///
/// * `threads` - How many more threads may start.
pub fn expect(threads: usize) {
    if let Ok(mut watched) = WATCHED.lock() {
        watched.reserve(threads);
    }
}

/// Starts a thread, named `name`, with the stack its role is given, and
/// measured from its start.
///
/// # Arguments
///
/// * `role` - What the thread is for: a node's and a ground link's are
///            given [`stack`] bytes, every other [`SERVICE_STACK`].
/// * `node` - The node it belongs to, or [`NO_NODE`].
/// * `name` - What the thread is called in a log, a debugger and `ps -T`.
/// * `f`    - What the thread runs.
///
/// # Returns
///
/// The thread.
///
/// # Errors
///
/// Why the thread could not be started.
pub fn spawn<F, T>(
    role: ThreadRole,
    node: NodeId,
    name: impl Into<String>,
    f: F,
) -> std::io::Result<JoinHandle<T>>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let given = if role.has_node() {
        stack()
    } else {
        SERVICE_STACK
    };
    Builder::new()
        .name(name.into())
        .stack_size(given)
        .spawn(move || {
            let _watched = Watch::start(role, node, given);
            run(f)
        })
}

/// Runs what a thread was started for, in a frame of its own.
///
/// Never inlined, so that `f`'s variables are in frames below where
/// [`Watch::start`] filled from, and are measured as they are used, rather
/// than in the frame of the closure that called it, above the filling,
/// where all of them would be counted from the start.
///
/// # Arguments
///
/// * `f` - What the thread runs.
///
/// # Returns
///
/// What `f` returns.
#[inline(never)]
fn run<F: FnOnce() -> T, T>(f: F) -> T {
    f()
}

/// Measures the main thread from here on.
///
/// Called first thing in `main`, so that as little of its stack as can be is
/// in use already: what is in use is counted, but not how deep it went
/// before.
pub fn watch_main() {
    std::mem::forget(Watch::start(ThreadRole::Main, NO_NODE, 0));
}

/// A thread being measured, which takes itself out of [`WATCHED`] when it
/// is dropped, as its thread ends.
struct Watch {
    /// Its key in [`WATCHED`], or `None` where it could not be measured.
    key: Option<u64>,
}

impl Watch {
    /// Fills the calling thread's stack and puts it in [`WATCHED`].
    ///
    /// # Arguments
    ///
    /// * `role`  - What the thread is for.
    /// * `node`  - The node it belongs to, or [`NO_NODE`].
    /// * `given` - The stack it was given, in bytes, or 0 for the main
    ///             thread, whose is `ulimit -s`.
    ///
    /// # Returns
    ///
    /// The watch, which measures nothing where the thread's stack could not
    /// be found, so that a thread is never kept from starting by this.
    fn start(role: ThreadRole, node: NodeId, given: usize) -> Watch {
        let Some((bottom, top, limit)) = tcsos::thread::stack_bounds() else {
            return Watch { key: None };
        };
        let low = if role == ThreadRole::Main {
            bottom.max(here().saturating_sub(MAIN_FILLED))
        } else {
            bottom
        };
        fill(low);
        let key = NEXT_KEY.fetch_add(1, Ordering::Relaxed);
        let Ok(mut watched) = WATCHED.lock() else {
            return Watch { key: None };
        };
        watched.push(Watched {
            key,
            role,
            node,
            given: if given == 0 { limit } else { given },
            low,
            top,
        });
        Watch { key: Some(key) }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        if let (Some(key), Ok(mut watched)) = (self.key, WATCHED.lock()) {
            watched.retain(|thread| thread.key != key);
        }
    }
}

/// An address in the calling function's frame.
///
/// # Returns
///
/// It, which is a little below the caller's own frame.
#[inline(never)]
fn here() -> usize {
    let mark = 0u8;
    std::hint::black_box(&mark) as *const u8 as usize
}

/// Fills the calling thread's stack with [`PATTERN`], from `low` up to a
/// little below this function's frame.
///
/// Downward, from the frame to `low`, so that the main thread's stack,
/// which the kernel grows as it is touched, grows a page at a time.
///
/// # Arguments
///
/// * `low` - The lowest address to fill: the bottom of a thread's stack.
#[inline(never)]
fn fill(low: usize) {
    let end = here().saturating_sub(UNFILLED) & !7;
    let mut at = end;
    let low = (low + 7) & !7;
    while at > low {
        at -= 8;
        // SAFETY: the calling thread's own stack, below every frame it has
        // and above its guard, which nothing holds a reference into. A
        // volatile write, because to the compiler it is memory no program
        // reads.
        unsafe { (at as *mut u64).write_volatile(PATTERN) };
    }
}

/// How deep a thread has been into its stack.
///
/// # Arguments
///
/// * `thread` - The thread, which is running: [`WATCHED`] is held.
///
/// # Returns
///
/// From the top of its stack to the lowest byte of the filling it has
/// overwritten, in bytes.
fn deepest(thread: &Watched) -> usize {
    let mut at = (thread.low + 7) & !7;
    // SAFETY: another thread's stack, which is mapped for as long as the
    // thread runs, and it runs until it has taken itself out of WATCHED,
    // which the caller holds. The thread may be writing where this reads;
    // a volatile read of eight aligned bytes sees either what was there or
    // what was written, and either is a fair answer to how deep it has
    // been.
    while at < thread.top
        && unsafe { (at as *const u64).read_volatile() } == PATTERN
    {
        at += 8;
    }
    thread.top - at
}

/// Tells of the measured threads, from the `first`.
///
/// Allocates nothing, so that it may be asked from the loop: what it tells
/// goes into `into`.
///
/// # Arguments
///
/// * `first` - The position, in the order they started, of the first
///             thread to tell of.
/// * `into`  - Where each is told of, from its start.
///
/// # Returns
///
/// How many threads are measured in all, and how many of `into` were
/// filled.
pub fn stacks(first: usize, into: &mut [ThreadStack]) -> (usize, usize) {
    let Ok(watched) = WATCHED.lock() else {
        return (0, 0);
    };
    let told = watched.iter().skip(first).zip(into.iter_mut());
    let mut filled = 0;
    for (thread, slot) in told {
        *slot = ThreadStack {
            role: thread.role,
            node: thread.node,
            stack: u32::try_from(thread.given).unwrap_or(u32::MAX),
            deepest: u32::try_from(deepest(thread)).unwrap_or(u32::MAX),
        };
        filled += 1;
    }
    (watched.len(), filled)
}

/// Has every thread allocate from the one malloc arena the main thread
/// does.
///
/// glibc gives each thread that allocates its own arena, reserving 64 MiB
/// of address space for it, up to eight times as many arenas as there are
/// processors. The space is mapped but never touched, so it costs no
/// memory, but locking the process's memory is refused when everything
/// mapped is more than `RLIMIT_MEMLOCK`, and three threads' arenas are 192
/// MiB of it, and each node another 64. tcspecial allocates while it
/// starts a configuration and not in its loop, so the threads have little
/// to contend for in one arena. Called before any thread is made, since a
/// thread is given its arena the first time it allocates.
///
/// # Panics
///
/// If glibc refuses the setting, which it does only for a parameter it
/// does not know.
pub fn one_arena() {
    assert!(tcsos::memory::one_arena(), "glibc takes M_ARENA_MAX");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Finds this test's own threads among those measured.
    fn of_role(node: NodeId) -> Vec<ThreadStack> {
        let mut into = [ThreadStack::default(); 64];
        let (_, filled) = stacks(0, &mut into);
        into[..filled]
            .iter()
            .filter(|thread| thread.node == node)
            .copied()
            .collect()
    }

    /// A thread's depth is told while it runs, goes up when it goes deeper,
    /// is never less than it went, and the thread is gone from what is told
    /// once it has ended.
    #[test]
    fn a_thread_is_measured_while_it_runs() {
        use tcsos::sync::mpsc::channel;
        let node = NodeId(0x7e57);
        let (deeper, go_deeper) = channel::<()>();
        let (went, gone_deeper) = channel::<usize>();
        // Twenty kibibytes of stack, written so they are used, in a frame
        // of their own, which the thread makes only when told.
        #[inline(never)]
        fn deep() -> usize {
            let mut deep = [0u8; 20 * 1024];
            std::hint::black_box(&mut deep).fill(1);
            deep.len()
        }
        let thread = spawn(ThreadRole::Node, node, "measured", move || {
            go_deeper.recv().expect("told to go deeper");
            went.send(deep()).expect("said");
            go_deeper.recv().expect("told to end");
        })
        .expect("the thread starts");

        // Waits for the thread to be measured, which it is as it starts.
        let mut before = Vec::new();
        for _ in 0..1000 {
            before = of_role(node);
            if !before.is_empty() {
                break;
            }
            tcsos::thread::sleep(tcsos::time::Duration::from_millis(1));
        }
        assert_eq!(before.len(), 1, "{before:?}");
        assert_eq!(before[0].role, ThreadRole::Node);
        assert_eq!(before[0].stack as usize, stack());
        assert!((before[0].deepest as usize) < 16 * 1024, "{before:?}");

        deeper.send(()).expect("sent");
        let used = gone_deeper.recv().expect("it went deeper");
        let after = of_role(node);
        assert!(after[0].deepest as usize >= used, "{after:?}");
        assert!(after[0].deepest > before[0].deepest, "{after:?}");

        deeper.send(()).expect("sent");
        thread.join().expect("it ends");
        assert!(of_role(node).is_empty(), "an ended thread is not told of");
    }

    /// A tcspecial thread has the service stack, whatever a node's is.
    #[test]
    fn a_service_thread_has_the_service_stack() {
        let node = NodeId(0x5e4c);
        let (end, ended) = tcsos::sync::mpsc::channel::<()>();
        // A service thread belongs to no node, but this one is given an id
        // so that the test can find it among the others.
        let thread = spawn(ThreadRole::Beacons, node, "service", move || {
            ended.recv().expect("told to end");
        })
        .expect("the thread starts");
        let mut told = Vec::new();
        for _ in 0..1000 {
            told = of_role(node);
            if !told.is_empty() {
                break;
            }
            tcsos::thread::sleep(tcsos::time::Duration::from_millis(1));
        }
        assert_eq!(told[0].stack as usize, SERVICE_STACK, "{told:?}");
        end.send(()).expect("sent");
        thread.join().expect("it ends");
    }
}
