//! A setpoint, a dead band, and a decision.
//!
//! The end of the thermostat: a temperature arrives, and this says whether the
//! heaters should be on. More than `half_band` over the setpoint, off; more
//! than `half_band` under it, on; between the two, nothing at all.
//!
//! The band in the middle is the point of it. A comparison against a bare
//! setpoint switches on and off every time the temperature wobbles across one
//! number, which on a heater circuit is a relay cycling several times a minute
//! and a vehicle's power draw oscillating. Two thresholds with a gap between
//! them mean the decision only changes when the temperature has really moved.
//!
//! # What goes out, and what is recorded
//!
//! Two different things, and they used to be one. This emitted only when the
//! decision *changed*, the argument being that a node re-sending the same
//! command would fill the telemetry log with a decision nobody made and leave
//! a reader unable to see when the heaters actually switched.
//!
//! That argument is about the log, and it was applied to the link. The link
//! cannot afford it. A heater's state lived in one place -- the last decision
//! here -- and was asserted only on a change, with nothing reading back what
//! the heater was doing, so a single lost datagram stood for ever: the
//! vehicle believing it had switched the heaters off, the mass climbing past
//! the top of the band, and this deciding `Off` again, finding it had already
//! said so, and staying silent. The decision that would correct it is the one
//! it thinks it has already sent. One lost packet, a mass with no bound on
//! where it goes.
//!
//! So the decision is now asserted on every sample -- the devices are held at
//! what the vehicle last decided, and a lost datagram costs one sample
//! instead of the mission -- and a re-assertion reaches the devices alone.
//! Nothing records it: a log and a ground link still see the transitions and
//! nothing else, which is what the argument above was really asking for. The
//! state it keeps is still the last decision; what changed is that silence is
//! no longer how it is held.

use tcslibgs::node::vehicle::compute::HysteresisParams;

use super::{from_text, from_value, Function};

/// What the heaters should be doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Asserted: the heaters are on.
    On,
    /// Unasserted: the heaters are off.
    Off,
}

impl Decision {
    /// What goes along the link.
    ///
    /// `1` and `0`, which is what a set of discrete lines takes: the value to
    /// drive them to. Text, like every other value on a link, so a telemetry
    /// log shows the decision rather than a byte.
    fn as_value(self) -> Vec<u8> {
        match self {
            Decision::On => b"1".to_vec(),
            Decision::Off => b"0".to_vec(),
        }
    }
}

/// A setpoint and how far either side of it the decision changes.
#[derive(Debug)]
pub struct Hysteresis {
    setpoint: f64,
    /// Half the band's width, the setpoint being in the middle of it.
    half_band: f64,
    /// The last decision, or nothing before the first one.
    ///
    /// Nothing, and not Off: a vehicle that comes up cold must decide On on
    /// the first reading, and one that starts at Off would be silent because
    /// the decision had not changed.
    last: Option<Decision>,
}

impl Hysteresis {
    /// The thermostat these settled parameters describe.
    ///
    /// Nothing to refuse: that a band must be more than nought is
    /// `tcslibgs::node::vehicle::compute`'s rule, where the ground tool applies it.
    /// What is refused here is a *command* that would break it -- see
    /// [`Hysteresis::set`] -- because a command arrives after the checking.
    pub fn of(params: HysteresisParams) -> Hysteresis {
        Hysteresis {
            setpoint: params.setpoint,
            half_band: params.half_band,
            last: None,
        }
    }

    /// Set the setpoint, which is what a command from the ground does.
    ///
    /// The decision is not recomputed here and the last one is kept: the next
    /// reading will decide, one sample away, and acting on a setpoint before
    /// measuring against it would switch the heaters on the strength of a
    /// number rather than a temperature.
    pub fn set_setpoint(&mut self, setpoint: f64) {
        self.setpoint = setpoint;
    }

    pub fn setpoint(&self) -> f64 {
        self.setpoint
    }

    /// What this temperature calls for, whatever was decided before.
    fn decide(&self, temperature: f64) -> Option<Decision> {
        if temperature > self.setpoint + self.half_band {
            Some(Decision::Off)
        } else if temperature < self.setpoint - self.half_band {
            Some(Decision::On)
        } else {
            // In the band: whatever is being done now goes on being done.
            None
        }
    }
}

impl Function for Hysteresis {
    fn give(&mut self, _input: Option<&str>, data: &[u8]) -> Result<Option<Vec<u8>>, String> {
        let temperature = from_value(data, "the temperature")?;

        let Some(wanted) = self.decide(temperature) else {
            return Ok(None);
        };
        if self.last == Some(wanted) {
            // Already doing it, so there is nothing new to pass on. What
            // holds the devices at it is `held`, which the port asks on
            // every arrival that produced nothing: the decision reaches them
            // again and reaches nothing that records it.
            return Ok(None);
        }

        self.last = Some(wanted);
        Ok(Some(wanted.as_value()))
    }

    /// The decision the heaters are held at, until it changes.
    ///
    /// Nothing before the first one: a thermostat that has decided nothing
    /// has nothing to hold its heaters at, and asserting `Off` because it had
    /// not yet looked would be a decision nobody made. A reading in the band
    /// before any decision answers this way too, which is the one case where
    /// a sample produces nothing at all.
    fn held(&self) -> Option<Vec<u8>> {
        self.last.map(Decision::as_value)
    }

    /// The setpoint and the band, which are what the ground commands.
    ///
    /// The same rules as the configuration's: a band of nought or less is
    /// refused here as it is there, because a bare comparison cycles a relay
    /// on a wobble whether it was written in a file or sent from the ground.
    /// Nothing is half applied -- the value is read and checked before
    /// anything changes -- so a command that is refused leaves a thermostat
    /// running on what it had.
    fn set(&mut self, attribute: &str, value: &[u8]) -> Result<(), String> {
        // Text, where a reading is four bytes of micro-kelvin: a command's value
        // is what an operator typed, and it travels as the characters they
        // typed rather than as a number the ground station assembled. See
        // `super::from_text`.
        let said = std::str::from_utf8(value)
            .map_err(|_| format!("{attribute}: what the ground sent is not text"))?;
        let wanted = from_text(said, attribute)?;

        match attribute {
            "setpoint" => self.set_setpoint(wanted),
            "half_band" => {
                if wanted <= 0.0 {
                    return Err(format!(
                        "half_band: {wanted} is not more than nought, and a band \
                         of nothing is a bare comparison, which cycles a relay on \
                         a wobble"
                    ));
                }
                self.half_band = wanted;
            }
            // The two the band used to be, told the name it has now rather
            // than refused as unknown: a file or a command written against
            // the older vocabulary is a thing to correct, and the correction
            // is the one fact the sender needs.
            retired @ ("above" | "below") => {
                return Err(format!(
                    "\"{retired}\" is now half_band, one width with the setpoint in \
                     the middle of it"
                ))
            }
            other => {
                return Err(format!(
                    "a hysteresis has nothing called \"{other}\" to set: there are \
                     setpoint and half_band"
                ))
            }
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "hysteresis"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The free function beside `Decision::as_value`: that method makes a
    // decision, and this makes a temperature.
    use crate::node::vehicle::compute::as_value;

    /// A temperature as it arrives on a link: four bytes of micro-kelvin.
    ///
    /// The refusal is `as_value`'s range, which these temperatures are all
    /// well inside; a test that meant to try the range says so.
    fn kelvin(number: f64) -> Vec<u8> {
        as_value(number).expect("a temperature a value carries")
    }

    fn thermostat() -> Hysteresis {
        Hysteresis::of(HysteresisParams {
            setpoint: 293.15,
            half_band: 2.0,
        })
    }

    /// Cold turns the heaters on; hot turns them off.
    #[test]
    fn the_decision_follows_the_temperature() {
        let mut h = thermostat();

        // More than two degrees under: on.
        assert_eq!(h.give(None, &kelvin(290_f64)).unwrap(), Some(b"1".to_vec()));
        // More than two degrees over: off.
        assert_eq!(h.give(None, &kelvin(296_f64)).unwrap(), Some(b"0".to_vec()));
    }

    /// Inside the band, nothing happens.
    ///
    /// The reason the band exists: a bare comparison switches a relay every
    /// time the temperature wobbles across one number.
    #[test]
    fn nothing_happens_inside_the_band() {
        let mut h = thermostat();

        for wobble in [293.15, 294.0, 292.2, 295.1, 291.2] {
            assert_eq!(
                h.give(None, &kelvin(wobble)).unwrap(),
                None,
                "{wobble} is inside the band and should decide nothing"
            );
        }
    }

    /// A decision is sent once, not on every reading.
    ///
    /// A node that re-sent the same command every three seconds would fill
    /// the log with decisions nobody made, and a reader could not see when
    /// the heaters actually switched.
    #[test]
    fn only_a_change_is_sent() {
        let mut h = thermostat();

        assert_eq!(h.give(None, &kelvin(280_f64)).unwrap(), Some(b"1".to_vec()));
        // Still cold, and still on: nothing to say.
        for _ in 0..10 {
            assert_eq!(h.give(None, &kelvin(281_f64)).unwrap(), None);
        }
        // Now hot: one command.
        assert_eq!(h.give(None, &kelvin(300_f64)).unwrap(), Some(b"0".to_vec()));
        assert_eq!(h.give(None, &kelvin(301_f64)).unwrap(), None);
        // And cold again.
        assert_eq!(h.give(None, &kelvin(280_f64)).unwrap(), Some(b"1".to_vec()));
    }

    /// A vehicle that comes up cold decides on its first reading.
    ///
    /// Which is why the last decision starts as nothing rather than as Off: a
    /// thermostat that began at Off would be silent on a cold start, because
    /// the decision had not changed.
    #[test]
    fn a_cold_start_decides_immediately() {
        let mut h = thermostat();
        assert_eq!(
            h.give(None, &kelvin(250_f64)).unwrap(),
            Some(b"1".to_vec()),
            "a cold vehicle should switch the heaters on at once"
        );
    }

    /// The ground can move the setpoint, and the next reading acts on it.
    #[test]
    fn the_setpoint_can_be_moved_from_the_ground() {
        let mut h = thermostat();
        assert_eq!(h.give(None, &kelvin(290_f64)).unwrap(), Some(b"1".to_vec()));

        // Ask for a much colder cabin: 290 is now too warm.
        h.set_setpoint(280.0);
        assert_eq!(h.setpoint(), 280.0);
        assert_eq!(
            h.give(None, &kelvin(290_f64)).unwrap(),
            Some(b"0".to_vec()),
            "the same temperature should now call for the heaters off"
        );
    }

    /// Moving the setpoint does not itself decide anything.
    ///
    /// The next reading decides, which is at most one interval away. Acting on
    /// the number alone would switch the heaters on the strength of a command
    /// rather than of a temperature.
    #[test]
    fn moving_the_setpoint_waits_for_a_reading() {
        let mut h = thermostat();
        h.give(None, &kelvin(290_f64)).unwrap();
        let before = h.give(None, &kelvin(291_f64)).unwrap();
        assert_eq!(before, None);

        h.set_setpoint(250.0);
        // Nothing was emitted by the setting itself; the state is unchanged
        // until something is measured.
        assert_eq!(h.setpoint(), 250.0);
    }

    /// A band of nothing is refused where a file is read, not here.
    ///
    /// `tcslibgs::node::vehicle::compute` applies that rule, so the ground tool
    /// refuses such a node. What this refuses is a *command* that would make
    /// one, which arrives long after the checking: see the setting test
    /// below.
    #[test]
    fn a_band_of_nothing_is_refused_before_it_is_built() {
        use tcslibgs::node::vehicle::compute::params_of;

        for half_band in ["0", "-1"] {
            let said = params_of(
                "thermostat",
                &[
                    ("function", "hysteresis"),
                    ("setpoint", "293"),
                    ("half_band", half_band),
                ]
                .iter()
                .map(|(k, v)| (k.to_string(), tcslibgs::Scalar::of(*v)))
                .collect(),
            )
            .expect_err("a band of nothing");
            assert!(said.contains("more than nought"), "{said}");
        }
    }

    /// A datum that is not a temperature is refused.
    ///
    /// What makes it refusable is its width: four bytes are a count of
    /// micro-kelvin whatever they hold, so a four-character word would be
    /// read as a temperature and decided on. What a link can deliver here
    /// that is not a reading -- a sensor's own bytes, a decision, a line of
    /// text -- is almost never four bytes long, and the one that is would be
    /// caught by nothing. See `super::from_value`.
    #[test]
    fn a_reading_that_is_not_a_temperature_is_refused() {
        let mut h = thermostat();
        let said = h.give(None, b"cold enough").expect_err("not a value");
        assert!(said.contains("temperature"), "{said}");
    }

    /// What the heaters are held at between decisions.
    ///
    /// The thermostat says nothing new while its decision stands, and the
    /// devices are driven to it all the same -- so what it is holding them at
    /// has to be askable. A thermostat that has decided nothing holds nothing:
    /// asserting `Off` before it had looked would be a decision nobody made.
    #[test]
    fn it_holds_the_heaters_at_the_last_decision() {
        let mut h = Hysteresis::of(HysteresisParams {
            setpoint: 293.15,
            half_band: 2.0,
        });

        assert_eq!(h.held(), None, "nothing is decided before the first reading");

        // In the band, and still nothing decided: there is nothing to hold
        // them at yet, which is the one sample that drives nothing at all.
        assert_eq!(h.give(None, &kelvin(293.0_f64)).unwrap(), None);
        assert_eq!(h.held(), None);

        // Cold: a decision, and from here on that is what they are held at.
        assert_eq!(h.give(None, &kelvin(290.0_f64)).unwrap(), Some(b"1".to_vec()));
        assert_eq!(h.held(), Some(b"1".to_vec()));

        // Still cold: nothing new to send, and the same thing to hold them
        // at. The two together are what puts the decision on the wire every
        // sample without putting it in the log every sample.
        assert_eq!(h.give(None, &kelvin(290.0_f64)).unwrap(), None);
        assert_eq!(h.held(), Some(b"1".to_vec()));

        // Back inside the band: the decision stands, because that is what a
        // band is for.
        assert_eq!(h.give(None, &kelvin(293.0_f64)).unwrap(), None);
        assert_eq!(h.held(), Some(b"1".to_vec()));

        // And over the top of it: a new decision, held from now on.
        assert_eq!(h.give(None, &kelvin(296.0_f64)).unwrap(), Some(b"0".to_vec()));
        assert_eq!(h.held(), Some(b"0".to_vec()));
    }
}

