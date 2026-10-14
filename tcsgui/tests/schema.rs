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
        // The editor's own, last in the file: where each node was drawn.
        "tcsgui",
    ] {
        assert!(sections.contains(section), "the schema has no {section}");
    }
    // And nothing else: a section the schema offers and the reader refuses
    // would be a schema telling somebody to write a file that cannot be read.
    assert_eq!(sections.len(), 10, "{sections:?}");

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

/// Where the descriptions this repository ships are.
fn nodes() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("share/nodes")
}

/// Every description in the tree, by path relative to its root.
fn described() -> Vec<PathBuf> {
    fn walk(root: &Path, here: &Path, found: &mut Vec<PathBuf>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(here)
            .unwrap_or_else(|e| panic!("{}: {e}", here.display()))
            .map(|entry| entry.expect("an entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(root, &path, found);
            } else if matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("yaml") | Some("yml") | Some("xml")
            ) {
                found.push(path.strip_prefix(root).expect("under the root").to_path_buf());
            }
        }
    }

    let mut found = Vec::new();
    walk(&nodes(), &nodes(), &mut found);
    assert!(!found.is_empty(), "the description tree is empty");
    found
}

/// Every description file is valid against the schema for its spelling.
///
/// The tree is data the editor reads at run time, which is exactly the kind of
/// data that drifts: a misspelled word in one of these files is a palette
/// entry that quietly fails to appear. The XML ones go to `xmllint`; the YAML
/// ones have every key looked for in the JSON Schema, which is the same
/// shallower check the shipped configurations get and catches the same thing.
#[test]
fn every_node_description_is_valid_against_its_schema() {
    let schema_json = read("node.schema.json");
    let allowed = properties(&schema_json, &[]);
    let allowed_attribute = properties(&schema_json, &["attributes"]);
    let have_xmllint = Command::new("xmllint").arg("--version").output().is_ok();
    if !have_xmllint {
        eprintln!("xmllint is not installed: the XML descriptions were not validated");
    }

    for name in described() {
        let path = nodes().join(&name);
        if path.extension().and_then(|e| e.to_str()) == Some("xml") {
            if !have_xmllint {
                continue;
            }
            let output = Command::new("xmllint")
                .arg("--noout")
                .arg("--schema")
                .arg(schema().join("node.xsd"))
                .arg(&path)
                .output()
                .expect("xmllint runs");
            assert!(
                output.status.success(),
                "{}: not valid against node.xsd:\n{}",
                name.display(),
                String::from_utf8_lossy(&output.stderr)
            );
            continue;
        }

        let text = std::fs::read_to_string(&path).expect("a description");
        let said: serde_norway::Value = serde_norway::from_str(&text)
            .unwrap_or_else(|e| panic!("{}: {e}", name.display()));
        let mapping = said.as_mapping().expect("a description is a mapping");
        for (key, value) in mapping {
            let key = key.as_str().expect("a named word").to_string();
            assert!(
                allowed.contains(&key),
                "{}: {key} is not a word a description knows",
                name.display()
            );
            if key != "attributes" {
                continue;
            }
            for attribute in value.as_sequence().expect("a list of attributes") {
                for (word, _) in attribute.as_mapping().expect("an attribute is a mapping") {
                    let word = word.as_str().expect("a named word").to_string();
                    assert!(
                        allowed_attribute.contains(&word),
                        "{}: an attribute says {word}, which a description does not know",
                        name.display()
                    );
                }
            }
        }
    }
}

/// The two node schemas describe the same node.
///
/// One shape in two spellings, as the configuration's pair is: a word allowed
/// in YAML and refused in XML would be a description that reads in one
/// spelling and not in the other, which is the one thing a pair of schemas
/// must not say. Compared by the words each allows, which is what a schema is
/// read for.
#[test]
fn the_two_node_schemas_describe_the_same_node() {
    let json = read("node.schema.json");
    let xsd = std::fs::read_to_string(schema().join("node.xsd")).expect("the XSD");

    // What the XSD allows inside <node>, read from its element declarations.
    let declared: BTreeSet<String> = xsd
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("<xs:element name=\"")?;
            let name = rest.split('"').next()?;
            // The document element is the file itself, not a word inside it.
            (line.contains("type=") && !line.contains("ref=")).then(|| name.to_string())
        })
        .collect();
    // `node` is both the document element and the word inside it that names
    // the type, so it is in the set either way.
    let inside: BTreeSet<String> = declared
        .iter()
        .filter(|name| !matches!(name.as_str(), "name" | "required"))
        .cloned()
        .collect();

    let allowed = properties(&json, &[]);
    assert_eq!(
        inside, allowed,
        "the two node schemas allow different words"
    );

    // And an attribute's own words agree as well.
    let attribute: BTreeSet<String> = ["name", "required", "about"]
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(
        properties(&json, &["attributes"]),
        attribute,
        "the JSON Schema's attribute is not the XSD's"
    );
    for word in &attribute {
        assert!(
            xsd.contains(&format!("<xs:element name=\"{word}\"")),
            "the XSD's attribute does not take {word}"
        );
    }
}

/// One description per node, in one spelling.
///
/// A node described twice is a palette with two entries that place the same
/// thing, and worse, two statements of what its terminals are: the editor
/// would offer whichever the tree was walked into first. A node described in
/// both spellings is the same fault wearing a second extension.
#[test]
fn every_node_is_described_once_and_in_one_spelling() {
    let mut stems: BTreeSet<PathBuf> = BTreeSet::new();
    let mut names: BTreeSet<String> = BTreeSet::new();

    for name in described() {
        let stem = name.with_extension("");
        assert!(
            stems.insert(stem.clone()),
            "{}: described twice, once in each spelling",
            stem.display()
        );

        let text = std::fs::read_to_string(nodes().join(&name)).expect("a description");
        // The node's own name, from whichever spelling this is.
        let said = if name.extension().and_then(|e| e.to_str()) == Some("xml") {
            text.lines()
                .filter_map(|line| {
                    line.trim()
                        .strip_prefix("<node>")?
                        .strip_suffix("</node>")
                        .map(str::to_string)
                })
                .next()
                .unwrap_or_else(|| panic!("{}: no node name", name.display()))
        } else {
            let said: serde_norway::Value =
                serde_norway::from_str(&text).expect("a description");
            said.get("node")
                .and_then(|node| node.as_str().map(str::to_string))
                .unwrap_or_else(|| panic!("{}: no node name", name.display()))
        };
        assert!(
            names.insert(said.clone()),
            "{}: \"{said}\" is described twice",
            name.display()
        );
    }
}
