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

/// How long the working panes are, in the coordinates the file holds.
///
/// One length for all three, which is what makes them scroll together: they
/// are three columns of one sheet rather than three panes of their own, so a
/// node at y 900 in the vehicle pane and one at y 900 in the payload pane are
/// level, and one scrollbar moves all three. Three panes that scrolled
/// separately would put a link's two ends at two heights that have nothing to
/// do with each other, and a diagram whose lines go off the top of one column
/// and in at the middle of another cannot be read.
///
/// Longer than a window on purpose. A vehicle with forty payloads needs room
/// for forty, and a sheet the size of the window would mean the fortieth could
/// not be put anywhere -- so the sheet is scrolled and a new node is placed as
/// high as it fits, which is [`place_in`].
pub const WORKING_HEIGHT: i32 = 2400;

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
    /// The label in the pane's top left corner is in the markup, that being
    /// window text; these are the same words, for what the tests say when a
    /// pane rule is broken.
    #[cfg(test)]
    pub fn label(self) -> &'static str {
        match self {
            Pane::Moc => "MOC",
            Pane::Vehicle => "Vehicle",
            Pane::Payloads => "Payloads",
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

/// The band at the top of a pane that the pane's own name is written in.
///
/// `(top, height)`, and the same for all three: each band sits at the very top
/// of its own pane and all three are of one depth. How wide one is comes from
/// the dividers rather than from here -- a band spans the pane it belongs to,
/// and the panes' edges move -- which is what [`names_bands`] is for.
///
/// Three bands and not a strip across the sheet. A name belongs to a pane: the
/// band is painted in that pane's own tint, so the colour that says which pane
/// a node belongs in is at the head of the pane it belongs to. See `Surface`
/// in `main.slint`, which draws them, and whose depth is asked of the labels
/// themselves.
///
/// Nothing the editor places itself goes in one. A node may still be *dragged*
/// over a name -- it is somebody's drawing and the editor does not own the
/// sheet -- but a node the editor put there would cover a name nobody asked
/// it to cover, and the name is what a pane has to say when it is empty or
/// when somebody has lost their bearings.
///
/// From the top of the pane rather than four pixels down, because the band is
/// drawn and not just reserved: a tint that began below the top edge would be
/// a stripe across the pane rather than its head. Twenty-four pixels deep,
/// which is the line box of the small type and a pixel with `Space.tight`
/// above it and below -- the markup asks the labels how deep they are and this
/// is the generous answer to the same question, so the row the placing keeps
/// off is never shallower than the row that is drawn.
pub const NAMES: (i32, i32) = (0, 24);

/// Those bands as rectangles, one per pane, for the placing to keep off.
///
/// They abut at the dividers, so what the placing keeps off is the same row
/// either way; three is what says the row is three panes' heads rather than
/// one strip laid over them, and it is what the window draws.
fn names_bands(dividers: Dividers) -> [(i32, i32, u32, u32); 3] {
    let (top, height) = NAMES;
    [Pane::Moc, Pane::Vehicle, Pane::Payloads].map(|pane| {
        let (left, right) = edges_of(pane, dividers);
        (left, top, (right - left).max(0) as u32, height as u32)
    })
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

/// Where a new node of this type goes.
///
/// As high as it will fit in its own pane without covering anything, and in
/// view: a node placed where it cannot be seen is a press that did nothing as
/// far as the person who made it can tell.
///
/// `taken` is every box already on the sheet and `visible` is the part of the
/// sheet the window is showing: the top of the view and how much of it there
/// is. Three places are tried, in the order somebody would look:
///
/// the highest place in view that covers nothing. This is the ordinary case
/// and the only one that happens on a sheet with room in it;
///
/// then the *lowest* place above the view that covers nothing -- lowest
/// because it is the nearest, so the scroll that follows is the shortest;
///
/// then the highest place below the view. Which is the last resort because
/// scrolling down past what is already there is the longer journey.
///
/// Rows, left to right and top to bottom: at each height every place across
/// the pane is tried before moving down. A column would fill the left of a
/// wide pane and scroll away from three node widths of empty space.
///
/// Where nothing fits anywhere -- a full sheet -- the top of the pane, which
/// is a node drawn over another and visibly so. The alternative is a press
/// that does nothing.
pub fn place_in(
    taken: &[(i32, i32, u32, u32)],
    node_type: &str,
    dividers: Dividers,
    visible: (i32, i32),
) -> (i32, i32) {
    let (width, height) = size_of(node_type);
    let (left, right) = pane_for(node_type, dividers);
    let right = right.max(left);
    let bottom = WORKING_HEIGHT - height as i32;

    // What is already drawn, and the band at the top of each pane that its
    // name is written in: nothing the editor places itself covers a name. See
    // `NAMES`.
    let bands = names_bands(dividers);
    let clear = |x: i32, y: i32| {
        !taken
            .iter()
            .chain(bands.iter())
            .any(|(theirs, tall, their_width, their_height)| {
                x < theirs + *their_width as i32
                    && *theirs < x + width as i32
                    && y < tall + *their_height as i32
                    && *tall < y + height as i32
            })
    };
    let across = |y: i32| {
        let mut x = left;
        while x <= right {
            if clear(x, y) {
                return Some((x, y));
            }
            x += GRID;
        }
        None
    };

    // In view, highest first. The view's own bottom, not the sheet's: a node
    // hanging half off the bottom of the window is not in view.
    let (top, how_much) = visible;
    let top = top.max(0);
    let mut y = top;
    while y <= (top + how_much - height as i32).min(bottom) {
        if let Some(at) = across(y) {
            return at;
        }
        y += GRID;
    }

    // Above the view, lowest first: the nearest place is the shortest scroll.
    let mut y = (top - height as i32).min(bottom);
    while y >= 0 {
        if let Some(at) = across(y) {
            return at;
        }
        y -= GRID;
    }

    // Below it, highest first.
    let mut y = (top + how_much).max(0);
    while y <= bottom {
        if let Some(at) = across(y) {
            return at;
        }
        y += GRID;
    }

    (left, 0)
}

/// The nominal size of a type, for placing a link's ends.
pub fn size_of(node_type: &str) -> (u32, u32) {
    NodeType::ALL
        .iter()
        .find(|t| t.spelling() == node_type)
        .map(|t| t.appearance().size)
        .unwrap_or((104, 40))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A band apiece, each across its own pane, all of one depth.
    ///
    /// The arrangement [`NAMES`] is half of: the depth and the top are here,
    /// the width comes from the dividers, and the window paints each band in
    /// its own pane's tint. Three bands that did not abut would leave a gap in
    /// the row where the editor could place a node with a name beside it, and
    /// three of different depths would read as three panes drawn separately.
    #[test]
    fn each_pane_has_a_band_of_its_own_across_it() {
        let (top, height) = NAMES;

        for dividers in [
            default_dividers(),
            dividers_for(10, 20),
            dividers_for(60, 30),
        ] {
            let bands = names_bands(dividers);

            // Each band is its pane's, from its left edge to the next
            // divider, at the top of the sheet and of the one depth.
            for (band, pane) in bands.iter().zip([Pane::Moc, Pane::Vehicle, Pane::Payloads]) {
                let (left, right) = edges_of(pane, dividers);
                assert_eq!(
                    *band,
                    (left, top, (right - left) as u32, height as u32),
                    "the {} band",
                    pane.label()
                );
            }

            // And the three of them are one row across the sheet: no gap
            // between them, and nothing left over at either end.
            assert_eq!(bands[0].0, 0);
            assert_eq!(bands[0].0 + bands[0].2 as i32, bands[1].0);
            assert_eq!(bands[1].0 + bands[1].2 as i32, bands[2].0);
            assert_eq!(bands[2].0 + bands[2].2 as i32, WORKING_WIDTH);
        }
    }

    /// A node the editor places keeps off its own pane's band.
    ///
    /// Asked of every type in every pane, and of a canvas with nothing else on
    /// it: the first node placed is the one that would otherwise go in the top
    /// left corner, which is where the name is.
    #[test]
    fn a_placed_node_goes_below_the_band() {
        let (top, height) = NAMES;
        let dividers = default_dividers();

        for kind in NodeType::ALL {
            let at = place_in(&[], kind.spelling(), dividers, (0, 700));
            assert!(
                at.1 >= top + height,
                "{} was placed at {at:?}, over the band at the top of its pane",
                kind.spelling()
            );
            // In its own pane, which is the other half of where a node may be.
            let (left, right) = edges_of(pane_of(kind.spelling()), dividers);
            assert!(
                at.0 >= left && at.0 < right,
                "{} is in its own pane",
                kind.spelling()
            );
        }
    }
}
