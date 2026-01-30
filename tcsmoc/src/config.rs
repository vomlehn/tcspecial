//! Configuration loading for tcsmoc

/// Configuration constants
pub mod constants {
    use slint::Color;
    use std::sync::LazyLock;
    use std::time::Duration;

    use crate::beacon_receive::{IndicatorState, IndicatorStates};

    // Color constants - using functions since Color::from_rgb_u8 isn't const
    fn red() -> Color { Color::from_rgb_u8(255, 0, 0) }
    fn green() -> Color { Color::from_rgb_u8(0, 255, 0) }
    fn yellow() -> Color { Color::from_rgb_u8(255, 255, 0) }
    fn blue() -> Color { Color::from_rgb_u8(0, 0, 255) }
    fn grey() -> Color { Color::from_rgb_u8(127, 127, 127) }
    fn transparent() -> Color { Color::from_argb_u8(0, 0, 0, 0) }

    // Information defining the behavior of the Beacon indicator
    pub static BEACON_INDICATOR: LazyLock<IndicatorStates> = LazyLock::new(|| {
        IndicatorStates::new(
            grey(),                         // unset color
            [
                IndicatorState::Steady(Duration::from_millis(1000), green()),
                IndicatorState::Blinking(Duration::from_millis(4000),
                    Duration::from_millis(1000), Duration::from_millis(1000),
                    blue(), transparent()),
                IndicatorState::Blinking(Duration::MAX,
                    Duration::from_millis(750), Duration::from_millis(750),
                    red(), transparent()),
            ].to_vec(),
        )
    });
}
