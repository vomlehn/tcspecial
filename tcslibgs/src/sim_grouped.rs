//! How a simulator file writes its entries: the simulated payloads grouped by
//! type, and the groups of settings each named by the word `group`.
//!
//! ```yaml
//! simulated_payload_groups:
//!   - group: zone_temps
//!     packet_interval: 100 ms
//!
//! simulated_payloads:
//!   - type network:
//!       - payload: temp_1
//!         protocol: udp
//!         group: zone_temps
//! ```
//!
//! ```xml
//! <simulated_payload_groups>
//!     <group name="zone_temps">
//!         <packet_interval>100 ms</packet_interval>
//!     </group>
//! </simulated_payload_groups>
//! <simulated_payloads>
//!     <type name="network">
//!         <payload name="temp_1">
//!             <protocol>udp</protocol>
//!             <group>zone_temps</group>
//!         </payload>
//!     </type>
//! </simulated_payloads>
//! ```
//!
//! A simulated payload's type is the kind of the payload it simulates, in the
//! configuration's word -- the type that payload is written under there -- and
//! is said once, by the entry its payloads are grouped under. It is the
//! configuration's and not this file's, and is said here only to be compared
//! with it: see [`crate::sim_config`], which refuses a payload whose type the
//! configuration does not give it. A group says no type, a payload's being the
//! one it is written under.
//!
//! The order of the types, of the payloads of one and of the groups means
//! nothing: each list is held in name order, whatever order it is written
//! in.
//!
//! Read by hand, as a configuration's nodes are -- see
//! [`crate::graph::grouped`] -- because the two languages give the one shape
//! differently: a list of entries in YAML, and an element of named elements
//! in XML. A payload's and a group's settings are read by their own derived
//! readers, given the entry with its name under the key `name`, so that a
//! setting is checked as it always was. A file in the shape before this one
//! is refused before it reaches here: see [`crate::sim_reshaped`].

use std::fmt;
use std::marker::PhantomData;

use serde::de::value::MapAccessDeserializer;
use serde::de::DeserializeOwned;
use serde::de::{
    self, DeserializeSeed, IntoDeserializer, MapAccess, SeqAccess, Visitor,
};
use serde::Deserializer;

use crate::sim_config::{SimGroup, SimPayload};
use crate::types::DHType;

/// An entry of either list, as some reader holds it: the simulator's own,
/// [`SimPayload`] and [`SimGroup`], or an editor's, which keeps every setting
/// as the words it is.
///
/// Read by its own derived reader, from the entry's map with the key that
/// names it given as `name`: see [`Named`].
pub trait SimEntry: DeserializeOwned {
    /// What the entry is called.
    ///
    /// # Returns
    ///
    /// Its name, or an empty one where the file gave it none.
    fn name(&self) -> &str;

    /// Gives a simulated payload the type it is written under. A group has
    /// none, and takes nothing.
    ///
    /// # Arguments
    ///
    /// * `kind` - The type, as the file writes it.
    fn written_under(&mut self, kind: &str) {
        let _ = kind;
    }
}

impl SimEntry for SimPayload {
    fn name(&self) -> &str {
        &self.name
    }

    fn written_under(&mut self, kind: &str) {
        self.settings.dh_type = Some(kind.to_string());
    }
}

impl SimEntry for SimGroup {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Which language a section is written in, as its shape shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Language {
    Yaml,
    Xml,
}

/// Which of the two kinds of entry is being read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    /// A simulated payload, named by `payload`.
    Payload,
    /// A group of settings, named by `group`.
    Group,
}

impl Entry {
    /// The word a YAML entry of this kind is named by.
    ///
    /// # Returns
    ///
    /// `payload` or `group`.
    fn word(self) -> &'static str {
        match self {
            Entry::Payload => "payload",
            Entry::Group => "group",
        }
    }

    /// What an entry of this kind is called in a message.
    ///
    /// # Returns
    ///
    /// `simulated payload` or `simulated payload group`.
    fn what(self) -> &'static str {
        match self {
            Entry::Payload => "simulated payload",
            Entry::Group => "simulated payload group",
        }
    }

    /// Says how an entry of this kind is written, for a message.
    ///
    /// # Arguments
    ///
    /// * `language` - The language it is written in.
    ///
    /// # Returns
    ///
    /// `- payload: <name>`, `<group name="...">` and so on.
    fn spelled(self, language: Language) -> String {
        let word = self.word();
        match language {
            Language::Yaml => format!("`- {word}: <name>`"),
            Language::Xml => format!("<{word} name=\"...\">"),
        }
    }
}

/// Reads the `simulated_payloads` section.
///
/// # Arguments
///
/// * `d` - The deserializer, at the section's value.
///
/// # Returns
///
/// Every simulated payload, of every type, its type set from the entry it
/// is written under, in name order.
///
/// # Errors
///
/// An entry that is not a type and its payloads, a type that is no kind of
/// payload or that is written twice, a type with no payloads, and whatever
/// an entry's own reader refuses: see [`Named`].
pub fn payloads<'de, D: Deserializer<'de>, P: SimEntry>(
    d: D,
) -> Result<Vec<P>, D::Error> {
    let mut payloads: Vec<P> =
        d.deserialize_any(PayloadsVisitor(PhantomData))?;
    payloads.sort_by(|a, b| a.name().cmp(b.name()));
    Ok(payloads)
}

/// Reads the `simulated_payload_groups` section.
///
/// # Arguments
///
/// * `d` - The deserializer, at the section's value.
///
/// # Returns
///
/// Every group, in name order.
///
/// # Errors
///
/// An entry that is not a group, and whatever an entry's own reader
/// refuses: see [`Named`].
pub fn groups<'de, D: Deserializer<'de>, G: SimEntry>(
    d: D,
) -> Result<Vec<G>, D::Error> {
    let mut groups: Vec<G> = d.deserialize_any(GroupsVisitor(PhantomData))?;
    groups.sort_by(|a, b| a.name().cmp(b.name()));
    Ok(groups)
}

/// Says what is wrong with a type a simulated payload is written under, if
/// anything.
///
/// # Arguments
///
/// * `named` - The type, as the file wrote it.
///
/// # Returns
///
/// `Ok(())` for a kind of payload.
///
/// # Errors
///
/// A message for anything else, listing the kinds.
fn kind_of(named: &str) -> Result<(), String> {
    if DHType::from_spelling(named).is_some() {
        return Ok(());
    }
    if named.trim().is_empty() || named.chars().any(char::is_whitespace) {
        return Err(format!(
            "\"{named}\" is not a type: a type is one word, the kind of the \
             payloads written under it"
        ));
    }
    Err(format!(
        "{named} is not a kind of payload that can be simulated, \
         which are {}",
        DHType::spellings()
    ))
}

/// Says how a key read from an XML element was written, for a message.
///
/// # Arguments
///
/// * `key` - The key, as quick-xml gives it.
///
/// # Returns
///
/// `the attribute x`, `text` or `<x>`.
fn shown(key: &str) -> String {
    if let Some(attribute) = key.strip_prefix('@') {
        format!("the attribute {attribute}")
    } else if key == "$text" || key == "$value" {
        "text".to_string()
    } else {
        format!("<{key}>")
    }
}

/// Reads the payloads section's types, in whichever language.
struct PayloadsVisitor<P>(PhantomData<P>);

impl<P: SimEntry> PayloadsVisitor<P> {
    /// Adds a type's payloads to those read, refusing a type written twice.
    ///
    /// # Arguments
    ///
    /// * `types`    - The types read so far.
    /// * `payloads` - The payloads read so far.
    /// * `typed`    - The next type and its payloads.
    ///
    /// # Errors
    ///
    /// A message for a type already written.
    fn add<E: de::Error>(
        types: &mut Vec<String>,
        payloads: &mut Vec<P>,
        typed: (String, Vec<P>),
    ) -> Result<(), E> {
        let (named, theirs) = typed;
        if types.contains(&named) {
            return Err(E::custom(format!(
                "type {named} is written twice in simulated_payloads: put its \
                 payloads under one"
            )));
        }
        types.push(named);
        payloads.extend(theirs);
        Ok(())
    }
}

impl<'de, P: SimEntry> Visitor<'de> for PayloadsVisitor<P> {
    type Value = Vec<P>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "the simulated payloads, grouped by type")
    }

    /// A section written with nothing in it.
    fn visit_unit<E: de::Error>(self) -> Result<Vec<P>, E> {
        Ok(Vec::new())
    }

    fn visit_none<E: de::Error>(self) -> Result<Vec<P>, E> {
        Ok(Vec::new())
    }

    /// An empty XML element, which arrives as no text.
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Vec<P>, E> {
        if v.trim().is_empty() {
            return Ok(Vec::new());
        }
        Err(E::custom(format!(
            "simulated_payloads holds types and their payloads, not \"{v}\""
        )))
    }

    /// YAML: a list of entries, each a type and its payloads.
    fn visit_seq<A: SeqAccess<'de>>(
        self,
        mut seq: A,
    ) -> Result<Vec<P>, A::Error> {
        let (mut types, mut payloads) = (Vec::new(), Vec::new());
        while let Some(typed) = seq.next_element_seed(YamlType(PhantomData))? {
            Self::add(&mut types, &mut payloads, typed)?;
        }
        Ok(payloads)
    }

    /// XML: an element of `<type>` elements.
    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> Result<Vec<P>, A::Error> {
        let (mut types, mut payloads) = (Vec::new(), Vec::new());
        while let Some(key) = map.next_key::<String>()? {
            if key != "type" {
                return Err(de::Error::custom(format!(
                    "<simulated_payloads> holds <type name=\"...\"> elements \
                     and nothing else, and this one holds {}",
                    shown(&key)
                )));
            }
            let typed = map.next_value_seed(XmlType(PhantomData))?;
            Self::add(&mut types, &mut payloads, typed)?;
        }
        Ok(payloads)
    }
}

/// One YAML entry of the payloads section: `type X:` and its payloads.
struct YamlType<P>(PhantomData<P>);

impl<'de, P: SimEntry> DeserializeSeed<'de> for YamlType<P> {
    type Value = (String, Vec<P>);

    fn deserialize<D: Deserializer<'de>>(
        self,
        d: D,
    ) -> Result<Self::Value, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de, P: SimEntry> Visitor<'de> for YamlType<P> {
    type Value = (String, Vec<P>);

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "an entry of simulated_payloads naming a type, as `type \
             network:`, with its payloads under it"
        )
    }

    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> Result<Self::Value, A::Error> {
        let Some(key) = map.next_key::<String>()? else {
            return Err(de::Error::custom(
                "an entry of simulated_payloads is empty: it names a type, as \
                 `- type network:`, with its payloads under it",
            ));
        };
        let Some(named) = key.strip_prefix("type ") else {
            return Err(de::Error::custom(format!(
                "\"{key}\" is not a type: each entry of simulated_payloads is \
                 the word type, a space and the kind of the payloads under \
                 it, as `- type network:`"
            )));
        };
        kind_of(named).map_err(de::Error::custom)?;
        let named = named.to_string();
        let payloads =
            map.next_value_seed(YamlPayloads(named.clone(), PhantomData))?;
        if let Some(more) = map.next_key::<String>()? {
            return Err(de::Error::custom(format!(
                "the entry for type {named} says {more} as well: an entry of \
                 simulated_payloads names one type, and its payloads are the \
                 list under it"
            )));
        }
        if payloads.is_empty() {
            return Err(de::Error::custom(format!(
                "type {named} in simulated_payloads has no payloads under it: \
                 write its payloads, or leave the type out"
            )));
        }
        Ok((named, payloads))
    }
}

/// The list of payloads under a YAML type.
struct YamlPayloads<P>(String, PhantomData<P>);

impl<'de, P: SimEntry> DeserializeSeed<'de> for YamlPayloads<P> {
    type Value = Vec<P>;

    fn deserialize<D: Deserializer<'de>>(
        self,
        d: D,
    ) -> Result<Vec<P>, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de, P: SimEntry> Visitor<'de> for YamlPayloads<P> {
    type Value = Vec<P>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "the list of {} payloads, each `- payload: <name>`",
            self.0
        )
    }

    /// No payloads, which the type refuses with a message of its own.
    fn visit_unit<E: de::Error>(self) -> Result<Vec<P>, E> {
        Ok(Vec::new())
    }

    fn visit_none<E: de::Error>(self) -> Result<Vec<P>, E> {
        Ok(Vec::new())
    }

    fn visit_seq<A: SeqAccess<'de>>(
        self,
        mut seq: A,
    ) -> Result<Vec<P>, A::Error> {
        let mut payloads = Vec::new();
        while let Some(payload) = seq.next_element_seed(PayloadSeed {
            kind: &self.0,
            language: Language::Yaml,
            entry: PhantomData,
        })? {
            payloads.push(payload);
        }
        Ok(payloads)
    }
}

/// One XML `<type name="...">` element and the payloads in it.
struct XmlType<P>(PhantomData<P>);

impl<'de, P: SimEntry> DeserializeSeed<'de> for XmlType<P> {
    type Value = (String, Vec<P>);

    fn deserialize<D: Deserializer<'de>>(
        self,
        d: D,
    ) -> Result<Self::Value, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de, P: SimEntry> Visitor<'de> for XmlType<P> {
    type Value = (String, Vec<P>);

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "a <type name=\"...\"> element holding <payload> elements"
        )
    }

    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> Result<Self::Value, A::Error> {
        let mut named: Option<String> = None;
        let mut payloads = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            if key == "@name" {
                if named.is_some() {
                    return Err(de::Error::custom(
                        "a <type> names its type twice",
                    ));
                }
                let said = map.next_value::<String>()?;
                kind_of(&said).map_err(de::Error::custom)?;
                named = Some(said);
            } else if key == "payload" {
                // quick-xml gives an element's attributes before what is in
                // it, so the type is known by the time a payload is reached.
                let Some(kind) = named.as_deref() else {
                    return Err(de::Error::custom(
                        "a <type> holding <payload> elements has no name: \
                         write it as <type name=\"...\">",
                    ));
                };
                payloads.push(map.next_value_seed(PayloadSeed {
                    kind,
                    language: Language::Xml,
                    entry: PhantomData,
                })?);
            } else {
                return Err(de::Error::custom(format!(
                    "a <type> in <simulated_payloads> holds <payload \
                     name=\"...\"> elements and has a name attribute, and \
                     nothing else: this one has {}",
                    shown(&key)
                )));
            }
        }
        let Some(named) = named else {
            return Err(de::Error::custom(
                "a <type> in <simulated_payloads> has no name: write it as \
                 <type name=\"...\">",
            ));
        };
        if payloads.is_empty() {
            return Err(de::Error::custom(format!(
                "type {named} in <simulated_payloads> has no payloads in it: \
                 write its payloads, or leave the type out"
            )));
        }
        Ok((named, payloads))
    }
}

/// One simulated payload, of a type its entry has said.
struct PayloadSeed<'a, P> {
    kind: &'a str,
    language: Language,
    entry: PhantomData<P>,
}

impl<'de, P: SimEntry> DeserializeSeed<'de> for PayloadSeed<'_, P> {
    type Value = P;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<P, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de, P: SimEntry> Visitor<'de> for PayloadSeed<'_, P> {
    type Value = P;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "a {} payload, written {}",
            self.kind,
            Entry::Payload.spelled(self.language)
        )
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<P, A::Error> {
        let mut payload = P::deserialize(MapAccessDeserializer::new(
            Named::new(map, Entry::Payload, self.language),
        ))?;
        if payload.name().trim().is_empty() {
            return Err(de::Error::custom(format!(
                "a {} payload has no name: write it as {}",
                self.kind,
                Entry::Payload.spelled(self.language)
            )));
        }
        payload.written_under(self.kind);
        Ok(payload)
    }
}

/// Reads the groups section's groups, in whichever language.
struct GroupsVisitor<G>(PhantomData<G>);

impl<'de, G: SimEntry> Visitor<'de> for GroupsVisitor<G> {
    type Value = Vec<G>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "the groups of settings, each named by group")
    }

    /// A section written with nothing in it.
    fn visit_unit<E: de::Error>(self) -> Result<Vec<G>, E> {
        Ok(Vec::new())
    }

    fn visit_none<E: de::Error>(self) -> Result<Vec<G>, E> {
        Ok(Vec::new())
    }

    /// An empty XML element, which arrives as no text.
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Vec<G>, E> {
        if v.trim().is_empty() {
            return Ok(Vec::new());
        }
        Err(E::custom(format!(
            "simulated_payload_groups holds groups, not \"{v}\""
        )))
    }

    /// YAML: a list of groups.
    fn visit_seq<A: SeqAccess<'de>>(
        self,
        mut seq: A,
    ) -> Result<Vec<G>, A::Error> {
        let mut groups = Vec::new();
        while let Some(group) =
            seq.next_element_seed(GroupSeed(Language::Yaml, PhantomData))?
        {
            groups.push(group);
        }
        Ok(groups)
    }

    /// XML: an element of `<group>` elements.
    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> Result<Vec<G>, A::Error> {
        let mut groups = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            if key != "group" {
                return Err(de::Error::custom(format!(
                    "<simulated_payload_groups> holds <group name=\"...\"> \
                     elements and nothing else, and this one holds {}",
                    shown(&key)
                )));
            }
            groups.push(
                map.next_value_seed(GroupSeed(Language::Xml, PhantomData))?,
            );
        }
        Ok(groups)
    }
}

/// One group of settings.
struct GroupSeed<G>(Language, PhantomData<G>);

impl<'de, G: SimEntry> DeserializeSeed<'de> for GroupSeed<G> {
    type Value = G;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<G, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de, G: SimEntry> Visitor<'de> for GroupSeed<G> {
    type Value = G;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a group, written {}", Entry::Group.spelled(self.0))
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<G, A::Error> {
        let group = G::deserialize(MapAccessDeserializer::new(Named::new(
            map,
            Entry::Group,
            self.0,
        )))?;
        if group.name().trim().is_empty() {
            return Err(de::Error::custom(format!(
                "a simulated payload group has no name: write it as {}",
                Entry::Group.spelled(self.0)
            )));
        }
        Ok(group)
    }
}

/// An entry's map, as its derived reader is to see it: the key that names
/// it given as `name`, and the keys it may not say refused.
///
/// What it may not say: `name`, the entry being named by its word; `type`,
/// a payload's type being the type it is written under and a group having
/// none; in XML any attribute but `name`; and in YAML `payload` in a group.
/// `group` in a payload is the group it takes its settings from.
struct Named<A> {
    map: A,
    entry: Entry,
    language: Language,
}

impl<A> Named<A> {
    /// Wraps an entry's map.
    ///
    /// # Arguments
    ///
    /// * `map`      - The map.
    /// * `entry`    - Which kind of entry it is.
    /// * `language` - The language it is written in.
    ///
    /// # Returns
    ///
    /// The map, as the entry's reader is to see it.
    fn new(map: A, entry: Entry, language: Language) -> Named<A> {
        Named {
            map,
            entry,
            language,
        }
    }

    /// The key that names an entry, as the file writes it.
    ///
    /// # Returns
    ///
    /// The entry's word in YAML, and `@name` in XML.
    fn naming(&self) -> &'static str {
        match self.language {
            Language::Yaml => self.entry.word(),
            Language::Xml => "@name",
        }
    }

    /// Says why a key may not be written in an entry, if it may not.
    ///
    /// # Arguments
    ///
    /// * `key` - The key, as the file writes it.
    ///
    /// # Returns
    ///
    /// The message, or `None` for a key that may be written.
    fn refusal(&self, key: &str) -> Option<String> {
        let what = self.entry.what();
        let spelled = self.entry.spelled(self.language);
        let said = match self.language {
            Language::Xml => shown(key),
            Language::Yaml => format!("`{key}:`"),
        };
        match key {
            "name" | "@name" => Some(format!(
                "a {what} is named as {spelled}, and not by {said}"
            )),
            "type" | "@type" => Some(match self.entry {
                Entry::Payload => format!(
                    "a simulated payload says no type of its own: its type is \
                     the type it is written under, and here it says {said}"
                ),
                Entry::Group => format!(
                    "a simulated payload group says no type: a payload's type \
                     is the type it is written under, and here it says {said}"
                ),
            }),
            "payload" if self.language == Language::Yaml => Some(format!(
                "a {what} is written {spelled}, and this one says `payload:`"
            )),
            attribute if attribute.starts_with('@') => Some(format!(
                "<{}> has a name attribute and no other, and this one has {}: \
                 write it as an element inside the entry",
                self.entry.word(),
                shown(attribute)
            )),
            _ => None,
        }
    }
}

impl<'de, A: MapAccess<'de>> MapAccess<'de> for Named<A> {
    type Error = A::Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, A::Error> {
        let Some(key) = self.map.next_key::<String>()? else {
            return Ok(None);
        };
        let key = if key == self.naming() {
            "name".to_string()
        } else if let Some(why) = self.refusal(&key) {
            return Err(de::Error::custom(why));
        } else {
            key
        };
        seed.deserialize(key.into_deserializer()).map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, A::Error> {
        self.map.next_value_seed(seed)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::sim_config::SimConfigFile;

    /// The files of `tests/refused-sim`: simulator files, some with a
    /// physics section in a shape it had before, or a wrong one.
    ///
    /// # Returns
    ///
    /// Each file's path, in name order.
    fn refused() -> Vec<PathBuf> {
        let dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/refused-sim");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .expect("tests/refused-sim")
            .map(|entry| entry.expect("an entry").path())
            .collect();
        files.sort();
        files
    }

    /// Reads a simulator file.
    ///
    /// # Arguments
    ///
    /// * `file` - The file.
    ///
    /// # Returns
    ///
    /// What is wrong with it, or `None` where it reads.
    fn read(file: &Path) -> Option<String> {
        SimConfigFile::load(file).err().map(|e| e.to_string())
    }

    /// Every file of `tests/refused-sim` is refused, at a line.
    #[test]
    fn every_file_of_the_old_or_a_wrong_shape_is_refused() {
        let files = refused();
        assert!(files.len() > 10, "{files:?}");
        for file in files {
            let why = read(&file)
                .unwrap_or_else(|| panic!("{} is read", file.display()));
            assert!(why.contains("line"), "{}: {why}", file.display());
        }
    }

    /// Every simulator file and physics model the manual and the design
    /// show in YAML is in the shape they are read in.
    #[test]
    fn the_documents_simulator_files_and_models_read() {
        let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs");
        let mut shown = 0;
        for name in ["manual.rst", "design.rst"] {
            let text =
                std::fs::read_to_string(docs.join(name)).expect("a document");
            let lines: Vec<&str> = text.lines().collect();
            for (at, line) in lines.iter().enumerate() {
                if line.trim() != ".. code-block:: yaml" {
                    continue;
                }
                let indent = line.len() - line.trim_start().len() + 3;
                let block: Vec<&str> = lines[at + 1..]
                    .iter()
                    .take_while(|l| {
                        l.trim().is_empty()
                            || l.len() - l.trim_start().len() >= indent
                    })
                    .map(|l| l.get(indent..).unwrap_or(""))
                    .collect();
                let block = block.join("\n");
                let keys = |key: &str| {
                    block.lines().any(|l| l.trim_end() == format!("{key}:"))
                };
                let said = if keys("world")
                    || keys("simulated_payloads")
                    || keys("simulated_payload_groups")
                {
                    SimConfigFile::parse(&block, crate::ConfigFormat::Yaml)
                        .map(|_| ())
                } else {
                    continue;
                };
                shown += 1;
                if let Err(e) = said {
                    panic!("{name}:{}: {e}\n{block}", at + 1);
                }
            }
        }
        assert!(shown >= 7, "{shown}");
    }

    /// A file of the old shape is told to run the converter, and the
    /// converter rewrites it into a file that reads, which it then leaves
    /// as it is.
    #[test]
    fn the_converter_rewrites_the_old_shape_into_one_that_reads() {
        let converter = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tools/convert-format.py");
        let dir = tempfile::tempdir().expect("a directory");
        for file in refused() {
            let name = file.file_name().expect("a name").to_string_lossy();
            if !name.starts_with("old-") {
                continue;
            }
            let why = read(&file).expect("refused");
            assert!(why.contains("convert-format.py"), "{name}: {why}");

            let copy = dir.path().join(name.as_ref());
            std::fs::copy(&file, &copy).expect("copied");
            let convert = || {
                let ran = std::process::Command::new("python3")
                    .arg(&converter)
                    .arg(&copy)
                    .status()
                    .expect("python3 runs the converter");
                assert!(ran.success(), "{name}");
                std::fs::read_to_string(&copy).expect("read")
            };
            let once = convert();
            assert_eq!(read(&copy), None, "{name}: {once}");
            assert_eq!(convert(), once, "{name}: converted twice");
        }
    }
}
