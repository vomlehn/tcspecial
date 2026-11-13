//! A body: a mass falling under gravity, pushed by a force, losing the mass
//! it burns, until it reaches the ground.
//!
//! Its terminals are `tcslib_protocol::node::physics::body`'s: `force`, in
//! newtons, and
//! `flow`, in kilograms a second, each the sum of what is joined to it;
//! `altitude`, in metres; `velocity`, in metres a second, up more than
//! nought; and `contact`, a decision asserted while it is on the ground.
//!
//! # The arithmetic
//!
//! With the force `F` and the flow `q` held, its mass falls in a straight
//! line, `m - q t`, and the rocket equation with gravity has a closed form:
//!
//! ```text
//! v(t) = v - (F / q) ln(1 - u) - g t
//! x(t) = x + v t - g t^2 / 2 + (F m / q^2) h(u)
//! u    = q t / m
//! h(u) = (1 - u) ln(1 - u) + u
//! ```
//!
//! which is exact, and is what this advances by. `h` is taken by its series
//! where `u` is small, where the two terms of its closed form are nearly
//! equal and the difference between them would be lost; as the flow goes to
//! nought both forms go to the constant acceleration's, `F t^2 / 2m`.
//!
//! # The ground
//!
//! Where `x(t)` reaches nought has no closed form, so it is found, and the
//! moment is the body's next event. Found by stepping that can never step
//! past it: with the force and the flow held, the acceleration
//! `F / (m - q t) - g` only grows, so the one it has now is the least it
//! will have, and the earliest it could reach the ground is where it would
//! with that acceleration held, which is a quadratic's root. Step there,
//! and again from there, and each step lands short of the ground or on it,
//! closing on it from above until it is within a micrometre. The world
//! stops at that moment, and the body is set down: altitude and velocity
//! nought, `contact` asserted. It stays down until its force lifts it.

use tcslib_protocol::node::physics::body::Resolved;

use super::component::Component;

/// Its terminals' places, in
/// `tcslib_protocol::node::physics::body::TERMINALS`'s order, which a test
/// holds this to.
const FORCE: usize = 0;
const FLOW: usize = 1;
const ALTITUDE: usize = 2;
const VELOCITY: usize = 3;
const CONTACT: usize = 4;

/// How close to the ground, in metres, the search for touchdown has to come
/// before it calls it touchdown: a micrometre, well inside what any
/// altimeter resolves.
const CLOSE_ENOUGH: f64 = 1e-6;

/// The most steps the search for touchdown takes. Each closes on the ground
/// by much more than half of what is left, so this is never reached by a
/// descent; it bounds the work for one that skims the ground forever.
const MOST_STEPS: usize = 200;

/// The least mass a body is let fall to, in kilograms. A body joined to an
/// engine whose fuel weighs more than it does is a model written wrongly,
/// and its mass would reach nought and its acceleration be infinite; this
/// keeps the arithmetic finite while the fault is found.
const LEAST_MASS: f64 = 1.0;

/// A body of the world.
#[derive(Debug)]
pub struct Body {
    /// Metres a second a second.
    gravity: f64,
    /// Kilograms.
    mass: f64,
    /// Metres.
    altitude: f64,
    /// Metres a second, up more than nought.
    velocity: f64,
    /// Whether it is on the ground.
    contact: bool,
    /// The force on it and the mass it loses, as last held.
    force: f64,
    flow: f64,
}

/// `h(u) = (1 - u) ln(1 - u) + u`, by its series where `u` is small.
///
/// # Arguments
///
/// * `u` - The fraction of the mass lost, from nought up to but not one.
///
/// # Returns
///
/// `h(u)`.
fn h(u: f64) -> f64 {
    if u < 1e-4 {
        // u^2/2 + u^3/6 + u^4/12: the next term, u^5/20, is below a
        // double's resolution of the first here.
        u * u * (0.5 + u * (1.0 / 6.0 + u / 12.0))
    } else {
        (1.0 - u) * (-u).ln_1p() + u
    }
}

impl Body {
    /// Makes a body where its figures say it starts: on the ground, if it
    /// starts at nought altitude going nowhere but down.
    ///
    /// # Arguments
    ///
    /// * `figures` - What the model says of it.
    ///
    /// # Returns
    ///
    /// The body, with nothing on it.
    pub fn new(figures: Resolved) -> Body {
        let down = figures.altitude <= 0.0 && figures.velocity <= 0.0;
        Body {
            gravity: figures.gravity,
            mass: figures.mass,
            altitude: figures.altitude.max(0.0),
            velocity: if down { 0.0 } else { figures.velocity },
            contact: down,
            force: 0.0,
            flow: 0.0,
        }
    }

    /// The flow it loses mass at over an interval, slowed where it would
    /// take the body below [`LEAST_MASS`].
    ///
    /// # Arguments
    ///
    /// * `seconds` - The interval.
    ///
    /// # Returns
    ///
    /// Kilograms a second.
    fn flow_over(&self, seconds: f64) -> f64 {
        if seconds > 0.0 && self.mass - self.flow * seconds < LEAST_MASS {
            ((self.mass - LEAST_MASS) / seconds).max(0.0)
        } else {
            self.flow
        }
    }

    /// Where it would be, and how fast, after a time in flight.
    ///
    /// # Arguments
    ///
    /// * `t` - Seconds from now.
    ///
    /// # Returns
    ///
    /// Its altitude and velocity then.
    fn flown(&self, t: f64) -> (f64, f64) {
        let (x, v, m, g, f) = (
            self.altitude,
            self.velocity,
            self.mass,
            self.gravity,
            self.force,
        );
        let q = self.flow_over(t);
        let u = q * t / m;
        let (pushed_x, pushed_v) = if u < 1e-12 {
            // No mass lost to speak of: constant acceleration.
            (f * t * t / (2.0 * m), f * t / m)
        } else {
            (f * m / (q * q) * h(u), -(f / q) * (-u).ln_1p())
        };
        (x + v * t - g * t * t / 2.0 + pushed_x, v - g * t + pushed_v)
    }

    /// Its acceleration after a time in flight, up more than nought.
    ///
    /// # Arguments
    ///
    /// * `t` - Seconds from now.
    ///
    /// # Returns
    ///
    /// Metres a second a second.
    fn acceleration(&self, t: f64) -> f64 {
        let mass = (self.mass - self.flow_over(t) * t).max(LEAST_MASS);
        self.force / mass - self.gravity
    }
}

/// The least time after which something at `x`, going at `v` and
/// accelerating at no less than `a`, could reach nought.
///
/// # Arguments
///
/// * `x` - Its altitude, more than nought.
/// * `v` - Its velocity, up more than nought.
/// * `a` - The least its acceleration will be, up more than nought.
///
/// # Returns
///
/// The time, or `None` where with that acceleration it never would.
fn soonest(x: f64, v: f64, a: f64) -> Option<f64> {
    // x + v t + a t^2 / 2 = 0, by the form of the roots that loses nothing
    // to cancellation.
    if a == 0.0 {
        return (v < 0.0).then(|| x / -v);
    }
    let discriminant = v * v - 2.0 * a * x;
    if discriminant < 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    let sign = if v >= 0.0 { 1.0 } else { -1.0 };
    let k = -(v + sign * root) / 2.0;
    let mut roots = [k / (a / 2.0), if k != 0.0 { x / k } else { f64::NAN }];
    roots.sort_by(f64::total_cmp);
    roots.into_iter().find(|t| *t > 0.0 && t.is_finite())
}

impl Component for Body {
    fn hold(&mut self, inputs: &[f64]) {
        self.force = inputs[FORCE];
        self.flow = inputs[FLOW].max(0.0);
        // Lifted off the ground by a force that beats its weight.
        if self.contact && self.force > self.mass * self.gravity {
            self.contact = false;
        }
    }

    fn advance(&mut self, from: f64, to: f64) {
        let seconds = to - from;
        if seconds <= 0.0 {
            return;
        }
        let q = self.flow_over(seconds);
        if !self.contact {
            let (x, v) = self.flown(seconds);
            if x < 0.0 {
                // Past the ground, which the search for touchdown is there
                // to prevent, and so a fault to put right rather than a
                // body to leave underground.
                self.altitude = 0.0;
                self.velocity = 0.0;
                self.contact = true;
            } else {
                self.altitude = x;
                self.velocity = v;
            }
        }
        self.mass -= q * seconds;
    }

    fn output(&self, terminal: usize) -> f64 {
        match terminal {
            ALTITUDE => self.altitude,
            VELOCITY => self.velocity,
            CONTACT if self.contact => 1.0,
            _ => 0.0,
        }
    }

    fn next_event(&self, now: f64) -> Option<f64> {
        if self.contact {
            return None;
        }
        let mut t = 0.0;
        for _ in 0..MOST_STEPS {
            let (x, v) = self.flown(t);
            if x <= CLOSE_ENOUGH && v < 0.0 {
                return Some(now + t);
            }
            // The acceleration now is the least it will be from here on.
            t += soonest(x.max(0.0), v, self.acceleration(t))?;
        }
        Some(now + t)
    }

    fn event(&mut self, _at: f64) {
        self.altitude = 0.0;
        self.velocity = 0.0;
        self.contact = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslib_protocol::node::physics::body::TERMINALS;

    /// The places here are the type's.
    #[test]
    fn the_terminals_are_the_types() {
        assert_eq!(TERMINALS[FORCE].name, "force");
        assert_eq!(TERMINALS[FLOW].name, "flow");
        assert_eq!(TERMINALS[ALTITUDE].name, "altitude");
        assert_eq!(TERMINALS[VELOCITY].name, "velocity");
        assert_eq!(TERMINALS[CONTACT].name, "contact");
    }

    /// A body on the Moon at a hundred metres, falling at ten metres a
    /// second.
    ///
    /// # Returns
    ///
    /// The body, of fifteen hundred kilograms, with nothing pushing it.
    fn falling() -> Body {
        Body::new(Resolved {
            mass: 1500.0,
            gravity: 1.62,
            altitude: 100.0,
            velocity: -10.0,
        })
    }

    /// Falling freely, it lands when the quadratic says, and is set down.
    #[test]
    fn a_falling_body_lands_when_the_quadratic_says() {
        let mut body = falling();
        let landed = body.next_event(0.0).expect("it lands");
        // 100 - 10 t - 0.81 t^2 = 0.
        let exact = (-10.0 + (100.0f64 + 4.0 * 0.81 * 100.0).sqrt()) / 1.62;
        assert!((landed - exact).abs() < 1e-6, "{landed} against {exact}");
        body.advance(0.0, landed);
        body.event(landed);
        assert_eq!(body.output(CONTACT), 1.0);
        assert_eq!(body.output(ALTITUDE), 0.0);
        assert_eq!(body.next_event(landed), None);
    }

    /// Its closed form burning fuel agrees with integrating the rocket
    /// equation numerically.
    #[test]
    fn a_burning_body_is_advanced_exactly() {
        let mut body = falling();
        body.hold(&[3000.0, 2.0, 0.0, 0.0, 0.0]);
        let (x, v) = body.flown(8.0);
        // Fourth-order Runge-Kutta, a thousandth of a second at a time.
        let slope =
            |t: f64, s: [f64; 2]| [s[1], 3000.0 / (1500.0 - 2.0 * t) - 1.62];
        let (mut t, mut s, step) = (0.0, [100.0, -10.0], 0.001);
        while t < 8.0 - 1e-9 {
            let k1 = slope(t, s);
            let k2 = slope(
                t + step / 2.0,
                [s[0] + step / 2.0 * k1[0], s[1] + step / 2.0 * k1[1]],
            );
            let k3 = slope(
                t + step / 2.0,
                [s[0] + step / 2.0 * k2[0], s[1] + step / 2.0 * k2[1]],
            );
            let k4 =
                slope(t + step, [s[0] + step * k3[0], s[1] + step * k3[1]]);
            for i in 0..2 {
                s[i] +=
                    step / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
            }
            t += step;
        }
        assert!((x - s[0]).abs() < 1e-7, "{x} against {}", s[0]);
        assert!((v - s[1]).abs() < 1e-9, "{v} against {}", s[1]);
        body.advance(0.0, 8.0);
        assert!((body.mass - 1484.0).abs() < 1e-9);
    }

    /// Braking, it lands where its closed form reaches the ground, and the
    /// search stops within a micrometre of it.
    #[test]
    fn a_braking_body_lands_where_its_closed_form_meets_the_ground() {
        let mut body = falling();
        // Less than its weight: it still lands, more slowly.
        body.hold(&[2000.0, 1.0, 0.0, 0.0, 0.0]);
        let landed = body.next_event(0.0).expect("it lands");
        let (x, v) = body.flown(landed);
        assert!((0.0..=CLOSE_ENOUGH).contains(&x), "{x}");
        assert!(v < 0.0);
        // A hundredth of a second sooner it was still above the ground.
        assert!(body.flown(landed - 0.01).0 > 0.0);
    }

    /// A body braked to a hover never lands, and a body on the ground is
    /// lifted only by a force that beats its weight.
    #[test]
    fn it_lands_only_falling_and_lifts_only_pushed() {
        let mut body = falling();
        body.hold(&[1500.0 * 1.62 * 2.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(body.next_event(0.0), None, "it is going up for ever");

        let mut down = Body::new(Resolved {
            mass: 1000.0,
            gravity: 1.62,
            altitude: 0.0,
            velocity: 0.0,
        });
        assert_eq!(down.output(CONTACT), 1.0);
        down.hold(&[1000.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(down.output(CONTACT), 1.0, "less than its weight");
        down.hold(&[2000.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(down.output(CONTACT), 0.0, "lifted");
    }
}
