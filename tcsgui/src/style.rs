//! What each node type looks like, and the rules the looks obey.
//!
//! Two rules, and both are checked by the tests at the bottom rather than
//! trusted to whoever edits the table next.
//!
//! **Nothing is told apart by colour alone.** Every node type differs from
//! every other in at least two channels that are not colour -- its outline
//! shape, how wide its border is, how its inside is shaded, how large it is,
//! and how its label is set. A reader who sees no colour at all still sees
//! twelve distinguishable nodes, and a reader who sees colour gets the same
//! information twice. Two channels rather than one on purpose: a single
//! distinguishing channel is one print, one projector or one pair of eyes away
//! from being no channel.
//!
//! Type is one of those channels. A label in a monospaced face, a heavier
//! weight or an italic is a difference that survives a monochrome print and a
//! reader who cannot pick the shape out at a small size, and it is read
//! without being decoded -- which is more than can be said for a legend.
//!
//! **Red means something is wrong.** No node type, no selection, no hover and
//! no link is red. It is reserved so that red anywhere on the canvas means an
//! error and can be looked for, which is worth more than a tenth palette
//! entry.

/// The outline a node is drawn with.
///
/// Three, one to a pane: a payload is a parallelogram, a part of the vehicle
/// a rectangle with rounded corners, and the ground the plain rectangle. So
/// the outline alone says which of the three a node belongs to, and what
/// tells two of one family apart is the line around it, the line's weight and
/// the way the name is set. Every one is drawn as a path, from
/// [`polygon_of`], so every one can take any of the edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outline {
    /// Square corners. Something outside the system: the ground station.
    ///
    /// The external entity of a data-flow diagram, which is what the ground
    /// is: the far end of the link, not part of the vehicle.
    Square,
    /// A parallelogram. Data arriving or leaving: a payload, every one.
    ///
    /// The flowchart symbol for input and output on an unspecified medium,
    /// which is exactly what a payload is -- a bus, a socket, a channel, a
    /// sample buffer, a file, lines driven, a program -- and the one symbol in
    /// the notation that means *data crossing a boundary* rather than a thing
    /// or a step.
    ///
    /// It leans right, as the symbol is drawn, so its missing corners are the
    /// top left and the bottom right. A terminal in a corner the shape has
    /// given away would float outside its node, so the terminals of a slanted
    /// shape move in with the slant -- see [`room_at`] -- and so does the row
    /// its name is padded away from.
    Parallelogram,
    /// Rounded corners. A part of the vehicle, every one: a computation, a
    /// log, a null node, a tee, a page of its own, an app aboard.
    ///
    /// The process of a data-flow diagram, which is what the vehicle's own
    /// graph is made of. Drawn as a path, with each corner an arc of
    /// [`RADIUS`] in [`ARC_STEPS`] straight pieces, rather than as a corner
    /// radius on a rectangle: a radius cannot be dashed or doubled, and seven
    /// vehicle types of one outline need the edges to be told apart by.
    Rounded,
}

/// How round a rounded outline's corners are, in pixels.
///
/// Tight enough that a terminal three pixels in from the border -- see
/// [`TERMINAL_INSET`] -- is inside the curve, so a rounded node's terminals sit
/// where every other node's do and nothing has to move in for the corner:
/// at nine, the corner of a terminal's box is eight and a half pixels from the
/// arc's centre, inside even the straight pieces the arc is drawn in.
pub const RADIUS: f32 = 9.0;

/// How many straight pieces each rounded corner is drawn in: enough that the
/// pieces read as a curve at any size the canvas is drawn at.
pub const ARC_STEPS: usize = 8;

/// How a node's outline is drawn: one line, two, or a broken one.
///
/// The channel that used to be the inside of the node. A body was filled flat,
/// hatched, left empty or given a second outline, and three of those were
/// about the inside; a pattern inside a node is now not drawn at all -- a body
/// is its own colour and nothing else -- so what told two nodes apart has
/// moved to the line around the outside, where it does not compete with the
/// name or with the colour the body carries.
///
/// A line rather than a colour, so it survives a monochrome rendering, and
/// four values from two independent facts: a line may be doubled, broken, both
/// or neither.
///
/// A broken line is only available to a straight-sided outline. Dashes are
/// walked along the polygon's own sides -- see [`polygon_of`] -- and a rounded
/// outline has no polygon, so the two types drawn with a radius are held to a
/// single line by the test below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    /// One line.
    Single,
    /// Two lines, the inner one inset.
    Double,
    /// One broken line.
    Dashed,
    /// Two broken lines, which is the two facts at once.
    DashedDouble,
    /// One line of dots: broken, and broken finely enough that it does not
    /// read as the dashed one. A fifth edge because one height for every node
    /// took away the channel that told nine payload types apart, and four
    /// edges by two weights is eight -- see `HEIGHT`.
    Dotted,
    /// Two lines of dots: the dotted line doubled, as the dashed one is. A
    /// sixth edge because every payload is a parallelogram, and twelve
    /// payload types need twelve lines between six edges and two weights.
    DottedDouble,
}

impl Edge {
    /// Whether a second line is drawn inside the first.
    pub fn doubled(self) -> bool {
        matches!(self, Edge::Double | Edge::DashedDouble | Edge::DottedDouble)
    }

    /// Whether the line is broken.
    pub fn broken(self) -> bool {
        matches!(
            self,
            Edge::Dashed | Edge::DashedDouble | Edge::Dotted | Edge::DottedDouble
        )
    }

    /// The pattern a broken line is drawn in: how long a piece of it is, and
    /// the gap after it.
    fn pattern(self) -> (f32, f32) {
        match self {
            Edge::Dotted | Edge::DottedDouble => (DOT, DOT_GAP),
            _ => (DASH, GAP),
        }
    }
}

/// How wide the outline is drawn, in pixels.
///
/// A third non-colour channel, and the one that reads at a glance across a
/// whole canvas: the nodes a control loop runs through are drawn heavily.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Narrow,
    Wide,
}

/// How a node's label is set.
///
/// The fifth channel, and the one that works at a size where a shape has
/// stopped being legible. `Mono` is for the types whose attributes are
/// addresses and register numbers -- a bus, a chip select -- because the file
/// shows those in a monospaced column and the canvas should agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    /// The system's ordinary face.
    Plain,
    /// The system's monospaced face.
    Mono,
}

/// What a node type looks like.
///
/// `tint` is never the whole of the distinction -- see the rule at the top of
/// this file -- and there is one per type. It is named rather than given as a
/// value so the window can hold the colours and this table can hold which
/// type has which.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Appearance {
    pub outline: Outline,
    pub edge: Edge,
    pub weight: Weight,
    /// The nominal width and height, in pixels, before a label widens it.
    ///
    /// Width is [`WIDTH`] for every type, so what varies here is the height:
    /// size is still the fourth channel -- a MOC link is visibly taller than a
    /// sensor -- and it is one channel whichever of the two carries it.
    pub size: (u32, u32),
    /// How the label is set: the face, its size in pixels, its weight, and
    /// whether it is italic. The fifth channel.
    pub face: Face,
    pub label_size: u32,
    pub label_weight: u32,
    pub label_italic: bool,
}

/// The ground each working pane is painted, by the pane.
///
/// Light and neutral, as asked: the three are 14 CIE units apart under every
/// one of the four ways of seeing, and none has a chroma above 14, so a pane
/// reads as a tinted sheet of paper rather than as a coloured panel.
///
/// The nodes drawn on one are a ramp of darker colours in its own family's
/// hue, starting below the line that carries its data and darkening down the
/// panel -- `tcsgui::tint_of` is the rule and `tcsgui::ground_of` the grounds
/// themselves. A node's colour then says both which pane it belongs in and
/// where in that pane's list it sits.
///
/// Here as a meaning and painted in `main.rs`: this file says which pane has
/// which ground, and the window says what that ground is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ground {
    Moc,
    Vehicle,
    Payloads,
}

/// Which ground a pane is painted.
///
/// For the tests, which ask it of every pane; the window paints each pane's
/// ground directly.
#[cfg(test)]
pub fn ground_of(pane: crate::pane::Pane) -> Ground {
    match pane {
        crate::pane::Pane::Moc => Ground::Moc,
        crate::pane::Pane::Vehicle => Ground::Vehicle,
        crate::pane::Pane::Payloads => Ground::Payloads,
    }
}

/// How wide every node is drawn, in pixels.
///
/// One width for all of them, and the reason is alignment: a column of nodes
/// of four different widths has no edge to line up on, so a diagram built from
/// them looks approximate however carefully each node was placed. Height is
/// still per type -- it is one of the channels that tells two types apart --
/// and height alone is enough, as the accessibility test below proves by
/// still passing.
///
/// Wide enough for the longest type name at the largest label size, which is
/// what sets the floor: a label that elides is a label that has stopped being
/// a channel.
pub const WIDTH: u32 = 152;

/// How tall every node is drawn, in pixels.
///
/// One height for all of them, for the reason there is one width: the
/// terminals are drawn a fixed inset from the top and bottom edges, so one
/// height is what puts every node's two rows of terminals the same distance
/// apart. Two nodes level on the grid then have their terminals level too, and
/// a line between them runs straight across rather than stepping by the
/// difference between two heights.
///
/// It was a channel that told two types apart. The outline, the edge, the
/// weight and the colour still do -- see the accessibility tests below -- and
/// a diagram whose lines run straight is worth the channel.
///
/// Room for the three lines of a label -- the name, the type and a summary --
/// between the two rows of terminals.
pub const HEIGHT: u32 = 64;

/// The one red in the program, and the only thing it is used for.
///
/// A node whose configuration is refused, a link with an unbound input, a
/// compute process that will not start. Nothing else.
pub const ERROR_TINT: &str = "#c0392b";

/// A type's terminals, as the canvas needs them.
///
/// The same two lists [`tcslibgs::graph::terminals::Terminals`] holds, and for
/// most types literally those: this is a borrowed copy without the merge rule,
/// which is a checking question rather than a drawing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Terminals {
    pub sinks: &'static [&'static str],
    pub sources: &'static [&'static str],
}

impl Terminals {
    /// Whether this type has the named terminal at either end.
    pub fn has(&self, terminal: &str) -> bool {
        self.sinks.contains(&terminal) || self.sources.contains(&terminal)
    }

    /// Every terminal of this type, sinks and then sources.
    ///
    /// For the tests, which ask questions about all of them at once. The
    /// window asks about each by name, because the markup draws each in its
    /// own corner.
    #[cfg(test)]
    pub fn each(&self) -> impl Iterator<Item = &&'static str> {
        self.sinks.iter().chain(self.sources.iter())
    }
}

/// Where a terminal is drawn, and which way its arrow points.
///
/// Two rows and two columns, and the arrangement is the two streams: the
/// groundward one along the top, left to right, arriving from the payload at
/// `payload-in` and leaving for the ground at `payload-out`; the vehicleward
/// one along the bottom, right to left, arriving from the ground at `moc-in`
/// and leaving for the payload at `moc-out`.
///
/// Which means the arrows all point the way the data actually travels across
/// the canvas, because the canvas is laid out the same way: the payloads are
/// the leftmost pane and the ground the rightmost. A reader who has noticed that
/// once does not have to read the names again.
///
/// `None` for a name that is not one of the four, which is a computation's
/// own input: those are drawn where the link arrives and not as a terminal.
pub fn corner_of(terminal: &str) -> Option<Corner> {
    match terminal {
        // The payload's own data along the top, travelling rightwards: it
        // arrives from a payload on the left and leaves for the ground on the
        // right.
        "payload-in" => Some(Corner {
            top: true,
            left: true,
            rightward: true,
        }),
        "payload-out" => Some(Corner {
            top: true,
            left: false,
            rightward: true,
        }),
        // And what the ground sends along the bottom, travelling leftwards:
        // it arrives from the ground on the right and leaves for a payload on
        // the left.
        "moc-out" => Some(Corner {
            top: false,
            left: true,
            rightward: false,
        }),
        "moc-in" => Some(Corner {
            top: false,
            left: false,
            rightward: false,
        }),
        _ => None,
    }
}

/// Which corner of a node a terminal is drawn in, and which way its arrow
/// points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Corner {
    pub top: bool,
    pub left: bool,
    /// Whether the arrow points right, which is to say towards the ground.
    ///
    /// The way the data on this terminal travels across the canvas, which is
    /// the way the canvas is laid out: the payloads are the leftmost working
    /// pane and the ground the rightmost. So the payload's own data --
    /// telemetry, on its way to the ground -- points right, and what the ground
    /// sends points left.
    ///
    /// All four carry one. A terminal's name says whose data it is and whether
    /// it arrives or leaves -- `payload-in`, `moc-out` -- and the arrow says
    /// which way that is on the screen, which is the half a reader can take in
    /// without reading.
    pub rightward: bool,
}

/// The corners of a node's outline, where the outline is straight-sided.
///
/// One list of points, and everything the markup draws from it -- the shape it
/// fills, the line it strokes, the second line of a doubled edge, the dashes
/// of a broken one -- is worked out here from these. The alternative was four
/// strings each formatting its own arithmetic, which is four places to get a
/// corner wrong.
///
/// A rounded outline's corners are arcs, each drawn as [`ARC_STEPS`] straight
/// pieces, so it is a polygon like the others and takes a dash or a second
/// line the same way.
///
/// Wound clockwise, starting at the top left, and inset half a pixel so the
/// stroke sits inside the node: a path drawn on the boundary is drawn half
/// outside it, and a node that overhangs its own box by a pixel is a node
/// whose terminals and lines are all a pixel out.
pub fn polygon_of(outline: Outline, (width, height): (u32, u32)) -> Vec<(f32, f32)> {
    let (w, h) = (width as f32 - 0.5, height as f32 - 0.5);
    let (left, top) = (0.5, 0.5);
    match outline {
        Outline::Square => vec![(left, top), (w, top), (w, h), (left, h)],
        // Each corner an arc about a centre RADIUS in from both of its sides,
        // clockwise from the top left as the others are wound.
        Outline::Rounded => {
            let r = RADIUS.min((w - left) / 2.0).min((h - top) / 2.0);
            let corners = [
                ((left + r, top + r), std::f32::consts::PI),
                ((w - r, top + r), 1.5 * std::f32::consts::PI),
                ((w - r, h - r), 0.0),
                ((left + r, h - r), 0.5 * std::f32::consts::PI),
            ];
            let mut points = Vec::with_capacity(4 * (ARC_STEPS + 1));
            for ((cx, cy), from) in corners {
                for step in 0..=ARC_STEPS {
                    let angle = from + std::f32::consts::FRAC_PI_2 * step as f32 / ARC_STEPS as f32;
                    points.push((cx + r * angle.cos(), cy + r * angle.sin()));
                }
            }
            points
        }
        Outline::Parallelogram => {
            let slant = (h - top) / 2.0;
            vec![(left + slant, top), (w, top), (w - slant, h), (left, h)]
        }
    }
}

/// The shape to fill, as path data in the node's own coordinates.
///
/// Closed, whatever the line does, and empty for a rounded outline -- which
/// the markup fills with a Rectangle's own background. One pixel to one
/// coordinate, so the markup draws it one to one and a stroke keeps its width:
/// a viewbox stretched to fit would stretch the line with it.
pub fn shape_of(outline: Outline, size: (u32, u32)) -> String {
    let points = polygon_of(outline, size);
    if points.is_empty() {
        return String::new();
    }
    let mut out = run_of(&points);
    out.push_str(" Z");
    out
}

/// The line to stroke: the outline itself, broken into dashes where the edge
/// is a broken one.
///
pub fn edge_of(outline: Outline, edge: Edge, size: (u32, u32)) -> String {
    line_of(&polygon_of(outline, size), edge)
}

/// The second line of a doubled edge, inset four pixels from the first.
///
/// Empty unless the edge is a doubled one. The inset is taken as a smaller box
/// rather than by moving the points, so that each shape insets the way it is
/// shaped: a parallelogram's inner line leans by its own slant, and a rounded
/// one's corners are rounded about their own centres.
pub fn inner_of(outline: Outline, edge: Edge, (width, height): (u32, u32)) -> String {
    if !edge.doubled() || width <= 2 * DOUBLE || height <= 2 * DOUBLE {
        return String::new();
    }
    let inset = DOUBLE as f32;
    let points: Vec<(f32, f32)> = polygon_of(outline, (width - 2 * DOUBLE, height - 2 * DOUBLE))
        .into_iter()
        .map(|(x, y)| (x + inset, y + inset))
        .collect();
    line_of(&points, edge)
}

/// How far the second line of a doubled edge is from the first, in pixels.
///
/// Four: at three the two lines read as one thick one at a glance, and at six
/// the inner line starts to crowd a name on the shortest node.
const DOUBLE: u32 = 4;

/// How long a dash is, and the gap after it.
///
/// Eight and five, which at the sizes here gives a node's shortest side three
/// or four dashes: fewer reads as a damaged line and more as a dotted one.
const DASH: f32 = 8.0;
const GAP: f32 = 5.0;

/// And a dotted line's: short enough to be a dot and not a dash, and the
/// gaps no wider than the dashed line's so it still reads as one outline.
const DOT: f32 = 2.0;
const DOT_GAP: f32 = 4.0;

/// One polygon as a line, dashed or whole.
fn line_of(points: &[(f32, f32)], edge: Edge) -> String {
    if points.is_empty() {
        return String::new();
    }
    if !edge.broken() {
        let mut out = run_of(points);
        out.push_str(" Z");
        return out;
    }
    dashes_of(points, true, edge.pattern())
}

/// A polygon's points as one move and a line to each of the rest.
fn run_of(points: &[(f32, f32)]) -> String {
    let mut out = format!("M {:.1} {:.1}", points[0].0, points[0].1);
    for (x, y) in &points[1..] {
        out.push_str(&format!(" L {x:.1} {y:.1}"));
    }
    out
}

/// A polygon as a broken line: one subpath per dash.
///
/// Walked as one line rather than side by side, so the pattern carries on
/// around a corner instead of restarting at each: a dash that began again at
/// every corner would put a dash on all four of them and read as a shape with
/// marked corners.
///
/// Subpaths because that is the only dash a path has here. Slint's Path takes
/// no dash pattern, so a broken line is drawn as the pieces that are there --
/// which is what `route.rs` does for the line being dragged, and for the same
/// reason.
fn dashes_of(points: &[(f32, f32)], closes: bool, (dash, gap): (f32, f32)) -> String {
    let mut sides: Vec<((f32, f32), (f32, f32))> =
        points.windows(2).map(|pair| (pair[0], pair[1])).collect();
    if closes {
        sides.push((points[points.len() - 1], points[0]));
    }

    let mut out = String::new();
    // Where along the dash-and-gap pattern the next side starts, so the
    // pattern runs on round the shape.
    let mut into = 0.0_f32;
    for (from, to) in sides {
        let (run_x, run_y) = (to.0 - from.0, to.1 - from.1);
        let span = (run_x * run_x + run_y * run_y).sqrt();
        if span <= f32::EPSILON {
            continue;
        }
        let at = |along: f32| (from.0 + run_x * along / span, from.1 + run_y * along / span);

        let mut along = -into;
        while along < span {
            let start = along.max(0.0);
            let end = (along + dash).min(span);
            if end > start {
                let (sx, sy) = at(start);
                let (ex, ey) = at(end);
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(&format!("M {sx:.1} {sy:.1} L {ex:.1} {ey:.1}"));
            }
            along += dash + gap;
        }
        // Whatever of the pattern this side ended partway through.
        into = (into + span) % (dash + gap);
    }
    out
}

/// How far in from a side a shape may be, anywhere on it.
///
/// What the column a name is padded away from has to allow for, over and
/// above the terminals themselves: a leaning shape moves its terminals in, so
/// a name padded by the terminal column alone would be padded into them.
/// Nothing for the shapes that reach their own sides all the way down, which
/// is every outline but the slanted one.
///
/// The second line of a doubled edge needs none of this: it is inset as a
/// smaller box and leans by its own slant. See [`inner_of`].
pub fn room_of(outline: Outline, (_width, height): (u32, u32)) -> i32 {
    match outline {
        Outline::Parallelogram => (height as f32 / 2.0).ceil() as i32,
        _ => 0,
    }
}

/// How far in from its side the shape is, at the row this terminal sits in.
///
/// A terminal is drawn in a corner, and a slanted shape has given two of its
/// corners away, so the terminals of one move in by however much the shape is
/// missing at their own row -- and the line that arrives at a terminal moves
/// with it, or it would end in the notch beside the node.
///
/// Measured at the edge of the terminal's box nearest the top or the bottom,
/// which is where a leaning side is furthest in across the box's own height:
/// the box has to clear the shape along all of it, not at its middle.
pub fn room_at(outline: Outline, (width, height): (u32, u32), corner: Corner) -> i32 {
    match outline {
        Outline::Parallelogram => {
            let (h, slant) = (height as f32 - 1.0, (height as f32 - 1.0) / 2.0);
            let inset = TERMINAL_INSET as f32;
            let tall = TERMINAL.1 as f32;
            // The left side leans in as it goes down and the right side leans
            // in as it goes up, so each is furthest in at a different edge of
            // the box: the top of it on the left, the bottom of it on the
            // right. Taking the row's own middle would leave half the box
            // hanging out of the shape.
            let room = if corner.left {
                let y = if corner.top { inset } else { h - inset - tall };
                slant * (1.0 - y / h)
            } else {
                let y = if corner.top { inset + tall } else { h - inset };
                slant * (y / h)
            };
            room.max(0.0).ceil() as i32
        }
        _ => {
            let _ = (width, corner);
            0
        }
    }
}

/// How large a terminal is drawn, in pixels.
///
/// It was 24 by 10 -- wide enough for an arrow and no wider -- and that is a
/// strip a person has to aim at, which is the complaint this answers: a press
/// has to land on a terminal exactly, since the terminal is what says which
/// stream the line carries, where the release only has to land on a node.
/// Twice the area, and both ways about, so the hand has room in whichever
/// direction it is short.
///
/// What bounds it is the smallest node, which is every one at 40 tall, and
/// the type with all four, `app_all_veh`: two rows of terminals and their insets have to fit
/// inside that height and leave the rows apart, and the two columns have to
/// fit inside [`WIDTH`] -- see
/// `tests::the_terminals_of_every_type_fit_inside_it`, which is what says so
/// rather than this comment. The name is clear of them either way: a label is
/// padded away from whichever side carries terminals, so the boxes and the
/// name cannot collide however tall they grow.
pub const TERMINAL: (u32, u32) = (36, 14);

/// How far a terminal sits from the corner it is in.
///
/// Inside the node and inside its border, so that the box a person presses and
/// the box a link arrives at are the same box.
pub const TERMINAL_INSET: u32 = 3;

/// Where a terminal is drawn on a node of this size at `at`, relative to the
/// node.
///
/// Here rather than in the markup for the reason the link routes are here:
/// geometry is arithmetic, and arithmetic in a layout language cannot be
/// tested. The test below is what says the four of them land in four
/// different corners and clear each other.
///
/// On the grid: the box's upper left corner is on one of the lines five
/// millimetres apart -- see `pane::NODE_STEP_MM` -- so the terminals of nodes
/// line up with each other and with those lines, and a line between two level
/// terminals runs along one. Which is why the node's place is asked: a
/// millimetre is not a whole number of pixels, so the same corner of two nodes
/// is not the same number of pixels in from each. The first line in from the
/// border at the top and the left, the last at the bottom and the right, so a
/// terminal is never pushed out of its node or across its slant and keeps at
/// least its inset on every side. A node where a node may be -- a step of five
/// millimetres -- has its two rows ten millimetres apart.
pub fn terminal_at(
    outline: Outline,
    corner: Corner,
    (width, height): (u32, u32),
    at: (i32, i32),
) -> (i32, i32) {
    use crate::pane::{line_ceil, line_floor};

    let inset = TERMINAL_INSET as i32;
    let far = |whole: u32, box_: u32| whole as i32 - box_ as i32 - inset;
    // In by whatever the outline is missing at this row: see `room_at`.
    let room = room_at(outline, (width, height), corner);
    let x = if corner.left {
        line_ceil(at.0 + inset + room)
    } else {
        line_floor(at.0 + far(width, TERMINAL.0) - room)
    };
    let y = if corner.top {
        line_ceil(at.1 + inset)
    } else {
        line_floor(at.1 + far(height, TERMINAL.1))
    };
    (x - at.0, y - at.1)
}

/// Which terminal a link end means on a node of this type, for the drawing.
///
/// `named` is what the link wrote, where it wrote one, and `sink` says which
/// end of the link this is. Three answers in order of authority: the terminal
/// the link named, where the type has it; the only terminal of that kind,
/// where there is one; and otherwise the groundward one -- `payload-in`
/// where data arrives and `payload-out` where it leaves -- because telemetry on its
/// way to the ground is what a node does unless the file says otherwise.
///
/// `None` only for a type this build does not know, which is drawn as a node
/// with no terminals at all.
///
/// This is the drawing's question and not the checking's. A link into a
/// computation that named `in0` resolves here to the sink the line should be
/// drawn to; what `in0` *means* is the computation's business, and
/// [`tcslibgs::graph::terminals::terminal_at`] is what the checking asks.
pub fn terminal_for(node_type: &str, named: Option<&str>, sink: bool) -> Option<&'static str> {
    let terminals = NodeType::ALL
        .iter()
        .find(|t| t.spelling() == node_type)?
        .terminals();
    let ours = if sink {
        terminals.sinks
    } else {
        terminals.sources
    };

    if let Some(named) = named {
        if let Some(found) = ours.iter().find(|terminal| **terminal == named) {
            return Some(found);
        }
    }
    match ours {
        [only] => Some(only),
        _ => {
            let groundward = if sink { "payload-in" } else { "payload-out" };
            ours.iter()
                .find(|terminal| **terminal == groundward)
                .copied()
        }
    }
}

/// Where a line joins a node at this terminal, and which way it points there.
///
/// The outer edge of the terminal's own box, halfway down it: a line that
/// stopped at the node's edge beside the box would read as a line that misses
/// it. The direction is the box's own arrow -- a terminal on the left side
/// faces left -- which is what makes the line, the arrowhead and the arrow
/// drawn inside the terminal all say the same thing.
///
/// `at` is where the node is. `None` for a type this build does not know, or a
/// name that is not one of the four.
pub fn attachment(node_type: &str, at: (i32, i32), terminal: &str) -> Option<((f32, f32), bool)> {
    let found = NodeType::ALL.iter().find(|t| t.spelling() == node_type)?;
    let corner = corner_of(terminal)?;
    let drawn = found.appearance();
    let (width, height) = drawn.size;
    let (_, y) = terminal_at(drawn.outline, corner, (width, height), at);

    // In from the side by whatever the outline is missing at this row, so a
    // line meets a slanted node where the node is rather than where its box
    // is: the terminal moved in by the same amount, and a line that stopped
    // at the box would stop in the notch beside the shape.
    let room = room_at(drawn.outline, (width, height), corner) as f32;
    let edge = if corner.left {
        at.0 as f32 + room
    } else {
        (at.0 + width as i32) as f32 - room
    };
    Some((
        (edge, at.1 as f32 + y as f32 + TERMINAL.1 as f32 / 2.0),
        corner.rightward,
    ))
}

/// The box a terminal is drawn in, relative to the node.
///
/// For the tests, which let a line go of over a terminal because that is where
/// a person naturally aims. The gesture itself does not ask: a line is drawn
/// from the terminal that was pressed, and where it lands is a question about
/// the node under the pointer and not about these boxes -- see `landed` in
/// `main.rs`, which is what made aiming at them unnecessary.
///
/// `None` for a type this build does not know or a name that is not one of the
/// four.
#[cfg(test)]
pub fn terminal_box(
    node_type: &str,
    at: (i32, i32),
    terminal: &str,
) -> Option<(f32, f32, f32, f32)> {
    let found = NodeType::ALL.iter().find(|t| t.spelling() == node_type)?;
    let corner = corner_of(terminal)?;
    let drawn = found.appearance();
    let (x, y) = terminal_at(drawn.outline, corner, drawn.size, at);
    Some((
        (at.0 + x) as f32,
        (at.1 + y) as f32,
        TERMINAL.0 as f32,
        TERMINAL.1 as f32,
    ))
}

/// How much of a node's width a terminal takes up, with room to spare.
///
/// What the label is padded by on whichever side carries terminals: the name
/// stays in the middle of the node and stops short of the corner boxes, rather
/// than a row being reserved at the top and the bottom -- which would leave
/// a few pixels for a name on the shortest node.
pub const TERMINAL_COLUMN: u32 = TERMINAL.0 + 2 * TERMINAL_INSET;

/// Every node type the editor can place.
///
/// The order is the order the palette shows them in: the devices a vehicle has,
/// then what happens to their data, then where it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    I2c,
    Spi,
    Serial,
    /// A synchronous serial line, which Linux drives as a network interface.
    Hdlc,
    Can,
    Iio,
    Rpmsg,
    Network,
    DeviceFile,
    Gpio,
    Compute,
    /// A vehicle node that takes what the ground sends and drops it.
    NullMoc,
    /// A vehicle node that takes a payload's data and drops it.
    NullPayload,
    /// A vehicle node that takes what the ground sends and sends it on both
    /// ways: as MOC data, and again as payload data.
    TeeMoc,
    /// A vehicle node that takes a payload's data and sends it on both ways:
    /// as payload data, and again as MOC data.
    TeePayload,
    /// A vehicle node that answers each ping from the ground with the
    /// vehicle's time. See `tcslibgs::node::vehicle::ping`.
    Ping,
    Log,
    /// A program of the vehicle's own, taking part in the graph over the file
    /// interfaces `tcsapp` describes. Three of them, which differ in which of
    /// the two data interfaces the program is run with: see
    /// `tcslibgs::node::app`.
    AppPayloadVeh,
    AppMocVeh,
    AppAllVeh,
    /// And a payload that is a program rather than a device: one that is asked
    /// for a reading and answers, and one that is never asked and reports of
    /// its own accord. The same file interfaces, on the other side of the
    /// graph, so these two are placed on the payload pane.
    AppPl,
    AppAllPl,
    Moc,
    /// A diagram of its own, kept in its own file and shown here as one box.
    Subdiagram,
}

impl NodeType {
    /// Every type, for the palette and for the tests.
    pub const ALL: [NodeType; 24] = [
        NodeType::I2c,
        NodeType::Spi,
        NodeType::Serial,
        NodeType::Hdlc,
        NodeType::Can,
        NodeType::Iio,
        NodeType::Rpmsg,
        NodeType::Network,
        NodeType::DeviceFile,
        NodeType::Gpio,
        NodeType::Compute,
        NodeType::TeeMoc,
        NodeType::TeePayload,
        NodeType::Ping,
        NodeType::Log,
        NodeType::NullMoc,
        NodeType::NullPayload,
        NodeType::AppPayloadVeh,
        NodeType::AppMocVeh,
        NodeType::AppAllVeh,
        NodeType::AppPl,
        NodeType::AppAllPl,
        NodeType::Moc,
        NodeType::Subdiagram,
    ];

    /// The word the configuration file uses, which is the word the editor
    /// shows: an operator reading the file and an operator reading the canvas
    /// should not have to translate.
    pub fn spelling(self) -> &'static str {
        match self {
            NodeType::I2c => "i2c",
            NodeType::Spi => "spi",
            NodeType::Serial => "serial",
            NodeType::Hdlc => "hdlc",
            NodeType::Can => "can",
            NodeType::Iio => "iio",
            NodeType::Rpmsg => "rpmsg",
            NodeType::Network => "network",
            NodeType::DeviceFile => "device-file",
            NodeType::Gpio => "gpio",
            NodeType::Compute => "compute",
            NodeType::NullMoc => "null-moc",
            NodeType::NullPayload => "null-payload",
            NodeType::TeeMoc => "tee-moc",
            NodeType::TeePayload => "tee-payload",
            NodeType::Ping => "ping",
            NodeType::Log => "log",
            NodeType::AppPayloadVeh => "app_payload_veh",
            NodeType::AppMocVeh => "app_moc_veh",
            NodeType::AppAllVeh => "app_all_veh",
            NodeType::AppPl => "app_pl",
            NodeType::AppAllPl => "app_all_pl",
            NodeType::Moc => "moc",
            NodeType::Subdiagram => "subdiagram",
        }
    }

    /// What a node of this type says about itself.
    ///
    /// The attributes the configuration gives each type, in the order the
    /// manual lists them, with the ones that must be stated marked. Two things
    /// come of having them written down rather than inferred from what a node
    /// happens to hold:
    ///
    /// the editor can show a node's fields before any of them are filled in,
    /// so a person placing an I2C device is told it needs a bus and an address
    /// rather than having to know; and a required attribute left empty is a
    /// fault the editor can mark, which is the same check `tcsverify` makes on
    /// the ground and tcspecial makes again aboard.
    ///
    /// Not exhaustive for `compute`, whose parameters depend on which function
    /// it runs: those are whatever the function wants, so the editor takes
    /// what it is given there and the validation belongs to the function.
    pub fn attributes(self) -> &'static [(&'static str, bool)] {
        match self {
            NodeType::I2c => &[
                ("path", true),
                ("address", true),
                ("group", false),
                ("trigger", false),
                ("trigger_interval", false),
                ("read_delay", false),
                ("poll_interval", false),
                ("read_size", false),
                ("ten_bit", false),
                ("pec", false),
                ("retries", false),
                ("timeout", false),
            ],
            NodeType::Spi => &[
                ("path", true),
                ("max_speed", true),
                ("spi_mode", true),
                ("group", false),
                ("trigger", false),
                ("trigger_interval", false),
                ("read_delay", false),
                ("poll_interval", false),
                ("read_size", false),
                ("bits_per_word", false),
                ("bit_order", false),
                ("cs_active", false),
            ],
            NodeType::Serial => &[
                ("path", true),
                ("datarate", true),
                ("byte_length", true),
                ("stop_bits", true),
                ("parity", false),
                ("group", false),
                ("trigger", false),
                ("trigger_interval", false),
                ("read_delay", false),
                ("poll_interval", false),
            ],
            // An interface, not a path: see tcslibgs::node::payload::hdlc. The
            // standard and the clock are required because a line carries no
            // negotiation; the rate is required only where this end
            // generates the clock, which the palette cannot say, so it is
            // shown as optional and the checking has the rule.
            NodeType::Hdlc => &[
                ("interface", true),
                ("standard", true),
                ("clock_type", true),
                ("clock_rate", false),
                ("encoding", false),
                ("frame_check", false),
                ("loopback", false),
                ("group", false),
                ("trigger", false),
                ("trigger_interval", false),
                ("read_delay", false),
                ("poll_interval", false),
            ],
            NodeType::Can => &[
                ("interface", true),
                ("can_id", true),
                ("can_fd", false),
                ("loopback", false),
                ("group", false),
                ("trigger", false),
                ("trigger_interval", false),
                ("read_delay", false),
                ("poll_interval", false),
            ],
            NodeType::Iio => &[
                ("path", true),
                ("group", false),
                ("trigger", false),
                ("trigger_interval", false),
                ("read_delay", false),
                ("poll_interval", false),
            ],
            NodeType::Rpmsg => &[
                ("path", true),
                ("service", true),
                ("group", false),
                ("trigger", false),
                ("trigger_interval", false),
                ("read_delay", false),
                ("poll_interval", false),
            ],
            NodeType::Network => &[
                ("protocol", true),
                ("address", true),
                ("port", true),
                ("group", false),
                ("trigger", false),
                ("trigger_interval", false),
                ("read_delay", false),
                ("poll_interval", false),
            ],
            NodeType::DeviceFile => &[
                ("path", true),
                ("group", false),
                ("trigger", false),
                ("trigger_interval", false),
                ("read_delay", false),
                ("poll_interval", false),
            ],
            // A set of discrete lines is neither asked nor polled: it is
            // watched, by its edges or by a report of what it is, which is
            // `state_interval`. So none of the words that drive a payload
            // that is read -- a trigger, the rate it goes out at, the delay
            // before reading, the floor between reads -- is one of these.
            NodeType::Gpio => &[
                ("path", true),
                ("lines", true),
                ("direction", false),
                ("edge", false),
                ("bias", false),
                ("debounce", false),
                ("group", false),
                ("state_interval", false),
            ],
            // Every term any computation takes, which is what this list is
            // for: the function itself is required of all of them, and each of
            // the rest is required by the one function that uses it -- a rule
            // this list cannot state and `tcslibgs::node::vehicle::compute`
            // does. Grouped by computation, in the order the tree offers them.
            //
            // Held against that module by
            // `tests::the_compute_fields_are_every_computations_terms`: three
            // of these were missing, and a hysteresis placed from a palette
            // with no description behind it had nowhere to type its setpoint.
            NodeType::Compute => &[
                ("function", true),
                // to_kelvin
                ("format", false),
                ("scale", false),
                ("offset", false),
                // vote
                ("inputs", false),
                ("algorithm", false),
                // hysteresis
                ("setpoint", false),
                ("half_band", false),
                // process
                ("executable", false),
                ("timeout", false),
                ("restart", false),
            ],
            // Nothing at all. A null node drops what arrives and a tee sends
            // it on both ways: a name and a type are the whole of what there
            // is to say, and the configuration check refuses any attribute
            // written on one.
            NodeType::NullMoc | NodeType::NullPayload | NodeType::TeeMoc | NodeType::TeePayload => {
                &[]
            }
            // Nothing either, for the same reason: what a ping node does is
            // fixed by the format of a ping, and the check refuses an
            // attribute on one as it does on a null node.
            NodeType::Ping => &[],
            NodeType::Log => &[("directory", true), ("format", true)],
            // Read from the rules rather than repeated, as the terminals are:
            // what an app node may say is `tcslibgs::node::app`'s, and it
            // differs by kind -- the argument for a stream is said by the
            // kinds that handle that stream -- so each of the five asks for
            // its own and an attribute added there appears in a node's window
            // without this file being touched.
            NodeType::AppPayloadVeh
            | NodeType::AppMocVeh
            | NodeType::AppAllVeh
            | NodeType::AppPl
            | NodeType::AppAllPl => tcslibgs::node::app::attributes_of(self.spelling())
                .expect("a type that is one of the app kinds"),
            NodeType::Moc => &[("ground_base", true), ("address", false), ("port", false)],
            NodeType::Subdiagram => &[("file", true)],
        }
    }

    /// Which terminals a node of this type has, as the canvas draws them.
    ///
    /// The four names and what each one is belong to
    /// [`tcslibgs::graph::terminals`], and for every type that module has an
    /// answer for -- a payload kind, the ground link, a tee --
    /// this reads it rather than repeating it: a terminal added there appears
    /// on the canvas without this file being touched, and the two cannot
    /// disagree about what a payload has.
    ///
    /// The rest are the types whose terminals that module calls their own
    /// business. The canvas has to draw something, because a terminal is the
    /// only thing a link can be dragged from or to -- a node with none drawn
    /// is a node nothing can be linked to -- and what is drawn is this:
    ///
    /// * a computation may be on either stream in either direction. The
    ///   readings arrive at it, its result goes to the ground, and the shipped
    ///   thermostat drives a line with it -- so it has all four. Its named
    ///   inputs, `in0` and the rest, are a vote's own affair and are not
    ///   terminals: a line dragged to a computation that declares inputs fills
    ///   the next free one, and no terminal is written;
    /// * a log is written to and read by nobody aboard, so it has the one
    ///   arriving terminal and no leaving one. Nothing can be dragged out of a
    ///   log, which is the rule the flight software enforces anyway;
    /// * a page of its own has all four, because what is inside it may have
    ///   any of them and nothing out here knows which.
    ///
    /// What is *drawn* and what is *written* are not the same question. A
    /// terminal is written into the file only where the type has more than one
    /// of that kind -- see [`NodeType::must_name`] -- so the file keeps saying
    /// `{ from: temp_1, to: moc }`, which is what the schema reads it as
    /// anyway.
    pub fn terminals(self) -> Terminals {
        // What tcslibgs says, where it says anything.
        if let Some(terminals) = tcslibgs::graph::terminals::terminals_of(self.spelling()) {
            return Terminals {
                sinks: terminals.sinks,
                sources: terminals.sources,
            };
        }
        match self {
            NodeType::Log => Terminals {
                sinks: &["payload-in"],
                sources: &[],
            },
            // A computation and a page: both ends of both streams.
            _ => Terminals {
                sinks: &["payload-in", "moc-in"],
                sources: &["payload-out", "moc-out"],
            },
        }
    }

    /// Whether a link leaving or arriving here has to say which terminal.
    ///
    /// Only where the type has more than one of that kind, which is the
    /// schema's own rule for what a link end may leave unwritten -- and the
    /// rule that keeps the editor from writing a name into every link in every
    /// file it saves. `sink` says which end: `true` for where data arrives.
    ///
    /// Asked of [`NodeType::terminals`], which is the table the canvas drew
    /// from: the question is about the gesture, and what the gesture pointed at
    /// is one of the boxes on the screen. Asking the schema's table instead
    /// threw away what a person had just said -- a computation has two data
    /// sinks drawn on it and the schema calls its sinks its own business, so a
    /// line dragged to the `moc-in` in its upper left was recorded as a link
    /// naming no terminal, and then drawn arriving at the other corner.
    pub fn must_name(self, sink: bool) -> bool {
        let terminals = self.terminals();
        let how_many = if sink {
            terminals.sinks.len()
        } else {
            terminals.sources.len()
        };
        how_many > 1
    }

    /// Which family this type belongs to.    /// Which family this type belongs to.
    ///
    /// The families are what the types are for, within the three outlines: a
    /// parallelogram is data crossing a boundary, a payload; a rounded box is
    /// a process, a part of the vehicle; and a plain rectangle is something
    /// outside the system, the ground. The palette shows the types under these
    /// headings, which is how the grammar gets taught -- a flat list of twenty
    /// shapes hides the pattern that would make them learnable.
    pub fn family(self) -> &'static str {
        match self {
            NodeType::I2c
            | NodeType::Spi
            | NodeType::Serial
            | NodeType::Hdlc
            | NodeType::Can
            | NodeType::Iio
            | NodeType::Rpmsg
            | NodeType::Network
            | NodeType::DeviceFile => "Devices",
            NodeType::Gpio => "Driven",
            NodeType::Compute => "Computation",
            // Where a stream goes both ways, and where one stops.
            NodeType::TeeMoc | NodeType::TeePayload => "Pass-through",
            NodeType::NullMoc | NodeType::NullPayload => "Ends of the graph",
            NodeType::Ping => "Link checks",
            // Two families of one, because they are two kinds of end and now
            // two shapes: the log is where the data stops on board and the
            // ground station is where it leaves the vehicle. They are also in
            // two different panes, so one heading over both was never read as
            // a group by anybody.
            NodeType::Log => "Ends of the graph",
            // A family of three, and the heading says what they have in
            // common: the node is a program, and what it does is not in this
            // software at all.
            NodeType::AppPayloadVeh | NodeType::AppMocVeh | NodeType::AppAllVeh => {
                "Programs of their own"
            }
            // The same thing on the other side of the canvas, and a heading of
            // its own because it is a different sentence: these are not parts
            // of the system written elsewhere, they are what the vehicle
            // carries -- a payload whose readings come from something the
            // mission wrote rather than from a bus. Which pane a family
            // appears on is `is_payload`, and the two must agree: see
            // `tests::a_payload_is_a_device_or_a_driven_line`.
            NodeType::AppPl | NodeType::AppAllPl => "Payloads of their own",
            NodeType::Moc => "The ground station",
            NodeType::Subdiagram => "Pages",
        }
    }

    /// Whether this type is a payload.
    ///
    /// A payload is something the vehicle carries and reads: a device on a
    /// bus, a line it drives, a link to something aboard. The rest of the
    /// graph is not -- a computation, a log, the ground station, a page of its
    /// own are all parts of the telemetry and control system itself rather
    /// than things bolted to the vehicle.
    ///
    /// The editor keeps the two apart on the canvas: payloads live in the
    /// payload working pane and everything else in the vehicle working pane,
    /// which is what makes a diagram readable as "here is the vehicle, and
    /// here is what it is carrying this flight". The flight is the thing that
    /// changes between missions, and a drawing that puts the changing part in
    /// one place is a drawing that can be compared with last mission's.
    ///
    /// Listed rather than derived from [`NodeType::family`] so that a new type
    /// has to say which side it is on. The test below checks the two agree.
    pub fn is_payload(self) -> bool {
        match self {
            NodeType::I2c
            | NodeType::Spi
            | NodeType::Serial
            | NodeType::Hdlc
            | NodeType::Can
            | NodeType::Iio
            | NodeType::Rpmsg
            | NodeType::Network
            | NodeType::DeviceFile
            | NodeType::Gpio
            // A payload need not be a device. These two are programs, and
            // what makes them payloads is that they are what the vehicle
            // carries this flight: they are written in the `payloads` section
            // and read the way a sensor is read.
            | NodeType::AppPl
            | NodeType::AppAllPl => true,
            NodeType::Compute
            | NodeType::NullMoc
            | NodeType::NullPayload
            | NodeType::TeeMoc
            | NodeType::TeePayload
            | NodeType::Ping
            | NodeType::Log
            | NodeType::AppPayloadVeh
            | NodeType::AppMocVeh
            | NodeType::AppAllVeh
            | NodeType::Moc
            | NodeType::Subdiagram => false,
        }
    }

    /// How this type is drawn.
    ///
    /// One outline to a pane, so the outline says which pane a node belongs in:
    /// a parallelogram is data crossing a boundary, a payload; rounded corners
    /// are a process aboard, a part of the vehicle; and the plain rectangle is
    /// something outside the system altogether, the ground. Within an outline
    /// the types differ by the edge, the weight and the way the label is set
    /// -- not the size, every node being one size: see `HEIGHT`.
    pub fn appearance(self) -> Appearance {
        use Edge::*;
        use Outline::*;
        use Weight::*;
        match self {
            // Buses and the rest of the payloads: the data symbol, and told
            // apart from each other by the line around them, its weight and
            // the label. Nine of them, which is more than four edges by two
            // weights: so the label differs for every one, and so does the
            // line, a dotted one among them.
            NodeType::I2c => Appearance {
                face: Face::Mono,
                label_size: 12,
                label_weight: 700,
                label_italic: false,
                outline: Parallelogram,
                edge: Single,
                weight: Narrow,
                size: (WIDTH, HEIGHT),
            },
            NodeType::Spi => Appearance {
                face: Face::Mono,
                label_size: 12,
                label_weight: 400,
                label_italic: false,
                outline: Parallelogram,
                edge: Double,
                weight: Narrow,
                size: (WIDTH, HEIGHT),
            },
            NodeType::Serial => Appearance {
                face: Face::Mono,
                label_size: 10,
                label_weight: 400,
                label_italic: true,
                outline: Parallelogram,
                edge: Dashed,
                weight: Narrow,
                size: (WIDTH, HEIGHT),
            },
            // Wide, double-outlined and in the sand tint, with a plain face
            // at the largest label size: a square outline because it is
            // hardware, and nothing else here is sand and double at once.
            NodeType::Hdlc => Appearance {
                face: Face::Plain,
                label_size: 14,
                label_weight: 700,
                label_italic: false,
                outline: Parallelogram,
                edge: Double,
                weight: Wide,
                size: (WIDTH, HEIGHT),
            },
            NodeType::Can => Appearance {
                face: Face::Mono,
                label_size: 14,
                label_weight: 700,
                label_italic: false,
                outline: Parallelogram,
                edge: DashedDouble,
                weight: Narrow,
                size: (WIDTH, HEIGHT),
            },
            NodeType::Iio => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 400,
                label_italic: true,
                outline: Parallelogram,
                edge: DashedDouble,
                weight: Wide,
                size: (WIDTH, HEIGHT),
            },
            NodeType::Rpmsg => Appearance {
                face: Face::Mono,
                label_size: 10,
                label_weight: 700,
                label_italic: true,
                outline: Parallelogram,
                edge: Single,
                weight: Wide,
                size: (WIDTH, HEIGHT),
            },
            NodeType::Network => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 700,
                label_italic: false,
                outline: Parallelogram,
                edge: Dashed,
                weight: Wide,
                size: (WIDTH, HEIGHT),
            },
            NodeType::DeviceFile => Appearance {
                face: Face::Mono,
                label_size: 10,
                label_weight: 400,
                label_italic: false,
                outline: Parallelogram,
                edge: Dotted,
                weight: Wide,
                size: (WIDTH, HEIGHT),
            },
            // Driven rather than read: a payload, so a parallelogram, told from
            // the others by a dotted line and a plain name.
            NodeType::Gpio => Appearance {
                face: Face::Plain,
                label_size: 14,
                label_weight: 400,
                label_italic: false,
                outline: Parallelogram,
                edge: Dotted,
                weight: Narrow,
                size: (WIDTH, HEIGHT),
            },
            // Software aboard.
            // The node that sends a payload's data on both ways and does
            // nothing else to it: rounded, because it is software aboard, and
            // then open, narrow and set in the smallest type. A shape with
            // the least in it for a node with the least in it -- and the
            // narrow line and the small label are what tell it from the
            // computation without colour.
            NodeType::TeePayload => Appearance {
                face: Face::Plain,
                label_size: 10,
                label_weight: 400,
                label_italic: false,
                outline: Rounded,
                edge: Single,
                weight: Narrow,
                size: (WIDTH, HEIGHT),
            },
            // And the one that sends the ground's data on both ways: the line
            // doubled and broken, being two ways at once, and the label
            // italic, which is what a node the ground drives is set in.
            NodeType::TeeMoc => Appearance {
                face: Face::Plain,
                label_size: 10,
                label_weight: 700,
                label_italic: true,
                outline: Rounded,
                edge: DashedDouble,
                weight: Narrow,
                size: (WIDTH, HEIGHT),
            },
            // The nodes where a stream stops: the dotted line doubled, the
            // one rounded edge nothing else has, a line that runs out. The
            // one that takes the ground's data italic, as above, and narrow;
            // the one that takes a payload's upright and wide.
            NodeType::NullMoc => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 400,
                label_italic: true,
                outline: Rounded,
                edge: DottedDouble,
                weight: Narrow,
                size: (WIDTH, HEIGHT),
            },
            NodeType::NullPayload => Appearance {
                face: Face::Plain,
                label_size: 10,
                label_weight: 400,
                label_italic: false,
                outline: Rounded,
                edge: DottedDouble,
                weight: Wide,
                size: (WIDTH, HEIGHT),
            },
            // The node that answers the ground: rounded, being software aboard,
            // and narrow and dashed, a dashed line being the one that comes
            // and goes -- which is a ping, out and back. No other type is
            // drawn rounded, dashed and narrow, so it is told apart without
            // colour.
            NodeType::Ping => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 700,
                label_italic: false,
                outline: Rounded,
                edge: Dashed,
                weight: Narrow,
                size: (WIDTH, HEIGHT),
            },
            NodeType::Compute => Appearance {
                face: Face::Plain,
                label_size: 14,
                label_weight: 400,
                label_italic: false,
                outline: Rounded,
                edge: Single,
                weight: Wide,
                size: (WIDTH, HEIGHT),
            },
            // A program of its own aboard: a part of the vehicle, so rounded,
            // and told from the vehicle's other nodes and from each other by
            // the line and the label -- the one with both interfaces is
            // doubled and bold, the one the ground drives is dashed and
            // italic, and the payload's is doubled and broken. Which stream
            // each carries is said by its terminals, that being what terminals
            // are for.
            NodeType::AppPayloadVeh => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 400,
                label_italic: false,
                outline: Rounded,
                edge: DashedDouble,
                weight: Wide,
                size: (WIDTH, HEIGHT),
            },
            NodeType::AppMocVeh => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 400,
                label_italic: true,
                outline: Rounded,
                edge: Dashed,
                weight: Wide,
                size: (WIDTH, HEIGHT),
            },
            NodeType::AppAllVeh => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 700,
                label_italic: false,
                outline: Rounded,
                edge: Double,
                weight: Wide,
                size: (WIDTH, HEIGHT),
            },
            // The two payloads that are programs: parallelograms, because they
            // are payloads and every payload wears that shape, with the dotted
            // line doubled -- the one edge no other node has -- saying "a
            // program" among them.
            //
            // Told apart from each other by the weight and the label: the one
            // nothing arrives at is narrow and set small, the one that is
            // asked is wide.
            NodeType::AppPl => Appearance {
                face: Face::Plain,
                label_size: 10,
                label_weight: 400,
                label_italic: false,
                outline: Parallelogram,
                edge: DottedDouble,
                weight: Narrow,
                size: (WIDTH, HEIGHT),
            },
            NodeType::AppAllPl => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 400,
                label_italic: false,
                outline: Parallelogram,
                edge: DottedDouble,
                weight: Wide,
                size: (WIDTH, HEIGHT),
            },
            // A diagram of its own: a part of the vehicle, so rounded, with a
            // second line inside the first and a large bold name -- a page is
            // a step whose insides are drawn somewhere else.
            NodeType::Subdiagram => Appearance {
                face: Face::Plain,
                label_size: 14,
                label_weight: 700,
                label_italic: false,
                outline: Rounded,
                edge: Double,
                weight: Narrow,
                size: (WIDTH, HEIGHT),
            },
            // Where the data is kept aboard: rounded, as every part of the
            // vehicle is, and told from the others by a dotted line and a small
            // italic name.
            NodeType::Log => Appearance {
                face: Face::Plain,
                label_size: 10,
                label_weight: 400,
                label_italic: true,
                outline: Rounded,
                edge: Dotted,
                weight: Narrow,
                size: (WIDTH, HEIGHT),
            },
            // The ground station: a plain rectangle with square corners,
            // which is the external entity of a data-flow diagram -- the thing
            // outside the system that data comes from and goes to. That is
            // what the ground is, and now that the payloads wear the data
            // symbol the rectangle belongs to the one node it is the standard
            // mark for.
            //
            // It has had a pill and a five-sided point before this. Both said
            // "not like the others" by being unlike anything, where the
            // rectangle says it by being the one shape a reader of these
            // notations already knows the meaning of. Wide, solid, azure and
            // set in the largest italic, so it is the one that catches the eye
            // without being the one shaped oddly.
            NodeType::Moc => Appearance {
                face: Face::Plain,
                label_size: 16,
                label_weight: 700,
                label_italic: true,
                outline: Square,
                edge: Single,
                weight: Wide,
                size: (WIDTH, HEIGHT),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every outline is a polygon, stays inside its own box, and holds the
    /// terminals it is drawn with.
    ///
    /// A polygon because a polygon is what a dash and a second line are walked
    /// along: the rounded outline was a corner radius once, and could take
    /// neither, which left the vehicle's seven types two channels to be told
    /// apart by. The markup draws the paths, so an outline with no polygon
    /// would be drawn not at all and nothing would say so.
    #[test]
    fn every_outline_is_a_polygon() {
        for kind in NodeType::ALL {
            let drawn = kind.appearance();
            let (width, height) = drawn.size;
            let points = polygon_of(drawn.outline, drawn.size);
            let shape = shape_of(drawn.outline, drawn.size);
            let edge = edge_of(drawn.outline, drawn.edge, drawn.size);

            assert!(!points.is_empty(), "{} has no polygon", kind.spelling());
            assert!(
                !shape.is_empty() && !edge.is_empty(),
                "{}'s polygon and its paths disagree: {shape:?}, {edge:?}",
                kind.spelling()
            );

            // Inside its own box, every coordinate of it. A shape that
            // overhangs is a shape whose terminals and lines are all drawn
            // somewhere its outline is not.
            for (x, y) in &points {
                assert!(
                    (0.0..=width as f32).contains(x) && (0.0..=height as f32).contains(y),
                    "{} has a corner at ({x}, {y}), outside {width} by {height}",
                    kind.spelling()
                );
            }

            // The second line of a doubled edge is inside the first. Inset as
            // a smaller box, so each shape insets the way it is shaped -- and
            // a parallelogram's inner line has to lean by its own slant or it
            // crosses the outer one.
            let inner = inner_of(drawn.outline, drawn.edge, drawn.size);
            assert_eq!(
                drawn.edge.doubled(),
                !inner.is_empty(),
                "{} has a {:?} edge and an inner line of {inner:?}",
                kind.spelling(),
                drawn.edge
            );
            for spot in points_of(&inner) {
                assert!(
                    within(&points, spot),
                    "{}'s inner line reaches {spot:?}, outside its own outline",
                    kind.spelling()
                );
            }

            // A broken line is the shape's own line with pieces missing, so
            // every piece of it lies on the outline. A dash walked off the
            // shape would be a line beside the node.
            for spot in points_of(&edge) {
                assert!(
                    on_edge(&points, spot, true),
                    "{}'s edge has a dash at {spot:?}, which is not on its \
                     outline",
                    kind.spelling()
                );
            }

            // And it holds every terminal it is drawn with. This is the rule
            // the shapes are chosen by: a terminal is placed in the corner of
            // the node's box, so a shape that cuts a corner off cuts a
            // terminal loose, and a terminal drawn outside its own outline
            // reads as something floating beside the node rather than part of
            // it. It is why the ground station's point was on the left -- its
            // own two terminals are both on the right -- and it is checked
            // here so that a shape kept and a terminal moved cannot pass.
            for terminal in kind
                .terminals()
                .sinks
                .iter()
                .chain(kind.terminals().sources)
            {
                let corner = corner_of(terminal).expect("a terminal of a known type");
                let (x, y) = terminal_at(drawn.outline, corner, drawn.size, (0, 0));
                let (wide, tall) = (TERMINAL.0 as i32, TERMINAL.1 as i32);
                for spot in [(x, y), (x + wide, y), (x, y + tall), (x + wide, y + tall)] {
                    assert!(
                        within(&points, (spot.0 as f32, spot.1 as f32)),
                        "{}'s {terminal} reaches ({}, {}), outside its own outline: {shape}",
                        kind.spelling(),
                        spot.0,
                        spot.1
                    );
                }
            }
        }
    }

    /// Whether a spot lies on one of a polygon's sides.
    ///
    /// Within half a pixel of it, as [`within`] is, and along it rather than
    /// merely in line with it: a dash is a piece of a side, so both ends of it
    /// have to be between the side's own ends.
    fn on_edge(points: &[(f32, f32)], (x, y): (f32, f32), closes: bool) -> bool {
        type Point = (f32, f32);
        let mut sides: Vec<(&Point, &Point)> = points.windows(2).map(|p| (&p[0], &p[1])).collect();
        if closes {
            sides.push((&points[points.len() - 1], &points[0]));
        }
        sides.iter().any(|((ax, ay), (bx, by))| {
            let (run_x, run_y) = (bx - ax, by - ay);
            let along = (run_x * run_x + run_y * run_y).sqrt();
            if along <= f32::EPSILON {
                return false;
            }
            let cross = (run_x * (y - ay) - run_y * (x - ax)) / along;
            let into = (run_x * (x - ax) + run_y * (y - ay)) / along;
            cross.abs() <= 0.5 && (-0.5..=along + 0.5).contains(&into)
        })
    }

    /// Whether a spot is inside a path's corners.
    ///
    /// The shapes here are convex, so this is "on the inside of every edge"
    /// rather than a crossing count: with the corners wound one way round, the
    /// cross product of an edge and the spot has one sign for every edge of a
    /// shape that holds it. On an edge counts as inside -- a terminal flush
    /// with the side of a square node is not outside it.
    fn within(points: &[(f32, f32)], (x, y): (f32, f32)) -> bool {
        points
            .iter()
            .zip(points.iter().cycle().skip(1))
            .all(|((ax, ay), (bx, by))| {
                let cross = (bx - ax) * (y - ay) - (by - ay) * (x - ax);
                // Divided by the edge's own length, so the half pixel of
                // slack is half a pixel of distance from the edge whatever
                // the edge's length: a cross product is an area, and half a
                // unit of area beside a 128-pixel side is no slack at all.
                // Half a pixel, because every one of these shapes is inset
                // half a pixel to keep its stroke inside the node.
                let along = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
                cross / along.max(f32::EPSILON) >= -0.5
            })
    }

    /// The corners of a path, read back out of its commands.
    ///
    /// Only the shapes above are read, which are moves and lines and nothing
    /// else, so this is two words at a time and not a parser.
    fn points_of(shape: &str) -> Vec<(f32, f32)> {
        let words: Vec<&str> = shape.split_whitespace().collect();
        let mut points = Vec::new();
        let mut word = 0;
        while word < words.len() {
            match words[word] {
                "M" | "L" => {
                    let x = words[word + 1].parse().expect("a coordinate");
                    let y = words[word + 2].parse().expect("a coordinate");
                    points.push((x, y));
                    word += 3;
                }
                "Z" => word += 1,
                other => panic!("{other} is not a move, a line or a close: {shape}"),
            }
        }
        points
    }

    /// No two node types are told apart by colour alone.
    ///
    /// The rule this module exists for, and the reason it is a test rather
    /// than a sentence in a comment: a tenth node type added later with a new
    /// colour and a borrowed shape would satisfy every reviewer and nobody
    /// with deuteranopia. Two channels must differ, not one, so that losing a
    /// channel to a projector or a printer still leaves a distinction.
    #[test]
    fn every_pair_of_types_differs_in_two_things_that_are_not_colour() {
        for (a, b) in NodeType::ALL
            .iter()
            .enumerate()
            .flat_map(|(i, a)| NodeType::ALL[i + 1..].iter().map(move |b| (*a, *b)))
        {
            let one = a.appearance();
            let two = b.appearance();
            let differences = [
                one.outline != two.outline,
                one.edge != two.edge,
                one.weight != two.weight,
                one.size != two.size,
                // How the label is set, which is one channel however many
                // properties carry it: a face, a size, a weight and an italic
                // are all type, and counting them separately would let four
                // typographic tweaks stand in for a shape.
                (one.face, one.label_size, one.label_weight, one.label_italic)
                    != (two.face, two.label_size, two.label_weight, two.label_italic),
            ]
            .iter()
            .filter(|differs| **differs)
            .count();

            assert!(
                differences >= 2,
                "{} and {} differ in {differences} ways that are not colour: \
                 {one:?} against {two:?}",
                a.spelling(),
                b.spelling()
            );
        }
    }

    /// Every type is drawn differently from every other, colour included.
    ///
    /// Weaker than the rule above and worth stating separately: this one would
    /// pass for two types that differ only in colour, which is why it is not
    /// the only test here.
    #[test]
    fn no_two_types_look_the_same() {
        for (i, a) in NodeType::ALL.iter().enumerate() {
            for b in &NodeType::ALL[i + 1..] {
                assert_ne!(
                    a.appearance(),
                    b.appearance(),
                    "{} and {} are drawn identically",
                    a.spelling(),
                    b.spelling()
                );
            }
        }
    }

    /// Every type has its own tint.
    ///
    /// Asked for, and worth a test rather than a glance down the table:
    /// Every terminal a type has is drawn somewhere, and in the right corner.
    ///
    /// The arrangement is the argument: the payload's own data along the top
    /// going right, what the ground sends along the bottom going left, so
    /// every arrow points the way the data travels across a canvas whose
    /// leftmost pane is the payloads and whose rightmost is the ground. A
    /// terminal in the wrong corner would make the drawing say the opposite of
    /// the file.
    #[test]
    fn every_terminal_is_drawn_in_the_corner_its_stream_runs_through() {
        for t in NodeType::ALL {
            for terminal in t.terminals().each() {
                let corner = corner_of(terminal)
                    .unwrap_or_else(|| panic!("{}: {terminal} has no corner", t.spelling()));
                // The top row is the payload's own data, on its way to the
                // ground, and the bottom row is what the ground sends back.
                // Each row runs one way.
                let payloads = terminal.starts_with("payload-");
                assert_eq!(corner.top, payloads, "{terminal}");
                assert_eq!(corner.rightward, payloads, "{terminal}");
                // Leaving on the side the data is going to, arriving on the
                // side it comes from: the payloads are to the left of the
                // vehicle and the ground to its right.
                let from_the_left = *terminal == "payload-in" || *terminal == "moc-out";
                assert_eq!(corner.left, from_the_left, "{terminal}");
            }
        }
        // And a computation's own input is not a terminal: it is drawn where
        // the link arrives, with its name on it.
        assert_eq!(corner_of("in0"), None);
    }

    /// Every terminal a type has fits inside it, clear of the others.
    ///
    /// What bounds [`TERMINAL`]. An `app_all_veh` is 40 pixels tall and
    /// carries all four, so growing a terminal is growing it against that: two rows
    /// have to fit inside the height with daylight between them, and two
    /// columns inside the width. A box that overran would be a box hanging
    /// outside its node -- pressable where nothing is drawn -- or two boxes
    /// lying on top of each other, where a press means whichever happens to be
    /// on top.
    #[test]
    fn the_terminals_of_every_type_fit_inside_it() {
        let inset = TERMINAL_INSET as i32;
        let (box_width, box_height) = (TERMINAL.0 as i32, TERMINAL.1 as i32);

        // Wherever a node may be: a terminal's place in it depends on where
        // the millimetres fall in pixels, so a run of the places a node may
        // be is tried, which covers every way they can fall.
        let places: Vec<i32> = std::iter::successors(Some(crate::pane::node_ceil(0)), |at| {
            Some(crate::pane::node_after(*at))
        })
        .take(40)
        .collect();
        for (t, node) in NodeType::ALL.iter().flat_map(|t| {
            places
                .iter()
                .zip(places.iter().rev())
                .map(move |(x, y)| (t, (*x, *y)))
        }) {
            let (width, height) = t.appearance().size;
            let mut boxes: Vec<(i32, i32)> = Vec::new();
            for terminal in t.terminals().each() {
                let corner = corner_of(terminal).expect("a corner");
                let at = terminal_at(t.appearance().outline, corner, (width, height), node);
                // On the lines five millimetres apart, where the node is.
                assert!(
                    crate::pane::is_on_line(node.0 + at.0)
                        && crate::pane::is_on_line(node.1 + at.1),
                    "{}: {terminal} at {at:?} in a node at {node:?} is off the lines",
                    t.spelling()
                );
                // Inside the node, with its inset on every side.
                assert!(at.0 >= inset && at.1 >= inset, "{terminal}: {at:?}");
                assert!(
                    at.0 + box_width <= width as i32 - inset
                        && at.1 + box_height <= height as i32 - inset,
                    "{}: {terminal} at {at:?} overruns {width}x{height}",
                    t.spelling()
                );
                boxes.push(at);
            }

            // And clear of each other, which is the rule the smallest node is
            // measured against.
            for (one, at) in boxes.iter().enumerate() {
                for theirs in boxes.iter().skip(one + 1) {
                    let over = at.0 < theirs.0 + box_width
                        && theirs.0 < at.0 + box_width
                        && at.1 < theirs.1 + box_height
                        && theirs.1 < at.1 + box_height;
                    assert!(
                        !over,
                        "{}: terminals at {at:?} and {theirs:?} overlap",
                        t.spelling()
                    );
                }
            }
        }
    }

    /// The compute fields are every computation's own terms.
    ///
    /// The window asks a node of a described kind for what its description
    /// lists -- a mean for a mean's attributes -- and falls back to this table
    /// for a type the tree does not describe. So the table has to hold every
    /// term any computation takes, or a compute node reached that way has
    /// nowhere to type one: a hysteresis had nowhere to type its setpoint,
    /// which is what this test was written for.
    ///
    /// The computations are read from the tree, so a fifth one arriving with
    /// its own terms fails here until they are offered.
    #[test]
    fn the_compute_fields_are_every_computations_terms() {
        let tree = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("share/nodes");
        let (described, trouble) = crate::nodes::read_tree(&tree);
        assert!(trouble.is_empty(), "{trouble:?}");

        let offered = NodeType::Compute.attributes();
        let mut asked = 0;
        for one in described.iter().filter(|one| one.node_type == "compute") {
            let function = one
                .sets
                .get("function")
                .unwrap_or_else(|| panic!("{}: a computation sets its function", one.node));
            let terms = tcslibgs::node::vehicle::compute::attributes_of(function)
                .unwrap_or_else(|| panic!("{function} is not a computation"));
            for term in terms {
                assert!(
                    offered.iter().any(|(name, _)| name == term),
                    "a {function} takes \"{term}\" and the compute fields do not offer it"
                );
                asked += 1;
            }
        }
        assert!(asked > 4, "no computation was asked about");

        // And nothing is offered that no computation takes: a field nobody
        // can use is a field somebody fills in and wonders about.
        let functions: Vec<&String> = described
            .iter()
            .filter(|one| one.node_type == "compute")
            .filter_map(|one| one.sets.get("function"))
            .collect();
        for (name, _) in offered {
            assert!(
                functions.iter().any(|function| {
                    tcslibgs::node::vehicle::compute::attributes_of(function)
                        .is_some_and(|terms| terms.contains(name))
                }),
                "\"{name}\" is offered and no computation takes it"
            );
        }
    }

    /// Every box carries an arrow, pointing the way its data travels.
    ///
    /// The name says whose data a terminal carries and whether it arrives or
    /// leaves -- `payload-in`, `moc-out` -- and the arrow says which way that
    /// is on the screen. One statement of it, derived from the stream rather
    /// than written out per corner: the payload's own data is on its way to
    /// the ground, which is rightwards, and what the ground sends is on its
    /// way to a payload, which is leftwards.
    #[test]
    fn every_terminal_carries_an_arrow_the_way_its_data_goes() {
        for terminal in ["payload-in", "payload-out"] {
            assert!(
                corner_of(terminal).expect("a corner").rightward,
                "{terminal}: the payload's own data is on its way to the ground"
            );
        }
        for terminal in ["moc-in", "moc-out"] {
            assert!(
                !corner_of(terminal).expect("a corner").rightward,
                "{terminal}: what the ground sends is on its way to a payload"
            );
        }
    }

    /// The editor draws the terminals the schema says a type has.
    ///
    /// Read from `tcslibgs` rather than copied, so this is a test that the
    /// reading happens: a terminal added to the schema has to appear on the
    /// canvas without anybody remembering to come here.
    #[test]
    fn the_canvas_shows_the_terminals_the_schema_gives_a_type() {
        for t in NodeType::ALL {
            let Some(schema) = tcslibgs::graph::terminals::terminals_of(t.spelling()) else {
                continue;
            };
            assert_eq!(t.terminals().sinks, schema.sinks, "{}", t.spelling());
            assert_eq!(t.terminals().sources, schema.sources, "{}", t.spelling());
        }

        // A payload is the left-hand end of both streams, so both of its
        // terminals are drawn on its right; the ground is the right-hand end,
        // and both of its are on its left. Spelled out here because it is
        // the thing a reader of the canvas sees first.
        for t in NodeType::ALL.iter().filter(|t| t.is_payload()) {
            for terminal in t.terminals().each() {
                assert!(!corner_of(terminal).unwrap().left, "{terminal}");
            }
        }
        for terminal in NodeType::Moc.terminals().each() {
            assert!(corner_of(terminal).unwrap().left, "{terminal}");
        }
    }

    /// The types the schema leaves open are drawn as what they are.
    ///
    /// Every one of them has something to drag a link from or to, because a
    /// terminal is now the only thing a link can be dragged from or to: a node
    /// drawn without one is a node nothing can be linked to. A log is the
    /// exception that proves it -- nothing is read from a log, so it has no
    /// data source and nothing can be dragged out of it, which is the rule the
    /// flight software enforces anyway.
    #[test]
    fn every_type_has_something_to_link_to() {
        for t in NodeType::ALL {
            assert!(
                !t.terminals().sinks.is_empty() || !t.terminals().sources.is_empty(),
                "{} has no terminal, so nothing can be linked to it",
                t.spelling()
            );
        }

        assert!(NodeType::Log.terminals().has("payload-in"));
        assert!(
            NodeType::Log.terminals().sources.is_empty(),
            "nothing is read from a log"
        );
        // A computation may be on either stream in either direction, and so
        // may a page: what is inside a page may have any of the four.
        assert_eq!(NodeType::Compute.terminals().each().count(), 4);
        assert_eq!(NodeType::Subdiagram.terminals().each().count(), 4);
    }

    /// A terminal is written into a file only where it has to be.
    ///
    /// Which is where the type has two of that kind, so that the file says
    /// which of them a link means. A payload and the ground link have one of
    /// each and their links name neither; a tee has two sources, and a
    /// computation and a page two of each, and theirs do.
    ///
    /// Asked of the table the canvas draws from, because the question is about
    /// a gesture: what somebody dragged a line to is one of the boxes on the
    /// screen, and a type whose terminals the schema leaves open still has
    /// them drawn.
    #[test]
    fn a_terminal_is_written_only_where_it_says_which() {
        for t in [NodeType::AppAllVeh, NodeType::Compute, NodeType::Subdiagram] {
            assert!(t.must_name(true), "{}", t.spelling());
            assert!(t.must_name(false), "{}", t.spelling());
        }
        // A tee has one sink, which needs no naming, and two sources, which
        // do.
        for t in [NodeType::TeeMoc, NodeType::TeePayload] {
            assert!(!t.must_name(true), "{}", t.spelling());
            assert!(t.must_name(false), "{}", t.spelling());
        }
        // One data sink and one data source: nothing to say which.
        for t in NodeType::ALL.iter().filter(|t| t.is_payload()) {
            assert!(!t.must_name(true), "{}", t.spelling());
            assert!(!t.must_name(false), "{}", t.spelling());
        }
        assert!(!NodeType::Moc.must_name(true));
        assert!(!NodeType::Moc.must_name(false));
        // A log receives on one and sends on none.
        assert!(!NodeType::Log.must_name(true));
        assert!(!NodeType::Log.must_name(false));
    }

    /// Which terminal a line is drawn to, where the link does not say.
    #[test]
    fn a_line_is_drawn_to_the_terminal_the_link_means() {
        // Named, and the type has it.
        assert_eq!(
            terminal_for("app_all_veh", Some("moc-in"), true),
            Some("moc-in")
        );
        // Named, and the type has not: a computation's own input, which is
        // not a terminal. The groundward sink is where readings arrive.
        assert_eq!(
            terminal_for("compute", Some("in0"), true),
            Some("payload-in")
        );
        // Nothing named, and one of that kind: the one.
        assert_eq!(terminal_for("i2c", None, false), Some("payload-out"));
        assert_eq!(terminal_for("moc", None, true), Some("payload-in"));
        // Nothing named and two of that kind: the groundward one, which is
        // what a node does unless the file says otherwise. The checking
        // refuses such a file; the canvas still has to draw it.
        assert_eq!(
            terminal_for("app_all_veh", None, false),
            Some("payload-out")
        );
        assert_eq!(terminal_for("app_all_veh", None, true), Some("payload-in"));
        // Nothing to read from a log, and nothing known about a type this
        // build does not have.
        assert_eq!(terminal_for("log", None, false), None);
        assert_eq!(terminal_for("contraption", None, true), None);
    }

    /// A line joins a node at the outer edge of the terminal's own box.
    ///
    /// Which is the side of the node where the shape reaches it, and in from
    /// the side where it does not: a payload leans, so the line to the
    /// terminal in its missing corner stops where the slant is rather than
    /// out in the notch beside it.
    #[test]
    fn a_line_joins_a_node_at_its_terminals_edge() {
        let drawn = NodeType::I2c.appearance();
        let (width, height) = drawn.size;
        let at = (400, 200);

        // A payload's pair, both on its right: its readings leave at the top
        // going right, and what asks for them arrives at the bottom going
        // left.
        let ((x, y), rightward) = attachment("i2c", at, "payload-out").expect("a payload");
        let room = room_at(drawn.outline, drawn.size, corner_of("payload-out").unwrap());
        assert!(
            room > 0,
            "a payload leans, so its upper right is in from the side"
        );
        assert_eq!(
            x,
            (at.0 + width as i32) as f32 - room as f32,
            "a terminal on the right joins the shape's own right"
        );
        assert!(rightward, "the readings leave towards the ground");
        assert!(y < at.1 as f32 + height as f32 / 2.0, "the upper corner");

        let ((_, lower), rightward) = attachment("i2c", at, "moc-in").expect("a payload");
        assert!(
            !rightward && lower > y,
            "what asks arrives at the lower corner"
        );

        // And the ground link's pair, both on its left.
        let ((x, _), _) = attachment("moc", at, "payload-in").expect("the ground");
        assert_eq!(x, at.0 as f32);

        // A terminal a node does not have is not a place to join it, and
        // neither is a name that is not a terminal at all.
        assert_eq!(attachment("i2c", at, "in0"), None);
        assert_eq!(attachment("contraption", at, "payload-out"), None);
        let _ = width;
    }

    /// Red is not in the palette at all.    /// Red is not in the palette at all.
    ///
    /// Every label is set legibly.
    ///
    /// Type is a distinguishing channel, which makes it tempting to reach for
    /// a six-pixel italic to tell two things apart. A distinction nobody can
    /// read is not one, so there is a floor.
    #[test]
    fn no_label_is_too_small_to_read() {
        for t in NodeType::ALL {
            let a = t.appearance();
            assert!(
                a.label_size >= 10,
                "{} sets its label at {}px",
                t.spelling(),
                a.label_size
            );
            assert!(
                a.label_weight == 400 || a.label_weight == 700,
                "{} asks for a weight no system face is sure to have: {}",
                t.spelling(),
                a.label_weight
            );
        }
    }

    /// Every node is drawn the same width.
    ///
    /// Alignment is the point: nodes of four widths in a column have no edge
    /// to line up on, and a diagram of them looks approximate however
    /// carefully it was laid out. A test rather than a comment because the
    /// next type added will be written by copying one of these, and the one it
    /// gets copied from is whichever was most convenient.
    #[test]
    fn every_type_is_drawn_the_same_width() {
        for t in NodeType::ALL {
            assert_eq!(
                t.appearance().size.0,
                WIDTH,
                "{} is {}px wide and everything else is {WIDTH}px",
                t.spelling(),
                t.appearance().size.0
            );
        }
    }

    /// A width that holds the longest name it has to hold.
    ///
    /// The label is a channel only while it can be read: the type name is
    /// shown under every node, and one that elides to "device-fi..." is a
    /// distinction that has stopped working. Measured generously -- a little
    /// over half the label size per character, which no system face exceeds
    /// for lower case -- because the point is a floor and not a layout.
    #[test]
    fn the_width_holds_the_longest_type_name() {
        for t in NodeType::ALL {
            let wanted = t.spelling().len() as f32 * 7.0 + 8.0;
            assert!(
                WIDTH as f32 >= wanted,
                "{} wants about {wanted:.0}px and a node is {WIDTH}px",
                t.spelling()
            );
        }
    }

    /// What is a payload and what is not, said twice and agreeing.
    ///
    /// [`NodeType::is_payload`] is written out type by type so that a new type
    /// has to declare which side of the canvas it belongs on. That leaves two
    /// statements of the same fact, and two statements of one fact drift: this
    /// is what stops them. The families are the shape grammar, and the grammar
    /// already divides what the vehicle carries from what the system is made
    /// of.
    ///
    /// Three families are what the vehicle carries now, the third being the
    /// payloads that are programs. A family belongs to one pane or the other
    /// and never to both: that is why the apps are two families and not one
    /// heading over five types, the vehicle's three being on the vehicle pane
    /// and the payload pair on the payload pane.
    #[test]
    fn a_payload_is_a_device_or_a_driven_line() {
        for t in NodeType::ALL {
            let by_family = matches!(t.family(), "Devices" | "Driven" | "Payloads of their own");
            assert_eq!(
                t.is_payload(),
                by_family,
                "{} is in the {} family and says is_payload() is {}",
                t.spelling(),
                t.family(),
                t.is_payload()
            );
        }

        // And both panes have something to offer: an empty selection pane is
        // a pane nobody can tell the purpose of.
        assert!(NodeType::ALL.iter().any(|t| t.is_payload()));
        assert!(NodeType::ALL.iter().any(|t| !t.is_payload()));
    }

    /// Every type says what it needs, and needs something.
    ///
    /// A type with no required attribute is a node that can be placed and
    /// left meaning nothing -- which is a configuration the flight software
    /// would refuse for reasons the editor could have given on the spot.
    #[test]
    fn every_type_states_what_it_requires() {
        for t in NodeType::ALL {
            let attributes = t.attributes();
            // The null nodes, the tees and a ping node are the types with
            // nothing to say: one drops what arrives, one sends it on both
            // ways and the last answers pings, so a name and a type are the
            // whole of any of them, and the configuration check refuses any
            // attribute written on one. Every other type that said nothing
            // would be a node that means nothing.
            if matches!(
                t,
                NodeType::NullMoc
                    | NodeType::NullPayload
                    | NodeType::TeeMoc
                    | NodeType::TeePayload
                    | NodeType::Ping
            ) {
                assert!(attributes.is_empty(), "{} says nothing", t.spelling());
                continue;
            }
            assert!(!attributes.is_empty(), "{} says nothing", t.spelling());
            assert!(
                attributes.iter().any(|(_, required)| *required),
                "{} requires nothing, so an empty one would be a node that \
                 means nothing",
                t.spelling()
            );

            // Required ones first, so the panel asks for what it must have
            // before what it may.
            let first_optional = attributes.iter().position(|(_, r)| !*r);
            if let Some(at) = first_optional {
                assert!(
                    attributes[at..].iter().all(|(_, r)| !*r),
                    "{}'s required attributes are not all first",
                    t.spelling()
                );
            }

            // And no attribute twice, which would give the panel two fields
            // writing to one value.
            let mut names: Vec<&str> = attributes.iter().map(|(n, _)| *n).collect();
            names.sort_unstable();
            let before = names.len();
            names.dedup();
            assert_eq!(before, names.len(), "{} lists one twice", t.spelling());
        }
    }

    /// Every label size is a step on the scale.
    ///
    /// The sizes were 9, 10, 11, 12, 13 and 15 pixels, each chosen when the
    /// type it belonged to was added. Four steps is enough to tell thirteen
    /// types apart -- the accessibility test above proves it, since it still
    /// passes -- and a window built from a scale looks composed where one
    /// built from whatever was wanted at the time looks assembled.
    #[test]
    fn every_label_size_is_on_the_scale() {
        const SCALE: [u32; 4] = [10, 12, 14, 16];
        for t in NodeType::ALL {
            let size = t.appearance().label_size;
            assert!(
                SCALE.contains(&size),
                "{} is set at {size}px, which is not a step on {SCALE:?}",
                t.spelling()
            );
        }
    }

    /// A label fits the shape it is in.
    ///
    /// A type is only a channel while the text is inside the box: a fifteen
    /// pixel label in a forty-eight pixel node leaves no room for the type
    /// name under it, and a label that overflows is a label that gets elided
    /// to nothing.
    #[test]
    fn every_label_fits_its_node() {
        for t in NodeType::ALL {
            let a = t.appearance();
            // The name, the type beneath it, the padding, and a little room:
            // whatever the face, a line takes about one and a third times its
            // size.
            let wanted = (a.label_size as f32 * 1.35) + 12.0 + 12.0;
            assert!(
                a.size.1 as f32 >= wanted,
                "{} is {}px tall and its label wants {wanted:.0}px",
                t.spelling(),
                a.size.1
            );
        }
    }

    /// Every family's members share the outline that defines it.
    ///
    /// The families are only worth showing if they mean something, and what
    /// they mean is the shape: a reader who learns that square is hardware has
    /// learned it for all eight of them or for none.
    #[test]
    fn a_family_shares_one_outline() {
        use std::collections::BTreeMap;

        let mut outlines: BTreeMap<&str, Vec<Outline>> = BTreeMap::new();
        for t in NodeType::ALL {
            outlines
                .entry(t.family())
                .or_default()
                .push(t.appearance().outline);
        }
        for (family, mut shapes) in outlines {
            shapes.sort_by_key(|o| format!("{o:?}"));
            shapes.dedup();
            assert_eq!(
                shapes.len(),
                1,
                "{family} is drawn with {} different outlines",
                shapes.len()
            );
        }
    }

    /// Every type is in a family, and every family has a name worth reading.
    #[test]
    fn every_type_belongs_somewhere() {
        for t in NodeType::ALL {
            let family = t.family();
            assert!(!family.is_empty(), "{} is in no family", t.spelling());
            assert!(
                family.chars().next().unwrap().is_uppercase(),
                "{family} is a heading and should read as one"
            );
        }
    }

    /// Every type has a spelling, and it is the configuration's word for it.
    #[test]
    fn every_type_spells_itself_as_the_configuration_does() {
        for t in NodeType::ALL {
            let spelling = t.spelling();
            assert!(!spelling.is_empty());
            assert_eq!(
                spelling,
                spelling.to_lowercase(),
                "a type's spelling is the configuration's, which is lower case"
            );
        }
        // And no two share one, which would make a palette entry ambiguous.
        let mut spellings: Vec<_> = NodeType::ALL.iter().map(|t| t.spelling()).collect();
        spellings.sort_unstable();
        let before = spellings.len();
        spellings.dedup();
        assert_eq!(before, spellings.len(), "two types share a spelling");
    }
}
