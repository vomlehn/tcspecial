//! TCSpecial Spacecraft Process Library
//!
//! This library contains the core components of the tcspecial spacecraft process,
//! including the command interpreter (CI) and data handlers (DHs).

pub mod config;
pub mod endpoint;
pub mod relay;
pub mod dh;
pub mod ci;

pub use config::*;
pub use endpoint::*;
pub use relay::*;
pub use dh::*;
pub use ci::*;

// Re-export common types from tcslibgs
pub use tcslibgs::{
    Command, Telemetry, TcsError, TcsResult,
    DHId, DHName, DHType, ArmKey, BeaconTime, Statistics, Timestamp,
};
