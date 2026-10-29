//! Handing each filled telemetry log segment to `segsend`.
//!
//! The telemetry log is written in segment files, and a segment that has
//! filled is no longer the log's to keep: left where it is, a long flight
//! fills the directory it was given. So each one is handed, by name, to a
//! program called `segsend`, which takes it from there -- downlinks it,
//! compresses it, or, as the one shipped with tcspecial does for testing,
//! deletes it. What a mission does with its segments is the mission's
//! business, and a program of its own is where that business can be written
//! without changing the flight software.
//!
//! # Out of the log's name before it is handed over
//!
//! tcslog hands a filled segment over through `WriteCallbacks::send`, and
//! requires that when that returns there is no file of the log's naming
//! pattern left at that path. So the file is first renamed, by putting
//! [`PENDING_SUFFIX`] on the end of its name, and it is the renamed file that
//! `segsend` is given. A rename within a directory is a metadata operation,
//! cheap enough to do with the log's lock held, and it leaves the file where
//! it was if anything after it goes wrong: a segment `segsend` never got to is
//! found by its suffix at the next start and handed over then.
//!
//! # One at a time, on a thread of its own
//!
//! The log is written from the pass and from the command interpreter, and
//! neither may wait for a program to run. The callback puts the renamed name
//! on a queue of fixed size and returns; a thread started with the log takes
//! each name off it, starts `segsend` on it and waits for it to finish before
//! starting the next. One at a time because the segments are in order and a
//! downlink is one link: two `segsend`s at once would be racing for it.
//!
//! # No allocation once running
//!
//! A name is copied into a buffer of fixed size, the queue's room is made
//! when the queue is, and `segsend` is started with `posix_spawn` from those
//! buffers. Nothing a segment is handed over with comes from the heap. A
//! queue that is full -- `segsend` slower than the log fills -- leaves the
//! segment renamed in the directory, said once, for the next start to send.

use std::ffi::{CStr, CString};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;

use log::{error, info};

/// What the program a filled segment is handed to is called.
pub const SEGSEND: &str = "segsend";

/// What a segment's name has put on its end once it has left the log, and
/// before `segsend` has dealt with it.
///
/// Anything that does not end with the log's own suffix would take the file
/// out of the log's naming pattern; this one says why it was taken out.
pub const PENDING_SUFFIX: &str = ".segsend";

/// The longest path a segment can be handed over by, with the pending suffix
/// and the terminating nul: the kernel's own limit on a path.
const NAME_BYTES: usize = libc::PATH_MAX as usize;

/// How many segments can be waiting for `segsend` at once.
///
/// A segment is filled in the time it takes to write its size in bytes,
/// which is minutes at the rates a vehicle sends at, so a queue this deep
/// rides out a `segsend` that takes many segments' time to deal with one,
/// and the segments every log finds in its directory as it opens. Its room,
/// a path's length a segment, is made once, at the start.
const WAITING: usize = 64;

/// A segment's name, nul-terminated, in a buffer that does not move.
struct Name {
    bytes: [u8; NAME_BYTES],
}

impl Name {
    /// Makes the name of `path` with `suffix` put on its end.
    ///
    /// # Arguments
    ///
    /// * `path`   - The path to name.
    /// * `suffix` - What to put on its end; empty for the path as it is.
    ///
    /// # Returns
    ///
    /// The name, nul-terminated, or `None` where it is longer than a path may
    /// be or has a nul in it.
    fn of(path: &Path, suffix: &str) -> Option<Name> {
        let path = path.as_os_str().as_bytes();
        let suffix = suffix.as_bytes();
        let length = path.len() + suffix.len();
        if length >= NAME_BYTES || path.contains(&0) || suffix.contains(&0) {
            return None;
        }
        let mut bytes = [0u8; NAME_BYTES];
        bytes[..path.len()].copy_from_slice(path);
        bytes[path.len()..length].copy_from_slice(suffix);
        Some(Name { bytes })
    }

    fn as_c_str(&self) -> &CStr {
        // Made only by `of`, which leaves a nul after the name and none in it.
        CStr::from_bytes_until_nul(&self.bytes).expect("a name ends in a nul")
    }

    /// For saying what went wrong with it.
    fn shown(&self) -> std::borrow::Cow<'_, str> {
        self.as_c_str().to_string_lossy()
    }
}

/// The thread that runs `segsend`, and the queue every log hands its filled
/// segments to it by.
///
/// One for the whole vehicle, however many logs it keeps: the segments of
/// all of them go down one link, so they are handed over one at a time and in
/// the order they filled. Cloning yields another handle on the same queue.
#[derive(Clone)]
pub struct SegSend {
    /// `None` where nothing was started: see [`SegSend::none`].
    queue: Option<SyncSender<Name>>,
    /// The directories already looked through for segments a previous run
    /// left pending, so that two logs kept in one directory do not hand the
    /// same segment over twice.
    swept: Arc<Mutex<Vec<PathBuf>>>,
}

impl SegSend {
    /// Starts the thread that hands segments to `program`.
    ///
    /// For startup: it allocates.
    ///
    /// # Arguments
    ///
    /// * `program` - The `segsend` to run on each filled segment.
    ///
    /// # Returns
    ///
    /// A handle on the queue the thread takes segments from.
    ///
    /// # Errors
    ///
    /// What is wrong, said in words, where `program`'s name has a nul in it
    /// or the thread cannot be started.
    pub fn start(program: &Path) -> Result<SegSend, String> {
        let program_c =
            CString::new(program.as_os_str().as_bytes()).map_err(|_| {
                format!("{} has a nul in its name", program.display())
            })?;
        let (queue, waiting) = sync_channel::<Name>(WAITING);
        let shown = program.display().to_string();
        thread::Builder::new()
            .name(SEGSEND.to_string())
            .spawn(move || send_each(&program_c, &shown, waiting))
            .map_err(|e| {
                format!("the thread that runs {SEGSEND} cannot be started: {e}")
            })?;
        Ok(SegSend {
            queue: Some(queue),
            swept: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Makes a handle that starts nothing and leaves filled segments where
    /// they are.
    ///
    /// For a vehicle that keeps no log -- no `log.directory` and no `log`
    /// node -- which has nothing to hand over and so does not need `segsend`
    /// to be there; and for the tests, which fill no segments.
    ///
    /// # Returns
    ///
    /// The handle, with no queue.
    pub fn none() -> SegSend {
        SegSend {
            queue: None,
            swept: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Makes what a log kept in `dir` is opened with, having first handed
    /// over every segment a previous run left pending there.
    ///
    /// For startup, before the log is opened: it reads the directory, and
    /// opening the log hands over the segments it finds, which should go
    /// after the ones renamed before them.
    ///
    /// A directory already swept, by another log kept in it, is not swept
    /// again. A pending segment whose name is too long is said and left.
    ///
    /// # Arguments
    ///
    /// * `dir` - The directory the log is kept in.
    ///
    /// # Returns
    ///
    /// The log's end of the queue, which hands over nothing where this
    /// handle was made by [`SegSend::none`].
    ///
    /// # Errors
    ///
    /// What is wrong, said in words, where `dir` cannot be read or the
    /// thread that runs `segsend` has stopped.
    pub fn handoff(&self, dir: &Path) -> Result<Handoff, String> {
        if let Some(queue) = &self.queue {
            let key = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
            let mut swept =
                self.swept.lock().expect("nothing panics holding it");
            if !swept.contains(&key) {
                swept.push(key);
                for pending in left_pending(dir)? {
                    let Some(name) = Name::of(&pending, "") else {
                        error!(
                            "{}: too long a name to hand to {SEGSEND}; left \
                             where it is",
                            pending.display()
                        );
                        continue;
                    };
                    // Waited on where the queue is full, there being no loop
                    // yet to hold up.
                    if queue.send(name).is_err() {
                        return Err(format!(
                            "the thread that runs {SEGSEND} has stopped"
                        ));
                    }
                }
            }
        }
        Ok(Handoff {
            queue: self.queue.clone(),
            overflowing: false,
        })
    }
}

/// Where one log hands its filled segments over: its end of the queue.
///
/// What a log is opened with, as its `WriteCallbacks`.
pub struct Handoff {
    queue: Option<SyncSender<Name>>,
    /// Whether the last segment found the queue full, so that a run of them
    /// is said once rather than once a segment.
    overflowing: bool,
}

impl tcslog::WriteCallbacks for Handoff {
    /// Rename the filled segment out of the log's naming pattern and queue it
    /// for `segsend`.
    ///
    /// An error only where the file could not be renamed, which is the one
    /// case in which the log's bound on its directory has stopped holding.
    fn send(&mut self, path: &Path) -> std::io::Result<()> {
        let Some(queue) = &self.queue else {
            // Nothing to hand it to: see `SegSend::none`.
            return Ok(());
        };
        let (Some(from), Some(to)) =
            (Name::of(path, ""), Name::of(path, PENDING_SUFFIX))
        else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "a segment's name is too long to rename",
            ));
        };
        // SAFETY: both are nul-terminated buffers that outlive the call.
        if unsafe {
            libc::rename(from.as_c_str().as_ptr(), to.as_c_str().as_ptr())
        } != 0
        {
            return Err(std::io::Error::last_os_error());
        }

        match queue.try_send(to) {
            Ok(()) => {
                if self.overflowing {
                    info!("{SEGSEND} is keeping up again");
                }
                self.overflowing = false;
            }
            Err(TrySendError::Full(to)) => {
                if !self.overflowing {
                    error!(
                        "{} is waiting for {SEGSEND}, which has {WAITING} \
                         segments still to deal with; it and any others like \
                         it are left in the log directory and handed over at \
                         the next start",
                        to.shown()
                    );
                }
                self.overflowing = true;
            }
            Err(TrySendError::Disconnected(to)) => {
                error!(
                    "{} is left in the log directory: the thread that runs \
                     {SEGSEND} has stopped",
                    to.shown()
                );
            }
        }
        Ok(())
    }
}

/// Finds the segments a previous run renamed and did not hand over.
///
/// # Arguments
///
/// * `dir` - The log directory to look in.
///
/// # Returns
///
/// The paths of the files in `dir` whose names end in [`PENDING_SUFFIX`],
/// oldest first. Oldest by name, which is how tcslog names segments: by the
/// time each was begun.
///
/// # Errors
///
/// What is wrong, naming `dir`, where it cannot be read.
fn left_pending(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| {
        format!("the telemetry log directory, {}: {e}", dir.display())
    })?;
    let mut pending: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name().is_some_and(|name| {
                name.as_bytes().ends_with(PENDING_SUFFIX.as_bytes())
            })
        })
        .collect();
    pending.sort();
    Ok(pending)
}

/// Hands each name that arrives to `program`, one at a time, until the queue
/// is closed.
///
/// A `program` that cannot be run, or ends with a status other than 0, is
/// reported and the next name handed over.
///
/// # Arguments
///
/// * `program` - The `segsend` to run.
/// * `shown`   - Its path, for saying what went wrong with it.
/// * `waiting` - The queue the names arrive on.
fn send_each(program: &CStr, shown: &str, waiting: Receiver<Name>) {
    while let Ok(name) = waiting.recv() {
        match run(program, &name) {
            Ok(0) => {}
            Ok(status) => error!(
                "{shown} {} ended with status {status}; what it did with the \
                 segment is its own to say",
                name.shown()
            ),
            Err(e) => error!("{shown} {} could not be run: {e}", name.shown()),
        }
    }
}

extern "C" {
    /// The process's environment, which `segsend` is started with: it is
    /// part of how a mission's own program is told where to send.
    static environ: *const *mut libc::c_char;
}

/// Runs `program` with `name` as its one argument and waits for it.
///
/// # Arguments
///
/// * `program` - The program to run.
/// * `name`    - Its one argument, the segment to hand over.
///
/// # Returns
///
/// How it ended, as a shell would say: its exit status, or 128 and the
/// signal that ended it.
///
/// # Errors
///
/// What `posix_spawn` or `waitpid` failed with.
fn run(program: &CStr, name: &Name) -> std::io::Result<i32> {
    let argv: [*mut libc::c_char; 3] = [
        program.as_ptr() as *mut libc::c_char,
        name.as_c_str().as_ptr() as *mut libc::c_char,
        std::ptr::null_mut(),
    ];
    let mut pid: libc::pid_t = 0;
    // SAFETY: the program, the argument and the vector of them are
    // nul-terminated and outlive the call, which copies them; no file actions
    // or attributes are given, so the child inherits what any child of this
    // process does, and every descriptor tcspecial opens is closed on exec.
    let started = unsafe {
        libc::posix_spawn(
            &mut pid,
            program.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            argv.as_ptr(),
            environ,
        )
    };
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

/// Finds `segsend`: beside tcspecial's own executable, where a build puts
/// both and where tcsmoc finds tcspecial.
///
/// Found once, as the vehicle starts, and refused then if it is not there: a
/// vehicle that discovered at its first full segment that it had nothing to
/// hand it to would fill its log directory without a word.
///
/// # Returns
///
/// The path of `segsend`.
///
/// # Errors
///
/// What is wrong, said in the user's terms, where this program's own path
/// cannot be found or `segsend` is not a program this process may run.
pub fn beside_this_program() -> Result<PathBuf, String> {
    let here = std::env::current_exe().map_err(|e| {
        format!("where tcspecial is cannot be found, to find {SEGSEND}: {e}")
    })?;
    let program = here.with_file_name(SEGSEND);
    usable(&program)?;
    Ok(program)
}

/// Checks that `program` is a file this process may run.
///
/// # Arguments
///
/// * `program` - The path of `segsend`.
///
/// # Returns
///
/// Nothing, where it may be run.
///
/// # Errors
///
/// What is wrong, said in the user's terms with how to put it right, where
/// `program` has a nul in its name, is not there, is not a file or is not
/// executable by this user.
fn usable(program: &Path) -> Result<(), String> {
    let fix = format!(
        "put the program that takes filled telemetry log segments there, named \
         {SEGSEND} (`cargo build --workspace` builds the one that deletes \
         them), or take log.directory out of the configuration's tcspecial \
         section"
    );
    let name = CString::new(program.as_os_str().as_bytes())
        .map_err(|_| format!("{} has a nul in its name", program.display()))?;
    match std::fs::metadata(program) {
        Ok(found) if found.is_file() => {}
        Ok(_) => {
            return Err(format!(
                "{} is not a program: {fix}",
                program.display()
            ))
        }
        Err(_) => {
            return Err(format!("{} is not there: {fix}", program.display()))
        }
    }
    // SAFETY: a nul-terminated name that outlives the call.
    if unsafe { libc::access(name.as_ptr(), libc::X_OK) } != 0 {
        return Err(format!(
            "{} cannot be run by this user: make it executable, or {fix}",
            program.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};
    use tcslog::WriteCallbacks;

    /// A stand-in for `segsend` that records each name it is given, in a
    /// file beside it, and deletes the segment as the shipped one does.
    ///
    /// Run once with no name before it is returned, which it ignores: see
    /// `crate::when_not_busy`, which is why a program a test has just written
    /// cannot be relied on to start the first time.
    ///
    /// # Arguments
    ///
    /// * `dir` - Where to put it, and the file of names it records.
    ///
    /// # Returns
    ///
    /// The stand-in's path.
    ///
    /// # Panics
    ///
    /// If it cannot be written, made executable or run.
    fn recording(dir: &Path) -> PathBuf {
        let program = dir.join("segsend");
        let record = dir.join("handed");
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\n[ -n \"$1\" ] || exit 0\necho \"$1\" >> '{}'\nrm \
                 -f \"$1\"\n",
                record.display()
            ),
        )
        .expect("written");
        std::fs::set_permissions(
            &program,
            std::fs::Permissions::from_mode(0o755),
        )
        .expect("made executable");
        crate::when_not_busy(|| {
            std::process::Command::new(&program)
                .status()
                .map_err(|e| e.to_string())
        })
        .expect("the stand-in runs");
        program
    }

    /// Reads what the stand-in made by `recording` has been handed.
    ///
    /// # Arguments
    ///
    /// * `dir` - The directory the stand-in is in.
    ///
    /// # Returns
    ///
    /// The names it was given, in order; none where it has been given none.
    fn handed(dir: &Path) -> Vec<String> {
        std::fs::read_to_string(dir.join("handed"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// Waits, for up to ten seconds, for something to come true.
    ///
    /// # Arguments
    ///
    /// * `what` - Whether it has, asked every 10 ms.
    ///
    /// # Returns
    ///
    /// Whether it came true in time.
    fn until(what: impl Fn() -> bool) -> bool {
        let by = Instant::now() + Duration::from_secs(10);
        while Instant::now() < by {
            if what() {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        false
    }

    /// A filled segment leaves the log's name at once, and `segsend` is given
    /// its new name and deals with it.
    #[test]
    fn a_filled_segment_is_renamed_and_handed_to_segsend() {
        let tools = tempfile::tempdir().expect("a directory");
        let logs = tempfile::tempdir().expect("a directory");
        let program = recording(tools.path());
        let mut handoff = SegSend::start(&program)
            .and_then(|segsend| segsend.handoff(logs.path()))
            .expect("started");

        let segment = logs.path().join("telem-1.tcslog");
        std::fs::write(&segment, b"filled").expect("written");
        handoff.send(&segment).expect("handed over");
        assert!(!segment.exists(), "the segment is still in the log's name");

        let pending = format!("{}{PENDING_SUFFIX}", segment.display());
        assert!(
            until(|| handed(tools.path()) == [pending.clone()]),
            "{SEGSEND} was given {:?}",
            handed(tools.path())
        );
        assert!(
            until(|| !Path::new(&pending).exists()),
            "{pending} was not deleted"
        );
    }

    /// What a previous run renamed and did not hand over is handed over at
    /// the next start, oldest first.
    #[test]
    fn segments_left_pending_are_handed_over_at_the_start() {
        let tools = tempfile::tempdir().expect("a directory");
        let logs = tempfile::tempdir().expect("a directory");
        let program = recording(tools.path());
        let older = logs.path().join(format!("telem-1.tcslog{PENDING_SUFFIX}"));
        let newer = logs.path().join(format!("telem-2.tcslog{PENDING_SUFFIX}"));
        std::fs::write(&newer, b"").expect("written");
        std::fs::write(&older, b"").expect("written");
        std::fs::write(logs.path().join("telem-3.tcslog"), b"")
            .expect("written");

        let segsend = SegSend::start(&program).expect("started");
        let _handoff = segsend.handoff(logs.path()).expect("swept");
        // A second log in the same directory hands nothing over again.
        let _another = segsend.handoff(logs.path()).expect("not swept twice");
        let want = [older.display().to_string(), newer.display().to_string()];
        assert!(
            until(|| handed(tools.path()) == want),
            "{SEGSEND} was given {:?}",
            handed(tools.path())
        );
    }

    /// A `segsend` that is not there, or cannot be run, is refused by name.
    #[test]
    fn a_segsend_that_cannot_be_run_is_refused_by_name() {
        let tools = tempfile::tempdir().expect("a directory");
        let missing = tools.path().join("segsend");
        let said = usable(&missing).expect_err("nothing is there");
        assert!(said.contains(&missing.display().to_string()), "{said}");
        assert!(said.contains("log.directory"), "{said}");

        std::fs::write(&missing, b"#!/bin/sh\n").expect("written");
        std::fs::set_permissions(
            &missing,
            std::fs::Permissions::from_mode(0o644),
        )
        .expect("made unexecutable");
        let said = usable(&missing).expect_err("it cannot be run");
        assert!(said.contains("executable"), "{said}");
    }
}
