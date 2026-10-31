//! The three working panes, and where a node may be in them.
//!
//! The canvas is divided into three columns, left to right: the payload working
//! pane, which holds everything the vehicle carries; the vehicle working pane,
//! which holds the vehicle's own graph; and the gateway working pane, which
//! holds the ground link. `design.rst` says why, and the short of it
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

/// How many of the canvas's pixels make a millimetre.
///
/// The grid a node's upper left corner sits on is a millimetre, which is a
/// length a person can name and measure where a pixel count is how this build
/// happens to draw. A pixel here is the window's logical pixel, ninety-six to
/// the inch -- which is what the window toolkit means by a millimetre too, so
/// the grid on the screen is a millimetre on the screen.
///
/// Not a whole number of pixels, so the grid's points are each millimetre
/// rounded to the nearest pixel: three or four pixels apart, never drifting,
/// because each is worked out from its own count of millimetres rather than
/// by adding a step to the one before. Positions stay whole pixels, which is
/// what the file holds.
pub const PX_PER_MM: f64 = 96.0 / 25.4;

/// The pixel the grid point `mm` millimetres from the edge is at.
pub fn grid_point(mm: i32) -> i32 {
    (mm as f64 * PX_PER_MM).round() as i32
}

/// The grid point nearest `v`.
pub fn on_grid(v: i32) -> i32 {
    grid_point((v as f64 / PX_PER_MM).round() as i32)
}

/// The lowest grid point at or above `v`.
///
/// # Arguments
///
/// * `v` - A position, in pixels.
///
/// # Returns
///
/// The grid point, in pixels.
pub fn grid_ceil(v: i32) -> i32 {
    let mut mm = (v as f64 / PX_PER_MM).floor() as i32;
    while grid_point(mm) < v {
        mm += 1;
    }
    grid_point(mm)
}

/// The highest grid point at or below `v`.
///
/// # Arguments
///
/// * `v` - A position, in pixels.
///
/// # Returns
///
/// The grid point, in pixels.
pub fn grid_floor(v: i32) -> i32 {
    let mut mm = (v as f64 / PX_PER_MM).ceil() as i32;
    while grid_point(mm) > v {
        mm -= 1;
    }
    grid_point(mm)
}

/// The grid point after `v`, which is a step of one millimetre from a point.
pub fn grid_after(v: i32) -> i32 {
    grid_ceil(v + 1)
}

/// And the one before it.
pub fn grid_before(v: i32) -> i32 {
    grid_floor(v - 1)
}

/// Whether `v` is a grid point.
pub fn is_on_grid(v: i32) -> bool {
    on_grid(v) == v
}

/// How far apart, in millimetres, the places a node may be are, and the lines
/// its terminals are on.
///
/// A node's terminals are on lines five millimetres apart -- see
/// `style::terminal_at` -- so that a line between two level terminals runs
/// along one, and so a node moves in steps of five: a node at an arbitrary
/// millimetre would have its terminals rounded up to five millimetres from
/// where they belong, and in a node seventeen millimetres tall the two rows
/// would meet.
pub const NODE_STEP_MM: i32 = 5;

/// Where in each step a node's upper left corner is: a millimetre short of a
/// line, so the terminals' first line is a millimetre inside the node -- in
/// from its border, as a terminal is meant to be -- and the node's corner is
/// still on the millimetre grid.
pub const NODE_OFFSET_MM: i32 = NODE_STEP_MM - 1;

/// Whether the millimetre `mm` is a place a node's corner may be.
fn is_node_mm(mm: i32) -> bool {
    mm.rem_euclid(NODE_STEP_MM) == NODE_OFFSET_MM
}

/// The millimetre a pixel is nearest.
fn mm_of(v: i32) -> i32 {
    (v as f64 / PX_PER_MM).round() as i32
}

/// The place a node may be that is nearest `v`.
///
/// # Arguments
///
/// * `v` - A position, in pixels; the lower of two equally near places
///         wins.
///
/// # Returns
///
/// The place, in pixels.
pub fn on_node_grid(v: i32) -> i32 {
    let mm = mm_of(v);
    let below = mm - (mm - NODE_OFFSET_MM).rem_euclid(NODE_STEP_MM);
    let above = below + NODE_STEP_MM;
    let (low, high) = (grid_point(below), grid_point(above));
    if v - low <= high - v {
        low
    } else {
        high
    }
}

/// The lowest place a node may be at or above `v`.
///
/// # Arguments
///
/// * `v` - A position, in pixels.
///
/// # Returns
///
/// The place, in pixels.
pub fn node_ceil(v: i32) -> i32 {
    let mut mm = mm_of(grid_ceil(v));
    while !is_node_mm(mm) {
        mm += 1;
    }
    grid_point(mm)
}

/// The highest place a node may be at or below `v`.
///
/// # Arguments
///
/// * `v` - A position, in pixels.
///
/// # Returns
///
/// The place, in pixels.
pub fn node_floor(v: i32) -> i32 {
    let mut mm = mm_of(grid_floor(v));
    while !is_node_mm(mm) {
        mm -= 1;
    }
    grid_point(mm)
}

/// The next place a node may be after `v`, one step on.
pub fn node_after(v: i32) -> i32 {
    node_ceil(v + 1)
}

/// And the one before it.
pub fn node_before(v: i32) -> i32 {
    node_floor(v - 1)
}

/// The lowest point on one of the lines terminals are on -- every
/// [`NODE_STEP_MM`] millimetres -- at or after `v`.
///
/// # Arguments
///
/// * `v` - A position, in pixels.
///
/// # Returns
///
/// The point on the line, in pixels.
pub fn line_ceil(v: i32) -> i32 {
    let mut mm = (v as f64 / PX_PER_MM).floor() as i32;
    while mm.rem_euclid(NODE_STEP_MM) != 0 || grid_point(mm) < v {
        mm += 1;
    }
    grid_point(mm)
}

/// And the highest at or before it.
///
/// # Arguments
///
/// * `v` - A position, in pixels.
///
/// # Returns
///
/// The point on the line, in pixels.
pub fn line_floor(v: i32) -> i32 {
    let mut mm = (v as f64 / PX_PER_MM).ceil() as i32;
    while mm.rem_euclid(NODE_STEP_MM) != 0 || grid_point(mm) > v {
        mm -= 1;
    }
    grid_point(mm)
}

/// Whether `v` is on one of those lines.
pub fn is_on_line(v: i32) -> bool {
    line_ceil(v) == v
}

/// Whether `v` is a place a node may be.
pub fn is_on_node_grid(v: i32) -> bool {
    is_on_grid(v) && is_node_mm(mm_of(v))
}

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
/// `payloads` is where the payload working pane ends and the vehicle's
/// begins, and `vehicle` is where the vehicle's ends and the gateway working
/// pane's begins. Together they say where all three are, the first starting
/// at nought and the last ending at [`WORKING_WIDTH`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dividers {
    pub payloads: i32,
    pub vehicle: i32,
}

/// Where the dividers fall, for panes of these percentages of the whole.
///
/// `gateway_percent` and `vehicle_percent` are the gateway and vehicle panes'
/// shares; the payload working pane takes what is left. Kept by pane rather
/// than by position, so the settings say the same thing whichever order the
/// panes are drawn in: the payloads on the left, then the vehicle, then the
/// gateway. The dividers are movable, so this is arithmetic rather than a pair
/// of constants. What is kept is the percentages -- see `settings.rs`, which
/// says why those and not pixel counts -- and this is the one place they
/// become positions.
///
/// Snapped to the grid, which is not decoration: a node held against a divider
/// is a node whose position is that divider, and a divider off the grid would
/// put it off the grid too.
///
/// # Arguments
///
/// * `gateway_percent` - The gateway pane's share of the working width, in
///                       per cent; brought inside the limits
///                       `settings::shares` keeps.
/// * `vehicle_percent` - The vehicle pane's share, likewise.
///
/// # Returns
///
/// The two dividers, on the grid.
pub fn dividers_for(gateway_percent: i32, vehicle_percent: i32) -> Dividers {
    let (gateway_percent, vehicle_percent) =
        settings::shares(gateway_percent, vehicle_percent);
    Dividers {
        payloads: at_percent(100 - gateway_percent - vehicle_percent),
        vehicle: at_percent(100 - gateway_percent),
    }
}

/// Where the dividers sit until somebody moves them.
///
/// The defaults, for the tests that are about the panes rather than about
/// moving a line between them. A running editor reads them from the settings
/// instead -- see `settings::load` -- because somebody may have moved them.
pub fn default_dividers() -> Dividers {
    dividers_for(settings::DEFAULT_GATEWAY, settings::DEFAULT_VEHICLE)
}

/// A position in the working pane, snapped to the grid.
fn at_percent(percent: i32) -> i32 {
    on_grid(WORKING_WIDTH * percent / 100)
}

/// And back: what percentages of the working pane these dividers leave the
/// gateway and vehicle panes.
///
/// Rounded to whole percents, which is what is written. The round trip is
/// stable -- a percent is a little over a grid step, so dividers snapped to
/// the grid come back to themselves -- and `tests::the_dividers_survive_being
/// _written_down` is what says so.
///
/// # Arguments
///
/// * `dividers` - Where the panes meet.
///
/// # Returns
///
/// The gateway and vehicle panes' shares of the working width, in whole per
/// cent, brought inside the limits `settings::shares` keeps.
pub fn percents_for(dividers: Dividers) -> (i32, i32) {
    let whole =
        |x: i32| (x as f32 * 100.0 / WORKING_WIDTH as f32).round() as i32;
    settings::shares(
        100 - whole(dividers.vehicle),
        whole(dividers.vehicle) - whole(dividers.payloads),
    )
}

/// Whether a node of this type is a payload, and so belongs on the left.
///
/// A type this build does not know is not a payload: it goes in the vehicle
/// working pane, which is the largest of the three and the one a reader looks
/// at first -- and a node whose type nothing recognises is a node somebody has
/// to look at.
///
/// # Arguments
///
/// * `node_type` - The node type's spelling, as in a configuration's
///                 `type:`.
///
/// # Returns
///
/// Whether the type is a known payload type.
pub fn is_payload(node_type: &str) -> bool {
    NodeType::ALL
        .iter()
        .find(|t| t.spelling() == node_type)
        .is_some_and(|t| t.is_payload())
}

/// Whether a node of this type is the ground link, and so belongs on the right.
///
/// The one type in its pane. It is there because it is not part of the vehicle
/// at all: it is the other end of the link, and a drawing that puts it in the
/// vehicle's own column says the vehicle contains the ground station.
///
/// # Arguments
///
/// * `node_type` - The node type's spelling, as in a configuration's
///                 `type:`.
///
/// # Returns
///
/// Whether the type is `gateway` or `gateway_fprime`, the known ground link
/// types.
pub fn is_gateway(node_type: &str) -> bool {
    NodeType::ALL
        .iter()
        .find(|t| t.spelling() == node_type)
        .is_some_and(|t| {
            matches!(t, NodeType::GatewayRadio | NodeType::GatewayFprime)
        })
}

/// Which working pane a node of this type belongs in.
///
/// The one place the question is answered. Three panes and three families: the
/// ground link, the vehicle's own graph, and what it carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Payloads,
    Vehicle,
    Gateway,
}

impl Pane {
    /// What the pane is called where a person can see it.
    ///
    /// The label in the pane's top left corner is in the markup, that being
    /// window text; these are the same words, for what the tests say when a
    /// pane rule is broken.
    ///
    /// # Returns
    ///
    /// The pane's name: `Gateway`, `Vehicle` or `Payloads`.
    pub fn label(self) -> &'static str {
        match self {
            Pane::Gateway => "Gateway",
            Pane::Vehicle => "Vehicle",
            Pane::Payloads => "Payloads",
        }
    }
}

/// Which pane this type belongs in.
///
/// # Arguments
///
/// * `node_type` - The node type's spelling, as in a configuration's
///                 `type:`.
///
/// # Returns
///
/// The pane; [`Pane::Vehicle`] for a type this build does not know.
pub fn pane_of(node_type: &str) -> Pane {
    if is_payload(node_type) {
        Pane::Payloads
    } else if is_gateway(node_type) {
        Pane::Gateway
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
/// a stripe across the pane rather than its head. Thirty pixels deep, which
/// is the line box of the small type -- twelve points, sixteen pixels, every
/// window's one size -- and a pixel, with `Space.tight` above it and below:
/// the markup asks the labels how deep they are and this is the generous
/// answer to the same question, so the row the placing keeps off is never
/// shallower than the row that is drawn.
pub const NAMES: (i32, i32) = (0, 30);

/// Those bands as rectangles, one per pane, for the placing to keep off.
///
/// They abut at the dividers, so what the placing keeps off is the same row
/// either way; three is what says the row is three panes' heads rather than
/// one strip laid over them, and it is what the window draws.
///
/// # Arguments
///
/// * `dividers` - Where the panes meet.
///
/// # Returns
///
/// Each band's left, top, width and height, in pixels, for the payload,
/// vehicle and gateway panes in that order.
fn names_bands(dividers: Dividers) -> [(i32, i32, u32, u32); 3] {
    let (top, height) = NAMES;
    [Pane::Payloads, Pane::Vehicle, Pane::Gateway].map(|pane| {
        let (left, right) = edges_of(pane, dividers);
        (left, top, (right - left).max(0) as u32, height as u32)
    })
}

/// Where a node of this type may sit: the leftmost and rightmost x.
///
/// The right-hand bound allows for the node's own width, so a node confined to
/// a pane is drawn inside it rather than hanging over its edge. Both bounds
/// are grid points, inside the pane: a node held against an edge is on the
/// grid as well as in its pane.
///
/// # Arguments
///
/// * `node_type` - The node type's spelling, which gives its pane and
///                 its width.
/// * `dividers`  - Where the panes meet.
///
/// # Returns
///
/// The leftmost and rightmost x the node's upper left corner may have,
/// in pixels. The right may be left of the left when the pane is
/// narrower than the node.
pub fn pane_for(node_type: &str, dividers: Dividers) -> (i32, i32) {
    let (width, _) = size_of(node_type);
    let width = width as i32;
    let (left, right) = edges_of(pane_of(node_type), dividers);
    (node_ceil(left), node_floor(right - width))
}

/// Where a pane begins and ends, in the coordinates the file holds.
///
/// For the window, which draws the labels and the dividers from it, and for
/// placing a new node at the top of its own pane.
///
/// # Arguments
///
/// * `pane`     - The pane.
/// * `dividers` - Where the panes meet.
///
/// # Returns
///
/// The pane's left and right edges, in pixels.
pub fn edges_of(pane: Pane, dividers: Dividers) -> (i32, i32) {
    match pane {
        Pane::Payloads => (0, dividers.payloads),
        Pane::Vehicle => (dividers.payloads, dividers.vehicle),
        Pane::Gateway => (dividers.vehicle, WORKING_WIDTH),
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
///
/// # Arguments
///
/// * `node_type` - The node type's spelling, which gives its pane and
///                 its width.
/// * `dividers`  - Where the panes meet.
/// * `(x, y)`    - Where the node's upper left corner is asked to be, in
///                 pixels.
///
/// # Returns
///
/// The nearest place a node may be that is inside its pane and not above
/// the top of the sheet.
pub fn confine(
    node_type: &str,
    dividers: Dividers,
    (x, y): (i32, i32),
) -> (i32, i32) {
    let (left, right) = pane_for(node_type, dividers);
    // The lower bound first: a pane narrower than a node -- which the five per
    // cent rule allows, any pane at its narrowest being 48 pixels --
    // has a right bound to the left of its left one, and a node in it sits at
    // its left edge and overhangs.
    //
    // On the grid: the nearest place a node may be -- a step of five
    // millimetres, see `NODE_STEP_MM` -- which the bounds are as well, so a
    // node held at an edge is on the grid too. Never above the first of them:
    // a node off the top of the sheet is a node nobody can reach.
    let y = on_node_grid(y).max(node_ceil(0));
    (on_node_grid(x).clamp(left, right.max(left)), y)
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
///
/// # Arguments
///
/// * `taken`     - Every box already on the sheet: left, top, width and
///                 height, in pixels.
/// * `node_type` - The new node's type spelling, which gives its pane and
///                 its size.
/// * `dividers`  - Where the panes meet.
/// * `visible`   - The top of the part of the sheet the window shows, and
///                 its height, in pixels.
///
/// # Returns
///
/// Where the new node's upper left corner goes, in pixels.
pub fn place_in(
    taken: &[(i32, i32, u32, u32)],
    node_type: &str,
    dividers: Dividers,
    visible: (i32, i32),
) -> (i32, i32) {
    let (width, height) = size_of(node_type);
    let (left, right) = pane_for(node_type, dividers);
    let right = right.max(left);
    let bottom = node_floor(WORKING_HEIGHT - height as i32);

    // What is already drawn, and the band at the top of each pane that its
    // name is written in: nothing the editor places itself covers a name. See
    // `NAMES`.
    let bands = names_bands(dividers);
    let clear = |x: i32, y: i32| {
        !taken.iter().chain(bands.iter()).any(
            |(theirs, tall, their_width, their_height)| {
                x < theirs + *their_width as i32
                    && *theirs < x + width as i32
                    && y < tall + *their_height as i32
                    && *tall < y + height as i32
            },
        )
    };
    // Every place tried is one a node may be, a step apart.
    let across = |y: i32| {
        let mut x = left;
        while x <= right {
            if clear(x, y) {
                return Some((x, y));
            }
            x = node_after(x);
        }
        None
    };

    // In view, highest first. The view's own bottom, not the sheet's: a node
    // hanging half off the bottom of the window is not in view.
    let (top, how_much) = visible;
    let top = top.max(0);
    let mut y = node_ceil(top);
    while y <= (top + how_much - height as i32).min(bottom) {
        if let Some(at) = across(y) {
            return at;
        }
        y = node_after(y);
    }

    // Above the view, lowest first: the nearest place is the shortest scroll.
    let mut y = node_floor((top - height as i32).min(bottom));
    while y >= 0 {
        if let Some(at) = across(y) {
            return at;
        }
        y = node_before(y);
    }

    // Below it, highest first.
    let mut y = node_ceil((top + how_much).max(0));
    while y <= bottom {
        if let Some(at) = across(y) {
            return at;
        }
        y = node_after(y);
    }

    (left, node_ceil(0))
}

/// The nominal size of a type, for placing a link's ends.
///
/// # Arguments
///
/// * `node_type` - The node type's spelling, as in a configuration's
///                 `type:`.
///
/// # Returns
///
/// Its width and height in pixels; 104 by 40 for a type this build does
/// not know.
pub fn size_of(node_type: &str) -> (u32, u32) {
    NodeType::ALL
        .iter()
        .find(|t| t.spelling() == node_type)
        .map(|t| t.appearance().size)
        .unwrap_or((104, 40))
}

/// Where a node is to be drawn, as [`settle`] moves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placing<'a> {
    pub node_type: &'a str,
    pub at: (i32, i32),
}

/// Puts every node inside the pane its type belongs to, and on the grid.
///
/// What a configuration is drawn from when it is read: a file written before
/// the canvas was divided, or by hand, or with the divider somewhere else, has
/// nodes outside their own panes and off the grid, and a drawing has to say
/// which of the two to believe. A node that has to move keeps its height and
/// takes the near edge of its own pane, which is the smallest move that obeys
/// the rule -- see [`confine`]. Where that lands it on top of another node it
/// steps down the grid until it is clear: a payload column read out of a file
/// where the payloads were in a row would otherwise become one node with
/// eleven behind it.
///
/// Every node is checked against every other, whichever pane it is in, because
/// a node of the other pane overlapping this one is just as unreadable.
///
/// One rule for the editor, which settles what it opens, and for tcspecial's
/// visual window, which draws what the editor would.
///
/// # Arguments
///
/// * `nodes`    - The nodes' types and positions; each position is moved
///                in place.
/// * `dividers` - Where the panes meet.
///
/// # Returns
///
/// How many nodes were moved.
pub fn settle(nodes: &mut [Placing], dividers: Dividers) -> usize {
    let mut moved = 0;

    for at in 0..nodes.len() {
        let node = nodes[at];
        let want = confine(node.node_type, dividers, node.at);
        if want == node.at {
            continue;
        }

        // Clear of the nodes that are already settled, and of the ones that
        // were in the right place to begin with.
        let (width, height) = size_of(node.node_type);
        let mut want = want;
        for _ in 0..200 {
            let clashes = nodes.iter().enumerate().any(|(other, node)| {
                if other == at {
                    return false;
                }
                let (theirs, tall) = size_of(node.node_type);
                want.0 < node.at.0 + theirs as i32
                    && node.at.0 < want.0 + width as i32
                    && want.1 < node.at.1 + tall as i32
                    && node.at.1 < want.1 + height as i32
            });
            if !clashes {
                break;
            }
            want.1 = node_ceil(want.1 + 32);
        }

        nodes[at].at = want;
        moved += 1;
    }

    moved
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
            for (band, pane) in
                bands
                    .iter()
                    .zip([Pane::Payloads, Pane::Vehicle, Pane::Gateway])
            {
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
