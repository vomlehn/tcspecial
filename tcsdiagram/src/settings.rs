//! What tcsgui remembers between sessions, in
//! `$HOME/.tcsgui/preferences.xml`.
//!
//! Four settings, in two pairs. How much of the working pane the moc working
//! pane takes and how much the vehicle's takes, across; and how much of the
//! selection column the moc list takes and how much the vehicle's takes,
//! down. Each set of panes is always in the same order -- payloads, vehicle,
//! moc -- so two percentages and that sentence are the whole of where a pair
//! of dividers is, the payload one taking what is left. The two are kept by
//! the pane they are about and not by position, so a file written when the
//! moc pane came first means the same widths now that it comes last.
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
//! Read here and written by the editor: this is shared with tcspecial's
//! visual window, which divides its panes where the person's editor does so
//! that a node is drawn where the editor would draw it, and which writes
//! nothing. Making the directory and writing the file are tcsgui's own.
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

/// And the most one pane may take, which is the same rule from the other
/// side: the other two keep a twentieth each.
pub const MOST: i32 = 100 - 2 * LEAST;

/// How the canvas is divided until somebody divides it differently.
///
/// A narrow column for the ground link, which is one node; most of the rest to
/// the vehicle, its own graph being most of a configuration; and a column for
/// what it carries this flight.
///
/// The moc pane's share is enough for a node to sit in rather than overhang
/// with a margin either side: a node is 152 pixels of a working pane of 960,
/// so a seventh would not do.
pub const DEFAULT_MOC: i32 = 20;
pub const DEFAULT_VEHICLE: i32 = 55;

/// How the selection column is divided until somebody divides it
/// differently.
///
/// Down rather than across, and the shares are not the working panes': what a
/// selection list has to show is its *entries*, and there are one, four and
/// ten of them. Each share is enough for the list's heading and the first node
/// under it at the window's own height, which is the rule
/// `tests::every_selection_list_shows_a_node_when_the_window_opens` holds
/// these to -- a list opened with nothing in it is a list somebody has to
/// discover is there.
///
/// The payload list takes what is left, which is the largest share for the
/// longest list.
pub const DEFAULT_LIST_MOC: i32 = 20;
pub const DEFAULT_LIST_VEHICLE: i32 = 35;

/// What the editor remembers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// How much of the working pane the moc working pane takes.
    pub moc_percent: i32,
    /// And how much the vehicle working pane takes. The payload working pane
    /// takes what is left.
    pub vehicle_percent: i32,
    /// How much of the selection column the moc list takes.
    pub list_moc_percent: i32,
    /// And how much the vehicle list takes. The payload list takes what is
    /// left.
    pub list_vehicle_percent: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            moc_percent: DEFAULT_MOC,
            vehicle_percent: DEFAULT_VEHICLE,
            list_moc_percent: DEFAULT_LIST_MOC,
            list_vehicle_percent: DEFAULT_LIST_VEHICLE,
        }
    }
}

/// Two shares that leave every pane something to show.
///
/// Each of the three keeps at least [`LEAST`] per cent, and the two stated
/// here are brought inside that however they were asked for -- by a file
/// somebody edited by hand, or by a divider dragged to the edge. The payload
/// pane's share is what is left, so it is the one that gives way when the
/// first two ask for too much between them.
///
/// Three shares and a floor, which is the same arithmetic whichever way the
/// panes are divided: the working panes across and the selection lists down
/// both come through here.
pub fn shares(moc_percent: i32, vehicle_percent: i32) -> (i32, i32) {
    let moc = moc_percent.clamp(LEAST, MOST);
    let vehicle = vehicle_percent.clamp(LEAST, MOST - moc + LEAST);
    // And the payload pane's own floor, which the two above can still have
    // eaten between them.
    let over = (moc + vehicle) - (100 - LEAST);
    if over > 0 {
        (moc, (vehicle - over).max(LEAST))
    } else {
        (moc, vehicle)
    }
}

/// The directory the editor keeps its own files in, found and not made.
///
/// Not made here, because this is read by tcspecial's visual window as well,
/// which has no business leaving a directory in somebody's home: the editor
/// makes it on the way in. See tcsgui's own settings.
pub fn home() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    if home.is_empty() {
        return None;
    }
    Some(PathBuf::from(home).join(".tcsgui"))
}

/// The settings file itself.
pub fn path() -> Option<PathBuf> {
    Some(home()?.join("preferences.xml"))
}

/// What the file was called before it was called `preferences.xml`.
///
/// Read when the current name is not there, so that the rename does not
/// silently forget what somebody had already arranged. The value is carried
/// into the new file and the old one is left alone for its owner to delete:
/// removing a file somebody might want to look at is not this program's
/// business, and this program did not ask for the rename.
///
/// It can go once no machine has one.
fn older() -> Option<PathBuf> {
    Some(home()?.join("config.xml"))
}

/// The settings as the file says them, and whether there was a file to say
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Read {
    pub settings: Settings,
    /// No file yet, so these are the defaults or what the older file said --
    /// which the editor then writes, so that the first run leaves a file whose
    /// comments say what may be put in it.
    pub missing: bool,
}

/// Read the settings, or the defaults, and write nothing.
///
/// Every way of failing ends in the defaults: no `HOME`, no file yet, a file
/// that is not XML, a percentage that is not a number or is outside what is
/// allowed. A preference is not worth a refusal, and a file somebody has
/// edited by hand into something unreadable should leave the editor working.
pub fn read() -> Read {
    let read = |settings| Read {
        settings,
        missing: false,
    };
    let path = match path() {
        Some(path) => path,
        None => return read(Settings::default()),
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => read(settings_in(&text, &path.display().to_string())),
        // Not there yet is the ordinary case, the first time the editor runs
        // -- or the first run since the file was renamed, which is why the
        // older name is tried before the defaults are.
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Read {
            settings: older()
                .and_then(|older| std::fs::read_to_string(older).ok())
                .map(|text| settings_in(&text, "the file this one replaced"))
                .unwrap_or_default(),
            missing: true,
        },
        Err(why) => {
            eprintln!("tcsgui: {} cannot be read: {why}", path.display());
            read(Settings::default())
        }
    }
}

/// The two shares, as this text states them.
///
/// `whose` names the file in anything said about it, because a complaint about
/// a percentage is only useful with the file it came from.
///
/// A file that says nothing about the moc working pane is a file written
/// before there was one. The default share is then taken out of the vehicle's
/// rather than out of the payloads': the column somebody arranged was the
/// payload column, and the vehicle pane is the one with room to give.
fn settings_in(text: &str, whose: &str) -> Settings {
    let vehicle =
        percent_in(text, whose, "vehicle_pane_percent", DEFAULT_VEHICLE);
    let (moc, vehicle) = match (
        element(text, "moc_pane_percent"),
        element(text, "vehicle_pane_percent"),
    ) {
        // Both stated, which is every file this build writes.
        (Some(_), _) => (
            percent_in(text, whose, "moc_pane_percent", DEFAULT_MOC),
            vehicle,
        ),
        // A file written before there was a moc working pane: it arranged the
        // other two, so the new pane's share comes out of the vehicle's.
        (None, Some(_)) => (DEFAULT_MOC, vehicle - DEFAULT_MOC),
        // And a file that says nothing about either -- or is not a settings
        // file at all -- which is the defaults and not arithmetic on them.
        (None, None) => (DEFAULT_MOC, DEFAULT_VEHICLE),
    };

    let (moc_percent, vehicle_percent) = shares(moc, vehicle);

    // And the selection column, which is the same arithmetic down. A file
    // written before the lists could be resized says nothing about them,
    // which is the defaults -- not a share taken out of anything, because
    // nobody arranged these yet.
    let (list_moc_percent, list_vehicle_percent) = shares(
        percent_in(text, whose, "moc_list_percent", DEFAULT_LIST_MOC),
        percent_in(text, whose, "vehicle_list_percent", DEFAULT_LIST_VEHICLE),
    );

    Settings {
        moc_percent,
        vehicle_percent,
        list_moc_percent,
        list_vehicle_percent,
    }
}

/// One share, as this text states it, or `default`.
fn percent_in(text: &str, whose: &str, name: &str, default: i32) -> i32 {
    let said = element(text, name);
    match said.and_then(|said| said.parse::<i32>().ok()) {
        Some(percent) if (LEAST..=MOST).contains(&percent) => percent,
        Some(percent) => {
            eprintln!(
                "tcsgui: {whose} asks for a {name} of {percent}%, which is \
                 outside {LEAST}% to {MOST}%; using {default}%"
            );
            default
        }
        None => {
            if said.is_some() {
                eprintln!(
                    "tcsgui: {whose} does not say a number of percent for \
                     {name}; using {default}%"
                );
            }
            default
        }
    }
}

/// The settings file's text.
///
/// XML by the same conventions the configuration uses -- every value a child
/// element, never an attribute -- so that a person who has read one file has
/// read both. With a comment saying what the number means, because a bare 75
/// in a file is a number somebody will guess at.
pub fn written(
    moc: i32,
    vehicle: i32,
    list_moc: i32,
    list_vehicle: i32,
) -> String {
    format!(
        "\
<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!-- tcsgui's own settings. Written by the editor as they are changed, and
     read when it starts.

     This is not a configuration: nothing here flies. It is where the editor
     keeps what the person using it arranged. -->
<settings>
    <!-- How much of the working pane the moc and vehicle working panes
         take, as a percentage of the whole. The three are always in this
         order, left to right: the payload working pane, which holds what the
         vehicle carries; the vehicle working pane, which holds the graph
         itself; and the moc working pane, which holds the ground link. The
         payload pane takes what is left, so {rest}% here.

         Each between {LEAST} and {MOST}, and every pane keeps at least
         {LEAST}%: a pane with nothing visible in it is a pane whose nodes
         cannot be reached. -->
    <moc_pane_percent>{moc}</moc_pane_percent>
    <vehicle_pane_percent>{vehicle}</vehicle_pane_percent>

    <!-- And how much of the selection column down the left the moc and
         vehicle lists take, under the same rule and in the same order top
         to bottom: the payload list, the vehicle list and the moc list, the
         payload list taking what is left, so {list_rest}% here. -->
    <moc_list_percent>{list_moc}</moc_list_percent>
    <vehicle_list_percent>{list_vehicle}</vehicle_list_percent>
</settings>
",
        rest = 100 - moc - vehicle,
        list_rest = 100 - list_moc - list_vehicle
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
        for (moc, vehicle) in [
            (LEAST, LEAST),
            (10, 25),
            (DEFAULT_MOC, DEFAULT_VEHICLE),
            (LEAST, MOST),
            (MOST, LEAST),
        ] {
            // Both pairs, and crossed over: the lists' shares are written
            // beside the panes' and read back from the same file, so a reader
            // that took one for the other would be a session that remembered
            // the wrong thing.
            let (list_moc, list_vehicle) = (vehicle, moc);
            let text = written(moc, vehicle, list_moc, list_vehicle);
            let said = |name| {
                element(&text, name).and_then(|said| said.parse::<i32>().ok())
            };
            assert_eq!(said("moc_pane_percent"), Some(moc));
            assert_eq!(said("vehicle_pane_percent"), Some(vehicle));
            assert_eq!(said("moc_list_percent"), Some(list_moc));
            assert_eq!(said("vehicle_list_percent"), Some(list_vehicle));
            // And through the reader, which is what the editor would use.
            let settings = settings_in(&text, "a test");
            assert_eq!(
                (settings.moc_percent, settings.vehicle_percent),
                (moc, vehicle)
            );
            assert_eq!(
                (settings.list_moc_percent, settings.list_vehicle_percent),
                (list_moc, list_vehicle)
            );
        }

        // A file written before the lists could be resized says nothing about
        // them, and that is the defaults rather than nothing: a session that
        // read a nought would open with a list of no height.
        let older = written(
            DEFAULT_MOC,
            DEFAULT_VEHICLE,
            DEFAULT_LIST_MOC,
            DEFAULT_LIST_VEHICLE,
        )
        .replace("<moc_list_percent>", "<was_moc_list_percent>")
        .replace("<vehicle_list_percent>", "<was_vehicle_list_percent>");
        let settings = settings_in(&older, "a file from before");
        assert_eq!(
            (settings.list_moc_percent, settings.list_vehicle_percent),
            (DEFAULT_LIST_MOC, DEFAULT_LIST_VEHICLE)
        );
    }

    /// The file says what the third pane gets, too.
    #[test]
    fn the_file_says_what_every_pane_takes() {
        let text = written(15, 60, 20, 30);
        assert!(text.contains("15"), "{text}");
        assert!(text.contains("60"), "{text}");
        assert!(text.contains("25%"), "the payload pane's share: {text}");
        assert!(text.contains("50%"), "the payload list's share: {text}");
    }

    /// Two shares that leave the third pane nothing are brought inside the
    /// rule.
    ///
    /// However they were asked for: by a file somebody edited by hand, or by
    /// a divider dragged to the edge. The payload pane is the one that gives
    /// way, being the one that is not stated.
    #[test]
    fn every_pane_keeps_a_twentieth() {
        assert_eq!(shares(15, 60), (15, 60));
        // Between them they asked for everything.
        assert_eq!(shares(50, 50), (50, 45));
        assert_eq!(shares(MOST, MOST), (MOST, LEAST));
        // And neither may be squeezed out itself.
        assert_eq!(shares(0, 200), (LEAST, MOST));
        assert_eq!(shares(-3, -3), (LEAST, LEAST));
    }

    /// A file written before there was a moc working pane keeps the payload
    /// column it arranged.
    ///
    /// The new pane's share comes out of the vehicle's, which is the one with
    /// room to give: taking it out of the payloads' would quietly narrow the
    /// column somebody had set.
    #[test]
    fn a_file_with_no_moc_pane_takes_its_share_from_the_vehicle() {
        let settings = settings_in(
            "<settings><vehicle_pane_percent>75\
             </vehicle_pane_percent></settings>",
            "a test",
        );
        assert_eq!(settings.moc_percent, DEFAULT_MOC);
        assert_eq!(settings.vehicle_percent, 75 - DEFAULT_MOC);
    }

    /// An element that is not there is not an error.
    #[test]
    fn a_missing_element_is_simply_absent() {
        assert_eq!(
            element("<settings></settings>", "vehicle_pane_percent"),
            None
        );
        assert_eq!(element("", "vehicle_pane_percent"), None);
        // An opening with no closing is not half a value.
        assert_eq!(element("<a>7", "a"), None);
        // And one that is there is found whatever surrounds it.
        assert_eq!(element("<a> 7 </a>", "a"), Some("7"));
        assert_eq!(element("<b>1</b><a>7</a>", "a"), Some("7"));
    }

    /// A value outside what is allowed is replaced rather than obeyed.
    ///
    /// A hand-edited 0 would be a vehicle working pane with nothing in it, and
    /// an editor that did as it was told there would open on a canvas nobody
    /// could work out. Through the reader itself, so what is checked is what
    /// the editor would use.
    #[test]
    fn a_percentage_outside_the_rule_is_refused() {
        for (said, expected) in [
            ("60", 60),
            ("5", LEAST),
            ("90", MOST),
            ("0", DEFAULT_VEHICLE),
            ("100", DEFAULT_VEHICLE),
            ("-20", DEFAULT_VEHICLE),
            ("three quarters", DEFAULT_VEHICLE),
            ("", DEFAULT_VEHICLE),
        ] {
            let text =
                format!("<vehicle_pane_percent>{said}</vehicle_pane_percent>");
            assert_eq!(
                percent_in(
                    &text,
                    "a test",
                    "vehicle_pane_percent",
                    DEFAULT_VEHICLE
                ),
                expected,
                "{said:?}"
            );
        }
        // And a file that says nothing about it at all.
        assert_eq!(
            percent_in(
                "<settings></settings>",
                "a test",
                "vehicle_pane_percent",
                DEFAULT_VEHICLE
            ),
            DEFAULT_VEHICLE
        );
    }

    /// Reading writes nothing: no directory and no file. The visual window
    /// reads this, and a vehicle's tooling has no business leaving either in
    /// somebody's home.
    #[test]
    fn reading_the_settings_leaves_nothing_behind() {
        let where_ = std::env::temp_dir()
            .join(format!("tcsdiagram-settings-{}", std::process::id()));
        std::fs::create_dir_all(&where_).expect("a directory");
        let was = std::env::var_os("HOME");
        std::env::set_var("HOME", &where_);

        let found = read();
        assert_eq!(found.settings, Settings::default());
        assert!(found.missing, "there is no file yet");
        assert!(!where_.join(".tcsgui").exists(), "nothing is made");

        match was {
            Some(was) => std::env::set_var("HOME", was),
            None => std::env::remove_var("HOME"),
        }
        std::fs::remove_dir_all(&where_).expect("tidied");
    }
}
