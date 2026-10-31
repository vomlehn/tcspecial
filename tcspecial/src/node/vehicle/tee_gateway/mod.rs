//! The node that takes what the ground sends and sends it on both ways: a
//! `tee-gateway`.
//!
//! A link carries one stream and may not carry it into the other -- a
//! `gateway-out` goes to a `gateway-in` and nowhere else -- so where what the
//! ground sends has to reach a `payload-in` as well, a tee is what does it. A
//! `tee-gateway` takes each datum the ground sends at its `gateway-in` and
//! sends it on by its `gateway-out`, as it was going, and again by its
//! `payload-out`. Its mirror is the `tee-payload`, in `super::tee_payload`,
//! which holds what arrives in the same [`Through`].
//!
//! What a tee is told is nothing. It has no attributes, and the
//! configuration check refuses any written on one: its name and its type are
//! the whole of it.
//!
//! One stream in, so nothing here has to keep two apart: each datum is handed
//! back once, on no stream in particular, and the fabric delivers it down
//! every link leaving the node -- one copy for each, which is the
//! duplication. See `crate::runtime::fabric::Taken`.

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

/// How many data a tee has room for before it would grow: normally one
/// is waiting at most, the fabric taking it at once.
const WAITING_AT_ONCE: usize = 4;

impl Through {
    /// Makes a pass-through with nothing in it, and buffers made for a few
    /// data of the largest size a datum has: made with the node, so that
    /// passing data through allocates nothing.
    ///
    /// # Returns
    ///
    /// The empty pass-through.
    pub fn new() -> Through {
        Through {
            waiting: VecDeque::with_capacity(WAITING_AT_ONCE),
            spare: (0..WAITING_AT_ONCE)
                .map(|_| Vec::with_capacity(tcslibgs::DATUM_BYTES))
                .collect(),
        }
    }

    /// Holds what arrived.
    ///
    /// Copied into a spare buffer. Only where more are waiting than there
    /// were buffers made is one made, and it is kept after, so that happens
    /// once if it happens at all.
    ///
    /// # Arguments
    ///
    /// * `data` - What arrived.
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

    /// Hands on the oldest datum, or nothing where there is none.
    ///
    /// A datum that will not fit is reported and kept rather than cut down:
    /// part of a value is not a smaller value, and the queue holds it so
    /// that a caller with a larger buffer can still have it.
    ///
    /// # Arguments
    ///
    /// * `what`   - The node's name, for the one complaint this makes.
    /// * `buffer` - Where the datum is copied.
    ///
    /// # Returns
    ///
    /// The datum's length in bytes, or 0 if none was waiting.
    ///
    /// # Errors
    ///
    /// A message naming the node if the datum is longer than `buffer`; the
    /// datum is kept.
    pub fn take(
        &mut self,
        what: &str,
        buffer: &mut [u8],
    ) -> Result<usize, String> {
        let Some(datum) = self.waiting.pop_front() else {
            return Ok(0);
        };

        if datum.len() > buffer.len() {
            let wanted = datum.len();
            self.waiting.push_front(datum);
            return Err(format!(
                "{what}: what arrived is {wanted} bytes and the buffer holds \
                 {}: part of a value is not a smaller value",
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
