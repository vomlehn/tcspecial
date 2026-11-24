//! The lander flown offline, against its own physical model, a step at a time,
//! and watched as tcs-sim's window watches it: no world, no clock and no
//! payload. For pictures of the windows that show the lander -- tcs-sim's
//! pane, and the lander example's ground station, which draws it with
//! tcs-sim's own drawing -- made from flights as they really go: see
//! `make screenshots-lander`.
//!
//! Three flights of the lander example's lander, each to a moment worth a
//! picture: [`flying`], by hand, low over the pad and coming down close to
//! the limit; [`crashed`], by a hand that cuts the engine once low, a
//! second after it hits; and [`landed`], by Autoland, three seconds into the
//! pad's wait.

use std::collections::VecDeque;
use std::path::Path;

use tcsprotocol::node::physical::lander::{Resolved, TERMINALS};
use tcsprotocol::node::physical::Resolution;
use tcsprotocol::sim_config::SimConfigFile;

use crate::lander_view::{
    Autopilot, Frame, Touchdown, Watch, CRASHED, FUEL, LANDED, THROTTLE_X,
    THROTTLE_Y, Y,
};
use crate::landing_rating as rating;
use crate::node::physical::component::Component;
use crate::node::physical::lander::Lander;

/// How long a step of a [`Flight`] is, in seconds.
const STEP: f64 = 0.05;

/// How long a flight may go on before it is given up, in seconds.
const LONGEST: f64 = 300.0;

/// How high over the pad the lander is, in metres, where a hand eases the
/// engine off: see [`eased`].
const LOW: f64 = 150.0;

/// What share of the autopilot's push up the hand gives, low, in
/// [`flying`]: enough to land, but faster than Autoland would.
const EASED: f64 = 0.8;

/// A flight of a lander, flown against its own physical model a step at a time,
/// each command held back a tenth of a second as the vehicles hold it back,
/// and watched as tcs-sim's window watches it.
struct Flight {
    /// The lander's figures.
    f: Resolved,
    /// The lander.
    lander: Lander,
    /// The commands the vehicles have yet to pass on: two steps' worth.
    on_the_way: VecDeque<(f64, f64)>,
    /// The throttles the lander is held at, X and Y, which the world gives
    /// the window beside the lander's outputs.
    held: (f64, f64),
    /// The simulated time, in seconds from the start.
    now: f64,
    /// The lander's terminals' values now, as the world gives the window
    /// them.
    out: Vec<f64>,
    /// What the window has seen of it.
    watch: Watch,
    /// What the window shows now.
    frame: Frame,
}

impl Flight {
    /// Starts a flight, the lander at its start.
    ///
    /// # Arguments
    ///
    /// * `f` - The lander's figures.
    ///
    /// # Returns
    ///
    /// The flight, watched once.
    fn new(f: &Resolved) -> Flight {
        let mut flight = Flight {
            f: f.clone(),
            lander: Lander::new(f.clone()),
            on_the_way: VecDeque::from([(0.0, 0.0), (0.0, 0.0)]),
            held: (0.0, 0.0),
            now: 0.0,
            out: Vec::new(),
            watch: Watch::default(),
            frame: Frame::default(),
        };
        flight.look();
        flight
    }

    /// Reads the lander, as the world gives the window it -- its inputs
    /// and its outputs -- and has the window watch it.
    fn look(&mut self) {
        let mut out: Vec<f64> = (0..TERMINALS.len())
            .map(|t| self.lander.output(t))
            .collect();
        out[THROTTLE_X] = self.held.0;
        out[THROTTLE_Y] = self.held.1;
        self.frame = self.watch.frame(&self.f, &out, self.now, self.now);
        self.out = out;
    }

    /// Flies a step.
    ///
    /// # Arguments
    ///
    /// * `axes` - The throttles commanded now, X and Y, from -1 to 1.
    fn step(&mut self, axes: (f64, f64)) {
        self.on_the_way.push_back(axes);
        let (x, y) = self.on_the_way.pop_front().expect("two steps held");
        self.held = (x, y);
        let mut inputs = vec![0.0; TERMINALS.len()];
        inputs[THROTTLE_X] = x;
        inputs[THROTTLE_Y] = y;
        self.lander.hold(&inputs);
        let to = self.now + STEP;
        let mut at = self.now;
        while let Some(event) =
            self.lander.next_event(at).filter(|event| *event <= to)
        {
            self.lander.advance(at, event);
            self.lander.event(event);
            at = event;
        }
        self.lander.advance(at, to);
        self.now = to;
        self.look();
    }

    /// Lets time pass, the throttles let go.
    ///
    /// # Arguments
    ///
    /// * `seconds` - How long.
    fn wait(&mut self, seconds: f64) {
        let until = self.now + seconds;
        while self.now < until - 1e-9 {
            self.step((0.0, 0.0));
        }
    }

    /// Whether the flight is over: the lander landed or crashed, or given
    /// up on.
    ///
    /// # Returns
    ///
    /// True once it is.
    fn over(&self) -> bool {
        self.out[LANDED] > 0.5 || self.out[CRASHED] > 0.5 || self.now > LONGEST
    }

    /// How high the lander's feet are over the pad.
    ///
    /// # Returns
    ///
    /// The height, in metres.
    fn over_the_pad(&self) -> f64 {
        self.out[Y] - self.f.ground[self.f.pad].1
    }

    /// A hand flying the lander too fast towards its pad: Autoland's
    /// steering, which brings it over the pad, but with the engine eased off
    /// once it is low, so that it comes down faster than Autoland would.
    ///
    /// # Arguments
    ///
    /// * `pilot` - The autopilot it steers by.
    /// * `ease`  - What share of the autopilot's push up the hand gives,
    ///             once low.
    ///
    /// # Returns
    ///
    /// The throttles, X and Y.
    fn eased(&self, pilot: &mut Autopilot, ease: f64) -> (f64, f64) {
        let (x, y) = pilot.steer(&self.f, &self.out);
        if self.over_the_pad() < LOW {
            (x, y * ease)
        } else {
            (x, y)
        }
    }
}

/// The lander's figures, from a simulator file.
///
/// # Arguments
///
/// * `sim_path` - The simulator file: the lander example's,
///                `examples/lander-common/lander-sim.yaml`.
///
/// # Returns
///
/// The figures of the first lander in its world.
///
/// # Panics
///
/// Where the file is refused or its world has no lander.
pub fn figures(sim_path: &Path) -> Resolved {
    let sim_file = SimConfigFile::load(sim_path)
        .unwrap_or_else(|e| panic!("{}: {e}", sim_path.display()));
    let model = sim_file
        .physical()
        .expect("its world resolves")
        .expect("it has a world");
    model
        .components
        .iter()
        .find_map(|c| match &c.what {
            Resolution::Lander(f) => Some(f.clone()),
            _ => None,
        })
        .expect("its world has a lander")
}

/// Flies by hand, low over the pad, until it comes down at "close" to the
/// touchdown limit.
///
/// # Arguments
///
/// * `f` - The lander's figures.
///
/// # Returns
///
/// The frame then, and where the hand holds the joystick, X and Y.
///
/// # Panics
///
/// Where it never comes down close to the limit while low.
pub fn flying(f: &Resolved) -> (Frame, (f64, f64)) {
    let mut flight = Flight::new(f);
    let mut pilot = Autopilot::default();
    while !flight.over() {
        let axes = flight.eased(&mut pilot, EASED);
        flight.step(axes);
        if flight.over_the_pad() < LOW
            && flight.frame.touchdown_y == Touchdown::Close
        {
            return (flight.frame, axes);
        }
    }
    panic!("never close to the limit while low: {:?}", flight.out);
}

/// Flies by hand, cutting the engine once low, until a second after it
/// hits.
///
/// # Arguments
///
/// * `f` - The lander's figures.
///
/// # Returns
///
/// The frame then.
///
/// # Panics
///
/// Where it does not crash.
pub fn crashed(f: &Resolved) -> Frame {
    let mut flight = Flight::new(f);
    let mut pilot = Autopilot::default();
    while !flight.over() {
        let axes = flight.eased(&mut pilot, 0.0);
        flight.step(axes);
    }
    assert!(flight.out[CRASHED] > 0.5, "no crash: {:?}", flight.out);
    // A step past the second, so that the steps' sum is past it too.
    flight.wait(1.0 + STEP);
    flight.frame
}

/// Flies by Autoland until three seconds after it lands.
///
/// # Arguments
///
/// * `f` - The lander's figures.
///
/// # Returns
///
/// The frame then, and the landing's rating as tcs-sim's pane says it.
///
/// # Panics
///
/// Where it does not land.
pub fn landed(f: &Resolved) -> (Frame, String) {
    let mut flight = Flight::new(f);
    let mut pilot = Autopilot::default();
    while !flight.over() {
        let axes = pilot.steer(&flight.f, &flight.out);
        flight.step(axes);
    }
    assert!(flight.out[LANDED] > 0.5, "no landing: {:?}", flight.out);
    let from = rating::State {
        at: f.start,
        velocity: (0.0, 0.0),
        mass: rating::mass_with(f, 1.0),
    };
    let share =
        rating::rating(f, &from, rating::mass_with(f, flight.out[FUEL]));
    flight.wait(3.0 + STEP);
    let said = format!("Efficiency {:.0}% w/ Autoland", share * 100.0);
    (flight.frame, said)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped lander example's three flights each reach their moment:
    /// close while low, crashed too fast, and landed with the pad counting
    /// down.
    #[test]
    fn the_flights_reach_their_moments() {
        let f = figures(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../examples/lander-common/lander-sim.yaml"),
        );
        let (frame, _) = flying(&f);
        assert_eq!(frame.touchdown_y, Touchdown::Close);
        let frame = crashed(&f);
        assert_eq!(frame.touchdown_y, Touchdown::TooFast);
        assert_eq!(frame.countdown, Some(4));
        let (frame, rating) = landed(&f);
        assert_eq!(frame.countdown, Some(7));
        assert!(rating.ends_with("% w/ Autoland"), "{rating}");
    }
}
