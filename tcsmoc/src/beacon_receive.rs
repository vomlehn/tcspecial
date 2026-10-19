//! The beacon, in this window.
//!
//! What a beacon is, how stale the last one is and what its number says about
//! the ones before it are not this program's business: they are the same for
//! any ground station, and they live in [`tcslib::beacon`]. What a panel
//! showing them looks like is `tcsui`, whose markup this window imports.
//!
//! What is left here is this window's own: a thread to run the loop on, the
//! window to put the answers in, the question to ask the operator when the
//! beacons announce a configuration this MOC did not read, and the contact
//! flag the panels act on.
//!
//! It used to be the whole of it -- the socket, the digest comparison, the
//! sequence counting, the colours and the blink schedule, in one file with
//! the window. The `beacon` example is what divided it: a second ground
//! station showing the same panel would otherwise have been a second copy of
//! all of that, free to drift from this one in what it said about the same
//! spacecraft.

use std::sync::{Arc, Mutex};
use std::thread;

use slint::{SharedString, Weak};

use tcslib::beacon::{Beacons, Contact, Link, Schedule};
use tcslibgs::ConfigDigest;
use tcsui::Shown;

use crate::MainWindow;

/// Asks the operator, with what differs, whether to continue anyway.
pub type Ask = Arc<dyn Fn(String) + Send + Sync>;

/// That this window's beacons are being watched.
///
/// It holds nothing. The thread owns the socket and what it has heard, and the
/// contact flag belongs to the caller, which hands it in so that the rest of
/// the window can read it. What this is for is saying whether the watch
/// started at all: `None` from [`BeaconReceive::new`] is a socket that could
/// not be bound, which is a ground station that will never hear a beacon.
pub struct BeaconReceive;

impl BeaconReceive {
    /// Start watching, and return nothing where the socket cannot be bound.
    ///
    /// A ground station that cannot bind where beacons arrive is a ground
    /// station that will never hear one, which is said here rather than left
    /// to look like a spacecraft that is not beaconing.
    pub fn new(
        ui_weak: Weak<MainWindow>,
        link: Link,
        schedule: Schedule,
        contact: Arc<Mutex<Contact>>,
        expected: Option<ConfigDigest>,
        ask: Option<Ask>,
    ) -> Option<BeaconReceive> {
        let mut beacons = match Beacons::listening(link, schedule, expected) {
            Ok(beacons) => beacons,
            Err(e) => {
                eprintln!("beacons cannot be received: {e}");
                return None;
            }
        };

        let held = contact.clone();
        thread::spawn(move || loop {
            let pass = beacons.pass();

            // Said on the console: a configuration that differs, beacons that
            // did not arrive, a spacecraft that has restarted. The window has
            // room for what arrived, and these are about what did not.
            for remark in &pass.remarks {
                eprintln!("{remark}");
            }
            // The one remark an operator must not be left to find in a
            // terminal. It is asked from this thread, so it takes the
            // question to the window itself.
            if let Some(ask) = &ask {
                for remark in &pass.remarks {
                    if remark.starts_with("CONFIGURATION DIFFERS") {
                        ask(remark.clone());
                    }
                }
            }

            // The panels' half of the same reading, left where the poller can
            // read it. Every pass, not only the ones that received something:
            // contact is lost by nothing arriving, which is what the passes
            // that time out are.
            if let Ok(mut contact) = held.lock() {
                *contact = pass.contact;
            }

            let shown = Shown::of(&pass);
            let ui_weak = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    show_beacon(&ui, &shown);
                }
            });
        });

        Some(BeaconReceive)
    }
}

/// Put the beacon's state in the window: the light's colour, when the last
/// beacon arrived, and what it said.
///
/// All three at once, from the one reading of what the last beacon was: a
/// colour that said beacons were arriving beside a time that said none had
/// would be two answers to one question.
pub(crate) fn show_beacon(ui: &MainWindow, shown: &Shown) {
    ui.set_indicator_color(shown.indicator);
    ui.set_beacon_last_recv(SharedString::from(shown.received.clone()));
    ui.set_beacon_last_msg(SharedString::from(shown.message.clone()));
}
