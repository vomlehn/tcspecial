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
//!     group: steady_1hz
//!   - name: DH3
//!     packet_interval_ms: 500
//! ```
//!
//! A group carries what several simulated payloads have in common; a payload
//! overrides any of it for itself. The format is chosen from the file extension
//! by [`tcslibgs::load_config_file`], so the same configuration can be written
//! in YAML, JSON, or XML.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use serde::Deserialize;
use tcslibgs::{load_config_file, DHConfig, EndpointConfig, NetworkProtocol, TcsResult};

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
    IntervalForATriggeredPayload { payload: String },
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
            SimConfigError::IntervalForATriggeredPayload { payload } => write!(
                f,
                "simulated payload \"{}\" is triggered, so it sends when tcspecial \
                 asks and has no interval of its own: the trigger and how often it \
                 is sent are in the payload configuration, and nothing about the \
                 timing of this payload belongs here",
                payload
            ),
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

/// Settings a group or a simulated payload may state.
///
/// Every one is optional in the file: a payload states what it does not take
/// from its group, and a group states what its payloads share.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct SimSettings {
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
}

/// A simulator configuration file, as it parses.
#[derive(Debug, Clone, Deserialize)]
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
        let mut groups: BTreeMap<&str, &SimSettings> = BTreeMap::new();
        for group in &self.simulated_payload_groups {
            if groups.insert(group.name.as_str(), &group.settings).is_some() {
                return Err(SimConfigError::Duplicate {
                    what: "simulated payload group",
                    name: group.name.clone(),
                });
            }
        }

        let mut payloads: BTreeMap<&str, &SimPayload> = BTreeMap::new();
        for payload in &self.simulated_payloads {
            if payloads.insert(payload.name.as_str(), payload).is_some() {
                return Err(SimConfigError::Duplicate {
                    what: "simulated payload",
                    name: payload.name.clone(),
                });
            }
        }

        // A name here that no data handler has would otherwise be a line with
        // no effect, which is what a misspelled handler name looks like.
        for name in payloads.keys() {
            if !handlers.iter().any(|dh| dh.name.0 == *name) {
                return Err(SimConfigError::UnknownPayload {
                    payload: (*name).to_string(),
                });
            }
        }

        let resolved: Vec<ResolvedSim> = handlers
            .iter()
            .map(|dh| {
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

                // Which kind of payload this is was settled by the payload
                // configuration, and decides whether an interval here means
                // anything. A triggered payload sends when it is asked: an
                // interval for one is a setting with nothing to govern, and
                // the file that gave it has not understood which kind of
                // payload it is simulating.
                let triggered = dh.mode.polling().is_some();
                let packet_interval_ms = match (triggered, settings.packet_interval_ms) {
                    (true, Some(_)) => {
                        return Err(SimConfigError::IntervalForATriggeredPayload {
                            payload: payload.name.clone(),
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
                    packet_interval_ms,
                    segment_interval_ms: settings
                        .segment_interval_ms
                        .unwrap_or(packet_interval_ms),
                    segment_size,
                    triggered,
                    faults,
                })
            })
            .collect::<Result<_, SimConfigError>>()?;

        // A group no payload names has no effect on the simulation, which is
        // exactly what a group whose name a payload misspelled looks like.
        // Checked after the payloads, so that the misspelling is reported from
        // the payload's end, where the name actually is.
        let named: BTreeSet<&str> = self
            .simulated_payloads
            .iter()
            .filter_map(|payload| payload.group.as_deref())
            .collect();
        if let Some(unused) = self
            .simulated_payload_groups
            .iter()
            .find(|group| !named.contains(group.name.as_str()))
        {
            return Err(SimConfigError::UnusedGroup {
                group: unused.name.clone(),
            });
        }

        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslibgs::{
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
        tcslibgs::ConfigFormat::Yaml.parse(text).expect("parses")
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
    packet_interval_ms: 1
  - name: DH0
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
    packet_interval_ms: 0
  - name: DH7
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

    /// The faults are the simulator's alone, and are checked for being
    /// possible before a run is started on them.
    #[test]
    fn a_fault_is_checked_before_a_run_is_started_on_it() {
        // A percentage is nought to a hundred. Anything else is a
        // misunderstanding rather than a severe fault.
        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    packet_interval_ms: 100\n    \
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
            "simulated_payloads:\n  - name: DH0\n    packet_interval_ms: 100\n    \
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
            "simulated_payloads:\n  - name: DH0\n    packet_interval_ms: 100\n    \
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
             simulated_payloads:\n  - name: DH0\n    group: flaky\n    \
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
        let file = parse("simulated_payloads:\n  - name: DH0\n    packet_interval_ms: 100\n");
        let resolved = file.resolve(&[handler("DH0", 4)]).expect("resolves");
        assert_eq!(resolved[0].faults, Faults::default());
        assert!(!resolved[0].faults.any());
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
                trigger: "READ".to_string(),
                interval_ms: 500,
            },
        };

        // Stated, and refused in terms of where it belongs.
        let file = parse(
            "simulated_payloads:\n  - name: DH0\n    packet_interval_ms: 1000\n",
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
            "simulated_payloads:\n  - name: DH0\n    segment_size: 2\n",
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
    packet_interval_ms: 1
  - name: DH0
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
