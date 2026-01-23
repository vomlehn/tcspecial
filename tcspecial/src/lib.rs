//! TCSpecial - Spacecraft Command Interpreter and Data Handler Manager
//!
//! TCSpecial runs on the spacecraft and manages communication between
//! ground operations and payloads.

pub mod ci;
pub mod config;
pub mod dh;
pub mod endpoint;
pub mod relay;

pub use ci::*;
pub use config::*;
pub use dh::*;
pub use endpoint::*;
pub use relay::*;
