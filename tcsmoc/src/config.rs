//! Configuration loading for tcsmoc

/// Configuration constants
pub mod constants {
    use tcslib::beacon::Schedule;

    /// How the beacon's light ages, which is `tcslib`'s standard schedule.
    ///
    /// Not this window's own any more. The stages and their timings are the
    /// same for any ground station watching the same spacecraft -- see
    /// `tcslib::beacon::Schedule::standard` -- and a second station with a
    /// schedule of its own would be a second account of how late a beacon is.
    ///
    /// A panel's data indicator took its colours from `tcsui`'s palette the
    /// same way, and went with version 1: what it was judged against was a
    /// payload's `stale_interval`, which only a payload set ever stated.
    pub fn beacon_schedule() -> Schedule {
        Schedule::standard()
    }
}
