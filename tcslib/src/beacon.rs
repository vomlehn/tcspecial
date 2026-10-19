//! Listening for a spacecraft's beacons.
//!
//! A beacon says that tcspecial is running, which build it is, which version
//! of a configuration it is serving and the digest of the files it read. It
//! arrives on its own, every `beacon.interval`, over a socket of its own, and
//! it keeps arriving whether or not a ground station has connected -- so it is
//! the one thing a ground station can know about a spacecraft without having
//! asked it anything.
//!
//! What a ground station does with that divides in two, and this is the half
//! that is not a window.
//!
//! * The facts: what a datagram turned out to be, what its number says about
//!   the ones before it, whether the configuration it announces is the one
//!   this station read, and how long ago the last one was.
//! * The look: a colour, a light that blinks, a line of text in a box.
//!
//! Only the first is here. The second needs a toolkit and a window to put it
//! in, and lives in `tcsui`, whose panel shows what this reports. They are
//! separated because the facts outlast any one window: tcsmoc shows them in a
//! panel beside its command link, the `beacon` example shows the same panel
//! and nothing else, and a ground station with no window at all -- a logger, a
//! test -- wants them with no toolkit linked in.
//!
//! # Staleness
//!
//! A beacon is not a request and is not answered, so the only thing that ever
//! happens is that one fails to arrive. How long ago the last one was is
//! therefore the whole of the state, and what a ground station shows is how
//! that age has grown: recently heard, ageing, late, silent. [`Schedule`] is
//! those stages and how long each lasts, with no colours in it -- a stage is
//! what is true, and which colour says so is the window's business.
//!
//! The stages also decide how long to wait on the socket. A station whose
//! indicator blinks has to wake up to blink it even when nothing arrives, so
//! [`Showing::until`] is how long until the next change and is what the read
//! timeout is set to.

use std::io;
use std::net::UdpSocket;
use std::time::{Duration, SystemTime};

use tcslibgs::{came_from, ConfigDigest, ConfigVersion, Telemetry, Timestamp, NO_TRANSFER_TIME};

/// Where beacons come from and where they are received.
///
/// The shared library's [`tcslibgs::BeaconLink`], under the name this module
/// uses for it: one type for the pair, so that the end that sends and the end
/// that listens cannot come to disagree about where the link is. Taken from
/// the configuration rather than stated again by a ground station, because a
/// station listening where the spacecraft is not sending hears nothing and
/// says nothing about why.
pub use tcslibgs::BeaconLink as Link;

/// How stale the last beacon is.
///
/// Named by what is true of the spacecraft rather than by what the window
/// does about it. A colour is a way of saying one of these, and not the only
/// one: a program acts on [`Contact`], an operator reads a light, and a log
/// line would write the word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// No beacon has arrived, or the last one is in the future -- a clock
    /// that moved, which is no reading at all.
    Unheard,
    /// One arrived recently enough to count: the spacecraft is being heard
    /// from.
    ///
    /// Recently enough allows for a beacon that did not arrive, one lost
    /// datagram being a network rather than a spacecraft. The standard
    /// schedule holds this stage for twice the beacon interval, steady at
    /// first and then blinking -- a light that has begun to blink says the
    /// beacons are ageing, which is worth noticing before it is worth
    /// doubting.
    Heard,
    /// Late enough that contact is not claimed, and not long enough to call
    /// it silence.
    ///
    /// Several beacons have not arrived. Nothing says whether the spacecraft,
    /// the link or this station is the reason, which is why the stage is
    /// named for what is known.
    Ageing,
    /// Long enough that nothing is being heard from the spacecraft.
    Silent,
}

impl Stage {
    /// Whether the spacecraft is being heard from at this stage.
    ///
    /// [`Stage::Heard`] and nothing else. It is the one question a program can
    /// act on: a panel may go on showing what a command implied only while the
    /// spacecraft is still being heard from, because a status nobody can
    /// confirm is a status nobody knows. An ageing beacon is not a confirmed
    /// one, so the line is drawn where the light stops saying all is well.
    pub fn in_contact(self) -> bool {
        matches!(self, Stage::Heard)
    }
}

/// Whether the spacecraft is still being heard from.
///
/// [`Stage::in_contact`] in the form the rest of a program passes around.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contact {
    /// Beacons are arriving, or the last one is recent enough to count.
    Made,
    /// None has arrived, or the last one is old enough not to.
    Lost,
}

impl Contact {
    /// The contact a stage amounts to.
    pub fn at(stage: Stage) -> Contact {
        if stage.in_contact() {
            Contact::Made
        } else {
            Contact::Lost
        }
    }
}

/// How long a light is on and off while it blinks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blink {
    pub on: Duration,
    pub off: Duration,
}

/// One stage of staleness and how long it lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    /// How long after the step before it this one holds.
    pub holds: Duration,
    /// What is true of the spacecraft while it does.
    pub stage: Stage,
    /// Whether what shows it blinks, and how fast. `None` is steady.
    ///
    /// Here rather than in the window because the blink decides when there is
    /// something to do: a station whose light blinks must wake to blink it,
    /// so the rate is what a read timeout is worked out from.
    pub blink: Option<Blink>,
}

/// The stages a beacon's age passes through, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    steps: Vec<Step>,
}

/// What to show, for an age.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Showing {
    /// How stale the last beacon is.
    pub stage: Stage,
    /// Whether a light showing it is lit now. A steady stage is always lit;
    /// a blinking one is lit for `on` and dark for `off`.
    pub lit: bool,
    /// How long until this changes by itself, where it will.
    ///
    /// `None` is a stage that will not change without a beacon: nothing has
    /// been heard, or the last stage has been reached and does not blink.
    /// That is what a socket may wait on indefinitely.
    pub until: Option<Duration>,
}

impl Showing {
    /// Nothing heard, and nothing to wait for but a beacon.
    const UNHEARD: Showing = Showing {
        stage: Stage::Unheard,
        lit: true,
        until: None,
    };
}

impl Schedule {
    /// A schedule of these steps, in order of age.
    pub fn of(steps: Vec<Step>) -> Schedule {
        Schedule { steps }
    }

    /// What tcsmoc shows, and what the `beacon` example shows.
    ///
    /// Four seconds steady, four more blinking at the same stage, six at the
    /// next and silence after that. The figures are read against a beacon
    /// interval of five seconds: one beacon may be missed without the stage
    /// changing to anything alarming, because one lost datagram is a network
    /// and not a spacecraft, and two missed is worth an operator's attention.
    ///
    /// The last step never ends. A schedule whose steps ran out would leave
    /// the oldest reading looking like the newest -- see [`Schedule::at`] --
    /// so the last one holds for ever, and blinks, because a light that has
    /// stopped changing is one an operator stops reading.
    pub fn standard() -> Schedule {
        let blink = |on: u64, off: u64| {
            Some(Blink {
                on: Duration::from_millis(on),
                off: Duration::from_millis(off),
            })
        };

        Schedule::of(vec![
            Step {
                holds: Duration::from_secs(4),
                stage: Stage::Heard,
                blink: None,
            },
            Step {
                holds: Duration::from_secs(4),
                stage: Stage::Heard,
                blink: blink(1000, 1000),
            },
            Step {
                holds: Duration::from_secs(6),
                stage: Stage::Ageing,
                blink: blink(1000, 1000),
            },
            Step {
                holds: Duration::MAX,
                stage: Stage::Silent,
                blink: blink(500, 500),
            },
        ])
    }

    /// The steps, for a window that colours them.
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// How long contact is held after a beacon.
    ///
    /// The leading run of steps whose stage counts as contact. A later step of
    /// such a stage is not part of it and there is no sense in one: these are
    /// read in order and say how stale the last beacon is, so a stage
    /// returning after an older one would be an indicator going backwards.
    pub fn while_in_contact(&self) -> Duration {
        let mut held = Duration::ZERO;
        for step in &self.steps {
            if !step.stage.in_contact() {
                break;
            }
            held = held.saturating_add(step.holds);
        }
        held
    }

    /// What to show for a beacon received this long ago.
    ///
    /// An age past the last step is the last step, which the standard
    /// schedule makes unreachable by not ending.
    pub fn at(&self, elapsed: Duration) -> Showing {
        let mut from = Duration::ZERO;

        for step in &self.steps {
            let ends = from.saturating_add(step.holds);
            if elapsed >= ends && step.holds != Duration::MAX {
                from = ends;
                continue;
            }

            let left = ends.saturating_sub(elapsed);
            let Some(blink) = step.blink else {
                return Showing {
                    stage: step.stage,
                    lit: true,
                    until: Some(left),
                };
            };

            // Where in the blink this age falls. Worked out in nanoseconds
            // because the cycle may be any length and a remainder is the only
            // way to ask.
            let cycle = blink.on.saturating_add(blink.off).as_nanos();
            if cycle == 0 {
                return Showing {
                    stage: step.stage,
                    lit: true,
                    until: Some(left),
                };
            }
            let into = elapsed.saturating_sub(from).as_nanos() % cycle;
            let on = blink.on.as_nanos();
            let (lit, until) = if into < on {
                (true, Duration::from_nanos((on - into) as u64))
            } else {
                (false, Duration::from_nanos((cycle - into) as u64))
            };

            return Showing {
                stage: step.stage,
                lit,
                until: Some(until.min(left)),
            };
        }

        Showing::UNHEARD
    }

    /// What to show, given when the last beacon arrived and the time now.
    ///
    /// Nothing heard is [`Stage::Unheard`], and so is a last beacon in the
    /// future: a clock that has moved leaves an age that cannot be worked
    /// out, and that is not a reason to claim anything about the spacecraft.
    pub fn after(&self, last: Option<SystemTime>, now: SystemTime) -> Showing {
        let Some(last) = last else {
            return Showing::UNHEARD;
        };
        let Ok(elapsed) = now.duration_since(last) else {
            return Showing::UNHEARD;
        };
        self.at(elapsed)
    }

    /// Whether the spacecraft is being heard from, for a beacon this old.
    ///
    /// Against the leading run of stages and not against the stage this age
    /// falls in -- see [`Schedule::while_in_contact`]. The two differ only for
    /// a schedule that returns to a stage of contact after leaving it, which
    /// is a schedule saying the spacecraft is heard from again because time
    /// has passed. The run is what contact means.
    pub fn contact(&self, elapsed: Duration) -> Contact {
        if elapsed < self.while_in_contact() {
            Contact::Made
        } else {
            Contact::Lost
        }
    }

    /// Whether the spacecraft is being heard from, `now`.
    ///
    /// Nothing heard is [`Contact::Lost`], and so is a last beacon in the
    /// future: an age that cannot be worked out is not a reason to claim
    /// contact.
    pub fn contact_at(&self, last: Option<SystemTime>, now: SystemTime) -> Contact {
        let Some(last) = last else {
            return Contact::Lost;
        };
        let Ok(elapsed) = now.duration_since(last) else {
            return Contact::Lost;
        };
        self.contact(elapsed)
    }
}

/// What the message line says before any beacon has arrived.
///
/// Said rather than left empty, because an empty line next to a label reads as
/// a line the program has not filled in rather than one the spacecraft has
/// not.
pub const NO_BEACON_MESSAGE: &str = "<none>";

/// When a beacon arrived, as a window shows it.
///
/// The time it arrived at the ground station, not a time the spacecraft put in
/// it: what the line is read for is whether beacons are still coming, and a
/// spacecraft whose clock had stopped would otherwise look as though its
/// beacons had. Shown the way every other time is, by the one function that
/// shows them.
pub fn received_at(at: Option<SystemTime>) -> String {
    match at {
        Some(at) => Timestamp::at(at).time_of_day(),
        None => NO_TRANSFER_TIME.to_string(),
    }
}

/// What one end of the link is running and reading, as one line.
///
/// Three things, each labelled: the version of the software, the version the
/// configuration states, and the digest of the files it read.
///
/// ```text
/// tcspecial v0.1.0, config v1.0.0, md5 36999e46a4bf436854442d8f97a2248d
/// ```
///
/// Both versions are three decimal parts, as they go on the link -- a byte
/// each -- and the digest is the hex of its sixteen bytes with nothing between
/// them.
///
/// One function because two messages carry these three facts: a beacon, which
/// arrives on its own, and the answer to a CONNECT, which is asked for. They
/// say the same thing about the same spacecraft, so they are read against each
/// other, and two format strings would eventually disagree about how to write
/// one of them.
pub fn what_it_read(
    version: ConfigVersion,
    config_version: ConfigVersion,
    digest: ConfigDigest,
) -> String {
    format!("tcspecial v{version}, config v{config_version}, md5 {digest}")
}

/// What a datagram on the beacon link turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heard {
    /// What a window shows for it.
    pub message: String,
    /// The beacon's own number, if it was a beacon this build can read.
    pub sequence: Option<u32>,
    /// What is wrong with the configuration it announces, if anything: see
    /// [`differs`].
    pub differs: Option<String>,
}

/// Read a datagram from the beacon link, comparing the digest it carries with
/// `expected`, this station's own.
pub fn hear(datagram: &[u8], expected: Option<ConfigDigest>) -> Heard {
    let beacon = match serde_json::from_slice::<Telemetry>(datagram) {
        Ok(Telemetry::Beacon(beacon)) => Some(beacon),
        _ => None,
    };
    let differs = match (&beacon, expected) {
        (Some(beacon), Some(ours)) => differs(beacon.digest, ours),
        _ => None,
    };
    let message = message_of(datagram);
    Heard {
        message: match &differs {
            Some(why) => format!("{why} -- {message}"),
            None => message,
        },
        sequence: beacon.map(|beacon| beacon.header.sequence),
        differs,
    }
}

/// Whether the configuration tcspecial announces is the one this station read.
///
/// `theirs` is the digest tcspecial computed from the files it read and sent in
/// its beacon; `ours` is the one this station computed from the files it read.
/// Different digests are different configurations -- a node, an attribute or a
/// device id that is not the same -- so what this station says about where
/// each source's data arrives may not be where the vehicle sends it. `None`
/// when they agree.
pub fn differs(theirs: ConfigDigest, ours: ConfigDigest) -> Option<String> {
    (theirs != ours).then(|| {
        format!(
            "CONFIGURATION DIFFERS: tcspecial is serving md5 {theirs}, and this MOC \
             read md5 {ours}"
        )
    })
}

/// What a beacon said, as a window shows it.
///
/// A datagram this cannot read is shown as what arrived, cut short. Something
/// else is sending to this address, or something is sending a beacon this
/// build does not understand, and either is worth seeing on the line rather
/// than hidden behind a green light.
pub fn message_of(datagram: &[u8]) -> String {
    match serde_json::from_slice::<Telemetry>(datagram) {
        Ok(Telemetry::Beacon(beacon)) => {
            what_it_read(beacon.version, beacon.config_version, beacon.digest)
        }
        Ok(other) => format!("not a beacon: {:?}", other.tm_type()),
        Err(_) => {
            let text = String::from_utf8_lossy(datagram);
            let head: String = text.chars().take(40).collect();
            if head.len() < text.len() {
                format!("unreadable: {head}...")
            } else {
                format!("unreadable: {head}")
            }
        }
    }
}

/// What a beacon's number says about the ones before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Counted {
    /// The first beacon this station has heard.
    First,
    /// The one after the last: nothing was missed.
    Next,
    /// This many beacons did not arrive.
    Missed(u32),
    /// The numbering started again, which is a spacecraft that restarted.
    Restarted,
}

/// What `now` says about the beacons between it and `last`.
///
/// This is what the number is for. A beacon says nothing but that the
/// spacecraft is alive, and without a number a ground station cannot tell a
/// beacon it missed -- a lost datagram, a network that dropped it -- from one
/// the spacecraft never sent. The age says how long ago the last one was; this
/// says how many there should have been.
pub fn counted(last: Option<u32>, now: u32) -> Counted {
    match last {
        None => Counted::First,
        // A counter kept by the sender goes backwards only when the sender
        // starts again, so that is what this is. It would also do so after
        // 2^32 beacons, which at five seconds each is six centuries; a wrap is
        // not worth telling from a restart.
        Some(last) if now <= last => Counted::Restarted,
        Some(last) if now == last + 1 => Counted::Next,
        Some(last) => Counted::Missed(now - last - 1),
    }
}

/// What a pass of the loop found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arrived {
    /// A beacon, and what it said.
    Beacon(String),
    /// Nothing, for as long as the pass waited.
    Nothing,
    /// The socket failed, which says nothing about the spacecraft.
    Broken,
}

/// What one pass of the loop leaves to be shown and said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pass {
    /// What this pass found.
    pub arrived: Arrived,
    /// How stale the last beacon is now, and whether a light is lit.
    pub showing: Showing,
    /// When the last beacon arrived, if one ever has.
    ///
    /// Kept through the passes that find nothing, and through a pass whose
    /// socket broke: a beacon arrived when it arrived, whatever the passes
    /// after it find, and blanking the line would say the spacecraft had gone
    /// quiet about something it has not.
    pub at: Option<SystemTime>,
    /// What that beacon said.
    pub said: String,
    /// Whether the spacecraft is being heard from, which is the same reading
    /// as the stage in the form a program acts on.
    pub contact: Contact,
    /// What is worth saying on a console about this pass, in order.
    ///
    /// Here rather than printed, so that a window may show them, a test may
    /// read them and a program with no console may ignore them -- and so that
    /// two ground stations watching one spacecraft say the same words about
    /// the same thing. Empty on almost every pass: these are the things that
    /// have changed.
    pub remarks: Vec<String>,
}

/// A ground station's beacon socket and what it has heard.
///
/// The loop is the caller's: a window runs it on a thread of its own and a
/// program with no window runs it where it stands. What a pass does is in
/// [`Beacons::pass`], which waits no longer than the next change of what is
/// shown.
pub struct Beacons {
    socket: UdpSocket,
    link: Link,
    schedule: Schedule,
    /// The digest this station computed from the configuration it read, which
    /// every beacon's digest is compared with. `None` where it could not
    /// digest its own, and then nothing is compared.
    expected: Option<ConfigDigest>,
    /// When the last beacon arrived.
    at: Option<SystemTime>,
    /// What it said.
    said: String,
    /// The number of the last beacon, so the next can say what happened in
    /// between.
    sequence: Option<u32>,
    /// Whether the last beacon announced a different configuration, so that
    /// the change is remarked on and the state is not.
    differed: bool,
}

impl Beacons {
    /// Bind where the configuration says beacons are received.
    ///
    /// Plain UDP from one named place to one other. It was a bind to a port on
    /// every interface followed by a join to a multicast group on a named one:
    /// that bought any number of ground stations hearing one beacon, and cost
    /// an address that was nobody's and an interface attribute whose being
    /// wrong lost every beacon in silence.
    pub fn listening(
        link: Link,
        schedule: Schedule,
        expected: Option<ConfigDigest>,
    ) -> io::Result<Beacons> {
        let socket = UdpSocket::bind(link.recv.to_string())?;
        Ok(Beacons {
            socket,
            link,
            schedule,
            expected,
            at: None,
            said: NO_BEACON_MESSAGE.to_string(),
            sequence: None,
            differed: false,
        })
    }

    /// Where this is listening and where it expects beacons from.
    pub fn link(&self) -> &Link {
        &self.link
    }

    /// The stages it ages the last beacon through.
    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    /// Wait for a beacon, or for the moment what is shown would change.
    ///
    /// One pass of the loop. It returns on a datagram, on the socket's
    /// timeout -- which is [`Showing::until`], so that a light blinks whether
    /// or not anything arrives -- or on a failure of the socket, which is
    /// reported as [`Arrived::Broken`] and says nothing about the spacecraft.
    pub fn pass(&mut self) -> Pass {
        let waiting = self.schedule.after(self.at, SystemTime::now());
        // A failure to set the timeout is treated as the socket having
        // failed, which is what it is: the alternative is a pass that waits
        // for ever on a socket the operating system has refused to time out.
        if self.socket.set_read_timeout(waiting.until).is_err() {
            return self.after(Arrived::Broken, Vec::new());
        }

        let mut buffer = [0u8; 65535];
        let mut remarks = Vec::new();
        let arrived = match self.socket.recv_from(&mut buffer) {
            // Only from where the configuration says beacons come from. A
            // datagram from anywhere else at this address is somebody else's
            // traffic, and treating it as a beacon would be reporting the
            // spacecraft alive on the word of whoever sent it.
            Ok((_, from)) if !came_from(&self.link.send, &from) => {
                remarks.push(format!(
                    "a beacon came from {from} and not from {}, which is where the \
                     configuration says they come from -- ignored",
                    self.link.send
                ));
                Arrived::Nothing
            }
            Ok((size, _)) => {
                let heard = hear(&buffer[..size], self.expected);

                // A configuration that differs is the one thing a beacon can
                // say that makes every panel suspect, so it is remarked on
                // when it starts and when it stops -- and not on every beacon
                // in between, which would bury it.
                if heard.differs.is_some() != self.differed {
                    match &heard.differs {
                        Some(why) => remarks.push(why.clone()),
                        None => remarks.push(
                            "tcspecial is serving the configuration this MOC read again"
                                .to_string(),
                        ),
                    }
                    self.differed = heard.differs.is_some();
                }

                if let Some(sequence) = heard.sequence {
                    match counted(self.sequence, sequence) {
                        Counted::Missed(missed) => remarks.push(format!(
                            "{missed} beacon{} did not arrive: #{sequence} came after #{}",
                            if missed == 1 { "" } else { "s" },
                            self.sequence.unwrap_or(0)
                        )),
                        Counted::Restarted => remarks.push(format!(
                            "the beacons start again at #{sequence}, so tcspecial has \
                             restarted"
                        )),
                        Counted::First | Counted::Next => {}
                    }
                    self.sequence = Some(sequence);
                }

                Arrived::Beacon(heard.message)
            }
            Err(ref e)
                if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut =>
            {
                Arrived::Nothing
            }
            Err(_) => Arrived::Broken,
        };

        self.after(arrived, remarks)
    }

    /// Fold what a pass found into what is known, and say what to show.
    ///
    /// Apart from the socket so that it can be exercised without one. What it
    /// keeps is the point: a pass that found nothing still has the beacon
    /// before it to show, and so does a pass whose socket broke.
    fn after(&mut self, arrived: Arrived, remarks: Vec<String>) -> Pass {
        let now = SystemTime::now();

        if let Arrived::Beacon(message) = &arrived {
            self.at = Some(now);
            self.said = message.clone();
        }

        // A broken socket is not an age: the station has stopped listening,
        // which is nothing the spacecraft did, so what is shown is that
        // nothing is known rather than a beacon growing stale.
        let showing = match arrived {
            Arrived::Broken => Showing::UNHEARD,
            _ => self.schedule.after(self.at, now),
        };

        Pass {
            arrived,
            showing,
            at: self.at,
            said: self.said.clone(),
            // From the same reading of the same beacon as the stage, and from
            // the beacon rather than from the socket: a station whose socket
            // failed knows nothing new about the spacecraft, so what it last
            // heard still decides whether it is in contact.
            contact: self.schedule.contact_at(self.at, now),
            remarks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslibgs::{BeaconTelemetry, CommandStatus, HostPort, PingTelemetry};

    /// A schedule with one stage of contact and one without, so that a test
    /// can tell a fresh beacon from a stale one.
    fn schedule() -> Schedule {
        Schedule::of(vec![
            Step {
                holds: Duration::from_secs(4),
                stage: Stage::Heard,
                blink: None,
            },
            Step {
                holds: Duration::MAX,
                stage: Stage::Silent,
                blink: None,
            },
        ])
    }

    /// Contact is held while the stage is one of contact, and lost after.
    ///
    /// This is what the panels act on: a handler's status is the answer to a
    /// command, and the ground may go on showing it only while it is still
    /// hearing from the spacecraft. The stages an operator reads age through
    /// several looks; what a program needs out of that is the one question of
    /// whether anything is being heard.
    #[test]
    fn contact_is_held_while_the_stage_is_one_of_contact() {
        let schedule = schedule();
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let last = Some(at);

        // Four seconds of it, so contact is held through them and lost after.
        // The instant it arrived counts: a beacon just received is the
        // strongest contact there is.
        assert_eq!(schedule.contact_at(last, at), Contact::Made);
        assert_eq!(
            schedule.contact_at(last, at + Duration::from_millis(3_999)),
            Contact::Made
        );
        assert_eq!(
            schedule.contact_at(last, at + Duration::from_millis(4_001)),
            Contact::Lost,
            "the spacecraft has gone silent, so it is not being heard from"
        );

        // Nothing heard at all is the same position as having stopped
        // hearing: the ground knows nothing either way.
        assert_eq!(schedule.contact_at(None, at), Contact::Lost);

        // A beacon in the future is a clock that moved, which is treated as
        // never having heard anything: an elapsed time that cannot be worked
        // out is not a reason to claim contact.
        assert_eq!(
            schedule.contact_at(Some(at + Duration::from_secs(60)), at),
            Contact::Lost
        );
    }

    /// A blinking stage of contact is still contact, and only the leading run
    /// of stages counts.
    ///
    /// The standard schedule holds the heard stage steady for four seconds and
    /// then blinks it for four more: a blinking light says beacons are
    /// arriving and ageing, not that they have stopped. The run stops at the
    /// first stage that is not one of contact, whatever comes after it.
    #[test]
    fn a_blinking_stage_of_contact_is_still_contact() {
        let schedule = Schedule::of(vec![
            Step {
                holds: Duration::from_secs(4),
                stage: Stage::Heard,
                blink: None,
            },
            Step {
                holds: Duration::from_secs(4),
                stage: Stage::Heard,
                blink: Some(Blink {
                    on: Duration::from_secs(1),
                    off: Duration::from_secs(1),
                }),
            },
            Step {
                holds: Duration::from_secs(6),
                stage: Stage::Silent,
                blink: None,
            },
            // A stage of contact again, which is not reached: the run ended
            // above.
            Step {
                holds: Duration::MAX,
                stage: Stage::Heard,
                blink: None,
            },
        ]);

        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let last = Some(at);

        assert_eq!(schedule.while_in_contact(), Duration::from_secs(8));
        assert_eq!(
            schedule.contact_at(last, at + Duration::from_millis(7_999)),
            Contact::Made,
            "it is still blinking at a stage of contact"
        );
        assert_eq!(
            schedule.contact_at(last, at + Duration::from_millis(8_001)),
            Contact::Lost
        );
        assert_eq!(
            schedule.contact_at(last, at + Duration::from_secs(20)),
            Contact::Lost,
            "a later stage of contact is not a run of it"
        );
    }

    /// A blinking stage is lit and dark in turn, and says when it next
    /// changes.
    ///
    /// The second half is what a socket's read timeout is set to: a light that
    /// blinks has to be woken up to blink, so a pass that found nothing must
    /// still be told when the next thing happens.
    #[test]
    fn a_blinking_stage_says_when_it_next_changes() {
        let schedule = Schedule::of(vec![
            Step {
                holds: Duration::from_secs(2),
                stage: Stage::Heard,
                blink: None,
            },
            Step {
                holds: Duration::MAX,
                stage: Stage::Silent,
                blink: Some(Blink {
                    on: Duration::from_millis(500),
                    off: Duration::from_millis(500),
                }),
            },
        ]);

        // Steady to begin with, and the wait is until it stops being steady.
        let fresh = schedule.at(Duration::ZERO);
        assert_eq!(fresh.stage, Stage::Heard);
        assert!(fresh.lit);
        assert_eq!(fresh.until, Some(Duration::from_secs(2)));

        // Into the blinking stage: lit for the first half of the cycle, dark
        // for the second, and each says how long is left of it.
        let lit = schedule.at(Duration::from_millis(2_100));
        assert_eq!(lit.stage, Stage::Silent);
        assert!(lit.lit);
        assert_eq!(lit.until, Some(Duration::from_millis(400)));

        let dark = schedule.at(Duration::from_millis(2_600));
        assert!(!dark.lit, "the second half of the cycle is dark");
        assert_eq!(dark.until, Some(Duration::from_millis(400)));

        // And it goes on blinking however old the beacon is, because a light
        // that stopped changing is one an operator stops reading.
        let old = schedule.at(Duration::from_secs(3_600));
        assert_eq!(old.stage, Stage::Silent);
        assert!(old.until.is_some());
    }

    /// The standard schedule is what the figures in it are for.
    ///
    /// Read against a beacon interval of five seconds: one may be missed
    /// without the stage saying anything alarming, because one lost datagram
    /// is a network and not a spacecraft.
    #[test]
    fn the_standard_schedule_holds_contact_through_one_missed_beacon() {
        let schedule = Schedule::standard();

        assert_eq!(schedule.at(Duration::ZERO).stage, Stage::Heard);
        assert_eq!(
            schedule.at(Duration::from_secs(6)).stage,
            Stage::Heard,
            "one beacon missed at five seconds is still being heard from"
        );
        assert_eq!(schedule.while_in_contact(), Duration::from_secs(8));
        assert_eq!(schedule.at(Duration::from_secs(9)).stage, Stage::Ageing);
        assert_eq!(schedule.at(Duration::from_secs(20)).stage, Stage::Silent);
        assert_eq!(
            schedule.at(Duration::from_secs(86_400)).stage,
            Stage::Silent,
            "the last stage never ends"
        );
    }

    /// The digest tcspecial sent is compared with the one this station
    /// computed: the same says nothing, and a different one says both.
    #[test]
    fn a_beacon_announcing_another_configuration_says_so() {
        let ours = ConfigDigest([0xab; 16]);
        let theirs = ConfigDigest([0xcd; 16]);
        assert_eq!(differs(ours, ours), None);
        let said = differs(theirs, ours).expect("they differ");
        assert!(said.contains(&theirs.to_string()), "{said}");
        assert!(said.contains(&ours.to_string()), "{said}");

        let version = ConfigVersion::of_this_build();
        let datagram = serde_json::to_vec(&Telemetry::Beacon(BeaconTelemetry::new(
            1, version, version, theirs,
        )))
        .expect("it serializes");

        let heard = hear(&datagram, Some(ours));
        assert!(heard.differs.is_some());
        assert!(
            heard.message.starts_with("CONFIGURATION DIFFERS"),
            "{}",
            heard.message
        );

        let heard = hear(&datagram, Some(theirs));
        assert_eq!(heard.differs, None);
        assert!(!heard.message.contains("DIFFERS"), "{}", heard.message);

        // And with nothing to compare against, nothing is said.
        assert_eq!(hear(&datagram, None).differs, None);
    }

    /// A beacon's own words are what the line shows: the build flying and the
    /// configuration it read.
    #[test]
    fn a_beacon_is_shown_by_what_it_said() {
        let version = ConfigVersion {
            major: 1,
            minor: 2,
            patch: 3,
        };
        let config_version = ConfigVersion {
            major: 4,
            minor: 5,
            patch: 6,
        };
        let digest = ConfigDigest([0xab; 16]);
        let beacon = Telemetry::Beacon(BeaconTelemetry::new(3, version, config_version, digest));
        let datagram = serde_json::to_vec(&beacon).expect("it serializes");

        // Each labelled, each version three decimal parts, and the digest hex
        // with nothing between the bytes.
        let want = format!("tcspecial v1.2.3, config v4.5.6, md5 {}", "ab".repeat(16));
        assert_eq!(message_of(&datagram), want);

        // And the answer to a CONNECT is written by the same function, so the
        // two lines cannot come to disagree about how to say it.
        assert_eq!(what_it_read(version, config_version, digest), want);

        // The beacon's own number comes back beside the message, which is
        // what tells a beacon that was missed from one that was never sent.
        assert_eq!(hear(&datagram, None).sequence, Some(3));
        assert_eq!(
            hear(b"hello", None).sequence,
            None,
            "not a beacon, no number"
        );

        // Something else sending to this address is shown rather than hidden:
        // a green light beside a line that said nothing would be a window
        // reporting a beacon it had not had.
        let shown = message_of(b"hello");
        assert!(
            shown.contains("unreadable") && shown.contains("hello"),
            "{shown}"
        );

        // And a long one is cut short rather than filling the window.
        let shown = message_of(&[b'x'; 200]);
        assert!(shown.ends_with("..."), "{shown}");
        assert!(shown.len() < 60, "{shown}");

        // Telemetry that is not a beacon says so. Nothing sends a response to
        // this address, so this is a misconfiguration worth naming.
        let other = Telemetry::Ping(PingTelemetry::new(1, CommandStatus::Success));
        let datagram = serde_json::to_vec(&other).expect("it serializes");
        let shown = message_of(&datagram);
        assert!(shown.contains("not a beacon"), "{shown}");
    }

    /// A beacon's number says how many did not arrive.
    ///
    /// Which is the whole of what it is for. A beacon says nothing but that
    /// the spacecraft is alive, so without a number a ground station cannot
    /// tell a beacon it missed from one the spacecraft never sent -- and
    /// those are a network to look at and a spacecraft to look at.
    #[test]
    fn a_beacons_number_says_what_happened_before_it() {
        assert_eq!(counted(None, 1), Counted::First);
        assert_eq!(
            counted(None, 4096),
            Counted::First,
            "a station may start late"
        );

        assert_eq!(counted(Some(1), 2), Counted::Next);
        assert_eq!(counted(Some(41), 42), Counted::Next);

        assert_eq!(counted(Some(5), 7), Counted::Missed(1));
        assert_eq!(counted(Some(5), 105), Counted::Missed(99));

        // A number that does not go forward is a sender that started again:
        // the count is the sender's, and it begins at one.
        assert_eq!(counted(Some(99), 1), Counted::Restarted);
        assert_eq!(counted(Some(99), 99), Counted::Restarted, "the same twice");
    }

    /// A beacon that has arrived is shown by the time it arrived, and one that
    /// has not is said not to have.
    ///
    /// The line was never set at all in the first window that had one: it
    /// declared the line, Rust set the indicator's colour beside it and
    /// nothing else, so it said no beacon had ever arrived for as long as the
    /// station ran. That is worse than saying nothing, because the colour next
    /// to it was green.
    #[test]
    fn a_beacon_is_shown_by_when_it_arrived() {
        assert_eq!(received_at(None), NO_TRANSFER_TIME);

        // The time of day it arrived, as every other time is shown: one
        // function formats them all.
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(3661);
        assert_eq!(received_at(Some(at)), "01:01:01");

        // A clock reading before the epoch is a clock that could not be read,
        // and shows the same placeholder as no beacon at all: the one thing it
        // must not do is panic, because a window that fell over because the
        // system clock was being set would be a worse fault than a time it
        // cannot show. Nor may it show the epoch, which it used to -- a line
        // reading 00:00:00 is a time an operator would believe.
        let before = SystemTime::UNIX_EPOCH - Duration::from_secs(1);
        assert_eq!(received_at(Some(before)), NO_TRANSFER_TIME);
    }

    /// Every pass says when the last beacon arrived, not only the pass that
    /// received one.
    ///
    /// This is what the first window was missing: the time of the last beacon
    /// has to survive the passes that find nothing, or the line would be
    /// filled in for an instant and emptied by the next timeout. A pass whose
    /// socket broke keeps it too -- the spacecraft has said nothing about
    /// having stopped, and blanking the line would say that it had.
    #[test]
    fn every_pass_says_when_the_last_beacon_arrived() {
        // A station listening on a port of its own, with no beacons coming:
        // the passes here are made by hand, so nothing is sent to it.
        let link = Link {
            send: HostPort {
                address: "127.0.0.1".to_string(),
                port: 1,
            },
            recv: HostPort {
                address: "127.0.0.1".to_string(),
                port: 0,
            },
        };
        let mut beacons =
            Beacons::listening(link, schedule(), None).expect("a socket on a port of its own");

        // Nothing has arrived yet, so there is nothing to show and nothing is
        // being heard from.
        let nothing = beacons.after(Arrived::Nothing, Vec::new());
        assert_eq!(nothing.at, None);
        assert_eq!(nothing.showing.stage, Stage::Unheard);
        assert_eq!(nothing.said, NO_BEACON_MESSAGE);
        assert_eq!(
            nothing.contact,
            Contact::Lost,
            "a station that has heard nothing is in contact with nothing"
        );

        // A beacon: the time is the time of this pass, and the line says what
        // the beacon said.
        let heard = beacons.after(
            Arrived::Beacon("tcspecial v9.9.9, config v1.0.0, md5 abc".to_string()),
            Vec::new(),
        );
        let arrival = heard.at.expect("a pass that received a beacon says when");
        assert_eq!(heard.said, "tcspecial v9.9.9, config v1.0.0, md5 abc");
        assert_eq!(heard.showing.stage, Stage::Heard);
        assert_eq!(
            heard.contact,
            Contact::Made,
            "a beacon has just arrived, so the spacecraft is being heard from"
        );

        // A later pass that finds nothing keeps that time and that message:
        // the beacon did arrive when it arrived and said what it said,
        // whatever the passes after it find.
        let quiet = beacons.after(Arrived::Nothing, Vec::new());
        assert_eq!(
            quiet.at,
            Some(arrival),
            "the time of the last beacon was lost"
        );
        assert_eq!(
            quiet.said, "tcspecial v9.9.9, config v1.0.0, md5 abc",
            "the last message was lost"
        );

        // And a broken socket says nothing about the spacecraft: the time and
        // the message stand, and what is *shown* is that nothing is known --
        // the station has stopped listening, which is not a beacon growing
        // stale.
        let broken = beacons.after(Arrived::Broken, Vec::new());
        assert_eq!(broken.at, Some(arrival));
        assert_eq!(broken.said, "tcspecial v9.9.9, config v1.0.0, md5 abc");
        assert_eq!(broken.showing.stage, Stage::Unheard);
        assert_eq!(
            broken.contact,
            Contact::Made,
            "the socket failed, and the last beacon is what says whether the \
             spacecraft is being heard from"
        );
    }

    /// A beacon from somewhere else is not a beacon.
    ///
    /// The address it comes from is in the configuration, so a datagram from
    /// anywhere else is somebody else's traffic: reporting the spacecraft
    /// alive on the word of whoever sent it is the one thing this must not do.
    #[test]
    fn a_datagram_from_the_wrong_place_is_not_a_beacon() {
        let listening = UdpSocket::bind("127.0.0.1:0").expect("a port for the station");
        let recv = listening.local_addr().expect("its address");
        drop(listening);

        // The configuration says beacons come from port 9: the sender below
        // has a port of its own and is therefore not it.
        let link = Link {
            send: HostPort {
                address: "127.0.0.1".to_string(),
                port: 9,
            },
            recv: HostPort {
                address: recv.ip().to_string(),
                port: recv.port(),
            },
        };
        let mut beacons = match Beacons::listening(link, Schedule::standard(), None) {
            Ok(beacons) => beacons,
            // The port was taken between being chosen and being bound, which
            // is a machine being busy and not a fault in any of this.
            Err(_) => return,
        };

        let stranger = UdpSocket::bind("127.0.0.1:0").expect("a socket for the stranger");
        let version = ConfigVersion::of_this_build();
        let datagram = serde_json::to_vec(&Telemetry::Beacon(BeaconTelemetry::new(
            1,
            version,
            version,
            ConfigDigest([0; 16]),
        )))
        .expect("it serializes");
        stranger
            .send_to(&datagram, recv)
            .expect("the stranger sends");

        let pass = beacons.pass();
        assert_eq!(
            pass.arrived,
            Arrived::Nothing,
            "a datagram from the wrong address was taken for a beacon"
        );
        assert_eq!(pass.at, None, "and it must not count as contact");
        assert!(
            pass.remarks.iter().any(|said| said.contains("ignored")),
            "the pass should say what it ignored: {:?}",
            pass.remarks
        );
    }
}
