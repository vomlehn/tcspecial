//! The state file: what tcspecial keeps across a restart.
//!
//! The *configuration stack*: the configurations a start may run, by
//! checksum, the top being the one the next child runs, above the
//! configuration on tcspecial's command line, which is the bottom, is not
//! written, and is never popped. A matching CONNECT pushes the running
//! configuration, and an armed RESTART the one it names; the supervisor pops
//! the top when its child fails. With it, how many attempts the top has had
//! since a matching CONNECT was last taken on it, and whether the next start
//! is one the ground commanded. It is kept in the file the `tcspecial`
//! section's `state_file` names, on storage that survives a power cycle,
//! because what is kept in memory is gone at every crash and every power
//! cycle -- which are the restarts this is most needed across. How it is
//! used is in [`crate::runtime::supervisor`] and [`crate::runtime::choose`].
//!
//! There is no generation number. A start of tcspecial is told by its
//! beacon's restart flag and its number going back, and which configuration
//! it runs by its checksum; nothing in the state file counts starts but the
//! attempts on the top. A file written by a build that kept a generation
//! still has a `generation` line, which is read and dropped, and the start
//! says so in its log. One written before the stack, which named a `next`
//! and a `confirmed` configuration, is read as the stack of the two, the
//! confirmed one beneath.
//!
//! The file is a few lines of text, so that somebody at a console can read
//! it, and ends with a CRC of everything before it. The stack is written
//! from the bottom up, one `stack` line for each configuration:
//!
//! ```text
//! # tcspecial's state, which it writes on every start: do not edit.
//! stack 0123456789abcdef0123456789abcdef
//! stack 89abcdef0123456789abcdef01234567
//! trials 1
//! next_start commanded
//! crc16 0x1a2b
//! ```
//!
//! The `next_start` line is there only between a RESTART's being taken and
//! the next start, which reads it and writes the file again without it.
//!
//! It is written whole to a file beside it, flushed, and renamed over it, and
//! the directory is flushed after: a power failure part way through leaves
//! the old file or the new one, never half of each. Every change is made
//! under the *short lock*, an exclusive `flock` on the file beside it named
//! with `.update` added, held while the file is read, changed and renamed:
//! tcspecial's supervisor, its child and software outside tcspecial that
//! pushes or pops the stack all change it, and two changes made at once
//! would lose one. See "The state file" in `docs/tcspecial-design.rst`.

use std::io::Write;
use std::path::{Path, PathBuf};
use tcsos::fs::{File, OpenOptions};

use tcsprotocol::crc::crc16;
use tcsprotocol::ConfigDigest;

/// What a state file holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    /// The configuration stack above the command line's, bottom first: the
    /// last is the top, which the next child runs. Empty is the command
    /// line's alone.
    pub stack: Vec<ConfigDigest>,
    /// How many children have been started on the top since a matching
    /// CONNECT was last taken on it: the supervisor adds one before each,
    /// and a matching CONNECT sets it back to nought, so one that has run
    /// until it was matched leaves nought behind. Of a stacked
    /// configuration only: the command line's is never given up.
    pub trials: u32,
    /// Whether the next start is one the ground commanded.
    pub commanded: bool,
}

impl State {
    /// The configuration the next child runs, where it is a stacked one.
    ///
    /// # Returns
    ///
    /// The top, or `None` where the stack holds only the command line's.
    pub fn top(&self) -> Option<ConfigDigest> {
        self.stack.last().copied()
    }

    /// Pushes `digest`, unless it is the top already, its attempts starting
    /// again at nought.
    ///
    /// # Arguments
    ///
    /// * `digest` - The configuration's checksum.
    fn push(&mut self, digest: ConfigDigest) {
        if self.top() != Some(digest) {
            self.stack.push(digest);
            self.trials = 0;
        }
    }
}

/// The command line's configuration, which is the bottom of the stack and is
/// not written in it: set once a start knows it, so that a CONNECT matching
/// it pushes nothing.
static BOTTOM: std::sync::OnceLock<ConfigDigest> = std::sync::OnceLock::new();

/// Says which configuration is the bottom of the stack.
///
/// # Arguments
///
/// * `digest` - The command line's configuration's checksum.
pub fn set_bottom(digest: ConfigDigest) {
    let _ = BOTTOM.set(digest);
}

/// What a start found in the state file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// What it holds: nothing, where it is missing or damaged.
    pub state: State,
    /// What was wrong with the file that was there, where something was: the
    /// start then took it to name nothing, and this says why, for the log.
    pub damaged: Option<String>,
    /// The generation a file written by an older build held, which this
    /// start read and dropped, for the log: there is no generation now.
    pub dropped_generation: Option<u32>,
}

/// Reads the state file at `path`.
///
/// A file that does not exist names nothing: a first start, which nobody
/// commanded. One that exists and cannot be read, or fails its CRC, is
/// reported in [`Found::damaged`] and also names nothing: a vehicle that
/// would not start because its state file was damaged would be lost to a
/// file. A damaged file says nothing of the next start, which is then taken
/// for one nobody commanded: a vehicle that claimed a commanded start it
/// could not vouch for would be telling the ground its definitions had
/// changed when they might not have.
///
/// # Arguments
///
/// * `path` - The state file, which the configuration's `state_file` names.
///
/// # Returns
///
/// What it holds, what was wrong with it, and the generation an older
/// build's file held.
pub fn read(path: &Path) -> Found {
    let (state, damaged) = match tcsos::fs::read(path) {
        Ok(text) => match state_in(&text) {
            Ok(said) => (said, None),
            Err(why) => {
                (Said::default(), Some(format!("{}: {why}", path.display())))
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            (Said::default(), None)
        }
        Err(e) => (
            Said::default(),
            Some(format!("{} cannot be read: {e}", path.display())),
        ),
    };
    Found {
        state: state.state,
        damaged,
        dropped_generation: state.generation,
    }
}

/// Says in the state file at `path` that the next start is one the ground
/// commanded, and pushes the configuration whose checksum is `target`, which
/// the next child then runs: an armed RESTART naming it has been taken.
///
/// Its attempts start at nought, since nothing has yet tried it, and the
/// configuration it was pushed above stays beneath it, to be run again if it
/// fails.
///
/// Written before the RESTART is answered and before anything stops, so that
/// a RESTART the file cannot say is refused and nothing restarts: a vehicle
/// that came back on the old configuration, saying in its log that it had
/// not been commanded, would tell whoever reads it the RESTART had not
/// taken, when it had.
///
/// Allocates, in the command loop: the file is read, its text made, and the
/// path formatted into a refusal. Once, for a RESTART, after which the
/// process ends, and there is no way to write a file without the bytes it
/// holds.
///
/// # Arguments
///
/// * `path`   - The state file.
/// * `target` - The checksum the RESTART named.
///
/// # Errors
///
/// Why the file could not be written, naming it and the attribute.
pub fn commanded_restart(
    path: &Path,
    target: ConfigDigest,
) -> Result<(), String> {
    update(path, |state| {
        if BOTTOM.get() == Some(&target) && state.stack.is_empty() {
            state.trials = 0;
        } else {
            state.push(target);
        }
        state.commanded = true;
    })
    .map(|_| ())
}

/// Says in the state file at `path` that the configuration running, whose
/// checksum is `running`, has been heard: a CONNECT carrying it has been
/// taken. It is pushed, unless it is the top already or is the bottom with
/// nothing above it, and its attempts go back to nought.
///
/// Allocates, in the command loop: the file is read and its text made.
/// Once a start, at its match, and a match that could not be kept would be
/// one the next start did not know of.
///
/// # Arguments
///
/// * `path`    - The state file.
/// * `running` - The running configuration's checksum.
///
/// # Errors
///
/// Why the file could not be written, naming it and the attribute.
pub fn confirmed(path: &Path, running: ConfigDigest) -> Result<(), String> {
    update(path, |state| {
        if !(BOTTOM.get() == Some(&running) && state.stack.is_empty()) {
            state.push(running);
        }
        state.trials = 0;
    })
    .map(|_| ())
}

/// Changes the state file at `path`, under its short lock: reads it, lets
/// `change` change what it holds, and writes it whole.
///
/// A damaged or missing file is changed from nothing, as [`read`] reads it.
///
/// # Arguments
///
/// * `path`   - The state file.
/// * `change` - What to do to what it holds.
///
/// # Returns
///
/// What it holds now.
///
/// # Errors
///
/// Why the lock could not be taken or the file written, naming it and the
/// attribute.
pub fn update(
    path: &Path,
    change: impl FnOnce(&mut State),
) -> Result<State, String> {
    let _held = short_lock(path)?;
    let mut state = read(path).state;
    change(&mut state);
    write(path, &state)?;
    Ok(state)
}

/// The file the short lock is taken on: beside the state file, with
/// `.update` added. Never removed, because a lock file removed while
/// another waits on it is two lock files.
///
/// # Arguments
///
/// * `path` - The state file.
///
/// # Returns
///
/// The lock file's path.
pub fn short_lock_of(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".update");
    path.with_file_name(name)
}

/// Takes the short lock on the state file at `path`, waiting for whoever
/// holds it.
///
/// # Arguments
///
/// * `path` - The state file.
///
/// # Returns
///
/// The lock file, held until it is dropped.
///
/// # Errors
///
/// Why the lock could not be made or taken, naming it and the attribute.
fn short_lock(path: &Path) -> Result<File, String> {
    let lock = short_lock_of(path);
    let cannot = |e: std::io::Error| {
        format!(
            "the state file's short lock {} cannot be taken ({e}): \
             state_file in the configuration's tcspecial section must name \
             a file in a directory tcspecial can write",
            lock.display()
        )
    };
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock)
        .map_err(cannot)?;
    loop {
        match file.lock() {
            Ok(()) => return Ok(file),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(cannot(e)),
        }
    }
}

/// What a state file's text says, a generation an older build wrote
/// included.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Said {
    state: State,
    /// The generation an older build's file held, which is dropped.
    generation: Option<u32>,
}

/// Makes the state file's text.
///
/// # Arguments
///
/// * `state` - What it holds.
///
/// # Returns
///
/// The text, its CRC line last.
fn text_of(state: &State) -> Vec<u8> {
    let mut text = String::from(
        "# tcspecial's state, which it writes on every start: do not edit.\n",
    );
    for digest in &state.stack {
        text.push_str(&format!("stack {digest}\n"));
    }
    if !state.stack.is_empty() {
        text.push_str(&format!("trials {}\n", state.trials));
    }
    if state.commanded {
        text.push_str("next_start commanded\n");
    }
    let mut text = text.into_bytes();
    let crc = crc16(&text);
    text.extend_from_slice(format!("crc16 {crc:#06x}\n").as_bytes());
    text
}

/// Reads the state a state file's text holds.
///
/// # Arguments
///
/// * `text` - The file's text, CRC line and all.
///
/// # Returns
///
/// What it says, and the generation an older build's file held.
///
/// # Errors
///
/// Why it holds no state, said in words: no CRC line last, a CRC that is
/// not one or does not match, text that is not UTF-8, or a line that is not
/// state.
fn state_in(text: &[u8]) -> Result<Said, String> {
    const CRC: &[u8] = b"crc16 ";
    // The CRC's line is the last, and covers every byte before it.
    let body_end = text
        .windows(CRC.len())
        .rposition(|window| window == CRC)
        .filter(|&at| at == 0 || text[at - 1] == b'\n')
        .ok_or(
            "it has no crc16 line, so it was cut short or is not a state \
                file",
        )?;
    let (body, crc_line) = text.split_at(body_end);
    let crc_line = std::str::from_utf8(crc_line)
        .map_err(|_| "its crc16 line is not text")?
        .trim_end_matches('\n');
    let said = crc_line[CRC.len()..].trim();
    let said = said
        .strip_prefix("0x")
        .and_then(|hex| u16::from_str_radix(hex, 16).ok())
        .ok_or_else(|| format!("its crc16 line says {said:?}, not a CRC"))?;
    let computed = crc16(body);
    if said != computed {
        return Err(format!(
            "its CRC is {said:#06x} and what it holds has {computed:#06x}: it \
             was damaged or edited"
        ));
    }
    let body = std::str::from_utf8(body).map_err(|_| "it is not text")?;
    let checksum = |what: &str, said: &str| {
        ConfigDigest::of_hex(said.trim()).ok_or_else(|| {
            format!("its {what} is {said:?}, not a configuration's checksum")
        })
    };
    let count = |what: &str, said: &str| {
        said.trim()
            .parse::<u32>()
            .map_err(|_| format!("its {what} is {said:?}, not a count"))
    };
    let mut read = Said::default();
    // What a file written before the stack named, read as the stack of the
    // two, the confirmed one beneath.
    let (mut next, mut confirmed) = (None, None);
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match line.split_once(' ') {
            Some(("stack", said)) => {
                read.state.stack.push(checksum("stack", said)?)
            }
            Some(("next", said)) => next = Some(checksum("next", said)?),
            Some(("trials", said)) => {
                read.state.trials = count("trials", said)?
            }
            Some(("confirmed", said)) => {
                confirmed = Some(checksum("confirmed", said)?)
            }
            Some(("next_start", "commanded")) => read.state.commanded = true,
            // What a build that counted generations wrote, read so that its
            // file is not taken for a damaged one and dropped: there is no
            // generation now.
            Some(("generation", said)) => {
                read.generation = Some(count("generation", said)?)
            }
            _ => return Err(format!("it says {line:?}, which is not state")),
        }
    }
    if read.state.stack.is_empty() {
        // The trials were next's, which is the top however it got there.
        let trials = read.state.trials;
        read.state.stack.extend(confirmed);
        if let Some(next) = next {
            read.state.push(next);
        }
        read.state.trials = trials;
    }
    Ok(read)
}

/// Writes the state file at `path`, whole or not at all.
///
/// Written to a file beside it, flushed and renamed over it, and the
/// directory flushed after.
///
/// # Arguments
///
/// * `path`  - The state file.
/// * `state` - What it is to hold.
///
/// # Returns
///
/// Nothing, once the file and its directory have reached storage.
///
/// # Errors
///
/// What could not be done, naming the file and the setting to change, where
/// it cannot be written, renamed or flushed.
pub fn write(path: &Path, state: &State) -> Result<(), String> {
    let cannot = |what: &str, e: std::io::Error| {
        format!(
            "the state file {} cannot be {what} ({e}): state_file in the \
             configuration's tcspecial section must name a file in a \
             directory tcspecial can write, on storage that survives a power \
             cycle",
            path.display()
        )
    };
    let beside = beside(path);
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&beside)
        .map_err(|e| cannot("written", e))?;
    file.write_all(&text_of(state))
        .and_then(|()| file.sync_all())
        .map_err(|e| cannot("written", e))?;
    drop(file);
    tcsos::fs::rename(&beside, path).map_err(|e| cannot("replaced", e))?;
    // The rename is in the directory, so the directory is what has to reach
    // storage for it to be the file that is there after a power failure.
    let directory = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    File::open(directory)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| cannot("flushed to storage", e))
}

/// The file a new state is written to before it is renamed over `path`: in
/// the same directory, because a rename is whole only within one file system.
fn beside(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".new");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: ConfigDigest = ConfigDigest([0x11; 16]);
    const TWO: ConfigDigest = ConfigDigest([0x22; 16]);

    /// A file that is not there names nothing, and is no damage.
    #[test]
    fn a_missing_file_names_nothing() {
        let dir = tempfile::tempdir().expect("a directory");
        let found = read(&dir.path().join("tcspecial.state"));
        assert_eq!(found.state, State::default());
        assert_eq!(found.damaged, None);
        assert_eq!(found.dropped_generation, None);
    }

    /// What is written reads back, and is text a person can read, holding
    /// no generation: nothing in it counts starts but the trials. The stack
    /// is written bottom first.
    #[test]
    fn what_is_written_reads_back_and_is_text() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("tcspecial.state");
        let state = State {
            stack: vec![ONE, TWO],
            trials: 2,
            commanded: true,
        };
        write(&path, &state).expect("written");
        assert_eq!(read(&path).state, state);
        assert_eq!(read(&path).state.top(), Some(TWO));
        let text = tcsos::fs::read_to_string(&path).expect("it is text");
        assert!(
            text.contains(&format!("\nstack {ONE}\nstack {TWO}\ntrials 2\n")),
            "{text}"
        );
        assert!(!text.contains("generation"), "{text}");
        assert!(
            text.lines().last().unwrap().starts_with("crc16 0x"),
            "{text}"
        );
        assert!(!beside(&path).exists(), "nothing is left beside it");
    }

    /// A RESTART taken pushes its target, with no trials yet, and says it is
    /// commanded, keeping what it was pushed above; a matching CONNECT on
    /// the top pushes nothing and sets the trials back to nought, and one on
    /// a configuration that is not the top pushes it.
    #[test]
    fn a_restart_pushes_its_target_and_a_connect_resets_the_trials() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("tcspecial.state");
        write(
            &path,
            &State {
                stack: vec![ONE],
                trials: 1,
                commanded: false,
            },
        )
        .expect("written");

        commanded_restart(&path, TWO).expect("said");
        assert_eq!(
            read(&path).state,
            State {
                stack: vec![ONE, TWO],
                trials: 0,
                commanded: true,
            }
        );

        update(&path, |state| {
            state.trials = 2;
            state.commanded = false;
        })
        .expect("written");
        confirmed(&path, TWO).expect("confirmed");
        assert_eq!(
            read(&path).state,
            State {
                stack: vec![ONE, TWO],
                trials: 0,
                commanded: false,
            }
        );

        let three = ConfigDigest([0x33; 16]);
        confirmed(&path, three).expect("confirmed");
        assert_eq!(read(&path).state.stack, vec![ONE, TWO, three]);
    }

    /// A file written before the stack, naming a next and a confirmed
    /// configuration, is read as the stack of the two, the confirmed one
    /// beneath, with next's trials; one that names only one of them is the
    /// stack of that one.
    #[test]
    fn a_file_from_before_the_stack_is_read_as_one() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("tcspecial.state");
        let old = |body: String| {
            let mut text = format!(
                "# tcspecial's state, which it writes on every start: do not \
                 edit.\n{body}"
            )
            .into_bytes();
            let crc = crc16(&text);
            text.extend_from_slice(format!("crc16 {crc:#06x}\n").as_bytes());
            text
        };
        tcsos::fs::write(
            &path,
            old(format!("next {TWO}\ntrials 2\nconfirmed {ONE}\n")),
        )
        .expect("written");
        assert_eq!(
            read(&path).state,
            State {
                stack: vec![ONE, TWO],
                trials: 2,
                commanded: false,
            }
        );
        tcsos::fs::write(
            &path,
            old(format!("next {ONE}\ntrials 1\nconfirmed {ONE}\n")),
        )
        .expect("written");
        assert_eq!(read(&path).state.stack, vec![ONE]);
        assert_eq!(read(&path).state.trials, 1);
    }

    /// Two changes made at once are both kept: each is made under the short
    /// lock, so neither reads what the other is about to replace.
    #[test]
    fn changes_made_at_once_are_both_kept() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("tcspecial.state");
        let threads: Vec<_> = (0..8u8)
            .map(|at| {
                let path = path.clone();
                tcsos::thread::spawn(move || {
                    update(&path, |state| {
                        state.stack.push(ConfigDigest([at; 16]))
                    })
                    .expect("changed")
                })
            })
            .collect();
        for thread in threads {
            thread.join().expect("it ends");
        }
        assert_eq!(read(&path).state.stack.len(), 8);
    }

    /// A file an older build wrote, which counted generations, is read: its
    /// generation is dropped and said, and what else it says is kept.
    #[test]
    fn an_older_builds_file_has_its_generation_dropped() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("tcspecial.state");
        let mut old = b"# tcspecial's state, which it writes on every start: \
do not edit.\ngeneration 7\nnext_start commanded\n"
            .to_vec();
        let crc = crc16(&old);
        old.extend_from_slice(format!("crc16 {crc:#06x}\n").as_bytes());
        tcsos::fs::write(&path, &old).expect("written");

        let found = read(&path);
        assert_eq!(found.dropped_generation, Some(7));
        assert_eq!(found.damaged, None, "not a damaged file");
        assert!(found.state.commanded, "what else it says is read");
        assert_eq!(found.state.top(), None, "and it names no configuration");
    }

    /// A file damaged on its storage, edited by hand or cut short is said to
    /// be, and names nothing, rather than the vehicle not starting.
    #[test]
    fn a_damaged_file_names_nothing_and_says_why() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("tcspecial.state");
        commanded_restart(&path, ONE).expect("written");
        let good = tcsos::fs::read(&path).expect("it reads");
        let edited = String::from_utf8(good.clone())
            .unwrap()
            .replace(&ONE.to_string(), &TWO.to_string());
        let cut = good[..good.len() - 10].to_vec();
        for (damage, said) in [
            (edited.into_bytes(), "CRC"),
            (cut, "crc16"),
            (b"nothing like it".to_vec(), "crc16"),
        ] {
            tcsos::fs::write(&path, &damage).expect("it is damaged");
            let found = read(&path);
            assert_eq!(found.state, State::default(), "it names nothing");
            let why = found.damaged.expect("and says why");
            assert!(why.contains(said), "{why}");
            assert!(why.contains("tcspecial.state"), "{why}");
        }
    }

    /// A file that cannot be written is refused, naming the attribute to
    /// change.
    #[test]
    fn a_state_file_that_cannot_be_written_is_refused() {
        let why = write(
            Path::new("/nonexistent/directory/tcspecial.state"),
            &State::default(),
        )
        .expect_err("there is nowhere to write it");
        assert!(why.contains("state_file"), "{why}");
        assert!(why.contains("/nonexistent/directory"), "{why}");
    }
}
