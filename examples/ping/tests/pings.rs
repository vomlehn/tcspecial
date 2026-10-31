//! The ground station, run against a vehicle the Rust plays.
//!
//! This test is tcspecial, as far as the ground station can tell: it sends
//! beacons serialized as tcspecial serializes them, from where the
//! configuration says they come from, and it answers each ping with
//! `tcslibgs::node::vehicle::ping::reply`, the function the `ping` node
//! answers with. So what the ground station prints is what it makes of the
//! vehicle's own bytes, and a change to either -- the beacon's layout, the ping
//! format -- that the Python has not followed fails here.
//!
//! The ports are free ones, found as the test starts, so this runs beside
//! anything else, the example itself included.

use std::io::{BufRead, BufReader, Read};
use std::net::UdpSocket;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tcslibgs::config_digest::{digest_of_file, ConfigVersion};
use tcslibgs::node::vehicle::ping;
use tcslibgs::Beacon;

/// A socket on a port nothing holds.
fn bound() -> UdpSocket {
    UdpSocket::bind("127.0.0.1:0").expect("a port of its own")
}

fn port(socket: &UdpSocket) -> u16 {
    socket.local_addr().expect("its address").port()
}

/// A port nothing holds, for the ground station to bind.
fn free() -> u16 {
    port(&bound())
}

#[test]
fn each_ping_is_answered_and_timed_and_the_beacon_is_heard() {
    // The vehicle's two sockets: where pings arrive, and where beacons are
    // sent from -- which the configuration names, as tcspecial's does.
    let uplink = bound();
    uplink
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("a timeout");
    let beacon = bound();
    let (answers, beacons) = (free(), free());

    let dir = tempfile::tempdir().expect("a directory");
    let configuration = dir.path().join("ping.xml");
    std::fs::write(
        &configuration,
        format!(
            "<configuration>
    <tcspecial>
        <gateway>radio</gateway>
        <interface>ad9aac4dd5a6d88dd5e78824ba8a83ac</interface>
        <beacon><default_interval>5 s</default_interval>\
<reset_interval>1 s</reset_interval><recv>127.0.0.1:{beacons}</recv>\
<send>127.0.0.1:{}</send></beacon>
    </tcspecial>
    <gateway>
        <name>gateway</name><type>gateway_radio</type>
        <address>127.0.0.1</address><port>{}</port>
        <ground_base>127.0.0.1:{}</ground_base>
        <payload-in>true</payload-in>
        <gateway-out><target>ping</target></gateway-out>
    </gateway>
    <vehicle>
        <name>ping</name><type>ping</type><gateway-in>true</gateway-in>
        <payload-out><target>gateway</target></payload-out>
    </vehicle>
</configuration>
",
            port(&beacon),
            port(&uplink),
            answers - 1,
        ),
    )
    .expect("written");

    // The digest the vehicle announces is the configuration's own, so the
    // ground, which digests it too, finds they agree -- and says so by saying
    // nothing about it.
    let digest = digest_of_file(&configuration).expect("it digests");

    let mut ground = Command::new("python3")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/ping_gateway.py"))
        .arg(&configuration)
        // This test is the vehicle, so the ground station starts none.
        // Verbose, because it is the line saying it is listening that this
        // waits for.
        .args(["--pings", "2", "--no-vehicle", "--verbose"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the ground station starts");

    // Once it says it is listening, and not before: a beacon sent before the
    // ground has its socket is one nobody will ever read.
    let mut said = String::new();
    BufReader::new(ground.stderr.take().expect("its complaints"))
        .read_line(&mut said)
        .expect("what it said");
    // A ground station that cannot digest its configuration says so first,
    // and the usual reason is that nothing has built tcsverify: this test's
    // own package does not, a program of another package being nothing cargo
    // builds for it.
    assert!(
        said.contains("Listening for beacons"),
        "{said}{}",
        if said.contains("not built") {
            "build the workspace's programs first: cargo build --workspace"
        } else {
            ""
        }
    );

    // The beacons, until the ground has had enough of them.
    let stop = Arc::new(AtomicBool::new(false));
    let beaconing = {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let version = ConfigVersion::of_this_build();
            let mut sequence = 0;
            while !stop.load(Ordering::Relaxed) {
                sequence += 1;
                let datagram =
                    Beacon::new(sequence, version, digest).to_bytes();
                let _ = beacon.send_to(&datagram, ("127.0.0.1", beacons));
                std::thread::sleep(Duration::from_millis(100));
            }
        })
    };

    // The vehicle: each ping answered with the vehicle's time, as the node
    // answers it, at ground_base plus the ping node's device id.
    for count in 1..=2u64 {
        let mut buffer = [0u8; ping::REPLY_BYTES];
        let (got, _) = uplink.recv_from(&mut buffer).expect("a ping arrives");
        let had = ping::read_message(&buffer[..got]).expect("it is a ping");
        assert_eq!(had.count, count);
        let answer = ping::reply(&buffer[..got], ping::now().expect("a clock"))
            .expect("answered");
        uplink
            .send_to(answer.as_bytes(), ("127.0.0.1", answers))
            .expect("sent down");
    }

    let mut printed = String::new();
    ground
        .stdout
        .take()
        .expect("its output")
        .read_to_string(&mut printed)
        .expect("what it printed");
    let status = ground.wait().expect("it ends");
    stop.store(true, Ordering::Relaxed);
    beaconing.join().expect("the beacons stop");

    assert!(status.success(), "{printed}");
    // The beacon, read from the vehicle's own bytes.
    assert!(
        printed.contains(&format!(
            "beacon: tcspecial v{0} md5 {digest}",
            ConfigVersion::of_this_build(),
        )),
        "{printed}"
    );
    // And each ping, its answer, and the three times.
    for count in 1..=2 {
        assert!(
            printed.contains(&format!("sent: ping #{count} ")),
            "{printed}"
        );
        assert!(
            printed.contains(&format!("received: ping #{count} ")),
            "{printed}"
        );
    }
    assert_eq!(
        printed.matches("times: round trip ").count(),
        2,
        "{printed}"
    );
}
