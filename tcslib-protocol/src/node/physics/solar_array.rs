//! A solar array: panels that turn the light falling on them into current.
//!
//! It takes `light`, a ratio, from the sun, and gives out `current`, in
//! amperes: its `current` in full sunlight, in proportion to the light. A
//! real array's current falls a little as its bus's voltage rises; the bus
//! of a spacecraft without a regulator sits where its battery holds it, on
//! the flat of the array's curve, where the current is very nearly the
//! array's short-circuit figure. The arithmetic is tcs-sim's
//! `physics::solar_array`.

use serde::{Deserialize, Serialize};

use super::setting::positive;
use super::terminal::{Kind, Terminal};
use crate::unit::Unit;

/// Its terminals.
pub const TERMINALS: &[Terminal] = &[
    Terminal::input("light", Kind::Value(Unit::Ratio)).must_be_joined(),
    Terminal::output("current", Kind::Value(Unit::Ampere)),
];

/// Everything it states, by the name a file writes, the
/// `component` naming it apart: the one list, from which an editor builds
/// its form.
pub const SETTINGS: &[&str] = &["current"];

/// A solar array, as a physics model states it.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SolarArray {
    /// What it is called, which is how a connection or a simulated payload
    /// names it.
    #[serde(rename = "component")]
    pub name: String,
    /// What it gives out in full sunlight: `"7 A"`.
    pub current: String,
}

/// A solar array with its figures read.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Resolved {
    /// Amperes, in full sunlight.
    pub current: f64,
}

impl SolarArray {
    /// Reads its figures.
    ///
    /// # Returns
    ///
    /// Them.
    ///
    /// # Errors
    ///
    /// What is wrong with its current, naming it.
    pub fn resolve(&self) -> Result<Resolved, String> {
        Ok(Resolved {
            current: positive("current", &self.current, Some("A"))?,
        })
    }
}
