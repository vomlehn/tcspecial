//! Sockets: what the standard library's do not offer -- a datagram's whole
//! length, a datagram discarded without waiting -- and, on Linux alone,
//! Unix-domain sockets, named by a path, or by a name in Linux's abstract
//! namespace, rather than by an address. A browser's sockets are all the
//! page's own, and named by address.
//!
//! For Unix-domain sockets the standard library's types are the interface,
//! re-exported here so that no other crate names `std::os::unix`; what it
//! has no call for -- Linux's autobind, a socket file's kind -- is here
//! beside them.

pub use std::os::unix::net::{
    SocketAddr, UnixDatagram, UnixListener, UnixStream,
};

/// Binds a socket to a name the kernel chooses in the abstract namespace.
///
/// Linux's autobind: a bind that gives the address family and no name. The
/// name is unique, so two sockets never contend for one, and it is not a
/// file, so nothing outlives the socket. The standard library has no call
/// for it.
///
/// # Arguments
///
/// * `socket` - The unbound socket to bind.
///
/// # Errors
///
/// What `bind` failed with.
pub fn autobind(socket: &UnixDatagram) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;

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
        Err(std::io::Error::last_os_error())
    }
}

/// The name a socket address has in the abstract namespace, where it has
/// one.
///
/// # Arguments
///
/// * `address` - The address.
///
/// # Returns
///
/// The name's bytes, or `None` for a path or an unnamed socket.
pub fn abstract_name(address: &SocketAddr) -> Option<&[u8]> {
    use std::os::linux::net::SocketAddrExt;

    address.as_abstract_name()
}

/// A socket address in the abstract namespace.
///
/// # Arguments
///
/// * `name` - The name's bytes.
///
/// # Returns
///
/// The address.
///
/// # Errors
///
/// What the standard library said of a name too long.
pub fn abstract_address(name: &[u8]) -> std::io::Result<SocketAddr> {
    use std::os::linux::net::SocketAddrExt;

    SocketAddr::from_abstract_name(name)
}

/// Whether what the file system says of a file is that it is a socket.
///
/// # Arguments
///
/// * `found` - The file's metadata.
///
/// # Returns
///
/// `true` where it is one.
pub fn is_a_socket(found: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::FileTypeExt;

    found.file_type().is_socket()
}

/// The system's numbers for address families and socket types, for code
/// that converts its own names for them to the system's.
pub mod numbers {
    pub use libc::{
        AF_DECnet, AF_ALG, AF_APPLETALK, AF_AX25, AF_BLUETOOTH, AF_CAN, AF_IB,
        AF_INET, AF_INET6, AF_IPX, AF_KEY, AF_LLC, AF_MPLS, AF_NETLINK,
        AF_PACKET, AF_PPPOX, AF_RDS, AF_TIPC, AF_UNIX, AF_VSOCK, AF_X25,
        AF_XDP, SOCK_DGRAM, SOCK_RAW, SOCK_SEQPACKET, SOCK_STREAM,
    };
}

/// Reads one datagram, and says how long it was rather than how much of it
/// fitted, so that one cut short is told from one that happened to fill
/// the buffer.
///
/// # Arguments
///
/// * `socket` - The socket.
/// * `into`   - Where as much of it as fits goes.
///
/// # Returns
///
/// The datagram's whole length, which may be more than `into` holds.
///
/// # Errors
///
/// What the system said.
pub fn receive_whole_length(
    socket: &std::net::UdpSocket,
    into: &mut [u8],
) -> std::io::Result<usize> {
    use std::os::fd::AsRawFd;

    // SAFETY: recv needs an open socket and a buffer writable for the length
    // it is given: the socket is borrowed, and the buffer and its length are
    // both `into`'s. MSG_TRUNC changes what is returned, not how much is
    // written.
    let got = unsafe {
        libc::recv(
            socket.as_raw_fd(),
            into.as_mut_ptr().cast(),
            into.len(),
            libc::MSG_TRUNC,
        )
    };
    usize::try_from(got).map_err(|_| std::io::Error::last_os_error())
}

/// Takes one datagram that is waiting on a socket and throws it away,
/// without waiting and without changing how the socket's other readers
/// read it.
///
/// # Arguments
///
/// * `source` - The socket, which its owner keeps open for the call.
///
/// # Returns
///
/// Whether one was waiting.
///
/// # Errors
///
/// What the system said, other than that nothing was waiting.
pub fn discard_one(
    source: crate::linux::wait::Source,
) -> std::io::Result<bool> {
    // One byte of room: with MSG_TRUNC the rest of a datagram is discarded,
    // which is all that is wanted of it.
    let mut room = [0u8; 1];
    loop {
        // SAFETY: recv needs an open socket and a buffer writable for the
        // length it is given; the caller keeps the socket open, and the
        // buffer is `room`, with its length.
        let got = unsafe {
            libc::recv(
                source.raw_fd(),
                room.as_mut_ptr().cast(),
                room.len(),
                libc::MSG_DONTWAIT | libc::MSG_TRUNC,
            )
        };
        if got >= 0 {
            return Ok(true);
        }
        let e = std::io::Error::last_os_error();
        match e.kind() {
            std::io::ErrorKind::Interrupted => continue,
            std::io::ErrorKind::WouldBlock => return Ok(false),
            _ => return Err(e),
        }
    }
}
