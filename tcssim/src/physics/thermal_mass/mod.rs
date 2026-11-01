//! A thermal mass: a body whose temperature heaters raise, the environment
//! pulls back, and sensors report.

pub mod sensor;
// The module is named for the directory's subject, as is the directory.
#[allow(clippy::module_inception)]
pub mod thermal_mass;
