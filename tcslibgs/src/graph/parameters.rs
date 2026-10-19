//! What tcspecial itself is configured with, as a graph states it.
//!
//! The `tcspecial` section: where commands are taken, where beacons go, where
//! the telemetry log is written. A graph describes what the vehicle carries and
//! what is done with it, and none of that reaches the ground until these are
//! answered -- so the section is required, and [`crate::graph::Graph::check`]
//! says so about a configuration that has none.
//!
//! [`CIConfigJson`] is the one statement of what the command interpreter needs,
//! and a second one written for the graph would be two statements drifting
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
use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::endpoint_config::Scalar;
use crate::graph::Graph;
use crate::types::{
    default_log_segment_bytes, BeaconJson, CIConfigJson, LogJson, PayloadCommandJson,
    PayloadTelemetryJson, TcsCmdJson,
};

/// One entry of the section: a value, or a subsection of values.
///
/// The section has subsections because four of the things in it are one thing
/// apiece with more than one attribute to it -- the spacecraft command link,
/// where a payload's telemetry goes, the beacon, the log -- and a prefix
/// repeated on two or three flat names is a subsection somebody declined to
/// write. `beacon.send` says what `beacon_send` said, and says it where the
/// reader is already looking at the beacon.
///
/// Read leniently, like everything else about a graph's section: a subsection
/// is whatever the file wrote under a name, whether or not it belongs there,
/// and [`ci_config_of`] is where it is told so by name.
#[derive(Debug, Clone, PartialEq)]
pub enum Parameter {
    /// A value written against a name at the top of the section.
    Value(Scalar),
    /// A subsection: the values written under one name.
    Group(BTreeMap<String, Scalar>),
}

impl Serialize for Parameter {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Parameter::Value(value) => value.serialize(s),
            Parameter::Group(group) => group.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for Parameter {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Parameter, D::Error> {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = Parameter;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a value, or a subsection of values")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Parameter, E> {
                Ok(Parameter::Value(Scalar::of(v)))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Parameter, E> {
                Ok(Parameter::Value(Scalar::of(v.to_string())))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Parameter, E> {
                Ok(Parameter::Value(Scalar::of(v.to_string())))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Parameter, E> {
                Ok(Parameter::Value(Scalar::of(v.to_string())))
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Parameter, E> {
                Ok(Parameter::Value(Scalar::of(v.to_string())))
            }

            /// A subsection -- or, in XML, an element with text in it.
            ///
            /// quick-xml hands the text of an element as a map of one entry
            /// named `$text`, which is how a value and a subsection arrive in
            /// the same shape there. One entry so named is the value; anything
            /// else is the subsection it looks like.
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Parameter, A::Error> {
                let mut group: BTreeMap<String, Scalar> = BTreeMap::new();
                while let Some((name, value)) = map.next_entry::<String, Scalar>()? {
                    group.insert(name, value);
                }
                match group.len() {
                    1 if group.contains_key("$text") => {
                        Ok(Parameter::Value(group.remove("$text").unwrap()))
                    }
                    _ => Ok(Parameter::Group(group)),
                }
            }
        }

        d.deserialize_any(V)
    }
}

/// The section as one flat set of names, a subsection's written `parent.child`.
///
/// What the rules and the editor work in: a complaint names the attribute the
/// way the manual does, and a field in a window is one name. The file's shape
/// is the file's business, and [`nest`] puts it back.
pub fn flatten(said: &BTreeMap<String, Parameter>) -> BTreeMap<String, Scalar> {
    let mut flat = BTreeMap::new();
    for (name, parameter) in said {
        match parameter {
            Parameter::Value(value) => {
                flat.insert(name.clone(), value.clone());
            }
            Parameter::Group(group) => {
                for (child, value) in group {
                    flat.insert(format!("{name}.{child}"), value.clone());
                }
            }
        }
    }
    flat
}

/// And the other way, for writing a section back out.
///
/// Split at the first dot, so a name with none is a value at the top of the
/// section and `beacon.send` is `send` under `beacon`.
pub fn nest(flat: &BTreeMap<String, Scalar>) -> BTreeMap<String, Parameter> {
    let mut said: BTreeMap<String, Parameter> = BTreeMap::new();
    for (name, value) in flat {
        match name.split_once('.') {
            None => {
                said.insert(name.clone(), Parameter::Value(value.clone()));
            }
            Some((parent, child)) => match said
                .entry(parent.to_string())
                .or_insert_with(|| Parameter::Group(BTreeMap::new()))
            {
                Parameter::Group(group) => {
                    group.insert(child.to_string(), value.clone());
                }
                // A file that wrote a value against a name and then something
                // under it: the value is what it says, and the rules below
                // name whichever of the two does not belong.
                Parameter::Value(_) => {}
            },
        }
    }
    said
}

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
    ("tcs_cmd.recv", true),
    ("tcs_cmd.send", true),
    ("payload_command.source_base", true),
    ("payload_command.dest_base", true),
    ("payload_telemetry.source_base", true),
    ("payload_telemetry.dest_base", true),
    ("beacon.interval", true),
    ("beacon.send", true),
    ("beacon.recv", true),
    ("log.directory", false),
    ("log.segment_size", false),
];

/// What a new configuration is given, where anything.
///
/// Almost nothing, and that is the editor's whole argument about laziness: a
/// configuration that was silently given an answer looks as answered as one
/// somebody thought about, and where commands are taken and where beacons go
/// are not questions to be answered by whatever a file was given.
///
/// The beacon's interval is the exception, and it is one because it is a
/// different kind of question. A socket is a fact about this mission -- there
/// is no value that is right until somebody says so -- where "how often does
/// the vehicle say it is alive" has an answer that is right until somebody has
/// a reason: once a second. A new configuration starts with that and can be
/// argued with.
pub fn default_of(attribute: &str) -> Option<&'static str> {
    match attribute {
        "beacon.interval" => Some("1 s"),
        _ => None,
    }
}

/// An attribute this section used to have, and what to write instead.
///
/// `None` for a name that was never one. Said by name rather than refused as
/// a word nothing knows, because the value has to change with the name: a
/// file that says `interval_ms: 5000` is not a file with a misspelling in it,
/// it is a file written for the spelling before this one.
pub fn retired(attribute: &str) -> Option<&'static str> {
    match attribute {
        "beacon.interval_ms" => {
            Some("say beacon.interval, as a length of time: 5 s rather than 5000")
        }
        _ => None,
    }
}

/// The section as the command interpreter wants it, or what is wrong with it.
///
/// Every attribute of the section is one the interpreter reads: the section
/// says what tcspecial is configured with, a configuration is read when
/// tcspecial starts, and there is nothing in it about building the software.
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
            // One this section used to have is told what to say instead: see
            // `retired`, and the note there about why it is not a
            // misspelling.
            if let Some(instead) = retired(name) {
                return Err(format!("\"{name}\" is no longer an attribute: {instead}"));
            }
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
        match said
            .get(name)
            .map(|value| value.as_str().trim().to_string())
        {
            Some(value) if !value.is_empty() => Ok(value),
            _ => Err(format!(
                "{name} is not stated, and it has no default: where commands \
                 are taken and where beacons go are not questions to be \
                 answered by whatever a file was silently given"
            )),
        }
    };
    // Every integer in these files is written the same way: decimal, or
    // hexadecimal with an `0x` prefix, underscores allowed among the digits.
    // See `crate::whole` -- the beacon's interval is an integer like any
    // other, so `0x3e8` and `1_000` are the thousand milliseconds `1000` is.
    let number = |name: &str| -> Result<u32, String> {
        let said = text(name)?;
        crate::whole::whole_u32(&said).map_err(|why| format!("{name} is {why}"))
    };

    Ok(CIConfigJson {
        tcs_cmd: TcsCmdJson {
            recv: text("tcs_cmd.recv")?,
            send: text("tcs_cmd.send")?,
        },
        payload_command: PayloadCommandJson {
            source_base: text("payload_command.source_base")?,
            dest_base: text("payload_command.dest_base")?,
        },
        payload_telemetry: PayloadTelemetryJson {
            source_base: text("payload_telemetry.source_base")?,
            dest_base: text("payload_telemetry.dest_base")?,
        },
        beacon: BeaconJson {
            // Kept as the file wrote it, a length of time, and turned into
            // the milliseconds the protocol counts where that is needed --
            // `CIConfigJson::to_ci_config`, which does it for either kind of
            // configuration. Read here all the same, so a length of time that
            // is not one is refused where the section is rather than at
            // whatever later moment something asks how often beacons go.
            interval: {
                let said = text("beacon.interval")?;
                crate::graph::parse_interval(&said)
                    .map_err(|why| format!("beacon.interval: {why}"))?;
                said
            },
            // The name `interval` used to have, which a graph never writes:
            // this section's words are checked against `ATTRIBUTES`, which
            // names the retired one there.
            interval_ms: None,
            send: text("beacon.send")?,
            recv: text("beacon.recv")?,
        },
        log: LogJson {
            // Absent is "run without a telemetry log", which is a
            // configuration somebody may mean: a vehicle with no writable
            // storage.
            directory: said
                .get("log.directory")
                .map(|value| value.as_str().trim().to_string())
                .filter(|value| !value.is_empty()),
            segment_size: match said.get("log.segment_size") {
                Some(value) if !value.as_str().trim().is_empty() => number("log.segment_size")?,
                _ => default_log_segment_bytes(),
            },
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
/// Where a configuration says its beacons are: both ends of the link.
///
/// The `tcspecial` section's `beacon.send` and `beacon.recv`. `None` for a
/// configuration with no section, which sends no beacons -- the section is
/// where the two addresses are written down.
///
/// Here rather than in each ground program because every ground program needs
/// it and the answer is the file's: tcsmoc reads it to put a beacon panel in
/// its window, the `beacon` example reads it to show the same panel, and a
/// station that worked the pair out for itself would be a second reading of
/// one file.
pub fn beacon_of(graph: &Graph) -> Result<Option<crate::BeaconLink>, String> {
    let Some(section) = graph.tcspecial.as_ref() else {
        return Ok(None);
    };
    let ci = ci_config_of(&flatten(section))?.to_ci_config()?;
    Ok(Some(crate::BeaconLink::of(&ci)))
}

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
            ("tcs_cmd.recv", "0.0.0.0:4000"),
            ("tcs_cmd.send", "127.0.0.1:4100"),
            ("payload_command.source_base", "127.0.0.1:4300"),
            ("payload_command.dest_base", "0.0.0.0:4200"),
            ("payload_telemetry.source_base", "0.0.0.0:6000"),
            ("payload_telemetry.dest_base", "127.0.0.1:6100"),
            ("beacon.interval", "5 s"),
            ("beacon.send", "0.0.0.0:5550"),
            ("beacon.recv", "127.0.0.1:5650"),
            ("log.directory", "/var/log/tcspecial"),
            ("log.segment_size", "65536"),
        ]
    }

    /// A section with everything in it reads as what the command interpreter
    /// wants.
    #[test]
    fn a_whole_section_is_what_the_command_interpreter_takes() {
        let config = ci_config_of(&said(&whole())).expect("this one is complete");
        assert_eq!(config.tcs_cmd.recv, "0.0.0.0:4000");
        assert_eq!(config.tcs_cmd.send, "127.0.0.1:4100");
        assert_eq!(config.payload_command.source_base, "127.0.0.1:4300");
        assert_eq!(config.payload_command.dest_base, "0.0.0.0:4200");
        assert_eq!(config.beacon.interval, "5 s");
        assert_eq!(config.beacon.send, "0.0.0.0:5550");
        assert_eq!(config.log.directory.as_deref(), Some("/var/log/tcspecial"));
        assert_eq!(config.log.segment_size, 65536);

        // And it holds together by the rules of the one piece of code that
        // knows them -- which is also where the one address and the one port
        // come out of the one attribute.
        let settled = config.to_ci_config().expect("and sound");
        assert_eq!(settled.address, "0.0.0.0");
        assert_eq!(settled.port, 4000);
        assert_eq!(settled.moc_cmd_send.to_string(), "127.0.0.1:4100");
        // And the bases, which are counted on by a payload's own id.
        assert_eq!(settled.payload_source_base.at(0).unwrap().port, 6000);
        assert_eq!(settled.payload_dest_base.at(3).unwrap().port, 6103);
    }

    /// Every attribute that must be stated is complained about by name.
    ///
    /// Nine attributes and one complaint apiece: "the tcspecial section is
    /// wrong" is a sentence that sends somebody to read all nine.
    #[test]
    fn a_missing_attribute_is_named() {
        for (name, required) in ATTRIBUTES {
            let left_out: Vec<(&str, &str)> =
                whole().into_iter().filter(|(had, _)| had != name).collect();
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
        cleared.retain(|(name, _)| *name != "tcs_cmd.recv");
        cleared.push(("tcs_cmd.recv", "   "));
        let why = ci_config_of(&said(&cleared)).expect_err("an empty command interface is none");
        assert!(why.contains("tcs_cmd.recv"), "{why}");
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

        let said = |text: &str| host_port_of("tcs_cmd.recv", text);

        let one = said("0.0.0.0:4000").expect("a plain one");
        assert_eq!((one.address.as_str(), one.port), ("0.0.0.0", 4000));
        // Whatever the host is called, the port is after the last colon.
        let named = said(" vehicle.local:4000 ").expect("a named host");
        assert_eq!(
            (named.address.as_str(), named.port),
            ("vehicle.local", 4000)
        );

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
            assert!(why.contains("tcs_cmd.recv"), "{why}");
        }
    }

    /// A value that is not a number is refused, saying what it was.
    #[test]
    fn a_port_that_is_not_a_port_is_refused() {
        // The numbers this module parses. The four attributes that are a host
        // and a port are parsed where the rest of their rules are -- see
        // CIConfigJson::to_ci_config -- so they are asked below.
        for (name, value) in [
            ("beacon.interval", "often"),
            ("beacon.interval", "-1"),
            ("log.segment_size", "a few"),
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
            (
                "payload_command.dest_base",
                "0.0.0.0:four thousand",
                "not a port",
            ),
            ("payload_command.dest_base", "0.0.0.0:0", "port 0"),
            ("payload_command.source_base", "0.0.0.0:70000", "not a port"),
            ("payload_command.source_base", "0.0.0.0", "address:port"),
            ("tcs_cmd.send", "nowhere", "address:port"),
            (
                "payload_telemetry.source_base",
                ":6000",
                "states no address",
            ),
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
        wrong.push(("beacon_adress", "0.0.0.0:5550"));
        let why = ci_config_of(&said(&wrong)).expect_err("a typo should be refused");
        assert!(why.contains("beacon_adress"), "{why}");
        // And the complaint lists what there is, so the typo can be fixed
        // without opening the manual.
        assert!(why.contains("beacon.send"), "{why}");
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
