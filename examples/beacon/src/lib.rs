//! The example's window, and the one thing this program does to it.
//!
//! A library beside the binary so that the window can be opened by a test as
//! well as by a person: what the test checks is the wiring below, which is the
//! whole of what this example adds to the two halves it is made of -- see
//! `src/bin/beacon_watch.rs`.

use tcsui::Shown;

slint::include_modules!();

/// Put what the beacon loop reported into the window.
///
/// All three at once, from the one reading: a light that said beacons were
/// arriving beside a time that said none had would be two answers to one
/// question. The window passes them down to the panel, which is `tcsui`'s.
pub fn show(window: &WatchWindow, shown: &Shown) {
    window.set_indicator_color(shown.indicator);
    window.set_last_received(shown.received.clone().into());
    window.set_last_message(shown.message.clone().into());
}

/// And the same for a window this does not own, which is what a background
/// thread has.
pub fn show_on(window: &slint::Weak<WatchWindow>, shown: Shown) {
    let window = window.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(window) = window.upgrade() {
            show(&window, &shown);
        }
    });
}
