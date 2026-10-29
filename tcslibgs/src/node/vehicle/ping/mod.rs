//! What a ping and its answer say, which the ground and the vehicle share.
//!
//! `type: ping` is a vehicle node that answers the ground: each message the
//! MOC sends it comes back with the time the vehicle had it written on the
//! end. A ping is
//!
//! ```text
//! ping #<count> <sent>
//! ```
//!
//! and its answer is the ping with one space and the vehicle's time after it:
//!
//! ```text
//! ping #<count> <sent> <answered>
//! ```
//!
//! The count is a decimal number from one, which the MOC adds one to for each
//! ping it sends. Each time is the number of seconds since the UNIX epoch,
//! written with exactly nine digits after the point, so that it says the time
//! to the nanosecond: `1760900000.000000001`. So the ground can tell from a
//! count which ping an answer is for, and from the three numbers how long the
//! ping took to arrive and to come back -- the two clocks' offset included, a
//! ping being the one exchange that can show it.
//!
//! Here rather than in tcspecial for the reason every rule in this crate is:
//! the ground station writes these and reads the answers, the vehicle reads
//! these and writes the answers, and a format stated twice is two formats on
//! the day one of them changes. `tcspecial::node::vehicle::ping` is the node.
//!
//! # A fixed size
//!
//! Every number has a largest value -- a count and a whole number of seconds
//! are each at most `u64::MAX`, which is twenty digits -- so a ping has a
//! largest size, and so does its answer: [`MESSAGE_BYTES`] and
//! [`REPLY_BYTES`]. A ping is read into a buffer of the second size, which
//! holds the largest ping there is, the space and the largest time the
//! vehicle could add, so an answer is never cut short and nothing is
//! allocated per ping. A message longer than the largest ping is not a ping,
//! and it is refused as one rather than read as far as it fits.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// What every ping begins with.
pub const PREFIX: &str = "ping #";

/// The most digits a count can have: those of `u64::MAX`.
const COUNT_DIGITS: usize = 20;

/// The most bytes a time can take: twenty digits of seconds, the point, and
/// nine digits of nanoseconds.
pub const TIME_BYTES: usize = 20 + 1 + 9;

/// The largest ping there is: the prefix, the largest count, a space and the
/// largest time.
pub const MESSAGE_BYTES: usize = PREFIX.len() + COUNT_DIGITS + 1 + TIME_BYTES;

/// The largest answer there is: the largest ping, the space, and the largest
/// time the vehicle could add to it.
pub const REPLY_BYTES: usize = MESSAGE_BYTES + 1 + TIME_BYTES;

/// A ping, as the ground sent it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ping {
    /// Which ping this is, counting from one.
    pub count: u64,
    /// When the ground sent it, since the UNIX epoch.
    pub sent: Duration,
}

/// An answer to a ping, as the vehicle sent it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pong {
    /// The ping it answers.
    pub ping: Ping,
    /// When the vehicle had the ping, since the UNIX epoch.
    pub answered: Duration,
}

/// An answer, in a buffer that holds the largest there is.
///
/// A fixed array rather than a `Vec`: its size is the format's own bound, so
/// an answer that would not fit cannot be made, and a vehicle answering pings
/// allocates nothing to do it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Reply {
    bytes: [u8; REPLY_BYTES],
    len: usize,
}

impl Reply {
    /// The answer's bytes, as they are sent.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

impl std::fmt::Debug for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Reply({:?})", String::from_utf8_lossy(self.as_bytes()))
    }
}

/// The time now, since the UNIX epoch.
///
/// An error only for a clock set before 1970, which is a clock that is wrong
/// and that a ping exists to show.
pub fn now() -> Result<Duration, String> {
    SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
        "the clock says a time before the UNIX epoch, so no time since it can \
         be written: set the clock"
            .to_string()
    })
}

/// Writes a time as a ping writes it. The one way a ping's time is written,
/// and the way the vehicle writes it, allocating nothing: see [`reply`].
///
/// # Arguments
///
/// * `since_epoch` - The time, since the UNIX epoch.
/// * `out`         - Where it is written, from the start.
///
/// # Returns
///
/// How many bytes it took, or `None` where it does not fit.
fn write_time(since_epoch: Duration, out: &mut [u8]) -> Option<usize> {
    use std::fmt::Write;

    /// A slice written into, as far as it has room.
    struct Into<'a> {
        out: &'a mut [u8],
        at: usize,
    }

    impl Write for Into<'_> {
        fn write_str(&mut self, said: &str) -> std::fmt::Result {
            let end = self.at + said.len();
            let room = self.out.get_mut(self.at..end).ok_or(std::fmt::Error)?;
            room.copy_from_slice(said.as_bytes());
            self.at = end;
            Ok(())
        }
    }

    let mut into = Into { out, at: 0 };
    write!(
        into,
        "{}.{:09}",
        since_epoch.as_secs(),
        since_epoch.subsec_nanos()
    )
    .ok()?;
    Some(into.at)
}

/// A time as a ping writes it: whole seconds, a point, and nine digits.
pub fn time(since_epoch: Duration) -> String {
    let mut bytes = [0u8; TIME_BYTES];
    let len = write_time(since_epoch, &mut bytes).unwrap_or(0);
    String::from_utf8_lossy(&bytes[..len]).into_owned()
}

/// The ping numbered `count`, sent at `sent`.
///
/// The count starts at one, so nought is refused: an answer numbered nought
/// would be an answer to a ping nobody sent.
pub fn message(count: u64, sent: Duration) -> Result<String, String> {
    let mut message = String::new();
    write_message(count, sent, &mut message)?;
    Ok(message)
}

/// Writes [`message`] onto the end of `out`: how a ground station that
/// keeps the room for one pings without allocating.
///
/// # Arguments
///
/// * `count` - The ping's number, from one.
/// * `sent`  - When it is sent, since the UNIX epoch.
/// * `out`   - Where the ping is appended.
///
/// # Errors
///
/// A message, with nothing written, when `count` is nought.
pub fn write_message(
    count: u64,
    sent: Duration,
    out: &mut String,
) -> Result<(), String> {
    use std::fmt::Write;

    if count == 0 {
        return Err(
            "a ping's count starts at one, and this one is nought".to_string()
        );
    }
    let mut stamp = [0u8; TIME_BYTES];
    let len = write_time(sent, &mut stamp).unwrap_or(0);
    let stamp = std::str::from_utf8(&stamp[..len]).unwrap_or_default();
    let _ = write!(out, "{PREFIX}{count} {stamp}");
    Ok(())
}

/// Reads what a ping says.
///
/// # Arguments
///
/// * `bytes` - The ping as it arrived.
///
/// # Returns
///
/// Its count and the time it was sent.
///
/// # Errors
///
/// A message quoting it when it is too long, is not ASCII text, lacks the
/// prefix or the space, or has a count or time that does not read.
pub fn read_message(bytes: &[u8]) -> Result<Ping, String> {
    if bytes.len() > MESSAGE_BYTES {
        return Err(format!(
            "{} bytes is longer than any ping, which is at most \
             {MESSAGE_BYTES}: {}",
            bytes.len(),
            said(bytes)
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| {
        format!("a ping is ASCII text, and this is not: {}", said(bytes))
    })?;
    let (count, sent) = text
        .strip_prefix(PREFIX)
        .and_then(|rest| rest.split_once(' '))
        .ok_or_else(|| {
            format!(
                "{} is not a ping, which is \"{PREFIX}<count> \
                 <seconds>.<nanoseconds>\"",
                said(bytes)
            )
        })?;
    Ok(Ping {
        count: read_count(count)
            .map_err(|why| format!("{}: {why}", said(bytes)))?,
        sent: read_time(sent)
            .map_err(|why| format!("{}: {why}", said(bytes)))?,
    })
}

/// Answers a ping: the ping, one space, and the time.
///
/// The ping is read before it is answered, so what goes back is always a ping
/// and its time: a message that is not one is refused here, and the reason
/// names what was wrong with it.
///
/// # Arguments
///
/// * `message`  - The ping as it arrived.
/// * `answered` - When it is answered, since the UNIX epoch.
///
/// # Returns
///
/// The answer, in a fixed array.
///
/// # Errors
///
/// What [`read_message`] refuses, and a time that does not fit the answer.
pub fn reply(message: &[u8], answered: Duration) -> Result<Reply, String> {
    read_message(message)?;

    // The time written straight into the answer, which is a fixed array:
    // answering a ping allocates nothing.
    let mut reply = Reply {
        bytes: [0; REPLY_BYTES],
        len: 0,
    };
    reply.bytes[..message.len()].copy_from_slice(message);
    reply.bytes[message.len()] = b' ';
    let stamp = write_time(answered, &mut reply.bytes[message.len() + 1..])
        .ok_or_else(|| "the time does not fit in an answer".to_string())?;
    reply.len = message.len() + 1 + stamp;
    Ok(reply)
}

/// Reads what an answer to a ping says.
///
/// # Arguments
///
/// * `bytes` - The answer as it arrived.
///
/// # Returns
///
/// The ping it answers and when the vehicle answered it.
///
/// # Errors
///
/// A message quoting it when it is too long, has no space, holds a ping
/// [`read_message`] refuses, or has a time that does not read.
pub fn read_reply(bytes: &[u8]) -> Result<Pong, String> {
    if bytes.len() > REPLY_BYTES {
        return Err(format!(
            "{} bytes is longer than any answer to a ping, which is at most \
             {REPLY_BYTES}: {}",
            bytes.len(),
            said(bytes)
        ));
    }
    // The vehicle's time is after the last space, and the ping is everything
    // before it.
    let at = bytes.iter().rposition(|&b| b == b' ').ok_or_else(|| {
        format!(
            "{} is not an answer to a ping, which is the ping, a space and a \
             time",
            said(bytes)
        )
    })?;
    let ping = read_message(&bytes[..at])?;
    let answered = std::str::from_utf8(&bytes[at + 1..])
        .map_err(|_| {
            format!(
                "an answer to a ping is ASCII text, and this is not: {}",
                said(bytes)
            )
        })
        .and_then(|text| {
            read_time(text).map_err(|why| format!("{}: {why}", said(bytes)))
        })?;
    Ok(Pong { ping, answered })
}

/// Reads a count: decimal digits, from one, with no leading nought.
///
/// # Arguments
///
/// * `text` - The count as the ping wrote it.
///
/// # Returns
///
/// The count.
///
/// # Errors
///
/// A message when it is not digits, starts with a nought or does not fit in
/// 64 bits.
fn read_count(text: &str) -> Result<u64, String> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("the count \"{text}\" is not a whole number"));
    }
    if text.starts_with('0') {
        return Err(format!(
            "the count \"{text}\" starts with a nought: counts start at one \
             and are written without leading noughts"
        ));
    }
    text.parse().map_err(|_| {
        format!("the count \"{text}\" is larger than {}", u64::MAX)
    })
}

/// Reads a time: whole seconds, a point, and exactly nine digits of
/// nanoseconds.
///
/// Exactly nine, not up to nine: a time written to the millisecond would read
/// as the same number, and the format promises nanoseconds. The whole seconds
/// are held to the one spelling as well, no leading nought but in nought
/// itself, so that a time has one way to be written.
///
/// # Arguments
///
/// * `text` - The time as the ping wrote it.
///
/// # Returns
///
/// The time, since the UNIX epoch.
///
/// # Errors
///
/// A message when it is not in that one spelling, or has more seconds than
/// 64 bits hold.
fn read_time(text: &str) -> Result<Duration, String> {
    let wrong = || {
        format!(
            "the time \"{text}\" is not seconds since the UNIX epoch with nine \
             digits after the point, such as 1760900000.000000001"
        )
    };
    let (seconds, nanoseconds) = text.split_once('.').ok_or_else(wrong)?;
    let digits = |part: &str| {
        !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
    };
    if !digits(seconds)
        || !digits(nanoseconds)
        || nanoseconds.len() != 9
        || (seconds.len() > 1 && seconds.starts_with('0'))
    {
        return Err(wrong());
    }
    let seconds: u64 = seconds.parse().map_err(|_| {
        format!("the time \"{text}\" has more seconds than {}", u64::MAX)
    })?;
    let nanoseconds: u32 = nanoseconds.parse().map_err(|_| wrong())?;
    Ok(Duration::new(seconds, nanoseconds))
}

/// Shows a message for a complaint: as text where it is text, and cut short
/// where it is long, so that one bad datagram cannot fill a log.
///
/// # Arguments
///
/// * `bytes` - The message.
///
/// # Returns
///
/// It quoted and escaped, with "..." where it was cut.
fn said(bytes: &[u8]) -> String {
    let shown = &bytes[..bytes.len().min(REPLY_BYTES)];
    let more = if bytes.len() > shown.len() { "..." } else { "" };
    format!(
        "\"{}{more}\"",
        String::from_utf8_lossy(shown).escape_debug()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SENT: Duration = Duration::new(1_760_900_000, 1);
    const ANSWERED: Duration = Duration::new(1_760_900_000, 250_000_000);

    /// A ping is the prefix, the count, a space and the time to the
    /// nanosecond.
    #[test]
    fn a_ping_is_written_as_the_format_says() {
        assert_eq!(
            message(1, SENT).expect("a ping"),
            "ping #1 1760900000.000000001"
        );
        assert_eq!(time(Duration::ZERO), "0.000000000");
        assert!(message(0, SENT)
            .expect_err("nought")
            .contains("starts at one"));
    }

    /// The answer is the ping, one space, and the vehicle's time.
    #[test]
    fn the_answer_is_the_ping_a_space_and_the_time() {
        let ping = message(7, SENT).expect("a ping");
        let reply = reply(ping.as_bytes(), ANSWERED).expect("an answer");
        assert_eq!(
            reply.as_bytes(),
            b"ping #7 1760900000.000000001 1760900000.250000000"
        );

        // And the ground reads back what was sent and when it was answered.
        let pong = read_reply(reply.as_bytes()).expect("it reads");
        assert_eq!(
            pong,
            Pong {
                ping: Ping {
                    count: 7,
                    sent: SENT
                },
                answered: ANSWERED
            }
        );
    }

    /// The largest ping there is, answered at the largest time there is,
    /// fills the reply buffer exactly.
    ///
    /// Which is what the two sizes promise: the buffer holds every ping, the
    /// space and every time, and is no larger than that.
    #[test]
    fn the_largest_ping_fills_the_buffer_exactly() {
        let largest = Duration::new(u64::MAX, 999_999_999);
        assert_eq!(time(largest).len(), TIME_BYTES);

        let ping = message(u64::MAX, largest).expect("the largest ping");
        assert_eq!(ping.len(), MESSAGE_BYTES);

        let reply = reply(ping.as_bytes(), largest).expect("its answer");
        assert_eq!(reply.as_bytes().len(), REPLY_BYTES);
        assert_eq!(
            read_reply(reply.as_bytes()).expect("reads").ping.count,
            u64::MAX
        );
    }

    /// Every way a message can fail to be a ping is refused, and the reason
    /// says what was wrong.
    #[test]
    fn what_is_not_a_ping_is_refused() {
        for (message, says) in [
            ("", "is not a ping"),
            ("pong #1 1760900000.000000001", "is not a ping"),
            ("ping #1", "is not a ping"),
            ("ping 1 1760900000.000000001", "is not a ping"),
            ("ping #0 1760900000.000000001", "starts with a nought"),
            ("ping #01 1760900000.000000001", "starts with a nought"),
            ("ping #x 1760900000.000000001", "is not a whole number"),
            ("ping #1  1760900000.000000001", "nine digits"),
            ("ping #1 1760900000", "nine digits"),
            ("ping #1 1760900000.0001", "nine digits"),
            ("ping #1 1760900000.0000000001", "nine digits"),
            ("ping #1 1760900000.000000001\n", "nine digits"),
            ("ping #1 01760900000.000000001", "nine digits"),
            ("ping #1 -1.000000000", "nine digits"),
            ("ping #18446744073709551616 1.000000000", "larger than"),
            (
                "ping #1 18446744073709551616.000000000",
                "more seconds than",
            ),
        ] {
            let why = read_message(message.as_bytes()).expect_err(message);
            assert!(why.contains(says), "{message:?}: {why}");
            assert!(
                reply(message.as_bytes(), ANSWERED).is_err(),
                "{message:?}"
            );
        }

        let why = read_message(&[0xff; 8]).expect_err("not text");
        assert!(why.contains("ASCII"), "{why}");
    }

    /// A message longer than any ping is refused before it is read, and the
    /// complaint shows only the start of it.
    #[test]
    fn a_message_longer_than_any_ping_is_refused() {
        let long = format!("ping #1 1760900000.000000001{}", " ".repeat(4000));
        let why = read_message(long.as_bytes()).expect_err("too long");
        assert!(why.contains("longer than any ping"), "{why}");
        assert!(why.len() < 200, "{why}");
    }

    /// An answer that is not one is refused too, as the ground reads it.
    #[test]
    fn what_is_not_an_answer_is_refused() {
        for answer in [
            "ping #1 1760900000.000000001",
            "ping #1 1760900000.000000001 now",
            "ping #1 1760900000.000000001 1760900000.25",
            "nonsense",
        ] {
            assert!(read_reply(answer.as_bytes()).is_err(), "{answer:?}");
        }
    }
}
