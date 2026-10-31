//! Pinging a `ping` node, from the ground.
//!
//! A `ping` vehicle node answers each ping the ground sends it with the time
//! the vehicle had it written on the end: see `tcslibgs::node::vehicle::ping`,
//! which is what a ping and its answer say. This is the ground's half of the
//! exchange, as [`crate::beacon`] is the ground's half of a beacon, and with no
//! window in it for the same reason: tcsmoc shows it in a panel, and a program
//! with no screen -- a test, a logger -- wants it without a toolkit.
//!
//! Three pieces:
//!
//! * [`route_of`] finds a `ping` node the ground can reach, and the `gateway`
//!   node a ping goes up through and its answer comes down through.
//! * [`Uplink`] sends to a gateway node: the address and port it binds aboard,
//!   which is what leaves it by its `gateway-out` links. The answer comes down
//!   as any node's data does, through [`crate::gateway::GatewayReceiver`].
//! * [`Pinger`] keeps what there is to show: the last ping sent and when, the
//!   last answer and when, and the three times between them.
//!
//! # What is kept
//!
//! A ping sent clears the time received and the three times, because the
//! answer they were about is not the answer to this ping. They come back when
//! an answer arrives whose count is the count of the ping last sent. An answer
//! to an earlier ping is still the last thing received, and changes none of
//! the times.
//!
//! # The three times
//!
//! The round trip is the ground's clock at both ends. Up -- the ground sending
//! the ping to the vehicle having it -- and down -- the vehicle having it to
//! the ground receiving the answer -- are each read across two clocks, so each
//! includes how far apart they are, and either may be negative. Their sum is
//! the round trip whatever the offset, which is how to tell a clock that is
//! out from a link that is slow.
//!
//! `examples/ping/ping_ground.py` is this in Python, for the example's
//! Python ground station.

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration;

use tcslibgs::graph::reach::reach_of;
use tcslibgs::graph::telemetry::{self, Via};
use tcslibgs::graph::{Destination, Graph};
use tcslibgs::node::vehicle::ping;
use tcslibgs::EndpointConfig;

/// A `ping` node the ground can reach, and the gateway node between them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// The `ping` node.
    pub node: String,
    /// The gateway node a ping goes up through, by its `gateway-out` link to
    /// the `ping` node, and the answer comes down through, by the `ping` node's
    /// link to it.
    pub gateway: String,
}

/// Finds the first `ping` node in the configuration the ground can reach
/// both ways, in the order the file writes them.
///
/// Both ways: a gateway node that binds an address and port aboard -- somewhere
/// for a ping to go up to -- with a link to the `ping` node, and a link from
/// the `ping` node back down to that same gateway node.
///
/// # Arguments
///
/// * `graph` - The configuration's graph.
///
/// # Returns
///
/// The `ping` node and its gateway node, or `None` where there is no such
/// node, which is a configuration with nothing to ping; a ping node
/// reachable one way only is one a ping would vanish into.
pub fn route_of(graph: &Graph) -> Option<Route> {
    let down = telemetry::of(graph).ok()?;
    graph
        .nodes
        .iter()
        .filter(|node| node.node_type == "ping")
        .find_map(|node| {
            down.iter()
                .filter(|link| link.source == node.name)
                .find_map(|link| match &link.via {
                    Via::Gateway(gateway)
                        if feeds(graph, gateway, &node.name)
                            && binds(graph, gateway) =>
                    {
                        Some(Route {
                            node: node.name.clone(),
                            gateway: gateway.clone(),
                        })
                    }
                    _ => None,
                })
        })
}

/// Tells whether a link leaves one node for another.
///
/// # Arguments
///
/// * `graph` - The configuration's graph.
/// * `from`  - The name of the node the link leaves.
/// * `to`    - The name of the node it goes to, among its destinations.
///
/// # Returns
///
/// `true` if some link leaves `from` for `to`.
fn feeds(graph: &Graph, from: &str, to: &str) -> bool {
    graph.links.iter().any(|link| {
        link.origin().node == from
            && link
                .to
                .each()
                .iter()
                .any(|destination| Destination::parse(destination).node == to)
    })
}

/// Whether a gateway node binds an address and port aboard.
fn binds(graph: &Graph, gateway: &str) -> bool {
    graph
        .node(gateway)
        .is_some_and(tcslibgs::graph::reach::binds)
}

/// The ground's end of a gateway node's way up.
#[derive(Debug)]
pub struct Uplink {
    socket: UdpSocket,
    to: SocketAddr,
}

impl Uplink {
    /// Opens the way up to a gateway node: where it binds aboard, which the
    /// configuration says.
    ///
    /// # Arguments
    ///
    /// * `graph`   - The configuration's graph.
    /// * `gateway` - The name of the gateway node.
    ///
    /// # Returns
    ///
    /// The uplink, with a socket of the address's family to send from.
    ///
    /// # Errors
    ///
    /// A message saying why, for a node that does not exist, is not a
    /// gateway node, or binds nothing -- it only sends, to its
    /// `ground_base`, and has nowhere for the ground to send to -- and for
    /// an address that does not resolve or a socket that cannot be made.
    pub fn open(graph: &Graph, gateway: &str) -> Result<Uplink, String> {
        let node = graph
            .node(gateway)
            .ok_or_else(|| format!("there is no node called \"{gateway}\""))?;
        if !tcslibgs::gateway::is_gateway(&node.node_type) {
            return Err(format!(
                "\"{gateway}\" is a {} node, not a gateway node",
                node.node_type
            ));
        }
        if !tcslibgs::graph::reach::binds(node) {
            return Err(format!(
                "\"{gateway}\" states no address and port, so it binds nothing \
                 aboard and the ground has nowhere to send to it"
            ));
        }
        let EndpointConfig::Network(at) = reach_of(node)?.endpoint else {
            return Err(format!("\"{gateway}\" is not reached over a socket"));
        };
        let to = (at.address.as_str(), at.port)
            .to_socket_addrs()
            .map_err(|e| {
                format!("\"{gateway}\" at {}:{}: {e}", at.address, at.port)
            })?
            .next()
            .ok_or_else(|| {
                format!(
                    "\"{gateway}\" at {}:{} is no address",
                    at.address, at.port
                )
            })?;
        let socket =
            UdpSocket::bind(if to.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" })
                .map_err(|e| {
                    format!("a socket to send to \"{gateway}\" from: {e}")
                })?;
        Ok(Uplink { socket, to })
    }

    /// Where it sends to.
    pub fn to(&self) -> SocketAddr {
        self.to
    }

    /// Sends bytes up, in one datagram.
    ///
    /// # Arguments
    ///
    /// * `data` - What to send.
    ///
    /// # Returns
    ///
    /// Nothing, once it has gone.
    ///
    /// # Errors
    ///
    /// A message saying where it could not be sent, and why.
    pub fn send(&self, data: &[u8]) -> Result<(), String> {
        self.socket
            .send_to(data, self.to)
            .map(|_| ())
            .map_err(|e| format!("cannot send to {}: {e}", self.to))
    }
}

/// What there is to show of the pings, at one moment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Exchange {
    /// The last ping sent, as it went.
    pub sent: Option<String>,
    /// When it went, since the epoch, by the ground's clock.
    pub sent_at: Option<Duration>,
    /// The last thing received from the `ping` node, as it arrived.
    pub received: Option<String>,
    /// When the answer to the last ping arrived, by the ground's clock;
    /// `None` from the moment a ping is sent until its answer arrives.
    pub received_at: Option<Duration>,
    /// When the vehicle had the last ping, by the vehicle's clock.
    pub answered_at: Option<Duration>,
}

/// Nanoseconds from `from` to `to`, which may be negative.
fn between(from: Duration, to: Duration) -> i128 {
    to.as_nanos() as i128 - from.as_nanos() as i128
}

impl Exchange {
    /// Ground sent to ground received, in nanoseconds.
    pub fn round_trip(&self) -> Option<i128> {
        Some(between(self.sent_at?, self.received_at?))
    }

    /// Ground sent to vehicle had, in nanoseconds, across two clocks.
    pub fn up(&self) -> Option<i128> {
        self.received_at?;
        Some(between(self.sent_at?, self.answered_at?))
    }

    /// Vehicle had to ground received, in nanoseconds, across two clocks.
    pub fn down(&self) -> Option<i128> {
        Some(between(self.answered_at?, self.received_at?))
    }
}

/// The ground's count of pings and what came of the last.
///
/// The text of the last ping and of the last answer is written each time
/// into room kept from the time before, so that pinging and folding in the
/// answers allocates nothing once the first of each has been made.
#[derive(Debug)]
pub struct Pinger {
    count: u64,
    exchange: Exchange,
    /// The room the next ping is written in, which changes places with the
    /// last one sent; the room the first ping sent is kept in; and the room
    /// for an answer before one has arrived. All made with the pinger.
    room_sent: String,
    room_first: String,
    room_received: String,
}

impl Default for Pinger {
    fn default() -> Pinger {
        Pinger::new()
    }
}

/// The room a pinger keeps for what it was sent, which is more than an
/// answer and enough for most of what is not one.
const RECEIVED_BYTES: usize = 256;

impl Pinger {
    /// Makes a pinger that has sent no ping yet, with the room its text is
    /// kept in.
    ///
    /// # Returns
    ///
    /// The pinger.
    pub fn new() -> Pinger {
        Pinger {
            count: 0,
            exchange: Exchange::default(),
            room_sent: String::with_capacity(ping::MESSAGE_BYTES),
            room_first: String::with_capacity(ping::MESSAGE_BYTES),
            room_received: String::with_capacity(RECEIVED_BYTES),
        }
    }

    /// What there is to show now.
    pub fn exchange(&self) -> &Exchange {
        &self.exchange
    }

    /// Makes the next ping: the count goes up by one, from one, and the
    /// time received is cleared. What was received last is kept.
    ///
    /// # Arguments
    ///
    /// * `at` - When it is sent, since the epoch, by the ground's clock.
    ///
    /// # Returns
    ///
    /// The ping to send, which is the exchange's `sent`.
    ///
    /// # Errors
    ///
    /// A message if the count has run out or the ping cannot be written;
    /// the last ping is then left as it was sent.
    pub fn ping(&mut self, at: Duration) -> Result<&str, String> {
        let count = self
            .count
            .checked_add(1)
            .ok_or("the count of pings has run out")?;
        // Written into the room the ping before last was in, and the last
        // ping's room kept for the next: a refused ping leaves the last one
        // as it was sent.
        let room = &mut self.room_sent;
        room.clear();
        ping::write_message(count, at, room)?;
        match &mut self.exchange.sent {
            Some(last) => std::mem::swap(last, room),
            None => {
                let first = std::mem::take(&mut self.room_first);
                self.exchange.sent = Some(std::mem::replace(room, first));
            }
        }
        self.count = count;
        self.exchange.sent_at = Some(at);
        self.exchange.received_at = None;
        self.exchange.answered_at = None;
        Ok(self.exchange.sent.as_deref().unwrap_or_default())
    }

    /// Fold in what the `ping` node sent down, which arrived at `at`.
    ///
    /// Shown whatever it is, because something arrived and an operator should
    /// see what; timed only where it is the answer to the last ping sent.
    ///
    /// Allocates where what arrived is not an answer, to say why: something
    /// is sending the ground what the ping node would not.
    ///
    /// # Arguments
    ///
    /// * `data` - What arrived.
    /// * `at`   - When it arrived, since the epoch, by the ground's clock.
    pub fn answered(&mut self, data: &[u8], at: Duration) {
        use std::fmt::Write;

        let mut received = self
            .exchange
            .received
            .take()
            .unwrap_or_else(|| std::mem::take(&mut self.room_received));
        received.clear();
        match ping::read_reply(data) {
            Ok(pong) => {
                received.push_str(&String::from_utf8_lossy(data));
                if pong.ping.count == self.count {
                    self.exchange.received_at = Some(at);
                    self.exchange.answered_at = Some(pong.answered);
                }
            }
            Err(why) => {
                let _ = write!(received, "not an answer: {why}");
            }
        }
        self.exchange.received = Some(received);
    }
}

/// What a panel shows for a ping or an answer before there is one: the
/// beacon panel's placeholder, so that the two say "nothing yet" alike.
pub const NO_MESSAGE: &str = crate::beacon::NO_BEACON_MESSAGE;

/// What [`precise_time`] shows for no time: the beacon panel's `--:--:--`,
/// with the fraction, so that a field keeps its width when one arrives.
pub const NO_PRECISE_TIME: &str = "--:--:--.---------";

/// What [`interval`] shows for no length of time: the shape of one, as
/// [`NO_PRECISE_TIME`] is the shape of a time.
pub const NO_INTERVAL: &str = "--.------ ms";

/// Shows a time of day to the nanosecond, UTC, as a ping is timed: the
/// panels' `HH:MM:SS` with the fraction a ping is about.
///
/// # Arguments
///
/// * `since_epoch` - The time, since the epoch, or `None` for no time.
///
/// # Returns
///
/// The time as `HH:MM:SS.nnnnnnnnn`, or [`NO_PRECISE_TIME`] for no time.
pub fn precise_time(since_epoch: Option<Duration>) -> String {
    let Some(at) = since_epoch else {
        return NO_PRECISE_TIME.to_string();
    };
    let within_a_day = at.as_secs() % 86_400;
    format!(
        "{:02}:{:02}:{:02}.{:09}",
        within_a_day / 3600,
        (within_a_day % 3600) / 60,
        within_a_day % 60,
        at.subsec_nanos()
    )
}

/// Shows a length of time between two of a ping's times, in milliseconds
/// to the nanosecond, with its sign.
///
/// # Arguments
///
/// * `nanoseconds` - The length of time, which may be negative, or `None`
///                   for none.
///
/// # Returns
///
/// The length as `[-]M.mmmmmm ms`, or [`NO_INTERVAL`] for none.
pub fn interval(nanoseconds: Option<i128>) -> String {
    let Some(nanoseconds) = nanoseconds else {
        return NO_INTERVAL.to_string();
    };
    let sign = if nanoseconds < 0 { "-" } else { "" };
    let magnitude = nanoseconds.unsigned_abs();
    format!(
        "{sign}{}.{:06} ms",
        magnitude / 1_000_000,
        magnitude % 1_000_000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: Duration = Duration::new(1_760_900_000, 0);

    fn after(nanoseconds: u64) -> Duration {
        AT + Duration::from_nanos(nanoseconds)
    }

    /// A ping goes numbered from one and timed, and the next counts on.
    #[test]
    fn a_ping_is_numbered_and_timed() {
        let mut pinger = Pinger::new();
        assert_eq!(pinger.ping(AT), Ok("ping #1 1760900000.000000000"));
        let second = pinger.ping(after(1)).expect("the second");
        assert_eq!(
            ping::read_message(second.as_bytes()).expect("a ping").count,
            2
        );
        assert_eq!(pinger.exchange().sent_at, Some(after(1)));
    }

    /// The answer to the last ping is timed, and the two halves sum to the
    /// round trip.
    #[test]
    fn the_answer_to_the_last_ping_is_timed() {
        let mut pinger = Pinger::new();
        let message = pinger.ping(AT).expect("a ping");
        let answer = ping::reply(message.as_bytes(), after(3_000_000))
            .expect("answered");
        pinger.answered(answer.as_bytes(), after(5_000_000));

        let exchange = pinger.exchange();
        assert_eq!(
            exchange.received.as_deref(),
            Some("ping #1 1760900000.000000000 1760900000.003000000")
        );
        assert_eq!(exchange.received_at, Some(after(5_000_000)));
        assert_eq!(exchange.round_trip(), Some(5_000_000));
        assert_eq!(exchange.up(), Some(3_000_000));
        assert_eq!(exchange.down(), Some(2_000_000));
    }

    /// A ping clears the time received and the three times, and keeps what
    /// was received last.
    #[test]
    fn a_ping_clears_the_time_received_and_keeps_what_was() {
        let mut pinger = Pinger::new();
        let first = pinger.ping(AT).expect("a ping").to_string();
        pinger.answered(
            ping::reply(first.as_bytes(), after(1))
                .expect("answered")
                .as_bytes(),
            after(2),
        );
        let received = pinger.exchange().received.clone();

        pinger.ping(after(1_000)).expect("another");
        let exchange = pinger.exchange();
        assert_eq!(exchange.received_at, None);
        assert_eq!(exchange.round_trip(), None);
        assert_eq!(exchange.up(), None);
        assert_eq!(exchange.down(), None);
        assert_eq!(exchange.received, received);
    }

    /// An earlier ping's answer is shown and not timed, and so is what is
    /// not an answer at all.
    #[test]
    fn what_does_not_answer_the_last_ping_is_shown_and_not_timed() {
        let mut pinger = Pinger::new();
        let first = pinger.ping(AT).expect("a ping").to_string();
        pinger.ping(after(1_000)).expect("another");
        pinger.answered(
            ping::reply(first.as_bytes(), after(7))
                .expect("answered")
                .as_bytes(),
            after(2_000),
        );
        assert!(pinger
            .exchange()
            .received
            .as_deref()
            .unwrap()
            .starts_with("ping #1 "));
        assert_eq!(
            pinger.exchange().received_at,
            None,
            "it answers ping #1, not #2"
        );

        pinger.answered(b"hello", after(3_000));
        assert!(pinger
            .exchange()
            .received
            .as_deref()
            .unwrap()
            .starts_with("not an answer: "));
        assert_eq!(pinger.exchange().received_at, None);
    }

    /// A time is shown to the nanosecond and UTC, and an interval in
    /// milliseconds with its sign: the same as the Python's.
    #[test]
    fn times_are_shown_as_the_python_shows_them() {
        assert_eq!(
            precise_time(Some(Duration::new(3600 + 120 + 3, 4))),
            "01:02:03.000000004"
        );
        assert_eq!(precise_time(None), "--:--:--.---------");
        assert_eq!(interval(Some(16_441_799)), "16.441799 ms");
        assert_eq!(interval(Some(-250_000)), "-0.250000 ms");
        assert_eq!(interval(Some(0)), "0.000000 ms");
        assert_eq!(interval(None), "--.------ ms");
        assert_eq!(NO_MESSAGE, "<none>");
    }

    /// The shipped example's ping node is found, through its gateway node, and
    /// a ping goes up to where that node binds.
    #[test]
    fn the_example_is_routed_and_its_gateway_node_is_sent_to() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let graph = Graph::load(root.join("examples/ping/ping.yaml"))
            .expect("it reads");
        assert_eq!(
            route_of(&graph),
            Some(Route {
                node: "ping".to_string(),
                gateway: "gateway".to_string()
            })
        );
        let uplink = Uplink::open(&graph, "gateway").expect("it binds aboard");
        assert_eq!(uplink.to().to_string(), "127.0.0.1:7600");

        // And a configuration whose gateway node binds nothing has no way up.
        let beacon = Graph::load(root.join("examples/beacon/beacon.yaml"))
            .expect("it reads");
        assert_eq!(route_of(&beacon), None);
    }

    /// What is sent up arrives where the gateway node binds.
    #[test]
    fn an_uplink_sends_to_where_the_gateway_node_binds() {
        let vehicle = UdpSocket::bind("127.0.0.1:0").expect("a port");
        vehicle
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("a timeout");
        let port = vehicle.local_addr().expect("an address").port();
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("vehicle.yaml");
        std::fs::write(
            &path,
            format!(
                "gateway:\n  - {{ name: gateway, type: gateway_radio, \
                 address: \"127.0.0.1\", port: \"{port}\", \
                 ground_base: \"127.0.0.1:9000\" }}\n"
            ),
        )
        .expect("written");
        let graph = Graph::load(&path).expect("it reads");

        Uplink::open(&graph, "gateway")
            .expect("it binds aboard")
            .send(b"ping #1 1760900000.000000000")
            .expect("sent");
        let mut buffer = [0u8; 64];
        let (got, _) = vehicle.recv_from(&mut buffer).expect("it arrived");
        assert_eq!(&buffer[..got], b"ping #1 1760900000.000000000");

        assert!(Uplink::open(&graph, "nobody").is_err());
    }
}
