//! Moving bytes along the links.
//!
//! The routing says where a node's output goes and the schedule says when a
//! group is read; this is what does it. One method of consequence --
//! [`Fabric::deliver`] -- which takes what a node produced and hands it to
//! every place the configuration says it goes.
//!
//! # A thread per node
//!
//! Every node has a thread of its own, made when the node is first held, and
//! a pass is a run of stages: the reads of a group, then a wave of deliveries,
//! then the next wave. Every operation of a stage runs at once, each on its
//! node's thread, and the stage ends when the last of them has -- so a slow
//! device holds up nothing but itself, and three sensors of a group are read
//! at one moment rather than at three. See [`Worker`] for why a thread per
//! node rather than a thread per operation or a pool.
//!
//! What a node *is* is deliberately a trait here rather than an endpoint. The
//! flight-side nodes are endpoints and plug straight in, but a fabric that
//! could only hold endpoints could only be tested with real ones -- a bus, a
//! socket, a chip -- and the behaviour worth testing is the fabric's: that a
//! fan-out reaches all three destinations, that a named input arrives as
//! itself, that a node refusing a datum does not stop the others getting
//! theirs. With a trait those are tests against a recording node and no
//! hardware at all.
//!
//! # Which stream a datum is on
//!
//! A configuration has two streams in it -- what travels groundward, from what
//! the vehicle carries towards the ground, and what travels vehicleward, the
//! other way -- and most nodes need no opinion about which one a datum
//! belongs to. A device has one source terminal, so where its readings go is
//! the link's business; a computation's result is one result however many ways
//! the configuration sends it. Those answer [`Taken`] with no stream, and
//! every link out of them is followed, which is what this did before any of
//! this was here.
//!
//! A node with more than one of either has to be told and has to say. An
//! `app_all_veh` is one: it reads payload data on one interface and MOC data
//! on the other, so a datum it produces belongs to one of the two and
//! delivering it to the other's links would send telemetry out as a command.
//! So the stream travels with the bytes -- [`Taken::stream`] on the way out,
//! [`Arriving::stream`] on the way in -- and [`Fabric::deliver_on`] follows
//! only the links that carry it. Which stream a link carries is settled once,
//! where the table is built: see `route::stream_of`.
//!
//! Delivery is best-effort per destination and the failures are counted. That
//! is the reliability choice and it is worth stating: a log that cannot be
//! written must not stop the ground getting its telemetry, and a heater
//! circuit that will not take a command must not stop the next reading being
//! taken. What must never happen is a failure that nobody hears about, so
//! every one is counted and named.

use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::thread::JoinHandle;

use tcslibgs::graph::terminals::Stream;

use super::node_state::Outcome;
use super::route::{Delivery, Routes};

/// Where a datum's bytes are while it is passed on.
///
/// Never owned, so that passing one on allocates nothing: the datum a
/// delivery was asked to start from is copied into the fabric's own buffer,
/// and everything after it is a result, in the node that produced it.
#[derive(Debug, Clone, Copy)]
enum Bytes {
    /// The datum [`Fabric::deliver`] was given, in [`Shared::origin`].
    Origin,
    /// A result, in one of a node's two banks: see [`Worker::banks`].
    Result {
        node: usize,
        bank: usize,
        item: usize,
    },
}

/// One datum on its way to the places a node's links lead.
///
/// Which node's links it follows, where its bytes are, how far it has already
/// been passed on -- see the waves in [`Fabric::cascade`] -- which stream it is
/// on, and whether it is something said again rather than something new.
#[derive(Debug, Clone, Copy)]
struct Passing {
    /// The node whose links it follows, by its index in [`Shared::names`].
    from: usize,
    bytes: Bytes,
    len: usize,
    depth: usize,
    stream: Option<Stream>,
    again: bool,
}

/// One datum handed to one node: a passing, and which of its links it came
/// down, by its place among `Routes::from(passing.from)`.
#[derive(Debug, Clone, Copy)]
struct Arrival {
    passing: Passing,
    delivery: usize,
}

/// A result a node produced in a wave, at `item` in the bank that wave
/// writes.
#[derive(Debug, Clone, Copy)]
struct Produced {
    item: usize,
    taken: Taken,
    depth: usize,
}

/// How a node is read in a stage of reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Read {
    /// Take what it has: [`Port::take`].
    Take,
    /// Take what has arrived without asking it: [`Port::take_unasked`].
    Unasked,
    /// The same, and then send its trigger: [`Port::ask`]. For a payload
    /// that sends on its own and is asked on a timer of its own, when the
    /// timer says.
    UnaskedThenAsk,
}

/// What a node's thread is given to do in one stage.
#[derive(Debug, Clone, Copy)]
enum Job {
    /// Read it, the result going to item 0 of `bank`.
    Read { how: Read, bank: usize },
    /// Give it everything in its inbox, in order, taking a result after each
    /// one it has a result for, into `bank`. `most` is how far a result may
    /// have been passed on before it has gone round a cycle.
    Deliver { bank: usize, most: usize },
}

/// What a node handed over.
///
/// The bytes, and which of the two streams they belong to where the node keeps
/// them apart. `None` is "wherever my links go", which is what every node here
/// answered before an app node could be one of them: a device has one source
/// terminal, so which way its data is going is the link's business and not
/// its own, and a computation's result is one result however many ways the
/// configuration sends it.
///
/// `Some` is a node that produces on more than one terminal and knows which
/// -- an `app_all_veh`, whose two interfaces are two streams. Delivering its
/// reading to the links leaving its `moc-out` would send telemetry out as a
/// command, so the stream travels with the bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Taken {
    /// How many bytes of the buffer were filled.
    pub bytes: usize,
    /// Which stream they are on, where the node says.
    pub stream: Option<Stream>,
    /// Whether this is something said again rather than something new.
    ///
    /// A thermostat asserts its decision on every sample, so that a heater is
    /// held at what the vehicle last decided and a lost datagram costs one
    /// sample rather than standing for ever. Only the devices are told again:
    /// a log or a ground link that recorded every repeat would bury the
    /// moment anything switched. See [`Fabric::deliver_on`].
    pub again: bool,
}

impl Taken {
    /// Nothing to say.
    pub const NOTHING: Taken = Taken {
        bytes: 0,
        stream: None,
        again: false,
    };

    /// This many bytes, to go wherever this node's links go.
    pub fn of(bytes: usize) -> Taken {
        Taken {
            bytes,
            stream: None,
            again: false,
        }
    }

    /// And this many, on one stream.
    pub fn on(bytes: usize, stream: Stream) -> Taken {
        Taken {
            bytes,
            stream: Some(stream),
            again: false,
        }
    }

    /// This many bytes, which are what was said before, said again.
    pub fn again(bytes: usize) -> Taken {
        Taken {
            bytes,
            stream: None,
            again: true,
        }
    }
}

/// What arrived at a node, and where it arrived.
///
/// Two questions a delivery answers, and a node may care about either, both or
/// neither: which of its named inputs this one fills, and which stream it is
/// on. A device cares about neither -- it has one way in -- a mean cares about
/// the first, and an `app_all_veh` cares about the second.
#[derive(Debug, Clone, Copy, Default)]
pub struct Arriving<'a> {
    /// Which of the node's named inputs it fills, where it names them.
    ///
    /// `None` is a node that takes one input and does not name it, which is
    /// most of them.
    pub input: Option<&'a str>,
    /// Which stream it is on, where the link settles it. See
    /// `route::stream_of`.
    pub stream: Option<Stream>,
}

impl<'a> Arriving<'a> {
    /// At a node's one way in, on no stream in particular.
    ///
    /// What a caller with nothing to say about either wants, which is most of
    /// the tests and anything handing a datum to a device.
    pub const ANYWHERE: Arriving<'static> = Arriving {
        input: None,
        stream: None,
    };

    /// At a named input.
    pub fn named(input: &'a str) -> Arriving<'a> {
        Arriving {
            input: Some(input),
            stream: None,
        }
    }

    /// On one stream, at whichever way in that stream arrives by.
    pub fn on(stream: Stream) -> Arriving<'static> {
        Arriving {
            input: None,
            stream: Some(stream),
        }
    }
}

/// Anything the fabric can hand bytes to, or take them from.
///
/// Both halves are optional in practice: a sensor is read and never written,
/// a set of heater lines is written and never read. A node says which it does
/// by what it returns -- [`Port::accepts`] and [`Port::produces`] -- rather
/// than by implementing one of two traits, because the fabric holds them all
/// in one table and a table of two kinds would be two tables.
///
/// `Send`, because a node operation is to run on a thread of its own: see the
/// lock each port is held behind in [`Fabric`]. Every port here is a thing
/// with a descriptor, a child process or a file, all of which move between
/// threads; what the bound rules out is a port holding something that cannot,
/// which a stand-in in a test is the likeliest thing to be.
pub trait Port: Send {
    /// Take what this node has produced, if anything.
    ///
    /// Nought bytes is "nothing to say", which is the ordinary answer from a
    /// device that is not due. An error is a failure to read a node that
    /// should have had something.
    fn take(&mut self, buffer: &mut [u8]) -> Result<Taken, String>;

    /// Take what has arrived at this node, without asking it anything.
    ///
    /// For a payload that sends on its own and is asked on a timer of its own:
    /// its answers are taken on whatever pass they arrive on, and the asking is
    /// [`Port::ask`]'s, when the timer says. The same as [`Port::take`] for
    /// every node that is never asked, which is what the default is.
    fn take_unasked(&mut self, buffer: &mut [u8]) -> Result<Taken, String> {
        self.take(buffer)
    }

    /// How many datagrams this node turned away as a stranger's since this
    /// was last asked: see [`crate::endpoint::EndpointReadable::strangers`].
    /// Nought for every node that has nobody to turn away.
    fn strangers(&mut self) -> u64 {
        0
    }

    /// Ask this node for a reading, and take nothing: send its trigger.
    ///
    /// Nothing for a node that has no trigger, which is every node but a
    /// payload that is asked.
    fn ask(&mut self) -> Result<(), String> {
        Ok(())
    }

    /// Hand this node something, saying where it arrived.
    fn give(&mut self, arriving: Arriving<'_>, data: &[u8]) -> Result<(), String>;

    /// Whether anything may be handed to this node.
    fn accepts(&self) -> bool {
        true
    }

    /// Whether this node is a device that is held at what it was last told.
    ///
    /// A heater and a valve are: the vehicle writes a decision and the thing
    /// stays that way until it is written again, so a decision asserted a
    /// second time is worth sending and the one that went missing is worth
    /// correcting. Everything else is not -- a log, a ground link, another
    /// computation -- and a repeat tells it nothing it does not have. Which is
    /// what decides where a re-assertion goes: see [`Fabric::deliver_on`].
    fn holds(&self) -> bool {
        false
    }

    /// Whether anything may be taken from it.
    fn produces(&self) -> bool {
        true
    }

    /// Set a named attribute of this node, as a command from the ground does.
    ///
    /// Refused by default, which is what a device answers: a bus address or a
    /// set of line numbers is what the configuration said, and a vehicle whose
    /// hardware could be re-described from the ground in flight would have no
    /// checked configuration at all. What the ground may change is a parameter
    /// of a computation -- a setpoint, a band -- and the configuration says
    /// which, as a command binding.
    fn set(&mut self, attribute: &str, _value: &[u8]) -> Result<(), String> {
        Err(format!(
            "there is nothing called \"{attribute}\" on this node that the ground \
             may set"
        ))
    }

    /// Push whatever this node has buffered to where it belongs.
    ///
    /// For a log, which does not write a record to its file as it takes it: a
    /// log flushed per arrival is a write system call per reading. Nothing for
    /// every other kind of node, which is why the default does nothing -- a
    /// socket and a bus have handed their bytes over by the time a write
    /// returns.
    fn flush(&mut self) -> Result<(), String> {
        Ok(())
    }

    /// Whether this node has a result waiting as a consequence of what it was
    /// just given.
    ///
    /// The difference between a node that produces on its own schedule and one
    /// that produces because something arrived. A sensor is the first: it has
    /// a reading when its group is due, and nothing a delivery does changes
    /// that -- so the default here is `false` and a device never answers
    /// otherwise. A computation is the second: a mean handed its third input
    /// has an average to pass on *now*, and a pass that ended with the
    /// delivery would leave it there until something else happened to read the
    /// node.
    ///
    /// Asked rather than simply reading every destination after a delivery,
    /// because reading a device is not free and reading a MOC link would take
    /// a command off it and pass it on as though it were a result.
    fn ready(&self) -> bool {
        false
    }
}

/// Whether this link carries a datum on this stream.
///
/// Both ways round, and the answer is yes unless the two disagree: a delivery
/// whose stream nothing settled takes whatever comes, and a datum on no stream
/// in particular goes everywhere its node's links go. Which keeps every
/// configuration that worked before working: it is only a node that says which
/// stream it produced on, delivering to a link that says which stream it
/// carries, that can be told apart at all.
fn carries(delivery: &Delivery, stream: Option<Stream>) -> bool {
    match (delivery.stream, stream) {
        (Some(link), Some(datum)) => link == datum,
        _ => true,
    }
}

/// What a pass did, and what went wrong with it.
///
/// Counted rather than returned as an error, because a pass with one failed
/// delivery in it is still a pass that moved everything else, and a caller
/// that treated it as a failure would stop a vehicle over a full log.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Moved {
    /// How many deliveries were made.
    pub delivered: usize,
    /// How many bytes those carried.
    pub bytes: usize,
    /// What failed, as node and reason, in the order it happened.
    ///
    /// Every one of them, not a count: an operator asking why a reading is
    /// not reaching the ground wants to know which link refused it.
    pub refused: Vec<(String, String)>,
    /// How many deliveries were dropped because the node was disabled.
    ///
    /// A count and not a list of reasons, which is the difference between this
    /// and `refused`: a delivery to a node the ground has stopped is the
    /// configuration working as asked, and naming it every pass would fill a
    /// log with the news that a payload is off. What it must not be is
    /// invisible, so it is counted.
    pub dropped: usize,
}

impl Moved {
    /// Whether anything went wrong.
    pub fn all_delivered(&self) -> bool {
        self.refused.is_empty()
    }

    /// Take in what another pass moved.
    ///
    /// Public because a pass is made of several: a group, then another, then
    /// the polled nodes, then the logs -- and what the caller wants at the end
    /// is one account of all of it, failures included.
    pub fn add(&mut self, other: Moved) {
        self.delivered += other.delivered;
        self.bytes += other.bytes;
        self.dropped += other.dropped;
        self.refused.extend(other.refused);
    }
}

/// What the fabric asks about a node, and tells about it.
///
/// Two things, and they belong together because both are the node's own
/// rather than the fabric's. Whether a node may be touched is set by the
/// command interpreter from a thread of its own, and what a node has moved is
/// read by that interpreter while the pass is writing it: see
/// [`crate::runtime::node_state`]. Behind a trait so that the fabric itself
/// stays a thing that can be tested with no node states and no command link
/// in sight.
///
/// It was called `Gate` when it only answered the first question. A gate that
/// also counts is a gate in name only.
pub trait Watch: Send + Sync {
    /// Whether this node may be touched. A name this knows nothing about is
    /// enabled: a link to the ground is not something the ground can stop, and
    /// the node that feeds it is.
    fn is_enabled(&self, node: &str) -> bool;

    /// What the vehicle took from this node.
    ///
    /// Nothing by default, so a fabric nobody is watching counts nothing and
    /// costs nothing: the counts exist to be asked for, and nothing in a pass
    /// decides anything by them.
    fn took(&self, node: &str, how: Outcome) {
        let _ = (node, how);
    }

    /// What it gave it.
    fn gave(&self, node: &str, how: Outcome) {
        let _ = (node, how);
    }

    /// A delivery dropped because the node is disabled.
    fn dropped(&self, node: &str) {
        let _ = node;
    }

    /// A trigger sent to it, or one it would not take.
    fn asked(&self, node: &str, how: Outcome) {
        let _ = (node, how);
    }

    /// Datagrams its socket turned away as a stranger's.
    fn strangers(&self, node: &str, count: u64) {
        let _ = (node, count);
    }

    /// The last datum taken from it, kept for an operator who wants the bytes.
    fn sample_taken(&self, node: &str, data: &[u8]) {
        let _ = (node, data);
    }

    /// And the last given it.
    fn sample_given(&self, node: &str, data: &[u8]) {
        let _ = (node, data);
    }
}

/// What a fabric nobody is watching has: everything enabled, nothing counted.
///
/// What the fabric did before there was a command link, and what its own tests
/// use. A separate type rather than an `Option<Arc<dyn Watch>>` with a branch
/// at each call, because a branch per delivery is a branch to get wrong.
struct Unwatched;

impl Watch for Unwatched {
    fn is_enabled(&self, _node: &str) -> bool {
        true
    }
}

/// One node's thread, and what it shares with the conductor.
///
/// A thread per node, started when the node is first held, rather than a
/// thread per operation or a pool. One per operation would be a thread made
/// and joined for every read and every delivery, and making a thread
/// allocates -- its stack, its handle, the closure it runs -- which a pass
/// may not do. A pool has a size, and nothing can choose it: one smaller than
/// a stage serialises operations the graph says may run together, and the
/// figure that is too small depends on the configuration. A thread per node
/// is the parallelism the file describes and no other figure: a stage never
/// has more operations than nodes, since a node is held by one thread at a
/// time, and the threads are made where everything else is, as the vehicle
/// starts.
///
/// The conductor and the thread take turns. The conductor fills the mailbox
/// and sets a job while the thread waits; the thread does the job holding the
/// mailbox and the port; the conductor reads the mailbox once every thread of
/// the stage has said it has finished. Nothing is ever touched by both at
/// once, and every wait on either side is on a condition variable, which
/// allocates nothing.
struct Worker {
    /// What the configuration calls the node, for everything said about it.
    name: String,
    /// The node, while it is held: `None` once the ground has released it.
    ///
    /// Locked by this node's thread for an operation, and by the conductor
    /// between stages -- to set an attribute, flush a log, release it -- and by
    /// nothing else: a node is a thing with a descriptor, a position in a
    /// stream and a buffer, and two threads inside one port would be two
    /// readers of one socket.
    port: Mutex<Option<Box<dyn Port>>>,
    /// Where this node's results are written, two banks of them.
    ///
    /// A wave delivers what the wave before it produced and produces what the
    /// next will deliver, so a wave reads one bank of every node and writes
    /// the other, and the two swap at the next wave. A node whose result is on
    /// its way to its destinations can be given something itself in the same
    /// wave -- a diamond in the graph does that -- and a single buffer would
    /// be written while it was read.
    ///
    /// Each bank holds as many results as this node has ever produced in one
    /// wave, made the first time it produces that many and kept: a node
    /// produces one result per arrival at most, and most produce one.
    banks: [RwLock<Vec<Vec<u8>>>; 2],
    mail: Mutex<Mail>,
    /// What the thread waits on for a job.
    wake: Condvar,
    /// Whether there is a thread at all. A thread the system would not start
    /// leaves the node's work to the conductor, done in turn rather than at
    /// once: slower, and still the vehicle's work done.
    threaded: bool,
}

/// What passes between the conductor and one node's thread.
///
/// Made when the node is first held, and reused: the inbox and the list of
/// results keep the room the busiest wave gave them.
#[derive(Default)]
struct Mail {
    job: Option<Job>,
    /// What this node is given in the wave under way, in the order it
    /// arrived.
    inbox: Vec<Arrival>,
    /// What it produced.
    produced: Vec<Produced>,
    /// What it moved and what went wrong, for the conductor to add to the
    /// pass's account.
    moved: Moved,
    /// Whether the fabric has gone, and the thread with it.
    stop: bool,
}

/// What every node's thread and the conductor share.
struct Shared {
    /// Every node ever held, and every node a delivery was asked to start
    /// from, by index. Added to only between stages.
    names: RwLock<Vec<String>>,
    /// Each node's thread, by the same index. `None` for a name that is only
    /// ever started from and never held.
    workers: RwLock<Vec<Option<Arc<Worker>>>>,
    routes: Routes,
    /// What says whether a node may be touched, and is told what it moved:
    /// see [`Watch`].
    watch: RwLock<Arc<dyn Watch>>,
    /// How large a datum may be, which is the buffer a read is given.
    buffer: usize,
    /// The datum [`Fabric::deliver`] was given, copied here so that every
    /// thread can read it.
    origin: RwLock<Vec<u8>>,
    /// How many threads of the stage under way have not finished, and what
    /// the conductor waits on for it to reach nought.
    remaining: Mutex<usize>,
    finished: Condvar,
}

/// A lock, taken whether or not a thread panicked holding it.
///
/// A poisoned lock means a thread panicked while it held this, which is a
/// fault to find in a log rather than a reason to stop reading a sensor for
/// the rest of the flight: what it guards is in whatever state that operation
/// left it, and the next operation is the one thing that might get it out of
/// there.
fn locked<T: ?Sized>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(|e| e.into_inner())
}

fn read<T: ?Sized>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|e| e.into_inner())
}

fn write<T: ?Sized>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(|e| e.into_inner())
}

/// The nodes, and the table saying how they are joined.
///
/// Each node has a thread of its own, and a pass is a run of stages: the
/// reads of a group, then the deliveries of what they read, a wave at a time.
/// Every operation of a stage runs at once, each on its node's thread, and the
/// stage ends when the last of them has; nothing of the next stage starts
/// before. That is the only order kept, and the one the graph's meaning rests
/// on: readings that will be compared are taken together, and every
/// destination of a wave has a datum before any of their own destinations are
/// given one.
pub struct Fabric {
    shared: Arc<Shared>,
    /// The index of each name in [`Shared::names`].
    index: BTreeMap<String, usize>,
    /// How many nodes are held.
    held: usize,
    /// The threads, joined when the fabric goes.
    threads: Vec<JoinHandle<()>>,

    // What a pass works in, made as nodes are held and reused on every pass
    // after: so that reading a node and passing on what it produced allocates
    // nothing once the vehicle is running.
    /// What the wave under way delivers.
    wave: Vec<Passing>,
    /// And what it produces, for the next.
    next: Vec<Passing>,
    /// The nodes given work in the stage under way, in the order they were
    /// first given it.
    busy: Vec<usize>,
}

impl Fabric {
    pub fn new(routes: Routes, buffer: usize) -> Fabric {
        Fabric {
            shared: Arc::new(Shared {
                names: RwLock::new(Vec::new()),
                workers: RwLock::new(Vec::new()),
                routes,
                watch: RwLock::new(Arc::new(Unwatched)),
                buffer,
                origin: RwLock::new(vec![0; buffer]),
                remaining: Mutex::new(0),
                finished: Condvar::new(),
            }),
            index: BTreeMap::new(),
            held: 0,
            threads: Vec::new(),
            wave: Vec::new(),
            next: Vec::new(),
            busy: Vec::new(),
        }
    }

    /// Let something say which nodes are enabled, and be told what they move:
    /// see [`Watch`].
    pub fn watched_by(&mut self, watch: Arc<dyn Watch>) {
        *write(&self.shared.watch) = watch;
    }

    /// The index of a name, given one the first time it is seen.
    ///
    /// Allocates that first time, which for a node is when it is first held,
    /// as the vehicle starts. A delivery started from a name never held is a
    /// test's, and is given one the same way.
    fn indexed(&mut self, name: &str) -> usize {
        if let Some(&at) = self.index.get(name) {
            return at;
        }
        let at = {
            let mut names = write(&self.shared.names);
            names.push(name.to_string());
            names.len() - 1
        };
        write(&self.shared.workers).push(None);
        self.index.insert(name.to_string(), at);
        at
    }

    /// Put a node in, under the name the configuration calls it.
    ///
    /// Allocates: the node's place in the table, and the first time a node of
    /// this name is held, its thread, its mailbox and room in what a pass
    /// works in. This is done as the vehicle starts, and again only when the
    /// ground starts a node it had stopped -- never in a pass.
    pub fn hold(&mut self, name: &str, port: Box<dyn Port>) {
        let at = self.indexed(name);
        let existing = read(&self.shared.workers)[at].clone();
        let worker = match existing {
            Some(worker) => worker,
            None => {
                let worker = self.start(name);
                write(&self.shared.workers)[at] = Some(worker.clone());
                // Room for every node in a stage, and for a result from each
                // in a wave.
                let nodes = self.index.len();
                self.busy.reserve(nodes.saturating_sub(self.busy.len()));
                self.wave.reserve(nodes.saturating_sub(self.wave.len()));
                self.next.reserve(nodes.saturating_sub(self.next.len()));
                worker
            }
        };
        if locked(&worker.port).replace(port).is_none() {
            self.held += 1;
        }
    }

    /// Make a node's thread, and what it shares with the conductor.
    ///
    /// A thread that will not start -- the system is out of them -- is said
    /// once and leaves the node to the conductor, rather than refusing a
    /// vehicle whose every node opened.
    fn start(&mut self, name: &str) -> Arc<Worker> {
        let mut worker = Worker {
            name: name.to_string(),
            port: Mutex::new(None),
            banks: [
                RwLock::new(vec![vec![0; self.shared.buffer]]),
                RwLock::new(vec![vec![0; self.shared.buffer]]),
            ],
            mail: Mutex::new(Mail::default()),
            wake: Condvar::new(),
            threaded: true,
        };
        // Room for an arrival on every link into it, which is the most a
        // wave gives a node in a graph where no node produces twice in one.
        let into = self.shared.routes.into_count(name).max(1);
        {
            let mail = worker.mail.get_mut().unwrap_or_else(|e| e.into_inner());
            mail.inbox.reserve(into);
            mail.produced.reserve(into);
        }

        let (shared, handed) = (self.shared.clone(), Arc::new(worker));
        let theirs = handed.clone();
        match std::thread::Builder::new()
            .name(format!("node {name}"))
            .spawn(move || serve(&theirs, &shared))
        {
            Ok(thread) => {
                self.threads.push(thread);
                handed
            }
            Err(e) => {
                log::error!(
                    "\"{name}\": no thread can be started for it ({e}), so its reads \
                     and deliveries are made in turn with the conductor's own"
                );
                worker = Arc::try_unwrap(handed).unwrap_or_else(|_| {
                    unreachable!("the thread that would have held a copy never started")
                });
                worker.threaded = false;
                Arc::new(worker)
            }
        }
    }

    /// This node's thread and port, if it is held.
    fn worker(&self, name: &str) -> Option<Arc<Worker>> {
        let at = *self.index.get(name)?;
        read(&self.shared.workers)[at].clone()
    }

    pub fn holds(&self, name: &str) -> bool {
        self.worker(name)
            .is_some_and(|worker| locked(&worker.port).is_some())
    }

    /// Do `f` with the port this node is, between stages.
    ///
    /// `None` is a node this fabric does not hold, which a checked
    /// configuration has no link to and which a node the ground released is
    /// until it is enabled again.
    fn with_port<R>(&self, name: &str, f: impl FnOnce(&mut dyn Port) -> R) -> Option<R> {
        let worker = self.worker(name)?;
        let mut port = locked(&worker.port);
        port.as_mut().map(|port| f(port.as_mut()))
    }

    /// Take a node out, and with it whatever it had open.
    ///
    /// For a node the ground has disabled: dropping the port is what closes
    /// the device, frees the bus address or gives up the socket, because every
    /// port owns what it opened. `None` is a node that was not held, which a
    /// stop arriving twice gives.
    ///
    /// Nothing else changes. The routing still names the node, its thread
    /// waits for the node to be held again, and a delivery to it is dropped
    /// before it gets that far, by what [`Watch`] says of it.
    pub fn release(&mut self, name: &str) -> Option<Box<dyn Port>> {
        let released = locked(&self.worker(name)?.port).take();
        if released.is_some() {
            self.held -= 1;
        }
        released
    }

    /// How many nodes are held.
    pub fn len(&self) -> usize {
        self.held
    }

    /// Whether any node is held, which a checked configuration always has.
    pub fn is_empty(&self) -> bool {
        self.held == 0
    }

    /// Flush one node, saying what went wrong if anything did.
    ///
    /// `None` is "nothing to report", which is what every node but a log
    /// answers. A name that is not held is reported, as a delivery to one is:
    /// a log that is not there is a log nothing is being written to.
    pub fn flush(&mut self, name: &str) -> Option<String> {
        // A node the ground has stopped holds nothing to flush, and may hold
        // no port at all: nothing has been delivered to it since it stopped.
        if !read(&self.shared.watch).is_enabled(name) {
            return None;
        }
        match self.with_port(name, |node| node.flush()) {
            Some(how) => how.err(),
            None => Some("there is no such node in this fabric".to_string()),
        }
    }

    /// Hand `data` to every place `from`'s output goes, and pass on whatever
    /// that produces.
    ///
    /// Each destination is attempted whatever happened to the ones before it.
    /// A log that cannot be written must not stop the ground getting its
    /// telemetry: they are two destinations of one link, and the link is not
    /// a unit that succeeds or fails as a whole.
    ///
    /// A delivery that produces a result carries on from there, which is what
    /// makes a chain of computations a chain: a reading delivered to a
    /// converter becomes a temperature, which goes to the mean and the ground,
    /// and the average goes to the thermostat, whose decision reaches the
    /// heaters -- all of it from the one delivery of the one reading, and all
    /// of it in the pass that read the sensor. A node is asked whether it has
    /// a result ([`Port::ready`]) rather than simply read, because reading a
    /// device is not free and reading a MOC link would take a command off it
    /// and pass that on as though it were a result.
    ///
    /// Breadth first, a wave at a time, so that the three destinations of a
    /// "T" are all reached before any of their own destinations are. The
    /// ground gets a reading before an on-board computation over it starts,
    /// which is the order an operator watching telemetry would expect, and the
    /// order that loses least if a later step fails. Within a wave every
    /// destination is given its datum at once, on its own thread.
    pub fn deliver(&mut self, from: &str, data: &[u8]) -> Moved {
        self.deliver_on(from, data, None)
    }

    /// And the same for something said again rather than something new.
    ///
    /// Only the devices are told: see [`Taken::again`] and [`Port::holds`].
    pub fn deliver_again(&mut self, from: &str, data: &[u8]) -> Moved {
        self.deliver_as(from, data, None, true)
    }

    /// And the same for a node that keeps its two streams apart.
    ///
    /// `stream` is which of them this datum is on, as the node that produced
    /// it said: see [`Taken`]. Only the links leaving that stream's terminal
    /// are followed, which is the whole point -- an `app_all_veh` reading on
    /// its payload interface and writing on its MOC one would otherwise have
    /// its reading delivered to both, and the ground's commands would carry
    /// telemetry.
    ///
    /// `None` is a node that does not keep them apart, and then every link out
    /// of it is followed, which is what the fabric did before and what every
    /// device and computation still wants.
    pub fn deliver_on(&mut self, from: &str, data: &[u8], stream: Option<Stream>) -> Moved {
        self.deliver_as(from, data, stream, false)
    }

    /// Deliver a datum, saying whether it is new or said again.
    ///
    /// A re-assertion goes to the devices this node drives and to nothing
    /// else. It is not a reading and not a command: it is the vehicle holding
    /// something at what it last decided, so a log, a ground link or a
    /// further computation is told nothing -- the first two would bury the
    /// moment anything switched, and the third would be made to compute again
    /// over a value that has not changed.
    ///
    /// Nothing is counted for a destination passed over. A re-assertion that
    /// does not go to a log is not a datum dropped: it is a datum that was
    /// never for the log, which the counters would otherwise report as loss.
    fn deliver_as(
        &mut self,
        from: &str,
        data: &[u8],
        stream: Option<Stream>,
        again: bool,
    ) -> Moved {
        let at = self.indexed(from);
        {
            let mut origin = write(&self.shared.origin);
            // Larger than a read could be only where a test hands over
            // something no node produced; made once and kept.
            if origin.len() < data.len() {
                origin.resize(data.len(), 0);
            }
            origin[..data.len()].copy_from_slice(data);
        }
        self.wave.clear();
        self.wave.push(Passing {
            from: at,
            bytes: Bytes::Origin,
            len: data.len(),
            depth: 0,
            stream,
            again,
        });
        let mut moved = Moved::default();
        self.cascade(1, &mut moved);
        moved
    }

    /// Deliver the wave in [`Fabric::wave`], and every wave it gives rise to,
    /// until one produces nothing. Each wave reads the bank the one before it
    /// wrote, starting from `bank`'s other.
    ///
    /// How far a result may be passed on is bounded. A path through a graph
    /// with no cycle in it visits each node at most once, so a cascade longer
    /// than the number of nodes has been round a cycle -- and the links are
    /// checked for cycles before any of this runs, which makes this the
    /// backstop for a check that was wrong rather than the rule itself.
    /// Stopped and named rather than left to run: a loop here would be a
    /// vehicle that stops reading its sensors.
    fn cascade(&mut self, mut bank: usize, moved: &mut Moved) {
        let most = self.held.max(1);
        let shared = self.shared.clone();
        let watch = read(&shared.watch).clone();

        while !self.wave.is_empty() {
            self.busy.clear();
            {
                let names = read(&shared.names);
                let workers = read(&shared.workers);
                for passing in &self.wave {
                    // Borrowed, not copied: the routes are only read here.
                    for (at, delivery) in
                        shared.routes.from(&names[passing.from]).iter().enumerate()
                    {
                        // A datum on one stream does not go down the other's
                        // links. Nothing is said about it: a node with two
                        // streams has two sets of links and every datum it
                        // produces belongs to one of them, so the others are
                        // not refusals -- they are the rest of the graph,
                        // going about its business.
                        if !carries(delivery, passing.stream) {
                            continue;
                        }

                        // Dropped and counted, not queued. A queue for a node
                        // nobody has re-enabled is a queue that grows without
                        // bound, and a reading delivered minutes later is a
                        // reading that describes a moment that has passed.
                        if !watch.is_enabled(&delivery.to) {
                            watch.dropped(&delivery.to);
                            moved.dropped += 1;
                            continue;
                        }

                        let Some(to) = self
                            .index
                            .get(delivery.to.as_str())
                            .and_then(|&to| workers[to].as_ref().map(|worker| (to, worker)))
                        else {
                            // A checked configuration has no such link.
                            // Counted rather than ignored, because the
                            // alternative is telemetry going quietly nowhere.
                            moved.refused.push((
                                delivery.to.clone(),
                                "there is no such node in this fabric".to_string(),
                            ));
                            continue;
                        };
                        let mut mail = locked(&to.1.mail);
                        if mail.inbox.is_empty() {
                            self.busy.push(to.0);
                        }
                        mail.inbox.push(Arrival {
                            passing: *passing,
                            delivery: at,
                        });
                    }
                }
                for &to in &self.busy {
                    if let Some(worker) = &workers[to] {
                        locked(&worker.mail).job = Some(Job::Deliver { bank, most });
                    }
                }
            }

            self.stage();

            // What the wave produced is the next wave, node by node in the
            // order they were first given something, and each node's in the
            // order it produced it.
            self.next.clear();
            {
                let workers = read(&shared.workers);
                for &at in &self.busy {
                    let Some(worker) = &workers[at] else {
                        continue;
                    };
                    let mut mail = locked(&worker.mail);
                    mail.inbox.clear();
                    moved.add(std::mem::take(&mut mail.moved));
                    for made in mail.produced.drain(..) {
                        self.next.push(Passing {
                            from: at,
                            bytes: Bytes::Result {
                                node: at,
                                bank,
                                item: made.item,
                            },
                            len: made.taken.bytes,
                            depth: made.depth,
                            stream: made.taken.stream,
                            again: made.taken.again,
                        });
                    }
                }
            }
            std::mem::swap(&mut self.wave, &mut self.next);
            bank ^= 1;
        }
    }

    /// Run the stage whose jobs are set for the nodes in [`Fabric::busy`]:
    /// every one at once, each on its own thread, and wait for the last.
    fn stage(&mut self) {
        let shared = self.shared.clone();
        let workers = read(&shared.workers);
        let threaded = self
            .busy
            .iter()
            .filter(|&&at| workers[at].as_ref().is_some_and(|worker| worker.threaded))
            .count();

        *locked(&shared.remaining) = threaded;
        for &at in &self.busy {
            if let Some(worker) = &workers[at] {
                if worker.threaded {
                    worker.wake.notify_one();
                }
            }
        }
        // A node with no thread of its own is the conductor's to do, while
        // the threads do theirs.
        for &at in &self.busy {
            if let Some(worker) = &workers[at] {
                if !worker.threaded {
                    let mut mail = locked(&worker.mail);
                    if let Some(job) = mail.job.take() {
                        perform(worker, &shared, &mut mail, job);
                    }
                }
            }
        }

        let mut left = locked(&shared.remaining);
        while *left > 0 {
            left = shared
                .finished
                .wait(left)
                .unwrap_or_else(|e| e.into_inner());
        }
    }

    /// Apply a command from the ground.
    ///
    /// Two things have to be true, and the order matters. The configuration
    /// must say the ground may set this attribute of this node -- an
    /// attribute no binding mentions cannot be set, which is what makes what
    /// the ground can change in flight something written down and checkable
    /// rather than whatever a computation happens to accept. And the node must
    /// then accept the value, which is where a setpoint that is not a number
    /// or a band of nought is refused.
    ///
    /// One answer rather than a [`Moved`]: a command is one thing, and the
    /// ground is told whether it took effect.
    pub fn set(&mut self, node: &str, attribute: &str, value: &[u8]) -> Result<(), String> {
        if !self.shared.routes.is_commanded(node, attribute) {
            return Err(format!(
                "no command binding says the ground may set \"{node}.{attribute}\": \
                 what a command may change is in the configuration, so that it is \
                 checkable before it flies"
            ));
        }

        self.with_port(node, |port| port.set(attribute, value))
            .unwrap_or_else(|| Err(format!("there is no node called \"{node}\" in this fabric")))
    }

    /// Read one node and deliver whatever it had.
    ///
    /// Nothing to say is not a failure and not a delivery: a device that is
    /// not due returns no bytes, which is the ordinary case on most passes.
    pub fn pump(&mut self, name: &str) -> Moved {
        self.read_stage([(name, Read::Take)])
    }

    /// Read a node without asking it, and deliver what it had: see
    /// [`Port::take_unasked`].
    pub fn pump_unasked(&mut self, name: &str) -> Moved {
        self.read_stage([(name, Read::Unasked)])
    }

    /// Ask a node, and take nothing: see [`Port::ask`]. What went wrong, if
    /// anything did.
    pub fn ask(&mut self, name: &str) -> Option<String> {
        // A node the ground has stopped is not asked, and that is not a
        // failure to report: it is what stopping it means.
        let watch = read(&self.shared.watch).clone();
        if !watch.is_enabled(name) {
            return None;
        }
        match self.with_port(name, |node| node.ask()) {
            None => Some("there is no such node in this fabric".to_string()),
            // A trigger that went out, which is counted on its own: see
            // `NodeCounts::asks`.
            Some(Ok(())) => {
                watch.asked(name, Outcome::Moved(0));
                None
            }
            Some(Err(why)) => {
                watch.asked(name, Outcome::Failed);
                Some(why)
            }
        }
    }

    /// Read every node of a group at once, and deliver what they had.
    ///
    /// At once, which is what a group is for: three sensors read together are
    /// three readings of one moment, where three reads one after another are
    /// readings of three moments being averaged as though they were one. And
    /// every reading is taken before any is delivered, so that what is
    /// computed from them is computed from the set.
    pub fn pump_group(&mut self, members: &[String]) -> Moved {
        self.read_stage(members.iter().map(|member| (member.as_str(), Read::Take)))
    }

    /// Read every node named, each as it says, all at once, and then deliver
    /// what they had, a wave at a time.
    ///
    /// A node the ground has stopped is not read, and nothing is counted for
    /// it: a read that was not attempted is not a datum that was dropped, and
    /// the pass has nothing to say about a node that is off. A node named
    /// twice is read once.
    pub fn read_stage<'a>(&mut self, reads: impl IntoIterator<Item = (&'a str, Read)>) -> Moved {
        let mut moved = Moved::default();
        let shared = self.shared.clone();
        let watch = read(&shared.watch).clone();

        self.busy.clear();
        {
            let workers = read(&shared.workers);
            for (name, how) in reads {
                if !watch.is_enabled(name) {
                    continue;
                }
                let Some(worker) = self
                    .index
                    .get(name)
                    .and_then(|&at| workers[at].as_ref().map(|worker| (at, worker)))
                else {
                    moved.refused.push((
                        name.to_string(),
                        "there is no such node in this fabric".to_string(),
                    ));
                    continue;
                };
                let mut mail = locked(&worker.1.mail);
                if mail.job.is_none() {
                    mail.job = Some(Job::Read { how, bank: 0 });
                    self.busy.push(worker.0);
                }
            }
        }

        self.stage();

        // Every reading, as one wave: see `cascade`.
        self.wave.clear();
        {
            let workers = read(&shared.workers);
            for &at in &self.busy {
                let Some(worker) = &workers[at] else {
                    continue;
                };
                let mut mail = locked(&worker.mail);
                moved.add(std::mem::take(&mut mail.moved));
                for made in mail.produced.drain(..) {
                    self.wave.push(Passing {
                        from: at,
                        bytes: Bytes::Result {
                            node: at,
                            bank: 0,
                            item: made.item,
                        },
                        len: made.taken.bytes,
                        depth: 0,
                        stream: made.taken.stream,
                        again: made.taken.again,
                    });
                }
            }
        }
        self.cascade(1, &mut moved);
        moved
    }
}

impl Drop for Fabric {
    /// Stop every node's thread and wait for it, so that a fabric that has
    /// gone has left nothing running -- and the ports go with it, every one
    /// closing what it opened.
    fn drop(&mut self) {
        for worker in read(&self.shared.workers).iter().flatten() {
            locked(&worker.mail).stop = true;
            worker.wake.notify_one();
        }
        for thread in self.threads.drain(..) {
            // A thread that panicked has said so; the fabric is going either
            // way.
            let _ = thread.join();
        }
    }
}

/// A node's thread: wait for a job, do it, say so, and wait again, until the
/// fabric goes.
fn serve(worker: &Worker, shared: &Shared) {
    loop {
        let mut mail = locked(&worker.mail);
        while mail.job.is_none() && !mail.stop {
            mail = worker.wake.wait(mail).unwrap_or_else(|e| e.into_inner());
        }
        if mail.stop {
            return;
        }
        if let Some(job) = mail.job.take() {
            perform(worker, shared, &mut mail, job);
        }
        drop(mail);

        let mut left = locked(&shared.remaining);
        *left = left.saturating_sub(1);
        if *left == 0 {
            shared.finished.notify_all();
        }
    }
}

/// Do one job, and say so whatever happens in it.
///
/// A port that panics is a fault in that port, and is reported against its
/// node like any other failure. What it must not do is take the stage with
/// it: a thread that died without saying it had finished would leave the
/// conductor waiting for it for the rest of the flight.
fn perform(worker: &Worker, shared: &Shared, mail: &mut Mail, job: Job) {
    let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match job {
        Job::Read { how, bank } => read_node(worker, shared, mail, how, bank),
        Job::Deliver { bank, most } => deliver_to(worker, shared, mail, bank, most),
    }));
    if done.is_err() {
        mail.produced.clear();
        mail.moved.refused.push((
            worker.name.clone(),
            "the node panicked part way through an operation, which is a fault in \
             it to find in the log; the pass carried on without it"
                .to_string(),
        ));
    }
}

/// Read a node, into item 0 of `bank`.
fn read_node(worker: &Worker, shared: &Shared, mail: &mut Mail, how: Read, bank: usize) {
    let watch = read(&shared.watch).clone();
    let name = worker.name.as_str();
    let mut port = locked(&worker.port);
    let Some(node) = port.as_mut() else {
        mail.moved.refused.push((
            name.to_string(),
            "there is no such node in this fabric".to_string(),
        ));
        return;
    };

    if node.produces() {
        let mut out = write(&worker.banks[bank]);
        let taken = match how {
            Read::Take => node.take(&mut out[0]),
            Read::Unasked | Read::UnaskedThenAsk => node.take_unasked(&mut out[0]),
        };
        match taken {
            // Nought bytes is "nothing to say", which is the ordinary answer
            // from a device that is not due: counted as neither a read nor a
            // failure, or a silent payload would be the busiest thing in the
            // telemetry.
            Ok(taken) if taken.bytes == 0 => {}
            Ok(taken) => {
                watch.took(name, Outcome::Moved(taken.bytes));
                watch.sample_taken(name, &out[0][..taken.bytes]);
                mail.produced.push(Produced {
                    item: 0,
                    taken,
                    depth: 0,
                });
            }
            Err(why) => {
                watch.took(name, Outcome::Failed);
                mail.moved.refused.push((name.to_string(), why));
            }
        }
    }
    // Counted whatever the read came to: a read that turned strangers away
    // and found nothing of the payload's is the one a flood looks like.
    let strangers = node.strangers();
    if strangers > 0 {
        watch.strangers(name, strangers);
    }

    if how == Read::UnaskedThenAsk {
        match node.ask() {
            // A trigger that went out, which is counted on its own: see
            // `NodeCounts::asks`.
            Ok(()) => watch.asked(name, Outcome::Moved(0)),
            Err(why) => {
                watch.asked(name, Outcome::Failed);
                mail.moved.refused.push((name.to_string(), why));
            }
        }
    }
}

/// Give a node everything in its inbox, in the order it arrived, and take a
/// result after each arrival it has one for, into `bank`.
fn deliver_to(worker: &Worker, shared: &Shared, mail: &mut Mail, bank: usize, most: usize) {
    let watch = read(&shared.watch).clone();
    let names = read(&shared.names);
    let workers = read(&shared.workers);
    let name = worker.name.as_str();
    let Mail {
        inbox,
        produced,
        moved,
        ..
    } = mail;

    let mut port = locked(&worker.port);
    let mut out = write(&worker.banks[bank]);

    for arrival in inbox.iter() {
        let passing = arrival.passing;
        let Some(node) = port.as_mut() else {
            moved.refused.push((
                name.to_string(),
                "there is no such node in this fabric".to_string(),
            ));
            continue;
        };

        // Said again, to something that is not held at it: nothing to do and
        // nothing to report. See `Fabric::deliver_as`.
        if passing.again && !node.holds() {
            continue;
        }

        if !node.accepts() {
            moved.refused.push((
                name.to_string(),
                "this node takes nothing: it is read, not written".to_string(),
            ));
            continue;
        }

        let delivery = &shared.routes.from(&names[passing.from])[arrival.delivery];
        let arriving = Arriving {
            input: delivery.input.as_deref(),
            stream: delivery.stream,
        };

        // The bytes, where the wave before put them. Never the bank this wave
        // writes -- see `Worker::banks` -- so never a lock this thread holds.
        let given = match passing.bytes {
            Bytes::Origin => {
                let origin = read(&shared.origin);
                give(node, arriving, &origin[..passing.len], name, &*watch, moved)
            }
            Bytes::Result {
                node: from,
                bank: from_bank,
                item,
            } => match &workers[from] {
                Some(source) => {
                    let results = read(&source.banks[from_bank]);
                    give(
                        node,
                        arriving,
                        &results[item][..passing.len],
                        name,
                        &*watch,
                        moved,
                    )
                }
                None => false,
            },
        };
        if !given || !node.ready() {
            continue;
        }

        if passing.depth + 1 >= most {
            moved.refused.push((
                name.to_string(),
                format!(
                    "a result is still being passed on after {most} nodes, which is \
                     more than there are: the links must have a cycle in them"
                ),
            ));
            continue;
        }

        // Room for the result, made the first time this node produces this
        // many in one wave and kept.
        let item = produced.len();
        if out.len() <= item {
            out.push(vec![0; shared.buffer]);
        }
        match node.take(&mut out[item]) {
            Ok(taken) if taken.bytes == 0 => {}
            Ok(taken) => {
                // A result is something the vehicle took from this node,
                // however it came about: an operator asking what a
                // computation has produced is asking for this.
                watch.took(name, Outcome::Moved(taken.bytes));
                watch.sample_taken(name, &out[item][..taken.bytes]);
                produced.push(Produced {
                    item,
                    taken,
                    depth: passing.depth + 1,
                });
            }
            // A node that said it had a result and then did not give one up.
            // Counted, because the result is gone either way.
            Err(why) => {
                watch.took(name, Outcome::Failed);
                moved.refused.push((name.to_string(), why));
            }
        }
    }
}

/// Hand a node one datum, counting how it went. Whether it took it.
fn give(
    node: &mut Box<dyn Port>,
    arriving: Arriving<'_>,
    bytes: &[u8],
    name: &str,
    watch: &dyn Watch,
    moved: &mut Moved,
) -> bool {
    match node.give(arriving, bytes) {
        Ok(()) => {
            watch.gave(name, Outcome::Moved(bytes.len()));
            watch.sample_given(name, bytes);
            moved.delivered += 1;
            moved.bytes += bytes.len();
            true
        }
        Err(why) => {
            watch.gave(name, Outcome::Failed);
            moved.refused.push((name.to_string(), why));
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::node_state::Abandon;

    use std::path::Path;
    use std::sync::Mutex as TestLock;
    use tcslibgs::graph::Graph;

    /// What a node was given, and what it will say when read.
    #[derive(Default)]
    struct Record {
        /// What arrived: the input it named, the stream it was on, and the
        /// bytes.
        given: Vec<(Option<String>, Option<Stream>, Vec<u8>)>,
        to_say: Vec<Vec<u8>>,
        /// Which stream this node says what it produces is on, for a node
        /// that keeps them apart. `None` is every node that does not.
        says_a_stream: Option<Stream>,
        refuse: Option<String>,
        accepts: bool,
        produces: bool,
        /// Whether this stands in for a device held at what it was told.
        holds: bool,
    }

    /// A node that records what it is handed and says what it was told to.
    struct Recorder(Arc<TestLock<Record>>);

    impl Port for Recorder {
        fn take(&mut self, buffer: &mut [u8]) -> Result<Taken, String> {
            let mut record = self.0.lock().expect("the record");
            if record.to_say.is_empty() {
                return Ok(Taken::NOTHING);
            }
            let datum = record.to_say.remove(0);
            buffer[..datum.len()].copy_from_slice(&datum);
            Ok(match record.says_a_stream {
                Some(stream) => Taken::on(datum.len(), stream),
                None => Taken::of(datum.len()),
            })
        }

        fn give(&mut self, arriving: Arriving<'_>, data: &[u8]) -> Result<(), String> {
            let mut record = self.0.lock().expect("the record");
            if let Some(why) = record.refuse.clone() {
                return Err(why);
            }
            record.given.push((
                arriving.input.map(str::to_string),
                arriving.stream,
                data.to_vec(),
            ));
            Ok(())
        }

        fn accepts(&self) -> bool {
            self.0.lock().expect("the record").accepts
        }

        fn produces(&self) -> bool {
            self.0.lock().expect("the record").produces
        }

        fn holds(&self) -> bool {
            self.0.lock().expect("the record").holds
        }
    }

    fn recorder() -> (Arc<TestLock<Record>>, Box<dyn Port>) {
        let record = Arc::new(TestLock::new(Record {
            accepts: true,
            produces: true,
            ..Record::default()
        }));
        (record.clone(), Box::new(Recorder(record)))
    }

    /// A recorder standing in for a device: a heater, a valve.
    ///
    /// The difference is [`Port::holds`], which is what decides whether a
    /// decision asserted a second time is sent to it.
    fn device() -> (Arc<TestLock<Record>>, Box<dyn Port>) {
        let record = Arc::new(TestLock::new(Record {
            accepts: true,
            produces: true,
            holds: true,
            ..Record::default()
        }));
        (record.clone(), Box::new(Recorder(record)))
    }

    /// A temperature on a link, which is a value and not text.
    ///
    /// What a sensor's conversion produces and what a computation reads: see
    /// [`crate::node::vehicle::compute::as_value`]. A decision is still text,
    /// so the decisions these tests look for are written as `b"1"` and
    /// `b"0"`.
    fn kelvin(number: f64) -> Vec<u8> {
        crate::node::vehicle::compute::as_value(number).expect("a temperature a value carries")
    }

    /// The example's links, and only its links.
    ///
    /// Without its links to the ground: these tests hold a recorder for every
    /// node the file draws and are about what the drawn links do. The links to
    /// the ground are `runtime::route`'s and `runtime::system`'s to test,
    /// where they are worked out and opened.
    fn thermostat() -> Routes {
        Routes::of(&drawn())
    }

    /// The example itself, without its links to the ground: see
    /// [`thermostat`]. With no `tcspecial` section no payload reports on its
    /// own, and with no `ground_base` on the moc node what is drawn into it
    /// is delivered to it, as drawn.
    fn drawn() -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut graph = Graph::load(root.join("tests/manual/thermostat.yaml")).expect("it reads");
        graph.tcspecial = None;
        for node in graph.nodes.iter_mut().filter(|n| n.node_type == "moc") {
            node.attributes.remove("ground_base");
        }
        graph
    }

    /// One datum reaches all three places its link names.
    ///
    /// The "T", moving actual bytes: the ground, the mean and the log each get
    /// the reading, and the one into the mean arrives as `in0`.
    #[test]
    fn a_fan_out_delivers_to_every_destination() {
        let mut fabric = Fabric::new(thermostat(), 64);
        let (moc, moc_port) = recorder();
        let (mean, mean_port) = recorder();
        let (log, log_port) = recorder();
        fabric.hold("moc", moc_port);
        fabric.hold("cabin_temp", mean_port);
        fabric.hold("housekeeping", log_port);

        let moved = fabric.deliver("k_1", &[1, 2, 3, 4]);

        assert!(moved.all_delivered(), "{:?}", moved.refused);
        assert_eq!(moved.delivered, 3);
        assert_eq!(moved.bytes, 12, "four bytes to each of three places");

        let groundward = Some(Stream::Groundward);
        assert_eq!(
            moc.lock().expect("the record").given,
            vec![(None, groundward, vec![1, 2, 3, 4])]
        );
        assert_eq!(
            log.lock().expect("the record").given,
            vec![(None, groundward, vec![1, 2, 3, 4])]
        );
        assert_eq!(
            mean.lock().expect("the record").given,
            vec![(Some("in0".to_string()), groundward, vec![1, 2, 3, 4])],
            "the mean should be told which input this is"
        );
    }

    /// A tee sends each datum on both ways, and a null node takes what it is
    /// sent: each one stream in.
    ///
    /// The ground's command reaches the valve, as it was going, and comes
    /// back to the ground as payload data; the valve's reading reaches the
    /// ground and goes again vehicleward, to a node that drops it.
    #[test]
    fn a_tee_sends_each_datum_both_ways() {
        use crate::runtime::port::{NullPort, TeePort};

        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("tees.yaml");
        std::fs::write(
            &path,
            "moc:
  - name: ground
    type: moc
    moc-out: [commands]
payloads:
  - name: valve
    type: network
    protocol: udp
    address: \"127.0.0.1\"
    port: \"7001\"
    payload-out: [readings]
vehicle:
  - name: commands
    type: tee-moc
    moc-out: [valve]
    payload-out: [ground]
  - name: readings
    type: tee-payload
    payload-out: [ground]
    moc-out: [dropped]
  - name: dropped
    type: null-moc
",
        )
        .expect("written");
        let graph = Graph::load(&path).expect("it reads");
        let mut fabric = Fabric::new(Routes::of(&graph), 64);
        let (ground, ground_port) = recorder();
        let (valve, valve_port) = device();
        fabric.hold("ground", ground_port);
        fabric.hold("valve", valve_port);
        fabric.hold("commands", Box::new(TeePort::of("commands")));
        fabric.hold("readings", Box::new(TeePort::of("readings")));
        fabric.hold("dropped", Box::new(NullPort::of()));

        // A command: to the tee, then from it to the valve and the ground.
        let moved = fabric.deliver("ground", b"open");
        assert!(moved.all_delivered(), "{:?}", moved.refused);
        assert_eq!(moved.delivered, 3, "the tee, the valve and the ground");
        assert_eq!(
            valve.lock().expect("the record").given,
            vec![(None, Some(Stream::Vehicleward), b"open".to_vec())]
        );
        assert_eq!(
            ground.lock().expect("the record").given,
            vec![(None, Some(Stream::Groundward), b"open".to_vec())],
            "the command should come back to the ground as payload data"
        );

        // A reading: to the tee, then the ground and the node that drops it.
        ground.lock().expect("the record").given.clear();
        let moved = fabric.deliver("valve", b"293.15");
        assert!(moved.all_delivered(), "{:?}", moved.refused);
        assert_eq!(moved.delivered, 3, "the tee, the ground and the null node");
        assert_eq!(
            ground.lock().expect("the record").given,
            vec![(None, Some(Stream::Groundward), b"293.15".to_vec())]
        );
        assert_eq!(
            valve.lock().expect("the record").given.len(),
            1,
            "unchanged"
        );
    }

    /// Three sources fill three named inputs of one node.
    #[test]
    fn a_fan_in_arrives_as_named_inputs() {
        let mut fabric = Fabric::new(thermostat(), 64);
        let (mean, mean_port) = recorder();
        let (moc, moc_port) = recorder();
        let (log, log_port) = recorder();
        fabric.hold("cabin_temp", mean_port);
        fabric.hold("moc", moc_port);
        fabric.hold("housekeeping", log_port);

        fabric.deliver("k_1", &[1]);
        fabric.deliver("k_2", &[2]);
        fabric.deliver("k_3", &[3]);

        let given = mean.lock().expect("the record");
        assert_eq!(
            given.given,
            vec![
                (Some("in0".to_string()), Some(Stream::Groundward), vec![1]),
                (Some("in1".to_string()), Some(Stream::Groundward), vec![2]),
                (Some("in2".to_string()), Some(Stream::Groundward), vec![3]),
            ]
        );
        // And the ground and the log got all three as well, which are the
        // other two legs: a fan-in on one destination does not take anything
        // away from the fan-out that feeds it.
        assert_eq!(moc.lock().expect("the record").given.len(), 3);
        assert_eq!(log.lock().expect("the record").given.len(), 3);
    }

    /// The worked example, computed end to end from three raw readings.
    ///
    /// The test this module exists for. Three sensors are read, and from those
    /// three deliveries alone the converters produce temperatures, the ground
    /// and the log get every one of them, the mean produces an average on the
    /// third, the thermostat compares it with its setpoint, and the heaters
    /// are asserted -- in the pass that read the sensors, with nothing else
    /// driving it.
    ///
    /// The computations are built from the shipped configuration rather than
    /// written here, so this is the example that is documented and not an
    /// arrangement that resembles it.
    #[test]
    fn the_worked_example_computes_from_three_readings() {
        use crate::runtime::port::ComputePort;

        let graph = drawn();
        let routes = Routes::of(&graph);

        let mut fabric = Fabric::new(routes.clone(), 64);
        for name in ["k_1", "k_2", "k_3", "cabin_temp", "thermostat"] {
            let node = graph.node(name).expect("a node of the example");
            let inputs = routes.inputs_of(name);
            let port = ComputePort::make(node, &inputs, &Abandon::nothing())
                .unwrap_or_else(|why| panic!("{name}: {why}"));
            fabric.hold(name, Box::new(port));
        }

        let (moc, moc_port) = recorder();
        let (log, log_port) = recorder();
        let (first, first_port) = recorder();
        let (second, second_port) = recorder();
        fabric.hold("moc", moc_port);
        fabric.hold("housekeeping", log_port);
        fabric.hold("heaters_1", first_port);
        fabric.hold("heaters_2", second_port);

        // Three raw readings, each in its own part's format, all of them
        // cold: 289.15, 289.15 and 287.3188 kelvin. The vote over them is
        // 289.15, the middle of the three, which is more than two degrees
        // below the setpoint of 293.15.
        let mut moved = Moved::default();
        moved.add(fabric.deliver("temp_1", &[0x01, 0x00]));
        moved.add(fabric.deliver("temp_2", &[0x01, 0x00]));
        moved.add(fabric.deliver("temp_3", &[0x00, 0x10, 0x00]));
        assert!(moved.all_delivered(), "{:?}", moved.refused);

        // What the ground saw: three temperatures, the one voted out of them,
        // and the decision.
        //
        // A temperature is four bytes of micro-kelvin and a decision is text, so
        // they are read back differently -- which is what the two are: a
        // reading, and the thing a set of discrete lines is driven to.
        let given: Vec<Vec<u8>> = moc
            .lock()
            .expect("the record")
            .given
            .iter()
            .map(|(_, _, datum)| datum.clone())
            .collect();
        let temperatures: Vec<f64> = given
            .iter()
            .take(4)
            .map(|datum| {
                crate::node::vehicle::compute::from_value(datum, "the ground").expect("a value")
            })
            .collect();
        for (shown, wanted) in temperatures.iter().zip([289.15, 289.15, 287.3188, 289.15]) {
            assert!(
                (shown - wanted).abs() < 1e-9,
                // 289.15 and not the mean's 288.5396: the low reading is the
                // outlier, and the median leaves it out.
                "the ground should have had {wanted} and had {shown}"
            );
        }
        assert_eq!(
            given.last().map(|datum| datum.as_slice()),
            Some(&b"1"[..]),
            "the decision reaches the ground as well as the heaters"
        );

        // The log saw the same three and the decision, which is the other
        // leg of each "T".
        assert_eq!(
            log.lock().expect("the record").given.len(),
            4,
            "{:?}",
            log.lock().expect("the record").given
        );

        // And both heaters were asserted, from the one pass: one decision
        // reaching each of them, so that one failing leaves the other
        // warming the mass.
        for (which, heater) in [("heaters_1", &first), ("heaters_2", &second)] {
            let driven: Vec<Vec<u8>> = heater
                .lock()
                .expect("the record")
                .given
                .iter()
                .map(|(_, _, datum)| datum.clone())
                .collect();
            assert_eq!(
                driven,
                vec![b"1".to_vec()],
                "{which}: a cabin below the band should have its heaters on"
            );
        }
    }

    /// A decision that has not changed is not sent again.
    ///
    /// The other half of the cascade: a pass that produces nothing to pass on
    /// is a pass with no deliveries in it, and that is correct rather than a
    /// failure. A relay commanded to the state it is already in every three
    /// seconds would be a relay nobody could tell had stopped answering.
    #[test]
    fn a_pass_that_changes_nothing_delivers_nothing_onward() {
        use crate::runtime::port::ComputePort;

        let graph = drawn();
        let routes = Routes::of(&graph);

        let mut fabric = Fabric::new(routes.clone(), 64);
        for name in ["k_1", "k_2", "k_3", "cabin_temp", "thermostat"] {
            let node = graph.node(name).expect("a node of the example");
            let inputs = routes.inputs_of(name);
            fabric.hold(
                name,
                Box::new(
                    ComputePort::make(node, &inputs, &Abandon::nothing()).expect("the computation"),
                ),
            );
        }
        let (_, moc_port) = recorder();
        let (_, log_port) = recorder();
        let (heaters, heater_port) = recorder();
        let (_, second_port) = recorder();
        fabric.hold("moc", moc_port);
        fabric.hold("housekeeping", log_port);
        fabric.hold("heaters_1", heater_port);
        fabric.hold("heaters_2", second_port);

        // Two passes of the same cold readings. The first decides, the second
        // decides the same thing.
        for _ in 0..2 {
            fabric.deliver("temp_1", &[0x01, 0x00]);
            fabric.deliver("temp_2", &[0x01, 0x00]);
            fabric.deliver("temp_3", &[0x00, 0x10, 0x00]);
        }

        assert_eq!(
            heaters.lock().expect("the record").given.len(),
            1,
            "the heaters should have been commanded once, not once a pass"
        );
    }

    /// A node that will not take what arrives stops that branch and no other.
    ///
    /// A computation refusing a datum is counted and the pass carries on:
    /// the ground still gets the reading the computation could not use, which
    /// is exactly the reading somebody will want to look at.
    #[test]
    fn a_computation_that_refuses_does_not_stop_the_telemetry() {
        use crate::runtime::port::ComputePort;

        let graph = drawn();
        let routes = Routes::of(&graph);

        let mut fabric = Fabric::new(routes.clone(), 64);
        let node = graph.node("k_1").expect("the converter");
        fabric.hold(
            "k_1",
            Box::new(
                ComputePort::make(node, &routes.inputs_of("k_1"), &Abandon::nothing())
                    .expect("it builds"),
            ),
        );
        let (moc, moc_port) = recorder();
        fabric.hold("moc", moc_port);

        // Three bytes where the part's format is two: a bus that returned
        // something else, or a sensor read wrongly.
        let moved = fabric.deliver("temp_1", &[0x01, 0x00, 0x00]);

        assert_eq!(moved.refused.len(), 1, "{:?}", moved.refused);
        assert_eq!(moved.refused[0].0, "k_1");
        assert!(
            moved.refused[0].1.contains("to_kelvin"),
            "the complaint should name the computation: {}",
            moved.refused[0].1
        );
        // Nothing reached the ground along that branch, and nothing was lost
        // quietly: the refusal is in hand.
        assert!(moc.lock().expect("the record").given.is_empty());
    }

    /// A command from the ground changes what the thermostat decides.
    ///
    /// The last link in the worked example's chain. The example's binding says
    /// the ground may set `thermostat.setpoint`, and a cabin that was within
    /// the band becomes a cabin the heaters are wanted in -- without the
    /// readings changing at all, which is what makes this the command's doing
    /// and not the sensors'.
    #[test]
    fn a_commanded_setpoint_changes_the_decision() {
        use crate::runtime::port::ComputePort;

        let graph = drawn();
        let routes = Routes::of(&graph);

        let mut fabric = Fabric::new(routes.clone(), 64);
        let thermostat = graph.node("thermostat").expect("the thermostat");
        fabric.hold(
            "thermostat",
            Box::new(ComputePort::make(thermostat, &[], &Abandon::nothing()).expect("it builds")),
        );
        let (heaters, heater_port) = recorder();
        let (_, second_port) = recorder();
        let (_, log_port) = recorder();
        let (_, moc_port) = recorder();
        fabric.hold("heaters_1", heater_port);
        fabric.hold("heaters_2", second_port);
        fabric.hold("housekeeping", log_port);
        fabric.hold("moc", moc_port);

        // 294 kelvin against a setpoint of 293.15 with two degrees either
        // side: inside the band, so nothing is decided.
        let moved = fabric.deliver("cabin_temp", &kelvin(294.0));
        assert!(moved.all_delivered(), "{:?}", moved.refused);
        assert!(
            heaters.lock().expect("the record").given.is_empty(),
            "a cabin inside the band should not have been acted on"
        );

        // The ground asks for a warmer cabin. The decision is not taken on
        // the strength of the number -- the next reading decides -- so the
        // heaters are still untouched.
        assert_eq!(fabric.set("thermostat", "setpoint", b"300.0"), Ok(()));
        assert!(heaters.lock().expect("the record").given.is_empty());

        // And the same reading now calls for heat.
        let moved = fabric.deliver("cabin_temp", &kelvin(294.0));
        assert!(moved.all_delivered(), "{:?}", moved.refused);
        assert_eq!(
            heaters.lock().expect("the record").given,
            // Vehicleward: what a thermostat sends a heater is a command, and
            // the link it goes down is the one leaving the computation's
            // `moc-out`.
            vec![(None, Some(Stream::Vehicleward), b"1".to_vec())],
            "294 is more than two degrees below 300"
        );
    }

    /// The decision is asserted on every sample, and only to the heaters.
    ///
    /// What holds a heater at what the vehicle last decided. A decision sent
    /// once and then left to stand is a decision one lost datagram undoes for
    /// ever: the vehicle believes the heaters are off, the mass climbs, and
    /// the thermostat decides `Off` again and says nothing because it thinks
    /// it has already said it. So every sample asserts the decision to the
    /// devices.
    ///
    /// And to nothing else. A log that recorded the decision ten times a
    /// second would bury the moment the heaters switched, which is the one
    /// thing a reader of it is looking for, and the ground link would carry
    /// the same repetition for the same loss.
    #[test]
    fn a_decision_is_asserted_every_sample_and_recorded_once() {
        use crate::runtime::port::ComputePort;

        let graph = drawn();
        let routes = Routes::of(&graph);

        let mut fabric = Fabric::new(routes.clone(), 64);
        let thermostat = graph.node("thermostat").expect("the thermostat");
        fabric.hold(
            "thermostat",
            Box::new(ComputePort::make(thermostat, &[], &Abandon::nothing()).expect("it builds")),
        );
        let (heaters, heater_port) = device();
        let (second, second_port) = device();
        let (log, log_port) = recorder();
        let (moc, moc_port) = recorder();
        fabric.hold("heaters_1", heater_port);
        fabric.hold("heaters_2", second_port);
        fabric.hold("housekeeping", log_port);
        fabric.hold("moc", moc_port);

        // Cold: the first decision, which is a change and goes everywhere its
        // links lead.
        let moved = fabric.deliver("cabin_temp", &kelvin(280.0));
        assert!(moved.all_delivered(), "{:?}", moved.refused);
        // The decisions a node was handed, and not the temperatures: the
        // example logs the voted temperature and reports it to the ground as
        // well, so both of those hold a reading per sample whatever the
        // thermostat does.
        let said = |record: &Arc<TestLock<Record>>| -> Vec<Vec<u8>> {
            record
                .lock()
                .expect("the record")
                .given
                .iter()
                .map(|(_, _, data)| data.clone())
                .filter(|data| data == b"1" || data == b"0")
                .collect()
        };
        assert_eq!(said(&heaters), vec![b"1".to_vec()]);
        assert_eq!(said(&log), vec![b"1".to_vec()]);
        assert_eq!(said(&moc), vec![b"1".to_vec()]);

        // Four more samples, still cold. Each asserts the decision to both
        // heaters again -- it is what they are to be held at -- and the
        // record of it stands at the one transition.
        for _ in 0..4 {
            let moved = fabric.deliver("cabin_temp", &kelvin(280.0));
            assert!(moved.all_delivered(), "{:?}", moved.refused);
        }
        assert_eq!(said(&heaters).len(), 5, "{:?}", said(&heaters));
        assert_eq!(said(&second).len(), 5, "both heaters are held, not one");
        assert!(
            said(&heaters).iter().all(|datum| datum == b"1"),
            "every assertion is the same decision: {:?}",
            said(&heaters)
        );
        assert_eq!(
            said(&log),
            vec![b"1".to_vec()],
            "the log should hold the transition and not the repetitions"
        );
        assert_eq!(said(&moc), vec![b"1".to_vec()], "and so should the ground");

        // A reading inside the band decides nothing new, and the heaters are
        // still held at what they were told: that is what the band means.
        let moved = fabric.deliver("cabin_temp", &kelvin(294.0));
        assert!(moved.all_delivered(), "{:?}", moved.refused);
        assert_eq!(said(&heaters).len(), 6);
        assert_eq!(said(&log).len(), 1);

        // Warm enough to switch them off: a change, so it is recorded with
        // the first one and the heaters are driven to it.
        let moved = fabric.deliver("cabin_temp", &kelvin(300.0));
        assert!(moved.all_delivered(), "{:?}", moved.refused);
        assert_eq!(said(&heaters).last(), Some(&b"0".to_vec()));
        assert_eq!(
            said(&log),
            vec![b"1".to_vec(), b"0".to_vec()],
            "a reader of the log sees when the heaters actually switched"
        );
    }

    /// Only what the configuration says may be set can be set.
    #[test]
    fn an_attribute_no_binding_names_cannot_be_set() {
        use crate::runtime::port::ComputePort;

        let graph = drawn();
        let routes = Routes::of(&graph);

        let mut fabric = Fabric::new(routes, 64);
        let thermostat = graph.node("thermostat").expect("the thermostat");
        fabric.hold(
            "thermostat",
            Box::new(ComputePort::make(thermostat, &[], &Abandon::nothing()).expect("it builds")),
        );

        // The band either side of the setpoint is in the file and no binding
        // names it, so the ground cannot change it however willing the
        // computation is.
        let said = fabric
            .set("thermostat", "above", b"5.0")
            .expect_err("an attribute no binding names");
        assert!(
            said.contains("thermostat.above") && said.contains("checkable"),
            "the refusal should name what was asked and why not: {said}"
        );

        // A value that is not a number is refused by the computation, which
        // is the other half of what a command has to get past.
        let said = fabric
            .set("thermostat", "setpoint", b"warm")
            .expect_err("a setpoint that is not a number");
        assert!(
            said.contains("thermostat") && said.contains("warm"),
            "the refusal should name the node and the value: {said}"
        );
    }

    /// A device's configuration is not changed from the ground.
    #[test]
    fn a_device_refuses_a_setting() {
        let (_, mut heaters) = recorder();
        let said = heaters
            .set("lines", b"6,7,8")
            .expect_err("a chip re-described in flight");
        assert!(
            said.contains("lines"),
            "the refusal should name what was asked: {said}"
        );
    }

    /// A cycle in the links is stopped and named, not run.
    ///
    /// The links are checked for cycles before any of this runs, so reaching
    /// this means the checking was wrong -- which is exactly when a backstop
    /// earns its place. What it must not do is loop: a fabric going round a
    /// cycle is a vehicle that has stopped reading its sensors, and no
    /// telemetry would say why.
    #[test]
    fn a_cycle_is_stopped_and_named() {
        use tcslibgs::graph::{Destinations, Link, Node};

        /// A node that always has a result, which is what makes the cycle
        /// turn.
        struct Echo;

        impl Port for Echo {
            fn take(&mut self, buffer: &mut [u8]) -> Result<Taken, String> {
                buffer[0] = 1;
                Ok(Taken::of(1))
            }

            fn give(&mut self, _arriving: Arriving<'_>, _data: &[u8]) -> Result<(), String> {
                Ok(())
            }

            fn ready(&self) -> bool {
                true
            }
        }

        let node = |name: &str| Node {
            name: name.to_string(),
            node_type: "compute".to_string(),
            group: None,
            attributes: Default::default(),
        };
        let link = |from: &str, to: &str| Link {
            from: from.to_string(),
            to: Destinations::of(vec![to.to_string()]),
            comment: None,
        };

        let graph = Graph {
            description: None,
            // What tcspecial is configured with is nothing to do with the
            // routing, the ports or the schedule: these tests build a graph
            // by hand and the checking that requires a section is not on
            // their path.
            tcspecial: None,
            groups: Vec::new(),
            nodes: vec![node("a"), node("b")],
            links: vec![link("a", "b"), link("b", "a")],
            commands: Vec::new(),
            files: Vec::new(),
            homes: Default::default(),
            placed: Default::default(),
            set_aside: Default::default(),
        };

        let mut fabric = Fabric::new(Routes::of(&graph), 64);
        fabric.hold("a", Box::new(Echo));
        fabric.hold("b", Box::new(Echo));

        let moved = fabric.deliver("a", &[1]);

        assert_eq!(moved.refused.len(), 1, "{:?}", moved.refused);
        assert!(
            moved.refused[0].1.contains("cycle"),
            "the complaint should say what it is: {}",
            moved.refused[0].1
        );
    }

    /// One destination refusing does not stop the others.
    ///
    /// The reliability choice this module exists to make: a log that cannot be
    /// written must not stop the ground getting its telemetry.
    #[test]
    fn a_refused_delivery_does_not_stop_the_rest() {
        let mut fabric = Fabric::new(thermostat(), 64);
        let (moc, moc_port) = recorder();
        let (mean, mean_port) = recorder();
        let (log, log_port) = recorder();
        log.lock().expect("the record").refuse = Some("the device is full".to_string());
        fabric.hold("moc", moc_port);
        fabric.hold("cabin_temp", mean_port);
        fabric.hold("housekeeping", log_port);

        let moved = fabric.deliver("k_1", &[9]);

        assert_eq!(moved.delivered, 2, "the other two should still have got it");
        assert!(!moved.all_delivered());
        assert_eq!(
            moved.refused,
            vec![("housekeeping".to_string(), "the device is full".to_string())],
            "the refusal should name the node and say why"
        );
        assert_eq!(moc.lock().expect("the record").given.len(), 1);
        assert_eq!(mean.lock().expect("the record").given.len(), 1);
    }

    /// A node that is not there is counted, not ignored.
    #[test]
    fn a_missing_node_is_reported_rather_than_dropped() {
        let mut fabric = Fabric::new(thermostat(), 64);
        let (moc, moc_port) = recorder();
        fabric.hold("moc", moc_port);
        // cabin_temp and housekeeping are not held.

        let moved = fabric.deliver("k_1", &[7]);

        assert_eq!(moved.delivered, 1);
        assert_eq!(moved.refused.len(), 2, "{:?}", moved.refused);
        assert!(moved
            .refused
            .iter()
            .all(|(_, why)| why.contains("no such node")));
        assert_eq!(moc.lock().expect("the record").given.len(), 1);
    }

    /// A node that is only written is not read, and is not an error.
    #[test]
    fn a_driven_node_is_not_read() {
        let mut fabric = Fabric::new(thermostat(), 64);
        let (heaters, heaters_port) = recorder();
        heaters.lock().expect("the record").produces = false;
        fabric.hold("heaters", heaters_port);

        let moved = fabric.pump("heaters");
        assert_eq!(moved, Moved::default(), "{moved:?}");
    }

    /// A node with nothing to say is not a delivery and not a failure.
    #[test]
    fn nothing_to_say_is_not_a_failure() {
        let mut fabric = Fabric::new(thermostat(), 64);
        let (_sensor, sensor_port) = recorder();
        let (_k, k_port) = recorder();
        fabric.hold("temp_1", sensor_port);
        fabric.hold("k_1", k_port);

        let moved = fabric.pump("temp_1");
        assert_eq!(moved.delivered, 0);
        assert!(moved.all_delivered(), "{:?}", moved.refused);
    }

    /// Reading a node delivers what it had.
    #[test]
    fn what_a_node_produces_goes_where_its_link_says() {
        let mut fabric = Fabric::new(thermostat(), 64);
        let (sensor, sensor_port) = recorder();
        let (k, k_port) = recorder();
        sensor.lock().expect("the record").to_say = vec![vec![0x01, 0x90]];
        fabric.hold("temp_1", sensor_port);
        fabric.hold("k_1", k_port);

        let moved = fabric.pump("temp_1");

        assert_eq!(moved.delivered, 1);
        assert_eq!(moved.bytes, 2);
        assert_eq!(
            k.lock().expect("the record").given,
            vec![(None, Some(Stream::Groundward), vec![0x01, 0x90])]
        );
    }

    /// A group's members are read in order, one pass each.
    #[test]
    fn a_group_is_read_in_order() {
        let mut fabric = Fabric::new(thermostat(), 64);

        let mut sensors = Vec::new();
        for (sensor, converter) in [("temp_1", "k_1"), ("temp_2", "k_2"), ("temp_3", "k_3")] {
            let (record, port) = recorder();
            record.lock().expect("the record").to_say = vec![vec![sensors.len() as u8]];
            fabric.hold(sensor, port);
            let (got, got_port) = recorder();
            fabric.hold(converter, got_port);
            sensors.push(got);
        }

        let members: Vec<String> = ["temp_1", "temp_2", "temp_3"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let moved = fabric.pump_group(&members);

        assert_eq!(moved.delivered, 3, "{:?}", moved.refused);
        for (n, got) in sensors.iter().enumerate() {
            assert_eq!(
                got.lock().expect("the record").given,
                vec![(None, Some(Stream::Groundward), vec![n as u8])],
                "converter {n} got the wrong reading"
            );
        }
    }

    /// A graph of computations joined as `links` says, built by hand: what
    /// the tests of a stage want is the shape, and nothing else.
    fn shaped(nodes: &[&str], links: &[(&str, &str)]) -> Routes {
        use tcslibgs::graph::{Destinations, Link, Node};
        Routes::of(&Graph {
            description: None,
            tcspecial: None,
            groups: Vec::new(),
            nodes: nodes
                .iter()
                .map(|name| Node {
                    name: name.to_string(),
                    node_type: "compute".to_string(),
                    group: None,
                    attributes: Default::default(),
                })
                .collect(),
            links: links
                .iter()
                .map(|(from, to)| Link {
                    from: from.to_string(),
                    to: Destinations::of(vec![to.to_string()]),
                    comment: None,
                })
                .collect(),
            commands: Vec::new(),
            files: Vec::new(),
            homes: Default::default(),
            placed: Default::default(),
            set_aside: Default::default(),
        })
    }

    /// What happened, in the order it happened, across every thread.
    type Events = Arc<TestLock<Vec<String>>>;

    /// A node that takes `slow` over a read and over a delivery, saying when
    /// each happened.
    struct Slow {
        name: &'static str,
        slow: std::time::Duration,
        events: Events,
    }

    impl Port for Slow {
        fn take(&mut self, buffer: &mut [u8]) -> Result<Taken, String> {
            std::thread::sleep(self.slow);
            self.events
                .lock()
                .unwrap()
                .push(format!("read {}", self.name));
            buffer[0] = 7;
            Ok(Taken::of(1))
        }

        fn give(&mut self, _arriving: Arriving<'_>, _data: &[u8]) -> Result<(), String> {
            self.events
                .lock()
                .unwrap()
                .push(format!("given {}", self.name));
            std::thread::sleep(self.slow);
            Ok(())
        }
    }

    fn slow(name: &'static str, events: &Events) -> Box<dyn Port> {
        Box::new(Slow {
            name,
            slow: std::time::Duration::from_millis(200),
            events: events.clone(),
        })
    }

    /// A group's reads are made at once, each on its node's own thread: three
    /// reads of 200 ms take about 200 ms, not 600.
    ///
    /// And every reading is taken before any is delivered, which is the
    /// order a group exists for: a computation over the three is a
    /// computation over one moment.
    #[test]
    fn a_groups_reads_are_made_at_once_and_all_before_any_is_delivered() {
        let events: Events = Default::default();
        let mut fabric = Fabric::new(
            shaped(
                &["s1", "s2", "s3", "sink"],
                &[("s1", "sink"), ("s2", "sink"), ("s3", "sink")],
            ),
            64,
        );
        fabric.hold("s1", slow("s1", &events));
        fabric.hold("s2", slow("s2", &events));
        fabric.hold("s3", slow("s3", &events));
        // Quick to give to, so that what is timed is the reads.
        fabric.hold(
            "sink",
            Box::new(Slow {
                name: "sink",
                slow: std::time::Duration::ZERO,
                events: events.clone(),
            }),
        );

        let started = std::time::Instant::now();
        let moved = fabric.pump_group(&["s1".to_string(), "s2".to_string(), "s3".to_string()]);
        let took = started.elapsed();

        assert!(moved.all_delivered(), "{:?}", moved.refused);
        assert_eq!(moved.delivered, 3, "every reading arrives");
        assert!(
            took < std::time::Duration::from_millis(450),
            "three 200 ms reads took {took:?}: they were made one after another"
        );
        let happened: Vec<String> = events.lock().unwrap().clone();
        assert_eq!(happened.len(), 6, "{happened:?}");
        assert!(
            happened[..3].iter().all(|event| event.starts_with("read"))
                && happened[3..].iter().all(|event| event == "given sink"),
            "every reading should be taken before any is delivered: {happened:?}"
        );
    }

    /// The destinations of a wave are given their data at once: one reading
    /// to three slow destinations takes about one destination's time.
    #[test]
    fn a_waves_deliveries_are_made_at_once() {
        let events: Events = Default::default();
        let mut fabric = Fabric::new(
            shaped(
                &["source", "d1", "d2", "d3"],
                &[("source", "d1"), ("source", "d2"), ("source", "d3")],
            ),
            64,
        );
        fabric.hold("d1", slow("d1", &events));
        fabric.hold("d2", slow("d2", &events));
        fabric.hold("d3", slow("d3", &events));

        let started = std::time::Instant::now();
        let moved = fabric.deliver("source", &[1]);
        let took = started.elapsed();

        assert_eq!(moved.delivered, 3, "{moved:?}");
        assert!(
            took < std::time::Duration::from_millis(450),
            "three 200 ms deliveries took {took:?}: they were made one after another"
        );
    }

    /// A port that panics is a failure of its node, reported against it, and
    /// not a stage that never ends: the other destinations get their data,
    /// and the next pass runs.
    #[test]
    fn a_port_that_panics_is_reported_and_the_pass_goes_on() {
        struct Panics;

        impl Port for Panics {
            fn take(&mut self, _buffer: &mut [u8]) -> Result<Taken, String> {
                Ok(Taken::NOTHING)
            }

            fn give(&mut self, _arriving: Arriving<'_>, _data: &[u8]) -> Result<(), String> {
                panic!("a fault in this port");
            }
        }

        let mut fabric = Fabric::new(
            shaped(
                &["source", "bad", "good"],
                &[("source", "bad"), ("source", "good")],
            ),
            64,
        );
        fabric.hold("bad", Box::new(Panics));
        let (good, port) = recorder();
        fabric.hold("good", port);

        for pass in 1..=2 {
            let moved = fabric.deliver("source", &[pass]);
            assert_eq!(moved.refused.len(), 1, "{:?}", moved.refused);
            assert_eq!(moved.refused[0].0, "bad");
            assert!(
                moved.refused[0].1.contains("panicked"),
                "{:?}",
                moved.refused
            );
            assert_eq!(good.lock().unwrap().given.len(), pass as usize);
        }
    }
}
