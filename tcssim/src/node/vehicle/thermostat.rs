//! A thermostat: the temperatures the sensors report, a setpoint, a dead
//! band, and the heaters switched on or off.
//!
//! What a configuration's hysteresis node does aboard, done in the simulator
//! so that a zone's loop closes with no tcspecial running. It stands
//! in for one hysteresis node of the configuration, whose `setpoint` and
//! `half_band` it takes, and the simulator file names that node: see
//! `tcslibgs::sim_config::SimTcssim::thermostats`. Several zones, or a
//! staged thermostat, are several of these, each switching heaters of its
//! own.
//!
//! # What it reads
//!
//! The bytes each sensor stand-in sends, as they leave it: so a reading the
//! simulator drops never arrives, and one it truncates or corrupts arrives
//! as the vehicle would see it. Each is read as the conversion the sensor
//! feeds reads it -- `Format::counts`, then the scale and the offset -- and
//! one that conversion would refuse is ignored.
//!
//! The sensors are the simulated payloads that report the world whose data
//! reaches the hysteresis node in the configuration; the heaters are the
//! ones driving it that data from the node reaches.
//!
//! # What it decides
//!
//! On every reading, from the median of each sensor's latest: more than
//! `half_band` over the setpoint, off; more than `half_band` under it, on;
//! between the two, the last decision. The median is what three sensors are
//! voted to in the thermostat example, and it lets one sensor that has gone
//! wrong be outvoted.
//!
//! The decision is asserted on every reading, as the hysteresis node does
//! aboard: a heater is held at what was last decided rather than told only
//! when it changes.

use std::collections::BTreeMap;
use std::sync::Mutex;

use tcslibgs::node::vehicle::compute::{HysteresisParams, ToValueParams};

use crate::physics::{driven, Attachment};

/// What the heaters should be doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// The heaters are on.
    On,
    /// The heaters are off.
    Off,
}

/// What the thermostat has seen and decided.
#[derive(Debug, Default)]
struct State {
    /// Each sensor's latest temperature, in kelvin, by payload name.
    latest: BTreeMap<String, f64>,
    /// What it last decided; `None` until the first reading outside the
    /// band.
    decision: Option<Decision>,
}

/// A thermostat the simulator runs in place of a hysteresis node.
///
/// Shared by the sensor stand-ins that report to it, each from its own
/// thread, so what it has seen is behind a lock.
#[derive(Debug)]
pub struct Thermostat {
    /// The hysteresis node it stands in for.
    name: String,
    /// The temperature it holds the zone at, in kelvin.
    setpoint: f64,
    /// How far from the setpoint the decision changes, each way.
    half_band: f64,
    /// How each sensor's bytes are read, by payload name.
    sensors: BTreeMap<String, ToValueParams>,
    /// The heaters it switches, by payload name.
    heaters: Vec<(String, Attachment)>,
    state: Mutex<State>,
}

impl Thermostat {
    /// Makes a thermostat that has seen nothing and decided nothing.
    ///
    /// # Arguments
    ///
    /// * `name`    - The hysteresis node it stands in for.
    /// * `params`  - That node's setpoint and half band.
    /// * `sensors` - Each sensor it reads, by payload name, with how its
    ///               bytes are read.
    /// * `heaters` - Each heater it switches, by payload name, with what it
    ///               is to the world.
    ///
    /// # Returns
    ///
    /// The thermostat.
    pub fn new(
        name: &str,
        params: HysteresisParams,
        sensors: BTreeMap<String, ToValueParams>,
        heaters: Vec<(String, Attachment)>,
    ) -> Thermostat {
        Thermostat {
            name: name.to_string(),
            setpoint: params.setpoint,
            half_band: params.half_band,
            sensors,
            heaters,
            state: Mutex::new(State::default()),
        }
    }

    /// The hysteresis node it stands in for.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Says whether it reads a sensor.
    ///
    /// # Arguments
    ///
    /// * `sensor` - The sensor's payload name.
    ///
    /// # Returns
    ///
    /// Whether that sensor's readings come here.
    pub fn reads(&self, sensor: &str) -> bool {
        self.sensors.contains_key(sensor)
    }

    /// The heaters it switches.
    ///
    /// # Returns
    ///
    /// Each heater's payload name.
    pub fn heaters(&self) -> impl Iterator<Item = &str> {
        self.heaters.iter().map(|(heater, _)| heater.as_str())
    }

    /// Says whether it switches a heater.
    ///
    /// # Arguments
    ///
    /// * `heater` - The heater's payload name.
    ///
    /// # Returns
    ///
    /// Whether that heater is one of its own.
    pub fn switches(&self, heater: &str) -> bool {
        self.heaters().any(|named| named == heater)
    }

    /// Takes a reading a sensor has just sent, and switches the heaters.
    ///
    /// # Arguments
    ///
    /// * `sensor` - The sensor's payload name; one this thermostat does not
    ///              read is ignored.
    /// * `bytes`  - What it sent.
    ///
    /// # Returns
    ///
    /// What the heaters are now held at, or `None` while nothing has been
    /// decided or when the reading was ignored.
    pub fn sample(&self, sensor: &str, bytes: &[u8]) -> Option<Decision> {
        let reading = self.sensors.get(sensor)?;
        // A reading the conversion would refuse is one the vehicle never
        // acts on.
        let counts = reading.format.counts(bytes).ok()?;
        let kelvin = counts * reading.scale + reading.offset;

        let decision = {
            let mut state =
                self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.latest.insert(sensor.to_string(), kelvin);
            let temperature = median(state.latest.values().copied());
            if let Some(decided) = self.decide(temperature) {
                state.decision = Some(decided);
            }
            state.decision?
        };

        let data: &[u8] = match decision {
            Decision::On => b"1",
            Decision::Off => b"0",
        };
        for (heater, attachment) in &self.heaters {
            driven(&Some(attachment.clone()), heater, data);
        }
        Some(decision)
    }

    /// Decides from one temperature.
    ///
    /// # Arguments
    ///
    /// * `temperature` - In kelvin.
    ///
    /// # Returns
    ///
    /// Off above the band, on below it, and `None` within it.
    fn decide(&self, temperature: f64) -> Option<Decision> {
        if temperature > self.setpoint + self.half_band {
            Some(Decision::Off)
        } else if temperature < self.setpoint - self.half_band {
            Some(Decision::On)
        } else {
            None
        }
    }
}

/// The median of some temperatures: the middle one, or the mean of the
/// middle two of an even number.
///
/// # Arguments
///
/// * `temperatures` - At least one temperature.
///
/// # Returns
///
/// Their median.
fn median(temperatures: impl Iterator<Item = f64>) -> f64 {
    let mut sorted: Vec<f64> = temperatures.collect();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::tests::zone_world;
    use crate::physics::{encode, world::World};
    use std::sync::Arc;
    use std::time::Instant;
    use tcslibgs::node::vehicle::compute::Format;
    use tcslibgs::physics::Place;

    /// The thermostat example's sensor's encoding.
    ///
    /// # Returns
    ///
    /// Sixteenths of a kelvin above freezing, two bytes, signed, most
    /// significant first.
    fn sixteenths() -> ToValueParams {
        ToValueParams {
            format: Format::I16Be,
            scale: 0.0625,
            offset: 273.15,
            unit: tcslibgs::unit::Unit::Kelvin,
        }
    }

    /// A thermostat of the example's figures reading three sensors and
    /// switching one heater of the example's world.
    ///
    /// # Returns
    ///
    /// The thermostat, the world, and the heater's heat.
    fn thermostat() -> (Thermostat, Arc<World>, Place) {
        let (world, place) = zone_world();
        let sensors = ["temp_1", "temp_2", "temp_3"]
            .iter()
            .map(|name| (name.to_string(), sixteenths()))
            .collect();
        let heaters = vec![(
            "heaters_1".to_string(),
            Attachment::Drives {
                world: world.clone(),
                place: place("heater_1.on"),
                unit: None,
                power: Arc::new(crate::physics::Power::new(true, true)),
            },
        )];
        let params = HysteresisParams {
            setpoint: 293.15,
            half_band: 2.0,
        };
        (
            Thermostat::new("thermostat", params, sensors, heaters),
            world,
            place("heater_1.heat"),
        )
    }

    /// What a heater gives out now.
    fn heat(world: &World, heat: Place) -> f64 {
        world.read(heat, Instant::now())
    }

    /// Below the band it switches the heaters on, above it off, and within
    /// it holds what it last decided.
    #[test]
    fn it_holds_the_band() {
        let (thermostat, world, warming) = thermostat();
        let send = |sensor: &str, kelvin: f64| {
            thermostat.sample(sensor, &encode(&sixteenths(), kelvin))
        };

        assert_eq!(send("temp_1", 283.15), Some(Decision::On));
        assert!(heat(&world, warming) > 0.0, "the heater should be on");
        // Within the band: still on.
        assert_eq!(send("temp_1", 293.15), Some(Decision::On));
        assert_eq!(send("temp_1", 296.0), Some(Decision::Off));
        assert_eq!(heat(&world, warming), 0.0, "the heater should be off");
        assert_eq!(send("temp_1", 293.15), Some(Decision::Off));
    }

    /// A heater disabled in its panel stays off whatever the stood-in
    /// thermostat decides, and takes its latest decision when enabled.
    #[test]
    fn a_disabled_heater_ignores_the_stood_in_thermostat() {
        let (thermostat, world, warming) = thermostat();
        let heater = thermostat.heaters[0].1.clone();
        heater.enable(false);
        let send = |sensor: &str, kelvin: f64| {
            thermostat.sample(sensor, &encode(&sixteenths(), kelvin))
        };

        assert_eq!(send("temp_1", 283.15), Some(Decision::On));
        assert_eq!(heat(&world, warming), 0.0, "disabled, it stays off");
        heater.enable(true);
        assert!(heat(&world, warming) > 0.0, "enabled, it takes the on");
        heater.enable(false);
        assert_eq!(heat(&world, warming), 0.0, "disabled again, off");
    }

    /// Nothing is decided until a reading leaves the band.
    #[test]
    fn nothing_is_decided_within_the_band() {
        let (thermostat, world, warming) = thermostat();
        let reading = encode(&sixteenths(), 293.15);
        assert_eq!(thermostat.sample("temp_1", &reading), None);
        assert_eq!(heat(&world, warming), 0.0);
    }

    /// One sensor gone wrong is outvoted by the other two.
    #[test]
    fn the_median_outvotes_one_sensor() {
        let (thermostat, _, _) = thermostat();
        let send = |sensor: &str, kelvin: f64| {
            thermostat.sample(sensor, &encode(&sixteenths(), kelvin))
        };
        send("temp_1", 296.0);
        send("temp_2", 296.0);
        // Far below the band, and alone: the median is still above it.
        assert_eq!(send("temp_3", 200.0), Some(Decision::Off));
    }

    /// A reading the conversion would refuse, or from a payload it does
    /// not read, changes nothing.
    #[test]
    fn a_reading_it_cannot_read_is_ignored() {
        let (thermostat, world, warming) = thermostat();
        assert_eq!(thermostat.sample("temp_1", &[0x00]), None);
        let cold = encode(&sixteenths(), 283.15);
        assert_eq!(thermostat.sample("elsewhere", &cold), None);
        assert_eq!(heat(&world, warming), 0.0);
    }
}
