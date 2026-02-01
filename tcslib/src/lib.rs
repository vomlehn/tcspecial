//! TCSpecial Ground Software Library (tcslib)
//!
//! This library provides a client interface for ground software to communicate
//! with the TCSpecial command interpreter running on spacecraft.

//pub mod client;
pub mod connection;

//pub use client::*;
pub use connection::*;
pub use tcslibgs::*;
