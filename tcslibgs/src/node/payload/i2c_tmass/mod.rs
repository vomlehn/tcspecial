//! A temperature sensor on an I2C bus, fixed to a thermal mass.
//!
//! An `i2c` device in everything a vehicle does with it: the same bus and
//! address, the same terms, the same reads. What differs is what it is, which
//! a configuration says by its type and a reader takes in at a glance, and
//! one thing a desk needs: a run with stand-ins may reach it over UDP at its
//! `stand_in`, where tcssim answers with the temperature of the thermal mass
//! it is joined to. See [`crate::graph::reach::stands_in_over_udp`].
//!
//! So nothing here is a rule of its own. The terms are `i2c`'s, read by
//! [`super::i2c::group_kind_named`] under this type's name so that a
//! complaint names the type the file wrote, and a bus is judged a stand-in
//! as `i2c` judges one. A rule written for `i2c` is this type's too, in one
//! place.

use crate::endpoint_config::{EndpointConfigResult, GroupKind, LinkTerms};

/// The word a file writes for this type.
pub const TYPE: &str = "i2c_tmass";

/// Settles an `i2c_tmass` node's terms, as an `i2c` node's are settled.
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
/// What [`super::i2c::group_kind_named`] refuses, naming this type.
pub(crate) fn group_kind_of(
    name: &str,
    g: LinkTerms,
) -> EndpointConfigResult<GroupKind> {
    super::i2c::group_kind_named(name, TYPE, g)
}

/// Judges whether the bus is a stand-in, as an `i2c` node's is: see
/// [`super::i2c::judged`].
///
/// # Arguments
///
/// * `host` - The host, whose `/sys` is read.
/// * `bus`  - The bus's device path.
///
/// # Returns
///
/// The judgement.
pub fn judged(
    host: &crate::simulate::Host,
    bus: &str,
) -> crate::simulate::Judged {
    super::i2c::judged(host, bus)
}
