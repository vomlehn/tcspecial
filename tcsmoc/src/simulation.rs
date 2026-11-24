//! Simulation done by the ground: what a MOC program does to stand for
//! something that is not there -- a delay on the link as long as the light
//! time to the Moon, say -- allowed only in a run that simulates.
//!
//! Simulation belongs on the ground, in the MOC's software, rather than
//! aboard, so that none of it can reach the flight software. But a ground
//! station that delays or damages what crosses its link in a flight would
//! be as wrong as a vehicle that did, so each such feature asks here first:
//! the run's simulation mode, stated in its simulator file as `simulate` in
//! the `tcspecial` section -- see `tcsprotocol::simulate` -- must not be
//! `none`, the flight setting. The same rule the vehicle keeps for its own
//! test code, under "Test Code Never Flies" in `docs/tcspecial-design.rst`,
//! and in the same words; the Python ground's library has it too, as
//! `tcsmoc.simulation.allow`.

use tcsprotocol::simulate::{stated, Mode};

/// Allows a simulation feature of the ground's in a run, or refuses it.
///
/// # Arguments
///
/// * `feature` - What the feature is, as the program's user asked for it:
///               "--delay=1.3", say.
/// * `mode`    - The run's simulation mode.
/// * `sim`     - The simulator file that states it; `None` where the run
///               has none.
///
/// # Errors
///
/// Under the flight setting, saying what was asked, what the run is, and
/// what to change.
pub fn allow(
    feature: &str,
    mode: Mode,
    sim: Option<&str>,
) -> Result<(), String> {
    if !mode.is_flight() {
        return Ok(());
    }
    let whose = match sim {
        Some(sim) => format!("{sim} states {}, or no mode", stated(mode)),
        None => "there is no simulator file".to_string(),
    };
    Err(format!(
        "{feature} is simulation, done by the ground, and {whose}, so this \
         run is the flight setting, where nothing is simulated: state {} or \
         {} in the simulator file's tcspecial section, or leave {feature} \
         out",
        stated(Mode::Some),
        stated(Mode::All)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A simulated run allows the ground's simulation; the flight setting
    /// refuses it, naming the feature, the file and what to state.
    #[test]
    fn only_a_simulated_run_allows_it() {
        assert_eq!(allow("--delay=1.3", Mode::All, Some("x-sim.yaml")), Ok(()));
        assert_eq!(allow("--delay=1.3", Mode::Some, None), Ok(()));
        let said =
            allow("--delay=1.3", Mode::None, Some("x-sim.yaml")).unwrap_err();
        assert!(said.starts_with("--delay=1.3 is simulation"), "{said}");
        assert!(said.contains("x-sim.yaml states simulate: none"), "{said}");
        assert!(said.contains("simulate: some or simulate: all"), "{said}");
        let said = allow("--delay=1.3", Mode::None, None).unwrap_err();
        assert!(said.contains("there is no simulator file"), "{said}");
    }
}
