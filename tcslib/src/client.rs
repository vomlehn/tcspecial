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
    ArmKey, BeaconTime, Command, CommandLink, CommandStatus, ConfigCommand, DHId, DHName,
    DHSample, DHType,
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
    /// A payload's link: start one, stop one, ask one what it has moved.
    payload: Box<dyn Connection>,
    sequence: AtomicU32,
    timeout: Duration,
}

impl TcsClient {
    /// Create a new client with a connection to each of the two links.
    ///
    /// Both, because a client with one could only send half of what there is
    /// to send: the spacecraft refuses a payload command that arrives on its
    /// own link, and rightly, since a command on the wrong link means the two
    /// ends are not reading the same configuration.
    pub fn new(connection: Box<dyn Connection>, payload: Box<dyn Connection>) -> Self {
        Self {
            connection,
            payload,
            sequence: AtomicU32::new(1),
            timeout: DEFAULT_TIMEOUT,
        }
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
    fn link_for(&mut self, command: &Command) -> &mut Box<dyn Connection> {
        match command.link() {
            CommandLink::Payload => &mut self.payload,
            CommandLink::Spacecraft | CommandLink::Either => &mut self.connection,
        }
    }

    /// Send a command and wait for the response
    fn send_command(&mut self, command: Command) -> TcsResult<Telemetry> {
        let timeout = self.timeout;
        let link = self.link_for(&command);
        link.send(&command)?;
        link.receive_timeout(timeout)
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
        let payload = self.connect_on(ConnectLink::Payload, version, digest)?;

        // Both links, and they have to answer alike. Two links pointed at two
        // processes is the one way this arrangement can be wrong that nothing
        // else would catch: each link would work, each answer would look
        // right, and the payloads being commanded would not be the ones being
        // restarted.
        if spacecraft != payload {
            return Err(TcsError::Protocol(format!(
                "the two command links answered differently -- {} {} {} on one and \
                 {} {} {} on the other -- so they are not one spacecraft",
                spacecraft.0, spacecraft.1, spacecraft.2, payload.0, payload.1, payload.2
            )));
        }

        Ok(spacecraft)
    }

    /// Ask one link what the end that answers it read.
    fn connect_on(
        &mut self,
        which: ConnectLink,
        version: ConfigVersion,
        digest: ConfigDigest,
    ) -> TcsResult<(ConfigVersion, ConfigVersion, ConfigDigest)> {
        let seq = self.next_sequence();
        let command = Command::Connect(ConnectCommand::new(seq, version, digest));
        let timeout = self.timeout;
        let link = match which {
            ConnectLink::Spacecraft => &mut self.connection,
            ConnectLink::Payload => &mut self.payload,
        };

        link.send(&command)?;
        match link.receive_timeout(timeout)? {
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
    pub fn start_dh(&mut self, dh_id: DHId, dh_type: DHType, name: DHName) -> TcsResult<CommandStatus> {
        let seq = self.next_sequence();
        let cmd = Command::StartDH(StartDHCommand::new(seq, dh_id, dh_type, name));
        let response = self.send_command(cmd)?;

        match response {
            Telemetry::StartDH(tm) => Ok(tm.header.status),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
        }
    }

    /// Send a STOP_DH command
    pub fn stop_dh(&mut self, dh_id: DHId) -> TcsResult<CommandStatus> {
        let seq = self.next_sequence();
        let cmd = Command::StopDH(StopDHCommand::new(seq, dh_id));
        let response = self.send_command(cmd)?;

        match response {
            Telemetry::StopDH(tm) => Ok(tm.header.status),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
        }
    }

    /// Send a QUERY_DH command
    pub fn query_dh(&mut self, dh_id: DHId) -> TcsResult<(CommandStatus, Statistics)> {
        let seq = self.next_sequence();
        let cmd = Command::QueryDH(QueryDHCommand::new(seq, dh_id));
        let response = self.send_command(cmd)?;

        match response {
            Telemetry::QueryDH(tm) => Ok((tm.header.status, tm.statistics)),
            _ => Err(TcsError::Protocol("Unexpected telemetry type".to_string())),
        }
    }

    /// Send a QUERY_DH_SAMPLE command
    ///
    /// Returns what the handler last sent and last received, in that order.
    pub fn query_dh_sample(
        &mut self,
        dh_id: DHId,
    ) -> TcsResult<(CommandStatus, DHSample, DHSample)> {
        let seq = self.next_sequence();
        let cmd = Command::QueryDHSample(QueryDHSampleCommand::new(seq, dh_id));
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

    /// Close both links.
    ///
    /// Both are closed even if the first complains, because a client whose
    /// links are half closed is worse than one whose close was reported: the
    /// error comes back, the sockets go.
    pub fn close(&mut self) -> TcsResult<()> {
        let first = self.connection.close();
        let second = self.payload.close();
        first.and(second)
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
        payload: Box<dyn Connection>,
    ) -> TcsClient {
        let mut client = TcsClient::new(connection, payload);
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
            Canned::answering(an_answer(a_version(2))),
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
            Canned::answering(an_answer(a_version(1))),
        );
        let (_, config_version, _) = client
            .connect(ConfigVersion::of_this_build(), ConfigDigest([0x5A; 16]))
            .expect("one spacecraft");
        assert_eq!(config_version, a_version(1));
    }
}
