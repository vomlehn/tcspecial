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
//! So an IIO payload is a device file a handler reads, which is a `device`
//! payload with two differences worth a kind of its own. It is read-only: a
//! buffer is a stream from the sensor and there is nothing to write, so such
//! a payload cannot be triggered -- there is nowhere to send a request. And
//! what the samples mean depends on which channels were enabled and how fast
//! they are sampled, which a `device` payload has no words for.
//!
//! Those two terms are recorded and not applied. Enabling a channel and a
//! buffer is done through `sysfs` -- `/sys/bus/iio/devices/iio:deviceN/` --
//! by the platform that brought the sensor up, not by a program that opens
//! the character device; a handler that wrote there would be reconfiguring a
//! sensor other programs may be reading. What the terms are for is saying
//! what the file expects, so that a stream of samples nobody can account for
//! can be held against what the configuration said.

use serde::{Deserialize, Serialize};

use crate::endpoint_config::{
    bad, parse_u32, reject_foreign_fields, reject_stream, EndpointConfigResult, GroupKind,
    LinkTerms,
};

/// What an IIO payload's device is expected to be producing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IioParams {
    /// How many channels are enabled, if the file says.
    ///
    /// Recorded, not applied. A reader of a buffer has to know how many
    /// channels are in each sample set to make sense of the bytes, and
    /// nothing in the byte stream says: this is where a file says it.
    pub channels: Option<u32>,
    /// How many sample sets a second the device is expected to produce, if
    /// the file says.
    ///
    /// Recorded, not applied, and not a packet interval: how fast a *real*
    /// sensor samples is a property of the equipment, where a simulated
    /// payload's rate is the simulator's own setting. This is what the
    /// equipment is expected to be doing.
    pub sample_rate: Option<u32>,
}

/// What an IIO payload says about its device, and what it must say.
///
/// Nothing is required. A payload that states neither term is a handler that
/// reads whatever the buffer hands it, which is what a `device` payload does
/// and is a perfectly good thing to be; the terms are for a file that knows
/// what the sensor was set up to produce.
pub(crate) fn group_kind_of(name: &str, g: LinkTerms) -> EndpointConfigResult<GroupKind> {
    reject_foreign_fields(name, "iio", &g, &["channels", "sample_rate"])?;

    // A buffer read hands over whole sample sets, as many as are ready, so
    // there is nothing for a rule about where a read ends to say.
    reject_stream(name, "iio", g.stream.is_some())?;

    let channels = match g.channels.as_ref() {
        Some(stated) => {
            let channels = parse_u32(name, "channels", stated)?;
            if channels == 0 {
                return Err(bad(
                    name,
                    "channels must be greater than zero: a buffer with no channels \
                     enabled produces nothing, and a payload that produces nothing \
                     is a payload nobody can read",
                ));
            }
            Some(channels)
        }
        None => None,
    };

    let sample_rate = match g.sample_rate.as_ref() {
        Some(stated) => {
            let rate = parse_u32(name, "sample_rate", stated)?;
            if rate == 0 {
                return Err(bad(name, "sample_rate must be greater than zero"));
            }
            Some(rate)
        }
        None => None,
    };

    Ok(GroupKind::Iio(IioParams {
        channels,
        sample_rate,
    }))
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
                "channels" => g.channels = value,
                "sample_rate" => g.sample_rate = value,
                "datarate" => g.datarate = value,
                other => panic!("{other} is not a term this test states"),
            }
        }
        g
    }

    fn params(stated: &[(&str, &str)]) -> IioParams {
        match group_kind_of("sensor", terms(stated)).expect("a device") {
            GroupKind::Iio(iio) => iio,
            other => panic!("expected an IIO device, got {other:?}"),
        }
    }

    /// A payload that says nothing is a buffer read as it comes.
    #[test]
    fn a_sensor_need_not_say_what_it_produces() {
        assert_eq!(
            params(&[]),
            IioParams {
                channels: None,
                sample_rate: None,
            }
        );
    }

    /// Both terms are read, and neither may be nought.
    #[test]
    fn what_a_sensor_may_say_about_itself() {
        assert_eq!(
            params(&[("channels", "3"), ("sample_rate", "100")]),
            IioParams {
                channels: Some(3),
                sample_rate: Some(100),
            }
        );

        for (field, value) in [("channels", "0"), ("sample_rate", "0")] {
            let said = group_kind_of("sensor", terms(&[(field, value)]))
                .expect_err("a sensor producing nothing");
            assert!(
                format!("{said}").contains(field),
                "the error should name the term: {said}"
            );
        }
    }

    /// A term of another kind of link is refused.
    #[test]
    fn a_term_of_another_link_is_refused() {
        let said = group_kind_of("sensor", terms(&[("datarate", "9600")]))
            .expect_err("a line's rate on a sensor");
        assert!(
            format!("{said}").contains("datarate"),
            "the error should name the term: {said}"
        );
    }
}
