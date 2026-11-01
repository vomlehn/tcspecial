//! The simulation's clock: how a moment of the wall clock maps to a moment
//! of simulated time.
//!
//! The world is advanced to whatever moment a stand-in asks about, and a
//! stand-in asks at the wall clock's time, because the vehicle at the other
//! end of its link runs on the wall clock. So simulated time tracks it: a
//! second of simulated time for a second of the wall clock, from when the
//! world was made.
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

use std::time::Instant;

/// How the wall clock maps to simulated time.
#[derive(Debug, Clone, Copy)]
pub struct Clock {
    /// The wall clock's moment at simulated time nought.
    start: Instant,
    /// Simulated seconds for each second of the wall clock.
    rate: f64,
}

impl Clock {
    /// Makes a clock starting now, at a rate.
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
    /// If the rate is not more than nought: a clock that stood still or ran
    /// backwards would have a world never reach its next event.
    pub fn new(start: Instant, rate: f64) -> Clock {
        assert!(rate > 0.0 && rate.is_finite(), "a clock's rate is {rate}");
        Clock { start, rate }
    }

    /// A clock keeping pace with the wall clock, starting at `start`.
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
        now.saturating_duration_since(self.start).as_secs_f64() * self.rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// A clock keeping pace counts wall seconds, one at ten times counts
    /// ten for each, and a moment before the start is the start.
    #[test]
    fn a_clock_maps_the_wall_clock_at_its_rate() {
        let start = Instant::now();
        let later = start + Duration::from_millis(2500);
        assert!((Clock::keeping_pace(start).at(later) - 2.5).abs() < 1e-9);
        let fast = Clock::new(start, 10.0);
        assert!((fast.at(later) - 25.0).abs() < 1e-9);
        assert_eq!(fast.at(start - Duration::from_secs(1)), 0.0);
    }

    /// A clock that does not run is refused as it is made.
    #[test]
    #[should_panic(expected = "rate")]
    fn a_clock_that_does_not_run_is_refused() {
        Clock::new(Instant::now(), 0.0);
    }
}
