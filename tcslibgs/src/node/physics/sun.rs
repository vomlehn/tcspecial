//! The sun: what light falls on the vehicle, through its orbit.
//!
//! It gives out `light`, a ratio: one in sunlight and nought in the
//! planet's shadow. An orbit of `period` spends `eclipse` of itself in
//! shadow, a low orbit a third or so, and the change between the two is
//! sudden, the penumbra lasting seconds of an orbit's ninety minutes. So
//! each change is an event of the world's, foreseen from the orbit: the
//! arithmetic is tcssim's `physics::sun`.
//!
//! It starts at the beginning of the sunlit part of its orbit, or `start`
//! of the way round from it.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::setting::{quantity, time_of};
use super::terminal::{Kind, Terminal};
use crate::sim_config::blank_is_unstated;
use crate::unit::Unit;

/// Its terminals.
pub const TERMINALS: &[Terminal] =
    &[Terminal::output("light", Kind::Value(Unit::Ratio))];

/// Everything it states, by the name a file writes, the
/// `component` naming it apart: the one list, from which an editor builds
/// its form.
pub const SETTINGS: &[&str] = &["period", "eclipse", "start"];

/// The sun, as a physics model states it.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Sun {
    /// What it is called, which is how a connection or a simulated payload
    /// names it.
    #[serde(rename = "component")]
    pub name: String,
    /// How long one orbit takes, as a length of time: `"90 min"`.
    pub period: String,
    /// How much of an orbit is in shadow, as a fraction, written bare:
    /// `"0.35"`. Nought is an orbit always in sunlight.
    pub eclipse: String,
    /// How far round the orbit it starts, from the start of its sunlight,
    /// as a fraction, written bare. Unstated is nought.
    #[serde(default, deserialize_with = "blank_is_unstated")]
    pub start: Option<String>,
}

/// The sun with its figures read.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Resolved {
    pub period: Duration,
    /// The fraction of an orbit in shadow, from nought up to but not one.
    pub eclipse: f64,
    /// The fraction of an orbit it starts at, from nought up to but not
    /// one.
    pub start: f64,
}

impl Sun {
    /// Reads its figures.
    ///
    /// # Returns
    ///
    /// Them.
    ///
    /// # Errors
    ///
    /// What is wrong with the first that does not read, naming it: an
    /// eclipse or a start that is not a fraction of an orbit.
    pub fn resolve(&self) -> Result<Resolved, String> {
        let fraction = |what: &str, said: &str| {
            let value = quantity(what, said, None)?;
            if (0.0..1.0).contains(&value) {
                Ok(value)
            } else {
                Err(format!(
                    "{what}: {value} is not a fraction of an orbit: it is \
                     from nought up to but not one"
                ))
            }
        };
        Ok(Resolved {
            period: time_of("period", &self.period)?,
            eclipse: fraction("eclipse", &self.eclipse)?,
            start: match &self.start {
                None => 0.0,
                Some(said) => fraction("start", said)?,
            },
        })
    }
}
