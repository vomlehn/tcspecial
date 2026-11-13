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
//! Nothing is produced until a majority of the inputs -- two of three --
//! have reported. A vote over one is that sensor's word taken on trust,
//! which is what a trio is built to avoid; but waiting for every input was
//! worse, because a sensor dead from power-up then held the vote, and every
//! decision downstream of it, at nothing for the whole flight -- the single
//! failure the trio exists to survive. Two of three is the least that is
//! still a vote, and the most that one dead sensor cannot withhold.
//!
//! # Readings that have stopped arriving
//!
//! A reading is kept until a newer one from the same input replaces it, so
//! a sensor that stops is voted on at its last word. With `age` stated, a
//! reading older than that is no longer counted: the vote is over the
//! readings still current. If, once voting has begun, fewer than a majority
//! are current, nothing is produced, and each arrival says so as a failure
//! -- a vote that has lost its majority cannot do what it is for, and
//! silence would leave a decision downstream acting on a temperature that
//! is no longer being measured without anyone hearing of it. Without `age`,
//! readings never expire, as before it existed.

use std::time::{Duration, Instant};

use tcslib_protocol::node::vehicle::compute::{Algorithm, VoteParams};
use tcslib_protocol::unit::Unit;

use super::{from_value, Function, Out};

/// The one value from a set of named inputs, by the algorithm the node names.
#[derive(Debug)]
pub struct Vote {
    /// The last reading from each input, and when it arrived, by its place
    /// in `expected`. An input with none has not reported yet.
    ///
    /// Kept rather than cleared after a vote: the readings are of one
    /// quantity, so the last from each is the best that is known about it
    /// until a newer one arrives -- or until it is older than `age`. By
    /// place rather than by name, so that a reading is kept without its name
    /// being made again.
    latest: Vec<Option<(Instant, f64)>>,
    /// The inputs this votes over, which the routing found bound.
    expected: Vec<String>,
    /// How the one value is picked out of them.
    algorithm: Algorithm,
    /// How old a reading may be and still be counted, or `None` for no
    /// limit.
    age: Option<Duration>,
    /// What the readings are numbers of, which the vote's value is in too.
    unit: Unit,
    /// Whether a majority has been current at some arrival: before then, too
    /// few is a vote starting, and after, it is a vote that has lost its
    /// majority.
    voted: bool,
    /// Where the readings are sorted for a verdict: made with room for every
    /// input when the vote is, so that a verdict allocates nothing.
    sorted: Vec<f64>,
}

impl Vote {
    /// Makes a vote over the inputs the node declares.
    ///
    /// The declared names, which `Graph::check` has compared with the links
    /// that arrive: a vote over three sensors whose third link was deleted has
    /// two inputs, each bound exactly once, and the declaration is what makes
    /// that a configuration fault rather than a vote over two. Nothing to
    /// refuse here -- that a vote is over at least three inputs, each named
    /// once, and names an algorithm it knows, is
    /// `tcslib_protocol::node::vehicle::compute`'s rule.
    ///
    /// # Arguments
    ///
    /// * `params` - The inputs' names, the algorithm to vote by, and how old
    ///              a reading may be.
    /// * `unit`   - What the readings are numbers of.
    ///
    /// # Returns
    ///
    /// The vote, with no input reported yet.
    pub fn of(params: VoteParams, unit: Unit) -> Vote {
        let inputs = params.inputs.len();
        Vote {
            latest: vec![None; inputs],
            expected: params.inputs,
            algorithm: params.algorithm,
            age: params.age,
            unit,
            voted: false,
            sorted: Vec::with_capacity(inputs),
        }
    }

    /// How many current readings a vote needs: more than half its inputs.
    ///
    /// # Returns
    ///
    /// Two for three inputs, three for four or five.
    pub fn majority(&self) -> usize {
        self.expected.len() / 2 + 1
    }

    /// Takes one reading, arriving now as far as the vote can tell.
    ///
    /// What `Function::give` does, with the time it arrived given rather than
    /// read from the clock, so that what `age` does can be tested.
    ///
    /// # Arguments
    ///
    /// * `input` - The input it arrived at.
    /// * `data`  - The reading.
    /// * `now`   - When it arrived.
    ///
    /// # Returns
    ///
    /// The vote's value, or `None` while fewer than a majority have reported
    /// for the first time.
    ///
    /// # Errors
    ///
    /// A message for a reading at no input or one the vote is not over, a
    /// reading that is not a value, and -- once voting has begun -- fewer
    /// than a majority of current readings.
    pub fn give_at(
        &mut self,
        input: Option<&str>,
        data: &[u8],
        now: Instant,
    ) -> Result<Option<Out>, String> {
        let input = input.ok_or_else(|| {
            "a vote takes named inputs, and this arrived without a name: a \
             link into it is written to.in0 and not to"
                .to_string()
        })?;

        let Some(at) = self.expected.iter().position(|e| e == input) else {
            return Err(format!(
                "\"{input}\" is not one of this vote's inputs ({})",
                self.expected.join(", ")
            ));
        };

        // The newest reading from that input, replacing whatever it said
        // before: this votes over the latest from each, so a sensor
        // reporting twice simply has its later reading used.
        let reading = from_value(data, self.unit, input)?;
        self.latest[at] = Some((now, reading));

        // The readings that still count, sorted for a verdict.
        let age = self.age;
        self.sorted.clear();
        self.sorted.extend(self.latest.iter().flatten().filter_map(
            |&(when, reading)| {
                let current = age.is_none_or(|age| {
                    now.saturating_duration_since(when) <= age
                });
                current.then_some(reading)
            },
        ));

        let majority = self.majority();
        if self.sorted.len() < majority {
            if !self.voted {
                // Still starting: see the module comment.
                return Ok(None);
            }
            // The one place this allocates as it runs, and only while the
            // vote has lost its majority, which is a fault to be heard of
            // and not a reading to be voted on.
            return Err(format!(
                "only {} of {} inputs have reported within {}: a vote needs \
                 {majority}, and nothing is voted until they have",
                self.sorted.len(),
                self.expected.len(),
                age.map_or_else(String::new, |age| format!("{age:?}")),
            ));
        }
        self.voted = true;

        // The verdict is one of the readings, or the mean of two of them,
        // so it is inside the range every reading came in by -- but it is
        // made into a value the one way everything else is.
        let unit = self.unit;
        Ok(Some(Out::value(self.verdict(), unit)?))
    }

    /// Works out the value the current readings come to.
    ///
    /// Taken in sorted order, which is what both algorithms are about: the
    /// middle of them, or the closest pair.
    ///
    /// # Returns
    ///
    /// The value the vote's algorithm picks from `sorted`.
    ///
    /// # Panics
    ///
    /// If `sorted` holds fewer readings than the algorithm needs: one for a
    /// median, two for a closest mean. `give_at` asks only with a majority,
    /// and a vote is over at least three inputs, so a majority is two.
    fn verdict(&mut self) -> f64 {
        let readings = &mut self.sorted;
        // By partial order, there being no total order on a float. Nothing
        // here is NaN -- `from_value` refuses what will not parse as a number
        // -- so the comparison never falls back, and the fallback is written
        // as Equal rather than as a panic because a sort is not the place to
        // discover a bad reading.
        readings.sort_by(|a, b| {
            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
        });

        match self.algorithm {
            Algorithm::Median => middle(readings),
            Algorithm::ClosestMean => closest_pair_mean(readings),
        }
    }
}

/// Finds the middle of a sorted set of readings.
///
/// For an even count, the mean of the two in the middle. An odd count is what
/// a vote wants -- three readings have a middle one and four do not -- and the
/// even case is defined rather than refused so that the arithmetic is total.
///
/// # Arguments
///
/// * `sorted` - The readings, in ascending order.
///
/// # Returns
///
/// The middle reading, or the mean of the middle two.
///
/// # Panics
///
/// If `sorted` is empty.
fn middle(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

/// Finds the mean of the two readings closest to each other.
///
/// In sorted order the closest pair is always adjacent, so this is one walk
/// along the readings looking for the smallest gap. The first smallest gap
/// wins where two gaps are equal, which only matters for readings that are
/// equally spaced -- and then every pair gives the same answer as the median
/// anyway.
///
/// # Arguments
///
/// * `sorted` - The readings, in ascending order.
///
/// # Returns
///
/// The mean of the closest adjacent pair.
///
/// # Panics
///
/// If `sorted` holds fewer than two readings.
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
    fn give(
        &mut self,
        input: Option<&str>,
        data: &[u8],
    ) -> Result<Option<Out>, String> {
        self.give_at(input, data, Instant::now())
    }

    fn name(&self) -> &'static str {
        "vote"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::vehicle::compute::as_value;

    /// A temperature as it arrives on a link: four bytes of micro-kelvin.
    ///
    /// The refusal is `as_value`'s range, which these temperatures are all
    /// well inside; a test that meant to try the range says so.
    fn kelvin(number: f64) -> Vec<u8> {
        as_value(number, tcslib_protocol::unit::Unit::Kelvin)
            .expect("a temperature a value carries")
    }

    /// Makes a vote over three inputs, `in0` to `in2`.
    ///
    /// # Arguments
    ///
    /// * `algorithm` - The algorithm to vote by.
    ///
    /// # Returns
    ///
    /// The vote.
    fn vote(algorithm: Algorithm) -> Vote {
        aged(algorithm, None)
    }

    /// Makes a vote over three inputs, `in0` to `in2`, whose readings count
    /// for so long.
    ///
    /// # Arguments
    ///
    /// * `algorithm` - The algorithm to vote by.
    /// * `age`       - How old a reading may be, or `None` for no limit.
    ///
    /// # Returns
    ///
    /// The vote.
    fn aged(algorithm: Algorithm, age: Option<Duration>) -> Vote {
        Vote::of(
            VoteParams {
                inputs: vec![
                    "in0".to_string(),
                    "in1".to_string(),
                    "in2".to_string(),
                ],
                algorithm,
                age,
            },
            tcslib_protocol::unit::Unit::Kelvin,
        )
    }

    /// What a vote says, as a number.
    ///
    /// # Arguments
    ///
    /// * `out` - What the vote produced.
    ///
    /// # Returns
    ///
    /// The number, in kelvin.
    fn said(out: Option<Out>) -> f64 {
        from_value(
            &out.expect("a value"),
            tcslib_protocol::unit::Unit::Kelvin,
            "a vote",
        )
        .expect("a value")
    }

    /// Nothing is produced until a majority has reported, and from then
    /// on the vote is over whatever has.
    ///
    /// One reading is that sensor's word taken on trust; two of three is a
    /// vote, and is what one dead sensor cannot withhold.
    #[test]
    fn nothing_is_produced_until_a_majority_has_reported() {
        let mut vote = vote(Algorithm::Median);
        assert_eq!(vote.majority(), 2);

        assert_eq!(vote.give(Some("in0"), &kelvin(290_f64)).unwrap(), None);
        let out = vote.give(Some("in1"), &kelvin(294_f64)).unwrap();
        assert_eq!(said(out), 292.0, "the middle of 290 and 294");

        let out = vote.give(Some("in2"), &kelvin(296_f64)).unwrap();
        assert_eq!(said(out), 294.0, "the middle of 290, 294 and 296");
    }

    /// A sensor dead from the start does not hold the vote at nothing.
    ///
    /// The fault this rule exists for: waiting for every input turned one
    /// sensor that never reported into no temperature, and no decision, for
    /// the whole flight.
    #[test]
    fn a_sensor_dead_from_the_start_does_not_stop_the_vote() {
        let mut vote = vote(Algorithm::Median);
        for reading in [290_f64, 291_f64, 292_f64] {
            vote.give(Some("in0"), &kelvin(reading)).unwrap();
            let out = vote.give(Some("in1"), &kelvin(reading + 1.0)).unwrap();
            assert!(out.is_some(), "in2 never reports, and two still vote");
        }
    }

    /// With `age`, a reading older than that is not counted, and a vote that
    /// has lost its majority says so on every arrival rather than voting on
    /// one sensor's word; it votes again once a majority is current.
    #[test]
    fn a_reading_older_than_age_is_not_counted() {
        let mut vote = aged(Algorithm::Median, Some(Duration::from_secs(2)));
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);

        vote.give_at(Some("in0"), &kelvin(290_f64), at(0)).unwrap();
        vote.give_at(Some("in1"), &kelvin(292_f64), at(0)).unwrap();
        let out = vote.give_at(Some("in2"), &kelvin(320_f64), at(1)).unwrap();
        assert_eq!(said(out), 292.0, "all three are current");

        // At three seconds in0 and in1 are older than two: in2 alone is not
        // a majority, and that is a failure to be heard, not a vote.
        let why = vote
            .give_at(Some("in2"), &kelvin(320_f64), at(3))
            .expect_err("one current reading is no majority");
        assert!(why.contains("only 1 of 3"), "{why}");

        // in0 reports again: two current, and the vote is over those two.
        let out = vote.give_at(Some("in0"), &kelvin(291_f64), at(3)).unwrap();
        assert_eq!(said(out), 305.5, "the middle of 291 and 320");
    }

    /// Without `age`, a reading stands until its input sends another, however
    /// long that is.
    #[test]
    fn without_age_a_reading_never_expires() {
        let mut vote = vote(Algorithm::Median);
        let t0 = Instant::now();
        vote.give_at(Some("in0"), &kelvin(290_f64), t0).unwrap();
        vote.give_at(Some("in1"), &kelvin(292_f64), t0).unwrap();
        let late = t0 + Duration::from_secs(3600);
        let out = vote.give_at(Some("in2"), &kelvin(294_f64), late).unwrap();
        assert_eq!(said(out), 292.0, "an hour-old reading still counts");
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
            vote.give(Some("in1"), &kelvin(290_f64)).unwrap(),
            None,
            "a refused reading is not a reading: in0 has still not reported, \
             so in1 alone is no majority"
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
