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
//! A component whose heat, or whatever else it puts out, all goes into one
//! other component is drawn inside that one, as a heater is inside the zone
//! it warms: the two are one body on the vehicle, and what joins the
//! sensors and the heaters' lines to them is drawn to one block, not
//! spread over several. The connection between them is the nesting, and is
//! not drawn again as a line.
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

/// How far a component drawn inside another is in from that one's sides,
/// and how far below the one above it, in pixels.
pub const INSIDE: u32 = 8;

/// Where one component is drawn, in the coordinates the file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    /// Its upper left corner.
    pub at: (i32, i32),
    /// Its width and height.
    pub size: (u32, u32),
    /// The right edge of the block it is drawn in: its own, or the right
    /// edge of the component it is inside. Every line to the component
    /// leaves the block from there.
    pub right: i32,
}

/// Which component each is drawn inside, if any.
///
/// One whose every connection goes from it into one other component, which
/// itself goes into nothing, is drawn inside that one: a heater inside the
/// zone it warms. Only one deep, so that a block is never a nest of blocks
/// that a narrow pane could not hold; anything else stands alone.
///
/// # Arguments
///
/// * `names`       - The components' names.
/// * `connections` - The model's connections, from and to, as the file
///                   writes them.
///
/// # Returns
///
/// For each name, the index in `names` of the one it is drawn inside.
pub fn hosts_of(
    names: &[&str],
    connections: &[(&str, &str)],
) -> Vec<Option<usize>> {
    let component = |end: &str| {
        let name = end.split_once('.').map_or(end, |(name, _)| name);
        names.iter().position(|n| *n == name)
    };
    // Where each component's connections go, by index; None for one that
    // goes nowhere, and a component's own index for one whose connections
    // go to more than one.
    let mut into: Vec<Option<usize>> = vec![None; names.len()];
    for (from, to) in connections {
        let (Some(a), Some(b)) = (component(from), component(to)) else {
            continue;
        };
        into[a] = match into[a] {
            None => Some(b),
            Some(was) if was == b => Some(b),
            Some(_) => Some(a),
        };
    }
    (0..names.len())
        .map(|i| match into[i] {
            Some(host) if host != i && into[host].is_none() => Some(host),
            _ => None,
        })
        .collect()
}

/// Where each component goes: down the Physics pane, in name order, a step
/// apart, clear of the pane's heading, and each drawn inside another under
/// that one's name.
///
/// # Arguments
///
/// * `names`    - The components' names, in the order they are drawn.
/// * `hosts`    - Which each is drawn inside: see [`hosts_of`].
/// * `dividers` - Where the panes are, which says how wide the Physics pane
///                is.
///
/// # Returns
///
/// Each name's place.
pub fn layout(
    names: &[&str],
    hosts: &[Option<usize>],
    dividers: Dividers,
) -> Vec<Place> {
    let (left, right) = edges_of(Pane::Physics, dividers);
    let x = node_ceil(left)
        .min(node_floor(right - SIZE.0 as i32))
        .max(left);
    // One step of the node grid between one block and the next.
    let gap = (NODE_STEP_MM as f64 * PX_PER_MM).round() as i32;
    let (width, height, inside) = (SIZE.0 as i32, SIZE.1 as i32, INSIDE as i32);
    let mut places = vec![
        Place {
            at: (x, 0),
            size: SIZE,
            right: x + width,
        };
        names.len()
    ];
    let mut y = node_ceil(NAMES.0 + NAMES.1);
    for host in 0..names.len() {
        if hosts[host].is_some() {
            continue;
        }
        let guests: Vec<usize> = (0..names.len())
            .filter(|&g| hosts[g] == Some(host))
            .collect();
        for (n, &guest) in guests.iter().enumerate() {
            places[guest].at =
                (x + inside, y + height + n as i32 * (height + inside));
            places[guest].size = (SIZE.0 - 2 * INSIDE, SIZE.1);
        }
        let tall = height + guests.len() as i32 * (height + inside);
        places[host].at = (x, y);
        places[host].size = (SIZE.0, tall as u32);
        y = node_ceil(y + tall + gap);
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
        width: size.0 as f32,
        height: size.1 as f32,
        outline: "square",
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
        gateway_in: none.clone(),
        gateway_out: none.clone(),
        payload_in: none.clone(),
        payload_out: none,
        terminal_column: 0.0,
        inset: 0.0,
        // A block with others drawn inside it is named at its top, above
        // them, where its name is not covered.
        label_top: size.1 > SIZE.1,
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
    let hosts = hosts_of(&names, connections);
    let places = layout(&names, &hosts, dividers);
    let place_of =
        |name: &str| names.iter().position(|n| *n == name).map(|i| places[i]);

    // Each block before what is drawn inside it, so that what is inside is
    // drawn over it and is what a press on it reaches.
    let mut drawn: Vec<NodeLook> = Vec::new();
    for i in 0..components.len() {
        if hosts[i].is_some() {
            continue;
        }
        for j in std::iter::once(i)
            .chain((0..components.len()).filter(|&g| hosts[g] == Some(i)))
        {
            let place = places[j];
            drawn.push(node_look(
                &components[j],
                (place.at.0 + shift, place.at.1),
                place.size,
            ));
        }
    }

    let height = SIZE.1 as i32;
    // Where a line to a component leaves its block: the block's right
    // edge, level with the component's own name.
    let side_of = |place: Place, at: i32| {
        ((place.right + shift) as f32, (place.at.1 + at) as f32)
    };
    let mut links = Vec::new();
    // Each connection a loop out of the right of its output's block and back
    // into the right of its input's, a little further out each, so that two
    // do not run down one line. One into the block it is drawn inside is
    // that nesting, and not drawn again.
    for (n, (from, to)) in connections.iter().enumerate() {
        let (Some((a, _)), Some((b, _))) =
            (from.split_once('.'), to.split_once('.'))
        else {
            continue;
        };
        let (Some(a_at), Some(b_at)) = (place_of(a), place_of(b)) else {
            continue;
        };
        let (Some(i), Some(j)) = (
            names.iter().position(|n| *n == a),
            names.iter().position(|n| *n == b),
        ) else {
            continue;
        };
        if hosts[i] == Some(j) {
            continue;
        }
        let start = side_of(a_at, height / 3);
        let end = side_of(b_at, 2 * height / 3);
        let out = start.0.max(end.0) + 8.0 + 6.0 * (n % 3) as f32;
        let points = [start, (out, start.1), (out, end.1), end];
        links.push(link(&points));
    }

    // Each join from the right of its component's block to the left of its
    // payload's, the arrow the way the data goes: out of an output to the
    // stand-in that reports it, and from a stand-in into an input it drives.
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
        let mine = side_of(place, height / 2);
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
        let places = layout(
            &["heater_1", "heater_2", "zone"],
            &[None, None, None],
            dividers,
        );
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

    /// Heaters that warm a zone are drawn inside the zone's block, under its
    /// name, and every line to them leaves that one block: the sensors and
    /// the heaters' lines are joined to one thermal mass, not to three
    /// blocks.
    #[test]
    fn the_heaters_are_drawn_inside_the_zone_they_warm() {
        let names = ["heater_1", "heater_2", "zone"];
        let connections = [
            ("heater_1.heat", "zone.heat"),
            ("heater_2.heat", "zone.heat"),
        ];
        let hosts = hosts_of(&names, &connections);
        assert_eq!(hosts, [Some(2), Some(2), None]);

        let dividers = default_dividers();
        let places = layout(&names, &hosts, dividers);
        let zone = places[2];
        // Right and bottom edges.
        let end =
            |p: &Place| (p.at.0 + p.size.0 as i32, p.at.1 + p.size.1 as i32);
        for heater in &places[..2] {
            assert!(
                heater.at.0 > zone.at.0
                    && end(heater).0 < end(&zone).0
                    && heater.at.1 >= zone.at.1 + SIZE.1 as i32
                    && end(heater).1 < end(&zone).1,
                "inside the zone, under its name: {places:?}"
            );
            assert_eq!(heater.right, zone.right);
        }
        assert!(places[1].at.1 >= places[0].at.1 + SIZE.1 as i32);

        let component = |name, kind| Component {
            name,
            kind,
            faulty: false,
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
        // The zone first, so that the heaters are drawn over it and a press
        // on one reaches the heater.
        let order: Vec<&str> = drawn.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(order, ["zone", "heater_1", "heater_2"]);
        assert!(drawn[0].label_top && !drawn[1].label_top);
        // No line for the heat: the nesting says it.
        assert_eq!(links.len(), 2, "only the two joins are drawn");
        let edge = format!("{:.1}", (zone.right + dividers.physics) as f32);
        for link in &links {
            assert!(
                link.route.contains(&edge),
                "every join leaves the zone's block at {edge}: {}",
                link.route
            );
        }
    }

    /// A component whose connections go to two others, or into one that
    /// goes on into another, stands alone: a block is one deep.
    #[test]
    fn only_a_component_feeding_one_end_is_drawn_inside_it() {
        let names = ["a", "b", "c"];
        assert_eq!(
            hosts_of(&names, &[("a.out", "b.in"), ("a.out", "c.in")]),
            [None, None, None]
        );
        assert_eq!(
            hosts_of(&names, &[("a.out", "b.in"), ("b.out", "c.in")]),
            [None, Some(2), None]
        );
    }

    /// A join is drawn between a component and its payload's node, dashed
    /// and in the Physics pane's line colour; a join to a node not on the
    /// page is not drawn.
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
        // The heater is inside the zone, drawn after it.
        assert!(drawn[0].faulty && !drawn[1].faulty);
        assert_eq!(drawn[1].node_type, "heater");
        assert!(drawn.iter().all(|n| n.x >= 0.0), "shifted into view");
        assert_eq!(
            links.len(),
            1,
            "the join to elsewhere is not drawn, nor the heat into the \
             zone the heater is drawn inside"
        );
        for link in &links {
            assert_eq!(link.ink, PHYSICS_LINE);
            assert!(link.route.matches('M').count() > 2, "dashed");
        }
        // The zone's temperature is an output: its join arrives at the
        // payload, going right.
        assert!(links[0]
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
