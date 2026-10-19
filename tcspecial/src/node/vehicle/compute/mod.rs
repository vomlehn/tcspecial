//! Computation aboard: what a compute node runs.
//!
//! A compute node takes what arrives, works something out, may remember
//! something between arrivals, and hands on a result. The three in the
//! thermostat are the shape of it: one converts a sensor's raw counts to
//! kelvin, one averages three of those, and one compares the average against a
//! setpoint and decides whether the heaters should be on.
//!
//! # What a value looks like on a link
//!
//! Decimal text. A reading leaves `to_kelvin` as `293.15` and arrives at the
//! mean as the same five characters.
//!
//! This is a choice and the alternatives are real -- eight bytes of IEEE
//! double would be smaller and quicker -- so the reasons are worth writing
//! down, in the order this project ranks things.
//!
//! *Reliability.* Text has no endianness to get wrong between a vehicle and a
//! ground station, and no alignment. A value that is not a number is
//! detectable: `29x.15` fails to parse and is reported, where four bytes
//! misread as a float are a plausible temperature nobody questions.
//!
//! *Ease of use.* The telemetry log holds what happened in a form a person can
//! read without a decoder, which is what somebody reconstructing a control
//! loop at three in the morning actually has. A number in a log is worth more
//! than a byte saved on a link.
//!
//! The cost is a few bytes per reading and a parse per arrival, which for a
//! control loop running every three seconds is nothing worth having the other
//! properties for.
//!
//! # What a function may assume
//!
//! Nothing about order, and nothing about completeness. Arrivals come as the
//! devices produce them, so a function over several inputs is handed one at a
//! time and says for itself when it has enough -- which is why [`Function`]
//! hands back an `Option` rather than a value. A vote over three sensors emits
//! once all three have arrived and not before, so a reading is never averaged
//! against a stale one from the pass before.

pub mod hysteresis;
pub mod process;
pub mod to_kelvin;
pub mod vote;

use tcslibgs::graph::Node;
use tcslibgs::node::vehicle::compute::{self, Computation};
use crate::runtime::node_state::Abandon;

/// Something a compute node runs.
///
/// Given what arrived and told which input it arrived at; hands back what to
/// send on, if anything is ready to send.
pub trait Function: Send {
    /// Take one arrival.
    ///
    /// `Ok(None)` means "nothing to send yet", which is the ordinary answer
    /// from a function waiting for the rest of its inputs, and from one that
    /// decided nothing needs to change. An error is a datum the function
    /// cannot use, and is reported rather than guessed at.
    fn give(&mut self, input: Option<&str>, data: &[u8]) -> Result<Option<Vec<u8>>, String>;

    /// Set one of this function's parameters, as a command from the ground
    /// does.
    ///
    /// A setpoint arrives this way, and the value is text as everything on a
    /// link is. Most functions take none: a vote has nothing to set, and a
    /// conversion's scale is a property of the part rather than something the
    /// ground changes -- so the default refuses, naming the function, and a
    /// function that does take settings says which.
    ///
    /// What a command may set is in the configuration as a binding, and the
    /// checking refuses one naming an attribute its node will not accept. This
    /// is where that promise is kept: an attribute no function accepts is
    /// refused here however it was reached.
    fn set(&mut self, attribute: &str, _value: &[u8]) -> Result<(), String> {
        Err(format!(
            "{} has nothing called \"{attribute}\" to set, and takes no settings \
             from the ground at all",
            self.name()
        ))
    }

    /// What a device this function drives should be held at, where it drives
    /// one.
    ///
    /// Asked on every arrival that produced nothing new, and sent again to
    /// the devices alone: see [`crate::runtime::fabric::Fabric::deliver_on`].
    /// A thermostat answers with the decision it last made, because a relay
    /// is held at what it was last told and a decision that went missing on
    /// the way to it would otherwise stand for ever -- the vehicle believing
    /// it had switched the heaters off, the mass climbing, and the decision
    /// that would correct it being the one the vehicle thinks it has already
    /// sent.
    ///
    /// `None` is every other function, and means what it says: there is
    /// nothing a device should be held at between results. A conversion and a
    /// vote answer that way because what they produce is a measurement -- a
    /// reading nobody took is not a reading, where a decision nobody has
    /// changed is still the decision.
    fn held(&self) -> Option<Vec<u8>> {
        None
    }

    /// What this function is called, for a complaint to name.
    fn name(&self) -> &'static str;
}

/// Build the function a node's configuration asks for.
///
/// `inputs` is what the routing found bound to this node, which a function
/// over several of them needs: a vote cannot know it is waiting for three
/// until something tells it how many links arrive.
pub fn make(
    node: &Node,
    inputs: &[&str],
    abandon: &Abandon,
) -> Result<Box<dyn Function>, String> {
    // What the node says, by the rules the ground tool checks it against: see
    // tcslibgs::node::vehicle::compute. Every complaint from there already names the
    // node and the computation.
    let what = compute::params_of(&node.name, &node.attributes)?;

    match what {
        Computation::ToKelvin(params) => Ok(Box::new(to_kelvin::ToKelvin::of(params))),
        Computation::Vote(params) => {
            // The declared inputs and the bound ones are checked against each
            // other by `Graph::check`, so a vote that reaches here is one
            // whose links are the inputs it says. Checked again all the same,
            // because what this builds is the thing that waits for the first
            // reading from each: a vote waiting on an input nothing delivers
            // would never produce anything, and nothing in the telemetry
            // would say why.
            let mut bound: Vec<&str> = inputs.to_vec();
            let mut declared: Vec<&str> =
                params.inputs.iter().map(String::as_str).collect();
            bound.sort();
            declared.sort();
            if bound != declared {
                return Err(format!(
                    "\"{}\" (vote) is over {declared:?} and the links bind \
                     {bound:?}",
                    node.name
                ));
            }

            Ok(Box::new(vote::Vote::of(params)))
        }
        Computation::Hysteresis(params) => Ok(Box::new(hysteresis::Hysteresis::of(params))),
        // Started here, as the configuration is loaded: a program that
        // cannot be executed is a fault found on the ground rather than three
        // days into a mission. See node::vehicle::compute::process.
        Computation::Process(params) => {
            // The node's own wakeup, so that disabling the node gives up the
            // wait for an answer rather than waiting out a deadline that may
            // be seconds long.
            let mut program = process::Process::of(params).abandoned_for(abandon.clone());
            program
                .start()
                .map_err(|why| format!("\"{}\" (process): {why}", node.name))?;
            Ok(Box::new(program))
        }
    }
}

/// How wide a value is on a link: a 64-bit float.
pub(crate) const VALUE_BYTES: usize = 8;

/// A value as it travels: eight bytes of IEEE 754 binary64, little endian.
///
/// A temperature in kelvin is what this carries, and it carries the double the
/// arithmetic produced rather than a rendering of it. It was decimal text
/// rounded to four places, which cost two things. The rounding was real: three
/// sensors of one place differ in the fourth and fifth figures, and a vote over
/// readings rounded to the fourth is a vote over ties the sensors did not
/// report. And a reading was re-parsed at every hop, so a conversion, a vote
/// and a thermostat each turned text into a number again -- three places for a
/// locale, a space or a stray sign to be read differently.
///
/// Little endian because every other number this software puts on a wire is:
/// a timestamp's microseconds, a node's id, a beacon interval. A sensor's own
/// bytes are the exception and say which way round they are -- `i16_be`,
/// `u24_be` -- because the part decides that and the file states it.
///
/// What it costs is that a value in a telemetry log is eight bytes rather than
/// a readable number, and so is the datum a panel shows for a source. A
/// decision is still text, `1` or `0`, because what it drives is a set of
/// discrete lines and that is the value they take.
pub(crate) fn as_value(number: f64) -> Vec<u8> {
    number.to_le_bytes().to_vec()
}

/// A value that arrived, as a number.
///
/// Exactly eight bytes, and a finite number in them. Both refusals are the
/// same kind of thing: a datum that is not a value must not be read as one.
///
/// A width that is not eight is a link carrying something that is not a value
/// -- a sensor's raw bytes delivered where a conversion's output was meant, or
/// a node whose output is a decision -- and taking eight of them would be a
/// temperature made of whatever was there.
///
/// A NaN or an infinity is a reading nothing can act on, and the one place it
/// must not reach is a comparison: a vote sorts its readings, and NaN compares
/// as neither less nor greater, so a sort that met one would order the
/// readings by where they happened to be and the middle one would be the
/// middle of nothing. Refused here, where the complaint can name the node that
/// sent it.
pub(crate) fn from_value(data: &[u8], what: &str) -> Result<f64, String> {
    let bytes: [u8; VALUE_BYTES] = data.try_into().map_err(|_| {
        format!(
            "{what}: a value is {VALUE_BYTES} bytes of a 64-bit float, and {} arrived",
            data.len()
        )
    })?;

    let number = f64::from_le_bytes(bytes);
    if !number.is_finite() {
        return Err(format!(
            "{what}: {number} is not a number anything can be decided on"
        ));
    }
    Ok(number)
}

/// A value as a program reads and writes one: decimal text.
///
/// The one place a value is text, because the far end is a program reading
/// lines -- see [`process`] -- and a line is text. The shortest decimal that
/// reads back as the same double, so the trip through a program's pipe loses
/// nothing either.
pub(crate) fn as_text(number: f64) -> String {
    format!("{number}")
}

/// And a number a program wrote, as a value.
pub(crate) fn from_text(text: &str, what: &str) -> Result<f64, String> {
    let said = text.trim();
    let number: f64 = said
        .parse()
        .map_err(|_| format!("{what}: \"{said}\" is not a number"))?;
    if !number.is_finite() {
        return Err(format!(
            "{what}: {number} is not a number anything can be decided on"
        ));
    }
    Ok(number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// A value survives the trip there and back.
    #[test]
    fn a_value_reads_back_as_itself() {
        for number in [0.0, 1.0, 273.15, 293.15, -40.0, 0.0625, 1000000.0] {
            let sent = as_value(number);
            let back = from_value(&sent, "a value").expect("it reads back");
            assert!(
                (back - number).abs() < 0.0001,
                "{number} came back as {back} ({})",
                String::from_utf8_lossy(&sent)
            );
        }
    }

    /// A value is eight bytes of the double itself, least significant first.
    ///
    /// Nothing is rounded on the way: a reading goes on the link as the
    /// arithmetic left it, which is what lets a vote over three sensors see
    /// the figures they differ in.
    #[test]
    fn a_value_is_the_double_itself() {
        assert_eq!(as_value(293.15), 293.15_f64.to_le_bytes());
        assert_eq!(as_value(0.0).len(), VALUE_BYTES);

        // A third reads back as a third, which four decimal places could not
        // say.
        let third = from_value(&as_value(1.0 / 3.0), "a third").expect("it reads back");
        assert_eq!(third, 1.0 / 3.0);

        // And the trip is exact for every temperature a vehicle sees, which
        // is the point of carrying the double.
        for number in [0.0, 273.15, 293.15, -40.0, 0.0625, 1.0e6, f64::MIN, f64::MAX] {
            assert_eq!(
                from_value(&as_value(number), "a value").expect("it reads back"),
                number
            );
        }
    }

    /// What is not a value is reported rather than read as one.
    #[test]
    fn a_datum_that_is_not_a_value_is_reported() {
        // A width that is not eight: a sensor's own bytes delivered where a
        // conversion's output was meant, or a decision, which is text.
        let said = from_value(b"293.15", "the fore sensor").expect_err("not a value");
        assert!(said.contains("fore sensor"), "{said}");
        assert!(said.contains('6'), "the width is worth saying: {said}");
        assert!(from_value(b"1", "a decision").is_err());

        // And a NaN, which must not reach a comparison: see `from_value`.
        for wrong in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let said = from_value(&wrong.to_le_bytes(), "a sensor")
                .expect_err("nothing can be decided on it");
            assert!(said.contains("sensor"), "{said}");
        }
    }

    /// A program's pipe carries a decimal, and it round-trips.
    ///
    /// The one place a value is written out and read back: see
    /// `super::process`. The shortest decimal that reads back as the same
    /// double, so a number does not drift by going through a program.
    #[test]
    fn a_program_reads_and_writes_a_decimal() {
        assert_eq!(as_text(293.15), "293.15");
        assert_eq!(as_text(-40.0), "-40");

        for number in [0.0, 293.15, 1.0 / 3.0, -40.0, 1.0e-9] {
            assert_eq!(
                from_text(&as_text(number), "a program").expect("it reads back"),
                number
            );
        }

        assert!(from_text("nearly", "a program").is_err());
        assert!(
            from_text("NaN", "a program").is_err(),
            "a program may not answer with a number nothing can be decided on"
        );
    }

    /// The thermostat, end to end, from counts to a heater decision.
    ///
    /// The three built-ins joined as the shipped example joins them, because
    /// each of them being right is not the same as the three of them agreeing:
    /// the vote has to accept what to_kelvin produces, and the hysteresis has
    /// to accept what the vote produces. A representation change in one of
    /// them would pass that one's own tests and fail here.
    #[test]
    fn the_thermostat_runs_from_counts_to_a_decision() {
        let sensor = |scale: f64, offset: f64| {
            to_kelvin::ToKelvin::of(tcslibgs::node::vehicle::compute::ToKelvinParams {
                format: tcslibgs::node::vehicle::compute::Format::I16Be,
                scale,
                offset,
            })
        };

        use tcslibgs::node::vehicle::compute::{Algorithm, VoteParams};

        // Three sensors of one make, for simplicity: a sixteenth of a degree
        // a count, above freezing.
        let mut converters = [
            sensor(0.0625, 273.15),
            sensor(0.0625, 273.15),
            sensor(0.0625, 273.15),
        ];
        let mut voted = vote::Vote::of(VoteParams {
            inputs: vec!["in0".to_string(), "in1".to_string(), "in2".to_string()],
            algorithm: Algorithm::Median,
        });

        let mut thermostat =
            hysteresis::Hysteresis::of(tcslibgs::node::vehicle::compute::HysteresisParams {
                setpoint: 293.15,
                half_band: 2.0,
            });

        /// One pass: three readings in, a decision out if there is one.
        fn pass(
            converters: &mut [to_kelvin::ToKelvin; 3],
            voted: &mut vote::Vote,
            thermostat: &mut hysteresis::Hysteresis,
            counts: [i16; 3],
        ) -> Option<Vec<u8>> {
            let mut averaged = None;
            for (n, count) in counts.iter().enumerate() {
                let kelvin = converters[n]
                    .give(None, &count.to_be_bytes())
                    .expect("a conversion")
                    .expect("a value");
                let input = ["in0", "in1", "in2"][n];
                if let Some(out) = voted.give(Some(input), &kelvin).expect("the mean") {
                    averaged = Some(out);
                }
            }
            let averaged = averaged.expect("three readings make a mean");
            thermostat.give(None, &averaged).expect("a decision")
        }

        // A cold cabin: 280 counts is about 290.6 K, more than two under.
        let decision = pass(&mut converters, &mut voted, &mut thermostat, [280, 280, 280]);
        assert_eq!(
            decision,
            Some(b"1".to_vec()),
            "a cold cabin should assert the heater lines"
        );

        // Warming through the band: nothing to say.
        let decision = pass(&mut converters, &mut voted, &mut thermostat, [320, 320, 320]);
        assert_eq!(decision, None, "inside the band nothing should change");

        // Over the top: the heaters go off, once.
        let decision = pass(&mut converters, &mut voted, &mut thermostat, [360, 360, 360]);
        assert_eq!(decision, Some(b"0".to_vec()));
        let decision = pass(&mut converters, &mut voted, &mut thermostat, [365, 365, 365]);
        assert_eq!(decision, None, "the same decision should not be repeated");

        // And the ground asks for a warmer cabin, which the next pass acts on.
        thermostat.set_setpoint(300.0);
        let decision = pass(&mut converters, &mut voted, &mut thermostat, [365, 365, 365]);
        assert_eq!(
            decision,
            Some(b"1".to_vec()),
            "a higher setpoint should call for the heaters again"
        );
    }

    /// Three different sensors averaged give one temperature.
    ///
    /// The example's actual arrangement: two parts of one make and a third of
    /// another, each with its own conversion, averaged into one number.
    #[test]
    fn three_unlike_sensors_average_to_one_temperature() {
        use tcslibgs::node::vehicle::compute::{Algorithm, Format, ToKelvinParams, VoteParams};

        let part = |format: Format, scale: f64| {
            to_kelvin::ToKelvin::of(ToKelvinParams {
                format,
                scale,
                offset: 273.15,
            })
        };
        let mut fore = part(Format::I16Be, 0.0625);
        let mut aft = part(Format::I16Be, 0.0625);
        // A different part: three bytes, and a different scale.
        let mut bay = part(Format::U24Be, 0.001);

        let mut voted = vote::Vote::of(VoteParams {
            inputs: vec!["in0".to_string(), "in1".to_string(), "in2".to_string()],
            algorithm: Algorithm::Median,
        });

        // All three sitting at 20 above freezing, in their own units.
        let a = fore.give(None, &320i16.to_be_bytes()).unwrap().unwrap();
        let b = aft.give(None, &320i16.to_be_bytes()).unwrap().unwrap();
        let c = bay.give(None, &[0x00, 0x4e, 0x20]).unwrap().unwrap();

        assert_eq!(voted.give(Some("in0"), &a).unwrap(), None);
        assert_eq!(voted.give(Some("in1"), &b).unwrap(), None);
        let out = voted.give(Some("in2"), &c).unwrap().expect("a vote");

        let averaged = from_value(&out, "the mean").unwrap();
        assert!(
            (averaged - 293.15).abs() < 0.01,
            "three sensors at one temperature averaged to {averaged}"
        );
    }

    /// A node that names no function is refused, and told what there is.
    #[test]
    fn a_compute_node_must_say_what_it_computes() {
        let node = Node {
            name: "mystery".to_string(),
            node_type: "compute".to_string(),
            group: None,
            attributes: BTreeMap::new(),
        };
        let said = match make(&node, &[], &Abandon::nothing()) {
            Err(said) => said,
            Ok(f) => panic!("a node with no function became {}", f.name()),
        };
        assert!(said.contains("mystery"), "{said}");
        assert!(said.contains("to_kelvin"), "it should say what there is: {said}");
    }

    /// A function nobody has written is refused by name.
    #[test]
    fn an_unknown_function_is_refused() {
        let mut attributes = BTreeMap::new();
        attributes.insert("function".to_string(), tcslibgs::Scalar::of("sorcery"));
        let node = Node {
            name: "n".to_string(),
            node_type: "compute".to_string(),
            group: None,
            attributes,
        };
        let said = match make(&node, &[], &Abandon::nothing()) {
            Err(said) => said,
            Ok(f) => panic!("sorcery became {}", f.name()),
        };
        assert!(said.contains("sorcery"), "{said}");
        assert!(said.contains("built-in"), "{said}");
    }

    /// A node whose computation is wrong is refused by the one set of rules.
    ///
    /// Which is in `tcslibgs::node::vehicle::compute` now, so the ground tool applies
    /// it too: what this holds is that `make` goes through those rules rather
    /// than having a second set of its own.
    #[test]
    fn a_parameter_with_no_sensible_default_is_required() {
        let mut attributes = BTreeMap::new();
        attributes.insert("function".to_string(), tcslibgs::Scalar::of("to_kelvin"));
        attributes.insert("format".to_string(), tcslibgs::Scalar::of("i16_be"));
        attributes.insert("offset".to_string(), tcslibgs::Scalar::of("273.15"));
        let node = Node {
            name: "k_1".to_string(),
            node_type: "compute".to_string(),
            group: None,
            attributes,
        };

        let said = match make(&node, &[], &Abandon::nothing()) {
            Err(said) => said,
            Ok(f) => panic!("a conversion with no scale became {}", f.name()),
        };
        assert!(said.contains("k_1") && said.contains("scale"), "{said}");
    }

}
