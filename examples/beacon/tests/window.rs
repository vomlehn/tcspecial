//! The example's window shows what the beacon loop reported.
//!
//! Three properties, passed down to a panel that is not this example's: the
//! kind of wiring that fails silently, a window going on showing its defaults
//! while the program behind it works perfectly. So it is opened headlessly,
//! through Slint's testing backend, and the text in it is read back.
//!
//! What the panel looks like is `tcsui`'s business and what a beacon means is
//! `tcslib::beacon`'s; this is the join.

use std::time::{Duration, SystemTime};

use i_slint_backend_testing::ElementHandle;
use slint::ComponentHandle;

use beacon_example::{show, WatchWindow};
use tcslib::beacon::{Schedule, NO_BEACON_MESSAGE};
use tcslibgs::NO_TRANSFER_TIME;
use tcsui::{palette, Shown};

/// What the window has on it, as an operator would read it.
///
/// # Arguments
///
/// * `window` - The window, shown.
///
/// # Returns
///
/// The accessible label of every text element on it.
fn on_screen(window: &WatchWindow) -> Vec<String> {
    ElementHandle::find_by_element_type_name(window, "Text")
        .filter_map(|element| element.accessible_label())
        .map(|label| label.to_string())
        .collect()
}

#[test]
fn the_window_shows_what_a_pass_reported() {
    i_slint_backend_testing::init_no_event_loop();

    let window = WatchWindow::new().expect("the window opens");
    window.show().expect("it is shown");

    // Before anything has arrived: the placeholder every other time line
    // uses, and the message line saying nothing has been said. An empty line
    // beside a label reads as a line the program has not filled in.
    let shown = on_screen(&window);
    assert!(
        shown.iter().any(|line| line == NO_TRANSFER_TIME),
        "the panel does not say that nothing has arrived: {shown:?}"
    );
    assert!(
        shown.iter().any(|line| line == NO_BEACON_MESSAGE),
        "the panel does not say that nothing has been said: {shown:?}"
    );

    // A beacon, as the loop would report it: the time it arrived, what it
    // said, and an indicator that says the spacecraft is being heard from.
    let said = "tcspecial v0.1.0 md5 0123456789abcdef";
    show(
        &window,
        &Shown::at(
            &Schedule::standard().at(Duration::ZERO),
            Some(SystemTime::UNIX_EPOCH + Duration::from_secs(7322)),
            said,
        ),
    );

    let shown = on_screen(&window);
    assert!(
        shown.iter().any(|line| line == "02:02:02"),
        "the beacon's arrival time is not on screen: {shown:?}"
    );
    assert!(
        shown.iter().any(|line| line == said),
        "what the beacon said is not on screen: {shown:?}"
    );
    assert_eq!(
        window.get_indicator_color(),
        palette::well(),
        "a beacon has just arrived, so the indicator says all is well"
    );

    // And long enough after it, the indicator says so without anything else
    // having to happen: the loop reports the age, and the age is what the
    // colour is worked out from.
    show(
        &window,
        &Shown::at(
            &Schedule::standard().at(Duration::from_secs(30)),
            Some(SystemTime::UNIX_EPOCH + Duration::from_secs(7322)),
            said,
        ),
    );
    assert_ne!(
        window.get_indicator_color(),
        palette::well(),
        "half a minute with no beacon should not still look well"
    );
}

/// The stage line says which stage the indicator is at, in a word, from the
/// window opening -- Unheard -- through every stage a beacon ages through.
#[test]
fn the_stage_line_follows_the_stage() {
    use tcslib::beacon::Stage;

    i_slint_backend_testing::init_no_event_loop();
    let window = WatchWindow::new().expect("the window opens");
    window.show().expect("it is shown");

    // Before any pass: the word the markup opens with, which has to be the
    // one the library calls the first stage.
    assert_eq!(window.get_stage(), Stage::Unheard.label());
    assert!(on_screen(&window).iter().any(|line| line == "Unheard"));

    // Nothing heard, as a pass reports it.
    let schedule = Schedule::standard();
    show(
        &window,
        &Shown::at(&schedule.after(None, SystemTime::now()), None, "<none>"),
    );
    assert_eq!(window.get_stage(), "Unheard");

    // A beacon, then a minute of it ageing: the line goes through the stages
    // in order, and says each one the indicator is showing.
    let at = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(7322));
    let mut seen: Vec<String> = Vec::new();
    for ms in (0..60_000).step_by(100) {
        let showing = schedule.at(Duration::from_millis(ms));
        show(&window, &Shown::at(&showing, at, "said"));
        let said = window.get_stage().to_string();
        assert_eq!(said, showing.stage.label(), "at {ms} ms");
        if seen.last() != Some(&said) {
            seen.push(said);
        }
    }
    assert_eq!(seen, ["Heard", "Aging", "Waiting"]);
    assert!(on_screen(&window).iter().any(|line| line == "Waiting"));

    // Where it is: on the Status row, after the label and the indicator,
    // with no label of its own.
    assert!(
        !on_screen(&window)
            .iter()
            .any(|line| line.starts_with("Stage")),
        "the stage has a label of its own: {:?}",
        on_screen(&window)
    );
    let text = |said: &str| {
        ElementHandle::find_by_element_type_name(&window, "Text")
            .find(|element| element.accessible_label().as_deref() == Some(said))
            .unwrap_or_else(|| panic!("no {said:?} on screen"))
    };
    let (status, stage) = (text("Status:"), text("Waiting"));
    let status_right = status.absolute_position().x + status.size().width;
    assert!(
        stage.absolute_position().x > status_right,
        "the stage is not to the right of the Status label"
    );
    let middle =
        |e: &ElementHandle| e.absolute_position().y + e.size().height / 2.0;
    assert!(
        (middle(&stage) - middle(&status)).abs() <= 2.0,
        "the stage is not on the Status row"
    );
    // The indicator is the round thing on that row, between the label and
    // the stage.
    let indicator =
        ElementHandle::find_by_element_type_name(&window, "Rectangle")
            .filter(|e| (middle(e) - middle(&status)).abs() <= 2.0)
            .filter(|e| {
                e.size().width > 0.0
                    && (e.size().width - e.size().height).abs() < 0.5
            })
            .min_by(|a, b| {
                a.absolute_position().x.total_cmp(&b.absolute_position().x)
            })
            .expect("the indicator");
    assert!(
        indicator.absolute_position().x > status_right,
        "the indicator is not after the Status label"
    );
    assert!(
        stage.absolute_position().x
            >= indicator.absolute_position().x + indicator.size().width,
        "the stage is not after the indicator"
    );
}

/// Everything is left justified: the labels start at one place, the values
/// after them all start at another, just past the widest label -- the
/// indicator being the Status row's -- and nothing moves as the stage
/// changes.
#[test]
fn the_values_line_up_after_their_labels() {
    use beacon_example::prepare;
    use tcslib::beacon::Stage;

    i_slint_backend_testing::init_no_event_loop();
    let window = WatchWindow::new().expect("the window opens");
    prepare(&window);
    window.show().expect("it is shown");

    // The panel's own copy of the stage's words is Stage::label's.
    let words: Vec<String> =
        [Stage::Unheard, Stage::Heard, Stage::Aging, Stage::Waiting]
            .iter()
            .map(|stage| stage.label().to_string())
            .collect();
    let panel =
        ElementHandle::find_by_element_id(&window, "WatchWindow::panel")
            .next()
            .expect("the window has the panel");
    let x_of = |text: &str| {
        ElementHandle::find_by_element_type_name(&window, "Text")
            .filter(|e| e.accessible_label().as_deref() == Some(text))
            .map(|e| e.absolute_position().x)
            .next()
            .unwrap_or_else(|| panic!("no {text:?} on the window"))
    };
    let indicator_x = || {
        ElementHandle::find_by_element_type_name(&window, "Rectangle")
            .filter(|e| {
                e.size().width > 0.0
                    && (e.size().width - e.size().height).abs() < 0.5
            })
            .map(|e| e.absolute_position().x)
            .next()
            .expect("the indicator")
    };

    let mut seen = None;
    for word in &words {
        window.set_stage(word.as_str().into());
        let labels = [x_of("Status:"), x_of("Last rcvd:"), x_of("Last msg:")];
        let values = [
            indicator_x(),
            x_of(NO_TRANSFER_TIME),
            x_of(NO_BEACON_MESSAGE),
        ];
        assert!(
            labels.iter().all(|&x| x == labels[0]),
            "{word}: the labels start at {labels:?}"
        );
        assert!(
            values.iter().all(|&x| x == values[0]),
            "{word}: the values start at {values:?}"
        );
        assert!(
            (labels[0] - panel.absolute_position().x) < values[0] - labels[0],
            "{word}: the labels are not at the left of the panel"
        );
        let now = (labels[0], values[0], x_of(word));
        assert_eq!(*seen.get_or_insert(now), now, "{word}: something moved");
    }

    // And the values start just past the widest label and the column's
    // spacing, not halfway across a wide panel.
    let widest_label = ["Status:", "Last rcvd:", "Last msg:"]
        .iter()
        .map(|text| {
            ElementHandle::find_by_element_type_name(&window, "Text")
                .filter(|e| e.accessible_label().as_deref() == Some(*text))
                .map(|e| e.size().width)
                .next()
                .expect("the label")
        })
        .fold(0.0f32, f32::max);
    let (labels, values, _) = seen.expect("a stage was shown");
    assert!(
        values - labels <= widest_label + 12.0 + 1.0,
        "the values start {} past the labels, the widest label being \
         {widest_label}",
        values - labels
    );
}
