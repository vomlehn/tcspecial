//! How efficiently a landing was flown: the least propellant any pilot
//! could have landed with, from where the visitor took the lander, over what
//! the visitor used.
//!
//! A landing from high up and far away needs more propellant than one from
//! just over the pad, so propellant alone says more about where a flight
//! began than about how it was flown. Each landing is measured against the
//! best that could have been done from its own start instead: the lander's
//! place, its speeds and its mass at the moment the visitor took it. The
//! rating is that best over what was used, so a perfect landing is 100 %,
//! and two visitors who started in different places are measured alike.
//!
//! Propellant is counted as the change in velocity it buys. The lander burns
//! fuel in proportion to its thrust along each axis, `|Fx| + |Fy|`, so the
//! mass falls as `m = m0 exp(-D / ve)`, where `D` is the integral of
//! `|ax| + |ay|` and `ve` the exhaust velocity: the propellant used and `D`
//! say the same thing, and `D` does not depend on how full the tank was.
//!
//! The best is a lower bound, worked out under physics more generous than
//! the lander's, so that no landing can be rated over 100 %:
//!
//! - thrust along X is taken as instant, any change of sideways speed being
//!   made at once;
//! - thrust along Y is no more than its full thrust over the least mass the
//!   best flight could end with -- no less than the visitor's lander had at
//!   touchdown, the best flight burning no more;
//! - the terrain is ignored, the lander needing only to reach the pad.
//!
//! What it must still do is what the lander must: come down onto the pad,
//! over it end to end, no faster than its landing speed and drifting no
//! faster than its landing drift. For a landing in a time `T` the two axes
//! are apart, the cost along each depending only on `T`; the best over every
//! `T` is the bound.

use tcsprotocol::node::physics::lander::{Point, Resolved};

use crate::node::physics::lander::STANDARD_GRAVITY;

/// How long, at most, a landing is looked for over, in seconds: far longer
/// than any lander could hover for, hovering costing gravity each second.
const LONGEST: f64 = 600.0;

/// How many landing times are tried, spread over the times up to
/// [`LONGEST`], closer together where landings are quick.
const TRIES: usize = 20_000;

/// The lander at a moment: where the place between its feet is, how fast
/// it is going, and its mass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct State {
    /// Where the place between its feet is, in metres.
    pub at: Point,
    /// Its velocity along X and Y, in metres a second, positive up.
    pub velocity: Point,
    /// Its mass, dry and the fuel it has, in kilograms.
    pub mass: f64,
}

/// The lander's mass when it has a share of a full tank.
///
/// # Arguments
///
/// * `figures` - The lander's figures.
/// * `share`   - The share of a full tank it has, from nought to one.
///
/// # Returns
///
/// Its mass, in kilograms.
pub fn mass_with(figures: &Resolved, share: f64) -> f64 {
    figures.dry_mass + share.clamp(0.0, 1.0) * figures.fuel
}

/// How efficiently a landing was flown.
///
/// # Arguments
///
/// * `figures`   - The lander's figures.
/// * `from`      - The lander as the visitor took it.
/// * `touchdown` - Its mass as it landed, in kilograms.
///
/// # Returns
///
/// The rating, from nought to one: the least propellant that could have
/// landed it from `from`, over what was used. One where nothing was used,
/// a lander that needed no thrust having been flown as well as it could be.
pub fn rating(figures: &Resolved, from: &State, touchdown: f64) -> f64 {
    let exhaust = figures.isp.as_secs_f64() * STANDARD_GRAVITY;
    let used = exhaust * (from.mass / touchdown.max(f64::MIN_POSITIVE)).ln();
    if used <= 1e-9 {
        return 1.0;
    }
    let up = figures.thrust_y / touchdown.max(f64::MIN_POSITIVE);
    (least(figures, from, up) / used).clamp(0.0, 1.0)
}

/// The least change of velocity, `|ax| + |ay|` over the flight, any landing
/// from a state could be made with: see the module.
///
/// # Arguments
///
/// * `figures` - The lander's figures: its pad, its outline, its limits.
/// * `from`    - Where it is, and how fast it is going.
/// * `up`      - The most it can speed up by along Y, in metres a second a
///               second, gravity not counted.
///
/// # Returns
///
/// The least, in metres a second; nought where it has landed already.
pub fn least(figures: &Resolved, from: &State, up: f64) -> f64 {
    let (x0, x1, y0, _) = figures.extent();
    let ((pad0, pad_y), (pad1, _)) = figures.pad_ends();
    // Where the place between its feet may be at touchdown, across, and how
    // far its lowest point is above the pad.
    let across = (pad0 - x0 - from.at.0, pad1 - x1 - from.at.0);
    let height = from.at.1 + y0 - pad_y;
    (1..=TRIES)
        .map(|n| {
            // Squared, so that the short times, where the answer usually
            // is, are tried closest together.
            let share = n as f64 / TRIES as f64;
            LONGEST * share * share
        })
        .filter_map(|t| {
            let down = vertical(
                height,
                from.velocity.1,
                figures.gravity,
                up,
                figures.landing_speed,
                t,
            )?;
            Some(
                down + sideways(
                    across,
                    from.velocity.0,
                    figures.landing_drift,
                    t,
                ),
            )
        })
        .fold(f64::INFINITY, f64::min)
}

/// The least change of velocity along X to arrive over the pad at a time,
/// drifting no faster than the landing drift, thrust along X being instant:
/// a change at once to the speed that carries it there, and another at the
/// end into the drift allowed.
///
/// # Arguments
///
/// * `across` - How far the place between its feet may be moved, least and
///              most, to be over the pad, in metres.
/// * `speed`  - Its speed along X now, in metres a second.
/// * `drift`  - The fastest it may drift at touchdown.
/// * `t`      - The time it lands at, in seconds.
///
/// # Returns
///
/// The change, in metres a second.
fn sideways(across: (f64, f64), speed: f64, drift: f64, t: f64) -> f64 {
    let (lo, hi) = (across.0 / t, across.1 / t);
    // The cost, for a coasting speed, of changing to it and then into the
    // drift allowed: convex, and least at one of these speeds.
    let cost = |v: f64| (v - speed).abs() + (v.abs() - drift).max(0.0);
    [
        lo,
        hi,
        speed.clamp(lo, hi),
        drift.clamp(lo, hi),
        (-drift).clamp(lo, hi),
    ]
    .into_iter()
    .map(cost)
    .fold(f64::INFINITY, f64::min)
}

/// The least change of velocity along Y to come down onto the pad at a
/// time, no faster than the landing speed, by thrust up of no more than
/// `up` against gravity.
///
/// The change, `I`, is fixed by the speed it lands at, `v`: `I = v - v0 +
/// g t`, landing faster costing less. Where it lands is fixed by when the
/// thrust is spent: all of it at the end leaves it lowest, `I^2 / 2 up`
/// above where it would have fallen to, and all of it at the start highest,
/// `I t - I^2 / 2 up` above. It lands at the pad's height where that lies
/// between the two, and its thrust fits within the time, `I <= up t`.
///
/// # Arguments
///
/// * `height`  - How far its lowest point is above the pad, in metres.
/// * `speed`   - Its speed along Y now, positive up.
/// * `gravity` - The Moon's gravity, positive down.
/// * `up`      - The most its thrust can speed it up by.
/// * `landing` - The fastest it may come down at.
/// * `t`       - The time it lands at, in seconds.
///
/// # Returns
///
/// The change, in metres a second; none where it cannot land at that time.
fn vertical(
    height: f64,
    speed: f64,
    gravity: f64,
    up: f64,
    landing: f64,
    t: f64,
) -> Option<f64> {
    if up <= 0.0 {
        return None;
    }
    // How far above where it would fall to the pad must be reached by
    // thrust.
    let lift = -(height + speed * t - gravity * t * t / 2.0);
    if lift < 0.0 {
        return None;
    }
    // The least change for which the earliest thrust lifts it far enough,
    // and the most for which the latest does not lift it too far.
    let reach = up * up * t * t - 2.0 * up * lift;
    if reach < 0.0 {
        return None;
    }
    let lo = (up * t - reach.sqrt()).max(0.0);
    let hi = (2.0 * up * lift).sqrt().min(up * t);
    // And the changes the landing speeds allowed give: down at no more
    // than the landing speed, and not up.
    let fastest = -landing - speed + gravity * t;
    let slowest = -speed + gravity * t;
    let least = lo.max(fastest);
    (least <= hi.min(slowest) + 1e-9).then_some(least)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcsos::time::Duration;

    /// A lander over flat ground with a pad from -10 m to 10 m at nought:
    /// a 4 m square outline about the place between its feet, the Moon's
    /// gravity, and the Apollo module's limits.
    ///
    /// # Returns
    ///
    /// Its figures.
    fn figures() -> Resolved {
        Resolved {
            dry_mass: 2000.0,
            fuel: 1000.0,
            isp: Duration::from_secs(300),
            thrust_x: 5000.0,
            thrust_y: 15000.0,
            gravity: 1.62,
            start: (0.0, 100.0),
            width: 400.0,
            height: 200.0,
            ground: vec![
                (-200.0, 0.0),
                (-10.0, 0.0),
                (10.0, 0.0),
                (200.0, 0.0),
            ],
            pad: 1,
            outline: vec![vec![
                (-2.0, 0.0),
                (2.0, 0.0),
                (2.0, 4.0),
                (-2.0, 4.0),
            ]],
            landing_speed: 3.05,
            landing_drift: 1.22,
            pause: Duration::from_secs(3),
            pad_wait: Duration::from_secs(10),
        }
    }

    /// Falling straight down onto the pad from rest, the least is the
    /// suicide burn's: falling freely, then full thrust at the last moment
    /// to come down at the landing speed, which `v_b^2 (1/g + 1/(A-g)) =
    /// 2h + V^2/(A-g)` and `I = A (v_b - V) / (A - g)` give.
    #[test]
    fn a_fall_straight_down_costs_the_suicide_burn() {
        let f = figures();
        let up = 4.0;
        let (g, v, h) = (f.gravity, f.landing_speed, 100.0);
        let net = up - g;
        let vb = ((2.0 * h + v * v / net) / (1.0 / g + 1.0 / net)).sqrt();
        let burn = up * (vb - v) / net;
        let got = least(
            &f,
            &State {
                at: (0.0, h),
                velocity: (0.0, 0.0),
                mass: 3000.0,
            },
            up,
        );
        assert!((got - burn).abs() < 0.05 * burn, "{got} against {burn}");
    }

    /// Standing on the pad already, still, there is nothing to spend.
    #[test]
    fn a_lander_on_the_pad_needs_nothing() {
        let got = least(
            &figures(),
            &State {
                at: (0.0, 0.0),
                velocity: (0.0, 0.0),
                mass: 3000.0,
            },
            4.0,
        );
        // Nothing but the gravity of the shortest time tried.
        assert!(got < 1e-3, "{got}");
    }

    /// Further away, or higher up, never costs less.
    #[test]
    fn further_never_costs_less() {
        let f = figures();
        let at = |x: f64, y: f64| State {
            at: (x, y),
            velocity: (0.0, 0.0),
            mass: 3000.0,
        };
        let cost = |s: State| least(&f, &s, 4.0);
        assert!(cost(at(0.0, 200.0)) >= cost(at(0.0, 100.0)));
        assert!(cost(at(150.0, 100.0)) >= cost(at(0.0, 100.0)));
        assert!(cost(at(150.0, 100.0)) > cost(at(0.0, 100.0)) + 0.1);
    }

    /// A rating is a share: one for the least, less for more, never more
    /// than one; and one where nothing was burnt.
    #[test]
    fn a_rating_is_a_share() {
        let f = figures();
        let from = State {
            at: (0.0, 100.0),
            velocity: (0.0, 0.0),
            mass: 3000.0,
        };
        let exhaust = f.isp.as_secs_f64() * STANDARD_GRAVITY;
        let after = |dv: f64| from.mass / (dv / exhaust).exp();
        let up = |m: f64| f.thrust_y / m;
        let best = least(&f, &from, up(after(30.0)));
        let wasteful = rating(&f, &from, after(4.0 * best));
        assert!((0.2..0.3).contains(&wasteful), "{wasteful}");
        assert!(rating(&f, &from, after(best / 2.0)) <= 1.0);
        assert_eq!(rating(&f, &from, from.mass), 1.0);
    }
}
