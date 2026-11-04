//! How a physics setting is read: a number with its unit, a length of time,
//! or a quantity that wanders.
//!
//! A physical figure is written with its unit -- `"283.15 K"`, `"60 W"`,
//! `"20 J/K"` -- and one written without it is refused. A bare `60` could be
//! watts or kelvin a second, and the two differ by the zone's heat capacity:
//! a file that says which is a file that cannot be read the wrong way. A
//! fraction, which has no unit, is written bare.
//!
//! The units a setting may be in are wider than those a value on a link may
//! be ([`crate::unit::Unit`]): a heat capacity is a figure a model needs and
//! no node ever sends. So a setting names its unit by symbol, and the symbol
//! is checked as written.
//!
//! Used by the physics model's components and by the simulator file's
//! settings of a stand-in attached to one, whose errors are in the unit of
//! what it is attached to: one reading of a figure for both files.

use std::time::Duration;

/// A quantity that wanders at random and is drawn back towards nought: an
/// Ornstein-Uhlenbeck process, which is what a slow drift or a fluctuation
/// with a memory is.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Wander {
    /// Its standard deviation, once it has settled; nought is no wander.
    pub size: f64,
    /// How long it remembers where it was: its time constant. Zero only
    /// where `size` is.
    pub time: Duration,
}

/// Reads a number written with its unit.
///
/// # Arguments
///
/// * `what`   - The setting's name, for the complaint.
/// * `said`   - The setting as the file states it: `"283.15 K"`, or for a
///              unit of `None`, a bare number.
/// * `symbol` - The unit it must be in, as a file writes it, or `None` for
///              a fraction, which has none.
///
/// # Returns
///
/// The number, in that unit.
///
/// # Errors
///
/// What is wrong, naming the unit wanted: a number with no unit where one
/// is wanted, one with another unit, one with a unit where none is, or
/// something that is not a number.
pub fn quantity(
    what: &str,
    said: &str,
    symbol: Option<&str>,
) -> Result<f64, String> {
    let said = said.trim();
    // The number is the text up to the first character that cannot be in
    // one, and the unit is what follows it.
    let end = said
        .find(|c: char| {
            !(c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | 'e' | 'E'))
        })
        .unwrap_or(said.len());
    let (number, unit) = (said[..end].trim(), said[end..].trim());
    let value: f64 = number
        .parse()
        .ok()
        .filter(|v: &f64| v.is_finite())
        .ok_or_else(|| match symbol {
            Some(symbol) => format!(
                "{what}: \"{said}\" is not a number of {symbol}: write it \
                 as a number and its unit, such as \"1 {symbol}\""
            ),
            None => format!("{what}: \"{said}\" is not a number"),
        })?;
    match (symbol, unit) {
        (Some(symbol), unit) if unit == symbol => Ok(value),
        (Some(symbol), "") => Err(format!(
            "{what}: \"{said}\" has no unit: write \"{number} {symbol}\", \
             so that nobody has to guess what it is in"
        )),
        (Some(symbol), unit) => Err(format!(
            "{what}: \"{said}\" is in {unit}, and {what} is in {symbol}"
        )),
        (None, "") => Ok(value),
        (None, unit) => Err(format!(
            "{what}: \"{said}\" has a unit, {unit}, and {what} is a fraction, \
             which has none: write the number alone"
        )),
    }
}

/// Reads a quantity that may not be negative, unstated being nought.
///
/// # Arguments
///
/// * `what`   - The setting's name, for the complaint.
/// * `said`   - The setting as the file states it, if it does.
/// * `symbol` - Its unit, or `None` for a fraction: see [`quantity`].
///
/// # Returns
///
/// Its value; nought where it is unstated.
///
/// # Errors
///
/// What [`quantity`] says, or that it is negative.
pub fn size_of(
    what: &str,
    said: &Option<String>,
    symbol: Option<&str>,
) -> Result<f64, String> {
    let Some(said) = said else {
        return Ok(0.0);
    };
    let value = quantity(what, said, symbol)?;
    if value < 0.0 {
        return Err(format!(
            "{what}: {value} is less than nought, and a size is not"
        ));
    }
    Ok(value)
}

/// Reads a quantity that must be more than nought.
///
/// # Arguments
///
/// * `what`   - The setting's name, for the complaint.
/// * `said`   - The setting as the file states it.
/// * `symbol` - Its unit: see [`quantity`].
///
/// # Returns
///
/// Its value.
///
/// # Errors
///
/// What [`quantity`] says, or that it is not more than nought.
pub fn positive(
    what: &str,
    said: &str,
    symbol: Option<&str>,
) -> Result<f64, String> {
    let value = quantity(what, said, symbol)?;
    if value <= 0.0 {
        return Err(format!(
            "{what}: {value} is not more than nought, and a {what} has to be"
        ));
    }
    Ok(value)
}

/// Reads a length of time, which `parse_interval` makes more than nought.
///
/// # Arguments
///
/// * `what` - The setting's name, for the complaint.
/// * `said` - The setting as the file states it.
///
/// # Returns
///
/// The length of time.
///
/// # Errors
///
/// What is wrong, if it is not a length of time or is none.
pub fn time_of(what: &str, said: &str) -> Result<Duration, String> {
    crate::graph::parse_interval(said.trim())
        .map_err(|why| format!("{what}: {why}"))
}

/// Reads a wander: its size and the time constant that goes with it.
///
/// # Arguments
///
/// * `size_name` - The size setting's name, such as `drift`.
/// * `time_name` - Its time constant's, such as `drift_time`.
/// * `size`      - The size as the file states it, if it does.
/// * `time`      - The time constant, if the file states it.
/// * `symbol`    - The size's unit, or `None` for a fraction.
///
/// # Returns
///
/// The wander; none where neither is stated or the size is nought.
///
/// # Errors
///
/// What is wrong, if either does not read, or a size of more than nought
/// has no time constant, or a time constant has no size.
pub fn wander_of(
    size_name: &str,
    time_name: &str,
    size: &Option<String>,
    time: &Option<String>,
    symbol: Option<&str>,
) -> Result<Wander, String> {
    let amount = size_of(size_name, size, symbol)?;
    match time {
        None if amount > 0.0 => Err(format!(
            "{time_name} is not set: {size_name} says how far something \
             wanders, and {time_name} how slowly"
        )),
        None => Ok(Wander::default()),
        Some(_) if size.is_none() => Err(format!(
            "{time_name} says how slowly {size_name} wanders, and \
             {size_name} is not set"
        )),
        Some(said) => Ok(Wander {
            size: amount,
            time: time_of(time_name, said)?,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A figure reads with its unit, spaced or not, and signed.
    #[test]
    fn a_figure_reads_with_its_unit() {
        assert_eq!(quantity("e", "283.15 K", Some("K")), Ok(283.15));
        assert_eq!(quantity("e", "60W", Some("W")), Ok(60.0));
        assert_eq!(quantity("e", "+0.2 K", Some("K")), Ok(0.2));
        assert_eq!(quantity("e", "-0.3 K", Some("K")), Ok(-0.3));
        assert_eq!(quantity("e", "20 J/K", Some("J/K")), Ok(20.0));
        assert_eq!(quantity("e", "1e3 W", Some("W")), Ok(1000.0));
        assert_eq!(quantity("r", "0.02", None), Ok(0.02));
    }

    /// A figure with no unit, the wrong one, or one where none belongs, is
    /// refused, saying what to write.
    #[test]
    fn a_figure_in_the_wrong_unit_is_refused() {
        for (said, symbol, says) in [
            ("283.15", Some("K"), "write \"283.15 K\""),
            ("60 K", Some("W"), "is in K, and power is in W"),
            ("0.02 K", None, "which has none"),
            ("warm K", Some("K"), "not a number of K"),
            ("inf K", Some("K"), "not a number"),
        ] {
            let why = quantity("power", said, symbol).expect_err(said);
            assert!(why.contains(says), "{said}: {why}");
        }
    }

    /// A size is not negative, and a positive figure is more than nought.
    #[test]
    fn sizes_and_positives_are_held_to_their_signs() {
        assert_eq!(size_of("n", &None, Some("K")), Ok(0.0));
        assert!(size_of("n", &Some("-1 K".into()), Some("K")).is_err());
        assert!(positive("power", "0 W", Some("W")).is_err());
        assert_eq!(positive("power", "3 W", Some("W")), Ok(3.0));
    }

    /// A wander needs both its size and its time, or neither.
    #[test]
    fn a_wander_needs_its_size_and_its_time() {
        let none = wander_of("w", "wt", &None, &None, Some("K"));
        assert_eq!(none, Ok(Wander::default()));
        let both = wander_of(
            "w",
            "wt",
            &Some("0.5 K".into()),
            &Some("10 s".into()),
            Some("K"),
        );
        assert_eq!(
            both,
            Ok(Wander {
                size: 0.5,
                time: Duration::from_secs(10)
            })
        );
        assert!(
            wander_of("w", "wt", &Some("0.5 K".into()), &None, Some("K"))
                .unwrap_err()
                .contains("wt is not set")
        );
        assert!(wander_of("w", "wt", &None, &Some("1 s".into()), Some("K"))
            .unwrap_err()
            .contains("w is not set"));
    }
}
