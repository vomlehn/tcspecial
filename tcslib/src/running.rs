//! That a ground station's loop, once running, allocates nothing: a beacon
//! pass, a datum from a `moc` node, a ping and its answer, and a command
//! sent. A beacon pass allocates only what unpacking the beacon does, which
//! `tcslibgs::Beacon::read` says, and why.
//!
//! One test binary has one allocator, so these are together, under the one
//! that counts.

use std::cell::Cell;
use std::net::UdpSocket;
use std::time::Duration;

use tcslibgs::graph::Graph;
use tcslibgs::{
    Beacon, Command, ConfigDigest, ConfigVersion, HostPort, PingCommand,
};

use crate::beacon::{Arrived, Beacons, Link, Schedule};
use crate::connection::{Connection, UdpConnection};
use crate::moc::{Moc, DATUM_BYTES};
use crate::ping::Pinger;

/// The process's allocator, counting what one thread asks of it while that
/// thread says to.
mod counting {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        pub static COUNTING: Cell<bool> = const { Cell::new(false) };
        pub static COUNT: Cell<usize> = const { Cell::new(0) };
    }

    pub struct Counting;

    fn note() {
        if COUNTING.try_with(Cell::get).unwrap_or(false) {
            let _ = COUNT.try_with(|count| count.set(count.get() + 1));
        }
    }

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            note();
            System.alloc(layout)
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            note();
            System.alloc_zeroed(layout)
        }
        unsafe fn realloc(
            &self,
            ptr: *mut u8,
            layout: Layout,
            size: usize,
        ) -> *mut u8 {
            note();
            System.realloc(ptr, layout, size)
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            System.dealloc(ptr, layout)
        }
    }

    #[global_allocator]
    static ALLOCATOR: Counting = Counting;
}

/// How many allocations `work` made on this thread.
fn allocations_in(work: impl FnOnce()) -> usize {
    counting::COUNT.with(|count| count.set(0));
    counting::COUNTING.with(|on| on.set(true));
    work();
    counting::COUNTING.with(|on| on.set(false));
    counting::COUNT.with(Cell::get)
}

/// A port nothing holds.
fn a_free_port() -> u16 {
    UdpSocket::bind("127.0.0.1:0")
        .expect("a port")
        .local_addr()
        .expect("its address")
        .port()
}

/// How many allocations unpacking one beacon makes: tcs-xtce's Rust decoder
/// reads the name and the space as Strings and the digest as a Vec. See
/// `tcslibgs::Beacon::read`. Should tcs-xtce come to decode into fixed
/// storage, as its C decoder does, this is nought and the test below says so.
const UNPACKING_ALLOCATES: usize = 3;

/// Once the first has been heard, a beacon pass allocates nothing but what
/// unpacking the beacon does: not for what the beacon said, nor for the pass
/// it hands back.
#[test]
fn a_beacon_pass_allocates_nothing_but_the_unpacking() {
    let vehicle = UdpSocket::bind("127.0.0.1:0").expect("the vehicle's socket");
    let from = vehicle.local_addr().expect("its address");
    let port = a_free_port();
    let link = Link {
        send: HostPort {
            address: "127.0.0.1".to_string(),
            port: from.port(),
        },
        recv: HostPort {
            address: "127.0.0.1".to_string(),
            port,
        },
    };
    let digest = ConfigDigest([7; 16]);
    let Ok(mut beacons) =
        Beacons::listening(link, Schedule::standard(), Some(digest))
    else {
        // The port was taken between being chosen and being bound.
        return;
    };
    let beacon = |sequence: u32| {
        Beacon::new(sequence, ConfigVersion::of_this_build(), digest).to_bytes()
    };
    let datagrams: Vec<Vec<u8>> = (1..=23).map(beacon).collect();

    // Where to, worked out once: a name and a port is looked up each time
    // it is sent to, which allocates.
    let to = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let mut sequence = datagrams.iter();
    let mut heard = |beacons: &mut Beacons| {
        let datagram = sequence.next().expect("a beacon to send");
        vehicle.send_to(datagram, to).expect("sent");
        let pass = beacons.pass();
        assert_eq!(pass.arrived, Arrived::Beacon);
        assert!(pass.remarks.is_empty(), "{:?}", pass.remarks);
        assert!(pass.said.starts_with("tcspecial v"), "{}", pass.said);
    };
    for _ in 0..3 {
        heard(&mut beacons);
    }
    let made = allocations_in(|| {
        for _ in 0..20 {
            heard(&mut beacons);
        }
    });
    assert_eq!(
        made,
        20 * UNPACKING_ALLOCATES,
        "twenty beacon passes allocated {made} times, where unpacking twenty \
         beacons allocates {}",
        20 * UNPACKING_ALLOCATES
    );
}

/// A datum received into a buffer made beforehand allocates nothing.
#[test]
fn a_datum_received_into_a_buffer_allocates_nothing() {
    let port = a_free_port();
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("vehicle.yaml");
    std::fs::write(
        &path,
        format!(
            "moc:
  - {{ name: ground, type: moc, ground_base: \"127.0.0.1:{}\" }}
payloads:
  - {{ name: a, type: network, protocol: udp, address: \"127.0.0.1\", \
    port: \"7001\", payload-out: ground }}
",
            port - 1
        ),
    )
    .expect("written");
    let graph = Graph::load(&path).expect("it reads");
    let Ok(mut moc) = Moc::open(&graph, "ground") else {
        return;
    };
    let vehicle = UdpSocket::bind("127.0.0.1:0").expect("the vehicle's socket");
    let mut buffer = vec![0u8; DATUM_BYTES];
    let to = std::net::SocketAddr::from(([127, 0, 0, 1], port));

    let mut round = |moc: &mut Moc| {
        vehicle.send_to(b"293.15", to).expect("sent");
        let arrival = moc
            .receive_into(Some(Duration::from_secs(5)), &mut buffer)
            .expect("it reads")
            .expect("it arrives");
        assert_eq!(moc.source(arrival.source), Some("a"));
        assert_eq!(&buffer[..arrival.length], b"293.15");
    };
    round(&mut moc);
    let made = allocations_in(|| {
        for _ in 0..20 {
            round(&mut moc);
        }
    });
    assert_eq!(made, 0, "twenty data allocated {made} times");
}

/// A datum longer than the buffer is refused, not cut short.
#[test]
fn a_datum_longer_than_the_buffer_is_refused() {
    let port = a_free_port();
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("vehicle.yaml");
    std::fs::write(
        &path,
        format!(
            "moc:
  - {{ name: ground, type: moc, ground_base: \"127.0.0.1:{}\" }}
payloads:
  - {{ name: a, type: network, protocol: udp, address: \"127.0.0.1\", \
    port: \"7001\", payload-out: ground }}
",
            port - 1
        ),
    )
    .expect("written");
    let graph = Graph::load(&path).expect("it reads");
    let Ok(mut moc) = Moc::open(&graph, "ground") else {
        return;
    };
    let vehicle = UdpSocket::bind("127.0.0.1:0").expect("the vehicle's socket");
    vehicle
        .send_to(b"293.15", ("127.0.0.1", port))
        .expect("sent");
    let mut small = [0u8; 3];
    let why = moc
        .receive_into(Some(Duration::from_secs(5)), &mut small)
        .expect_err("six bytes do not fit in three");
    assert!(why.contains("sent 6 bytes"), "{why}");
}

/// Pinging, and folding in the answer, allocate nothing once the first of
/// each has been made.
#[test]
fn pinging_allocates_nothing() {
    let mut pinger = Pinger::new();
    let mut answer = [0u8; tcslibgs::node::vehicle::ping::REPLY_BYTES];
    let mut at = Duration::new(1_760_900_000, 0);

    let mut round = |pinger: &mut Pinger| {
        at += Duration::from_millis(1);
        let sent = pinger.ping(at).expect("a ping");
        let reply = tcslibgs::node::vehicle::ping::reply(sent.as_bytes(), at)
            .expect("answered");
        let length = reply.as_bytes().len();
        answer[..length].copy_from_slice(reply.as_bytes());
        pinger.answered(&answer[..length], at + Duration::from_millis(1));
        assert_eq!(pinger.exchange().round_trip(), Some(1_000_000));
    };
    // From the first ping: the pinger's room is made with it, and twenty
    // takes the count from one digit to two.
    let made = allocations_in(|| {
        for _ in 0..20 {
            round(&mut pinger);
        }
    });
    assert_eq!(made, 0, "twenty pings allocated {made} times");
}

/// Sending a command allocates nothing once the first has been sent.
#[test]
fn sending_a_command_allocates_nothing() {
    let vehicle = UdpSocket::bind("127.0.0.1:0").expect("the vehicle's socket");
    let at = vehicle.local_addr().expect("its address");
    let mut link = UdpConnection::new("127.0.0.1:0", &at.to_string())
        .expect("a connection");
    let command = Command::Ping(PingCommand::new(1));
    let mut taken = [0u8; 1024];

    let mut round = |link: &mut UdpConnection| {
        link.send(&command).expect("sent");
        vehicle.recv(&mut taken).expect("it arrives");
    };
    round(&mut link);
    let made = allocations_in(|| {
        for _ in 0..20 {
            round(&mut link);
        }
    });
    assert_eq!(made, 0, "twenty commands allocated {made} times");
}
