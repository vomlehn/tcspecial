//! The world's randomness: seeds, and quantities that wander.

use rand::rngs::StdRng;
use rand::Rng;
use rand_distr::StandardNormal;
use tcslib_protocol::node::physics::Wander;

/// A seed for one thing in a run, from the run's seed and the thing's name.
///
/// So that each component and each sensor draws from a generator of its own:
/// one shared generator would hand out its numbers in whatever order the
/// payloads' threads happened to ask, and a run would not vary the same way
/// twice.
///
/// # Arguments
///
/// * `seed` - The run's seed.
/// * `what` - The thing's name, such as `component:zone` or `sensor:temp_1`.
///
/// # Returns
///
/// The thing's seed: FNV-1a of its name, mixed with the run's.
pub fn seed_for(seed: u64, what: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in what.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash ^ seed
}

/// Steps a wander by its exact transition.
///
/// An Ornstein-Uhlenbeck process, a departure drawn back towards nought:
///
/// ```text
/// x(t + dt) = x * exp(-dt / theta) + size * sqrt(1 - exp(-2 dt / theta)) * z
/// ```
///
/// with `z` a standard normal draw, so how much it wanders does not depend
/// on how often it is stepped.
///
/// # Arguments
///
/// * `x`       - Where it is.
/// * `wander`  - Its size and time constant; one of no size stays at nought.
/// * `seconds` - How long to step it by.
/// * `rng`     - Where the normal draw comes from.
///
/// # Returns
///
/// Where it is `seconds` later.
pub fn wandered(
    x: f64,
    wander: &Wander,
    seconds: f64,
    rng: &mut StdRng,
) -> f64 {
    if wander.size <= 0.0 || seconds <= 0.0 {
        return x;
    }
    let keep = (-seconds / wander.time.as_secs_f64()).exp();
    let z: f64 = rng.sample(StandardNormal);
    x * keep + wander.size * (1.0 - keep * keep).sqrt() * z
}

/// Where a wander is when it starts: a draw from where a settled one would
/// be, since one started at nought would show a quiet beginning no real
/// quantity has.
///
/// # Arguments
///
/// * `wander` - Its size and time constant.
/// * `rng`    - Where the draw comes from.
///
/// # Returns
///
/// Where it starts; nought for one of no size, with nothing drawn.
pub fn settled(wander: &Wander, rng: &mut StdRng) -> f64 {
    if wander.size > 0.0 {
        wander.size * rng.sample::<f64, _>(StandardNormal)
    } else {
        0.0
    }
}
