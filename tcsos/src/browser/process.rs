//! Programs, in a page: each a function registered by name and run on a
//! thread of its own, with arguments, variables and a directory of its
//! own, in place of a process started from a file.
//!
//! tcs-moc starts tcs-sim and tcspecial; tcspecial's supervisor starts
//! tcspecial again as its child. On Linux each is a program in a file. In a
//! page, the page registers each program's entry point under the name it
//! would have in a directory of programs -- see [`register`] -- and
//! [`Command`] starts the one its path names, as the standard library's
//! would, on a thread: see [`crate::browser::context`] for what is the
//! program's own. Its standard streams are not the page's to give: what a
//! program is handed is its interfaces, by [`crate::browser::interfaces`].
//!
//! A program asked to stop is told as SIGTERM would tell it, through its
//! stop flag; one that will not stop cannot be killed, a thread being
//! nothing that can be, so [`Child::kill`] asks.

pub use std::process::{ExitCode, Termination};

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use crate::browser::context::{self, Program};
use crate::browser::thread::{Builder, JoinHandle};

/// A program's entry point: given its arguments, its name first, it runs
/// until it ends and says what it ended with.
pub type Entry = fn(Vec<String>) -> i32;

/// The programs the page can start, by name.
fn programs() -> &'static Mutex<BTreeMap<String, Entry>> {
    static PROGRAMS: OnceLock<Mutex<BTreeMap<String, Entry>>> = OnceLock::new();
    PROGRAMS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Registers a program the page can start, under the name its file would
/// have: `tcspecial`, `tcs-sim`. A path whose last part is the name starts
/// it, wherever the path says it is.
///
/// # Arguments
///
/// * `name`  - Its name.
/// * `entry` - Its entry point.
pub fn register(name: &str, entry: Entry) {
    if let Ok(mut programs) = programs().lock() {
        programs.insert(name.to_string(), entry);
    }
}

/// The program a path names, by its last part.
///
/// # Arguments
///
/// * `path` - The path.
///
/// # Returns
///
/// Its entry point, where one is registered.
fn entry_of(path: &Path) -> Option<Entry> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    programs().lock().ok()?.get(&name).copied()
}

/// Whether a path names a program the page can start.
///
/// # Arguments
///
/// * `path` - The path.
///
/// # Returns
///
/// `true` where one is registered under its last part.
pub fn is_registered(path: &Path) -> bool {
    entry_of(path).is_some()
}

/// Where a program's standard stream goes: nowhere, in a page.
#[derive(Debug)]
pub struct Stdio;

impl Stdio {
    /// To nothing.
    pub fn null() -> Stdio {
        Stdio
    }

    /// To where the starting program's goes: the page's console.
    pub fn inherit() -> Stdio {
        Stdio
    }

    /// To a pipe: a page has none, so to nothing.
    pub fn piped() -> Stdio {
        Stdio
    }
}

impl From<std::fs::File> for Stdio {
    fn from(_file: std::fs::File) -> Stdio {
        Stdio
    }
}

impl From<crate::browser::fs::File> for Stdio {
    fn from(_file: crate::browser::fs::File) -> Stdio {
        Stdio
    }
}

/// How a program ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitStatus(i32);

impl ExitStatus {
    /// Whether it ended well.
    ///
    /// # Returns
    ///
    /// `true` for nought.
    pub fn success(&self) -> bool {
        self.0 == 0
    }

    /// What it ended with.
    ///
    /// # Returns
    ///
    /// The status: always, a program in a page ending only by returning.
    pub fn code(&self) -> Option<i32> {
        Some(self.0)
    }
}

impl fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exit status: {}", self.0)
    }
}

/// What a program that ended said: nothing, a page having no pipes.
#[derive(Debug)]
pub struct Output {
    /// How it ended.
    pub status: ExitStatus,
    /// What it wrote to its standard output: nothing.
    pub stdout: Vec<u8>,
    /// What it wrote to its standard error: nothing.
    pub stderr: Vec<u8>,
}

/// How a program is to be started.
#[derive(Debug)]
pub struct Command {
    /// The program's path.
    program: PathBuf,
    /// Its arguments, after its name.
    args: Vec<String>,
    /// Its variables: the starting program's, as changed here.
    env: BTreeMap<String, String>,
    /// Its directory, where not the starting program's.
    cwd: Option<PathBuf>,
    /// The interfaces it is handed, by slot.
    slots: [Option<crate::browser::net::TcpStream>; 3],
}

impl Command {
    /// A program to start.
    ///
    /// # Arguments
    ///
    /// * `program` - Its path.
    ///
    /// # Returns
    ///
    /// The command, with the starting program's variables.
    pub fn new(program: impl AsRef<OsStr>) -> Command {
        Command {
            program: PathBuf::from(program.as_ref()),
            args: Vec::new(),
            env: crate::browser::env::vars().collect(),
            cwd: None,
            slots: [None, None, None],
        }
    }

    /// Adds an argument.
    ///
    /// # Arguments
    ///
    /// * `arg` - The argument.
    ///
    /// # Returns
    ///
    /// The command.
    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Command {
        self.args.push(arg.as_ref().to_string_lossy().into_owned());
        self
    }

    /// Adds arguments.
    ///
    /// # Arguments
    ///
    /// * `args` - The arguments.
    ///
    /// # Returns
    ///
    /// The command.
    pub fn args<I, S>(&mut self, args: I) -> &mut Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for arg in args {
            self.arg(arg);
        }
        self
    }

    /// Sets a variable.
    ///
    /// # Arguments
    ///
    /// * `name`  - Its name.
    /// * `value` - Its value.
    ///
    /// # Returns
    ///
    /// The command.
    pub fn env(
        &mut self,
        name: impl AsRef<OsStr>,
        value: impl AsRef<OsStr>,
    ) -> &mut Command {
        self.env.insert(
            name.as_ref().to_string_lossy().into_owned(),
            value.as_ref().to_string_lossy().into_owned(),
        );
        self
    }

    /// Sets variables.
    ///
    /// # Arguments
    ///
    /// * `vars` - Their names and values.
    ///
    /// # Returns
    ///
    /// The command.
    pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Command
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        for (name, value) in vars {
            self.env(name, value);
        }
        self
    }

    /// Takes a variable away.
    ///
    /// # Arguments
    ///
    /// * `name` - Its name.
    ///
    /// # Returns
    ///
    /// The command.
    pub fn env_remove(&mut self, name: impl AsRef<OsStr>) -> &mut Command {
        self.env.remove(name.as_ref().to_string_lossy().as_ref());
        self
    }

    /// Takes every variable away.
    ///
    /// # Returns
    ///
    /// The command.
    pub fn env_clear(&mut self) -> &mut Command {
        self.env.clear();
        self
    }

    /// Sets the program's directory.
    ///
    /// # Arguments
    ///
    /// * `dir` - The directory.
    ///
    /// # Returns
    ///
    /// The command.
    pub fn current_dir(&mut self, dir: impl AsRef<Path>) -> &mut Command {
        self.cwd = Some(crate::browser::fs::absolute(dir.as_ref()));
        self
    }

    /// Where its standard input comes from: nowhere, in a page.
    ///
    /// # Arguments
    ///
    /// * `_from` - Where.
    ///
    /// # Returns
    ///
    /// The command.
    pub fn stdin(&mut self, _from: impl Into<Stdio>) -> &mut Command {
        self
    }

    /// Where its standard output goes: nowhere, in a page.
    ///
    /// # Arguments
    ///
    /// * `_to` - Where.
    ///
    /// # Returns
    ///
    /// The command.
    pub fn stdout(&mut self, _to: impl Into<Stdio>) -> &mut Command {
        self
    }

    /// Where its standard error goes: nowhere, in a page.
    ///
    /// # Arguments
    ///
    /// * `_to` - Where.
    ///
    /// # Returns
    ///
    /// The command.
    pub fn stderr(&mut self, _to: impl Into<Stdio>) -> &mut Command {
        self
    }

    /// Hands it an interface in a slot: see
    /// [`crate::browser::interfaces::hand_over`].
    ///
    /// # Arguments
    ///
    /// * `slot`   - 0, 1 or 2.
    /// * `stream` - The program's end.
    pub(crate) fn hand(
        &mut self,
        slot: usize,
        stream: crate::browser::net::TcpStream,
    ) {
        self.slots[slot] = Some(stream);
    }

    /// The program's path.
    ///
    /// # Returns
    ///
    /// It.
    pub fn get_program(&self) -> &OsStr {
        self.program.as_os_str()
    }

    /// Starts the program, on a thread of its own.
    ///
    /// # Returns
    ///
    /// The program, running.
    ///
    /// # Errors
    ///
    /// `NotFound` where no program is registered under the path's last
    /// part, as a missing file would be.
    pub fn spawn(&mut self) -> io::Result<Child> {
        let Some(entry) = entry_of(&self.program) else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "{}: no program by that name is in this page",
                    self.program.display()
                ),
            ));
        };
        let mut args = vec![self.program.to_string_lossy().into_owned()];
        args.extend(self.args.iter().cloned());
        let cwd = self.cwd.clone().unwrap_or_else(|| {
            crate::browser::env::current_dir().unwrap_or_default()
        });
        let program = context::new_program(args.clone(), self.env.clone(), cwd);
        if let Ok(mut slots) = program.slots.lock() {
            for (to, from) in slots.iter_mut().zip(self.slots.iter_mut()) {
                *to = from.take();
            }
        }
        let started = program.clone();
        let name = self
            .program
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let handle = Builder::new().name(name).spawn(move || {
            context::enter(started, true);
            run(entry, args)
        })?;
        Ok(Child {
            program,
            handle: Arc::new(Mutex::new(Some(handle))),
            ended: Arc::new(Mutex::new(None)),
        })
    }

    /// Starts the program and waits for it to end.
    ///
    /// # Returns
    ///
    /// How it ended.
    ///
    /// # Errors
    ///
    /// As for [`Command::spawn`].
    pub fn status(&mut self) -> io::Result<ExitStatus> {
        self.spawn()?.wait()
    }

    /// Starts the program and waits for it to end, keeping what it said:
    /// nothing, in a page.
    ///
    /// # Returns
    ///
    /// How it ended.
    ///
    /// # Errors
    ///
    /// As for [`Command::spawn`].
    pub fn output(&mut self) -> io::Result<Output> {
        Ok(Output {
            status: self.status()?,
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
}

/// Runs a program's entry point, turning an [`exit`] from inside it into
/// what it ends with, as a process's exit would be.
///
/// # Arguments
///
/// * `entry` - The entry point.
/// * `args`  - Its arguments.
///
/// # Returns
///
/// What it ended with: 101 where it panicked, as a process's panic does.
fn run(entry: Entry, args: Vec<String>) -> i32 {
    match std::panic::catch_unwind(move || entry(args)) {
        Ok(code) => code,
        Err(why) => why.downcast_ref::<Exited>().map_or(101, |exited| exited.0),
    }
}

/// A program the page started, running or ended.
#[derive(Debug, Clone)]
pub struct Child {
    /// The program.
    program: Arc<Program>,
    /// Its thread, until it is joined.
    handle: Arc<Mutex<Option<JoinHandle<i32>>>>,
    /// How it ended, once it has.
    ended: Arc<Mutex<Option<ExitStatus>>>,
}

impl Child {
    /// Its number, as a process id would be.
    ///
    /// # Returns
    ///
    /// It.
    pub fn id(&self) -> u32 {
        self.program.id
    }

    /// Asks it to stop, as SIGTERM would: a thread cannot be killed.
    ///
    /// # Errors
    ///
    /// None.
    pub fn kill(&mut self) -> io::Result<()> {
        context::ask_to_stop(self.program.id);
        Ok(())
    }

    /// How it ended, where it has.
    ///
    /// # Returns
    ///
    /// Its status, or `None` while it runs.
    ///
    /// # Errors
    ///
    /// None.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        if let Some(ended) = *self.ended.lock().expect("a status") {
            return Ok(Some(ended));
        }
        let finished = self
            .handle
            .lock()
            .expect("a handle")
            .as_ref()
            .is_some_and(|handle| handle.is_finished());
        if finished {
            return self.wait().map(Some);
        }
        Ok(None)
    }

    /// Waits for it to end.
    ///
    /// # Returns
    ///
    /// How it ended.
    ///
    /// # Errors
    ///
    /// None.
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        if let Some(ended) = *self.ended.lock().expect("a status") {
            return Ok(ended);
        }
        let handle = self.handle.lock().expect("a handle").take();
        let status = match handle {
            Some(handle) => ExitStatus(handle.join().unwrap_or(101)),
            None => loop {
                if let Some(ended) = *self.ended.lock().expect("a status") {
                    break ended;
                }
                crate::browser::thread::sleep(
                    std::time::Duration::from_millis(10),
                );
            },
        };
        *self.ended.lock().expect("a status") = Some(status);
        Ok(status)
    }

    /// Waits for it to end, keeping what it said: nothing, in a page.
    ///
    /// # Returns
    ///
    /// How it ended.
    ///
    /// # Errors
    ///
    /// None.
    pub fn wait_with_output(mut self) -> io::Result<Output> {
        Ok(Output {
            status: self.wait()?,
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
}

/// The calling program's number.
///
/// # Returns
///
/// It.
pub fn id() -> u32 {
    context::current().id
}

/// What [`exit`] unwinds with, for [`run`] to turn into the program's
/// status.
#[derive(Debug, Clone, Copy)]
pub struct Exited(pub i32);

/// Ends the calling program: in a page, its thread, by unwinding to its
/// entry point, which is the one way out of a thread from inside it.
///
/// # Arguments
///
/// * `code` - What it ends with.
pub fn exit(code: i32) -> ! {
    std::panic::resume_unwind(Box::new(Exited(code)))
}

/// Ends the page's WebAssembly where it is.
pub fn abort() -> ! {
    core::arch::wasm32::unreachable()
}

/// The signal a program was ended by: never, in a page.
///
/// # Arguments
///
/// * `_status` - How it ended.
///
/// # Returns
///
/// `None`.
pub fn ended_by(_status: &ExitStatus) -> Option<i32> {
    None
}

/// Asks a program to stop, by its number, as SIGTERM would.
///
/// # Arguments
///
/// * `pid` - Its number.
///
/// # Errors
///
/// `NotFound` where no program has the number.
pub fn terminate(pid: u32) -> io::Result<()> {
    if context::ask_to_stop(pid) {
        Ok(())
    } else {
        Err(io::Error::from(io::ErrorKind::NotFound))
    }
}

/// Asks a program to stop, by its number: no thread can be killed.
///
/// # Arguments
///
/// * `pid` - Its number.
///
/// # Errors
///
/// As for [`terminate`].
pub fn kill(pid: u32) -> io::Result<()> {
    terminate(pid)
}

/// Runs a function as the page ends: never, a page ending all at once.
///
/// # Arguments
///
/// * `_run` - What would be run.
pub fn at_exit(_run: extern "C" fn()) {}
