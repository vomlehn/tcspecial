//! What the vehicle carries, and the ground link with it.
//!
//! A device on a bus, a line the vehicle drives, a link to something aboard.
//! These are the types the configuration's `payloads` section holds -- the
//! part that changes between missions, which is why the tree is divided this
//! way.
//!
//! Two of these are payloads that are not devices: `app_pl` and `app_all_pl`
//! are programs the mission wrote, run by tcspecial and read the way a sensor
//! is read. What a file may say about either is `tcslibgs::node::app`, that
//! rule being one rule for all five app node types, and the interfaces their
//! programs are run with are `tcsapp`.
//!
//! `network` is here and serves two sections: a network payload and the ground
//! link are the same endpoint code, a socket being a socket whichever end of
//! the mission is at the other end of it. That is why there is no `moc`
//! directory -- see `tcslibgs::node`.
//!
//! Which of the types are here is chosen when tcspecial is built: each is a
//! Cargo feature named as a configuration writes the type, and [`BUILT`] is
//! the list of those this build contains. `network` is always here, because
//! the ground link is one; its raw protocol is the feature `raw`. See "Which
//! Node Types Are Built" in docs/design.rst.

#[cfg(feature = "can")]
pub mod can;
#[cfg(feature = "device-file")]
pub mod device_file;
#[cfg(feature = "gpio")]
pub mod gpio;
#[cfg(feature = "hdlc")]
pub mod hdlc;
#[cfg(feature = "i2c")]
pub mod i2c;
#[cfg(feature = "iio")]
pub mod iio;
pub mod network;
#[cfg(feature = "rpmsg")]
pub mod rpmsg;
#[cfg(feature = "serial")]
pub mod serial;
#[cfg(feature = "spi")]
pub mod spi;
// A payload that is a program rather than a device: one that is asked for a
// reading, and one that is never asked.
pub mod app_all_pl;
pub mod app_pl;

/// The optional payload types this build contains, as a configuration writes
/// them.
///
/// The one list of them: the check made as a configuration is loaded, the
/// startup summary and `tcspecial --types` all read it, so none can disagree
/// with the others about what was built. The words are
/// [`tcslibgs::graph::built::OPTIONAL_TYPES`], in that order.
pub const BUILT: &[&str] = &[
    #[cfg(feature = "device-file")]
    "device-file",
    #[cfg(feature = "serial")]
    "serial",
    #[cfg(feature = "i2c")]
    "i2c",
    #[cfg(feature = "spi")]
    "spi",
    #[cfg(feature = "can")]
    "can",
    #[cfg(feature = "iio")]
    "iio",
    #[cfg(feature = "rpmsg")]
    "rpmsg",
    #[cfg(feature = "gpio")]
    "gpio",
    #[cfg(feature = "hdlc")]
    "hdlc",
    #[cfg(feature = "raw")]
    "raw",
];

// A type the platform cannot have stops the build, rather than building a
// tcspecial that fails only when a node of the type is opened. Each of these
// is a Linux interface: a feature says what was chosen, and this says what is
// possible.
#[cfg(all(feature = "i2c", not(target_os = "linux")))]
compile_error!(
    "the i2c payload type needs Linux's i2c-dev, which this platform does not \
     have: build without the i2c feature"
);
#[cfg(all(feature = "spi", not(target_os = "linux")))]
compile_error!(
    "the spi payload type needs Linux's spidev, which this platform does not \
     have: build without the spi feature"
);
#[cfg(all(feature = "can", not(target_os = "linux")))]
compile_error!(
    "the can payload type needs Linux's SocketCAN, which this platform does \
     not have: build without the can feature"
);
#[cfg(all(feature = "iio", not(target_os = "linux")))]
compile_error!(
    "the iio payload type needs Linux's Industrial I/O subsystem, which this \
     platform does not have: build without the iio feature"
);
#[cfg(all(feature = "rpmsg", not(target_os = "linux")))]
compile_error!(
    "the rpmsg payload type needs Linux's rpmsg character devices, which this \
     platform does not have: build without the rpmsg feature"
);
#[cfg(all(feature = "gpio", not(target_os = "linux")))]
compile_error!(
    "the gpio payload type needs Linux's GPIO character devices, which this \
     platform does not have: build without the gpio feature"
);
#[cfg(all(feature = "hdlc", not(target_os = "linux")))]
compile_error!(
    "the hdlc payload type needs Linux's generic HDLC, which this platform \
     does not have: build without the hdlc feature"
);

#[cfg(test)]
mod tests {
    use super::BUILT;
    use tcslibgs::graph::built::OPTIONAL_TYPES;

    /// Built with its defaults, tcspecial contains every type a build may
    /// leave out, in the order that list has. A feature misnamed here, or a
    /// type added there and not here, is a type no build could contain.
    #[test]
    #[cfg(all(
        feature = "device-file",
        feature = "serial",
        feature = "i2c",
        feature = "spi",
        feature = "can",
        feature = "iio",
        feature = "rpmsg",
        feature = "gpio",
        feature = "hdlc",
        feature = "raw"
    ))]
    fn every_type_is_built_by_default() {
        assert_eq!(BUILT, OPTIONAL_TYPES.as_slice());
    }

    /// Every word in the list is one a configuration can need: a word that
    /// was not would say a type was built that nothing could ask for.
    #[test]
    fn every_built_type_is_an_optional_type() {
        for built in BUILT {
            assert!(OPTIONAL_TYPES.contains(built), "{built}");
        }
    }

    /// The features in Cargo.toml are the optional types, exactly: one
    /// missing there could not be built, and one extra would build nothing.
    #[test]
    fn the_features_are_the_optional_types() {
        let manifest = include_str!("../../../Cargo.toml");
        let features = manifest
            .split("\n[features]\n")
            .nth(1)
            .expect("Cargo.toml has a [features] table");
        let features = features.split("\n[").next().unwrap_or(features);
        let mut declared: Vec<&str> = features
            .lines()
            .filter_map(|line| line.strip_suffix(" = []"))
            .collect();
        declared.sort_unstable();
        let mut optional = OPTIONAL_TYPES.to_vec();
        optional.sort_unstable();
        assert_eq!(declared, optional);
    }
}
