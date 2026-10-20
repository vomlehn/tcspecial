//! The shape of the grid of panels
//!
//! The window has one panel per simulated payload, and how many there are is
//! not known until the configuration files have been read, so the arrangement
//! is worked out rather than written down.
//!
//! The window is kept wider than it is tall, and among the shapes that satisfy
//! that, the one nearest square is chosen. The bias is deliberate: screens are
//! wider than they are tall, so a column of panels runs off the bottom of one
//! long before a row runs off the side.
//!
//! tcsmoc shapes its own window by the same rule, with the sizes of its own
//! panels.

/// Measure a panel and tell the window what it came to.
///
/// Every window needs this before it lays anything out: the grid's cells are
/// the panel size, so a window that was never told comes up with cells of
/// nought and builds no panels at all. `Sizes::of` does it as it reads.
pub fn settle(ui: &crate::MainWindow) -> Sizes {
    Sizes::of(ui)
}

/// The sizes the window is built from, as the markup reports them.
///
/// Measured rather than declared. `ui/main.slint` lays out a panel of its own
/// and hands over what it came to, and states the chrome as the sum of the
/// pieces it holds -- the padding round the whole window, the heading, the row
/// with the quit button, and the grid's own padding inside the scrolling area.
/// This module does arithmetic on those and keeps no copy of them.
///
/// It kept copies until recently, and they went out of step in three ways at
/// once: a panel was called 210 tall where its rows take 176, 320 wide where
/// it wants 328, and when the height was corrected the window's own floor was
/// left at the old one. A number that exists once cannot disagree with
/// itself, which is the whole reason this is a parameter and not a constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sizes {
    pub panel_width: f32,
    pub panel_height: f32,
    pub chrome_width: f32,
    pub chrome_height: f32,
}

impl Sizes {
    /// What the window says about itself.
    ///
    /// Read before the window is sized, which is why the markup reports what a
    /// panel *wants* rather than what it got: a preferred size is a layout
    /// question and is answered without laying anything out.
    pub fn of(ui: &crate::MainWindow) -> Self {
        // A panel's size comes from a window of its own -- see PanelProbe in
        // ui/main.slint -- and the chrome from the real one, which knows its
        // own padding and the two rows above and below the grid. The probe is
        // built, read and dropped here; it is never shown.
        let probe = crate::PanelProbe::new().expect("a window to measure a panel in");
        let sizes = Self {
            panel_width: probe.get_panel_width(),
            panel_height: probe.get_panel_height(),
            chrome_width: ui.get_chrome_width(),
            chrome_height: ui.get_chrome_height(),
        };

        // What MainWindow lays the grid out by, so that the cells are the
        // size the measurement says and the window is not asked to agree with
        // a number of its own.
        ui.set_panel_width(sizes.panel_width);
        ui.set_panel_height(sizes.panel_height);
        sizes
    }

    /// The sizes the shipped window reports, for tests with no window to ask.
    ///
    /// The shape rule does not depend on what a panel actually comes to -- a
    /// grid that holds its panels holds them whatever size they are -- so a
    /// test of that rule needs numbers rather than the numbers.
    /// `the_markup_reports_the_sizes_the_tests_assume` is what keeps these
    /// from drifting away from the markup they were read off.
    #[cfg(test)]
    pub fn for_tests() -> Self {
        Self {
            panel_width: 394.0,
            panel_height: 192.0,
            chrome_width: 24.0,
            chrome_height: 92.0,
        }
    }
}

/// How many rows of panels the window looks ahead for.
///
/// Asked for rather than worked out: a window with room for only the rows a
/// particular tcspecial configuration happens to have is one that has to be
/// resized before a larger file can be watched, and three rows is the room that
/// was wanted. It is not a floor, though -- see [`rows_of_room`]. tcsmoc
/// follows the same rule, in its own panel sizes.
pub const MIN_PANEL_ROWS: usize = 3;

/// How many rows of panels the window opens with room for.
///
/// Never fewer than the grid has, so that nothing is cut off; never more than
/// [`MIN_PANEL_ROWS`], which is as far ahead as the window looks; and never
/// more rows than there are payloads to put in them, so a file of one or two
/// payloads opens only as tall as those need rather than keeping room it
/// could not fill.
pub fn rows_of_room(rows: usize, panels: usize) -> usize {
    rows.max(MIN_PANEL_ROWS.min(panels.max(1)))
}

/// How tall a window with room for `rows` rows of panels is. The grid sets no
/// gap between them, so there is none to allow for.
///
/// One row of them is the window's `min-height`, the least it is ever opened
/// at; [`rows_of_room`] says how many rows a particular tcspecial configuration
/// opens with.
pub fn height_for_rows(sizes: &Sizes, rows: usize) -> f32 {
    rows.max(1) as f32 * sizes.panel_height + sizes.chrome_height
}

/// The size the window opens at for a grid of `shape` holding `panels`.
pub fn window_size(sizes: &Sizes, shape: &GridShape, panels: usize) -> slint::LogicalSize {
    slint::LogicalSize::new(
        shape.width,
        height_for_rows(sizes, rows_of_room(shape.rows, panels)),
    )
}

/// How the grid of payload panels is shaped, and the window size that shape
/// asks for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridShape {
    pub columns: usize,
    pub rows: usize,
    pub width: f32,
    pub height: f32,
}

/// The most panels tcssim puts in a row, and so the fewest columns a grid of
/// that many payloads or more is given.
///
/// Three of them is a window most screens have the width for and a row an eye
/// takes in at once. tcsmoc says the same number for the same reason.
pub const MAX_COLUMNS: usize = 3;

/// How many columns `panels` payloads are laid out in.
///
/// A row is filled before another is started under it, up to [`MAX_COLUMNS`]
/// across, which is the rule tcsmoc already followed.
///
/// This replaced a rule that chose the shape nearest square among those at
/// least as wide as they were tall. It read well and laid the panels out
/// badly, and in the same way tcsmoc's did: five payloads came out two
/// columns by three rows, because a 680x620 window is squarer than a 1008x444
/// one. Squareness was never the thing wanted -- it was a proxy for a window
/// that fits a screen, and a column count says that directly.
///
/// The window the markup lays out works the same count back out of the width
/// it was given, so that resizing reflows the grid; see `columns` in
/// `ui/main.slint`. Opening the window at exactly this many panels' width is
/// what makes the two agree at startup.
pub fn columns_for(panels: usize) -> usize {
    panels.clamp(1, MAX_COLUMNS)
}

/// Choose the grid shape for `panels` simulated payloads.
pub fn grid_shape(sizes: &Sizes, panels: usize) -> GridShape {
    // An empty configuration still has to produce a window.
    let panels = panels.max(1);
    let columns = columns_for(panels);
    let rows = panels.div_ceil(columns);

    GridShape {
        columns,
        rows,
        width: columns as f32 * sizes.panel_width + sizes.chrome_width,
        height: height_for_rows(sizes, rows),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The room the window opens with, which is a rule rather than a floor.
    ///
    /// Three rows are looked ahead for, so a file that grows a row can be
    /// watched without resizing the window; but a file with fewer panels than
    /// that opens only as tall as the panels it has, rather than with room
    /// below that nothing could fill. A grid taller than three rows is given
    /// The sizes the shipped window reports, so that the shape rule is
    /// exercised on the numbers it actually runs on.
    ///
    /// A fixture and not a constant the program uses: the program asks the
    /// markup, and these are only here so the arithmetic has something to be
    /// done on. `the_markup_reports_the_sizes_these_tests_assume` is what
    /// stops them drifting apart.
    fn sizes() -> Sizes {
        Sizes::for_tests()
    }

    /// all of them.
    #[test]
    fn the_window_keeps_room_for_the_rows_there_are_to_fill() {
        // One and two payloads: as tall as they need and no taller.
        assert_eq!(rows_of_room(1, 1), 1);
        assert_eq!(rows_of_room(2, 2), 2);

        // Three or more: three rows of room, whatever the grid came to.
        assert_eq!(rows_of_room(1, 3), 3);
        assert_eq!(rows_of_room(2, 4), 3);
        assert_eq!(rows_of_room(3, 9), 3);

        // And never fewer rows than the grid actually has.
        assert_eq!(rows_of_room(4, 16), 4);
        assert_eq!(rows_of_room(8, 64), 8);

        // An empty tcspecial configuration still opens a window.
        assert_eq!(rows_of_room(1, 0), 1);
    }

    /// The shape chosen for the shipped four payloads.
    #[test]
    fn four_panels_make_a_row_of_three_and_a_row_of_one() {
        let shape = grid_shape(&sizes(), 4);
        assert_eq!((shape.columns, shape.rows), (3, 2));
        // Three panels across and two rows down, plus the chrome: 3 x 394 + 24
        // and 2 x 192 + 92. Written out rather than computed from the fixture,
        // so that a change to a panel's size has to be looked at here rather
        // than silently agreeing with itself.
        assert_eq!((shape.width, shape.height), (1206.0, 476.0));
    }

    /// A row is filled before another is started under it, three across.
    ///
    /// This is the rule that replaced choosing the shape nearest square. The
    /// old one put five payloads in two columns -- a 680x620 window being
    /// squarer than a 1008x444 one -- which is a narrow window with the panels
    /// running down it, and is what was asked about.
    #[test]
    fn a_row_holds_up_to_three_panels() {
        for panels in 1..=32 {
            let shape = grid_shape(&sizes(), panels);
            assert_eq!(
                shape.columns,
                panels.min(MAX_COLUMNS),
                "{panels} panels went into {} columns",
                shape.columns
            );
        }
    }

    /// Three payloads or more are three across, which is what was wanted.
    #[test]
    fn three_payloads_or_more_fill_the_row() {
        for panels in 3..=16 {
            assert_eq!(columns_for(panels), 3, "{panels} panels");
        }
        assert_eq!(columns_for(1), 1);
        assert_eq!(columns_for(2), 2);
    }

    #[test]
    fn every_shape_has_room_for_every_panel_and_no_empty_row() {
        for panels in 1..=64 {
            let shape = grid_shape(&sizes(), panels);
            assert!(
                shape.columns * shape.rows >= panels,
                "{panels} panels do not fit in {}x{}",
                shape.columns,
                shape.rows
            );
            // A shape with a row to spare is a taller window than the panels
            // need.
            assert!(
                shape.columns * (shape.rows - 1) < panels,
                "{panels} panels in {}x{} leave an empty row",
                shape.columns,
                shape.rows
            );
        }
    }

    /// The grid is wider than tall while it fits two rows.
    ///
    /// Not a rule the shape obeys for every count -- ten panels or more are
    /// four rows of a three-wide grid, which is taller than it is wide, and
    /// they scroll as too many panels always have. tcsmoc says the same.
    #[test]
    fn the_grid_is_wider_than_tall_while_it_fits_two_rows() {
        for panels in 1..=(MAX_COLUMNS * 2) {
            let shape = grid_shape(&sizes(), panels);
            assert!(
                shape.width >= shape.height,
                "{panels} panels gives {}x{}, taller than wide",
                shape.width,
                shape.height
            );
        }
    }

    /// Both GUIs follow the same rule, so a shape is chosen the same way
    /// whatever the panel size is: wide enough, and no wider than it must be.
    #[test]
    fn one_panel_is_one_cell() {
        let shape = grid_shape(&sizes(), 1);
        assert_eq!((shape.columns, shape.rows), (1, 1));
    }

    #[test]
    fn no_panels_still_gives_a_window() {
        let shape = grid_shape(&sizes(), 0);
        assert_eq!((shape.columns, shape.rows), (1, 1));
        assert!(shape.width >= shape.height);
    }
}
