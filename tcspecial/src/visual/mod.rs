//! The visual window: what a running tcspecial is doing, drawn.
//!
//! The graph is drawn as tcsgui draws the configuration -- the three working
//! panes, every node where the editor would put it, every link along the
//! route the editor gives it, all of it from `tcsdiagram` -- and a link is lit
//! while data crosses it: a line three times as wide and a shade lighter is
//! drawn under it, for [`LINE_BLINK`] after each datum is sent. The ordinary
//! line stays on top, so the drawing is the editor's whether anything is
//! moving or not.
//!
//! **Never to be used in flight.** It is built only into a tcspecial compiled
//! with the `visual` feature, which no default build has, and asked for with
//! `--visual`. The window takes no press -- a node is not dragged, a terminal
//! draws no link -- and changes nothing the vehicle does: it keeps only what
//! the window manager gives every window, and its close box stops tcspecial,
//! as an interrupt would.
//!
//! What the vehicle does to be shown is one store per datum sent, into a slot
//! of a table made before the vehicle starts: see [`sent`]. It allocates
//! nothing, takes no lock and waits for nothing, so a window that is slow to
//! draw, or not drawn at all, cannot slow the vehicle. The window looks at the
//! table on a timer of its own, on the main thread, which is the one a window
//! has to run on; the vehicle runs beside it on a thread of its own.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use slint::{ComponentHandle, Model, ModelRc, VecModel};
use tcsdiagram::look::{self, ground_of};
use tcsdiagram::pane::{self, Placing, WORKING_HEIGHT, WORKING_WIDTH};
use tcsdiagram::style;
use tcsdiagram::view::{self, LineOnSheet, LinkLook, NodeLook, OnSheet};
use tcslibgs::graph::{Destination, Graph};

use crate::runtime;

mod ui {
    slint::include_modules!();
}

/// How long a link stays lit after data is sent over it.
///
/// A quarter of a second: long enough to be seen at a glance, and short
/// enough that a link carrying a reading a second is seen to blink once a
/// second rather than to be lit all the time.
pub const LINE_BLINK: Duration = Duration::from_millis(250);

/// How often the window looks at what has been sent.
///
/// Well under [`LINE_BLINK`], so that a link is lit for close to the time it
/// is meant to be and never missed: a datum is shown if the window looks at
/// any moment of the quarter second after it.
const LOOK_EVERY: Duration = Duration::from_millis(40);

/// When each line of the drawing last carried data.
struct Sent {
    /// What the times are counted from.
    since: Instant,
    /// One slot a line, in the order [`tcspecial::runtime::route`] numbers
    /// them: nanoseconds after `since`, plus one, so that nought is a line
    /// nothing has crossed yet.
    at: Box<[AtomicU64]>,
}

/// The table, made once before the vehicle starts, and only where the window
/// was asked for. Absent, [`sent`] does nothing.
static SENT: OnceLock<Sent> = OnceLock::new();

/// Records that data has been sent over this line of the drawing.
///
/// Called by the fabric for every delivery, in the main loop, so it is one
/// clock read and one atomic store: no allocation, no lock, nothing that
/// waits. A line the table has no slot for -- there is none -- is left alone.
///
/// # Arguments
///
/// * `line` - The line, as [`tcspecial::runtime::route`] numbers it.
pub fn sent(line: usize) {
    if let Some(sent) = SENT.get() {
        if let Some(slot) = sent.at.get(line) {
            let after = sent.since.elapsed().as_nanos();
            slot.store(
                u64::try_from(after).unwrap_or(u64::MAX - 1) + 1,
                Ordering::Relaxed,
            );
        }
    }
}

/// Says whether this line is lit at `now`: whether data crossed it less
/// than [`LINE_BLINK`] before.
///
/// # Arguments
///
/// * `line` - The line, as [`tcspecial::runtime::route`] numbers it.
/// * `now`  - The time to judge at.
///
/// # Returns
///
/// `true` if data crossed the line within [`LINE_BLINK`] before `now`;
/// `false` if not, if nothing has crossed it, or if there is no table.
fn is_lit(line: usize, now: Instant) -> bool {
    let Some(sent) = SENT.get() else {
        return false;
    };
    let Some(at) = sent.at.get(line).map(|slot| slot.load(Ordering::Relaxed))
    else {
        return false;
    };
    if at == 0 {
        return false;
    }
    let when = sent.since + Duration::from_nanos(at - 1);
    now.saturating_duration_since(when) < LINE_BLINK
}

/// Makes the table that [`sent`] records in, once per process.
///
/// # Arguments
///
/// * `lines` - How many lines the drawing has, one slot each.
fn watch(lines: usize) {
    let at: Box<[AtomicU64]> = (0..lines).map(|_| AtomicU64::new(0)).collect();
    // Made once: a second window in one process is not something this does.
    let _ = SENT.set(Sent {
        since: Instant::now(),
        at,
    });
}

/// One line of the configuration, as the drawing holds it: owned, because the
/// graph it was read from is the vehicle's.
struct Line {
    from: String,
    origin: Option<String>,
    to: String,
    input: Option<String>,
    comment: Option<String>,
}

/// Lists every line of the configuration, one per destination, in the
/// file's order: the order the route table numbers them in. See
/// `crate::runtime::route::Delivery::line`.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// The lines, owned.
fn lines_of(graph: &Graph) -> Vec<Line> {
    let mut lines = Vec::new();
    for link in &graph.links {
        let origin = link.origin();
        for destination in link.to.each() {
            let Destination { node, input } = Destination::parse(destination);
            lines.push(Line {
                from: origin.node.clone(),
                origin: origin.input.clone(),
                to: node,
                input,
                comment: link.comment.clone(),
            });
        }
    }
    lines
}

/// Runs a vehicle with the window open, and says what to exit with.
///
/// The vehicle on a thread of its own and the window on this one. Each ends
/// the other: a window closed stops the vehicle, as an interrupt would, and a
/// vehicle that stops -- or never starts -- closes the window.
///
/// # Arguments
///
/// * `path` - The vehicle's configuration file.
/// * `mode` - The `--simulate` mode to run it under.
///
/// # Returns
///
/// The exit status: the vehicle's, or 1 if the window or the vehicle's
/// thread cannot be started.
pub fn run(path: &str, mode: tcslibgs::simulate::Mode) -> i32 {
    // Read again rather than handed over: the vehicle reads its own copy,
    // and checked it before this was called.
    let graph = match Graph::load(path) {
        Ok(graph) => graph,
        // Said by the vehicle, which reads the same file and says why.
        Err(_) => return runtime::start::run_or_report(path, mode),
    };
    let lines = lines_of(&graph);
    watch(lines.len());

    let window = match ui::VisualWindow::new() {
        Ok(window) => window,
        Err(why) => {
            eprintln!("tcspecial --visual: the window cannot be opened: {why}");
            return 1;
        }
    };
    let shown = draw(&window, path, &graph, &lines);

    let flying = {
        let path = path.to_string();
        std::thread::Builder::new()
            .name("vehicle".to_string())
            .spawn(move || {
                let said = runtime::start::run_or_report(&path, mode);
                // The window goes with the vehicle. Asked of the window's
                // own thread, which is the one that may close it.
                let _ = slint::invoke_from_event_loop(|| {
                    let _ = slint::quit_event_loop();
                });
                said
            })
    };
    let flying = match flying {
        Ok(flying) => flying,
        Err(why) => {
            eprintln!(
                "tcspecial --visual: the vehicle's thread cannot be \
                       started: {why}"
            );
            return 1;
        }
    };

    // Each line lit or put out as the window looks. Only a line whose state
    // changed is told, so a quiet vehicle is a window that does no work.
    let timer = slint::Timer::default();
    {
        let links = shown.links.clone();
        let rows = shown.rows;
        timer.start(slint::TimerMode::Repeated, LOOK_EVERY, move || {
            let now = Instant::now();
            for (row, line) in rows.iter().enumerate() {
                let lit = is_lit(*line, now);
                if let Some(mut link) = links.row_data(row) {
                    if link.lit != lit {
                        link.lit = lit;
                        // An allocation in the window's loop that cannot be
                        // avoided: Slint copies the row and tells the view,
                        // and what it does inside to redraw is its own. The
                        // window's loop is not the vehicle's, and a tcspecial
                        // with a window does not fly.
                        links.set_row_data(row, link);
                    }
                }
            }
        });
    }

    if let Err(why) = window.run() {
        eprintln!("tcspecial --visual: the window stopped: {why}");
    }
    drop(timer);

    // Closed, so the vehicle stops; or the vehicle stopped, and this is
    // already so.
    runtime::start::stop_as_signalled();
    flying.join().unwrap_or(1)
}

/// What the window draws, and which line of the configuration each row of
/// its links is.
struct Shown {
    links: std::rc::Rc<VecModel<ui::LinkView>>,
    rows: Vec<usize>,
}

/// Puts the graph in the window, as tcsgui would draw its top page.
///
/// Where the panes are divided is the person's own tcsgui setting, read and
/// never written, and every node is settled into its pane as the editor
/// settles a file it opens -- so a node is drawn where the editor would draw
/// it. A node in an included file is on that file's page, not this one; a
/// line to it is drawn to a stub, as the editor draws it.
///
/// # Arguments
///
/// * `window` - The window to draw in.
/// * `path`   - The configuration file, which the window names.
/// * `graph`  - The configuration read from it.
/// * `lines`  - Its lines, from [`lines_of`].
///
/// # Returns
///
/// The links drawn, and which line of the configuration each row is.
fn draw(
    window: &ui::VisualWindow,
    path: &str,
    graph: &Graph,
    lines: &[Line],
) -> Shown {
    let settings = tcsdiagram::settings::read().settings;
    let dividers =
        pane::dividers_for(settings.moc_percent, settings.vehicle_percent);

    let top = graph.files.first();
    let here = |name: &str| graph.homes.get(name) == top || top.is_none();

    let mut placing: Vec<Placing> = graph
        .nodes
        .iter()
        .map(|node| Placing {
            node_type: &node.node_type,
            at: graph
                .placed
                .get(&node.name)
                .map_or((0, 0), |placed| (placed.x, placed.y)),
        })
        .collect();
    pane::settle(&mut placing, dividers);

    let sheet: Vec<OnSheet> = graph
        .nodes
        .iter()
        .zip(&placing)
        .map(|(node, placed)| OnSheet {
            name: node.name.clone(),
            node_type: &node.node_type,
            at: placed.at,
            here: here(&node.name),
        })
        .collect();

    let nodes: Vec<ui::NodeView> = sheet
        .iter()
        .filter(|node| node.here)
        .map(|node| {
            node_view(view::node_look(
                &node.name,
                node.node_type,
                node.at,
                false,
                None,
            ))
        })
        .collect();

    // The lines with an end on this page, and which line of the
    // configuration each is, so the window can light it.
    let (rows, on_page): (Vec<usize>, Vec<LineOnSheet>) = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| here(&line.from) || here(&line.to))
        .map(|(at, line)| {
            (
                at,
                LineOnSheet {
                    from: &line.from,
                    origin: line.origin.as_deref(),
                    to: &line.to,
                    input: line.input.as_deref(),
                    comment: line.comment.as_deref(),
                },
            )
        })
        .unzip();
    let links = std::rc::Rc::new(VecModel::from(
        view::link_looks(&sheet, &on_page)
            .into_iter()
            .map(link_view)
            .collect::<Vec<_>>(),
    ));

    window.global::<ui::Session>().set_read_only(true);
    window.set_configuration(path.into());
    window.set_nodes(ModelRc::new(VecModel::from(nodes)));
    window.set_links(ModelRc::from(links.clone()));
    window.set_working_width(WORKING_WIDTH as f32);
    window.set_working_height(WORKING_HEIGHT as f32);
    window.set_payload_divider(dividers.payloads as f32);
    window.set_vehicle_divider(dividers.vehicle as f32);
    window.set_payload_ground(colour(&ground_of(style::Ground::Payloads)));
    window.set_vehicle_ground(colour(&ground_of(style::Ground::Vehicle)));
    window.set_moc_ground(colour(&ground_of(style::Ground::Moc)));

    Shown { links, rows }
}

/// A hex colour as Slint's. See tcsdiagram::look::rgb.
fn colour(hex: &str) -> slint::Color {
    let (red, green, blue) = look::rgb(hex);
    slint::Color::from_rgb_u8(red, green, blue)
}

/// Turns a node's look into what this window's markup takes.
///
/// The same mapping tcsgui makes of the same look, into its own build's
/// struct: a Slint struct of one program is not a type of the other, so each
/// says it once. Never selected: nothing here can be.
///
/// # Arguments
///
/// * `look` - How tcsdiagram draws the node.
///
/// # Returns
///
/// The node as the window draws it.
fn node_view(look: NodeLook) -> ui::NodeView {
    let terminal = |look: view::TerminalLook| ui::TerminalView {
        shown: look.shown,
        x: look.x,
        y: look.y,
        rightward: look.rightward,
    };
    ui::NodeView {
        name: look.name.into(),
        node_type: look.node_type.into(),
        x: look.x,
        y: look.y,
        width: look.width,
        height: look.height,
        outline: look.outline.into(),
        shape: look.shape.into(),
        edge: look.edge.into(),
        inner: look.inner.into(),
        border: look.border,
        face: look.face.into(),
        label_size: look.label_size,
        label_weight: look.label_weight,
        label_italic: look.label_italic,
        tint: colour(&look.tint),
        mark: colour(look.mark),
        faulty: look.faulty,
        selected: false,
        summary: look.summary.into(),
        openable: look.openable,
        moc_in: terminal(look.moc_in),
        moc_out: terminal(look.moc_out),
        payload_in: terminal(look.payload_in),
        payload_out: terminal(look.payload_out),
        terminal_column: look.terminal_column,
        inset: look.inset,
    }
}

/// Turns a line's look into what this window's markup takes, put out
/// until data crosses it.
///
/// # Arguments
///
/// * `look` - How tcsdiagram draws the line.
///
/// # Returns
///
/// The line as the window draws it, not lit.
fn link_view(look: LinkLook) -> ui::LinkView {
    ui::LinkView {
        glow: colour(&look::lighter(&look.ink)),
        ink: colour(&look.ink),
        lit: false,
        route: look.route.into(),
        head: look.head.into(),
        marker: look.marker.into(),
        label_x: look.label_x,
        label_y: look.label_y,
        wide: look.wide,
        label: look.label.into(),
        comment: look.comment.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A line is lit for LINE_BLINK after data crosses it, and not before or
    /// after; a line nothing crossed is never lit.
    #[test]
    fn a_line_is_lit_for_line_blink_after_data_crosses_it() {
        watch(3);
        sent(1);
        let now = Instant::now();
        assert!(is_lit(1, now));
        assert!(!is_lit(0, now), "nothing crossed it");
        assert!(!is_lit(2, now), "nothing crossed it");
        assert!(!is_lit(1, now + LINE_BLINK), "put out after LINE_BLINK");
        // A line the table has no slot for is left alone.
        sent(99);
        assert!(!is_lit(99, now));
    }

    /// The lines are numbered as the route table numbers its deliveries, so
    /// that the line a delivery says it travelled is the line that is lit.
    #[test]
    fn the_lines_are_numbered_as_the_routes_number_them() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let graph =
            Graph::load(root.join("examples/thermostat/thermostat.yaml"))
                .expect("the example reads");
        let lines = lines_of(&graph);
        let routes = crate::runtime::route::Routes::of(&graph);
        let mut checked = 0;
        for node in &graph.nodes {
            for delivery in routes.from(&node.name) {
                let Some(line) = delivery.line else { continue };
                assert_eq!(lines[line].from, node.name, "line {line}");
                checked += 1;
            }
        }
        assert_eq!(checked, lines.len(), "every line is some delivery's");
    }

    /// Every line of the example is drawn: its nodes are all in one file, so
    /// all of them are on the page.
    #[test]
    fn every_line_of_a_configuration_in_one_file_is_drawn() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let graph =
            Graph::load(root.join("examples/thermostat/thermostat.yaml"))
                .expect("the example reads");
        let lines = lines_of(&graph);
        let top = graph.files.first();
        assert!(lines.iter().all(|line| graph.homes.get(&line.from) == top
            && graph.homes.get(&line.to) == top));
    }
}
