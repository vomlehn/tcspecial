//! Command Interpreter implementation for TCSpecial
//!
//! The CI processes commands from the OC and manages data handlers.

use std::collections::BTreeMap;
use log::{debug, error, info, trace};
use crate::beacon_send::BeaconSend;
use std::net::UdpSocket;
//use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};
//use std::thread;
use std::time::{Duration, Instant};
use tcslibgs::{
    ArmKey, BeaconTime, CIConfig, Command, CommandStatus, ConfigTelemetry,
    ConfigDigest, ConfigVersion, ConnectTelemetry,
    DHConfig, DHId, DHName, DHSample, EndpointConfig, PingTelemetry, QueryDHSampleTelemetry,
    QueryDHTelemetry, RestartArmTelemetry, RestartTelemetry,
    StartDHTelemetry, Statistics, StopDHTelemetry, TcsError, TcsResult, Telemetry,
};

use crate::config::constants::RESTART_ARM_TIMEOUT;
use crate::dh::{DHState, DataHandler};
use crate::endpoint::bind_endpoint_pair;
use crate::telemetry_log::TelemetryLog;

/// Command interpreter state
pub struct CommandInterpreter {
    /// The beacon sender, once the main loop has started it. Held so that a
    /// Config command can retime it.
    beacon: Option<BeaconSend>,
    beacon_interval: BeaconTime,
    /// Where beacons go, as the configuration said: the multicast group and
    /// the interface to send it on.
    beacon_address: std::net::SocketAddr,
    beacon_interface: std::net::Ipv4Addr,
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
    /// What this build is and what it read, answered to a CONNECT.
    ///
    /// Settled at startup rather than when a CONNECT arrives: the digest is of
    /// the file this process read, which is the file it read *then*, and a
    /// file edited since would otherwise have the spacecraft claiming a
    /// configuration it is not serving.
    version: ConfigVersion,
    /// The version the payload configuration file states, which every beacon
    /// carries beside this build's. The file's own, versioned by whoever
    /// writes the payload set.
    config_version: ConfigVersion,
    digest: ConfigDigest,
}

/// Say what could not be bound, and what that usually means.
///
/// A bare io::Error carries the kind and nothing else, so the whole of what a
/// reader got was "Address already in use (os error 98)" -- not which address,
/// and no hint that the usual cause is a second copy of the program. Both ends
/// of a data handler bind an address too, so this is used for all of them.
pub fn bind_failed(what: &str, addr: &str, e: std::io::Error) -> TcsError {
    if e.kind() == std::io::ErrorKind::AddrInUse {
        TcsError::Config(format!(
            "cannot listen on {addr} for the {what}: address already in use. \
             Something else holds it -- most often another tcspecial, or a \
             tcsmoc that starts one of its own"
        ))
    } else {
        TcsError::Config(format!("cannot listen on {addr} for the {what}: {e}"))
    }
}

/// Why a START_DH found no handler, in the words of the command.
///
/// Said with the name the command carries rather than with anything read
/// here: the whole of a NotFound is that this process has no entry to take a
/// name from, and the name in the command is the one on the button the
/// operator pressed.
///
/// What it usually means is that the two ends are reading different payload
/// configurations. A tcsmoc attaches to a tcspecial that is already listening
/// rather than starting a second one, so a tcspecial left running from an
/// earlier payload set serves that set's handlers while the panels in front of
/// the operator are the new set's -- and the only sign of it was a status with
/// no explanation. So this says what this process does serve, which is the
/// fact that identifies the stale end.
fn no_such_handler(name: &DHName, dh_id: DHId, configs: &[DHConfig]) -> String {
    let served = if configs.is_empty() {
        "no handlers at all".to_string()
    } else {
        configs
            .iter()
            .map(|c| format!("{} ({})", c.dh_id.0, c.name.0))
            .collect::<Vec<_>>()
            .join(", ")
    };

    format!(
        "{}: START_DH names dh_id {}, which this tcspecial does not have; it \
         serves {}. The ground and this process are reading different payload \
         configurations -- most often a tcspecial left running from an earlier \
         payload set, which a tcsmoc attaches to rather than replacing",
        name.0, dh_id.0, served
    )
}

/// Start a data handler moving data.
///
/// The handler already exists -- initialize_handlers made one for every entry
/// in the payload file -- so this opens its OC endpoints and sets it running.
///
/// The OC endpoints are opened here because this is where the OC's address is
/// known, from `oc` in the handler's configuration. It is opened once and both
/// halves come from that one open, for the reason the payload endpoint is: a
/// UDP address cannot be bound twice.
///
/// A handler with no OC address cannot be started. It has nowhere to send what
/// it reads from its payload and nowhere to read what it should write there,
/// so this says so rather than starting a handler that moves nothing.
fn start_handler(dh: &mut DataHandler, config: &DHConfig) -> TcsResult<()> {
    let oc = config.oc.clone().ok_or_else(|| {
        TcsError::Config(format!(
            "data handler \"{}\" has no OC address: give it oc_address and oc_port",
            config.name.0
        ))
    })?;
    let (oc_reader, oc_writer) =
        bind_endpoint_pair(&config.name.0, &EndpointConfig::Network(oc.clone()))?;
    dh.start(oc_reader, oc_writer)?;

    // Said on the way out, not only on the way wrong. Until this was here
    // tcspecial logged a handler that failed to start and nothing at all about
    // one that started, so a session where START_DH answered Success and no
    // data moved left no record of what the handler had been pointed at.
    info!(
        "{} started: payload {}, OC {}:{}",
        config.name.0,
        endpoint_description(&config.endpoint),
        oc.address,
        oc.port
    );

    Ok(())
}

/// What a handler's payload endpoint is, in one line of log.
///
/// Each kind says what locates it, and the terms that cannot be read off the
/// name: a log line that said only "device /dev/ttyS0" for a serial line left
/// the rate it was opened at nowhere to be found.
fn endpoint_description(endpoint: &EndpointConfig) -> String {
    match endpoint {
        EndpointConfig::Network(net) => {
            format!("{:?} {}:{}", net.protocol, net.address, net.port)
        }
        EndpointConfig::Device(dev) => format!("device {}", dev.path),
        EndpointConfig::Serial(serial) => format!(
            "serial {} at {} baud, {} data bits, {}",
            serial.path,
            serial.datarate,
            serial.byte_length,
            match serial.stop_bits {
                Some(stop_bits) => format!("{stop_bits} stop"),
                None => "synchronous".to_string(),
            }
        ),
        EndpointConfig::I2c(i2c) => format!(
            "I2C device {:#04X} on bus {}{}",
            i2c.address,
            i2c.bus,
            if i2c.pec { ", with PEC" } else { "" }
        ),
        EndpointConfig::Spi(spi) => format!(
            "SPI {} in {} at up to {} Hz, {} bits per word",
            spi.path, spi.mode, spi.max_speed, spi.bits_per_word
        ),
    }
}

impl CommandInterpreter {
    /// Create a new command interpreter
    pub fn new(
        config: CIConfig,
        payload_config: Vec<DHConfig>,
        config_version: ConfigVersion,
        digest: ConfigDigest,
    ) -> TcsResult<Self> {
        let addr = format!("{}:{}", config.address, config.port);
        let socket = UdpSocket::bind(&addr).map_err(|e| bind_failed("command interpreter", &addr, e))?;
        socket.set_nonblocking(false)?;

        // Opened here, before the main loop: see TelemetryLog::open.
        let telemetry_log = TelemetryLog::open(&config)?;

        Ok(Self {
            beacon_interval: config.beacon_interval,
            beacon_address: config.beacon_address,
            beacon_interface: config.beacon_interface,
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
            version: ConfigVersion::of_this_build(),
            config_version,
            digest,
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
            Command::Connect(cmd) => {
                // Said rather than judged. The ground is the end that knows
                // what it put on the screen, so it compares; this end answers
                // with what it read and logs the disagreement it can see.
                if cmd.version != self.version || cmd.digest != self.digest {
                    error!(
                        "CONNECT from a ground reading {} {} -- this tcspecial has {} {}",
                        cmd.version, cmd.digest, self.version, self.digest
                    );
                }
                Telemetry::Connect(ConnectTelemetry::new(
                    cmd.header.sequence,
                    self.version,
                    self.config_version,
                    self.digest,
                ))
            }
            Command::StartDH(cmd) => {
                let status = {
                    let mut handlers = match self.data_handlers.lock() {
                        Ok(h) => h,
                        Err(_) => return Telemetry::StartDH(StartDHTelemetry::new(cmd.header.sequence, CommandStatus::Failure)),
                    };

                    // A handler in the map always came from the
                    // configuration, so a missing config cannot happen; it is
                    // a status rather than a panic because a command
                    // interpreter should not die of a surprise.
                    let config = self
                        .payload_config
                        .iter()
                        .find(|c| c.dh_id == cmd.dh_id);

                    match (handlers.get_mut(&cmd.dh_id), config) {
                        (None, _) | (_, None) => {
                            error!("{}", no_such_handler(&cmd.name, cmd.dh_id, &self.payload_config));
                            CommandStatus::NotFound
                        }

                        // The kind the command names has to be the kind the
                        // handler is. Both ends read the same configuration
                        // file, so a mismatch is not a handler that needs
                        // starting differently -- it is the two ends
                        // disagreeing about what that file says, or a command
                        // made by hand. Starting it anyway would open the
                        // endpoint the configuration describes while the
                        // ground believed it had started something else.
                        //
                        // Checked before the state, so that a mis-named
                        // START_DH is refused whether or not the handler
                        // happens to be running: what is wrong with it is
                        // wrong either way.
                        (Some(_), Some(config)) if cmd.dh_type != config.endpoint.kind() => {
                            error!(
                                "{}: START_DH asks for a {:?} handler, but it is {:?}",
                                config.name.0,
                                cmd.dh_type,
                                config.endpoint.kind()
                            );
                            CommandStatus::InvalidParameter
                        }

                        // Existing is not the same as started.
                        // initialize_handlers puts a handler in this map for
                        // every one the configuration describes, all of them
                        // Created and none of them running, so a check for
                        // existence answered Success to every START_DH and
                        // started nothing. What makes this idempotent is the
                        // state, not the presence.
                        (Some(dh), Some(_)) if dh.state() == DHState::Active => {
                            // Genuinely already started.
                            CommandStatus::Success
                        }

                        (Some(dh), Some(config)) => match start_handler(dh, config) {
                            Ok(()) => CommandStatus::Success,
                            Err(e) => {
                                error!("{}: cannot start: {}", config.name.0, e);
                                CommandStatus::Failure
                            }
                        },
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
                            Ok(_) => {
                                info!("{} stopped", dh.name().0);
                                CommandStatus::Success
                            }
                            Err(e) => {
                                error!("{} cannot stop: {}", dh.name().0, e);
                                CommandStatus::Failure
                            }
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

        debug!(
            "beacon group {} on interface {}",
            self.beacon_address, self.beacon_interface
        );
        // Beacons go out at the configured interval, and the sender is kept
        // so that a Config command can retime it. It records into the same
        // log as the responses sent below, so the log holds everything that
        // went to the ground.
        self.beacon = BeaconSend::new(
            Duration::from_millis(self.beacon_interval.0 as u64),
            self.beacon_address,
            self.beacon_interface,
            self.telemetry_log.clone(),
            self.version,
            self.config_version,
            self.digest,
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

    /// The version a payload set states, where which version is not what is
    /// being tested.
    fn a_config_version() -> ConfigVersion {
        ConfigVersion::of_text("1.0").expect("a version")
    }

    /// A digest for an interpreter whose digest is not what is being tested.
    fn a_digest() -> ConfigDigest {
        ConfigDigest([0; 16])
    }

    /// A group to send beacons to that nothing has joined.
    ///
    /// Port 0 rather than the real one: these interpreters run in a test
    /// process, several at once, and a beacon sent to the port a real tcsmoc
    /// listens on would reach one that happened to be running.
    fn a_beacon_address() -> std::net::SocketAddr {
        "239.255.0.1:0".parse().expect("an address")
    }

    #[test]
    fn test_ci_creation() {
        let config = CIConfig {
            address: "127.0.0.1".to_string(),
            port: 0, // Let OS assign port
            protocol: NetworkProtocol::Udp,
            beacon_interval: BeaconTime(5000),
            beacon_address: a_beacon_address(),
            beacon_interface: std::net::Ipv4Addr::LOCALHOST,
            log_dir: None,
            log_segment_bytes: 65_536,
        };

        let ci = CommandInterpreter::new(config, vec![], a_config_version(), a_digest());
        assert!(ci.is_ok());
    }

    /// A CONNECT is answered with what this end read.
    ///
    /// The ground compares; this end says. What it says has to be its own
    /// version and its own digest even when the ground's differ, because a
    /// spacecraft that echoed the ground's would turn the one check there is
    /// into a check of nothing.
    #[test]
    fn a_connect_is_answered_with_what_this_end_read() {
        use tcslibgs::ConnectCommand;

        let mine = ConfigDigest([0xAB; 16]);
        let mut ci = CommandInterpreter::new(
            CIConfig {
                address: "127.0.0.1".to_string(),
                port: 0,
                protocol: NetworkProtocol::Udp,
                beacon_interval: BeaconTime(5000),
                beacon_address: a_beacon_address(),
                beacon_interface: std::net::Ipv4Addr::LOCALHOST,
                log_dir: None,
                log_segment_bytes: 65_536,
            },
            vec![],
            a_config_version(),
            mine,
        )
        .expect("an interpreter");

        // A ground reading something else entirely.
        let theirs = ConfigDigest([0x11; 16]);
        let older = ConfigVersion {
            major: 0,
            minor: 0,
            patch: 1,
        };

        match ci.process_command(Command::Connect(ConnectCommand::new(7, older, theirs))) {
            Telemetry::Connect(tm) => {
                assert_eq!(tm.header.sequence, 7, "the answer is to that command");
                assert_eq!(tm.digest, mine, "it echoed the ground's digest");
                assert_eq!(
                    tm.version,
                    ConfigVersion::of_this_build(),
                    "it echoed the ground's version"
                );
            }
            other => panic!("expected CONNECT telemetry, got {other:?}"),
        }
    }

    /// A START_DH for a handler this process does not have says so, and says
    /// enough to find out why.
    ///
    /// What this was written for: a tcsmoc showing payload set 2's two panels
    /// pressed Transmit on the second and was answered `NotFound` with the
    /// payload's name in it, which reads as a name that was looked up and
    /// missed. Nothing is looked up by name -- the id is what addresses a
    /// handler -- and the real cause was a tcspecial left running from before
    /// that payload set had a second payload, which the panels had attached
    /// to. The status alone could not say that, so the log now does.
    #[test]
    fn a_start_dh_for_a_handler_that_is_not_here_says_what_is_here() {
        use tcslibgs::{DHConfig, DHName, DHType, DeviceConfig, EndpointConfig, StartDHCommand};

        let handler = DHConfig {
            dh_id: DHId(0),
            name: DHName::new("auto-send"),
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/urandom".to_string(),
            }),
            packet_size: 1,
            oc: None,
            mode: Default::default(),
        };

        let mut ci = CommandInterpreter::new(
            CIConfig {
                address: "127.0.0.1".to_string(),
                port: 0,
                protocol: NetworkProtocol::Udp,
                beacon_interval: BeaconTime(5000),
                beacon_address: a_beacon_address(),
                beacon_interface: std::net::Ipv4Addr::LOCALHOST,
                log_dir: None,
                log_segment_bytes: 65_536,
            },
            vec![handler],
            a_config_version(),
            a_digest(),
        )
        .expect("an interpreter");
        ci.initialize_handlers().expect("handlers are made at startup");

        // The one this process has not got, named as the ground names it.
        match ci.process_command(Command::StartDH(StartDHCommand::new(
            1,
            DHId(1),
            DHType::Network,
            DHName::new("triggered-send"),
        ))) {
            Telemetry::StartDH(tm) => {
                assert_eq!(tm.header.status, CommandStatus::NotFound)
            }
            other => panic!("expected START_DH telemetry, got {other:?}"),
        }

        // And the message says which payload was asked for, which id it came
        // in as, and what this process does serve -- the three facts that tell
        // an operator the two ends are reading different files.
        let said = no_such_handler(&DHName::new("triggered-send"), DHId(1), &ci.payload_config);
        assert!(said.contains("triggered-send"), "no payload named: {said}");
        assert!(said.contains("dh_id 1"), "no id said: {said}");
        assert!(
            said.contains("0 (auto-send)"),
            "it does not say what is served: {said}"
        );
        assert!(
            said.contains("different payload configurations"),
            "it does not say what that usually means: {said}"
        );
    }

    /// A START_DH that does find its handler starts the asking.
    ///
    /// The other half of the session above: once the two ends were reading the
    /// same payload file, the command the button sends had to do what the
    /// panel says it does. A triggered handler moves nothing until it has
    /// asked, so what Success has to mean here is a trigger on its way to the
    /// payload at the configured interval, sent of the handler's own accord
    /// with the ground having said nothing.
    #[test]
    fn start_dh_on_a_triggered_handler_starts_the_asking() {
        use std::io::Read;
        use std::net::{TcpListener, UdpSocket};
        use std::time::{Duration, Instant};
        use tcslibgs::{
            DHConfig, DHMode, DHName, DHType, EndpointConfig, NetworkConfig, StartDHCommand,
        };

        // TCP, as tcspecial2.yaml's triggered payload is: a handler connects to
        // a stream payload, so it knows where to send before anything has
        // been heard, where a datagram payload would have to speak first --
        // which is the one thing a triggered payload does not do.
        let payload = TcpListener::bind("127.0.0.1:0").expect("the payload listens");
        let payload_addr = payload.local_addr().expect("its address");

        let oc = UdpSocket::bind("127.0.0.1:0").expect("a free OC port");
        let oc_addr = oc.local_addr().expect("its address");
        drop(oc);

        let handler = DHConfig {
            dh_id: DHId(1),
            name: DHName::new("triggered-send"),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Tcp,
                address: payload_addr.ip().to_string(),
                port: payload_addr.port(),
            }),
            packet_size: 12,
            oc: Some(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: oc_addr.ip().to_string(),
                port: oc_addr.port(),
            }),
            mode: DHMode::Triggered {
                trigger: b"go".to_vec(),
                interval_ms: 50,
            },
        };

        let mut ci = CommandInterpreter::new(
            CIConfig {
                address: "127.0.0.1".to_string(),
                port: 0,
                protocol: NetworkProtocol::Udp,
                beacon_interval: BeaconTime(5000),
                beacon_address: a_beacon_address(),
                beacon_interface: std::net::Ipv4Addr::LOCALHOST,
                log_dir: None,
                log_segment_bytes: 65_536,
            },
            vec![handler],
            a_config_version(),
            a_digest(),
        )
        .expect("an interpreter");
        ci.initialize_handlers().expect("handlers are made at startup");

        match ci.process_command(Command::StartDH(StartDHCommand::new(
            1,
            DHId(1),
            DHType::Network,
            DHName::new("triggered-send"),
        ))) {
            Telemetry::StartDH(tm) => assert!(
                tm.header.status.is_success(),
                "START_DH said {:?}",
                tm.header.status
            ),
            other => panic!("expected START_DH telemetry, got {other:?}"),
        }

        let (mut asked, _) = payload.accept().expect("the handler connects");

        // Unprompted, and at an interval: counted over a measured window
        // rather than timed between two reads, for the reason given in
        // dh.rs's a_triggered_handler_asks_its_payload_and_carries_the_answer
        // -- the interval runs from when the handler sent, and a read can
        // return long after that.
        let window = Duration::from_millis(300);
        let expected = window.as_millis() as u64 / 50;
        asked
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();

        let until = Instant::now() + window;
        let mut came = Vec::new();
        while Instant::now() < until {
            let mut buffer = [0u8; 256];
            match asked.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => came.extend_from_slice(&buffer[..n]),
                Err(_) => {}
            }
        }

        assert!(
            !came.is_empty() && came.chunks(2).all(|chunk| chunk == b"go"),
            "what came down the line was not triggers: {came:?}"
        );
        let triggers = came.len() as u64 / 2;
        assert!(
            triggers > 1,
            "{triggers} trigger(s) in {window:?}: it is sent once, not at an interval"
        );
        assert!(
            triggers <= expected * 3,
            "{triggers} triggers in {window:?}, where an interval of 50ms allows \
             about {expected}: there is no interval between them"
        );
    }

    /// With nothing configured at all the message still reads as a sentence.
    #[test]
    fn a_tcspecial_serving_nothing_says_so() {
        use tcslibgs::DHName;

        let said = no_such_handler(&DHName::new("auto-send"), DHId(0), &[]);
        assert!(
            said.contains("no handlers at all"),
            "an empty configuration is not said: {said}"
        );
    }

    /// A START_DH that names the wrong kind is refused, and starts nothing.
    ///
    /// The kind travelled in the command and nothing compared it with the
    /// handler's own, so the ground could ask for a network handler, be
    /// answered Success, and have a device opened instead. Both ends read the
    /// same configuration file, so a mismatch is the two of them disagreeing
    /// about what that file says -- or a command made by hand -- and neither
    /// is a thing to carry out.
    #[test]
    fn a_start_dh_naming_the_wrong_kind_is_refused() {
        use tcslibgs::{DHConfig, DHName, DHType, DeviceConfig, EndpointConfig, StartDHCommand};

        let handler = DHConfig {
            dh_id: DHId(2),
            name: DHName::new("DH2"),
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/urandom".to_string(),
            }),
            packet_size: 1,
            // No OC address: a handler that were started would fail for want
            // of one, and nothing here should get that far.
            oc: None,
                    mode: Default::default(),
        };

        let mut ci = CommandInterpreter::new(
            CIConfig {
                address: "127.0.0.1".to_string(),
                port: 0,
                protocol: NetworkProtocol::Udp,
                beacon_interval: BeaconTime(5000),
                beacon_address: a_beacon_address(),
                beacon_interface: std::net::Ipv4Addr::LOCALHOST,
                log_dir: None,
                log_segment_bytes: 65_536,
            },
            vec![handler],
            a_config_version(),
            a_digest(),
        )
        .expect("an interpreter");
        ci.initialize_handlers().expect("handlers are made at startup");

        // Every kind but the one it is.
        for wrong in [DHType::Network, DHType::Serial, DHType::I2c, DHType::Spi] {
            match ci.process_command(Command::StartDH(StartDHCommand::new(
                1,
                DHId(2),
                wrong,
                DHName::new("DH2"),
            ))) {
                Telemetry::StartDH(tm) => assert_eq!(
                    tm.header.status,
                    CommandStatus::InvalidParameter,
                    "START_DH as {wrong:?} said {:?}",
                    tm.header.status
                ),
                other => panic!("expected START_DH telemetry, got {other:?}"),
            }
        }

        // And it started nothing: a refused command leaves the handler as it
        // was, not half way into being something else.
        let handlers = ci.data_handlers.lock().expect("the handler map");
        assert_eq!(
            handlers
                .get(&DHId(2))
                .expect("the handler is still there")
                .state(),
            DHState::Created
        );
    }

    /// StartDH actually starts a handler that then moves data.
    ///
    /// Written because START_DH answered Success in a running tcsmoc session
    /// while the handler's OC port was never bound and every counter stayed
    /// at zero.
    #[test]
    fn start_dh_starts_a_handler_that_moves_data() {
        use std::net::UdpSocket;
        use std::time::Duration;
        use tcslibgs::{
            DHConfig, DHName, DeviceConfig, EndpointConfig, NetworkConfig, StartDHCommand,
        };

        let oc = UdpSocket::bind("127.0.0.1:0").unwrap();
        let oc_addr = oc.local_addr().unwrap();
        drop(oc);

        let handler = DHConfig {
            dh_id: DHId(2),
            name: DHName::new("DH2"),
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/urandom".to_string(),
            }),
            packet_size: 1,
            oc: Some(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: oc_addr.ip().to_string(),
                port: oc_addr.port(),
            }),
                    mode: Default::default(),
        };

        let mut ci = CommandInterpreter::new(
            CIConfig {
                address: "127.0.0.1".to_string(),
                port: 0,
                protocol: NetworkProtocol::Udp,
                beacon_interval: BeaconTime(5000),
                beacon_address: a_beacon_address(),
                beacon_interface: std::net::Ipv4Addr::LOCALHOST,
                log_dir: None,
                log_segment_bytes: 65_536,
            },
            vec![handler],
            a_config_version(),
            a_digest(),
        )
        .expect("an interpreter");

        // As main does, and as the test did not: this is what puts a handler
        // in the map, Created and not running, and what made a check for
        // existence answer Success to every START_DH.
        ci.initialize_handlers().expect("handlers are made at startup");

        match ci.process_command(Command::StartDH(StartDHCommand::new(
            1,
            DHId(2),
            tcslibgs::DHType::Device,
            DHName::new("DH2"),
        ))) {
            Telemetry::StartDH(tm) => {
                assert!(tm.header.status.is_success(), "START_DH said {:?}", tm.header.status)
            }
            other => panic!("expected START_DH telemetry, got {other:?}"),
        }

        // Success has to mean the handler is there and listening. Speak to it
        // and require an answer: that is the whole of what starting it is for.
        let ground = UdpSocket::bind("127.0.0.1:0").unwrap();
        ground
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        ground
            .send_to(b"ground", oc_addr)
            .expect("the handler's OC address should be bound");

        let mut buffer = [0u8; 8192];
        let (n, _) = ground
            .recv_from(&mut buffer)
            .expect("a started handler should send payload data back");
        assert_ne!(n, 0);

        // And still there a moment later: a conduit thread that breaks out of
        // its loop drops the endpoints it owns, which closes the sockets and
        // leaves a handler that answers Success and then does nothing.
        std::thread::sleep(Duration::from_millis(500));
        ground
            .send_to(b"again", oc_addr)
            .expect("the OC address should still be bound");
        let (n, _) = ground
            .recv_from(&mut buffer)
            .expect("the handler should still be moving data half a second later");
        assert_ne!(n, 0);

        let stats = match ci.process_command(Command::QueryDH(tcslibgs::QueryDHCommand::new(
            2,
            DHId(2),
        ))) {
            Telemetry::QueryDH(tm) => tm.statistics,
            other => panic!("expected QUERY_DH telemetry, got {other:?}"),
        };
        assert_ne!(
            stats.bytes_received, 0,
            "a handler that has been spoken to should report receiving something"
        );
    }

    /// Starting a handler twice is harmless, and the second time starts
    /// nothing.
    ///
    /// This is what the old existence check was reaching for, and it belongs
    /// on the state: a handler already Active is already started, where one
    /// merely present is not.
    #[test]
    fn starting_a_running_handler_again_is_harmless() {
        use std::net::UdpSocket;
        use tcslibgs::{
            DHConfig, DHName, DeviceConfig, EndpointConfig, NetworkConfig, StartDHCommand,
        };

        let probe = UdpSocket::bind("127.0.0.1:0").unwrap();
        let oc_addr = probe.local_addr().unwrap();
        drop(probe);

        let handler = DHConfig {
            dh_id: DHId(2),
            name: DHName::new("DH2"),
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/urandom".to_string(),
            }),
            packet_size: 1,
            oc: Some(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: oc_addr.ip().to_string(),
                port: oc_addr.port(),
            }),
                    mode: Default::default(),
        };

        let mut ci = CommandInterpreter::new(
            CIConfig {
                address: "127.0.0.1".to_string(),
                port: 0,
                protocol: NetworkProtocol::Udp,
                beacon_interval: BeaconTime(5000),
                beacon_address: a_beacon_address(),
                beacon_interface: std::net::Ipv4Addr::LOCALHOST,
                log_dir: None,
                log_segment_bytes: 65_536,
            },
            vec![handler],
            a_config_version(),
            a_digest(),
        )
        .expect("an interpreter");
        ci.initialize_handlers().unwrap();

        for attempt in 1..=2 {
            match ci.process_command(Command::StartDH(StartDHCommand::new(
                attempt,
                DHId(2),
                tcslibgs::DHType::Device,
                DHName::new("DH2"),
            ))) {
                Telemetry::StartDH(tm) => assert!(
                    tm.header.status.is_success(),
                    "attempt {attempt} said {:?}",
                    tm.header.status
                ),
                other => panic!("expected START_DH telemetry, got {other:?}"),
            }
        }

        // The second start must not have opened the OC address a second time,
        // which would have failed: it is still bound by the first.
        let ground = UdpSocket::bind("127.0.0.1:0").unwrap();
        ground
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        ground.send_to(b"ground", oc_addr).unwrap();
        let mut buffer = [0u8; 8192];
        let (n, _) = ground
            .recv_from(&mut buffer)
            .expect("the handler started once is still the one running");
        assert_ne!(n, 0);
    }

    /// A port already in use says so, and says which.
    ///
    /// The bare io::Error behind this said "Address already in use (os error
    /// 98)" and nothing more -- not the address, and no hint that the usual
    /// cause is a second copy of the program running.
    #[test]
    fn an_address_already_in_use_names_itself() {
        use std::net::UdpSocket;

        // Hold a port, then ask for the same one.
        let held = UdpSocket::bind("127.0.0.1:0").expect("a port");
        let addr = held.local_addr().expect("its address");

        let config = CIConfig {
            address: addr.ip().to_string(),
            port: addr.port(),
            protocol: NetworkProtocol::Udp,
            beacon_interval: BeaconTime(5000),
            beacon_address: a_beacon_address(),
            beacon_interface: std::net::Ipv4Addr::LOCALHOST,
            log_dir: None,
            log_segment_bytes: 65_536,
        };

        let message = CommandInterpreter::new(config, vec![], a_config_version(), a_digest())
            .err()
            .expect("binding a held port must fail")
            .to_string();

        assert!(
            message.contains(&addr.to_string()),
            "the error should name the address, but said: {message}"
        );
        assert!(
            message.contains("already in use"),
            "the error should say what is wrong, but said: {message}"
        );
        assert!(
            message.contains("tcspecial"),
            "the error should suggest the usual cause, but said: {message}"
        );
    }

    /// A command interpreter on a port the OS picks, with no payloads.
    fn interpreter() -> CommandInterpreter {
        CommandInterpreter::new(
            CIConfig {
                address: "127.0.0.1".to_string(),
                port: 0,
                protocol: NetworkProtocol::Udp,
                beacon_interval: BeaconTime(5000),
                beacon_address: a_beacon_address(),
                beacon_interface: std::net::Ipv4Addr::LOCALHOST,
                log_dir: None,
                log_segment_bytes: 65_536,
            },
            vec![],
            a_config_version(),
            a_digest(),
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
