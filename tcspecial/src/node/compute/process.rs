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

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use nix::poll::{poll, PollFd, PollFlags};
use tcslibgs::Scalar;

use super::Function;

/// What to do when the program stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restart {
    /// Leave it stopped and refuse the link. The default.
    Never,
    /// Start it again, up to this many times.
    UpTo(u32),
}

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
    program: String,
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
}

impl Process {
    pub fn of(attributes: &BTreeMap<String, Scalar>) -> Result<Process, String> {
        let program = attributes
            .get("program")
            .map(|p| p.as_str().to_string())
            .ok_or_else(|| {
                "program is not set: a compute node whose function is \"process\" \
                 is the program it names"
                    .to_string()
            })?;

        // Required, with no default. How long a computation may take is a
        // property of the computation, and a default would be this software
        // guessing at somebody else's arithmetic -- then treating a slow
        // answer as a failure on the strength of the guess.
        let timeout = attributes
            .get("timeout")
            .ok_or_else(|| {
                "timeout is not set, and there is no default for it: how long a \
                 computation may take is a property of the computation"
                    .to_string()
            })
            .and_then(|t| {
                tcslibgs::graph::parse_interval(t.as_str())
                    .map_err(|why| format!("timeout: {why}"))
            })?;

        let restart = match attributes.get("restart").map(Scalar::as_str) {
            None | Some("never") => Restart::Never,
            Some(other) => other
                .parse::<u32>()
                .map(Restart::UpTo)
                .map_err(|_| {
                    format!(
                        "restart: \"{other}\" is neither \"never\" nor a number \
                         of times"
                    )
                })?,
        };

        Ok(Process {
            program,
            timeout,
            restart,
            running: None,
            restarts: 0,
            stopped: None,
        })
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
        let mut child = Command::new(&self.program)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Left alone: whatever the program complains about should arrive
            // wherever this program's own complaints do.
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("\"{}\" cannot be run: {e}", self.program))?;

        // Both pipes are taken out of the child, which is why they were asked
        // for; a child without them is one nothing could be said to.
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| format!("\"{}\" has no standard input", self.program))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| format!("\"{}\" has no standard output", self.program))?;

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

            // SAFETY: a descriptor the child's pipe owns, borrowed for the
            // call and not kept.
            let borrowed = unsafe {
                std::os::fd::BorrowedFd::borrow_raw(running.stdout.as_raw_fd())
            };
            let mut fds = [PollFd::new(&borrowed, PollFlags::POLLIN)];
            let waiting = left.as_millis().min(i32::MAX as u128) as i32;
            match poll(&mut fds, waiting) {
                Ok(0) => continue,
                Ok(_) => {}
                Err(e) => return Err(format!("waiting for its answer failed: {e}")),
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
    pub fn program(&self) -> &str {
        &self.program
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn restart(&self) -> Restart {
        self.restart
    }

    /// Whether another restart is allowed.
    pub fn may_restart(&self) -> bool {
        match self.restart {
            Restart::Never => false,
            Restart::UpTo(limit) => self.restarts < limit,
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

/// What the child does not inherit.
///
/// Everything this process opens, it opens close-on-exec: the standard
/// library does that for files, sockets and pipes, and the endpoints that
/// open their own descriptors ask for it. So a computation starts with its
/// two pipes and the standard error it shares, and not with the command
/// socket, the chip another node holds or the log's segment file -- none of
/// which it has any business with, and one of which, a chip's line request,
/// is given to a single owner at a time.
///
/// Closing them in the child by hand was tried and taken out again. The
/// descriptor the standard library reports a failed exec on is one of the
/// ones a sweep like that closes, and with it closed a program that cannot be
/// run looks like a program that started -- which is the one failure this
/// node exists to make visible.

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
            return Err(format!("\"{}\" is not running: {why}", self.program));
        }

        // Not running and not stopped is a node nobody started, which the
        // loading should have done. Started here rather than refused, since a
        // program that can be started is better than a link that is refused
        // for the sake of where it was started from.
        if self.running.is_none() {
            self.start()?;
        }

        // What the program reads: the value, and the input's name where the
        // link gave one. See the module comment.
        let datum = std::str::from_utf8(data)
            .map_err(|_| "what arrived is not text, and a line is text".to_string())?;
        let line = match input {
            Some(input) => format!("{input}={}\n", datum.trim_end()),
            None => format!("{}\n", datum.trim_end()),
        };

        match self.ask(&line) {
            Ok(answer) => Ok(answer),
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
                            self.program
                        ));
                    }
                    return Err(format!(
                        "\"{}\" {why}; it has been started again, and this datum is \
                         lost",
                        self.program
                    ));
                }

                Err(format!("\"{}\" {why}", self.program))
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

    fn attributes(pairs: &[(&str, &str)]) -> BTreeMap<String, Scalar> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), Scalar::of(*v)))
            .collect()
    }

    /// A program and a deadline are both required.
    #[test]
    fn a_program_and_a_deadline_are_both_required() {
        let said = Process::of(&attributes(&[("timeout", "500ms")]))
            .expect_err("no program");
        assert!(said.contains("program is not set"), "{said}");

        let said = Process::of(&attributes(&[("program", "/usr/bin/mean")]))
            .expect_err("no timeout");
        assert!(
            said.contains("no default for it"),
            "a default deadline would be this software guessing: {said}"
        );

        let said = Process::of(&attributes(&[
            ("program", "/usr/bin/mean"),
            ("timeout", "soon"),
        ]))
        .expect_err("an unreadable timeout");
        assert!(said.contains("timeout:"), "{said}");
    }

    /// Restarting is never, unless a number says otherwise.
    #[test]
    fn restarting_is_never_by_default() {
        let p = Process::of(&attributes(&[
            ("program", "/usr/bin/mean"),
            ("timeout", "500ms"),
        ]))
        .unwrap();
        assert_eq!(p.restart(), Restart::Never);
        assert!(!p.may_restart());

        let p = Process::of(&attributes(&[
            ("program", "/usr/bin/mean"),
            ("timeout", "500ms"),
            ("restart", "never"),
        ]))
        .unwrap();
        assert_eq!(p.restart(), Restart::Never);

        let p = Process::of(&attributes(&[
            ("program", "/usr/bin/mean"),
            ("timeout", "500ms"),
            ("restart", "3"),
        ]))
        .unwrap();
        assert_eq!(p.restart(), Restart::UpTo(3));
        assert!(p.may_restart());
    }

    /// A restart allowance is spent, and then it is gone.
    #[test]
    fn restarts_are_bounded() {
        let mut p = Process::of(&attributes(&[
            ("program", "/usr/bin/mean"),
            ("timeout", "500ms"),
            ("restart", "2"),
        ]))
        .unwrap();

        assert!(p.stopped("it exited"), "the first restart is allowed");
        assert!(p.stopped("it exited"), "and the second");
        assert!(!p.stopped("it exited"), "the third is not");
        assert_eq!(
            p.why_stopped(),
            Some("it exited"),
            "once it is stopped for good, it should say why"
        );
    }

    /// A program that will not restart holds why, so every refusal says it.
    #[test]
    fn a_stopped_program_keeps_its_reason() {
        let mut p = Process::of(&attributes(&[
            ("program", "/usr/bin/mean"),
            ("timeout", "500ms"),
        ]))
        .unwrap();

        assert_eq!(p.why_stopped(), None, "it has not stopped yet");
        assert!(!p.stopped("it could not be executed"));
        assert_eq!(p.why_stopped(), Some("it could not be executed"));
    }

    /// A restart allowance that is not a number is refused.
    #[test]
    fn an_unreadable_restart_allowance_is_refused() {
        let said = Process::of(&attributes(&[
            ("program", "/usr/bin/mean"),
            ("timeout", "500ms"),
            ("restart", "sometimes"),
        ]))
        .expect_err("not a number");
        assert!(said.contains("sometimes"), "{said}");
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

    fn running(script: &str, timeout: &str) -> (tempfile::TempDir, Process) {
        let (where_, name) = program(script);
        let mut p = Process::of(&attributes(&[("program", &name), ("timeout", timeout)]))
            .expect("the attributes");
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
            "2s",
        );

        assert_eq!(p.give(None, b"21").expect("an answer"), Some(b"42".to_vec()));
        assert_eq!(p.give(None, b"50").expect("an answer"), Some(b"100".to_vec()));
    }

    /// A named input reaches the program as its name and its value.
    #[test]
    fn a_named_input_arrives_named() {
        let (_where, mut p) = running("while read line; do echo \"$line\"; done\n", "2s");

        assert_eq!(
            p.give(Some("in1"), b"293.15").expect("an answer"),
            Some(b"in1=293.15".to_vec()),
            "the program should see which input this is"
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
             if [ $n -eq 3 ]; then echo ready; else echo; fi; done\n",
            "2s",
        );

        assert_eq!(p.give(None, b"1").expect("a pass"), None);
        assert_eq!(p.give(None, b"2").expect("a pass"), None);
        assert_eq!(
            p.give(None, b"3").expect("a pass"),
            Some(b"ready".to_vec()),
            "the third arrival is the one that answers"
        );
    }

    /// A program that answers in two writes still answers once.
    #[test]
    fn an_answer_split_across_writes_is_one_answer() {
        let (_where, mut p) = running(
            "while read line; do printf '29'; sleep 0.2; printf '1.5\\n'; done\n",
            "3s",
        );

        assert_eq!(
            p.give(None, b"go").expect("an answer"),
            Some(b"291.5".to_vec())
        );
    }

    /// A program that does not answer in time is treated as having failed.
    #[test]
    fn a_program_that_misses_its_deadline_has_failed() {
        let (_where, mut p) = running("while read line; do sleep 5; echo late; done\n", "200ms");

        let said = p.give(None, b"1").expect_err("a program that took too long");
        assert!(
            said.contains("did not answer within") && said.contains("rather than waited for"),
            "{said}"
        );

        // And it is put down rather than left running with an answer nobody
        // will read: the next datum is refused with the same reason, so a
        // ground station sees a computation that has stopped.
        assert!(!p.is_running());
        let again = p.give(None, b"2").expect_err("a stopped computation");
        assert!(again.contains("is not running"), "{again}");
    }

    /// A program that exits is reported with what it exited with.
    #[test]
    fn a_program_that_exits_is_reported() {
        let (_where, mut p) = running("read line\nexit 3\n", "2s");

        let said = p.give(None, b"1").expect_err("a program that exited");
        assert!(said.contains("exited with status 3"), "{said}");
        assert!(!p.is_running());
    }

    /// A program a file names and this machine cannot run is a load-time
    /// fault.
    #[test]
    fn a_program_that_cannot_be_run_is_refused_at_the_start() {
        let mut p = Process::of(&attributes(&[
            ("program", "/nonesuch/computation"),
            ("timeout", "500ms"),
        ]))
        .expect("the attributes read");

        let said = p.start().expect_err("a program that is not there");
        assert!(
            said.contains("/nonesuch/computation") && said.contains("cannot be run"),
            "{said}"
        );
        assert!(!p.is_running());
    }

    /// A restart allowance buys a restart, and the datum that spent it is
    /// still lost.
    #[test]
    fn a_restart_is_taken_and_the_datum_is_still_lost() {
        let (_where, name) = program("read line\nexit 1\n");
        let mut p = Process::of(&attributes(&[
            ("program", &name),
            ("timeout", "2s"),
            ("restart", "1"),
        ]))
        .expect("the attributes");
        start_despite_the_race(&mut p);

        // The first datum kills it, and the allowance starts it again. The
        // datum is refused all the same: it went into a program that died
        // without answering, and saying otherwise would be inventing a
        // result.
        let said = p.give(None, b"1").expect_err("the program exited");
        assert!(
            said.contains("started again") && said.contains("this datum is lost"),
            "{said}"
        );
        assert!(p.is_running(), "the allowance should have been spent on it");

        // The second kills it for good, there being no allowance left.
        let said = p.give(None, b"2").expect_err("the program exited again");
        assert!(!said.contains("started again"), "{said}");
        assert!(!p.is_running());
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
