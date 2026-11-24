//! Relays between a ground station and a vehicle that hold each datagram a
//! while, lose some, or both: the link's simulation, done by the ground, for
//! a ground station that has no link of its own to do it in -- Yamcs, which
//! the Yamcs examples' grounds move off the vehicle's ports for a relay to
//! stand on them. See `tcs-relay`, the program, and `tcsmoc.relay` in the
//! Python ground's library, which starts it.
//!
//! A relay listens at one address and passes each datagram that arrives
//! there on to another: one way, for what only one side sends -- telemetry,
//! the beacon -- or both ways, for commands, whose answers come back to the
//! socket that sent them and are passed back to whoever last asked. Each
//! datagram is held `delay` before it goes, the light time to the Moon say,
//! and lost whole where [`tcsmoc::loss`] says a bit error loses it. All of
//! a set's relays are served by one thread, which waits on their sockets and
//! on when the next held datagram is due, and on nothing else.
//!
//! Both the delay and the loss are simulation, refused in a flight run: the
//! program asks [`tcsmoc::simulation::allow`] first.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::str::FromStr;

use tcsos::net::{SocketAddr, UdpSocket};
use tcsos::sync::atomic::{AtomicBool, Ordering};
use tcsos::sync::Arc;
use tcsos::time::{Duration, Instant};
use tcsos::wait::{Ready, Waitable, Wakeup, Watch};

/// The largest datagram a relay passes on: the largest UDP carries.
const LARGEST: usize = 65_535;

/// One relay: where it listens, where it passes what arrives on to, and
/// whether answers come back the other way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Route {
    /// Where it listens.
    pub listen: SocketAddr,
    /// Where it passes each datagram on to.
    pub forward: SocketAddr,
    /// Whether what comes back to the socket it forwards from is passed
    /// back to whoever last sent: true for a link of commands and answers.
    pub both_ways: bool,
    /// For a relay both ways, where the socket it forwards from is bound:
    /// the ground's own address, which a vehicle takes commands only from,
    /// and which a vehicle on another machine is reached from. `None` is
    /// the loopback, any port, for a vehicle on this machine.
    pub from: Option<SocketAddr>,
}

impl FromStr for Route {
    type Err = String;

    /// Reads a route as `tcs-relay` takes it: `LISTEN>FORWARD` one way, or
    /// `LISTEN<>FORWARD` both ways, each address `host:port`, the second
    /// followed, for a relay both ways, by `@FROM`, where it forwards from.
    ///
    /// # Arguments
    ///
    /// * `text` - The route.
    ///
    /// # Returns
    ///
    /// The route.
    ///
    /// # Errors
    ///
    /// A message saying how a route is written, where this is not one.
    fn from_str(text: &str) -> Result<Route, String> {
        let (listen, forward, both_ways) =
            match (text.split_once("<>"), text.split_once('>')) {
                (Some((listen, forward)), _) => (listen, forward, true),
                (None, Some((listen, forward))) => (listen, forward, false),
                (None, None) => {
                    return Err(format!(
                        "{text} is not a route: write LISTEN>FORWARD for one \
                         way or LISTEN<>FORWARD[@FROM] for both, each \
                         host:port"
                    ))
                }
            };
        let (forward, from) = match forward.split_once('@') {
            Some((forward, from)) if both_ways => (forward, Some(from)),
            Some(_) => {
                return Err(format!(
                    "{text}: only a route both ways says where it forwards \
                     from, as LISTEN<>FORWARD@FROM; one way forwards from \
                     where it listens"
                ))
            }
            None => (forward, None),
        };
        let address = |said: &str| {
            said.parse::<SocketAddr>().map_err(|e| {
                format!("{text}: {said} is not an address, host:port: {e}")
            })
        };
        Ok(Route {
            listen: address(listen)?,
            forward: address(forward)?,
            both_ways,
            from: from.map(address).transpose()?,
        })
    }
}

/// What one socket of a set does with what arrives at it.
#[derive(Debug, Clone, Copy)]
enum Way {
    /// Passes it on, from this socket, to an address.
    Forward(SocketAddr),
    /// Passes it on from another socket, the far side of a two-way relay,
    /// and remembers who sent it, to be answered.
    Ask { far: usize, forward: SocketAddr },
    /// Passes it back, from the near socket, to whoever last asked there.
    Answer { near: usize },
}

/// A datagram held until it is due.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Held {
    /// When it is due.
    due: Instant,
    /// The order it arrived in, so that two due together go in that order.
    order: u64,
    /// The socket it goes from.
    from: usize,
    /// Where it goes.
    to: SocketAddr,
    /// The datagram.
    data: Vec<u8>,
}

/// A set of relays, all served by one thread.
pub struct Relays {
    /// How long each datagram is held.
    delay: Duration,
    /// Each socket of the set.
    sockets: Vec<UdpSocket>,
    /// What each socket does with what arrives at it.
    ways: Vec<Way>,
    /// Who last asked at each socket, where it is the near side of a
    /// two-way relay.
    askers: Vec<Option<SocketAddr>>,
    /// The datagrams held, the soonest due first.
    held: BinaryHeap<Reverse<Held>>,
    /// How many datagrams have arrived, for their order.
    arrived: u64,
    /// Set to stop the thread serving the set.
    stopping: Arc<AtomicBool>,
    /// Wakes the thread serving the set, to stop.
    wakeup: Arc<Wakeup>,
}

/// What stops a set of relays being served, from another thread.
#[derive(Clone)]
pub struct Stopper {
    /// Set to stop.
    stopping: Arc<AtomicBool>,
    /// Wakes the serving thread to see it.
    wakeup: Arc<Wakeup>,
}

impl Stopper {
    /// Stops the set: its thread returns once it next looks.
    pub fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        self.wakeup.poke();
    }
}

impl Relays {
    /// Binds a set of relays.
    ///
    /// # Arguments
    ///
    /// * `routes` - Each relay.
    /// * `delay`  - How long each datagram is held.
    ///
    /// # Returns
    ///
    /// The set, not yet served.
    ///
    /// # Errors
    ///
    /// Where an address cannot be listened at, saying which.
    pub fn bind(routes: &[Route], delay: Duration) -> Result<Relays, String> {
        let bound = |at: SocketAddr| {
            UdpSocket::bind(at)
                .map_err(|e| format!("a relay cannot listen at {at}: {e}"))
        };
        let mut sockets = Vec::new();
        let mut ways = Vec::new();
        for route in routes {
            let near = sockets.len();
            sockets.push(bound(route.listen)?);
            if route.both_ways {
                let far = near + 1;
                let any = SocketAddr::from(([127, 0, 0, 1], 0));
                sockets.push(bound(route.from.unwrap_or(any))?);
                ways.push(Way::Ask {
                    far,
                    forward: route.forward,
                });
                ways.push(Way::Answer { near });
            } else {
                ways.push(Way::Forward(route.forward));
            }
        }
        let wakeup = Wakeup::new()
            .map_err(|e| format!("the relays cannot be woken: {e}"))?;
        Ok(Relays {
            delay,
            askers: vec![None; sockets.len()],
            sockets,
            ways,
            held: BinaryHeap::new(),
            arrived: 0,
            stopping: Arc::new(AtomicBool::new(false)),
            wakeup: Arc::new(wakeup),
        })
    }

    /// What stops this set being served, from another thread.
    ///
    /// # Returns
    ///
    /// The stopper.
    pub fn stopper(&self) -> Stopper {
        Stopper {
            stopping: Arc::clone(&self.stopping),
            wakeup: Arc::clone(&self.wakeup),
        }
    }

    /// Where each relay's near side listens, in the order the routes were
    /// given: what a route of port 0 was given.
    ///
    /// # Returns
    ///
    /// The addresses.
    pub fn listening(&self) -> Vec<SocketAddr> {
        self.ways
            .iter()
            .zip(&self.sockets)
            .filter(|(way, _)| !matches!(way, Way::Answer { .. }))
            .filter_map(|(_, socket)| socket.local_addr().ok())
            .collect()
    }

    /// Serves the set until stopped: receives, holds and sends.
    ///
    /// # Errors
    ///
    /// Where waiting on the sockets fails.
    pub fn serve(&mut self) -> Result<(), String> {
        let mut set: Vec<Watch> = self
            .sockets
            .iter()
            .map(|socket| Watch::new(socket.source(), Ready::READ))
            .collect();
        set.push(Watch::new(self.wakeup.source(), Ready::READ));
        let mut room = vec![0u8; LARGEST];
        while !self.stopping.load(Ordering::SeqCst) {
            self.send_what_is_due(Instant::now());
            let wait = self.held.peek().map(|Reverse(next)| {
                next.due.saturating_duration_since(Instant::now())
            });
            match tcsos::wait::wait(&mut set, wait) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(format!("the relays' wait failed: {e}")),
            }
            if set[self.sockets.len()].got.read {
                self.wakeup.drain();
            }
            let ready: Vec<usize> = set[..self.sockets.len()]
                .iter()
                .enumerate()
                .filter(|(_, watch)| watch.got.read)
                .map(|(at, _)| at)
                .collect();
            for at in ready {
                self.take(at, &mut room);
            }
        }
        Ok(())
    }

    /// Takes the datagram waiting at a socket, and holds it to be passed on
    /// -- unless a bit error loses it.
    ///
    /// # Arguments
    ///
    /// * `at`   - The socket.
    /// * `room` - Where it is read into.
    fn take(&mut self, at: usize, room: &mut [u8]) {
        let Ok((size, sender)) = self.sockets[at].recv_from(room) else {
            return;
        };
        let (from, to) = match self.ways[at] {
            Way::Forward(to) => (at, to),
            Way::Ask { far, forward } => {
                // Remembered even where the question is lost: a link that
                // loses a command still carried who asked.
                self.askers[at] = Some(sender);
                (far, forward)
            }
            Way::Answer { near } => match self.askers[near] {
                Some(asker) => (near, asker),
                // Nobody has asked yet: nowhere to answer.
                None => return,
            },
        };
        // Lost to a bit error, where the run asks the link to lose
        // datagrams: see tcsmoc::loss.
        if tcsmoc::loss::lost(size) {
            return;
        }
        self.arrived += 1;
        self.held.push(Reverse(Held {
            due: Instant::now() + self.delay,
            order: self.arrived,
            from,
            to,
            data: room[..size].to_vec(),
        }));
    }

    /// Sends every held datagram due by a moment.
    ///
    /// # Arguments
    ///
    /// * `now` - The moment.
    fn send_what_is_due(&mut self, now: Instant) {
        while self
            .held
            .peek()
            .is_some_and(|Reverse(next)| next.due <= now)
        {
            let Some(Reverse(held)) = self.held.pop() else {
                return;
            };
            if let Err(e) = self.sockets[held.from].send_to(&held.data, held.to)
            {
                eprintln!("tcs-relay: cannot send to {}: {e}", held.to);
            }
        }
    }
}

/// What `tcs-relay --help` prints.
const USAGE: &str = "\
usage: tcs-relay [--delay=SECONDS] [--simulator=FILE] ROUTE...

  ROUTE              LISTEN>FORWARD, passing what arrives at LISTEN on to
                     FORWARD, or LISTEN<>FORWARD, passing answers back too,
                     forwarding from the loopback or, with @FROM after it,
                     from FROM, the ground's own address; each address
                     host:port
  --delay=SECONDS    hold each datagram this long first (default: 0)
  --simulator=FILE   the run's simulator file: its simulation mode, which
                     must not be none, the flight setting, for a delay or
                     loss; and the bit errors its gateway section asks for
  -h, --help         print this, and exit
";

/// Runs `tcs-relay`: relays between a ground station and a vehicle, until
/// it is stopped.
///
/// Exits with status 2 on a command line it cannot take, and 1 where the
/// relays are refused or cannot listen.
pub fn main() {
    let args: Vec<String> = tcsos::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print!("{USAGE}");
        return;
    }
    match run(&args) {
        Ok(()) => {}
        Err((status, why)) => {
            eprintln!("tcs-relay: {why}");
            tcsos::process::exit(status);
        }
    }
}

/// Reads the command line, checks the run allows what it asks, and serves
/// the relays.
///
/// # Arguments
///
/// * `args` - The arguments, without the program's name.
///
/// # Errors
///
/// The status to exit with, and why.
fn run(args: &[String]) -> Result<(), (i32, String)> {
    let mut delay = Duration::ZERO;
    let mut sim = None;
    let mut routes = Vec::new();
    for arg in args {
        if let Some(seconds) = arg.strip_prefix("--delay=") {
            delay = seconds
                .parse::<f64>()
                .ok()
                .and_then(|s| Duration::try_from_secs_f64(s).ok())
                .ok_or_else(|| {
                    (
                        2,
                        format!(
                            "{arg}: give a number of seconds, nought or more"
                        ),
                    )
                })?;
        } else if let Some(file) = arg.strip_prefix("--simulator=") {
            sim = Some(file.to_string());
        } else if arg.starts_with("--") {
            return Err((2, format!("{arg} is not an option\n{USAGE}")));
        } else {
            routes.push(arg.parse::<Route>().map_err(|why| (2, why))?);
        }
    }
    if routes.is_empty() {
        return Err((2, format!("nothing to relay\n{USAGE}")));
    }
    let mode = match &sim {
        Some(sim) => {
            tcsprotocol::simulate::mode_of(sim).map_err(|why| (1, why))?
        }
        None => tcsprotocol::simulate::Mode::None,
    };
    if !delay.is_zero() {
        tcsmoc::simulation::allow(
            &format!("--delay={}", delay.as_secs_f64()),
            mode,
            sim.as_deref(),
        )
        .map_err(|why| (1, why))?;
    }
    if let Some(armed) =
        tcsmoc::loss::arm_for(sim.as_deref(), mode).map_err(|why| (1, why))?
    {
        eprintln!("tcs-relay: {armed}");
    }
    let mut relays = Relays::bind(&routes, delay).map_err(|why| (1, why))?;
    relays.serve().map_err(|why| (1, why))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Routes are read one way and both ways, and anything else is refused
    /// saying how one is written.
    #[test]
    fn routes_are_read_as_written() {
        let one: Route = "127.0.0.1:1>127.0.0.1:2".parse().expect("a route");
        assert_eq!(one.listen.port(), 1);
        assert_eq!(one.forward.port(), 2);
        assert!(!one.both_ways);
        let both: Route = "127.0.0.1:3<>127.0.0.1:4".parse().expect("a route");
        assert!(both.both_ways);
        assert_eq!((both.listen.port(), both.forward.port()), (3, 4));
        assert_eq!(both.from, None);
        let from: Route = "127.0.0.1:3<>127.0.0.1:4@127.0.0.5:0"
            .parse()
            .expect("a route");
        assert_eq!(from.from, Some("127.0.0.5:0".parse().expect("an address")));
        let said = "127.0.0.1:1".parse::<Route>().unwrap_err();
        assert!(said.contains("LISTEN>FORWARD"), "{said}");
        assert!("x>127.0.0.1:2".parse::<Route>().is_err());
        let said = "127.0.0.1:1>127.0.0.1:2@127.0.0.5:0"
            .parse::<Route>()
            .unwrap_err();
        assert!(said.contains("only a route both ways"), "{said}");
    }

    /// A datagram is passed on one way after the delay, and a two-way
    /// relay passes the answer back to whoever asked.
    #[test]
    fn datagrams_are_held_and_answers_come_back() {
        let vehicle = UdpSocket::bind("127.0.0.1:0").expect("a socket");
        let ground = UdpSocket::bind("127.0.0.1:0").expect("a socket");
        let any: SocketAddr = "127.0.0.1:0".parse().expect("an address");
        let routes = [
            Route {
                listen: any,
                forward: ground.local_addr().expect("bound"),
                both_ways: false,
                from: None,
            },
            Route {
                listen: any,
                forward: vehicle.local_addr().expect("bound"),
                both_ways: true,
                from: Some("127.0.0.5:0".parse().expect("an address")),
            },
        ];
        let delay = Duration::from_millis(200);
        let mut relays = Relays::bind(&routes, delay).expect("bound");
        let [down, up] = relays.listening()[..] else {
            panic!("two relays");
        };
        let stopper = relays.stopper();
        let serving = tcsos::thread::spawn(move || relays.serve());

        let sent = Instant::now();
        vehicle.send_to(b"telemetry", down).expect("sent");
        let mut room = [0u8; 64];
        let (size, _) = ground.recv_from(&mut room).expect("arrived");
        assert_eq!(&room[..size], b"telemetry");
        assert!(sent.elapsed() >= delay, "held {:?}", sent.elapsed());

        ground.send_to(b"command", up).expect("sent");
        let (size, relay) = vehicle.recv_from(&mut room).expect("arrived");
        assert_eq!(&room[..size], b"command");
        // From the ground's address the route named, which the vehicle
        // takes commands from.
        assert_eq!(relay.ip().to_string(), "127.0.0.5");
        vehicle.send_to(b"answer", relay).expect("sent");
        let (size, from) = ground.recv_from(&mut room).expect("answered");
        assert_eq!(&room[..size], b"answer");
        assert_eq!(from, up, "from the relay the command went to");

        stopper.stop();
        serving.join().expect("it ends").expect("it served");
    }
}
