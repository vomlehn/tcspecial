//! Window pieces shared by the ground programs, and what to show on them.
//!
//! The other half of [`tcslib::beacon`]. That one listens and says what is
//! true of the spacecraft -- how stale the last beacon is, what it said, and
//! whether anything is being heard from; this one says what that looks like:
//! which colour the indicator is, and the two lines of text beside it.
//!
//! They are apart because the facts outlast any one window. A ground station
//! with no window wants the facts with no toolkit linked in, and two stations
//! that do have windows want the same panel rather than two that have drifted:
//! an operator who has read one has read the other, where an indicator in a
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
//! The Rust side sets its three properties from [`Shown::of`].
//!
//! The ping panel, `ui/ping.slint`, is the same arrangement for
//! [`tcslib::ping`]: the markup shows seven rows, and [`PingShown::of`] says
//! what each says.

use std::time::SystemTime;

use slint::Color;
use tcslib::beacon::{received_at, Pass, Showing, Stage};

/// The colours an indicator is drawn in.
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

    /// An indicator that has shown a colour and is not showing one now: the
    /// dark half of a blink, once a beacon has arrived. White and not grey,
    /// so that grey keeps the one meaning it had -- nothing has been heard
    /// yet -- and an indicator that has been heard from never goes back to
    /// looking as if it had not.
    pub fn off() -> Color {
        Color::from_rgb_u8(255, 255, 255)
    }

    /// All is well: the spacecraft is being heard from.
    ///
    /// A deeper green, `#22c55e`, rather than pure green, which is harsh on a
    /// white window and so nearly as light as white -- 1.37 to one -- that its
    /// blink was faint. This is 2.28 to one, so the blink is plain, and still
    /// less than red's, which is to be acted on.
    pub fn well() -> Color {
        Color::from_rgb_u8(0x22, 0xc5, 0x5e)
    }

    /// Worth looking at.
    ///
    /// A golden yellow, `#ffe100`, rather than pure yellow. A blinking
    /// indicator alternates with [`off`], which is white, and pure yellow is
    /// so nearly as light as white -- a contrast of 1.07 to one -- that its
    /// blink could hardly be seen, the one stage that asks to be looked at
    /// being the hardest to see. This is 1.31 to one, enough for the blink to
    /// show, and is still read as yellow rather than amber: a yellow with
    /// more red in it, which would blink more plainly, was read as on its way
    /// to orange, and so to something nearer red. The price is a blink less
    /// plain than green's, at 2.28.
    pub fn watch() -> Color {
        Color::from_rgb_u8(255, 225, 0)
    }

    /// Wrong.
    pub fn wrong() -> Color {
        Color::from_rgb_u8(255, 0, 0)
    }
}

/// What an indicator shows for a stage of staleness, `heard` saying whether a
/// beacon has ever arrived.
///
/// Green while the spacecraft is being heard from, yellow once it is late
/// enough that contact is not claimed, red once nothing is being heard, and
/// grey before anything has arrived. Grey is for that and nothing else: once
/// a beacon has arrived the indicator has shown a colour, and wherever it
/// shows none after that it is white. So the dark half of a blink is white --
/// a blinking indicator is one colour going dark and back, rather than two
/// colours alternating, because two would read as two different things being
/// said -- and so is a last beacon that a moved clock puts in the future.
///
/// `heard` is [`Pass::at`] having a time in it, which it keeps for good once
/// a beacon arrives. A beacon arriving is always shown in a colour, being
/// [`Stage::Heard`], so the indicator has been coloured exactly when `heard`
/// is true and no state of its own need be kept.
///
/// No red for the stages in between, and no green for the last: red is for a
/// condition somebody must act on, and an indicator that is red whenever a
/// datagram is late is an indicator nobody reads.
///
/// # Arguments
///
/// * `showing` - The stage of staleness the beacon is at, and whether the
///               indicator is lit or in the dark half of a blink.
/// * `heard`   - Whether a beacon has ever arrived.
///
/// # Returns
///
/// The indicator's colour.
pub fn indicator(showing: &Showing, heard: bool) -> Color {
    let dark = if heard {
        palette::off()
    } else {
        palette::unknown()
    };
    if !showing.lit {
        return dark;
    }

    match showing.stage {
        Stage::Unheard => dark,
        Stage::Heard => palette::well(),
        Stage::Aging => palette::watch(),
        Stage::Waiting => palette::wrong(),
    }
}

/// What the panel's three properties should be, after a pass.
///
/// All three from one reading of one beacon. A colour that said beacons were
/// arriving beside a time that said none had would be two answers to one
/// question, so they are worked out together and set together.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    /// The indicator's colour.
    pub indicator: Color,
    /// When the last beacon arrived, as a time of day, or the placeholder.
    pub received: String,
    /// What that beacon said.
    pub message: String,
    /// Which stage the indicator is at, in words: see
    /// [`tcslib::beacon::Stage::label`]. Read from the same reading as the
    /// colour, so the two cannot disagree.
    pub stage: String,
    /// What the field beside the indicator says of a restart:
    /// [`RESTARTED_CONNECT`] where the last beacon carried the restart flag,
    /// and nothing where it did not.
    pub restart: &'static str,
}

/// What the field beside the indicator says while the beacons carry the
/// restart flag: tcspecial has started -- any start, the first included --
/// or taken a RESTART, and has not been matched since. The ground is to
/// CONNECT with the configuration whose checksum the beacons carry, which is
/// what clears the flag; until then nothing but beacons and answers comes
/// down.
pub const RESTARTED_CONNECT: &str = "Restarted\u{2014}connect";

/// What that field says of a beacon whose restart flag is `flag`.
///
/// # Arguments
///
/// * `flag` - The beacon's restart flag.
///
/// # Returns
///
/// [`RESTARTED_CONNECT`] where it is set, and nothing where it is not.
pub fn restart_of(flag: bool) -> &'static str {
    if flag {
        RESTARTED_CONNECT
    } else {
        ""
    }
}

impl Shown {
    /// What to show for a pass of the beacon loop.
    ///
    /// # Arguments
    ///
    /// * `pass` - What the pass read: the stage, when the last beacon
    ///            arrived, and what it said.
    ///
    /// # Returns
    ///
    /// The panel's properties for that pass.
    pub fn of(pass: &Pass) -> Shown {
        Shown {
            indicator: indicator(&pass.showing, pass.at.is_some()),
            received: received_at(pass.at),
            message: pass.said.clone(),
            stage: pass.showing.stage.label().to_string(),
            restart: restart_of(pass.restart_flag),
        }
    }

    /// What to show for a beacon received at `at`, shown this way, saying
    /// this.
    ///
    /// For a window that keeps the three itself -- a test, or a program that
    /// shows a beacon it did not receive on this pass.
    ///
    /// # Arguments
    ///
    /// * `showing` - The stage the indicator is at, and whether it is lit.
    /// * `at`      - When the last beacon arrived, or `None` if none has.
    /// * `said`    - What that beacon said.
    ///
    /// # Returns
    ///
    /// The panel's properties for that beacon.
    pub fn at(showing: &Showing, at: Option<SystemTime>, said: &str) -> Shown {
        Shown {
            indicator: indicator(showing, at.is_some()),
            received: received_at(at),
            message: said.to_string(),
            stage: showing.stage.label().to_string(),
            restart: "",
        }
    }
}

/// What the ping panel's rows should say: `ui/ping.slint`'s seven
/// properties, from what [`tcslib::ping::Pinger`] keeps.
///
/// All from one reading of one exchange, for the reason [`Shown`] is: a time
/// received beside a ping it does not answer would be two accounts of one
/// moment.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PingShown {
    pub sent: String,
    pub sent_time: String,
    pub received: String,
    pub received_time: String,
    pub round_trip: String,
    pub up: String,
    pub down: String,
}

impl PingShown {
    /// What to show for this exchange.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The last ping sent and its answer, with their times.
    ///
    /// # Returns
    ///
    /// The rows' text, with the placeholder for a message not sent or not
    /// answered.
    pub fn of(exchange: &tcslib::ping::Exchange) -> PingShown {
        use tcslib::ping::{interval, precise_time, NO_MESSAGE};
        PingShown {
            sent: exchange
                .sent
                .clone()
                .unwrap_or_else(|| NO_MESSAGE.to_string()),
            sent_time: precise_time(exchange.sent_at),
            received: exchange
                .received
                .clone()
                .unwrap_or_else(|| NO_MESSAGE.to_string()),
            received_time: precise_time(exchange.received_at),
            round_trip: interval(exchange.round_trip()),
            up: interval(exchange.up()),
            down: interval(exchange.down()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tcslib::beacon::Schedule;

    /// The contrast between two colours, by WCAG's relative luminance:
    /// one for the same light, twenty-one for black against white.
    ///
    /// # Arguments
    ///
    /// * `a` - One colour.
    /// * `b` - The other; the order does not matter.
    ///
    /// # Returns
    ///
    /// The contrast ratio, from 1 to 21.
    fn contrast(a: Color, b: Color) -> f64 {
        let luminance = |c: Color| {
            let linear = |u: u8| {
                let v = f64::from(u) / 255.0;
                if v <= 0.040_45 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(c.red())
                + 0.7152 * linear(c.green())
                + 0.0722 * linear(c.blue())
        };
        let (light, dark) = {
            let (x, y) = (luminance(a), luminance(b));
            if x > y {
                (x, y)
            } else {
                (y, x)
            }
        };
        (light + 0.05) / (dark + 0.05)
    }

    /// Each lit colour blinks visibly against white: yellow, worth looking
    /// at, plainly more than pure yellow's 1.07 to one, and red, which is to
    /// be acted on, plainest of all.
    #[test]
    fn each_stage_blinks_visibly_against_white() {
        let against_off = |c| contrast(c, palette::off());
        let well = against_off(palette::well());
        let watch = against_off(palette::watch());
        let wrong = against_off(palette::wrong());
        assert!(watch >= 1.25, "watch is {watch:.2} to one against white");
        assert!(well >= 1.25, "well is {well:.2} to one against white");
        assert!(
            wrong >= watch.max(well),
            "wrong {wrong:.2} against white, watch {watch:.2}, well {well:.2}"
        );
    }

    /// The standard schedule's indicator ages green, yellow, red, and blinks
    /// through white; grey is for before anything has arrived.
    ///
    /// The colours are the point of the panel: an operator reads the
    /// indicator and not the stage behind it, so what each stage looks like
    /// is worth holding to.
    #[test]
    fn the_indicator_ages_through_the_palette() {
        let schedule = Schedule::standard();

        // Nothing heard: the colour of knowing nothing, which is not green.
        assert_eq!(
            indicator(&schedule.after(None, SystemTime::now()), false),
            palette::unknown()
        );

        // Just arrived, and one beacon missed: still well.
        assert_eq!(
            indicator(&schedule.at(Duration::ZERO), true),
            palette::well()
        );
        assert_eq!(
            indicator(&schedule.at(Duration::from_secs(6)), true),
            palette::well()
        );

        // Late enough to look at, and long enough to be wrong. Both stages
        // blink, so the instants are chosen in the lit half of a blink --
        // which is what the stage's own colour is shown in.
        let aging = schedule.at(Duration::from_millis(20_200));
        assert!(aging.lit, "{aging:?}");
        assert_eq!(indicator(&aging, true), palette::watch());

        let waiting = schedule.at(Duration::from_millis(40_200));
        assert!(waiting.lit, "{waiting:?}");
        assert_eq!(indicator(&waiting, true), palette::wrong());

        // And the dark half of either is the same white, whichever stage it
        // is: what the blink says is that time is passing, and what the
        // colour says is how much.
        assert_eq!(
            indicator(&schedule.at(Duration::from_millis(20_700)), true),
            palette::off()
        );
        assert_eq!(
            indicator(&schedule.at(Duration::from_millis(40_700)), true),
            palette::off()
        );

        // The dark half of a blink is white rather than a second colour: the
        // indicator is one colour going dark and back. And not grey, a beacon
        // having arrived: grey would say nothing had been heard.
        let blinking = schedule.at(Duration::from_millis(11_500));
        assert!(!blinking.lit, "ten to twenty seconds is the blinking stage");
        assert_eq!(indicator(&blinking, true), palette::off());
        assert_ne!(palette::off(), palette::unknown());
    }

    /// Once a beacon has arrived the indicator is never grey again: wherever
    /// it shows no colour, it is white.
    ///
    /// Grey says nothing has been heard. An indicator that went grey after
    /// being green would say the spacecraft had never been heard from, which
    /// is not so, and would look like the window had just opened.
    #[test]
    fn once_heard_the_indicator_is_never_grey_again() {
        let schedule = Schedule::standard();

        // Nothing has arrived: grey, as the window opens.
        let nothing =
            Shown::at(&schedule.after(None, SystemTime::now()), None, "<none>");
        assert_eq!(nothing.indicator, palette::unknown());

        // A beacon, then every moment after it over the whole schedule: a
        // colour or white, and never grey.
        let at = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(3661));
        for ms in (0..60_000).step_by(100) {
            let shown =
                Shown::at(&schedule.at(Duration::from_millis(ms)), at, "said");
            assert_ne!(shown.indicator, palette::unknown(), "at {ms} ms");
        }

        // And a last beacon in the future, which a clock that moved makes and
        // which is Unheard: white too, a beacon having arrived.
        let in_the_future = schedule.after(at, SystemTime::UNIX_EPOCH);
        assert_eq!(in_the_future.stage, Stage::Unheard);
        assert_eq!(
            Shown::at(&in_the_future, at, "said").indicator,
            palette::off()
        );
    }

    /// The three things the panel shows come from one reading.
    #[test]
    fn what_is_shown_is_one_reading_of_one_beacon() {
        let schedule = Schedule::standard();
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(3661);

        let shown = Shown::at(
            &schedule.at(Duration::ZERO),
            Some(at),
            "tcspecial v0.1.0 md5 abc",
        );
        assert_eq!(shown.indicator, palette::well());
        assert_eq!(shown.received, "01:01:01");
        assert_eq!(shown.message, "tcspecial v0.1.0 md5 abc");

        // Before anything has arrived, the time says so rather than being
        // left empty: an empty line next to a label reads as a line the
        // program has not filled in.
        let nothing =
            Shown::at(&schedule.after(None, SystemTime::now()), None, "<none>");
        assert_eq!(nothing.received, "--:--:--");
        assert_eq!(nothing.indicator, palette::unknown());
    }

    /// The field beside the indicator says to connect while the beacons
    /// carry the restart flag, and nothing while they do not.
    #[test]
    fn the_restart_field_says_to_connect_while_the_bit_is_set() {
        assert_eq!(restart_of(true), "Restarted\u{2014}connect");
        assert_eq!(restart_of(false), "");
    }

    /// The ping panel's rows, before anything is sent and once an answer has
    /// come back: empty, and then each in the words the Python shows.
    #[test]
    fn the_ping_panel_says_what_the_exchange_is() {
        use tcslib::node::vehicle::ping;
        use tcslib::ping::{Exchange, Pinger};

        // Before anything, each field shows its placeholder, in the shape of
        // what will be there: the markup's own defaults.
        let nothing = PingShown::of(&Exchange::default());
        assert_eq!(nothing.sent, "<none>");
        assert_eq!(nothing.received, "<none>");
        assert_eq!(nothing.sent_time, "--:--:--.---------");
        assert_eq!(nothing.received_time, "--:--:--.---------");
        for time in [&nothing.round_trip, &nothing.up, &nothing.down] {
            assert_eq!(time, "--.------ ms");
        }

        let at = Duration::new(1_760_900_000, 0);
        let mut pinger = Pinger::new();
        let message = pinger.ping(at).expect("a ping");
        let answer =
            ping::reply(message.as_bytes(), at + Duration::from_millis(3))
                .expect("an answer");
        pinger.answered(answer.as_bytes(), at + Duration::from_millis(5));

        let shown = PingShown::of(pinger.exchange());
        assert_eq!(shown.sent, "ping #1 1760900000.000000000");
        assert!(shown.received.starts_with("ping #1 "), "{shown:?}");
        assert_eq!(shown.round_trip, "5.000000 ms");
        assert_eq!(shown.up, "3.000000 ms");
        assert_eq!(shown.down, "2.000000 ms");
        assert!(shown.received_time.ends_with(".005000000"), "{shown:?}");
    }
}
