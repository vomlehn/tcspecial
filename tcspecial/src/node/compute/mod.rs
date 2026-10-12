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
//! hands back an `Option` rather than a value. A mean over three sensors emits
//! once all three have arrived and not before, so a reading is never averaged
//! against a stale one from the pass before.

pub mod hysteresis;
pub mod mean;
pub mod process;
pub mod to_kelvin;

use std::collections::BTreeMap;

use tcslibgs::graph::Node;

/// Something a compute node runs.
///
/// Given what arrived and told which input it arrived at; hands back what to
/// send on, if anything is ready to send.
pub trait Function {
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
    /// link is. Most functions take none: a mean has nothing to set, and a
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

    /// What this function is called, for a complaint to name.
    fn name(&self) -> &'static str;
}

/// Build the function a node's configuration asks for.
///
/// `inputs` is what the routing found bound to this node, which a function
/// over several of them needs: a mean cannot know it is waiting for three
/// until something tells it how many links arrive.
pub fn make(node: &Node, inputs: &[&str]) -> Result<Box<dyn Function>, String> {
    let attributes = &node.attributes;
    let function = attributes
        .get("function")
        .map(|s| s.as_str().to_string())
        .ok_or_else(|| {
            format!(
                "\"{}\" is a compute node and says no function: one of \
                 to_kelvin, mean, hysteresis, or process",
                node.name
            )
        })?;

    match function.as_str() {
        "to_kelvin" => Ok(Box::new(to_kelvin::ToKelvin::of(attributes)?)),
        "mean" => Ok(Box::new(mean::Mean::of(inputs)?)),
        "hysteresis" => Ok(Box::new(hysteresis::Hysteresis::of(attributes)?)),
        // Started here, as the configuration is loaded: a program that
        // cannot be executed is a fault found on the ground rather than three
        // days into a mission. See node::compute::process.
        "process" => {
            let mut program = process::Process::of(attributes)?;
            program.start()?;
            Ok(Box::new(program))
        }
        other => Err(format!(
            "\"{}\": there is no computation called \"{other}\". The built-in \
             ones are to_kelvin, mean and hysteresis; \"process\" runs a \
             program of your own",
            node.name
        )),
    }
}

/// A number an attribute says, or what is wrong with it.
///
/// Required, because every parameter these functions take is one a
/// computation cannot be done without. A scale of nought or an absent
/// setpoint is not a default worth inventing -- it is a configuration that
/// has not said what it wants, and saying so is better than averaging against
/// zero.
pub(crate) fn number(
    attributes: &BTreeMap<String, tcslibgs::Scalar>,
    name: &str,
) -> Result<f64, String> {
    let text = attributes
        .get(name)
        .ok_or_else(|| format!("{name} is not set, and there is no sensible default for it"))?;
    text.as_str()
        .parse()
        .map_err(|_| format!("{name}: \"{}\" is not a number", text.as_str()))
}

/// A value as it travels: decimal text.
pub(crate) fn as_value(number: f64) -> Vec<u8> {
    // Enough places for a temperature in kelvin to a hundredth and for the
    // ratios a scale produces, and few enough that a log stays readable. A
    // format that printed seventeen significant figures would be honest about
    // the double and useless to read.
    format!("{number:.4}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .as_bytes()
        .to_vec()
}

/// A value that arrived, as a number.
pub(crate) fn from_value(data: &[u8], what: &str) -> Result<f64, String> {
    let text = std::str::from_utf8(data)
        .map_err(|_| format!("{what}: what arrived is not text, so it is not a value"))?;
    text.trim()
        .parse()
        .map_err(|_| format!("{what}: \"{}\" is not a number", text.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// A value is written in a form a person can read.
    #[test]
    fn a_value_is_readable() {
        assert_eq!(as_value(293.15), b"293.15");
        assert_eq!(as_value(0.0), b"0");
        assert_eq!(as_value(-40.0), b"-40");
        // Not seventeen significant figures: honest about the double and
        // useless in a log.
        assert_eq!(as_value(1.0 / 3.0), b"0.3333");
    }

    /// What is not a number is reported rather than guessed at.
    #[test]
    fn a_datum_that_is_not_a_value_is_reported() {
        let said = from_value(b"29x.15", "the fore sensor").expect_err("not a number");
        assert!(said.contains("fore sensor"), "{said}");
        assert!(said.contains("29x.15"), "{said}");

        // And bytes that are not text at all.
        let said = from_value(&[0xff, 0xfe], "a sensor").expect_err("not text");
        assert!(said.contains("not text"), "{said}");
    }

    /// The thermostat, end to end, from counts to a heater decision.
    ///
    /// The three built-ins joined as the shipped example joins them, because
    /// each of them being right is not the same as the three of them agreeing:
    /// the mean has to accept what to_kelvin produces, and the hysteresis has
    /// to accept what the mean produces. A representation change in one of
    /// them would pass that one's own tests and fail here.
    #[test]
    fn the_thermostat_runs_from_counts_to_a_decision() {
        let sensor = |scale: &str, offset: &str| {
            let mut attributes = BTreeMap::new();
            attributes.insert("format".to_string(), tcslibgs::Scalar::of("i16_be"));
            attributes.insert("scale".to_string(), tcslibgs::Scalar::of(scale));
            attributes.insert("offset".to_string(), tcslibgs::Scalar::of(offset));
            to_kelvin::ToKelvin::of(&attributes).expect("a conversion")
        };

        // Three sensors of one make, for simplicity: a sixteenth of a degree
        // a count, above freezing.
        let mut converters = [
            sensor("0.0625", "273.15"),
            sensor("0.0625", "273.15"),
            sensor("0.0625", "273.15"),
        ];
        let mut mean = mean::Mean::of(&["in0", "in1", "in2"]).expect("a mean");

        let mut thermostat = {
            let mut attributes = BTreeMap::new();
            attributes.insert("setpoint".to_string(), tcslibgs::Scalar::of("293.15"));
            attributes.insert("above".to_string(), tcslibgs::Scalar::of("2.0"));
            attributes.insert("below".to_string(), tcslibgs::Scalar::of("2.0"));
            hysteresis::Hysteresis::of(&attributes).expect("a thermostat")
        };

        /// One pass: three readings in, a decision out if there is one.
        fn pass(
            converters: &mut [to_kelvin::ToKelvin; 3],
            mean: &mut mean::Mean,
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
                if let Some(out) = mean.give(Some(input), &kelvin).expect("the mean") {
                    averaged = Some(out);
                }
            }
            let averaged = averaged.expect("three readings make a mean");
            thermostat.give(None, &averaged).expect("a decision")
        }

        // A cold cabin: 280 counts is about 290.6 K, more than two under.
        let decision = pass(&mut converters, &mut mean, &mut thermostat, [280, 280, 280]);
        assert_eq!(
            decision,
            Some(b"1".to_vec()),
            "a cold cabin should assert the heater lines"
        );

        // Warming through the band: nothing to say.
        let decision = pass(&mut converters, &mut mean, &mut thermostat, [320, 320, 320]);
        assert_eq!(decision, None, "inside the band nothing should change");

        // Over the top: the heaters go off, once.
        let decision = pass(&mut converters, &mut mean, &mut thermostat, [360, 360, 360]);
        assert_eq!(decision, Some(b"0".to_vec()));
        let decision = pass(&mut converters, &mut mean, &mut thermostat, [365, 365, 365]);
        assert_eq!(decision, None, "the same decision should not be repeated");

        // And the ground asks for a warmer cabin, which the next pass acts on.
        thermostat.set_setpoint(300.0);
        let decision = pass(&mut converters, &mut mean, &mut thermostat, [365, 365, 365]);
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
        let i2c = |_| {
            let mut a = BTreeMap::new();
            a.insert("format".to_string(), tcslibgs::Scalar::of("i16_be"));
            a.insert("scale".to_string(), tcslibgs::Scalar::of("0.0625"));
            a.insert("offset".to_string(), tcslibgs::Scalar::of("273.15"));
            to_kelvin::ToKelvin::of(&a).unwrap()
        };
        let mut fore = i2c(());
        let mut aft = i2c(());
        // A different part: three bytes, a different scale and offset.
        let mut bay = {
            let mut a = BTreeMap::new();
            a.insert("format".to_string(), tcslibgs::Scalar::of("u24_be"));
            a.insert("scale".to_string(), tcslibgs::Scalar::of("0.001"));
            a.insert("offset".to_string(), tcslibgs::Scalar::of("273.15"));
            to_kelvin::ToKelvin::of(&a).unwrap()
        };

        let mut mean = mean::Mean::of(&["in0", "in1", "in2"]).unwrap();

        // All three sitting at 20 above freezing, in their own units.
        let a = fore.give(None, &320i16.to_be_bytes()).unwrap().unwrap();
        let b = aft.give(None, &320i16.to_be_bytes()).unwrap().unwrap();
        let c = bay.give(None, &[0x00, 0x4e, 0x20]).unwrap().unwrap();

        assert_eq!(mean.give(Some("in0"), &a).unwrap(), None);
        assert_eq!(mean.give(Some("in1"), &b).unwrap(), None);
        let out = mean.give(Some("in2"), &c).unwrap().expect("a mean");

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
            editor: None,
        };
        let said = match make(&node, &[]) {
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
            editor: None,
        };
        let said = match make(&node, &[]) {
            Err(said) => said,
            Ok(f) => panic!("sorcery became {}", f.name()),
        };
        assert!(said.contains("sorcery"), "{said}");
        assert!(said.contains("built-in"), "{said}");
    }

    /// A missing parameter is refused rather than defaulted.
    #[test]
    fn a_parameter_with_no_sensible_default_is_required() {
        let attributes = BTreeMap::new();
        let said = number(&attributes, "scale").expect_err("a scale is required");
        assert!(said.contains("no sensible default"), "{said}");

        let mut attributes = BTreeMap::new();
        attributes.insert("scale".to_string(), tcslibgs::Scalar::of("quite a lot"));
        let said = number(&attributes, "scale").expect_err("not a number");
        assert!(said.contains("quite a lot"), "{said}");
    }
}
