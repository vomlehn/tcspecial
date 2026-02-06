mod EndpointNetwork {
use std::collections::BTreeMap;
use std::sync::LazyLock;

enum LinkType {
    Packet,
    Stream,
}


static protocols: LazyLock<BTreeMap<&[&'static str], i32>> = LazyLock::new(|| {
    BTreeMap::from([
        (&["unix", "local"], 12),
		(&["inet"], 23),
		(&["ax25"], 23),
		(&["ipx"], 23),
		(&["appletalk"], 23),
		(&["x25"], 23),
		(&["inet6"], 23),
		(&["decnet"], 23),
		(&["key"], 23),
		(&["netlink"], 23),
		(&["packet"], 23),
		(&["rds"], 23),
		(&["pppox"], 23),
		(&["llc"], 23),
		(&["ib"], 23),
		(&["mpls"], 23),
		(&["can"], 23),
		(&["tipc"], 23),
		(&["bluetooth"], 23),
		(&["alg"], 23),
		(&["vsock"], 23),
		(&["xdp"], 23),
    ])
});

struct Protocol<'a> {
    name:   &'a [&'static str],
}

impl Protocol<'_> {
    fn new<'a>(name: &'a[&'static str]) -> Protocol<'a>  {
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
}
