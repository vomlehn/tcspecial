//! The description tree: what vehicle nodes there are, as files rather than
//! as code in the editor.
//!
//! A vehicle node is a thing the vehicle itself runs -- a computation, a log, a
//! pass-through -- and a new one arrives as a Rust file in tcspecial and a
//! description here. Two trees with one structure: a description at
//! `compute/mean.yaml` has its Rust at
//! `tcspecial/src/node/vehicle/compute/mean.rs`, and one at
//! `serial/keypad.yaml` would have `node/vehicle/serial/keypad.rs` or a
//! directory `node/vehicle/serial/keypad/` with files under it.
//!
//! Every node type has one, and the tree's families are the code's: a
//! description at `vehicle/compute/mean.yaml` is about the Rust at
//! `tcspecial/src/node/vehicle/compute/mean.rs`, and one at
//! `payload/i2c.yaml` about `node/payload/i2c/`. `moc.yaml` is at the root
//! because the ground station has no Rust of its own -- it is the other end of
//! the link rather than part of the vehicle, and the code that serves one is
//! the network payload's.
//!
//! Two trees rather than one, which was a decision and is worth the reason.
//! The descriptions are data the editor reads at run time and will be able to
//! edit; the Rust is compiled into the flight software. Keeping them apart
//! means the thing that is installed on a workstation is not the thing that is
//! built for the vehicle -- and a `.yaml` beside a `.rs` in `src/` is a file
//! `cargo` neither builds nor ships, sitting where everything else is both.
//! What the structure buys is that the correspondence is checkable, and
//! `tests::every_description_has_its_rust` is where it is checked.
//!
//! Either format, file by file: a description is YAML or XML as its extension
//! says, and nothing cares which a given node used. One of the shipped ones is
//! XML for exactly that reason.
//!
//! # What a description says
//!
//! ```yaml
//! node: mean                 # what the palette shows, and the name a placed
//!                            # node starts with
//! type: compute              # the configuration's own word for the type
//! about: The mean of the readings arriving at its named inputs.
//! sets:                      # attributes the editor fills in and does not ask
//!   function: mean
//! attributes:                # what a node of this kind says for itself
//!   - name: inputs
//!     required: true
//!     kind: text             # what it takes: see KINDS. Unstated is text
//!     about: Which inputs it is over, comma separated.
//! ```
//!
//! `sets` is what makes a type with variants into several entries in the
//! palette: `compute` is one node type and `mean`, `hysteresis`, `to_kelvin`
//! and `process` are four things to place, each of which is a compute node with
//! its `function` already answered.
//!
//! `kind` is what the node's window offers a field with: a `boolean` takes a
//! control with two states, and everything else a box to write in. See
//! [`KINDS`] for the words and for where the rules behind them live.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use tcslibgs::Scalar;

/// One vehicle node, as its description file says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Described {
    /// What the palette shows, and the name a placed node starts with.
    pub node: String,
    /// The configuration's word for the node type.
    pub node_type: String,
    /// One line about what it does, for the palette and the node window.
    pub about: String,
    /// Attributes the editor fills in rather than asking for: the `function`
    /// of a computation, and whatever else picks one variant of a type.
    pub sets: BTreeMap<String, String>,
    /// What a node of this kind says for itself: the name, whether it must be
    /// stated, and a line about it.
    pub attributes: Vec<Attribute>,
    /// Which of the four terminals a node of this type has.
    ///
    /// A property of the type and not of a node, which is why it is here: a
    /// configuration says what the vehicle carries and what each node is told,
    /// and "an i2c node receives what asks it for a reading" is true of every
    /// i2c node there will ever be. It was written in Rust, in
    /// `tcslibgs::graph::terminals`, where nobody editing a node type could
    /// see it; `tests::the_tree_agrees_with_the_schemas_terminals` keeps the
    /// two in step.
    pub terminals: Terminals,
    /// Where the description was read from, relative to the tree's root.
    ///
    /// Kept because it is the other half of the correspondence: the Rust for
    /// this node is at the same path under `tcspecial/src/node`, and a test
    /// says so.
    pub path: PathBuf,
}

/// Which terminals a node type has, as its description declares them.
///
/// Four flags rather than two lists of names, because that is what a file can
/// state plainly and what a reader of one can check at a glance: a type either
/// has somewhere for a stream to arrive or it has not. The names the four
/// answer to are `tcslibgs::graph::terminals`'s -- `payload-sink` is the
/// terminal called `payload-in` -- and [`Terminals::of`] is the one place that
/// correspondence is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Terminals {
    /// Where the payload's own data arrives: the terminal `payload-in`.
    pub payload_sink: bool,
    /// And where it leaves: `payload-out`.
    pub payload_source: bool,
    /// Where what the ground sends arrives: `moc-in`.
    pub moc_sink: bool,
    /// And where it leaves: `moc-out`.
    pub moc_source: bool,
}

impl Terminals {
    /// The four, by the terminal name each answers to.
    ///
    /// The one statement of which flag is which terminal, so that a test can
    /// hold the tree against the schema's own table without either side
    /// repeating the other. Which is its only caller so far: the editor still
    /// takes its terminals from `tcslibgs::graph::terminals`, and the tree is
    /// held against that -- see
    /// `tests::the_trees_terminals_are_the_schemas`.
    #[cfg(test)]
    pub fn each(&self) -> [(&'static str, bool); 4] {
        [
            ("payload-in", self.payload_sink),
            ("payload-out", self.payload_source),
            ("moc-in", self.moc_sink),
            ("moc-out", self.moc_source),
        ]
    }
}

/// One attribute of a described node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub required: bool,
    pub about: String,
    /// What kind of value it takes, as the description says.
    ///
    /// What the window offers to type it with: a true or false takes a
    /// control with two states and not a box to write the word in. The kinds
    /// are [`KINDS`] and nothing else, and a description that says nothing
    /// means [`TEXT`] -- which is what every description written before this
    /// existed means, and what an attribute whose rule is its own business
    /// means still.
    pub kind: String,
}

/// The kind an attribute has where its description says nothing.
///
/// A box to write in, which is what every field was before the kinds existed:
/// an attribute nobody has classified is one the window asks for in the way
/// that can say anything.
pub const TEXT: &str = "text";

/// What an attribute may say it takes.
///
/// One word each, and the list is short on purpose: a kind earns its place by
/// changing what the window offers or what it checks, not by naming a rule.
/// The rules themselves are `tcslibgs`' -- `whole::whole_u32`,
/// `graph::parse_interval`, `trigger::trigger_bytes`,
/// `node::vehicle::Restart::of` -- and this says which of them a field is
/// typed into, so the window can offer the right thing and say what is wanted
/// before somebody types the wrong one.
///
/// * `text` -- anything, which is the default: a name, a word a rule of its
///   own reads, a path on the far side of a link.
/// * `path` -- a file, a device, a program: text, and known apart from it
///   because a window will one day offer to go and find one.
/// * `whole` -- a whole number, decimal or `0x`, as `whole::whole_u32` reads
///   one.
/// * `interval` -- a length of time with its unit, `500ms` or `3s`, as
///   `graph::parse_interval` reads one.
/// * `bytes` -- hex bytes, as a trigger is written.
/// * `boolean` -- `true` or `false`, and nothing else.
pub const KINDS: &[&str] = &[TEXT, "path", "whole", "interval", "bytes", "boolean"];

/// A description as a file writes it.
///
/// Apart from [`Described`] because the two have different jobs: this is the
/// file's shape, with everything optional that a file may leave out, and that
/// is what the editor works in.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Said {
    node: Scalar,
    #[serde(rename = "type")]
    node_type: Scalar,
    #[serde(default)]
    about: Option<Scalar>,
    #[serde(default)]
    sets: BTreeMap<String, Scalar>,
    /// Repeated in XML and a list in YAML, which serde reads as the same Vec.
    #[serde(default)]
    attributes: Vec<SaidAttribute>,
    /// The four terminal flags, each absent where a type has not got that one.
    ///
    /// Hyphenated: the words a file holds are the words a person writes, and
    /// these read as English either way about.
    #[serde(default, rename = "payload-sink")]
    payload_sink: Option<Scalar>,
    #[serde(default, rename = "payload-source")]
    payload_source: Option<Scalar>,
    #[serde(default, rename = "moc-sink")]
    moc_sink: Option<Scalar>,
    #[serde(default, rename = "moc-source")]
    moc_source: Option<Scalar>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SaidAttribute {
    name: Scalar,
    #[serde(default)]
    required: Option<Scalar>,
    #[serde(default)]
    kind: Option<Scalar>,
    #[serde(default)]
    about: Option<Scalar>,
}

/// Every description in the tree at `root`, and everything wrong with it.
///
/// Both, and neither instead of the other. A tree with one unreadable file in
/// it still describes every other node, and an editor that offered nothing
/// because one file has a typo in it would be an editor a typo turns off; but a
/// file that was meant to describe a node and does not is worth saying out
/// loud, because the node it describes has silently stopped being offered.
///
/// In the order a reader meets them: sorted by path, which puts a directory's
/// descriptions together and keeps the palette's order stable from one run to
/// the next.
pub fn read_tree(root: &Path) -> (Vec<Described>, Vec<String>) {
    let mut described = Vec::new();
    let mut trouble = Vec::new();

    let mut found = Vec::new();
    walk(root, root, &mut found, &mut trouble);
    found.sort();

    for path in found {
        match read_one(root, &path) {
            Ok(one) => described.push(one),
            Err(why) => trouble.push(why),
        }
    }

    // A name picks out one node, here as in a configuration: two descriptions
    // of one name would be two palette entries nothing could tell apart, and
    // the second would place the first's attributes.
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for one in &described {
        *seen.entry(one.node.as_str()).or_insert(0) += 1;
    }
    for (name, count) in seen {
        if count > 1 {
            trouble.push(format!(
                "{count} descriptions are of a node called \"{name}\", and a name \
                 picks out one node"
            ));
        }
    }

    (described, trouble)
}

/// Every description file under `where_`, as paths relative to `root`.
fn walk(root: &Path, where_: &Path, found: &mut Vec<PathBuf>, trouble: &mut Vec<String>) {
    let entries = match std::fs::read_dir(where_) {
        Ok(entries) => entries,
        Err(e) => {
            trouble.push(format!("{}: {e}", where_.display()));
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                trouble.push(format!("{}: {e}", where_.display()));
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, found, trouble);
            continue;
        }
        // Only the two formats, and silently: a tree people edit has a README
        // in it and an editor's leftovers beside it, and a complaint about
        // every one of those would bury the complaint that matters.
        if tcslibgs::ConfigFormat::from_path(&path).is_none() {
            continue;
        }
        match path.strip_prefix(root) {
            Ok(relative) => found.push(relative.to_path_buf()),
            Err(_) => trouble.push(format!("{}: not under the tree", path.display())),
        }
    }
}

/// One description, read from `root`-relative `path`.
fn read_one(root: &Path, path: &Path) -> Result<Described, String> {
    let whole = root.join(path);
    let said: Said = tcslibgs::load_config_file(&whole)
        .map_err(|e| format!("{}: {e}", whole.display()))?;

    let named = |what: &str| format!("{}: {what}", whole.display());
    let node = said.node.as_str().trim().to_string();
    if node.is_empty() {
        return Err(named("a description says which node it is about, and this says nothing"));
    }
    let node_type = said.node_type.as_str().trim().to_string();
    if node_type.is_empty() {
        return Err(named(&format!(
            "\"{node}\" says no type: a description names the configuration's own \
             word for what it is"
        )));
    }

    let mut attributes = Vec::new();
    for attribute in &said.attributes {
        let name = attribute.name.as_str().trim().to_string();
        if name.is_empty() {
            return Err(named(&format!("\"{node}\" has an attribute with no name")));
        }
        let required = match attribute.required.as_ref().map(|r| r.as_str().trim().to_string()) {
            None => false,
            Some(said) => match said.as_str() {
                "true" => true,
                "false" => false,
                other => {
                    return Err(named(&format!(
                        "\"{node}\", attribute \"{name}\": required is true or false, \
                         and this says \"{other}\""
                    )))
                }
            },
        };
        // What it takes, and one of the words there are for it: a kind
        // nothing knows would be a field offered in whatever way the window
        // falls back to, which is the kind of mistake a description is read
        // by a program to catch.
        let said_kind = attribute.kind.as_ref().map(|k| k.as_str().trim().to_string());
        let kind = match said_kind {
            None => TEXT.to_string(),
            Some(said) if KINDS.contains(&said.as_str()) => said,
            Some(other) => {
                return Err(named(&format!(
                    "\"{node}\", attribute \"{name}\": there is no kind called \
                     \"{other}\". They are {}",
                    KINDS.join(", ")
                )))
            }
        };
        attributes.push(Attribute {
            name,
            required,
            kind,
            about: said_about(attribute.about.as_ref()),
        });
    }

    // The four terminal flags, each true, false or unsaid. Unsaid is false:
    // a type that does not mention a terminal has not got one, which is what
    // every description written before these existed means.
    let flag = |what: &str, said: Option<&Scalar>| -> Result<bool, String> {
        match said.map(|s| s.as_str().trim().to_string()) {
            None => Ok(false),
            Some(said) => match said.as_str() {
                "true" => Ok(true),
                "false" => Ok(false),
                other => Err(named(&format!(
                    "\"{node}\": {what} is true or false, and this says \"{other}\""
                ))),
            },
        }
    };
    let terminals = Terminals {
        payload_sink: flag("payload-sink", said.payload_sink.as_ref())?,
        payload_source: flag("payload-source", said.payload_source.as_ref())?,
        moc_sink: flag("moc-sink", said.moc_sink.as_ref())?,
        moc_source: flag("moc-source", said.moc_source.as_ref())?,
    };

    Ok(Described {
        node,
        node_type,
        about: said_about(said.about.as_ref()),
        sets: said
            .sets
            .iter()
            .map(|(name, value)| (name.clone(), value.as_str().trim().to_string()))
            .collect(),
        attributes,
        terminals,
        path: path.to_path_buf(),
    })
}

/// A line of prose as the file wrote it, with its line breaks folded away.
///
/// A description is written for people and wraps where it suits the file; what
/// shows it is one line of a palette or one row of a panel.
fn said_about(said: Option<&Scalar>) -> String {
    said.map(|about| {
        about
            .as_str()
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
    })
    .unwrap_or_default()
}

impl Described {
    /// What a node of this kind is asked for, in the order the panel asks.
    ///
    /// What the description answers for it first -- a computation's function,
    /// which is what makes this node a mean rather than a thermostat -- and
    /// then what it says for itself. Both are required: an attribute the
    /// description fills in is one the node cannot do without, that being why
    /// it is filled in.
    ///
    /// Shown rather than hidden, for the reason an unrecognised attribute is
    /// shown: a value that is in the file and not on the panel is a value
    /// somebody has to open the file to find.
    #[cfg(test)]
    pub fn asked(&self) -> Vec<(String, bool)> {
        self.fields()
            .into_iter()
            .map(|(name, required, _)| (name, required))
            .collect()
    }

    /// The same, with what each one takes.
    ///
    /// For the window, which offers a control per kind: see [`KINDS`]. An
    /// attribute the description sets rather than asks for is text, like any
    /// other it says nothing about -- it is not offered at all, the editor
    /// having filled it in.
    pub fn fields(&self) -> Vec<(String, bool, String)> {
        let mut asked: Vec<(String, bool, String)> = self
            .sets
            .keys()
            .map(|name| (name.clone(), true, TEXT.to_string()))
            .collect();
        for attribute in &self.attributes {
            if !asked.iter().any(|(named, _, _)| named == &attribute.name) {
                asked.push((
                    attribute.name.clone(),
                    attribute.required,
                    attribute.kind.clone(),
                ));
            }
        }
        asked
    }
}

/// The description that fits a node, where the tree has one.
///
/// The type has to match and so does everything the description answers: a
/// compute node whose `function` is `mean` is described by `compute/mean`, and
/// one whose function is something else is not described by it at all. Where
/// several fit, the one that answers the most -- a description that states two
/// attributes is about a narrower thing than one that states one, and the
/// narrower is the one a panel should ask for.
///
/// `None` for a node the tree says nothing about, which is every payload: the
/// editor then asks for what the *type* says it needs, as it did before there
/// was a tree.
pub fn matching<'a>(
    described: &'a [Described],
    node_type: &str,
    attributes: &BTreeMap<String, String>,
) -> Option<&'a Described> {
    described
        .iter()
        .filter(|one| one.node_type == node_type)
        .filter(|one| {
            one.sets
                .iter()
                .all(|(name, value)| attributes.get(name) == Some(value))
        })
        .max_by_key(|one| one.sets.len())
}

/// Where tcsgui's own data is: the description tree, and the schemas beside it.
///
/// The first of these that exists, which is a search and not one path because
/// the same program is run from three places. In order:
///
/// * `$TCSGUI_DATA`, for a copy somebody is working on;
/// * `$XDG_DATA_HOME/tcsgui`, or `~/.local/share/tcsgui`, which is where a
///   person's own descriptions belong and where edited ones will be written;
/// * `/usr/local/share/tcsgui` and `/usr/share/tcsgui`, which is where
///   installing puts them -- see the `install-data` target;
/// * `tcsgui/share` and `share` under the working directory, which is the
///   source tree: running the editor out of a checkout should find the
///   checkout's own descriptions.
///
/// `None` if none of them is there, and that is not an error: an editor with no
/// description tree offers the node types it was built knowing, which is what
/// it offered before there was a tree at all.
pub fn data_dir() -> Option<PathBuf> {
    let mut tried: Vec<PathBuf> = Vec::new();

    if let Some(said) = std::env::var_os("TCSGUI_DATA") {
        tried.push(PathBuf::from(said));
    }
    if let Some(home) = std::env::var_os("XDG_DATA_HOME") {
        tried.push(PathBuf::from(home).join("tcsgui"));
    } else if let Some(home) = std::env::var_os("HOME") {
        tried.push(PathBuf::from(home).join(".local/share/tcsgui"));
    }
    tried.push(PathBuf::from("/usr/local/share/tcsgui"));
    tried.push(PathBuf::from("/usr/share/tcsgui"));
    tried.push(PathBuf::from("tcsgui/share"));
    tried.push(PathBuf::from("share"));

    tried.into_iter().find(|where_| where_.is_dir())
}

/// The schemas inside a data directory.
///
/// What they are for is the one thing an editor cannot do for a file somebody
/// is writing by hand or with another tool: tell them, in their own editor,
/// that `payloads` is a section and `payload` is not. They check the shape of a
/// file; whether a configuration can be flown is `tcsverify`'s to say. See
/// `share/schema/README`.
pub fn schema_in(data: &Path) -> PathBuf {
    data.join("schema")
}

/// The XSD and the JSON Schema that describe a version 2 configuration.
///
/// Named here rather than spelled out where a file is written, because a
/// configuration says which schema describes it and the two names have to be
/// the names the schemas are installed under.
pub const CONFIG_XSD: &str = "tcspecial.xsd";
pub const CONFIG_JSON_SCHEMA: &str = "tcspecial.schema.json";

/// And the JSON Schema that describes a simulator's own file.
///
/// No XSD beside it: a simulator configuration is written in YAML, and nothing
/// writes one in XML.
pub const SIM_JSON_SCHEMA: &str = "tcspecial-sim.schema.json";

/// How a file written to `destination` says which schema describes it.
///
/// A configuration carries a pointer to its own schema, so that an editor
/// opening one validates it without being told where the schema is: that is
/// the whole use of shipping the schemas, and a reference nobody has to set up
/// by hand is the only kind that gets used.
///
/// Relative to the file where the schemas are above it, which is the source
/// tree, and absolute where they are not, which is everywhere else. Both
/// spellings are resolved against the file by every tool that reads one, and
/// the relative one is what keeps a checkout's own examples pointed at the
/// checkout's own schemas rather than at a copy somebody may not have
/// installed.
///
/// Where no schema can be found at all this still names the installed path:
/// a file that says where its schema would be is more use to a reader than a
/// file that says nothing, and the one being written is not the place to
/// report that this program was not installed.
pub fn schema_reference(destination: &Path, schema: &str) -> String {
    // The source tree, from the file outwards. Six levels is past the deepest
    // a configuration sits in this repository (`tcslibgs/tests/actual`) with
    // room to spare, and a search without a limit is a search that walks to
    // the root of the filesystem.
    let here = destination.parent().unwrap_or(Path::new(""));
    let mut up = PathBuf::new();
    let mut back = String::new();
    for _ in 0..=6 {
        let candidate = here.join(&up).join("tcsgui/share/schema").join(schema);
        if candidate.is_file() {
            return format!("{back}tcsgui/share/schema/{schema}");
        }
        up.push("..");
        back.push_str("../");
    }

    // Then where installing puts them. Only an absolute answer: `data_dir`
    // searches the working directory too, and a path relative to wherever the
    // editor happened to be started is not a fact about the file.
    if let Some(data) = data_dir().filter(|data| data.is_absolute()) {
        let installed = schema_in(&data).join(schema);
        if installed.is_file() {
            return installed.display().to_string();
        }
    }

    format!("/usr/local/share/tcsgui/schema/{schema}")
}

/// The description tree inside a data directory.
pub fn nodes_in(data: &Path) -> PathBuf {
    data.join("nodes")
}

/// Every description the editor can find, and everything wrong with the tree.
///
/// Nothing and no complaint where there is no tree: see [`data_dir`].
pub fn described() -> (Vec<Described>, Vec<String>) {
    match data_dir().map(|data| nodes_in(&data)) {
        Some(tree) if tree.is_dir() => read_tree(&tree),
        _ => (Vec::new(), Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tree this repository ships.
    fn shipped() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("share/nodes")
    }

    /// The shipped tree reads, and says what it is expected to say.
    #[test]
    fn the_shipped_descriptions_read() {
        let (described, trouble) = read_tree(&shipped());
        assert_eq!(trouble, Vec::<String>::new());

        let names: Vec<&str> = described.iter().map(|one| one.node.as_str()).collect();
        assert_eq!(
            names,
            vec![
                // The ground link, which is at the root because it has no
                // Rust of its own, then the two families in the order the
                // code tree holds them.
                "moc",
                // What the vehicle carries: the two that are programs first,
                // the tree being sorted by path.
                "app_all_pl",
                "app_pl",
                "can",
                "device-file",
                "gpio",
                "hdlc",
                "i2c",
                "iio",
                "network",
                "rpmsg",
                "serial",
                "spi",
                // The vehicle's own, and the three programs among them come
                // first because the tree is sorted by path.
                "app_all_veh",
                "app_moc_veh",
                "app_payload_veh",
                "hysteresis",
                "mean",
                "process",
                "to_kelvin",
                "log",
                "null",
            ],
            "sorted by path, which puts a family's descriptions together"
        );

        // A computation is a compute node with its function already answered,
        // which is what `sets` is for.
        let mean = described.iter().find(|one| one.node == "mean").expect("mean");
        assert_eq!(mean.node_type, "compute");
        assert_eq!(mean.sets.get("function").map(String::as_str), Some("mean"));
        assert_eq!(mean.attributes.len(), 1);
        assert_eq!(mean.attributes[0].name, "inputs");
        assert!(mean.attributes[0].required);
        assert!(mean.attributes[0].about.contains("comma separated"), "{mean:?}");
        // Folded into one line, however the file wrapped it.
        assert!(!mean.attributes[0].about.contains('\n'));

        // The one written in XML says the same kind of thing, which is the
        // point of it being written in XML.
        let process = described
            .iter()
            .find(|one| one.node == "process")
            .expect("process");
        assert_eq!(process.node_type, "compute");
        assert_eq!(
            process.sets.get("function").map(String::as_str),
            Some("process")
        );
        let asked: Vec<&str> = process
            .attributes
            .iter()
            .map(|a| a.name.as_str())
            .collect();
        assert_eq!(asked, vec!["program", "timeout", "restart"]);
        assert!(process.attributes.iter().all(|a| a.required));

        // And a node with nothing to be told says nothing.
        let null = described.iter().find(|one| one.node == "null").expect("null");
        assert_eq!(null.node_type, "null");
        assert!(null.attributes.is_empty());
        assert!(null.sets.is_empty());
        assert!(!null.about.is_empty(), "every description says what it is");
    }

    /// Every attribute says what it takes, and says one of the words there
    /// are for it.
    ///
    /// The reader refuses a kind it does not know, so what this adds is the
    /// other direction: that the words in the tree are the words the window
    /// offers controls for, and that the ones which say `boolean` are the
    /// attributes `tcslibgs` reads as true or false. Two statements of one
    /// fact, so one is held against the other -- an attribute that became a
    /// true or false in the rules and stayed text here would be a field
    /// offered as a box to type `treu` into.
    #[test]
    fn every_kind_is_one_the_window_knows() {
        let (described, trouble) = read_tree(&shipped());
        assert!(trouble.is_empty(), "{trouble:?}");

        // What `tcslibgs` reads as true or false: `endpoint_config::parse_flag`
        // for a device's own flags, and `node::vehicle::Restart::of` for
        // whether a program comes back.
        let booleans = [
            "ten_bit",
            "pec",
            "rs485",
            "rs485_rts_on_send",
            "loopback",
            "can_fd",
            "restart",
        ];

        for one in &described {
            for attribute in &one.attributes {
                assert!(
                    KINDS.contains(&attribute.kind.as_str()),
                    "{}: \"{}\" takes a kind called \"{}\", which is not one of {}",
                    one.path.display(),
                    attribute.name,
                    attribute.kind,
                    KINDS.join(", ")
                );
                assert_eq!(
                    attribute.kind == "boolean",
                    booleans.contains(&attribute.name.as_str()),
                    "{}: \"{}\" says the kind \"{}\", and the rules read it the \
                     other way",
                    one.path.display(),
                    attribute.name,
                    attribute.kind
                );
            }
        }

        // And the tree says it where it matters: every app node is asked
        // whether its program comes back, and the answer is a true or false.
        let apps: Vec<&Described> = described
            .iter()
            .filter(|one| one.node.starts_with("app_"))
            .collect();
        assert_eq!(apps.len(), 5, "five kinds of app node");
        for app in apps {
            let restart = app
                .attributes
                .iter()
                .find(|attribute| attribute.name == "restart")
                .unwrap_or_else(|| panic!("{} says nothing about restarting", app.node));
            assert_eq!(restart.kind, "boolean", "{}", app.node);
        }
    }

    /// The tree's terminals are the ones the vehicle is flown by.
    ///
    /// The flags in a description are what a person reads and edits; the table
    /// in `tcslibgs::graph::terminals` is what the checking and the flight
    /// software read. Two statements of one fact, so one has to be held
    /// against the other -- and it is the tree that is checked, because the
    /// table is the one the vehicle obeys.
    ///
    /// Only the types that module has an answer for. A computation's sinks are
    /// the inputs it declares and a log's are whatever is written to it, so
    /// the table says nothing about those and the description is the only
    /// statement there is.
    #[test]
    fn the_trees_terminals_are_the_schemas() {
        let (described, trouble) = read_tree(&shipped());
        assert!(trouble.is_empty(), "{trouble:?}");
        assert!(!described.is_empty());

        for one in &described {
            let Some(table) = tcslibgs::graph::terminals::terminals_of(&one.node_type)
            else {
                continue;
            };
            for (terminal, said) in one.terminals.each() {
                let known =
                    table.sinks.contains(&terminal) || table.sources.contains(&terminal);
                assert_eq!(
                    said,
                    known,
                    "{}: {terminal} is {} in the description and {} in the table",
                    one.path.display(),
                    if said { "declared" } else { "not declared" },
                    if known { "declared" } else { "not" }
                );
            }
        }
    }

    /// A description that is about a whole type says what that type says.
    ///
    /// Two statements of one fact, and the same answer as for the terminals:
    /// the file is what a person reads and edits, `style::NodeType::attributes`
    /// is what a node's window falls back to for a type the tree does not
    /// describe, and a test holds one against the other. An attribute in one
    /// and not the other is a field somebody looks for and cannot find, or a
    /// field with nothing said about it.
    ///
    /// Asked only of the descriptions that are about a type as a whole -- a
    /// payload kind, the ground link, a log -- which is what `node ==
    /// node_type` means. A description that is one *variant* of a type is a
    /// different thing: `mean` lists what a mean needs and `hysteresis` what a
    /// hysteresis needs, where the table lists what any compute node may say,
    /// so the two are not the same list and are not asked to be. For those the
    /// description is the authority, the window being built from it.
    ///
    /// And every attribute anywhere in the tree has something said about it: a
    /// field with an empty line beside it is a field somebody has to guess at.
    #[test]
    fn a_description_of_a_whole_type_says_what_the_type_says() {
        let (described, trouble) = read_tree(&shipped());
        assert!(trouble.is_empty(), "{trouble:?}");

        for one in &described {
            for attribute in &one.attributes {
                assert!(
                    !attribute.about.trim().is_empty(),
                    "{}: \"{}\" has nothing said about it",
                    one.path.display(),
                    attribute.name
                );
            }

            if one.node != one.node_type {
                continue;
            }
            let Some(found) = crate::style::NodeType::ALL
                .iter()
                .find(|t| t.spelling() == one.node_type)
            else {
                continue;
            };

            let described: Vec<(&str, bool)> = one
                .attributes
                .iter()
                .map(|a| (a.name.as_str(), a.required))
                .collect();
            let offered: Vec<(&str, bool)> = found
                .attributes()
                .iter()
                .map(|(name, required)| (*name, *required))
                .collect();
            assert_eq!(
                described,
                offered,
                "{}: the description and the editor offer different attributes",
                one.path.display()
            );
        }
    }

    /// Every description has its Rust, and every vehicle node its description.
    ///
    /// The correspondence the two trees exist for. One way round, a description
    /// with no Rust is a node the editor offers and the vehicle cannot run; the
    /// other, a computation implemented and not described is a node nobody can
    /// place -- which is how a feature gets built and never used.
    ///
    /// The tree mirrors the code tree, family by family: a description at
    /// `vehicle/compute/mean.yaml` is about the Rust at
    /// `tcspecial/src/node/vehicle/compute/mean.rs`, and one at
    /// `payload/i2c.yaml` about `node/payload/i2c/`.
    ///
    /// `moc.yaml` is the exception, and sits at the root because of it: the
    /// ground station has no Rust of its own. It is not part of the vehicle at
    /// all -- it is the other end of the link -- and the code that serves one
    /// is the network payload's.
    ///
    /// The second direction is asked only of the directories the descriptions
    /// cover: once a family has one description in it, all of its files need
    /// one, which is what keeps a family from being half described.
    #[test]
    fn every_description_has_its_rust() {
        let code = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tcspecial/src/node");
        let (described, _) = read_tree(&shipped());
        assert!(!described.is_empty());

        let mut families: Vec<PathBuf> = Vec::new();
        for one in &described {
            // The ground link, which has none: see above.
            if one.path == Path::new("moc.yaml") {
                continue;
            }
            // The same path, with the extension taken off:
            // `vehicle/compute/mean.yaml` is `vehicle/compute/mean.rs`, or
            // the directory `vehicle/compute/mean/`.
            let stem = one.path.with_extension("");
            let file = code.join(&stem).with_extension("rs");
            let directory = code.join(&stem);
            assert!(
                file.is_file() || directory.is_dir(),
                "{}: nothing at {} or {}",
                one.path.display(),
                file.display(),
                directory.display()
            );
            if let Some(family) = stem.parent() {
                if !family.as_os_str().is_empty() && !families.contains(&family.to_path_buf()) {
                    families.push(family.to_path_buf());
                }
            }
        }

        for family in families {
            let where_ = code.join(&family);
            for entry in std::fs::read_dir(&where_).expect("the family's code") {
                let path = entry.expect("an entry").path();
                // `mod.rs` is the family's own module rather than one of its
                // nodes, and a directory is a node whose code is several
                // files -- described by the entry above it.
                if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                    continue;
                }
                if path.file_name().and_then(|n| n.to_str()) == Some("mod.rs") {
                    continue;
                }
                let stem = path.file_stem().expect("a name").to_string_lossy().to_string();
                assert!(
                    described
                        .iter()
                        .any(|one| one.path.with_extension("") == family.join(&stem)),
                    "{} is implemented and nothing in {} describes it",
                    path.display(),
                    shipped().join(&family).display()
                );
            }
        }
    }

    /// A computation's description asks for what the computation takes.
    ///
    /// The other correspondence, and the one that would drift: the parameters
    /// of each computation are the flight software's own list -- see
    /// `tcslibgs::node::vehicle::compute::attributes_of` -- and a description that asked
    /// for something else would be a panel that fills in a field the checking
    /// then refuses, or leaves out one it requires.
    #[test]
    fn a_computations_description_asks_for_what_it_takes() {
        let (described, _) = read_tree(&shipped());
        let computations: Vec<&Described> = described
            .iter()
            .filter(|one| one.node_type == "compute")
            .collect();
        assert_eq!(computations.len(), 4, "every computation is described");

        for one in computations {
            let function = one
                .sets
                .get("function")
                .unwrap_or_else(|| panic!("{}: a computation says which", one.node));
            let takes = tcslibgs::node::vehicle::compute::attributes_of(function)
                .unwrap_or_else(|| panic!("{}: there is no such computation", one.node));

            // What the file asks for, plus what it fills in itself, is what
            // the computation takes -- no more and no less.
            let mut said: Vec<&str> = one.attributes.iter().map(|a| a.name.as_str()).collect();
            said.extend(one.sets.keys().map(String::as_str));
            said.sort_unstable();
            let mut wanted: Vec<&str> = takes.to_vec();
            wanted.sort_unstable();
            assert_eq!(said, wanted, "{}", one.node);
        }
    }

    /// A node is described by the description that answers what it says.
    ///
    /// Which is how one node type becomes several things to place and several
    /// things to ask about: a compute node whose function is `mean` is a mean,
    /// and the panel should ask it for a mean's attributes rather than for
    /// every attribute any computation has.
    #[test]
    fn a_node_is_matched_to_the_description_that_fits_it() {
        let (described, _) = read_tree(&shipped());

        let said = |pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect::<BTreeMap<String, String>>()
        };

        let mean = matching(&described, "compute", &said(&[("function", "mean")]))
            .expect("a compute node with a function is described");
        assert_eq!(mean.node, "mean");
        // What the panel asks: the function it is, and then what a mean says.
        assert_eq!(
            mean.asked(),
            vec![("function".to_string(), true), ("inputs".to_string(), true)]
        );

        let thermostat = matching(
            &described,
            "compute",
            &said(&[("function", "hysteresis"), ("setpoint", "293.15")]),
        )
        .expect("and so is this one");
        assert_eq!(thermostat.node, "hysteresis");

        // A computation that says no function yet is not described by any of
        // them: a node halfway through being filled in is not a mean because
        // mean is the first description in the tree.
        assert_eq!(matching(&described, "compute", &said(&[])), None);
        // Nor one whose function is not a computation at all.
        assert_eq!(
            matching(&described, "compute", &said(&[("function", "fourier")])),
            None
        );

        // A type described without answering anything matches whatever it is
        // told, there being nothing to disagree with.
        let null = matching(&described, "null", &said(&[])).expect("the null node");
        assert_eq!(null.node, "null");
        assert!(null.asked().is_empty(), "a null node says nothing");

        // And a payload is described as itself: one kind, one description, no
        // `sets`, so whatever a node of that type says it matches.
        let sensor = matching(&described, "i2c", &said(&[("address", "0x48")]))
            .expect("an i2c node is described");
        assert_eq!(sensor.node, "i2c");
        assert!(sensor.terminals.payload_source && sensor.terminals.moc_sink);
        assert!(!sensor.terminals.payload_sink && !sensor.terminals.moc_source);
    }

    /// A description that is not one is refused by name, and the rest are read.
    ///
    /// A tree people edit has a half-written file in it sooner or later, and an
    /// editor that offered nothing because of one of them would be an editor a
    /// typo turns off.
    #[test]
    fn a_description_that_is_not_one_is_refused_on_its_own() {
        let where_ = tempfile::tempdir().expect("a directory");
        let root = where_.path();
        std::fs::create_dir_all(root.join("compute")).expect("a family");

        std::fs::write(
            root.join("compute/mean.yaml"),
            "node: mean\ntype: compute\nabout: An average.\n",
        )
        .expect("a good one");
        // A word nothing reads, which is a value silently ignored if it is not
        // refused.
        std::fs::write(
            root.join("compute/odd.yaml"),
            "node: odd\ntype: compute\nabuot: a typo\n",
        )
        .expect("a bad one");
        // And one that says nothing about which node it is.
        std::fs::write(root.join("nameless.yaml"), "node: \"\"\ntype: compute\n")
            .expect("a nameless one");
        // Not a description at all, and not complained about either.
        std::fs::write(root.join("README"), "The tree.\n").expect("a readme");

        let (described, trouble) = read_tree(root);
        let names: Vec<&str> = described.iter().map(|one| one.node.as_str()).collect();
        assert_eq!(names, vec!["mean"], "the good one is still read");
        assert_eq!(trouble.len(), 2, "{trouble:?}");
        assert!(
            trouble.iter().any(|said| said.contains("abuot")),
            "{trouble:?}"
        );
        assert!(
            trouble.iter().any(|said| said.contains("nameless.yaml")),
            "{trouble:?}"
        );
    }

    /// Two descriptions of one name are refused, naming it.
    #[test]
    fn a_name_describes_one_node() {
        let where_ = tempfile::tempdir().expect("a directory");
        let root = where_.path();
        std::fs::create_dir_all(root.join("compute")).expect("a family");
        for path in ["compute/mean.yaml", "mean.yaml"] {
            std::fs::write(
                root.join(path),
                "node: mean\ntype: compute\nabout: An average.\n",
            )
            .expect("a description");
        }

        let (described, trouble) = read_tree(root);
        assert_eq!(described.len(), 2, "both are read");
        assert!(
            trouble.iter().any(|said| said.contains("\"mean\"")),
            "{trouble:?}"
        );
    }

    /// The editor finds the tree it was built beside.
    ///
    /// Run from the source tree, which is where the tests are run from: the
    /// search has to end at the checkout's own descriptions rather than at
    /// nothing, or every test of the palette would be a test of an empty one.
    #[test]
    fn the_tree_is_found_from_the_source_directory() {
        // Whatever this machine has installed is not what is being tested, so
        // the search starts where this crate is.
        let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("share");
        assert!(data.is_dir(), "{}", data.display());
        let tree = nodes_in(&data);
        assert!(tree.is_dir(), "{}", tree.display());

        let (described, trouble) = read_tree(&tree);
        assert!(!described.is_empty());
        assert_eq!(trouble, Vec::<String>::new());
    }
}
