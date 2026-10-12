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
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    simulated_payload_groups: Vec<Entry>,
    #[serde(default)]
    simulated_payloads: Vec<Entry>,
}

/// Read the simulator configuration at `path`.
///
/// A file that is not there is not an error: a name that does not exist yet is
/// how a new one is begun, and an empty simulator configuration is what that
/// is.
pub fn read(path: &Path) -> Result<Simulator, String> {
    let mut simulator = Simulator::new(path);
    if !path.exists() {
        return Ok(simulator);
    }

    let format = ConfigFormat::of_file(path).map_err(|e| format!("{e}"))?;
    let text = std::fs::read_to_string(path).map_err(|e| format!("{e}"))?;
    let file: File = format.parse(&text).map_err(|e| format!("{e}"))?;

    simulator.description = file.description;
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
    simulator.groups = settled(file.simulated_payload_groups);
    simulator.payloads = settled(file.simulated_payloads);
    Ok(simulator)
}

/// The simulator configuration as YAML.
pub fn to_yaml(simulator: &Simulator) -> String {
    let mut out = String::new();
    out.push_str("version: \"1.0\"\n");
    if let Some(description) = &simulator.description {
        out.push_str(&format!("description: {}\n", quoted(description)));
    }

    for (section, entries) in [
        ("simulated_payload_groups", &simulator.groups),
        ("simulated_payloads", &simulator.payloads),
    ] {
        let entries: Vec<(&String, &Settings)> = entries
            .iter()
            .filter(|(_, said)| said.values().any(|value| !value.trim().is_empty()))
            .collect();
        if entries.is_empty() {
            continue;
        }
        out.push_str(&format!("\n{section}:\n"));
        for (name, said) in entries {
            out.push_str(&format!("  - name: {}\n", quoted(name)));
            for (setting, value) in said {
                // A setting somebody cleared is left out rather than written
                // empty, as everything else the editor writes is: absent is
                // what the simulator's own reader means by "not stated".
                if value.trim().is_empty() {
                    continue;
                }
                out.push_str(&format!("    {setting}: {}\n", quoted(value)));
            }
        }
    }
    out
}

/// The simulator configuration as XML, by the conventions the other files use:
/// every value a child element, and a sequence as repeated siblings.
pub fn to_xml(simulator: &Simulator) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<simulator>\n    <version>1.0</version>\n");
    if let Some(description) = &simulator.description {
        out.push_str(&format!(
            "    <description>{}</description>\n",
            escape(description)
        ));
    }

    for (section, entries) in [
        ("simulated_payload_groups", &simulator.groups),
        ("simulated_payloads", &simulator.payloads),
    ] {
        for (name, said) in entries {
            let stated: Vec<(&String, &String)> = said
                .iter()
                .filter(|(_, value)| !value.trim().is_empty())
                .collect();
            if stated.is_empty() {
                continue;
            }
            out.push_str(&format!("    <{section}>\n"));
            out.push_str(&format!("        <name>{}</name>\n", escape(name)));
            for (setting, value) in stated {
                out.push_str(&format!(
                    "        <{setting}>{}</{setting}>\n",
                    escape(value)
                ));
            }
            out.push_str(&format!("    </{section}>\n"));
        }
    }
    out.push_str("</simulator>\n");
    out
}

/// A YAML scalar, quoted unless it is a plain whole number.
///
/// Not the rule the configuration writer uses, and the difference is the
/// reader at the other end. A graph's attributes are read as `Scalar`, which
/// takes a value however the file spelled it, so that writer quotes everything
/// and a round trip cannot change what a file means. The simulator's settings
/// are read into the types it wants -- `packet_interval_ms` is a `u32` -- and
/// a quoted 1000 is refused as "invalid type: string".
///
/// So a value that is a whole number goes out as one and everything else is
/// quoted. Mechanical rather than a second list of which settings are numeric:
/// a list would be a third statement of the schema, and the one thing it would
/// buy -- writing `protocol: "1"` quoted -- is a value the simulator refuses
/// whichever way it is spelled.
fn quoted(value: &str) -> String {
    if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
        return value.to_string();
    }
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Text as XML character data.
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(simulator: &Simulator, as_xml: bool) -> String {
        if as_xml {
            to_xml(simulator)
        } else {
            to_yaml(simulator)
        }
    }

    /// What is written is what is read, in both spellings.
    ///
    /// The one thing a file format has to do. A writer and a reader that
    /// disagree leave an editor that loses a setting every time it is opened,
    /// which is a failure nobody sees until a simulated payload stops doing
    /// what it was asked.
    #[test]
    fn a_simulator_configuration_survives_a_round_trip() {
        for extension in ["yaml", "xml"] {
            let where_ = tempfile::tempdir().expect("a directory");
            let path = where_.path().join(format!("vehiclesim.{extension}"));

            let mut simulator = Simulator::new(&path);
            simulator.description = Some("Simulator settings".to_string());
            simulator.groups.insert(
                "steady_1hz".to_string(),
                [("packet_interval_ms", "1000"), ("segment_interval_ms", "250")]
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
                    ("jitter_ms", ""),
                ]
                .into_iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
            );

            std::fs::write(&path, written(&simulator, extension == "xml"))
                .expect("written");
            let read_back = read(&path).expect("read back");

            assert_eq!(read_back.description, simulator.description, "{extension}");
            assert_eq!(read_back.groups, simulator.groups, "{extension}");
            // Everything but the setting that was cleared, which is left out
            // of the file as everything else the editor clears is.
            let mut expected = simulator.payloads.clone();
            expected.get_mut("payload").unwrap().remove("jitter_ms");
            assert_eq!(read_back.payloads, expected, "{extension}");
        }
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
            [("packet_interval_ms", "1000")]
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
                        _ => "1",
                    };
                    (name.to_string(), value.to_string())
                })
                .collect(),
        );

        std::fs::write(&path, to_yaml(&simulator)).expect("written");
        let theirs = tcslibgs::sim_config::SimConfigFile::load(&path)
            .expect("the simulator should read what the editor wrote");
        assert_eq!(theirs.simulated_payloads.len(), 1);
        assert!(
            theirs.simulated_payloads[0].unknown.is_empty(),
            "the editor wrote a name the simulator does not know: {:?}",
            theirs.simulated_payloads[0].unknown.keys().collect::<Vec<_>>()
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
            "simulated_payloads:\n  - name: payload\n    future_thing: 7\n",
        )
        .expect("a file");

        let simulator = read(&path).expect("read");
        assert_eq!(
            simulator.payloads["payload"].get("future_thing").map(String::as_str),
            Some("7")
        );
        assert!(to_yaml(&simulator).contains("future_thing"));
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
