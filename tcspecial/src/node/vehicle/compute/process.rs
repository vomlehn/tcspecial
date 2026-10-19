//! A computation that is a program of its own.
//!
//! What a built-in cannot be: a calculation somebody wants to change without
//! rebuilding the flight software, or to write in something other than Rust.
//! tcspecial starts the program, writes what arrives on its standard input a
//! line at a time, and reads a result from its standard output.
//!
//! Flexibility is last in this project's ranking, and this is the piece that
//! buys it, so the arrangement is deliberately strict. Every rule below is
//! there because the alternative is a control loop that fails quietly:
//!
//! * the program is started when the configuration is loaded, not at the
//!   first datum, so one that cannot be executed is a fault found on the
//!   ground rather than three days into a mission.
//! * every result has a deadline. `timeout` is required -- there is no
//!   sensible default for how long a computation may take -- and a program
//!   that has not answered within it is treated as having failed, not waited
//!   for.
//! * a failed program does not silently swallow data. Its link is refused and
//!   the refusal is counted and named, so the ground sees a computation stop
//!   rather than seeing plausible numbers that are stale.
//! * restarting is bounded and declared, and `never` is the default. A heater
//!   driven by a computation that restarts under load is worse than a heater
//!   driven by nothing: the second is obviously broken.
//!
//! # What the program sees
//!
//! A line per arrival on its standard input, and a line per answer on its
//! standard output. Lines, because a value on a link is decimal text and a
//! line is what every language reads without a library.
//!
//! ```text
//!   in:  293.15          an arrival on a link that names no input
//!   in:  in1=293.15      an arrival on a link that names one
//!   out: 291.0           a result, which is passed on
//!   out:                 an empty line: nothing to send yet
//! ```
//!
//! The empty line is not decoration. A computation over several inputs has to
//! be able to say "not yet" -- an external mean over three sensors answers on
//! the third arrival and not before -- and a program that answered something
//! on every line would have to invent a value twice per pass. It is the same
//! answer a built-in gives by handing back `None`.
//!
//! Standard error is left alone, so whatever the program complains about
//! arrives wherever tcspecial's own complaints do.

use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use nix::poll::{poll, PollFd, PollFlags};

use crate::runtime::node_state::Abandon;
use tcslibgs::node::vehicle::compute::{ProcessParams, Restart};

use super::Function;

/// The program, running.
///
/// Its two pipes and whatever has arrived on the output pipe but is not yet a
/// whole line. The partial line is kept because a pipe hands over whatever has
/// been written so far: a program that writes a result in two writes is a
/// program whose answer arrives in two reads, and throwing away the first half
/// would lose the answer and then blame the program for not giving one.
#[derive(Debug)]
struct Running {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    partial: Vec<u8>,
}

/// A computation run as a program.
#[derive(Debug)]
pub struct Process {
    executable: String,
    timeout: Duration,
    restart: Restart,
    /// The program while it is running.
    running: Option<Running>,
    /// How many times it has been restarted.
    restarts: u32,
    /// Why it is not running, once it is not.
    ///
    /// Held so that every refused datum says the same thing, rather than the
    /// first one saying why and the rest saying nothing.
    stopped: Option<String>,
    /// What ends the wait for an answer, besides the answer and the deadline.
    ///
    /// The node's own, so that disabling the node abandons a wait it is in the
    /// middle of: a deadline may be seconds, and a disable that waited one out
    /// is a disable that arrives seconds late. See
    /// [`crate::runtime::node_state::Abandon`].
    abandon: Abandon,
}

impl Process {
    /// The program these settled parameters name, not yet started.
    ///
    /// Nothing to refuse: what a file may say -- the program, the deadline it
    /// cannot do without, and whether it may be restarted -- is
    /// `tcslibgs::node::vehicle::compute`'s, where the ground tool checks it. What is
    /// found here is the machine's answer: see [`Process::start`].
    pub fn of(params: ProcessParams) -> Process {
        Process {
            executable: params.executable,
            timeout: params.timeout,
            restart: params.restart,
            running: None,
            restarts: 0,
            stopped: None,
            abandon: Abandon::nothing(),
        }
    }

    /// What ends the wait for an answer besides the answer and the deadline.
    ///
    /// Given when the node is opened. A process given none waits out its
    /// deadline, which is what one in a test does.
    pub fn abandoned_for(mut self, abandon: Abandon) -> Process {
        self.abandon = abandon;
        self
    }

    /// Start the program.
    ///
    /// Called when the configuration is loaded and not at the first datum, so
    /// that a program which cannot be executed is a fault found on the ground
    /// rather than three days into a mission. A failure here is the
    /// configuration's: the file names a program this machine cannot run.
    pub fn start(&mut self) -> Result<(), String> {
        self.running = Some(self.spawn()?);
        Ok(())
    }

    /// Whether the program is running.
    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    /// Start the program, whatever happened to the last one.
    fn spawn(&self) -> Result<Running, String> {
        let mut child = Command::new(&self.executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Left alone: whatever the program complains about should arrive
            // wherever this program's own complaints do.
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("\"{}\" cannot be run: {e}", self.executable))?;

        // Both pipes are taken out of the child, which is why they were asked
        // for; a child without them is one nothing could be said to.
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| format!("\"{}\" has no standard input", self.executable))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| format!("\"{}\" has no standard output", self.executable))?;

        Ok(Running {
            child,
            stdin,
            stdout,
            partial: Vec::new(),
        })
    }

    /// Hand one line over and wait for the answer, or say what went wrong.
    ///
    /// The deadline is measured from here and not from the write: a program
    /// that takes its `timeout` to answer has used all of it, whatever the
    /// writing cost.
    fn ask(&mut self, line: &str) -> Result<Option<Vec<u8>>, String> {
        let deadline = Instant::now() + self.timeout;
        let Some(running) = self.running.as_mut() else {
            return Err("it is not running".to_string());
        };

        running
            .stdin
            .write_all(line.as_bytes())
            .and_then(|()| running.stdin.flush())
            .map_err(|e| format!("it would not take what arrived: {e}"))?;

        loop {
            // A whole line already in hand, from a read that brought two.
            if let Some(answer) = take_line(&mut running.partial) {
                return Ok(answer);
            }

            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(format!(
                    "it did not answer within {:?}, so it is treated as having \
                     failed rather than waited for",
                    self.timeout
                ));
            }

            // SAFETY: both descriptors are owned by something that outlives
            // the call -- the child's pipe by `running`, the node's wakeup by
            // the Arc inside `abandon` -- and neither is kept.
            let borrowed =
                unsafe { std::os::fd::BorrowedFd::borrow_raw(running.stdout.as_raw_fd()) };
            let woken = self
                .abandon
                .io_fd()
                .map(|fd| unsafe { std::os::fd::BorrowedFd::borrow_raw(fd) });
            let mut fds = vec![PollFd::new(&borrowed, PollFlags::POLLIN)];
            if let Some(woken) = woken.as_ref() {
                fds.push(PollFd::new(woken, PollFlags::POLLIN));
            }
            let waiting = left.as_millis().min(i32::MAX as u128) as i32;
            match poll(&mut fds, waiting) {
                Ok(0) => continue,
                Ok(_) => {}
                Err(e) => return Err(format!("waiting for its answer failed: {e}")),
            }

            // The node has been disabled. The wait is given up where it
            // stands and no answer is produced: not an error, because
            // stopping the node is what the ground asked for, and the program
            // is put down by the disabling itself. What it must not do is
            // wait out the rest of a deadline that may be seconds long.
            if fds
                .get(1)
                .and_then(|fd| fd.revents())
                .is_some_and(|events| events.contains(PollFlags::POLLIN))
            {
                return Ok(None);
            }

            let mut buffer = [0u8; 256];
            match running.stdout.read(&mut buffer) {
                // The far end of a pipe closed: the program has gone, or is
                // going. Which is reported, with whatever it exited with.
                Ok(0) => return Err(exit_of(&mut running.child)),
                Ok(n) => running.partial.extend_from_slice(&buffer[..n]),
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(format!("its answer could not be read: {e}")),
            }
        }
    }

    /// What program this runs.
    pub fn executable(&self) -> &str {
        &self.executable
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn restart(&self) -> Restart {
        self.restart
    }

    /// Whether it is started again when it stops.
    ///
    /// The answer the configuration gave, and the same answer every time: a
    /// program told to come back comes back however often it stops. What the
    /// count below is for is the telemetry -- how many times this one has --
    /// and not a bound on it.
    pub fn may_restart(&self) -> bool {
        match self.restart {
            Restart::Never => false,
            Restart::WhenItStops => true,
        }
    }

    /// Record that it stopped, and say whether it will be started again.
    pub fn stopped(&mut self, why: &str) -> bool {
        if self.may_restart() {
            self.restarts += 1;
            self.stopped = None;
            true
        } else {
            self.stopped = Some(why.to_string());
            false
        }
    }

    /// Why it is not running, if it is not.
    pub fn why_stopped(&self) -> Option<&str> {
        self.stopped.as_deref()
    }

    /// Kill whatever is running and wait for it.
    ///
    /// Waited for, which is the part that matters: a child that is killed and
    /// not waited for is a zombie, and a vehicle that restarted a computation
    /// a few times would collect one per restart.
    fn put_down(&mut self) {
        if let Some(mut running) = self.running.take() {
            let _ = running.child.kill();
            let _ = running.child.wait();
        }
    }
}

// What the child does not inherit.
//
// Everything this process opens, it opens close-on-exec: the standard
// library does that for files, sockets and pipes, and the endpoints that
// open their own descriptors ask for it. So a computation starts with its
// two pipes and the standard error it shares, and not with the command
// socket, the chip another node holds or the log's segment file -- none of
// which it has any business with, and one of which, a chip's line request,
// is given to a single owner at a time.
//
// Closing them in the child by hand was tried and taken out again. The
// descriptor the standard library reports a failed exec on is one of the
// ones a sweep like that closes, and with it closed a program that cannot be
// run looks like a program that started -- which is the one failure this
// node exists to make visible.

/// One whole line out of what has arrived, if there is one.
///
/// `Some(None)` is an empty line, which is the program saying "nothing to send
/// yet"; `Some(Some(bytes))` is a result. `None` is "no whole line yet", which
/// is what makes the caller wait rather than guess. A trailing carriage return
/// is dropped, so a program written on a machine that writes them is not
/// refused for the one byte.
fn take_line(buffer: &mut Vec<u8>) -> Option<Option<Vec<u8>>> {
    let end = buffer.iter().position(|b| *b == b'\n')?;
    let mut line: Vec<u8> = buffer.drain(..=end).collect();
    line.pop();
    if line.last() == Some(&b'\r') {
        line.pop();
    }

    if line.is_empty() {
        Some(None)
    } else {
        Some(Some(line))
    }
}

/// What a program that has gone exited with, said in a sentence.
///
/// Asked after its pipe closed, which is how this finds out: a program is not
/// waited for while it is working, so its end is noticed when it stops talking
/// and the status is read afterwards.
fn exit_of(child: &mut Child) -> String {
    match child.wait() {
        Ok(status) => match status.code() {
            Some(0) => "it closed its output and exited cleanly".to_string(),
            Some(code) => format!("it exited with status {code}"),
            // Killed by a signal, which on Linux is what the status says
            // instead of a code.
            None => format!("it was killed ({status})"),
        },
        Err(e) => format!("it stopped and could not be waited for: {e}"),
    }
}

impl Function for Process {
    /// Hand one arrival to the program, and pass on what it answers.
    ///
    /// Every failure here ends the same way: the program is put down, the
    /// restart allowance is spent or the node is stopped for good, and the
    /// datum is refused with the reason. Nothing is swallowed. A computation
    /// that has stopped must look stopped, because the alternative is a
    /// control loop acting on numbers that stopped changing.
    fn give(&mut self, input: Option<&str>, data: &[u8]) -> Result<Option<Vec<u8>>, String> {
        if let Some(why) = self.why_stopped() {
            return Err(format!("\"{}\" is not running: {why}", self.executable));
        }

        // Not running and not stopped is a node nobody started, which the
        // loading should have done. Started here rather than refused, since a
        // program that can be started is better than a link that is refused
        // for the sake of where it was started from.
        if self.running.is_none() {
            self.start()?;
        }

        // What the program reads: the value as a decimal, and the input's name
        // where the link gave one. See the module comment.
        //
        // The value arrives as four bytes of micro-kelvin -- see
        // `super::as_value` -- and goes to the program as text, because the
        // far end is a program reading lines. This is the one boundary where a
        // value is written out and read back, and both halves of it are here:
        // the number this writes and the number the answer is read as.
        let datum = super::as_text(super::from_value(data, "what arrived")?);
        let line = match input {
            Some(input) => format!("{input}={datum}\n"),
            None => format!("{datum}\n"),
        };

        match self.ask(&line) {
            // The program's answer is a number it wrote as text, and what
            // leaves this node is a value like any other.
            Ok(Some(answer)) => {
                let said = std::str::from_utf8(&answer).map_err(|_| {
                    format!(
                        "\"{}\" answered with something that is not text",
                        self.executable
                    )
                })?;
                let number = super::from_text(said, &format!("\"{}\"", self.executable))?;
                Ok(Some(super::as_value(number)?))
            }
            Ok(None) => Ok(None),
            Err(why) => {
                // Put down whatever is left of it before deciding: a program
                // that missed its deadline is still running, and a second
                // datum handed to it would be answered with the first datum's
                // result.
                self.put_down();

                let again = self.stopped(&why);
                if again {
                    // The allowance paid for a restart, which is attempted
                    // now rather than at the next datum: a vehicle is better
                    // off learning that the program will not start again
                    // while something is still watching the pass that killed
                    // it.
                    if let Err(e) = self.start() {
                        self.stopped(&e);
                        return Err(format!(
                            "\"{}\" {why}, and would not start again: {e}",
                            self.executable
                        ));
                    }
                    return Err(format!(
                        "\"{}\" {why}; it has been started again, and this datum is \
                         lost",
                        self.executable
                    ));
                }

                Err(format!("\"{}\" {why}", self.executable))
            }
        }
    }

    fn name(&self) -> &'static str {
        "process"
    }
}

/// Nothing of this is left running when the node goes.
///
/// A vehicle that stopped and left a computation behind would be a vehicle
/// whose next start finds a program already holding whatever the last one held.
/// Killed rather than asked to stop: there is no protocol for asking, and a
/// program reading a pipe that closed will see that anyway.
impl Drop for Process {
    fn drop(&mut self) {
        self.put_down();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // What a program is handed and what it answers with: a value on the link
    // and a decimal in the pipe. See `give`.
    use crate::node::vehicle::compute::{as_value, from_value};

    /// A temperature as it arrives on a link: four bytes of micro-kelvin.
    ///
    /// The refusal is `as_value`'s range, which these temperatures are all
    /// well inside; a test that meant to try the range says so.
    fn kelvin(number: f64) -> Vec<u8> {
        as_value(number).expect("a temperature a value carries")
    }

    /// A program of these parameters, not started.
    ///
    /// What a file may say about one -- that the program and the deadline are
    /// required, that a restart allowance is a number or "never" -- is
    /// `tcslibgs::node::vehicle::compute`'s rule, where the ground tool applies it.
    /// These tests are about what the supervision does.
    fn process(executable: &str, timeout: Duration, restart: Restart) -> Process {
        Process::of(ProcessParams {
            executable: executable.to_string(),
            timeout,
            restart,
        })
    }

    /// A program told nothing is a program left stopped.
    #[test]
    fn restarting_is_never_unless_it_is_asked_for() {
        let p = process("/usr/bin/mean", Duration::from_millis(500), Restart::Never);
        assert_eq!(p.restart(), Restart::Never);
        assert!(!p.may_restart());

        let p = process(
            "/usr/bin/mean",
            Duration::from_millis(500),
            Restart::WhenItStops,
        );
        assert_eq!(p.restart(), Restart::WhenItStops);
        assert!(p.may_restart());
    }

    /// A program told to come back comes back every time it stops.
    ///
    /// The answer the configuration gave, and the same answer however often
    /// it is asked: `restart` is whether, not how many times. A program that
    /// stops as fast as it is started is a program somebody has to look at,
    /// and what says so is the count in the telemetry rather than a bound
    /// that quietly leaves it stopped.
    #[test]
    fn restarting_is_not_bounded() {
        let mut p = process(
            "/usr/bin/mean",
            Duration::from_millis(500),
            Restart::WhenItStops,
        );

        for which in 1..=5 {
            assert!(p.stopped("it exited"), "restart {which} is allowed");
        }
        assert_eq!(
            p.why_stopped(),
            None,
            "it is coming back, so it is not stopped for good"
        );
    }

    /// A program that will not restart holds why, so every refusal says it.
    #[test]
    fn a_stopped_program_keeps_its_reason() {
        let mut p = process("/usr/bin/mean", Duration::from_millis(500), Restart::Never);

        assert_eq!(p.why_stopped(), None, "it has not stopped yet");
        assert!(!p.stopped("it could not be executed"));
        assert_eq!(p.why_stopped(), Some("it could not be executed"));
    }

    /// A program written for this test, in whatever shell there is.
    ///
    /// A shell script rather than a Rust helper binary: what is being tested
    /// is the arrangement a configuration can actually name, which is a
    /// program on this machine reading lines and writing them.
    fn program(script: &str) -> (tempfile::TempDir, String) {
        use std::io::Write as _;
        use std::os::unix::fs::PermissionsExt;

        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("computation");
        let mut file = std::fs::File::create(&path).expect("the program is written");
        write!(file, "#!/bin/sh\n{script}").expect("the program is written");
        file.flush().expect("flushed");
        drop(file);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("and made runnable");

        let name = path.to_str().expect("a path").to_string();
        (where_, name)
    }

    fn running(script: &str, timeout: Duration) -> (tempfile::TempDir, Process) {
        let (where_, name) = program(script);
        let mut p = process(&name, timeout, Restart::Never);
        start_despite_the_race(&mut p);
        assert!(p.is_running());
        (where_, p)
    }

    /// Start it, past a race these tests have and a vehicle does not.
    ///
    /// A program just written by this process cannot be executed while any
    /// process holds it open for writing, and in a multi-threaded program
    /// every spawn briefly hands every open descriptor to a child between the
    /// fork and the exec. So one test writing its program while another spawns
    /// can see `ETXTBSY` from the kernel. Nothing in flight writes the program
    /// it runs, which is why the retry is here and not in `Process::start`:
    /// there, a text file busy is news.
    fn start_despite_the_race(p: &mut Process) {
        for attempt in 1..=50 {
            match p.start() {
                Ok(()) => return,
                Err(why) if why.contains("Text file busy") && attempt < 50 => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(why) => panic!("the program starts: {why}"),
            }
        }
    }

    /// A line in, a line out: what the program is for.
    #[test]
    fn a_program_answers_what_it_is_handed() {
        // Doubles whatever arrives, which is arithmetic enough to tell an
        // answer from an echo.
        let (_where, mut p) = running(
            "while read line; do echo $(( $line * 2 )); done\n",
            Duration::from_secs(2),
        );

        assert_eq!(answered(&mut p, 21.0), 42.0);
        assert_eq!(answered(&mut p, 50.0), 100.0);
    }

    /// What a program answered, as a number.
    ///
    /// The value goes in as eight bytes, reaches the program as a decimal,
    /// and its answer comes back as eight bytes again: see `give`.
    fn answered(p: &mut Process, number: f64) -> f64 {
        let out = p.give(None, &kelvin(number)).expect("an answer");
        from_value(&out.expect("a value"), "the program").expect("a value")
    }

    /// A named input reaches the program as its name and its value.
    ///
    /// The program answers 1 for a line that begins with the name and 0 for
    /// one that does not, because what it answers with is a number: the line
    /// itself is not one, and a program that echoed it would be answering
    /// with something this cannot read as a value.
    #[test]
    fn a_named_input_arrives_named() {
        let (_where, mut p) = running(
            "while read line; do case \"$line\" in in1=293.15) echo 1;; *) echo 0;; esac; \
             done\n",
            Duration::from_secs(2),
        );

        let out = p
            .give(Some("in1"), &kelvin(293.15))
            .expect("an answer")
            .expect("a value");
        assert_eq!(
            from_value(&out, "the program").expect("a value"),
            1.0,
            "the program should see which input this is, and its value"
        );
    }

    /// An empty line is the program saying "nothing yet".
    ///
    /// Which is what lets a computation over several inputs be written as a
    /// program at all: an external mean over three sensors answers on the
    /// third arrival and not before.
    #[test]
    fn an_empty_line_is_nothing_to_send_yet() {
        let (_where, mut p) = running(
            "n=0\nwhile read line; do n=$(( $n + 1 )); \
             if [ $n -eq 3 ]; then echo $n; else echo; fi; done\n",
            Duration::from_secs(2),
        );

        assert_eq!(p.give(None, &kelvin(1.0)).expect("a pass"), None);
        assert_eq!(p.give(None, &kelvin(2.0)).expect("a pass"), None);
        assert_eq!(
            answered(&mut p, 3.0),
            3.0,
            "the third arrival is the one that answers"
        );
    }

    /// A program that answers in two writes still answers once.
    #[test]
    fn an_answer_split_across_writes_is_one_answer() {
        let (_where, mut p) = running(
            "while read line; do printf '29'; sleep 0.2; printf '1.5\\n'; done\n",
            Duration::from_secs(3),
        );

        assert_eq!(answered(&mut p, 1.0), 291.5);
    }

    /// A program that does not answer in time is treated as having failed.
    #[test]
    fn a_program_that_misses_its_deadline_has_failed() {
        let (_where, mut p) = running(
            "while read line; do sleep 5; echo late; done\n",
            Duration::from_millis(200),
        );

        let said = p
            .give(None, &kelvin(1_f64))
            .expect_err("a program that took too long");
        assert!(
            said.contains("did not answer within") && said.contains("rather than waited for"),
            "{said}"
        );

        // And it is put down rather than left running with an answer nobody
        // will read: the next datum is refused with the same reason, so a
        // ground station sees a computation that has stopped.
        assert!(!p.is_running());
        let again = p
            .give(None, &kelvin(2_f64))
            .expect_err("a stopped computation");
        assert!(again.contains("is not running"), "{again}");
    }

    /// A program that exits is reported with what it exited with.
    #[test]
    fn a_program_that_exits_is_reported() {
        let (_where, mut p) = running("read line\nexit 3\n", Duration::from_secs(2));

        let said = p
            .give(None, &kelvin(1_f64))
            .expect_err("a program that exited");
        assert!(said.contains("exited with status 3"), "{said}");
        assert!(!p.is_running());
    }

    /// A program a file names and this machine cannot run is a load-time
    /// fault.
    #[test]
    fn a_program_that_cannot_be_run_is_refused_at_the_start() {
        let mut p = process(
            "/nonesuch/computation",
            Duration::from_millis(500),
            Restart::Never,
        );

        let said = p.start().expect_err("a program that is not there");
        assert!(
            said.contains("/nonesuch/computation") && said.contains("cannot be run"),
            "{said}"
        );
        assert!(!p.is_running());
    }

    /// A restart happens at once, and the datum that died with the program is
    /// still lost.
    #[test]
    fn a_restart_is_taken_and_the_datum_is_still_lost() {
        let (_where, name) = program("read line\nexit 1\n");
        let mut p = process(&name, Duration::from_secs(2), Restart::WhenItStops);
        start_despite_the_race(&mut p);

        // The first datum kills it, and it is started again. The datum is
        // refused all the same: it went into a program that died without
        // answering, and saying otherwise would be inventing a result.
        let said = p
            .give(None, &kelvin(1_f64))
            .expect_err("the program exited");
        assert!(
            said.contains("started again") && said.contains("this datum is lost"),
            "{said}"
        );
        assert!(p.is_running(), "it should have been started again");

        // And the second the same way: a program told to come back comes back
        // however often it stops, and every datum that died with it is lost.
        let said = p
            .give(None, &kelvin(2_f64))
            .expect_err("the program exited again");
        assert!(
            said.contains("started again") && said.contains("this datum is lost"),
            "{said}"
        );
        assert!(p.is_running(), "it should have been started again");
    }

    /// Whole lines are taken from what has arrived, and nothing else is.
    #[test]
    fn a_line_is_taken_only_when_it_is_whole() {
        let mut buffer = b"291".to_vec();
        assert_eq!(take_line(&mut buffer), None, "no newline yet");
        assert_eq!(buffer, b"291", "and nothing is taken");

        buffer.extend_from_slice(b".5\n292.0\n");
        assert_eq!(take_line(&mut buffer), Some(Some(b"291.5".to_vec())));
        assert_eq!(take_line(&mut buffer), Some(Some(b"292.0".to_vec())));
        assert_eq!(take_line(&mut buffer), None);

        // An empty line is an answer of its own, and a carriage return before
        // the newline is not part of one.
        let mut buffer = b"\n293.15\r\n".to_vec();
        assert_eq!(take_line(&mut buffer), Some(None));
        assert_eq!(take_line(&mut buffer), Some(Some(b"293.15".to_vec())));
    }
}
