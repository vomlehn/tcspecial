//! A temperature sensor on an I2C bus, fixed to a thermal mass.
//!
//! Opened and read as an `i2c` device is, by the same endpoint -- see
//! [`super::i2c`] and `crate::endpoint` -- because in flight it is one: a
//! bus, an address and a register read. Nothing of its own is run here, and
//! on purpose: a second copy of the code that reads a bus would be a second
//! place for a fault in it to be fixed in and missed.
//!
//! What makes it a type of its own is what it is, which a configuration says by
//! its type, and that a run that simulates it may reach it at its `sim_address`
//! over UDP, where tcs-sim answers with the temperature of the thermal mass it
//! is joined to. Under the flight setting `sim_address` is never used, and the
//! bus is what is opened: see `tcslib_protocol::graph::reach::Reach::opened`.
