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

use std::collections::BTreeMap;

use tcslibgs::Scalar;

use super::{as_value, number, Function};

/// How a sensor's bytes are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Two bytes, signed, most significant first.
    I16Be,
    /// Two bytes, unsigned, most significant first.
    U16Be,
    /// Three bytes, unsigned, most significant first.
    U24Be,
    /// Two bytes, signed, least significant first.
    I16Le,
}

impl Format {
    /// The format this name asks for.
    fn of(name: &str) -> Result<Format, String> {
        match name {
            "i16_be" => Ok(Format::I16Be),
            "u16_be" => Ok(Format::U16Be),
            "u24_be" => Ok(Format::U24Be),
            "i16_le" => Ok(Format::I16Le),
            other => Err(format!(
                "format: \"{other}\" is not one this knows. There are i16_be, \
                 u16_be, u24_be and i16_le"
            )),
        }
    }

    /// How many bytes a reading takes.
    fn width(self) -> usize {
        match self {
            Format::U24Be => 3,
            _ => 2,
        }
    }

    /// The number these bytes hold.
    ///
    /// Exactly the width and no more: a reading longer than the format is a
    /// sensor that was read wrongly or a bus that returned something else, and
    /// taking the first two bytes of it would turn that into a plausible
    /// temperature. Shorter is the same mistake the other way.
    fn read(self, data: &[u8]) -> Result<f64, String> {
        if data.len() != self.width() {
            return Err(format!(
                "a {:?} reading is {} bytes and {} arrived",
                self,
                self.width(),
                data.len()
            ));
        }

        Ok(match self {
            Format::I16Be => i16::from_be_bytes([data[0], data[1]]) as f64,
            Format::U16Be => u16::from_be_bytes([data[0], data[1]]) as f64,
            Format::I16Le => i16::from_le_bytes([data[0], data[1]]) as f64,
            Format::U24Be => {
                u32::from_be_bytes([0, data[0], data[1], data[2]]) as f64
            }
        })
    }
}

/// Counts to kelvin: read, scale, offset.
#[derive(Debug)]
pub struct ToKelvin {
    format: Format,
    scale: f64,
    offset: f64,
}

impl ToKelvin {
    pub fn of(attributes: &BTreeMap<String, Scalar>) -> Result<ToKelvin, String> {
        let format = attributes
            .get("format")
            .ok_or_else(|| {
                "format is not set: a sensor's bytes cannot be read without \
                 knowing how they are laid out"
                    .to_string()
            })
            .and_then(|f| Format::of(f.as_str()))?;

        let scale = number(attributes, "scale")?;
        if scale == 0.0 {
            // Every reading would become the offset, which is a sensor whose
            // value never changes -- and that looks like a working sensor
            // reporting a steady temperature.
            return Err("scale: nought would make every reading the same".to_string());
        }

        Ok(ToKelvin {
            format,
            scale,
            offset: number(attributes, "offset")?,
        })
    }
}

impl Function for ToKelvin {
    fn give(&mut self, _input: Option<&str>, data: &[u8]) -> Result<Option<Vec<u8>>, String> {
        let counts = self.format.read(data)?;
        Ok(Some(as_value(counts * self.scale + self.offset)))
    }

    fn name(&self) -> &'static str {
        "to_kelvin"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::compute::from_value;

    fn attributes(pairs: &[(&str, &str)]) -> BTreeMap<String, Scalar> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), Scalar::of(*v)))
            .collect()
    }

    /// A reading becomes the temperature its datasheet says.
    #[test]
    fn counts_become_kelvin() {
        let mut f = ToKelvin::of(&attributes(&[
            ("format", "i16_be"),
            ("scale", "0.0625"),
            ("offset", "273.15"),
        ]))
        .expect("the fore sensor");

        // 320 counts at a sixteenth of a degree each is 20 above freezing.
        let out = f.give(None, &[0x01, 0x40]).expect("a reading").expect("a value");
        let kelvin = from_value(&out, "the result").unwrap();
        assert!((kelvin - 293.15).abs() < 0.001, "{kelvin}");
    }

    /// A sensor below freezing is a negative count and a positive kelvin.
    ///
    /// Which is why kelvin: a sign error here is a value obviously wrong
    /// rather than plausibly cold.
    #[test]
    fn a_negative_count_is_still_a_positive_temperature() {
        let mut f = ToKelvin::of(&attributes(&[
            ("format", "i16_be"),
            ("scale", "0.0625"),
            ("offset", "273.15"),
        ]))
        .unwrap();

        let out = f.give(None, &[0xff, 0xc0]).unwrap().unwrap();
        let kelvin = from_value(&out, "the result").unwrap();
        assert!((kelvin - 269.15).abs() < 0.001, "{kelvin}");
        assert!(kelvin > 0.0);
    }

    /// Each format reads the bytes its own way.
    #[test]
    fn every_format_reads_its_own_layout() {
        for (format, bytes, expected) in [
            ("i16_be", vec![0x01, 0x40], 320.0),
            ("u16_be", vec![0xff, 0xc0], 65472.0),
            ("i16_le", vec![0x40, 0x01], 320.0),
            ("u24_be", vec![0x01, 0x00, 0x00], 65536.0),
        ] {
            let mut f = ToKelvin::of(&attributes(&[
                ("format", format),
                ("scale", "1"),
                ("offset", "0"),
            ]))
            .unwrap();
            let out = f.give(None, &bytes).unwrap().unwrap();
            let got = from_value(&out, "the result").unwrap();
            assert!((got - expected).abs() < 0.001, "{format}: {got}");
        }
    }

    /// A reading of the wrong length is refused, not truncated.
    ///
    /// Taking the first two bytes of a four-byte answer would turn a bus that
    /// returned something else into a plausible temperature, which is the
    /// worst thing a conversion can do.
    #[test]
    fn a_reading_of_the_wrong_length_is_refused() {
        let mut f = ToKelvin::of(&attributes(&[
            ("format", "i16_be"),
            ("scale", "1"),
            ("offset", "0"),
        ]))
        .unwrap();

        for wrong in [vec![], vec![0x01], vec![0x01, 0x02, 0x03]] {
            let said = f.give(None, &wrong).expect_err("the wrong length");
            assert!(said.contains("2 bytes"), "{said}");
        }
    }

    /// A configuration that cannot convert anything is refused at the start.
    #[test]
    fn what_cannot_be_converted_is_refused_before_it_runs() {
        // No format.
        let said = ToKelvin::of(&attributes(&[("scale", "1"), ("offset", "0")]))
            .expect_err("no format");
        assert!(said.contains("how they are laid out"), "{said}");

        // A format nobody has written.
        let said = ToKelvin::of(&attributes(&[
            ("format", "f32_be"),
            ("scale", "1"),
            ("offset", "0"),
        ]))
        .expect_err("an unknown format");
        assert!(said.contains("i16_be"), "it should say what there is: {said}");

        // A scale of nought: every reading would become the offset, which
        // looks exactly like a working sensor reporting a steady temperature.
        let said = ToKelvin::of(&attributes(&[
            ("format", "i16_be"),
            ("scale", "0"),
            ("offset", "273.15"),
        ]))
        .expect_err("a scale of nought");
        assert!(said.contains("every reading the same"), "{said}");

        // And a missing offset, which is not nought by default: a sensor
        // reporting counts above absolute zero is not the same as one
        // reporting counts above freezing.
        let said = ToKelvin::of(&attributes(&[("format", "i16_be"), ("scale", "1")]))
            .expect_err("no offset");
        assert!(said.contains("offset"), "{said}");
    }
}
