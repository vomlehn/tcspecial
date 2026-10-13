//! The schemas say what the software says.
//!
//! `tcsgui/share/schema` holds a schema for each file tcsgui edits, in each
//! spelling: a JSON Schema for the YAML and an XSD for the XML. They are data,
//! installed on a workstation so that somebody's own editor can tell them that
//! `payloads` is a section and `payload` is not.
//!
//! Data that nothing checks is data that drifts, and a schema that has drifted
//! is worse than no schema: it refuses files the software accepts, or accepts
//! what the software refuses, and either way the person believes the schema.
//! So these tests hold them against the lists the software itself reads from --
//! not against a copy of those lists written out here, which would be the same
//! problem one step further away.
//!
//! What they do not check is whether a configuration can be flown. The schemas
//! are about shape, `tcsverify` is about meaning, and `share/schema/README`
//! says so to whoever installs them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the schemas this repository ships are.
fn schema() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("share/schema")
}

/// The repository's root, for the shipped configurations.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// One schema, read.
fn read(name: &str) -> serde_json::Value {
    let path = schema().join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The property names of an object in a schema, at a path of keys.
fn properties(schema: &serde_json::Value, at: &[&str]) -> BTreeSet<String> {
    let mut here = schema;
    for key in at {
        here = here
            .get("properties")
            .and_then(|properties| properties.get(key))
            .unwrap_or_else(|| panic!("the schema has no {key}"));
        // An array's shape is its items', which is where an entry's own
        // properties are.
        if here.get("items").is_some() {
            here = &here["items"];
        }
    }
    here.get("properties")
        .and_then(|properties| properties.as_object())
        .map(|properties| properties.keys().cloned().collect())
        .unwrap_or_default()
}

/// The configuration schema offers the sections a configuration has.
#[test]
fn the_configuration_schema_has_the_sections_the_reader_reads() {
    let schema = read("tcspecial.schema.json");
    let sections = properties(&schema, &[]);

    // The three node sections are the editor's three working panes, and the
    // rest is what a file says about itself and about the whole of it.
    for section in [
        "version",
        "description",
        "include",
        "tcspecial",
        "payload_groups",
        "moc",
        "vehicle",
        "payloads",
        "commands",
    ] {
        assert!(sections.contains(section), "the schema has no {section}");
    }
    // And nothing else: a section the schema offers and the reader refuses
    // would be a schema telling somebody to write a file that cannot be read.
    assert_eq!(sections.len(), 9, "{sections:?}");

    // The sections that are gone are gone from the schema too. A file written
    // to the earlier shape is refused where it is read, and an editor offering
    // to complete `links:` would be an editor arguing with the reader.
    for retired in ["nodes", "links", "groups"] {
        assert!(
            !sections.contains(retired),
            "{retired} is no longer a section"
        );
    }
}

/// What the tcspecial section may say, held against the one list of it.
///
/// `graph::parameters::ATTRIBUTES` is what the checker refuses anything else
/// by, what the editor builds its window from, and now what the schema is
/// measured against: three readers of one list rather than three lists.
#[test]
fn the_tcspecial_section_is_what_the_parameters_say() {
    let schema = read("tcspecial.schema.json");

    let mut wanted: BTreeSet<String> = BTreeSet::new();
    for (attribute, _) in tcslibgs::graph::parameters::ATTRIBUTES {
        // Dotted in the rules, nested in the file: `beacon.send` is `send`
        // under `beacon`. One of them is not under a subsection at all --
        // `rust-code-path`, which is about building the software rather than
        // running it -- and sits at the top of the section.
        match attribute.split_once('.') {
            Some((parent, name)) => {
                wanted.insert(parent.to_string());
                let under = properties(&schema, &["tcspecial", parent]);
                assert!(
                    under.contains(name),
                    "the schema's {parent} has no {name}: {under:?}"
                );
            }
            None => {
                wanted.insert((*attribute).to_string());
            }
        }
    }

    let said = properties(&schema, &["tcspecial"]);
    assert_eq!(said, wanted, "the subsections do not match");
}

/// What a simulated payload may say, held against the one list of it.
#[test]
fn the_simulator_schema_is_what_the_settings_say() {
    let schema = read("tcspecial-sim.schema.json");

    for section in ["simulated_payloads", "simulated_payload_groups"] {
        let said = properties(&schema, &[section]);
        for setting in tcslibgs::sim_config::SETTINGS {
            assert!(
                said.contains(*setting),
                "{section} has no {setting}: {said:?}"
            );
        }
        assert!(said.contains("name"));
        // A payload takes its group's settings for whatever it does not state;
        // a group names no group of its own.
        assert_eq!(
            said.contains("group"),
            section == "simulated_payloads",
            "{section}: {said:?}"
        );
    }

    // And no version: a configuration states one so that it can be checked
    // against the software that will read it on the vehicle, and nothing
    // aboard reads this file.
    assert!(!properties(&schema, &[]).contains("version"));
}

/// Every shipped XML file is valid against its schema.
///
/// Which is what makes the XSDs worth shipping: a schema nothing is checked
/// against is a schema that has drifted and nobody has noticed. It also keeps
/// the editor's own XML writer honest -- an XML comment may not contain two
/// hyphens in a row, and the writer learned that here.
///
/// Skipped, loudly, where xmllint is not installed: a test that fails because
/// a tool is absent teaches a reader to ignore it.
#[test]
fn every_shipped_xml_file_is_valid_against_its_schema() {
    if Command::new("xmllint").arg("--version").output().is_err() {
        eprintln!("xmllint is not installed: the XML schemas were not checked");
        return;
    }

    let files: &[(&str, &str)] = &[
        ("tcspecial.xsd", "example.xml"),
        ("tcspecial.xsd", "tests/manual/thermostat.xml"),
    ];
    for (schema_name, file) in files {
        let output = Command::new("xmllint")
            .arg("--noout")
            .arg("--schema")
            .arg(schema().join(schema_name))
            .arg(root().join(file))
            .output()
            .expect("xmllint runs");
        assert!(
            output.status.success(),
            "{file} is not valid against {schema_name}:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Every key of every shipped configuration is one the schema allows.
///
/// The other half of the XML test, for the spelling that has no validator to
/// hand: the file is read as YAML and every name in it is looked for in the
/// schema. It is a shallower check than a validator's -- it says nothing about
/// types or patterns -- and it catches the drift that matters, which is a word
/// the software reads and the schema has never heard of.
#[test]
fn every_key_of_a_shipped_configuration_is_in_the_schema() {
    let schema = read("tcspecial.schema.json");

    for file in ["example.yaml", "tests/manual/thermostat.yaml"] {
        let path = root().join(file);
        let text = std::fs::read_to_string(&path).expect("a shipped configuration");
        let said: serde_norway::Value =
            serde_norway::from_str(&text).unwrap_or_else(|e| panic!("{file}: {e}"));

        let sections = properties(&schema, &[]);
        let mapping = said.as_mapping().expect("a configuration is a mapping");
        for (key, value) in mapping {
            let key = key.as_str().expect("a section is named").to_string();
            assert!(sections.contains(&key), "{file}: {key} is not in the schema");

            // The node sections, whose entries are what a schema has most to
            // say about.
            if !["moc", "vehicle", "payloads"].contains(&key.as_str()) {
                continue;
            }
            let allowed = properties(&schema, &[&key]);
            for node in value.as_sequence().expect("a list of nodes") {
                let node = node.as_mapping().expect("a node is a mapping");
                for (attribute, _) in node {
                    let attribute = attribute.as_str().expect("named").to_string();
                    assert!(
                        allowed.contains(&attribute),
                        "{file}: {key} has a node stating {attribute}, which the \
                         schema does not allow"
                    );
                }
            }
        }
    }
}
