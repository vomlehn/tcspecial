//! One module per node type: what the simulator plays, in families.
//!
//! What goes at the far end of a node so that a configuration can be run with
//! no hardware present. The attributes come from `tcslibgs::node` and the
//! flight side is `tcspecial::node`; the three trees carry the same names, so
//! a type is `node/payload/i2c` wherever you are standing.
//!
//! See `tcslibgs::node` for why there are three trees rather than one, and for
//! what the families are.
//!
//! Mostly `payload`, and the reason is what a simulator is for. A simulation of
//! a thing is only needed where the thing is absent, and what is absent on a
//! workstation is the hardware: a computation and a log run on the workstation
//! exactly as they run aboard, and a ground link is a socket that tcsmoc is at
//! the other end of.
//!
//! The exception is `vehicle::thermostat`, which simulates a thermostat
//! node so that a zone's loop -- sensors, a decision, heaters -- can
//! be closed in the simulator alone, with no tcspecial running.
//!
//! Not every payload kind has one either. A sensor's buffer is filled by the
//! sensor and an IIO node cannot be triggered, so what takes its place is the
//! kernel's dummy device rather than anything here.

/// What the vehicle carries: simulations of hardware that is not here.
pub mod payload;
/// The physics model the simulated payloads measure and act on: one module
/// per component type, beside the payloads that are joined to them.
pub mod physics;
/// What the vehicle computes, where the simulator simulates it.
pub mod vehicle;
