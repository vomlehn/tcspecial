//! The simulator's configuration, as the editor holds it.
//!
//! A second file, given as a second argument, and a different thing from the
//! configuration a vehicle flies: it says how fast a *simulated* payload
//! produces packets, how a packet is divided into segments, and what the
//! payload does wrong. None of that is a property of the payload -- a flight
//! configuration describing hardware that drops a tenth of its packets would
//! be describing hardware nobody would fly -- so it lives in its own file,
//! joined to the configuration by name.
//!
//! # What of it is on the canvas
//!
//! Its physics model, and nothing else. The simulator file adds settings to
//! payloads that are already on the canvas and declares groups of settings
//! for them to share, and a drawing of those would be the same drawing with
//! numbers on it: they are edited from the menu, one window for the payloads
//! and one for the groups. The physics model is different: its components
//! are things joined to one another and to the payloads, so they are drawn,
//! as nodes in the Physics pane, with their connections and each stand-in's
//! `connects` as links. See [`Physics`].
//!
//! # Held as text
//!
//! Every setting as the words a file holds, for the reason the `tcspecial`
//! section is: the editor has to be able to write an interval somebody is
//! halfway through typing, and a file it cannot read back is a file it has
//! destroyed. The names come from `tcslibgs::sim_config::SETTINGS`, which is
//! the one list of what a setting may be called, and a value nothing
//! recognises is kept rather than dropped -- a file from a later build is
//! readable, and a setting nobody can see is one that gets written back out
//! without anybody knowing it is there.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use tcslibgs::endpoint_config::Scalar;
use tcslibgs::format::ConfigFormat;

/// What one group or one simulated payload says, by setting name.
pub type Settings = BTreeMap<String, String>;

/// The simulator configuration being edited.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
pub struct Simulator {
    /// The file it came from and goes back to.
    ///
    /// Held here rather than derived from the configuration's name, because it
    /// is a second argument and may be called anything: the shipped sets name
    /// it after the configuration with `sim` on the end, and nothing requires
    /// that.
    pub path: PathBuf,
    /// What the file says it is, kept so that saving does not drop it.
    pub description: Option<String>,
    /// The `tcssim` section -- where tcssim answers whether it is ready --
    /// kept so that saving does not drop it, as the masses are. Without one
    /// tcssim will not start, and an editor that wrote it away would be the
    /// thing that broke a set that worked.
    #[serde(default)]
    pub tcssim: Settings,
    /// The world: the file's `world` section, its ambient temperature and
    /// its physics model's components and their connections. Empty for a
    /// file without one, and written only where it holds something.
    #[serde(default)]
    pub world: World,
    /// The groups of settings, by name.
    pub groups: BTreeMap<String, Settings>,
    /// The simulated payloads, by the name of the payload each stands in for.
    ///
    /// The names are the configuration's: every payload of a configuration
    /// needs settings here, and a name here that no payload has is an error
    /// rather than a line with no effect. So the editor offers one entry per
    /// payload node and never invents a name.
    pub payloads: BTreeMap<String, Settings>,
    /// The type each simulated payload was written under in the file, by
    /// its name: not a setting, and offered by no window, the configuration
    /// deciding a payload's type. Kept for an entry the configuration has
    /// no payload of, which is written back under the type it came with --
    /// and as the list of entries the file has, each written back while it
    /// is here whether or not it states anything.
    #[serde(default)]
    pub kinds: BTreeMap<String, String>,
}

/// The world, as the editor holds it: every setting as the words a file
/// holds, as the payloads' are, so that a figure somebody is halfway
/// through typing can be written.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
pub struct World {
    /// The temperature every thermal mass's surroundings are about, as
    /// written: `288.15 K`. Empty where unstated.
    #[serde(default)]
    pub ambient_temp: String,
    /// The physics model's components, by name.
    pub components: BTreeMap<String, PhysicsComponent>,
    /// The connections, each an output and the input it is joined to, as
    /// `heater_1.heat` and `zone.heat`. A set, in name order: nothing in
    /// a model is in an order that means anything.
    pub connections: std::collections::BTreeSet<(String, String)>,
    /// Where each component was put in the Physics pane, by name, as the
    /// file's `tcsgui` section says: a component named nowhere here is laid
    /// out by name. Kept for a component removed, so that undo puts it back
    /// where it was.
    #[serde(default)]
    pub placed: BTreeMap<String, (i32, i32)>,
}

/// One component of the physics model.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
pub struct PhysicsComponent {
    /// Its type, one of `tcslibgs::physics::TYPES`.
    pub kind: String,
    /// What it states, by setting name.
    pub settings: Settings,
}

impl World {
    /// Whether there is nothing in it.
    ///
    /// # Returns
    ///
    /// Whether it states no ambient temperature, and has no components and
    /// no connections.
    pub fn is_empty(&self) -> bool {
        self.ambient_temp.trim().is_empty()
            && self.components.is_empty()
            && self.connections.is_empty()
    }

    /// A name for a new component of a type: the type and the first number
    /// from 1 that no component has, as `heater_3`.
    ///
    /// # Arguments
    ///
    /// * `kind` - The type.
    ///
    /// # Returns
    ///
    /// The name.
    pub fn fresh_name(&self, kind: &str) -> String {
        (1..)
            .map(|n| format!("{kind}_{n}"))
            .find(|name| !self.components.contains_key(name))
            .expect("a free name among the numbers")
    }

    /// Everything the simulator's own reader says is wrong with the world as
    /// it stands: a figure without its unit or in another, a setting a type
    /// requires and is not given, an ambient temperature missing where there
    /// is a zone, a connection that names nothing or joins two outputs. What
    /// the stand-ins are joined to is not here: the configuration says which
    /// payloads there are, and `tcsverify` checks the two together.
    ///
    /// # Returns
    ///
    /// Each problem: the component it is about, or `None` for one about the
    /// world or its connections, and what is wrong, in the reader's words.
    pub fn problems(&self) -> Vec<(Option<String>, String)> {
        if self.is_empty() {
            return Vec::new();
        }
        let mut text = String::new();
        world_written(&mut text, self, true);
        match tcslibgs::sim_config::SimConfigFile::parse(
            &text,
            ConfigFormat::Yaml,
        ) {
            Ok(file) => file
                .world
                .map(|world| world.problems())
                .unwrap_or_default()
                .into_iter()
                .map(|problem| (problem.component.clone(), problem.to_string()))
                .collect(),
            Err(why) => vec![(None, why.to_string())],
        }
    }

    /// The connections that join a component, to or from it.
    ///
    /// # Arguments
    ///
    /// * `name` - The component.
    ///
    /// # Returns
    ///
    /// Each, as (from, to).
    pub fn connections_of(&self, name: &str) -> Vec<(String, String)> {
        let of = |end: &str| end.split_once('.').map(|(c, _)| c) == Some(name);
        self.connections
            .iter()
            .filter(|(from, to)| of(from) || of(to))
            .cloned()
            .collect()
    }
}

impl Simulator {
    /// The simulated payloads joined to a component, each with its whole
    /// `connects` as it stands: what removing the component takes out of
    /// them, and undoing it puts back.
    ///
    /// # Arguments
    ///
    /// * `name` - The component.
    ///
    /// # Returns
    ///
    /// Each payload, as the file keys it, and its `connects`, in name order.
    pub fn joins_of(&self, name: &str) -> Vec<(String, String)> {
        let mine = format!("{name}.");
        self.payloads
            .iter()
            .filter_map(|(payload, settings)| {
                let said = settings.get("connects")?;
                tcslibgs::sim_config::terminals_of(said)
                    .any(|terminal| terminal.starts_with(&mine))
                    .then(|| (payload.clone(), said.clone()))
            })
            .collect()
    }

    /// An empty simulator configuration for this file.
    ///
    /// # Arguments
    ///
    /// * `path` - The file the configuration is to be written to.
    ///
    /// # Returns
    ///
    /// A configuration that says nothing, for `path`.
    pub fn new(path: &Path) -> Self {
        Simulator {
            path: path.to_path_buf(),
            ..Simulator::default()
        }
    }

    /// A simulator configuration for a file the editor begins: each payload
    /// named under its type, and nothing else.
    ///
    /// So that the new file says which payloads there are to simulate,
    /// without a key for every question a payload could be asked: a key
    /// with nothing after it is a line that says nothing, and the windows
    /// are where the questions are.
    ///
    /// # Arguments
    ///
    /// * `path`     - The file the configuration is to be written to.
    /// * `payloads` - The configuration's payloads that can be simulated,
    ///                each with its type.
    ///
    /// # Returns
    ///
    /// The configuration, with an entry and no settings for each payload.
    pub fn begun(path: &Path, payloads: &[(String, String)]) -> Self {
        let mut simulator = Simulator::new(path);
        for (name, kind) in payloads {
            simulator.payloads.insert(name.clone(), Settings::new());
            simulator.kinds.insert(name.clone(), kind.clone());
        }
        simulator
    }

    /// The name a node's simulated settings are held under, from the name
    /// the editor holds the node under.
    ///
    /// The node's own name, without the subdiagram it is drawn in: the file
    /// joins a simulated payload to its payload by that name alone, as
    /// tcssim does, and the reader allows no two nodes one name across every
    /// file of a configuration. So a payload in an included file is
    /// `gpio-in-tmass` here, never `../thermostat-common/thermostat/
    /// gpio-in-tmass`, and the settings are held exactly as the file keys
    /// them.
    ///
    /// # Arguments
    ///
    /// * `held` - The node's name as the editor holds it, qualified by its
    ///            subdiagram or not.
    ///
    /// # Returns
    ///
    /// The key into [`Simulator::payloads`].
    pub fn key_of(held: &str) -> String {
        crate::Qualified::parse(held).written()
    }

    /// Renames a simulated payload, its settings and the type it was
    /// written under with it.
    ///
    /// # Arguments
    ///
    /// * `was` - The name it had.
    /// * `now` - The name it has.
    pub fn rename(&mut self, was: &str, now: &str) {
        if let Some(settings) = self.payloads.remove(was) {
            self.payloads.insert(now.to_string(), settings);
        }
        if let Some(kind) = self.kinds.remove(was) {
            self.kinds.insert(now.to_string(), kind);
        }
    }

    /// What a payload or a group may be asked, for a window to show.
    ///
    /// The settings list, and for a payload the group it takes its settings
    /// from. None of them is required: a payload that says nothing takes
    /// everything from its group, and a group that says nothing is a group
    /// nothing is settled by -- which the simulator's own checking reports.
    ///
    /// # Arguments
    ///
    /// * `payload` - Whether the fields are a payload's, which adds `group`,
    ///               rather than a group's.
    ///
    /// # Returns
    ///
    /// The names of the fields, in the order a window shows them.
    pub fn fields(payload: bool) -> Vec<&'static str> {
        let mut fields = Vec::new();
        if payload {
            fields.push("group");
        }
        fields.extend(tcslibgs::sim_config::SETTINGS.iter().copied());
        fields
    }
}

/// One entry of either list, as a file writes it.
///
/// Its name, the type a simulated payload is written under, and everything
/// else: the settings are taken as the words they are, so that one nothing
/// recognises is kept rather than thrown away. Read by
/// `tcslibgs::sim_grouped`, as the simulator reads its own, so that the two
/// read the one shape.
#[derive(Debug, Deserialize)]
struct Entry {
    #[serde(default)]
    name: String,
    #[serde(skip)]
    kind: Option<String>,
    #[serde(flatten)]
    said: BTreeMap<String, Scalar>,
}

impl tcslibgs::sim_grouped::SimEntry for Entry {
    fn name(&self) -> &str {
        &self.name
    }

    fn written_under(&mut self, kind: &str) {
        self.kind = Some(kind.to_string());
    }
}

/// A simulator configuration file, as it parses.
///
/// Permissive on purpose, and not [`tcslibgs::sim_config::SimConfigFile`]: that
/// one reads the settings into the types the simulator wants, which is right
/// for running and wrong for editing. Half a number is not a number yet.
#[derive(Debug, Deserialize)]
struct File {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    tcssim: BTreeMap<String, Scalar>,
    #[serde(default)]
    world: Option<WorldFile>,
    /// A section a simulator file held before the physics model was made of
    /// parts: read only to be refused, as every program refuses it.
    #[serde(default)]
    thermal_mass: Option<serde::de::IgnoredAny>,
    #[serde(default, deserialize_with = "tcslibgs::sim_grouped::groups")]
    simulated_payload_groups: Vec<Entry>,
    #[serde(default, deserialize_with = "tcslibgs::sim_grouped::payloads")]
    simulated_payloads: Vec<Entry>,
    /// Where the editor drew each physics component, read as every program
    /// reads it.
    #[serde(default)]
    tcsgui: Option<tcslibgs::sim_config::SimTcsgui>,
}

/// The `world` section, as it parses: permissive, as [`File`] is.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldFile {
    #[serde(default)]
    ambient_temp: Option<Scalar>,
    #[serde(default)]
    physics: Option<PhysicsFile>,
}

/// The world's `physics`, as it parses.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PhysicsFile {
    /// Each entry `type <kind>:` and the list of that type's components,
    /// each a map whose `component` names it.
    #[serde(default)]
    components: Vec<BTreeMap<String, Vec<BTreeMap<String, Scalar>>>>,
    #[serde(default)]
    connections: Vec<ConnectionFile>,
}

/// One connection, as it parses.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectionFile {
    from: String,
    to: String,
}

/// Takes in the `world` section as the editor holds it.
///
/// # Arguments
///
/// * `file` - The section, as it parsed.
///
/// # Returns
///
/// The world.
///
/// # Errors
///
/// A message for an entry that is not `type` and a type the model has, a
/// component with no name, and two components of one name: the editor
/// cannot draw or write back what it cannot tell apart.
fn world_of(file: WorldFile) -> Result<World, String> {
    let mut world = World {
        ambient_temp: file
            .ambient_temp
            .map(|said| said.as_str().to_string())
            .unwrap_or_default(),
        ..World::default()
    };
    let Some(file) = file.physics else {
        return Ok(world);
    };
    for entry in file.components {
        for (key, components) in entry {
            let kind = key
                .strip_prefix("type ")
                .filter(|kind| tcslibgs::physics::TYPES.contains(kind))
                .ok_or_else(|| {
                    format!(
                        "physics components: \"{key}\" is not the word type \
                         and one of {}",
                        tcslibgs::physics::TYPES.join(", ")
                    )
                })?;
            for mut said in components {
                let name = said
                    .remove("component")
                    .map(|name| name.as_str().to_string())
                    .filter(|name| !name.is_empty())
                    .ok_or_else(|| {
                        format!(
                            "physics components: a {kind} has no name: name \
                             it with component: <name>"
                        )
                    })?;
                // A setting written empty is one not stated, and the form
                // writes it empty again: held as not there.
                let settings = said
                    .into_iter()
                    .map(|(setting, value)| {
                        (setting, value.as_str().to_string())
                    })
                    .filter(|(_, value)| !value.is_empty())
                    .collect();
                let component = PhysicsComponent {
                    kind: kind.to_string(),
                    settings,
                };
                if world.components.insert(name.clone(), component).is_some() {
                    return Err(format!(
                        "two physics components are called \"{name}\"; a \
                         connection or a stand-in names one, so a name has \
                         to name one component"
                    ));
                }
            }
        }
    }
    world.connections = file
        .connections
        .into_iter()
        .map(|c| (c.from.trim().to_string(), c.to.trim().to_string()))
        .collect();
    Ok(world)
}

/// Whether this is a name a simulator configuration may have.
///
/// It must end in `.yaml`. A simulator configuration is a YAML file: that is
/// what the shipped sets are, and one spelling of it is one thing for a reader
/// to know rather than two. The editor could write it as XML -- the formats
/// are two spellings of one set of values everywhere else in the project --
/// and does not, because nothing wanted the second spelling and a file nobody
/// writes is a format nobody has read back.
///
/// `.yml` is refused with the rest, for the same reason: it is the same
/// language under a second name.
///
/// The case is not part of it, as it is not anywhere else a configuration's
/// extension is read.
///
/// # Arguments
///
/// * `path` - The file name to check.
///
/// # Errors
///
/// A message naming the file and saying it must end in `.yaml`.
pub fn must_be_yaml(path: &Path) -> Result<(), String> {
    let yaml = path
        .extension()
        .map(|given| given.eq_ignore_ascii_case("yaml"))
        .unwrap_or(false);
    if yaml {
        return Ok(());
    }

    Err(format!(
        "{}: a simulator configuration is written as YAML, so its name has to \
         end in \".yaml\"",
        path.display()
    ))
}

/// Why a simulator file was not opened, and what to do about it.
///
/// Two kinds, because they want opposite answers. A file the editor cannot
/// *read* is one it cannot write back faithfully either, so the session goes
/// on with an empty one and somewhere to type -- an editor that would not
/// start on a damaged file is an editor that cannot be used to fix one. A file
/// the editor can read but must not *use* is a different matter: it parses, so
/// adopting half of it would mean writing the other half away at the next
/// edit. That one is refused, and the file is left on the disk exactly as it
/// is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    /// It cannot be parsed, or its name is not a YAML one.
    Unreadable(String),
    /// It parses, and says something the two files cannot both mean.
    Unusable(String),
}

impl Trouble {
    /// What to say about it.
    pub fn why(&self) -> &str {
        match self {
            Trouble::Unreadable(why) | Trouble::Unusable(why) => why,
        }
    }

    /// Whether the session should carry on with an empty one in its place.
    pub fn leaves_somewhere_to_type(&self) -> bool {
        matches!(self, Trouble::Unreadable(_))
    }
}

/// Reads the simulator configuration at `path`.
///
/// A file that is not there is not an error: a name that does not exist yet is
/// how a new one is begun, and an empty simulator configuration is what that
/// is. A name that is not YAML is an error whether or not the file exists --
/// see [`must_be_yaml`] -- because it is the name this would be written back
/// to.
///
/// # Arguments
///
/// * `path` - The simulator configuration file, which must be named `.yaml`.
///
/// # Returns
///
/// The configuration read, or an empty one for a file that is not there.
///
/// # Errors
///
/// [`Trouble::Unreadable`] for a name that is not YAML or a file that cannot be
/// read or parsed; [`Trouble::Unusable`] for a file with two entries of one
/// name in a list, or written in the shape before its payloads were grouped
/// by type.
pub fn read(path: &Path) -> Result<Simulator, Trouble> {
    must_be_yaml(path).map_err(Trouble::Unreadable)?;

    let mut simulator = Simulator::new(path);
    if !path.exists() {
        return Ok(simulator);
    }

    let unreadable =
        |e: &dyn std::fmt::Display| Trouble::Unreadable(format!("{e}"));
    let format = ConfigFormat::from_data_path(path)
        .ok_or_else(|| unreadable(&"not named .yaml"))?;
    let text = std::fs::read_to_string(path).map_err(|e| unreadable(&e))?;
    // A file in the shape it had before its payloads were grouped by type
    // is refused and left as it is, as every program refuses it: read as
    // nothing, it would be written away at the next edit.
    if let Some(why) = tcslibgs::sim_reshaped::sim_refused(&text, format) {
        return Err(Trouble::Unusable(why));
    }
    let file: File = format.parse(&text).map_err(|e| unreadable(&e))?;

    if file.thermal_mass.is_some() {
        return Err(Trouble::Unusable(
            tcslibgs::sim_config::MOVED_THERMAL_MASS.to_string(),
        ));
    }
    simulator.description = file.description;
    if let Some(world) = file.world {
        simulator.world = world_of(world).map_err(Trouble::Unusable)?;
    }
    // Where each component was drawn: see World::placed.
    if let Some(tcsgui) = &file.tcsgui {
        simulator.world.placed = tcsgui
            .nodes
            .iter()
            .map(|placed| (placed.name.clone(), (placed.x, placed.y)))
            .collect();
    }
    simulator.tcssim = file
        .tcssim
        .into_iter()
        .map(|(name, value)| (name, value.as_str().to_string()))
        .collect();

    // Two entries of one name are refused rather than read. A name is how
    // this file is joined to the configuration, so two of them are two sets
    // of settings for one payload and the file does not say which applies --
    // and reading it as a map would quietly keep the second and throw the
    // first away. Refusing leaves the file on the disk for somebody to fix,
    // which is what it would not do if the editor adopted half of it and
    // wrote it back on the next edit.
    for (what, entries) in [
        ("simulated payload group", &file.simulated_payload_groups),
        ("simulated payload", &file.simulated_payloads),
    ] {
        let mut seen: std::collections::BTreeSet<&str> =
            std::collections::BTreeSet::new();
        for entry in entries {
            if !seen.insert(entry.name.as_str()) {
                return Err(Trouble::Unusable(format!(
                    "two {what}s are called \"{}\"; a name is how this file \
                     and the configuration are joined, so it has to name one \
                     entry",
                    entry.name
                )));
            }
        }
    }

    let settled = |entries: Vec<Entry>| -> BTreeMap<String, Settings> {
        entries
            .into_iter()
            .map(|entry| {
                let said = entry
                    .said
                    .into_iter()
                    .map(|(name, value)| (name, value.as_str().to_string()))
                    .collect();
                (entry.name, said)
            })
            .collect()
    };
    simulator.kinds = file
        .simulated_payloads
        .iter()
        .filter_map(|entry| Some((entry.name.clone(), entry.kind.clone()?)))
        .collect();
    simulator.groups = settled(file.simulated_payload_groups);
    simulator.payloads = settled(file.simulated_payloads);
    Ok(simulator)
}

/// The simulator configuration as YAML.
///
/// What the file says and nothing else: every key that holds a value, and
/// no key that does not. A setting somebody empties leaves the file, as one
/// cleared in the configuration does, and a payload is written with only
/// what it states. A file the editor begins names each payload of the
/// configuration under its type and nothing more -- see
/// [`Simulator::begun`] -- and from then on an entry is written while it is
/// in the file or states something.
///
/// Scalars as a person writes them: bare wherever YAML reads the bare word
/// as what was typed, so that `true` is a boolean, `1 s` a time and
/// `heater_1` a name, and in double quotes only where it would read as
/// something else. A physics figure is the exception, and is quoted as
/// every shipped world quotes it: each is read as text, its unit and all,
/// and a bare `0.02` would be a number. A description too long for a line
/// is folded over several, as one written by hand is.
///
/// `payloads` is the configuration's payload nodes and their types, because
/// it is the configuration that decides what each payload is: an entry is
/// written under its payload's type. An entry the simulator file holds for a
/// name that is not among them is written out too, under the type the file
/// gave it: it is somebody's typing, and `tcsverify` is what says a simulated
/// payload names no payload.
///
/// In the one order a simulator file has, whatever order anything was made
/// in: the groups by name, the simulated payloads grouped by type, the types
/// by name and a type's payloads by name, and each entry's settings by name.
/// The file a session read is written back as it was, with only the edit
/// made to it -- see `crate::faithful` -- so this order is the order of
/// what the editor adds, and of a file it begins.
///
/// # Arguments
///
/// * `simulator` - The simulator configuration to write.
/// * `payloads`  - The configuration's payload nodes, each with its type.
///
/// # Returns
///
/// The file's text.
pub fn to_yaml(simulator: &Simulator, payloads: &[(String, String)]) -> String {
    let mut out = String::new();

    // Which schema describes this file, in the spelling a YAML language
    // server reads. The same thing a configuration says about its own and for
    // the same reason: a schema an editor has to be pointed at by hand is a
    // schema nobody points it at. The blank line under it is what keeps it a
    // note about the file rather than about the first entry -- here it comes
    // from whatever is written next, each of which opens with one.
    out.push_str(&format!(
        "# yaml-language-server: $schema={}\n",
        crate::nodes::schema_reference(
            &simulator.path,
            crate::nodes::SIM_JSON_SCHEMA
        )
    ));

    if let Some(description) = &simulator.description {
        out.push('\n');
        out.push_str(&folded("description: ", description));
    }

    // What tcssim itself is told, as the file said it. A section with
    // nothing in it is one the simulator refuses, and what it must say -- an
    // address no other run uses -- is not the editor's to invent.
    let said = stated(&simulator.tcssim);
    if !said.is_empty() {
        out.push_str("\ntcssim:\n");
        for (field, value) in said {
            out.push_str(&format!("  {field}: {}\n", setting(value)));
        }
    }

    // The physics model the payloads attach to, before them, because that
    // is the order the file reads in: a model exists and then payloads attach
    // to it.
    world_written(&mut out, &simulator.world, false);

    // Each group, in name order, as the map holds them, a blank line apart.
    if !simulator.groups.is_empty() {
        out.push_str("\nsimulated_payload_groups:\n");
        for (at, (name, said)) in simulator.groups.iter().enumerate() {
            if at > 0 {
                out.push('\n');
            }
            out.push_str(&format!("  - group: {}\n", name_scalar(name)));
            entry_settings(&mut out, said, "    ");
        }
    }

    // Each entry under its payload's type, or failing that under the type
    // the file gave it. A name with no type from either -- which reading a
    // file never makes, every entry read being written under one -- has
    // nowhere to be written, and is left out.
    let mut typed: BTreeMap<&str, BTreeMap<&str, &Settings>> = BTreeMap::new();
    let empty = Settings::new();
    let mut names: Vec<&String> = simulator.payloads.keys().collect();
    names.extend(simulator.kinds.keys());
    for name in names {
        let said = simulator.payloads.get(name).unwrap_or(&empty);
        if stated(said).is_empty() && !simulator.kinds.contains_key(name) {
            continue;
        }
        let kind = payloads
            .iter()
            .find(|(theirs, _)| theirs == name)
            .map(|(_, kind)| kind)
            .or_else(|| simulator.kinds.get(name));
        if let Some(kind) = kind {
            typed.entry(kind.as_str()).or_default().insert(name, said);
        }
    }
    if !typed.is_empty() {
        out.push_str("\nsimulated_payloads:\n");
        for (at, (kind, entries)) in typed.into_iter().enumerate() {
            if at > 0 {
                out.push('\n');
            }
            out.push_str(&format!("  - type {kind}:\n"));
            for (at, (name, said)) in entries.into_iter().enumerate() {
                if at > 0 {
                    out.push('\n');
                }
                out.push_str(&format!(
                    "      - payload: {}\n",
                    name_scalar(name)
                ));
                entry_settings(&mut out, said, "        ");
            }
        }
    }

    // Where each component was drawn, last, as a configuration's own
    // tcsgui section is: about a screen and not the vehicle. Only the
    // components the world has, as a configuration writes its nodes'
    // positions from the nodes it has: a position left behind by one
    // removed is kept for undo, and goes from the file.
    let placed: Vec<(&String, &(i32, i32))> = simulator
        .world
        .placed
        .iter()
        .filter(|(name, _)| simulator.world.components.contains_key(*name))
        .collect();
    if !placed.is_empty() {
        out.push_str("\ntcsgui:\n  nodes:\n");
        for (name, (x, y)) in placed {
            out.push_str(&format!(
                "    - {{ node: {}, x: {x}, y: {y} }}\n",
                name_scalar(name)
            ));
        }
    }
    out
}

/// The settings that hold a value, in name order.
///
/// # Arguments
///
/// * `said` - An entry's settings.
///
/// # Returns
///
/// Each setting's name and value, trimmed, leaving out any with nothing in
/// it: a setting written empty is one not stated.
fn stated(said: &Settings) -> Vec<(&String, &str)> {
    said.iter()
        .map(|(field, value)| (field, value.trim()))
        .filter(|(_, value)| !value.is_empty())
        .collect()
}

/// Writes the world section, where the world holds anything.
///
/// In the one order a world has: its ambient temperature, then the types of
/// its physics by name, a type's components by name, each with what it
/// states, by name; and the connections by what they join. A blank line
/// between components and between types, as the shipped worlds have.
///
/// For the checking, the same with every setting a component's type states
/// written, and an ambient temperature, as an empty text where nobody has
/// given one, so that a figure the reader requires and is not given is
/// refused at its component, as one that does not read is, rather than as
/// a file that does not parse. The file has no such line: a key with
/// nothing in it is not written.
///
/// # Arguments
///
/// * `out`      - Where the lines are written.
/// * `world`    - The world.
/// * `checking` - Whether this is for the checking rather than the file.
fn world_written(out: &mut String, world: &World, checking: bool) {
    if world.is_empty() {
        return;
    }
    out.push_str("\nworld:\n");
    let ambient = world.ambient_temp.trim();
    if !ambient.is_empty() || checking {
        out.push_str(&format!("  ambient_temp: {}\n", quoted(ambient)));
    }
    if world.components.is_empty() && world.connections.is_empty() {
        return;
    }
    if !ambient.is_empty() {
        out.push('\n');
    }
    out.push_str("  physics:\n");
    let mut typed: BTreeMap<&str, Vec<(&String, &PhysicsComponent)>> =
        BTreeMap::new();
    for (name, component) in &world.components {
        typed
            .entry(component.kind.as_str())
            .or_default()
            .push((name, component));
    }
    if !typed.is_empty() {
        out.push_str("    components:\n");
    }
    for (at, (kind, components)) in typed.iter().enumerate() {
        if at > 0 {
            out.push('\n');
        }
        out.push_str(&format!("      - type {kind}:\n"));
        for (at, (name, component)) in components.iter().enumerate() {
            if at > 0 {
                out.push('\n');
            }
            out.push_str(&format!(
                "          - component: {}\n",
                name_scalar(name)
            ));
            let mut settings: Vec<&str> = if checking {
                tcslibgs::physics::settings_of(kind)
                    .unwrap_or_default()
                    .to_vec()
            } else {
                Vec::new()
            };
            settings.extend(
                stated(&component.settings)
                    .into_iter()
                    .map(|(setting, _)| setting.as_str())
                    .filter(|setting| !settings.contains(setting))
                    .collect::<Vec<_>>(),
            );
            settings.sort_unstable();
            for setting in settings {
                let value = component.settings.get(setting).map(|v| v.trim());
                out.push_str(&format!(
                    "            {setting}: {}\n",
                    quoted(value.unwrap_or_default())
                ));
            }
        }
    }
    if !world.connections.is_empty() {
        if !typed.is_empty() {
            out.push('\n');
        }
        out.push_str("    connections:\n");
        for (from, to) in &world.connections {
            out.push_str(&format!(
                "      - from: {}\n        to: {}\n",
                name_scalar(from),
                name_scalar(to)
            ));
        }
    }
}

/// Writes the settings an entry states, by name: those this build knows and
/// anything else the file carried, which is kept for the reason the window
/// shows it -- a line nobody can see is a line that gets written back out
/// without anybody knowing it is there.
///
/// # Arguments
///
/// * `out`    - Where the lines are written.
/// * `said`   - What the entry says.
/// * `indent` - What each line begins with.
fn entry_settings(out: &mut String, said: &Settings, indent: &str) {
    for (setting, value) in stated(said) {
        out.push_str(&format!("{indent}{setting}: {}\n", self::setting(value)));
    }
}

/// What YAML reads a bare word as, if it reads it as one value.
///
/// # Arguments
///
/// * `word` - The word, as it would follow a key's colon.
///
/// # Returns
///
/// The value it reads as, or `None` where it does not read as a single
/// value: where it is a list, a map, or not YAML at all.
fn bare_reading(word: &str) -> Option<serde_norway::Value> {
    if word.is_empty() || word.trim() != word || word.contains('\n') {
        return None;
    }
    let read: serde_norway::Mapping =
        serde_norway::from_str(&format!("k: {word}\n")).ok()?;
    let value = read.get("k")?.clone();
    match value {
        serde_norway::Value::Sequence(_)
        | serde_norway::Value::Mapping(_)
        | serde_norway::Value::Tagged(_) => None,
        value => Some(value),
    }
}

/// A name, bare where YAML reads it bare as that name, and quoted
/// otherwise: `heater_1` bare, `null` and `0x48` quoted.
///
/// # Arguments
///
/// * `name` - The name.
///
/// # Returns
///
/// The name as YAML.
fn name_scalar(name: &str) -> String {
    match bare_reading(name) {
        Some(serde_norway::Value::String(read)) if read == name => {
            name.to_string()
        }
        _ => quoted(name),
    }
}

/// A setting's value, bare where YAML reads it bare as what was typed, and
/// quoted otherwise.
///
/// Not the rule for a name, and the difference is the reader at the other
/// end. The simulator reads its settings into the types it wants --
/// `simulate` is a boolean, `segment_size` a number and `packet_interval` a
/// length of time -- and a quoted `true` or `1000` is refused as "invalid
/// type: string". So a value that reads bare as a number or a boolean
/// spelled as it was typed goes out bare, as one, and so does one that
/// reads bare as the text typed; anything else is quoted.
///
/// # Arguments
///
/// * `value` - A setting's value as typed.
///
/// # Returns
///
/// The value as YAML.
fn setting(value: &str) -> String {
    use serde_norway::Value;
    let same = match bare_reading(value) {
        Some(Value::String(read)) => read == value,
        Some(Value::Bool(read)) => read.to_string() == value,
        Some(Value::Number(read)) => read.to_string() == value,
        _ => false,
    };
    if same {
        value.to_string()
    } else {
        quoted(value)
    }
}

/// A key and a text value as lines, folded over several where it runs past
/// eighty columns and YAML reads the folded lines as the one text.
///
/// # Arguments
///
/// * `key`   - The key and its colon and space, as `description: `.
/// * `value` - The text.
///
/// # Returns
///
/// The lines, each ending in a newline: the text bare and folded with an
/// indent of two where that reads back as it, bare on one line where only
/// that does, and quoted on one line otherwise.
fn folded(key: &str, value: &str) -> String {
    let one = format!("{key}{}\n", name_scalar(value));
    if one.len() <= 81 || name_scalar(value) != value {
        return one;
    }
    let mut lines: Vec<String> = vec![key.to_string()];
    for word in value.split(' ') {
        let last = lines.last_mut().expect("a line");
        let fresh = last.trim().is_empty() || last.ends_with(": ");
        if fresh {
            last.push_str(word);
        } else if last.len() + 1 + word.len() <= 80 {
            last.push(' ');
            last.push_str(word);
        } else {
            lines.push(format!("  {word}"));
        }
    }
    let text = lines.join("\n") + "\n";
    let reads: Option<serde_norway::Mapping> =
        serde_norway::from_str(&text).ok();
    let key_name = key.trim_end().trim_end_matches(':');
    let same = reads
        .as_ref()
        .and_then(|map| map.get(key_name))
        .and_then(|value| value.as_str())
        == Some(value);
    if same {
        text
    } else {
        one
    }
}

/// A value in double quotes, with its backslashes and double quotes
/// escaped.
///
/// # Arguments
///
/// * `value` - The value.
///
/// # Returns
///
/// It, quoted.
fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What is written is what is read.
    ///
    /// The one thing a file format has to do. A writer and a reader that
    /// disagree leave an editor that loses a setting every time it is opened,
    /// which is a failure nobody sees until a simulated payload stops doing
    /// what it was asked.
    ///
    /// YAML alone, that being what a simulator configuration is: see
    /// `must_be_yaml`.
    #[test]
    fn a_simulator_configuration_survives_a_round_trip() {
        {
            let extension = "yaml";
            let where_ = tempfile::tempdir().expect("a directory");
            let path = where_.path().join(format!("vehiclesim.{extension}"));

            let mut simulator = Simulator::new(&path);
            simulator.description = Some("Simulator settings".to_string());
            simulator.groups.insert(
                "steady_1hz".to_string(),
                [("packet_interval", "1 s"), ("segment_interval", "250 ms")]
                    .into_iter()
                    .map(|(name, value)| (name.to_string(), value.to_string()))
                    .collect(),
            );
            simulator.payloads.insert(
                "payload".to_string(),
                [
                    ("protocol", "udp"),
                    ("group", "steady_1hz"),
                    ("drop_percent", "10"),
                    // Cleared, so it should not come back.
                    ("jitter", ""),
                ]
                .into_iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
            );

            let payloads = [("payload".to_string(), "network".to_string())];
            std::fs::write(&path, to_yaml(&simulator, &payloads))
                .expect("written");
            let read_back = read(&path).expect("read back");
            assert_eq!(read_back.kinds["payload"], "network", "{extension}");

            assert_eq!(
                read_back.description, simulator.description,
                "{extension}"
            );
            // What comes back is what went in: each setting that holds a
            // value, and nothing for one that does not.
            for (name, said) in &simulator.groups {
                let back = read_back.groups.get(name).expect("the group");
                for (setting, value) in said {
                    assert_eq!(
                        back.get(setting),
                        Some(value),
                        "{extension} {setting}"
                    );
                }
            }
            let back = read_back.payloads.get("payload").expect("the payload");
            for (setting, value) in &simulator.payloads["payload"] {
                if value.is_empty() {
                    continue;
                }
                assert_eq!(
                    back.get(setting),
                    Some(value),
                    "{extension} {setting}"
                );
            }
            // But not the one that was cleared, or any nobody has given: a
            // key with nothing after it is not written.
            assert!(!back.contains_key("jitter"), "{extension}");
            assert!(!back.contains_key("segment_size"), "{extension}");
            assert_eq!(back.len(), 3, "{extension}: {back:?}");
        }
    }

    /// The physics model goes back out as it came in, in name order, and
    /// the simulator's own reader takes what the editor wrote; a file that
    /// still names a model of its own is refused and left as it is.
    #[test]
    fn the_physics_model_survives_the_editor() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehiclesim.yaml");
        let given = concat!(
            "tcssim:\n",
            "  ready: \"127.0.0.1:7999\"\n",
            "\n",
            "world:\n",
            "  ambient_temp: \"283.15 K\"\n",
            "  physics:\n",
            "    components:\n",
            "      - type thermal_mass:\n",
            "          - component: zone\n",
            "            time_constant: \"50 s\"\n",
            "            capacity: \"20 J/K\"\n",
            "      - type heater:\n",
            "          - component: heater_1\n",
            "            power: \"4 W\"\n",
            "            voltage: \"28 V\"\n",
            "            ripple: \"0.02\"\n",
            "            ripple_time: \"20 s\"\n",
            "    connections:\n",
            "      - from: heater_1.heat\n",
            "        to: zone.heat\n",
            "\n",
            "simulated_payloads:\n",
            "  - type i2c:\n",
            "      - payload: temp_1\n        simulate: true\n",
            "        connects: zone.temperature\n",
            "  - type gpio:\n",
            "      - payload: heat_1\n        simulate: true\n",
            "        connects: heater_1.on\n",
        );
        std::fs::write(&path, given).expect("written");

        let read_in = read(&path).expect("it reads");
        let zone = &read_in.world.components["zone"];
        assert_eq!(zone.kind, "thermal_mass");
        assert_eq!(zone.settings["capacity"], "20 J/K");
        assert!(read_in
            .world
            .connections
            .contains(&("heater_1.heat".to_string(), "zone.heat".to_string())));

        // Saved, as every other edit saves the whole file, and read again.
        let payloads = [
            ("temp_1".to_string(), "i2c".to_string()),
            ("heat_1".to_string(), "gpio".to_string()),
        ];
        let written = to_yaml(&read_in, &payloads);
        let at = |what: &str| written.find(what).expect(what);
        assert!(at("type heater:") < at("type thermal_mass:"), "{written}");
        assert!(at("capacity:") < at("time_constant:"), "{written}");
        std::fs::write(&path, &written).expect("written");
        let back = read(&path).expect("it reads again");
        assert_eq!(back.world, read_in.world, "{written}");
        assert_eq!(back.world.ambient_temp, "283.15 K");

        // And the simulator's own reader takes what the editor wrote.
        let file = tcslibgs::sim_config::SimConfigFile::load(&path)
            .expect("the simulator reads what the editor wrote");
        let model = file.world.expect("the model");
        assert_eq!(model.physics.components.len(), 2);
        model.resolve().expect("its figures read");

        // A file naming a model of its own is refused, and left alone.
        let old = "physics: vehicle-physics.yaml\n";
        std::fs::write(&path, old).expect("written");
        let Err(Trouble::Unusable(why)) = read(&path) else {
            panic!("a file naming a model was read");
        };
        assert!(why.contains("world:"), "{why}");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), old);
    }

    /// The world's problems are the reader's, each at its component: a
    /// figure without its unit, and one required and not given.
    #[test]
    fn the_worlds_problems_are_the_readers() {
        let mut world = World {
            ambient_temp: "283.15 K".to_string(),
            ..World::default()
        };
        assert!(world.problems().is_empty());
        world.components.insert(
            "heater_1".to_string(),
            PhysicsComponent {
                kind: "heater".to_string(),
                settings: [("power", "4"), ("voltage", "28 V")]
                    .into_iter()
                    .map(|(a, b)| (a.to_string(), b.to_string()))
                    .collect(),
            },
        );
        world.components.insert(
            "zone".to_string(),
            PhysicsComponent {
                kind: "thermal_mass".to_string(),
                settings: Settings::new(),
            },
        );
        let problems = world.problems();
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert_eq!(problems[0].0.as_deref(), Some("heater_1"));
        assert!(problems[0].1.contains("has no unit"), "{problems:?}");
        assert_eq!(problems[1].0.as_deref(), Some("zone"));
    }

    /// Two physics components of one name are refused, and a new one is
    /// given a name nothing has.
    #[test]
    fn physics_names_are_one_each() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehiclesim.yaml");
        std::fs::write(
            &path,
            "world:\n  physics:\n    components:\n      - type bus:\n          \
             - component: b\n      - type load:\n          - component: b\n",
        )
        .expect("written");
        let Err(Trouble::Unusable(why)) = read(&path) else {
            panic!("two of one name were read");
        };
        assert!(why.contains("two physics components"), "{why}");

        let mut world = World::default();
        world
            .components
            .insert("bus_1".to_string(), PhysicsComponent::default());
        assert_eq!(world.fresh_name("bus"), "bus_2");
        assert_eq!(world.fresh_name("load"), "load_1");
    }

    /// A file that still declares its masses itself is refused, told where
    /// they go now, rather than opened and saved without them.
    #[test]
    fn a_file_declaring_masses_itself_is_refused() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehiclesim.yaml");
        std::fs::write(
            &path,
            "thermal_mass:\n  - name: zone\n    environment: \"1\"\n",
        )
        .expect("written");
        let Err(Trouble::Unusable(why)) = read(&path) else {
            panic!("a file declaring masses was read");
        };
        assert!(why.contains("world section"), "{why}");
    }

    /// What the simulator's own reader makes of what the editor writes.
    ///
    /// The editor is permissive and the simulator is not, so the file the
    /// editor writes has to be one the simulator reads -- and with nothing in
    /// `unknown`, which is where a misspelling would land.
    #[test]
    fn the_simulator_reads_what_the_editor_writes() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehiclesim.yaml");

        let mut simulator = Simulator::new(&path);
        simulator.groups.insert(
            "steady_1hz".to_string(),
            [("packet_interval", "1 s")]
                .into_iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
        );
        simulator.payloads.insert(
            "payload".to_string(),
            Simulator::fields(true)
                .into_iter()
                .map(|name| {
                    let value = match name {
                        "group" => "steady_1hz",
                        "protocol" => "udp",
                        "payload_address" => "127.0.0.1",
                        "simulate" => "true",
                        _ => "1",
                    };
                    (name.to_string(), value.to_string())
                })
                .collect(),
        );

        let payloads = [("payload".to_string(), "network".to_string())];
        std::fs::write(&path, to_yaml(&simulator, &payloads)).expect("written");
        let theirs = tcslibgs::sim_config::SimConfigFile::load(&path)
            .expect("the simulator should read what the editor wrote");
        assert_eq!(theirs.simulated_payloads.len(), 1);
        assert!(
            theirs.simulated_payloads[0].unknown.is_empty(),
            "the editor wrote a name the simulator does not know: {:?}",
            theirs.simulated_payloads[0]
                .unknown
                .keys()
                .collect::<Vec<_>>()
        );
        assert!(theirs.simulated_payload_groups[0].unknown.is_empty());
    }

    /// A file that is not there yet opens as nothing.
    #[test]
    fn a_simulator_configuration_that_is_not_there_is_empty() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehiclesim.yaml");
        let simulator = read(&path).expect("a name that does not exist yet");
        assert_eq!(simulator.path, path);
        assert!(simulator.groups.is_empty() && simulator.payloads.is_empty());
    }

    /// A setting this build does not know is kept, not dropped.
    ///
    /// A file from a later build is readable, and a setting nobody can see is
    /// one that gets written back out without anybody knowing it is there.
    #[test]
    fn an_unknown_setting_is_kept() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehiclesim.yaml");
        std::fs::write(
            &path,
            "simulated_payloads:\n  - type spi:\n      \
             - payload: payload\n        \
             simulate: true\n        future_thing: 7\n",
        )
        .expect("a file");

        let simulator = read(&path).expect("read");
        assert_eq!(
            simulator.payloads["payload"]
                .get("future_thing")
                .map(String::as_str),
            Some("7")
        );
        let payloads = [("payload".to_string(), "spi".to_string())];
        assert!(to_yaml(&simulator, &payloads).contains("future_thing"));

        // And an entry the configuration has no payload of goes back out
        // under the type it came with.
        let written = to_yaml(&simulator, &[]);
        assert!(
            written.contains("  - type spi:\n      - payload: payload\n"),
            "{written}"
        );
    }

    /// The editor writes the one order a simulator file has, whatever order
    /// it came to hold things in: types by name, a type's payloads by name,
    /// and an entry's settings by name; and a file in the shape before
    /// payloads were grouped by type is refused and left as it is.
    #[test]
    fn the_editor_writes_in_name_order_and_refuses_the_old_shape() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehiclesim.yaml");
        let mut simulator = Simulator::new(&path);
        for name in ["b", "a", "c"] {
            simulator.payloads.insert(
                name.to_string(),
                [("simulate", "true"), ("packet_interval", "1 s")]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            );
        }
        let payloads = [
            ("c".to_string(), "spi".to_string()),
            ("b".to_string(), "network".to_string()),
            ("a".to_string(), "spi".to_string()),
        ];
        let written = to_yaml(&simulator, &payloads);
        let at = |what: &str| written.find(what).expect(what);
        assert!(at("type network:") < at("type spi:"), "{written}");
        assert!(at("payload: a\n") < at("payload: c\n"), "{written}");
        // A boolean and a time each bare, as a person writes them.
        assert!(written.contains("        simulate: true\n"), "{written}");
        assert!(
            written.contains("        packet_interval: 1 s\n"),
            "{written}"
        );
        assert!(at("packet_interval:") < at("simulate:"), "{written}");
        std::fs::write(&path, &written).expect("written");
        tcslibgs::sim_config::SimConfigFile::load(&path)
            .expect("the simulator reads it");

        let old = "simulated_payloads:\n  - name: a\n    type: spi\n";
        std::fs::write(&path, old).expect("written");
        let Err(Trouble::Unusable(why)) = read(&path) else {
            panic!("a file in the old shape was read");
        };
        assert!(why.contains("convert-format.py"), "{why}");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), old);
    }

    /// Every field a window offers is a setting, and the group is one of them
    /// only for a payload.
    #[test]
    fn a_window_offers_the_settings_and_nothing_else() {
        let payload = Simulator::fields(true);
        let group = Simulator::fields(false);

        assert_eq!(payload.first(), Some(&"group"));
        assert!(!group.contains(&"group"), "a group has no group");
        for setting in tcslibgs::sim_config::SETTINGS {
            assert!(payload.contains(setting), "{setting}");
            assert!(group.contains(setting), "{setting}");
        }
        assert_eq!(payload.len(), group.len() + 1);
        // Never `name`: a payload's name is the payload it stands in for and
        // comes from the configuration, and a group's is how it is chosen.
        assert!(!payload.contains(&"name"));
    }
}
