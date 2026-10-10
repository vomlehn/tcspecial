//! Which line of a file an entry was written on.
//!
//! The rules in `tcslibgs` have never seen a file. A parser hands up a
//! document and the lines are gone by then, so a problem says which payload
//! or group it is about and this works out where that payload or group is
//! written. It is the same thing a reader does with the error a program
//! prints: search the file for the name.
//!
//! Which is why this reads the text rather than the document. A line number
//! cannot be had from the document at all, and the alternative -- carrying
//! one through every struct in the library -- would put the file's shape into
//! types that exist so that two file formats can describe one thing.
//!
//! It is a scan and not a parse. The file has already parsed by the time this
//! is asked, so the question is never what the file means, only where a name
//! appears in it; and a file that has *not* parsed is reported from where the
//! parser stopped, which the parser says itself.

use std::collections::BTreeMap;

use tcslibgs::{About, ConfigFormat};

/// Where each named entry of a file is, and where each of its sections is.
///
/// Keyed by the section an entry is in as well as by its name, because a
/// payload and a group of them may share a name without being the same thing.
#[derive(Debug, Default)]
pub struct Where {
    entries: BTreeMap<(String, String), usize>,
    sections: BTreeMap<String, usize>,
}

/// The line a problem about nothing in particular is reported on.
///
/// The first, which is where a reader opens the file.
pub const THE_FILE_ITSELF: usize = 1;

impl Where {
    /// Scan `text` for the entries of a file written in `format`.
    pub fn of(text: &str, format: ConfigFormat) -> Where {
        match format {
            ConfigFormat::Yaml => yaml(text),
            ConfigFormat::Xml => xml(text),
        }
    }

    /// The line to report a problem about `about` on.
    ///
    /// The first line an entry's name is written on. A name written twice --
    /// which is itself a problem -- is reported from the first of them, that
    /// being the definition the others are duplicates of.
    pub fn line_of(&self, about: &About) -> usize {
        let (section, name) = match about {
            About::File => return THE_FILE_ITSELF,
            About::Section(section) => {
                return self
                    .sections
                    .get(*section)
                    .copied()
                    .unwrap_or(THE_FILE_ITSELF)
            }
            About::Payload(name) => ("payloads", name),
            About::Group(name) => ("payload_groups", name),
            About::SimPayload(name) => ("simulated_payloads", name),
            About::SimGroup(name) => ("simulated_payload_groups", name),
        };

        self.entries
            .get(&(section.to_string(), name.clone()))
            .copied()
            // The section it belongs to, if the entry itself cannot be found:
            // a problem about a payload a reader cannot be pointed at is
            // still a problem about the payloads.
            .or_else(|| self.sections.get(section).copied())
            .unwrap_or(THE_FILE_ITSELF)
    }

    fn note(&mut self, section: &str, name: &str, line: usize) {
        if section.is_empty() || name.is_empty() {
            return;
        }
        self.entries
            .entry((section.to_string(), name.to_string()))
            .or_insert(line);
    }

    fn note_section(&mut self, section: &str, line: usize) {
        self.sections.entry(section.to_string()).or_insert(line);
    }
}

/// What a YAML file says where.
///
/// A section is a key at the left margin; an entry is a `name:` under one.
/// That is the whole of the shape these files have -- every section is a list
/// of named things, or the `tcspecial` mapping -- so nothing here has to know
/// what any of them mean.
fn yaml(text: &str) -> Where {
    let mut found = Where::default();
    let mut section = String::new();

    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // A key at the left margin opens a section, whether or not it has a
        // value of its own: `payloads:` opens one and `version: "1.0"` opens
        // one nothing ever asks about.
        if !line.starts_with(' ') && !line.starts_with('\t') {
            if let Some(key) = trimmed.split(':').next() {
                section = key.trim().to_string();
                found.note_section(&section, number);
            }
            continue;
        }

        // An entry's name, whether it is the first key of a list item or a
        // later one.
        let key = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        if let Some(value) = key.strip_prefix("name:") {
            found.note(&section, unquoted(value), number);
        }
    }

    found
}

/// What an XML file says where.
///
/// The sections are the elements under the root and the entries are their
/// `name` children, which is the convention a payload configuration in XML
/// follows. A `name` attribute is read as well: it is how the format that is
/// gone wrote one, and a file written that way is better pointed at than not.
fn xml(text: &str) -> Where {
    let mut found = Where::default();
    let mut section = String::new();
    let mut depth = 0usize;

    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let mut rest = line;

        while let Some(start) = rest.find('<') {
            let after = &rest[start + 1..];
            let Some(end) = after.find('>') else { break };
            let tag = &after[..end];
            rest = &after[end + 1..];

            // Declarations, comments and doctypes are not elements.
            if tag.starts_with('?') || tag.starts_with('!') {
                continue;
            }

            if tag.starts_with('/') {
                depth = depth.saturating_sub(1);
                continue;
            }

            let self_closing = tag.ends_with('/');
            let body = tag.trim_end_matches('/').trim();
            let element = body.split_whitespace().next().unwrap_or("");

            // An element under the root is a section. One with no children --
            // a payload written entirely as attributes -- is its own section
            // as well as its own entry.
            if depth == 1 {
                section = element.to_string();
                found.note_section(&section, number);
            }

            if let Some(value) = attribute(body, "name") {
                found.note(&section, value, number);
            }

            if self_closing {
                continue;
            }
            depth += 1;

            // `<name>DH0</name>`, which is how a payload file written in XML
            // says which payload this is.
            if element == "name" {
                if let Some(close) = rest.find("</") {
                    found.note(&section, rest[..close].trim(), number);
                }
            }
        }
    }

    found
}

/// The value of one attribute of a start tag, if it has one.
fn attribute<'a>(body: &'a str, wanted: &str) -> Option<&'a str> {
    let mut rest = body;
    while let Some(equals) = rest.find('=') {
        let key = rest[..equals].trim().rsplit(char::is_whitespace).next()?;
        let after = rest[equals + 1..].trim_start();
        let quote = after.chars().next()?;
        if quote != '"' && quote != '\'' {
            rest = &rest[equals + 1..];
            continue;
        }
        let value_start = &after[quote.len_utf8()..];
        let close = value_start.find(quote)?;
        if key == wanted {
            return Some(&value_start[..close]);
        }
        rest = &value_start[close + quote.len_utf8()..];
    }
    None
}

/// A YAML scalar as the name it is, without the quotes a file may put on it.
fn unquoted(value: &str) -> &str {
    let value = value.trim();
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|v| v.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "\
version: \"1.0\"
description: a set

tcspecial:
  address: 0.0.0.0
  port: 4000

payload_groups:
  - name: shared
    type: device

payloads:
  - dh_id: 0
    name: DH0
    group: shared
    packet_size: 1

  - dh_id: 1
    name: \"DH1\"
    type: device
    path: /dev/zero
    packet_size: 1
";

    #[test]
    fn a_yaml_file_says_where_each_payload_is() {
        let found = Where::of(YAML, ConfigFormat::Yaml);

        assert_eq!(found.line_of(&About::Payload("DH0".to_string())), 14);
        // Quoted or not: the name is the name.
        assert_eq!(found.line_of(&About::Payload("DH1".to_string())), 19);
        assert_eq!(found.line_of(&About::Group("shared".to_string())), 9);
        assert_eq!(found.line_of(&About::Section("tcspecial")), 4);
    }

    /// A payload and a group of them may share a name, and the two are not
    /// the same line.
    #[test]
    fn a_name_is_looked_for_in_the_section_it_belongs_to() {
        let text = "payload_groups:\n  - name: alike\npayloads:\n  - name: alike\n";
        let found = Where::of(text, ConfigFormat::Yaml);
        assert_eq!(found.line_of(&About::Group("alike".to_string())), 2);
        assert_eq!(found.line_of(&About::Payload("alike".to_string())), 4);
    }

    /// A name the file does not have falls back to the section, and a section
    /// it does not have to the first line.
    ///
    /// Not a guess at a line: a problem has to be reported somewhere, and the
    /// section an entry belongs in is the next most useful place after the
    /// entry itself.
    #[test]
    fn what_cannot_be_found_is_reported_from_the_nearest_thing() {
        let found = Where::of(YAML, ConfigFormat::Yaml);
        assert_eq!(
            found.line_of(&About::Payload("nobody".to_string())),
            12,
            "the payloads section is where a payload would be"
        );
        assert_eq!(found.line_of(&About::SimPayload("nobody".to_string())), 1);
        assert_eq!(found.line_of(&About::File), 1);
    }

    #[test]
    fn a_simulator_file_says_where_each_payload_is() {
        let text = "\
version: \"1.0\"
description: settings

simulated_payload_groups:
  - name: steady
    packet_interval_ms: 1000

simulated_payloads:
  - name: DH0
    type: device
    group: steady
";
        let found = Where::of(text, ConfigFormat::Yaml);
        assert_eq!(found.line_of(&About::SimGroup("steady".to_string())), 5);
        assert_eq!(found.line_of(&About::SimPayload("DH0".to_string())), 9);
    }

    #[test]
    fn an_xml_file_says_where_each_payload_is() {
        let text = "\
<payload-configuration>
  <version>1.0</version>
  <payload_groups>
    <name>shared</name>
    <type>device</type>
  </payload_groups>
  <payloads>
    <dh_id>0</dh_id>
    <name>DH0</name>
    <group>shared</group>
  </payloads>
  <payloads>
    <dh_id>1</dh_id>
    <name>DH1</name>
  </payloads>
</payload-configuration>
";
        let found = Where::of(text, ConfigFormat::Xml);
        assert_eq!(found.line_of(&About::Group("shared".to_string())), 4);
        assert_eq!(found.line_of(&About::Payload("DH0".to_string())), 9);
        assert_eq!(found.line_of(&About::Payload("DH1".to_string())), 14);
    }

    /// A name written as an attribute is found too, on one line or several.
    #[test]
    fn an_xml_name_may_be_an_attribute() {
        let text = "<doc>\n  <payloads name=\"DH0\" dh_id=\"0\"/>\n  \
                    <payloads dh_id='1' name='DH1'/>\n</doc>\n";
        let found = Where::of(text, ConfigFormat::Xml);
        assert_eq!(found.line_of(&About::Payload("DH0".to_string())), 2);
        assert_eq!(found.line_of(&About::Payload("DH1".to_string())), 3);
    }

    /// The scan does not mistake the root for a section, nor a closing tag
    /// for an opening one.
    ///
    /// Both are how a depth count goes wrong, and a wrong depth puts every
    /// entry in the wrong section -- which reads as a file whose payloads are
    /// nowhere.
    #[test]
    fn the_root_is_not_a_section() {
        let text = "<payload-configuration><payloads><name>DH0</name></payloads>\
                    </payload-configuration>\n";
        let found = Where::of(text, ConfigFormat::Xml);
        assert_eq!(found.line_of(&About::Payload("DH0".to_string())), 1);
    }
}
