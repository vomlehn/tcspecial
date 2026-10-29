//! Where on the display a window is put.
//!
//! The two test windows are read together: tcsmoc shows what the ground sees
//! of each payload and tcssim what the payload is doing about it, so a link
//! being watched is watched in both at once. Left where a window manager puts
//! them they overlap, and an operator spends the session dragging one off the
//! other -- so tcsmoc asks for the top of the display and tcssim for the
//! bottom.
//!
//! Here rather than in either program because both ask the same question, and
//! an answer each would be two answers to drift apart. It sits in the library
//! the two already share for want of a better home: nothing else about a
//! display belongs to a library about configuration and telemetry, and a
//! third crate for twenty lines would be worse.
//!
//! A request is all this is. X11 honours a client's position; Wayland leaves
//! placement to the compositor and may ignore it, and a tiling window manager
//! will place windows as it sees fit whatever anyone asks. Nothing here can
//! tell which, and nothing depends on the answer: a window that lands
//! somewhere else is a window in the wrong place, not a program that failed.

use std::fs;

/// Which edge of the display a window asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    /// The top, where tcsmoc goes: it is the window commands are sent from.
    Top,
    /// The bottom, where tcssim goes.
    Bottom,
}

/// Works out where to put a window on a display.
///
/// Physical pixels, because that is what a window reports its size in and
/// what a position is read in: going through logical pixels would mean
/// knowing the scale factor, which is the display's business and not this
/// program's.
///
/// A window taller than the display sits at the top whichever edge it asked
/// for: there is no position that shows the bottom of it, and the top is
/// where its name and its controls are.
///
/// # Arguments
///
/// * `edge`          - Which edge of the display the window asks for.
/// * `screen_height` - The display's height in physical pixels, `None` when
///                     it is unknown.
/// * `window_height` - The window's height in physical pixels.
///
/// # Returns
///
/// The window's top edge, in physical pixels from the top of the display.
/// `None` when the display's height is unknown -- then nothing is asked for,
/// rather than a position guessed at from a height nobody measured.
pub fn top_of(
    edge: Edge,
    screen_height: Option<u32>,
    window_height: u32,
) -> Option<i32> {
    let screen_height = screen_height?;

    let y = match edge {
        Edge::Top => 0,
        Edge::Bottom => screen_height.saturating_sub(window_height),
    };

    Some(i32::try_from(y).unwrap_or(0))
}

/// How tall the display is, in physical pixels, if it can be found out.
///
/// Read from the kernel rather than from the window system: a program that
/// has a window has a connection to a display server, but neither X11 nor
/// Wayland is reachable through the toolkit's own interface here, and asking
/// the display server by running a command would be a process started to read
/// one number.
///
/// `/sys/class/drm/*/status` says which outputs have something plugged into
/// them and `modes` lists what each can do, most preferred first. The first
/// line of a connected output's modes is the one it is most likely to be
/// running.
///
/// The tallest connected output, where there are several. It is a guess at
/// which one the windows are on -- nothing here can know that -- and the
/// tallest is the one that makes a window asking for the bottom land on the
/// display rather than above it.
///
/// # Returns
///
/// The height of the tallest connected output, in physical pixels, or
/// `None` if no connected output says how tall it is.
pub fn screen_height() -> Option<u32> {
    let mut tallest = None;

    for output in fs::read_dir("/sys/class/drm").ok()? {
        let output = match output {
            Ok(output) => output.path(),
            Err(_) => continue,
        };

        // A card itself has no status; only its outputs do.
        match fs::read_to_string(output.join("status")) {
            Ok(status) if status.trim() == "connected" => {}
            _ => continue,
        }

        let modes = match fs::read_to_string(output.join("modes")) {
            Ok(modes) => modes,
            Err(_) => continue,
        };

        if let Some(height) = modes.lines().next().and_then(height_of) {
            tallest =
                Some(tallest.map_or(height, |so_far: u32| so_far.max(height)));
        }
    }

    tallest
}

/// The height in a mode line, which reads `3840x2400`.
fn height_of(mode: &str) -> Option<u32> {
    mode.trim().split_once('x')?.1.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mode_line_says_how_tall_the_display_is() {
        assert_eq!(height_of("3840x2400"), Some(2400));
        assert_eq!(height_of("1920x1080"), Some(1080));
        assert_eq!(height_of(" 1280x720 "), Some(720));

        // What a disconnected output's modes file holds, and what a line that
        // is not a mode looks like.
        assert_eq!(height_of(""), None);
        assert_eq!(height_of("1920"), None);
        assert_eq!(height_of("widexhigh"), None);
    }

    /// The top is the top, and the bottom is as far down as the window fits.
    #[test]
    fn a_window_is_put_against_the_edge_it_asked_for() {
        assert_eq!(top_of(Edge::Top, Some(2400), 900), Some(0));
        assert_eq!(top_of(Edge::Bottom, Some(2400), 900), Some(1500));

        // Exactly filling it: one is at the top either way.
        assert_eq!(top_of(Edge::Bottom, Some(900), 900), Some(0));
    }

    /// A window taller than the display sits at the top, not above it.
    ///
    /// Asking for the bottom of a display a window overflows would put its
    /// title bar and its controls off the top of the screen, where nothing
    /// can reach them.
    #[test]
    fn a_window_taller_than_the_display_is_not_pushed_off_it() {
        assert_eq!(top_of(Edge::Bottom, Some(900), 1200), Some(0));
    }

    /// With no height to work from, nothing is asked for.
    #[test]
    fn an_unknown_display_is_left_to_the_window_manager() {
        assert_eq!(top_of(Edge::Top, None, 900), None);
        assert_eq!(top_of(Edge::Bottom, None, 900), None);
    }

    /// Whatever this machine reports is a plausible height, or nothing.
    ///
    /// The one thing a test can say about a real display: it does not assert
    /// a size, because the machine running the tests is not the machine the
    /// windows open on and may have no display at all.
    #[test]
    fn the_display_this_runs_on_is_read_or_not_found() {
        if let Some(height) = screen_height() {
            assert!(
                (240..=16_384).contains(&height),
                "{height} is not a display height"
            );
        }
    }
}
