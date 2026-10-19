//! Window pieces shared by the ground programs, and what to show on them.
//!
//! The other half of [`tcslib::beacon`]. That one listens and says what is
//! true of the spacecraft -- how stale the last beacon is, what it said, and
//! whether anything is being heard from; this one says what that looks like:
//! which colour the light is, and the two lines of text beside it.
//!
//! They are apart because the facts outlast any one window. A ground station
//! with no window wants the facts with no toolkit linked in, and two stations
//! that do have windows want the same panel rather than two that have drifted:
//! an operator who has read one has read the other, where a light in a
//! different place or a label in different words would be two things to learn
//! about one spacecraft.
//!
//! The markup is in `ui/beacon.slint` and is imported by path, which is what
//! Slint resolves relative to the importing file:
//!
//! ```text
//! import { BeaconPanel } from "../../tcsui/ui/beacon.slint";
//! ```
//!
//! The Rust side sets its three properties from [`shown`].

use std::time::SystemTime;

use slint::Color;
use tcslib::beacon::{received_at, Pass, Showing, Stage};

/// The colours a light is drawn in.
///
/// Complete rather than only what is used: a palette is read as a whole, and a
/// colour that is here and unused is one a reader can see is spare.
pub mod palette {
    use slint::Color;

    /// Nothing is known: no beacon has arrived, or the station has stopped
    /// listening.
    pub fn unknown() -> Color {
        Color::from_rgb_u8(196, 196, 196)
    }

    /// All is well: the spacecraft is being heard from.
    pub fn well() -> Color {
        Color::from_rgb_u8(0, 255, 0)
    }

    /// Worth looking at.
    pub fn watch() -> Color {
        Color::from_rgb_u8(255, 255, 0)
    }

    /// Wrong.
    pub fn wrong() -> Color {
        Color::from_rgb_u8(255, 0, 0)
    }
}

/// What a light shows for a stage of staleness.
///
/// Green while the spacecraft is being heard from, yellow once it is late
/// enough that contact is not claimed, red once nothing is being heard, and
/// grey before anything has arrived. The dark half of a blink is grey too: a
/// blinking light is one colour going dark and back, rather than two colours
/// alternating, because two would read as two different things being said.
///
/// No red for the stages in between, and no green for the last: red is for a
/// condition somebody must act on, and a light that is red whenever a
/// datagram is late is a light nobody reads.
pub fn indicator(showing: &Showing) -> Color {
    if !showing.lit {
        return palette::unknown();
    }

    match showing.stage {
        Stage::Unheard => palette::unknown(),
        Stage::Heard => palette::well(),
        Stage::Ageing => palette::watch(),
        Stage::Silent => palette::wrong(),
    }
}

/// What the panel's three properties should be, after a pass.
///
/// All three from one reading of one beacon. A colour that said beacons were
/// arriving beside a time that said none had would be two answers to one
/// question, so they are worked out together and set together.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    /// The light's colour.
    pub indicator: Color,
    /// When the last beacon arrived, as a time of day, or the placeholder.
    pub received: String,
    /// What that beacon said.
    pub message: String,
}

impl Shown {
    /// What to show for a pass of the beacon loop.
    pub fn of(pass: &Pass) -> Shown {
        Shown {
            indicator: indicator(&pass.showing),
            received: received_at(pass.at),
            message: pass.said.clone(),
        }
    }

    /// What to show for a beacon received at `at`, shown this way, saying
    /// this.
    ///
    /// For a window that keeps the three itself -- a test, or a program that
    /// shows a beacon it did not receive on this pass.
    pub fn at(showing: &Showing, at: Option<SystemTime>, said: &str) -> Shown {
        Shown {
            indicator: indicator(showing),
            received: received_at(at),
            message: said.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tcslib::beacon::Schedule;

    /// The standard schedule's light ages green, yellow, red, and blinks
    /// through grey.
    ///
    /// The colours are the point of the panel: an operator reads the light
    /// and not the stage behind it, so what each stage looks like is worth
    /// holding to.
    #[test]
    fn the_light_ages_through_the_palette() {
        let schedule = Schedule::standard();

        // Nothing heard: the colour of knowing nothing, which is not green.
        assert_eq!(indicator(&schedule.after(None, SystemTime::now())), palette::unknown());

        // Just arrived, and one beacon missed: still well.
        assert_eq!(indicator(&schedule.at(Duration::ZERO)), palette::well());
        assert_eq!(indicator(&schedule.at(Duration::from_secs(6))), palette::well());

        // Late enough to look at, and long enough to be wrong. Both stages
        // blink, so the instants are chosen in the lit half of a blink --
        // which is what the stage's own colour is shown in.
        let ageing = schedule.at(Duration::from_millis(8_200));
        assert!(ageing.lit, "{ageing:?}");
        assert_eq!(indicator(&ageing), palette::watch());

        let silent = schedule.at(Duration::from_millis(30_200));
        assert!(silent.lit, "{silent:?}");
        assert_eq!(indicator(&silent), palette::wrong());

        // And the dark half of either is the same grey, whichever stage it
        // is: what the blink says is that time is passing, and what the
        // colour says is how much.
        assert_eq!(
            indicator(&schedule.at(Duration::from_millis(9_200))),
            palette::unknown()
        );
        assert_eq!(
            indicator(&schedule.at(Duration::from_millis(30_700))),
            palette::unknown()
        );

        // The dark half of a blink is the colour of nothing known rather than
        // a second colour: the light is one colour going dark and back.
        let blinking = schedule.at(Duration::from_millis(5_500));
        assert!(!blinking.lit, "four to eight seconds is the blinking stage");
        assert_eq!(indicator(&blinking), palette::unknown());
    }

    /// The three things the panel shows come from one reading.
    #[test]
    fn what_is_shown_is_one_reading_of_one_beacon() {
        let schedule = Schedule::standard();
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(3661);

        let shown = Shown::at(
            &schedule.at(Duration::ZERO),
            Some(at),
            "tcspecial v0.1.0, config v2.0.0, md5 abc",
        );
        assert_eq!(shown.indicator, palette::well());
        assert_eq!(shown.received, "01:01:01");
        assert_eq!(shown.message, "tcspecial v0.1.0, config v2.0.0, md5 abc");

        // Before anything has arrived, the time says so rather than being
        // left empty: an empty line next to a label reads as a line the
        // program has not filled in.
        let nothing = Shown::at(&schedule.after(None, SystemTime::now()), None, "<none>");
        assert_eq!(nothing.received, "--:--:--");
        assert_eq!(nothing.indicator, palette::unknown());
    }
}
