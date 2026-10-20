//! Configuration loading for tcsmoc

/// Configuration constants
pub mod constants {
    use tcslib::beacon::Schedule;

    /// How the beacon's light ages, for beacons sent every `interval`:
    /// `tcslib`'s schedule for that interval.
    ///
    /// Not this window's own any more. The stages and their timings are the
    /// same for any ground station watching the same spacecraft -- see
    /// `tcslib::beacon::Schedule::for_interval` -- and a second station with
    /// a schedule of its own would be a second account of how late a beacon
    /// is.
    pub fn beacon_schedule(interval: std::time::Duration) -> Schedule {
        Schedule::for_interval(interval)
    }
}
