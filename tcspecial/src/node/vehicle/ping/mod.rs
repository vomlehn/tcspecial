//! The node that answers the ground's pings.
//!
//! Each ping the MOC sends is handed back with the time the vehicle had it
//! written on the end, so that the ground can see how long one takes to
//! arrive and to come back, and how far the two clocks are apart. What a ping
//! and its answer say is `tcslibgs::node::vehicle::ping`, which the ground
//! reads the answers with; this is only the node.
//!
//! What a `ping` node is told is nothing. It has no attributes, and the
//! configuration check refuses any written on one: what it does is fixed by
//! the format.
//!
//! The time is taken when the ping is handed over rather than when the answer
//! is taken from the node, because it is the vehicle's account of when the
//! ping arrived: the time an answer waited for the pass is the vehicle's own
//! business and not part of the link's.

use std::collections::VecDeque;
use std::time::Duration;

use tcslibgs::node::vehicle::ping::{self, Reply};

/// The answers made and not yet taken.
///
/// A queue and not a slot, for the reason a `null` node's is: a second ping
/// arriving before the first answer is taken would otherwise overwrite it, and
/// a ping that goes unanswered where nothing says so is a link that looks
/// lossy when it is not. Each answer is a [`Reply`], whose buffer holds the
/// largest ping there is, the space and the largest time, so nothing is
/// allocated per ping beyond its place in the queue.
#[derive(Debug, Default)]
pub struct Answers {
    waiting: VecDeque<Reply>,
}

impl Answers {
    /// A ping node that has answered nothing, with room for a few answers
    /// made with it: an answer is a fixed array, so answering a ping
    /// allocates nothing.
    pub fn new() -> Answers {
        Answers {
            waiting: VecDeque::with_capacity(4),
        }
    }

    /// Answer what arrived, with the time now.
    ///
    /// `what` names the node, for the complaint about what is not a ping.
    pub fn give(&mut self, what: &str, data: &[u8]) -> Result<(), String> {
        let now = ping::now().map_err(|why| format!("{what}: {why}"))?;
        self.give_at(what, data, now)
    }

    /// Answer what arrived, with this time: [`Answers::give`] with the clock
    /// stated, so that what an answer says can be tested.
    ///
    /// What is not a ping is refused and nothing is answered: an answer to it
    /// would be a message the ground could not read as one, which tells it
    /// less than the refusal, which is logged aboard.
    pub fn give_at(&mut self, what: &str, data: &[u8], now: Duration) -> Result<(), String> {
        let reply = ping::reply(data, now).map_err(|why| format!("{what}: {why}"))?;
        self.waiting.push_back(reply);
        Ok(())
    }

    /// Whether an answer is waiting to be sent.
    pub fn waiting(&self) -> bool {
        !self.waiting.is_empty()
    }

    /// Hand on the oldest answer, or nothing where there is none.
    ///
    /// An answer that will not fit is reported and kept, as a `null` node
    /// keeps a datum: part of an answer is not a shorter answer.
    pub fn take(&mut self, what: &str, buffer: &mut [u8]) -> Result<usize, String> {
        let Some(reply) = self.waiting.front() else {
            return Ok(0);
        };
        let bytes = reply.as_bytes();
        if bytes.len() > buffer.len() {
            return Err(format!(
                "{what}: the answer is {} bytes and the buffer holds {}: part of an \
                 answer is not a shorter answer",
                bytes.len(),
                buffer.len()
            ));
        }
        let len = bytes.len();
        buffer[..len].copy_from_slice(bytes);
        self.waiting.pop_front();
        Ok(len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: Duration = Duration::new(1_760_900_000, 250_000_000);

    /// Each ping is answered with itself, a space and the time, in the order
    /// the pings arrived.
    #[test]
    fn each_ping_is_answered_in_order() {
        let mut answers = Answers::new();
        assert!(!answers.waiting());
        assert_eq!(answers.take("p", &mut [0u8; 8]), Ok(0));

        answers
            .give_at("p", b"ping #1 1760900000.000000001", AT)
            .expect("the first");
        answers
            .give_at(
                "p",
                b"ping #2 1760900001.000000001",
                AT + Duration::from_secs(1),
            )
            .expect("the second");
        assert!(answers.waiting());

        let mut buffer = [0u8; ping::REPLY_BYTES];
        let got = answers.take("p", &mut buffer).expect("the first");
        assert_eq!(
            &buffer[..got],
            b"ping #1 1760900000.000000001 1760900000.250000000"
        );
        let got = answers.take("p", &mut buffer).expect("the second");
        assert_eq!(
            &buffer[..got],
            b"ping #2 1760900001.000000001 1760900001.250000000"
        );
        assert!(!answers.waiting());
    }

    /// What is not a ping is refused, the complaint names the node, and
    /// nothing is answered.
    #[test]
    fn what_is_not_a_ping_is_refused_and_not_answered() {
        let mut answers = Answers::new();
        let why = answers
            .give_at("pinger", b"hello", AT)
            .expect_err("not a ping");
        assert!(why.starts_with("pinger: "), "{why}");
        assert!(!answers.waiting());
    }

    /// An answer too large for the buffer is kept, not cut short.
    #[test]
    fn an_answer_too_large_for_the_buffer_is_kept() {
        let mut answers = Answers::new();
        answers
            .give_at("p", b"ping #1 1760900000.000000001", AT)
            .expect("a ping");

        let why = answers.take("p", &mut [0u8; 8]).expect_err("too small");
        assert!(why.contains("p: the answer is 49 bytes"), "{why}");
        assert!(answers.waiting(), "the answer is still here");

        let mut buffer = [0u8; ping::REPLY_BYTES];
        assert_eq!(answers.take("p", &mut buffer), Ok(49));
    }

    /// The time is the clock's, now.
    #[test]
    fn the_time_is_the_clock_now() {
        let before = ping::now().expect("a clock");
        let mut answers = Answers::new();
        answers
            .give("p", b"ping #1 1760900000.000000001")
            .expect("a ping");
        let after = ping::now().expect("a clock");

        let mut buffer = [0u8; ping::REPLY_BYTES];
        let got = answers.take("p", &mut buffer).expect("an answer");
        let pong = ping::read_reply(&buffer[..got]).expect("it reads");
        assert!(
            before <= pong.answered && pong.answered <= after,
            "{pong:?}"
        );
    }
}
