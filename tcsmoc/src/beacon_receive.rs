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

    pub fn unset_color(&self) -> Color {
        self.unset
    }

    /*
     * Compute the current color and the delay to the next event that will
     * change the color (if a beacon message is not received). It
     * returns a 2-tuple with a color and an Option<Duration>. The
     * Option<Duration> is None if no timeout should be set, i.e. if we
     * wait only on receipt of a beacon message. If it's Some(), the
     * wait is on receipt of the message or the given Duration value.
     *
     * ------- ------------- ---------------------------
     * ^      ^      ^      ^      ^      ^      ^      ^
     * |      |      |      |      |      |      |      |
     * |      blink  blink  blink  blink  blink  blink  |
     * |      0 on   0 off  1 on   1 off  1 on   1 off  |
     * |      |             |                           |
     * msg    indicator     indicator                   |
     * recvd  0 start       1 start                     |
     * |      |             |                           |
     * |      |<--duration->|<---------duration-------->|
     * |      |             |                           |
     * |      |<--blink---->|<--blink---->|<--blink---->|
     * |      |   duration  |   duration  |   duration  |
     * |      |             |             |             |
     * elapsed
     *        indicator_start
     *                      indicator_end
     *
     *                      indicator_start
     *                                                 indicator_end
     *                                    
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
eprintln!("\nlast {:?} > now {:?}", last, now);
        if last > now {
eprintln!("No beacon seen");
            return (None, self.unset);
        }

        // Time since the last time we got a beacon message. We want to
        // find the first indicator state containing this time
        let elapsed = now.duration_since(last).unwrap();
        let mut indicator_start = Duration::ZERO;
let mut i = 0;
        
        for indicator_state in &self.indicator_states {
            if indicator_start < elapsed {
            }

            // State covered by this indicator state
            let duration = match indicator_state {
                IndicatorState::Steady(duration, _) => duration,
                IndicatorState::Blinking(duration, _, _, _, _) => duration,
            };

            // Determine the time relative to the arrival of the last beacon
            // message at which this indicator state ends
            let indicator_end = indicator_start + *duration;

            // If we are not within the duration of this indicator state,
            // update the start and try again
            if elapsed > indicator_end {
                indicator_start = indicator_end;
eprintln!("elapsed {:?} > indicator_end {:?}", elapsed, indicator_end);
                continue;
            }

            // Okay, we're within the indicator state
            match indicator_state {
                IndicatorState::Steady(_, color) => {
eprintln!("Return steady state({:?}, {:?}", indicator_end, color);
                    return (Some(indicator_end), *color);
                },
                IndicatorState::Blinking(_, time_on, time_off, color_on, color_off) => {
                    // Compute which blink we're in, and the offset to the end
                    // of the blink.
                    let blink_period = *time_on + *time_off;
                    let blink_period_ns = blink_period.as_nanos();
                    let indicator_end_ns = indicator_end.as_nanos();
                    let elapsed_ns = elapsed.as_nanos();

                    let n_blink_ns = indicator_end_ns - elapsed_ns / blink_period_ns;
                    let blink_period_start_ns = n_blink_ns * blink_period_ns;
                    let blink_period_start = Duration::new(
                        (blink_period_start_ns / 1_000_000_000).try_into().unwrap(),
                        (blink_period_start_ns % 1_000_000_000).try_into().unwrap());

                    let delta_in_blink = elapsed - blink_period_start;
eprintln!("n_blink {:?} blink_period_start {:?} delta_in_blink {:?}", n_blink_ns, blink_period_start, delta_in_blink);
                
                    if delta_in_blink < *time_on {
                        let blink_end_ns = (blink_period_start + *time_on).as_nanos();
                        let blink_end = Duration::new(
                            (blink_end_ns / 1_000_000_000).try_into().unwrap(),
                            (blink_end_ns % 1_000_000_000).try_into().unwrap());
eprintln!("In time on {:?}", *color_on);
                        return (Some(blink_end), *color_on);
                    } else {
                        let blink_end_ns = (blink_period_start + blink_period).as_nanos();
                        let blink_end = Duration::new(
                            (blink_end_ns / 1_000_000_000).try_into().unwrap(),
                            (blink_end_ns % 1_000_000_000).try_into().unwrap());
eprintln!("In time off {:?}", *color_off);
                        return (Some(blink_end), *color_off);
                    }
                }
            }
        }
eprintln!("Out of indicator states");
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
eprintln!("timeout {:?} color {:?} color", timeout, color);

            // Set socket timeout
            eprintln!("receive_beacon: timeout: {:?}", timeout);
            socket.set_read_timeout(timeout)?;

            // Receive beacon data from socket (or timeout)
            let status = socket.recv_from(&mut buf);
eprintln!("=== receive_beacon: [{:?}] status {:?}", SystemTime::now(), status);
            let new_color = match status {
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    // Timeout - update the color
                    eprintln!("receive_beacon: timeout, updating color");
                    let (_, color) = self.indicator_states.delay_and_color(&*last_beacon);
                    Some(color)
                }
                Err(e) => {
                    // I/O error
                    eprintln!("receive_beacon: error receiving: {}", e);
                    Some(self.indicator_states.unset_color())
                }
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
eprintln!("color {:?} color", color);
                    Some(color)
                }
            };

eprintln!("Set new_color to {:?}", new_color);
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
