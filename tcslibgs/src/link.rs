//! Commands and answers, as they go on the link.
//!
//! What the MOC sends tcspecial and what tcspecial sends back, each a CCSDS
//! Space Packet. The layouts are stated once, in XTCE, in
//! `tcslibgs/xtce/link.xml`, and packed and unpacked by the code tcs-xtce
//! generates from it, `link_packet.rs`, which allocates nothing. This module
//! is the one place that turns those packets into the [`Command`] and
//! [`Telemetry`] the rest of the software works with, and back:
//!
//! * [`Command::read`] and [`Telemetry::read`], which refuse a datagram that
//!   is not exactly one packet of the layout;
//! * [`CommandPacker`] and [`AnswerPacker`], which pack into a buffer they
//!   keep, so that a loop that sends allocates nothing.
//!
//! A packet carries no CRC of its own: the transport under it checks it,
//! as UDP's checksum does on this link, and a damaged datagram is refused
//! there before it reaches this.
//!
//! It replaced JSON, which no ground system reads from a mission database,
//! and which named every field in every packet. See "Every packet is XTCE" in `docs/design.rst`.

use std::fmt;

use tcs_xtce::packet::{Codec, FixedBytes, FixedString, Packet, PacketError};

use crate::commands::{
    Attribute, Command, CommandHeader, CommandType, ConfigCommand,
    ConnectCommand, NodeCommand, PingCommand, RestartArmCommand,
    RestartCommand, SetNodeCommand, Value,
};
use crate::config_digest::{ConfigDigest, ConfigVersion};
use crate::link_packet as packet;
use crate::link_packet::StatusType;
use crate::telemetry::{
    ConfigTelemetry, ConnectTelemetry, NodeTelemetry, PingTelemetry,
    QueryNodeSampleTelemetry, QueryNodeTelemetry, Reason, RestartArmTelemetry,
    RestartTelemetry, SetNodeTelemetry, Telemetry, TelemetryHeader,
    TelemetryType,
};
use crate::types::{
    ArmKey, BeaconInterval, CommandStatus, DHSample, NodeCounts, NodeId,
    Timestamp, DH_SAMPLE_BYTES,
};

/// The longest command there is, in bytes: a SET_NODE with the longest
/// attribute and value.
pub const COMMAND_BYTES: usize = packet::CommandKind::MAX_BYTES;

/// The longest answer there is, in bytes: a SET_NODE's with the longest
/// reason.
pub const ANSWER_BYTES: usize = packet::AnswerLayoutKind::MAX_BYTES;

/// Why a datagram is not a command or an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// It holds one packet and then more bytes, which no sender of these
    /// writes.
    Trailing { length: usize, packet: usize },
    /// It is not a packet of the layout.
    Packet(PacketError),
    /// It is a packet of the layout and says something the software cannot
    /// hold: a status no build has, an attribute that is not text.
    Value(&'static str),
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::Trailing { length, packet } => write!(
                f,
                "{length} bytes, of which the packet is the first {packet}: \
                 something other than tcspecial or its ground, or one of \
                 another layout, is sending here"
            ),
            LinkError::Packet(why) => write!(
                f,
                "not a packet of tcspecial's link ({why}): something other \
                 than tcspecial or its ground, or one of another layout, is \
                 sending here"
            ),
            LinkError::Value(why) => write!(f, "a packet that says {why}"),
        }
    }
}

impl std::error::Error for LinkError {}

/// Unpacks the one packet a datagram holds, as `P` unpacks it.
///
/// # Arguments
///
/// * `datagram` - The datagram received.
///
/// # Returns
///
/// The packet.
///
/// # Errors
///
/// [`LinkError::Packet`] if it does not start with a packet of the layout,
/// and [`LinkError::Trailing`] if there are bytes after the packet.
fn whole<P: Packet>(datagram: &[u8]) -> Result<P, LinkError> {
    let (read, used) = P::decode_prefix(datagram).map_err(LinkError::Packet)?;
    if used != datagram.len() {
        return Err(LinkError::Trailing {
            length: datagram.len(),
            packet: used,
        });
    }
    Ok(read)
}

/// The CCSDS sequence count of a packet whose sequence is `sequence`: its
/// low fourteen bits, which is all the header has room for.
fn seq_count(sequence: u32) -> u16 {
    (sequence & 0x3fff) as u16
}

/// Makes the header of a command read from the link.
///
/// # Arguments
///
/// * `sequence`   - The command's sequence number.
/// * `generation` - The generation it was sent for.
/// * `cmd_type`   - Which kind of command it is.
///
/// # Returns
///
/// The header.
fn header(
    sequence: u32,
    generation: u32,
    cmd_type: CommandType,
) -> CommandHeader {
    CommandHeader {
        sequence,
        cmd_type,
        generation,
    }
}

fn digest_of(bytes: &[u8]) -> ConfigDigest {
    let mut digest = [0u8; 16];
    digest.copy_from_slice(bytes);
    ConfigDigest(digest)
}

/// Sixteen bytes into a buffer of sixteen, which cannot fail.
fn fixed<const N: usize>(bytes: &[u8]) -> FixedBytes<N> {
    let mut out = FixedBytes::new();
    let _ = out.extend_from_slice(&bytes[..bytes.len().min(N)]);
    out
}

/// Text into a buffer at least as long as any the type allows, which cannot
/// fail either: the types it is called for are no longer than the buffers.
fn text<const N: usize>(said: &str) -> FixedString<N> {
    FixedString::try_from(said).unwrap_or_default()
}

impl Command {
    /// Reads the command a datagram holds.
    ///
    /// Allocates nothing: the packet is unpacked into fixed buffers, and the
    /// attribute and value of a SET_NODE are copied into the room the command
    /// holds for them.
    ///
    /// # Arguments
    ///
    /// * `datagram` - The datagram received.
    ///
    /// # Returns
    ///
    /// The command.
    ///
    /// # Errors
    ///
    /// What [`whole`] refuses, and [`LinkError::Value`] for a SET_NODE
    /// attribute or value too long for the command to hold.
    pub fn read(datagram: &[u8]) -> Result<Command, LinkError> {
        use packet::CommandKind as K;
        Ok(match whole::<K>(datagram)? {
            K::Ping(p) => Command::Ping(PingCommand {
                header: header(p.sequence, p.generation, CommandType::Ping),
            }),
            K::Connect(p) => Command::Connect(ConnectCommand {
                header: header(p.sequence, p.generation, CommandType::Connect),
                version: ConfigVersion {
                    major: p.major,
                    minor: p.minor,
                    patch: p.patch,
                },
                digest: digest_of(&p.digest),
            }),
            K::RestartArm(p) => Command::RestartArm(RestartArmCommand {
                header: header(
                    p.sequence,
                    p.generation,
                    CommandType::RestartArm,
                ),
                arm_key: ArmKey(p.arm_key),
            }),
            K::Restart(p) => Command::Restart(RestartCommand {
                header: header(p.sequence, p.generation, CommandType::Restart),
                arm_key: ArmKey(p.arm_key),
            }),
            K::Config(p) => Command::Config(ConfigCommand {
                header: header(p.sequence, p.generation, CommandType::Config),
                beacon_interval: BeaconInterval(p.beacon_interval),
            }),
            K::StartNode(p) => Command::StartNode(NodeCommand {
                header: header(
                    p.sequence,
                    p.generation,
                    CommandType::StartNode,
                ),
                node: NodeId(p.node),
            }),
            K::StopNode(p) => Command::StopNode(NodeCommand {
                header: header(p.sequence, p.generation, CommandType::StopNode),
                node: NodeId(p.node),
            }),
            K::QueryNode(p) => Command::QueryNode(NodeCommand {
                header: header(
                    p.sequence,
                    p.generation,
                    CommandType::QueryNode,
                ),
                node: NodeId(p.node),
            }),
            K::QueryNodeSample(p) => Command::QueryNodeSample(NodeCommand {
                header: header(
                    p.sequence,
                    p.generation,
                    CommandType::QueryNodeSample,
                ),
                node: NodeId(p.node),
            }),
            K::SetNode(p) => Command::SetNode(SetNodeCommand {
                header: header(p.sequence, p.generation, CommandType::SetNode),
                node: NodeId(p.node),
                // Both fit: the XTCE's capacities are the command's.
                attribute: Attribute::of(&p.attribute)
                    .map_err(|_| LinkError::Value("an attribute too long"))?,
                value: Value::of(&p.value_)
                    .map_err(|_| LinkError::Value("a value too long"))?,
            }),
        })
    }

    /// This command as it goes on the link, in a vector of its own.
    ///
    /// For a command sent once, as a test sends one. Something that sends
    /// commands in a loop packs them with a [`CommandPacker`].
    pub fn to_bytes(&self) -> Vec<u8> {
        CommandPacker::new().pack(self).to_vec()
    }

    /// Makes the generated packet that says this command.
    ///
    /// # Returns
    ///
    /// The packet, its CCSDS sequence count taken from the sequence number.
    fn packet(&self) -> packet::CommandKind {
        use packet::CommandKind as K;
        let sequence = self.sequence();
        let generation = self.generation();
        let seq_count = seq_count(sequence);
        match self {
            Command::Ping(_) => K::Ping(packet::Ping {
                seq_count,
                sequence,
                generation,
            }),
            Command::Connect(c) => K::Connect(packet::Connect {
                seq_count,
                sequence,
                generation,
                major: c.version.major,
                minor: c.version.minor,
                patch: c.version.patch,
                digest: fixed(&c.digest.0),
            }),
            Command::RestartArm(c) => K::RestartArm(packet::RestartArm {
                seq_count,
                sequence,
                generation,
                arm_key: c.arm_key.0,
            }),
            Command::Restart(c) => K::Restart(packet::Restart {
                seq_count,
                sequence,
                generation,
                arm_key: c.arm_key.0,
            }),
            Command::Config(c) => K::Config(packet::Config {
                seq_count,
                sequence,
                generation,
                beacon_interval: c.beacon_interval.0,
            }),
            Command::StartNode(c) => K::StartNode(packet::StartNode {
                seq_count,
                sequence,
                generation,
                node: c.node.0,
            }),
            Command::StopNode(c) => K::StopNode(packet::StopNode {
                seq_count,
                sequence,
                generation,
                node: c.node.0,
            }),
            Command::QueryNode(c) => K::QueryNode(packet::QueryNode {
                seq_count,
                sequence,
                generation,
                node: c.node.0,
            }),
            Command::QueryNodeSample(c) => {
                K::QueryNodeSample(packet::QueryNodeSample {
                    seq_count,
                    sequence,
                    generation,
                    node: c.node.0,
                })
            }
            Command::SetNode(c) => K::SetNode(packet::SetNode {
                seq_count,
                sequence,
                generation,
                node: c.node.0,
                attribute: text(c.attribute.as_str()),
                value_: fixed(c.value.as_bytes()),
            }),
        }
    }
}

/// Packs commands into a buffer it keeps, for a loop that sends them.
pub struct CommandPacker {
    codec: Codec,
}

impl CommandPacker {
    pub fn new() -> Self {
        Self {
            codec: Codec::for_packet::<packet::CommandKind>(),
        }
    }

    /// `command`, packed, in the packer's own buffer.
    pub fn pack(&mut self, command: &Command) -> &[u8] {
        // Every field is in range by the command's own types -- an attribute
        // of at most 64 bytes, a value of at most 256 -- which are the
        // layout's, so there is nothing a command can hold that does not
        // pack.
        self.codec
            .encode(&command.packet())
            .expect("every command fits the layout")
    }
}

impl Default for CommandPacker {
    fn default() -> Self {
        Self::new()
    }
}

/// Gives a status as the link carries it.
///
/// # Arguments
///
/// * `status` - The status.
///
/// # Returns
///
/// Its value on the link.
fn status_of(status: CommandStatus) -> StatusType {
    match status {
        CommandStatus::Success => StatusType::SUCCESS,
        CommandStatus::Failure => StatusType::FAILURE,
        CommandStatus::InvalidCommand => StatusType::INVALID_COMMAND,
        CommandStatus::InvalidParameter => StatusType::INVALID_PARAMETER,
        CommandStatus::NotArmed => StatusType::NOT_ARMED,
        CommandStatus::NotFound => StatusType::NOT_FOUND,
        CommandStatus::AlreadyExists => StatusType::ALREADY_EXISTS,
        CommandStatus::Timeout => StatusType::TIMEOUT,
        CommandStatus::WrongGeneration => StatusType::WRONG_GENERATION,
        CommandStatus::Stale => StatusType::STALE,
    }
}

/// Reads a status from the link: the inverse of [`status_of`].
///
/// # Arguments
///
/// * `status` - The status as the link carries it.
///
/// # Returns
///
/// The status.
///
/// # Errors
///
/// [`LinkError::Value`] for a status no build has.
fn status_from(status: StatusType) -> Result<CommandStatus, LinkError> {
    Ok(match status {
        StatusType::SUCCESS => CommandStatus::Success,
        StatusType::FAILURE => CommandStatus::Failure,
        StatusType::INVALID_COMMAND => CommandStatus::InvalidCommand,
        StatusType::INVALID_PARAMETER => CommandStatus::InvalidParameter,
        StatusType::NOT_ARMED => CommandStatus::NotArmed,
        StatusType::NOT_FOUND => CommandStatus::NotFound,
        StatusType::ALREADY_EXISTS => CommandStatus::AlreadyExists,
        StatusType::TIMEOUT => CommandStatus::Timeout,
        StatusType::WRONG_GENERATION => CommandStatus::WrongGeneration,
        StatusType::STALE => CommandStatus::Stale,
        _ => return Err(LinkError::Value("a status no build has")),
    })
}

/// Gives a time that may not have happened as the link carries it.
///
/// # Arguments
///
/// * `time` - The time, or `None` where it has not happened.
///
/// # Returns
///
/// Whether it has happened, and when in microseconds, nought where it has
/// not.
fn when_of(time: Option<Timestamp>) -> (bool, u64) {
    match time {
        Some(time) => (true, time.micros),
        None => (false, 0),
    }
}

fn when_from(known: bool, micros: u64) -> Option<Timestamp> {
    known.then_some(Timestamp { micros })
}

/// Reads what a sample says, from the fields the link carries it in.
///
/// # Arguments
///
/// * `known` - Whether the sample's time is known.
/// * `time`  - Its time in microseconds, meaningless unless `known`.
/// * `len`   - How many of the bytes shown are the sample's.
/// * `total` - How many bytes the datum held, of which these are the first.
/// * `shown` - The bytes shown, at least [`DH_SAMPLE_BYTES`] of them.
///
/// # Returns
///
/// The sample.
///
/// # Errors
///
/// [`LinkError::Value`] when `len` is more than [`DH_SAMPLE_BYTES`].
///
/// # Panics
///
/// If `shown` is shorter than [`DH_SAMPLE_BYTES`], which the layout does
/// not allow.
fn sample_from(
    known: bool,
    time: u64,
    len: u8,
    total: u32,
    shown: &[u8],
) -> Result<DHSample, LinkError> {
    if usize::from(len) > DH_SAMPLE_BYTES {
        return Err(LinkError::Value("a sample longer than one is shown"));
    }
    let mut bytes = [0u8; DH_SAMPLE_BYTES];
    bytes.copy_from_slice(&shown[..DH_SAMPLE_BYTES]);
    Ok(DHSample {
        time: when_from(known, time),
        len,
        total,
        bytes,
    })
}

impl Telemetry {
    /// Reads the answer a datagram holds.
    ///
    /// # Arguments
    ///
    /// * `datagram` - The datagram received.
    ///
    /// # Returns
    ///
    /// The answer.
    ///
    /// # Errors
    ///
    /// What [`whole`] refuses, a status no build has, and a sample longer
    /// than one is shown.
    pub fn read(datagram: &[u8]) -> Result<Telemetry, LinkError> {
        use packet::AnswerLayoutKind as K;
        // The header every answer starts with, from the fields every packet
        // has under the same names.
        macro_rules! head {
            ($p:expr, $kind:expr) => {
                TelemetryHeader {
                    sequence: $p.sequence,
                    generation: $p.generation,
                    tm_type: $kind,
                    status: status_from($p.status)?,
                    timestamp: Timestamp {
                        micros: $p.timestamp,
                    },
                }
            };
        }
        Ok(match whole::<K>(datagram)? {
            K::PingAnswer(p) => Telemetry::Ping(PingTelemetry {
                header: head!(p, TelemetryType::Ping),
            }),
            K::RestartArmAnswer(p) => {
                Telemetry::RestartArm(RestartArmTelemetry {
                    header: head!(p, TelemetryType::RestartArm),
                })
            }
            K::RestartAnswer(p) => Telemetry::Restart(RestartTelemetry {
                header: head!(p, TelemetryType::Restart),
            }),
            K::ConnectAnswer(p) => Telemetry::Connect(ConnectTelemetry {
                header: head!(p, TelemetryType::Connect),
                version: ConfigVersion {
                    major: p.major,
                    minor: p.minor,
                    patch: p.patch,
                },
                digest: digest_of(&p.digest),
                newest_taken: p.newest_taken,
            }),
            K::ConfigAnswer(p) => Telemetry::Config(ConfigTelemetry {
                header: head!(p, TelemetryType::Config),
            }),
            K::StartNodeAnswer(p) => Telemetry::StartNode(NodeTelemetry {
                header: head!(p, TelemetryType::StartNode),
                node: NodeId(p.node),
            }),
            K::StopNodeAnswer(p) => Telemetry::StopNode(NodeTelemetry {
                header: head!(p, TelemetryType::StopNode),
                node: NodeId(p.node),
            }),
            K::SetNodeAnswer(p) => Telemetry::SetNode(SetNodeTelemetry {
                header: head!(p, TelemetryType::SetNode),
                node: NodeId(p.node),
                reason: Reason::of(&p.reason),
            }),
            K::QueryNodeAnswer(p) => Telemetry::QueryNode(QueryNodeTelemetry {
                header: head!(p, TelemetryType::QueryNode),
                node: NodeId(p.node),
                enabled: p.enabled,
                counts: NodeCounts {
                    taken_bytes: p.taken_bytes,
                    reads: p.reads,
                    reads_failed: p.reads_failed,
                    given_bytes: p.given_bytes,
                    writes: p.writes,
                    writes_failed: p.writes_failed,
                    dropped: p.dropped,
                    asks: p.asks,
                    asks_failed: p.asks_failed,
                    strangers: p.strangers,
                    last_moved: when_from(p.moved_known, p.last_moved),
                },
            }),
            K::QueryNodeSampleAnswer(p) => {
                Telemetry::QueryNodeSample(QueryNodeSampleTelemetry {
                    header: head!(p, TelemetryType::QueryNodeSample),
                    node: NodeId(p.node),
                    taken: sample_from(
                        p.taken_known,
                        p.taken_time,
                        p.taken_length,
                        p.taken_total,
                        &p.taken_bytes_shown,
                    )?,
                    given: sample_from(
                        p.given_known,
                        p.given_time,
                        p.given_length,
                        p.given_total,
                        &p.given_bytes_shown,
                    )?,
                })
            }
        })
    }

    /// This answer as it goes on the link, in a vector of its own: for an
    /// answer sent once, as a test's stand-in sends one.
    pub fn to_bytes(&self) -> Vec<u8> {
        AnswerPacker::new().pack(self).to_vec()
    }

    /// Makes the generated packet that says this answer.
    ///
    /// # Returns
    ///
    /// The packet, its CCSDS sequence count taken from the sequence number.
    fn packet(&self) -> packet::AnswerLayoutKind {
        use packet::AnswerLayoutKind as K;
        macro_rules! fill {
            ($ty:ident, $h:expr; $($field:ident: $value:expr),* $(,)?) => {
                packet::$ty {
                    seq_count: seq_count($h.sequence),
                    sequence: $h.sequence,
                    generation: $h.generation,
                    status: status_of($h.status),
                    timestamp: $h.timestamp.micros,
                    $($field: $value,)*
                }
            };
        }
        match self {
            Telemetry::Ping(t) => K::PingAnswer(fill!(PingAnswer, t.header;)),
            Telemetry::RestartArm(t) => {
                K::RestartArmAnswer(fill!(RestartArmAnswer, t.header;))
            }
            Telemetry::Restart(t) => {
                K::RestartAnswer(fill!(RestartAnswer, t.header;))
            }
            Telemetry::Connect(t) => K::ConnectAnswer(fill!(
                ConnectAnswer,
                t.header;
                    major: t.version.major,
                    minor: t.version.minor,
                    patch: t.version.patch,
                    digest: fixed(&t.digest.0),
                    newest_taken: t.newest_taken,
            )),
            Telemetry::Config(t) => {
                K::ConfigAnswer(fill!(ConfigAnswer, t.header;))
            }
            Telemetry::StartNode(t) => K::StartNodeAnswer(fill!(
                StartNodeAnswer,
                t.header; node: t.node.0
            )),
            Telemetry::StopNode(t) => K::StopNodeAnswer(fill!(
                StopNodeAnswer,
                t.header; node: t.node.0
            )),
            Telemetry::SetNode(t) => K::SetNodeAnswer(fill!(
                SetNodeAnswer,
                t.header;
                    node: t.node.0,
                    reason: text(t.reason.as_str()),
            )),
            Telemetry::QueryNode(t) => {
                let (moved_known, last_moved) = when_of(t.counts.last_moved);
                let c = &t.counts;
                K::QueryNodeAnswer(fill!(
                    QueryNodeAnswer,
                    t.header;
                        node: t.node.0,
                        enabled: t.enabled,
                        taken_bytes: c.taken_bytes,
                        reads: c.reads,
                        reads_failed: c.reads_failed,
                        given_bytes: c.given_bytes,
                        writes: c.writes,
                        writes_failed: c.writes_failed,
                        dropped: c.dropped,
                        asks: c.asks,
                        asks_failed: c.asks_failed,
                        strangers: c.strangers,
                        moved_known: moved_known,
                        last_moved: last_moved,
                ))
            }
            Telemetry::QueryNodeSample(t) => {
                let (taken_known, taken_time) = when_of(t.taken.time);
                let (given_known, given_time) = when_of(t.given.time);
                K::QueryNodeSampleAnswer(fill!(
                    QueryNodeSampleAnswer,
                    t.header;
                        node: t.node.0,
                        taken_known: taken_known,
                        taken_time: taken_time,
                        taken_length: t.taken.len,
                        taken_total: t.taken.total,
                        taken_bytes_shown: fixed(&t.taken.bytes),
                        given_known: given_known,
                        given_time: given_time,
                        given_length: t.given.len,
                        given_total: t.given.total,
                        given_bytes_shown: fixed(&t.given.bytes),
                ))
            }
        }
    }
}

/// Packs answers into a buffer it keeps, for the command interpreter, which
/// answers every command it takes from its main loop.
pub struct AnswerPacker {
    codec: Codec,
}

impl AnswerPacker {
    pub fn new() -> Self {
        Self {
            codec: Codec::for_packet::<packet::AnswerLayoutKind>(),
        }
    }

    /// `answer`, packed, in the packer's own buffer.
    pub fn pack(&mut self, answer: &Telemetry) -> &[u8] {
        // Every field is in range by the answer's own types -- a reason of
        // at most 160 bytes -- which are the layout's.
        self.codec
            .encode(&answer.packet())
            .expect("every answer fits the layout")
    }
}

impl Default for AnswerPacker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Makes one command of every kind, and a SET_NODE as long as one can be.
    ///
    /// # Returns
    ///
    /// The commands.
    fn every_command() -> Vec<Command> {
        vec![
            Command::Ping(PingCommand::new(1)),
            Command::Connect(ConnectCommand::new(
                2,
                ConfigVersion::of_this_build(),
                ConfigDigest([0x5a; 16]),
            )),
            Command::RestartArm(RestartArmCommand::new(3, ArmKey(0x1234))),
            Command::Restart(RestartCommand::new(4, ArmKey(0x1234))),
            Command::Config(ConfigCommand::new(5, BeaconInterval(2_000_000))),
            Command::StartNode(NodeCommand::start(6, NodeId(7))),
            Command::StopNode(NodeCommand::stop(7, NodeId(7))),
            Command::QueryNode(NodeCommand::query(8, NodeId(7))),
            Command::QueryNodeSample(NodeCommand::query_sample(9, NodeId(7))),
            Command::SetNode(
                SetNodeCommand::new(10, NodeId(7), "setpoint", b"295.0")
                    .expect("it fits"),
            ),
            Command::SetNode(
                SetNodeCommand::new(
                    0x0004_0001,
                    NodeId(u32::MAX),
                    &"a".repeat(crate::ATTRIBUTE_BYTES),
                    &[0xff; crate::VALUE_BYTES],
                )
                .expect("the longest fits"),
            ),
        ]
    }

    /// Makes answers of every kind, with every field given a value.
    ///
    /// # Returns
    ///
    /// The answers.
    fn every_answer() -> Vec<Telemetry> {
        let mut taken = DHSample::new();
        taken.record(b"hello, world");
        taken.time = Some(Timestamp { micros: 1_234_567 });
        let counts = NodeCounts {
            taken_bytes: 1,
            reads: 2,
            reads_failed: 3,
            given_bytes: 4,
            writes: 5,
            writes_failed: 6,
            dropped: 7,
            asks: 8,
            asks_failed: 9,
            strangers: 10,
            last_moved: Some(Timestamp::CLOCK_FAULT),
        };
        vec![
            Telemetry::Ping(PingTelemetry::new(1, CommandStatus::Success)),
            Telemetry::Connect(ConnectTelemetry::new(
                2,
                ConfigVersion::of_this_build(),
                ConfigDigest([0xa5; 16]),
                0x0102_0304,
            )),
            Telemetry::RestartArm(RestartArmTelemetry::new(
                3,
                CommandStatus::Success,
            )),
            Telemetry::Restart(RestartTelemetry::new(
                4,
                CommandStatus::NotArmed,
            )),
            Telemetry::Config(ConfigTelemetry::new(5, CommandStatus::Timeout)),
            Telemetry::Config(ConfigTelemetry::new(12, CommandStatus::Stale)),
            Telemetry::StartNode(NodeTelemetry::started(
                6,
                CommandStatus::Success,
                NodeId(7),
            )),
            Telemetry::StopNode(NodeTelemetry::stopped(
                7,
                CommandStatus::NotFound,
                NodeId(70),
            )),
            Telemetry::SetNode(
                SetNodeTelemetry::new(
                    8,
                    CommandStatus::InvalidParameter,
                    NodeId(7),
                )
                .because(Reason::of(&"no binding".repeat(16))),
            ),
            Telemetry::QueryNode(QueryNodeTelemetry::new(
                9,
                CommandStatus::Success,
                NodeId(7),
                true,
                counts,
            )),
            Telemetry::QueryNode(QueryNodeTelemetry::refused(
                10,
                CommandStatus::NotFound,
                NodeId(7),
            )),
            Telemetry::QueryNodeSample(QueryNodeSampleTelemetry::new(
                11,
                CommandStatus::Success,
                NodeId(7),
                taken,
                DHSample::new(),
            )),
        ]
    }

    #[test]
    fn every_command_reads_back_as_itself() {
        // Each of a generation of its own, so that a generation read from
        // another command's place in the packet would be seen.
        for (generation, command) in (7..).zip(every_command()) {
            let command = command.for_generation(generation);
            let packed = command.to_bytes();
            assert!(packed.len() <= COMMAND_BYTES);
            assert_eq!(Command::read(&packed), Ok(command.clone()));
        }
    }

    #[test]
    fn every_answer_reads_back_as_itself() {
        // Each of a generation of its own, as the commands are.
        for (generation, answer) in (7..).zip(every_answer()) {
            let answer = answer.for_generation(generation);
            let packed = answer.to_bytes();
            assert!(packed.len() <= ANSWER_BYTES);
            assert_eq!(Telemetry::read(&packed), Ok(answer.clone()));
        }
    }

    /// A command is a CCSDS telecommand of APID 0x100, its length field the
    /// CCSDS one, then the opcode, the sequence and the generation, most
    /// significant byte first, then what it carries.
    #[test]
    fn a_command_is_a_ccsds_packet() {
        let packed = Command::RestartArm(RestartArmCommand::new(
            0x0102_4005,
            ArmKey(0x1122_3344_5566_7788),
        ))
        .for_generation(0x0a0b_0c0d)
        .to_bytes();
        // Version 0, type 1, no secondary header, APID 0x100; unsegmented,
        // count 0x4005 & 0x3fff; 23 bytes in all, 17 after the header,
        // less one. Then RESTART_ARM, the sequence, the generation and the
        // key.
        let want = vec![
            0x11, 0x00, 0xc0, 0x05, 0x00, 0x10, //
            0x02, //
            0x01, 0x02, 0x40, 0x05, //
            0x0a, 0x0b, 0x0c, 0x0d, //
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
        ];
        assert_eq!(packed, want);
    }

    /// An answer is a CCSDS telemetry packet of the same APID, its kind,
    /// sequence, generation, status and time after the header.
    #[test]
    fn an_answer_is_a_ccsds_packet() {
        let mut ping = PingTelemetry::new(7, CommandStatus::NotArmed);
        ping.header.timestamp = Timestamp { micros: 0x0102 };
        let packed =
            Telemetry::Ping(ping).for_generation(0x0a0b_0c0d).to_bytes();
        // Version 0, type 0, no secondary header, APID 0x100; unsegmented,
        // count 7; 24 bytes in all. Then the answer to a PING, the
        // sequence, the generation that made it, NotArmed and the time.
        let want = vec![
            0x01, 0x00, 0xc0, 0x07, 0x00, 0x11, //
            0x81, //
            0x00, 0x00, 0x00, 0x07, //
            0x0a, 0x0b, 0x0c, 0x0d, //
            0x04, //
            0, 0, 0, 0, 0, 0, 0x01, 0x02,
        ];
        assert_eq!(packed, want);
    }

    /// A packet whose header is not this link's, whose length field does
    /// not match its datagram, or that has more after it, is refused. One
    /// damaged on the way is the transport's to refuse, by its checksum.
    #[test]
    fn a_packet_of_another_header_or_length_is_refused() {
        let good =
            Command::StartNode(NodeCommand::start(6, NodeId(7))).to_bytes();
        assert!(Command::read(&good).is_ok());
        // The version, the packet type, the secondary header flag, the
        // APID, the sequence flags and the length field.
        for (at, bits) in [
            (0, 0x20),
            (0, 0x10),
            (0, 0x08),
            (1, 0x01),
            (0, 0x01),
            (2, 0x40),
            (5, 0x01),
        ] {
            let mut other = good.clone();
            other[at] ^= bits;
            assert!(Command::read(&other).is_err(), "byte {at} ^ {bits:#04x}");
        }
        let mut padded = good.clone();
        padded.push(0);
        assert!(
            Command::read(&padded).is_err(),
            "a byte after the packet is not part of a command"
        );
        assert!(Command::read(&good[..good.len() - 1]).is_err());
        assert!(Command::read(b"").is_err());
    }

    /// An answer sent to where commands are taken is not a command, though
    /// a node answer's kind is a node command's opcode: the packet type
    /// says which it is.
    #[test]
    fn an_answer_is_not_a_command() {
        let answer = Telemetry::StartNode(NodeTelemetry::started(
            6,
            CommandStatus::Success,
            NodeId(7),
        ))
        .to_bytes();
        assert!(Command::read(&answer).is_err());
        let command =
            Command::StartNode(NodeCommand::start(6, NodeId(7))).to_bytes();
        assert!(Telemetry::read(&command).is_err());
    }

    /// A reason cut to fit reads back as the text it was cut to, and with
    /// the mark it ends in.
    #[test]
    fn a_cut_reason_reads_back_as_what_was_sent() {
        let sent = SetNodeTelemetry::new(
            8,
            CommandStatus::InvalidParameter,
            NodeId(7),
        )
        .because(Reason::of(&"no binding, ".repeat(20)));
        let packed = Telemetry::SetNode(sent).to_bytes();
        let Ok(Telemetry::SetNode(read)) = Telemetry::read(&packed) else {
            panic!("it reads back");
        };
        assert_eq!(read.reason.as_str(), sent.reason.as_str());
        assert!(read.reason.as_str().ends_with("..."));
    }

    /// The opcode and the kind on the link are the ones `CommandType` and
    /// `TelemetryType` name: the tables those keep are held to the XTCE
    /// here, which is the one statement of what goes on the link.
    #[test]
    fn the_opcodes_are_the_tables_numbers() {
        for command in every_command() {
            assert_eq!(
                command.to_bytes()[6],
                command.cmd_type().to_u8(),
                "{:?}",
                command.cmd_type()
            );
        }
        for answer in every_answer() {
            assert_eq!(
                answer.to_bytes()[6],
                answer.tm_type().to_u8(),
                "{:?}",
                answer.tm_type()
            );
        }
    }

    /// One packer packs command after command into the one buffer.
    #[test]
    fn one_packer_packs_command_after_command_in_the_one_buffer() {
        let mut packer = CommandPacker::new();
        let first = packer.pack(&Command::Ping(PingCommand::new(1))).as_ptr();
        let set = Command::SetNode(
            SetNodeCommand::new(2, NodeId(0), "setpoint", b"280.0")
                .expect("it fits"),
        );
        let second = packer.pack(&set);
        assert_eq!(second.as_ptr(), first);
        assert_eq!(Command::read(second), Ok(set));
    }
}
