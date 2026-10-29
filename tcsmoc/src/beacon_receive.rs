//! The beacon, in this window.
//!
//! What a beacon is, how stale the last one is and what its number says about
//! the ones before it are not this program's business: they are the same for
//! any ground station, and they live in [`tcslib::beacon`]. What a panel
//! showing them looks like is `tcsui`, whose markup this window imports.
//!
//! What is left here is this window's own: a thread to run the loop on, the
//! window to put the answers in, the question to ask the operator when the
//! beacons announce a configuration this MOC did not read, the contact flag
//! the panels act on, and handing each beacon to the window's commands.
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

use tcslib::beacon::{Arrived, Beacons, Contact, Link, Schedule};
use tcslibgs::ConfigDigest;
use tcsui::Shown;

use crate::MainWindow;

/// Asks the operator, with what differs, whether to continue anyway.
pub type Ask = Arc<dyn Fn(String) + Send + Sync>;

/// What to do when contact with the spacecraft changes.
///
/// Called from the beacon thread, once, on the pass where contact changes --
/// see [`contact_lost`] and [`contact_made`] -- and not on the passes after
/// it, which are the same state going on. What it does with the window is
/// its own business, and it has to take that to the window's thread itself.
pub type OnContact = Arc<dyn Fn() + Send + Sync>;

/// What the beacons decide and the rest of the window reads: whether the
/// spacecraft is being heard from, which the panels act on, and each beacon
/// that arrives, which the commands wait on.
pub struct Decided {
    pub contact: Arc<Mutex<Contact>>,
    /// Given, for each beacon that arrives, whether it says tcspecial has
    /// started again and whether a commanded restart is under way: what
    /// ends the window's wait after an Initiate restart.
    pub heard: Option<OnBeacon>,
}

/// What to do with each beacon that arrives: whether it says tcspecial has
/// started again, and whether a commanded restart is under way. Called from
/// the beacon thread, as [`OnContact`] is.
pub type OnBeacon = Arc<dyn Fn(bool, bool) + Send + Sync>;

/// What the window does as contact is lost and as it is made.
#[derive(Clone, Default)]
pub struct Contacts {
    /// As contact is lost: every node's status goes stale.
    pub lost: Option<OnContact>,
    /// As contact is made, on the first beacon and at the end of an outage:
    /// the spacecraft is asked what each node is doing.
    pub made: Option<OnContact>,
}

/// Whether a pass that reads `now` is the one where contact was lost, the
/// pass before it having read `was`.
///
/// Only a contact that was made can be lost. Before the first beacon the
/// ground has never heard from the spacecraft, so there is nothing for a
/// node's status to have gone stale from.
pub fn contact_lost(was: Contact, now: Contact) -> bool {
    was == Contact::Made && now == Contact::Lost
}

/// Whether a pass that reads `now` is the one where contact was made: the
/// first beacon, or the first after an outage.
pub fn contact_made(was: Contact, now: Contact) -> bool {
    was == Contact::Lost && now == Contact::Made
}

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
        decided: Decided,
        expected: Option<ConfigDigest>,
        ask: Option<Ask>,
        contacts: Contacts,
    ) -> Option<BeaconReceive> {
        let Decided { contact, heard } = decided;
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

            // Every beacon to the window's commands, whose wait after an
            // Initiate restart only a beacon ends.
            if let (Arrived::Beacon, Some(heard)) =
                (&pass.arrived, heard.as_ref())
            {
                heard(pass.restarted, pass.commanded_restart);
            }

            // Said on the console: a configuration that differs, beacons that
            // did not arrive, a spacecraft that has restarted. The window has
            // room for what arrived, and these are about what did not.
            for remark in pass.remarks.iter().chain(&pass.warnings) {
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
            let was = match held.lock() {
                Ok(mut contact) => {
                    std::mem::replace(&mut *contact, pass.contact)
                }
                Err(_) => pass.contact,
            };
            // And what the nodes do about a change, once, as it happens. A
            // loss: every status a panel shows is what the spacecraft said
            // while it was being heard from, and none of it can be taken as
            // true now. Contact made: the spacecraft can be asked again.
            let then = if contact_lost(was, pass.contact) {
                contacts.lost.as_ref()
            } else if contact_made(was, pass.contact) {
                contacts.made.as_ref()
            } else {
                None
            };
            if let Some(then) = then {
                then();
            }

            let shown = Shown::of(pass);
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

/// Put the beacon's state in the window: the indicator's colour, when the
/// last beacon arrived, and what it said.
///
/// All three at once, from the one reading of what the last beacon was: a
/// colour that said beacons were arriving beside a time that said none had
/// would be two answers to one question.
pub(crate) fn show_beacon(ui: &MainWindow, shown: &Shown) {
    ui.set_indicator_color(shown.indicator);
    ui.set_beacon_restart(SharedString::from(shown.restart));
    ui.set_beacon_last_recv(SharedString::from(shown.received.clone()));
    ui.set_beacon_last_msg(SharedString::from(shown.message.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Contact is lost on the one pass where it goes from made to lost: not
    /// before the first beacon, when it was never made, and not on the passes
    /// after, which are the same loss going on.
    #[test]
    fn contact_is_lost_once_and_only_after_it_was_made() {
        assert!(contact_lost(Contact::Made, Contact::Lost));
        assert!(!contact_lost(Contact::Lost, Contact::Lost));
        assert!(!contact_lost(Contact::Made, Contact::Made));
        assert!(!contact_lost(Contact::Lost, Contact::Made));
    }

    /// Contact is made on the first beacon and on the first after an
    /// outage, and on no pass while it simply holds.
    #[test]
    fn contact_is_made_once_on_each_return() {
        assert!(contact_made(Contact::Lost, Contact::Made));
        assert!(!contact_made(Contact::Made, Contact::Made));
        assert!(!contact_made(Contact::Lost, Contact::Lost));
        assert!(!contact_made(Contact::Made, Contact::Lost));
    }
}
