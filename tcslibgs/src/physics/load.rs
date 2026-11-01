//! A load: something on the bus that draws a current while it is on, and
//! can be shed.
//!
//! It gives out the `current` it draws, in amperes, and takes `shed`, a
//! decision: while it is asserted the load is switched off and draws
//! nothing. Unjoined, a load is never shed, as an essential one is not;
//! joined to a stand-in, the vehicle sheds it by driving that stand-in, as
//! a hysteresis node does when the battery runs low. The arithmetic is
//! tcssim's `physics::load`.

use serde::Deserialize;

use super::setting::positive;
use super::terminal::{Kind, Terminal};
use crate::unit::Unit;

/// Its terminals.
pub const TERMINALS: &[Terminal] = &[
    Terminal::input("shed", Kind::Decision),
    Terminal::output("current", Kind::Value(Unit::Ampere)),
];

/// Everything it states, by the name a file writes, `name` and `type`
/// apart: the one list, from which an editor builds its form.
pub const SETTINGS: &[&str] = &["current"];

/// A load, as a physics model states it.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Load {
    /// What it is called, which is how a connection or a stand-in names it.
    pub name: String,
    /// What it draws while it is on: `"2 A"`.
    pub current: String,
}

/// A load with its figures read.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Resolved {
    /// Amperes, while it is on.
    pub current: f64,
}

impl Load {
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
