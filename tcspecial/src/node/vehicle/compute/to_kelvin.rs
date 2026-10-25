//! Raw counts to kelvin.
//!
//! What a sensor puts on a bus is a number in its own units, and three sensors
//! of three makes put three different numbers there for the same temperature.
//! This is the node that makes them comparable: it reads the bytes the way the
//! part's datasheet says to, scales them, and adds an offset.
//!
//! Kelvin and not Celsius, for one reason worth stating: it has no negative
//! numbers in any range a vehicle sees, so a comparison, a mean and a
//! threshold are all arithmetic on positive numbers, and a sign error is a
//! value that is obviously wrong rather than plausibly cold.

use tcslibgs::node::vehicle::compute::{Format, ToKelvinParams};

use super::{Function, Out};

/// The number these bytes hold.
///
/// Exactly the width and no more: a reading longer than the format is a
/// sensor that was read wrongly or a bus that returned something else, and
/// taking the first two bytes of it would turn that into a plausible
/// temperature. Shorter is the same mistake the other way.
///
/// The format itself is `tcslibgs::node::vehicle::compute::Format`, where the
/// ground tool can see it; what a byte means is arithmetic and lives here.
fn read(format: Format, data: &[u8]) -> Result<f64, String> {
    if data.len() != format.width() {
        return Err(format!(
            "a {:?} reading is {} bytes and {} arrived",
            format,
            format.width(),
            data.len()
        ));
    }

    Ok(match format {
        Format::I16Be => i16::from_be_bytes([data[0], data[1]]) as f64,
        Format::U16Be => u16::from_be_bytes([data[0], data[1]]) as f64,
        Format::I16Le => i16::from_le_bytes([data[0], data[1]]) as f64,
        Format::U24Be => {
            u32::from_be_bytes([0, data[0], data[1], data[2]]) as f64
        }
    })
}

/// Counts to kelvin: read, scale, offset.
#[derive(Debug)]
pub struct ToKelvin {
    format: Format,
    scale: f64,
    offset: f64,
}

impl ToKelvin {
    /// The conversion these settled parameters describe.
    ///
    /// Nothing to refuse: what a file may say and what each value may be are
    /// `tcslibgs::node::vehicle::compute`'s, where the ground tool checks them.
    pub fn of(params: ToKelvinParams) -> ToKelvin {
        ToKelvin {
            format: params.format,
            scale: params.scale,
            offset: params.offset,
        }
    }
}

impl Function for ToKelvin {
    fn give(
        &mut self,
        _input: Option<&str>,
        data: &[u8],
    ) -> Result<Option<Out>, String> {
        let counts = read(self.format, data)?;
        // A scale and an offset that put the part's counts outside what a
        // value carries are refused here, naming the temperature: a part
        // read with somebody else's scale is the likeliest way to get one,
        // and it is better said than carried.
        Ok(Some(Out::value(counts * self.scale + self.offset)?))
    }

    fn name(&self) -> &'static str {
        "to_kelvin"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::vehicle::compute::from_value;

    /// A conversion of these parameters.
    ///
    /// What a file may say -- that the format is one of four, that a scale of
    /// nought is refused, that an offset has no default -- is
    /// `tcslibgs::node::vehicle::compute`'s rule, where the ground tool applies
    /// it. These tests are about the arithmetic.
    fn conversion(format: Format, scale: f64, offset: f64) -> ToKelvin {
        ToKelvin::of(ToKelvinParams {
            format,
            scale,
            offset,
        })
    }

    /// A reading becomes the temperature its datasheet says.
    #[test]
    fn counts_become_kelvin() {
        let mut f = conversion(Format::I16Be, 0.0625, 273.15);

        // 320 counts at a sixteenth of a degree each is 20 above freezing.
        let out = f
            .give(None, &[0x01, 0x40])
            .expect("a reading")
            .expect("a value");
        let kelvin = from_value(&out, "the result").unwrap();
        assert!((kelvin - 293.15).abs() < 0.001, "{kelvin}");
    }

    /// A sensor below freezing is a negative count and a positive kelvin.
    ///
    /// Which is why kelvin: a sign error here is a value obviously wrong
    /// rather than plausibly cold.
    #[test]
    fn a_negative_count_is_still_a_positive_temperature() {
        let mut f = conversion(Format::I16Be, 0.0625, 273.15);

        let out = f.give(None, &[0xff, 0xc0]).unwrap().unwrap();
        let kelvin = from_value(&out, "the result").unwrap();
        assert!((kelvin - 269.15).abs() < 0.001, "{kelvin}");
        assert!(kelvin > 0.0);
    }

    /// Each format reads the bytes its own way.
    ///
    /// Scaled by a thousandth rather than read as kelvin: the counts here are
    /// what each layout makes of its bytes, and a count of 65 536 read as
    /// kelvin is outside what a value carries -- which is the right answer to
    /// a part read with no scale, and not what this test is about.
    #[test]
    fn every_format_reads_its_own_layout() {
        for (format, bytes, expected) in [
            (Format::I16Be, vec![0x01, 0x40], 0.320),
            (Format::U16Be, vec![0xff, 0xc0], 65.472),
            (Format::I16Le, vec![0x40, 0x01], 0.320),
            (Format::U24Be, vec![0x01, 0x00, 0x00], 65.536),
        ] {
            let mut f = conversion(format, 0.001, 0.0);
            let out = f.give(None, &bytes).unwrap().unwrap();
            let got = from_value(&out, "the result").unwrap();
            assert!((got - expected).abs() < 0.001, "{format:?}: {got}");
        }
    }

    /// A reading of the wrong length is refused, not truncated.
    ///
    /// Taking the first two bytes of a four-byte answer would turn a bus that
    /// returned something else into a plausible temperature, which is the
    /// worst thing a conversion can do.
    #[test]
    fn a_reading_of_the_wrong_length_is_refused() {
        let mut f = conversion(Format::I16Be, 1.0, 0.0);

        for wrong in [vec![], vec![0x01], vec![0x01, 0x02, 0x03]] {
            let said = f.give(None, &wrong).expect_err("the wrong length");
            assert!(said.contains("2 bytes"), "{said}");
        }
    }

    /// What cannot convert anything is refused where a file is read.
    ///
    /// By the one set of rules, in `tcslibgs::node::vehicle::compute`, which
    /// the ground tool applies too: a format nobody has written, a scale of
    /// nought -- every reading would become the offset, which looks exactly
    /// like a working sensor reporting a steady temperature -- and a missing
    /// offset, which is not nought by default.
    #[test]
    fn what_cannot_be_converted_is_refused_before_it_runs() {
        use tcslibgs::node::vehicle::compute::params_of;

        let refused = |pairs: &[(&str, &str)]| {
            params_of(
                "k_1",
                &pairs
                    .iter()
                    .map(|(k, v)| (k.to_string(), tcslibgs::Scalar::of(*v)))
                    .collect(),
            )
            .expect_err("a conversion that cannot convert")
        };

        for (pairs, wanted) in [
            (
                vec![
                    ("function", "to_kelvin"),
                    ("scale", "1"),
                    ("offset", "0"),
                ],
                "how they are laid out",
            ),
            (
                vec![
                    ("function", "to_kelvin"),
                    ("format", "f32_be"),
                    ("scale", "1"),
                    ("offset", "0"),
                ],
                "i16_be",
            ),
            (
                vec![
                    ("function", "to_kelvin"),
                    ("format", "i16_be"),
                    ("scale", "0"),
                    ("offset", "273.15"),
                ],
                "every reading the same",
            ),
            (
                vec![
                    ("function", "to_kelvin"),
                    ("format", "i16_be"),
                    ("scale", "1"),
                ],
                "offset",
            ),
        ] {
            let said = refused(&pairs);
            assert!(said.contains(wanted), "{pairs:?}: {said}");
        }
    }
}
