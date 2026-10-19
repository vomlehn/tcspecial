//! What arrives from a payload, as the ground receives it.
//!
//! The MOC commanded payloads and asked them what they had moved, and never
//! read a byte of what they sent. Every panel reading was therefore the
//! spacecraft's own account: the statistics it counted and the samples it
//! kept. Those are worth having -- they are what a handler saw -- but they
//! are not evidence that anything reached the ground, and the one reading an
//! operator wants from a payload that sends on its own is the data itself.
//!
//! So each payload with a ground link gets a listener here, bound where the
//! configuration says the ground's end of that link is --
//! `payload_telemetry.dest_base` counted on by the payload's id -- which reads
//! what the handler sends and keeps the latest for the panel.
//!
//! It used to say hello first: an empty datagram once a second to the
//! handler's own end, because a handler's OC endpoint learnt where to answer
//! from what arrived there -- `recv_from`, then `send_to` -- and with nobody
//! there it had no peer, so a payload's data went into a handler and no
//! further. Repeated once a second because START_DH and STOP_DH rebind that
//! socket and the learnt peer went with it, which made a ground station that
//! said hello once silently cut off.
//!
//! None of that is needed now that both ends of the link are written down:
//! tcspecial sends to the end this binds, from the first datagram, without
//! being told where it is. See `tcspecial::endpoint::bind_oc_pair`.

use std::sync::{Arc, Mutex};

use tcslibgs::{DHSample, Timestamp};

/// What one payload last sent to the ground, as the ground received it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arrival {
    /// When it arrived here. The ground's clock, and said so: the spacecraft
    /// puts no time in a payload datagram, and the panel's other times are
    /// the spacecraft's.
    pub at: Timestamp,
    /// The head of what arrived, kept the way a handler's samples are so that
    /// the panel shows the three lines alike.
    pub data: DHSample,
    /// How many have arrived from this payload since the MOC started.
    ///
    /// Counted here because this is what counts them: the panel's other two
    /// lines are what the ground received, and so is this. The count the
    /// panel showed before was the spacecraft's own -- the bytes its handler
    /// had read from the ground, which is the other direction and reads
    /// nought for a payload nobody sends to.
    pub packets: u64,
    /// Whether the node this came from sends values, so that a reading is
    /// shown as the temperature it is and not as four bytes of hexadecimal.
    ///
    /// Kept per arrival because it is a property of the node it came from,
    /// and this is where the node is known: the receiving loop has the name
    /// the datagram came from and the configuration to ask about it. The
    /// panel, by then, has a row and a string.
    ///
    /// From the configuration and never from the data. Every four bytes are a
    /// value, so a decision of four characters would be shown as a
    /// temperature by anything that guessed from the width -- see
    /// `tcslibgs::node::vehicle::compute::node_sends_a_value`.
    pub sends_values: bool,
}

impl Arrival {
    /// The two strings a panel shows: when, and what.
    pub fn panel_lines(&self) -> (String, String) {
        let (_, data) = self.data.panel_lines_of(self.sends_values);
        (self.at.time_of_day(), data)
    }
}

/// What every payload last sent to the ground, one slot per panel row.
pub type Arrivals = Arc<Mutex<Vec<Option<Arrival>>>>;



#[cfg(test)]
mod tests {
    use super::*;

    /// What came from a node that sends values is shown as the temperature.
    ///
    /// The reason the arrival carries that fact at all. The same four bytes
    /// are shown as bytes for a node that sends something else, because every
    /// four bytes are a value: the configuration is the only thing that can
    /// tell a reading from a four-character decision, and it is asked where
    /// the datagram is received.
    #[test]
    fn a_reading_is_shown_as_a_temperature() {
        let at = Timestamp::at(
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(3661),
        );
        let mut data = DHSample::new();
        data.record(
            &tcslibgs::node::vehicle::compute::as_value(293.15).expect("a temperature"),
        );

        let arrival = Arrival {
            at,
            data: data.clone(),
            packets: 1,
            sends_values: true,
        };
        assert_eq!(arrival.panel_lines(), (at.time_of_day(), "293.15 K".to_string()));

        // And from a node that does not send values, the bytes.
        let arrival = Arrival {
            at,
            data,
            packets: 1,
            sends_values: false,
        };
        assert_eq!(
            arrival.panel_lines(),
            (at.time_of_day(), "30 1D 79 11".to_string())
        );
    }

    /// A panel's two lines of an arrival: when it came, and what it was.
    ///
    /// What the ground received, which is the whole of what this module is
    /// for now. It used to open a socket per payload and listen on it -- the
    /// version 1 arrangement, where each payload had a ground endpoint of its
    /// own -- and a version 2 node's data reaches the ground through
    /// `tcslib::moc`, which is where the receiving is tested.
    #[test]
    fn an_arrival_says_when_it_came_and_what_it_was() {
        let at = Timestamp::at(
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(3661),
        );
        let mut data = DHSample::new();
        data.record(b"hello");
        let arrival = Arrival {
            at,
            data,
            packets: 7,
            sends_values: false,
        };

        // The time is the ground's clock, as every time in the window is, and
        // the data is the head of what arrived in hex -- the same way a
        // handler's samples are shown, so that the panel's lines read alike.
        let (when, what) = arrival.panel_lines();
        assert_eq!(when, at.time_of_day());
        assert_eq!(what, "68 65 6C 6C 6F", "{what}");
    }
}
