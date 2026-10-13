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

pub mod commands;
pub mod config;
pub mod config_digest;
pub mod display;
pub mod endpoint_config;
pub mod node;
pub mod error;
pub mod format;
pub mod graph;
pub mod parameters;
pub mod protocol;
/// The simulator's own configuration language.
///
/// Here with the other three rather than in tcssim: both GUIs read it now --
/// one to simulate payloads and one to show what a payload set says -- and a
/// configuration language read by two programs cannot live inside one of
/// them.
pub mod sim_config;
pub mod telemetry;
pub mod trigger;
pub mod types;
pub mod verify;

pub use commands::*;
// The value types a runtime endpoint configuration carries. They are defined
// with the kind of group that states them and named from here as well, since
// by the time a handler holds one it is no longer a matter of configuration
// files.
pub use endpoint_config::Scalar;
pub use node::payload::serial::{ByteLength, Parity, StopBits};
pub use node::payload::hdlc::{ClockType, Encoding, FrameCheck, HdlcParams, Standard};
pub use node::payload::can::CanParams;
pub use node::payload::gpio::{Bias, Edge as LineEdge, GpioParams};
pub use node::payload::iio::IioParams;
pub use node::payload::rpmsg::RpmsgParams;
pub use node::payload::serial::rs485::Rs485Params;
pub use node::payload::spi::{BitOrder, BitsPerWord, CsActive, SpiMode};
pub use config::*;
pub use display::{screen_height, Edge};
pub use error::*;
pub use format::*;
pub use parameters::*;
pub use protocol::*;
pub use sim_config::*;
pub use telemetry::*;
pub use config_digest::*;
pub use trigger::*;
pub use types::*;
pub use verify::*;
