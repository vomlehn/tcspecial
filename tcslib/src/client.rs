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
    ArmKey, BeaconTime, Command, CommandLink, CommandStatus, ConfigCommand, DHName, DHState,
    DHSample, DHType, Timestamp,
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
            (CommandLink::Spacecraft | CommandLink::Either, _) => Ok(&mut self.connection),
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
        let spacecraft = self.connect_on(ConnectLink::Spacecraft, version, digest)?;

        // And the first payload's link, where there is one. Two links pointed
        // at two processes is the one way this arrangement can be wrong that
        // nothing else would catch: each link would work, each answer would
        // look right, and the payloads being commanded would not be the ones
        // being restarted.
        //
        // The first of them rather than all: the payload links are sockets of
        // one process, so one of them answering as the spacecraft link does
        // says what asking all of them would, and a set of forty would be
        // forty exchanges before the window opened.
        if !self.payloads.is_empty() {
            let payload = self.connect_on(ConnectLink::Payload, version, digest)?;
            if spacecraft != payload {
                return Err(TcsError::Protocol(format!(
                    "the two command links answered differently -- {} {} {} on one \
                     and {} {} {} on the other -- so they are not one spacecraft",
                    spacecraft.0, spacecraft.1, spacecraft.2, payload.0, payload.1,
                    payload.2
                )));
            }
        }

        Ok(spacecraft)
    }

    /// Ask one link what the end that answers it read.
    ///
    /// `ConnectLink::Payload` is the first payload's link, and asking it with
    /// no payloads is a caller's mistake rather than a thing to guess at.
    fn connect_on(
        &mut self,
        which: ConnectLink,
        version: ConfigVersion,
        digest: ConfigDigest,
    ) -> TcsResult<(ConfigVersion, ConfigVersion, ConfigDigest)> {
        let seq = self.next_sequence();
        let command = Command::Connect(ConnectCommand::new(seq, version, digest));
        let timeout = self.timeout;
        let link = Self::connect_link(&mut self.connection, &mut self.payloads, which)?;

        link.send(&command)?;
        self.last_sent = Some(Timestamp::now());

        let answer =
            Self::connect_link(&mut self.connection, &mut self.payloads, which)?
                .receive_timeout(timeout)?;
        self.last_received = Some(Timestamp::now());

        match answer {
            Telemetry::Connect(tm) => Ok((tm.version, tm.config_version, tm.digest)),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
        }
    }

    /// Which link a CONNECT goes on, as a borrow of one field or another.
    ///
    /// Taken apart from `self` so that the send and the receive can each have
    /// it in turn: one `&mut self` method cannot hand out two.
    fn connect_link<'a>(
        connection: &'a mut Box<dyn Connection>,
        payloads: &'a mut [(DHName, Box<dyn Connection>)],
        which: ConnectLink,
    ) -> TcsResult<&'a mut Box<dyn Connection>> {
        match which {
            ConnectLink::Spacecraft => Ok(connection),
            ConnectLink::Payload => payloads
                .first_mut()
                .map(|(_, link)| link)
                .ok_or_else(|| {
                    TcsError::Config(
                        "this payload set describes no payloads, so there is no \
                         payload link to ask"
                            .to_string(),
                    )
                }),
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
    pub fn configure(&mut self, beacon_interval: BeaconTime) -> TcsResult<CommandStatus> {
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

/// Which link a CONNECT is being asked on.
///
/// A CONNECT is answered on either, so asking takes a side; every other
/// command has one by its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectLink {
    Spacecraft,
    Payload,
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

    /// Two links pointed at two spacecraft are refused.
    ///
    /// The one way this arrangement can be wrong that nothing else would
    /// catch: each link works, each answer looks right, and the payloads
    /// being commanded are not the ones being restarted. So the handshake is
    /// asked of both links and the answers have to agree.
    #[test]
    fn two_links_that_answer_differently_are_not_one_spacecraft() {
        let mut client = TcsClient::new(
            Canned::answering(an_answer(a_version(1))),
            vec![(DHName::new("DH0"), Canned::answering(an_answer(a_version(2))))],
        );

        let said = client
            .connect(ConfigVersion::of_this_build(), ConfigDigest([0x5A; 16]))
            .expect_err("two spacecraft are not one")
            .to_string();
        assert!(
            said.contains("not one spacecraft") && said.contains("1.0.0") && said.contains("2.0.0"),
            "the error should name both answers, but said: {said}"
        );

        // And two links at one spacecraft answer alike, which is what the
        // test above is about: see them agree before believing the refusal.
        let mut client = TcsClient::new(
            Canned::answering(an_answer(a_version(1))),
            vec![(DHName::new("DH0"), Canned::answering(an_answer(a_version(1))))],
        );
        let (_, config_version, _) = client
            .connect(ConfigVersion::of_this_build(), ConfigDigest([0x5A; 16]))
            .expect("one spacecraft");
        assert_eq!(config_version, a_version(1));
    }
}
