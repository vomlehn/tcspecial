//! Commands and answers, as they go on the link.
//!
//! What the MOC sends tcspecial and what tcspecial sends back, with no
//! header: a command is its opcode, its sequence and what it carries, and an
//! answer the opcode and sequence of the command it answers, a status, a
//! time and what it carries. The layouts are stated once, in XTCE: the three
//! fixed commands -- CONNECT, RESTART_ARM and RESTART -- in
//! `tcslibgs/xtce/fixed.xml`, whose layouts never change, and every other
//! command in `tcslibgs/xtce/link.xml`. Each is packed and unpacked by the
//! code tcs-xtce generates from it, `fixed_packet.rs` and `link_packet.rs`,
//! which allocates nothing. This module is the one place that turns those
//! packets into the [`Command`] and [`Telemetry`] the rest of the software
//! works with, and back:
//!
//! * [`Command::read`] and [`Telemetry::read`], which refuse a datagram whose
//!   opcode no command has or whose length is not exactly its opcode's
//!   layout's;
//! * [`CommandPacker`] and [`AnswerPacker`], which pack into a buffer they
//!   keep, so that a loop that sends allocates nothing.
//!
//! Which way a packet goes is said by who sends it and who receives it: the
//! vehicle reads only commands and a ground station only answers, so nothing
//! in a packet says which it is. A packet carries no CRC of its own: the
//! transport under it checks it, as UDP's checksum does on this link, and a
//! damaged datagram is refused there before it reaches this.
//!
//! It replaced CCSDS Space Packets, whose header said what the link already
//! did, and before them JSON. See "The link without a header" in
//! `docs/design.rst`.

use std::fmt;

use tcs_xtce::packet::{Codec, FixedBytes, FixedString, Packet, PacketError};

use crate::commands::{
    Attribute, Command, CommandHeader, CommandType, ConnectCommand,
    NodeCommand, PingCommand, RestartArmCommand, RestartCommand,
    SetNodeCommand, Value,
};
use crate::config_digest::{ConfigDigest, ConfigVersion};
use crate::fixed_packet as fixed;
use crate::link_packet as packet;
use crate::telemetry::{
    ConnectTelemetry, NodeTelemetry, PingTelemetry, QueryNodeSampleTelemetry,
    QueryNodeTelemetry, Reason, RestartArmTelemetry, RestartTelemetry,
    SetNodeTelemetry, Telemetry, TelemetryHeader, TelemetryType,
};
use crate::types::{
    ArmKey, CommandStatus, DHSample, NodeCounts, NodeId, Timestamp,
    DH_SAMPLE_BYTES,
};

/// The larger of two lengths, for the constants below.
///
/// # Arguments
///
/// * `a` - One length.
/// * `b` - The other.
///
/// # Returns
///
/// Whichever is larger.
const fn larger(a: usize, b: usize) -> usize {
    if a > b {
        a
    } else {
        b
    }
}

/// The longest command there is, in bytes: a SET_NODE with the longest
/// attribute and value.
pub const COMMAND_BYTES: usize = larger(
    packet::TcsCommandKind::MAX_BYTES,
    fixed::FixedCommandKind::MAX_BYTES,
);

/// The longest answer there is, in bytes: a SET_NODE's with the longest
/// reason.
pub const ANSWER_BYTES: usize = larger(
    packet::AnswerLayoutKind::MAX_BYTES,
    fixed::FixedAnswerKind::MAX_BYTES,
);

/// Why a datagram is not a command or an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// It holds nothing, so not even an opcode.
    Empty,
    /// Its first byte is an opcode no command has.
    Opcode(u8),
    /// It holds one packet of its opcode's layout and then more bytes, which
    /// no sender of these writes.
    Trailing { length: usize, packet: usize },
    /// It is not a packet of its opcode's layout: too short, or a field out
    /// of the layout's range.
    Packet(PacketError),
    /// It is a packet of the layout and says something the software cannot
    /// hold: an attribute that is not text, a sample longer than one is
    /// shown.
    Value(&'static str),
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::Empty => write!(
                f,
                "an empty datagram, which holds no opcode: something other \
                 than tcspecial or its ground is sending here"
            ),
            LinkError::Opcode(opcode) => write!(
                f,
                "opcode {opcode:#04x}, which no command of this interface \
                 has: something other than tcspecial or its ground, or one \
                 built from another interface, is sending here"
            ),
            LinkError::Trailing { length, packet } => write!(
                f,
                "{length} bytes, where its opcode's layout is {packet}: \
                 something other than tcspecial or its ground, or one built \
                 from another interface, is sending here"
            ),
            LinkError::Packet(why) => write!(
                f,
                "not a packet of its opcode's layout ({why}): something \
                 other than tcspecial or its ground, or one built from \
                 another interface, is sending here"
            ),
            LinkError::Value(why) => write!(f, "a packet that says {why}"),
        }
    }
}

impl std::error::Error for LinkError {}

/// Reads which command a datagram is, or answers, by its opcode. The three
/// [`CommandType::taken_unmatched`] names are the fixed commands, which
/// `xtce/fixed.xml` states; every other opcode is read by `xtce/link.xml`'s
/// layouts.
///
/// # Arguments
///
/// * `datagram` - The datagram received.
///
/// # Returns
///
/// The kind of command the opcode names, whose
/// [`CommandType::taken_unmatched`] says which document's layout it has.
///
/// # Errors
///
/// [`LinkError::Empty`] for an empty datagram, and [`LinkError::Opcode`]
/// for an opcode no command has.
fn opcode_of(datagram: &[u8]) -> Result<CommandType, LinkError> {
    let opcode = *datagram.first().ok_or(LinkError::Empty)?;
    CommandType::from_u8(opcode).ok_or(LinkError::Opcode(opcode))
}

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

/// Makes the header of a command read from the link.
///
/// # Arguments
///
/// * `sequence` - The command's sequence number.
/// * `cmd_type` - Which kind of command it is.
///
/// # Returns
///
/// The header.
fn header(sequence: u32, cmd_type: CommandType) -> CommandHeader {
    CommandHeader { sequence, cmd_type }
}

fn digest_of(bytes: &[u8]) -> ConfigDigest {
    let mut digest = [0u8; 16];
    digest.copy_from_slice(bytes);
    ConfigDigest(digest)
}

/// Sixteen bytes into a buffer of sixteen, which cannot fail.
fn fixed_bytes<const N: usize>(bytes: &[u8]) -> FixedBytes<N> {
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
    /// What [`opcode_of`] and [`whole`] refuse -- an opcode no command has,
    /// or a length that is not exactly its layout's -- and
    /// [`LinkError::Value`] for a SET_NODE attribute or value too long for
    /// the command to hold.
    pub fn read(datagram: &[u8]) -> Result<Command, LinkError> {
        if opcode_of(datagram)?.taken_unmatched() {
            use fixed::FixedCommandKind as F;
            return Ok(match whole::<F>(datagram)? {
                F::RestartArm(p) => Command::RestartArm(RestartArmCommand {
                    header: header(p.sequence, CommandType::RestartArm),
                    arm_key: ArmKey(p.arm_key),
                    target: digest_of(&p.target),
                }),
                F::Restart(p) => Command::Restart(RestartCommand {
                    header: header(p.sequence, CommandType::Restart),
                    arm_key: ArmKey(p.arm_key),
                    target: digest_of(&p.target),
                }),
                F::Connect(p) => Command::Connect(ConnectCommand {
                    header: header(p.sequence, CommandType::Connect),
                    digest: digest_of(&p.checksum),
                }),
            });
        }
        use packet::TcsCommandKind as K;
        Ok(match whole::<K>(datagram)? {
            K::Ping(p) => Command::Ping(PingCommand {
                header: header(p.sequence, CommandType::Ping),
            }),
            K::StartNode(p) => Command::StartNode(NodeCommand {
                header: header(p.sequence, CommandType::StartNode),
                node: NodeId(p.node),
            }),
            K::StopNode(p) => Command::StopNode(NodeCommand {
                header: header(p.sequence, CommandType::StopNode),
                node: NodeId(p.node),
            }),
            K::QueryNode(p) => Command::QueryNode(NodeCommand {
                header: header(p.sequence, CommandType::QueryNode),
                node: NodeId(p.node),
            }),
            K::QueryNodeSample(p) => Command::QueryNodeSample(NodeCommand {
                header: header(p.sequence, CommandType::QueryNodeSample),
                node: NodeId(p.node),
            }),
            K::SetNode(p) => Command::SetNode(SetNodeCommand {
                header: header(p.sequence, CommandType::SetNode),
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
}

/// Packs commands into a buffer it keeps, for a loop that sends them.
///
/// Two codecs, one for each document, each with a buffer of its own, made
/// when the packer is: packing allocates nothing after that.
pub struct CommandPacker {
    fixed: Codec,
    ordinary: Codec,
}

impl CommandPacker {
    /// Makes a packer, with a buffer for the longest command of each
    /// document.
    ///
    /// # Returns
    ///
    /// The packer.
    pub fn new() -> Self {
        Self {
            fixed: Codec::for_packet::<fixed::FixedCommandKind>(),
            ordinary: Codec::for_packet::<packet::TcsCommandKind>(),
        }
    }

    /// Packs a command, in the packer's own buffer.
    ///
    /// # Arguments
    ///
    /// * `command` - The command.
    ///
    /// # Returns
    ///
    /// Its bytes on the link, good until the next pack.
    pub fn pack(&mut self, command: &Command) -> &[u8] {
        use fixed::FixedCommandKind as F;
        use packet::TcsCommandKind as K;
        let sequence = command.sequence();
        let ordinary = match command {
            Command::RestartArm(c) => {
                return encoded(
                    &mut self.fixed,
                    &F::RestartArm(fixed::RestartArm {
                        sequence,
                        arm_key: c.arm_key.0,
                        target: fixed_bytes(&c.target.0),
                    }),
                )
            }
            Command::Restart(c) => {
                return encoded(
                    &mut self.fixed,
                    &F::Restart(fixed::Restart {
                        sequence,
                        arm_key: c.arm_key.0,
                        target: fixed_bytes(&c.target.0),
                    }),
                )
            }
            Command::Connect(c) => {
                return encoded(
                    &mut self.fixed,
                    &F::Connect(fixed::Connect {
                        sequence,
                        checksum: fixed_bytes(&c.digest.0),
                    }),
                )
            }
            Command::Ping(_) => K::Ping(packet::Ping { sequence }),
            Command::StartNode(c) => K::StartNode(packet::StartNode {
                sequence,
                node: c.node.0,
            }),
            Command::StopNode(c) => K::StopNode(packet::StopNode {
                sequence,
                node: c.node.0,
            }),
            Command::QueryNode(c) => K::QueryNode(packet::QueryNode {
                sequence,
                node: c.node.0,
            }),
            Command::QueryNodeSample(c) => {
                K::QueryNodeSample(packet::QueryNodeSample {
                    sequence,
                    node: c.node.0,
                })
            }
            Command::SetNode(c) => K::SetNode(packet::SetNode {
                sequence,
                node: c.node.0,
                attribute: text(c.attribute.as_str()),
                value_: fixed_bytes(c.value.as_bytes()),
            }),
        };
        encoded(&mut self.ordinary, &ordinary)
    }
}

/// Packs `packet` with `codec`, into the codec's own buffer.
///
/// Every field of a command or an answer is in range by its own types -- an
/// attribute of at most 64 bytes, a value of at most 256, a reason of at
/// most 160 -- which are the layouts', so there is nothing one can hold
/// that does not pack.
///
/// # Arguments
///
/// * `codec`  - The codec of the document `packet` is stated in.
/// * `packet` - The generated packet.
///
/// # Returns
///
/// The packed bytes.
fn encoded<'a, P: Packet>(codec: &'a mut Codec, packet: &P) -> &'a [u8] {
    codec
        .encode(packet)
        .expect("every command and answer fits its layout")
}

impl Default for CommandPacker {
    fn default() -> Self {
        Self::new()
    }
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

/// Reads a status from its number on the link. A number this build does not
/// know is read as [`CommandStatus::Unknown`], not refused: see there.
fn status_from(raw: i64) -> CommandStatus {
    // The field is eight bits wide, so the number always fits.
    CommandStatus::of_number(raw as u8)
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
    /// What [`opcode_of`] and [`whole`] refuse -- an opcode no command has,
    /// or a length that is not exactly its layout's -- and a sample longer
    /// than one is shown.
    pub fn read(datagram: &[u8]) -> Result<Telemetry, LinkError> {
        // The header every answer starts with, from the fields every packet
        // has under the same names.
        macro_rules! head {
            ($p:expr, $kind:expr) => {
                TelemetryHeader {
                    sequence: $p.sequence,
                    tm_type: $kind,
                    status: status_from($p.status.0),
                    timestamp: Timestamp {
                        micros: $p.timestamp,
                    },
                }
            };
        }
        if opcode_of(datagram)?.taken_unmatched() {
            use fixed::FixedAnswerKind as F;
            return Ok(match whole::<F>(datagram)? {
                F::RestartArmAnswer(p) => {
                    Telemetry::RestartArm(RestartArmTelemetry {
                        header: head!(p, TelemetryType::RestartArm),
                    })
                }
                F::RestartAnswer(p) => Telemetry::Restart(RestartTelemetry {
                    header: head!(p, TelemetryType::Restart),
                }),
                F::ConnectAnswer(p) => Telemetry::Connect(ConnectTelemetry {
                    header: head!(p, TelemetryType::Connect),
                    version: ConfigVersion {
                        major: p.major,
                        minor: p.minor,
                        patch: p.patch,
                    },
                    digest: digest_of(&p.running),
                    newest_taken: p.newest_taken,
                }),
            });
        }
        use packet::AnswerLayoutKind as K;
        Ok(match whole::<K>(datagram)? {
            K::PingAnswer(p) => Telemetry::Ping(PingTelemetry {
                header: head!(p, TelemetryType::Ping),
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
}

/// Packs answers into a buffer it keeps, for the command interpreter, which
/// answers every command it takes from its main loop.
///
/// Two codecs, one for each document, as [`CommandPacker`] has.
pub struct AnswerPacker {
    fixed: Codec,
    ordinary: Codec,
}

impl AnswerPacker {
    /// Makes a packer, with a buffer for the longest answer of each
    /// document.
    ///
    /// # Returns
    ///
    /// The packer.
    pub fn new() -> Self {
        Self {
            fixed: Codec::for_packet::<fixed::FixedAnswerKind>(),
            ordinary: Codec::for_packet::<packet::AnswerLayoutKind>(),
        }
    }

    /// Packs an answer, in the packer's own buffer.
    ///
    /// # Arguments
    ///
    /// * `answer` - The answer.
    ///
    /// # Returns
    ///
    /// Its bytes on the link, good until the next pack.
    pub fn pack(&mut self, answer: &Telemetry) -> &[u8] {
        use fixed::FixedAnswerKind as F;
        use packet::AnswerLayoutKind as K;
        // The four fields every answer starts with, in either document.
        macro_rules! fill {
            ($doc:ident, $ty:ident, $h:expr; $($field:ident: $value:expr),*
             $(,)?) => {
                $doc::$ty {
                    sequence: $h.sequence,
                    status: $doc::StatusType(i64::from($h.status.number())),
                    timestamp: $h.timestamp.micros,
                    $($field: $value,)*
                }
            };
        }
        let p = match answer {
            Telemetry::RestartArm(t) => {
                let p = F::RestartArmAnswer(fill!(
                    fixed,
                    RestartArmAnswer,
                    t.header;
                ));
                return encoded(&mut self.fixed, &p);
            }
            Telemetry::Restart(t) => {
                let p =
                    F::RestartAnswer(fill!(fixed, RestartAnswer, t.header;));
                return encoded(&mut self.fixed, &p);
            }
            Telemetry::Connect(t) => {
                let p = F::ConnectAnswer(fill!(
                    fixed,
                    ConnectAnswer,
                    t.header;
                        running: fixed_bytes(&t.digest.0),
                        major: t.version.major,
                        minor: t.version.minor,
                        patch: t.version.patch,
                        newest_taken: t.newest_taken,
                ));
                return encoded(&mut self.fixed, &p);
            }
            Telemetry::Ping(t) => {
                K::PingAnswer(fill!(packet, PingAnswer, t.header;))
            }
            Telemetry::StartNode(t) => K::StartNodeAnswer(fill!(
                packet,
                StartNodeAnswer,
                t.header; node: t.node.0
            )),
            Telemetry::StopNode(t) => K::StopNodeAnswer(fill!(
                packet,
                StopNodeAnswer,
                t.header; node: t.node.0
            )),
            Telemetry::SetNode(t) => K::SetNodeAnswer(fill!(
                packet,
                SetNodeAnswer,
                t.header;
                    node: t.node.0,
                    reason: text(t.reason.as_str()),
            )),
            Telemetry::QueryNode(t) => {
                let (moved_known, last_moved) = when_of(t.counts.last_moved);
                let c = &t.counts;
                K::QueryNodeAnswer(fill!(
                    packet,
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
                    packet,
                    QueryNodeSampleAnswer,
                    t.header;
                        node: t.node.0,
                        taken_known: taken_known,
                        taken_time: taken_time,
                        taken_length: t.taken.len,
                        taken_total: t.taken.total,
                        taken_bytes_shown: fixed_bytes(&t.taken.bytes),
                        given_known: given_known,
                        given_time: given_time,
                        given_length: t.given.len,
                        given_total: t.given.total,
                        given_bytes_shown: fixed_bytes(&t.given.bytes),
                ))
            }
        };
        encoded(&mut self.ordinary, &p)
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

    /// The tcs-xtce digest of `xtce/fixed.xml`, which must never change.
    ///
    /// The fixed commands are what two ends that agree on nothing else still
    /// agree on, so a change to their document is a change to every ground
    /// station and every vehicle at once, including the ones already flying.
    /// A test that fails here is not a constant to update: the change to
    /// `fixed.xml` is to be undone, and a new fixed command made instead,
    /// with an opcode of its own and a document of its own.
    const FIXED_DIGEST: &str = "87aa3a5da4546803bba49afe56c66ba8";

    /// Makes one command of every kind, and a SET_NODE as long as one can be.
    ///
    /// # Returns
    ///
    /// The commands.
    fn every_command() -> Vec<Command> {
        vec![
            Command::Ping(PingCommand::new(1)),
            Command::Connect(ConnectCommand::new(2, ConfigDigest([0x5a; 16]))),
            Command::RestartArm(RestartArmCommand::new(
                3,
                ArmKey(0x1234),
                ConfigDigest([0x11; 16]),
            )),
            Command::Restart(RestartCommand::new(
                4,
                ArmKey(0x1234),
                ConfigDigest([0x22; 16]),
            )),
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
            Telemetry::Restart(RestartTelemetry::new(
                5,
                CommandStatus::Timeout,
            )),
            Telemetry::RestartArm(RestartArmTelemetry::new(
                12,
                CommandStatus::NotFound,
            )),
            Telemetry::Ping(PingTelemetry::new(13, CommandStatus::Unmatched)),
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
                CommandStatus::Stale,
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
        for command in every_command() {
            let packed = command.to_bytes();
            assert!(packed.len() <= COMMAND_BYTES);
            assert_eq!(Command::read(&packed), Ok(command.clone()));
        }
    }

    #[test]
    fn every_answer_reads_back_as_itself() {
        for answer in every_answer() {
            let packed = answer.to_bytes();
            assert!(packed.len() <= ANSWER_BYTES);
            assert_eq!(Telemetry::read(&packed), Ok(answer.clone()));
        }
    }

    /// The fixed commands' document is the one their layouts are pinned
    /// by: see [`FIXED_DIGEST`].
    #[test]
    fn the_fixed_commands_document_never_changes() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("xtce/fixed.xml");
        let digest = tcs_xtce::digest::digest_files_bytes(&[path])
            .expect("fixed.xml digests");
        let said: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            said, FIXED_DIGEST,
            "tcslibgs/xtce/fixed.xml has changed, and it must never change: \
             the layouts of CONNECT, RESTART_ARM and RESTART are what every \
             ground station and every vehicle agree on whatever else they \
             run. Undo the change, and make a new fixed command, with an \
             opcode of its own from 0x00 to 0x0F, in a document of its own"
        );
    }

    /// Each fixed command and answer is exactly the bytes the design's
    /// table gives, most significant byte first, with no header.
    #[test]
    fn the_fixed_commands_are_laid_out_as_the_table_says() {
        let target = ConfigDigest(core::array::from_fn(|i| 0xa0 + i as u8));
        let arm = Command::RestartArm(RestartArmCommand::new(
            0x0102_0304,
            ArmKey(0x1122_3344_5566_7788),
            target,
        ))
        .to_bytes();
        let mut want = vec![0x02, 0x01, 0x02, 0x03, 0x04];
        want.extend([0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]);
        want.extend(target.0);
        assert_eq!(arm, want, "RESTART_ARM: opcode, sequence, key, target");
        assert_eq!(arm.len(), 29);

        let restart = Command::Restart(RestartCommand::new(
            0x0102_0304,
            ArmKey(0x1122_3344_5566_7788),
            target,
        ))
        .to_bytes();
        want[0] = 0x03;
        assert_eq!(restart, want, "RESTART: as RESTART_ARM, opcode 0x03");

        let connect =
            Command::Connect(ConnectCommand::new(7, target)).to_bytes();
        let mut want = vec![0x04, 0, 0, 0, 7];
        want.extend(target.0);
        assert_eq!(connect, want, "CONNECT: opcode, sequence, checksum");
        assert_eq!(connect.len(), 21);

        let mut answer = ConnectTelemetry::new(
            7,
            ConfigVersion {
                major: 1,
                minor: 2,
                patch: 3,
            },
            target,
            0x0a0b_0c0d,
        );
        answer.header.timestamp = Timestamp { micros: 0x0102 };
        let packed = Telemetry::Connect(answer).to_bytes();
        let mut want = vec![0x04, 0, 0, 0, 7, 0];
        want.extend([0, 0, 0, 0, 0, 0, 0x01, 0x02]);
        want.extend(target.0);
        want.extend([1, 2, 3, 0x0a, 0x0b, 0x0c, 0x0d]);
        assert_eq!(
            packed, want,
            "CONNECT's answer: opcode, sequence, status, time, running \
             checksum, build version, newest sequence taken"
        );
        assert_eq!(packed.len(), 37);

        for (answer, opcode) in [
            (
                Telemetry::RestartArm(RestartArmTelemetry::new(
                    9,
                    CommandStatus::NotFound,
                )),
                0x02,
            ),
            (
                Telemetry::Restart(RestartTelemetry::new(
                    9,
                    CommandStatus::NotFound,
                )),
                0x03,
            ),
        ] {
            let packed = answer.to_bytes();
            assert_eq!(packed.len(), 14, "{answer:?}");
            assert_eq!(&packed[..6], &[opcode, 0, 0, 0, 9, 5], "{answer:?}");
        }
    }

    /// An ordinary command and answer have no header either: the opcode,
    /// the sequence and what they carry, and for an answer the status and
    /// the time between.
    #[test]
    fn an_ordinary_command_and_answer_have_no_header() {
        let packed =
            Command::StartNode(NodeCommand::start(0x0102_0304, NodeId(7)))
                .to_bytes();
        assert_eq!(packed, vec![0x30, 1, 2, 3, 4, 0, 0, 0, 7]);

        let mut ping = PingTelemetry::new(7, CommandStatus::NotArmed);
        ping.header.timestamp = Timestamp { micros: 0x0102 };
        let packed = Telemetry::Ping(ping).to_bytes();
        assert_eq!(
            packed,
            vec![0x01, 0, 0, 0, 7, 0x04, 0, 0, 0, 0, 0, 0, 0x01, 0x02]
        );
    }

    /// A datagram is taken only if its length is exactly what its opcode's
    /// layout gives, and only with an opcode a command has: one too short,
    /// one too long and one of an unknown opcode are all refused, commands
    /// and answers alike.
    #[test]
    fn a_datagram_of_the_wrong_length_or_opcode_is_refused() {
        for command in every_command() {
            let good = command.to_bytes();
            let mut long = good.clone();
            long.push(0);
            assert!(Command::read(&long).is_err(), "{command:?} and a byte");
            assert!(
                Command::read(&good[..good.len() - 1]).is_err(),
                "{command:?} a byte short"
            );
        }
        for answer in every_answer() {
            let good = answer.to_bytes();
            let mut long = good.clone();
            long.push(0);
            assert!(Telemetry::read(&long).is_err(), "{answer:?} and a byte");
            assert!(
                Telemetry::read(&good[..good.len() - 1]).is_err(),
                "{answer:?} a byte short"
            );
        }
        assert_eq!(Command::read(b""), Err(LinkError::Empty));
        assert_eq!(Telemetry::read(b""), Err(LinkError::Empty));
        for opcode in [0x00, 0x05, 0x0f, 0x20, 0x81, 0x84, 0xff] {
            let datagram = [opcode, 0, 0, 0, 1];
            assert_eq!(
                Command::read(&datagram),
                Err(LinkError::Opcode(opcode)),
                "{opcode:#04x}"
            );
        }
    }

    /// A status this build does not know is read as one, with its number,
    /// rather than refusing the answer: a vehicle of a later build may add
    /// one. It is a refusal, and says its number.
    #[test]
    fn a_status_this_build_does_not_know_is_read_with_its_number() {
        for answer in [
            Telemetry::Ping(PingTelemetry::new(3, CommandStatus::Success)),
            Telemetry::Connect(ConnectTelemetry::new(
                3,
                ConfigVersion::of_this_build(),
                ConfigDigest([1; 16]),
                3,
            )),
        ] {
            let mut packed = answer.to_bytes();
            packed[5] = 42;
            let read = Telemetry::read(&packed).expect("it is still an answer");
            assert_eq!(read.status(), CommandStatus::Unknown(42));
            assert!(!read.status().is_success());
            let said = read.status().to_string();
            assert!(said.contains("refused"), "{said}");
            assert!(said.contains("42"), "{said}");
            // And it goes back on the link as the number it came as.
            assert_eq!(read.to_bytes()[5], 42);
        }
    }

    /// The status numbers are the XTCE's, in both documents.
    #[test]
    fn the_status_numbers_are_the_documents() {
        for (status, link, fixed) in [
            (
                CommandStatus::Success,
                packet::StatusType::SUCCESS,
                Some(fixed::StatusType::SUCCESS),
            ),
            (
                CommandStatus::Failure,
                packet::StatusType::FAILURE,
                Some(fixed::StatusType::FAILURE),
            ),
            (
                CommandStatus::InvalidCommand,
                packet::StatusType::INVALID_COMMAND,
                Some(fixed::StatusType::INVALID_COMMAND),
            ),
            (
                CommandStatus::InvalidParameter,
                packet::StatusType::INVALID_PARAMETER,
                Some(fixed::StatusType::INVALID_PARAMETER),
            ),
            (
                CommandStatus::NotArmed,
                packet::StatusType::NOT_ARMED,
                Some(fixed::StatusType::NOT_ARMED),
            ),
            (
                CommandStatus::NotFound,
                packet::StatusType::NOT_FOUND,
                Some(fixed::StatusType::NOT_FOUND),
            ),
            (
                CommandStatus::AlreadyExists,
                packet::StatusType::ALREADY_EXISTS,
                None,
            ),
            (CommandStatus::Timeout, packet::StatusType::TIMEOUT, None),
            (
                CommandStatus::Stale,
                packet::StatusType::STALE,
                Some(fixed::StatusType::STALE),
            ),
            (
                CommandStatus::Unmatched,
                packet::StatusType::UNMATCHED,
                Some(fixed::StatusType::UNMATCHED),
            ),
        ] {
            assert_eq!(i64::from(status.number()), link.0, "{status:?}");
            if let Some(fixed) = fixed {
                assert_eq!(i64::from(status.number()), fixed.0, "{status:?}");
            }
            assert_eq!(CommandStatus::of_number(status.number()), status);
        }
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

    /// The opcode on the link is the one `CommandType` names, for a command
    /// and for its answer alike: the tables those keep are held to the XTCE
    /// here, which is the one statement of what goes on the link.
    #[test]
    fn the_opcodes_are_the_tables_numbers() {
        for command in every_command() {
            assert_eq!(
                command.to_bytes()[0],
                command.cmd_type().to_u8(),
                "{:?}",
                command.cmd_type()
            );
        }
        for answer in every_answer() {
            assert_eq!(
                answer.to_bytes()[0],
                answer.tm_type().to_u8(),
                "{:?}",
                answer.tm_type()
            );
            assert_eq!(
                TelemetryType::from_u8(answer.tm_type().to_u8()),
                Some(answer.tm_type())
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
