//! Moving bytes along the links.
//!
//! The routing says where a node's output goes and the schedule says when a
//! group is read; this is what does it. One method of consequence --
//! [`Fabric::deliver`] -- which takes what a node produced and hands it to
//! every place the configuration says it goes.
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
//! where the table is built: see [`crate::runtime::route::stream_of`].
//!
//! Delivery is best-effort per destination and the failures are counted. That
//! is the reliability choice and it is worth stating: a log that cannot be
//! written must not stop the ground getting its telemetry, and a heater
//! circuit that will not take a command must not stop the next reading being
//! taken. What must never happen is a failure that nobody hears about, so
//! every one is counted and named.

use std::collections::{BTreeMap, VecDeque};

use tcslibgs::graph::terminals::Stream;

use super::route::{Delivery, Routes};

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
}

impl Taken {
    /// Nothing to say.
    pub const NOTHING: Taken = Taken {
        bytes: 0,
        stream: None,
    };

    /// This many bytes, to go wherever this node's links go.
    pub fn of(bytes: usize) -> Taken {
        Taken {
            bytes,
            stream: None,
        }
    }

    /// And this many, on one stream.
    pub fn on(bytes: usize, stream: Stream) -> Taken {
        Taken {
            bytes,
            stream: Some(stream),
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
    /// [`crate::runtime::route::stream_of`].
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
pub trait Port {
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
        self.refused.extend(other.refused);
    }
}

/// The nodes, and the table saying how they are joined.
pub struct Fabric {
    nodes: BTreeMap<String, Box<dyn Port>>,
    routes: Routes,
    /// How large a datum may be, which is the buffer a read is given.
    buffer: usize,
}

impl Fabric {
    pub fn new(routes: Routes, buffer: usize) -> Fabric {
        Fabric {
            nodes: BTreeMap::new(),
            routes,
            buffer,
        }
    }

    /// Put a node in, under the name the configuration calls it.
    pub fn hold(&mut self, name: &str, port: Box<dyn Port>) {
        self.nodes.insert(name.to_string(), port);
    }

    pub fn holds(&self, name: &str) -> bool {
        self.nodes.contains_key(name)
    }

    /// How many nodes are held.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether any node is held, which a checked configuration always has.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Flush one node, saying what went wrong if anything did.
    ///
    /// `None` is "nothing to report", which is what every node but a log
    /// answers. A name that is not held is reported, as a delivery to one is:
    /// a log that is not there is a log nothing is being written to.
    pub fn flush(&mut self, name: &str) -> Option<String> {
        match self.nodes.get_mut(name) {
            Some(node) => node.flush().err(),
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
    /// Breadth first, so that the three destinations of a "T" are all reached
    /// before any of their own destinations are. The ground gets a reading
    /// before an on-board computation over it starts, which is the order an
    /// operator watching telemetry would expect, and the order that loses
    /// least if a later step fails.
    pub fn deliver(&mut self, from: &str, data: &[u8]) -> Moved {
        self.deliver_on(from, data, None)
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
        let mut moved = Moved::default();

        // How far a result may be passed on. A path through a graph with no
        // cycle in it visits each node at most once, so a cascade longer than
        // the number of nodes has been round a cycle -- and the links are
        // checked for cycles before any of this runs, which makes this the
        // backstop for a check that was wrong rather than the rule itself.
        // Stopped and named rather than left to run: a loop here would be a
        // vehicle that stops reading its sensors.
        let most = self.nodes.len().max(1);

        let mut work: VecDeque<(String, Vec<u8>, usize, Option<Stream>)> =
            VecDeque::from(vec![(from.to_string(), data.to_vec(), 0, stream)]);

        while let Some((from, data, depth, stream)) = work.pop_front() {
            // Cloned because the deliveries are borrowed from the table while
            // the nodes are borrowed mutably from the same struct. A handful
            // of names per datum is not the cost worth restructuring for.
            let deliveries: Vec<_> = self.routes.from(&from).to_vec();

            for delivery in deliveries {
                // A datum on one stream does not go down the other's links.
                // Nothing is said about it: a node with two streams has two
                // sets of links and every datum it produces belongs to one of
                // them, so the others are not refusals -- they are the rest of
                // the graph, going about its business.
                if !carries(&delivery, stream) {
                    continue;
                }

                let Some(node) = self.nodes.get_mut(&delivery.to) else {
                    // A checked configuration has no such link. Counted
                    // rather than ignored, because the alternative is
                    // telemetry going quietly nowhere.
                    moved.refused.push((
                        delivery.to.clone(),
                        "there is no such node in this fabric".to_string(),
                    ));
                    continue;
                };

                if !node.accepts() {
                    moved.refused.push((
                        delivery.to.clone(),
                        "this node takes nothing: it is read, not written".to_string(),
                    ));
                    continue;
                }

                let arriving = Arriving {
                    input: delivery.input.as_deref(),
                    stream: delivery.stream,
                };
                match node.give(arriving, &data) {
                    Ok(()) => {
                        moved.delivered += 1;
                        moved.bytes += data.len();
                    }
                    Err(why) => {
                        moved.refused.push((delivery.to.clone(), why));
                        continue;
                    }
                }

                if !node.ready() {
                    continue;
                }

                if depth + 1 >= most {
                    moved.refused.push((
                        delivery.to.clone(),
                        format!(
                            "a result is still being passed on after {most} nodes,                              which is more than there are: the links must have a                              cycle in them"
                        ),
                    ));
                    continue;
                }

                let mut buffer = vec![0u8; self.buffer];
                match node.take(&mut buffer) {
                    Ok(taken) if taken.bytes == 0 => {}
                    Ok(taken) => {
                        buffer.truncate(taken.bytes);
                        work.push_back((
                            delivery.to.clone(),
                            buffer,
                            depth + 1,
                            taken.stream,
                        ));
                    }
                    // A node that said it had a result and then did not give
                    // one up. Counted, because the result is gone either way.
                    Err(why) => moved.refused.push((delivery.to.clone(), why)),
                }
            }
        }

        moved
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
        if !self.routes.is_commanded(node, attribute) {
            return Err(format!(
                "no command binding says the ground may set \"{node}.{attribute}\": \
                 what a command may change is in the configuration, so that it is \
                 checkable before it flies"
            ));
        }

        let Some(port) = self.nodes.get_mut(node) else {
            return Err(format!("there is no node called \"{node}\" in this fabric"));
        };

        port.set(attribute, value)
    }

    /// Read one node and deliver whatever it had.
    ///
    /// Nothing to say is not a failure and not a delivery: a device that is
    /// not due returns no bytes, which is the ordinary case on most passes.
    pub fn pump(&mut self, name: &str) -> Moved {
        self.pump_by(name, false)
    }

    /// Read a node without asking it, and deliver what it had: see
    /// [`Port::take_unasked`].
    pub fn pump_unasked(&mut self, name: &str) -> Moved {
        self.pump_by(name, true)
    }

    /// Ask a node, and take nothing: see [`Port::ask`]. What went wrong, if
    /// anything did.
    pub fn ask(&mut self, name: &str) -> Option<String> {
        match self.nodes.get_mut(name) {
            None => Some("there is no such node in this fabric".to_string()),
            Some(node) => node.ask().err(),
        }
    }

    /// Read a node, asking it or not, and deliver what it had.
    fn pump_by(&mut self, name: &str, unasked: bool) -> Moved {
        let mut moved = Moved::default();

        let mut buffer = vec![0u8; self.buffer];
        let taken = {
            let Some(node) = self.nodes.get_mut(name) else {
                moved.refused.push((
                    name.to_string(),
                    "there is no such node in this fabric".to_string(),
                ));
                return moved;
            };
            if !node.produces() {
                return moved;
            }
            if unasked {
                node.take_unasked(&mut buffer)
            } else {
                node.take(&mut buffer)
            }
        };

        match taken {
            Ok(taken) if taken.bytes == 0 => moved,
            Ok(taken) => {
                let data = buffer[..taken.bytes].to_vec();
                moved.add(self.deliver_on(name, &data, taken.stream));
                moved
            }
            Err(why) => {
                moved.refused.push((name.to_string(), why));
                moved
            }
        }
    }

    /// Read every node of a group, in order, and deliver what they had.
    ///
    /// In order and without anything between them, which is what a group is
    /// for: three sensors read one after another are three readings of nearly
    /// the same moment, where three reads a third of an interval apart are
    /// three readings of three different moments being averaged as though
    /// they were one.
    pub fn pump_group(&mut self, members: &[String]) -> Moved {
        let mut moved = Moved::default();
        for member in members {
            moved.add(self.pump(member));
        }
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::path::Path;
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
    }

    /// A node that records what it is handed and says what it was told to.
    struct Recorder(Rc<RefCell<Record>>);

    impl Port for Recorder {
        fn take(&mut self, buffer: &mut [u8]) -> Result<Taken, String> {
            let mut record = self.0.borrow_mut();
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
            let mut record = self.0.borrow_mut();
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
            self.0.borrow().accepts
        }

        fn produces(&self) -> bool {
            self.0.borrow().produces
        }
    }

    fn recorder() -> (Rc<RefCell<Record>>, Box<dyn Port>) {
        let record = Rc::new(RefCell::new(Record {
            accepts: true,
            produces: true,
            ..Record::default()
        }));
        (record.clone(), Box::new(Recorder(record)))
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
        fabric.hold("mean_temp", mean_port);
        fabric.hold("housekeeping", log_port);

        let moved = fabric.deliver("k_fore", &[1, 2, 3, 4]);

        assert!(moved.all_delivered(), "{:?}", moved.refused);
        assert_eq!(moved.delivered, 3);
        assert_eq!(moved.bytes, 12, "four bytes to each of three places");

        let groundward = Some(Stream::Groundward);
        assert_eq!(
            moc.borrow().given,
            vec![(None, groundward, vec![1, 2, 3, 4])]
        );
        assert_eq!(
            log.borrow().given,
            vec![(None, groundward, vec![1, 2, 3, 4])]
        );
        assert_eq!(
            mean.borrow().given,
            vec![(Some("in0".to_string()), groundward, vec![1, 2, 3, 4])],
            "the mean should be told which input this is"
        );
    }

    /// Three sources fill three named inputs of one node.
    #[test]
    fn a_fan_in_arrives_as_named_inputs() {
        let mut fabric = Fabric::new(thermostat(), 64);
        let (mean, mean_port) = recorder();
        let (moc, moc_port) = recorder();
        let (log, log_port) = recorder();
        fabric.hold("mean_temp", mean_port);
        fabric.hold("moc", moc_port);
        fabric.hold("housekeeping", log_port);

        fabric.deliver("k_fore", &[1]);
        fabric.deliver("k_aft", &[2]);
        fabric.deliver("k_bay", &[3]);

        let given = mean.borrow();
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
        assert_eq!(moc.borrow().given.len(), 3);
        assert_eq!(log.borrow().given.len(), 3);
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
        for name in ["k_fore", "k_aft", "k_bay", "mean_temp", "thermostat"] {
            let node = graph.node(name).expect("a node of the example");
            let inputs = routes.inputs_of(name);
            let port = ComputePort::make(node, &inputs)
                .unwrap_or_else(|why| panic!("{name}: {why}"));
            fabric.hold(name, Box::new(port));
        }

        let (moc, moc_port) = recorder();
        let (log, log_port) = recorder();
        let (heaters, heater_port) = recorder();
        fabric.hold("moc", moc_port);
        fabric.hold("housekeeping", log_port);
        fabric.hold("heaters", heater_port);

        // Three raw readings, each in its own part's format, all of them
        // cold: 289.15, 289.15 and 287.3188 kelvin, whose average is more
        // than two degrees below the setpoint of 293.15.
        let mut moved = Moved::default();
        moved.add(fabric.deliver("temp_fore", &[0x01, 0x00]));
        moved.add(fabric.deliver("temp_aft", &[0x01, 0x00]));
        moved.add(fabric.deliver("temp_bay", &[0x00, 0x10, 0x00]));
        assert!(moved.all_delivered(), "{:?}", moved.refused);

        // What the ground saw: three temperatures and the average.
        let payload_out: Vec<String> = moc
            .borrow()
            .given
            .iter()
            .map(|(_, _, datum)| String::from_utf8_lossy(datum).to_string())
            .collect();
        assert_eq!(
            payload_out,
            vec!["289.15", "289.15", "287.3188", "288.5396"],
            "the ground should have the three readings and their mean"
        );

        // The log saw the same three and the decision, which is the other
        // leg of each "T".
        assert_eq!(log.borrow().given.len(), 4, "{:?}", log.borrow().given);

        // And the heaters were asserted, from the one pass.
        let to_heaters: Vec<Vec<u8>> = heaters
            .borrow()
            .given
            .iter()
            .map(|(_, _, datum)| datum.clone())
            .collect();
        assert_eq!(
            to_heaters,
            vec![b"1".to_vec()],
            "a cabin below the band should have its heaters on"
        );
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
        for name in ["k_fore", "k_aft", "k_bay", "mean_temp", "thermostat"] {
            let node = graph.node(name).expect("a node of the example");
            let inputs = routes.inputs_of(name);
            fabric.hold(
                name,
                Box::new(ComputePort::make(node, &inputs).expect("the computation")),
            );
        }
        let (_, moc_port) = recorder();
        let (_, log_port) = recorder();
        let (heaters, heater_port) = recorder();
        fabric.hold("moc", moc_port);
        fabric.hold("housekeeping", log_port);
        fabric.hold("heaters", heater_port);

        // Two passes of the same cold readings. The first decides, the second
        // decides the same thing.
        for _ in 0..2 {
            fabric.deliver("temp_fore", &[0x01, 0x00]);
            fabric.deliver("temp_aft", &[0x01, 0x00]);
            fabric.deliver("temp_bay", &[0x00, 0x10, 0x00]);
        }

        assert_eq!(
            heaters.borrow().given.len(),
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
        let node = graph.node("k_fore").expect("the converter");
        fabric.hold(
            "k_fore",
            Box::new(ComputePort::make(node, &routes.inputs_of("k_fore")).expect("it builds")),
        );
        let (moc, moc_port) = recorder();
        fabric.hold("moc", moc_port);

        // Three bytes where the part's format is two: a bus that returned
        // something else, or a sensor read wrongly.
        let moved = fabric.deliver("temp_fore", &[0x01, 0x00, 0x00]);

        assert_eq!(moved.refused.len(), 1, "{:?}", moved.refused);
        assert_eq!(moved.refused[0].0, "k_fore");
        assert!(
            moved.refused[0].1.contains("to_kelvin"),
            "the complaint should name the computation: {}",
            moved.refused[0].1
        );
        // Nothing reached the ground along that branch, and nothing was lost
        // quietly: the refusal is in hand.
        assert!(moc.borrow().given.is_empty());
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
            Box::new(ComputePort::make(thermostat, &[]).expect("it builds")),
        );
        let (heaters, heater_port) = recorder();
        let (_, log_port) = recorder();
        let (_, moc_port) = recorder();
        fabric.hold("heaters", heater_port);
        fabric.hold("housekeeping", log_port);
        fabric.hold("moc", moc_port);

        // 294 kelvin against a setpoint of 293.15 with two degrees either
        // side: inside the band, so nothing is decided.
        let moved = fabric.deliver("mean_temp", b"294.0");
        assert!(moved.all_delivered(), "{:?}", moved.refused);
        assert!(
            heaters.borrow().given.is_empty(),
            "a cabin inside the band should not have been acted on"
        );

        // The ground asks for a warmer cabin. The decision is not taken on
        // the strength of the number -- the next reading decides -- so the
        // heaters are still untouched.
        assert_eq!(fabric.set("thermostat", "setpoint", b"300.0"), Ok(()));
        assert!(heaters.borrow().given.is_empty());

        // And the same reading now calls for heat.
        let moved = fabric.deliver("mean_temp", b"294.0");
        assert!(moved.all_delivered(), "{:?}", moved.refused);
        assert_eq!(
            heaters.borrow().given,
            // Vehicleward: what a thermostat sends a heater is a command, and
            // the link it goes down is the one leaving the computation's
            // `moc-out`.
            vec![(None, Some(Stream::Vehicleward), b"1".to_vec())],
            "294 is more than two degrees below 300"
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
            Box::new(ComputePort::make(thermostat, &[]).expect("it builds")),
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
        log.borrow_mut().refuse = Some("the device is full".to_string());
        fabric.hold("moc", moc_port);
        fabric.hold("mean_temp", mean_port);
        fabric.hold("housekeeping", log_port);

        let moved = fabric.deliver("k_fore", &[9]);

        assert_eq!(moved.delivered, 2, "the other two should still have got it");
        assert!(!moved.all_delivered());
        assert_eq!(
            moved.refused,
            vec![("housekeeping".to_string(), "the device is full".to_string())],
            "the refusal should name the node and say why"
        );
        assert_eq!(moc.borrow().given.len(), 1);
        assert_eq!(mean.borrow().given.len(), 1);
    }

    /// A node that is not there is counted, not ignored.
    #[test]
    fn a_missing_node_is_reported_rather_than_dropped() {
        let mut fabric = Fabric::new(thermostat(), 64);
        let (moc, moc_port) = recorder();
        fabric.hold("moc", moc_port);
        // mean_temp and housekeeping are not held.

        let moved = fabric.deliver("k_fore", &[7]);

        assert_eq!(moved.delivered, 1);
        assert_eq!(moved.refused.len(), 2, "{:?}", moved.refused);
        assert!(moved.refused.iter().all(|(_, why)| why.contains("no such node")));
        assert_eq!(moc.borrow().given.len(), 1);
    }

    /// A node that is only written is not read, and is not an error.
    #[test]
    fn a_driven_node_is_not_read() {
        let mut fabric = Fabric::new(thermostat(), 64);
        let (heaters, heaters_port) = recorder();
        heaters.borrow_mut().produces = false;
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
        fabric.hold("temp_fore", sensor_port);
        fabric.hold("k_fore", k_port);

        let moved = fabric.pump("temp_fore");
        assert_eq!(moved.delivered, 0);
        assert!(moved.all_delivered(), "{:?}", moved.refused);
    }

    /// Reading a node delivers what it had.
    #[test]
    fn what_a_node_produces_goes_where_its_link_says() {
        let mut fabric = Fabric::new(thermostat(), 64);
        let (sensor, sensor_port) = recorder();
        let (k, k_port) = recorder();
        sensor.borrow_mut().to_say = vec![vec![0x01, 0x90]];
        fabric.hold("temp_fore", sensor_port);
        fabric.hold("k_fore", k_port);

        let moved = fabric.pump("temp_fore");

        assert_eq!(moved.delivered, 1);
        assert_eq!(moved.bytes, 2);
        assert_eq!(
            k.borrow().given,
            vec![(None, Some(Stream::Groundward), vec![0x01, 0x90])]
        );
    }

    /// A group's members are read in order, one pass each.
    #[test]
    fn a_group_is_read_in_order() {
        let mut fabric = Fabric::new(thermostat(), 64);

        let mut sensors = Vec::new();
        for (sensor, converter) in [
            ("temp_fore", "k_fore"),
            ("temp_aft", "k_aft"),
            ("temp_bay", "k_bay"),
        ] {
            let (record, port) = recorder();
            record.borrow_mut().to_say = vec![vec![sensors.len() as u8]];
            fabric.hold(sensor, port);
            let (got, got_port) = recorder();
            fabric.hold(converter, got_port);
            sensors.push(got);
        }

        let members: Vec<String> = ["temp_fore", "temp_aft", "temp_bay"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let moved = fabric.pump_group(&members);

        assert_eq!(moved.delivered, 3, "{:?}", moved.refused);
        for (n, got) in sensors.iter().enumerate() {
            assert_eq!(
                got.borrow().given,
                vec![(None, Some(Stream::Groundward), vec![n as u8])],
                "converter {n} got the wrong reading"
            );
        }
    }
}
