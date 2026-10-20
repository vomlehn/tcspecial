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

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use log::{error, info, trace};
use tcslibgs::{BeaconTelemetry, ConfigDigest, ConfigVersion, TcsResult, Telemetry};

use crate::telemetry_log::TelemetryLog;

#[derive(Clone)]
pub struct BeaconSend {
    pair: ArcCondPair<SystemTime>,
    interval: Arc<Mutex<Duration>>,
    /// Where beacons go: `beacon_recv`, which is what tcsmoc makes its
    /// receiving socket out of.
    dest_addr: tcslibgs::HostPort,
    /// Where they come from: `beacon_send`, which is what the socket below is
    /// bound to, so every beacon this sends leaves from there.
    ///
    /// Stated rather than left to the system, and that is the whole of what
    /// replaced multicast: a beacon goes from one named place to one other, so
    /// both are written down, and a firewall rule or a capture filter can
    /// describe the link. Before this it was a multicast group and the name of
    /// a local interface to join it on -- which bought any number of ground
    /// stations and cost an attribute whose being wrong lost every beacon in
    /// silence.
    from_addr: tcslibgs::HostPort,
    /// The telemetry log every beacon is recorded in, shared with the
    /// command interpreter so that one log holds all of the telemetry.
    log: TelemetryLog,
    /// How many beacons this process has sent.
    ///
    /// Shared with every duplicate of this sender, because the thread that
    /// sends them holds one: two counters would both say "the third beacon"
    /// about different beacons.
    sent: Arc<AtomicU32>,
    /// What this build is and the digest of the configuration it read,
    /// carried in every beacon.
    ///
    /// Settled once, here, rather than read when a beacon goes out: the
    /// digest is of the file this process read, and a file edited since would
    /// otherwise have the beacons claiming a configuration nothing is
    /// serving.
    version: ConfigVersion,
    digest: ConfigDigest,
}

/// Room for a beacon as JSON, which it comes well inside: a sequence number,
/// a version, a digest and a header.
const BEACON_BYTES: usize = 512;

impl BeaconSend {
    /// Bind where beacons come from, find where they go, and start sending.
    ///
    /// The socket is bound and the destination resolved here, on the caller's
    /// thread, and not on the thread that sends: a `beacon.send` some other
    /// program holds, or a `beacon.recv` that names no address, is then a
    /// vehicle that will not start, said to whoever started it. Found on the
    /// thread, it was a thread that ended in silence and a vehicle that flew
    /// on without beacons -- which the ground cannot tell from a dead one.
    ///
    /// An interval of nought sends none, and is `Ok(None)`.
    pub fn new(
        interval: Duration,
        from_addr: tcslibgs::HostPort,
        dest_addr: tcslibgs::HostPort,
        log: TelemetryLog,
        version: ConfigVersion,
        digest: ConfigDigest,
    ) -> TcsResult<Option<BeaconSend>> {
        if interval == Duration::from_secs(0) {
            return Ok(None);
        }

        // Bound where the configuration says beacons come from, port and all.
        // A port the system chose would work -- the datagram carries its
        // source, so the ground could answer it -- and would leave the link
        // describable from one end only: nothing could be filtered, captured
        // or firewalled by a rule somebody could write down beforehand.
        let socket = UdpSocket::bind(from_addr.to_string()).map_err(|e| {
            tcslibgs::TcsError::Config(format!(
                "beacons cannot be sent from {from_addr} ({e}): change beacon.send \
                 in the configuration's tcspecial section to an address this \
                 machine has and a port nothing else is using"
            ))
        })?;

        // Where they go, resolved once, before the loop: so that sending a
        // beacon allocates nothing. Resolving a name is a lookup that
        // allocates, and the destination does not move.
        let unresolved = || {
            tcslibgs::TcsError::Config(format!(
                "beacons cannot be sent to {dest_addr}, which does not resolve to \
                 an address: change beacon.recv in the configuration's tcspecial \
                 section to the address the ground listens for beacons on"
            ))
        };
        let dest: SocketAddr = (dest_addr.address.as_str(), dest_addr.port)
            .to_socket_addrs()
            .map_err(|_| unresolved())?
            .next()
            .ok_or_else(unresolved)?;

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
            from_addr,
            log,
            version,
            digest,
        };

        let b_clone = b.clone();
        thread::spawn(move || b_clone.beacon_send(socket, dest));

        Ok(Some(b))
    }

    /// Send a beacon now, and then one every interval, for as long as the
    /// process runs.
    ///
    /// Nothing here can end it: everything that could stop beacons for good
    /// was found by `new`. A send that fails -- a route gone, a buffer full --
    /// is said once, when the beacons stop going, and once when they go again,
    /// rather than every interval: a vehicle that cannot reach the ground for
    /// an hour should leave a log that says so in two lines.
    fn beacon_send(&self, socket: UdpSocket, dest: SocketAddr) {
        // The buffer each beacon is written into, made once.
        let mut data = Vec::with_capacity(BEACON_BYTES);
        let mut failing = Failing::default();

        let outcome = self.send_beacon(&socket, dest, &mut data);
        failing.after(outcome, &self.from_addr, &self.dest_addr);

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

            let outcome = self.send_beacon(&socket, dest, &mut data);
            failing.after(outcome, &self.from_addr, &self.dest_addr);

            // Calculate next expiration time
            let interval = *self.interval.lock().unwrap();
            let now = SystemTime::now();
            *expiration = now + interval;
        }
    }

    /// Send the next beacon to `dest`, written into `data`, which is the
    /// thread's own buffer and is reused.
    pub fn send_beacon(
        &self,
        socket: &UdpSocket,
        dest: SocketAddr,
        data: &mut Vec<u8>,
    ) -> TcsResult<()> {
        // From one: the first beacon is the first beacon, and nought is what
        // every beacon used to carry.
        let sequence = self.sent.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        let beacon = Telemetry::Beacon(BeaconTelemetry::new(sequence, self.version, self.digest));
        data.clear();
        serde_json::to_writer(&mut *data, &beacon)?;
        trace!("send_beacon: sending to {dest}");
        // Recorded before it is sent, as a command response is, so that a
        // beacon the send fails on is still known to have been produced.
        self.log.record(data);
        socket.send_to(data, dest)?;
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

/// How many beacons in a row have failed to go, so that a run of failures is
/// reported when it starts and when it ends, and not at every beacon.
#[derive(Default)]
struct Failing {
    since: u32,
}

impl Failing {
    /// Take account of one beacon's send, saying what has changed.
    fn after(
        &mut self,
        outcome: TcsResult<()>,
        from: &tcslibgs::HostPort,
        to: &tcslibgs::HostPort,
    ) {
        match outcome {
            Ok(()) => {
                if self.since > 0 {
                    info!(
                        "beacons are going from {from} to {to} again, after {} \
                         that could not be sent",
                        self.since
                    );
                }
                self.since = 0;
            }
            Err(e) => {
                if self.since == 0 {
                    error!(
                        "a beacon could not be sent from {from} to {to} ({e}); \
                         this is said again only when one is"
                    );
                }
                self.since = self.since.saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslibgs::CIConfig;

    /// Every beacon carries the next number, from one.
    ///
    /// Through the thread that really sends them, because the counting is
    /// what is being tested and the sender holds the counter: a test that
    /// called the send twice on one BeaconSend would not say that the beacons
    /// a running spacecraft emits are numbered in order.
    ///
    /// Sent from one stated address to another, which is what beacons are
    /// now: the ground socket is bound first so that its port can be given to
    /// the sender as the place to send to, and the sender's own end is a
    /// loopback port the system chooses, because a test that named one would
    /// fail whenever something else on the machine held it.
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
            payload_command_source_base: tcslibgs::HostPort {
                address: "127.0.0.1".to_string(),
                port: 0,
            },
            payload_command_dest_base: tcslibgs::HostPort {
                address: "127.0.0.1".to_string(),
                port: 0,
            },
            moc_cmd_send: tcslibgs::HostPort {
                address: "127.0.0.1".to_string(),
                port: 4100,
            },
            payload_source_base: tcslibgs::HostPort {
                address: "0.0.0.0".to_string(),
                port: 6000,
            },
            payload_dest_base: tcslibgs::HostPort {
                address: "127.0.0.1".to_string(),
                port: 6100,
            },
            beacon_interval: Duration::from_millis(50),
            beacon_send: tcslibgs::HostPort {
                address: "127.0.0.1".to_string(),
                port: 0,
            },
            beacon_recv: tcslibgs::HostPort {
                address: at.ip().to_string(),
                port: at.port(),
            },
            // Nothing to write: what is being tested is what goes out.
            log_dir: None,
            log_segment_bytes: 65_536,
        })
        .expect("a log that logs nothing");

        let _beacon = BeaconSend::new(
            Duration::from_millis(50),
            // From a port of the system's choosing, to where the ground
            // socket above is bound.
            tcslibgs::HostPort {
                address: "127.0.0.1".to_string(),
                port: 0,
            },
            tcslibgs::HostPort {
                address: at.ip().to_string(),
                port: at.port(),
            },
            log,
            ConfigVersion::of_this_build(),
            ConfigDigest([0u8; 16]),
        )
        .expect("a sender")
        .expect("an interval that sends");

        let mut buffer = [0u8; 65535];
        for expected in 1..=3u32 {
            // Waited on again where a signal interrupted it: another test in
            // this binary starts child processes, and a child exiting is
            // enough to interrupt a blocking receive. The link's own loops do
            // the same, for the same reason.
            let (size, _) = loop {
                match ground.recv_from(&mut buffer) {
                    Ok(got) => break got,
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => panic!("a beacon: {e}"),
                }
            };
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

    /// A `beacon.send` something else holds is refused when the sender is
    /// made, naming the setting, and is not left to a thread that would end
    /// without a word.
    #[test]
    fn a_beacon_address_that_cannot_be_bound_is_refused_at_the_start() {
        let held = std::net::UdpSocket::bind("127.0.0.1:0").expect("a socket");
        let at = held.local_addr().expect("where it is");

        let refused = BeaconSend::new(
            Duration::from_millis(50),
            tcslibgs::HostPort {
                address: at.ip().to_string(),
                port: at.port(),
            },
            tcslibgs::HostPort {
                address: "127.0.0.1".to_string(),
                port: 9,
            },
            TelemetryLog::disabled(),
            ConfigVersion::of_this_build(),
            ConfigDigest([0u8; 16]),
        );
        let said = match refused {
            Err(e) => e.to_string(),
            Ok(_) => panic!("{at} is held, and a sender was made from it"),
        };
        assert!(said.contains("beacon.send"), "{said}");
        assert!(said.contains(&at.to_string()), "{said}");
    }

    /// A `beacon.recv` that resolves to nothing is refused the same way.
    #[test]
    fn a_beacon_destination_that_does_not_resolve_is_refused_at_the_start() {
        let refused = BeaconSend::new(
            Duration::from_millis(50),
            tcslibgs::HostPort {
                address: "127.0.0.1".to_string(),
                port: 0,
            },
            tcslibgs::HostPort {
                address: "no-such-host.invalid".to_string(),
                port: 5550,
            },
            TelemetryLog::disabled(),
            ConfigVersion::of_this_build(),
            ConfigDigest([0u8; 16]),
        );
        let said = match refused {
            Err(e) => e.to_string(),
            Ok(_) => panic!("a sender was made to nowhere"),
        };
        assert!(said.contains("beacon.recv"), "{said}");
    }
}
