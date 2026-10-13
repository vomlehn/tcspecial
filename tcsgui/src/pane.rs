//! The three working panes, and where a node may be in them.
//!
//! The canvas is divided into three columns, left to right: the moc working
//! pane, which holds the ground link; the vehicle working pane, which holds
//! the vehicle's own graph; and the payload working pane, which holds
//! everything the vehicle carries. `design.rst` says why, and the short of it
//! is that the three parts change on different schedules -- the ground is the
//! other end of the link and not the vehicle at all, the vehicle's own graph
//! persists, and what it carries changes between flights -- so a drawing that
//! puts each in its own column can be compared with last mission's.
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

/// Where the three working panes meet: two positions in the coordinates the
/// file holds.
///
/// `moc` is where the moc working pane ends and the vehicle's begins, and
/// `vehicle` is where the vehicle's ends and the payloads' begins. Together
/// they say where all three are, the first starting at nought and the last
/// ending at [`WORKING_WIDTH`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dividers {
    pub moc: i32,
    pub vehicle: i32,
}

/// Where the dividers fall, for panes of these percentages of the whole.
///
/// `moc_percent` and `vehicle_percent` are the first two panes' shares; the
/// payload working pane takes what is left. The dividers are movable, so this
/// is arithmetic rather than a pair of constants. What is kept is the
/// percentages -- see `settings.rs`, which says why those and not pixel
/// counts -- and this is the one place they become positions.
///
/// Snapped to the grid, which is not decoration: a node held against a divider
/// is a node whose position is that divider, and a divider off the grid would
/// put it off the grid too.
pub fn dividers_for(moc_percent: i32, vehicle_percent: i32) -> Dividers {
    let (moc_percent, vehicle_percent) = settings::shares(moc_percent, vehicle_percent);
    Dividers {
        moc: at_percent(moc_percent),
        vehicle: at_percent(moc_percent + vehicle_percent),
    }
}

/// Where the dividers sit until somebody moves them.
///
/// The defaults, for the tests that are about the panes rather than about
/// moving a line between them. A running editor reads them from the settings
/// instead -- see `settings::load` -- because somebody may have moved them.
#[cfg(test)]
pub fn default_dividers() -> Dividers {
    dividers_for(settings::DEFAULT_MOC, settings::DEFAULT_VEHICLE)
}

/// A position in the working pane, snapped to the grid.
fn at_percent(percent: i32) -> i32 {
    crate::snap((WORKING_WIDTH * percent / 100, 0)).0
}

/// And back: what percentages of the working pane these dividers leave the
/// first two panes.
///
/// Rounded to whole percents, which is what is written. The round trip is
/// stable -- a percent is a little over a grid step, so dividers snapped to
/// the grid come back to themselves -- and `tests::the_dividers_survive_being
/// _written_down` is what says so.
pub fn percents_for(dividers: Dividers) -> (i32, i32) {
    let whole = |x: i32| (x as f32 * 100.0 / WORKING_WIDTH as f32).round() as i32;
    settings::shares(whole(dividers.moc), whole(dividers.vehicle - dividers.moc))
}

/// Whether a node of this type is a payload, and so belongs on the right.
///
/// A type this build does not know is not a payload: it goes in the vehicle
/// working pane, which is the largest of the three and the one a reader looks
/// at first -- and a node whose type nothing recognises is a node somebody has
/// to look at.
pub fn is_payload(node_type: &str) -> bool {
    NodeType::ALL
        .iter()
        .find(|t| t.spelling() == node_type)
        .is_some_and(|t| t.is_payload())
}

/// Whether a node of this type is the ground link, and so belongs on the left.
///
/// The one type in its pane. It is there because it is not part of the vehicle
/// at all: it is the other end of the link, and a drawing that puts it in the
/// vehicle's own column says the vehicle contains the ground station.
pub fn is_moc(node_type: &str) -> bool {
    NodeType::ALL
        .iter()
        .find(|t| t.spelling() == node_type)
        .is_some_and(|t| *t == NodeType::Moc)
}

/// Which working pane a node of this type belongs in.
///
/// The one place the question is answered. Three panes and three families: the
/// ground link, the vehicle's own graph, and what it carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Moc,
    Vehicle,
    Payloads,
}

impl Pane {
    /// What the pane is called where a person can see it.
    ///
    /// The label at the top of the pane is in the markup, that being window
    /// text; this is the same word, for what the tests say when a pane rule is
    /// broken.
    #[cfg(test)]
    pub fn label(self) -> &'static str {
        match self {
            Pane::Moc => "moc",
            Pane::Vehicle => "vehicle",
            Pane::Payloads => "payloads",
        }
    }
}

/// Which pane this type belongs in.
pub fn pane_of(node_type: &str) -> Pane {
    if is_payload(node_type) {
        Pane::Payloads
    } else if is_moc(node_type) {
        Pane::Moc
    } else {
        Pane::Vehicle
    }
}

/// Where a node of this type may sit: the leftmost and rightmost x.
///
/// The right-hand bound allows for the node's own width, so a node confined to
/// a pane is drawn inside it rather than hanging over its edge.
pub fn pane_for(node_type: &str, dividers: Dividers) -> (i32, i32) {
    let (width, _) = size_of(node_type);
    let width = width as i32;
    match pane_of(node_type) {
        Pane::Moc => (0, dividers.moc - width),
        Pane::Vehicle => (dividers.moc, dividers.vehicle - width),
        Pane::Payloads => (dividers.vehicle, WORKING_WIDTH - width),
    }
}

/// Where a pane begins and ends, in the coordinates the file holds.
///
/// For the window, which draws the labels and the dividers from it, and for
/// placing a new node at the top of its own pane.
pub fn edges_of(pane: Pane, dividers: Dividers) -> (i32, i32) {
    match pane {
        Pane::Moc => (0, dividers.moc),
        Pane::Vehicle => (dividers.moc, dividers.vehicle),
        Pane::Payloads => (dividers.vehicle, WORKING_WIDTH),
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
pub fn confine(node_type: &str, dividers: Dividers, (x, y): (i32, i32)) -> (i32, i32) {
    let (left, right) = pane_for(node_type, dividers);
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
