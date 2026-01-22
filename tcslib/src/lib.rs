//! TCSpecial Library (tcslib)
//!
//! The TCSpecial library provides a set of operations for global control and status
//! and a set of per-payload interface operations. It is used for building control
//! applications using mission control software such as YAMCS or MCT.

pub mod client;
pub mod connection;

pub use client::*;
pub use connection::*;

// Re-export common types from tcslibgs
pub use tcslibgs::{
    Command, Telemetry, TcsError, TcsResult,
    DHId, DHName, DHType, ArmKey, BeaconTime, Statistics, Timestamp,
};
