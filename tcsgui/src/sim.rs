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
//! # Why it is not on the canvas
//!
//! There is nothing to draw. The simulator file adds settings to payloads that
//! are already on the canvas and declares groups of settings for them to
//! share; it introduces no nodes and no connections, so a drawing of it would
//! be the same drawing with numbers on it. It is edited from the menu instead,
//! one window for the payloads and one for the groups, which is what was asked
//! for and is also what the file is: two lists of settings.
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
    /// The thermal masses the file declares, by name.
    ///
    /// Kept so that saving does not drop them. The editor offers no window
    /// for a mass -- what a mass is is three numbers about the world the
    /// payloads sit in, and nothing was asked for -- but a file that declares
    /// one is a file that goes back out with it: an editor that silently
    /// wrote away a section it does not show is an editor that destroys the
    /// file it was given.
    pub masses: BTreeMap<String, Settings>,
    /// The groups of settings, by name.
    pub groups: BTreeMap<String, Settings>,
    /// The simulated payloads, by the name of the payload each stands in for.
    ///
    /// The names are the configuration's: every payload of a configuration
    /// needs settings here, and a name here that no payload has is an error
    /// rather than a line with no effect. So the editor offers one entry per
    /// payload node and never invents a name.
    pub payloads: BTreeMap<String, Settings>,
}

impl Simulator {
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
/// `name` and everything else: the settings are taken as the words they are,
/// so that one nothing recognises is kept rather than thrown away.
#[derive(Debug, Deserialize)]
struct Entry {
    name: String,
    #[serde(flatten)]
    said: BTreeMap<String, Scalar>,
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
    thermal_mass: Vec<Entry>,
    #[serde(default)]
    simulated_payload_groups: Vec<Entry>,
    #[serde(default)]
    simulated_payloads: Vec<Entry>,
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
/// name in a list.
pub fn read(path: &Path) -> Result<Simulator, Trouble> {
    must_be_yaml(path).map_err(Trouble::Unreadable)?;

    let mut simulator = Simulator::new(path);
    if !path.exists() {
        return Ok(simulator);
    }

    let unreadable =
        |e: &dyn std::fmt::Display| Trouble::Unreadable(format!("{e}"));
    let format = ConfigFormat::of_file(path).map_err(|e| unreadable(&e))?;
    let text = std::fs::read_to_string(path).map_err(|e| unreadable(&e))?;
    let file: File = format.parse(&text).map_err(|e| unreadable(&e))?;

    simulator.description = file.description;
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
        ("thermal mass", &file.thermal_mass),
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
    simulator.masses = settled(file.thermal_mass);
    simulator.groups = settled(file.simulated_payload_groups);
    simulator.payloads = settled(file.simulated_payloads);
    Ok(simulator)
}

/// The simulator configuration as YAML.
///
/// A form, not a summary. Every payload the configuration has gets an entry
/// and every entry gets every setting, written empty where nobody has filled
/// it in -- so the file read on its own says what there is to configure
/// instead of saying nothing until something has been typed. The simulator's
/// own reader takes a setting written empty as a setting not stated, which is
/// what makes a half-filled form a file that still loads: see
/// `tcslibgs::sim_config`.
///
/// `payloads` is the configuration's payload nodes, in the order the
/// configuration describes them, because it is the configuration that decides
/// which payloads there are to simulate. An entry the simulator file holds for
/// a name that is not among them is written out too, after them: it is
/// somebody's typing, and `tcsverify` is what says a simulated payload names
/// no payload.
///
/// # Arguments
///
/// * `simulator` - The simulator configuration to write.
/// * `payloads`  - The configuration's payload nodes, in its order; each gets
///                 an entry.
///
/// # Returns
///
/// The file's text.
pub fn to_yaml(simulator: &Simulator, payloads: &[String]) -> String {
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
        out.push_str(&format!("\ndescription: {}\n", quoted(description)));
    }

    // What tcssim itself is told, as the file said it. Not written as a form:
    // a section with nothing in it is one the simulator refuses, and what it
    // must say -- an address no other run uses -- is not the editor's to
    // invent.
    let said: Vec<(&String, &str)> = simulator
        .tcssim
        .iter()
        .map(|(field, value)| (field, value.trim()))
        .filter(|(_, value)| !value.is_empty())
        .collect();
    if !said.is_empty() {
        out.push_str("\ntcssim:\n");
        for (field, value) in said {
            out.push_str(&format!("  {field}: {}\n", quoted(value)));
        }
    }

    // The masses first, because that is the order the file reads in: a mass
    // exists and then payloads attach to it. Not written as a form, as the
    // two lists below are: a mass is three numbers with no defaults, so an
    // entry invented with them blank would be a declaration the simulator
    // refuses. What is written is what the file said.
    if !simulator.masses.is_empty() {
        out.push_str("\nthermal_mass:\n");
        for (name, said) in &simulator.masses {
            out.push_str(&format!("  - name: {}\n", quoted(name)));
            let mut fields: Vec<&str> =
                tcslibgs::sim_config::MASS_SETTINGS.to_vec();
            let theirs: Vec<&str> = said
                .keys()
                .map(String::as_str)
                .filter(|field| !fields.contains(field))
                .collect();
            fields.extend(theirs);
            for field in fields {
                if let Some(value) = said.get(field).map(|value| value.trim()) {
                    if !value.is_empty() {
                        out.push_str(&format!(
                            "    {field}: {}\n",
                            quoted(value)
                        ));
                    }
                }
            }
        }
    }

    // Each payload of the configuration, then whatever the file holds for a
    // name that is not one.
    let mut named: Vec<&String> = payloads.iter().collect();
    named.extend(
        simulator
            .payloads
            .keys()
            .filter(|name| !payloads.contains(name)),
    );

    let empty = Settings::new();
    let groups: Vec<(&String, &Settings)> = simulator.groups.iter().collect();
    let payloads: Vec<(&String, &Settings)> = named
        .into_iter()
        .map(|name| (name, simulator.payloads.get(name).unwrap_or(&empty)))
        .collect();

    for (section, entries, payload) in [
        ("simulated_payload_groups", groups, false),
        ("simulated_payloads", payloads, true),
    ] {
        if entries.is_empty() {
            continue;
        }
        out.push_str(&format!("\n{section}:\n"));
        for (name, said) in entries {
            out.push_str(&format!("  - name: {}\n", quoted(name)));
            // Every setting this kind of entry may state, in the order the
            // window offers them, and then anything the file carried that
            // this build does not know about -- which is kept for the reason
            // the window shows it: a line nobody can see is a line that gets
            // written back out without anybody knowing it is there.
            let mut settings: Vec<&str> = Simulator::fields(payload);
            let theirs: Vec<&str> = said
                .keys()
                .map(String::as_str)
                .filter(|setting| !settings.contains(setting))
                .collect();
            settings.extend(theirs);
            for setting in settings {
                // A setting nobody has answered is written with nothing after
                // the colon rather than with an empty string. The two read the
                // same -- see `blank_is_unstated` -- and they do not say the
                // same: `""` is a value somebody could have typed, and nothing
                // at all is a question not yet answered, which is what the
                // form is for. The name stays in the file either way, because
                // what the form is for is saying which questions there are.
                match said.get(setting).map(|value| value.trim()) {
                    Some(value) if !value.is_empty() => {
                        out.push_str(&format!(
                            "    {setting}: {}\n",
                            quoted(value)
                        ));
                    }
                    _ => out.push_str(&format!("    {setting}:\n")),
                }
            }
        }
    }
    out
}

/// A YAML scalar, quoted unless it is a plain whole number.
///
/// Not the rule the configuration writer uses, and the difference is the
/// reader at the other end. A graph's attributes are read as `Scalar`, which
/// takes a value however the file spelled it, so that writer quotes everything
/// and a round trip cannot change what a file means. The simulator's settings
/// are read into the types it wants -- `packet_interval` is a length of time
/// and `segment_size` a number -- and
/// a quoted 1000 is refused as "invalid type: string".
///
/// So a value that is a whole number goes out as one and everything else is
/// quoted. Mechanical rather than a second list of which settings are numeric:
/// a list would be a third statement of the schema, and the one thing it would
/// buy -- writing `protocol: "1"` quoted -- is a value the simulator refuses
/// whichever way it is spelled.
///
/// # Arguments
///
/// * `value` - A setting's value as typed.
///
/// # Returns
///
/// The value as YAML: bare if it is all digits, otherwise double-quoted with
/// backslashes and quotes escaped.
fn quoted(value: &str) -> String {
    if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
        return value.to_string();
    }
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
                    ("type", "network"),
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

            let payloads = vec!["payload".to_string()];
            std::fs::write(&path, to_yaml(&simulator, &payloads))
                .expect("written");
            let read_back = read(&path).expect("read back");

            assert_eq!(
                read_back.description, simulator.description,
                "{extension}"
            );
            // The file is a form: every setting of every entry is written,
            // empty where nobody has filled it in, so what comes back is what
            // went in plus the blanks.
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
                assert_eq!(
                    back.get(setting),
                    Some(value),
                    "{extension} {setting}"
                );
            }
            // Including the one that was cleared, which comes back empty: the
            // simulator's own reader takes a setting written empty as a
            // setting not stated, so a blank in the form is not a value.
            assert_eq!(back.get("jitter").map(String::as_str), Some(""));
            assert!(
                back.contains_key("segment_size"),
                "{extension}: every setting is offered, filled in or not"
            );
        }
    }

    /// A thermal mass the editor does not show still goes back out.
    ///
    /// The section has no window: what a mass is is three numbers about the
    /// world the payloads sit in, and nobody asked to edit them here. That is
    /// a reason to leave them alone and not a reason to lose them -- an
    /// editor that wrote away a section it does not show is an editor that
    /// destroys the file it was opened on, and the loss would show up as a
    /// simulator that had gone back to sending noise.
    #[test]
    fn a_thermal_mass_survives_the_editor() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehiclesim.yaml");

        // As a simulator file writes it, including a field from a later build
        // that this one knows nothing about.
        let given = concat!(
            "tcssim:\n",
            "  ready: \"127.0.0.1:7999\"\n",
            "\n",
            "thermal_mass:\n",
            "  - name: cabin\n",
            "    environment: \"283.15\"\n",
            "    time_constant: \"50 s\"\n",
            "    stirred: \"yes\"\n",
            "\n",
            "simulated_payloads:\n",
            "  - name: temp_1\n    simulate: true\n",
            "    reads: cabin\n",
        );
        std::fs::write(&path, given).expect("written");

        let read_in = read(&path).expect("it reads");
        let cabin = read_in.masses.get("cabin").expect("the mass");
        assert_eq!(
            cabin.get("environment").map(String::as_str),
            Some("283.15")
        );
        assert_eq!(
            cabin.get("time_constant").map(String::as_str),
            Some("50 s")
        );
        assert_eq!(cabin.get("stirred").map(String::as_str), Some("yes"));

        // Saved, as every other edit saves the whole file, and read again.
        let payloads = vec!["temp_1".to_string()];
        let written = to_yaml(&read_in, &payloads);
        std::fs::write(&path, &written).expect("written");
        let back = read(&path).expect("it reads again");
        assert_eq!(back.masses, read_in.masses, "{written}");
        assert_eq!(
            back.tcssim.get("ready").map(String::as_str),
            Some("127.0.0.1:7999"),
            "{written}"
        );
        assert_eq!(
            back.payloads["temp_1"].get("reads").map(String::as_str),
            Some("cabin"),
            "what a payload is attached to is a setting like any other"
        );

        // And the simulator's own reader takes what the editor wrote. Without
        // the invented field, which this build's simulator refuses by name --
        // being carried through the editor is exactly what lets a file from a
        // later build be opened here and read by the build it was written
        // for.
        let mut this_build = back;
        this_build
            .masses
            .get_mut("cabin")
            .expect("the mass")
            .remove("stirred");
        let plain = where_.path().join("plainsim.yaml");
        this_build.path = plain.clone();
        std::fs::write(&plain, to_yaml(&this_build, &payloads))
            .expect("written");
        tcslibgs::sim_config::SimConfigFile::load(&plain)
            .expect("the simulator reads what the editor wrote");
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
                        "type" => "network",
                        "protocol" => "udp",
                        "payload_address" => "127.0.0.1",
                        "simulate" => "true",
                        _ => "1",
                    };
                    (name.to_string(), value.to_string())
                })
                .collect(),
        );

        let payloads = vec!["payload".to_string()];
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
            "simulated_payloads:\n  - name: payload\n    simulate: \
             true\n    future_thing: 7\n",
        )
        .expect("a file");

        let simulator = read(&path).expect("read");
        assert_eq!(
            simulator.payloads["payload"]
                .get("future_thing")
                .map(String::as_str),
            Some("7")
        );
        let payloads = vec!["payload".to_string()];
        assert!(to_yaml(&simulator, &payloads).contains("future_thing"));
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
