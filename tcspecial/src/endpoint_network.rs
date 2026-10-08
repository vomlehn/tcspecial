//! Address family, socket type, and protocol tables for network endpoints.
//!
//! Work in progress: only the unix family is filled in, and the commented-out
//! blocks below are the families still to be described.

use libc::{IPPROTO_TCP, IPPROTO_UDP, IPPROTO_SCTP, IPPROTO_UDPLITE};
use socket2::{Domain, Type};
use std::collections::BTreeMap;
use std::sync::LazyLock;

pub enum LinkType {
    Packet,
    Stream,
}

pub struct AddressFamily<'a> {
    pub af:         Domain,
    pub sock_info:  BTreeMap<&'a str, SockInfo<'a>>,
}

pub struct SockInfo<'a> {
    pub sock_type:  Type,
    pub ipproto:   BTreeMap<&'a str, i32>,
//    link_type:  LinkType,
}

pub static PROTOCOLS: LazyLock<BTreeMap<&str, AddressFamily>> = LazyLock::new(|| {
    BTreeMap::from([
        (
            "unix", AddressFamily {
                af: Domain::UNIX, sock_info: BTreeMap::from([
                    (
                        "stream", SockInfo {
                            sock_type: Type::STREAM, ipproto: BTreeMap::from([
                                ("tcp", IPPROTO_TCP),
                                ("udp", IPPROTO_UDP),
                                ("sctp", IPPROTO_SCTP),
                                ("udplite", IPPROTO_UDPLITE),
                                // FIXME: handle SOCK_RAW from RFC 1709
                            ])
                        }
                    )
                ])
            }
        )
    ])
});
        




//                SockInfo{sock_type: Type::STREAM, protocol: &[], 
//                    link_type: LinkType::Stream},
//            }])}
/*
                "dgram",  {type: SOCK_dgram, protocol: None, link_type: LinkType::Packet},
                "seqpacket", {type: SOCK_seqpacket, protocol: None, link_type: LinkType::Packet},
*/
/*
        {"local", AddressFamily(AF_local, 
                "stream", SOCK_stream, None, LinkType::Stream,
                "dgram",  SOCK_dgram, None, LinkType::Packet,
                "seqpacket", SOCK_seqpacket, None, LinkType::Packet,
            ],
        {"inet", AddressFamily(AF_inet, BTreeMap::from([
                "stream", SOCK_stream, None, LinkType:::Stream 
                "dgram", SOCK_dgram, None, :datagram
                "raw", SOCK_raw, None, LinkType::Packet,
            ],
*/
/*
        {"ax25", AddressFamily(AF_ax25, :tbd:tbd:tbd
        {"ipx", AddressFamily(AF_ipx, :tbd:tbd:tbd
        {"appletalk", AddressFamily(AF_appletalk, BTreeMap::from(["dgram", SOCK_dgram, None,:yes:datagram sock_raw:yes:datagram
        {"x25", AddressFamily(AF_x25, BTreeMap::from(["seqpacket", SOCK_seqpacket, None,:0:datagram
        {"inet6", AddressFamily(AF_inet6, BTreeMap::from(["stream", SOCK_stream, None,:yes:stream sock_dgram:yes:datagram sock_raw:yes:datagram
        {"decnet", AddressFamily(AF_decnet, :tbd:tbd:tbd
        {"key", AddressFamily(AF_key, :tbd:tbd:tbd
        {"netlink", AddressFamily(AF_netlink, BTreeMap::from(["dgram", SOCK_dgram, None,:yes:datagram sock_raw:yes:datagram
        {"packet", AddressFamily(AF_packet, BTreeMap::from(["dgram", SOCK_dgram, None,:yes:datagram sock_raw:yes:datagram
        {"rds", AddressFamily(AF_rds, :tbd:tbd:tbd
        {"pppox", AddressFamily(AF_pppox, :tbd:tbd:tbd
        {"llc", AddressFamily(AF_llc, :tbd:tbd:tbd
        {"ib", AddressFamily(AF_ib, :tbd:tbd:tbd
        {"mpls", AddressFamily(AF_mpls, :tbd:tbd:tbd
        {"can", AddressFamily(AF_can, :tbd:tbd:tbd
        {"tipc", AddressFamily(AF_tipc, :tbd:tbd:tbd
        {"bluetooth", AddressFamily(AF_bluetooth, :tbd:tbd:tbd
        {"alg", AddressFamily(AF_alg, :tbd:tbd:tbd
        {"vsock", AddressFamily(AF_vsock, BTreeMap::from(["dgram", SOCK_dgram, None,:yes:datagram sock_raw:yes:datagram
        {"xdp", AddressFamily(AF_xdp, :tbd:tbd:tbd
    ])
});
*/

pub struct Protocol<'a> {
    pub name:   &'a [&'static str],
}

impl Protocol<'_> {
    pub fn new<'a>(name: &'a[&'static str]) -> Protocol<'a>  {
        Protocol {
            name,
        }
    }
}


/*
AF_UNIX or AF_LOCAL	SOCK_STREAM	0	stream
SOCK_DGRAM	0	datagram
SOCK_SEQPACKET	0	datagram
AF_INET	SOCK_STREAM	0	stream
SOCK_DGRAM	0	datagram
SOCK_RAW	yes	datagram
AF_AX25	TBD	TBD	TBD
AF_IPX	TBD	TBD	TBD
AF_APPLETALK	SOCK_DGRAM	yes	datagram
SOCK_RAW	yes	datagram
AF_X25	SOCK_SEQPACKET	0	datagram
AF_INET6	SOCK_STREAM	yes	stream
SOCK_DGRAM	yes	datagram
SOCK_RAW	yes	datagram
AF_DECnet	TBD	TBD	TBD
AF_KEY	TBD	TBD	TBD
AF_NETLINK	SOCK_DGRAM	yes	datagram
SOCK_RAW	yes	datagram
AF_PACKET	SOCK_DGRAM	yes	datagram
SOCK_RAW	yes	datagram
AF_RDS	TBD	TBD	TBD
AF_PPPOX	TBD	TBD	TBD
AF_LLC	TBD	TBD	TBD
AF_IB	TBD	TBD	TBD
AF_MPLS	TBD	TBD	TBD
AF_CAN	TBD	TBD	TBD
AF_TIPC	TBD	TBD	TBD
AF_BLUETOOTH	TBD	TBD	TBD
AF_ALG	TBD	TBD	TBD
AF_VSOCK	SOCK_DGRAM	yes	datagram
SOCK_RAW	yes	datagram
AF_XDP	TBD	TBD	TBD
*/

// Notes kept from working out the tables above: how the socket calls this
// module describes are spelled in socket2, and what they correspond to in C.
/*
 * use socket2::{Socket, Domain, Type, Protocol, SockAddr};
use std::net::SocketAddr;

fn main() -> std::io::Result<()> {
    // Equivalent to: socket(AF_INET, SOCK_STREAM, IPPROTO_TCP)
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;

    // Set options before connecting
    socket.set_reuse_address(true)?;
    socket.set_nodelay(true)?;

    // Connect
    let addr: SocketAddr = "127.0.0.1:7878".parse().unwrap();
    socket.connect(&SockAddr::from(addr))?;

    // Send/receive
    socket.send(b"Hello!")?;

    let mut buf = [0u8; 1024];
    let n = socket.recv(&mut buf)?;
    println!("Received: {}", String::from_utf8_lossy(&buf[..n]));

    Ok(())
}

use socket2::{Socket, Domain, Type, Protocol, SockAddr};
use std::net::SocketAddr;

fn main() -> std::io::Result<()> {
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;

    socket.set_reuse_address(true)?;

    let addr: SocketAddr = "127.0.0.1:7878".parse().unwrap();
    socket.bind(&SockAddr::from(addr))?;
    socket.listen(128)?; // backlog of 128

    let (client, client_addr) = socket.accept()?;
    println!("Connection from: {:?}", client_addr.as_socket());

    Ok(())
}

let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
socket.bind(&SockAddr::from(addr))?;
socket.send_to(b"ping", &SockAddr::from(remote_addr))?;

Key mappings to C
C                                           socket2
socket(AF_INET, SOCK_STREAM, 0) Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))

socket(AF_INET6, SOCK_DGRAM, 0) Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP))

socket(AF_UNIX, SOCK_STREAM, 0) Socket::new(Domain::UNIX, Type::STREAM, None)

setsockopt(...)                 socket.set_reuse_address(true), etc.

bind(), listen(), accept(), connect()   Same method names on Socket

Converting to std types
You can convert a socket2::Socket into a std::net::TcpStream (or TcpListener, UdpSocket) when you're done with low-level setup:

let std_stream: std::net::TcpStream = socket.into();

This is a common pattern: use socket2 for fine-grained control during setup, then convert to std types for ergonomic I/O.
 */
