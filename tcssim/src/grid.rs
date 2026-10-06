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

/// A panel's nominal size, and the height of everything above and below the
/// grid of them: the heading and the row holding the quit button.
///
/// These are used only to choose how many columns the grid has and how large
/// the window opens; the layout itself stretches panels to fit. They have to
/// agree with `ui/main.slint`, which is given the same panel size.
pub const PANEL_WIDTH: f32 = 320.0;
pub const PANEL_HEIGHT: f32 = 210.0;
pub const CHROME_HEIGHT: f32 = 76.0;

/// How the grid of payload panels is shaped, and the window size that shape
/// asks for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridShape {
    pub columns: usize,
    pub rows: usize,
    pub width: f32,
    pub height: f32,
}

impl GridShape {
    /// Width over height. 1.0 is square, above it is wider than tall.
    fn aspect(&self) -> f32 {
        self.width / self.height
    }
}

/// Choose the grid shape for `panels` simulated payloads.
///
/// The window should be wider than tall but no wider than it has to be, so of
/// the shapes that are at least square the squarest one wins. Taller-than-wide
/// shapes are not candidates at all, which is what makes the bias a rule rather
/// than a tendency.
///
/// A shape may have a cell to spare: three panels are better shown two above
/// and one below than in a row of three, and the empty cell costs nothing.
pub fn grid_shape(panels: usize) -> GridShape {
    // An empty configuration still has to produce a window.
    let panels = panels.max(1);

    let shape_for = |columns: usize| {
        let rows = panels.div_ceil(columns);
        GridShape {
            columns,
            rows,
            width: columns as f32 * PANEL_WIDTH,
            height: rows as f32 * PANEL_HEIGHT + CHROME_HEIGHT,
        }
    };

    // One column per panel is always at least square -- a single row is only
    // CHROME_HEIGHT + PANEL_HEIGHT tall -- so there is always a candidate, and
    // the fallback is unreachable unless those constants change.
    (1..=panels)
        .map(shape_for)
        .filter(|shape| shape.width >= shape.height)
        .min_by(|a, b| a.aspect().total_cmp(&b.aspect()))
        .unwrap_or_else(|| shape_for(panels))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape chosen for the shipped four payloads.
    #[test]
    fn four_panels_make_a_two_by_two() {
        let shape = grid_shape(4);
        assert_eq!((shape.columns, shape.rows), (2, 2));
        assert_eq!((shape.width, shape.height), (640.0, 496.0));
    }

    #[test]
    fn every_window_is_at_least_as_wide_as_it_is_tall() {
        for panels in 1..=64 {
            let shape = grid_shape(panels);
            assert!(
                shape.width >= shape.height,
                "{panels} panels gave {}x{}, which is taller than it is wide",
                shape.width,
                shape.height
            );
        }
    }

    #[test]
    fn every_shape_has_room_for_every_panel_and_no_empty_row() {
        for panels in 1..=64 {
            let shape = grid_shape(panels);
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

    /// Of the shapes that are wide enough, the chosen one is nearest square.
    #[test]
    fn no_other_shape_is_nearer_square() {
        for panels in 1..=64 {
            let chosen = grid_shape(panels);

            for columns in 1..=panels {
                let rows = panels.div_ceil(columns);
                let width = columns as f32 * PANEL_WIDTH;
                let height = rows as f32 * PANEL_HEIGHT + CHROME_HEIGHT;
                if width < height {
                    continue;
                }
                assert!(
                    width / height >= chosen.aspect(),
                    "{panels} panels: {columns}x{rows} is nearer square than the \
                     chosen {}x{}",
                    chosen.columns,
                    chosen.rows
                );
            }
        }
    }

    /// Both GUIs follow the same rule, so a shape is chosen the same way
    /// whatever the panel size is: wide enough, and no wider than it must be.
    #[test]
    fn one_panel_is_one_cell() {
        let shape = grid_shape(1);
        assert_eq!((shape.columns, shape.rows), (1, 1));
    }

    #[test]
    fn no_panels_still_gives_a_window() {
        let shape = grid_shape(0);
        assert_eq!((shape.columns, shape.rows), (1, 1));
        assert!(shape.width >= shape.height);
    }
}
