//! TCSpecial Ground Software Library (tcslib)
//!
//! This library provides a client interface for ground software to communicate
//! with the TCSpecial command interpreter running on spacecraft.
//!
//! It is also how ground software gets the vehicle's data from a `moc` node:
//! see [`moc`].

pub mod client;
pub mod connection;
pub mod moc;

pub use client::*;
pub use connection::*;
pub use tcslibgs::*;
