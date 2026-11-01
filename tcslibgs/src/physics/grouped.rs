//! How a physics model writes its components: grouped by type.
//!
//! The `physics` section is a list of entries, one for each type, and a
//! component says its name and its figures but not its type, which is the
//! entry's:
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
//! ```
//!
//! As a configuration groups its nodes -- see [`crate::graph::grouped`] --
//! and for the same reason: a type is said once for every component of it.
//! The order of the entries, and of the components of one, means nothing:
//! the components are held in name order whatever order they are written in,
//! which is the order the world is made and run in. See [`super::Model`].
//!
//! A model written before its components were grouped is refused before it
//! reaches here, by its line and with the shape it has now: see
//! [`crate::sim_reshaped`]. What is refused here is a shape that is neither.

use std::fmt;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

use super::{
    battery, body, bus, engine, heater, load, solar_array, sun, thermal_zone,
    Element, Moved, TYPES,
};

/// Reads the `physics` section.
///
/// # Arguments
///
/// * `d` - The deserializer, at the section's value.
///
/// # Returns
///
/// Every component, of every type, in name order.
///
/// # Errors
///
/// An entry that is not a type and its components, a type there is none of
/// or written twice, a type with no components under it, and whatever a
/// component's own reader refuses.
pub fn components<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Vec<Element>, D::Error> {
    let mut components = d.deserialize_any(SectionVisitor)?;
    components.sort_by(|a, b| a.name().cmp(b.name()));
    Ok(components)
}

/// Says what is wrong with a type an entry names, if anything.
///
/// # Arguments
///
/// * `key` - The entry's key: `type`, one space and the type.
///
/// # Returns
///
/// The type.
///
/// # Errors
///
/// A message for a key that is not the word `type`, one space and one of the
/// [`TYPES`] -- or `thermal_mass`, which is read to be refused, saying what
/// it is now.
fn type_in_key(key: &str) -> Result<String, String> {
    let Some(named) = key.strip_prefix("type ") else {
        return Err(format!(
            "\"{key}\" is not a type: each entry of physics is the word type, \
             a space and the type its components are, as `- type heater:`"
        ));
    };
    if named.starts_with(char::is_whitespace) || named.trim().is_empty() {
        return Err(format!(
            "\"{key}\" is not `type`, one space and a type: write it as `type \
             {}`",
            named.trim()
        ));
    }
    if !TYPES.contains(&named) && named != "thermal_mass" {
        return Err(format!(
            "{named} is not a type of physics component, which are {}",
            TYPES.join(", ")
        ));
    }
    Ok(named.to_string())
}

/// Reads the section's entries.
struct SectionVisitor;

impl<'de> Visitor<'de> for SectionVisitor {
    type Value = Vec<Element>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "a list of types, each `- type heater:` with its components"
        )
    }

    /// A section written with nothing in it.
    fn visit_unit<E: de::Error>(self) -> Result<Vec<Element>, E> {
        Ok(Vec::new())
    }

    fn visit_none<E: de::Error>(self) -> Result<Vec<Element>, E> {
        Ok(Vec::new())
    }

    fn visit_seq<A: SeqAccess<'de>>(
        self,
        mut seq: A,
    ) -> Result<Vec<Element>, A::Error> {
        let mut types: Vec<String> = Vec::new();
        let mut components = Vec::new();
        while let Some((named, theirs)) = seq.next_element_seed(TypeEntry)? {
            if types.contains(&named) {
                return Err(de::Error::custom(format!(
                    "type {named} is written twice in physics: put its \
                     components under one"
                )));
            }
            types.push(named);
            components.extend(theirs);
        }
        Ok(components)
    }
}

/// One entry of the section: `type X:` and its list of components.
struct TypeEntry;

impl<'de> DeserializeSeed<'de> for TypeEntry {
    type Value = (String, Vec<Element>);

    fn deserialize<D: Deserializer<'de>>(
        self,
        d: D,
    ) -> Result<Self::Value, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for TypeEntry {
    type Value = (String, Vec<Element>);

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "an entry of physics naming a type, as `type heater:`, with its \
             components under it"
        )
    }

    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> Result<Self::Value, A::Error> {
        let Some(key) = map.next_key::<String>()? else {
            return Err(de::Error::custom(
                "an entry of physics is empty: it names a type, as `- type \
                 heater:`, with its components under it",
            ));
        };
        let named = type_in_key(&key).map_err(de::Error::custom)?;
        let components = map.next_value_seed(Components(named.clone()))?;
        if let Some(more) = map.next_key::<String>()? {
            return Err(de::Error::custom(format!(
                "the entry for type {named} says {more} as well: an entry of \
                 physics names one type, and its components are the list \
                 under it"
            )));
        }
        if components.is_empty() {
            return Err(de::Error::custom(format!(
                "type {named} in physics has no components under it: write \
                 its components, or leave the type out"
            )));
        }
        Ok((named, components))
    }
}

/// The list of components under a type.
struct Components(String);

impl<'de> DeserializeSeed<'de> for Components {
    type Value = Vec<Element>;

    fn deserialize<D: Deserializer<'de>>(
        self,
        d: D,
    ) -> Result<Vec<Element>, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Components {
    type Value = Vec<Element>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "the list of {} components, each `- component: <name>`",
            self.0
        )
    }

    /// No components, which the type refuses with a message of its own.
    fn visit_unit<E: de::Error>(self) -> Result<Vec<Element>, E> {
        Ok(Vec::new())
    }

    fn visit_none<E: de::Error>(self) -> Result<Vec<Element>, E> {
        Ok(Vec::new())
    }

    fn visit_seq<A: SeqAccess<'de>>(
        self,
        mut seq: A,
    ) -> Result<Vec<Element>, A::Error> {
        let mut components = Vec::new();
        while let Some(one) = seq.next_element_seed(Component(&self.0))? {
            components.push(one);
        }
        Ok(components)
    }
}

/// One component, of the type its entry has said.
struct Component<'a>(&'a str);

impl<'de> DeserializeSeed<'de> for Component<'_> {
    type Value = Element;

    /// Reads it with its type's own reader, which refuses a word the type
    /// does not take, at its line.
    fn deserialize<D: Deserializer<'de>>(
        self,
        d: D,
    ) -> Result<Element, D::Error> {
        Ok(match self.0 {
            "heater" => Element::Heater(heater::Heater::deserialize(d)?),
            "thermal_zone" => {
                Element::ThermalZone(thermal_zone::ThermalZone::deserialize(d)?)
            }
            "sun" => Element::Sun(sun::Sun::deserialize(d)?),
            "solar_array" => {
                Element::SolarArray(solar_array::SolarArray::deserialize(d)?)
            }
            "load" => Element::Load(load::Load::deserialize(d)?),
            "bus" => Element::Bus(bus::Bus::deserialize(d)?),
            "battery" => Element::Battery(battery::Battery::deserialize(d)?),
            "engine" => Element::Engine(engine::Engine::deserialize(d)?),
            "body" => Element::Body(body::Body::deserialize(d)?),
            "thermal_mass" => Element::ThermalMass(Moved::deserialize(d)?),
            other => {
                return Err(de::Error::custom(format!(
                    "{other} is not a type of physics component"
                )))
            }
        })
    }
}
