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
//! The route is computed here rather than in the markup, for two reasons. It
//! is arithmetic, and arithmetic in a layout language is hard to read and
//! impossible to test; and this way the shape of every route is checkable --
//! that it starts where the source is, ends where the target is, and has no
//! diagonal in it.
//!
//! What comes back is SVG path data, which is what Slint's `Path` takes as its
//! `commands`. Absolute canvas coordinates, so the element can span the canvas
//! and need no bounding box of its own.

/// How far a corner is rounded, in pixels.
///
/// Small: a corner rounded much more than this stops reading as a right angle
/// and the route stops looking like a route. It is also reduced where a
/// segment is too short to give it room -- see [`corner`].
const RADIUS: f32 = 8.0;

/// How far a line runs straight out of a node before it turns.
///
/// Without it a link leaving a node would turn immediately at the node's edge,
/// and two links leaving the same node would overlap for their whole first
/// segment. A stub gives each one somewhere to be.
const STUB: f32 = 16.0;

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

/// The path of a link from `(x1, y1)` to `(x2, y2)`.
///
/// Three segments and two corners, in the plane: out of the source, across,
/// and into the target. Where the two ends are at the same height the middle
/// segment has no length and the route is one straight line, which is the
/// common case and wants no corners at all.
pub fn route(x1: f32, y1: f32, x2: f32, y2: f32) -> String {
    // Where the vertical run happens. Halfway between the two stubs, so that
    // two links between the same pair of columns do not lie on top of each
    // other when their nodes are at different heights.
    let out = x1 + STUB;
    let in_ = x2 - STUB;
    let across = (out + in_) / 2.0;

    // Level: one line, no corners. Checked against a pixel rather than
    // against nought, because a node's centre and another's are rarely
    // exactly equal and a corner of half a pixel is a smudge.
    if (y2 - y1).abs() < 1.0 {
        return format!("M {x1:.1} {y1:.1} L {x2:.1} {y2:.1}");
    }

    // A link that goes backwards -- the target is to the left of the source --
    // cannot run through the middle without doubling back through both nodes.
    // It goes around instead: out, across at the far side, and back in.
    if in_ <= out {
        let around = y1 + (y2 - y1) / 2.0;
        return format!(
            "M {x1:.1} {y1:.1} L {out:.1} {y1:.1} {} L {out:.1} {} {} L {in_:.1} {around:.1} \
             {} L {in_:.1} {} {} L {x2:.1} {y2:.1}",
            corner(out, y1, 1.0, sign(y2 - y1), (out - x1).abs(), (around - y1).abs()),
            fmt(around - sign(y2 - y1) * radius((out - x1).abs(), (around - y1).abs())),
            corner_turn(out, around, -1.0, sign(y2 - y1), (out - in_).abs(), (around - y1).abs()),
            corner(in_, around, -1.0, sign(y2 - y1), (out - in_).abs(), (y2 - around).abs()),
            fmt(y2 - sign(y2 - y1) * radius((in_ - x2).abs(), (y2 - around).abs())),
            corner_turn(in_, y2, 1.0, sign(y2 - y1), (x2 - in_).abs(), (y2 - around).abs()),
        );
    }

    let down = sign(y2 - y1);
    let r = radius((across - x1).abs(), (y2 - y1).abs() / 2.0);

    format!(
        "M {x1:.1} {y1:.1} L {} {y1:.1} Q {across:.1} {y1:.1} {across:.1} {} \
         L {across:.1} {} Q {across:.1} {y2:.1} {} {y2:.1} L {x2:.1} {y2:.1}",
        fmt(across - r),
        fmt(y1 + down * r),
        fmt(y2 - down * r),
        fmt(across + r),
    )
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

/// A second arrow, in the middle of the route's longest run.
///
/// The head at the end says which way the data goes, and says it where the
/// line meets a node -- which is the one place a diagram is already crowded,
/// and the one place two links between the same pair of nodes look alike. A
/// marker out in the middle of the run says it where there is room to see it,
/// so a pair of links between one pair of nodes can be told apart at a glance
/// instead of by following each to its end.
///
/// On the vertical run where the link changes height, since that run belongs
/// to this link alone; on the long return run where the link goes backwards;
/// and in the middle of the line where it is level. A run with no room for one
/// gets none -- see [`ROOM`] -- which is a short link, where the head at the
/// end is already next to everything else about it.
pub fn marker(x1: f32, y1: f32, x2: f32, y2: f32) -> String {
    let out = x1 + STUB;
    let in_ = x2 - STUB;
    let across = (out + in_) / 2.0;

    // Level: the line is one horizontal run, and its middle is the middle of
    // the link.
    if (y2 - y1).abs() < 1.0 {
        if (x2 - x1).abs() < ROOM {
            return String::new();
        }
        let way = if x2 > x1 { Way::Right } else { Way::Left };
        return triangle((x1 + x2) / 2.0, y1, way, MARKER);
    }

    // Backwards: the long run is the one that carries it back past the
    // source, at the height between the two.
    if in_ <= out {
        let around = y1 + (y2 - y1) / 2.0;
        if (out - in_).abs() < ROOM {
            return String::new();
        }
        return triangle((out + in_) / 2.0, around, Way::Left, MARKER);
    }

    // Otherwise the vertical run, which is this link's own.
    if (y2 - y1).abs() < ROOM {
        return String::new();
    }
    let way = if y2 > y1 { Way::Down } else { Way::Up };
    triangle(across, (y1 + y2) / 2.0, way, MARKER)
}

/// Which way the route turns: down the canvas is positive.
fn sign(d: f32) -> f32 {
    if d < 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// How far a corner may be rounded, given the room on either side of it.
///
/// Never more than half the shorter segment, or the two corners of one turn
/// would overlap and the line would bulge where it should bend.
fn radius(along: f32, across: f32) -> f32 {
    RADIUS.min(along / 2.0).min(across / 2.0).max(0.0)
}

/// A rounded corner arriving horizontally and leaving vertically.
fn corner(x: f32, y: f32, from: f32, down: f32, along: f32, across: f32) -> String {
    let r = radius(along, across);
    format!(
        "Q {x:.1} {y:.1} {:.1} {:.1}",
        x,
        y + down * r - 0.0 * from
    )
}

/// A rounded corner arriving vertically and leaving horizontally.
fn corner_turn(x: f32, y: f32, to: f32, _down: f32, along: f32, across: f32) -> String {
    let r = radius(along, across);
    format!("Q {x:.1} {y:.1} {:.1} {y:.1}", x + to * r)
}

/// One number, as the path data wants it.
fn fmt(v: f32) -> String {
    format!("{v:.1}")
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

    /// A route starts at its source and ends at its target.
    ///
    /// The one thing that must be true of every route, whatever shape it took
    /// to get there: a line that does not touch the nodes it joins is a line
    /// that says nothing.
    #[test]
    fn every_route_joins_the_two_points_it_was_given() {
        for (x1, y1, x2, y2) in [
            (0.0, 0.0, 200.0, 0.0),
            (0.0, 0.0, 200.0, 120.0),
            (0.0, 120.0, 200.0, 0.0),
            (300.0, 40.0, 100.0, 200.0),
            (10.0, 10.0, 12.0, 400.0),
            (0.0, 0.0, 0.0, 0.0),
        ] {
            let path = route(x1, y1, x2, y2);
            let visited = points(&path);
            assert!(path.starts_with('M'), "{path}");
            assert_eq!(visited.first().copied(), Some((x1, y1)), "{path}");
            assert_eq!(visited.last().copied(), Some((x2, y2)), "{path}");
        }
    }

    /// A route level with itself is one straight line.
    ///
    /// The common case -- two nodes placed in a row -- and a corner in it
    /// would be a bend in a line that does not bend.
    #[test]
    fn a_level_route_is_a_single_line() {
        let path = route(0.0, 64.0, 240.0, 64.0);
        assert_eq!(path, "M 0.0 64.0 L 240.0 64.0");
        assert!(!path.contains('Q'), "a level route was given a corner");

        // And within a pixel of level, which is where two nodes of the same
        // height but different sizes land.
        assert!(!route(0.0, 64.0, 240.0, 64.4).contains('Q'));
    }

    /// A route that turns does so at right angles.
    ///
    /// Checked by walking the straight segments: each one holds either its x
    /// or its y, never both. The corners are quadratics and are allowed to be
    /// neither, which is what makes them corners.
    #[test]
    fn a_turning_route_has_no_diagonal_in_it() {
        let path = route(0.0, 0.0, 240.0, 120.0);
        assert!(path.contains('Q'), "a turning route should have corners");

        // Every straight segment: the L commands, in order, from wherever the
        // path happened to be.
        let mut at = (0.0f32, 0.0f32);
        for token in path.split(" L ").enumerate() {
            let (n, piece) = token;
            let numbers: Vec<f32> = piece
                .split_whitespace()
                .filter_map(|t| t.parse::<f32>().ok())
                .collect();
            if n == 0 {
                at = (numbers[0], numbers[1]);
                continue;
            }
            let to = (numbers[0], numbers[1]);
            assert!(
                (to.0 - at.0).abs() < 0.05 || (to.1 - at.1).abs() < 0.05,
                "the segment from {at:?} to {to:?} is a diagonal: {path}"
            );
            at = numbers
                .chunks(2)
                .last()
                .map(|c| (c[0], c[1]))
                .unwrap_or(to);
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
        let path = route(0.0, 0.0, 34.0, 4.0);
        assert!(points(&path).first().copied() == Some((0.0, 0.0)), "{path}");
        assert!(points(&path).last().copied() == Some((34.0, 4.0)), "{path}");
    }

    /// A link that goes backwards goes around rather than through.
    ///
    /// A target to the left of its source cannot be reached through the middle
    /// without the line doubling back across both nodes.
    #[test]
    fn a_backwards_route_does_not_double_back_through_its_nodes() {
        let path = route(300.0, 40.0, 100.0, 200.0);
        let visited = points(&path);
        assert_eq!(visited.first().copied(), Some((300.0, 40.0)));
        assert_eq!(visited.last().copied(), Some((100.0, 200.0)));

        // It leaves to the right of its source and arrives from the left of
        // its target, which is what the handles on a node mean.
        assert!(visited[1].0 > 300.0, "{path}");
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
        let said = marker(40.0, 100.0, 240.0, 100.0);
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

        // Down the canvas: on the vertical run, pointing down.
        let said = marker(40.0, 40.0, 240.0, 200.0);
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
        let said = marker(40.0, 200.0, 240.0, 40.0);
        let visited = points(&said);
        assert!(
            visited[0].1 < visited[1].1 && visited[0].1 < visited[2].1,
            "a link that climbs points up: {said}"
        );

        // Backwards: the target is to the left of the source, so the long run
        // carries the data back the way it came.
        let said = marker(400.0, 40.0, 40.0, 200.0);
        let visited = points(&said);
        assert!(
            visited[0].0 < visited[1].0 && visited[0].0 < visited[2].0,
            "a link that goes backwards points left: {said}"
        );
        assert!(
            (visited[0].1 - 120.0).abs() < MARKER.0,
            "at the height between the two: {said}"
        );
    }

    /// A run with no room for a marker gets none.
    ///
    /// A triangle on a run barely longer than itself touches both corners and
    /// reads as a thickening of the line, which says nothing about direction
    /// and makes the line look like something else.
    #[test]
    fn a_short_run_has_no_marker() {
        // Two nodes almost touching, level.
        assert_eq!(marker(40.0, 100.0, 60.0, 100.0), "");
        // And two at nearly one height, where the vertical run is a step.
        assert_eq!(marker(40.0, 100.0, 240.0, 110.0), "");
    }
}
