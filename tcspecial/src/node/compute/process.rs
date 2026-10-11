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

use std::collections::BTreeMap;
use std::time::Duration;

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

/// A computation run as a program.
#[derive(Debug)]
pub struct Process {
    program: String,
    timeout: Duration,
    restart: Restart,
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
            restarts: 0,
            stopped: None,
        })
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
}

impl Function for Process {
    fn give(&mut self, _input: Option<&str>, _data: &[u8]) -> Result<Option<Vec<u8>>, String> {
        // The supervision -- spawning, writing, reading with a deadline,
        // noticing an exit -- is not written yet, and this says so rather
        // than pretending. A node configured for it is refused at startup by
        // the same path a program that cannot be executed is refused by, so
        // nothing reaches here in flight.
        Err(format!(
            "running \"{}\" as a computation is specified and not yet built; \
             the built-in functions are what works today",
            self.program
        ))
    }

    fn name(&self) -> &'static str {
        "process"
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

    /// Running one says it is not built rather than pretending.
    #[test]
    fn running_a_program_says_it_is_not_built_yet() {
        let mut p = Process::of(&attributes(&[
            ("program", "/usr/bin/mean"),
            ("timeout", "500ms"),
        ]))
        .unwrap();

        let said = p.give(None, b"1").expect_err("not built");
        assert!(said.contains("not yet built"), "{said}");
        assert!(said.contains("/usr/bin/mean"), "{said}");
    }
}
