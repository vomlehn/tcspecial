//! Starting this program again as a child, waiting for it, and passing the
//! two stop signals on to it: what a supervisor asks of the system.
//!
//! The supervisor is single-threaded and allocates nothing while it waits,
//! so what a child is started with is made once, by [`Spawner::new`], and
//! [`Spawner::start`] only points the system at it. A child starts with
//! SIGINT and SIGTERM blocked, as they are in its supervisor, so that one
//! sent before it is ready to take them waits for it rather than ending it.

use std::ffi::{CStr, CString, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::time::Duration;

/// A child this process started, by its process id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Child(libc::pid_t);

impl std::fmt::Display for Child {
    /// Shows the child by its process id, as a log names a process.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// How a child ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// It exited with this status.
    Exited(i32),
    /// A signal ended it: the signal's number.
    Signalled(i32),
}

impl Ended {
    /// Whether a stop signal, SIGINT or SIGTERM, ended it.
    ///
    /// # Returns
    ///
    /// `true` where one of the two did.
    pub fn by_a_stop(self) -> bool {
        matches!(self, Ended::Signalled(libc::SIGINT | libc::SIGTERM))
    }
}

/// The two stop signals and SIGCHLD, as a set.
///
/// # Arguments
///
/// * `child` - Whether SIGCHLD is in it.
///
/// # Returns
///
/// The set.
fn signals(child: bool) -> libc::sigset_t {
    // SAFETY: a set made empty and then given signals, on the stack.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::sigaddset(&mut set, libc::SIGTERM);
        if child {
            libc::sigaddset(&mut set, libc::SIGCHLD);
        }
        set
    }
}

/// Blocks the two stop signals and SIGCHLD in this process, which must
/// have one thread, so that each waits to be taken by [`wait_for`] or
/// [`pause`]. The first thing a supervisor does.
pub fn block_stops_and_children() {
    let all = signals(true);
    // SAFETY: blocking signals in this, the only thread, with a set that
    // lives across the call.
    unsafe { libc::sigprocmask(libc::SIG_BLOCK, &all, std::ptr::null_mut()) };
}

/// What every child is started with, made once.
pub struct Spawner {
    /// The program, this one.
    program: CString,
    /// Its arguments, its own name first, as this process was given them:
    /// held for `argv` to point into.
    _args: Vec<CString>,
    /// This process's environment, with the child's variable added: held
    /// for `envp` to point into.
    _env: Vec<CString>,
    /// The arguments, as `posix_spawn` takes them, ending in a null.
    argv: Vec<*mut libc::c_char>,
    /// The argument a child started the other way is given, held for
    /// `other` to point into.
    _other_arg: CString,
    /// The arguments with that argument after the program's name.
    other: Vec<*mut libc::c_char>,
    /// The environment, as `posix_spawn` takes it, ending in a null.
    envp: Vec<*mut libc::c_char>,
}

impl Spawner {
    /// Makes what every child is started with: this program, its
    /// arguments, and its environment with one variable set.
    ///
    /// # Arguments
    ///
    /// * `variable`  - The variable every child is given, which tells it
    ///                 it is one; its value is this process's id.
    /// * `other_arg` - An argument a child may be started with, after the
    ///                 program's name, as well as its own.
    ///
    /// # Returns
    ///
    /// The spawner.
    ///
    /// # Errors
    ///
    /// Why not: a nul in the program's name, an argument or the
    /// environment.
    pub fn new(variable: &str, other_arg: &str) -> Result<Spawner, String> {
        let program = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("/proc/self/exe"));
        let c = |said: &[u8]| {
            CString::new(said).map_err(|_| {
                "a nul in the program's name, an argument or the environment"
                    .to_string()
            })
        };
        let args = std::env::args_os()
            .map(|arg: OsString| c(arg.as_bytes()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut env = std::env::vars_os()
            .filter(|(name, _)| name != variable)
            .map(|(name, value)| {
                let mut pair = name.as_bytes().to_vec();
                pair.push(b'=');
                pair.extend_from_slice(value.as_bytes());
                c(&pair)
            })
            .collect::<Result<Vec<_>, _>>()?;
        env.push(c(format!("{variable}={}", std::process::id()).as_bytes())?);
        // Pointers into the strings, whose bytes stay where they are when
        // the vectors holding them move.
        let pointers = |all: &[CString]| {
            all.iter()
                .map(|one| one.as_ptr() as *mut libc::c_char)
                .chain(std::iter::once(std::ptr::null_mut()))
                .collect::<Vec<_>>()
        };
        let other_arg = c(other_arg.as_bytes())?;
        let mut other = pointers(&args);
        other.insert(1.min(args.len()), other_arg.as_ptr() as *mut _);
        Ok(Spawner {
            program: c(program.as_os_str().as_bytes())?,
            argv: pointers(&args),
            envp: pointers(&env),
            _other_arg: other_arg,
            other,
            _args: args,
            _env: env,
        })
    }

    /// Starts a child, with SIGINT and SIGTERM blocked in it as they are
    /// here, and SIGCHLD not.
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
    /// What `posix_spawn` failed with.
    pub fn start(&self, other: bool) -> std::io::Result<Child> {
        let argv = if other { &self.other } else { &self.argv };
        let mask = signals(false);
        let mut pid: libc::pid_t = 0;
        // SAFETY: the attributes are made, used and destroyed here; the
        // program, the arguments, the environment and the vectors of them
        // are nul-terminated and outlive the call, which copies them. Every
        // descriptor the TCS programs open is closed on exec, so the child
        // is handed what this process was handed and nothing else.
        let started = unsafe {
            let mut attr: libc::posix_spawnattr_t = std::mem::zeroed();
            libc::posix_spawnattr_init(&mut attr);
            libc::posix_spawnattr_setsigmask(&mut attr, &mask);
            libc::posix_spawnattr_setflags(
                &mut attr,
                libc::POSIX_SPAWN_SETSIGMASK as libc::c_short,
            );
            let started = libc::posix_spawn(
                &mut pid,
                self.program.as_ptr(),
                std::ptr::null(),
                &attr,
                argv.as_ptr(),
                self.envp.as_ptr(),
            );
            libc::posix_spawnattr_destroy(&mut attr);
            started
        };
        if started != 0 {
            return Err(std::io::Error::from_raw_os_error(started));
        }
        Ok(Child(pid))
    }
}

/// Waits for a child to end, passing SIGINT and SIGTERM on to it.
///
/// # Arguments
///
/// * `child`    - The child.
/// * `look`     - How long each wait for a signal lasts before it looks
///                again whether the child has ended.
/// * `stopping` - Set where a stop signal was taken, and passed on.
///
/// # Returns
///
/// How it ended.
pub fn wait_for(child: Child, look: Duration, stopping: &mut bool) -> Ended {
    let set = signals(true);
    let look = libc::timespec {
        tv_sec: look.as_secs() as libc::time_t,
        tv_nsec: look.subsec_nanos() as libc::c_long,
    };
    loop {
        let mut status: libc::c_int = 0;
        // SAFETY: `status` is this frame's own, and the pid is a child this
        // process started and nothing else waits for.
        let waited =
            unsafe { libc::waitpid(child.0, &mut status, libc::WNOHANG) };
        if waited == child.0 {
            return if libc::WIFEXITED(status) {
                Ended::Exited(libc::WEXITSTATUS(status))
            } else {
                Ended::Signalled(libc::WTERMSIG(status))
            };
        }
        // SAFETY: waiting on a set and a timeout that live across the call.
        let taken =
            unsafe { libc::sigtimedwait(&set, std::ptr::null_mut(), &look) };
        if taken == libc::SIGINT || taken == libc::SIGTERM {
            *stopping = true;
            // SAFETY: a signal sent to the child this process started.
            unsafe { libc::kill(child.0, taken) };
        }
    }
}

/// Waits for a time, unless a stop signal comes first.
///
/// # Arguments
///
/// * `delay` - How long.
///
/// # Returns
///
/// Whether a stop signal came.
pub fn pause(delay: Duration) -> bool {
    let set = signals(false);
    let wait = libc::timespec {
        tv_sec: delay.as_secs() as libc::time_t,
        tv_nsec: delay.subsec_nanos() as libc::c_long,
    };
    // SAFETY: waiting on a set and a timeout that live across the call.
    let taken =
        unsafe { libc::sigtimedwait(&set, std::ptr::null_mut(), &wait) };
    taken == libc::SIGINT || taken == libc::SIGTERM
}

/// Makes this process be sent SIGTERM when its parent ends, so that no
/// child runs without the process that started it, and says whether the
/// parent is still the one it should be.
///
/// # Arguments
///
/// * `parent` - The process id the parent should have, where it is known.
///
/// # Returns
///
/// `false` where the parent has ended already, between starting this and
/// now: this process has been handed to another.
pub fn follow_parent(parent: Option<u32>) -> bool {
    // SAFETY: prctl with a signal number, and getppid, take and return
    // integers only.
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM as libc::c_ulong);
    }
    parent.is_none_or(|pid| std::os::unix::process::parent_id() == pid)
}

/// The signals a supervisor's tests name, by what they are.
pub mod signal {
    /// SIGINT.
    pub const INTERRUPT: i32 = libc::SIGINT;
    /// SIGTERM.
    pub const TERMINATE: i32 = libc::SIGTERM;
    /// SIGSEGV.
    pub const SEGMENTATION: i32 = libc::SIGSEGV;
    /// SIGKILL.
    pub const KILL: i32 = libc::SIGKILL;
}

extern "C" {
    /// The process's environment, which a program is started with.
    static environ: *const *mut libc::c_char;
}

/// Runs a program with one argument, with no signal blocked and this
/// process's environment, and waits for it, allocating nothing.
///
/// Started with no signal blocked, as every program the TCS programs start
/// is: a thread that runs this has the stop signals blocked, and the block
/// would otherwise pass to the program.
///
/// # Arguments
///
/// * `program` - The program to run.
/// * `arg`     - Its one argument.
///
/// # Returns
///
/// How it ended, as a shell would say: its exit status, or 128 and the
/// signal that ended it.
///
/// # Errors
///
/// What `posix_spawn` or `waitpid` failed with.
pub fn run(program: &CStr, arg: &CStr) -> std::io::Result<i32> {
    let argv: [*mut libc::c_char; 3] = [
        program.as_ptr() as *mut libc::c_char,
        arg.as_ptr() as *mut libc::c_char,
        std::ptr::null_mut(),
    ];
    let mut pid: libc::pid_t = 0;
    // SAFETY: the attributes and the empty set are this frame's own; the
    // attributes are destroyed once made, whatever comes after, and on a
    // failure to make or set them the program is not started.
    let mut attributes: libc::posix_spawnattr_t = unsafe { std::mem::zeroed() };
    let made = unsafe { libc::posix_spawnattr_init(&mut attributes) };
    if made != 0 {
        return Err(std::io::Error::from_raw_os_error(made));
    }
    let set = unsafe {
        let mut none: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut none);
        match libc::posix_spawnattr_setsigmask(&mut attributes, &none) {
            0 => libc::posix_spawnattr_setflags(
                &mut attributes,
                libc::POSIX_SPAWN_SETSIGMASK as libc::c_short,
            ),
            failed => failed,
        }
    };
    if set != 0 {
        // SAFETY: made by posix_spawnattr_init above, and used no more.
        unsafe { libc::posix_spawnattr_destroy(&mut attributes) };
        return Err(std::io::Error::from_raw_os_error(set));
    }
    // SAFETY: the program, the argument and the vector of them are
    // nul-terminated and outlive the call, which copies them, as it does the
    // attributes; no file actions are given, and every descriptor the TCS
    // programs open is closed on exec.
    let started = unsafe {
        libc::posix_spawn(
            &mut pid,
            program.as_ptr(),
            std::ptr::null(),
            &attributes,
            argv.as_ptr(),
            environ,
        )
    };
    // SAFETY: made by posix_spawnattr_init above, and used no more.
    unsafe { libc::posix_spawnattr_destroy(&mut attributes) };
    if started != 0 {
        return Err(std::io::Error::from_raw_os_error(started));
    }

    let mut status: libc::c_int = 0;
    loop {
        // SAFETY: `status` is this frame's own, and `pid` is a child this
        // thread started and nothing else waits for.
        let waited = unsafe { libc::waitpid(pid, &mut status, 0) };
        if waited == pid {
            break;
        }
        let e = std::io::Error::last_os_error();
        if e.kind() != std::io::ErrorKind::Interrupted {
            return Err(e);
        }
    }
    if libc::WIFEXITED(status) {
        Ok(libc::WEXITSTATUS(status))
    } else {
        Ok(128 + libc::WTERMSIG(status))
    }
}

/// How a child is ended when its parent is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// SIGTERM, which a program can take as a stop and finish what it is
    /// doing.
    Terminate,
    /// SIGKILL, which no program can.
    Kill,
}

/// Has a program a command starts be ended when the thread that started it
/// ends, so that none is left running without what started it; and not be
/// started at all where its parent ended between the fork and the asking.
///
/// The kernel sends the signal when the thread that started the child
/// ends, not the process, so children are best started from a thread that
/// lasts as long as the program does.
///
/// # Arguments
///
/// * `command` - The command, not yet spawned.
/// * `ending`  - What the child is sent.
/// * `gone`    - What the child's start fails with where its parent has
///               already ended.
pub fn dies_with_parent(
    command: &mut std::process::Command,
    ending: Ending,
    gone: &'static str,
) {
    use std::os::unix::process::CommandExt;

    let parent = std::process::id();
    let signal = match ending {
        Ending::Terminate => libc::SIGTERM,
        Ending::Kill => libc::SIGKILL,
    };
    // SAFETY: prctl and getppid are async-signal-safe, and touch nothing of
    // this process's but the child's own death signal.
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, signal) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            if std::os::unix::process::parent_id() != parent {
                return Err(std::io::Error::other(gone));
            }
            Ok(())
        });
    }
}
