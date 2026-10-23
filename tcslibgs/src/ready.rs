//! Whether the simulator is ready: the messages a harness and tcssim exchange
//! on tcssim's readiness socket.
//!
//! A stand-in has to exist before tcspecial looks for it, so whatever starts a
//! run with stand-ins -- the harness -- starts tcssim first and asks it, at
//! the UDP address its simulator file states as `tcssim.ready`, whether it has
//! made them. tcspecial takes no part: flight software that knows nothing of
//! a simulator cannot wait for the wrong one. See "Waiting for the Simulator"
//! in `docs/design.rst`.
//!
//! The exchange is a request and an answer and nothing is held between
//! exchanges: a request carries a sequence number and the answer carries it
//! back, so a harness can be stopped, started again and ask as often as it
//! likes, and a lost request is simply sent again.
//!
//! Both messages are fixed in size and read and written in fixed buffers, so
//! answering is no allocation in tcssim's running loop. Big-endian, as the
//! rest of the link is.

use crate::config_digest::ConfigDigest;
use crate::simulate::Mode;

/// What a request begins with: `TSRQ`.
const REQUEST_MAGIC: [u8; 4] = *b"TSRQ";
/// What an answer begins with: `TSRA`.
const ANSWER_MAGIC: [u8; 4] = *b"TSRA";

/// How long a request is: the magic and a sequence number.
pub const REQUEST_BYTES: usize = 8;

/// The most of a failure's reason an answer carries. A longer one is cut
/// short, at a character, as every other text the protocol carries is.
pub const REASON_BYTES: usize = 240;

/// How long an answer is: the magic, the sequence number, the state, the
/// mode, the two digests, the reason's length and the reason.
pub const ANSWER_BYTES: usize = 4 + 4 + 1 + 1 + 16 + 16 + 2 + REASON_BYTES;

/// A harness asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    pub sequence: u32,
}

impl Request {
    pub fn encode(&self) -> [u8; REQUEST_BYTES] {
        let mut out = [0u8; REQUEST_BYTES];
        out[..4].copy_from_slice(&REQUEST_MAGIC);
        out[4..].copy_from_slice(&self.sequence.to_be_bytes());
        out
    }

    /// A request, or `None` for a datagram that is not one: something else
    /// sent to the address is ignored rather than answered.
    pub fn decode(bytes: &[u8]) -> Option<Request> {
        if bytes.len() != REQUEST_BYTES || bytes[..4] != REQUEST_MAGIC {
            return None;
        }
        Some(Request {
            sequence: u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        })
    }
}

/// The simulator's state now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Still making stand-ins.
    Starting,
    /// Every stand-in it will provide exists and answers: each link made,
    /// each socket bound or listening, each `i2c-stub` chip found and loaded.
    /// The stand-ins are there; nothing is said about any packet.
    Ready,
    /// It could not make a stand-in, or one has stopped: the reason says
    /// which payload and why.
    Failed,
}

impl State {
    fn code(self) -> u8 {
        match self {
            State::Starting => 0,
            State::Ready => 1,
            State::Failed => 2,
        }
    }

    fn of(code: u8) -> Option<State> {
        match code {
            0 => Some(State::Starting),
            1 => Some(State::Ready),
            2 => Some(State::Failed),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            State::Starting => "starting",
            State::Ready => "ready",
            State::Failed => "failed",
        }
    }
}

fn mode_code(mode: Mode) -> u8 {
    match mode {
        Mode::None => 0,
        Mode::Some => 1,
        Mode::All => 2,
    }
}

fn mode_of(code: u8) -> Option<Mode> {
    match code {
        0 => Some(Mode::None),
        1 => Some(Mode::Some),
        2 => Some(Mode::All),
        _ => None,
    }
}

/// A failure's reason, held in a fixed buffer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Reason {
    bytes: [u8; REASON_BYTES],
    len: usize,
}

impl Reason {
    pub const EMPTY: Reason = Reason {
        bytes: [0; REASON_BYTES],
        len: 0,
    };

    /// `text`, cut short at a character where it would not fit.
    pub fn of(text: &str) -> Reason {
        let mut len = text.len().min(REASON_BYTES);
        while !text.is_char_boundary(len) {
            len -= 1;
        }
        let mut reason = Reason::EMPTY;
        reason.bytes[..len].copy_from_slice(&text.as_bytes()[..len]);
        reason.len = len;
        reason
    }

    pub fn as_str(&self) -> &str {
        // Only ever filled from a str, cut at a character, or checked as it
        // was decoded.
        std::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
}

impl std::fmt::Debug for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

/// What the simulator is set up for: what a harness compares with its own
/// before it starts tcspecial.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetUpFor {
    /// The digest of the tcspecial configuration it read, as
    /// `crate::config_digest::digest_of_graph` takes it.
    pub configuration: ConfigDigest,
    /// The digest of its simulator file: see
    /// `crate::config_digest::digest_of_simulator_file`.
    pub simulator: ConfigDigest,
    /// Its `--simulate` mode.
    pub mode: Mode,
}

impl SetUpFor {
    /// Each way this differs from `theirs`, which is the simulator's, named
    /// with what each side has: empty where the two agree.
    pub fn differences(&self, theirs: &SetUpFor) -> Vec<String> {
        let mut differ = Vec::new();
        if self.configuration != theirs.configuration {
            differ.push(format!(
                "the configuration: md5 {} here, md5 {} in the simulator",
                self.configuration, theirs.configuration
            ));
        }
        if self.simulator != theirs.simulator {
            differ.push(format!(
                "the simulator file: md5 {} here, md5 {} in the simulator",
                self.simulator, theirs.simulator
            ));
        }
        if self.mode != theirs.mode {
            differ.push(format!(
                "the mode: --simulate={} here, --simulate={} in the simulator",
                self.mode, theirs.mode
            ));
        }
        differ
    }
}

/// The simulator answering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Answer {
    /// The request's sequence number, back.
    pub sequence: u32,
    pub state: State,
    pub set_up_for: SetUpFor,
    /// Why, for `Failed`; empty otherwise.
    pub reason: Reason,
}

impl Answer {
    /// Written into `out`, whose first [`ANSWER_BYTES`] bytes it fills.
    pub fn encode(&self, out: &mut [u8; ANSWER_BYTES]) {
        out.fill(0);
        out[..4].copy_from_slice(&ANSWER_MAGIC);
        out[4..8].copy_from_slice(&self.sequence.to_be_bytes());
        out[8] = self.state.code();
        out[9] = mode_code(self.set_up_for.mode);
        out[10..26].copy_from_slice(&self.set_up_for.configuration.0);
        out[26..42].copy_from_slice(&self.set_up_for.simulator.0);
        out[42..44].copy_from_slice(&(self.reason.len as u16).to_be_bytes());
        out[44..44 + self.reason.len].copy_from_slice(&self.reason.bytes[..self.reason.len]);
    }

    /// An answer, or `None` for a datagram that is not one.
    pub fn decode(bytes: &[u8]) -> Option<Answer> {
        if bytes.len() != ANSWER_BYTES || bytes[..4] != ANSWER_MAGIC {
            return None;
        }
        let digest = |at: usize| {
            let mut digest = [0u8; 16];
            digest.copy_from_slice(&bytes[at..at + 16]);
            ConfigDigest(digest)
        };
        let len = u16::from_be_bytes([bytes[42], bytes[43]]) as usize;
        if len > REASON_BYTES {
            return None;
        }
        let text = std::str::from_utf8(&bytes[44..44 + len]).ok()?;
        Some(Answer {
            sequence: u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            state: State::of(bytes[8])?,
            set_up_for: SetUpFor {
                configuration: digest(10),
                simulator: digest(26),
                mode: mode_of(bytes[9])?,
            },
            reason: Reason::of(text),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(state: State, reason: &str) -> Answer {
        Answer {
            sequence: 0xdead_beef,
            state,
            set_up_for: SetUpFor {
                configuration: ConfigDigest([1; 16]),
                simulator: ConfigDigest([2; 16]),
                mode: Mode::All,
            },
            reason: Reason::of(reason),
        }
    }

    #[test]
    fn a_request_and_an_answer_come_back_as_they_went() {
        let request = Request { sequence: 7 };
        assert_eq!(Request::decode(&request.encode()), Some(request));

        for (state, reason) in [
            (State::Starting, ""),
            (State::Ready, ""),
            (State::Failed, "temp_3: /dev/x is taken"),
        ] {
            let sent = answer(state, reason);
            let mut out = [0u8; ANSWER_BYTES];
            sent.encode(&mut out);
            assert_eq!(Answer::decode(&out), Some(sent));
        }
    }

    #[test]
    fn what_is_not_a_message_is_not_read_as_one() {
        assert_eq!(Request::decode(b"TSRQ"), None);
        assert_eq!(Request::decode(b"XXXX\0\0\0\x01"), None);
        let mut out = [0u8; ANSWER_BYTES];
        answer(State::Ready, "").encode(&mut out);
        out[8] = 9;
        assert_eq!(Answer::decode(&out), None, "no such state");
        assert_eq!(Answer::decode(&out[..10]), None, "too short");
    }

    /// A reason too long to carry is cut short at a character, not in one.
    #[test]
    fn a_long_reason_is_cut_at_a_character() {
        let long = "é".repeat(REASON_BYTES);
        let reason = Reason::of(&long);
        assert!(reason.as_str().len() <= REASON_BYTES);
        assert!(reason.as_str().chars().all(|c| c == 'é'));
    }

    #[test]
    fn a_difference_names_what_differs_and_what_each_side_has() {
        let ours = answer(State::Ready, "").set_up_for;
        assert!(ours.differences(&ours).is_empty());
        let theirs = SetUpFor {
            mode: Mode::Some,
            ..ours
        };
        assert_eq!(
            ours.differences(&theirs),
            ["the mode: --simulate=all here, --simulate=some in the simulator"]
        );
    }
}
