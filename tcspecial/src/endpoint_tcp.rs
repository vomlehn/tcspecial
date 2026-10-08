//! The TCP endpoint: a stream, listened for or connected to.
//!
//! The one kind of endpoint where the link itself says who is at the far end:
//! a stream has to be accepted before anything passes either way. A stream
//! payload therefore listens and the handler connects to it, which is also
//! the only kind of endpoint that can be refused -- and refusal is ordinary
//! rather than a fault, so it is waited out here.
//!
//! See [`crate::endpoint`] for the traits.

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::{AsRawFd, RawFd};
use std::thread;
use std::time::Instant;

use log::info;
use nix::poll::PollFlags;
use tcslibgs::{NetworkConfig, TcsError, TcsResult};

use crate::ci::bind_failed;
use crate::config::constants::{
    ENDPOINT_BUFFER_SIZE, ENDPOINT_CONNECT_BUDGET, ENDPOINT_DELAY_INIT, ENDPOINT_DELAY_MAX,
};
use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable, WaitResult,
};

/// TCP endpoint for stream communication
pub struct TcpEndpoint {
    stream: Option<TcpStream>,
    listener: Option<TcpListener>,
    _buffer: Vec<u8>,
    _is_server: bool,
}

impl TcpEndpoint {
    pub fn new_server(config: &NetworkConfig) -> TcsResult<Self> {
        let addr = format!("{}:{}", config.address, config.port);
        let listener =
            TcpListener::bind(&addr).map_err(|e| bind_failed("TCP endpoint", &addr, e))?;
        listener.set_nonblocking(true)?;

        Ok(Self {
            stream: None,
            listener: Some(listener),
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
            _is_server: true,
        })
    }

    /// A second endpoint on the same listener or stream.
    ///
    /// See [`UdpEndpoint::try_clone`]: one bind, two handles.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            stream: match &self.stream {
                Some(stream) => Some(stream.try_clone()?),
                None => None,
            },
            listener: match &self.listener {
                Some(listener) => Some(listener.try_clone()?),
                None => None,
            },
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
            _is_server: self._is_server,
        })
    }

    pub fn new_client(config: &NetworkConfig) -> TcsResult<Self> {
        let addr = format!("{}:{}", config.address, config.port);
        let stream = TcpStream::connect(&addr)?;
        stream.set_nonblocking(true)?;

        Ok(Self {
            stream: Some(stream),
            listener: None,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
            _is_server: false,
        })
    }

    /// Connect as a client, retrying while refused.
    ///
    /// See [`connect_retrying`]. A handler may be started before the payload
    /// it reaches, and a payload that is not listening yet refuses rather than
    /// failing in any way that waiting cannot fix.
    pub fn connect_retrying(config: &NetworkConfig) -> TcsResult<Self> {
        let addr = format!("{}:{}", config.address, config.port);
        let stream = connect_retrying(&addr, || TcpStream::connect(&addr))?;
        stream.set_nonblocking(true)?;

        Ok(Self {
            stream: Some(stream),
            listener: None,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
            _is_server: false,
        })
    }

    pub fn accept(&mut self) -> TcsResult<bool> {
        if let Some(ref listener) = self.listener {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(true)?;
                    self.stream = Some(stream);
                    Ok(true)
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(false),
                Err(e) => Err(TcsError::Io(e)),
            }
        } else {
            Ok(false)
        }
    }

    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }
}

impl EndpointWaitable for TcpEndpoint {
    fn io_fd(&self) -> RawFd {
        if let Some(ref stream) = self.stream {
            stream.as_raw_fd()
        } else if let Some(ref listener) = self.listener {
            listener.as_raw_fd()
        } else {
            -1
        }
    }

    fn wait_for_event(&self, cmd_fd: RawFd, timeout_ms: i32) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_fd(), cmd_fd, PollFlags::POLLIN, timeout_ms)
    }
}

impl EndpointReadable for TcpEndpoint {
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        if let Some(ref mut stream) = self.stream {
            match stream.read(buffer) {
                Ok(n) => Ok(n),
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
                Err(e) => Err(TcsError::Io(e)),
            }
        } else {
            Ok(0)
        }
    }
}

impl EndpointWritable for TcpEndpoint {
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        if let Some(ref mut stream) = self.stream {
            match stream.write(data) {
                Ok(n) => Ok(n),
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
                Err(e) => Err(TcsError::Io(e)),
            }
        } else {
            Ok(0)
        }
    }
}

/// Reach `addr`, retrying while the connection is refused.
///
/// A refusal means nothing is listening yet, which is the ordinary case when a
/// handler is started before the payload it serves: the simulated payload is a
/// program someone has to press Start on. So the attempt is repeated, with the
/// delay doubling from [`ENDPOINT_DELAY_INIT`] and never exceeding
/// [`ENDPOINT_DELAY_MAX`], until [`ENDPOINT_CONNECT_BUDGET`] is spent.
///
/// Only a refusal is retried. An address that cannot be resolved, or a network
/// that cannot be reached, will not become right by being asked again, and
/// repeating those would turn a clear fault into a slow one.
pub(crate) fn connect_retrying<T>(
    addr: &str,
    mut attempt: impl FnMut() -> io::Result<T>,
) -> TcsResult<T> {
    let deadline = Instant::now() + ENDPOINT_CONNECT_BUDGET;
    let mut delay = ENDPOINT_DELAY_INIT;
    let mut refusals = 0u32;

    loop {
        match attempt() {
            Ok(opened) => {
                if refusals > 0 {
                    info!(
                        "reached {addr} after {refusals} refusal(s): the far end was \
                         not listening yet"
                    );
                }
                return Ok(opened);
            }
            Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
                refusals += 1;
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err(TcsError::Config(format!(
                        "cannot reach {addr}: connection refused for {:?}. Nothing is \
                         listening there -- a simulated payload has to be started \
                         before the handler that reaches it",
                        ENDPOINT_CONNECT_BUDGET
                    )));
                }
                thread::sleep(delay.min(left));
                delay = (delay * 2).min(ENDPOINT_DELAY_MAX);
            }
            Err(e) => return Err(TcsError::Config(format!("cannot reach {addr}: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A refused connection is retried until the far end is listening.
    ///
    /// This is the ordinary case: a handler may be started before the payload
    /// it reaches, because the simulated payload is a program someone has to
    /// press Start on. The listener here appears after the first attempt must
    /// already have failed.
    #[test]
    fn a_refused_connection_is_retried_until_it_is_accepted() {
        use std::net::{TcpListener, TcpStream};
        use std::time::{Duration, Instant};
        use tcslibgs::NetworkProtocol;

        // A port with nothing on it yet, which is what refuses.
        let probe = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = probe.local_addr().unwrap();
        drop(probe);

        let listening = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            let listener = TcpListener::bind(addr).expect("the far end starts late");
            listener.accept().expect("and accepts");
        });

        let config = NetworkConfig {
            protocol: NetworkProtocol::Tcp,
            address: addr.ip().to_string(),
            port: addr.port(),
        };

        let started = Instant::now();
        let endpoint = TcpEndpoint::connect_retrying(&config)
            .expect("a refusal should be waited out, not reported");
        let waited = started.elapsed();

        assert!(endpoint.is_connected());
        assert!(
            waited >= Duration::from_millis(300),
            "it cannot have connected before the far end was listening: {waited:?}"
        );

        drop(endpoint);
        listening.join().unwrap();
        // Quiet the unused-import warning when the type is only named above.
        let _ = TcpStream::connect(addr);
    }

    /// A far end that never listens is reported, and promptly.
    ///
    /// Promptness matters: StartDH is answered on the command interpreter's
    /// own thread, so a handler that waited longer than the ground's command
    /// timeout would leave the ground with nothing rather than an answer.
    #[test]
    fn a_connection_nothing_ever_accepts_is_reported() {
        use std::net::TcpListener;
        use std::time::Instant;
        use tcslibgs::NetworkProtocol;

        let probe = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = probe.local_addr().unwrap();
        drop(probe);

        let config = NetworkConfig {
            protocol: NetworkProtocol::Tcp,
            address: addr.ip().to_string(),
            port: addr.port(),
        };

        let started = Instant::now();
        let message = match TcpEndpoint::connect_retrying(&config) {
            Ok(_) => panic!("nothing is listening, so this must fail"),
            Err(e) => e.to_string(),
        };
        let waited = started.elapsed();

        assert!(
            message.contains("refused") && message.contains("started before"),
            "the error should say what to do about it, but said: {message}"
        );
        assert!(
            waited < ENDPOINT_CONNECT_BUDGET * 2,
            "it should give up near its budget, but waited {waited:?}"
        );
    }
}
