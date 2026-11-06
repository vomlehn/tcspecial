//! The physical world the simulated payloads measure and act on.
//!
//! Where `node` holds the simulations of the hardware at the far end of each
//! node, this holds what that hardware would sense and change: the components
//! of the vehicle's physics model -- see `tcslibgs::node::physics` -- each a
//! part simulated alone, joined into one [`world::World`] that is advanced from
//! event to event as the simulated payloads ask about it. So what a sensor
//! reports follows from what the vehicle did.
//!
//! A simulated payload is joined to one terminal of the world, which is what
//! its entry's `connects` names: it reports an output, or drives an input. That
//! is its [`Attachment`].

pub mod battery;
pub mod body;
pub mod bus;
pub mod clock;
pub mod component;
pub mod engine;
pub mod heater;
pub mod load;
pub mod queue;
pub mod random;
pub mod sensor;
pub mod solar_array;
pub mod sun;
pub mod thermal_mass;
pub mod world;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use tcslibgs::node::physics::Place;
use tcslibgs::node::vehicle::compute::{from_value, Format, ToValueParams};
use tcslibgs::unit::Unit;

use sensor::Sensor;
use world::World;

/// What one simulated payload is to the world.
///
/// Held by the payload rather than by the world, because it is the
/// payload's business: a sensor's thread asks the world what its terminal
/// is when a packet is due, and a heater's thread tells it when the vehicle
/// drives it.
#[derive(Debug, Clone)]
pub enum Attachment {
    /// It reports this output, from a place this far from what the output
    /// says, in this encoding.
    Reports {
        world: Arc<World>,
        place: Place,
        /// In the output's unit.
        offset: f64,
        /// Taken from the conversion the readings go to, so the vehicle
        /// reads back what the output is: see
        /// `tcslibgs::graph::simulated::reading_of`.
        reading: ToValueParams,
        /// What the sensor gets wrong, and what it has got wrong so far.
        sensor: Arc<Sensor>,
    },
    /// It signals this output, a decision, as the text `1` or `0`: at once
    /// when it changes, and again every packet interval.
    Signals {
        world: Arc<World>,
        place: Place,
        /// What it last said, as the bits of the value: what a change is a
        /// change from. Shared by the copies of its payload's configuration.
        said: Arc<AtomicU64>,
    },
    /// It drives this input to what the vehicle sends it: a decision where
    /// `unit` is `None`, and a value in that unit where it is not -- while
    /// its panel has it enabled: see [`Power`].
    Drives {
        world: Arc<World>,
        place: Place,
        unit: Option<Unit>,
        /// Whether it is enabled, and what the vehicle last sent it.
        /// Shared by the copies of its payload's configuration and by a
        /// thermostat tcssim simulates, so that all of them see the one
        /// panel.
        power: Arc<Power>,
    },
    /// It pulls each of a GPIO payload's lines, in the order its node names
    /// them, to one of these decision outputs: high while the output is on.
    Pulls {
        world: Arc<World>,
        places: Vec<Place>,
    },
}

/// Whether a driven part is enabled in its payload's panel, and what the
/// vehicle last sent it.
///
/// The panel's button is the one control a payload has, and for a part the
/// vehicle drives -- a heater -- it is the part's power: one that is not
/// enabled is unpowered, or has failed, and stays off whatever the vehicle
/// decides, its input held at nought. Nothing aboard changes, and the
/// ground sees it only by what the world does without it: a zone that
/// cools.
///
/// What arrives while it is disabled is still kept, the latest replacing
/// the one before, and enabling it applies that at once: the vehicle sends
/// a decision whenever it decides, but a heater that waited for the next
/// change before going on would sit cold through a whole band. A part that
/// was never sent anything stays at nought.
#[derive(Debug)]
pub struct Power {
    /// Whether it is enabled: the same flag as its payload's
    /// `PayloadConfig::sending`, which the panel's button sets.
    enabled: Arc<AtomicBool>,
    /// Whether what it drives is a heater, which its panel says is heating
    /// rather than on.
    heater: bool,
    /// What it has been sent and what it holds the world's input at.
    held: Mutex<Powered>,
}

/// What a driven part has been sent, and what it holds its input at.
#[derive(Debug, Default, Clone, Copy)]
struct Powered {
    /// What the vehicle last sent, as the value to drive to; `None` until
    /// anything has come.
    latest: Option<f64>,
    /// What the world's input is held at by this part: nought while it is
    /// disabled, and the latest while it is enabled.
    applied: f64,
}

/// What a driven part is doing, as its panel says it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Driving {
    /// Disabled in its panel, so held at nought.
    Disabled,
    /// Enabled, and held at this.
    At(f64),
}

impl Power {
    /// Makes the power of a driven part.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether it starts enabled: false for a payload whose
    ///               panel has not been pressed.
    /// * `heater`  - Whether what it drives is a heater.
    ///
    /// # Returns
    ///
    /// Its power, with nothing yet sent to it.
    pub fn new(enabled: bool, heater: bool) -> Power {
        Power {
            enabled: Arc::new(AtomicBool::new(enabled)),
            heater,
            held: Mutex::new(Powered::default()),
        }
    }

    /// The flag that says whether it is enabled, for its payload's
    /// configuration to share as its `sending`.
    pub fn enabled_flag(&self) -> Arc<AtomicBool> {
        self.enabled.clone()
    }

    /// Locks what it holds, even after a panic elsewhere: what it holds is
    /// two numbers, never half written.
    fn held(&self) -> MutexGuard<'_, Powered> {
        self.held.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Attachment {
    /// Takes what the vehicle sent a driven part, at a simulated time: it is
    /// kept as the latest, and the world's input is driven to it if the
    /// part is enabled. Nothing for any other attachment.
    ///
    /// # Arguments
    ///
    /// * `value`   - What it was sent, as the value to drive to: one for on
    ///               and nought for off, for a decision.
    /// * `seconds` - The simulated time, as [`World::drive_at`] takes it.
    pub fn sent_at(&self, value: f64, seconds: f64) {
        let Attachment::Drives {
            world,
            place,
            power,
            ..
        } = self
        else {
            return;
        };
        // Held across the drive, so that a press of the panel's button and a
        // decision arriving together leave the input where the later says.
        let mut held = power.held();
        held.latest = Some(value);
        if power.enabled.load(Ordering::SeqCst) && held.applied != value {
            held.applied = value;
            world.drive_at(*place, value, seconds);
        }
    }

    /// Enables or disables a driven part, at a simulated time: disabled, its
    /// input is held at nought; enabled, at the latest it was sent, or
    /// nought if it was sent nothing. Nothing for any other attachment.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether it is to be enabled.
    /// * `seconds` - The simulated time, as [`World::drive_at`] takes it.
    pub fn enable_at(&self, enabled: bool, seconds: f64) {
        let Attachment::Drives {
            world,
            place,
            power,
            ..
        } = self
        else {
            return;
        };
        let mut held = power.held();
        power.enabled.store(enabled, Ordering::SeqCst);
        let value = if enabled {
            held.latest.unwrap_or(0.0)
        } else {
            0.0
        };
        // Only a change: the world is not asked anything, and its clock not
        // started, by a part that was off and stays off.
        if held.applied != value {
            held.applied = value;
            world.drive_at(*place, value, seconds);
        }
    }

    /// Enables or disables a driven part now, as its panel's button does:
    /// see [`Attachment::enable_at`].
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether it is to be enabled.
    pub fn enable(&self, enabled: bool) {
        if let Attachment::Drives { world, .. } = self {
            self.enable_at(enabled, world.seconds(Instant::now()));
        }
    }

    /// Says what a driven part is doing.
    ///
    /// # Returns
    ///
    /// Disabled, or what its input is held at; `None` for any other
    /// attachment.
    pub fn driving(&self) -> Option<Driving> {
        let Attachment::Drives { power, .. } = self else {
            return None;
        };
        let held = power.held();
        Some(if power.enabled.load(Ordering::SeqCst) {
            Driving::At(held.applied)
        } else {
            Driving::Disabled
        })
    }

    /// Says what a driven part is doing in words, for its panel: whether a
    /// heater is heating, off, or disabled and so off. Words, so that the
    /// panel says it in more than a colour.
    ///
    /// # Returns
    ///
    /// The part and what it is doing -- `heater_1: heating` -- or empty for
    /// any other attachment.
    pub fn driving_said(&self) -> String {
        let (
            Attachment::Drives {
                world,
                place,
                unit,
                power,
            },
            Some(driving),
        ) = (self, self.driving())
        else {
            return String::new();
        };
        let name = world.name_of(*place);
        let part = name.split('.').next().unwrap_or(&name);
        let doing = match (driving, unit) {
            (Driving::Disabled, _) => "disabled, so off".to_string(),
            (Driving::At(value), None) if value > 0.5 => {
                if power.heater { "heating" } else { "on" }.to_string()
            }
            (Driving::At(_), None) => "off".to_string(),
            (Driving::At(value), Some(unit)) => {
                format!("at {value} {}", unit.symbol())
            }
        };
        format!("{part}: {doing}")
    }

    /// Says whether it drives an input, which a simulated payload that sends
    /// nothing asks.
    pub fn drives(&self) -> bool {
        matches!(self, Attachment::Drives { .. })
    }

    /// Says what it is, for the log.
    ///
    /// # Arguments
    ///
    /// * `payload` - Its payload's name.
    ///
    /// # Returns
    ///
    /// A sentence.
    pub fn describe(&self, payload: &str) -> String {
        match self {
            // Read at simulated time nought, which does not start the
            // clock: the run begins when a simulated payload does.
            Attachment::Reports { world, place, .. } => format!(
                "{payload} reports {}, which starts at {:.2}",
                world.name_of(*place),
                world.read_at(*place, 0.0)
            ),
            Attachment::Signals { world, place, .. } => {
                format!("{payload} signals {}", world.name_of(*place))
            }
            Attachment::Drives { world, place, .. } => {
                format!("{payload} drives {}", world.name_of(*place))
            }
            Attachment::Pulls { world, places } => {
                let names: Vec<String> =
                    places.iter().map(|p| world.name_of(*p)).collect();
                format!("{payload} pulls its lines to {}", names.join(", "))
            }
        }
    }

    /// What a pulling payload's lines are to be now: each high where its
    /// decision is on. Written into `levels`, which its caller made once,
    /// so that asking allocates nothing.
    ///
    /// # Arguments
    ///
    /// * `levels` - One for each line, in the node's order.
    ///
    /// # Returns
    ///
    /// Whether this is a pulling payload: for one of any other kind,
    /// `levels` is left as it was.
    pub fn levels(&self, levels: &mut [bool]) -> bool {
        let Attachment::Pulls { world, places } = self else {
            return false;
        };
        let now = Instant::now();
        for (level, place) in levels.iter_mut().zip(places) {
            *level = world.read(*place, now) > 0.5;
        }
        true
    }

    /// What a reporting or signalling payload sends now: its terminal,
    /// read by its sensor, in its encoding, or a decision as the text `1`
    /// or `0`.
    ///
    /// # Returns
    ///
    /// The bytes, or `None` for one that drives.
    pub fn reading(&self) -> Option<Vec<u8>> {
        let now = Instant::now();
        match self {
            Attachment::Reports {
                world,
                place,
                offset,
                reading,
                sensor,
            } => {
                let value = world.read(*place, now) + offset;
                Some(encode(reading, sensor.read(value, world.seconds(now))))
            }
            Attachment::Signals { world, place, said } => {
                let value = world.read(*place, now);
                said.store(value.to_bits(), Ordering::Relaxed);
                Some(if value > 0.5 { b"1" } else { b"0" }.to_vec())
            }
            Attachment::Drives { .. } | Attachment::Pulls { .. } => None,
        }
    }

    /// Waits until a signalling payload's decision changes from what it last
    /// said, or until a moment, whichever is first.
    ///
    /// What lets the vehicle hear of a change the moment it happens rather than
    /// at the payload's next turn to send: see [`World::await_change`]. A
    /// simulated payload of any other kind is not woken early, and waits out
    /// the time.
    ///
    /// # Arguments
    ///
    /// * `until`   - The moment to stop waiting, on the wall clock.
    /// * `running` - The payload's flag: cleared, the wait ends within a
    ///               twentieth of a second.
    ///
    /// # Returns
    ///
    /// Whether it changed.
    pub fn await_change(&self, until: Instant, running: &AtomicBool) -> bool {
        let Attachment::Signals { world, place, said } = self else {
            return false;
        };
        let from = f64::from_bits(said.load(Ordering::Relaxed));
        while running.load(Ordering::SeqCst) {
            // In slices, so that a payload told to stop does not wait out a
            // packet interval first.
            let slice = (Instant::now() + Duration::from_millis(50)).min(until);
            if world.await_change(*place, from, slice).is_some() {
                return true;
            }
            if Instant::now() >= until {
                return false;
            }
        }
        false
    }
}

/// Tells the world what the vehicle just drove this payload to, if it
/// drives an input.
///
/// What arrives at a heater is the thermostat's decision, which a thermostat
/// node sends as the text `1` or `0`: the value to drive a set of discrete
/// lines to. What arrives at an engine's throttle is a value, four bytes of a
/// count of its unit's steps, as every value on a link is. Anything else is
/// left alone rather than guessed at -- a simulated payload sent something that
/// is not what its input takes is a configuration pointing the wrong link at
/// it, and a simulator that read it as "off" would hide that by behaving
/// plausibly.
///
/// Nothing at all for a payload that drives nothing: the caller does not
/// have to ask first.
///
/// # Arguments
///
/// * `attachment` - What the payload is to the world, if anything.
/// * `which`      - The payload's name, for a complaint.
/// * `data`       - What the vehicle sent it.
pub fn driven(attachment: &Option<Attachment>, which: &str, data: &[u8]) {
    let Some(
        driving @ Attachment::Drives {
            world, place, unit, ..
        },
    ) = attachment
    else {
        return;
    };
    let now = Instant::now();
    let value = match unit {
        Some(unit) => from_value(data, *unit, which),
        None => match data.trim_ascii() {
            b"1" => Ok(1.0),
            b"0" => Ok(0.0),
            _ => Err(format!(
                "{which} was sent {} bytes that are not a decision: a \
                 heater is driven to 1 or 0",
                data.len()
            )),
        },
    };
    match value {
        Ok(value) => driving.sent_at(value, world.seconds(now)),
        Err(why) => eprintln!("{why}, and drives {}", world.name_of(*place)),
    }
}

/// Encodes a value as the bytes a sensor of this encoding would send.
///
/// The inverse of what a conversion does: it reads counts and answers
/// `counts * scale + offset`, so this takes the value back to counts and
/// lays them out in the format the conversion expects.
///
/// Rounded to the nearest count, which is a real sensor's quantisation and
/// worth having in a simulation: the example's parts are a sixteenth of a
/// kelvin and eight thousandths, so a loop that only worked on exact
/// temperatures would be a loop that does not work.
///
/// A value outside what the format can hold is clamped to its ends rather
/// than wrapped. A wrapped count is a sensor reporting the other end of its
/// range, which is a reading an operator would act on; a clamped one is a
/// sensor at the end of its range, which is what a real part does.
///
/// # Arguments
///
/// * `reading` - The conversion the readings go to, which gives the
///               format, scale and offset.
/// * `value`   - The value, in the conversion's unit.
///
/// # Returns
///
/// The sensor's bytes, two or three of them as the format says.
pub fn encode(reading: &ToValueParams, value: f64) -> Vec<u8> {
    let counts = ((value - reading.offset) / reading.scale).round();
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
pub(crate) mod tests {
    use super::*;
    use crate::payload::{PayloadConfig, PayloadProtocol, SimulatedPayload};
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;
    use tcslibgs::node::physics::terminal::At;
    use tcslibgs::node::physics::WorldSection;
    use tcslibgs::sim_config::SensorErrors;
    use tcslibgs::unit::Unit;

    /// Reads a model's components and connections, written as they would be
    /// at the left margin, as the physics of a world whose ambient
    /// temperature is 283.15 K.
    ///
    /// # Arguments
    ///
    /// * `text` - The physics section's value.
    ///
    /// # Returns
    ///
    /// The world.
    ///
    /// # Errors
    ///
    /// What the reader says of it.
    pub(crate) fn parsed(
        text: &str,
    ) -> tcslibgs::error::TcsResult<WorldSection> {
        let physics: String =
            text.lines().map(|line| format!("  {line}\n")).collect();
        WorldSection::parse(&format!(
            "ambient_temp: \"283.15 K\"\nphysics:\n{physics}"
        ))
    }

    /// A model of the thermostat example's shape with bigger heaters: two of
    /// three kelvin a second each on the zone, which is ten kelvin below a
    /// setpoint of 293.15 and loses the difference with a fifty second
    /// constant.
    pub(crate) const ZONE: &str = "components:
  - type heater:
      - component: heater_1
        power: \"60 W\"
        voltage: \"28 V\"
      - component: heater_2
        power: \"60 W\"
        voltage: \"28 V\"
  - type thermal_mass:
      - component: zone
        capacity: \"20 J/K\"
        time_constant: 50 s
connections:
  - from: heater_1.heat
    to: zone.heat
  - from: heater_2.heat
    to: zone.heat
";

    /// The world of [`ZONE`], keeping pace with the wall clock from now,
    /// and a terminal of it by name.
    ///
    /// # Returns
    ///
    /// The world, and what finds a terminal of it from its name, such as
    /// `zone.temperature`.
    pub(crate) fn zone_world() -> (Arc<World>, impl Fn(&str) -> Place) {
        let model = parsed(ZONE)
            .expect("the model parses")
            .resolve()
            .expect("it resolves");
        let world = Arc::new(World::new(
            &model,
            clock::Clock::keeping_pace(Instant::now()),
            0,
        ));
        let place = move |name: &str| {
            model
                .find(&At::parse("test", name).expect("a terminal"))
                .expect("the model has it")
                .0
        };
        (world, place)
    }

    /// The battery example's power model, with its orbit shortened to a
    /// hundred seconds, forty of them in shadow, and a battery of a
    /// hundredth of an ampere-hour, so that a test sees it fill and drain.
    const POWER: &str = "components:
  - type battery:
      - component: battery
        capacity: \"0.01 Ah\"
        charge: \"0.5\"
        empty: \"24 V\"
        full: \"32 V\"
        resistance: \"0.1 ohm\"
  - type bus:
      - component: bus
  - type load:
      - component: avionics
        current: \"2 A\"
      - component: science
        current: \"2 A\"
  - type solar_array:
      - component: array
        current: \"7 A\"
  - type sun:
      - component: sun
        eclipse: \"0.4\"
        period: 100 s
connections:
  - { from: sun.light, to: array.light }
  - { from: array.current, to: bus.supply }
  - { from: avionics.current, to: bus.demand }
  - { from: science.current, to: bus.demand }
  - { from: bus.current, to: battery.offered }
";

    /// The world of a model, keeping pace with the wall clock from now,
    /// and a terminal of it by name.
    ///
    /// # Arguments
    ///
    /// * `text` - The model.
    ///
    /// # Returns
    ///
    /// The world, and what finds a terminal of it from its name.
    fn world_of(text: &str) -> (Arc<World>, impl Fn(&str) -> Place) {
        let model = parsed(text)
            .expect("the model parses")
            .resolve()
            .expect("it resolves");
        let world = Arc::new(World::new(
            &model,
            clock::Clock::keeping_pace(Instant::now()),
            0,
        ));
        let place = move |name: &str| {
            model
                .find(&At::parse("test", name).expect("a terminal"))
                .expect("the model has it")
                .0
        };
        (world, place)
    }

    /// Through an orbit the battery charges in sunlight, is full before the
    /// eclipse and charges no more, discharges in shadow, and has a load
    /// shed from the moment it is driven to be: each change at the moment it
    /// falls, however seldom the world is asked.
    #[test]
    fn a_battery_follows_its_orbit_and_its_loads() {
        let (world, place) = world_of(POWER);
        let (charge, current, volts) = (
            place("battery.charge"),
            place("battery.current"),
            place("battery.voltage"),
        );

        // In sunlight: seven amperes in, four out, three into the battery,
        // whose half of 36 coulombs of room fills in six seconds.
        assert!((world.read_at(current, 1.0) - 3.0).abs() < 1e-12);
        assert!((world.read_at(charge, 3.0) - 0.75).abs() < 1e-12);
        // Asked about once, long after: full at six seconds, and nothing
        // has flowed in since, the charging current being shunted.
        assert_eq!(world.read_at(charge, 59.0), 1.0);
        assert_eq!(world.read_at(current, 59.0), 0.0);
        assert_eq!(world.read_at(volts, 59.0), 32.0);

        // Into shadow at sixty seconds: four amperes out of it.
        assert!((world.read_at(current, 61.0) + 4.0).abs() < 1e-12);
        let after = world.read_at(charge, 64.0);
        assert!((after - (1.0 - 4.0 * 4.0 / 36.0)).abs() < 1e-9, "{after}");

        // Science shed: half the drain, from that moment.
        world.drive_at(place("science.shed"), 1.0, 64.0);
        assert!((world.read_at(current, 65.0) + 2.0).abs() < 1e-12);
        let shed = world.read_at(charge, 68.0);
        assert!((shed - (after - 2.0 * 4.0 / 36.0)).abs() < 1e-9, "{shed}");

        // Out of shadow at a hundred, and charging again.
        assert!((world.read_at(current, 101.0) - 5.0).abs() < 1e-12);
    }

    /// A lander two metres up and falling at one metre a second, its engine
    /// joined and shut.
    const LANDER: &str = "components:
  - type body:
      - component: lander
        altitude: \"2 m\"
        gravity: \"1.62 m/s2\"
        mass: \"1500 kg\"
        velocity: \"-1 m/s\"
  - type engine:
      - component: engine
        fuel: \"800 kg\"
        isp: 311 s
        thrust: \"16000 N\"
connections:
  - { from: engine.force, to: lander.force }
  - { from: engine.flow, to: lander.flow }
";

    /// A simulated payload waiting on the lander's contact is woken at
    /// touchdown -- the moment the fall reaches the ground, with nothing else
    /// asking the world anything -- and not before.
    #[test]
    fn a_waiting_simulated_payload_is_woken_at_touchdown() {
        let started = Instant::now();
        let (world, place) = world_of(LANDER);
        let contact = Attachment::Signals {
            world: world.clone(),
            place: place("lander.contact"),
            said: Arc::new(AtomicU64::new(0)),
        };
        assert_eq!(contact.reading(), Some(b"0".to_vec()));
        let running = AtomicBool::new(true);
        assert!(
            contact.await_change(started + Duration::from_secs(5), &running)
        );
        let woken = started.elapsed().as_secs_f64();
        // 2 - t - 0.81 t^2 = 0.
        let landed = (-1.0 + (1.0f64 + 4.0 * 0.81 * 2.0).sqrt()) / 1.62;
        assert!(
            woken >= landed && woken < landed + 0.2,
            "woken at {woken} s, landed at {landed} s"
        );
        assert_eq!(contact.reading(), Some(b"1".to_vec()));
    }

    /// The vehicle's throttle, a value of four bytes, drives the engine,
    /// whose thrust brakes the lander; something that is not a value drives
    /// nothing.
    #[test]
    fn a_value_from_the_vehicle_drives_the_engine() {
        let (world, place) = world_of(LANDER);
        let throttle = Some(Attachment::Drives {
            world: world.clone(),
            place: place("engine.throttle"),
            unit: Some(Unit::Ratio),
            power: Arc::new(Power::new(true, false)),
        });
        let force = place("engine.force");
        let half =
            tcslibgs::node::vehicle::compute::value_bytes(0.5, Unit::Ratio)
                .expect("a ratio");
        driven(&throttle, "engine", &half);
        assert!((world.read(force, Instant::now()) - 8000.0).abs() < 1e-6);
        driven(&throttle, "engine", b"1");
        assert!(
            (world.read(force, Instant::now()) - 8000.0).abs() < 1e-6,
            "one byte is not a value, and changes nothing"
        );
    }

    /// Two suns on one array is an array given two lights, which it takes
    /// one of: refused, naming both.
    #[test]
    fn an_input_that_takes_one_output_is_given_one() {
        let text = POWER.replace(
            "connections:\n",
            "      - component: moon\n        period: 100 s\n        \
             eclipse: \"0.5\"\nconnections:\n  - { from: moon.light, to: \
             array.light }\n",
        );
        let problems = parsed(&text).expect("parses").problems();
        assert!(
            problems.iter().any(|p| p
                .why
                .contains("array.light takes one output, and is given")),
            "{problems:?}"
        );
    }

    /// Heaters joined to a zone warm it, two twice as fast, and the same
    /// heater driven on twice is one heater.
    #[test]
    fn heaters_warm_their_zone_and_count_once_each() {
        let (world, place) = zone_world();
        let (on_1, on_2) = (place("heater_1.on"), place("heater_2.on"));
        let temperature = place("zone.temperature");
        let warmed = |from: f64| {
            let before = world.read_at(temperature, from);
            world.read_at(temperature, from + 0.01) - before
        };

        // At the environment, where there is nothing to lose, one heater
        // gains very nearly its three kelvin a second.
        world.drive_at(on_1, 1.0, 0.0);
        assert!((warmed(0.0) / 0.01 - 3.0).abs() < 0.01);
        // Driven on again, which a vehicle does on every sample: still one.
        world.drive_at(on_1, 1.0, 0.01);
        assert!((warmed(0.01) / 0.01 - 3.0).abs() < 0.01);
        world.drive_at(on_2, 1.0, 0.02);
        assert!((warmed(0.02) / 0.01 - 6.0).abs() < 0.02);
        world.drive_at(on_1, 0.0, 0.03);
        world.drive_at(on_2, 0.0, 0.03);
        assert!(warmed(0.03) < 0.0, "off, it cools");
    }

    /// A heater disabled in its panel stays off whatever it is sent, and
    /// keeps the latest it was sent; enabled, it takes that at once, and
    /// disabled again it goes off at once. Its panel says which, in words.
    #[test]
    fn a_disabled_heater_stays_off_and_takes_the_latest_when_enabled() {
        let (world, place) = zone_world();
        let heat = place("heater_1.heat");
        let heater = Attachment::Drives {
            world: world.clone(),
            place: place("heater_1.on"),
            unit: None,
            power: Arc::new(Power::new(false, true)),
        };
        let heating = |at: f64| world.read_at(heat, at) > 0.0;
        assert_eq!(heater.driving(), Some(Driving::Disabled));
        assert_eq!(heater.driving_said(), "heater_1: disabled, so off");

        // Sent on, and off, and on again while disabled: nothing heats.
        for (at, value) in [(1.0, 1.0), (2.0, 0.0), (3.0, 1.0)] {
            heater.sent_at(value, at);
            assert!(!heating(at + 0.5), "disabled, at {at}");
        }

        // Enabled, it takes the last of those at once.
        heater.enable_at(true, 4.0);
        assert!(heating(4.1), "the latest, on, at once");
        assert_eq!(heater.driving(), Some(Driving::At(1.0)));
        assert_eq!(heater.driving_said(), "heater_1: heating");
        heater.sent_at(0.0, 5.0);
        assert!(!heating(5.1));
        assert_eq!(heater.driving_said(), "heater_1: off");
        heater.sent_at(1.0, 6.0);
        assert!(heating(6.1));

        // Disabled while on, it goes off at once.
        heater.enable_at(false, 7.0);
        assert!(!heating(7.1), "disabled while heating");

        // One never sent anything stays off when enabled.
        let other = Attachment::Drives {
            world: world.clone(),
            place: place("heater_2.on"),
            unit: None,
            power: Arc::new(Power::new(false, true)),
        };
        other.enable_at(true, 8.0);
        assert!(world.read_at(place("heater_2.heat"), 8.1) == 0.0);
        assert_eq!(other.driving_said(), "heater_2: off");
    }

    /// The payload's flag and the heater's power are one: the panel's
    /// button, through `SimulatedPayload::send`, powers the heater, and a
    /// decision arriving over its socket is held off while it is disabled.
    #[test]
    fn a_heaters_panel_button_is_its_power() {
        let (world, place) = zone_world();
        let heat = place("heater_1.heat");
        let power = Arc::new(Power::new(false, true));
        let attachment = Attachment::Drives {
            world: world.clone(),
            place: place("heater_1.on"),
            unit: None,
            power: power.clone(),
        };
        let mut config =
            attached("heaters_1", a_free_port(), 1, 20, attachment.clone());
        config.sending = power.enabled_flag();
        let payload = SimulatedPayload::new(config);
        let attachment = Some(attachment);

        driven(&attachment, "heaters_1", b"1");
        assert_eq!(
            attachment.as_ref().and_then(Attachment::driving),
            Some(Driving::Disabled)
        );
        assert_eq!(world.read(heat, Instant::now()), 0.0);
        payload.send(true);
        assert!(world.read(heat, Instant::now()) > 0.0, "on at once");
        assert_eq!(payload.driving_said(), "heater_1: heating");
        payload.send(false);
        assert_eq!(world.read(heat, Instant::now()), 0.0, "off at once");
        assert_eq!(payload.driving_said(), "heater_1: disabled, so off");
    }

    /// A heater switched on halfway through an interval warms its zone for
    /// half of it, however the interval is asked about.
    #[test]
    fn a_heater_warms_from_the_moment_it_is_switched() {
        let at = |reads: u32| {
            let (world, place) = zone_world();
            let temperature = place("zone.temperature");
            world.drive_at(place("heater_1.on"), 1.0, 5.0);
            for i in 1..=reads {
                world.read_at(
                    temperature,
                    10.0 * f64::from(i) / f64::from(reads),
                );
            }
            world.read_at(temperature, 10.0)
        };
        // Five seconds at three a second against a fifty second constant.
        let rest = 283.15 + 3.0 * 50.0;
        let exact = rest + (283.15 - rest) * (-5.0f64 / 50.0).exp();
        for reads in [1, 7, 1000] {
            assert!((at(reads) - exact).abs() < 1e-9, "{reads} reads");
        }
    }

    /// A temperature encodes as the bytes the vehicle reads it back from.
    ///
    /// Against the example's two parts, in their own formats and scales:
    /// this is the one piece of arithmetic in the simulator whose answer
    /// another program checks, so what it is held to is the round trip.
    #[test]
    fn a_value_encodes_as_the_bytes_the_vehicle_reads_back() {
        let parts = [
            // temp_1 and temp_2: two bytes, a sixteenth of a kelvin a
            // count, above freezing.
            ToValueParams {
                format: Format::I16Be,
                scale: 0.0625,
                offset: 273.15,
                unit: Unit::Kelvin,
            },
            // temp_3: three bytes, eight thousandths of a kelvin, its own
            // offset.
            ToValueParams {
                format: Format::U24Be,
                scale: 0.0078,
                offset: 255.37,
                unit: Unit::Kelvin,
            },
        ];

        for reading in parts {
            for kelvin in [283.15, 291.15, 293.15, 295.15, 300.0] {
                let bytes = encode(&reading, kelvin);
                let back = reading.format.counts(&bytes).expect("counts")
                    * reading.scale
                    + reading.offset;
                // Within half a count, which is the sensor's quantisation
                // and not an error in the arithmetic.
                assert!(
                    (back - kelvin).abs() <= reading.scale / 2.0 + 1e-9,
                    "{kelvin} K encoded as {bytes:?} reads back as {back}"
                );
            }
        }

        // And a value the format cannot hold is clamped, not wrapped: a
        // wrapped count is a sensor reporting the other end of its range.
        let part = ToValueParams {
            format: Format::U16Be,
            scale: 1.0,
            offset: 0.0,
            unit: Unit::Kelvin,
        };
        assert_eq!(encode(&part, -50.0), vec![0x00, 0x00]);
        assert_eq!(encode(&part, 1e9), vec![0xFF, 0xFF]);
    }

    /// A free port, for a payload that is the end which waits.
    ///
    /// A heater binds the address the configuration gives, because a device
    /// the vehicle drives is the end that has to be findable, so the test
    /// has to name one that is free rather than take whatever a socket of
    /// its own was given.
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

    /// Configures a simulated payload of the thermostat example, joined to
    /// the world.
    ///
    /// # Arguments
    ///
    /// * `name`       - The payload's name.
    /// * `at`         - The loopback UDP port it uses.
    /// * `bytes`      - Its packet size, and its segment size too.
    /// * `every`      - The packet interval in ms: for a sensor how often it
    ///                  reports, and for a heater -- which sends nothing --
    ///                  how often it looks for what the vehicle decided.
    /// * `attachment` - What it is to the world.
    ///
    /// # Returns
    ///
    /// Its configuration, sending from the start.
    fn attached(
        name: &str,
        at: u16,
        bytes: u32,
        every: u32,
        attachment: Attachment,
    ) -> PayloadConfig {
        PayloadConfig {
            attachment: Some(attachment),
            thermostats: Vec::new(),
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

    /// The example's thermostat, as the vehicle's own nodes would decide it.
    ///
    /// The median of the three readings, and a heat decision that turns on
    /// below the band and off above it -- `vote` and `thermostat`, whose own
    /// arithmetic is tested where it lives. Here they play the
    /// vehicle, so that what is tested is the loop: the world, the encoding,
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
    /// Faster than the sensors sample, and for a reason the overshoot
    /// shows: a decision is only as prompt as the slower of the two, so a
    /// heater that looked every sample would double the time the vehicle
    /// spends heating a zone that has already arrived.
    const LOOK_MS: u32 = 20;
    /// What each heater adds, in kelvin a second: sixty watts on twenty
    /// joules a kelvin.
    const HEATER_RATE: f64 = 3.0;

    /// The loop closes: the zone the sensors report is warmed by what the
    /// vehicle decided, the band is held, and a lost decision is corrected.
    ///
    /// Everything here is the real thing but the vehicle: three simulated
    /// sensors and two simulated heaters, each a payload thread of its own
    /// sending and receiving over real sockets, joined to one world. The
    /// vehicle's part is [`decide`], which is what its `vote` and
    /// `thermostat` nodes come to.
    ///
    /// It is the one test that can fail for a reason outside this file -- a
    /// loaded machine delays a packet, and a late sample is a late decision
    /// -- so what it holds is the shape of the answer and not a trajectory:
    /// the zone starts cold, reaches the band, and from then on stays inside
    /// it within one sample of heating. A bound of one sample is the
    /// physics: the vehicle cannot switch heaters off between two readings,
    /// so the most it can overshoot by is what they add in the interval it
    /// did not look.
    #[test]
    fn the_heaters_hold_the_zone_in_band_through_a_lost_decision() {
        let (world, place) = zone_world();
        let temperature = place("zone.temperature");

        // The two encodings of the example, so that what is voted over is
        // readings of one temperature in different formats.
        let two_byte = ToValueParams {
            format: Format::I16Be,
            scale: 0.0625,
            offset: 273.15,
            unit: Unit::Kelvin,
        };
        let three_byte = ToValueParams {
            format: Format::U24Be,
            scale: 0.0078,
            offset: 255.37,
            unit: Unit::Kelvin,
        };

        // The vehicle's end of each sensor link, which is the end that
        // waits: a sensor speaks first and this is where it speaks to.
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
                Attachment::Reports {
                    world: world.clone(),
                    place: temperature,
                    offset: 0.0,
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
        // Their ports are asked for by binding and letting go, so a port
        // that was free when it was chosen may be taken by the time the
        // heater binds it -- which would leave the loop open at one end. So
        // the pair is asked for again: a port that was free a moment ago is
        // not a port that is free, and a heater that lost one says so in its
        // own words on the way past.
        //
        // Waiting for each to be listening is what tells a lost port from a
        // slow start. Without it a heater that never bound would show up at
        // the end of this test as a zone that would not warm, which says
        // nothing about why.
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
                    Attachment::Drives {
                        world: world.clone(),
                        place: place(&format!("heater_{}.on", n + 1)),
                        unit: None,
                        power: Arc::new(Power::new(true, true)),
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
                "ten pairs of ports were taken between being chosen and \
                 being bound"
            );
        }

        // The vehicle's end of the heater links: it sends where the file
        // says the heater is, which is what `reaching_endpoint_pair` does
        // aboard.
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
                    if let Ok(counts) = reading.format.counts(&buffer[..got]) {
                        latest[n] =
                            Some(counts * reading.scale + reading.offset);
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
                // the thermostat node asserts its decision each time a
                // temperature arrives and the devices are held at it.
                //
                // The first word to switch them off is thrown away, as a
                // lost datagram throws one away. That is the failure this
                // arrangement exists for: a decision sent once and left to
                // stand would leave the heaters on with the vehicle
                // believing otherwise, and the zone with no bound on where
                // it goes.
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

        // It started cold and the heaters brought it to the band. Six
        // kelvin a second over eight kelvin is under two seconds, and the
        // bound here is loose enough for a loaded machine to be slow at it.
        let reached = reached.unwrap_or_else(|| {
            panic!(
                "the zone never reached the band: it is at {} K, and the \
                 vehicle {} driving the heaters",
                world.read(temperature, Instant::now()),
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
        // band crossed until the next sample, the heaters do not hear about
        // it until they next look, and the first word to switch them off was
        // thrown away -- so one more sample of heating before the next
        // assertion corrects it. A tenth of a kelvin on top of that is the
        // scheduler, which is a thirtieth of the interval and not a
        // trajectory this test is holding anybody to.
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
