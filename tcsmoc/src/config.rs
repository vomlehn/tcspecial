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
    pub fn beacon_schedule() -> Schedule {
        Schedule::standard()
    }
}
