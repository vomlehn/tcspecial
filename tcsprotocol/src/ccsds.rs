//! The CCSDS Space Packet primary header, as a Yamcs ground station reads it.
//!
//! A vehicle whose configuration answers to Yamcs (`tcspecial.gateway:
//! yamcs`) carries every packet on every link to and from the ground as a
//! CCSDS Space Packet (CCSDS 133.0-B): a six-byte primary header, no
//! secondary header, and then exactly what the radio's link would carry in a
//! datagram. This is the one statement of that header, read by the vehicle
//! and by whatever takes the place of the ground.
//!
//! ```text
//! bits   3        1      1          11     2         14         16
//!        version  type   secondary  APID   sequence  sequence   data
//!        0        0 TM   header     0 to   flags     count      length
//!                 1 TC   flag 0     0x7FF  0b11                 (bytes after
//!                                                               the header,
//!                                                               less one)
//! ```
//!
//! Every number is most significant byte first. A packet with nothing after
//! its header cannot be stated -- the length field counts one less than what
//! follows -- and tcspecial never sends one, so writing one is refused.
//!
//! What arrives is checked against everything the header says but the
//! sequence count: the ground counts its own packets, and a count that
//! skipped is a packet lost on the link, which the packet after it is still
//! worth reading. [`crate::cfs`] writes a CCSDS primary header too, with the
//! secondary header and fields cFS adds; this has neither.
//!
//! Nothing here allocates: a header is written into room the caller left
//! before the data, and read in place.

use std::fmt;

/// How long the primary header is.
pub const HEADER_BYTES: usize = 6;

/// The highest APID: eleven bits.
pub const MAX_APID: u16 = 0x7FF;

/// The highest sequence count, after which the count goes back to nought:
/// fourteen bits.
pub const MAX_SEQUENCE: u16 = 0x3FFF;

/// The most data one packet can carry: the length field's sixteen bits
/// count one less than what follows the header.
pub const MAX_DATA_BYTES: usize = 0x1_0000;

/// The packet type bit, set in a telecommand's first word.
const TYPE_BIT: u16 = 0x1000;

/// The secondary header flag, set in a packet that has one.
const SECONDARY_BIT: u16 = 0x0800;

/// The sequence flags of an unsegmented packet, in the second word.
const UNSEGMENTED: u16 = 0xC000;

/// Which way a packet goes, which its type bit says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    /// Vehicle to ground: type 0.
    Telemetry,
    /// Ground to vehicle: type 1.
    Telecommand,
}

impl PacketType {
    /// Names the type, in words a complaint can use.
    ///
    /// # Returns
    ///
    /// "telemetry" or "a telecommand".
    fn said(self) -> &'static str {
        match self {
            PacketType::Telemetry => "telemetry",
            PacketType::Telecommand => "a telecommand",
        }
    }
}

/// What is wrong with a packet, written or read.
///
/// An enumeration rather than a message so that refusing a packet allocates
/// nothing: the command links refuse on the thread that must not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// An APID past eleven bits.
    Apid(u16),
    /// Nothing after the header, which the length field cannot say.
    Empty,
    /// More after the header than the length field can say.
    TooLong(usize),
    /// Less room than a header and one byte of data, for writing, or a
    /// packet shorter than that, read.
    Short(usize),
    /// A version number that is not 0.
    Version(u16),
    /// The other packet type.
    Type(PacketType),
    /// The secondary header flag set.
    Secondary,
    /// Another APID than the link's: the one it carries.
    WrongApid {
        /// What the link's packets carry.
        expected: u16,
        /// What this one carries.
        found: u16,
    },
    /// Sequence flags that say a segment of a packet.
    Segmented(u16),
    /// A length field that does not say the datagram's length.
    Length {
        /// What the length field says follows the header.
        said: usize,
        /// What did.
        found: usize,
    },
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Fault::Apid(apid) => write!(
                f,
                "APID {apid:#x} is past the eleven bits an APID has, at most \
                 {MAX_APID:#x}"
            ),
            Fault::Empty => f.write_str(
                "a packet with nothing after its header cannot be stated: \
                 its length field counts one less than what follows",
            ),
            Fault::TooLong(bytes) => write!(
                f,
                "{bytes} bytes after the header is more than a packet's \
                 length field can say, at most {MAX_DATA_BYTES}"
            ),
            Fault::Short(bytes) => write!(
                f,
                "{bytes} bytes is shorter than a CCSDS primary header and \
                 one byte of data"
            ),
            Fault::Version(version) => {
                write!(f, "CCSDS version number {version}, and not 0")
            }
            Fault::Type(found) => write!(f, "{} on this link", found.said()),
            Fault::Secondary => f.write_str(
                "a secondary header flag set, and these packets have none",
            ),
            Fault::WrongApid { expected, found } => write!(
                f,
                "APID {found:#x}, where this link's packets are \
                 {expected:#x}"
            ),
            Fault::Segmented(flags) => write!(
                f,
                "sequence flags {flags:#04b}, a segment of a packet rather \
                 than a whole one"
            ),
            Fault::Length { said, found } => write!(
                f,
                "a length field saying {said} bytes follow the header, \
                 where {found} do"
            ),
        }
    }
}

impl std::error::Error for Fault {}

/// Checks an APID is one: eleven bits.
///
/// # Arguments
///
/// * `apid` - The APID.
///
/// # Returns
///
/// The APID.
///
/// # Errors
///
/// [`Fault::Apid`] for one past [`MAX_APID`].
pub fn apid(apid: u16) -> Result<u16, Fault> {
    match apid <= MAX_APID {
        true => Ok(apid),
        false => Err(Fault::Apid(apid)),
    }
}

/// Writes a primary header into the first [`HEADER_BYTES`] of a packet whose
/// data already follows it.
///
/// # Arguments
///
/// * `packet`      - The whole packet: room for the header, then the data,
///                   and nothing after it.
/// * `packet_type` - Which way it goes.
/// * `apid`        - Its APID.
/// * `sequence`    - Its sequence count, of which the low fourteen bits are
///                   carried.
///
/// # Returns
///
/// Nothing, once the header is written.
///
/// # Errors
///
/// [`Fault::Apid`] for an APID past eleven bits, [`Fault::Empty`] for no
/// data after the header, [`Fault::Short`] for less room than the header,
/// and [`Fault::TooLong`] for more data than the length field can say.
pub fn write_header(
    packet: &mut [u8],
    packet_type: PacketType,
    apid: u16,
    sequence: u16,
) -> Result<(), Fault> {
    let apid = self::apid(apid)?;
    let data = match packet.len().checked_sub(HEADER_BYTES) {
        None => return Err(Fault::Short(packet.len())),
        Some(0) => return Err(Fault::Empty),
        Some(data) if data > MAX_DATA_BYTES => {
            return Err(Fault::TooLong(data))
        }
        Some(data) => data,
    };
    // Version 0, the type, no secondary header, and the APID.
    let kind = match packet_type {
        PacketType::Telemetry => 0,
        PacketType::Telecommand => TYPE_BIT,
    };
    let first = kind | apid;
    let second = UNSEGMENTED | (sequence & MAX_SEQUENCE);
    // Bounded by MAX_DATA_BYTES above, so it fits.
    let length = (data - 1) as u16;
    for (at, word) in [first, second, length].into_iter().enumerate() {
        packet[2 * at..2 * at + 2].copy_from_slice(&word.to_be_bytes());
    }
    Ok(())
}

/// Checks the primary header of a packet that arrived, and says where its
/// data is.
///
/// Everything the header says is checked but the sequence count: see the
/// module's note on why.
///
/// # Arguments
///
/// * `packet`      - The whole datagram, as it arrived.
/// * `packet_type` - Which way the link's packets go.
/// * `apid`        - The APID the link's packets carry.
///
/// # Returns
///
/// The data after the header, which is never empty.
///
/// # Errors
///
/// What is wrong with the header, as a [`Fault`].
pub fn read_header(
    packet: &[u8],
    packet_type: PacketType,
    apid: u16,
) -> Result<&[u8], Fault> {
    if packet.len() < HEADER_BYTES + 1 {
        return Err(Fault::Short(packet.len()));
    }
    let word =
        |at: usize| u16::from_be_bytes([packet[2 * at], packet[2 * at + 1]]);
    let first = word(0);
    let version = first >> 13;
    if version != 0 {
        return Err(Fault::Version(version));
    }
    let found = match first & TYPE_BIT != 0 {
        true => PacketType::Telecommand,
        false => PacketType::Telemetry,
    };
    if found != packet_type {
        return Err(Fault::Type(found));
    }
    if first & SECONDARY_BIT != 0 {
        return Err(Fault::Secondary);
    }
    let carried = first & MAX_APID;
    if carried != apid {
        return Err(Fault::WrongApid {
            expected: apid,
            found: carried,
        });
    }
    let flags = word(1) >> 14;
    if flags != UNSEGMENTED >> 14 {
        return Err(Fault::Segmented(flags));
    }
    let said = usize::from(word(2)) + 1;
    let found = packet.len() - HEADER_BYTES;
    if said != found {
        return Err(Fault::Length { said, found });
    }
    Ok(&packet[HEADER_BYTES..])
}

/// A sequence count: what one sender has sent under one APID, counting from
/// nought and going back to nought after [`MAX_SEQUENCE`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SequenceCount {
    /// The count the next packet carries.
    next: u16,
}

impl SequenceCount {
    /// A count from nought.
    ///
    /// # Returns
    ///
    /// The count, whose first packet carries nought.
    pub fn new() -> SequenceCount {
        SequenceCount { next: 0 }
    }

    /// Takes the count the next packet carries, and moves on to the one
    /// after it.
    ///
    /// # Returns
    ///
    /// The count.
    pub fn take(&mut self) -> u16 {
        let now = self.next;
        self.next = match now {
            MAX_SEQUENCE => 0,
            _ => now + 1,
        };
        now
    }
}

/// One sender's packets under one APID: the type, the APID and the count.
///
/// Each link that sends keeps one of its own, so its count is its own: the
/// beacon's, each command link's answers and each source's downlink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sender {
    /// Which way its packets go.
    packet_type: PacketType,
    /// The APID they carry.
    apid: u16,
    /// What the next one counts.
    count: SequenceCount,
}

impl Sender {
    /// A sender of packets of one type under one APID, counting from
    /// nought.
    ///
    /// # Arguments
    ///
    /// * `packet_type` - Which way its packets go.
    /// * `apid`        - The APID they carry.
    ///
    /// # Returns
    ///
    /// The sender.
    ///
    /// # Errors
    ///
    /// [`Fault::Apid`] for an APID past eleven bits.
    pub fn new(packet_type: PacketType, apid: u16) -> Result<Sender, Fault> {
        Ok(Sender {
            packet_type,
            apid: self::apid(apid)?,
            count: SequenceCount::new(),
        })
    }

    /// The APID its packets carry.
    pub fn apid(&self) -> u16 {
        self.apid
    }

    /// Writes the next header into the first [`HEADER_BYTES`] of a packet
    /// whose data already follows it, counting it.
    ///
    /// A packet refused is not counted, so the count says what was sent.
    ///
    /// # Arguments
    ///
    /// * `packet` - The whole packet: room for the header, then the data.
    ///
    /// # Returns
    ///
    /// Nothing, once the header is written.
    ///
    /// # Errors
    ///
    /// What [`write_header`] refuses.
    pub fn stamp(&mut self, packet: &mut [u8]) -> Result<(), Fault> {
        let mut after = self.count;
        write_header(packet, self.packet_type, self.apid, after.take())?;
        self.count = after;
        Ok(())
    }

    /// Puts `data` after a header in `room`, counting it.
    ///
    /// For a sender whose data is somewhere else: copied once, into room the
    /// caller made before it was needed, so that framing allocates nothing.
    ///
    /// # Arguments
    ///
    /// * `room` - Where the packet is written, at least [`HEADER_BYTES`]
    ///            longer than `data`.
    /// * `data` - What the packet carries.
    ///
    /// # Returns
    ///
    /// The packet, the front of `room`.
    ///
    /// # Errors
    ///
    /// [`Fault::Short`] where `room` is too small, and what [`Sender::stamp`]
    /// refuses.
    pub fn frame<'a>(
        &mut self,
        room: &'a mut [u8],
        data: &[u8],
    ) -> Result<&'a [u8], Fault> {
        let total = HEADER_BYTES + data.len();
        if data.is_empty() {
            return Err(Fault::Empty);
        }
        if room.len() < total {
            return Err(Fault::Short(room.len()));
        }
        room[HEADER_BYTES..total].copy_from_slice(data);
        self.stamp(&mut room[..total])?;
        Ok(&room[..total])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Telemetry under APID 0x101, the first of its count, carrying three
    /// bytes, worked by hand: 0x0101 (version 0, type 0, no secondary
    /// header, APID 0x101), 0xC000 (unsegmented, count 0), 0x0002 (three
    /// bytes, less one).
    #[test]
    fn telemetry_is_written_as_worked_by_hand() {
        let mut packet = [0u8; 9];
        packet[6..].copy_from_slice(&[0xAA, 0xBB, 0xCC]);
        write_header(&mut packet, PacketType::Telemetry, 0x101, 0)
            .expect("a header");
        assert_eq!(
            packet,
            [0x01, 0x01, 0xC0, 0x00, 0x00, 0x02, 0xAA, 0xBB, 0xCC]
        );
    }

    /// A telecommand under APID 0x7FF, count 0x3FFF, carrying one byte:
    /// 0x17FF (type 1 and every APID bit), 0xFFFF (unsegmented, every
    /// count bit), 0x0000 (one byte, less one).
    #[test]
    fn a_telecommand_is_written_as_worked_by_hand() {
        let mut packet = [0u8; 7];
        packet[6] = 0x42;
        write_header(&mut packet, PacketType::Telecommand, 0x7FF, 0x3FFF)
            .expect("a header");
        assert_eq!(packet, [0x17, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x42]);
    }

    /// What is written reads back, the data where it was put.
    #[test]
    fn a_header_reads_back() {
        let mut packet = [0u8; 10];
        packet[6..].copy_from_slice(b"ping");
        write_header(&mut packet, PacketType::Telecommand, 0x123, 77)
            .expect("a header");
        let data = read_header(&packet, PacketType::Telecommand, 0x123)
            .expect("a packet");
        assert_eq!(data, b"ping");
    }

    /// What cannot be stated is refused as it is written.
    #[test]
    fn what_cannot_be_stated_is_not_written() {
        let mut empty = [0u8; HEADER_BYTES];
        assert_eq!(
            write_header(&mut empty, PacketType::Telemetry, 1, 0),
            Err(Fault::Empty)
        );
        let mut short = [0u8; 3];
        assert_eq!(
            write_header(&mut short, PacketType::Telemetry, 1, 0),
            Err(Fault::Short(3))
        );
        let mut packet = [0u8; 8];
        assert_eq!(
            write_header(&mut packet, PacketType::Telemetry, 0x800, 0),
            Err(Fault::Apid(0x800))
        );
        let mut long = vec![0u8; HEADER_BYTES + MAX_DATA_BYTES + 1];
        assert_eq!(
            write_header(&mut long, PacketType::Telemetry, 1, 0),
            Err(Fault::TooLong(MAX_DATA_BYTES + 1))
        );
        let mut longest = vec![0u8; HEADER_BYTES + MAX_DATA_BYTES];
        write_header(&mut longest, PacketType::Telemetry, 1, 0)
            .expect("the longest a packet can be");
        assert_eq!(&longest[4..6], &[0xFF, 0xFF]);
    }

    /// Each thing a header can have wrong is refused, by what it is.
    #[test]
    fn every_fault_in_a_header_is_refused() {
        let good = || {
            let mut packet = [0u8; 8];
            write_header(&mut packet, PacketType::Telecommand, 0x10, 5)
                .expect("a header");
            packet
        };
        let read = |packet: &[u8]| {
            read_header(packet, PacketType::Telecommand, 0x10).map(|_| ())
        };
        assert_eq!(read(&good()), Ok(()));
        assert_eq!(read(&good()[..6]), Err(Fault::Short(6)));

        let mut version = good();
        version[0] |= 0x20;
        assert_eq!(read(&version), Err(Fault::Version(1)));

        let mut telemetry = good();
        telemetry[0] &= !0x10;
        assert_eq!(read(&telemetry), Err(Fault::Type(PacketType::Telemetry)));

        let mut secondary = good();
        secondary[0] |= 0x08;
        assert_eq!(read(&secondary), Err(Fault::Secondary));

        let mut apid = good();
        apid[1] = 0x11;
        assert_eq!(
            read(&apid),
            Err(Fault::WrongApid {
                expected: 0x10,
                found: 0x11
            })
        );

        let mut segment = good();
        segment[2] &= 0x7F;
        assert_eq!(read(&segment), Err(Fault::Segmented(0b01)));

        let mut length = good();
        length[5] = 2;
        assert_eq!(read(&length), Err(Fault::Length { said: 3, found: 2 }));

        // The sequence count is not looked at.
        let mut count = good();
        count[2] = 0xC0 | 0x3F;
        count[3] = 0xFF;
        assert_eq!(read(&count), Ok(()));
    }

    /// A count goes from nought to 0x3FFF and back to nought.
    #[test]
    fn a_count_wraps_at_fourteen_bits() {
        let mut count = SequenceCount::new();
        assert_eq!(count.take(), 0);
        assert_eq!(count.take(), 1);
        let mut count = SequenceCount { next: MAX_SEQUENCE };
        assert_eq!(count.take(), MAX_SEQUENCE);
        assert_eq!(count.take(), 0);
    }

    /// A sender counts each packet it frames, and not one it refuses.
    #[test]
    fn a_sender_counts_what_it_sends() {
        let mut sender =
            Sender::new(PacketType::Telemetry, 0x101).expect("an APID");
        let mut room = [0u8; 16];
        let packet = sender.frame(&mut room, &[1, 2, 3]).expect("framed");
        assert_eq!(packet, [0x01, 0x01, 0xC0, 0x00, 0x00, 0x02, 1, 2, 3]);
        assert_eq!(sender.frame(&mut room, &[]), Err(Fault::Empty));
        assert_eq!(
            sender.frame(&mut room, &[0; 11]),
            Err(Fault::Short(16)),
            "no room"
        );
        let packet = sender.frame(&mut room, &[9]).expect("framed");
        assert_eq!(packet, [0x01, 0x01, 0xC0, 0x01, 0x00, 0x00, 9]);
        assert_eq!(
            Sender::new(PacketType::Telemetry, 0x800),
            Err(Fault::Apid(0x800))
        );
    }
}
