//! TCSpecial client for ground software integration

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tcslibgs::{
    ArmKey, BeaconTime, Command, CommandStatus, ConfigCommand, DHId, DHName, DHType,
    PingCommand, QueryDHCommand, RestartArmCommand, RestartCommand, StartDHCommand,
    Statistics, StopDHCommand, TcsError, TcsResult, Telemetry,
};

use tcslib::Connection;

/// Default timeout for command responses
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// TCSpecial client for sending commands and receiving telemetry
pub struct TcsClient {
    connection: Box<dyn Connection>,
    sequence: AtomicU32,
    timeout: Duration,
}

impl TcsClient {
    /// Create a new client with the given connection
    pub fn new(connection: Box<dyn Connection>) -> Self {
        Self {
            connection,
            sequence: AtomicU32::new(1),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Set the command timeout
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Get the next sequence number
    fn next_sequence(&self) -> u32 {
        self.sequence.fetch_add(1, Ordering::SeqCst)
    }

    /// Send a command and wait for the response
    fn send_command(&mut self, command: Command) -> TcsResult<Telemetry> {
        self.connection.send(&command)?;
        self.connection.receive_timeout(self.timeout)
    }

    /// Send a PING command
    pub fn ping(&mut self) -> TcsResult<tcslibgs::PingTelemetry> {
        let seq = self.next_sequence();
        let cmd = Command::Ping(PingCommand::new(seq));
eprintln!("sending ping command");
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

    /// Close the client connection
    pub fn close(&mut self) -> TcsResult<()> {
        self.connection.close()
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

    pub fn build(self, connection: Box<dyn Connection>) -> TcsClient {
        let mut client = TcsClient::new(connection);
eprintln!("build: set client");
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
}
