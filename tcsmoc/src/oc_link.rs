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

use tcslibgs::{DHConfig, DHSample, NetworkConfig, Timestamp};

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
/// A handler with no OC address gets neither: it cannot be started, so there
/// is nothing to listen for.
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

        let socket = match UdpSocket::bind("0.0.0.0:0") {
            Ok(socket) => socket,
            Err(_) => continue,
        };
        if socket.set_read_timeout(Some(HELLO_INTERVAL)).is_err() {
            continue;
        }

        let slot = arrivals.clone();
        thread::spawn(move || serve(socket, address, slot, row));
    }

    arrivals
}

/// Where a handler's OC endpoint is, as an address to send to.
fn oc_address(oc: &NetworkConfig) -> Option<SocketAddr> {
    format!("{}:{}", oc.address, oc.port).parse().ok()
}

/// Say hello, then keep what arrives.
fn serve(socket: UdpSocket, handler: SocketAddr, arrivals: Arrivals, row: usize) {
    let mut buffer = vec![0u8; 65535];

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
                Ok((size, _from)) if size > 0 => {
                    let mut data = DHSample::new();
                    data.record(&buffer[..size]);
                    if let Ok(mut slots) = arrivals.lock() {
                        if let Some(slot) = slots.get_mut(row) {
                            *slot = Some(Arrival {
                                at: Timestamp::now(),
                                data,
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
    use tcslibgs::{DHId, DHMode, DHName, EndpointConfig, NetworkProtocol};

    /// A handler as the payload file describes one, with an OC address of its
    /// own.
    fn handler(oc: Option<SocketAddr>) -> DHConfig {
        DHConfig {
            dh_id: DHId(0),
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
