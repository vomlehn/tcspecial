//! What the `tcs-verify` program does besides checking, and what the ground
//! programs share of how they show what was found.
//!
//! The checking is `tcsverify`'s, which tcspecial runs aboard: this crate
//! is the tool's side of it, which never flies -- writing a configuration's
//! interface digest, its own XTCE and its F´ model, laying out its summary,
//! and gathering what a program found as it started for the window tcs-moc
//! and tcs-sim open before their main one.

pub mod fprime_model;
pub mod own_xtce;
pub mod set_interface;
pub mod startup;
pub mod summary;
