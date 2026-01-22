//! Command Interpreter implementation for tcspecial
//!
//! The CI has threads to manage commands from the OC and status messages to the OC.
//! I/O is done with datagrams. Status messages are queued with a fixed-length queue.

use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use log::{info, warn, error, debug};
use tcslibgs::{
    Command, Telemetry, TcsError, TcsResult,
    ArmKey, BeaconTime, DHId, DHName, DHType, Timestamp,
    commands::*,
    telemetry::*,
    protocol::{ProtocolMessage, MessagePayload, MAX_MESSAGE_SIZE},
};
use crate::config::{TcspecialConfig, DHConfig};
use crate::dh::DHManager;

/// Arm window duration in seconds
const ARM_WINDOW_SECS: u64 = 60;

/// Command Interpreter state
pub struct CommandInterpreter {
    /// Configuration
    config: TcspecialConfig,
    /// UDP socket for OC communication
    socket: UdpSocket,
    /// Data handler manager
    dh_manager: DHManager,
    /// Running flag
    running: Arc<AtomicBool>,
    /// Last arm key and time
    arm_state: Option<(ArmKey, Instant)>,
    /// Beacon interval
    beacon_interval: BeaconTime,
    /// Last beacon time
    last_beacon: Instant,
    /// Receive buffer
    recv_buffer: Vec<u8>,
}

impl CommandInterpreter {
    /// Create a new command interpreter
    pub fn new(config: TcspecialConfig) -> TcsResult<Self> {
        info!("Creating Command Interpreter, listening on {}", config.listen_addr);

        let socket = UdpSocket::bind(config.listen_addr)?;
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;
        socket.set_nonblocking(false)?;

        let dh_manager = DHManager::new(config.max_data_handlers);
        let beacon_interval = config.beacon_interval;

        Ok(Self {
            config,
            socket,
            dh_manager,
            running: Arc::new(AtomicBool::new(false)),
            arm_state: None,
            beacon_interval,
            last_beacon: Instant::now(),
            recv_buffer: vec![0u8; MAX_MESSAGE_SIZE],
        })
    }

    /// Get a clone of the running flag
    pub fn running_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    /// Run the main loop
    pub fn run(&mut self) -> TcsResult<()> {
        info!("Starting Command Interpreter main loop");
        self.running.store(true, Ordering::SeqCst);

        while self.running.load(Ordering::SeqCst) {
            // Check for incoming commands
            if let Some((cmd, src_addr)) = self.try_recv_command()? {
                debug!("Received command from {}: {:?}", src_addr, cmd.id());
                let response = self.process_command(cmd, src_addr);
                self.send_telemetry(&response, src_addr)?;
            }

            // Send beacon if needed
            if self.last_beacon.elapsed() >= self.config.beacon_duration() {
                self.send_beacon()?;
                self.last_beacon = Instant::now();
            }
        }

        info!("Command Interpreter shutting down");
        self.shutdown()?;

        Ok(())
    }

    /// Stop the command interpreter
    pub fn stop(&self) {
        info!("Stopping Command Interpreter");
        self.running.store(false, Ordering::SeqCst);
    }

    /// Try to receive a command (non-blocking)
    fn try_recv_command(&mut self) -> TcsResult<Option<(Command, SocketAddr)>> {
        match self.socket.recv_from(&mut self.recv_buffer) {
            Ok((n, src_addr)) => {
                if n == 0 {
                    return Ok(None);
                }

                let msg = ProtocolMessage::from_bytes(&self.recv_buffer[..n])?;
                match msg.payload {
                    MessagePayload::Command(cmd) => Ok(Some((cmd, src_addr))),
                    _ => {
                        warn!("Received non-command message from {}", src_addr);
                        Ok(None)
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(None),
            Err(e) => Err(TcsError::Io(e)),
        }
    }

    /// Process a command and return the response telemetry
    fn process_command(&mut self, cmd: Command, src_addr: SocketAddr) -> Telemetry {
        match cmd {
            Command::Ping(ping_cmd) => self.handle_ping(ping_cmd),
            Command::RestartArm(restart_arm_cmd) => self.handle_restart_arm(restart_arm_cmd),
            Command::Restart(restart_cmd) => self.handle_restart(restart_cmd),
            Command::StartDH(start_dh_cmd) => self.handle_start_dh(start_dh_cmd, src_addr),
            Command::StopDH(stop_dh_cmd) => self.handle_stop_dh(stop_dh_cmd),
            Command::QueryDH(query_dh_cmd) => self.handle_query_dh(query_dh_cmd),
            Command::Config(config_cmd) => self.handle_config(config_cmd),
            Command::ConfigDH(config_dh_cmd) => self.handle_config_dh(config_dh_cmd),
        }
    }

    /// Handle PING command
    fn handle_ping(&self, cmd: PingCommand) -> Telemetry {
        Telemetry::Ping(PingTelemetry::new(cmd.sequence, Timestamp::now()))
    }

    /// Handle RESTART_ARM command
    fn handle_restart_arm(&mut self, cmd: RestartArmCommand) -> Telemetry {
        info!("Arming restart with key {:?}", cmd.arm_key);
        self.arm_state = Some((cmd.arm_key, Instant::now()));
        Telemetry::RestartArm(RestartArmTelemetry::success(cmd.sequence))
    }

    /// Handle RESTART command
    fn handle_restart(&mut self, cmd: RestartCommand) -> Telemetry {
        match &self.arm_state {
            Some((stored_key, arm_time)) => {
                if arm_time.elapsed() > Duration::from_secs(ARM_WINDOW_SECS) {
                    warn!("Restart arm window expired");
                    self.arm_state = None;
                    return Telemetry::Restart(RestartTelemetry::failure(
                        cmd.sequence,
                        ErrorCode::ArmWindowExpired,
                    ));
                }

                if *stored_key != cmd.arm_key {
                    warn!("Restart arm key mismatch");
                    return Telemetry::Restart(RestartTelemetry::failure(
                        cmd.sequence,
                        ErrorCode::InvalidArmKey,
                    ));
                }

                info!("Restart confirmed, initiating restart");
                self.arm_state = None;
                self.running.store(false, Ordering::SeqCst);
                Telemetry::Restart(RestartTelemetry::success(cmd.sequence))
            }
            None => {
                warn!("Restart attempted without arming");
                Telemetry::Restart(RestartTelemetry::failure(
                    cmd.sequence,
                    ErrorCode::RestartNotArmed,
                ))
            }
        }
    }

    /// Handle START_DH command
    fn handle_start_dh(&mut self, cmd: StartDHCommand, src_addr: SocketAddr) -> Telemetry {
        info!("Starting DH {}: type={:?}, name={}", cmd.dh_id, cmd.dh_type, cmd.name.0);

        let config = match cmd.dh_type {
            DHType::Network => DHConfig::datagram(),
            DHType::Device => DHConfig::stream(),
        };

        if let Err(e) = self.dh_manager.create_dh(cmd.dh_id, cmd.dh_type, cmd.name, config) {
            error!("Failed to create DH: {}", e);
            return Telemetry::StartDH(StartDHTelemetry::failure(
                cmd.sequence,
                e.to_error_code(),
            ));
        }

        if let Err(e) = self.dh_manager.activate_dh(cmd.dh_id, src_addr) {
            error!("Failed to activate DH: {}", e);
            return Telemetry::StartDH(StartDHTelemetry::failure(
                cmd.sequence,
                e.to_error_code(),
            ));
        }

        Telemetry::StartDH(StartDHTelemetry::success(cmd.sequence))
    }

    /// Handle STOP_DH command
    fn handle_stop_dh(&mut self, cmd: StopDHCommand) -> Telemetry {
        info!("Stopping DH {}", cmd.dh_id);

        match self.dh_manager.stop_dh(cmd.dh_id) {
            Ok(()) => Telemetry::StopDH(StopDHTelemetry::success(cmd.sequence)),
            Err(e) => {
                // For idempotency, stopping an already-stopped DH is success
                if matches!(e, TcsError::Command { .. }) {
                    Telemetry::StopDH(StopDHTelemetry::success(cmd.sequence))
                } else {
                    Telemetry::StopDH(StopDHTelemetry::failure(cmd.sequence, e.to_error_code()))
                }
            }
        }
    }

    /// Handle QUERY_DH command
    fn handle_query_dh(&self, cmd: QueryDHCommand) -> Telemetry {
        debug!("Querying DH {}", cmd.dh_id);

        match self.dh_manager.get_dh_stats(cmd.dh_id) {
            Ok(stats) => Telemetry::QueryDH(QueryDHTelemetry::success(cmd.sequence, stats)),
            Err(e) => Telemetry::QueryDH(QueryDHTelemetry::failure(cmd.sequence, e.to_error_code())),
        }
    }

    /// Handle CONFIG command
    fn handle_config(&mut self, cmd: ConfigCommand) -> Telemetry {
        info!("Configuring CI");

        if let Some(interval) = cmd.beacon_interval {
            self.beacon_interval = interval;
            self.config.beacon_interval = interval;
        }

        Telemetry::Config(ConfigTelemetry::success(cmd.sequence))
    }

    /// Handle CONFIG_DH command
    fn handle_config_dh(&mut self, cmd: ConfigDHCommand) -> Telemetry {
        info!("Configuring DH {}", cmd.dh_id);
        // Currently no DH-specific configuration options
        Telemetry::ConfigDH(ConfigDHTelemetry::success(cmd.sequence))
    }

    /// Send telemetry to an address
    fn send_telemetry(&self, tlm: &Telemetry, addr: SocketAddr) -> TcsResult<()> {
        let msg = ProtocolMessage::from_telemetry(tlm.clone())?;
        let bytes = msg.to_bytes()?;
        self.socket.send_to(&bytes, addr)?;
        Ok(())
    }

    /// Send beacon telemetry to broadcast
    fn send_beacon(&self) -> TcsResult<()> {
        debug!("Sending beacon");
        let beacon = BeaconTelemetry::now();
        let msg = ProtocolMessage::from_telemetry(Telemetry::Beacon(beacon))?;
        let bytes = msg.to_bytes()?;

        // For now, we don't have a broadcast address, so beacons are just logged
        // In a real implementation, this would send to configured ground stations
        debug!("Beacon: {} bytes", bytes.len());

        Ok(())
    }

    /// Shutdown the command interpreter
    fn shutdown(&mut self) -> TcsResult<()> {
        info!("Shutting down all data handlers");
        self.dh_manager.stop_all()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ci_creation() {
        let mut config = TcspecialConfig::default();
        config.listen_addr = "127.0.0.1:0".parse().unwrap();
        let ci = CommandInterpreter::new(config);
        assert!(ci.is_ok());
    }

    #[test]
    fn test_arm_window() {
        assert_eq!(ARM_WINDOW_SECS, 60);
    }
}
