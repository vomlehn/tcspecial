//! A temperature sensor that is a SPI peripheral, fixed to a thermal mass.
//!
//! A `spi` peripheral in everything a vehicle does with it, differing in what
//! it is, which the type says: see [`super::i2c_tmass`], which is the same
//! thing on an I2C bus and says why the two are types of their own. A run that
//! simulates it may reach it over UDP at its `sim_address`, as it may a `spi`
//! node, where tcs-sim answers with the temperature of the thermal mass it is
//! joined to.

use crate::endpoint_config::{EndpointConfigResult, GroupKind, LinkTerms};

/// The word a file writes for this type.
pub const TYPE: &str = "spi_tmass";

/// Settles a `spi_tmass` node's terms, as a `spi` node's are settled.
///
/// # Arguments
///
/// * `name` - The node's name, for the messages.
/// * `g`    - The terms as the file states them.
///
/// # Returns
///
/// The bus's terms.
///
/// # Errors
///
/// What [`super::spi::group_kind_named`] refuses, naming this type.
pub(crate) fn group_kind_of(
    name: &str,
    g: LinkTerms,
) -> EndpointConfigResult<GroupKind> {
    super::spi::group_kind_named(name, TYPE, g)
}

/// Judges whether the peripheral is a simulated device, as a `spi` node's is:
/// see [`super::spi::judged`].
///
/// # Arguments
///
/// * `path` - The device's path.
///
/// # Returns
///
/// The judgement.
pub fn judged(path: &str) -> crate::simulate::Judged {
    super::spi::judged(path)
}
