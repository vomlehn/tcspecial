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
use std::time::{Duration, Instant};

use nix::poll::{poll, PollFd, PollFlags};

use tcsapp::parent::ReadInto;
use tcslibgs::graph::reach::reach_of;
use tcslibgs::graph::terminals::Stream;
use tcslibgs::graph::Node;
use tcslibgs::node::app;
use tcslibgs::{EndpointConfig, TcsError, TcsResult};

use crate::node::payload::app_all_pl::AppAllPl;
use crate::node::payload::app_pl::AppPl;
use crate::node::vehicle::app_all_veh::AppAllVeh;
use crate::node::vehicle::app_moc_veh::AppMocVeh;
use crate::node::vehicle::app_payload_veh::AppPayloadVeh;
use crate::node::vehicle::compute::{self, Function, Out};
use crate::node::vehicle::log::LogNode;
use crate::node::vehicle::ping::Answers;
use crate::node::vehicle::tee_moc::Through;

use crate::runtime::frame::Framer;
use crate::runtime::node_state::Abandon;

use crate::endpoint::{
    bind_endpoint_pair, connect_endpoint_pair, driven_endpoint_pair, EndpointReadable,
    EndpointWritable, WaitResult,
};
use crate::runtime::fabric::{Arriving, Port, Taken};
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
    /// How long to wait between asking and reading, where the node says.
    ///
    /// See [`EndpointPort::waiting`].
    read_delay: Option<Duration>,
    /// How many bytes one read asks for, where the node says.
    ///
    /// See [`EndpointPort::reading`].
    read_size: Option<usize>,
    /// Where one read of a line ends, and what has arrived of the read under
    /// way.
    ///
    /// See [`EndpointPort::framed_by`].
    framer: Option<Framer>,
    /// What the wait between asking and reading is abandoned for.
    ///
    /// The one wait this port makes that is a wait rather than a look: see
    /// [`Abandon`], and [`EndpointPort::abandoned_for`].
    abandon: Abandon,
    /// Whether this is a device held at what it was last told.
    ///
    /// See [`EndpointPort::is_a_device`] and [`Port::holds`].
    holds: bool,
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

    /// Reach out to a payload that is only driven, as one node.
    ///
    /// For a heater or a valve: see [`crate::endpoint::driven_endpoint_pair`]
    /// for why the ends swap roles when nothing is read.
    pub fn driving(what: &str, config: &EndpointConfig, uses: Use) -> TcsResult<EndpointPort> {
        let (reader, writer) = driven_endpoint_pair(what, config)?;
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
            read_delay: None,
            read_size: None,
            framer: None,
            abandon: Abandon::nothing(),
            holds: false,
        }
    }

    /// Say that this port is a device, which a re-asserted decision is sent
    /// to again.
    ///
    /// Set where the node type is known, which is [`EndpointPort::open`]: a
    /// payload is a device and a MOC link is not, and the two are the same
    /// kind of socket otherwise. Off by default, so a port built by hand --
    /// a test's, a ground link's -- is not one by accident.
    pub fn is_a_device(mut self) -> EndpointPort {
        self.holds = true;
        self
    }

    /// Open the node a configuration describes.
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
    pub fn open(node: &Node, routes: &Routes, abandon: &Abandon) -> Result<EndpointPort, String> {
        let uses = Use::of(routes, &node.name)?;
        let reach = reach_of(node)?;

        let opened = if node.node_type == "moc" {
            EndpointPort::bind(&node.name, &reach.endpoint, uses)
        } else if uses == Use::Driven {
            // Nothing is read from this one, so there is nobody to learn from:
            // what the node says is where the device is, and this sends there.
            EndpointPort::driving(&node.name, &reach.endpoint, uses)
        } else {
            EndpointPort::connect(&node.name, &reach.endpoint, uses)
        };
        let mut port = opened.map_err(|e| format!("{e}"))?;

        // A payload is a thing in the world that stays as it was put: a
        // decision written to it holds until another one is. A MOC link is
        // not, so a re-assertion is not sent down it.
        if node.is_payload() && uses.accepts() {
            port = port.is_a_device();
        }

        if let Some(size) = reach.read_size {
            port = port.reading(size as usize);
        }
        if let Some(rule) = reach.stream {
            port = port.framed_by(rule);
        }
        if let Some(trigger) = reach.trigger {
            port = port.triggered_by(trigger);
        }
        if let Some(delay) = reach.read_delay {
            port = port.waiting(delay);
        }
        port = port.abandoned_for(abandon.clone());

        // What the payload starts in, written before anything else is: a
        // heater the vehicle has not yet decided about is off rather than in
        // whatever state the hardware came up in. A write that fails is a
        // node that did not open -- at startup the vehicle is refused, and on
        // a START_NODE the ground is told why -- because a payload whose
        // starting state nobody knows is the thing an init exists to prevent.
        if let Some(init) = reach.init {
            port.write_all(&init, "init").map_err(|why| {
                format!("{why}; the node is not started without what init says to set it to")
            })?;
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

    /// End each read of this line where the node's stream rule says.
    ///
    /// A line delivers bytes with no frame boundaries of its own, so what one
    /// read of it is -- a sentence ended by a newline, a fixed record of
    /// twelve bytes -- is the node's to say: see [`crate::runtime::frame`].
    /// Without this, a pass would take whatever bytes happened to be waiting,
    /// which is half a sentence as often as a whole one.
    pub fn framed_by(mut self, rule: tcslibgs::endpoint_config::StreamParams) -> EndpointPort {
        self.framer = Some(Framer::new(rule));
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

    /// How long to wait between asking and reading.
    ///
    /// What turns read-then-ask into ask-wait-read. With a delay, a pass
    /// writes the trigger, waits the time the part's interface document says
    /// it takes to answer, and reads the answer it asked for -- so the
    /// reading is of this moment rather than of the last interval. Without
    /// one, the order above stands and nothing waits.
    ///
    /// The delay is the part's and not the schedule's, so it belongs to a
    /// node whatever reads it: a payload in a group that answers in two
    /// milliseconds answers in two milliseconds however often the group is
    /// read.
    pub fn waiting(mut self, read_delay: Duration) -> EndpointPort {
        self.read_delay = Some(read_delay);
        self
    }

    /// What ends the wait between asking and reading, besides the clock.
    ///
    /// The node's own, so that disabling the node abandons an exchange it is
    /// in the middle of rather than letting the delay run out first. A port
    /// given none waits out its delay, which is what a port in a test does
    /// and what a port nothing can disable does.
    pub fn abandoned_for(mut self, abandon: Abandon) -> EndpointPort {
        self.abandon = abandon;
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
    /// Read what is there, and nothing if nothing is: never waiting.
    ///
    /// Asked before read, and this is not an optimisation. Some endpoints
    /// are opened blocking -- a device file is opened as a file -- so a read
    /// of one with nothing in it does not return. A pass that blocked there
    /// would be a pass that never ends: the group that was due is never
    /// finished, every other group starves, and a stop is not noticed because
    /// a stop is noticed between passes. So the descriptor is asked whether
    /// there is anything, with no waiting at all, and a node with nothing to
    /// say is a read of no bytes -- which is the ordinary answer from a device
    /// that is not due.
    /// One read of this node, as far as `want` bytes: what is there, or for a
    /// line, the next read its stream rule has ended.
    ///
    /// A read already ended is handed over before anything more is asked of
    /// the line, and no more is asked than would fill the read under way:
    /// see [`crate::runtime::frame`]. Nought bytes is a read still under way,
    /// which a pass treats as a node with nothing to say yet.
    fn read_one(&mut self, buffer: &mut [u8], want: usize) -> Result<usize, String> {
        let Some(mut framer) = self.framer.take() else {
            return self.read_what_is_there(&mut buffer[..want]);
        };
        let read = self.read_framed(&mut framer, buffer, want);
        self.framer = Some(framer);
        read
    }

    fn read_framed(
        &mut self,
        framer: &mut Framer,
        buffer: &mut [u8],
        want: usize,
    ) -> Result<usize, String> {
        if let Some(ended) = framer.frame(Instant::now(), buffer) {
            return Ok(ended);
        }
        // A prefixed frame is asked for twice: its prefix, and then as much
        // as the prefix gives. Both in one pass, or every frame would wait a
        // group's interval between the two. A read with no prefix is asked
        // for once, since an ask that is answered in full fills it.
        let mut ended = None;
        for _ in 0..2 {
            let ask = framer.room().min(want);
            if ask == 0 {
                break;
            }
            let got = self.read_what_is_there(&mut buffer[..ask])?;
            framer.took(&buffer[..got], Instant::now());
            ended = framer.frame(Instant::now(), buffer);
            if ended.is_some() || got < ask {
                break;
            }
        }
        self.say_what_was_let_go(framer);
        Ok(ended.unwrap_or(0))
    }

    /// Say that prefixed frames were let go, which is a line out of step or
    /// a payload sending what its configuration does not expect: either way
    /// readings that never reach the ground, and nothing else would say so.
    /// At most a frame's worth of warnings a timeout or a max_length apart,
    /// so a line that is wrong is heard without drowning the log.
    fn say_what_was_let_go(&self, framer: &mut Framer) {
        let let_go = framer.let_go();
        if let_go.too_long > 0 {
            log::warn!(
                "{}: a frame's length_prefix gave more than max_length holds, so \
                 the frame was let go. Raise max_length, or check the prefix the \
                 payload sends against length_prefix",
                self.what
            );
        }
        if let_go.cut_short > 0 {
            log::warn!(
                "{}: a frame was still short of the length its prefix gave when \
                 read_timeout ran out, so it was let go and the next byte is read as \
                 a prefix. Check that read_timeout is longer than a frame takes to \
                 arrive",
                self.what
            );
        }
    }

    fn read_what_is_there(&mut self, buffer: &mut [u8]) -> Result<usize, String> {
        match self.ready_to_read() {
            Ok(true) => self
                .reader
                .read(buffer)
                .map_err(|e| format!("{}: the read failed: {}", self.what, self.plainly(e))),
            Ok(false) => Ok(0),
            Err(why) => Err(why),
        }
    }

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
    /// What this device had, on no stream in particular.
    ///
    /// A device has one source terminal, so which way its data goes is the
    /// link's business and not its own: see [`Taken`].
    fn take(&mut self, buffer: &mut [u8]) -> Result<Taken, String> {
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

        // A node that is asked and says how long it takes to answer is
        // asked here, before the read, and waited for: the answer the pass
        // takes is then the answer to the question the pass asked. A node
        // with no delay is asked after the read instead -- see below -- so
        // that nothing waits on a part that has not been told how long it
        // needs.
        if let Some(delay) = self.read_delay {
            if let Some(trigger) = self.trigger.take() {
                let asked = self.write_all(&trigger, "trigger");
                self.trigger = Some(trigger);
                asked?;
                // The delay, or the node being disabled, whichever comes
                // first. Abandoned rather than read anyway: the node has been
                // told to stop, and what is on its interface is an answer to
                // a question that is no longer being asked. Nothing is
                // reported, a node the ground stopped being the
                // configuration working as asked.
                if self.abandon.wait(delay) {
                    return Ok(Taken::NOTHING);
                }
            }
        }

        let got = self.read_one(buffer, want);

        // Asked after being read, and asked whether or not the read found
        // anything: a device that answered nothing is a device that most
        // wants asking again. The failure to ask is reported even where the
        // read succeeded, because a reading taken now and no question asked
        // is the last reading this node will ever produce.
        //
        // A node with a read delay was asked above, before the read, so it is
        // not asked again here: one question per pass either way.
        let ask = match self.read_delay {
            Some(_) => None,
            None => self.trigger.take(),
        };
        match (got, ask) {
            (got, None) => got.map(Taken::of),
            (got, Some(trigger)) => {
                let asked = self.write_all(&trigger, "trigger");
                self.trigger = Some(trigger);
                match (got, asked) {
                    (Ok(n), Ok(())) => Ok(Taken::of(n)),
                    (Ok(_), Err(why)) => Err(why),
                    // A read that failed is the worse news of the two, and
                    // the one an operator is looking for.
                    (Err(why), _) => Err(why),
                }
            }
        }
    }

    fn take_unasked(&mut self, buffer: &mut [u8]) -> Result<Taken, String> {
        if !self.uses.produces() {
            return self.take(buffer);
        }
        let want = self
            .read_size
            .map_or(buffer.len(), |size| size.min(buffer.len()));
        self.read_one(buffer, want).map(Taken::of)
    }

    fn ask(&mut self) -> Result<(), String> {
        match self.trigger.take() {
            None => Ok(()),
            Some(trigger) => {
                let asked = self.write_all(&trigger, "trigger");
                self.trigger = Some(trigger);
                asked
            }
        }
    }

    fn give(&mut self, arriving: Arriving<'_>, data: &[u8]) -> Result<(), String> {
        let input = arriving.input;
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

    fn holds(&self) -> bool {
        self.holds
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
/// How many results a computation's queue has room for before it would grow:
/// normally one is waiting at most -- see `ComputePort::ready` -- so this is
/// room to spare.
const RESULTS_AT_ONCE: usize = 4;

pub struct ComputePort {
    /// The node's name in the configuration, which every complaint names.
    what: String,
    /// What it computes.
    function: Box<dyn Function>,
    /// Results waiting to be taken, and whether each is a re-assertion.
    ///
    /// Normally at most one: the fabric takes a result as soon as the node
    /// says it has one. A queue rather than a slot all the same, because the
    /// alternative when a second result arrives before the first is taken is
    /// to overwrite it -- a reading dropped where nothing says so, which is
    /// the one outcome this project will not have.
    ///
    /// The flag travels with the bytes rather than being asked for
    /// afterwards: by the time a result is taken the function has moved on,
    /// and a question answered about the wrong result is worse than no
    /// question.
    ///
    /// Made with room for a few results when the node is, and the results
    /// are held in place, so that a computation's result is queued and taken
    /// without anything being allocated.
    ready: VecDeque<(Out, bool)>,
}

impl ComputePort {
    /// Hold a computation that is already built.
    pub fn of(what: &str, function: Box<dyn Function>) -> ComputePort {
        ComputePort {
            what: what.to_string(),
            function,
            ready: VecDeque::with_capacity(RESULTS_AT_ONCE),
        }
    }

    /// Build what this node's configuration asks for.
    ///
    /// `inputs` is what the routing found bound to the node, which a
    /// computation over several of them needs: a mean cannot know it is
    /// waiting for three until something tells it how many links arrive.
    pub fn make(node: &Node, inputs: &[&str], abandon: &Abandon) -> Result<ComputePort, String> {
        Ok(ComputePort::of(
            &node.name,
            compute::make(node, inputs, abandon)?,
        ))
    }

    /// What this node computes, for a log line that says what is running.
    pub fn function(&self) -> &str {
        self.function.name()
    }
}

impl Port for ComputePort {
    /// What this computation worked out, on no stream in particular.
    ///
    /// A result is one result however many ways the configuration sends it: a
    /// thermostat's answer goes to the heaters and to the ground, and neither
    /// of those is the stream it is "on". See [`Taken`].
    fn take(&mut self, buffer: &mut [u8]) -> Result<Taken, String> {
        let Some((result, again)) = self.ready.pop_front() else {
            return Ok(Taken::NOTHING);
        };

        // A result that will not fit is reported and not cut down to size.
        // Half of a number is a different number, and the queue keeps it so
        // that a caller with a larger buffer can still have it.
        if result.len() > buffer.len() {
            let wanted = result.len();
            self.ready.push_front((result, again));
            return Err(format!(
                "{}: the result of {} is {wanted} bytes and the buffer holds {}:                  part of a value is not a smaller value",
                self.what,
                self.function.name(),
                buffer.len()
            ));
        }

        buffer[..result.len()].copy_from_slice(&result);
        Ok(if again {
            Taken::again(result.len())
        } else {
            Taken::of(result.len())
        })
    }

    fn give(&mut self, arriving: Arriving<'_>, data: &[u8]) -> Result<(), String> {
        match self.function.give(arriving.input, data) {
            Ok(Some(result)) => {
                self.ready.push_back((result, false));
                Ok(())
            }
            // Nothing new. Either there is something the devices this
            // computation drives are held at -- a thermostat's last decision,
            // which goes out again so that one lost datagram does not stand
            // for ever -- or there is not, which is a function waiting for
            // the rest of its inputs. Neither is a failure.
            Ok(None) => {
                if let Some(held) = self.function.held() {
                    self.ready.push_back((held, true));
                }
                Ok(())
            }
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
    pub fn open(node: &Node, segsend: &crate::segsend::SegSend) -> Result<LogPort, String> {
        Ok(LogPort(LogNode::of(node, segsend)?))
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

    fn take(&mut self, _buffer: &mut [u8]) -> Result<Taken, String> {
        Err(format!(
            "{}: nothing is read from a log -- it is where data stops, and a link \
             out of one would be a log read back into the vehicle that wrote it",
            self.0.name()
        ))
    }

    fn give(&mut self, _arriving: Arriving<'_>, data: &[u8]) -> Result<(), String> {
        self.0.record(data)
    }

    fn produces(&self) -> bool {
        false
    }
}

/// A `null-moc` or `null-payload` node, as a node of the graph: what
/// arrives is taken and dropped.
///
/// For a configuration that has to send a stream somewhere and has nowhere
/// for it to go -- a payload's reading nobody is listening to yet, the
/// ground's commands to a payload not yet fitted. Which stream it takes is its
/// terminal's business, in `tcslibgs::graph::terminals`; here, either is
/// dropped alike, and is counted as delivered, because it was.
///
/// Also what a `moc` node is held as where nothing is read from it and it
/// binds nothing: what is sent to the ground goes by each source's own link
/// (see `crate::runtime::system::open_ground_link`), so nothing handed to the
/// node itself has anywhere to go.
pub struct NullPort;

impl NullPort {
    /// A node that drops what it is given.
    pub fn of() -> NullPort {
        NullPort
    }
}

impl Port for NullPort {
    /// Nothing, ever: nothing is kept to hand back.
    fn take(&mut self, _buffer: &mut [u8]) -> Result<Taken, String> {
        Ok(Taken::NOTHING)
    }

    fn give(&mut self, _arriving: Arriving<'_>, _data: &[u8]) -> Result<(), String> {
        Ok(())
    }

    fn produces(&self) -> bool {
        false
    }
}

/// A `tee-moc` or `tee-payload` node, as a node of the graph.
///
/// The thinnest adapter there is, because the node itself holds what there
/// is to hold: see [`crate::node::vehicle::tee_moc::Through`], which is where the
/// queue and the rule about a datum that will not fit live. Here for the
/// reason the log's adapter and the computation's are here -- the questions a
/// port answers are the same whatever is behind it, and answers kept together
/// stay the same.
pub struct TeePort {
    /// The node's name in the configuration, which every complaint names.
    what: String,
    through: Through,
}

impl TeePort {
    /// A tee for this node.
    pub fn of(what: &str) -> TeePort {
        TeePort {
            what: what.to_string(),
            through: Through::new(),
        }
    }
}

impl Port for TeePort {
    /// What arrived, on no stream in particular: so that it goes down every
    /// link leaving the node, the `moc-out` ones and the `payload-out` ones
    /// alike, which is what a tee is for. One stream arrives at a tee, so no
    /// other is mixed into it.
    fn take(&mut self, buffer: &mut [u8]) -> Result<Taken, String> {
        self.through.take(&self.what, buffer).map(Taken::of)
    }

    fn give(&mut self, _arriving: Arriving<'_>, data: &[u8]) -> Result<(), String> {
        self.through.give(data);
        Ok(())
    }

    /// It has something to pass on as a consequence of what arrived.
    ///
    /// A tee produces because something came in, as a computation does, and
    /// never on a schedule of its own: a pass that ended with the delivery
    /// would leave the datum here until something else happened to read the
    /// node.
    fn ready(&self) -> bool {
        self.through.waiting()
    }
}

/// A `ping` node, as a node of the graph.
///
/// As thin as the `null` node's adapter, and for the same reason: the node
/// holds what there is to hold -- see
/// [`crate::node::vehicle::ping::Answers`], which is where the answers wait
/// and where a ping is told from what is not one.
pub struct PingPort {
    /// The node's name in the configuration, which every complaint names.
    what: String,
    answers: Answers,
}

impl PingPort {
    /// A ping node that has answered nothing yet.
    pub fn of(what: &str) -> PingPort {
        PingPort {
            what: what.to_string(),
            answers: Answers::new(),
        }
    }
}

impl Port for PingPort {
    /// The oldest answer, to go wherever this node's links go, which is
    /// groundward: a ping node has one data source.
    fn take(&mut self, buffer: &mut [u8]) -> Result<Taken, String> {
        self.answers.take(&self.what, buffer).map(Taken::of)
    }

    /// A ping, answered as it is handed over: the time is when it arrived.
    fn give(&mut self, _arriving: Arriving<'_>, data: &[u8]) -> Result<(), String> {
        self.answers.give(&self.what, data)
    }

    /// It has something to send as a consequence of what arrived, as a
    /// `null` node does, and never on a schedule of its own.
    fn ready(&self) -> bool {
        self.answers.waiting()
    }
}

/// An app node: a program of the vehicle's own, in the fabric.
///
/// One port for the three kinds, because the fabric holds one port per node
/// and the kind is what the node is: which of the two file interfaces its
/// program was started with, and so which streams it takes and which it
/// produces. See [`crate::node::vehicle::app_payload_veh`] and its two
/// neighbours, which are what this holds, and `tcsapp` for the interfaces
/// themselves.
///
/// Polled rather than ready: an app is another process, so what it makes of a
/// datum arrives when it arrives and not inside the delivery that handed it
/// over. [`Port::ready`] is left false and the pass asks every app every time,
/// which is what `system::is_polled` does for a node in no group -- a read of
/// nothing is the ordinary answer and costs one system call.
#[derive(Debug)]
pub struct AppPort {
    what: String,
    program: Program,
    /// Which stream was taken from last, for a node that has two.
    ///
    /// So that two busy streams take turns: a pass takes one datum from a
    /// node, and an app whose groundward side is always ready would otherwise
    /// never have its vehicleward side read.
    last: Option<Stream>,
    /// Why nothing more will come, once that is so.
    ///
    /// Held so that every pass says the same thing rather than the first one
    /// saying it and the rest saying nothing: the same arrangement
    /// `node::vehicle::compute::process` keeps for a program that has
    /// stopped.
    ended: Option<String>,
}

/// The program, as whichever kind of app node it is.
#[derive(Debug)]
enum Program {
    Payload(AppPayloadVeh),
    Moc(AppMocVeh),
    All(AppAllVeh),
    /// A payload that is never asked: it produces and takes nothing.
    Pl(AppPl),
    /// And one that is asked, which is a payload's own shape.
    AllPl(AppAllPl),
}

impl AppPort {
    /// Start this node's program and hold it.
    ///
    /// Started here, as the configuration is opened, so a program that cannot
    /// be executed is a fault found before the vehicle is doing anything --
    /// the rule a `process` computation is held to as well.
    pub fn open(node: &Node) -> Result<AppPort, String> {
        let params = app::params_of(&node.name, &node.node_type, &node.attributes)?;
        let program = match params.kind {
            app::Kind::PayloadVeh => Program::Payload(AppPayloadVeh::of(&params)?),
            app::Kind::MocVeh => Program::Moc(AppMocVeh::of(&params)?),
            app::Kind::AllVeh => Program::All(AppAllVeh::of(&params)?),
            app::Kind::Pl => Program::Pl(AppPl::of(&params)?),
            app::Kind::AllPl => Program::AllPl(AppAllPl::of(&params)?),
        };
        Ok(AppPort {
            what: node.name.clone(),
            program,
            last: None,
            ended: None,
        })
    }

    /// Whatever has arrived on this stream's interface, without waiting.
    ///
    /// Read straight into `buffer`, so that a packet reaches the fabric with
    /// nothing allocated on the way: see `tcsapp::parent::ReadInto`.
    fn asked(&mut self, stream: Stream, buffer: &mut [u8]) -> Result<ReadInto, String> {
        match (&mut self.program, stream) {
            // What the app writes as MOC data, which it does on the payload
            // interface: see `tcsapp`.
            (Program::Payload(app), Stream::Vehicleward) => app.take_moc_data_now_into(buffer),
            (Program::Moc(app), Stream::Groundward) => app.take_payload_data_now_into(buffer),
            (Program::All(app), Stream::Vehicleward) => app.take_moc_data_now_into(buffer),
            (Program::All(app), Stream::Groundward) => app.take_payload_data_now_into(buffer),
            // A payload's readings, which are groundward whether anything
            // asked for them or not.
            (Program::Pl(app), Stream::Groundward) => app.take_payload_data_now_into(buffer),
            (Program::AllPl(app), Stream::Groundward) => app.take_payload_data_now_into(buffer),
            // A stream this kind does not produce on. Nothing, rather than a
            // refusal: it is not a fault to ask, and `take` asks only what the
            // kind has.
            _ => Ok(ReadInto::Nothing),
        }
    }

    /// Which streams to ask, in the order to ask them.
    fn to_ask(&self) -> &'static [Stream] {
        match (&self.program, self.last) {
            (Program::Payload(_), _) => &[Stream::Vehicleward],
            (Program::Moc(_), _) => &[Stream::Groundward],
            // A payload produces on the one stream, asked or not, so there is
            // nothing to take turns with.
            (Program::Pl(_), _) | (Program::AllPl(_), _) => &[Stream::Groundward],
            // Turn and turn about.
            (Program::All(_), Some(Stream::Groundward)) => {
                &[Stream::Vehicleward, Stream::Groundward]
            }
            (Program::All(_), _) => &[Stream::Groundward, Stream::Vehicleward],
        }
    }
}

impl Port for AppPort {
    /// One datum from one of its interfaces, saying which.
    ///
    /// The stream travels with the bytes because this is the node that needs
    /// it to: an `app_all_veh` writes payload data on the one interface and
    /// MOC data on the other, and delivering a reading down the links leaving
    /// its `moc-out` would send telemetry out as a command. See [`Taken`].
    ///
    /// A packet of no bytes is read and goes no further. The interfaces carry
    /// one -- an app with nothing to say and an app that has said something
    /// empty are different things to `tcsapp` -- but the fabric's own contract
    /// is that nought bytes is nothing to say, and an empty datum has nothing
    /// to deliver.
    fn take(&mut self, buffer: &mut [u8]) -> Result<Taken, String> {
        if let Some(why) = &self.ended {
            return Err(why.clone());
        }

        for stream in self.to_ask() {
            match self.asked(*stream, buffer) {
                Ok(ReadInto::Nothing) => {}
                Ok(ReadInto::Packet(len)) => {
                    self.last = Some(*stream);
                    return Ok(Taken::on(len, *stream));
                }
                // A datum that will not fit is reported and not cut down:
                // part of a packet is not a smaller packet, and the app wrote
                // what it wrote.
                Ok(ReadInto::TooLong(len)) => {
                    return Err(format!(
                        "{}: a packet of {len} bytes arrived and the buffer holds \
                         {}: part of a packet is not a smaller packet",
                        self.what,
                        buffer.len()
                    ));
                }
                Ok(ReadInto::Ended) => {
                    let why = format!(
                        "{}: its program has finished with the {} interface, so \
                         nothing more will come from it",
                        self.what,
                        stream.label()
                    );
                    self.ended = Some(why.clone());
                    return Err(why);
                }
                Err(why) => return Err(format!("{}: {why}", self.what)),
            }
        }
        Ok(Taken::NOTHING)
    }

    /// Hand it a datum, on the interface its stream belongs to.
    fn give(&mut self, arriving: Arriving<'_>, data: &[u8]) -> Result<(), String> {
        if let Some(why) = &self.ended {
            return Err(why.clone());
        }

        let given = match (&mut self.program, arriving.stream) {
            // A kind with one interface takes what arrives on it, and a link
            // into it says the same thing its one sink does.
            (Program::Payload(app), Some(Stream::Groundward) | None) => app.give_payload_data(data),
            (Program::Moc(app), Some(Stream::Vehicleward) | None) => app.give_moc_data(data),
            // What asks a payload that is a program for a reading, which is
            // the datum itself: see `crate::node::payload::app_all_pl`.
            (Program::AllPl(app), Some(Stream::Vehicleward) | None) => app.give_moc_data(data),
            // And the kind nothing asks. It has no data sink at all, so
            // `Graph::check` refuses the link that would bring this about --
            // this is the backstop, and it says what the type is rather than
            // which stream was wrong, there being no stream it would take.
            (Program::Pl(_), _) => Err(format!(
                "{}: a datum arrived and nothing may be linked into an \
                 {}: it is a payload that is never asked, and its program \
                 does not read",
                self.what,
                app::Kind::Pl.node_type()
            )),
            (Program::All(app), Some(Stream::Groundward)) => app.give_payload_data(data),
            (Program::All(app), Some(Stream::Vehicleward)) => app.give_moc_data(data),
            // A node with two ways in, and a link that does not say which.
            // `Graph::check` refuses that as a `BadTerminal`, so this is the
            // backstop: guessing would put the ground's commands on the
            // groundward stream half the time.
            (Program::All(_), None) => Err(format!(
                "{}: a link into it says which of its two streams it carries, \
                 and this one says neither",
                self.what
            )),
            // And a datum on the stream this kind does not take at all.
            (_, Some(stream)) => Err(format!(
                "{}: a datum on the {} stream arrived, and this app does not \
                 take that stream",
                self.what,
                stream.label()
            )),
        };
        given.map_err(|why| {
            if why.starts_with(&self.what) {
                why
            } else {
                format!("{}: {why}", self.what)
            }
        })
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

    /// A temperature on a link: four bytes of micro-kelvin, which is what a
    /// conversion produces and what a computation reads.
    fn kelvin(number: f64) -> Vec<u8> {
        compute::as_value(number).expect("a temperature a value carries")
    }

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

    /// A line's reads end where its stream rule says, not wherever the bytes
    /// happened to be split.
    ///
    /// A sentence that arrives in pieces is held until its terminator, the
    /// terminator ends it, and what came after is the start of the next one:
    /// so each datum the pass takes is a whole sentence, which is what a
    /// node reading the line was told it would get.
    #[test]
    fn a_line_is_read_a_whole_read_at_a_time() {
        let (_script, port) = port_of_script(
            "gps",
            Use::Read,
            vec![
                Ok(b"$GPGGA".to_vec()),
                Ok(b",1\n$GP".to_vec()),
                Ok(b"RMC\n".to_vec()),
            ],
        );
        let mut port = port.framed_by(tcslibgs::endpoint_config::StreamParams {
            max_length: 64,
            timeout: None,
            terminators: vec![b'\n'],
            length_prefix: None,
        });
        let mut buffer = [0u8; 64];
        let mut read = || {
            let taken = port.take(&mut buffer).expect("a read").bytes;
            buffer[..taken].to_vec()
        };
        assert_eq!(read(), b"".to_vec(), "half a sentence is nothing yet");
        assert_eq!(read(), b"$GPGGA,1\n".to_vec());
        assert_eq!(read(), b"$GPRMC\n".to_vec());
    }

    /// A prefixed frame waiting on the line is taken in one pass: its
    /// prefix, and then the rest the prefix gives, rather than the prefix in
    /// one pass and the rest a group's interval later.
    #[test]
    fn a_prefixed_frame_is_read_in_one_pass() {
        let (_script, port) = port_of_script(
            "imu",
            Use::Read,
            vec![
                Ok(vec![3]),
                Ok(b"abc".to_vec()),
                Ok(vec![1]),
                Ok(b"d".to_vec()),
            ],
        );
        let mut port = port.framed_by(tcslibgs::endpoint_config::StreamParams {
            max_length: 64,
            timeout: None,
            terminators: Vec::new(),
            length_prefix: Some(tcslibgs::endpoint_config::LengthPrefix::U8),
        });
        let mut buffer = [0u8; 64];
        let mut read = || {
            let taken = port.take(&mut buffer).expect("a read").bytes;
            buffer[..taken].to_vec()
        };
        assert_eq!(read(), b"\x03abc".to_vec());
        assert_eq!(read(), b"\x01d".to_vec());
    }

    /// A serial node on a real line hands over whole reads, however the
    /// bytes arrive.
    ///
    /// End to end: a pseudo-terminal, which is the line tcssim stands in
    /// with, a node on it as a configuration states one, opened as the vehicle
    /// opens it. The line is raw, so the kernel hands over whatever bytes have
    /// arrived; a sentence written in pieces comes out whole only because the
    /// node's stream rule said where it ends.
    #[test]
    fn a_line_on_a_pseudo_terminal_is_read_a_whole_read_at_a_time() {
        use std::io::Write;
        use std::os::unix::io::FromRawFd;
        use tcslibgs::graph::{Destinations, Link, Node};

        let master_fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
        assert!(
            master_fd >= 0,
            "no pty: {}",
            std::io::Error::last_os_error()
        );
        let mut master = unsafe { std::fs::File::from_raw_fd(master_fd) };
        assert_eq!(unsafe { libc::grantpt(master_fd) }, 0);
        assert_eq!(unsafe { libc::unlockpt(master_fd) }, 0);
        let mut name = [0 as libc::c_char; 128];
        assert_eq!(
            unsafe { libc::ptsname_r(master_fd, name.as_mut_ptr(), name.len()) },
            0
        );
        let line = unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) }
            .to_string_lossy()
            .into_owned();

        let gps = Node {
            name: "gps".to_string(),
            node_type: "serial".to_string(),
            group: None,
            attributes: [
                ("path", line.as_str()),
                ("datarate", "9600"),
                ("stop_bits", "1"),
                ("byte_length", "8"),
                ("max_length", "64"),
                ("terminators", "0x0A"),
            ]
            .into_iter()
            .map(|(a, v)| (a.to_string(), tcslibgs::Scalar::of(v)))
            .collect(),
        };
        let moc = Node {
            name: "moc".to_string(),
            node_type: "moc".to_string(),
            group: None,
            attributes: Default::default(),
        };
        let graph = Graph {
            description: None,
            tcspecial: None,
            groups: Vec::new(),
            nodes: vec![gps.clone(), moc],
            links: vec![Link {
                from: "gps".to_string(),
                to: Destinations::of(vec!["moc".to_string()]),
                comment: None,
            }],
            commands: Vec::new(),
            files: Vec::new(),
            homes: Default::default(),
            placed: Default::default(),
            set_aside: Default::default(),
        };
        let mut port = EndpointPort::open(&gps, &Routes::of(&graph), &Abandon::nothing())
            .expect("the line opens");

        // What the next few passes take, for long enough that a byte written
        // to the far end has crossed the line.
        let taken = |port: &mut EndpointPort| -> Vec<u8> {
            let mut buffer = [0u8; 64];
            for _ in 0..50 {
                let n = port.take(&mut buffer).expect("a read").bytes;
                if n > 0 {
                    return buffer[..n].to_vec();
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            Vec::new()
        };

        master.write_all(b"$GPGGA").expect("written");
        assert_eq!(
            taken(&mut port),
            b"".to_vec(),
            "half a sentence is nothing yet"
        );
        master.write_all(b",1\n$GP").expect("written");
        assert_eq!(taken(&mut port), b"$GPGGA,1\n".to_vec());
        master.write_all(b"RMC\n").expect("written");
        assert_eq!(taken(&mut port), b"$GPRMC\n".to_vec());
    }

    /// The links say which way each node is used.
    #[test]
    fn the_links_say_which_way_a_node_is_used() {
        let routes = thermostat();

        // A sensor's output goes somewhere and nothing arrives at it.
        assert_eq!(Use::of(&routes, "temp_1"), Ok(Use::Read));
        // The heaters are the other way round.
        assert_eq!(Use::of(&routes, "heaters_1"), Ok(Use::Driven));
        // And the ground link is read: the setpoint is commanded through it,
        // which is the binding and not a link -- see the commanded case
        // below. Nothing is delivered to it: what is drawn into it goes to
        // each source's own link to its ground_base.
        assert_eq!(Use::of(&routes, "moc"), Ok(Use::Read));
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
            placed: Default::default(),
            set_aside: Default::default(),
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
        let (script, mut port) = port("temp_1", Use::Read);
        script.lock().expect("the script").to_read = vec![Ok(b"293.15".to_vec())];

        let mut buffer = [0u8; 64];
        let got = port
            .take(&mut buffer)
            .map(|taken| taken.bytes)
            .expect("the reading");
        assert_eq!(&buffer[..got], b"293.15");

        // And the pass after it, with the device not due.
        assert_eq!(
            port.take(&mut buffer).map(|taken| taken.bytes),
            Ok(0),
            "a device with nothing to say has not failed"
        );
    }

    /// A read that fails is reported, and names the node.
    #[test]
    fn a_read_that_fails_is_reported() {
        let (script, mut port) = port("temp_1", Use::Read);
        script.lock().expect("the script").to_read = vec![Err(TcsError::Endpoint(
            "the bus did not answer".to_string(),
        ))];

        let said = port
            .take(&mut [0u8; 64])
            .expect_err("a bus that did not answer");
        assert!(
            said.contains("temp_1") && said.contains("did not answer"),
            "the complaint should name the node and carry the reason: {said}"
        );
    }

    /// What arrives at a driven node is written whole.
    #[test]
    fn a_command_is_written_whole() {
        let (script, mut port) = port("heaters_1", Use::Driven);

        assert_eq!(port.give(Arriving::ANYWHERE, b"1"), Ok(()));
        assert_eq!(
            script.lock().expect("the script").written,
            vec![b"1".to_vec()]
        );
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
            .give(Arriving::ANYWHERE, b"293.15")
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

        let said = port
            .give(Arriving::ANYWHERE, b"293.15")
            .expect_err("a datum dropped");
        assert!(
            said.contains("moved none of them") && said.contains("dropped"),
            "the complaint should say the datum is gone: {said}"
        );
    }

    /// A write that failed outright carries the endpoint's own reason.
    #[test]
    fn a_write_that_fails_carries_the_reason() {
        let (script, mut port) = port("heaters_1", Use::Driven);
        script.lock().expect("the script").write_answers = vec![Err(TcsError::Endpoint(
            "cannot drive the lines of /dev/gpiochip0".to_string(),
        ))];

        let said = port
            .give(Arriving::ANYWHERE, b"1")
            .expect_err("a chip that refused");
        assert!(
            said.contains("heaters_1") && said.contains("/dev/gpiochip0"),
            "the complaint should name the node and the chip: {said}"
        );
        assert_eq!(
            said.matches("heaters_1").count(),
            1,
            "the node is named once: every endpoint names the node it belongs \
             to, and so does the line this is reported on -- {said}"
        );
    }

    /// Each direction refuses what the configuration does not use it for.
    #[test]
    fn a_node_refuses_the_direction_it_is_not_used_in() {
        let (_, mut sensor) = port("temp_1", Use::Read);
        let said = sensor
            .give(Arriving::ANYWHERE, b"1")
            .expect_err("a write to a sensor");
        assert!(
            said.contains("read and not written"),
            "the complaint should say which way it is used: {said}"
        );
        assert!(!sensor.accepts() && sensor.produces());

        let (_, mut heaters) = port("heaters_1", Use::Driven);
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
        let (script, mut port) = port("heaters_1", Use::Driven);

        let said = port
            .give(Arriving::named("in0"), b"1")
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
        let (script, port) = port("temp_1", Use::Read);
        let mut port = port.reading(2);
        script.lock().expect("the script").to_read = vec![Ok(b"293.15".to_vec())];

        // A sensor that answers in two bytes, read into a buffer with room
        // for sixty-four: two is what is asked for.
        let mut buffer = [0u8; 64];
        let got = port
            .take(&mut buffer)
            .map(|taken| taken.bytes)
            .expect("a reading");
        assert_eq!(got, 2, "a bus master clocks what it asks for and no more");

        // And the buffer is the other bound: a caller with room for one byte
        // gets one, whatever the node says.
        let (script, port) = port_of_script("temp_1", Use::Read, vec![Ok(b"293.15".to_vec())]);
        let mut port = port.reading(2);
        let got = port.take(&mut [0u8; 1]).expect("a reading");
        assert_eq!(got.bytes, 1);
        assert!(script.lock().expect("the script").to_read.is_empty());
    }

    /// A payload is written its init as it opens, before anything else is,
    /// and one the init cannot be written to does not open.
    ///
    /// A device file, driven: the heater the vehicle has not decided about
    /// yet, which init says starts off. `/dev/full` is a device every write to
    /// fails, which is the payload that would come up in a state nobody knows.
    #[test]
    fn a_payload_is_written_its_init_as_it_opens() {
        use tcslibgs::graph::{Destinations, Link};

        let heater = |path: &str| Node {
            name: "heater".to_string(),
            node_type: "device-file".to_string(),
            group: None,
            attributes: [("path", path), ("init", "\"0\"")]
                .into_iter()
                .map(|(a, v)| (a.to_string(), tcslibgs::Scalar::of(v)))
                .collect(),
        };
        let routes = |heater: &Node| {
            Routes::of(&Graph {
                description: None,
                tcspecial: None,
                groups: Vec::new(),
                nodes: vec![
                    heater.clone(),
                    Node {
                        name: "decide".to_string(),
                        node_type: "compute".to_string(),
                        group: None,
                        attributes: Default::default(),
                    },
                ],
                links: vec![Link {
                    from: "decide".to_string(),
                    to: Destinations::of(vec!["heater".to_string()]),
                    comment: None,
                }],
                commands: Vec::new(),
                files: Vec::new(),
                homes: Default::default(),
                placed: Default::default(),
                set_aside: Default::default(),
            })
        };

        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("heater");
        std::fs::write(&path, b"").expect("the device");
        let node = heater(path.to_str().expect("a path"));
        let port = EndpointPort::open(&node, &routes(&node), &Abandon::nothing())
            .expect("the heater opens");
        drop(port);
        assert_eq!(
            std::fs::read(&path).expect("what was written"),
            b"0",
            "the heater should have been set to what init says as it opened"
        );

        let node = heater("/dev/full");
        let said = match EndpointPort::open(&node, &routes(&node), &Abandon::nothing()) {
            Err(why) => why,
            Ok(_) => panic!("a payload its init could not be written to was opened"),
        };
        assert!(
            said.contains("heater") && said.contains("init"),
            "the complaint should name the node and the init: {said}"
        );
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

        let said = match EndpointPort::open(
            graph.node("temp_1").expect("a sensor"),
            &routes,
            &Abandon::nothing(),
        ) {
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
            &Abandon::nothing(),
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
        assert_eq!(
            unsafe { libc::mkfifo(name.as_ptr(), 0o600) },
            0,
            "a named pipe"
        );

        let config = EndpointConfig::Device(DeviceConfig {
            path: path.to_string_lossy().to_string(),
        });
        let mut port = EndpointPort::connect("sensor", &config, Use::Read).expect("the pipe opens");

        // Nothing has been written, and this returns: without the readiness
        // question in take, this test would hang rather than fail.
        let mut buffer = [0u8; 64];
        assert_eq!(
            port.take(&mut buffer).map(|taken| taken.bytes),
            Ok(0),
            "nothing to say is not a wait"
        );

        // And when there is something, it is read.
        let mut writer =
            EndpointPort::connect("writer", &config, Use::Both).expect("the other end");
        assert_eq!(writer.give(Arriving::ANYWHERE, b"293.15"), Ok(()));
        let got = port
            .take(&mut buffer)
            .map(|taken| taken.bytes)
            .expect("the reading");
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
        let mut port =
            EndpointPort::connect("loopback", &config, Use::Both).expect("the pipe opens");

        // Written and read back through the kernel, which is as much of a
        // device as a pipe can be.
        assert_eq!(port.give(Arriving::ANYWHERE, b"293.15"), Ok(()));

        let mut buffer = [0u8; 64];
        let got = port
            .take(&mut buffer)
            .map(|taken| taken.bytes)
            .expect("what was written");
        assert_eq!(&buffer[..got], b"293.15");

        // And the descriptor a waiting loop would poll.
        assert!(port.io_fd() >= 0);
    }

    /// A computation holds its result until it is taken.
    #[test]
    fn a_computation_has_a_result_when_it_has_enough() {
        use crate::node::vehicle::compute::vote::Vote;
        use tcslibgs::node::vehicle::compute::{Algorithm, VoteParams};

        let mut port = ComputePort::of(
            "cabin_temp",
            Box::new(Vote::of(VoteParams {
                algorithm: Algorithm::Median,
                inputs: vec!["in0".to_string(), "in1".to_string()],
            })),
        );
        assert_eq!(port.function(), "vote");

        // The first arrival is not a result: a mean of one of two sensors
        // would be an average of a reading and nothing.
        assert_eq!(port.give(Arriving::named("in0"), &kelvin(290.0)), Ok(()));
        assert!(!port.ready(), "a mean of one input is not an average");
        assert_eq!(port.take(&mut [0u8; 64]).map(|taken| taken.bytes), Ok(0));

        // The second is.
        assert_eq!(port.give(Arriving::named("in1"), &kelvin(292.0)), Ok(()));
        assert!(port.ready());

        let mut buffer = [0u8; 64];
        let got = port
            .take(&mut buffer)
            .map(|taken| taken.bytes)
            .expect("the average");
        assert_eq!(
            compute::from_value(&buffer[..got], "the average"),
            Ok(291.0)
        );

        // And it is gone once taken, rather than being handed out again.
        assert!(!port.ready());
        assert_eq!(port.take(&mut buffer).map(|taken| taken.bytes), Ok(0));
    }

    /// A ping node answers each ping as it is handed over, and refuses what is
    /// not one.
    #[test]
    fn a_ping_node_answers_what_arrives() {
        let mut port = PingPort::of("pinger");
        assert!(!port.ready(), "nothing has arrived");

        assert_eq!(
            port.give(Arriving::ANYWHERE, b"ping #1 1760900000.000000001"),
            Ok(())
        );
        assert!(port.ready(), "an answer is made as the ping arrives");

        let mut buffer = [0u8; tcslibgs::DATUM_BYTES];
        let got = port
            .take(&mut buffer)
            .map(|taken| taken.bytes)
            .expect("an answer");
        let pong = tcslibgs::node::vehicle::ping::read_reply(&buffer[..got]).expect("it reads");
        assert_eq!(pong.ping.count, 1);
        assert!(!port.ready());

        let why = port
            .give(Arriving::ANYWHERE, b"290.0")
            .expect_err("not a ping");
        assert!(why.starts_with("pinger: "), "{why}");
        assert!(!port.ready(), "and nothing is answered");
    }

    /// A tee hands on what arrives, in order and unchanged, on no stream in
    /// particular: so that the fabric sends it down every link the node has.
    #[test]
    fn a_tee_hands_on_what_arrives() {
        let mut port = TeePort::of("through");

        assert!(!port.ready(), "nothing has arrived");
        assert_eq!(port.take(&mut [0u8; 64]), Ok(Taken::NOTHING));

        assert_eq!(port.give(Arriving::ANYWHERE, b"290.0"), Ok(()));
        assert_eq!(port.give(Arriving::ANYWHERE, b"go\n"), Ok(()));
        assert!(port.ready());

        // In order, whole, and on no stream, so that it goes both ways.
        let mut buffer = [0u8; 64];
        let taken = port.take(&mut buffer).expect("the first");
        assert_eq!(&buffer[..taken.bytes], b"290.0");
        assert_eq!(taken.stream, None);
        let taken = port.take(&mut buffer).expect("the second");
        assert_eq!(&buffer[..taken.bytes], b"go\n");

        // And gone once taken, rather than handed out again.
        assert!(!port.ready());
        assert_eq!(port.take(&mut buffer).map(|taken| taken.bytes), Ok(0));

        // A datum that will not fit is kept rather than cut down: part of a
        // value is not a smaller value.
        assert_eq!(port.give(Arriving::ANYWHERE, b"2901234"), Ok(()));
        let said = port.take(&mut [0u8; 4]).expect_err("it does not fit");
        assert!(
            said.contains("through") && said.contains("7 bytes"),
            "{said}"
        );
        assert!(port.ready(), "the datum is still here");
        let got = port
            .take(&mut buffer)
            .map(|taken| taken.bytes)
            .expect("with room");
        assert_eq!(&buffer[..got], b"2901234");
    }

    /// A null node takes what it is given and has nothing to hand back.
    #[test]
    fn a_null_node_drops_what_arrives() {
        let mut port = NullPort::of();
        assert!(port.accepts() && !port.produces());
        assert_eq!(port.give(Arriving::ANYWHERE, b"290.0"), Ok(()));
        assert!(!port.ready());
        assert_eq!(port.take(&mut [0u8; 64]), Ok(Taken::NOTHING));
    }

    /// A datum a computation cannot use is refused, naming both.
    #[test]
    fn a_datum_a_computation_cannot_use_is_refused() {
        use crate::node::vehicle::compute::vote::Vote;
        use tcslibgs::node::vehicle::compute::{Algorithm, VoteParams};

        let mut port = ComputePort::of(
            "cabin_temp",
            Box::new(Vote::of(VoteParams {
                algorithm: Algorithm::Median,
                inputs: vec!["in0".to_string(), "in1".to_string()],
            })),
        );

        // Two ways a datum is not a value, and both are refused here rather
        // than read as a temperature: text where a value was meant, which is
        // the wrong width, and eight bytes that hold no number, which a vote
        // could not sort.
        for (datum, what) in [
            (&b"29x.15"[..], "text where a value was meant"),
            (
                &f64::NAN.to_le_bytes()[..],
                "eight bytes that are not a number",
            ),
        ] {
            let said = port.give(Arriving::named("in0"), datum).unwrap_err();
            assert!(
                said.contains("cabin_temp") && said.contains("vote"),
                "the complaint about {what} should name the node and the \
                 computation: {said}"
            );
        }
    }

    /// A result that will not fit is reported and kept, not cut down.
    #[test]
    fn a_result_too_large_for_the_buffer_is_kept() {
        use crate::node::vehicle::compute::vote::Vote;
        use tcslibgs::node::vehicle::compute::{Algorithm, VoteParams};

        let mut port = ComputePort::of(
            "cabin_temp",
            Box::new(Vote::of(VoteParams {
                algorithm: Algorithm::Median,
                inputs: vec!["in0".to_string(), "in1".to_string()],
            })),
        );
        port.give(Arriving::named("in0"), &kelvin(293.15))
            .expect("a reading");
        port.give(Arriving::named("in1"), &kelvin(293.15))
            .expect("the other reading");
        assert!(port.ready(), "the set is complete");

        let said = port.take(&mut [0u8; 2]).expect_err("a buffer too small");
        assert!(
            said.contains("not a smaller value"),
            "the complaint should say why it was not truncated: {said}"
        );

        // Still there, for a caller with room for it.
        assert!(port.ready(), "the result should not have been thrown away");
        let mut buffer = [0u8; 64];
        let got = port
            .take(&mut buffer)
            .map(|taken| taken.bytes)
            .expect("the result");
        assert_eq!(
            compute::from_value(&buffer[..got], "the result"),
            Ok(293.15)
        );
    }

    /// Every source of a log writes into the one log, in order.
    #[test]
    fn a_log_takes_what_several_nodes_produced() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut port = LogPort::open(
            &Node {
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
            },
            &crate::segsend::SegSend::none(),
        )
        .expect("a log");

        // Three readings and a decision, arriving as the fabric delivers
        // them: unnamed, because a log has no named inputs.
        for record in [&b"289.15"[..], b"289.15", b"287.3188", b"1"] {
            assert_eq!(port.give(Arriving::ANYWHERE, record), Ok(()));
        }
        // And one that arrives with a name anyway, which a log takes: an
        // arrival cannot be ambiguous to a node that writes everything down.
        assert_eq!(port.give(Arriving::named("in0"), b"291.0"), Ok(()));
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
        let mut port = LogPort::open(
            &Node {
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
            },
            &crate::segsend::SegSend::none(),
        )
        .expect("a log");

        assert_eq!(port.give(Arriving::ANYWHERE, b"abcd"), Ok(()));
        let said = port
            .give(Arriving::ANYWHERE, b"289.15")
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

        assert_eq!(port.give(Arriving::ANYWHERE, b"1"), Ok(()));
        assert!(
            !port.ready(),
            "a reading waiting on a socket is not a result of what was sent to it"
        );
    }

    /// A device that must be asked is asked after each read.
    #[test]
    fn a_triggered_device_is_asked_after_it_is_read() {
        let (script, port) = port("temp_3", Use::Read);
        let mut port = port.triggered_by(b"R".to_vec());
        script.lock().expect("the script").to_read = vec![Ok(b"42".to_vec()), Ok(b"43".to_vec())];

        let mut buffer = [0u8; 64];

        // A pass takes what is there and then asks. On a real device the
        // first pass takes nothing, the question not having been asked yet;
        // the stand-in answers at once, which is what lets the order be seen.
        let got = port
            .take(&mut buffer)
            .map(|taken| taken.bytes)
            .expect("a pass");
        assert_eq!(&buffer[..got], b"42");
        assert_eq!(
            script.lock().expect("the script").written,
            vec![b"R".to_vec()],
            "the device should have been asked once"
        );

        // And the pass after it takes the answer to that question and asks
        // again.
        let got = port
            .take(&mut buffer)
            .map(|taken| taken.bytes)
            .expect("a pass");
        assert_eq!(&buffer[..got], b"43");
        assert_eq!(script.lock().expect("the script").written.len(), 2);
    }

    /// A device asked on a timer of its own is read without being asked, and
    /// asked without being read.
    ///
    /// For a part that sends: its answers are taken on whatever pass they
    /// arrive on, and the question goes out when the timer says, so the two
    /// halves of a pass that `take` joins are apart here.
    #[test]
    fn a_device_can_be_read_without_asking_and_asked_without_reading() {
        let (script, opened) = port("temp_3", Use::Read);
        let mut asked = opened.triggered_by(b"R".to_vec());
        script.lock().expect("the script").to_read = vec![Ok(b"42".to_vec())];

        let mut buffer = [0u8; 64];
        let got = asked
            .take_unasked(&mut buffer)
            .map(|taken| taken.bytes)
            .expect("a read");
        assert_eq!(&buffer[..got], b"42");
        assert!(
            script.lock().expect("the script").written.is_empty(),
            "a read without asking asked"
        );

        assert_eq!(asked.ask(), Ok(()));
        assert_eq!(
            script.lock().expect("the script").written,
            vec![b"R".to_vec()]
        );
        assert!(
            script.lock().expect("the script").to_read.is_empty(),
            "nothing more was read"
        );

        // And a device with no trigger is asked nothing.
        let (script, mut unasked) = port("temp_1", Use::Read);
        assert_eq!(unasked.ask(), Ok(()));
        assert!(script.lock().expect("the script").written.is_empty());
    }

    /// A device that says how long it takes to answer is asked, waited for,
    /// and read in the one pass.
    ///
    /// The other order -- read, then ask -- hands back the answer to the
    /// previous question, which is a reading one interval old. A part whose
    /// interface document says how long it needs can be asked and read in the
    /// same pass, and then the reading is of this moment.
    #[test]
    fn a_device_with_a_read_delay_is_asked_before_it_is_read() {
        let (script, port) = port("temp_3", Use::Read);
        let mut port = port
            .triggered_by(b"R".to_vec())
            .waiting(Duration::from_millis(1));
        script.lock().expect("the script").to_read = vec![Ok(b"42".to_vec())];

        let mut buffer = [0u8; 64];
        let got = port
            .take(&mut buffer)
            .map(|taken| taken.bytes)
            .expect("a pass");

        assert_eq!(
            &buffer[..got],
            b"42",
            "the answer to the question just asked"
        );
        assert_eq!(
            script.lock().expect("the script").written,
            vec![b"R".to_vec()],
            "asked once, before the read rather than after it"
        );
    }

    /// Disabling a node ends the wait between asking and reading, where the
    /// delay would otherwise have run out first.
    ///
    /// The one wait this port makes that is a wait rather than a look. A
    /// disable that waited it out would arrive up to that delay late, which on
    /// a part that has stopped answering is exactly when it is wanted. The
    /// delay here is five seconds and the test takes none of them: what proves
    /// the point is that it returns at once.
    ///
    /// Nothing is produced and nothing is reported. The node has been told to
    /// stop, and what is on its interface is an answer to a question that is
    /// no longer being asked.
    #[test]
    fn disabling_a_node_ends_the_wait_between_asking_and_reading() {
        use crate::runtime::node_state::NodeState;
        use tcslibgs::NodeId;

        let state = NodeState::new("temp_3", NodeId::of(0)).expect("a state");
        let (script, port) = port("temp_3", Use::Read);
        let mut port = port
            .triggered_by(b"R".to_vec())
            .waiting(Duration::from_secs(5))
            .abandoned_for(state.abandon());
        script.lock().expect("the script").to_read = vec![Ok(b"42".to_vec())];

        // Stopped before the read, as the command interpreter stops it: the
        // flag goes down and the node's wakeup is poked.
        state.disable();

        let began = std::time::Instant::now();
        let taken = port.take(&mut [0u8; 64]).expect("no failure to report");
        assert_eq!(taken.bytes, 0, "nothing is produced");
        assert!(
            began.elapsed() < Duration::from_secs(1),
            "the five-second delay was waited out: {:?}",
            began.elapsed()
        );
        assert_eq!(
            script.lock().expect("the script").written,
            vec![b"R".to_vec()],
            "the question went out, and the answer was not waited for"
        );
        assert_eq!(
            script.lock().expect("the script").to_read.len(),
            1,
            "and the reading on its interface was left there"
        );
    }

    /// A device that cannot be asked is reported, read or no read.
    ///
    /// The reading in hand is the last one this node will produce, so a pass
    /// that took one and could not ask for the next has not succeeded.
    #[test]
    fn a_trigger_that_cannot_be_written_is_reported() {
        let (script, port) = port("temp_3", Use::Read);
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

    /// An app node is the program it names, and the streams do not cross.
    ///
    /// The whole of what the fabric's streams are for, end to end: a datum
    /// handed to an app node on one stream reaches its program, and what the
    /// program answers comes back saying which stream *that* is -- which for
    /// an `app_payload_veh` is the other one, it being a node that takes
    /// readings and answers with commands.
    mod app {
        use super::*;
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;

        /// A program standing in for an app, written in shell.
        ///
        /// `cat` with its descriptors arranged for the kind of app it is
        /// standing in for: the real apps are the `app_payload_veh`,
        /// `app_moc_veh` and `app_all_veh` crates, which have tests of their
        /// own. What is tested here is this port and the fabric's streams, and
        /// five lines of shell keep that test about those.
        fn stands_in(where_: &Path, redirect: &str) -> std::path::PathBuf {
            let path = where_.join("stands_in");
            let mut file = std::fs::File::create(&path).expect("a program");
            write!(file, "#!/bin/sh\nexec cat {redirect}\n").expect("a program");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("a program that can be run");
            path
        }

        /// A node of this type, running that program.
        ///
        /// With the argument its kind is run with for each stream it handles,
        /// those being required of a node: asked of `app::Kind` rather than
        /// listed per type here, so a kind that gains an argument gains it in
        /// these tests too.
        fn node(name: &str, node_type: &str, program: &std::path::Path) -> Node {
            // The whole path, this test module being called `app` itself.
            let kind = tcslibgs::node::app::Kind::of(node_type).expect("a kind of app");
            let mut attributes = std::collections::BTreeMap::new();
            attributes.insert(
                "executable".to_string(),
                tcslibgs::Scalar::of(program.display().to_string()),
            );
            if kind.handles_payload_data() {
                attributes.insert(
                    "app-payload-arg".to_string(),
                    tcslibgs::Scalar::of("--readings"),
                );
            }
            if kind.handles_moc_data() {
                attributes.insert(
                    "app-moc-arg".to_string(),
                    tcslibgs::Scalar::of("--commands"),
                );
            }
            Node {
                name: name.to_string(),
                node_type: node_type.to_string(),
                group: None,
                attributes,
            }
        }

        /// What the port has, once the program has answered.
        ///
        /// Polled, because that is what the pass does: an app is another
        /// process and answers when it answers. The bound is so that a
        /// program which never answers fails the test instead of hanging it.
        fn taken_soon(port: &mut AppPort, buffer: &mut [u8]) -> Taken {
            let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                let taken = port.take(buffer).expect("a datum");
                if taken.bytes > 0 {
                    return taken;
                }
                assert!(
                    std::time::Instant::now() < until,
                    "the program answered nothing"
                );
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }

        /// What an `app_payload_veh` answers is on the other stream.
        #[test]
        fn what_an_app_payload_veh_answers_is_vehicleward() {
            let where_ = tempfile::tempdir().expect("a directory");
            // Reads payload data at descriptor 0 and writes what it read back
            // to the same descriptor, which is MOC data there.
            let program = stands_in(where_.path(), ">&0");
            let mut port =
                crate::when_not_busy(|| AppPort::open(&node("fore", "app_payload_veh", &program)))
                    .expect("it starts");

            port.give(Arriving::on(Stream::Groundward), b"293.15")
                .expect("a reading it takes");

            let mut buffer = [0u8; 64];
            let taken = taken_soon(&mut port, &mut buffer);
            assert_eq!(
                taken.stream,
                Some(Stream::Vehicleward),
                "an app_payload_veh answers with what travels vehicleward"
            );
            assert_eq!(&buffer[..taken.bytes], b"293.15");
        }

        /// And an `app_all_veh` keeps the two apart.
        #[test]
        fn an_app_all_veh_keeps_the_two_streams_apart() {
            let where_ = tempfile::tempdir().expect("a directory");
            // Plain `cat`: payload data read at descriptor 0 is written as
            // payload data at descriptor 1, and the vehicleward side is left
            // alone.
            let program = stands_in(where_.path(), "");
            let mut port =
                crate::when_not_busy(|| AppPort::open(&node("through", "app_all_veh", &program)))
                    .expect("it starts");

            port.give(Arriving::on(Stream::Groundward), b"293.15")
                .expect("a reading it takes");

            let mut buffer = [0u8; 64];
            let taken = taken_soon(&mut port, &mut buffer);
            assert_eq!(
                taken.stream,
                Some(Stream::Groundward),
                "what came in groundward goes on groundward"
            );
            assert_eq!(&buffer[..taken.bytes], b"293.15");
        }

        /// A datum on a stream an app does not take is refused, and named.
        #[test]
        fn a_datum_on_the_wrong_stream_is_refused() {
            let where_ = tempfile::tempdir().expect("a directory");
            let program = stands_in(where_.path(), ">&0");
            let mut port =
                crate::when_not_busy(|| AppPort::open(&node("fore", "app_payload_veh", &program)))
                    .expect("it starts");

            let said = port
                .give(Arriving::on(Stream::Vehicleward), b"SET 300")
                .expect_err("a stream it does not take");
            assert!(said.contains("fore"), "{said}");
            assert!(said.contains("vehicleward"), "{said}");
        }

        /// A payload that is a program produces on the groundward stream.
        ///
        /// Asked for a reading and answering with one, which is what every
        /// payload does: what tells this from an `app_moc_veh` is the section
        /// of the file it is written in, and the fabric asks nothing about
        /// that.
        #[test]
        fn what_an_app_all_pl_answers_is_groundward() {
            let where_ = tempfile::tempdir().expect("a directory");
            // Reads at descriptor 1 and writes back to it, which is payload
            // data there.
            let program = stands_in(where_.path(), "<&1");
            let mut port =
                crate::when_not_busy(|| AppPort::open(&node("bay", "app_all_pl", &program)))
                    .expect("it starts");

            port.give(Arriving::on(Stream::Vehicleward), b"READ")
                .expect("what asks it for a reading");

            let mut buffer = [0u8; 64];
            let taken = taken_soon(&mut port, &mut buffer);
            assert_eq!(
                taken.stream,
                Some(Stream::Groundward),
                "a payload's reading travels groundward"
            );
            assert_eq!(&buffer[..taken.bytes], b"READ");
        }

        /// And a payload nothing asks is read and never written to.
        ///
        /// The one node type with no data sink at all. `Graph::check` refuses
        /// the link that would deliver to one; this is the port saying the same
        /// thing, and saying what the type is rather than which stream was
        /// wrong.
        #[test]
        fn an_app_pl_is_read_and_takes_nothing() {
            let where_ = tempfile::tempdir().expect("a directory");
            // One packet of payload data, written to descriptor 1 and read by
            // nobody: six bytes after a length of six.
            let path = where_.path().join("reports");
            let mut file = std::fs::File::create(&path).expect("a program");
            write!(file, "#!/bin/sh\nprintf '\\006\\000\\000\\000293.15' >&1\n")
                .expect("a program");
            // Closed before it is run: a program still open for writing is a
            // program the kernel refuses to execute.
            drop(file);
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("a program that can be run");

            let mut port = crate::when_not_busy(|| AppPort::open(&node("bay", "app_pl", &path)))
                .expect("it starts");

            let mut buffer = [0u8; 64];
            let taken = taken_soon(&mut port, &mut buffer);
            assert_eq!(taken.stream, Some(Stream::Groundward));
            assert_eq!(&buffer[..taken.bytes], b"293.15");

            // Nothing may be delivered to it, on either stream or on none.
            for arriving in [
                Arriving::on(Stream::Vehicleward),
                Arriving::on(Stream::Groundward),
                Arriving::ANYWHERE,
            ] {
                let said = port
                    .give(arriving, b"READ")
                    .expect_err("a payload that is never asked");
                assert!(said.contains("bay"), "{said}");
                assert!(said.contains("app_pl"), "{said}");
            }
        }

        /// A node whose program cannot be started is said so at once.
        #[test]
        fn a_program_that_cannot_be_started_is_said_so() {
            let where_ = tempfile::tempdir().expect("a directory");
            let said = AppPort::open(&node(
                "fore",
                "app_payload_veh",
                &where_.path().join("nothing-here"),
            ))
            .expect_err("a program that is not there");
            assert!(said.contains("nothing-here"), "{said}");
        }
    }
}
