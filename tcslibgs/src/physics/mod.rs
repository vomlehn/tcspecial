//! The physics model: the vehicle's physical components, which the
//! simulator's stand-ins measure and act on.
//!
//! The `physics` of the simulator file's `world` section, beside the
//! stand-ins that are joined to it; the world also states its
//! `ambient_temp`, the one temperature every thermal zone's surroundings
//! are about: see [`WorldSection`]. It was once a file of its own, named by the simulator file,
//! so that one vehicle could be shared by several configurations of its
//! payloads; but a model in another file is a second file to find, to keep
//! and to send with the first, and a stand-in's `connects` names a terminal
//! that only the other file says is there. One file says the whole of what
//! tcssim simulates. A simulator file that still names a model, and a model
//! written as a file of its own, are refused, told where it goes now: see
//! [`crate::sim_reshaped`].
//!
//! # What it holds
//!
//! Its components, grouped by type as a configuration groups its nodes, and
//! the connections between them, as a configuration holds links:
//!
//! ```yaml
//! world:
//!   ambient_temp: "283.15 K"
//!   physics:
//!     components:
//!       - type heater:
//!           - component: heater_1
//!             power: "60 W"
//!             voltage: "28 V"
//!       - type thermal_zone:
//!           - component: zone
//!             time_constant: "50 s"
//!             capacity: "20 J/K"
//!     connections:
//!       - from: heater_1.heat
//!         to: zone.heat
//! ```
//!
//! Nothing in it is in an order that means anything: not the types, not the
//! components of one, and not the connections. The components are held in
//! name order and the connections in the order of the terminals they join,
//! by name, so that the world made of a model, and every tie in it, is the
//! same however the file is written: see [`Model`].
//!
//! Each type is a simple part, simulated alone, with its own words for what
//! goes in and comes out: a heater is switched `on` and gives out `heat`; a
//! zone takes `heat` and has a `temperature`. A vehicle is made of them by
//! joining them, rather than by a type that is a whole thermostat, so that a
//! part is written once and a different vehicle is a different set of
//! connections. Each type's terminals are fixed, in its own module, each
//! with its direction and its kind -- a value in a unit, or a decision -- and
//! a connection joins an output to an input of the same kind: see
//! [`terminal`].
//!
//! A stand-in elsewhere in the simulator file is joined to a terminal by
//! naming it, `connects: zone.temperature`: a sensor reports an output, and
//! a stand-in the vehicle drives drives an input. This section says only
//! what the world is.
//!
//! # Its figures
//!
//! Each written with its unit, which is checked: see [`setting`].
//!
//! YAML only. A model is written by hand or by a tool that writes YAML, and
//! a second spelling would be a second thing to keep the same as the first:
//! a simulator file written in XML has no `world`.

pub mod battery;
pub mod body;
pub mod bus;
pub mod engine;
pub mod grouped;
pub mod heater;
pub mod load;
pub mod setting;
pub mod solar_array;
pub mod sun;
pub mod terminal;
pub mod thermal_zone;

use std::collections::BTreeMap;

use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use crate::error::TcsResult;
use crate::sim_config::blank_is_unstated;
use crate::verify::About;
pub use setting::Wander;
use terminal::{At, Direction, Terminal};

/// Every component type a physics model may hold, by the name a file writes
/// after `type` in the entry its components are grouped under. The one list
/// of them.
pub const TYPES: &[&str] = &[
    "heater",
    "thermal_zone",
    "sun",
    "solar_array",
    "load",
    "bus",
    "battery",
    "engine",
    "body",
];

/// The simulator file's `world` section, as it parses: the vehicle's
/// physics, and the temperature its surroundings are about.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WorldSection {
    /// The temperature every thermal zone's surroundings are about, in
    /// kelvin: `"288.15 K"`. One for the world, because a vehicle's zones
    /// are in one set of surroundings; each zone says how they vary about
    /// it, by its `swing` and its `wander`. Required where there is a
    /// thermal zone.
    ///
    /// Fixed, and not a figure that follows the vehicle's setpoint: a zone
    /// whose surroundings tracked the setpoint could never be driven away
    /// from it, and commanding the setpoint from the ground would then show
    /// nothing at all.
    #[serde(default, deserialize_with = "blank_is_unstated")]
    pub ambient_temp: Option<String>,
    /// The components and their connections.
    #[serde(default)]
    pub physics: PhysicsSection,
}

/// A physics model, as the `physics` of the `world` section parses.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PhysicsSection {
    /// Its components, in name order, whatever order the file writes them
    /// in: see [`grouped`].
    #[serde(default, deserialize_with = "grouped::components")]
    pub components: Vec<Component>,
    /// How its components are joined, each an output to an input.
    #[serde(default)]
    pub connections: Vec<Connection>,
}

/// One component of a physics model, of one of the [`TYPES`], as a file
/// names it.
///
/// Read by [`grouped`], the type being the entry's it is written under, and
/// written out with its type beside it for the simulator file's digest: see
/// [`crate::config_digest::interpretation_of_simulator`].
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type")]
pub enum Component {
    #[serde(rename = "heater")]
    Heater(heater::Heater),
    #[serde(rename = "thermal_zone")]
    ThermalZone(thermal_zone::ThermalZone),
    #[serde(rename = "sun")]
    Sun(sun::Sun),
    #[serde(rename = "solar_array")]
    SolarArray(solar_array::SolarArray),
    #[serde(rename = "load")]
    Load(load::Load),
    #[serde(rename = "bus")]
    Bus(bus::Bus),
    #[serde(rename = "battery")]
    Battery(battery::Battery),
    #[serde(rename = "engine")]
    Engine(engine::Engine),
    #[serde(rename = "body")]
    Body(body::Body),
    /// The one type there was before a model was made of parts. Read only
    /// to be refused, saying what it is now: see [`MASS_MADE_OF_PARTS`].
    #[serde(rename = "thermal_mass")]
    ThermalMass(Moved),
}

/// A component of a type there is no longer, read only to be refused.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Moved {
    #[serde(rename = "component")]
    pub name: String,
    #[serde(flatten, skip_serializing)]
    pub rest: BTreeMap<String, IgnoredAny>,
}

/// Where a problem with the connections is reported: the `connections` of
/// the simulator file's `world: physics:`.
pub const CONNECTIONS: &str = "world.physics.connections";

/// Where a problem with the world's `ambient_temp` is reported.
pub const AMBIENT: &str = "world.ambient_temp";

/// What a model with a `thermal_mass` is told.
pub const MASS_MADE_OF_PARTS: &str = "a thermal_mass is now made of parts: \
    a thermal_zone, with time_constant and a capacity in J/K, its \
    surroundings the world's ambient_temp, and a heater for each heater, \
    with its power in W and its voltage in V, joined under connections: \
    from each heater's heat to the zone's heat. A stand-in then names the \
    terminal it is joined to, connects: zone.temperature or connects: \
    heater_1.on, in place of reads, heats, output, input and rate. The \
    world section of examples/thermostat-common/thermostat-sim.yaml is one \
    written so";

/// A connection between two components, as a file writes it.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    /// An output: `heater_1.heat`.
    pub from: String,
    /// An input: `zone.heat`.
    pub to: String,
}

impl Component {
    /// What it is called.
    ///
    /// # Returns
    ///
    /// Its name, as the file writes it.
    pub fn name(&self) -> &str {
        match self {
            Component::Heater(e) => &e.name,
            Component::ThermalZone(e) => &e.name,
            Component::Sun(e) => &e.name,
            Component::SolarArray(e) => &e.name,
            Component::Load(e) => &e.name,
            Component::Bus(e) => &e.name,
            Component::Battery(e) => &e.name,
            Component::Engine(e) => &e.name,
            Component::Body(e) => &e.name,
            Component::ThermalMass(e) => &e.name,
        }
    }

    /// The type a file writes it as.
    ///
    /// # Returns
    ///
    /// Its type, as a file writes it.
    pub fn type_name(&self) -> &'static str {
        match self {
            Component::Heater(_) => "heater",
            Component::ThermalZone(_) => "thermal_zone",
            Component::Sun(_) => "sun",
            Component::SolarArray(_) => "solar_array",
            Component::Load(_) => "load",
            Component::Bus(_) => "bus",
            Component::Battery(_) => "battery",
            Component::Engine(_) => "engine",
            Component::Body(_) => "body",
            Component::ThermalMass(_) => "thermal_mass",
        }
    }
}

/// The settings a component of a type states, the `component` naming it
/// apart: the one list for each, from which an editor builds a component's
/// form.
///
/// # Arguments
///
/// * `type_name` - The type, as a file writes it.
///
/// # Returns
///
/// Its settings, or `None` for a type that is not one of the [`TYPES`].
pub fn settings_of(type_name: &str) -> Option<&'static [&'static str]> {
    match type_name {
        "heater" => Some(heater::SETTINGS),
        "thermal_zone" => Some(thermal_zone::SETTINGS),
        "sun" => Some(sun::SETTINGS),
        "solar_array" => Some(solar_array::SETTINGS),
        "load" => Some(load::SETTINGS),
        "bus" => Some(bus::SETTINGS),
        "battery" => Some(battery::SETTINGS),
        "engine" => Some(engine::SETTINGS),
        "body" => Some(body::SETTINGS),
        _ => None,
    }
}

/// The terminals of a component of a type.
///
/// # Arguments
///
/// * `type_name` - The type, as a file writes it.
///
/// # Returns
///
/// Its terminals, or `None` for a type that is not one of the [`TYPES`].
pub fn terminals_of(type_name: &str) -> Option<&'static [Terminal]> {
    match type_name {
        "heater" => Some(heater::TERMINALS),
        "thermal_zone" => Some(thermal_zone::TERMINALS),
        "sun" => Some(sun::TERMINALS),
        "solar_array" => Some(solar_array::TERMINALS),
        "load" => Some(load::TERMINALS),
        "bus" => Some(bus::TERMINALS),
        "battery" => Some(battery::TERMINALS),
        "engine" => Some(engine::TERMINALS),
        "body" => Some(body::TERMINALS),
        _ => None,
    }
}

/// Something wrong with a physics model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicsProblem {
    /// The component it is about, or `None` for one about the connections
    /// or the world's `ambient_temp`.
    pub component: Option<String>,
    /// Whether it is about the world's `ambient_temp`.
    pub ambient: bool,
    /// What is wrong.
    pub why: String,
}

impl PhysicsProblem {
    /// Makes a problem with a component.
    ///
    /// # Arguments
    ///
    /// * `component` - The component's name.
    /// * `why`       - What is wrong.
    ///
    /// # Returns
    ///
    /// The problem.
    pub fn of(component: &str, why: impl Into<String>) -> PhysicsProblem {
        PhysicsProblem {
            component: Some(component.to_string()),
            ambient: false,
            why: why.into(),
        }
    }

    /// Makes a problem with the world's `ambient_temp`.
    ///
    /// # Arguments
    ///
    /// * `why` - What is wrong.
    ///
    /// # Returns
    ///
    /// The problem.
    fn of_ambient(why: impl Into<String>) -> PhysicsProblem {
        PhysicsProblem {
            component: None,
            ambient: true,
            why: why.into(),
        }
    }

    /// Makes a problem with the connections.
    ///
    /// # Arguments
    ///
    /// * `why` - What is wrong.
    ///
    /// # Returns
    ///
    /// The problem.
    fn joining(why: impl Into<String>) -> PhysicsProblem {
        PhysicsProblem {
            component: None,
            ambient: false,
            why: why.into(),
        }
    }

    /// What it is about, which is how the line it belongs to is found in
    /// the simulator file.
    ///
    /// # Returns
    ///
    /// Its component, the connections or the world's `ambient_temp`.
    pub fn about(&self) -> About {
        match &self.component {
            Some(name) => About::PhysicsComponent(name.clone()),
            None if self.ambient => About::Section(AMBIENT),
            None => About::Section(CONNECTIONS),
        }
    }
}

impl std::fmt::Display for PhysicsProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.component {
            Some(name) => {
                write!(f, "physics component \"{name}\": {}", self.why)
            }
            None if self.ambient => {
                write!(f, "world ambient_temp: {}", self.why)
            }
            None => write!(f, "physics connections: {}", self.why),
        }
    }
}

/// A physics model with every figure read and every connection found.
///
/// In one order, whatever order its file is written in: the components by
/// name, so that a component's place -- by which the world holds it, and by
/// which two of its events due at one moment are taken -- is its place among
/// the names; and the connections by the names of the terminals they join,
/// from and then to, so that what is summed at an input is summed in one
/// order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Model {
    /// Its components, in name order.
    pub components: Vec<Resolved>,
    /// Its connections, each from an output to an input, in the order of
    /// the names of the terminals they join.
    pub connections: Vec<Joined>,
}

/// One component with its figures read.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub name: String,
    pub what: Resolution,
}

/// What a component is, with its figures read.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolution {
    Heater(heater::Resolved),
    ThermalZone(thermal_zone::Resolved),
    Sun(sun::Resolved),
    SolarArray(solar_array::Resolved),
    Load(load::Resolved),
    /// A bus has no figures: it is its connections.
    Bus,
    Battery(battery::Resolved),
    Engine(engine::Resolved),
    Body(body::Resolved),
}

impl Resolution {
    /// The type a file writes it as.
    ///
    /// # Returns
    ///
    /// Its type, as a file writes it.
    pub fn type_name(&self) -> &'static str {
        match self {
            Resolution::Heater(_) => "heater",
            Resolution::ThermalZone(_) => "thermal_zone",
            Resolution::Sun(_) => "sun",
            Resolution::SolarArray(_) => "solar_array",
            Resolution::Load(_) => "load",
            Resolution::Bus => "bus",
            Resolution::Battery(_) => "battery",
            Resolution::Engine(_) => "engine",
            Resolution::Body(_) => "body",
        }
    }

    /// Its terminals, in its type's order.
    ///
    /// # Returns
    ///
    /// Its type's terminals.
    pub fn terminals(&self) -> &'static [Terminal] {
        match self {
            Resolution::Heater(_) => heater::TERMINALS,
            Resolution::ThermalZone(_) => thermal_zone::TERMINALS,
            Resolution::Sun(_) => sun::TERMINALS,
            Resolution::SolarArray(_) => solar_array::TERMINALS,
            Resolution::Load(_) => load::TERMINALS,
            Resolution::Bus => bus::TERMINALS,
            Resolution::Battery(_) => battery::TERMINALS,
            Resolution::Engine(_) => engine::TERMINALS,
            Resolution::Body(_) => body::TERMINALS,
        }
    }
}

/// A terminal of a model, by place: which component, and which of its type's
/// terminals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place {
    pub component: usize,
    pub terminal: usize,
}

/// A connection of a model, from an output to an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Joined {
    pub from: Place,
    pub to: Place,
}

impl Model {
    /// Finds a terminal a file names.
    ///
    /// # Arguments
    ///
    /// * `at` - The component and terminal, as named.
    ///
    /// # Returns
    ///
    /// Where it is, and what it is.
    ///
    /// # Errors
    ///
    /// A message naming what there is, for a component the model has not or
    /// a terminal its type has not.
    pub fn find(&self, at: &At) -> Result<(Place, Terminal), String> {
        let Some(component) =
            self.components.iter().position(|e| e.name == at.component)
        else {
            let every: Vec<&str> =
                self.components.iter().map(|e| e.name.as_str()).collect();
            return Err(format!(
                "{at}: the physics model has no component \"{}\"; it has {}",
                at.component,
                if every.is_empty() {
                    "none".to_string()
                } else {
                    every.join(", ")
                }
            ));
        };
        let what = &self.components[component].what;
        let terminals = what.terminals();
        let Some(terminal) =
            terminals.iter().position(|t| t.name == at.terminal)
        else {
            let every: Vec<&str> = terminals.iter().map(|t| t.name).collect();
            return Err(format!(
                "{at}: a {} has no terminal \"{}\"; it has {}",
                what.type_name(),
                at.terminal,
                every.join(", ")
            ));
        };
        Ok((
            Place {
                component,
                terminal,
            },
            terminals[terminal],
        ))
    }

    /// Says what a terminal is.
    ///
    /// # Arguments
    ///
    /// * `place` - A terminal of this model.
    ///
    /// # Returns
    ///
    /// Its component's name and its own, joined as a file writes them.
    pub fn name_of(&self, place: Place) -> String {
        let component = &self.components[place.component];
        format!(
            "{}.{}",
            component.name,
            component.what.terminals()[place.terminal].name
        )
    }
}

impl WorldSection {
    /// Reads a world section from its text, as it would stand alone:
    /// `ambient_temp:` and `physics:` at the left margin. For a test, or a
    /// document's example; a program reads the section with the rest of its
    /// simulator file.
    ///
    /// # Arguments
    ///
    /// * `text` - The section's value, as YAML.
    ///
    /// # Returns
    ///
    /// The world.
    ///
    /// # Errors
    ///
    /// What the YAML reader says, with the line it says it at.
    pub fn parse(text: &str) -> TcsResult<WorldSection> {
        crate::format::ConfigFormat::Yaml.parse(text)
    }

    /// Reads every figure and finds every connection.
    ///
    /// # Returns
    ///
    /// The model.
    ///
    /// # Errors
    ///
    /// The first of [`WorldSection::problems`].
    pub fn resolve(&self) -> Result<Model, PhysicsProblem> {
        let (model, mut problems) = self.built();
        if problems.is_empty() {
            Ok(model)
        } else {
            Err(problems.remove(0))
        }
    }

    /// Everything wrong with the world as it stands alone: an
    /// `ambient_temp` that does not read, or is not stated where a thermal
    /// zone needs it; a component whose figures do not read, two of one
    /// name; a connection that names no terminal, joins two outputs or two
    /// inputs, joins a value in one unit to an input in another, or is a
    /// second output on an input that takes one.
    ///
    /// What the stand-ins are joined to is the rest of the simulator
    /// file's, and checked there: see
    /// `crate::sim_config::SimConfigFile::physics`.
    ///
    /// # Returns
    ///
    /// Each problem: the ambient temperature's, then the components', in
    /// name order, then the connections'.
    pub fn problems(&self) -> Vec<PhysicsProblem> {
        self.built().1
    }

    /// The model as far as it reads, and what is wrong with it.
    ///
    /// # Returns
    ///
    /// Every component that read and every connection found, and each
    /// problem.
    fn built(&self) -> (Model, Vec<PhysicsProblem>) {
        let mut problems = Vec::new();
        let mut model = Model::default();
        let zones = self
            .physics
            .components
            .iter()
            .any(|c| matches!(c, Component::ThermalZone(_)));
        let ambient = match &self.ambient_temp {
            Some(said) => {
                match setting::quantity("ambient_temp", said, Some("K")) {
                    Ok(kelvin) => Some(kelvin),
                    Err(why) => {
                        problems.push(PhysicsProblem::of_ambient(why));
                        None
                    }
                }
            }
            None if zones => {
                problems.push(PhysicsProblem::of_ambient(
                    "it is not stated, and a thermal_zone's surroundings are \
                     about it: state it under world:, as ambient_temp: \
                     \"288.15 K\"",
                ));
                None
            }
            None => None,
        };
        // By name, as [`grouped`] holds them already: a model made some
        // other way is in one order too.
        let mut components: Vec<&Component> =
            self.physics.components.iter().collect();
        components.sort_by(|a, b| a.name().cmp(b.name()));
        for component in components {
            let name = component.name();
            if name.trim().is_empty() || name.contains('.') {
                problems.push(PhysicsProblem::of(
                    name,
                    "a name is not empty and has no dot in it: a terminal is \
                     written as the name, a dot and the terminal's",
                ));
                continue;
            }
            if model.components.iter().any(|e| e.name == name) {
                problems.push(PhysicsProblem::of(
                    name,
                    "two components have this name, and a connection or a \
                     stand-in names one",
                ));
                continue;
            }
            let what = match component {
                Component::Heater(e) => e.resolve().map(Resolution::Heater),
                // A zone with no ambient temperature to be about has been
                // refused for that already, once, at the world.
                Component::ThermalZone(e) => match ambient {
                    Some(ambient) => {
                        e.resolve(ambient).map(Resolution::ThermalZone)
                    }
                    None => continue,
                },
                Component::Sun(e) => e.resolve().map(Resolution::Sun),
                Component::SolarArray(e) => {
                    e.resolve().map(Resolution::SolarArray)
                }
                Component::Load(e) => e.resolve().map(Resolution::Load),
                Component::Bus(_) => Ok(Resolution::Bus),
                Component::Battery(e) => e.resolve().map(Resolution::Battery),
                Component::Engine(e) => e.resolve().map(Resolution::Engine),
                Component::Body(e) => e.resolve().map(Resolution::Body),
                Component::ThermalMass(_) => {
                    Err(MASS_MADE_OF_PARTS.to_string())
                }
            };
            match what {
                Ok(what) => model.components.push(Resolved {
                    name: name.to_string(),
                    what,
                }),
                Err(why) => problems.push(PhysicsProblem::of(name, why)),
            }
        }

        // The connections only where every component read: one naming a
        // component that did not would be reported as naming nothing, which
        // is the wrong fault.
        if !problems.is_empty() {
            return (model, problems);
        }
        for connection in &self.physics.connections {
            match joined(&model, connection) {
                Ok(joined) => model.connections.push(joined),
                Err(why) => problems.push(PhysicsProblem::joining(why)),
            }
        }
        problems
            .extend(crowded(&model).into_iter().map(PhysicsProblem::joining));
        // In the order of what they join, by name, and not as the file lists
        // them: an input that sums is given its outputs in this order.
        let mut connections = std::mem::take(&mut model.connections);
        connections.sort_by_cached_key(|c| {
            (model.name_of(c.from), model.name_of(c.to))
        });
        model.connections = connections;
        (model, problems)
    }
}

/// Finds a connection's two ends, and checks they can be joined.
///
/// # Arguments
///
/// * `model`      - The model, with its components read.
/// * `connection` - The connection.
///
/// # Returns
///
/// The connection, by place.
///
/// # Errors
///
/// A message for an end that names nothing, a `from` that is not an
/// output, a `to` that is not an input, ends of different kinds, or a
/// connection made twice.
fn joined(model: &Model, connection: &Connection) -> Result<Joined, String> {
    let from = At::parse("from", &connection.from)?;
    let to = At::parse("to", &connection.to)?;
    let (from_place, from_terminal) = model.find(&from)?;
    let (to_place, to_terminal) = model.find(&to)?;
    if from_terminal.direction != Direction::Out {
        return Err(format!(
            "from {from}, which is an input: a connection goes from an output \
             to an input"
        ));
    }
    if to_terminal.direction != Direction::In {
        return Err(format!(
            "to {to}, which is an output: a connection goes from an output to \
             an input"
        ));
    }
    if from_terminal.kind != to_terminal.kind {
        return Err(format!(
            "{from} gives {} and {to} takes {}: a connection joins two \
             terminals of one kind",
            from_terminal.kind.describe(),
            to_terminal.kind.describe()
        ));
    }
    let joined = Joined {
        from: from_place,
        to: to_place,
    };
    if model.connections.contains(&joined) {
        return Err(format!(
            "from {from} to {to} is made twice, and would count twice"
        ));
    }
    Ok(joined)
}

/// Finds every input that takes one output and was given more.
///
/// # Arguments
///
/// * `model` - The model, with its connections found.
///
/// # Returns
///
/// A message for each.
fn crowded(model: &Model) -> Vec<String> {
    let mut counted: BTreeMap<Place, Vec<Place>> = BTreeMap::new();
    for joined in &model.connections {
        counted.entry(joined.to).or_default().push(joined.from);
    }
    counted
        .iter()
        .filter(|(to, from)| {
            from.len() > 1
                && !model.components[to.component].what.terminals()[to.terminal]
                    .sums
        })
        .map(|(to, from)| {
            let named: Vec<String> =
                from.iter().map(|f| model.name_of(*f)).collect();
            format!(
                "{} takes one output, and is given {}",
                model.name_of(*to),
                named.join(" and ")
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::Unit;

    /// Reads a model's components and connections, written as they would be
    /// at the left margin, as the physics of a world whose ambient
    /// temperature is 283.15 K.
    ///
    /// # Arguments
    ///
    /// * `text` - The physics section's value.
    ///
    /// # Returns
    ///
    /// The world.
    fn parse(text: &str) -> TcsResult<WorldSection> {
        let physics: String =
            text.lines().map(|line| format!("  {line}\n")).collect();
        WorldSection::parse(&format!(
            "ambient_temp: \"283.15 K\"\nphysics:\n{physics}"
        ))
    }

    /// The thermostat's model: two heaters warming one zone.
    const ZONE: &str = "\
components:
  - type thermal_zone:
      - component: zone
        time_constant: 50 s
        capacity: \"20 J/K\"
  - type heater:
      - component: heater_2
        power: \"60 W\"
        voltage: \"28 V\"
        ripple: \"0.02\"
        ripple_time: 20 s
      - component: heater_1
        power: \"60 W\"
        voltage: \"28 V\"
connections:
  - from: heater_2.heat
    to: zone.heat
  - from: heater_1.heat
    to: zone.heat
";

    /// A model's components are typed, their figures read in their units, and
    /// its connections are found; and each is held in name order, however
    /// the file wrote them.
    #[test]
    fn a_model_holds_typed_components_and_their_connections() {
        let model = parse(ZONE)
            .expect("it parses")
            .resolve()
            .expect("it resolves");
        assert_eq!(model.components.len(), 3);
        let names: Vec<&str> =
            model.components.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["heater_1", "heater_2", "zone"]);
        assert_eq!(model.name_of(model.connections[0].from), "heater_1.heat");
        let Resolution::Heater(heater) = &model.components[1].what else {
            panic!("a heater: {:?}", model.components[1]);
        };
        assert_eq!((heater.power, heater.voltage), (60.0, 28.0));
        assert_eq!(heater.supply.size, 0.02);
        let Resolution::ThermalZone(zone) = &model.components[2].what else {
            panic!("a zone: {:?}", model.components[2]);
        };
        assert_eq!((zone.environment, zone.capacity), (283.15, 20.0));
        assert_eq!(model.connections.len(), 2);
        assert_eq!(model.name_of(model.connections[1].from), "heater_2.heat");
        assert_eq!(model.name_of(model.connections[1].to), "zone.heat");

        let (_, terminal) = model
            .find(&At::parse("t", "zone.temperature").expect("a terminal"))
            .expect("it is there");
        assert_eq!(terminal.kind, terminal::Kind::Value(Unit::Kelvin));
    }

    /// A model written in another order is the same model: its components,
    /// and so their places, by name, and its connections by what they
    /// join, so that the world made of it sums and breaks ties alike.
    #[test]
    fn a_model_in_another_order_is_the_same_model() {
        let other = "\
connections:
  - from: heater_1.heat
    to: zone.heat
  - from: heater_2.heat
    to: zone.heat
components:
  - type heater:
      - component: heater_1
        voltage: \"28 V\"
        power: \"60 W\"
      - component: heater_2
        ripple_time: 20 s
        ripple: \"0.02\"
        voltage: \"28 V\"
        power: \"60 W\"
  - type thermal_zone:
      - component: zone
        capacity: \"20 J/K\"
        time_constant: 50 s
";
        let one = parse(ZONE).expect("parses");
        let two = parse(other).expect("parses");
        assert_eq!(one.physics.components, two.physics.components);
        assert_eq!(
            one.resolve().expect("resolves"),
            two.resolve().expect("resolves")
        );
    }

    /// A component of no type, or of one there is none of, or with a
    /// setting its type has not, is refused, naming what is wrong.
    #[test]
    fn an_component_of_no_known_type_is_refused() {
        for (text, said) in [
            ("components:\n  - component: zone\n", "is not a type"),
            (
                "components:\n  - type boiler:\n      - component: zone\n",
                "boiler",
            ),
            ("components:\n  - type heater:\n", "no components"),
            (
                "components:\n  - type heater:\n      - component: h\n        \
                 power: \"1 W\"\n        voltage: \"1 V\"\n        \
                 colour: red\n",
                "colour",
            ),
            (
                "components:\n  - type bus:\n      - component: a\n  \
                 - type bus:\n      - component: b\n",
                "written twice",
            ),
            (
                "components:\n  - type bus:\n      - component: a\n        \
                 type: bus\n",
                "type",
            ),
        ] {
            let why = parse(text).expect_err(text).to_string();
            assert!(why.contains(said), "{text}: {why}");
        }
    }

    /// A thermal mass, which a model held before it was made of parts, is
    /// refused, saying what it is now.
    #[test]
    fn a_thermal_mass_is_told_what_it_is_now() {
        let file = parse(
            "components:\n  - type thermal_mass:\n      \
             - component: basic\n        \
             environment: \"283.15\"\n        inputs: \"in1\"\n",
        )
        .expect("it parses, to be refused");
        let problem = file.resolve().expect_err("refused");
        assert_eq!(problem.about(), About::PhysicsComponent("basic".into()));
        assert!(problem.why.contains("thermal_zone"), "{problem}");
        assert!(problem.why.contains("connects:"), "{problem}");
    }

    /// Every type listed reads, with the settings its list names, and has
    /// terminals.
    #[test]
    fn every_type_listed_reads_with_its_settings() {
        for name in TYPES {
            let settings = settings_of(name).expect("its settings");
            assert!(terminals_of(name).is_some_and(|t| !t.is_empty()));
            let mut text = format!(
                "components:\n  - type {name}:\n      - component: x\n"
            );
            for setting in settings {
                text.push_str(&format!("        {setting}: \"1\"\n"));
            }
            assert!(parse(&text).is_ok(), "{text}");
        }
    }

    /// A figure without its unit is refused, at its component.
    #[test]
    fn a_figure_without_its_unit_is_refused() {
        let text = ZONE.replace("\"60 W\"", "\"60\"");
        let problems = parse(&text).expect("parses").problems();
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert_eq!(problems[0].component.as_deref(), Some("heater_1"));
        assert!(problems[0].why.contains("\"60 W\""), "{problems:?}");
    }

    /// Each way a connection can be wrong is refused, saying which.
    #[test]
    fn a_wrong_connection_is_refused() {
        for (from, to, says) in [
            (
                "heater_1.heat",
                "zone.nowhere",
                "has no terminal \"nowhere\"",
            ),
            ("heater_9.heat", "zone.heat", "no component \"heater_9\""),
            ("zone.heat", "heater_1.on", "which is an input"),
            ("heater_1.heat", "zone.temperature", "which is an output"),
            (
                "heater_1.current",
                "zone.heat",
                "amperes and zone.heat takes",
            ),
            ("heater_1.heat", "zone.heat", "made twice"),
            ("heater_1", "zone.heat", "joined by a dot"),
        ] {
            let text = format!("{ZONE}  - from: {from}\n    to: {to}\n");
            let problems = parse(&text).expect("parses").problems();
            assert!(
                problems
                    .iter()
                    .any(|p| p.component.is_none() && p.why.contains(says)),
                "{from} -> {to}: {problems:?}"
            );
        }
    }

    /// Two components of one name, or a name with a dot, are refused.
    #[test]
    fn names_are_one_each_and_dotless() {
        let twice = ZONE.replace("component: heater_2", "component: heater_1");
        let problems = parse(&twice).expect("parses").problems();
        assert!(problems[0].why.contains("two components"), "{problems:?}");
        let dotted = ZONE.replace("component: zone", "component: the.zone");
        let problems = parse(&dotted).expect("parses").problems();
        assert!(problems[0].why.contains("no dot"), "{problems:?}");
    }

    /// A zone takes its surroundings from the world's ambient temperature:
    /// one stating its own environment is told where it is now, and a
    /// world with a zone and no ambient temperature is refused at it, once.
    #[test]
    fn a_zone_is_in_the_worlds_ambient_temperature() {
        let model = parse(ZONE).expect("parses").resolve().expect("resolves");
        let Resolution::ThermalZone(zone) = &model.components[2].what else {
            panic!("a zone: {:?}", model.components[2]);
        };
        assert_eq!(zone.environment, 283.15);

        let own = ZONE.replace(
            "capacity: \"20 J/K\"",
            "capacity: \"20 J/K\"\n        environment: \"283.15 K\"",
        );
        let problem = parse(&own).expect("parses").resolve().unwrap_err();
        assert_eq!(problem.about(), About::PhysicsComponent("zone".into()));
        assert!(problem.why.contains("ambient_temp"), "{problem}");

        let physics: String =
            ZONE.lines().map(|line| format!("  {line}\n")).collect();
        let none = WorldSection::parse(&format!("physics:\n{physics}"))
            .expect("parses");
        let problems = none.problems();
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(problems[0].about(), About::Section(AMBIENT));
        let wrong = WorldSection::parse(&format!(
            "ambient_temp: \"20 C\"\nphysics:\n{physics}"
        ))
        .expect("parses");
        assert_eq!(wrong.problems()[0].about(), About::Section(AMBIENT));
    }
}
