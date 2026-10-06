//! Command Interpreter implementation for TCSpecial
//!
//! The CI processes commands from the OC and manages data handlers.

use std::collections::BTreeMap;
use crate::beacon_send::BeaconSend;
use std::net::UdpSocket;
//use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};
//use std::thread;
use std::time::{Duration, Instant};
use tcslibgs::{
    ArmKey, BeaconTime, CIConfig, Command, CommandStatus, ConfigTelemetry,
    DHConfig, DHId, PingTelemetry, QueryDHTelemetry, RestartArmTelemetry, RestartTelemetry,
    StartDHTelemetry, Statistics, StopDHTelemetry, TcsError, TcsResult, Telemetry,
};

use crate::config::constants::{BEACON_NETADDR, RESTART_ARM_TIMEOUT};
use crate::dh::DataHandler;
use crate::telemetry_log::TelemetryLog;

/// Command interpreter state
pub struct CommandInterpreter {
    /// The beacon sender, once the main loop has started it. Held so that a
    /// Config command can retime it.
    beacon: Option<BeaconSend>,
    beacon_interval: BeaconTime,
    _config: CIConfig,
    socket: UdpSocket,
    data_handlers: Arc<Mutex<BTreeMap<DHId, DataHandler>>>,
    payload_config: Vec<DHConfig>,
    arm_key: Option<ArmKey>,
    arm_time: Option<Instant>,
    running: bool,
    _global_stats: Statistics,
    /// Telemetry log, shared with every other sender of telemetry.
    telemetry_log: TelemetryLog,
}

impl CommandInterpreter {
    /// Create a new command interpreter
    pub fn new(config: CIConfig, payload_config: Vec<DHConfig>) -> TcsResult<Self> {
        let addr = format!("{}:{}", config.address, config.port);
        let socket = UdpSocket::bind(&addr)?;
        socket.set_nonblocking(false)?;

        // Opened here, before the main loop: see TelemetryLog::open.
        let telemetry_log = TelemetryLog::open(&config)?;

        Ok(Self {
            beacon_interval: config.beacon_interval,
            beacon: None,
            _config: config,
            socket,
            data_handlers: Arc::new(Mutex::new(BTreeMap::new())),
            payload_config,
            arm_key: None,
            arm_time: None,
            running: false,
            _global_stats: Statistics::new(),
            telemetry_log,
        })
    }

    /// Initialize data handlers from configuration
    pub fn initialize_handlers(&mut self) -> TcsResult<()> {
        let mut handlers = self.data_handlers.lock()
            .map_err(|_| TcsError::DataHandler("Lock poisoned".to_string()))?;

        for config in &self.payload_config {
            let dh = DataHandler::new(config.clone())?;
            handlers.insert(config.dh_id, dh);
        }

        Ok(())
    }

    /// Process a command and return the response telemetry
    fn process_command(&mut self, command: Command) -> Telemetry {
eprintln!("process_command: {:?}", command);
        match command {
            Command::Ping(cmd) => {
                Telemetry::Ping(PingTelemetry::new(cmd.header.sequence, CommandStatus::Success))
            }
            Command::RestartArm(cmd) => {
                self.arm_key = Some(cmd.arm_key);
                self.arm_time = Some(Instant::now());
                Telemetry::RestartArm(RestartArmTelemetry::new(cmd.header.sequence, CommandStatus::Success))
            }
            Command::Restart(cmd) => {
                let status = if let (Some(arm_key), Some(arm_time)) = (self.arm_key, self.arm_time) {
                    if arm_key == cmd.arm_key && arm_time.elapsed() < RESTART_ARM_TIMEOUT {
                        self.running = false;
                        CommandStatus::Success
                    } else {
                        CommandStatus::InvalidParameter
                    }
                } else {
                    CommandStatus::NotArmed
                };
                Telemetry::Restart(RestartTelemetry::new(cmd.header.sequence, status))
            }
            Command::StartDH(cmd) => {
                let status = {
                    let mut handlers = match self.data_handlers.lock() {
                        Ok(h) => h,
                        Err(_) => return Telemetry::StartDH(StartDHTelemetry::new(cmd.header.sequence, CommandStatus::Failure)),
                    };

                    if handlers.contains_key(&cmd.dh_id) {
                        // Idempotent - already exists
                        CommandStatus::Success
                    } else {
                        // Find config and create handler
                        if let Some(config) = self.payload_config.iter().find(|c| c.dh_id == cmd.dh_id) {
                            match DataHandler::new(config.clone()) {
                                Ok(dh) => {
                                    handlers.insert(cmd.dh_id, dh);
                                    CommandStatus::Success
                                }
                                Err(_) => CommandStatus::Failure,
                            }
                        } else {
                            CommandStatus::NotFound
                        }
                    }
                };
                Telemetry::StartDH(StartDHTelemetry::new(cmd.header.sequence, status))
            }
            Command::StopDH(cmd) => {
                let status = {
                    let mut handlers = match self.data_handlers.lock() {
                        Ok(h) => h,
                        Err(_) => return Telemetry::StopDH(StopDHTelemetry::new(cmd.header.sequence, CommandStatus::Failure)),
                    };

                    if let Some(dh) = handlers.get_mut(&cmd.dh_id) {
                        match dh.stop() {
                            Ok(_) => CommandStatus::Success,
                            Err(_) => CommandStatus::Failure,
                        }
                    } else {
                        // Idempotent - not found is also success
                        CommandStatus::Success
                    }
                };
                Telemetry::StopDH(StopDHTelemetry::new(cmd.header.sequence, status))
            }
            Command::QueryDH(cmd) => {
                let (status, stats) = {
                    let handlers = match self.data_handlers.lock() {
                        Ok(h) => h,
                        Err(_) => return Telemetry::QueryDH(QueryDHTelemetry::new(
                            cmd.header.sequence,
                            CommandStatus::Failure,
                            cmd.dh_id,
                            Statistics::new(),
                        )),
                    };

                    if let Some(dh) = handlers.get(&cmd.dh_id) {
                        (CommandStatus::Success, dh.statistics())
                    } else {
                        (CommandStatus::NotFound, Statistics::new())
                    }
                };
                Telemetry::QueryDH(QueryDHTelemetry::new(cmd.header.sequence, status, cmd.dh_id, stats))
            }
            Command::Config(cmd) => {
                self.beacon_interval = cmd.beacon_interval;
                // Retime the running sender too, or the new interval would
                // be recorded and never take effect. set_interval also
                // expires the current wait, so the next beacon goes out at
                // once rather than after the old interval elapses.
                if let Some(beacon) = self.beacon.as_mut() {
                    beacon.set_interval(Duration::from_millis(cmd.beacon_interval.0 as u64));
                }
                Telemetry::Config(ConfigTelemetry::new(cmd.header.sequence, CommandStatus::Success))
            }
            Command::ConfigDH(_cmd) => {
                // Not yet implemented
                Telemetry::ConfigDH(tcslibgs::ConfigDHTelemetry::new(_cmd.header.sequence, CommandStatus::Success))
            }
        }
    }

    /// Run the command interpreter main loop
    pub fn run(&mut self) -> TcsResult<()> {
        self.running = true;
        let mut recv_buffer = vec![0u8; 65535];
        let _last_beacon = Instant::now();
        let mut _last_client_addr: Option<std::net::SocketAddr> = None;

eprintln!("run: BEACON_NETADDR {:?}", BEACON_NETADDR);
        // Beacons go out at the configured interval, and the sender is kept
        // so that a Config command can retime it. It records into the same
        // log as the responses sent below, so the log holds everything that
        // went to the ground.
        self.beacon = BeaconSend::new(
            Duration::from_millis(self.beacon_interval.0 as u64),
            BEACON_NETADDR.parse().unwrap(),
            self.telemetry_log.clone(),
        );

        while self.running {
            // Try to receive a command
            match self.socket.recv_from(&mut recv_buffer) {
                Ok((size, addr)) => {
eprintln!("run::recv_from {:?}", addr);
                    _last_client_addr = Some(addr);

                    // Parse and process command
                    match serde_json::from_slice::<Command>(&recv_buffer[..size]) {
                        Ok(command) => {
                            let response = self.process_command(command);
                            if let Ok(data) = serde_json::to_vec(&response) {
                                self.telemetry_log.record(&data);
eprintln!("run::sendto {:?}", addr);
                                let _ = self.socket.send_to(&data, addr);
                            }
                        }
                        Err(_) => {
                            // Invalid command - ignore
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Timeout - continue loop
                    continue;
                }
                Err(e) => {
                    return Err(TcsError::Io(e));
                }
            }
        }

        Ok(())
    }

    /// Stop the command interpreter
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Shut down all data handlers
    pub fn shutdown(&mut self) -> TcsResult<()> {
        self.running = false;

        let mut handlers = self.data_handlers.lock()
            .map_err(|_| TcsError::DataHandler("Lock poisoned".to_string()))?;

        for (_, dh) in handlers.iter_mut() {
            let _ = dh.stop();
        }
        drop(handlers);

        self.telemetry_log.flush();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslibgs::NetworkProtocol;

    #[test]
    fn test_ci_creation() {
        let config = CIConfig {
            address: "127.0.0.1".to_string(),
            port: 0, // Let OS assign port
            protocol: NetworkProtocol::Udp,
            beacon_interval: BeaconTime(5000),
            log_dir: None,
            log_segment_bytes: 65_536,
        };

        let ci = CommandInterpreter::new(config, vec![]);
        assert!(ci.is_ok());
    }
}
