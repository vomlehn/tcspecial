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

use crate::runtime::node_state::Abandon;
use tcslibgs::graph::Node;
use tcslibgs::node::vehicle::compute::{self, Computation};

/// The most a computation hands on at once.
///
/// Every computation's result is a value -- four bytes, see
/// `tcslibgs::node::vehicle::compute::VALUE_BYTES` -- or a decision of one
/// byte, so this is room to spare rather than a limit anything meets.
pub const OUT_BYTES: usize = 16;

/// What a computation hands on: a few bytes, held in place.
///
/// Not a `Vec`, so that working something out allocates nothing once the
/// vehicle is running: a result is made, copied into the node's queue and
/// taken out of it without anything being made on the way.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Out {
    bytes: [u8; OUT_BYTES],
    len: usize,
}

impl Out {
    /// A value, as every computation's result over temperatures is: see
    /// `tcslibgs::node::vehicle::compute::as_value`.
    pub fn value(number: f64) -> Result<Out, String> {
        let value = value_bytes(number)?;
        let mut bytes = [0u8; OUT_BYTES];
        bytes[..value.len()].copy_from_slice(&value);
        Ok(Out {
            bytes,
            len: value.len(),
        })
    }

    /// One byte: a decision.
    pub const fn byte(byte: u8) -> Out {
        let mut bytes = [0u8; OUT_BYTES];
        bytes[0] = byte;
        Out { bytes, len: 1 }
    }
}

impl std::ops::Deref for Out {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

impl std::fmt::Debug for Out {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Out({:?})", &**self)
    }
}

impl PartialEq<[u8]> for Out {
    fn eq(&self, other: &[u8]) -> bool {
        **self == *other
    }
}

impl PartialEq<Vec<u8>> for Out {
    fn eq(&self, other: &Vec<u8>) -> bool {
        **self == **other
    }
}

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
    fn give(
        &mut self,
        input: Option<&str>,
        data: &[u8],
    ) -> Result<Option<Out>, String>;

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
            "{} has nothing called \"{attribute}\" to set, and takes no \
             settings from the ground at all",
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
    fn held(&self) -> Option<Out> {
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
    // tcslibgs::node::vehicle::compute. Every complaint from there already
    // names the node and the computation.
    let what = compute::params_of(&node.name, &node.attributes)?;

    match what {
        Computation::ToKelvin(params) => {
            Ok(Box::new(to_kelvin::ToKelvin::of(params)))
        }
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
        Computation::Hysteresis(params) => {
            Ok(Box::new(hysteresis::Hysteresis::of(params)))
        }
        // Started here, as the configuration is loaded: a program that
        // cannot be executed is a fault found on the ground rather than three
        // days into a mission. See node::vehicle::compute::process.
        Computation::Process(params) => {
            // The node's own wakeup, so that disabling the node gives up the
            // wait for an answer rather than waiting out a deadline that may
            // be seconds long.
            let mut program =
                process::Process::of(params).abandoned_for(abandon.clone());
            program
                .start()
                .map_err(|why| format!("\"{}\" (process): {why}", node.name))?;
            Ok(Box::new(program))
        }
    }
}

// The form a value takes on a link is `tcslibgs`'s, with the range it can
// carry: it is one fact the vehicle writes and the ground reads back, and the
// range is also a rule about what a configuration may say, which `tcsverify`
// checks on a desk. Named here because every computation in this tree calls
// these by their short names, and because this is where a reader of the
// arithmetic looks for them.
pub(crate) use tcslibgs::node::vehicle::compute::{
    from_text, from_value, value_bytes,
};
// What the vehicle no longer makes as it runs -- a value as a `Vec` -- and the
// tests still compare against.
#[cfg(test)]
pub(crate) use tcslibgs::node::vehicle::compute::as_value;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

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
            to_kelvin::ToKelvin::of(
                tcslibgs::node::vehicle::compute::ToKelvinParams {
                    format: tcslibgs::node::vehicle::compute::Format::I16Be,
                    scale,
                    offset,
                },
            )
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
            inputs: vec![
                "in0".to_string(),
                "in1".to_string(),
                "in2".to_string(),
            ],
            algorithm: Algorithm::Median,
        });

        let mut thermostat = hysteresis::Hysteresis::of(
            tcslibgs::node::vehicle::compute::HysteresisParams {
                setpoint: 293.15,
                half_band: 2.0,
            },
        );

        /// One pass: three readings in, a decision out if there is one.
        fn pass(
            converters: &mut [to_kelvin::ToKelvin; 3],
            voted: &mut vote::Vote,
            thermostat: &mut hysteresis::Hysteresis,
            counts: [i16; 3],
        ) -> Option<Out> {
            let mut averaged = None;
            for (n, count) in counts.iter().enumerate() {
                let kelvin = converters[n]
                    .give(None, &count.to_be_bytes())
                    .expect("a conversion")
                    .expect("a value");
                let input = ["in0", "in1", "in2"][n];
                if let Some(out) =
                    voted.give(Some(input), &kelvin).expect("the mean")
                {
                    averaged = Some(out);
                }
            }
            let averaged = averaged.expect("three readings make a mean");
            thermostat.give(None, &averaged).expect("a decision")
        }

        // A cold cabin: 280 counts is about 290.6 K, more than two under.
        let decision = pass(
            &mut converters,
            &mut voted,
            &mut thermostat,
            [280, 280, 280],
        );
        assert_eq!(
            decision,
            Some(Out::byte(b'1')),
            "a cold cabin should assert the heater lines"
        );

        // Warming through the band: nothing to say.
        let decision = pass(
            &mut converters,
            &mut voted,
            &mut thermostat,
            [320, 320, 320],
        );
        assert_eq!(decision, None, "inside the band nothing should change");

        // Over the top: the heaters go off, once.
        let decision = pass(
            &mut converters,
            &mut voted,
            &mut thermostat,
            [360, 360, 360],
        );
        assert_eq!(decision, Some(Out::byte(b'0')));
        let decision = pass(
            &mut converters,
            &mut voted,
            &mut thermostat,
            [365, 365, 365],
        );
        assert_eq!(decision, None, "the same decision should not be repeated");

        // And the ground asks for a warmer cabin, which the next pass acts on.
        thermostat.set_setpoint(300.0);
        let decision = pass(
            &mut converters,
            &mut voted,
            &mut thermostat,
            [365, 365, 365],
        );
        assert_eq!(
            decision,
            Some(Out::byte(b'1')),
            "a higher setpoint should call for the heaters again"
        );
    }

    /// Three different sensors averaged give one temperature.
    ///
    /// The example's actual arrangement: two parts of one make and a third of
    /// another, each with its own conversion, averaged into one number.
    #[test]
    fn three_unlike_sensors_average_to_one_temperature() {
        use tcslibgs::node::vehicle::compute::{
            Algorithm, Format, ToKelvinParams, VoteParams,
        };

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
            inputs: vec![
                "in0".to_string(),
                "in1".to_string(),
                "in2".to_string(),
            ],
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
        assert!(
            said.contains("to_kelvin"),
            "it should say what there is: {said}"
        );
    }

    /// A function nobody has written is refused by name.
    #[test]
    fn an_unknown_function_is_refused() {
        let mut attributes = BTreeMap::new();
        attributes
            .insert("function".to_string(), tcslibgs::Scalar::of("sorcery"));
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
    /// Which is in `tcslibgs::node::vehicle::compute` now, so the ground tool
    /// applies it too: what this holds is that `make` goes through those rules
    /// rather than having a second set of its own.
    #[test]
    fn a_parameter_with_no_sensible_default_is_required() {
        let mut attributes = BTreeMap::new();
        attributes
            .insert("function".to_string(), tcslibgs::Scalar::of("to_kelvin"));
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
