//! The endpoints as nodes of the graph.
//!
//! [`crate::runtime::fabric`] moves bytes between things that satisfy
//! [`Port`]: take what a node produced, give a node what arrived. An endpoint
//! satisfies neither of those by itself -- it satisfies
//! [`EndpointReadable`] and [`EndpointWritable`], which are a pair of halves
//! with their own error type and their own idea of what a short write means.
//! This is the piece between: one [`EndpointPort`] holding both halves of an
//! opened endpoint and presenting them as one node.
//!
//! One type for every interface and not one per interface, because every
//! endpoint already answers the same two traits. What differs between an i2c
//! sensor and a socket to the ground is inside the endpoint, and a port per
//! type would be nine copies of the same delegation with nine places for them
//! to drift apart.
//!
//! [`ComputePort`] and [`LogPort`] are the other two adapters, and they are
//! here rather than beside the computations and the log for the same reason:
//! they answer the same questions as [`EndpointPort`] -- what a refusal says,
//! what a partial datum means, which direction a node may be used in -- and
//! answers kept together are answers that stay the same.
//!
//! # What this decides
//!
//! Three things, and each is a reliability choice worth stating because the
//! quiet alternative is available in all three cases.
//!
//! *A datum is delivered whole or refused.* A write that moved some of the
//! bytes is reported, naming how many moved. A sensor reading cut in half is
//! not a smaller reading, it is a different number; and the ground reading a
//! truncated record cannot tell that is what it has.
//!
//! *A write of nothing is a failure and not a success of zero bytes.* An
//! endpoint answers `Ok(0)` where the kernel would have blocked -- a socket
//! whose buffer is full, a far end not reading -- and the datum is then
//! dropped. Counted as delivered it would be telemetry lost silently.
//!
//! *Nothing to read is not a failure.* `Ok(0)` from a read is the ordinary
//! answer from a device that is not due, which on most passes is most
//! devices, and every endpoint turns a would-block into it. A port that
//! reported those would bury the real faults in them.

use std::collections::VecDeque;
use std::os::fd::BorrowedFd;
use std::os::unix::io::RawFd;

use nix::poll::{poll, PollFd, PollFlags};

use tcslibgs::graph::reach::reach_of;
use tcslibgs::graph::Node;
use tcslibgs::{EndpointConfig, TcsError, TcsResult};

use crate::node::vehicle::compute::{self, Function};
use crate::node::vehicle::log::LogNode;
use crate::node::vehicle::null::Through;

use crate::endpoint::{
    bind_endpoint_pair, connect_endpoint_pair, EndpointReadable, EndpointWritable,
    WaitResult,
};
use crate::runtime::fabric::Port;
use crate::runtime::route::Routes;

/// Which way the graph uses a node.
///
/// Not a property of the interface: an i2c device can be read and written,
/// and whether this one is depends on what the links say. A set of GPIO lines
/// is the case where the interface has an opinion too -- see the `direction`
/// of a GPIO node -- and the two must agree, which is what [`Use::of`] is
/// for: it reads the graph, and opening the endpoint then says whether the
/// device will stand for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Use {
    /// Read and never written: a sensor, a switch.
    Read,
    /// Written and never read: a heater, a valve, a latch.
    Driven,
    /// Both: a link to the MOC, a channel to another core, a device that
    /// answers what it is asked.
    Both,
}

impl Use {
    /// How this configuration uses the node, or why it does not.
    ///
    /// A node no link touches is refused rather than held as a port that does
    /// nothing. A device opened and never read is a line held, a bus address
    /// claimed and an operator told nothing: the usual cause is a node whose
    /// links were deleted and whose declaration was left behind.
    ///
    /// `Graph::check` refuses such a configuration at load, where the
    /// complaint can name the file and the line. This is the backstop, for
    /// the running software reached by any other road.
    pub fn of(routes: &Routes, node: &str) -> Result<Use, String> {
        // Commands count as something read, and they are the case a table of
        // links alone gets wrong: a thermostat commanded from the ground has
        // a `commands` entry and no link out of the MOC node, so a MOC link
        // judged by its links looks like a node that is only written -- and
        // the setpoint would then never arrive.
        let produces = routes.fan_out(node) > 0 || !routes.commands_on(node).is_empty();
        let accepts = !routes.arriving_at(node).is_empty();

        match (produces, accepts) {
            (true, true) => Ok(Use::Both),
            (true, false) => Ok(Use::Read),
            (false, true) => Ok(Use::Driven),
            (false, false) => Err(format!(
                "no link touches \"{node}\", so there is nothing for it to do: \
                 a device opened and never read holds its lines and says nothing"
            )),
        }
    }

    /// Whether anything may be taken from a node used this way.
    pub fn produces(self) -> bool {
        matches!(self, Use::Read | Use::Both)
    }

    /// Whether anything may be given to a node used this way.
    pub fn accepts(self) -> bool {
        matches!(self, Use::Driven | Use::Both)
    }

    /// How to say this in a complaint.
    fn said(self) -> &'static str {
        match self {
            Use::Read => "read and not written",
            Use::Driven => "driven and not read",
            Use::Both => "read and written",
        }
    }
}

/// An opened endpoint, as a node of the graph.
pub struct EndpointPort {
    /// The node's name in the configuration, which every complaint names:
    /// "cannot write" says nothing without it.
    what: String,
    /// The half a read goes to.
    reader: Box<dyn EndpointReadable + Send>,
    /// The half a write goes to, and a trigger.
    writer: Box<dyn EndpointWritable + Send>,
    /// Which way the graph uses it.
    uses: Use,
    /// What to write to make this device answer, where it must be asked.
    ///
    /// See [`EndpointPort::triggered_by`].
    trigger: Option<Vec<u8>>,
    /// How many bytes one read asks for, where the node says.
    ///
    /// See [`EndpointPort::reading`].
    read_size: Option<usize>,
}

impl EndpointPort {
    /// Open what this configuration describes and hold it as one node.
    ///
    /// For a device: this end reaches out to it. See [`EndpointPort::bind`]
    /// for the end something else reaches, which is what a MOC link is.
    pub fn connect(what: &str, config: &EndpointConfig, uses: Use) -> TcsResult<EndpointPort> {
        let (reader, writer) = connect_endpoint_pair(what, config)?;
        Ok(EndpointPort::of(what, reader, writer, uses))
    }

    /// Wait at the address this configuration describes, as one node.
    ///
    /// For a link to the MOC, which sends to an address it was told, so this
    /// end binds it.
    pub fn bind(what: &str, config: &EndpointConfig, uses: Use) -> TcsResult<EndpointPort> {
        let (reader, writer) = bind_endpoint_pair(what, config)?;
        Ok(EndpointPort::of(what, reader, writer, uses))
    }

    /// Hold halves that are already open.
    ///
    /// Both halves are kept whichever way the node is used, rather than the
    /// unused one dropped. They are not independent: a UDP writer learns
    /// where the far end is from what its reader received, so a socket that
    /// kept only the half the links use today could not answer at all. And a
    /// device that is only read today is one a command binding reaches
    /// tomorrow, which is a configuration change and should not be a code
    /// change. What [`Use`] decides is which half is *used*, not which is
    /// held.
    pub fn of(
        what: &str,
        reader: Box<dyn EndpointReadable + Send>,
        writer: Box<dyn EndpointWritable + Send>,
        uses: Use,
    ) -> EndpointPort {
        EndpointPort {
            what: what.to_string(),
            reader,
            writer,
            uses,
            trigger: None,
            read_size: None,
        }
    }

    /// Open the node a version 2 configuration describes.
    ///
    /// Everything the node says about reaching it comes from
    /// [`tcslibgs::graph::reach`], and which way it is used from the links --
    /// so this is the whole of what turns a configured graph into something
    /// that moves bytes.
    ///
    /// A MOC link is bound and everything else is reached out to. That
    /// follows from which end exists first: the ground sends to an address it
    /// was told, so the vehicle waits at it, where a device is something the
    /// vehicle opens.
    pub fn open(node: &Node, routes: &Routes) -> Result<EndpointPort, String> {
        let uses = Use::of(routes, &node.name)?;
        let reach = reach_of(node)?;

        let opened = if node.node_type == "moc" {
            EndpointPort::bind(&node.name, &reach.endpoint, uses)
        } else {
            EndpointPort::connect(&node.name, &reach.endpoint, uses)
        };
        let mut port = opened.map_err(|e| format!("{e}"))?;

        if let Some(size) = reach.read_size {
            port = port.reading(size as usize);
        }
        if let Some(trigger) = reach.trigger {
            port = port.triggered_by(trigger);
        }

        Ok(port)
    }

    /// Ask for no more than this many bytes at a time.
    ///
    /// What a read asks for is a property of the part on the other end: a bus
    /// master clocks exactly as many bytes as it asks for, so a sensor that
    /// answers in two bytes read sixty-four bytes at a time is a sensor
    /// clocked sixty-two bytes past the end of what it has to say. The
    /// buffer a caller offers is the other bound, and the smaller of the two
    /// wins.
    pub fn reading(mut self, bytes: usize) -> EndpointPort {
        self.read_size = Some(bytes);
        self
    }

    /// Ask this device before reading it.
    ///
    /// Some devices answer when they are asked and not before: a sensor whose
    /// reading is a register read, a payload that sends a frame when it gets
    /// a command. The bytes that ask are written before each read.
    ///
    /// The order is read-then-ask and not ask-then-read, which is the part
    /// worth knowing. A device asked and read in the same breath has had no
    /// time to answer, so a reading taken that way is either nothing or the
    /// answer to the *previous* question arriving by luck -- and a control
    /// loop cannot tell those apart. So a pass takes whatever the last ask
    /// produced and then asks again, which costs the first pass after a start
    /// (it has nothing to take, which a read of no bytes already means) and
    /// makes every later pass a reading of a known age: one interval.
    pub fn triggered_by(mut self, trigger: Vec<u8>) -> EndpointPort {
        self.trigger = Some(trigger);
        self
    }

    /// Which way the graph uses this node.
    pub fn uses(&self) -> Use {
        self.uses
    }

    /// The descriptor a read waits on, for a loop that waits rather than
    /// polls.
    pub fn io_fd(&self) -> RawFd {
        self.reader.io_fd()
    }

    /// Wait until there is something to read, a command, or the time is up.
    pub fn wait_for_event(&self, cmd_fd: RawFd, timeout_ms: i32) -> TcsResult<WaitResult> {
        self.reader.wait_for_event(cmd_fd, timeout_ms)
    }

    /// Whether there is something to read, without waiting to find out.
    ///
    /// A poll with no timeout. An error is the poll's own and is reported: a
    /// descriptor that cannot be asked is a node that cannot be read, and
    /// quietly reporting no bytes would make a broken endpoint look like an
    /// idle one.
    fn ready_to_read(&self) -> Result<bool, String> {
        // SAFETY: a descriptor the reader owns, borrowed for the call and not
        // kept.
        let borrowed = unsafe { BorrowedFd::borrow_raw(self.reader.io_fd()) };
        let mut fds = [PollFd::new(&borrowed, PollFlags::POLLIN)];

        match poll(&mut fds, 0) {
            Ok(0) => Ok(false),
            Ok(_) => Ok(fds[0].revents().is_some_and(|events| {
                // Readable, and also an end or an error: a descriptor at its
                // end is read, which is how an end is noticed at all.
                events.intersects(PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR)
            })),
            Err(e) => Err(format!(
                "{}: cannot ask whether there is anything to read: {e}",
                self.what
            )),
        }
    }

    /// What an endpoint said, without this node's name in it twice.
    ///
    /// Every endpoint names the node it belongs to, and so does the line a
    /// refusal is reported on, so the plain text of an error read
    /// "payload: the write failed: ... payload: nothing has been received".
    /// The name is worth saying once.
    fn plainly(&self, e: TcsError) -> String {
        format!("{e}").replacen(&format!("{}: ", self.what), "", 1)
    }

    /// Write the whole of `data`, or say what happened instead.
    fn write_all(&mut self, data: &[u8], doing: &str) -> Result<(), String> {
        match self.writer.write(data) {
            Ok(n) if n == data.len() => Ok(()),
            Ok(0) => Err(format!(
                "{}: the {doing} of {} bytes moved none of them and was dropped: \
                 commonly a far end that is not reading, or a socket with \
                 nowhere to send yet",
                self.what,
                data.len()
            )),
            Ok(n) => Err(format!(
                "{}: the {doing} moved {n} of {} bytes, and part of a datum is \
                 not a smaller datum",
                self.what,
                data.len()
            )),
            Err(e) => Err(format!(
                "{}: the {doing} failed: {}",
                self.what,
                self.plainly(e)
            )),
        }
    }
}

impl Port for EndpointPort {
    fn take(&mut self, buffer: &mut [u8]) -> Result<usize, String> {
        if !self.uses.produces() {
            return Err(format!(
                "{}: nothing is read from this node -- the configuration has it \
                 {}",
                self.what,
                self.uses.said()
            ));
        }

        // No more than the part has to say, and no more than there is room
        // for: see EndpointPort::reading.
        let want = self
            .read_size
            .map_or(buffer.len(), |size| size.min(buffer.len()));

        // Asked before read, and this is not an optimisation. Some endpoints
        // are opened blocking -- a device file is opened as a file -- so a
        // read of one with nothing in it does not return. A pass that blocked
        // there would be a pass that never ends: the group that was due is
        // never finished, every other group starves, and a stop is not
        // noticed because a stop is noticed between passes. So the descriptor
        // is asked whether there is anything, with no waiting at all, and a
        // node with nothing to say is a read of no bytes -- which is the
        // ordinary answer from a device that is not due.
        let got = match self.ready_to_read() {
            Ok(true) => self
                .reader
                .read(&mut buffer[..want])
                .map_err(|e| format!("{}: the read failed: {}", self.what, self.plainly(e))),
            Ok(false) => Ok(0),
            Err(why) => Err(why),
        };

        // Asked after being read, and asked whether or not the read found
        // anything: a device that answered nothing is a device that most
        // wants asking again. The failure to ask is reported even where the
        // read succeeded, because a reading taken now and no question asked
        // is the last reading this node will ever produce.
        match (got, self.trigger.take()) {
            (got, None) => got,
            (got, Some(trigger)) => {
                let asked = self.write_all(&trigger, "trigger");
                self.trigger = Some(trigger);
                match (got, asked) {
                    (Ok(n), Ok(())) => Ok(n),
                    (Ok(_), Err(why)) => Err(why),
                    // A read that failed is the worse news of the two, and
                    // the one an operator is looking for.
                    (Err(why), _) => Err(why),
                }
            }
        }
    }

    fn give(&mut self, input: Option<&str>, data: &[u8]) -> Result<(), String> {
        if !self.uses.accepts() {
            return Err(format!(
                "{}: nothing is written to this node -- the configuration has it \
                 {}",
                self.what,
                self.uses.said()
            ));
        }

        // A device has one way in and does not name it. A link that named one
        // was written for a computation and points at a device, which is a
        // mis-wiring: refused rather than delivered anyway, because delivering
        // it would make a file that says something it does not mean work by
        // accident.
        if let Some(input) = input {
            return Err(format!(
                "{}: a link arrives at the input \"{input}\", and a device has no \
                 named inputs: only a computation does",
                self.what
            ));
        }

        self.write_all(data, "write")
    }

    fn accepts(&self) -> bool {
        self.uses.accepts()
    }

    fn produces(&self) -> bool {
        self.uses.produces()
    }
}

/// A computation, as a node of the graph.
///
/// The whole of what this adds to a [`Function`] is somewhere to put a result
/// until the fabric takes it. A function is handed one arrival at a time and
/// says for itself when it has enough -- a mean over three sensors answers on
/// the third -- so the result appears as a consequence of a delivery and not
/// of a schedule, which is what [`Port::ready`] is for.
pub struct ComputePort {
    /// The node's name in the configuration, which every complaint names.
    what: String,
    /// What it computes.
    function: Box<dyn Function>,
    /// Results waiting to be taken.
    ///
    /// Normally at most one: the fabric takes a result as soon as the node
    /// says it has one. A queue rather than a slot all the same, because the
    /// alternative when a second result arrives before the first is taken is
    /// to overwrite it -- a reading dropped where nothing says so, which is
    /// the one outcome this project will not have.
    ready: VecDeque<Vec<u8>>,
}

impl ComputePort {
    /// Hold a computation that is already built.
    pub fn of(what: &str, function: Box<dyn Function>) -> ComputePort {
        ComputePort {
            what: what.to_string(),
            function,
            ready: VecDeque::new(),
        }
    }

    /// Build what this node's configuration asks for.
    ///
    /// `inputs` is what the routing found bound to the node, which a
    /// computation over several of them needs: a mean cannot know it is
    /// waiting for three until something tells it how many links arrive.
    pub fn make(node: &Node, inputs: &[&str]) -> Result<ComputePort, String> {
        Ok(ComputePort::of(&node.name, compute::make(node, inputs)?))
    }

    /// What this node computes, for a log line that says what is running.
    pub fn function(&self) -> &str {
        self.function.name()
    }
}

impl Port for ComputePort {
    fn take(&mut self, buffer: &mut [u8]) -> Result<usize, String> {
        let Some(result) = self.ready.pop_front() else {
            return Ok(0);
        };

        // A result that will not fit is reported and not cut down to size.
        // Half of a number is a different number, and the queue keeps it so
        // that a caller with a larger buffer can still have it.
        if result.len() > buffer.len() {
            let wanted = result.len();
            self.ready.push_front(result);
            return Err(format!(
                "{}: the result of {} is {wanted} bytes and the buffer holds {}:                  part of a value is not a smaller value",
                self.what,
                self.function.name(),
                buffer.len()
            ));
        }

        buffer[..result.len()].copy_from_slice(&result);
        Ok(result.len())
    }

    fn give(&mut self, input: Option<&str>, data: &[u8]) -> Result<(), String> {
        match self.function.give(input, data) {
            Ok(Some(result)) => {
                self.ready.push_back(result);
                Ok(())
            }
            // Nothing to send yet: a function waiting for the rest of its
            // inputs, or one that decided nothing needs to change. The
            // ordinary answer, and not a failure.
            Ok(None) => Ok(()),
            Err(why) => Err(format!(
                "{}: {} cannot use what arrived: {why}",
                self.what,
                self.function.name()
            )),
        }
    }

    fn set(&mut self, attribute: &str, value: &[u8]) -> Result<(), String> {
        self.function
            .set(attribute, value)
            .map_err(|why| format!("{}: {why}", self.what))
    }

    fn ready(&self) -> bool {
        !self.ready.is_empty()
    }
}

/// A log, as a node of the graph.
///
/// The thinnest of the three adapters, because a log is the simplest kind of
/// node: what arrives is written, nothing is read, and there is no result to
/// pass on. The input name is ignored on purpose -- a log is the one node type
/// with no fixed number of inputs, so an arrival cannot be ambiguous to it,
/// and every source writing into one log in the order things happened is the
/// whole point of having it.
pub struct LogPort(LogNode);

impl LogPort {
    /// Hold a log that is already open.
    pub fn of(log: LogNode) -> LogPort {
        LogPort(log)
    }

    /// Open the log this node describes.
    pub fn open(node: &Node) -> Result<LogPort, String> {
        Ok(LogPort(LogNode::of(node)?))
    }

}

impl Port for LogPort {
    /// Push what has been written to the file.
    ///
    /// For the end of a pass, and for anything about to happen that the log
    /// should be on the far side of. Not done per record: a log flushed per
    /// arrival is a write system call per reading, and the records are in the
    /// kernel's hands either way.
    fn flush(&mut self) -> Result<(), String> {
        self.0.flush()
    }

    fn take(&mut self, _buffer: &mut [u8]) -> Result<usize, String> {
        Err(format!(
            "{}: nothing is read from a log -- it is where data stops, and a link \
             out of one would be a log read back into the vehicle that wrote it",
            self.0.name()
        ))
    }

    fn give(&mut self, _input: Option<&str>, data: &[u8]) -> Result<(), String> {
        self.0.record(data)
    }

    fn produces(&self) -> bool {
        false
    }
}

/// A `null` node, as a node of the graph.
///
/// The thinnest adapter of the four, because the node itself holds what there
/// is to hold: see [`crate::node::vehicle::null::Through`], which is where the queue and
/// the rule about a datum that will not fit live. Here for the reason the log's
/// adapter and the computation's are here -- the questions a port answers are
/// the same whatever is behind it, and answers kept together stay the same.
pub struct NullPort {
    /// The node's name in the configuration, which every complaint names.
    what: String,
    through: Through,
}

impl NullPort {
    /// A pass-through for this node.
    pub fn of(what: &str) -> NullPort {
        NullPort {
            what: what.to_string(),
            through: Through::new(),
        }
    }
}

impl Port for NullPort {
    fn take(&mut self, buffer: &mut [u8]) -> Result<usize, String> {
        self.through.take(&self.what, buffer)
    }

    fn give(&mut self, _input: Option<&str>, data: &[u8]) -> Result<(), String> {
        self.through.give(data);
        Ok(())
    }

    /// It has something to pass on as a consequence of what arrived.
    ///
    /// A `null` node produces because something came in, as a computation
    /// does, and never on a schedule of its own: a pass that ended with the
    /// delivery would leave the datum here until something else happened to
    /// read the node.
    fn ready(&self) -> bool {
        self.through.waiting()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsRawFd;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use tcslibgs::graph::Graph;
    use tcslibgs::{TcsError, TcsResult};

    use crate::endpoint::EndpointWaitable;

    /// What a stand-in endpoint was asked to do, and what it will answer.
    ///
    /// A pair of halves with no kernel behind them. Everything this module
    /// decides is about the two traits and not about any one interface, so
    /// the tests are against the traits: a socket or a chip here would make
    /// them tests of a socket or a chip.
    #[derive(Default)]
    struct Script {
        /// What a read answers, in order. Each is a result, because "nothing
        /// to read" and "the read failed" are the two cases the port must
        /// tell apart.
        to_read: Vec<TcsResult<Vec<u8>>>,
        /// What was written, in order.
        written: Vec<Vec<u8>>,
        /// What a write answers: the bytes it claims to have moved, or a
        /// failure. `None` is "all of them", which is the ordinary case.
        write_answers: Vec<TcsResult<usize>>,
    }

    /// A stand-in endpoint, and a descriptor that is always readable.
    ///
    /// `/dev/null` for the descriptor, which `EndpointPort::take` polls
    /// before reading: the stand-in answers from its script rather than from
    /// the descriptor, and what the poll has to say is only "there is
    /// something", which a read of /dev/null always does.
    struct Half(Arc<Mutex<Script>>, std::fs::File);

    impl Half {
        fn of(script: &Arc<Mutex<Script>>) -> Half {
            Half(
                Arc::clone(script),
                std::fs::File::open("/dev/null").expect("every machine has one"),
            )
        }
    }

    impl EndpointWaitable for Half {
        fn io_fd(&self) -> RawFd {
            self.1.as_raw_fd()
        }

        fn wait_for_event(&self, _cmd_fd: RawFd, _timeout_ms: i32) -> TcsResult<WaitResult> {
            Ok(WaitResult::IoReady)
        }
    }

    impl EndpointReadable for Half {
        fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
            let mut script = self.0.lock().expect("the script");
            if script.to_read.is_empty() {
                return Ok(0);
            }
            match script.to_read.remove(0) {
                Ok(datum) => {
                    // No more than the slice holds, as a device read into a
                    // short buffer answers what fits.
                    let got = datum.len().min(buffer.len());
                    buffer[..got].copy_from_slice(&datum[..got]);
                    Ok(got)
                }
                Err(e) => Err(e),
            }
        }
    }

    impl EndpointWritable for Half {
        fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
            let mut script = self.0.lock().expect("the script");
            script.written.push(data.to_vec());
            if script.write_answers.is_empty() {
                return Ok(data.len());
            }
            script.write_answers.remove(0)
        }
    }

    /// A port over a stand-in endpoint, and the script both halves share.
    fn port(what: &str, uses: Use) -> (Arc<Mutex<Script>>, EndpointPort) {
        let script = Arc::new(Mutex::new(Script::default()));
        let port = EndpointPort::of(
            what,
            Box::new(Half::of(&script)),
            Box::new(Half::of(&script)),
            uses,
        );
        (script, port)
    }

    /// A port whose stand-in is already scripted.
    fn port_of_script(
        what: &str,
        uses: Use,
        to_read: Vec<TcsResult<Vec<u8>>>,
    ) -> (Arc<Mutex<Script>>, EndpointPort) {
        let (script, port) = port(what, uses);
        script.lock().expect("the script").to_read = to_read;
        (script, port)
    }

    /// The shipped example, as the routing sees it.
    fn thermostat() -> Routes {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        Routes::of(&Graph::load(root.join("tests/manual/thermostat.yaml")).expect("it reads"))
    }

    /// The links say which way each node is used.
    #[test]
    fn the_links_say_which_way_a_node_is_used() {
        let routes = thermostat();

        // A sensor's output goes somewhere and nothing arrives at it.
        assert_eq!(Use::of(&routes, "temp_fore"), Ok(Use::Read));
        // The heaters are the other way round.
        assert_eq!(Use::of(&routes, "heaters"), Ok(Use::Driven));
        // And the ground link is both: telemetry goes to it, and the
        // setpoint is commanded through it. The second half of that is the
        // binding and not a link -- see the commanded case below.
        assert_eq!(Use::of(&routes, "moc"), Ok(Use::Both));
        // A log takes and produces nothing of its own.
        assert_eq!(Use::of(&routes, "housekeeping"), Ok(Use::Driven));

        assert!(Use::Read.produces() && !Use::Read.accepts());
        assert!(Use::Driven.accepts() && !Use::Driven.produces());
        assert!(Use::Both.accepts() && Use::Both.produces());
    }

    /// A MOC link commands arrive on is read, though no link leaves it.
    ///
    /// The case a table of links alone gets wrong. A command sets an
    /// attribute rather than travelling a link, so the only statement that
    /// anything arrives from the ground is the `commands` section -- and a
    /// MOC link judged by its links would be opened as something only
    /// written, with the setpoint never arriving and nothing saying so.
    #[test]
    fn a_commanded_ground_link_is_read_as_well() {
        use tcslibgs::graph::{CommandBinding, Destinations, Link, Node};

        let node = |name: &str, node_type: &str| Node {
            name: name.to_string(),
            node_type: node_type.to_string(),
            group: None,
            attributes: Default::default(),
            editor: None,
        };

        let graph = Graph {
            description: None,
            // What tcspecial is configured with is nothing to do with the
            // routing, the ports or the schedule: these tests build a graph
            // by hand and the checking that requires a section is not on
            // their path.
            tcspecial: None,
            groups: Vec::new(),
            nodes: vec![
                node("sensor", "i2c"),
                node("thermostat", "compute"),
                node("moc", "moc"),
            ],
            links: vec![Link {
                from: "sensor".to_string(),
                to: Destinations::of(vec!["moc".to_string()]),
                comment: None,
            }],
            commands: vec![CommandBinding {
                from: "moc".to_string(),
                sets: "thermostat.setpoint".to_string(),
            }],
            files: Vec::new(),
            homes: Default::default(),
        };
        let routes = Routes::of(&graph);

        assert_eq!(
            Use::of(&routes, "moc"),
            Ok(Use::Both),
            "a commanded ground link is read as well as written"
        );
        // And what the command sets, which is how the fabric will find the
        // thermostat's setpoint.
        let commanded = routes.commands_on("moc");
        assert_eq!(commanded.len(), 1);
        assert_eq!(commanded[0].node, "thermostat");
        assert_eq!(commanded[0].attribute, "setpoint");
    }

    /// A node no link touches is refused, and the complaint says why.
    #[test]
    fn a_node_no_link_touches_is_refused() {
        let routes = thermostat();
        let said = Use::of(&routes, "left_over").expect_err("a node with no links");
        assert!(
            said.contains("left_over") && said.contains("nothing for it to do"),
            "the complaint should name the node and say why: {said}"
        );
    }

    /// What a device produces is taken, and nothing to read is not a failure.
    #[test]
    fn a_reading_is_taken_and_quiet_is_not_a_failure() {
        let (script, mut port) = port("temp_fore", Use::Read);
        script.lock().expect("the script").to_read = vec![Ok(b"293.15".to_vec())];

        let mut buffer = [0u8; 64];
        let got = port.take(&mut buffer).expect("the reading");
        assert_eq!(&buffer[..got], b"293.15");

        // And the pass after it, with the device not due.
        assert_eq!(
            port.take(&mut buffer),
            Ok(0),
            "a device with nothing to say has not failed"
        );
    }

    /// A read that fails is reported, and names the node.
    #[test]
    fn a_read_that_fails_is_reported() {
        let (script, mut port) = port("temp_fore", Use::Read);
        script.lock().expect("the script").to_read =
            vec![Err(TcsError::Endpoint("the bus did not answer".to_string()))];

        let said = port
            .take(&mut [0u8; 64])
            .expect_err("a bus that did not answer");
        assert!(
            said.contains("temp_fore") && said.contains("did not answer"),
            "the complaint should name the node and carry the reason: {said}"
        );
    }

    /// What arrives at a driven node is written whole.
    #[test]
    fn a_command_is_written_whole() {
        let (script, mut port) = port("heaters", Use::Driven);

        assert_eq!(port.give(None, b"1"), Ok(()));
        assert_eq!(script.lock().expect("the script").written, vec![b"1".to_vec()]);
    }

    /// A write that moved some of the bytes is a failure, and says how many.
    ///
    /// The quiet alternative is available here and is the reason this is
    /// tested: the endpoint answered `Ok`, and a port that passed that on
    /// would have the fabric count a truncated datum as delivered.
    #[test]
    fn a_part_written_datum_is_refused() {
        let (script, mut port) = port("moc", Use::Both);
        script.lock().expect("the script").write_answers = vec![Ok(3)];

        let said = port
            .give(None, b"293.15")
            .expect_err("three bytes of a six-byte reading");
        assert!(
            said.contains("moved 3 of 6") && said.contains("not a smaller datum"),
            "the complaint should say what was lost: {said}"
        );
    }

    /// A write that moved nothing is a failure and not a success.
    #[test]
    fn a_write_of_nothing_is_a_failure() {
        let (script, mut port) = port("moc", Use::Both);
        script.lock().expect("the script").write_answers = vec![Ok(0)];

        let said = port.give(None, b"293.15").expect_err("a datum dropped");
        assert!(
            said.contains("moved none of them") && said.contains("dropped"),
            "the complaint should say the datum is gone: {said}"
        );
    }

    /// A write that failed outright carries the endpoint's own reason.
    #[test]
    fn a_write_that_fails_carries_the_reason() {
        let (script, mut port) = port("heaters", Use::Driven);
        script.lock().expect("the script").write_answers = vec![Err(TcsError::Endpoint(
            "cannot drive the lines of /dev/gpiochip0".to_string(),
        ))];

        let said = port.give(None, b"1").expect_err("a chip that refused");
        assert!(
            said.contains("heaters") && said.contains("/dev/gpiochip0"),
            "the complaint should name the node and the chip: {said}"
        );
        assert_eq!(
            said.matches("heaters").count(),
            1,
            "the node is named once: every endpoint names the node it belongs \
             to, and so does the line this is reported on -- {said}"
        );
    }

    /// Each direction refuses what the configuration does not use it for.
    #[test]
    fn a_node_refuses_the_direction_it_is_not_used_in() {
        let (_, mut sensor) = port("temp_fore", Use::Read);
        let said = sensor.give(None, b"1").expect_err("a write to a sensor");
        assert!(
            said.contains("read and not written"),
            "the complaint should say which way it is used: {said}"
        );
        assert!(!sensor.accepts() && sensor.produces());

        let (_, mut heaters) = port("heaters", Use::Driven);
        let said = heaters
            .take(&mut [0u8; 64])
            .expect_err("a read of the heaters");
        assert!(
            said.contains("driven and not read"),
            "the complaint should say which way it is used: {said}"
        );
        assert!(heaters.accepts() && !heaters.produces());
    }

    /// A named input arriving at a device is a mis-wiring, not a datum.
    #[test]
    fn a_named_input_on_a_device_is_refused() {
        let (script, mut port) = port("heaters", Use::Driven);

        let said = port
            .give(Some("in0"), b"1")
            .expect_err("a computation's input name on a device");
        assert!(
            said.contains("in0") && said.contains("no named inputs"),
            "the complaint should name the input: {said}"
        );
        assert!(
            script.lock().expect("the script").written.is_empty(),
            "nothing should have been written"
        );
    }

    /// A read asks for what the part answers in, not what the buffer holds.
    #[test]
    fn a_read_asks_for_what_the_node_says() {
        let (script, port) = port("temp_fore", Use::Read);
        let mut port = port.reading(2);
        script.lock().expect("the script").to_read = vec![Ok(b"293.15".to_vec())];

        // A sensor that answers in two bytes, read into a buffer with room
        // for sixty-four: two is what is asked for.
        let mut buffer = [0u8; 64];
        let got = port.take(&mut buffer).expect("a reading");
        assert_eq!(got, 2, "a bus master clocks what it asks for and no more");

        // And the buffer is the other bound: a caller with room for one byte
        // gets one, whatever the node says.
        let (script, port) = port_of_script("temp_fore", Use::Read, vec![Ok(b"293.15".to_vec())]);
        let mut port = port.reading(2);
        let got = port.take(&mut [0u8; 1]).expect("a reading");
        assert_eq!(got, 1);
        assert!(script.lock().expect("the script").to_read.is_empty());
    }

    /// A node of the shipped example is opened from its configuration.
    ///
    /// As far as a test without hardware can go: the example's sensors are on
    /// an I2C bus and a SPI bus that are not on this machine, so what is
    /// checked is that the attempt is the right one -- the complaint names the
    /// device the file named -- and that a node the file describes wrongly
    /// never gets as far as opening anything.
    #[test]
    fn a_node_is_opened_from_its_configuration() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let graph = Graph::load(root.join("tests/manual/thermostat.yaml")).expect("it reads");
        let routes = Routes::of(&graph);

        let said = match EndpointPort::open(graph.node("temp_fore").expect("a sensor"), &routes) {
            Err(why) => why,
            Ok(_) => {
                // A machine with an I2C bus 1 and something at 0x48 on it.
                // Nothing to complain about, and nothing more to test here.
                return;
            }
        };
        assert!(
            said.contains("/dev/i2c-1"),
            "the complaint should name the bus the file named: {said}"
        );

        // A computation is not opened at all, and says so before anything is
        // reached for.
        let said = match EndpointPort::open(
            graph.node("thermostat").expect("the thermostat"),
            &routes,
        ) {
            Err(why) => why,
            Ok(_) => panic!("a computation was opened as a device"),
        };
        assert!(said.contains("not something that is opened"), "{said}");
    }

    /// A node with nothing to say does not hold up the pass.
    ///
    /// The failure this prevents is total: a device file is opened as a file,
    /// so a read of one with nothing in it does not return, and a pass that
    /// blocked there would never end -- the group that was due unfinished,
    /// every other group starved, and a stop unnoticed because a stop is
    /// noticed between passes. A named pipe with nothing in it is exactly
    /// that endpoint.
    #[test]
    fn a_quiet_node_does_not_block_the_pass() {
        use std::ffi::CString;
        use tcslibgs::DeviceConfig;

        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("pipe");
        let name = CString::new(path.to_str().expect("a path")).expect("a name");
        // SAFETY: a path in a directory this test owns, and a mode.
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0, "a named pipe");

        let config = EndpointConfig::Device(DeviceConfig {
            path: path.to_string_lossy().to_string(),
        });
        let mut port =
            EndpointPort::connect("sensor", &config, Use::Read).expect("the pipe opens");

        // Nothing has been written, and this returns: without the readiness
        // question in take, this test would hang rather than fail.
        let mut buffer = [0u8; 64];
        assert_eq!(port.take(&mut buffer), Ok(0), "nothing to say is not a wait");

        // And when there is something, it is read.
        let mut writer = EndpointPort::connect("writer", &config, Use::Both)
            .expect("the other end");
        assert_eq!(writer.give(None, b"293.15"), Ok(()));
        let got = port.take(&mut buffer).expect("the reading");
        assert_eq!(&buffer[..got], b"293.15");
    }

    /// A real endpoint, opened by the factory, moves bytes both ways.
    ///
    /// The one test here that opens something. Everything else is against the
    /// traits, which is where this module's decisions are -- but `connect`
    /// itself is a line of delegation that no trait test would exercise, and
    /// a port that cannot open anything is of no use however well it
    /// delegates. A named pipe stands in for the device: it needs no
    /// hardware, no privileges and no network, and it is a device file as far
    /// as the endpoint is concerned.
    #[test]
    fn a_device_file_opened_by_the_factory_carries_both_ways() {
        use std::ffi::CString;
        use tcslibgs::DeviceConfig;

        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("pipe");
        let name = CString::new(path.to_str().expect("a path")).expect("a name");
        // SAFETY: a path in a directory this test owns, and a mode.
        assert_eq!(
            unsafe { libc::mkfifo(name.as_ptr(), 0o600) },
            0,
            "a named pipe: {}",
            std::io::Error::last_os_error()
        );

        let config = EndpointConfig::Device(DeviceConfig {
            path: path.to_string_lossy().to_string(),
        });
        let mut port = EndpointPort::connect("loopback", &config, Use::Both)
            .expect("the pipe opens");

        // Written and read back through the kernel, which is as much of a
        // device as a pipe can be.
        assert_eq!(port.give(None, b"293.15"), Ok(()));

        let mut buffer = [0u8; 64];
        let got = port.take(&mut buffer).expect("what was written");
        assert_eq!(&buffer[..got], b"293.15");

        // And the descriptor a waiting loop would poll.
        assert!(port.io_fd() >= 0);
    }

    /// A computation holds its result until it is taken.
    #[test]
    fn a_computation_has_a_result_when_it_has_enough() {
        use crate::node::vehicle::compute::mean::Mean;
        use tcslibgs::node::vehicle::compute::MeanParams;

        let mut port = ComputePort::of(
            "mean_temp",
            Box::new(Mean::of(MeanParams {
                inputs: vec!["in0".to_string(), "in1".to_string()],
            })),
        );
        assert_eq!(port.function(), "mean");

        // The first arrival is not a result: a mean of one of two sensors
        // would be an average of a reading and nothing.
        assert_eq!(port.give(Some("in0"), b"290.0"), Ok(()));
        assert!(!port.ready(), "a mean of one input is not an average");
        assert_eq!(port.take(&mut [0u8; 64]), Ok(0));

        // The second is.
        assert_eq!(port.give(Some("in1"), b"292.0"), Ok(()));
        assert!(port.ready());

        let mut buffer = [0u8; 64];
        let got = port.take(&mut buffer).expect("the average");
        assert_eq!(&buffer[..got], b"291");

        // And it is gone once taken, rather than being handed out again.
        assert!(!port.ready());
        assert_eq!(port.take(&mut buffer), Ok(0));
    }

    /// A null node passes what arrives through, in order and unchanged.
    ///
    /// Both streams through the one node: what came from the payload side and
    /// what came from the ground are both passed on, in the order they
    /// arrived. Which is also why the arrivals are not labelled here -- a
    /// terminal is resolved where the routing table is built and never reaches
    /// a port, so a null node is handed data and not a name.
    #[test]
    fn a_null_node_carries_what_arrives_through() {
        let mut port = NullPort::of("through");

        assert!(!port.ready(), "nothing has arrived");
        assert_eq!(port.take(&mut [0u8; 64]), Ok(0));

        assert_eq!(port.give(None, b"290.0"), Ok(()));
        assert_eq!(port.give(None, b"go\n"), Ok(()));
        assert!(port.ready());

        // In order, and whole.
        let mut buffer = [0u8; 64];
        let got = port.take(&mut buffer).expect("the first");
        assert_eq!(&buffer[..got], b"290.0");
        let got = port.take(&mut buffer).expect("the second");
        assert_eq!(&buffer[..got], b"go\n");

        // And gone once taken, rather than handed out again.
        assert!(!port.ready());
        assert_eq!(port.take(&mut buffer), Ok(0));

        // A datum that will not fit is kept rather than cut down: part of a
        // value is not a smaller value.
        assert_eq!(port.give(None, b"2901234"), Ok(()));
        let said = port.take(&mut [0u8; 4]).expect_err("it does not fit");
        assert!(said.contains("through") && said.contains("7 bytes"), "{said}");
        assert!(port.ready(), "the datum is still here");
        let got = port.take(&mut buffer).expect("with room");
        assert_eq!(&buffer[..got], b"2901234");
    }

    /// A datum a computation cannot use is refused, naming both.
    #[test]
    fn a_datum_a_computation_cannot_use_is_refused() {
        use crate::node::vehicle::compute::mean::Mean;
        use tcslibgs::node::vehicle::compute::MeanParams;

        let mut port = ComputePort::of(
            "mean_temp",
            Box::new(Mean::of(MeanParams {
                inputs: vec!["in0".to_string(), "in1".to_string()],
            })),
        );

        let said = port
            .give(Some("in0"), b"29x.15")
            .expect_err("a reading that is not a number");
        assert!(
            said.contains("mean_temp") && said.contains("mean"),
            "the complaint should name the node and the computation: {said}"
        );
    }

    /// A result that will not fit is reported and kept, not cut down.
    #[test]
    fn a_result_too_large_for_the_buffer_is_kept() {
        use crate::node::vehicle::compute::mean::Mean;
        use tcslibgs::node::vehicle::compute::MeanParams;

        let mut port = ComputePort::of(
            "mean_temp",
            Box::new(Mean::of(MeanParams {
                inputs: vec!["in0".to_string(), "in1".to_string()],
            })),
        );
        port.give(Some("in0"), b"293.15").expect("a reading");
        port.give(Some("in1"), b"293.15").expect("the other reading");
        assert!(port.ready(), "the set is complete");

        let said = port.take(&mut [0u8; 2]).expect_err("a buffer too small");
        assert!(
            said.contains("not a smaller value"),
            "the complaint should say why it was not truncated: {said}"
        );

        // Still there, for a caller with room for it.
        assert!(port.ready(), "the result should not have been thrown away");
        let mut buffer = [0u8; 64];
        let got = port.take(&mut buffer).expect("the result");
        assert_eq!(&buffer[..got], b"293.15");
    }

    /// Every source of a log writes into the one log, in order.
    #[test]
    fn a_log_takes_what_several_nodes_produced() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut port = LogPort::open(&Node {
            name: "housekeeping".to_string(),
            node_type: "log".to_string(),
            group: None,
            attributes: [
                (
                    "directory".to_string(),
                    tcslibgs::Scalar::of(where_.path().to_str().expect("a path")),
                ),
                ("format".to_string(), tcslibgs::Scalar::of("variable_ts_rc")),
            ]
            .into_iter()
            .collect(),
            editor: None,
        })
        .expect("a log");

        // Three readings and a decision, arriving as the fabric delivers
        // them: unnamed, because a log has no named inputs.
        for record in [&b"289.15"[..], b"289.15", b"287.3188", b"1"] {
            assert_eq!(port.give(None, record), Ok(()));
        }
        // And one that arrives with a name anyway, which a log takes: an
        // arrival cannot be ambiguous to a node that writes everything down.
        assert_eq!(port.give(Some("in0"), b"291.0"), Ok(()));
        port.flush().expect("the log is flushed");

        assert!(!port.produces(), "a log is where data stops");
        assert!(port.accepts());
        let said = port
            .take(&mut [0u8; 64])
            .expect_err("a log read back into the vehicle");
        assert!(
            said.contains("housekeeping") && said.contains("where data stops"),
            "{said}"
        );
    }

    /// A record the log's format cannot hold is refused, and named.
    #[test]
    fn a_log_refuses_what_its_format_cannot_hold() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut port = LogPort::open(&Node {
            name: "fixed_log".to_string(),
            node_type: "log".to_string(),
            group: None,
            attributes: [
                (
                    "directory".to_string(),
                    tcslibgs::Scalar::of(where_.path().to_str().expect("a path")),
                ),
                ("format".to_string(), tcslibgs::Scalar::of("fixed:4")),
            ]
            .into_iter()
            .collect(),
            editor: None,
        })
        .expect("a log");

        assert_eq!(port.give(None, b"abcd"), Ok(()));
        let said = port
            .give(None, b"289.15")
            .expect_err("six bytes in a four-byte log");
        assert!(said.contains("fixed_log"), "{said}");
    }

    /// A device does not produce because something was delivered to it.
    ///
    /// The distinction [`Port::ready`] draws: an endpoint has data when its
    /// device has data, which a write to it does not change.
    #[test]
    fn an_endpoint_is_never_ready_as_a_consequence_of_a_delivery() {
        let (script, mut port) = port("moc", Use::Both);
        script.lock().expect("the script").to_read = vec![Ok(b"293.15".to_vec())];

        assert_eq!(port.give(None, b"1"), Ok(()));
        assert!(
            !port.ready(),
            "a reading waiting on a socket is not a result of what was sent to it"
        );
    }

    /// A device that must be asked is asked after each read.
    #[test]
    fn a_triggered_device_is_asked_after_it_is_read() {
        let (script, port) = port("temp_bay", Use::Read);
        let mut port = port.triggered_by(b"R".to_vec());
        script.lock().expect("the script").to_read = vec![Ok(b"42".to_vec()), Ok(b"43".to_vec())];

        let mut buffer = [0u8; 64];

        // A pass takes what is there and then asks. On a real device the
        // first pass takes nothing, the question not having been asked yet;
        // the stand-in answers at once, which is what lets the order be seen.
        let got = port.take(&mut buffer).expect("a pass");
        assert_eq!(&buffer[..got], b"42");
        assert_eq!(
            script.lock().expect("the script").written,
            vec![b"R".to_vec()],
            "the device should have been asked once"
        );

        // And the pass after it takes the answer to that question and asks
        // again.
        let got = port.take(&mut buffer).expect("a pass");
        assert_eq!(&buffer[..got], b"43");
        assert_eq!(script.lock().expect("the script").written.len(), 2);
    }

    /// A device that cannot be asked is reported, read or no read.
    ///
    /// The reading in hand is the last one this node will produce, so a pass
    /// that took one and could not ask for the next has not succeeded.
    #[test]
    fn a_trigger_that_cannot_be_written_is_reported() {
        let (script, port) = port("temp_bay", Use::Read);
        let mut port = port.triggered_by(b"R".to_vec());
        script.lock().expect("the script").to_read = vec![Ok(b"42".to_vec())];
        script.lock().expect("the script").write_answers = vec![Err(TcsError::Endpoint(
            "the bus will not take it".to_string(),
        ))];

        let said = port
            .take(&mut [0u8; 64])
            .expect_err("a device that cannot be asked");
        assert!(
            said.contains("trigger") && said.contains("will not take it"),
            "the complaint should say the asking failed: {said}"
        );
    }
}
