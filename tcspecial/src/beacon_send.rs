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
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use log::{error, info, trace};
use tcslibgs::{Beacon, BeaconPacker, ConfigDigest, ConfigVersion, TcsResult};

use crate::telemetry_log::TelemetryLog;

/// How often beacons go: normally, and across a restart.
///
/// Both from the configuration's `beacon` section. See "The beacon across a
/// restart" in `docs/design.rst`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeaconTiming {
    /// `beacon.default_interval`: once the MOC has been heard from.
    pub default_interval: Duration,
    /// `beacon.reset_interval`: from a restart's being initiated, and
    /// from every start until the MOC is heard from.
    pub reset_interval: Duration,
}

#[derive(Clone)]
pub struct BeaconSend {
    pair: ArcCondPair<SystemTime>,
    /// How long until the next beacon: `reset_interval` as the process
    /// starts and once a restart is initiated, the timing's `interval` once
    /// the MOC is heard from, and what a CONFIG says after that.
    interval: Arc<Mutex<Duration>>,
    timing: BeaconTiming,
    /// Whether the beacons are still going every `reset_interval`
    /// because nothing valid has come from the MOC since this process
    /// started. Shared, so that the command interpreter's copy clears it for
    /// all of them, and asked without a lock, because it is asked of every
    /// command that arrives.
    awaiting_moc: Arc<AtomicBool>,
    /// Whether a restart has been initiated, which every beacon from then
    /// until the process ends says.
    restarting: Arc<AtomicBool>,
    /// This start's generation, from the state file: see [`crate::state`].
    generation: u32,
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
    ///
    /// The first beacon goes at once, and the next every
    /// `reset_interval` until [`BeaconSend::heard_from_moc`] is
    /// called: every start is a restart as far as the ground can tell, and is
    /// beaconed as one.
    pub fn new(
        timing: BeaconTiming,
        from_addr: tcslibgs::HostPort,
        dest_addr: tcslibgs::HostPort,
        log: TelemetryLog,
        version: ConfigVersion,
        digest: ConfigDigest,
        generation: u32,
    ) -> TcsResult<Option<BeaconSend>> {
        if timing.default_interval == Duration::from_secs(0) {
            return Ok(None);
        }
        let interval = timing.reset_interval;

        // Bound where the configuration says beacons come from, port and all.
        // A port the system chose would work -- the datagram carries its
        // source, so the ground could answer it -- and would leave the link
        // describable from one end only: nothing could be filtered, captured
        // or firewalled by a rule somebody could write down beforehand.
        let socket = UdpSocket::bind(from_addr.to_string()).map_err(|e| {
            tcslibgs::TcsError::Config(format!(
                "beacons cannot be sent from {from_addr} ({e}): change \
                 beacon.send in the configuration's tcspecial section to an \
                 address this machine has and a port nothing else is using"
            ))
        })?;

        // Where they go, resolved once, before the loop: so that sending a
        // beacon allocates nothing. Resolving a name is a lookup that
        // allocates, and the destination does not move.
        let unresolved = || {
            tcslibgs::TcsError::Config(format!(
                "beacons cannot be sent to {dest_addr}, which does not resolve \
                 to an address: change beacon.recv in the configuration's \
                 tcspecial section to the address the ground listens for \
                 beacons on"
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
            timing,
            awaiting_moc: Arc::new(AtomicBool::new(true)),
            restarting: Arc::new(AtomicBool::new(false)),
            generation,
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
        // What each beacon is packed with, buffer and all, made once.
        let mut packer = BeaconPacker::new();
        let mut failing = Failing::default();

        let outcome = self.send_beacon(&socket, dest, &mut packer);
        failing.after(outcome, &self.from_addr, &self.dest_addr);

        loop {
            let mut expiration = self.pair.lock.lock().unwrap();

            // Wait until expiration time or until notified
            while *expiration > SystemTime::now() {
                let timeout = expiration
                    .duration_since(SystemTime::now())
                    .unwrap_or(Duration::from_millis(1));

                let (guard, result) =
                    self.pair.cvar.wait_timeout(expiration, timeout).unwrap();
                expiration = guard;

                if result.timed_out() {
                    break;
                }
            }

            let outcome = self.send_beacon(&socket, dest, &mut packer);
            failing.after(outcome, &self.from_addr, &self.dest_addr);

            // Calculate next expiration time
            let interval = *self.interval.lock().unwrap();
            let now = SystemTime::now();
            *expiration = now + interval;
        }
    }

    /// Send the next beacon to `dest`, packed by `packer`, which is the
    /// thread's own and is reused.
    pub fn send_beacon(
        &self,
        socket: &UdpSocket,
        dest: SocketAddr,
        packer: &mut BeaconPacker,
    ) -> TcsResult<()> {
        // From one: the first beacon is the first beacon, and nought is what
        // every beacon used to carry.
        let sequence =
            self.sent.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        let beacon =
            Beacon::new(sequence, self.version, self.digest, self.generation)
                .restarting(self.restarting.load(Ordering::SeqCst));
        let data = packer.pack(&beacon);
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

        self.retime(interval);
    }

    /// A valid packet has arrived from the MOC: the beacons go back to
    /// `beacon.default_interval`, if they were going every `reset_interval`
    /// because nothing had.
    ///
    /// Called for every packet the command interpreter takes, so the case
    /// that matters -- every one after the first -- is one atomic swap. Not
    /// once a restart has been initiated: from then until the process ends,
    /// the beacons are the ground's news of the restart, and they stay fast.
    pub fn heard_from_moc(&self) {
        if self.restarting.load(Ordering::SeqCst) {
            return;
        }
        if self.awaiting_moc.swap(false, Ordering::SeqCst) {
            // Slower from the next beacon on, and not one sent now: the
            // ground has just been heard from, and a beacon now would say
            // nothing it was not told a moment ago.
            *self.interval.lock().unwrap() = self.timing.default_interval;
            // The one allocation this makes in the command loop, where the
            // log formats the line: once a start, on the first packet, and
            // worth it for the record of when the ground was first heard.
            info!(
                "the MOC has been heard from: beacons every {:?} again",
                self.timing.default_interval
            );
        }
    }

    /// A restart has been initiated: from now until the process ends, every
    /// beacon says so, one goes at once, and the rest every
    /// `reset_interval`.
    pub fn restart_initiated(&self) {
        self.restarting.store(true, Ordering::SeqCst);
        self.retime(self.timing.reset_interval);
    }

    /// Beacon every `interval` from now on, starting with one now.
    fn retime(&self, interval: Duration) {
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

    /// Beacons every fifty milliseconds, across a restart or not.
    fn fifty() -> BeaconTiming {
        BeaconTiming {
            default_interval: Duration::from_millis(50),
            reset_interval: Duration::from_millis(50),
        }
    }

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
        let ground =
            std::net::UdpSocket::bind("127.0.0.1:0").expect("a ground socket");
        ground
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a timeout");
        let at = ground.local_addr().expect("where it is");

        let log = TelemetryLog::open(
            &CIConfig {
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
                reset_interval: Duration::from_millis(50),
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
                restart_arm_valid: tcslibgs::RESTART_ARM_VALID_DEFAULT,
                gateway: tcslibgs::gateway::GatewayKind::Radio,
                state_file: String::new(),
            },
            &crate::segsend::SegSend::none(),
        )
        .expect("a log that logs nothing");

        let _beacon = BeaconSend::new(
            fifty(),
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
            1,
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
                    Err(ref e)
                        if e.kind() == std::io::ErrorKind::Interrupted =>
                    {
                        continue
                    }
                    Err(e) => panic!("a beacon: {e}"),
                }
            };
            let beacon = Beacon::read(&buffer[..size]).expect("a beacon");
            assert_eq!(
                beacon.sequence, expected,
                "beacon {expected} is numbered {}",
                beacon.sequence
            );
        }
    }

    /// A `beacon.send` something else holds is refused when the sender is
    /// made, naming the setting, and is not left to a thread that would end
    /// without a word.
    /// A ground socket, and a sender beaconing to it from a port of its own
    /// with `timing`, as generation 7.
    fn sending(timing: BeaconTiming) -> (std::net::UdpSocket, BeaconSend) {
        let ground =
            std::net::UdpSocket::bind("127.0.0.1:0").expect("a ground socket");
        let at = ground.local_addr().expect("where it is");
        let sender = BeaconSend::new(
            timing,
            tcslibgs::HostPort {
                address: "127.0.0.1".to_string(),
                port: 0,
            },
            tcslibgs::HostPort {
                address: at.ip().to_string(),
                port: at.port(),
            },
            TelemetryLog::disabled(),
            ConfigVersion::of_this_build(),
            ConfigDigest([0u8; 16]),
            7,
        )
        .expect("a sender")
        .expect("an interval that sends");
        (ground, sender)
    }

    /// The next beacon to arrive within `wait`, or `None`.
    fn next(ground: &std::net::UdpSocket, wait: Duration) -> Option<Beacon> {
        ground.set_read_timeout(Some(wait)).expect("a timeout");
        let mut buffer = [0u8; 512];
        loop {
            match ground.recv_from(&mut buffer) {
                Ok((size, _)) => {
                    return Some(
                        Beacon::read(&buffer[..size]).expect("a beacon"),
                    )
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => return None,
            }
        }
    }

    /// Every start beacons fast, with its generation and the restarting bit
    /// clear, until the MOC is heard from; then at the interval. A restart
    /// initiated sends one at once with the bit set, fast again, and being
    /// heard from does not slow them after that.
    #[test]
    fn beacons_go_fast_across_a_restart_and_slow_when_the_moc_is_heard() {
        let fast = Duration::from_millis(20);
        let (ground, sender) = sending(BeaconTiming {
            default_interval: Duration::from_secs(60),
            reset_interval: fast,
        });
        let wait = Duration::from_secs(5);
        for _ in 0..3 {
            let beacon =
                next(&ground, wait).expect("fast beacons as it starts");
            assert_eq!(beacon.generation, 7);
            assert!(!beacon.restarting, "a process that has just started");
        }

        sender.heard_from_moc();
        // One may already have been on its way; after that, none for a
        // minute, and a quarter of a second is a dozen fast ones.
        let _ = next(&ground, Duration::from_millis(100));
        assert_eq!(
            next(&ground, Duration::from_millis(250)),
            None,
            "the beacons slowed when the MOC was heard from"
        );

        sender.restart_initiated();
        let beacon = next(&ground, wait).expect("one at once");
        assert!(beacon.restarting, "it says the restart was initiated");
        sender.heard_from_moc();
        for _ in 0..3 {
            let beacon = next(&ground, wait).expect("fast until it ends");
            assert!(beacon.restarting);
        }
    }

    #[test]
    fn a_beacon_address_that_cannot_be_bound_is_refused_at_the_start() {
        let held = std::net::UdpSocket::bind("127.0.0.1:0").expect("a socket");
        let at = held.local_addr().expect("where it is");

        let refused = BeaconSend::new(
            fifty(),
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
            1,
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
            fifty(),
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
            1,
        );
        let said = match refused {
            Err(e) => e.to_string(),
            Ok(_) => panic!("a sender was made to nowhere"),
        };
        assert!(said.contains("beacon.recv"), "{said}");
    }
}
