//! Unix-domain endpoints: sockets named by a path rather than an address.
//!
//! Two protocols, and they divide the way TCP and UDP divide. A stream is
//! accepted, so the connection itself says who is at each end and the payload
//! is the end that listens. A datagram is not, so -- as over UDP, and for the
//! same reason -- the handler is the end that waits at the configured path and
//! the payload reaches out to it, a payload producing data because it is
//! running rather than because it was asked.
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
use std::path::Path;

use log::info;
use nix::poll::PollFlags;
use tcslibgs::{NetworkConfig, TcsError, TcsResult};

use crate::ci::bind_failed;
use crate::config::constants::ENDPOINT_BUFFER_SIZE;
use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable, WaitResult,
};
use crate::endpoint_tcp::connect_retrying;

/// A Unix-domain stream: the payload listens and the handler connects.
pub struct UnixStreamEndpoint {
    stream: UnixStream,
    _buffer: Vec<u8>,
}

impl UnixStreamEndpoint {
    /// Connect to the payload's path, retrying while refused.
    ///
    /// A refusal means nothing is listening there yet, which is the ordinary
    /// case when a handler is started before its payload; see
    /// [`connect_retrying`]. A path with no socket at all is a different
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

/// A Unix-domain datagram socket: the handler waits and the payload sends.
///
/// The far end is learnt rather than configured, as it is over UDP: a
/// datagram's sender is the only statement of where it came from. A payload's
/// own path is its own business, so nothing here knows it until it writes.
pub struct UnixDatagramEndpoint {
    /// The handler this endpoint belongs to, as the payload configuration
    /// names it.
    what: String,
    socket: UnixDatagram,
    /// Where the far end last wrote from, if it has.
    ///
    /// Shared between the two endpoints of a conduit pair, so that the one
    /// writing sends where the one reading learnt.
    peer: std::sync::Arc<std::sync::Mutex<Option<std::os::unix::net::SocketAddr>>>,
    _buffer: Vec<u8>,
}

impl UnixDatagramEndpoint {
    /// Bind the configured path, replacing a socket nothing answers.
    pub fn bind(what: &str, config: &NetworkConfig) -> TcsResult<Self> {
        let path = config.address.clone();
        clear_a_dead_socket(what, Path::new(&path))?;

        let socket =
            UnixDatagram::bind(&path).map_err(|e| {
                bind_failed(&format!("{what} Unix datagram endpoint"), &path, e)
            })?;
        socket.set_nonblocking(true)?;

        Ok(Self {
            what: what.to_string(),
            socket,
            peer: std::sync::Arc::new(std::sync::Mutex::new(None)),
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }

    /// A second endpoint on the same socket, sharing what it learns.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            what: self.what.clone(),
            socket: self.socket.try_clone()?,
            peer: self.peer.clone(),
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
        // recv_from rather than recv, so that answering is possible.
        match self.socket.recv_from(buffer) {
            Ok((n, from)) => {
                if let Ok(mut peer) = self.peer.lock() {
                    *peer = Some(from);
                }
                Ok(n)
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(TcsError::Io(e)),
        }
    }
}

impl EndpointWritable for UnixDatagramEndpoint {
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        let peer = self
            .peer
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .ok_or_else(|| {
                TcsError::Endpoint(format!(
                    "{}: nothing has been received on this socket yet, so there is \
                     no path to send to",
                    self.what
                ))
            })?;

        // An unnamed sender has no path to be answered at. A payload that
        // never bound one cannot be written to, which is worth saying rather
        // than failing as though the write itself had gone wrong.
        let path = peer.as_pathname().ok_or_else(|| {
            TcsError::Endpoint(format!(
                "{}: the far end of this socket has no path of its own, so it cannot \
                 be sent to: a datagram payload binds a path in order to be answered",
                self.what
            ))
        })?;

        match self.socket.send_to(data, path) {
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(TcsError::Io(e)),
        }
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

    /// A stream endpoint connects to a payload that is listening, and carries
    /// bytes both ways.
    #[test]
    fn a_stream_endpoint_reaches_a_listening_payload() {
        let path = a_path("stream");
        std::fs::remove_file(&path).ok();

        // Standing in for tcssim: the payload listens at its path.
        let payload = UnixListener::bind(&path).expect("the payload listens");

        let mut endpoint = UnixStreamEndpoint::connect_retrying(
            "beacon",
            &config_at(&path, NetworkProtocol::UnixStream),
        )
        .expect("the handler connects");

        let (mut accepted, _) = payload.accept().expect("the payload accepts");

        accepted.write_all(b"from the payload").expect("it sends");
        let mut buffer = [0u8; 64];
        let mut got = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while got.len() < b"from the payload".len() && std::time::Instant::now() < deadline {
            match endpoint.read(&mut buffer) {
                Ok(n) if n > 0 => got.extend_from_slice(&buffer[..n]),
                _ => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        }
        assert_eq!(got, b"from the payload");

        endpoint.write(b"to the payload").expect("the handler sends");
        let mut back = [0u8; 64];
        let n = accepted.read(&mut back).expect("the payload reads");
        assert_eq!(&back[..n], b"to the payload");

        std::fs::remove_file(&path).ok();
    }

    /// A datagram endpoint binds its path, learns where the payload is from
    /// the first datagram, and answers there.
    #[test]
    fn a_datagram_endpoint_learns_where_its_payload_is() {
        let handler_path = a_path("dgram");
        let payload_path = a_path("dgram-payload");
        std::fs::remove_file(&handler_path).ok();
        std::fs::remove_file(&payload_path).ok();

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

        // Standing in for tcssim: the payload binds a path of its own, so
        // that it can be answered, and speaks first.
        let payload = UnixDatagram::bind(&payload_path).expect("the payload binds");
        payload
            .send_to(b"from the payload", &handler_path)
            .expect("the payload sends");

        let mut buffer = [0u8; 64];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut got = 0;
        while got == 0 && std::time::Instant::now() < deadline {
            got = endpoint.read(&mut buffer).expect("a read");
            if got == 0 {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        assert_eq!(&buffer[..got], b"from the payload");

        // And now it knows where to answer.
        endpoint.write(b"to the payload").expect("the handler sends");
        let mut back = [0u8; 64];
        let (n, _) = payload.recv_from(&mut back).expect("the payload reads");
        assert_eq!(&back[..n], b"to the payload");

        std::fs::remove_file(&handler_path).ok();
        std::fs::remove_file(&payload_path).ok();
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
