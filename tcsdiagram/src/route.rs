//! Where a link's line goes, and the arrow at the end of it.
//!
//! Links were drawn as straight lines between two points, which is the
//! obvious thing and the wrong one. In a graph of twenty nodes, diagonals
//! cross each other at arbitrary angles, and a line that crosses three others
//! at three different angles cannot be followed to its end -- which is the one
//! thing a reader needs from it. So a link is routed the way a schematic
//! routes one: out horizontally, across vertically, and in horizontally, with
//! the corners rounded.
//!
//! And around whatever is in the way. A line that runs through a node says
//! there is a connection where there is none, which is worse than a line that
//! is hard to follow: a reader tracing it arrives at the wrong node and has no
//! way of knowing. So every route is chosen rather than computed -- a handful
//! of shapes are laid out, each is checked against every node on the page, and
//! the one that crosses fewest, bends least and runs shortest is the one
//! drawn, in that order of priority.
//!
//! And around the lines already drawn, which is the other thing a reader
//! cannot recover. Two lines that *cross* are two lines: the crossing is a
//! point, and each can be followed through it. Two lines that run along the
//! same track are one line on the screen -- whichever was drawn second is the
//! only one anybody sees, and a link that is invisible is a link a reader
//! takes to be absent. So a route is laid against the routes already laid,
//! every column and row is offered in lanes either side of itself, and a
//! shared run costs more than a bend: see [`shared`] and [`LANE`].
//!
//! The route is computed here rather than in the markup, for two reasons. It
//! is arithmetic, and arithmetic in a layout language is hard to read and
//! impossible to test; and this way the shape of every route is checkable --
//! that it starts where the source is, ends where the target is, has no
//! diagonal in it, and keeps off the nodes.
//!
//! What comes back is SVG path data, which is what Slint's `Path` takes as its
//! `commands`. Absolute canvas coordinates, so the element can span the canvas
//! and need no bounding box of its own.

/// How far a corner is rounded, in pixels.
///
/// Small: a corner rounded much more than this stops reading as a right angle
/// and the route stops looking like a route. It is also reduced where a
/// segment is too short to give it room -- see [`radius`].
const RADIUS: f32 = 8.0;

/// How far a line runs straight out of a node before it turns.
///
/// Without it a link leaving a node would turn immediately at the node's edge,
/// and two links leaving the same node would overlap for their whole first
/// segment. A stub gives each one somewhere to be.
const STUB: f32 = 16.0;

/// How far a line keeps off a node it is not joined to.
///
/// A line that grazes a node's edge reads as a line that touches it, and a
/// reader cannot tell "passes close" from "arrives here" at a glance. Six
/// pixels is enough to see the gap at the size a diagram is read at, and
/// little enough that the detour it forces is small.
const CLEAR: f32 = 6.0;

/// The arrowhead's length along the line, and its half-width across it.
///
/// Large enough to be seen without looking for it. It was nine by five, which
/// is the size an arrowhead is in a drawing you are already reading closely --
/// butted against a node's edge, in the line's own grey, it was easy to miss
/// entirely and the direction of a link with it.
const HEAD: (f32, f32) = (14.0, 6.0);

/// The flow marker's length along the line, and its half-width across it.
///
/// Smaller than the head: it says the same thing in the middle of the run,
/// where there is nothing else to confuse it with, and a second arrow as big
/// as the first would read as a second link.
const MARKER: (f32, f32) = (11.0, 5.0);

/// How long a run has to be before a marker is put in the middle of it.
///
/// A marker on a run barely longer than itself is a shape with no room to
/// point: it would touch both corners and read as a thickening of the line.
const ROOM: f32 = 28.0;

/// How far apart two lines have to be to read as two lines, in pixels.
///
/// A line is drawn five pixels wide, so two tracks less than that apart are
/// one track with a thick line on it. Two millimetres, seven and a half
/// pixels: the width and a little, which is a visible gap at the sizes here
/// without spreading a bundle of links across half a pane -- and a whole
/// number of millimetres, so a lane moved off a run on the grid is on the grid
/// too. See [`on_mm`].
///
/// It is both the spacing of the lanes a route may be moved into and the
/// tolerance for calling two runs the same run.
const LANE: f32 = (2.0 * crate::pane::PX_PER_MM) as f32;

/// The nearest millimetre to `v`, which is where a run of a route goes.
///
/// A route's runs are on the millimetre grid, as the nodes are, so the lines
/// of a drawing line up with each other and with the boxes they run between:
/// every vertical run, and every horizontal one but the two level with the
/// line's own ends, which are where its terminals put them. See
/// `pane::PX_PER_MM`.
fn on_mm(v: f32) -> f32 {
    crate::pane::on_grid(v.round() as i32) as f32
}

/// The millimetre at or beyond `v` going the way `sign` says: out from a node,
/// so a run moved onto the grid is never moved nearer to the node it is
/// keeping clear of.
///
/// # Arguments
///
/// * `v`    - A position, in pixels.
/// * `sign` - Which way is out: positive for increasing positions,
///            anything else for decreasing.
///
/// # Returns
///
/// The grid point, in pixels.
fn mm_out(v: f32, sign: f32) -> f32 {
    let at = v.round() as i32;
    if sign > 0.0 {
        crate::pane::grid_ceil(at) as f32
    } else {
        crate::pane::grid_floor(at) as f32
    }
}

/// How much two runs have to share before it counts as an overlap.
///
/// A stub and a little. Two links leaving one terminal leave from the same
/// point, so their first segments run along each other for the length of a
/// stub whatever either of them does afterwards -- there is nothing to route
/// around, and at that length it reads as one line dividing rather than as a
/// line gone missing. Anything longer is a route that could have gone
/// elsewhere.
///
/// It also covers the shorter case: two lines touching where they pass, which
/// is what a crossing looks like from close up. Without a floor here a route
/// would take a detour to save three pixels of coincidence.
const TOUCH: f32 = STUB + 2.0;

/// Something on the page a line should not run through: a node.
///
/// Given to the router rather than looked up by it, because this module knows
/// about geometry and nothing about documents -- which is what lets every rule
/// below be tested with four numbers and no editor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Obstacle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Obstacle {
    /// A node of this size at this place.
    ///
    /// # Arguments
    ///
    /// * `x`      - Its left edge, in pixels.
    /// * `y`      - Its top edge, in pixels.
    /// * `width`  - Its width, in pixels.
    /// * `height` - Its height, in pixels.
    ///
    /// # Returns
    ///
    /// The obstacle.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Obstacle {
            x,
            y,
            width,
            height,
        }
    }
}

/// Where a line starts and finishes, and which way it points at each end.
///
/// The directions are what terminals made necessary. A link used to leave
/// every node by the middle of its right edge and arrive at the middle of the
/// next one's left edge, so "out, across, in" meant one thing and the
/// arrowhead always pointed right. Now a link leaves the terminal it is
/// attached to -- and the vehicleward stream leaves a node's *left* side, on
/// its way from the ground link to a payload drawn further left -- so the way
/// out of the source and the way into the target are two separate facts, and
/// neither of them is always right.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ends {
    /// Where the line starts: a point on the source's edge.
    pub from: (f32, f32),
    /// Where it finishes: a point on the target's edge.
    pub to: (f32, f32),
    /// Whether it goes right as it leaves the source.
    pub leaves_right: bool,
    /// Whether it is going right as it reaches the target, which is to say it
    /// arrives at the target's left side. The arrowhead points this way.
    pub arrives_right: bool,
}

impl Ends {
    /// Left to right at both ends, which is how every link was drawn before
    /// there were terminals.
    ///
    /// For the tests of the shapes themselves, which are about geometry and
    /// not about which corner a stream leaves by. Every line the canvas draws
    /// leaves a terminal and arrives at one, so every line it draws says which
    /// way each of its ends points.
    ///
    /// # Arguments
    ///
    /// * `x1` - The source end's x, in pixels.
    /// * `y1` - The source end's y, in pixels.
    /// * `x2` - The target end's x, in pixels.
    /// * `y2` - The target end's y, in pixels.
    ///
    /// # Returns
    ///
    /// The ends, leaving and arriving rightward.
    pub fn forward(x1: f32, y1: f32, x2: f32, y2: f32) -> Ends {
        Ends {
            from: (x1, y1),
            to: (x2, y2),
            leaves_right: true,
            arrives_right: true,
        }
    }

    /// Both ends as the terminals put them.
    ///
    /// # Arguments
    ///
    /// * `from`          - Where the line starts, on the source's edge, in
    ///                     pixels.
    /// * `to`            - Where it finishes, on the target's edge, in pixels.
    /// * `leaves_right`  - Whether it goes right as it leaves the source.
    /// * `arrives_right` - Whether it is going right as it reaches the target.
    ///
    /// # Returns
    ///
    /// The ends.
    pub fn new(
        from: (f32, f32),
        to: (f32, f32),
        leaves_right: bool,
        arrives_right: bool,
    ) -> Ends {
        Ends {
            from,
            to,
            leaves_right,
            arrives_right,
        }
    }

    /// Where the line turns after leaving the source.
    ///
    /// A stub out along the edge it left, so that two links leaving one node
    /// do not overlap for their whole first segment -- and out *away* from the
    /// node, which is the half of it the direction decides: a stub to the
    /// right of a terminal on the left side would run back through the node it
    /// just left.
    fn out(&self) -> f32 {
        self.from.0 + if self.leaves_right { STUB } else { -STUB }
    }

    /// And where it makes its last turn before the target.
    fn in_(&self) -> f32 {
        self.to.0 + if self.arrives_right { -STUB } else { STUB }
    }

    /// Whether the two ends point the same way, so that a route can run
    /// straight from one stub to the other.
    fn agree(&self) -> bool {
        self.leaves_right == self.arrives_right
    }
}

/// A box a line may not cross, as two corners.
type Box = (f32, f32, f32, f32);

/// What makes one route better than another, worst thing first.
///
/// How many nodes it crosses, how many times it turns, and how far it runs.
/// Compared in that order, which is what the ordering of a tuple does: a route
/// that crosses nothing beats one that crosses something however long it is,
/// and length only settles a tie between two routes that turn as often as each
/// other.
/// What a route costs, in the order the terms are weighed: how many nodes it
/// runs through, how far it runs along a line already drawn, how many times it
/// bends, and how far it goes.
///
/// Nodes first, because a line through a node is a lie about the graph.
/// Overlap next, ahead of bends: a line that cannot be seen is worse than a
/// line with a corner in it, and the corner is what buys the lane. The shared
/// run is counted in lane-widths rather than pixels so that a few pixels of
/// coincidence cannot outweigh a bend.
type Cost = (usize, u32, usize, u32);

/// A route and what it costs.
type Weighed = (Cost, Vec<(f32, f32)>);

/// The path of a link from `(x1, y1)` to `(x2, y2)`, around `around`.
///
/// Orthogonal throughout: out of the source, across, and into the target, with
/// as many more turns as it takes to miss what is in the way. Where the two
/// ends are at the same height and nothing is between them the route is one
/// straight line, which is the common case and wants no corners at all.
pub fn route(ends: Ends, around: &[Obstacle]) -> String {
    drawn(&way(ends, around, &[]))
}

/// The points a link's line visits, laid clear of the routes already laid.
///
/// For the caller that draws a canvas full of links: lay each one, keep the
/// points, and hand them to the next. Everything else about a link's drawing
/// -- its path, its arrowhead, its marker, where its label sits -- is derived
/// from the points rather than worked out again, so the five of them cannot
/// disagree about where the line went.
///
/// `taken` is every route already drawn on this canvas, in the order they were
/// drawn. A link is routed against them so that no two lines run along the
/// same track -- see [`shared`].
pub fn lay(
    ends: Ends,
    around: &[Obstacle],
    taken: &[Vec<(f32, f32)>],
) -> Vec<(f32, f32)> {
    way(ends, around, taken)
}

/// One laid route as path data.
pub fn drawn_route(points: &[(f32, f32)]) -> String {
    drawn(points)
}

/// The arrowhead at the target end, as a filled triangle.
///
/// A separate path because it is filled and the route is stroked, and Slint
/// gives one `Path` one of each. Pointing along the last segment, which is
/// always horizontal -- and which way along it is [`Ends::arrives_right`]: a
/// groundward link arrives at its target's right side travelling left, and an
/// arrowhead that pointed right there would point back up its own line.
///
/// # Arguments
///
/// * `ends` - The link's ends; only `to` and `arrives_right` are used.
///
/// # Returns
///
/// The triangle as SVG path data.
pub fn arrow(ends: Ends) -> String {
    let (length, half) = HEAD;
    let (x2, y2) = ends.to;
    // The tip at the node's edge, where the line ends, so the arrow touches
    // what it points at: a head set back from the edge leaves a stub of line
    // sticking out of its point.
    if ends.arrives_right {
        triangle(x2 - length / 2.0, y2, Way::Right, (length, half))
    } else {
        triangle(x2 + length / 2.0, y2, Way::Left, (length, half))
    }
}

/// A second arrow, in the middle of the route's longest run.
///
/// The head at the end says which way the data goes, and says it where the
/// line meets a node -- which is the one place a diagram is already crowded,
/// and the one place two links between the same pair of nodes look alike. A
/// marker out in the middle of the longest run says it where there is room to
/// see it, so a pair of links between one pair of nodes can be told apart at a
/// glance instead of by following each to its end.
///
/// The longest run rather than a chosen one, because the route is chosen now
/// and its shape is not known in advance: whichever segment the route gave
/// most room to is the segment with room for this. A route whose longest run
/// has no room for one gets none -- see [`ROOM`] -- which is a short link,
/// where the head at the end is already next to everything else about it.
pub fn marker(ends: Ends, around: &[Obstacle]) -> String {
    marker_on(&way(ends, around, &[]))
}

/// The same, on a route already laid: see [`lay`]. Which way it points is the
/// run's own direction, so the route's ends are not asked.
///
/// # Arguments
///
/// * `points` - The route's points, as [`lay`] gives them.
///
/// # Returns
///
/// The marker as SVG path data, or an empty string when the longest run
/// is shorter than [`ROOM`] or there is no run at all.
pub fn marker_on(points: &[(f32, f32)]) -> String {
    let (from, to) = match longest(points) {
        Some(run) => run,
        None => return String::new(),
    };
    if length(from, to) < ROOM {
        return String::new();
    }

    let way_ = if (to.1 - from.1).abs() < 0.5 {
        if to.0 > from.0 {
            Way::Right
        } else {
            Way::Left
        }
    } else if to.1 > from.1 {
        Way::Down
    } else {
        Way::Up
    };
    triangle((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0, way_, MARKER)
}

/// Where a link's label goes: the middle of its longest run.
///
/// On the line rather than halfway between the two nodes, which is where it
/// used to be. A route that detours around a node has a middle nowhere near
/// the straight line between its ends, and a label sitting there is a label
/// beside a different link -- or beside nothing at all.
pub fn label_at(ends: Ends, around: &[Obstacle]) -> (f32, f32) {
    label_on(ends, &way(ends, around, &[]))
}

/// The same, on a route already laid: see [`lay`].
///
/// # Arguments
///
/// * `ends`   - The link's ends, whose midpoint is the answer for a
///              route with no run.
/// * `points` - The route's points, as [`lay`] gives them.
///
/// # Returns
///
/// The point the label is centred on, in pixels.
pub fn label_on(ends: Ends, points: &[(f32, f32)]) -> (f32, f32) {
    match longest(points) {
        Some((from, to)) => ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0),
        None => (
            (ends.from.0 + ends.to.0) / 2.0,
            (ends.from.1 + ends.to.1) / 2.0,
        ),
    }
}

/// The line a gesture is drawing, from where it started to where the pointer
/// is, as a dashed path.
///
/// Straight, where a link's own route is orthogonal: this one is in a person's
/// hand and follows it, and a route that turned corners as the hand moved
/// would be a line that jumps about while being aimed. Dashed, so that a line
/// which is not a connection yet does not read as one -- and dashed by
/// construction, a run of short subpaths, because a path in this toolkit has
/// no dash pattern of its own.
///
/// The anchor is the end that is staying put: the terminal the gesture started
/// at when a line is being drawn, and the far end of the line when one is
/// being carried by its end.
///
/// # Arguments
///
/// * `anchor` - The end that is staying put, in pixels.
/// * `at`     - Where the pointer is, in pixels.
///
/// # Returns
///
/// The dashes as SVG path data, or an empty string when the two points
/// are less than a pixel apart.
pub fn drawing(anchor: (f32, f32), at: (f32, f32)) -> String {
    /// How long a dash is, and the gap after it.
    const DASH: f32 = 6.0;
    const GAP: f32 = 5.0;

    let (dx, dy) = (at.0 - anchor.0, at.1 - anchor.1);
    let span = (dx * dx + dy * dy).sqrt();
    // Nowhere to draw to, which is the first moment of every gesture.
    if span < 1.0 {
        return String::new();
    }
    let (ux, uy) = (dx / span, dy / span);

    let mut out = String::new();
    let dash = |from: f32, to: f32, out: &mut String| {
        out.push_str(&format!(
            "M {:.1} {:.1} L {:.1} {:.1} ",
            anchor.0 + ux * from,
            anchor.1 + uy * from,
            anchor.0 + ux * to,
            anchor.1 + uy * to
        ));
    };

    let mut along = 0.0;
    while along < span {
        let mut end = (along + DASH).min(span);
        // The last dash ends under the pointer rather than wherever the
        // pattern happened to stop: the tip is what a person is aiming with,
        // and a line that stopped a gap short of their hand would miss by a
        // gap. So a dash with less than a gap left after it runs to the end.
        if span - end < GAP {
            end = span;
        }
        dash(along, end, &mut out);
        along = end + GAP;
    }
    out.trim_end().to_string()
}

/// How many nodes other than its own ends a link's line runs through.
///
/// The nodes it is joined to do not count, which is the same distinction
/// [`blocking`] draws: a line leaves its source and arrives at its target, so
/// crossing those two boxes is what it is for. It is not a distinction about
/// edges either, now that a point where a line meets a node can be inside the
/// node's box -- a slanted outline does not reach its own box at the row a
/// terminal sits in, so the line stops where the shape is, which is in from
/// the side. See `style::room_at`.
///
/// For the tests of the window, which have a whole configuration -- nodes,
/// links and all -- but not the shapes between them. The rules here are
/// checked against four numbers and a box or two, which is how a rule should
/// be checked; this is how a test of a real configuration asks the same
/// question of every link in it at once.
///
/// # Arguments
///
/// * `ends`   - The link's ends.
/// * `around` - Every node on the page.
///
/// # Returns
///
/// How many of `around` the line runs through, not counting a node that
/// holds either end.
pub fn runs_through(ends: Ends, around: &[Obstacle]) -> usize {
    let (from, to) = (ends.from, ends.to);
    let points = way(ends, around, &[]);
    around
        .iter()
        .filter(|b| !holds(b, from) && !holds(b, to))
        .filter(|b| {
            let box_ = (b.x, b.y, b.x + b.width, b.y + b.height);
            points.windows(2).any(|s| crosses(s[0], s[1], box_))
        })
        .count()
}

/// The points a link's line visits, corners and all.
///
/// Every shape worth trying is laid out and the best is taken. "Best" is four
/// things in order: it crosses the fewest nodes, it shares the least of its
/// length with a line already drawn, it bends the fewest times, and it is the
/// shortest. Crossings first because a line through a node is a lie about the
/// graph; a shared run before bends because a line hidden under another is a
/// link a reader takes to be absent, and a corner is a small price for being
/// visible; bends before length because the lengths of two orthogonal routes
/// between one pair of points are usually equal, and of two equal routes the
/// one with fewer turns is the one that can be followed.
///
/// # Arguments
///
/// * `ends`   - The link's ends.
/// * `around` - Every node on the page, its own two included.
/// * `taken`  - The points of every route already laid on the canvas.
///
/// # Returns
///
/// The best route's points, from the source to the target; the two ends
/// alone if no shape was offered.
fn way(
    ends: Ends,
    around: &[Obstacle],
    taken: &[Vec<(f32, f32)>],
) -> Vec<(f32, f32)> {
    let boxes = blocking(around, ends.from, ends.to);
    let mut best: Option<Weighed> = None;

    for points in shapes(ends, around) {
        let cost: Cost = (
            crossings(&points, &boxes),
            shared(&points, taken),
            points.len() - 2,
            walked(&points),
        );
        // Strictly better, so the first shape to reach a cost keeps it: the
        // shapes are offered in the order they are preferred, and a tie goes
        // to the one offered first.
        if best.as_ref().is_none_or(|(theirs, _)| cost < *theirs) {
            best = Some((cost, points));
        }
    }

    best.map(|(_, points)| points)
        .unwrap_or_else(|| vec![ends.from, ends.to])
}

/// Every route worth considering, in the order they are preferred.
///
/// # Arguments
///
/// * `ends`   - The link's ends.
/// * `around` - Every node on the page, which gives the columns and rows
///              a run may be moved to.
///
/// # Returns
///
/// The routes, each as its points with the stops that are not turns
/// taken out.
fn shapes(ends: Ends, around: &[Obstacle]) -> Vec<Vec<(f32, f32)>> {
    let ((x1, y1), (x2, y2)) = (ends.from, ends.to);
    let (out, in_) = (ends.out(), ends.in_());
    // Whether the target lies the way the line set off, which is what makes a
    // straight run or a Z possible at all: a line that left rightward and has
    // to arrive somewhere on its left has to go around, whichever way it is
    // pointing when it gets there.
    let ahead = if ends.leaves_right {
        in_ > out
    } else {
        in_ < out
    };
    let mut tries: Vec<Vec<(f32, f32)>> = Vec::new();

    // Straight through: the two ends level, pointing the same way, and the
    // target ahead of the source. Nothing beats it when it is clear.
    if (y2 - y1).abs() < 1.0 && ends.agree() && ahead {
        tries.push(vec![(x1, y1), (x2, y2)]);
    }

    // A Z: out, across, in. Down the middle first, then down either side of
    // anything in the way. Only where both ends point the same way -- a Z
    // between two stubs that point at each other's nodes would double back
    // through one of them.
    if ends.agree() && ahead {
        for across in columns(out, in_, around) {
            tries.push(vec![(x1, y1), (across, y1), (across, y2), (x2, y2)]);
        }
    }

    // Around: out, along a clear row, and back in. The only shape that
    // reaches a target behind its source, and the one a link falls back to
    // when every column between the two is blocked.
    //
    // Its two vertical runs are at the stubs, so two links leaving one column
    // of nodes would run down the same line as each other for as far as they
    // both go -- which is the overlap this shape used to be the main source
    // of. Each is offered in lanes as well, stepped *away* from its own node:
    // outward is clear space, where a step inward would cross the node the
    // stub belongs to.
    let outward = if ends.leaves_right { 1.0 } else { -1.0 };
    let inward = if ends.arrives_right { -1.0 } else { 1.0 };
    let lanes = |v: f32, sign: f32| -> Vec<f32> {
        // The stub itself first, then lanes out from it on the grid: the stub
        // is where the line leaves the node, and moving it is only worth it to
        // get off another line's track. Each a millimetre, the stub's taken
        // outward to the next one so it is still a stub's length from its
        // node.
        let stub = mm_out(v, sign);
        (0..4)
            .map(|n| on_mm(stub + sign * n as f32 * LANE))
            .collect()
    };
    for band in rows(y1, y2, around) {
        for left in lanes(out, outward) {
            for right in lanes(in_, inward) {
                tries.push(vec![
                    (x1, y1),
                    (left, y1),
                    (left, band),
                    (right, band),
                    (right, y2),
                    (x2, y2),
                ]);
            }
        }
    }

    tries.iter().map(|points| tidied(points)).collect()
}

/// Where a vertical run might go: down the middle, and beside each node.
///
/// # Arguments
///
/// * `out`    - Where the line turns after leaving the source, in pixels.
/// * `in_`    - Where it makes its last turn before the target, in
///              pixels.
/// * `around` - Every node on the page.
///
/// # Returns
///
/// The x of each column, in the order they are preferred, all between
/// `out` and `in_`.
fn columns(out: f32, in_: f32, around: &[Obstacle]) -> Vec<f32> {
    // On the millimetre grid, each beside a node taken out from it so it
    // keeps its clearance: see `on_mm`.
    let mut xs = vec![on_mm((out + in_) / 2.0)];
    for b in around {
        xs.push(mm_out(b.x - CLEAR, -1.0));
        xs.push(mm_out(b.x + b.width + CLEAR, 1.0));
    }
    // And a lane either side of each, and a second one beyond that, so two
    // links that would run down the same column have somewhere else to go.
    // After the columns themselves, because the shapes are tried in the order
    // they are offered and a clear middle is still the route to prefer.
    xs = laned(xs).into_iter().map(on_mm).collect();
    // Only between the two stubs: a vertical run outside them would double
    // back through the node it just left. Either order, because a leftward
    // link's stubs run the other way about.
    let (near, far) = (out.min(in_), out.max(in_));
    xs.retain(|x| *x >= near - 0.05 && *x <= far + 0.05);
    tidy_numbers(xs)
}

/// Where a horizontal run might go: between the two ends, level with either
/// of them, beside each node, and clear above and below everything.
///
/// # Arguments
///
/// * `y1`     - The source end's y, in pixels.
/// * `y2`     - The target end's y, in pixels.
/// * `around` - Every node on the page.
///
/// # Returns
///
/// The y of each row, in the order they are preferred, none above
/// [`CLEAR`].
fn rows(y1: f32, y2: f32, around: &[Obstacle]) -> Vec<f32> {
    // On the millimetre grid, as the columns are, but for the two rows level
    // with the line's own ends: those are where its terminals are, and a run
    // along one of them is the line leaving or arriving.
    let mut ys = vec![on_mm((y1 + y2) / 2.0), y2, y1];
    for b in around {
        ys.push(mm_out(b.y - CLEAR, -1.0));
        ys.push(mm_out(b.y + b.height + CLEAR, 1.0));
    }
    let top = around.iter().map(|b| b.y).fold(f32::MAX, f32::min);
    let bottom = around
        .iter()
        .map(|b| b.y + b.height)
        .fold(f32::MIN, f32::max);
    if top < f32::MAX {
        ys.push(mm_out(top - ROOM, -1.0));
        ys.push(mm_out(bottom + ROOM, 1.0));
    }
    // Lanes either side of each, as the columns have: see `laned`. The two
    // ends' own rows are kept as they are; every lane is on the grid.
    ys = laned(ys)
        .into_iter()
        .map(|y| if y == y1 || y == y2 { y } else { on_mm(y) })
        .collect();
    // Nothing above the top of the canvas: a run off the edge is a run
    // nobody can see, and the clipping makes it look like a line that stops.
    ys.retain(|y| *y >= CLEAR);
    tidy_numbers(ys)
}

/// The same numbers, near-duplicates dropped, order kept.
///
/// Order kept because it is the order the shapes are preferred in: the middle
/// first, and whatever a node pushed the line to after it.
///
/// # Arguments
///
/// * `values` - The numbers, in the order they are preferred.
///
/// # Returns
///
/// The first of each group of numbers less than half a pixel apart.
fn tidy_numbers(values: Vec<f32>) -> Vec<f32> {
    let mut kept: Vec<f32> = Vec::with_capacity(values.len());
    for v in values {
        if !kept.iter().any(|k| (k - v).abs() < 0.5) {
            kept.push(v);
        }
    }
    kept
}

/// The same points, with the stops that are not turns taken out.
///
/// A shape laid out from a formula can repeat a point -- a band level with one
/// of the ends gives a corner with nothing either side of it -- and can leave
/// three points in a line. Both would be drawn as corners that do not turn,
/// and both would count against the shape when the turns are counted.
///
/// # Arguments
///
/// * `points` - The shape's points, in order.
///
/// # Returns
///
/// The points that are turns, and the two ends.
fn tidied(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut kept: Vec<(f32, f32)> = Vec::with_capacity(points.len());
    for p in points {
        match kept.last() {
            Some(last) if length(*last, *p) < 0.5 => continue,
            _ => kept.push(*p),
        }
    }
    // And again for the straight-through stops: a, b, c where b is on the
    // line from a to c.
    let mut out: Vec<(f32, f32)> = Vec::with_capacity(kept.len());
    for (i, p) in kept.iter().enumerate() {
        let between = i > 0 && i + 1 < kept.len() && {
            let (before, after) = (kept[i - 1], kept[i + 1]);
            // On the line, and with its neighbours on opposite sides of it.
            // The second half matters as much as the first: three points in a
            // row where the outer two are on the *same* side is a line that
            // runs out and comes back, which is a reversal and not a point to
            // drop. Dropping one turned a route that approached a target from
            // the right into one that approached it from the left -- which is
            // a line arriving at the wrong side of a node, and with terminals
            // that is a line arriving at the wrong terminal.
            let flat = (before.1 - p.1).abs() < 0.5
                && (after.1 - p.1).abs() < 0.5
                && (before.0 - p.0) * (after.0 - p.0) < 0.0;
            let upright = (before.0 - p.0).abs() < 0.5
                && (after.0 - p.0).abs() < 0.5
                && (before.1 - p.1) * (after.1 - p.1) < 0.0;
            flat || upright
        };
        if !between {
            out.push(*p);
        }
    }
    out
}

/// Each node as a box a line may not cross.
///
/// Every node is given the clearance it is owed, except the two this link
/// joins: a line has to touch those, and it leaves them along their own edge.
/// Which two is not told but worked out, from the ends of the route lying on
/// their boundaries -- so the caller hands over the page and not a list with
/// two holes in it.
///
/// # Arguments
///
/// * `around` - Every node on the page.
/// * `from`   - Where the line starts, in pixels.
/// * `to`     - Where it finishes, in pixels.
///
/// # Returns
///
/// Each node's box, as its left, top, right and bottom, grown by
/// [`CLEAR`] on every side unless it holds `from` or `to`.
fn blocking(around: &[Obstacle], from: (f32, f32), to: (f32, f32)) -> Vec<Box> {
    around
        .iter()
        .map(|b| {
            let joined = holds(b, from) || holds(b, to);
            let by = if joined { 0.0 } else { CLEAR };
            (b.x - by, b.y - by, b.x + b.width + by, b.y + b.height + by)
        })
        .collect()
}

/// Whether a point is on or inside a node.
///
/// # Arguments
///
/// * `b`      - The node.
/// * `(x, y)` - The point, in pixels; within half a pixel of the node's
///              edge counts as on it.
///
/// # Returns
///
/// Whether the node holds the point.
fn holds(b: &Obstacle, (x, y): (f32, f32)) -> bool {
    x >= b.x - 0.5
        && x <= b.x + b.width + 0.5
        && y >= b.y - 0.5
        && y <= b.y + b.height + 0.5
}

/// The same tracks, and then a lane either side of each, and a second beyond
/// that.
///
/// The tracks themselves first, so a route still prefers the middle of the gap
/// or the side of a node; the lanes after, in the order a route should try
/// them -- one lane out either way, then two, and so on to four. Four a side
/// is eight millimetres, thirty pixels, which is as far as a line can be
/// moved and still read as the line between those two nodes; a ninth line
/// through one gap is a diagram with something else wrong with it.
///
/// # Arguments
///
/// * `tracks` - The positions of the tracks, in pixels.
///
/// # Returns
///
/// The tracks, followed by each moved one to four lanes either way.
fn laned(tracks: Vec<f32>) -> Vec<f32> {
    let mut out = tracks.clone();
    for step in [
        LANE,
        -LANE,
        2.0 * LANE,
        -2.0 * LANE,
        3.0 * LANE,
        -3.0 * LANE,
        4.0 * LANE,
        -4.0 * LANE,
    ] {
        for track in &tracks {
            out.push(track + step);
        }
    }
    out
}

/// How far this route runs along a route already drawn, in lane-widths.
///
/// Only straight runs on the same track count. Two lines crossing share a
/// point, which is a crossing and reads as one; two lines on the same track
/// share a length, and the one drawn first is not on the screen at all. A
/// shorter coincidence than [`TOUCH`] is not counted, because it is what a
/// crossing looks like from close up and is not worth a detour.
///
/// In lane-widths rather than pixels so that the number is comparable with the
/// bends it is weighed against: see [`Cost`].
///
/// # Arguments
///
/// * `points` - The route's points.
/// * `taken`  - The points of every route already drawn.
///
/// # Returns
///
/// The length shared, in whole lane-widths.
pub fn shared(points: &[(f32, f32)], taken: &[Vec<(f32, f32)>]) -> u32 {
    let mut along = 0.0;
    for mine in points.windows(2) {
        for theirs in taken.iter().flat_map(|route| route.windows(2)) {
            along += together(mine[0], mine[1], theirs[0], theirs[1]);
        }
    }
    (along / LANE) as u32
}

/// How far two segments run along one another, in pixels.
///
/// # Arguments
///
/// * `a1` - One end of the first segment, in pixels.
/// * `a2` - The other end of the first segment.
/// * `b1` - One end of the second segment.
/// * `b2` - The other end of the second segment.
///
/// # Returns
///
/// The length the two run along one another when both are horizontal or
/// both vertical, no more than a lane apart, and share more than
/// [`TOUCH`]; otherwise 0.
fn together(
    a1: (f32, f32),
    a2: (f32, f32),
    b1: (f32, f32),
    b2: (f32, f32),
) -> f32 {
    let flat = |p: (f32, f32), q: (f32, f32)| (p.1 - q.1).abs() < 0.5;
    let upright = |p: (f32, f32), q: (f32, f32)| (p.0 - q.0).abs() < 0.5;
    let run = |lo1: f32, hi1: f32, lo2: f32, hi2: f32| {
        let shared = hi1.min(hi2) - lo1.max(lo2);
        if shared > TOUCH {
            shared
        } else {
            0.0
        }
    };

    if flat(a1, a2) && flat(b1, b2) && (a1.1 - b1.1).abs() <= LANE {
        return run(
            a1.0.min(a2.0),
            a1.0.max(a2.0),
            b1.0.min(b2.0),
            b1.0.max(b2.0),
        );
    }
    if upright(a1, a2) && upright(b1, b2) && (a1.0 - b1.0).abs() <= LANE {
        return run(
            a1.1.min(a2.1),
            a1.1.max(a2.1),
            b1.1.min(b2.1),
            b1.1.max(b2.1),
        );
    }
    0.0
}

/// How many nodes a route runs through.
///
/// # Arguments
///
/// * `points` - The route's points.
/// * `boxes`  - The boxes it may not cross, as [`blocking`] gives them.
///
/// # Returns
///
/// How many of `boxes` some segment of the route crosses.
fn crossings(points: &[(f32, f32)], boxes: &[Box]) -> usize {
    boxes
        .iter()
        .filter(|b| points.windows(2).any(|s| crosses(s[0], s[1], **b)))
        .count()
}

/// Whether an axis-aligned segment runs through a box.
///
/// Open on every side on purpose: a segment along a box's edge touches it and
/// does not cross it, which is exactly what a line leaving its own node does.
fn crosses(
    from: (f32, f32),
    to: (f32, f32),
    (bx1, by1, bx2, by2): Box,
) -> bool {
    let (x1, x2) = (from.0.min(to.0), from.0.max(to.0));
    let (y1, y2) = (from.1.min(to.1), from.1.max(to.1));
    x1 < bx2 && bx1 < x2 && y1 < by2 && by1 < y2
}

/// How far a route runs, in whole pixels.
///
/// # Arguments
///
/// * `points` - The route's points.
///
/// # Returns
///
/// The sum of its segments' lengths, rounded to whole pixels.
fn walked(points: &[(f32, f32)]) -> u32 {
    points
        .windows(2)
        .map(|s| length(s[0], s[1]))
        .sum::<f32>()
        .round() as u32
}

/// The length of one segment.
fn length(from: (f32, f32), to: (f32, f32)) -> f32 {
    (to.0 - from.0).abs() + (to.1 - from.1).abs()
}

/// The longest straight run of a route.
///
/// # Arguments
///
/// * `points` - The route's points.
///
/// # Returns
///
/// The two ends of its longest segment, or `None` if it has fewer than
/// two points.
fn longest(points: &[(f32, f32)]) -> Option<((f32, f32), (f32, f32))> {
    points
        .windows(2)
        .max_by(|a, b| {
            length(a[0], a[1])
                .partial_cmp(&length(b[0], b[1]))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|s| (s[0], s[1]))
}

/// A route's points as path data, with the corners rounded.
///
/// Each turn becomes a straight run stopping short of the corner, a quadratic
/// through it, and the next run from where the quadratic left off. The radius
/// is cut down to fit the shorter of the two segments, so a corner never
/// bulges past the point it is meant to bend at.
///
/// # Arguments
///
/// * `points` - The route's points.
///
/// # Returns
///
/// The route as SVG path data, or an empty string if it has fewer than
/// two points.
fn drawn(points: &[(f32, f32)]) -> String {
    if points.len() < 2 {
        return String::new();
    }

    let mut path = format!("M {:.1} {:.1}", points[0].0, points[0].1);
    for i in 1..points.len() - 1 {
        let (before, at, after) = (points[i - 1], points[i], points[i + 1]);
        let r = radius(length(before, at), length(at, after));
        let into = toward(at, before, r);
        let out = toward(at, after, r);
        path.push_str(&format!(
            " L {:.1} {:.1} Q {:.1} {:.1} {:.1} {:.1}",
            into.0, into.1, at.0, at.1, out.0, out.1
        ));
    }
    let last = points[points.len() - 1];
    path.push_str(&format!(" L {:.1} {:.1}", last.0, last.1));
    path
}

/// The point `by` pixels from `at` in the direction of `toward`.
///
/// # Arguments
///
/// * `at`     - The point to start from, in pixels.
/// * `toward` - The point to go towards.
/// * `by`     - How far to go, in pixels.
///
/// # Returns
///
/// The point reached, or `at` itself when `toward` is on top of it.
fn toward(at: (f32, f32), toward: (f32, f32), by: f32) -> (f32, f32) {
    let run = length(at, toward);
    if run < 0.05 {
        return at;
    }
    (
        at.0 + (toward.0 - at.0) / run * by,
        at.1 + (toward.1 - at.1) / run * by,
    )
}

/// How far a corner may be rounded, given the room on either side of it.
///
/// Never more than half the shorter segment, or the two corners of one turn
/// would overlap and the line would bulge where it should bend.
fn radius(along: f32, across: f32) -> f32 {
    RADIUS.min(along / 2.0).min(across / 2.0).max(0.0)
}

/// Which way along a run the data goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Way {
    Right,
    Left,
    Down,
    Up,
}

/// A filled triangle at `(cx, cy)`, pointing this way.
///
/// # Arguments
///
/// * `cx`   - The x of the triangle's centre, in pixels.
/// * `cy`   - The y of its centre, in pixels.
/// * `way`  - Which way it points.
/// * `size` - Its length along the way it points and its half-width
///            across it, in pixels.
///
/// # Returns
///
/// The triangle as SVG path data.
fn triangle(cx: f32, cy: f32, way: Way, size: (f32, f32)) -> String {
    let (length, half) = size;
    let (tip, back) = (length / 2.0, length / 2.0);
    let (apex, left, right) = match way {
        Way::Right => (
            (cx + tip, cy),
            (cx - back, cy - half),
            (cx - back, cy + half),
        ),
        Way::Left => (
            (cx - tip, cy),
            (cx + back, cy - half),
            (cx + back, cy + half),
        ),
        Way::Down => (
            (cx, cy + tip),
            (cx - half, cy - back),
            (cx + half, cy - back),
        ),
        Way::Up => (
            (cx, cy - tip),
            (cx - half, cy + back),
            (cx + half, cy + back),
        ),
    };

    format!(
        "M {:.1} {:.1} L {:.1} {:.1} L {:.1} {:.1} Z",
        apex.0, apex.1, left.0, left.1, right.0, right.1
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every number in a path, in the order they appear.
    fn numbers(path: &str) -> Vec<f32> {
        path.split_whitespace()
            .filter_map(|t| t.parse::<f32>().ok())
            .collect()
    }

    /// The points a path visits, as pairs.
    fn points(path: &str) -> Vec<(f32, f32)> {
        numbers(path).chunks(2).map(|c| (c[0], c[1])).collect()
    }

    /// Whether a route runs through a node.
    ///
    /// Against the node's own box rather than the box plus its clearance, so
    /// this answers the question that matters -- is the line inside the shape
    /// -- and answers it the same way for a node the link joins as for one it
    /// passes. A line leaving its own node runs along that node's edge, which
    /// is touching and not crossing.
    fn through(route: &[(f32, f32)], b: &Obstacle) -> bool {
        let box_ = (b.x, b.y, b.x + b.width, b.y + b.height);
        route.windows(2).any(|s| crosses(s[0], s[1], box_))
    }

    /// A node on the page, by the editor's own idea of a node's size.
    fn node(x: f32, y: f32) -> Obstacle {
        Obstacle::new(x, y, 152.0, 56.0)
    }

    /// Two links that would run along one track are laid along two.
    ///
    /// The rule this costs a bend for: two lines on the same track are one
    /// line on the screen, and the one laid first is not on it at all -- so a
    /// reader counting the lines out of a node counts one and takes the other
    /// to be absent. Crossing is fine, and so is touching where they pass;
    /// what must not happen is a shared *run*.
    #[test]
    fn two_lines_do_not_run_along_one_track() {
        // Two links from two terminals to two others, far enough apart that
        // nothing forces them together -- and both wanting the same column
        // down the middle of the gap, which is the shape each prefers.
        let first = Ends::forward(192.0, 68.0, 560.0, 240.0);
        let second = Ends::forward(192.0, 120.0, 560.0, 300.0);

        let one = way(first, &[], &[]);
        // Laid on its own, the second takes the same column as the first:
        // that is the shape it prefers, and the reason this rule is needed.
        let alone = way(second, &[], &[]);
        assert!(
            shared(&alone, std::slice::from_ref(&one)) > 0,
            "the two would share a run if nothing stopped them: {one:?} and \
             {alone:?}"
        );

        // Laid against it, it does not.
        let two = way(second, &[], std::slice::from_ref(&one));
        assert_eq!(
            shared(&two, std::slice::from_ref(&one)),
            0,
            "laid against the first, the second should share no run with it: \
             {one:?} and {two:?}"
        );

        // And it is still a route: orthogonal, from the source to the target.
        assert_eq!(two.first().copied(), Some(second.from));
        assert_eq!(two.last().copied(), Some(second.to));
        for pair in two.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            assert!(
                (a.0 - b.0).abs() < 0.5 || (a.1 - b.1).abs() < 0.5,
                "a diagonal crept in: {a:?} to {b:?}"
            );
        }
    }

    /// Crossing costs nothing, and neither does touching where two lines pass.
    ///
    /// The distinction the rule rests on. Two lines that cross share a point,
    /// which reads as a crossing and can be followed through; a line is only
    /// lost when it shares a length. So a short coincidence is not counted,
    /// and a route is not bent to avoid one.
    #[test]
    fn crossing_is_not_overlapping() {
        // One across, one down, meeting in the middle: a crossing.
        let across = vec![(100.0, 200.0), (400.0, 200.0)];
        let down = vec![(250.0, 100.0), (250.0, 300.0)];
        assert_eq!(
            shared(&down, std::slice::from_ref(&across)),
            0,
            "a crossing is free"
        );

        // Two runs on the same line, overlapping by less than a line's own
        // width and a little: what a crossing looks like from close up.
        let brief = vec![(100.0, 200.0), (108.0, 200.0)];
        assert_eq!(
            shared(&brief, std::slice::from_ref(&across)),
            0,
            "a touch is free"
        );

        // And a real shared run, which is not.
        let along = vec![(150.0, 203.0), (350.0, 203.0)];
        assert!(
            shared(&along, &[across]) > 0,
            "two hundred pixels on the same track is an overlap"
        );
    }

    /// A route starts at its source and ends at its target.
    ///
    /// The one thing that must be true of every route, whatever shape it took
    /// to get there: a line that does not touch the nodes it joins is a line
    /// that says nothing.
    #[test]
    fn every_route_joins_the_two_points_it_was_given() {
        let crowd = [
            node(100.0, 60.0),
            node(300.0, 60.0),
            node(300.0, 200.0),
            node(500.0, 120.0),
        ];
        for around in [&[][..], &crowd[..]] {
            for (x1, y1, x2, y2) in [
                (0.0, 0.0, 200.0, 0.0),
                (0.0, 0.0, 200.0, 120.0),
                (0.0, 120.0, 200.0, 0.0),
                (300.0, 40.0, 100.0, 200.0),
                (10.0, 10.0, 12.0, 400.0),
                (0.0, 0.0, 0.0, 0.0),
                (252.0, 88.0, 300.0, 228.0),
            ] {
                let path = route(Ends::forward(x1, y1, x2, y2), around);
                let visited = points(&path);
                assert!(path.starts_with('M'), "{path}");
                assert_eq!(visited.first().copied(), Some((x1, y1)), "{path}");
                assert_eq!(visited.last().copied(), Some((x2, y2)), "{path}");
            }
        }
    }

    /// A route level with itself is one straight line.
    ///
    /// The common case -- two nodes placed in a row -- and a corner in it
    /// would be a bend in a line that does not bend.
    #[test]
    fn a_level_route_is_a_single_line() {
        let path = route(Ends::forward(0.0, 64.0, 240.0, 64.0), &[]);
        assert_eq!(path, "M 0.0 64.0 L 240.0 64.0");
        assert!(!path.contains('Q'), "a level route was given a corner");

        // And within a pixel of level, which is where two nodes of the same
        // height but different sizes land.
        assert!(
            !route(Ends::forward(0.0, 64.0, 240.0, 64.4), &[]).contains('Q')
        );
    }

    /// A route that turns does so at right angles.
    ///
    /// Checked by walking the straight segments: each one holds either its x
    /// or its y, never both. The corners are quadratics and are allowed to be
    /// neither, which is what makes them corners.
    #[test]
    fn a_turning_route_has_no_diagonal_in_it() {
        // Including the routes that have to go around something, which are
        // the ones with segments nobody wrote out by hand.
        for (x1, y1, x2, y2, around) in [
            (0.0, 0.0, 240.0, 120.0, &[][..]),
            (0.0, 0.0, 600.0, 120.0, &[node(280.0, 0.0)][..]),
            (
                600.0,
                40.0,
                40.0,
                240.0,
                &[node(40.0, 212.0), node(600.0, 12.0)][..],
            ),
        ] {
            let route_ = way(Ends::forward(x1, y1, x2, y2), around, &[]);
            for pair in route_.windows(2) {
                let (from, to) = (pair[0], pair[1]);
                assert!(
                    (to.0 - from.0).abs() < 0.05
                        || (to.1 - from.1).abs() < 0.05,
                    "the segment from {from:?} to {to:?} is a diagonal: \
                     {route_:?}"
                );
            }
            assert!(
                route_.len() > 2,
                "a route that changes height should turn: {route_:?}"
            );
        }
    }

    /// A corner is never rounded further than there is room for.
    ///
    /// Two nodes almost level, or almost touching, leave a very short segment;
    /// a corner rounded by the full radius there would bulge past the point it
    /// is meant to bend at.
    #[test]
    fn a_corner_is_rounded_only_as_far_as_it_fits() {
        assert_eq!(radius(100.0, 100.0), RADIUS);
        assert_eq!(radius(4.0, 100.0), 2.0);
        assert_eq!(radius(100.0, 6.0), 3.0);
        assert_eq!(radius(0.0, 0.0), 0.0);

        // And a route between two nodes two pixels apart is still a route.
        let path = route(Ends::forward(0.0, 0.0, 34.0, 4.0), &[]);
        assert!(points(&path).first().copied() == Some((0.0, 0.0)), "{path}");
        assert!(points(&path).last().copied() == Some((34.0, 4.0)), "{path}");
    }

    /// A link that goes backwards goes around rather than through.
    ///
    /// A target to the left of its source cannot be reached through the middle
    /// without the line doubling back across both nodes.
    #[test]
    fn a_backwards_route_does_not_double_back_through_its_nodes() {
        let path = route(Ends::forward(300.0, 40.0, 100.0, 200.0), &[]);
        let visited = points(&path);
        assert_eq!(visited.first().copied(), Some((300.0, 40.0)));
        assert_eq!(visited.last().copied(), Some((100.0, 200.0)));

        // It leaves to the right of its source and arrives from the left of
        // its target, which is what the handles on a node mean.
        assert!(visited[1].0 > 300.0, "{path}");
    }

    /// A route goes around a node that is in its way.
    ///
    /// The thing this module was rewritten for. A straight run between two
    /// nodes with a third between them used to be drawn through the third,
    /// which says there is a connection where there is none -- and a reader
    /// tracing the line arrives at the wrong node with no way of knowing.
    #[test]
    fn a_route_goes_around_a_node_in_the_way() {
        let between = node(280.0, 40.0);
        let around = [node(40.0, 40.0), between, node(560.0, 40.0)];

        // Two nodes level with each other, with a third exactly between them:
        // the one case a straight line would otherwise be drawn for.
        let route_ = way(Ends::forward(192.0, 68.0, 560.0, 68.0), &around, &[]);
        assert!(
            !through(&route_, &between),
            "the line runs through the node between its ends: {route_:?}"
        );
        assert!(
            route_.len() > 2,
            "it should have had to turn to miss it: {route_:?}"
        );
        // And it still joins the two it is for.
        assert_eq!(route_.first().copied(), Some((192.0, 68.0)));
        assert_eq!(route_.last().copied(), Some((560.0, 68.0)));
    }

    /// A route that goes backwards misses the node it is going to.
    ///
    /// The fault that was found in the shipped example: the return link's long
    /// run was placed at the height between its two ends, which was inside the
    /// box of the node it was arriving at -- so the line entered the target
    /// through its middle and left again before coming back round to its
    /// edge.
    #[test]
    fn a_backwards_route_does_not_run_through_its_target() {
        let ground = Obstacle::new(24.0, 40.0, 152.0, 128.0);
        let payload = node(744.0, 160.0);
        let around = [payload, ground];

        // From the payload's right edge back round to the ground station's
        // left edge, which is the shape of every telemetry link.
        let route_ =
            way(Ends::forward(896.0, 188.0, 24.0, 104.0), &around, &[]);
        assert!(
            !through(&route_, &payload),
            "the line runs back through its own source: {route_:?}"
        );
        assert!(
            !through(&route_, &ground),
            "the line runs through the node it arrives at: {route_:?}"
        );
    }

    /// A line keeps its distance from a node it only passes.
    ///
    /// Not merely outside it: a line along a node's edge reads as a line that
    /// touches it, and a reader cannot tell "passes close" from "arrives here"
    /// at a glance.
    #[test]
    fn a_line_keeps_off_a_node_it_passes() {
        let passed = node(300.0, 100.0);
        let around = [node(40.0, 40.0), passed, node(600.0, 300.0)];
        let route_ =
            way(Ends::forward(192.0, 68.0, 600.0, 328.0), &around, &[]);

        let kept = (
            passed.x - CLEAR,
            passed.y - CLEAR,
            passed.x + passed.width + CLEAR,
            passed.y + passed.height + CLEAR,
        );
        assert!(
            !route_.windows(2).any(|s| crosses(s[0], s[1], kept)),
            "the line comes within {CLEAR} pixels of a node it passes: \
             {route_:?}"
        );
    }

    /// A route with nowhere clear to go is still a route.
    ///
    /// Boxed in -- a target walled off by nodes above, below and between --
    /// there is no shape that misses everything, and the best available is
    /// drawn rather than nothing. A link with no line is a connection that
    /// does not appear in the drawing at all, which is a worse failure than a
    /// line somebody has to look at twice.
    #[test]
    fn a_route_with_nowhere_clear_to_go_is_still_drawn() {
        let wall: Vec<Obstacle> = (0..6)
            .map(|n| Obstacle::new(240.0, n as f32 * 56.0, 152.0, 56.0))
            .collect();
        let path = route(Ends::forward(192.0, 28.0, 600.0, 300.0), &wall);
        let visited = points(&path);
        assert!(!path.is_empty());
        assert_eq!(visited.first().copied(), Some((192.0, 28.0)));
        assert_eq!(visited.last().copied(), Some((600.0, 300.0)));
    }

    /// Of two routes that miss everything, the straighter one is drawn.
    ///
    /// Crossings first, then turns, then length -- and turns before length
    /// because two orthogonal routes between one pair of points are usually
    /// the same length, and of two equal routes the one with fewer corners is
    /// the one that can be followed.
    #[test]
    fn a_clear_route_turns_as_little_as_it_can() {
        // Nothing in the way: two turns, which is a Z and the fewest a route
        // that changes height can have.
        let route_ = way(Ends::forward(192.0, 68.0, 600.0, 328.0), &[], &[]);
        assert_eq!(route_.len(), 4, "{route_:?}");
        // Down the middle, so that two links between the same pair of columns
        // do not lie on top of each other -- the millimetre nearest it, every
        // run of a route being on the grid.
        let middle = (192.0 + 16.0 + 600.0 - 16.0) / 2.0;
        assert_eq!(route_[1].0, on_mm(middle), "{route_:?}");
        assert!(crate::pane::is_on_grid(route_[1].0 as i32), "{route_:?}");
    }

    /// Every run of a route is on the millimetre grid but the two level with
    /// its ends, which are where its terminals put them.
    #[test]
    fn a_routes_runs_are_on_the_millimetre_grid() {
        let on_grid =
            |v: f32| crate::pane::is_on_grid(v as i32) && v.fract() == 0.0;
        let around = [node(300.0, 120.0), node(300.0, 200.0)];
        for ends in [
            Ends::forward(192.0, 68.0, 600.0, 328.0),
            // Backwards, which goes around: two vertical runs and a row.
            Ends::forward(600.0, 100.0, 150.0, 260.0),
            Ends::new((500.0, 90.0), (120.0, 140.0), false, false),
        ] {
            let route_ = way(ends, &around, &[]);
            for pair in route_.windows(2) {
                let ((x1, y1), (x2, y2)) = (pair[0], pair[1]);
                let at_an_end = |p: (f32, f32)| p == ends.from || p == ends.to;
                if (x1 - x2).abs() < 0.5
                    && !at_an_end(pair[0])
                    && !at_an_end(pair[1])
                {
                    assert!(on_grid(x1), "a vertical run at {x1}: {route_:?}");
                }
                let level_with_an_end = (y1 - ends.from.1).abs() < 0.5
                    || (y1 - ends.to.1).abs() < 0.5;
                if (y1 - y2).abs() < 0.5 && !level_with_an_end {
                    assert!(
                        on_grid(y1),
                        "a horizontal run at {y1}: {route_:?}"
                    );
                }
            }
        }
    }

    /// The arrowhead points along the line and sits at its end.
    #[test]
    fn the_arrowhead_is_at_the_target_and_points_into_it() {
        let head = arrow(Ends::forward(0.0, 64.0, 240.0, 64.0));
        let visited = points(&head);

        assert_eq!(visited[0], (240.0, 64.0), "the tip is at the target");
        assert!(head.ends_with('Z'), "the head should be a closed triangle");
        // And the two other corners are behind the tip, one either side.
        assert!(visited[1].0 < 240.0 && visited[2].0 < 240.0, "{head}");
        assert!(visited[1].1 < 64.0 && visited[2].1 > 64.0, "{head}");

        // Big enough to be seen without looking for it. The size is the
        // reason this was asked for twice: the direction was drawn all along,
        // in nine pixels of the line's own grey, against a node's edge.
        let (length, half) = HEAD;
        assert!(length >= 12.0 && half >= 5.0, "{HEAD:?} is a small arrow");
        assert_eq!(
            240.0 - visited[1].0,
            length,
            "the head is as long as it says"
        );
        assert_eq!(visited[2].1 - visited[1].1, half * 2.0, "and as wide");
    }

    /// A link that goes the other way is routed the other way about.
    ///
    /// The vehicleward stream, which is what terminals made necessary: it
    /// leaves a node's left side and arrives at the next one's right side, and
    /// the next one is drawn further left -- the ground link being the
    /// rightmost working pane and the payloads the leftmost. Every piece of
    /// the route has to turn around for it: the stub, the straight run, the
    /// arrowhead.
    #[test]
    fn a_leftward_link_runs_leftward() {
        let leftward = |x1: f32, y1: f32, x2: f32, y2: f32| {
            Ends::new((x1, y1), (x2, y2), false, false)
        };

        // Level and clear: one straight line, as the forward case is.
        assert_eq!(
            route(leftward(400.0, 100.0, 100.0, 100.0), &[]),
            "M 400.0 100.0 L 100.0 100.0"
        );

        // A step down, which is a Z the other way about: the first turn is to
        // the *left* of the source, because a stub to its right would run back
        // through the node the line just left.
        let route_ = way(leftward(400.0, 100.0, 100.0, 200.0), &[], &[]);
        assert_eq!(route_.len(), 4, "{route_:?}");
        assert!(route_[1].0 < 400.0, "the stub goes left: {route_:?}");
        assert!(route_[1].0 > 100.0, "and not past the target: {route_:?}");
        assert_eq!(route_.first().copied(), Some((400.0, 100.0)));
        assert_eq!(route_.last().copied(), Some((100.0, 200.0)));

        // And the head points left, into the target's right side. An
        // arrowhead pointing right there would point back up its own line.
        let head = arrow(leftward(400.0, 100.0, 100.0, 100.0));
        let visited = points(&head);
        assert_eq!(visited[0], (100.0, 100.0), "the tip is at the target");
        assert!(
            visited[1].0 > 100.0 && visited[2].0 > 100.0,
            "the head points left: {head}"
        );
    }

    /// A route whose ends point at each other goes around.
    ///
    /// The mixed case: a line leaving a node's right side that has to arrive
    /// at a node's right side. There is no Z that does it -- a vertical run
    /// between the two stubs would double back through one of the nodes -- so
    /// the shape is out, along a clear row, and back in, and both ends are
    /// still joined.
    #[test]
    fn a_route_between_ends_that_face_each_other_goes_around() {
        let facing = Ends::new((200.0, 100.0), (500.0, 220.0), true, false);
        let route_ = way(facing, &[], &[]);
        assert_eq!(route_.first().copied(), Some((200.0, 100.0)), "{route_:?}");
        assert_eq!(route_.last().copied(), Some((500.0, 220.0)), "{route_:?}");
        // Out to the right of the source and in from the right of the target,
        // which is what the two directions say.
        assert!(route_[1].0 > 200.0, "{route_:?}");
        assert!(
            route_[route_.len() - 2].0 > 500.0,
            "the last turn is to the right of the target: {route_:?}"
        );
        // Orthogonal throughout, like every other route.
        for pair in route_.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            assert!(
                (a.0 - b.0).abs() < 0.5 || (a.1 - b.1).abs() < 0.5,
                "{route_:?} has a diagonal"
            );
        }
    }

    /// A second arrow out in the middle of the run, pointing the way the data
    /// goes.
    ///
    /// Three routes, three directions: a level link points along itself, a
    /// link that changes height points down or up its own vertical run, and
    /// one that goes backwards points back the way it goes. Without this the
    /// direction was only legible where the line met a node -- which is where
    /// two links between one pair of nodes look most alike.
    #[test]
    fn the_marker_points_the_way_the_data_goes() {
        // Level: the middle of the line, pointing along it.
        let said = marker(Ends::forward(40.0, 100.0, 240.0, 100.0), &[]);
        let visited = points(&said);
        assert!(said.ends_with('Z'), "{said}");
        assert!(
            visited[0].0 > visited[1].0 && visited[0].0 > visited[2].0,
            "a level link's marker points right: {said}"
        );
        assert!(
            (visited[0].0 - 140.0).abs() < MARKER.0,
            "and sits in the middle of the line: {said}"
        );

        // Down the canvas: the vertical run is the longest, and it points
        // down it.
        let said = marker(Ends::forward(40.0, 40.0, 240.0, 240.0), &[]);
        let visited = points(&said);
        assert!(
            visited[0].1 > visited[1].1 && visited[0].1 > visited[2].1,
            "a link that descends points down: {said}"
        );
        assert!(
            (visited[0].0 - 140.0).abs() < MARKER.0,
            "on the vertical run, which is halfway across: {said}"
        );

        // And up it.
        let said = marker(Ends::forward(40.0, 240.0, 240.0, 40.0), &[]);
        let visited = points(&said);
        assert!(
            visited[0].1 < visited[1].1 && visited[0].1 < visited[2].1,
            "a link that climbs points up: {said}"
        );

        // Backwards: the target is to the left of the source, so the long run
        // carries the data back the way it came, and the marker points that
        // way. With nothing in the way the long run is the one that arrives
        // at the target, which is the fewest turns the shape can be drawn in.
        let said = marker(Ends::forward(400.0, 40.0, 40.0, 200.0), &[]);
        let visited = points(&said);
        assert!(
            visited[0].0 < visited[1].0 && visited[0].0 > 40.0,
            "a link that goes backwards points left: {said}"
        );
        assert!(
            (visited[0].1 - 200.0).abs() < MARKER.0,
            "on the long run back, which arrives at the target's own \
             height: {said}"
        );
    }

    /// A marker and a label sit on the line they belong to.
    ///
    /// Both used to be placed halfway between the two nodes, which is on the
    /// line only when the line is straight. A route that detours has a middle
    /// nowhere near that point, and a label there is a label beside a
    /// different link.
    #[test]
    fn the_marker_and_the_label_are_on_the_line() {
        let between = node(280.0, 40.0);
        let around = [node(40.0, 40.0), between, node(560.0, 40.0)];
        let (x1, y1, x2, y2) = (192.0, 68.0, 560.0, 68.0);
        let route_ = way(Ends::forward(x1, y1, x2, y2), &around, &[]);

        let on_the_line = |(x, y): (f32, f32)| {
            route_.windows(2).any(|s| {
                let (lo, hi) =
                    (s[0].0.min(s[1].0) - 0.5, s[0].0.max(s[1].0) + 0.5);
                let (top, bottom) =
                    (s[0].1.min(s[1].1) - 0.5, s[0].1.max(s[1].1) + 0.5);
                (lo..=hi).contains(&x) && (top..=bottom).contains(&y)
            })
        };

        let at = label_at(Ends::forward(x1, y1, x2, y2), &around);
        assert!(on_the_line(at), "the label is at {at:?}, off the line");
        // And not halfway between the two nodes, which is the old answer and
        // is inside the node this route goes around.
        assert!(
            !through(&[at, at], &between),
            "the label is inside the node the line avoids"
        );

        let said = marker(Ends::forward(x1, y1, x2, y2), &around);
        let tip = points(&said)[0];
        assert!(
            on_the_line((
                (points(&said)[1].0 + points(&said)[2].0) / 2.0,
                (points(&said)[1].1 + points(&said)[2].1) / 2.0,
            )) || on_the_line(tip),
            "the marker is at {tip:?}, off the line: {said}"
        );
    }

    /// A run with no room for a marker gets none.
    ///
    /// A triangle on a run barely longer than itself touches both corners and
    /// reads as a thickening of the line, which says nothing about direction
    /// and makes the line look like something else.
    #[test]
    fn a_short_run_has_no_marker() {
        // Two nodes almost touching, level: the whole route is one short run.
        assert_eq!(marker(Ends::forward(40.0, 100.0, 60.0, 100.0), &[]), "");

        // Where only some of the runs are short the marker goes on the
        // longest, which is the rule now that the route's shape is chosen
        // rather than known: two nodes nearly level have a vertical run of
        // ten pixels and two horizontal runs of a hundred, and the direction
        // is perfectly legible on one of those.
        let said = marker(Ends::forward(40.0, 100.0, 240.0, 110.0), &[]);
        assert!(!said.is_empty(), "a long run should carry a marker");
        let visited = points(&said);
        assert!(
            visited[0].0 > visited[1].0,
            "and it points the way the data goes: {said}"
        );
    }

    /// The line in somebody's hand is dashed, straight, and reaches the
    /// pointer.
    ///
    /// Straight because it is being aimed: a route that turned corners as the
    /// hand moved would jump about under it. Dashed because it is not a
    /// connection yet, and in pieces because a path here has no dash pattern
    /// of its own -- so the test is that there are several subpaths and that
    /// the last of them ends where the pointer is.
    #[test]
    fn the_line_being_drawn_is_a_dashed_run_to_the_pointer() {
        let said = drawing((0.0, 0.0), (100.0, 0.0));
        assert!(said.matches('M').count() > 5, "{said}");
        assert!(said.starts_with("M 0.0 0.0 L 6.0 0.0"), "{said}");
        assert!(said.ends_with("L 100.0 0.0"), "{said}");

        // Diagonal, and still ending under the pointer.
        let said = drawing((10.0, 10.0), (60.0, 60.0));
        assert!(said.ends_with("L 60.0 60.0"), "{said}");

        // Whatever the length, the tip is under the pointer: that is what a
        // person is aiming with, and a dash pattern that stopped where it
        // stopped would miss by a gap.
        for span in 2..400 {
            let at = (span as f32 / 3.0, span as f32);
            let said = drawing((0.0, 0.0), at);
            assert!(
                said.ends_with(&format!("L {:.1} {:.1}", at.0, at.1)),
                "{span}: {said}"
            );
        }

        // And nothing at all where the pointer has not moved, which is the
        // first moment of every gesture.
        assert_eq!(drawing((20.0, 20.0), (20.0, 20.5)), "");
    }
}
