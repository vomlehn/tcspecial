//! What one tcspecial holds against another: its state file and its devices.
//!
//! Two nodes in one configuration may not want the same device, as
//! tcs-verify says (`tcsprotocol::graph::check`, "two nodes want one
//! thing"), but nothing in a configuration can see another process. Two
//! tcspecials given configurations that name one serial line would each
//! read part of what it carries and report it as the whole, and two given
//! one state file would each overwrite what the other is to start with. So
//! each takes a lock on what it claims, and a second that finds the lock
//! held is refused, saying which file and why.
//!
//! The locks are the kernel's advisory ones. They bind nothing but another
//! tcspecial, which is what they are for, and they are let go of when the
//! descriptor is closed, so a tcspecial that dies leaves nothing behind
//! for the next start to clear. A whole file is held with `flock`, which
//! does not care whether it was opened for reading or for writing. A device
//! on an I2C bus is held with an open file description lock on one byte of
//! the bus, the byte at its address: the bus is shared by design and only
//! the address is claimed, as tcs-verify claims it, and two nodes of one
//! tcspecial on one bus are two opens, each with its own lock, that do not
//! collide.
//!
//! Only a device -- a character or a block device -- is locked. A named
//! pipe, a socket or a plain file named as a device file is a channel two
//! processes may share by design, one writing and one reading, so it is
//! left to them. What the kernel already gives to one holder at a time is
//! not locked here either: a GPIO line, which a second request is refused, and a socket's
//! address. A CAN or HDLC interface is not a file, and every socket on one
//! is handed every frame, so two tcspecials on one interface each see all
//! of it rather than part.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use log::warn;
use tcsprotocol::{TcsError, TcsResult};

/// Whether a lock was taken.
#[derive(Debug)]
enum Taken {
    /// It was, and is held for as long as the descriptor is open.
    Yes,
    /// Another open of the file holds it.
    Elsewhere,
    /// The file cannot be locked, the kernel says why: a file system or a
    /// driver without locks.
    Cannot(io::Error),
}

/// Takes `file` whole for this process, without waiting.
///
/// # Arguments
///
/// * `file` - The open file.
///
/// # Returns
///
/// Whether it was taken.
fn whole(file: &File) -> Taken {
    // SAFETY: flock on a descriptor `file` holds open, with no pointer.
    let said =
        unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    taken(said)
}

/// Takes the byte at `at` of `file` for this open of it, without waiting.
///
/// # Arguments
///
/// * `file` - The open file, opened for writing.
/// * `at`   - Which byte.
///
/// # Returns
///
/// Whether it was taken.
fn one_byte(file: &File, at: u16) -> Taken {
    // SAFETY: flock is plain data, for which zero is a valid value.
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_WRLCK as libc::c_short;
    lock.l_whence = libc::SEEK_SET as libc::c_short;
    lock.l_start = libc::off_t::from(at);
    lock.l_len = 1;
    // l_pid stays nought, as an open file description lock requires.
    //
    // SAFETY: F_OFD_SETLK on a descriptor `file` holds open, with a pointer
    // to a flock that lives across the call.
    let said =
        unsafe { libc::fcntl(file.as_raw_fd(), libc::F_OFD_SETLK, &lock) };
    taken(said)
}

/// Tries `take` until it is not held elsewhere, for a moment at most.
///
/// A process that starts a program holds a copy of every descriptor of the
/// one that started it until the program runs, so a device this process has
/// just closed may be held by its own child for that moment: segsend's,
/// an app node's, or, in a test, another test's. Another tcspecial holds it
/// for as long as it runs, so a lock still held after [`HELD_FOR`] is held
/// by one.
///
/// # Arguments
///
/// * `take` - The lock call.
///
/// # Returns
///
/// What the last try said.
fn retried(take: impl Fn() -> Taken) -> Taken {
    let tries = HELD_FOR.as_millis() / TRY_EVERY.as_millis();
    for _ in 1..tries {
        match take() {
            Taken::Elsewhere => std::thread::sleep(TRY_EVERY),
            said => return said,
        }
    }
    take()
}

/// How long a lock found held is tried again before it is taken to be
/// another tcspecial's: see [`retried`].
const HELD_FOR: std::time::Duration = std::time::Duration::from_millis(500);

/// How often it is tried in that time.
const TRY_EVERY: std::time::Duration = std::time::Duration::from_millis(10);

/// What a lock call's result says.
///
/// # Arguments
///
/// * `said` - What `flock` or `fcntl` returned.
///
/// # Returns
///
/// Whether the lock was taken, from `errno` where it was not.
fn taken(said: libc::c_int) -> Taken {
    if said == 0 {
        return Taken::Yes;
    }
    let e = io::Error::last_os_error();
    match e.raw_os_error() {
        Some(libc::EWOULDBLOCK) | Some(libc::EACCES) => Taken::Elsewhere,
        _ => Taken::Cannot(e),
    }
}

/// The refusal of a device another process holds.
///
/// # Arguments
///
/// * `what`  - The node.
/// * `whose` - What it wanted: the device, or the place on a bus.
///
/// # Returns
///
/// The error that refuses the node.
fn held_elsewhere(what: &str, whose: &str) -> TcsError {
    TcsError::Endpoint(format!(
        "{what}: {whose} is held by another process -- another tcspecial, \
         most likely, whose configuration names it too. Give each running \
         tcspecial its own devices, or stop the other"
    ))
}

/// Takes the device `file` is, at `path`, for node `what`.
///
/// A device that cannot be locked at all is opened unlocked, said in the
/// log: a device the vehicle needs is not refused for the want of a lock
/// only another tcspecial would ask for.
///
/// # Arguments
///
/// * `what` - The node.
/// * `path` - The device's path, as the configuration names it.
/// * `file` - The device, open.
///
/// # Returns
///
/// Nothing, once the device is held or cannot be.
///
/// # Errors
///
/// `TcsError::Endpoint`, naming the node and the device, if another
/// process holds it.
pub fn device(what: &str, path: &str, file: &File) -> TcsResult<()> {
    let is_device = file.metadata().is_ok_and(|meta| {
        meta.file_type().is_char_device() || meta.file_type().is_block_device()
    });
    if !is_device {
        return Ok(());
    }
    held(what, path, file)
}

/// Takes the file `file` is, at `path`, for node `what`, whatever kind of
/// file it is: [`device`] once it has found a device.
///
/// # Arguments
///
/// * `what` - The node.
/// * `path` - The file's path, as the configuration names it.
/// * `file` - The file, open.
///
/// # Returns
///
/// Nothing, once the file is held or cannot be.
///
/// # Errors
///
/// `TcsError::Endpoint`, naming the node and the file, if another
/// process holds it.
fn held(what: &str, path: &str, file: &File) -> TcsResult<()> {
    match retried(|| whole(file)) {
        Taken::Yes => Ok(()),
        Taken::Elsewhere => Err(held_elsewhere(what, path)),
        Taken::Cannot(e) => {
            warn!(
                "{what}: {path} cannot be locked ({e}), so another tcspecial \
                 could open it beside this one"
            );
            Ok(())
        }
    }
}

/// Takes the device at `address` on the I2C bus `file` is, at `bus`, for
/// node `what`.
///
/// # Arguments
///
/// * `what`    - The node.
/// * `bus`     - The bus's path, as the configuration names it.
/// * `address` - The device's address on it.
/// * `file`    - The bus, open for reading and writing.
///
/// # Returns
///
/// Nothing, once the address is held or cannot be.
///
/// # Errors
///
/// `TcsError::Endpoint`, naming the node, the bus and the address, if
/// another process holds the address.
pub fn on_bus(
    what: &str,
    bus: &str,
    address: u16,
    file: &File,
) -> TcsResult<()> {
    let whose = || format!("the address {address:#04X} on {bus}");
    match retried(|| one_byte(file, address)) {
        Taken::Yes => Ok(()),
        Taken::Elsewhere => Err(held_elsewhere(what, &whose())),
        Taken::Cannot(e) => {
            warn!(
                "{what}: {} cannot be locked ({e}), so another tcspecial \
                 could open it beside this one",
                whose()
            );
            Ok(())
        }
    }
}

/// The state file's lock, held from the start until the process ends or
/// runs itself again, and which state file it is the lock of.
static STATE_LOCK: Mutex<Option<(PathBuf, File)>> = Mutex::new(None);

/// The file locked for state file `path`: beside it, with `.lock` added.
///
/// Not the state file itself, because that is replaced by a rename at every
/// write, and a lock on the file renamed over would hold nothing. Never
/// removed, because a lock file removed while another process waits to open
/// it is two lock files.
///
/// # Arguments
///
/// * `path` - The state file.
///
/// # Returns
///
/// The lock file's path.
pub fn lock_file_of(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".lock");
    path.with_file_name(name)
}

/// Takes state file `path` for this process, until it ends or runs itself
/// again.
///
/// # Arguments
///
/// * `path` - The state file, as the configuration's `state_file` names it.
///
/// # Returns
///
/// Nothing, once it is held, or where this process holds it already.
///
/// # Errors
///
/// What is wrong, naming the file and the setting to change: the lock
/// file cannot be made or locked, or another process holds it.
pub fn state_file(path: &Path) -> Result<(), String> {
    let mut held = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if held.as_ref().is_some_and(|(was, _)| was == path) {
        return Ok(());
    }
    let lock = lock_file_of(path);
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock)
        .map_err(|e| {
            format!(
                "the state file's lock {} cannot be made ({e}): state_file \
                 in the configuration's tcspecial section must name a file \
                 in a directory tcspecial can write",
                lock.display()
            )
        })?;
    match retried(|| whole(&file)) {
        Taken::Yes => {
            *held = Some((path.to_path_buf(), file));
            Ok(())
        }
        Taken::Elsewhere => Err(format!(
            "the state file {} is held by another process -- another \
             tcspecial, most likely, whose configuration names it too. Give \
             each running tcspecial its own state_file in its \
             configuration's tcspecial section, or stop the other",
            path.display()
        )),
        Taken::Cannot(e) => Err(format!(
            "the state file's lock {} cannot be taken ({e}): state_file in \
             the configuration's tcspecial section must name a file on a \
             file system with locks",
            lock.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file in a fresh directory of its own.
    ///
    /// # Arguments
    ///
    /// * `name` - What the test calls it.
    ///
    /// # Returns
    ///
    /// The directory, which removes itself, and the file's path in it.
    fn a_file(name: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join(name);
        std::fs::write(&path, b"").expect("a file");
        (dir, path)
    }

    /// Opens `path` for reading and writing, as a second process would.
    fn open(path: &Path) -> File {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .expect("it opens")
    }

    #[test]
    fn a_device_held_by_another_open_is_refused() {
        let (_dir, path) = a_file("ttyS0");
        let shown = path.display().to_string();
        let first = open(&path);
        held("first", &shown, &first).expect("the first takes it");
        let second = open(&path);
        let refused = held("second", &shown, &second)
            .expect_err("the second is refused")
            .to_string();
        assert!(refused.contains("second"), "{refused}");
        assert!(refused.contains("another tcspecial"), "{refused}");
        drop(first);
        // A child another test forks holds a copy of every descriptor until
        // it runs its program, so the lock can outlast the drop by a moment.
        let mut tries = 0;
        while held("second", &shown, &second).is_err() {
            tries += 1;
            assert!(tries < 100, "it is never let go");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn a_file_that_is_not_a_device_is_not_locked() {
        // A plain file, as a named pipe or a socket would be: shared by
        // design, one writing and one reading.
        let (_dir, path) = a_file("pipe");
        let shown = path.display().to_string();
        let writer = open(&path);
        device("writer", &shown, &writer).expect("it is opened");
        let reader = open(&path);
        device("reader", &shown, &reader).expect("and opened beside it");
        assert!(matches!(whole(&reader), Taken::Yes), "nothing held it");
    }

    #[test]
    fn a_device_shares_its_lock_with_its_duplicate() {
        // The reader and the writer of a node are one open, duplicated.
        let (_dir, path) = a_file("spidev0.0");
        let shown = path.display().to_string();
        let reader = open(&path);
        held("node", &shown, &reader).expect("it is taken");
        let writer = reader.try_clone().expect("a duplicate");
        held("node", &shown, &writer).expect("the duplicate holds it too");
    }

    #[test]
    fn an_address_on_a_bus_is_held_and_the_bus_is_not() {
        let (_dir, path) = a_file("i2c-1");
        let shown = path.display().to_string();
        let first = open(&path);
        on_bus("first", &shown, 0x48, &first).expect("it takes 0x48");
        let beside = open(&path);
        on_bus("beside", &shown, 0x49, &beside).expect("0x49 is free");
        let second = open(&path);
        let refused = on_bus("second", &shown, 0x48, &second)
            .expect_err("0x48 is held")
            .to_string();
        assert!(refused.contains("0x48"), "{refused}");
    }

    #[test]
    fn a_state_file_held_elsewhere_is_refused() {
        let (_dir, path) = a_file("state");
        // Another process, as this one sees it: an open of the lock file
        // that is not this process's own.
        let other = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_file_of(&path))
            .expect("the lock file");
        assert!(matches!(whole(&other), Taken::Yes));
        let refused = state_file(&path).expect_err("it is held");
        assert!(refused.contains("state_file"), "{refused}");
        drop(other);
        // As above, a forked child may hold the lock a moment longer.
        let mut tries = 0;
        while state_file(&path).is_err() {
            tries += 1;
            assert!(tries < 100, "it is never let go");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        state_file(&path).expect("and taken again is still held");
    }
}
