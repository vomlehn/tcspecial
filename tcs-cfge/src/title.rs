//! The main window's title: what the window is, and which file it is editing.
//!
//! The file is said in the title bar because that is where somebody looks to
//! tell one window from another, and two editors open on two configurations
//! look the same everywhere else. A path is often longer than a title bar,
//! and a window system that cuts a title short cuts it at the end -- which is
//! the file's name, the one part that cannot be spared. So the path is
//! shortened here first, the way file managers and editors shorten one: the
//! home directory said as `~`, then the first element and as many of the
//! last as there is room for, with an ellipsis for the elements left out
//! between them.

/// How many logical pixels wide a character of the title is, taken across a
/// typical title: a title bar's font is the desktop's, which this program
/// cannot ask, so this is an estimate that errs towards too wide. An estimate
/// too wide leaves room unused; one too narrow has the window system cut the
/// file's name off the end.
const CHARACTER: f32 = 8.0;

/// How much of a title bar's width is not the title's: the window system's
/// buttons, and the margins it leaves either side of the title.
const DECORATION: f32 = 160.0;

/// What stands for the elements left out of a shortened path.
const LEFT_OUT: &str = "\u{2026}";

/// The title for a window this wide, editing this file.
///
/// # Arguments
///
/// * `base`  - What the window is, which is said in full.
/// * `path`  - The file being edited; nothing more is said when it is empty.
/// * `width` - How wide the window is, in logical pixels.
///
/// # Returns
///
/// `base`, then a colon and a space and the path, shortened to fit.
pub fn titled(base: &str, path: &str, width: f32) -> String {
    if path.is_empty() {
        return base.to_string();
    }
    let whole = std::path::absolute(path)
        .map(|absolute| absolute.display().to_string())
        .unwrap_or_else(|_| path.to_string());
    let whole = homed(&whole, std::env::var("HOME").ok().as_deref());
    let room = ((width - DECORATION) / CHARACTER).max(0.0) as usize;
    let room = room.saturating_sub(base.chars().count() + 2);
    format!("{base}: {}", shortened(&whole, room))
}

/// A path with the home directory it is in said as `~`, as shells and file
/// managers say it.
///
/// # Arguments
///
/// * `path` - The path, absolute.
/// * `home` - The home directory, if there is one.
///
/// # Returns
///
/// The path, with `~` for the home directory where it starts with it.
pub fn homed(path: &str, home: Option<&str>) -> String {
    let separator = std::path::MAIN_SEPARATOR_STR;
    let Some(home) = home.map(|h| h.trim_end_matches(separator)) else {
        return path.to_string();
    };
    if home.is_empty() {
        return path.to_string();
    }
    match path.strip_prefix(home) {
        Some("") => "~".to_string(),
        Some(rest) if rest.starts_with(separator) => format!("~{rest}"),
        _ => path.to_string(),
    }
}

/// A path shortened to fit in `room` characters, if it can be.
///
/// The first element and the last are always kept, so the shortest a path is
/// made is `/home/…/thermostat.xtcs`, even where that is longer than `room`:
/// a title that named neither where the file is nor what it is called would
/// say nothing. Between them, as many of the last elements as fit, because
/// the ones nearest the file are the ones that tell one configuration from
/// another.
///
/// # Arguments
///
/// * `path` - The path, as it is to be shown.
/// * `room` - How many characters it may take.
///
/// # Returns
///
/// The path, whole if it fits and shortened if not.
pub fn shortened(path: &str, room: usize) -> String {
    let length = |s: &str| s.chars().count();
    if length(path) <= room {
        return path.to_string();
    }
    let separator = std::path::MAIN_SEPARATOR_STR;
    // Kept apart from the elements so that an absolute path keeps the
    // separator it starts with.
    let rooted = path.starts_with(separator);
    let elements: Vec<&str> = path
        .split(separator)
        .filter(|element| !element.is_empty())
        .collect();
    if elements.len() < 3 {
        return path.to_string();
    }
    let first = format!(
        "{}{}{separator}{LEFT_OUT}",
        if rooted { separator } else { "" },
        elements[0]
    );
    let mut kept = elements.len() - 1;
    let mut tail = length(elements[kept]) + 1;
    // Leftwards from the file's own name, each element only if it and its
    // separator still fit; never the first, which is kept already, and an
    // ellipsis that stood for nothing would be a lie about the path.
    while kept > 2 {
        let more = length(elements[kept - 1]) + 1;
        if length(&first) + tail + more > room {
            break;
        }
        tail += more;
        kept -= 1;
    }
    let mut out = first;
    for element in &elements[kept..] {
        out.push_str(separator);
        out.push_str(element);
    }
    out
}

/// An absolute path of these elements, for the tests, which build paths with
/// the separator of the system they run on.
#[cfg(test)]
fn joined(elements: &[&str]) -> String {
    let separator = std::path::MAIN_SEPARATOR_STR;
    format!("{separator}{}", elements.join(separator))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The home directory is said as `~`, and only where the path is in it.
    #[test]
    fn the_home_directory_is_a_tilde() {
        let home = joined(&["home", "someone"]);
        let file = joined(&["home", "someone", "working", "v.xtcs"]);
        let separator = std::path::MAIN_SEPARATOR_STR;
        assert_eq!(
            homed(&file, Some(&home)),
            format!("~{separator}working{separator}v.xtcs")
        );
        assert_eq!(homed(&home, Some(&home)), "~");
        let beside = joined(&["home", "someone-else", "v.xtcs"]);
        assert_eq!(homed(&beside, Some(&home)), beside);
        assert_eq!(homed(&file, None), file);
    }

    const ELEMENTS: [&str; 5] = [
        "home",
        "someone",
        "working",
        "thermostat",
        "thermostat.xtcs",
    ];

    #[test]
    fn a_path_that_fits_is_left_whole() {
        let path = joined(&ELEMENTS);
        assert_eq!(shortened(&path, path.len()), path);
    }

    #[test]
    fn a_long_path_keeps_its_first_element_and_as_many_last_as_fit() {
        let path = joined(&ELEMENTS);
        // Room for "/home/…/thermostat/thermostat.xtcs", and not for
        // "working" as well.
        let room = "/home/…/thermostat/thermostat.xtcs".chars().count();
        assert_eq!(
            shortened(&path, room),
            joined(&["home", "…", "thermostat", "thermostat.xtcs"])
        );
    }

    #[test]
    fn a_path_too_long_for_any_room_still_says_its_first_and_last() {
        let path = joined(&ELEMENTS);
        assert_eq!(
            shortened(&path, 3),
            joined(&["home", "…", "thermostat.xtcs"])
        );
    }

    #[test]
    fn a_relative_path_is_shortened_without_a_leading_separator() {
        let separator = std::path::MAIN_SEPARATOR_STR;
        let path = ELEMENTS.join(separator);
        assert_eq!(
            shortened(&path, 3),
            ["home", "…", "thermostat.xtcs"].join(separator)
        );
    }

    #[test]
    fn a_path_of_two_elements_has_nothing_to_leave_out() {
        let path = joined(&["home", "thermostat.xtcs"]);
        assert_eq!(shortened(&path, 3), path);
    }

    #[test]
    fn the_title_is_what_the_window_is_then_the_file() {
        let path = joined(&ELEMENTS);
        assert_eq!(
            titled("TCSpecial Configuration", &path, 2000.0),
            format!("TCSpecial Configuration: {path}")
        );
        // Narrow: the name of the window in full, and the file's name.
        let narrow = titled("TCSpecial Configuration", &path, 300.0);
        assert!(narrow.starts_with("TCSpecial Configuration: "), "{narrow}");
        assert!(narrow.ends_with("thermostat.xtcs"), "{narrow}");
        assert!(narrow.contains('…'), "{narrow}");
    }

    #[test]
    fn a_window_editing_nothing_has_its_name_alone() {
        assert_eq!(
            titled("TCSpecial Configuration", "", 2000.0),
            "TCSpecial Configuration"
        );
    }
}
