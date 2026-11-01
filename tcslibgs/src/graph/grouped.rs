//! How a file writes its nodes: under three sections, and within each by
//! type.
//!
//! A section -- `gateways`, `vehicle` or `payloads` -- holds its nodes in
//! groups, one for each type, and a node says its name and what it is told
//! but not its type, which is the group's:
//!
//! ```yaml
//! payloads:
//!   - type i2c:
//!       - payload: temp_1
//!         address: "0x48"
//! ```
//!
//! ```xml
//! <payloads>
//!     <type name="i2c">
//!         <payload name="temp_1">
//!             <address>0x48</address>
//!         </payload>
//!     </type>
//! </payloads>
//! ```
//!
//! A node of the gateways section is a `gateway`, of the vehicle section a
//! `compute` whatever its type, and of the payloads section a `payload`. The
//! type is said once for every node of it, and a type that is not one of its
//! section's is refused where it is written, so a node cannot be in the wrong
//! section at all.
//!
//! The order of the groups, of the nodes in a group and of the attributes of
//! a node means nothing: a configuration is the same configuration whichever
//! order it is written in. See [`crate::config_digest`], which digests it in
//! one order, and [`crate::graph::telemetry::ids`], which numbers its nodes
//! in that order.
//!
//! The editor's own section is grouped the same way: a node's position is a
//! `node` entry naming it, `- { node: temp_1, x: 34, y: 34 }` in YAML and
//! `<node name="temp_1" x="34" y="34"/>` in XML, and so is a concern set
//! aside.
//!
//! Read by hand, because the two languages give the one shape differently.
//! A YAML section is a list of entries, each a mapping of one key -- `type`,
//! a space, and the type -- whose value is the list of nodes; an XML section
//! is an element of `<type>` elements, each named by an attribute. Which
//! language it is shows in the shape: a list is YAML, and an element is a
//! map. Every refusal here is a serde error, so the parser puts the line
//! beside it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::Deserializer;

use super::{
    entries, said_so, section_of, types_in, Aside, Destinations, Listed, Node,
    NodeFile, Placed, Section, LISTED,
};
use crate::endpoint_config::Scalar;

/// One type's nodes, as a section groups them.
#[derive(Debug, Clone, PartialEq)]
pub struct Typed {
    /// The type, as the group names it.
    pub node_type: String,
    /// Its nodes, each with this type.
    pub nodes: Vec<NodeFile>,
}

/// Which language a section is written in, as its shape shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Language {
    Yaml,
    Xml,
}

/// Reads the gateways section.
///
/// # Arguments
///
/// * `d` - The deserializer, at the section's value.
///
/// # Returns
///
/// Its groups of nodes.
///
/// # Errors
///
/// Anything [`SectionVisitor`] refuses.
pub fn gateways<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Vec<Typed>, D::Error> {
    d.deserialize_any(SectionVisitor(Section::Gateway))
}

/// Reads the vehicle section.
///
/// # Arguments
///
/// * `d` - The deserializer, at the section's value.
///
/// # Returns
///
/// Its groups of nodes.
///
/// # Errors
///
/// Anything [`SectionVisitor`] refuses.
pub fn vehicle<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Vec<Typed>, D::Error> {
    d.deserialize_any(SectionVisitor(Section::Vehicle))
}

/// Reads the payloads section.
///
/// # Arguments
///
/// * `d` - The deserializer, at the section's value.
///
/// # Returns
///
/// Its groups of nodes.
///
/// # Errors
///
/// Anything [`SectionVisitor`] refuses.
pub fn payloads<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Vec<Typed>, D::Error> {
    d.deserialize_any(SectionVisitor(Section::Payloads))
}

/// Says what is wrong with a type a section names, if anything.
///
/// # Arguments
///
/// * `section` - The section it is written in.
/// * `named`   - The type, as the file wrote it.
///
/// # Returns
///
/// `Ok(())` for one of the section's types.
///
/// # Errors
///
/// A message for an empty type, a type with a space in it, a type of another
/// section, saying which, and a type there is none of, listing the section's.
pub fn type_of_section(section: Section, named: &str) -> Result<(), String> {
    if named.is_empty() {
        return Err(format!(
            "a type in {} names no type: write the type after the word",
            section.name()
        ));
    }
    if named.chars().any(char::is_whitespace) {
        return Err(format!(
            "\"{named}\" is not a type: a type is one word, with no space in \
             it"
        ));
    }
    if types_in(section).contains(&named) {
        return Ok(());
    }
    let belongs = section_of(named);
    if belongs != section && types_in(belongs).contains(&named) {
        return Err(format!(
            "{named} is a type of the {} section, and is written under {} \
             rather than {}",
            belongs.name(),
            belongs.name(),
            section.name()
        ));
    }
    // The null node is gone, and a file written before it went names it:
    // it is told what replaced it, it being the one type a file can name
    // that once was one.
    let was_null = if named == "null" {
        " The null node, which took both streams, is gone: write null-gateway \
         or null-payload for a node that drops the one stream it takes, or \
         tee-gateway or tee-payload for one that sends it on both ways."
    } else {
        ""
    };
    Err(format!(
        "{named} is not a type of node the {} section holds, which are {}.\
         {was_null}",
        section.name(),
        types_in(section).join(", ")
    ))
}

/// Reads the type a YAML entry's key names: `type`, one space, and the type.
///
/// # Arguments
///
/// * `section` - The section the entry is in.
/// * `key`     - The key.
///
/// # Returns
///
/// The type.
///
/// # Errors
///
/// A message for a key that is not the word `type`, one space and a type of
/// the section's.
fn type_in_key(section: Section, key: &str) -> Result<String, String> {
    let Some(named) = key.strip_prefix("type ") else {
        return Err(format!(
            "\"{key}\" is not a type: each entry of {} is the word type, a \
             space and the type its nodes are, as `- type {}:`",
            section.name(),
            types_in(section).first().copied().unwrap_or("i2c")
        ));
    };
    if named.starts_with(char::is_whitespace) {
        return Err(format!(
            "\"{key}\" has more than one space after type: write one, as \
             `type {}`",
            named.trim()
        ));
    }
    type_of_section(section, named)?;
    Ok(named.to_string())
}

/// Reads one section's groups, in whichever language it is written.
struct SectionVisitor(Section);

impl SectionVisitor {
    /// Adds a group to those read, refusing a type grouped twice.
    ///
    /// # Arguments
    ///
    /// * `groups` - The groups read so far.
    /// * `typed`  - The next.
    ///
    /// # Errors
    ///
    /// A message for a type already grouped in this section.
    fn add<E: de::Error>(
        &self,
        groups: &mut Vec<Typed>,
        typed: Typed,
    ) -> Result<(), E> {
        if groups.iter().any(|g| g.node_type == typed.node_type) {
            return Err(E::custom(format!(
                "type {} is written twice in {}: put its nodes under one",
                typed.node_type,
                self.0.name()
            )));
        }
        groups.push(typed);
        Ok(())
    }
}

impl<'de> Visitor<'de> for SectionVisitor {
    type Value = Vec<Typed>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "the {} section's types and their nodes", self.0.name())
    }

    /// A section written with nothing in it.
    fn visit_unit<E: de::Error>(self) -> Result<Vec<Typed>, E> {
        Ok(Vec::new())
    }

    fn visit_none<E: de::Error>(self) -> Result<Vec<Typed>, E> {
        Ok(Vec::new())
    }

    /// An empty XML element, which arrives as no text.
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Vec<Typed>, E> {
        if v.trim().is_empty() {
            return Ok(Vec::new());
        }
        Err(E::custom(format!(
            "{} holds types and their nodes, not \"{v}\"",
            self.0.name()
        )))
    }

    /// YAML: a list of entries, each a type and its nodes.
    fn visit_seq<A: SeqAccess<'de>>(
        self,
        mut seq: A,
    ) -> Result<Vec<Typed>, A::Error> {
        let mut groups = Vec::new();
        while let Some(typed) = seq.next_element_seed(YamlType(self.0))? {
            self.add(&mut groups, typed)?;
        }
        Ok(groups)
    }

    /// XML: an element of `<type>` elements.
    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> Result<Vec<Typed>, A::Error> {
        let mut groups = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            if key != "type" {
                return Err(de::Error::custom(format!(
                    "<{}> holds <type name=\"...\"> elements and nothing \
                     else, and this one holds {}",
                    self.0.name(),
                    shown(&key)
                )));
            }
            let typed = map.next_value_seed(XmlType(self.0))?;
            self.add(&mut groups, typed)?;
        }
        Ok(groups)
    }
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

/// One YAML entry of a section: `type X:` and its list of nodes.
struct YamlType(Section);

impl<'de> DeserializeSeed<'de> for YamlType {
    type Value = Typed;

    fn deserialize<D: Deserializer<'de>>(
        self,
        d: D,
    ) -> Result<Typed, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for YamlType {
    type Value = Typed;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "an entry of {} naming a type, as `type i2c:`, with its nodes \
             under it",
            self.0.name()
        )
    }

    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> Result<Typed, A::Error> {
        let Some(key) = map.next_key::<String>()? else {
            return Err(de::Error::custom(format!(
                "an entry of {} is empty: it names a type, as `- type \
                 {}:`, with its nodes under it",
                self.0.name(),
                types_in(self.0).first().copied().unwrap_or("i2c")
            )));
        };
        let node_type = type_in_key(self.0, &key).map_err(de::Error::custom)?;
        let nodes = map.next_value_seed(YamlNodes {
            section: self.0,
            node_type: node_type.clone(),
        })?;
        if let Some(more) = map.next_key::<String>()? {
            return Err(de::Error::custom(format!(
                "the entry for type {node_type} says {more} as well: an \
                 entry of {} names one type, and its nodes are the list \
                 under it",
                self.0.name()
            )));
        }
        if nodes.is_empty() {
            return Err(de::Error::custom(format!(
                "type {node_type} in {} has no nodes under it: write its \
                 nodes, or leave the type out",
                self.0.name()
            )));
        }
        Ok(Typed { node_type, nodes })
    }
}

/// The list of nodes under a YAML type.
struct YamlNodes {
    section: Section,
    node_type: String,
}

impl<'de> DeserializeSeed<'de> for YamlNodes {
    type Value = Vec<NodeFile>;

    fn deserialize<D: Deserializer<'de>>(
        self,
        d: D,
    ) -> Result<Vec<NodeFile>, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for YamlNodes {
    type Value = Vec<NodeFile>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "the list of {} nodes, each `- {}: <name>`",
            self.node_type,
            self.section.node_word()
        )
    }

    /// No nodes, which the type refuses with a message of its own.
    fn visit_unit<E: de::Error>(self) -> Result<Vec<NodeFile>, E> {
        Ok(Vec::new())
    }

    fn visit_none<E: de::Error>(self) -> Result<Vec<NodeFile>, E> {
        Ok(Vec::new())
    }

    fn visit_seq<A: SeqAccess<'de>>(
        self,
        mut seq: A,
    ) -> Result<Vec<NodeFile>, A::Error> {
        let mut nodes = Vec::new();
        while let Some(node) = seq.next_element_seed(NodeSeed {
            section: self.section,
            node_type: self.node_type.clone(),
            language: Language::Yaml,
        })? {
            nodes.push(node);
        }
        Ok(nodes)
    }
}

/// One XML `<type name="...">` element and the nodes in it.
struct XmlType(Section);

impl<'de> DeserializeSeed<'de> for XmlType {
    type Value = Typed;

    fn deserialize<D: Deserializer<'de>>(
        self,
        d: D,
    ) -> Result<Typed, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for XmlType {
    type Value = Typed;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "a <type name=\"...\"> element holding <{}> elements",
            self.0.node_word()
        )
    }

    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> Result<Typed, A::Error> {
        let word = self.0.node_word();
        let mut node_type: Option<String> = None;
        let mut nodes = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            if key == "@name" {
                if node_type.is_some() {
                    return Err(de::Error::custom(
                        "a <type> names its type twice",
                    ));
                }
                let named = map.next_value::<Scalar>()?.as_str().to_string();
                type_of_section(self.0, &named).map_err(de::Error::custom)?;
                node_type = Some(named);
            } else if key == word {
                // quick-xml gives an element's attributes before what is in
                // it, so the name is known by the time a node is reached.
                let Some(named) = node_type.clone() else {
                    return Err(de::Error::custom(format!(
                        "a <type> holding <{word}> elements has no name: \
                         write it as <type name=\"...\">"
                    )));
                };
                nodes.push(map.next_value_seed(NodeSeed {
                    section: self.0,
                    node_type: named,
                    language: Language::Xml,
                })?);
            } else {
                return Err(de::Error::custom(format!(
                    "a <type> in <{}> holds <{word} name=\"...\"> elements \
                     and has a name attribute, and nothing else: this one \
                     has {}",
                    self.0.name(),
                    shown(&key)
                )));
            }
        }
        let Some(node_type) = node_type else {
            return Err(de::Error::custom(format!(
                "a <type> in <{}> has no name: write it as <type \
                 name=\"...\">",
                self.0.name()
            )));
        };
        if nodes.is_empty() {
            return Err(de::Error::custom(format!(
                "type {node_type} in <{}> has no nodes in it: write its \
                 nodes, or leave the type out",
                self.0.name()
            )));
        }
        Ok(Typed { node_type, nodes })
    }
}

/// One node, of a type its group has said.
struct NodeSeed {
    section: Section,
    node_type: String,
    language: Language,
}

impl<'de> DeserializeSeed<'de> for NodeSeed {
    type Value = NodeFile;

    fn deserialize<D: Deserializer<'de>>(
        self,
        d: D,
    ) -> Result<NodeFile, D::Error> {
        d.deserialize_map(self)
    }
}

impl NodeSeed {
    /// Says how a node of this section is written, for a message.
    ///
    /// # Returns
    ///
    /// `- payload: <name>` or `<payload name="...">`.
    fn spelled(&self) -> String {
        let word = self.section.node_word();
        match self.language {
            Language::Yaml => format!("`- {word}: <name>`"),
            Language::Xml => format!("<{word} name=\"...\">"),
        }
    }

    /// Whether a key may be written more than once in a node.
    ///
    /// In XML a link with several destinations and a listed attribute are
    /// written as one element for each, so those repeat; in YAML they are
    /// lists, and no key repeats.
    ///
    /// # Arguments
    ///
    /// * `key` - The key.
    ///
    /// # Returns
    ///
    /// Whether it may repeat.
    fn repeats(&self, key: &str) -> bool {
        self.language == Language::Xml
            && (key == "payload-out"
                || key == "gateway-out"
                || LISTED.contains(&key))
    }
}

impl<'de> Visitor<'de> for NodeSeed {
    type Value = NodeFile;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a {} node, written {}", self.node_type, self.spelled())
    }

    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> Result<NodeFile, A::Error> {
        let word = self.section.node_word();
        let name_key = match self.language {
            Language::Yaml => word,
            Language::Xml => "@name",
        };
        let mut name: Option<String> = None;
        let mut group: Option<String> = None;
        let mut seen: BTreeSet<String> = BTreeSet::new();

        let mut attributes: BTreeMap<String, Scalar> = BTreeMap::new();
        let mut payload_out: Vec<String> = Vec::new();
        let mut gateway_out: Vec<String> = Vec::new();
        let mut payload_in: Option<bool> = None;
        let mut gateway_in: Option<bool> = None;
        let mut lists: BTreeMap<String, Vec<String>> = BTreeMap::new();

        let who = |name: &Option<String>| match name {
            Some(name) => format!("\"{name}\""),
            None => format!("a {} node", self.node_type),
        };

        while let Some(key) = map.next_key::<String>()? {
            if !seen.insert(key.clone()) && !self.repeats(&key) {
                let said = match self.language {
                    Language::Xml => shown(&key),
                    Language::Yaml => key.clone(),
                };
                return Err(de::Error::custom(format!(
                    "{} states {said} twice",
                    who(&name)
                )));
            }
            match key.as_str() {
                named if named == name_key => {
                    let said = map.next_value::<Scalar>()?.as_str().to_string();
                    if said.trim().is_empty() {
                        return Err(de::Error::custom(format!(
                            "a {} node has an empty name: every node is named, \
                             {}",
                            self.node_type,
                            self.spelled()
                        )));
                    }
                    name = Some(said);
                }
                // The words that name a node in the other sections: a node
                // written under the wrong word is told the right one.
                "gateway" | "compute" | "payload"
                    if self.language == Language::Yaml =>
                {
                    return Err(de::Error::custom(format!(
                        "a node of the {} section is written {}, and this \
                         one is written `{key}:`",
                        self.section.name(),
                        self.spelled()
                    )));
                }
                "name" | "@name" => {
                    let said = match self.language {
                        Language::Xml => shown(&key),
                        Language::Yaml => format!("`{key}:`"),
                    };
                    return Err(de::Error::custom(format!(
                        "a node is named as {}, and not by {said}",
                        self.spelled(),
                    )));
                }
                "type" | "@type" => {
                    return Err(de::Error::custom(format!(
                        "{} says a type of its own: a node's type is the type \
                         it is written under, and it says none itself",
                        who(&name)
                    )));
                }
                attribute if attribute.starts_with('@') => {
                    return Err(de::Error::custom(format!(
                        "<{word}> has a name attribute and no other, and \
                         {} has {}: write it as an element inside the node",
                        who(&name),
                        shown(attribute)
                    )));
                }
                "group" => {
                    group =
                        Some(map.next_value::<Scalar>()?.as_str().to_string())
                }
                // Where this used to be, and said so rather than read as an
                // attribute nothing knows: a position is the editor's own
                // and lives in the `tcsgui` section at the end of the file.
                "editor" => {
                    return Err(de::Error::custom(
                        "a node's position is not written on the node any \
                         more: it is in the tcsgui section at the end of the \
                         file, as `- { node: <node>, x: <x>, y: <y> }`",
                    ))
                }
                // A data source, and the one place a file says what is
                // linked to what. Repeats add to it rather than replacing
                // it: that is what several XML elements of one name mean.
                "payload-out" => payload_out.extend(
                    map.next_value::<Destinations>()?
                        .each()
                        .into_iter()
                        .map(str::to_string),
                ),
                "gateway-out" => gateway_out.extend(
                    map.next_value::<Destinations>()?
                        .each()
                        .into_iter()
                        .map(str::to_string),
                ),
                // A data sink, which says whether a line arrives there -- a
                // record of links written on the nodes they leave, and not a
                // link. Checked against those links as the configuration is
                // read: see `mirror`.
                "payload-in" => {
                    payload_in = Some(said_so("payload-in", &mut map)?)
                }
                "gateway-in" => {
                    gateway_in = Some(said_so("gateway-in", &mut map)?)
                }
                // A list of alternatives, each a run of bytes: a YAML list,
                // or a repeated XML element, whose repeats add to it as
                // payload-out's do. See `Listed`.
                listed if LISTED.contains(&listed) => {
                    let attribute = listed.to_string();
                    let listed = map.next_value::<Listed>()?;
                    for one in &listed.0 {
                        if entries(one.as_str()).len() > 1 {
                            return Err(de::Error::custom(format!(
                                "{} writes the {attribute} {:?} as one value \
                                 with commas between them: write each as an \
                                 entry of its own -- a YAML list, or one \
                                 <{attribute}> element for each",
                                who(&name),
                                one.as_str()
                            )));
                        }
                    }
                    lists.entry(attribute).or_default().extend(
                        listed
                            .0
                            .iter()
                            .map(|one| one.as_str().trim().to_string()),
                    );
                }
                // Everything else is an attribute of the node, which is
                // where a word nothing reads ends up -- and is refused by
                // name against what the type takes. See `reach::reach_of`.
                _ => {
                    let value = map.next_value::<Scalar>()?;
                    attributes.insert(key.clone(), value);
                }
            }
        }

        // Held as one value, the alternatives joined by commas, which is
        // what a node's attribute is and what the editor shows: the commas
        // are between parts, where none can be otherwise, so
        // `crate::trigger::sequences_of` splits them back apart exactly. The
        // file has the one shape; this is the model's.
        for (attribute, each) in lists {
            attributes.insert(attribute, Scalar::of(each.join(", ")));
        }

        let Some(name) = name else {
            return Err(de::Error::custom(format!(
                "a {} node has no name: write it as {}",
                self.node_type,
                self.spelled()
            )));
        };
        Ok(NodeFile {
            node: Node {
                name,
                node_type: self.node_type,
                group,
                attributes,
            },
            payload_out,
            gateway_out,
            payload_in,
            gateway_in,
        })
    }
}

/// Reads where the editor drew each node: the `nodes` of the `tcsgui`
/// section.
///
/// # Arguments
///
/// * `d` - The deserializer, at the list's value.
///
/// # Returns
///
/// Each node's place.
///
/// # Errors
///
/// An entry that does not name a node and give two whole numbers, and a node
/// placed twice.
pub fn layout<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Vec<Placed>, D::Error> {
    let entries = d.deserialize_any(Entries {
        what: "nodes",
        names: &["x", "y"],
    })?;
    let mut placed = Vec::new();
    for (name, mut said) in entries {
        let mut at = |axis: &str| -> Result<i32, D::Error> {
            let text = said.remove(axis).unwrap_or_default();
            text.trim().parse().map_err(|_| {
                de::Error::custom(format!(
                    "\"{name}\" is placed at {axis} \"{text}\", which is not \
                     a whole number"
                ))
            })
        };
        let x = at("x")?;
        let y = at("y")?;
        placed.push(Placed { name, x, y });
    }
    Ok(placed)
}

/// Reads the concerns the editor was told to set aside: the `set-aside` of
/// the `tcsgui` section.
///
/// # Arguments
///
/// * `d` - The deserializer, at the list's value.
///
/// # Returns
///
/// Each node and the concern set aside about it.
///
/// # Errors
///
/// An entry that does not name a node and a concern.
pub fn set_aside<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Vec<Aside>, D::Error> {
    let entries = d.deserialize_any(Entries {
        what: "set-aside",
        names: &["concern"],
    })?;
    Ok(entries
        .into_iter()
        .map(|(node, mut said)| Aside {
            node,
            concern: said.remove("concern").unwrap_or_default(),
        })
        .collect())
}

/// The entries of a list in the editor's section, each naming a node and
/// saying some words about it.
///
/// In YAML a list of mappings, `- { node: temp_1, x: 34, y: 34 }`; in XML an
/// element of `<node>` elements, the same words as attributes,
/// `<node name="temp_1" x="34" y="34"/>`.
struct Entries {
    /// The list's name, for the messages.
    what: &'static str,
    /// The words each entry says beside the node it names, all required.
    names: &'static [&'static str],
}

impl<'de> Visitor<'de> for Entries {
    type Value = Vec<(String, BTreeMap<String, String>)>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "the entries of {}, each naming a node", self.what)
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(Vec::new())
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(Vec::new())
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        if v.trim().is_empty() {
            return Ok(Vec::new());
        }
        Err(E::custom(format!(
            "{} holds entries naming nodes, not \"{v}\"",
            self.what
        )))
    }

    /// YAML: a list of mappings.
    fn visit_seq<A: SeqAccess<'de>>(
        self,
        mut seq: A,
    ) -> Result<Self::Value, A::Error> {
        let mut all = Vec::new();
        while let Some(one) = seq.next_element_seed(Entry {
            what: self.what,
            names: self.names,
            language: Language::Yaml,
        })? {
            once(self.what, &all, &one)?;
            all.push(one);
        }
        Ok(all)
    }

    /// XML: an element of `<node>` elements.
    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> Result<Self::Value, A::Error> {
        let mut all = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            if key != "node" {
                return Err(de::Error::custom(format!(
                    "<{}> holds <node name=\"...\"> elements and nothing \
                     else, and this one has {}",
                    self.what,
                    shown(&key)
                )));
            }
            let one = map.next_value_seed(Entry {
                what: self.what,
                names: self.names,
                language: Language::Xml,
            })?;
            once(self.what, &all, &one)?;
            all.push(one);
        }
        Ok(all)
    }
}

/// Refuses an entry of the editor's section that says again what one before
/// it said.
///
/// # Arguments
///
/// * `what` - The list's name, for the message.
/// * `all`  - The entries read so far.
/// * `one`  - The next.
///
/// # Errors
///
/// A message for a node placed twice, or a concern set aside twice.
fn once<E: de::Error>(
    what: &str,
    all: &[(String, BTreeMap<String, String>)],
    one: &(String, BTreeMap<String, String>),
) -> Result<(), E> {
    let again = all.iter().any(|(name, said)| {
        name == &one.0 && (what == "nodes" || said == &one.1)
    });
    if again {
        return Err(E::custom(format!(
            "{what} names \"{}\" twice with nothing to choose between them",
            one.0
        )));
    }
    Ok(())
}

/// One entry of a list in the editor's section.
struct Entry {
    what: &'static str,
    names: &'static [&'static str],
    language: Language,
}

impl<'de> DeserializeSeed<'de> for Entry {
    type Value = (String, BTreeMap<String, String>);

    fn deserialize<D: Deserializer<'de>>(
        self,
        d: D,
    ) -> Result<Self::Value, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for Entry {
    type Value = (String, BTreeMap<String, String>);

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "an entry of {} naming a node", self.what)
    }

    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> Result<Self::Value, A::Error> {
        let name_key = match self.language {
            Language::Yaml => "node",
            Language::Xml => "@name",
        };
        let spelled = || match self.language {
            Language::Yaml => format!(
                "`- {{ node: <name>, {} }}`",
                self.names
                    .iter()
                    .map(|n| format!("{n}: ..."))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Language::Xml => format!(
                "<node name=\"...\" {}/>",
                self.names
                    .iter()
                    .map(|n| format!("{n}=\"...\""))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        };
        let mut name: Option<String> = None;
        let mut said: BTreeMap<String, String> = BTreeMap::new();
        while let Some(key) = map.next_key::<String>()? {
            let value = map.next_value::<Scalar>()?.as_str().to_string();
            let word = match self.language {
                Language::Yaml => Some(key.as_str()),
                Language::Xml => key.strip_prefix('@'),
            };
            let fresh = if key == name_key {
                name.replace(value).is_none()
            } else if let Some(word) = word.filter(|w| self.names.contains(w)) {
                said.insert(word.to_string(), value).is_none()
            } else {
                return Err(de::Error::custom(format!(
                    "an entry of {} is written {}, and this one says {}",
                    self.what,
                    spelled(),
                    match self.language {
                        Language::Xml => shown(&key),
                        Language::Yaml => key.clone(),
                    }
                )));
            };
            if !fresh {
                return Err(de::Error::custom(format!(
                    "an entry of {} says {key} twice",
                    self.what
                )));
            }
        }
        let name = name.filter(|n| !n.trim().is_empty()).ok_or_else(|| {
            de::Error::custom(format!(
                "an entry of {} names no node: write it as {}",
                self.what,
                spelled()
            ))
        })?;
        if let Some(missing) =
            self.names.iter().find(|n| !said.contains_key(**n))
        {
            return Err(de::Error::custom(format!(
                "the entry of {} for \"{name}\" says no {missing}: write it \
                 as {}",
                self.what,
                spelled()
            )));
        }
        Ok((name, said))
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Graph, GraphFile};
    use crate::format::ConfigFormat;

    /// Reads a file's text, saying what is refused in it.
    ///
    /// # Arguments
    ///
    /// * `format` - Its language.
    /// * `text`   - The text.
    ///
    /// # Returns
    ///
    /// The file as read, or the refusal as text.
    fn read(format: ConfigFormat, text: &str) -> Result<GraphFile, String> {
        format.parse::<GraphFile>(text).map_err(|e| e.to_string())
    }

    /// Says that a text is refused, and that the refusal says each of some
    /// words.
    ///
    /// # Arguments
    ///
    /// * `format` - Its language.
    /// * `text`   - The text.
    /// * `words`  - What the refusal must say.
    ///
    /// # Panics
    ///
    /// If the text is read, or the refusal does not say every word.
    fn refused(format: ConfigFormat, text: &str, words: &[&str]) {
        let why = match read(format, text) {
            Ok(file) => panic!("{text}\nwas read: {file:?}"),
            Err(why) => why,
        };
        for word in words {
            assert!(why.contains(word), "{word:?} not in: {why}\n{text}");
        }
    }

    /// The XML of a configuration whose payloads section holds this.
    ///
    /// # Arguments
    ///
    /// * `inside` - What `<payloads>` holds.
    ///
    /// # Returns
    ///
    /// The text.
    fn xml(inside: &str) -> String {
        format!("<configuration><payloads>{inside}</payloads></configuration>")
    }

    /// A file in the new shape is read in both languages, the same.
    #[test]
    fn the_new_shape_is_read_alike_in_both_languages() {
        let yaml = read(
            ConfigFormat::Yaml,
            "gateways:\n  - type gateway_radio:\n      - gateway: g\n        \
             address: \"127.0.0.1\"\n\
             payloads:\n  - type i2c:\n      - payload: t1\n        \
             address: \"0x48\"\n      - payload: t2\n  \
             - type network:\n      - { payload: n, port: \"1\" }\n\
             tcsgui:\n  nodes:\n    - { node: t1, x: 1, y: 2 }\n  \
             set-aside:\n    - { node: t1, concern: c }\n",
        )
        .expect("YAML");
        let xml = read(
            ConfigFormat::Xml,
            "<configuration><gateways><type name=\"gateway_radio\">\
             <gateway name=\"g\"><address>127.0.0.1</address></gateway>\
             </type></gateways><payloads><type name=\"i2c\">\
             <payload name=\"t1\"><address>0x48</address></payload>\
             <payload name=\"t2\"/></type><type name=\"network\">\
             <payload name=\"n\"><port>1</port></payload></type></payloads>\
             <tcsgui><nodes><node name=\"t1\" x=\"1\" y=\"2\"/></nodes>\
             <set-aside><node name=\"t1\" concern=\"c\"/></set-aside>\
             </tcsgui></configuration>",
        )
        .expect("XML");
        assert_eq!(yaml.gateways, xml.gateways);
        assert_eq!(yaml.payloads, xml.payloads);
        assert_eq!(yaml.tcsgui, xml.tcsgui);
        assert_eq!(yaml.payloads.len(), 2);
        assert_eq!(yaml.payloads[0].node_type, "i2c");
        assert_eq!(yaml.payloads[0].nodes[1].node.name, "t2");
        assert_eq!(yaml.payloads[0].nodes[1].node.node_type, "i2c");
    }

    /// A type that is not one of its section's is refused, told where it
    /// belongs, in either language.
    #[test]
    fn a_type_of_another_section_is_refused() {
        refused(
            ConfigFormat::Yaml,
            "vehicle:\n  - type i2c:\n      - compute: a\n",
            &["i2c is a type of the payloads section"],
        );
        refused(
            ConfigFormat::Xml,
            "<configuration><vehicle><type name=\"i2c\">\
             <compute name=\"a\"/></type></vehicle></configuration>",
            &["i2c is a type of the payloads section"],
        );
        refused(
            ConfigFormat::Yaml,
            "payloads:\n  - type bogus:\n      - payload: a\n",
            &["bogus is not a type of node the payloads section holds"],
        );
        refused(
            ConfigFormat::Xml,
            &xml("<type name=\"bogus\"><payload name=\"a\"/></type>"),
            &["bogus is not a type of node the payloads section holds"],
        );
    }

    /// The key that names a type is the word, one space, and the type.
    #[test]
    fn a_type_key_is_type_one_space_and_a_name() {
        for key in ["type  i2c", "typei2c", "Type i2c", "type i2c x", "kind"] {
            refused(
                ConfigFormat::Yaml,
                &format!("payloads:\n  - \"{key}\":\n      - payload: a\n"),
                &[],
            );
        }
        refused(
            ConfigFormat::Yaml,
            "payloads:\n  - \"type \":\n      - payload: a\n",
            &["names no type"],
        );
    }

    /// A node says no type of its own, in either language.
    #[test]
    fn a_node_with_a_type_of_its_own_is_refused() {
        refused(
            ConfigFormat::Yaml,
            "payloads:\n  - type i2c:\n      - payload: a\n        type: i2c\n",
            &["says a type of its own"],
        );
        refused(
            ConfigFormat::Xml,
            &xml(
                "<type name=\"i2c\"><payload name=\"a\" type=\"i2c\"/></type>",
            ),
            &["says a type of its own"],
        );
        refused(
            ConfigFormat::Xml,
            &xml("<type name=\"i2c\"><payload name=\"a\"><type>i2c</type>\
                 </payload></type>"),
            &["says a type of its own"],
        );
    }

    /// Every node is named, and not with an empty name.
    #[test]
    fn a_node_without_a_name_is_refused() {
        refused(
            ConfigFormat::Yaml,
            "payloads:\n  - type i2c:\n      - address: \"0x48\"\n",
            &["has no name", "- payload: <name>"],
        );
        refused(
            ConfigFormat::Xml,
            &xml("<type name=\"i2c\"><payload><address>1</address></payload>\
                  </type>"),
            &["has no name"],
        );
        refused(
            ConfigFormat::Yaml,
            "payloads:\n  - type i2c:\n      - payload: \"\"\n",
            &["empty name"],
        );
        refused(
            ConfigFormat::Xml,
            &xml("<type name=\"i2c\"><payload name=\" \"/></type>"),
            &["empty name"],
        );
        // And named by the word for its section and nothing else.
        refused(
            ConfigFormat::Yaml,
            "payloads:\n  - type i2c:\n      - name: a\n",
            &["not by `name:`"],
        );
        refused(
            ConfigFormat::Yaml,
            "payloads:\n  - type i2c:\n      - compute: a\n",
            &["written `- payload: <name>`"],
        );
        refused(
            ConfigFormat::Xml,
            &xml("<type name=\"i2c\"><payload name=\"a\"><name>a</name>\
                  </payload></type>"),
            &["not by <name>"],
        );
        refused(
            ConfigFormat::Xml,
            &xml("<type name=\"i2c\"><compute name=\"a\"/></type>"),
            &["<compute>"],
        );
    }

    /// A word a node says twice is refused, in either language; a link and
    /// a listed attribute repeat in XML, which is how XML writes a list.
    #[test]
    fn a_word_said_twice_is_refused() {
        refused(
            ConfigFormat::Yaml,
            "payloads:\n  - type i2c:\n      - payload: a\n        \
             address: \"1\"\n        address: \"2\"\n",
            &["address"],
        );
        refused(
            ConfigFormat::Yaml,
            "payloads:\n  - type i2c:\n      - payload: a\n        \
             payload: b\n",
            &["payload"],
        );
        refused(
            ConfigFormat::Xml,
            &xml(
                "<type name=\"i2c\"><payload name=\"a\"><address>1</address>\
                  <address>2</address></payload></type>",
            ),
            &["states <address> twice"],
        );
        refused(
            ConfigFormat::Xml,
            &xml("<type name=\"i2c\"><payload name=\"a\"><group>g</group>\
                  <group>h</group></payload></type>"),
            &["states <group> twice"],
        );
        let file = read(
            ConfigFormat::Xml,
            &xml("<type name=\"i2c\"><payload name=\"a\">\
                 <payload-out><target>b</target></payload-out>\
                 <payload-out><target>c</target></payload-out>\
                 </payload></type>"),
        )
        .expect("a link to two places");
        assert_eq!(file.payloads[0].nodes[0].payload_out, ["b", "c"]);
    }

    /// A type grouped twice in one section is refused: its nodes go under
    /// one.
    #[test]
    fn a_type_grouped_twice_is_refused() {
        refused(
            ConfigFormat::Yaml,
            "payloads:\n  - type i2c:\n      - payload: a\n  \
             - type i2c:\n      - payload: b\n",
            &["type i2c is written twice in payloads"],
        );
        refused(
            ConfigFormat::Xml,
            &xml("<type name=\"i2c\"><payload name=\"a\"/></type>\
                  <type name=\"i2c\"><payload name=\"b\"/></type>"),
            &["type i2c is written twice in payloads"],
        );
        // And a type with nothing in it says nothing.
        refused(
            ConfigFormat::Yaml,
            "payloads:\n  - type i2c:\n",
            &["has no nodes"],
        );
        refused(
            ConfigFormat::Xml,
            &xml("<type name=\"i2c\"/>"),
            &["has no nodes"],
        );
    }

    /// An attribute an XML node element does not have is refused.
    #[test]
    fn an_xml_node_has_a_name_attribute_and_no_other() {
        refused(
            ConfigFormat::Xml,
            &xml("<type name=\"i2c\"><payload name=\"a\" address=\"1\"/>\
                  </type>"),
            &["the attribute address"],
        );
        refused(
            ConfigFormat::Xml,
            &xml("<type name=\"i2c\" x=\"1\"><payload name=\"a\"/></type>"),
            &["the attribute x"],
        );
    }

    /// The editor's entries name their nodes as the sections do.
    #[test]
    fn a_position_names_its_node() {
        refused(
            ConfigFormat::Yaml,
            "tcsgui:\n  nodes:\n    - { x: 1, y: 2 }\n",
            &["names no node"],
        );
        refused(
            ConfigFormat::Yaml,
            "tcsgui:\n  nodes:\n    - { node: a, x: 1 }\n",
            &["says no y"],
        );
        refused(
            ConfigFormat::Yaml,
            "tcsgui:\n  nodes:\n    - { node: a, x: 1, y: up }\n",
            &["not a whole number"],
        );
        refused(
            ConfigFormat::Yaml,
            "tcsgui:\n  nodes:\n    - { node: a, x: 1, y: 2 }\n    \
             - { node: a, x: 3, y: 4 }\n",
            &["twice"],
        );
        refused(
            ConfigFormat::Xml,
            "<configuration><tcsgui><nodes><node x=\"1\" y=\"2\"/></nodes>\
             </tcsgui></configuration>",
            &["names no node"],
        );
    }

    /// A node's name is one across the whole configuration: two nodes of
    /// one name are refused, in one file or across an include.
    #[test]
    fn a_name_is_one_across_the_configuration() {
        let dir = tempfile::tempdir().expect("a directory");
        let top = dir.path().join("top.ytcs");
        std::fs::write(
            &top,
            "payloads:\n  - type i2c:\n      - payload: a\n  \
             - type spi:\n      - payload: a\n",
        )
        .expect("written");
        let why = Graph::load(&top).expect_err("twice").to_string();
        assert!(why.contains("\"a\" twice in this file"), "{why}");

        std::fs::write(
            &top,
            "include:\n  - other\npayloads:\n  - type i2c:\n      \
             - payload: a\n",
        )
        .expect("written");
        std::fs::write(
            dir.path().join("other.ytcs"),
            "vehicle:\n  - type ping:\n      - compute: a\n",
        )
        .expect("written");
        let why = Graph::load(&top).expect_err("across").to_string();
        assert!(why.contains("other.ytcs"), "{why}");
        assert!(why.contains("\"a\" here and in"), "{why}");
    }
}
