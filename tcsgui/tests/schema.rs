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

/// The examples, as paths from the root: every file whose name ends `ending`
/// in a directory of `examples/`, each example having one of its own.
/// Discovered, so that an example added there is held against the schemas
/// without anyone naming it here.
fn examples(ending: &str) -> Vec<String> {
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

/// The examples' tcspecial configurations in YAML, which is every YAML file
/// there but the simulator's.
fn example_configurations() -> Vec<String> {
    examples(".yaml")
        .into_iter()
        .filter(|name| !name.ends_with("-sim.yaml"))
        .collect()
}

/// One schema, read.
fn read(name: &str) -> serde_json::Value {
    let path = schema().join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
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
    assert_eq!(sections.len(), 9, "{sections:?}");
    // And no version: a configuration states none, and the reader refuses
    // one, so a schema offering it would be offering a refusal.
    assert!(!sections.contains("version"), "{sections:?}");

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

/// The values an XSD's simple type enumerates, in the order it lists them.
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
    let path = schema().join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Each section of a configuration takes the node types the software knows
/// for it, in both schemas, and no others.
///
/// Held against `tcslibgs::graph::types_in`, which is what the checker
/// refuses an unknown type by: a type the schemas offer and the checker
/// refuses, or the other way round, is a schema arguing with the software.
#[test]
fn each_section_takes_the_node_types_the_software_knows() {
    use tcslibgs::graph::{types_in, Section};

    let json = read("tcspecial.schema.json");
    let xsd = text_of("tcspecial.xsd");
    for (section, key, simple_type) in [
        (Section::Moc, "moc", "mocType"),
        (Section::Vehicle, "vehicle", "vehicleType"),
        (Section::Payloads, "payloads", "payloadType"),
    ] {
        let wanted: Vec<String> =
            types_in(section).into_iter().map(str::to_string).collect();

        let said: Vec<String> = json["properties"][key]["items"]["properties"]
            ["type"]["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("{key}: the JSON schema lists no types"))
            .iter()
            .map(|value| value.as_str().expect("a type is a word").to_string())
            .collect();
        assert_eq!(said, wanted, "{key}, in the JSON schema");

        assert_eq!(enumerated(&xsd, simple_type), wanted, "{key}, in the XSD");
    }
}

/// A simulated payload stands in for one of the interfaces, in both
/// schemas, and an XML simulator file with every word in it is valid and
/// reads.
///
/// The kinds are `DHType`'s, which is what the simulator reads a `type`
/// with. Empty is allowed, as not stated. And the XSD offers every setting
/// the one list of them has, which the XSD once did not: it was missing the
/// thermal masses and what attaches a payload to one.
#[test]
fn the_simulator_schemas_take_the_kinds_and_words_the_simulator_reads() {
    let mut kinds: Vec<String> = tcslibgs::DHType::ALL
        .iter()
        .map(|kind| kind.spelling().to_string())
        .collect();
    kinds.push(String::new());

    let json = read("tcspecial-sim.schema.json");
    for section in ["simulated_payloads", "simulated_payload_groups"] {
        let enumerated = json["properties"][section]["items"]["properties"]
            ["type"]["enum"]
            .as_array()
            .unwrap_or_else(|| {
                panic!("{section}: the JSON schema lists no kinds")
            });
        let said: Vec<String> = enumerated
            .iter()
            .filter_map(|value| value.as_str().map(str::to_string))
            .collect();
        assert_eq!(said, kinds, "{section}, in the JSON schema");
        assert!(enumerated.contains(&serde_json::Value::Null), "{section}");
    }

    let xsd = text_of("tcspecial-sim.xsd");
    assert_eq!(enumerated(&xsd, "kind"), kinds, "in the XSD");
    for word in tcslibgs::sim_config::SETTINGS
        .iter()
        .chain(&["thermal_mass", "group"])
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
  <thermal_mass>
    <name>cabin</name>
    <environment>280</environment>
    <time_constant>60 s</time_constant>
  </thermal_mass>
  <simulated_payload_groups>
    <name>fast</name>
    <type>network</type>
    <protocol>udp</protocol>
    <packet_interval>100 ms</packet_interval>
  </simulated_payload_groups>
  <simulated_payloads>
    <name>temp</name>
    <group>fast</group>
    <type>i2c</type>
    <reads>cabin</reads>
    <drop_percent>1</drop_percent>
  </simulated_payloads>
  <simulated_payloads>
    <name>heater</name>
    <type>gpio</type>
    <heats>cabin</heats>
    <rate>0.5</rate>
  </simulated_payloads>
</simulator>
",
    )
    .expect("written");
    tcslibgs::sim_config::SimConfigFile::load(&file)
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
            .arg(schema().join("tcspecial-sim.xsd"))
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
    let output =
        valid(&every.replace("<type>gpio</type>", "<type>ping</type>"));
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
    for (attribute, _) in tcslibgs::graph::parameters::ATTRIBUTES {
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
        ("tcspecial.xsd", "example.xml".to_string()),
        ("tcspecial.xsd", "tests/manual/thermostat.xml".to_string()),
    ];
    files.extend(
        examples(".xml")
            .into_iter()
            .map(|file| ("tcspecial.xsd", file)),
    );
    for (schema_name, file) in &files {
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

    let mut files: Vec<String> = ["example.xml", "tests/manual/thermostat.xml"]
        .map(str::to_string)
        .to_vec();
    files.extend(examples(".xml"));
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
            "tests/manual/thermostat.xml",
            "../../tcsgui/share/schema/tcspecial.xsd",
        ),
        (
            "tests/manual/thermostat.yaml",
            "../../tcsgui/share/schema/tcspecial.schema.json",
        ),
        (
            "example-sim.yaml",
            "tcsgui/share/schema/tcspecial-sim.schema.json",
        ),
    ]
    .map(|(file, said)| (file.to_string(), said))
    .to_vec();
    // Each example is two directories below the root.
    for file in examples(".xml") {
        shipped.push((file, "../../tcsgui/share/schema/tcspecial.xsd"));
    }
    for file in example_configurations() {
        shipped.push((file, "../../tcsgui/share/schema/tcspecial.schema.json"));
    }
    for file in examples("-sim.yaml") {
        shipped.push((
            file,
            "../../tcsgui/share/schema/tcspecial-sim.schema.json",
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
        "example.yaml".to_string(),
        "tests/manual/thermostat.yaml".to_string(),
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
            // say about.
            if !["moc", "vehicle", "payloads"].contains(&key.as_str()) {
                continue;
            }
            let allowed = properties(&schema, &[&key]);
            for node in value.as_sequence().expect("a list of nodes") {
                let node = node.as_mapping().expect("a node is a mapping");
                for (attribute, _) in node {
                    let attribute =
                        attribute.as_str().expect("named").to_string();
                    assert!(
                        allowed.contains(&attribute),
                        "{file}: {key} has a node stating {attribute}, which \
                         the schema does not allow"
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
    let xsd =
        std::fs::read_to_string(schema().join("node.xsd")).expect("the XSD");

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
