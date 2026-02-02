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

const DEBUG_BEACON: bool = false;

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
        if last > now {
            return (None, self.unset);
        }

        // Time since the last time we got a beacon message. We want to
        // find the first indicator state containing this time
        let elapsed = now.duration_since(last).unwrap();
        let mut indicator_start = Duration::ZERO;
        
        for indicator_state in &self.indicator_states {
            // State covered by this indicator state
            let duration = match indicator_state {
                IndicatorState::Steady(duration, _) => *duration,
                IndicatorState::Blinking(duration, _, _, _, _) => *duration,
            };

            // Determine the time relative to the arrival of the last beacon
            // message at which this indicator state ends
            let indicator_end = indicator_start.saturating_add(duration);

            // If we are not within the duration of this indicator state,
            // update the start and try again
            if elapsed >= indicator_end {
                indicator_start = indicator_end;
                continue;
            }

            // Time into this indicator state
            let time_into_state = elapsed - indicator_start;

            // Okay, we're within the indicator state
            match indicator_state {
                IndicatorState::Steady(_, color) => {
                    // Time until end of this steady state
                    let time_remaining = indicator_end.saturating_sub(elapsed);
                    return (Some(time_remaining), *color);
                },
                IndicatorState::Blinking(_, time_on, time_off, color_on, color_off) => {
                    // Compute which blink we're in, and the offset within the blink
                    let blink_period = *time_on + *time_off;
                    let blink_period_ns = blink_period.as_nanos();
                    let time_into_state_ns = time_into_state.as_nanos();

                    // Offset within the current blink cycle
                    let offset_in_blink_ns = time_into_state_ns % blink_period_ns;
                    let time_on_ns = time_on.as_nanos();

                    if offset_in_blink_ns < time_on_ns {
                        // We're in the "on" part of the blink
                        let time_to_off_ns = time_on_ns - offset_in_blink_ns;
                        let time_to_off = Duration::from_nanos(time_to_off_ns as u64);
                        let time_remaining_in_state = indicator_end.saturating_sub(elapsed);
                        let timeout = time_to_off.min(time_remaining_in_state);
                        return (Some(timeout), *color_on);
                    } else {
                        // We're in the "off" part of the blink
                        let time_to_on_ns = blink_period_ns - offset_in_blink_ns;
                        let time_to_on = Duration::from_nanos(time_to_on_ns as u64);
                        let time_remaining_in_state = indicator_end.saturating_sub(elapsed);
                        let timeout = time_to_on.min(time_remaining_in_state);
                        return (Some(timeout), *color_off);
                    }
                }
            }
        }
        (None, self.unset)
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

impl BeaconReceive {
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
                panic!("Beacon receive error: {}", e);
            }
        });

        Some(b)
    }

    /*
     * Receive beacon messages in a loop
     */
    fn receive_beacon(&self) -> TcsResult<()> {
        // Bind to a local address to receive messages
        let socket = UdpSocket::bind(self.src_addr)?;

        let mut buf = [0u8; 65535];

        loop {
            // Get current color and timeout duration
            let last_beacon_guard = self.last_beacon.lock.lock().unwrap();
            let last_beacon_value = *last_beacon_guard;
            drop(last_beacon_guard);

            let (timeout, color) = self.indicator_states.delay_and_color(&last_beacon_value);
if DEBUG_BEACON {
eprintln!("First: timeout {:?} color {:?}", timeout, color);
}

            // Set socket timeout
            socket.set_read_timeout(timeout)?;

            // Receive beacon data from socket (or timeout)
            let status = socket.recv_from(&mut buf);
            
            let new_color = match status {
                Ok((size, addr)) => {
//eprintln!("beacon received {} bytes from {}", size, addr);
                    // Update last beacon time
                    let mut last_beacon_guard = self.last_beacon.lock.lock().unwrap();
                    *last_beacon_guard = Some(SystemTime::now());
                    self.last_beacon.cvar.notify_all();
                    let last_beacon_value = *last_beacon_guard;
                    drop(last_beacon_guard);

                    // Recalculate color after receiving
                    let (_, color) = self.indicator_states.delay_and_color(&last_beacon_value);
if DEBUG_BEACON {
eprintln!("Msg rcvd: timeout {:?} color {:?}", timeout, color);
}
                    Some(color)
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    // Timeout - update the color
//eprintln!("beacon timeout");
                    let last_beacon_guard = self.last_beacon.lock.lock().unwrap();
                    let last_beacon_value = *last_beacon_guard;
                    drop(last_beacon_guard);
                    let (_, color) = self.indicator_states.delay_and_color(&last_beacon_value);
if DEBUG_BEACON {
eprintln!("Timedout: timeout {:?} color {:?}", timeout, color);
}
                    Some(color)
                }
                Err(e) => {
                    // I/O error
//panic!("receive_beacon: error receiving: {}", e);
                    Some(self.indicator_states.unset_color())
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
use std::net::UdpSocket;
use std::time::{Duration, SystemTime};
use slint::{Color, Weak, ComponentHandle};
use crate::config::constants::BEACON_INDICATOR; // Adjust path as needed
*/

/* FIXME: compare with version above. Claude said:
 * Why this works
 * The "Blink" Logic: By using set_read_timeout with the duration returned by
 * color_and_delay, the loop "wakes up" exactly when it's time to toggle the
 * light (e.g., every 500ms for a blink), even if no network data has arrived.
 * 
 * The Weak Pointer: We use Weak<MainWindow> so that the background thread
 * doesn't prevent the UI from closing. If the user closes the window,
 * ui_handle.upgrade() will return None, and the thread can shut down
 * gracefully.
 * 
 * SystemTime Error Handling: Inside color_and_delay (the code I provided in
 * the previous step), we used unwrap_or(Duration::ZERO) for the time
 * subtraction. This ensures that if the system clock drifts slightly, your
 * app doesn't crash.
 *
    pub fn receive_beacon(&self, ui_handle: Weak<MainWindow>) -> TcsResult<()> {
        let socket = UdpSocket::bind("0.0.0.0:0")?; // Bind to any available port
        // Note: You'll likely want to connect or join a multicast group here

        loop {
            // 1. Get the current status from our configuration logic
            let last_beacon = *self.last_beacon.lock.lock().unwrap();
            let (current_color, next_event_delay) = BEACON_INDICATOR.color_and_delay(last_beacon);

            // 2. Update the UI color
            let ui_clone = ui_handle.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_clone.upgrade() {
                    // Assuming your .slint file has a property called 'beacon_color'
                    ui.set_beacon_color(current_color);
                }
            }).unwrap();

            // 3. Set the socket timeout based on the next state change
            // If next_event_delay is None, we wait indefinitely (or a default)
            socket.set_read_timeout(next_event_delay.or(Some(Duration::from_secs(1))))?;

            // 4. Try to receive data
            let mut buf = [0u8; 1024];
            match socket.recv_from(&mut buf) {
                Ok((_amt, _src)) => {
                    // We got a beacon! Update the timestamp
                    let mut last_beacon_lock = self.last_beacon.lock.lock().unwrap();
                    *last_beacon_lock = Some(SystemTime::now());
                    self.last_beacon.cvar.notify_all();
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                    // No data received, but that's okay—the loop will restart,
                    // re-calculate the color (for blinking), and wait again.
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
*/
}

type ArcCondPair<T> = Arc<CondPair<T>>;

struct CondPair<T> {
    lock: Mutex<T>,
    cvar: Condvar,
}
