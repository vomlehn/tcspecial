//! An engine: a throttle, a thrust, and the fuel it burns.
//!
//! It takes `throttle`, a ratio from nought to one, which a stand-in the
//! vehicle drives sets, and gives out its `force`, in newtons -- its full
//! `thrust` times the throttle -- and the `flow` of fuel it burns doing so,
//! in kilograms a second. An engine of specific impulse `isp` burns
//!
//! ```text
//! flow = force / (isp * 9.80665)
//! ```
//!
//! and has `fuel` to burn. When it is gone the engine gives out nothing,
//! whatever it is told: a moment foreseen from the flow, and an event of the
//! world's. The arithmetic is tcssim's `physics::engine`.

use std::time::Duration;

use serde::Deserialize;

use super::setting::{positive, time_of};
use super::terminal::{Kind, Terminal};
use crate::unit::Unit;

/// Its terminals.
pub const TERMINALS: &[Terminal] = &[
    Terminal::input("throttle", Kind::Value(Unit::Ratio)).must_be_joined(),
    Terminal::output("force", Kind::Value(Unit::Newton)),
    Terminal::output("flow", Kind::Value(Unit::KilogramPerSecond)),
];

/// Everything it states, by the name a file writes, `name` and `type`
/// apart: the one list, from which an editor builds its form.
pub const SETTINGS: &[&str] = &["thrust", "isp", "fuel"];

/// An engine, as a physics model states it.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Engine {
    /// What it is called, which is how a connection or a stand-in names it.
    pub name: String,
    /// Its thrust at full throttle: `"16000 N"`.
    pub thrust: String,
    /// Its specific impulse, as a length of time: `"311 s"`.
    pub isp: String,
    /// The fuel it starts with: `"800 kg"`.
    pub fuel: String,
}

/// An engine with its figures read.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Resolved {
    /// Newtons, at full throttle.
    pub thrust: f64,
    pub isp: Duration,
    /// Kilograms.
    pub fuel: f64,
}

impl Engine {
    /// Reads its figures.
    ///
    /// # Returns
    ///
    /// Them.
    ///
    /// # Errors
    ///
    /// What is wrong with the first that does not read, naming it.
    pub fn resolve(&self) -> Result<Resolved, String> {
        Ok(Resolved {
            thrust: positive("thrust", &self.thrust, Some("N"))?,
            isp: time_of("isp", &self.isp)?,
            fuel: positive("fuel", &self.fuel, Some("kg"))?,
        })
    }
}
