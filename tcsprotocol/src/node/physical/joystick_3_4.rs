//! A joystick: two axes, each set by hand, for the vehicle to read.
//!
//! It takes `set_x` and `set_y`, each a ratio from minus one to one, which
//! nothing in the model drives: tcs-sim's window sets them as its joystick
//! is moved, the way a simulated payload drives an input. It gives them out
//! as `x` and `y`, for simulated payloads joined to them to report: a hand
//! controller aboard, read by the vehicle like any other sensor. The
//! arithmetic is tcs-sim's `physical::joystick`.

use serde::{Deserialize, Serialize};

use super::terminal::{Kind, Terminal};
use crate::unit::Unit;

/// Its terminals.
pub const TERMINALS: &[Terminal] = &[
    Terminal::input("set_x", Kind::Value(Unit::Ratio)),
    Terminal::input("set_y", Kind::Value(Unit::Ratio)),
    Terminal::output("x", Kind::Value(Unit::Ratio)),
    Terminal::output("y", Kind::Value(Unit::Ratio)),
];

/// Everything it states, by the name a file writes, the `component` naming
/// it apart: nothing, since it is set by hand.
pub const SETTINGS: &[&str] = &[];

/// A joystick, as a physical model states it: by its name alone.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Joystick34 {
    /// What it is called, which is how a connection, a simulated payload or
    /// tcs-sim's window names it.
    #[serde(rename = "component")]
    pub name: String,
}
