//! Where one read of a line ends.
//!
//! A line delivers bytes with no frame boundaries of its own, so a node on one
//! states where a read ends -- `max_length`, `read_timeout`, `terminators`;
//! see `tcslibgs::endpoint_config::StreamParams` -- and this holds what has
//! arrived until one of them says the read is done. A read then ends on
//! whichever comes first:
//!
//! * a terminator, which is the last byte of the read;
//! * `max_length` bytes;
//! * `read_timeout` since the read's first byte, with what has arrived by then.
//!
//! A read with no timeout and no terminators ends only on `max_length`, which
//! is how a file asks for a fixed number of bytes at a time.
//!
//! A pass never waits on a line, so nothing here blocks. A timeout is noticed
//! by the first pass after it has passed, and a read that ends by it is that
//! much late; a node that wants its reads on time says so with the interval
//! its group is read at.
//!
//! What is held never exceeds `max_length`, and no more is asked of the line
//! than would fill it: what has not been asked for stays in the line's own
//! buffer, so a line that sends faster than it is read is held back by its
//! driver rather than by a queue here that grows.

use std::time::{Duration, Instant};

use tcslibgs::endpoint_config::StreamParams;

/// What has arrived of the read that is under way, and the rule that ends it.
#[derive(Debug)]
pub struct Framer {
    rule: StreamParams,
    held: Vec<u8>,
    /// When the read's first byte arrived, which is what its timeout counts
    /// from. `None` while nothing is held.
    began: Option<Instant>,
}

impl Framer {
    /// Nothing held yet, under this rule.
    pub fn new(rule: StreamParams) -> Framer {
        Framer {
            held: Vec::with_capacity(rule.max_length as usize),
            rule,
            began: None,
        }
    }

    /// How many bytes to ask the line for: what would fill the read.
    pub fn room(&self) -> usize {
        (self.rule.max_length as usize).saturating_sub(self.held.len())
    }

    /// Hold these, which arrived at `now`.
    ///
    /// No more than [`Framer::room`] -- what has not been asked for is not
    /// handed over -- and the clock starts at the first of them.
    pub fn took(&mut self, bytes: &[u8], now: Instant) {
        let fits = bytes.len().min(self.room());
        if fits > 0 && self.held.is_empty() {
            self.began = Some(now);
        }
        self.held.extend_from_slice(&bytes[..fits]);
    }

    /// A read that has ended by `now`, if one has, copied into `out`.
    ///
    /// Its length, or `None` while the read is still under way. Whatever
    /// arrived after the end of it -- the bytes past a terminator -- is kept
    /// as the start of the next one, whose clock starts now: those bytes have
    /// been waiting since they arrived, but counting from then would end a
    /// read before the pass that took them had looked at it.
    pub fn frame(&mut self, now: Instant, out: &mut [u8]) -> Option<usize> {
        let ends = self.ends(now)?;
        let length = ends.min(out.len());
        out[..length].copy_from_slice(&self.held[..length]);
        self.held.drain(..ends);
        self.began = if self.held.is_empty() { None } else { Some(now) };
        Some(length)
    }

    /// Where the read that is under way ends, if it has ended by `now`.
    fn ends(&self, now: Instant) -> Option<usize> {
        if let Some(at) = self
            .held
            .iter()
            .position(|byte| self.rule.terminators.contains(byte))
        {
            return Some(at + 1);
        }
        if self.held.len() >= self.rule.max_length as usize {
            return Some(self.held.len());
        }
        match (self.rule.timeout, self.began) {
            (Some(timeout), Some(began)) if timed_out(began, now, timeout) => {
                Some(self.held.len())
            }
            _ => None,
        }
    }
}

fn timed_out(began: Instant, now: Instant, timeout: Duration) -> bool {
    now.saturating_duration_since(began) >= timeout
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(max_length: u32, timeout: Option<Duration>, terminators: &[u8]) -> StreamParams {
        StreamParams {
            max_length,
            timeout,
            terminators: terminators.to_vec(),
        }
    }

    /// Hold `bytes` and say what read, if any, has ended by then.
    fn after(framer: &mut Framer, bytes: &[u8], now: Instant) -> Option<Vec<u8>> {
        framer.took(bytes, now);
        let mut out = [0u8; 64];
        framer.frame(now, &mut out).map(|n| out[..n].to_vec())
    }

    /// A terminator ends a read and is its last byte; what follows it is the
    /// start of the next one.
    #[test]
    fn a_terminator_ends_a_read_and_is_part_of_it() {
        let now = Instant::now();
        let mut framer = Framer::new(rule(64, None, b"\n"));
        assert_eq!(after(&mut framer, b"$GPGGA", now), None, "no end yet");
        assert_eq!(after(&mut framer, b",1\n$GP", now), Some(b"$GPGGA,1\n".to_vec()));
        assert_eq!(after(&mut framer, b"RMC\n", now), Some(b"$GPRMC\n".to_vec()));
    }

    /// Two reads that arrived together come out one at a time, the second
    /// without anything more being read.
    #[test]
    fn two_reads_that_arrive_together_come_out_in_turn() {
        let now = Instant::now();
        let mut framer = Framer::new(rule(64, None, b"\r\n"));
        assert_eq!(after(&mut framer, b"ab\rcd\n", now), Some(b"ab\r".to_vec()));
        assert_eq!(after(&mut framer, b"", now), Some(b"cd\n".to_vec()));
        assert_eq!(after(&mut framer, b"", now), None);
    }

    /// With neither a timeout nor terminators, a read is max_length bytes,
    /// however long they take.
    #[test]
    fn a_fixed_length_read_waits_for_all_of_itself() {
        let start = Instant::now();
        let mut framer = Framer::new(rule(4, None, &[]));
        assert_eq!(after(&mut framer, b"ab", start), None);
        let much_later = start + Duration::from_secs(3_600);
        assert_eq!(after(&mut framer, b"", much_later), None, "no timeout ends it");
        assert_eq!(after(&mut framer, b"cdef", much_later), Some(b"abcd".to_vec()));
        // What did not fit was never taken: the line still has it.
        assert_eq!(framer.room(), 4);
    }

    /// A read ends at its timeout with what has arrived, counted from its
    /// first byte -- and not before.
    #[test]
    fn a_timeout_ends_a_read_with_what_has_arrived() {
        let start = Instant::now();
        let mut framer = Framer::new(rule(64, Some(Duration::from_millis(250)), &[]));
        assert_eq!(after(&mut framer, b"", start), None, "nothing held, nothing ends");
        assert_eq!(after(&mut framer, b"ab", start), None);
        let early = start + Duration::from_millis(249);
        assert_eq!(after(&mut framer, b"c", early), None, "the clock is the first byte's");
        let late = start + Duration::from_millis(250);
        assert_eq!(after(&mut framer, b"", late), Some(b"abc".to_vec()));
        // And the next read's clock has not started.
        let later = late + Duration::from_secs(1);
        assert_eq!(after(&mut framer, b"", later), None);
    }

    /// max_length ends a read whatever else the rule says, and nothing more
    /// than it is ever held.
    #[test]
    fn max_length_bounds_every_read() {
        let now = Instant::now();
        let mut framer = Framer::new(rule(3, Some(Duration::from_secs(1)), b"\n"));
        assert_eq!(framer.room(), 3);
        assert_eq!(after(&mut framer, b"abcdef", now), Some(b"abc".to_vec()));
        assert_eq!(framer.room(), 3, "the rest was not taken");
    }

    /// A read longer than the buffer it is copied into is cut to fit, and
    /// all of it is let go: a datum is delivered whole or not at all, and the
    /// configuration check keeps max_length within one.
    #[test]
    fn a_read_is_cut_to_the_buffer_it_is_given() {
        let now = Instant::now();
        let mut framer = Framer::new(rule(8, None, &[]));
        framer.took(b"abcdefgh", now);
        let mut small = [0u8; 4];
        assert_eq!(framer.frame(now, &mut small), Some(4));
        assert_eq!(&small, b"abcd");
        assert_eq!(framer.room(), 8, "the whole read was let go");
    }
}
