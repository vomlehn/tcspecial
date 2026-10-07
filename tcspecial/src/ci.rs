//! Command Interpreter implementation for TCSpecial
//!
//! The CI processes commands from the OC and manages data handlers.

use std::collections::BTreeMap;
use log::{debug, error, trace};
use crate::beacon_send::BeaconSend;
use std::net::UdpSocket;
//use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};
//use std::thread;
use std::time::{Duration, Instant};
use tcslibgs::{
    ArmKey, BeaconTime, CIConfig, Command, CommandStatus, ConfigTelemetry,
    DHConfig, DHId, DHSample, EndpointConfig, PingTelemetry, QueryDHSampleTelemetry,
    QueryDHTelemetry, RestartArmTelemetry, RestartTelemetry,
    StartDHTelemetry, Statistics, StopDHTelemetry, TcsError, TcsResult, Telemetry,
};

use crate::config::constants::{BEACON_NETADDR, RESTART_ARM_TIMEOUT};
use crate::dh::DataHandler;
use crate::endpoint::create_endpoint_pair;
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

/// Build a data handler and start it moving data.
///
/// The OC endpoints are opened here because this is where the OC's address is
/// known, from `oc` in the handler's configuration. It is opened once and both
/// halves come from that one open, for the reason the payload endpoint is: a
/// UDP address cannot be bound twice.
///
/// A handler with no OC address cannot be started. It has nowhere to send what
/// it reads from its payload and nowhere to read what it should write there,
/// so this says so rather than starting a handler that moves nothing.
fn start_handler(config: &DHConfig) -> TcsResult<DataHandler> {
    let oc = config.oc.clone().ok_or_else(|| {
        TcsError::Config(format!(
            "data handler \"{}\" has no OC address: give it oc_address and oc_port",
            config.name.0
        ))
    })?;

    let mut dh = DataHandler::new(config.clone())?;
    let (oc_reader, oc_writer) = create_endpoint_pair(&EndpointConfig::Network(oc))?;
    dh.start(oc_reader, oc_writer)?;
    Ok(dh)
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
        trace!("process_command: {:?}", command);
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
                            match start_handler(config) {
                                Ok(dh) => {
                                    handlers.insert(cmd.dh_id, dh);
                                    CommandStatus::Success
                                }
                                Err(e) => {
                                    error!(
                                        "{}: cannot start: {}",
                                        config.name.0, e
                                    );
                                    CommandStatus::Failure
                                }
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
            Command::QueryDHSample(cmd) => {
                let (status, samples) = {
                    let handlers = match self.data_handlers.lock() {
                        Ok(h) => h,
                        Err(_) => {
                            return Telemetry::QueryDHSample(QueryDHSampleTelemetry::new(
                                cmd.header.sequence,
                                CommandStatus::Failure,
                                cmd.dh_id,
                                DHSample::new(),
                                DHSample::new(),
                            ))
                        }
                    };

                    match handlers.get(&cmd.dh_id) {
                        Some(dh) => (CommandStatus::Success, dh.samples()),
                        None => (CommandStatus::NotFound, Default::default()),
                    }
                };
                Telemetry::QueryDHSample(QueryDHSampleTelemetry::new(
                    cmd.header.sequence,
                    status,
                    cmd.dh_id,
                    samples.sent,
                    samples.received,
                ))
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
            Command::ConfigDH(cmd) => {
                // Refused rather than answered Success. Nothing here
                // configures a data handler, and the command carries nothing
                // to configure it with: ConfigDHCommand has a dh_id and a
                // note that settings could be added to it. Answering Success
                // told the ground a handler had been reconfigured when
                // nothing had happened, which is worse than saying so,
                // because there is no way to tell it from the real thing.
                //
                // InvalidCommand rather than Failure: the handler did not
                // fail to be configured, the command is one tcspecial does
                // not implement.
                error!(
                    "CONFIG_DH for {:?} refused: tcspecial does not implement it",
                    cmd.dh_id
                );
                Telemetry::ConfigDH(tcslibgs::ConfigDHTelemetry::new(
                    cmd.header.sequence,
                    CommandStatus::InvalidCommand,
                ))
            }
        }
    }

    /// Run the command interpreter main loop
    pub fn run(&mut self) -> TcsResult<()> {
        self.running = true;
        let mut recv_buffer = vec![0u8; 65535];
        let _last_beacon = Instant::now();
        let mut _last_client_addr: Option<std::net::SocketAddr> = None;

        debug!("beacon destination {}", BEACON_NETADDR);
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
                    trace!("run: received from {:?}", addr);
                    _last_client_addr = Some(addr);

                    // Parse and process command
                    match serde_json::from_slice::<Command>(&recv_buffer[..size]) {
                        Ok(command) => {
                            let response = self.process_command(command);
                            if let Ok(data) = serde_json::to_vec(&response) {
                                self.telemetry_log.record(&data);
                                trace!("run: sending to {:?}", addr);
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
    use tcslibgs::{ConfigDHCommand, NetworkProtocol};

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

    /// A command interpreter on a port the OS picks, with no payloads.
    fn interpreter() -> CommandInterpreter {
        CommandInterpreter::new(
            CIConfig {
                address: "127.0.0.1".to_string(),
                port: 0,
                protocol: NetworkProtocol::Udp,
                beacon_interval: BeaconTime(5000),
                log_dir: None,
                log_segment_bytes: 65_536,
            },
            vec![],
        )
        .expect("an interpreter")
    }

    /// CONFIG_DH is refused rather than answered with success.
    ///
    /// It answered Success while doing nothing, and the command carries
    /// nothing to configure a handler with, so the ground was told a handler
    /// had been reconfigured when none had. A control application cannot tell
    /// a success it earned from one it did not; this pins the refusal, and is
    /// what will say so if the command is ever implemented.
    #[test]
    fn config_dh_is_refused_rather_than_answered_with_success() {
        let mut ci = interpreter();

        let answer = ci.process_command(Command::ConfigDH(ConfigDHCommand::new(7, DHId(0))));

        match answer {
            Telemetry::ConfigDH(tm) => {
                assert!(
                    !tm.header.status.is_success(),
                    "an unimplemented command must not report success"
                );
                assert_eq!(tm.header.status, CommandStatus::InvalidCommand);
                // The answer still belongs to the command that asked.
                assert_eq!(tm.header.sequence, 7);
            }
            other => panic!("expected CONFIG_DH telemetry, got {other:?}"),
        }
    }

    /// A command that is implemented still answers success, so the test above
    /// is about CONFIG_DH and not about every command being refused.
    #[test]
    fn an_implemented_command_still_succeeds() {
        let mut ci = interpreter();

        match ci.process_command(Command::Ping(tcslibgs::PingCommand::new(1))) {
            Telemetry::Ping(tm) => assert!(tm.header.status.is_success()),
            other => panic!("expected PING telemetry, got {other:?}"),
        }
    }
}
