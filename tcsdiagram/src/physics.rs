//! How the simulator file's physics model is drawn: its components as nodes
//! in the Physics pane, its connections as links between them, and each
//! stand-in's `connects` as a link between its component and the payload
//! node it stands in for.
//!
//! The Physics pane is left of the payloads, at negative x in the
//! coordinates the configuration holds -- see `crate::pane` -- and its
//! nodes are laid out here, by name, top to bottom, rather than placed by
//! hand: a configuration file is where a node's position is kept, and a
//! physics component is never in one. So the drawing is the same every
//! time a model is opened, in the one order the model has.
//!
//! A physics node is the one shape no node of a configuration's has -- a
//! square outline with a dotted edge -- in the Physics pane's own reddish
//! purple, and labelled with its type as well as its name. A physics link
//! is dashed and drawn in that purple, where a configuration's links are
//! solid: two things, and not the colour alone, say it is the simulator's.

use crate::look::{PHYSICS_LINE, PHYSICS_TINT};
use crate::pane::{
    edges_of, node_ceil, node_floor, Dividers, Pane, NAMES, NODE_STEP_MM,
    PX_PER_MM,
};
use crate::view::{LinkLook, NodeLook, OnSheet, TerminalLook};
use crate::{route, style};

/// How big a physics node is drawn, in pixels: narrower than every node of
/// a configuration's, so that its size is a second thing that tells it
/// apart, and so that the links leaving it have room beside it in the
/// Physics pane.
pub const SIZE: (u32, u32) = (120, 44);

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

/// Where each component goes: down the Physics pane, in name order, a step
/// apart, clear of the pane's heading.
///
/// # Arguments
///
/// * `names`    - The components' names, in the order they are drawn.
/// * `dividers` - Where the panes are, which says how wide the Physics pane
///                is.
///
/// # Returns
///
/// Each name's upper left corner, in the coordinates the file holds.
pub fn layout(names: &[&str], dividers: Dividers) -> Vec<(i32, i32)> {
    let (left, right) = edges_of(Pane::Physics, dividers);
    let x = node_ceil(left).min(node_floor(right - SIZE.0 as i32));
    // One step of the node grid between one node and the next.
    let gap = (NODE_STEP_MM as f64 * PX_PER_MM).round() as i32;
    let mut y = node_ceil(NAMES.0 + NAMES.1);
    names
        .iter()
        .map(|_| {
            let at = (x.max(left), y);
            y = node_ceil(y + SIZE.1 as i32 + gap);
            at
        })
        .collect()
}

/// What a physics node looks like.
///
/// # Arguments
///
/// * `component` - The component.
/// * `at`        - Where its upper left corner is, in pixels.
///
/// # Returns
///
/// Everything the markup needs to draw it.
pub fn node_look(component: &Component, at: (i32, i32)) -> NodeLook {
    let outline = style::Outline::Square;
    let edge = style::Edge::Dotted;
    let none = TerminalLook {
        shown: false,
        x: 0.0,
        y: 0.0,
        rightward: false,
    };
    NodeLook {
        name: component.name.to_string(),
        node_type: component.kind.to_string(),
        x: at.0 as f32,
        y: at.1 as f32,
        width: SIZE.0 as f32,
        height: SIZE.1 as f32,
        outline: "square",
        shape: style::shape_of(outline, SIZE),
        edge: style::edge_of(outline, edge, SIZE),
        inner: style::inner_of(outline, edge, SIZE),
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
        gateway_in: none.clone(),
        gateway_out: none.clone(),
        payload_in: none.clone(),
        payload_out: none,
        terminal_column: 0.0,
        inset: 0.0,
    }
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
    let places = layout(&names, dividers);
    let at_of = |name: &str| {
        names
            .iter()
            .position(|n| *n == name)
            .map(|i| (places[i].0 + shift, places[i].1))
    };

    let drawn: Vec<NodeLook> = components
        .iter()
        .zip(&places)
        .map(|(component, at)| node_look(component, (at.0 + shift, at.1)))
        .collect();

    let (width, height) = (SIZE.0 as i32, SIZE.1 as i32);
    let mut links = Vec::new();
    // Each connection a loop out of the right of its output's node and back
    // into the right of its input's, a little further out each, so that two
    // do not run down one line.
    for (n, (from, to)) in connections.iter().enumerate() {
        let (Some((a, _)), Some((b, _))) =
            (from.split_once('.'), to.split_once('.'))
        else {
            continue;
        };
        let (Some(a_at), Some(b_at)) = (at_of(a), at_of(b)) else {
            continue;
        };
        let out = (a_at.0 + width) as f32 + 8.0 + 6.0 * (n % 3) as f32;
        let y1 = (a_at.1 + height / 3) as f32;
        let y2 = (b_at.1 + 2 * height / 3) as f32;
        let points = [
            ((a_at.0 + width) as f32, y1),
            (out, y1),
            (out, y2),
            ((b_at.0 + width) as f32, y2),
        ];
        links.push(link(&points));
    }

    // Each join from the right of its component's node to the left of its
    // payload's, the arrow the way the data goes: out of an output to the
    // stand-in that reports it, and from a stand-in into an input it drives.
    for (n, join) in joins.iter().enumerate() {
        let Some((component, terminal)) = join.terminal.split_once('.') else {
            continue;
        };
        let Some(at) = at_of(component) else {
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
        let mine = ((at.0 + width) as f32, (at.1 + height / 2) as f32);
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
        let places = layout(&["heater_1", "heater_2", "zone"], dividers);
        let (left, right) = edges_of(Pane::Physics, dividers);
        for (i, (x, y)) in places.iter().enumerate() {
            assert!(*x >= left && x + SIZE.0 as i32 <= right, "{places:?}");
            assert!(*y >= NAMES.0 + NAMES.1, "{places:?}");
            assert!(crate::pane::is_on_node_grid(*y), "{places:?}");
            if i > 0 {
                assert!(*y >= places[i - 1].1 + SIZE.1 as i32, "{places:?}");
            }
        }
        assert!(right <= 0, "the Physics pane is left of the payloads");
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
            },
            Component {
                name: "zone",
                kind: "thermal_zone",
                faulty: true,
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
