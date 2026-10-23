//! Waiting for the simulator: how a harness learns that tcssim has made its
//! stand-ins before it starts tcspecial.
//!
//! The one way every harness waits -- tcsmoc, `tcssim --wait`, and a test --
//! so that there is one statement of what the exchange is and of what each
//! answer means. The messages are `tcslibgs::ready`; the Python ground
//! station's library has the same function as `tcslib.simulator.wait`.

use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use tcslibgs::ready::{Answer, Request, SetUpFor, State, ANSWER_BYTES};

/// How long one request is given before it is sent again. Short, because a
/// lost request costs nothing but another, and a harness waits on the whole.
pub const RESEND_EVERY: Duration = Duration::from_millis(200);

/// What waiting came to, where it was not a simulator ready for this run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotReady {
    /// Nothing answered at all: a simulator not running, or an address that
    /// is wrong.
    Silent { at: SocketAddr, waited: Duration },
    /// It answered, and was still starting when the time ran out: a
    /// simulator that has not finished.
    StillStarting { at: SocketAddr, waited: Duration },
    /// It could not make a stand-in, or one has stopped.
    Failed { at: SocketAddr, reason: String },
    /// It is set up for something else: each difference, with what each side
    /// has.
    Differs {
        at: SocketAddr,
        differences: Vec<String>,
    },
    /// The harness's own socket could not be made.
    Socket { at: SocketAddr, why: String },
}

impl std::fmt::Display for NotReady {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotReady::Silent { at, waited } => write!(
                f,
                "nothing answered at {at} in {} s: tcssim is not running, or tcssim.ready in \
                 the simulator file is not where it is",
                waited.as_secs_f32()
            ),
            NotReady::StillStarting { at, waited } => write!(
                f,
                "the simulator at {at} was still making its stand-ins after {} s: look at \
                 what tcssim says, or give it longer",
                waited.as_secs_f32()
            ),
            NotReady::Failed { at, reason } => {
                write!(f, "the simulator at {at} could not stand in: {reason}")
            }
            NotReady::Differs { at, differences } => write!(
                f,
                "the simulator at {at} is set up for another run, and so is not used -- {}: \
                 stop it, or start this run on what it read",
                differences.join("; ")
            ),
            NotReady::Socket { at, why } => {
                write!(f, "cannot ask the simulator at {at}: {why}")
            }
        }
    }
}

/// Ask the simulator at `at` once, giving it `patience` to answer, and give
/// what it said, or `None` where nothing answered.
///
/// Requests go every [`RESEND_EVERY`] until one is answered or the time is
/// up; each carries its own number, and an answer to an earlier one counts,
/// since the state it gives is the state then and the next answer would say
/// the same or later.
pub fn ask(at: SocketAddr, patience: Duration) -> Result<Option<Answer>, NotReady> {
    let unspecified: SocketAddr = if at.is_ipv4() {
        "0.0.0.0:0".parse().expect("an address")
    } else {
        "[::]:0".parse().expect("an address")
    };
    let socket = UdpSocket::bind(unspecified).map_err(|e| NotReady::Socket {
        at,
        why: e.to_string(),
    })?;
    let started = Instant::now();
    let mut sequence = 0u32;
    let mut buffer = [0u8; ANSWER_BYTES];
    loop {
        let left = patience.saturating_sub(started.elapsed());
        if left.is_zero() {
            return Ok(None);
        }
        // An address nothing is bound at is refused by the kernel on a
        // loopback send, which is the same as no answer: ask again.
        let _ = socket.send_to(&Request { sequence }.encode(), at);
        let wait = left.min(RESEND_EVERY);
        socket
            .set_read_timeout(Some(wait.max(Duration::from_millis(1))))
            .map_err(|e| NotReady::Socket {
                at,
                why: e.to_string(),
            })?;
        let until = Instant::now() + wait;
        while Instant::now() < until {
            match socket.recv_from(&mut buffer) {
                Ok((n, from)) if from == at => {
                    if let Some(answer) = Answer::decode(&buffer[..n]) {
                        if answer.sequence <= sequence {
                            return Ok(Some(answer));
                        }
                    }
                }
                // Something else, or from elsewhere: not this exchange.
                Ok(_) => {}
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    break
                }
                // Refused, which a loopback send to a port nothing holds is
                // told at once: waited out as silence is, so that asking
                // nobody does not spin.
                Err(_) => {
                    std::thread::sleep(until.saturating_duration_since(Instant::now()));
                    break;
                }
            }
        }
        sequence = sequence.wrapping_add(1);
    }
}

/// Wait for the simulator at `at` to be ready for the run `ours` describes,
/// for as long as `patience`.
///
/// Ready, set up for this run, is `Ok`. A difference and a failure are said
/// at once, being things more waiting does not mend; silence and a simulator
/// still starting are said when the time is up, and are told apart, being
/// different faults with different remedies.
pub fn wait(at: SocketAddr, ours: &SetUpFor, patience: Duration) -> Result<(), NotReady> {
    let started = Instant::now();
    let mut heard = false;
    loop {
        let left = patience.saturating_sub(started.elapsed());
        if left.is_zero() {
            let waited = started.elapsed();
            return Err(if heard {
                NotReady::StillStarting { at, waited }
            } else {
                NotReady::Silent { at, waited }
            });
        }
        let Some(answer) = ask(at, left)? else {
            continue;
        };
        heard = true;
        let differences = ours.differences(&answer.set_up_for);
        if !differences.is_empty() {
            return Err(NotReady::Differs { at, differences });
        }
        match answer.state {
            State::Ready => return Ok(()),
            State::Failed => {
                return Err(NotReady::Failed {
                    at,
                    reason: answer.reason.as_str().to_string(),
                })
            }
            State::Starting => std::thread::sleep(RESEND_EVERY.min(left)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::Arc;
    use tcslibgs::config_digest::ConfigDigest;
    use tcslibgs::ready::{Reason, Request};
    use tcslibgs::simulate::Mode;

    fn set_up(mode: Mode) -> SetUpFor {
        SetUpFor {
            configuration: ConfigDigest([3; 16]),
            simulator: ConfigDigest([4; 16]),
            mode,
        }
    }

    /// A simulator that answers with whatever state `state` holds, as tcssim
    /// does: 0 starting, 1 ready, 2 failed.
    fn simulator(mode: Mode, state: Arc<AtomicU8>) -> SocketAddr {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let at = socket.local_addr().unwrap();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 64];
            let mut out = [0u8; ANSWER_BYTES];
            while let Ok((n, from)) = socket.recv_from(&mut buffer) {
                let Some(request) = Request::decode(&buffer[..n]) else {
                    continue;
                };
                let state = match state.load(Ordering::SeqCst) {
                    0 => State::Starting,
                    1 => State::Ready,
                    _ => State::Failed,
                };
                Answer {
                    sequence: request.sequence,
                    state,
                    set_up_for: set_up(mode),
                    reason: Reason::of("temp_3: no pty"),
                }
                .encode(&mut out);
                let _ = socket.send_to(&out, from);
            }
        });
        at
    }

    #[test]
    fn a_ready_simulator_set_up_for_this_run_is_waited_for() {
        let state = Arc::new(AtomicU8::new(0));
        let at = simulator(Mode::All, state.clone());
        let later = state.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            later.store(1, Ordering::SeqCst);
        });
        wait(at, &set_up(Mode::All), Duration::from_secs(5)).expect("ready");
    }

    #[test]
    fn a_failure_and_a_difference_are_said_at_once() {
        let at = simulator(Mode::All, Arc::new(AtomicU8::new(2)));
        let said = wait(at, &set_up(Mode::All), Duration::from_secs(5)).unwrap_err();
        assert_eq!(
            said,
            NotReady::Failed {
                at,
                reason: "temp_3: no pty".into()
            }
        );

        let at = simulator(Mode::Some, Arc::new(AtomicU8::new(1)));
        let said = wait(at, &set_up(Mode::All), Duration::from_secs(5)).unwrap_err();
        assert!(matches!(said, NotReady::Differs { .. }), "{said}");
    }

    #[test]
    fn silence_and_starting_are_told_apart() {
        let at = simulator(Mode::All, Arc::new(AtomicU8::new(0)));
        let said = wait(at, &set_up(Mode::All), Duration::from_millis(500)).unwrap_err();
        assert!(matches!(said, NotReady::StillStarting { .. }), "{said}");

        // A port nothing answers at.
        let nobody = UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let said = wait(nobody, &set_up(Mode::All), Duration::from_millis(500)).unwrap_err();
        assert!(matches!(said, NotReady::Silent { .. }), "{said}");
    }
}
