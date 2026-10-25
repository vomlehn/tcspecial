//! IIO payloads: a sensor's buffered samples.
//!
//! **Experimental, and not yet supported.** The terms parse and reach a
//! handler, and the handler reads the device; nothing has been run against a
//! real sensor.
//!
//! The kernel's Industrial I/O subsystem is what a thermistor, an
//! accelerometer or an ADC looks like on Linux. A driver offers channels, and
//! a program that wants a stream of them enables the channels it cares about,
//! enables a buffer, and then reads `/dev/iio:deviceN` -- which hands over
//! packed samples, as many as are ready, for as long as the buffer is
//! enabled.
//!
//! So an IIO payload is a device file a handler reads, and it is a type of its
//! own for one reason: it is read-only. A buffer is a stream from the sensor
//! and there is nothing to write, so such a payload cannot be triggered --
//! there is nowhere to send a request -- and a file asking for one is refused
//! rather than left to fail as silence.
//!
//! It takes no terms. It used to take two, `channels` and `sample_rate`, which
//! said how many channels were enabled and how fast the device samples; both
//! were recorded and neither was applied, because enabling a channel and a
//! buffer is done through `sysfs` -- `/sys/bus/iio/devices/iio:deviceN/` -- by
//! whoever brought the sensor up, and a handler that wrote there would be
//! reconfiguring a sensor other programs may be reading. A payload documents
//! and takes only what it can set, so they are gone: a value a file states and
//! nothing applies is a file that looks configured and is not.

use serde::{Deserialize, Serialize};

use crate::endpoint_config::{
    reject_foreign_fields, reject_stream, EndpointConfigResult, GroupKind,
    LinkTerms,
};

/// What an IIO payload says about its buffer.
///
/// Nothing, which is the point of its being a type with no terms. A buffer is
/// read as it comes, and everything that decides what is in it -- which
/// channels are enabled, how often the device samples -- is set through
/// `sysfs` by whoever brought the sensor up. A payload that stated those
/// would be stating values nothing could apply: see the requirement in
/// `design.rst` that a payload says only what it can set.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub struct IioParams;

/// What an IIO payload says about its buffer, which is nothing.
///
/// Here all the same, and not as a shortcut: this is where a term belonging to
/// another type of link is refused, and where a term that used to belong to
/// this one is told that it no longer does.
pub(crate) fn group_kind_of(
    name: &str,
    g: LinkTerms,
) -> EndpointConfigResult<GroupKind> {
    reject_foreign_fields(name, "iio", &g, &[])?;

    // A buffer read hands over whole sample sets, as many as are ready, so
    // there is nothing for a rule about where a read ends to say.
    reject_stream(name, "iio", g.stream.is_some())?;

    Ok(GroupKind::Iio(IioParams))
}

/// What `--simulate` asks to change on an IIO node that is not a stand-in.
const TO_STAND_IN: &str =
    "point path at the iio_dummy sensor, which bin/mkiio makes, or at a \
     pseudo-terminal tcssim makes";

/// Whether the device at `path` is a stand-in: a sensor of the kernel's
/// `iio_dummy`, or a pseudo-terminal. See [`crate::simulate`].
///
/// A dummy sensor is made through configfs, under
/// `kernel/config/iio/devices/dummy`, and the directory made there is the
/// device's name; a real sensor's name is its part's.
pub fn judged(
    host: &crate::simulate::Host,
    path: &str,
) -> crate::simulate::Judged {
    let on_a_terminal =
        crate::simulate::pseudo_terminal(path, "path", TO_STAND_IN);
    if on_a_terminal.is_stand_in() {
        return on_a_terminal;
    }
    match host.device_of(path) {
        Ok(device) => sensor_judged(host, path, &device),
        Err(found) => found.judged("path", TO_STAND_IN),
    }
}

/// Whether the device at `path` is a sensor of the kernel's `iio_dummy`, made
/// by `bin/mkiio`: a stand-in the kernel makes, which the simulator finds
/// rather than makes, as it finds an I2C stub.
pub fn is_dummy(host: &crate::simulate::Host, path: &str) -> bool {
    match host.device_of(path) {
        Ok(device) => sensor_judged(host, path, &device).is_stand_in(),
        Err(_) => false,
    }
}

/// The judgement of a sensor whose kernel directory is `device`.
fn sensor_judged(
    host: &crate::simulate::Host,
    path: &str,
    device: &std::path::Path,
) -> crate::simulate::Judged {
    use crate::simulate::{is_dir, read_line, Judged};
    let Some(name) = read_line(&device.join("name")) else {
        return Judged::cannot_tell(
            format!("{path} has no name in {}", device.display()),
            "path",
            TO_STAND_IN,
        );
    };
    if !name.is_empty()
        && is_dir(&host.sys.join("kernel/config/iio/devices/dummy").join(&name))
    {
        Judged::stand_in(
            format!("{path} is {name}, a sensor of the kernel's iio_dummy"),
            "path",
            TO_STAND_IN,
        )
    } else {
        Judged::hardware(
            format!("{path} is {name}, a real sensor"),
            "path",
            TO_STAND_IN,
        )
    }
}

#[cfg(test)]
mod stand_in_tests {
    use super::*;
    use crate::simulate::{Host, Verdict};

    #[test]
    fn a_sensor_made_in_configfs_is_a_stand_in_and_another_is_not() {
        let sys = tempfile::tempdir().unwrap();
        let host = Host {
            sys: sys.path().to_path_buf(),
        };
        std::fs::create_dir_all(
            sys.path().join("kernel/config/iio/devices/dummy/tcssim"),
        )
        .unwrap();
        let dummy = sys.path().join("devices/iio:device0");
        std::fs::create_dir_all(&dummy).unwrap();
        std::fs::write(dummy.join("name"), "tcssim\n").unwrap();
        let real = sys.path().join("devices/iio:device1");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::write(real.join("name"), "bmp280\n").unwrap();

        assert_eq!(
            sensor_judged(&host, "/dev/iio:device0", &dummy).verdict,
            Verdict::StandIn
        );
        let judged = sensor_judged(&host, "/dev/iio:device1", &real);
        assert_eq!(judged.verdict, Verdict::Hardware);
        assert_eq!(judged.found, "/dev/iio:device1 is bmp280, a real sensor");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoint_config::Scalar;

    fn terms(stated: &[(&str, &str)]) -> LinkTerms {
        let mut g = LinkTerms::default();
        for (field, value) in stated {
            let value = Some(Scalar::of(*value));
            match *field {
                "datarate" => g.datarate = value,
                other => panic!("{other} is not a term this test states"),
            }
        }
        g
    }

    /// A buffer is read as it comes, and says nothing about itself.
    #[test]
    fn a_buffer_has_no_terms_of_its_own() {
        match group_kind_of("sensor", LinkTerms::default()).expect("a buffer") {
            GroupKind::Iio(_) => {}
            other => panic!("expected an IIO device, got {other:?}"),
        }
    }

    /// A term of another type of link is refused.
    #[test]
    fn a_term_of_another_link_is_refused() {
        let said = group_kind_of("sensor", terms(&[("datarate", "9600")]))
            .expect_err("a line's rate on a buffer");
        assert!(
            format!("{said}").contains("datarate"),
            "the error should name the term: {said}"
        );
    }
}
