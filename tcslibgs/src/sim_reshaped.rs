//! The shapes a simulator file and a physics model used to be written in,
//! refused.
//!
//! A simulated payload was an entry of its own, its name and its kind beside
//! its settings, and a group of settings was named by `name`:
//!
//! ```yaml
//! simulated_payload_groups:
//!   - name: zone_temps
//!     packet_interval: 100 ms
//!
//! simulated_payloads:
//!   - name: temp_1
//!     type: network
//!     group: zone_temps
//! ```
//!
//! and a physics model's components were written the same way, `name:` and
//! `type:` beside the figures. In XML a simulated payload was a
//! `<simulated_payloads>` element of its own, with `<name>` and `<type>` in
//! it, and a group a `<simulated_payload_groups>` element.
//!
//! The simulated payloads are grouped by type now, a payload named by the
//! word `payload`; a group is named by the word `group`; and a model's
//! components are grouped by type, each named by the word `component`: see
//! [`crate::sim_grouped`] and [`crate::physics::grouped`]. A file in the old
//! shape is refused here, at the line of its first entry, and shown how that
//! entry is written now, as [`crate::graph::reshaped`] refuses a
//! configuration in its old shape and for the same reason: a file that was
//! right once deserves to be told what to write, and not what serde did not
//! expect.
//!
//! What a file says is read as its language reads it, so that a word in a
//! comment is not taken for a key; the text is searched only for the line
//! to point at.

use quick_xml::events::Event;

use crate::format::ConfigFormat;
use crate::graph::reshaped::{line_of_key, scalar, CONVERTER};

/// Says how a simulator file still written in the old shape must be written
/// now.
///
/// # Arguments
///
/// * `text`   - The file's text.
/// * `format` - What it is written in.
///
/// # Returns
///
/// A message naming the line of the first entry in the old shape and showing
/// it in the new; or `None` for a file in the new shape, or one that does
/// not parse, which the parser has its own words for.
pub fn sim_refused(text: &str, format: ConfigFormat) -> Option<String> {
    match format {
        ConfigFormat::Yaml => sim_in_yaml(text),
        ConfigFormat::Xml => sim_in_xml(text),
    }
}

/// Says how a physics model still written in the old shape must be written
/// now.
///
/// # Arguments
///
/// * `text` - The model's text, which is YAML.
///
/// # Returns
///
/// A message naming the line of the first component in the old shape and
/// showing it in the new; or `None` for a model in the new shape, or one
/// that does not parse.
pub fn physics_refused(text: &str) -> Option<String> {
    let file: serde_norway::Value = serde_norway::from_str(text).ok()?;
    let (name, component_type) = file
        .get("physics")?
        .as_sequence()?
        .iter()
        .find(|entry| is_old(entry))
        .map(|entry| (scalar(entry.get("name")), scalar(entry.get("type"))))?;
    let line = line_of_key(text, "physics", "name", &name);
    Some(format!(
        "at line {line}, the physics component \"{name}\" is written as a \
         component was before components were grouped by type, with name: \
         and type: of its own. Write it under its type now, named by the \
         word component, as `physics: [ {{ type {component_type}: [ {{ \
         component: {name}, ... }} ] }} ]`; {CONVERTER}"
    ))
}

/// Whether a YAML entry of a list is in the old shape: a mapping with a
/// `name` or a `type` of its own.
///
/// # Arguments
///
/// * `entry` - The entry.
///
/// # Returns
///
/// Whether it is.
fn is_old(entry: &serde_norway::Value) -> bool {
    entry.get("name").is_some() || entry.get("type").is_some()
}

/// Finds the first entry a YAML simulator file writes in the old shape.
///
/// # Arguments
///
/// * `text` - The file's text.
///
/// # Returns
///
/// The refusal, or `None`.
fn sim_in_yaml(text: &str) -> Option<String> {
    let file: serde_norway::Value = serde_norway::from_str(text).ok()?;
    let entries = |section: &str| -> Vec<serde_norway::Value> {
        file.get(section)
            .and_then(serde_norway::Value::as_sequence)
            .cloned()
            .unwrap_or_default()
    };
    let groups = entries("simulated_payload_groups");
    let mut found: Vec<(usize, String)> = Vec::new();

    if let Some(group) = groups.iter().find(|entry| is_old(entry)) {
        let name = scalar(group.get("name"));
        let line = line_of_key(text, "simulated_payload_groups", "name", &name);
        found.push((
            line,
            format!(
                "at line {line}, the simulated payload group \"{name}\" is \
                 written as a group was before simulated payloads were \
                 grouped by type, with name: of its own. Write it named by \
                 the word group, as `simulated_payload_groups: [ {{ group: \
                 {name}, ... }} ]`, and say no type in it: a payload's type \
                 is the type it is written under; {CONVERTER}"
            ),
        ));
    }

    let payloads = entries("simulated_payloads");
    if let Some(payload) = payloads.iter().find(|entry| is_old(entry)) {
        let name = scalar(payload.get("name"));
        // A payload took its kind from its group, where it did not say it.
        let kind = payload
            .get("type")
            .or_else(|| {
                let group = payload.get("group")?;
                groups
                    .iter()
                    .find(|g| g.get("name") == Some(group))?
                    .get("type")
            })
            .map_or_else(|| "<type>".to_string(), |t| scalar(Some(t)));
        let line = line_of_key(text, "simulated_payloads", "name", &name);
        found.push((
            line,
            format!(
                "at line {line}, the simulated payload \"{name}\" is written \
                 as a simulated payload was before they were grouped by type, \
                 with name: and type: of its own. Write it under its type now, \
                 named by the word payload, as `simulated_payloads: [ {{ type \
                 {kind}: [ {{ payload: {name}, ... }} ] }} ]`; {CONVERTER}"
            ),
        ));
    }
    found
        .into_iter()
        .min_by_key(|(line, _)| *line)
        .map(|(_, why)| why)
}

/// What is known of an XML entry in the old shape while it is being read.
struct Old {
    /// Its line.
    line: usize,
    /// The section it is: `simulated_payloads` or
    /// `simulated_payload_groups`.
    section: String,
    /// What its `name` and `type` elements say, as far as they are read.
    name: Option<String>,
    kind: Option<String>,
    /// Whether it is in the old shape: decided by what is in it.
    old: bool,
}

/// Finds the first entry an XML simulator file writes in the old shape.
///
/// # Arguments
///
/// * `text` - The file's text.
///
/// # Returns
///
/// The refusal, or `None`.
fn sim_in_xml(text: &str) -> Option<String> {
    let mut reader = quick_xml::Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let line_at = |at: u64| {
        let at = usize::try_from(at).unwrap_or(usize::MAX).min(text.len());
        text.as_bytes()[..at]
            .iter()
            .filter(|b| **b == b'\n')
            .count()
            + 1
    };
    let mut depth = 0usize;
    let mut entry: Option<Old> = None;
    // The element whose text is being read, as name or type.
    let mut reading: Option<String> = None;
    loop {
        let event = reader.read_event().ok()?;
        let at = line_at(reader.buffer_position());
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(tag) | Event::Empty(tag) => {
                let element = tag.name().as_ref().to_string();
                let level = depth + 1;
                if level == 2
                    && (element == "simulated_payloads"
                        || element == "simulated_payload_groups")
                {
                    entry = Some(Old {
                        line: at,
                        section: element.clone(),
                        name: None,
                        kind: None,
                        old: false,
                    });
                }
                if level == 3 {
                    if let Some(old) = entry.as_mut() {
                        let named = tag
                            .attributes()
                            .flatten()
                            .any(|a| a.key.as_ref() == "name");
                        if element == "name" || element == "type" && !named {
                            old.old = true;
                            reading = (!empty).then(|| element.clone());
                        }
                    }
                }
                if !empty {
                    depth = level;
                }
            }
            Event::Text(said) => {
                if let (Some(field), Some(old)) =
                    (reading.as_ref(), entry.as_mut())
                {
                    let said = said.into_inner();
                    let said = said.trim().to_string();
                    if field == "name" {
                        old.name = Some(said);
                    } else {
                        old.kind = Some(said);
                    }
                }
            }
            Event::End(_) => {
                reading = None;
                if depth == 2 {
                    if let Some(old) = entry.take().filter(|old| old.old) {
                        return Some(xml_refused(&old));
                    }
                }
                depth = depth.saturating_sub(1);
            }
            Event::Eof => return None,
            _ => {}
        }
    }
}

/// The refusal of an XML entry in the old shape.
///
/// # Arguments
///
/// * `old` - The entry, as far as it was read.
///
/// # Returns
///
/// The message.
fn xml_refused(old: &Old) -> String {
    let name = old.name.as_deref().unwrap_or("...");
    if old.section == "simulated_payload_groups" {
        return format!(
            "at line {}, the simulated payload group \"{name}\" is written as \
             a group was before simulated payloads were grouped by type, as a \
             <simulated_payload_groups> element with <name> in it. Write the \
             groups in one, each named by an attribute, as \
             <simulated_payload_groups><group name=\"{name}\">...</group>\
             </simulated_payload_groups>, and say no type in it; {CONVERTER}",
            old.line
        );
    }
    let kind = old.kind.as_deref().unwrap_or("...");
    format!(
        "at line {}, the simulated payload \"{name}\" is written as a \
         simulated payload was before they were grouped by type, as a \
         <simulated_payloads> element with <name> and <type> in it. Write it \
         under its type now, as <simulated_payloads><type name=\"{kind}\">\
         <payload name=\"{name}\">...</payload></type></simulated_payloads>; \
         {CONVERTER}",
        old.line
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A YAML simulated payload with name: and type: is refused at its line,
    /// shown written under its type, and one taking its type from its group
    /// is shown under that.
    #[test]
    fn a_yaml_payload_of_the_old_shape_is_refused_at_its_line() {
        let text = "tcssim:\n  ready: 127.0.0.1:7900\n\
                    simulated_payloads:\n  \
                    # a sensor\n  \
                    - name: temp_1\n    \
                    type: network\n";
        let why = sim_refused(text, ConfigFormat::Yaml).expect("refused");
        assert!(why.starts_with("at line 5,"), "{why}");
        assert!(why.contains("type network:"), "{why}");
        assert!(why.contains("payload: temp_1"), "{why}");
        assert!(why.contains("convert-format.py"), "{why}");
        assert!(!why.contains('\n'), "one line: {why}");

        let text = "simulated_payload_groups:\n  - group: g\n    \
                    packet_interval: 1 s\n\
                    simulated_payloads:\n  - name: a\n    group: g\n";
        let why = sim_refused(text, ConfigFormat::Yaml).expect("refused");
        assert!(why.starts_with("at line 5,"), "{why}");
        assert!(why.contains("type <type>:"), "{why}");
    }

    /// A group of the old shape is refused, and its type shown on the
    /// payloads that took it.
    #[test]
    fn a_yaml_group_of_the_old_shape_is_refused() {
        let text = "simulated_payload_groups:\n  - name: g\n    \
                    type: i2c\n\
                    simulated_payloads:\n  - name: a\n    group: g\n";
        let why = sim_refused(text, ConfigFormat::Yaml).expect("refused");
        assert!(why.starts_with("at line 2,"), "{why}");
        assert!(why.contains("group: g"), "{why}");
    }

    /// An XML entry with <name> in it is refused at its line.
    #[test]
    fn an_xml_entry_of_the_old_shape_is_refused_at_its_line() {
        let text = "<simulator>\n<simulated_payloads>\n<name>a</name>\n\
                    <type>spi</type>\n</simulated_payloads>\n</simulator>\n";
        let why = sim_refused(text, ConfigFormat::Xml).expect("refused");
        assert!(why.starts_with("at line 2,"), "{why}");
        assert!(why.contains("<type name=\"spi\">"), "{why}");
        assert!(why.contains("<payload name=\"a\">"), "{why}");

        let text = "<simulator>\n<simulated_payload_groups>\n\
                    <name>g</name>\n</simulated_payload_groups>\n\
                    </simulator>\n";
        let why = sim_refused(text, ConfigFormat::Xml).expect("refused");
        assert!(why.starts_with("at line 2,"), "{why}");
        assert!(why.contains("<group name=\"g\">"), "{why}");
    }

    /// A physics component of the old shape is refused at its line.
    #[test]
    fn a_physics_component_of_the_old_shape_is_refused_at_its_line() {
        let text = "description: d\nphysics:\n  - name: zone\n    \
                    type: thermal_zone\n";
        let why = physics_refused(text).expect("refused");
        assert!(why.starts_with("at line 3,"), "{why}");
        assert!(why.contains("type thermal_zone:"), "{why}");
        assert!(why.contains("component: zone"), "{why}");
    }

    /// Files in the new shape are taken, and so is a word of the old one
    /// only talked about.
    #[test]
    fn files_in_the_new_shape_are_taken() {
        let yaml = "# - name: a\n#   type: spi\n\
                    description: \"name: type:\"\n\
                    simulated_payload_groups:\n  - group: g\n    \
                    simulate: true\n\
                    simulated_payloads:\n  - type spi:\n      \
                    - payload: a\n        group: g\n";
        assert_eq!(sim_refused(yaml, ConfigFormat::Yaml), None);
        let xml = "<simulator>\n<!-- <simulated_payloads><name>a</name> -->\n\
                   <simulated_payload_groups><group name=\"g\">\
                   <simulate>true</simulate></group>\
                   </simulated_payload_groups>\n\
                   <simulated_payloads><type name=\"spi\">\
                   <payload name=\"a\"><group>g</group></payload></type>\
                   </simulated_payloads>\n</simulator>\n";
        assert_eq!(sim_refused(xml, ConfigFormat::Xml), None);
        let physics = "physics:\n  - type bus:\n      - component: b\n";
        assert_eq!(physics_refused(physics), None);
    }
}
