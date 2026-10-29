//! What a page of a configuration looks like, as numbers and text.
//!
//! A node becomes a [`NodeLook`] and a line between two nodes a [`LinkLook`]:
//! every path, size and colour the markup in `ui/diagram.slint` paints, worked
//! out here where it can be tested. The editor turns these into the structs
//! its window takes, and so does tcspecial's visual window, so that the two
//! draw one configuration alike.
//!
//! What is drawn is said in the configuration's own terms and nothing more: a
//! node is a name, a type and where it was put ([`OnSheet`]), and a line is
//! the two ends it joins ([`LineOnSheet`]). Which page a node is on, what is
//! selected and what is being dragged are the editor's, and it says so by
//! which nodes and lines it hands over.

use crate::look::{line_ink, tint_of, MARK};
use crate::pane::size_of;
use crate::route;
use crate::style::{self, Edge, Face, NodeType, Outline, Weight, ERROR_TINT};

/// One terminal of a node: whether its type has it, and where it is drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalLook {
    pub shown: bool,
    pub x: f32,
    pub y: f32,
    /// Whether the arrow points right, towards the ground. See
    /// [`style::Corner::rightward`].
    pub rightward: bool,
}

/// One node, as the markup's `NodeView` wants it, but for the two things only
/// a window knows: whether it is selected, and the colours as its toolkit's.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeLook {
    pub name: String,
    pub node_type: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// `square`, `rounded` or `parallelogram`.
    pub outline: &'static str,
    pub shape: String,
    pub edge: String,
    pub inner: String,
    pub border: f32,
    /// `plain` or `mono`.
    pub face: &'static str,
    pub label_size: f32,
    pub label_weight: i32,
    pub label_italic: bool,
    /// What the body is painted, as hex.
    pub tint: String,
    /// What is drawn on the body, as hex. See [`MARK`].
    pub mark: &'static str,
    pub faulty: bool,
    pub summary: String,
    pub openable: bool,
    pub moc_in: TerminalLook,
    pub moc_out: TerminalLook,
    pub payload_in: TerminalLook,
    pub payload_out: TerminalLook,
    pub terminal_column: f32,
    pub inset: f32,
}

/// What a node of this type looks like, at `at`.
///
/// `faulty` paints it in the error colour; `summary` is what a closed
/// subdiagram holds, and nothing for any other type.
///
/// # Arguments
///
/// * `name`      - The node's name, as its label shows it.
/// * `node_type` - The node type's spelling; a type this build does not
///                 know is drawn as a small square in the error colour.
/// * `at`        - Where the node's upper left corner is, in pixels.
/// * `faulty`    - Whether the node is marked faulty.
/// * `summary`   - What a closed subdiagram holds, or `None`.
///
/// # Returns
///
/// Everything the markup needs to draw the node.
pub fn node_look(
    name: &str,
    node_type: &str,
    at: (i32, i32),
    faulty: bool,
    summary: Option<&str>,
) -> NodeLook {
    let found = NodeType::ALL.iter().find(|t| t.spelling() == node_type);
    let appearance = found.map(|t| t.appearance());
    let terminals = found.map(|t| t.terminals()).unwrap_or(style::Terminals {
        sinks: &[],
        sources: &[],
    });

    // A type this build does not know is drawn as a small open square, which
    // is a shape no real type uses, and marked faulty. Showing it beats
    // refusing to open the file: a configuration written for a later build is
    // readable, and what cannot be placed is visibly what cannot be placed.
    let (
        outline,
        edge,
        weight,
        size,
        tint,
        face,
        label_size,
        label_weight,
        label_italic,
    ) = match appearance {
        Some(a) => (
            a.outline,
            a.edge,
            a.weight,
            a.size,
            tint_of(node_type),
            a.face,
            a.label_size,
            a.label_weight,
            a.label_italic,
        ),
        None => (
            Outline::Square,
            Edge::Single,
            Weight::Narrow,
            (104u32, 40u32),
            ERROR_TINT.to_string(),
            Face::Plain,
            11,
            400,
            true,
        ),
    };

    NodeLook {
        name: name.to_string(),
        node_type: node_type.to_string(),
        x: at.0 as f32,
        y: at.1 as f32,
        width: size.0 as f32,
        height: size.1 as f32,
        outline: match outline {
            Outline::Square => "square",
            Outline::Rounded => "rounded",
            Outline::Parallelogram => "parallelogram",
        },
        // The shape to fill, the line to stroke and the second line of a
        // doubled edge, all as paths in the node's own coordinates, for every
        // outline. See style::polygon_of, which all three come from.
        shape: style::shape_of(outline, size),
        edge: style::edge_of(outline, edge, size),
        inner: style::inner_of(outline, edge, size),
        border: match weight {
            Weight::Narrow => 1.0,
            Weight::Wide => 3.0,
        },
        face: match face {
            Face::Plain => "plain",
            Face::Mono => "mono",
        },
        label_size: label_size as f32,
        label_weight: label_weight as i32,
        label_italic,
        tint,
        // What is drawn *on* the body, now that the body is painted in the
        // tint: see NodeView::mark.
        mark: MARK,
        faulty,
        summary: summary.unwrap_or_default().to_string(),
        // A diagram of its own can be opened; nothing else can.
        openable: node_type == "subdiagram",
        // The terminals this type has, each drawn in its own corner. A type
        // this build does not know has none: the shape already says it cannot
        // be placed, and four arrows on it would be four claims about a type
        // nothing here can describe.
        moc_in: terminal_look("moc-in", terminals, outline, size, at),
        moc_out: terminal_look("moc-out", terminals, outline, size, at),
        payload_in: terminal_look("payload-in", terminals, outline, size, at),
        payload_out: terminal_look("payload-out", terminals, outline, size, at),
        // The column the name is padded away from, plus whatever the outline
        // is missing at its widest: a slanted shape moves its terminals in,
        // and a name padded by the column alone would be padded into them.
        terminal_column: (style::TERMINAL_COLUMN as i32
            + style::room_of(outline, size)) as f32,
        // The room anything drawn inside the shape has to leave at the sides,
        // which is nothing unless the shape leans: see style::room_of.
        inset: style::room_of(outline, size) as f32,
    }
}

/// One terminal of a node.
///
/// Whether this type has it and where it goes. The corner and the arrow are
/// `style`'s to say -- see [`style::corner_of`] -- so that the rule "the
/// vehicleward stream along the top going right, the groundward stream along
/// the bottom going left" is stated once and tested where it is stated.
///
/// # Arguments
///
/// * `terminal`  - The terminal's name.
/// * `terminals` - The terminals the node's type has.
/// * `outline`   - The node's outline.
/// * `size`      - The node's width and height, in pixels.
/// * `at`        - Where the node is, in pixels, which is what puts each
///                 terminal on the grid: see `style::terminal_at`.
///
/// # Returns
///
/// Whether the terminal is shown, where its box goes relative to the
/// node, and whether its arrow points right.
fn terminal_look(
    terminal: &str,
    terminals: style::Terminals,
    outline: Outline,
    size: (u32, u32),
    at: (i32, i32),
) -> TerminalLook {
    let corner = style::corner_of(terminal);
    let (x, y) = corner
        .map(|corner| style::terminal_at(outline, corner, size, at))
        .unwrap_or_default();
    TerminalLook {
        shown: terminals.has(terminal),
        x: x as f32,
        y: y as f32,
        rightward: corner.is_some_and(|corner| corner.rightward),
    }
}

/// A node, as a drawing needs it: what it is called, what it is and where it
/// was put.
///
/// `here` says whether it is on the page being drawn. A node on another page
/// is still handed over, because a line to it is drawn to a stub whose height
/// is taken from it.
///
/// `name` is the name the lines use for it, which for a node on a subdiagram's
/// page is the name qualified by that page -- `cabin/temp_1` -- and so not
/// always a name the configuration wrote down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnSheet<'a> {
    pub name: String,
    pub node_type: &'a str,
    pub at: (i32, i32),
    pub here: bool,
}

/// One line between two nodes, as a drawing needs it.
///
/// One per destination, as the configuration's links are read: a link to
/// three places is three lines. `origin` is the terminal it leaves by and
/// `input` the input or terminal it arrives at, where the file names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineOnSheet<'a> {
    pub from: &'a str,
    pub origin: Option<&'a str>,
    pub to: &'a str,
    pub input: Option<&'a str>,
    pub comment: Option<&'a str>,
}

/// One line, as the markup's `LinkView` wants it, but for the colours as its
/// toolkit's.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkLook {
    /// What it is drawn in, as hex, which says which stream it carries: see
    /// [`line_ink`].
    pub ink: String,
    pub route: String,
    pub head: String,
    pub marker: String,
    pub label_x: f32,
    pub label_y: f32,
    /// A line into a named input, drawn wide.
    pub wide: bool,
    pub label: String,
    pub comment: String,
}

/// How wide a line is drawn, as the markup draws it: a link into a named
/// input wide, and one into a node's only input narrow.
///
/// Here as well as in the markup so that a window drawing something in
/// proportion to a line -- tcspecial's lit line, three times as wide -- has
/// the line's width to be in proportion to.
///
/// # Arguments
///
/// * `wide` - Whether the line goes into a named input.
///
/// # Returns
///
/// The line's width, in pixels.
pub fn line_width(wide: bool) -> f32 {
    if wide {
        WIDE_LINE
    } else {
        NARROW_LINE
    }
}

/// The two widths of a line, in pixels. `LinkLine` in `ui/diagram.slint`
/// draws them, and a test holds the two to each other.
pub const NARROW_LINE: f32 = 2.5;
pub const WIDE_LINE: f32 = 5.0;

/// The node of this name, among `nodes`.
fn node<'a>(nodes: &'a [OnSheet<'a>], name: &str) -> Option<&'a OnSheet<'a>> {
    nodes.iter().find(|node| node.name == name)
}

/// Where a line starts and finishes, and which way it points at each end.
///
/// Each end at the terminal it is attached to, which is where the line has to
/// start and finish now that a link is drawn from a terminal: a line that left
/// the middle of a node's right edge while its arrow sat in the lower left
/// corner would be two statements about one link. See [`style::attachment`].
///
/// The direction comes with the point, and that is the other half of it. A
/// vehicleward link leaves a node's left side and arrives at the next one's
/// right side, so "out of the source, across, into the target" is no longer
/// always left to right -- see [`route::Ends`].
///
/// An end on another page is a stub: at the left edge for a line arriving
/// from elsewhere, and to the right of its source for one leaving.
///
/// # Arguments
///
/// * `nodes` - Every node the page's lines may reach, on this page or
///             another.
/// * `line`  - The line.
///
/// # Returns
///
/// The line's two ends and the way it points at each.
pub fn ends(nodes: &[OnSheet], line: &LineOnSheet) -> route::Ends {
    let from = node(nodes, line.from);
    let to = node(nodes, line.to);

    let joined = |node: &OnSheet, named: Option<&str>, sink: bool| {
        let terminal = style::terminal_for(node.node_type, named, sink)?;
        style::attachment(node.node_type, node.at, terminal)
    };
    // Which data sink the line arrives at, where the link does not say. The
    // one that pairs with the source it left: a link leaving `moc-out`
    // arrives at a `moc-in`, that being the same stream named at its other
    // end -- see tcslibgs::graph::terminals::answers. Without this the drawing
    // fell back to the groundward sink whatever the link carried, so a command
    // from the ground was drawn arriving at the corner the readings leave by.
    let pairing = |from: Option<&OnSheet>, origin: Option<&str>| {
        let from = from?;
        let leaves = style::terminal_for(from.node_type, origin, false)?;
        tcslibgs::graph::terminals::answers(leaves)
    };

    let ((x1, y1), leaves_right) = match from {
        Some(node) if node.here => joined(node, line.origin, false)
            .unwrap_or_else(|| {
                // A type this build does not know, drawn as a node with no
                // terminals: the middle of its right edge, where every link
                // used to leave.
                let (w, h) = size_of(node.node_type);
                (
                    (
                        (node.at.0 + w as i32) as f32,
                        (node.at.1 + h as i32 / 2) as f32,
                    ),
                    true,
                )
            }),
        // Off this page: a stub at the left edge.
        _ => (
            (0.0, to.map(|n| n.at.1 as f32 + 24.0).unwrap_or(24.0)),
            true,
        ),
    };
    let ((x2, y2), arrives_right) = match to {
        Some(node) if node.here => {
            // What the link wrote, where that is a terminal this type has; the
            // terminal that pairs with the source otherwise. A computation's
            // own input -- `in0` -- is neither, and falls through to the
            // pairing one: a mean's three lines arrive where its readings do.
            let wanted = line
                .input
                .filter(|input| {
                    NodeType::ALL
                        .iter()
                        .find(|t| t.spelling() == node.node_type)
                        .is_some_and(|t| t.terminals().has(input))
                })
                .or_else(|| pairing(from, line.origin));
            joined(node, wanted, true).unwrap_or_else(|| {
                let (_, h) = size_of(node.node_type);
                ((node.at.0 as f32, (node.at.1 + h as i32 / 2) as f32), true)
            })
        }
        // Off this page: a stub to the right of its source.
        _ => ((x1 + 72.0, y1), true),
    };

    route::Ends::new((x1, y1), (x2, y2), leaves_right, arrives_right)
}

/// Whether a node of this name is on the page.
fn here(nodes: &[OnSheet], name: &str) -> bool {
    node(nodes, name).is_some_and(|node| node.here)
}

/// What every line looks like, in the order given.
///
/// Each is laid clear of the ones laid before it -- two lines that run along
/// one track are one line on the screen, and the one drawn first is not on it
/// at all; see [`route::lay`] -- so the order is the order of the file, and
/// the same configuration is drawn the same way every time it is opened.
///
/// `lines` is the lines of this page: each has at least one end on it, and
/// one whose other end is elsewhere is drawn to a stub, labelled with the far
/// node's name.
///
/// # Arguments
///
/// * `nodes` - Every node the lines may reach, on this page or another.
/// * `lines` - The page's lines, in the file's order.
///
/// # Returns
///
/// Each line's look, in the same order.
pub fn link_looks(nodes: &[OnSheet], lines: &[LineOnSheet]) -> Vec<LinkLook> {
    // What a line may not run through: every node on this page, where it is
    // and how big it is. Worked out once for the page rather than once per
    // link, and handed to the router, which knows about geometry and nothing
    // about documents.
    let obstacles: Vec<route::Obstacle> = nodes
        .iter()
        .filter(|node| node.here)
        .map(|node| {
            let (width, height) = size_of(node.node_type);
            route::Obstacle::new(
                node.at.0 as f32,
                node.at.1 as f32,
                width as f32,
                height as f32,
            )
        })
        .collect();

    let mut taken: Vec<Vec<(f32, f32)>> = Vec::new();
    lines
        .iter()
        .map(|line| {
            let ends = ends(nodes, line);

            // The label says which input it arrives at, and names the far
            // node where that node is on another page.
            let from_here = here(nodes, line.from);
            let crossing = !from_here || !here(nodes, line.to);
            let far_end = if from_here { line.to } else { line.from };
            //
            // A terminal is not one of those, and is not labelled: it is drawn
            // as the corner the line is attached to and as the arrow in it,
            // which says the same thing in the place a reader is already
            // looking. A label reading "payload-in" beside every line would
            // be the name of the thing the picture is showing.
            let named_input = line.input.filter(|input| {
                tcslibgs::graph::terminals::stream_of(input).is_none()
            });
            let label = match (named_input, crossing) {
                (Some(input), false) => input.to_string(),
                (Some(input), true) => format!("{far_end} . {input}"),
                (None, true) => far_end.to_string(),
                (None, false) => String::new(),
            };

            // The line itself, laid once and clear of the lines already laid.
            // Everything else about its drawing comes from these points, so
            // the path, the marker and the label cannot disagree about where
            // the line went.
            let points = route::lay(ends, &obstacles, &taken);
            let drawn = route::drawn_route(&points);
            let marker = route::marker_on(&points);
            // The label goes at the middle of the route's longest run, which
            // is on the line wherever the line had to go.
            let (label_x, label_y) = route::label_on(ends, &points);
            taken.push(points);

            LinkLook {
                // Which stream it carries, as a colour: see `line_ink`.
                ink: line_ink(node(nodes, line.from).and_then(|from| {
                    style::terminal_for(from.node_type, line.origin, false)
                })),
                route: drawn,
                head: route::arrow(ends),
                marker,
                label_x,
                label_y,
                wide: named_input.is_some(),
                label,
                comment: line.comment.unwrap_or_default().to_string(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The markup draws the two widths this says, so a window drawing in
    /// proportion to a line is in proportion to the line it can see.
    #[test]
    fn the_widths_are_the_ones_the_markup_draws() {
        let markup = include_str!("../ui/diagram.slint");
        assert!(
            markup.contains(&format!(
                "stroke-width: link.wide ? {WIDE_LINE}px : {NARROW_LINE}px;"
            )),
            "LinkLine no longer draws {WIDE_LINE}px and {NARROW_LINE}px"
        );
        // And a lit line, under it, three times as wide.
        assert!(
            markup.contains(&format!(
                "stroke-width: 3 * (link.wide ? {WIDE_LINE}px : \
                 {NARROW_LINE}px);"
            )),
            "a lit line is no longer three times the width of the line it \
             lights"
        );
    }

    /// A line between two nodes on the page leaves the one and reaches the
    /// other, in the colour of the stream it carries, and is labelled with
    /// the named input it arrives at.
    #[test]
    fn a_line_between_two_nodes_is_drawn_in_its_streams_colour() {
        let nodes = [
            OnSheet {
                name: "temp".into(),
                node_type: "i2c",
                at: (40, 80),
                here: true,
            },
            OnSheet {
                name: "mean".into(),
                node_type: "compute",
                at: (400, 80),
                here: true,
            },
        ];
        let lines = [LineOnSheet {
            from: "temp",
            origin: None,
            to: "mean",
            input: Some("in0"),
            comment: None,
        }];
        let looks = link_looks(&nodes, &lines);
        assert_eq!(looks.len(), 1);
        assert_eq!(looks[0].ink, crate::look::PAYLOAD_LINE);
        assert_eq!(looks[0].label, "in0");
        assert!(looks[0].wide);
        assert!(looks[0].route.starts_with('M'), "{}", looks[0].route);
    }
}
