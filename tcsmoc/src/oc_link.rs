//! What arrives from a payload, as the ground receives it.
//!
//! The MOC commanded payloads and asked them what they had moved, and never
//! read a byte of what they sent. Every panel reading was therefore the
//! spacecraft's own account: the statistics it counted and the samples it
//! kept. Those are worth having -- they are what a handler saw -- but they
//! are not evidence that anything reached the ground, and the one reading an
//! operator wants from a payload that sends on its own is the data itself.
//!
//! Worse, nothing on the ground sending to a handler's OC address meant the
//! handler could not send at all. A handler's OC endpoint binds the address
//! the ground sends to and learns where to answer from what arrives there:
//! `recv_from`, then `send_to`. With nobody there it had no peer, so every
//! write to the ground failed, and a payload's data went into a handler and
//! no further.
//!
//! So each handler with an OC address gets a listener here. It says hello --
//! an empty datagram, which teaches the handler where the ground is and
//! carries nothing into the payload link -- and then reads what the handler
//! sends, keeping the latest for the panel.

use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tcslibgs::{came_from, DHConfig, DHSample, HostPort, NetworkConfig, Timestamp};

/// How long a listener waits for data before saying hello again.
///
/// Said again because a handler forgets: STOP_DH and START_DH rebind its OC
/// socket, and the peer it learnt went with the socket. A ground station that
/// said hello once would then be silently cut off, which is exactly the fault
/// this exists to remove. One second, so a restarted handler is heard from
/// again before an operator has finished reading the panel.
const HELLO_INTERVAL: Duration = Duration::from_secs(1);

/// What one payload last sent to the ground, as the ground received it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arrival {
    /// When it arrived here. The ground's clock, and said so: the spacecraft
    /// puts no time in a payload datagram, and the panel's other times are
    /// the spacecraft's.
    pub at: Timestamp,
    /// The head of what arrived, kept the way a handler's samples are so that
    /// the panel shows the three lines alike.
    pub data: DHSample,
    /// How many have arrived from this payload since the MOC started.
    ///
    /// Counted here because this is what counts them: the panel's other two
    /// lines are what the ground received, and so is this. The count the
    /// panel showed before was the spacecraft's own -- the bytes its handler
    /// had read from the ground, which is the other direction and reads
    /// nought for a payload nobody sends to.
    pub packets: u64,
}

impl Arrival {
    /// The two strings a panel shows: when, and what.
    pub fn panel_lines(&self) -> (String, String) {
        let (_, data) = self.data.panel_lines();
        (self.at.time_of_day(), data)
    }
}

/// What every payload last sent to the ground, one slot per panel row.
pub type Arrivals = Arc<Mutex<Vec<Option<Arrival>>>>;

/// Open the payload data path for every handler that has one.
///
/// One socket and one thread per handler, because one handler's data has
/// nothing to do with another's and a datagram is answered where it arrived.
/// A handler with no data link gets neither: it cannot be started, so there is
/// nothing to listen for.
///
/// Each socket is bound where the configuration says the ground's end of that
/// payload's link is -- `payload_telemetry.dest_base` counted on by the payload's id -- and
/// says hello to where tcspecial's end is. Both ends come out of the one file
/// both ends read, so neither has to be told the other's.
///
/// Nothing is reported from here. A payload data socket that cannot be opened
/// is not a reason to refuse to command payloads, which is what the MOC is
/// for; the panel then shows no arrivals, which is the truth about what the
/// ground received.
pub fn listen(dh_configs: &[DHConfig]) -> Arrivals {
    let arrivals: Arrivals = Arc::new(Mutex::new(vec![None; dh_configs.len()]));

    for (row, dh) in dh_configs.iter().enumerate() {
        let Some(oc) = &dh.oc else { continue };
        let Some(address) = oc_address(oc) else {
            continue;
        };

        // The ground's own end of this payload's data link, where the
        // configuration says it is: `payload_telemetry.dest_base` counted on by the payload's
        // id. It was whatever port the system chose, which worked because the
        // hello told tcspecial where to answer -- and left the ground's own
        // ports unwritten anywhere, so nothing between the two ends could be
        // checked or firewalled by a rule anybody could state.
        //
        // A set with no section has no end to bind, as it has none to send
        // to.
        let Some(ground) = &dh.ground else { continue };
        let socket = match UdpSocket::bind(format!("{}:{}", ground.address, ground.port))
        {
            Ok(socket) => socket,
            Err(_) => continue,
        };
        if socket.set_read_timeout(Some(HELLO_INTERVAL)).is_err() {
            continue;
        }

        let slot = arrivals.clone();
        // Where this payload's telemetry is supposed to come from, which is
        // the same place the hello goes to: `payload_telemetry.source_base`
        // counted on by the payload's id.
        let expected = HostPort {
            address: oc.address.clone(),
            port: oc.port,
        };
        thread::spawn(move || serve(socket, address, expected, slot, row));
    }

    arrivals
}

/// Where a handler's OC endpoint is, as an address to send to.
fn oc_address(oc: &NetworkConfig) -> Option<SocketAddr> {
    format!("{}:{}", oc.address, oc.port).parse().ok()
}

/// Say hello, then keep what arrives.
fn serve(
    socket: UdpSocket,
    handler: SocketAddr,
    expected: HostPort,
    arrivals: Arrivals,
    row: usize,
) {
    let mut buffer = vec![0u8; 65535];

    // How many have arrived from this payload. Kept here rather than read
    // back out of the slot, so that a panel that has not been looked at yet
    // cannot cost a count.
    let mut packets = 0u64;

    loop {
        // An empty datagram. The handler's reader records the address it came
        // from and hands up nought bytes, which its conduit drops rather than
        // writing on to the payload -- so this teaches the handler where the
        // ground is without putting a byte into the payload's link.
        if socket.send_to(&[], handler).is_err() {
            // Nothing to be done about it here, and the next pass tries
            // again: a handler that has not been started yet is the usual
            // reason, and it will be.
            thread::sleep(HELLO_INTERVAL);
            continue;
        }

        // Everything that arrives until the hello is due again. Each datagram
        // replaces the one before it: a panel shows the last thing that came,
        // as it does for the spacecraft's own samples.
        loop {
            match socket.recv_from(&mut buffer) {
                // Only from the payload's own end of this link. A datagram
                // from anywhere else is another payload's, or another
                // program's, and showing it in this panel would be reporting
                // one payload's telemetry as another's.
                Ok((_, from)) if !came_from(&expected, &from) => {
                    eprintln!(
                        "a payload datagram came from {from} and not from \
                         {expected}, which is where the configuration says this \
                         payload sends from -- ignored"
                    );
                }
                Ok((size, _from)) if size > 0 => {
                    let mut data = DHSample::new();
                    data.record(&buffer[..size]);
                    packets += 1;
                    if let Ok(mut slots) = arrivals.lock() {
                        if let Some(slot) = slots.get_mut(row) {
                            *slot = Some(Arrival {
                                at: Timestamp::now(),
                                data,
                                packets,
                            });
                        }
                    }
                }
                // A datagram of no bytes is somebody else saying hello, which
                // is not data from a payload.
                Ok(_) => {}
                // The read timed out, which is how often hello is said.
                Err(_) => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslibgs::{DHMode, DHName, EndpointConfig, NetworkProtocol};

    /// A handler as a payload set describes one: both ends of its data link,
    /// which the two bases of the tcspecial section give it.
    ///
    /// The ground's end is bound by the listener, so it is a port the system
    /// chooses here: a test that named one would fail whenever something else
    /// on the machine happened to hold it.
    fn handler(oc: Option<SocketAddr>) -> DHConfig {
        DHConfig {
            name: DHName::new("auto-send"),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: "localhost".to_string(),
                port: 5000,
            }),
            packet_size: 12,
            oc: oc.map(|addr| NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: addr.ip().to_string(),
                port: addr.port(),
            }),
            ground: oc.map(|_| NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: "127.0.0.1".to_string(),
                port: 0,
            }),
            mode: DHMode::Periodic,
        }
    }

    /// The ground says hello and then keeps what the handler sends.
    ///
    /// Both halves matter. Without the hello a handler has no peer and cannot
    /// send at all -- which is how a payload's data used to reach a handler
    /// and go no further -- and without keeping what arrives there is nothing
    /// for a panel to show.
    #[test]
    fn the_ground_says_hello_and_keeps_what_arrives() {
        // Standing in for a handler's OC endpoint, which binds the address
        // the ground sends to.
        let oc = UdpSocket::bind("127.0.0.1:0").expect("a handler's OC socket");
        oc.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a timeout");
        let at = oc.local_addr().expect("where it is");

        let arrivals = listen(&[handler(Some(at))]);

        // The hello: a datagram of no bytes, which is what teaches a handler
        // where the ground is without carrying anything into the payload.
        let mut buffer = [0u8; 64];
        let (size, ground) = oc.recv_from(&mut buffer).expect("the ground said hello");
        assert_eq!(size, 0, "the hello must carry nothing into the payload link");

        // What a payload sent, as a handler would forward it.
        oc.send_to(b"from the payload", ground).expect("it is sent");

        let found = (0..50)
            .find_map(|_| {
                thread::sleep(Duration::from_millis(20));
                arrivals.lock().ok().and_then(|slots| slots[0].clone())
            })
            .expect("the ground kept what arrived");
        let (time, data) = found.panel_lines();
        assert_eq!(time.len(), "00:00:00".len(), "a time of day: {time}");
        assert!(
            data.contains("66") && data.contains("72"),
            "the first bytes of \"from the payload\": {data}"
        );
        assert_eq!(found.packets, 1, "one datagram arrived, and it was counted");

        // And a second is counted as a second, which is what the panel shows.
        oc.send_to(b"again", ground).expect("it is sent");
        let counted = (0..50)
            .find_map(|_| {
                thread::sleep(Duration::from_millis(20));
                arrivals
                    .lock()
                    .ok()
                    .and_then(|slots| slots[0].clone())
                    .filter(|arrival| arrival.packets == 2)
            })
            .expect("the second arrival was not counted");
        assert_eq!(counted.packets, 2);
    }

    /// Only what comes from the payload's own end is kept.
    ///
    /// A datagram from somewhere else at this socket is another payload's or
    /// another program's, and putting it in this panel would be reporting one
    /// payload's telemetry as another's. The payload's own datagram follows
    /// the stranger's here, so this says which was kept rather than that
    /// nothing arrived.
    #[test]
    fn a_datagram_from_elsewhere_is_not_this_payloads() {
        let oc = UdpSocket::bind("127.0.0.1:0").expect("a handler's OC socket");
        oc.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a timeout");
        let at = oc.local_addr().expect("where it is");

        let arrivals = listen(&[handler(Some(at))]);

        let mut buffer = [0u8; 64];
        let (_, ground) = oc.recv_from(&mut buffer).expect("the ground said hello");

        // A stranger first, then the payload: what the panel ends up with
        // must be the payload's, and the count must be one.
        let stranger = UdpSocket::bind("127.0.0.1:0").expect("another socket");
        stranger.send_to(b"\xff\xff\xff", ground).expect("it is sent");
        oc.send_to(b"from the payload", ground).expect("it is sent");

        let found = (0..50)
            .find_map(|_| {
                thread::sleep(Duration::from_millis(20));
                arrivals.lock().ok().and_then(|slots| slots[0].clone())
            })
            .expect("the ground kept what arrived");
        let (_, data) = found.panel_lines();
        assert!(
            data.contains("66") && data.contains("72"),
            "the payload's own bytes: {data}"
        );
        assert_eq!(
            found.packets, 1,
            "the stranger's datagram was counted as the payload's"
        );
    }

    /// A handler with no OC address is not listened for.
    ///
    /// It cannot be started -- there is nowhere for its data to go -- so
    /// there is nothing to hear, and a socket opened for it would be a thread
    /// waiting on nobody.
    #[test]
    fn a_handler_with_no_oc_address_is_not_listened_for() {
        let arrivals = listen(&[handler(None)]);
        assert_eq!(arrivals.lock().expect("the slots").len(), 1);
        assert!(
            arrivals.lock().expect("the slots")[0].is_none(),
            "nothing can have arrived for a handler with nowhere to send"
        );
    }
}
