//! The ground's commands, answered on a thread of their own.
//!
//! A version 2 vehicle takes commands the way a version 1 one does -- two UDP
//! links, each from the one address the configuration states -- and answers
//! them from a thread that is not the thread making the pass. That is the
//! whole reason this is a module and not a branch inside the loop: a command
//! interpreter that took its turn inside a pass would answer at the rate of
//! the slowest group in the configuration, and the one command that matters
//! most -- stop that node -- would be the one waiting on it.
//!
//! # The two links
//!
//! One for the spacecraft and one for commands about nodes, as version 1 has,
//! and for version 1's reason: what the ground needs in a hurry must never be
//! queued behind what it asked for at leisure, and a RESTART waiting behind a
//! node's statistics is a vehicle that cannot be rescued while it is busy. On
//! a space link the two become two virtual channels, which is where the
//! priority between them is really decided; over IP they are two ports.
//!
//! Where version 2 differs is how many sockets the second link is. Version 1
//! bound one per payload, which did two things: it said which payload a
//! command was about, and it kept a command that takes time from delaying a
//! command about another payload. The node id in the message does the first
//! now -- see [`tcslibgs::NodeId`] -- so the link is one socket, at
//! `payload_command.dest_base` with nothing added to it. A graph has more
//! nodes than a payload set has payloads, so a socket apiece would spend ports
//! on computations and logs that nothing addresses from the ground.
//!
//! The check version 1 made by binding a socket per payload -- a command about
//! one payload arriving on another's is one end reading a different
//! configuration -- is not lost with the sockets. It is made earlier and for
//! every command: the ids two ends use are the same ids exactly when their
//! digests agree, and CONNECT is what establishes that.
//!
//! # One thread, waiting on both
//!
//! Both sockets are in one poll set, with the stop descriptor beside them.
//! The thread therefore waits for a command or for the stop and for nothing
//! else: it wakes when there is something to do, and a vehicle whose ground
//! station has gone quiet costs it nothing. Version 1 gave each link a thread
//! and a read timeout, which is a wakeup every two hundred milliseconds per
//! link to ask a question whose answer is almost always no.
//!
//! # What it does not do on this thread
//!
//! A node's work. Disabling a node is a store and a poke -- see
//! [`crate::runtime::node_state`] -- which is why it can be answered here and
//! at once, and that is the point: a disable must not wait for the operation
//! the node is in the middle of. Work that has to happen between stages of a
//! pass belongs to the pass, and the answer goes out when the work is done.

use std::net::{SocketAddr, UdpSocket};
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use log::{error, trace, warn};
use nix::poll::{poll, PollFd, PollFlags};
use std::os::fd::BorrowedFd;

use tcslibgs::config_digest::{ConfigDigest, ConfigVersion};
use tcslibgs::{
    answer_to, came_from, CIConfig, Command, CommandLink, CommandStatus, ConfigTelemetry,
    ConnectTelemetry, HostPort, NodeTelemetry, PingTelemetry, TcsError, TcsResult, Telemetry,
};

use crate::beacon_send::BeaconSend;
use crate::runtime::node_state::{NodeStates, Wakeup};
use crate::telemetry_log::TelemetryLog;

/// The most a command datagram may be.
///
/// A whole datagram, because a command that arrived truncated would be read as
/// a different command or as none: the buffer is the largest a UDP payload can
/// be, which is what version 1's links use.
const COMMAND_BYTES: usize = 65535;

/// What this version calls each of the two links.
///
/// [`CommandLink::spelling`] says "payload command", which is what the link is
/// in version 1 and what a version 1 message calls it. A version 2 graph has
/// no payloads to command: the same link carries commands about nodes, so a
/// message from a version 2 vehicle says so. The wire is unaffected -- this is
/// the word in a log line somebody reads.
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
/// The beacons carry the same three facts, from the same place.
#[derive(Debug, Clone, Copy)]
pub struct Flying {
    /// The version of this build.
    pub version: ConfigVersion,
    /// The version the configuration file states.
    pub config_version: ConfigVersion,
    /// The digest of the configuration file this process read.
    pub digest: ConfigDigest,
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
        flying: Flying,
        beacon: Option<BeaconSend>,
        log: TelemetryLog,
    ) -> TcsResult<Commands> {
        let spacecraft = Link {
            socket: bound(
                "the spacecraft command link",
                // Its own two fields rather than a HostPort, which is how
                // version 1's own configuration states it: `tcs_cmd.recv`
                // becomes the address and the port beside each other.
                &HostPort {
                    address: ci.address.clone(),
                    port: ci.port,
                },
            )?,
            which: CommandLink::Spacecraft,
            from: ci.moc_cmd_send.clone(),
        };
        let nodes = Link {
            socket: bound("the node command link", &ci.payload_command_dest_base)?,
            which: CommandLink::Payload,
            from: ci.payload_command_source_base.clone(),
        };

        let spacecraft_at = spacecraft.socket.local_addr()?;
        let nodes_at = nodes.socket.local_addr()?;

        let stopping = Arc::new(AtomicBool::new(false));
        let wakeup = Arc::new(Wakeup::new()?);

        let serving = Serving {
            states,
            flying,
            beacon,
            log,
            stopping: stopping.clone(),
            wakeup: wakeup.clone(),
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
    flying: Flying,
    /// The beacon sender, so that a CONFIG retimes the one that is running.
    /// `None` for a configuration that sends no beacons.
    beacon: Option<BeaconSend>,
    /// Where every answer is recorded, which is the log the beacons use: one
    /// log holds everything that went to the ground.
    log: TelemetryLog,
    stopping: Arc<AtomicBool>,
    wakeup: Arc<Wakeup>,
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
            let borrowed: Vec<BorrowedFd> = links
                .iter()
                .map(|link| unsafe { BorrowedFd::borrow_raw(link.socket.as_raw_fd()) })
                .chain(std::iter::once(unsafe {
                    BorrowedFd::borrow_raw(self.wakeup.io_fd())
                }))
                .collect();
            let mut set: Vec<PollFd> = borrowed
                .iter()
                .map(|fd| PollFd::new(fd, PollFlags::POLLIN))
                .collect();

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

        let command = match serde_json::from_slice::<Command>(&buffer[..size]) {
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

        trace!("{}: {command:?}", spelling(link.which));
        let answer = self.answer(link.which, &command);

        if let Ok(data) = serde_json::to_vec(&answer) {
            // Recorded before it is sent, as a beacon is, so that an answer
            // the send fails on is still known to have been produced.
            self.log.record(&data);
            // Back to the address it came from, which is version 1's rule: a
            // datagram's sender is the only statement of where the ground is,
            // and a configuration that stated it as well would be a second
            // answer that can disagree.
            let _ = link.socket.send_to(&data, from);
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
                "{:?} arrived on the {} link and belongs on the {} one: the two \
                 ends are not reading the same configuration",
                command.cmd_type(),
                spelling(arrived),
                spelling(command.link())
            );
            return answer_to(command, CommandStatus::InvalidCommand);
        }

        match command {
            Command::Ping(_) => {
                Telemetry::Ping(PingTelemetry::new(sequence, CommandStatus::Success))
            }

            // The three facts this vehicle is flying, which is what licenses
            // every command that carries a node id: a ground station whose
            // digest does not match does not send them.
            Command::Connect(_) => Telemetry::Connect(ConnectTelemetry::new(
                sequence,
                self.flying.version,
                self.flying.config_version,
                self.flying.digest,
            )),

            // The beacon thread is retimed where it waits, so the next beacon
            // goes at the new interval rather than after the old one elapses.
            Command::Config(cmd) => {
                if let Some(beacon) = self.beacon.as_mut() {
                    beacon.set_interval(cmd.beacon_interval.duration());
                }
                Telemetry::Config(ConfigTelemetry::new(sequence, CommandStatus::Success))
            }

            Command::StartNode(cmd) => {
                let status = self.set_state(cmd.node, true);
                Telemetry::StartNode(NodeTelemetry::started(sequence, status, cmd.node))
            }
            Command::StopNode(cmd) => {
                let status = self.set_state(cmd.node, false);
                Telemetry::StopNode(NodeTelemetry::stopped(sequence, status, cmd.node))
            }

            // Not answered yet, and refused rather than left unanswered. A
            // command the vehicle does not implement is a thing the ground can
            // see and work around; a command that goes quiet is a link the
            // ground cannot tell from a dead one.
            //
            // RESTART_ARM and RESTART are next: what a version 2 restart does
            // -- end the process and leave it to whatever supervises it -- is
            // a decision about how a vehicle is run and not only about this
            // module.
            //
            // The five DH commands are version 1's, naming a data handler by
            // name. A graph has nodes, and the ones that answer for a node are
            // START_NODE and STOP_NODE above; QUERY, QUERY_SAMPLE and CONFIG
            // for a node want per-node counters and the pass's hand in the
            // work, neither of which is here yet.
            Command::RestartArm(_)
            | Command::Restart(_)
            | Command::StartDH(_)
            | Command::StopDH(_)
            | Command::QueryDH(_)
            | Command::QueryDHSample(_)
            | Command::ConfigDH(_) => {
                warn!(
                    "{:?} is not answered by a version 2 vehicle yet",
                    command.cmd_type()
                );
                answer_to(command, CommandStatus::InvalidCommand)
            }
        }
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
    fn set_state(&self, node: tcslibgs::NodeId, enabled: bool) -> CommandStatus {
        let Some(state) = self.states.at(node) else {
            error!(
                "there is no node {node} in this configuration, which has {}: the \
                 ground is reading a configuration this vehicle is not flying",
                self.states.len()
            );
            return CommandStatus::NotFound;
        };
        if enabled {
            state.enable();
        } else {
            state.disable();
        }
        CommandStatus::Success
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
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
            beacon_send: here(0),
            beacon_recv: here(0),
            // Nothing to write: what is being tested is what goes out.
            log_dir: None,
            log_segment_bytes: 65_536,
        }
    }

    /// Two nodes, so that an id can be right and an id can be out of range.
    fn two_nodes() -> Graph {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("two.yaml");
        std::fs::write(
            &path,
            "version: \"2.0\"\n\
             payloads:\n\
             \x20 - { name: inside, type: i2c, path: /dev/i2c-1, address: \"0x48\", \
             poll_interval: \"1 s\", payload-out: average }\n\
             vehicle:\n\
             \x20 - { name: average, type: compute, function: mean, inputs: 1 }\n",
        )
        .expect("written");
        Graph::load(&path).expect("it reads")
    }

    fn flying() -> Flying {
        Flying {
            version: ConfigVersion::of_this_build(),
            config_version: ConfigVersion::of_text("2.0").expect("a version"),
            digest: ConfigDigest([0x5Au8; 16]),
        }
    }

    /// Ask on a link and read the answer, or say nothing came.
    fn ask(ground: &UdpSocket, at: SocketAddr, command: &Command) -> Option<Telemetry> {
        let asked = serde_json::to_vec(command).expect("a command");
        ground.send_to(&asked, at).expect("it is sent");
        let mut buffer = [0u8; COMMAND_BYTES];
        loop {
            match ground.recv_from(&mut buffer) {
                Ok((size, _)) => {
                    return Some(
                        serde_json::from_slice::<Telemetry>(&buffer[..size]).expect("telemetry"),
                    )
                }
                // A child of this test binary exiting interrupts a blocking
                // receive; the links' own loops wait again for the same
                // reason.
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return None,
            }
        }
    }

    fn started(states: Arc<NodeStates>) -> (UdpSocket, Commands) {
        let ground = UdpSocket::bind("127.0.0.1:0").expect("a ground socket");
        ground
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a timeout");
        let commands = Commands::start(
            &links_for(&ground),
            states,
            flying(),
            None,
            TelemetryLog::disabled(),
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
                assert_eq!(tm.header.sequence, 7, "the answer carries the command's");
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
                assert_eq!(tm.digest, ConfigDigest([0x5Au8; 16]), "the file it read");
                assert_eq!(tm.config_version, ConfigVersion::of_text("2.0").unwrap());
            }
            other => panic!("expected a connect answer, got {other:?}"),
        }
    }

    /// A STOP_NODE disables that node and nothing else, and a START_NODE
    /// enables it again.
    ///
    /// By id, which is the node's position: the ground shows the name and the
    /// link carries the number.
    #[test]
    fn a_node_is_stopped_and_started_by_its_id() {
        let graph = two_nodes();
        let states = Arc::new(NodeStates::of(&graph).expect("states"));
        let ids = tcslibgs::graph::telemetry::ids(&graph);
        let inside = NodeId::of(ids["inside"]);

        let (ground, commands) = started(states.clone());
        let at = commands.node_address();

        match ask(&ground, at, &Command::StopNode(NodeCommand::stop(1, inside))) {
            Some(Telemetry::StopNode(tm)) => {
                assert_eq!(tm.header.status, CommandStatus::Success);
                assert_eq!(tm.node, inside, "the answer says which node");
            }
            other => panic!("expected a stop answer, got {other:?}"),
        }
        assert!(!states.is_enabled("inside"), "the pass must not touch it");
        assert!(states.is_enabled("average"), "and only the one named");

        // Twice is a state and not an action.
        match ask(&ground, at, &Command::StopNode(NodeCommand::stop(2, inside))) {
            Some(Telemetry::StopNode(tm)) => {
                assert_eq!(tm.header.status, CommandStatus::Success, "asked twice")
            }
            other => panic!("expected a stop answer, got {other:?}"),
        }

        match ask(&ground, at, &Command::StartNode(NodeCommand::start(3, inside))) {
            Some(Telemetry::StartNode(tm)) => {
                assert_eq!(tm.header.status, CommandStatus::Success);
                assert_eq!(tm.node, inside);
            }
            other => panic!("expected a start answer, got {other:?}"),
        }
        assert!(states.is_enabled("inside"));
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
        match ask(&ground, at, &Command::StopNode(NodeCommand::stop(4, nowhere))) {
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

        let asked = serde_json::to_vec(&Command::StopNode(NodeCommand::stop(9, NodeId::of(0))))
            .expect("a command");
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
}
