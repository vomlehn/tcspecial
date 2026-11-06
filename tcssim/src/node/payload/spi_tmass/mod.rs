//! The simulation of a temperature sensor that is a SPI peripheral, fixed to
//! a thermal mass.
//!
//! Simulated as [`super::i2c_tmass`] is, and for its reasons: a UDP
//! payload at the node's `sim_address` where it gives one, and otherwise the
//! pseudo-terminal a `spi` peripheral is simulated by -- see
//! [`crate::node::payload::spi`]. What it reads is the thermal mass it is
//! joined to.
