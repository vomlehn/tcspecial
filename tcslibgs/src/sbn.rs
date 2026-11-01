//! SBN, the Software Bus Network, as tcspecial speaks it: one peer of a cFS
//! software bus, over SBN's UDP protocol module.
//!
//! The one statement of SBN's wire format and of how a peer is kept, read by
//! the vehicle and by whatever stands in for the cFS end. See "What SBN is,
//! on the wire" in `docs/design.rst`, which says what in it was checked
//! against SBN v7.0.1 and how.
//!
//! # A datagram
//!
//! ```text
//! length 4 | type 1 | processor ID 4 | spacecraft ID 4 | the message
//! ```
//!
//! Thirteen bytes and then the message, every number most significant byte
//! first. The length is the message's, not the datagram's.
//!
//! # A subscription
//!
//! ```text
//! ident 48 | count 2 | count x (message ID 4 | priority 1 | reliability 1)
//! ```
//!
//! An unsubscription is laid out the same. The ident is what opens every
//! one, and a subscription whose ident is not the receiver's is refused:
//! see [`ident_of`].
//!
//! # Keeping the peer
//!
//! [`Liveness`] is the timing, and nothing but: when to announce, when to
//! send a heartbeat and when silence has gone on long enough to disconnect.
//! It is a value asked with the time, so that what it decides can be tested
//! without waiting five or ten seconds for it.

use std::time::{Duration, Instant};

/// An SBN release this build speaks, and what its peers say about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Release {
    /// The release, as `sbn.release` names it.
    pub name: &'static str,
    /// Its `SBN_PROTO_VER`, which its protocol-version message carries.
    pub protocol_version: u8,
}

/// Every release this build speaks. The release decides the layout of every
/// message, so a release is added here only with the layouts it has, and
/// tested against them.
pub const RELEASES: &[Release] = &[Release {
    name: "v7.0.1",
    protocol_version: 11,
}];

/// Reads `sbn.release`.
///
/// # Arguments
///
/// * `said` - The release, as the file wrote it.
///
/// # Returns
///
/// The release.
///
/// # Errors
///
/// A message naming the releases this build speaks, for any other.
pub fn release_of(said: &str) -> Result<Release, String> {
    let said = said.trim();
    RELEASES
        .iter()
        .find(|release| release.name == said)
        .copied()
        .ok_or_else(|| {
            format!(
                "sbn.release is \"{said}\", which this build of tcspecial does \
                 not speak: the release decides the layout of every message, \
                 and this build speaks {}",
                RELEASES
                    .iter()
                    .map(|r| r.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

/// The length of a datagram's header.
pub const HEADER_BYTES: usize = 13;

/// The length of `SBN_IDENT`.
pub const IDENT_BYTES: usize = 48;

/// What one subscription takes in a subscription message.
const SUBSCRIPTION_BYTES: usize = 6;

/// The message types, as the header's type byte says them.
pub mod kind {
    /// A subscription: the message IDs the sender wants.
    pub const SUBSCRIBE: u8 = 1;
    /// An unsubscription: message IDs it no longer wants.
    pub const UNSUBSCRIBE: u8 = 2;
    /// An application's message: a cFS packet, carried whole.
    pub const APPLICATION: u8 = 3;
    /// SBN's protocol version, one byte.
    pub const PROTOCOL: u8 = 4;
    /// The UDP module's heartbeat, which carries nothing.
    pub const HEARTBEAT: u8 = 0xA0;
    /// Its announcement, which carries nothing.
    pub const ANNOUNCE: u8 = 0xA1;
    /// Its disconnect, which carries nothing.
    pub const DISCONNECT: u8 = 0xA2;
}

/// How long a peer may go without sending before it sends a heartbeat.
pub const HEARTBEAT_AFTER: Duration = Duration::from_secs(5);

/// How long a peer may be silent before it is disconnected.
pub const SILENCE: Duration = Duration::from_secs(10);

/// How often a peer not yet connected is announced to.
///
/// SBN's own is not one this side has to match: the far end connects on
/// whatever arrives first. Once a second is soon enough for a link coming
/// up and slow enough that a refused peer's every refusal can be logged.
pub const ANNOUNCE_EVERY: Duration = Duration::from_secs(1);

/// A datagram's header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// The message type.
    pub kind: u8,
    /// The sender's processor ID.
    pub processor_id: u32,
    /// The sender's spacecraft ID.
    pub spacecraft_id: u32,
}

impl Header {
    /// Writes a datagram: this header and then `message`.
    ///
    /// # Arguments
    ///
    /// * `into`    - Room for the header and the message.
    /// * `message` - What follows the header.
    ///
    /// # Returns
    ///
    /// The datagram's length, or `None` where it does not fit.
    pub fn write(&self, into: &mut [u8], message: &[u8]) -> Option<usize> {
        let total = HEADER_BYTES + message.len();
        if total > into.len() {
            return None;
        }
        self.write_before(into, message.len());
        into[HEADER_BYTES..total].copy_from_slice(message);
        Some(total)
    }

    /// Writes only this header, for a message already in place after it.
    ///
    /// # Arguments
    ///
    /// * `into`   - At least [`HEADER_BYTES`] of room.
    /// * `length` - The length of the message that follows.
    pub fn write_before(&self, into: &mut [u8], length: usize) {
        into[..4].copy_from_slice(&(length as u32).to_be_bytes());
        into[4] = self.kind;
        into[5..9].copy_from_slice(&self.processor_id.to_be_bytes());
        into[9..13].copy_from_slice(&self.spacecraft_id.to_be_bytes());
    }

    /// Reads a datagram's header, refusing one whose length is not what
    /// follows it.
    ///
    /// # Arguments
    ///
    /// * `datagram` - The whole datagram.
    ///
    /// # Returns
    ///
    /// The header and the message after it.
    ///
    /// # Errors
    ///
    /// What is wrong, in words a log line can use.
    pub fn read(datagram: &[u8]) -> Result<(Header, &[u8]), &'static str> {
        if datagram.len() < HEADER_BYTES {
            return Err("shorter than an SBN header");
        }
        let number = |at: usize| {
            u32::from_be_bytes([
                datagram[at],
                datagram[at + 1],
                datagram[at + 2],
                datagram[at + 3],
            ])
        };
        if number(0) as usize != datagram.len() - HEADER_BYTES {
            return Err("of a length its SBN header does not say");
        }
        Ok((
            Header {
                kind: datagram[4],
                processor_id: number(5),
                spacecraft_id: number(9),
            },
            &datagram[HEADER_BYTES..],
        ))
    }
}

/// Makes `SBN_IDENT` from the hash `sbn.ident` states.
///
/// `SBN_IDENT` is git's `$Id$` of SBN's `sbn_types.h` as it was checked out:
/// `$Id: ` and the file's 40-digit hash and ` $`, which with the C string's
/// terminating nought is the 48 bytes that open every subscription.
///
/// # Arguments
///
/// * `said` - `sbn.ident`, as the file wrote it.
///
/// # Returns
///
/// The 48 bytes.
///
/// # Errors
///
/// A message saying what to write, for anything but 40 hexadecimal digits.
pub fn ident_of(said: &str) -> Result<[u8; IDENT_BYTES], String> {
    let said = said.trim();
    if said.len() != 40 || !said.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!(
            "sbn.ident is \"{said}\", which is not the 40 hexadecimal digits \
             of the far end's SBN_IDENT: copy the hash SBN prints in its \
             start-up event on the cFS side"
        ));
    }
    let mut ident = [0u8; IDENT_BYTES];
    ident[..5].copy_from_slice(b"$Id: ");
    ident[5..45].copy_from_slice(said.to_ascii_lowercase().as_bytes());
    ident[45..47].copy_from_slice(b" $");
    Ok(ident)
}

/// Says an ident as text, for a log line: the hash inside it where it is
/// one, and its bytes in hexadecimal where it is not.
///
/// Writes into `out` rather than allocating, because it is said each time a
/// peer is refused, which happens in the loop.
///
/// # Arguments
///
/// * `ident` - The 48 bytes.
/// * `out`   - Room for the text: 96 bytes is always enough.
///
/// # Returns
///
/// The text.
pub fn ident_text<'a>(ident: &[u8], out: &'a mut [u8; 96]) -> &'a str {
    let printable = ident
        .iter()
        .take_while(|&&b| b != 0)
        .all(|b| b.is_ascii_graphic() || *b == b' ');
    if printable {
        let end = ident.iter().position(|&b| b == 0).unwrap_or(ident.len());
        let end = end.min(out.len());
        out[..end].copy_from_slice(&ident[..end]);
        return std::str::from_utf8(&out[..end]).unwrap_or("");
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut at = 0;
    for byte in ident.iter().take(out.len() / 2) {
        out[at] = HEX[usize::from(byte >> 4)];
        out[at + 1] = HEX[usize::from(byte & 0x0F)];
        at += 2;
    }
    std::str::from_utf8(&out[..at]).unwrap_or("")
}

/// Writes a subscription or unsubscription message.
///
/// Every message ID at priority and reliability nought, which is what SBN
/// gives a subscription that asks for nothing in particular.
///
/// # Arguments
///
/// * `into`   - Room for the message.
/// * `ident`  - The far end's `SBN_IDENT`.
/// * `msgids` - The message IDs.
///
/// # Returns
///
/// The message's length, or `None` where it does not fit or there are more
/// message IDs than a count of two bytes holds.
pub fn write_subscriptions(
    into: &mut [u8],
    ident: &[u8; IDENT_BYTES],
    msgids: impl ExactSizeIterator<Item = u16>,
) -> Option<usize> {
    let count = u16::try_from(msgids.len()).ok()?;
    let total = IDENT_BYTES + 2 + usize::from(count) * SUBSCRIPTION_BYTES;
    if total > into.len() {
        return None;
    }
    into[..IDENT_BYTES].copy_from_slice(ident);
    into[IDENT_BYTES..IDENT_BYTES + 2].copy_from_slice(&count.to_be_bytes());
    let mut at = IDENT_BYTES + 2;
    for msgid in msgids {
        into[at..at + 4].copy_from_slice(&u32::from(msgid).to_be_bytes());
        into[at + 4] = 0;
        into[at + 5] = 0;
        at += SUBSCRIPTION_BYTES;
    }
    Some(total)
}

/// Why a subscription message was not taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionRefusal<'a> {
    /// It is not laid out as a subscription is.
    Malformed(&'static str),
    /// It opens with another ident: the far end was built from another SBN.
    Ident(&'a [u8]),
}

/// Reads a subscription or unsubscription message, checking its ident.
///
/// # Arguments
///
/// * `message` - The message, after the datagram's header.
/// * `ident`   - The ident it must open with.
///
/// # Returns
///
/// Its message IDs, in the order it lists them. A message ID wider than the
/// sixteen bits of the version 2 mapping is passed on as it is, cut to
/// nothing: none of tcspecial's is, so it matches nothing.
///
/// # Errors
///
/// Why it was not taken.
pub fn read_subscriptions<'a>(
    message: &'a [u8],
    ident: &[u8; IDENT_BYTES],
) -> Result<impl Iterator<Item = Option<u16>> + 'a, SubscriptionRefusal<'a>> {
    if message.len() < IDENT_BYTES + 2 {
        return Err(SubscriptionRefusal::Malformed(
            "a subscription shorter than its ident and count",
        ));
    }
    if &message[..IDENT_BYTES] != ident {
        return Err(SubscriptionRefusal::Ident(&message[..IDENT_BYTES]));
    }
    let count = usize::from(u16::from_be_bytes([
        message[IDENT_BYTES],
        message[IDENT_BYTES + 1],
    ]));
    let entries = &message[IDENT_BYTES + 2..];
    if entries.len() != count * SUBSCRIPTION_BYTES {
        return Err(SubscriptionRefusal::Malformed(
            "a subscription whose count is not how many it lists",
        ));
    }
    Ok(entries
        .as_chunks::<SUBSCRIPTION_BYTES>()
        .0
        .iter()
        .map(|entry| {
            u16::try_from(u32::from_be_bytes([
                entry[0], entry[1], entry[2], entry[3],
            ]))
            .ok()
        }))
}

/// The processor ID a ground station joins a stand-in for the cFS end as:
/// one more than the larger of the vehicle's and the far end's.
///
/// Only a stand-in takes a ground station as a peer. SBN on a real cFS
/// passes nothing from one peer on to another -- each peer's pipe is made
/// with `CFE_SB_PIPEOPTS_IGNOREMINE`, so what SBN itself published is never
/// sent out again -- so a ground station joining a real cFS's SBN would be
/// heard by nothing. The ID is worked out rather than stated because the
/// configuration flies, and a station's ID is no fact of the vehicle's; it
/// is worked out here, once, because the station and the stand-in must both
/// arrive at it.
///
/// # Arguments
///
/// * `processor_id`      - `sbn.processor_id`, the vehicle's.
/// * `peer_processor_id` - `sbn.peer_processor_id`, the far end's.
///
/// # Returns
///
/// The ground station's processor ID, which is neither.
pub fn ground_processor_id(processor_id: u32, peer_processor_id: u32) -> u32 {
    processor_id.max(peer_processor_id).saturating_add(1)
}

/// What a peer is due to do, by the clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Due {
    /// Nothing yet.
    Nothing,
    /// Announce itself: it is not connected, and has not announced lately.
    Announce,
    /// Send a heartbeat: it has sent nothing for [`HEARTBEAT_AFTER`].
    Heartbeat,
    /// Disconnect: the far end has been silent for [`SILENCE`].
    Disconnect,
}

/// When a peer last sent and last heard, and whether it is connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Liveness {
    connected: bool,
    sent: Instant,
    heard: Instant,
    announced: Option<Instant>,
}

impl Liveness {
    /// A peer that has heard nothing and announced nothing.
    ///
    /// # Arguments
    ///
    /// * `now` - The time.
    ///
    /// # Returns
    ///
    /// The peer, not connected and due an announcement at once, its
    /// silence and the time since it last sent both counted from `now`.
    pub fn new(now: Instant) -> Liveness {
        Liveness {
            connected: false,
            sent: now,
            heard: now,
            announced: None,
        }
    }

    /// Whether the peer is connected.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Something has arrived from the far end, which connects a peer that
    /// was not.
    ///
    /// # Arguments
    ///
    /// * `now` - When it arrived.
    ///
    /// # Returns
    ///
    /// Whether this is what connected it.
    pub fn heard(&mut self, now: Instant) -> bool {
        self.heard = now;
        let was = self.connected;
        self.connected = true;
        !was
    }

    /// Something has been sent to the far end.
    ///
    /// The later of this and what was said before is kept, so that sends
    /// counted on more than one thread can each be told in any order.
    ///
    /// # Arguments
    ///
    /// * `now` - When it was sent.
    pub fn sent(&mut self, now: Instant) {
        self.sent = self.sent.max(now);
    }

    /// An announcement has been sent.
    ///
    /// # Arguments
    ///
    /// * `now` - When it was sent.
    pub fn announced(&mut self, now: Instant) {
        self.announced = Some(now);
        self.sent(now);
    }

    /// The peer is disconnected, and is announced to again at once.
    pub fn disconnect(&mut self) {
        self.connected = false;
        self.announced = None;
    }

    /// Says what is due now.
    ///
    /// # Arguments
    ///
    /// * `now` - The time.
    ///
    /// # Returns
    ///
    /// What is due; silence before a heartbeat, where both are.
    pub fn due(&self, now: Instant) -> Due {
        if !self.connected {
            return match self.announced {
                Some(at)
                    if now.saturating_duration_since(at) < ANNOUNCE_EVERY =>
                {
                    Due::Nothing
                }
                _ => Due::Announce,
            };
        }
        if now.saturating_duration_since(self.heard) >= SILENCE {
            Due::Disconnect
        } else if now.saturating_duration_since(self.sent) >= HEARTBEAT_AFTER {
            Due::Heartbeat
        } else {
            Due::Nothing
        }
    }

    /// Says how long until something is next due, for a wait to time out
    /// at.
    ///
    /// # Arguments
    ///
    /// * `now` - The time.
    ///
    /// # Returns
    ///
    /// How long, nought where something is due already.
    pub fn until_due(&self, now: Instant) -> Duration {
        if !self.connected {
            return match self.announced {
                Some(at) => {
                    (at + ANNOUNCE_EVERY).saturating_duration_since(now)
                }
                None => Duration::ZERO,
            };
        }
        let silence = (self.heard + SILENCE).saturating_duration_since(now);
        let heartbeat =
            (self.sent + HEARTBEAT_AFTER).saturating_duration_since(now);
        silence.min(heartbeat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "3b339a5e0f686cef822a5d6ea95004222ab7af33";

    /// A header written and read back is the header, with the message after
    /// it, laid out as SBN lays it.
    #[test]
    fn a_header_round_trips() {
        let header = Header {
            kind: kind::APPLICATION,
            processor_id: 2,
            spacecraft_id: 0x42,
        };
        let mut room = [0u8; 32];
        let size = header.write(&mut room, b"abc").expect("it fits");
        assert_eq!(
            &room[..size],
            &[0, 0, 0, 3, 3, 0, 0, 0, 2, 0, 0, 0, 0x42, b'a', b'b', b'c']
        );
        assert_eq!(Header::read(&room[..size]), Ok((header, &b"abc"[..])));
        assert!(Header::read(&room[..size - 1]).is_err());
        assert!(Header::read(&room[..12]).is_err());
    }

    /// The ident is git's `$Id$`, 48 bytes with its terminating nought, and
    /// is refused unless it is a hash.
    #[test]
    fn the_ident_is_gits_id_of_the_hash() {
        let ident = ident_of(HASH).expect("a hash");
        assert_eq!(&ident[..5], b"$Id: ");
        assert_eq!(&ident[5..45], HASH.as_bytes());
        assert_eq!(&ident[45..], b" $\0");
        let mut text = [0u8; 96];
        assert_eq!(ident_text(&ident, &mut text), format!("$Id: {HASH} $"));
        for wrong in ["", "v7.0.1", &HASH[1..], &format!("{}g", &HASH[1..])] {
            let why = ident_of(wrong).expect_err(wrong);
            assert!(why.contains("40 hexadecimal digits"), "{why}");
        }
    }

    /// A subscription written and read back lists its message IDs, and one
    /// with another ident is refused, saying which it carried.
    #[test]
    fn a_subscription_round_trips_and_its_ident_is_held_to() {
        let ident = ident_of(HASH).unwrap();
        let mut room = [0u8; 128];
        let size = write_subscriptions(
            &mut room,
            &ident,
            [0x4280u16, 0x4290].into_iter(),
        )
        .expect("it fits");
        assert_eq!(size, 48 + 2 + 12);
        assert_eq!(&room[48..50], &[0, 2]);
        assert_eq!(&room[50..56], &[0, 0, 0x42, 0x80, 0, 0]);
        let read: Vec<_> = read_subscriptions(&room[..size], &ident)
            .expect("taken")
            .collect();
        assert_eq!(read, vec![Some(0x4280), Some(0x4290)]);

        let other = ident_of(&"1".repeat(40)).unwrap();
        match read_subscriptions(&room[..size], &other) {
            Err(SubscriptionRefusal::Ident(carried)) => {
                assert_eq!(carried, &ident[..])
            }
            _ => panic!("another ident is refused"),
        }
        assert!(matches!(
            read_subscriptions(&room[..size - 1], &ident),
            Err(SubscriptionRefusal::Malformed(_))
        ));
    }

    /// A ground station's processor ID is neither of the two the
    /// configuration states.
    #[test]
    fn a_ground_station_is_a_third_processor() {
        assert_eq!(ground_processor_id(2, 1), 3);
        assert_eq!(ground_processor_id(1, 7), 8);
    }

    /// A release this build does not speak is refused naming the ones it
    /// does.
    #[test]
    fn a_release_is_one_this_build_speaks() {
        assert_eq!(release_of(" v7.0.1 ").unwrap().protocol_version, 11);
        let why = release_of("v6.4.0").unwrap_err();
        assert!(why.contains("v7.0.1") && why.contains("v6.4.0"), "{why}");
    }

    /// A peer announces until it hears, heartbeats after five seconds of
    /// sending nothing, and disconnects after ten of hearing nothing --
    /// asked with the time rather than waited for.
    #[test]
    fn a_peer_announces_heartbeats_and_disconnects_on_time() {
        let start = Instant::now();
        let at = |seconds: f64| start + Duration::from_secs_f64(seconds);
        let mut peer = Liveness::new(start);
        assert_eq!(peer.due(start), Due::Announce);
        peer.announced(start);
        assert_eq!(peer.due(at(0.5)), Due::Nothing);
        assert_eq!(peer.until_due(at(0.5)), Duration::from_millis(500));
        assert_eq!(peer.due(at(1.0)), Due::Announce);
        peer.announced(at(1.0));

        assert!(peer.heard(at(1.5)), "the first datagram connects it");
        assert!(!peer.heard(at(1.5)), "and the second does not again");
        peer.sent(at(1.5));
        assert_eq!(peer.due(at(6.4)), Due::Nothing);
        assert_eq!(peer.due(at(6.5)), Due::Heartbeat);
        peer.sent(at(6.5));
        assert_eq!(peer.until_due(at(6.5)), Duration::from_secs(5));
        peer.sent(at(11.0));
        assert_eq!(peer.due(at(11.4)), Due::Nothing);
        assert_eq!(peer.due(at(11.5)), Due::Disconnect);

        peer.disconnect();
        assert!(!peer.is_connected());
        assert_eq!(peer.due(at(11.5)), Due::Announce);
    }
}
