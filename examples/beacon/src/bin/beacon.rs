//! The example's ground end: a window showing the spacecraft's beacon.
//!
//! What a ground station can know about a spacecraft without asking it
//! anything. tcspecial beacons every `beacon.interval` for as long as it
//! runs, over a socket of its own, whether or not anything has connected to
//! it -- so this program commands nothing, connects to nothing, and still
//! says whether the vehicle is alive, which build it is, which version of a
//! configuration it is serving and whether that is the configuration this end
//! read.
//!
//! It is also the example of how the two halves fit together. Nothing about
//! beacons is written here:
//!
//! * [`tcslib::beacon`] binds the socket, reads each datagram, counts the
//!   beacons that did not arrive, compares the digest with this end's own and
//!   says how stale the last one is.
//! * `tcsui` holds the panel and works out what to show on it.
//!
//! What is left is this program: read the configuration, start the loop, put
//! each pass in the window. And a button that starts tcspecial on the same
//! configuration and stops it again, so that the light can be watched coming
//! on and going out: see [`beacon_example::Vehicle`]. tcsmoc does the same thing with the same two
//! halves, in a panel beside its command link; an operator who has read one
//! has read the other.
//!
//! Arguments, both optional: the configuration, and nothing else. Run from the
//! repository's root it needs none.

use std::cell::RefCell;
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use slint::ComponentHandle;

use beacon_example::{notice_exit, show_on, wire, Vehicle, WatchWindow};
use tcslib::beacon::{Beacons, Schedule};
use tcslibgs::config_digest::digest_of_file;
use tcslibgs::graph::parameters::beacon_of;
use tcslibgs::graph::Graph;
use tcsui::Shown;

/// The configuration, where nobody says otherwise.
const CONFIGURATION: &str = "examples/beacon/beacon.yaml";

fn main() -> Result<(), slint::PlatformError> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| CONFIGURATION.to_string());

    let graph = match Graph::load(&path) {
        Ok(graph) => graph,
        Err(why) => {
            eprintln!("{path}: {why}");
            std::process::exit(1);
        }
    };

    // Where the beacons are, which the configuration says and this program
    // does not: a station listening where the spacecraft is not sending hears
    // nothing and says nothing about why.
    let link = match beacon_of(&graph) {
        Ok(Some(link)) => link,
        Ok(None) => {
            eprintln!(
                "{path} has no tcspecial section, so it says nothing about where its \
                 beacons go -- and a configuration with no section sends none"
            );
            std::process::exit(1);
        }
        Err(why) => {
            eprintln!("{path}: {why}");
            std::process::exit(1);
        }
    };

    // The digest of the configuration this end read, which every beacon's
    // digest is compared with. A difference is the one thing a beacon can say
    // that makes everything else suspect: the vehicle is serving a
    // configuration this station has not read, so what this station believes
    // about the vehicle's nodes may not be what is aboard. Nothing is
    // compared where this cannot be worked out.
    let expected = match digest_of_file(&path) {
        Ok(digest) => Some(digest),
        Err(why) => {
            eprintln!("{path} cannot be digested, so no beacon's digest is checked: {why}");
            None
        }
    };

    let mut beacons = match Beacons::listening(link, Schedule::standard(), expected) {
        Ok(beacons) => beacons,
        Err(e) => {
            eprintln!("beacons cannot be received: {e}");
            std::process::exit(1);
        }
    };
    eprintln!(
        "Listening for beacons at {}, which {} sends from",
        beacons.link().recv,
        beacons.link().send
    );

    // The vehicle the button starts, which is not running until it is
    // pressed. Found before the window opens, so that a tcspecial nobody has
    // built is said once, here, and not on every press.
    let vehicle = match Vehicle::beside_this_program(&path) {
        Ok(vehicle) => vehicle,
        Err(why) => {
            eprintln!("{why}");
            std::process::exit(1);
        }
    };
    let vehicle = Rc::new(RefCell::new(vehicle));

    let window = WatchWindow::new()?;
    let weak = window.as_weak();
    wire(&window, Rc::clone(&vehicle));

    // Twice a second, whether the vehicle has stopped by itself, so that the
    // button does not go on offering to stop a tcspecial that is gone.
    let watching = slint::Timer::default();
    {
        let weak = window.as_weak();
        let vehicle = Rc::clone(&vehicle);
        watching.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(500),
            move || {
                if let Some(window) = weak.upgrade() {
                    notice_exit(&window, &mut vehicle.borrow_mut());
                }
            },
        );
    }

    // One thread, for as long as the window is open. A pass waits for a
    // beacon or for the moment the light would change by itself, so the light
    // blinks whether or not anything arrives.
    thread::spawn(move || loop {
        let pass = beacons.pass();

        // What did not arrive, said where a terminal can keep it: beacons
        // that were missed, a spacecraft that restarted, a configuration that
        // differs. The window has room for what did arrive.
        for remark in &pass.remarks {
            eprintln!("{remark}");
        }

        // Into the window, on the thread that owns it.
        show_on(&weak, Shown::of(&pass));
    });

    let ran = window.run();
    // The window is closed: stop the vehicle it started, rather than leave
    // it beaconing with no button to stop it.
    if let Err(why) = vehicle.borrow_mut().stop() {
        eprintln!("{why}");
    }
    ran
}
