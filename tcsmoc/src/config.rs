//! Configuration loading for tcsmoc

/// Configuration constants
pub mod constants {
    use slint::Color;
    use std::sync::LazyLock;
    use std::time::Duration;

    use crate::beacon_receive::{IndicatorPhase, IndicatorPhases};

    // Color constants - using functions since Color::from_rgb_u8 isn't const
    fn red() -> Color { Color::from_rgb_u8(255, 0, 0) }
    fn green() -> Color { Color::from_rgb_u8(0, 255, 0) }
    fn yellow() -> Color { Color::from_rgb_u8(255, 255, 0) }
    fn grey() -> Color { Color::from_rgb_u8(127, 127, 127) }
    fn transparent() -> Color { Color::from_argb_u8(0, 0, 0, 0) }

    // Information defining the behavior of the Beacon indicator
    pub static BEACON_INDICATOR: LazyLock<IndicatorPhases> = LazyLock::new(|| {
        IndicatorPhases::new(
            Duration::from_millis(500),  // blink duration
            grey(),                       // unset color
            vec![
                IndicatorPhase::new(
                    Duration::from_millis(5000),
                    green(),
                    None,  // no blinking
                ),
                IndicatorPhase::new(
                    Duration::from_millis(5000),
                    yellow(),
                    Some(transparent()),  // blinks
                ),
                IndicatorPhase::new(
                    Duration::MAX,
                    red(),
                    Some(transparent()),  // blinks
                ),
            ],
        )
    });
}
