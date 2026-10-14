//! Where the undo history is kept, and how long it lives.
//!
//! The editor writes the configuration after every edit, so the history is not
//! protecting anybody's work: the work is on the disk already. What it buys is
//! the ability to undo past a restart or a crash -- a convenience and not a
//! safety net, which is what decides everything below.
//!
//! # Where
//!
//! In the user's state directory: `$XDG_STATE_HOME/tcsgui`, or
//! `~/.local/state/tcsgui` where that is not set. The specification names this
//! exact case -- "state that should persist between restarts but is not
//! important enough for data", with history and layout given as the examples
//! -- and it is not the cache directory, because a cache is something a
//! program can regenerate and this is not.
//!
//! It used to sit beside the configuration, as `<name>.tcsgui-history`, on the
//! argument that a configuration copied to another machine should take its
//! history with it. That argument was worth less than it cost. The
//! configuration lives in a directory under version control, so the file
//! turned up in `git status`, in review and in release tarballs -- and nobody
//! has ever wanted to undo an edit on a machine they copied a file to.
//!
//! The name is the configuration's stem and a digest of its absolute path:
//! `graph-3f91c2d4e5f60718.yaml`. The stem so that a person looking in the
//! directory can tell what belongs to what, the digest so that two
//! configurations called `graph.yaml` in two directories are two histories.
//!
//! The digest is FNV-1a, written out here, and the reason is stability: a
//! history is found again by recomputing its name, so the function must give
//! the same answer in a year. `DefaultHasher` does not promise that across
//! Rust releases, and a hash that changes with the toolchain would orphan
//! every history on the disk without saying so.
//!
//! # How long
//!
//! Deleted on a clean exit, which is what every other editor does: the history
//! of a session that ended tidily is of no further use, and a directory that
//! only ever grows is a directory somebody has to tidy.
//!
//! Kept when the editor does *not* exit tidily, which is the whole point. A
//! history on the disk at startup means the last session ended badly, and the
//! undo it offers is the one somebody will want.
//!
//! And pruned at startup, two ways: a history older than a week, and one whose
//! configuration is no longer there. The first is the ordinary bound -- a
//! crash a fortnight ago is not an undo anybody is waiting for -- and the
//! second is tidiness that nothing else would do, a configuration being free
//! to be deleted or renamed by anything at all.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::journal::Journal;

/// How long a history outlives the session that wrote it.
///
/// A week. The question it answers is "did something go wrong recently enough
/// that the undo is still wanted", and a week is generous for that: nobody
/// comes back to a fortnight-old crash meaning to press undo. What it buys by
/// being generous is that somebody who crashed on Friday still has their
/// history on Monday.
const KEPT_FOR: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// A history as it is written down: whose it is, and what it holds.
///
/// The configuration's path is in the file because the name is a digest and a
/// digest cannot be read backwards. Two things need it: the prune, which asks
/// whether the configuration is still there, and a person looking at the
/// directory wondering what a file is.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kept {
    /// The configuration this is the history of, absolute.
    pub configuration: PathBuf,
    pub journal: Journal,
}

/// Where tcsgui keeps what it remembers between sessions.
///
/// `None` where neither `XDG_STATE_HOME` nor `HOME` says: an editor with
/// nowhere to put a history still edits, and what is lost is undo across a
/// crash. Saying so would be saying it to somebody who cannot act on it.
pub fn state_dir() -> Option<PathBuf> {
    // Under test, a directory of this process's own, whatever the environment
    // says. The editor writes a history on every edit, so a test run that used
    // the real one would leave its litter in the state directory of whoever
    // ran it -- and a test that pointed the environment somewhere instead
    // would be pointing it there for every other test in the process.
    #[cfg(test)]
    {
        return Some(
            std::env::temp_dir().join(format!("tcsgui-tests-{}", std::process::id())),
        );
    }

    #[cfg(not(test))]
    said_by(
        std::env::var_os("XDG_STATE_HOME"),
        std::env::var_os("HOME"),
    )
}

/// The rule itself, given what the environment says.
///
/// Apart from [`state_dir`] so that it can be checked: `$XDG_STATE_HOME` where
/// that is set to an absolute path, `~/.local/state` where it is not, and
/// nowhere at all where neither is said -- which is a session with no history
/// and an editor that still edits.
///
/// An `XDG_STATE_HOME` that is not absolute is ignored rather than resolved,
/// which is what the specification says to do with a relative one.
fn said_by(
    xdg: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    if let Some(said) = xdg {
        let said = PathBuf::from(said);
        if said.is_absolute() {
            return Some(said.join("tcsgui"));
        }
    }
    home.map(|home| PathBuf::from(home).join(".local/state/tcsgui"))
}

/// The file this configuration's history goes in.
///
/// `None` where there is nowhere to keep one: see [`state_dir`].
pub fn journal_for(configuration: &Path) -> Option<PathBuf> {
    Some(journal_in(&state_dir()?, configuration))
}

/// The same, in a directory given rather than found.
///
/// Every function here has this shape: one that asks where the state
/// directory is, and one that is told. The telling is what the tests use, and
/// not as a convenience -- a test that set `XDG_STATE_HOME` would be setting
/// it for every other test in the process, and the editor writes a history on
/// every edit, so the tests would be writing into each other's directories as
/// they were being deleted.
pub fn journal_in(directory: &Path, configuration: &Path) -> PathBuf {
    let whole = absolute(configuration);
    let stem = whole
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "configuration".to_string());
    let stem: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();

    directory.join(format!("{stem}-{:016x}.yaml", digest(&whole)))
}

/// The configuration's path, made absolute without asking the disk.
///
/// Absolute because the digest is of a path and two names for one file must
/// digest alike: `graph.yaml` and `./graph.yaml` in one directory are one
/// configuration and must find one history. Not canonicalised, which would
/// ask the disk and fail for a configuration that does not exist yet -- which
/// is exactly the case of a new one.
fn absolute(path: &Path) -> PathBuf {
    let whole = if path.is_absolute() {
        path.to_path_buf()
    } else {
        match std::env::current_dir() {
            Ok(here) => here.join(path),
            Err(_) => path.to_path_buf(),
        }
    };

    // And tidied, because the digest is of the text: `graph.yaml`,
    // `./graph.yaml` and `vehicles/../graph.yaml` are one configuration, and
    // three histories for it would be three answers to one question. Tidied
    // here rather than by asking the disk, which is what canonicalising does
    // and which fails for a configuration that does not exist yet -- exactly
    // the case of a new one.
    let mut tidy = PathBuf::new();
    for part in whole.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                // Only where there is something to go up from: `/..` is `/`,
                // and a `..` that walked off the top would be a path this had
                // invented.
                if tidy.parent().is_some() {
                    tidy.pop();
                }
            }
            other => tidy.push(other.as_os_str()),
        }
    }
    tidy
}

/// FNV-1a over the path's bytes.
///
/// Written out rather than taken from the standard library on purpose: see the
/// module comment. It has to give the same answer next year.
fn digest(path: &Path) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.as_os_str().to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

/// Read this configuration's history, where there is one.
///
/// `None` for every way of not having one, and none of them is worth a word to
/// the person at the keyboard: no state directory, no file, a file that cannot
/// be read, a file that is a history of something else. The last is the one
/// worth naming -- a digest collision, or a file somebody copied in -- and the
/// answer is the same as for a damaged one: the work is open and correct, and
/// what is lost is the undo.
pub fn read(configuration: &Path) -> Option<Journal> {
    read_in(&state_dir()?, configuration)
}

/// The same, in a directory given rather than found.
pub fn read_in(directory: &Path, configuration: &Path) -> Option<Journal> {
    let text = std::fs::read_to_string(journal_in(directory, configuration)).ok()?;
    let kept: Kept = serde_yaml::from_str(&text).ok()?;
    (kept.configuration == absolute(configuration)).then_some(kept.journal)
}

/// Write this configuration's history, or say what stopped it.
///
/// The directory is made if it is not there, this being the first thing to use
/// it.
pub fn write(configuration: &Path, journal: &Journal) -> Result<(), String> {
    let Some(directory) = state_dir() else {
        return Err("there is nowhere to keep a history: neither XDG_STATE_HOME \
                    nor HOME is set"
            .to_string());
    };
    write_in(&directory, configuration, journal)
}

/// The same, in a directory given rather than found.
pub fn write_in(
    directory: &Path,
    configuration: &Path,
    journal: &Journal,
) -> Result<(), String> {
    let path = journal_in(directory, configuration);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("{} cannot be made: {e}", parent.display()))?;
    }

    let kept = Kept {
        configuration: absolute(configuration),
        journal: journal.clone(),
    };
    let text = serde_yaml::to_string(&kept).map_err(|why| why.to_string())?;
    crate::save::write_atomically(&path, text.as_bytes()).map_err(|why| why.to_string())
}

/// Throw this configuration's history away.
///
/// For a clean exit. Nothing is said about a failure: the session is over, the
/// configuration is written, and a message nobody will read is a message.
pub fn forget(configuration: &Path) {
    if let Some(directory) = state_dir() {
        forget_in(&directory, configuration);
    }
}

/// The same, in a directory given rather than found.
pub fn forget_in(directory: &Path, configuration: &Path) {
    let _ = std::fs::remove_file(journal_in(directory, configuration));
}

/// Delete the histories that have outlived their use.
///
/// Two kinds, and the second is the one nothing else would do: a history older
/// than a week, and one whose configuration is no longer where it was. A
/// configuration is free to be deleted or renamed by anything at all, and the
/// editor is the only thing that would ever notice its history was left
/// behind.
///
/// Returns how many went, which is what a caller would log. Silent about
/// failures for the reason [`forget`] is: nobody can act on them.
pub fn prune(now: SystemTime) -> usize {
    match state_dir() {
        Some(directory) => prune_in(&directory, now),
        None => 0,
    }
}

/// The same, in a directory given rather than found.
pub fn prune_in(directory: &Path, now: SystemTime) -> usize {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return 0;
    };

    let mut gone = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }

        let stale = entry
            .metadata()
            .and_then(|about| about.modified())
            .map(|when| {
                now.duration_since(when)
                    .map(|since| since > KEPT_FOR)
                    .unwrap_or(false)
            })
            .unwrap_or(false);

        // A history of a configuration that is not there any more. A file that
        // cannot be read at all is left alone: it is not this function's to
        // decide that somebody else's file in this directory is rubbish.
        let orphaned = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_yaml::from_str::<Kept>(&text) {
                Ok(kept) => !kept.configuration.exists(),
                Err(_) => false,
            },
            Err(_) => false,
        };

        if (stale || orphaned) && std::fs::remove_file(&path).is_ok() {
            gone += 1;
        }
    }
    gone
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::Edit;

    /// A history is kept away from the configuration, named after it, and
    /// thrown away when nobody is coming back for it.
    ///
    /// One test and not five, and told where to put things rather than setting
    /// `XDG_STATE_HOME`: the environment is the process's, the editor writes a
    /// history on every edit, and tests run at the same time -- so a test that
    /// set it would have every other test in the binary writing into a
    /// directory it was about to delete.
    #[test]
    fn a_history_is_kept_away_from_the_configuration_and_pruned() {
        let where_ = tempfile::tempdir().expect("a directory");
        let state = where_.path().join("state");
        let one = where_.path().join("vehicles/aurora/graph.yaml");
        let two = where_.path().join("vehicles/borealis/graph.yaml");

        // Named after the configuration, so a person can tell what is what,
        // and digested so that two files of one name are two histories.
        let named = journal_in(&state, &one);
        assert!(
            named
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("graph-"),
            "{named:?}"
        );
        assert_eq!(named.parent(), Some(state.as_path()));
        assert_ne!(named, journal_in(&state, &two));
        // And the same answer every time, which is what finding one again
        // depends on.
        assert_eq!(named, journal_in(&state, &one));
        // A relative name and an absolute one are one configuration: the
        // digest is of the absolute path.
        let here = std::env::current_dir().expect("somewhere to stand");
        assert_eq!(
            journal_in(&state, Path::new("./graph.yaml")),
            journal_in(&state, &here.join("graph.yaml"))
        );

        // Nothing to read before anything is written, and a history written
        // comes back as what it was.
        assert!(read_in(&state, &one).is_none());
        let mut journal = Journal::new();
        journal.record(Edit::AddNode {
            name: "sensor".to_string(),
            node_type: "i2c".to_string(),
            home: None,
        });
        std::fs::create_dir_all(one.parent().unwrap()).expect("the vehicle's own");
        std::fs::write(&one, "version: \"2.0\"\n").expect("a configuration");
        write_in(&state, &one, &journal).expect("the history is written");
        assert!(named.exists(), "{named:?}");
        assert_eq!(read_in(&state, &one).map(|kept| kept.depth()), Some(1));

        // A history of another configuration is not this one's, however it
        // came to be under this name: a digest collision, or a file somebody
        // copied in.
        let elsewhere = Kept {
            configuration: two.clone(),
            journal: journal.clone(),
        };
        std::fs::write(&named, serde_yaml::to_string(&elsewhere).unwrap())
            .expect("a file of somebody else's");
        assert!(
            read_in(&state, &one).is_none(),
            "a history belongs to one configuration"
        );
        write_in(&state, &one, &journal).expect("written again");

        // A clean exit throws it away.
        forget_in(&state, &one);
        assert!(!named.exists());

        // Pruned when the configuration is gone, whatever its age: nothing
        // else would ever notice, a configuration being free to be deleted or
        // renamed by anything at all.
        write_in(&state, &one, &journal).expect("written again");
        std::fs::remove_file(&one).expect("the configuration goes");
        assert_eq!(prune_in(&state, SystemTime::now()), 1);
        assert!(!named.exists());

        // And pruned when it is old: a crash a fortnight ago is not an undo
        // anybody is waiting for.
        std::fs::write(&one, "version: \"2.0\"\n").expect("a configuration again");
        write_in(&state, &one, &journal).expect("written again");
        let fortnight = SystemTime::now() + Duration::from_secs(14 * 24 * 60 * 60);
        assert_eq!(prune_in(&state, fortnight), 1);
        assert!(!named.exists());

        // A young one whose configuration is there is left alone. That is the
        // history of a session that ended badly, and it is the one somebody
        // will want.
        write_in(&state, &one, &journal).expect("written again");
        assert_eq!(prune_in(&state, SystemTime::now()), 0);
        assert!(named.exists());

        // And nothing this does not understand is touched, whatever is in it:
        // it is not the prune's place to decide that somebody else's file is
        // rubbish.
        let theirs = state.join("notes.yaml");
        std::fs::write(&theirs, "not a history\n").expect("somebody's file");
        prune_in(&state, SystemTime::now());
        assert!(theirs.exists());
    }

    /// Where the state directory is, which is the one thing the environment
    /// decides.
    ///
    /// The rule is asked directly rather than through the environment, which
    /// is the process's and not a test's to set -- and which `state_dir`
    /// ignores under test anyway, so that a test run leaves nothing in the
    /// state directory of whoever ran it.
    #[test]
    fn the_state_directory_is_the_one_the_specification_names() {
        use std::ffi::OsString;

        // What the specification names, where it is set.
        assert_eq!(
            said_by(Some(OsString::from("/home/a/.local/state")), None),
            Some(PathBuf::from("/home/a/.local/state/tcsgui"))
        );
        // And where it is not: the default the specification gives.
        assert_eq!(
            said_by(None, Some(OsString::from("/home/a"))),
            Some(PathBuf::from("/home/a/.local/state/tcsgui"))
        );
        // A relative one is ignored rather than resolved, which is what the
        // specification says to do with it.
        assert_eq!(
            said_by(Some(OsString::from("state")), Some(OsString::from("/home/a"))),
            Some(PathBuf::from("/home/a/.local/state/tcsgui"))
        );
        // And neither: nowhere to keep a history. The editor still edits, and
        // what is lost is undo across a crash.
        assert_eq!(said_by(None, None), None);
    }
}
