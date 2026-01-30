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

/*
 * Indicator states
 * Steady:      State that does not blink
 *              Duration    Duration of this state
 *              Color:      Color to display
 * Blinking:    Blinking state
 *              Duration    Duration of this state
 *              Duration    Duration of first color ("on")
 *              Duration    Duration of second color ("off")
 *              Color       "On" color
 *              Color       "Off" color
 */
#[derive(Copy, Clone)]
pub enum IndicatorState {
    Steady(Duration, Color),
    Blinking(Duration, Duration, Duration, Color, Color),    // Alternating colors
}

/*
 * Collection of indicators
 * unset        Color to use if the indicator has never received a message
 * indicators   Array of Indicator states
 */
#[derive(Clone)]
pub struct IndicatorStates {
    unset:              Color,
    indicator_states:   Vec<IndicatorState>,
}

impl IndicatorStates {
    pub fn new(unset: Color, indicator_states: Vec<IndicatorState>) -> Self {
        IndicatorStates {
            unset,
            indicator_states,
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
    pub fn delay_and_color(&self, last_beacon: &Option<SystemTime>) -> (Option<Duration>, Color) {
        // If we haven't seen any beacon messages at all, just return the unset
        // value and sleep until we get a message
        let last = match last_beacon {
            None => return (None, self.unset),
            Some(last) => *last,
        };

        // If the last time a beacon message was received is after the
        // current time, the system time has changed. The beacon indicator
        // needs to go back to unset.
        let now = SystemTime::now();
        if last > now {
            return (None, self.unset);
        }

        let elapsed = now.duration_since(last).unwrap();
        let mut cumulative = Duration::ZERO;

        for indicator_state in &self.indicator_states {
            let duration = match indicator_state {
                IndicatorState::Steady(duration, _) => duration,
                IndicatorState::Blinking(duration, _, _, _, _) => duration,
            };

            let indicator_state_end = cumulative + *duration;

            // Check if we are in this phase
            if elapsed < indicator_state_end {
                let time_into_state = elapsed - cumulative;
                return self.blink_info(&indicator_state, time_into_state);
            }

            cumulative = indicator_state_end;
        }

        // Past all phases - return last phase color with no timeout
        // FIXME: verify somewhere that the final duration is Duration::MAX
        (None, self.unset)
    }

    /*
     * Determine the current color and how long to the next non-message
     * reception change of color.
     * indicator:   Reference to an Indicator state
     * time_into_state: Time elapsed since start of this indicator state
     *
     * Returns: pair of time to next state change and color to set
     */
    fn blink_info(&self, indicator: &IndicatorState, time_into_state: Duration) -> (Option<Duration>, Color) {
        match indicator {
            IndicatorState::Steady(duration, color) => {
                let remaining = duration.saturating_sub(time_into_state);
                (Some(remaining), *color)
            }
            IndicatorState::Blinking(duration, time_on, time_off, color_on, color_off) => {
                let time_on_ns = time_on.as_nanos();
                let time_off_ns = time_off.as_nanos();
                let blink_period_ns = time_on_ns + time_off_ns;
                let time_into_state_ns = time_into_state.as_nanos();

                // Compute offset within the current blink cycle
                let time_offset_ns = time_into_state_ns % blink_period_ns;

                // Are we in the "on" part of the cycle?
                let on_cycle = time_offset_ns < time_on_ns;
                eprintln!("time_into_state {:?} time_offset {:?} on_cycle {:?}", time_into_state_ns, time_offset_ns, on_cycle);

                // Time remaining in this state (until next indicator state)
                let time_remaining_in_state = duration.saturating_sub(time_into_state);

                let result = if on_cycle {
                    // In "on" part. How much longer until we switch to "off"?
                    let time_to_off_ns = time_on_ns - time_offset_ns;
                    let time_to_off = Duration::from_nanos(time_to_off_ns as u64);
                    // Return the minimum of time to next blink change or time to next state
                    (Some(time_to_off.min(time_remaining_in_state)), *color_on)
                } else {
                    // In "off" part. How much longer until we switch to "on"?
                    let time_to_on_ns = blink_period_ns - time_offset_ns;
                    let time_to_on = Duration::from_nanos(time_to_on_ns as u64);
                    (Some(time_to_on.min(time_remaining_in_state)), *color_off)
                };
                eprintln!("time to next change {:?}, color {:?}", result.0, result.1);
                result
            }
        }
    }
}

/*
 * last_beacon  Time of last received beacon message
 * src_addr     Address from which to receive beacon messages
 * ui_weak      Slint window with beacon information
 * indicators   Indicator state configuration
 */
#[derive(Clone)]
pub struct BeaconReceive {
    last_beacon:        ArcCondPair<Option<SystemTime>>,
    src_addr:           std::net::SocketAddr,
    ui_weak:            Weak<MainWindow>,
    indicator_states:   IndicatorStates,
}

impl<'a> BeaconReceive {
    pub fn new(
        ui_weak:            Weak<MainWindow>,
        src_addr:           std::net::SocketAddr,
        indicator_states:   IndicatorStates,
    ) -> Option<BeaconReceive> {
        let last_beacon = Arc::new(CondPair {
            lock: Mutex::new(None),
            cvar: Condvar::new(),
        });

        let b = BeaconReceive {
            last_beacon,
            src_addr,
            ui_weak,
            indicator_states,
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
     * Receive beacon messages in a loop
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
            let (timeout, color) = self.indicator_states.delay_and_color(&*last_beacon);
            drop(last_beacon);

            // Set socket timeout
            eprintln!("receive_beacon: timeout: {:?}", timeout);
            socket.set_read_timeout(timeout)?;

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
                    let (_, color) = self.indicator_states.delay_and_color(&*last_beacon);
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
}

type ArcCondPair<T> = Arc<CondPair<T>>;

struct CondPair<T> {
    lock: Mutex<T>,
    cvar: Condvar,
}
