/*
 * Receive beacon messages
 */

use std::net::UdpSocket;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};
use slint::{Color, Weak};

use crate::MainWindow;
use tcslibgs::TcsResult;

// Configuration constants (you may want to move these to a config module)
const BEACON_GREEN: Duration = Duration::from_secs(5);
const BEACON_YELLOW: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct BeaconReceive {
    last_beacon: ArcCondPair<Option<SystemTime>>,
    src_addr: std::net::SocketAddr,
    ui_weak: Weak<MainWindow>,
}

impl BeaconReceive {
    pub fn new(ui_weak: Weak<MainWindow>, src_addr: std::net::SocketAddr) -> Option<BeaconReceive> {
        let last_beacon = Arc::new(CondPair {
            lock: Mutex::new(None),
            cvar: Condvar::new(),
        });

        let b = BeaconReceive {
            last_beacon,
            src_addr,
            ui_weak,
        };

        let b_clone = b.clone();
        thread::spawn(move || {
            if let Err(e) = b_clone.receive_beacon() {
                eprintln!("Beacon receive error: {}", e);
            }
        });

        eprintln!("BeaconReceive::new: exit");
        Some(b)
    }

    /*
     * Called when a beacon message is received
     */
    fn receive_beacon(&self) -> TcsResult<()> {
        eprintln!("BeaconReceive::receive_beacon: entered");
        // Bind to a local address to receive messages
        let socket = UdpSocket::bind(self.src_addr)?;
        // Set a timeout so we can periodically update the color even without new messages
        socket.set_read_timeout(Some(Duration::from_secs(1)))?;
        eprintln!("receive_beacon: socket {:?}", socket);

        let mut buf = [0u8; 65535];

        loop {
            // Receive beacon data from socket (or timeout)
            match socket.recv_from(&mut buf) {
                Ok((size, addr)) => {
                    eprintln!("receive_beacon: received {} bytes from {}", size, addr);

                    // Update last beacon time
                    let mut last_beacon = self.last_beacon.lock.lock().unwrap();
                    *last_beacon = Some(SystemTime::now());
                    self.last_beacon.cvar.notify_all();
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Timeout - just update the color
                }
                Err(e) => {
                    eprintln!("receive_beacon: error receiving: {}", e);
                }
            }

            // Update the UI color
            let color = self.beacon_color();
            let ui_weak = self.ui_weak.clone();

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_indicator_color(color);
                }
            });

            // Log next event info
            match self.beacon_next_event() {
                None => { eprintln!("Stay red (or no beacon yet)"); }
                Some(next_event) => {
                    if let Ok(duration) = next_event.duration_since(SystemTime::now()) {
                        eprintln!("Will change color in {:?}", duration);
                    }
                }
            }
        }
    }

    /*
     * Returns the color of the beacon indicator
     */
    pub fn beacon_color(&self) -> Color {
        let last_beacon = self.last_beacon.lock.lock().unwrap();
        let last_beacon_time = *last_beacon;

        if last_beacon_time.is_none() {
            return Color::from_argb_u8(255, 128, 128, 128); // gray - no beacon yet
        }

        let last_time = last_beacon_time.unwrap();
        let now = SystemTime::now();
        let elapsed = now.duration_since(last_time).unwrap_or(Duration::MAX);

        if elapsed < BEACON_GREEN {
            Color::from_argb_u8(255, 0, 255, 0) // green
        } else if elapsed < BEACON_YELLOW {
            Color::from_argb_u8(255, 255, 255, 0) // yellow
        } else {
            Color::from_argb_u8(255, 255, 0, 0) // red
        }
    }

    /*
     * Returns the time that the next change of the beacon should take
     * place. If the beacon has never been seen, this is None. Otherwise,
     * we return the time.
     */
    pub fn beacon_next_event(&self) -> Option<SystemTime> {
        let last_beacon = self.last_beacon.lock.lock().unwrap();
        let last_beacon_time = (*last_beacon)?;

        let now = SystemTime::now();
        let elapsed = now.duration_since(last_beacon_time).unwrap_or(Duration::MAX);

        if elapsed < BEACON_GREEN {
            Some(last_beacon_time + BEACON_GREEN)
        } else if elapsed < BEACON_YELLOW {
            Some(last_beacon_time + BEACON_YELLOW)
        } else {
            None // Already red, no more transitions until next message rcvd
        }
    }

    /*
     * Wait for the beacon color to change
     */
    #[allow(dead_code)]
    pub fn wait_for_color_change(&self, timeout: Duration) -> bool {
        let last_beacon = self.last_beacon.lock.lock().unwrap();
        let (_guard, result) = self.last_beacon.cvar.wait_timeout(last_beacon, timeout).unwrap();
        !result.timed_out()
    }
}

type ArcCondPair<T> = Arc<CondPair<T>>;

struct CondPair<T> {
    lock: Mutex<T>,
    cvar: Condvar,
}
