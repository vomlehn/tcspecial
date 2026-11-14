//! Configuration loading for tcs-moc

/// Configuration constants
pub mod constants {
    use tcsmoc::beacon::Schedule;

    /// How the beacon's indicator ages, for beacons sent every `interval`:
    /// `tcsmoc`'s schedule for that interval.
    ///
    /// Not this window's own any more. The stages and their timings are the
    /// same for any ground station watching the same spacecraft -- see
    /// `tcsmoc::beacon::Schedule::for_interval` -- and a second station
    /// with a schedule of its own would be a second account of how late a
    /// beacon is.
    pub fn beacon_schedule(interval: std::time::Duration) -> Schedule {
        Schedule::for_interval(interval)
    }
}
