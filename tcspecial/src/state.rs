//! The state file: what tcspecial keeps across a restart.
//!
//! For now that is the generation number, which every start of tcspecial adds
//! one to before it sends its first beacon, so that the ground can tell from a
//! beacon alone that the vehicle has started again. It is kept in the file the
//! `tcspecial` section's `state_file` names, on storage that survives a power
//! cycle, because a count kept in memory would start again at every crash and
//! every power cycle -- which are the restarts the ground most needs to know
//! about.
//!
//! The file is a few lines of text, so that somebody at a console can read
//! it, and ends with a CRC of everything before it:
//!
//! ```text
//! # tcspecial's state, which it writes on every start: do not edit.
//! generation 7
//! crc16 0x1a2b
//! ```
//!
//! It is written whole to a file beside it, flushed, and renamed over it, and
//! the directory is flushed after: a power failure part way through leaves
//! the old file or the new one, never half of each. See "The generation
//! number and the state file" in `docs/design.rst`.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use tcslibgs::crc::crc16;

/// What a start found in the state file and what it made of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    /// This start's generation, already written to the file.
    pub generation: u32,
    /// What was wrong with the file that was there, where something was: the
    /// generation then started again from one, and this says why, for the
    /// log.
    pub damaged: Option<String>,
}

/// Count this start: read the generation the state file at `path` holds, add
/// one, and write it back before anything is sent.
///
/// A file that does not exist is a first start, generation 1. One that
/// exists and cannot be read, or fails its CRC, is reported in
/// [`Started::damaged`] and the count starts again from 1: a vehicle that
/// would not start because a counter was damaged would be lost to a counter,
/// and the ground judges a restart by any change of generation, so a count
/// that went back is still seen.
///
/// Where the new count cannot be written this is an error, and the vehicle
/// does not start: one that beaconed a generation it had not kept would
/// beacon the same one after its next start, and the ground would not see
/// that start at all.
pub fn start(path: &Path) -> Result<Started, String> {
    let (before, damaged) = match std::fs::read(path) {
        Ok(text) => match generation_in(&text) {
            Ok(generation) => (generation, None),
            Err(why) => (0, Some(format!("{}: {why}", path.display()))),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (0, None),
        Err(e) => (0, Some(format!("{} cannot be read: {e}", path.display()))),
    };
    // Nought is never a generation, so that a beacon of generation 0 is one
    // from a build that kept none, and never a count that wrapped.
    let generation = match before.wrapping_add(1) {
        0 => 1,
        next => next,
    };
    write(path, generation)?;
    Ok(Started {
        generation,
        damaged,
    })
}

/// The file's text for `generation`, its CRC last.
fn text_of(generation: u32) -> Vec<u8> {
    let mut text = format!(
        "# tcspecial's state, which it writes on every start: do not edit.\n\
         generation {generation}\n"
    )
    .into_bytes();
    let crc = crc16(&text);
    text.extend_from_slice(format!("crc16 {crc:#06x}\n").as_bytes());
    text
}

/// The generation a state file's text holds, or why it holds none.
fn generation_in(text: &[u8]) -> Result<u32, String> {
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
    let mut generation = None;
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match line.split_once(' ') {
            Some(("generation", number)) => {
                generation =
                    Some(number.trim().parse::<u32>().map_err(|_| {
                        format!("its generation is {number:?}, not a count")
                    })?);
            }
            _ => return Err(format!("it says {line:?}, which is not state")),
        }
    }
    generation.ok_or_else(|| "it holds no generation".to_string())
}

/// Write `generation` to the state file at `path`, whole or not at all.
fn write(path: &Path, generation: u32) -> Result<(), String> {
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
    file.write_all(&text_of(generation))
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
    fn a_first_start_is_generation_one_and_each_start_adds_one() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("tcspecial.state");
        for expected in 1..=3 {
            let started = start(&path).expect("it starts");
            assert_eq!(started.generation, expected);
            assert_eq!(started.damaged, None);
        }
        assert!(!beside(&path).exists(), "nothing is left beside it");
    }

    #[test]
    fn the_file_is_text_a_person_can_read() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("tcspecial.state");
        start(&path).expect("it starts");
        let text = std::fs::read_to_string(&path).expect("it is text");
        assert!(text.contains("\ngeneration 1\n"), "{text}");
        assert!(
            text.lines().last().unwrap().starts_with("crc16 0x"),
            "{text}"
        );
    }

    /// A file damaged on its storage, edited by hand or cut short is said to
    /// be, and the count starts again rather than the vehicle not starting.
    #[test]
    fn a_damaged_file_starts_the_count_again_and_says_why() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("tcspecial.state");
        for _ in 0..4 {
            start(&path).expect("it starts");
        }
        let good = std::fs::read(&path).expect("it reads");
        let edited = String::from_utf8(good.clone())
            .unwrap()
            .replace("generation 4", "generation 9");
        let cut = good[..good.len() - 10].to_vec();
        for (damage, said) in [
            (edited.into_bytes(), "CRC"),
            (cut, "crc16"),
            (b"nothing like it".to_vec(), "crc16"),
        ] {
            std::fs::write(&path, &damage).expect("it is damaged");
            let started = start(&path).expect("it still starts");
            assert_eq!(started.generation, 1);
            let why = started.damaged.expect("and says why");
            assert!(why.contains(said), "{why}");
            assert!(why.contains("tcspecial.state"), "{why}");
        }
    }

    /// The count never wraps to nought, which is no generation.
    #[test]
    fn the_count_wraps_past_nought() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("tcspecial.state");
        std::fs::write(&path, text_of(u32::MAX)).expect("written");
        assert_eq!(start(&path).expect("it starts").generation, 1);
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
