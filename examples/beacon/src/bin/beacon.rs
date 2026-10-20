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
//! Arguments, all optional: the configuration, and `-v` or `--verbose`. Run
//! from the repository's root it needs none.
//!
//! Quiet unless `-v` or `--verbose` is given: the console then says where it
//! is listening, each remark a beacon pass makes -- beacons missed, a vehicle
//! that restarted, a configuration that differs -- and a tcspecial that
//! stopped by itself. Without it the window is the whole account. Errors and
//! warnings are said either way, because a program that cannot do what was
//! asked, or is doing less of it, must say so to whoever started it.

use std::cell::RefCell;
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use slint::ComponentHandle;

use beacon_example::{notice_exit, show_on, wire, Vehicle, WatchWindow};
use tcslib::beacon::{Beacons, Schedule, STANDARD_INTERVAL};
use tcslibgs::config_digest::digest_of_file;
use tcslibgs::graph::parameters::{beacon_interval_of, beacon_of};
use tcslibgs::graph::Graph;
use tcsui::Shown;

/// The configuration, where nobody says otherwise.
const CONFIGURATION: &str = "examples/beacon/beacon.yaml";

/// What the command line says: the configuration, and whether to talk.
struct Arguments {
    path: String,
    verbose: bool,
}

impl Arguments {
    /// The arguments, or what is wrong with them, said for the person who
    /// typed them.
    fn of(args: impl Iterator<Item = String>) -> Result<Arguments, String> {
        let mut path = None;
        let mut verbose = false;
        for arg in args {
            match arg.as_str() {
                "-v" | "--verbose" => verbose = true,
                flag if flag.starts_with('-') => {
                    return Err(format!(
                        "{flag} is not an option: the options are -v and --verbose"
                    ))
                }
                _ if path.is_some() => {
                    return Err(format!(
                        "{arg}: one configuration, and only one, may be named"
                    ))
                }
                _ => path = Some(arg),
            }
        }
        Ok(Arguments {
            path: path.unwrap_or_else(|| CONFIGURATION.to_string()),
            verbose,
        })
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let Arguments { path, verbose } = match Arguments::of(std::env::args().skip(1)) {
        Ok(arguments) => arguments,
        Err(why) => {
            eprintln!("{why}");
            eprintln!("usage: beacon [-v | --verbose] [configuration]");
            std::process::exit(2);
        }
    };

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

    // How late a beacon is depends on how often they are sent, which is the
    // configuration's to say. The section was read for the link just now, so
    // it can be read for this; the standard interval is only for the
    // configuration that has neither.
    let interval = beacon_interval_of(&graph)
        .ok()
        .flatten()
        .unwrap_or(STANDARD_INTERVAL);

    let schedule = Schedule::for_interval(interval);
    let mut beacons = match Beacons::listening(link, schedule, expected) {
        Ok(beacons) => beacons,
        Err(e) => {
            eprintln!("beacons cannot be received: {e}");
            std::process::exit(1);
        }
    };
    if verbose {
        eprintln!(
            "Listening for beacons at {}, which {} sends from",
            beacons.link().recv,
            beacons.link().send
        );
    }

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
                    notice_exit(&window, &mut vehicle.borrow_mut(), verbose);
                }
            },
        );
    }

    // One thread, for as long as the window is open. A pass waits for a
    // beacon or for the moment the light would change by itself, so the light
    // blinks whether or not anything arrives.
    thread::spawn(move || loop {
        let pass = beacons.pass();

        // What did not arrive, said where a terminal can keep it, when asked
        // to be: beacons
        // that were missed, a spacecraft that restarted, a configuration that
        // differs. The window has room for what did arrive.
        if verbose {
            for remark in &pass.remarks {
                eprintln!("{remark}");
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn of(args: &[&str]) -> Result<Arguments, String> {
        Arguments::of(args.iter().map(|arg| arg.to_string()))
    }

    /// Quiet unless asked, by either spelling, before or after the
    /// configuration.
    #[test]
    fn verbose_is_asked_for() {
        let quiet = of(&[]).expect("no arguments are arguments");
        assert!(!quiet.verbose);
        assert_eq!(quiet.path, CONFIGURATION);

        for args in [
            &["-v"][..],
            &["--verbose"],
            &["a.yaml", "-v"],
            &["-v", "a.yaml"],
        ] {
            assert!(of(args).expect("they read").verbose, "{args:?}");
        }
        assert_eq!(of(&["-v", "a.yaml"]).expect("they read").path, "a.yaml");
    }

    /// What is not an option, and a second configuration, are refused rather
    /// than taken for something.
    #[test]
    fn what_is_not_understood_is_refused() {
        assert!(of(&["-q"]).is_err());
        assert!(of(&["a.yaml", "b.yaml"]).is_err());
    }
}
