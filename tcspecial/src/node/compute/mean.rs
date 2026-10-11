//! The arithmetic mean of several readings.
//!
//! Three sensors in a group are read one after another and their converted
//! readings arrive here as three separate arrivals. This waits for all of
//! them and then emits one number.
//!
//! Waiting for all of them is the whole of the difficulty, and getting it
//! wrong has a particular flavour of wrongness: a mean that emitted on every
//! arrival would average the new reading against two from the pass before, and
//! the answer would be plausible -- a temperature, in range, moving smoothly.
//! Nobody would notice. So this holds its inputs, emits when the set is
//! complete, and then forgets them, so that every number it produces is made
//! of one pass's readings and no other's.

use std::collections::BTreeMap;

use super::{as_value, from_value, Function};

/// The mean over a fixed set of named inputs.
#[derive(Debug)]
pub struct Mean {
    /// What has arrived this pass, by input name. An input not in here has
    /// not arrived yet.
    arrived: BTreeMap<String, f64>,
    /// The inputs this is waiting for, which the routing found bound.
    expected: Vec<String>,
}

impl Mean {
    /// A mean over these inputs.
    ///
    /// The names come from the routing rather than from the configuration's
    /// own `inputs` attribute, because what matters is what is actually linked
    /// here: a node that says three and has two bound is a configuration the
    /// checking refuses, and one that says nothing and has three bound is a
    /// mean over three.
    pub fn of(inputs: &[&str]) -> Result<Mean, String> {
        if inputs.is_empty() {
            return Err("a mean needs inputs, and nothing is linked to this one"
                .to_string());
        }
        Ok(Mean {
            arrived: BTreeMap::new(),
            expected: inputs.iter().map(|i| i.to_string()).collect(),
        })
    }

    /// How many are still to come this pass.
    pub fn waiting_for(&self) -> usize {
        self.expected.len() - self.arrived.len()
    }
}

impl Function for Mean {
    fn give(&mut self, input: Option<&str>, data: &[u8]) -> Result<Option<Vec<u8>>, String> {
        let input = input.ok_or_else(|| {
            "a mean takes named inputs, and this arrived without a name: a link \
             into it is written to.in0 and not to"
                .to_string()
        })?;

        if !self.expected.iter().any(|e| e == input) {
            return Err(format!(
                "\"{input}\" is not one of this mean's inputs ({})",
                self.expected.join(", ")
            ));
        }

        let value = from_value(data, input)?;

        // A second arrival at one input before the set is complete. The pass
        // is abandoned rather than the old value kept or the new one dropped:
        // either choice would average two readings from one sensor and none
        // from another, and say nothing about it.
        if self.arrived.insert(input.to_string(), value).is_some() {
            let partial = self.arrived.len();
            self.arrived.clear();
            self.arrived.insert(input.to_string(), value);
            return Err(format!(
                "\"{input}\" arrived twice before the other {} did; the pass is \
                 abandoned rather than averaged over the wrong readings",
                self.expected.len() - partial + 1
            ));
        }

        if self.arrived.len() < self.expected.len() {
            return Ok(None);
        }

        let total: f64 = self.arrived.values().sum();
        let mean = total / self.arrived.len() as f64;
        // Forgotten, so the next number is made of the next pass's readings
        // and no part of this one's.
        self.arrived.clear();
        Ok(Some(as_value(mean)))
    }

    fn name(&self) -> &'static str {
        "mean"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing comes out until every input has arrived.
    ///
    /// The test this module exists for. A mean that emitted early would
    /// average a new reading against stale ones and produce a plausible
    /// number nobody would question.
    #[test]
    fn nothing_is_emitted_until_the_set_is_complete() {
        let mut mean = Mean::of(&["in0", "in1", "in2"]).unwrap();

        assert_eq!(mean.give(Some("in0"), b"290").unwrap(), None);
        assert_eq!(mean.waiting_for(), 2);
        assert_eq!(mean.give(Some("in1"), b"293").unwrap(), None);
        assert_eq!(mean.waiting_for(), 1);

        let out = mean
            .give(Some("in2"), b"296")
            .unwrap()
            .expect("the third completes it");
        assert_eq!(from_value(&out, "the mean").unwrap(), 293.0);
        assert_eq!(mean.waiting_for(), 3, "it should be waiting again");
    }

    /// Each pass is made of its own readings.
    #[test]
    fn a_pass_is_not_averaged_against_the_one_before() {
        let mut mean = Mean::of(&["in0", "in1"]).unwrap();

        mean.give(Some("in0"), b"100").unwrap();
        let first = mean.give(Some("in1"), b"200").unwrap().unwrap();
        assert_eq!(from_value(&first, "x").unwrap(), 150.0);

        // A second pass of quite different readings: the answer is theirs
        // alone.
        mean.give(Some("in0"), b"300").unwrap();
        let second = mean.give(Some("in1"), b"400").unwrap().unwrap();
        assert_eq!(
            from_value(&second, "x").unwrap(),
            350.0,
            "the second pass was averaged against the first"
        );
    }

    /// Arrivals in any order give the same answer.
    ///
    /// Which order they come in is the devices' business, not the mean's.
    #[test]
    fn the_order_of_arrival_does_not_matter() {
        let mut forwards = Mean::of(&["in0", "in1", "in2"]).unwrap();
        forwards.give(Some("in0"), b"1").unwrap();
        forwards.give(Some("in1"), b"2").unwrap();
        let one = forwards.give(Some("in2"), b"3").unwrap().unwrap();

        let mut backwards = Mean::of(&["in0", "in1", "in2"]).unwrap();
        backwards.give(Some("in2"), b"3").unwrap();
        backwards.give(Some("in1"), b"2").unwrap();
        let other = backwards.give(Some("in0"), b"1").unwrap().unwrap();

        assert_eq!(one, other);
        assert_eq!(from_value(&one, "x").unwrap(), 2.0);
    }

    /// A sensor arriving twice before the others abandons the pass.
    ///
    /// The alternative is averaging two readings from one sensor and none
    /// from another, which is a wrong answer that looks like a right one.
    #[test]
    fn a_repeated_input_abandons_the_pass_rather_than_guessing() {
        let mut mean = Mean::of(&["in0", "in1", "in2"]).unwrap();
        mean.give(Some("in0"), b"100").unwrap();
        mean.give(Some("in1"), b"200").unwrap();

        let said = mean
            .give(Some("in0"), b"101")
            .expect_err("a second arrival at one input");
        assert!(said.contains("arrived twice"), "{said}");

        // The pass started again from the repeated reading, so the next
        // complete set is a whole one.
        assert_eq!(mean.waiting_for(), 2);
        mean.give(Some("in1"), b"201").unwrap();
        let out = mean.give(Some("in2"), b"301").unwrap().unwrap();
        assert_eq!(from_value(&out, "x").unwrap(), 201.0);
    }

    /// An arrival with no input name is refused, and says how to write one.
    #[test]
    fn an_unnamed_arrival_is_refused() {
        let mut mean = Mean::of(&["in0"]).unwrap();
        let said = mean.give(None, b"1").expect_err("no input name");
        assert!(said.contains("to.in0"), "it should say how: {said}");
    }

    /// An input nobody declared is refused by name.
    #[test]
    fn an_unexpected_input_is_refused() {
        let mut mean = Mean::of(&["in0", "in1"]).unwrap();
        let said = mean.give(Some("in7"), b"1").expect_err("not an input");
        assert!(said.contains("in7"), "{said}");
        assert!(said.contains("in0, in1"), "it should say what there is: {said}");
    }

    /// A reading that is not a number is refused and does not count.
    #[test]
    fn a_reading_that_is_not_a_number_does_not_fill_an_input() {
        let mut mean = Mean::of(&["in0", "in1"]).unwrap();
        assert!(mean.give(Some("in0"), b"warm").is_err());
        assert_eq!(
            mean.waiting_for(),
            2,
            "a refused reading should not count as arrived"
        );
    }

    /// A mean with nothing linked to it is refused at the start.
    #[test]
    fn a_mean_over_nothing_is_refused() {
        let said = Mean::of(&[]).expect_err("no inputs");
        assert!(said.contains("needs inputs"), "{said}");
    }
}
