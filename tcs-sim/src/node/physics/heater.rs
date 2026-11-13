//! A heater: a resistance on a supply, switched on and off.
//!
//! Its terminals are `tcslib_protocol::node::physics::heater`'s: `on`, the
//! decision it is driven to; `heat`, in watts, what it gives out; `current`, in
//! amperes, what it draws. Both outputs are nought while it is off. `heating`
//! is one while it is on and nought while it is off.
//!
//! # The arithmetic
//!
//! A resistance's power goes as the square of its voltage and its current
//! as the voltage, so with its supply `v` away from nominal as a fraction:
//!
//! ```text
//! heat    = power * (1 + v)^2
//! current = power / voltage * (1 + v)
//! ```
//!
//! `v` wanders, where the model gives it a `ripple`, by its exact
//! transition (see [`super::random::wandered`]), and is held at its value
//! at the start of each interval for that interval: exact for any interval
//! much shorter than its time constant, which is tens of seconds where an
//! interval is a sample's. It wanders while the heater is off, as a supply
//! does.

use rand::rngs::StdRng;
use rand::SeedableRng;
use tcslib_protocol::node::physics::heater::Resolved;

use super::component::Component;
use super::random::{seed_for, settled, wandered};

/// Its terminals' places, in
/// `tcslib_protocol::node::physics::heater::TERMINALS`'s order, which a test
/// holds this to.
const ON: usize = 0;
const HEAT: usize = 1;
const CURRENT: usize = 2;
const HEATING: usize = 3;

/// A heater of the world.
#[derive(Debug)]
pub struct Heater {
    figures: Resolved,
    /// Whether it is driven on.
    on: bool,
    /// Its supply's departure from nominal, as a fraction of it.
    supply: f64,
    rng: StdRng,
}

impl Heater {
    /// Makes a heater, off, with its supply where a settled one would be.
    ///
    /// # Arguments
    ///
    /// * `name`    - Its name, which seeds its draws.
    /// * `figures` - What the model says of it.
    /// * `seed`    - The run's seed.
    ///
    /// # Returns
    ///
    /// The heater.
    pub fn new(name: &str, figures: Resolved, seed: u64) -> Heater {
        let mut rng =
            StdRng::seed_from_u64(seed_for(seed, &format!("component:{name}")));
        let supply = settled(&figures.supply, &mut rng);
        Heater {
            figures,
            on: false,
            supply,
            rng,
        }
    }
}

impl Component for Heater {
    fn hold(&mut self, inputs: &[f64]) {
        self.on = inputs[ON] > 0.5;
    }

    fn advance(&mut self, from: f64, to: f64) {
        self.supply = wandered(
            self.supply,
            &self.figures.supply,
            to - from,
            &mut self.rng,
        );
    }

    fn output(&self, terminal: usize) -> f64 {
        if !self.on {
            return 0.0;
        }
        let v = 1.0 + self.supply;
        match terminal {
            HEATING => 1.0,
            HEAT => self.figures.power * v * v,
            CURRENT => self.figures.power / self.figures.voltage * v,
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslib_protocol::node::physics::heater::TERMINALS;
    use tcslib_protocol::node::physics::Wander;

    /// The places here are the type's.
    #[test]
    fn the_terminals_are_the_types() {
        assert_eq!(TERMINALS[ON].name, "on");
        assert_eq!(TERMINALS[HEAT].name, "heat");
        assert_eq!(TERMINALS[CURRENT].name, "current");
        assert_eq!(TERMINALS[HEATING].name, "heating");
    }

    /// Off it gives and draws nothing and is not heating; on, it gives its
    /// power, draws its power over its voltage, and is heating.
    #[test]
    fn a_heater_gives_its_power_while_on() {
        let figures = Resolved {
            power: 60.0,
            voltage: 28.0,
            supply: Wander::default(),
        };
        let mut heater = Heater::new("h", figures, 0);
        assert_eq!((heater.output(HEAT), heater.output(CURRENT)), (0.0, 0.0));
        assert_eq!(heater.output(HEATING), 0.0);
        heater.hold(&[1.0, 0.0, 0.0]);
        assert_eq!(heater.output(HEAT), 60.0);
        assert!((heater.output(CURRENT) - 60.0 / 28.0).abs() < 1e-12);
        assert_eq!(heater.output(HEATING), 1.0);
        heater.hold(&[0.0, 0.0, 0.0]);
        assert_eq!(heater.output(HEAT), 0.0);
        assert_eq!(heater.output(HEATING), 0.0);
    }

    /// Its supply's ripple warms by its power times (1 + v)^2 on average,
    /// and spreads it by twice the ripple.
    #[test]
    fn its_ripple_goes_into_its_heat_as_a_square() {
        let figures = Resolved {
            power: 3.0,
            voltage: 28.0,
            supply: Wander {
                size: 0.05,
                time: std::time::Duration::from_secs(30),
            },
        };
        let mut heater = Heater::new("h", figures, 3);
        heater.hold(&[1.0, 0.0, 0.0]);
        let (mut sum, mut squares) = (0.0, 0.0);
        let n = 100_000;
        for i in 0..n {
            heater.advance(30.0 * i as f64, 30.0 * (i + 1) as f64);
            let heat = heater.output(HEAT);
            sum += heat;
            squares += heat * heat;
        }
        let mean = sum / n as f64;
        let spread = (squares / n as f64 - mean * mean).sqrt();
        assert!((mean - 3.0 * (1.0 + 0.05 * 0.05)).abs() < 0.005, "{mean}");
        assert!((spread - 3.0 * 2.0 * 0.05).abs() < 0.02, "{spread}");
    }
}
