//! TCSpecial client for ground software integration
//!
//! This is what a control application built on tcslib talks to tcspecial
//! through: one set of operations for global control and status, and one set
//! per data handler. A mission control system such as YAMCS or MCT drives a
//! [`TcsClient`] rather than speaking the command and telemetry protocol
//! itself.
//!
//! The transport is a [`Connection`]. The link to tcspecial is UDP, and
//! [`UdpConnection`](crate::UdpConnection) is the one a client is built on;
//! the trait is what lets the tests here stand a canned spacecraft in its
//! place.
//!
//! There are two of them, because the spacecraft takes commands on two links:
//! what the ground needs in a hurry must never be queued behind what it asked
//! for at leisure. Which command goes on which is not this module's decision
//! -- it is [`tcslibgs::CommandType::link`], the one table both ends read.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use tcslibgs::{
    sequence_is_before, ArmKey, Command, CommandLink, CommandStatus,
    CommandType, ConfigDigest, ConfigVersion, ConnectCommand, DHSample,
    NodeCommand, NodeCounts, NodeId, PingCommand, Reason, RestartArmCommand,
    RestartCommand, SetNodeCommand, TcsError, TcsResult, Telemetry, Timestamp,
};

use crate::connection::Connection;
use crate::matched::Matched;

/// Default timeout for command responses
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// The two command links a configuration's `tcspecial` section states, from
/// the ground's end.
///
/// Read from the file rather than stated again by a ground station, because
/// tcspecial takes commands only from where its configuration says the MOC
/// sends them: a station sending from anywhere else is ignored, and says
/// nothing about why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandLinks {
    /// Where the spacecraft's link is sent from: `tcs_cmd.send`.
    pub from: String,
    /// Where the vehicle's command interpreter takes it: the section's
    /// address and port.
    pub interpreter: String,
    /// Where node commands are sent from: `payload_command.source_base`.
    pub nodes_from: tcslibgs::HostPort,
    /// And where they go: `payload_command.dest_base`, one socket for every
    /// node, a command carrying the node's id.
    pub nodes_to: tcslibgs::HostPort,
}

/// Reads the command links of a configuration.
///
/// One reading of them, here, for every ground station: tcsmoc and the C/C++
/// library alike.
///
/// # Arguments
///
/// * `graph` - The configuration's graph.
///
/// # Returns
///
/// The links, or `None` for a configuration with no `tcspecial` section,
/// which states no ports and so nowhere to command. For a configuration
/// that answers to cFS they are the loopback links of this process's peer
/// of the bus, started here if it was not already: see [`crate::bus`].
///
/// # Errors
///
/// A message saying what is wrong with the `tcspecial` section's command
/// interpreter settings, or why the peer of a bus cannot be started.
pub fn command_links_of(
    graph: &tcslibgs::graph::Graph,
) -> Result<Option<CommandLinks>, String> {
    use tcslibgs::graph::parameters::{ci_config_of, flatten};

    let Some(section) = graph.tcspecial.as_ref() else {
        return Ok(None);
    };
    let ci = ci_config_of(&flatten(section))?.to_ci_config()?;
    let Some(ci) = ci.sockets() else {
        // Over a software bus: the station's peer of it, whose loopback
        // links are commanded as the UDP ones are. See `crate::bus`.
        let Some(ground) = crate::bus::Ground::of(graph)? else {
            return Ok(None);
        };
        let anywhere = tcslibgs::HostPort {
            address: "127.0.0.1".to_string(),
            port: 0,
        };
        return Ok(Some(CommandLinks {
            from: anywhere.to_string(),
            interpreter: ground.spacecraft().to_string(),
            nodes_from: anywhere,
            nodes_to: tcslibgs::HostPort {
                address: ground.nodes().ip().to_string(),
                port: ground.nodes().port(),
            },
        }));
    };
    Ok(Some(CommandLinks {
        from: ci.moc_cmd_send.to_string(),
        interpreter: format!("{}:{}", ci.address, ci.port),
        nodes_from: ci.payload_command_source_base.clone(),
        nodes_to: ci.payload_command_dest_base.clone(),
    }))
}

/// TCSpecial client for sending commands and receiving telemetry
pub struct TcsClient {
    /// The spacecraft's own link: ping it, connect, arm it, restart it.
    connection: Box<dyn Connection>,
    /// The one link every node is commanded on.
    ///
    /// One socket and not one per node: a node command carries the node's id
    /// -- see [`Command::node_id`] -- so the message says which node and a
    /// socket apiece would only say it again. A graph also has nodes that are
    /// not payloads, so a socket each would spend ports on computations and
    /// logs that nothing addresses from the ground.
    ///
    /// `None` for a client that has not been given one, which can command the
    /// spacecraft and no node of it.
    nodes: Option<Box<dyn Connection>>,
    sequence: AtomicU32,
    timeout: Duration,
    /// When the last command went out, and when its answer came back.
    ///
    /// Kept here because this is where every command passes: a caller that
    /// wanted to time them would otherwise have to remember at each of its
    /// own call sites, and the one it forgot would be the one that hung.
    /// `received` stays as it was when an answer does not come, so the pair
    /// says *sent at this time, nothing back since* rather than blanking.
    last_sent: Option<Timestamp>,
    last_received: Option<Timestamp>,
    /// How many answers arrived that were not to the command being waited
    /// for, and were dropped: see [`answer_to`].
    answers_dropped: u64,
    /// Whether the station is matched with the vehicle, which its beacon
    /// watch keeps: until it is matched for commanding, nothing but
    /// CONNECT, RESTART_ARM and RESTART is sent. Every answer that arrives
    /// is counted here, with the station's data.
    ///
    /// Nothing is queued here to throw away when the match ends. A command
    /// is built when its method is called and sent at once, so "a command
    /// built under the old configuration" is a node id or an attribute the
    /// caller took from that configuration and has not yet passed in: the
    /// caller takes it again from the configuration it now uses, and the
    /// refusal here stops it in the meantime.
    matched: Matched,
}

impl TcsClient {
    /// Create a client: the spacecraft's link, and the one link every node is
    /// commanded on.
    ///
    /// Two links, because what the ground needs in a hurry must never be
    /// queued behind what it asked for at leisure. The second is one socket
    /// for every node, at `payload_command.dest_base` with nothing added to
    /// it, because a command carries the node's id.
    ///
    /// It was a link per payload, and there was a constructor apiece: version
    /// 1 bound one socket for each payload, that being how a command said
    /// which payload it was about, so a client without a payload's link
    /// could not send anything about that payload.
    ///
    /// # Arguments
    ///
    /// * `connection` - The spacecraft's own link.
    /// * `nodes`      - The one link every node is commanded on.
    /// * `matched`    - Whether the station is matched with the vehicle,
    ///                  which its beacon watch keeps.
    ///
    /// # Returns
    ///
    /// The client, with the default timeout, counting from sequence 1.
    pub fn of_a_graph(
        connection: Box<dyn Connection>,
        nodes: Box<dyn Connection>,
        matched: Matched,
    ) -> Self {
        Self {
            connection,
            nodes: Some(nodes),
            sequence: AtomicU32::new(1),
            timeout: DEFAULT_TIMEOUT,
            last_sent: None,
            last_received: None,
            answers_dropped: 0,
            matched,
        }
    }

    /// Opens a client on a configuration's two command links, over UDP.
    ///
    /// # Arguments
    ///
    /// * `links`   - Where each link is sent from and goes to, as
    ///               [`command_links_of`] reads them.
    /// * `matched` - Whether the station is matched with the vehicle, which
    ///               its beacon watch keeps.
    ///
    /// # Returns
    ///
    /// The client.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if a link's address cannot be bound, and
    /// `TcsError::Config` if where one goes is not a socket address.
    pub fn open(links: &CommandLinks, matched: Matched) -> TcsResult<Self> {
        let connection =
            crate::UdpConnection::new(&links.from, &links.interpreter)?;
        let nodes = crate::UdpConnection::new(
            &links.nodes_from.to_string(),
            &links.nodes_to.to_string(),
        )?;
        Ok(Self::of_a_graph(
            Box::new(connection),
            Box::new(nodes),
            matched,
        ))
    }

    /// A client with the spacecraft's link and no node link.
    ///
    /// For a program that commands the vehicle itself and no node of it: a
    /// PING, a CONNECT, a RESTART. A node command is refused rather than
    /// sent -- see `link_for` -- because there is nowhere for it to go.
    ///
    /// # Arguments
    ///
    /// * `connection` - The spacecraft's own link.
    /// * `matched`    - Whether the station is matched with the vehicle,
    ///                  which its beacon watch keeps.
    ///
    /// # Returns
    ///
    /// The client, with the default timeout, counting from sequence 1.
    pub fn spacecraft_only(
        connection: Box<dyn Connection>,
        matched: Matched,
    ) -> Self {
        Self {
            connection,
            nodes: None,
            sequence: AtomicU32::new(1),
            timeout: DEFAULT_TIMEOUT,
            last_sent: None,
            last_received: None,
            answers_dropped: 0,
            matched,
        }
    }

    /// Whether the station is matched with the vehicle, which this client's
    /// commands wait for, and what it has received.
    pub fn matched(&self) -> &Matched {
        &self.matched
    }

    /// When the last command went out, if one has.
    pub fn last_sent(&self) -> Option<Timestamp> {
        self.last_sent
    }

    /// When the last answer came back, if one has.
    pub fn last_received(&self) -> Option<Timestamp> {
        self.last_received
    }

    /// Says how many answers this client has dropped because they did not
    /// answer the command it was waiting for.
    ///
    /// # Returns
    ///
    /// The count, since the client was made.
    pub fn answers_dropped(&self) -> u64 {
        self.answers_dropped
    }

    /// Set the command timeout, on both links.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Get the next sequence number
    ///
    /// One counter for both links, so that one telemetry log -- the
    /// spacecraft keeps a single one -- reads in the order the commands were
    /// sent.
    fn next_sequence(&self) -> u32 {
        self.sequence.fetch_add(1, Ordering::SeqCst)
    }

    /// The link a command of this kind goes on.
    ///
    /// By the table both ends read, so this cannot send a command to a link
    /// the spacecraft would refuse it on. A CONNECT goes on the spacecraft
    /// link here, that being the one every caller has; [`Self::connect`] asks
    /// the other one as well.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to be sent.
    ///
    /// # Returns
    ///
    /// The link it goes on.
    ///
    /// # Errors
    ///
    /// `TcsError::Config` for a node command where the client has no node
    /// link.
    fn link_for(
        &mut self,
        command: &Command,
    ) -> TcsResult<&mut Box<dyn Connection>> {
        match command.link() {
            // Every node command on the one link, the id in the message
            // saying which node it is about.
            CommandLink::Payload => self.nodes.as_mut().ok_or_else(|| {
                TcsError::Config(format!(
                    "{:?} is about a node and this client has no link for one: \
                     it was given the spacecraft's link and nothing else",
                    command.cmd_type()
                ))
            }),
            CommandLink::Spacecraft => Ok(&mut self.connection),
        }
    }

    /// Says whether a command of kind `kind` may go: only where this station
    /// is matched for commanding, but for CONNECT, RESTART_ARM and RESTART,
    /// which go either way because they are how it matches and how it
    /// rescues a vehicle that will not.
    ///
    /// Matched for commanding is what says which start of tcspecial is
    /// flying, and with which configuration: its last beacon is recent,
    /// clear of the restart flag and carries the station's checksum, and a
    /// CONNECT of its own has been answered with that checksum since. A
    /// start's beacons carry the flag, so a start ends the match, and
    /// nothing but the three goes until the station has matched the start
    /// that came back. There is no generation to know beside it.
    ///
    /// Public so that a station can ask before it builds a command, and say
    /// the refusal its own way -- tcsmoc in a pop-up -- by the one rule
    /// every command sent here is held to.
    ///
    /// # Arguments
    ///
    /// * `kind` - What kind of command is to be sent.
    ///
    /// # Errors
    ///
    /// `TcsError::Unmatched` where the station is not matched for
    /// commanding and the command is not one of the three.
    pub fn may_go(&self, kind: CommandType) -> TcsResult<()> {
        if !kind.taken_unmatched() && !self.matched.for_commanding() {
            return Err(TcsError::Unmatched(kind));
        }
        Ok(())
    }

    /// [`TcsClient::may_go`], for a command that has been built.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to be sent.
    ///
    /// # Errors
    ///
    /// `TcsError::Unmatched` as [`TcsClient::may_go`] says.
    fn may_send(&self, command: &Command) -> TcsResult<()> {
        self.may_go(command.cmd_type())
    }

    /// Sends a command on its link, and waits for the answer, timing both.
    ///
    /// # Arguments
    ///
    /// * `command` - The command, with its sequence number.
    ///
    /// # Returns
    ///
    /// The answer, whatever kind of telemetry it is.
    ///
    /// # Errors
    ///
    /// `TcsError::Config` if the client has no link for it,
    /// `TcsError::Unmatched` if the station is not matched for commanding,
    /// and `TcsError::Io` if it cannot be sent or no answer comes in time.
    fn send_command(&mut self, command: Command) -> TcsResult<Telemetry> {
        // A command with no link to go on says so before anything is said
        // of the match: that is the client's own configuration, and no
        // beacon would change it.
        self.link_for(&command)?;
        self.may_send(&command)?;
        let timeout = self.timeout;
        let link = self.link_for(&command)?;

        // Timed here, around the one place every command goes through. The
        // times are the ground's own clock: what they are read for is how
        // long an answer took and whether one came at all, which the
        // spacecraft's own time in the answer cannot say.
        link.send(&command)?;
        self.last_sent = Some(Timestamp::now());

        let mut dropped = 0;
        let matched = self.matched.clone();
        let answer = answer_to(
            self.link_for(&command)?,
            command.sequence(),
            timeout,
            &mut dropped,
            &matched,
        );
        self.answers_dropped += dropped;
        if answer.is_ok() {
            self.last_received = Some(Timestamp::now());
        }
        answer
    }

    /// Start the count again past `newest`, the newest command the vehicle
    /// says it has taken, unless it is already past it.
    ///
    /// A station whose count has gone back -- started again, or a second
    /// station -- would otherwise have every command it sends answered
    /// `Stale`. Forward only, so that a CONNECT answered late cannot take
    /// the count back to where it was.
    ///
    /// # Arguments
    ///
    /// * `newest` - The sequence number of the newest command the vehicle
    ///              has taken.
    fn count_past(&self, newest: u32) {
        let next = self.sequence.load(Ordering::SeqCst);
        if !sequence_is_before(newest, next) {
            self.sequence
                .store(newest.wrapping_add(1), Ordering::SeqCst);
        }
    }

    // -- global control and status ----------------------------------------

    /// Sends a PING and waits for its answer.
    ///
    /// # Returns
    ///
    /// The PING's answer.
    ///
    /// # Errors
    ///
    /// `TcsError::Unmatched` if the station is not matched for commanding,
    /// `TcsError::Io` if it cannot be sent or no answer comes in time,
    /// and `TcsError::Protocol` if the answer is not this command's.
    pub fn ping(&mut self) -> TcsResult<tcslibgs::PingTelemetry> {
        let seq = self.next_sequence();
        let cmd = Command::Ping(PingCommand::new(seq));
        let response = self.send_command(cmd)?;

        match response {
            Telemetry::Ping(tm) => Ok(tm),
            _ => {
                Err(TcsError::Protocol("Unexpected telemetry type".to_string()))
            }
        }
    }

    /// Sends a RESTART_ARM and waits for its answer.
    ///
    /// The vehicle refuses one whose target it does not hold `NotFound`,
    /// and arms nothing.
    ///
    /// # Arguments
    ///
    /// * `arm_key` - The key the RESTART that follows must carry.
    /// * `target`  - The checksum of the configuration to restart onto,
    ///               which the RESTART must carry too: the running
    ///               configuration's for a plain restart.
    ///
    /// # Returns
    ///
    /// The status the vehicle answered with.
    ///
    /// # Errors
    ///
    /// `TcsError::Unmatched` if the station is not matched for commanding,
    /// `TcsError::Io` if it cannot be sent or no answer comes in time,
    /// and `TcsError::Protocol` if the answer is not this command's.
    pub fn restart_arm(
        &mut self,
        arm_key: ArmKey,
        target: ConfigDigest,
    ) -> TcsResult<CommandStatus> {
        let seq = self.next_sequence();
        let cmd =
            Command::RestartArm(RestartArmCommand::new(seq, arm_key, target));
        let response = self.send_command(cmd)?;

        match response {
            Telemetry::RestartArm(tm) => Ok(tm.header.status),
            _ => {
                Err(TcsError::Protocol("Unexpected telemetry type".to_string()))
            }
        }
    }

    /// Say which configuration this end read, and hear which the other end
    /// is running.
    ///
    /// Sent when a link is opened. What comes back is the spacecraft's own
    /// version and the checksum of the configuration it is serving, for the
    /// caller to hold against the one it sent: this returns both rather than
    /// a verdict, because the caller is the end that knows which file it read
    /// and can say so.
    ///
    /// Where the answer is a success the sequence count goes on from past
    /// the newest command the vehicle has taken.
    ///
    /// It goes before the station is matched, which is what it is for:
    /// answered with the station's own checksum, it makes the station
    /// matched for commanding once it is matched for listening -- see
    /// [`Matched::connect_answered`]. Answered with another, it matches
    /// nothing, and the vehicle changes nothing. Refused, it matches
    /// nothing either, whatever checksum it carries: a start that has
    /// taken a RESTART refuses every CONNECT.
    ///
    /// # Arguments
    ///
    /// * `digest` - The checksum of the configuration this end read.
    ///
    /// # Returns
    ///
    /// The spacecraft's version and the checksum of what it runs.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if it cannot be sent or no answer comes in time,
    /// `TcsError::Command` if the vehicle refused it, saying why, and
    /// `TcsError::Protocol` if the answer is not this command's.
    pub fn connect(
        &mut self,
        digest: ConfigDigest,
    ) -> TcsResult<(ConfigVersion, ConfigDigest)> {
        let seq = self.next_sequence();
        let command = Command::Connect(ConnectCommand::new(seq, digest));
        let timeout = self.timeout;
        self.may_send(&command)?;
        let mark = self.matched.before_connect();

        // The spacecraft's own link, and only that one. A CONNECT is about
        // having a connection to the vehicle, which is what this link is; a
        // payload's link carries what is asked of a payload, and a ground
        // station with no connection has nothing to ask one. See
        // `tcslibgs::CommandType::link`.
        self.connection.send(&command)?;
        self.last_sent = Some(Timestamp::now());

        let mut dropped = 0;
        let answer = answer_to(
            &mut self.connection,
            seq,
            timeout,
            &mut dropped,
            &self.matched,
        );
        self.answers_dropped += dropped;
        let answer = answer?;
        self.last_received = Some(Timestamp::now());

        match answer {
            // Where it answered at all, the count goes on from past the
            // newest command it has taken.
            // A refusal is not an answer to hold the checksum against: a
            // start that has taken a RESTART refuses every CONNECT, with
            // its checksum all the same, and a caller that compared it
            // would say it agreed with a start that is going.
            Telemetry::Connect(tm) if tm.header.status.is_success() => {
                self.count_past(tm.newest_taken);
                self.matched.connect_answered(tm.digest, mark);
                Ok((tm.version, tm.digest))
            }
            Telemetry::Connect(tm)
                if tm.header.status == CommandStatus::Unmatched =>
            {
                Err(TcsError::Command(
                    "CONNECT refused: tcspecial has taken a RESTART and is \
                     ending, so it matches no station. Wait for the beacons \
                     of the start that comes back, then connect again"
                        .to_string(),
                ))
            }
            Telemetry::Connect(tm) => Err(TcsError::Command(format!(
                "CONNECT refused ({:?}): nothing was matched; check that it \
                 went on the vehicle's own command link, then connect again",
                tm.header.status
            ))),
            _ => {
                Err(TcsError::Protocol("Unexpected telemetry type".to_string()))
            }
        }
    }

    /// Send a RESTART command
    ///
    /// Nothing is held back here after it goes. The vehicle that takes it
    /// answers every command but CONNECT, RESTART_ARM and RESTART
    /// `Unmatched` until it has gone, and the start that comes back does
    /// the same until a CONNECT carries its checksum, so a command sent
    /// after a RESTART is refused there rather than read with definitions
    /// it was not built with. The beacons say whether the RESTART was
    /// taken, and their restart flag ends the station's match: see "The
    /// beacon across a restart" in `docs/tcspecial-design.rst`.
    ///
    /// # Arguments
    ///
    /// * `arm_key` - The key the RESTART_ARM before it gave.
    /// * `target`  - The checksum the RESTART_ARM before it gave.
    ///
    /// # Returns
    ///
    /// The status the vehicle answered with.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if it cannot be sent or no answer comes in time,
    /// and `TcsError::Protocol` if the answer is not this command's.
    pub fn restart(
        &mut self,
        arm_key: ArmKey,
        target: ConfigDigest,
    ) -> TcsResult<CommandStatus> {
        let seq = self.next_sequence();
        let cmd = Command::Restart(RestartCommand::new(seq, arm_key, target));
        let response = self.send_command(cmd)?;

        match response {
            Telemetry::Restart(tm) => Ok(tm.header.status),
            _ => {
                Err(TcsError::Protocol("Unexpected telemetry type".to_string()))
            }
        }
    }

    // -- a graph's nodes ---------------------------------------------------

    /// Enable a node: START_NODE.
    ///
    /// By its node id, which is the node's position among the
    /// configuration's nodes -- see [`NodeId`]. What makes the two ends mean
    /// the same node by one id is the digest, so a ground station whose
    /// digest does not match the vehicle's does not send these at all; that
    /// is the caller's rule to keep, CONNECT being where the two digests are
    /// compared.
    ///
    /// The answer carries the id that was sent, so an answer can be placed
    /// without the name even if two overtake one another.
    ///
    /// # Arguments
    ///
    /// * `node` - The node's id.
    ///
    /// # Returns
    ///
    /// The status the vehicle answered with, and the id it answered for.
    ///
    /// # Errors
    ///
    /// `TcsError::Config` if the client has no node link,
    /// `TcsError::Unmatched` if the station is not matched for commanding,
    /// `TcsError::Io` if it cannot be sent or no answer comes in time,
    /// and `TcsError::Protocol` if the answer is not this command's.
    pub fn start_node(
        &mut self,
        node: NodeId,
    ) -> TcsResult<(CommandStatus, NodeId)> {
        let seq = self.next_sequence();
        let cmd = Command::StartNode(NodeCommand::start(seq, node));
        match self.send_command(cmd)? {
            Telemetry::StartNode(tm) => Ok((tm.header.status, tm.node)),
            _ => {
                Err(TcsError::Protocol("Unexpected telemetry type".to_string()))
            }
        }
    }

    /// Disable a node: STOP_NODE.
    ///
    /// Answered when the node has stopped *and* given up what it held, so a
    /// success means its device, bus address or socket is free.
    ///
    /// # Arguments
    ///
    /// * `node` - The node's id.
    ///
    /// # Returns
    ///
    /// The status the vehicle answered with, and the id it answered for.
    ///
    /// # Errors
    ///
    /// `TcsError::Config` if the client has no node link,
    /// `TcsError::Unmatched` if the station is not matched for commanding,
    /// `TcsError::Io` if it cannot be sent or no answer comes in time,
    /// and `TcsError::Protocol` if the answer is not this command's.
    pub fn stop_node(
        &mut self,
        node: NodeId,
    ) -> TcsResult<(CommandStatus, NodeId)> {
        let seq = self.next_sequence();
        let cmd = Command::StopNode(NodeCommand::stop(seq, node));
        match self.send_command(cmd)? {
            Telemetry::StopNode(tm) => Ok((tm.header.status, tm.node)),
            _ => {
                Err(TcsError::Protocol("Unexpected telemetry type".to_string()))
            }
        }
    }

    /// Set an attribute of a node: SET_NODE.
    ///
    /// Refused unless the configuration binds that attribute of that node to
    /// the ground -- the `commands` section is what says a setpoint may be
    /// set in flight -- and refused again if the node will not take the
    /// value. Both come back as `InvalidParameter`: they are the same thing
    /// from the ground's side, the configuration not allowing what was asked.
    ///
    /// The value is bytes because what an attribute takes is the node's
    /// business: a setpoint is a temperature and a band is a width, and the
    /// node is what reads them.
    ///
    /// # Arguments
    ///
    /// * `node`      - The node's id.
    /// * `attribute` - The attribute's name, as the configuration binds it.
    /// * `value`     - The value, as the node reads it.
    ///
    /// # Returns
    ///
    /// The status the vehicle answered with, the id it answered for, and
    /// the reason. The reason says why where the vehicle refused it --
    /// which attribute no binding names, with its spelling where that was
    /// what was wrong; what the node would not take -- and is empty where
    /// it did not: see [`tcslibgs::SetNodeTelemetry::reason`].
    ///
    /// # Errors
    ///
    /// An attribute longer than `ATTRIBUTE_BYTES` or a value longer than
    /// `VALUE_BYTES` is a `TcsError::Command` and is not sent: the vehicle
    /// would refuse it unanswered. Otherwise
    /// `TcsError::Config` if the client has no node link,
    /// `TcsError::Unmatched` if the station is not matched for commanding,
    /// `TcsError::Io` if it cannot be sent or no answer comes in time,
    /// and `TcsError::Protocol` if the answer is not this command's.
    pub fn set_node(
        &mut self,
        node: NodeId,
        attribute: &str,
        value: &[u8],
    ) -> TcsResult<(CommandStatus, NodeId, Reason)> {
        let seq = self.next_sequence();
        let cmd = Command::SetNode(
            SetNodeCommand::new(seq, node, attribute, value)
                .map_err(TcsError::Command)?,
        );
        match self.send_command(cmd)? {
            Telemetry::SetNode(tm) => {
                Ok((tm.header.status, tm.node, tm.reason))
            }
            _ => {
                Err(TcsError::Protocol("Unexpected telemetry type".to_string()))
            }
        }
    }

    /// What a node has moved, and whether it is running: QUERY_NODE.
    ///
    /// Both, because they are read together: counts that have stopped going
    /// up mean one thing for a node the ground disabled and another for one
    /// that is running. A disabled node answers with its counters all the
    /// same -- they say what it has done since the vehicle started, not since
    /// it was last enabled.
    ///
    /// # Arguments
    ///
    /// * `node` - The node's id.
    ///
    /// # Returns
    ///
    /// The status the vehicle answered with, whether the node is enabled,
    /// and its counts.
    ///
    /// # Errors
    ///
    /// `TcsError::Config` if the client has no node link,
    /// `TcsError::Unmatched` if the station is not matched for commanding,
    /// `TcsError::Io` if it cannot be sent or no answer comes in time,
    /// and `TcsError::Protocol` if the answer is not this command's.
    pub fn query_node(
        &mut self,
        node: NodeId,
    ) -> TcsResult<(CommandStatus, bool, NodeCounts)> {
        let seq = self.next_sequence();
        let cmd = Command::QueryNode(NodeCommand::query(seq, node));
        match self.send_command(cmd)? {
            Telemetry::QueryNode(tm) => {
                Ok((tm.header.status, tm.enabled, tm.counts))
            }
            _ => {
                Err(TcsError::Protocol("Unexpected telemetry type".to_string()))
            }
        }
    }

    /// The last datum a node moved each way: QUERY_NODE_SAMPLE.
    ///
    /// Taken first and given second, which is the order the answer carries
    /// them. For an operator who wants to see the bytes rather than a count
    /// of them; the counts are [`TcsClient::query_node`]'s, and asking for
    /// both every poll would spend downlink on data nobody reads.
    ///
    /// # Arguments
    ///
    /// * `node` - The node's id.
    ///
    /// # Returns
    ///
    /// The status the vehicle answered with, the datum taken and the datum
    /// given.
    ///
    /// # Errors
    ///
    /// `TcsError::Config` if the client has no node link,
    /// `TcsError::Unmatched` if the station is not matched for commanding,
    /// `TcsError::Io` if it cannot be sent or no answer comes in time,
    /// and `TcsError::Protocol` if the answer is not this command's.
    pub fn query_node_sample(
        &mut self,
        node: NodeId,
    ) -> TcsResult<(CommandStatus, DHSample, DHSample)> {
        let seq = self.next_sequence();
        let cmd =
            Command::QueryNodeSample(NodeCommand::query_sample(seq, node));
        match self.send_command(cmd)? {
            Telemetry::QueryNodeSample(tm) => {
                Ok((tm.header.status, tm.taken, tm.given))
            }
            _ => {
                Err(TcsError::Protocol("Unexpected telemetry type".to_string()))
            }
        }
    }

    // -- telemetry and the connection itself -------------------------------

    /// Receive telemetry (blocking)
    pub fn receive_telemetry(&mut self) -> TcsResult<Telemetry> {
        self.connection.receive()
    }

    /// Receive telemetry with timeout
    pub fn receive_telemetry_timeout(
        &mut self,
        timeout: Duration,
    ) -> TcsResult<Telemetry> {
        self.connection.receive_timeout(timeout)
    }

    /// Check if there is telemetry available
    pub fn has_telemetry(&self) -> TcsResult<bool> {
        self.connection.has_data()
    }

    /// Close every link.
    ///
    /// All of them are closed even if one complains, because a client whose
    /// links are half closed is worse than one whose close was reported: the
    /// error comes back, the sockets go.
    ///
    /// # Returns
    ///
    /// Nothing, once every link is closed.
    ///
    /// # Errors
    ///
    /// The first error any link gave in closing.
    pub fn close(&mut self) -> TcsResult<()> {
        let mut outcome = self.connection.close();
        if let Some(nodes) = self.nodes.as_mut() {
            outcome = outcome.and(nodes.close());
        }
        outcome
    }
}

/// Builder for TcsClient
pub struct TcsClientBuilder {
    timeout: Duration,
}

impl TcsClientBuilder {
    pub fn new() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn build(
        self,
        connection: Box<dyn Connection>,
        nodes: Box<dyn Connection>,
        matched: Matched,
    ) -> TcsClient {
        let mut client = TcsClient::of_a_graph(connection, nodes, matched);
        client.set_timeout(self.timeout);
        client
    }
}

impl Default for TcsClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Waits on `link` for the answer to the command numbered `sequence`, and
/// drops every other answer that arrives first.
///
/// An answer is taken only for the command it says it answers. One that
/// answers another command -- late, after its own command timed out;
/// repeated by the link; or made before a restart, on either side -- may
/// say something only another command, or another configuration, gives a
/// meaning to, so it is counted and dropped rather than read as this one's.
/// See "On the ground" in `docs/tcspecial-design.rst`.
///
/// Every answer that arrives is counted in `matched` once: processed where
/// it is the one waited for, dropped for a mismatch where it is another's,
/// malformed where it cannot be read as one of the link's packets.
///
/// # Arguments
///
/// * `link`     - The link the command went on.
/// * `sequence` - The command's sequence number.
/// * `timeout`  - How long to wait in all, however many are dropped.
/// * `dropped`  - Counts the answers dropped, for the caller to keep.
/// * `matched`  - Where every answer that arrives is counted.
///
/// # Returns
///
/// The answer to that command.
///
/// # Errors
///
/// `TcsError::Io` if the link fails, or no answer to that command comes
/// within `timeout`; whatever the link returns for an answer it cannot
/// read.
fn answer_to(
    link: &mut Box<dyn Connection>,
    sequence: u32,
    timeout: Duration,
    dropped: &mut u64,
    matched: &Matched,
) -> TcsResult<Telemetry> {
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(TcsError::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("no answer to command #{sequence} came in time"),
            )));
        }
        let answer = match link.receive_timeout(left) {
            Ok(answer) => answer,
            // Nothing arrived, or the socket failed: no packet to count.
            Err(TcsError::Io(e)) => return Err(TcsError::Io(e)),
            // A packet from where the command went that is not one of the
            // link's.
            Err(e) => {
                matched.malformed();
                return Err(e);
            }
        };
        if answer.sequence() == sequence {
            matched.processed();
            return Ok(answer);
        }
        matched.mismatched(1);
        *dropped += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beacon::Stage;

    /// The checksum of the configuration the stations in these tests use.
    const OURS: ConfigDigest = ConfigDigest([0x5A; 16]);

    /// A station matched for listening only: its last beacon was clear,
    /// carried its checksum and is recent, and no CONNECT of its own has
    /// been answered.
    ///
    /// # Returns
    ///
    /// The station's state.
    fn listening() -> Matched {
        let matched = Matched::for_configuration(OURS);
        matched.beacon(false, Some(OURS), Stage::Heard, false);
        matched
    }

    /// A station matched for commanding: [`listening`], and a CONNECT of its
    /// own answered with its checksum. What most of these tests are about
    /// comes after the match.
    ///
    /// # Returns
    ///
    /// The station's state.
    fn commanding() -> Matched {
        let matched = listening();
        let mark = matched.before_connect();
        matched.connect_answered(OURS, mark);
        matched
    }

    /// While the station is not matched for commanding, every command but
    /// CONNECT, RESTART_ARM and RESTART is refused unsent, saying the
    /// configurations are not yet matched; the three go, and a CONNECT
    /// answered with the station's checksum makes it matched for commanding
    /// once it is matched for listening.
    #[test]
    fn an_unmatched_station_sends_only_the_three() {
        let (connection, asked) = Canned::recording(an_answer(a_version(1)));
        let (nodes, node_asked) = Canned::recording(Telemetry::StopNode(
            tcslibgs::NodeTelemetry::stopped(
                1,
                CommandStatus::Success,
                NodeId(2),
            ),
        ));
        let matched = Matched::for_configuration(OURS);
        let mut client =
            TcsClient::of_a_graph(connection, nodes, matched.clone());

        let refused = client.ping().expect_err("not matched");
        assert!(matches!(refused, TcsError::Unmatched(CommandType::Ping)));
        let said = refused.to_string();
        assert!(said.contains("not yet matched"), "{said}");
        assert!(said.contains("checksum"), "{said}");
        assert!(matches!(
            client.stop_node(NodeId(2)),
            Err(TcsError::Unmatched(CommandType::StopNode))
        ));
        assert!(asked.lock().unwrap().is_empty(), "none of it was sent");
        assert!(node_asked.lock().unwrap().is_empty());

        // CONNECT goes, and is answered with ours; flagged beacons still
        // keep the station from commanding.
        client.connect(OURS).expect("CONNECT goes unmatched");
        assert_eq!(asked.lock().unwrap().len(), 1);
        assert!(matches!(
            client.stop_node(NodeId(2)),
            Err(TcsError::Unmatched(_))
        ));

        // A clear beacon of ours: matched, and the node command goes.
        matched.beacon(false, Some(OURS), Stage::Heard, false);
        client.stop_node(NodeId(2)).expect("sent once matched");
        assert_eq!(node_asked.lock().unwrap().len(), 1);

        // A flagged beacon ends it again.
        matched.beacon(true, Some(OURS), Stage::Heard, false);
        assert!(matches!(
            client.stop_node(NodeId(2)),
            Err(TcsError::Unmatched(_))
        ));
    }

    /// A CONNECT answered with another configuration's checksum does not
    /// make the station matched for commanding.
    #[test]
    fn a_connect_answered_with_another_checksum_does_not_match() {
        let theirs = Telemetry::Connect(tcslibgs::ConnectTelemetry::new(
            1,
            a_version(1),
            ConfigDigest([0x11; 16]),
            40,
        ));
        let mut client =
            TcsClient::spacecraft_only(Canned::answering(theirs), listening());
        client.connect(OURS).expect("answered");
        assert!(!client.matched().for_commanding());
        assert!(matches!(client.ping(), Err(TcsError::Unmatched(_))));
    }

    /// A CONNECT refused, though its answer carries the station's own
    /// checksum, is not a match and is not said to be one: a start that
    /// has taken a RESTART refuses every CONNECT `Unmatched`, and a station
    /// that took the answer for a match would be matched with a start that
    /// is going -- and would stay so where the next start's first beacons
    /// were lost and another station matched it.
    #[test]
    fn a_connect_refused_does_not_match() {
        let refused = Telemetry::Connect(
            tcslibgs::ConnectTelemetry::new(1, a_version(1), OURS, 40)
                .refused(CommandStatus::Unmatched),
        );
        let matched = listening();
        let mut client = TcsClient::spacecraft_only(
            Canned::answering(refused),
            matched.clone(),
        );
        let said = client.connect(OURS).expect_err("refused").to_string();
        assert!(said.contains("RESTART"), "says why: {said}");
        matched.beacon(false, Some(OURS), Stage::Heard, false);
        assert!(!matched.for_commanding(), "and nothing was matched");
    }

    /// Every answer that arrives is counted once: the one waited for as
    /// processed, the others as dropped for a mismatch, and one that is not
    /// a packet of the link as malformed.
    #[test]
    fn every_answer_is_counted_once() {
        let matched = commanding();
        let mut client = TcsClient::spacecraft_only(
            Scripted::answering(&[-1, 3, 0]),
            matched.clone(),
        );
        client.ping().expect("its own answer, third");
        let counts = matched.counts();
        assert_eq!((counts.processed, counts.mismatched), (1, 2));

        let mut client =
            TcsClient::spacecraft_only(Box::new(Unreadable), matched.clone());
        assert!(client.ping().is_err());
        let counts = matched.counts();
        assert_eq!(counts.malformed, 1);
        assert_eq!(counts.received(), 4);
    }

    /// A link whose every answer is a packet that is not one of the link's.
    struct Unreadable;

    impl Connection for Unreadable {
        fn send(&mut self, _command: &Command) -> TcsResult<()> {
            Ok(())
        }

        fn receive(&mut self) -> TcsResult<Telemetry> {
            self.receive_timeout(Duration::ZERO)
        }

        fn receive_timeout(
            &mut self,
            _timeout: Duration,
        ) -> TcsResult<Telemetry> {
            Ok(Telemetry::read(b"not a packet")?)
        }

        fn has_data(&self) -> TcsResult<bool> {
            Ok(true)
        }

        fn close(&mut self) -> TcsResult<()> {
            Ok(())
        }
    }

    /// RESTART_ARM and RESTART carry the target they are given, which need
    /// not be the station's own configuration.
    #[test]
    fn the_arm_and_the_restart_carry_their_target() {
        let other = ConfigDigest([0x77; 16]);
        let (connection, asked) = Canned::recording(Telemetry::RestartArm(
            tcslibgs::RestartArmTelemetry::new(1, CommandStatus::Success),
        ));
        let mut client = TcsClient::spacecraft_only(connection, commanding());
        client.restart_arm(ArmKey(9), other).expect("answered");
        let _ = client.restart(ArmKey(9), other);
        let asked = asked.lock().unwrap();
        match (&asked[0], &asked[1]) {
            (Command::RestartArm(arm), Command::Restart(restart)) => {
                assert_eq!((arm.target, arm.arm_key), (other, ArmKey(9)));
                assert_eq!(
                    (restart.target, restart.arm_key),
                    (other, ArmKey(9))
                );
            }
            other => panic!("an arm and a restart: {other:?}"),
        }
    }

    #[test]
    fn test_client_builder() {
        let builder = TcsClientBuilder::new().timeout(Duration::from_secs(10));
        assert_eq!(builder.timeout, Duration::from_secs(10));
    }

    /// A link that answers whatever it is told to, and remembers what it was
    /// asked.
    struct Canned {
        answer: Telemetry,
        asked: std::sync::Arc<std::sync::Mutex<Vec<Command>>>,
    }

    impl Canned {
        /// Makes a link that answers every command with one answer.
        ///
        /// # Arguments
        ///
        /// * `answer` - What it answers with.
        ///
        /// # Returns
        ///
        /// The link, boxed to be given to a client.
        fn answering(answer: Telemetry) -> Box<Canned> {
            Box::new(Canned {
                answer,
                asked: Default::default(),
            })
        }

        /// Makes a link as [`Canned::answering`] does, and keeps what it is
        /// asked where a test can read it.
        ///
        /// # Arguments
        ///
        /// * `answer` - What it answers with.
        ///
        /// # Returns
        ///
        /// The link, and the record of the commands it is sent.
        fn recording(
            answer: Telemetry,
        ) -> (Box<Canned>, std::sync::Arc<std::sync::Mutex<Vec<Command>>>)
        {
            let canned = Canned::answering(answer);
            let asked = canned.asked.clone();
            (canned, asked)
        }
    }

    impl Canned {
        /// Gives its one answer, as the answer to the last command it was
        /// asked, as a vehicle would.
        ///
        /// # Returns
        ///
        /// The answer, carrying that command's sequence number.
        fn answer_to_the_last(&self) -> Telemetry {
            let asked = self.asked.lock().expect("the record");
            match asked.last() {
                Some(command) => {
                    self.answer.clone().for_sequence(command.sequence())
                }
                None => self.answer.clone(),
            }
        }
    }

    impl Connection for Canned {
        fn send(&mut self, command: &Command) -> TcsResult<()> {
            self.asked.lock().expect("the record").push(command.clone());
            Ok(())
        }

        fn receive(&mut self) -> TcsResult<Telemetry> {
            Ok(self.answer_to_the_last())
        }

        fn receive_timeout(
            &mut self,
            _timeout: Duration,
        ) -> TcsResult<Telemetry> {
            Ok(self.answer_to_the_last())
        }

        fn has_data(&self) -> TcsResult<bool> {
            Ok(true)
        }

        fn close(&mut self) -> TcsResult<()> {
            Ok(())
        }
    }

    /// A link that answers each command with a script of answers: each
    /// is a PING's answer for the command asked, offset by a number, so
    /// that nought is the right answer and anything else answers another
    /// command. Once the script is spent it times out, as a quiet link
    /// does.
    struct Scripted {
        offsets: std::collections::VecDeque<i64>,
        asked: u32,
    }

    impl Scripted {
        /// Makes a link that answers with `offsets`, in order.
        ///
        /// # Arguments
        ///
        /// * `offsets` - How far each answer's sequence is from the
        ///               command's.
        ///
        /// # Returns
        ///
        /// The link, boxed to be given to a client.
        fn answering(offsets: &[i64]) -> Box<Scripted> {
            Box::new(Scripted {
                offsets: offsets.iter().copied().collect(),
                asked: 0,
            })
        }
    }

    impl Connection for Scripted {
        fn send(&mut self, command: &Command) -> TcsResult<()> {
            self.asked = command.sequence();
            Ok(())
        }

        fn receive(&mut self) -> TcsResult<Telemetry> {
            self.receive_timeout(Duration::ZERO)
        }

        fn receive_timeout(
            &mut self,
            _timeout: Duration,
        ) -> TcsResult<Telemetry> {
            match self.offsets.pop_front() {
                Some(offset) => {
                    Ok(Telemetry::Ping(tcslibgs::PingTelemetry::new(
                        (i64::from(self.asked) + offset) as u32,
                        CommandStatus::Success,
                    )))
                }
                None => Err(TcsError::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "nothing more to say",
                ))),
            }
        }

        fn has_data(&self) -> TcsResult<bool> {
            Ok(!self.offsets.is_empty())
        }

        fn close(&mut self) -> TcsResult<()> {
            Ok(())
        }
    }

    /// An answer is taken only for the command it answers: a late answer
    /// to an earlier command, and one to a later number, are dropped and
    /// counted, and the client goes on waiting for its own.
    #[test]
    fn an_answer_to_another_command_is_dropped_and_counted() {
        let mut client = TcsClient::spacecraft_only(
            Scripted::answering(&[-1, 3, 0]),
            commanding(),
        );
        let sent = client.sequence.load(Ordering::SeqCst);
        let answer = client.ping().expect("its own answer, third");
        assert_eq!(answer.header.sequence, sent);
        assert_eq!(client.answers_dropped(), 2);
    }

    /// Where its own answer never comes, the wait ends in a timeout, not in
    /// another command's answer.
    #[test]
    fn only_answers_to_other_commands_is_no_answer() {
        let mut client = TcsClient::spacecraft_only(
            Scripted::answering(&[-2, -1]),
            commanding(),
        );
        assert!(matches!(client.ping(), Err(TcsError::Io(_))));
        assert_eq!(client.answers_dropped(), 2);
    }

    /// Makes a CONNECT's answer from a vehicle which has taken up to command
    /// 40.
    ///
    /// # Arguments
    ///
    /// * `version` - The configuration version it answers with.
    ///
    /// # Returns
    ///
    /// The answer.
    fn an_answer(version: ConfigVersion) -> Telemetry {
        Telemetry::Connect(tcslibgs::ConnectTelemetry::new(
            1,
            version,
            ConfigDigest([0x5A; 16]),
            40,
        ))
    }

    /// Makes a configuration version of `major`.0.0.
    ///
    /// # Arguments
    ///
    /// * `major` - Its major number.
    ///
    /// # Returns
    ///
    /// The version.
    fn a_version(major: u8) -> ConfigVersion {
        ConfigVersion {
            major,
            minor: 0,
            patch: 0,
        }
    }

    /// A client times the commands it sends.
    ///
    /// Here because this is where every command passes: a caller that timed
    /// them at its own call sites would eventually add one that did not, and
    /// the one it forgot would be the one that hung.
    #[test]
    fn a_client_times_the_commands_it_sends() {
        let mut client = TcsClient::of_a_graph(
            Canned::answering(Telemetry::Ping(tcslibgs::PingTelemetry::new(
                1,
                CommandStatus::Success,
            ))),
            Canned::answering(an_answer(a_version(1))),
            commanding(),
        );

        assert_eq!(client.last_sent(), None, "nothing has been sent yet");
        assert_eq!(client.last_received(), None);

        let before = Timestamp::now();
        client.ping().expect("the canned answer");
        let after = Timestamp::now();

        let sent = client.last_sent().expect("the command was timed");
        let received = client.last_received().expect("so was the answer");
        assert!(sent >= before && sent <= after, "sent at {sent}");
        assert!(
            received >= sent && received <= after,
            "answered at {received}"
        );
    }

    /// A CONNECT is asked of the spacecraft's link and of no node's.
    ///
    /// It is about having a connection to the vehicle, which is what that
    /// link is; the other link carries what is asked of a node, and a ground
    /// station with no connection has nothing to ask one.
    ///
    /// The node link here answers with something a CONNECT never gets, so a
    /// handshake that asked it would fail rather than pass by luck.
    #[test]
    fn what_each_end_read_is_asked_of_the_spacecraft_alone() {
        let mut client = TcsClient::of_a_graph(
            Canned::answering(an_answer(a_version(1))),
            Canned::answering(Telemetry::Ping(tcslibgs::PingTelemetry::new(
                1,
                CommandStatus::Success,
            ))),
            commanding(),
        );

        let (version, _) = client
            .connect(ConfigDigest([0x5A; 16]))
            .expect("the spacecraft's link is the one that answers it");
        assert_eq!(version, a_version(1));
    }

    /// A start ends the station's match, so a node command waits for a
    /// CONNECT of the station's own to be answered again with its checksum:
    /// there is no generation for an answer to carry, and the beacons and
    /// the CONNECT say which start is flying. A beacon of the same start
    /// changes nothing.
    #[test]
    fn a_start_ends_the_match_until_a_connect_is_answered_again() {
        let (nodes, asked) = Canned::recording(Telemetry::StopNode(
            tcslibgs::NodeTelemetry::stopped(
                1,
                CommandStatus::Success,
                NodeId(2),
            ),
        ));
        let matched = commanding();
        let mut client = TcsClient::of_a_graph(
            Canned::answering(an_answer(a_version(1))),
            nodes,
            matched.clone(),
        );
        client.stop_node(NodeId(2)).expect("matched, so sent");
        matched.beacon(false, Some(OURS), Stage::Heard, false);
        client.stop_node(NodeId(2)).expect("the same start goes on");
        assert_eq!(asked.lock().unwrap().len(), 2);

        // The start that came back: its beacon carries the flag.
        matched.beacon(true, Some(OURS), Stage::Heard, true);
        assert!(matches!(
            client.stop_node(NodeId(2)),
            Err(TcsError::Unmatched(_))
        ));
        assert_eq!(asked.lock().unwrap().len(), 2, "and it was not sent");

        // Matched again: its flag clear, and a CONNECT answered with ours.
        matched.beacon(false, Some(OURS), Stage::Heard, false);
        client.connect(OURS).expect("connected");
        client
            .stop_node(NodeId(2))
            .expect("sent once matched again");
        assert_eq!(asked.lock().unwrap().len(), 3);
    }

    /// A CONNECT's answer says the newest command the vehicle has taken, and
    /// the count goes on from past it -- forward only, so that a station
    /// already past it stays where it is.
    #[test]
    fn the_count_goes_on_from_past_the_newest_the_vehicle_took() {
        let (connection, asked) = Canned::recording(an_answer(a_version(1)));
        let mut client = TcsClient::spacecraft_only(connection, commanding());
        client.connect(ConfigDigest([0x5A; 16])).expect("connected");
        // Answered with the canned CONNECT, which a PING takes for an
        // answer of the wrong kind: what is asked here is what was sent.
        let _ = client.ping();
        assert_eq!(
            asked.lock().unwrap()[1].sequence(),
            41,
            "past the 40 the vehicle took"
        );

        client.sequence.store(100, Ordering::SeqCst);
        client.connect(ConfigDigest([0x5A; 16])).expect("connected");
        let _ = client.ping();
        assert_eq!(asked.lock().unwrap()[3].sequence(), 101, "not back");
    }
}
