//! TCSpecial Ground/Space Library (tcslibgs)
//!
//! This library contains definitions shared between the ground portion of the
//! software (tcslib) and the space portion (tcspecial).

pub mod commands;
pub mod config;
pub mod endpoint_config;
pub mod endpoint_config_i2c;
pub mod endpoint_config_network;
pub mod endpoint_config_serial;
pub mod endpoint_config_spi;
pub mod error;
pub mod format;
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
pub mod types;

pub use commands::*;
// The value types a runtime endpoint configuration carries. They are defined
// with the kind of group that states them and named from here as well, since
// by the time a handler holds one it is no longer a matter of configuration
// files.
pub use endpoint_config_serial::{ByteLength, StopBits};
pub use endpoint_config_spi::{BitOrder, BitsPerWord, CsActive, SpiMode};
pub use config::*;
pub use error::*;
pub use format::*;
pub use parameters::*;
pub use protocol::*;
pub use sim_config::*;
pub use telemetry::*;
pub use types::*;
