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

#[derive(Clone, Copy, PartialEq)]
pub enum Status {
    None,           // No status info
    Green,
    Yellow,
    Red,
}

/*
 * Define one phase of the lighted indicator
 * duration:    Duration of this phase
 * color_on:    Color when indicator is on
 * color_off:   If the indicator blinks, this is a Some(Color) and the color
 *              is used when the indicator is off. If it doesn't blink, this
 *              is None.
 */
#[derive(Clone)]
pub struct IndicatorPhase {
    pub duration: Duration,
    pub color_on: Color,
    pub color_off: Option<Color>,
}

impl IndicatorPhase {
    pub fn new(duration: Duration, color_on: Color, color_off: Option<Color>) -> IndicatorPhase {
        IndicatorPhase {
            duration,
            color_on,
            color_off,
        }
    }
}

/*
 * Define the behavior of an indicator as the time since a beacon message was
 * received.
 * blink_duration:  Length of a cycle of blinking, e.g. the off or on period
 * unset:           Color if we've never seen a beacon message for this indicator
 * phases:          List of all phases
 */
#[derive(Clone)]
pub struct IndicatorPhases {
    pub blink_duration: Duration,
    pub unset: Color,
    pub phases: Vec<IndicatorPhase>,
}

impl IndicatorPhases {
    pub fn new(blink_duration: Duration, unset: Color, phases: Vec<IndicatorPhase>) -> Self {
        IndicatorPhases {
            blink_duration,
            unset,
            phases,
        }
    }

    /*
     * Compute the current color and the delay to the next event that will
     * change the color (if a beacon message is not received). It
     * returns a 2-tuple with a color and an Option<Duration>. The
     * Option<Duration> is None if no timeout should be set, i.e. if we
     * wait only on receipt of a beacon message. If it's Some(), the
     * wait is on receipt of the message or the given Duration value.
     */
    pub fn color_and_delay(&self, last_beacon: &Option<SystemTime>) -> (Color, Option<Duration>) {
        // If we haven't seen any beacon messages, just return the unset
        // value and sleep until we get a message
        let last = match last_beacon {
            None => return (self.unset, None),
            Some(last) => *last,
        };

        // If the last time a beacon message was received is after the
        // current time, the system time has changed. The beacon indicator
        // needs to go back to unset.
        let now = SystemTime::now();
        if last > now {
            return (self.unset, None);
        }

        let elapsed = now.duration_since(last).unwrap();
        let mut cumulative = Duration::ZERO;

        for phase in &self.phases {
            let phase_end = cumulative + phase.duration;

            // Check if we are in this phase
            if elapsed < phase_end {
                let time_into_phase = elapsed - cumulative;
                return self.blink_info(phase, time_into_phase, phase_end - elapsed);
            }

            cumulative = phase_end;
        }

        // Past all phases - return last phase color with no timeout
        if let Some(last_phase) = self.phases.last() {
            (last_phase.color_on, None)
        } else {
            (self.unset, None)
        }
    }

    /*
     * Determine the current color and how long to the next non-message
     * reception change of color.
     */
    fn blink_info(&self, phase: &IndicatorPhase, time_into_phase: Duration, time_to_next_phase: Duration) -> (Color, Option<Duration>) {
        match phase.color_off {
            None => {
                // Not blinking - return on color and time until next phase
                (phase.color_on, Some(time_to_next_phase))
            }
            Some(color_off) => {
                // Blinking - determine if we're in on or off part of blink cycle
                let blink_offset = time_into_phase.as_millis() % self.blink_duration.as_millis();
                let half_blink = self.blink_duration.as_millis() / 2;

                if blink_offset < half_blink {
                    // In first half (on)
                    let time_to_off = Duration::from_millis((half_blink - blink_offset) as u64);
                    (phase.color_on, Some(time_to_off.min(time_to_next_phase)))
                } else {
                    // In second half (off)
                    let time_to_on = Duration::from_millis((self.blink_duration.as_millis() - blink_offset) as u64);
                    (color_off, Some(time_to_on.min(time_to_next_phase)))
                }
            }
        }
    }
}

/*
 * last_beacon  Time of last received beacon message
 * src_addr     Address from which to receive beacon messages
 * ui_weak      Slint window with beacon information
 * phases       Indicator phase configuration
 */
#[derive(Clone)]
pub struct BeaconReceive {
    last_beacon: ArcCondPair<Option<SystemTime>>,
    src_addr: std::net::SocketAddr,
    ui_weak: Weak<MainWindow>,
    phases: IndicatorPhases,
}

impl BeaconReceive {
    pub fn new(
        ui_weak: Weak<MainWindow>,
        src_addr: std::net::SocketAddr,
        phases: IndicatorPhases,
    ) -> Option<BeaconReceive> {
        let last_beacon = Arc::new(CondPair {
            lock: Mutex::new(None),
            cvar: Condvar::new(),
        });

        let b = BeaconReceive {
            last_beacon,
            src_addr,
            ui_weak,
            phases,
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
        eprintln!("receive_beacon: socket {:?}", socket);

        let mut buf = [0u8; 65535];

        loop {
            // Get current color and timeout duration
            let last_beacon = self.last_beacon.lock.lock().unwrap();
            let (color, timeout) = self.phases.color_and_delay(&*last_beacon);
            drop(last_beacon);

            // Set socket timeout
            socket.set_read_timeout(timeout)?;
            eprintln!("receive_beacon: timeout: {:?}", timeout);

            // Receive beacon data from socket (or timeout)
            let new_color = match socket.recv_from(&mut buf) {
                Ok((size, addr)) => {
                    eprintln!("receive_beacon: received {} bytes from {}", size, addr);
                    // Update last beacon time
                    let mut last_beacon = self.last_beacon.lock.lock().unwrap();
                    *last_beacon = Some(SystemTime::now());
                    self.last_beacon.cvar.notify_all();
                    drop(last_beacon);

                    // Recalculate color after receiving
                    let last_beacon = self.last_beacon.lock.lock().unwrap();
                    let (color, _) = self.phases.color_and_delay(&*last_beacon);
                    Some(color)
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    // Timeout - update the color
                    eprintln!("receive_beacon: timeout, updating color");
                    Some(color)
                }
                Err(e) => {
                    // I/O error
                    eprintln!("receive_beacon: error receiving: {}", e);
                    None
                }
            };

            // Set the indicator color
            if let Some(color) = new_color {
                let ui_weak = self.ui_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.set_indicator_color(color);
                    }
                });
            }
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
