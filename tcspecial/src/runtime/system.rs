//! A configuration, running.
//!
//! The piece that was missing between a checked graph and a vehicle doing
//! something: it opens a port for every node, holds them in a fabric, and
//! lets the schedule decide when each group is read. Everything it does is
//! done by the four pieces beside it -- this is the assembly and the loop, and
//! deliberately little else.
//!
//! # A pass
//!
//! One pass is: every group that is due, read in full and in order; then the
//! nodes that are in no group, polled; then the logs flushed. A group is read
//! in full before anything else happens because that is what a group is for,
//! and each reading is delivered the moment it is taken -- so a converted
//! reading reaches the ground, the mean and the log before the next sensor is
//! read. The mean waits for its third input, which arrives in the same pass,
//! and the decision reaches the heaters in it too.
//!
//! # Why everything must open
//!
//! A vehicle starts only if every node opens, and the complaint lists every
//! reason rather than the first. The configuration was checked on the ground,
//! so a failure here is the hardware: a chip whose lines another program
//! holds, a bus that is not there. Carrying on without one node sounds kinder
//! and is not -- a thermostat whose third sensor did not open would hold its
//! last decision for ever, because a mean over three inputs waits for three,
//! and nothing in the telemetry would say that is why. An operator who wants
//! to fly without a sensor changes the configuration, which is checkable,
//! visible in the diagram and recorded in the beacon's digest.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tcslibgs::graph::telemetry::Telemetered;
use tcslibgs::graph::Graph;

use std::sync::Arc;

use crate::endpoint::bind_oc_pair;
use crate::node::vehicle::log::LogNode;
use crate::runtime::fabric::{Fabric, Moved, Port, Read};
use crate::runtime::node_state::{Abandon, NodeStates, Nudge};
use crate::runtime::port::{
    AppPort, ComputePort, EndpointPort, LogPort, NullPort, PingPort, TeePort, Use,
};
use crate::runtime::route::Routes;
use crate::runtime::schedule::Schedule;
use crate::segsend::SegSend;

/// How large a datum may be: see [`tcslibgs::DATUM_BYTES`], which is where the
/// configuration is held to it.
pub use tcslibgs::DATUM_BYTES;

/// How long a pass may wait when a node is polled rather than scheduled.
///
/// A node in no group is read as it becomes readable, which means asking. The
/// pause bounds how stale such a reading can be; it does not apply to a
/// configuration whose nodes are all in groups, which waits for the next group
/// instead and wakes exactly when it is due.
pub const POLL_PAUSE: Duration = Duration::from_millis(20);

/// What the command interpreter asks the pass to do.
///
/// Opening a device takes time and can fail, so it is not done on the thread
/// that reads commands: that thread hands the work over and answers when the
/// work is done, so a START_NODE opening a bus does not stop the next command
/// being read.
///
/// Disabling is here too, although the *stopping* of a node is not: that is a
/// flag and a poke, done where the command arrives, because a node the ground
/// has disabled must stop at once and not after the current read finishes.
/// What is left for the pass is releasing what the node held, which must not
/// happen while an operation is using it.
///
/// Nothing in it is made for it: a node is named by the name its state holds,
/// shared rather than copied, and an attribute and a value are in room of a
/// fixed size, as the SET_NODE carried them. So handing work over allocates
/// nothing; what the work itself takes is the pass's to say.
///
/// As large as a setting, for the reason `tcslibgs::Command` is: a setting
/// boxed would be an allocation for each one handed over.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Work {
    /// Open this node again and let the pass touch it.
    Enable(Arc<str>),
    /// Release what it held, it having already been told to stop.
    Disable(Arc<str>),
    /// Set one of its attributes, as the configuration binds it.
    ///
    /// The pass's work and not the command interpreter's, because what it
    /// changes is a node's configuration: a node whose setpoint moved in the
    /// middle of a computation would be a node doing half of one thing and
    /// half of another.
    Set {
        node: Arc<str>,
        attribute: tcslibgs::Attribute,
        value: tcslibgs::Value,
    },
}

/// One piece of work, and the ticket its answer is matched to.
pub struct Job {
    pub work: Work,
    ticket: u64,
}

/// How a piece of work went, and which job it was.
type Answer = (u64, Result<(), String>);

/// How many jobs, and how many answers, the channels between the command
/// interpreter and the pass have room for. One is in flight at a time -- see
/// [`Workbench::ask`] -- and the rest is room for the answers to jobs whose
/// asker gave up waiting.
const JOBS_AT_ONCE: usize = 4;

/// The pass's end of the workbench: the jobs handed to it, and where it
/// answers them.
pub struct Desk {
    jobs: Receiver<Job>,
    answers: SyncSender<Answer>,
}

impl Desk {
    /// The next job, if there is one, without waiting.
    pub fn take(&self) -> Option<Job> {
        self.jobs.try_recv().ok()
    }

    /// The next job, waiting for it; `None` once nothing can hand one over.
    pub fn wait(&self) -> Option<Job> {
        self.jobs.recv().ok()
    }

    /// Say how a job went.
    ///
    /// Never waits: an answer with no room is one to a job nobody is waiting
    /// for any longer, which is not a reason to hold up the pass, and the
    /// work was done all the same.
    pub fn answer(&self, job: &Job, how: Result<(), String>) {
        let _ = self.answers.try_send((job.ticket, how));
    }
}

/// How the command interpreter hands work to the pass and waits for it.
///
/// Cheap to clone and safe to hold: everything in it is a channel or a
/// descriptor, and nothing the pass owns is reachable through it. The channels
/// are made once, with room for [`JOBS_AT_ONCE`], so handing a job over and
/// hearing how it went allocate nothing.
#[derive(Clone)]
pub struct Workbench {
    jobs: SyncSender<Job>,
    /// Where the pass's answers arrive. Behind a lock, which is also what
    /// keeps one job in flight at a time: a second asker waits for the first
    /// to be answered.
    answers: Arc<Mutex<Receiver<Answer>>>,
    /// The next job's ticket.
    tickets: Arc<AtomicU64>,
    nudge: Nudge,
}

/// How long to wait for the pass to do a piece of work.
///
/// Generous, because the work may be opening a device: what this is really for
/// is a pass that has stopped making passes at all, where waiting for ever
/// would take the command interpreter down with it. A ground station is better
/// told that the vehicle did not answer than left with a command in flight.
const WORK_DEADLINE: Duration = Duration::from_secs(10);

impl Workbench {
    /// A workbench and the jobs handed to it, for a caller that will do the
    /// work itself.
    ///
    /// What [`System`] makes for itself, made on its own: a test of the
    /// command interpreter wants to see which work a command caused, and a
    /// program that drives the pass some other way wants the same handover
    /// without this module's loop.
    pub fn in_two() -> std::io::Result<(Workbench, Desk)> {
        let (jobs, taken) = std::sync::mpsc::sync_channel(JOBS_AT_ONCE);
        let (answering, answers) = std::sync::mpsc::sync_channel(JOBS_AT_ONCE);
        Ok((
            Workbench {
                jobs,
                answers: Arc::new(Mutex::new(answers)),
                tickets: Arc::new(AtomicU64::new(0)),
                nudge: Nudge::new()?,
            },
            Desk {
                jobs: taken,
                answers: answering,
            },
        ))
    }

    /// Ask the pass to do this, and wait until it has.
    ///
    /// The nudge is poked after the job is queued, so a pass waiting for the
    /// next group to come due takes the work now rather than when the group
    /// is read.
    ///
    /// Allocates nothing to hand the work over or to hear how it went; what
    /// the work takes is the pass's -- a START opens a device again, which
    /// allocates whatever the device needs to be open: see `System::reopen`.
    ///
    /// An answer to an earlier job, whose asker gave up waiting, is passed
    /// over: each answer carries its job's ticket, so a late one cannot be
    /// taken for this one's.
    pub fn ask(&self, work: Work) -> Result<(), String> {
        let answers = self
            .answers
            .lock()
            .map_err(|_| "the command interpreter's answers are lost".to_string())?;
        let ticket = self.tickets.fetch_add(1, Ordering::Relaxed);
        self.jobs
            .send(Job { work, ticket })
            .map_err(|_| "the vehicle is no longer making passes".to_string())?;
        self.nudge.poke();
        let deadline = Instant::now() + WORK_DEADLINE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match answers.recv_timeout(left) {
                Ok((answered, how)) if answered == ticket => return how,
                Ok(_) => continue,
                Err(_) => {
                    return Err(format!(
                        "the vehicle did not do it within {WORK_DEADLINE:?}"
                    ))
                }
            }
        }
    }
}

/// Everything a configuration needs in order to run.
/// How a node is opened: the same call that opened everything at startup.
///
/// A type of its own because it is written down twice -- the field that holds
/// one and the parameter that supplies it -- and a closure's type spelled
/// twice is two places to keep in step.
type Opener =
    Box<dyn FnMut(&tcslibgs::graph::Node, &Routes, &Abandon) -> Result<Box<dyn Port>, String>>;

pub struct System {
    /// The nodes, and the table saying how they are joined.
    fabric: Fabric,
    /// When each group is next read.
    schedule: Schedule,
    /// The nodes that are in no group and produce something, which are polled.
    polled: Vec<String>,
    /// Those of them that state how often, each with how its rate applies and
    /// when it is next due: see [`Pace`]. A polled node not here is read on
    /// every pass.
    paced: BTreeMap<String, (Pace, Instant)>,
    /// The logs, which are flushed at the end of a pass.
    logs: Vec<String>,
    /// What each node's state is, shared with whatever reads commands.
    ///
    /// The fabric holds the same `Arc` as its gate, so the pass asks the one
    /// table the command interpreter stores into: see
    /// [`crate::runtime::node_state`].
    states: Arc<NodeStates>,
    /// The configuration's nodes, by name, so that one can be opened again.
    ///
    /// Kept rather than looked up from the graph, which the caller owns and
    /// need not keep: a node the ground re-enables has to be opened from what
    /// the file said about it, and the file said it once.
    nodes: BTreeMap<String, tcslibgs::graph::Node>,
    /// The routing, which opening a node needs as much as the node does.
    routes: Routes,
    /// What a node becomes, kept so that a node can become it again.
    ///
    /// The same function that opened everything at startup, because enabling a
    /// node is the same work as starting it was and must be able to fail the
    /// same ways.
    opener: Opener,
    /// Work the command interpreter has handed over, taken between stages.
    desk: Desk,
    /// How the interpreter asks for it, and wakes a pass that is waiting.
    bench: Workbench,
}

impl System {
    /// Open everything this configuration describes.
    ///
    /// The graph is expected checked -- `Graph::check` first, or `tcsverify`
    /// on the ground -- so what fails here is the hardware rather than the
    /// file. Every failure is reported, not the first: an operator holding a
    /// list of what is wrong can fix a vehicle in one pass of the hangar,
    /// where one error at a time is one trip each.
    ///
    /// Every log is opened with `segsend`, which is what its filled segments
    /// are handed to.
    pub fn of(graph: &Graph, now: Instant, segsend: SegSend) -> Result<System, Vec<String>> {
        System::of_opening(
            graph,
            now,
            move |node, routes, abandon| open(node, routes, abandon, &segsend),
            open_ground_link,
        )
    }

    /// Open everything, with `opener` deciding what a node becomes and
    /// `ground` what a payload's link to the ground becomes.
    ///
    /// For the tests, which have no bus, no chip and no ground station: the
    /// assembly and the loop are what is worth testing here, and testing them
    /// against real hardware would be testing the hardware.
    pub fn of_opening(
        graph: &Graph,
        now: Instant,
        mut opener: impl FnMut(&tcslibgs::graph::Node, &Routes, &Abandon) -> Result<Box<dyn Port>, String>
            + 'static,
        mut ground: impl FnMut(&Telemetered) -> Result<Box<dyn Port>, String>,
    ) -> Result<System, Vec<String>> {
        let routes = Routes::of(graph);
        // Before anything opens, because a node's state is what decides
        // whether the pass touches it and every node has one from the start.
        let states = Arc::new(
            NodeStates::of(graph).map_err(|e| vec![format!("a node's wakeup descriptor: {e}")])?,
        );
        let mut fabric = Fabric::new(routes.clone(), DATUM_BYTES);
        fabric.watched_by(states.clone());
        let mut refused = Vec::new();
        let mut logs = Vec::new();
        let mut polled = Vec::new();
        let mut paced = BTreeMap::new();

        for node in &graph.nodes {
            // The node's own wakeup goes into the port, so that a wait inside
            // one of its operations ends when the ground disables it.
            let abandon = states
                .named(&node.name)
                .map_or_else(Abandon::nothing, |state| state.abandon());
            match opener(node, &routes, &abandon) {
                Ok(port) => {
                    // What is polled and what is flushed, worked out while
                    // the node is in hand: a pass that asked these questions
                    // of every node every time would ask them thousands of
                    // times an hour for an answer that cannot change.
                    if is_polled(node, &routes, port.as_ref()) {
                        polled.push(node.name.clone());
                        if let Some(pace) = pace_of(node) {
                            // Due at once, as a group is: a vehicle that has
                            // just started reads everything before settling
                            // into its intervals.
                            paced.insert(node.name.clone(), (pace, now));
                        }
                    }
                    if node.node_type == "log" {
                        logs.push(node.name.clone());
                    }
                    fabric.hold(&node.name, port);
                }
                Err(why) => refused.push(why),
            }
        }

        // Each payload's link to the ground, which the table delivers to
        // under a name of its own. Opened with everything else, and refused
        // with everything else: a payload whose telemetry cannot be sent is a
        // vehicle reporting nothing about it.
        for link in routes.ground_links() {
            match ground(link) {
                Ok(port) => fabric.hold(&link.name, port),
                Err(why) => refused.push(why),
            }
        }

        if !refused.is_empty() {
            return Err(refused);
        }

        let (bench, desk) =
            Workbench::in_two().map_err(|e| vec![format!("the loop's own wakeup: {e}")])?;

        Ok(System {
            fabric,
            schedule: Schedule::of(graph, now),
            polled,
            paced,
            logs,
            states,
            nodes: graph
                .nodes
                .iter()
                .map(|node| (node.name.clone(), node.clone()))
                .collect(),
            routes,
            opener: Box::new(opener),
            desk,
            bench,
        })
    }

    /// One pass: what is due, then what is polled, then the logs.
    ///
    /// Every group that is due is read in full before the next is started,
    /// its members all at once, each on its node's own thread, which is what a
    /// group is for. Its readings are all taken before any is delivered, and
    /// then delivered a wave at a time, so everything downstream of a group --
    /// a conversion, a mean, a decision, a command to a heater -- happens
    /// inside the pass that read it. See [`Fabric::read_stage`].
    pub fn pass(&mut self, now: Instant) -> Moved {
        let mut moved = Moved::default();

        // Before the first stage, and between each pair of them: see
        // `take_the_work`.
        self.take_the_work();

        // By the group's place, not its name, and each group's members
        // borrowed from the schedule while the fabric reads them: the two are
        // different fields, so nothing is copied to read a group.
        while let Some(group) = self.schedule.most_overdue(now) {
            moved.add(self.fabric.pump_group(self.schedule.members_at(group)));
            self.schedule.read_at(group, now);
            self.take_the_work();
        }

        // The nodes in no group, which are read as they become readable: a
        // read of no bytes is the ordinary answer and costs a system call.
        // One that states how often is read, or asked, as its `Pace` says.
        // All of them at once, as one stage, and what they read delivered
        // after: see `Fabric::read_stage`. Borrowed, as the groups' members
        // are: the fabric and the paces are other fields.
        let paced = &mut self.paced;
        let reads = self.polled.iter().filter_map(|node| {
            let Some((pace, due)) = paced.get(node).copied() else {
                return Some((node.as_str(), Read::Take));
            };
            let is_due = now >= due;
            if is_due {
                // In place: the entry is there, and replacing it would make
                // its name again.
                if let Some(entry) = paced.get_mut(node) {
                    *entry = (pace, pace.next(due, now));
                }
            }
            match pace {
                // Read, and asked if it is asked, only when due.
                Pace::Exchange(_) | Pace::Floor(_) => is_due.then_some((node.as_str(), Read::Take)),
                // Read on every pass, taking what has arrived; asked only
                // when due.
                Pace::Ask(_) if is_due => Some((node.as_str(), Read::UnaskedThenAsk)),
                Pace::Ask(_) => Some((node.as_str(), Read::Unasked)),
            }
        });
        moved.add(self.fabric.read_stage(reads));

        self.take_the_work();

        // The logs, once, at the end. A log flushed per record is a write
        // system call per reading; a log flushed per pass is every record of
        // a pass on the far side of one call.
        for log in &self.logs {
            if let Some(why) = self.fabric.flush(log) {
                moved.refused.push((log.clone(), why));
            }
        }

        moved
    }

    /// How long until the next pass is worth making.
    ///
    /// The next group's due time, the next paced node's, or the polling pause
    /// where a node is polled on every pass -- whichever is sooner. A
    /// configuration whose nodes are all in groups, or all state their
    /// intervals, waits exactly until the next one is due and wakes for
    /// nothing else.
    pub fn until_next_pass(&self, now: Instant) -> Duration {
        let groups = self.schedule.until_next(now);
        let paced = self
            .paced
            .values()
            .map(|(_, due)| due.saturating_duration_since(now))
            .min();
        let due = match (groups, paced) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        // Only a node read on every pass needs the pause.
        let every_pass = self.polled.iter().any(|node| {
            !matches!(
                self.paced.get(node),
                Some((Pace::Exchange(_) | Pace::Floor(_), _))
            )
        });
        match (due, every_pass) {
            (Some(due), true) => due.min(POLL_PAUSE),
            (Some(due), false) => due,
            // Nothing scheduled at all: a configuration of ungrouped nodes
            // that state no interval.
            (None, true) => POLL_PAUSE,
            // Nothing scheduled and nothing polled, which a checked
            // configuration does not have -- every node is on a link, and a
            // node that produces is read somehow. Waited out rather than spun
            // on.
            (None, false) => POLL_PAUSE,
        }
    }

    /// Run until `stop` says otherwise, telling `report` about each pass.
    ///
    /// The whole of the loop. Passes are made when something is due and the
    /// time between them is spent waiting, which is also what makes a stop
    /// take effect within one pause rather than at the next group.
    ///
    /// Nothing is reported for a pass that moved nothing and refused nothing,
    /// which is most of them.
    pub fn run(
        &mut self,
        stop: &dyn Fn() -> bool,
        mut report: impl FnMut(&Moved),
        pause: impl Fn(Duration),
    ) {
        while !stop() {
            let now = Instant::now();
            let moved = self.pass(now);
            if moved.delivered > 0 || !moved.all_delivered() {
                report(&moved);
            }
            pause(self.until_next_pass(Instant::now()));
        }
    }

    /// What this system holds, for a line at startup saying what is running.
    pub fn holds(&self) -> usize {
        self.fabric.len()
    }

    /// The nodes that are polled rather than scheduled.
    pub fn polled(&self) -> &[String] {
        &self.polled
    }

    /// The logs this system flushes.
    pub fn logs(&self) -> &[String] {
        &self.logs
    }

    /// How the command interpreter hands this system work.
    ///
    /// Handed out rather than reached through, as the node states are: the
    /// interpreter runs on a thread of its own, and a channel and a descriptor
    /// are the whole of what it shares with the pass.
    pub fn workbench(&self) -> Workbench {
        self.bench.clone()
    }

    /// What the loop waits on between passes, so work can cut the wait short.
    pub fn nudge(&self) -> Nudge {
        self.bench.nudge.clone()
    }

    /// Do whatever the command interpreter has handed over.
    ///
    /// Between stages and never inside one, which is the rule: a node whose
    /// configuration changed in the middle of an operation would be a node
    /// doing half of one thing and half of another. Every job queued when this
    /// is called is done before it returns, so a burst of commands costs one
    /// gap between stages rather than one each.
    fn take_the_work(&mut self) {
        while let Some(job) = self.desk.take() {
            let how = match &job.work {
                Work::Enable(node) => self.reopen(node),
                Work::Disable(node) => self.release(node),
                Work::Set {
                    node,
                    attribute,
                    value,
                } => self.set(node, attribute, value),
            };
            // A reply nobody is waiting for is a caller that gave up, which is
            // not a reason to have left the work undone.
            self.desk.answer(&job, how);
        }
    }

    /// Open a node again and let the pass touch it.
    ///
    /// The same work as starting it at startup, by the same function, so it
    /// can fail the same ways: a device that is gone, an address already
    /// taken, a program that will not execute. A node that will not open is
    /// left disabled and the reason is given back, rather than being left
    /// enabled over a port that is not there.
    ///
    /// A node that is already open is success and nothing is done. An
    /// operator who presses Enable twice has asked for a state.
    ///
    /// Allocates: opening a node again is opening it, as at startup -- its
    /// port, the device's handles and buffers -- and the node's description
    /// is copied to open it from. Done for a START_NODE the ground sends,
    /// and never by a pass of its own accord.
    pub fn reopen(&mut self, name: &str) -> Result<(), String> {
        let Some(node) = self.nodes.get(name).cloned() else {
            return Err(format!("there is no node called \"{name}\" here"));
        };
        if self.fabric.holds(name) {
            // Held already: the flag is the only thing that may be down, which
            // is the case of a node disabled before this version released
            // anything.
            self.enable(name);
            return Ok(());
        }

        let abandon = self
            .states
            .named(name)
            .map_or_else(Abandon::nothing, |state| state.abandon());
        let port = (self.opener)(&node, &self.routes, &abandon)?;
        self.fabric.hold(name, port);
        // Enabled only once it is open, and in that order: a node marked
        // enabled over a port that is not there would be read by the next
        // pass and refused by it.
        self.enable(name);
        Ok(())
    }

    /// Release what a node held, the ground having stopped it.
    ///
    /// The endpoint is closed and the device, bus address or socket is free. A
    /// node held open but idle looks from outside like a node that is running,
    /// and the port it keeps is a port nothing else may take.
    ///
    /// The node is expected to be disabled already -- stopping it is a flag
    /// and a poke where the command arrives, so that it stops at once -- and
    /// this is what the pass does about it between stages. Releasing a node
    /// that holds nothing is success: a stop that arrives twice is the
    /// ordinary case on a link with latency.
    pub fn release(&mut self, name: &str) -> Result<(), String> {
        if !self.nodes.contains_key(name) {
            return Err(format!("there is no node called \"{name}\" here"));
        }
        // Dropped, which is what closes it: every port owns what it opened.
        drop(self.fabric.release(name));
        Ok(())
    }

    /// Let the pass touch this node again.
    fn enable(&self, name: &str) {
        if let Some(state) = self.states.named(name) {
            state.enable();
        }
    }

    /// What each node's state is, for whatever reads commands.
    ///
    /// Handed out rather than reached through: the command interpreter runs on
    /// a thread of its own and this is the whole of what it shares with the
    /// pass, which is what keeps a command from waiting on a pass and a pass
    /// from waiting on a command.
    pub fn states(&self) -> Arc<NodeStates> {
        self.states.clone()
    }

    /// Apply a command from the ground.
    ///
    /// Passed through so that whatever reads commands does not have to know
    /// how the nodes are held: see [`Fabric::set`].
    pub fn set(&mut self, node: &str, attribute: &str, value: &[u8]) -> Result<(), String> {
        self.fabric.set(node, attribute, value)
    }
}

/// What is running, in a line: how many nodes, which are polled, which are
/// logs.
///
/// Written by hand because the ports are not things to show -- a socket, an
/// open chip, a log's file position -- and what a line about a running system
/// wants is the shape of it.
impl std::fmt::Debug for System {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} nodes, {} polled, {} groups, logs {:?}",
            self.fabric.len(),
            self.polled.len(),
            self.schedule.groups().len(),
            self.logs
        )
    }
}

/// How a node in no group that states how often is read.
///
/// Three, because what the timer drives depends on the interface -- see
/// `tcslibgs::graph::interface` -- and on whether the node is asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pace {
    /// A `trigger_interval` on a part that originates nothing: the whole
    /// exchange -- ask, wait the `read_delay`, read -- every interval, counted
    /// from when it was due so that it does not drift.
    Exchange(Duration),
    /// A `trigger_interval` on a part that sends: the trigger alone every
    /// interval, counted from when it was due. Its answers arrive when they
    /// arrive and are taken on the pass they do, so the node is read on every
    /// pass without being asked.
    Ask(Duration),
    /// A `poll_interval`: a floor and not a rate, the least time between one
    /// read and the next. Counted from the read, since nothing says how soon
    /// there will be anything to read and a read that came late has still
    /// just looked.
    Floor(Duration),
}

impl Pace {
    /// When a node read or asked at `now`, having been due at `due`, is next
    /// due.
    ///
    /// A rate from when it was due, so it does not drift -- and one so far
    /// behind that the next time is already past is put an interval from now
    /// rather than left due for ever. A floor from now.
    fn next(self, due: Instant, now: Instant) -> Instant {
        match self {
            Pace::Exchange(interval) | Pace::Ask(interval) => {
                let then = due + interval;
                if then <= now {
                    now + interval
                } else {
                    then
                }
            }
            Pace::Floor(interval) => now + interval,
        }
    }
}

/// How a polled node is paced, where it says how often: see [`Pace`]. `None`
/// is a node read on every pass.
///
/// The graph is checked before this -- `Graph::check` refuses both intervals
/// on one node, either in a group, and a value that is not a length of time
/// -- so a value that will not read here is a node read on every pass rather
/// than a vehicle that stops.
fn pace_of(node: &tcslibgs::graph::Node) -> Option<Pace> {
    let read = |name: &str| {
        node.attributes
            .get(name)
            .and_then(|said| tcslibgs::graph::parse_interval(said.as_str().trim()).ok())
    };
    if let Some(interval) = read("trigger_interval") {
        return Some(if tcslibgs::graph::interface::sends(&node.node_type) {
            Pace::Ask(interval)
        } else {
            Pace::Exchange(interval)
        });
    }
    read("poll_interval").map(Pace::Floor)
}

/// Whether this node is read by being asked on every pass.
///
/// Three things have to be true, and each of them rules out a node that would
/// otherwise be read for nothing.
///
/// *It is in no group.* A node in a group is read when its group is due, which
/// is the whole point of putting it in one.
///
/// *Its output goes somewhere.* A node read when nothing takes what it
/// produces is a read whose result is discarded, and the one that matters here
/// is a MOC link: what arrives on one is a command, which sets an attribute
/// rather than travelling a link, so polling it would be taking commands off
/// the socket and throwing them away. A ground link that *does* feed the graph
/// says so with a link out of it, and is then polled like anything else.
///
/// *It produces by the clock and not by arrival.* A computation produces
/// because something was delivered to it, and the fabric passes that on in the
/// same pass -- see [`Port::ready`]. Polling one would be asking a question
/// whose answer has already been taken.
fn is_polled(node: &tcslibgs::graph::Node, routes: &Routes, port: &dyn Port) -> bool {
    node.group.is_none()
        && port.produces()
        && routes.fan_out(&node.name) > 0
        // A computation, a tee and a `ping` node produce because something
        // arrived, which `Port::ready` has already said and the pass has
        // already taken: polling one would be asking a question whose answer
        // is gone. (A null node produces nothing, and says so.)
        && !matches!(
            node.node_type.as_str(),
            "compute" | "log" | "tee-moc" | "tee-payload" | "ping"
        )
}

/// One source's link to the ground: bound at tcspecial's end and sending to
/// the ground's, as `tcslibgs::graph::telemetry` counts them.
///
/// Written to and never read. The ground's end is written down, so this sends
/// there from the first reading rather than waiting to be spoken to.
pub fn open_ground_link(link: &Telemetered) -> Result<Box<dyn Port>, String> {
    let what = link.name.as_str();
    let (reader, writer) = bind_oc_pair(what, &link.vehicle, Some(&link.ground)).map_err(|e| {
        format!(
            "{what}: cannot send from {}:{} to the ground at {}:{}: {e}",
            link.vehicle.address, link.vehicle.port, link.ground.address, link.ground.port
        )
    })?;
    Ok(Box::new(EndpointPort::of(
        what,
        reader,
        writer,
        Use::Driven,
    )))
}

/// What a node becomes: a port over whatever it is.
///
/// The three kinds of node there are, which is the one place that mapping is
/// written. A type this does not know is a type `Graph::check` should have
/// refused, and it is refused again here rather than skipped: a node nothing
/// holds is a link that delivers nowhere.
pub fn open(
    node: &tcslibgs::graph::Node,
    routes: &Routes,
    abandon: &Abandon,
    segsend: &SegSend,
) -> Result<Box<dyn Port>, String> {
    match node.node_type.as_str() {
        "compute" => {
            let inputs = routes.inputs_of(&node.name);
            Ok(Box::new(ComputePort::make(node, &inputs, abandon)?))
        }
        "log" => Ok(Box::new(LogPort::of(LogNode::of(node, segsend)?))),
        // A node that drops the one stream it takes, and one that sends it on
        // both ways. Nothing is opened for either: neither is an interface.
        "null-moc" | "null-payload" => Ok(Box::new(NullPort::of())),
        "tee-moc" | "tee-payload" => Ok(Box::new(TeePort::of(&node.name))),
        // A node that answers the ground's pings. Nothing is opened for it
        // either: what it hears and where its answers go are its links.
        "ping" => Ok(Box::new(PingPort::of(&node.name))),
        // A ground link is where the ground's commands and lines arrive.
        // What goes to the ground does not pass through it: each source has
        // a link of its own to the node's ground_base, opened beside it --
        // see `open_ground_link`. So one that binds nothing, or that nothing
        // is read from, is held as a node that is never asked anything.
        "moc" if !tcslibgs::graph::reach::binds(node) || Use::of(routes, &node.name).is_err() => {
            Ok(Box::new(NullPort::of()))
        }
        // A node whose work is a program, started here and talked to over the
        // file interfaces `tcsapp` describes. Three of the five kinds are the
        // vehicle's own and two are payloads -- a payload that is a program
        // rather than a device -- and one port holds any of them.
        other if tcslibgs::node::app::Kind::of(other).is_some() => {
            Ok(Box::new(AppPort::open(node)?))
        }
        // Everything else is reached over an interface, and what a node says
        // about reaching it is tcslibgs::graph::reach's business.
        _ => Ok(Box::new(EndpointPort::open(node, routes, abandon)?)),
    }
}

/// What each node of a configuration would be opened as, without opening it.
///
/// For a line at startup, and for a ground tool that wants to say what a file
/// describes: the type and which way the links use it -- `i2c, read` -- or the
/// type alone where it says everything, as a computation's and a log's do.
/// Nothing is opened, so this answers for a vehicle that is not there.
pub fn described(graph: &Graph) -> BTreeMap<String, String> {
    let routes = Routes::of(graph);
    graph
        .nodes
        .iter()
        .map(|node| {
            let how = match node.node_type.as_str() {
                "compute" | "log" => String::new(),
                "null-moc" | "null-payload" => "drops what arrives".to_string(),
                "tee-moc" | "tee-payload" => "sends what arrives both ways".to_string(),
                "ping" => "answers pings".to_string(),
                // An app node is a program of its own, and what it does with
                // what arrives is in that program: nothing here can say more
                // than which of the two streams it is run with, which its type
                // already says. See `crate::node::vehicle::app_payload_veh`
                // and its four neighbours.
                other if tcslibgs::node::app::Kind::of(other).is_some() => "a program".to_string(),
                // A ground link, where nothing the vehicle sends passes
                // through the node itself: each source has a link of its own
                // to the node's `ground_base`. So one that nothing is read
                // from and nothing arrives at is doing its work all the same,
                // and saying "no link touches it" of one would be reporting a
                // fault that is not there. See `open`, which holds such a
                // node as a `NullPort` for the same reason.
                "moc" => match Use::of(&routes, &node.name) {
                    Ok(Use::Read) => "read".to_string(),
                    Ok(Use::Driven) => "driven".to_string(),
                    Ok(Use::Both) => "read and written".to_string(),
                    Err(_) => "to the ground".to_string(),
                },
                _ => match Use::of(&routes, &node.name) {
                    Ok(Use::Read) => "read".to_string(),
                    Ok(Use::Driven) => "driven".to_string(),
                    Ok(Use::Both) => "read and written".to_string(),
                    Err(why) => why,
                },
            };
            let said = if how.is_empty() {
                node.node_type.clone()
            } else {
                format!("{}, {how}", node.node_type)
            };
            (node.name.clone(), said)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // What a port answers with, for the stand-ins below.
    use crate::runtime::fabric::{Arriving, Taken};
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex as TestLock;

    /// What a stand-in port records, shared with the test that reads it.
    ///
    /// A lock and not a `RefCell`, and an `Arc` and not an `Rc`, because a
    /// port is `Send`: a node operation runs on a thread of its own, so
    /// anything a stand-in holds has to be able to go with it.
    type Kept = Arc<TestLock<Vec<Vec<u8>>>>;

    /// And a count it keeps, for the same reason.
    type Tally = Arc<AtomicUsize>;

    fn kept() -> Kept {
        Arc::new(TestLock::new(Vec::new()))
    }

    fn tally() -> Tally {
        Arc::new(AtomicUsize::new(0))
    }

    use tcslibgs::graph::Node;
    use tcslibgs::Scalar;

    /// A device that answers with whatever it was given, in order.
    ///
    /// Standing in for the three sensors, because the example's sensors are
    /// on buses that are not on this machine. What is being tested is the
    /// assembly and the loop: with real hardware here these would be tests of
    /// a bus.
    struct Sensor {
        readings: Kept,
    }

    impl Port for Sensor {
        fn take(&mut self, buffer: &mut [u8]) -> Result<Taken, String> {
            let mut readings = self.readings.lock().expect("the record");
            if readings.is_empty() {
                return Ok(Taken::NOTHING);
            }
            let datum = readings.remove(0);
            buffer[..datum.len()].copy_from_slice(&datum);
            Ok(Taken::of(datum.len()))
        }

        fn give(&mut self, _arriving: Arriving<'_>, _data: &[u8]) -> Result<(), String> {
            Err("a sensor is read and not written".to_string())
        }

        fn accepts(&self) -> bool {
            false
        }
    }

    /// Somewhere the ground link's telemetry can be looked at.
    #[derive(Default)]
    struct Ground {
        got: Kept,
    }

    impl Port for Ground {
        fn take(&mut self, _buffer: &mut [u8]) -> Result<Taken, String> {
            Ok(Taken::NOTHING)
        }

        fn give(&mut self, _arriving: Arriving<'_>, data: &[u8]) -> Result<(), String> {
            self.got.lock().expect("the record").push(data.to_vec());
            Ok(())
        }
    }

    /// A node that counts how often it is read and how often it is asked, and
    /// has nothing to say.
    struct Counted {
        reads: Tally,
        asks: Tally,
    }

    impl Port for Counted {
        fn take(&mut self, _buffer: &mut [u8]) -> Result<Taken, String> {
            self.reads.fetch_add(1, Ordering::Relaxed);
            Ok(Taken::NOTHING)
        }

        fn ask(&mut self) -> Result<(), String> {
            self.asks.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn give(&mut self, _arriving: Arriving<'_>, _data: &[u8]) -> Result<(), String> {
            Ok(())
        }
    }

    /// A device that answers with whatever it was given and counts the asks.
    ///
    /// [`Sensor`] with the asking counted, for the one test where stopping a
    /// node has to stop an ask as well as a read.
    struct Asked {
        readings: Kept,
        asks: Tally,
    }

    impl Port for Asked {
        fn take(&mut self, buffer: &mut [u8]) -> Result<Taken, String> {
            let mut readings = self.readings.lock().expect("the record");
            if readings.is_empty() {
                return Ok(Taken::NOTHING);
            }
            let datum = readings.remove(0);
            buffer[..datum.len()].copy_from_slice(&datum);
            Ok(Taken::of(datum.len()))
        }

        fn ask(&mut self) -> Result<(), String> {
            self.asks.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn give(&mut self, _arriving: Arriving<'_>, _data: &[u8]) -> Result<(), String> {
            Err("a part that is asked is read and not written".to_string())
        }

        fn accepts(&self) -> bool {
            false
        }
    }

    /// Reads and asks, by node.
    type Counts = BTreeMap<String, (Tally, Tally)>;

    /// Payloads in no group, each sending to the ground and each paced every
    /// 100 ms in one of the three ways -- `exchanged`, an I2C part that is
    /// asked; `asked`, a socket that is asked; `floored`, a socket with a poll
    /// interval -- and, where `every_pass`, `always`, which states no interval.
    fn paced(now: Instant, every_pass: bool) -> (System, Counts) {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("paced.yaml");
        let mut text = String::from(
            "moc:
  - { name: ground, type: moc, ground_base: \"127.0.0.1:5100\" }
payloads:
  - { name: exchanged, type: i2c, path: \"/dev/i2c-1\", address: \"0x48\",
      trigger: \"55\", trigger_interval: \"100 ms\", payload-out: ground }
  - { name: asked, type: network, protocol: udp, address: \"127.0.0.1\", port: \"7002\",
      trigger: \"55\", trigger_interval: \"100 ms\", payload-out: ground }
  - { name: floored, type: network, protocol: udp, address: \"127.0.0.1\", port: \"7001\",
      poll_interval: \"100 ms\", payload-out: ground }
",
        );
        if every_pass {
            text.push_str(
                "  - { name: always, type: network, protocol: udp, address: \"127.0.0.1\", \
                 port: \"7003\", payload-out: ground }\n",
            );
        }
        std::fs::write(&path, text).expect("written");
        let graph = Graph::load(&path).expect("it reads");

        let counts: Counts = graph
            .nodes
            .iter()
            .map(|node| (node.name.clone(), (tally(), tally())))
            .collect();
        // Cloned into the opener, which the system keeps so that a node can
        // be opened again: the counters are reference counted, so the test
        // and the opener watch the same ones.
        let opening = counts.clone();
        let system = System::of_opening(
            &graph,
            now,
            move |node: &Node, _: &Routes, _: &Abandon| {
                let (reads, asks) = &opening[&node.name];
                Ok(Box::new(Counted {
                    reads: Arc::clone(reads),
                    asks: Arc::clone(asks),
                }) as Box<dyn Port>)
            },
            |_: &Telemetered| Ok(Box::new(Ground::default()) as Box<dyn Port>),
        )
        .expect("it opens");
        (system, counts)
    }

    /// A part that originates nothing and is asked has the whole exchange
    /// every `trigger_interval`, counted from when it was due.
    #[test]
    fn a_quiet_part_is_asked_and_read_every_trigger_interval() {
        let start = Instant::now();
        let (mut system, counts) = paced(start, false);
        let reads = || counts["exchanged"].0.load(Ordering::Relaxed);

        system.pass(start);
        assert_eq!(reads(), 1, "everything is due as the vehicle starts");
        system.pass(start + Duration::from_millis(50));
        assert_eq!(reads(), 1);
        // Late, and the next time still counted from when it was due.
        system.pass(start + Duration::from_millis(130));
        assert_eq!(reads(), 2);
        system.pass(start + Duration::from_millis(199));
        assert_eq!(reads(), 2);
        system.pass(start + Duration::from_millis(200));
        assert_eq!(reads(), 3);
        // So far behind that the next time has passed: once, and an interval
        // on, rather than every pass to catch up.
        let late = start + Duration::from_secs(1);
        system.pass(late);
        system.pass(late + Duration::from_millis(10));
        assert_eq!(reads(), 4);
        system.pass(late + Duration::from_millis(100));
        assert_eq!(reads(), 5);
    }

    /// A part that sends and is asked has its trigger alone every
    /// `trigger_interval`, and is read on every pass, its answers being
    /// taken as they arrive.
    #[test]
    fn a_sending_part_is_asked_on_its_timer_and_read_on_every_pass() {
        let start = Instant::now();
        let (mut system, counts) = paced(start, false);
        let (reads, asks) = (&counts["asked"].0, &counts["asked"].1);

        system.pass(start);
        assert_eq!(
            (reads.load(Ordering::Relaxed), asks.load(Ordering::Relaxed)),
            (1, 1)
        );
        system.pass(start + Duration::from_millis(20));
        system.pass(start + Duration::from_millis(40));
        assert_eq!(
            (reads.load(Ordering::Relaxed), asks.load(Ordering::Relaxed)),
            (3, 1),
            "read every pass, asked once"
        );
        system.pass(start + Duration::from_millis(100));
        assert_eq!(
            (reads.load(Ordering::Relaxed), asks.load(Ordering::Relaxed)),
            (4, 2)
        );
    }

    /// A `poll_interval` is a floor: the least time between one read and the
    /// next, counted from the read.
    #[test]
    fn a_poll_interval_is_the_least_time_between_reads() {
        let start = Instant::now();
        let (mut system, counts) = paced(start, false);
        let reads = || counts["floored"].0.load(Ordering::Relaxed);

        system.pass(start);
        system.pass(start + Duration::from_millis(99));
        assert_eq!(reads(), 1);
        // Read late, at 130: the next is a whole interval after that, where a
        // rate would have made it 200.
        system.pass(start + Duration::from_millis(130));
        assert_eq!(reads(), 2);
        system.pass(start + Duration::from_millis(200));
        assert_eq!(reads(), 2);
        system.pass(start + Duration::from_millis(230));
        assert_eq!(reads(), 3);
        // Never asked: it has no trigger.
        assert_eq!(counts["floored"].1.load(Ordering::Relaxed), 0);
    }

    /// A node that states no interval is read on every pass.
    #[test]
    fn a_node_with_no_interval_is_read_on_every_pass() {
        let start = Instant::now();
        let (mut system, counts) = paced(start, true);
        for n in 0..5 {
            system.pass(start + Duration::from_millis(n * 20));
        }
        assert_eq!(counts["always"].0.load(Ordering::Relaxed), 5);
    }

    /// The loop waits until the next node is due, and wakes no sooner unless
    /// something is read on every pass -- which a part that sends and is
    /// asked on a timer is.
    #[test]
    fn the_loop_waits_for_the_next_node_that_is_due() {
        let start = Instant::now();

        // A configuration of only a quiet part on a timer and a floored one:
        // the wait is until the next is due.
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("quiet.yaml");
        std::fs::write(
            &path,
            "moc:
  - { name: ground, type: moc, ground_base: \"127.0.0.1:5100\" }
payloads:
  - { name: exchanged, type: i2c, path: \"/dev/i2c-1\", address: \"0x48\",
      trigger: \"55\", trigger_interval: \"100 ms\", payload-out: ground }
  - { name: floored, type: network, protocol: udp, address: \"127.0.0.1\", port: \"7001\",
      poll_interval: \"300 ms\", payload-out: ground }
",
        )
        .expect("written");
        let graph = Graph::load(&path).expect("it reads");
        let mut system = System::of_opening(
            &graph,
            start,
            |_: &Node, _: &Routes, _: &Abandon| {
                Ok(Box::new(Counted {
                    reads: tally(),
                    asks: tally(),
                }) as Box<dyn Port>)
            },
            |_: &Telemetered| Ok(Box::new(Ground::default()) as Box<dyn Port>),
        )
        .expect("it opens");
        assert_eq!(system.until_next_pass(start), Duration::ZERO);
        system.pass(start);
        assert_eq!(
            system.until_next_pass(start + Duration::from_millis(30)),
            Duration::from_millis(70)
        );

        // With a part asked on a timer that is read on every pass, no longer
        // than the pause.
        let (mut system, _) = paced(start, false);
        system.pass(start);
        assert_eq!(
            system.until_next_pass(start + Duration::from_millis(30)),
            POLL_PAUSE
        );
    }

    /// A node the ground has stopped is not read, not asked, and not
    /// delivered to.
    ///
    /// Through a whole pass rather than through the fabric alone, because what
    /// is being tested is that the state the command interpreter stores into
    /// is the state the pass reads: the fabric asks its gate, the gate is the
    /// node states, and `System` is where the two are joined.
    ///
    /// Two nodes, so that both ends of a link can be stopped in turn -- the
    /// one that produces and the one that is delivered to -- which are the two
    /// things stopping a node has to mean.
    #[test]
    fn a_node_the_ground_has_stopped_moves_nothing() {
        let start = Instant::now();

        // A part that sends and is triggered on a timer, which is the pace
        // whose ask is a step of its own: stopping such a node has to stop the
        // trigger as well as the read, and the two are separate calls.
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("stopped.yaml");
        std::fs::write(
            &path,
            "payloads:
  - { name: asked, type: network, protocol: udp, address: \"127.0.0.1\", port: \"7005\",
      trigger: \"55\", trigger_interval: \"100 ms\", payload-out: average }
vehicle:
  - { name: average, type: compute, function: mean, inputs: 1 }
",
        )
        .expect("written");
        let graph = Graph::load(&path).expect("it reads");

        let readings = kept();
        let arrived: Kept = kept();
        let asks = tally();

        let scripted = readings.clone();
        let watched = arrived.clone();
        let counted = asks.clone();
        let mut system = System::of_opening(
            &graph,
            start,
            move |node: &Node, _: &Routes, _: &Abandon| match node.node_type.as_str() {
                "network" => Ok(Box::new(Asked {
                    readings: scripted.clone(),
                    asks: counted.clone(),
                }) as Box<dyn Port>),
                _ => Ok(Box::new(Ground {
                    got: watched.clone(),
                }) as Box<dyn Port>),
            },
            |_: &Telemetered| Ok(Box::new(Ground::default()) as Box<dyn Port>),
        )
        .expect("it opens");

        // Running: the part is asked, the reading is taken, and it reaches the
        // node the link goes to.
        readings.lock().expect("the record").push(b"warm".to_vec());
        let moved = system.pass(start);
        assert_eq!(moved.delivered, 1, "{moved:?}");
        assert_eq!(moved.dropped, 0);
        assert_eq!(asks.load(Ordering::Relaxed), 1, "it was asked");
        assert_eq!(arrived.lock().expect("the record").len(), 1);

        let states = system.states();

        // Stopped, as a STOP_NODE stops it: not asked, not read, and the
        // reading waiting in it stays there.
        states.named("asked").expect("the node").disable();
        assert_eq!(states.disabled(), vec!["asked"]);
        readings.lock().expect("the record").push(b"cold".to_vec());
        let moved = system.pass(start + Duration::from_millis(100));
        assert_eq!(moved.delivered, 0, "{moved:?}");
        assert!(moved.all_delivered(), "and nothing is reported: {moved:?}");
        assert_eq!(
            asks.load(Ordering::Relaxed),
            1,
            "a stopped node is not asked either"
        );
        assert_eq!(
            readings.lock().expect("the record").len(),
            1,
            "the reading was not taken"
        );
        assert_eq!(arrived.lock().expect("the record").len(), 1);

        // And with the other end stopped instead, the reading is taken and the
        // delivery is dropped and counted -- not queued. A queue for a node
        // nobody has re-enabled is a queue that grows without bound, and a
        // reading delivered minutes later is a reading that describes a moment
        // that has passed.
        states.named("asked").expect("the node").enable();
        states.named("average").expect("the computation").disable();
        let moved = system.pass(start + Duration::from_millis(200));
        assert_eq!(moved.dropped, 1, "the delivery was dropped: {moved:?}");
        assert_eq!(moved.delivered, 0, "{moved:?}");
        assert!(
            moved.all_delivered(),
            "dropped is not refused: a node the ground stopped is the \
             configuration working as asked and not a fault: {moved:?}"
        );
        assert_eq!(
            arrived.lock().expect("the record").len(),
            1,
            "nothing new arrived"
        );

        // Started again, it moves what it moves.
        states.named("average").expect("the computation").enable();
        assert!(states.disabled().is_empty());
        readings.lock().expect("the record").push(b"warm".to_vec());
        let moved = system.pass(start + Duration::from_millis(300));
        assert_eq!(moved.delivered, 1, "{moved:?}");
        assert_eq!(moved.dropped, 0);
        assert_eq!(arrived.lock().expect("the record").len(), 2);
    }

    /// Every node counts what it has moved, and the counts survive a stop.
    ///
    /// What a node has done since the vehicle started, which is a different
    /// claim from what a pass did: a query answered from a pass's figures
    /// would answer about the last twenty milliseconds. So the counts are the
    /// node's own, they go on where a disable left them, and a node that was
    /// stopped and started again has moved what it moved before -- an
    /// operator asking what a payload has done wants the answer for the
    /// flight.
    #[test]
    fn a_node_counts_what_it_has_moved_and_keeps_it_through_a_stop() {
        let start = Instant::now();
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("counted.yaml");
        std::fs::write(
            &path,
            "payloads:
  - { name: asked, type: network, protocol: udp, address: \"127.0.0.1\", port: \"7009\",
      trigger: \"55\", trigger_interval: \"100 ms\", payload-out: average }
vehicle:
  - { name: average, type: compute, function: mean, inputs: 1 }
",
        )
        .expect("written");
        let graph = Graph::load(&path).expect("it reads");

        let readings = kept();
        let asks = tally();
        let scripted = readings.clone();
        let counted = asks.clone();
        let mut system = System::of_opening(
            &graph,
            start,
            move |node: &Node, _: &Routes, _: &Abandon| match node.node_type.as_str() {
                "network" => Ok(Box::new(Asked {
                    readings: scripted.clone(),
                    asks: counted.clone(),
                }) as Box<dyn Port>),
                _ => Ok(Box::new(Ground::default()) as Box<dyn Port>),
            },
            |_: &Telemetered| Ok(Box::new(Ground::default()) as Box<dyn Port>),
        )
        .expect("it opens");

        let states = system.states();
        let counts = |node: &str| states.named(node).expect("the node").counts();

        // Nothing has moved, and nothing has a time: a node that has never
        // spoken is not a node that went quiet.
        assert_eq!(counts("asked").taken_bytes, 0);
        assert_eq!(counts("asked").last_moved, None);

        // A pass with nothing to read: the trigger goes out and is counted,
        // and a read of nought bytes is counted as neither a read nor a
        // failure -- a silent payload would otherwise be the busiest thing in
        // the telemetry.
        system.pass(start);
        assert_eq!(counts("asked").asks, 1, "the trigger went out");
        assert_eq!(counts("asked").reads, 0, "nothing was there to read");
        assert_eq!(counts("asked").last_moved, None);

        // And one with a reading, which the computation is given.
        readings.lock().expect("the record").push(b"warm".to_vec());
        system.pass(start + Duration::from_millis(100));
        let taken = counts("asked");
        assert_eq!((taken.reads, taken.taken_bytes), (1, 4));
        assert!(taken.last_moved.is_some(), "and when it last moved");
        let given = counts("average");
        assert_eq!(
            (given.writes, given.given_bytes),
            (1, 4),
            "the computation was given it"
        );

        // The sample is the bytes themselves, for an operator who wants to see
        // them rather than count them.
        let samples = states.named("asked").expect("the node").samples();
        assert_eq!(samples.taken.data(), b"warm");
        assert_eq!(
            states
                .named("average")
                .expect("the node")
                .samples()
                .given
                .data(),
            b"warm"
        );

        // Stopped: a delivery to it is dropped and counted apart from the
        // refusals, a dropped delivery being the configuration working as the
        // ground asked rather than a node that would not take what it was
        // given.
        states.named("average").expect("the node").disable();
        readings.lock().expect("the record").push(b"cold".to_vec());
        system.pass(start + Duration::from_millis(200));
        assert_eq!(counts("average").dropped, 1);
        assert_eq!(counts("average").writes, 1, "and no second delivery");

        // Started again, the counts go on from where they were.
        states.named("average").expect("the node").enable();
        readings.lock().expect("the record").push(b"warm".to_vec());
        system.pass(start + Duration::from_millis(300));
        let given = counts("average");
        assert_eq!(
            (given.writes, given.given_bytes),
            (2, 8),
            "a node stopped and started again has moved what it moved before"
        );
        assert_eq!(given.dropped, 1, "and the drop is still counted");
    }

    /// A node turned some datagrams away as a stranger's.
    struct Besieged {
        turned_away: u64,
    }

    impl Port for Besieged {
        fn take(&mut self, _buffer: &mut [u8]) -> Result<Taken, String> {
            Ok(Taken::NOTHING)
        }

        fn strangers(&mut self) -> u64 {
            std::mem::take(&mut self.turned_away)
        }

        fn give(&mut self, _arriving: Arriving<'_>, _data: &[u8]) -> Result<(), String> {
            Ok(())
        }
    }

    /// What a node's socket turned away is in its counts, for the ground to
    /// see, though the read that turned it away found nothing of the
    /// payload's: that is the read a flood of strangers looks like.
    #[test]
    fn the_strangers_a_node_turned_away_are_counted() {
        let start = Instant::now();
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("besieged.yaml");
        std::fs::write(
            &path,
            "payloads:
  - { name: asked, type: network, protocol: udp, address: \"127.0.0.1\", port: \"7010\",
      trigger: \"[55]\", trigger_interval: \"100 ms\", payload-out: average }
vehicle:
  - { name: average, type: compute, function: mean, inputs: 1 }
",
        )
        .expect("written");
        let graph = Graph::load(&path).expect("it reads");

        let mut system = System::of_opening(
            &graph,
            start,
            |node: &Node, _: &Routes, _: &Abandon| match node.node_type.as_str() {
                "network" => Ok(Box::new(Besieged { turned_away: 3 }) as Box<dyn Port>),
                _ => Ok(Box::new(Ground::default()) as Box<dyn Port>),
            },
            |_: &Telemetered| Ok(Box::new(Ground::default()) as Box<dyn Port>),
        )
        .expect("it opens");
        let states = system.states();
        let counts = |node: &str| states.named(node).expect("the node").counts();

        system.pass(start);
        assert_eq!(counts("asked").strangers, 3);
        assert_eq!(
            counts("asked").reads,
            0,
            "nothing of the payload's was read"
        );
        system.pass(start + Duration::from_millis(100));
        assert_eq!(counts("asked").strangers, 3, "and each is counted once");
    }

    /// A stopped node gives up what it held, and starting it opens it again.
    ///
    /// Through the opener, because that is the claim: releasing a node drops
    /// the port, and the port is what owns the device, the bus address or the
    /// socket -- so a node held open but idle, which looks from outside like a
    /// node that is running, is exactly what this rules out. Opening it again
    /// is the same work as starting it was, by the same function, so it can
    /// fail the same ways.
    #[test]
    fn a_stopped_node_gives_up_what_it_held() {
        let start = Instant::now();
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("released.yaml");
        std::fs::write(
            &path,
            "payloads:
  - { name: asked, type: network, protocol: udp, address: \"127.0.0.1\", port: \"7006\",
      poll_interval: \"100 ms\", payload-out: average }
vehicle:
  - { name: average, type: compute, function: mean, inputs: 1 }
",
        )
        .expect("written");
        let graph = Graph::load(&path).expect("it reads");

        // How many times each node has been opened, and whether the next
        // opening is to fail.
        let opens = tally();
        let broken = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let counted = opens.clone();
        let failing = broken.clone();
        let mut system = System::of_opening(
            &graph,
            start,
            move |_: &Node, _: &Routes, _: &Abandon| {
                if failing.load(Ordering::Relaxed) {
                    return Err("/dev/nothing is not there".to_string());
                }
                counted.fetch_add(1, Ordering::Relaxed);
                Ok(Box::new(Ground::default()) as Box<dyn Port>)
            },
            |_: &Telemetered| Ok(Box::new(Ground::default()) as Box<dyn Port>),
        )
        .expect("it opens");

        assert_eq!(
            opens.load(Ordering::Relaxed),
            2,
            "both nodes were opened at startup"
        );
        let states = system.states();

        // Stopped, as the command interpreter stops it, and then released as
        // the pass does between stages.
        states.named("asked").expect("the node").disable();
        system.release("asked").expect("it is released");
        assert_eq!(
            system.holds(),
            1,
            "the node is gone and the computation it fed remains"
        );

        // Releasing twice is success: a stop that arrives twice is the
        // ordinary case on a link with latency.
        system.release("asked").expect("releasing twice is success");

        // A node that will not open is left stopped and the reason given
        // back, rather than left enabled over a port that is not there.
        broken.store(true, Ordering::Relaxed);
        let why = system.reopen("asked").expect_err("it cannot open");
        assert!(why.contains("not there"), "{why}");
        assert!(
            !states.is_enabled("asked"),
            "a node that would not open stays stopped"
        );

        // And opened again, it is opened by the same function and let back in.
        broken.store(false, Ordering::Relaxed);
        system.reopen("asked").expect("it opens again");
        assert_eq!(opens.load(Ordering::Relaxed), 3, "the opener ran again");
        assert!(states.is_enabled("asked"), "and the flag goes up with it");
        assert!(states.disabled().is_empty());

        // A name the configuration does not have is refused rather than
        // quietly doing nothing.
        assert!(system.reopen("nobody").is_err());
        assert!(system.release("nobody").is_err());
    }

    /// Work handed over by the command interpreter is done by the pass, and
    /// the thread that asked is told how it went.
    ///
    /// From another thread, because that is where it comes from: the command
    /// interpreter has its own, and the whole point of the handover is that
    /// neither thread does the other's work.
    #[test]
    fn work_handed_over_is_done_by_the_pass() {
        let start = Instant::now();
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("handed.yaml");
        std::fs::write(
            &path,
            "payloads:
  - { name: asked, type: network, protocol: udp, address: \"127.0.0.1\", port: \"7007\",
      poll_interval: \"100 ms\", payload-out: average }
vehicle:
  - { name: average, type: compute, function: mean, inputs: 1 }
",
        )
        .expect("written");
        let graph = Graph::load(&path).expect("it reads");
        let mut system = System::of_opening(
            &graph,
            start,
            |_: &Node, _: &Routes, _: &Abandon| Ok(Box::new(Ground::default()) as Box<dyn Port>),
            |_: &Telemetered| Ok(Box::new(Ground::default()) as Box<dyn Port>),
        )
        .expect("it opens");

        let states = system.states();
        let bench = system.workbench();
        states.named("asked").expect("the node").disable();

        // Asked from another thread, which blocks until a pass takes it.
        let asking = std::thread::spawn(move || bench.ask(Work::Disable("asked".into())));

        // The pass takes the work between its stages and answers it.
        let waited = Instant::now();
        while system.holds() == 2 && waited.elapsed() < Duration::from_secs(5) {
            system.pass(Instant::now());
        }
        assert_eq!(system.holds(), 1, "the pass released it");
        assert_eq!(
            asking.join().expect("the asking thread"),
            Ok(()),
            "and the thread that asked was told"
        );
    }

    /// The shipped example, with the three sensors scripted and the ground
    /// link and the heaters watched.
    struct Vehicle {
        system: System,
        ground: Kept,
        heaters: Kept,
        /// What the sensors sent over their own links to the ground.
        telemetry: Kept,
        log: std::path::PathBuf,
        /// Each sensor's script, kept so that a test can feed it again.
        ///
        /// A test of one pass scripts the readings once and is done; a test
        /// of something that happens *between* passes has to give the sensors
        /// something to say on the second one.
        sensors: BTreeMap<String, Kept>,
    }

    impl Vehicle {
        /// Give one sensor another reading, for the next pass to take.
        fn scripted(&self, name: &str, datum: &[u8]) {
            self.sensors
                .get(name)
                .unwrap_or_else(|| panic!("there is no sensor called \"{name}\" here"))
                .lock()
                .expect("the record")
                .push(datum.to_vec());
        }
    }

    fn vehicle(now: Instant, readings: &[(&str, &[u8])]) -> Vehicle {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut graph =
            Graph::load(root.join("tests/manual/thermostat.yaml")).expect("the example reads");

        // The log writes into a directory of this test's own; the example's is
        // a place on a vehicle. Leaked on purpose: the directory must outlive
        // the system that is writing into it, and a test that tidied it up
        // first would be a test of a log with nowhere to write.
        let where_ = Box::leak(Box::new(tempfile::tempdir().expect("a directory")));
        for node in graph.nodes.iter_mut() {
            if node.node_type == "log" {
                node.attributes.insert(
                    "directory".to_string(),
                    Scalar::of(where_.path().to_str().expect("a path")),
                );
            }
        }
        let log = where_.path().to_path_buf();

        // One script per sensor, shared with the test: see `Vehicle::scripted`.
        let sensors: BTreeMap<String, Kept> = readings
            .iter()
            .map(|(name, datum)| {
                (
                    name.to_string(),
                    Arc::new(TestLock::new(vec![datum.to_vec()])),
                )
            })
            .collect();

        let ground = kept();
        let heaters = kept();
        let telemetry = kept();

        // Cloned into the opener for the reason above: what the test watches
        // and what the opener fills are the same reference-counted places.
        let opening_heaters = Arc::clone(&heaters);
        let opening_ground = Arc::clone(&ground);
        let opening_sensors = sensors.clone();
        let system = System::of_opening(
            &graph,
            now,
            move |node: &Node, routes: &Routes, abandon: &Abandon| {
                match node.node_type.as_str() {
                    // The nodes that would touch hardware.
                    "i2c" | "spi" => Ok(Box::new(Sensor {
                        readings: opening_sensors
                            .get(&node.name)
                            .map_or_else(kept, Arc::clone),
                    }) as Box<dyn Port>),
                    "gpio" => Ok(Box::new(Ground {
                        got: Arc::clone(&opening_heaters),
                    }) as Box<dyn Port>),
                    "moc" => Ok(Box::new(Ground {
                        got: Arc::clone(&opening_ground),
                    }) as Box<dyn Port>),
                    // And the ones that need nothing but a configuration, which
                    // are opened for real.
                    _ => open(node, routes, abandon, &SegSend::none()),
                }
            },
            // What goes to the ground, watched rather than sent: there is no
            // ground station here to bind the other end. A line drawn into the
            // moc node arrives at its source's own link, so those are what the
            // ground is watched at; an untouched payload's link is watched
            // apart.
            |link: &Telemetered| {
                let got = match link.via {
                    tcslibgs::graph::telemetry::Via::Moc(_) => Arc::clone(&ground),
                    tcslibgs::graph::telemetry::Via::PayloadTelemetry => Arc::clone(&telemetry),
                };
                Ok(Box::new(Ground { got }) as Box<dyn Port>)
            },
        )
        .expect("the example opens");

        Vehicle {
            system,
            ground,
            heaters,
            telemetry,
            log,
            sensors,
        }
    }

    /// Counting the allocations made on one thread, for
    /// [`a_running_pass_allocates_nothing`].
    ///
    /// The process's allocator, counting a thread's allocations while that
    /// thread asks it to and passing everything to the system allocator.
    /// Per thread, so that the other tests of this binary, running beside it,
    /// are not counted.
    mod counting {
        use std::alloc::{GlobalAlloc, Layout, System};
        use std::cell::Cell;

        thread_local! {
            static COUNTING: Cell<bool> = const { Cell::new(false) };
            static COUNT: Cell<usize> = const { Cell::new(0) };
        }

        pub struct Counting;

        fn note() {
            // `try_with`, because an allocation can come while a thread is
            // being torn down, when its locals are gone.
            if COUNTING.try_with(Cell::get).unwrap_or(false) {
                let _ = COUNT.try_with(|count| count.set(count.get() + 1));
            }
        }

        unsafe impl GlobalAlloc for Counting {
            unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
                note();
                System.alloc(layout)
            }

            unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
                note();
                System.alloc_zeroed(layout)
            }

            unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
                note();
                System.realloc(ptr, layout, size)
            }

            unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
                System.dealloc(ptr, layout)
            }
        }

        #[global_allocator]
        static ALLOCATOR: Counting = Counting;

        /// How many allocations `work` makes on this thread.
        pub fn allocations_in(work: impl FnOnce()) -> usize {
            COUNT.with(|count| count.set(0));
            COUNTING.with(|on| on.set(true));
            work();
            COUNTING.with(|on| on.set(false));
            COUNT.with(Cell::get)
        }
    }

    /// A sensor that answers every read with the same reading.
    struct Steady {
        reading: &'static [u8],
    }

    impl Port for Steady {
        fn take(&mut self, buffer: &mut [u8]) -> Result<Taken, String> {
            buffer[..self.reading.len()].copy_from_slice(self.reading);
            Ok(Taken::of(self.reading.len()))
        }

        fn give(&mut self, _arriving: Arriving<'_>, _data: &[u8]) -> Result<(), String> {
            Err("a sensor is read and not written".to_string())
        }

        fn accepts(&self) -> bool {
            false
        }
    }

    /// Somewhere a datum can go, keeping nothing but a count.
    struct Sink {
        got: Tally,
    }

    impl Port for Sink {
        fn take(&mut self, _buffer: &mut [u8]) -> Result<Taken, String> {
            Ok(Taken::NOTHING)
        }

        fn give(&mut self, _arriving: Arriving<'_>, _data: &[u8]) -> Result<(), String> {
            self.got.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    /// Once it is running, a pass allocates nothing.
    ///
    /// The rule for everything after the main loop is entered: what a pass
    /// needs is set up as the vehicle starts and reused. The worked example,
    /// its sensors reading steadily, run for a while to settle -- the first
    /// passes may grow what is kept for later ones, once -- and then counted:
    /// reading the group, converting and voting, deciding, driving the
    /// heaters, telling the ground and writing the log.
    #[test]
    fn a_running_pass_allocates_nothing() {
        let where_ = tempfile::tempdir().expect("a directory");
        let delivered = tally();
        let start = Instant::now();
        let mut system = steady_thermostat(where_.path(), &delivered, start);

        // A group's reading every 100 ms: one pass per interval.
        let at = |pass: u32| start + Duration::from_millis(100) * pass;
        for pass in 0..10 {
            let moved = system.pass(at(pass));
            assert!(moved.all_delivered(), "{:?}", moved.refused);
        }
        let before = delivered.load(Ordering::Relaxed);

        let made = counting::allocations_in(|| {
            for pass in 10..30 {
                let _ = system.pass(at(pass));
            }
        });

        assert!(
            delivered.load(Ordering::Relaxed) > before,
            "the passes counted should have delivered something"
        );
        assert_eq!(
            made, 0,
            "twenty running passes allocated {made} times: something in the main \
             loop is allocating again"
        );
    }

    /// Setting a setpoint from the ground allocates nothing on the way: the
    /// SET_NODE read off the link, the work handed to the pass, and the pass
    /// applying it and answering.
    ///
    /// Two threads, as aboard: one reads the commands and asks, and the other
    /// makes the passes. Each counts its own.
    #[test]
    fn setting_a_setpoint_allocates_nothing() {
        use tcslibgs::{Command, NodeId, SetNodeCommand};

        let where_ = tempfile::tempdir().expect("a directory");
        let delivered = tally();
        let start = Instant::now();
        let mut system = steady_thermostat(where_.path(), &delivered, start);
        let bench = system.workbench();
        let thermostat = system
            .states()
            .named("thermostat")
            .expect("the thermostat")
            .shared_name();

        // The commands as they arrive, made before anything is counted.
        let datagrams: Vec<Vec<u8>> = (0..30)
            .map(|sequence| {
                let value = if sequence % 2 == 0 { "280.0" } else { "300.0" };
                serde_json::to_vec(&Command::SetNode(
                    SetNodeCommand::new(sequence, NodeId::of(0), "setpoint", value.as_bytes())
                        .expect("it fits"),
                ))
                .expect("it serializes")
            })
            .collect();

        // When the asking thread starts counting, so that the passes start
        // then too, and when it has finished.
        let counting = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let started = Arc::clone(&counting);
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let finished = Arc::clone(&done);
        let asking = std::thread::spawn(move || {
            // What the command thread does with each: read it, and hand the
            // setting to the pass by the node's shared name.
            let round = |datagram: &[u8]| {
                let Ok(Command::SetNode(set)) = serde_json::from_slice::<Command>(datagram) else {
                    panic!("a SET_NODE");
                };
                bench.ask(Work::Set {
                    node: Arc::clone(&thermostat),
                    attribute: set.attribute,
                    value: set.value,
                })
            };
            for datagram in &datagrams[..10] {
                round(datagram).expect("the first ones are set");
            }
            let mut answers = Vec::with_capacity(20);
            started.store(true, Ordering::SeqCst);
            let made = counting::allocations_in(|| {
                for datagram in &datagrams[10..] {
                    answers.push(round(datagram));
                }
            });
            finished.store(true, Ordering::SeqCst);
            (made, answers)
        });

        let at = |pass: u32| start + Duration::from_millis(100) * pass;
        let mut pass = 0;
        // Until the asking thread is counting: the passes that answer its
        // first ten.
        while !counting.load(Ordering::SeqCst) {
            let _ = system.pass(at(pass));
            pass += 1;
        }
        let made = counting::allocations_in(|| {
            while !done.load(Ordering::SeqCst) {
                let _ = system.pass(at(pass));
                pass += 1;
            }
        });
        let (asked, answers) = asking.join().expect("the asking thread");

        assert!(
            answers.iter().all(Result::is_ok),
            "every setpoint is set: {answers:?}"
        );
        assert_eq!(answers.len(), 20);
        assert_eq!(asked, 0, "reading and handing over allocated {asked} times");
        assert_eq!(made, 0, "the passes doing the work allocated {made} times");
    }

    /// The shipped thermostat, with its sensors steady, its heaters and the
    /// ground counting what reaches them, and its log in `log_in`: a vehicle
    /// that can be made to pass for ever.
    fn steady_thermostat(log_in: &Path, delivered: &Tally, start: Instant) -> System {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut graph =
            Graph::load(root.join("tests/manual/thermostat.yaml")).expect("the example reads");
        for node in graph.nodes.iter_mut() {
            if node.node_type == "log" {
                node.attributes.insert(
                    "directory".to_string(),
                    Scalar::of(log_in.to_str().expect("a path")),
                );
            }
        }

        let opening = Arc::clone(delivered);
        let watched = Arc::clone(delivered);
        System::of_opening(
            &graph,
            start,
            move |node: &Node, routes: &Routes, abandon: &Abandon| match node.node_type.as_str() {
                "i2c" => Ok(Box::new(Steady {
                    reading: &[0x01, 0x00],
                }) as Box<dyn Port>),
                "spi" => Ok(Box::new(Steady {
                    reading: &[0x00, 0x10, 0x00],
                }) as Box<dyn Port>),
                "gpio" | "moc" => Ok(Box::new(Sink {
                    got: Arc::clone(&opening),
                }) as Box<dyn Port>),
                _ => open(node, routes, abandon, &SegSend::none()),
            },
            move |_: &Telemetered| {
                Ok(Box::new(Sink {
                    got: Arc::clone(&watched),
                }) as Box<dyn Port>)
            },
        )
        .expect("the example opens")
    }

    /// What every command and every beacon goes through allocates nothing:
    /// whether a datagram came from where the configuration says, an answer
    /// written as JSON into a buffer that is reused, and a beacon packed by a
    /// packer that is. What the command thread and the beacon thread do for
    /// each, but for the socket.
    #[test]
    fn answering_and_beaconing_allocate_nothing() {
        use tcslibgs::{
            came_from, Beacon, BeaconPacker, CommandStatus, ConfigDigest, ConfigVersion, HostPort,
            PingTelemetry, Telemetry,
        };

        let stated = HostPort {
            address: "127.0.0.1".to_string(),
            port: 4000,
        };
        let from: std::net::SocketAddr = "127.0.0.1:4000".parse().expect("an address");
        let stranger: std::net::SocketAddr = "10.0.0.1:4000".parse().expect("an address");
        let beacon = Beacon::new(7, ConfigVersion::of_this_build(), ConfigDigest([0x5a; 16]));
        let answer = Telemetry::Ping(PingTelemetry::new(3, CommandStatus::Success));
        // Made once, as each thread makes its own.
        let mut data: Vec<u8> = Vec::with_capacity(1024);
        let mut packer = BeaconPacker::new();

        let made = counting::allocations_in(|| {
            for _ in 0..10 {
                assert!(came_from(&stated, &from));
                assert!(!came_from(&stated, &stranger));
                data.clear();
                serde_json::to_writer(&mut data, &answer).expect("it is written");
                assert!(!packer.pack(&beacon).is_empty());
            }
        });
        assert_eq!(
            made, 0,
            "a command's or a beacon's way allocated {made} times"
        );
    }

    /// A pass of the worked example, from the sensors to the heaters.
    ///
    /// The test this module exists for: one pass, driven by the schedule,
    /// doing everything the configuration describes. Three cold readings in,
    /// three temperatures and their mean to the ground, four records in the
    /// log, and the heaters asserted.
    #[test]
    fn one_pass_runs_the_whole_configuration() {
        let start = Instant::now();
        let mut vehicle = vehicle(
            start,
            &[
                ("temp_1", &[0x01, 0x00]),
                ("temp_2", &[0x01, 0x00]),
                ("temp_3", &[0x00, 0x10, 0x00]),
            ],
        );

        // Everything is due at a start, so one pass reads the group.
        let moved = vehicle.system.pass(start);
        assert!(moved.all_delivered(), "{:?}", moved.refused);

        // A temperature on a link is a value, so it is read back as one; a
        // decision is text, and comes through as it stands. Written the one
        // way for both so that what the ground received reads as what it
        // means rather than as four bytes.
        let to_ground: Vec<String> = vehicle
            .ground
            .lock()
            .expect("the record")
            .iter()
            .map(
                |datum| match crate::node::vehicle::compute::from_value(datum, "the ground") {
                    Ok(number) => format!("{number}"),
                    Err(_) => String::from_utf8_lossy(datum).to_string(),
                },
            )
            .collect();
        // The three readings, the one temperature voted out of them, and the
        // decision. The vote is 289.15 and not the mean's 288.5396: sorted,
        // the readings are 287.3188, 289.15 and 289.15, so the median is
        // 289.15 and the low one is left out -- which is what a trio is for.
        //
        // The readings in any order among themselves: each goes to the ground
        // on a link of its own, and the three are sent at once, in one wave,
        // where the order three sockets are written in is nobody's. What is
        // ordered is the waves: every reading is on its way before the vote
        // over them, and the vote before the decision taken from it.
        assert_eq!(to_ground.len(), 5, "{to_ground:?}");
        let mut readings = to_ground[..3].to_vec();
        readings.sort();
        assert_eq!(
            readings,
            ["287.3188", "289.15", "289.15"],
            "the ground should have the three readings first: {to_ground:?}"
        );
        assert_eq!(
            to_ground[3..],
            ["289.15", "1"],
            "and then the vote over them, and the decision"
        );
        assert_eq!(
            vehicle.heaters.lock().expect("the record").clone(),
            // One decision, reaching both heaters: they are driven together,
            // so that one failing leaves the other warming the mass.
            vec![b"1".to_vec(), b"1".to_vec()],
            "a cabin below the band should have both its heaters on"
        );
        assert!(
            vehicle.telemetry.lock().expect("the record").is_empty(),
            "the sensors are drawn into their conversions, and report only through them"
        );

        // And the log holds the pass: one segment file with something in it,
        // flushed by the pass itself rather than by anything this test did.
        let written: Vec<std::fs::DirEntry> = std::fs::read_dir(&vehicle.log)
            .expect("the log directory")
            .map(|entry| entry.expect("an entry"))
            .collect();
        assert_eq!(written.len(), 1, "one segment file");
        assert!(
            written[0].metadata().expect("its size").len() > 0,
            "the pass should have flushed the log"
        );
    }

    /// A payload nothing in the drawing touches is read, and what it reads
    /// goes to the ground over its own link: the minimal example.
    #[test]
    fn an_undrawn_payload_reports_over_its_own_link() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let graph =
            Graph::load(root.join("examples/minimal/minimal.yaml")).expect("the example reads");

        let sent = kept();
        let mut system = System::of_opening(
            &graph,
            Instant::now(),
            |_node: &Node, _routes: &Routes, _: &Abandon| {
                Ok(Box::new(Sensor {
                    readings: Arc::new(TestLock::new(vec![b"reading 1".to_vec()])),
                }) as Box<dyn Port>)
            },
            |link: &Telemetered| {
                assert_eq!(link.source, "sensor");
                Ok(Box::new(Ground {
                    got: Arc::clone(&sent),
                }) as Box<dyn Port>)
            },
        )
        .expect("the example opens");

        assert_eq!(system.polled(), ["sensor"], "it is read, being in no group");
        let moved = system.pass(Instant::now());
        assert!(moved.all_delivered(), "{:?}", moved.refused);
        assert_eq!(
            sent.lock().expect("the record").clone(),
            vec![b"reading 1".to_vec()]
        );
    }

    /// A ping from the ground is answered to the ground in the same pass.
    ///
    /// The ground link stands in for the socket the MOC sends to; the ping
    /// node is the real one. What reaches the ground is the ping, a space and
    /// the vehicle's time, over the link drawn from the ping node to the
    /// `moc` node; and what is not a ping is refused and answers nothing.
    #[test]
    fn a_ping_from_the_ground_is_answered_to_the_ground() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("ping.yaml");
        std::fs::write(
            &path,
            "moc:\n\
             \x20 - { name: ground, type: moc, address: 127.0.0.1, port: \"5000\", \
             ground_base: \"127.0.0.1:5100\", moc-out: pinger }\n\
             vehicle:\n\
             \x20 - { name: pinger, type: ping, payload-out: ground }\n",
        )
        .expect("written");
        let graph = Graph::load(&path).expect("it reads");

        let pings = Arc::new(TestLock::new(vec![
            b"ping #1 1760900000.000000001".to_vec(),
            b"hello".to_vec(),
        ]));
        let sent = kept();
        let mut system = System::of_opening(
            &graph,
            Instant::now(),
            move |node: &Node, routes: &Routes, abandon: &Abandon| {
                if node.node_type == "moc" {
                    Ok(Box::new(Sensor {
                        readings: Arc::clone(&pings),
                    }) as Box<dyn Port>)
                } else {
                    open(node, routes, abandon, &SegSend::none())
                }
            },
            |link: &Telemetered| {
                assert_eq!(link.source, "pinger");
                assert_eq!(
                    link.via,
                    tcslibgs::graph::telemetry::Via::Moc("ground".to_string())
                );
                Ok(Box::new(Ground {
                    got: Arc::clone(&sent),
                }) as Box<dyn Port>)
            },
        )
        .expect("it opens");
        assert_eq!(
            system.polled(),
            ["ground"],
            "the ground link is read; the ping node is not"
        );

        let moved = system.pass(Instant::now());
        assert!(moved.all_delivered(), "{:?}", moved.refused);
        let answers = sent.lock().expect("the record").clone();
        assert_eq!(answers.len(), 1, "{answers:?}");
        let pong = tcslibgs::node::vehicle::ping::read_reply(&answers[0]).expect("an answer");
        assert_eq!(pong.ping.count, 1);
        assert!(answers[0].starts_with(b"ping #1 1760900000.000000001 "));

        // The second datum is not a ping: refused by the node, and nothing goes
        // to the ground.
        let moved = system.pass(Instant::now());
        assert_eq!(moved.refused.len(), 1, "{:?}", moved.refused);
        assert_eq!(moved.refused[0].0, "pinger");
        assert_eq!(sent.lock().expect("the record").len(), 1);
    }

    /// The group is read on its interval and not before.
    #[test]
    fn a_group_is_read_when_it_is_due_and_not_otherwise() {
        let start = Instant::now();
        let mut vehicle = vehicle(start, &[("temp_1", &[0x01, 0x00])]);

        // The first pass reads the group, the sensors having been scripted
        // with one reading each.
        let first = vehicle.system.pass(start);
        assert!(first.delivered > 0, "the group is due at a start");

        // A pass a moment later is not: a tenth of a second is the example's
        // interval, so nothing is read and nothing is delivered.
        let soon = start + Duration::from_millis(30);
        let second = vehicle.system.pass(soon);
        assert_eq!(second.delivered, 0, "{:?}", second);
        assert!(second.all_delivered(), "{:?}", second.refused);

        // And what the loop waits for is the rest of the interval.
        let waiting = vehicle.system.until_next_pass(soon);
        assert!(
            waiting > Duration::from_millis(50) && waiting <= Duration::from_millis(70),
            "{waiting:?} is not the rest of a tenth of a second"
        );
    }

    /// Nothing is polled in a configuration whose nodes are all in groups.
    ///
    /// Which decides what the loop waits for: a vehicle whose sensors are all
    /// scheduled sleeps until the next group is due and wakes for nothing
    /// else.
    #[test]
    fn a_configuration_of_groups_waits_for_its_groups() {
        let start = Instant::now();
        let vehicle = vehicle(start, &[]);

        assert!(
            vehicle.system.polled().is_empty(),
            "the example's sensors are all in cabin_temps: {:?}",
            vehicle.system.polled()
        );
        assert_eq!(vehicle.system.logs(), ["housekeeping"]);
        assert_eq!(
            vehicle.system.holds(),
            17,
            "twelve nodes in the example, and a link to the ground for each of the five \
             nodes drawn into its moc node"
        );
    }

    /// A command reaches the computation the configuration says it may.
    #[test]
    fn a_command_reaches_its_node() {
        let start = Instant::now();
        let mut vehicle = vehicle(start, &[]);

        assert_eq!(
            vehicle.system.set("thermostat", "setpoint", b"300.0"),
            Ok(())
        );
        assert!(vehicle
            .system
            .set("thermostat", "above", b"5.0")
            .is_err_and(|why| why.contains("no command binding")));
    }

    /// A node that will not open stops the vehicle, and every reason is
    /// given.
    ///
    /// Not the first reason: an operator holding a list of what is wrong can
    /// fix a vehicle in one pass of the hangar, where one error at a time is
    /// one trip each.
    #[test]
    fn a_vehicle_starts_only_if_everything_opens() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let graph =
            Graph::load(root.join("tests/manual/thermostat.yaml")).expect("the example reads");

        let refused = System::of_opening(
            &graph,
            Instant::now(),
            |node, routes, abandon| {
                if node.node_type == "i2c" {
                    return Err(format!("{}: /dev/i2c-1 is not there", node.name));
                }
                if node.node_type == "log" {
                    return Err(format!("{}: /var/log/tcspecial is not there", node.name));
                }
                match node.node_type.as_str() {
                    "spi" | "gpio" | "moc" => Ok(Box::new(Ground::default()) as Box<dyn Port>),
                    _ => open(node, routes, abandon, &SegSend::none()),
                }
            },
            |_link| Ok(Box::new(Ground::default()) as Box<dyn Port>),
        )
        .expect_err("a vehicle whose bus is missing");

        assert_eq!(refused.len(), 3, "{refused:?}");
        assert!(
            refused.iter().any(|why| why.contains("temp_1"))
                && refused.iter().any(|why| why.contains("temp_2"))
                && refused.iter().any(|why| why.contains("housekeeping")),
            "every reason should be given: {refused:?}"
        );
    }

    /// What a configuration describes, without opening any of it.
    #[test]
    fn a_configuration_can_be_described_without_a_vehicle() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let graph =
            Graph::load(root.join("tests/manual/thermostat.yaml")).expect("the example reads");

        let described = described(&graph);
        assert_eq!(described["temp_1"], "i2c, read");
        assert_eq!(described["heaters_1"], "gpio, driven");
        assert_eq!(described["heaters_2"], "gpio, driven");
        assert_eq!(described["moc"], "moc, read");
        assert_eq!(described["k_1"], "compute");
        assert_eq!(described["housekeeping"], "log");
    }

    /// A setpoint the ground sets through the pass changes what the vehicle
    /// decides.
    ///
    /// The whole way through, on the shipped example: the command interpreter
    /// hands `Work::Set` over from a thread of its own, the pass applies it
    /// between stages, and the next pass reads the same temperatures and
    /// tells the heaters the opposite thing. That is what the `commands`
    /// section is for: it is checked when the file is read, and this is the
    /// test that it is applied.
    ///
    /// An attribute no binding mentions is refused, and the reason reaches
    /// the thread that asked: what the ground may change in flight is written
    /// down, so that it is checkable before it flies.
    #[test]
    fn a_setpoint_the_ground_sets_changes_what_the_vehicle_decides() {
        let start = Instant::now();
        // A cabin at 289.15 K, which is below the example's band around
        // 293.15: the heaters go on.
        let readings = [
            ("temp_1", &[0x01, 0x00][..]),
            ("temp_2", &[0x01, 0x00][..]),
            ("temp_3", &[0x00, 0x10, 0x00][..]),
        ];
        let mut vehicle = vehicle(start, &readings);
        vehicle.system.pass(start);
        assert_eq!(
            vehicle.heaters.lock().expect("the record").clone(),
            // One decision reaching both heaters.
            vec![b"1".to_vec(), b"1".to_vec()],
            "a cabin below the band has both its heaters on"
        );

        // The ground lowers the setpoint under the cabin, from a thread of
        // its own as the command interpreter does.
        let bench = vehicle.system.workbench();
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let finished = done.clone();
        let asking = std::thread::spawn(move || {
            let set = bench.ask(Work::Set {
                node: "thermostat".into(),
                attribute: tcslibgs::Attribute::of("setpoint").expect("a name"),
                value: tcslibgs::Value::of(b"280.0").expect("a value"),
            });
            // And an attribute the configuration does not bind, which must
            // come back refused rather than applied.
            let unbound = bench.ask(Work::Set {
                node: "thermostat".into(),
                attribute: tcslibgs::Attribute::of("above").expect("a name"),
                value: tcslibgs::Value::of(b"5.0").expect("a value"),
            });
            finished.store(true, std::sync::atomic::Ordering::SeqCst);
            (set, unbound)
        });

        // The passes that take the work, with the sensors holding nothing:
        // a pass is what answers the asking thread, and a pass that read a
        // temperature would have the thermostat decide before the setpoint
        // arrived -- which is the test asking the wrong question.
        let waited = Instant::now();
        while !done.load(std::sync::atomic::Ordering::SeqCst)
            && waited.elapsed() < Duration::from_secs(5)
        {
            vehicle.system.pass(start);
            std::thread::sleep(Duration::from_millis(2));
        }

        let (set, unbound) = asking.join().expect("the asking thread");
        assert_eq!(set, Ok(()), "a bound attribute is set");
        let why = unbound.expect_err("an attribute no binding mentions");
        assert!(
            why.contains("command binding") && why.contains("thermostat.above"),
            "the reason should reach the ground: {why}"
        );

        // And now the same temperatures again, on a clock past the group's
        // three-second interval so that the pass reads them.
        for (name, datum) in readings {
            vehicle.scripted(name, datum);
        }
        vehicle.system.pass(start + Duration::from_secs(4));

        assert_eq!(
            vehicle.heaters.lock().expect("the record").clone(),
            // On, then off: each decision reaching both heaters.
            vec![b"1".to_vec(), b"1".to_vec(), b"0".to_vec(), b"0".to_vec()],
            "the same cabin is now above the band, so both heaters go off"
        );
    }

    /// The loop makes passes until it is told to stop.
    #[test]
    fn the_loop_runs_until_it_is_stopped() {
        let start = Instant::now();
        let mut vehicle = vehicle(
            start,
            &[
                ("temp_1", &[0x01, 0x00]),
                ("temp_2", &[0x01, 0x00]),
                ("temp_3", &[0x00, 0x10, 0x00]),
            ],
        );

        // Stopped after the first pause, which is the shape of a vehicle
        // commanded to stop: the pass it was making finishes.
        let passes = std::cell::Cell::new(0);
        let waits = std::cell::Cell::new(Duration::ZERO);
        vehicle.system.run(
            &|| passes.get() > 0,
            |_moved| passes.set(passes.get() + 1),
            |waiting| waits.set(waits.get() + waiting),
        );

        assert_eq!(passes.get(), 1, "one pass, then the stop");
        // It waited, and for no longer than the group's own interval. Not a
        // tighter bound than that: what is left of the interval depends on
        // how long the pass took, and a machine running the rest of this
        // suite beside it takes longer -- which is a loaded machine and not a
        // loop that failed to wait.
        assert!(
            waits.get() > Duration::ZERO && waits.get() <= Duration::from_millis(100),
            "the loop should have waited for the next group: {:?}",
            waits.get()
        );
        assert_eq!(
            vehicle.heaters.lock().expect("the record").len(),
            2,
            "one decision, reaching both heaters"
        );
    }
}
