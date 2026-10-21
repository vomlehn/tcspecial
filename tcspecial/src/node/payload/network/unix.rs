//! Unix-domain endpoints: sockets named by a path rather than an address.
//!
//! Two protocols, and they divide the way TCP and UDP divide. A stream is
//! accepted, so the connection itself says who is at each end and the device
//! is the end that listens. A datagram is not, so -- as over UDP, and for the
//! same reason -- the end that speaks first reaches out and the other waits at
//! the configured path: the handler, for a device that produces data because
//! it is running, and the device, for one that is driven or asked.
//!
//! What neither TCP nor UDP has is a file to tidy up. A Unix socket's address
//! is a path in the filesystem, and binding one leaves an entry there that
//! outlives the process unless it is removed: a handler that is killed rather
//! than stopped leaves a socket file nothing is listening on, and the next
//! bind of it fails with `EADDRINUSE`. So a bind here looks first, and
//! replaces a socket nothing answers rather than refusing to start for the
//! sake of a file its own predecessor left.
//!
//! See [`crate::endpoint`] for the traits.

use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::{UnixDatagram, UnixStream};
use std::path::{Path, PathBuf};

use log::info;
use nix::poll::PollFlags;
use tcslibgs::{NetworkConfig, SendsFrom, TcsError, TcsResult};

use crate::config::constants::ENDPOINT_BUFFER_SIZE;
use crate::endpoint::bind_failed;
use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable, WaitResult,
    STRANGERS_PER_READ,
};
use crate::node::payload::network::tcp::connect_retrying;

/// A Unix-domain stream: the device listens and the handler connects.
pub struct UnixStreamEndpoint {
    stream: UnixStream,
    _buffer: Vec<u8>,
}

impl UnixStreamEndpoint {
    /// Connect to the device's path, retrying while refused.
    ///
    /// A refusal means nothing is listening there yet, which is the ordinary
    /// case when a handler is started before its device; see
    /// `connect_retrying`. A path with no socket at all is a different
    /// matter and is not waited out: it will not become one by being asked
    /// again.
    pub fn connect_retrying(what: &str, config: &NetworkConfig) -> TcsResult<Self> {
        let path = config.address.clone();
        let stream = connect_retrying(what, &path, || UnixStream::connect(&path))?;
        stream.set_nonblocking(true)?;

        Ok(Self {
            stream,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }

    /// A second endpoint on the same connection.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            stream: self.stream.try_clone()?,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }
}

impl EndpointWaitable for UnixStreamEndpoint {
    fn io_fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }

    fn wait_for_event(&self, cmd_fd: RawFd, timeout_ms: i32) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_fd(), cmd_fd, PollFlags::POLLIN, timeout_ms)
    }
}

impl EndpointReadable for UnixStreamEndpoint {
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        match self.stream.read(buffer) {
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(TcsError::Io(e)),
        }
    }
}

impl EndpointWritable for UnixStreamEndpoint {
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        match self.stream.write(data) {
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(TcsError::Io(e)),
        }
    }
}

/// A Unix-domain datagram socket: the handler waits and the device sends.
///
/// The far end is learnt rather than configured, as it is over UDP: a
/// datagram's sender is the only statement of where it came from. A device's
/// own path is its own business, so nothing here knows it until it writes.
///
/// Except for a node that is spoken to first -- driven, or asked -- where
/// there is nothing to learn from before the first write and the configured
/// path is where the device is: see [`UnixDatagramEndpoint::reaching`].
pub struct UnixDatagramEndpoint {
    /// The handler this endpoint belongs to, as the device configuration
    /// names it.
    what: String,
    socket: UnixDatagram,
    /// Where the far end last wrote from, if it has.
    ///
    /// Shared between the two endpoints of a conduit pair, so that the one
    /// writing sends where the one reading learnt.
    peer: std::sync::Arc<std::sync::Mutex<Option<std::os::unix::net::SocketAddr>>>,
    /// Where the configuration says the device is, for a node that is
    /// spoken to first.
    ///
    /// What is written down beats what was learnt, as it does over UDP: a
    /// node that is spoken to first has nothing to learn from, and one that
    /// speaks first is bound to its path and leaves this `None`. Not shared
    /// between the pair, because it does not change: it is the file's word,
    /// and both endpoints are made from the same file.
    ///
    told: Option<PathBuf>,
    /// The one path whose datagrams are read, where it is known: the path a
    /// device reached at its address is at, or the one a device that speaks
    /// first sends from, where its node says. Anything from elsewhere is a
    /// stranger's, turned away and counted rather than handed on as the
    /// device's reading.
    only_from: Option<PathBuf>,
    /// Datagrams turned away since [`EndpointReadable::strangers`] last asked.
    strangers: u64,
    _buffer: Vec<u8>,
}

impl UnixDatagramEndpoint {
    /// Bind the configured path, replacing a socket nothing answers.
    pub fn bind(what: &str, config: &NetworkConfig) -> TcsResult<Self> {
        let path = config.address.clone();
        clear_a_dead_socket(what, Path::new(&path))?;

        let socket = UnixDatagram::bind(&path)
            .map_err(|e| bind_failed(&format!("{what} Unix datagram endpoint"), &path, e))?;
        socket.set_nonblocking(true)?;

        Ok(Self {
            what: what.to_string(),
            socket,
            peer: std::sync::Arc::new(std::sync::Mutex::new(None)),
            told: None,
            only_from: None,
            strangers: 0,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }

    /// Send where the configuration says the device is, from a socket of this
    /// end's own.
    ///
    /// For a node that is spoken to first -- a heater, a valve, a latch, or a
    /// sensor that is asked -- which [`UnixDatagramEndpoint::bind`] cannot
    /// serve. Binding the path and waiting to hear from the device is what a
    /// node that speaks first does, and a device waiting to be spoken to does
    /// not write, so the wait would not end and every write would be refused
    /// for want of a path to send to. The address in the file is where the
    /// device is, which is what it always meant.
    ///
    /// The socket is bound to a name the kernel chooses, in the abstract
    /// namespace rather than the filesystem, so that a device that is asked
    /// has somewhere to send its answer, and nothing is left behind to be
    /// cleared up when the node goes. A device that is only driven does not
    /// answer, and is none the worse for being able to.
    pub fn reaching(what: &str, config: &NetworkConfig) -> TcsResult<Self> {
        let socket = UnixDatagram::unbound()
            .and_then(|socket| autobind(&socket).map(|()| socket))
            .map_err(|e| {
                TcsError::Endpoint(format!(
                    "{what}: a Unix datagram socket to reach \"{}\" with could not \
                     be made: {e}",
                    config.address
                ))
            })?;
        socket.set_nonblocking(true)?;

        Ok(Self {
            what: what.to_string(),
            socket,
            peer: std::sync::Arc::new(std::sync::Mutex::new(None)),
            told: Some(PathBuf::from(&config.address)),
            only_from: Some(PathBuf::from(&config.address)),
            strangers: 0,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }

    /// Read datagrams from this path and turn every other away, for a device
    /// that speaks first and whose node says where from.
    pub fn hear_only(&mut self, what: &str, from: &SendsFrom) -> TcsResult<()> {
        match from {
            SendsFrom::Path(path) => {
                self.only_from = Some(PathBuf::from(path));
                Ok(())
            }
            SendsFrom::Socket(at) => Err(TcsError::Config(format!(
                "{what}: sends_from {at} is a host and a port, and this is a Unix \
                 datagram socket, whose device sends from a path"
            ))),
        }
    }

    /// A second endpoint on the same socket, sharing what it learns.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            what: self.what.clone(),
            socket: self.socket.try_clone()?,
            peer: self.peer.clone(),
            told: self.told.clone(),
            only_from: self.only_from.clone(),
            // Counted where they are read, which is one of the pair.
            strangers: 0,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }
}

impl EndpointWaitable for UnixDatagramEndpoint {
    fn io_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }

    fn wait_for_event(&self, cmd_fd: RawFd, timeout_ms: i32) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_fd(), cmd_fd, PollFlags::POLLIN, timeout_ms)
    }
}

impl EndpointReadable for UnixDatagramEndpoint {
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        // recv_from rather than recv, so that answering is possible. A
        // stranger's datagram is passed over, a bounded number a read, as it
        // is over UDP.
        for _ in 0..STRANGERS_PER_READ {
            match self.socket.recv_from(buffer) {
                Ok((_, from))
                    if self
                        .only_from
                        .as_deref()
                        .is_some_and(|only| from.as_pathname() != Some(only)) =>
                {
                    self.strangers += 1;
                }
                Ok((n, from)) => {
                    if let Ok(mut peer) = self.peer.lock() {
                        *peer = Some(from);
                    }
                    return Ok(n);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(0),
                Err(e) => return Err(TcsError::Io(e)),
            }
        }
        Ok(0)
    }

    fn strangers(&mut self) -> u64 {
        std::mem::take(&mut self.strangers)
    }
}

impl EndpointWritable for UnixDatagramEndpoint {
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        // What the file says, where it says anything: a node that is
        // spoken to first is sent to at the path it names, without waiting
        // to be written to first. See `UnixDatagramEndpoint::reaching`.
        //
        // Sent to an address as it is held, either way: a learnt one is not
        // made into a path first, so that answering allocates nothing.
        let sent = match &self.told {
            Some(told) => self.socket.send_to(data, told),
            None => {
                // A copy of the address, which holds its path inline.
                let peer = self
                    .peer
                    .lock()
                    .ok()
                    .and_then(|guard| guard.clone())
                    .ok_or_else(|| {
                        TcsError::Endpoint(format!(
                            "{}: nothing has been received on this socket yet, so \
                             there is no path to send to",
                            self.what
                        ))
                    })?;

                // An unnamed sender has no path to be answered at. A device
                // that never bound one cannot be written to, which is worth
                // saying rather than failing as though the write itself had
                // gone wrong.
                if peer.as_pathname().is_none() {
                    return Err(TcsError::Endpoint(format!(
                        "{}: the far end of this socket has no path of its own, \
                         so it cannot be sent to: a datagram device binds a path \
                         in order to be answered",
                        self.what
                    )));
                }
                self.socket.send_to_addr(data, &peer)
            }
        };

        match sent {
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(TcsError::Io(e)),
        }
    }
}

/// Bind a socket to a name the kernel chooses in the abstract namespace.
///
/// Linux's autobind: a bind that gives the address family and no name. The
/// name is unique, so two nodes, or two runs, never contend for one, and it
/// is not a file, so nothing outlives the socket. std has no call for it.
fn autobind(socket: &UnixDatagram) -> io::Result<()> {
    // SAFETY: an all-zero sockaddr_un is a valid value of it, and bind reads
    // only the family from it, the length given covering nothing more.
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    let length = std::mem::size_of::<libc::sa_family_t>() as libc::socklen_t;
    // SAFETY: the descriptor is the socket's own and open for as long as it
    // is borrowed, and the address is valid for the length given.
    let bound = unsafe {
        libc::bind(
            socket.as_raw_fd(),
            &address as *const libc::sockaddr_un as *const libc::sockaddr,
            length,
        )
    };
    if bound == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Remove a socket file nothing is listening on, so that a bind can have the
/// path.
///
/// A Unix socket's address is a file, and it outlives the process that bound
/// it: a handler killed rather than stopped leaves one behind, and the next
/// bind of that path fails with `EADDRINUSE` for the sake of a file its own
/// predecessor left. So what is there is tried first. A socket that answers
/// belongs to something that is running and is left alone -- two handlers at
/// one path is a configuration to be reported, not resolved by unlinking. A
/// socket that answers nothing is dead and is removed. Anything else at that
/// path is not a socket and is left for the bind to refuse, which says so
/// better than this could.
fn clear_a_dead_socket(what: &str, path: &Path) -> TcsResult<()> {
    let found = match std::fs::symlink_metadata(path) {
        Ok(found) => found,
        // Nothing there, which is the ordinary case.
        Err(_) => return Ok(()),
    };

    if !is_a_socket(&found) {
        return Ok(());
    }

    // A datagram socket cannot be "connected to" in a way that proves
    // anything, so what is asked is whether a datagram can be sent to it: a
    // live socket takes it, and a dead one answers ECONNREFUSED.
    let probe = UnixDatagram::unbound().map_err(TcsError::Io)?;
    match probe.send_to(&[], path) {
        Ok(_) => Err(TcsError::Endpoint(format!(
            "{} is a socket something is already listening on, so this handler \
             would take its place: two handlers cannot share one path",
            path.display()
        ))),
        Err(_) => {
            info!(
                "{what}: {} is a socket nothing answers, left by a handler that was \
                 killed rather than stopped; replacing it",
                path.display()
            );
            std::fs::remove_file(path).map_err(TcsError::Io)
        }
    }
}

/// Whether what is at a path is a socket.
fn is_a_socket(found: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::FileTypeExt;
    found.file_type().is_socket()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use tcslibgs::NetworkProtocol;

    fn a_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "tcs-unix-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    fn config_at(path: &Path, protocol: NetworkProtocol) -> NetworkConfig {
        NetworkConfig {
            protocol,
            address: path.display().to_string(),
            port: 0,
        }
    }

    /// A stream endpoint connects to a device that is listening, and carries
    /// bytes both ways.
    #[test]
    fn a_stream_endpoint_reaches_a_listening_device() {
        let path = a_path("stream");
        std::fs::remove_file(&path).ok();

        // Standing in for tcssim: the device listens at its path.
        let device = UnixListener::bind(&path).expect("the device listens");

        let mut endpoint = UnixStreamEndpoint::connect_retrying(
            "beacon",
            &config_at(&path, NetworkProtocol::UnixStream),
        )
        .expect("the handler connects");

        let (mut accepted, _) = device.accept().expect("the device accepts");

        accepted.write_all(b"from the device").expect("it sends");
        let mut buffer = [0u8; 64];
        let mut got = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while got.len() < b"from the device".len() && std::time::Instant::now() < deadline {
            match endpoint.read(&mut buffer) {
                Ok(n) if n > 0 => got.extend_from_slice(&buffer[..n]),
                _ => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        }
        assert_eq!(got, b"from the device");

        endpoint.write(b"to the device").expect("the handler sends");
        let mut back = [0u8; 64];
        let n = accepted.read(&mut back).expect("the device reads");
        assert_eq!(&back[..n], b"to the device");

        std::fs::remove_file(&path).ok();
    }

    /// A datagram endpoint binds its path, learns where the device is from
    /// the first datagram, and answers there.
    #[test]
    fn a_datagram_endpoint_learns_where_its_device_is() {
        let handler_path = a_path("dgram");
        let device_path = a_path("dgram-device");
        std::fs::remove_file(&handler_path).ok();
        std::fs::remove_file(&device_path).ok();

        let mut endpoint = UnixDatagramEndpoint::bind(
            "beacon",
            &config_at(&handler_path, NetworkProtocol::UnixDgram),
        )
        .expect("the handler binds");

        // Nothing has been heard, so there is nowhere to write.
        assert!(
            endpoint.write(b"into the dark").is_err(),
            "it wrote somewhere without having been written to"
        );

        // Standing in for tcssim: the device binds a path of its own, so
        // that it can be answered, and speaks first.
        let device = UnixDatagram::bind(&device_path).expect("the device binds");
        device
            .send_to(b"from the device", &handler_path)
            .expect("the device sends");

        let mut buffer = [0u8; 64];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut got = 0;
        while got == 0 && std::time::Instant::now() < deadline {
            got = endpoint.read(&mut buffer).expect("a read");
            if got == 0 {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        assert_eq!(&buffer[..got], b"from the device");

        // And now it knows where to answer.
        endpoint.write(b"to the device").expect("the handler sends");
        let mut back = [0u8; 64];
        let (n, _) = device.recv_from(&mut back).expect("the device reads");
        assert_eq!(&back[..n], b"to the device");

        std::fs::remove_file(&handler_path).ok();
        std::fs::remove_file(&device_path).ok();
    }

    /// A node that is only driven is sent to where the file says it is.
    ///
    /// The case binding cannot serve: a device that is never read never
    /// writes, so a handler waiting to learn where it is would wait for ever
    /// and refuse every write meanwhile -- which for a heater or a valve is
    /// the vehicle deciding and nothing happening. Driven from a socket with
    /// a name outside the filesystem, so nothing is left there.
    #[test]
    fn a_driven_datagram_device_is_sent_to_where_it_is() {
        let device_path = a_path("driven-device");
        std::fs::remove_file(&device_path).ok();

        // The device, standing in for tcssim: it binds the path the
        // configuration names and waits to be driven, saying nothing.
        let device = UnixDatagram::bind(&device_path).expect("the device binds");

        let mut endpoint = UnixDatagramEndpoint::reaching(
            "heaters",
            &config_at(&device_path, NetworkProtocol::UnixDgram),
        )
        .expect("the vehicle's end opens");

        // Written at once, where a bound endpoint would have refused this for
        // want of anything to answer.
        endpoint.write(b"1").expect("the decision goes out");
        let mut back = [0u8; 64];
        let (n, from) = device.recv_from(&mut back).expect("the device reads");
        assert_eq!(&back[..n], b"1");
        assert_eq!(
            from.as_pathname(),
            None,
            "the vehicle's end should leave nothing in the filesystem"
        );

        // Again, because a decision is asserted on every sample rather than
        // once: the endpoint holds where the device is rather than learning
        // it, so the second write goes the same way as the first.
        endpoint.write(b"0").expect("the next decision goes out");
        let (n, _) = device.recv_from(&mut back).expect("the device reads again");
        assert_eq!(&back[..n], b"0");

        // And nothing of this end is in the filesystem to clear up.
        drop(endpoint);
        std::fs::remove_file(&device_path).ok();
    }

    /// A device that is asked is sent its trigger where the file says it is,
    /// and its answer comes back: the vehicle's end has a name to be answered
    /// at, though not one in the filesystem.
    #[test]
    fn an_asked_datagram_device_is_answered_back() {
        use std::os::linux::net::SocketAddrExt;

        let device_path = a_path("asked-device");
        std::fs::remove_file(&device_path).ok();
        let device = UnixDatagram::bind(&device_path).expect("the device binds");

        let mut endpoint = UnixDatagramEndpoint::reaching(
            "temp",
            &config_at(&device_path, NetworkProtocol::UnixDgram),
        )
        .expect("the vehicle's end opens");

        endpoint.write(b"READ").expect("the trigger goes out");
        let mut asked = [0u8; 64];
        let (n, from) = device.recv_from(&mut asked).expect("the device reads");
        assert_eq!(&asked[..n], b"READ");
        assert!(
            from.as_abstract_name().is_some(),
            "the vehicle's end should have a name to be answered at: {from:?}"
        );

        device
            .send_to_addr(b"21.5", &from)
            .expect("the answer goes");
        let mut into = [0u8; 64];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let n = loop {
            match endpoint.read(&mut into).expect("a read") {
                0 if std::time::Instant::now() < deadline => std::thread::yield_now(),
                n => break n,
            }
        };
        assert_eq!(&into[..n], b"21.5");

        std::fs::remove_file(&device_path).ok();
    }

    /// A device reached at its path reads only what comes from there: a
    /// datagram from any other socket is turned away and counted, and the
    /// device's own behind it is still read.
    #[test]
    fn a_stranger_is_turned_away_and_counted() {
        let device_path = a_path("strangers-device");
        let stranger_path = a_path("strangers-stranger");
        for path in [&device_path, &stranger_path] {
            std::fs::remove_file(path).ok();
        }
        let device = UnixDatagram::bind(&device_path).expect("the device binds");
        let stranger = UnixDatagram::bind(&stranger_path).expect("a stranger binds");

        let mut endpoint = UnixDatagramEndpoint::reaching(
            "temp",
            &config_at(&device_path, NetworkProtocol::UnixDgram),
        )
        .expect("the vehicle's end opens");
        // The vehicle's end has a name only the kernel knows; asking the
        // device something is how to learn it.
        endpoint.write(b"READ").expect("the trigger goes out");
        let mut asked = [0u8; 64];
        let (_, vehicle) = device.recv_from(&mut asked).expect("the device reads");

        stranger
            .send_to_addr(b"99.9", &vehicle)
            .expect("the stranger sends");
        device
            .send_to_addr(b"21.5", &vehicle)
            .expect("the answer goes");

        let mut into = [0u8; 64];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let n = loop {
            match endpoint.read(&mut into).expect("a read") {
                0 if std::time::Instant::now() < deadline => std::thread::yield_now(),
                n => break n,
            }
        };
        assert_eq!(
            &into[..n],
            b"21.5",
            "the stranger's was read as the device's"
        );
        assert_eq!(endpoint.strangers(), 1);

        for path in [&device_path, &stranger_path] {
            std::fs::remove_file(path).ok();
        }
    }

    /// A device that speaks first, from a path its node states, is read from
    /// that path only: a stranger's datagram to the vehicle's path is turned
    /// away and counted.
    #[test]
    fn a_device_that_says_where_it_sends_from_is_read_only_from_there() {
        let vehicle_path = a_path("hearing-vehicle");
        let device_path = a_path("hearing-device");
        let stranger_path = a_path("hearing-stranger");
        for path in [&vehicle_path, &device_path, &stranger_path] {
            std::fs::remove_file(path).ok();
        }
        let mut endpoint = UnixDatagramEndpoint::bind(
            "temp",
            &config_at(&vehicle_path, NetworkProtocol::UnixDgram),
        )
        .expect("the vehicle binds");
        endpoint
            .hear_only("temp", &SendsFrom::Path(device_path.display().to_string()))
            .expect("a path for a path");

        let device = UnixDatagram::bind(&device_path).expect("the device binds");
        let stranger = UnixDatagram::bind(&stranger_path).expect("a stranger binds");
        stranger
            .send_to(b"99.9", &vehicle_path)
            .expect("the stranger sends");
        device
            .send_to(b"21.5", &vehicle_path)
            .expect("the device sends");

        let mut into = [0u8; 64];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let n = loop {
            match endpoint.read(&mut into).expect("a read") {
                0 if std::time::Instant::now() < deadline => std::thread::yield_now(),
                n => break n,
            }
        };
        assert_eq!(
            &into[..n],
            b"21.5",
            "the stranger's was read as the device's"
        );
        assert_eq!(endpoint.strangers(), 1);

        for path in [&vehicle_path, &device_path, &stranger_path] {
            std::fs::remove_file(path).ok();
        }
    }

    /// A socket file left by a handler that was killed is replaced rather than
    /// refused: the path is the address, and a dead socket at it would stop
    /// every start from then on.
    #[test]
    fn a_dead_socket_file_is_replaced() {
        let path = a_path("stale");
        std::fs::remove_file(&path).ok();

        // Bound and then dropped, which leaves the file behind.
        drop(UnixDatagram::bind(&path).expect("a socket to abandon"));
        assert!(path.exists(), "the file should outlive the socket");

        let endpoint =
            UnixDatagramEndpoint::bind("beacon", &config_at(&path, NetworkProtocol::UnixDgram))
                .expect("a dead socket is not a reason to refuse");
        drop(endpoint);

        std::fs::remove_file(&path).ok();
    }

    /// A socket something is listening on is not displaced: two handlers at
    /// one path is a configuration to report, not to resolve by unlinking.
    #[test]
    fn a_live_socket_is_not_displaced() {
        let path = a_path("live");
        std::fs::remove_file(&path).ok();

        let live = UnixDatagram::bind(&path).expect("a socket that is listening");

        let e = match UnixDatagramEndpoint::bind(
            "beacon",
            &config_at(&path, NetworkProtocol::UnixDgram),
        ) {
            Ok(_) => panic!("it took the path of a live socket"),
            Err(e) => format!("{e}"),
        };
        assert!(
            e.contains("already listening"),
            "the refusal does not say what is there: {e}"
        );

        drop(live);
        std::fs::remove_file(&path).ok();
    }
}
