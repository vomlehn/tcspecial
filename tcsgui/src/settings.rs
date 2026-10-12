//! What tcsgui remembers between sessions, in `$HOME/.tcsgui/config.xml`.
//!
//! One setting so far: how much of the working pane the vehicle working pane
//! takes, as a percentage. The vehicle working pane is always the left-hand
//! one, so a percentage and that sentence are the whole of where the divider
//! is.
//!
//! Here rather than in the configuration, which is the distinction worth
//! stating. A node's position is in the configuration because it is part of
//! the diagram -- two people reading one configuration should see one drawing,
//! and `design.rst` says why. How the canvas is divided is not part of the
//! diagram: it is a working preference of the person at the keyboard, and
//! writing it into the configuration would make resizing a pane a change to a
//! file that flies.
//!
//! A percentage rather than pixels for the same reason: a percentage is the
//! thing the person chose, where a pixel count is how this build happens to
//! lay a canvas out.
//!
//! Nothing here can stop the editor. A settings file that cannot be read, or a
//! directory that cannot be made, leaves the default in place and says so on
//! the error output -- an editor that refused to start because it could not
//! remember a pane width would be an editor traded for a preference.

use std::path::PathBuf;

/// The least of the working pane either side may be reduced to, as a
/// percentage.
///
/// Asked for: neither pane may be squeezed below a twentieth of the working
/// pane. A pane with nothing visible in it is a pane whose nodes cannot be
/// reached, and a divider dragged to the edge is easy to do by accident and
/// hard to understand afterwards.
pub const LEAST: i32 = 5;

/// And the most, which is the same rule from the other side.
pub const MOST: i32 = 100 - LEAST;

/// How the canvas is divided until somebody divides it differently.
///
/// Three quarters to the vehicle: its own graph -- the computations, the log,
/// the ground link -- is most of a configuration, and what it carries this
/// flight is a column down one side.
pub const DEFAULT: i32 = 75;

/// What the editor remembers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// How much of the working pane the vehicle working pane takes.
    pub vehicle_percent: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            vehicle_percent: DEFAULT,
        }
    }
}

/// The directory the editor keeps its own files in, made if it is not there.
///
/// Made on the way in rather than on the first write, so that somebody who
/// wants to look at the file or put one there finds the directory waiting.
pub fn directory() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    if home.is_empty() {
        return None;
    }
    let directory = PathBuf::from(home).join(".tcsgui");
    if let Err(why) = std::fs::create_dir_all(&directory) {
        eprintln!(
            "tcsgui: {} cannot be made, so settings will not be kept: {why}",
            directory.display()
        );
        return None;
    }
    Some(directory)
}

/// The settings file itself.
pub fn path() -> Option<PathBuf> {
    Some(directory()?.join("config.xml"))
}

/// Read the settings, or the defaults.
///
/// Every way of failing ends in the defaults: no `HOME`, no file yet, a file
/// that is not XML, a percentage that is not a number or is outside what is
/// allowed. A preference is not worth a refusal, and a file somebody has
/// edited by hand into something unreadable should leave the editor working.
///
/// A file that is not there yet is written, with the defaults in it. So the
/// first run leaves a settings file whose comments say what may be put in it,
/// which is a better answer to "where do I set this" than an empty directory
/// and a manual.
pub fn load() -> Settings {
    let path = match path() {
        Some(path) => path,
        None => return Settings::default(),
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        // Not there yet is the ordinary case, the first time the editor runs.
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => {
            let settings = Settings::default();
            save(settings);
            return settings;
        }
        Err(why) => {
            eprintln!("tcsgui: {} cannot be read: {why}", path.display());
            return Settings::default();
        }
    };

    let said = element(&text, "vehicle_pane_percent");
    let percent = match said.and_then(|said| said.parse::<i32>().ok()) {
        Some(percent) if (LEAST..=MOST).contains(&percent) => percent,
        Some(percent) => {
            eprintln!(
                "tcsgui: {} asks for a vehicle working pane of {percent}%, \
                 which is outside {LEAST}% to {MOST}%; using {DEFAULT}%",
                path.display()
            );
            DEFAULT
        }
        None => {
            if said.is_some() {
                eprintln!(
                    "tcsgui: {} does not say a number of percent; using \
                     {DEFAULT}%",
                    path.display()
                );
            }
            DEFAULT
        }
    };

    Settings {
        vehicle_percent: percent,
    }
}

/// Write the settings.
///
/// Atomically, with the same writer the configuration is written with: a
/// settings file half replaced by a crash would be a file the next session
/// complains about, and the complaint would be the editor's own doing.
pub fn save(settings: Settings) {
    let path = match path() {
        Some(path) => path,
        None => return,
    };
    let percent = settings.vehicle_percent.clamp(LEAST, MOST);
    if let Err(why) = crate::save::write_atomically(&path, written(percent).as_bytes()) {
        eprintln!("tcsgui: {} cannot be written: {why}", path.display());
    }
}

/// The settings file's text.
///
/// XML by the same conventions the configuration uses -- every value a child
/// element, never an attribute -- so that a person who has read one file has
/// read both. With a comment saying what the number means, because a bare 75
/// in a file is a number somebody will guess at.
fn written(percent: i32) -> String {
    format!(
        "\
<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!-- tcsgui's own settings. Written by the editor as they are changed, and
     read when it starts.

     This is not a configuration: nothing here flies. It is where the editor
     keeps what the person using it arranged. -->
<settings>
    <!-- How much of the working pane the vehicle working pane takes, as a
         percentage of the whole. The vehicle working pane is the left-hand
         one and holds the graph itself; the payload working pane is the
         right-hand one and holds what the vehicle carries. The rest of the
         working pane is the payload working pane's, so {rest}% here.

         Between {LEAST} and {MOST}: neither pane may be squeezed below a
         twentieth of the working pane, because a pane with nothing visible in
         it is a pane whose nodes cannot be reached. -->
    <vehicle_pane_percent>{percent}</vehicle_pane_percent>
</settings>
",
        rest = 100 - percent
    )
}

/// The text of the first `<name>` element, if there is one.
///
/// A scan rather than a parser. One element of one file, and bringing a schema
/// and a deserialiser to it would be more code than the thing it reads -- and
/// this file has to survive being edited by hand, which means being forgiving
/// about whatever else is in it.
fn element<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let opening = format!("<{name}>");
    let closing = format!("</{name}>");
    let from = text.find(&opening)? + opening.len();
    let to = text[from..].find(&closing)? + from;
    Some(text[from..to].trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What is written is what is read.
    ///
    /// The one thing a settings file has to do, and the one that breaks
    /// silently: a writer and a reader that disagree leave an editor that
    /// forgets what it was told every time it starts, which looks like the
    /// setting not being saved at all.
    #[test]
    fn a_percentage_written_is_the_percentage_read() {
        for percent in [LEAST, 25, DEFAULT, 80, MOST] {
            let text = written(percent);
            assert_eq!(
                element(&text, "vehicle_pane_percent")
                    .and_then(|said| said.parse::<i32>().ok()),
                Some(percent)
            );
        }
    }

    /// The file says what the other pane gets, too.
    #[test]
    fn the_file_says_what_both_panes_take() {
        let text = written(60);
        assert!(text.contains("60"), "{text}");
        assert!(text.contains("40%"), "the payload pane's share: {text}");
    }

    /// An element that is not there is not an error.
    #[test]
    fn a_missing_element_is_simply_absent() {
        assert_eq!(element("<settings></settings>", "vehicle_pane_percent"), None);
        assert_eq!(element("", "vehicle_pane_percent"), None);
        // An opening with no closing is not half a value.
        assert_eq!(element("<a>7", "a"), None);
        // And one that is there is found whatever surrounds it.
        assert_eq!(element("<a> 7 </a>", "a"), Some("7"));
        assert_eq!(element("<b>1</b><a>7</a>", "a"), Some("7"));
    }

    /// A value outside what is allowed is replaced rather than obeyed.
    ///
    /// Through the same arithmetic `load` uses. A hand-edited 0 would be a
    /// vehicle working pane with nothing in it, and an editor that did as it
    /// was told there would open on a canvas nobody could work out.
    #[test]
    fn a_percentage_outside_the_rule_is_refused() {
        for (said, expected) in [
            ("75", 75),
            ("5", 5),
            ("95", 95),
            ("0", DEFAULT),
            ("100", DEFAULT),
            ("-20", DEFAULT),
            ("three quarters", DEFAULT),
            ("", DEFAULT),
        ] {
            let percent = match said.parse::<i32>() {
                Ok(percent) if (LEAST..=MOST).contains(&percent) => percent,
                _ => DEFAULT,
            };
            assert_eq!(percent, expected, "{said:?}");
        }
    }

    /// The settings live under the home directory, in their own place.
    #[test]
    fn the_settings_are_in_a_directory_of_their_own() {
        let where_ = tempfile::tempdir().expect("a directory");
        // One process, one HOME: this is the only test that changes it, and
        // it puts it back.
        let was = std::env::var_os("HOME");
        std::env::set_var("HOME", where_.path());

        let path = path().expect("a settings path");
        assert_eq!(path, where_.path().join(".tcsgui/config.xml"));
        // Made on the way in, before anything is written.
        assert!(
            where_.path().join(".tcsgui").is_dir(),
            "the directory should be made automatically"
        );

        // And the first read leaves a file behind, with the defaults in it:
        // an empty directory is a worse answer to "where do I set this" than
        // a file whose comments say.
        assert!(!path.exists());
        assert_eq!(load().vehicle_percent, DEFAULT);
        assert!(path.exists(), "the settings file should be created");
        assert_eq!(load().vehicle_percent, DEFAULT);

        // And a round trip through the file itself.
        save(Settings {
            vehicle_percent: 40,
        });
        assert_eq!(load().vehicle_percent, 40);

        // A file nobody can make sense of leaves the default standing.
        std::fs::write(&path, "not xml at all").expect("a file");
        assert_eq!(load().vehicle_percent, DEFAULT);

        match was {
            Some(was) => std::env::set_var("HOME", was),
            None => std::env::remove_var("HOME"),
        }
    }
}
