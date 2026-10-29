//! The ground's commands, answered on a thread of their own.
//!
//! The vehicle takes commands on two UDP links, each from the one address the
//! configuration states, and answers them from a thread that is not the thread
//! making the pass. That is the whole reason this is a module and not a branch
//! inside the loop: a command interpreter that took its turn inside a pass
//! would answer at the rate of the slowest group in the configuration, and the
//! one command that matters most -- stop that node -- would be the one waiting
//! on it.
//!
//! # The two links
//!
//! One for the spacecraft and one for commands about nodes, because what the
//! ground needs in a hurry must never be queued behind what it asked for at
//! leisure, and a RESTART waiting behind a node's statistics is a vehicle that
//! cannot be rescued while it is busy. On a space link the two become two
//! virtual channels, which is where the priority between them is really
//! decided; over IP they are two ports.
//!
//! The second link is one socket, at `payload_command.dest_base` with nothing
//! added to it. The node id in the message says which node a command is about
//! -- see [`tcslibgs::NodeId`] -- so the socket need not. A socket per node
//! would spend ports on computations and logs that nothing addresses from the
//! ground.
//!
//! A command about one node read as being about another is one end reading a
//! different configuration, and that is checked for every command without a
//! socket apiece: the ids two ends use are the same ids exactly when their
//! digests agree, and CONNECT is what establishes that.
//!
//! # One thread, waiting on both
//!
//! Both sockets are in one poll set, with the stop descriptor beside them.
//! The thread therefore waits for a command or for the stop and for nothing
//! else: it wakes when there is something to do, and a vehicle whose ground
//! station has gone quiet costs it nothing. A thread per link with a read
//! timeout would be a wakeup every two hundred milliseconds per link to ask a
//! question whose answer is almost always no.
//!
//! # What it does not do on this thread
//!
//! A node's work. Disabling a node is a store and a poke -- see
//! [`crate::runtime::node_state`] -- which is why it can be answered here and
//! at once, and that is the point: a disable must not wait for the operation
//! the node is in the middle of. Work that has to happen between stages of a
//! pass belongs to the pass, and the answer goes out when the work is done.

use std::fmt::Write as _;
use std::net::{SocketAddr, UdpSocket};
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use log::{error, trace, warn};
use nix::poll::{poll, PollFd, PollFlags};
use std::os::fd::BorrowedFd;

use tcslibgs::config_digest::{ConfigDigest, ConfigVersion};
use tcslibgs::{
    answer_to, came_from, AnswerPacker, ArmKey, CIConfig, Command, CommandLink,
    CommandStatus, ConfigTelemetry, ConnectTelemetry, HostPort, NodeTelemetry,
    PingTelemetry, QueryNodeSampleTelemetry, QueryNodeTelemetry, Reason,
    SetNodeTelemetry, TcsError, TcsResult, Telemetry,
};

use crate::beacon_send::BeaconSend;
use crate::runtime::node_state::{NodeStates, Wakeup};
use crate::runtime::system::{Work, Workbench};
use crate::telemetry_log::TelemetryLog;

/// The most a command datagram may be.
///
/// A whole datagram, and not the longest command, `tcslibgs::COMMAND_BYTES`:
/// a datagram longer than any command is then read whole and refused for
/// what it is, rather than cut to a command's length and read as one.
const COMMAND_BYTES: usize = 65535;

/// What this vehicle calls each of the two links.
///
/// [`CommandLink::spelling`] says "payload command", after the link's name in
/// the protocol and the configuration. A graph has no payloads to command: the
/// link carries commands about nodes, so a message from this vehicle says so.
/// The wire is unaffected -- this is the word in a log line somebody reads.
fn spelling(which: CommandLink) -> &'static str {
    match which {
        CommandLink::Spacecraft => "spacecraft command",
        CommandLink::Payload => "node command",
    }
}

/// One of the two links, bound, with where its commands must come from.
struct Link {
    socket: UdpSocket,
    /// Which of the two this is: what decides whether a command that arrived
    /// here belongs here.
    which: CommandLink,
    /// The one address commands on it are taken from.
    ///
    /// A command is a command to restart a vehicle or stop a node; taking one
    /// from whoever sent it would be taking the ground's word from a stranger.
    from: HostPort,
}

/// What this vehicle answers a CONNECT with.
///
/// Settled when the process starts, not read when a command arrives: the
/// digest is of the file this process read, and a file edited since would
/// otherwise have the answers claiming a configuration nothing is serving.
/// The beacons carry the same two facts, from the same place.
#[derive(Debug, Clone, Copy)]
pub struct Flying {
    /// The version of this build.
    pub version: ConfigVersion,
    /// The digest of the configuration file this process read.
    pub digest: ConfigDigest,
    /// This start's generation, from the state file: a command sent for
    /// any other is refused, but for PING and CONNECT. See
    /// [`tcslibgs::CommandType::takes_any_generation`].
    pub generation: u32,
}

/// The command interpreter: two links, one thread.
///
/// Held by whatever started the vehicle, and stopped when that goes: dropping
/// this tells the thread to stop and waits for it, so the sockets are closed
/// by the time the process is.
pub struct Commands {
    /// Where the two links were bound, as the sockets report it: not what the
    /// configuration asked for, because a configuration may ask for port 0 and
    /// then the socket is the only statement of which port that is.
    spacecraft_at: SocketAddr,
    nodes_at: SocketAddr,
    /// The thread, until it is joined.
    thread: Option<JoinHandle<()>>,
    /// Down, and then a byte: what tells the thread to stop. The flag alone
    /// would not be noticed until the next command arrived, which on a quiet
    /// link is never.
    stopping: Arc<AtomicBool>,
    wakeup: Arc<Wakeup>,
}

impl Commands {
    /// Bind both links and start answering.
    ///
    /// A link that will not bind stops the start and says which it was: a
    /// vehicle that came up with no command link is a vehicle nobody can stop,
    /// and finding that out in flight is finding it out too late.
    pub fn start(
        ci: &CIConfig,
        states: Arc<NodeStates>,
        bench: Workbench,
        flying: Flying,
        beacon: Option<BeaconSend>,
        log: TelemetryLog,
        restart: fn(),
    ) -> TcsResult<Commands> {
        let spacecraft = Link {
            socket: bound(
                "the spacecraft command link",
                // Its own two fields rather than a HostPort, which is how
                // the configuration states it: `tcs_cmd.recv` becomes the
                // address and the port beside each other.
                &HostPort {
                    address: ci.address.clone(),
                    port: ci.port,
                },
            )?,
            which: CommandLink::Spacecraft,
            from: ci.moc_cmd_send.clone(),
        };
        let nodes = Link {
            socket: bound(
                "the node command link",
                &ci.payload_command_dest_base,
            )?,
            which: CommandLink::Payload,
            from: ci.payload_command_source_base.clone(),
        };

        let spacecraft_at = spacecraft.socket.local_addr()?;
        let nodes_at = nodes.socket.local_addr()?;

        let stopping = Arc::new(AtomicBool::new(false));
        let wakeup = Arc::new(Wakeup::new()?);

        let serving = Serving {
            states,
            bench,
            flying,
            beacon,
            log,
            stopping: stopping.clone(),
            wakeup: wakeup.clone(),
            answer: AnswerPacker::new(),
            armed: None,
            arm_valid: ci.restart_arm_valid,
            restart,
            restarting: false,
        };
        let thread = std::thread::spawn(move || serving.run(spacecraft, nodes));

        Ok(Commands {
            spacecraft_at,
            nodes_at,
            thread: Some(thread),
            stopping,
            wakeup,
        })
    }

    /// Where spacecraft commands are taken, as the socket was bound.
    pub fn spacecraft_address(&self) -> SocketAddr {
        self.spacecraft_at
    }

    /// Where commands about nodes are taken, as the socket was bound.
    pub fn node_address(&self) -> SocketAddr {
        self.nodes_at
    }

    /// Stop answering, and wait for the thread to finish what it is doing.
    ///
    /// The flag down and then a byte, which is the order that matters: a
    /// thread woken before the flag was down would look up, find itself still
    /// serving and wait again.
    pub fn stop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        self.wakeup.poke();
        if let Some(thread) = self.thread.take() {
            // A failed join is a thread that panicked, which it has already
            // reported. Nothing here can do anything about it, and a vehicle
            // shutting down must not fail to shut down over it.
            let _ = thread.join();
        }
    }
}

impl Drop for Commands {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Bind one link, saying which it was if it will not bind.
fn bound(which: &str, at: &HostPort) -> TcsResult<UdpSocket> {
    UdpSocket::bind(at.to_string()).map_err(|e| {
        TcsError::Config(format!(
            "{which} cannot be bound at {at}: {e} -- a vehicle with no command \
             link is a vehicle nobody can stop"
        ))
    })
}

/// What the thread holds while it serves.
struct Serving {
    states: Arc<NodeStates>,
    /// How the work a command causes is handed to the pass: opening a device
    /// takes time and can fail, and neither belongs on this thread.
    bench: Workbench,
    flying: Flying,
    /// The beacon sender, so that a CONFIG retimes the one that is running.
    /// `None` for a configuration that sends no beacons.
    beacon: Option<BeaconSend>,
    /// Where every answer is recorded, which is the log the beacons use: one
    /// log holds everything that went to the ground.
    log: TelemetryLog,
    stopping: Arc<AtomicBool>,
    wakeup: Arc<Wakeup>,
    /// What an answer is packed into before it is sent: made with the
    /// thread and reused, so that answering allocates nothing of its own.
    answer: AnswerPacker,
    /// The key of the last RESTART_ARM and when it was taken, until a RESTART
    /// uses it.
    armed: Option<(ArmKey, Instant)>,
    /// How long an arm licenses a RESTART for: the configuration's
    /// `restart_arm_valid`.
    arm_valid: Duration,
    /// What an armed RESTART calls, once it has been answered: asks the
    /// vehicle to stop and run itself again. See [`crate::runtime::restart`].
    restart: fn(),
    /// Whether the command just answered was an armed RESTART, so that the
    /// restart is asked for once its answer has gone.
    restarting: bool,
}

impl Serving {
    /// Wait for a command on either link, answer it, and wait again.
    fn run(mut self, spacecraft: Link, nodes: Link) {
        let links = [spacecraft, nodes];
        let mut buffer = vec![0u8; COMMAND_BYTES];

        while !self.stopping.load(Ordering::SeqCst) {
            // SAFETY: every descriptor here is owned by something that
            // outlives the poll -- the two sockets by `links`, the wakeup by
            // the Arc this holds.
            //
            // On the stack, the two links and the wakeup: waiting allocates
            // nothing.
            let borrowed: [BorrowedFd; 3] = unsafe {
                [
                    BorrowedFd::borrow_raw(links[0].socket.as_raw_fd()),
                    BorrowedFd::borrow_raw(links[1].socket.as_raw_fd()),
                    BorrowedFd::borrow_raw(self.wakeup.io_fd()),
                ]
            };
            let mut set: [PollFd; 3] = [
                PollFd::new(&borrowed[0], PollFlags::POLLIN),
                PollFd::new(&borrowed[1], PollFlags::POLLIN),
                PollFd::new(&borrowed[2], PollFlags::POLLIN),
            ];

            // No timeout: the stop is in the set, so there is nothing to wake
            // up for that is not in it. A signal interrupting the wait is not
            // an error -- it is what arrives when a child of this process
            // exits -- so it waits again.
            match poll(&mut set, -1) {
                Ok(_) => {}
                Err(nix::errno::Errno::EINTR) => continue,
                Err(e) => {
                    error!("the command links cannot be waited on: {e}");
                    return;
                }
            }

            if self.stopping.load(Ordering::SeqCst) {
                return;
            }

            for (which, link) in links.iter().enumerate() {
                let ready = set[which]
                    .revents()
                    .is_some_and(|events| events.contains(PollFlags::POLLIN));
                if ready {
                    self.take_one(link, &mut buffer);
                }
            }
        }
    }

    /// Take one datagram from this link and answer it.
    fn take_one(&mut self, link: &Link, buffer: &mut [u8]) {
        let (size, from) = match link.socket.recv_from(buffer) {
            Ok(got) => got,
            // Nothing there after all, which a poll can say: another reader
            // does not exist here, but a spurious readiness is cheaper to
            // tolerate than to rule out.
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => return,
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => return,
            Err(e) => {
                error!("{}: cannot be read: {e}", spelling(link.which));
                return;
            }
        };

        // Only from where the configuration says the ground commands from, and
        // nothing is sent back to anywhere else: an answer would tell whoever
        // sent it that something is listening here.
        if !came_from(&link.from, &from) {
            warn!(
                "a {} came from {from} and not from {}, which is where the \
                 configuration says the ground sends it from -- ignored",
                spelling(link.which),
                link.from
            );
            return;
        }

        // Read without allocating, a SET_NODE's attribute and value
        // included: they are read into room the command holds. A packet that
        // is not one of the link's is refused here, so nothing it says is
        // acted on and it does not count as the MOC being heard.
        let command = match Command::read(&buffer[..size]) {
            Ok(command) => command,
            // Not a command. Nothing to answer and nobody to answer it to: the
            // sender is whatever sent this.
            Err(why) => {
                error!(
                    "{}: {size} bytes that are not a command: {why}",
                    spelling(link.which)
                );
                return;
            }
        };

        // A valid packet from the MOC: from where the configuration says, and
        // a command. The beacons go back to their interval if they were
        // going fast because nothing had come since this process started.
        if let Some(beacon) = self.beacon.as_ref() {
            beacon.heard_from_moc();
        }

        trace!("{}: {command:?}", spelling(link.which));
        let answer = self.answer(link.which, &command);

        // Into the thread's own packer, which keeps its buffer.
        let data = self.answer.pack(&answer);
        // Recorded before it is sent, as a beacon is, so that an answer the
        // send fails on is still known to have been produced.
        self.log.record(data);
        // Back to the address it came from, because a datagram's sender is
        // the only statement of where the ground is, and a configuration
        // that stated it as well would be a second answer that can disagree.
        let _ = link.socket.send_to(data, from);

        // After the answer has gone, so that the ground hears the RESTART was
        // taken before the links close under it.
        if self.restarting {
            self.restarting = false;
            // Initiated now: the beacons say so from here until the process
            // ends, starting with one at once.
            if let Some(beacon) = self.beacon.as_ref() {
                beacon.restart_initiated();
            }
            (self.restart)();
        }
    }

    /// What this vehicle answers that command with.
    ///
    /// A match over every kind rather than a rule with an exception, so that a
    /// command added later is answered deliberately -- by someone who had to
    /// decide -- rather than by whichever side of a default it fell on.
    fn answer(&mut self, arrived: CommandLink, command: &Command) -> Telemetry {
        let sequence = command.sequence();

        // On the wrong link. Refused rather than served: it reached the
        // vehicle, so something can be said about it, and what has gone wrong
        // is that one end is not reading the configuration the other is.
        // Serving it would hide that in the one place the split is supposed to
        // be reliable.
        if !command.cmd_type().may_arrive_on(arrived) {
            error!(
                "{:?} arrived on the {} link and belongs on the {} one: the \
                 two ends are not reading the same configuration",
                command.cmd_type(),
                spelling(arrived),
                spelling(command.link())
            );
            return answer_to(command, CommandStatus::InvalidCommand);
        }

        // Sent for another start of tcspecial than this one: before a
        // restart and delivered after it, or by a station that has not heard
        // which generation is flying. Refused rather than read, because the
        // definitions it was written for may not be this start's.
        if !command.cmd_type().takes_any_generation()
            && command.generation() != self.flying.generation
        {
            warn!(
                "{:?} {} was sent for generation {} and this is generation \
                 {}: refused, and nothing in it acted on",
                command.cmd_type(),
                sequence,
                command.generation(),
                self.flying.generation
            );
            return answer_to(command, CommandStatus::WrongGeneration);
        }

        match command {
            Command::Ping(_) => Telemetry::Ping(PingTelemetry::new(
                sequence,
                CommandStatus::Success,
            )),

            // What this vehicle is flying, which is what licenses every
            // command that carries a node id: a ground station whose digest
            // does not match does not send them.
            Command::Connect(_) => Telemetry::Connect(ConnectTelemetry::new(
                sequence,
                self.flying.version,
                self.flying.digest,
                self.flying.generation,
            )),

            // The beacon thread is retimed where it waits, so the next beacon
            // goes at the new interval rather than after the old one elapses.
            Command::Config(cmd) => {
                if let Some(beacon) = self.beacon.as_mut() {
                    beacon.set_interval(cmd.beacon_interval.duration());
                }
                Telemetry::Config(ConfigTelemetry::new(
                    sequence,
                    CommandStatus::Success,
                ))
            }

            Command::StartNode(cmd) => {
                let status = self.set_state(cmd.node, true);
                Telemetry::StartNode(NodeTelemetry::started(
                    sequence, status, cmd.node,
                ))
            }
            Command::StopNode(cmd) => {
                let status = self.set_state(cmd.node, false);
                Telemetry::StopNode(NodeTelemetry::stopped(
                    sequence, status, cmd.node,
                ))
            }

            Command::SetNode(cmd) => {
                let (status, reason) =
                    self.set_attribute(cmd.node, cmd.attribute, cmd.value);
                Telemetry::SetNode(
                    SetNodeTelemetry::new(sequence, status, cmd.node)
                        .because(reason),
                )
            }

            // The two queries are answered on this thread and hand nothing
            // over. What they ask for is the node's own -- atomics it writes
            // as it moves data, and a sample behind a lock this takes only if
            // it is free -- so neither waits for a pass, and a query cannot
            // be what stalls the loop.
            Command::QueryNode(cmd) => match self.states.at(cmd.node) {
                Some(state) => Telemetry::QueryNode(QueryNodeTelemetry::new(
                    sequence,
                    CommandStatus::Success,
                    cmd.node,
                    state.is_enabled(),
                    state.counts(),
                )),
                None => {
                    self.no_such_node(cmd.node);
                    Telemetry::QueryNode(QueryNodeTelemetry::refused(
                        sequence,
                        CommandStatus::NotFound,
                        cmd.node,
                    ))
                }
            },

            Command::QueryNodeSample(cmd) => match self.states.at(cmd.node) {
                Some(state) => {
                    let samples = state.samples();
                    Telemetry::QueryNodeSample(QueryNodeSampleTelemetry::new(
                        sequence,
                        CommandStatus::Success,
                        cmd.node,
                        samples.taken,
                        samples.given,
                    ))
                }
                None => {
                    self.no_such_node(cmd.node);
                    Telemetry::QueryNodeSample(QueryNodeSampleTelemetry::new(
                        sequence,
                        CommandStatus::NotFound,
                        cmd.node,
                        tcslibgs::DHSample::new(),
                        tcslibgs::DHSample::new(),
                    ))
                }
            },

            // Armed for the next restart_arm_valid, with this key. A second
            // arm replaces the first, so the arm that counts is the one the
            // operator gave last.
            Command::RestartArm(cmd) => {
                self.armed = Some((cmd.arm_key, Instant::now()));
                answer_to(command, CommandStatus::Success)
            }

            // Only with the key of an arm taken within restart_arm_valid, and
            // the arm is used up: one lost or repeated datagram must not be
            // able to restart the flight software. A key that does not match
            // leaves the arm as it was, so an operator who mistyped it can
            // send the RESTART again without arming again.
            Command::Restart(cmd) => match self.armed {
                Some((key, taken))
                    if key == cmd.arm_key
                        && taken.elapsed() < self.arm_valid =>
                {
                    self.armed = None;
                    self.restarting = true;
                    answer_to(command, CommandStatus::Success)
                }
                Some((key, _)) if key != cmd.arm_key => {
                    warn!(
                        "a RESTART came with a key that is not the arm's: \
                           refused"
                    );
                    answer_to(command, CommandStatus::NotArmed)
                }
                _ => {
                    warn!(
                        "a RESTART came with no RESTART_ARM in the last {:?}, \
                         which is how long restart_arm_valid says an arm \
                         lasts: refused",
                        self.arm_valid
                    );
                    answer_to(command, CommandStatus::NotArmed)
                }
            },
        }
    }

    /// Say that the ground named a node this configuration has not got.
    ///
    /// One wording, because the news is the same whichever command carried
    /// the id: the ground is reading a configuration this vehicle is not
    /// flying, and the id it sent means a node of that one.
    fn no_such_node(&self, node: tcslibgs::NodeId) {
        error!(
            "there is no node {node} in this configuration, which has {}: the \
             ground is reading a configuration this vehicle is not flying",
            self.states.len()
        );
    }

    /// Enable or disable the node with that id, and say how it went.
    ///
    /// An id the configuration does not have is `NotFound` and says which id
    /// and how many nodes there are: a fixed-length field holds any number, so
    /// a command carrying one is a thing to answer rather than a thing to
    /// parse, and the range check is the whole of what the vehicle can say
    /// about it.
    ///
    /// A node already in the state asked for is success. A command that
    /// arrives twice, or whose answer was lost, is the ordinary case on a link
    /// with latency, and an operator who presses Disable twice has asked for a
    /// state and not for an action.
    ///
    /// Allocates nothing of its own: the work is handed over by the node's
    /// shared name. What starting the node again takes is the pass's -- see
    /// `System::reopen` -- and a failure is logged in words, which are made.
    fn set_state(
        &self,
        node: tcslibgs::NodeId,
        enabled: bool,
    ) -> CommandStatus {
        let Some(state) = self.states.at(node) else {
            self.no_such_node(node);
            return CommandStatus::NotFound;
        };
        let name = state.shared_name();

        // A node the ground may not start or stop -- a ping node, which is
        // how the ground knows the vehicle is there -- is refused and left as
        // it was: see `tcslibgs::graph::starts_and_stops`. Before anything
        // below, because stopping begins the moment this is asked.
        if !state.is_switchable() {
            warn!(
                "\"{name}\" may not be {} from the ground, and is left as it \
                 was",
                if enabled { "started" } else { "stopped" }
            );
            return CommandStatus::InvalidCommand;
        }

        // Stopping takes effect here and now, before anything is handed over:
        // a node the ground has disabled must stop, not stop after the
        // operation it is in the middle of. The flag goes down and the node's
        // wakeup is poked, so a wait inside an operation ends at once.
        if !enabled {
            state.disable();
        }

        // And then the work that belongs to the pass: releasing what a
        // stopped node held, or opening a node again. The answer waits for it,
        // so a success means the device is free or the node is running --
        // which is what a ground station about to hand the device to something
        // else needs to know.
        let work = if enabled {
            Work::Enable(Arc::clone(&name))
        } else {
            Work::Disable(Arc::clone(&name))
        };
        match self.bench.ask(work) {
            Ok(()) => CommandStatus::Success,
            Err(why) => {
                error!(
                    "\"{name}\" could not be {}: {why}",
                    if enabled { "started" } else { "released" }
                );
                CommandStatus::Failure
            }
        }
    }

    /// Set an attribute of the node with that id, and say how it went.
    ///
    /// The work is the pass's, as enabling and releasing are: what this
    /// changes is the node's configuration, and a node whose setpoint moved
    /// in the middle of a computation would be a node doing half of one thing
    /// and half of another. The answer waits for it, so a success means the
    /// node is running on the new value.
    ///
    /// Applied whether or not the node is disabled: what it sets is the
    /// node's configuration rather than its traffic, so a payload the ground
    /// stopped can be reconfigured before it is started again.
    ///
    /// `InvalidParameter` for an attribute no binding mentions and for a
    /// value the node will not take: both are the ground asking for something
    /// the configuration does not allow, and the status an operator acts on
    /// is the same. Which it was is the reason, which the answer carries as
    /// well as the log, so that the operator is told what to change without
    /// reading a log aboard.
    ///
    /// Allocates nothing of its own: the node goes by its shared name, the
    /// attribute and the value in the room the SET_NODE was read into, and
    /// the reason is written into room the answer holds. What says why a
    /// node refused is the node's own message, made where it refused.
    fn set_attribute(
        &self,
        node: tcslibgs::NodeId,
        attribute: tcslibgs::Attribute,
        value: tcslibgs::Value,
    ) -> (CommandStatus, Reason) {
        let Some(state) = self.states.at(node) else {
            self.no_such_node(node);
            let mut reason = Reason::default();
            let _ = write!(
                reason,
                "there is no node {node} in this configuration, which has {}",
                self.states.len()
            );
            return (CommandStatus::NotFound, reason);
        };
        let name = state.shared_name();

        match self.bench.ask(Work::Set {
            node: Arc::clone(&name),
            attribute,
            value,
        }) {
            Ok(()) => (CommandStatus::Success, Reason::default()),
            Err(why) => {
                error!("\"{name}\".{attribute} could not be set: {why}");
                (CommandStatus::InvalidParameter, Reason::of(&why))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslibgs::graph::Graph;
    use tcslibgs::{Command, NodeCommand, NodeId};

    /// A configuration whose command links are on loopback ports the system
    /// chooses, and whose ground is this test.
    ///
    /// Port 0 throughout: a test that named ports would fail whenever anything
    /// else on the machine held them, and the sockets say where they landed.
    fn links_for(ground: &UdpSocket) -> CIConfig {
        let at = ground.local_addr().expect("where the ground is");
        let here = |port| HostPort {
            address: "127.0.0.1".to_string(),
            port,
        };
        CIConfig {
            address: "127.0.0.1".to_string(),
            port: 0,
            // Both links take commands from the one socket this test sends
            // from, which is what a ground station is here.
            moc_cmd_send: here(at.port()),
            payload_command_source_base: here(at.port()),
            payload_command_dest_base: here(0),
            payload_source_base: here(0),
            payload_dest_base: here(0),
            beacon_interval: Duration::from_secs(5),
            reset_interval: Duration::from_secs(1),
            beacon_send: here(0),
            beacon_recv: here(0),
            // Nothing to write: what is being tested is what goes out.
            log_dir: None,
            log_segment_bytes: 65_536,
            restart_arm_valid: tcslibgs::RESTART_ARM_VALID_DEFAULT,
            gateway: tcslibgs::gateway::GatewayKind::Radio,
            // Nothing reads it: the state file is counted where beacons
            // start, which this does not.
            state_file: String::new(),
        }
    }

    /// Two nodes, so that an id can be right and an id can be out of range.
    fn two_nodes() -> Graph {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("two.yaml");
        std::fs::write(
            &path,
            "payloads:\n\
             \x20 - { name: inside, type: i2c, path: /dev/i2c-1, address: \
             \"0x48\", poll_interval: \"1 s\", payload-out: average }\n\
             vehicle:\n\
             \x20 - { name: average, type: compute, function: mean, inputs: 1 \
             }\n",
        )
        .expect("written");
        Graph::load(&path).expect("it reads")
    }

    /// The generation the vehicle in these tests is flying.
    const GENERATION: u32 = 3;

    fn flying() -> Flying {
        Flying {
            version: ConfigVersion::of_this_build(),
            digest: ConfigDigest([0x5Au8; 16]),
            generation: GENERATION,
        }
    }

    /// Ask on a link and read the answer, or say nothing came.
    ///
    /// For the generation flying, as a station that has heard it sends, but
    /// where the command was given one of its own.
    fn ask(
        ground: &UdpSocket,
        at: SocketAddr,
        command: &Command,
    ) -> Option<Telemetry> {
        let command = match command.generation() {
            0 => command.clone().for_generation(GENERATION),
            _ => command.clone(),
        };
        let asked = command.to_bytes();
        ground.send_to(&asked, at).expect("it is sent");
        let mut buffer = [0u8; COMMAND_BYTES];
        loop {
            match ground.recv_from(&mut buffer) {
                Ok((size, _)) => {
                    return Some(
                        Telemetry::read(&buffer[..size]).expect("telemetry"),
                    )
                }
                // A child of this test binary exiting interrupts a blocking
                // receive; the links' own loops wait again for the same
                // reason.
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    continue
                }
                Err(_) => return None,
            }
        }
    }

    /// Stand in for the pass, doing whatever work the interpreter hands over.
    ///
    /// `how` says what each job comes to, so a test can have an opening fail
    /// with no device to fail on. The work is recorded as well as answered,
    /// because a test wants to say that a command caused the right work and
    /// not only that it got the right answer back.
    fn a_pass_answering(
        how: impl Fn(&Work) -> Result<(), String> + Send + 'static,
    ) -> (Workbench, Arc<std::sync::Mutex<Vec<Work>>>) {
        let (bench, desk) = Workbench::in_two().expect("a workbench");
        let done = Arc::new(std::sync::Mutex::new(Vec::new()));
        let kept = done.clone();
        std::thread::spawn(move || {
            while let Some(job) = desk.wait() {
                kept.lock().expect("the record").push(job.work.clone());
                desk.answer(&job, how(&job.work));
            }
        });
        (bench, done)
    }

    fn started(states: Arc<NodeStates>) -> (UdpSocket, Commands) {
        let (bench, _) = a_pass_answering(|_| Ok(()));
        started_with(states, bench)
    }

    fn started_with(
        states: Arc<NodeStates>,
        bench: Workbench,
    ) -> (UdpSocket, Commands) {
        let ground = UdpSocket::bind("127.0.0.1:0").expect("a ground socket");
        ground
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a timeout");
        let commands = Commands::start(
            &links_for(&ground),
            states,
            bench,
            flying(),
            None,
            TelemetryLog::disabled(),
            || {},
        )
        .expect("both links bind");
        (ground, commands)
    }

    /// A PING is answered, and a CONNECT says what this vehicle is flying.
    ///
    /// On the spacecraft link, which is where both belong: a CONNECT is about
    /// having a connection to the vehicle at all, and a ground station with no
    /// connection has nothing to ask a node.
    #[test]
    fn the_spacecraft_link_answers_a_ping_and_says_what_it_flies() {
        let graph = two_nodes();
        let states = Arc::new(NodeStates::of(&graph).expect("states"));
        let (ground, commands) = started(states);
        let at = commands.spacecraft_address();

        match ask(&ground, at, &Command::Ping(tcslibgs::PingCommand::new(7))) {
            Some(Telemetry::Ping(tm)) => {
                assert_eq!(
                    tm.header.sequence, 7,
                    "the answer carries the command's"
                );
                assert_eq!(tm.header.status, CommandStatus::Success);
            }
            other => panic!("expected a ping answer, got {other:?}"),
        }

        let connect = Command::Connect(tcslibgs::ConnectCommand::new(
            8,
            ConfigVersion::of_this_build(),
            ConfigDigest([0x5Au8; 16]),
        ));
        match ask(&ground, at, &connect) {
            Some(Telemetry::Connect(tm)) => {
                assert_eq!(tm.header.status, CommandStatus::Success);
                assert_eq!(
                    tm.digest,
                    ConfigDigest([0x5Au8; 16]),
                    "the file it read"
                );
                assert_eq!(
                    tm.version,
                    ConfigVersion::of_this_build(),
                    "this build"
                );
                assert_eq!(tm.generation, GENERATION, "this start");
            }
            other => panic!("expected a connect answer, got {other:?}"),
        }
    }

    /// A command sent for another generation is refused and not acted on,
    /// whichever generation that is -- the one before, or none at all from
    /// a station that has heard neither a beacon nor a CONNECT's answer --
    /// but a PING and a CONNECT are answered whatever generation they say,
    /// because they are how a station finds out which one is flying.
    #[test]
    fn a_command_for_another_generation_is_refused() {
        let graph = two_nodes();
        let states = Arc::new(NodeStates::of(&graph).expect("states"));
        let ids = tcslibgs::graph::telemetry::ids(&graph);
        let inside = NodeId::of(ids["inside"]);
        let (bench, done) = a_pass_answering(|_| Ok(()));
        let (ground, commands) = started_with(states.clone(), bench);
        let node_at = commands.node_address();
        let at = commands.spacecraft_address();

        for other in [GENERATION - 1, GENERATION + 1, 0] {
            let stop = Command::StopNode(NodeCommand::stop(4, inside));
            // `ask` stamps nought as the generation flying, so nought is
            // sent by hand.
            let answer = if other == 0 {
                ground
                    .send_to(&stop.to_bytes(), node_at)
                    .expect("it is sent");
                let mut buffer = [0u8; COMMAND_BYTES];
                let (size, _) = ground.recv_from(&mut buffer).expect("one");
                Telemetry::read(&buffer[..size]).ok()
            } else {
                ask(&ground, node_at, &stop.for_generation(other))
            };
            match answer {
                Some(Telemetry::StopNode(tm)) => assert_eq!(
                    tm.header.status,
                    CommandStatus::WrongGeneration,
                    "generation {other}"
                ),
                other => panic!("expected a stop answer, got {other:?}"),
            }
            assert!(states.is_enabled("inside"), "the node was not stopped");
            assert!(done.lock().expect("the record").is_empty());

            let ping = Command::Ping(tcslibgs::PingCommand::new(5))
                .for_generation(other.max(1) + 10);
            match ask(&ground, at, &ping) {
                Some(Telemetry::Ping(tm)) => {
                    assert_eq!(tm.header.status, CommandStatus::Success)
                }
                other => panic!("expected a ping answer, got {other:?}"),
            }
            let connect = Command::Connect(tcslibgs::ConnectCommand::new(
                6,
                ConfigVersion::of_this_build(),
                ConfigDigest([0x5Au8; 16]),
            ))
            .for_generation(other.max(1) + 10);
            match ask(&ground, at, &connect) {
                Some(Telemetry::Connect(tm)) => {
                    assert_eq!(tm.header.status, CommandStatus::Success);
                    assert_eq!(tm.generation, GENERATION);
                }
                other => panic!("expected a connect answer, got {other:?}"),
            }
        }
    }

    /// A STOP_NODE stops that node at once and has the pass release what it
    /// held; a START_NODE has the pass open it again.
    ///
    /// The two halves of stopping a node are both tested here because they
    /// happen in different places and the division is the point. The flag goes
    /// down on this thread, so the node stops without waiting for anything;
    /// releasing what it held is work for the pass, because a port must not be
    /// closed under an operation using it. The answer waits for the pass, so a
    /// success means the device is free -- which is what a ground station
    /// about to hand that device to something else needs to know.
    ///
    /// Enabling is the pass's throughout: the node is let back in only once
    /// its port is open, since a node marked enabled over a port that is not
    /// there would be read by the next pass and refused by it.
    #[test]
    fn a_node_is_stopped_and_started_by_its_id() {
        let graph = two_nodes();
        let states = Arc::new(NodeStates::of(&graph).expect("states"));
        let ids = tcslibgs::graph::telemetry::ids(&graph);
        let inside = NodeId::of(ids["inside"]);

        let (bench, done) = a_pass_answering(|_| Ok(()));
        let (ground, commands) = started_with(states.clone(), bench);
        let at = commands.node_address();
        let handed_over = || done.lock().expect("the record").clone();

        match ask(
            &ground,
            at,
            &Command::StopNode(NodeCommand::stop(1, inside)),
        ) {
            Some(Telemetry::StopNode(tm)) => {
                assert_eq!(tm.header.status, CommandStatus::Success);
                assert_eq!(tm.node, inside, "the answer says which node");
            }
            other => panic!("expected a stop answer, got {other:?}"),
        }
        assert!(
            !states.is_enabled("inside"),
            "it stops where the command arrives, not when the pass gets to it"
        );
        assert!(states.is_enabled("average"), "and only the one named");
        assert_eq!(
            handed_over(),
            vec![Work::Disable("inside".into())],
            "and the pass is asked to release what it held"
        );

        // Twice is a state and not an action.
        match ask(
            &ground,
            at,
            &Command::StopNode(NodeCommand::stop(2, inside)),
        ) {
            Some(Telemetry::StopNode(tm)) => {
                assert_eq!(
                    tm.header.status,
                    CommandStatus::Success,
                    "asked twice"
                )
            }
            other => panic!("expected a stop answer, got {other:?}"),
        }

        match ask(
            &ground,
            at,
            &Command::StartNode(NodeCommand::start(3, inside)),
        ) {
            Some(Telemetry::StartNode(tm)) => {
                assert_eq!(tm.header.status, CommandStatus::Success);
                assert_eq!(tm.node, inside);
            }
            other => panic!("expected a start answer, got {other:?}"),
        }
        assert_eq!(
            handed_over().last(),
            Some(&Work::Enable("inside".into())),
            "opening a node is the pass's work, and the flag goes up with it"
        );
    }

    /// A ping node is neither stopped nor started: both are answered with a
    /// bad status, and the node is left running with nothing handed over.
    #[test]
    fn a_ping_node_is_neither_stopped_nor_started() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("ping.yaml");
        std::fs::write(
            &path,
            "moc:\n\
             \x20 - { name: ground, type: moc, address: 127.0.0.1, port: \
             \"5000\", ground_base: \"127.0.0.1:5100\", moc-out: pinger }\n\
             vehicle:\n\
             \x20 - { name: pinger, type: ping, payload-out: ground }\n",
        )
        .expect("written");
        let graph = Graph::load(&path).expect("it reads");
        let states = Arc::new(NodeStates::of(&graph).expect("states"));
        let pinger =
            NodeId::of(tcslibgs::graph::telemetry::ids(&graph)["pinger"]);

        let (bench, done) = a_pass_answering(|_| Ok(()));
        let (ground, commands) = started_with(states.clone(), bench);
        let at = commands.node_address();

        match ask(
            &ground,
            at,
            &Command::StopNode(NodeCommand::stop(1, pinger)),
        ) {
            Some(Telemetry::StopNode(tm)) => {
                assert_eq!(tm.header.status, CommandStatus::InvalidCommand);
                assert_eq!(tm.node, pinger, "the answer says which node");
            }
            other => panic!("expected a stop answer, got {other:?}"),
        }
        assert!(states.is_enabled("pinger"), "a refused stop stops nothing");

        match ask(
            &ground,
            at,
            &Command::StartNode(NodeCommand::start(2, pinger)),
        ) {
            Some(Telemetry::StartNode(tm)) => {
                assert_eq!(tm.header.status, CommandStatus::InvalidCommand);
                assert_eq!(tm.node, pinger);
            }
            other => panic!("expected a start answer, got {other:?}"),
        }
        assert!(states.is_enabled("pinger"));
        assert_eq!(
            *done.lock().expect("the record"),
            Vec::<Work>::new(),
            "and the pass is asked to do nothing"
        );
    }

    /// An attribute the configuration binds is set, and one it does not is
    /// refused.
    ///
    /// The work goes to the pass, as enabling and releasing do: what a
    /// SET_NODE changes is a node's configuration, and a node whose setpoint
    /// moved in the middle of a computation would be doing half of one thing
    /// and half of another. The answer waits for it, so a success means the
    /// node is running on the new value.
    ///
    /// Both ways of being refused come back as `InvalidParameter` -- an
    /// attribute no binding mentions and a value the node will not take --
    /// because from the ground's side they are the same thing: the
    /// configuration does not allow what was asked. The log tells them apart.
    #[test]
    fn an_attribute_the_configuration_binds_is_set() {
        use tcslibgs::SetNodeCommand;

        let graph = two_nodes();
        let states = Arc::new(NodeStates::of(&graph).expect("states"));
        let ids = tcslibgs::graph::telemetry::ids(&graph);
        let average = NodeId::of(ids["average"]);

        // The pass stands in, and refuses what a fabric would refuse: an
        // attribute no binding mentions.
        let (bench, done) = a_pass_answering(|work| match work {
            Work::Set { attribute, .. } if attribute == "setpoint" => Ok(()),
            Work::Set {
                node, attribute, ..
            } => Err(format!(
                "no command binding says the ground may set \
                 \"{node}.{attribute}\""
            )),
            _ => Ok(()),
        });
        let (ground, commands) = started_with(states, bench);
        let at = commands.node_address();

        let set = |sequence, attribute: &str, value: &[u8]| {
            Command::SetNode(
                SetNodeCommand::new(sequence, average, attribute, value)
                    .expect("it fits"),
            )
        };

        match ask(&ground, at, &set(1, "setpoint", b"295.0")) {
            Some(Telemetry::SetNode(tm)) => {
                assert_eq!(tm.header.status, CommandStatus::Success);
                assert_eq!(tm.node, average, "the answer says which node");
                assert!(
                    tm.reason.is_empty(),
                    "a SET_NODE done says nothing of why"
                );
            }
            other => panic!("expected a set answer, got {other:?}"),
        }
        assert_eq!(
            done.lock().expect("the record").last(),
            Some(&Work::Set {
                node: "average".into(),
                attribute: tcslibgs::Attribute::of("setpoint").expect("a name"),
                value: tcslibgs::Value::of(b"295.0").expect("a value"),
            }),
            "the pass is asked to set it, and told what to"
        );

        match ask(&ground, at, &set(2, "address", b"0x49")) {
            Some(Telemetry::SetNode(tm)) => {
                assert_eq!(
                    tm.header.status,
                    CommandStatus::InvalidParameter,
                    "an attribute no binding mentions"
                );
                assert_eq!(tm.node, average);
                // And why, as the pass said it: the ground is told which word
                // to change, not only that something was refused.
                assert_eq!(
                    tm.reason.as_str(),
                    "no command binding says the ground may set \
                     \"average.address\""
                );
            }
            other => panic!("expected a set answer, got {other:?}"),
        }

        // And an id the configuration does not have, which is answered
        // before any work is handed over.
        let nowhere = NodeId::of(99);
        match ask(
            &ground,
            at,
            &Command::SetNode(
                SetNodeCommand::new(3, nowhere, "setpoint", b"295.0")
                    .expect("it fits"),
            ),
        ) {
            Some(Telemetry::SetNode(tm)) => {
                assert_eq!(tm.header.status, CommandStatus::NotFound);
                assert_eq!(tm.node, nowhere, "the id the command carried");
                assert!(
                    tm.reason.as_str().contains("no node #99"),
                    "the reason names the id: {}",
                    tm.reason
                );
            }
            other => panic!("expected a set answer, got {other:?}"),
        }
        assert_eq!(
            done.lock().expect("the record").len(),
            2,
            "nothing is handed over for a node that is not there"
        );
    }

    /// A node that will not open is answered with the failure and left
    /// stopped.
    ///
    /// Enabling a node is the same work as starting it at startup and can fail
    /// the same ways: a device that is gone, an address already taken, a
    /// program that will not execute. What must not happen is a node reported
    /// started that is not running.
    #[test]
    fn a_node_that_will_not_open_is_answered_with_the_failure() {
        let graph = two_nodes();
        let states = Arc::new(NodeStates::of(&graph).expect("states"));
        let ids = tcslibgs::graph::telemetry::ids(&graph);
        let inside = NodeId::of(ids["inside"]);

        let (bench, _) = a_pass_answering(|work| match work {
            Work::Enable(_) => Err("/dev/i2c-1 is not there".to_string()),
            _ => Ok(()),
        });
        let (ground, commands) = started_with(states.clone(), bench);
        let at = commands.node_address();

        // Stopped first, so that starting it is a real opening.
        ask(
            &ground,
            at,
            &Command::StopNode(NodeCommand::stop(1, inside)),
        );
        assert!(!states.is_enabled("inside"));

        match ask(
            &ground,
            at,
            &Command::StartNode(NodeCommand::start(2, inside)),
        ) {
            Some(Telemetry::StartNode(tm)) => {
                assert_eq!(tm.header.status, CommandStatus::Failure);
                assert_eq!(tm.node, inside);
            }
            other => panic!("expected a start answer, got {other:?}"),
        }
        assert!(
            !states.is_enabled("inside"),
            "a node that would not open stays stopped"
        );
    }

    /// An id the configuration does not have is answered, with the id it
    /// carried.
    ///
    /// Answered rather than ignored: a fixed-length field holds any number, so
    /// this is a thing to say something about, and what the ground needs to
    /// see is which id it sent.
    #[test]
    fn an_id_this_configuration_does_not_have_is_answered() {
        let graph = two_nodes();
        let states = Arc::new(NodeStates::of(&graph).expect("states"));
        let (ground, commands) = started(states);
        let at = commands.node_address();

        let nowhere = NodeId::of(99);
        match ask(
            &ground,
            at,
            &Command::StopNode(NodeCommand::stop(4, nowhere)),
        ) {
            Some(Telemetry::StopNode(tm)) => {
                assert_eq!(tm.header.status, CommandStatus::NotFound);
                assert_eq!(tm.node, nowhere, "the id the command carried");
            }
            other => panic!("expected a stop answer, got {other:?}"),
        }
    }

    /// A command on the wrong link is refused, and refused as the kind it is.
    ///
    /// The telemetry type has to match the command, because that is how the
    /// ground pairs an answer with what it asked: a refusal of the wrong type
    /// is an answer the ground cannot place, and the command then looks
    /// unanswered.
    #[test]
    fn a_command_on_the_wrong_link_is_refused() {
        let graph = two_nodes();
        let states = Arc::new(NodeStates::of(&graph).expect("states"));
        let (ground, commands) = started(states.clone());

        // A node command on the spacecraft link.
        let node = NodeId::of(0);
        match ask(
            &ground,
            commands.spacecraft_address(),
            &Command::StopNode(NodeCommand::stop(5, node)),
        ) {
            Some(Telemetry::StopNode(tm)) => {
                assert_eq!(tm.header.status, CommandStatus::InvalidCommand);
                assert_eq!(tm.node, node);
            }
            other => panic!("expected a stop answer, got {other:?}"),
        }
        assert!(
            states.is_enabled("inside"),
            "a refused command must not have done the thing"
        );

        // And a spacecraft command on the node link.
        match ask(
            &ground,
            commands.node_address(),
            &Command::Ping(tcslibgs::PingCommand::new(6)),
        ) {
            Some(Telemetry::Ping(tm)) => {
                assert_eq!(tm.header.status, CommandStatus::InvalidCommand)
            }
            other => panic!("expected a ping answer, got {other:?}"),
        }
    }

    /// A command from anywhere but the stated address is ignored, and nothing
    /// is sent back.
    ///
    /// Nothing back on purpose: an answer would tell whoever sent it that
    /// something is listening here.
    #[test]
    fn a_command_from_a_stranger_is_ignored() {
        let graph = two_nodes();
        let states = Arc::new(NodeStates::of(&graph).expect("states"));
        let (_ground, commands) = started(states.clone());

        let stranger = UdpSocket::bind("127.0.0.1:0").expect("another socket");
        stranger
            .set_read_timeout(Some(Duration::from_millis(300)))
            .expect("a timeout");

        let asked =
            Command::StopNode(NodeCommand::stop(9, NodeId::of(0))).to_bytes();
        stranger
            .send_to(&asked, commands.node_address())
            .expect("it is sent");

        let mut buffer = [0u8; 64];
        assert!(
            stranger.recv_from(&mut buffer).is_err(),
            "nothing is sent back to a stranger"
        );
        assert!(
            states.is_enabled("inside"),
            "and the command is not acted on"
        );
    }

    /// A RESTART is taken with the key of the last RESTART_ARM, and only
    /// with it, and is asked for only once its answer has gone.
    ///
    /// Through the links, as the ground sends them, with a restart that
    /// counts rather than restarts: what is tested is when the interpreter
    /// asks for one, and running a test binary again is not that.
    #[test]
    fn a_restart_is_taken_only_with_the_arms_key() {
        static ASKED: std::sync::atomic::AtomicU32 =
            std::sync::atomic::AtomicU32::new(0);
        fn counted() {
            ASKED.fetch_add(1, Ordering::SeqCst);
        }

        let graph = two_nodes();
        let states = Arc::new(NodeStates::of(&graph).expect("states"));
        let ground = UdpSocket::bind("127.0.0.1:0").expect("a ground socket");
        ground
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a timeout");
        let (bench, _) = a_pass_answering(|_| Ok(()));
        let commands = Commands::start(
            &links_for(&ground),
            states,
            bench,
            flying(),
            None,
            TelemetryLog::disabled(),
            counted,
        )
        .expect("both links bind");
        let at = commands.spacecraft_address();
        let restart = |sequence, key| match ask(
            &ground,
            at,
            &Command::Restart(tcslibgs::RestartCommand::new(
                sequence,
                ArmKey(key),
            )),
        ) {
            Some(Telemetry::Restart(tm)) => tm.header.status,
            other => panic!("expected a restart answer, got {other:?}"),
        };

        // Not armed.
        assert_eq!(restart(1, 42), CommandStatus::NotArmed);

        match ask(
            &ground,
            at,
            &Command::RestartArm(tcslibgs::RestartArmCommand::new(
                2,
                ArmKey(42),
            )),
        ) {
            Some(Telemetry::RestartArm(tm)) => {
                assert_eq!(tm.header.status, CommandStatus::Success)
            }
            other => panic!("expected an arm answer, got {other:?}"),
        }

        // Armed, and the wrong key: refused, and the arm stands.
        assert_eq!(restart(3, 41), CommandStatus::NotArmed);
        assert_eq!(
            ASKED.load(Ordering::SeqCst),
            0,
            "a refused RESTART restarts nothing"
        );

        // The right key: taken, once.
        assert_eq!(restart(4, 42), CommandStatus::Success);
        assert_eq!(
            ASKED.load(Ordering::SeqCst),
            1,
            "the restart was asked for"
        );

        // And the arm is used up.
        assert_eq!(restart(5, 42), CommandStatus::NotArmed);
        assert_eq!(ASKED.load(Ordering::SeqCst), 1);
    }

    /// An arm lasts as long as the configuration's `restart_arm_valid` and
    /// no longer: within it a RESTART is taken, after it refused.
    ///
    /// Asked of the interpreter directly, with arms taken a known while ago,
    /// against a length the test chooses: a test that waited twenty minutes
    /// would not be run.
    #[test]
    fn an_arm_lasts_as_long_as_the_configuration_says() {
        let graph = two_nodes();
        let serving = |valid: Duration| {
            let (bench, _) = a_pass_answering(|_| Ok(()));
            Serving {
                states: Arc::new(NodeStates::of(&graph).expect("states")),
                bench,
                flying: flying(),
                beacon: None,
                log: TelemetryLog::disabled(),
                stopping: Arc::new(AtomicBool::new(false)),
                wakeup: Arc::new(Wakeup::new().expect("a wakeup")),
                answer: AnswerPacker::new(),
                armed: None,
                arm_valid: valid,
                restart: || {},
                restarting: false,
            }
        };
        let restart = |serving: &mut Serving, ago: Duration| {
            let taken = Instant::now()
                .checked_sub(ago)
                .expect("a machine up longer than a few seconds");
            serving.armed = Some((ArmKey(7), taken));
            serving
                .answer(
                    CommandLink::Spacecraft,
                    &Command::Restart(tcslibgs::RestartCommand::new(
                        1,
                        ArmKey(7),
                    ))
                    .for_generation(GENERATION),
                )
                .status()
        };

        // Two seconds ago, against an arm that lasts one: lapsed.
        let mut short = serving(Duration::from_secs(1));
        assert_eq!(
            restart(&mut short, Duration::from_secs(2)),
            CommandStatus::NotArmed
        );
        assert!(!short.restarting, "a lapsed arm restarts nothing");

        // The same two seconds, against one that lasts a minute: taken.
        let mut long = serving(Duration::from_secs(60));
        assert_eq!(
            restart(&mut long, Duration::from_secs(2)),
            CommandStatus::Success
        );
        assert!(long.restarting, "an arm within its time restarts");
    }
}
