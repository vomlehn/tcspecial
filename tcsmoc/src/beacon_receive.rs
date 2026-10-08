/*
 * Receive beacon messages
 */

use std::net::UdpSocket;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};
use slint::{Color, Weak};

use crate::MainWindow;
use slint::SharedString;
use tcslibgs::{TcsResult, Timestamp, NO_TRANSFER_TIME};

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

/// What a pass of the receive loop found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Arrived {
    /// A beacon, now.
    Beacon,
    /// Nothing, for as long as the pass waited.
    Nothing,
    /// The socket failed, which says nothing about the spacecraft.
    Broken,
}

/// What the window should show after a pass, and when the last beacon was.
///
/// Separate from the loop so that it can be exercised without a socket. The
/// time it returns is the point: a pass that found nothing still has the time
/// of the beacon before it to show, and a pass whose socket broke has it too
/// -- the spacecraft has said nothing about having stopped, and blanking the
/// line would say it had.
pub(crate) fn after_a_pass(
    states: &IndicatorStates,
    last: &CondPair<Option<SystemTime>>,
    arrived: Arrived,
    now: SystemTime,
) -> (Color, Option<SystemTime>) {
    match arrived {
        Arrived::Beacon => {
            let mut guard = last.lock.lock().unwrap();
            *guard = Some(now);
            last.cvar.notify_all();
            let at = *guard;
            drop(guard);
            (states.delay_and_color(&at).1, at)
        }
        Arrived::Nothing => {
            let at = *last.lock.lock().unwrap();
            (states.delay_and_color(&at).1, at)
        }
        Arrived::Broken => (states.unset_color(), *last.lock.lock().unwrap()),
    }
}

/// When the last beacon arrived, as the window shows it.
///
/// The time it arrived here, not a time the spacecraft put in it: the line is
/// labelled received, and what it is read for is whether beacons are still
/// coming -- a spacecraft whose clock had stopped would otherwise look as
/// though its beacons had. Shown the way every other time in the window is,
/// by the one function that shows them.
fn beacon_last_received(at: Option<SystemTime>) -> String {
    match at {
        Some(at) => Timestamp::at(at).time_of_day(),
        None => NO_TRANSFER_TIME.to_string(),
    }
}

/// Put the beacon's state in the window: the indicator's colour, and when the
/// last beacon arrived.
///
/// Both at once, from the one reading of when the last beacon was: a colour
/// that said beacons were arriving beside a time that said none had would be
/// two answers to one question.
pub(crate) fn show_beacon(ui: &MainWindow, at: Option<SystemTime>, color: Color) {
    ui.set_indicator_color(color);
    ui.set_beacon_last_recv(SharedString::from(beacon_last_received(at)));
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
            // How long to wait: until the colour would change by itself, so
            // that a blinking indicator blinks whether or not anything
            // arrives.
            let (timeout, _) = self
                .indicator_states
                .delay_and_color(&self.last_beacon.lock.lock().unwrap());
            if DEBUG_BEACON {
                eprintln!("waiting {:?}", timeout);
            }
            socket.set_read_timeout(timeout)?;

            let arrived = match socket.recv_from(&mut buf) {
                Ok((_size, _addr)) => Arrived::Beacon,
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    Arrived::Nothing
                }
                Err(_) => Arrived::Broken,
            };

            let (color, at) = after_a_pass(
                &self.indicator_states,
                &self.last_beacon,
                arrived,
                SystemTime::now(),
            );
            if DEBUG_BEACON {
                eprintln!("{:?}: colour {:?}, last beacon {:?}", arrived, color, at);
            }

            let ui_weak = self.ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    show_beacon(&ui, at, color);
                }
            });
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

pub(crate) struct CondPair<T> {
    lock: Mutex<T>,
    cvar: Condvar,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_beacon_yet() -> CondPair<Option<SystemTime>> {
        CondPair {
            lock: Mutex::new(None),
            cvar: Condvar::new(),
        }
    }

    /// A green indicator and the colours either side of it, so a test can
    /// tell a fresh beacon from a stale one by the colour alone.
    fn states() -> IndicatorStates {
        IndicatorStates::new(
            Color::from_rgb_u8(0xC0, 0xC0, 0xC0),
            vec![
                IndicatorState::Steady(Duration::from_millis(4000), Color::from_rgb_u8(0, 255, 0)),
                IndicatorState::Steady(Duration::MAX, Color::from_rgb_u8(255, 0, 0)),
            ],
        )
    }

    /// Every pass of the loop says when the last beacon arrived, not only the
    /// pass that received one.
    ///
    /// This is what the window was missing: the time of the last beacon has
    /// to survive the passes that find nothing, or the line would be filled
    /// in for an instant and emptied by the next timeout. A pass whose socket
    /// broke keeps it too -- the spacecraft has said nothing about having
    /// stopped, and blanking the line would say that it had.
    ///
    /// The colour is asserted only where it does not depend on how long ago
    /// the beacon was: `delay_and_color` ages it against the system clock,
    /// which a test cannot wind on. What the colour does with age is the
    /// indicator's own business and is tested where that lives.
    #[test]
    fn every_pass_says_when_the_last_beacon_arrived() {
        let states = states();
        let last = no_beacon_yet();

        // Nothing has arrived yet, so there is nothing to show and the
        // indicator is at the colour of never having heard anything.
        let (color, at) = after_a_pass(&states, &last, Arrived::Nothing, SystemTime::now());
        assert_eq!(at, None);
        assert_eq!(color, states.unset_color());

        // A beacon: the time is the time of this pass.
        let arrival = SystemTime::now();
        let (fresh, at) = after_a_pass(&states, &last, Arrived::Beacon, arrival);
        assert_eq!(at, Some(arrival), "a pass that received a beacon must say when");
        assert_ne!(
            fresh,
            states.unset_color(),
            "a beacon that has just arrived is not nothing heard from"
        );

        // A later pass that finds nothing keeps that time: the beacon did
        // arrive when it arrived, whatever the passes after it find.
        let (_, at) = after_a_pass(&states, &last, Arrived::Nothing, SystemTime::now());
        assert_eq!(at, Some(arrival), "the time of the last beacon was lost");

        // And a broken socket says nothing about the spacecraft: the time
        // stands, and the indicator goes to the colour of not knowing.
        let (unknown, at) = after_a_pass(&states, &last, Arrived::Broken, SystemTime::now());
        assert_eq!(at, Some(arrival));
        assert_eq!(unknown, states.unset_color());
    }

    /// A beacon that has arrived is shown by the time it arrived, and one
    /// that has not is said not to have.
    ///
    /// The line was never set at all: the window declared it, Rust set the
    /// indicator's colour beside it and nothing else, so it said no beacon
    /// had ever arrived for as long as the MOC ran. That is worse than saying
    /// nothing, because the colour next to it was green.
    #[test]
    fn a_beacon_is_shown_by_when_it_arrived() {
        assert_eq!(beacon_last_received(None), NO_TRANSFER_TIME);

        // The time of day it arrived, as every other time in the window is
        // shown: one function formats them all.
        let at = std::time::UNIX_EPOCH + Duration::from_secs(3661);
        assert_eq!(beacon_last_received(Some(at)), "01:01:01");

        // A clock reading before the epoch cannot be formatted and must not
        // panic: a window that fell over because the system clock was being
        // set would be a worse fault than a wrong time.
        let before = std::time::UNIX_EPOCH - Duration::from_secs(1);
        assert_eq!(beacon_last_received(Some(before)), "00:00:00");
    }
}
