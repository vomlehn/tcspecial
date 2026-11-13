//! A battery: a store of charge on the bus, which takes what the sources
//! have left over and makes up what they fall short by.
//!
//! It takes what is `offered`, in amperes, what the bus has left over: more
//! than nought charges it, less discharges it. It gives out its `voltage`,
//! in volts, the `current` that actually flows into it, and its `charge`,
//! the fraction of its capacity it holds, as a ratio.
//!
//! # The arithmetic
//!
//! Charge is current over time, so with the current held steady its charge
//! moves in a straight line:
//!
//! ```text
//! charge(t + dt) = charge + current * dt / capacity
//! ```
//!
//! and its voltage is its open-circuit voltage, straight from `empty` to
//! `full` with its charge, plus what its internal resistance drops:
//!
//! ```text
//! voltage = empty + (full - empty) * charge + resistance * current
//! ```
//!
//! A straight line meets full or empty at a moment it can say in advance,
//! which is the world's next event. Full, a charging current is shunted --
//! the regulator a spacecraft has for the purpose turns it into heat -- and
//! nothing flows in; empty, it is held there, a cell past what it is rated
//! for being outside what this models. The arithmetic is tcs-sim's
//! `physics::battery`.

use serde::{Deserialize, Serialize};

use super::setting::{positive, quantity};
use super::terminal::{Kind, Terminal};
use crate::unit::Unit;

/// Its terminals.
pub const TERMINALS: &[Terminal] = &[
    Terminal::input("offered", Kind::Value(Unit::Ampere)).must_be_joined(),
    Terminal::output("voltage", Kind::Value(Unit::Volt)),
    Terminal::output("current", Kind::Value(Unit::Ampere)),
    Terminal::output("charge", Kind::Value(Unit::Ratio)),
];

/// Everything it states, by the name a file writes, the
/// `component` naming it apart: the one list, from which an editor builds
/// its form.
pub const SETTINGS: &[&str] =
    &["capacity", "empty", "full", "resistance", "charge"];

/// A battery, as a physics model states it.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Battery {
    /// What it is called, which is how a connection or a simulated payload
    /// names it.
    #[serde(rename = "component")]
    pub name: String,
    /// How much charge it holds full, in ampere-hours: `"10 Ah"`.
    pub capacity: String,
    /// Its open-circuit voltage empty: `"24 V"`.
    pub empty: String,
    /// Its open-circuit voltage full, which is more than `empty`:
    /// `"32.8 V"`.
    pub full: String,
    /// Its internal resistance, in ohms: `"0.05 ohm"`.
    pub resistance: String,
    /// How full it starts, as a fraction of its capacity, written bare:
    /// `"0.6"`. Stated, there being no figure to assume: a vehicle can be
    /// launched with its battery at any charge.
    pub charge: String,
}

/// A battery with its figures read.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Resolved {
    /// Ampere-hours.
    pub capacity: f64,
    /// Volts, empty.
    pub empty: f64,
    /// Volts, full.
    pub full: f64,
    /// Ohms.
    pub resistance: f64,
    /// The fraction of its capacity it starts with.
    pub charge: f64,
}

impl Battery {
    /// Reads its figures.
    ///
    /// # Returns
    ///
    /// Them.
    ///
    /// # Errors
    ///
    /// What is wrong with the first that does not read, naming it; a full
    /// voltage no more than its empty one; or a starting charge that is not
    /// a fraction of its capacity.
    pub fn resolve(&self) -> Result<Resolved, String> {
        let empty = positive("empty", &self.empty, Some("V"))?;
        let full = positive("full", &self.full, Some("V"))?;
        if full <= empty {
            return Err(format!(
                "full is {full} V and empty {empty} V: a battery's voltage \
                 rises as it charges, so full is the higher"
            ));
        }
        let resistance = quantity("resistance", &self.resistance, Some("ohm"))?;
        if resistance < 0.0 {
            return Err(format!(
                "resistance: {resistance} is less than nought, and a \
                 resistance is not"
            ));
        }
        let charge = quantity("charge", &self.charge, None)?;
        if !(0.0..=1.0).contains(&charge) {
            return Err(format!(
                "charge: {charge} is not a fraction of its capacity: it is \
                 from nought to one"
            ));
        }
        Ok(Resolved {
            capacity: positive("capacity", &self.capacity, Some("Ah"))?,
            empty,
            full,
            resistance,
            charge,
        })
    }
}
