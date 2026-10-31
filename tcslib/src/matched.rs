//! Whether a ground station and the vehicle are using the same
//! configuration, and what the station has received meanwhile.
//!
//! A station is *matched for listening* while the last beacon it has heard,
//! since it began using its configuration, has its restart flag clear,
//! carries that configuration's checksum, and is recent: the beacon watch
//! says the vehicle is heard from ([`Stage::Heard`]). A clear flag says the
//! vehicle has taken a CONNECT carrying the checksum in that beacon, from
//! this station or another, so a station that only listens is matched as
//! surely as one that commands. It is *matched for commanding* while it is
//! matched for listening and a CONNECT of its own has been answered with its
//! checksum since the match last ended.
//!
//! A beacon with the flag set, one carrying another checksum, or the beacon
//! going overdue ends both; one whose number goes back -- a start of
//! tcspecial -- ends the commanding half whatever else it says, since the
//! CONNECT it rested on was answered by a start that has gone. A station
//! that changes the configuration it uses makes a new [`Matched`] for it,
//! and has heard no beacon for it yet.
//!
//! While a station is not matched it reads none of the vehicle's data: it
//! drops it and counts it, and on becoming matched it first drops and counts
//! whatever was already queued on its data sockets, since that was all made
//! before the match. Its client sends nothing but CONNECT, RESTART_ARM and
//! RESTART until it is matched for commanding. See "On the ground" in
//! `docs/design.rst`.
//!
//! One place for every part of a station -- the beacon watch that feeds it,
//! the client and the data receivers that ask it -- which hold clones of
//! one [`Matched`]. Atomics where it is asked: a receiver asks it for every
//! datum, in a loop that must neither allocate nor wait for the beacon
//! thread. The one lock is round the list of data sockets to empty at a
//! match, which the beacon thread takes only as a match begins and a
//! receiver only as it is opened and closed.

use std::os::fd::RawFd;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tcslibgs::ConfigDigest;

use crate::beacon::{Pass, Stage};

/// What a station has received from the vehicle, data and answers alike,
/// each in exactly one of three counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    /// Read, and handed on: a datum to whoever receives it, an answer to the
    /// command it answers.
    pub processed: u64,
    /// Not readable with this station's configuration's definitions: a
    /// datum longer than any it defines, an answer that is not one of the
    /// link's packets.
    pub malformed: u64,
    /// Dropped because the two ends may not be using the same
    /// configuration: data while the station was not matched or queued as
    /// the match began, and answers to anything but the command waited for.
    pub mismatched: u64,
}

impl Counts {
    /// Every packet received from the vehicle: the three counts together.
    pub fn received(&self) -> u64 {
        self.processed + self.malformed + self.mismatched
    }
}

/// What the shared state holds.
#[derive(Debug)]
struct Inner {
    /// The checksum of the configuration this station uses, or `None` where
    /// it has none, which nothing then matches.
    ours: Option<ConfigDigest>,
    /// Whether the station is matched for listening.
    listening: AtomicBool,
    /// The data sockets of every receiver gated by this, which are emptied
    /// as a match begins: see [`Matched::queued_on`].
    queues: Mutex<Vec<RawFd>>,
    /// How many times a match has ended, or a start of tcspecial been heard:
    /// the commanding half rests on a CONNECT answered since the last of
    /// these.
    ended: AtomicU64,
    /// One more than the value of `ended` when a CONNECT of this station's
    /// was last answered with its checksum, or nought for none. Commanding
    /// is matched while it is one more than `ended` still.
    connected: AtomicU64,
    processed: AtomicU64,
    malformed: AtomicU64,
    mismatched: AtomicU64,
}

/// Whether a station is matched with the vehicle, shared by every part of
/// it that asks.
#[derive(Debug, Clone)]
pub struct Matched(Arc<Inner>);

impl Matched {
    /// A station beginning to use the configuration whose checksum is
    /// `ours`: matched for nothing yet, having heard no beacon for it.
    ///
    /// # Arguments
    ///
    /// * `ours` - The checksum of the configuration the station uses.
    ///
    /// # Returns
    ///
    /// The shared state, of which each part of the station takes a clone.
    pub fn for_configuration(ours: ConfigDigest) -> Matched {
        Matched::of(Some(ours))
    }

    /// A station whose configuration could not be digested, so that there
    /// is no checksum to hold a beacon or an answer to: it is never
    /// matched, and reads and commands nothing but the three commands.
    ///
    /// # Returns
    ///
    /// The shared state.
    pub fn with_no_checksum() -> Matched {
        Matched::of(None)
    }

    /// The shared state for a station whose checksum is `ours`.
    fn of(ours: Option<ConfigDigest>) -> Matched {
        Matched(Arc::new(Inner {
            ours,
            listening: AtomicBool::new(false),
            queues: Mutex::new(Vec::new()),
            ended: AtomicU64::new(0),
            connected: AtomicU64::new(0),
            processed: AtomicU64::new(0),
            malformed: AtomicU64::new(0),
            mismatched: AtomicU64::new(0),
        }))
    }

    /// The checksum of the configuration this station uses, if it has one.
    pub fn ours(&self) -> Option<ConfigDigest> {
        self.0.ours
    }

    /// Whether the station is matched for listening: it may read the
    /// vehicle's data.
    pub fn for_listening(&self) -> bool {
        self.0.listening.load(Ordering::SeqCst)
    }

    /// Whether the station is matched for commanding: it may send every
    /// command, not only CONNECT, RESTART_ARM and RESTART.
    pub fn for_commanding(&self) -> bool {
        self.for_listening()
            && self.0.connected.load(Ordering::SeqCst)
                == self.0.ended.load(Ordering::SeqCst) + 1
    }

    /// Takes in one pass of the beacon watch: what its last beacon said and
    /// how recent it is. Every pass, not only the ones a beacon arrived on,
    /// because the match ends by a beacon going overdue, which is what the
    /// passes that time out are.
    ///
    /// # Arguments
    ///
    /// * `pass` - The pass, as [`crate::beacon::Beacons::pass`] gave it.
    pub fn heard(&self, pass: &Pass) {
        self.beacon(
            pass.restart_flag,
            pass.digest,
            pass.showing.stage,
            pass.restarted,
        );
    }

    /// [`Matched::heard`], from what it reads of a pass: for a station that
    /// keeps its own beacon watch, and for the tests.
    ///
    /// # Arguments
    ///
    /// * `restart_flag` - Whether the last beacon's restart flag was set.
    /// * `digest`       - The checksum it carried, or `None` where none has
    ///                    arrived.
    /// * `stage`        - How recent it is.
    /// * `restarted`    - Whether this pass heard a beacon whose number went
    ///                    back: a start of tcspecial.
    pub fn beacon(
        &self,
        restart_flag: bool,
        digest: Option<ConfigDigest>,
        stage: Stage,
        restarted: bool,
    ) {
        let now = !restart_flag
            && digest.is_some()
            && digest == self.0.ours
            && stage == Stage::Heard;
        let was = self.0.listening.load(Ordering::SeqCst);
        // A match beginning: everything already queued on the data sockets
        // was made before it, so it is dropped and counted before the
        // receivers are let read. Emptied here, as the match begins, and
        // not by each receiver at its next read, which would drop what
        // arrived after the match too. A datum a receiver reads meanwhile
        // is dropped by it, since it is not yet matched.
        if now && !was {
            self.drop_queued();
        }
        self.0.listening.store(now, Ordering::SeqCst);
        // The commanding half ends with the listening half, and at every
        // start of tcspecial: the CONNECT it rested on was answered by a
        // start that has gone, whatever the new one's beacons say.
        if (was && !now) || restarted {
            self.0.ended.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Notes a data socket to empty each time a match begins, for a
    /// receiver gated by this. Called as the receiver is opened, which may
    /// allocate; a receiver says [`Matched::no_longer_queued_on`] before its
    /// socket closes, so that a number the system gives another descriptor
    /// later is never read from here.
    ///
    /// # Arguments
    ///
    /// * `fd` - The socket's descriptor.
    pub(crate) fn queued_on(&self, fd: RawFd) {
        if let Ok(mut queues) = self.0.queues.lock() {
            queues.push(fd);
        }
    }

    /// Forgets a data socket [`Matched::queued_on`] noted, before it closes.
    ///
    /// # Arguments
    ///
    /// * `fd` - The socket's descriptor.
    pub(crate) fn no_longer_queued_on(&self, fd: RawFd) {
        if let Ok(mut queues) = self.0.queues.lock() {
            queues.retain(|queued| *queued != fd);
        }
    }

    /// Drops every datagram queued on the noted data sockets, unread and
    /// without waiting, and counts each as dropped for a mismatch.
    fn drop_queued(&self) {
        let Ok(queues) = self.0.queues.lock() else {
            return;
        };
        // One byte of room: with MSG_TRUNC the rest of a datagram is
        // discarded, which is all that is wanted of it.
        let mut room = [0u8; 1];
        let mut dropped = 0;
        for &fd in queues.iter() {
            loop {
                // SAFETY: recv needs an open socket and a buffer writable for
                // the length it is given. Each descriptor is a receiver's
                // open socket, which it takes off this list, under this lock,
                // before it closes; the buffer is `room`, and its length.
                let got = unsafe {
                    libc::recv(
                        fd,
                        room.as_mut_ptr().cast(),
                        room.len(),
                        libc::MSG_DONTWAIT | libc::MSG_TRUNC,
                    )
                };
                if got >= 0 {
                    dropped += 1;
                    continue;
                }
                if std::io::Error::last_os_error().kind()
                    == std::io::ErrorKind::Interrupted
                {
                    continue;
                }
                // Empty, or a socket with nothing to give: its receiver's
                // next read says which.
                break;
            }
        }
        self.mismatched(dropped);
    }

    /// Notes where the station's CONNECTs stand, before one is sent, for
    /// [`Matched::connect_answered`] to be handed when it is answered.
    ///
    /// # Returns
    ///
    /// A mark that a match ending in between makes stale.
    pub fn before_connect(&self) -> u64 {
        self.0.ended.load(Ordering::SeqCst)
    }

    /// A CONNECT of this station's has been answered with `theirs`, the
    /// checksum of the configuration the vehicle is running. Where it is the
    /// station's own, and no match has ended since the CONNECT went, the
    /// station is matched for commanding as soon as -- or for as long as --
    /// it is matched for listening. A CONNECT answered with another checksum
    /// does not match, and leaves what was matched as it was.
    ///
    /// # Arguments
    ///
    /// * `theirs` - The checksum the answer carried.
    /// * `mark`   - What [`Matched::before_connect`] said before it went.
    pub fn connect_answered(&self, theirs: ConfigDigest, mark: u64) {
        if Some(theirs) == self.0.ours {
            self.0.connected.store(mark + 1, Ordering::SeqCst);
        }
    }

    /// What the station has received from the vehicle so far.
    pub fn counts(&self) -> Counts {
        Counts {
            processed: self.0.processed.load(Ordering::Relaxed),
            malformed: self.0.malformed.load(Ordering::Relaxed),
            mismatched: self.0.mismatched.load(Ordering::Relaxed),
        }
    }

    /// Counts a packet read and handed on.
    pub(crate) fn processed(&self) {
        self.0.processed.fetch_add(1, Ordering::Relaxed);
    }

    /// Counts a packet that could not be read with the configuration's
    /// definitions.
    pub(crate) fn malformed(&self) {
        self.0.malformed.fetch_add(1, Ordering::Relaxed);
    }

    /// Counts packets dropped because the two ends may not be using the
    /// same configuration.
    ///
    /// # Arguments
    ///
    /// * `how_many` - How many were dropped.
    pub(crate) fn mismatched(&self, how_many: u64) {
        self.0.mismatched.fetch_add(how_many, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OURS: ConfigDigest = ConfigDigest([0x5A; 16]);
    const THEIRS: ConfigDigest = ConfigDigest([0x11; 16]);

    /// Through every transition: a flagged beacon, a clear one, a CONNECT
    /// answered, a beacon overdue, one of another configuration, a start of
    /// tcspecial, and a CONNECT answered with another checksum.
    #[test]
    fn a_station_is_matched_through_each_transition() {
        let matched = Matched::for_configuration(OURS);
        let client = matched.clone();
        assert!(!matched.for_listening(), "no beacon heard yet");
        assert!(!matched.for_commanding());

        // A start of tcspecial, flagged: nothing.
        matched.beacon(true, Some(OURS), Stage::Heard, false);
        assert!(!matched.for_listening(), "flagged");

        // A CONNECT answered with ours while still flagged: remembered, so
        // that the clear beacon that follows matches commanding too.
        let mark = client.before_connect();
        client.connect_answered(OURS, mark);
        assert!(!client.for_commanding(), "not listening yet");
        matched.beacon(false, Some(OURS), Stage::Heard, false);
        assert!(matched.for_listening(), "clear, ours, recent");
        assert!(client.for_commanding(), "and our CONNECT was answered");

        // Overdue: both end, and the next clear beacon matches listening
        // only, a new match, with a new CONNECT to make.
        matched.beacon(false, Some(OURS), Stage::Aging, false);
        assert!(!matched.for_listening(), "overdue");
        assert!(!client.for_commanding());
        matched.beacon(false, Some(OURS), Stage::Heard, false);
        assert!(matched.for_listening());
        assert!(!client.for_commanding(), "the old CONNECT does not count");

        // A CONNECT answered with another checksum does not match.
        let mark = client.before_connect();
        client.connect_answered(THEIRS, mark);
        assert!(!client.for_commanding(), "answered with another checksum");
        let mark = client.before_connect();
        client.connect_answered(OURS, mark);
        assert!(client.for_commanding());

        // Another configuration's beacon ends both.
        matched.beacon(false, Some(THEIRS), Stage::Heard, false);
        assert!(!matched.for_listening(), "another checksum");
        assert!(!client.for_commanding());

        // A start of tcspecial ends commanding even where its first beacon
        // heard is somehow clear and ours.
        matched.beacon(false, Some(OURS), Stage::Heard, false);
        let mark = client.before_connect();
        client.connect_answered(OURS, mark);
        assert!(client.for_commanding());
        matched.beacon(false, Some(OURS), Stage::Heard, true);
        assert!(matched.for_listening());
        assert!(!client.for_commanding(), "the start that answered is gone");

        // A CONNECT whose answer comes after the match ended does not
        // match: it was answered, if at all, by what ended.
        let mark = client.before_connect();
        matched.beacon(true, Some(OURS), Stage::Heard, false);
        client.connect_answered(OURS, mark);
        matched.beacon(false, Some(OURS), Stage::Heard, false);
        assert!(!client.for_commanding(), "answered across the end");
    }

    /// Every packet counted is in exactly one of the three counts, and the
    /// received count is their sum.
    #[test]
    fn the_counts_add_up() {
        let matched = Matched::for_configuration(OURS);
        matched.processed();
        matched.processed();
        matched.malformed();
        matched.mismatched(4);
        let counts = matched.counts();
        assert_eq!(
            counts,
            Counts {
                processed: 2,
                malformed: 1,
                mismatched: 4
            }
        );
        assert_eq!(counts.received(), 7);
    }

    /// A station with no checksum of its own is never matched.
    #[test]
    fn a_station_with_no_checksum_is_never_matched() {
        let matched = Matched::with_no_checksum();
        matched.beacon(false, Some(OURS), Stage::Heard, false);
        let mark = matched.before_connect();
        matched.connect_answered(OURS, mark);
        assert!(!matched.for_listening());
        assert!(!matched.for_commanding());
    }
}
