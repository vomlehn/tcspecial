//! The ground's end of a cFS software bus: a ground station's SBN peer, and the
//! simulated cFS end that tcssim runs.
//!
//! A vehicle that answers to cFS has no command socket, no beacon socket and
//! no telemetry sockets: everything it takes and sends crosses one SBN peer
//! as cFS packets told apart by message ID. See "What crosses the bus" in
//! `docs/design.rst`, [`tcslibgs::sbn`] for the wire and [`tcslibgs::cfs`]
//! for the packets on it; nothing here states either again.
//!
//! # The simulated cFS end
//!
//! [`SimulatedCfs`] is the far end of the bus as tcssim plays it: it binds
//! `sbn.peer`, is processor `sbn.peer_processor_id` to the vehicle, and is a
//! bus to every ground station that joins it -- what the vehicle publishes goes
//! to each station that subscribed to it, and what a station publishes goes to
//! the vehicle where the vehicle subscribed to it. A real cFS does not do that
//! last part for a ground station: SBN never sends one peer what another
//! published (see [`tcslibgs::sbn::ground_processor_id`]), so a ground station
//! is a peer of the simulated end and of nothing else, and the ground commands
//! a vehicle behind a real cFS through that cFS's own ground system.
//!
//! It is kept here, beside the station's peer, rather than in tcssim, so
//! that the two ends are tested together and against the vehicle itself.
//!
//! # The station's peer
//!
//! [`Ground`] is a station's one peer of the bus, kept on a thread of its
//! own, which announces, sends heartbeats and refuses a far end of another
//! release or ident, as the vehicle does. It turns the bus back into the UDP
//! links a station already speaks: a loopback socket for the spacecraft's
//! commands and one for the nodes', which [`crate::TcsClient`] sends to and
//! hears its answers from, and a loopback socket of its own for each reader
//! of telemetry -- the beacon watch, a gateway node -- made by
//! [`Ground::receiving`]. So everything built on those links works on the
//! bus unchanged, and the bus is spoken in one place.
//!
//! One peer for each configuration's bus in a process, made the first time
//! anything asks for it and kept for as long as the process runs: its
//! command links are addresses a station may connect to again and again,
//! and a station that opened a second peer of the same bus would be two
//! processors of one ID.
//!
//! # What allocates
//!
//! Starting either end, and [`Ground::receiving`], which are done when a
//! station opens its links. Once running, each end reads into and writes
//! from the room it was started with; what allocates is a line said about
//! the peer's coming and going or about something refused.

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tcslibgs::graph::parameters::{ci_config_of, flatten};
use tcslibgs::graph::telemetry::{msgids, Carries, MsgIdUse};
use tcslibgs::graph::Graph;
use tcslibgs::sbn::{self, kind, Due, Header, Liveness, SubscriptionRefusal};
use tcslibgs::{came_from, cfs, Bus, Command, HostPort};

/// The longest datagram either end reads: the longest UDP datagram.
const ROOM: usize = 65_536;

/// How long a wait may last before either end looks at whether it has been
/// told to stop, and at the clock.
const TICK: Duration = Duration::from_millis(100);

/// How many ground stations the simulated end keeps at once: more than a desk
/// runs, and few enough to be made at once when it starts.
pub const STATIONS_AT_ONCE: usize = 8;

/// How many words the subscription bit set takes: one bit per message ID.
const WORDS: usize = (u16::MAX as usize + 1) / 64;

/// Reads the bus a configuration answers over.
///
/// # Arguments
///
/// * `graph` - The configuration's graph.
///
/// # Returns
///
/// The bus, or `None` for a configuration whose links are sockets or that
/// has no `tcspecial` section.
///
/// # Errors
///
/// What is wrong with the `tcspecial` section.
pub fn bus_of(graph: &Graph) -> Result<Option<Bus>, String> {
    let Some(section) = graph.tcspecial.as_ref() else {
        return Ok(None);
    };
    let ci = ci_config_of(&flatten(section))?.to_ci_config()?;
    Ok(ci.bus().cloned())
}

/// Resolves an address the configuration states.
///
/// # Arguments
///
/// * `what` - The attribute, for the message.
/// * `at`   - The address.
///
/// # Returns
///
/// The first socket address it resolves to.
///
/// # Errors
///
/// A message naming the attribute where it resolves to nothing.
fn resolved(what: &str, at: &HostPort) -> Result<SocketAddr, String> {
    at.to_string()
        .to_socket_addrs()
        .ok()
        .and_then(|mut all| all.next())
        .ok_or_else(|| format!("{what} is {at}, which resolves to no address"))
}

/// Binds a socket on loopback at a port the system chooses.
///
/// # Arguments
///
/// * `what` - What it is for, for the message.
///
/// # Returns
///
/// The socket.
///
/// # Errors
///
/// A message saying what could not be bound.
fn loopback(what: &str) -> Result<UdpSocket, String> {
    UdpSocket::bind("127.0.0.1:0")
        .map_err(|e| format!("the bus's {what} cannot be bound: {e}"))
}

/// Waits for any of `fds` to be readable, for at most `wait`.
///
/// # Arguments
///
/// * `fds`  - What to wait on; each one's `revents` is set.
/// * `wait` - The longest to wait.
fn wait_on(fds: &mut [libc::pollfd], wait: Duration) {
    for fd in fds.iter_mut() {
        fd.revents = 0;
    }
    let timeout = i32::try_from(wait.as_millis()).unwrap_or(i32::MAX);
    // SAFETY: poll needs an array of as many pollfds as it is told, writable
    // for the call: the count is the slice's own length. An interrupted or
    // failed poll leaves every `revents` nought, which reads as nothing
    // ready, and the caller's loop asks again.
    unsafe {
        libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout);
    }
}

/// The identity a datagram is sent with.
#[derive(Debug, Clone, Copy)]
struct Identity {
    processor_id: u32,
    spacecraft_id: u32,
}

impl Identity {
    /// Writes the header of a datagram of `what` type before a message of
    /// `length` bytes already in place after it.
    ///
    /// # Arguments
    ///
    /// * `what`   - The SBN message type.
    /// * `out`    - Room for the datagram, the message already after the header.
    /// * `length` - How long the message is.
    fn header(&self, what: u8, out: &mut [u8], length: usize) {
        Header {
            kind: what,
            processor_id: self.processor_id,
            spacecraft_id: self.spacecraft_id,
        }
        .write_before(out, length);
    }
}

/// Where the ground's peer sends what arrives under a message ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    /// An answer to a spacecraft command: to whoever sent the last one.
    SpacecraftAnswer,
    /// An answer to a node command: to whoever sent the last one.
    NodeAnswer,
    /// Anything else: to each reader that asked for it.
    Readers,
}

/// A ground station's peer of a cFS software bus, kept on a thread of its
/// own: see the module's documentation.
#[derive(Debug)]
pub struct Ground {
    /// Where the spacecraft's commands are sent, and its answers come from.
    spacecraft: SocketAddr,
    /// Where the nodes' commands are sent, and their answers come from.
    nodes: SocketAddr,
    /// Where what each reader asked for comes from.
    delivering: SocketAddr,
    /// Each reader's message ID and the address it is sent to.
    readers: Arc<Mutex<Vec<(u16, SocketAddr)>>>,
    /// Whether the far end is connected and has said its protocol version
    /// is the release's.
    verified: Arc<AtomicBool>,
}

/// The peers this process has started, one for each bus.
static GROUNDS: Mutex<Vec<(Bus, Arc<Ground>)>> = Mutex::new(Vec::new());

impl Ground {
    /// This process's peer of the bus a configuration answers over, started
    /// the first time it is asked for.
    ///
    /// # Arguments
    ///
    /// * `graph` - The configuration's graph.
    ///
    /// # Returns
    ///
    /// The peer, or `None` for a configuration that does not answer over a
    /// bus.
    ///
    /// # Errors
    ///
    /// What is wrong with the `tcspecial` section or its message IDs, or
    /// what could not be bound.
    pub fn of(graph: &Graph) -> Result<Option<Arc<Ground>>, String> {
        let Some(bus) = bus_of(graph)? else {
            return Ok(None);
        };
        let mut grounds = GROUNDS.lock().map_err(|_| {
            "the bus's peers were left locked by a thread that panicked"
                .to_string()
        })?;
        if let Some((_, ground)) = grounds.iter().find(|(b, _)| *b == bus) {
            return Ok(Some(ground.clone()));
        }
        let uses = msgids(graph).map_err(|refused| refused.why)?;
        let ground = Arc::new(Ground::start(&bus, &uses)?);
        grounds.push((bus, ground.clone()));
        Ok(Some(ground))
    }

    /// Starts a peer of the bus, on a thread of its own.
    ///
    /// # Arguments
    ///
    /// * `bus`  - The bus, as the configuration states it.
    /// * `uses` - Every message ID the configuration uses, and what for.
    ///
    /// # Returns
    ///
    /// The peer, announcing itself to the far end.
    ///
    /// # Errors
    ///
    /// A message saying what could not be resolved or bound.
    fn start(bus: &Bus, uses: &[MsgIdUse]) -> Result<Ground, String> {
        let peer = resolved("sbn.peer", &bus.sbn.peer)?;
        let any = if peer.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };
        let socket = UdpSocket::bind(any).map_err(|e| {
            format!("the ground's SBN socket cannot be bound: {e}")
        })?;
        let spacecraft = loopback("spacecraft command link")?;
        let nodes = loopback("node command link")?;
        let delivering = loopback("telemetry socket")?;
        let address = |socket: &UdpSocket| {
            socket
                .local_addr()
                .map_err(|e| format!("the bus's socket has no address: {e}"))
        };
        let readers = Arc::new(Mutex::new(Vec::new()));
        let verified = Arc::new(AtomicBool::new(false));
        let ground = Ground {
            spacecraft: address(&spacecraft)?,
            nodes: address(&nodes)?,
            delivering: address(&delivering)?,
            readers: readers.clone(),
            verified: verified.clone(),
        };

        let mut routes: Vec<(u16, Route)> = uses
            .iter()
            .filter(|used| !used.command)
            .map(|used| {
                let route = match used.carries {
                    Carries::SpacecraftAnswers => Route::SpacecraftAnswer,
                    Carries::NodeAnswers(_) => Route::NodeAnswer,
                    _ => Route::Readers,
                };
                (used.msgid, route)
            })
            .collect();
        routes.sort_unstable_by_key(|r| r.0);
        let node_commands = uses
            .iter()
            .filter_map(|used| match used.carries {
                Carries::NodeCommands(id) => Some((id, used.msgid)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut keeper = Keeper {
            socket,
            peer,
            peer_at: bus.sbn.peer.clone(),
            us: Identity {
                processor_id: sbn::ground_processor_id(
                    bus.sbn.processor_id,
                    bus.sbn.peer_processor_id,
                ),
                spacecraft_id: bus.sbn.spacecraft_id,
            },
            peer_processor_id: bus.sbn.peer_processor_id,
            ident: bus.sbn.ident,
            release: bus.sbn.release,
            liveness: Liveness::new(Instant::now()),
            verified,
            spacecraft,
            nodes,
            delivering,
            spacecraft_client: None,
            nodes_client: None,
            routes,
            spacecraft_commands: bus.cmd_msgid,
            node_commands,
            readers,
            room: vec![0u8; ROOM],
            out: vec![0u8; ROOM],
            sequence: 0,
        };
        std::thread::Builder::new()
            .name("sbn-ground".to_string())
            .spawn(move || keeper.run())
            .map_err(|e| format!("the bus's peer cannot be started: {e}"))?;
        Ok(ground)
    }

    /// Where the spacecraft's commands are sent, and its answers come from:
    /// what [`crate::CommandLinks::interpreter`] is on a bus.
    pub fn spacecraft(&self) -> SocketAddr {
        self.spacecraft
    }

    /// Where the nodes' commands are sent, and their answers come from.
    pub fn nodes(&self) -> SocketAddr {
        self.nodes
    }

    /// Where what a reader asked for comes from, which a reader takes only
    /// from.
    pub fn delivering(&self) -> SocketAddr {
        self.delivering
    }

    /// Whether the far end is connected and has said its protocol version
    /// is the release's.
    pub fn is_verified(&self) -> bool {
        self.verified.load(Ordering::SeqCst)
    }

    /// A socket of the reader's own, to which what the vehicle publishes
    /// under `msgid` is sent, as it would be sent to the ground over UDP:
    /// what follows the cFS packet's headers, whole, from
    /// [`Ground::delivering`].
    ///
    /// # Arguments
    ///
    /// * `msgid` - The message ID the reader wants.
    ///
    /// # Returns
    ///
    /// The socket, on loopback.
    ///
    /// # Errors
    ///
    /// A message where it cannot be bound.
    pub fn receiving(&self, msgid: u16) -> Result<UdpSocket, String> {
        let socket = loopback("reader's socket")?;
        let at = socket
            .local_addr()
            .map_err(|e| format!("a reader's socket has no address: {e}"))?;
        self.readers
            .lock()
            .map_err(|_| "the bus's readers were left locked".to_string())?
            .push((msgid, at));
        Ok(socket)
    }
}

/// The ground's peer, as its thread keeps it.
struct Keeper {
    socket: UdpSocket,
    peer: SocketAddr,
    peer_at: HostPort,
    us: Identity,
    peer_processor_id: u32,
    ident: [u8; sbn::IDENT_BYTES],
    release: sbn::Release,
    liveness: Liveness,
    verified: Arc<AtomicBool>,
    spacecraft: UdpSocket,
    nodes: UdpSocket,
    delivering: UdpSocket,
    /// Who sent the last command on each link, whom its answers go to.
    spacecraft_client: Option<SocketAddr>,
    nodes_client: Option<SocketAddr>,
    /// Every telemetry message ID, sorted, and where it goes.
    routes: Vec<(u16, Route)>,
    spacecraft_commands: u16,
    /// Each node's device id and the message ID its commands go under.
    node_commands: Vec<(usize, u16)>,
    readers: Arc<Mutex<Vec<(u16, SocketAddr)>>>,
    room: Vec<u8>,
    out: Vec<u8>,
    sequence: u16,
}

impl Keeper {
    /// Keeps the peer for as long as the process runs.
    fn run(&mut self) {
        let mut fds = [
            self.socket.as_raw_fd(),
            self.spacecraft.as_raw_fd(),
            self.nodes.as_raw_fd(),
        ]
        .map(|fd| libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        });
        loop {
            self.tick(Instant::now());
            let wait = self.liveness.until_due(Instant::now()).min(TICK);
            wait_on(&mut fds, wait);
            if fds[0].revents & libc::POLLIN != 0 {
                self.take_from_bus(Instant::now());
            }
            if fds[1].revents & libc::POLLIN != 0 {
                self.command(false);
            }
            if fds[2].revents & libc::POLLIN != 0 {
                self.command(true);
            }
        }
    }

    /// Does what the clock says is due.
    ///
    /// # Arguments
    ///
    /// * `now` - The time.
    fn tick(&mut self, now: Instant) {
        match self.liveness.due(now) {
            Due::Nothing => {}
            Due::Announce => {
                self.say(kind::ANNOUNCE, 0);
                self.liveness.announced(now);
            }
            Due::Heartbeat => self.say(kind::HEARTBEAT, 0),
            Due::Disconnect => {
                eprintln!(
                    "tcslib: the SBN peer at {} has been silent for {:?}: \
                     disconnected, and announcing to it again",
                    self.peer_at,
                    sbn::SILENCE
                );
                self.disconnect();
            }
        }
    }

    /// Takes what the far end sent.
    ///
    /// # Arguments
    ///
    /// * `now` - When it arrived.
    fn take_from_bus(&mut self, now: Instant) {
        let Ok((size, from)) = self.socket.recv_from(&mut self.room) else {
            return;
        };
        if !came_from(&self.peer_at, &from) {
            return;
        }
        let Ok((header, _)) = Header::read(&self.room[..size]) else {
            return;
        };
        if header.processor_id != self.peer_processor_id
            || header.spacecraft_id != self.us.spacecraft_id
        {
            return;
        }
        if self.liveness.heard(now) {
            eprintln!(
                "tcslib: the SBN peer at {} is connected, as processor {} \
                 of spacecraft {}",
                self.peer_at, self.us.processor_id, self.us.spacecraft_id
            );
            self.out[sbn::HEADER_BYTES] = self.release.protocol_version;
            self.say(kind::PROTOCOL, 1);
            self.subscribe();
        }
        let body = sbn::HEADER_BYTES..size;
        match header.kind {
            kind::PROTOCOL => {
                if self.room[body] == [self.release.protocol_version] {
                    self.verified.store(true, Ordering::SeqCst);
                } else {
                    eprintln!(
                        "tcslib: the SBN peer at {} says its protocol version \
                         is {:?}, and SBN {} speaks {}: refused",
                        self.peer_at,
                        &self.room[sbn::HEADER_BYTES..size],
                        self.release.name,
                        self.release.protocol_version
                    );
                    self.refuse();
                }
            }
            kind::SUBSCRIBE | kind::UNSUBSCRIBE => {
                let refused = if let Err(SubscriptionRefusal::Ident(carried)) =
                    sbn::read_subscriptions(&self.room[body], &self.ident)
                {
                    let (mut arrived, mut expected) = ([0u8; 96], [0u8; 96]);
                    eprintln!(
                        "tcslib: the SBN peer at {} subscribed with the ident \
                         \"{}\", and sbn.ident says \"{}\": refused",
                        self.peer_at,
                        sbn::ident_text(carried, &mut arrived),
                        sbn::ident_text(&self.ident, &mut expected)
                    );
                    true
                } else {
                    false
                };
                if refused {
                    self.refuse();
                }
            }
            kind::APPLICATION => self.telemetry(body),
            kind::DISCONNECT => self.disconnect(),
            _ => {}
        }
    }

    /// Sends a telemetry packet's data on to whoever it is for.
    ///
    /// # Arguments
    ///
    /// * `body` - Where the message, after SBN's header, is in the room.
    fn telemetry(&mut self, body: std::ops::Range<usize>) {
        if !self.verified.load(Ordering::SeqCst) {
            return;
        }
        let Ok(frame) = cfs::read_telemetry(&self.room[body]) else {
            return;
        };
        let Ok(at) = self.routes.binary_search_by_key(&frame.msgid, |r| r.0)
        else {
            return;
        };
        // A send that fails is a reader that has gone, which is the
        // reader's to notice; the bus goes on.
        match self.routes[at].1 {
            Route::SpacecraftAnswer => {
                if let Some(to) = self.spacecraft_client {
                    let _ = self.spacecraft.send_to(frame.data, to);
                }
            }
            Route::NodeAnswer => {
                if let Some(to) = self.nodes_client {
                    let _ = self.nodes.send_to(frame.data, to);
                }
            }
            Route::Readers => {
                let Ok(readers) = self.readers.lock() else {
                    return;
                };
                for (_, to) in readers.iter().filter(|r| r.0 == frame.msgid) {
                    let _ = self.delivering.send_to(frame.data, to);
                }
            }
        }
    }

    /// Publishes a command a station sent to one of the command links.
    ///
    /// # Arguments
    ///
    /// * `node` - Whether it came to the nodes' link (`true`) or the
    ///            spacecraft's.
    fn command(&mut self, node: bool) {
        let link = if node { &self.nodes } else { &self.spacecraft };
        let Ok((size, from)) = link.recv_from(&mut self.room) else {
            return;
        };
        if node {
            self.nodes_client = Some(from);
        } else {
            self.spacecraft_client = Some(from);
        }
        if !self.verified.load(Ordering::SeqCst) {
            eprintln!(
                "tcslib: a command was not sent: the SBN peer at {} is not \
                 connected",
                self.peer_at
            );
            return;
        }
        let Ok(command) = Command::read(&self.room[..size]) else {
            return;
        };
        let msgid = if node {
            let Some(id) = command.node_id() else {
                return;
            };
            match self
                .node_commands
                .iter()
                .find(|(at, _)| *at as u64 == u64::from(id.0))
            {
                Some((_, msgid)) => *msgid,
                None => {
                    eprintln!(
                        "tcslib: a command about node {} was not sent: the \
                         configuration has no node of that id",
                        id.0
                    );
                    return;
                }
            }
        } else {
            self.spacecraft_commands
        };
        let Some(length) = cfs::write_command(
            &mut self.out[sbn::HEADER_BYTES..],
            msgid,
            self.sequence,
            self.us.spacecraft_id,
            command.cmd_type().to_u8(),
            &self.room[..size],
        ) else {
            return;
        };
        self.sequence = self.sequence.wrapping_add(1);
        self.say(kind::APPLICATION, length);
    }

    /// Subscribes to every message ID the vehicle publishes.
    fn subscribe(&mut self) {
        let Some(length) = sbn::write_subscriptions(
            &mut self.out[sbn::HEADER_BYTES..],
            &self.ident,
            self.routes.iter().map(|r| r.0),
        ) else {
            return;
        };
        self.say(kind::SUBSCRIBE, length);
    }

    /// Refuses the far end: tells it so, and disconnects.
    fn refuse(&mut self) {
        // Disconnected before the far end is told, so that nothing it does
        // on hearing so finds this side still saying it is verified.
        self.disconnect();
        self.say(kind::DISCONNECT, 0);
    }

    /// Disconnects on this side, and announces again.
    fn disconnect(&mut self) {
        self.verified.store(false, Ordering::SeqCst);
        self.liveness.disconnect();
    }

    /// Sends what is in `out` after its header, a message of `length`
    /// bytes of this type.
    ///
    /// # Arguments
    ///
    /// * `what`   - The SBN message type, one of [`kind`]'s.
    /// * `length` - How many bytes of `out`, after the header, the message is.
    fn say(&mut self, what: u8, length: usize) {
        self.us.header(what, &mut self.out, length);
        if self
            .socket
            .send_to(&self.out[..sbn::HEADER_BYTES + length], self.peer)
            .is_ok()
        {
            self.liveness.sent(Instant::now());
        }
    }
}

/// One peer of the simulated end, as it keeps it.
struct Kept {
    /// Where it is, once heard from.
    at: Option<SocketAddr>,
    liveness: Liveness,
    /// Whether it has said its protocol version is the release's.
    verified: bool,
    /// The message IDs it has subscribed to, a bit each.
    wanted: Box<[u64]>,
}

impl Kept {
    /// A peer not yet heard from.
    ///
    /// # Arguments
    ///
    /// * `now` - The time, which its silence is counted from.
    ///
    /// # Returns
    ///
    /// The peer, not connected and subscribed to nothing.
    fn new(now: Instant) -> Kept {
        Kept {
            at: None,
            liveness: Liveness::new(now),
            verified: false,
            wanted: vec![0u64; WORDS].into_boxed_slice(),
        }
    }

    fn wants(&self, msgid: u16) -> bool {
        let at = usize::from(msgid);
        self.wanted[at / 64] & (1 << (at % 64)) != 0
    }

    /// Notes the peer's subscribing to `msgid`, or unsubscribing.
    ///
    /// # Arguments
    ///
    /// * `msgid`  - The message ID.
    /// * `wanted` - Whether it subscribed (`true`) or unsubscribed.
    fn want(&mut self, msgid: u16, wanted: bool) {
        let at = usize::from(msgid);
        if wanted {
            self.wanted[at / 64] |= 1 << (at % 64);
        } else {
            self.wanted[at / 64] &= !(1 << (at % 64));
        }
    }

    /// Forgets everything the peer said: it says it all again when it
    /// connects again.
    fn forget(&mut self) {
        self.verified = false;
        self.liveness.disconnect();
        self.wanted.fill(0);
    }
}

/// Which of the simulated end's peers a datagram is from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Who {
    Vehicle,
    Station(usize),
}

/// The cFS end of the bus, simulated: see the module's documentation.
///
/// Stopped, and its thread joined, when it is dropped.
pub struct SimulatedCfs {
    at: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl SimulatedCfs {
    /// Simulates the cFS end of the bus a configuration answers over: binds
    /// `sbn.peer` and keeps it on a thread of its own.
    ///
    /// # Arguments
    ///
    /// * `graph` - The configuration's graph.
    ///
    /// # Returns
    ///
    /// The simulated end, or `None` for a configuration that does not answer
    /// over a bus.
    ///
    /// # Errors
    ///
    /// What is wrong with the `tcspecial` section or its message IDs, or a
    /// message naming `sbn.peer` where it cannot be bound -- something else
    /// holds it, a cFS or a simulated end left running.
    pub fn open(graph: &Graph) -> Result<Option<SimulatedCfs>, String> {
        let Some(bus) = bus_of(graph)? else {
            return Ok(None);
        };
        let uses = msgids(graph).map_err(|refused| refused.why)?;
        let socket =
            UdpSocket::bind(bus.sbn.peer.to_string()).map_err(|e| {
                format!(
                "the cFS end cannot be simulated at sbn.peer, {}: {e} -- \
                 is a cFS, or a simulator left running, holding it?",
                bus.sbn.peer
            )
            })?;
        let at = socket.local_addr().map_err(|e| {
            format!("the simulated cFS end's socket has no address: {e}")
        })?;
        let now = Instant::now();
        let mut hub = Hub {
            socket,
            us: Identity {
                processor_id: bus.sbn.peer_processor_id,
                spacecraft_id: bus.sbn.spacecraft_id,
            },
            vehicle_id: bus.sbn.processor_id,
            station_id: sbn::ground_processor_id(
                bus.sbn.processor_id,
                bus.sbn.peer_processor_id,
            ),
            ident: bus.sbn.ident,
            release: bus.sbn.release,
            vehicle: Kept::new(now),
            stations: (0..STATIONS_AT_ONCE).map(|_| Kept::new(now)).collect(),
            telemetry: uses
                .iter()
                .filter(|u| !u.command)
                .map(|u| u.msgid)
                .collect(),
            commands: uses
                .iter()
                .filter(|u| u.command)
                .map(|u| u.msgid)
                .collect(),
            room: vec![0u8; ROOM],
            out: vec![0u8; ROOM],
        };
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let thread = std::thread::Builder::new()
            .name("sbn-simulated".to_string())
            .spawn(move || hub.run(&stopping))
            .map_err(|e| {
                format!("the simulated cFS end cannot be started: {e}")
            })?;
        Ok(Some(SimulatedCfs {
            at,
            stop,
            thread: Some(thread),
        }))
    }

    /// Where it is bound.
    pub fn local_addr(&self) -> SocketAddr {
        self.at
    }
}

impl Drop for SimulatedCfs {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The simulated end, as its thread keeps it.
struct Hub {
    socket: UdpSocket,
    us: Identity,
    vehicle_id: u32,
    station_id: u32,
    ident: [u8; sbn::IDENT_BYTES],
    release: sbn::Release,
    vehicle: Kept,
    stations: Vec<Kept>,
    /// What it subscribes to on the vehicle: everything the vehicle
    /// publishes, as a cFS whose ground system takes all of it would.
    telemetry: Vec<u16>,
    /// And what it says a station's bus wants: every command message ID.
    commands: Vec<u16>,
    room: Vec<u8>,
    out: Vec<u8>,
}

impl Hub {
    /// Keeps the bus until told to stop.
    ///
    /// # Arguments
    ///
    /// * `stop` - Set when the simulated end is to stop.
    fn run(&mut self, stop: &AtomicBool) {
        let mut fds = [libc::pollfd {
            fd: self.socket.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        }];
        while !stop.load(Ordering::SeqCst) {
            self.tick(Instant::now());
            wait_on(&mut fds, TICK);
            if fds[0].revents & libc::POLLIN != 0 {
                self.take(Instant::now());
            }
        }
    }

    /// The peer `who` names.
    ///
    /// # Arguments
    ///
    /// * `who` - Which peer.
    ///
    /// # Returns
    ///
    /// The peer, to change.
    fn kept(&mut self, who: Who) -> &mut Kept {
        match who {
            Who::Vehicle => &mut self.vehicle,
            Who::Station(at) => &mut self.stations[at],
        }
    }

    /// Sends heartbeats, and disconnects peers silent too long. The simulated
    /// end never announces: SBN on a cFS does not speak first to a peer on its
    /// own spacecraft.
    ///
    /// # Arguments
    ///
    /// * `now` - The time.
    fn tick(&mut self, now: Instant) {
        let every = std::iter::once(Who::Vehicle)
            .chain((0..STATIONS_AT_ONCE).map(Who::Station));
        for who in every {
            let kept = self.kept(who);
            if !kept.liveness.is_connected() {
                continue;
            }
            let due = kept.liveness.due(now);
            match due {
                Due::Heartbeat => self.say(who, kind::HEARTBEAT, 0),
                Due::Disconnect => {
                    eprintln!(
                        "tcssim: the cFS end's {} has been silent for {:?}: \
                         disconnected",
                        Self::named(who),
                        sbn::SILENCE
                    );
                    self.forget(who);
                }
                _ => {}
            }
        }
    }

    /// Forgets a peer: a station's slot is then free for another.
    ///
    /// # Arguments
    ///
    /// * `who` - Which peer.
    fn forget(&mut self, who: Who) {
        let kept = self.kept(who);
        kept.forget();
        if who != Who::Vehicle {
            kept.at = None;
        }
    }

    /// Names a peer, for a line said about it.
    ///
    /// # Arguments
    ///
    /// * `who` - Which peer.
    ///
    /// # Returns
    ///
    /// What it is, in a word or two.
    fn named(who: Who) -> &'static str {
        match who {
            Who::Vehicle => "vehicle",
            Who::Station(_) => "ground station",
        }
    }

    /// Takes one datagram.
    ///
    /// # Arguments
    ///
    /// * `now` - When it arrived.
    fn take(&mut self, now: Instant) {
        let Ok((size, from)) = self.socket.recv_from(&mut self.room) else {
            return;
        };
        let Ok((header, _)) = Header::read(&self.room[..size]) else {
            return;
        };
        if header.spacecraft_id != self.us.spacecraft_id {
            return;
        }
        let who = if header.processor_id == self.vehicle_id {
            self.vehicle.at = Some(from);
            Who::Vehicle
        } else if header.processor_id == self.station_id {
            let known = self.stations.iter().position(|s| s.at == Some(from));
            let free = || self.stations.iter().position(|s| s.at.is_none());
            let Some(at) = known.or_else(free) else {
                eprintln!(
                    "tcssim: a ground station at {from} is not taken: the \
                     cFS end keeps {STATIONS_AT_ONCE} at once"
                );
                return;
            };
            self.stations[at].at = Some(from);
            Who::Station(at)
        } else {
            return;
        };

        if self.kept(who).liveness.heard(now) {
            eprintln!(
                "tcssim: the cFS end's {} at {from} is connected",
                Self::named(who)
            );
            self.out[sbn::HEADER_BYTES] = self.release.protocol_version;
            self.say(who, kind::PROTOCOL, 1);
            let wanted = match who {
                Who::Vehicle => &self.telemetry,
                Who::Station(_) => &self.commands,
            };
            if let Some(length) = sbn::write_subscriptions(
                &mut self.out[sbn::HEADER_BYTES..],
                &self.ident,
                wanted.iter().copied(),
            ) {
                self.say(who, kind::SUBSCRIBE, length);
            }
        }

        let body = sbn::HEADER_BYTES..size;
        match header.kind {
            kind::PROTOCOL => {
                if self.room[body] == [self.release.protocol_version] {
                    self.kept(who).verified = true;
                } else {
                    eprintln!(
                        "tcssim: the cFS end's {} says its protocol version \
                         is {:?}, and SBN {} speaks {}: refused",
                        Self::named(who),
                        &self.room[sbn::HEADER_BYTES..size],
                        self.release.name,
                        self.release.protocol_version
                    );
                    self.refuse(who);
                }
            }
            kind::SUBSCRIBE | kind::UNSUBSCRIBE => {
                let subscribe = header.kind == kind::SUBSCRIBE;
                let kept = match who {
                    Who::Vehicle => &mut self.vehicle,
                    Who::Station(at) => &mut self.stations[at],
                };
                let refused = match sbn::read_subscriptions(
                    &self.room[body],
                    &self.ident,
                ) {
                    Ok(msgids) => {
                        for msgid in msgids.flatten() {
                            kept.want(msgid, subscribe);
                        }
                        false
                    }
                    Err(SubscriptionRefusal::Ident(carried)) => {
                        let (mut arrived, mut expected) =
                            ([0u8; 96], [0u8; 96]);
                        eprintln!(
                            "tcssim: the cFS end's {} subscribed with the \
                             ident \"{}\", and sbn.ident says \"{}\": refused",
                            Self::named(who),
                            sbn::ident_text(carried, &mut arrived),
                            sbn::ident_text(&self.ident, &mut expected)
                        );
                        true
                    }
                    Err(SubscriptionRefusal::Malformed(_)) => false,
                };
                if refused {
                    self.refuse(who);
                }
            }
            kind::APPLICATION => self.application(who, body),
            kind::DISCONNECT => {
                eprintln!(
                    "tcssim: the cFS end's {} disconnected",
                    Self::named(who)
                );
                self.forget(who);
            }
            _ => {}
        }
    }

    /// Passes a packet on: from the vehicle to each station that subscribed
    /// to it, and from a station to the vehicle where it subscribed to it.
    ///
    /// # Arguments
    ///
    /// * `who`  - Which peer it came from.
    /// * `body` - Where the message, after SBN's header, is in the room.
    fn application(&mut self, who: Who, body: std::ops::Range<usize>) {
        if !self.kept(who).verified {
            return;
        }
        let length = body.len();
        let to_vehicle = who != Who::Vehicle;
        let msgid = if to_vehicle {
            match cfs::read_command(&self.room[body.clone()]) {
                Ok(frame) => frame.msgid,
                Err(_) => return,
            }
        } else {
            match cfs::read_telemetry(&self.room[body.clone()]) {
                Ok(frame) => frame.msgid,
                Err(_) => return,
            }
        };
        self.out[sbn::HEADER_BYTES..sbn::HEADER_BYTES + length]
            .copy_from_slice(&self.room[body]);
        if to_vehicle {
            if self.vehicle.verified && self.vehicle.wants(msgid) {
                self.say(Who::Vehicle, kind::APPLICATION, length);
            }
            return;
        }
        for at in 0..STATIONS_AT_ONCE {
            let station = &self.stations[at];
            if station.verified && station.wants(msgid) {
                self.say(Who::Station(at), kind::APPLICATION, length);
            }
        }
    }

    /// Refuses a peer: tells it so, and forgets it.
    fn refuse(&mut self, who: Who) {
        // Forgotten before it is told, as the station's peer refuses; its
        // address is kept for the telling.
        self.kept(who).forget();
        self.say(who, kind::DISCONNECT, 0);
    }

    /// Sends what is in `out` after its header to a peer, a message of
    /// `length` bytes of this type.
    ///
    /// # Arguments
    ///
    /// * `who`    - Which peer it goes to.
    /// * `what`   - The SBN message type, one of [`kind`]'s.
    /// * `length` - How many bytes of `out`, after the header, the message is.
    fn say(&mut self, who: Who, what: u8, length: usize) {
        let Some(to) = self.kept(who).at else {
            return;
        };
        self.us.header(what, &mut self.out, length);
        if self
            .socket
            .send_to(&self.out[..sbn::HEADER_BYTES + length], to)
            .is_ok()
        {
            self.kept(who).liveness.sent(Instant::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "3b339a5e0f686cef822a5d6ea95004222ab7af33";

    /// The cfs example, its far end moved to a port nothing holds.
    ///
    /// # Returns
    ///
    /// The directory it is in, and its graph.
    fn example() -> (tempfile::TempDir, Graph) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let text = std::fs::read_to_string(root.join("examples/cfs/cfs.ytcs"))
            .expect("the example reads");
        let port = loopback("test").unwrap().local_addr().unwrap().port();
        let text = text.replace("127.0.0.1:2234", &format!("127.0.0.1:{port}"));
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("cfs.ytcs");
        std::fs::write(&path, text).expect("written");
        let graph = Graph::load(&path).expect("it reads");
        (dir, graph)
    }

    /// A peer of the simulated end, played by the test.
    struct Played {
        socket: UdpSocket,
        processor_id: u32,
        to: SocketAddr,
    }

    impl Played {
        /// A peer of this processor ID, speaking to the simulated end at `to`.
        ///
        /// # Arguments
        ///
        /// * `processor_id` - The processor ID it says it is.
        /// * `to`           - Where the simulated end is.
        ///
        /// # Returns
        ///
        /// The peer.
        fn new(processor_id: u32, to: SocketAddr) -> Played {
            let socket = loopback("test").unwrap();
            socket
                .set_read_timeout(Some(Duration::from_millis(100)))
                .unwrap();
            Played {
                socket,
                processor_id,
                to,
            }
        }

        /// Sends a message of `what` type.
        ///
        /// # Arguments
        ///
        /// * `what`    - The SBN message type.
        /// * `message` - The message, after SBN's header.
        fn say(&self, what: u8, message: &[u8]) {
            let mut room = vec![0u8; sbn::HEADER_BYTES + message.len()];
            let size = Header {
                kind: what,
                processor_id: self.processor_id,
                spacecraft_id: 0x42,
            }
            .write(&mut room, message)
            .unwrap();
            self.socket.send_to(&room[..size], self.to).unwrap();
        }

        /// Subscribes to `msgids` with the ident of `hash`.
        ///
        /// # Arguments
        ///
        /// * `hash`   - The hash its ident is made from.
        /// * `msgids` - What it subscribes to.
        fn subscribe(&self, hash: &str, msgids: &[u16]) {
            let mut room = [0u8; 256];
            let size = sbn::write_subscriptions(
                &mut room,
                &sbn::ident_of(hash).unwrap(),
                msgids.iter().copied(),
            )
            .unwrap();
            self.say(kind::SUBSCRIBE, &room[..size]);
        }

        /// Joins: says something, then its protocol version and what it
        /// subscribes to.
        fn join(&self, hash: &str, msgids: &[u16]) {
            self.say(kind::HEARTBEAT, &[]);
            self.say(kind::PROTOCOL, &[11]);
            self.subscribe(hash, msgids);
        }

        /// Waits up to two seconds for a message of `what` type.
        ///
        /// # Arguments
        ///
        /// * `what` - The SBN message type waited for.
        ///
        /// # Returns
        ///
        /// The message, after SBN's header, or `None` where none came.
        fn until(&self, what: u8) -> Option<Vec<u8>> {
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut room = vec![0u8; ROOM];
            while Instant::now() < deadline {
                let Ok(size) = self.socket.recv(&mut room) else {
                    continue;
                };
                let (header, message) = Header::read(&room[..size]).unwrap();
                assert_eq!(header.processor_id, 1, "the far end's ID");
                if header.kind == what {
                    return Some(message.to_vec());
                }
            }
            None
        }
    }

    /// One peer for each bus in a process, however often it is asked for.
    #[test]
    fn one_peer_for_each_bus() {
        let (_dir, graph) = example();
        let first = Ground::of(&graph).unwrap().expect("a bus");
        let again = Ground::of(&graph).unwrap().expect("a bus");
        assert!(Arc::ptr_eq(&first, &again));
    }

    /// What the vehicle publishes reaches a station that subscribed to it,
    /// and what a station publishes reaches the vehicle that subscribed to
    /// it; a station of another ident is refused.
    #[test]
    fn the_simulated_end_passes_on_what_was_subscribed_to() {
        let (_dir, graph) = example();
        let cfs = SimulatedCfs::open(&graph).unwrap().expect("a bus");
        let at = cfs.local_addr();
        let vehicle = Played::new(2, at);
        let station = Played::new(3, at);

        vehicle.join(HASH, &[0x4280]);
        assert_eq!(vehicle.until(kind::PROTOCOL), Some(vec![11]));
        let wanted = vehicle.until(kind::SUBSCRIBE).expect("subscribed");
        let ident = sbn::ident_of(HASH).unwrap();
        let wanted: Vec<_> = sbn::read_subscriptions(&wanted, &ident)
            .unwrap()
            .flatten()
            .collect();
        assert!(wanted.contains(&0x4201), "the beacon: {wanted:?}");
        station.join(HASH, &[0x4201]);
        assert!(station.until(kind::SUBSCRIBE).is_some());

        let mut room = [0u8; 64];
        let size =
            cfs::write_telemetry(&mut room, 0x4201, 0, 0x42, [0; 6], b"b")
                .unwrap();
        vehicle.say(kind::APPLICATION, &room[..size]);
        assert_eq!(
            station.until(kind::APPLICATION).as_deref(),
            Some(&room[..size])
        );

        let size =
            cfs::write_command(&mut room, 0x4280, 0, 0x42, 1, b"c").unwrap();
        station.say(kind::APPLICATION, &room[..size]);
        assert_eq!(
            vehicle.until(kind::APPLICATION).as_deref(),
            Some(&room[..size])
        );

        let stranger = Played::new(3, at);
        stranger.say(kind::HEARTBEAT, &[]);
        stranger.subscribe(&"1".repeat(40), &[0x4201]);
        assert!(stranger.until(kind::DISCONNECT).is_some());
    }
}
