//! Conduit implementation for TCSpecial
//!
//! Conduits move data between endpoints in one direction.

use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tcslibgs::{DHSample, Statistics, TcsError, TcsResult};

use crate::config::constants::ENDPOINT_BUFFER_SIZE;
use crate::endpoint::{pause_until_commanded, EndpointReadable, EndpointWritable, WaitResult};

/// How long a conduit waits on its endpoint before looking at anything else.
const WAIT_MS: i32 = 1000;

/// How many reads of no bytes in a row are taken for an endpoint at its end.
///
/// One is ordinary: a datagram of no bytes is the ground saying where it is,
/// and the wait after it blocks as usual. A stream whose far end has closed
/// reports itself readable for ever and reads nothing every time, which is a
/// loop that spins. Three in a row says which of the two this is without
/// delaying the first.
const QUIET_READS: u32 = 3;

/// What a handler sends its payload to get anything back, and how often.
///
/// Only a triggered handler has one, and only its ground-to-payload conduit:
/// the trigger goes the way the ground's data goes, because it goes to the
/// same place.
#[derive(Debug, Clone)]
pub struct Polling {
    pub trigger: Vec<u8>,
    pub interval: Duration,
}

/// Direction of data flow in a conduit
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConduitDirection {
    /// Ground to payload
    GroundToPayload,
    /// Payload to ground
    PayloadToGround,
}

/// Command for conduit control
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConduitCommand {
    /// Stop the conduit
    Stop,
    /// Get statistics
    GetStats,
}

/// What a data handler last moved, in each direction.
///
/// Shared between the conduit threads that write it and the command
/// interpreter that reads it, because a conduit's statistics reach the handler
/// only when the conduit stops, and a panel asks while data is still flowing.
///
/// Both senses are the handler's, as its statistics are: `received` is data
/// that came from the ground and `sent` is data that went to it. So one
/// conduit fills each -- the ground-to-payload conduit on its read, the
/// payload-to-ground conduit on its write -- rather than both filling both,
/// which would leave each field holding whichever direction moved last.
#[derive(Debug, Clone, Copy, Default)]
pub struct DHSamples {
    /// The last data sent to the ground.
    pub sent: DHSample,
    /// The last data received from the ground.
    pub received: DHSample,
}

/// Conduit thread state
pub struct Conduit {
    direction: ConduitDirection,
    running: Arc<AtomicBool>,
    /// Counted as data moves, and shared so it can be read while it moves.
    ///
    /// A conduit's thread used to keep these to itself and hand them back when
    /// it was joined, which meant a running handler reported nothing: every
    /// counter read zero until it was stopped, however much had gone through.
    ///
    /// Locked rather than tried, unlike the samples beside it: a sample missed
    /// under contention costs one stale line on a display, where a count
    /// missed is a count wrong from then on. The lock is held for a few
    /// additions and by a reader only long enough to copy.
    stats: Arc<Mutex<Statistics>>,
    thread_handle: Option<JoinHandle<()>>,
    cmd_pipe_write: RawFd,
}

/// Add to the counts.
///
/// Unlike [`record`], this waits for the lock: a count that went missing would
/// stay missing, and the hold is a few additions long.
fn count(stats: &Mutex<Statistics>, f: impl FnOnce(&mut Statistics)) {
    if let Ok(mut guard) = stats.lock() {
        f(&mut guard);
    }
}

/// Record a sample, unless someone is reading them.
///
/// Deliberately best effort: the samples exist for a display, and payload data
/// must not wait on a panel that happens to be asking. A lost sample costs one
/// stale line on a screen, where a blocked conduit costs data.
fn record(samples: &Mutex<DHSamples>, f: impl FnOnce(&mut DHSamples)) {
    if let Ok(mut guard) = samples.try_lock() {
        f(&mut guard);
    }
}

impl Conduit {
    /// Create a new conduit
    ///
    /// The endpoints belong to [`Conduit::start`], not here: a conduit that
    /// has not started owns nothing to read or write, and taking them twice
    /// once meant the caller had to hand over two sets.
    pub fn new(direction: ConduitDirection, cmd_pipe_write: RawFd) -> Self {
        let running = Arc::new(AtomicBool::new(false));

        Self {
            direction,
            running,
            stats: Arc::new(Mutex::new(Statistics::new())),
            thread_handle: None,
            cmd_pipe_write,
        }
    }

    /// Start the conduit thread
    ///
    /// `polling` is what this conduit sends its payload of its own accord, and
    /// is given only to the ground-to-payload conduit of a triggered handler.
    /// Every other conduit only ever copies: it reads one endpoint and writes
    /// the other, and originates nothing.
    pub fn start(
        &mut self,
        mut reader: Box<dyn EndpointReadable + Send>,
        mut writer: Box<dyn EndpointWritable + Send>,
        cmd_fd: RawFd,
        samples: Arc<Mutex<DHSamples>>,
        polling: Option<Polling>,
    ) -> TcsResult<()> {
        if self.running.load(Ordering::SeqCst) {
            return Err(TcsError::DataHandler("Conduit already running".to_string()));
        }

        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);

        let direction = self.direction;
        let stats = self.stats.clone();

        let handle = thread::spawn(move || {
            let mut buffer = vec![0u8; ENDPOINT_BUFFER_SIZE];

            // When the next trigger is due, for a conduit that sends one. Now,
            // to begin with: a payload that answers requests has nothing to
            // say until it has been asked, so waiting out an interval first
            // would be a handler that starts by doing nothing.
            let mut due = Instant::now();

            // Reads of no bytes in a row; see QUIET_READS.
            let mut quiet = 0;

            // Wait out the rest of the interval, or a whole wait where there
            // is no interval to wait out.
            //
            // For the passes that have nothing to do and nothing to wait on
            // either: an endpoint in error or at its end is ready the instant
            // it is polled, so a loop that did not pause would spin. A
            // conduit that asks still asks on time, because what this waits
            // out is the time until it is due; one that does not ask has
            // nothing to be on time for and waits as it always waits.
            //
            // Spent on the command pipe rather than asleep, so that a stop is
            // answered while a pause is still running; see
            // `pause_until_commanded`.
            let most = Duration::from_millis(WAIT_MS as u64);
            let rest = |polls: bool, due: Instant| {
                let left = if polls {
                    due.saturating_duration_since(Instant::now())
                } else {
                    most
                };
                let ms = (left.min(most).as_millis() as i32).clamp(1, WAIT_MS);
                pause_until_commanded(cmd_fd, ms);
            };

            while running.load(Ordering::SeqCst) {
                // A conduit that polls waits only as long as the next trigger
                // allows, so that the interval is the interval rather than the
                // interval rounded up to the next wait. One that does not poll
                // keeps the wait it always had.
                let timeout_ms = match &polling {
                    None => WAIT_MS,
                    Some(_) => {
                        // At least a millisecond. The wait is in whole
                        // milliseconds and the time left is not, so the last
                        // fraction of every interval truncated to a wait of
                        // nought -- a poll that returns at once, a loop that
                        // spins through the rest of the interval, and a core
                        // held down by a handler that was doing nothing.
                        let left = due.saturating_duration_since(Instant::now());
                        (left.as_millis() as i32).clamp(1, WAIT_MS)
                    }
                };

                // Wait for I/O or command
                match reader.wait_for_event(cmd_fd, timeout_ms) {
                    Ok(WaitResult::CommandPending) | Ok(WaitResult::Both) => {
                        // Read command byte from pipe
                        let mut cmd_buf = [0u8; 1];
                        unsafe {
                            libc::read(cmd_fd, cmd_buf.as_mut_ptr() as *mut libc::c_void, 1);
                        }
                        // Check if we should stop
                        if cmd_buf[0] == 0 {
                            break;
                        }
                    }
                    Ok(WaitResult::IoReady) => {
                        // Read from source
                        match reader.read(&mut buffer) {
                            // Nothing to copy. Not a reason to go round
                            // without asking, which is what this did: a
                            // triggered handler whose payload had closed read
                            // nothing for ever and never reached the asking
                            // below, so it stopped asking and spun while it
                            // did so.
                            Ok(0) => {
                                quiet += 1;
                                if quiet >= QUIET_READS {
                                    rest(polling.is_some(), due);
                                }
                            }
                            Ok(n) => {
                                quiet = 0;
                                count(&stats, |s| {
                                    s.bytes_received += n as u64;
                                    s.reads_completed += 1;
                                });
                                // Only the ground-facing half of each conduit
                                // is a sample, so that the two lines a panel
                                // shows mean what the two byte counters beside
                                // them mean: received is data from the ground,
                                // sent is data to it. Recording both halves of
                                // both conduits leaves each field holding
                                // whichever direction moved last.
                                if direction == ConduitDirection::GroundToPayload {
                                    record(&samples, |s| s.received.record(&buffer[..n]));
                                }

                                // Write to destination
                                match writer.write(&buffer[..n]) {
                                    Ok(written) => {
                                        count(&stats, |s| {
                                            s.bytes_sent += written as u64;
                                            s.writes_completed += 1;
                                        });
                                        if direction == ConduitDirection::PayloadToGround {
                                            record(&samples, |s| {
                                                s.sent.record(&buffer[..written])
                                            });
                                        }
                                    }
                                    Err(_) => {
                                        count(&stats, |s| s.writes_failed += 1);
                                    }
                                }
                            }
                            Err(_) => {
                                count(&stats, |s| s.reads_failed += 1);
                            }
                        }
                    }
                    Ok(WaitResult::Timeout) => quiet = 0,
                    // The endpoint reported something this did not ask for --
                    // POLLHUP from a payload that has gone away, POLLERR
                    // after a write that got nowhere -- or the wait itself
                    // failed.
                    //
                    // This used to end the conduit, and with it the asking: a
                    // triggered handler whose payload went away stopped
                    // asking and never asked again, which is the one thing
                    // such a handler is for. The payload may come back, and a
                    // handler is stopped by a STOP_DH rather than by its
                    // payload's silence.
                    //
                    // Counted as a failed read, which is what it is from
                    // here: the wait for something to read came to nothing.
                    Ok(WaitResult::Error) | Err(_) => {
                        count(&stats, |s| s.reads_failed += 1);
                        rest(polling.is_some(), due);
                    }
                }

                // And then the asking, if this conduit asks. After the copying
                // rather than before it, so that a trigger never delays data
                // the ground has already sent.
                    if let Some(polling) = &polling {
                    if Instant::now() >= due {
                        match writer.write(&polling.trigger) {
                            Ok(written) => count(&stats, |s| {
                                s.bytes_sent += written as u64;
                                s.writes_completed += 1;
                                s.triggers_sent += 1;
                            }),
                            // A payload that will not take a trigger is one
                            // that will not be answering either, which this
                            // count is what says so. Not `writes_failed`:
                            // that is data on its way to the ground, which a
                            // trigger is not, and a handler reports the two
                            // apart.
                            Err(_) => count(&stats, |s| s.triggers_failed += 1),
                        }
                        due = Instant::now() + polling.interval;
                    }
                }
            }

        });

        self.thread_handle = Some(handle);
        Ok(())
    }

    /// What this conduit has counted so far.
    ///
    /// Readable while the conduit runs, which is the point of it: a handler is
    /// asked for its statistics far more often than it is stopped.
    pub fn statistics(&self) -> Statistics {
        self.stats.lock().map(|guard| *guard).unwrap_or_default()
    }

    /// Stop the conduit thread
    pub fn stop(&mut self) -> TcsResult<Statistics> {
        self.running.store(false, Ordering::SeqCst);

        // Send stop command through pipe
        let cmd = [0u8; 1];
        unsafe {
            libc::write(self.cmd_pipe_write, cmd.as_ptr() as *const libc::c_void, 1);
        }

        if let Some(handle) = self.thread_handle.take() {
            handle
                .join()
                .map_err(|_| TcsError::DataHandler("Thread join failed".to_string()))?;
        }

        // Read after the join, so nothing the thread was in the middle of
        // counting is left out of what the handler accumulates.
        Ok(self.statistics())
    }

    /// Check if the conduit is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get the conduit direction
    pub fn direction(&self) -> ConduitDirection {
        self.direction
    }
}

impl Drop for Conduit {
    fn drop(&mut self) {
        if self.is_running() {
            let _ = self.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::os::unix::io::RawFd;

    use crate::endpoint::{EndpointWaitable, WaitResult};

    #[test]
    fn test_conduit_direction() {
        assert_ne!(ConduitDirection::GroundToPayload, ConduitDirection::PayloadToGround);
    }

    /// An endpoint whose wait always comes to nothing.
    ///
    /// What POLLERR looks like to a conduit: the wait returns, and neither
    /// the endpoint nor the command pipe is what returned it.
    struct Erring;

    impl EndpointWaitable for Erring {
        fn io_fd(&self) -> RawFd {
            -1
        }

        fn wait_for_event(&self, _cmd_fd: RawFd, _timeout_ms: i32) -> TcsResult<WaitResult> {
            Ok(WaitResult::Error)
        }
    }

    impl EndpointReadable for Erring {
        fn read(&mut self, _buffer: &mut [u8]) -> TcsResult<usize> {
            Ok(0)
        }
    }

    /// A payload that has gone away: every write to it fails, and it counts
    /// how many times it was tried.
    struct Gone(Arc<Mutex<usize>>);

    impl EndpointWaitable for Gone {
        fn io_fd(&self) -> RawFd {
            -1
        }

        fn wait_for_event(&self, _cmd_fd: RawFd, _timeout_ms: i32) -> TcsResult<WaitResult> {
            Ok(WaitResult::Error)
        }
    }

    impl EndpointWritable for Gone {
        fn write(&mut self, _data: &[u8]) -> TcsResult<usize> {
            *self.0.lock().expect("the count") += 1;
            Err(TcsError::DataHandler("gone".to_string()))
        }
    }

    /// An endpoint that is readable for ever and reads nothing every time.
    ///
    /// What a stream whose far end has closed looks like: poll says there is
    /// something, and the read says end of file. The count is how many times
    /// it was asked.
    struct Ended(Arc<Mutex<usize>>);

    impl EndpointWaitable for Ended {
        fn io_fd(&self) -> RawFd {
            -1
        }

        fn wait_for_event(&self, _cmd_fd: RawFd, _timeout_ms: i32) -> TcsResult<WaitResult> {
            Ok(WaitResult::IoReady)
        }
    }

    impl EndpointReadable for Ended {
        fn read(&mut self, _buffer: &mut [u8]) -> TcsResult<usize> {
            *self.0.lock().expect("the count") += 1;
            Ok(0)
        }
    }

    /// A payload that takes everything and says nothing about it.
    struct Sink;

    impl EndpointWaitable for Sink {
        fn io_fd(&self) -> RawFd {
            -1
        }

        fn wait_for_event(&self, _cmd_fd: RawFd, _timeout_ms: i32) -> TcsResult<WaitResult> {
            Ok(WaitResult::Timeout)
        }
    }

    impl EndpointWritable for Sink {
        fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
            Ok(data.len())
        }
    }

    /// A conduit whose endpoint is at its end waits rather than spinning.
    ///
    /// A read of no bytes is not a reason to go straight round again: an
    /// endpoint whose far end has closed is ready the instant it is polled
    /// and reads nothing every time, so a conduit that only ever went round
    /// held a core down for as long as the handler was running.
    #[test]
    fn a_conduit_waits_out_an_endpoint_at_its_end() {
        let mut pipe = [0 as RawFd; 2];
        assert_eq!(unsafe { libc::pipe(pipe.as_mut_ptr()) }, 0, "a command pipe");
        let (cmd_read, cmd_write) = (pipe[0], pipe[1]);

        let reads = Arc::new(Mutex::new(0usize));
        let mut conduit = Conduit::new(ConduitDirection::PayloadToGround, cmd_write);
        conduit
            .start(
                Box::new(Ended(reads.clone())),
                Box::new(Sink),
                cmd_read,
                Arc::new(Mutex::new(DHSamples::default())),
                None,
            )
            .expect("the conduit starts");

        thread::sleep(Duration::from_millis(200));
        let asked = *reads.lock().expect("the count");

        // And the pause is spent where a stop can cut it short: a conduit
        // that slept it out would answer a STOP_DH only when it woke.
        let asked_to_stop = Instant::now();
        let _ = conduit.stop();
        let took = asked_to_stop.elapsed();
        assert!(
            took < Duration::from_millis(500),
            "the stop waited {took:?} for the pause to run out"
        );

        // A handful, not a hundred thousand: enough to tell an end from a
        // datagram of no bytes, and then the wait it always had.
        assert!(
            asked <= QUIET_READS as usize + 2,
            "it read {asked} times in 200 ms: it is spinning"
        );
        assert!(asked > 0, "it did not read at all");

        unsafe {
            libc::close(cmd_read);
            libc::close(cmd_write);
        }
    }

    /// A conduit whose endpoint errs and whose payload refuses goes on asking.
    ///
    /// Both halves of what a triggered handler owes its payload. The wait
    /// erring used to end the conduit, and a trigger that would not go used
    /// to be counted as a write to the ground; either way a payload that
    /// stopped answering was a handler that stopped asking, and a payload
    /// that came back was never heard from again.
    #[test]
    fn a_polling_conduit_asks_on_through_errors() {
        let mut pipe = [0 as RawFd; 2];
        assert_eq!(unsafe { libc::pipe(pipe.as_mut_ptr()) }, 0, "a command pipe");
        let (cmd_read, cmd_write) = (pipe[0], pipe[1]);

        let tries = Arc::new(Mutex::new(0usize));
        let mut conduit = Conduit::new(ConduitDirection::GroundToPayload, cmd_write);
        conduit
            .start(
                Box::new(Erring),
                Box::new(Gone(tries.clone())),
                cmd_read,
                Arc::new(Mutex::new(DHSamples::default())),
                Some(Polling {
                    trigger: b"READ\r".to_vec(),
                    interval: Duration::from_millis(5),
                }),
            )
            .expect("the conduit starts");

        // Several intervals' worth of asking, which is what an endpoint in
        // error must not put a stop to.
        let waited = (0..100)
            .find(|_| {
                thread::sleep(Duration::from_millis(10));
                *tries.lock().expect("the count") >= 3
            })
            .is_some();
        assert!(
            waited,
            "it asked {} times through an erring endpoint",
            *tries.lock().expect("the count")
        );
        assert!(conduit.is_running(), "an endpoint in error ended the conduit");

        // And the asking is in its own counters, apart from the data it
        // carries for the ground.
        let stats = conduit.stop().expect("it stops");
        assert!(stats.triggers_failed >= 3, "{stats:?}");
        assert_eq!(stats.triggers_sent, 0, "nothing was taken: {stats:?}");
        assert_eq!(stats.writes_failed, 0, "a trigger is not data: {stats:?}");

        unsafe {
            libc::close(cmd_read);
            libc::close(cmd_write);
        }
    }
}
