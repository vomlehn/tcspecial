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

use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// How the wall clock maps to simulated time.
#[derive(Debug, Clone)]
pub struct Clock {
    /// The wall clock's moment at simulated time nought, once there is one.
    start: OnceLock<Instant>,
    /// Simulated seconds for each second of the wall clock.
    rate: f64,
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
        now.saturating_duration_since(self.start(now)).as_secs_f64() * self.rate
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
    /// one beyond anything a wait could mean, which is no time at all.
    pub fn instant_of(&self, seconds: f64) -> Instant {
        let wall = (seconds / self.rate).max(0.0);
        let now = Instant::now();
        let day = now + Duration::from_secs(86_400);
        match Duration::try_from_secs_f64(wall) {
            Ok(after) => self
                .start(now)
                .checked_add(after)
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
}
