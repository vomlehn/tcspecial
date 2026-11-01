//! A sensor on a terminal of the physics model, and what it gets wrong.
//!
//! A real sensor does not report what the body it is on is. A temperature
//! sensor reports its own temperature, which trails the body's; it is off by a calibration
//! error that stays put; that error drifts, slowly; and its electronics add
//! noise to every reading. Each is a datasheet figure, stated in the
//! simulator file in the unit of what the sensor reports, and off unless it
//! is (see `tcslibgs::sim_config::SensorErrors`):
//!
//! * **Lag.** The sensor is a small mass of its own, warmed by the body
//!   through its mounting, so it follows the body's temperature through a
//!   first-order lag with its own time constant. Between two readings the
//!   body's temperature is taken to change at a steady rate, for which the
//!   lag has a closed form:
//!
//!   ```text
//!   m        = (body(t1) - body(t0)) / dt
//!   own(t1)  = body(t1) - lag m + (own(t0) - body(t0) + lag m) exp(-dt / lag)
//!   ```
//!
//! * **Calibration.** A fixed error, drawn once a run from a normal
//!   distribution with a third of the stated accuracy as its standard
//!   deviation, and never past the accuracy: a datasheet's accuracy is a
//!   bound most parts sit well inside.
//! * **Drift.** An error that wanders slowly, drawn back towards nought:
//!   an Ornstein-Uhlenbeck process, stepped by its exact transition (see
//!   [`super::random::wandered`]).
//! * **Noise.** A normal draw on each reading, independent of the last.
//!
//! The sensor's random draws come from a generator of its own, seeded from
//! the run's seed and the sensor's name.

use std::sync::Mutex;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rand_distr::StandardNormal;
use tcslibgs::sim_config::SensorErrors;

use super::random::{seed_for, wandered};

/// What a sensor has read so far.
#[derive(Debug)]
struct State {
    /// Where its drift has got to.
    drift: f64,
    /// When it last read, in simulated seconds, what the body was then
    /// and what it was itself.
    last: Option<(f64, f64, f64)>,
    /// Where its random draws come from.
    rng: StdRng,
}

/// One sensor on a terminal of the physics model.
///
/// Shared by the copies of its payload's configuration, and read from its
/// payload's thread, so what it has read is behind a lock.
#[derive(Debug)]
pub struct Sensor {
    /// What it gets wrong, as the simulator file states it.
    errors: SensorErrors,
    /// Its calibration error this run.
    bias: f64,
    state: Mutex<State>,
}

impl Sensor {
    /// Makes a sensor, drawing its calibration error for the run.
    ///
    /// # Arguments
    ///
    /// * `name`   - Its payload's name, which seeds its draws.
    /// * `errors` - What it gets wrong.
    /// * `seed`   - The run's seed.
    ///
    /// # Returns
    ///
    /// The sensor, with its drift where a settled one would be.
    pub fn new(name: &str, errors: SensorErrors, seed: u64) -> Sensor {
        let mut rng =
            StdRng::seed_from_u64(seed_for(seed, &format!("sensor:{name}")));
        let mut draw = || rng.sample::<f64, _>(StandardNormal);
        let bias = (errors.accuracy / 3.0 * draw())
            .clamp(-errors.accuracy, errors.accuracy);
        let drift = errors.drift.size * draw();
        Sensor {
            errors,
            bias,
            state: Mutex::new(State {
                drift,
                last: None,
                rng,
            }),
        }
    }

    /// Its calibration error this run.
    #[cfg(test)]
    pub fn bias(&self) -> f64 {
        self.bias
    }

    /// Reads the body it is on.
    ///
    /// # Arguments
    ///
    /// * `body` - What the place it is on is, in its terminal's unit.
    /// * `now`  - The time of the reading, in simulated seconds.
    ///
    /// # Returns
    ///
    /// What it reports, in its terminal's unit: its own lagging value, with
    /// its calibration error, its drift and this reading's noise.
    pub fn read(&self, body: f64, now: f64) -> f64 {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let lag = self.errors.lag.as_secs_f64();
        let (own, seconds) = match state.last {
            // The first reading finds the sensor settled on the body.
            None => (body, 0.0),
            Some((then, body_then, own_then)) => {
                let seconds = (now - then).max(0.0);
                let own = if lag <= 0.0 || seconds <= 0.0 {
                    if lag <= 0.0 {
                        body
                    } else {
                        own_then
                    }
                } else {
                    let m = (body - body_then) / seconds;
                    body - lag * m
                        + (own_then - body_then + lag * m)
                            * (-seconds / lag).exp()
                };
                (own, seconds)
            }
        };
        state.last = Some((now, body, own));

        let State { drift, rng, .. } = &mut *state;
        *drift = wandered(*drift, &self.errors.drift, seconds, rng);
        let noise = if self.errors.noise > 0.0 {
            self.errors.noise * rng.sample::<f64, _>(StandardNormal)
        } else {
            0.0
        };
        own + self.bias + *drift + noise
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tcslibgs::physics::Wander;

    /// A sensor with no errors reports the body exactly.
    #[test]
    fn a_perfect_sensor_reads_the_body() {
        let sensor = Sensor::new("t", SensorErrors::default(), 1);
        assert_eq!(sensor.read(290.0, 0.0), 290.0);
        assert_eq!(sensor.read(291.5, 1.0), 291.5);
    }

    /// Its calibration error stays within the accuracy, stays put, and
    /// differs between sensors and between seeds.
    #[test]
    fn calibration_stays_within_the_accuracy() {
        let errors = SensorErrors {
            accuracy: 0.5,
            ..SensorErrors::default()
        };
        let biases: Vec<f64> = (0..200)
            .map(|seed| Sensor::new("t", errors, seed).bias())
            .collect();
        assert!(biases.iter().all(|b| b.abs() <= 0.5), "{biases:?}");
        // A third of the accuracy as its spread: most sit well inside.
        let spread = (biases.iter().map(|b| b * b).sum::<f64>()
            / biases.len() as f64)
            .sqrt();
        assert!((0.12..0.21).contains(&spread), "{spread}");

        let sensor = Sensor::new("t", errors, 7);
        let first = sensor.read(290.0, 0.0) - 290.0;
        let later = sensor.read(290.0, 60.0) - 290.0;
        assert_eq!(first, later, "a calibration error does not move");
        assert_ne!(
            Sensor::new("t", errors, 7).bias(),
            Sensor::new("u", errors, 7).bias()
        );
    }

    /// The same seed gives the same readings.
    #[test]
    fn a_seed_repeats_a_run() {
        let errors = SensorErrors {
            noise: 0.1,
            accuracy: 0.5,
            drift: Wander {
                size: 0.2,
                time: Duration::from_secs(60),
            },
            lag: Duration::from_secs(3),
        };
        let run = |seed| {
            let sensor = Sensor::new("t", errors, seed);
            (0..10)
                .map(|i| sensor.read(290.0 + i as f64, i as f64))
                .collect::<Vec<f64>>()
        };
        assert_eq!(run(3), run(3));
        assert_ne!(run(3), run(4));
    }

    /// Noise on each reading has the standard deviation it is given.
    #[test]
    fn noise_has_its_standard_deviation() {
        let errors = SensorErrors {
            noise: 0.05,
            ..SensorErrors::default()
        };
        let sensor = Sensor::new("t", errors, 11);
        let readings: Vec<f64> = (0..20_000)
            .map(|i| sensor.read(300.0, i as f64 / 1000.0) - 300.0)
            .collect();
        let n = readings.len() as f64;
        let mean = readings.iter().sum::<f64>() / n;
        let sd = (readings.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / n)
            .sqrt();
        assert!(mean.abs() < 0.002, "{mean}");
        assert!((sd - 0.05).abs() < 0.002, "{sd}");
    }

    /// After a step in the body's temperature, a lagging sensor has closed
    /// all but 1/e of the gap in one time constant -- however often it is
    /// read on the way.
    #[test]
    fn a_lagging_sensor_follows_by_its_time_constant() {
        let errors = SensorErrors {
            lag: Duration::from_secs(5),
            ..SensorErrors::default()
        };
        for reads in [1u64, 10, 1000] {
            let sensor = Sensor::new("t", errors, 1);
            sensor.read(280.0, 0.0);
            // The body jumps to 290 and stays: read it at evenly spaced
            // times up to one time constant. The jump itself happens over
            // the first interval, so start just after it.
            sensor.read(290.0, 1e-9);
            let mut seen = 0.0;
            for i in 1..=reads {
                let at = 1e-9 + 5.0 * i as f64 / reads as f64;
                seen = sensor.read(290.0, at);
            }
            let left = 290.0 - seen;
            assert!(
                (left - 10.0 * (-1.0f64).exp()).abs() < 1e-3,
                "{reads} reads: {left} of the 10 K left"
            );
        }
    }

    /// Drift settles at the size it is given, whatever the reading rate.
    #[test]
    fn drift_settles_at_its_size() {
        let drift = Wander {
            size: 0.3,
            time: Duration::from_secs(10),
        };
        let errors = SensorErrors {
            drift,
            ..SensorErrors::default()
        };
        let sensor = Sensor::new("t", errors, 5);
        let readings: Vec<f64> = (0..50_000)
            .map(|i| sensor.read(300.0, i as f64) - 300.0)
            .collect();
        let n = readings.len() as f64;
        let sd = (readings.iter().map(|r| r * r).sum::<f64>() / n).sqrt();
        assert!((sd - 0.3).abs() < 0.02, "{sd}");
    }
}
