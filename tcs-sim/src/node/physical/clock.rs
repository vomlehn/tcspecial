//! The simulation's clock: how a moment of the wall clock maps to a moment
//! of simulated time.
//!
//! The world is advanced to whatever moment a simulated payload asks about, and
//! a payload asks at the wall clock's time, because the vehicle at the other
//! end of its link runs on the wall clock. So simulated time tracks it: a
//! second of simulated time for a second of the wall clock.
//!
//! From when the run begins, which is when a simulated payload first asks the
//! world anything, and not from when the world was made. The simulated payloads
//! start stopped, and are started from tcs-sim's window, so a world counting
//! from its making would have run on with nobody joined to it: a lander would
//! be on the ground before its radar was switched on.
//!
//! The rate is a factor on that, so that a world can run faster than the
//! wall clock -- an hour's descent in a minute -- where nothing at the other
//! end needs it to keep pace. A run with tcspecial keeps it at one: the
//! vehicle's sample intervals and timeouts are the wall clock's, and a world
//! ten times as fast would have its zone cool ten times faster than the
//! vehicle's thermostat was written for.
//!
//! Simulated time is seconds, as a number, from the world's start. A number
//! rather than an `Instant` because an advance between two events is
//! arithmetic on it, and because a world running at another rate has
//! moments that are no moment of the wall clock's at all.
//!
//! It can be paused, from tcs-sim's window, and resumed: simulated time stands
//! still while it is paused, and goes on from there, the time spent paused
//! left out of it.

use tcsos::sync::{Arc, Mutex, OnceLock};
use tcsos::time::{Duration, Instant};

/// How soon a thread waiting for a simulated moment is told to look again
/// while the clock is paused: the moment is no moment of the wall clock's
/// until the clock goes on.
const PAUSED_LOOK_AGAIN: Duration = Duration::from_millis(100);

/// Whether a clock is paused, and how long it has been.
#[derive(Debug, Default)]
struct Pause {
    /// When it was paused, while it is.
    since: Option<Instant>,
    /// The wall clock's time spent paused before, which simulated time
    /// leaves out.
    held: Duration,
}

/// How the wall clock maps to simulated time.
#[derive(Debug, Clone)]
pub struct Clock {
    /// The wall clock's moment at simulated time nought, once there is one.
    start: OnceLock<Instant>,
    /// Simulated seconds for each second of the wall clock.
    rate: f64,
    /// Whether it is paused, and for how long it has been: shared by its
    /// copies, which are one clock.
    pause: Arc<Mutex<Pause>>,
}

impl Clock {
    /// Makes a clock started at a moment, at a rate: what a test that has
    /// to know when time began makes.
    ///
    /// # Arguments
    ///
    /// * `start` - The wall clock's moment at simulated time nought.
    /// * `rate`  - Simulated seconds for each second of the wall clock:
    ///             one keeps pace.
    ///
    /// # Returns
    ///
    /// The clock.
    ///
    /// # Panics
    ///
    /// As [`Clock::starting_when_asked`] does.
    #[cfg(test)]
    pub fn new(start: Instant, rate: f64) -> Clock {
        let clock = Clock::starting_when_asked(rate);
        let _ = clock.start.set(start);
        clock
    }

    /// Makes a clock that starts the first time it is asked what simulated
    /// time it is: when the run begins.
    ///
    /// # Arguments
    ///
    /// * `rate` - Simulated seconds for each second of the wall clock.
    ///
    /// # Returns
    ///
    /// The clock, not yet started.
    ///
    /// # Panics
    ///
    /// If the rate is not more than nought: a clock that stood still or ran
    /// backwards would have a world never reach its next event.
    pub fn starting_when_asked(rate: f64) -> Clock {
        assert!(rate > 0.0 && rate.is_finite(), "a clock's rate is {rate}");
        Clock {
            start: OnceLock::new(),
            rate,
            pause: Arc::default(),
        }
    }

    /// The wall clock's moment at simulated time nought: `now` for a clock
    /// asked for the first time.
    ///
    /// # Arguments
    ///
    /// * `now` - The moment it is asked at.
    ///
    /// # Returns
    ///
    /// Its start.
    fn start(&self, now: Instant) -> Instant {
        *self.start.get_or_init(|| now)
    }

    /// A clock keeping pace with the wall clock, starting at `start`: what
    /// a test that has to know when time began makes.
    #[cfg(test)]
    pub fn keeping_pace(start: Instant) -> Clock {
        Clock::new(start, 1.0)
    }

    /// Says what simulated time a moment of the wall clock is, if the clock
    /// has started, without starting it.
    ///
    /// What a window watching the world asks: a window looking is not a
    /// payload asking, and a world started by being looked at would run on
    /// before anything was joined to it.
    ///
    /// # Arguments
    ///
    /// * `now` - The moment.
    ///
    /// # Returns
    ///
    /// Simulated seconds since the start, or `None` before it.
    pub fn started_at(&self, now: Instant) -> Option<f64> {
        self.start
            .get()
            .map(|start| self.running(*start, now).as_secs_f64() * self.rate)
    }

    /// How long the clock has run, from its start to a moment of the wall
    /// clock, leaving out the time it has been paused.
    ///
    /// # Arguments
    ///
    /// * `start` - Its start.
    /// * `now`   - The moment.
    ///
    /// # Returns
    ///
    /// The time it has run, on the wall clock.
    fn running(&self, start: Instant, now: Instant) -> Duration {
        let pause = self.pause.lock().unwrap();
        let until = pause.since.map_or(now, |since| since.min(now));
        until
            .saturating_duration_since(start)
            .saturating_sub(pause.held)
    }

    /// Pauses it: simulated time stands still from `now` until it is
    /// resumed. Pausing a paused clock does nothing.
    ///
    /// # Arguments
    ///
    /// * `now` - The moment it is paused at.
    pub fn pause(&self, now: Instant) {
        let mut pause = self.pause.lock().unwrap();
        if pause.since.is_none() {
            pause.since = Some(now);
        }
    }

    /// Resumes it: simulated time goes on from where it stood. Resuming a
    /// clock that is not paused does nothing.
    ///
    /// # Arguments
    ///
    /// * `now` - The moment it is resumed at.
    pub fn resume(&self, now: Instant) {
        let mut pause = self.pause.lock().unwrap();
        if let Some(since) = pause.since.take() {
            pause.held += now.saturating_duration_since(since);
        }
    }

    /// Says whether it is paused.
    ///
    /// # Returns
    ///
    /// True between a pause and the resume after it.
    pub fn paused(&self) -> bool {
        self.pause.lock().unwrap().since.is_some()
    }

    /// Says what simulated time a moment of the wall clock is.
    ///
    /// # Arguments
    ///
    /// * `now` - The moment.
    ///
    /// # Returns
    ///
    /// Simulated seconds since the start; nought for a moment before it.
    pub fn at(&self, now: Instant) -> f64 {
        self.running(self.start(now), now).as_secs_f64() * self.rate
    }

    /// Says what moment of the wall clock a simulated time is: when a thread
    /// waiting for something at that simulated time wakes.
    ///
    /// # Arguments
    ///
    /// * `seconds` - Simulated seconds since the start.
    ///
    /// # Returns
    ///
    /// The moment; the start for a time before it, and a day after now for
    /// one beyond anything a wait could mean, which is no time at all. While
    /// the clock is paused, a moment a little after now, for the waiting
    /// thread to look again.
    pub fn instant_of(&self, seconds: f64) -> Instant {
        let wall = (seconds / self.rate).max(0.0);
        let now = Instant::now();
        let day = now + Duration::from_secs(86_400);
        let held = {
            let pause = self.pause.lock().unwrap();
            if pause.since.is_some() {
                return now + PAUSED_LOOK_AGAIN;
            }
            pause.held
        };
        match Duration::try_from_secs_f64(wall) {
            Ok(after) => self
                .start(now)
                .checked_add(after + held)
                .map_or(day, |at| at.min(day)),
            Err(_) => day,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clock keeping pace counts wall seconds, one at ten times counts
    /// ten for each, a moment before the start is the start, and a
    /// simulated time maps back to the moment it came from.
    #[test]
    fn a_clock_maps_the_wall_clock_at_its_rate() {
        let start = Instant::now();
        let later = start + Duration::from_millis(2500);
        assert!((Clock::keeping_pace(start).at(later) - 2.5).abs() < 1e-9);
        let fast = Clock::new(start, 10.0);
        assert!((fast.at(later) - 25.0).abs() < 1e-9);
        assert_eq!(fast.at(start - Duration::from_secs(1)), 0.0);
        let back = fast.instant_of(25.0);
        assert!(back.duration_since(later) < Duration::from_micros(1));
        assert!(fast.instant_of(f64::INFINITY) > Instant::now());
    }

    /// A clock started when asked counts from its first question, however
    /// long after its making that is.
    #[test]
    fn a_clock_started_when_asked_counts_from_the_question() {
        let clock = Clock::starting_when_asked(1.0);
        let asked = Instant::now() + Duration::from_secs(30);
        assert_eq!(clock.at(asked), 0.0);
        let later = asked + Duration::from_millis(1500);
        assert!((clock.at(later) - 1.5).abs() < 1e-9);
    }

    /// A clock that does not run is refused as it is made.
    #[test]
    #[should_panic(expected = "rate")]
    fn a_clock_that_does_not_run_is_refused() {
        Clock::new(Instant::now(), 0.0);
    }

    /// Paused, simulated time stands still; resumed, it goes on from where
    /// it stood, the pause left out.
    #[test]
    fn a_paused_clock_stands_still_and_goes_on() {
        let start = Instant::now();
        let clock = Clock::keeping_pace(start);
        let at = |ms| start + Duration::from_millis(ms);
        assert!(!clock.paused());
        clock.pause(at(1000));
        assert!(clock.paused());
        assert_eq!(clock.at(at(1000)), 1.0);
        assert_eq!(clock.at(at(4000)), 1.0);
        assert_eq!(clock.started_at(at(4000)), Some(1.0));
        clock.resume(at(4000));
        assert!(!clock.paused());
        assert_eq!(clock.at(at(4500)), 1.5);
        // A copy is the same clock.
        let copy = clock.clone();
        copy.pause(at(5000));
        assert!(clock.paused());
        copy.resume(at(5000));
        assert_eq!(clock.at(at(6000)), 3.0);
    }
}
