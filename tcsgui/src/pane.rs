//! The two working panes, and where a node may be in them.
//!
//! The canvas is divided: the right-hand part is the payload working pane and
//! holds everything the vehicle carries, and the rest is the vehicle working
//! pane and holds the rest of the graph. `design.rst` says why, and the short
//! of it is that the vehicle's own graph is the part that persists while what
//! it carries is the part that changes between flights -- so a drawing that
//! puts the changing part in one column can be compared with last mission's.
//!
//! Its own module because three parts of the program need the same answers and
//! none of them should be asking another: the window places and confines nodes
//! as they are dragged, the document brings a configuration's nodes inside
//! their panes as it is read, and the preferences turn a percentage into a
//! position. All of it is arithmetic over four numbers, which is why the tests
//! for it need no editor.

use crate::settings;
use crate::style::NodeType;

/// The grid nodes sit on, in pixels. See [`snap`].
pub const GRID: i32 = 8;

/// How wide the whole working area is, in the coordinates the file holds.
///
/// A fixed width, and that is the point: the two working panes are divided at
/// a place in the document rather than at a fraction of the window. Which pane
/// a node is in says what it is allowed to be, so a divider that moved with
/// the window would make a payload into a vehicle node by being looked at on a
/// smaller screen -- and two people editing one configuration would disagree
/// about where things may go.
pub const WORKING_WIDTH: i32 = 960;

/// Where the vehicle working pane ends and the payload working pane begins,
/// for a vehicle working pane of this percentage of the whole.
///
/// The divider is movable, so this is arithmetic rather than a constant. What
/// is kept is the percentage -- see `settings.rs`, which says why that and not
/// a pixel count -- and this is the one place it becomes a position.
///
/// Snapped to the grid, which is not decoration: a node held against the
/// divider is a node whose position is the divider, and a divider off the grid
/// would put it off the grid too.
pub fn divider_for(percent: i32) -> i32 {
    crate::snap((
        WORKING_WIDTH * percent.clamp(settings::LEAST, settings::MOST) / 100,
        0,
    ))
    .0
}

/// And back: what percentage of the working pane a divider here leaves the
/// vehicle.
///
/// Rounded to a whole percent, which is what is written. The round trip is
/// stable -- a percent is a little over a grid step, so a divider snapped to
/// the grid comes back to itself -- and [`tests::the_divider_survives_being
/// _written_down`] is what says so.
pub fn percent_for(divider: i32) -> i32 {
    ((divider as f32 * 100.0 / WORKING_WIDTH as f32).round() as i32)
        .clamp(settings::LEAST, settings::MOST)
}

/// Whether a node of this type is a payload, and so belongs on the right.
///
/// A type this build does not know is not a payload: it goes in the vehicle
/// working pane, which is the larger of the two and the one a reader looks at
/// first -- and a node whose type nothing recognises is a node somebody has to
/// look at.
pub fn is_payload(node_type: &str) -> bool {
    NodeType::ALL
        .iter()
        .find(|t| t.spelling() == node_type)
        .is_some_and(|t| t.is_payload())
}

/// Where a node of this type may sit: the leftmost and rightmost x.
///
/// The right-hand bound allows for the node's own width, so a node confined to
/// a pane is drawn inside it rather than hanging over its edge.
pub fn pane_for(node_type: &str, divider: i32) -> (i32, i32) {
    let (width, _) = size_of(node_type);
    let width = width as i32;
    if is_payload(node_type) {
        (divider, WORKING_WIDTH - width)
    } else {
        (0, divider - width)
    }
}

/// `at`, brought inside the pane its type belongs to.
///
/// What keeps a payload in the payload working pane and everything else out of
/// it. Applied when a node is placed and when a drag ends, which is every way
/// a position is decided in the editor -- and not when a file is read: a
/// configuration says where its nodes are, and an editor that silently moved
/// them on opening would be an editor that changed a file by being pointed at
/// it. A node read in from the wrong side stays where the file put it until
/// somebody drags it, and then it comes home.
pub fn confine(node_type: &str, divider: i32, (x, y): (i32, i32)) -> (i32, i32) {
    let (left, right) = pane_for(node_type, divider);
    // The lower bound first: a pane narrower than a node -- which the five per
    // cent rule allows, the payload pane at its narrowest being 48 pixels --
    // has a right bound to the left of its left one, and a node in it sits at
    // its left edge and overhangs.
    (x.clamp(left, right.max(left)), y.max(0))
}

/// The nominal size of a type, for placing a link's ends.
pub fn size_of(node_type: &str) -> (u32, u32) {
    NodeType::ALL
        .iter()
        .find(|t| t.spelling() == node_type)
        .map(|t| t.appearance().size)
        .unwrap_or((104, 40))
}
