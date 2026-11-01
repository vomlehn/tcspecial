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
//! # What it holds
//!
//! Its components, grouped by type as a configuration groups its nodes, and
//! the connections between them, as a configuration holds links:
//!
//! ```yaml
//! physics:
//!   - type heater:
//!       - component: heater_1
//!         power: "60 W"
//!         voltage: "28 V"
//!   - type thermal_zone:
//!       - component: zone
//!         environment: "283.15 K"
//!         time_constant: "50 s"
//!         capacity: "20 J/K"
//!
//! connections:
//!   - from: heater_1.heat
//!     to: zone.heat
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
//! A stand-in in the simulator file is joined to a terminal by naming it,
//! `connects: zone.temperature`: a sensor reports an output, and a
//! stand-in the vehicle drives drives an input. This file says only what the
//! world is.
//!
//! # Its figures
//!
//! Each written with its unit, which is checked: see [`setting`].
//!
//! YAML only. A model is written by hand or by a tool that writes YAML, and
//! a second spelling would be a second thing to keep the same as the first.

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
use std::path::Path;

use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use crate::error::{TcsError, TcsResult};
use crate::verify::About;
pub use setting::Wander;
use terminal::{At, Direction, Terminal};

/// Every element type a physics model may hold, by the name a file writes
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

/// A physics model, as it parses.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PhysicsFile {
    /// What the model is of.
    #[serde(default)]
    pub description: Option<String>,
    /// Its components, in name order, whatever order the file writes them
    /// in: see [`grouped`].
    #[serde(default, deserialize_with = "grouped::components")]
    pub physics: Vec<Element>,
    /// How its elements are joined, each an output to an input.
    #[serde(default)]
    pub connections: Vec<Connection>,
}

/// One element of a physics model, of one of the [`TYPES`]: a component, as
/// a file names it.
///
/// Read by [`grouped`], the type being the entry's it is written under, and
/// written out with its type beside it for the simulator file's digest: see
/// [`crate::config_digest::interpretation_of_simulator`].
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type")]
pub enum Element {
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

/// An element of a type there is no longer, read only to be refused.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Moved {
    #[serde(rename = "component")]
    pub name: String,
    #[serde(flatten, skip_serializing)]
    pub rest: BTreeMap<String, IgnoredAny>,
}

/// What a model with a `thermal_mass` is told.
pub const MASS_MADE_OF_PARTS: &str = "a thermal_mass is now made of parts: \
    a thermal_zone, with environment, time_constant and a capacity in J/K, \
    and a heater for each heater, with its power in W and its voltage in V, \
    joined under connections: from each heater's heat to the zone's heat. A \
    stand-in then names the terminal it is joined to, connects: \
    zone.temperature or connects: heater_1.on, in place of reads, heats, \
    output, input and rate. examples/thermostat/thermostat-physics.yaml is \
    one written so";

/// A connection between two elements, as a file writes it.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    /// An output: `heater_1.heat`.
    pub from: String,
    /// An input: `zone.heat`.
    pub to: String,
}

impl Element {
    /// What it is called.
    ///
    /// # Returns
    ///
    /// Its name, as the file writes it.
    pub fn name(&self) -> &str {
        match self {
            Element::Heater(e) => &e.name,
            Element::ThermalZone(e) => &e.name,
            Element::Sun(e) => &e.name,
            Element::SolarArray(e) => &e.name,
            Element::Load(e) => &e.name,
            Element::Bus(e) => &e.name,
            Element::Battery(e) => &e.name,
            Element::Engine(e) => &e.name,
            Element::Body(e) => &e.name,
            Element::ThermalMass(e) => &e.name,
        }
    }

    /// The type a file writes it as.
    ///
    /// # Returns
    ///
    /// Its type, as a file writes it.
    pub fn type_name(&self) -> &'static str {
        match self {
            Element::Heater(_) => "heater",
            Element::ThermalZone(_) => "thermal_zone",
            Element::Sun(_) => "sun",
            Element::SolarArray(_) => "solar_array",
            Element::Load(_) => "load",
            Element::Bus(_) => "bus",
            Element::Battery(_) => "battery",
            Element::Engine(_) => "engine",
            Element::Body(_) => "body",
            Element::ThermalMass(_) => "thermal_mass",
        }
    }
}

/// The settings an element of a type states, the `component` naming it
/// apart: the one list for each, from which an editor builds an element's
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

/// The terminals of an element of a type.
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

/// Something wrong with a physics model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicsProblem {
    /// The element it is about, or `None` for one about the connections.
    pub element: Option<String>,
    /// What is wrong.
    pub why: String,
}

impl PhysicsProblem {
    /// Makes a problem with an element.
    ///
    /// # Arguments
    ///
    /// * `element` - The element's name.
    /// * `why`     - What is wrong.
    ///
    /// # Returns
    ///
    /// The problem.
    fn of(element: &str, why: impl Into<String>) -> PhysicsProblem {
        PhysicsProblem {
            element: Some(element.to_string()),
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
            element: None,
            why: why.into(),
        }
    }

    /// What it is about, which is how the line it belongs to is found in
    /// the model's file.
    ///
    /// # Returns
    ///
    /// Its element, or the connections.
    pub fn about(&self) -> About {
        match &self.element {
            Some(name) => About::PhysicsElement(name.clone()),
            None => About::Section("connections"),
        }
    }
}

impl std::fmt::Display for PhysicsProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.element {
            Some(name) => {
                write!(f, "physics component \"{name}\": {}", self.why)
            }
            None => write!(f, "connections: {}", self.why),
        }
    }
}

/// A physics model with every figure read and every connection found.
///
/// In one order, whatever order its file is written in: the elements by
/// name, so that an element's place -- by which the world holds it, and by
/// which two of its events due at one moment are taken -- is its place among
/// the names; and the connections by the names of the terminals they join,
/// from and then to, so that what is summed at an input is summed in one
/// order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Model {
    /// Its elements, in name order.
    pub elements: Vec<Resolved>,
    /// Its connections, each from an output to an input, in the order of
    /// the names of the terminals they join.
    pub connections: Vec<Joined>,
}

/// One element with its figures read.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub name: String,
    pub what: Resolution,
}

/// What an element is, with its figures read.
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

/// A terminal of a model, by place: which element, and which of its type's
/// terminals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place {
    pub element: usize,
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
    /// * `at` - The element and terminal, as named.
    ///
    /// # Returns
    ///
    /// Where it is, and what it is.
    ///
    /// # Errors
    ///
    /// A message naming what there is, for an element the model has not or
    /// a terminal its type has not.
    pub fn find(&self, at: &At) -> Result<(Place, Terminal), String> {
        let Some(element) =
            self.elements.iter().position(|e| e.name == at.element)
        else {
            let every: Vec<&str> =
                self.elements.iter().map(|e| e.name.as_str()).collect();
            return Err(format!(
                "{at}: the physics model has no component \"{}\"; it has {}",
                at.element,
                if every.is_empty() {
                    "none".to_string()
                } else {
                    every.join(", ")
                }
            ));
        };
        let what = &self.elements[element].what;
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
        Ok((Place { element, terminal }, terminals[terminal]))
    }

    /// Says what a terminal is.
    ///
    /// # Arguments
    ///
    /// * `place` - A terminal of this model.
    ///
    /// # Returns
    ///
    /// Its element's name and its own, joined as a file writes them.
    pub fn name_of(&self, place: Place) -> String {
        let element = &self.elements[place.element];
        format!(
            "{}.{}",
            element.name,
            element.what.terminals()[place.terminal].name
        )
    }
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
        PhysicsFile::parse(&text).map_err(|e| match e {
            TcsError::Config(why) => named(why),
            other => named(other.to_string()),
        })
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
    /// What the YAML reader says, with the line it says it at, and a model
    /// in the shape it had before its components were grouped by type, at
    /// its line.
    pub fn parse(text: &str) -> TcsResult<PhysicsFile> {
        // A model written before its components were grouped by type,
        // refused by its line and shown the shape now.
        if let Some(why) = crate::sim_reshaped::physics_refused(text) {
            return Err(TcsError::Config(why));
        }
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
    /// The first of [`PhysicsFile::problems`].
    pub fn resolve(&self) -> Result<Model, PhysicsProblem> {
        let (model, mut problems) = self.built();
        if problems.is_empty() {
            Ok(model)
        } else {
            Err(problems.remove(0))
        }
    }

    /// Everything wrong with the model as it stands alone: an element whose
    /// figures do not read, two of one name, a connection that names no
    /// terminal, joins two outputs or two inputs, joins a value in one unit
    /// to an input in another, or is a second output on an input that takes
    /// one.
    ///
    /// What the stand-ins are joined to is the simulator file's, and checked
    /// there: see `crate::sim_config::SimConfigFile::physics`.
    ///
    /// # Returns
    ///
    /// Each problem, the elements' first, in name order.
    pub fn problems(&self) -> Vec<PhysicsProblem> {
        self.built().1
    }

    /// The model as far as it reads, and what is wrong with it.
    ///
    /// # Returns
    ///
    /// Every element that read and every connection found, and each
    /// problem.
    fn built(&self) -> (Model, Vec<PhysicsProblem>) {
        let mut problems = Vec::new();
        let mut model = Model::default();
        // By name, as [`grouped`] holds them already: a model made some
        // other way is in one order too.
        let mut elements: Vec<&Element> = self.physics.iter().collect();
        elements.sort_by(|a, b| a.name().cmp(b.name()));
        for element in elements {
            let name = element.name();
            if name.trim().is_empty() || name.contains('.') {
                problems.push(PhysicsProblem::of(
                    name,
                    "a name is not empty and has no dot in it: a terminal is \
                     written as the name, a dot and the terminal's",
                ));
                continue;
            }
            if model.elements.iter().any(|e| e.name == name) {
                problems.push(PhysicsProblem::of(
                    name,
                    "two components have this name, and a connection or a \
                     stand-in names one",
                ));
                continue;
            }
            let what = match element {
                Element::Heater(e) => e.resolve().map(Resolution::Heater),
                Element::ThermalZone(e) => {
                    e.resolve().map(Resolution::ThermalZone)
                }
                Element::Sun(e) => e.resolve().map(Resolution::Sun),
                Element::SolarArray(e) => {
                    e.resolve().map(Resolution::SolarArray)
                }
                Element::Load(e) => e.resolve().map(Resolution::Load),
                Element::Bus(_) => Ok(Resolution::Bus),
                Element::Battery(e) => e.resolve().map(Resolution::Battery),
                Element::Engine(e) => e.resolve().map(Resolution::Engine),
                Element::Body(e) => e.resolve().map(Resolution::Body),
                Element::ThermalMass(_) => Err(MASS_MADE_OF_PARTS.to_string()),
            };
            match what {
                Ok(what) => model.elements.push(Resolved {
                    name: name.to_string(),
                    what,
                }),
                Err(why) => problems.push(PhysicsProblem::of(name, why)),
            }
        }

        // The connections only where every element read: one naming an
        // element that did not would be reported as naming nothing, which
        // is the wrong fault.
        if !problems.is_empty() {
            return (model, problems);
        }
        for connection in &self.connections {
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
/// * `model`      - The model, with its elements read.
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
                && !model.elements[to.element].what.terminals()[to.terminal]
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

    /// The thermostat's model: two heaters warming one zone.
    const ZONE: &str = "\
physics:
  - type thermal_zone:
      - component: zone
        environment: \"283.15 K\"
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

    /// A model's elements are typed, their figures read in their units, and
    /// its connections are found; and each is held in name order, however
    /// the file wrote them.
    #[test]
    fn a_model_holds_typed_elements_and_their_connections() {
        let model = PhysicsFile::parse(ZONE)
            .expect("it parses")
            .resolve()
            .expect("it resolves");
        assert_eq!(model.elements.len(), 3);
        let names: Vec<&str> =
            model.elements.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["heater_1", "heater_2", "zone"]);
        assert_eq!(model.name_of(model.connections[0].from), "heater_1.heat");
        let Resolution::Heater(heater) = &model.elements[1].what else {
            panic!("a heater: {:?}", model.elements[1]);
        };
        assert_eq!((heater.power, heater.voltage), (60.0, 28.0));
        assert_eq!(heater.supply.size, 0.02);
        let Resolution::ThermalZone(zone) = &model.elements[2].what else {
            panic!("a zone: {:?}", model.elements[2]);
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
physics:
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
        environment: \"283.15 K\"
";
        let one = PhysicsFile::parse(ZONE).expect("parses");
        let two = PhysicsFile::parse(other).expect("parses");
        assert_eq!(one.physics, two.physics);
        assert_eq!(
            one.resolve().expect("resolves"),
            two.resolve().expect("resolves")
        );
    }

    /// An element of no type, or of one there is none of, or with a
    /// setting its type has not, is refused, naming what is wrong.
    #[test]
    fn an_element_of_no_known_type_is_refused() {
        for (text, said) in [
            ("physics:\n  - component: zone\n", "is not a type"),
            (
                "physics:\n  - type boiler:\n      - component: zone\n",
                "boiler",
            ),
            ("physics:\n  - type heater:\n", "no components"),
            (
                "physics:\n  - type heater:\n      - component: h\n        \
                 power: \"1 W\"\n        voltage: \"1 V\"\n        \
                 colour: red\n",
                "colour",
            ),
            (
                "physics:\n  - type bus:\n      - component: a\n  \
                 - type bus:\n      - component: b\n",
                "written twice",
            ),
            (
                "physics:\n  - type bus:\n      - component: a\n        \
                 type: bus\n",
                "type",
            ),
        ] {
            let why = PhysicsFile::parse(text).expect_err(text).to_string();
            assert!(why.contains(said), "{text}: {why}");
        }
    }

    /// A thermal mass, which a model held before it was made of parts, is
    /// refused, saying what it is now.
    #[test]
    fn a_thermal_mass_is_told_what_it_is_now() {
        let file = PhysicsFile::parse(
            "physics:\n  - type thermal_mass:\n      \
             - component: basic\n        \
             environment: \"283.15\"\n        inputs: \"in1\"\n",
        )
        .expect("it parses, to be refused");
        let problem = file.resolve().expect_err("refused");
        assert_eq!(problem.about(), About::PhysicsElement("basic".into()));
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
            let mut text =
                format!("physics:\n  - type {name}:\n      - component: x\n");
            for setting in settings {
                text.push_str(&format!("        {setting}: \"1\"\n"));
            }
            assert!(PhysicsFile::parse(&text).is_ok(), "{text}");
        }
    }

    /// A figure without its unit is refused, at its element.
    #[test]
    fn a_figure_without_its_unit_is_refused() {
        let text = ZONE.replace("\"60 W\"", "\"60\"");
        let problems = PhysicsFile::parse(&text).expect("parses").problems();
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert_eq!(problems[0].element.as_deref(), Some("heater_1"));
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
            let problems =
                PhysicsFile::parse(&text).expect("parses").problems();
            assert!(
                problems
                    .iter()
                    .any(|p| p.element.is_none() && p.why.contains(says)),
                "{from} -> {to}: {problems:?}"
            );
        }
    }

    /// Two elements of one name, or a name with a dot, are refused.
    #[test]
    fn names_are_one_each_and_dotless() {
        let twice = ZONE.replace("component: heater_2", "component: heater_1");
        let problems = PhysicsFile::parse(&twice).expect("parses").problems();
        assert!(problems[0].why.contains("two components"), "{problems:?}");
        let dotted = ZONE.replace("component: zone", "component: the.zone");
        let problems = PhysicsFile::parse(&dotted).expect("parses").problems();
        assert!(problems[0].why.contains("no dot"), "{problems:?}");
    }

    /// A model is YAML, and a file named otherwise is refused by its name.
    #[test]
    fn a_model_is_yaml() {
        let why = PhysicsFile::load("zone.xml").expect_err("not YAML");
        assert!(why.to_string().contains("is YAML"), "{why}");
    }
}
