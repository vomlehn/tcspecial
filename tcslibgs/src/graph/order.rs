//! The order a file declares its nodes in.
//!
//! A node's device id is its position among the configuration's nodes, and
//! that has to be the order a person reading the file sees them in: the order
//! they are written. The parsed file does not keep it -- the nodes arrive
//! sorted into their three sections, and in the XML spelling, where each node
//! is an element of its own, sections may interleave -- so it is read here
//! from the text itself and the loader puts each file's nodes in this order.
//! See `crate::graph::telemetry` for what the ids are for.

use std::path::Path;

use quick_xml::events::Event;

use crate::error::{TcsError, TcsResult};
use crate::format::ConfigFormat;

/// The sections a node is written under.
const SECTIONS: &[&str] = &["moc", "vehicle", "payloads"];

/// The names of the nodes this file declares, in the order it declares them.
pub fn declared(path: &Path) -> TcsResult<Vec<String>> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| TcsError::Config(format!("{}: {e}", path.display())))?;
    match ConfigFormat::of_file(path)? {
        ConfigFormat::Yaml => yaml(&text),
        ConfigFormat::Xml => xml(&text),
    }
    .map_err(|why| TcsError::Config(format!("{}: {why}", path.display())))
}

/// In YAML each section is a key once, so the order is the keys' order and
/// then each section's entries'.
fn yaml(text: &str) -> Result<Vec<String>, String> {
    let file: serde_norway::Value =
        serde_norway::from_str(text).map_err(|e| e.to_string())?;
    let mut names = Vec::new();
    let Some(mapping) = file.as_mapping() else {
        return Ok(names);
    };
    for (key, value) in mapping {
        if !key.as_str().is_some_and(|k| SECTIONS.contains(&k)) {
            continue;
        }
        for entry in value.as_sequence().into_iter().flatten() {
            if let Some(name) = entry.get("name").and_then(|n| n.as_str()) {
                names.push(name.to_string());
            }
        }
    }
    Ok(names)
}

/// In XML each node is an element under the root, named for its section, with
/// the node's name in a `name` element of its own.
fn xml(text: &str) -> Result<Vec<String>, String> {
    let mut reader = quick_xml::Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut names = Vec::new();
    let mut depth = 0usize;
    // Inside a node element, and inside its own `name` element.
    let mut in_node = false;
    let mut in_name = false;
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(element) => {
                depth += 1;
                let tag = element.name();
                let tag: &str = tag.as_ref();
                if depth == 2 && SECTIONS.contains(&tag) {
                    in_node = true;
                } else if depth == 3 && in_node && tag == "name" {
                    in_name = true;
                }
            }
            Event::Text(said) if in_name => {
                names.push(said.into_inner().trim().to_string());
            }
            Event::End(_) => {
                if depth == 3 {
                    in_name = false;
                }
                if depth == 2 {
                    in_node = false;
                }
                depth = depth.saturating_sub(1);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(
        name: &str,
        text: &str,
    ) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join(name);
        std::fs::write(&path, text).expect("written");
        (dir, path)
    }

    /// YAML: the sections in the order they are written, not moc first.
    #[test]
    fn yaml_sections_are_in_the_order_written() {
        let (_dir, path) = written(
            "v.yaml",
            "payloads:
  - { name: p1, type: i2c }
  - { name: p2, type: i2c }
moc:
  - { name: m, type: moc }
vehicle:
  - { name: v, type: \"null\" }
",
        );
        assert_eq!(declared(&path).unwrap(), vec!["p1", "p2", "m", "v"]);
    }

    /// XML: elements in the order written, sections interleaved.
    #[test]
    fn xml_elements_are_in_the_order_written() {
        let (_dir, path) = written(
            "v.xml",
            "<?xml version=\"1.0\"?>
<configuration>
    <payloads><name>p1</name><type>i2c</type></payloads>
    <moc><name>m</name><type>moc</type></moc>
    <payloads><name>p2</name><type>i2c</type></payloads>
    <tcsgui><nodes><name>p1</name><x>0</x><y>0</y></nodes></tcsgui>
</configuration>
",
        );
        assert_eq!(declared(&path).unwrap(), vec!["p1", "m", "p2"]);
    }
}
