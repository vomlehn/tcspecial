//! One module per node type: the simulator's stand-in, in families.
//!
//! What goes at the far end of a node so that a configuration can be run with
//! no hardware present. The attributes come from `tcslibgs::node` and the
//! flight side is `tcspecial::node`; the three trees carry the same names, so
//! a type is `node/payload/i2c` wherever you are standing.
//!
//! See `tcslibgs::node` for why there are three trees rather than one, and for
//! what the families are.
//!
//! Mostly `payload`, and the reason is what a simulator is for. What stands
//! in for a thing is only needed where the thing is absent, and what is absent
//! on a workstation is the hardware: a computation and a log run on the
//! workstation exactly as they run aboard, and a ground link is a socket that
//! tcsmoc is at the other end of.
//!
//! The exception is `vehicle::thermostat`, which stands in for a thermostat
//! node so that a zone's loop -- sensors, a decision, heaters -- can
//! be closed in the simulator alone, with no tcspecial running.
//!
//! Not every payload kind has one either. A sensor's buffer is filled by the
//! sensor and an IIO node cannot be triggered, so what stands in for one is
//! the kernel's dummy rather than anything here.

/// What the vehicle carries: the stand-ins for hardware that is not here.
pub mod payload;
/// The physics model the stand-ins measure and act on: one module per
/// component type, beside the stand-ins that are joined to them.
pub mod physics;
/// What the vehicle computes, where the simulator stands in for it.
pub mod vehicle;
