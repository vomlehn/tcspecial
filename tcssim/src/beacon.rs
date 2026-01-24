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
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

#[derive(Clone)]
pub struct Beacon {
    pair: ArcCondPair<SystemTime>,
    interval: Arc<Mutex<Duration>>,
}

impl Beacon {
    pub fn new(interval: Duration) -> Option<Beacon> {
        if interval == Duration::from_secs(0) {
            return None;
        }

        let expiration_time = SystemTime::now() + interval;
        let pair = Arc::new(CondPair {
            lock: Mutex::new(expiration_time),
            cvar: Condvar::new(),
        });

        let b = Beacon {
            pair,
            interval: Arc::new(Mutex::new(interval)),
        };

        let b_clone = b.clone();
        thread::spawn(move || {
            b_clone.beacon();
        });

        Some(b)
    }

    fn beacon(&self) {
        self.send_beacon();

        loop {
            let mut expiration = self.pair.lock.lock().unwrap();

            // Wait until expiration time or until notified
            while *expiration > SystemTime::now() {
                eprintln!("Worker: waiting for signal...");
                let timeout = expiration
                    .duration_since(SystemTime::now())
                    .unwrap_or(Duration::from_millis(1));

                let (guard, result) = self.pair.cvar.wait_timeout(expiration, timeout).unwrap();
                expiration = guard;

                if result.timed_out() {
                    break;
                }
            }

            eprintln!("Signal received or timeout");

            // Send the beacon
            self.send_beacon();

            // Calculate next expiration time
            let interval = *self.interval.lock().unwrap();
            let now = SystemTime::now();
            *expiration = now + interval;
        }
    }

    /// Send a single beacon message
    fn send_beacon(&self) {
        eprintln!("Sending beacon at {:?}", SystemTime::now());
        /*
        let msg = BeaconTelemetry::new();
        send(msg);
        */
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
