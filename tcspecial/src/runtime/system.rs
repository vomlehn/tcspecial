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

use tcslibgs::graph::Graph;

use crate::node::log::LogNode;
use crate::runtime::fabric::{Fabric, Moved, Port};
use crate::runtime::port::{ComputePort, EndpointPort, LogPort, Use};
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
        System::of_opening(graph, now, open)
    }

    /// Open everything, with `opener` deciding what a node becomes.
    ///
    /// For the tests, which have no bus, no chip and no ground station: the
    /// assembly and the loop are what is worth testing here, and testing them
    /// against real hardware would be testing the hardware.
    pub fn of_opening(
        graph: &Graph,
        now: Instant,
        mut opener: impl FnMut(&tcslibgs::graph::Node, &Routes) -> Result<Box<dyn Port>, String>,
    ) -> Result<System, Vec<String>> {
        let routes = Routes::of(graph);
        let mut fabric = Fabric::new(routes.clone(), DATUM_BYTES);
        let mut refused = Vec::new();
        let mut logs = Vec::new();
        let mut polled = Vec::new();

        for node in &graph.nodes {
            match opener(node, &routes) {
                Ok(port) => {
                    // What is polled and what is flushed, worked out while
                    // the node is in hand: a pass that asked these questions
                    // of every node every time would ask them thousands of
                    // times an hour for an answer that cannot change.
                    if is_polled(node, &routes, port.as_ref()) {
                        polled.push(node.name.clone());
                    }
                    if node.node_type == "log" {
                        logs.push(node.name.clone());
                    }
                    fabric.hold(&node.name, port);
                }
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
        let polled: Vec<String> = self.polled.clone();
        for node in polled {
            moved.add(self.fabric.pump(&node));
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
    /// The next group's due time, or the polling pause where anything is
    /// polled -- whichever is sooner. A configuration whose nodes are all in
    /// groups waits exactly until the next one is due and wakes for nothing
    /// else.
    pub fn until_next_pass(&self, now: Instant) -> Duration {
        let due = self.schedule.until_next(now);
        match (due, self.polled.is_empty()) {
            (Some(due), false) => due.min(POLL_PAUSE),
            (Some(due), true) => due,
            // Nothing scheduled at all: a configuration of ungrouped nodes,
            // which is every version 1 payload set's shape.
            (None, false) => POLL_PAUSE,
            // Nothing scheduled and nothing polled, which a checked
            // configuration does not have -- every node is on a link, and a
            // node that produces is read somehow. Waited out rather than spun
            // on.
            (None, true) => POLL_PAUSE,
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
        && !matches!(node.node_type.as_str(), "compute" | "log")
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
        // Everything else is reached over an interface, and what a node says
        // about reaching it is tcslibgs::graph::reach's business.
        _ => Ok(Box::new(EndpointPort::open(node, routes)?)),
    }
}

/// What each node of a configuration would be opened as, without opening it.
///
/// For a line at startup, and for a ground tool that wants to say what a file
/// describes: the type and which way the links use it. Nothing is opened, so
/// this answers for a vehicle that is not there.
pub fn described(graph: &Graph) -> BTreeMap<String, String> {
    let routes = Routes::of(graph);
    graph
        .nodes
        .iter()
        .map(|node| {
            let how = match node.node_type.as_str() {
                "compute" => "computes".to_string(),
                "log" => "writes a log".to_string(),
                _ => match Use::of(&routes, &node.name) {
                    Ok(Use::Read) => "is read".to_string(),
                    Ok(Use::Driven) => "is driven".to_string(),
                    Ok(Use::Both) => "is read and written".to_string(),
                    Err(why) => why,
                },
            };
            (node.name.clone(), format!("{} that {how}", node.node_type))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
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
        fn take(&mut self, buffer: &mut [u8]) -> Result<usize, String> {
            let mut readings = self.readings.borrow_mut();
            if readings.is_empty() {
                return Ok(0);
            }
            let datum = readings.remove(0);
            buffer[..datum.len()].copy_from_slice(&datum);
            Ok(datum.len())
        }

        fn give(&mut self, _input: Option<&str>, _data: &[u8]) -> Result<(), String> {
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
        fn take(&mut self, _buffer: &mut [u8]) -> Result<usize, String> {
            Ok(0)
        }

        fn give(&mut self, _input: Option<&str>, data: &[u8]) -> Result<(), String> {
            self.got.borrow_mut().push(data.to_vec());
            Ok(())
        }
    }

    /// The shipped example, with the three sensors scripted and the ground
    /// link and the heaters watched.
    struct Vehicle {
        system: System,
        ground: Rc<RefCell<Vec<Vec<u8>>>>,
        heaters: Rc<RefCell<Vec<Vec<u8>>>>,
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

        let system = System::of_opening(&graph, now, |node: &Node, routes: &Routes| {
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
        })
        .expect("the example opens");

        Vehicle {
            system,
            ground,
            heaters,
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
        assert_eq!(vehicle.system.holds(), 11, "eleven nodes in the example");
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
        })
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
        assert_eq!(described["temp_fore"], "i2c that is read");
        assert_eq!(described["heaters"], "gpio that is driven");
        assert_eq!(described["moc"], "moc that is read and written");
        assert_eq!(described["k_fore"], "compute that computes");
        assert_eq!(described["housekeeping"], "log that writes a log");
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
