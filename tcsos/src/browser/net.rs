//! The page's own network: UDP and TCP between the programs in the page, at
//! the addresses their configurations give, in place of the system's.
//!
//! A page has no sockets. What the TCS programs say to one another over
//! loopback -- a vehicle to its simulator, a ground station to a vehicle --
//! is said here through queues in the page's memory, with the standard
//! library's interface: `UdpSocket`, `TcpListener` and `TcpStream`, bound,
//! connected, read and written as the standard library's are, blocking or
//! not, with timeouts. There is one host: an address's port is what is
//! looked at, whatever its IP address, as two programs on one machine see
//! loopback. A UDP datagram to a port nothing is bound to is lost, as one
//! would be; a TCP connection to a port nothing listens on is refused.
//!
//! TCP here is reliable and ordered and carries bytes, as TCP does; what is
//! framed on it -- the TCS programs' four bytes of length before each
//! packet -- is the programs' own.
//!
//! Every socket is a [`Source`], so [`crate::browser::wait`] can wait on any
//! of them, as `poll` does on Linux.

pub use std::net::{
    AddrParseError, IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr,
    SocketAddrV4, SocketAddrV6, ToSocketAddrs,
};

use std::collections::{BTreeMap, VecDeque};
use std::io::{self, Read, Write};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Duration;

use crate::browser::context;
use crate::browser::time::Instant;
use crate::browser::wait::{Ready, Source};

/// What one socket is, and what is waiting on it.
#[derive(Debug)]
enum Kind {
    /// A UDP socket.
    Udp {
        /// What has arrived, with where it came from.
        queue: VecDeque<(SocketAddr, Vec<u8>)>,
        /// Where it sends, and the only place it hears from, once connected.
        peer: Option<SocketAddr>,
    },
    /// A TCP listener.
    Listener {
        /// The connections made to it, not yet accepted: their streams.
        backlog: VecDeque<u32>,
    },
    /// One end of a TCP connection.
    Stream {
        /// The other end, until it is gone.
        peer: Option<u32>,
        /// Where the other end is.
        peer_addr: SocketAddr,
        /// What has arrived and not been read.
        rx: VecDeque<u8>,
        /// Whether the other end will write no more.
        ended: bool,
        /// Whether this end will write no more.
        shut: bool,
    },
    /// A wakeup: see [`crate::browser::wait::Wakeup`].
    Wakeup {
        /// Whether it has been poked since it was drained.
        poked: bool,
    },
}

/// One socket.
#[derive(Debug)]
struct Socket {
    /// What it is.
    kind: Kind,
    /// Where it is bound.
    addr: SocketAddr,
    /// Whether a read or an accept returns at once rather than waiting.
    nonblocking: bool,
    /// How long a read waits, at most.
    read_timeout: Option<Duration>,
    /// How many handles there are on it; it goes when the last does.
    handles: usize,
    /// The program it belongs to, whose ending closes it as a process's
    /// exit closes its sockets on Linux: see [`close_owned_by`].
    owner: u32,
}

/// Every socket in the page.
#[derive(Debug, Default)]
struct Net {
    /// The sockets, by number.
    sockets: BTreeMap<u32, Socket>,
    /// The number the next socket is given.
    next: u32,
    /// The UDP socket bound to each port.
    udp: BTreeMap<u16, u32>,
    /// The listener on each port.
    tcp: BTreeMap<u16, u32>,
    /// The next port given to a socket bound to port nought.
    ephemeral: u16,
    /// The sockets closed by their program's ending: what a thread of that
    /// program sends on one after, which on Linux would never be sent, the
    /// process being gone, goes nowhere and is not refused.
    gone: std::collections::BTreeSet<u32>,
}

/// The page's network.
static NET: Mutex<Option<Net>> = Mutex::new(None);
/// Notified whenever anything changes, for whatever waits.
static CHANGED: Condvar = Condvar::new();

/// Takes the network, without ever waiting for it in the browser's sense:
/// the page's own thread may not wait, so a held lock is spun on, which is
/// brief, as nothing holds it for long.
///
/// # Returns
///
/// The network.
fn net() -> MutexGuard<'static, Option<Net>> {
    loop {
        match NET.try_lock() {
            Ok(mut held) => {
                held.get_or_insert_with(|| Net {
                    next: 1,
                    ephemeral: 49152,
                    ..Net::default()
                });
                return held;
            }
            Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                return poisoned.into_inner();
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                std::hint::spin_loop();
            }
        }
    }
}

/// Waits for something to change, or for a time: on a worker, on the
/// condition variable; on the page's own thread, which may not wait, by
/// giving the lock up and taking it again.
///
/// # Arguments
///
/// * `held`  - The network, held.
/// * `until` - When to stop waiting, if ever.
///
/// # Returns
///
/// The network, held again.
fn changed(
    held: MutexGuard<'static, Option<Net>>,
    until: Option<Instant>,
) -> MutexGuard<'static, Option<Net>> {
    if !context::may_wait() {
        drop(held);
        std::hint::spin_loop();
        return net();
    }
    match until {
        None => CHANGED.wait(held).unwrap_or_else(|e| e.into_inner()),
        Some(until) => {
            let left = until.saturating_duration_since(Instant::now());
            CHANGED
                .wait_timeout(held, left)
                .map(|(held, _)| held)
                .unwrap_or_else(|e| e.into_inner().0)
        }
    }
}

/// Says that something changed, to whatever waits.
fn notify() {
    CHANGED.notify_all();
}

impl Net {
    /// Gives a socket a number and a place.
    ///
    /// # Arguments
    ///
    /// * `kind` - What it is.
    /// * `addr` - Where it is.
    ///
    /// # Returns
    ///
    /// Its number.
    fn add(&mut self, kind: Kind, addr: SocketAddr) -> u32 {
        let id = self.next;
        self.next += 1;
        self.sockets.insert(
            id,
            Socket {
                kind,
                addr,
                nonblocking: false,
                read_timeout: None,
                handles: 1,
                owner: context::current().id,
            },
        );
        id
    }

    /// A port for a socket bound to port nought: one nothing of the kind
    /// is bound to.
    ///
    /// # Arguments
    ///
    /// * `udp` - Whether it is for a UDP socket.
    ///
    /// # Returns
    ///
    /// The port.
    fn ephemeral(&mut self, udp: bool) -> u16 {
        loop {
            let port = self.ephemeral;
            self.ephemeral = if port == u16::MAX { 49152 } else { port + 1 };
            let taken = if udp {
                self.udp.contains_key(&port)
            } else {
                self.tcp.contains_key(&port)
            };
            if !taken {
                return port;
            }
        }
    }

    /// A socket, by number.
    ///
    /// # Arguments
    ///
    /// * `id` - Its number.
    ///
    /// # Returns
    ///
    /// It.
    ///
    /// # Errors
    ///
    /// `NotConnected` where it is gone.
    fn get(&mut self, id: u32) -> io::Result<&mut Socket> {
        self.sockets
            .get_mut(&id)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotConnected))
    }

    /// Lets a handle on a socket go, and the socket with the last.
    ///
    /// # Arguments
    ///
    /// * `id` - Its number.
    fn release(&mut self, id: u32) {
        let Some(socket) = self.sockets.get_mut(&id) else {
            return;
        };
        socket.handles -= 1;
        if socket.handles > 0 {
            return;
        }
        let socket = self.sockets.remove(&id).expect("just found");
        let port = socket.addr.port();
        match socket.kind {
            Kind::Udp { .. } => {
                if self.udp.get(&port) == Some(&id) {
                    self.udp.remove(&port);
                }
            }
            Kind::Listener { backlog } => {
                if self.tcp.get(&port) == Some(&id) {
                    self.tcp.remove(&port);
                }
                for stream in backlog {
                    self.release(stream);
                }
            }
            Kind::Stream { peer, .. } => {
                if let Some(peer) = peer {
                    if let Some(Socket {
                        kind: Kind::Stream { peer, ended, .. },
                        ..
                    }) = self.sockets.get_mut(&peer)
                    {
                        *peer = None;
                        *ended = true;
                    }
                }
            }
            Kind::Wakeup { .. } => {}
        }
        notify();
    }

    /// What a socket is ready for.
    ///
    /// # Arguments
    ///
    /// * `id` - Its number.
    ///
    /// # Returns
    ///
    /// What it is ready for: trouble, where it is gone.
    fn ready(&self, id: u32) -> Ready {
        let Some(socket) = self.sockets.get(&id) else {
            return Ready {
                read: false,
                write: false,
                trouble: true,
            };
        };
        match &socket.kind {
            Kind::Udp { queue, .. } => Ready {
                read: !queue.is_empty(),
                write: true,
                trouble: false,
            },
            Kind::Listener { backlog } => Ready {
                read: !backlog.is_empty(),
                write: false,
                trouble: false,
            },
            Kind::Stream {
                rx, ended, peer, ..
            } => Ready {
                read: !rx.is_empty() || *ended,
                write: peer.is_some(),
                trouble: peer.is_none() && rx.is_empty(),
            },
            Kind::Wakeup { poked } => Ready {
                read: *poked,
                write: false,
                trouble: false,
            },
        }
    }
}

/// Where a bind is to be: the first address an argument gives.
///
/// # Arguments
///
/// * `addr` - The address, in any form the standard library takes.
///
/// # Returns
///
/// The address.
///
/// # Errors
///
/// `InvalidInput` where it gives none.
fn first(addr: impl ToSocketAddrs) -> io::Result<SocketAddr> {
    addr.to_socket_addrs()?.next().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "no address was given")
    })
}

/// What the readiness of a source is, for [`crate::browser::wait`].
///
/// # Arguments
///
/// * `net`     - The sockets the sources are.
/// * `sources` - The sources, and what each is wanted ready for.
/// * `got`     - Set to what each was found ready for.
///
/// # Returns
///
/// How many are ready for something wanted, or in trouble.
fn readiness(
    net: &Net,
    sources: &[(Source, Ready)],
    got: &mut [Ready],
) -> usize {
    let mut count = 0;
    for ((source, want), got) in sources.iter().zip(got.iter_mut()) {
        if *source == Source::NONE {
            *got = Ready::default();
            continue;
        }
        let ready = net.ready(source.id());
        *got = Ready {
            read: ready.read && want.read,
            write: ready.write && want.write,
            trouble: ready.trouble,
        };
        if got.any() {
            count += 1;
        }
    }
    count
}

/// Waits until any of a set of sources is ready for what is wanted of it,
/// or the time is up: what [`crate::browser::wait::wait`] does.
///
/// # Arguments
///
/// * `sources` - The sources, and what each is wanted ready for.
/// * `got`     - Set to what each was found ready for.
/// * `timeout` - How long to wait at most; `None` for ever.
///
/// # Returns
///
/// How many are ready.
pub(crate) fn wait_on(
    sources: &[(Source, Ready)],
    got: &mut [Ready],
    timeout: Option<Duration>,
) -> usize {
    let until = timeout.map(|wait| Instant::now() + wait);
    let mut held = net();
    loop {
        let count = readiness(held.as_ref().expect("made"), sources, got);
        if count > 0 || until.is_some_and(|until| Instant::now() >= until) {
            return count;
        }
        held = changed(held, until);
    }
}

/// Makes a wakeup's source: see [`crate::browser::wait::Wakeup`].
///
/// # Returns
///
/// Its number.
pub(crate) fn new_wakeup() -> u32 {
    let mut held = net();
    let net = held.as_mut().expect("made");
    net.add(
        Kind::Wakeup { poked: false },
        SocketAddr::from(([0, 0, 0, 0], 0)),
    )
}

/// Pokes a wakeup, or drains it.
///
/// # Arguments
///
/// * `id`    - Its number.
/// * `poked` - Whether it is poked, or drained.
pub(crate) fn set_wakeup(id: u32, poked: bool) {
    let mut held = net();
    if let Ok(Socket {
        kind: Kind::Wakeup { poked: was },
        ..
    }) = held.as_mut().expect("made").get(id)
    {
        *was = poked;
    }
    drop(held);
    if poked {
        notify();
    }
}

/// Closes every socket a program holds, as a process's exit closes its
/// sockets on Linux: their ports are free again at once, and a thread of
/// the program still using one finds it gone. Its wakeups are left to its
/// threads.
///
/// # Arguments
///
/// * `owner` - The program's number.
pub(crate) fn close_owned_by(owner: u32) {
    let mut held = net();
    let net = held.as_mut().expect("made");
    let owned: Vec<u32> = net
        .sockets
        .iter()
        .filter(|(_, socket)| {
            socket.owner == owner && !matches!(socket.kind, Kind::Wakeup { .. })
        })
        .map(|(id, _)| *id)
        .collect();
    for id in owned {
        if let Some(socket) = net.sockets.get_mut(&id) {
            socket.handles = 1;
        }
        net.release(id);
        net.gone.insert(id);
    }
}

/// Hands a socket to another program, as a descriptor handed to a child
/// stays open in the child when its parent ends.
///
/// # Arguments
///
/// * `id`    - The socket's number.
/// * `owner` - The program's number.
pub(crate) fn hand_to(id: u32, owner: u32) {
    if let Some(socket) = net().as_mut().expect("made").sockets.get_mut(&id) {
        socket.owner = owner;
    }
}

/// Lets a source go: a wakeup dropped.
///
/// # Arguments
///
/// * `id` - Its number.
pub(crate) fn release(id: u32) {
    net().as_mut().expect("made").release(id);
}

/// What a blocking call does once it has been told it cannot go on yet:
/// waits, where the socket blocks, until its timeout.
///
/// # Arguments
///
/// * `held`     - The network, held.
/// * `id`       - The socket.
/// * `deadline` - When its timeout runs out, where it has one.
///
/// # Returns
///
/// The network, held again.
///
/// # Errors
///
/// `WouldBlock` where the socket does not block or its timeout has run out,
/// as the standard library says on Linux.
fn blocked(
    held: MutexGuard<'static, Option<Net>>,
    id: u32,
    deadline: Option<Instant>,
) -> io::Result<MutexGuard<'static, Option<Net>>> {
    let nonblocking = held
        .as_ref()
        .and_then(|net| net.sockets.get(&id))
        .is_none_or(|socket| socket.nonblocking);
    if nonblocking || deadline.is_some_and(|until| Instant::now() >= until) {
        return Err(io::Error::from(io::ErrorKind::WouldBlock));
    }
    Ok(changed(held, deadline))
}

/// When a read on a socket gives up waiting.
///
/// # Arguments
///
/// * `net` - The sockets.
/// * `id`  - The socket's number.
///
/// # Returns
///
/// Now and the socket's read timeout, or `None` where it has none or
/// is gone.
fn deadline_of(net: &Net, id: u32) -> Option<Instant> {
    net.sockets
        .get(&id)
        .and_then(|socket| socket.read_timeout)
        .map(|wait| Instant::now() + wait)
}

/// The settings every kind of socket has.
macro_rules! settings {
    ($type:ident) => {
        impl $type {
            /// Where this end is bound.
            ///
            /// # Returns
            ///
            /// The address.
            ///
            /// # Errors
            ///
            /// `NotConnected` where the socket is gone.
            pub fn local_addr(&self) -> io::Result<SocketAddr> {
                Ok(net().as_mut().expect("made").get(self.id)?.addr)
            }

            /// Has reads and accepts return at once rather than wait.
            ///
            /// # Arguments
            ///
            /// * `on` - Whether they do.
            ///
            /// # Errors
            ///
            /// `NotConnected` where the socket is gone.
            pub fn set_nonblocking(&self, on: bool) -> io::Result<()> {
                net().as_mut().expect("made").get(self.id)?.nonblocking = on;
                Ok(())
            }

            /// Has a read wait no longer than a time.
            ///
            /// # Arguments
            ///
            /// * `wait` - The time; `None` for ever.
            ///
            /// # Errors
            ///
            /// `InvalidInput` for a time of nought, as the standard
            /// library says, and `NotConnected` where the socket is gone.
            pub fn set_read_timeout(
                &self,
                wait: Option<Duration>,
            ) -> io::Result<()> {
                if wait == Some(Duration::ZERO) {
                    return Err(io::Error::from(io::ErrorKind::InvalidInput));
                }
                net().as_mut().expect("made").get(self.id)?.read_timeout = wait;
                Ok(())
            }

            /// Has a write wait no longer than a time: a write here never
            /// waits.
            ///
            /// # Arguments
            ///
            /// * `_wait` - The time.
            ///
            /// # Errors
            ///
            /// None.
            pub fn set_write_timeout(
                &self,
                _wait: Option<Duration>,
            ) -> io::Result<()> {
                Ok(())
            }

            /// How long a read waits at most.
            ///
            /// # Returns
            ///
            /// The time; `None` for ever.
            ///
            /// # Errors
            ///
            /// `NotConnected` where the socket is gone.
            pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
                Ok(net().as_mut().expect("made").get(self.id)?.read_timeout)
            }

            /// A second handle on the same socket.
            ///
            /// # Returns
            ///
            /// The handle.
            ///
            /// # Errors
            ///
            /// `NotConnected` where the socket is gone.
            pub fn try_clone(&self) -> io::Result<$type> {
                net().as_mut().expect("made").get(self.id)?.handles += 1;
                Ok($type { id: self.id })
            }

            /// A failure the socket has been keeping: none, here.
            ///
            /// # Returns
            ///
            /// `None`.
            ///
            /// # Errors
            ///
            /// None.
            pub fn take_error(&self) -> io::Result<Option<io::Error>> {
                Ok(None)
            }
        }

        impl Drop for $type {
            fn drop(&mut self) {
                net().as_mut().expect("made").release(self.id);
            }
        }

        impl crate::browser::wait::Waitable for $type {
            fn source(&self) -> Source {
                Source::of(self.id)
            }
        }
    };
}

/// A UDP socket in the page.
#[derive(Debug)]
pub struct UdpSocket {
    /// Its number.
    id: u32,
}

settings!(UdpSocket);

impl UdpSocket {
    /// Binds a socket to an address.
    ///
    /// # Arguments
    ///
    /// * `addr` - Where: port nought for one the page chooses.
    ///
    /// # Returns
    ///
    /// The socket.
    ///
    /// # Errors
    ///
    /// `AddrInUse` where a socket is bound to the port already.
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<UdpSocket> {
        let mut addr = first(addr)?;
        let mut held = net();
        let net = held.as_mut().expect("made");
        if addr.port() == 0 {
            addr.set_port(net.ephemeral(true));
        }
        if net.udp.contains_key(&addr.port()) {
            return Err(io::Error::from(io::ErrorKind::AddrInUse));
        }
        let id = net.add(
            Kind::Udp {
                queue: VecDeque::new(),
                peer: None,
            },
            addr,
        );
        net.udp.insert(addr.port(), id);
        Ok(UdpSocket { id })
    }

    /// Sends to one place, and hears only from it.
    ///
    /// # Arguments
    ///
    /// * `addr` - Where.
    ///
    /// # Errors
    ///
    /// `NotConnected` where the socket is gone.
    pub fn connect(&self, addr: impl ToSocketAddrs) -> io::Result<()> {
        let addr = first(addr)?;
        if let Kind::Udp { peer, .. } =
            &mut net().as_mut().expect("made").get(self.id)?.kind
        {
            *peer = Some(addr);
        }
        Ok(())
    }

    /// Where a connected socket sends.
    ///
    /// # Returns
    ///
    /// The address.
    ///
    /// # Errors
    ///
    /// `NotConnected` where it is not connected.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        match &net().as_mut().expect("made").get(self.id)?.kind {
            Kind::Udp {
                peer: Some(peer), ..
            } => Ok(*peer),
            _ => Err(io::Error::from(io::ErrorKind::NotConnected)),
        }
    }

    /// Sends a datagram to a place.
    ///
    /// # Arguments
    ///
    /// * `data` - The datagram.
    /// * `to`   - Where.
    ///
    /// # Returns
    ///
    /// Its length: a datagram is sent whole or not at all, and one to a
    /// port nothing is bound to is lost, as it would be.
    ///
    /// # Errors
    ///
    /// `NotConnected` where the socket is gone.
    pub fn send_to(
        &self,
        data: &[u8],
        to: impl ToSocketAddrs,
    ) -> io::Result<usize> {
        let to = first(to)?;
        let mut held = net();
        let net = held.as_mut().expect("made");
        if net.gone.contains(&self.id) {
            return Ok(data.len());
        }
        let from = net.get(self.id)?.addr;
        let from = if from.ip().is_unspecified() {
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), from.port())
        } else {
            from
        };
        if let Some(&there) = net.udp.get(&to.port()) {
            if let Ok(Socket {
                kind: Kind::Udp { queue, peer },
                ..
            }) = net.get(there)
            {
                if peer.is_none_or(|peer| peer.port() == from.port()) {
                    queue.push_back((from, data.to_vec()));
                    drop(held);
                    notify();
                }
            }
        }
        Ok(data.len())
    }

    /// Sends a datagram where the socket is connected.
    ///
    /// # Arguments
    ///
    /// * `data` - The datagram.
    ///
    /// # Returns
    ///
    /// Its length.
    ///
    /// # Errors
    ///
    /// `NotConnected` where it is not connected.
    pub fn send(&self, data: &[u8]) -> io::Result<usize> {
        if net().as_ref().expect("made").gone.contains(&self.id) {
            return Ok(data.len());
        }
        let to = self.peer_addr()?;
        self.send_to(data, to)
    }

    /// Takes the next datagram, or looks at it and leaves it.
    ///
    /// # Arguments
    ///
    /// * `into` - Where as much of it as fits goes.
    /// * `take` - Whether it is taken.
    ///
    /// # Returns
    ///
    /// How much was copied, its whole length, and where it came from.
    ///
    /// # Errors
    ///
    /// `WouldBlock` where none has come and the socket does not wait, or
    /// its timeout has run out.
    fn next(
        &self,
        into: &mut [u8],
        take: bool,
    ) -> io::Result<(usize, usize, SocketAddr)> {
        let mut held = net();
        let deadline = deadline_of(held.as_ref().expect("made"), self.id);
        loop {
            if let Kind::Udp { queue, .. } =
                &mut held.as_mut().expect("made").get(self.id)?.kind
            {
                let found = if take {
                    queue.pop_front()
                } else {
                    queue.front().cloned()
                };
                if let Some((from, datagram)) = found {
                    let copied = datagram.len().min(into.len());
                    into[..copied].copy_from_slice(&datagram[..copied]);
                    return Ok((copied, datagram.len(), from));
                }
            }
            held = blocked(held, self.id, deadline)?;
        }
    }

    /// Takes the next datagram: what does not fit is lost.
    ///
    /// # Arguments
    ///
    /// * `into` - Where it goes.
    ///
    /// # Returns
    ///
    /// How much was copied, and where it came from.
    ///
    /// # Errors
    ///
    /// `WouldBlock` where none has come and the socket does not wait.
    pub fn recv_from(
        &self,
        into: &mut [u8],
    ) -> io::Result<(usize, SocketAddr)> {
        self.next(into, true)
            .map(|(copied, _, from)| (copied, from))
    }

    /// Takes the next datagram, from wherever.
    ///
    /// # Arguments
    ///
    /// * `into` - Where it goes.
    ///
    /// # Returns
    ///
    /// How much was copied.
    ///
    /// # Errors
    ///
    /// `WouldBlock` where none has come and the socket does not wait.
    pub fn recv(&self, into: &mut [u8]) -> io::Result<usize> {
        self.recv_from(into).map(|(copied, _)| copied)
    }

    /// Looks at the next datagram and leaves it.
    ///
    /// # Arguments
    ///
    /// * `into` - Where it is copied.
    ///
    /// # Returns
    ///
    /// How much was copied, and where it came from.
    ///
    /// # Errors
    ///
    /// `WouldBlock` where none has come and the socket does not wait.
    pub fn peek_from(
        &self,
        into: &mut [u8],
    ) -> io::Result<(usize, SocketAddr)> {
        self.next(into, false)
            .map(|(copied, _, from)| (copied, from))
    }

    /// Looks at the next datagram, from wherever, and leaves it.
    ///
    /// # Arguments
    ///
    /// * `into` - Where it is copied.
    ///
    /// # Returns
    ///
    /// How much was copied.
    ///
    /// # Errors
    ///
    /// `WouldBlock` where none has come and the socket does not wait.
    pub fn peek(&self, into: &mut [u8]) -> io::Result<usize> {
        self.next(into, false).map(|(copied, _, _)| copied)
    }

    /// Allows broadcast: nothing to allow in a page of one host.
    ///
    /// # Arguments
    ///
    /// * `_on` - Whether it is allowed.
    ///
    /// # Errors
    ///
    /// None.
    pub fn set_broadcast(&self, _on: bool) -> io::Result<()> {
        Ok(())
    }
}

/// Reads one datagram, and says how long it was rather than how much of it
/// fitted.
///
/// # Arguments
///
/// * `socket` - The socket.
/// * `into`   - Where as much of it as fits goes.
///
/// # Returns
///
/// The datagram's whole length.
///
/// # Errors
///
/// As for [`UdpSocket::recv_from`].
pub fn receive_whole_length(
    socket: &UdpSocket,
    into: &mut [u8],
) -> io::Result<usize> {
    socket.next(into, true).map(|(_, whole, _)| whole)
}

/// Takes one datagram that is waiting on a socket and throws it away,
/// without waiting.
///
/// # Arguments
///
/// * `source` - The socket.
///
/// # Returns
///
/// Whether one was waiting.
///
/// # Errors
///
/// `NotConnected` where the socket is gone.
pub fn discard_one(source: Source) -> io::Result<bool> {
    let mut held = net();
    match &mut held.as_mut().expect("made").get(source.id())?.kind {
        Kind::Udp { queue, .. } => Ok(queue.pop_front().is_some()),
        _ => Ok(false),
    }
}

/// A TCP listener in the page.
#[derive(Debug)]
pub struct TcpListener {
    /// Its number.
    id: u32,
}

settings!(TcpListener);

impl TcpListener {
    /// Listens on an address.
    ///
    /// # Arguments
    ///
    /// * `addr` - Where: port nought for one the page chooses.
    ///
    /// # Returns
    ///
    /// The listener.
    ///
    /// # Errors
    ///
    /// `AddrInUse` where something listens on the port already.
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<TcpListener> {
        let mut addr = first(addr)?;
        let mut held = net();
        let net = held.as_mut().expect("made");
        if addr.port() == 0 {
            addr.set_port(net.ephemeral(false));
        }
        if net.tcp.contains_key(&addr.port()) {
            return Err(io::Error::from(io::ErrorKind::AddrInUse));
        }
        let id = net.add(
            Kind::Listener {
                backlog: VecDeque::new(),
            },
            addr,
        );
        net.tcp.insert(addr.port(), id);
        Ok(TcpListener { id })
    }

    /// Takes the next connection made to it.
    ///
    /// # Returns
    ///
    /// This end of it, and where the other end is.
    ///
    /// # Errors
    ///
    /// `WouldBlock` where none has been made and the listener does not
    /// wait.
    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        let mut held = net();
        loop {
            let net = held.as_mut().expect("made");
            if let Kind::Listener { backlog } = &mut net.get(self.id)?.kind {
                if let Some(stream) = backlog.pop_front() {
                    let peer = match &net.get(stream)?.kind {
                        Kind::Stream { peer_addr, .. } => *peer_addr,
                        _ => unreachable!("a backlog holds streams"),
                    };
                    return Ok((TcpStream { id: stream }, peer));
                }
            }
            held = blocked(held, self.id, None)?;
        }
    }

    /// The connections made to it, one at a time.
    ///
    /// # Returns
    ///
    /// An iterator, which waits as `accept` does.
    pub fn incoming(&self) -> impl Iterator<Item = io::Result<TcpStream>> + '_ {
        std::iter::repeat_with(move || self.accept().map(|(stream, _)| stream))
    }
}

/// One end of a TCP connection in the page.
#[derive(Debug)]
pub struct TcpStream {
    /// Its number.
    pub(crate) id: u32,
}

settings!(TcpStream);

impl TcpStream {
    /// Makes the two ends of a connection between two places, with nothing
    /// listening: what an app's interfaces are, in a page.
    ///
    /// # Returns
    ///
    /// The two ends.
    pub fn pair() -> (TcpStream, TcpStream) {
        let mut held = net();
        let net = held.as_mut().expect("made");
        let here = SocketAddr::from(([127, 0, 0, 1], 0));
        let (a, b) = (net.next, net.next + 1);
        let end = |peer| Kind::Stream {
            peer: Some(peer),
            peer_addr: here,
            rx: VecDeque::new(),
            ended: false,
            shut: false,
        };
        net.add(end(b), here);
        net.add(end(a), here);
        (TcpStream { id: a }, TcpStream { id: b })
    }

    /// Connects to whatever listens at an address.
    ///
    /// # Arguments
    ///
    /// * `addr` - Where.
    ///
    /// # Returns
    ///
    /// This end of the connection.
    ///
    /// # Errors
    ///
    /// `ConnectionRefused` where nothing listens there.
    pub fn connect(addr: impl ToSocketAddrs) -> io::Result<TcpStream> {
        let to = first(addr)?;
        let mut held = net();
        let net = held.as_mut().expect("made");
        let Some(&listener) = net.tcp.get(&to.port()) else {
            return Err(io::Error::from(io::ErrorKind::ConnectionRefused));
        };
        let here = SocketAddr::from(([127, 0, 0, 1], net.ephemeral(false)));
        let (mine, theirs) = (net.next, net.next + 1);
        net.add(
            Kind::Stream {
                peer: Some(theirs),
                peer_addr: to,
                rx: VecDeque::new(),
                ended: false,
                shut: false,
            },
            here,
        );
        net.add(
            Kind::Stream {
                peer: Some(mine),
                peer_addr: here,
                rx: VecDeque::new(),
                ended: false,
                shut: false,
            },
            to,
        );
        // The far end is the listening program's, whose ending closes it.
        let listening = net.get(listener)?.owner;
        net.get(theirs)?.owner = listening;
        if let Kind::Listener { backlog } = &mut net.get(listener)?.kind {
            backlog.push_back(theirs);
        }
        drop(held);
        notify();
        Ok(TcpStream { id: mine })
    }

    /// Connects, giving up after a time: a connection here is made at once
    /// or refused.
    ///
    /// # Arguments
    ///
    /// * `addr`  - Where.
    /// * `_wait` - The time.
    ///
    /// # Returns
    ///
    /// This end of the connection.
    ///
    /// # Errors
    ///
    /// As for [`TcpStream::connect`].
    pub fn connect_timeout(
        addr: &SocketAddr,
        _wait: Duration,
    ) -> io::Result<TcpStream> {
        TcpStream::connect(addr)
    }

    /// Where the other end is.
    ///
    /// # Returns
    ///
    /// The address.
    ///
    /// # Errors
    ///
    /// `NotConnected` where the socket is gone.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        match &net().as_mut().expect("made").get(self.id)?.kind {
            Kind::Stream { peer_addr, .. } => Ok(*peer_addr),
            _ => Err(io::Error::from(io::ErrorKind::NotConnected)),
        }
    }

    /// Says that this end will write, or read, no more.
    ///
    /// # Arguments
    ///
    /// * `how` - Which.
    ///
    /// # Errors
    ///
    /// `NotConnected` where the socket is gone.
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        let mut held = net();
        let net = held.as_mut().expect("made");
        let peer = match &mut net.get(self.id)?.kind {
            Kind::Stream { peer, shut, .. } => {
                if how != Shutdown::Read {
                    *shut = true;
                }
                *peer
            }
            _ => None,
        };
        if how != Shutdown::Read {
            if let Some(peer) = peer {
                if let Ok(Socket {
                    kind: Kind::Stream { ended, .. },
                    ..
                }) = net.get(peer)
                {
                    *ended = true;
                }
            }
        }
        drop(held);
        notify();
        Ok(())
    }

    /// Sends what is written at once: it always is, here.
    ///
    /// # Arguments
    ///
    /// * `_on` - Whether it is.
    ///
    /// # Errors
    ///
    /// None.
    pub fn set_nodelay(&self, _on: bool) -> io::Result<()> {
        Ok(())
    }

    /// Reads what has arrived.
    ///
    /// # Arguments
    ///
    /// * `into` - Where it goes.
    ///
    /// # Returns
    ///
    /// How much: nought at the end.
    ///
    /// # Errors
    ///
    /// `WouldBlock` where nothing has arrived and the stream does not wait.
    fn read_into(&self, into: &mut [u8]) -> io::Result<usize> {
        if into.is_empty() {
            return Ok(0);
        }
        let mut held = net();
        let deadline = deadline_of(held.as_ref().expect("made"), self.id);
        loop {
            if let Kind::Stream { rx, ended, .. } =
                &mut held.as_mut().expect("made").get(self.id)?.kind
            {
                if !rx.is_empty() {
                    let count = rx.len().min(into.len());
                    for (to, from) in into.iter_mut().zip(rx.drain(..count)) {
                        *to = from;
                    }
                    return Ok(count);
                }
                if *ended {
                    return Ok(0);
                }
            }
            held = blocked(held, self.id, deadline)?;
        }
    }

    /// Writes to the other end.
    ///
    /// # Arguments
    ///
    /// * `data` - What.
    ///
    /// # Returns
    ///
    /// How much: all of it.
    ///
    /// # Errors
    ///
    /// `BrokenPipe` where the other end is gone or this end shut.
    fn write_from(&self, data: &[u8]) -> io::Result<usize> {
        let mut held = net();
        let net = held.as_mut().expect("made");
        let peer = match &net.get(self.id)?.kind {
            Kind::Stream {
                peer: Some(peer),
                shut: false,
                ..
            } => *peer,
            _ => return Err(io::Error::from(io::ErrorKind::BrokenPipe)),
        };
        if let Ok(Socket {
            kind: Kind::Stream { rx, .. },
            ..
        }) = net.get(peer)
        {
            rx.extend(data);
        }
        drop(held);
        notify();
        Ok(data.len())
    }
}

impl Read for TcpStream {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        self.read_into(into)
    }
}

impl Read for &TcpStream {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        self.read_into(into)
    }
}

impl Write for TcpStream {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.write_from(data)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Write for &TcpStream {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.write_from(data)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
