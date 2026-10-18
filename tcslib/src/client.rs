//! TCSpecial client for ground software integration
//!
//! This is what a control application built on tcslib talks to tcspecial
//! through: one set of operations for global control and status, and one set
//! per data handler. A mission control system such as YAMCS or MCT drives a
//! [`TcsClient`] rather than speaking the command and telemetry protocol
//! itself.
//!
//! The transport is a [`Connection`], so a client is as happy over a UDP
//! socket to a radio as over a TCP socket to a test harness, and neither the
//! operations nor their callers change with it.
//!
//! There are two of them, because the spacecraft takes commands on two links:
//! what the ground needs in a hurry must never be queued behind what it asked
//! for at leisure. Which command goes on which is not this module's decision
//! -- it is [`tcslibgs::CommandType::link`], the one table both ends read.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tcslibgs::{
    ArmKey, BeaconInterval, Command, CommandLink, CommandStatus, ConfigCommand, DHName, DHState,
    DHSample, DHType, NodeCommand, NodeCounts, NodeId, SetNodeCommand, Timestamp,
    PingCommand, QueryDHCommand, QueryDHSampleCommand, RestartArmCommand, RestartCommand,
    ConfigDigest, ConfigVersion, ConnectCommand,
    StartDHCommand, Statistics, StopDHCommand, TcsError, TcsResult, Telemetry,
};

use crate::connection::Connection;

/// Default timeout for command responses
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// TCSpecial client for sending commands and receiving telemetry
pub struct TcsClient {
    /// The spacecraft's own link: ping it, arm it, restart it, configure it.
    connection: Box<dyn Connection>,
    /// One link per payload: start one, stop one, ask one what it has moved.
    ///
    /// By name, because that is what a payload command carries -- see
    /// [`Command::payload_name`] -- and the spacecraft takes each payload's
    /// commands on a socket of its own, counted from
    /// `payload_command.dest_base` by that payload's id. A command about a
    /// payload this client has no link for is refused here rather than sent
    /// somewhere it would be refused: the two would mean different things,
    /// and this one is about the ground's own configuration.
    payloads: Vec<(DHName, Box<dyn Connection>)>,
    /// The one link every node of a version 2 graph is commanded on.
    ///
    /// One socket and not one per node: a node command carries the node's id
    /// -- see [`Command::node_id`] -- so the message says which node, where
    /// version 1 needed a socket apiece to say it. A graph also has more
    /// nodes than a payload set has payloads, so a socket each would spend
    /// ports on computations and logs that nothing addresses from the ground.
    ///
    /// `None` for a client built for a version 1 payload set, which has no
    /// nodes to command.
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
}

impl TcsClient {
    /// Create a new client with the spacecraft's link and one per payload.
    ///
    /// All of them, because a client without a payload's link could not send
    /// anything about that payload: the spacecraft takes each payload's
    /// commands on its own socket and refuses one that arrives on another's,
    /// and rightly, since a command on the wrong link means the two ends are
    /// not reading the same configuration.
    pub fn new(
        connection: Box<dyn Connection>,
        payloads: Vec<(DHName, Box<dyn Connection>)>,
    ) -> Self {
        Self {
            connection,
            payloads,
            nodes: None,
            sequence: AtomicU32::new(1),
            timeout: DEFAULT_TIMEOUT,
            last_sent: None,
            last_received: None,
        }
    }

    /// A client for a version 2 vehicle: the spacecraft's link and the one
    /// link every node is commanded on.
    ///
    /// The two links are version 1's two, for version 1's reason, and the
    /// second is where they differ: one socket for every node, at
    /// `payload_command.dest_base` with nothing added to it.
    ///
    /// A client built this way has no payload links and sends no version 1
    /// payload command, which is as it should be -- a graph has nodes, and a
    /// vehicle flying one refuses a command naming a data handler.
    pub fn of_a_graph(connection: Box<dyn Connection>, nodes: Box<dyn Connection>) -> Self {
        Self {
            connection,
            payloads: Vec::new(),
            nodes: Some(nodes),
            sequence: AtomicU32::new(1),
            timeout: DEFAULT_TIMEOUT,
            last_sent: None,
            last_received: None,
        }
    }

    /// When the last command went out, if one has.
    pub fn last_sent(&self) -> Option<Timestamp> {
        self.last_sent
    }

    /// When the last answer came back, if one has.
    pub fn last_received(&self) -> Option<Timestamp> {
        self.last_received
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
    fn link_for(&mut self, command: &Command) -> TcsResult<&mut Box<dyn Connection>> {
        // A version 2 node command, which names a node rather than a payload:
        // one link for all of them, the id in the message saying which. Asked
        // first because it is the narrower question -- a command either
        // carries a node id or it does not.
        if command.node_id().is_some() {
            return self.nodes.as_mut().ok_or_else(|| {
                TcsError::Config(format!(
                    "{:?} is about a node of a version 2 graph and this client has \
                     no link for one: it was built for a version 1 payload set",
                    command.cmd_type()
                ))
            });
        }

        match (command.link(), command.payload_name()) {
            (CommandLink::Payload, Some(name)) => {
                let name = name.clone();
                self.payloads
                    .iter_mut()
                    .find(|(theirs, _)| *theirs == name)
                    .map(|(_, link)| link)
                    .ok_or_else(|| {
                        TcsError::Config(format!(
                            "there is no link for payload \"{}\": this ground \
                             software read a payload set that does not describe it",
                            name.0
                        ))
                    })
            }
            // A payload command with no payload in it cannot be placed, and
            // the table says every one of them carries a name: this is the
            // case that cannot happen, said rather than guessed at.
            (CommandLink::Payload, None) => Err(TcsError::Protocol(format!(
                "{:?} is a payload command and names no payload",
                command.cmd_type()
            ))),
            (CommandLink::Spacecraft, _) => Ok(&mut self.connection),
        }
    }

    /// Send a command and wait for the response
    fn send_command(&mut self, command: Command) -> TcsResult<Telemetry> {
        let timeout = self.timeout;
        let link = self.link_for(&command)?;

        // Timed here, around the one place every command goes through. The
        // times are the ground's own clock: what they are read for is how
        // long an answer took and whether one came at all, which the
        // spacecraft's own time in the answer cannot say.
        link.send(&command)?;
        self.last_sent = Some(Timestamp::now());

        let answer = self.link_for(&command)?.receive_timeout(timeout);
        if answer.is_ok() {
            self.last_received = Some(Timestamp::now());
        }
        answer
    }

    // -- global control and status ----------------------------------------

    /// Send a PING command
    pub fn ping(&mut self) -> TcsResult<tcslibgs::PingTelemetry> {
        let seq = self.next_sequence();
        let cmd = Command::Ping(PingCommand::new(seq));
        let response = self.send_command(cmd)?;

        match response {
            Telemetry::Ping(tm) => Ok(tm),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
        }
    }

    /// Send a RESTART_ARM command
    pub fn restart_arm(&mut self, arm_key: ArmKey) -> TcsResult<CommandStatus> {
        let seq = self.next_sequence();
        let cmd = Command::RestartArm(RestartArmCommand::new(seq, arm_key));
        let response = self.send_command(cmd)?;

        match response {
            Telemetry::RestartArm(tm) => Ok(tm.header.status),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
        }
    }

    /// Say what this end read, and hear what the other end read.
    ///
    /// Sent when a link is opened. What comes back is the spacecraft's own
    /// version and the digest of the configuration it is serving, for the
    /// caller to hold against the two it sent: this returns both rather than
    /// a verdict, because the caller is the end that knows which file it read
    /// and can say so.
    pub fn connect(
        &mut self,
        version: ConfigVersion,
        digest: ConfigDigest,
    ) -> TcsResult<(ConfigVersion, ConfigVersion, ConfigDigest)> {
        let seq = self.next_sequence();
        let command = Command::Connect(ConnectCommand::new(seq, version, digest));
        let timeout = self.timeout;

        // The spacecraft's own link, and only that one. A CONNECT is about
        // having a connection to the vehicle, which is what this link is; a
        // payload's link carries what is asked of a payload, and a ground
        // station with no connection has nothing to ask one. See
        // `tcslibgs::CommandType::link`.
        self.connection.send(&command)?;
        self.last_sent = Some(Timestamp::now());

        let answer = self.connection.receive_timeout(timeout)?;
        self.last_received = Some(Timestamp::now());

        match answer {
            Telemetry::Connect(tm) => Ok((tm.version, tm.config_version, tm.digest)),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
        }
    }

    /// Send a RESTART command
    pub fn restart(&mut self, arm_key: ArmKey) -> TcsResult<CommandStatus> {
        let seq = self.next_sequence();
        let cmd = Command::Restart(RestartCommand::new(seq, arm_key));
        let response = self.send_command(cmd)?;

        match response {
            Telemetry::Restart(tm) => Ok(tm.header.status),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
        }
    }

    // -- per data handler --------------------------------------------------

    /// Send a START_DH command
    pub fn start_dh(&mut self, dh_type: DHType, name: DHName) -> TcsResult<CommandStatus> {
        let seq = self.next_sequence();
        let cmd = Command::StartDH(StartDHCommand::new(seq, dh_type, name));
        let response = self.send_command(cmd)?;

        match response {
            Telemetry::StartDH(tm) => Ok(tm.header.status),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
        }
    }

    /// Send a STOP_DH command
    pub fn stop_dh(&mut self, name: DHName) -> TcsResult<CommandStatus> {
        let seq = self.next_sequence();
        let cmd = Command::StopDH(StopDHCommand::new(seq, name));
        let response = self.send_command(cmd)?;

        match response {
            Telemetry::StopDH(tm) => Ok(tm.header.status),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
        }
    }

    // -- a version 2 graph's nodes -----------------------------------------

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
    pub fn start_node(&mut self, node: NodeId) -> TcsResult<(CommandStatus, NodeId)> {
        let seq = self.next_sequence();
        let cmd = Command::StartNode(NodeCommand::start(seq, node));
        match self.send_command(cmd)? {
            Telemetry::StartNode(tm) => Ok((tm.header.status, tm.node)),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
        }
    }

    /// Disable a node: STOP_NODE.
    ///
    /// Answered when the node has stopped *and* given up what it held, so a
    /// success means its device, bus address or socket is free.
    pub fn stop_node(&mut self, node: NodeId) -> TcsResult<(CommandStatus, NodeId)> {
        let seq = self.next_sequence();
        let cmd = Command::StopNode(NodeCommand::stop(seq, node));
        match self.send_command(cmd)? {
            Telemetry::StopNode(tm) => Ok((tm.header.status, tm.node)),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
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
    pub fn set_node(
        &mut self,
        node: NodeId,
        attribute: &str,
        value: &[u8],
    ) -> TcsResult<(CommandStatus, NodeId)> {
        let seq = self.next_sequence();
        let cmd = Command::SetNode(SetNodeCommand::new(seq, node, attribute, value));
        match self.send_command(cmd)? {
            Telemetry::SetNode(tm) => Ok((tm.header.status, tm.node)),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
        }
    }

    /// What a node has moved, and whether it is running: QUERY_NODE.
    ///
    /// Both, because they are read together: counts that have stopped going
    /// up mean one thing for a node the ground disabled and another for one
    /// that is running. A disabled node answers with its counters all the
    /// same -- they say what it has done since the vehicle started, not since
    /// it was last enabled.
    pub fn query_node(
        &mut self,
        node: NodeId,
    ) -> TcsResult<(CommandStatus, bool, NodeCounts)> {
        let seq = self.next_sequence();
        let cmd = Command::QueryNode(NodeCommand::query(seq, node));
        match self.send_command(cmd)? {
            Telemetry::QueryNode(tm) => Ok((tm.header.status, tm.enabled, tm.counts)),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
        }
    }

    /// The last datum a node moved each way: QUERY_NODE_SAMPLE.
    ///
    /// Taken first and given second, which is the order the answer carries
    /// them. For an operator who wants to see the bytes rather than a count
    /// of them; the counts are [`TcsClient::query_node`]'s, and asking for
    /// both every poll would spend downlink on data nobody reads.
    pub fn query_node_sample(
        &mut self,
        node: NodeId,
    ) -> TcsResult<(CommandStatus, DHSample, DHSample)> {
        let seq = self.next_sequence();
        let cmd = Command::QueryNodeSample(NodeCommand::query_sample(seq, node));
        match self.send_command(cmd)? {
            Telemetry::QueryNodeSample(tm) => Ok((tm.header.status, tm.taken, tm.given)),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
        }
    }

    // -- one payload of a version 1 set ------------------------------------

    /// Send a QUERY_DH command
    ///
    /// Three things come back: how the command was answered, what the handler
    /// is doing, and what it has moved. The state is `None` for a handler the
    /// spacecraft has not got, which is the NotFound answer.
    pub fn query_dh(
        &mut self,
        name: DHName,
    ) -> TcsResult<(CommandStatus, Option<DHState>, Statistics)> {
        let seq = self.next_sequence();
        let cmd = Command::QueryDH(QueryDHCommand::new(seq, name));
        let response = self.send_command(cmd)?;

        match response {
            Telemetry::QueryDH(tm) => Ok((tm.header.status, tm.state, tm.statistics)),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
        }
    }

    /// Send a QUERY_DH_SAMPLE command
    ///
    /// Returns what the handler last sent and last received, in that order.
    pub fn query_dh_sample(
        &mut self,
        name: DHName,
    ) -> TcsResult<(CommandStatus, DHSample, DHSample)> {
        let seq = self.next_sequence();
        let cmd = Command::QueryDHSample(QueryDHSampleCommand::new(seq, name));
        let response = self.send_command(cmd)?;

        match response {
            Telemetry::QueryDHSample(tm) => Ok((tm.header.status, tm.sent, tm.received)),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
        }
    }

    /// Send a CONFIG command
    pub fn configure(&mut self, beacon_interval: BeaconInterval) -> TcsResult<CommandStatus> {
        let seq = self.next_sequence();
        let cmd = Command::Config(ConfigCommand::new(seq, beacon_interval));
        let response = self.send_command(cmd)?;

        match response {
            Telemetry::Config(tm) => Ok(tm.header.status),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
        }
    }

    // -- telemetry and the connection itself -------------------------------

    /// Receive telemetry (blocking)
    pub fn receive_telemetry(&mut self) -> TcsResult<Telemetry> {
        self.connection.receive()
    }

    /// Receive telemetry with timeout
    pub fn receive_telemetry_timeout(&mut self, timeout: Duration) -> TcsResult<Telemetry> {
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
    pub fn close(&mut self) -> TcsResult<()> {
        let mut outcome = self.connection.close();
        for (_, link) in self.payloads.iter_mut() {
            outcome = outcome.and(link.close());
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
        payloads: Vec<(DHName, Box<dyn Connection>)>,
    ) -> TcsClient {
        let mut client = TcsClient::new(connection, payloads);
        client.set_timeout(self.timeout);
        client
    }
}

impl Default for TcsClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_builder() {
        let builder = TcsClientBuilder::new()
            .timeout(Duration::from_secs(10));
        assert_eq!(builder.timeout, Duration::from_secs(10));
    }

    /// A link that answers whatever it is told to, and remembers what it was
    /// asked.
    struct Canned {
        answer: Telemetry,
        asked: Vec<Command>,
    }

    impl Canned {
        fn answering(answer: Telemetry) -> Box<Canned> {
            Box::new(Canned {
                answer,
                asked: Vec::new(),
            })
        }
    }

    impl Connection for Canned {
        fn send(&mut self, command: &Command) -> TcsResult<()> {
            self.asked.push(command.clone());
            Ok(())
        }

        fn receive(&mut self) -> TcsResult<Telemetry> {
            Ok(self.answer.clone())
        }

        fn receive_timeout(&mut self, _timeout: Duration) -> TcsResult<Telemetry> {
            Ok(self.answer.clone())
        }

        fn has_data(&self) -> TcsResult<bool> {
            Ok(true)
        }

        fn close(&mut self) -> TcsResult<()> {
            Ok(())
        }
    }

    fn an_answer(config_version: ConfigVersion) -> Telemetry {
        Telemetry::Connect(tcslibgs::ConnectTelemetry::new(
            1,
            ConfigVersion::of_this_build(),
            config_version,
            ConfigDigest([0x5A; 16]),
        ))
    }

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
        let mut client = TcsClient::new(
            Canned::answering(Telemetry::Ping(tcslibgs::PingTelemetry::new(
                1,
                CommandStatus::Success,
            ))),
            vec![(DHName::new("DH0"), Canned::answering(an_answer(a_version(1))))],
        );

        assert_eq!(client.last_sent(), None, "nothing has been sent yet");
        assert_eq!(client.last_received(), None);

        let before = Timestamp::now();
        client.ping().expect("the canned answer");
        let after = Timestamp::now();

        let sent = client.last_sent().expect("the command was timed");
        let received = client.last_received().expect("so was the answer");
        assert!(sent >= before && sent <= after, "sent at {sent}");
        assert!(received >= sent && received <= after, "answered at {received}");
    }

    /// A CONNECT is asked of the spacecraft's link and of no payload's.
    ///
    /// It is about having a connection to the vehicle, which is what that
    /// link is; a payload's link carries what is asked of a payload, and a
    /// ground station with no connection has nothing to ask one.
    ///
    /// The payload link here answers with something a CONNECT never gets, so
    /// a handshake that asked it would fail rather than pass by luck.
    #[test]
    fn what_each_end_read_is_asked_of_the_spacecraft_alone() {
        let mut client = TcsClient::new(
            Canned::answering(an_answer(a_version(1))),
            vec![(
                DHName::new("DH0"),
                Canned::answering(Telemetry::Ping(tcslibgs::PingTelemetry::new(
                    1,
                    CommandStatus::Success,
                ))),
            )],
        );

        let (_, config_version, _) = client
            .connect(ConfigVersion::of_this_build(), ConfigDigest([0x5A; 16]))
            .expect("the spacecraft's link is the one that answers it");
        assert_eq!(config_version, a_version(1));
    }
}
