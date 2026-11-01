//! A thermal zone: one body at one temperature, which heat warms and its
//! surroundings pull back.
//!
//! Its terminals are `tcslibgs::physics::thermal_zone`'s: `heat`, in watts,
//! the sum of what every heater joined to it gives; `temperature`, in
//! kelvin, what it is.
//!
//! # The arithmetic
//!
//! Newton's cooling with a steady heat:
//!
//! ```text
//! dT/dt = -(T - environment) / tau + heat / capacity
//! ```
//!
//! The world holds `heat` steady between one moment and the next, and over
//! such an interval the equation has a closed form:
//!
//! ```text
//! T(t + dt) = rest + (T - rest) * exp(-dt / tau)
//! rest      = environment + heat / capacity * tau
//! ```
//!
//! which is what this advances by. Exact rather than integrated: the world
//! is advanced whenever a stand-in asks or a heater is switched, so the
//! intervals are whatever length the traffic makes them, and a method
//! whose error grew with the interval would make the simulated temperature
//! depend on how often somebody looked at it.
//!
//! `rest` is where the zone settles with that heat on for ever, which is
//! also the figure that says whether its heaters can win at all: with a
//! fifty second constant, twenty joules a kelvin and a hundred and twenty
//! watts, it is three hundred kelvin above the environment.
//!
//! # Starting
//!
//! At the environment. A zone that started at its setpoint would hide the
//! one part of a loop that is interesting to watch -- the first climb, with
//! everything on -- and a vehicle coming up cold is the case a thermostat
//! exists for.
//!
//! # What varies
//!
//! Two things a real zone sees, each optional and each off unless the
//! model asks for it:
//!
//! * The environment swings as a sine wave, `swing` either side of its
//!   middle over `period`: a day and a night, or an orbit's sun and shadow.
//!   That is still a linear equation, and its closed form adds the swing's
//!   steady response,
//!
//!   ```text
//!   swung(t) = swing * (sin(w t) - w tau cos(w t)) / (1 + (w tau)^2)
//!   T(t + dt) = rest + swung(t + dt)
//!               + (T - rest - swung(t)) * exp(-dt / tau)
//!   ```
//!
//!   with `w` the swing's angular frequency, so the swing is as exact as
//!   the rest.
//! * The environment wanders at random (see [`super::random::wandered`]),
//!   held at its value at the start of each interval for that interval:
//!   exact for any interval much shorter than its time constant, which is
//!   minutes where an interval is a sample's.

use std::f64::consts::TAU;

use rand::rngs::StdRng;
use rand::SeedableRng;
use tcslibgs::physics::thermal_zone::Resolved;

use super::component::Component;
use super::random::{seed_for, settled, wandered};

/// Its terminals' places, in `tcslibgs::physics::thermal_zone::TERMINALS`'s
/// order, which a test holds this to.
const HEAT: usize = 0;
const TEMPERATURE: usize = 1;

/// A thermal zone of the world.
#[derive(Debug)]
pub struct ThermalZone {
    figures: Resolved,
    /// Its time constant, in seconds.
    tau: f64,
    /// The swing's angular frequency, in radians a second; nought for none.
    omega: f64,
    /// Kelvin.
    temperature: f64,
    /// What warms it, in watts, held since it was last given its inputs.
    heat: f64,
    /// How far the environment has wandered from where it swings, in
    /// kelvin.
    wander: f64,
    rng: StdRng,
}

impl ThermalZone {
    /// Makes a zone at its environment, with nothing warming it.
    ///
    /// # Arguments
    ///
    /// * `name`    - Its name, which seeds its draws.
    /// * `figures` - What the model says of it.
    /// * `seed`    - The run's seed.
    ///
    /// # Returns
    ///
    /// The zone, at its environment as it is at simulated time nought:
    /// wandered as far as a first draw says, and at the middle of its
    /// swing.
    pub fn new(name: &str, figures: Resolved, seed: u64) -> ThermalZone {
        let mut rng =
            StdRng::seed_from_u64(seed_for(seed, &format!("component:{name}")));
        let wander = settled(&figures.wander, &mut rng);
        let omega = if figures.swing > 0.0 {
            TAU / figures.period.as_secs_f64()
        } else {
            0.0
        };
        ThermalZone {
            tau: figures.time_constant.as_secs_f64(),
            omega,
            temperature: figures.environment + wander,
            heat: 0.0,
            wander,
            rng,
            figures,
        }
    }

    /// The swing's steady response at a time: what the sine wave of the
    /// environment adds to the zone once its start has died away.
    ///
    /// # Arguments
    ///
    /// * `t` - Simulated seconds.
    ///
    /// # Returns
    ///
    /// Kelvin; nought for a zone whose environment does not swing.
    fn swung(&self, t: f64) -> f64 {
        if self.figures.swing <= 0.0 {
            return 0.0;
        }
        let wt = self.omega * self.tau;
        let phase = self.omega * t;
        self.figures.swing * (phase.sin() - wt * phase.cos()) / (1.0 + wt * wt)
    }
}

impl Component for ThermalZone {
    fn hold(&mut self, inputs: &[f64]) {
        self.heat = inputs[HEAT];
    }

    fn advance(&mut self, from: f64, to: f64) {
        let seconds = to - from;
        if seconds <= 0.0 {
            return;
        }
        let rest = self.figures.environment
            + self.wander
            + self.heat / self.figures.capacity * self.tau;
        let decay = (-seconds / self.tau).exp();
        self.temperature = rest
            + self.swung(to)
            + (self.temperature - rest - self.swung(from)) * decay;
        // Then what was held for the interval moves on to where it is now.
        self.wander =
            wandered(self.wander, &self.figures.wander, seconds, &mut self.rng);
    }

    fn output(&self, terminal: usize) -> f64 {
        match terminal {
            TEMPERATURE => self.temperature,
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tcslibgs::physics::thermal_zone::TERMINALS;
    use tcslibgs::physics::Wander;

    /// The example's zone: ten kelvin below a setpoint of 293.15, losing
    /// the difference with a fifty second constant, twenty joules a kelvin.
    ///
    /// # Returns
    ///
    /// Its figures.
    fn zone() -> Resolved {
        Resolved {
            environment: 283.15,
            time_constant: Duration::from_secs(50),
            capacity: 20.0,
            ..Default::default()
        }
    }

    /// The places here are the type's.
    #[test]
    fn the_terminals_are_the_types() {
        assert_eq!(TERMINALS[HEAT].name, "heat");
        assert_eq!(TERMINALS[TEMPERATURE].name, "temperature");
    }

    /// A zone with no heat on it settles at its environment, from either
    /// side.
    #[test]
    fn a_zone_settles_at_its_environment() {
        let mut zone = ThermalZone::new("zone", zone(), 0);

        // It starts there, which is what a vehicle coming up cold is.
        assert!((zone.output(TEMPERATURE) - 283.15).abs() < 1e-9);

        // Warmer, and left alone: it falls back, and by the time constant
        // it has lost about two thirds of the difference.
        zone.temperature = 293.15;
        zone.advance(0.0, 50.0);
        let left = zone.output(TEMPERATURE) - 283.15;
        assert!(
            (left - 10.0 * (-1.0f64).exp()).abs() < 1e-9,
            "a time constant should leave 1/e of the ten kelvin: {left}"
        );

        // And a long time later it is there.
        zone.advance(50.0, 10_000.0);
        assert!((zone.output(TEMPERATURE) - 283.15).abs() < 1e-6);
    }

    /// The rate at the setpoint is the figure the example is written from.
    ///
    /// Ten kelvin above the environment with a fifty second constant is a
    /// fifth of a kelvin a second, which is what the thermostat's cooling
    /// leg is specified as. Checked here because the model states the
    /// constant and a reader is owed the arithmetic that turns it into the
    /// rate.
    #[test]
    fn the_cooling_rate_at_the_setpoint_is_a_fifth_of_a_kelvin() {
        let mut zone = ThermalZone::new("zone", zone(), 0);
        zone.temperature = 293.15;
        // Over a tenth of a second, which is the example's sample interval.
        zone.advance(0.0, 0.1);
        let fell = 293.15 - zone.output(TEMPERATURE);
        assert!(
            (fell - 0.02).abs() < 1e-4,
            "a fifth of a kelvin a second is 0.02 K in 100 ms: {fell}"
        );
    }

    /// Its heat, over its capacity, decides where it settles: a hundred
    /// and twenty watts on twenty joules a kelvin with a fifty second
    /// constant rests three hundred kelvin above the environment.
    #[test]
    fn its_heat_decides_where_it_settles() {
        let mut zone = ThermalZone::new("zone", zone(), 0);
        zone.hold(&[120.0, 0.0]);
        zone.advance(0.0, 10_000.0);
        let settled = zone.output(TEMPERATURE);
        assert!((settled - (283.15 + 300.0)).abs() < 1e-6, "{settled}");
    }

    /// The swing's closed form agrees with integrating the equation
    /// numerically.
    #[test]
    fn a_swinging_environment_is_advanced_exactly() {
        let swing = Resolved {
            swing: 5.0,
            period: Duration::from_secs(100),
            ..zone()
        };
        let mut zone = ThermalZone::new("zone", swing, 0);
        // Two watts on twenty joules a kelvin: a tenth of a kelvin a second.
        zone.hold(&[2.0, 0.0]);
        zone.temperature = 290.0;
        let seconds = 300.0;
        zone.advance(0.0, seconds);
        let exact = zone.output(TEMPERATURE);

        // Fourth-order Runge-Kutta over the same 300 s, a hundredth of a
        // second at a time.
        let tau = 50.0;
        let w = TAU / 100.0;
        let slope =
            |t: f64, x: f64| -(x - (283.15 + 5.0 * (w * t).sin())) / tau + 0.1;
        let (mut t, mut x, h) = (0.0, 290.0, 0.01);
        while t < seconds - 1e-9 {
            let k1 = slope(t, x);
            let k2 = slope(t + h / 2.0, x + h / 2.0 * k1);
            let k3 = slope(t + h / 2.0, x + h / 2.0 * k2);
            let k4 = slope(t + h, x + h * k3);
            x += h / 6.0 * (k1 + 2.0 * k2 + 2.0 * k3 + k4);
            t += h;
        }
        assert!((exact - x).abs() < 1e-6, "{exact} against {x}");
    }

    /// The environment's wander settles at its size.
    #[test]
    fn its_wander_settles_at_its_size() {
        let wandering = Resolved {
            wander: Wander {
                size: 0.5,
                time: Duration::from_secs(30),
            },
            ..zone()
        };
        let mut zone = ThermalZone::new("zone", wandering, 3);
        let mut squares = 0.0;
        let n = 100_000;
        for i in 0..n {
            zone.advance(30.0 * i as f64, 30.0 * (i + 1) as f64);
            squares += zone.wander * zone.wander;
        }
        let size = (squares / n as f64).sqrt();
        assert!((size - 0.5).abs() < 0.02, "{size}");
    }

    /// The same seed wanders the same way, and another seed does not.
    #[test]
    fn a_seed_repeats_a_zone() {
        let wandering = Resolved {
            wander: Wander {
                size: 0.5,
                time: Duration::from_secs(30),
            },
            ..zone()
        };
        let run = |seed| {
            let mut zone = ThermalZone::new("zone", wandering, seed);
            (1..20)
                .map(|i| {
                    zone.advance((i - 1) as f64, i as f64);
                    zone.output(TEMPERATURE)
                })
                .collect::<Vec<f64>>()
        };
        assert_eq!(run(1), run(1));
        assert_ne!(run(1), run(2));
    }

    /// An interval of no time changes nothing, which is what a sensor read
    /// twice in one instant must not do.
    #[test]
    fn an_interval_of_no_time_changes_nothing() {
        let mut zone = ThermalZone::new("zone", zone(), 0);
        zone.hold(&[60.0, 0.0]);
        zone.advance(0.0, 1.0);
        let first = zone.output(TEMPERATURE);
        zone.advance(1.0, 1.0);
        assert_eq!(zone.output(TEMPERATURE), first);
    }
}
