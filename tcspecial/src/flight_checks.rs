//! The checks that no test code is flying, made before the first beacon.
//!
//! Test code is code for testing that must never run aboard: the visual
//! window (`--visual`). It is in a build only with a feature that never
//! flies, and is refused under `simulate: none`, the flight setting, even in
//! a build that has it. Every such check is made here, in [`check`], and
//! nowhere after it.
//!
//! What simulates the link to the ground -- a delay, bit errors -- is not
//! here, nor anywhere aboard: the ground does it, so that none of it can
//! reach the flight software. See `tcsmoc::loss` and `tcs-moc`'s relay.
//!
//! What the checks found is said in every beacon, as its flight check: see
//! [`tcsprotocol::beacon::FlightCheck`]. In every beacon rather than once,
//! because any one packet can be lost. A beacon can only be sent with a
//! [`Checked`], and only [`check`] makes one, so no beacon goes before every
//! check has been made: a MOC that hears `flight` hears it of checks that
//! are finished. A new piece of test code adds its check to [`check`].
//!
//! See "Test Code Never Flies" in `docs/tcspecial-design.rst`.

use tcsprotocol::beacon::FlightCheck;
use tcsprotocol::simulate::Mode;

/// The test code a run asks for, on its command line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Asked {
    /// Whether `--visual` was given: the window showing the graph.
    pub visual: bool,
}

/// That every check for test code has been made, and what they found.
///
/// Only [`check`] makes one, and a beacon cannot be sent without one: see
/// the module's documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checked {
    /// What the checks found, which every beacon says.
    found: FlightCheck,
}

impl Checked {
    /// What the checks found.
    ///
    /// # Returns
    ///
    /// [`FlightCheck::Flight`] for flight code and nothing else, and
    /// [`FlightCheck::Test`] otherwise.
    pub fn found(self) -> FlightCheck {
        self.found
    }

    /// A check for a test of the code beneath, which says `test` as a
    /// test's run would.
    ///
    /// # Returns
    ///
    /// The check.
    #[cfg(test)]
    pub fn for_a_test() -> Checked {
        Checked {
            found: FlightCheck::Test,
        }
    }
}

/// Every feature that never flies, by name: test code. A feature added for
/// test code is added here, and to [`never_flies`].
pub const NEVER_FLIES: [&str; 1] = ["visual"];

/// The features of this build that never fly, by name.
///
/// # Returns
///
/// Each one of [`NEVER_FLIES`] this build has; none in a build that may
/// fly.
pub fn never_flies() -> Vec<&'static str> {
    let built = [cfg!(feature = "visual")];
    NEVER_FLIES
        .iter()
        .zip(built)
        .filter_map(|(name, built)| built.then_some(*name))
        .collect()
}

/// Makes every check for test code.
///
/// Each piece of test code asked for is refused under the flight setting,
/// and refused where this build lacks it. The run is
/// flight code only where it is under the flight setting and this build has
/// no feature that never flies: a build that has one says `test` even where
/// none of it is asked for, because it is not the build that was meant to
/// fly.
///
/// # Arguments
///
/// * `mode`  - The run's simulation mode, from its simulator file.
/// * `asked` - The test code its command line asks for.
///
/// # Returns
///
/// That the checks were made, and what they found.
///
/// # Errors
///
/// Every reason the run is refused, each a line to say.
pub fn check(mode: Mode, asked: &Asked) -> Result<Checked, Vec<String>> {
    let mut refused = Vec::new();

    // First, so that what a flight launch meets does not depend on the
    // build: a run on real devices alone is either flight or a mistake about
    // which run this is.
    if asked.visual && mode.is_flight() {
        refused.push(format!(
            "--visual is for a run with simulated or dummy devices, and this \
             one is simulate: {mode}, the flight setting: state \
             simulate: some or simulate: all in the simulator file's \
             tcspecial section"
        ));
    } else if asked.visual && !cfg!(feature = "visual") {
        refused.push(
            "--visual is only in a tcspecial built with the visual feature, \
             which this one was not: build it with --features visual"
                .to_string(),
        );
    }

    if !refused.is_empty() {
        return Err(refused);
    }
    let found = if mode.is_flight() && never_flies().is_empty() {
        FlightCheck::Flight
    } else {
        FlightCheck::Test
    };
    Ok(Checked { found })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test code asked of a flight run is refused, whatever the build.
    #[test]
    fn test_code_is_refused_under_the_flight_setting() {
        let visual = Asked { visual: true };
        let said = check(Mode::None, &visual).unwrap_err();
        assert!(said[0].contains("the flight setting"), "{said:?}");
    }

    /// Flight is said only of the flight setting in a build that may fly.
    #[test]
    fn flight_is_said_only_of_a_flight_run_of_a_flight_build() {
        let nothing = Asked::default();
        let flight = check(Mode::None, &nothing).expect("nothing asked");
        let want = if never_flies().is_empty() {
            FlightCheck::Flight
        } else {
            FlightCheck::Test
        };
        assert_eq!(flight.found(), want);
        for mode in [Mode::Some, Mode::All] {
            let found = check(mode, &nothing).expect("nothing asked");
            assert_eq!(found.found(), FlightCheck::Test, "{mode}");
        }
    }
}
