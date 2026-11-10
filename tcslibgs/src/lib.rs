//! TCSpecial Ground/Space Library (tcslibgs)
//!
//! What the ground portion of the software (tcslib) and the space portion
//! (tcspecial) share: the commands, the telemetry, and the types a data
//! handler is described in.
//!
//! The two GUIs share it as well, which is why more than the link's own
//! definitions live here. Every configuration file format is parsed in this
//! library, the simulator's included, so that no program reads a file in a
//! way of its own; and a time or a sample of a transfer is formatted here, so
//! that a panel in tcsmoc and a panel in tcssim show the one transfer the
//! same way.

/// The beacon, as it goes on the link.
pub mod beacon;
/// The beacon's packing code, which tcs-xtce generated from
/// `xtce/beacon.xtce`: used through [`beacon`].
mod beacon_packet;
/// The CCSDS Space Packet primary header every packet to and from a Yamcs
/// ground station carries.
pub mod ccsds;
/// What crosses a cFS software bus: the cFS packet around a tcspecial one.
pub mod cfs;
pub mod commands;
pub mod config;
pub mod config_digest;
/// The CRC every packet ends with, for what is not a packet.
pub mod crc;
pub mod display;
pub mod endpoint_config;
pub mod error;
/// The fixed commands' and answers' packing code, which tcs-xtce generated
/// from `xtce/fixed.xtce`: used through [`link`].
mod fixed_packet;
pub mod format;
/// What the vehicle answers to: the ground by radio, cFS, or F´.
pub mod gateway;
pub mod graph;
/// The XTCE documents every packet on the link is stated in, and the digest
/// that says which of them a program speaks.
pub mod interface;
/// The digest of this build's interface, which `make interface-digest`
/// generated from `xtce/link.xtce` and `xtce/types.xtce`: used through
/// [`interface`].
mod interface_digest;
/// Commands and answers, as they go on the link.
pub mod link;
/// The ordinary commands' and answers' packing code, which tcs-xtce
/// generated from `xtce/link.xtce`: used through [`link`].
mod link_packet;
/// The type a link between two nodes carries: see "Typed links" in
/// `docs/design.rst`.
pub mod link_type;
pub mod node;
pub mod parameters;
/// The ping's and its answer's packing code, which tcs-xtce generated from
/// `xtce/ping.xtce`, the ping node's interface document: what the tests of
/// [`node::vehicle::ping`] hold the document to that module with. Not
/// compiled into anything else, the ping node answering by that module.
#[cfg(test)]
mod ping_packet;
pub mod protocol;
/// Whether the simulator is ready: what a harness and tcssim say on tcssim's
/// readiness socket.
pub mod ready;
/// SBN, the Software Bus Network, as one peer of a cFS software bus speaks it.
pub mod sbn;
/// The simulator's own configuration language.
///
/// Here with the other three rather than in tcssim: both GUIs read it now --
/// one to simulate payloads and one to show what a payload set says -- and a
/// configuration language read by two programs cannot live inside one of
/// them.
pub mod sim_config;
/// How a simulator file groups its simulated payloads by type and names its
/// groups.
pub mod sim_grouped;
/// The shapes simulator files and physics models were written in before
/// that, refused by line.
pub mod sim_reshaped;
/// The simulation mode, `--simulate`, as tcspecial, tcssim and tcsmoc take it.
pub mod simulate;
/// Configurations kept on board, named by their checksum.
pub mod store;
pub mod telemetry;
pub mod trigger;
pub mod types;
/// The codec of each type a link between nodes can carry, which tcs-xtce
/// generated from `xtce/types.xtce`: see "Typed links" in
/// `docs/design.rst`.
pub mod types_packet;
/// The units a value may be in, and how finely each is carried.
pub mod unit;
pub mod verify;
pub mod whole;

pub use beacon::*;
pub use commands::*;
// The value types a runtime endpoint configuration carries. They are defined
// with the kind of group that states them and named from here as well, since
// by the time a handler holds one it is no longer a matter of configuration
// files.
pub use config::*;
pub use config_digest::*;
pub use display::{screen_height, Edge};
pub use endpoint_config::Scalar;
pub use error::*;
pub use format::*;
pub use interface::InterfaceDigest;
pub use link::{
    AnswerPacker, CommandPacker, LinkError, ANSWER_BYTES, COMMAND_BYTES,
};
pub use node::payload::can::CanParams;
pub use node::payload::gpio::{Bias, GpioParams};
pub use node::payload::hdlc::{
    ClockType, Encoding, FrameCheck, HdlcParams, Standard,
};
pub use node::payload::iio::IioParams;
pub use node::payload::rpmsg::RpmsgParams;
pub use node::payload::serial::rs485::Rs485Params;
pub use node::payload::serial::{ByteLength, Parity, StopBits};
pub use node::payload::spi::{BitOrder, BitsPerWord, CsActive, SpiMode};
pub use parameters::*;
pub use protocol::*;
pub use sim_config::*;
pub use telemetry::*;
pub use trigger::*;
pub use types::*;
pub use verify::*;
