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
//! Delivery is best-effort per destination and the failures are counted. That
//! is the reliability choice and it is worth stating: a log that cannot be
//! written must not stop the ground getting its telemetry, and a heater
//! circuit that will not take a command must not stop the next reading being
//! taken. What must never happen is a failure that nobody hears about, so
//! every one is counted and named.

use std::collections::BTreeMap;

use super::route::Routes;

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
    /// `Ok(0)` is "nothing to say", which is the ordinary answer from a
    /// device that is not due. An error is a failure to read a node that
    /// should have had something.
    fn take(&mut self, buffer: &mut [u8]) -> Result<usize, String>;

    /// Hand this node something, arriving at `input` where it names its
    /// inputs.
    fn give(&mut self, input: Option<&str>, data: &[u8]) -> Result<(), String>;

    /// Whether anything may be handed to this node.
    fn accepts(&self) -> bool {
        true
    }

    /// Whether anything may be taken from it.
    fn produces(&self) -> bool {
        true
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

    fn add(&mut self, other: Moved) {
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

    /// Hand `data` to every place `from`'s output goes.
    ///
    /// Each destination is attempted whatever happened to the ones before it.
    /// A log that cannot be written must not stop the ground getting its
    /// telemetry: they are two destinations of one link, and the link is not
    /// a unit that succeeds or fails as a whole.
    pub fn deliver(&mut self, from: &str, data: &[u8]) -> Moved {
        let mut moved = Moved::default();

        // Cloned because the deliveries are borrowed from the table while the
        // nodes are borrowed mutably from the same struct. A handful of names
        // per datum is not the cost worth restructuring for.
        let deliveries: Vec<_> = self.routes.from(from).to_vec();

        for delivery in deliveries {
            let Some(node) = self.nodes.get_mut(&delivery.to) else {
                // A checked configuration has no such link. Counted rather
                // than ignored, because the alternative is telemetry going
                // quietly nowhere.
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

            match node.give(delivery.input.as_deref(), data) {
                Ok(()) => {
                    moved.delivered += 1;
                    moved.bytes += data.len();
                }
                Err(why) => moved.refused.push((delivery.to.clone(), why)),
            }
        }

        moved
    }

    /// Read one node and deliver whatever it had.
    ///
    /// Nothing to say is not a failure and not a delivery: a device that is
    /// not due returns no bytes, which is the ordinary case on most passes.
    pub fn pump(&mut self, name: &str) -> Moved {
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
            node.take(&mut buffer)
        };

        match taken {
            Ok(0) => moved,
            Ok(n) => {
                let data = buffer[..n].to_vec();
                moved.add(self.deliver(name, &data));
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
        given: Vec<(Option<String>, Vec<u8>)>,
        to_say: Vec<Vec<u8>>,
        refuse: Option<String>,
        accepts: bool,
        produces: bool,
    }

    /// A node that records what it is handed and says what it was told to.
    struct Recorder(Rc<RefCell<Record>>);

    impl Port for Recorder {
        fn take(&mut self, buffer: &mut [u8]) -> Result<usize, String> {
            let mut record = self.0.borrow_mut();
            if record.to_say.is_empty() {
                return Ok(0);
            }
            let datum = record.to_say.remove(0);
            buffer[..datum.len()].copy_from_slice(&datum);
            Ok(datum.len())
        }

        fn give(&mut self, input: Option<&str>, data: &[u8]) -> Result<(), String> {
            let mut record = self.0.borrow_mut();
            if let Some(why) = record.refuse.clone() {
                return Err(why);
            }
            record
                .given
                .push((input.map(str::to_string), data.to_vec()));
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

    fn thermostat() -> Routes {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let graph = Graph::load(root.join("tests/manual/thermostat.yaml")).expect("it reads");
        Routes::of(&graph)
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

        assert_eq!(moc.borrow().given, vec![(None, vec![1, 2, 3, 4])]);
        assert_eq!(log.borrow().given, vec![(None, vec![1, 2, 3, 4])]);
        assert_eq!(
            mean.borrow().given,
            vec![(Some("in0".to_string()), vec![1, 2, 3, 4])],
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
                (Some("in0".to_string()), vec![1]),
                (Some("in1".to_string()), vec![2]),
                (Some("in2".to_string()), vec![3]),
            ]
        );
        // And the ground got all three as well, which is the other leg.
        assert_eq!(moc.borrow().given.len(), 3);
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
        assert_eq!(k.borrow().given, vec![(None, vec![0x01, 0x90])]);
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
                vec![(None, vec![n as u8])],
                "converter {n} got the wrong reading"
            );
        }
    }
}
