//! Command Interpreter implementation for TCSpecial
//!
//! The CI processes commands from the OC and manages data handlers.

use std::collections::BTreeMap;
use crate::beacon_send::BeaconSend;
use std::net::UdpSocket;
//use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};
//use std::thread;
//use std::time::{Duration, Instant};
use std::time::Instant;
use tcslibgs::{
    ArmKey, BeaconTelemetry, BeaconTime, CIConfig, Command, CommandStatus, ConfigTelemetry,
    DHConfig, DHId, PingTelemetry, QueryDHTelemetry, RestartArmTelemetry, RestartTelemetry,
    StartDHTelemetry, Statistics, StopDHTelemetry, TcsError, TcsResult, Telemetry,
};

use crate::config::constants::{BEACON_DEFAULT_MS, BEACON_NETADDR, RESTART_ARM_TIMEOUT};
use crate::dh::DataHandler;

/// Command interpreter state
pub struct CommandInterpreter {
    _beacon: Option<BeaconSend>,
    beacon_interval: BeaconTime,
    _config: CIConfig,
    socket: UdpSocket,
    data_handlers: Arc<Mutex<BTreeMap<DHId, DataHandler>>>,
    dh_configs: Vec<DHConfig>,
    arm_key: Option<ArmKey>,
    arm_time: Option<Instant>,
    running: bool,
    _global_stats: Statistics,
}

impl CommandInterpreter {
    /// Create a new command interpreter
    pub fn new(config: CIConfig, dh_configs: Vec<DHConfig>) -> TcsResult<Self> {
        let addr = format!("{}:{}", config.address, config.port);
        let socket = UdpSocket::bind(&addr)?;
        socket.set_nonblocking(false)?;

        Ok(Self {
            beacon_interval: config.beacon_interval,
            _beacon: None,
            _config: config,
            socket,
            data_handlers: Arc::new(Mutex::new(BTreeMap::new())),
            dh_configs,
            arm_key: None,
            arm_time: None,
            running: false,
            _global_stats: Statistics::new(),
        })
    }

    /// Initialize data handlers from configuration
    pub fn initialize_handlers(&mut self) -> TcsResult<()> {
        let mut handlers = self.data_handlers.lock()
            .map_err(|_| TcsError::DataHandler("Lock poisoned".to_string()))?;

        for config in &self.dh_configs {
            let dh = DataHandler::new(config.clone())?;
            handlers.insert(config.dh_id, dh);
        }

        Ok(())
    }

    /// Process a command and return the response telemetry
    fn process_command(&mut self, command: Command) -> Telemetry {
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
                        if let Some(config) = self.dh_configs.iter().find(|c| c.dh_id == cmd.dh_id) {
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
                Telemetry::Config(ConfigTelemetry::new(cmd.header.sequence, CommandStatus::Success))
            }
            Command::ConfigDH(_cmd) => {
                // Not yet implemented
                Telemetry::ConfigDH(tcslibgs::ConfigDHTelemetry::new(_cmd.header.sequence, CommandStatus::Success))
            }
        }
    }

    /// Send a beacon telemetry message
    fn _send_beacon(&self, addr: &std::net::SocketAddr) -> TcsResult<()> {
        let beacon = Telemetry::Beacon(BeaconTelemetry::new());
        let data = serde_json::to_vec(&beacon)?;
        self.socket.send_to(&data, addr)?;
        Ok(())
    }

    /// Run the command interpreter main loop
    pub fn run(&mut self) -> TcsResult<()> {
        self.running = true;
        let mut recv_buffer = vec![0u8; 65535];
        let _last_beacon = Instant::now();
        let mut _last_client_addr: Option<std::net::SocketAddr> = None;

/*
        // Set a timeout for receiving so we can send beacons
        self.socket.set_read_timeout(Some(Duration::from_millis(100)))?;
*/
eprintln!("run: BEACON_NETADDR {:?}", BEACON_NETADDR);
        let _beacon = BeaconSend::new(BEACON_DEFAULT_MS, BEACON_NETADDR.parse().unwrap());
//        let _beacon = BeaconSend::new(BEACON_DEFAULT_MS, "0.0.0.0:5550".parse().unwrap());

        while self.running {
/*
            // Check if we need to send a beacon
            if last_beacon.elapsed() >= Duration::from_millis(self.beacon_interval.0 as u64) {
                if let Some(addr) = last_client_addr {
                    let _ = self.send_beacon(&addr);
                }
                last_beacon = Instant::now();
            }
*/

            // Try to receive a command
            match self.socket.recv_from(&mut recv_buffer) {
                Ok((size, addr)) => {
                    _last_client_addr = Some(addr);

                    // Parse and process command
                    match serde_json::from_slice::<Command>(&recv_buffer[..size]) {
                        Ok(command) => {
                            let response = self.process_command(command);
                            if let Ok(data) = serde_json::to_vec(&response) {
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
        };

        let ci = CommandInterpreter::new(config, vec![]);
        assert!(ci.is_ok());
    }
}
