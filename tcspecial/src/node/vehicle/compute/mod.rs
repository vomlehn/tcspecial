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

/// How wide a value is on a link: a 32-bit integer.
pub(crate) const VALUE_BYTES: usize = 4;

/// How many of them are in a kelvin.
///
/// A micro-kelvin is finer than anything a spacecraft's thermometry resolves
/// -- the parts the examples use report in sixteenths and hundredths of a
/// kelvin -- so the unit is below the noise of every sensor that will be read
/// through it, which is what a unit has to be if rounding to it is to be
/// nothing at all.
pub(crate) const MICRO_KELVIN: f64 = 1_000_000.0;

/// A value as it travels: four bytes of a signed count of micro-kelvin,
/// little endian.
///
/// A temperature is carried as a whole number of micro-kelvin rather than as
/// the double the arithmetic produced. The unit is finer than any sensor
/// here resolves, so nothing a part reported is lost by it, and what is
/// gained is that the number on the link is exact: a conversion, a vote and a
/// thermostat all see the same integer, where three doubles of the same
/// temperature can differ in their last bit and a comparison between them
/// turns on dust. It halves what a reading costs in a log and on a link as
/// well.
///
/// Signed, although a temperature in kelvin is not, for two reasons. The
/// arithmetic that produces one is signed -- a conversion is `counts * scale +
/// offset` and a wrong scale produces a negative, which must arrive as a
/// negative to be seen for what it is rather than as a temperature near
/// 4295 K. And a difference of two temperatures is a temperature's width,
/// which a band is, and that is signed by nature.
///
/// Little endian because every other number this software puts on a wire is:
/// a timestamp's microseconds, a node's id, a beacon interval. A sensor's own
/// bytes are the exception and say which way round they are -- `i16_be`,
/// `u24_be` -- because the part decides that and the file states it.
///
/// What it costs is the range, which is where this is refused rather than
/// wrapped: see [`micro_kelvin`]. And a value in a telemetry log is four bytes
/// rather than a readable number, as is the datum a panel shows for a source.
/// A decision is still text, `1` or `0`, because what it drives is a set of
/// discrete lines and that is the value they take.
pub(crate) fn as_value(number: f64) -> Result<Vec<u8>, String> {
    Ok(micro_kelvin(number)?.to_le_bytes().to_vec())
}

/// A value that arrived, as a number of kelvin.
///
/// Exactly four bytes, and every four bytes are a value: a count of
/// micro-kelvin is a number whatever the bits say, so the width is the whole
/// of what there is to check here. What cannot be represented is refused
/// where a value is made instead, by [`micro_kelvin`], which is where the
/// complaint can name the number that would not fit.
///
/// A width that is not four is a link carrying something that is not a value
/// -- a sensor's raw bytes delivered where a conversion's output was meant, or
/// a node whose output is a decision -- and taking four of them would be a
/// temperature made of whatever was there.
pub(crate) fn from_value(data: &[u8], what: &str) -> Result<f64, String> {
    let bytes: [u8; VALUE_BYTES] = data.try_into().map_err(|_| {
        format!(
            "{what}: a value is {VALUE_BYTES} bytes of micro-kelvin, and {} arrived",
            data.len()
        )
    })?;

    Ok(f64::from(i32::from_le_bytes(bytes)) / MICRO_KELVIN)
}

/// A temperature as the count of micro-kelvin a value carries, or why it
/// cannot be one.
///
/// The one place the range of a value is stated, and everything that turns a
/// number into a temperature comes here: a reading on its way onto a link, and
/// a number read from text, which is a program's answer or an operator's
/// command.
///
/// A signed count of micro-kelvin reaches a little past 2147 K either way,
/// which is above anything these nodes are pointed at and below a great deal
/// that exists -- a thruster's throat is hotter. A temperature outside it is
/// refused and named rather than wrapped or pinned to the end of the range:
/// wrapping turns 2200 K into a cabin that is 42 K below zero, pinning turns
/// a runaway into a reading that looks like a hot but surviving part, and
/// either is a thermostat acting confidently on a number nobody produced. A
/// node that must carry more than this wants a unit of its own, and that is a
/// change to make deliberately.
///
/// A NaN or an infinity is refused here too, and it is the comparison that
/// makes it matter: a vote sorts its readings, and NaN compares as neither
/// less nor greater, so a sort that met one would order the readings by where
/// they happened to lie and the middle one would be the middle of nothing.
pub(crate) fn micro_kelvin(number: f64) -> Result<i32, String> {
    if !number.is_finite() {
        return Err(format!(
            "{number} is not a number anything can be decided on"
        ));
    }

    // Rounded to the nearest micro-kelvin rather than truncated: the
    // conversions land a hair either side of the figure the part reported --
    // 289.15 K arrives from a count as 289.14999999999998 -- and truncating
    // would take a micro-kelvin off half of them.
    let micro = (number * MICRO_KELVIN).round();
    if micro < f64::from(i32::MIN) || micro > f64::from(i32::MAX) {
        return Err(format!(
            "{number} K is outside what a value carries, which is \
             {:.6} K to {:.6} K in micro-kelvin",
            f64::from(i32::MIN) / MICRO_KELVIN,
            f64::from(i32::MAX) / MICRO_KELVIN
        ));
    }
    Ok(micro as i32)
}

/// A value as a program reads and writes one: decimal text.
///
/// The one place a value is text, because the far end is a program reading
/// lines -- see [`process`] -- and a line is text. Written out of the kelvin
/// the value carried, which is a whole number of micro-kelvin, so a program is
/// handed at most six decimal places and what it reads back is what arrived.
pub(crate) fn as_text(number: f64) -> String {
    format!("{number}")
}

/// And a number a program wrote, as a number of kelvin.
///
/// Held to the same range as a reading, by [`micro_kelvin`], because what is
/// read here becomes one: a program's answer goes straight onto a link, and
/// an operator's setpoint or band is compared against readings that cannot
/// leave that range. A number outside it is told so when it is sent rather
/// than carried as far as the arithmetic and lost there.
pub(crate) fn from_text(text: &str, what: &str) -> Result<f64, String> {
    let said = text.trim();
    let number: f64 = said
        .parse()
        .map_err(|_| format!("{what}: \"{said}\" is not a number"))?;
    micro_kelvin(number).map_err(|why| format!("{what}: {why}"))?;
    Ok(number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// A value survives the trip there and back.
    #[test]
    fn a_value_reads_back_as_itself() {
        for number in [0.0, 1.0, 273.15, 293.15, -40.0, 0.0625, 1000.0] {
            let sent = as_value(number).expect("a temperature a value carries");
            let back = from_value(&sent, "a value").expect("it reads back");
            assert!(
                (back - number).abs() < 0.0001,
                "{number} came back as {back}"
            );
        }
    }

    /// A value is four bytes of micro-kelvin, least significant first.
    ///
    /// Rounded to the unit and exact in it: what goes on the link is a whole
    /// number of micro-kelvin, so every node that reads it reads the same
    /// number rather than its own last bit of a double.
    #[test]
    fn a_value_is_a_count_of_micro_kelvin() {
        assert_eq!(
            as_value(293.15).expect("a temperature"),
            293_150_000_i32.to_le_bytes()
        );
        assert_eq!(as_value(0.0).expect("nought").len(), VALUE_BYTES);
        // Below zero, which is what the sign is for: a conversion with the
        // wrong offset produces one, and it must arrive as what it is.
        assert_eq!(
            as_value(-40.0).expect("below zero"),
            (-40_000_000_i32).to_le_bytes()
        );

        // The trip is exact for every temperature that is a whole number of
        // micro-kelvin, which is every temperature a value carries.
        for number in [0.0, 273.15, 293.15, -40.0, 0.0625, 1000.0, 0.000_001] {
            assert_eq!(
                from_value(&as_value(number).expect("a temperature"), "a value")
                    .expect("it reads back"),
                number
            );
        }

        // And what is finer than the unit is rounded to it, rather than
        // truncated: a third of a kelvin is 333_333 micro-kelvin and a hair.
        let third = from_value(&as_value(1.0 / 3.0).expect("a third"), "a third")
            .expect("it reads back");
        assert_eq!(third, 0.333_333);
    }

    /// A temperature a value cannot carry is refused, and named.
    ///
    /// Refused where it is made rather than where it is read, because this is
    /// where the number is still in hand to put in the complaint. Wrapping it
    /// would turn 2200 K into a cabin 42 K below zero and a thermostat would
    /// act on that.
    #[test]
    fn a_temperature_outside_the_range_is_refused() {
        for wrong in [2200.0, -2200.0, 1.0e9, f64::NAN, f64::INFINITY] {
            let said = as_value(wrong).expect_err("outside what a value carries");
            assert!(
                said.contains("2147.483647") || said.contains("number"),
                "the complaint should say what the range is: {said}"
            );
        }

        // The ends themselves are carried, and are what they say.
        assert_eq!(
            from_value(&as_value(2147.483647).expect("the top"), "the top"),
            Ok(2147.483647)
        );
        assert_eq!(
            from_value(&as_value(-2147.483648).expect("the bottom"), "the bottom"),
            Ok(-2147.483648)
        );
    }

    /// What is not a value is reported rather than read as one.
    #[test]
    fn a_datum_that_is_not_a_value_is_reported() {
        // A width that is not four: a sensor's own bytes delivered where a
        // conversion's output was meant, or a decision, which is text.
        let said = from_value(b"293.15", "the fore sensor").expect_err("not a value");
        assert!(said.contains("fore sensor"), "{said}");
        assert!(said.contains('6'), "the width is worth saying: {said}");
        assert!(from_value(b"1", "a decision").is_err());

        // Four bytes are a value whatever they hold, there being no bit
        // pattern of a 32-bit integer that is not a number of micro-kelvin.
        // What a sensor cannot produce is refused where a value is made.
        assert_eq!(from_value(&[0xFF, 0xFF, 0xFF, 0x7F], "the top"), Ok(2147.483647));
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

        for number in [0.0, 293.15, 0.333_333, -40.0, 1.0e-9] {
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
        // And the range a reading is held to, because what a program answers
        // with becomes a reading: see `from_text`.
        let said = from_text("3000", "a program").expect_err("outside the range");
        assert!(
            said.contains("a program") && said.contains("2147.483647"),
            "the refusal should name the program and the range: {said}"
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
