//! Bit errors on the link to the vehicle, for a test of a ground station or
//! a vehicle against a link that loses packets: simulation done by the
//! ground.
//!
//! The run's simulator file's `gateway` section, its `bit_error_rate` in
//! flips per bit, has every packet that crosses the link lost as a bit error
//! would lose it: a packet of `n` bits, each flipped with probability `p`,
//! has at least one flipped with probability `1 - (1 - p)^n`, and a packet
//! with a flipped bit is dropped whole, as a link whose check fails drops
//! it. Up and down alike: the commands and what the ground sends through
//! the vehicle's gateway, going up, and the answers, the beacons and the
//! gateway's data, coming down. `n` is the packet as the ground reads or
//! writes it, eight bits a byte; what the transport adds under it is not
//! counted.
//!
//! Done here, on the ground, rather than aboard, so that the flight
//! software holds no injector, no random numbers and no rate. A ground
//! program arms it with [`arm_for`], once, for the run: refused under the
//! flight setting by [`crate::simulation::allow`]. Each of this library's
//! links -- commands and their answers, the beacon, the gateway's data and
//! what is sent up through it -- asks [`lost`] of every packet. Nothing is
//! lost in a program that arms nothing. Kept for each program, so that the
//! programs a web page runs side by side do not share it.

use tcsos::local::Local;
use tcsos::sync::Mutex;

use tcsprotocol::sim_config::{
    GatewaySeed, SimConfigFile, SimGateway, DEFAULT_BIT_ERROR_SEED,
};
use tcsprotocol::simulate::Mode;

/// The rate armed for this program, in flips per bit, and the state of its
/// random numbers: xorshift64*, which needs a state that is never nought.
/// `None`, and nothing lost, until [`arm`] is given a rate.
static ARMED: Local<Mutex<Option<(f64, u64)>>> =
    Local::new(Mutex::new(None), || Mutex::new(None));

/// Arms the bit errors for the rest of this program.
///
/// # Arguments
///
/// * `rate` - Flips per bit, from 0 to 1.
/// * `seed` - Where the random numbers start, so that a run can be
///            repeated packet for packet.
///
/// # Errors
///
/// Why it was not armed: a rate that is not a probability, or a rate armed
/// already.
pub fn arm(rate: f64, seed: u64) -> Result<(), String> {
    if !(0.0..=1.0).contains(&rate) {
        return Err(format!(
            "a bit error rate of {rate} is not a probability: give flips per \
             bit from 0 to 1, such as 1e-6"
        ));
    }
    let mut armed = ARMED
        .lock()
        .map_err(|_| "the bit errors' lock is poisoned")?;
    if armed.is_some() {
        return Err("the bit errors are armed already".to_string());
    }
    // Never nought, which xorshift would stay at.
    *armed = Some((rate, seed.max(1)));
    Ok(())
}

/// Arms the bit errors a run's simulator file asks for, where it asks for
/// any: the rate in its `gateway` section, from the seed it states, from
/// [`DEFAULT_BIT_ERROR_SEED`] where it states none, and from the clock
/// where it says `random`.
///
/// # Arguments
///
/// * `sim`  - The run's simulator file, where it has one.
/// * `mode` - The run's simulation mode, which must allow simulation.
///
/// # Returns
///
/// A line saying the rate, the seed and the file, for the program to say
/// as it starts; `None` where nothing is asked for, and nothing armed.
///
/// # Errors
///
/// Where the file cannot be read, where it asks for bit errors in the
/// flight setting, or where they cannot be armed.
pub fn arm_for(
    sim: Option<&str>,
    mode: Mode,
) -> Result<Option<String>, String> {
    let Some(sim) = sim else {
        return Ok(None);
    };
    let file = SimConfigFile::load(sim).map_err(|why| {
        format!(
            "{sim} cannot be read, so whether it asks for bit errors on the \
             link cannot be known: {why}"
        )
    })?;
    let Some(SimGateway {
        bit_error_rate: Some(rate),
        seed,
    }) = file.gateway
    else {
        return Ok(None);
    };
    crate::simulation::allow(
        &format!("bit errors on the link (bit_error_rate in {sim})"),
        mode,
        Some(sim),
    )?;
    let (seed, chosen) = match seed {
        None => (DEFAULT_BIT_ERROR_SEED, "the default"),
        Some(GatewaySeed::Given(seed)) => (seed, "as stated"),
        Some(GatewaySeed::Random) => (
            from_the_clock(),
            "drawn at random: state it to have it again",
        ),
    };
    arm(rate, seed).map_err(|why| format!("{sim}: {why}"))?;
    Ok(Some(format!(
        "bit errors: {rate} flips per bit, seed {seed} ({chosen}), from {sim}: \
         simulation done by the ground"
    )))
}

/// A seed drawn from the clock, for `seed: random`: the nanoseconds since
/// the epoch, mixed so that two runs a moment apart differ in every bit.
///
/// # Returns
///
/// The seed.
fn from_the_clock() -> u64 {
    let nanos = tcsos::time::SystemTime::now()
        .duration_since(tcsos::time::UNIX_EPOCH)
        .map_or(1, |since| since.as_nanos() as u64);
    // splitmix64's finish.
    let mut z = nanos.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The probability that a packet of `bytes` bytes has a flipped bit:
/// `1 - (1 - p)^bits`, worked in a form that keeps its precision for the
/// small rates a link has.
///
/// # Arguments
///
/// * `rate`  - Flips per bit.
/// * `bytes` - How long the packet is.
///
/// # Returns
///
/// The probability, from 0 to 1.
pub fn chance(rate: f64, bytes: usize) -> f64 {
    if rate >= 1.0 {
        return if bytes == 0 { 0.0 } else { 1.0 };
    }
    let bits = (bytes as f64) * 8.0;
    -(bits * (-rate).ln_1p()).exp_m1()
}

/// The next random number of a state, uniform from 0 up to but not 1, and
/// the state after it: xorshift64*.
///
/// # Arguments
///
/// * `state` - The state, never nought.
///
/// # Returns
///
/// The number, and the next state.
fn uniform(state: u64) -> (f64, u64) {
    let mut x = state;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    let drawn = x.wrapping_mul(0x2545_f491_4f6c_dd1d);
    // The top 53 bits, which a double holds exactly.
    ((drawn >> 11) as f64 / (1u64 << 53) as f64, x)
}

/// Whether a packet crossing the link is lost to a bit error.
///
/// # Arguments
///
/// * `bytes` - How long the packet is.
///
/// # Returns
///
/// `true` where it is to be dropped whole; `false` where nothing is armed.
pub fn lost(bytes: usize) -> bool {
    let Ok(mut armed) = ARMED.lock() else {
        return false;
    };
    let Some((rate, state)) = armed.as_mut() else {
        return false;
    };
    let (drawn, next) = uniform(*state);
    *state = next;
    drawn < chance(*rate, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The chance is `1 - (1 - p)^n`, at the rates a link has as well as
    /// the ends.
    #[test]
    fn the_chance_is_one_less_the_chance_of_no_flip() {
        assert_eq!(chance(0.0, 1000), 0.0);
        assert_eq!(chance(1.0, 1), 1.0);
        assert_eq!(chance(1.0, 0), 0.0);
        let near = |a: f64, b: f64| (a - b).abs() <= 1e-9 * b;
        let want = 1.0 - (1.0 - 1e-6f64).powi(800);
        assert!(near(chance(1e-6, 100), want), "{}", chance(1e-6, 100));
        // Small enough that 1 - p is 1 in a double, and still counted.
        assert!(chance(1e-18, 100) > 7.9e-16);
    }

    /// The numbers drawn are spread from 0 to 1, and the same seed draws
    /// the same numbers.
    #[test]
    fn the_numbers_are_uniform_and_repeatable() {
        let draw = |seed: u64| {
            let mut state = seed;
            (0..10_000)
                .map(|_| {
                    let (n, next) = uniform(state);
                    state = next;
                    n
                })
                .collect::<Vec<f64>>()
        };
        let drawn = draw(7);
        assert_eq!(drawn, draw(7));
        assert!(drawn.iter().all(|n| (0.0..1.0).contains(n)));
        let mean = drawn.iter().sum::<f64>() / drawn.len() as f64;
        assert!((0.48..0.52).contains(&mean), "{mean}");
    }

    /// A file asking for bit errors is refused in the flight setting, and
    /// one asking for none arms nothing.
    #[test]
    fn a_rate_is_refused_in_the_flight_setting() {
        let dir = tempfile::tempdir().expect("a directory");
        let sim = dir.path().join("x-sim.yaml");
        let named = sim.to_string_lossy().into_owned();
        tcsos::fs::write(&sim, "gateway:\n  bit_error_rate: 1e-6\n")
            .expect("written");
        let said = arm_for(Some(&named), Mode::None).unwrap_err();
        assert!(said.starts_with("bit errors on the link"), "{said}");
        tcsos::fs::write(&sim, "description: no rate\n").expect("written");
        assert_eq!(arm_for(Some(&named), Mode::All), Ok(None));
        assert_eq!(arm_for(None, Mode::All), Ok(None));
    }
}
