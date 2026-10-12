//! A setpoint, a dead band, and a decision.
//!
//! The end of the thermostat: a temperature arrives, and this says whether the
//! heaters should be on. More than `above` degrees over the setpoint, off;
//! more than `below` degrees under it, on; between the two, nothing at all.
//!
//! The band in the middle is the point of it. A comparison against a bare
//! setpoint switches on and off every time the temperature wobbles across one
//! number, which on a heater circuit is a relay cycling several times a minute
//! and a vehicle's power draw oscillating. Two thresholds with a gap between
//! them mean the decision only changes when the temperature has really moved.
//!
//! It emits only when the decision *changes*. A node that re-sent the same
//! command every three seconds would work, and would fill the telemetry log
//! with a decision nobody made and leave a reader unable to see when the
//! heaters actually switched. So the state it keeps is the last decision, and
//! silence means nothing has changed.

use std::collections::BTreeMap;

use tcslibgs::Scalar;

use super::{from_value, number, Function};

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

/// A setpoint and the band either side of it.
#[derive(Debug)]
pub struct Hysteresis {
    setpoint: f64,
    above: f64,
    below: f64,
    /// The last decision, or nothing before the first one.
    ///
    /// Nothing, and not Off: a vehicle that comes up cold must decide On on
    /// the first reading, and one that starts at Off would be silent because
    /// the decision had not changed.
    last: Option<Decision>,
}

impl Hysteresis {
    pub fn of(attributes: &BTreeMap<String, Scalar>) -> Result<Hysteresis, String> {
        let above = number(attributes, "above")?;
        let below = number(attributes, "below")?;

        if above <= 0.0 || below <= 0.0 {
            return Err(
                "above and below are how far from the setpoint the decision \
                 changes, and must be more than nought: a band of nothing is a \
                 bare comparison, which cycles a relay on a wobble"
                    .to_string(),
            );
        }

        Ok(Hysteresis {
            setpoint: number(attributes, "setpoint")?,
            above,
            below,
            last: None,
        })
    }

    /// Set the setpoint, which is what a command from the ground does.
    ///
    /// The decision is not recomputed here and the last one is kept: the next
    /// reading will decide, three seconds away at most, and acting on a
    /// setpoint before measuring against it would switch the heaters on the
    /// strength of a number rather than a temperature.
    pub fn set_setpoint(&mut self, setpoint: f64) {
        self.setpoint = setpoint;
    }

    pub fn setpoint(&self) -> f64 {
        self.setpoint
    }

    /// What this temperature calls for, whatever was decided before.
    fn decide(&self, temperature: f64) -> Option<Decision> {
        if temperature > self.setpoint + self.above {
            Some(Decision::Off)
        } else if temperature < self.setpoint - self.below {
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
            // Already doing it. Silence rather than the same command again:
            // see the module comment.
            return Ok(None);
        }

        self.last = Some(wanted);
        Ok(Some(wanted.as_value()))
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
        let wanted = from_value(value, attribute)?;

        match attribute {
            "setpoint" => self.set_setpoint(wanted),
            "above" | "below" => {
                if wanted <= 0.0 {
                    return Err(format!(
                        "{attribute}: {wanted} is not more than nought, and a band \
                         of nothing is a bare comparison, which cycles a relay on \
                         a wobble"
                    ));
                }
                if attribute == "above" {
                    self.above = wanted;
                } else {
                    self.below = wanted;
                }
            }
            other => {
                return Err(format!(
                    "a hysteresis has nothing called \"{other}\" to set: there are \
                     setpoint, above and below"
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

    fn thermostat() -> Hysteresis {
        Hysteresis::of(
            &[
                ("setpoint", "293.15"),
                ("above", "2.0"),
                ("below", "2.0"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), Scalar::of(*v)))
            .collect(),
        )
        .expect("a thermostat")
    }

    /// Cold turns the heaters on; hot turns them off.
    #[test]
    fn the_decision_follows_the_temperature() {
        let mut h = thermostat();

        // More than two degrees under: on.
        assert_eq!(h.give(None, b"290").unwrap(), Some(b"1".to_vec()));
        // More than two degrees over: off.
        assert_eq!(h.give(None, b"296").unwrap(), Some(b"0".to_vec()));
    }

    /// Inside the band, nothing happens.
    ///
    /// The reason the band exists: a bare comparison switches a relay every
    /// time the temperature wobbles across one number.
    #[test]
    fn nothing_happens_inside_the_band() {
        let mut h = thermostat();

        for wobble in ["293.15", "294", "292.2", "295.1", "291.2"] {
            assert_eq!(
                h.give(None, wobble.as_bytes()).unwrap(),
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

        assert_eq!(h.give(None, b"280").unwrap(), Some(b"1".to_vec()));
        // Still cold, and still on: nothing to say.
        for _ in 0..10 {
            assert_eq!(h.give(None, b"281").unwrap(), None);
        }
        // Now hot: one command.
        assert_eq!(h.give(None, b"300").unwrap(), Some(b"0".to_vec()));
        assert_eq!(h.give(None, b"301").unwrap(), None);
        // And cold again.
        assert_eq!(h.give(None, b"280").unwrap(), Some(b"1".to_vec()));
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
            h.give(None, b"250").unwrap(),
            Some(b"1".to_vec()),
            "a cold vehicle should switch the heaters on at once"
        );
    }

    /// The ground can move the setpoint, and the next reading acts on it.
    #[test]
    fn the_setpoint_can_be_moved_from_the_ground() {
        let mut h = thermostat();
        assert_eq!(h.give(None, b"290").unwrap(), Some(b"1".to_vec()));

        // Ask for a much colder cabin: 290 is now too warm.
        h.set_setpoint(280.0);
        assert_eq!(h.setpoint(), 280.0);
        assert_eq!(
            h.give(None, b"290").unwrap(),
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
        h.give(None, b"290").unwrap();
        let before = h.give(None, b"291").unwrap();
        assert_eq!(before, None);

        h.set_setpoint(250.0);
        // Nothing was emitted by the setting itself; the state is unchanged
        // until something is measured.
        assert_eq!(h.setpoint(), 250.0);
    }

    /// A band of nothing is refused at the start.
    #[test]
    fn a_band_of_nothing_is_refused() {
        for (above, below) in [("0", "2"), ("2", "0"), ("-1", "2")] {
            let said = Hysteresis::of(
                &[("setpoint", "293"), ("above", above), ("below", below)]
                    .iter()
                    .map(|(k, v)| (k.to_string(), Scalar::of(*v)))
                    .collect(),
            )
            .expect_err("a band of nothing");
            assert!(said.contains("more than nought"), "{said}");
        }
    }

    /// A temperature that is not a number is refused.
    #[test]
    fn a_reading_that_is_not_a_temperature_is_refused() {
        let mut h = thermostat();
        let said = h.give(None, b"cold").expect_err("not a number");
        assert!(said.contains("temperature"), "{said}");
    }
}
