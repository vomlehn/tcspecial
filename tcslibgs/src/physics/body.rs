//! A body: a mass falling under gravity, pushed by a force, losing the mass
//! it burns, until it reaches the ground.
//!
//! It takes the `force` pushing it up, in newtons, and the `flow` of mass it
//! loses, in kilograms a second, each the sum of what is joined to it: an
//! engine's thrust and the fuel it burns. It gives out its `altitude`, in
//! metres, its `velocity`, in metres a second, up being more than nought,
//! and `contact`, a decision asserted while it is on the ground.
//!
//! # The arithmetic
//!
//! With the force and the flow held, its mass falls in a straight line and
//! its acceleration is the force over that mass, less gravity:
//!
//! ```text
//! m(t) = m - q t
//! v(t) = v - (F / q) ln(1 - q t / m) - g t
//! x(t) = x + v t - g t^2 / 2 + (F m / q^2) h(q t / m)
//! h(u) = (1 - u) ln(1 - u) + u
//! ```
//!
//! which is the rocket equation with gravity, and exact. Where it meets the
//! ground has no closed form, so it is found: the moment is the body's next
//! event, and the world stops there, sets it down and asserts `contact`. On
//! the ground it stays until its force lifts it. The arithmetic is tcssim's
//! `physics::body`.

use serde::Deserialize;

use super::setting::{positive, quantity};
use super::terminal::{Kind, Terminal};
use crate::unit::Unit;

/// Its terminals.
pub const TERMINALS: &[Terminal] = &[
    Terminal::input("force", Kind::Value(Unit::Newton)).summing(),
    Terminal::input("flow", Kind::Value(Unit::KilogramPerSecond)).summing(),
    Terminal::output("altitude", Kind::Value(Unit::Metre)),
    Terminal::output("velocity", Kind::Value(Unit::MetrePerSecond)),
    Terminal::output("contact", Kind::Decision),
];

/// Everything it states, by the name a file writes, `name` and `type`
/// apart: the one list, from which an editor builds its form.
pub const SETTINGS: &[&str] = &["mass", "gravity", "altitude", "velocity"];

/// A body, as a physics model states it.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Body {
    /// What it is called, which is how a connection or a stand-in names it.
    pub name: String,
    /// Its mass at the start, fuel and all: `"1500 kg"`.
    pub mass: String,
    /// The gravity it falls in, in metres a second a second: `"1.62 m/s2"`,
    /// the Moon's.
    pub gravity: String,
    /// How high it starts: `"500 m"`. Nought starts it on the ground.
    pub altitude: String,
    /// How fast it starts, up being more than nought: `"-20 m/s"`.
    pub velocity: String,
}

/// A body with its figures read.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Resolved {
    /// Kilograms.
    pub mass: f64,
    /// Metres a second a second.
    pub gravity: f64,
    /// Metres.
    pub altitude: f64,
    /// Metres a second, up more than nought.
    pub velocity: f64,
}

impl Body {
    /// Reads its figures.
    ///
    /// # Returns
    ///
    /// Them.
    ///
    /// # Errors
    ///
    /// What is wrong with the first that does not read, naming it, or an
    /// altitude below the ground.
    pub fn resolve(&self) -> Result<Resolved, String> {
        let altitude = quantity("altitude", &self.altitude, Some("m"))?;
        if altitude < 0.0 {
            return Err(format!(
                "altitude: {altitude} m is below the ground, which is at \
                 nought"
            ));
        }
        Ok(Resolved {
            mass: positive("mass", &self.mass, Some("kg"))?,
            gravity: positive("gravity", &self.gravity, Some("m/s2"))?,
            altitude,
            velocity: quantity("velocity", &self.velocity, Some("m/s"))?,
        })
    }
}
