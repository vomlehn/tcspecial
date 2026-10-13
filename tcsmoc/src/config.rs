//! Configuration loading for tcsmoc

/// Configuration constants
pub mod constants {
    use slint::Color;
    use std::sync::LazyLock;
    use std::time::Duration;

    use crate::beacon_receive::{IndicatorState, IndicatorStates};

    // Color constants - using functions since Color::from_rgb_u8 isn't const.
    // The palette is complete rather than only what is used: blue and
    // transparent belong to indicators not yet defined, so they are kept and
    // marked rather than removed and looked up again later.
    fn red() -> Color { Color::from_rgb_u8(255, 0, 0) }
    fn green() -> Color { Color::from_rgb_u8(0, 255, 0) }
    fn yellow() -> Color { Color::from_rgb_u8(255, 255, 0) }
    #[allow(dead_code)]
    fn blue() -> Color { Color::from_rgb_u8(0, 0, 255) }
    fn grey() -> Color { Color::from_rgb_u8(196, 196, 196) }
    #[allow(dead_code)]
    fn transparent() -> Color { Color::from_argb_u8(0, 0, 0, 0) }

    /// What a payload panel's data indicator shows, by what the ground can
    /// say about it; see `crate::freshness`.
    ///
    /// Three colours and no red: a payload whose data has stopped may be a
    /// payload with nothing to say, one somebody stopped, or a link that has
    /// failed, and nothing on the ground can tell those apart. Red is for a
    /// dangerous condition; this is one worth looking at.
    pub fn freshness_color(freshness: crate::freshness::Freshness) -> Color {
        use crate::freshness::Freshness;

        match freshness {
            Freshness::Unjudged => grey(),
            Freshness::Current => green(),
            Freshness::Stale => yellow(),
        }
    }

    // Information defining the behavior of the Beacon indicator
    pub static BEACON_INDICATOR: LazyLock<IndicatorStates> = LazyLock::new(|| {
        IndicatorStates::new(
            grey(),                         // unset color
            green(),                        // the colour of being heard from
            vec![
                IndicatorState::Steady(Duration::from_millis(4000), green()),
                IndicatorState::Blinking(Duration::from_millis(4000),
                    Duration::from_millis(1000), Duration::from_millis(1000),
                    green(), grey()),
                IndicatorState::Blinking(Duration::from_millis(6000),
                    Duration::from_millis(1000), Duration::from_millis(1000),
                    yellow(), grey()),
                IndicatorState::Blinking(Duration::MAX,
                    Duration::from_millis(500), Duration::from_millis(500),
                    red(), grey()),
            ],
        )
    });
}
