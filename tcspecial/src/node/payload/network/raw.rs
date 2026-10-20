//! The raw endpoint: IP datagrams of one protocol number, on a raw socket.
//!
//! `protocol: raw` with an `ip_protocol` -- `SOCK_RAW` in the family of the
//! payload's address, opened with that number. For a payload that speaks IP
//! itself and nothing above it: no UDP header, no ports, the protocol number
//! being what says which datagrams are its, and IANA's list being where the
//! numbers are given out (253 and 254 are the two set aside for experiments).
//!
//! # Connected, both ways
//!
//! The socket is connected to the payload's address, and that is the whole of
//! the arrangement. A connected raw socket is handed only the datagrams of its
//! protocol that come from that address, and sends to it; there is no port to
//! bind and nobody has to speak first, so the question that divides UDP --
//! which end waits -- does not arise, and a driven payload, a read one and a
//! triggered one are all opened the same way.
//!
//! # What a read hands over
//!
//! The payload's bytes, which is what every other endpoint hands over. An IPv4
//! raw socket gives each datagram with its IP header in front -- and an IPv6
//! one without -- so the header is taken off here, in the caller's buffer,
//! rather than being passed on for every node downstream to know about. A
//! write is the payload's bytes too: the kernel puts the header on.
//!
//! # Privilege
//!
//! A raw socket needs `CAP_NET_RAW`. Without it the socket is refused as the
//! node opens, and the refusal says so in those terms, since `Operation not
//! permitted` names neither the capability nor the node.

use std::io::{self, Read, Write};
use std::net::{SocketAddr, ToSocketAddrs};
use std::os::unix::io::{AsRawFd, RawFd};

use nix::poll::PollFlags;
use socket2::{Domain, Protocol, Socket, Type};
use tcslibgs::{NetworkConfig, NetworkProtocol, TcsError, TcsResult};

use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable, WaitResult,
};

/// A raw socket connected to one payload.
pub struct RawEndpoint {
    /// The node this endpoint is for, which every complaint names.
    what: String,
    socket: Socket,
    /// Whether each datagram read arrives with an IPv4 header in front of it.
    v4: bool,
}

impl RawEndpoint {
    /// Open a raw socket of the node's protocol, connected to its address.
    pub fn connect(what: &str, config: &NetworkConfig) -> TcsResult<Self> {
        let NetworkProtocol::Raw(protocol) = config.protocol else {
            return Err(TcsError::Config(format!(
                "{what}: a raw endpoint was asked for a node reached over {}",
                config.protocol.spelling()
            )));
        };

        // No port: the number is the address family's business, and nought
        // is what an address with none says.
        let to: SocketAddr = (config.address.as_str(), 0)
            .to_socket_addrs()
            .ok()
            .and_then(|mut found| found.next())
            .ok_or_else(|| {
                TcsError::Config(format!(
                    "{what}: \"{}\" is where the configuration says the payload is, \
                     and it does not resolve to an address",
                    config.address
                ))
            })?;

        let socket = Socket::new(
            Domain::for_address(to),
            Type::from(libc::SOCK_RAW),
            Some(Protocol::from(i32::from(protocol.number()))),
        )
        .map_err(|e| match e.raw_os_error() {
            Some(libc::EPERM) | Some(libc::EACCES) => TcsError::Config(format!(
                "{what}: a raw socket for IP protocol {protocol} cannot be opened \
                 without CAP_NET_RAW, which this tcspecial has not got: give it \
                 the capability (setcap cap_net_raw+ep on the executable), or \
                 reach the payload over udp"
            )),
            _ => TcsError::Config(format!(
                "{what}: a raw socket for IP protocol {protocol} cannot be opened: {e}"
            )),
        })?;
        socket.connect(&to.into()).map_err(|e| {
            TcsError::Config(format!(
                "{what}: a raw socket cannot be pointed at {}: {e}",
                config.address
            ))
        })?;
        socket.set_nonblocking(true)?;

        Ok(Self {
            what: what.to_string(),
            socket,
            v4: to.is_ipv4(),
        })
    }

    /// A second endpoint on the same socket, for the other half of the pair.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            what: self.what.clone(),
            socket: self.socket.try_clone()?,
            v4: self.v4,
        })
    }
}

/// Take the IPv4 header off the `got` bytes of a datagram at the front of
/// `buffer`, leaving its payload at the front, and say how long that is.
///
/// The header's length is in it -- the low four bits of its first byte, in
/// words of four bytes -- since options make it longer than twenty. A
/// datagram shorter than the header it claims is refused rather than handed
/// on as data.
fn without_ipv4_header(what: &str, buffer: &mut [u8], got: usize) -> TcsResult<usize> {
    let header = buffer
        .first()
        .map(|first| usize::from(first & 0x0f) * 4)
        .unwrap_or(0);
    if got == 0 {
        return Ok(0);
    }
    if header < 20 || header > got {
        return Err(TcsError::Endpoint(format!(
            "{what}: a {got}-byte datagram arrived with a {header}-byte IPv4 header, \
             which is not a datagram"
        )));
    }
    buffer.copy_within(header..got, 0);
    Ok(got - header)
}

impl EndpointWaitable for RawEndpoint {
    fn io_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }

    fn wait_for_event(&self, cmd_fd: RawFd, timeout_ms: i32) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_fd(), cmd_fd, PollFlags::POLLIN, timeout_ms)
    }
}

impl EndpointReadable for RawEndpoint {
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        match (&self.socket).read(buffer) {
            Ok(got) if self.v4 => without_ipv4_header(&self.what, buffer, got),
            Ok(got) => Ok(got),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(TcsError::Io(e)),
        }
    }
}

impl EndpointWritable for RawEndpoint {
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        match (&self.socket).write(data) {
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(TcsError::Io(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslibgs::IpProtocol;

    fn raw(address: &str) -> NetworkConfig {
        NetworkConfig {
            protocol: NetworkProtocol::Raw(IpProtocol::of(253).expect("a protocol")),
            address: address.to_string(),
            port: 0,
        }
    }

    /// What a read hands over is the payload, the IPv4 header off it, however
    /// long the header is.
    #[test]
    fn an_ipv4_header_is_taken_off_what_is_read() {
        // Twenty bytes of header, the plainest there is, and three of payload.
        let mut buffer = [0u8; 64];
        buffer[0] = 0x45;
        buffer[20..23].copy_from_slice(b"abc");
        assert_eq!(
            without_ipv4_header("n", &mut buffer, 23).expect("a datagram"),
            3
        );
        assert_eq!(&buffer[..3], b"abc");

        // Twenty-four, with an option in it.
        let mut buffer = [0u8; 64];
        buffer[0] = 0x46;
        buffer[24..26].copy_from_slice(b"xy");
        assert_eq!(
            without_ipv4_header("n", &mut buffer, 26).expect("a datagram"),
            2
        );
        assert_eq!(&buffer[..2], b"xy");

        // Shorter than the header it claims.
        let mut buffer = [0u8; 64];
        buffer[0] = 0x45;
        let said = without_ipv4_header("n", &mut buffer, 10).expect_err("not a datagram");
        assert!(said.to_string().contains("n:"), "{said}");
    }

    /// Opened without the privilege a raw socket needs, the node is refused
    /// in those terms; with it, the socket is opened and connected.
    ///
    /// Which of the two happens depends on who runs the test, so the test
    /// asks of whichever it is.
    #[test]
    fn a_raw_socket_is_opened_or_refused_for_want_of_cap_net_raw() {
        match RawEndpoint::connect("sounder", &raw("127.0.0.1")) {
            Ok(endpoint) => {
                assert!(endpoint.v4);
                endpoint.try_clone().expect("a second endpoint on it");
            }
            Err(e) => {
                let said = e.to_string();
                assert!(said.contains("sounder"), "{said}");
                assert!(said.contains("CAP_NET_RAW"), "{said}");
            }
        }
    }

    /// An address that is not one is refused naming it.
    #[test]
    fn an_address_that_does_not_resolve_is_refused() {
        let said = RawEndpoint::connect("sounder", &raw("no-such-host.invalid"))
            .err()
            .expect("refused")
            .to_string();
        assert!(said.contains("no-such-host.invalid"), "{said}");
    }

    /// Where the test is run with CAP_NET_RAW, a datagram sent to this host
    /// on an experimental protocol arrives as its payload alone.
    #[test]
    fn a_datagram_arrives_as_its_payload_where_raw_sockets_can_be_opened() {
        let Ok(mut here) = RawEndpoint::connect("here", &raw("127.0.0.1")) else {
            eprintln!("no CAP_NET_RAW: a raw datagram cannot be sent from this test");
            return;
        };
        let mut there = here.try_clone().expect("a second endpoint");
        there.write(b"sounding").expect("sent");
        let mut buffer = [0u8; 256];
        let by = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let got = here.read(&mut buffer).expect("read");
            if got > 0 {
                assert_eq!(&buffer[..got], b"sounding");
                return;
            }
            assert!(std::time::Instant::now() < by, "nothing arrived");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
