//! The checks that no test code is flying, made before the first beacon.
//!
//! Test code is code for testing that must never run aboard: the visual
//! window (`--visual`) and bit errors on the link to the ground (the
//! simulator file's `gateway` section, its `bit_error_rate`). Each is in a
//! build only with a feature that never
//! flies, and each is refused under `--simulate=none`, the flight setting,
//! even in a build that has it. Every such check is made here, in [`check`],
//! and nowhere after it.
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
#[cfg(test)]
use tcsprotocol::sim_config::GatewaySeed;
use tcsprotocol::sim_config::{SimConfigFile, SimGateway};
use tcsprotocol::simulate::Mode;

/// The test code a run asks for: on its command line, and in the
/// simulator file beside its configuration.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Asked {
    /// Whether `--visual` was given: the window showing the graph.
    pub visual: bool,
    /// Bit errors on the link to the ground, where the simulator file states
    /// a rate: the file, for saying so, and its `gateway` section.
    pub bit_errors: Option<(String, SimGateway)>,
}

/// Finds the bit errors the simulator file for a configuration asks for.
///
/// The simulator file is the one tcs-sim and tcs-moc read for the
/// configuration: the one `PAYLOAD_SIM_YAML` names, and otherwise the one
/// named for it beside it. Not read at all by a build that may fly, run in
/// the flight setting: it has no bit errors to apply and nothing to refuse.
/// A file that cannot be read is refused by a build with the bit errors,
/// which cannot know what it asks, and left to tcs-sim by any other.
///
/// # Arguments
///
/// * `config` - The configuration's path.
/// * `mode`   - The run's simulation mode.
///
/// # Returns
///
/// The file and its `gateway` section, where the section states a rate;
/// `None` where it states none, or there is no file.
///
/// # Errors
///
/// Why the file could not be read, in a build with the bit errors.
pub fn bit_errors_for(
    config: &str,
    mode: Mode,
) -> Result<Option<(String, SimGateway)>, String> {
    let built = cfg!(feature = "bit-errors");
    if mode.is_flight() && never_flies().is_empty() {
        return Ok(None);
    }
    let sim = tcsos::env::var("PAYLOAD_SIM_YAML")
        .ok()
        .filter(|named| !named.is_empty())
        .or_else(|| tcsprotocol::config::sim_path_beside(config));
    let Some(sim) = sim else {
        return Ok(None);
    };
    match SimConfigFile::load(&sim) {
        Ok(file) => Ok(file
            .gateway
            .filter(|gateway| gateway.bit_error_rate.is_some())
            .map(|gateway| (sim, gateway))),
        Err(why) if built => Err(format!(
            "{sim} cannot be read, so whether it asks for bit errors on the \
             link to the ground cannot be known: {why}"
        )),
        Err(_) => Ok(None),
    }
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

/// What the checks armed, a line for the startup summary: the bit errors'
/// rate and seed. Kept for the caller to say, since the supervisor makes the
/// checks too and only the child it starts sends packets: see
/// [`armed`].
#[cfg(feature = "bit-errors")]
static ARMED: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// What the checks armed, for the startup summary.
///
/// # Returns
///
/// A line saying the bit errors' rate, seed and file, where they were
/// armed; `None` where nothing was, and always in a build without them.
pub fn armed() -> Option<String> {
    #[cfg(feature = "bit-errors")]
    {
        ARMED.get().cloned()
    }
    #[cfg(not(feature = "bit-errors"))]
    {
        None
    }
}

/// Every feature that never flies, by name: test code. A feature added for
/// test code is added here, and to [`never_flies`].
pub const NEVER_FLIES: [&str; 2] = ["visual", "bit-errors"];

/// The features of this build that never fly, by name.
///
/// # Returns
///
/// Each one of [`NEVER_FLIES`] this build has; none in a build that may
/// fly.
pub fn never_flies() -> Vec<&'static str> {
    let built = [cfg!(feature = "visual"), cfg!(feature = "bit-errors")];
    NEVER_FLIES
        .iter()
        .zip(built)
        .filter_map(|(name, built)| built.then_some(*name))
        .collect()
}

/// Makes every check for test code, and arms what is asked for and allowed.
///
/// Each piece of test code asked for is refused under the flight setting,
/// and refused where this build lacks it; what is left is armed. The run is
/// flight code only where it is under the flight setting and this build has
/// no feature that never flies: a build that has one says `test` even where
/// none of it is asked for, because it is not the build that was meant to
/// fly.
///
/// # Arguments
///
/// * `mode`  - The run's simulation mode, `--simulate`.
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
             one is --simulate={mode}, the flight setting: give \
             --simulate=some or --simulate=all with it"
        ));
    } else if asked.visual && !cfg!(feature = "visual") {
        refused.push(
            "--visual is only in a tcspecial built with the visual feature, \
             which this one was not: build it with --features visual"
                .to_string(),
        );
    }

    if let Some((sim, gateway)) = &asked.bit_errors {
        if mode.is_flight() {
            refused.push(format!(
                "{sim} asks for bit errors on the link to the ground, which \
                 are test code, and this run is --simulate={mode}, the \
                 flight setting: take bit_error_rate out of its gateway \
                 section, or give --simulate=some or --simulate=all"
            ));
        } else {
            arm_bit_errors(sim, *gateway, mode, &mut refused);
        }
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

/// Arms the bit errors, in a build that has them.
///
/// # Arguments
///
/// * `sim`     - The simulator file that asks for them.
/// * `gateway` - Its `gateway` section: the rate, and where the random
///               numbers start -- the default seed where it does not say,
///               and one from the system's source of randomness where it
///               says `random`.
/// * `mode`    - The run's simulation mode.
/// * `refused` - Where a refusal is added.
#[cfg(feature = "bit-errors")]
fn arm_bit_errors(
    sim: &str,
    gateway: SimGateway,
    mode: Mode,
    refused: &mut Vec<String>,
) {
    use tcsprotocol::sim_config::{GatewaySeed, DEFAULT_BIT_ERROR_SEED};

    let rate = gateway.bit_error_rate.unwrap_or(0.0);
    let (seed, chosen) = match gateway.seed {
        None => (DEFAULT_BIT_ERROR_SEED, "the default"),
        Some(GatewaySeed::Given(seed)) => (seed, "as stated"),
        Some(GatewaySeed::Random) => match getrandom::u64() {
            Ok(seed) => (seed, "drawn at random: state it to have it again"),
            Err(why) => {
                refused.push(format!(
                    "{sim} asks for a random seed for the bit errors, and \
                     none could be drawn from the system's source of \
                     randomness: {why}"
                ));
                return;
            }
        },
    };
    match crate::runtime::bit_errors::arm(rate, seed, mode) {
        Ok(()) => {
            // Set once, as the bit errors are: a second arming is refused
            // above.
            let _ = ARMED.set(format!(
                "bit errors: {rate} flips per bit, seed {seed} ({chosen}), \
                 from {sim}: test code"
            ));
        }
        Err(why) => refused.push(format!("{sim}: {why}")),
    }
}

/// Refuses the bit errors, in a build that has none: a test that asked
/// for a lossy link and was given a perfect one would pass for the wrong
/// reason.
///
/// # Arguments
///
/// * `sim`      - The simulator file that asks for them.
/// * `_gateway` - Its `gateway` section.
/// * `_mode`    - The run's simulation mode.
/// * `refused`  - Where the refusal is added.
#[cfg(not(feature = "bit-errors"))]
fn arm_bit_errors(
    sim: &str,
    _gateway: SimGateway,
    _mode: Mode,
    refused: &mut Vec<String>,
) {
    refused.push(format!(
        "{sim} asks for bit errors on the link to the ground, which are \
         only in a tcspecial built with the bit-errors feature, and this \
         one was not: build it with --features bit-errors, or take \
         bit_error_rate out of the file's gateway section"
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bit errors at a rate, as a simulator file would ask for them.
    ///
    /// # Arguments
    ///
    /// * `rate` - Flips per bit.
    ///
    /// # Returns
    ///
    /// The file's name and its gateway section.
    fn errors(rate: f64) -> Option<(String, SimGateway)> {
        Some((
            "x-sim.yaml".to_string(),
            SimGateway {
                bit_error_rate: Some(rate),
                seed: Some(GatewaySeed::Given(7)),
            },
        ))
    }

    /// Test code asked of a flight run is refused, whatever the build.
    #[test]
    fn test_code_is_refused_under_the_flight_setting() {
        let visual = Asked {
            visual: true,
            ..Asked::default()
        };
        let said = check(Mode::None, &visual).unwrap_err();
        assert!(said[0].contains("the flight setting"), "{said:?}");
        let lossy = Asked {
            bit_errors: errors(1e-6),
            ..Asked::default()
        };
        let said = check(Mode::None, &lossy).unwrap_err();
        assert!(said[0].contains("x-sim.yaml"), "{said:?}");
        assert!(said[0].contains("the flight setting"), "{said:?}");
        let both = Asked {
            visual: true,
            bit_errors: errors(1e-6),
        };
        assert_eq!(check(Mode::None, &both).unwrap_err().len(), 2);
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

    /// The rate is read from the simulator file beside the configuration,
    /// and a file stating none, or no file, asks for none.
    #[test]
    fn the_rate_is_read_from_the_simulator_file_beside() {
        let dir = tempfile::tempdir().expect("a directory");
        let config = dir.path().join("x.ytcs");
        let config = config.to_str().expect("a path");
        let sim = dir.path().join("x-sim.yaml");
        assert_eq!(bit_errors_for(config, Mode::All), Ok(None));
        tcsos::fs::write(&sim, "gateway:\n  bit_error_rate:\n")
            .expect("written");
        assert_eq!(bit_errors_for(config, Mode::All), Ok(None));
        tcsos::fs::write(&sim, "gateway:\n  bit_error_rate: 1e-6\n")
            .expect("written");
        let (named, gateway) = bit_errors_for(config, Mode::All)
            .expect("read")
            .expect("a rate");
        assert!(named.ends_with("x-sim.yaml"), "{named}");
        assert_eq!(gateway.bit_error_rate, Some(1e-6));
        // A build that may fly, in the flight setting, does not look.
        if never_flies().is_empty() {
            assert_eq!(bit_errors_for(config, Mode::None), Ok(None));
        }
    }

    /// What is armed is kept for the summary rather than said, so that
    /// the supervisor, which makes the checks too, does not say it as well
    /// as its child. A rate of nought, so that nothing else this process
    /// sends is lost.
    #[cfg(feature = "bit-errors")]
    #[test]
    fn what_is_armed_is_kept_for_the_summary() {
        let lossless = Asked {
            bit_errors: errors(0.0),
            ..Asked::default()
        };
        check(Mode::All, &lossless).expect("armed");
        let said = armed().expect("kept");
        assert!(said.contains("seed 7 (as stated)"), "{said}");
        assert!(said.contains("x-sim.yaml"), "{said}");
    }

    /// Nothing is armed in a build without the bit errors.
    #[cfg(not(feature = "bit-errors"))]
    #[test]
    fn nothing_is_armed_without_the_bit_errors() {
        assert_eq!(armed(), None);
    }

    /// A build without the bit errors says so when a file asks for them.
    #[cfg(not(feature = "bit-errors"))]
    #[test]
    fn a_build_without_bit_errors_refuses_them() {
        let lossy = Asked {
            bit_errors: errors(1e-6),
            ..Asked::default()
        };
        let said = check(Mode::All, &lossy).unwrap_err();
        assert!(said[0].contains("--features bit-errors"), "{said:?}");
    }
}
