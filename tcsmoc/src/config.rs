//! Configuration loading for tcsmoc

/// Configuration constants
pub mod constants {
    use slint::Color;

    use tcslib::beacon::Schedule;

    /// What a payload panel's data indicator shows, by what the ground can
    /// say about it; see `crate::freshness`.
    ///
    /// Three colours and no red: a payload whose data has stopped may be a
    /// payload with nothing to say, one somebody stopped, or a link that has
    /// failed, and nothing on the ground can tell those apart. Red is for a
    /// dangerous condition; this is one worth looking at.
    ///
    /// The colours are `tcsui`'s, which is where the beacon's light takes
    /// them from too: one palette, so that green means the same thing
    /// wherever this window shows it.
    pub fn freshness_color(freshness: crate::freshness::Freshness) -> Color {
        use crate::freshness::Freshness;
        use tcsui::palette;

        match freshness {
            Freshness::Unjudged => palette::unknown(),
            Freshness::Current => palette::well(),
            Freshness::Stale => palette::watch(),
        }
    }

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
