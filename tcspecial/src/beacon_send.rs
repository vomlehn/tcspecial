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
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use tcslibgs::{BeaconTelemetry, TcsResult, Telemetry};

#[derive(Clone)]
pub struct BeaconSend {
    pair:       ArcCondPair<SystemTime>,
    interval:   Arc<Mutex<Duration>>,
    dest_addr:  std::net::SocketAddr
}

impl BeaconSend {
    pub fn new(interval: Duration, dest_addr: std::net::SocketAddr) -> Option<BeaconSend> {
        if interval == Duration::from_secs(0) {
            return None;
        }

        let expiration_time = SystemTime::now() + interval;
        let pair = Arc::new(CondPair {
            lock: Mutex::new(expiration_time),
            cvar: Condvar::new(),
        });

        let b = BeaconSend {
            pair,
            interval: Arc::new(Mutex::new(interval)),
            dest_addr,
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
        // Bind to a local address
//        let socket = UdpSocket::bind(BEACON_NETADDR)?; // 0 = let OS pick a port
        let socket = UdpSocket::bind("0.0.0.0:0"); // 0 = let OS pick a port
let socket = socket?;

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
        let beacon = Telemetry::Beacon(BeaconTelemetry::new());
        let data = serde_json::to_vec(&beacon)?;
        let status = socket.send_to(&data, dest_addr);
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
