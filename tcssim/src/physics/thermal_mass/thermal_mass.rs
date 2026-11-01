//! A thermal mass, warmed by heaters and read by sensors.
//!
//! The first thing in this simulator with a state. Every other simulated
//! payload makes its bytes up -- `crate::payload::a_packet` fills a buffer
//! from a random number generator -- which is enough to exercise a link and
//! nothing at all to exercise a control loop: three sensors sending noise
//! never agree, so nothing downstream of them can be seen to work.
//!
//! A mass is what makes them agree. The sensors attached to one report its
//! temperature, each in its own encoding, and the heaters attached to it warm
//! it; so what the vehicle reads is a consequence of what the vehicle drove,
//! and a thermostat can be watched holding a band.
//!
//! # The arithmetic
//!
//! Newton's cooling with a constant heat:
//!
//! ```text
//! dT/dt = -(T - environment) / time_constant + heat
//! ```
//!
//! `heat` is the sum of the rates of the heaters that are on, in kelvin a
//! second. Between two changes of that sum it is constant, and then the
//! equation has a closed form:
//!
//! ```text
//! T(t + dt) = rest + (T - rest) * exp(-dt / time_constant)
//! rest      = environment + heat * time_constant
//! ```
//!
//! which is what this steps by. Exact rather than integrated: the mass is
//! stepped whenever it is read or a heater changes, so the steps are whatever
//! length the traffic makes them, and a method whose error grew with the step
//! would make the simulated temperature depend on how often somebody looked
//! at it.
//!
//! `rest` is where the mass settles with that heat on for ever, which is also
//! the figure that says whether the heaters can win at all: with a fifty
//! second constant and six kelvin a second of heat, it is three hundred kelvin
//! above the environment.
//!
//! # Starting
//!
//! At the environment, with the heaters off. A mass that started at its
//! setpoint would hide the one part of the loop that is interesting to watch
//! -- the first climb, with everything on -- and a vehicle coming up cold is
//! the case the thermostat exists for.
//!
//! # What varies
//!
//! Three things a real mass sees, each optional and each off unless the
//! simulator file asks for it:
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
//!   with `w` the swing's angular frequency and `tau` the time constant, so
//!   the swing is as exact as the rest.
//! * The environment wanders at random, and each heater's supply voltage
//!   does: each is an Ornstein-Uhlenbeck process, a departure drawn back
//!   towards nought, stepped by its exact transition,
//!
//!   ```text
//!   x(t + dt) = x * exp(-dt / theta) + size * sqrt(1 - exp(-2 dt / theta)) * z
//!   ```
//!
//!   with `z` a standard normal draw, so how much it wanders does not depend
//!   on how often the mass is stepped. Each is held at its value at the
//!   start of a step for that step, which is exact for any step much shorter
//!   than its time constant -- and a wander's time constant is minutes where
//!   a step is a sample interval. A heater's power goes as the square of its
//!   voltage, so it warms by `rate * (1 + v)^2` with `v` its supply's
//!   departure as a fraction of nominal.
//! * Each output may sit a fixed distance from the mass's temperature: a
//!   body warmed in one place is not one temperature throughout.
//!
//! The random draws come from a generator seeded from the run's seed and the
//! mass's name, so that a run with the same seed varies the same way.

use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rand_distr::StandardNormal;
use tcslibgs::node::vehicle::compute::{Format, ToKelvinParams};
use tcslibgs::sim_config::{ResolvedMass, Wander};

use super::sensor::Sensor;

/// A seed for one thing in a run, from the run's seed and the thing's name.
///
/// So that each mass and each sensor draws from a generator of its own: one
/// shared generator would hand out its numbers in whatever order the
/// payloads' threads happened to ask, and a run would not vary the same way
/// twice.
///
/// # Arguments
///
/// * `seed` - The run's seed.
/// * `what` - The thing's name, such as `mass:basic` or `sensor:temp_1`.
///
/// # Returns
///
/// The thing's seed: FNV-1a of its name, mixed with the run's.
pub fn seed_for(seed: u64, what: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in what.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash ^ seed
}

/// Steps a wander by its exact transition.
///
/// # Arguments
///
/// * `x`       - Where it is.
/// * `wander`  - Its size and time constant; one of no size stays at nought.
/// * `seconds` - How long to step it by.
/// * `rng`     - Where the normal draw comes from.
///
/// # Returns
///
/// Where it is `seconds` later.
pub fn wandered(
    x: f64,
    wander: &Wander,
    seconds: f64,
    rng: &mut StdRng,
) -> f64 {
    if wander.size <= 0.0 || seconds <= 0.0 {
        return x;
    }
    let keep = (-seconds / wander.time.as_secs_f64()).exp();
    let z: f64 = rng.sample(StandardNormal);
    x * keep + wander.size * (1.0 - keep * keep).sqrt() * z
}

/// One heater that is on: how hard it warms the mass at its nominal
/// voltage; its supply is in `State::supplies`.
#[derive(Debug, Clone, Copy)]
struct Heating {
    /// Kelvin a second, at its nominal voltage.
    rate: f64,
}

/// What a mass is at, and when that was worked out.
#[derive(Debug)]
struct State {
    /// Kelvin.
    temperature: f64,
    /// When `temperature` was last brought up to date.
    at: Instant,
    /// Which inputs have their heater on, by input name, and how hard each
    /// warms it.
    ///
    /// By name rather than a count, because the same heater being driven
    /// twice is one heater: a vehicle sends a command whenever its decision
    /// changes, and a simulator that added a rate per command would have the
    /// mass warming faster every time the thermostat changed its mind.
    heating: BTreeMap<String, Heating>,
    /// Each input's supply voltage, as its departure from nominal as a
    /// fraction of it, and how it wanders: kept, and wandering, while its
    /// heater is off, as a supply does.
    supplies: BTreeMap<String, (f64, Wander)>,
    /// How far the environment has wandered from where it swings, in kelvin.
    wander: f64,
    /// Where this mass's random draws come from.
    rng: StdRng,
}

/// A thermal mass the payloads are attached to.
///
/// Shared: the sensors read it and the heaters warm it, each from its own
/// thread, so the state is behind a lock. Taken for the length of an
/// arithmetic step and never across anything that waits.
#[derive(Debug)]
pub struct ThermalMass {
    name: String,
    /// What it tends towards with nothing warming it, in kelvin.
    environment: f64,
    /// How quickly it loses the difference, in seconds.
    time_constant: f64,
    /// The names of the places a sensor reads its temperature from.
    outputs: Vec<String>,
    /// How far each output sits from the mass's temperature, in kelvin.
    offsets: BTreeMap<String, f64>,
    /// How far the environment swings either side of `environment`.
    swing: f64,
    /// The swing's angular frequency, in radians a second; nought for none.
    omega: f64,
    /// How the environment wanders.
    wander: Wander,
    /// The time the swing is measured from.
    start: Instant,
    state: Mutex<State>,
}

impl ThermalMass {
    /// Makes a mass as its file states it, at the environment with nothing
    /// on.
    ///
    /// # Arguments
    ///
    /// * `mass` - The mass the simulator file declares.
    /// * `now`  - The time its temperature is at.
    /// * `seed` - The run's seed.
    ///
    /// # Returns
    ///
    /// The mass, at its environment as it is at `now`: wandered as far as a
    /// first draw says, and at the middle of its swing.
    pub fn new(mass: &ResolvedMass, now: Instant, seed: u64) -> ThermalMass {
        let mut rng = StdRng::seed_from_u64(seed_for(
            seed,
            &format!("mass:{}", mass.name),
        ));
        // Where a settled wander would be: started at nought, it would show
        // a quiet beginning no real environment has.
        let wander = if mass.wander.size > 0.0 {
            mass.wander.size * rng.sample::<f64, _>(StandardNormal)
        } else {
            0.0
        };
        let omega = if mass.swing > 0.0 {
            TAU / mass.period.as_secs_f64()
        } else {
            0.0
        };
        ThermalMass {
            name: mass.name.clone(),
            environment: mass.environment,
            time_constant: mass.time_constant.as_secs_f64(),
            outputs: mass.outputs.clone(),
            offsets: mass.offsets.clone(),
            swing: mass.swing,
            omega,
            wander: mass.wander,
            start: now,
            state: Mutex::new(State {
                temperature: mass.environment + wander,
                at: now,
                heating: BTreeMap::new(),
                supplies: BTreeMap::new(),
                wander,
                rng,
            }),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// What one of its outputs reads now.
    ///
    /// The mass is one lump, at one temperature throughout, so every output
    /// reads the same: what tells them apart is which sensor is on which.
    ///
    /// # Arguments
    ///
    /// * `output` - The output's name.
    ///
    /// # Returns
    ///
    /// Its temperature in kelvin, with how far that output sits from the
    /// mass, or `None` for a name the mass does not list among its outputs.
    pub fn output(&self, output: &str) -> Option<f64> {
        if !self.outputs.iter().any(|o| o == output) {
            return None;
        }
        let away = self.offsets.get(output).copied().unwrap_or(0.0);
        Some(self.temperature() + away)
    }

    /// What it is now, in kelvin.
    pub fn temperature(&self) -> f64 {
        let mut state = self.held();
        self.step(&mut state, Instant::now());
        state.temperature
    }

    /// Tells it that the heater on one of its inputs is on, or off.
    ///
    /// The mass is stepped first, so the rate that was in force applies up to
    /// this moment and the new one from here: a heater switched on halfway
    /// through a second warms the mass for half of it.
    ///
    /// # Arguments
    ///
    /// * `which`  - The input's name; the same input twice is one heater.
    /// * `rate`   - How hard it warms the mass while on, in kelvin a second,
    ///              at its nominal voltage.
    /// * `supply` - How its supply voltage wanders.
    /// * `on`     - Whether it is now on.
    pub fn heater(&self, which: &str, rate: f64, supply: Wander, on: bool) {
        let mut state = self.held();
        self.step(&mut state, Instant::now());
        if on {
            if supply.size > 0.0 && !state.supplies.contains_key(which) {
                // A supply settled where it would be, as the wander is.
                let v =
                    supply.size * state.rng.sample::<f64, _>(StandardNormal);
                state.supplies.insert(which.to_string(), (v, supply));
            }
            state.heating.insert(which.to_string(), Heating { rate });
        } else {
            state.heating.remove(which);
        }
    }

    /// How hard it is being warmed, in kelvin a second.
    ///
    /// What the tests of the heaters check, and nothing in a run asks: the
    /// mass applies the heat itself and what a run shows is the temperature.
    #[cfg(test)]
    pub fn heat(&self) -> f64 {
        let state = self.held();
        heat_of(&state)
    }

    /// The lock, taken through one place so a poisoned one is dealt with once.
    ///
    /// A poisoned lock means a thread panicked holding the state, which is a
    /// fault to find in the log rather than a reason for a simulator to stop
    /// answering: the state is a temperature and a time, and the worst that
    /// is in it is a temperature that was not stepped.
    fn held(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Brings the temperature up to a time.
    ///
    /// The closed form of the module comment, which is exact for the heat
    /// that was in force over the interval -- and it was in force over all of
    /// it, because every change of it steps the mass first.
    ///
    /// # Arguments
    ///
    /// * `state` - The mass's state, held locked by the caller.
    /// * `now`   - The time to bring it to; one no later than its own
    ///             changes nothing.
    fn step(&self, state: &mut State, now: Instant) {
        let from = state.at;
        let seconds = now.saturating_duration_since(from).as_secs_f64();
        state.at = now;
        if seconds <= 0.0 {
            return;
        }

        let heat = heat_of(state);
        let rest = self.environment + state.wander + heat * self.time_constant;
        let decay = (-seconds / self.time_constant).exp();
        let then = self.swung(from);
        state.temperature =
            rest + self.swung(now) + (state.temperature - rest - then) * decay;

        // Then what was held for the step moves on to where it is now.
        state.wander =
            wandered(state.wander, &self.wander, seconds, &mut state.rng);
        let State { supplies, rng, .. } = state;
        for (v, supply) in supplies.values_mut() {
            *v = wandered(*v, supply, seconds, rng);
        }
    }

    /// The swing's steady response at a time: what the sine wave of the
    /// environment adds to the mass once its start has died away.
    ///
    /// # Arguments
    ///
    /// * `at` - The time.
    ///
    /// # Returns
    ///
    /// Kelvin; nought for a mass whose environment does not swing.
    fn swung(&self, at: Instant) -> f64 {
        if self.swing <= 0.0 {
            return 0.0;
        }
        let t = at.saturating_duration_since(self.start).as_secs_f64();
        let wt = self.omega * self.time_constant;
        let phase = self.omega * t;
        self.swing * (phase.sin() - wt * phase.cos()) / (1.0 + wt * wt)
    }
}

/// How hard a mass is being warmed now, in kelvin a second: each heater's
/// rate at its supply's voltage, whose square its power goes as.
///
/// # Arguments
///
/// * `state` - The mass's state.
///
/// # Returns
///
/// The sum.
fn heat_of(state: &State) -> f64 {
    state
        .heating
        .iter()
        .map(|(input, heating)| {
            let v = state.supplies.get(input).map_or(0.0, |(v, _)| *v);
            heating.rate * (1.0 + v) * (1.0 + v)
        })
        .sum()
}

/// What one simulated payload is to a thermal mass.
///
/// Held by the payload rather than by the mass, because it is the payload's
/// business: a sensor's thread asks the mass what it is when a packet is due,
/// and a heater's thread tells it when the vehicle drives it.
#[derive(Debug, Clone)]
pub enum Thermal {
    /// It reports the temperature at this output of this mass, in this
    /// encoding.
    Reads {
        mass: Arc<ThermalMass>,
        output: String,
        /// Taken from the conversion the readings go to, so the vehicle reads
        /// back the temperature the mass is at: see
        /// `tcslibgs::graph::simulated::reading_of`.
        reading: ToKelvinParams,
        /// What the sensor gets wrong, and what it has got wrong so far.
        sensor: Arc<Sensor>,
    },
    /// It warms this mass through this input, by this many kelvin a second
    /// while it is on.
    Heats {
        mass: Arc<ThermalMass>,
        input: String,
        rate: f64,
        /// How its supply voltage wanders.
        supply: Wander,
    },
}

/// Tells the mass what the vehicle just drove this heater to, if it is one.
///
/// What arrives at a heater is the thermostat's decision, which a hysteresis
/// node sends as the text `1` or `0`: the value to drive a set of discrete
/// lines to. Anything else is left alone rather than guessed at -- a payload
/// that heats a mass and is sent something that is not a decision is a
/// configuration pointing the wrong link at it, and a simulator that read it
/// as "off" would hide that by behaving plausibly.
///
/// Nothing at all for a payload that heats nothing, which is every payload
/// but a heater: the caller does not have to ask first.
///
/// # Arguments
///
/// * `thermal` - What the payload is to a thermal mass, if anything.
/// * `which`   - The payload's name, for a complaint.
/// * `data`    - What the vehicle sent it: `1` for on, `0` for off,
///               surrounding white space ignored.
pub fn driven(thermal: &Option<Thermal>, which: &str, data: &[u8]) {
    let Some(Thermal::Heats {
        mass,
        input,
        rate,
        supply,
    }) = thermal
    else {
        return;
    };
    match data.trim_ascii() {
        b"1" => mass.heater(input, *rate, *supply, true),
        b"0" => mass.heater(input, *rate, *supply, false),
        _ => eprintln!(
            "{which} warms the thermal mass {} and was sent {} bytes that are \
             not a decision: a heater is driven to 1 or 0",
            mass.name(),
            data.len()
        ),
    }
}

/// Encodes a temperature as the bytes a sensor of this encoding would
/// send.
///
/// The inverse of what `to_kelvin` does: it reads counts and answers
/// `counts * scale + offset`, so this takes the temperature back to counts
/// and lays them out in the format the conversion expects.
///
/// Rounded to the nearest count, which is a real sensor's quantisation and
/// worth having in a simulation: the example's parts are a sixteenth of a
/// kelvin and eight thousandths, so a loop that only worked on exact
/// temperatures would be a loop that does not work.
///
/// A temperature outside what the format can hold is clamped to its ends
/// rather than wrapped. A wrapped count is a sensor reporting a temperature at
/// the other end of its range, which is a reading an operator would act on; a
/// clamped one is a sensor at the end of its range, which is what a real part
/// does.
///
/// # Arguments
///
/// * `reading` - The conversion the readings go to, which gives the
///               format, scale and offset.
/// * `kelvin`  - The temperature, in kelvin.
///
/// # Returns
///
/// The sensor's bytes, two or three of them as the format says.
pub fn encode(reading: &ToKelvinParams, kelvin: f64) -> Vec<u8> {
    let counts = ((kelvin - reading.offset) / reading.scale).round();
    match reading.format {
        Format::I16Be => {
            let held = counts.clamp(i16::MIN as f64, i16::MAX as f64) as i16;
            held.to_be_bytes().to_vec()
        }
        Format::I16Le => {
            let held = counts.clamp(i16::MIN as f64, i16::MAX as f64) as i16;
            held.to_le_bytes().to_vec()
        }
        Format::U16Be => {
            let held = counts.clamp(0.0, u16::MAX as f64) as u16;
            held.to_be_bytes().to_vec()
        }
        Format::U24Be => {
            let held = counts.clamp(0.0, 0xFF_FFFF as f64) as u32;
            held.to_be_bytes()[1..].to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload::{PayloadConfig, PayloadProtocol, SimulatedPayload};
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;
    use tcslibgs::sim_config::SensorErrors;

    /// The example's mass: ten kelvin below a setpoint of 293.15, losing the
    /// difference with a fifty second constant.
    ///
    /// # Returns
    ///
    /// The mass, named `cabin`.
    fn cabin() -> ResolvedMass {
        ResolvedMass {
            name: "cabin".to_string(),
            environment: 283.15,
            time_constant: Duration::from_secs(50),
            inputs: vec!["in1".to_string(), "in2".to_string()],
            outputs: ["out1", "out2", "out3"].map(String::from).to_vec(),
            ..Default::default()
        }
    }

    /// The swing's closed form agrees with integrating the equation
    /// numerically, in one step or many.
    #[test]
    fn a_swinging_environment_is_stepped_exactly() {
        let swing = ResolvedMass {
            swing: 5.0,
            period: Duration::from_secs(100),
            ..cabin()
        };
        let start = Instant::now();
        let mass = ThermalMass::new(&swing, start, 0);
        mass.heater("in1", 0.1, Wander::default(), true);
        let mut state = mass.held();
        state.at = start;
        state.temperature = 290.0;
        let seconds = 300.0;
        mass.step(&mut state, start + Duration::from_secs_f64(seconds));
        let exact = state.temperature;

        // Fourth-order Runge-Kutta over the same 300 s, a hundredth of a
        // second at a time.
        let tau = 50.0;
        let w = std::f64::consts::TAU / 100.0;
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

    /// The environment's wander settles at its size, and the supply's
    /// ripple warms the mass by its rate times (1 + v)^2 on average.
    #[test]
    fn wanders_settle_at_their_sizes() {
        let wandering = ResolvedMass {
            wander: Wander {
                size: 0.5,
                time: Duration::from_secs(30),
            },
            ..cabin()
        };
        let start = Instant::now();
        let mass = ThermalMass::new(&wandering, start, 3);
        let supply = Wander {
            size: 0.05,
            time: Duration::from_secs(30),
        };
        mass.heater("in1", 3.0, supply, true);
        let mut state = mass.held();
        let (mut wander, mut heat, mut heat2) = (0.0, 0.0, 0.0);
        let n = 100_000;
        for i in 1..=n {
            mass.step(&mut state, start + Duration::from_secs(30 * i));
            wander += state.wander * state.wander;
            let h = heat_of(&state);
            heat += h;
            heat2 += h * h;
        }
        let n = n as f64;
        let wander = (wander / n).sqrt();
        assert!((wander - 0.5).abs() < 0.02, "{wander}");
        let mean = heat / n;
        let sd = (heat2 / n - mean * mean).sqrt();
        assert!((mean - 3.0 * (1.0 + 0.05 * 0.05)).abs() < 0.005, "{mean}");
        assert!((sd - 3.0 * 2.0 * 0.05).abs() < 0.02, "{sd}");
    }

    /// An output sits its offset from the mass, and the others where it is.
    #[test]
    fn an_output_sits_its_offset_away() {
        let mut offsets = BTreeMap::new();
        offsets.insert("out2".to_string(), 0.3);
        let offset = ResolvedMass { offsets, ..cabin() };
        let mass = ThermalMass::new(&offset, Instant::now(), 0);
        let out1 = mass.output("out1").expect("out1");
        let out2 = mass.output("out2").expect("out2");
        assert!((out2 - out1 - 0.3).abs() < 1e-9, "{out1} {out2}");
        assert_eq!(mass.output("out9"), None);
    }

    /// The same seed wanders the same way.
    #[test]
    fn a_seed_repeats_a_mass() {
        let wandering = ResolvedMass {
            wander: Wander {
                size: 0.5,
                time: Duration::from_secs(30),
            },
            ..cabin()
        };
        let start = Instant::now();
        let run = |seed| {
            let mass = ThermalMass::new(&wandering, start, seed);
            let mut state = mass.held();
            (1..20)
                .map(|i| {
                    mass.step(&mut state, start + Duration::from_secs(i));
                    state.temperature
                })
                .collect::<Vec<f64>>()
        };
        assert_eq!(run(1), run(1));
        assert_ne!(run(1), run(2));
    }

    /// A mass with no heat on it settles at its environment, from either side.
    #[test]
    fn a_mass_settles_at_its_environment() {
        let mass = ThermalMass::new(&cabin(), Instant::now(), 0);
        let mut state = mass.held();

        // It starts there, which is what a vehicle coming up cold is.
        assert!((state.temperature - 283.15).abs() < 1e-9);

        // Warmer, and left alone: it falls back, and by the time constant it
        // has lost about two thirds of the difference.
        state.temperature = 293.15;
        let start = state.at;
        mass.step(&mut state, start + Duration::from_secs(50));
        let left = state.temperature - 283.15;
        assert!(
            (left - 10.0 * (-1.0f64).exp()).abs() < 1e-9,
            "a time constant should leave 1/e of the ten kelvin: {left}"
        );

        // And a long time later it is there.
        mass.step(&mut state, start + Duration::from_secs(10_000));
        assert!((state.temperature - 283.15).abs() < 1e-6);
    }

    /// The rate at the setpoint is the figure the example is written from.
    ///
    /// Ten kelvin above the environment with a fifty second constant is a
    /// fifth of a kelvin a second, which is what the thermostat's cooling leg
    /// is specified as. Checked here because the file states the constant and
    /// a reader is owed the arithmetic that turns it into the rate.
    #[test]
    fn the_cooling_rate_at_the_setpoint_is_a_fifth_of_a_kelvin() {
        let mass = ThermalMass::new(&cabin(), Instant::now(), 0);
        let mut state = mass.held();
        state.temperature = 293.15;
        let start = state.at;

        // Over a tenth of a second, which is the example's sample interval.
        mass.step(&mut state, start + Duration::from_millis(100));
        let fell = 293.15 - state.temperature;
        assert!(
            (fell - 0.02).abs() < 1e-4,
            "a fifth of a kelvin a second is 0.02 K in 100 ms: {fell}"
        );
    }

    /// A heater warms it, two warm it twice as fast, and the same heater
    /// twice is one heater.
    #[test]
    fn heaters_warm_it_and_are_counted_once_each() {
        let mass = ThermalMass::new(&cabin(), Instant::now(), 0);

        // One heater of three kelvin a second: at the environment, where
        // there is nothing to lose, the mass gains very nearly that.
        mass.heater("heaters_1", 3.0, Wander::default(), true);
        assert!((mass.heat() - 3.0).abs() < 1e-9);

        // Driven on again, which a vehicle does whenever its decision
        // changes: still one heater, and still three.
        mass.heater("heaters_1", 3.0, Wander::default(), true);
        assert!(
            (mass.heat() - 3.0).abs() < 1e-9,
            "one heater driven twice is one heater: {}",
            mass.heat()
        );

        // And the second one makes it six.
        mass.heater("heaters_2", 3.0, Wander::default(), true);
        assert!((mass.heat() - 6.0).abs() < 1e-9);

        mass.heater("heaters_1", 3.0, Wander::default(), false);
        assert!(
            (mass.heat() - 3.0).abs() < 1e-9,
            "one of the two off leaves the other on"
        );
    }

    /// Where a mass settles with its heaters on, which is what says whether
    /// they can win.
    #[test]
    fn the_heaters_decide_where_it_settles() {
        let mass = ThermalMass::new(&cabin(), Instant::now(), 0);
        mass.heater("heaters_1", 3.0, Wander::default(), true);
        mass.heater("heaters_2", 3.0, Wander::default(), true);

        let mut state = mass.held();
        let start = state.at;
        // Long enough to settle: six kelvin a second against a fifty second
        // constant rests three hundred kelvin above the environment.
        mass.step(&mut state, start + Duration::from_secs(10_000));
        assert!(
            (state.temperature - (283.15 + 300.0)).abs() < 1e-6,
            "{}",
            state.temperature
        );
    }

    /// A temperature encodes as the bytes the vehicle reads it back from.
    ///
    /// Against the example's two parts, in their own formats and scales: this
    /// is the one piece of arithmetic in the simulator whose answer another
    /// program checks, so what it is held to is the round trip.
    #[test]
    fn a_temperature_encodes_as_the_bytes_the_vehicle_reads_back() {
        let parts = [
            // temp_1 and temp_2: two bytes, a sixteenth of a kelvin a count,
            // above freezing.
            ToKelvinParams {
                format: Format::I16Be,
                scale: 0.0625,
                offset: 273.15,
            },
            // temp_3: three bytes, eight thousandths of a kelvin, its own
            // offset.
            ToKelvinParams {
                format: Format::U24Be,
                scale: 0.0078,
                offset: 255.37,
            },
        ];

        for reading in parts {
            for kelvin in [283.15, 291.15, 293.15, 295.15, 300.0] {
                let bytes = encode(&reading, kelvin);
                // Read back the way the vehicle reads it: counts times the
                // scale, plus the offset.
                let counts = match reading.format {
                    Format::I16Be => {
                        i16::from_be_bytes([bytes[0], bytes[1]]) as f64
                    }
                    Format::U24Be => {
                        u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]])
                            as f64
                    }
                    _ => unreachable!("the parts above"),
                };
                let back = counts * reading.scale + reading.offset;
                // Within half a count, which is the sensor's quantisation and
                // not an error in the arithmetic.
                assert!(
                    (back - kelvin).abs() <= reading.scale / 2.0 + 1e-9,
                    "{kelvin} K encoded as {bytes:?} reads back as {back}"
                );
            }
        }

        // And a temperature the format cannot hold is clamped, not wrapped: a
        // wrapped count is a sensor reporting the other end of its range.
        let part = ToKelvinParams {
            format: Format::U16Be,
            scale: 1.0,
            offset: 0.0,
        };
        assert_eq!(encode(&part, -50.0), vec![0x00, 0x00]);
        assert_eq!(encode(&part, 1e9), vec![0xFF, 0xFF]);
    }

    /// A step of no time changes nothing, which is what a sensor read twice in
    /// one instant must not do.
    #[test]
    fn a_step_of_no_time_changes_nothing() {
        let mass = ThermalMass::new(&cabin(), Instant::now(), 0);
        mass.heater("heaters_1", 3.0, Wander::default(), true);
        let first = mass.temperature();
        let again = {
            let mut state = mass.held();
            let at = state.at;
            mass.step(&mut state, at);
            state.temperature
        };
        assert!((first - again).abs() < 1e-12);
    }

    /// A free port, for a payload that is the end which waits.
    ///
    /// A heater binds the address the configuration gives, because a device
    /// the vehicle drives is the end that has to be findable, so the test has
    /// to name one that is free rather than take whatever a socket of its own
    /// was given.
    ///
    /// # Returns
    ///
    /// A loopback UDP port that was free a moment ago.
    ///
    /// # Panics
    ///
    /// If no socket can be bound.
    fn a_free_port() -> u16 {
        let socket =
            std::net::UdpSocket::bind("127.0.0.1:0").expect("a socket");
        let port = socket.local_addr().expect("its address").port();
        drop(socket);
        port
    }

    /// Configures a simulated payload of the thermostat example, attached to
    /// the mass.
    ///
    /// # Arguments
    ///
    /// * `name`    - The payload's name.
    /// * `at`      - The loopback UDP port it uses.
    /// * `bytes`   - Its packet size, and its segment size too.
    /// * `every`   - The packet interval in ms: for a sensor how often it
    ///               reports, and for a heater -- which sends nothing -- how
    ///               often it looks for what the vehicle decided.
    /// * `thermal` - What it is to the mass.
    ///
    /// # Returns
    ///
    /// Its configuration, sending from the start.
    fn attached(
        name: &str,
        at: u16,
        bytes: u32,
        every: u32,
        thermal: Thermal,
    ) -> PayloadConfig {
        PayloadConfig {
            thermal: Some(thermal),
            thermostat: None,
            name: name.to_string(),
            protocol: PayloadProtocol::Udp,
            address: "127.0.0.1".to_string(),
            port: at,
            bus_address: 0,
            can_id: 0,
            can_fd: false,
            gpio_lines: Vec::new(),
            triggered: false,
            faults: Default::default(),
            own_address: None,
            own_port: None,
            packet_size: Arc::new(AtomicU32::new(bytes)),
            segment_size: Arc::new(AtomicU32::new(bytes)),
            packet_interval_ms: Arc::new(AtomicU32::new(every)),
            segment_interval_ms: Arc::new(AtomicU32::new(0)),
            sending: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            standing: Default::default(),
            driven: false,
        }
    }

    /// What the vehicle reads a sensor's bytes back as.
    ///
    /// The conversion a `to_kelvin` node does, written out here because it is
    /// tcspecial's and this is the other program: counts times the scale,
    /// plus the offset. Its agreement with [`encode`] is what
    /// `a_temperature_encodes_as_the_bytes_the_vehicle_reads_back` holds.
    ///
    /// # Arguments
    ///
    /// * `reading` - The sensor's conversion.
    /// * `bytes`   - What it sent.
    ///
    /// # Returns
    ///
    /// The temperature in kelvin, or `None` if there are too few bytes for
    /// the format or the format is not one the example's sensors use.
    fn kelvin_of(reading: &ToKelvinParams, bytes: &[u8]) -> Option<f64> {
        let counts = match reading.format {
            Format::I16Be if bytes.len() >= 2 => {
                i16::from_be_bytes([bytes[0], bytes[1]]) as f64
            }
            Format::U24Be if bytes.len() >= 3 => {
                u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]]) as f64
            }
            _ => return None,
        };
        Some(counts * reading.scale + reading.offset)
    }

    /// The example's thermostat, as the vehicle's own nodes would decide it.
    ///
    /// The median of the three readings, and a heat decision that turns on
    /// below the band and off above it -- `vote` and `hysteresis`, whose own
    /// arithmetic is tested where it lives. Here they stand in for the
    /// vehicle, so that what is tested is the loop: the mass, the encoding,
    /// the two links and the heaters.
    ///
    /// # Arguments
    ///
    /// * `readings` - The sensors' latest temperatures, in kelvin.
    /// * `heating`  - Whether the heaters are on now, which is kept inside
    ///                the band.
    ///
    /// # Returns
    ///
    /// Whether the heaters are to be on.
    ///
    /// # Panics
    ///
    /// If a reading is NaN, or there are none.
    fn decide(readings: &[f64], heating: bool) -> bool {
        let mut sorted = readings.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).expect("no NaN temperatures"));
        let voted = sorted[sorted.len() / 2];
        match voted {
            t if t < SETPOINT - HALF_BAND => true,
            t if t > SETPOINT + HALF_BAND => false,
            _ => heating,
        }
    }

    const SETPOINT: f64 = 293.15;
    const HALF_BAND: f64 = 2.0;
    /// The example's sample rate: how often each sensor reports.
    const SAMPLE_MS: u32 = 100;
    /// How often a heater looks for a decision.
    ///
    /// Faster than the sensors sample, and for a reason the overshoot shows:
    /// a decision is only as prompt as the slower of the two, so a heater
    /// that looked every sample would double the time the vehicle spends
    /// heating a mass that has already arrived.
    const LOOK_MS: u32 = 20;
    /// What each heater adds, in kelvin a second.
    const HEATER_RATE: f64 = 3.0;

    /// The loop closes: the mass the sensors report is warmed by what the
    /// vehicle decided, the band is held, and a lost decision is corrected.
    ///
    /// Everything here is the real thing but the vehicle: three simulated
    /// sensors and two simulated heaters, each a payload thread of its own
    /// sending and receiving over real sockets, attached to one mass. The
    /// vehicle's part is [`decide`], which is what its `vote` and
    /// `hysteresis` nodes come to.
    ///
    /// It is the one test that can fail for a reason outside this file -- a
    /// loaded machine delays a packet, and a late sample is a late decision --
    /// so what it holds is the shape of the answer and not a trajectory: the
    /// mass starts cold, reaches the band, and from then on stays inside it
    /// within one sample of heating. A bound of one sample is the physics: the
    /// vehicle cannot switch heaters off between two readings, so the most it
    /// can overshoot by is what they add in the interval it did not look.
    #[test]
    fn the_heaters_hold_the_mass_in_band_through_a_lost_decision() {
        let mass = Arc::new(ThermalMass::new(&cabin(), Instant::now(), 0));

        // The two encodings of the example, so that what is voted over is
        // readings of one temperature in different formats.
        let two_byte = ToKelvinParams {
            format: Format::I16Be,
            scale: 0.0625,
            offset: 273.15,
        };
        let three_byte = ToKelvinParams {
            format: Format::U24Be,
            scale: 0.0078,
            offset: 255.37,
        };

        // The vehicle's end of each sensor link, which is the end that waits:
        // a sensor speaks first and this is where it speaks to.
        let readings = [two_byte, two_byte, three_byte];
        let waiting: Vec<std::net::UdpSocket> = (0..3)
            .map(|_| {
                let socket = std::net::UdpSocket::bind("127.0.0.1:0")
                    .expect("a socket for the vehicle");
                socket.set_nonblocking(true).expect("non-blocking");
                socket
            })
            .collect();

        let mut payloads: Vec<SimulatedPayload> = Vec::new();
        for (n, (socket, reading)) in waiting.iter().zip(readings).enumerate() {
            let at = socket.local_addr().expect("its address").port();
            let bytes = if reading.format == Format::U24Be {
                3
            } else {
                2
            };
            let mut sensor = SimulatedPayload::new(attached(
                &format!("temp_{}", n + 1),
                at,
                bytes,
                SAMPLE_MS,
                Thermal::Reads {
                    mass: mass.clone(),
                    output: format!("out{}", n + 1),
                    reading,
                    sensor: Arc::new(Sensor::new(
                        "sensor",
                        SensorErrors::default(),
                        0,
                    )),
                },
            ));
            sensor.start().expect("a sensor starts");
            payloads.push(sensor);
        }

        // And the heaters, which wait to be driven.
        //
        // Their ports are asked for by binding and letting go, so a port that
        // was free when it was chosen may be taken by the time the heater
        // binds it -- which would leave the loop open at one end. So the pair
        // is asked for again: a port that was free a moment ago is not a port
        // that is free, and a heater that lost one says so in its own words
        // on the way past.
        //
        // Waiting for each to be listening is what tells a lost port from a
        // slow start. Without it a heater that never bound would show up at
        // the end of this test as a thermal mass that would not warm, which
        // says nothing about why.
        let mut heaters = Vec::new();
        for attempt in 1..=10 {
            let ports: Vec<u16> = (0..2).map(|_| a_free_port()).collect();
            let mut started: Vec<SimulatedPayload> = Vec::new();
            for (n, at) in ports.iter().enumerate() {
                let mut heater = SimulatedPayload::new(attached(
                    &format!("heaters_{}", n + 1),
                    *at,
                    1,
                    LOOK_MS,
                    Thermal::Heats {
                        mass: mass.clone(),
                        input: format!("in{}", n + 1),
                        rate: HEATER_RATE,
                        supply: Wander::default(),
                    },
                ));
                heater.start().expect("a heater starts");
                started.push(heater);
            }

            let listening = ports.iter().all(|at| {
                (0..100).any(|_| {
                    if std::net::UdpSocket::bind(("127.0.0.1", *at)).is_err() {
                        return true;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                    false
                })
            });
            if listening {
                payloads.extend(started);
                heaters = ports;
                break;
            }

            for mut heater in started {
                heater.stop();
            }
            assert!(
                attempt < 10,
                "ten pairs of ports were taken between being chosen and being \
                 bound"
            );
        }

        // The vehicle's end of the heater links: it sends where the file says
        // the heater is, which is what `reaching_endpoint_pair` does aboard.
        let driving = std::net::UdpSocket::bind("127.0.0.1:0")
            .expect("a socket for the vehicle");
        let drive = |on: bool| {
            for at in &heaters {
                let _ = driving
                    .send_to(if on { b"1" } else { b"0" }, ("127.0.0.1", *at));
            }
        };

        // What the vehicle has heard from each sensor, and what it decided.
        let mut latest: Vec<Option<f64>> = vec![None; 3];
        let mut heating = false;
        // Whether the one decision this test throws away has been thrown
        // away yet.
        let mut lost = false;
        let mut reached: Option<Duration> = None;
        let mut lowest = f64::MAX;
        let mut highest = f64::MIN;

        let start = Instant::now();
        // Long enough to climb eight kelvin at six a second and then to be
        // watched holding the band for a while after it has.
        let watch = Duration::from_secs(4);
        while start.elapsed() < watch {
            let mut arrived = false;
            for (n, (socket, reading)) in
                waiting.iter().zip(&readings).enumerate()
            {
                let mut buffer = [0u8; 64];
                while let Ok(got) = socket.recv(&mut buffer) {
                    if let Some(kelvin) = kelvin_of(reading, &buffer[..got]) {
                        latest[n] = Some(kelvin);
                        arrived = true;
                    }
                }
            }

            // Decided on every arrival and not on every pass, which is what
            // the vote node does -- and what decides how often the heaters
            // are told anything.
            if !arrived {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            if let Some(reported) =
                latest.iter().copied().collect::<Option<Vec<f64>>>()
            {
                // Driven on every sample, which is what the vehicle does:
                // the hysteresis node asserts its decision each time a
                // temperature arrives and the devices are held at it. The
                // transitions are what gets recorded, and nothing here
                // records anything.
                //
                // The first word to switch them off is thrown away, as a lost
                // datagram throws one away. That is the failure this
                // arrangement exists for: a decision sent once and left to
                // stand would leave the heaters on with the vehicle believing
                // otherwise, and the mass with no bound on where it goes.
                let want = decide(&reported, heating);
                let switching_off = heating && !want;
                heating = want;
                if switching_off && !lost {
                    lost = true;
                } else {
                    drive(heating);
                }

                let voted = {
                    let mut sorted = reported.clone();
                    sorted.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
                    sorted[1]
                };
                if reached.is_none() && voted >= SETPOINT - HALF_BAND {
                    reached = Some(start.elapsed());
                }
                if reached.is_some() {
                    lowest = lowest.min(voted);
                    highest = highest.max(voted);
                }
            }

            std::thread::sleep(Duration::from_millis(5));
        }

        for payload in &mut payloads {
            payload.stop();
        }

        // It started cold and the heaters brought it to the band. Six kelvin
        // a second over eight kelvin is under two seconds, and the bound here
        // is loose enough for a loaded machine to be slow at it.
        let reached = reached.unwrap_or_else(|| {
            panic!(
                "the mass never reached the band: it is at {} K, and the \
                 vehicle {} driving the heaters",
                mass.temperature(),
                if heating { "is" } else { "is not" }
            )
        });
        assert!(
            reached < Duration::from_secs(3),
            "eight kelvin at six a second took {reached:?}"
        );

        // The loss has to have happened, or this test is only the band test
        // under a longer name.
        assert!(
            lost,
            "the heaters were never switched off, so no decision was lost"
        );

        // And from then on it is held, within what the loop cannot help.
        // Three delays make the overshoot here: the vehicle does not see the
        // band crossed until the next sample, the heaters do not hear about it
        // until they next look, and the first word to switch them off was
        // thrown away -- so one more sample of heating before the next
        // assertion corrects it. A tenth of a kelvin on top of that is the
        // scheduler, which is a thirtieth of the interval and not a
        // trajectory this test is holding anybody to.
        //
        // That it is a bound at all is the point. Before the decision was
        // asserted every sample, the thrown-away word would have stood for
        // ever: the thermostat would have gone on deciding `Off`, found it
        // had already said so, and said nothing, with the mass climbing three
        // hundred kelvin towards where two heaters rest it.
        let delay = (2 * SAMPLE_MS + LOOK_MS) as f64 / 1000.0;
        let slack = 2.0 * HEATER_RATE * delay + 0.1;
        assert!(
            highest <= SETPOINT + HALF_BAND + slack,
            "it went {highest} K, over a band ending at {} K",
            SETPOINT + HALF_BAND
        );
        assert!(
            lowest >= SETPOINT - HALF_BAND - slack,
            "it fell to {lowest} K, under a band starting at {} K",
            SETPOINT - HALF_BAND
        );
    }
}
