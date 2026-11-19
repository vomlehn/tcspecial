//! Bit errors on the link to the ground, for a test: test code that never
//! flies.
//!
//! The simulator file's `gateway` section, its `bit_error_rate` in flips per
//! bit, has every packet that crosses the link to the ground lost as a bit
//! error would lose it: a packet of `n` bits,
//! each flipped with probability `p`, has at least one flipped with
//! probability `1 - (1 - p)^n`, and a packet with a flipped bit is dropped
//! whole, as a link whose check fails drops it. Up and down alike: the
//! commands and the gateway's data arriving, and the answers, the beacons
//! and the gateway's data leaving. `n` is the packet as tcspecial reads or
//! writes it, eight bits a byte; what the transport adds under it is not
//! counted.
//!
//! Only in a build with the `bit-errors` feature, which never flies. A build
//! without it has [`lost`] answer `false`, which the compiler takes out where
//! it is asked, so the flight software holds no injector, no random numbers
//! and no rate: nothing to switch on by mistake. A build with it refuses a
//! rate under `--simulate=none`, the flight setting, here and again in
//! [`crate::flight_checks`], and every beacon it sends says `test` whatever
//! it is asked: see "Test Code Never Flies" in `docs/tcspecial-design.rst`.

#[cfg(feature = "bit-errors")]
pub use built::*;

#[cfg(not(feature = "bit-errors"))]
/// Whether a packet is lost to a bit error: never, in a build without the
/// `bit-errors` feature.
///
/// # Arguments
///
/// * `_bytes` - How long the packet is.
///
/// # Returns
///
/// `false`.
#[inline(always)]
pub const fn lost(_bytes: usize) -> bool {
    false
}

#[cfg(feature = "bit-errors")]
mod built {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::OnceLock;

    use tcsprotocol::simulate::Mode;

    /// The rate armed for this process, in flips per bit, and the state of
    /// its random numbers. Unset, and nothing lost, until [`arm`] is given
    /// a rate.
    static ARMED: OnceLock<f64> = OnceLock::new();

    /// The random numbers' state: xorshift64*, which needs a state that is
    /// never nought. Advanced without a lock, since the beacons, the command
    /// links and the gateway's node each draw from their own thread.
    static STATE: AtomicU64 = AtomicU64::new(0x9e37_79b9_7f4a_7c15);

    /// Arms the bit errors for the rest of this process.
    ///
    /// # Arguments
    ///
    /// * `rate` - Flips per bit, from 0 to 1.
    /// * `seed` - Where the random numbers start, so that a run can be
    ///            repeated packet for packet.
    /// * `mode` - The run's simulation mode.
    ///
    /// # Returns
    ///
    /// Nothing, once armed.
    ///
    /// # Errors
    ///
    /// Why it was not armed: the flight setting, a rate that is not a
    /// probability, or a rate armed already.
    pub fn arm(rate: f64, seed: u64, mode: Mode) -> Result<(), String> {
        if mode.is_flight() {
            return Err(format!(
                "bit errors are test code, and this run is \
                 --simulate={mode}, the flight setting: give \
                 --simulate=some or --simulate=all with it"
            ));
        }
        if !(0.0..=1.0).contains(&rate) {
            return Err(format!(
                "a bit error rate of {rate} is not a probability: give flips \
                 per bit from 0 to 1, such as 1e-6"
            ));
        }
        // Never nought, which xorshift would stay at.
        STATE.store(seed.max(1), Ordering::Relaxed);
        ARMED
            .set(rate)
            .map_err(|_| "the bit errors are armed already".to_string())
    }

    /// The probability that a packet of `bytes` bytes has a flipped bit:
    /// `1 - (1 - p)^bits`, worked in a form that keeps its precision for
    /// the small rates a link has.
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

    /// The next random number, uniform from 0 up to but not 1.
    ///
    /// # Returns
    ///
    /// The number.
    fn uniform() -> f64 {
        let step = |mut x: u64| {
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            x
        };
        // A plain compare-and-swap, which every toolchain spells alike.
        let mut was = STATE.load(Ordering::Relaxed);
        loop {
            match STATE.compare_exchange_weak(
                was,
                step(was),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(now) => was = now,
            }
        }
        let drawn = step(was).wrapping_mul(0x2545_f491_4f6c_dd1d);
        // The top 53 bits, which a double holds exactly.
        (drawn >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Whether a packet is lost to a bit error.
    ///
    /// # Arguments
    ///
    /// * `bytes` - How long the packet is.
    ///
    /// # Returns
    ///
    /// `true` where it is to be dropped whole; `false` where nothing is
    /// armed.
    pub fn lost(bytes: usize) -> bool {
        match ARMED.get() {
            None => false,
            Some(&rate) => uniform() < chance(rate, bytes),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// The chance is `1 - (1 - p)^n`, at the rates a link has as well
        /// as the ends.
        #[test]
        fn the_chance_is_one_less_the_chance_of_no_flip() {
            assert_eq!(chance(0.0, 1000), 0.0);
            assert_eq!(chance(1.0, 1), 1.0);
            assert_eq!(chance(1.0, 0), 0.0);
            // Within what the plain form keeps of its precision, which is
            // less than this keeps: a part in 10^9 at these rates.
            let near = |a: f64, b: f64| (a - b).abs() <= 1e-9 * b;
            // 100 bytes at 1e-6: 1 - (1 - 1e-6)^800.
            let want = 1.0 - (1.0 - 1e-6f64).powi(800);
            assert!(near(chance(1e-6, 100), want), "{}", chance(1e-6, 100));
            // Small enough that 1 - p is 1 in a double, and still counted.
            assert!(chance(1e-18, 100) > 7.9e-16);
        }

        /// Drawn numbers are from 0 up to but not 1.
        #[test]
        fn a_draw_is_from_nought_up_to_one() {
            for _ in 0..10_000 {
                let u = uniform();
                assert!((0.0..1.0).contains(&u), "{u}");
            }
        }

        /// The flight setting and a rate that is no probability are
        /// refused.
        #[test]
        fn flight_and_a_rate_out_of_range_are_refused() {
            assert!(arm(1e-6, 1, Mode::None)
                .unwrap_err()
                .contains("flight setting"));
            assert!(arm(1.5, 1, Mode::All)
                .unwrap_err()
                .contains("not a probability"));
            assert!(arm(-1e-6, 1, Mode::All).is_err());
            assert!(arm(f64::NAN, 1, Mode::All).is_err());
        }
    }
}
