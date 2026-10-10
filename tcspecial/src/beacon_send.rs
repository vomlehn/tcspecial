/*
 * Implements the beaconing function
 *
 * Why not just sleep in a loop? Because I want to be able to wake up when the
 * interval changes and send a beacon immediately. This is pretty close to
 * the behavior of the Toyota Camry intermittent wiper functionality.
 *
 * NOTE: Once started, it is not possible to turn beaconing off. Still, setting
 * the interval to a very large interval will effectively do so.
 */

use std::net::UdpSocket;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use log::trace;
use tcslibgs::{BeaconTelemetry, ConfigDigest, ConfigVersion, TcsResult, Telemetry};

use crate::telemetry_log::TelemetryLog;

#[derive(Clone)]
pub struct BeaconSend {
    pair:       ArcCondPair<SystemTime>,
    interval:   Arc<Mutex<Duration>>,
    dest_addr:  std::net::SocketAddr,
    /// The local interface beacons go out on; see `beacon_send`.
    interface:  std::net::Ipv4Addr,
    /// The telemetry log every beacon is recorded in, shared with the
    /// command interpreter so that one log holds all of the telemetry.
    log:        TelemetryLog,
    /// How many beacons this process has sent.
    ///
    /// Shared with every duplicate of this sender, because the thread that
    /// sends them holds one: two counters would both say "the third beacon"
    /// about different beacons.
    sent:       Arc<AtomicU32>,
    /// What this build is, what the configuration says it is, and what it
    /// read, carried in every beacon.
    ///
    /// Settled once, here, rather than read when a beacon goes out: the
    /// digest is of the file this process read, and a file edited since would
    /// otherwise have the beacons claiming a configuration nothing is
    /// serving.
    version:    ConfigVersion,
    config_version: ConfigVersion,
    digest:     ConfigDigest,
}

impl BeaconSend {
    pub fn new(
        interval: Duration,
        dest_addr: std::net::SocketAddr,
        interface: std::net::Ipv4Addr,
        log: TelemetryLog,
        version: ConfigVersion,
        config_version: ConfigVersion,
        digest: ConfigDigest,
    ) -> Option<BeaconSend> {
        if interval == Duration::from_secs(0) {
            return None;
        }

        let expiration_time = SystemTime::now() + interval;
        let pair = Arc::new(CondPair {
            lock: Mutex::new(expiration_time),
            cvar: Condvar::new(),
        });

        let b = BeaconSend {
            sent: Arc::new(AtomicU32::new(0)),
            pair,
            interval: Arc::new(Mutex::new(interval)),
            dest_addr,
            interface,
            log,
            version,
            config_version,
            digest,
        };

        let b_clone = b.clone();
        thread::spawn(move || {
// FIXME: add check for error
            let _ = b_clone.beacon_send();
        });

        Some(b)
    }

    // FIXME: check result type
    fn beacon_send(&self) -> TcsResult<()> {
        // Bound to the interface the beacon is to go out on, not to every
        // interface. A multicast datagram from a socket bound to 0.0.0.0 goes
        // out the default route, which on a host with a second interface or a
        // tunnel is not the interface the ground station joined the group on
        // -- and then not one beacon arrives, with nothing anywhere to say so.
        // Binding the interface's own address is how std selects it; there is
        // no set_multicast_if.
        let socket = UdpSocket::bind((self.interface, 0))?;

        // Local scope. A beacon is for the ground station on this network,
        // and a default of 1 is what keeps it from being forwarded off it;
        // said rather than assumed because it is the one option that decides
        // how far the thing travels.
        socket.set_multicast_ttl_v4(1)?;

// FIXME: add check for error
        let _ = self.send_beacon(&socket, &self.dest_addr);

        loop {
            let mut expiration = self.pair.lock.lock().unwrap();

            // Wait until expiration time or until notified
            while *expiration > SystemTime::now() {
                let timeout = expiration
                    .duration_since(SystemTime::now())
                    .unwrap_or(Duration::from_millis(1));

                let (guard, result) = self.pair.cvar.wait_timeout(expiration, timeout).unwrap();
                expiration = guard;

                if result.timed_out() {
                    break;
                }
            }

            // Send the beacon
// FIXME: add check for error
            let _ = self.send_beacon(&socket, &self.dest_addr);

            // Calculate next expiration time1G
            let interval = *self.interval.lock().unwrap();
            let now = SystemTime::now();
            *expiration = now + interval;
        }
    }

    pub fn send_beacon(&self, socket: &UdpSocket, dest_addr: &std::net::SocketAddr) -> TcsResult<()> {
        // From one: the first beacon is the first beacon, and nought is what
        // every beacon used to carry.
        let sequence = self.sent.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        let beacon = Telemetry::Beacon(BeaconTelemetry::new(
            sequence,
            self.version,
            self.config_version,
            self.digest,
        ));
        let data = serde_json::to_vec(&beacon)?;
        trace!("send_beacon: sending to {:?}", dest_addr);
        // Recorded before it is sent, as a command response is, so that a
        // beacon the send fails on is still known to have been produced.
        self.log.record(&data);
        socket.send_to(&data, dest_addr)?;
        Ok(())
    }

    /// Reset the interval to the given value. This will result in the immediate
    /// sending of a beacon message
    pub fn set_interval(&mut self, interval: Duration) {
        if interval == Duration::from_secs(0) {
            return;
        }

        // Update the interval
        *self.interval.lock().unwrap() = interval;

        // Set expiration to now to trigger immediate beacon
        *self.pair.lock.lock().unwrap() = SystemTime::now();

        // Wake the worker thread
        self.pair.cvar.notify_one();
    }
}

type ArcCondPair<T> = Arc<CondPair<T>>;

struct CondPair<T> {
    lock: Mutex<T>,
    cvar: Condvar,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslibgs::{BeaconTime, CIConfig, NetworkProtocol};

    /// Every beacon carries the next number, from one.
    ///
    /// Through the thread that really sends them, because the counting is
    /// what is being tested and the sender holds the counter: a test that
    /// called the send twice on one BeaconSend would not say that the beacons
    /// a running spacecraft emits are numbered in order.
    ///
    /// Sent to a plain address on the loopback interface rather than to a
    /// multicast group. What goes out is the same datagram, and a test that
    /// joined a group would be testing the network stack's multicast rather
    /// than this.
    #[test]
    fn the_beacons_are_numbered_in_order_from_one() {
        let ground = std::net::UdpSocket::bind("127.0.0.1:0").expect("a ground socket");
        ground
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a timeout");
        let at = ground.local_addr().expect("where it is");

        let log = TelemetryLog::open(&CIConfig {
            address: "127.0.0.1".to_string(),
            port: 0,
            payload_port: 0,
            protocol: NetworkProtocol::Udp,
            beacon_interval: BeaconTime(50),
            beacon_address: at,
            beacon_interface: std::net::Ipv4Addr::LOCALHOST,
            // Nothing to write: what is being tested is what goes out.
            log_dir: None,
            log_segment_bytes: 65_536,
        })
        .expect("a log that logs nothing");

        let _beacon = BeaconSend::new(
            Duration::from_millis(50),
            at,
            std::net::Ipv4Addr::LOCALHOST,
            log,
            ConfigVersion::of_this_build(),
            ConfigVersion::of_text("1.0").expect("a version"),
            ConfigDigest([0u8; 16]),
        )
        .expect("a sender");

        let mut buffer = [0u8; 65535];
        for expected in 1..=3u32 {
            let (size, _) = ground.recv_from(&mut buffer).expect("a beacon");
            match serde_json::from_slice::<Telemetry>(&buffer[..size]).expect("telemetry") {
                Telemetry::Beacon(beacon) => assert_eq!(
                    beacon.header.sequence, expected,
                    "beacon {expected} is numbered {}",
                    beacon.header.sequence
                ),
                other => panic!("expected a beacon, got {other:?}"),
            }
        }
    }
}
