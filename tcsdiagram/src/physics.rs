//! How the simulator file's physics model is drawn: its components as nodes
//! in the Physics pane, its connections as links between them, and each
//! stand-in's `connects` as a link between its component and the payload
//! node it stands in for.
//!
//! The Physics pane is left of the payloads, at negative x in the
//! coordinates the configuration holds -- see `crate::pane`. A component
//! is drawn where the simulator file's `tcsgui` section says it was put,
//! kept inside the pane; one the section does not place is laid out here,
//! down the pane by name, so that a model nobody has arranged is drawn the
//! same every time it is opened, in the one order a model has.
//!
//! A physics node is the one shape no node of a configuration's has -- a
//! hexagon, pointed at both ends, with a dotted edge -- in the Physics pane's
//! own reddish purple, and labelled with its type as well as its name. It
//! has at most two terminals, both on its right, towards the payloads: one
//! output, at the top, that every value it gives out leaves by, and one
//! input, under it, that everything it takes in arrives at. A component's
//! terminals are many and named, but a line's terminal is in the file, and
//! a box for each would leave no room for the name. A physics link
//! is dashed and drawn in that purple, where a configuration's links are
//! solid: two things, and not the colour alone, say it is the simulator's.

use crate::look::{PHYSICS_LINE, PHYSICS_TINT};
use crate::pane::{
    edges_of, node_ceil, Dividers, Pane, NAMES, NODE_STEP_MM, PX_PER_MM,
};
use crate::view::{LinkLook, NodeLook, OnSheet, TerminalLook};
use crate::{route, style};

/// How big a physics node is drawn, in pixels: narrower than every node of
/// a configuration's, so that its size is a second thing that tells it
/// apart, and so that the links leaving it have room beside it in the
/// Physics pane; and wide enough for a name beside its terminals.
pub const SIZE: (u32, u32) = (150, 44);

/// How far from a terminal's left a line meets it, and how far from its top:
/// the middle of its right side, where [`looks`] ends every physics line.
pub const TERMINAL_RIGHT: f32 = style::TERMINAL.0 as f32;
pub const TERMINAL_MIDDLE: f32 = style::TERMINAL.1 as f32 / 2.0;

/// The type a physics node is drawn as, for the windows' lists: never one of
/// a configuration's.
pub const NODE_TYPE: &str = "physics";

/// One component of a model, as the drawing needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component<'a> {
    /// What it is called.
    pub name: &'a str,
    /// Its type: `heater`, `thermal_zone` and so on.
    pub kind: &'a str,
    /// Whether something is wrong with it, which paints it red.
    pub faulty: bool,
    /// Where the simulator file's `tcsgui` section says it was put, if it
    /// says.
    pub placed: Option<(i32, i32)>,
}

/// A stand-in's `connects`, as the drawing needs it: which payload node, and
/// which terminal of which component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Join<'a> {
    /// The payload node that is simulated.
    pub payload: &'a str,
    /// The terminal it is joined to, as the file writes it:
    /// `zone.temperature`.
    pub terminal: &'a str,
}

/// A simulator file's physics model and joins, owned, as a window that reads
/// the file itself draws them: see [`of_simulator`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Drawn {
    /// Each component's name and type, in name order.
    pub components: Vec<(String, String)>,
    /// Each connection, from and to, in the order of what they join.
    pub connections: Vec<(String, String)>,
    /// Each stand-in's payload and a terminal it is joined to.
    pub joins: Vec<(String, String)>,
    /// Where the `tcsgui` section put each component it places, by name.
    pub placed: std::collections::BTreeMap<String, (i32, i32)>,
}

impl Drawn {
    /// The nodes and links of this model, as [`looks`] makes them.
    ///
    /// # Arguments
    ///
    /// * `nodes`    - The configuration's nodes on the page drawn.
    /// * `dividers` - Where the panes are.
    /// * `shift`    - What is added to every x: see [`looks`].
    ///
    /// # Returns
    ///
    /// The nodes, and the links.
    pub fn looks(
        &self,
        nodes: &[OnSheet],
        dividers: Dividers,
        shift: i32,
    ) -> (Vec<NodeLook>, Vec<LinkLook>) {
        let components: Vec<Component> = self
            .components
            .iter()
            .map(|(name, kind)| Component {
                name,
                kind,
                faulty: false,
                placed: self.placed.get(name).copied(),
            })
            .collect();
        let connections: Vec<(&str, &str)> = self
            .connections
            .iter()
            .map(|(from, to)| (from.as_str(), to.as_str()))
            .collect();
        let joins: Vec<Join> = self
            .joins
            .iter()
            .map(|(payload, terminal)| Join { payload, terminal })
            .collect();
        looks(&components, &connections, &joins, nodes, dividers, shift)
    }
}

/// What a simulator file's world draws: its components, its connections,
/// and every simulated payload's `connects`, its group's where it states
/// none.
///
/// # Arguments
///
/// * `sim` - The simulator file.
///
/// # Returns
///
/// What is drawn, in the one order the model has.
pub fn of_simulator(sim: &tcslibgs::sim_config::SimConfigFile) -> Drawn {
    let mut drawn = Drawn::default();
    if let Some(world) = &sim.world {
        drawn.components = world
            .physics
            .components
            .iter()
            .map(|c| (c.name().to_string(), c.type_name().to_string()))
            .collect();
        drawn.components.sort();
        drawn.connections = world
            .physics
            .connections
            .iter()
            .map(|c| (c.from.clone(), c.to.clone()))
            .collect();
        drawn.connections.sort();
    }
    if let Some(tcsgui) = &sim.tcsgui {
        drawn.placed = tcsgui
            .nodes
            .iter()
            .map(|placed| (placed.name.clone(), (placed.x, placed.y)))
            .collect();
    }
    for payload in &sim.simulated_payloads {
        if let Some(said) = sim.settings_of(payload).connects {
            for terminal in tcslibgs::sim_config::terminals_of(&said) {
                drawn
                    .joins
                    .push((payload.name.clone(), terminal.to_string()));
            }
        }
    }
    drawn.joins.sort();
    drawn
}

/// How much of the Physics pane right of its nodes is kept for the lanes
/// the joins run down to the payloads, in pixels.
pub const LANES: i32 = 24;

/// The narrowest a node is drawn, however narrow the pane: its two
/// terminals and the hexagon's points, with the name elided to its first
/// letters. A tenth of the working width, the narrowest the pane is let be
/// (see `settings::LEAST`), less the lanes: the node and its lanes fit even
/// then, and the pane is dragged wider to read the names.
pub const NARROWEST: u32 = 72;

/// Where one component is drawn, in the coordinates the file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    /// Its upper left corner.
    pub at: (i32, i32),
    /// Its width and height.
    pub size: (u32, u32),
}

/// How wide a physics node is drawn in a pane this wide.
///
/// As wide as [`SIZE`] where the pane has room for that and the lanes the
/// joins run down beside it, and narrower where it has not, so that a
/// narrow pane keeps its nodes in it rather than under the payloads. Never
/// narrower than [`NARROWEST`], which still holds both terminals and a
/// little of the name.
///
/// # Arguments
///
/// * `dividers` - Where the panes are.
///
/// # Returns
///
/// The width, in pixels.
pub fn width_in(dividers: Dividers) -> i32 {
    let (left, right) = edges_of(Pane::Physics, dividers);
    let room = right - node_ceil(left) - LANES;
    let room = if room >= SIZE.0 as i32 {
        room
    } else {
        right - left - LANES
    };
    (SIZE.0 as i32).min(room).max(NARROWEST as i32)
}

/// Where a physics node put at `at` is drawn: inside the Physics pane, clear
/// of its heading, and left of the lanes the joins run down.
///
/// Brought back inside rather than refused, as a configuration's node is
/// (see `crate::pane::confine`): a node pushed at the edge moves along it,
/// and a position from a file written for wider panes still draws the node
/// where it can be seen and pressed.
///
/// # Arguments
///
/// * `at`       - Where it was put, its upper left corner, in the
///                coordinates the file holds.
/// * `dividers` - Where the panes are.
///
/// # Returns
///
/// Where it is drawn.
pub fn confine(at: (i32, i32), dividers: Dividers) -> (i32, i32) {
    let (left, right) = edges_of(Pane::Physics, dividers);
    let furthest = (right - LANES - width_in(dividers)).max(left);
    let top = node_ceil(NAMES.0 + NAMES.1);
    (at.0.clamp(left, furthest), at.1.max(top))
}

/// Where each component goes.
///
/// One the `tcsgui` section placed goes where it was put, brought inside the
/// pane by [`confine`]; one it did not goes down the pane, in name order, a
/// step apart, clear of the heading. Then, in that order, any that covers
/// one before it steps down until it is clear, as a configuration's nodes
/// do (see `crate::pane::settle`): of two in one place the first keeps it,
/// and none is drawn where it hides another.
///
/// # Arguments
///
/// * `placed`   - For each component, in the order drawn, where it was put,
///                if anywhere.
/// * `dividers` - Where the panes are, which says how wide the Physics pane
///                is.
///
/// # Returns
///
/// Each component's place.
pub fn layout(placed: &[Option<(i32, i32)>], dividers: Dividers) -> Vec<Place> {
    let (left, right) = edges_of(Pane::Physics, dividers);
    let width = width_in(dividers);
    let x = if right - node_ceil(left) - LANES >= SIZE.0 as i32 {
        node_ceil(left)
    } else {
        left
    };
    // One step of the node grid between one node and the next.
    let gap = (NODE_STEP_MM as f64 * PX_PER_MM).round() as i32;
    let size = (width as u32, SIZE.1);
    let mut y = node_ceil(NAMES.0 + NAMES.1);
    let mut places: Vec<Place> = placed
        .iter()
        .map(|placed| match placed {
            Some(at) => Place {
                at: confine(*at, dividers),
                size,
            },
            None => {
                let place = Place { at: (x, y), size };
                y = node_ceil(y + SIZE.1 as i32 + gap);
                place
            }
        })
        .collect();
    let covers = |a: (i32, i32), b: (i32, i32)| {
        a.0 < b.0 + width
            && b.0 < a.0 + width
            && a.1 < b.1 + SIZE.1 as i32
            && b.1 < a.1 + SIZE.1 as i32
    };
    for at in 1..places.len() {
        // A bound on the steps, as settle has, so that nothing a file says
        // can keep this going.
        for _ in 0..200 {
            let here = places[at].at;
            if !places[..at].iter().any(|other| covers(here, other.at)) {
                break;
            }
            places[at].at.1 = node_ceil(here.1 + 32);
        }
    }
    places
}

/// What a physics node looks like.
///
/// # Arguments
///
/// * `component` - The component.
/// * `at`        - Where its upper left corner is, in pixels.
/// * `size`      - Its width and height: [`SIZE`], or what [`layout`]
///                 makes it.
///
/// # Returns
///
/// Everything the markup needs to draw it.
pub fn node_look(
    component: &Component,
    at: (i32, i32),
    size: (u32, u32),
) -> NodeLook {
    let outline = style::Outline::Hexagon;
    let edge = style::Edge::Dotted;
    let none = TerminalLook {
        shown: false,
        x: 0.0,
        y: 0.0,
        rightward: false,
    };
    // Each terminal shown where the type has a terminal that way, its arrow
    // the way the value goes: out of the node rightwards, or into it.
    let terminal = |output: bool| {
        let (x, y) = terminal_at(size, output);
        TerminalLook {
            shown: has(component.kind, output),
            x: x as f32,
            y: y as f32,
            rightward: output,
        }
    };
    NodeLook {
        name: component.name.to_string(),
        node_type: component.kind.to_string(),
        x: at.0 as f32,
        y: at.1 as f32,
        width: size.0 as f32,
        height: size.1 as f32,
        outline: "hexagon",
        shape: style::shape_of(outline, size),
        edge: style::edge_of(outline, edge, size),
        inner: style::inner_of(outline, edge, size),
        border: 1.0,
        face: "mono",
        label_size: 13.0,
        label_weight: 700,
        label_italic: false,
        tint: if component.faulty {
            style::ERROR_TINT
        } else {
            PHYSICS_TINT
        }
        .to_string(),
        mark: crate::look::MARK,
        faulty: component.faulty,
        // Its type is said under its name, as every node's is: a physics
        // node's shape is the one shape for every type, so the words are
        // what tell a heater from a zone.
        summary: String::new(),
        openable: false,
        // The two on the right, in the two slots the markup draws on the
        // right, so the name is padded away from them; which slots says
        // nothing more, a physics node being joined only by its lines.
        gateway_in: none.clone(),
        gateway_out: terminal(true),
        payload_in: terminal(false),
        payload_out: none,
        terminal_column: (style::TERMINAL_COLUMN as i32
            + style::room_of(outline, size)) as f32,
        inset: style::room_of(outline, size) as f32,
    }
}

/// Whether a component of type `kind` has a terminal that way.
///
/// # Arguments
///
/// * `kind`   - The component's type.
/// * `output` - Whether the terminal asked about is an output.
///
/// # Returns
///
/// `true` where one of its type's terminals goes that way; `false` for a
/// type there is no such component of.
fn has(kind: &str, output: bool) -> bool {
    use tcslibgs::physics::terminal::Direction;
    let wanted = if output {
        Direction::Out
    } else {
        Direction::In
    };
    tcslibgs::physics::terminals_of(kind)
        .is_some_and(|all| all.iter().any(|t| t.direction == wanted))
}

/// Where a physics node's output or input terminal is drawn, from its upper
/// left corner.
///
/// At the right, the output in the top row and the input in the bottom
/// one, in from the right by however far the hexagon's side is in along the
/// terminal's rows.
///
/// # Arguments
///
/// * `size`   - The node's width and height.
/// * `output` - Whether it is the output.
///
/// # Returns
///
/// The terminal's upper left corner, in pixels from the node's.
pub fn terminal_at(size: (u32, u32), output: bool) -> (i32, i32) {
    let inset = style::TERMINAL_INSET as i32;
    let tall = style::TERMINAL.1 as i32;
    let y = if output {
        inset
    } else {
        size.1 as i32 - inset - tall
    };
    let room = style::hexagon_room(size.1, y, y + tall);
    (size.0 as i32 - inset - style::TERMINAL.0 as i32 - room, y)
}

/// Every physics node and link, as the markup draws them.
///
/// Each connection joins two components; each join joins a component to the
/// payload node that stands in for it, and is drawn only where that node is
/// on the page drawn. A connection or join naming a component the model has
/// not is drawn nowhere: what is wrong with it is said elsewhere, by the
/// checking, and a line to nowhere would be a guess about where it went.
///
/// # Arguments
///
/// * `components`  - The model's components, in the order they are laid out.
/// * `connections` - The model's connections, each from an output to an
///                   input, as the file writes them.
/// * `joins`       - Each stand-in's `connects`.
/// * `nodes`       - The configuration's nodes on the page drawn.
/// * `dividers`    - Where the panes are.
/// * `shift`       - What is added to every x, for a window that draws the
///                   sheet with the Physics pane at its own nought.
///
/// # Returns
///
/// The nodes, and the links.
pub fn looks(
    components: &[Component],
    connections: &[(&str, &str)],
    joins: &[Join],
    nodes: &[OnSheet],
    dividers: Dividers,
    shift: i32,
) -> (Vec<NodeLook>, Vec<LinkLook>) {
    let names: Vec<&str> = components.iter().map(|c| c.name).collect();
    let placed: Vec<Option<(i32, i32)>> =
        components.iter().map(|c| c.placed).collect();
    let places = layout(&placed, dividers);
    let place_of =
        |name: &str| names.iter().position(|n| *n == name).map(|i| places[i]);

    let drawn: Vec<NodeLook> = components
        .iter()
        .zip(&places)
        .map(|(component, place)| {
            node_look(component, (place.at.0 + shift, place.at.1), place.size)
        })
        .collect();

    // Where a line leaves or arrives at a component: the right of its
    // output or its input terminal, halfway down it.
    let end_at = |place: Place, output: bool| {
        let (x, y) = terminal_at(place.size, output);
        (
            (place.at.0 + shift + x + style::TERMINAL.0 as i32) as f32,
            (place.at.1 + y + style::TERMINAL.1 as i32 / 2) as f32,
        )
    };
    let mut links = Vec::new();
    // Each connection a loop out of its output's terminal, past the right of
    // the nodes, and back into its input's, a little further out each, so
    // that two do not run down one line.
    for (n, (from, to)) in connections.iter().enumerate() {
        let (Some((a, _)), Some((b, _))) =
            (from.split_once('.'), to.split_once('.'))
        else {
            continue;
        };
        let (Some(a_at), Some(b_at)) = (place_of(a), place_of(b)) else {
            continue;
        };
        let start = end_at(a_at, true);
        let end = end_at(b_at, false);
        let right = (a_at.at.0 + a_at.size.0 as i32 + shift) as f32;
        let out = right + 8.0 + 6.0 * (n % 3) as f32;
        let points = [start, (out, start.1), (out, end.1), end];
        links.push(link(&points));
    }

    // Each join between its component's terminal and the left of its
    // payload's node, the arrow the way the data goes: out of the output to
    // the stand-in that reports it, and from a stand-in into the input it
    // drives.
    for (n, join) in joins.iter().enumerate() {
        let Some((component, terminal)) = join.terminal.split_once('.') else {
            continue;
        };
        let Some(place) = place_of(component) else {
            continue;
        };
        let Some(node) = nodes
            .iter()
            .find(|node| node.here && node.name == join.payload)
        else {
            continue;
        };
        let kind = components
            .iter()
            .find(|c| c.name == component)
            .map(|c| c.kind)
            .unwrap_or_default();
        let input = tcslibgs::physics::terminals_of(kind)
            .and_then(|all| all.iter().find(|t| t.name == terminal))
            .is_some_and(|t| {
                t.direction == tcslibgs::physics::terminal::Direction::In
            });
        let (_, tall) = crate::pane::size_of(node.node_type);
        let mine = end_at(place, !input);
        let theirs = (
            (node.at.0 + shift) as f32,
            (node.at.1 + tall as i32 / 2) as f32,
        );
        // Down the gap between the Physics pane and the payloads, each a
        // little apart.
        let across = (shift as f32 - 6.0 - 4.0 * (n % 4) as f32).max(mine.0);
        let mut points =
            vec![mine, (across, mine.1), (across, theirs.1), theirs];
        if input {
            points.reverse();
        }
        links.push(link(&points));
    }
    (drawn, links)
}

/// One physics link along `points`.
///
/// # Arguments
///
/// * `points` - The route, from where the data leaves to where it arrives.
///
/// # Returns
///
/// The link, dashed, with its arrowhead at the last point, and no label:
/// the lanes between the Physics pane and the payloads are narrow, and a
/// terminal's name in each would be written over the next. What a link
/// joins is in the physics window, by name.
fn link(points: &[(f32, f32)]) -> LinkLook {
    let last = points[points.len() - 1];
    let before = points[points.len() - 2];
    let arrives_right = last.0 >= before.0;
    let ends = route::Ends {
        from: points[0],
        to: last,
        leaves_right: points[1].0 >= points[0].0,
        arrives_right,
    };
    let (label_x, label_y) = route::label_on(ends, points);
    LinkLook {
        ink: PHYSICS_LINE.to_string(),
        route: dashed(points),
        head: route::arrow(ends),
        marker: String::new(),
        label_x,
        label_y,
        wide: false,
        label: String::new(),
        comment: String::new(),
    }
}

/// A route drawn as dashes, as path data: a dash of eight pixels and a gap
/// of five, the pattern carried round each corner.
///
/// # Arguments
///
/// * `points` - The route's corners, in order.
///
/// # Returns
///
/// The path data, one `M`/`L` pair a dash.
pub fn dashed(points: &[(f32, f32)]) -> String {
    const DASH: f32 = 8.0;
    const GAP: f32 = 5.0;
    let mut out = String::new();
    // How far into the pattern the next run starts, so that a dash cut by a
    // corner goes on round it.
    let mut phase = 0.0f32;
    for pair in points.windows(2) {
        let ((x1, y1), (x2, y2)) = (pair[0], pair[1]);
        let span = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
        if span < 0.5 {
            continue;
        }
        let (ux, uy) = ((x2 - x1) / span, (y2 - y1) / span);
        let mut along = 0.0f32;
        while along < span {
            if phase < DASH {
                let end = (along + DASH - phase).min(span);
                out.push_str(&format!(
                    "M {:.1} {:.1} L {:.1} {:.1} ",
                    x1 + ux * along,
                    y1 + uy * along,
                    x1 + ux * end,
                    y1 + uy * end
                ));
                phase += end - along;
                along = end;
            } else {
                let end = (along + DASH + GAP - phase).min(span);
                phase += end - along;
                along = end;
            }
            if phase >= DASH + GAP {
                phase = 0.0;
            }
        }
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::default_dividers;

    /// The components go down the Physics pane in the order given, inside
    /// it, on the node grid, clear of its heading, and none on another.
    #[test]
    fn the_components_go_down_the_physics_pane() {
        let dividers = default_dividers();
        let places = layout(&[None, None, None], dividers);
        let (left, right) = edges_of(Pane::Physics, dividers);
        for (i, place) in places.iter().enumerate() {
            let (x, y) = place.at;
            assert_eq!(place.size, SIZE, "{places:?}");
            assert!(x >= left && x + SIZE.0 as i32 <= right, "{places:?}");
            assert!(y >= NAMES.0 + NAMES.1, "{places:?}");
            assert!(crate::pane::is_on_node_grid(y), "{places:?}");
            if i > 0 {
                assert!(y >= places[i - 1].at.1 + SIZE.1 as i32, "{places:?}");
            }
        }
        assert!(right <= 0, "the Physics pane is left of the payloads");
    }

    /// Heaters that warm a zone are drawn as nodes of their own, apart from
    /// the zone, with a line from each heater's output terminal into the
    /// zone's input terminal; and a join leaves or arrives at the terminal
    /// of the component it names.
    #[test]
    fn the_heaters_stand_apart_from_the_zone_they_warm() {
        let connections = [
            ("heater_1.heat", "zone.heat"),
            ("heater_2.heat", "zone.heat"),
        ];
        let dividers = default_dividers();
        let places = layout(&[None, None, None], dividers);
        let zone = places[2];
        for pair in places.windows(2) {
            assert!(
                pair[1].at.1 >= pair[0].at.1 + pair[0].size.1 as i32,
                "none on another: {places:?}"
            );
        }

        let component = |name, kind| Component {
            name,
            kind,
            faulty: false,
            placed: None,
        };
        let components = [
            component("heater_1", "heater"),
            component("heater_2", "heater"),
            component("zone", "thermal_zone"),
        ];
        let node = |name: &str, y| OnSheet {
            name: name.to_string(),
            node_type: "network",
            at: (34, y),
            here: true,
        };
        let nodes = [node("temp_1", 34), node("gpio-out-1", 200)];
        let joins = [
            Join {
                payload: "temp_1",
                terminal: "zone.temperature",
            },
            Join {
                payload: "gpio-out-1",
                terminal: "heater_1.on",
            },
        ];
        let (drawn, links) = looks(
            &components,
            &connections,
            &joins,
            &nodes,
            dividers,
            dividers.physics,
        );
        let order: Vec<&str> = drawn.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(order, ["heater_1", "heater_2", "zone"]);
        assert!(drawn.iter().all(|n| n.height == SIZE.1 as f32));
        // Two lines for the heat and two for the joins.
        assert_eq!(links.len(), 4);

        // The zone's temperature leaves by the zone's output terminal, and
        // heater_1's switching arrives at heater_1's input terminal: each at
        // the right of its terminal, halfway down it. A dashed line starts
        // with a dash, but may end in a gap, so the end is looked for within
        // a gap of the terminal, on its row.
        let point = |place: Place, output: bool| {
            let (x, y) = terminal_at(place.size, output);
            (
                (place.at.0 + dividers.physics + x) as f32
                    + style::TERMINAL.0 as f32,
                (place.at.1 + y) as f32 + style::TERMINAL.1 as f32 / 2.0,
            )
        };
        let parse = |said: &str| {
            let mut it = said.split(' ').map(|n| n.parse::<f32>().unwrap());
            (it.next().unwrap(), it.next().unwrap())
        };
        let first =
            |n: usize| parse(links[n].route[2..].split(" L ").next().unwrap());
        let ends = |n: usize, wanted: (f32, f32)| {
            let last = parse(links[n].route.rsplit(" L ").next().unwrap());
            assert!(
                last.1 == wanted.1
                    && (0.0..=5.0).contains(&(last.0 - wanted.0)),
                "ends at {last:?}, not at {wanted:?}: {}",
                links[n].route
            );
        };
        // Each heater's heat out of its output and into the zone's input.
        for (n, heater) in places[..2].iter().enumerate() {
            assert_eq!(first(n), point(*heater, true), "{}", links[n].route);
            ends(n, point(zone, false));
        }
        assert_eq!(first(2), point(zone, true), "{}", links[2].route);
        ends(3, point(places[0], false));
    }

    /// A physics node is a hexagon, the one outline no configuration's node
    /// has, with one output terminal and one input terminal, both inside it
    /// and clear of each other, and none of a kind its type has not.
    #[test]
    fn a_physics_node_is_a_hexagon_with_one_terminal_each_way() {
        let component = |kind| Component {
            name: "it",
            kind,
            faulty: false,
            placed: None,
        };
        for size in [SIZE, (super::NARROWEST, SIZE.1)] {
            let look = node_look(&component("heater"), (0, 0), size);
            assert_eq!(look.outline, "hexagon");
            let shown: Vec<&TerminalLook> = [
                &look.gateway_in,
                &look.gateway_out,
                &look.payload_in,
                &look.payload_out,
            ]
            .into_iter()
            .filter(|t| t.shown)
            .collect();
            assert_eq!(shown.len(), 2, "one each way: {look:?}");
            let (out, into) = (&look.gateway_out, &look.payload_in);
            assert!(out.rightward && !into.rightward);
            assert!(out.y + style::TERMINAL.1 as f32 <= into.y, "apart");
            // Both inside the hexagon at both of their corners on the right.
            let polygon = style::polygon_of(style::Outline::Hexagon, size);
            let half = size.1 as f32 / 2.0;
            for t in [out, into] {
                assert!(t.y + style::TERMINAL.1 as f32 <= size.1 as f32);
                let right = t.x + style::TERMINAL.0 as f32;
                for y in [t.y, t.y + style::TERMINAL.1 as f32] {
                    let edge =
                        polygon[2].0 - style::POINT * ((y - half).abs() / half);
                    assert!(right <= edge, "{t:?} pokes out at row {y}");
                }
            }
        }
        // A zone gives out its temperature and takes heat; a type there is
        // no such component of has neither.
        let zone = node_look(&component("thermal_zone"), (0, 0), SIZE);
        assert!(zone.gateway_out.shown && zone.payload_in.shown);
        let unknown = node_look(&component("nonesuch"), (0, 0), SIZE);
        assert!(!unknown.gateway_out.shown && !unknown.payload_in.shown);
        // And no node type is drawn as one.
        for kind in style::NodeType::ALL {
            assert_ne!(kind.appearance().outline, style::Outline::Hexagon);
        }
    }

    /// A component the `tcsgui` section placed is drawn where it was put,
    /// brought inside the pane when it was put outside it; one it did not
    /// place goes down the pane; and one put on top of another steps down
    /// until it is clear, the first keeping its place.
    #[test]
    fn a_placed_component_is_drawn_where_it_was_put() {
        let dividers = default_dividers();
        let (left, right) = edges_of(Pane::Physics, dividers);
        let top = node_ceil(NAMES.0 + NAMES.1);
        let put = (left + 4, 300);
        let places = layout(
            &[Some(put), None, Some((right + 500, -40)), Some(put)],
            dividers,
        );
        assert_eq!(places[0].at, put, "where it was put");
        assert_eq!(places[1].at.1, top, "the first unplaced, at the top");
        let pushed = places[2].at;
        assert!(pushed.1 >= top, "below the heading: {places:?}");
        assert!(
            pushed.0 + places[2].size.0 as i32 + LANES <= right,
            "left of the lanes: {places:?}"
        );
        // Brought in where confine brings it, and then down past the
        // unplaced one at the top, which was there first.
        let brought = confine((right + 500, -40), dividers);
        assert_eq!(brought.1, top);
        assert_eq!(pushed.0, brought.0);
        assert!(pushed.1 >= places[1].at.1 + SIZE.1 as i32, "{places:?}");
        assert_eq!(places[3].at.0, put.0);
        assert!(
            places[3].at.1 >= put.1 + SIZE.1 as i32,
            "the second in one place steps down: {places:?}"
        );

        // And what the file says reaches the drawing.
        let drawn = Drawn {
            components: vec![("zone".into(), "thermal_zone".into())],
            placed: [("zone".to_string(), put)].into_iter().collect(),
            ..Drawn::default()
        };
        let (nodes, _) = drawn.looks(&[], dividers, 0);
        assert_eq!((nodes[0].x, nodes[0].y), (put.0 as f32, put.1 as f32));
    }

    /// A narrow Physics pane keeps its nodes in it, with the joins' lanes
    /// beside them, rather than drawing them under the payloads; a wide one
    /// draws them at their full width.
    #[test]
    fn a_narrow_pane_narrows_its_nodes() {
        let least = crate::pane::WORKING_WIDTH * crate::settings::LEAST / 100;
        for physics in [least, 105, 192, 400] {
            let dividers = Dividers {
                physics,
                ..default_dividers()
            };
            let (left, right) = edges_of(Pane::Physics, dividers);
            let places = layout(&[None, None], dividers);
            for place in &places {
                let end = place.at.0 + place.size.0 as i32;
                assert!(place.at.0 >= left, "{physics}: {places:?}");
                assert!(end + LANES <= right, "{physics}: {places:?}");
                assert_eq!(
                    place.size.0 == SIZE.0,
                    right - left >= SIZE.0 as i32 + LANES + 8,
                    "{physics}: {places:?}"
                );
            }
        }
    }

    /// A connection is drawn between its two components and a join between
    /// a component and its payload's node, both dashed and in the Physics
    /// pane's line colour; a join to a node not on the page is not drawn.
    #[test]
    fn connections_and_joins_are_dashed_links() {
        let components = [
            Component {
                name: "heater_1",
                kind: "heater",
                faulty: false,
                placed: None,
            },
            Component {
                name: "zone",
                kind: "thermal_zone",
                faulty: true,
                placed: None,
            },
        ];
        let nodes = [OnSheet {
            name: "temp_1".to_string(),
            node_type: "network",
            at: (34, 34),
            here: true,
        }];
        let joins = [
            Join {
                payload: "temp_1",
                terminal: "zone.temperature",
            },
            Join {
                payload: "elsewhere",
                terminal: "heater_1.on",
            },
        ];
        let dividers = default_dividers();
        let (drawn, links) = looks(
            &components,
            &[("heater_1.heat", "zone.heat")],
            &joins,
            &nodes,
            dividers,
            dividers.physics,
        );
        assert_eq!(drawn.len(), 2);
        assert!(drawn[1].faulty && !drawn[0].faulty);
        assert_eq!(drawn[0].node_type, "heater");
        assert!(drawn.iter().all(|n| n.x >= 0.0), "shifted into view");
        assert_eq!(links.len(), 2, "the join to elsewhere is not drawn");
        for link in &links {
            assert_eq!(link.ink, PHYSICS_LINE);
            assert!(link.route.matches('M').count() > 2, "dashed");
        }
        // The zone's temperature is an output: its join arrives at the
        // payload, going right.
        assert!(links[1]
            .head
            .contains(&format!("{}", 34 + dividers.physics)));
    }

    /// Dashes go round corners, and an empty route draws nothing.
    #[test]
    fn a_dashed_route_keeps_its_pattern() {
        assert_eq!(dashed(&[(0.0, 0.0), (0.0, 0.0)]), "");
        let one = dashed(&[(0.0, 0.0), (26.0, 0.0)]);
        assert_eq!(one.matches('M').count(), 2, "{one}");
        let corner = dashed(&[(0.0, 0.0), (4.0, 0.0), (4.0, 20.0)]);
        assert!(corner.starts_with("M 0.0 0.0 L 4.0 0.0"), "{corner}");
        assert!(corner.contains("M 4.0 0.0 L 4.0 4.0"), "{corner}");
    }
}
