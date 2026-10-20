//! Where one read of a line ends.
//!
//! A line delivers bytes with no frame boundaries of its own, so a node on one
//! states where a read ends -- `max_length`, `read_timeout`, `terminators`,
//! `length_prefix`; see `tcslibgs::endpoint_config::StreamParams` -- and this
//! holds what has arrived until one of them says the read is done. A read
//! then ends on whichever comes first:
//!
//! * a terminator, which is the last byte of the read;
//! * `max_length` bytes;
//! * `read_timeout` since the read's first byte, with what has arrived by then.
//!
//! A read with no timeout and no terminators ends only on `max_length`, which
//! is how a file asks for a fixed number of bytes at a time.
//!
//! A read with a length prefix ends when it holds the prefix and as many
//! bytes as the prefix gives, and is delivered prefix and all. Its timeout
//! does not deliver what has arrived: a frame that says how long it is and
//! is shorter than that is known to be broken, so it is let go, and the
//! next byte is read as the start of a new prefix -- which is how the reads
//! come back into step after a byte is lost. A frame longer than `max_length`
//! is let go too, byte by byte as it arrives, so that the one after it is
//! read from its own prefix. What is let go is counted, for the port to say;
//! see [`Framer::let_go`].
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

use tcslibgs::endpoint_config::{LengthPrefix, StreamParams};

/// What has arrived of the read that is under way, and the rule that ends it.
#[derive(Debug)]
pub struct Framer {
    rule: StreamParams,
    held: Vec<u8>,
    /// When the read's first byte arrived, which is what its timeout counts
    /// from. `None` while nothing is held and nothing is being let go.
    began: Option<Instant>,
    /// Bytes still to come of a prefixed frame too long to hold, which are
    /// taken from the line and dropped. Nought except while one is.
    skipping: usize,
    /// What has been let go since the port last asked.
    let_go: LetGo,
}

/// Prefixed frames let go rather than delivered, by why.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LetGo {
    /// Frames whose prefix gave a length that `max_length` cannot hold.
    pub too_long: u32,
    /// Frames the timeout ended before they were whole.
    pub cut_short: u32,
}

impl Framer {
    /// Nothing held yet, under this rule.
    pub fn new(rule: StreamParams) -> Framer {
        Framer {
            held: Vec::with_capacity(rule.max_length as usize),
            rule,
            began: None,
            skipping: 0,
            let_go: LetGo::default(),
        }
    }

    /// How many bytes to ask the line for: what would fill the read.
    ///
    /// For a prefixed read, no more than the prefix until it is whole, and
    /// then no more than the frame it gives -- so that the next frame's
    /// prefix stays in the line until this frame is done with.
    pub fn room(&self) -> usize {
        let most = self.rule.max_length as usize;
        let Some(prefix) = self.rule.length_prefix else {
            return most.saturating_sub(self.held.len());
        };
        if self.skipping > 0 {
            return self.skipping.min(most);
        }
        match frame_length(prefix, &self.held) {
            None => prefix.width() - self.held.len(),
            Some(whole) => whole.min(most).saturating_sub(self.held.len()),
        }
    }

    /// Hold these, which arrived at `now`.
    ///
    /// No more than [`Framer::room`] -- what has not been asked for is not
    /// handed over -- and the clock starts at the first of them.
    pub fn took(&mut self, bytes: &[u8], now: Instant) {
        let fits = bytes.len().min(self.room());
        if fits == 0 {
            return;
        }
        if self.skipping > 0 {
            self.skipping -= fits;
            if self.skipping == 0 {
                self.began = None;
            }
            return;
        }
        if self.held.is_empty() {
            self.began = Some(now);
        }
        self.held.extend_from_slice(&bytes[..fits]);
        // A prefix just made whole may give more than a read can hold. The
        // frame is let go as it arrives rather than held, and the clock that
        // started at its first byte goes on, so that a frame that never
        // finishes still ends at its timeout.
        if let Some(prefix) = self.rule.length_prefix {
            if let Some(whole) = frame_length(prefix, &self.held) {
                if whole > self.rule.max_length as usize {
                    self.skipping = whole - self.held.len();
                    self.held.clear();
                    self.let_go.too_long += 1;
                }
            }
        }
    }

    /// What has been let go since this was last asked, which starts the
    /// count again.
    pub fn let_go(&mut self) -> LetGo {
        std::mem::take(&mut self.let_go)
    }

    /// A read that has ended by `now`, if one has, copied into `out`.
    ///
    /// Its length, or `None` while the read is still under way. Whatever
    /// arrived after the end of it -- the bytes past a terminator -- is kept
    /// as the start of the next one, whose clock starts now: those bytes have
    /// been waiting since they arrived, but counting from then would end a
    /// read before the pass that took them had looked at it.
    pub fn frame(&mut self, now: Instant, out: &mut [u8]) -> Option<usize> {
        if self.rule.length_prefix.is_some() {
            self.give_up_on_a_late_frame(now);
        }
        let ends = self.ends(now)?;
        let length = ends.min(out.len());
        out[..length].copy_from_slice(&self.held[..length]);
        self.held.drain(..ends);
        self.began = if self.held.is_empty() {
            None
        } else {
            Some(now)
        };
        Some(length)
    }

    /// Let go of a prefixed frame its timeout has ended before it was whole,
    /// or stop letting go of one too long to hold, so that the next byte is
    /// read as a prefix.
    fn give_up_on_a_late_frame(&mut self, now: Instant) {
        let (Some(timeout), Some(began)) = (self.rule.timeout, self.began) else {
            return;
        };
        if !timed_out(began, now, timeout) {
            return;
        }
        if self.skipping > 0 {
            // Already counted, as too long.
            self.skipping = 0;
        } else if self.ends(now).is_none() {
            self.held.clear();
            self.let_go.cut_short += 1;
        } else {
            // Whole, just as its time ran out: delivered, not let go.
            return;
        }
        self.began = None;
    }

    /// Where the read that is under way ends, if it has ended by `now`.
    fn ends(&self, now: Instant) -> Option<usize> {
        if let Some(prefix) = self.rule.length_prefix {
            // Only ever whole: what the timeout ends is let go, not
            // delivered, and room() never asks past the frame's end.
            return frame_length(prefix, &self.held).filter(|&whole| self.held.len() == whole);
        }
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
            (Some(timeout), Some(began)) if timed_out(began, now, timeout) => Some(self.held.len()),
            _ => None,
        }
    }
}

/// How long the prefixed frame these bytes begin is, prefix and all, once
/// they hold the whole of its prefix.
fn frame_length(prefix: LengthPrefix, held: &[u8]) -> Option<usize> {
    (held.len() >= prefix.width()).then(|| prefix.width() + prefix.length(held))
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
            length_prefix: None,
        }
    }

    fn prefixed(max_length: u32, timeout: Option<Duration>, prefix: LengthPrefix) -> StreamParams {
        StreamParams {
            length_prefix: Some(prefix),
            ..rule(max_length, timeout, &[])
        }
    }

    /// Hold as much of `bytes` as is asked for, as a line would hand them
    /// over, and say what read, if any, has ended by then. What is not asked
    /// for is dropped, as though it were left in the line.
    fn after(framer: &mut Framer, bytes: &[u8], now: Instant) -> Option<Vec<u8>> {
        feed(framer, bytes, now);
        let mut out = [0u8; 64];
        framer.frame(now, &mut out).map(|n| out[..n].to_vec())
    }

    /// Hand over `bytes` for as long as the framer asks for more: a prefix,
    /// and then the frame it gives.
    fn feed(framer: &mut Framer, bytes: &[u8], now: Instant) {
        let mut from = 0;
        while from < bytes.len() {
            let ask = framer.room().min(bytes.len() - from);
            if ask == 0 {
                break;
            }
            framer.took(&bytes[from..from + ask], now);
            from += ask;
        }
    }

    /// A terminator ends a read and is its last byte; what follows it is the
    /// start of the next one.
    #[test]
    fn a_terminator_ends_a_read_and_is_part_of_it() {
        let now = Instant::now();
        let mut framer = Framer::new(rule(64, None, b"\n"));
        assert_eq!(after(&mut framer, b"$GPGGA", now), None, "no end yet");
        assert_eq!(
            after(&mut framer, b",1\n$GP", now),
            Some(b"$GPGGA,1\n".to_vec())
        );
        assert_eq!(
            after(&mut framer, b"RMC\n", now),
            Some(b"$GPRMC\n".to_vec())
        );
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
        assert_eq!(
            after(&mut framer, b"", much_later),
            None,
            "no timeout ends it"
        );
        assert_eq!(
            after(&mut framer, b"cdef", much_later),
            Some(b"abcd".to_vec())
        );
        // What did not fit was never taken: the line still has it.
        assert_eq!(framer.room(), 4);
    }

    /// A read ends at its timeout with what has arrived, counted from its
    /// first byte -- and not before.
    #[test]
    fn a_timeout_ends_a_read_with_what_has_arrived() {
        let start = Instant::now();
        let mut framer = Framer::new(rule(64, Some(Duration::from_millis(250)), &[]));
        assert_eq!(
            after(&mut framer, b"", start),
            None,
            "nothing held, nothing ends"
        );
        assert_eq!(after(&mut framer, b"ab", start), None);
        let early = start + Duration::from_millis(249);
        assert_eq!(
            after(&mut framer, b"c", early),
            None,
            "the clock is the first byte's"
        );
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

    /// A prefixed read is the prefix and as many bytes as it gives, and is
    /// delivered prefix and all; the next frame's prefix is not asked for
    /// until this one is done.
    #[test]
    fn a_length_prefix_ends_a_read_and_is_part_of_it() {
        let now = Instant::now();
        let mut framer = Framer::new(prefixed(64, None, LengthPrefix::U8));
        assert_eq!(framer.room(), 1, "the prefix first");
        assert_eq!(after(&mut framer, b"\x03", now), None);
        assert_eq!(framer.room(), 3, "then the frame it gives");
        assert_eq!(after(&mut framer, b"ab", now), None);
        // A terminator's byte means nothing here.
        assert_eq!(
            after(&mut framer, b"\nXYZ", now),
            Some(b"\x03ab\n".to_vec())
        );
        assert_eq!(framer.room(), 1, "nothing of the next was taken");
    }

    /// The two-byte prefixes are read in the order each names, and a length
    /// of nought is a frame of the prefix alone.
    #[test]
    fn two_byte_prefixes_are_read_in_their_own_order() {
        let now = Instant::now();
        let mut big = Framer::new(prefixed(300, None, LengthPrefix::U16Be));
        let mut frame = vec![0x01, 0x00];
        frame.extend(std::iter::repeat_n(7u8, 256));
        big.took(&frame, now);
        assert_eq!(big.room(), 256, "only the prefix was taken");
        feed(&mut big, &frame[2..], now);
        let mut out = [0u8; 300];
        assert_eq!(big.frame(now, &mut out), Some(258));

        let mut little = Framer::new(prefixed(8, None, LengthPrefix::U16Le));
        assert_eq!(after(&mut little, &[0x02, 0x00], now), None);
        assert_eq!(
            after(&mut little, b"hi", now),
            Some(vec![0x02, 0x00, b'h', b'i'])
        );
        assert_eq!(
            after(&mut little, &[0x00, 0x00], now),
            Some(vec![0x00, 0x00])
        );
    }

    /// A prefixed frame its timeout ends is let go, not delivered, and the
    /// next byte is read as a prefix: which is how the reads come back into
    /// step after a byte is lost.
    #[test]
    fn a_timeout_lets_go_of_a_prefixed_frame_cut_short() {
        let start = Instant::now();
        let mut framer = Framer::new(prefixed(
            64,
            Some(Duration::from_millis(100)),
            LengthPrefix::U8,
        ));
        assert_eq!(after(&mut framer, b"\x04ab", start), None);
        let late = start + Duration::from_millis(100);
        assert_eq!(after(&mut framer, b"", late), None, "let go, not delivered");
        assert_eq!(
            framer.let_go(),
            LetGo {
                too_long: 0,
                cut_short: 1
            }
        );
        assert_eq!(framer.let_go(), LetGo::default(), "counted once");
        assert_eq!(after(&mut framer, b"\x01z", late), Some(b"\x01z".to_vec()));
    }

    /// A frame whose prefix gives more than max_length is let go as it
    /// arrives, and the frame after it is read from its own prefix.
    #[test]
    fn a_prefixed_frame_too_long_to_hold_is_let_go() {
        let now = Instant::now();
        let mut framer = Framer::new(prefixed(4, None, LengthPrefix::U8));
        assert_eq!(after(&mut framer, b"\x06", now), None);
        assert_eq!(framer.let_go().too_long, 1);
        assert_eq!(framer.room(), 4, "no more than a read at a time");
        assert_eq!(after(&mut framer, b"abcd", now), None);
        assert_eq!(framer.room(), 2, "what is left of it");
        // The rest of it, and the whole of the next, arriving together.
        assert_eq!(
            after(&mut framer, b"ef\x02gh", now),
            Some(b"\x02gh".to_vec())
        );
        assert_eq!(framer.let_go(), LetGo::default(), "the next was not let go");
    }

    /// A frame too long to hold that stops arriving stops being let go at
    /// its timeout, and is not counted twice.
    #[test]
    fn a_timeout_ends_the_letting_go_of_a_frame_that_stopped() {
        let start = Instant::now();
        let mut framer = Framer::new(prefixed(
            4,
            Some(Duration::from_millis(50)),
            LengthPrefix::U8,
        ));
        assert_eq!(after(&mut framer, b"\xFFab", start), None);
        let late = start + Duration::from_millis(50);
        assert_eq!(after(&mut framer, b"", late), None);
        assert_eq!(
            framer.let_go(),
            LetGo {
                too_long: 1,
                cut_short: 0
            }
        );
        assert_eq!(framer.room(), 1, "a prefix is looked for again");
        assert_eq!(after(&mut framer, b"\x00", late), Some(vec![0x00]));
    }

    /// A frame that is whole when its time runs out is delivered: a timeout
    /// ends only what is still under way.
    #[test]
    fn a_whole_prefixed_frame_is_delivered_however_late() {
        let start = Instant::now();
        let mut framer = Framer::new(prefixed(
            8,
            Some(Duration::from_millis(10)),
            LengthPrefix::U8,
        ));
        feed(&mut framer, b"\x01q", start);
        let mut out = [0u8; 8];
        let late = start + Duration::from_secs(1);
        assert_eq!(framer.frame(late, &mut out), Some(2));
        assert_eq!(framer.let_go(), LetGo::default());
    }
}
