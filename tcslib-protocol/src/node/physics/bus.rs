//! A power bus: where the sources' currents and the loads' meet.
//!
//! It takes `supply`, the sum of every source's current joined to it, and
//! `demand`, the sum of every load's, both in amperes, and gives out the
//! `current` left over, which is what charges the battery joined to it, or,
//! where it is less than nought, what the battery has to make up. It has no
//! settings: a bus is its connections. The arithmetic is tcs-sim's
//! `physics::bus`.

use serde::{Deserialize, Serialize};

use super::terminal::{Kind, Terminal};
use crate::unit::Unit;

/// Its terminals.
pub const TERMINALS: &[Terminal] = &[
    Terminal::input("supply", Kind::Value(Unit::Ampere)).summing(),
    Terminal::input("demand", Kind::Value(Unit::Ampere)).summing(),
    Terminal::output("current", Kind::Value(Unit::Ampere)),
];

/// Everything it states, by the name a file writes, the
/// `component` naming it apart: nothing.
pub const SETTINGS: &[&str] = &[];

/// A bus, as a physics model states it.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Bus {
    /// What it is called, which is how a connection or a simulated payload
    /// names it.
    #[serde(rename = "component")]
    pub name: String,
}
