//! A thermal zone: one body at one temperature, which heat warms and its
//! surroundings pull back.
//!
//! It takes `heat`, in watts, the sum of every heater's connected to it, and
//! gives out its `temperature`, in kelvin. Newton's cooling with that heat:
//!
//! ```text
//! dT/dt = -(T - environment) / time_constant + heat / capacity
//! ```
//!
//! Its surroundings may swing, as a sine wave of `swing` either side of
//! `environment` over `period` -- an orbit's sun and shadow -- and wander at
//! random by `wander`, over `wander_time`. The arithmetic is tcssim's
//! `physics::thermal_zone`.

use std::time::Duration;

use serde::Deserialize;

use super::setting::{positive, quantity, size_of, time_of, wander_of, Wander};
use super::terminal::{Kind, Terminal};
use crate::sim_config::blank_is_unstated;
use crate::unit::Unit;

/// Its terminals.
pub const TERMINALS: &[Terminal] = &[
    Terminal::input("heat", Kind::Value(Unit::Watt)).summing(),
    Terminal::output("temperature", Kind::Value(Unit::Kelvin)),
];

/// Everything it states, by the name a file writes, `name` and `type`
/// apart: the one list, from which an editor builds its form.
pub const SETTINGS: &[&str] = &[
    "environment",
    "time_constant",
    "capacity",
    "swing",
    "period",
    "wander",
    "wander_time",
];

/// A thermal zone, as a physics model states it.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ThermalZone {
    /// What it is called, which is how a connection or a stand-in names it.
    pub name: String,
    /// The temperature it tends towards with nothing warming it, in kelvin:
    /// `"283.15 K"`.
    ///
    /// Fixed, and not a figure that follows the vehicle's setpoint: a zone
    /// whose surroundings tracked the setpoint could never be driven away
    /// from it, and commanding the setpoint from the ground would then show
    /// nothing at all.
    pub environment: String,
    /// How quickly it loses the difference, as a length of time.
    ///
    /// The time constant of Newton's cooling: with nothing warming it, the
    /// gap between the zone and its surroundings falls to a third of itself
    /// in this long. Stated as a time rather than as a rate because the rate
    /// depends on how far from its surroundings the zone happens to be, and
    /// the constant does not.
    pub time_constant: String,
    /// How much heat it takes to warm it a kelvin, in joules a kelvin:
    /// `"20 J/K"`. What turns a heater's watts into kelvin a second.
    pub capacity: String,
    /// How far its surroundings swing either side of `environment`, in
    /// kelvin, as a sine wave. Unstated is none.
    #[serde(default, deserialize_with = "blank_is_unstated")]
    pub swing: Option<String>,
    /// How long one swing takes, as a length of time. Required with `swing`
    /// and refused without it.
    #[serde(default, deserialize_with = "blank_is_unstated")]
    pub period: Option<String>,
    /// How far its surroundings wander at random, in kelvin: the standard
    /// deviation of a departure drawn back towards nought with
    /// `wander_time` as its time constant. Unstated is none.
    #[serde(default, deserialize_with = "blank_is_unstated")]
    pub wander: Option<String>,
    /// How slowly its surroundings wander, as a length of time. Required
    /// with `wander` and refused without it.
    #[serde(default, deserialize_with = "blank_is_unstated")]
    pub wander_time: Option<String>,
}

/// A thermal zone with its figures read.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Resolved {
    /// Kelvin.
    pub environment: f64,
    pub time_constant: Duration,
    /// Joules a kelvin.
    pub capacity: f64,
    /// How far its surroundings swing either side of their middle, in
    /// kelvin.
    pub swing: f64,
    /// How long one swing takes; zero only where `swing` is.
    pub period: Duration,
    /// How its surroundings wander at random, in kelvin.
    pub wander: Wander,
}

impl ThermalZone {
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
        let swing = size_of("swing", &self.swing, Some("K"))?;
        let period = match (&self.period, self.swing.is_some()) {
            (Some(said), true) => time_of("period", said)?,
            (Some(_), false) => {
                return Err("period says how long the surroundings' swing \
                            takes, and swing is not set"
                    .to_string())
            }
            (None, true) => {
                return Err("period is not set: swing says how far the \
                            surroundings swing, and period how long a swing \
                            takes"
                    .to_string())
            }
            (None, false) => Duration::ZERO,
        };
        Ok(Resolved {
            environment: quantity("environment", &self.environment, Some("K"))?,
            time_constant: time_of("time_constant", &self.time_constant)?,
            capacity: positive("capacity", &self.capacity, Some("J/K"))?,
            swing,
            period,
            wander: wander_of(
                "wander",
                "wander_time",
                &self.wander,
                &self.wander_time,
                Some("K"),
            )?,
        })
    }
}
