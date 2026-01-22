//! High-level client interface for tcslib
//!
//! Provides a convenient API for sending commands and receiving telemetry.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use tcslibgs::{
    commands::*,
    telemetry::*,
    TcsError, TcsResult,
    ArmKey, BeaconTime, DHId, DHName, DHType, Statistics,
};
use crate::connection::{Connection, ConnectionConfig};

/// High-level client for communicating with tcspecial
pub struct TcsClient {
    connection: Connection,
    sequence: AtomicU32,
}

impl TcsClient {
    /// Create a new client connecting to the given remote address
    pub fn connect(remote_addr: SocketAddr, local_addr: SocketAddr) -> TcsResult<Self> {
        let config = ConnectionConfig::new(remote_addr, local_addr);
        let connection = Connection::new(config)?;
        Ok(Self {
            connection,
            sequence: AtomicU32::new(1),
        })
    }

    /// Get the next sequence number
    fn next_sequence(&self) -> u32 {
        self.sequence.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a PING command and receive response
    pub fn ping(&mut self) -> TcsResult<PingTelemetry> {
        let seq = self.next_sequence();
        let cmd = Command::Ping(PingCommand::new(seq));
        let tlm = self.connection.send_and_recv(cmd)?;

        match tlm {
            Telemetry::Ping(ping_tlm) => Ok(ping_tlm),
            _ => Err(TcsError::protocol("Unexpected telemetry type")),
        }
    }

    /// Arm a restart with the given key
    pub fn restart_arm(&mut self, arm_key: ArmKey) -> TcsResult<RestartArmTelemetry> {
        let seq = self.next_sequence();
        let cmd = Command::RestartArm(RestartArmCommand::new(seq, arm_key));
        let tlm = self.connection.send_and_recv(cmd)?;

        match tlm {
            Telemetry::RestartArm(restart_arm_tlm) => Ok(restart_arm_tlm),
            _ => Err(TcsError::protocol("Unexpected telemetry type")),
        }
    }

    /// Execute a restart (must be armed first)
    pub fn restart(&mut self, arm_key: ArmKey) -> TcsResult<RestartTelemetry> {
        let seq = self.next_sequence();
        let cmd = Command::Restart(RestartCommand::new(seq, arm_key));
        let tlm = self.connection.send_and_recv(cmd)?;

        match tlm {
            Telemetry::Restart(restart_tlm) => Ok(restart_tlm),
            _ => Err(TcsError::protocol("Unexpected telemetry type")),
        }
    }

    /// Start a data handler
    pub fn start_dh(
        &mut self,
        dh_id: DHId,
        dh_type: DHType,
        name: DHName,
    ) -> TcsResult<StartDHTelemetry> {
        let seq = self.next_sequence();
        let cmd = Command::StartDH(StartDHCommand::new(seq, dh_id, dh_type, name));
        let tlm = self.connection.send_and_recv(cmd)?;

        match tlm {
            Telemetry::StartDH(start_dh_tlm) => Ok(start_dh_tlm),
            _ => Err(TcsError::protocol("Unexpected telemetry type")),
        }
    }

    /// Stop a data handler
    pub fn stop_dh(&mut self, dh_id: DHId) -> TcsResult<StopDHTelemetry> {
        let seq = self.next_sequence();
        let cmd = Command::StopDH(StopDHCommand::new(seq, dh_id));
        let tlm = self.connection.send_and_recv(cmd)?;

        match tlm {
            Telemetry::StopDH(stop_dh_tlm) => Ok(stop_dh_tlm),
            _ => Err(TcsError::protocol("Unexpected telemetry type")),
        }
    }

    /// Query statistics from a data handler
    pub fn query_dh(&mut self, dh_id: DHId) -> TcsResult<Statistics> {
        let seq = self.next_sequence();
        let cmd = Command::QueryDH(QueryDHCommand::new(seq, dh_id));
        let tlm = self.connection.send_and_recv(cmd)?;

        match tlm {
            Telemetry::QueryDH(query_dh_tlm) => {
                if let ResponseStatus::Failure(code) = query_dh_tlm.base.status {
                    return Err(TcsError::command(code, "Query DH failed"));
                }
                query_dh_tlm.statistics.ok_or_else(|| {
                    TcsError::protocol("Missing statistics in response")
                })
            }
            _ => Err(TcsError::protocol("Unexpected telemetry type")),
        }
    }

    /// Configure TCSpecial global settings
    pub fn configure(&mut self, beacon_interval: Option<BeaconTime>) -> TcsResult<ConfigTelemetry> {
        let seq = self.next_sequence();
        let mut cmd_inner = ConfigCommand::new(seq);
        if let Some(interval) = beacon_interval {
            cmd_inner = cmd_inner.with_beacon_interval(interval);
        }
        let cmd = Command::Config(cmd_inner);
        let tlm = self.connection.send_and_recv(cmd)?;

        match tlm {
            Telemetry::Config(config_tlm) => Ok(config_tlm),
            _ => Err(TcsError::protocol("Unexpected telemetry type")),
        }
    }

    /// Configure a data handler
    pub fn configure_dh(&mut self, dh_id: DHId) -> TcsResult<ConfigDHTelemetry> {
        let seq = self.next_sequence();
        let cmd = Command::ConfigDH(ConfigDHCommand::new(seq, dh_id));
        let tlm = self.connection.send_and_recv(cmd)?;

        match tlm {
            Telemetry::ConfigDH(config_dh_tlm) => Ok(config_dh_tlm),
            _ => Err(TcsError::protocol("Unexpected telemetry type")),
        }
    }

    /// Receive asynchronous telemetry (like beacons)
    pub fn recv_async_telemetry(&mut self) -> TcsResult<Telemetry> {
        self.connection.recv_telemetry()
    }

    /// Get the underlying connection for advanced use
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Get mutable access to the underlying connection
    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.connection
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequence_increment() {
        // This test just verifies the atomic counter works
        let counter = AtomicU32::new(1);
        assert_eq!(counter.fetch_add(1, Ordering::Relaxed), 1);
        assert_eq!(counter.fetch_add(1, Ordering::Relaxed), 2);
    }
}
