//! The simulation of a temperature sensor on an I2C bus, fixed to a thermal
//! mass.
//!
//! Nothing of its own to run, and on purpose: a sensor is simulated where
//! tcspecial reaches it, and that is one of two places the simulator already
//! answers at. Where its node gives a `sim_address`, tcspecial reaches it over
//! UDP there, and the simulator is a UDP payload at that address -- see
//! [`crate::node::payload::network`]. Where it gives none, tcspecial opens the
//! bus as for an `i2c` device, and the simulator fills the i2c-stub's register
//! bank -- see [`crate::node::payload::i2c`]. `crate::endpoint` says which.
//!
//! Either way what it reads is what the simulator file joins it to: its
//! `connects` names the thermal mass's `temperature`, and each reading is
//! that temperature in the encoding the sensor's own conversion reads back.
//! See [`crate::node::physics`].
