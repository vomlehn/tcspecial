//! One value out of several readings of the same thing.
//!
//! A trio of sensors measures one quantity -- the same place, separated only
//! because two sensors cannot occupy one point -- and this turns their three
//! readings into the one value the rest of the configuration acts on. The
//! point of the trio is that it survives one of them failing, and the point of
//! this node is to be the thing that survives it.
//!
//! # Why not a mean
//!
//! This replaced a mean, which was the wrong arithmetic twice over.
//!
//! A mean includes the bad reading. Three sensors of one place disagree only
//! because one of them is wrong, and averaging carries that one into the
//! answer in proportion to how wrong it is: a sensor reading twenty kelvin
//! high moves the answer by seven. The median and the closest-mean both leave
//! an outlier out, which is what makes a trio worth having.
//!
//! And a mean waited for all of its inputs before emitting anything, which
//! turns one dead sensor into no output at all -- the exact failure the trio
//! exists to tolerate. This keeps the last reading from each input and
//! produces a value on every arrival, so a sensor that has stopped reporting
//! leaves its last reading in place and is outvoted by the two that are
//! working.
//!
//! The mean's own comment argued the other way, and it was right about a mean
//! over three *different* things: emitting on every arrival would average a
//! new reading against two from the pass before. For three readings of one
//! quantity that is not a flaw -- the latest reading from each sensor is
//! simply the best estimate of that quantity available at that moment.
//!
//! # Before the first vote
//!
//! Nothing is produced until every input has reported once. There is nothing
//! to vote on before then: a vote over two of three is a vote with no
//! majority, and a vote over one is that sensor's word taken on trust, which
//! is what the trio was built to avoid.

use std::collections::BTreeMap;

use tcslibgs::node::vehicle::compute::{Algorithm, VoteParams};

use super::{as_value, from_value, Function};

/// The one value from a set of named inputs, by the algorithm the node names.
#[derive(Debug)]
pub struct Vote {
    /// The last reading from each input, by input name. An input not in here
    /// has not reported yet.
    ///
    /// Kept rather than cleared after a vote: the readings are of one
    /// quantity, so the last from each is the best that is known about it
    /// until a newer one arrives.
    latest: BTreeMap<String, f64>,
    /// The inputs this votes over, which the routing found bound.
    expected: Vec<String>,
    /// How the one value is picked out of them.
    algorithm: Algorithm,
}

impl Vote {
    /// A vote over the inputs the node declares.
    ///
    /// The declared names, which `Graph::check` has compared with the links
    /// that arrive: a vote over three sensors whose third link was deleted has
    /// two inputs, each bound exactly once, and the declaration is what makes
    /// that a configuration fault rather than a vote over two. Nothing to
    /// refuse here -- that a vote is over at least three inputs, each named
    /// once, and names an algorithm it knows, is
    /// `tcslibgs::node::vehicle::compute`'s rule.
    pub fn of(params: VoteParams) -> Vote {
        Vote {
            latest: BTreeMap::new(),
            expected: params.inputs,
            algorithm: params.algorithm,
        }
    }

    /// How many inputs have yet to report for the first time.
    ///
    /// Nought once every one of them has, and nought for ever after: an input
    /// that has reported once has a reading, whether or not it is still
    /// arriving.
    pub fn waiting_for(&self) -> usize {
        self.expected.len() - self.latest.len()
    }

    /// The value this set of readings comes to.
    ///
    /// Taken in sorted order, which is what both algorithms are about: the
    /// middle of them, or the closest pair.
    fn verdict(&self) -> f64 {
        let mut readings: Vec<f64> = self.latest.values().copied().collect();
        // By partial order, there being no total order on a float. Nothing
        // here is NaN -- `from_value` refuses what will not parse as a number
        // -- so the comparison never falls back, and the fallback is written
        // as Equal rather than as a panic because a sort is not the place to
        // discover a bad reading.
        readings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        match self.algorithm {
            Algorithm::Median => middle(&readings),
            Algorithm::ClosestMean => closest_pair_mean(&readings),
        }
    }
}

/// The middle of a sorted set of readings.
///
/// For an even count, the mean of the two in the middle. An odd count is what
/// a vote wants -- three readings have a middle one and four do not -- and the
/// even case is defined rather than refused so that the arithmetic is total.
fn middle(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

/// The mean of the two readings closest to each other.
///
/// In sorted order the closest pair is always adjacent, so this is one walk
/// along the readings looking for the smallest gap. The first smallest gap
/// wins where two gaps are equal, which only matters for readings that are
/// equally spaced -- and then every pair gives the same answer as the median
/// anyway.
fn closest_pair_mean(sorted: &[f64]) -> f64 {
    let mut best = 0;
    let mut gap = f64::INFINITY;
    for i in 0..sorted.len().saturating_sub(1) {
        let theirs = sorted[i + 1] - sorted[i];
        if theirs < gap {
            gap = theirs;
            best = i;
        }
    }
    (sorted[best] + sorted[best + 1]) / 2.0
}

impl Function for Vote {
    fn give(&mut self, input: Option<&str>, data: &[u8]) -> Result<Option<Vec<u8>>, String> {
        let input = input.ok_or_else(|| {
            "a vote takes named inputs, and this arrived without a name: a link \
             into it is written to.in0 and not to"
                .to_string()
        })?;

        if !self.expected.iter().any(|e| e == input) {
            return Err(format!(
                "\"{input}\" is not one of this vote's inputs ({})",
                self.expected.join(", ")
            ));
        }

        // The newest reading from that input, replacing whatever it said
        // before. A second arrival from one input before the others have
        // reported is not a fault here, as it was for a mean: this votes over
        // the latest from each, so a sensor reporting twice simply has its
        // later reading used.
        self.latest
            .insert(input.to_string(), from_value(data, input)?);

        // Nothing to vote on until every input has reported once: see the
        // module comment.
        if self.latest.len() < self.expected.len() {
            return Ok(None);
        }

        // The verdict is one of the readings, or the mean of two of them,
        // so it is inside the range every reading came in by -- but it is
        // made into a value the one way everything else is.
        Ok(Some(as_value(self.verdict())?))
    }

    fn name(&self) -> &'static str {
        "vote"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temperature as it arrives on a link: four bytes of micro-kelvin.
    ///
    /// The refusal is `as_value`'s range, which these temperatures are all
    /// well inside; a test that meant to try the range says so.
    fn kelvin(number: f64) -> Vec<u8> {
        as_value(number).expect("a temperature a value carries")
    }

    /// A vote over three inputs, by the algorithm named.
    fn vote(algorithm: Algorithm) -> Vote {
        Vote::of(VoteParams {
            inputs: vec!["in0".to_string(), "in1".to_string(), "in2".to_string()],
            algorithm,
        })
    }

    /// What a vote says, as a number.
    fn said(out: Option<Vec<u8>>) -> f64 {
        from_value(&out.expect("a value"), "a vote").expect("a value")
    }

    /// Nothing is produced until every input has reported once.
    ///
    /// There is nothing to vote on before then: two of three readings have no
    /// majority between them, and one is that sensor's word taken on trust.
    #[test]
    fn nothing_is_produced_until_every_input_has_reported() {
        let mut vote = vote(Algorithm::Median);

        assert_eq!(vote.give(Some("in0"), &kelvin(290_f64)).unwrap(), None);
        assert_eq!(vote.waiting_for(), 2);
        assert_eq!(vote.give(Some("in1"), &kelvin(293_f64)).unwrap(), None);
        assert_eq!(vote.waiting_for(), 1);

        let out = vote.give(Some("in2"), &kelvin(296_f64)).unwrap();
        assert_eq!(said(out), 293.0, "the middle of 290, 293 and 296");
        assert_eq!(vote.waiting_for(), 0);
    }

    /// And after that, every arrival produces a value.
    ///
    /// The test this module exists for. A vote that waited for a full set
    /// would stop producing anything the moment one sensor stopped reporting,
    /// which is the failure a trio of sensors exists to tolerate.
    #[test]
    fn every_arrival_after_the_first_set_produces_a_value() {
        let mut vote = vote(Algorithm::Median);
        for (input, reading) in [
            ("in0", kelvin(290_f64)),
            ("in1", kelvin(293_f64)),
            ("in2", kelvin(296_f64)),
        ] {
            vote.give(Some(input), &reading).unwrap();
        }

        // One sensor reports again, and the vote is taken over its new
        // reading and the last from the other two.
        let out = vote.give(Some("in0"), &kelvin(294_f64)).unwrap();
        assert_eq!(said(out), 294.0, "the middle of 294, 293 and 296");

        // And again, from the same sensor: nothing is waiting for the others,
        // so the one that is still reporting keeps the value current.
        let out = vote.give(Some("in0"), &kelvin(299_f64)).unwrap();
        assert_eq!(said(out), 296.0, "the middle of 299, 293 and 296");
    }

    /// A sensor that has gone wrong is outvoted, by either algorithm.
    ///
    /// Which is the whole purpose. A mean over the three would have carried
    /// the bad reading into the answer in proportion to how bad it was.
    #[test]
    fn a_sensor_that_has_gone_wrong_is_outvoted() {
        for (algorithm, expected) in [
            // Sorted, the readings are 293, 293.4 and 313: the middle one is
            // 293.4, and the pair that agree are the first two, whose mean is
            // 293.2. Either way the sensor twenty kelvin high is left out.
            (Algorithm::Median, 293.4),
            (Algorithm::ClosestMean, 293.2),
        ] {
            let mut vote = vote(algorithm);
            vote.give(Some("in0"), &kelvin(293_f64)).unwrap();
            vote.give(Some("in1"), &kelvin(293.4_f64)).unwrap();
            // Twenty kelvin high, which is a sensor to replace rather than a
            // temperature to act on.
            let got = said(vote.give(Some("in2"), &kelvin(313_f64)).unwrap());
            assert!(
                (got - expected).abs() < 1e-9,
                "{}: {got} is not {expected}",
                algorithm.spelling(),
            );
        }
    }

    /// The closest-mean takes the pair that agree, wherever in the order they
    /// are.
    #[test]
    fn the_closest_mean_takes_the_pair_that_agree() {
        // The close pair is the two low ones, so the answer is below the
        // median rather than at it.
        let mut low = vote(Algorithm::ClosestMean);
        low.give(Some("in0"), &kelvin(290.0_f64)).unwrap();
        low.give(Some("in1"), &kelvin(290.2_f64)).unwrap();
        let got = said(low.give(Some("in2"), &kelvin(300.0_f64)).unwrap());
        assert!((got - 290.1).abs() < 1e-9, "{got}");

        // And the two high ones, which the median would miss the other way.
        let mut high = vote(Algorithm::ClosestMean);
        high.give(Some("in0"), &kelvin(280.0_f64)).unwrap();
        high.give(Some("in1"), &kelvin(299.8_f64)).unwrap();
        let got = said(high.give(Some("in2"), &kelvin(300.0_f64)).unwrap());
        assert!((got - 299.9).abs() < 1e-9, "{got}");
    }

    /// An input the node does not declare is refused, and so is one with no
    /// name.
    #[test]
    fn an_input_that_is_not_its_own_is_refused() {
        let mut vote = vote(Algorithm::Median);

        let said = vote
            .give(Some("in7"), &kelvin(290_f64))
            .expect_err("an input it does not have");
        assert!(said.contains("in7") && said.contains("in0"), "{said}");

        let said = vote
            .give(None, &kelvin(290_f64))
            .expect_err("no input name");
        assert!(said.contains("named inputs"), "{said}");
    }

    /// A reading that is not a number is refused, and does not become the
    /// input's value.
    #[test]
    fn a_reading_that_is_not_a_number_is_refused() {
        let mut vote = vote(Algorithm::Median);
        // Of a width no reading has: four bytes are a count of micro-kelvin
        // whatever they hold, so what this catches is a datum that is not one
        // rather than a word that is not a number. See `super::from_value`.
        assert!(vote.give(Some("in0"), b"warmish").is_err());
        assert_eq!(
            vote.waiting_for(),
            3,
            "a refused reading is not a reading: the input has still not reported"
        );
    }

    /// The even case is defined rather than refused, so the arithmetic is
    /// total.
    ///
    /// A vote wants an odd count -- three readings have a middle one and four
    /// do not -- and a configuration of four is not refused for it, so the
    /// median of four is the mean of the two in the middle.
    #[test]
    fn the_median_of_an_even_count_is_the_middle_pair() {
        assert_eq!(middle(&[1.0, 2.0, 3.0]), 2.0);
        assert_eq!(middle(&[1.0, 2.0, 4.0, 8.0]), 3.0);
        assert_eq!(closest_pair_mean(&[1.0, 2.0, 10.0]), 1.5);
    }
}
