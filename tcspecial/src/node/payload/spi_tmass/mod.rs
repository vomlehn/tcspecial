//! A temperature sensor that is a SPI peripheral, fixed to a thermal mass.
//!
//! Opened and read as a `spi` peripheral is, by the same endpoint -- see
//! [`super::spi`] -- and a type of its own for the reasons
//! [`super::i2c_tmass`] gives.
