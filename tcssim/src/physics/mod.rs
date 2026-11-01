//! The physical world the simulated payloads measure and act on.
//!
//! Where `node` holds the stand-ins for the hardware at the far end of each
//! node, this holds what that hardware would sense and change -- the state
//! of the world around the vehicle and how it evolves with time -- so that
//! what a sensor reports follows from what the vehicle did.

pub mod thermal_mass;
