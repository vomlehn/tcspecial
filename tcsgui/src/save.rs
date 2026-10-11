//! Writing a file without being able to damage the one already there.
//!
//! The editor saves after every edit, so it writes far more often than a
//! person saves by hand, and it writes configuration files a vehicle will fly
//! on. Those two together decide the whole of this module: a write that is
//! interrupted -- by power, by a full device, by the process being killed --
//! must leave either the file that was there before or the file that was being
//! written, and never half of either.
//!
//! The recipe is the one POSIX gives, and every step of it is load-bearing:
//!
//! 1. write the new contents to a temporary file *in the same directory* as
//!    the target. The same directory because the last step is a rename, and a
//!    rename is only atomic within one filesystem; a temporary file in `/tmp`
//!    would be a copy across a boundary and could be interrupted halfway.
//! 2. check every write. A short write is not an error to be retried blindly:
//!    on a full device it is how `ENOSPC` arrives, and the one thing that must
//!    not happen then is replacing a good file with a truncated one.
//! 3. `fsync` the temporary file, so its contents are on the device rather
//!    than in a cache that a power failure empties.
//! 4. rename it over the target. This is the atomic step: afterwards every
//!    reader sees the new file, before it every reader saw the old one, and
//!    there is no instant at which a reader sees neither.
//! 5. `fsync` the directory. Without this the rename itself can be lost, which
//!    gives the old contents back -- survivable, but not what was reported to
//!    the user as saved.
//!
//! If anything fails, the temporary file is removed and the error says what
//! happened. The target is untouched, which is the promise this module makes.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

/// What went wrong, and whether the file on disk is still the old one.
///
/// Every variant means the target was not replaced. That is the point of
/// saying it in the type rather than in a comment: a caller that sees an error
/// from here knows the previous contents are intact and can offer to retry
/// without having to ask what state anything is in.
#[derive(Debug)]
pub enum SaveError {
    /// The temporary file could not be created beside the target.
    Temporary { path: PathBuf, why: std::io::Error },
    /// The contents could not be written. A full device arrives here.
    Write { path: PathBuf, why: std::io::Error },
    /// The contents could not be forced to the device.
    Sync { path: PathBuf, why: std::io::Error },
    /// The temporary file could not be renamed over the target. The target is
    /// whatever it was.
    Replace { path: PathBuf, why: std::io::Error },
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Each one says what was being attempted and that nothing was lost,
        // because the first question asked of a failed save is whether the
        // previous file survived.
        match self {
            SaveError::Temporary { path, why } => write!(
                f,
                "{}: a temporary file could not be made beside it ({why}). \
                 The file on disk is unchanged",
                path.display()
            ),
            SaveError::Write { path, why } => write!(
                f,
                "{}: writing the new contents failed ({why}). The file on disk \
                 is unchanged -- if the device is full, nothing has been lost \
                 by this",
                path.display()
            ),
            SaveError::Sync { path, why } => write!(
                f,
                "{}: the new contents could not be forced to the device \
                 ({why}). The file on disk is unchanged",
                path.display()
            ),
            SaveError::Replace { path, why } => write!(
                f,
                "{}: the new contents are written but could not replace the \
                 old ones ({why}). The file on disk is unchanged",
                path.display()
            ),
        }
    }
}

impl std::error::Error for SaveError {}

/// Write `contents` to `path`, replacing whatever is there atomically.
///
/// Returns once the new contents are durable: a reader afterwards sees them,
/// and a power failure afterwards does not take them away. On any error the
/// previous contents are still there -- see [`SaveError`].
pub fn write_atomically(path: &Path, contents: &[u8]) -> Result<(), SaveError> {
    let directory = path.parent().filter(|p| !p.as_os_str().is_empty());
    let directory = match directory {
        Some(directory) => directory,
        // A bare file name means the working directory, which is what a
        // relative path with no parent is asking for.
        None => Path::new("."),
    };

    let temporary = temporary_beside(path);

    // Created rather than opened: a temporary this process did not make is one
    // something else may be using, and writing into it would be writing into
    // somebody's file. `create_new` fails instead.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|why| SaveError::Temporary {
            path: path.to_path_buf(),
            why,
        })?;

    // From here on every failure removes the temporary before returning, so a
    // device that is full does not stay full of this program's leavings.
    let written = write_all_and_sync(&mut file, contents);
    if let Err(what) = written {
        let _ = fs::remove_file(&temporary);
        return Err(match what {
            Attempt::Write(why) => SaveError::Write {
                path: path.to_path_buf(),
                why,
            },
            Attempt::Sync(why) => SaveError::Sync {
                path: path.to_path_buf(),
                why,
            },
        });
    }
    drop(file);

    // The atomic step.
    if let Err(why) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(SaveError::Replace {
            path: path.to_path_buf(),
            why,
        });
    }

    // And the step that makes the rename itself survive a power failure.
    // Note the target is already replaced by now, so a failure here is not a
    // reason to say nothing was written -- but it is a reason to say the save
    // is not durable, which is what the caller is told.
    sync_directory(directory).map_err(|why| SaveError::Sync {
        path: directory.to_path_buf(),
        why,
    })
}

/// Which step of writing the contents failed.
enum Attempt {
    Write(std::io::Error),
    Sync(std::io::Error),
}

/// Write the whole of `contents` and force it to the device.
///
/// `write_all` is used rather than a loop over `write`, because it is the same
/// loop and it gets the short-write case right; what it does not do is force
/// anything to the device, which is the second half of this.
fn write_all_and_sync(file: &mut File, contents: &[u8]) -> Result<(), Attempt> {
    file.write_all(contents).map_err(Attempt::Write)?;
    // Before the sync, so that a buffer not yet handed to the kernel is not
    // left out of what the sync forces.
    file.flush().map_err(Attempt::Write)?;
    file.sync_all().map_err(Attempt::Sync)
}

/// Force a directory's own contents -- its list of names -- to the device.
///
/// There is no portable way to do this through `std`: a directory cannot be
/// opened for writing, and `File::sync_all` on a read-only handle is what is
/// left. It works, and it is what every database does for the same reason.
fn sync_directory(directory: &Path) -> std::io::Result<()> {
    let handle = File::open(directory)?;
    // SAFETY: a valid descriptor from a handle that outlives the call.
    let result = unsafe { libc::fsync(handle.as_raw_fd()) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// A name for the temporary file, beside the target and unmistakably ours.
///
/// The process id is in it so that two editors saving the same file at once do
/// not write into one temporary. They would still race for the rename, and the
/// loser's contents would be the ones that survive -- which is the behaviour of
/// two editors on one file everywhere, and is not something this can fix.
fn temporary_beside(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.as_bytes().to_vec())
        .unwrap_or_default();
    let mut temporary = Vec::with_capacity(name.len() + 24);
    temporary.extend_from_slice(b".");
    temporary.extend_from_slice(&name);
    temporary.extend_from_slice(format!(".tcsgui-{}", std::process::id()).as_bytes());

    let mut full = path.to_path_buf();
    full.set_file_name(std::ffi::OsStr::from_bytes(&temporary));
    full
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What was asked for is what is there afterwards.
    #[test]
    fn a_file_written_holds_what_was_given() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("graph.yaml");

        write_atomically(&path, b"nodes: []\n").expect("the first write");
        assert_eq!(fs::read(&path).unwrap(), b"nodes: []\n");

        // And again, over contents that already exist, which is what the
        // editor does after every edit.
        write_atomically(&path, b"nodes: [a]\n").expect("the second write");
        assert_eq!(fs::read(&path).unwrap(), b"nodes: [a]\n");
    }

    /// Nothing of ours is left in the directory afterwards.
    ///
    /// A temporary left behind is not merely untidy: the editor saves after
    /// every edit, so one left per save fills the device it was being careful
    /// about.
    #[test]
    fn no_temporary_file_is_left_behind() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("graph.yaml");

        for n in 0..5 {
            write_atomically(&path, format!("nodes: [{n}]\n").as_bytes()).expect("a write");
        }

        let left: Vec<_> = fs::read_dir(where_.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name != "graph.yaml")
            .collect();
        assert!(left.is_empty(), "left behind: {left:?}");
    }

    /// A write that cannot be made leaves the previous file exactly as it was.
    ///
    /// The case this module exists for. A directory in place of the target is
    /// how it is provoked here -- the rename cannot replace a directory --
    /// which exercises the same path a full device takes: fail, clean up, and
    /// do not touch what was there.
    #[test]
    fn a_failed_write_leaves_the_old_file_alone() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("graph.yaml");
        write_atomically(&path, b"the good contents\n").expect("the good write");

        // A read-only directory: the temporary cannot be created in it.
        let locked = where_.path().join("locked");
        fs::create_dir(&locked).unwrap();
        let target = locked.join("graph.yaml");
        write_atomically(&target, b"first\n").expect("a file to overwrite");
        let mut permissions = fs::metadata(&locked).unwrap().permissions();
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(0o500);
        }
        fs::set_permissions(&locked, permissions).unwrap();

        let said = match write_atomically(&target, b"second\n") {
            Err(e) => format!("{e}"),
            Ok(()) => panic!("a write into a read-only directory succeeded"),
        };
        assert!(
            said.contains("unchanged"),
            "the error should say the old file survived: {said}"
        );
        assert_eq!(
            fs::read(&target).unwrap(),
            b"first\n",
            "the previous contents were damaged by a failed write"
        );

        // Put it back so the directory can be cleaned up.
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&locked).unwrap().permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(&locked, permissions).unwrap();
        }
    }

    /// The temporary is in the target's own directory.
    ///
    /// Which is what makes the last step a rename rather than a copy. Checked
    /// by name rather than by watching a save, because the file is gone by the
    /// time a save returns.
    #[test]
    fn the_temporary_is_beside_the_target() {
        let temporary = temporary_beside(Path::new("/var/lib/tcs/graph.yaml"));
        assert_eq!(temporary.parent(), Some(Path::new("/var/lib/tcs")));
        assert!(
            temporary
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".graph.yaml.tcsgui-"),
            "{temporary:?}"
        );
    }

    /// A bare file name is written in the working directory.
    #[test]
    fn a_name_with_no_directory_is_written_where_the_program_is() {
        let where_ = tempfile::tempdir().expect("a directory");
        let was = std::env::current_dir().expect("a working directory");
        std::env::set_current_dir(where_.path()).expect("to move there");

        let result = write_atomically(Path::new("graph.yaml"), b"nodes: []\n");

        std::env::set_current_dir(was).expect("to move back");
        result.expect("a write with no directory in the path");
        assert_eq!(fs::read(where_.path().join("graph.yaml")).unwrap(), b"nodes: []\n");
    }
}
