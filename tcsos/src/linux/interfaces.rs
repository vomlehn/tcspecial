//! The interfaces a program is started with, and what it is handed them
//! as: on Linux, descriptors 0, 1 and 2, each one end of a Unix-domain
//! socket pair.
//!
//! A program an app node is reads and writes packets on up to two data
//! interfaces and writes complaints on a third; what a packet is, and which
//! interface carries what, is tcsapp's. This is how the system hands them
//! over: [`pair`] makes one, [`hand_over`] starts a program with them in
//! its standard slots, and the program, once started, takes them back out
//! of those slots with [`take_stream`] and [`take_file`].

use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::process::{Command, Stdio};

/// What an interface is carried on: one end of a pair, read and written.
pub type Stream = std::os::unix::net::UnixStream;

/// What the error interface is carried on once taken: a file, as it is only
/// written.
pub type Written = File;

/// Makes the two ends of an interface.
///
/// # Returns
///
/// The ends, either of which may be handed over.
///
/// # Errors
///
/// What the system said.
pub fn pair() -> io::Result<(Stream, Stream)> {
    Stream::pair()
}

/// Has a program a command starts begin with no signal blocked, whatever
/// the thread that starts it has blocked: a signal mask passes through
/// `fork` and `exec` unchanged, and a program left with the stop signals
/// blocked could not be stopped by a plain `kill`.
///
/// # Arguments
///
/// * `command` - The command, not yet spawned.
pub fn unblocking(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    // SAFETY: pre_exec needs a closure that is safe to run between fork and
    // exec, in a child that may have one thread. sigemptyset fills a set on
    // this closure's own stack, and sigprocmask sets the calling thread's
    // mask, the child's only thread: both are async-signal-safe, and
    // neither allocates or locks.
    unsafe {
        command.pre_exec(|| {
            let mut none: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut none);
            if libc::sigprocmask(libc::SIG_SETMASK, &none, std::ptr::null_mut())
                != 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

/// A clone of the child's end of a pair, as a command takes a descriptor.
///
/// Cloned rather than moved, so that the caller holds the pair until it
/// has every one of them.
///
/// # Arguments
///
/// * `stream` - The child's end.
///
/// # Returns
///
/// What the command is given.
///
/// # Errors
///
/// What the system said where the end cannot be cloned.
fn childs_end(stream: &Stream) -> io::Result<Stdio> {
    Ok(Stdio::from(OwnedFd::from(stream.try_clone()?)))
}

/// Has a command start its program with its interfaces in the standard
/// slots -- the payload at 0, the gateway at 1, the errors at 2 -- with
/// no signal blocked, and a data interface it does not have not open at
/// all.
///
/// Not `/dev/null` in an empty slot, which would be a write that silently
/// succeeds and a read that silently ends -- the two outcomes an app node
/// must never have -- so an empty slot is closed in the child between the
/// fork and the exec.
///
/// # Arguments
///
/// * `command` - The command, not yet spawned.
/// * `payload` - The child's end of the payload interface, where it has
///               one.
/// * `gateway` - The child's end of the gateway interface, where it has
///               one.
/// * `errors`  - The child's end of the error interface.
///
/// # Errors
///
/// What the system said where an end cannot be cloned.
pub fn hand_over(
    command: &mut Command,
    payload: Option<&Stream>,
    gateway: Option<&Stream>,
    errors: &Stream,
) -> io::Result<()> {
    use std::os::unix::process::CommandExt;

    unblocking(command);
    match payload {
        Some(theirs) => command.stdin(childs_end(theirs)?),
        None => command.stdin(Stdio::null()),
    };
    match gateway {
        Some(theirs) => command.stdout(childs_end(theirs)?),
        None => command.stdout(Stdio::null()),
    };
    command.stderr(childs_end(errors)?);

    let shut: Vec<RawFd> = [
        payload.is_none().then_some(0),
        gateway.is_none().then_some(1),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !shut.is_empty() {
        // SAFETY: pre_exec needs a closure that is safe to run between fork
        // and exec, in a child that may have one thread: nothing that
        // allocates, locks or is not async-signal-safe. This walks a Vec it
        // owns, allocated before the fork, and calls close, which is
        // async-signal-safe; the descriptors it closes are the child's
        // copies, which nothing in the child uses again before exec.
        unsafe {
            command.pre_exec(move || {
                for fd in &shut {
                    libc::close(*fd);
                }
                Ok(())
            });
        }
    }
    Ok(())
}

/// Whether a standard slot has anything in it at all.
///
/// # Arguments
///
/// * `slot` - 0, 1 or 2.
///
/// # Returns
///
/// `true` where the descriptor is open.
pub fn is_open(slot: i32) -> bool {
    // SAFETY: F_GETFD only reads the descriptor table, so any number may be
    // asked about: one that is not open is answered with -1 and EBADF.
    unsafe { libc::fcntl(slot, libc::F_GETFD) != -1 }
}

/// Whether a standard slot holds an interface that was handed over: a
/// socket, which is what each data interface has to be, being read and
/// written -- where a Rust program's own standard library will have opened
/// `/dev/null` over anything it found closed, which answers "open" while
/// carrying nothing.
///
/// # Arguments
///
/// * `slot` - 0, 1 or 2.
///
/// # Returns
///
/// `true` where it is open and a socket.
pub fn is_handed_over(slot: i32) -> bool {
    let mut about = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: fstat needs a stat it can write, which `about` is; any
    // descriptor may be asked about, and one that is not open fails with
    // EBADF. assume_init needs the stat written, which it is once fstat has
    // not failed.
    unsafe {
        if libc::fstat(slot, about.as_mut_ptr()) == -1 {
            return false;
        }
        (about.assume_init().st_mode & libc::S_IFMT) == libc::S_IFSOCK
    }
}

/// Gives what is in a slot a descriptor of its own, the lowest above 2,
/// closed on exec, so that it is not the standard stream as well and a
/// program this one starts does not inherit it.
///
/// # Arguments
///
/// * `slot` - 0, 1 or 2.
///
/// # Returns
///
/// The new descriptor, owned.
///
/// # Errors
///
/// What the system said.
fn moved(slot: i32) -> io::Result<OwnedFd> {
    // SAFETY: F_DUPFD_CLOEXEC makes a new descriptor from an open one and
    // reads nothing through a pointer; one that is not open fails with
    // EBADF.
    let fd = unsafe { libc::fcntl(slot, libc::F_DUPFD_CLOEXEC, 3) };
    if fd == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a descriptor just made, which nothing else has the number of.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// Takes a data interface out of its slot.
///
/// # Arguments
///
/// * `slot` - 0 or 1.
///
/// # Returns
///
/// The interface, on a descriptor of its own.
///
/// # Errors
///
/// What the system said.
pub fn take_stream(slot: i32) -> io::Result<Stream> {
    Ok(Stream::from(moved(slot)?))
}

/// Takes the error interface out of its slot: it is only written, so need
/// be no more than a file.
///
/// # Arguments
///
/// * `slot` - 2.
///
/// # Returns
///
/// The interface, on a descriptor of its own.
///
/// # Errors
///
/// What the system said.
pub fn take_file(slot: i32) -> io::Result<Written> {
    Ok(File::from(moved(slot)?))
}

/// Writes to what is in a slot without taking it: for a complaint made
/// before the interfaces are taken, which must not close the slot as a
/// handle that owned it would.
///
/// # Arguments
///
/// * `slot`  - 2.
/// * `write` - What to do with it.
pub fn with_slot(slot: i32, write: impl FnOnce(&mut dyn io::Write)) {
    // SAFETY: from_raw_fd needs an open descriptor, which the caller has
    // found the slot to be, and the ManuallyDrop means this File never
    // closes it.
    let mut file =
        std::mem::ManuallyDrop::new(unsafe { File::from_raw_fd(slot) });
    write(&mut *file);
}

/// Covers the three standard slots with `/dev/null`, once the interfaces
/// have descriptors of their own: the standard library's streams name these
/// numbers without owning them, and from now reach nothing.
///
/// # Errors
///
/// What the system said.
pub fn quiet_standard_slots() -> io::Result<()> {
    let null = File::options().read(true).write(true).open("/dev/null")?;
    for fd in 0..=2 {
        // SAFETY: dup2 puts a copy of an open descriptor at a number,
        // closing what was there, and reads nothing through a pointer; what
        // is closed at 0, 1 and 2 nothing owns.
        if unsafe { libc::dup2(null.as_raw_fd(), fd) } == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}
