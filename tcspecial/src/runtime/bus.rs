//! The cFS software bus, as the vehicle is on it: one SBN peer that every
//! link is carried over.
//!
//! A vehicle that answers to cFS has no command socket, no beacon socket and
//! no telemetry sockets of its own. Everything it takes and sends crosses one
//! UDP socket to one SBN peer, as cFS packets told apart by message ID: see
//! "cFS: gateway_cfs" in `docs/design.rst`, [`tcslibgs::sbn`] for the wire,
//! and [`tcslibgs::cfs`] for the packets on it.
//!
//! # Who does what
//!
//! [`Bus`] is the socket and what every thread needs to know about the peer:
//! whether it is connected and has said its protocol version is the
//! release's, and which message IDs it has subscribed to. It is cheap to
//! clone and is shared.
//!
//! [`BusReader`] is the one reader of the socket, and keeps the peer: it
//! announces, sends heartbeats, disconnects on silence, refuses a peer of
//! another release or ident, and hands on what arrives -- a command to the
//! command interpreter, whose thread it runs on, and what a `gateway_cfs`
//! node sends on to that node's [`Inbox`].
//!
//! [`Publisher`] is how anything else sends: the answers, the beacon, and
//! each link to the ground. Each has room of its own made when it is, so
//! that publishing allocates nothing and no two threads share a buffer.
//!
//! Only what the peer has subscribed to is published, as SBN itself sends a
//! peer only what it asked for, and nothing at all until the peer has been
//! heard and its protocol version checked.
//!
//! # What allocates
//!
//! Opening the bus, and making a publisher, which are done at startup. Once
//! the vehicle runs, reading, publishing and keeping the peer use the room
//! each was made with; what allocates is a line logged about something
//! refused or the peer's coming and going, where the log formats it, as the
//! UDP command links' lines do.

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::os::fd::{AsRawFd, RawFd};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use log::{error, info, warn};
use tcslibgs::graph::telemetry::{Carries, MsgIdUse};
use tcslibgs::sbn::{self, kind, Due, Header, Liveness, SubscriptionRefusal};
use tcslibgs::{came_from, cfs, CommandLink, HostPort};

use crate::runtime::fabric::{Arriving, Port, Taken};

/// How many words the subscription bit set takes: one bit per message ID.
const WORDS: usize = (u16::MAX as usize + 1) / 64;

/// How many datagrams a `gateway_cfs` node's inbox holds before what
/// arrives is dropped: enough for every pass to have taken what came since
/// the last at any rate a flight computer commands at, and few enough that
/// a node nothing reads does not hold on to commands for long.
const INBOX_SLOTS: usize = 16;

/// The longest a datagram from the peer may be: the longest cFS packet the
/// bus carries, and SBN's header.
fn datagram_bytes(max_message: usize) -> usize {
    sbn::HEADER_BYTES + max_message
}

/// What every holder of the bus shares.
struct Shared {
    socket: UdpSocket,
    /// The peer, resolved once.
    peer: SocketAddr,
    /// And as the configuration states it, for `came_from` and for saying
    /// where it is.
    peer_at: HostPort,
    /// tcspecial's own SBN identity, which every datagram it sends carries.
    processor_id: u32,
    spacecraft_id: u32,
    /// The peer's processor ID, which every datagram from it carries.
    peer_processor_id: u32,
    ident: [u8; sbn::IDENT_BYTES],
    release: sbn::Release,
    /// The most a cFS packet may be on the bus.
    max_message: usize,
    cfe_time: cfs::CfeTime,
    /// Whether the peer is connected and has said its protocol version is
    /// the release's: nothing is published until it has.
    verified: AtomicBool,
    /// The message IDs the peer has subscribed to, a bit each.
    wanted: Box<[AtomicU64]>,
    /// When the bus was opened, which `last_sent` counts from.
    opened: Instant,
    /// When anything was last sent to the peer, in microseconds since
    /// `opened`, by whichever thread sent it.
    last_sent: AtomicU64,
    /// How many datagrams from the peer, or packets on it, were not taken.
    malformed: Arc<AtomicU64>,
}

impl Shared {
    /// Whether the peer has subscribed to `msgid`.
    fn wants(&self, msgid: u16) -> bool {
        let at = usize::from(msgid);
        self.wanted[at / 64].load(Ordering::Relaxed) & (1 << (at % 64)) != 0
    }

    /// Notes the peer's subscribing to `msgid`, or unsubscribing.
    fn want(&self, msgid: u16, wanted: bool) {
        let at = usize::from(msgid);
        let bit = 1u64 << (at % 64);
        if wanted {
            self.wanted[at / 64].fetch_or(bit, Ordering::Relaxed);
        } else {
            self.wanted[at / 64].fetch_and(!bit, Ordering::Relaxed);
        }
    }

    /// Forgets every subscription and the protocol version: the peer is
    /// disconnected, and says both again when it connects again.
    fn forget(&self) {
        self.verified.store(false, Ordering::SeqCst);
        for word in self.wanted.iter() {
            word.store(0, Ordering::Relaxed);
        }
    }

    /// Sends a datagram to the peer, noting when for the heartbeat.
    fn send(&self, datagram: &[u8]) -> std::io::Result<()> {
        self.socket.send_to(datagram, self.peer)?;
        let since = self.opened.elapsed().as_micros();
        self.last_sent.fetch_max(
            u64::try_from(since).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        Ok(())
    }

    /// When anything was last sent, by any thread.
    fn last_sent(&self) -> Instant {
        self.opened
            + Duration::from_micros(self.last_sent.load(Ordering::Relaxed))
    }

    /// Counts one more thing not taken, and says how many there have been.
    fn malformed(&self) -> u64 {
        self.malformed.fetch_add(1, Ordering::Relaxed) + 1
    }
}

/// The bus, shared.
#[derive(Clone)]
pub struct Bus {
    shared: Arc<Shared>,
    /// Each `gateway_cfs` node's inbox, by name.
    inboxes: Arc<Vec<(String, Arc<Inbox>)>>,
}

impl std::fmt::Debug for Bus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the cFS software bus through {}", self.shared.peer_at)
    }
}

impl Bus {
    /// Opens the bus a configuration states: binds `sbn.local` and resolves
    /// `sbn.peer`, and works out what is subscribed to and where each
    /// command goes.
    ///
    /// # Arguments
    ///
    /// * `bus`  - The section's settled links on the bus.
    /// * `used` - Every message ID the configuration uses:
    ///            [`tcslibgs::graph::telemetry::msgids`].
    ///
    /// # Returns
    ///
    /// The bus, and its one reader.
    ///
    /// # Errors
    ///
    /// A message naming the attribute to change, where `sbn.local` will not
    /// bind or `sbn.peer` does not resolve.
    pub fn open(
        bus: &tcslibgs::Bus,
        used: &[MsgIdUse],
    ) -> Result<(Bus, BusReader), String> {
        let sbn = &bus.sbn;
        let socket = UdpSocket::bind(sbn.local.to_string()).map_err(|e| {
            format!(
                "the SBN peer cannot be bound at {} ({e}): change sbn.local \
                 to an address this machine has and a port nothing else is \
                 using -- a vehicle with no bus is one nobody can command",
                sbn.local
            )
        })?;
        socket
            .set_nonblocking(true)
            .map_err(|e| format!("the SBN peer's socket: {e}"))?;
        let unresolved = || {
            format!(
                "sbn.peer is {}, which does not resolve to an address: give \
                 the address and port the cFS side's SBN listens on",
                sbn.peer
            )
        };
        let peer = (sbn.peer.address.as_str(), sbn.peer.port)
            .to_socket_addrs()
            .map_err(|_| unresolved())?
            .next()
            .ok_or_else(unresolved)?;

        let shared = Arc::new(Shared {
            socket,
            peer,
            peer_at: sbn.peer.clone(),
            processor_id: sbn.processor_id,
            spacecraft_id: sbn.spacecraft_id,
            peer_processor_id: sbn.peer_processor_id,
            ident: sbn.ident,
            release: sbn.release,
            max_message: sbn.max_message as usize,
            cfe_time: bus.cfe_time,
            verified: AtomicBool::new(false),
            wanted: (0..WORDS).map(|_| AtomicU64::new(0)).collect(),
            opened: Instant::now(),
            last_sent: AtomicU64::new(0),
            malformed: Arc::new(AtomicU64::new(0)),
        });

        let mut inboxes = Vec::new();
        let mut routes = Vec::new();
        let mut subscriptions = Vec::new();
        let mut spacecraft_answers = bus.response_msgid;
        let mut node_answers = Vec::new();
        for one in used {
            match &one.carries {
                Carries::SpacecraftCommands => {
                    routes.push((one.msgid, Route::Spacecraft))
                }
                Carries::NodeCommands(id) => {
                    routes.push((one.msgid, Route::Node(*id)))
                }
                Carries::Gateway(name) => {
                    routes.push((one.msgid, Route::Gateway(inboxes.len())));
                    inboxes.push((name.clone(), Arc::new(Inbox::new(sbn))));
                }
                Carries::SpacecraftAnswers => spacecraft_answers = one.msgid,
                Carries::NodeAnswers(id) => {
                    if node_answers.len() <= *id {
                        node_answers.resize(*id + 1, 0);
                    }
                    node_answers[*id] = one.msgid;
                }
                Carries::Beacon | Carries::Telemetry(_) => {}
            }
            if one.command {
                subscriptions.push(one.msgid);
            }
        }
        // Looked up by binary search for every command, which allocates
        // nothing.
        routes.sort_by_key(|(msgid, _)| *msgid);

        let room = datagram_bytes(shared.max_message);
        let reader = BusReader {
            shared: shared.clone(),
            liveness: Liveness::new(Instant::now()),
            room: vec![0; room],
            out: vec![0; room],
            routes,
            subscriptions,
            inboxes: inboxes.iter().map(|(_, inbox)| inbox.clone()).collect(),
            spacecraft_answers,
            node_answers,
        };
        Ok((
            Bus {
                shared,
                inboxes: Arc::new(inboxes),
            },
            reader,
        ))
    }

    /// Makes something to publish with, its room its own.
    ///
    /// Allocates the room, so it is made at startup by whatever will
    /// publish, never in a loop.
    pub fn publisher(&self) -> Publisher {
        Publisher {
            shared: self.shared.clone(),
            room: vec![0; datagram_bytes(self.shared.max_message)],
            sequence: 0,
        }
    }

    /// The inbox a `gateway_cfs` node takes what the bus sends it from.
    ///
    /// # Arguments
    ///
    /// * `node` - The node's name.
    ///
    /// # Returns
    ///
    /// Its inbox, or `None` for a node the bus sends nothing to: one that
    /// states no `cmd_msgid`.
    pub fn inbox(&self, node: &str) -> Option<Arc<Inbox>> {
        self.inboxes
            .iter()
            .find(|(name, _)| name == node)
            .map(|(_, inbox)| inbox.clone())
    }

    /// The counter of what the bus has not taken, which the command
    /// interpreter counts its own refusals into as well.
    pub fn malformed(&self) -> Arc<AtomicU64> {
        self.shared.malformed.clone()
    }

    /// Whether the peer is connected and has said its protocol version is
    /// the release's.
    pub fn is_verified(&self) -> bool {
        self.shared.verified.load(Ordering::SeqCst)
    }

    /// Where the bus is bound, as the socket says.
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.shared.socket.local_addr()
    }
}

/// How something publishes on the bus: room of its own, and a packet
/// sequence count of its own.
pub struct Publisher {
    shared: Arc<Shared>,
    room: Vec<u8>,
    sequence: u16,
}

impl Publisher {
    /// Publishes `data` as a telemetry packet under `msgid`, stamped with
    /// the time now in cFE's time.
    ///
    /// Allocates nothing where it is sent, or not sent because the peer is
    /// not connected or has not subscribed to `msgid`.
    ///
    /// # Arguments
    ///
    /// * `msgid` - The message ID.
    /// * `data`  - What the packet carries.
    ///
    /// # Returns
    ///
    /// Whether it was sent: `false` where the peer is not connected, has not
    /// said its protocol version, or has not subscribed to `msgid`.
    ///
    /// # Errors
    ///
    /// A message where `data` is too long for the bus, which the ground
    /// check refuses a configuration for, or the send fails.
    pub fn publish(&mut self, msgid: u16, data: &[u8]) -> Result<bool, String> {
        self.publish_at(msgid, data, SystemTime::now())
    }

    /// Publishes as [`Publisher::publish`] does, stamped with `at`.
    pub fn publish_at(
        &mut self,
        msgid: u16,
        data: &[u8],
        at: SystemTime,
    ) -> Result<bool, String> {
        let shared = &self.shared;
        if cfs::TELEMETRY_HEADER_BYTES + data.len() > shared.max_message {
            // Allocates, once per datum refused: the ground check refuses a
            // configuration whose sources can produce this, so it is a fault
            // and not a rate.
            return Err(format!(
                "{} bytes with a cFS packet's headers is more than \
                 sbn.max_message, {}",
                cfs::TELEMETRY_HEADER_BYTES + data.len(),
                shared.max_message
            ));
        }
        if !shared.verified.load(Ordering::SeqCst) || !shared.wants(msgid) {
            return Ok(false);
        }
        let time = shared.cfe_time.stamp(at);
        let Some(length) = cfs::write_telemetry(
            &mut self.room[sbn::HEADER_BYTES..],
            msgid,
            self.sequence,
            shared.spacecraft_id,
            time,
            data,
        ) else {
            return Err("too long for the bus".to_string());
        };
        self.sequence = self.sequence.wrapping_add(1);
        Header {
            kind: kind::APPLICATION,
            processor_id: shared.processor_id,
            spacecraft_id: shared.spacecraft_id,
        }
        .write_before(&mut self.room, length);
        shared
            .send(&self.room[..sbn::HEADER_BYTES + length])
            .map_err(|e| {
                format!("cannot be sent to {}: {e}", shared.peer_at)
            })?;
        Ok(true)
    }
}

/// Where a command arriving on the bus goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    /// To the command interpreter, as a command about the spacecraft.
    Spacecraft,
    /// To it, as a command about the node of this device id.
    Node(usize),
    /// To this `gateway_cfs` node's inbox.
    Gateway(usize),
}

/// A command the bus has taken, for the command interpreter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrival {
    /// Which of the interpreter's links it is on.
    pub link: CommandLink,
    /// The node its message ID is for, where it is a node command's.
    pub node: Option<usize>,
    /// Its function code.
    pub function_code: u8,
    /// Where its data is in the reader's room.
    start: usize,
    end: usize,
}

/// What [`BusReader::take`] found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Took {
    /// Nothing was waiting.
    Empty,
    /// A datagram, which was dealt with: a protocol message, what was not
    /// taken, or what a `gateway_cfs` node was sent.
    Done,
    /// A command, for the interpreter.
    Command(Arrival),
}

/// The one reader of the bus, which keeps the peer.
pub struct BusReader {
    shared: Arc<Shared>,
    liveness: Liveness,
    /// What a datagram is read into, made once.
    room: Vec<u8>,
    /// What protocol messages are written into, made once.
    out: Vec<u8>,
    /// Where each command message ID goes, sorted by message ID.
    routes: Vec<(u16, Route)>,
    /// Every command message ID, which is what tcspecial subscribes to.
    subscriptions: Vec<u16>,
    inboxes: Vec<Arc<Inbox>>,
    /// Where answers are published: the spacecraft's, and each node's by
    /// device id.
    spacecraft_answers: u16,
    node_answers: Vec<u16>,
}

impl BusReader {
    /// The socket's descriptor, for a poll.
    pub fn fd(&self) -> RawFd {
        self.shared.socket.as_raw_fd()
    }

    /// How long until the peer is due an announcement, a heartbeat or a
    /// disconnect, for a poll to time out at.
    ///
    /// # Arguments
    ///
    /// * `now` - The time.
    pub fn until_due(&mut self, now: Instant) -> Duration {
        self.liveness.sent(self.shared.last_sent());
        self.liveness.until_due(now)
    }

    /// Does whatever the clock says is due: announces to a peer not
    /// connected, sends a heartbeat to one sent nothing for five seconds,
    /// and disconnects one silent for ten.
    ///
    /// # Arguments
    ///
    /// * `now` - The time.
    pub fn tick(&mut self, now: Instant) {
        self.liveness.sent(self.shared.last_sent());
        match self.liveness.due(now) {
            Due::Nothing => {}
            Due::Announce => {
                self.say(kind::ANNOUNCE, 0);
                self.liveness.announced(now);
            }
            Due::Heartbeat => {
                self.say(kind::HEARTBEAT, 0);
                self.liveness.sent(now);
            }
            Due::Disconnect => {
                warn!(
                    "the SBN peer at {} has been silent for {} s: \
                     disconnected, and announced to again",
                    self.shared.peer_at,
                    sbn::SILENCE.as_secs()
                );
                self.disconnect();
            }
        }
    }

    /// Takes one datagram from the peer, if one is waiting, and does what
    /// it says: keeps the peer, notes its subscriptions, puts what a
    /// `gateway_cfs` node is sent in its inbox, or hands back a command.
    ///
    /// What is not taken -- from anywhere but the peer, not SBN's, not one
    /// whole cFS command, a command whose checksum is wrong or under a
    /// message ID tcspecial does not take -- is logged and counted as
    /// malformed, and nothing in it is acted on.
    ///
    /// # Arguments
    ///
    /// * `now` - The time.
    ///
    /// # Returns
    ///
    /// A command for the interpreter, where one arrived; otherwise whether
    /// anything was waiting.
    pub fn take(&mut self, now: Instant) -> Took {
        let (size, from) = match self.shared.socket.recv_from(&mut self.room) {
            Ok(got) => got,
            Err(ref e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::Interrupted
                ) =>
            {
                return Took::Empty
            }
            Err(e) => {
                error!("the SBN peer's socket cannot be read: {e}");
                return Took::Empty;
            }
        };
        let shared = self.shared.clone();
        if !came_from(&shared.peer_at, &from) {
            let count = shared.malformed();
            warn!(
                "{size} bytes came to the SBN socket from {from} and not from \
                 the peer, {}: not taken, and counted as malformed ({count})",
                shared.peer_at
            );
            return Took::Done;
        }
        let (header, _) = match Header::read(&self.room[..size]) {
            Ok(read) => read,
            Err(why) => {
                let count = shared.malformed();
                warn!(
                    "{size} bytes from the SBN peer are {why}: not taken, and \
                     counted as malformed ({count})"
                );
                return Took::Done;
            }
        };
        if header.processor_id != shared.peer_processor_id
            || header.spacecraft_id != shared.spacecraft_id
        {
            let count = shared.malformed();
            warn!(
                "an SBN message from {from} says it is from processor {} on \
                 spacecraft {}, and the peer is processor {} on spacecraft \
                 {}: not taken, and counted as malformed ({count}) -- check \
                 sbn.peer_processor_id and sbn.spacecraft_id against the far \
                 end's SBN table",
                header.processor_id,
                header.spacecraft_id,
                shared.peer_processor_id,
                shared.spacecraft_id
            );
            return Took::Done;
        }

        if self.liveness.heard(now) {
            info!(
                "the SBN peer at {} is connected: sending it the protocol \
                 version and tcspecial's subscriptions",
                shared.peer_at
            );
            self.say_protocol();
            self.subscribe();
        }

        let body = sbn::HEADER_BYTES..size;
        match header.kind {
            kind::HEARTBEAT | kind::ANNOUNCE => Took::Done,
            kind::DISCONNECT => {
                info!(
                    "the SBN peer at {} disconnected: announcing to it again",
                    shared.peer_at
                );
                self.disconnect();
                Took::Done
            }
            kind::PROTOCOL => {
                self.protocol(body);
                Took::Done
            }
            kind::SUBSCRIBE | kind::UNSUBSCRIBE => {
                self.subscriptions_of(header.kind == kind::SUBSCRIBE, body);
                Took::Done
            }
            kind::APPLICATION => match self.application(body) {
                Some(arrival) => Took::Command(arrival),
                None => Took::Done,
            },
            other => {
                let count = shared.malformed();
                warn!(
                    "the SBN peer sent a message of type {other:#04x}, which \
                     SBN {}'s UDP module has none of: not taken, and counted \
                     as malformed ({count})",
                    shared.release.name
                );
                Took::Done
            }
        }
    }

    /// The data of a command [`BusReader::take`] handed back: a tcspecial
    /// command, as the UDP link carries one.
    pub fn data(&self, arrival: &Arrival) -> &[u8] {
        &self.room[arrival.start..arrival.end]
    }

    /// Where the answer to a command is published: `tcs_cmd.response_msgid`,
    /// or the node's `payload_command.response_base` plus its id.
    pub fn answer_msgid(&self, arrival: &Arrival) -> u16 {
        match arrival.node {
            Some(id) => self
                .node_answers
                .get(id)
                .copied()
                .unwrap_or(self.spacecraft_answers),
            None => self.spacecraft_answers,
        }
    }

    /// Counts one more thing not taken, and says how many there have been.
    pub fn malformed(&self) -> u64 {
        self.shared.malformed()
    }

    /// Whether the peer is connected.
    pub fn is_connected(&self) -> bool {
        self.liveness.is_connected()
    }

    /// Reads the peer's protocol version, refusing a peer of another.
    fn protocol(&mut self, body: std::ops::Range<usize>) {
        let said = &self.room[body];
        let expected = self.shared.release.protocol_version;
        if said == [expected] {
            if !self.shared.verified.swap(true, Ordering::SeqCst) {
                info!(
                    "the SBN peer speaks protocol version {expected}, SBN \
                     {}'s: publishing to it",
                    self.shared.release.name
                );
            }
            return;
        }
        error!(
            "the SBN peer at {} says its protocol version is {said:?}, and \
             SBN {} speaks [{expected}]: refused and disconnected -- build \
             the far end from SBN {}, or name the release it was built from \
             in sbn.release",
            self.shared.peer_at,
            self.shared.release.name,
            self.shared.release.name
        );
        self.refuse();
    }

    /// Notes a subscription or an unsubscription, refusing a peer whose
    /// ident is not the one the configuration states.
    fn subscriptions_of(
        &mut self,
        subscribe: bool,
        body: std::ops::Range<usize>,
    ) {
        let shared = self.shared.clone();
        let refused =
            match sbn::read_subscriptions(&self.room[body], &shared.ident) {
                Ok(msgids) => {
                    for msgid in msgids.flatten() {
                        shared.want(msgid, subscribe);
                    }
                    false
                }
                Err(SubscriptionRefusal::Malformed(why)) => {
                    let count = shared.malformed();
                    warn!(
                        "the SBN peer sent {why}: not taken, and counted as \
                     malformed ({count})"
                    );
                    false
                }
                Err(SubscriptionRefusal::Ident(carried)) => {
                    let (mut expected, mut arrived) = ([0u8; 96], [0u8; 96]);
                    error!(
                    "the SBN peer at {} subscribed with the ident \"{}\", and \
                     sbn.ident says the far end's is \"{}\": refused and \
                     disconnected -- the far end was built from another \
                     sbn_types.h, so copy the hash its start-up event prints \
                     into sbn.ident",
                    shared.peer_at,
                    sbn::ident_text(carried, &mut arrived),
                    sbn::ident_text(&shared.ident, &mut expected)
                );
                    true
                }
            };
        if refused {
            self.refuse();
        }
    }

    /// Reads an application's message: a command, for the interpreter or a
    /// `gateway_cfs` node.
    fn application(&mut self, body: std::ops::Range<usize>) -> Option<Arrival> {
        let shared = self.shared.clone();
        if !shared.verified.load(Ordering::SeqCst) {
            let count = shared.malformed();
            warn!(
                "the SBN peer sent a packet before saying its protocol \
                 version: not taken, and counted as malformed ({count})"
            );
            return None;
        }
        let offset = body.start;
        let frame = match cfs::read_command(&self.room[body]) {
            Ok(frame) => frame,
            Err(why) => {
                let count = shared.malformed();
                warn!(
                    "the SBN peer sent {why}: not answered, not acted on, and \
                     counted as malformed ({count})"
                );
                return None;
            }
        };
        let Ok(at) = self.routes.binary_search_by_key(&frame.msgid, |r| r.0)
        else {
            let count = shared.malformed();
            warn!(
                "the SBN peer sent a command under message ID {:#06x}, which \
                 tcspecial does not subscribe to: not acted on, and counted \
                 as malformed ({count})",
                frame.msgid
            );
            return None;
        };
        let start = offset + cfs::COMMAND_HEADER_BYTES;
        let end = start + frame.data.len();
        let function_code = frame.function_code;
        let (link, node) = match self.routes[at].1 {
            Route::Spacecraft => (CommandLink::Spacecraft, None),
            Route::Node(id) => (CommandLink::Payload, Some(id)),
            Route::Gateway(index) => {
                if !self.inboxes[index].put(&self.room[start..end]) {
                    let count = shared.malformed();
                    warn!(
                        "a gateway_cfs node's inbox is full, so what the bus \
                         sent it under {:#06x} is dropped ({count}): nothing \
                         has taken from it for {INBOX_SLOTS} packets",
                        frame.msgid
                    );
                }
                return None;
            }
        };
        Some(Arrival {
            link,
            node,
            function_code,
            start,
            end,
        })
    }

    /// Refuses the peer: tells it so with a disconnect, so that it connects
    /// again and says its version and subscriptions again, each time to be
    /// checked and logged.
    fn refuse(&mut self) {
        self.say(kind::DISCONNECT, 0);
        self.disconnect();
    }

    /// Disconnects the peer on this side: nothing is published, every
    /// subscription is forgotten, and it is announced to again.
    fn disconnect(&mut self) {
        self.shared.forget();
        self.liveness.disconnect();
    }

    /// Sends the protocol version.
    fn say_protocol(&mut self) {
        self.out[sbn::HEADER_BYTES] = self.shared.release.protocol_version;
        self.say(kind::PROTOCOL, 1);
    }

    /// Sends tcspecial's subscriptions: every message ID it takes commands
    /// on, opened with the far end's ident.
    fn subscribe(&mut self) {
        let Some(length) = sbn::write_subscriptions(
            &mut self.out[sbn::HEADER_BYTES..],
            &self.shared.ident,
            self.subscriptions.iter().copied(),
        ) else {
            error!(
                "tcspecial's {} subscriptions do not fit in one SBN message: \
                 none are sent",
                self.subscriptions.len()
            );
            return;
        };
        self.say(kind::SUBSCRIBE, length);
    }

    /// Sends what is in `out` after its header, a message of `length`
    /// bytes of this type.
    fn say(&mut self, what: u8, length: usize) {
        Header {
            kind: what,
            processor_id: self.shared.processor_id,
            spacecraft_id: self.shared.spacecraft_id,
        }
        .write_before(&mut self.out, length);
        if let Err(e) =
            self.shared.send(&self.out[..sbn::HEADER_BYTES + length])
        {
            // Said each time: a send to the peer failing is a bus that is
            // not there, and each line is one announcement or heartbeat
            // apart.
            warn!(
                "an SBN message of type {what:#04x} cannot be sent to {}: {e}",
                self.shared.peer_at
            );
        }
    }
}

/// What the bus has sent a `gateway_cfs` node, waiting for a pass to take it.
///
/// A fixed ring of datagrams, made when the bus is opened, so that putting
/// and taking allocate nothing.
pub struct Inbox {
    ring: Mutex<Ring>,
}

struct Ring {
    slots: Vec<(usize, Vec<u8>)>,
    first: usize,
    count: usize,
}

impl Inbox {
    fn new(sbn: &tcslibgs::Sbn) -> Inbox {
        let room = (sbn.max_message as usize).min(tcslibgs::DATUM_BYTES);
        Inbox {
            ring: Mutex::new(Ring {
                slots: (0..INBOX_SLOTS).map(|_| (0, vec![0; room])).collect(),
                first: 0,
                count: 0,
            }),
        }
    }

    /// Puts a datagram in, unless the inbox is full or it is too long.
    ///
    /// # Returns
    ///
    /// Whether it was put.
    fn put(&self, data: &[u8]) -> bool {
        let Ok(mut ring) = self.ring.lock() else {
            return false;
        };
        if ring.count == ring.slots.len() {
            return false;
        }
        let at = (ring.first + ring.count) % ring.slots.len();
        let slot = &mut ring.slots[at];
        if data.len() > slot.1.len() {
            return false;
        }
        slot.1[..data.len()].copy_from_slice(data);
        slot.0 = data.len();
        ring.count += 1;
        true
    }

    /// Takes the oldest datagram out, into `buffer`.
    ///
    /// # Returns
    ///
    /// How many bytes of `buffer` it filled, or `None` where nothing was
    /// waiting.
    fn take(&self, buffer: &mut [u8]) -> Option<usize> {
        let mut ring = self.ring.lock().ok()?;
        if ring.count == 0 {
            return None;
        }
        let first = ring.first;
        let (length, data) = &ring.slots[first];
        let length = (*length).min(buffer.len());
        buffer[..length].copy_from_slice(&data[..length]);
        ring.first = (first + 1) % ring.slots.len();
        ring.count -= 1;
        Some(length)
    }
}

/// A `gateway_cfs` node, as a node of the graph: what the bus sends it,
/// taken on each pass and sent along its lines.
pub struct BusGatewayPort {
    inbox: Arc<Inbox>,
}

impl BusGatewayPort {
    /// The node, taking from its inbox.
    pub fn of(inbox: Arc<Inbox>) -> BusGatewayPort {
        BusGatewayPort { inbox }
    }
}

impl Port for BusGatewayPort {
    fn take(&mut self, buffer: &mut [u8]) -> Result<Taken, String> {
        Ok(self.inbox.take(buffer).map_or(Taken::NOTHING, Taken::of))
    }

    /// Nothing is given a gateway node: what is drawn into one goes to the
    /// bus on a link of its own, opened beside it.
    fn give(
        &mut self,
        _arriving: Arriving<'_>,
        _data: &[u8],
    ) -> Result<(), String> {
        Ok(())
    }
}

/// One source's link to the ground over the bus: what it is given is
/// published under its message ID.
pub struct BusDownlink {
    publisher: Publisher,
    msgid: u16,
}

impl BusDownlink {
    /// The link, publishing under `msgid`.
    pub fn of(publisher: Publisher, msgid: u16) -> BusDownlink {
        BusDownlink { publisher, msgid }
    }
}

impl Port for BusDownlink {
    fn take(&mut self, _buffer: &mut [u8]) -> Result<Taken, String> {
        Ok(Taken::NOTHING)
    }

    /// Published at once, stamped with the time it is given, which is the
    /// pass the reading was taken in: the datum carries no time of its own.
    fn give(
        &mut self,
        _arriving: Arriving<'_>,
        data: &[u8],
    ) -> Result<(), String> {
        self.publisher.publish(self.msgid, data).map(|_| ())
    }

    fn produces(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeSet;
    use std::path::Path;

    use tcslibgs::graph::parameters::{ci_config_of, flatten};
    use tcslibgs::graph::Graph;

    /// The example's spacecraft, and the two processors.
    const SPACECRAFT: u32 = 0x42;
    const OURS: u32 = 2;
    const THEIRS: u32 = 1;

    /// The far end of the bus, played by the test: a socket, and what it
    /// knows of the configuration.
    struct Peer {
        socket: UdpSocket,
        /// Where tcspecial is.
        vehicle: SocketAddr,
        ident: [u8; sbn::IDENT_BYTES],
    }

    impl Peer {
        /// Sends a message of `what` type, as the peer.
        fn say(&self, what: u8, message: &[u8]) {
            self.say_as(&self.socket, THEIRS, what, message);
        }

        /// Sends a message from `socket`, as processor `processor`.
        fn say_as(
            &self,
            socket: &UdpSocket,
            processor: u32,
            what: u8,
            message: &[u8],
        ) {
            let mut room = vec![0u8; sbn::HEADER_BYTES + message.len()];
            let size = Header {
                kind: what,
                processor_id: processor,
                spacecraft_id: SPACECRAFT,
            }
            .write(&mut room, message)
            .expect("it fits");
            socket.send_to(&room[..size], self.vehicle).expect("sent");
        }

        /// Publishes a command, as the cFS side does.
        fn command(&self, msgid: u16, function_code: u8, data: &[u8]) {
            let mut room = [0u8; 512];
            let size = cfs::write_command(
                &mut room,
                msgid,
                0,
                SPACECRAFT,
                function_code,
                data,
            )
            .expect("it fits");
            self.say(kind::APPLICATION, &room[..size]);
        }

        /// Subscribes to `msgids`, with the ident `ident`.
        fn subscribe(&self, ident: &[u8; sbn::IDENT_BYTES], msgids: &[u16]) {
            let mut room = [0u8; 512];
            let size = sbn::write_subscriptions(
                &mut room,
                ident,
                msgids.iter().copied(),
            )
            .expect("it fits");
            self.say(kind::SUBSCRIBE, &room[..size]);
        }

        /// What tcspecial sends next, or `None` where nothing comes within
        /// the socket's timeout.
        fn heard(&self) -> Option<(Header, Vec<u8>)> {
            let mut room = [0u8; 65536];
            let (size, from) = self.socket.recv_from(&mut room).ok()?;
            assert_eq!(from.port(), self.vehicle.port(), "from tcspecial");
            let (header, message) =
                Header::read(&room[..size]).expect("an SBN datagram");
            assert_eq!(
                (header.processor_id, header.spacecraft_id),
                (OURS, SPACECRAFT),
                "tcspecial says who it is in every header"
            );
            Some((header, message.to_vec()))
        }

        /// What tcspecial sends next, which must be of type `what`.
        fn expect(&self, what: u8) -> Vec<u8> {
            let (header, message) =
                self.heard().unwrap_or_else(|| panic!("no {what:#04x}"));
            assert_eq!(header.kind, what, "{message:?}");
            message
        }
    }

    /// A port nothing holds, for the bus to bind.
    fn free() -> u16 {
        UdpSocket::bind("127.0.0.1:0")
            .expect("a port")
            .local_addr()
            .expect("its address")
            .port()
    }

    /// Opens the bus the cfs example states, its peer this test.
    ///
    /// # Returns
    ///
    /// The bus, its reader and the peer.
    fn opened() -> (Bus, BusReader, Peer) {
        let socket = UdpSocket::bind("127.0.0.1:0").expect("the peer's port");
        socket
            .set_read_timeout(Some(Duration::from_millis(300)))
            .expect("a timeout");
        let example = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../examples/cfs/cfs.ytcs");
        let ours = free();
        let peer_port = socket.local_addr().expect("its address").port();
        let text = std::fs::read_to_string(example)
            .expect("the example reads")
            .replace("0.0.0.0:8634", &format!("127.0.0.1:{ours}"))
            .replace("127.0.0.1:2234", &format!("127.0.0.1:{peer_port}"));
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("cfs.ytcs");
        std::fs::write(&path, text).expect("written");
        let graph = Graph::load(&path).expect("it reads");
        let ci = ci_config_of(&flatten(graph.tcspecial.as_ref().unwrap()))
            .and_then(|json| json.to_ci_config())
            .expect("the section reads");
        let used = tcslibgs::graph::telemetry::msgids(&graph).expect("sound");
        let settled = ci.bus().expect("on the bus");
        let (bus, reader) = Bus::open(settled, &used).expect("it opens");
        let peer = Peer {
            socket,
            vehicle: format!("127.0.0.1:{ours}").parse().unwrap(),
            ident: settled.sbn.ident,
        };
        (bus, reader, peer)
    }

    /// Takes one datagram, waiting for it to arrive.
    fn take(reader: &mut BusReader, now: Instant) -> Took {
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            match reader.take(now) {
                Took::Empty if Instant::now() < until => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                took => return took,
            }
        }
    }

    /// Announces, and is answered with the right protocol version, so the
    /// peer is connected and verified.
    fn connected() -> (Bus, BusReader, Peer, Instant) {
        let (bus, mut reader, peer) = opened();
        let now = Instant::now();
        reader.tick(now);
        peer.expect(kind::ANNOUNCE);
        peer.say(kind::PROTOCOL, &[11]);
        assert_eq!(take(&mut reader, now), Took::Done);
        peer.expect(kind::PROTOCOL);
        peer.expect(kind::SUBSCRIBE);
        assert!(bus.is_verified());
        (bus, reader, peer, now)
    }

    /// tcspecial announces itself until it is heard, and once heard tells
    /// the peer the release's protocol version and subscribes, with the far
    /// end's ident, to its command message IDs and nothing else.
    #[test]
    fn a_peer_is_announced_to_and_subscribed_with_once_heard() {
        let (bus, mut reader, peer) = opened();
        let now = Instant::now();
        reader.tick(now);
        assert!(peer.expect(kind::ANNOUNCE).is_empty());
        // Not again until a second has gone.
        reader.tick(now + Duration::from_millis(500));
        assert!(peer.heard().is_none());
        reader.tick(now + Duration::from_secs(1));
        peer.expect(kind::ANNOUNCE);

        peer.say(kind::PROTOCOL, &[11]);
        assert_eq!(take(&mut reader, now), Took::Done);
        assert_eq!(peer.expect(kind::PROTOCOL), vec![11]);
        let subscribed = peer.expect(kind::SUBSCRIBE);
        assert_eq!(&subscribed[..sbn::IDENT_BYTES], &peer.ident[..]);
        let msgids: BTreeSet<u16> =
            sbn::read_subscriptions(&subscribed, &peer.ident)
                .expect("its own ident")
                .flatten()
                .collect();
        // The spacecraft's, each node's from cmd_base, and fsw's cmd_msgid.
        assert_eq!(
            msgids,
            BTreeSet::from([0x4280, 0x4281, 0x4290, 0x4291, 0x4292])
        );
        assert!(bus.is_verified());
        assert!(reader.is_connected());
    }

    /// A heartbeat goes after five seconds of sending nothing, and ten of
    /// hearing nothing disconnects the peer and announces to it again --
    /// asked of the reader with the time rather than waited for.
    #[test]
    fn a_heartbeat_goes_after_five_seconds_and_silence_disconnects() {
        let (bus, mut reader, peer, now) = connected();
        reader.tick(now + Duration::from_secs(4));
        assert!(peer.heard().is_none(), "nothing due at four seconds");
        reader.tick(now + Duration::from_millis(5100));
        assert!(peer.expect(kind::HEARTBEAT).is_empty());
        reader.tick(now + Duration::from_millis(9000));
        assert!(peer.heard().is_none(), "nor a second one yet");
        assert!(bus.is_verified());

        reader.tick(now + Duration::from_millis(10_100));
        assert!(!bus.is_verified(), "silence disconnects");
        assert!(!reader.is_connected());
        reader.tick(now + Duration::from_millis(10_100));
        peer.expect(kind::ANNOUNCE);
    }

    /// A peer whose protocol version is not the release's is refused: told
    /// so with a disconnect, and nothing is published to it.
    #[test]
    fn a_peer_of_another_protocol_version_is_refused() {
        let (bus, mut reader, peer) = opened();
        let now = Instant::now();
        peer.say(kind::PROTOCOL, &[10]);
        assert_eq!(take(&mut reader, now), Took::Done);
        peer.expect(kind::PROTOCOL);
        peer.expect(kind::SUBSCRIBE);
        peer.expect(kind::DISCONNECT);
        assert!(!bus.is_verified());
        assert!(!reader.is_connected());
    }

    /// A peer whose subscriptions carry another ident is refused, and what
    /// it subscribed to is not published.
    #[test]
    fn a_peer_with_another_ident_is_refused() {
        let (bus, mut reader, peer, now) = connected();
        let other = sbn::ident_of(&"1".repeat(40)).unwrap();
        peer.subscribe(&other, &[0x4242]);
        assert_eq!(take(&mut reader, now), Took::Done);
        peer.expect(kind::DISCONNECT);
        assert!(!bus.is_verified());
        assert_eq!(bus.publisher().publish(0x4242, b"x"), Ok(false));
    }

    /// Only what the peer has subscribed to is published, framed as cFS
    /// telemetry under its message ID, and nothing once it unsubscribes.
    #[test]
    fn only_what_the_peer_subscribed_to_is_published() {
        let (bus, mut reader, peer, now) = connected();
        let mut publisher = bus.publisher();
        assert_eq!(publisher.publish(0x4242, b"293.15"), Ok(false));

        peer.subscribe(&peer.ident, &[0x4242]);
        assert_eq!(take(&mut reader, now), Took::Done);
        assert_eq!(publisher.publish(0x4242, b"293.15"), Ok(true));
        let packet = peer.expect(kind::APPLICATION);
        let frame = cfs::read_telemetry(&packet).expect("telemetry");
        assert_eq!(frame.msgid, 0x4242);
        assert_eq!(frame.data, b"293.15");
        assert_ne!(frame.time, [0; 6], "stamped with the time");
        assert_eq!(publisher.publish(0x4243, b"x"), Ok(false));

        let mut room = [0u8; 128];
        let size = sbn::write_subscriptions(
            &mut room,
            &peer.ident,
            [0x4242u16].into_iter(),
        )
        .unwrap();
        peer.say(kind::UNSUBSCRIBE, &room[..size]);
        assert_eq!(take(&mut reader, now), Took::Done);
        assert_eq!(publisher.publish(0x4242, b"293.15"), Ok(false));
    }

    /// A command is handed on by its message ID: the spacecraft's and each
    /// node's to the interpreter, with where its answer goes, and fsw's to
    /// fsw's inbox.
    #[test]
    fn commands_are_routed_by_message_id() {
        let (bus, mut reader, peer, now) = connected();
        peer.command(0x4280, 0x04, b"connect");
        let Took::Command(arrival) = take(&mut reader, now) else {
            panic!("a command")
        };
        assert_eq!(arrival.link, CommandLink::Spacecraft);
        assert_eq!(arrival.function_code, 0x04);
        assert_eq!(reader.data(&arrival), b"connect");
        assert_eq!(reader.answer_msgid(&arrival), 0x4200);

        peer.command(0x4291, 0x33, b"query");
        let Took::Command(arrival) = take(&mut reader, now) else {
            panic!("a command")
        };
        assert_eq!(
            (arrival.link, arrival.node),
            (CommandLink::Payload, Some(1))
        );
        assert_eq!(reader.answer_msgid(&arrival), 0x4211);

        peer.command(0x4281, 0, b"on");
        assert_eq!(take(&mut reader, now), Took::Done);
        let inbox = bus.inbox("fsw").expect("fsw's inbox");
        let mut buffer = [0u8; 16];
        assert_eq!(inbox.take(&mut buffer), Some(2));
        assert_eq!(&buffer[..2], b"on");
        assert_eq!(inbox.take(&mut buffer), None);
    }

    /// What is not taken -- from a stranger, from another processor, not
    /// SBN's length, a command whose checksum is wrong, under a message ID
    /// tcspecial does not take, or not a command at all -- is counted as
    /// malformed, and nothing in it is handed on.
    #[test]
    fn what_is_not_taken_is_counted_as_malformed() {
        let (bus, mut reader, peer, now) = connected();
        let counted = bus.malformed();
        let count = || counted.load(Ordering::Relaxed);
        assert_eq!(count(), 0);

        let stranger = UdpSocket::bind("127.0.0.1:0").unwrap();
        peer.say_as(&stranger, THEIRS, kind::HEARTBEAT, &[]);
        assert_eq!(take(&mut reader, now), Took::Done);
        assert_eq!(count(), 1, "from a stranger");

        peer.say_as(&peer.socket, 7, kind::HEARTBEAT, &[]);
        assert_eq!(take(&mut reader, now), Took::Done);
        assert_eq!(count(), 2, "from another processor");

        let short = [0, 0, 0, 9, 0xA0, 0, 0, 0, 1, 0, 0, 0, 0x42];
        peer.socket.send_to(&short, peer.vehicle).unwrap();
        assert_eq!(take(&mut reader, now), Took::Done);
        assert_eq!(count(), 3, "a length its header does not say");

        let mut room = [0u8; 64];
        let size =
            cfs::write_command(&mut room, 0x4280, 0, SPACECRAFT, 4, b"x")
                .unwrap();
        room[size - 1] ^= 1;
        peer.say(kind::APPLICATION, &room[..size]);
        assert_eq!(take(&mut reader, now), Took::Done);
        assert_eq!(count(), 4, "a checksum that is wrong");

        peer.command(0x4282, 4, b"x");
        assert_eq!(take(&mut reader, now), Took::Done);
        assert_eq!(count(), 5, "a message ID not subscribed to");

        let size = cfs::write_telemetry(
            &mut room, 0x4242, 0, SPACECRAFT, [0; 6], b"x",
        )
        .unwrap();
        peer.say(kind::APPLICATION, &room[..size]);
        assert_eq!(take(&mut reader, now), Took::Done);
        assert_eq!(count(), 6, "telemetry, which tcspecial does not take");
    }
}
