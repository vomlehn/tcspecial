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
use std::time::{Duration, Instant};

use tcslibgs::graph::telemetry::Telemetered;
use tcslibgs::graph::Graph;

use crate::endpoint::bind_oc_pair;
use crate::node::vehicle::log::LogNode;
use crate::runtime::fabric::{Fabric, Moved, Port};
use crate::runtime::port::{AppPort, ComputePort, EndpointPort, LogPort, NullPort, Use};
use crate::runtime::route::Routes;
use crate::runtime::schedule::Schedule;

/// How large a datum may be.
///
/// The buffer every read is given, and the most one delivery can carry. A
/// sensor's reading is a handful of bytes and a line event is sixteen; this is
/// room for a whole segment of anything this software moves, and a node that
/// wants less says so with `read_size`.
pub const DATUM_BYTES: usize = 4096;

/// How long a pass may wait when a node is polled rather than scheduled.
///
/// A node in no group is read as it becomes readable, which means asking. The
/// pause bounds how stale such a reading can be; it does not apply to a
/// configuration whose nodes are all in groups, which waits for the next group
/// instead and wakes exactly when it is due.
pub const POLL_PAUSE: Duration = Duration::from_millis(20);

/// Everything a configuration needs in order to run.
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
}

impl System {
    /// Open everything this configuration describes.
    ///
    /// The graph is expected checked -- `Graph::check` first, or `tcsverify`
    /// on the ground -- so what fails here is the hardware rather than the
    /// file. Every failure is reported, not the first: an operator holding a
    /// list of what is wrong can fix a vehicle in one pass of the hangar,
    /// where one error at a time is one trip each.
    pub fn of(graph: &Graph, now: Instant) -> Result<System, Vec<String>> {
        System::of_opening(graph, now, open, open_ground_link)
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
        mut opener: impl FnMut(&tcslibgs::graph::Node, &Routes) -> Result<Box<dyn Port>, String>,
        mut ground: impl FnMut(&Telemetered) -> Result<Box<dyn Port>, String>,
    ) -> Result<System, Vec<String>> {
        let routes = Routes::of(graph);
        let mut fabric = Fabric::new(routes.clone(), DATUM_BYTES);
        let mut refused = Vec::new();
        let mut logs = Vec::new();
        let mut polled = Vec::new();
        let mut paced = BTreeMap::new();

        for node in &graph.nodes {
            match opener(node, &routes) {
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

        Ok(System {
            fabric,
            schedule: Schedule::of(graph, now),
            polled,
            paced,
            logs,
        })
    }

    /// One pass: what is due, then what is polled, then the logs.
    ///
    /// Every group that is due is read in full and in order before the next
    /// is started, which is what a group is for. Each reading is delivered as
    /// it is taken, so everything downstream of a group -- a conversion, a
    /// mean, a decision, a command to a heater -- happens inside the pass that
    /// read it.
    pub fn pass(&mut self, now: Instant) -> Moved {
        let mut moved = Moved::default();

        for due in self.schedule.due(now) {
            let members: Vec<String> = self.schedule.members(&due.group).to_vec();
            moved.add(self.fabric.pump_group(&members));
            self.schedule.read(&due.group, now);
        }

        // The nodes in no group, which are read as they become readable: a
        // read of no bytes is the ordinary answer and costs a system call.
        // One that states how often is read, or asked, as its `Pace` says.
        let polled: Vec<String> = self.polled.clone();
        for node in polled {
            let Some((pace, due)) = self.paced.get(&node).copied() else {
                moved.add(self.fabric.pump(&node));
                continue;
            };
            let is_due = now >= due;
            if is_due {
                self.paced.insert(node.clone(), (pace, pace.next(due, now)));
            }
            match pace {
                // Read, and asked if it is asked, only when due.
                Pace::Exchange(_) | Pace::Floor(_) => {
                    if is_due {
                        moved.add(self.fabric.pump(&node));
                    }
                }
                // Read on every pass, taking what has arrived; asked only
                // when due.
                Pace::Ask(_) => {
                    moved.add(self.fabric.pump_unasked(&node));
                    if is_due {
                        if let Some(why) = self.fabric.ask(&node) {
                            moved.refused.push((node.clone(), why));
                        }
                    }
                }
            }
        }

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
            // that state no interval, which is every version 1 payload set's
            // shape.
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
        // A computation and a `null` node produce because something arrived,
        // which `Port::ready` has already said and the pass has already
        // taken: polling one would be asking a question whose answer is gone.
        && !matches!(node.node_type.as_str(), "compute" | "log" | "null")
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
    Ok(Box::new(EndpointPort::of(what, reader, writer, Use::Driven)))
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
) -> Result<Box<dyn Port>, String> {
    match node.node_type.as_str() {
        "compute" => {
            let inputs = routes.inputs_of(&node.name);
            Ok(Box::new(ComputePort::make(node, &inputs)?))
        }
        "log" => Ok(Box::new(LogPort::of(LogNode::of(node)?))),
        // A node that carries both streams through and does nothing to what
        // passes. Nothing is opened for it: it is not an interface.
        "null" => Ok(Box::new(NullPort::of(&node.name))),
        // A ground link is where the ground's commands and lines arrive.
        // What goes to the ground does not pass through it: each source has
        // a link of its own to the node's ground_base, opened beside it --
        // see `open_ground_link`. So one that binds nothing, or that nothing
        // is read from, is held as a node that is never asked anything.
        "moc" if !tcslibgs::graph::reach::binds(node) || Use::of(routes, &node.name).is_err() => {
            Ok(Box::new(NullPort::of(&node.name)))
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
        _ => Ok(Box::new(EndpointPort::open(node, routes)?)),
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
                "null" => "passes through".to_string(),
                // An app node is a program of its own, and what it does with
                // what arrives is in that program: nothing here can say more
                // than which of the two streams it is run with, which its type
                // already says. See `crate::node::vehicle::app_payload_veh`
                // and its four neighbours.
                other if tcslibgs::node::app::Kind::of(other).is_some() => {
                    "a program".to_string()
                }
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
    use std::cell::RefCell;
    use std::path::Path;
    use std::rc::Rc;

    use tcslibgs::graph::Node;
    use tcslibgs::Scalar;

    /// A device that answers with whatever it was given, in order.
    ///
    /// Standing in for the three sensors, because the example's sensors are
    /// on buses that are not on this machine. What is being tested is the
    /// assembly and the loop: with real hardware here these would be tests of
    /// a bus.
    struct Sensor {
        readings: Rc<RefCell<Vec<Vec<u8>>>>,
    }

    impl Port for Sensor {
        fn take(&mut self, buffer: &mut [u8]) -> Result<Taken, String> {
            let mut readings = self.readings.borrow_mut();
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
        got: Rc<RefCell<Vec<Vec<u8>>>>,
    }

    impl Port for Ground {
        fn take(&mut self, _buffer: &mut [u8]) -> Result<Taken, String> {
            Ok(Taken::NOTHING)
        }

        fn give(&mut self, _arriving: Arriving<'_>, data: &[u8]) -> Result<(), String> {
            self.got.borrow_mut().push(data.to_vec());
            Ok(())
        }
    }

    /// A node that counts how often it is read and how often it is asked, and
    /// has nothing to say.
    struct Counted {
        reads: Rc<std::cell::Cell<usize>>,
        asks: Rc<std::cell::Cell<usize>>,
    }

    impl Port for Counted {
        fn take(&mut self, _buffer: &mut [u8]) -> Result<Taken, String> {
            self.reads.set(self.reads.get() + 1);
            Ok(Taken::NOTHING)
        }

        fn ask(&mut self) -> Result<(), String> {
            self.asks.set(self.asks.get() + 1);
            Ok(())
        }

        fn give(&mut self, _arriving: Arriving<'_>, _data: &[u8]) -> Result<(), String> {
            Ok(())
        }
    }

    /// Reads and asks, by node.
    type Counts = BTreeMap<String, (Rc<std::cell::Cell<usize>>, Rc<std::cell::Cell<usize>>)>;

    /// Payloads in no group, each sending to the ground and each paced every
    /// 100 ms in one of the three ways -- `exchanged`, an I2C part that is
    /// asked; `asked`, a socket that is asked; `floored`, a socket with a poll
    /// interval -- and, where `every_pass`, `always`, which states no interval.
    fn paced(now: Instant, every_pass: bool) -> (System, Counts) {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("paced.yaml");
        let mut text = String::from(
            "version: \"2.0\"
moc:
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
            .map(|node| {
                (
                    node.name.clone(),
                    (Rc::new(std::cell::Cell::new(0)), Rc::new(std::cell::Cell::new(0))),
                )
            })
            .collect();
        let system = System::of_opening(
            &graph,
            now,
            |node: &Node, _: &Routes| {
                let (reads, asks) = &counts[&node.name];
                Ok(Box::new(Counted {
                    reads: Rc::clone(reads),
                    asks: Rc::clone(asks),
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
        let reads = || counts["exchanged"].0.get();

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
        assert_eq!((reads.get(), asks.get()), (1, 1));
        system.pass(start + Duration::from_millis(20));
        system.pass(start + Duration::from_millis(40));
        assert_eq!((reads.get(), asks.get()), (3, 1), "read every pass, asked once");
        system.pass(start + Duration::from_millis(100));
        assert_eq!((reads.get(), asks.get()), (4, 2));
    }

    /// A `poll_interval` is a floor: the least time between one read and the
    /// next, counted from the read.
    #[test]
    fn a_poll_interval_is_the_least_time_between_reads() {
        let start = Instant::now();
        let (mut system, counts) = paced(start, false);
        let reads = || counts["floored"].0.get();

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
        assert_eq!(counts["floored"].1.get(), 0);
    }

    /// A node that states no interval is read on every pass.
    #[test]
    fn a_node_with_no_interval_is_read_on_every_pass() {
        let start = Instant::now();
        let (mut system, counts) = paced(start, true);
        for n in 0..5 {
            system.pass(start + Duration::from_millis(n * 20));
        }
        assert_eq!(counts["always"].0.get(), 5);
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
            "version: \"2.0\"
moc:
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
            |_: &Node, _: &Routes| {
                Ok(Box::new(Counted {
                    reads: Rc::new(std::cell::Cell::new(0)),
                    asks: Rc::new(std::cell::Cell::new(0)),
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

    /// The shipped example, with the three sensors scripted and the ground
    /// link and the heaters watched.
    struct Vehicle {
        system: System,
        ground: Rc<RefCell<Vec<Vec<u8>>>>,
        heaters: Rc<RefCell<Vec<Vec<u8>>>>,
        /// What the sensors sent over their own links to the ground.
        telemetry: Rc<RefCell<Vec<Vec<u8>>>>,
        log: std::path::PathBuf,
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

        let scripted: BTreeMap<String, Vec<Vec<u8>>> = readings
            .iter()
            .map(|(name, datum)| (name.to_string(), vec![datum.to_vec()]))
            .collect();

        let ground = Rc::new(RefCell::new(Vec::new()));
        let heaters = Rc::new(RefCell::new(Vec::new()));
        let telemetry = Rc::new(RefCell::new(Vec::new()));

        let system = System::of_opening(
            &graph,
            now,
            |node: &Node, routes: &Routes| {
            match node.node_type.as_str() {
                // The nodes that would touch hardware.
                "i2c" | "spi" => Ok(Box::new(Sensor {
                    readings: Rc::new(RefCell::new(
                        scripted.get(&node.name).cloned().unwrap_or_default(),
                    )),
                }) as Box<dyn Port>),
                "gpio" => Ok(Box::new(Ground {
                    got: Rc::clone(&heaters),
                }) as Box<dyn Port>),
                "moc" => Ok(Box::new(Ground {
                    got: Rc::clone(&ground),
                }) as Box<dyn Port>),
                // And the ones that need nothing but a configuration, which
                // are opened for real.
                _ => open(node, routes),
            }
            },
            // What goes to the ground, watched rather than sent: there is no
            // ground station here to bind the other end. A line drawn into the
            // moc node arrives at its source's own link, so those are what the
            // ground is watched at; an untouched payload's link is watched
            // apart.
            |link: &Telemetered| {
                let got = match link.via {
                    tcslibgs::graph::telemetry::Via::Moc(_) => Rc::clone(&ground),
                    tcslibgs::graph::telemetry::Via::PayloadTelemetry => Rc::clone(&telemetry),
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
        }
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
                ("temp_fore", &[0x01, 0x00]),
                ("temp_aft", &[0x01, 0x00]),
                ("temp_bay", &[0x00, 0x10, 0x00]),
            ],
        );

        // Everything is due at a start, so one pass reads the group.
        let moved = vehicle.system.pass(start);
        assert!(moved.all_delivered(), "{:?}", moved.refused);

        let to_ground: Vec<String> = vehicle
            .ground
            .borrow()
            .iter()
            .map(|datum| String::from_utf8_lossy(datum).to_string())
            .collect();
        assert_eq!(
            to_ground,
            vec!["289.15", "289.15", "287.3188", "288.5396"],
            "the ground should have three readings and their mean"
        );
        assert_eq!(
            vehicle.heaters.borrow().clone(),
            vec![b"1".to_vec()],
            "a cabin below the band should have its heaters on"
        );
        assert!(
            vehicle.telemetry.borrow().is_empty(),
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

        let sent = Rc::new(RefCell::new(Vec::new()));
        let mut system = System::of_opening(
            &graph,
            Instant::now(),
            |_node: &Node, _routes: &Routes| {
                Ok(Box::new(Sensor {
                    readings: Rc::new(RefCell::new(vec![b"reading 1".to_vec()])),
                }) as Box<dyn Port>)
            },
            |link: &Telemetered| {
                assert_eq!(link.source, "sensor");
                Ok(Box::new(Ground {
                    got: Rc::clone(&sent),
                }) as Box<dyn Port>)
            },
        )
        .expect("the example opens");

        assert_eq!(system.polled(), ["sensor"], "it is read, being in no group");
        let moved = system.pass(Instant::now());
        assert!(moved.all_delivered(), "{:?}", moved.refused);
        assert_eq!(sent.borrow().clone(), vec![b"reading 1".to_vec()]);
    }

    /// The group is read on its interval and not before.
    #[test]
    fn a_group_is_read_when_it_is_due_and_not_otherwise() {
        let start = Instant::now();
        let mut vehicle = vehicle(start, &[("temp_fore", &[0x01, 0x00])]);

        // The first pass reads the group, the sensors having been scripted
        // with one reading each.
        let first = vehicle.system.pass(start);
        assert!(first.delivered > 0, "the group is due at a start");

        // A pass a moment later is not: three seconds is the example's
        // interval, so nothing is read and nothing is delivered.
        let soon = start + Duration::from_millis(500);
        let second = vehicle.system.pass(soon);
        assert_eq!(second.delivered, 0, "{:?}", second);
        assert!(second.all_delivered(), "{:?}", second.refused);

        // And what the loop waits for is the rest of the interval.
        let waiting = vehicle.system.until_next_pass(soon);
        assert!(
            waiting > Duration::from_millis(2_000) && waiting <= Duration::from_millis(2_500),
            "{waiting:?} is not the rest of three seconds"
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
            15,
            "eleven nodes in the example, and a link to the ground for each of the four \
             nodes drawn into its moc node"
        );
    }

    /// A command reaches the computation the configuration says it may.
    #[test]
    fn a_command_reaches_its_node() {
        let start = Instant::now();
        let mut vehicle = vehicle(start, &[]);

        assert_eq!(vehicle.system.set("thermostat", "setpoint", b"300.0"), Ok(()));
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

        let refused = System::of_opening(&graph, Instant::now(), |node, routes| {
            if node.node_type == "i2c" {
                return Err(format!("{}: /dev/i2c-1 is not there", node.name));
            }
            if node.node_type == "log" {
                return Err(format!("{}: /var/log/tcspecial is not there", node.name));
            }
            match node.node_type.as_str() {
                "spi" | "gpio" | "moc" => Ok(Box::new(Ground::default()) as Box<dyn Port>),
                _ => open(node, routes),
            }
        }, |_link| Ok(Box::new(Ground::default()) as Box<dyn Port>))
        .expect_err("a vehicle whose bus is missing");

        assert_eq!(refused.len(), 3, "{refused:?}");
        assert!(
            refused.iter().any(|why| why.contains("temp_fore"))
                && refused.iter().any(|why| why.contains("temp_aft"))
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
        assert_eq!(described["temp_fore"], "i2c, read");
        assert_eq!(described["heaters"], "gpio, driven");
        assert_eq!(described["moc"], "moc, read");
        assert_eq!(described["k_fore"], "compute");
        assert_eq!(described["housekeeping"], "log");
    }

    /// The loop makes passes until it is told to stop.
    #[test]
    fn the_loop_runs_until_it_is_stopped() {
        let start = Instant::now();
        let mut vehicle = vehicle(
            start,
            &[
                ("temp_fore", &[0x01, 0x00]),
                ("temp_aft", &[0x01, 0x00]),
                ("temp_bay", &[0x00, 0x10, 0x00]),
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
        assert!(
            waits.get() >= Duration::from_millis(2_500),
            "the loop should have waited for the next group: {:?}",
            waits.get()
        );
        assert_eq!(vehicle.heaters.borrow().len(), 1);
    }
}
