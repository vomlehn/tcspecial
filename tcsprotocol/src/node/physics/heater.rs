//! A heater: a resistive element on a supply, switched on and off.
//!
//! Driven at `on` by a decision -- a thermostat node's, through a simulated
//! payload the vehicle writes to -- and giving out its `heat`, in watts, to
//! whatever it warms, and drawing its `current`, in amperes, from its supply.
//! Both are nought while it is off. `heating` says whether it is on, as a
//! decision: what a line sensing the heater reads, which is what it is doing
//! rather than what it was told -- a heater whose simulated payload is disabled
//! is told nothing, and is off.
//!
//! Its supply is not steady. Where `ripple` is stated the voltage wanders by
//! that fraction of nominal, slowly, as `ripple_time` says; a resistance's
//! power goes as the square of its voltage and its current as the voltage,
//! so both follow. The arithmetic is tcs-sim's `physics::heater`.

use serde::{Deserialize, Serialize};

use super::setting::{positive, wander_of, Wander};
use super::terminal::{Kind, Terminal};
use crate::sim_config::blank_is_unstated;
use crate::unit::Unit;

/// Its terminals.
pub const TERMINALS: &[Terminal] = &[
    Terminal::input("on", Kind::Decision).must_be_joined(),
    Terminal::output("heat", Kind::Value(Unit::Watt)),
    Terminal::output("current", Kind::Value(Unit::Ampere)),
    Terminal::output("heating", Kind::Decision),
];

/// Everything it states, by the name a file writes, the
/// `component` naming it apart: the one list, from which an editor builds
/// its form.
pub const SETTINGS: &[&str] = &["power", "voltage", "ripple", "ripple_time"];

/// A heater, as a physics model states it.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Heater {
    /// What it is called, which is how a connection or a simulated payload
    /// names it.
    #[serde(rename = "component")]
    pub name: String,
    /// What it gives out while on at its nominal voltage, in watts:
    /// `"60 W"`.
    pub power: String,
    /// Its supply's nominal voltage, in volts, which with its power says
    /// what current it draws: `"28 V"`.
    pub voltage: String,
    /// How far its supply's voltage wanders, as a fraction of nominal: the
    /// standard deviation, written bare. Unstated is none.
    #[serde(default, deserialize_with = "blank_is_unstated")]
    pub ripple: Option<String>,
    /// How slowly its supply wanders, as a length of time. Required with
    /// `ripple` and refused without it.
    #[serde(default, deserialize_with = "blank_is_unstated")]
    pub ripple_time: Option<String>,
}

/// A heater with its figures read.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Resolved {
    /// Watts, at nominal voltage.
    pub power: f64,
    /// Volts.
    pub voltage: f64,
    /// How its supply wanders, as a fraction of nominal.
    pub supply: Wander,
}

impl Heater {
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
            power: positive("power", &self.power, Some("W"))?,
            voltage: positive("voltage", &self.voltage, Some("V"))?,
            supply: wander_of(
                "ripple",
                "ripple_time",
                &self.ripple,
                &self.ripple_time,
                None,
            )?,
        })
    }
}
