//! The simulator's own configuration file
//!
//! A payload configuration file describes payloads: which data handlers exist,
//! how tcspecial reaches each one, and how big its packets are. None of that is
//! enough to simulate one. How fast a payload produces packets, and how the
//! simulator divides a packet into segments, are properties of the simulation
//! rather than of the payload, so they are carried here instead.
//!
//! The two files are joined by name. A simulated payload names the data handler
//! it stands in for, and may name a group to take its settings from:
//!
//! ```yaml
//! simulated_payload_groups:
//!   - name: steady_1hz
//!     packet_interval_ms: 1000
//!     segment_interval_ms: 1000
//!
//! simulated_payloads:
//!   - name: DH0
//!     type: network
//!     protocol: udp
//!     group: steady_1hz
//!   - name: DH3
//!     type: network
//!     protocol: tcp
//!     packet_interval_ms: 500
//! ```
//!
//! The kind and the transport are the payload configuration's, not this
//! file's: they are stated here only so that a simulator file paired with the
//! wrong payload file is an error rather than a run that produces nothing.
//!
//! A group carries what several simulated payloads have in common; a payload
//! overrides any of it for itself. The format is chosen from the file extension
//! by [`tcslibgs::load_config_file`], so the same configuration can be written
//! in YAML or XML.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use serde::de::IgnoredAny;
use serde::Deserialize;
use crate::verify::About;
use crate::{
    load_config_file, one_host, DHConfig, DHType, EndpointConfig, NetworkProtocol, TcsResult,
};

/// What can be wrong with a simulator configuration, once it has parsed.
///
/// Every one of these is a file that would otherwise simulate the wrong thing
/// quietly: a payload nobody drives, a name that matches nothing, or a rate
/// that was never stated.
#[derive(Debug, PartialEq, Eq)]
pub enum SimConfigError {
    /// Two groups, or two simulated payloads, share a name.
    Duplicate { what: &'static str, name: String },
    /// A simulated payload names a group the file does not define.
    UnknownGroup { payload: String, group: String },
    /// A simulated payload names no data handler of the payload file.
    UnknownPayload { payload: String },
    /// A data handler has no simulated payload to drive it.
    Unsimulated { handler: String },
    /// A group is defined that no simulated payload names.
    ///
    /// Reported in the wording the endpoint and payload configuration formats
    /// use for the same rule, so that one rule reads as one rule wherever it
    /// is met.
    UnusedGroup { group: String },
    /// Neither a simulated payload nor its group states a packet interval.
    NoPacketInterval { payload: String },
    /// A triggered payload was given a packet interval, which it has nothing
    /// to do with: it sends when it is asked.
    IntervalForATriggeredPayload {
        payload: String,
        setting: &'static str,
    },
    /// A percentage outside nought to a hundred.
    NotAPercentage {
        payload: String,
        setting: &'static str,
        given: u32,
    },
    /// A fault a payload of this kind cannot have.
    FaultForTheWrongPayload {
        payload: String,
        fault: &'static str,
        why: &'static str,
    },
    /// A simulated payload that does not say what kind of payload it is.
    NoKind { payload: String, actually: DHType },
    /// A word that names no kind of payload.
    NoSuchKind { payload: String, given: String },
    /// The two files disagree about what kind of payload this is.
    KindMismatch {
        payload: String,
        in_payload_file: DHType,
        in_sim_file: DHType,
    },
    /// A network payload that does not say which transport.
    NoProtocol {
        payload: String,
        actually: NetworkProtocol,
    },
    /// A word that names no transport.
    NoSuchProtocol { payload: String, given: String },
    /// The two files disagree about the transport.
    ProtocolMismatch {
        payload: String,
        in_payload_file: NetworkProtocol,
        in_sim_file: NetworkProtocol,
    },
    /// A setting that belongs to a network payload, stated by a payload of
    /// some other kind.
    NotForThisKind {
        payload: String,
        kind: DHType,
        setting: &'static str,
    },
    /// An address of its own, stated for a payload that has none to bind.
    NoAddressOfItsOwn {
        payload: String,
        setting: &'static str,
        why: &'static str,
    },
    /// A payload asked to bind the very socket its handler binds.
    OneSocketForBothEnds { payload: String, what: String },
    /// Where the payload is, written here, where it is not decided.
    TheOtherFilesBusiness {
        what: &'static str,
        name: String,
        given: &'static str,
    },
    /// A word that is not a setting at all, which is what a misspelled
    /// setting looks like.
    NoSuchSetting {
        what: &'static str,
        name: String,
        given: String,
    },
}

impl fmt::Display for SimConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimConfigError::Duplicate { what, name } => {
                write!(f, "{} \"{}\" is defined more than once", what, name)
            }
            SimConfigError::UnknownGroup { payload, group } => write!(
                f,
                "simulated payload \"{}\" names group \"{}\", which is not defined",
                payload, group
            ),
            SimConfigError::UnknownPayload { payload } => write!(
                f,
                "\"{}\" is simulated, but the payload configuration has no data \
                 handler of that name",
                payload
            ),
            SimConfigError::Unsimulated { handler } => write!(
                f,
                "data handler \"{}\" has no simulated payload: add one naming it, \
                 or remove the handler",
                handler
            ),
            SimConfigError::UnusedGroup { group } => write!(
                f,
                "simulated payload group \"{}\" is named by no simulated payload: \
                 name it from one, or remove the group",
                group
            ),
            SimConfigError::NoPacketInterval { payload } => write!(
                f,
                "simulated payload \"{}\" has no packet interval: state one for it \
                 or in the group it names",
                payload
            ),
            SimConfigError::NotAPercentage {
                payload,
                setting,
                given,
            } => write!(
                f,
                "simulated payload \"{}\" gives {} as {}, which is a percentage: \
                 nought to a hundred",
                payload, given, setting
            ),
            SimConfigError::FaultForTheWrongPayload { payload, fault, why } => write!(
                f,
                "simulated payload \"{}\" asks for {}, which it cannot have: {}",
                payload, fault, why
            ),
            SimConfigError::NoSuchSetting { what, name, given } => write!(
                f,
                "{} \"{}\" states \"{}\", which is not a setting. A word this file \
                 does not know is refused rather than ignored: a misspelled \
                 setting that was ignored would have the simulator do something \
                 other than what the file asked for, and say nothing",
                what, name, given
            ),
            SimConfigError::NoKind { payload, actually } => write!(
                f,
                "simulated payload \"{}\" does not say what kind of payload it \
                 stands in for: state type: {}, as the payload configuration \
                 does. It is stated in both files so that the two can be \
                 checked against each other, which is the only thing that \
                 catches a simulator file paired with the wrong payload file",
                payload,
                actually.spelling()
            ),
            SimConfigError::NoSuchKind { payload, given } => write!(
                f,
                "simulated payload \"{}\" is a \"{}\" payload, which is not a kind: \
                 expected {}",
                payload,
                given,
                DHType::spellings()
            ),
            SimConfigError::KindMismatch {
                payload,
                in_payload_file,
                in_sim_file,
            } => write!(
                f,
                "simulated payload \"{}\" is a {} payload here and a {} payload in \
                 the payload configuration. Both files describe the one payload, so \
                 they cannot disagree about its kind: either this is not the \
                 payload file these settings were written for, or one of the two \
                 has been changed and the other has not",
                payload,
                in_sim_file.spelling(),
                in_payload_file.spelling()
            ),
            SimConfigError::NoProtocol { payload, actually } => write!(
                f,
                "simulated payload \"{}\" is a network payload and does not say \
                 which transport: state protocol: {}, as the payload configuration \
                 does. Which transport it is decides what the payload can be made \
                 to do -- only a stream can be hung up on, and only a stream can \
                 be triggered -- so a network payload states it in both files",
                payload,
                actually.spelling()
            ),
            SimConfigError::NoSuchProtocol { payload, given } => write!(
                f,
                "simulated payload \"{}\" speaks \"{}\", which is not a protocol: \
                 expected {}",
                payload,
                given,
                NetworkProtocol::spellings()
            ),
            SimConfigError::ProtocolMismatch {
                payload,
                in_payload_file,
                in_sim_file,
            } => write!(
                f,
                "simulated payload \"{}\" speaks {} here and {} in the payload \
                 configuration. The transport decides how the payload is reached \
                 and what it can be asked to do, so the two files cannot disagree \
                 about it",
                payload,
                in_sim_file.spelling(),
                in_payload_file.spelling()
            ),
            SimConfigError::NotForThisKind {
                payload,
                kind,
                setting,
            } => write!(
                f,
                "simulated payload \"{}\" is a {} payload and states {}, which only \
                 a network payload has",
                payload,
                kind.spelling(),
                setting
            ),
            SimConfigError::NoAddressOfItsOwn {
                payload,
                setting,
                why,
            } => write!(
                f,
                "simulated payload \"{}\" states {}, which it has no use for: {}",
                payload, setting, why
            ),
            SimConfigError::OneSocketForBothEnds { payload, what } => write!(
                f,
                "simulated payload \"{}\" would bind {}, which is the handler's own \
                 end of the link: a socket cannot be bound twice, and the two ends \
                 of a link are two sockets",
                payload, what
            ),
            SimConfigError::TheOtherFilesBusiness { what, name, given } => write!(
                f,
                "{} \"{}\" states {}, which belongs in the payload configuration \
                 file: the simulator takes where a payload is from there. What this \
                 file may say is where the payload answers from -- payload_address \
                 and payload_port, the payload's own end of the link -- which \
                 nothing else states",
                what, name, given
            ),
            SimConfigError::IntervalForATriggeredPayload { payload, setting } => write!(
                f,
                "simulated payload \"{}\" is triggered, so it sends when tcspecial \
                 asks and has no timing of its own: {} belongs to a payload that \
                 sends on its own, and the rate a triggered one is asked at is \
                 packet_interval_ms in the payload configuration",
                payload, setting
            ),
        }
    }
}

impl SimConfigError {
    /// Which entry of the file this is about, which is how `tcsverify` finds
    /// the line it belongs to. See [`crate::verify`].
    ///
    /// A handler nothing simulates is the one that is about no entry: there
    /// is nothing in the file to point at, the mistake being that an entry is
    /// missing, so it is reported against the section the entry belongs in.
    pub fn about(&self) -> About {
        // Two of these name a payload or a group according to what they were
        // asked about, the rule being the same for both.
        let either = |what: &str, name: &str| {
            if what.contains("group") {
                About::SimGroup(name.to_string())
            } else {
                About::SimPayload(name.to_string())
            }
        };

        match self {
            SimConfigError::Duplicate { what, name } => either(what, name),
            SimConfigError::TheOtherFilesBusiness { what, name, .. } => either(what, name),
            SimConfigError::NoSuchSetting { what, name, .. } => either(what, name),

            SimConfigError::UnusedGroup { group } => About::SimGroup(group.clone()),

            SimConfigError::Unsimulated { .. } => About::Section("simulated_payloads"),

            SimConfigError::UnknownGroup { payload, .. }
            | SimConfigError::UnknownPayload { payload }
            | SimConfigError::NoPacketInterval { payload }
            | SimConfigError::IntervalForATriggeredPayload { payload, .. }
            | SimConfigError::NotAPercentage { payload, .. }
            | SimConfigError::FaultForTheWrongPayload { payload, .. }
            | SimConfigError::NoKind { payload, .. }
            | SimConfigError::NoSuchKind { payload, .. }
            | SimConfigError::KindMismatch { payload, .. }
            | SimConfigError::NoProtocol { payload, .. }
            | SimConfigError::NoSuchProtocol { payload, .. }
            | SimConfigError::ProtocolMismatch { payload, .. }
            | SimConfigError::NotForThisKind { payload, .. }
            | SimConfigError::NoAddressOfItsOwn { payload, .. }
            | SimConfigError::OneSocketForBothEnds { payload, .. } => {
                About::SimPayload(payload.clone())
            }
        }
    }
}

impl std::error::Error for SimConfigError {}

/// A percentage, or the complaint that it is not one.
fn a_percentage(
    payload: &str,
    setting: &'static str,
    given: Option<u32>,
) -> Result<u32, SimConfigError> {
    match given {
        None => Ok(0),
        Some(percent) if percent <= 100 => Ok(percent),
        Some(given) => Err(SimConfigError::NotAPercentage {
            payload: payload.to_string(),
            setting,
            given,
        }),
    }
}

/// Whether a payload of this kind has a connection it could close.
///
/// A stream has one: the handler connects and the payload may hang up. A
/// datagram socket has none, and the kinds the handler opens rather than
/// connects to -- a device, a bus, a line -- have nothing to hang up either.
fn has_a_connection(endpoint: &EndpointConfig) -> bool {
    matches!(
        endpoint,
        EndpointConfig::Network(net)
            if matches!(
                net.protocol,
                NetworkProtocol::Tcp | NetworkProtocol::UnixStream
            )
    )
}

/// Refuse a word that is not a setting.
///
/// The settings are flattened into the group and the payload that carry them,
/// and serde cannot refuse an unknown field of a flattened structure, so what
/// is left over is collected and reported here instead.
fn nothing_unknown(
    what: &'static str,
    name: &str,
    unknown: &BTreeMap<String, IgnoredAny>,
) -> Result<(), SimConfigError> {
    let Some(given) = unknown.keys().next() else {
        return Ok(());
    };

    // The two the payload file spells without a prefix. Someone writing a
    // simulator file beside a payload file, or copying from one, writes them
    // the way the payload file does -- so they are named here and answered
    // with the name this file uses, rather than reported as words nothing
    // knows.
    // The two the payload configuration states. Someone writing a simulator
    // file beside one, or copying from it, writes them the way that file does
    // -- so they are named here and answered with the file they belong in,
    // rather than reported as words nothing knows.
    if let Some(given) = ["address", "port"].iter().find(|plain| *plain == given) {
        return Err(SimConfigError::TheOtherFilesBusiness {
            what,
            name: name.to_string(),
            given,
        });
    }

    Err(SimConfigError::NoSuchSetting {
        what,
        name: name.to_string(),
        given: given.clone(),
    })
}

/// Check this entry against the payload it names.
///
/// The kind always, and the transport for a network payload. Everything else
/// in a simulator file is a property of the simulation and so is this file's
/// to decide; these two are the payload's own and are stated here only to be
/// compared, which is why a disagreement is an error rather than an override.
fn what_it_is(
    name: &str,
    settings: &SimSettings,
    endpoint: &EndpointConfig,
) -> Result<(), SimConfigError> {
    let actually = endpoint.kind();

    let stated = match settings.dh_type.as_deref() {
        Some(text) => DHType::from_spelling(text).ok_or_else(|| SimConfigError::NoSuchKind {
            payload: name.to_string(),
            given: text.to_string(),
        })?,
        None => {
            return Err(SimConfigError::NoKind {
                payload: name.to_string(),
                actually,
            })
        }
    };

    if stated != actually {
        return Err(SimConfigError::KindMismatch {
            payload: name.to_string(),
            in_payload_file: actually,
            in_sim_file: stated,
        });
    }

    // A transport and an address belong to a network payload and to no other
    // kind, so for every other kind the question is whether any of them was
    // stated at all.
    let EndpointConfig::Network(network) = endpoint else {
        let foreign = [
            ("a protocol", settings.protocol.is_some()),
            ("payload_address", settings.payload_address.is_some()),
            ("payload_port", settings.payload_port.is_some()),
        ];
        return match foreign.iter().find(|(_, stated)| *stated) {
            Some((setting, _)) => Err(SimConfigError::NotForThisKind {
                payload: name.to_string(),
                kind: actually,
                setting,
            }),
            None => Ok(()),
        };
    };

    let stated = match settings.protocol.as_deref() {
        Some(text) => {
            NetworkProtocol::from_spelling(text).ok_or_else(|| SimConfigError::NoSuchProtocol {
                payload: name.to_string(),
                given: text.to_string(),
            })?
        }
        None => {
            return Err(SimConfigError::NoProtocol {
                payload: name.to_string(),
                actually: network.protocol,
            })
        }
    };

    if stated != network.protocol {
        return Err(SimConfigError::ProtocolMismatch {
            payload: name.to_string(),
            in_payload_file: network.protocol,
            in_sim_file: stated,
        });
    }

    // Where the payload is. Stated or not, as the file likes: a kind and a
    // transport are what tell two payload sets apart whatever else they
    // share, where an address is commonly the same in both and so would be
    // duplication asked for and nothing caught. Stated, it is compared.
    let named_by_a_path = matches!(
        network.protocol,
        NetworkProtocol::UnixStream | NetworkProtocol::UnixDgram
    );

    // And the socket this payload binds for itself, which only the kinds
    // that reach out to a waiting handler have: a stream payload is the end
    // that waits, so it binds the address the payload configuration gives and
    // another stated here would be two answers to where it is.
    let datagram = matches!(
        network.protocol,
        NetworkProtocol::Udp | NetworkProtocol::UnixDgram
    );
    if !datagram {
        for (setting, stated) in [
            ("payload_address", settings.payload_address.is_some()),
            ("payload_port", settings.payload_port.is_some()),
        ] {
            if stated {
                return Err(SimConfigError::NoAddressOfItsOwn {
                    payload: name.to_string(),
                    setting,
                    why: "a stream payload is the end that waits, so it binds the \
                          address the payload configuration gives it",
                });
            }
        }
    }

    // A Unix socket is named by a path, so the payload's own end is a path
    // too and a port there would name nothing.
    if named_by_a_path && settings.payload_port.is_some() {
        return Err(SimConfigError::NoAddressOfItsOwn {
            payload: name.to_string(),
            setting: "payload_port",
            why: "a Unix datagram socket is named by a path, so payload_address is \
                  where this payload answers from",
        });
    }

    // And neither end may be the other. Two sockets cannot be one, and a
    // payload told to bind its handler's address would fail at the bind with
    // nothing to say about which file was wrong.
    if named_by_a_path {
        if settings.payload_address.as_deref() == Some(network.address.as_str()) {
            return Err(SimConfigError::OneSocketForBothEnds {
                payload: name.to_string(),
                what: network.address.clone(),
            });
        }
    } else if settings.payload_port == Some(network.port)
        && settings
            .payload_address
            .as_deref()
            .map(|address| one_host(address) == one_host(&network.address))
            .unwrap_or(false)
    {
        return Err(SimConfigError::OneSocketForBothEnds {
            payload: name.to_string(),
            what: format!("{}:{}", network.address, network.port),
        });
    }

    Ok(())
}

/// Settings a group or a simulated payload may state.
///
/// Every one is optional in the file: a payload states what it does not take
/// from its group, and a group states what its payloads share.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct SimSettings {
    // What payload this is, said again. Neither of these is a setting of the
    // simulation: the payload configuration decides both, and a simulator
    // file that disagreed with it would be simulating something else. They
    // are stated here so that the disagreement can be found -- the two files
    // are joined by name alone, and a name is exactly what a payload set
    // copied from another one keeps.
    /// What kind of payload this is, in the payload configuration's own word
    /// for it: network, device, serial, i2c, or spi.
    #[serde(rename = "type", default)]
    pub dh_type: Option<String>,
    /// Which transport, for a network payload, and refused for every other
    /// kind.
    #[serde(default)]
    pub protocol: Option<String>,
    // The socket the simulated payload binds for itself.
    //
    // Not where the payload is -- the payload configuration file says that,
    // and the simulator takes it from there -- but the other end of the same
    // link: where this payload answers from, which nothing else states at
    // all.
    /// The address the simulated payload binds for itself, and the path for a
    /// Unix datagram socket.
    ///
    /// Only the kinds where the payload has an address of its own to bind: a
    /// datagram payload reaches out to a handler that is waiting, so its own
    /// address is its own business and is otherwise whatever the system gives
    /// it. A stream payload is the end that waits, so it binds the address
    /// the payload configuration already gives, and stating another here
    /// would be two answers to where it is.
    ///
    /// Absent is what every simulator file asked for before this existed: any
    /// interface, and a port the system chooses.
    #[serde(default)]
    pub payload_address: Option<String>,
    /// The port the simulated payload binds for itself. Refused for a Unix
    /// datagram socket, which is named by a path.
    #[serde(default)]
    pub payload_port: Option<u16>,

    /// Milliseconds between packets. 0 is as fast as the payload can be driven.
    #[serde(default)]
    pub packet_interval_ms: Option<u32>,
    /// Milliseconds between the segments of one packet. Defaults to the packet
    /// interval.
    #[serde(default)]
    pub segment_interval_ms: Option<u32>,
    /// Bytes in one segment. Defaults to the whole packet, which is the packet
    /// size the payload configuration gives the data handler.
    #[serde(default)]
    pub segment_size: Option<u32>,

    // What the payload does wrong, which is the whole reason a simulator is
    // better than the hardware for some of what it is used for. None of this
    // has any business in a payload configuration: a payload file describing
    // a payload that drops packets would be describing hardware nobody would
    // fly. Every one of them is off when it is absent, so a file that says
    // nothing about faults describes a payload that works.
    /// Packets in a hundred that are not sent at all.
    #[serde(default)]
    pub drop_percent: Option<u32>,
    /// Packets in a hundred that go out with a byte of them altered.
    #[serde(default)]
    pub corrupt_percent: Option<u32>,
    /// Packets in a hundred that go out short of their packet size.
    #[serde(default)]
    pub truncate_percent: Option<u32>,
    /// How much later than it should a packet may be, in milliseconds.
    ///
    /// Late and never early: a packet sent before the interval the file asked
    /// for would be the simulator disobeying its own configuration. For a
    /// payload that answers requests this is the delay before an answer
    /// rather than an addition to an interval, there being no interval.
    #[serde(default)]
    pub jitter_ms: Option<u32>,
    /// Packets after which the payload stops sending, while staying
    /// connected.
    ///
    /// A payload that has gone quiet without going away, which is what a
    /// wedged instrument looks like from the handler's end and is harder to
    /// notice than one that has closed.
    #[serde(default)]
    pub silent_after: Option<u64>,
    /// Packets after which the payload closes the link.
    ///
    /// Only for the kinds that have a connection to close.
    #[serde(default)]
    pub close_after: Option<u64>,
    /// Requests in a hundred that a triggered payload does not answer.
    ///
    /// What exercises a handler's own waiting: a trigger sent and nothing
    /// returned is the case a response timeout exists for.
    #[serde(default)]
    pub ignore_trigger_percent: Option<u32>,
}

impl SimSettings {
    /// This settings' values, falling back to `base` for whatever it does not
    /// state. Used to lay a simulated payload over the group it names.
    fn over(&self, base: &SimSettings) -> SimSettings {
        SimSettings {
            dh_type: self.dh_type.clone().or_else(|| base.dh_type.clone()),
            protocol: self.protocol.clone().or_else(|| base.protocol.clone()),
            payload_address: self
                .payload_address
                .clone()
                .or_else(|| base.payload_address.clone()),
            payload_port: self.payload_port.or(base.payload_port),
            packet_interval_ms: self.packet_interval_ms.or(base.packet_interval_ms),
            segment_interval_ms: self.segment_interval_ms.or(base.segment_interval_ms),
            segment_size: self.segment_size.or(base.segment_size),
            drop_percent: self.drop_percent.or(base.drop_percent),
            corrupt_percent: self.corrupt_percent.or(base.corrupt_percent),
            truncate_percent: self.truncate_percent.or(base.truncate_percent),
            jitter_ms: self.jitter_ms.or(base.jitter_ms),
            silent_after: self.silent_after.or(base.silent_after),
            close_after: self.close_after.or(base.close_after),
            ignore_trigger_percent: self
                .ignore_trigger_percent
                .or(base.ignore_trigger_percent),
        }
    }
}

/// A named group of settings that several simulated payloads share.
#[derive(Debug, Clone, Deserialize)]
pub struct SimGroup {
    pub name: String,
    #[serde(flatten)]
    pub settings: SimSettings,
    /// Whatever the file wrote here that is not a setting.
    ///
    /// Kept by name rather than discarded so that it can be reported. A
    /// misspelled setting is otherwise the worst kind of silence: the file
    /// says what was wanted, nothing complains, and the run does something
    /// else -- a payload asked to drop a tenth of its packets drops none, and
    /// looks exactly like one that was never asked.
    #[serde(flatten)]
    pub unknown: BTreeMap<String, IgnoredAny>,
}

/// One simulated payload: the data handler it stands in for, the group it takes
/// its settings from, and whatever it states for itself.
#[derive(Debug, Clone, Deserialize)]
pub struct SimPayload {
    /// Name of the data handler this payload simulates.
    pub name: String,
    /// Name of the group supplying settings this payload does not state.
    #[serde(default)]
    pub group: Option<String>,
    #[serde(flatten)]
    pub settings: SimSettings,
    /// Whatever the file wrote here that is not a setting; see
    /// [`SimGroup::unknown`].
    #[serde(flatten)]
    pub unknown: BTreeMap<String, IgnoredAny>,
}

/// A simulator configuration file, as it parses.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimConfigFile {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub simulated_payload_groups: Vec<SimGroup>,
    #[serde(default)]
    pub simulated_payloads: Vec<SimPayload>,
}

/// What simulating one data handler takes, with every value settled.
///
/// This is what a [`SimConfigFile`] and a payload configuration produce
/// together: no optional values are left, because a payload that reached this
/// point has a rate and a segmentation whether its own entry stated them, its
/// group stated them, or they follow from the handler's packet size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSim {
    /// Milliseconds between packets, for a payload that sends on its own.
    ///
    /// Zero for a triggered payload, which sends when it is asked and at no
    /// rate of its own: there is nothing for an interval to mean, which is
    /// why stating one in this file for such a payload is an error rather
    /// than a setting that happens not to be used.
    pub packet_interval_ms: u32,
    pub segment_interval_ms: u32,
    pub segment_size: u32,
    /// Whether this payload answers requests rather than sending on its own,
    /// which the payload configuration decides and this file may not.
    pub triggered: bool,
    /// What this payload does wrong. All zero for one that works.
    pub faults: Faults,
    /// The address the simulated payload binds for itself, where the file
    /// states one: a host for a UDP payload and a path for a Unix datagram
    /// one. `None` is any interface, or a path beside the handler's.
    pub payload_address: Option<String>,
    /// The port it binds, where the file states one. `None` is whichever the
    /// system gives.
    pub payload_port: Option<u16>,
}

/// The ways a simulated payload is asked to misbehave.
///
/// Zero is off throughout, so the default is a payload that works: a file
/// that says nothing about faults gets none, which is what every simulator
/// configuration written before these existed asked for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Faults {
    pub drop_percent: u32,
    pub corrupt_percent: u32,
    pub truncate_percent: u32,
    pub jitter_ms: u32,
    pub silent_after: u64,
    pub close_after: u64,
    pub ignore_trigger_percent: u32,
}

impl Faults {
    /// Whether anything at all is being injected, which is worth one line of
    /// log when a payload starts: a run whose payload was dropping a tenth of
    /// its packets should not have to be guessed at afterwards.
    pub fn any(&self) -> bool {
        *self != Faults::default()
    }
}

impl SimConfigFile {
    /// Read a simulator configuration file.
    pub fn load<P: AsRef<Path>>(path: P) -> TcsResult<Self> {
        load_config_file(path)
    }

    /// Settle every data handler's simulation settings.
    ///
    /// The handlers come from the payload configuration file and this file adds
    /// to them, so the result has one entry per handler, in the order the
    /// payload file gave them. Both directions of the join are checked: a
    /// handler nothing simulates is as much an error as a simulated payload
    /// naming no handler.
    pub fn resolve(
        &self,
        handlers: &[DHConfig],
    ) -> Result<Vec<ResolvedSim>, SimConfigError> {
        let (resolved, problems) = self.check(handlers);
        match problems.into_iter().next() {
            Some(problem) => Err(problem),
            None => Ok(resolved),
        }
    }

    /// Everything wrong with this file against those handlers.
    ///
    /// The same rules [`Self::resolve`] asks, asked for all of their answers
    /// rather than the first: for a reader checking a file rather than a
    /// program running one. See [`crate::verify`].
    pub fn problems(&self, handlers: &[DHConfig]) -> Vec<SimConfigError> {
        self.check(handlers).1
    }

    /// Every handler's settled simulation settings, and everything wrong.
    ///
    /// The settings are the ones that settled, so a file with a problem still
    /// yields the payloads that have none.
    fn check(&self, handlers: &[DHConfig]) -> (Vec<ResolvedSim>, Vec<SimConfigError>) {
        let mut problems: Vec<SimConfigError> = Vec::new();

        let mut groups: BTreeMap<&str, &SimSettings> = BTreeMap::new();
        for group in &self.simulated_payload_groups {
            if let Err(e) = nothing_unknown("simulated payload group", &group.name, &group.unknown)
            {
                problems.push(e);
            }
            // Kept even when something is wrong with it, so that the payloads
            // naming it are not also reported as naming a group that is not
            // there. One mistake, one problem.
            if groups.insert(group.name.as_str(), &group.settings).is_some() {
                problems.push(SimConfigError::Duplicate {
                    what: "simulated payload group",
                    name: group.name.clone(),
                });
            }
        }

        let mut payloads: BTreeMap<&str, &SimPayload> = BTreeMap::new();
        for payload in &self.simulated_payloads {
            if let Err(e) = nothing_unknown("simulated payload", &payload.name, &payload.unknown) {
                problems.push(e);
            }
            if payloads.insert(payload.name.as_str(), payload).is_some() {
                problems.push(SimConfigError::Duplicate {
                    what: "simulated payload",
                    name: payload.name.clone(),
                });
            }
        }

        // A name here that no data handler has would otherwise be a line with
        // no effect, which is what a misspelled handler name looks like.
        for name in payloads.keys() {
            if !handlers.iter().any(|dh| dh.name.0 == *name) {
                problems.push(SimConfigError::UnknownPayload {
                    payload: (*name).to_string(),
                });
            }
        }

        // One handler at a time, because one payload's settings are its own:
        // a reader is told what is wrong with each rather than what is wrong
        // with the first.
        let settle = |dh: &DHConfig| -> Result<ResolvedSim, SimConfigError> {
                let payload = payloads.get(dh.name.0.as_str()).ok_or_else(|| {
                    SimConfigError::Unsimulated {
                        handler: dh.name.0.clone(),
                    }
                })?;

                let settings = match &payload.group {
                    Some(group) => {
                        let base = groups.get(group.as_str()).ok_or_else(|| {
                            SimConfigError::UnknownGroup {
                                payload: payload.name.clone(),
                                group: group.clone(),
                            }
                        })?;
                        payload.settings.over(base)
                    }
                    None => payload.settings.clone(),
                };

                // What payload this is, before anything this entry says
                // about it is believed. The files are joined by name, and a
                // payload set copied from another keeps the names -- so the
                // kind is what tells a simulator file paired with the wrong
                // payload file from one paired with the right one.
                what_it_is(&payload.name, &settings, &dh.endpoint)?;

                // Which kind of payload this is was settled by the payload
                // configuration, and decides whether an interval here means
                // anything. A triggered payload sends when it is asked: an
                // interval for one is a setting with nothing to govern, and
                // the file that gave it has not understood which kind of
                // payload it is simulating.
                let triggered = dh.mode.polling().is_some();
                // Neither interval belongs to a payload that answers
                // requests: it sends when it is asked, so a rate here would
                // govern nothing, and the segments of one answer go as fast
                // as they can.
                if triggered {
                    for (setting, stated) in [
                        ("packet_interval_ms", settings.packet_interval_ms.is_some()),
                        ("segment_interval_ms", settings.segment_interval_ms.is_some()),
                    ] {
                        if stated {
                            return Err(SimConfigError::IntervalForATriggeredPayload {
                                payload: payload.name.clone(),
                                setting,
                            });
                        }
                    }
                }

                let packet_interval_ms = match (triggered, settings.packet_interval_ms) {
                    (true, Some(_)) => {
                        return Err(SimConfigError::IntervalForATriggeredPayload {
                            payload: payload.name.clone(),
                            setting: "packet_interval_ms",
                        })
                    }
                    (true, None) => 0,
                    (false, Some(interval)) => interval,
                    (false, None) => {
                        return Err(SimConfigError::NoPacketInterval {
                            payload: payload.name.clone(),
                        })
                    }
                };

                // A packet is one segment unless the file divides it, and
                // segments arrive at the packet rate unless it says otherwise.
                let segment_size = settings
                    .segment_size
                    .unwrap_or_else(|| u32::try_from(dh.packet_size).unwrap_or(u32::MAX));

                // What this payload is asked to do wrong, and whether it is
                // a payload that could do it. A percentage above a hundred is
                // a misunderstanding rather than a severe fault, and a fault
                // a kind cannot have is a setting that would govern nothing.
                let faults = Faults {
                    drop_percent: a_percentage(&payload.name, "drop_percent",
                        settings.drop_percent)?,
                    corrupt_percent: a_percentage(&payload.name, "corrupt_percent",
                        settings.corrupt_percent)?,
                    truncate_percent: a_percentage(&payload.name, "truncate_percent",
                        settings.truncate_percent)?,
                    jitter_ms: settings.jitter_ms.unwrap_or(0),
                    silent_after: settings.silent_after.unwrap_or(0),
                    close_after: settings.close_after.unwrap_or(0),
                    ignore_trigger_percent: a_percentage(
                        &payload.name,
                        "ignore_trigger_percent",
                        settings.ignore_trigger_percent,
                    )?,
                };

                // A payload that is not asked for anything has no requests to
                // ignore, and the setting would sit there looking as though
                // it did something.
                if faults.ignore_trigger_percent > 0 && !triggered {
                    return Err(SimConfigError::FaultForTheWrongPayload {
                        payload: payload.name.clone(),
                        fault: "ignore_trigger_percent",
                        why: "this payload sends on its own, so it is never asked \
                              for anything to ignore",
                    });
                }

                // And closing a link needs a link that can be closed. A
                // datagram socket has none, and a device, a bus or a
                // pseudo-terminal is opened by the handler rather than
                // connected to.
                if faults.close_after > 0 && !has_a_connection(&dh.endpoint) {
                    return Err(SimConfigError::FaultForTheWrongPayload {
                        payload: payload.name.clone(),
                        fault: "close_after",
                        why: "this payload has no connection to close: only a \
                              stream has one, where the handler connects and the \
                              payload may hang up",
                    });
                }

                Ok(ResolvedSim {
                    payload_address: settings.payload_address.clone(),
                    payload_port: settings.payload_port,
                    packet_interval_ms,
                    segment_interval_ms: settings
                        .segment_interval_ms
                        .unwrap_or(packet_interval_ms),
                    segment_size,
                    triggered,
                    faults,
                })
        };

        let mut resolved: Vec<ResolvedSim> = Vec::new();
        for dh in handlers {
            match settle(dh) {
                Ok(settings) => resolved.push(settings),
                Err(e) => problems.push(e),
            }
        }

        // A group no payload names has no effect on the simulation, which is
        // exactly what a group whose name a payload misspelled looks like.
        // Checked after the payloads, so that a reader following the problems
        // in order meets the misspelling at the payload's end first, where
        // the name actually is.
        let named: BTreeSet<&str> = self
            .simulated_payloads
            .iter()
            .filter_map(|payload| payload.group.as_deref())
            .collect();
        for unused in self
            .simulated_payload_groups
            .iter()
            .filter(|group| !named.contains(group.name.as_str()))
        {
            problems.push(SimConfigError::UnusedGroup {
                group: unused.name.clone(),
            });
        }

        (resolved, problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DHId, DHMode, DHName, DeviceConfig, EndpointConfig, NetworkConfig, NetworkProtocol,
    };

    fn handler(name: &str, packet_size: usize) -> DHConfig {
        DHConfig {
            dh_id: DHId(0),
            name: DHName::new(name),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: "localhost".to_string(),
                port: 5000,
            }),
            packet_size,
            oc: None,
            mode: Default::default(),
        }
    }

    /// Parse YAML the way a loaded file is parsed, so these tests exercise
    /// the same deserialization a real file goes through.
    fn parse(text: &str) -> SimConfigFile {
        crate::ConfigFormat::Yaml.parse(text).expect("parses")
    }

    #[test]
    fn a_payload_takes_its_groups_settings() {
        let file = parse(
            "
simulated_payload_groups:
  - name: steady_1hz
    packet_interval_ms: 1000
    segment_interval_ms: 250
simulated_payloads:
  - name: DH0
    type: network
    protocol: udp
    group: steady_1hz
",
        );

        let resolved = file.resolve(&[handler("DH0", 12)]).unwrap();
        assert_eq!(
            resolved[0],
            ResolvedSim {
                packet_interval_ms: 1000,
                segment_interval_ms: 250,
                // Unstated, so a packet is one segment.
                segment_size: 12,
                triggered: false,
                faults: Default::default(),
                // Nothing stated, so the payload answers from whatever the
                // system gives it.
                payload_address: None,
                payload_port: None,
            }
        );
    }

    #[test]
    fn a_payload_overrides_its_group() {
        let file = parse(
            "
simulated_payload_groups:
  - name: steady_1hz
    packet_interval_ms: 1000
    segment_interval_ms: 1000
simulated_payloads:
  - name: DH0
    type: network
    protocol: udp
    group: steady_1hz
    packet_interval_ms: 500
    segment_size: 4
",
        );

        let resolved = file.resolve(&[handler("DH0", 12)]).unwrap();
        assert_eq!(resolved[0].packet_interval_ms, 500);
        assert_eq!(resolved[0].segment_size, 4);
        // Not overridden, so still the group's.
        assert_eq!(resolved[0].segment_interval_ms, 1000);
    }

    #[test]
    fn a_payload_needs_no_group() {
        let file = parse(
            "
simulated_payloads:
  - name: DH0
    type: network
    protocol: udp
    packet_interval_ms: 500
",
        );

        let resolved = file.resolve(&[handler("DH0", 8)]).unwrap();
        // The segment interval follows the packet interval when unstated.
        assert_eq!(resolved[0].segment_interval_ms, 500);
        assert_eq!(resolved[0].segment_size, 8);
    }

    #[test]
    fn settings_are_matched_to_handlers_by_name_not_by_order() {
        let file = parse(
            "
simulated_payloads:
  - name: DH1
    type: network
    protocol: udp
    packet_interval_ms: 1
  - name: DH0
    type: network
    protocol: udp
    packet_interval_ms: 0
",
        );

        let resolved = file
            .resolve(&[handler("DH0", 1), handler("DH1", 2)])
            .unwrap();
        // Resolved in handler order, with each handler's own settings.
        assert_eq!(resolved[0].packet_interval_ms, 0);
        assert_eq!(resolved[1].packet_interval_ms, 1);
    }

    #[test]
    fn a_handler_nothing_simulates_is_an_error() {
        let file = parse(
            "
simulated_payloads:
  - name: DH0
    type: network
    protocol: udp
    packet_interval_ms: 0
",
        );

        assert_eq!(
            file.resolve(&[handler("DH0", 1), handler("DH1", 1)]),
            Err(SimConfigError::Unsimulated {
                handler: "DH1".to_string()
            })
        );
    }

    #[test]
    fn a_name_matching_no_handler_is_an_error() {
        let file = parse(
            "
simulated_payloads:
  - name: DH0
    type: network
    protocol: udp
    packet_interval_ms: 0
  - name: DH7
    type: network
    protocol: udp
    packet_interval_ms: 0
",
        );

        assert_eq!(
            file.resolve(&[handler("DH0", 1)]),
            Err(SimConfigError::UnknownPayload {
                payload: "DH7".to_string()
            })
        );
    }

    #[test]
    fn an_undefined_group_is_an_error() {
        let file = parse(
            "
simulated_payloads:
  - name: DH0
    type: network
    protocol: udp
    group: nonesuch
",
        );

        assert_eq!(
            file.resolve(&[handler("DH0", 1)]),
            Err(SimConfigError::UnknownGroup {
                payload: "DH0".to_string(),
                group: "nonesuch".to_string(),
            })
        );
    }

    /// A device handler, for the tests that need a kind that is not network.
    fn device(name: &str) -> DHConfig {
        let mut dh = handler(name, 4);
        dh.endpoint = EndpointConfig::Device(DeviceConfig {
            path: "/dev/urandom".to_string(),
        });
        dh
    }

    /// The payload configuration's `address` and `port`, written here, are
    /// answered with the file they belong in.
    ///
    /// They are the two attributes someone writing a simulator file beside a
    /// payload file is likeliest to copy across, and the simulator takes
    /// where a payload is from that file -- so such a line is not so much a
    /// mistake as a statement in the wrong file, and is answered as one. The
    /// answer also names what this file may say instead, the two being easy
    /// to confuse.
    #[test]
    fn where_the_payload_is_belongs_in_the_other_file() {
        for plain in ["address: localhost", "port: 5000"] {
            let file = parse(&format!(
                "simulated_payloads:\n  - name: DH0\n    type: network\n    \
                 protocol: udp\n    packet_interval_ms: 100\n    {plain}\n"
            ));
            let said = format!("{}", file.resolve(&[handler("DH0", 4)]).unwrap_err());
            assert!(
                said.contains("payload configuration file") && said.contains("DH0"),
                "writing {plain} should name the file it belongs in: {said}"
            );
            assert!(
                said.contains("payload_address") && said.contains("payload_port"),
                "and what this file may say instead: {said}"
            );
        }

        // A group is read the same way.
        let file = parse(
            "simulated_payload_groups:\n  - name: g\n    packet_interval_ms: 100\n    \
             port: 5000\n\
             simulated_payloads:\n  - name: DH0\n    type: network\n    protocol: udp\n    \
             group: g\n",
        );
        let said = format!("{}", file.resolve(&[handler("DH0", 4)]).unwrap_err());
        assert!(
            said.contains("payload configuration file") && said.contains("\"g\""),
            "the error should name the group: {said}"
        );
    }

    /// A datagram payload may be given the socket it answers from.
    ///
    /// The other end of the link from the one the payload file states: a
    /// datagram payload reaches out to a handler that is waiting, so its own
    /// address is otherwise whatever the system gives it, and nothing else
    /// says what it should be.
    #[test]
    fn a_datagram_payload_may_be_told_what_to_bind() {
        let settled = |extra: &str, dh: &DHConfig| {
            parse(&format!(
                "simulated_payloads:\n  - name: DH0\n    type: network\n    \
                 protocol: udp\n    packet_interval_ms: 100\n{extra}"
            ))
            .resolve(std::slice::from_ref(dh))
        };
        let udp = handler("DH0", 4);

        // Nothing stated, which is what every simulator file asked for before
        // this existed: any interface, and a port the system chooses.
        let plain = settled("", &udp).expect("resolves");
        assert_eq!(
            (plain[0].payload_address.as_deref(), plain[0].payload_port),
            (None, None)
        );

        let told = settled("    payload_address: 127.0.0.1\n    payload_port: 7000\n", &udp)
            .expect("resolves");
        assert_eq!(told[0].payload_address.as_deref(), Some("127.0.0.1"));
        assert_eq!(told[0].payload_port, Some(7000));

        // A port alone is the common case -- bind this port on any interface
        // -- and an address alone leaves the port to the system.
        assert!(settled("    payload_port: 7000\n", &udp).is_ok());
        assert!(settled("    payload_address: 127.0.0.1\n", &udp).is_ok());

        // But not the handler's own socket: two sockets cannot be one, and a
        // payload told to bind it would fail at the bind with nothing to say
        // about which file was wrong.
        let said = format!(
            "{}",
            settled("    payload_address: localhost\n    payload_port: 5000\n", &udp)
                .unwrap_err()
        );
        assert!(
            said.contains("the handler's own end") && said.contains("5000"),
            "{said}"
        );
    }

    /// Only a payload with an address of its own to bind may be told what to
    /// bind.
    #[test]
    fn only_a_datagram_payload_binds_an_address_of_its_own() {
        // A stream payload is the end that waits, so it binds the address the
        // payload configuration gives it; another stated here would be two
        // answers to where it is.
        let mut stream = handler("DH0", 4);
        stream.endpoint = EndpointConfig::Network(NetworkConfig {
            protocol: NetworkProtocol::Tcp,
            address: "localhost".to_string(),
            port: 5000,
        });
        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    type: network\n    protocol: tcp\n    \
             packet_interval_ms: 100\n    payload_port: 7000\n",
        );
        let said = format!("{}", file.resolve(&[stream]).unwrap_err());
        assert!(
            said.contains("payload_port") && said.contains("the end that waits"),
            "{said}"
        );

        // A Unix datagram payload answers from a path, so it takes an address
        // and no port.
        let mut socket = handler("DH0", 4);
        socket.endpoint = EndpointConfig::Network(NetworkConfig {
            protocol: NetworkProtocol::UnixDgram,
            address: "/tmp/dh.sock".to_string(),
            port: 0,
        });
        let path = |extra: &str| {
            parse(&format!(
                "simulated_payloads:\n  - name: DH0\n    type: network\n    \
                 protocol: unix_dgram\n    packet_interval_ms: 100\n{extra}"
            ))
            .resolve(std::slice::from_ref(&socket))
        };
        assert!(path("    payload_address: /tmp/dh-payload.sock\n").is_ok());

        let said = format!("{}", path("    payload_port: 7000\n").unwrap_err());
        assert!(
            said.contains("payload_port") && said.contains("named by a path"),
            "{said}"
        );

        // And not the handler's own path.
        let said = format!("{}", path("    payload_address: /tmp/dh.sock\n").unwrap_err());
        assert!(said.contains("/tmp/dh.sock"), "{said}");

        // A device has no address at all.
        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    type: device\n    \
             packet_interval_ms: 0\n    payload_port: 7000\n",
        );
        let said = format!("{}", file.resolve(&[device("DH0")]).unwrap_err());
        assert!(said.contains("device") && said.contains("network"), "{said}");
    }

    /// A simulator file names the payloads of its own set, and two sets may
    /// name theirs alike.
    ///
    /// The rule that a name is defined once reaches no further than the file.
    /// Each simulator file is resolved against the payload file beside it, so
    /// two sets using the same names are two sets, not a clash -- and the
    /// shipped sets rely on it.
    #[test]
    fn two_simulator_files_may_name_their_payloads_alike() {
        let a_set = |interval: u32| {
            parse(&format!(
                "simulated_payload_groups:\n  - name: steady\n    \
                 packet_interval_ms: {interval}\n\
                 simulated_payloads:\n  - name: DH0\n    type: network\n    \
                 protocol: udp\n    group: steady\n"
            ))
        };

        // One name and one group name, in two files that know nothing of each
        // other, each resolved against its own payload file.
        let first = a_set(1000)
            .resolve(&[handler("DH0", 12)])
            .expect("the first set");
        let second = a_set(250)
            .resolve(&[handler("DH0", 8)])
            .expect("the second set");

        assert_eq!(first[0].packet_interval_ms, 1000);
        assert_eq!(
            second[0].packet_interval_ms, 250,
            "the two sets simulate different payloads under the one name"
        );
    }

    /// A misspelled setting is refused, not ignored.
    ///
    /// The worst silence in either file: it says what was wanted, nothing
    /// complains, and the run does something else. A payload asked to drop a
    /// tenth of its packets through a misspelling drops none, and looks
    /// exactly like one that was never asked.
    #[test]
    fn a_word_that_is_not_a_setting_is_refused() {
        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    type: network\n    \
             protocol: udp\n    packet_interval_ms: 100\n    drop_percnt: 10\n",
        );
        let said = format!("{}", file.resolve(&[handler("DH0", 4)]).unwrap_err());
        assert!(
            said.contains("drop_percnt") && said.contains("not a setting"),
            "the error should name the word it does not know: {said}"
        );

        // A group's settings are read the same way, and a group is where a
        // misspelling hides best: it is read once and applies to every
        // payload that names it.
        let file = parse(
            "simulated_payload_groups:\n  - name: slow\n    packet_intervl_ms: 100\n\
             simulated_payloads:\n  - name: DH0\n    type: network\n    \
             protocol: udp\n    group: slow\n",
        );
        let said = format!("{}", file.resolve(&[handler("DH0", 4)]).unwrap_err());
        assert!(
            said.contains("packet_intervl_ms") && said.contains("slow"),
            "the error should name the group and the word: {said}"
        );
    }

    /// And the sections of the file itself, for the same reason.
    #[test]
    fn a_section_that_is_not_a_section_is_refused() {
        let text = "simulated_payload:\n  - name: DH0\n    packet_interval_ms: 100\n";
        let e = crate::ConfigFormat::Yaml
            .parse::<SimConfigFile>(text)
            .expect_err("a file whose one section is misspelled describes nothing");
        let said = format!("{e}");
        assert!(
            said.contains("simulated_payload"),
            "the error should name the section: {said}"
        );
    }

    /// A simulator file paired with the wrong payload file is caught.
    ///
    /// This is what the kind and the transport are stated twice for. Two
    /// payload sets that differ only in how their payload is reached have the
    /// same handler names -- one is commonly a copy of the other -- and the
    /// two files are joined by name alone, so nothing else in either file
    /// would notice the mismatch. The simulator would bind a datagram socket
    /// for a handler that is going to connect to a stream, and the run would
    /// look like a payload that never sends.
    #[test]
    fn a_simulator_file_paired_with_the_wrong_payload_file_is_caught() {
        // The simulator file of the datagram set, against the payload file of
        // the stream set.
        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    type: network\n    \
             protocol: udp\n    packet_interval_ms: 1000\n",
        );

        let mut stream = handler("DH0", 4);
        stream.endpoint = EndpointConfig::Network(NetworkConfig {
            protocol: NetworkProtocol::Tcp,
            address: "localhost".to_string(),
            port: 5000,
        });

        let said = format!("{}", file.resolve(&[stream]).unwrap_err());
        assert!(
            said.contains("udp") && said.contains("tcp") && said.contains("DH0"),
            "the error should name the payload and both transports: {said}"
        );

        // And the same file against the payload it was written for.
        let datagram = handler("DH0", 4);
        file.resolve(&[datagram])
            .expect("the file its payload file was written for");
    }

    /// A disagreement about the kind itself, which is the same mistake one
    /// step larger.
    #[test]
    fn the_two_files_cannot_disagree_about_the_kind() {
        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    type: device\n    \
             packet_interval_ms: 1000\n",
        );
        let said = format!("{}", file.resolve(&[handler("DH0", 4)]).unwrap_err());
        assert!(
            said.contains("device") && said.contains("network"),
            "the error should name the kind each file gives: {said}"
        );
    }

    /// Saying nothing is not agreement.
    ///
    /// A payload that stated no kind could not be checked against anything,
    /// so it is refused -- and told which kind the payload file makes it,
    /// since that is the one thing an error here always knows.
    #[test]
    fn a_simulated_payload_says_what_it_stands_in_for() {
        let file = parse("simulated_payloads:\n  - name: DH0\n    packet_interval_ms: 1000\n");
        let said = format!("{}", file.resolve(&[handler("DH0", 4)]).unwrap_err());
        assert!(
            said.contains("type: network"),
            "the error should say what to write: {said}"
        );

        // A network payload says which transport, too, for the same reason:
        // the kind alone would not have caught the pairing above.
        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    type: network\n    \
             packet_interval_ms: 1000\n",
        );
        let said = format!("{}", file.resolve(&[handler("DH0", 4)]).unwrap_err());
        assert!(
            said.contains("protocol: udp"),
            "the error should say which transport to write: {said}"
        );
    }

    /// A transport belongs to a network payload and to no other kind.
    #[test]
    fn only_a_network_payload_has_a_transport() {
        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    type: device\n    \
             protocol: udp\n    packet_interval_ms: 0\n",
        );
        let said = format!("{}", file.resolve(&[device("DH0")]).unwrap_err());
        assert!(
            said.contains("device") && said.contains("protocol"),
            "{said}"
        );

        // And a device that states none resolves, which is the whole of what
        // a device has to say about how it is reached.
        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    type: device\n    packet_interval_ms: 0\n",
        );
        file.resolve(&[device("DH0")]).expect("a device needs no transport");
    }

    /// A word that names no kind, or no transport, is refused with the words
    /// that would have worked.
    #[test]
    fn a_kind_or_a_transport_that_is_neither_is_refused() {
        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    type: netwrok\n    \
             packet_interval_ms: 1000\n",
        );
        let said = format!("{}", file.resolve(&[handler("DH0", 4)]).unwrap_err());
        assert!(
            said.contains("netwrok") && said.contains("network") && said.contains("i2c"),
            "the error should list the kinds: {said}"
        );

        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    type: network\n    \
             protocol: udb\n    packet_interval_ms: 1000\n",
        );
        let said = format!("{}", file.resolve(&[handler("DH0", 4)]).unwrap_err());
        assert!(
            said.contains("udb") && said.contains("unix_dgram"),
            "the error should list the transports: {said}"
        );
    }

    /// What a payload stands in for may come from its group, like everything
    /// else a payload does not state.
    #[test]
    fn a_group_may_carry_what_its_payloads_stand_in_for() {
        let file = parse(
            "simulated_payload_groups:\n  - name: datagrams\n    type: network\n    \
             protocol: udp\n    packet_interval_ms: 1000\n\
             simulated_payloads:\n  - name: DH0\n    group: datagrams\n  \
             - name: DH1\n    group: datagrams\n",
        );
        file.resolve(&[handler("DH0", 4), handler("DH1", 4)])
            .expect("a group of payloads reached the same way");
    }

    /// The faults are the simulator's alone, and are checked for being
    /// possible before a run is started on them.
    #[test]
    fn a_fault_is_checked_before_a_run_is_started_on_it() {
        // A percentage is nought to a hundred. Anything else is a
        // misunderstanding rather than a severe fault.
        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    type: network\n    protocol: udp\n    packet_interval_ms: 100\n    \
             drop_percent: 150\n",
        );
        let e = file.resolve(&[handler("DH0", 4)]).unwrap_err();
        let said = format!("{e}");
        assert!(
            said.contains("percentage") && said.contains("150"),
            "{said}"
        );

        // A payload that sends on its own is never asked for anything, so it
        // has no request to ignore.
        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    type: network\n    protocol: udp\n    packet_interval_ms: 100\n    \
             ignore_trigger_percent: 50\n",
        );
        let e = file.resolve(&[handler("DH0", 4)]).unwrap_err();
        let said = format!("{e}");
        assert!(
            said.contains("ignore_trigger_percent") && said.contains("sends on its own"),
            "{said}"
        );

        // And hanging up needs a link to hang up. handler() makes a UDP
        // payload, which has none.
        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    type: network\n    protocol: udp\n    packet_interval_ms: 100\n    \
             close_after: 10\n",
        );
        let e = file.resolve(&[handler("DH0", 4)]).unwrap_err();
        let said = format!("{e}");
        assert!(
            said.contains("close_after") && said.contains("no connection to close"),
            "{said}"
        );
    }

    /// The faults a file states reach the payload, and a group carries them
    /// for the payloads that share them.
    #[test]
    fn the_faults_reach_the_payload_through_its_group() {
        let stream = |name: &str| DHConfig {
            dh_id: DHId(0),
            name: DHName::new(name),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Tcp,
                address: "localhost".to_string(),
                port: 5000,
            }),
            packet_size: 4,
            oc: None,
            mode: DHMode::Periodic,
        };

        let file = parse(
            "simulated_payload_groups:\n  - name: flaky\n    packet_interval_ms: 100\n    \
             drop_percent: 10\n    jitter_ms: 25\n\
             simulated_payloads:\n  - name: DH0\n    type: network\n    protocol: tcp\n    group: flaky\n    \
             close_after: 7\n",
        );

        let resolved = file.resolve(&[stream("DH0")]).expect("resolves");
        assert_eq!(
            resolved[0].faults,
            Faults {
                drop_percent: 10,
                jitter_ms: 25,
                close_after: 7,
                ..Faults::default()
            },
            "a payload takes its group's faults and states its own beside them"
        );
        assert!(resolved[0].faults.any());
    }

    /// A file that says nothing about faults asks for a payload that works,
    /// which is what every simulator configuration written before these
    /// existed asked for.
    #[test]
    fn a_file_with_no_faults_asks_for_a_payload_that_works() {
        let file = parse("simulated_payloads:\n  - name: DH0\n    type: network\n    protocol: udp\n    packet_interval_ms: 100\n");
        let resolved = file.resolve(&[handler("DH0", 4)]).expect("resolves");
        assert_eq!(resolved[0].faults, Faults::default());
        assert!(!resolved[0].faults.any());
    }

    /// Neither interval belongs to a triggered payload here.
    ///
    /// It sends when it is asked, so a packet interval would govern nothing,
    /// and the segments of one answer go as fast as they can. The rate such a
    /// payload is asked at is the payload configuration's packet interval,
    /// which is the whole of the division: one name, in whichever file the
    /// payload's kind puts it.
    #[test]
    fn a_triggered_payload_has_no_timing_here_at_all() {
        let triggered = |name: &str| DHConfig {
            dh_id: DHId(0),
            name: DHName::new(name),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Tcp,
                address: "localhost".to_string(),
                port: 5000,
            }),
            packet_size: 4,
            oc: None,
            mode: DHMode::Triggered {
                trigger: b"READ\r".to_vec(),
                interval_ms: 500,
            },
        };

        for stated in ["packet_interval_ms: 1000", "segment_interval_ms: 1000"] {
            let named = stated.split(':').next().unwrap();

            // Stated by the payload itself.
            let file = parse(&format!(
                "simulated_payloads:\n  - name: DH0\n    type: network\n    \
                 protocol: tcp\n    {stated}\n"
            ));
            let said = format!("{}", file.resolve(&[triggered("DH0")]).unwrap_err());
            assert!(
                said.contains(named) && said.contains("packet_interval_ms in the payload"),
                "{stated}: {said}"
            );

            // And taken from the group it names, which reaches the payload
            // exactly as its own would: a payload that inherited a rate it
            // may not state would be governed by a group it
            // only joined for the settings it may.
            let file = parse(&format!(
                "simulated_payload_groups:\n  - name: driven\n    {stated}\n\
                 simulated_payloads:\n  - name: DH0\n    type: network\n    \
                 protocol: tcp\n    group: driven\n"
            ));
            let said = format!("{}", file.resolve(&[triggered("DH0")]).unwrap_err());
            assert!(
                said.contains(named) && said.contains("packet_interval_ms in the payload"),
                "{stated}, inherited: {said}"
            );
        }

        // What such a payload may still say is how it divides an answer:
        // that is its own business, however it was prompted.
        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    type: network\n    protocol: tcp\n    \
             segment_size: 2\n",
        );
        let resolved = file
            .resolve(&[triggered("DH0")])
            .expect("a segment size is not a rate");
        assert_eq!(resolved[0].segment_size, 2);
        assert_eq!(resolved[0].packet_interval_ms, 0, "it sends when it is asked");
        assert_eq!(resolved[0].segment_interval_ms, 0, "and its segments as fast as they can");
    }

    /// A triggered payload takes no interval here, and a periodic one must
    /// have one. Which kind it is, this file does not decide.
    ///
    /// The division is the point: how fast tcspecial polls a payload is
    /// flight behaviour and is written in the payload configuration, while
    /// how fast a simulated payload produces data of its own is a choice
    /// about the simulation and is written here. A file that stated an
    /// interval for a triggered payload would be making a choice it does not
    /// have, and the setting would govern nothing.
    #[test]
    fn a_triggered_payload_takes_no_interval_here() {
        let triggered = |name: &str| DHConfig {
            dh_id: DHId(0),
            name: DHName::new(name),
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/urandom".to_string(),
            }),
            packet_size: 4,
            oc: None,
            mode: DHMode::Triggered {
                trigger: b"READ".to_vec(),
                interval_ms: 500,
            },
        };

        // Stated, and refused in terms of where it belongs.
        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    type: device\n    packet_interval_ms: 1000\n",
        );
        let e = file.resolve(&[triggered("DH0")]).unwrap_err();
        let said = format!("{e}");
        assert!(
            said.contains("payload configuration") && said.contains("triggered"),
            "{said}"
        );

        // Not stated, which is what a triggered payload's settings look like:
        // the segment settings still apply, since how a payload divides an
        // answer is its own business.
        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    type: device\n    segment_size: 2\n",
        );
        let resolved = file
            .resolve(&[triggered("DH0")])
            .expect("a triggered payload needs no interval");
        assert!(resolved[0].triggered);
        assert_eq!(resolved[0].packet_interval_ms, 0, "it sends when it is asked");
        assert_eq!(resolved[0].segment_size, 2);
    }

    #[test]
    fn a_payload_with_no_interval_anywhere_is_an_error() {
        let file = parse(
            "
simulated_payload_groups:
  - name: sizes_only
    segment_size: 4
simulated_payloads:
  - name: DH0
    type: network
    protocol: udp
    group: sizes_only
",
        );

        assert_eq!(
            file.resolve(&[handler("DH0", 8)]),
            Err(SimConfigError::NoPacketInterval {
                payload: "DH0".to_string()
            })
        );
    }

    #[test]
    fn a_group_no_payload_names_is_an_error() {
        let file = parse(
            "
simulated_payload_groups:
  - name: steady_1hz
    packet_interval_ms: 1000
  - name: continuous
    packet_interval_ms: 0
simulated_payloads:
  - name: DH0
    type: network
    protocol: udp
    group: steady_1hz
",
        );

        assert_eq!(
            file.resolve(&[handler("DH0", 12)]),
            Err(SimConfigError::UnusedGroup {
                group: "continuous".to_string()
            })
        );
    }

    #[test]
    fn a_misspelled_group_is_reported_from_the_payload_not_the_group() {
        // The typo leaves the group unnamed and the name undefined at once;
        // the payload's end is where the misspelling actually is.
        let file = parse(
            "
simulated_payload_groups:
  - name: steady_1hz
    packet_interval_ms: 1000
simulated_payloads:
  - name: DH0
    type: network
    protocol: udp
    group: steady_1hx
",
        );

        assert_eq!(
            file.resolve(&[handler("DH0", 12)]),
            Err(SimConfigError::UnknownGroup {
                payload: "DH0".to_string(),
                group: "steady_1hx".to_string(),
            })
        );
    }

    #[test]
    fn a_repeated_name_is_an_error() {
        let groups = parse(
            "
simulated_payload_groups:
  - name: g
    packet_interval_ms: 1
  - name: g
    packet_interval_ms: 2
simulated_payloads:
  - name: DH0
    type: network
    protocol: udp
    group: g
",
        );
        assert_eq!(
            groups.resolve(&[handler("DH0", 1)]),
            Err(SimConfigError::Duplicate {
                what: "simulated payload group",
                name: "g".to_string()
            })
        );

        let payloads = parse(
            "
simulated_payloads:
  - name: DH0
    type: network
    protocol: udp
    packet_interval_ms: 1
  - name: DH0
    type: network
    protocol: udp
    packet_interval_ms: 2
",
        );
        assert_eq!(
            payloads.resolve(&[handler("DH0", 1)]),
            Err(SimConfigError::Duplicate {
                what: "simulated payload",
                name: "DH0".to_string()
            })
        );
    }

    #[test]
    fn a_device_handlers_packet_size_still_sizes_its_segments() {
        let file = parse(
            "
simulated_payloads:
  - name: DEV
    type: device
    packet_interval_ms: 0
",
        );

        let mut dh = handler("DEV", 1);
        dh.endpoint = EndpointConfig::Device(DeviceConfig {
            path: "/dev/urandom".to_string(),
        });

        let resolved = file.resolve(&[dh]).unwrap();
        assert_eq!(resolved[0].segment_size, 1);
    }
}
