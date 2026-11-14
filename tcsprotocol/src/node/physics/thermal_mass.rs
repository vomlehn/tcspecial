//! A thermal mass: one body at one temperature, which heat warms and its
//! surroundings pull back.
//!
//! It takes `heat`, in watts, the sum of every heater's connected to it, and
//! gives out its `temperature`, in kelvin. And it has two heaters of its own,
//! `heater_1` and `heater_2`: inputs that take a decision -- a heater's switch,
//! a `gpio_out` or a `gpio_inout`, joined straight to the zone -- each adding
//! its `heater_1_power` or `heater_2_power` to the heat while it is on. Two,
//! because two heaters on one zone is what a vehicle that must not lose its
//! only way to warm a zone carries; and fixed, because a component's terminals
//! are its type's. Newton's cooling with that heat:
//!
//! ```text
//! dT/dt = -(T - environment) / time_constant + heat / capacity
//! ```
//!
//! Its environment is the world's `ambient_temp`, one temperature for every
//! zone of the vehicle: see [`super::WorldSection`]. About that, a zone's
//! surroundings may swing, as a sine wave of `swing` either side of it over
//! `period` -- an orbit's sun and shadow -- and wander at random by
//! `wander`, over `wander_time`. It starts at its surroundings, or at
//! `start` where that is stated. The arithmetic is tcs-sim's
//! `physics::thermal_mass`.

use std::time::Duration;

use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use super::setting::{positive, quantity, size_of, time_of, wander_of, Wander};
use super::terminal::{Kind, Terminal};
use crate::sim_config::blank_is_unstated;
use crate::unit::Unit;

/// Its terminals.
pub const TERMINALS: &[Terminal] = &[
    Terminal::input("heat", Kind::Value(Unit::Watt)).summing(),
    Terminal::output("temperature", Kind::Value(Unit::Kelvin)),
    Terminal::input("heater_1", Kind::Decision),
    Terminal::input("heater_2", Kind::Decision),
];

/// Its two heaters: each one's input, and the setting saying the watts it
/// gives while on, in the order of [`Resolved::heater_power`].
pub const HEATERS: [(&str, &str); 2] = [
    ("heater_1", "heater_1_power"),
    ("heater_2", "heater_2_power"),
];

/// Everything it states, by the name a file writes, the
/// `component` naming it apart: the one list, from which an editor builds
/// its form.
pub const SETTINGS: &[&str] = &[
    "time_constant",
    "capacity",
    "swing",
    "period",
    "wander",
    "wander_time",
    "start",
    "heater_1_power",
    "heater_2_power",
];

/// A thermal mass, as a physics model states it.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ThermalMass {
    /// What it is called, which is how a connection or a simulated payload
    /// names it.
    #[serde(rename = "component")]
    pub name: String,
    /// The temperature its surroundings were, which is the world's
    /// `ambient_temp` now. Read only to be refused, saying so: see
    /// [`MOVED_ENVIRONMENT`].
    #[serde(default, rename = "environment", skip_serializing)]
    pub moved_environment: Option<IgnoredAny>,
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
    /// How far its surroundings swing either side of the world's
    /// `ambient_temp`, in kelvin, as a sine wave. Unstated is none.
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
    /// Where it starts, in kelvin. Unstated is at its surroundings, which is
    /// a vehicle coming up cold.
    ///
    /// A zone already in its band is how a run looks once the first climb
    /// is over, and a run that starts there shows the thermostat at work from
    /// its first reading. One fixed figure rather than one drawn each run, so
    /// that every run of a model starts the same and two can be compared.
    #[serde(default, deserialize_with = "blank_is_unstated")]
    pub start: Option<String>,
    /// What its first heater gives while it is on, in watts: `"4 W"`.
    /// Wanted where `heater_1` is joined, and meaning nothing where it is
    /// not.
    #[serde(default, deserialize_with = "blank_is_unstated")]
    pub heater_1_power: Option<String>,
    /// And its second's.
    #[serde(default, deserialize_with = "blank_is_unstated")]
    pub heater_2_power: Option<String>,
}

/// What a zone that states its own `environment` is told.
pub const MOVED_ENVIRONMENT: &str = "environment is the world's now: state \
    it once, as ambient_temp under world:, beside physics:, and every \
    thermal_mass takes its surroundings from it";

/// A thermal mass with its figures read.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Resolved {
    /// The temperature it tends towards with nothing warming it, in kelvin:
    /// the world's `ambient_temp`.
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
    /// Where it starts, in kelvin; `None` is at its surroundings.
    pub start: Option<f64>,
    /// What each of its two heaters gives while on, in watts, in the order
    /// of [`HEATERS`]; `None` for one it does not state.
    pub heater_power: [Option<f64>; 2],
}

impl Resolved {
    /// What is wrong with joining something to one of its terminals, if
    /// anything: a heater of its own joined with no power stated for it,
    /// which would be a heater that switched and warmed nothing.
    ///
    /// # Arguments
    ///
    /// * `terminal` - The terminal joined.
    ///
    /// # Returns
    ///
    /// What to say, or `None` where the join is one it can take.
    pub fn unpowered(&self, terminal: &str) -> Option<String> {
        let (n, (_, setting)) = HEATERS
            .iter()
            .enumerate()
            .find(|(_, (name, _))| *name == terminal)?;
        self.heater_power[n].is_none().then(|| {
            format!(
                "{terminal} is joined and {setting} is not stated: say what \
                 that heater gives while on, as {setting}: \"4 W\""
            )
        })
    }
}

impl ThermalMass {
    /// Reads its figures.
    ///
    /// # Arguments
    ///
    /// * `ambient` - The world's `ambient_temp`, in kelvin, which its
    ///               surroundings are about.
    ///
    /// # Returns
    ///
    /// Them.
    ///
    /// # Errors
    ///
    /// What is wrong with the first that does not read, naming it, and
    /// [`MOVED_ENVIRONMENT`] for a zone stating its own environment.
    pub fn resolve(&self, ambient: f64) -> Result<Resolved, String> {
        if self.moved_environment.is_some() {
            return Err(MOVED_ENVIRONMENT.to_string());
        }
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
        let start = match &self.start {
            Some(said) => Some(quantity("start", said, Some("K"))?),
            None => None,
        };
        let power = |name: &str, said: &Option<String>| match said {
            Some(said) => positive(name, said, Some("W")).map(Some),
            None => Ok(None),
        };
        let heater_power = [
            power("heater_1_power", &self.heater_1_power)?,
            power("heater_2_power", &self.heater_2_power)?,
        ];
        Ok(Resolved {
            environment: ambient,
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
            start,
            heater_power,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The example's zone, as a file states it.
    ///
    /// # Returns
    ///
    /// It, with nothing optional stated.
    fn zone() -> ThermalMass {
        ThermalMass {
            name: "zone".to_string(),
            moved_environment: None,
            time_constant: "50 s".to_string(),
            capacity: "20 J/K".to_string(),
            swing: None,
            period: None,
            wander: None,
            wander_time: None,
            start: None,
            heater_1_power: None,
            heater_2_power: None,
        }
    }

    /// A start is read in kelvin; unstated, the zone starts at its
    /// surroundings.
    #[test]
    fn a_start_is_read_in_kelvin() {
        assert_eq!(zone().resolve(288.15).unwrap().start, None);

        let started = ThermalMass {
            start: Some("292.15 K".to_string()),
            ..zone()
        }
        .resolve(288.15)
        .unwrap();
        assert_eq!(started.start, Some(292.15));
    }

    /// Its heaters' powers are read in watts, and a heater joined with no
    /// power stated is told what to write; one not joined says nothing.
    #[test]
    fn its_heaters_powers_are_read_in_watts() {
        let both = ThermalMass {
            heater_1_power: Some("4 W".to_string()),
            heater_2_power: Some("6 W".to_string()),
            ..zone()
        }
        .resolve(288.15)
        .unwrap();
        assert_eq!(both.heater_power, [Some(4.0), Some(6.0)]);
        assert_eq!(both.unpowered("heater_1"), None);

        let none = zone().resolve(288.15).unwrap();
        let said = none.unpowered("heater_2").expect("unpowered");
        assert!(said.contains("heater_2_power: \"4 W\""), "{said}");
        assert_eq!(none.unpowered("heat"), None, "not a heater of its own");

        let err = ThermalMass {
            heater_1_power: Some("4 K".to_string()),
            ..zone()
        }
        .resolve(288.15)
        .unwrap_err();
        assert!(err.contains("heater_1_power"), "{err}");
    }

    /// A start with no unit, or in another unit, is refused, told what to
    /// write.
    #[test]
    fn a_start_that_cannot_be_used_is_refused() {
        let err = ThermalMass {
            start: Some("292.15".to_string()),
            ..zone()
        }
        .resolve(288.15)
        .unwrap_err();
        assert!(err.contains("start: \"292.15\" has no unit"), "{err}");

        let err = ThermalMass {
            start: Some("19 C".to_string()),
            ..zone()
        }
        .resolve(288.15)
        .unwrap_err();
        assert!(err.contains("start is in K"), "{err}");
    }
}
