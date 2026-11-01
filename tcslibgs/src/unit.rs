//! The units a value may be in, and how finely each is carried.
//!
//! A value on a link is four bytes of a signed count, most significant first:
//! see `crate::node::vehicle::compute`. What the count counts is the unit's
//! step, which is chosen per unit to be finer than anything a part measures
//! while leaving the range a vehicle needs: a micro-kelvin reaches 2147 K
//! either way, which is above anything these nodes are pointed at, where a
//! millimetre is needed for an altitude, a micro-metre reaching only 2 km.
//!
//! The unit is never on the link. The configuration says what each node
//! produces -- a conversion states its unit, and a vote's is its inputs' --
//! so both ends know it without a byte being spent on it, as they know which
//! nodes send values at all. A kelvin is counted in micro-kelvin as it was
//! before there were other units, so a link, a log or a configuration from
//! then reads the same.
//!
//! This is the one table of them. Adding a unit is a line here.

use std::fmt;

/// A unit a value may be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Unit {
    /// Temperature.
    Kelvin,
    /// Potential: a bus or a cell.
    Volt,
    /// Current: a load, a source, a bus.
    Ampere,
    /// Power.
    Watt,
    /// Length: an altitude, a distance.
    Metre,
    /// Speed.
    MetrePerSecond,
    /// Force: a thrust.
    Newton,
    /// A number with no unit: a state of charge, a throttle setting, as a
    /// fraction of the whole.
    Ratio,
}

/// One row of the table: a unit, how a file writes it, what it is called in
/// a sentence, how many of its steps are in one of it, and what a step is
/// called.
struct Row {
    unit: Unit,
    symbol: &'static str,
    name: &'static str,
    per_unit: f64,
    step: &'static str,
}

/// The table. A step of a millionth for most, so that a reading is never
/// rounded by being carried; of a thousandth where a millionth would leave
/// too little range -- an altitude in micro-metres reaches only 2 km, a
/// thrust in micro-newtons only 2 kN.
const TABLE: [Row; 8] = [
    Row {
        unit: Unit::Kelvin,
        symbol: "K",
        name: "kelvin",
        per_unit: 1e6,
        step: "micro-kelvin",
    },
    Row {
        unit: Unit::Volt,
        symbol: "V",
        name: "volts",
        per_unit: 1e6,
        step: "micro-volts",
    },
    Row {
        unit: Unit::Ampere,
        symbol: "A",
        name: "amperes",
        per_unit: 1e6,
        step: "micro-amperes",
    },
    Row {
        unit: Unit::Watt,
        symbol: "W",
        name: "watts",
        per_unit: 1e3,
        step: "milliwatts",
    },
    Row {
        unit: Unit::Metre,
        symbol: "m",
        name: "metres",
        per_unit: 1e3,
        step: "millimetres",
    },
    Row {
        unit: Unit::MetrePerSecond,
        symbol: "m/s",
        name: "metres a second",
        per_unit: 1e6,
        step: "micro-metres a second",
    },
    Row {
        unit: Unit::Newton,
        symbol: "N",
        name: "newtons",
        per_unit: 1e3,
        step: "millinewtons",
    },
    Row {
        unit: Unit::Ratio,
        symbol: "ratio",
        name: "a ratio",
        per_unit: 1e6,
        step: "millionths",
    },
];

impl Unit {
    /// Every unit, in the table's order.
    pub const ALL: [Unit; 8] = [
        Unit::Kelvin,
        Unit::Volt,
        Unit::Ampere,
        Unit::Watt,
        Unit::Metre,
        Unit::MetrePerSecond,
        Unit::Newton,
        Unit::Ratio,
    ];

    /// Finds this unit's row of the table.
    fn row(self) -> &'static Row {
        TABLE
            .iter()
            .find(|row| row.unit == self)
            .expect("every unit has a row: see the test")
    }

    /// Says how a file writes this unit: `K`, `V`, `m/s`.
    pub fn symbol(self) -> &'static str {
        self.row().symbol
    }

    /// Says what this unit is called in a sentence: `kelvin`, `volts`.
    pub fn name(self) -> &'static str {
        self.row().name
    }

    /// Says how many of the counts a value carries are in one of this unit.
    pub fn per_unit(self) -> f64 {
        self.row().per_unit
    }

    /// Says what one count of this unit is called: `micro-kelvin`.
    pub fn step(self) -> &'static str {
        self.row().step
    }

    /// Gives the least and the most a value in this unit can say: the ends
    /// of a signed 32-bit count of its steps.
    ///
    /// # Returns
    ///
    /// The least, then the most.
    pub fn range(self) -> (f64, f64) {
        (
            f64::from(i32::MIN) / self.per_unit(),
            f64::from(i32::MAX) / self.per_unit(),
        )
    }

    /// Reads the unit a file writes.
    ///
    /// # Arguments
    ///
    /// * `said` - The unit as the file wrote it: `K`, `V`, `m/s`.
    ///
    /// # Returns
    ///
    /// The unit.
    ///
    /// # Errors
    ///
    /// A message listing every unit there is, for one that is none of them.
    pub fn of(said: &str) -> Result<Unit, String> {
        let said = said.trim();
        TABLE
            .iter()
            .find(|row| row.symbol == said)
            .map(|row| row.unit)
            .ok_or_else(|| {
                let every: Vec<&str> =
                    TABLE.iter().map(|row| row.symbol).collect();
                format!(
                    "\"{said}\" is not a unit this knows. There are {}",
                    every.join(", ")
                )
            })
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.symbol())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every unit has one row, its symbol reads back as it, and no two
    /// share a symbol.
    #[test]
    fn every_unit_has_one_row_and_reads_back() {
        for unit in Unit::ALL {
            assert_eq!(
                TABLE.iter().filter(|row| row.unit == unit).count(),
                1,
                "{unit:?}"
            );
            assert_eq!(Unit::of(unit.symbol()), Ok(unit));
        }
        assert_eq!(TABLE.len(), Unit::ALL.len());
    }

    /// A kelvin is counted in micro-kelvin, as every value was before there
    /// were other units, so what was written then reads the same now.
    #[test]
    fn a_kelvin_is_still_a_million_counts() {
        assert_eq!(Unit::Kelvin.per_unit(), 1e6);
        let (_, hottest) = Unit::Kelvin.range();
        assert!((hottest - 2147.483647).abs() < 1e-9, "{hottest}");
    }

    /// Each range covers what the examples need: a descent from 15 km, a
    /// bus of 28 V, an engine of 4 kN.
    #[test]
    fn each_range_covers_what_a_vehicle_needs() {
        assert!(Unit::Metre.range().1 > 15_000.0);
        assert!(Unit::Volt.range().1 > 28.0);
        assert!(Unit::Newton.range().1 > 4_000.0);
        assert!(Unit::MetrePerSecond.range().1 > 1_700.0);
    }

    /// A unit that is not one is refused, naming every one there is.
    #[test]
    fn a_unit_that_is_not_one_is_refused() {
        let why = Unit::of("furlong").expect_err("not a unit");
        assert!(why.contains("furlong"), "{why}");
        assert!(why.contains("m/s") && why.contains("ratio"), "{why}");
    }
}
