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
    /// `beacon.default_interval`: once the MOC has connected.
    pub default_interval: Duration,
    /// `beacon.reset_interval`: from a restart's being initiated, and
    /// from every start until the MOC connects.
    pub reset_interval: Duration,
}

#[derive(Clone)]
pub struct BeaconSend {
    pair: ArcCondPair<SystemTime>,
    /// How long until the next beacon: `reset_interval` as the process
    /// starts and once a restart is initiated, `normal` once the MOC
    /// connects.
    interval: Arc<Mutex<Duration>>,
    timing: BeaconTiming,
    /// The interval the beacons go at once the MOC has connected:
    /// `default_interval` until a CONFIG says another.
    normal: Arc<Mutex<Duration>>,
    /// Whether the beacons are still going every `reset_interval` because
    /// no CONNECT has come from the MOC since this process started. Shared,
    /// so that the command interpreter's copy clears it for all of them.
    awaiting_connect: Arc<AtomicBool>,
    /// Whether a restart has been initiated, from which the beacons go
    /// every `reset_interval`, the commanded-restart bit set, until the
    /// process ends. See [`BeaconSend::restart_initiated`].
    ending: Arc<AtomicBool>,
    /// The commanded-restart bit every beacon carries: set from a RESTART
    /// being taken, through the start it asked for -- which begins with it
    /// set, from [`crate::state::Started::commanded`] -- until that start
    /// takes a CONNECT. Asked without a lock, as each beacon is packed.
    commanded: Arc<AtomicBool>,
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
    /// Binds where beacons come from, finds where they go, and starts
    /// sending.
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
    /// `reset_interval` until [`BeaconSend::connected`] is called: every
    /// start is a restart as far as the ground can tell, and is beaconed as
    /// one.
    ///
    /// # Arguments
    ///
    /// * `timing`    - How often beacons go, normally and across a restart.
    /// * `from_addr` - `beacon.send`, where beacons are sent from.
    /// * `dest_addr` - `beacon.recv`, where they go.
    /// * `log`       - The telemetry log every beacon is recorded in.
    /// * `version`   - What this build is, carried in every beacon.
    /// * `digest`    - The digest of the configuration read, carried in every
    ///                 beacon.
    /// * `started`   - This start, as the state file counted it, which says
    ///                 whether the ground commanded it.
    ///
    /// # Returns
    ///
    /// The sender, whose thread is already sending; `None` where
    /// `timing.default_interval` is nought.
    ///
    /// # Errors
    ///
    /// `TcsError::Config`, naming the setting to change, where `from_addr`
    /// cannot be bound or `dest_addr` does not resolve to an address.
    pub fn new(
        timing: BeaconTiming,
        from_addr: tcslibgs::HostPort,
        dest_addr: tcslibgs::HostPort,
        log: TelemetryLog,
        version: ConfigVersion,
        digest: ConfigDigest,
        started: &crate::state::Started,
    ) -> TcsResult<Option<BeaconSend>> {
        let commanded = started.commanded;
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
            normal: Arc::new(Mutex::new(timing.default_interval)),
            awaiting_connect: Arc::new(AtomicBool::new(true)),
            ending: Arc::new(AtomicBool::new(false)),
            commanded: Arc::new(AtomicBool::new(commanded)),
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

    /// Sends a beacon now, and then one every interval, for as long as the
    /// process runs.
    ///
    /// Nothing here can end it: everything that could stop beacons for good
    /// was found by `new`. A send that fails -- a route gone, a buffer full --
    /// is said once, when the beacons stop going, and once when they go again,
    /// rather than every interval: a vehicle that cannot reach the ground for
    /// an hour should leave a log that says so in two lines.
    ///
    /// # Arguments
    ///
    /// * `socket` - The socket bound to `beacon.send`, which beacons go out
    ///              of.
    /// * `dest`   - Where they go, already resolved.
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

    /// Sends the next beacon to `dest`, recording it in the telemetry log
    /// first.
    ///
    /// # Arguments
    ///
    /// * `socket` - The socket the beacon goes out of.
    /// * `dest`   - Where it goes.
    /// * `packer` - What it is packed with, which is the thread's own and is
    ///              reused.
    ///
    /// # Returns
    ///
    /// Nothing, once the beacon is sent.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` where the send fails.
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
        let beacon = Beacon::new(sequence, self.version, self.digest)
            .commanded_restart(self.commanded.load(Ordering::SeqCst));
        let data = packer.pack(&beacon);
        trace!("send_beacon: sending to {dest}");
        // Recorded before it is sent, as a command response is, so that a
        // beacon the send fails on is still known to have been produced.
        self.log.record(data);
        socket.send_to(data, dest)?;
        Ok(())
    }

    /// Takes the interval a CONFIG has set the beacons to go at once the
    /// MOC has connected. Where it has, and no restart is under way, they go
    /// at it from now, starting with one at once; otherwise from the next
    /// CONNECT.
    ///
    /// # Arguments
    ///
    /// * `interval` - How long from one beacon to the next; nought is ignored
    ///                and changes nothing.
    pub fn set_interval(&mut self, interval: Duration) {
        if interval == Duration::from_secs(0) {
            return;
        }
        *self.normal.lock().unwrap() = interval;
        if !self.awaiting_connect.load(Ordering::SeqCst)
            && !self.ending.load(Ordering::SeqCst)
        {
            self.retime(interval);
        }
    }

    /// A CONNECT has been taken from the MOC: the commanded-restart bit is
    /// cleared, and the beacons go back to their normal interval, starting
    /// with one at once so that the ground sees the bit clear.
    ///
    /// Not once a restart has been initiated: from then until the process
    /// ends, the beacons are the ground's news of the restart, and they stay
    /// fast, with the bit set.
    pub fn connected(&self) {
        if self.ending.load(Ordering::SeqCst) {
            return;
        }
        self.commanded.store(false, Ordering::SeqCst);
        let normal = *self.normal.lock().unwrap();
        if self.awaiting_connect.swap(false, Ordering::SeqCst) {
            // The one allocation this makes in the command loop, where the
            // log formats the line: once a start, at the first CONNECT, and
            // worth it for the record of when the ground connected.
            info!("the MOC has connected: beacons every {normal:?}");
        }
        self.retime(normal);
    }

    /// A restart has been initiated: the commanded-restart bit is set, one
    /// beacon goes at once, and the rest every `reset_interval` until the
    /// process ends. They come fast, because the ground is waiting for the
    /// first one of the start that comes back.
    pub fn restart_initiated(&self) {
        self.ending.store(true, Ordering::SeqCst);
        self.commanded.store(true, Ordering::SeqCst);
        self.retime(self.timing.reset_interval);
    }

    /// Beacons every `interval` from now on, starting with one now.
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
    /// Takes account of one beacon's send, saying what has changed: the
    /// first failure of a run, and the first success after one.
    ///
    /// # Arguments
    ///
    /// * `outcome` - How the send went.
    /// * `from`    - Where beacons are sent from, for saying so.
    /// * `to`      - Where they go, for saying so.
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

    /// Makes a start of tcspecial as the state file counted it.
    ///
    /// # Arguments
    ///
    /// * `generation` - Its generation.
    /// * `commanded`  - Whether the ground commanded it.
    ///
    /// # Returns
    ///
    /// The start, with nothing damaged.
    fn a_start(generation: u32, commanded: bool) -> crate::state::Started {
        crate::state::Started {
            generation,
            commanded,
            damaged: None,
        }
    }
    use tcslibgs::CIConfig;

    /// Makes a timing that beacons every fifty milliseconds, across a
    /// restart or not.
    ///
    /// # Returns
    ///
    /// The timing.
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
            &a_start(1, false),
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

    /// Makes a ground socket, and a sender beaconing to it from a port of
    /// its own with `timing`, from a start commanded or not.
    ///
    /// # Arguments
    ///
    /// * `timing`    - How often the sender beacons.
    /// * `commanded` - Whether its start was commanded.
    ///
    /// # Returns
    ///
    /// The ground socket and the sender.
    ///
    /// # Panics
    ///
    /// If either socket cannot be made.
    fn sending(
        timing: BeaconTiming,
        commanded: bool,
    ) -> (std::net::UdpSocket, BeaconSend) {
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
            &a_start(7, commanded),
        )
        .expect("a sender")
        .expect("an interval that sends");
        (ground, sender)
    }

    /// Reads the next beacon to arrive.
    ///
    /// # Arguments
    ///
    /// * `ground` - The socket it arrives at.
    /// * `wait`   - How long to wait for it.
    ///
    /// # Returns
    ///
    /// The beacon, or `None` where none arrived within `wait`.
    ///
    /// # Panics
    ///
    /// If the timeout cannot be set or what arrives is not a beacon.
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

    /// Every start beacons fast until the MOC connects; then at the
    /// interval, and a CONFIG's interval from the next CONNECT. A restart
    /// initiated sets the commanded-restart bit and sends one at once, fast
    /// again, and a CONNECT does not slow them or clear the bit after that.
    #[test]
    fn beacons_go_fast_across_a_restart_and_slow_when_the_moc_connects() {
        let fast = Duration::from_millis(20);
        let (ground, mut sender) = sending(
            BeaconTiming {
                default_interval: Duration::from_secs(60),
                reset_interval: fast,
            },
            false,
        );
        let wait = Duration::from_secs(5);
        for _ in 0..3 {
            let beacon =
                next(&ground, wait).expect("fast beacons as it starts");
            assert!(!beacon.commanded_restart, "nobody commanded this start");
        }
        // A CONFIG before a CONNECT changes nothing yet.
        sender.set_interval(Duration::from_secs(120));
        next(&ground, wait).expect("still fast");

        sender.connected();
        // One goes at once, and one may already have been on its way; after
        // that, none for two minutes, and a quarter of a second is a dozen
        // fast ones.
        let _ = next(&ground, Duration::from_millis(100));
        let _ = next(&ground, Duration::from_millis(100));
        assert_eq!(
            next(&ground, Duration::from_millis(250)),
            None,
            "the beacons slowed when the MOC connected"
        );

        sender.restart_initiated();
        let beacon = next(&ground, wait).expect("one at once");
        assert!(beacon.commanded_restart, "set as the restart is taken");
        sender.connected();
        for _ in 0..3 {
            let beacon = next(&ground, wait).expect("fast until it ends");
            assert!(beacon.commanded_restart, "until the process ends");
        }
    }

    /// A commanded start beacons with the bit set until the MOC connects,
    /// and with it clear after.
    #[test]
    fn a_commanded_start_says_so_until_the_moc_connects() {
        let (ground, sender) = sending(
            BeaconTiming {
                default_interval: Duration::from_millis(20),
                reset_interval: Duration::from_millis(20),
            },
            true,
        );
        let wait = Duration::from_secs(5);
        for _ in 0..2 {
            let beacon = next(&ground, wait).expect("a beacon");
            assert!(beacon.commanded_restart);
        }
        sender.connected();
        // The one already on its way may still carry it; the one after
        // does not.
        let _ = next(&ground, wait);
        let beacon = next(&ground, wait).expect("a beacon");
        assert!(!beacon.commanded_restart, "cleared by the CONNECT");
    }

    /// A `beacon.send` something else holds is refused when the sender is
    /// made, naming the setting, and is not left to a thread that would end
    /// without a word.
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
            &a_start(1, false),
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
            &a_start(1, false),
        );
        let said = match refused {
            Err(e) => e.to_string(),
            Ok(_) => panic!("a sender was made to nowhere"),
        };
        assert!(said.contains("beacon.recv"), "{said}");
    }
}
