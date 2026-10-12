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
    reject_foreign_fields, reject_stream, EndpointConfigResult, GroupKind, LinkTerms,
};

/// What an IIO payload says about its buffer.
///
/// Nothing, which is the point of its being a type with no terms. A buffer is
/// read as it comes, and everything that decides what is in it -- which
/// channels are enabled, how often the device samples -- is set through
/// `sysfs` by whoever brought the sensor up. A payload that stated those
/// would be stating values nothing could apply: see the requirement in
/// `design.rst` that a payload says only what it can set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct IioParams;

/// What an IIO payload says about its buffer, which is nothing.
///
/// Here all the same, and not as a shortcut: this is where a term belonging to
/// another type of link is refused, and where a term that used to belong to
/// this one is told that it no longer does.
pub(crate) fn group_kind_of(name: &str, g: LinkTerms) -> EndpointConfigResult<GroupKind> {
    reject_foreign_fields(name, "iio", &g, &[])?;

    // A buffer read hands over whole sample sets, as many as are ready, so
    // there is nothing for a rule about where a read ends to say.
    reject_stream(name, "iio", g.stream.is_some())?;

    Ok(GroupKind::Iio(IioParams))
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
