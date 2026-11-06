//! What crosses a cFS software bus: a cFS packet around a tcspecial packet.
//!
//! A vehicle that answers to cFS carries every command, answer, beacon and
//! datum as a cFS packet: the CCSDS primary header, cFS's extended header, a
//! secondary header, and then exactly what the UDP link would carry in a
//! datagram. This is the one statement of that framing, read by the vehicle and
//! by whatever simulates the cFS end. See "What crosses the bus" in
//! `docs/design.rst`.
//!
//! The layouts are cFE's, as its `default_cfe_msg_hdr_priext.h` and
//! `default_cfe_msg_sechdr.h` have them in a build with
//! `MISSION_INCLUDE_CCSDSEXT_HEADER`, and its message IDs are its version 2
//! mapping (`cfe_msg_msgid_v2.c`). Every number is most significant byte
//! first.
//!
//! ```text
//! command    primary 6 | extended 4 | function code 1, checksum 1 |
//!            spare 4 | the tcspecial command
//! telemetry  primary 6 | extended 4 | time 6 (seconds 4, 1/65536ths 2) |
//!            the answer, the beacon or the datum
//! ```
//!
//! The four spare bytes of a command are cFE's: its command header pads
//! itself to sixteen bytes so that what follows is aligned for any type, and
//! a cFS application reads its command's data from there. A packet without
//! them would be one every cFS application on the bus read four bytes out.
//!
//! Nothing here allocates: a packet is written into a buffer the caller made
//! at startup, and read in place.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The primary and extended headers together.
const BASE_BYTES: usize = 10;

/// What comes before a command's data: the base headers, the function code
/// and checksum, and cFE's four spare bytes.
pub const COMMAND_HEADER_BYTES: usize = BASE_BYTES + 2 + 4;

/// What comes before a telemetry packet's data: the base headers and the
/// six bytes of time.
pub const TELEMETRY_HEADER_BYTES: usize = BASE_BYTES + 6;

/// The most either header is, which is what a datum's room on the bus is
/// counted with.
pub const HEADER_BYTES: usize = if COMMAND_HEADER_BYTES > TELEMETRY_HEADER_BYTES
{
    COMMAND_HEADER_BYTES
} else {
    TELEMETRY_HEADER_BYTES
};

/// The bit of a message ID that says it is a command's, in the version 2
/// mapping.
pub const COMMAND_BIT: u16 = 0x0080;

/// The bits of a message ID taken from the primary header's APID.
const APID_BITS: u16 = 0x007F;

/// The EDS version cFE writes into every extended header unless its platform
/// configuration says otherwise: `CFE_PLATFORM_EDSVER`, 1.
const EDS_VERSION: u16 = 1;

/// Whether a message ID is one the version 2 mapping reads as a command's.
///
/// # Arguments
///
/// * `msgid` - The message ID.
///
/// # Returns
///
/// `true` where its command bit is set.
pub fn is_command(msgid: u16) -> bool {
    msgid & COMMAND_BIT != 0
}

/// Writes the primary and extended headers.
///
/// # Arguments
///
/// * `into`          - At least [`BASE_BYTES`] of room.
/// * `msgid`         - The message ID, whose command bit says the type.
/// * `sequence`      - The packet's sequence count, of which the low 14
///                     bits are carried.
/// * `total`         - The length of the whole packet, headers included.
/// * `spacecraft_id` - What the extended header's system ID says, cut to
///                     the 16 bits it has, as cFE cuts it.
fn write_base(
    into: &mut [u8],
    msgid: u16,
    sequence: u16,
    total: usize,
    spacecraft_id: u32,
) {
    // CCSDS version 0, the type from the message ID, a secondary header,
    // and the APID's low seven bits from the message ID; the rest of the
    // APID is zero, as the design has it.
    let command = if is_command(msgid) { 0x1000 } else { 0 };
    let stream = command | 0x0800 | (msgid & APID_BITS);
    // Unsegmented: every packet here is whole.
    let sequence = 0xC000 | (sequence & 0x3FFF);
    // CCSDS counts the length less seven.
    let length = (total - 7) as u16;
    // The EDS version, big-endian, not a playback, and the subsystem from
    // the message ID's high byte.
    let subsystem = (EDS_VERSION << 11) | (msgid >> 8);
    let system = spacecraft_id as u16;
    for (at, word) in [stream, sequence, length, subsystem, system]
        .into_iter()
        .enumerate()
    {
        into[2 * at..2 * at + 2].copy_from_slice(&word.to_be_bytes());
    }
}

/// What a packet's base headers say, read and checked.
struct Base {
    msgid: u16,
}

/// Reads the base headers, refusing what is not one whole cFS packet with a
/// secondary header.
///
/// # Arguments
///
/// * `packet`  - The whole packet, as SBN carried it.
/// * `command` - Whether it must be a command (`true`) or telemetry.
/// * `header`  - How long this type's headers are.
///
/// # Returns
///
/// The message ID.
///
/// # Errors
///
/// What is wrong, in words a log line can use.
fn read_base(
    packet: &[u8],
    command: bool,
    header: usize,
) -> Result<Base, &'static str> {
    if packet.len() < header {
        return Err("shorter than a cFS packet's headers");
    }
    let word =
        |at: usize| u16::from_be_bytes([packet[2 * at], packet[2 * at + 1]]);
    let stream = word(0);
    if stream >> 13 != 0 {
        return Err("not CCSDS version 1 (version number 0)");
    }
    if stream & 0x0800 == 0 {
        return Err("without the secondary header every cFS packet has");
    }
    if (stream & 0x1000 != 0) != command {
        return Err(if command {
            "telemetry where a command was expected"
        } else {
            "a command where telemetry was expected"
        });
    }
    if word(1) & 0xC000 != 0xC000 {
        return Err("a segment of a packet rather than a whole one");
    }
    if usize::from(word(2)) + 7 != packet.len() {
        return Err("of a length its primary header does not say");
    }
    let mut msgid = (stream & APID_BITS) | ((word(3) & 0x00FF) << 8);
    if command {
        msgid |= COMMAND_BIT;
    }
    Ok(Base { msgid })
}

/// cFE's checksum of a whole command: 0xFF, exclusive-ored with every byte.
/// A command whose checksum byte is right sums to nought.
fn checksum(packet: &[u8]) -> u8 {
    packet.iter().fold(0xFF, |sum, byte| sum ^ byte)
}

/// A command read off the bus: its message ID, its function code and its
/// data, which is a tcspecial command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandFrame<'a> {
    /// The message ID it was published under.
    pub msgid: u16,
    /// Its function code, seven bits.
    pub function_code: u8,
    /// What follows the headers.
    pub data: &'a [u8],
}

/// Reads a command off the bus, checking its checksum as cFS's own
/// applications check theirs.
///
/// # Arguments
///
/// * `packet` - The whole packet.
///
/// # Returns
///
/// The command.
///
/// # Errors
///
/// What is wrong with it: not one whole command packet, or a checksum that
/// does not sum to nought.
pub fn read_command(packet: &[u8]) -> Result<CommandFrame<'_>, &'static str> {
    let base = read_base(packet, true, COMMAND_HEADER_BYTES)?;
    if checksum(packet) != 0 {
        return Err("a command whose checksum is wrong");
    }
    Ok(CommandFrame {
        msgid: base.msgid,
        // The top bit is cFE's, reserved.
        function_code: packet[BASE_BYTES] & 0x7F,
        data: &packet[COMMAND_HEADER_BYTES..],
    })
}

/// Writes a command, its checksum made last over the rest.
///
/// What the vehicle never sends, and what simulates the cFS end does.
///
/// # Arguments
///
/// * `into`          - Room for the headers and `data`.
/// * `msgid`         - Its message ID, which must be a command's.
/// * `sequence`      - Its sequence count.
/// * `spacecraft_id` - The spacecraft it is on.
/// * `function_code` - Its function code, below 128.
/// * `data`          - What follows the headers.
///
/// # Returns
///
/// How many bytes of `into` the packet is, or `None` where it does not fit.
pub fn write_command(
    into: &mut [u8],
    msgid: u16,
    sequence: u16,
    spacecraft_id: u32,
    function_code: u8,
    data: &[u8],
) -> Option<usize> {
    let total = COMMAND_HEADER_BYTES + data.len();
    if total > into.len() || total - 7 > usize::from(u16::MAX) {
        return None;
    }
    write_base(into, msgid | COMMAND_BIT, sequence, total, spacecraft_id);
    into[BASE_BYTES] = function_code & 0x7F;
    into[BASE_BYTES + 1] = 0;
    into[BASE_BYTES + 2..COMMAND_HEADER_BYTES].fill(0);
    into[COMMAND_HEADER_BYTES..total].copy_from_slice(data);
    into[BASE_BYTES + 1] = checksum(&into[..total]);
    Some(total)
}

/// A telemetry packet read off the bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetryFrame<'a> {
    /// The message ID it was published under.
    pub msgid: u16,
    /// Its time, as cFE's six bytes.
    pub time: [u8; 6],
    /// What follows the headers.
    pub data: &'a [u8],
}

/// Reads a telemetry packet.
///
/// What the vehicle never takes, and what simulates the cFS end does.
///
/// # Arguments
///
/// * `packet` - The whole packet.
///
/// # Returns
///
/// The packet.
///
/// # Errors
///
/// What is wrong with it.
pub fn read_telemetry(
    packet: &[u8],
) -> Result<TelemetryFrame<'_>, &'static str> {
    let base = read_base(packet, false, TELEMETRY_HEADER_BYTES)?;
    let mut time = [0u8; 6];
    time.copy_from_slice(&packet[BASE_BYTES..TELEMETRY_HEADER_BYTES]);
    Ok(TelemetryFrame {
        msgid: base.msgid,
        time,
        data: &packet[TELEMETRY_HEADER_BYTES..],
    })
}

/// Writes a telemetry packet.
///
/// # Arguments
///
/// * `into`          - Room for the headers and `data`.
/// * `msgid`         - Its message ID, which must be telemetry's.
/// * `sequence`      - Its sequence count.
/// * `spacecraft_id` - The spacecraft it is from.
/// * `time`          - When what it carries was taken, as cFE's six bytes.
/// * `data`          - What follows the headers.
///
/// # Returns
///
/// How many bytes of `into` the packet is, or `None` where it does not fit.
pub fn write_telemetry(
    into: &mut [u8],
    msgid: u16,
    sequence: u16,
    spacecraft_id: u32,
    time: [u8; 6],
    data: &[u8],
) -> Option<usize> {
    let total = TELEMETRY_HEADER_BYTES + data.len();
    if total > into.len() || total - 7 > usize::from(u16::MAX) {
        return None;
    }
    write_base(into, msgid & !COMMAND_BIT, sequence, total, spacecraft_id);
    into[BASE_BYTES..TELEMETRY_HEADER_BYTES].copy_from_slice(&time);
    into[TELEMETRY_HEADER_BYTES..total].copy_from_slice(data);
    Some(total)
}

/// The time scale a cFS build counts in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    /// International Atomic Time, which is UTC plus the leap seconds.
    Tai {
        /// TAI less UTC, in seconds.
        leap_seconds: u32,
    },
    /// UTC.
    Utc,
}

/// How cFE's time is counted, as the configuration's `cfe_time` states it:
/// from the epoch and on the scale the cFS build was compiled with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CfeTime {
    /// The epoch, in seconds since the UNIX epoch, both being UTC.
    pub epoch: i64,
    /// The scale.
    pub scale: Scale,
}

impl CfeTime {
    /// A clock reading as cFE's six bytes: seconds since the epoch as four,
    /// and 1/65536ths of a second as two.
    ///
    /// A reading before the epoch is nought, and one past what four bytes of
    /// seconds count is all ones: neither is a time a vehicle flies at, and
    /// both say plainly that the clock is wrong rather than wrapping round
    /// to a time that looks right.
    ///
    /// # Arguments
    ///
    /// * `at` - The reading.
    ///
    /// # Returns
    ///
    /// The six bytes.
    pub fn stamp(&self, at: SystemTime) -> [u8; 6] {
        let since_unix = match at.duration_since(UNIX_EPOCH) {
            Ok(since) => since,
            Err(_) => return [0; 6],
        };
        let leap = match self.scale {
            Scale::Tai { leap_seconds } => i64::from(leap_seconds),
            Scale::Utc => 0,
        };
        let seconds = since_unix.as_secs() as i64 - self.epoch + leap;
        let seconds = match u32::try_from(seconds) {
            Ok(seconds) => seconds,
            Err(_) if seconds < 0 => return [0; 6],
            Err(_) => return [0xFF; 6],
        };
        let sub = (u64::from(since_unix.subsec_nanos()) << 16) / 1_000_000_000;
        let mut out = [0u8; 6];
        out[..4].copy_from_slice(&seconds.to_be_bytes());
        out[4..].copy_from_slice(&(sub as u16).to_be_bytes());
        out
    }

    /// Six bytes of cFE time back as a clock reading, for whatever reads
    /// what the vehicle sent.
    ///
    /// # Arguments
    ///
    /// * `time` - The six bytes.
    ///
    /// # Returns
    ///
    /// The reading, or `None` where it would be before the UNIX epoch.
    pub fn reading(&self, time: [u8; 6]) -> Option<SystemTime> {
        let seconds = u32::from_be_bytes([time[0], time[1], time[2], time[3]]);
        let sub = u16::from_be_bytes([time[4], time[5]]);
        let leap = match self.scale {
            Scale::Tai { leap_seconds } => i64::from(leap_seconds),
            Scale::Utc => 0,
        };
        let unix = i64::from(seconds) + self.epoch - leap;
        let unix = u64::try_from(unix).ok()?;
        let nanos = (u64::from(sub) * 1_000_000_000) >> 16;
        Some(UNIX_EPOCH + Duration::new(unix, nanos as u32))
    }
}

/// Reads a UTC date and time written `YYYY-MM-DDTHH:MM:SS`, as `cfe_time`'s
/// epoch is.
///
/// # Arguments
///
/// * `said` - The date and time.
///
/// # Returns
///
/// Seconds since the UNIX epoch, negative before it.
///
/// # Errors
///
/// A message saying how to write one, for anything else.
pub fn epoch_of(said: &str) -> Result<i64, String> {
    let wrong = || {
        format!(
            "\"{said}\" is not a date and time: write the UTC epoch the cFS \
             build was compiled with as YYYY-MM-DDTHH:MM:SS, as \
             1980-01-01T00:00:00"
        )
    };
    let b = said.trim().as_bytes();
    if b.len() != 19
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return Err(wrong());
    }
    let field = |from: usize, to: usize| -> Result<i64, String> {
        let text = std::str::from_utf8(&b[from..to]).map_err(|_| wrong())?;
        if !text.bytes().all(|c| c.is_ascii_digit()) {
            return Err(wrong());
        }
        text.parse::<i64>().map_err(|_| wrong())
    };
    let (year, month, day) = (field(0, 4)?, field(5, 7)?, field(8, 10)?);
    let (hour, minute, second) =
        (field(11, 13)?, field(14, 16)?, field(17, 19)?);
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days_in = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return Err(wrong()),
    };
    if day < 1 || day > days_in || hour > 23 || minute > 59 || second > 59 {
        return Err(wrong());
    }
    Ok(days_from_civil(year, month, day) * 86_400
        + hour * 3600
        + minute * 60
        + second)
}

/// Days since 1970-01-01 of a date in the proleptic Gregorian calendar:
/// Howard Hinnant's `days_from_civil`.
///
/// # Arguments
///
/// * `year`  - The year, as written: 2026 is 2026.
/// * `month` - The month, 1 for January to 12 for December.
/// * `day`   - The day of the month, from 1.
///
/// # Returns
///
/// How many days the date is after 1970-01-01, negative for one before it.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A command written and read back is the command, its checksum sums to
    /// nought, and its bytes are where cFE puts them.
    #[test]
    fn a_command_round_trips_and_is_laid_out_as_cfe_lays_it() {
        let mut room = [0u8; 64];
        let size = write_command(&mut room, 0x4280, 5, 0x42, 0x04, b"abc")
            .expect("it fits");
        assert_eq!(size, COMMAND_HEADER_BYTES + 3);
        let packet = &room[..size];
        // Version 0, a command, a secondary header, APID 0x00.
        assert_eq!(&packet[..2], &[0x18, 0x00]);
        // Unsegmented, sequence 5.
        assert_eq!(&packet[2..4], &[0xC0, 0x05]);
        // Length less seven.
        assert_eq!(&packet[4..6], &((size - 7) as u16).to_be_bytes());
        // EDS version 1, subsystem 0x42; system ID the spacecraft.
        assert_eq!(&packet[6..10], &[0x08, 0x42, 0x00, 0x42]);
        assert_eq!(packet[10], 0x04);
        assert_eq!(&packet[12..16], &[0, 0, 0, 0]);
        assert_eq!(checksum(packet), 0);
        let read = read_command(packet).expect("a command");
        assert_eq!(read.msgid, 0x4280);
        assert_eq!(read.function_code, 0x04);
        assert_eq!(read.data, b"abc");
    }

    /// The version 2 mapping: the low seven bits are the APID's, the
    /// command bit the type, and the high byte the subsystem.
    #[test]
    fn a_message_id_is_read_by_the_version_2_mapping() {
        let mut room = [0u8; 64];
        let size =
            write_telemetry(&mut room, 0x4223, 0, 1, [0; 6], b"x").unwrap();
        assert_eq!(&room[..2], &[0x08, 0x23]);
        assert_eq!(room[7], 0x42);
        let read = read_telemetry(&room[..size]).expect("telemetry");
        assert_eq!(read.msgid, 0x4223);
        assert_eq!(read.data, b"x");
        assert!(is_command(0x4280));
        assert!(!is_command(0x427F));
    }

    /// What is not one whole command is refused, saying what it is.
    #[test]
    fn what_is_not_a_whole_command_is_refused() {
        let mut room = [0u8; 64];
        let size = write_command(&mut room, 0x4281, 0, 1, 1, b"on").unwrap();
        let good = room[..size].to_vec();

        let mut bad_sum = good.clone();
        bad_sum[size - 1] ^= 0x01;
        assert!(read_command(&bad_sum).unwrap_err().contains("checksum"));

        assert!(read_command(&good[..size - 1])
            .unwrap_err()
            .contains("length"));
        assert!(read_command(&good[..8]).unwrap_err().contains("shorter"));

        let mut segment = good.clone();
        segment[2] &= 0x3F;
        assert!(read_command(&segment).unwrap_err().contains("segment"));

        let mut primary_only = good.clone();
        primary_only[0] &= !0x08;
        assert!(read_command(&primary_only)
            .unwrap_err()
            .contains("secondary"));

        let size =
            write_telemetry(&mut room, 0x4201, 0, 1, [0; 6], b"x").unwrap();
        assert!(read_command(&room[..size])
            .unwrap_err()
            .contains("telemetry where"));
    }

    /// A clock reading is seconds and 1/65536ths since the epoch, plus the
    /// leap seconds on TAI, and reads back.
    #[test]
    fn a_time_is_counted_from_the_epoch_on_the_scale() {
        let epoch = epoch_of("1980-01-01T00:00:00").expect("an epoch");
        assert_eq!(epoch, 315_532_800);
        let tai = CfeTime {
            epoch,
            scale: Scale::Tai { leap_seconds: 37 },
        };
        let at = UNIX_EPOCH + Duration::new(315_532_800 + 100, 500_000_000);
        let stamped = tai.stamp(at);
        assert_eq!(&stamped[..4], &137u32.to_be_bytes());
        assert_eq!(&stamped[4..], &0x8000u16.to_be_bytes());
        assert_eq!(tai.reading(stamped), Some(at));

        let utc = CfeTime {
            epoch,
            scale: Scale::Utc,
        };
        assert_eq!(&utc.stamp(at)[..4], &100u32.to_be_bytes());
        assert_eq!(utc.stamp(UNIX_EPOCH), [0; 6]);
    }

    /// An epoch that is not a date and time is refused saying how to write
    /// one.
    #[test]
    fn an_epoch_is_a_date_and_a_time() {
        assert_eq!(epoch_of("1970-01-01T00:00:00"), Ok(0));
        assert_eq!(epoch_of("2000-03-01T00:00:01"), Ok(951_868_801));
        for wrong in [
            "1980-01-01",
            "1980-02-30T00:00:00",
            "1980-13-01T00:00:00",
            "1980-01-01 00:00:00",
            "1980-01-01T24:00:00",
        ] {
            let why = epoch_of(wrong).expect_err(wrong);
            assert!(why.contains("YYYY-MM-DDTHH:MM:SS"), "{why}");
        }
    }
}
