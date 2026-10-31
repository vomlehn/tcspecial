//! TCSpecial Ground Software Library (tcslib)
//!
//! This library provides a client interface for ground software to communicate
//! with the TCSpecial command interpreter running on spacecraft.
//!
//! It is also how ground software gets the vehicle's data from a gateway node
//! -- see [`gateway`] -- how it listens for the spacecraft's beacons, which is
//! the one thing it can know without asking: see [`beacon`] -- and how it pings
//! a `ping` node and times the answer: see [`ping`]. A harness that starts
//! tcssim before tcspecial waits for it to be ready through [`simulator`].
//! Whether a station and the vehicle are using the same configuration, which
//! its commands and its data wait for, and what it has received, are
//! [`matched`]'s. A station that only listens sends its own CONNECT
//! through [`listening`], since the vehicle sends nothing down until one.

pub mod beacon;
pub mod client;
pub mod connection;
pub mod gateway;
pub mod listening;
pub mod matched;
pub mod ping;
pub mod simulator;

pub use client::*;
pub use connection::*;
pub use tcslibgs::*;

#[cfg(test)]
mod running;
