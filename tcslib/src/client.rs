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
/// which states no ports and so nowhere to command.
///
/// # Errors
///
/// A message saying what is wrong with the `tcspecial` section's command
/// interpreter settings.
pub fn command_links_of(
    graph: &tcslibgs::graph::Graph,
) -> Result<Option<CommandLinks>, String> {
    use tcslibgs::graph::parameters::{ci_config_of, flatten};

    let Some(section) = graph.tcspecial.as_ref() else {
        return Ok(None);
    };
    let ci = ci_config_of(&flatten(section))?.to_ci_config()?;
    Ok(Some(CommandLinks {
        from: ci.moc_cmd_send.to_string(),
        interpreter: format!("{}:{}", ci.address, ci.port),
        nodes_from: ci.payload_command_source_base,
        nodes_to: ci.payload_command_dest_base,
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
    /// How many commands have gone out, so that a caller can tell whether
    /// the one it asked for did.
    sent: u64,
    /// How many answers arrived that were not to the command being waited
    /// for, and were dropped: see [`answer_to`].
    answers_dropped: u64,
    /// The generation this station knows is flying: the one the last
    /// CONNECT's answer said. `None` until one has, and again once a beacon
    /// says tcspecial has started again or an answer has come from another
    /// start, when every command but PING and CONNECT is refused here until a
    /// CONNECT says again. A command carries no
    /// generation, only its sequence; this is what keeps a station from
    /// sending one to a start whose definitions it has not seen. See
    /// [`tcslibgs::CommandType::always_taken`].
    generation: Option<u32>,
    /// What a RESTART that went out is waiting for, until a beacon says it
    /// came: see [`RestartWait`].
    restart_wait: Option<RestartWait>,
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

/// A RESTART went out, and nothing but what [`RestartWait::lets`] may follow
/// it until a beacon says tcspecial has started again.
///
/// Every other command until then is refused here, unsent: from the moment
/// the vehicle takes the RESTART it is ending, and the vehicle that comes
/// back is another start, possibly with other definitions -- a command sent
/// in between would be read by whichever of the two it reached. See "What
/// the ground may send meanwhile" in `docs/design.rst`.
///
/// It no longer says how the restart ended: every start of tcspecial begins
/// with the restart flag set, so the flag says nothing of whether the start
/// that came back is the one the RESTART asked for. The checksum in its
/// beacons says which configuration it runs, and matching it is what lets
/// the station command it again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartWait;

impl RestartWait {
    /// Says whether a command of this kind may go while the wait lasts.
    ///
    /// The three that go before a match, because the vehicle is ending and
    /// the start that comes back is not yet matched:
    /// [`tcslibgs::CommandType::taken_unmatched`]. A RESTART that reaches
    /// the start that came back restarts it again, which is why it still
    /// needs the generation known: an answer from another start forgets it.
    ///
    /// # Arguments
    ///
    /// * `command` - The kind of command to be sent.
    ///
    /// # Returns
    ///
    /// `true` where it may go.
    pub fn lets(&self, command: CommandType) -> bool {
        command.taken_unmatched()
    }

    /// Says why a command is refused while this lasts.
    ///
    /// # Returns
    ///
    /// A `TcsError::AwaitingRestart` saying what the wait is for.
    fn refusal(&self) -> TcsError {
        TcsError::AwaitingRestart(
            "not sent: a RESTART went out, and nothing but CONNECT, \
             RESTART_ARM and RESTART may follow it until a beacon whose \
             number starts again says tcspecial has started again"
                .to_string(),
        )
    }
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
    /// The client, with the default timeout, counting from sequence 1 and
    /// knowing no generation yet.
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
            sent: 0,
            answers_dropped: 0,
            generation: None,
            restart_wait: None,
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
    /// The client, with the default timeout, counting from sequence 1 and
    /// knowing no generation yet.
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
            sent: 0,
            answers_dropped: 0,
            generation: None,
            restart_wait: None,
            matched,
        }
    }

    /// Takes note that a beacon has arrived: whether its number says
    /// tcspecial has started again since the one before it -- what
    /// [`crate::beacon::Pass::restarted`] says of it.
    ///
    /// A station that sends RESTART passes on every beacon it hears, because
    /// a beacon is what ends the wait a RESTART begins. A start, waited for or
    /// not, also makes this station forget which generation is flying, so
    /// that nothing but CONNECT goes until a CONNECT says. Whether the
    /// station is matched is its [`Matched`]'s, which the beacon watch keeps
    /// from the same beacons.
    ///
    /// # Arguments
    ///
    /// * `restarted` - Whether the beacon's number says tcspecial started
    ///                 again.
    ///
    /// # Returns
    ///
    /// Whether this ended a wait for a restart, for the station to say so.
    pub fn beacon_heard(&mut self, restarted: bool) -> bool {
        if !restarted {
            return false;
        }
        self.generation = None;
        self.restart_wait.take().is_some()
    }

    /// Whether the station is matched with the vehicle, which this client's
    /// commands wait for, and what it has received.
    pub fn matched(&self) -> &Matched {
        &self.matched
    }

    /// What a RESTART that went out is waiting for, if one is.
    pub fn restart_wait(&self) -> Option<RestartWait> {
        self.restart_wait
    }

    /// End a wait by hand, for an operator who knows the RESTART was never
    /// taken -- the beacons go on counting up, at their interval, with the
    /// restart flag clear -- and returns it so that the station can
    /// record that it did. Nothing else ends a wait but a beacon or a refusal.
    pub fn end_restart_wait(&mut self) -> Option<RestartWait> {
        self.restart_wait.take()
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

    /// The generation this station knows is flying, as the last CONNECT's
    /// answer said, or `None` where none has since the last start.
    pub fn generation(&self) -> Option<u32> {
        self.generation
    }

    /// Says whether `command` may go: only where this station is matched
    /// for commanding, but for CONNECT, RESTART_ARM and RESTART, which go
    /// either way because they are how it matches and how it rescues a
    /// vehicle that will not; and only where it knows which generation is
    /// flying, but for a PING or a CONNECT.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to be sent.
    ///
    /// # Errors
    ///
    /// `TcsError::Unmatched` where the station is not matched for
    /// commanding and the command is not one of the three;
    /// `TcsError::GenerationUnknown` where no generation is known and the
    /// command is not one that is always taken.
    fn may_send(&self, command: &Command) -> TcsResult<()> {
        let kind = command.cmd_type();
        if !kind.taken_unmatched() && !self.matched.for_commanding() {
            return Err(TcsError::Unmatched(kind));
        }
        match self.generation {
            Some(_) => Ok(()),
            None if command.cmd_type().always_taken() => Ok(()),
            None => Err(TcsError::GenerationUnknown(command.cmd_type())),
        }
    }

    /// Says whether `command` may go while a RESTART is waited on, if one
    /// is: see [`RestartWait::lets`].
    ///
    /// # Arguments
    ///
    /// * `command` - The command to be sent.
    ///
    /// # Errors
    ///
    /// `TcsError::AwaitingRestart` where a wait refuses it.
    fn wait_lets(&self, command: &Command) -> TcsResult<()> {
        match &self.restart_wait {
            Some(wait) if !wait.lets(command.cmd_type()) => Err(wait.refusal()),
            _ => Ok(()),
        }
    }

    /// Sends a command on its link, for the generation flying, and waits for
    /// the answer, timing both.
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
    /// `TcsError::AwaitingRestart` while a RESTART is awaited and the wait
    /// does not let this command go, `TcsError::Config` if the client has no
    /// link for it, `TcsError::Unmatched` if the station is not matched for
    /// commanding, `TcsError::GenerationUnknown` if no generation is known
    /// to send it for, and `TcsError::Io` if it cannot be sent or no answer
    /// comes in time.
    fn send_command(&mut self, command: Command) -> TcsResult<Telemetry> {
        self.wait_lets(&command)?;
        // A command with no link to go on says so before anything is said
        // of the generation: that is the client's own configuration, and no
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
        self.sent += 1;

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
        if let Ok(answer) = &answer {
            self.last_received = Some(Timestamp::now());
            self.heard_generation_of(answer);
        }
        answer
    }

    /// What an answer says of the generation flying.
    ///
    /// A vehicle that answered as another start than the one this station
    /// thought it was has started again since: nothing more goes until a
    /// beacon or a CONNECT says which start it is, because the definitions it
    /// started with may not be the ones this station is reading. Learning the
    /// generation from the answer instead would carry on as though nothing
    /// had happened. An answer of generation nought, which no start is, says
    /// nothing of it.
    ///
    /// # Arguments
    ///
    /// * `answer` - The answer to a command this client sent.
    fn heard_generation_of(&mut self, answer: &Telemetry) {
        let other = answer.generation() != 0
            && Some(answer.generation()) != self.generation;
        if other {
            self.generation = None;
        }
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
    /// `TcsError::AwaitingRestart` while a RESTART is awaited,
    /// `TcsError::GenerationUnknown` if no generation is known to send it
    /// for, `TcsError::Io` if it cannot be sent or no answer comes in time,
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
    /// # Arguments
    ///
    /// * `arm_key` - The key the RESTART that follows must carry.
    ///
    /// # Returns
    ///
    /// The status the vehicle answered with.
    ///
    /// # Errors
    ///
    /// `TcsError::AwaitingRestart` while a RESTART is awaited,
    /// `TcsError::GenerationUnknown` if no generation is known to send it
    /// for, `TcsError::Io` if it cannot be sent or no answer comes in time,
    /// and `TcsError::Protocol` if the answer is not this command's.
    pub fn restart_arm(&mut self, arm_key: ArmKey) -> TcsResult<CommandStatus> {
        let seq = self.next_sequence();
        let cmd = Command::RestartArm(RestartArmCommand::new(seq, arm_key));
        let response = self.send_command(cmd)?;

        match response {
            Telemetry::RestartArm(tm) => Ok(tm.header.status),
            _ => {
                Err(TcsError::Protocol("Unexpected telemetry type".to_string()))
            }
        }
    }

    /// Say what this end read, and hear what the other end read.
    ///
    /// Sent when a link is opened. What comes back is the spacecraft's own
    /// version and the digest of the configuration it is serving, for the
    /// caller to hold against the two it sent: this returns both rather than
    /// a verdict, because the caller is the end that knows which file it read
    /// and can say so.
    ///
    /// Where the answer is a success its generation is the one commands are
    /// sent for from then on, and the sequence count goes on from past the
    /// newest command the vehicle has taken.
    ///
    /// It goes during a wait for a restart too, without ending it: see
    /// [`RestartWait::lets`]. And it goes before the station is matched,
    /// which is what it is for: answered with the station's own checksum,
    /// it makes the station matched for commanding once it is matched for
    /// listening -- see [`Matched::connect_answered`]. Answered with
    /// another, it matches nothing, and the vehicle changes nothing.
    ///
    /// # Arguments
    ///
    /// * `version` - The version of the configuration this end read.
    /// * `digest`  - The digest of that configuration.
    ///
    /// # Returns
    ///
    /// The spacecraft's version and digest.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if it cannot be sent or no answer comes in time,
    /// and `TcsError::Protocol` if the answer is not this command's.
    pub fn connect(
        &mut self,
        version: ConfigVersion,
        digest: ConfigDigest,
    ) -> TcsResult<(ConfigVersion, ConfigDigest)> {
        let seq = self.next_sequence();
        let command =
            Command::Connect(ConnectCommand::new(seq, version, digest));
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
            // The generation it answers with is the one this station knows
            // is flying from here on, where it answered at all, and the count
            // goes on from past the newest command it has taken.
            Telemetry::Connect(tm) => {
                if tm.header.status.is_success() {
                    self.generation = Some(tm.header.generation);
                    self.count_past(tm.newest_taken);
                    self.matched.connect_answered(tm.digest, mark);
                }
                Ok((tm.version, tm.digest))
            }
            _ => {
                Err(TcsError::Protocol("Unexpected telemetry type".to_string()))
            }
        }
    }

    /// Send a RESTART command
    ///
    /// From the moment it goes, every command but CONNECT, RESTART_ARM and
    /// RESTART is refused here until a beacon passed to
    /// [`TcsClient::beacon_heard`] says tcspecial has started again: see
    /// [`RestartWait`]. A RESTART the vehicle answers as refused ends the
    /// wait it began, because nothing was initiated; one whose answer does
    /// not come does not, because it may have been taken. One sent again
    /// during a wait ends it neither way: the first may have been taken, and
    /// what is said of this one says nothing of that.
    ///
    /// # Arguments
    ///
    /// * `arm_key` - The key the RESTART_ARM before it gave.
    ///
    /// # Returns
    ///
    /// The status the vehicle answered with.
    ///
    /// # Errors
    ///
    /// `TcsError::AwaitingRestart` while a RESTART is awaited,
    /// `TcsError::GenerationUnknown` if no generation is known to send it
    /// for, `TcsError::Io` if it cannot be sent or no answer comes in time,
    /// and `TcsError::Protocol` if the answer is not this command's.
    pub fn restart(&mut self, arm_key: ArmKey) -> TcsResult<CommandStatus> {
        let seq = self.next_sequence();
        let cmd = Command::Restart(RestartCommand::new(seq, arm_key));
        let sent = self.sent;
        let waiting = self.restart_wait.is_some();
        let response = self.send_command(cmd);
        // Begun once it has gone, whatever comes back -- an answer lost is a
        // RESTART that may have been taken -- and not where it never went.
        if self.sent != sent && !waiting {
            self.restart_wait = Some(RestartWait);
        }

        match response? {
            Telemetry::Restart(tm) => {
                if !tm.header.status.is_success() && !waiting {
                    self.restart_wait = None;
                }
                Ok(tm.header.status)
            }
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
    /// `TcsError::AwaitingRestart` while a RESTART is awaited,
    /// `TcsError::GenerationUnknown` if no generation is known to send it
    /// for, `TcsError::Io` if it cannot be sent or no answer comes in time,
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
    /// `TcsError::AwaitingRestart` while a RESTART is awaited,
    /// `TcsError::GenerationUnknown` if no generation is known to send it
    /// for, `TcsError::Io` if it cannot be sent or no answer comes in time,
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
    /// `TcsError::AwaitingRestart` while a RESTART is awaited,
    /// `TcsError::GenerationUnknown` if no generation is known to send it
    /// for, `TcsError::Io` if it cannot be sent or no answer comes in time,
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
    /// `TcsError::AwaitingRestart` while a RESTART is awaited,
    /// `TcsError::GenerationUnknown` if no generation is known to send it
    /// for, `TcsError::Io` if it cannot be sent or no answer comes in time,
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
    /// `TcsError::AwaitingRestart` while a RESTART is awaited,
    /// `TcsError::GenerationUnknown` if no generation is known to send it
    /// for, `TcsError::Io` if it cannot be sent or no answer comes in time,
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
/// See "On the ground" in `docs/design.rst`.
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

    /// A wait ends at a beacon whose number says tcspecial has started
    /// again; a beacon of the old start does not end it.
    #[test]
    fn a_restart_wait_ends_at_the_next_start() {
        let mut client = TcsClient::spacecraft_only(
            Canned::answering(Telemetry::Ping(tcslibgs::PingTelemetry::new(
                1,
                CommandStatus::Success,
            ))),
            commanding(),
        );
        client.restart_wait = Some(RestartWait);
        assert!(!client.beacon_heard(false), "the old start");
        assert_eq!(client.restart_wait(), Some(RestartWait));
        assert!(client.beacon_heard(true), "a new start ends it");
        assert_eq!(client.restart_wait(), None);
    }

    /// While the station is not matched for commanding, every command but
    /// CONNECT, RESTART_ARM and RESTART is refused unsent, saying the
    /// configurations are not yet matched; the three go, and a CONNECT
    /// answered with the station's checksum makes it matched for commanding
    /// once it is matched for listening.
    #[test]
    fn an_unmatched_station_sends_only_the_three() {
        let (connection, asked) = Canned::recording(an_answer(a_version(1)));
        let (nodes, node_asked) = Canned::recording(
            Telemetry::StopNode(tcslibgs::NodeTelemetry::stopped(
                1,
                CommandStatus::Success,
                NodeId(2),
            ))
            .for_generation(7),
        );
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
        client
            .connect(ConfigVersion::of_this_build(), OURS)
            .expect("CONNECT goes unmatched");
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
        ))
        .for_generation(7);
        let mut client =
            TcsClient::spacecraft_only(Canned::answering(theirs), listening());
        client
            .connect(ConfigVersion::of_this_build(), OURS)
            .expect("answered");
        assert!(!client.matched().for_commanding());
        assert!(matches!(client.ping(), Err(TcsError::Unmatched(_))));
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

    /// A RESTART refused ends the wait it began, and one sent again during
    /// a wait and refused does not: the first may have been taken.
    #[test]
    fn only_the_restart_that_began_a_wait_ends_it_by_a_refusal() {
        let mut client = TcsClient::spacecraft_only(
            Canned::answering(Telemetry::Restart(
                tcslibgs::RestartTelemetry::new(1, CommandStatus::NotArmed),
            )),
            commanding(),
        );
        client.generation = Some(7);
        assert_eq!(
            client.restart(ArmKey(1)).expect("answered"),
            CommandStatus::NotArmed
        );
        assert_eq!(client.restart_wait(), None, "nothing was initiated");

        client.restart_wait = Some(RestartWait);
        assert_eq!(
            client.restart(ArmKey(1)).expect("sent during the wait"),
            CommandStatus::NotArmed
        );
        assert_eq!(
            client.restart_wait(),
            Some(RestartWait),
            "the first may stand"
        );
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

    /// Makes a CONNECT's answer from generation 7, which has taken up to
    /// command 40.
    ///
    /// # Arguments
    ///
    /// * `version` - The configuration version it answers with.
    ///
    /// # Returns
    ///
    /// The answer.
    fn an_answer(version: ConfigVersion) -> Telemetry {
        // Generation 7, which has taken up to command 40.
        Telemetry::Connect(tcslibgs::ConnectTelemetry::new(
            1,
            version,
            ConfigDigest([0x5A; 16]),
            40,
        ))
        .for_generation(7)
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
            .connect(ConfigVersion::of_this_build(), ConfigDigest([0x5A; 16]))
            .expect("the spacecraft's link is the one that answers it");
        assert_eq!(version, a_version(1));
    }

    /// A node command waits for the station to know which generation is
    /// flying, and goes once a CONNECT's answer has said. A vehicle that
    /// answers as another start, or whose beacons start again, is one the
    /// station no longer knows, until a CONNECT says again.
    #[test]
    fn a_command_waits_for_the_station_to_know_the_generation() {
        let stopped = |status| {
            Telemetry::StopNode(tcslibgs::NodeTelemetry::stopped(
                1,
                status,
                NodeId(2),
            ))
        };
        let (nodes, asked) = Canned::recording(
            stopped(CommandStatus::Success).for_generation(7),
        );
        let mut client = TcsClient::of_a_graph(
            Canned::answering(an_answer(a_version(1))),
            nodes,
            commanding(),
        );

        assert_eq!(client.generation(), None);
        assert!(matches!(
            client.stop_node(NodeId(2)),
            Err(TcsError::GenerationUnknown(_))
        ));
        assert!(asked.lock().unwrap().is_empty(), "and it was not sent");

        client
            .connect(ConfigVersion::of_this_build(), ConfigDigest([0x5A; 16]))
            .expect("connected");
        assert_eq!(client.generation(), Some(7), "the CONNECT's answer");
        client.stop_node(NodeId(2)).expect("sent");
        assert_eq!(asked.lock().unwrap().len(), 1);
        assert_eq!(client.generation(), Some(7), "an answer of the same");

        // An answer made by another start, done or not, is a vehicle that
        // has started again since: forgotten, and nothing more goes.
        let (nodes, _) = Canned::recording(
            stopped(CommandStatus::Success).for_generation(8),
        );
        client.nodes = Some(nodes);
        client.stop_node(NodeId(2)).expect("answered");
        assert_eq!(client.generation(), None);
        assert!(matches!(
            client.stop_node(NodeId(2)),
            Err(TcsError::GenerationUnknown(_))
        ));

        // A CONNECT says again, and a beacon that starts again forgets.
        client
            .connect(ConfigVersion::of_this_build(), ConfigDigest([0x5A; 16]))
            .expect("connected");
        assert_eq!(client.generation(), Some(7));
        client.beacon_heard(false);
        assert_eq!(client.generation(), Some(7), "the same start goes on");
        client.beacon_heard(true);
        assert_eq!(client.generation(), None, "a start");
    }

    /// A CONNECT's answer says the newest command the vehicle has taken, and
    /// the count goes on from past it -- forward only, so that a station
    /// already past it stays where it is.
    #[test]
    fn the_count_goes_on_from_past_the_newest_the_vehicle_took() {
        let (connection, asked) = Canned::recording(an_answer(a_version(1)));
        let mut client = TcsClient::spacecraft_only(connection, commanding());
        client
            .connect(ConfigVersion::of_this_build(), ConfigDigest([0x5A; 16]))
            .expect("connected");
        // Answered with the canned CONNECT, which a PING takes for an
        // answer of the wrong kind: what is asked here is what was sent.
        let _ = client.ping();
        assert_eq!(
            asked.lock().unwrap()[1].sequence(),
            41,
            "past the 40 the vehicle took"
        );

        client.sequence.store(100, Ordering::SeqCst);
        client
            .connect(ConfigVersion::of_this_build(), ConfigDigest([0x5A; 16]))
            .expect("connected");
        let _ = client.ping();
        assert_eq!(asked.lock().unwrap()[3].sequence(), 101, "not back");
    }
}
