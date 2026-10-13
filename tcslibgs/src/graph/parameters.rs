//! What tcspecial itself is configured with, as a graph states it.
//!
//! The `tcspecial` section: where commands are taken, where beacons go, where
//! the telemetry log is written. A graph describes what the vehicle carries and
//! what is done with it, and none of that reaches the ground until these are
//! answered -- so the section is required, and [`crate::graph::Graph::check`]
//! says so about a configuration that has none.
//!
//! The same section a payload set's file holds, and the same struct reads it:
//! [`CIConfigJson`] is the one statement of what the command interpreter needs,
//! and a second one for the version 2 schema would be two statements drifting
//! apart a flight at a time.
//!
//! # Why it is read as text first
//!
//! The section arrives here as the words a file holds, and is turned into
//! [`CIConfigJson`] by [`ci_config_of`] rather than by the deserialiser. That
//! is not ceremony. The editor has to be able to write a configuration that is
//! half filled in -- a port somebody has cleared on the way to typing another
//! -- and a file it cannot read back is a file it has destroyed. Read as text,
//! such a file opens, says what is missing, and can be put right; read
//! strictly, it would not open at all.
//!
//! So the strictness is here, where it can name the attribute and say what is
//! wrong with it, and `Scalar` is what holds a value as the file spelled it --
//! the same arrangement a node's attributes have, and for the same reason.

use std::collections::BTreeMap;

use crate::endpoint_config::Scalar;
use crate::types::{default_log_segment_bytes, CIConfigJson};

/// Every attribute of the section, and whether it must be stated.
///
/// The one list of them. The ground check reads it to refuse a name that
/// belongs to nothing, and the editor reads it to know which fields to offer
/// and which to mark when they are empty -- so a tenth attribute is added here
/// and appears in both.
///
/// Required ones first, so a panel built from this asks for what it must have
/// before what it may.
pub const ATTRIBUTES: &[(&str, bool)] = &[
    ("tcs_cmd_recv", true),
    ("moc_cmd_send", true),
    ("payload_interface", true),
    ("tcs_tc_base", true),
    ("moc_tc_base", true),
    ("beacon_interval_ms", true),
    ("beacon_send", true),
    ("beacon_interface", true),
    ("log_dir", false),
    ("log_segment_bytes", false),
];

/// The section as the command interpreter wants it, or what is wrong with it.
///
/// Every complaint names the attribute, because a configuration is read by
/// somebody who needs to know which of nine words to change.
pub fn ci_config_of(said: &BTreeMap<String, Scalar>) -> Result<CIConfigJson, String> {
    // A name belonging to nothing, first. A misspelled attribute is otherwise
    // a value silently ignored -- `beacon_adress` would leave beacons going
    // wherever the rest of the file happened to say -- and nothing in the
    // telemetry would ever mention it.
    for name in said.keys() {
        if !ATTRIBUTES.iter().any(|(known, _)| known == name) {
            return Err(format!(
                "there is no tcspecial attribute called \"{name}\". They are {}",
                ATTRIBUTES
                    .iter()
                    .map(|(name, _)| *name)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    let text = |name: &str| -> Result<String, String> {
        match said.get(name).map(|value| value.as_str().trim().to_string()) {
            Some(value) if !value.is_empty() => Ok(value),
            _ => Err(format!(
                "{name} is not stated, and it has no default: where commands \
                 are taken and where beacons go are not questions to be \
                 answered by whatever a file was silently given"
            )),
        }
    };
    let number = |name: &str| -> Result<u32, String> {
        let said = text(name)?;
        said.parse::<u32>()
            .map_err(|_| format!("{name} is \"{said}\", which is not a whole number"))
    };

    Ok(CIConfigJson {
        tcs_cmd_recv: text("tcs_cmd_recv")?,
        moc_cmd_send: text("moc_cmd_send")?,
        payload_interface: text("payload_interface")?,
        tcs_tc_base: text("tcs_tc_base")?,
        moc_tc_base: text("moc_tc_base")?,
        beacon_interval_ms: number("beacon_interval_ms")?,
        beacon_send: text("beacon_send")?,
        beacon_interface: text("beacon_interface")?,
        // Absent is "run without a telemetry log", which is a configuration
        // somebody may mean: a vehicle with no writable storage.
        log_dir: said
            .get("log_dir")
            .map(|value| value.as_str().trim().to_string())
            .filter(|value| !value.is_empty()),
        log_segment_bytes: match said.get("log_segment_bytes") {
            Some(value) if !value.as_str().trim().is_empty() => number("log_segment_bytes")?,
            _ => default_log_segment_bytes(),
        },
    })
}

/// The section as a graph would write it, for a configuration being started.
///
/// Every attribute and not one value. An editor that filled in a protocol or a
/// beacon interval on the grounds that they are the same on every vehicle
/// until somebody has a reason would be making a configuration that looks as
/// answered as one somebody thought about -- which is the argument the schema
/// itself makes for requiring these at all.
pub fn blank() -> BTreeMap<String, Scalar> {
    let mut said = BTreeMap::new();
    for (name, _) in ATTRIBUTES {
        said.insert((*name).to_string(), Scalar::of(""));
    }
    said
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(pairs: &[(&str, &str)]) -> BTreeMap<String, Scalar> {
        pairs
            .iter()
            .map(|(name, value)| (name.to_string(), Scalar::of(*value)))
            .collect()
    }

    fn whole() -> Vec<(&'static str, &'static str)> {
        vec![
            ("tcs_cmd_recv", "0.0.0.0:4000"),
            ("moc_cmd_send", "127.0.0.1:4100"),
            ("payload_interface", "0.0.0.0:4001"),
            ("tcs_tc_base", "0.0.0.0:6000"),
            ("moc_tc_base", "127.0.0.1:6100"),
            ("beacon_interval_ms", "5000"),
            ("beacon_send", "239.255.0.1:5550"),
            ("beacon_interface", "127.0.0.1"),
            ("log_dir", "/var/log/tcspecial"),
            ("log_segment_bytes", "65536"),
        ]
    }

    /// A section with everything in it reads as what the command interpreter
    /// wants.
    #[test]
    fn a_whole_section_is_what_the_command_interpreter_takes() {
        let config = ci_config_of(&said(&whole())).expect("this one is complete");
        assert_eq!(config.tcs_cmd_recv, "0.0.0.0:4000");
        assert_eq!(config.moc_cmd_send, "127.0.0.1:4100");
        assert_eq!(config.payload_interface, "0.0.0.0:4001");
        assert_eq!(config.beacon_interval_ms, 5000);
        assert_eq!(config.beacon_send, "239.255.0.1:5550");
        assert_eq!(config.log_dir.as_deref(), Some("/var/log/tcspecial"));
        assert_eq!(config.log_segment_bytes, 65536);

        // And it holds together by the rules of the one piece of code that
        // knows them -- which is also where the one address and the one port
        // come out of the one attribute.
        let settled = config.to_ci_config().expect("and sound");
        assert_eq!(settled.address, "0.0.0.0");
        assert_eq!(settled.port, 4000);
        assert_eq!(settled.moc_cmd_send.to_string(), "127.0.0.1:4100");
        // And the bases, which are counted on by a payload's own id.
        assert_eq!(settled.tcs_tc_base.at(0).unwrap().port, 6000);
        assert_eq!(settled.moc_tc_base.at(3).unwrap().port, 6103);
    }

    /// Every attribute that must be stated is complained about by name.
    ///
    /// Nine attributes and one complaint apiece: "the tcspecial section is
    /// wrong" is a sentence that sends somebody to read all nine.
    #[test]
    fn a_missing_attribute_is_named() {
        for (name, required) in ATTRIBUTES {
            let left_out: Vec<(&str, &str)> = whole()
                .into_iter()
                .filter(|(had, _)| had != name)
                .collect();
            let outcome = ci_config_of(&said(&left_out));
            if *required {
                let why = outcome.expect_err(&format!("{name} is required"));
                assert!(why.contains(name), "{why}");
            } else {
                outcome.unwrap_or_else(|why| panic!("{name} is optional: {why}"));
            }
        }
    }

    /// An attribute left empty is the same as one left out.
    ///
    /// Which is what the editor writes when somebody clears a field, and the
    /// reason the section is read as text at all: the file still opens, and
    /// says what is missing.
    #[test]
    fn an_empty_attribute_is_a_missing_one() {
        let mut cleared = whole();
        cleared.retain(|(name, _)| *name != "tcs_cmd_recv");
        cleared.push(("tcs_cmd_recv", "   "));
        let why = ci_config_of(&said(&cleared))
            .expect_err("an empty command interface is none");
        assert!(why.contains("tcs_cmd_recv"), "{why}");
    }

    /// A command interface is an address and a port, and says so when it is
    /// not.
    ///
    /// One attribute carrying two things has one way to go wrong that two
    /// attributes do not: half of it. So the complaint says what the shape is
    /// rather than only that the value is bad.
    #[test]
    fn a_command_interface_is_an_address_and_a_port() {
        use crate::types::host_port_of;

        let said = |text: &str| host_port_of("tcs_cmd_recv", text);

        let one = said("0.0.0.0:4000").expect("a plain one");
        assert_eq!((one.address.as_str(), one.port), ("0.0.0.0", 4000));
        // Whatever the host is called, the port is after the last colon.
        let named = said(" vehicle.local:4000 ").expect("a named host");
        assert_eq!((named.address.as_str(), named.port), ("vehicle.local", 4000));

        // A base counted on by an id, which is the one arithmetic in the
        // section: a payload's own port is the base plus its position.
        assert_eq!(one.at(0).unwrap().port, 4000);
        assert_eq!(one.at(7).unwrap().port, 4007);
        assert_eq!(one.at(7).unwrap().address, "0.0.0.0");
        // And a sum no port can be is refused while somebody is reading the
        // file rather than when the socket is made.
        let high = said("0.0.0.0:65535").unwrap();
        assert!(high.at(0).is_ok());
        assert!(high.at(1).is_err());

        for (said_, expected) in [
            // No port at all, which is the half-stated case.
            ("0.0.0.0", "address:port"),
            (":4000", "states no address"),
            ("0.0.0.0:", "not a port"),
            ("0.0.0.0:nought", "not a port"),
            ("0.0.0.0:70000", "not a port"),
            // And a port of nought, which asks for whatever is free.
            ("0.0.0.0:0", "port 0"),
        ] {
            let why = said(said_).expect_err(&format!("{said_} should be refused"));
            assert!(why.contains(expected), "{said_} was refused as {why:?}");
            // Every complaint names the attribute, there being four of these
            // in one section.
            assert!(why.contains("tcs_cmd_recv"), "{why}");
        }
    }

    /// A value that is not a number is refused, saying what it was.
    #[test]
    fn a_port_that_is_not_a_port_is_refused() {
        // The numbers this module parses. The four attributes that are a host
        // and a port are parsed where the rest of their rules are -- see
        // CIConfigJson::to_ci_config -- so they are asked below.
        for (name, value) in [
            ("beacon_interval_ms", "often"),
            ("beacon_interval_ms", "-1"),
            ("log_segment_bytes", "a few"),
        ] {
            let mut wrong = whole();
            wrong.retain(|(had, _)| *had != name);
            wrong.push((name, value));
            let why = ci_config_of(&said(&wrong))
                .expect_err(&format!("{name} = {value} should be refused"));
            assert!(why.contains(name), "{why}");
        }

        // And the links, each of which is a host and a port.
        for (name, value, expected) in [
            ("payload_interface", "0.0.0.0:four thousand", "not a port"),
            ("payload_interface", "0.0.0.0:0", "port 0"),
            ("payload_interface", "0.0.0.0:70000", "not a port"),
            ("payload_interface", "0.0.0.0", "address:port"),
            ("moc_cmd_send", "nowhere", "address:port"),
            ("tcs_tc_base", ":6000", "states no address"),
        ] {
            let mut wrong = whole();
            wrong.retain(|(had, _)| *had != name);
            wrong.push((name, value));
            let why = ci_config_of(&said(&wrong))
                .expect("the words are there")
                .to_ci_config()
                .expect_err(&format!("{name} = {value} should be refused"));
            assert!(why.contains(name), "{why}");
            assert!(why.contains(expected), "{name} = {value} said {why:?}");
        }
    }

    /// A name belonging to nothing is refused rather than ignored.
    ///
    /// `beacon_adress` silently ignored is a vehicle whose beacons go
    /// somewhere nobody is listening, and nothing in the telemetry would ever
    /// say so.
    #[test]
    fn a_misspelled_attribute_is_refused_by_name() {
        let mut wrong = whole();
        wrong.push(("beacon_adress", "239.255.0.1:5550"));
        let why = ci_config_of(&said(&wrong)).expect_err("a typo should be refused");
        assert!(why.contains("beacon_adress"), "{why}");
        // And the complaint lists what there is, so the typo can be fixed
        // without opening the manual.
        assert!(why.contains("beacon_send"), "{why}");
    }

    /// What a configuration being started is given, and what it is not.
    #[test]
    fn a_blank_section_asks_for_what_only_the_mission_knows() {
        let blank = blank();
        // Every attribute is there, so the window has a field for each, and
        // not one of them is answered: a value nobody chose looks as chosen as
        // one somebody did.
        for (name, _) in ATTRIBUTES {
            assert!(blank.contains_key(*name), "{name} is not offered");
            assert!(blank[*name].as_str().is_empty(), "{name} was guessed at");
        }
        // Which means a blank one is refused, naming the first thing to fill
        // in.
        assert!(ci_config_of(&blank).is_err());
    }
}
