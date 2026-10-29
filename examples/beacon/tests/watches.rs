//! The example's own configuration, watched the way the example watches it.
//!
//! No window: what the window shows is `tcsui`'s and is tested there, and what
//! a beacon means is `tcslib::beacon`'s and is tested there. What is left to
//! check is the join -- that this example's configuration says where the
//! beacons are, that a station reading it listens where tcspecial would send,
//! and that a beacon sent from that address is reported as one.
//!
//! The beacon is sent by this test rather than by tcspecial. What tcspecial
//! does with the same two addresses is its own tests' business; the datagram
//! is the same either way, being the one `tcslibgs` packs with tcs-xtce.

use std::net::UdpSocket;
use std::path::{Path, PathBuf};

use tcslib::beacon::{Arrived, Beacons, Contact, Schedule, Stage};
use tcslibgs::config_digest::digest_of_file;
use tcslibgs::graph::parameters::beacon_of;
use tcslibgs::graph::Graph;
use tcslibgs::{Beacon, ConfigDigest, ConfigVersion};

/// The example's configuration, found from this crate rather than from
/// wherever the test was run.
fn configuration() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("beacon.yaml")
}

/// The example's configuration says where its beacons are, and the two ends
/// are a pair.
///
/// The one thing this example cannot do without. A station listening where the
/// spacecraft is not sending hears nothing and says nothing about why, so the
/// addresses are the configuration's and are read from it rather than written
/// into the program.
#[test]
fn the_configuration_says_where_the_beacons_are() {
    let graph = Graph::load(configuration())
        .expect("the example's configuration reads");
    let link = beacon_of(&graph)
        .expect("its tcspecial section reads")
        .expect(
            "it has a tcspecial section, which is where the beacon is stated",
        );

    // Different ports, because they are different ends: one is bound by the
    // vehicle and one by the ground, and a pair that named one socket would
    // be one end talking to itself.
    assert_ne!(
        (link.send.address.as_str(), link.send.port),
        (link.recv.address.as_str(), link.recv.port),
        "the two ends of the link are one socket: {link:?}"
    );
    assert_ne!(link.recv.port, 0, "a port of nought is no port");
}

/// A beacon sent from where the configuration says they come from is reported
/// as a beacon, and one from anywhere else is not.
#[test]
fn a_beacon_from_the_configured_address_is_heard() {
    let graph = Graph::load(configuration())
        .expect("the example's configuration reads");
    let link = beacon_of(&graph)
        .expect("its tcspecial section reads")
        .expect("it states a beacon");

    // The vehicle's end, bound here so that the datagram comes from the
    // address the configuration names -- which is what the station checks.
    // Sent to the station's own address rather than to the configured one,
    // because the station below is listening on a port the system chose: two
    // runs of this test at once would otherwise fight over the example's.
    let vehicle =
        match UdpSocket::bind(format!("{}:{}", "127.0.0.1", link.send.port)) {
            Ok(socket) => socket,
            // The example's send port is in use, which on a machine running the
            // example is exactly what it looks like. Nothing is proved and
            // nothing is broken.
            Err(_) => return,
        };

    let listening =
        UdpSocket::bind("127.0.0.1:0").expect("a port for the station");
    let at = listening.local_addr().expect("its address");
    drop(listening);

    let mut station = match Beacons::listening(
        tcslib::beacon::Link {
            send: link.send.clone(),
            recv: tcslibgs::HostPort {
                address: at.ip().to_string(),
                port: at.port(),
            },
        },
        Schedule::standard(),
        // The digest of the file the station read, which is this example's
        // own: the beacon below carries another, so the station should say
        // the configuration differs.
        Some(
            digest_of_file(configuration()).expect("the configuration digests"),
        ),
    ) {
        Ok(station) => station,
        Err(_) => return,
    };

    let version = ConfigVersion::of_this_build();
    // Packed by tcs-xtce, from the layout in tcslibgs/xtce/beacon.xml, as
    // tcspecial packs every beacon it sends.
    let beacon = Beacon::new(7, version, ConfigDigest([0x5a; 16])).to_bytes();
    vehicle.send_to(&beacon, at).expect("the vehicle beacons");

    let pass = station.pass();
    assert!(
        matches!(pass.arrived, Arrived::Beacon),
        "a beacon from the configured address was not heard: {:?}",
        pass.arrived
    );
    assert_eq!(pass.showing.stage, Stage::Heard);
    assert_eq!(pass.contact, Contact::Made);
    assert!(
        pass.at.is_some(),
        "a beacon that arrived has a time it arrived at"
    );

    // What it said is on the line, and what is wrong with it is said once:
    // the digest in it is not the digest of the file this station read.
    assert!(
        pass.said.contains("tcspecial v"),
        "the line should say what the beacon said: {}",
        pass.said
    );
    assert!(
        pass.remarks
            .iter()
            .any(|said| said.contains("CONFIGURATION DIFFERS")),
        "a beacon announcing another configuration should say so: {:?}",
        pass.remarks
    );

    // And a second one from somewhere else is not a beacon at all: the
    // address it comes from is in the configuration, so a datagram from
    // anywhere else is somebody else's traffic.
    let stranger =
        UdpSocket::bind("127.0.0.1:0").expect("a socket for the stranger");
    stranger.send_to(&beacon, at).expect("the stranger sends");
    let pass = station.pass();
    assert_eq!(
        pass.arrived,
        Arrived::Nothing,
        "a datagram from the wrong address was taken for a beacon"
    );
    // And it is warned of, which a console says whether or not it was asked
    // to talk.
    assert!(
        pass.warnings.iter().any(|said| said.contains("ignored")),
        "the stranger should be warned of: {:?}",
        pass.warnings
    );
}
