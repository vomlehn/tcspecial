//! The visual window: what a running tcspecial is doing, drawn.
//!
//! The graph is drawn as tcs-cfge draws the configuration -- the three working
//! panes, every node where the editor would put it, every link along the
//! route the editor gives it, all of it from `tcs-diagram` -- with the
//! simulator file's physical model in the Physical pane to their left, as the
//! editor draws it too. One window a page, as the editor has: the top file's,
//! and one for each subdiagram it includes, with its edge. A link is lit
//! while data crosses it: a line three times as wide and a shade lighter is
//! drawn under it, for [`LINE_BLINK`] after each datum is sent. The ordinary
//! line stays on top, so the drawing is the editor's whether anything is
//! moving or not.
//!
//! **Never to be used in flight.** It is built only into a tcspecial compiled
//! with the `visual` feature, which no default build has, and asked for with
//! `--visual`. The window takes no press -- a node is not dragged, a terminal
//! draws no link -- and changes nothing the vehicle does: it keeps only what
//! the window manager gives every window, and the close box of any of them
//! stops tcspecial, as an interrupt would: with no press to open a page
//! again, a page closed alone would be a page gone for the rest of the run.
//!
//! What the vehicle does to be shown is one store per datum sent, into a slot
//! of a table made before the vehicle starts: see [`sent`]. It allocates
//! nothing, takes no lock and waits for nothing, so a window that is slow to
//! draw, or not drawn at all, cannot slow the vehicle. The window looks at the
//! table on a timer of its own, on the main thread, which is the one a window
//! has to run on; the vehicle runs beside it on a thread of its own.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use tcsos::time::{Duration, Instant};

use slint::{ComponentHandle, Model, ModelRc, VecModel};
use tcs_diagram::look::{self, ground_of};
use tcs_diagram::pane::{self, Placing, WORKING_HEIGHT, WORKING_WIDTH};
use tcs_diagram::style;
use tcs_diagram::view::{
    self, Drawn, LineOnSheet, LinkLook, NodeLook, OnSheet,
};
use tcsprotocol::graph::{Destination, Graph};

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

/// Lists every line of the configuration, one per destination, in the order
/// the configuration holds them: the order the route table numbers them in.
/// See
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
/// * `path`    - The vehicle's configuration file.
/// * `mode`    - The `--simulate` mode to run it under.
/// * `checked` - That every check for test code has been made.
///
/// # Returns
///
/// The exit status: the vehicle's, or 1 if the window or the vehicle's
/// thread cannot be started.
pub fn run(
    path: &str,
    mode: tcsprotocol::simulate::Mode,
    checked: crate::flight_checks::Checked,
) -> i32 {
    // Read again rather than handed over: the vehicle reads its own copy,
    // and checked it before this was called.
    let graph = match Graph::load(path) {
        Ok(graph) => graph,
        // Said by the vehicle, which reads the same file and says why.
        Err(_) => return runtime::start::run_or_report(path, mode, checked),
    };
    let lines = lines_of(&graph);
    watch(lines.len());

    let settings = tcs_diagram::settings::read().settings;
    let mut dividers =
        pane::dividers_for(settings.gateway_percent, settings.vehicle_percent);
    dividers.physical = pane::physical_width(settings.physical_percent);
    let pages = Pages::of(&graph, dividers);

    // The top page first, then each subdiagram in the order the files are
    // read: a subdiagram's own after it, at any depth.
    let mut windows = Vec::new();
    let mut shown = Vec::new();
    for page in std::iter::once(None).chain(
        graph
            .includes
            .iter()
            .map(|include| Some(include.name.as_str())),
    ) {
        let window = match ui::VisualWindow::new() {
            Ok(window) => window,
            Err(why) => {
                eprintln!(
                    "tcspecial --visual: the window for {} cannot be opened: \
                     {why}",
                    page.unwrap_or(path)
                );
                return 1;
            }
        };
        shown.push(draw(&window, path, &graph, &lines, &pages, page));
        // Any window closed stops the vehicle, as the top one's does.
        window.window().on_close_requested(|| {
            let _ = slint::quit_event_loop();
            slint::CloseRequestResponse::HideWindow
        });
        if let Err(why) = window.show() {
            eprintln!(
                "tcspecial --visual: the window for {} cannot be shown: {why}",
                page.unwrap_or(path)
            );
            return 1;
        }
        windows.push(window);
    }

    let flying = {
        let path = path.to_string();
        tcsos::thread::Builder::new()
            .name("vehicle".to_string())
            .spawn(move || {
                let said = runtime::start::run_or_report(&path, mode, checked);
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

    // Each line lit or put out as the windows look, on every page it is
    // drawn on. Only a line whose state changed is told, so a quiet vehicle
    // is a window that does no work.
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, LOOK_EVERY, move || {
        let now = Instant::now();
        for page in &shown {
            for (row, line) in page.rows.iter().enumerate() {
                let lit = is_lit(*line, now);
                if let Some(mut link) = page.links.row_data(row) {
                    if link.lit != lit {
                        link.lit = lit;
                        // An allocation in the window's loop that cannot be
                        // avoided: Slint copies the row and tells the view,
                        // and what it does inside to redraw is its own. The
                        // window's loop is not the vehicle's, and a tcspecial
                        // with a window does not fly.
                        page.links.set_row_data(row, link);
                    }
                }
            }
        }
    });

    if let Err(why) = slint::run_event_loop() {
        eprintln!("tcspecial --visual: the window stopped: {why}");
    }
    drop(timer);
    drop(windows);

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

/// Which page each node of the configuration is on, and where each node and
/// each subdiagram's box is drawn: worked out once, for every window.
struct Pages {
    /// By node, the subdiagram whose file it was read from: `None` for the
    /// top file.
    page: BTreeMap<String, Option<String>>,
    /// By subdiagram, the one whose file includes it: `None` for one the top
    /// file includes. Subdiagrams nest, and a box is drawn on the page of
    /// the subdiagram that includes it.
    within: BTreeMap<String, Option<String>>,
    /// By node and by subdiagram, where it is drawn, settled into its pane.
    at: BTreeMap<String, (i32, i32)>,
    /// Where the panes are divided.
    dividers: pane::Dividers,
}

impl Pages {
    /// Works out the pages of a configuration.
    ///
    /// Where the panes are divided is the person's own tcs-cfge setting, read
    /// and never written, and every node and every subdiagram's box is
    /// settled into its pane as the editor settles a file it opens, all of
    /// them together and in the order of their names -- see tcs-cfge's
    /// `Document::settle` -- so each is drawn where the editor would draw
    /// it.
    ///
    /// # Arguments
    ///
    /// * `graph`    - The configuration.
    /// * `dividers` - Where the panes are divided.
    ///
    /// # Returns
    ///
    /// The pages.
    fn of(graph: &Graph, dividers: pane::Dividers) -> Pages {
        let page: BTreeMap<String, Option<String>> = graph
            .nodes
            .iter()
            .map(|node| {
                let file = graph.homes.get(&node.name);
                let page = file.and_then(|file| graph.include_name(file));
                (node.name.clone(), page)
            })
            .collect();

        let mut named: Vec<(&str, &str)> = graph
            .nodes
            .iter()
            .map(|node| (node.name.as_str(), node.node_type.as_str()))
            .chain(
                graph
                    .includes
                    .iter()
                    .map(|include| (include.name.as_str(), "subdiagram")),
            )
            .collect();
        named.sort();
        let mut placing: Vec<Placing> = named
            .iter()
            .map(|(name, node_type)| Placing {
                node_type,
                at: graph
                    .placed
                    .get(*name)
                    .map_or((0, 0), |placed| (placed.x, placed.y)),
            })
            .collect();
        pane::settle(&mut placing, dividers);
        let at = named
            .iter()
            .zip(&placing)
            .map(|((name, _), placed)| (name.to_string(), placed.at))
            .collect();

        let within = graph
            .includes
            .iter()
            .map(|include| (include.name.clone(), include.within.clone()))
            .collect();
        Pages {
            page,
            within,
            at,
            dividers,
        }
    }

    /// Says which pages a page is inside: itself, then the subdiagram that
    /// includes it, and so on out to the top page's.
    ///
    /// # Arguments
    ///
    /// * `page` - A subdiagram's name, or `None` for the top page.
    ///
    /// # Returns
    ///
    /// The subdiagrams' names, innermost first; none for the top page.
    fn around<'a>(&'a self, page: Option<&'a str>) -> Vec<&'a str> {
        let mut around = Vec::new();
        let mut at = page;
        while let Some(name) = at {
            around.push(name);
            // Each subdiagram is included once, and the reader refuses a
            // loop, so this ends at the top page.
            at = self
                .within
                .get_key_value(name)
                .and_then(|(_, within)| within.as_deref());
        }
        around
    }

    /// Says where a node is seen from a page: on it, inside one of its
    /// boxes, at any depth, or outside it altogether.
    ///
    /// # Arguments
    ///
    /// * `node` - The node's name.
    /// * `page` - The page it is seen from: a subdiagram's name, or `None`
    ///            for the top.
    ///
    /// # Returns
    ///
    /// Where it is drawn on that page.
    fn seen_on<'a>(&'a self, node: &str, page: Option<&str>) -> Seen<'a> {
        let home = self.of_node(node);
        if home == page {
            return Seen::Here;
        }
        let around = self.around(home);
        // The box it is in on this page is the subdiagram just inside the
        // page on the way out from the node's own: the outermost one, from
        // the top page.
        let boxed = match page {
            None => around.last().copied(),
            Some(page) => around
                .iter()
                .position(|name| *name == page)
                .and_then(|at| at.checked_sub(1))
                .map(|at| around[at]),
        };
        boxed.map_or(Seen::Outside, Seen::InBox)
    }

    /// Says whether a node is inside a subdiagram, at any depth.
    ///
    /// # Arguments
    ///
    /// * `node`       - The node's name.
    /// * `subdiagram` - The subdiagram's name.
    ///
    /// # Returns
    ///
    /// Whether the node's file is the subdiagram's or one inside it.
    fn is_inside(&self, node: &str, subdiagram: &str) -> bool {
        self.around(self.of_node(node)).contains(&subdiagram)
    }

    /// Says which page a node is on.
    ///
    /// # Arguments
    ///
    /// * `node` - The node's name.
    ///
    /// # Returns
    ///
    /// Its subdiagram's name, or `None` for the top page.
    fn of_node(&self, node: &str) -> Option<&str> {
        self.page.get(node).and_then(|page| page.as_deref())
    }

    /// Says where a node or a subdiagram's box is drawn.
    fn at(&self, name: &str) -> (i32, i32) {
        self.at.get(name).copied().unwrap_or_default()
    }

    /// Says whether a line is drawn on a page: one with an end there, and
    /// one between two of its boxes or between a box and the page's edge,
    /// drawn between them as the editor draws it; not one between two nodes
    /// inside one box, nor one wholly outside the page.
    ///
    /// # Arguments
    ///
    /// * `line` - The line.
    /// * `page` - The page: a subdiagram's name, or `None` for the top.
    ///
    /// # Returns
    ///
    /// Whether it is drawn there.
    fn shows(&self, line: &Line, page: Option<&str>) -> bool {
        match (self.seen_on(&line.from, page), self.seen_on(&line.to, page)) {
            (Seen::Here, _) | (_, Seen::Here) => true,
            (Seen::Outside, Seen::Outside) => false,
            (from, to) => from != to,
        }
    }
}

/// Where a node is drawn, seen from a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seen<'a> {
    /// On the page itself.
    Here,
    /// Inside the box of the subdiagram named, which is on the page; the
    /// node may be in it or in a subdiagram inside it.
    InBox(&'a str),
    /// Outside the page altogether, drawn at its edge.
    Outside,
}

/// Puts one page of the graph in a window, as tcs-cfge would draw it.
///
/// A node on another page is drawn as the editor draws it: inside a box on
/// this page, at the box, however deep inside it is; outside this page, at
/// the page's edge. Each box on the page says what is inside it, as the
/// editor's does.
///
/// # Arguments
///
/// * `window` - The window to draw in.
/// * `path`   - The configuration file, which the window names.
/// * `graph`  - The configuration read from it.
/// * `lines`  - Its lines, from [`lines_of`].
/// * `pages`  - Which page each node is on, and where it is.
/// * `page`   - The page drawn: a subdiagram's name, or `None` for the top.
///
/// # Returns
///
/// The links drawn, and which line of the configuration each row is.
fn draw(
    window: &ui::VisualWindow,
    path: &str,
    graph: &Graph,
    lines: &[Line],
    pages: &Pages,
    page: Option<&str>,
) -> Shown {
    let dividers = pages.dividers;
    let inside = pane::inside_of(dividers);

    let mut sheet: Vec<OnSheet> = graph
        .nodes
        .iter()
        .map(|node| match pages.seen_on(&node.name, page) {
            // A node inside a box on this page, however deep, is drawn at
            // the box, whose terminals are the subdiagram's edge.
            Seen::InBox(boxed) => OnSheet {
                name: node.name.clone(),
                node_type: "subdiagram",
                at: pages.at(boxed),
                here: true,
                drawn: Drawn::AtItsBox,
            },
            seen => OnSheet {
                name: node.name.clone(),
                node_type: &node.node_type,
                at: pages.at(&node.name),
                here: seen == Seen::Here,
                // And a node outside this page at the page's own edge.
                drawn: if seen == Seen::Here {
                    Drawn::Itself
                } else {
                    Drawn::AtTheEdge(inside.0, inside.1)
                },
            },
        })
        .collect();
    let mut nodes: Vec<ui::NodeView> = sheet
        .iter()
        .zip(&graph.nodes)
        .filter(|(node, _)| node.here && node.drawn == Drawn::Itself)
        .map(|(node, said)| {
            // A compute node's type line names its function.
            node_view(view::node_look(
                &node.name,
                node.node_type,
                said.attributes.get("function").map(|f| f.as_str()),
                node.at,
                false,
                None,
            ))
        })
        .collect();
    // The boxes of the subdiagrams this page's file includes, each counting
    // what is inside it at any depth, and the lines crossing its edge.
    for include in &graph.includes {
        if include.within.as_deref() != page {
            continue;
        }
        let name = include.name.as_str();
        let inside =
            pages.page.keys().filter(|node| pages.is_inside(node, name));
        let crossing = lines.iter().filter(|line| {
            pages.is_inside(&line.from, name) != pages.is_inside(&line.to, name)
        });
        let summary = view::box_summary(inside.count(), crossing.count());
        nodes.push(node_view(view::node_look(
            name,
            "subdiagram",
            None,
            pages.at(name),
            false,
            Some(&summary),
        )));
        sheet.push(OnSheet {
            name: name.to_string(),
            node_type: "subdiagram",
            at: pages.at(name),
            here: true,
            drawn: Drawn::Itself,
        });
    }

    let edge = if page.is_some() {
        style::EDGE as i32
    } else {
        0
    };
    draw_physical(window, path, &sheet, dividers, page, edge);

    // The lines drawn on this page, and which line of the configuration
    // each is, so the window can light it.
    let (rows, on_page): (Vec<usize>, Vec<LineOnSheet>) = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| pages.shows(line, page))
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
    window.set_page(page.unwrap_or_default().into());
    window.set_edge_width(edge as f32);
    if page.is_some() {
        let inside = (inside.0 as f32, inside.1 as f32);
        let terminals: Vec<ui::EdgeView> = style::edge_terminals(inside)
            .into_iter()
            .map(|edge| ui::EdgeView {
                terminal: edge.terminal.into(),
                x: edge.x,
                y: edge.y,
                rightward: edge.rightward,
            })
            .collect();
        window.set_edge_terminals(ModelRc::new(VecModel::from(terminals)));
    }
    window.set_nodes(ModelRc::new(VecModel::from(nodes)));
    window.set_links(ModelRc::from(links.clone()));
    window.set_working_width(WORKING_WIDTH as f32);
    window.set_working_height(WORKING_HEIGHT as f32);
    window.set_payload_divider(dividers.payloads as f32);
    window.set_vehicle_divider(dividers.vehicle as f32);
    window.set_payload_ground(colour(&ground_of(style::Ground::Payloads)));
    window.set_vehicle_ground(colour(&ground_of(style::Ground::Vehicle)));
    window.set_gateway_ground(colour(&ground_of(style::Ground::Gateway)));

    Shown { links, rows }
}

/// Puts the simulator file's physical model in the window's Physical pane.
///
/// The simulator file is the one tcs-sim and tcs-moc would read for this
/// configuration -- the one `PAYLOAD_SIM_YAML` names, and otherwise the one
/// named for it beside it -- read here for the drawing and for nothing else:
/// the vehicle never reads it, and what it flies is not changed by it. With
/// none, the Physical pane is drawn empty. With one that cannot be read, the
/// pane is drawn empty and why is said: the window is to watch a vehicle by,
/// and a simulator file tcs-sim would refuse is tcs-sim's to refuse.
///
/// # Arguments
///
/// * `window`   - The window to draw in.
/// * `path`     - The configuration file.
/// * `sheet`    - The configuration's nodes, where they are drawn, which the
///                simulated payloads' links go to.
/// * `dividers` - Where the panes are, the Physical pane's width included.
/// * `page`     - The page drawn, whose components are drawn: a
///                subdiagram's name, or `None` for the top.
/// * `edge`     - How wide the page's edge is, left of the Physical pane.
fn draw_physical(
    window: &ui::VisualWindow,
    path: &str,
    sheet: &[OnSheet],
    dividers: pane::Dividers,
    page: Option<&str>,
    edge: i32,
) {
    window.set_physical_width(dividers.physical as f32);
    window.set_physical_ground(colour(&ground_of(style::Ground::Physical)));

    let sim = tcsos::env::var("PAYLOAD_SIM_YAML")
        .ok()
        .filter(|named| !named.is_empty())
        .or_else(|| tcsprotocol::config::sim_path_beside(path));
    let Some(sim) = sim else {
        return;
    };
    let file = match tcsprotocol::sim_config::SimConfigFile::load(&sim) {
        Ok(file) => file,
        Err(why) => {
            eprintln!(
                "tcspecial --visual: the Physical pane is drawn empty, as \
                 {sim} cannot be read: {why}"
            );
            return;
        }
    };
    let drawn = tcs_diagram::physical::of_simulator(&file, page);
    // Drawn from where the working panes begin, past the edge and the
    // Physical pane.
    let (nodes, links) = drawn.looks(sheet, dividers, dividers.physical + edge);
    window.set_physical_nodes(ModelRc::new(VecModel::from(
        nodes.into_iter().map(node_view).collect::<Vec<_>>(),
    )));
    window.set_physical_links(ModelRc::new(VecModel::from(
        links.into_iter().map(link_view).collect::<Vec<_>>(),
    )));
}

/// A hex colour as Slint's. See tcs_diagram::look::rgb.
fn colour(hex: &str) -> slint::Color {
    let (red, green, blue) = look::rgb(hex);
    slint::Color::from_rgb_u8(red, green, blue)
}

/// Turns a node's look into what this window's markup takes.
///
/// The same mapping tcs-cfge makes of the same look, into its own build's
/// struct: a Slint struct of one program is not a type of the other, so each
/// says it once. Never selected: nothing here can be.
///
/// # Arguments
///
/// * `look` - How tcs-diagram draws the node.
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
        type_line: look.type_line.into(),
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
        world_in: terminal(look.world_in),
        world_out: terminal(look.world_out),
        terminal_column: look.terminal_column,
        inset: look.inset,
        terminal_row: look.terminal_row,
        type_left: look.type_band.0,
        type_right: look.type_band.1,
        type_size: tcs_diagram::style::TYPE_SIZE as f32,
    }
}

/// Turns a line's look into what this window's markup takes, put out
/// until data crosses it.
///
/// # Arguments
///
/// * `look` - How tcs-diagram draws the line.
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
        tip: look.tip.into(),
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
            Graph::load(root.join("examples/thermostat/thermostat.ytcs"))
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

    /// Every line of the worked example is drawn on the top page: its nodes
    /// are all in one file, so there is no other page.
    #[test]
    fn every_line_of_a_configuration_in_one_file_is_drawn() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let graph = Graph::load(root.join("tests/manual/thermostat.ytcs"))
            .expect("the example reads");
        assert_eq!(graph.files.len(), 1, "it is one file");
        let pages = Pages::of(&graph, pane::default_dividers());
        assert!(lines_of(&graph).iter().all(|line| pages.shows(line, None)));
    }

    /// A configuration that includes a file is drawn on a page for each: a
    /// line inside the subdiagram on its page and not the top one, a line
    /// crossing its edge on both, and every line on some page. Each node is
    /// on the page of the file it was read from.
    #[test]
    fn each_subdiagram_has_a_page_of_its_own() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let graph =
            Graph::load(root.join("examples/thermostat/thermostat.ytcs"))
                .expect("the example reads");
        let pages = Pages::of(&graph, pane::default_dividers());
        let sub = Some("thermostat-common");
        assert_eq!(pages.of_node("gateway"), None);
        assert_eq!(pages.of_node("thermostat"), sub);

        let lines = lines_of(&graph);
        let mut inside = 0;
        let mut crossing = 0;
        for line in &lines {
            let ends = (pages.of_node(&line.from), pages.of_node(&line.to));
            let (top, there) =
                (pages.shows(line, None), pages.shows(line, sub));
            assert!(top || there, "{} to {} is drawn", line.from, line.to);
            if ends == (sub, sub) {
                assert!(there && !top, "{} to {}", line.from, line.to);
                inside += 1;
            } else if ends.0 != ends.1 {
                assert!(there && top, "{} to {}", line.from, line.to);
                crossing += 1;
            }
        }
        assert!(inside > 0 && crossing > 0, "{inside} in, {crossing} across");
        // And the box is placed, in the vehicle's panes or the payloads'.
        assert_ne!(pages.at("thermostat-common"), (0, 0));
    }

    /// Subdiagrams nest, and each page shows its own nodes, its own boxes
    /// and its edge: from the top page, a node two subdiagrams deep is at
    /// the outer box; from `zone`'s page, at `heater`'s box; from
    /// `heater`'s, itself. A line is drawn on a page where its ends are
    /// drawn apart, so the one inside `zone`, from `heater` to `zone`'s own
    /// node, is on `zone`'s page and `heater`'s and not the top one, where
    /// both its ends are in `zone`'s box.
    #[test]
    fn a_nested_subdiagram_is_drawn_at_each_page_around_it() {
        let where_ = tempfile::tempdir().expect("a directory");
        let dir = where_.path();
        let top = dir.join("vehicle.ytcs");
        tcsos::fs::write(
            &top,
            "include:\n  - subdiagram: zone\n    path: zone\n\
             gateways:\n  - type gateway_radio:\n      \
             - gateway: ground\n        world-in: [temp]\n\
             payloads:\n  - type i2c:\n      - payload: sensor\n        \
             world-out:\n          - temp.sensor\n",
        )
        .unwrap();
        tcsos::fs::write(
            dir.join("zone.ytcs"),
            "include:\n  - subdiagram: heater\n    path: heater\n\
             vehicle:\n  - type compute:\n      - compute: scale\n        \
             function: \"to_kelvin\"\n        world-in: [temp]\n",
        )
        .unwrap();
        tcsos::fs::write(
            dir.join("heater.ytcs"),
            "vehicle:\n  - type compute:\n      - compute: temp\n        \
             function: \"to_kelvin\"\n        world-in: [sensor]\n        \
             world-out:\n          - ground.temp\n          - scale.temp\n",
        )
        .unwrap();
        let graph = Graph::load(&top).expect("it reads");
        let pages = Pages::of(&graph, pane::default_dividers());
        let (zone, heater) = (Some("zone"), Some("heater"));

        assert_eq!(pages.seen_on("temp", None), Seen::InBox("zone"));
        assert_eq!(pages.seen_on("temp", zone), Seen::InBox("heater"));
        assert_eq!(pages.seen_on("temp", heater), Seen::Here);
        assert_eq!(pages.seen_on("scale", None), Seen::InBox("zone"));
        assert_eq!(pages.seen_on("scale", heater), Seen::Outside);
        assert_eq!(pages.seen_on("sensor", heater), Seen::Outside);
        assert!(pages.is_inside("temp", "zone"));
        assert!(!pages.is_inside("scale", "heater"));

        let lines = lines_of(&graph);
        let line = |from: &str, to: &str| {
            lines
                .iter()
                .find(|l| l.from == from && l.to == to)
                .unwrap_or_else(|| panic!("{from} to {to}"))
        };
        let on =
            |l: &Line| [None, zone, heater].map(|page| pages.shows(l, page));
        assert_eq!(on(line("sensor", "temp")), [true, true, true]);
        assert_eq!(on(line("temp", "ground")), [true, true, true]);
        assert_eq!(on(line("temp", "scale")), [false, true, true]);
    }
}
