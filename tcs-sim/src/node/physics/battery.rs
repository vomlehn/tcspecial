//! A battery: charge moving in a straight line with the current, until it
//! is full or empty.
//!
//! Its terminals are `tcsprotocol::node::physics::battery`'s: `offered`,
//! what the bus has left over, in amperes; `voltage`, in volts; `current`, what
//! actually flows in, in amperes; and `charge`, the fraction of its capacity it
//! holds.
//!
//! # The arithmetic
//!
//! With the current held, charge moves in a straight line, which is exact:
//!
//! ```text
//! charge(t + dt) = charge + current * dt / (3600 * capacity)
//! voltage        = empty + (full - empty) * charge + resistance * current
//! ```
//!
//! A straight line reaches full or empty at a moment it can say in advance,
//! and that moment is its next event: the world stops there, so the battery
//! is never stepped past full into charge it cannot hold. Full, a charging
//! current is shunted and none flows in, so it stays full and foresees
//! nothing until the bus asks it for current again. Empty, it is held
//! there, and what is drawn is still drawn: a cell past what it is rated for
//! is outside what this models, and its voltage, below empty, is what shows
//! it.

use tcsprotocol::node::physics::battery::Resolved;

use super::component::Component;

/// Its terminals' places, in
/// `tcsprotocol::node::physics::battery::TERMINALS`'s order, which a test
/// holds this to.
const OFFERED: usize = 0;
const VOLTAGE: usize = 1;
const CURRENT: usize = 2;
const CHARGE: usize = 3;

/// A battery of the world.
#[derive(Debug)]
pub struct Battery {
    figures: Resolved,
    /// How many coulombs it holds full.
    coulombs: f64,
    /// The fraction of its capacity it holds.
    charge: f64,
    /// What the bus offers it, as last held, in amperes.
    offered: f64,
}

impl Battery {
    /// Makes a battery at the charge its figures say it starts at.
    ///
    /// # Arguments
    ///
    /// * `figures` - What the model says of it.
    ///
    /// # Returns
    ///
    /// The battery, with nothing offered.
    pub fn new(figures: Resolved) -> Battery {
        Battery {
            coulombs: figures.capacity * 3600.0,
            charge: figures.charge,
            offered: 0.0,
            figures,
        }
    }

    /// What flows into it, in amperes: what is offered, but nothing into a
    /// full battery, whose charging current is shunted.
    ///
    /// # Returns
    ///
    /// The current, less than nought where it discharges.
    fn flowing(&self) -> f64 {
        if self.offered > 0.0 && self.charge >= 1.0 {
            0.0
        } else {
            self.offered
        }
    }
}

impl Component for Battery {
    fn hold(&mut self, inputs: &[f64]) {
        self.offered = inputs[OFFERED];
    }

    fn advance(&mut self, from: f64, to: f64) {
        let moved = self.flowing() * (to - from) / self.coulombs;
        self.charge = (self.charge + moved).clamp(0.0, 1.0);
    }

    fn output(&self, terminal: usize) -> f64 {
        let figures = &self.figures;
        match terminal {
            VOLTAGE => {
                figures.empty
                    + (figures.full - figures.empty) * self.charge
                    + figures.resistance * self.flowing()
            }
            CURRENT => self.flowing(),
            CHARGE => self.charge,
            _ => 0.0,
        }
    }

    fn next_event(&self, now: f64) -> Option<f64> {
        let current = self.flowing();
        if current > 0.0 && self.charge < 1.0 {
            Some(now + (1.0 - self.charge) * self.coulombs / current)
        } else if current < 0.0 && self.charge > 0.0 {
            Some(now + self.charge * self.coulombs / -current)
        } else {
            None
        }
    }

    fn event(&mut self, _at: f64) {
        // At full or at empty, give or take the last bit of arithmetic: it
        // is made exactly so, which is what stops a charging current from
        // flowing into it again.
        self.charge = if self.charge > 0.5 { 1.0 } else { 0.0 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcsprotocol::node::physics::battery::TERMINALS;

    /// A battery of one ampere-hour from 24 V to 32 V, a tenth of an ohm,
    /// half full.
    ///
    /// # Returns
    ///
    /// Its figures.
    fn half() -> Resolved {
        Resolved {
            capacity: 1.0,
            empty: 24.0,
            full: 32.0,
            resistance: 0.1,
            charge: 0.5,
        }
    }

    /// The places here are the type's.
    #[test]
    fn the_terminals_are_the_types() {
        assert_eq!(TERMINALS[OFFERED].name, "offered");
        assert_eq!(TERMINALS[VOLTAGE].name, "voltage");
        assert_eq!(TERMINALS[CURRENT].name, "current");
        assert_eq!(TERMINALS[CHARGE].name, "charge");
    }

    /// Two amperes into half an ampere-hour of room is fifteen minutes to
    /// full, foreseen; there it stops, and none flows in.
    #[test]
    fn it_charges_to_full_and_stops_there() {
        let mut battery = Battery::new(half());
        battery.hold(&[2.0, 0.0, 0.0, 0.0]);
        assert!((battery.output(VOLTAGE) - (28.0 + 0.2)).abs() < 1e-12);
        let full = battery.next_event(0.0).expect("it foresees full");
        assert!((full - 900.0).abs() < 1e-9, "{full}");

        battery.advance(0.0, full);
        battery.event(full);
        assert_eq!(battery.output(CHARGE), 1.0);
        assert_eq!(battery.output(CURRENT), 0.0, "shunted");
        assert_eq!(battery.output(VOLTAGE), 32.0);
        assert_eq!(battery.next_event(full), None);
    }

    /// Discharging, its charge falls in a straight line, its voltage with
    /// it less what its resistance drops, and it foresees empty.
    #[test]
    fn it_discharges_towards_empty() {
        let mut battery = Battery::new(half());
        battery.hold(&[-4.0, 0.0, 0.0, 0.0]);
        battery.advance(0.0, 225.0);
        assert!((battery.output(CHARGE) - 0.25).abs() < 1e-12);
        let volts = battery.output(VOLTAGE);
        assert!((volts - (24.0 + 2.0 - 0.4)).abs() < 1e-12, "{volts}");
        let empty = battery.next_event(225.0).expect("it foresees empty");
        assert!((empty - 450.0).abs() < 1e-9, "{empty}");
    }
}
