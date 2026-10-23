//! What tcsgui remembers between sessions, and writing it down.
//!
//! What the settings are and how the file is read are tcsdiagram's, which
//! tcspecial's visual window reads the same file with -- see
//! `tcsdiagram::settings` -- and they are all here under the names they had.
//! What is the editor's alone is the writing: making the directory, and the
//! file, the first time and every time something is moved.

pub use tcsdiagram::settings::*;

/// The directory the editor keeps its own files in, made if it is not there.
///
/// Made on the way in rather than on the first write, so that somebody who
/// wants to look at the file or put one there finds the directory waiting.
pub fn directory() -> Option<std::path::PathBuf> {
    let directory = home()?;
    if let Err(why) = std::fs::create_dir_all(&directory) {
        eprintln!(
            "tcsgui: {} cannot be made, so settings will not be kept: {why}",
            directory.display()
        );
        return None;
    }
    Some(directory)
}

/// Read the settings, or the defaults.
///
/// A file that is not there yet is written, with the defaults in it. So the
/// first run leaves a settings file whose comments say what may be put in it,
/// which is a better answer to "where do I set this" than an empty directory
/// and a manual. See [`read`] for what is read.
pub fn load() -> Settings {
    directory();
    let found = read();
    if found.missing {
        save(found.settings);
    }
    found.settings
}

/// Write the settings.
///
/// Atomically, with the same writer the configuration is written with: a
/// settings file half replaced by a crash would be a file the next session
/// complains about, and the complaint would be the editor's own doing.
pub fn save(settings: Settings) {
    if directory().is_none() {
        return;
    }
    let Some(path) = path() else {
        return;
    };
    let (moc, vehicle) = shares(settings.moc_percent, settings.vehicle_percent);
    let (list_moc, list_vehicle) = shares(settings.list_moc_percent, settings.list_vehicle_percent);
    if let Err(why) = crate::save::write_atomically(
        &path,
        written(moc, vehicle, list_moc, list_vehicle).as_bytes(),
    ) {
        eprintln!("tcsgui: {} cannot be written: {why}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The settings live under the home directory, in their own place.
    #[test]
    fn the_settings_are_in_a_directory_of_their_own() {
        let where_ = tempfile::tempdir().expect("a directory");
        // One process, one HOME: this is the only test that changes it, and
        // it puts it back.
        let was = std::env::var_os("HOME");
        std::env::set_var("HOME", where_.path());

        let path = path().expect("a settings path");
        assert_eq!(path, where_.path().join(".tcsgui/preferences.xml"));
        // Made on the way in, before anything is written.
        directory();
        assert!(
            where_.path().join(".tcsgui").is_dir(),
            "the directory should be made automatically"
        );

        // And the first read leaves a file behind, with the defaults in it:
        // an empty directory is a worse answer to "where do I set this" than
        // a file whose comments say.
        assert!(!path.exists());
        assert_eq!(load(), Settings::default());
        assert!(path.exists(), "the settings file should be created");
        assert_eq!(load(), Settings::default());

        // And a round trip through the file itself.
        save(Settings {
            moc_percent: 20,
            vehicle_percent: 40,
            list_moc_percent: 15,
            list_vehicle_percent: 45,
        });
        assert_eq!(load().moc_percent, 20);
        assert_eq!(load().vehicle_percent, 40);

        // A file nobody can make sense of leaves the defaults standing.
        std::fs::write(&path, "not xml at all").expect("a file");
        assert_eq!(load(), Settings::default());

        // The file was called config.xml before it was called
        // preferences.xml, and the rename does not forget what somebody had
        // already arranged: with only the old name there, the old name is
        // read and the new one is written from it.
        std::fs::remove_file(&path).expect("the file goes");
        let older = where_.path().join(".tcsgui/config.xml");
        std::fs::write(
            &older,
            "<settings><vehicle_pane_percent>30</vehicle_pane_percent></settings>",
        )
        .expect("a file");
        assert_eq!(load().vehicle_percent, 30 - DEFAULT_MOC);
        assert!(
            path.exists(),
            "the value should be carried into the new name"
        );
        assert_eq!(
            load().vehicle_percent,
            30 - DEFAULT_MOC,
            "and read from it afterwards"
        );
        assert!(
            older.exists(),
            "the old file is left for its owner to delete"
        );

        // The current name wins where both are there, it being the one that
        // is written.
        save(Settings {
            moc_percent: DEFAULT_MOC,
            vehicle_percent: 60,
            ..Settings::default()
        });
        assert_eq!(load().vehicle_percent, 60);

        match was {
            Some(was) => std::env::set_var("HOME", was),
            None => std::env::remove_var("HOME"),
        }
    }
}
