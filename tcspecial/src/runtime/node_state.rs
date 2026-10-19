//! What each node's state is, and how a command reaches a node that is busy.
//!
//! A node is enabled or disabled, and which it is belongs to the node rather
//! than to the pass. That is deliberate: the command interpreter runs on a
//! thread of its own so that a command is answered while the pass is running,
//! and a state the pass held would be a state the interpreter has to wait for
//! -- which would put the command back behind the pass and lose the whole
//! point of the separate thread. So a state is an atomic the interpreter
//! stores into and the pass loads, and neither waits for the other.
//!
//! # The wakeup descriptor
//!
//! Nothing in a pass waits for data to arrive -- a descriptor is asked whether
//! it is ready and a silent device reads as nought bytes -- but some
//! operations contain a wait of their own: a `process` computation waits for
//! its program's answer up to the deadline the configuration gives it, and a
//! write to a part that is not ready can wait on the part. A disable that had
//! to wait those out would arrive up to a timeout late, which on a part that
//! has stopped answering is exactly when it is wanted.
//!
//! So every node has a descriptor of its own that disabling writes a byte to.
//! A port's wait has that descriptor in its poll set beside the endpoint's, so
//! the wait returns at once and the operation is abandoned where it stands.
//!
//! # Why a pipe
//!
//! An eventfd would do and would cost one descriptor instead of two. A pipe is
//! used because it is what [`crate::endpoint`]'s waits already take: one
//! mechanism, understood in one way, in a program where the thing being woken
//! is a `poll` somebody has to be able to reason about.
//! Both ends are non-blocking, so a poke cannot block when nothing has drained
//! the pipe and a drain cannot block when nothing has poked it.

use std::collections::BTreeMap;
use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tcslibgs::graph::Graph;
use tcslibgs::{DHSample, NodeCounts, NodeId, Timestamp};

/// A descriptor a wait can be woken through.
///
/// Owned: both ends are closed when this is dropped, so a node's descriptors
/// go when the node's state does.
#[derive(Debug)]
pub struct Wakeup {
    /// The end that goes in a poll set.
    read_fd: RawFd,
    /// The end a poke writes to.
    write_fd: RawFd,
}

impl Wakeup {
    /// A new pipe, both ends non-blocking.
    pub fn new() -> std::io::Result<Wakeup> {
        let mut fds = [0i32; 2];
        // SAFETY: `fds` is two ints, which is what pipe2 writes, and the flag
        // is the one documented for it.
        let made = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_NONBLOCK) };
        if made != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Wakeup {
            read_fd: fds[0],
            write_fd: fds[1],
        })
    }

    /// The descriptor to put in a poll set.
    pub fn io_fd(&self) -> RawFd {
        self.read_fd
    }

    /// Wake whatever is waiting on this node.
    ///
    /// One byte, and a failure is ignored on purpose. The two ways it can fail
    /// are a full pipe -- which already holds a byte nobody has read, so the
    /// wait is already woken -- and a descriptor that is gone, which is a node
    /// whose state is being dropped. Neither is something a disable should
    /// report: what the ground asked for is a state, and the state is set by
    /// the store that happens beside this.
    pub fn poke(&self) {
        let byte = [0u8; 1];
        // SAFETY: a one-byte write from a buffer that lives across the call.
        unsafe {
            libc::write(self.write_fd, byte.as_ptr() as *const libc::c_void, 1);
        }
    }

    /// Take whatever pokes are waiting in the pipe.
    ///
    /// So that a wait woken by one disable does not return at once for ever
    /// after. Non-blocking, and an empty pipe is the ordinary answer.
    pub fn drain(&self) {
        let mut buffer = [0u8; 64];
        loop {
            // SAFETY: a read into a buffer that lives across the call.
            let got = unsafe {
                libc::read(
                    self.read_fd,
                    buffer.as_mut_ptr() as *mut libc::c_void,
                    buffer.len(),
                )
            };
            if got < buffer.len() as isize {
                return;
            }
        }
    }
}

impl Drop for Wakeup {
    fn drop(&mut self) {
        // SAFETY: two descriptors this type opened and nothing else holds.
        unsafe {
            libc::close(self.read_fd);
            libc::close(self.write_fd);
        }
    }
}

/// What a port waits on beside its own descriptor, so that a wait inside an
/// operation can be abandoned.
///
/// A node has two kinds of wait in it. The waits for data are not waits at
/// all -- a descriptor is asked whether it is ready and a silent device reads
/// as nought bytes -- but an operation can contain a wait of its own: the
/// delay between a trigger and the read that answers it, and a `process`
/// computation's deadline. This is what makes those abandonable.
///
/// Carried down into the ports and into a computation's own waiting, which is
/// the convention [`crate::endpoint`]'s waits take: an endpoint is handed a
/// `cmd_fd` and polls it beside the device's. The descriptor is readable exactly while
/// the node is disabled, and nothing but enabling the node drains it, so a
/// wait that finds it readable has been told to stop and not merely nudged.
///
/// [`Abandon::nothing`] is a port nothing can disable -- a link to the ground,
/// or a port in a test -- whose waits run to their deadlines as they always
/// did.
#[derive(Debug, Clone, Default)]
pub struct Abandon {
    /// The node's wakeup, held so the descriptor cannot be closed while a
    /// wait has it in a poll set.
    wakeup: Option<Arc<Wakeup>>,
}

impl Abandon {
    /// A wait that nothing interrupts.
    pub fn nothing() -> Abandon {
        Abandon { wakeup: None }
    }

    /// Whether the wait should be given up now, without waiting at all.
    ///
    /// Asked before an operation starts a wait, so that a disable that
    /// arrived between stages is not followed by one more delay.
    pub fn asked(&self) -> bool {
        self.wait(Duration::ZERO)
    }

    /// The descriptor to put in a poll set beside the node's own, where there
    /// is one.
    pub fn io_fd(&self) -> Option<RawFd> {
        self.wakeup.as_ref().map(|wakeup| wakeup.io_fd())
    }

    /// Wait `how_long`, or until the node is disabled, and say which happened.
    ///
    /// `true` is "give up": the node has been disabled and the operation stops
    /// where it stands. A disable that waited out a delay would arrive up to
    /// that delay late, which on a part that has stopped answering is exactly
    /// when it is wanted.
    ///
    /// A poll that fails is not treated as a disable. The failure is the
    /// poll's own -- a descriptor this closed, which cannot happen while the
    /// Arc above is held -- and abandoning the operation over it would stop a
    /// node nobody asked to stop.
    pub fn wait(&self, how_long: Duration) -> bool {
        use nix::poll::{poll, PollFd, PollFlags};

        let Some(wakeup) = self.wakeup.as_ref() else {
            if !how_long.is_zero() {
                std::thread::sleep(how_long);
            }
            return false;
        };

        // SAFETY: the descriptor is owned by the Wakeup this holds an Arc of,
        // which therefore outlives the poll.
        let borrowed = unsafe { std::os::fd::BorrowedFd::borrow_raw(wakeup.io_fd()) };
        let mut set = [PollFd::new(&borrowed, PollFlags::POLLIN)];
        let waiting = how_long.as_millis().min(i32::MAX as u128) as i32;
        matches!(poll(&mut set, waiting), Ok(n) if n > 0)
    }
}

/// What the loop waits on between passes, so that work can interrupt it.
///
/// A pass works out how long until the next thing is due and waits that long,
/// which may be seconds. Something that wants the pass sooner -- a command
/// that has to be acted on between stages -- pokes this, and the wait ends.
/// Without it a START_NODE would be answered when the next group came due,
/// which in a configuration read every ten seconds is nine seconds of a
/// ground station wondering whether its command arrived.
///
/// Drained after each wait, unlike a node's [`Abandon`]: a node stays disabled
/// until it is enabled, so its descriptor stays readable, where a nudge is one
/// event and a loop that never cleared it would spin.
#[derive(Debug, Clone)]
pub struct Nudge(Arc<Wakeup>);

impl Nudge {
    pub fn new() -> std::io::Result<Nudge> {
        Ok(Nudge(Arc::new(Wakeup::new()?)))
    }

    /// Ask for the wait to end now.
    pub fn poke(&self) {
        self.0.poke();
    }

    /// Wait up to `how_long`, or until something pokes this.
    ///
    /// A poll rather than a sleep, which is the whole point, and the pipe is
    /// drained afterwards whether it was the poke or the clock that ended the
    /// wait: anything in it has been dealt with by the pass that follows.
    pub fn wait(&self, how_long: Duration) {
        use nix::poll::{poll, PollFd, PollFlags};

        if !how_long.is_zero() {
            // SAFETY: the descriptor is owned by the Wakeup this holds an Arc
            // of, which therefore outlives the poll.
            let borrowed = unsafe { std::os::fd::BorrowedFd::borrow_raw(self.0.io_fd()) };
            let mut set = [PollFd::new(&borrowed, PollFlags::POLLIN)];
            let waiting = how_long.as_millis().min(i32::MAX as u128) as i32;
            // A failed poll is waited out instead. The failure is the poll's
            // own and a loop that treated it as a nudge would spin.
            if poll(&mut set, waiting).is_err() {
                std::thread::sleep(how_long);
            }
        }
        self.0.drain();
    }
}

/// How an operation went, for the count it belongs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// It happened, carrying this many bytes.
    Moved(usize),
    /// It was attempted and failed.
    Failed,
}

/// What a node has moved, counted as it moves.
///
/// Atomics, for the reason the enabled flag is one: the command interpreter
/// reads these from a thread of its own while the pass is writing them, and a
/// pass that had to take a lock the interpreter also takes would be a pass the
/// interpreter could stall.
///
/// Relaxed ordering throughout. These are counts an operator reads, not a
/// handshake: nothing decides anything by them, and a reader that saw the
/// bytes of a transfer before its count would be reading a number that is
/// about to be right. What matters is that none is lost, which the atomic
/// addition gives on its own.
///
/// They survive a disable, because what they say is what this node has done
/// since the vehicle started: see [`tcslibgs::NodeCounts`].
#[derive(Debug, Default)]
pub struct Counters {
    taken_bytes: AtomicU64,
    reads: AtomicU64,
    reads_failed: AtomicU64,
    given_bytes: AtomicU64,
    writes: AtomicU64,
    writes_failed: AtomicU64,
    dropped: AtomicU64,
    asks: AtomicU64,
    asks_failed: AtomicU64,
    /// Microseconds since the epoch, or nought for a node that has moved
    /// nothing. Nought and not a sentinel: the epoch is not a time anything
    /// here will move something at, and `None` is what it becomes on the way
    /// out.
    last_moved: AtomicU64,
}

impl Counters {
    fn moved(&self, at: Timestamp) {
        self.last_moved.store(at.micros(), Ordering::Relaxed);
    }

    /// What they say, as the ground is told it.
    pub fn counts(&self) -> NodeCounts {
        let read = |counter: &AtomicU64| counter.load(Ordering::Relaxed);
        NodeCounts {
            taken_bytes: read(&self.taken_bytes),
            reads: read(&self.reads),
            reads_failed: read(&self.reads_failed),
            given_bytes: read(&self.given_bytes),
            writes: read(&self.writes),
            writes_failed: read(&self.writes_failed),
            dropped: read(&self.dropped),
            asks: read(&self.asks),
            asks_failed: read(&self.asks_failed),
            last_moved: match read(&self.last_moved) {
                0 => None,
                micros => Some(Timestamp { micros }),
            },
        }
    }
}

/// The last datum each way, for an operator who wants the bytes rather than a
/// count of them.
///
/// Behind a mutex rather than an atomic, a sample being forty-odd bytes. Taken
/// with `try_lock` where it is recorded: keeping a sample is best effort, and
/// the path that moves payload data does not wait for a reader of the samples
/// to finish. A lost sample costs one stale line on a display, where a blocked
/// pass costs data.
#[derive(Debug, Default)]
pub struct Samples {
    /// The last thing the vehicle took from this node.
    pub taken: DHSample,
    /// And the last thing it gave it.
    pub given: DHSample,
}

/// One node's state.
///
/// Shared between the command interpreter's thread and the pass, which is why
/// the flag is an atomic and not a field behind a lock: see the module note.
#[derive(Debug)]
pub struct NodeState {
    /// What the node is called, for a message about it.
    name: String,
    /// Its node id, which is what a command names it by.
    id: NodeId,
    /// Whether the pass may read it, ask it or deliver to it.
    enabled: AtomicBool,
    /// The descriptor a disable wakes it through.
    ///
    /// Behind an `Arc` because the ports hold it too: a wait inside one of
    /// this node's operations has it in its poll set, and a descriptor closed
    /// while a poll held it would be a poll on nothing.
    wakeup: Arc<Wakeup>,
    /// What it has moved since the vehicle started.
    counters: Counters,
    /// The last datum each way.
    samples: Mutex<Samples>,
}

impl NodeState {
    /// A node as it comes up: enabled.
    ///
    /// Every node of a checked configuration is opened when the vehicle starts
    /// and runs until it is stopped, because what a graph describes is a
    /// system that works as a whole -- telemetry reaches the ground with no
    /// ground station having said anything, which is what a vehicle out of
    /// contact has to do. Nodes that came up idle and were told which to start
    /// would suit a set of separately useful payloads, and do not suit a graph
    /// whose links describe one machine.
    pub fn new(name: &str, id: NodeId) -> std::io::Result<NodeState> {
        Ok(NodeState {
            name: name.to_string(),
            id,
            enabled: AtomicBool::new(true),
            wakeup: Arc::new(Wakeup::new()?),
            counters: Counters::default(),
            samples: Mutex::new(Samples::default()),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    /// Whether the pass may touch this node.
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// Let the pass touch it again.
    ///
    /// Enabling one that is enabled is success and changes nothing, a command
    /// that arrives twice being the ordinary case on a link with latency.
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::SeqCst);
        // Whatever a previous disable put in the pipe, so that the next wait
        // this node makes is not woken by a disable that has been answered.
        self.wakeup.drain();
    }

    /// Stop the pass touching it, at once.
    ///
    /// The flag first and the poke second, which is the order that matters: a
    /// wait woken before the flag was down would look up, find the node
    /// enabled and carry on waiting, and the disable would arrive a timeout
    /// later after all.
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::SeqCst);
        self.wakeup.poke();
    }

    /// The descriptor to put in this node's poll sets.
    pub fn io_fd(&self) -> RawFd {
        self.wakeup.io_fd()
    }

    /// What this node has moved, as the ground is told it.
    pub fn counts(&self) -> NodeCounts {
        self.counters.counts()
    }

    /// The last datum each way, for an operator who wants the bytes.
    ///
    /// A lock that cannot be had gives nothing rather than waiting: the
    /// samples are best effort, and a reader is not a reason to hold up the
    /// pass that is writing them.
    pub fn samples(&self) -> Samples {
        match self.samples.try_lock() {
            Ok(held) => Samples {
                taken: held.taken,
                given: held.given,
            },
            Err(_) => Samples::default(),
        }
    }

    /// Record what the vehicle took from this node.
    pub fn took(&self, how: Outcome) {
        match how {
            Outcome::Moved(bytes) => {
                self.counters.taken_bytes.fetch_add(bytes as u64, Ordering::Relaxed);
                self.counters.reads.fetch_add(1, Ordering::Relaxed);
                self.counters.moved(Timestamp::now());
            }
            Outcome::Failed => {
                self.counters.reads_failed.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// And what it gave it.
    pub fn gave(&self, how: Outcome) {
        match how {
            Outcome::Moved(bytes) => {
                self.counters.given_bytes.fetch_add(bytes as u64, Ordering::Relaxed);
                self.counters.writes.fetch_add(1, Ordering::Relaxed);
                self.counters.moved(Timestamp::now());
            }
            Outcome::Failed => {
                self.counters.writes_failed.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// A delivery dropped because this node is disabled.
    pub fn dropped(&self) {
        self.counters.dropped.fetch_add(1, Ordering::Relaxed);
    }

    /// A trigger sent to it, or one it would not take.
    pub fn asked(&self, how: Outcome) {
        match how {
            Outcome::Moved(_) => {
                self.counters.asks.fetch_add(1, Ordering::Relaxed);
            }
            Outcome::Failed => {
                self.counters.asks_failed.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Keep the last datum taken from this node.
    pub fn sample_taken(&self, data: &[u8]) {
        if let Ok(mut held) = self.samples.try_lock() {
            held.taken.record(data);
        }
    }

    /// And the last given it.
    pub fn sample_given(&self, data: &[u8]) {
        if let Ok(mut held) = self.samples.try_lock() {
            held.given.record(data);
        }
    }

    /// What this node's ports wait on beside their own descriptors.
    ///
    /// Given to a port when it is opened, so that a wait inside one of its
    /// operations ends the moment the node is disabled: see [`Abandon`].
    pub fn abandon(&self) -> Abandon {
        Abandon {
            wakeup: Some(self.wakeup.clone()),
        }
    }
}

/// Every node's state, by name and by id.
///
/// Built once, from the graph, and shared: the command interpreter holds it to
/// change a state and the pass holds it to read one. Both tables are over the
/// same `Arc`s, so there is one state per node however it is reached.
#[derive(Debug, Default)]
pub struct NodeStates {
    by_name: BTreeMap<String, Arc<NodeState>>,
    /// Indexed by node id, which is a node's position: see
    /// [`tcslibgs::graph::telemetry::ids`].
    by_id: Vec<Arc<NodeState>>,
}

impl NodeStates {
    /// A state for every node of this configuration.
    ///
    /// Ids from `telemetry::ids`, which is the one algorithm that gives them:
    /// two tables computed separately could drift apart, and the drift would
    /// show as commands about the wrong node rather than as an error.
    pub fn of(graph: &Graph) -> std::io::Result<NodeStates> {
        let ids = tcslibgs::graph::telemetry::ids(graph);
        let mut states = NodeStates::default();
        // In id order, so that `by_id` can be indexed by the id itself.
        let mut in_order: Vec<(usize, &str)> = ids
            .iter()
            .map(|(name, &id)| (id, name.as_str()))
            .collect();
        in_order.sort_unstable();
        for (id, name) in in_order {
            let state = Arc::new(NodeState::new(name, NodeId::of(id))?);
            states.by_name.insert(name.to_string(), state.clone());
            states.by_id.push(state);
        }
        Ok(states)
    }

    /// How many nodes have a state here.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// The state of the node with this id, or `None` for an id the
    /// configuration does not have.
    ///
    /// A fixed-length field holds any number, so an id out of range is a thing
    /// to answer rather than a thing that failed to parse: the caller says
    /// which id and how many nodes there are.
    pub fn at(&self, id: NodeId) -> Option<&Arc<NodeState>> {
        self.by_id.get(id.position())
    }

    /// And by name, for the pass, which knows nodes by name.
    pub fn named(&self, name: &str) -> Option<&Arc<NodeState>> {
        self.by_name.get(name)
    }

    /// Whether the pass may touch this node.
    ///
    /// A name with no state is enabled. Every node of the graph has one, so
    /// the names that reach this without one are the links to the ground the
    /// system holds beside the nodes -- a payload's telemetry link is not a
    /// node the ground can stop, and the node that feeds it is.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.by_name
            .get(name)
            .is_none_or(|state| state.is_enabled())
    }

    /// The names of the nodes that are disabled, in order, for a message.
    pub fn disabled(&self) -> Vec<&str> {
        self.by_id
            .iter()
            .filter(|state| !state.is_enabled())
            .map(|state| state.name())
            .collect()
    }
}

/// What the fabric asks about a node, and tells about it.
///
/// The enabled answer is the one the command interpreter set, read without
/// taking anything: a pass that had to take a lock the interpreter also takes
/// would be a pass the interpreter could stall, which is the thing the
/// separate thread was for. The counts go the other way, written by the pass
/// and read by the interpreter, and are atomics for the same reason.
///
/// A name with no state is a link to the ground rather than a node, and
/// nothing is counted for it: what reaches the ground is counted at the node
/// that produced it, and counting the link as well would be the same bytes
/// twice.
impl crate::runtime::fabric::Watch for NodeStates {
    fn is_enabled(&self, node: &str) -> bool {
        NodeStates::is_enabled(self, node)
    }

    fn took(&self, node: &str, how: Outcome) {
        if let Some(state) = self.named(node) {
            state.took(how);
        }
    }

    fn gave(&self, node: &str, how: Outcome) {
        if let Some(state) = self.named(node) {
            state.gave(how);
        }
    }

    fn dropped(&self, node: &str) {
        if let Some(state) = self.named(node) {
            state.dropped();
        }
    }

    fn asked(&self, node: &str, how: Outcome) {
        if let Some(state) = self.named(node) {
            state.asked(how);
        }
    }

    fn sample_taken(&self, node: &str, data: &[u8]) {
        if let Some(state) = self.named(node) {
            state.sample_taken(data);
        }
    }

    fn sample_given(&self, node: &str, data: &[u8]) {
        if let Some(state) = self.named(node) {
            state.sample_given(data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A poke wakes a poll on the descriptor, and nothing else does.
    #[test]
    fn a_poke_wakes_a_wait_and_silence_does_not() {
        use nix::poll::{poll, PollFd, PollFlags};
        use std::os::fd::BorrowedFd;

        let wakeup = Wakeup::new().expect("a pipe");
        // SAFETY: the descriptor is owned by `wakeup`, which outlives this.
        let borrowed = unsafe { BorrowedFd::borrow_raw(wakeup.io_fd()) };

        let waited = |flags| {
            let mut set = [PollFd::new(&borrowed, flags)];
            poll(&mut set, 0).expect("a poll")
        };

        // Nothing has poked it: a wait on it finds nothing, which is what lets
        // it sit in a poll set beside a device's descriptor without waking
        // every pass.
        assert_eq!(waited(PollFlags::POLLIN), 0, "nothing has poked it");

        wakeup.poke();
        assert_eq!(waited(PollFlags::POLLIN), 1, "a poke is readable");

        // And drained, it is quiet again: a wait woken by one disable must not
        // return at once for ever after.
        wakeup.drain();
        assert_eq!(waited(PollFlags::POLLIN), 0, "drained");
    }

    /// Disabling a node wakes it; enabling it leaves nothing behind.
    #[test]
    fn disabling_a_node_wakes_it_and_enabling_it_clears_the_wake() {
        use nix::poll::{poll, PollFd, PollFlags};
        use std::os::fd::BorrowedFd;

        let state = NodeState::new("sensor", NodeId::of(3)).expect("a state");
        assert!(state.is_enabled(), "a node comes up enabled");
        assert_eq!(state.id(), NodeId::of(3));

        let readable = || {
            // SAFETY: the descriptor is owned by `state`, which outlives this.
            let borrowed = unsafe { BorrowedFd::borrow_raw(state.io_fd()) };
            let mut set = [PollFd::new(&borrowed, PollFlags::POLLIN)];
            poll(&mut set, 0).expect("a poll") == 1
        };

        assert!(!readable(), "nothing has disabled it");
        state.disable();
        assert!(!state.is_enabled());
        assert!(readable(), "a disable reaches a node that is waiting");

        // Twice is a state and not an action: a command that arrives twice, or
        // whose answer was lost, is the ordinary case on a link with latency.
        state.disable();
        assert!(!state.is_enabled());

        state.enable();
        assert!(state.is_enabled());
        assert!(
            !readable(),
            "an enabled node's next wait must not be woken by the disable before it"
        );
    }

    /// Every node of a configuration has a state, numbered as its telemetry
    /// is.
    ///
    /// The ids are the device ids, which is the whole reason a command names a
    /// node by one: the id in a command and the port that node's telemetry
    /// arrives on are the same number.
    #[test]
    fn every_node_has_a_state_numbered_as_its_telemetry_is() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("two.yaml");
        std::fs::write(
            &path,
            "payloads:\n\
             \x20 - { name: inside, type: i2c, path: /dev/i2c-1, address: \"0x48\", \
             poll_interval: \"1 s\", payload-out: average }\n\
             vehicle:\n\
             \x20 - { name: average, type: compute, function: mean, inputs: 1 }\n",
        )
        .expect("written");
        let graph = Graph::load(&path).expect("it reads");

        let states = NodeStates::of(&graph).expect("states");
        assert_eq!(states.len(), 2);

        let ids = tcslibgs::graph::telemetry::ids(&graph);
        for (name, &id) in &ids {
            let by_id = states.at(NodeId::of(id)).expect("that id");
            assert_eq!(by_id.name(), name, "id {id} is {name}");
            // And the same state however it is reached, which is what makes
            // the interpreter's store and the pass's load one fact.
            assert!(Arc::ptr_eq(by_id, states.named(name).expect("that name")));
        }

        // An id the configuration does not have is nothing rather than a
        // panic: a fixed-length field holds any number, so this is a thing to
        // answer.
        assert!(states.at(NodeId::of(2)).is_none());
        assert!(states.at(NodeId::of(u32::MAX as usize)).is_none());

        // Everything is enabled, and a name that is not a node -- a link to
        // the ground -- reads as enabled too: it is not something the ground
        // can stop, and the node feeding it is.
        assert!(states.disabled().is_empty());
        assert!(states.is_enabled("inside"));
        assert!(states.is_enabled("inside telemetry"));

        states.named("inside").expect("the node").disable();
        assert_eq!(states.disabled(), vec!["inside"]);
        assert!(!states.is_enabled("inside"));
        assert!(states.is_enabled("average"), "only the one named");
    }
}
