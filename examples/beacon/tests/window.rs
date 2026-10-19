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
    // said, and a light that says the spacecraft is being heard from.
    let said = "tcspecial v0.1.0, config v2.0.0, md5 0123456789abcdef";
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
        "a beacon has just arrived, so the light says all is well"
    );

    // And long enough after it, the light says so without anything else
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
