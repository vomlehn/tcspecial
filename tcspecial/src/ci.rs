//! Command Interpreter implementation for TCSpecial
//!
//! The CI processes commands from the OC and manages data handlers.

use std::collections::BTreeMap;
use log::{debug, error, info, trace, warn};
use crate::beacon_send::BeaconSend;
use std::net::UdpSocket;
//use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tcslibgs::{
    answer_to, came_from, ArmKey, HostPort, BeaconInterval, CIConfig, Command, CommandLink, CommandStatus, ConfigTelemetry,
    ConfigDigest, ConfigVersion, ConnectTelemetry,
    DHConfig, DHName, DHSample, DHState, EndpointConfig, PingTelemetry, QueryDHSampleTelemetry,
    QueryDHTelemetry, RestartArmTelemetry, RestartTelemetry,
    StartDHTelemetry, Statistics, StopDHTelemetry, TcsError, TcsResult, Telemetry,
};

use crate::config::constants::RESTART_ARM_TIMEOUT;
use crate::dh::DataHandler;
use crate::endpoint::bind_oc_pair;
use crate::telemetry_log::TelemetryLog;

/// What serves the payload command link.
///
/// Everything a command about a payload needs and nothing a command about the
/// spacecraft does: the handlers, the payloads as the file described them,
/// and the telemetry log. No command here touches the arming, the beacon or
/// what this end read -- a CONNECT is the spacecraft's link's -- which is what
/// lets this run in a thread of its own, and the reason for the split is that
/// it must, so that a payload command that takes its time cannot delay a
/// RESTART.
///
/// Clonable, and every clone is the same interpreter: the handler map and the
/// log are shared handles, and the rest is read-only.
#[derive(Clone)]
pub struct PayloadCommands {
    data_handlers: Arc<Mutex<BTreeMap<DHName, DataHandler>>>,
    payload_config: Arc<Vec<DHConfig>>,
    telemetry_log: TelemetryLog,
}

/// How often the payload link looks up from its socket to see whether the
/// interpreter has stopped.
///
/// A blocking read would serve commands just as well and would outlive the
/// shutdown that is waiting for it, so the socket has a timeout and the loop
/// asks. Short enough that a shutdown is not visibly delayed; long enough
/// that an idle link is not a program spinning.
const PAYLOAD_LINK_POLL: Duration = Duration::from_millis(200);

impl PayloadCommands {
    /// Answer a command that belongs on this link.
    pub fn process(&self, command: Command) -> Telemetry {
        trace!("payload link: {:?}", command);
        match command {
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
                        .find(|c| c.name == cmd.name);

                    match (handlers.get_mut(&cmd.name), config) {
                        (None, _) | (_, None) => {
                            error!("{}", no_such_handler(&cmd.name, &self.payload_config));
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

                    if let Some(dh) = handlers.get_mut(&cmd.name) {
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
                let (status, state, stats) = {
                    let handlers = match self.data_handlers.lock() {
                        Ok(h) => h,
                        Err(_) => return Telemetry::QueryDH(QueryDHTelemetry::new(
                            cmd.header.sequence,
                            CommandStatus::Failure,
                            cmd.name.clone(),
                            None,
                            Statistics::new(),
                        )),
                    };

                    // The state as well as the counters. A handler that
                    // stopped on the spacecraft -- or one the ground never
                    // started, this process having been running before the
                    // MOC attached to it -- is a panel saying what the last
                    // command implied, and nothing else on the link says
                    // otherwise.
                    if let Some(dh) = handlers.get(&cmd.name) {
                        (CommandStatus::Success, Some(dh.state()), dh.statistics())
                    } else {
                        (CommandStatus::NotFound, None, Statistics::new())
                    }
                };
                Telemetry::QueryDH(QueryDHTelemetry::new(
                    cmd.header.sequence,
                    status,
                    cmd.name.clone(),
                    state,
                    stats,
                ))
            }
            Command::QueryDHSample(cmd) => {
                let (status, samples) = {
                    let handlers = match self.data_handlers.lock() {
                        Ok(h) => h,
                        Err(_) => {
                            return Telemetry::QueryDHSample(QueryDHSampleTelemetry::new(
                                cmd.header.sequence,
                                CommandStatus::Failure,
                                cmd.name.clone(),
                                DHSample::new(),
                                DHSample::new(),
                            ))
                        }
                    };

                    match handlers.get(&cmd.name) {
                        Some(dh) => (CommandStatus::Success, dh.samples()),
                        None => (CommandStatus::NotFound, Default::default()),
                    }
                };
                Telemetry::QueryDHSample(QueryDHSampleTelemetry::new(
                    cmd.header.sequence,
                    status,
                    cmd.name.clone(),
                    samples.sent,
                    samples.received,
                ))
            }
            Command::ConfigDH(cmd) => {
                // Refused rather than answered Success. Nothing here
                // configures a data handler, and the command carries nothing
                // to configure it with: ConfigDHCommand has a name and a
                // note that settings could be added to it. Answering Success
                // told the ground a handler had been reconfigured when
                // nothing had happened, which is worse than saying so,
                // because there is no way to tell it from the real thing.
                //
                // InvalidCommand rather than Failure: the handler did not
                // fail to be configured, the command is one tcspecial does
                // not implement.
                error!(
                    "CONFIG_DH for {} refused: tcspecial does not implement it",
                    cmd.name.0
                );
                Telemetry::ConfigDH(tcslibgs::ConfigDHTelemetry::new(
                    cmd.header.sequence,
                    CommandStatus::InvalidCommand,
                ))
            }
            // The spacecraft's own commands. Refused rather than served, for
            // the reason a payload command is refused on the other link: a
            // command on the wrong one is one end not reading the
            // configuration the other is.
            theirs @ (Command::Ping(_)
            | Command::Connect(_)
            | Command::RestartArm(_)
            | Command::Restart(_)
            | Command::Config(_)) => refused_on_the_wrong_link(&theirs, CommandLink::Payload),
            // A version 2 node command, which this vehicle is not flying. It
            // belongs on this link -- node commands are the payload link's --
            // so it is not a wrong-link refusal: it is a command for a graph,
            // sent to a process running a payload set, and the ground is
            // reading a configuration this end has never seen. Refused as
            // unimplemented, which is what it is here.
            theirs @ (Command::StartNode(_)
            | Command::StopNode(_)
            | Command::SetNode(_)
            | Command::QueryNode(_)
            | Command::QueryNodeSample(_)) => {
                answer_to(&theirs, CommandStatus::InvalidCommand)
            }
        }
    }

    /// Take one payload's commands on `socket` until the interpreter stops.
    ///
    /// A loop of its own, which is the whole point of these links: a command
    /// that takes time -- START_DH opens the payload's endpoint and the
    /// handler's pair of OC sockets -- takes it here, where nothing the ground
    /// needs in a hurry is waiting behind it, and nothing about another
    /// payload either.
    ///
    /// `from` is where commands about this payload are supposed to come from,
    /// and anything else arriving here is ignored: see `tcslibgs::came_from`.
    /// `name` and `id` are the payload this socket is for, so that a command
    /// about another payload can be refused rather than served -- a command on
    /// the wrong socket is one end not reading the configuration the other is,
    /// which is the fault these links must not hide.
    ///
    /// Answers go back to the address the command came from, as the other
    /// link's do: a datagram's sender is the only statement of where the
    /// ground is.
    pub fn serve(
        &self,
        socket: UdpSocket,
        from: &HostPort,
        name: &DHName,
        id: usize,
        serving: Arc<AtomicBool>,
    ) -> TcsResult<()> {
        socket.set_read_timeout(Some(PAYLOAD_LINK_POLL))?;
        let mut buffer = vec![0u8; 65535];

        while serving.load(Ordering::Relaxed) {
            match socket.recv_from(&mut buffer) {
                // Only from where the configuration says the MOC commands
                // this payload from. Nothing is sent back, for the reason the
                // spacecraft link sends nothing back: an answer would tell
                // whoever sent it that something is listening here.
                Ok((_, addr)) if !came_from(from, &addr) => {
                    warn!(
                        "a command for payload {} came from {addr} and not from \
                         {from}, which is where the configuration says the MOC \
                         sends it from -- ignored",
                        name.0
                    );
                }
                Ok((size, addr)) => {
                    trace!("payload {} link: received from {:?}", name.0, addr);
                    match serde_json::from_slice::<Command>(&buffer[..size]) {
                        Ok(command) => {
                            let response = match about_another_payload(&command, name) {
                                Some(theirs) => {
                                    error!(
                                        "{:?} for payload {theirs} arrived on the link \
                                         of payload {} ({id}): the two ends are not \
                                         reading the same configuration",
                                        command.cmd_type(),
                                        name.0
                                    );
                                    answer_to(&command, CommandStatus::InvalidCommand)
                                }
                                None => self.process(command),
                            };
                            if let Ok(data) = serde_json::to_vec(&response) {
                                // Recorded before it is sent, as the other
                                // link's answers are, so that one log holds
                                // everything that went to the ground.
                                self.telemetry_log.record(&data);
                                let _ = socket.send_to(&data, addr);
                            }
                        }
                        // Not a command. Nothing to answer and nobody to
                        // answer it to: the sender is whatever sent this.
                        Err(_) => error!(
                            "payload {} link: {} bytes that are not a command",
                            name.0, size
                        ),
                    }
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut => {}
                // A signal arrived while this was waiting, which is not a
                // failure of the link: a compute node's program exiting is
                // enough to do it. Waited on again rather than returned,
                // because returning here stops the link a payload is
                // commanded over.
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(TcsError::Io(e)),
            }
        }

        Ok(())
    }
}

/// The payload a command is about, when that is not the payload whose link it
/// arrived on.
///
/// `None` where the command is about this payload, or about no payload at all:
/// CONNECT is answered on any link, being about the configuration the far end
/// read rather than about a payload.
fn about_another_payload<'a>(command: &'a Command, mine: &DHName) -> Option<&'a str> {
    match command.payload_name() {
        Some(theirs) if theirs != mine => Some(theirs.0.as_str()),
        _ => None,
    }
}

/// The answer to a command that arrived on the link it does not belong on.
///
/// Refused rather than served. It reached the spacecraft, so something can be
/// said about it, and what has gone wrong is that one end is not reading the
/// configuration the other is -- which serving it would hide, in the one
/// place the split is supposed to be reliable.
///
/// The answer is of the type the command asks for, because that is how the
/// ground pairs an answer with what it asked; see `tcslibgs::answer_to`.
fn refused_on_the_wrong_link(command: &Command, arrived: CommandLink) -> Telemetry {
    error!(
        "{:?} arrived on the {} link and belongs on the {} one: the two ends are \
         not reading the same configuration",
        command.cmd_type(),
        arrived.spelling(),
        command.link().spelling()
    );
    answer_to(command, CommandStatus::InvalidCommand)
}

/// Command interpreter state
pub struct CommandInterpreter {
    /// The beacon sender, once the main loop has started it. Held so that a
    /// Config command can retime it.
    beacon: Option<BeaconSend>,
    beacon_interval: BeaconInterval,
    /// Where beacons come from, as the configuration said: the address and
    /// port this binds to send them.
    beacon_send: tcslibgs::HostPort,
    /// And where they go, which the ground station binds.
    beacon_recv: tcslibgs::HostPort,
    /// Where the MOC's commands come from, as the configuration said:
    /// `tcs_cmd.send`, which is the socket the MOC binds to send them and to
    /// receive the responses on.
    ///
    /// Held so that the loop below can refuse a command from anywhere else. A
    /// command is a command to restart a spacecraft; taking one from whoever
    /// sent it would be taking the ground's word from a stranger.
    moc_cmd_send: tcslibgs::HostPort,
    _config: CIConfig,
    /// The spacecraft command link: ping, arm, restart, configure.
    socket: UdpSocket,
    /// One command link per payload, until [`CommandInterpreter::run`] hands
    /// each to the thread that serves it.
    ///
    /// All bound in `new`, with the spacecraft's, so that a port already in
    /// use is reported where every other configuration error is -- at
    /// startup, naming itself -- rather than when the first command about
    /// that payload arrives.
    payload_links: Vec<PayloadLink>,
    /// What serves those links, and what each thread serving one holds.
    payload: PayloadCommands,
    /// Whether those threads should still be serving. Cleared by `stop` and
    /// `shutdown`, which is how a loop in another thread is told.
    serving: Arc<AtomicBool>,
    data_handlers: Arc<Mutex<BTreeMap<DHName, DataHandler>>>,
    payload_config: Arc<Vec<DHConfig>>,
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
    /// The version the tcspecial configuration file states, which every beacon
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
fn no_such_handler(name: &DHName, configs: &[DHConfig]) -> String {
    let served = if configs.is_empty() {
        "no handlers at all".to_string()
    } else {
        configs
            .iter()
            .map(|c| c.name.0.clone())
            .collect::<Vec<_>>()
            .join(", ")
    };

    format!(
        "START_DH names the handler {}, which this tcspecial does not have; it \
         serves {}. The ground and this process are reading different \
         configurations -- most often a tcspecial left running from an earlier \
         payload set, which a tcsmoc attaches to rather than replacing",
        name.0, served
    )
}

/// Start a data handler moving data.
///
/// The handler already exists -- initialize_handlers made one for every entry
/// in the tcspecial configuration -- so this opens its OC endpoints and sets it
/// running.
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
            "data handler \"{}\" has nowhere to send what it reads: the payload \
             set states no tcspecial section, and a payload's ground link is \
             the section's payload_telemetry.dest_base counted on by the id",
            config.name.0
        ))
    })?;
    let (oc_reader, oc_writer) =
        bind_oc_pair(&config.name.0, &oc, config.ground.as_ref())?;
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
            "serial {} at {} baud, {} data bits, {} stop",
            serial.path, serial.datarate, serial.byte_length, serial.stop_bits
        ),
        // A synchronous line, which is an interface and not a path: the
        // standard and the clock are what a reader compares against the
        // equipment, the rest being what the framing is.
        EndpointConfig::Hdlc(hdlc) => format!(
            "hdlc {} as {}, clock {}{}, {} framing with {}",
            hdlc.interface,
            hdlc.standard,
            hdlc.clock_type,
            match hdlc.clock_rate {
                Some(rate) => format!(" at {rate} bit/s"),
                None => String::new(),
            },
            hdlc.encoding,
            hdlc.frame_check
        ),
        EndpointConfig::Gpio(gpio) => {
            let offsets: Vec<String> = gpio.lines.iter().map(|n| n.to_string()).collect();
            // Which way the lines go, because the offsets do not say: the
            // same three numbers are three temperatures read or three heaters
            // driven, and a log that did not say which is a log that cannot
            // answer why nothing arrived.
            format!(
                "gpio {} lines {} ({})",
                gpio.path,
                offsets.join(","),
                if gpio.direction.is_output() { "driven" } else { "read" }
            )
        }
        EndpointConfig::Rpmsg(rpmsg) => {
            format!("rpmsg {} ({})", rpmsg.path, rpmsg.service)
        }
        // A path and nothing else: what is in the buffer is settled through
        // sysfs by whoever brought the sensor up, so a payload says nothing
        // about it.
        EndpointConfig::Iio(iio) => format!("iio {}", iio.path),
        EndpointConfig::Can(can) => format!(
            "can {} id {:#x}, {}",
            can.interface,
            can.can_id,
            if can.fd { "CAN FD" } else { "classic" }
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

/// One payload's command link, as the interpreter holds it.
struct PayloadLink {
    /// The socket commands about this payload arrive on:
    /// `payload_command.dest_base` counted on by its id. `None` once the
    /// thread serving it has taken it.
    socket: Option<UdpSocket>,
    /// Where they are supposed to come from:
    /// `payload_command.source_base` counted on by the same id, which is what
    /// the MOC binds to send them. A command from anywhere else is ignored --
    /// see [`PayloadCommands::serve`].
    from: HostPort,
    /// Which payload this link is for, by name and by id. The id is the
    /// payload's position in the payload configuration, and the name is what
    /// a command about it carries.
    name: DHName,
    id: usize,
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

        // One command socket per payload, all bound here, before any loop
        // runs: a port already in use is then reported at startup, naming
        // itself, rather than when the first command about that payload
        // arrives.
        //
        // Separate links because what the ground needs in a hurry must never
        // be queued behind what it asked for at leisure -- see
        // CommandType::link -- and one per payload because a command about a
        // slow payload has nothing to do with a command about a quick one.
        // Each socket's own address and the one the ground sends from are the
        // two bases counted on by the payload's own id, which is its position
        // in the payload configuration.
        let mut payload_links = Vec::with_capacity(payload_config.len());
        for (id, dh) in payload_config.iter().enumerate() {
            let at = config
                .payload_command_dest_base
                .at(id)
                .map_err(TcsError::Config)?;
            let from = config
                .payload_command_source_base
                .at(id)
                .map_err(TcsError::Config)?;
            let addr = format!("{}:{}", at.address, at.port);
            let socket = UdpSocket::bind(&addr).map_err(|e| {
                bind_failed(&format!("command link of payload {}", dh.name.0), &addr, e)
            })?;
            payload_links.push(PayloadLink {
                socket: Some(socket),
                from: HostPort {
                    address: from.address,
                    port: from.port,
                },
                name: dh.name.clone(),
                id,
            });
        }

        // Opened here, before the main loop: see TelemetryLog::open.
        let telemetry_log = TelemetryLog::open(&config)?;

        let payload_config = Arc::new(payload_config);
        let data_handlers = Arc::new(Mutex::new(BTreeMap::new()));
        let version = ConfigVersion::of_this_build();
        let payload = PayloadCommands {
            data_handlers: data_handlers.clone(),
            payload_config: payload_config.clone(),
            telemetry_log: telemetry_log.clone(),
        };

        Ok(Self {
            beacon_interval: BeaconInterval::of(config.beacon_interval),
            beacon_send: config.beacon_send.clone(),
            beacon_recv: config.beacon_recv.clone(),
            beacon: None,
            moc_cmd_send: config.moc_cmd_send.clone(),
            _config: config,
            socket,
            payload_links,
            payload,
            serving: Arc::new(AtomicBool::new(true)),
            data_handlers,
            payload_config,
            arm_key: None,
            arm_time: None,
            running: false,
            _global_stats: Statistics::new(),
            telemetry_log,
            version,
            config_version,
            digest,
        })
    }

    /// Where spacecraft commands are taken, as the socket was bound.
    ///
    /// Not what the configuration asked for: a configuration may ask for port
    /// 0, which is the operating system's invitation to choose, and then the
    /// only statement of which port that is is the socket itself.
    pub fn command_address(&self) -> TcsResult<std::net::SocketAddr> {
        Ok(self.socket.local_addr()?)
    }

    /// Where one payload's commands are taken, as that socket was bound.
    ///
    /// By the payload's id, which is its position in the payload
    /// configuration. Not what the configuration asked for, for the reason
    /// [`CommandInterpreter::command_address`] is not: a port of 0 is the
    /// system's to choose, and then the socket is the only statement of it.
    pub fn payload_command_address(&self, id: usize) -> TcsResult<std::net::SocketAddr> {
        match self.payload_links.get(id) {
            Some(link) => match &link.socket {
                Some(socket) => Ok(socket.local_addr()?),
                // Taken by the thread serving it, which is the only other
                // place it can be: ask it there.
                None => Err(TcsError::Config(format!(
                    "the command link of payload {id} is already being served"
                ))),
            },
            None => Err(TcsError::Config(format!(
                "there is no payload {id} in this configuration"
            ))),
        }
    }

    /// Initialize data handlers from configuration
    pub fn initialize_handlers(&mut self) -> TcsResult<()> {
        let mut handlers = self.data_handlers.lock()
            .map_err(|_| TcsError::DataHandler("Lock poisoned".to_string()))?;

        for config in self.payload_config.iter() {
            let dh = DataHandler::new(config.clone())?;
            handlers.insert(config.name.clone(), dh);
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
            // What this end read, asked as the link comes up. On this link
            // and no other: a CONNECT is about having a connection to the
            // vehicle, and a ground station with none has nothing to ask a
            // payload. See CommandType::link.
            //
            // Said rather than judged. The ground is the end that knows what
            // it put on the screen, so it compares; this end answers with
            // what it read and logs the disagreement it can see.
            Command::Connect(cmd) => {
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
            Command::Config(cmd) => {
                self.beacon_interval = cmd.beacon_interval;
                // Retime the running sender too, or the new interval would
                // be recorded and never take effect. set_interval also
                // expires the current wait, so the next beacon goes out at
                // once rather than after the old interval elapses.
                if let Some(beacon) = self.beacon.as_mut() {
                    beacon.set_interval(cmd.beacon_interval.duration());
                }
                Telemetry::Config(ConfigTelemetry::new(cmd.header.sequence, CommandStatus::Success))
            }
            // The payload link's commands. Refused rather than served: a
            // command that arrived here is one end not reading the
            // configuration the other is, and serving it would hide that in
            // the one place the split is supposed to be reliable. See
            // CommandType::link.
            theirs @ (Command::StartDH(_)
            | Command::StopDH(_)
            | Command::QueryDH(_)
            | Command::QueryDHSample(_)
            | Command::ConfigDH(_)
            | Command::StartNode(_)
            | Command::StopNode(_)
            | Command::SetNode(_)
            | Command::QueryNode(_)
            | Command::QueryNodeSample(_)) => {
                refused_on_the_wrong_link(&theirs, CommandLink::Spacecraft)
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
            "beacons from {} to {}",
            self.beacon_send, self.beacon_recv
        );
        // Beacons go out at the configured interval, and the sender is kept
        // so that a Config command can retime it. It records into the same
        // log as the responses sent below, so the log holds everything that
        // went to the ground.
        self.beacon = BeaconSend::new(
            self.beacon_interval.duration(),
            self.beacon_send.clone(),
            self.beacon_recv.clone(),
            self.telemetry_log.clone(),
            self.version,
            self.config_version,
            self.digest,
        );

        // Each payload's command link, in a thread of its own. Its own loop
        // and its own socket, so that a payload command taking its time --
        // opening a device node, binding a handler's sockets -- delays
        // nothing on the link the ground restarts the spacecraft over, and
        // nothing about another payload either.
        for link in self.payload_links.iter_mut() {
            match link.socket.take() {
                Some(socket) => {
                    let payload = self.payload.clone();
                    let serving = self.serving.clone();
                    let from = link.from.clone();
                    let name = link.name.clone();
                    let id = link.id;
                    let where_it_is = socket.local_addr();
                    thread::spawn(move || {
                        if let Err(e) = payload.serve(socket, &from, &name, id, serving) {
                            error!("the command link of payload {} stopped: {}", name.0, e);
                        }
                    });
                    if let Ok(addr) = where_it_is {
                        debug!(
                            "commands for payload {} ({}) are taken on {}, from {}",
                            link.id, link.name.0, addr, link.from
                        );
                    }
                }
                // run called twice. The first call has the link, and nothing
                // about this one should take it from that thread.
                None => error!(
                    "the command link of payload {} is already being served",
                    link.name.0
                ),
            }
        }

        while self.running {
            // Try to receive a command
            match self.socket.recv_from(&mut recv_buffer) {
                // Only from where the configuration says the MOC commands
                // from. Nothing is sent back: an answer would tell whoever
                // sent it that something is listening here, and the one end
                // that is supposed to be talking to this socket is written
                // down in the file both ends read.
                Ok((_, addr)) if !came_from(&self.moc_cmd_send, &addr) => {
                    warn!(
                        "a command came from {addr} and not from {}, which is where \
                         the configuration says the MOC sends from -- ignored",
                        self.moc_cmd_send
                    );
                    continue;
                }
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
                // A signal arrived while this was waiting. Not a failure of
                // the link, and not a reason to stop the one the spacecraft
                // is commanded over: see the payload link's own loop.
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
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
        // And the link served in another thread, which has no way to see the
        // flag above.
        self.serving.store(false, Ordering::Relaxed);
    }

    /// Shut down all data handlers
    pub fn shutdown(&mut self) -> TcsResult<()> {
        self.running = false;
        self.serving.store(false, Ordering::Relaxed);

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
    use tcslibgs::{
        ConfigDHCommand, ConnectCommand, DHType, NetworkProtocol, StartDHCommand,
    };

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
    fn a_beacon_send() -> tcslibgs::HostPort {
        tcslibgs::HostPort {
            address: "127.0.0.1".to_string(),
            port: 0,
        }
    }

    #[test]
    fn test_ci_creation() {
        let config = CIConfig {
            address: "127.0.0.1".to_string(),
            port: 0, // Let OS assign port
            // And each payload's own command link, which a test binds at a
            // port of the system's choosing: a test that named one would fail
            // whenever something else on the machine held it.
            payload_command_source_base: tcslibgs::HostPort {
                address: "127.0.0.1".to_string(),
                port: 0,
            },
            payload_command_dest_base: tcslibgs::HostPort {
                address: "127.0.0.1".to_string(),
                port: 0,
            },
            // The far ends and the bases, which nothing in the
            // commanding reads: see tcslibgs::CIConfig.
            moc_cmd_send: tcslibgs::HostPort {
                address: "127.0.0.1".to_string(),
                port: 4100,
            },
            payload_source_base: tcslibgs::HostPort {
                address: "0.0.0.0".to_string(),
                port: 6000,
            },
            payload_dest_base: tcslibgs::HostPort {
                address: "127.0.0.1".to_string(),
                port: 6100,
            },
            beacon_interval: Duration::from_millis(5000),
            beacon_send: a_beacon_send(),
            beacon_recv: a_beacon_send(),
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
                // And each payload's own command link, which a test binds at a
                // port of the system's choosing: a test that named one would fail
                // whenever something else on the machine held it.
                payload_command_source_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 0,
                },
                payload_command_dest_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 0,
                },
                // The far ends and the bases, which nothing in the
                // commanding reads: see tcslibgs::CIConfig.
                moc_cmd_send: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 4100,
                },
                payload_source_base: tcslibgs::HostPort {
                    address: "0.0.0.0".to_string(),
                    port: 6000,
                },
                payload_dest_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 6100,
                },
                beacon_interval: Duration::from_millis(5000),
                beacon_send: a_beacon_send(),
                beacon_recv: a_beacon_send(),
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
    /// payload's name in it and nothing else: the real cause was a tcspecial
    /// left running from before that payload set had a second payload, which
    /// the panels had attached to. The status alone could not say that, so the
    /// log now says which handlers this process does serve.
    #[test]
    fn a_start_dh_for_a_handler_that_is_not_here_says_what_is_here() {
        use tcslibgs::{DHConfig, DHName, DHType, DeviceConfig, EndpointConfig, StartDHCommand};

        let handler = DHConfig {
            name: DHName::new("auto-send"),
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/urandom".to_string(),
            }),
            packet_size: 1,
            oc: None,
            ground: None,
            mode: Default::default(),
            stale_interval: None,
        };

        let mut ci = CommandInterpreter::new(
            CIConfig {
                address: "127.0.0.1".to_string(),
                port: 0,
                // And each payload's own command link, which a test binds at a
                // port of the system's choosing: a test that named one would fail
                // whenever something else on the machine held it.
                payload_command_source_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 0,
                },
                payload_command_dest_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 0,
                },
                // The far ends and the bases, which nothing in the
                // commanding reads: see tcslibgs::CIConfig.
                moc_cmd_send: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 4100,
                },
                payload_source_base: tcslibgs::HostPort {
                    address: "0.0.0.0".to_string(),
                    port: 6000,
                },
                payload_dest_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 6100,
                },
                beacon_interval: Duration::from_millis(5000),
                beacon_send: a_beacon_send(),
                beacon_recv: a_beacon_send(),
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
        match ci.payload.process(Command::StartDH(StartDHCommand::new(
            1,
            DHType::Network,
            DHName::new("triggered-send"),
        ))) {
            Telemetry::StartDH(tm) => {
                assert_eq!(tm.header.status, CommandStatus::NotFound)
            }
            other => panic!("expected START_DH telemetry, got {other:?}"),
        }

        // And the message says which payload was asked for and what this
        // process does serve -- the two facts that tell an operator the two
        // ends are reading different files.
        let said = no_such_handler(&DHName::new("triggered-send"), &ci.payload_config);
        assert!(
            said.contains("triggered-send"),
            "the handler asked for is not named: {said}"
        );
        assert!(
            said.contains("auto-send"),
            "it does not say what is served: {said}"
        );
        assert!(
            said.contains("different configurations"),
            "it does not say what that usually means: {said}"
        );
    }

    /// A START_DH that does find its handler starts the asking.
    ///
    /// The other half of the session above: once the two ends were reading the
    /// same tcspecial configuration, the command the button sends had to do
    /// what the panel says it does. A triggered handler moves nothing until it
    /// has asked, so what Success has to mean here is a trigger on its way to
    /// the payload at the configured interval, sent of the handler's own accord
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
            ground: None,
            mode: DHMode::Triggered {
                trigger: b"go".to_vec(),
                interval: Duration::from_millis(50),
            },
            stale_interval: None,
        };

        let mut ci = CommandInterpreter::new(
            CIConfig {
                address: "127.0.0.1".to_string(),
                port: 0,
                // And each payload's own command link, which a test binds at a
                // port of the system's choosing: a test that named one would fail
                // whenever something else on the machine held it.
                payload_command_source_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 0,
                },
                payload_command_dest_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 0,
                },
                // The far ends and the bases, which nothing in the
                // commanding reads: see tcslibgs::CIConfig.
                moc_cmd_send: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 4100,
                },
                payload_source_base: tcslibgs::HostPort {
                    address: "0.0.0.0".to_string(),
                    port: 6000,
                },
                payload_dest_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 6100,
                },
                beacon_interval: Duration::from_millis(5000),
                beacon_send: a_beacon_send(),
                beacon_recv: a_beacon_send(),
                log_dir: None,
                log_segment_bytes: 65_536,
            },
            vec![handler],
            a_config_version(),
            a_digest(),
        )
        .expect("an interpreter");
        ci.initialize_handlers().expect("handlers are made at startup");

        match ci.payload.process(Command::StartDH(StartDHCommand::new(
            1,
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
            "{triggers} triggers in {window:?}, where an interval of 50 ms allows \
             about {expected}: there is no interval between them"
        );
    }

    /// With nothing configured at all the message still reads as a sentence.
    #[test]
    fn a_tcspecial_serving_nothing_says_so() {
        use tcslibgs::DHName;

        let said = no_such_handler(&DHName::new("auto-send"), &[]);
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
            name: DHName::new("DH2"),
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/urandom".to_string(),
            }),
            packet_size: 1,
            // No OC address: a handler that were started would fail for want
            // of one, and nothing here should get that far.
            oc: None,
            ground: None,
                    mode: Default::default(),
                    stale_interval: None,
        };

        let mut ci = CommandInterpreter::new(
            CIConfig {
                address: "127.0.0.1".to_string(),
                port: 0,
                // And each payload's own command link, which a test binds at a
                // port of the system's choosing: a test that named one would fail
                // whenever something else on the machine held it.
                payload_command_source_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 0,
                },
                payload_command_dest_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 0,
                },
                // The far ends and the bases, which nothing in the
                // commanding reads: see tcslibgs::CIConfig.
                moc_cmd_send: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 4100,
                },
                payload_source_base: tcslibgs::HostPort {
                    address: "0.0.0.0".to_string(),
                    port: 6000,
                },
                payload_dest_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 6100,
                },
                beacon_interval: Duration::from_millis(5000),
                beacon_send: a_beacon_send(),
                beacon_recv: a_beacon_send(),
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
            match ci.payload.process(Command::StartDH(StartDHCommand::new(
                1,
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
                .get(&DHName::new("DH2"))
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
            ground: None,
                    mode: Default::default(),
                    stale_interval: None,
        };

        let mut ci = CommandInterpreter::new(
            CIConfig {
                address: "127.0.0.1".to_string(),
                port: 0,
                // And each payload's own command link, which a test binds at a
                // port of the system's choosing: a test that named one would fail
                // whenever something else on the machine held it.
                payload_command_source_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 0,
                },
                payload_command_dest_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 0,
                },
                // The far ends and the bases, which nothing in the
                // commanding reads: see tcslibgs::CIConfig.
                moc_cmd_send: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 4100,
                },
                payload_source_base: tcslibgs::HostPort {
                    address: "0.0.0.0".to_string(),
                    port: 6000,
                },
                payload_dest_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 6100,
                },
                beacon_interval: Duration::from_millis(5000),
                beacon_send: a_beacon_send(),
                beacon_recv: a_beacon_send(),
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

        match ci.payload.process(Command::StartDH(StartDHCommand::new(
            1,
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

        let (state, stats) = match ci.payload.process(Command::QueryDH(
            tcslibgs::QueryDHCommand::new(2, DHName::new("DH2")),
        )) {
            Telemetry::QueryDH(tm) => (tm.state, tm.statistics),
            other => panic!("expected QUERY_DH telemetry, got {other:?}"),
        };
        assert_ne!(
            stats.bytes_received, 0,
            "a handler that has been spoken to should report receiving something"
        );

        // And what it is doing, which nothing else on the link says: the
        // ground had only its own commands to go by, so a panel showed what
        // the last START_DH implied and went on showing it.
        assert_eq!(
            state,
            Some(DHState::Active),
            "a running handler should answer that it is running"
        );
    }

    /// A QUERY_DH for a handler this tcspecial has not got answers NotFound
    /// and no state.
    ///
    /// The pair is the point. A state would have to be invented for a handler
    /// that does not exist, and a ground station reading one would show it --
    /// which is exactly the fault the state was added to cure, in the one
    /// case that causes it: a MOC attached to a tcspecial left running for
    /// another payload set.
    #[test]
    fn a_query_for_a_handler_that_is_not_here_answers_with_no_state() {
        let ci = interpreter();

        match ci.payload.process(Command::QueryDH(tcslibgs::QueryDHCommand::new(
            3,
            DHName::new("nobody"),
        ))) {
            Telemetry::QueryDH(tm) => {
                assert_eq!(tm.header.status, CommandStatus::NotFound);
                assert_eq!(tm.state, None);
            }
            other => panic!("expected QUERY_DH telemetry, got {other:?}"),
        }
    }

    /// A handler that has been made and not started answers Created.
    ///
    /// Which a panel shows as stopped, that being what the word is read for;
    /// what matters here is that the state is the handler's own rather than
    /// the last thing the ground asked of it.
    #[test]
    fn a_query_before_any_start_answers_the_state_the_handler_is_in() {
        use tcslibgs::{DHConfig, DHName, DeviceConfig, EndpointConfig};

        let handler = DHConfig {
            name: DHName::new("auto-send"),
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/urandom".to_string(),
            }),
            packet_size: 1,
            oc: None,
            ground: None,
            mode: Default::default(),
            stale_interval: None,
        };

        let mut ci = CommandInterpreter::new(
            CIConfig {
                address: "127.0.0.1".to_string(),
                port: 0,
                payload_command_source_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 0,
                },
                payload_command_dest_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 0,
                },
                // The far ends and the bases, which nothing in the
                // commanding reads: see tcslibgs::CIConfig.
                moc_cmd_send: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 4100,
                },
                payload_source_base: tcslibgs::HostPort {
                    address: "0.0.0.0".to_string(),
                    port: 6000,
                },
                payload_dest_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 6100,
                },
                beacon_interval: Duration::from_millis(5000),
                beacon_send: a_beacon_send(),
                beacon_recv: a_beacon_send(),
                log_dir: None,
                log_segment_bytes: 65_536,
            },
            vec![handler],
            a_config_version(),
            a_digest(),
        )
        .expect("an interpreter");
        ci.initialize_handlers().expect("handlers are made at startup");

        match ci.payload.process(Command::QueryDH(tcslibgs::QueryDHCommand::new(
            4,
            DHName::new("auto-send"),
        ))) {
            Telemetry::QueryDH(tm) => {
                assert_eq!(tm.header.status, CommandStatus::Success);
                assert_eq!(tm.state, Some(DHState::Created));
            }
            other => panic!("expected QUERY_DH telemetry, got {other:?}"),
        }
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
            ground: None,
                    mode: Default::default(),
                    stale_interval: None,
        };

        let mut ci = CommandInterpreter::new(
            CIConfig {
                address: "127.0.0.1".to_string(),
                port: 0,
                // And each payload's own command link, which a test binds at a
                // port of the system's choosing: a test that named one would fail
                // whenever something else on the machine held it.
                payload_command_source_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 0,
                },
                payload_command_dest_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 0,
                },
                // The far ends and the bases, which nothing in the
                // commanding reads: see tcslibgs::CIConfig.
                moc_cmd_send: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 4100,
                },
                payload_source_base: tcslibgs::HostPort {
                    address: "0.0.0.0".to_string(),
                    port: 6000,
                },
                payload_dest_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 6100,
                },
                beacon_interval: Duration::from_millis(5000),
                beacon_send: a_beacon_send(),
                beacon_recv: a_beacon_send(),
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
            match ci.payload.process(Command::StartDH(StartDHCommand::new(
                attempt,
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
            // The port under test is the one already held; the other is the
            // OS's to choose, so what fails is the one this is about.
            payload_command_source_base: tcslibgs::HostPort {
                address: "127.0.0.1".to_string(),
                port: 0,
            },
            payload_command_dest_base: tcslibgs::HostPort {
                address: "127.0.0.1".to_string(),
                port: 0,
            },
            moc_cmd_send: tcslibgs::HostPort {
                address: "127.0.0.1".to_string(),
                port: 4100,
            },
            payload_source_base: tcslibgs::HostPort {
                address: "0.0.0.0".to_string(),
                port: 6000,
            },
            payload_dest_base: tcslibgs::HostPort {
                address: "127.0.0.1".to_string(),
                port: 6100,
            },
            beacon_interval: Duration::from_millis(5000),
            beacon_send: a_beacon_send(),
            beacon_recv: a_beacon_send(),
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
        interpreter_commanded_from(std::net::SocketAddr::from(([127, 0, 0, 1], 4100)))
    }

    /// An interpreter that takes its commands from a particular socket.
    ///
    /// For a test that really sends one: the loop refuses a command from
    /// anywhere but `tcs_cmd.send`, so a test's ground socket is bound first
    /// and the configuration is told where it is.
    fn interpreter_commanded_from(from: std::net::SocketAddr) -> CommandInterpreter {
        CommandInterpreter::new(
            CIConfig {
                address: "127.0.0.1".to_string(),
                port: 0,
                // Each payload's command link comes from the same socket the
                // spacecraft's commands do, which is what a test has: one
                // ground socket, and the payload at id 0 counted from it.
                payload_command_source_base: tcslibgs::HostPort {
                    address: from.ip().to_string(),
                    port: from.port(),
                },
                // And is taken at a port of the system's choosing: a test
                // that named one would fail whenever something else on the
                // machine held it.
                payload_command_dest_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 0,
                },
                // The far ends and the bases, which nothing in the
                // commanding reads: see tcslibgs::CIConfig.
                moc_cmd_send: tcslibgs::HostPort {
                    address: from.ip().to_string(),
                    port: from.port(),
                },
                payload_source_base: tcslibgs::HostPort {
                    address: "0.0.0.0".to_string(),
                    port: 6000,
                },
                payload_dest_base: tcslibgs::HostPort {
                    address: "127.0.0.1".to_string(),
                    port: 6100,
                },
                beacon_interval: Duration::from_millis(5000),
                beacon_send: a_beacon_send(),
                beacon_recv: a_beacon_send(),
                log_dir: None,
                log_segment_bytes: 65_536,
            },
            vec![a_payload()],
            a_config_version(),
            a_digest(),
        )
        .expect("an interpreter")
    }

    /// The one payload a test interpreter carries.
    ///
    /// One, because the commanding needs a payload to have a command link for
    /// at all: a set of none has no payload link to answer on. A device file,
    /// which is the kind that needs nothing made for it.
    fn a_payload() -> DHConfig {
        DHConfig {
            name: DHName::new(A_PAYLOAD),
            endpoint: EndpointConfig::Device(tcslibgs::DeviceConfig {
                path: "/dev/urandom".to_string(),
            }),
            packet_size: 1,
            oc: None,
            ground: None,
            mode: Default::default(),
            stale_interval: None,
        }
    }

    /// Its name, which a command about it has to carry: a command naming
    /// another payload is refused on this payload's link.
    const A_PAYLOAD: &str = "auto-send";

    /// CONFIG_DH is refused rather than answered with success.
    ///
    /// It answered Success while doing nothing, and the command carries
    /// nothing to configure a handler with, so the ground was told a handler
    /// had been reconfigured when none had. A control application cannot tell
    /// a success it earned from one it did not; this pins the refusal, and is
    /// what will say so if the command is ever implemented.
    #[test]
    fn config_dh_is_refused_rather_than_answered_with_success() {
        let ci = interpreter();

        let answer = ci
            .payload
            .process(Command::ConfigDH(ConfigDHCommand::new(
                7,
                DHName::new("whoever"),
            )));

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

    // -- the two links ------------------------------------------------------

    /// A command that arrived on the wrong link is refused, and the refusal
    /// is of the kind the command asked for.
    ///
    /// Refused rather than served, because a command on the wrong link is one
    /// end not reading the configuration the other is, and serving it would
    /// hide that in the one place the split has to be reliable. Of the right
    /// kind, because the type is how the ground pairs an answer with what it
    /// asked: a refusal it cannot place reads as a command that went
    /// unanswered, which is the one thing a refusal exists to deny.
    #[test]
    fn a_command_on_the_wrong_link_is_refused_in_its_own_words() {
        let mut ci = interpreter();

        // A payload command, offered to the spacecraft link.
        match ci.process_command(Command::StartDH(StartDHCommand::new(
            7,
            DHType::Network,
            DHName::new("whoever"),
        ))) {
            Telemetry::StartDH(tm) => {
                assert_eq!(tm.header.status, CommandStatus::InvalidCommand);
                assert_eq!(tm.header.sequence, 7, "the answer is to that command");
            }
            other => panic!("expected START_DH telemetry, got {other:?}"),
        }

        // And a spacecraft command, offered to the payload link.
        match ci.payload.process(Command::Restart(tcslibgs::RestartCommand::new(
            8,
            ArmKey(0x1234),
        ))) {
            Telemetry::Restart(tm) => {
                assert_eq!(tm.header.status, CommandStatus::InvalidCommand);
                assert_eq!(tm.header.sequence, 8);
            }
            other => panic!("expected RESTART telemetry, got {other:?}"),
        }

        // Each link still serves its own: the test above is about the link a
        // command arrived on and not about commands being refused.
        match ci.payload.process(Command::StartDH(StartDHCommand::new(
            9,
            DHType::Network,
            DHName::new("whoever"),
        ))) {
            Telemetry::StartDH(tm) => assert_eq!(
                tm.header.status,
                CommandStatus::NotFound,
                "the payload link reached the handlers and found none"
            ),
            other => panic!("expected START_DH telemetry, got {other:?}"),
        }
        match ci.process_command(Command::Ping(tcslibgs::PingCommand::new(10))) {
            Telemetry::Ping(tm) => assert!(tm.header.status.is_success()),
            other => panic!("expected PING telemetry, got {other:?}"),
        }
    }

    /// What each end read is answered on the spacecraft's link, and refused
    /// on a payload's.
    ///
    /// A CONNECT asks whether there is a connection to the vehicle, which is
    /// what that link is; a payload's link carries what is asked of a
    /// payload, and a ground station with no connection has nothing to ask
    /// one.
    #[test]
    fn a_connect_is_answered_on_the_spacecrafts_link_alone() {
        let mut ci = interpreter();
        let ask = || {
            Command::Connect(ConnectCommand::new(
                3,
                ConfigVersion::of_this_build(),
                a_digest(),
            ))
        };

        match ci.process_command(ask()) {
            Telemetry::Connect(answer) => {
                assert_eq!(answer.header.sequence, 3, "the answer is to that command");
                assert_eq!(answer.digest, a_digest());
                assert_eq!(answer.config_version, a_config_version());
            }
            other => panic!("expected CONNECT telemetry, got {other:?}"),
        }

        // And on a payload's link it is refused, as a command that belongs on
        // the other one: the two ends are not reading the same configuration,
        // and serving it would hide that.
        match ci.payload.process(ask()) {
            Telemetry::Connect(answer) => assert_eq!(
                answer.header.status,
                CommandStatus::InvalidCommand,
                "a CONNECT on a payload's link should be refused"
            ),
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    /// A payload's command link answers on its own socket.
    ///
    /// The loop, rather than what the loop calls: a link nothing serves is a
    /// command that goes unanswered, and nothing above this would notice.
    #[test]
    fn the_payload_link_answers_on_its_own_socket() {
        let ground = UdpSocket::bind("127.0.0.1:0").expect("a ground socket");
        ground
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("a timeout");
        let from = ground.local_addr().expect("where it is");

        let ci = interpreter_commanded_from(from);
        let link = ci.payload_command_address(0).expect("where it is");
        let serving = Arc::new(AtomicBool::new(true));

        let payload = ci.payload.clone();
        let socket = ci.payload_links[0]
            .socket
            .as_ref()
            .unwrap()
            .try_clone()
            .unwrap();
        let expected = tcslibgs::HostPort {
            address: from.ip().to_string(),
            port: from.port(),
        };
        let name = DHName::new(A_PAYLOAD);
        let stop = serving.clone();
        let served =
            thread::spawn(move || payload.serve(socket, &expected, &name, 0, serving));

        let asked = serde_json::to_vec(&Command::StartDH(StartDHCommand::new(
            11,
            DHType::Network,
            DHName::new(A_PAYLOAD),
        )))
        .expect("it serializes");
        ground.send_to(&asked, link).expect("it is sent");

        let mut buffer = [0u8; 65535];
        let (size, _) = ground.recv_from(&mut buffer).expect("an answer");
        match serde_json::from_slice::<Telemetry>(&buffer[..size]).expect("telemetry") {
            Telemetry::StartDH(tm) => {
                assert_eq!(tm.header.sequence, 11);
                assert_eq!(tm.header.status, CommandStatus::NotFound);
            }
            other => panic!("expected START_DH telemetry, got {other:?}"),
        }

        // And the loop stops being told to serve, which is how a shutdown
        // reaches a loop in another thread.
        stop.store(false, Ordering::Relaxed);
        served
            .join()
            .expect("the thread ends")
            .expect("it served without failing");
    }

    /// A command from anywhere but the MOC's own socket is ignored.
    ///
    /// Not answered, not logged as a command, not acted on. A command is a
    /// command to restart a spacecraft, and the one end that is supposed to
    /// send them is written down in the file both ends read -- so a datagram
    /// from elsewhere is somebody else's, and answering it would tell whoever
    /// sent it that something is listening here.
    ///
    /// The real socket is asked afterwards, so that this is about where the
    /// command came from and not about the loop having stopped.
    #[test]
    fn a_command_from_the_wrong_socket_is_ignored() {
        let expected = UdpSocket::bind("127.0.0.1:0").expect("the MOC's own socket");
        expected
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a timeout");

        let mut ci = interpreter_commanded_from(expected.local_addr().expect("where it is"));
        let link = ci.command_address().expect("where it is");
        thread::spawn(move || {
            let _ = ci.run();
        });

        // A stranger's ping, which must go unanswered.
        let stranger = UdpSocket::bind("127.0.0.1:0").expect("another socket");
        stranger
            .set_read_timeout(Some(Duration::from_millis(500)))
            .expect("a timeout");
        let ping = |sequence: u32| {
            serde_json::to_vec(&Command::Ping(tcslibgs::PingCommand::new(sequence)))
                .expect("it serializes")
        };
        stranger.send_to(&ping(30), link).expect("it is sent");
        let mut buffer = [0u8; 65535];
        assert!(
            stranger.recv_from(&mut buffer).is_err(),
            "a command from the wrong socket was answered"
        );

        // And the loop is still there, answering the socket the
        // configuration names.
        expected.send_to(&ping(31), link).expect("it is sent");
        let (size, _) = expected
            .recv_from(&mut buffer)
            .expect("the MOC's own socket is answered");
        match serde_json::from_slice::<Telemetry>(&buffer[..size]).expect("telemetry") {
            Telemetry::Ping(tm) => assert_eq!(tm.header.sequence, 31),
            other => panic!("expected PING telemetry, got {other:?}"),
        }
    }

    /// A command about another payload is refused on this payload's link.
    ///
    /// Every payload command carries the name of its payload and arrives on
    /// the socket that payload is commanded on, so the two disagreeing is one
    /// end not reading the configuration the other is -- which is the fault
    /// these links must not hide. Refused rather than served: it reached the
    /// spacecraft, so something can be said about it.
    ///
    /// And a command from anywhere but the socket the configuration names is
    /// ignored outright, with nothing sent back, as a spacecraft command is.
    #[test]
    fn a_command_about_another_payload_is_refused() {
        let ground = UdpSocket::bind("127.0.0.1:0").expect("a ground socket");
        ground
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("a timeout");
        let from = ground.local_addr().expect("where it is");

        let ci = interpreter_commanded_from(from);
        let link = ci.payload_command_address(0).expect("where it is");
        let serving = Arc::new(AtomicBool::new(true));
        let payload = ci.payload.clone();
        let socket = ci.payload_links[0]
            .socket
            .as_ref()
            .unwrap()
            .try_clone()
            .unwrap();
        let expected = tcslibgs::HostPort {
            address: from.ip().to_string(),
            port: from.port(),
        };
        let name = DHName::new(A_PAYLOAD);
        let stop = serving.clone();
        let served =
            thread::spawn(move || payload.serve(socket, &expected, &name, 0, serving));

        let asking = |sequence: u32, about: &str| {
            serde_json::to_vec(&Command::QueryDH(tcslibgs::QueryDHCommand::new(
                sequence,
                DHName::new(about),
            )))
            .expect("it serializes")
        };

        // Another payload's name on this payload's link.
        ground
            .send_to(&asking(40, "someone-else"), link)
            .expect("it is sent");
        let mut buffer = [0u8; 65535];
        let (size, _) = ground.recv_from(&mut buffer).expect("an answer");
        match serde_json::from_slice::<Telemetry>(&buffer[..size]).expect("telemetry") {
            // The type the command asked for, which is how the ground pairs
            // an answer with what it asked.
            Telemetry::QueryDH(tm) => {
                assert_eq!(tm.header.sequence, 40);
                assert_eq!(tm.header.status, CommandStatus::InvalidCommand);
            }
            other => panic!("expected QUERY_DH telemetry, got {other:?}"),
        }

        // A stranger's command about the right payload, which is not answered
        // at all.
        let stranger = UdpSocket::bind("127.0.0.1:0").expect("another socket");
        stranger
            .set_read_timeout(Some(Duration::from_millis(500)))
            .expect("a timeout");
        stranger
            .send_to(&asking(41, A_PAYLOAD), link)
            .expect("it is sent");
        assert!(
            stranger.recv_from(&mut buffer).is_err(),
            "a command from the wrong socket was answered"
        );

        // And this payload's own name, which is served.
        ground
            .send_to(&asking(42, A_PAYLOAD), link)
            .expect("it is sent");
        let (size, _) = ground.recv_from(&mut buffer).expect("an answer");
        match serde_json::from_slice::<Telemetry>(&buffer[..size]).expect("telemetry") {
            Telemetry::QueryDH(tm) => assert_eq!(tm.header.sequence, 42),
            other => panic!("expected QUERY_DH telemetry, got {other:?}"),
        }

        stop.store(false, Ordering::Relaxed);
        served
            .join()
            .expect("the thread ends")
            .expect("it served without failing");
    }

    /// A spacecraft command is answered while a payload command waits.
    ///
    /// This is the whole of why there are two links. The payload command here
    /// cannot finish -- the test holds the handler map it needs -- and the
    /// PING must be answered anyway. With one socket and one loop it could
    /// not be: the ping would be the next datagram behind a command that
    /// never finishes, which is a spacecraft that cannot be rescued while it
    /// is busy.
    #[test]
    fn a_spacecraft_command_is_answered_while_a_payload_command_waits() {
        // Bound first, because the loop takes commands only from where its
        // configuration says the MOC sends them.
        let ground = UdpSocket::bind("127.0.0.1:0").expect("a ground socket");
        ground
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a timeout");

        let mut ci = interpreter_commanded_from(ground.local_addr().expect("where it is"));
        let spacecraft_link = ci.command_address().expect("where it is");
        let payload_link = ci.payload_command_address(0).expect("where it is");
        let handlers = ci.data_handlers.clone();

        // Held for as long as this test wants the payload link stuck.
        let held = handlers.lock().expect("the handler map");

        thread::spawn(move || {
            let _ = ci.run();
        });

        let stuck = serde_json::to_vec(&Command::StartDH(StartDHCommand::new(
            20,
            DHType::Network,
            DHName::new(A_PAYLOAD),
        )))
        .expect("it serializes");
        ground.send_to(&stuck, payload_link).expect("it is sent");

        let ping = serde_json::to_vec(&Command::Ping(tcslibgs::PingCommand::new(21)))
            .expect("it serializes");
        ground.send_to(&ping, spacecraft_link).expect("it is sent");

        let mut buffer = [0u8; 65535];
        let (size, _) = ground
            .recv_from(&mut buffer)
            .expect("the spacecraft link answered");
        match serde_json::from_slice::<Telemetry>(&buffer[..size]).expect("telemetry") {
            Telemetry::Ping(tm) => {
                assert_eq!(tm.header.sequence, 21);
                assert!(tm.header.status.is_success());
            }
            other => panic!(
                "the first answer should be the ping's, since the payload \
                 command cannot finish: got {other:?}"
            ),
        }

        drop(held);
    }
}
