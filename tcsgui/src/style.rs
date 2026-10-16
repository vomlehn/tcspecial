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
/// Slint draws all of these from its own primitives -- a `Rectangle` with and
/// without `border-radius`, and one with a radius of half its shorter side for
/// a fully rounded end -- so none of this needs an image or a font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outline {
    /// Square corners. Something outside the system: the ground station.
    ///
    /// The external entity of a data-flow diagram, which is what the ground
    /// is: the far end of the link, not part of the vehicle. It was the
    /// payloads' shape, and they have the data symbol now, so the plain
    /// rectangle is free for the one node it is the standard symbol for.
    Square,
    /// A parallelogram. Data arriving or leaving: a payload.
    ///
    /// The flowchart symbol for input and output on an unspecified medium,
    /// which is exactly what the nine device kinds are -- a bus, a socket, a
    /// channel, a sample buffer, a file -- and the one symbol in the notation
    /// that means *data crossing a boundary* rather than a thing or a step.
    ///
    /// It leans right, as the symbol is drawn, so its missing corners are the
    /// top left and the bottom right. The top left is where a payload's
    /// `payload-out` terminal is, which is why [`room_at`] exists: a terminal
    /// in a corner the shape has given away is a terminal floating outside
    /// its node, so the terminals of a slanted shape move in with the slant,
    /// and so does the row its name is padded away from.
    ///
    /// Drawn from [`parallelogram`], for the reason [`Outline::Store`] gives.
    ///
    Parallelogram,
    /// Rounded corners. Software aboard: a computation.
    Rounded,
    /// Two long sides and one end: the data store of a data-flow diagram.
    ///
    /// What a log is. It was a pill, which is the flowchart symbol for a
    /// *terminator* -- where control enters or leaves a program -- and a log
    /// is not that: it is somewhere data is kept. Both notations in common use
    /// draw a store as a rectangle with at least one end left open, and
    /// leaving an end open is also the one way to stop being a rectangle
    /// without touching a corner.
    ///
    /// Open on the left, because a log's one terminal is `payload-in`, in its
    /// top right. The pill it replaced put that arrow about fifteen pixels
    /// outside its own outline, which is the defect this fixes as much as the
    /// meaning: a radius of half the shorter side rounds away the very corner
    /// the terminal is drawn in.
    ///
    /// Drawn from [`store`] rather than from a corner radius, because a
    /// radius cannot leave a side out. The arithmetic is there, where it can
    /// be tested, as the link routes' is.
    Store,
    /// A rectangle with one pair of corners rounded and one square, which
    /// reads as a direction. Interfaces that are driven rather than read.
    Keyed,
    /// A square rectangle with a bar inset at each end: a predefined process.
    ///
    /// The flowchart symbol for a step whose insides are written down
    /// somewhere else -- a subroutine -- which is what a page of its own is.
    /// It was a rounded rectangle with a second outline inside it, which was
    /// this symbol by accident and at one remove; the bars say the same thing
    /// the way every reader of a flowchart has already been taught it.
    ///
    /// Square corners, because a page of its own carries all four terminals
    /// and a terminal is drawn in a corner. The bars are inside the shape, so
    /// they cost nothing a corner needs: that is why this is the one standard
    /// symbol available to a node with every terminal.
    ///
    /// Drawn from a radius of nothing and two bars in the markup, not from a
    /// path: the bars are not one closed shape, and the markup already draws
    /// the keyed corner the same way.
    Barred,
}

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
}

impl Edge {
    /// Whether a second line is drawn inside the first.
    pub fn doubled(self) -> bool {
        matches!(self, Edge::Double | Edge::DashedDouble)
    }

    /// Whether the line is broken.
    pub fn broken(self) -> bool {
        matches!(self, Edge::Dashed | Edge::DashedDouble)
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
/// vehicleward one along the top, left to right, arriving from the ground at
/// `moc-in` and leaving for the payload at `moc-out`; the groundward one
/// along the bottom, right to left, arriving from the payload at
/// `payload-in` and leaving for the ground at `payload-out`.
///
/// Which means the arrows all point the way the data actually travels across
/// the canvas, because the canvas is laid out the same way: the ground is the
/// leftmost pane and the payloads the rightmost. A reader who has noticed that
/// once does not have to read the names again.
///
/// `None` for a name that is not one of the four, which is a computation's
/// own input: those are drawn where the link arrives and not as a terminal.
pub fn corner_of(terminal: &str) -> Option<Corner> {
    match terminal {
        // The payload's own data along the top, travelling leftwards: it
        // arrives from a payload on the right and leaves for the ground on
        // the left.
        "payload-out" => Some(Corner {
            top: true,
            left: true,
            rightward: false,
        }),
        "payload-in" => Some(Corner {
            top: true,
            left: false,
            rightward: false,
        }),
        // And what the ground sends along the bottom, travelling rightwards:
        // it arrives from the ground on the left and leaves for a payload on
        // the right.
        "moc-in" => Some(Corner {
            top: false,
            left: true,
            rightward: true,
        }),
        "moc-out" => Some(Corner {
            top: false,
            left: false,
            rightward: true,
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
    /// Whether the arrow points right, which is to say towards the payloads.
    ///
    /// The way the data on this terminal travels across the canvas, which is
    /// the way the canvas is laid out: the ground is the leftmost working pane
    /// and the payloads the rightmost. So the payload's own data -- telemetry,
    /// on its way to the ground -- points left, and what the ground sends
    /// points right.
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
/// Empty for a rounded outline, which is a corner radius and not a polygon:
/// an arc in a path is a different kind of arithmetic, and the two types drawn
/// with one need neither a dash nor a second line. The test below is what says
/// so, rather than this comment.
///
/// Wound clockwise, starting at the top left, and inset half a pixel so the
/// stroke sits inside the node: a path drawn on the boundary is drawn half
/// outside it, and a node that overhangs its own box by a pixel is a node
/// whose terminals and lines are all a pixel out.
pub fn polygon_of(outline: Outline, (width, height): (u32, u32)) -> Vec<(f32, f32)> {
    let (w, h) = (width as f32 - 0.5, height as f32 - 0.5);
    let (left, top) = (0.5, 0.5);
    match outline {
        // A radius, not a polygon.
        Outline::Rounded => Vec::new(),
        // The square one, and the two that are a square with something drawn
        // on it: the keyed corner and the two bars are overlays, so the
        // outline underneath them is the plain rectangle.
        Outline::Square | Outline::Keyed | Outline::Barred | Outline::Store => {
            vec![(left, top), (w, top), (w, h), (left, h)]
        }
        Outline::Parallelogram => {
            let slant = (h - top) / 2.0;
            vec![
                (left + slant, top),
                (w, top),
                (w - slant, h),
                (left, h),
            ]
        }
    }
}

/// Whether the outline's own line comes back to where it started.
///
/// Every shape's does but the data store's, which is a store because its left
/// side is not drawn. The shape it *fills* is closed all the same -- a fill
/// closes a path the way SVG does -- so this is a fact about the line and not
/// about the shape.
fn closes(outline: Outline) -> bool {
    outline != Outline::Store
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
/// Empty for a rounded outline, which the markup strokes with a Rectangle's
/// own border.
pub fn edge_of(outline: Outline, edge: Edge, size: (u32, u32)) -> String {
    line_of(&polygon_of(outline, size), outline, edge)
}

/// The second line of a doubled edge, inset four pixels from the first.
///
/// Empty unless the edge is a doubled one. The inset is taken as a smaller box
/// rather than by moving the points, so that each shape insets the way it is
/// shaped: a parallelogram's inner line leans by its own slant, and a store's
/// is open on the same side.
pub fn inner_of(outline: Outline, edge: Edge, (width, height): (u32, u32)) -> String {
    if !edge.doubled() || width <= 2 * DOUBLE || height <= 2 * DOUBLE {
        return String::new();
    }
    let inset = DOUBLE as f32;
    let points: Vec<(f32, f32)> = polygon_of(outline, (width - 2 * DOUBLE, height - 2 * DOUBLE))
        .into_iter()
        .map(|(x, y)| (x + inset, y + inset))
        .collect();
    line_of(&points, outline, edge)
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

/// One polygon as a line, dashed or whole.
fn line_of(points: &[(f32, f32)], outline: Outline, edge: Edge) -> String {
    if points.is_empty() {
        return String::new();
    }
    if !edge.broken() {
        let mut out = run_of(points);
        if closes(outline) {
            out.push_str(" Z");
        }
        return out;
    }
    dashes_of(points, closes(outline))
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
fn dashes_of(points: &[(f32, f32)], closes: bool) -> String {
    let mut sides: Vec<((f32, f32), (f32, f32))> = points
        .windows(2)
        .map(|pair| (pair[0], pair[1]))
        .collect();
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
        let at = |along: f32| {
            (
                from.0 + run_x * along / span,
                from.1 + run_y * along / span,
            )
        };

        let mut along = -into;
        while along < span {
            let start = along.max(0.0);
            let end = (along + DASH).min(span);
            if end > start {
                let (sx, sy) = at(start);
                let (ex, ey) = at(end);
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(&format!("M {sx:.1} {sy:.1} L {ex:.1} {ey:.1}"));
            }
            along += DASH + GAP;
        }
        // Whatever of the pattern this side ended partway through.
        into = (into + span) % (DASH + GAP);
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
/// What bounds it is the smallest node, which is the `null` one at 40 tall and
/// the one with all four: two rows of terminals and their insets have to fit
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

/// Where a terminal is drawn on a node of this size, relative to the node.
///
/// Here rather than in the markup for the reason the link routes are here:
/// geometry is arithmetic, and arithmetic in a layout language cannot be
/// tested. The test below is what says the four of them land in four
/// different corners and clear each other.
pub fn terminal_at(outline: Outline, corner: Corner, (width, height): (u32, u32)) -> (i32, i32) {
    let inset = TERMINAL_INSET as i32;
    let far = |whole: u32, box_: u32| whole as i32 - box_ as i32 - inset;
    // In by whatever the outline is missing at this row: see `room_at`.
    let room = room_at(outline, (width, height), corner);
    (
        if corner.left {
            inset + room
        } else {
            far(width, TERMINAL.0) - room
        },
        if corner.top { inset } else { far(height, TERMINAL.1) },
    )
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
            ours.iter().find(|terminal| **terminal == groundward).copied()
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
pub fn attachment(
    node_type: &str,
    at: (i32, i32),
    terminal: &str,
) -> Option<((f32, f32), bool)> {
    let found = NodeType::ALL.iter().find(|t| t.spelling() == node_type)?;
    let corner = corner_of(terminal)?;
    let drawn = found.appearance();
    let (width, height) = drawn.size;
    let (_, y) = terminal_at(drawn.outline, corner, (width, height));

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
    let (x, y) = terminal_at(drawn.outline, corner, drawn.size);
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
    /// A vehicle node that carries both streams through and does nothing to
    /// what passes. The one type with all four terminals, which is what makes
    /// it the statement of what a vehicle node is.
    Null,
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
    pub const ALL: [NodeType; 20] = [
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
        NodeType::Null,
        NodeType::Log,
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
            NodeType::Null => "null",
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
            ],
            NodeType::Can => &[
                ("interface", true),
                ("can_id", true),
                ("can_fd", false),
                ("loopback", false),
                ("group", false),
                ("trigger", false),
            ],
            NodeType::Iio => &[
                ("path", true),
                ("group", false),
                ("trigger", false),
            ],
            NodeType::Rpmsg => &[
                ("path", true),
                ("service", true),
                ("group", false),
                ("trigger", false),
            ],
            NodeType::Network => &[
                ("protocol", true),
                ("address", true),
                ("port", true),
                ("group", false),
                ("trigger", false),
            ],
            NodeType::DeviceFile => &[
                ("path", true),
                ("group", false),
                ("trigger", false),
            ],
            NodeType::Gpio => &[
                ("path", true),
                ("lines", true),
                ("direction", false),
                ("edge", false),
                ("bias", false),
                ("debounce_us", false),
                ("group", false),
                ("trigger", false),
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
                // mean
                ("inputs", false),
                // hysteresis
                ("setpoint", false),
                ("above", false),
                ("below", false),
                // process
                ("program", false),
                ("timeout", false),
                ("restart", false),
            ],
            // Nothing at all, and it is the one type with nothing. A null
            // node carries what arrives through: its name and its type are
            // the whole of what there is to say, and the configuration check
            // refuses any attribute written on one.
            NodeType::Null => &[],
            NodeType::Log => &[("directory", true), ("format", true)],
            // Read from the rules rather than repeated, as the terminals are:
            // what an app node may say is `tcslibgs::node::app`'s,
            // and a third attribute added there appears in a node's window
            // without this file being touched.
            NodeType::AppPayloadVeh
            | NodeType::AppMocVeh
            | NodeType::AppAllVeh
            | NodeType::AppPl
            | NodeType::AppAllPl => tcslibgs::node::app::ATTRIBUTES,
            NodeType::Moc => &[("address", true), ("port", true)],
            NodeType::Subdiagram => &[("file", true)],
        }
    }

    /// Which terminals a node of this type has, as the canvas draws them.
    ///
    /// The four names and what each one is belong to
    /// [`tcslibgs::graph::terminals`], and for every type that module has an
    /// answer for -- a payload kind, the ground link, a `null` vehicle node --
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
    ///   inputs, `in0` and the rest, are a mean's own affair and are not
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
    /// `{ from: temp_fore, to: moc }`, which is what the schema reads it as
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
    /// The families are the shape grammar, and it is the standard notations'
    /// grammar rather than one of our own: a parallelogram is data crossing a
    /// boundary, a rounded box is a process, a plain rectangle is something
    /// outside the system, a box with an end left open is somewhere data is
    /// kept, a barred one is a step written down elsewhere, and a keyed
    /// rectangle -- the one mark here that is nobody's standard -- is an
    /// interface that is driven rather than read. The palette shows them under these headings, which is how the
    /// grammar gets taught -- a flat list of thirteen shapes hides the pattern
    /// that would make them learnable.
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
            NodeType::Null => "Pass-through",
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
            | NodeType::Null
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
    /// The groupings are meant to be learnable rather than arbitrary: square
    /// a parallelogram is data crossing a boundary, rounded corners are a
    /// process aboard, a keyed rectangle is an interface that is driven, a
    /// barred one is a page written down somewhere else, and the two ends of
    /// the graph are the
    /// two shapes nothing else uses -- a pill where it stops on board, a
    /// store where it is kept, and the ground is the plain rectangle of
    /// something outside the system altogether. Within a shape the types
    /// differ by
    /// the edge, the weight and the size.
    pub fn appearance(self) -> Appearance {
        use Outline::*;
        use Edge::*;
        use Weight::*;
        match self {
            // Buses and the rest of the payloads: the data symbol, and told
            // apart from each other by the line around them and by size.
            NodeType::I2c => Appearance {
                face: Face::Mono,
                label_size: 12,
                label_weight: 700,
                label_italic: false,
                outline: Parallelogram,
                edge: Single,
                weight: Narrow,
                size: (WIDTH, 48),
            },
            NodeType::Spi => Appearance {
                face: Face::Mono,
                label_size: 12,
                label_weight: 400,
                label_italic: false,
                outline: Parallelogram,
                edge: Double,
                weight: Narrow,
                size: (WIDTH, 56),
            },
            NodeType::Serial => Appearance {
                face: Face::Mono,
                label_size: 10,
                label_weight: 400,
                label_italic: true,
                outline: Parallelogram,
                edge: Dashed,
                weight: Narrow,
                size: (WIDTH, 48),
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
                size: (WIDTH, 56),
            },
            NodeType::Can => Appearance {
                face: Face::Mono,
                label_size: 14,
                label_weight: 700,
                label_italic: false,
                outline: Parallelogram,
                edge: Double,
                weight: Narrow,
                size: (WIDTH, 64),
            },
            NodeType::Iio => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 400,
                label_italic: true,
                outline: Parallelogram,
                edge: DashedDouble,
                weight: Wide,
                size: (WIDTH, 56),
            },
            NodeType::Rpmsg => Appearance {
                face: Face::Mono,
                label_size: 10,
                label_weight: 700,
                label_italic: true,
                outline: Parallelogram,
                edge: Single,
                weight: Wide,
                size: (WIDTH, 48),
            },
            NodeType::Network => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 700,
                label_italic: false,
                outline: Parallelogram,
                edge: Dashed,
                weight: Wide,
                size: (WIDTH, 56),
            },
            NodeType::DeviceFile => Appearance {
                face: Face::Mono,
                label_size: 10,
                label_weight: 400,
                label_italic: false,
                outline: Parallelogram,
                edge: Double,
                weight: Wide,
                size: (WIDTH, 64),
            },
            // Driven rather than read, and shaped to say so.
            NodeType::Gpio => Appearance {
                face: Face::Plain,
                label_size: 14,
                label_weight: 700,
                label_italic: false,
                outline: Keyed,
                edge: Single,
                weight: Wide,
                size: (WIDTH, 72),
            },
            // Software aboard.
            // The one node that does nothing to what passes: rounded, because
            // it is software aboard, and then open, narrow and the shortest
            // thing on the canvas. A shape with the least in it for the node
            // with the least in it -- and a height nothing else uses, which is
            // what tells it from the other two rounded shapes without colour.
            NodeType::Null => Appearance {
                face: Face::Plain,
                label_size: 10,
                label_weight: 400,
                label_italic: false,
                outline: Rounded,
                edge: Single,
                weight: Narrow,
                size: (WIDTH, 40),
            },
            NodeType::Compute => Appearance {
                face: Face::Plain,
                label_size: 14,
                label_weight: 400,
                label_italic: false,
                outline: Rounded,
                edge: Single,
                weight: Wide,
                size: (WIDTH, 64),
            },
            // A program of its own: the barred rectangle, which is a step
            // whose insides are written down somewhere else. That is exactly
            // what one of these is -- the node is a program, and what it does
            // is in that program and not in this software -- and it is the
            // standard mark for it.
            //
            // The same shape a page of its own wears, which is the one place
            // this grammar doubles up, and for a reason that holds: a page and
            // an app are both a step written down elsewhere, and the two are
            // told apart by height and by the weight of the label, the page
            // being the tallest thing on the canvas and bold with it. The
            // three apps are one shape among themselves, as the nine devices
            // are: which of the two streams each carries is what its terminals
            // say, and terminals are drawn.
            // One shape for the three, as the nine devices wear one shape,
            // and told apart from each other by the line and the height: the
            // one with both interfaces is the tallest and doubled, the two
            // with one apiece are the shorter pair. Which stream each carries
            // is said by its terminals, that being what terminals are for.
            NodeType::AppPayloadVeh => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 400,
                label_italic: false,
                outline: Barred,
                edge: Single,
                weight: Wide,
                size: (WIDTH, 72),
            },
            NodeType::AppMocVeh => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 400,
                label_italic: false,
                outline: Barred,
                edge: Dashed,
                weight: Wide,
                size: (WIDTH, 64),
            },
            NodeType::AppAllVeh => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 400,
                label_italic: false,
                outline: Barred,
                edge: Double,
                weight: Wide,
                size: (WIDTH, 80),
            },
            // The two payloads that are programs: barred, because what they
            // do is written down somewhere else as every app's is, and narrow,
            // because they stand among the payloads and a payload's line is
            // narrow. So the shape says "a program" and the weight says "what
            // the vehicle carries", which is what each of them is.
            //
            // Told apart from each other by the line and the height, as the
            // other three are. The broken line doubled is the one nothing
            // arrives at: a payload with no data sink is the odd thing about
            // it, and a line that is twice broken is the line that says least
            // is coming.
            NodeType::AppPl => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 400,
                label_italic: false,
                outline: Barred,
                edge: DashedDouble,
                weight: Narrow,
                size: (WIDTH, 48),
            },
            NodeType::AppAllPl => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 400,
                label_italic: false,
                outline: Barred,
                edge: Double,
                weight: Narrow,
                size: (WIDTH, 56),
            },
            // A diagram of its own: a step whose insides are written down
            // somewhere else, which is a predefined process -- a rectangle
            // with a bar inset at each end. It was a rounded box with a second
            // outline inside it, which was this symbol by accident and at one
            // remove.
            //
            // Open now rather than doubled, because the bars are the second
            // thing inside the shape and two of those is a busy node. Still
            // the tallest thing on the canvas bar the log, because it stands
            // for a page rather than a device.
            NodeType::Subdiagram => Appearance {
                face: Face::Plain,
                label_size: 14,
                label_weight: 700,
                label_italic: false,
                outline: Barred,
                edge: Single,
                weight: Wide,
                size: (WIDTH, 88),
            },
            // Where the data is kept, which is a store and not an end: a
            // flowchart's rounded end is a terminator, where control enters or
            // leaves, and a log is neither. Narrow still, so the one thing
            // that changed is the thing that was wrong.
            NodeType::Log => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 400,
                label_italic: false,
                outline: Store,
                edge: Single,
                weight: Narrow,
                size: (WIDTH, 96),
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
            // the tallest of the ordinary band still, so it is the one that
            // catches the eye without being the one shaped oddly.
            NodeType::Moc => Appearance {
                face: Face::Plain,
                label_size: 16,
                label_weight: 700,
                label_italic: true,
                outline: Square,
                edge: Single,
                weight: Wide,
                size: (WIDTH, 72),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every outline is drawn by one of the two means, stays inside its own
    /// box, and holds the terminals it is drawn with.
    ///
    /// Two means, because a corner radius draws the rounded outline and
    /// nothing else: every other shape is a polygon, and a polygon is what a
    /// dash and a second line are walked along. The markup picks between them
    /// on the paths being empty, so an outline added later with a radius and a
    /// polygon both -- or with neither -- would be drawn twice or not at all,
    /// and in neither case would anything say so.
    #[test]
    fn every_outline_is_either_a_radius_or_a_polygon() {
        for kind in NodeType::ALL {
            let drawn = kind.appearance();
            let (width, height) = drawn.size;
            let points = polygon_of(drawn.outline, drawn.size);
            let shape = shape_of(drawn.outline, drawn.size);
            let edge = edge_of(drawn.outline, drawn.edge, drawn.size);
            let rounded = drawn.outline == Outline::Rounded;

            assert_eq!(
                rounded,
                points.is_empty(),
                "{} is {:?} and its polygon has {} corners",
                kind.spelling(),
                drawn.outline,
                points.len()
            );
            assert_eq!(
                points.is_empty(),
                shape.is_empty() && edge.is_empty(),
                "{}'s polygon and its paths disagree: {shape:?}, {edge:?}",
                kind.spelling()
            );

            if rounded {
                // A radius has no polygon to walk, so it can carry neither a
                // dash nor a second line, and the markup would silently draw
                // a plain rounded rectangle instead of saying so.
                assert!(
                    !drawn.edge.broken() && !drawn.edge.doubled(),
                    "{} is drawn with a radius and asks for a {:?} edge",
                    kind.spelling(),
                    drawn.edge
                );
                continue;
            }

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
                    on_edge(&points, spot, closes(drawn.outline)),
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
                let (x, y) = terminal_at(drawn.outline, corner, drawn.size);
                let (wide, tall) = (TERMINAL.0 as i32, TERMINAL.1 as i32);
                for spot in [
                    (x, y),
                    (x + wide, y),
                    (x, y + tall),
                    (x + wide, y + tall),
                ] {
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
        let mut sides: Vec<(&(f32, f32), &(f32, f32))> =
            points.windows(2).map(|p| (&p[0], &p[1])).collect();
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
        for (a, b) in NodeType::ALL.iter().enumerate().flat_map(|(i, a)| {
            NodeType::ALL[i + 1..].iter().map(move |b| (*a, *b))
        }) {
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
    /// going left, what the ground sends along the bottom going right, so
    /// every arrow points the way the data travels across a canvas whose
    /// leftmost pane is the ground and whose rightmost is the payloads. A
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
                assert_eq!(corner.rightward, !payloads, "{terminal}");
                // Leaving on the side the data is going to, arriving on the
                // side it comes from: the ground is to the left of the vehicle
                // and the payloads to its right.
                let from_the_left = *terminal == "moc-in" || *terminal == "payload-out";
                assert_eq!(corner.left, from_the_left, "{terminal}");
            }
        }
        // And a computation's own input is not a terminal: it is drawn where
        // the link arrives, with its name on it.
        assert_eq!(corner_of("in0"), None);
    }

    /// Every terminal a type has fits inside it, clear of the others.
    ///
    /// What bounds [`TERMINAL`]. The `null` node is 40 pixels tall and carries
    /// all four, so growing a terminal is growing it against that: two rows
    /// have to fit inside the height with daylight between them, and two
    /// columns inside the width. A box that overran would be a box hanging
    /// outside its node -- pressable where nothing is drawn -- or two boxes
    /// lying on top of each other, where a press means whichever happens to be
    /// on top.
    #[test]
    fn the_terminals_of_every_type_fit_inside_it() {
        let inset = TERMINAL_INSET as i32;
        let (box_width, box_height) = (TERMINAL.0 as i32, TERMINAL.1 as i32);

        for t in NodeType::ALL {
            let (width, height) = t.appearance().size;
            let mut boxes: Vec<(i32, i32)> = Vec::new();
            for terminal in t.terminals().each() {
                let corner = corner_of(terminal).expect("a corner");
                let at = terminal_at(t.appearance().outline, corner, (width, height));
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
    /// the ground, which is leftwards, and what the ground sends is on its way
    /// to a payload, which is rightwards.
    #[test]
    fn every_terminal_carries_an_arrow_the_way_its_data_goes() {
        for terminal in ["payload-in", "payload-out"] {
            assert!(
                !corner_of(terminal).expect("a corner").rightward,
                "{terminal}: the payload's own data is on its way to the ground"
            );
        }
        for terminal in ["moc-in", "moc-out"] {
            assert!(
                corner_of(terminal).expect("a corner").rightward,
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

        // A payload is the right-hand end of both streams, so both of its
        // terminals are drawn on its left; the ground is the left-hand end,
        // and both of its are on its right. Spelled out here because it is
        // the thing a reader of the canvas sees first.
        for t in NodeType::ALL.iter().filter(|t| t.is_payload()) {
            for terminal in t.terminals().each() {
                assert!(corner_of(terminal).unwrap().left, "{terminal}");
            }
        }
        for terminal in NodeType::Moc.terminals().each() {
            assert!(!corner_of(terminal).unwrap().left, "{terminal}");
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
    /// each and their links name neither; a `null` node, a computation and a
    /// page have two of each, and theirs do.
    ///
    /// Asked of the table the canvas draws from, because the question is about
    /// a gesture: what somebody dragged a line to is one of the boxes on the
    /// screen, and a type whose terminals the schema leaves open still has
    /// them drawn.
    #[test]
    fn a_terminal_is_written_only_where_it_says_which() {
        for t in [NodeType::Null, NodeType::Compute, NodeType::Subdiagram] {
            assert!(t.must_name(true), "{}", t.spelling());
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
            terminal_for("null", Some("moc-in"), true),
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
        assert_eq!(terminal_for("null", None, false), Some("payload-out"));
        assert_eq!(terminal_for("null", None, true), Some("payload-in"));
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

        // A payload's pair, both on its left: its readings leave at the top
        // going left, and what asks for them arrives at the bottom going
        // right.
        let ((x, y), rightward) = attachment("i2c", at, "payload-out").expect("a payload");
        let room = room_at(drawn.outline, drawn.size, corner_of("payload-out").unwrap());
        assert!(room > 0, "a payload leans, so its upper left is in from the side");
        assert_eq!(
            x,
            at.0 as f32 + room as f32,
            "a terminal on the left joins the shape's own left"
        );
        assert!(!rightward, "the readings leave towards the ground");
        assert!(y < at.1 as f32 + height as f32 / 2.0, "the upper corner");

        let ((_, lower), rightward) = attachment("i2c", at, "moc-in").expect("a payload");
        assert!(rightward && lower > y, "what asks arrives at the lower corner");

        // And the ground link's pair, both on its right.
        let ((x, _), _) = attachment("moc", at, "payload-in").expect("the ground");
        let (moc_width, _) = NodeType::Moc.appearance().size;
        assert_eq!(x, (at.0 + moc_width as i32) as f32);

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
            let by_family =
                matches!(t.family(), "Devices" | "Driven" | "Payloads of their own");
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
            // A null node is the one type with nothing to say: it carries
            // what arrives through, so its name and its type are the whole of
            // it, and the configuration check refuses any attribute written
            // on one. Every other type that said nothing would be a node that
            // means nothing.
            if t == NodeType::Null {
                assert!(attributes.is_empty(), "a null node says nothing");
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
