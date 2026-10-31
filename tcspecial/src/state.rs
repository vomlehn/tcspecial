//! The state file: what tcspecial keeps across a restart.
//!
//! For now that is whether the next start is one the ground commanded, which
//! the startup summary then says. (The beacon's restart flag does not depend
//! on it: every start begins with the flag set.) It is kept in the file the
//! `tcspecial` section's `state_file` names, on storage that survives a power
//! cycle, because what is kept in memory is gone at every crash and every
//! power cycle -- which are the restarts somebody reading the log afterwards
//! most needs told apart from a commanded one.
//!
//! There is no generation number. A start of tcspecial is told by its
//! beacon's restart flag and its number going back, and which configuration
//! it runs by its checksum; nothing in the state file counts starts. A file
//! written by a build that kept one still has a `generation` line, which is
//! read and dropped, and the start says so in its log.
//!
//! The file is a few lines of text, so that somebody at a console can read
//! it, and ends with a CRC of everything before it:
//!
//! ```text
//! # tcspecial's state, which it writes on every start: do not edit.
//! next_start commanded
//! crc16 0x1a2b
//! ```
//!
//! The `next_start` line is there only between a RESTART's being taken and
//! the next start, which reads it and writes the file again without it.
//!
//! It is written whole to a file beside it, flushed, and renamed over it, and
//! the directory is flushed after: a power failure part way through leaves
//! the old file or the new one, never half of each. See "The state
//! file" in `docs/design.rst`.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use tcslibgs::crc::crc16;

/// What a start found in the state file and what it made of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    /// Whether the ground commanded this start: the start before took an
    /// armed RESTART and said so in the file before it went. See
    /// [`commanded_restart`]. The startup summary says it, for somebody
    /// reading a log after a restart; the beacon's restart flag does not
    /// depend on it, since every start begins with the flag set.
    pub commanded: bool,
    /// What was wrong with the file that was there, where something was: the
    /// start then took it to say nothing, and this says why, for the log.
    pub damaged: Option<String>,
    /// The generation a file written by an older build held, which this
    /// start read and dropped, for the log: there is no generation now.
    pub dropped_generation: Option<u32>,
}

/// Reads what the state file at `path` says of this start, and writes it
/// back without the `next_start` line, before anything is sent.
///
/// A file that does not exist says nothing: a first start, which nobody
/// commanded. One that exists and cannot be read, or fails its CRC, is
/// reported in [`Started::damaged`] and also says nothing: a vehicle that
/// would not start because its state file was damaged would be lost to a
/// file.
///
/// Where the file cannot be written this is an error, and the vehicle does
/// not start: one that could not write it could not say a RESTART was
/// commanded either, and a later start of its own would read whatever the
/// old file said.
///
/// The file is written without the `next_start` line, whatever it held, so
/// that a later start nobody commanded -- a crash of this one -- is not taken
/// for a commanded one. A damaged file says nothing of the next start, and
/// the start is then taken for one nobody commanded: a vehicle that claimed a
/// commanded start it could not vouch for would be telling the ground its
/// definitions had changed when they might not have.
///
/// # Arguments
///
/// * `path` - The state file, which the configuration's `state_file` names.
///
/// # Returns
///
/// Whether the ground commanded this start, what was wrong with the file
/// that was there, and the generation an older build's file held.
///
/// # Errors
///
/// What is wrong, naming the file and the setting to change, where the file
/// cannot be written.
pub fn start(path: &Path) -> Result<Started, String> {
    let (state, damaged) = match std::fs::read(path) {
        Ok(text) => match state_in(&text) {
            Ok(state) => (state, None),
            Err(why) => {
                (State::default(), Some(format!("{}: {why}", path.display())))
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            (State::default(), None)
        }
        Err(e) => (
            State::default(),
            Some(format!("{} cannot be read: {e}", path.display())),
        ),
    };
    write(path, false)?;
    Ok(Started {
        commanded: state.commanded,
        damaged,
        dropped_generation: state.generation,
    })
}

/// Says in the state file at `path` that the next start is one the ground
/// commanded: an armed RESTART has been taken by the start running now.
///
/// Written before the RESTART is answered and before anything stops, so that
/// a RESTART the file cannot say is refused and nothing restarts: a vehicle
/// that came back saying in its log that it had not been commanded would
/// tell whoever reads it the RESTART had not taken, when it had.
///
/// Allocates, in the command loop: the file's text is made, and the path is
/// formatted into a refusal. Once, for a RESTART, after which the process
/// ends, and there is no way to write a file without the bytes it holds.
pub fn commanded_restart(path: &Path) -> Result<(), String> {
    write(path, true)
}

/// What a state file holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct State {
    /// Whether the next start is commanded.
    commanded: bool,
    /// The generation an older build's file held, which is dropped.
    generation: Option<u32>,
}

/// Makes the state file's text.
///
/// # Arguments
///
/// * `commanded` - Whether it says the next start is commanded.
///
/// # Returns
///
/// The text, its CRC line last.
fn text_of(commanded: bool) -> Vec<u8> {
    let next = if commanded {
        "next_start commanded\n"
    } else {
        ""
    };
    let mut text = format!(
        "# tcspecial's state, which it writes on every start: do not edit.\n\
         {next}"
    )
    .into_bytes();
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
/// Whether the next start is commanded, and the generation an older
/// build's file held.
///
/// # Errors
///
/// Why it holds no state, said in words: no CRC line last, a CRC that is
/// not one or does not match, text that is not UTF-8, or a line that is not
/// state.
fn state_in(text: &[u8]) -> Result<State, String> {
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
    let mut state = State::default();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match line.split_once(' ') {
            // What a build that counted generations wrote, read so that its
            // file is not taken for a damaged one and dropped: there is no
            // generation now.
            Some(("generation", number)) => {
                state.generation =
                    Some(number.trim().parse::<u32>().map_err(|_| {
                        format!("its generation is {number:?}, not a count")
                    })?);
            }
            Some(("next_start", "commanded")) => state.commanded = true,
            _ => return Err(format!("it says {line:?}, which is not state")),
        }
    }
    Ok(state)
}

/// Writes the state file at `path`, saying whether the next start is
/// commanded, whole or not at all.
///
/// Written to a file beside it, flushed and renamed over it, and the
/// directory flushed after.
///
/// # Arguments
///
/// * `path`      - The state file.
/// * `commanded` - Whether to say the next start is commanded.
///
/// # Returns
///
/// Nothing, once the file and its directory have reached storage.
///
/// # Errors
///
/// What could not be done, naming the file and the setting to change, where
/// it cannot be written, renamed or flushed.
fn write(path: &Path, commanded: bool) -> Result<(), String> {
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
    file.write_all(&text_of(commanded))
        .and_then(|()| file.sync_all())
        .map_err(|e| cannot("written", e))?;
    drop(file);
    std::fs::rename(&beside, path).map_err(|e| cannot("replaced", e))?;
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

    #[test]
    fn a_first_start_finds_nothing_and_leaves_a_file() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("tcspecial.state");
        for _ in 0..3 {
            let started = start(&path).expect("it starts");
            assert!(!started.commanded);
            assert_eq!(started.damaged, None);
            assert_eq!(started.dropped_generation, None);
        }
        assert!(path.exists());
        assert!(!beside(&path).exists(), "nothing is left beside it");
    }

    /// The file holds no generation: nothing in it counts starts.
    #[test]
    fn the_file_is_text_a_person_can_read_and_counts_nothing() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("tcspecial.state");
        start(&path).expect("it starts");
        let text = std::fs::read_to_string(&path).expect("it is text");
        assert!(!text.contains("generation"), "{text}");
        assert!(
            text.lines().last().unwrap().starts_with("crc16 0x"),
            "{text}"
        );
    }

    /// A file an older build wrote, which counted generations, is read: its
    /// generation is dropped and said, what else it says is kept, and the
    /// file is written again without it.
    #[test]
    fn an_older_builds_file_has_its_generation_dropped() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("tcspecial.state");
        let mut old = b"# tcspecial's state, which it writes on every start: \
do not edit.\ngeneration 7\nnext_start commanded\n"
            .to_vec();
        let crc = crc16(&old);
        old.extend_from_slice(format!("crc16 {crc:#06x}\n").as_bytes());
        std::fs::write(&path, &old).expect("written");

        let started = start(&path).expect("it starts");
        assert_eq!(started.dropped_generation, Some(7));
        assert_eq!(started.damaged, None, "not a damaged file");
        assert!(started.commanded, "what else it says is read");
        let text = std::fs::read_to_string(&path).expect("it is text");
        assert!(!text.contains("generation"), "{text}");
    }

    /// A file damaged on its storage, edited by hand or cut short is said to
    /// be, and says nothing, rather than the vehicle not starting.
    #[test]
    fn a_damaged_file_says_nothing_and_says_why() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("tcspecial.state");
        commanded_restart(&path).expect("written");
        let good = std::fs::read(&path).expect("it reads");
        let edited = String::from_utf8(good.clone())
            .unwrap()
            .replace("next_start commanded", "next_start commandeered");
        let cut = good[..good.len() - 10].to_vec();
        for (damage, said) in [
            (edited.into_bytes(), "CRC"),
            (cut, "crc16"),
            (b"nothing like it".to_vec(), "crc16"),
        ] {
            std::fs::write(&path, &damage).expect("it is damaged");
            let started = start(&path).expect("it still starts");
            assert!(!started.commanded, "a damaged file commands nothing");
            let why = started.damaged.expect("and says why");
            assert!(why.contains(said), "{why}");
            assert!(why.contains("tcspecial.state"), "{why}");
        }
    }

    /// A start is commanded when the start before said so, and only that
    /// one: the next start nobody commanded is not, though nothing was
    /// written in between.
    #[test]
    fn only_the_start_after_a_commanded_restart_is_commanded() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("tcspecial.state");
        let first = start(&path).expect("it starts");
        assert!(!first.commanded, "nobody commanded a first start");

        commanded_restart(&path).expect("it is said");
        let text = std::fs::read_to_string(&path).expect("it is text");
        assert!(text.contains("\nnext_start commanded\n"), "{text}");

        let second = start(&path).expect("it starts");
        assert!(second.commanded, "the start the RESTART asked for");

        let third = start(&path).expect("it starts");
        assert!(!third.commanded, "a crash after it is not commanded");
    }

    /// A file that cannot be written stops the start, naming the attribute
    /// to change.
    #[test]
    fn a_state_file_that_cannot_be_written_is_refused() {
        let why = start(Path::new("/nonexistent/directory/tcspecial.state"))
            .expect_err("there is nowhere to write it");
        assert!(why.contains("state_file"), "{why}");
        assert!(why.contains("/nonexistent/directory"), "{why}");
    }
}
