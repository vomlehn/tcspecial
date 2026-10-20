//! The node that carries both streams through and does nothing to what passes.
//!
//! The simplest vehicle node there is, and it is here for two reasons: it is
//! the one type with all four terminals, so it is what says what a vehicle
//! node *is* -- see `tcslibgs::graph::terminals` -- and it is what a
//! configuration reaches for when a stream has to pass through the vehicle on
//! its way somewhere, with nothing yet to do to it.
//!
//! What a `null` node is told is nothing. It has no attributes, and the
//! configuration check refuses any written on one: its name and its type are
//! the whole of it.
//!
//! Both streams through the one node, and nothing here keeps them apart. A
//! datum arriving from the payload side and one arriving from the ground are
//! both handed on, in the order they arrived, to wherever the node's data
//! sources go. That is the one thing a `null` node cannot be asked to get
//! right, and it is why a data sink is held to a single data source: a node
//! with one source into it and one sink out of it per stream carries each
//! stream to where it was going.

use std::collections::VecDeque;

/// What has arrived and not yet been taken.
///
/// A queue and not a slot, for the reason a computation's results are a queue:
/// the alternative when a second datum arrives before the first is taken is to
/// overwrite it, and a reading dropped where nothing says so is the one
/// outcome this project will not have.
#[derive(Debug, Default)]
pub struct Through {
    waiting: VecDeque<Vec<u8>>,
    /// Buffers taken out of `waiting`, kept to be filled again: so that a
    /// datum passed through is copied into one made earlier, not into one
    /// made for it.
    spare: Vec<Vec<u8>>,
}

/// How many data a null node has room for before it would grow: normally one
/// is waiting at most, the fabric taking it at once.
const WAITING_AT_ONCE: usize = 4;

impl Through {
    /// A pass-through with nothing in it, and buffers made for a few data of
    /// the largest size a datum has: made with the node, so that passing
    /// data through allocates nothing.
    pub fn new() -> Through {
        Through {
            waiting: VecDeque::with_capacity(WAITING_AT_ONCE),
            spare: (0..WAITING_AT_ONCE)
                .map(|_| Vec::with_capacity(tcslibgs::DATUM_BYTES))
                .collect(),
        }
    }

    /// Hold what arrived.
    ///
    /// Copied into a spare buffer. Only where more are waiting than there
    /// were buffers made is one made, and it is kept after, so that happens
    /// once if it happens at all.
    pub fn give(&mut self, data: &[u8]) {
        let mut buffer = self.spare.pop().unwrap_or_default();
        buffer.clear();
        buffer.extend_from_slice(data);
        self.waiting.push_back(buffer);
    }

    /// Whether anything is waiting to be handed on.
    pub fn waiting(&self) -> bool {
        !self.waiting.is_empty()
    }

    /// Hand on the oldest datum, or nothing where there is none.
    ///
    /// `what` names the node, for the one complaint this makes. A datum that
    /// will not fit is reported and kept rather than cut down: part of a value
    /// is not a smaller value, and the queue holds it so that a caller with a
    /// larger buffer can still have it.
    pub fn take(&mut self, what: &str, buffer: &mut [u8]) -> Result<usize, String> {
        let Some(datum) = self.waiting.pop_front() else {
            return Ok(0);
        };

        if datum.len() > buffer.len() {
            let wanted = datum.len();
            self.waiting.push_front(datum);
            return Err(format!(
                "{what}: what arrived is {wanted} bytes and the buffer holds {}: \
                 part of a value is not a smaller value",
                buffer.len()
            ));
        }

        buffer[..datum.len()].copy_from_slice(&datum);
        let len = datum.len();
        self.spare.push(datum);
        Ok(len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What arrives is handed on, in order and unchanged.
    #[test]
    fn what_arrives_is_handed_on_in_order() {
        let mut through = Through::new();
        assert!(!through.waiting());
        assert_eq!(through.take("through", &mut [0u8; 8]), Ok(0));

        through.give(b"290.0");
        through.give(b"go\n");
        assert!(through.waiting());

        let mut buffer = [0u8; 64];
        let got = through.take("through", &mut buffer).expect("the first");
        assert_eq!(&buffer[..got], b"290.0");
        let got = through.take("through", &mut buffer).expect("the second");
        assert_eq!(&buffer[..got], b"go\n");

        // And gone once taken, rather than handed out twice.
        assert!(!through.waiting());
        assert_eq!(through.take("through", &mut buffer), Ok(0));
    }

    /// A datum that will not fit is kept, and the complaint names the node.
    #[test]
    fn a_datum_too_large_for_the_buffer_is_kept() {
        let mut through = Through::new();
        through.give(b"2901234");

        let said = through
            .take("through", &mut [0u8; 4])
            .expect_err("it does not fit");
        assert!(
            said.contains("through") && said.contains("7 bytes"),
            "{said}"
        );
        assert!(through.waiting(), "the datum is still here");

        let mut buffer = [0u8; 64];
        let got = through.take("through", &mut buffer).expect("with room");
        assert_eq!(&buffer[..got], b"2901234");
    }
}
