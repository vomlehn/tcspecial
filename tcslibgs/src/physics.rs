//! The physics model: the vehicle's physical elements, which the simulator's
//! stand-ins measure and act on.
//!
//! A file of its own, beside the simulator file and named by it, because a
//! vehicle's physics outlives any one configuration of its payloads: the
//! zone is the same body whichever payloads fly. Kept in each simulator
//! file, the same vehicle would be written once per configuration, and the
//! copies would drift. One model, named by every simulator file that uses
//! it, is one statement of it.
//!
//! What it holds is a list of elements, each of a type, as a configuration
//! holds nodes:
//!
//! ```yaml
//! physics:
//!   - name: zone
//!     type: thermal_mass
//!     environment: "283.15"
//!     time_constant: "50 s"
//!     inputs: "in1,in2"
//!     outputs: "out1,out2,out3"
//! ```
//!
//! A simulated payload attaches to an element by its name and one of its
//! terminals -- a sensor to an output, a heater to an input -- in the
//! simulator file, which says what each stand-in is; this file says only
//! what the world is. One type so far, the thermal mass, whose arithmetic is
//! tcssim's `physics::thermal_mass`.
//!
//! YAML only. A model is written by hand or by a tool that writes YAML, and
//! a second spelling would be a second thing to keep the same as the first.

use std::path::Path;

use serde::Deserialize;

use crate::error::{TcsError, TcsResult};
use crate::sim_config::blank_is_unstated;

/// Every element type a physics model may hold, by the name a file writes as
/// its `type`. The one list of them.
pub const TYPES: &[&str] = &["thermal_mass"];

/// Everything a thermal mass states, by the name a file writes, `name` and
/// `type` apart.
///
/// The one list of them, from which an editor builds a mass's form: a
/// setting missing from it is one nobody can type there, and one in it that
/// the reader does not take is one the test below refuses.
pub const THERMAL_MASS_SETTINGS: &[&str] = &[
    "environment",
    "time_constant",
    "inputs",
    "outputs",
    "swing",
    "period",
    "wander",
    "wander_time",
];

/// A physics model, as it parses.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PhysicsFile {
    /// What the model is of.
    #[serde(default)]
    pub description: Option<String>,
    /// Its elements, in the order the file lists them.
    #[serde(default)]
    pub physics: Vec<Element>,
}

/// One element of a physics model, of one of the [`TYPES`].
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum Element {
    #[serde(rename = "thermal_mass")]
    ThermalMass(ThermalMass),
}

/// Says whether a file is named as a physics model is: `.yaml` or `.yml`.
///
/// # Arguments
///
/// * `path` - The file.
///
/// # Returns
///
/// Whether its name ends in either.
pub fn is_yaml_name(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e == "yaml" || e == "yml")
}

impl PhysicsFile {
    /// Reads a physics model.
    ///
    /// # Arguments
    ///
    /// * `path` - The file, which is YAML and named `.yaml` or `.yml`.
    ///
    /// # Returns
    ///
    /// The model.
    ///
    /// # Errors
    ///
    /// `TcsError::Config` naming the file where it is not YAML by its name,
    /// cannot be read, or does not parse as a model.
    pub fn load<P: AsRef<Path>>(path: P) -> TcsResult<PhysicsFile> {
        let path = path.as_ref();
        let named = |why: String| {
            TcsError::Config(format!("{}: {why}", path.display()))
        };
        if !is_yaml_name(path) {
            return Err(named(
                "a physics model is YAML, named .yaml or .yml".to_string(),
            ));
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| named(format!("cannot be read: {e}")))?;
        PhysicsFile::parse(&text).map_err(|e| named(e.to_string()))
    }

    /// Reads a physics model from its text.
    ///
    /// # Arguments
    ///
    /// * `text` - The model, as YAML.
    ///
    /// # Returns
    ///
    /// The model.
    ///
    /// # Errors
    ///
    /// What the YAML reader says, with the line it says it at.
    pub fn parse(text: &str) -> TcsResult<PhysicsFile> {
        crate::format::ConfigFormat::Yaml.parse(text)
    }

    /// Its thermal masses, in the order the file lists them.
    ///
    /// # Returns
    ///
    /// Every element of type `thermal_mass`.
    pub fn thermal_masses(&self) -> Vec<ThermalMass> {
        self.physics
            .iter()
            .map(|element| match element {
                Element::ThermalMass(mass) => mass.clone(),
            })
            .collect()
    }
}

/// A thermal mass, as a physics model states it: one body at one
/// temperature, which heaters warm and sensors read.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ThermalMass {
    /// What it is called, which is the name a payload names to attach to it.
    pub name: String,
    /// The temperature it tends towards with nothing warming it, in kelvin.
    ///
    /// Fixed, and not a figure that follows the vehicle's setpoint: a mass
    /// whose surroundings tracked the setpoint could never be driven away
    /// from it, and commanding the setpoint from the ground would then show
    /// nothing at all.
    pub environment: String,
    /// How quickly it loses the difference, as a length of time.
    ///
    /// The time constant of Newton's cooling: with nothing warming it, the
    /// gap between the mass and its surroundings falls to a third of itself
    /// in this long. A mass ten kelvin above its surroundings with a fifty
    /// second constant is cooling at a fifth of a kelvin a second.
    ///
    /// Stated as a time rather than as that rate because the rate depends on
    /// how far from its surroundings the mass happens to be, and the constant
    /// does not: one figure describes the mass at every temperature.
    pub time_constant: String,
    /// The names of the places a heater can be connected to it, separated
    /// by commas: `in1,in2`. None need be connected, and none is an empty
    /// list.
    #[serde(default, deserialize_with = "blank_is_unstated")]
    pub inputs: Option<String>,
    /// The names of the places a sensor can read its temperature from,
    /// separated by commas: `out1,out2,out3`. None need be connected, and
    /// none is an empty list. A name may be followed by `=` and how far,
    /// in kelvin, that place sits from the mass's temperature -- `out2=+0.3`
    /// -- since a body warmed in one place is not one temperature
    /// throughout.
    #[serde(default, deserialize_with = "blank_is_unstated")]
    pub outputs: Option<String>,
    /// How far its environment swings either side of `environment`, in
    /// kelvin, as a sine wave: a day and a night, or an orbit's sun and
    /// shadow. Unstated is none.
    #[serde(default, deserialize_with = "blank_is_unstated")]
    pub swing: Option<String>,
    /// How long one swing takes, as a length of time. Required with `swing`
    /// and refused without it.
    #[serde(default, deserialize_with = "blank_is_unstated")]
    pub period: Option<String>,
    /// How far its environment wanders at random: the standard deviation,
    /// in kelvin, of a departure drawn back towards nought with
    /// `wander_time` as its time constant. Unstated is none.
    #[serde(default, deserialize_with = "blank_is_unstated")]
    pub wander: Option<String>,
    /// How slowly its environment wanders, as a length of time. Required
    /// with `wander` and refused without it.
    #[serde(default, deserialize_with = "blank_is_unstated")]
    pub wander_time: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A model's elements are typed, and a mass reads as it did when it was
    /// a section of the simulator file.
    #[test]
    fn a_model_holds_typed_elements() {
        let model = PhysicsFile::parse(
            "description: a zone\nphysics:\n  - name: zone\n    \
             type: thermal_mass\n    environment: \"283.15\"\n    \
             time_constant: \"50 s\"\n    inputs: \"in1\"\n",
        )
        .expect("it parses");
        let masses = model.thermal_masses();
        assert_eq!(masses.len(), 1);
        assert_eq!(masses[0].name, "zone");
        assert_eq!(masses[0].environment, "283.15");
        assert_eq!(masses[0].inputs.as_deref(), Some("in1"));
    }

    /// An element of no type, or of one there is none of, or with a
    /// setting its type has not, is refused, naming what is wrong.
    #[test]
    fn an_element_of_no_known_type_is_refused() {
        for (text, said) in [
            ("physics:\n  - name: zone\n    environment: \"1\"\n", "type"),
            ("physics:\n  - name: zone\n    type: battery\n", "battery"),
            (
                "physics:\n  - name: c\n    type: thermal_mass\n    \
                 environment: \"1\"\n    time_constant: \"1 s\"\n    \
                 colour: red\n",
                "colour",
            ),
        ] {
            let why = PhysicsFile::parse(text).expect_err(text).to_string();
            assert!(why.contains(said), "{text}: {why}");
        }
    }

    /// Every type a model may hold is one an element reads as.
    #[test]
    fn every_type_listed_is_one_an_element_reads_as() {
        for name in TYPES {
            let text = format!(
                "physics:\n  - name: x\n    type: {name}\n    \
                 environment: \"1\"\n    time_constant: \"1 s\"\n"
            );
            assert!(PhysicsFile::parse(&text).is_ok(), "{name}");
        }
    }

    /// Every setting the list names is one a thermal mass takes.
    #[test]
    fn every_thermal_mass_setting_listed_is_read() {
        let mut text =
            String::from("physics:\n  - name: x\n    type: thermal_mass\n");
        for setting in THERMAL_MASS_SETTINGS {
            text.push_str(&format!("    {setting}: \"1\"\n"));
        }
        assert!(PhysicsFile::parse(&text).is_ok(), "{text}");
    }

    /// A model is YAML, and a file named otherwise is refused by its name.
    #[test]
    fn a_model_is_yaml() {
        let why = PhysicsFile::load("zone.xml").expect_err("not YAML");
        assert!(why.to_string().contains("is YAML"), "{why}");
    }
}
