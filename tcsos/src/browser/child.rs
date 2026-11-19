//! A supervisor's children, in a page: tcspecial starting itself again is
//! one more program on a thread of its own -- see
//! [`crate::browser::process`] -- and what a stop signal is on Linux is a
//! program's stop flag, passed on to its child.

use std::ffi::CStr;
use std::fmt;
use std::time::Duration;

use crate::browser::process::{Child as Program, Command};

/// A child this program started.
#[derive(Debug, Clone)]
pub struct Child(Program);

impl fmt::Display for Child {
    /// Shows the child by its number, as a log names a process.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.id())
    }
}

/// How a child ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// It exited with this status.
    Exited(i32),
    /// A signal ended it: never, in a page.
    Signalled(i32),
}

impl Ended {
    /// Whether a stop signal ended it: never, in a page, where a stopped
    /// program ends by returning.
    ///
    /// # Returns
    ///
    /// `false`.
    pub fn by_a_stop(self) -> bool {
        false
    }
}

/// Does nothing: there are no signals to block.
pub fn block_stops_and_children() {}

/// What every child is started with: this program, its arguments, and its
/// variables with one more.
#[derive(Debug)]
pub struct Spawner {
    /// This program's arguments, its name first.
    args: Vec<String>,
    /// The variable every child is given, and its value.
    variable: (String, String),
    /// The argument a child may be started with as well.
    other_arg: String,
}

impl Spawner {
    /// Makes what every child is started with.
    ///
    /// # Arguments
    ///
    /// * `variable`  - The variable every child is given, which tells it
    ///                 it is one; its value is this program's number.
    /// * `other_arg` - An argument a child may be started with, after the
    ///                 program's name, as well as its own.
    ///
    /// # Returns
    ///
    /// The spawner.
    ///
    /// # Errors
    ///
    /// None, in a page.
    pub fn new(variable: &str, other_arg: &str) -> Result<Spawner, String> {
        Ok(Spawner {
            args: crate::browser::env::args().collect(),
            variable: (
                variable.to_string(),
                crate::browser::process::id().to_string(),
            ),
            other_arg: other_arg.to_string(),
        })
    }

    /// Starts a child.
    ///
    /// # Arguments
    ///
    /// * `other` - Whether the child is given the other argument.
    ///
    /// # Returns
    ///
    /// The child.
    ///
    /// # Errors
    ///
    /// Why it could not be started.
    pub fn start(&self, other: bool) -> std::io::Result<Child> {
        let mut command = Command::new(&self.args[0]);
        if other {
            command.arg(&self.other_arg);
        }
        command
            .args(&self.args[1..])
            .env(&self.variable.0, &self.variable.1);
        Ok(Child(command.spawn()?))
    }
}

/// Waits for a child to end, passing a stop asked of this program on to
/// it.
///
/// # Arguments
///
/// * `child`    - The child.
/// * `look`     - How long each wait for a stop lasts before it looks
///                again whether the child has ended.
/// * `stopping` - Set where a stop was taken, and passed on.
///
/// # Returns
///
/// How it ended.
pub fn wait_for(child: &Child, look: Duration, stopping: &mut bool) -> Ended {
    let mut child = child.0.clone();
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return Ended::Exited(status.code().unwrap_or(101));
        }
        if crate::browser::signals::take(look).is_some() {
            *stopping = true;
            let _ = child.kill();
        }
    }
}

/// Waits for a time, unless a stop comes first.
///
/// # Arguments
///
/// * `delay` - How long.
///
/// # Returns
///
/// Whether a stop came.
pub fn pause(delay: Duration) -> bool {
    crate::browser::signals::take(delay).is_some()
}

/// Whether the parent is still the one it should be: always, in a page,
/// where a program's parent cannot end without the page.
///
/// # Arguments
///
/// * `_parent` - The parent's number.
///
/// # Returns
///
/// `true`.
pub fn follow_parent(_parent: Option<u32>) -> bool {
    true
}

/// The signals a supervisor's tests name, by what they are.
pub mod signal {
    /// SIGINT's number on Linux.
    pub const INTERRUPT: i32 = 2;
    /// SIGTERM's.
    pub const TERMINATE: i32 = 15;
    /// SIGSEGV's.
    pub const SEGMENTATION: i32 = 11;
    /// SIGKILL's.
    pub const KILL: i32 = 9;
}

/// How a child is ended when its parent is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// As SIGTERM would.
    Terminate,
    /// As SIGKILL would.
    Kill,
}

/// Has the child asked to stop when the program that starts it ends, as
/// the kernel's parent-death signal does on Linux; and a wait for its
/// parent waits for it too. Asked, either way the ending says: a thread
/// cannot be killed, only asked.
///
/// # Arguments
///
/// * `command` - The command, not yet spawned.
/// * `_ending` - What the child would be sent.
/// * `_gone`   - What its start would fail with, its parent gone.
pub fn dies_with_parent(
    command: &mut Command,
    _ending: Ending,
    _gone: &'static str,
) {
    command.die_with_parent();
}

/// Runs a program with one argument and waits for it, where the page has
/// it.
///
/// # Arguments
///
/// * `program` - The program.
/// * `arg`     - Its one argument.
///
/// # Returns
///
/// What it ended with.
///
/// # Errors
///
/// `NotFound` where the page has no such program.
pub fn run(program: &CStr, arg: &CStr) -> std::io::Result<i32> {
    let status = Command::new(&*program.to_string_lossy())
        .arg(&*arg.to_string_lossy())
        .status()?;
    Ok(status.code().unwrap_or(101))
}
