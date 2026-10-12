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
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Obstacle {
            x,
            y,
            width,
            height,
        }
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
type Cost = (usize, usize, u32);

/// A route and what it costs.
type Weighed = (Cost, Vec<(f32, f32)>);

/// The path of a link from `(x1, y1)` to `(x2, y2)`, around `around`.
///
/// Orthogonal throughout: out of the source, across, and into the target, with
/// as many more turns as it takes to miss what is in the way. Where the two
/// ends are at the same height and nothing is between them the route is one
/// straight line, which is the common case and wants no corners at all.
pub fn route(x1: f32, y1: f32, x2: f32, y2: f32, around: &[Obstacle]) -> String {
    drawn(&way(x1, y1, x2, y2, around))
}

/// The arrowhead at the target end, as a filled triangle.
///
/// A separate path because it is filled and the route is stroked, and Slint
/// gives one `Path` one of each. Pointing along the last segment, which is
/// always horizontal and always arrives from the left -- so the triangle does
/// not have to be rotated.
pub fn arrow(x2: f32, y2: f32) -> String {
    let (length, half) = HEAD;
    // The tip at the node's edge, where the line ends, so the arrow touches
    // what it points at: a head set back from the edge leaves a stub of line
    // sticking out of its point.
    triangle(x2 - length / 2.0, y2, Way::Right, (length, half))
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
pub fn marker(x1: f32, y1: f32, x2: f32, y2: f32, around: &[Obstacle]) -> String {
    let points = way(x1, y1, x2, y2, around);
    let (from, to) = match longest(&points) {
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
    triangle(
        (from.0 + to.0) / 2.0,
        (from.1 + to.1) / 2.0,
        way_,
        MARKER,
    )
}

/// Where a link's label goes: the middle of its longest run.
///
/// On the line rather than halfway between the two nodes, which is where it
/// used to be. A route that detours around a node has a middle nowhere near
/// the straight line between its ends, and a label sitting there is a label
/// beside a different link -- or beside nothing at all.
pub fn label_at(x1: f32, y1: f32, x2: f32, y2: f32, around: &[Obstacle]) -> (f32, f32) {
    let points = way(x1, y1, x2, y2, around);
    match longest(&points) {
        Some((from, to)) => ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0),
        None => ((x1 + x2) / 2.0, (y1 + y2) / 2.0),
    }
}

/// How many of `around` a route between these points runs through.
///
/// For the tests of the window, which have a whole configuration -- nodes,
/// links and all -- but not the shapes between them. The rules here are
/// checked against four numbers and a box or two, which is how a rule should
/// be checked; this is how a test of a real configuration asks the same
/// question of every link in it at once.
#[cfg(test)]
pub fn runs_through(x1: f32, y1: f32, x2: f32, y2: f32, around: &[Obstacle]) -> usize {
    let points = way(x1, y1, x2, y2, around);
    around
        .iter()
        .filter(|b| {
            let box_ = (b.x, b.y, b.x + b.width, b.y + b.height);
            points.windows(2).any(|s| crosses(s[0], s[1], box_))
        })
        .count()
}

/// The points a link's line visits, corners and all.
///
/// Every shape worth trying is laid out and the best is taken. "Best" is three
/// things in order: it crosses the fewest nodes, it bends the fewest times,
/// and it is the shortest. Crossings first because a line through a node is a
/// lie about the graph; bends before length because the lengths of two
/// orthogonal routes between one pair of points are usually equal, and of two
/// equal routes the one with fewer turns is the one that can be followed.
fn way(x1: f32, y1: f32, x2: f32, y2: f32, around: &[Obstacle]) -> Vec<(f32, f32)> {
    let boxes = blocking(around, (x1, y1), (x2, y2));
    let mut best: Option<Weighed> = None;

    for points in shapes(x1, y1, x2, y2, around) {
        let cost: Cost = (
            crossings(&points, &boxes),
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
        .unwrap_or_else(|| vec![(x1, y1), (x2, y2)])
}

/// Every route worth considering, in the order they are preferred.
fn shapes(x1: f32, y1: f32, x2: f32, y2: f32, around: &[Obstacle]) -> Vec<Vec<(f32, f32)>> {
    let out = x1 + STUB;
    let in_ = x2 - STUB;
    let mut tries: Vec<Vec<(f32, f32)>> = Vec::new();

    // Straight through: the two ends level and the target ahead of the
    // source. Nothing beats it when it is clear.
    if (y2 - y1).abs() < 1.0 && x2 > x1 {
        tries.push(vec![(x1, y1), (x2, y2)]);
    }

    // A Z: out, across, in. Down the middle first, then down either side of
    // anything in the way.
    if in_ > out {
        for across in columns(out, in_, around) {
            tries.push(vec![(x1, y1), (across, y1), (across, y2), (x2, y2)]);
        }
    }

    // Around: out, along a clear row, and back in. The only shape that
    // reaches a target to the left of its source, and the one a forward link
    // falls back to when every column through the middle is blocked.
    for band in rows(y1, y2, around) {
        tries.push(vec![
            (x1, y1),
            (out, y1),
            (out, band),
            (in_, band),
            (in_, y2),
            (x2, y2),
        ]);
    }

    tries.iter().map(|points| tidied(points)).collect()
}

/// Where a vertical run might go: down the middle, and beside each node.
fn columns(out: f32, in_: f32, around: &[Obstacle]) -> Vec<f32> {
    let mut xs = vec![(out + in_) / 2.0];
    for b in around {
        xs.push(b.x - CLEAR);
        xs.push(b.x + b.width + CLEAR);
    }
    // Only between the two stubs: a vertical run outside them would double
    // back through the node it just left.
    xs.retain(|x| *x >= out - 0.05 && *x <= in_ + 0.05);
    tidy_numbers(xs)
}

/// Where a horizontal run might go: between the two ends, level with either
/// of them, beside each node, and clear above and below everything.
fn rows(y1: f32, y2: f32, around: &[Obstacle]) -> Vec<f32> {
    let mut ys = vec![(y1 + y2) / 2.0, y2, y1];
    for b in around {
        ys.push(b.y - CLEAR);
        ys.push(b.y + b.height + CLEAR);
    }
    let top = around.iter().map(|b| b.y).fold(f32::MAX, f32::min);
    let bottom = around
        .iter()
        .map(|b| b.y + b.height)
        .fold(f32::MIN, f32::max);
    if top < f32::MAX {
        ys.push(top - ROOM);
        ys.push(bottom + ROOM);
    }
    // Nothing above the top of the canvas: a run off the edge is a run
    // nobody can see, and the clipping makes it look like a line that stops.
    ys.retain(|y| *y >= CLEAR);
    tidy_numbers(ys)
}

/// The same numbers, near-duplicates dropped, order kept.
///
/// Order kept because it is the order the shapes are preferred in: the middle
/// first, and whatever a node pushed the line to after it.
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
            let flat = (before.1 - p.1).abs() < 0.5 && (after.1 - p.1).abs() < 0.5;
            let upright = (before.0 - p.0).abs() < 0.5 && (after.0 - p.0).abs() < 0.5;
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
fn blocking(around: &[Obstacle], from: (f32, f32), to: (f32, f32)) -> Vec<Box> {
    around
        .iter()
        .map(|b| {
            let joined = holds(b, from) || holds(b, to);
            let by = if joined { 0.0 } else { CLEAR };
            (
                b.x - by,
                b.y - by,
                b.x + b.width + by,
                b.y + b.height + by,
            )
        })
        .collect()
}

/// Whether a point is on or inside a node.
fn holds(b: &Obstacle, (x, y): (f32, f32)) -> bool {
    x >= b.x - 0.5
        && x <= b.x + b.width + 0.5
        && y >= b.y - 0.5
        && y <= b.y + b.height + 0.5
}

/// How many nodes a route runs through.
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
fn crosses(from: (f32, f32), to: (f32, f32), (bx1, by1, bx2, by2): Box) -> bool {
    let (x1, x2) = (from.0.min(to.0), from.0.max(to.0));
    let (y1, y2) = (from.1.min(to.1), from.1.max(to.1));
    x1 < bx2 && bx1 < x2 && y1 < by2 && by1 < y2
}

/// How far a route runs, in whole pixels.
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
                let path = route(x1, y1, x2, y2, around);
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
        let path = route(0.0, 64.0, 240.0, 64.0, &[]);
        assert_eq!(path, "M 0.0 64.0 L 240.0 64.0");
        assert!(!path.contains('Q'), "a level route was given a corner");

        // And within a pixel of level, which is where two nodes of the same
        // height but different sizes land.
        assert!(!route(0.0, 64.0, 240.0, 64.4, &[]).contains('Q'));
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
            (600.0, 40.0, 40.0, 240.0, &[node(40.0, 212.0), node(600.0, 12.0)][..]),
        ] {
            let route_ = way(x1, y1, x2, y2, around);
            for pair in route_.windows(2) {
                let (from, to) = (pair[0], pair[1]);
                assert!(
                    (to.0 - from.0).abs() < 0.05 || (to.1 - from.1).abs() < 0.05,
                    "the segment from {from:?} to {to:?} is a diagonal: {route_:?}"
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
        let path = route(0.0, 0.0, 34.0, 4.0, &[]);
        assert!(points(&path).first().copied() == Some((0.0, 0.0)), "{path}");
        assert!(points(&path).last().copied() == Some((34.0, 4.0)), "{path}");
    }

    /// A link that goes backwards goes around rather than through.
    ///
    /// A target to the left of its source cannot be reached through the middle
    /// without the line doubling back across both nodes.
    #[test]
    fn a_backwards_route_does_not_double_back_through_its_nodes() {
        let path = route(300.0, 40.0, 100.0, 200.0, &[]);
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
        let route_ = way(192.0, 68.0, 560.0, 68.0, &around);
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
        let route_ = way(896.0, 188.0, 24.0, 104.0, &around);
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
        let route_ = way(192.0, 68.0, 600.0, 328.0, &around);

        let kept = (
            passed.x - CLEAR,
            passed.y - CLEAR,
            passed.x + passed.width + CLEAR,
            passed.y + passed.height + CLEAR,
        );
        assert!(
            !route_.windows(2).any(|s| crosses(s[0], s[1], kept)),
            "the line comes within {CLEAR} pixels of a node it passes: {route_:?}"
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
        let path = route(192.0, 28.0, 600.0, 300.0, &wall);
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
        let route_ = way(192.0, 68.0, 600.0, 328.0, &[]);
        assert_eq!(route_.len(), 4, "{route_:?}");
        // Down the middle, so that two links between the same pair of columns
        // do not lie on top of each other.
        let middle = (192.0 + 16.0 + 600.0 - 16.0) / 2.0;
        assert!((route_[1].0 - middle).abs() < 0.5, "{route_:?}");
    }

    /// The arrowhead points along the line and sits at its end.
    #[test]
    fn the_arrowhead_is_at_the_target_and_points_into_it() {
        let head = arrow(240.0, 64.0);
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
        assert_eq!(240.0 - visited[1].0, length, "the head is as long as it says");
        assert_eq!(visited[2].1 - visited[1].1, half * 2.0, "and as wide");
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
        let said = marker(40.0, 100.0, 240.0, 100.0, &[]);
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
        let said = marker(40.0, 40.0, 240.0, 240.0, &[]);
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
        let said = marker(40.0, 240.0, 240.0, 40.0, &[]);
        let visited = points(&said);
        assert!(
            visited[0].1 < visited[1].1 && visited[0].1 < visited[2].1,
            "a link that climbs points up: {said}"
        );

        // Backwards: the target is to the left of the source, so the long run
        // carries the data back the way it came, and the marker points that
        // way. With nothing in the way the long run is the one that arrives
        // at the target, which is the fewest turns the shape can be drawn in.
        let said = marker(400.0, 40.0, 40.0, 200.0, &[]);
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
        let route_ = way(x1, y1, x2, y2, &around);

        let on_the_line = |(x, y): (f32, f32)| {
            route_.windows(2).any(|s| {
                let (lo, hi) = (s[0].0.min(s[1].0) - 0.5, s[0].0.max(s[1].0) + 0.5);
                let (top, bottom) = (s[0].1.min(s[1].1) - 0.5, s[0].1.max(s[1].1) + 0.5);
                (lo..=hi).contains(&x) && (top..=bottom).contains(&y)
            })
        };

        let at = label_at(x1, y1, x2, y2, &around);
        assert!(on_the_line(at), "the label is at {at:?}, off the line");
        // And not halfway between the two nodes, which is the old answer and
        // is inside the node this route goes around.
        assert!(
            !through(&[at, at], &between),
            "the label is inside the node the line avoids"
        );

        let said = marker(x1, y1, x2, y2, &around);
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
        assert_eq!(marker(40.0, 100.0, 60.0, 100.0, &[]), "");

        // Where only some of the runs are short the marker goes on the
        // longest, which is the rule now that the route's shape is chosen
        // rather than known: two nodes nearly level have a vertical run of
        // ten pixels and two horizontal runs of a hundred, and the direction
        // is perfectly legible on one of those.
        let said = marker(40.0, 100.0, 240.0, 110.0, &[]);
        assert!(!said.is_empty(), "a long run should carry a marker");
        let visited = points(&said);
        assert!(
            visited[0].0 > visited[1].0,
            "and it points the way the data goes: {said}"
        );
    }
}

