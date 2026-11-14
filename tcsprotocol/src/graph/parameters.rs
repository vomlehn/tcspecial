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
use crate::gateway::GatewayKind;
use crate::graph::Graph;
use crate::types::{
    default_log_segment_bytes, BeaconJson, CIConfigJson, CfeTimeJson, LogJson,
    MsgIdRangeJson, PayloadCommandJson, PayloadTelemetryJson, SbnJson,
    TcsCmdJson,
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
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Parameter, A::Error> {
                let mut group: BTreeMap<String, Scalar> = BTreeMap::new();
                while let Some((name, value)) =
                    map.next_entry::<String, Scalar>()?
                {
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

/// Flattens the section into one set of names, a subsection's written
/// `parent.child`.
///
/// What the rules and the editor work in: a complaint names the attribute the
/// way the manual does, and a field in a window is one name. The file's shape
/// is the file's business, and [`nest`] puts it back.
///
/// # Arguments
///
/// * `said` - The section as the file nests it.
///
/// # Returns
///
/// Every value in it, by its flat name.
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

/// Nests a flat section again, for writing it back out: [`flatten`] the
/// other way.
///
/// Split at the first dot, so a name with none is a value at the top of the
/// section and `beacon.send` is `send` under `beacon`.
///
/// # Arguments
///
/// * `flat` - Every value, by its flat name.
///
/// # Returns
///
/// The section as a file nests it.
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

/// Which configurations an attribute belongs to, and whether it must be
/// stated in them.
///
/// The gateway kind decides how the links are written -- UDP sockets for the
/// radio, Yamcs and F´, software bus message IDs for cFS -- so an attribute of
/// the one is required of a configuration of that kind and refused, by name, in
/// one of the other: see "One kind per configuration" in
/// `docs/tcspecial-design.rst`. Yamcs adds the APIDs its CCSDS headers carry,
/// which only it states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Needed {
    /// Required of every configuration.
    Always,
    /// Optional in every configuration.
    Optional,
    /// Required where the links are UDP sockets, and refused where they are
    /// not.
    Sockets,
    /// Required where the links are software bus message IDs, and refused
    /// where they are not.
    Bus,
    /// Allowed only where the links are software bus message IDs, and
    /// required or refused there by a rule of its own:
    /// `cfe_time.leap_seconds`, stated on the TAI scale and not on UTC.
    BusByRule,
    /// Required where every packet is a CCSDS Space Packet -- the gateway is
    /// Yamcs -- and refused where it is not: the APIDs.
    Framed,
}

impl Needed {
    /// Whether an attribute so needed may be stated in a configuration that
    /// answers to `kind`.
    ///
    /// # Arguments
    ///
    /// * `kind` - The kind of gateway the configuration answers to.
    ///
    /// # Returns
    ///
    /// `true` where it belongs.
    pub fn belongs(self, kind: GatewayKind) -> bool {
        match self {
            Needed::Always | Needed::Optional => true,
            Needed::Sockets => !kind.on_bus(),
            Needed::Bus | Needed::BusByRule => kind.on_bus(),
            Needed::Framed => kind.framed(),
        }
    }

    /// Whether an attribute so needed must be stated in a configuration that
    /// answers to `kind`, for a window to mark.
    ///
    /// # Arguments
    ///
    /// * `kind` - The kind of gateway the configuration answers to, or
    ///            `None` where it does not yet say: only what every
    ///            configuration needs is required of that.
    ///
    /// # Returns
    ///
    /// `true` where it must be stated.
    pub fn required(self, kind: Option<GatewayKind>) -> bool {
        match (self, kind) {
            (Needed::Always, _) => true,
            (Needed::Sockets, Some(kind)) => !kind.on_bus(),
            (Needed::Bus, Some(kind)) => kind.on_bus(),
            (Needed::Framed, Some(kind)) => kind.framed(),
            _ => false,
        }
    }
}

/// Every attribute of the section, and which configurations need it.
///
/// The one list of them. The ground check reads it to refuse a name that
/// belongs to nothing, or to another kind of gateway, and the editor reads it
/// to know which fields to offer and which to mark when they are empty -- so
/// an attribute is added here and appears in both.
///
/// What every configuration must state first, then the radio's, Yamcs's and
/// F´'s, then Yamcs's APIDs, then cFS's, so a panel built from this asks for
/// what it must have before what it may.
pub const ATTRIBUTES: &[(&str, Needed)] = &[
    ("gateway", Needed::Always),
    ("interface", Needed::Always),
    ("beacon.default_interval", Needed::Always),
    ("beacon.reset_interval", Needed::Always),
    ("state_file", Needed::Always),
    ("tcs_cmd.recv", Needed::Sockets),
    ("tcs_cmd.send", Needed::Sockets),
    ("payload_command.source_base", Needed::Sockets),
    ("payload_command.dest_base", Needed::Sockets),
    ("payload_telemetry.source_base", Needed::Sockets),
    ("payload_telemetry.dest_base", Needed::Sockets),
    ("beacon.send", Needed::Sockets),
    ("beacon.recv", Needed::Sockets),
    ("beacon.apid", Needed::Framed),
    ("tcs_cmd.apid", Needed::Framed),
    ("payload_telemetry.apid_base", Needed::Framed),
    ("sbn.release", Needed::Bus),
    ("sbn.ident", Needed::Bus),
    ("sbn.spacecraft_id", Needed::Bus),
    ("sbn.processor_id", Needed::Bus),
    ("sbn.local", Needed::Bus),
    ("sbn.peer", Needed::Bus),
    ("sbn.peer_processor_id", Needed::Bus),
    ("sbn.max_message", Needed::Bus),
    ("msgid_range.first", Needed::Bus),
    ("msgid_range.last", Needed::Bus),
    ("tcs_cmd.cmd_msgid", Needed::Bus),
    ("tcs_cmd.response_msgid", Needed::Bus),
    ("payload_command.cmd_base", Needed::Bus),
    ("payload_command.response_base", Needed::Bus),
    ("payload_telemetry.tlm_base", Needed::Bus),
    ("beacon.tlm_msgid", Needed::Bus),
    ("cfe_time.epoch", Needed::Bus),
    ("cfe_time.scale", Needed::Bus),
    ("cfe_time.leap_seconds", Needed::BusByRule),
    ("log.directory", Needed::Optional),
    ("log.segment_size", Needed::Optional),
    ("restart_arm_valid", Needed::Optional),
    ("configurations", Needed::Optional),
    ("configurations_kept", Needed::Optional),
    ("load_trials", Needed::Optional),
];

/// Says which configurations an attribute of the section belongs to.
///
/// # Arguments
///
/// * `attribute` - The attribute's flat name.
///
/// # Returns
///
/// How it is needed, or `None` for a name the section does not have.
pub fn needed(attribute: &str) -> Option<Needed> {
    ATTRIBUTES
        .iter()
        .find(|(name, _)| *name == attribute)
        .map(|(_, needed)| *needed)
}

/// Gives what a new configuration is given for an attribute, where anything.
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
///
/// # Arguments
///
/// * `attribute` - The attribute's flat name.
///
/// # Returns
///
/// Its value in a new configuration, or `None` for one that starts blank.
pub fn default_of(attribute: &str) -> Option<&'static str> {
    match attribute {
        "beacon.default_interval" => Some("1 s"),
        // The same once a second, for the same reason: it may be no longer
        // than the interval, and a new configuration's must be allowed.
        "beacon.reset_interval" => Some("1 s"),
        _ => None,
    }
}

/// Says what to write instead of an attribute this section used to have.
///
/// Said by name rather than refused as a word nothing knows, because the
/// value has to change with the name: a file that says `interval_ms: 5000`
/// is not a file with a misspelling in it, it is a file written for the
/// spelling before this one.
///
/// # Arguments
///
/// * `attribute` - The attribute's flat name.
///
/// # Returns
///
/// What to write instead, or `None` for a name that was never one.
pub fn retired(attribute: &str) -> Option<&'static str> {
    match attribute {
        "beacon.interval" => Some(
            "say beacon.default_interval: there are two intervals now, the \
             other being beacon.reset_interval",
        ),
        "beacon.interval_ms" => Some(
            "say beacon.default_interval, as a length of time: 5 s rather \
             than 5000",
        ),
        _ => None,
    }
}

/// Reads the section as the command interpreter wants it.
///
/// Every attribute of the section is one the interpreter reads: the section
/// says what tcspecial is configured with, a configuration is read when
/// tcspecial starts, and there is nothing in it about building the software.
///
/// Every complaint names the attribute, because a configuration is read by
/// somebody who needs to know which of ten words to change.
///
/// # Arguments
///
/// * `said` - The section, flattened.
///
/// # Returns
///
/// The section, every value as the file wrote it.
///
/// # Errors
///
/// A message naming the attribute for one the section does not have, one
/// that is required and not stated, a gateway [`GatewayKind::of`] refuses,
/// an interval that is not a length of time, and a segment size that is not
/// a whole number.
///
/// [`GatewayKind::of`]: crate::gateway::GatewayKind::of
pub fn ci_config_of(
    said: &BTreeMap<String, Scalar>,
) -> Result<CIConfigJson, String> {
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
                return Err(format!(
                    "\"{name}\" is no longer an attribute: {instead}"
                ));
            }
            // And one written with hyphens for underscores, or the other way
            // round, is told its one spelling rather than handed the list to
            // find it in: see `crate::graph::spelled`.
            let known = |other: &str| {
                ATTRIBUTES.iter().any(|(known, _)| *known == other)
            };
            if let Some(hint) = crate::graph::spelled(name, known) {
                return Err(format!(
                    "there is no tcspecial attribute called \"{name}\": {hint}"
                ));
            }
            return Err(format!(
                "there is no tcspecial attribute called \"{name}\". They are \
                 {}",
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

    // First, because it decides how the rest is written: a configuration
    // that answers to a gateway this build cannot is refused here, by name,
    // before anything is said about its links.
    let gateway = GatewayKind::of(&text("gateway")?)?;

    // Then every link attribute that is another kind's. A `recv` in a
    // configuration that answers to cFS is an address somebody expects to
    // be listened on, and nothing will be, so it is refused by name rather
    // than ignored.
    for (name, value) in said {
        if value.as_str().trim().is_empty() {
            continue;
        }
        if let Some(needed) = needed(name) {
            if !needed.belongs(gateway) {
                return Err(misplaced(name, gateway));
            }
        }
    }

    // An attribute of this kind's links, which every configuration of the
    // kind states, and `None` for the other kind's, which it cannot: any
    // that was written has been refused above.
    let link = |name: &str| -> Result<Option<String>, String> {
        match needed(name) {
            Some(needed) if needed.required(Some(gateway)) => {
                text(name).map(Some)
            }
            _ => Ok(None),
        }
    };
    let stated = |name: &str| {
        said.get(name)
            .map(|value| value.as_str().trim().to_string())
            .filter(|value| !value.is_empty())
    };

    Ok(CIConfigJson {
        gateway,
        // Second, because a configuration written for another interface is
        // one whose commands this build would read with other layouts, and
        // nothing else it says is worth reading until that is settled.
        interface: interface_of(&text("interface")?)?,
        tcs_cmd: TcsCmdJson {
            recv: link("tcs_cmd.recv")?,
            send: link("tcs_cmd.send")?,
            cmd_msgid: link("tcs_cmd.cmd_msgid")?,
            response_msgid: link("tcs_cmd.response_msgid")?,
            apid: link("tcs_cmd.apid")?,
        },
        payload_command: PayloadCommandJson {
            source_base: link("payload_command.source_base")?,
            dest_base: link("payload_command.dest_base")?,
            cmd_base: link("payload_command.cmd_base")?,
            response_base: link("payload_command.response_base")?,
        },
        payload_telemetry: PayloadTelemetryJson {
            source_base: link("payload_telemetry.source_base")?,
            dest_base: link("payload_telemetry.dest_base")?,
            tlm_base: link("payload_telemetry.tlm_base")?,
            apid_base: link("payload_telemetry.apid_base")?,
        },
        // The peer, the allotment and the clock, which only a configuration
        // that answers to cFS has, and which it must state whole.
        sbn: match gateway.on_bus() {
            true => Some(SbnJson {
                release: text("sbn.release")?,
                ident: text("sbn.ident")?,
                spacecraft_id: text("sbn.spacecraft_id")?,
                processor_id: text("sbn.processor_id")?,
                local: text("sbn.local")?,
                peer: text("sbn.peer")?,
                peer_processor_id: text("sbn.peer_processor_id")?,
                max_message: text("sbn.max_message")?,
            }),
            false => None,
        },
        msgid_range: match gateway.on_bus() {
            true => Some(MsgIdRangeJson {
                first: text("msgid_range.first")?,
                last: text("msgid_range.last")?,
            }),
            false => None,
        },
        cfe_time: match gateway.on_bus() {
            true => Some(CfeTimeJson {
                epoch: text("cfe_time.epoch")?,
                scale: text("cfe_time.scale")?,
                leap_seconds: stated("cfe_time.leap_seconds"),
            }),
            false => None,
        },
        beacon: BeaconJson {
            // Kept as the file wrote it, a length of time, and turned into
            // the milliseconds the protocol counts where that is needed --
            // `CIConfigJson::to_ci_config`, which does it for either kind of
            // configuration. Read here all the same, so a length of time that
            // is not one is refused where the section is rather than at
            // whatever later moment something asks how often beacons go.
            default_interval: {
                let said = text("beacon.default_interval")?;
                crate::graph::parse_interval(&said)
                    .map_err(|why| format!("beacon.default_interval: {why}"))?;
                said
            },
            // The name `interval` used to have, which a graph never writes:
            // this section's words are checked against `ATTRIBUTES`, which
            // names the retired one there.
            interval: None,
            interval_ms: None,
            // Read here, as the interval is, so that one that is not a length
            // of time is refused where the section is. Whether it is longer
            // than the interval is `to_ci_config`'s question, which has both.
            reset_interval: {
                let said = text("beacon.reset_interval")?;
                crate::graph::parse_interval(&said)
                    .map_err(|why| format!("beacon.reset_interval: {why}"))?;
                said
            },
            send: link("beacon.send")?,
            recv: link("beacon.recv")?,
            tlm_msgid: link("beacon.tlm_msgid")?,
            apid: link("beacon.apid")?,
        },
        // Absent is the twenty minutes an arm has always lasted. Read here
        // all the same, as the beacon's interval is, so that a length of time
        // that is not one is refused where the section is.
        restart_arm_valid: match said.get("restart_arm_valid") {
            Some(value) if !value.as_str().trim().is_empty() => {
                let said = value.as_str().trim().to_string();
                crate::types::restart_arm_valid_of(&said)?;
                Some(said)
            }
            _ => None,
        },
        state_file: text("state_file")?,
        // Absent is a vehicle that keeps no configurations of its own: it
        // runs the one on its command line, and a RESTART may name only
        // that and the one running. A vehicle with no writable storage is
        // a configuration somebody may mean, as a missing log.directory is.
        configurations: said
            .get("configurations")
            .map(|value| value.as_str().trim().to_string())
            .filter(|value| !value.is_empty()),
        configurations_kept: match said.get("configurations_kept") {
            Some(value) if !value.as_str().trim().is_empty() => {
                Some(at_least_one("configurations_kept", &number)?)
            }
            _ => None,
        },
        load_trials: match said.get("load_trials") {
            Some(value) if !value.as_str().trim().is_empty() => {
                Some(at_least_one("load_trials", &number)?)
            }
            _ => None,
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
                Some(value) if !value.as_str().trim().is_empty() => {
                    number("log.segment_size")?
                }
                _ => default_log_segment_bytes(),
            },
        },
    })
}

/// Says why an attribute of one kind's links is refused in a configuration
/// that answers to another, and what that kind states instead.
///
/// # Arguments
///
/// * `name` - The attribute's flat name.
/// * `kind` - The kind the configuration answers to.
///
/// # Returns
///
/// The complaint.
fn misplaced(name: &str, kind: GatewayKind) -> String {
    // An APID, which only a configuration whose packets carry CCSDS headers
    // has: the links are sockets either way, so nothing else is offered.
    if needed(name) == Some(Needed::Framed) {
        return format!(
            "{name} is said only where the gateway is yamcs, whose packets \
             carry CCSDS headers, and this configuration's is {kind}: take it \
             out"
        );
    }
    let what = if kind.on_bus() {
        format!(
            "{name} is a UDP socket's, and this configuration's gateway is \
             {kind}, whose links are software bus message IDs: take it out"
        )
    } else {
        format!(
            "{name} is said only where the gateway is cfs, and this \
             configuration's is {kind}, whose links are UDP sockets: take it \
             out"
        )
    };
    // What the same subsection says for this kind, where it says anything.
    let parent = name.split_once('.').map(|(parent, _)| parent);
    let instead: Vec<&str> = ATTRIBUTES
        .iter()
        .filter(|(other, needed)| {
            parent.is_some()
                && other.split_once('.').map(|(p, _)| p) == parent
                && matches!(needed, Needed::Sockets | Needed::Bus)
                && needed.belongs(kind)
        })
        .map(|(other, _)| *other)
        .collect();
    match instead.is_empty() {
        true => what,
        false => format!("{what}, and state {} instead", instead.join(" and ")),
    }
}

/// Reads a count the section states that may not be nought.
///
/// # Arguments
///
/// * `name`   - The attribute's flat name.
/// * `number` - How the section reads a whole number of it.
///
/// # Returns
///
/// The count.
///
/// # Errors
///
/// What `number` refuses, and nought, naming the attribute and why.
fn at_least_one(
    name: &str,
    number: &dyn Fn(&str) -> Result<u32, String>,
) -> Result<u32, String> {
    match number(name)? {
        0 => Err(format!(
            "{name} is 0: it counts configurations, and nought of them would \
             be a vehicle that keeps none or never tries one, so make it 1 or \
             more, or leave it out for its default"
        )),
        count => Ok(count),
    }
}

/// Reads the section's `interface`, and refuses one that is not this build's.
///
/// The one place the rule is stated. tcspecial, tcs-verify, tcs-moc, tcsmoc
/// and the C/C++ library all read the section through [`ci_config_of`], so
/// all of them refuse a configuration written for another interface, saying
/// both digests: a build can speak only the interface it was compiled from,
/// and a configuration whose ordinary commands it read with other layouts
/// would be one whose every command meant something else.
///
/// # Arguments
///
/// * `said` - The attribute, as the file wrote it.
///
/// # Returns
///
/// The interface it names, which is this build's.
///
/// # Errors
///
/// A message naming the attribute for one that is not 32 lower-case hex
/// digits, and for one that is another interface's, saying what to write.
pub fn interface_of(said: &str) -> Result<crate::InterfaceDigest, String> {
    let ours = crate::InterfaceDigest::of_this_build();
    let named = crate::InterfaceDigest::of_hex(said).ok_or_else(|| {
        format!(
            "interface is \"{said}\", which is not an interface digest: \
             write the 32 lower-case hex digits `tcs-verify \
             --interface-digest` prints, which for this build are {ours}"
        )
    })?;
    if named != ours {
        return Err(format!(
            "interface is {named}, and this build speaks interface {ours}, \
             the digest of the tcsprotocol/xtce/link.xtce and types.xtce \
             it was built from: \
             fly this configuration with a build of interface {named}, or, \
             once its commands have been checked against this build's, write \
             interface: \"{ours}\""
        ));
    }
    Ok(named)
}

/// Finds where a configuration says its beacons are: both ends of the link,
/// the `tcspecial` section's `beacon.send` and `beacon.recv`.
///
/// Here rather than in each ground program because every ground program needs
/// it and the answer is the file's: tcs-moc reads it to put a beacon panel in
/// its window, the `beacon` example reads it to show the same panel, and a
/// station that worked the pair out for itself would be a second reading of
/// one file.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// Both ends of the beacon link. `None` for a configuration with no
/// section, which sends no beacons -- the section is where the two
/// addresses are written down -- and for one that answers to cFS, whose
/// beacon is published on the software bus and has no socket.
///
/// # Errors
///
/// What [`ci_config_of`] and the conversion to the interpreter's settings
/// refuse in the section.
pub fn beacon_of(graph: &Graph) -> Result<Option<crate::BeaconLink>, String> {
    let Some(section) = graph.tcspecial.as_ref() else {
        return Ok(None);
    };
    let ci = ci_config_of(&flatten(section))?.to_ci_config()?;
    Ok(crate::BeaconLink::of(&ci))
}

/// Finds how often tcspecial beacons: the `tcspecial` section's
/// `beacon.default_interval`, as a length of time.
///
/// What a ground station ages each beacon against -- see
/// `tcsmoc::beacon::Schedule::for_interval` -- so it is read here, from the
/// same section the link is, rather than assumed: a station that took every
/// vehicle to beacon every five seconds would call a ten-second beacon late
/// when it was on time.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// The interval. `None` for a configuration with no section, as
/// [`beacon_of`] is.
///
/// # Errors
///
/// What [`ci_config_of`] and the conversion to the interpreter's settings
/// refuse in the section.
pub fn beacon_interval_of(
    graph: &Graph,
) -> Result<Option<std::time::Duration>, String> {
    let Some(section) = graph.tcspecial.as_ref() else {
        return Ok(None);
    };
    Ok(Some(
        ci_config_of(&flatten(section))?
            .to_ci_config()?
            .beacon_interval,
    ))
}

/// Makes the section as a graph would write it, for a configuration being
/// started.
///
/// Every attribute and not one value. An editor that filled in a protocol or a
/// beacon interval on the grounds that they are the same on every vehicle
/// until somebody has a reason would be making a configuration that looks as
/// answered as one somebody thought about -- which is the argument the schema
/// itself makes for requiring these at all.
///
/// # Returns
///
/// Every attribute, by flat name, each empty.
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
    use std::time::Duration;

    /// Makes a flattened section as a file states it.
    ///
    /// # Arguments
    ///
    /// * `pairs` - Each attribute's flat name, with its value as written.
    ///
    /// # Returns
    ///
    /// The section.
    fn said(pairs: &[(&str, &str)]) -> BTreeMap<String, Scalar> {
        pairs
            .iter()
            .map(|(name, value)| (name.to_string(), Scalar::of(*value)))
            .collect()
    }

    /// Gives a section that states every attribute.
    ///
    /// # Returns
    ///
    /// Each attribute's flat name, with a value that reads.
    fn whole() -> Vec<(&'static str, &'static str)> {
        vec![
            ("gateway", "radio"),
            ("interface", "c7820f9bfe585feab72247ece0a2261d"),
            ("tcs_cmd.recv", "0.0.0.0:4000"),
            ("tcs_cmd.send", "127.0.0.1:4100"),
            ("payload_command.source_base", "127.0.0.1:4300"),
            ("payload_command.dest_base", "0.0.0.0:4200"),
            ("payload_telemetry.source_base", "0.0.0.0:6000"),
            ("payload_telemetry.dest_base", "127.0.0.1:6100"),
            ("beacon.default_interval", "5 s"),
            ("beacon.reset_interval", "1 s"),
            ("state_file", "/var/lib/tcspecial/state"),
            ("beacon.send", "0.0.0.0:5550"),
            ("beacon.recv", "127.0.0.1:5650"),
            ("log.directory", "/var/log/tcspecial"),
            ("log.segment_size", "65536"),
        ]
    }

    /// Gives a section that answers to cFS and states every attribute it
    /// must, laid out as the design's example is.
    ///
    /// # Returns
    ///
    /// Each attribute's flat name, with a value that reads.
    fn on_the_bus() -> Vec<(&'static str, &'static str)> {
        vec![
            ("gateway", "cfs"),
            ("interface", "c7820f9bfe585feab72247ece0a2261d"),
            ("sbn.release", "v7.0.1"),
            ("sbn.ident", "3b339a5e0f686cef822a5d6ea95004222ab7af33"),
            ("sbn.spacecraft_id", "0x42"),
            ("sbn.processor_id", "2"),
            ("sbn.local", "0.0.0.0:2234"),
            ("sbn.peer", "10.0.0.1:2234"),
            ("sbn.peer_processor_id", "1"),
            ("sbn.max_message", "32768"),
            ("msgid_range.first", "0x4200"),
            ("msgid_range.last", "0x42FF"),
            ("tcs_cmd.cmd_msgid", "0x4280"),
            ("tcs_cmd.response_msgid", "0x4200"),
            ("payload_command.cmd_base", "0x4290"),
            ("payload_command.response_base", "0x4210"),
            ("payload_telemetry.tlm_base", "0x4220"),
            ("beacon.default_interval", "5 s"),
            ("beacon.reset_interval", "1 s"),
            ("beacon.tlm_msgid", "0x4201"),
            ("cfe_time.epoch", "1980-01-01T00:00:00"),
            ("cfe_time.scale", "tai"),
            ("cfe_time.leap_seconds", "37"),
            ("state_file", "/var/lib/tcspecial/state"),
        ]
    }

    /// Settles a section on the bus with some attributes changed: those
    /// given an empty value are taken out.
    ///
    /// # Arguments
    ///
    /// * `change` - Attributes by flat name, each with the value it is given
    ///              in place of [`on_the_bus`]'s, or an empty value to take
    ///              it out; a name not there is added.
    ///
    /// # Returns
    ///
    /// The section, settled as the command interpreter takes it.
    ///
    /// # Errors
    ///
    /// What reading or settling the section says is wrong with it, as a
    /// configuration's check would say it.
    fn bus_with(
        change: &[(&'static str, &'static str)],
    ) -> Result<crate::CIConfig, String> {
        let mut all: Vec<_> = on_the_bus()
            .into_iter()
            .filter(|(name, _)| !change.iter().any(|(n, _)| n == name))
            .collect();
        all.extend(change.iter().filter(|(_, value)| !value.is_empty()));
        ci_config_of(&said(&all)).and_then(|config| config.to_ci_config())
    }

    /// A section that answers to cFS reads as its peer, its allotment, its
    /// message IDs and its clock.
    #[test]
    fn a_section_on_the_bus_is_what_the_command_interpreter_takes() {
        let settled = bus_with(&[]).expect("the whole section");
        assert!(settled.sockets().is_none());
        let bus = settled.bus().expect("on the bus");
        assert_eq!(bus.sbn.release.protocol_version, 11);
        assert_eq!(
            &bus.sbn.ident[5..45],
            b"3b339a5e0f686cef822a5d6ea95004222ab7af33"
        );
        assert_eq!(bus.sbn.spacecraft_id, 0x42);
        assert_eq!((bus.sbn.processor_id, bus.sbn.peer_processor_id), (2, 1));
        assert_eq!(bus.sbn.peer.to_string(), "10.0.0.1:2234");
        assert_eq!(bus.sbn.max_message, 32768);
        assert_eq!((bus.first, bus.last), (0x4200, 0x42FF));
        assert_eq!((bus.cmd_msgid, bus.response_msgid), (0x4280, 0x4200));
        assert_eq!((bus.cmd_base, bus.response_base), (0x4290, 0x4210));
        assert_eq!((bus.tlm_base, bus.beacon_msgid), (0x4220, 0x4201));
        assert_eq!(
            bus.cfe_time.scale,
            crate::cfs::Scale::Tai { leap_seconds: 37 }
        );
        assert_eq!(bus.cfe_time.epoch, 315_532_800);
        assert_eq!(
            bus_with(&[
                ("cfe_time.scale", "utc"),
                ("cfe_time.leap_seconds", "")
            ])
            .expect("on UTC")
            .bus()
            .unwrap()
            .cfe_time
            .scale,
            crate::cfs::Scale::Utc
        );
    }

    /// Every attribute a section on the bus must state is complained about
    /// by name when it is left out, and the rest may be.
    #[test]
    fn a_missing_attribute_on_the_bus_is_named() {
        for (name, needed) in ATTRIBUTES {
            if !needed.belongs(GatewayKind::Cfs) {
                continue;
            }
            let outcome = bus_with(&[(name, "")]);
            match needed {
                Needed::Always | Needed::Bus => {
                    let why = outcome.expect_err(name);
                    assert!(why.contains(name), "{name}: {why}");
                }
                // Stated on TAI, which the whole section is on.
                Needed::BusByRule => {
                    let why = outcome.expect_err(name);
                    assert!(why.contains(name), "{name}: {why}");
                }
                Needed::Optional | Needed::Sockets => {
                    outcome.unwrap_or_else(|why| panic!("{name}: {why}"));
                }
                Needed::Framed => unreachable!("{name} is not cFS's"),
            }
        }
    }

    /// What a section on the bus states wrong is refused, saying which
    /// attribute and what to do.
    #[test]
    fn what_is_wrong_on_the_bus_is_refused_by_name() {
        for (change, says) in [
            (("sbn.release", "v6.4.0"), &["sbn.release", "v7.0.1"][..]),
            (("sbn.ident", "v7.0.1"), &["sbn.ident", "40 hexadecimal"]),
            (("sbn.processor_id", "1"), &["sbn.peer_processor_id", "own"]),
            (("sbn.spacecraft_id", "many"), &["sbn.spacecraft_id"]),
            (("sbn.peer", "10.0.0.1"), &["sbn.peer", "address:port"]),
            (("sbn.local", "0.0.0.0:0"), &["sbn.local", "port 0"]),
            (("sbn.max_message", "16"), &["sbn.max_message", "32768"]),
            (
                ("msgid_range.last", "0x41FF"),
                &["msgid_range", "backwards"],
            ),
            (
                ("msgid_range.first", "0x10000"),
                &["msgid_range.first", "0xFFFF"],
            ),
            (
                ("tcs_cmd.cmd_msgid", "0x4300"),
                &["tcs_cmd.cmd_msgid", "0x4300", "msgid_range", "0x4200"],
            ),
            (
                ("tcs_cmd.cmd_msgid", "0x4270"),
                &["tcs_cmd.cmd_msgid", "telemetry", "0x80"],
            ),
            (
                ("tcs_cmd.response_msgid", "0x4281"),
                &["tcs_cmd.response_msgid", "a command"],
            ),
            (
                ("payload_telemetry.tlm_base", "0x42A0"),
                &["payload_telemetry.tlm_base", "a command"],
            ),
            (
                ("beacon.tlm_msgid", "0x4100"),
                &["beacon.tlm_msgid", "outside"],
            ),
            (("cfe_time.epoch", "1980"), &["cfe_time.epoch", "YYYY"]),
            (("cfe_time.scale", "gps"), &["cfe_time.scale", "tai or utc"]),
            (
                ("cfe_time.leap_seconds", "a few"),
                &["cfe_time.leap_seconds"],
            ),
        ] {
            let why = bus_with(&[change]).expect_err(change.0);
            for said in says {
                assert!(why.contains(said), "{change:?}: {why}");
            }
        }
        let why = bus_with(&[("cfe_time.scale", "utc")])
            .expect_err("leap seconds on UTC");
        assert!(why.contains("take cfe_time.leap_seconds out"), "{why}");
    }

    /// A link attribute of the other kind is refused by name, and told what
    /// this kind says instead: a `recv` on the bus is an address somebody
    /// expects to be listened on.
    #[test]
    fn a_link_of_the_other_kind_is_refused_by_name() {
        let why = bus_with(&[("tcs_cmd.recv", "0.0.0.0:4000")])
            .expect_err("a socket on the bus");
        assert!(why.contains("tcs_cmd.recv"), "{why}");
        assert!(why.contains("cfs"), "{why}");
        assert!(why.contains("tcs_cmd.cmd_msgid"), "{why}");

        let mut all = whole();
        all.push(("sbn.peer", "10.0.0.1:2234"));
        let why = ci_config_of(&said(&all)).expect_err("a peer on the radio");
        assert!(why.contains("sbn.peer"), "{why}");
        assert!(why.contains("radio"), "{why}");
        let mut all = whole();
        all.push(("beacon.tlm_msgid", "0x4201"));
        let why = ci_config_of(&said(&all)).expect_err("a message ID");
        assert!(why.contains("beacon.send and beacon.recv"), "{why}");
    }

    /// Which attributes each kind requires, as a window marks them.
    #[test]
    fn what_is_required_depends_on_the_kind() {
        assert!(Needed::Always.required(None));
        assert!(!Needed::Sockets.required(None));
        assert!(Needed::Sockets.required(Some(GatewayKind::Fprime)));
        assert!(!Needed::Sockets.required(Some(GatewayKind::Cfs)));
        assert!(Needed::Bus.required(Some(GatewayKind::Cfs)));
        assert!(!Needed::BusByRule.required(Some(GatewayKind::Cfs)));
        assert!(!Needed::Bus.belongs(GatewayKind::Radio));
        assert!(Needed::Sockets.required(Some(GatewayKind::Yamcs)));
        assert!(Needed::Framed.required(Some(GatewayKind::Yamcs)));
        assert!(!Needed::Framed.required(Some(GatewayKind::Radio)));
        assert!(!Needed::Framed.required(None));
        assert!(!Needed::Framed.belongs(GatewayKind::Fprime));
        assert!(!Needed::Framed.belongs(GatewayKind::Cfs));
    }

    /// Settles a section that answers to Yamcs, with some attributes
    /// changed: those given an empty value are taken out.
    ///
    /// # Arguments
    ///
    /// * `change` - Attributes by flat name, each with the value it is given
    ///              in place of the radio's whole section with Yamcs's
    ///              APIDs, or an empty value to take it out.
    ///
    /// # Returns
    ///
    /// The section, settled as the command interpreter takes it.
    ///
    /// # Errors
    ///
    /// What reading or settling the section says is wrong with it.
    fn yamcs_with(
        change: &[(&'static str, &'static str)],
    ) -> Result<crate::CIConfig, String> {
        let mut all: Vec<_> = whole()
            .into_iter()
            .filter(|(name, _)| *name != "gateway")
            .chain([
                ("gateway", "yamcs"),
                ("beacon.apid", "0x100"),
                ("tcs_cmd.apid", "257"),
                ("payload_telemetry.apid_base", "0x300"),
            ])
            .filter(|(name, _)| !change.iter().any(|(n, _)| n == name))
            .collect();
        all.extend(change.iter().filter(|(_, value)| !value.is_empty()));
        ci_config_of(&said(&all)).and_then(|config| config.to_ci_config())
    }

    /// A section that answers to Yamcs is the radio's sockets, the two
    /// APIDs, and the base payload telemetry's are counted from.
    #[test]
    fn a_section_answering_to_yamcs_carries_its_apids() {
        let settled = yamcs_with(&[]).expect("the whole section");
        assert_eq!(settled.gateway, GatewayKind::Yamcs);
        let sockets = settled.sockets().expect("on sockets");
        assert_eq!(
            sockets.framing,
            Some(crate::CcsdsFraming {
                beacon_apid: 0x100,
                cmd_apid: 0x101,
                payload_apid_base: 0x300,
            })
        );
        assert_eq!(sockets.beacon_recv.to_string(), "127.0.0.1:5650");
        assert_eq!(
            ci_config_of(&said(&whole()))
                .and_then(|c| c.to_ci_config())
                .expect("the radio's")
                .sockets()
                .expect("on sockets")
                .framing,
            None,
            "the radio frames nothing"
        );

        // Where the ground sends commands from may be any port of a host,
        // as a Yamcs station sends: the vehicle binds neither.
        let settled = yamcs_with(&[
            ("tcs_cmd.send", "127.0.0.1:0"),
            ("payload_command.source_base", "127.0.0.1:0"),
        ])
        .expect("senders at port 0");
        let sockets = settled.sockets().expect("on sockets");
        assert_eq!(sockets.moc_cmd_send.port, 0);
        assert_eq!(sockets.payload_command_source_base.port, 0);
        // But not where the vehicle listens.
        let why = yamcs_with(&[("tcs_cmd.recv", "127.0.0.1:0")])
            .expect_err("port 0 bound");
        assert!(why.contains("port 0"), "{why}");
    }

    /// Each APID left out, out of range or the same as the other is refused
    /// by name, saying what to do.
    #[test]
    fn what_is_wrong_with_yamcs_apids_is_refused_by_name() {
        for (change, says) in [
            (("beacon.apid", ""), &["beacon.apid", "not stated"][..]),
            (("tcs_cmd.apid", ""), &["tcs_cmd.apid", "not stated"]),
            (("beacon.apid", "0x800"), &["beacon.apid", "0x800", "0x7ff"]),
            (("tcs_cmd.apid", "2048"), &["tcs_cmd.apid", "2048", "0x7ff"]),
            (("tcs_cmd.apid", "many"), &["tcs_cmd.apid"]),
            (
                ("payload_telemetry.apid_base", ""),
                &["payload_telemetry.apid_base", "not stated"],
            ),
            (
                ("payload_telemetry.apid_base", "0x800"),
                &["payload_telemetry.apid_base", "0x800", "0x7ff"],
            ),
            (
                ("tcs_cmd.apid", "0x100"),
                &["beacon.apid", "tcs_cmd.apid", "0x100", "another APID"],
            ),
        ] {
            let why = yamcs_with(&[change]).expect_err(change.0);
            for said in says {
                assert!(why.contains(said), "{change:?}: {why}");
            }
        }
        // The highest and the lowest are APIDs.
        yamcs_with(&[("beacon.apid", "0x7FF"), ("tcs_cmd.apid", "0")])
            .expect("the ends of the range");
    }

    /// An APID in a configuration whose packets carry no CCSDS header is
    /// refused by name, whichever kind it is.
    #[test]
    fn an_apid_is_refused_where_the_gateway_is_not_yamcs() {
        for name in
            ["beacon.apid", "tcs_cmd.apid", "payload_telemetry.apid_base"]
        {
            let mut all = whole();
            all.push((name, "0x100"));
            let why = ci_config_of(&said(&all)).expect_err(name);
            assert!(why.contains(name), "{why}");
            assert!(why.contains("yamcs"), "{why}");
            assert!(why.contains("take it out"), "{why}");

            let why = bus_with(&[(name, "0x100")]).expect_err(name);
            assert!(why.contains(name), "{why}");
            assert!(why.contains("yamcs"), "{why}");
        }
    }

    /// A section with everything in it reads as what the command interpreter
    /// wants.
    #[test]
    fn a_whole_section_is_what_the_command_interpreter_takes() {
        let config =
            ci_config_of(&said(&whole())).expect("this one is complete");
        assert_eq!(config.tcs_cmd.recv.as_deref(), Some("0.0.0.0:4000"));
        assert_eq!(config.tcs_cmd.send.as_deref(), Some("127.0.0.1:4100"));
        assert_eq!(
            config.payload_command.source_base.as_deref(),
            Some("127.0.0.1:4300")
        );
        assert_eq!(
            config.payload_command.dest_base.as_deref(),
            Some("0.0.0.0:4200")
        );
        assert_eq!(config.beacon.default_interval, "5 s");
        assert_eq!(config.beacon.send.as_deref(), Some("0.0.0.0:5550"));
        assert_eq!(config.log.directory.as_deref(), Some("/var/log/tcspecial"));
        assert_eq!(config.log.segment_size, 65536);

        // And it holds together by the rules of the one piece of code that
        // knows them -- which is also where the one address and the one port
        // come out of the one attribute.
        let settled = config.to_ci_config().expect("and sound");
        let settled = settled.sockets().expect("the radio's are sockets");
        assert_eq!(settled.address, "0.0.0.0");
        assert_eq!(settled.port, 4000);
        assert_eq!(settled.moc_cmd_send.to_string(), "127.0.0.1:4100");
        // And the bases, which are counted on by a payload's own id.
        assert_eq!(settled.payload_source_base.at(0).unwrap().port, 6000);
        assert_eq!(settled.payload_dest_base.at(3).unwrap().port, 6103);
    }

    /// How often beacons go across a restart is `beacon.reset_interval`: a
    /// length of time, not nought and no longer than
    /// `beacon.default_interval`, each refusal naming the attribute. And
    /// `state_file` is required, as the beacon's are.
    #[test]
    fn the_reset_interval_and_the_state_file_are_required_and_checked() {
        let settled = |change: &[(&'static str, &'static str)]| {
            let mut all: Vec<_> = whole()
                .into_iter()
                .filter(|(name, _)| !change.iter().any(|(n, _)| n == name))
                .collect();
            all.extend(change.iter().filter(|(_, value)| !value.is_empty()));
            ci_config_of(&said(&all)).and_then(|config| config.to_ci_config())
        };
        let config = settled(&[]).expect("the whole section");
        assert_eq!(config.reset_interval, Duration::from_secs(1));
        assert_eq!(config.state_file, "/var/lib/tcspecial/state");
        assert_eq!(
            settled(&[("beacon.reset_interval", "5 s")])
                .expect("as long as the interval")
                .reset_interval,
            Duration::from_secs(5)
        );
        for (change, said) in [
            (("beacon.reset_interval", ""), "beacon.reset_interval"),
            (("beacon.reset_interval", "0 s"), "beacon.reset_interval"),
            (("beacon.reset_interval", "6 s"), "no longer than"),
            (("beacon.reset_interval", "soon"), "beacon.reset_interval"),
            (("state_file", ""), "state_file"),
        ] {
            let why = settled(&[change]).expect_err(change.1);
            assert!(why.contains(said), "{change:?}: {why}");
        }
    }

    /// `interface` is required, is 32 lower-case hex digits, and must be
    /// this build's: one that is not is refused naming both digests and the
    /// attribute to change.
    #[test]
    fn the_interface_is_required_and_must_be_this_builds() {
        let with = |interface: Option<&'static str>| {
            let mut all: Vec<_> = whole()
                .into_iter()
                .filter(|(name, _)| *name != "interface")
                .collect();
            if let Some(said) = interface {
                all.push(("interface", said));
            }
            ci_config_of(&said(&all))
        };
        let ours = crate::InterfaceDigest::of_this_build();
        assert_eq!(
            with(Some("c7820f9bfe585feab72247ece0a2261d"))
                .expect("this build's")
                .interface,
            ours,
            "the examples' interface is this build's: when link.xtce changes, \
             every configuration is given the new digest"
        );

        let why = with(None).expect_err("it is required");
        assert!(why.contains("interface is not stated"), "{why}");

        let why = with(Some("AD9AAC4DD5A6D88DD5E78824BA8A83AC"))
            .expect_err("upper case is not how a digest is written");
        assert!(why.contains("not an interface digest"), "{why}");
        assert!(why.contains("--interface-digest"), "{why}");

        let other = "0123456789abcdef0123456789abcdef";
        let why = with(Some(other)).expect_err("another interface");
        assert!(why.contains(other), "{why}");
        assert!(why.contains(&ours.to_string()), "{why}");
        assert!(why.contains("interface:"), "{why}");
    }

    /// Where configurations are kept, how many must be, and how many tries a
    /// configuration is given are all optional; the two counts have their
    /// defaults where they are left out, and nought is refused, naming the
    /// attribute.
    #[test]
    fn the_configurations_attributes_have_their_defaults_and_refuse_nought() {
        let settled = |extra: &[(&'static str, &'static str)]| {
            let mut all = whole();
            all.extend_from_slice(extra);
            ci_config_of(&said(&all)).and_then(|config| config.to_ci_config())
        };
        let left_out = settled(&[]).expect("left out");
        assert_eq!(left_out.configurations, None, "a vehicle that keeps none");
        assert_eq!(
            left_out.configurations_kept,
            crate::CONFIGURATIONS_KEPT_DEFAULT
        );
        assert_eq!(left_out.load_trials, crate::LOAD_TRIALS_DEFAULT);
        assert_eq!(crate::LOAD_TRIALS_DEFAULT, 3, "as the design says");

        let stated = settled(&[
            ("configurations", "/var/lib/tcspecial/configurations"),
            ("configurations_kept", "4"),
            ("load_trials", "5"),
        ])
        .expect("stated");
        assert_eq!(
            stated.configurations.as_deref(),
            Some("/var/lib/tcspecial/configurations")
        );
        assert_eq!(stated.configurations_kept, 4);
        assert_eq!(stated.load_trials, 5);

        for name in ["configurations_kept", "load_trials"] {
            let why = settled(&[(name, "0")]).expect_err(name);
            assert!(why.contains(name), "{why}");
            assert!(why.contains("1 or more"), "{why}");
            let why = settled(&[(name, "many")]).expect_err(name);
            assert!(why.contains(name), "{why}");
        }
    }

    /// A file still writing `beacon.interval`, from before there were two,
    /// is told the name it has now.
    #[test]
    fn the_beacons_old_interval_is_told_its_new_name() {
        let mut all = whole();
        all.push(("beacon.interval", "5 s"));
        let why = ci_config_of(&said(&all)).expect_err("a retired name");
        assert!(why.contains("beacon.default_interval"), "{why}");
    }

    /// How long an arm lasts is the section's `restart_arm_valid`, a length
    /// of time, and twenty minutes where it says nothing; nought, and what is
    /// not a length of time, are refused naming it.
    #[test]
    fn restart_arm_valid_is_a_length_of_time_with_a_default() {
        let settled = |extra: &[(&'static str, &'static str)]| {
            let mut all = whole();
            all.extend_from_slice(extra);
            ci_config_of(&said(&all)).and_then(|config| config.to_ci_config())
        };

        assert_eq!(
            settled(&[]).expect("left out").restart_arm_valid,
            crate::RESTART_ARM_VALID_DEFAULT
        );
        assert_eq!(
            settled(&[("restart_arm_valid", "")])
                .expect("left empty")
                .restart_arm_valid,
            crate::RESTART_ARM_VALID_DEFAULT
        );
        assert_eq!(
            settled(&[("restart_arm_valid", "90 s")])
                .expect("stated")
                .restart_arm_valid,
            std::time::Duration::from_secs(90)
        );
        for wrong in ["0 s", "20", "soon"] {
            let why =
                settled(&[("restart_arm_valid", wrong)]).expect_err(wrong);
            assert!(why.contains("restart_arm_valid"), "{wrong}: {why}");
        }
    }

    /// Every attribute that must be stated is complained about by name.
    ///
    /// Ten attributes and one complaint apiece: "the tcspecial section is
    /// wrong" is a sentence that sends somebody to read all ten.
    #[test]
    fn a_missing_attribute_is_named() {
        for (name, needed) in ATTRIBUTES {
            if !needed.belongs(GatewayKind::Radio) {
                continue;
            }
            let left_out: Vec<(&str, &str)> =
                whole().into_iter().filter(|(had, _)| had != name).collect();
            let outcome = ci_config_of(&said(&left_out));
            if needed.required(Some(GatewayKind::Radio)) {
                let why = outcome.expect_err(&format!("{name} is required"));
                assert!(why.contains(name), "{why}");
            } else {
                outcome
                    .unwrap_or_else(|why| panic!("{name} is optional: {why}"));
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
        let why = ci_config_of(&said(&cleared))
            .expect_err("an empty command interface is none");
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
            let why =
                said(said_).expect_err(&format!("{said_} should be refused"));
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
            ("beacon.default_interval", "often"),
            ("beacon.default_interval", "-1"),
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
        let why =
            ci_config_of(&said(&wrong)).expect_err("a typo should be refused");
        assert!(why.contains("beacon_adress"), "{why}");
        // And the complaint lists what there is, so the typo can be fixed
        // without opening the manual.
        assert!(why.contains("beacon.send"), "{why}");

        // One written with a hyphen for its underscore is told its spelling,
        // which is also what a file written before restart_arm_valid took
        // the underscores every other name has is told.
        let mut hyphened = whole();
        hyphened.push(("restart-arm-valid", "20 min"));
        let why = ci_config_of(&said(&hyphened)).expect_err("one spelling");
        assert!(why.contains("it is spelled restart_arm_valid"), "{why}");
        let mut hyphened = whole();
        hyphened.push(("log.segment-size", "65536"));
        let why = ci_config_of(&said(&hyphened)).expect_err("one spelling");
        assert!(why.contains("it is spelled log.segment_size"), "{why}");
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

    /// The interval a ground station ages beacons against is the one the
    /// section states, and a configuration with no section states none.
    #[test]
    fn the_beacon_interval_is_the_sections() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut graph = Graph::load(root.join("examples/ping/ping.ytcs"))
            .expect("the example reads");
        assert_eq!(
            beacon_interval_of(&graph),
            Ok(Some(std::time::Duration::from_secs(5)))
        );

        graph.tcspecial = None;
        assert_eq!(beacon_interval_of(&graph), Ok(None));
    }
}
