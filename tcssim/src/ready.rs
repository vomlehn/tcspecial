//! Whether this simulator has made its stand-ins, and the socket it says so
//! on.
//!
//! Bound before any stand-in is made and kept bound for as long as tcssim
//! runs, so that a harness asking while it is still starting is told so, and
//! so that a simulator left running from an earlier run is found: it holds
//! the address, and a second one started beside it is refused rather than
//! left to fight it for the payloads' paths. The messages are
//! `tcslibgs::ready`, and a harness waits through `tcslib::simulator`. See
//! "Waiting for the Simulator" in `docs/design.rst`.

use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use tcslibgs::ready::{
    Answer, Reason, Request, SetUpFor, State, ANSWER_BYTES, REQUEST_BYTES,
};

const MAKING: u8 = 0;
const MADE: u8 = 1;
const FAILED: u8 = 2;

/// Whether one payload's stand-in has been made: told by the payload's own
/// thread, which is the one that makes it.
#[derive(Debug, Default)]
pub struct Standing {
    state: AtomicU8,
    /// Why it failed, for a payload that did. Fixed in size, so that
    /// answering copies it and allocates nothing.
    reason: Mutex<Option<Reason>>,
}

impl Standing {
    /// The stand-in exists and answers.
    pub fn made(&self) {
        // A stand-in that failed stays failed: a later success is a retry
        // nothing here makes, and the answer must not go back to ready over
        // a failure nobody has seen.
        let _ = self.state.compare_exchange(
            MAKING,
            MADE,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
    }

    /// Records that the stand-in could not be made, or has stopped, and why.
    ///
    /// # Arguments
    ///
    /// * `why` - Why, as the harness is to be told it; only the first
    ///           failure's reason is kept.
    pub fn failed(&self, why: &str) {
        if let Ok(mut reason) = self.reason.lock() {
            if reason.is_none() {
                *reason = Some(Reason::of(why));
            }
        }
        self.state.store(FAILED, Ordering::SeqCst);
    }

    fn state(&self) -> u8 {
        self.state.load(Ordering::SeqCst)
    }

    /// Returns why the stand-in failed.
    ///
    /// # Returns
    ///
    /// The reason recorded by [`Standing::failed`], or `Reason::EMPTY` if
    /// there is none or the lock is poisoned.
    fn reason(&self) -> Reason {
        self.reason
            .lock()
            .ok()
            .and_then(|reason| *reason)
            .unwrap_or(Reason::EMPTY)
    }
}

/// Every payload's standing, and what this simulator is set up for.
pub struct Readiness {
    pub payloads: Vec<Arc<Standing>>,
    pub set_up_for: SetUpFor,
}

impl Readiness {
    /// Works out the state now: failed if any stand-in failed, ready if
    /// every one is made, and starting otherwise.
    ///
    /// # Returns
    ///
    /// The state, and the first failed stand-in's reason, or
    /// `Reason::EMPTY` if none failed.
    fn now(&self) -> (State, Reason) {
        if let Some(failed) = self.payloads.iter().find(|p| p.state() == FAILED)
        {
            return (State::Failed, failed.reason());
        }
        if self.payloads.iter().all(|p| p.state() == MADE) {
            (State::Ready, Reason::EMPTY)
        } else {
            (State::Starting, Reason::EMPTY)
        }
    }
}

/// Binds the readiness socket, or says why it cannot be: an address
/// already taken is most likely a simulator already running.
///
/// # Arguments
///
/// * `at` - The address to bind, the simulator file's `tcssim.ready`.
///
/// # Returns
///
/// The bound socket.
///
/// # Errors
///
/// A message for the user if the address is taken or cannot be bound.
pub fn bind(at: SocketAddr) -> Result<UdpSocket, String> {
    UdpSocket::bind(at).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AddrInUse {
            format!(
                "tcssim.ready, {at}, is already taken -- most likely by a \
                 tcssim left running from an earlier run: stop that one, or \
                 give this simulator file a tcssim.ready no other run uses"
            )
        } else {
            format!("tcssim.ready, {at}, cannot be bound: {e}")
        }
    })
}

/// Answers every request on a socket with the state now, for as long as
/// the process runs, from a thread of its own.
///
/// The loop allocates nothing: the request and the answer are read and
/// written in buffers made before it, and the state is copied out of fixed
/// storage.
///
/// # Arguments
///
/// * `socket`    - The readiness socket, from [`bind`].
/// * `readiness` - The payloads' standing and the set-up it answers with.
///
/// # Panics
///
/// If the thread cannot be started.
pub fn serve(socket: UdpSocket, readiness: Arc<Readiness>) {
    std::thread::Builder::new()
        .name("readiness".to_string())
        .spawn(move || {
            let mut request = [0u8; REQUEST_BYTES + 1];
            let mut answer = [0u8; ANSWER_BYTES];
            loop {
                let Ok((n, from)) = socket.recv_from(&mut request) else {
                    continue;
                };
                let Some(asked) = Request::decode(&request[..n]) else {
                    continue;
                };
                let (state, reason) = readiness.now();
                Answer {
                    sequence: asked.sequence,
                    state,
                    set_up_for: readiness.set_up_for,
                    reason,
                }
                .encode(&mut answer);
                let _ = socket.send_to(&answer, from);
            }
        })
        .expect("the readiness thread starts");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslibgs::config_digest::ConfigDigest;
    use tcslibgs::simulate::Mode;

    /// Makes a readiness for payloads none of which is made yet.
    ///
    /// # Arguments
    ///
    /// * `n` - How many payloads.
    ///
    /// # Returns
    ///
    /// The readiness, set up for all-zero digests and `Mode::All`.
    fn readiness(n: usize) -> Readiness {
        Readiness {
            payloads: (0..n).map(|_| Arc::new(Standing::default())).collect(),
            set_up_for: SetUpFor {
                configuration: ConfigDigest([0; 16]),
                simulator: ConfigDigest([0; 16]),
                mode: Mode::All,
            },
        }
    }

    #[test]
    fn ready_once_every_stand_in_is_made_and_failed_once_any_fails() {
        let r = readiness(2);
        assert_eq!(r.now().0, State::Starting);
        r.payloads[0].made();
        assert_eq!(r.now().0, State::Starting);
        r.payloads[1].made();
        assert_eq!(r.now().0, State::Ready);
        r.payloads[1].failed("b: its pty went away");
        let (state, reason) = r.now();
        assert_eq!(state, State::Failed);
        assert_eq!(reason.as_str(), "b: its pty went away");
        // And it stays failed.
        r.payloads[1].made();
        assert_eq!(r.now().0, State::Failed);
    }

    /// A second simulator on the same address is refused, naming it.
    #[test]
    fn an_address_already_taken_is_refused_naming_it() {
        let held = UdpSocket::bind("127.0.0.1:0").unwrap();
        let at = held.local_addr().unwrap();
        let said = bind(at).unwrap_err();
        assert!(said.contains(&at.to_string()), "{said}");
        assert!(said.contains("already taken"), "{said}");
    }
}
