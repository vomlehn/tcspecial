//! The schemas say what the software says.
//!
//! `tcslib-protocol/schema` holds a schema for each file tcsgui edits, in each
//! spelling: a JSON Schema for the YAML, YTCS, and an XSD for the XML, XTCS;
//! `tcsgui/share/schema` holds those of tcsgui's own node descriptions. They are data,
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
//! are about shape, `tcsverify` is about meaning, and
//! `tcslib-protocol/schema/README` says so to whoever installs them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where one of the schemas this repository ships is: the XTCS, YTCS and
/// simulator schemas in `tcslib-protocol/schema`, which every program reads,
/// and the node descriptions' own in this editor's `share/schema`.
///
/// # Arguments
///
/// * `name` - The schema's file name.
///
/// # Returns
///
/// Its path.
fn schema_of(name: &str) -> PathBuf {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    if name.starts_with("node.") {
        here.join("share/schema").join(name)
    } else {
        here.join("../tcslib-protocol/schema").join(name)
    }
}

/// The repository's root, for the shipped configurations.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// The examples, as paths from the root: every file whose name ends `ending`
/// in a directory of `examples/`, each example having one of its own.
/// Discovered, so that an example added there is held against the schemas
/// without anyone naming it here.
///
/// # Arguments
///
/// * `ending` - The end of the file names wanted, such as `.ytcs`.
///
/// # Returns
///
/// The paths, `examples/<example>/<file>`, sorted.
///
/// # Panics
///
/// If a directory of `examples/` cannot be read.
fn examples(ending: &str) -> Vec<String> {
    /// Lists the names of the entries in a directory.
    ///
    /// # Arguments
    ///
    /// * `dir` - The directory.
    ///
    /// # Returns
    ///
    /// The entries' names that are UTF-8, in the order the directory gives
    /// them.
    ///
    /// # Panics
    ///
    /// If the directory or one of its entries cannot be read.
    fn names(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .map(|entry| entry.expect("a readable directory entry").file_name())
            .filter_map(|name| name.to_str().map(str::to_string))
            .collect()
    }
    let top = root().join("examples");
    let mut found: Vec<String> = names(&top)
        .into_iter()
        .filter(|example| top.join(example).is_dir())
        .flat_map(|example| {
            names(&top.join(&example))
                .into_iter()
                .filter(|name| name.ends_with(ending))
                .map(move |name| format!("examples/{example}/{name}"))
        })
        .collect();
    found.sort();
    found
}

/// The examples' tcspecial configurations in YAML, the `.ytcs` files: the
/// simulator's beside them are `.yaml`.
///
/// # Returns
///
/// The paths, from the root, sorted.
fn example_configurations() -> Vec<String> {
    examples(".ytcs")
}

/// One schema, read.
///
/// # Arguments
///
/// * `name` - The schema's file name: see [`schema_of`].
///
/// # Returns
///
/// The schema as JSON.
///
/// # Panics
///
/// If the file cannot be read or is not JSON.
fn read(name: &str) -> serde_json::Value {
    let path = schema_of(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The property names of an object in a schema, at a path of keys.
///
/// # Arguments
///
/// * `schema` - The JSON Schema.
/// * `at`     - The keys of the nested properties to descend through, from the
///              top; an array is taken as its items.
///
/// # Returns
///
/// The names of the object's properties; none if it lists none.
///
/// # Panics
///
/// If a key on the path is not a property.
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
        "description",
        "include",
        // What of an included file the files around it may join.
        "exports",
        "tcspecial",
        "payload_groups",
        "gateways",
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
    // And no version: a configuration states none, and the reader refuses
    // one, so a schema offering it would be offering a refusal.
    assert!(!sections.contains("version"), "{sections:?}");

    // The sections that are gone are gone from the schema too. A file written
    // to the earlier shape is refused where it is read, and an editor offering
    // to complete `links:` would be an editor arguing with the reader.
    for retired in ["nodes", "links", "groups", "gateway"] {
        assert!(
            !sections.contains(retired),
            "{retired} is no longer a section"
        );
    }
}

/// The values an XSD's simple type enumerates, in the order it lists them.
///
/// # Arguments
///
/// * `xsd`         - The XSD's text.
/// * `simple_type` - The simple type's name.
///
/// # Returns
///
/// The values of its `xs:enumeration` elements.
///
/// # Panics
///
/// If the XSD has no such simple type or it is not closed.
fn enumerated(xsd: &str, simple_type: &str) -> Vec<String> {
    let opens = format!("<xs:simpleType name=\"{simple_type}\">");
    let start = xsd
        .find(&opens)
        .unwrap_or_else(|| panic!("the XSD has no simple type {simple_type}"));
    let body = &xsd
        [start..start + xsd[start..].find("</xs:simpleType>").expect("closed")];
    body.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("<xs:enumeration value=\"")?;
            Some(rest[..rest.find('"')?].to_string())
        })
        .collect()
}

/// The text of a schema in this repository.
fn text_of(name: &str) -> String {
    let path = schema_of(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Each section of a configuration takes the node types the software knows
/// for it, in both schemas, and no others.
///
/// Held against `tcslib_protocol::graph::types_in`, which is what the checker
/// refuses an unknown type by: a type the schemas offer and the checker
/// refuses, or the other way round, is a schema arguing with the software.
#[test]
fn each_section_takes_the_node_types_the_software_knows() {
    use tcslib_protocol::graph::{types_in, Section};

    let json = read("tcspecial.schema.json");
    let xsd = text_of("tcspecial.xsd");
    for (section, simple_type, group, node) in [
        (Section::Gateway, "gatewayType", "gatewayGroup", "gateway"),
        (Section::Vehicle, "vehicleType", "vehicleGroup", "compute"),
        (Section::Payloads, "payloadType", "payloadGroup", "payload"),
    ] {
        let key = section.name();
        let wanted: Vec<String> =
            types_in(section).into_iter().map(str::to_string).collect();

        // In JSON, an entry of the section is one key, the word type and
        // the type, whose value is a list of the section's nodes, each named
        // by its first key: the word for the section's nodes.
        let items = &json["properties"][key]["items"];
        let reference = items["propertyNames"]["$ref"]
            .as_str()
            .unwrap_or_else(|| panic!("{key}: no list of type keys"));
        let keys = &json["$defs"]
            [reference.strip_prefix("#/$defs/").expect("a definition")];
        let said: Vec<String> = keys["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("{key}: the JSON schema lists no types"))
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .and_then(|key| key.strip_prefix("type "))
                    .expect("a type key is the word type and a type")
                    .to_string()
            })
            .collect();
        assert_eq!(said, wanted, "{key}, in the JSON schema");
        assert_eq!(items["maxProperties"], 1, "{key}: one type an entry");
        let nodes = items["additionalProperties"]["items"]["$ref"]
            .as_str()
            .and_then(|r| r.strip_prefix("#/$defs/"))
            .unwrap_or_else(|| panic!("{key}: no node definition"));
        assert_eq!(
            json["$defs"][nodes]["required"],
            serde_json::json!([section.node_word()]),
            "{key}: a node is named by the word for its section"
        );
        let words = json["$defs"][nodes]["properties"]
            .as_object()
            .expect("a node's words");
        assert!(!words.contains_key("type"), "{key}: a node says no type");
        assert!(!words.contains_key("name"), "{key}: a node has no name key");

        // And in the XSD, the type named by the attribute of a <type>, and
        // the nodes in it elements of the word for the section's nodes.
        assert_eq!(enumerated(&xsd, simple_type), wanted, "{key}, in the XSD");
        assert_eq!(section.node_word(), node);
        let opens = format!("<xs:complexType name=\"{group}\">");
        let start = xsd
            .find(&opens)
            .unwrap_or_else(|| panic!("the XSD has no {group}"));
        let body = &xsd[start
            ..start + xsd[start..].find("</xs:complexType>").expect("closed")];
        assert!(
            body.contains(&format!(
                "<xs:element name=\"{node}\" type=\"node\""
            )),
            "{group}: {body}"
        );
        assert!(
            body.contains(&format!(
                "<xs:attribute name=\"name\" type=\"{simple_type}\""
            )),
            "{group}: {body}"
        );
    }
}

/// A simulated payload is written under one of the interfaces, in both
/// schemas, and an XML simulator file with every word in it is valid and
/// reads.
///
/// The kinds are `DHType`'s, which is what the simulator reads a type with.
/// And the XSD offers every setting the one list of them has, which the XSD
/// once did not: it was missing what attaches a payload to the physics
/// model.
#[test]
fn the_simulator_schemas_take_the_kinds_and_words_the_simulator_reads() {
    let kinds: Vec<String> = tcslib_protocol::DHType::ALL
        .iter()
        .map(|kind| kind.spelling().to_string())
        .collect();

    // In JSON, an entry of simulated_payloads is one key, the word type and
    // the kind, holding payloads named by `payload`; a group is named by
    // `group` and says no type.
    let json = read("tcspecial-sim.schema.json");
    let items = &json["properties"]["simulated_payloads"]["items"];
    assert_eq!(items["maxProperties"], 1, "one type an entry");
    let said: Vec<String> = json["$defs"]["simulatedType"]["enum"]
        .as_array()
        .expect("the JSON schema lists the kinds")
        .iter()
        .map(|value| {
            value
                .as_str()
                .and_then(|key| key.strip_prefix("type "))
                .expect("a type key is the word type and a kind")
                .to_string()
        })
        .collect();
    assert_eq!(said, kinds, "in the JSON schema");
    assert_eq!(
        json["$defs"]["simulatedPayload"]["required"],
        serde_json::json!(["payload"])
    );
    let group = &json["properties"]["simulated_payload_groups"]["items"];
    assert_eq!(group["required"], serde_json::json!(["group"]));
    for words in [&json["$defs"]["simulatedPayload"], group] {
        let words = words["properties"].as_object().expect("its words");
        assert!(!words.contains_key("type"), "an entry says no type");
        assert!(!words.contains_key("name"), "an entry has no name key");
    }

    let xsd = text_of("tcspecial-sim.xsd");
    assert_eq!(enumerated(&xsd, "kind"), kinds, "in the XSD");
    for word in tcslib_protocol::sim_config::SETTINGS
        .iter()
        .chain(&["group"])
    {
        assert!(
            xsd.contains(&format!("<xs:element name=\"{word}\"")),
            "the simulator XSD has no {word}"
        );
    }

    // A file that says everything, valid against the XSD and read by the
    // simulator's own reader.
    let dir = tempfile::tempdir().expect("a directory");
    let file = dir.path().join("every-sim.xml");
    std::fs::write(
        &file,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<simulator>
  <description>every word</description>
  <simulated_payload_groups>
    <group name=\"fast\">
      <packet_interval>100 ms</packet_interval>
    </group>
  </simulated_payload_groups>
  <simulated_payloads>
    <type name=\"i2c\">
      <payload name=\"temp\">
        <group>fast</group>
        <connects>zone.temperature</connects>
        <offset>+0.2 K</offset>
        <noise>0.03 K</noise>
        <drop_percent>1</drop_percent>
      </payload>
    </type>
    <type name=\"gpio_in\">
      <payload name=\"heater\">
        <connects>heater.on</connects>
      </payload>
    </type>
  </simulated_payloads>
</simulator>
",
    )
    .expect("written");
    // Read as far as a file is without its world: the payloads are joined
    // to terminals only a YAML simulator file's world can have.
    tcslib_protocol::sim_config::SimConfigFile::load(&file)
        .unwrap_or_else(|e| panic!("the simulator does not read it: {e}"));

    if Command::new("xmllint").arg("--version").output().is_err() {
        eprintln!(
            "xmllint is not installed: the simulator XSD was not checked"
        );
        return;
    }
    let valid = |text: &str| {
        std::fs::write(&file, text).expect("written");
        Command::new("xmllint")
            .arg("--noout")
            .arg("--schema")
            .arg(schema_of("tcspecial-sim.xsd"))
            .arg(&file)
            .output()
            .expect("xmllint runs")
    };
    let every = std::fs::read_to_string(&file).expect("read back");
    let output = valid(&every);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // And a kind that is not one is refused.
    let output = valid(
        &every.replace("<type name=\"gpio_in\">", "<type name=\"ping\">"),
    );
    assert!(!output.status.success(), "ping is not a kind of payload");
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
    for (attribute, _) in tcslib_protocol::graph::parameters::ATTRIBUTES {
        // Dotted in the rules, nested in the file: `beacon.send` is `send`
        // under `beacon`. One with no dot in it would be an attribute at the
        // top of the section, which the schema would hold as its own property.
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

    let words = |entry: &serde_json::Value| -> BTreeSet<String> {
        entry["properties"]
            .as_object()
            .expect("its words")
            .keys()
            .cloned()
            .collect()
    };
    for (section, said) in [
        (
            "simulated_payloads",
            words(&schema["$defs"]["simulatedPayload"]),
        ),
        (
            "simulated_payload_groups",
            words(&schema["properties"]["simulated_payload_groups"]["items"]),
        ),
    ] {
        for setting in tcslib_protocol::sim_config::SETTINGS {
            assert!(
                said.contains(*setting),
                "{section} has no {setting}: {said:?}"
            );
        }
        // A payload is named by `payload`, and takes its group's settings
        // for whatever it does not state; a group is named by `group`, which
        // is all `group` is in a group.
        let payload = section == "simulated_payloads";
        assert_eq!(said.contains("payload"), payload, "{section}: {said:?}");
        assert!(said.contains("group"), "{section}: {said:?}");
        let extra = if payload { 2 } else { 1 };
        assert_eq!(
            said.len(),
            tcslib_protocol::sim_config::SETTINGS.len() + extra,
            "{section}: {said:?}"
        );
    }

    // And no version, which no file states: the software reading a file is
    // what says which version of the language it reads.
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

    let mut files: Vec<(&str, String)> = vec![
        ("tcspecial.xsd", "example.xtcs".to_string()),
        ("tcspecial.xsd", "tests/manual/thermostat.xtcs".to_string()),
    ];
    files.extend(
        examples(".xtcs")
            .into_iter()
            .map(|file| ("tcspecial.xsd", file)),
    );
    for (schema_name, file) in &files {
        let output = Command::new("xmllint")
            .arg("--noout")
            .arg("--schema")
            .arg(schema_of(schema_name))
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

/// Whether a file is valid against one of the XSDs, by `xmllint`.
///
/// # Arguments
///
/// * `schema_name` - The XSD, by its file name: see [`schema_of`].
/// * `file`        - The file to check.
///
/// # Returns
///
/// `Ok` if it is valid.
///
/// # Errors
///
/// What `xmllint` said, where it is not.
///
/// # Panics
///
/// If `xmllint` cannot be run.
fn xsd_check(schema_name: &str, file: &Path) -> Result<(), String> {
    let output = Command::new("xmllint")
        .arg("--noout")
        .arg("--schema")
        .arg(schema_of(schema_name))
        .arg(file)
        .output()
        .expect("xmllint runs");
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

/// Copies one section of an XML file to the end of it, just inside the
/// root's closing tag, so that the file states it twice.
///
/// # Arguments
///
/// * `text`    - The file's text.
/// * `section` - The section's tag, which `text` holds once.
/// * `root`    - The root element's tag.
///
/// # Returns
///
/// The text with the section written twice.
///
/// # Panics
///
/// If `text` does not hold the section or the root's closing tag.
fn twice(text: &str, section: &str, root: &str) -> String {
    let start = text
        .find(&format!("<{section}>"))
        .unwrap_or_else(|| panic!("no <{section}>"));
    let close = format!("</{section}>");
    let end = start + text[start..].find(&close).expect("closed") + close.len();
    let at = text
        .rfind(&format!("</{root}>"))
        .expect("a closing root tag");
    format!("{}{}\n{}", &text[..at], &text[start..end], &text[at..])
}

/// A section written twice is refused by the XSD as it is by the reader.
///
/// A simulator file has no list among its sections, so each is held to
/// once by its XSD. A configuration's lists are repeated, which XSD 1.0
/// cannot mix with sections held to once, except for its description: a
/// value and not a section of child elements, which an identity constraint
/// can count. The other sections of a configuration are refused twice
/// where the file is read and nowhere else.
///
/// Skipped, loudly, where xmllint is not installed.
#[test]
fn a_section_written_twice_is_refused_by_the_xsd_and_the_reader() {
    if Command::new("xmllint").arg("--version").output().is_err() {
        eprintln!("xmllint is not installed: the XML schemas were not checked");
        return;
    }
    let dir = tempfile::tempdir().expect("a directory");

    let simulator = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<simulator>
  <description>once</description>
  <tcssim>
    <ready>127.0.0.1:7999</ready>
  </tcssim>
  <simulated_payload_groups>
    <group name=\"fast\">
      <packet_interval>100 ms</packet_interval>
    </group>
  </simulated_payload_groups>
  <simulated_payloads>
    <type name=\"spi\">
      <payload name=\"spi-1\">
        <group>fast</group>
      </payload>
    </type>
  </simulated_payloads>
</simulator>
";
    let file = dir.path().join("once-sim.xml");
    let check = |text: &str| {
        std::fs::write(&file, text).expect("written");
        let read = tcslib_protocol::sim_config::SimConfigFile::parse(
            text,
            tcslib_protocol::ConfigFormat::Xml,
        );
        (xsd_check("tcspecial-sim.xsd", &file), read.map(|_| ()))
    };
    let (valid, read) = check(simulator);
    assert_eq!(valid, Ok(()), "the simulator file is valid");
    assert!(read.is_ok(), "the simulator reads it: {read:?}");
    for section in [
        "description",
        "tcssim",
        "simulated_payload_groups",
        "simulated_payloads",
    ] {
        let (valid, read) = check(&twice(simulator, section, "simulator"));
        assert!(valid.is_err(), "the XSD takes <{section}> twice");
        assert!(read.is_err(), "the simulator reads <{section}> twice");
    }

    let configuration = std::fs::read_to_string(root().join("example.xtcs"))
        .expect("example.xtcs");
    let file = dir.path().join("twice.xtcs");
    std::fs::write(
        &file,
        twice(&configuration, "description", "configuration"),
    )
    .expect("written");
    assert!(
        xsd_check("tcspecial.xsd", &file).is_err(),
        "the XSD takes <description> twice"
    );
    assert!(
        tcslib_protocol::graph::Graph::load(&file).is_err(),
        "the reader takes <description> twice"
    );
}

/// Every XML file this repository ships is well formed XML.
///
/// Weaker than the validation above and over a wider set: a file with no
/// schema to be held against is still XML, and XML it is not is a file no
/// editor and no other tool can open whatever this program's own reader
/// tolerates. Four of these said
/// `--` inside a comment, which XML forbids, and nothing noticed because
/// `quick-xml` reads them anyway.
///
/// Skipped, loudly, where xmllint is not installed.
#[test]
fn every_shipped_xml_file_is_well_formed() {
    if Command::new("xmllint").arg("--version").output().is_err() {
        eprintln!("xmllint is not installed: the XML files were not checked");
        return;
    }

    let mut files: Vec<String> =
        ["example.xtcs", "tests/manual/thermostat.xtcs"]
            .map(str::to_string)
            .to_vec();
    files.extend(examples(".xtcs"));
    for file in &files {
        let path = root().join(file);
        assert!(path.is_file(), "{file} is not there any more");
        let output = Command::new("xmllint")
            .arg("--noout")
            .arg(&path)
            .output()
            .expect("xmllint runs");
        assert!(
            output.status.success(),
            "{file} is not well formed XML:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Every shipped configuration says which schema describes it.
///
/// The reference is what makes shipping the schemas worth anything: an editor
/// opening one of these validates it without being set up to, and a file that
/// has to be pointed at its schema by hand is a file nobody points at it. The
/// XML spelling says so in the two attributes every XML tool reads; the YAML
/// spelling says so in the comment every YAML language server reads, there
/// being no other way to say it in YAML.
///
/// The path is relative, which is how it stays true in a checkout: see
/// `tcsgui::nodes::schema_reference` for what is written where the schemas are
/// not above the file.
///
/// The worked example and not `example.*`: this pair is written by the editor
/// and compared byte for byte against what it writes, so a reference that
/// stopped being written would fail here. `example.*` is written by the same
/// writer from the same function and says the same thing.
///
/// Every simulator configuration as well, and those are every one this
/// repository ships: a payload set's file is the one somebody opens to change
/// how a simulated payload behaves, which is exactly the editing the schema
/// is installed for. They name the JSON Schema and no XSD, the editor writing
/// that file in YAML and nothing else.
#[test]
fn every_shipped_configuration_says_which_schema_describes_it() {
    let mut shipped: Vec<(String, &str)> = [
        (
            "tests/manual/thermostat.xtcs",
            "../../tcslib-protocol/schema/tcspecial.xsd",
        ),
        (
            "tests/manual/thermostat.ytcs",
            "../../tcslib-protocol/schema/tcspecial.schema.json",
        ),
        (
            "example-sim.yaml",
            "tcslib-protocol/schema/tcspecial-sim.schema.json",
        ),
    ]
    .map(|(file, said)| (file.to_string(), said))
    .to_vec();
    // Each example is two directories below the root.
    for file in examples(".xtcs") {
        shipped.push((file, "../../tcslib-protocol/schema/tcspecial.xsd"));
    }
    for file in example_configurations() {
        shipped
            .push((file, "../../tcslib-protocol/schema/tcspecial.schema.json"));
    }
    for file in examples("-sim.yaml") {
        shipped.push((
            file,
            "../../tcslib-protocol/schema/tcspecial-sim.schema.json",
        ));
    }
    for (file, said) in &shipped {
        let (file, said) = (file.as_str(), *said);
        let path = root().join(file);
        let text =
            std::fs::read_to_string(&path).expect("a shipped configuration");
        assert!(
            text.contains(said),
            "{file} does not say that {said} describes it"
        );

        // And the schema it names is where it says it is, which is the half
        // of a reference that rots.
        let named = path.parent().expect("a directory").join(said);
        assert!(named.is_file(), "{file} names {said}, which is not there");
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

    let mut files = vec![
        "example.ytcs".to_string(),
        "tests/manual/thermostat.ytcs".to_string(),
    ];
    files.extend(example_configurations());
    for file in &files {
        let file = file.as_str();
        let path = root().join(file);
        let text =
            std::fs::read_to_string(&path).expect("a shipped configuration");
        let said: serde_norway::Value = serde_norway::from_str(&text)
            .unwrap_or_else(|e| panic!("{file}: {e}"));

        let sections = properties(&schema, &[]);
        let mapping = said.as_mapping().expect("a configuration is a mapping");
        for (key, value) in mapping {
            let key = key.as_str().expect("a section is named").to_string();
            assert!(
                sections.contains(&key),
                "{file}: {key} is not in the schema"
            );

            // The node sections, whose entries are what a schema has most to
            // say about: each an entry of one key, the word type and the
            // type, holding the nodes of that type.
            let Some(section) = tcslib_protocol::graph::Section::ALL
                .into_iter()
                .find(|section| section.name() == key)
            else {
                continue;
            };
            let nodes = schema["properties"][key.as_str()]["items"]
                ["additionalProperties"]["items"]["$ref"]
                .as_str()
                .and_then(|r| r.strip_prefix("#/$defs/"))
                .expect("a node definition");
            let allowed: BTreeSet<String> = schema["$defs"][nodes]
                ["properties"]
                .as_object()
                .expect("a node's words")
                .keys()
                .cloned()
                .collect();
            for entry in value.as_sequence().expect("a list of types") {
                let entry = entry.as_mapping().expect("an entry is a mapping");
                for (_, typed) in entry {
                    for node in typed.as_sequence().expect("a list of nodes") {
                        let node =
                            node.as_mapping().expect("a node is a mapping");
                        assert!(
                            node.contains_key(section.node_word()),
                            "{file}: a node of {key} is not named by {}",
                            section.node_word()
                        );
                        for (attribute, _) in node {
                            let attribute =
                                attribute.as_str().expect("named").to_string();
                            assert!(
                                allowed.contains(&attribute),
                                "{file}: {key} has a node stating \
                                 {attribute}, which the schema does not allow"
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Every key of every shipped simulator file is one its schema allows, in
/// the shape it allows: the simulated payloads and the world's components
/// grouped by type, each named by its word, and the groups named by
/// `group`. The same shallow check as the configurations', for the same
/// reason; [`every_shipped_simulator_file_is_valid_against_its_schema`]
/// is the whole one.
#[test]
fn every_key_of_a_shipped_simulator_file_is_in_its_schema() {
    let sim = read("tcspecial-sim.schema.json");
    let words = |entry: &serde_json::Value| -> BTreeSet<String> {
        entry["properties"]
            .as_object()
            .expect("its words")
            .keys()
            .cloned()
            .collect()
    };
    let payload = words(&sim["$defs"]["simulatedPayload"]);
    let group = words(&sim["properties"]["simulated_payload_groups"]["items"]);
    let world = &sim["properties"]["world"];

    /// Checks one entry: named by its word, saying only what is allowed.
    ///
    /// # Arguments
    ///
    /// * `file`    - The file, for a message.
    /// * `entry`   - The entry.
    /// * `word`    - The word that names it.
    /// * `allowed` - What it may say.
    fn check(
        file: &str,
        entry: &serde_norway::Value,
        word: &str,
        allowed: &BTreeSet<String>,
    ) {
        let entry = entry.as_mapping().expect("an entry is a mapping");
        assert!(entry.contains_key(word), "{file}: not named by {word}");
        for (key, _) in entry {
            let key = key.as_str().expect("named");
            assert!(allowed.contains(key), "{file}: {key} is not allowed");
        }
    }

    /// Checks a list of entries grouped by type: each one key, `type` and
    /// a type, holding entries each named by `word`.
    ///
    /// # Arguments
    ///
    /// * `file`    - The file, for a message.
    /// * `list`    - The list.
    /// * `word`    - The word that names an entry.
    /// * `allowed` - What an entry of a type may say, by the type, or
    ///               `None` for a type that is not allowed.
    fn grouped(
        file: &str,
        list: &serde_norway::Value,
        word: &str,
        allowed: &dyn Fn(&str) -> Option<BTreeSet<String>>,
    ) {
        for entry in list.as_sequence().expect("a list of types") {
            let entry = entry.as_mapping().expect("a mapping");
            assert_eq!(entry.len(), 1, "{file}: one type an entry");
            for (kind, entries) in entry {
                let kind = kind.as_str().expect("a type key");
                let named =
                    kind.strip_prefix("type ").expect("type and the type");
                let allowed = allowed(named)
                    .unwrap_or_else(|| panic!("{file}: {kind} is not allowed"));
                for one in entries.as_sequence().expect("a list") {
                    check(file, one, word, &allowed);
                }
            }
        }
    }

    let kinds = &sim["$defs"]["simulatedType"]["enum"];
    let is_kind = |named: &str| {
        kinds
            .as_array()
            .expect("kinds")
            .iter()
            .any(|k| k.as_str() == Some(&format!("type {named}")))
    };
    let mut files = examples("-sim.yaml");
    files.push("example-sim.yaml".to_string());
    files.push("tests/manual/thermostat-sim.yaml".to_string());
    for file in &files {
        let text = std::fs::read_to_string(root().join(file))
            .unwrap_or_else(|e| panic!("{file}: {e}"));
        let said: serde_norway::Value = serde_norway::from_str(&text)
            .unwrap_or_else(|e| panic!("{file}: {e}"));
        let sections = properties(&sim, &[]);
        for (key, value) in said.as_mapping().expect("a mapping") {
            let key = key.as_str().expect("a section is named");
            assert!(sections.contains(key), "{file}: {key} is not allowed");
            match key {
                "simulated_payload_groups" => {
                    for entry in value.as_sequence().expect("a list") {
                        check(file, entry, "group", &group);
                    }
                }
                "simulated_payloads" => {
                    grouped(file, value, "payload", &|named| {
                        is_kind(named).then(|| payload.clone())
                    });
                }
                "world" => {
                    let value = value.as_mapping().expect("a mapping");
                    for (key, said) in value {
                        let key = key.as_str().expect("named");
                        assert!(
                            world["properties"].get(key).is_some(),
                            "{file}: world: {key} is not allowed"
                        );
                        if key != "physics" {
                            continue;
                        }
                        let physics = &world["properties"]["physics"];
                        for (key, said) in said.as_mapping().expect("a mapping")
                        {
                            let key = key.as_str().expect("named");
                            assert!(
                                physics["properties"].get(key).is_some(),
                                "{file}: world: physics: {key} is not allowed"
                            );
                            if key == "components" {
                                grouped(file, said, "component", &|named| {
                                    sim["$defs"].get(named).map(words)
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Every shipped simulator file is valid against the simulator schema, as
/// an editor's language server would hold it, by a JSON Schema validator:
/// every word, every type and every shape. And the shapes the physics model
/// had before -- a file of its own named by `physics: <path>`, or a
/// `physics` section at the top of the file -- are not.
#[test]
fn every_shipped_simulator_file_is_valid_against_its_schema() {
    let validator =
        jsonschema::validator_for(&read("tcspecial-sim.schema.json"))
            .expect("the simulator schema is a schema");
    /// A YAML text as the JSON a validator takes.
    ///
    /// # Arguments
    ///
    /// * `text` - The YAML.
    ///
    /// # Returns
    ///
    /// It, as JSON.
    fn json_of(text: &str) -> serde_json::Value {
        serde_norway::from_str(text).expect("YAML")
    }
    let mut files = examples("-sim.yaml");
    files.push("example-sim.yaml".to_string());
    files.push("tests/manual/thermostat-sim.yaml".to_string());
    files.push("examples/thermostat-common/thermostat-sim.yaml".to_string());
    for file in &files {
        let text = std::fs::read_to_string(root().join(file))
            .unwrap_or_else(|e| panic!("{file}: {e}"));
        let errors: Vec<String> = validator
            .iter_errors(&json_of(&text))
            .map(|e| format!("{}: {e}", e.instance_path()))
            .collect();
        assert!(errors.is_empty(), "{file}: {errors:#?}");
    }

    let world = "world:\n  ambient_temp: \"288.15 K\"\n  physics:\n    \
                 components:\n      - type heater:\n          \
                 - component: h\n            power: \"4 W\"\n            \
                 voltage: \"28 V\"\n      - type thermal_mass:\n          \
                 - component: zone\n            capacity: \"20 J/K\"\n            \
                 time_constant: \"50 s\"\n    connections:\n      \
                 - { from: h.heat, to: zone.heat }\n";
    assert!(validator.is_valid(&json_of(world)), "{world}");
    for refused in [
        "physics: ../thermostat-common/thermostat-physics.yaml\n".to_string(),
        world
            .replace("world:\n  ambient_temp: \"288.15 K\"\n  ", "")
            .replace("\n  ", "\n"),
        world.replace(
            "time_constant",
            "environment: \"283.15 K\"\n            time_constant",
        ),
        world.replace("type heater", "type boiler"),
        world.replace("from: h.heat", "via: h.heat"),
    ] {
        assert!(
            !validator.is_valid(&json_of(&refused)),
            "the schema takes:\n{refused}"
        );
    }
}

/// Where the descriptions this repository ships are.
fn nodes() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("share/nodes")
}

/// Every description in the tree, by path relative to its root.
///
/// # Returns
///
/// The YAML and XML files under `share/nodes`, sorted, relative to it.
///
/// # Panics
///
/// If the tree cannot be read or holds no descriptions.
fn described() -> Vec<PathBuf> {
    /// Adds every YAML and XML file under a directory, in sorted order.
    ///
    /// # Arguments
    ///
    /// * `root`  - The top of the tree, which the paths are made relative to.
    /// * `here`  - The directory to search, recursively.
    /// * `found` - Where each file's path is added.
    ///
    /// # Panics
    ///
    /// If a directory or one of its entries cannot be read.
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
                found.push(
                    path.strip_prefix(root)
                        .expect("under the root")
                        .to_path_buf(),
                );
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
    let have_xmllint =
        Command::new("xmllint").arg("--version").output().is_ok();
    if !have_xmllint {
        eprintln!(
            "xmllint is not installed: the XML descriptions were not validated"
        );
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
                .arg(schema_of("node.xsd"))
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
            for attribute in value.as_sequence().expect("a list of attributes")
            {
                for (word, _) in
                    attribute.as_mapping().expect("an attribute is a mapping")
                {
                    let word = word.as_str().expect("a named word").to_string();
                    assert!(
                        allowed_attribute.contains(&word),
                        "{}: an attribute says {word}, which a description \
                         does not know",
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
    let xsd = std::fs::read_to_string(schema_of("node.xsd")).expect("the XSD");

    // What the XSD allows inside <node>, read from its element declarations.
    let declared: BTreeSet<String> = xsd
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("<xs:element name=\"")?;
            let name = rest.split('"').next()?;
            // The document element is the file itself, not a word inside it.
            (line.contains("type=") && !line.contains("ref="))
                .then(|| name.to_string())
        })
        .collect();
    // `node` is both the document element and the word inside it that names
    // the type, so it is in the set either way. An attribute's own words are
    // not words inside `<node>`: they are compared below.
    let inside: BTreeSet<String> = declared
        .iter()
        .filter(|name| !matches!(name.as_str(), "name" | "required" | "kind"))
        .cloned()
        .collect();

    let allowed = properties(&json, &[]);
    assert_eq!(
        inside, allowed,
        "the two node schemas allow different words"
    );

    // And an attribute's own words agree as well.
    let attribute: BTreeSet<String> = ["name", "required", "kind", "about"]
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

        let text = std::fs::read_to_string(nodes().join(&name))
            .expect("a description");
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

/// The simulator schema's world offers every component type the model
/// reads, each an entry `type <type>` holding that type's components, and
/// for each type every setting the one list of its settings has, beside the
/// `component` naming it: the schema an editor checks a hand-written model
/// against says what tcssim reads.
#[test]
fn the_simulator_schema_takes_the_types_and_settings_the_model_reads() {
    let json = read("tcspecial-sim.schema.json");
    let world = &json["properties"]["world"]["properties"];
    assert!(
        world["ambient_temp"].is_object(),
        "the world's ambient_temp"
    );
    let item = &world["physics"]["properties"]["components"]["items"];
    assert_eq!(item["maxProperties"], 1, "one type an entry");
    let entries = item["properties"].as_object().expect("one entry a type");
    let types: BTreeSet<&str> = entries
        .keys()
        .map(|key| key.strip_prefix("type ").expect("the word type and a type"))
        .collect();
    let wanted: BTreeSet<&str> = tcslib_protocol::node::physics::TYPES
        .iter()
        .copied()
        .collect();
    assert_eq!(types, wanted);
    for name in types {
        let component = &json["$defs"][name];
        assert_eq!(
            entries[&format!("type {name}")]["items"]["$ref"],
            format!("#/$defs/{name}"),
            "{name}"
        );
        let mut said: Vec<&str> = component["properties"]
            .as_object()
            .expect("its settings")
            .keys()
            .map(String::as_str)
            .filter(|key| *key != "component")
            .collect();
        said.sort_unstable();
        let mut wanted = tcslib_protocol::node::physics::settings_of(name)
            .expect("a type the model reads")
            .to_vec();
        wanted.sort_unstable();
        assert_eq!(said, wanted, "{name}");
        assert_eq!(component["required"][0], "component", "{name}");
    }
}
