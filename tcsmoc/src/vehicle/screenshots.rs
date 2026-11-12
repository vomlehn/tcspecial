//! The screenshots in the tcsmoc manual, made from the window itself.
//!
//! Each picture in `docs/tcsmoc.rst` is this window, drawn by Slint's
//! software renderer into memory and written to `docs/shots/tcsmoc-*.png`.
//! Nothing is put on a screen: the platform below hands every window a
//! buffer of its own, and nothing else.
//!
//! The window is driven into each state by its own code -- the functions
//! `run` wires together, the beacon watch on its own thread, the receivers
//! that fill the panels and the plot -- and fed what it would be sent: real
//! beacons, real command answers and real data, from a simulated vehicle on
//! loopback ports this test chooses, so that nothing else's ports are
//! touched. Where `run` wires a closure to its window, this does the same
//! wiring, and where `run`'s timer repaints the panels, this calls what the
//! timer calls. So a picture is what the program shows, in the state named.
//!
//! Ignored, so that `cargo test` never makes them: `make
//! screenshots-tcsmoc` does, on demand. It takes a minute or two, most of it
//! waiting for the beacon indicator to age as the beacons stop. It must run
//! alone, as the target runs it: a Slint platform is set once per thread,
//! and the queue the beacon thread hands the window its work through is set
//! once per process.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use i_slint_backend_testing::{ElementHandle, ElementQuery};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{EventLoopProxy, Platform, WindowAdapter};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use tcslib::beacon::{Contact, Link};
use tcslib::gateway::GatewayReceiver;
use tcslib::matched::Matched;
use tcslibgs::graph::telemetry::{self, Reaches};
use tcslibgs::graph::Graph;
use tcslibgs::{
    Beacon, Command, CommandStatus, ConfigDigest, ConfigVersion,
    ConnectTelemetry, HostPort, NodeCounts, NodeTelemetry, QueryNodeTelemetry,
    Scalar, Telemetry, Timestamp,
};

use super::*;
use crate::beacon_receive::{BeaconReceive, Contacts, Decided, OnContact};
use crate::{DHInfo, MainWindow, MenuAction, PingWindow, PlotWindow};

thread_local! {
    /// Every window made on this thread, in the order they were made: what
    /// a component's window is drawn into is found here.
    static MADE: RefCell<Vec<Rc<MinimalSoftwareWindow>>> =
        const { RefCell::new(Vec::new()) };
}

/// What the beacon thread, and anything else off the window's thread, has
/// asked the window to do, waiting for [`settle`] to do it.
type Queued = Vec<Box<dyn FnOnce() + Send>>;

/// The work waiting for the window's thread.
static QUEUED: Mutex<Queued> = Mutex::new(Vec::new());

/// The widest a picture is, in pixels: about as wide as the manual's page.
/// A part of a window wider than this is shrunk to it.
const WIDEST: f32 = 800.0;

/// How many pixels a window is drawn with for each of its own, so
/// that a picture of it shrunk to [`WIDEST`] is made from more pixels than
/// it keeps and its text stays legible. Two, because Slint's style draws
/// its one-pixel borders only at a whole scale.
const DRAWN_AT: f32 = 2.0;

/// The platform the window is drawn on: a buffer for each window, and no
/// screen.
struct Headless;

impl Platform for Headless {
    /// Makes a window that draws into memory, and remembers it.
    ///
    /// # Returns
    ///
    /// The window.
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        MADE.with(|made| made.borrow_mut().push(window.clone()));
        Ok(window)
    }

    /// The queue `slint::invoke_from_event_loop` puts work on.
    ///
    /// # Returns
    ///
    /// The queue.
    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        Some(Box::new(Queue))
    }
}

/// Where work for the window's thread waits: see [`QUEUED`].
struct Queue;

impl EventLoopProxy for Queue {
    /// Nothing to quit: there is no event loop, only [`settle`].
    ///
    /// # Returns
    ///
    /// Success.
    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
        Ok(())
    }

    /// Queues work for the window's thread.
    ///
    /// # Arguments
    ///
    /// * `event` - The work.
    ///
    /// # Returns
    ///
    /// Success.
    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), slint::EventLoopError> {
        QUEUED.lock().expect("the queue").push(event);
        Ok(())
    }
}

/// Does what an event loop would have done by now: the work queued for the
/// window's thread, and the timers that are due.
fn settle() {
    loop {
        let queued: Queued = std::mem::take(&mut *QUEUED.lock().unwrap());
        slint::platform::update_timers_and_animations();
        if queued.is_empty() {
            return;
        }
        for work in queued {
            work();
        }
    }
}

/// Lets the threads that are not the window's run for a while, doing the
/// window's share of the work as it comes.
///
/// # Arguments
///
/// * `long` - How long.
fn pause(long: Duration) {
    let until = Instant::now() + long;
    while Instant::now() < until {
        settle();
        std::thread::sleep(Duration::from_millis(10));
    }
    settle();
}

/// Waits, doing the window's work as it comes, until something is true.
///
/// # Arguments
///
/// * `what` - What is waited for, said when it never comes.
/// * `done` - Whether it has come.
///
/// # Panics
///
/// If it has not come within two minutes.
fn until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        settle();
        if done() {
            return;
        }
        assert!(Instant::now() < deadline, "waited too long for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A part of a window, in logical pixels, which at a scale of one are the
/// picture's.
#[derive(Debug, Clone, Copy)]
struct Region {
    /// Its left edge.
    x: f32,
    /// Its top edge.
    y: f32,
    /// How wide it is.
    width: f32,
    /// How tall it is.
    height: f32,
}

impl Region {
    /// The part of a window an element takes, with a margin round it.
    ///
    /// # Arguments
    ///
    /// * `element` - The element.
    /// * `margin`  - How much more to take on every side.
    ///
    /// # Returns
    ///
    /// The region.
    fn around(element: &ElementHandle, margin: f32) -> Region {
        let at = element.absolute_position();
        let size = element.size();
        Region {
            x: at.x - margin,
            y: at.y - margin,
            width: size.width + 2.0 * margin,
            height: size.height + 2.0 * margin,
        }
    }

    /// The top of a window, down to a depth.
    ///
    /// # Arguments
    ///
    /// * `width`  - The window's width.
    /// * `height` - How far down.
    ///
    /// # Returns
    ///
    /// The region.
    fn top(width: f32, height: f32) -> Region {
        Region {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }
    }
}

/// Has a window drawn with [`DRAWN_AT`] pixels for each of its own. Before
/// it is given its size, which is kept in pixels.
///
/// # Arguments
///
/// * `window` - The window.
fn drawn_finely(window: &slint::Window) {
    window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged {
        scale_factor: DRAWN_AT,
    });
}

/// How wide a window is, in logical pixels.
///
/// # Arguments
///
/// * `window` - The window.
///
/// # Returns
///
/// Its width.
fn width_of(window: &slint::Window) -> f32 {
    window.size().to_logical(window.scale_factor()).width
}

/// Where the pictures go: `docs/shots`, beside the manual.
///
/// # Returns
///
/// The directory, made if it was not there.
fn shots() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("shots");
    std::fs::create_dir_all(&dir).expect("docs/shots");
    dir
}

/// Draws a window and writes a part of it as `docs/shots/tcsmoc-NAME.png`.
///
/// # Arguments
///
/// * `window` - The window.
/// * `region` - The part of it to keep; `None` for all of it.
/// * `name`   - The picture's name, after `tcsmoc-`.
///
/// # Panics
///
/// If the window was not made on this platform, or the picture cannot be
/// written.
fn shoot(window: &slint::Window, region: Option<Region>, name: &str) {
    let adapter = MADE
        .with(|made| {
            made.borrow()
                .iter()
                .find(|made| std::ptr::eq(made.window(), window))
                .cloned()
        })
        .expect("a window made here");
    // Drawn, given time for its timers, and drawn again, as a window being
    // shown is: a repeater nested in another's rows is otherwise drawn with
    // the rows it held before it was first drawn -- the plot's traces and
    // their legend, which its redraw timer fills. Each time told of a size
    // a pixel taller and then of its own, which lays it out afresh; a
    // window told of the size it has does nothing.
    let scale = window.scale_factor();
    let own = window.size().to_logical(scale);
    let taller = slint::LogicalSize::new(own.width, own.height + 1.0);
    let size = adapter.size();
    let (width, height) = (size.width as usize, size.height as usize);
    let mut buffer = vec![PremultipliedRgbaColor::default(); width * height];
    for _ in 0..2 {
        for size in [taller, own] {
            window
                .dispatch_event(slint::platform::WindowEvent::Resized { size });
        }
        pause(plot::REDRAW + Duration::from_millis(50));
        adapter.request_redraw();
        adapter.draw_if_needed(|renderer| {
            renderer.render(&mut buffer, width);
        });
    }

    // The region is in logical pixels, and the picture in physical ones.
    let whole = Region::top(width as f32 / scale, height as f32 / scale);
    let region = region.unwrap_or(whole);
    let clip =
        |at: f32, most: usize| ((at * scale).max(0.0) as usize).min(most);
    let (left, top) = (clip(region.x, width), clip(region.y, height));
    let right = clip(region.x + region.width, width);
    let bottom = clip(region.y + region.height, height);

    let (cut_width, cut_height) = (right - left, bottom - top);
    let mut rgb = Vec::with_capacity(cut_width * cut_height * 3);
    for y in top..bottom {
        for pixel in &buffer[y * width + left..y * width + right] {
            // Everything drawn is opaque, the window's background being
            // white, so premultiplied is plain.
            rgb.extend_from_slice(&[pixel.red, pixel.green, pixel.blue]);
        }
    }
    // Shrunk to the page where it is wider than that, and otherwise to one
    // pixel for each of the window's own.
    let wanted = (cut_width as f32 / scale).min(WIDEST);
    let (rgb, out_width, out_height) =
        shrink(&rgb, cut_width, cut_height, wanted / cut_width as f32);
    let path = shots().join(format!("tcsmoc-{name}.png"));
    let file = std::fs::File::create(&path).expect("the picture's file");
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        out_width as u32,
        out_height as u32,
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(&rgb))
        .expect("the picture written");
    eprintln!("wrote {}", path.display());
}

/// Shrinks a picture, each pixel of the result the average of the part of
/// the original it covers.
///
/// # Arguments
///
/// * `rgb`    - The picture, three bytes to a pixel, row by row.
/// * `width`  - Its width.
/// * `height` - Its height.
/// * `by`     - How much smaller: the result's size over the original's,
///              at most one.
///
/// # Returns
///
/// The shrunk picture, its width and its height.
fn shrink(
    rgb: &[u8],
    width: usize,
    height: usize,
    by: f32,
) -> (Vec<u8>, usize, usize) {
    if by >= 1.0 {
        return (rgb.to_vec(), width, height);
    }
    // What each pixel of the result takes from the original along one
    // axis: which pixels, and how much of each.
    let spans = |from: usize, to: usize| -> Vec<Vec<(usize, f32)>> {
        let step = from as f32 / to as f32;
        (0..to)
            .map(|i| {
                let (start, end) = (i as f32 * step, (i + 1) as f32 * step);
                let mut taken = Vec::new();
                let mut at = start.floor() as usize;
                while (at as f32) < end && at < from {
                    let overlap =
                        end.min(at as f32 + 1.0) - start.max(at as f32);
                    taken.push((at, overlap / step));
                    at += 1;
                }
                taken
            })
            .collect()
    };
    let out_width = ((width as f32 * by).round() as usize).max(1);
    let out_height = ((height as f32 * by).round() as usize).max(1);
    let across = spans(width, out_width);
    let down = spans(height, out_height);
    let mut out = Vec::with_capacity(out_width * out_height * 3);
    for rows in &down {
        for columns in &across {
            let mut sum = [0.0f32; 3];
            for &(y, wy) in rows {
                for &(x, wx) in columns {
                    let at = (y * width + x) * 3;
                    for (c, total) in sum.iter_mut().enumerate() {
                        *total += f32::from(rgb[at + c]) * wy * wx;
                    }
                }
            }
            out.extend(sum.map(|c| c.round().clamp(0.0, 255.0) as u8));
        }
    }
    (out, out_width, out_height)
}

/// Presses the one control with this label inside an element.
///
/// # Arguments
///
/// * `inside` - Where to look.
/// * `label`  - The control's label.
///
/// # Panics
///
/// If there is no such control.
fn press_in(inside: &ElementHandle, label: &str) {
    let wanted = label.to_string();
    inside
        .query_descendants()
        .match_predicate(move |e| {
            e.accessible_label().as_deref() == Some(wanted.as_str())
        })
        .find_first()
        .unwrap_or_else(|| panic!("no {label} here"))
        .invoke_accessible_default_action();
}

/// Presses the first control with this label in a window.
///
/// # Arguments
///
/// * `ui`    - The window.
/// * `label` - The control's label.
///
/// # Panics
///
/// If there is no such control.
fn press(ui: &MainWindow, label: &str) {
    ElementHandle::find_by_accessible_label(ui, label)
        .next()
        .unwrap_or_else(|| panic!("no {label} in the window"))
        .invoke_accessible_default_action();
}

/// The panels of the main window, in the order of its model.
///
/// # Arguments
///
/// * `ui` - The window.
///
/// # Returns
///
/// One element per panel.
fn panel_elements(ui: &MainWindow) -> Vec<ElementHandle> {
    ElementQuery::from_root(ui)
        .match_descendants()
        .match_inherits("DHPanel")
        .find_all()
}

/// The part of the main window from its top to the foot of a row of panels.
///
/// # Arguments
///
/// * `ui`  - The window.
/// * `row` - The panel whose foot it ends under.
///
/// # Returns
///
/// The region.
fn down_to_panel(ui: &MainWindow, row: usize) -> Region {
    let panel = &panel_elements(ui)[row];
    let foot = panel.absolute_position().y + panel.size().height + 8.0;
    Region::top(width_of(ui.window()), foot)
}

/// The part of the main window above its panels: the link, the beacon,
/// the commands and what they answered.
///
/// # Arguments
///
/// * `ui` - The window.
///
/// # Returns
///
/// The region.
fn above_panels(ui: &MainWindow) -> Region {
    let top = panel_elements(ui)[0].absolute_position().y;
    // The Data Handlers heading, and not the panels under it.
    Region::top(width_of(ui.window()), top - 12.0)
}

/// What the simulated vehicle holds: what it is running, which nodes are
/// running, and which one it refuses to start.
struct Aboard {
    /// The checksum of the configuration it runs, which a CONNECT is
    /// answered with.
    digest: ConfigDigest,
    /// Whether each node, by device id, is running.
    running: BTreeMap<u32, bool>,
    /// A node whose START_NODE is refused, as a node is whose device has
    /// failed: `None` where every node starts.
    faulty: Option<u32>,
}

/// The simulated vehicle's command interpreter: answers every command the
/// window sends, as tcspecial would, by the same serialisation.
///
/// # Arguments
///
/// * `socket` - Where the window's commands arrive.
/// * `aboard` - What the vehicle holds.
/// * `done`   - Set when the pictures are made, which ends it.
///
/// # Returns
///
/// The thread answering.
fn interpreter(
    socket: UdpSocket,
    aboard: Arc<Mutex<Aboard>>,
    done: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .expect("a timeout");
    std::thread::spawn(move || {
        let mut buffer = [0u8; 65535];
        while !done.load(Ordering::Relaxed) {
            let Ok((size, from)) = socket.recv_from(&mut buffer) else {
                continue;
            };
            let command = Command::read(&buffer[..size]).expect("a command");
            let answer = answer(&command, &mut aboard.lock().unwrap());
            socket.send_to(&answer.to_bytes(), from).expect("answered");
        }
    })
}

/// What the simulated vehicle answers a command with, changing what it
/// holds as tcspecial would.
///
/// # Arguments
///
/// * `command` - The command.
/// * `aboard`  - What the vehicle holds.
///
/// # Returns
///
/// The answer.
fn answer(command: &Command, aboard: &mut Aboard) -> Telemetry {
    let seq = command.sequence();
    match command {
        Command::Connect(_) => Telemetry::Connect(ConnectTelemetry::new(
            seq,
            ConfigVersion::of_this_build(),
            aboard.digest,
            seq,
        )),
        Command::QueryNode(cmd) => {
            let running =
                aboard.running.get(&cmd.node.0).copied().unwrap_or(true);
            Telemetry::QueryNode(QueryNodeTelemetry::new(
                seq,
                CommandStatus::Success,
                cmd.node,
                running,
                NodeCounts {
                    given_bytes: 2400,
                    writes: 600,
                    last_moved: Some(Timestamp::now()),
                    ..Default::default()
                },
            ))
        }
        Command::StopNode(cmd) => {
            aboard.running.insert(cmd.node.0, false);
            Telemetry::StopNode(NodeTelemetry::stopped(
                seq,
                CommandStatus::Success,
                cmd.node,
            ))
        }
        Command::StartNode(cmd) => {
            let status = if aboard.faulty == Some(cmd.node.0) {
                CommandStatus::Failure
            } else {
                aboard.running.insert(cmd.node.0, true);
                CommandStatus::Success
            };
            Telemetry::StartNode(NodeTelemetry::started(seq, status, cmd.node))
        }
        other => tcslibgs::answer_to(other, CommandStatus::Success),
    }
}

/// A free run of ports on loopback, `count` long.
///
/// # Arguments
///
/// * `count` - How many.
/// * `tries` - Which attempt this is, so that each looks somewhere new.
///
/// # Returns
///
/// The first port of the run.
fn free_base(count: u16, tries: u16) -> u16 {
    let port = UdpSocket::bind("127.0.0.1:0")
        .and_then(|socket| socket.local_addr())
        .expect("a free port")
        .port();
    // Somewhere below the top, so the run fits; moved on each try.
    port.min(65535 - count - 1).saturating_sub(tries * 97)
}

/// A gateway node's receiver, opened where the ground's end has been moved
/// to a free run of ports, so that nothing else's ports are bound.
///
/// # Arguments
///
/// * `graph`   - The configuration, which the ground end is moved in.
/// * `gateway` - The gateway node.
///
/// # Returns
///
/// The receiver, and the configuration as moved: where the vehicle is to
/// send each source's data.
fn receiver_on_free_ports(
    graph: &Graph,
    gateway: &str,
) -> (GatewayReceiver, Graph) {
    let most = graph.nodes.len() as u16 + 1;
    for tries in 0..50 {
        let base = free_base(most, tries);
        let mut moved = graph.clone();
        let node = moved
            .nodes
            .iter_mut()
            .find(|node| node.name == gateway)
            .expect("the gateway node");
        node.attributes.insert(
            "ground_base".to_string(),
            Scalar::of(format!("127.0.0.1:{base}")),
        );
        if let Ok(receiver) = GatewayReceiver::open(&moved, gateway) {
            return (receiver, moved);
        }
    }
    panic!("no free run of ports for {gateway}");
}

/// One window, on one shipped example, and the simulated vehicle it talks
/// to: everything `run` makes, but the children it starts.
struct Station {
    /// The configuration with the ground's end moved to free ports, which
    /// the vehicle sends data by.
    moved: Graph,
    /// The main window.
    ui: MainWindow,
    /// The panels.
    model: Rc<VecModel<DHInfo>>,
    /// The command link.
    link: Arc<Mutex<CiLink>>,
    /// Whether this window is matched with the vehicle.
    matched: Matched,
    /// The checksum of the configuration.
    digest: ConfigDigest,
    /// Where the command link goes, as the configuration states it: what
    /// the address box says.
    address: String,
    /// Where the simulated vehicle takes commands.
    vehicle_at: String,
    /// What the simulated vehicle holds.
    aboard: Arc<Mutex<Aboard>>,
    /// The vehicle's beacon socket, and where it sends them.
    beacon_from: UdpSocket,
    /// Where this window hears beacons.
    beacon_to: std::net::SocketAddr,
    /// The vehicle's data socket.
    data_from: UdpSocket,
    /// The plot, where the example has one.
    plot: Option<Arc<Mutex<Plot>>>,
    /// The plot window.
    plot_window: PlotWindow,
    /// The ping window.
    ping_window: PingWindow,
    /// What the panels last received.
    arrivals: Arrivals,
    /// Set when the pictures are made, which ends the simulated vehicle.
    done: Arc<AtomicBool>,
    /// Kept for as long as the station is: the plot's redraw and the ping
    /// window's.
    _timers: Vec<slint::Timer>,
}

impl Station {
    /// Opens a window on a shipped example, as `run` does, wired to a
    /// simulated vehicle.
    ///
    /// # Arguments
    ///
    /// * `name` - The example, under `examples/`. Its simulator file,
    ///            where it has one, is where the plot's ambient comes
    ///            from, as in a run with `--simulate=all`.
    ///
    /// # Returns
    ///
    /// The station, its window not yet matched with anything.
    fn on(name: &str) -> Station {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let path = root.join(format!("examples/{name}/{name}.ytcs"));
        let sim = root.join(format!("examples/{name}/{name}-sim.yaml"));
        let graph = Graph::load(&path).expect("the example reads");
        let links = telemetry::of(&graph).expect("its links");
        let digest = tcslibgs::config_digest::digest_of_graph(&graph)
            .expect("its checksum");

        let ui = MainWindow::new().expect("a window");
        let sizes = Sizes::settle(&ui);
        let rows_in_order = panels(&links, &settable_of(&graph));
        let rows: Arc<BTreeMap<String, usize>> = Arc::new(
            rows_in_order
                .iter()
                .enumerate()
                .map(|(row, info)| (info.name.to_string(), row))
                .collect(),
        );
        let units: Arc<BTreeMap<String, Unit>> = Arc::new(
            graph
                .nodes
                .iter()
                .filter_map(|node| {
                    tcslibgs::graph::units::unit_of(&graph, &node.name)
                        .map(|unit| (node.name.clone(), unit))
                })
                .collect(),
        );
        let count = rows_in_order.len();
        let model = Rc::new(VecModel::from(rows_in_order));
        ui.set_dh_model(ModelRc::from(model.clone()));
        // The size `run` opens it at, drawn finely enough to be shrunk to
        // the manual's page: see DRAWN_AT.
        drawn_finely(ui.window());
        let shape = grid_shape(&sizes, count);
        ui.window().set_size(window_size(&sizes, &shape, count));
        ui.show().expect("shown");

        let matched = Matched::for_configuration(digest);
        let (_, address, _) = command_links_of(&graph)
            .expect("its command links")
            .expect("a tcspecial section");
        let vehicle = UdpSocket::bind("127.0.0.1:0").expect("a socket");
        let vehicle_at = vehicle.local_addr().expect("where").to_string();
        let here = |at: &str| {
            let at: std::net::SocketAddr = at.parse().expect("an address");
            HostPort {
                address: at.ip().to_string(),
                port: at.port(),
            }
        };
        let link = Arc::new(Mutex::new(CiLink::down_commanding(
            "127.0.0.1:0",
            Commanded {
                from: here("127.0.0.1:0"),
                to: here(&vehicle_at),
            },
            matched.clone(),
        )));
        let aboard = Arc::new(Mutex::new(Aboard {
            digest,
            running: BTreeMap::new(),
            faulty: None,
        }));
        let done = Arc::new(AtomicBool::new(false));
        interpreter(vehicle, aboard.clone(), done.clone());

        ui.set_ci_status(SharedString::from(DISCONNECTED_STATUS));
        ui.set_ci_address(SharedString::from(address.as_str()));
        ui.set_last_response(SharedString::from(
            "Each panel is a node. Press Connect to command them",
        ));

        let agreed: Agreed = Arc::new(Mutex::new(None));
        let declined = declined_for(rows.len());
        let contact = Arc::new(Mutex::new(Contact::Lost));

        // The beacon watch, on a socket of this test's, wired as `run`
        // wires it.
        let socket = UdpSocket::bind("127.0.0.1:0").expect("a socket");
        let beacon_to = socket.local_addr().expect("where");
        let beacon_from = UdpSocket::bind("127.0.0.1:0").expect("a socket");
        let interval = tcslibgs::graph::parameters::beacon_interval_of(&graph)
            .expect("its interval")
            .unwrap_or(tcslib::beacon::STANDARD_INTERVAL);
        let beacons = tcslib::beacon::Beacons::on(
            socket,
            Link {
                send: here(&beacon_from.local_addr().unwrap().to_string()),
                recv: here(&beacon_to.to_string()),
            },
            crate::config::constants::beacon_schedule(interval),
            Some(digest),
        )
        .expect("the beacon watch");
        let lost: OnContact = {
            let weak = ui.as_weak();
            Arc::new(move || {
                let weak = weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = weak.upgrade() {
                        every_node_lost_contact(&ui.get_dh_model());
                    }
                });
            })
        };
        let made: OnContact = {
            let weak = ui.as_weak();
            let link = link.clone();
            let agreed = agreed.clone();
            let declined = declined.clone();
            Arc::new(move || {
                let weak = weak.clone();
                let link = link.clone();
                let agreed = agreed.clone();
                let declined = declined.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = weak.upgrade() {
                        contact_made(
                            &ui,
                            &link,
                            &agreed,
                            &ui.get_dh_model(),
                            &declined,
                        );
                    }
                });
            })
        };
        let heard: crate::beacon_receive::OnBeacon = {
            let weak = ui.as_weak();
            let link = link.clone();
            let agreed = agreed.clone();
            Arc::new(move |restarted| {
                beacon_to_commands(&weak, &link, &agreed, restarted)
            })
        };
        let _ = BeaconReceive::new(
            ui.as_weak(),
            beacons,
            Decided {
                contact: contact.clone(),
                heard: Some(heard),
                matched: matched.clone(),
            },
            None,
            Contacts {
                lost: Some(lost),
                made: Some(made),
            },
        );

        // The ping window, the plot and the receivers, as `run` makes them.
        let mut timers = Vec::new();
        let ping_window = PingWindow::new().expect("the ping window");
        let gateway = graph
            .nodes
            .iter()
            .find(|n| tcslibgs::gateway::is_gateway(&n.node_type))
            .map(|n| n.name.clone())
            .expect("a gateway node");
        let (receiver, moved) = receiver_on_free_ports(&graph, &gateway);
        let pings = tcslib::ping::route_of(&graph).map(|route| {
            let vehicle_ping =
                UdpSocket::bind("127.0.0.1:0").expect("a ping socket");
            let port = vehicle_ping.local_addr().unwrap().port();
            let mut aimed = graph.clone();
            if let Some(node) =
                aimed.nodes.iter_mut().find(|n| n.name == route.gateway)
            {
                node.attributes
                    .insert("port".to_string(), Scalar::of(port.to_string()));
            }
            let (through, feed) =
                ping_node(&ui, &ping_window, &aimed, &matched)
                    .expect("it can be pinged")
                    .expect("a ping node");
            ping_node_answers(vehicle_ping, &moved, &route.node, &done);
            timers.push(show_answers(feed.clone()));
            (through, feed)
        });
        let plot_window = PlotWindow::new().expect("the plot window");
        let ambient = sim
            .is_file()
            .then(|| ambient_of(sim.to_str(), Mode::All))
            .flatten();
        let plot =
            Plot::of(&graph, |name| links.iter().any(|l| l.source == name))
                .map(|plot| Arc::new(Mutex::new(plot.with_ambient(ambient))));
        if let Some(plot) = &plot {
            timers.push(plot::show(&ui, &plot_window, plot.clone()));
        }
        let arrivals: Arrivals = Arc::new(Mutex::new(vec![None; count]));
        let mut receiver = receiver;
        receiver.gated_by(matched.clone());
        {
            let arrivals = arrivals.clone();
            let plot = plot.clone();
            let feed = pings.map(|(_, feed)| feed);
            std::thread::spawn(move || {
                serve(receiver, rows, units, arrivals, feed, plot)
            });
        }

        command_buttons(
            &ui,
            &link,
            &agreed,
            Some(digest),
            &model,
            &declined,
            &contact,
            &Arc::new(graph.clone()),
        );
        settle();

        Station {
            moved,
            ui,
            model,
            link,
            matched,
            digest,
            address,
            vehicle_at,
            aboard,
            beacon_from,
            beacon_to,
            data_from: UdpSocket::bind("127.0.0.1:0").expect("a socket"),
            plot,
            plot_window,
            ping_window,
            arrivals,
            done,
            _timers: timers,
        }
    }

    /// Sends this window a beacon, and lets it take it.
    ///
    /// # Arguments
    ///
    /// * `number`  - The beacon's number.
    /// * `restart` - Whether its restart flag is set.
    fn beacon(&self, number: u32, restart: bool) {
        let beacon =
            Beacon::new(number, ConfigVersion::of_this_build(), self.digest)
                .restart_flag(restart);
        self.beacon_from
            .send_to(&beacon.to_bytes(), self.beacon_to)
            .expect("a beacon sent");
        pause(Duration::from_millis(300));
    }

    /// Presses Connect, as the operator does.
    ///
    /// The address box says where the configuration says tcspecial is; the
    /// simulated vehicle is elsewhere, on a port this test chose. So the
    /// box is pointed at it for the press, as an operator pointing the
    /// window at another tcspecial would, and put back for the picture.
    fn connect(&self) {
        self.ui
            .set_ci_address(SharedString::from(self.vehicle_at.as_str()));
        press(&self.ui, "Connect");
        self.ui
            .set_ci_address(SharedString::from(self.address.as_str()));
        settle();
    }

    /// Sends the window what the vehicle's nodes produce, each datum framed
    /// as its link carries it, to the receivers' ports: see `refresh` for
    /// the panels' repainting.
    ///
    /// # Arguments
    ///
    /// * `values` - Each source's datum, by name.
    fn send(&self, values: &BTreeMap<String, Vec<u8>>) {
        let links = telemetry::of(&self.moved).expect("its links");
        for link in &links {
            let Some(data) = values.get(&link.source) else {
                continue;
            };
            let Reaches::Socket { ground, .. } = &link.reaches else {
                continue;
            };
            let mut out = Vec::new();
            match link.count() {
                Some(count) => {
                    tcslibgs::link_type::counted(data, count, &mut out)
                        .expect("counted");
                }
                None => out.extend_from_slice(data),
            }
            self.data_from
                .send_to(&out, (ground.address.as_str(), ground.port))
                .expect("a datum sent");
        }
    }

    /// Repaints the panels and the counts from what has arrived, as `run`'s
    /// timer does once a second.
    fn refresh(&self) {
        pause(Duration::from_millis(200));
        show_arrivals(&self.model, &self.arrivals);
        self.ui.set_vehicle_packets(SharedString::from(packets_said(
            &self.matched.counts(),
        )));
        if let Ok(link) = self.link.try_lock() {
            let (sent, received) = link.command_times().unwrap_or((None, None));
            self.ui
                .set_cmd_sent_time(SharedString::from(crate::shown_time(sent)));
            self.ui
                .set_cmd_recv_time(SharedString::from(crate::shown_time(
                    received,
                )));
        }
        settle();
    }

    /// The row of a source's panel.
    ///
    /// # Arguments
    ///
    /// * `name` - The source.
    ///
    /// # Returns
    ///
    /// Its row.
    fn row_of(&self, name: &str) -> usize {
        (0..self.model.row_count())
            .find(|&row| self.model.row_data(row).unwrap().name == name)
            .unwrap_or_else(|| panic!("no panel for {name}"))
    }
}

/// The simulated vehicle's ping node: answers each ping that reaches it,
/// stamped with its clock, down to the ground's end of its link.
///
/// # Arguments
///
/// * `socket` - Where the gateway node's uplink delivers.
/// * `moved`  - The configuration with the ground's end moved, which says
///              where the answer goes.
/// * `node`   - The ping node's name.
/// * `done`   - Set when the pictures are made, which ends it.
fn ping_node_answers(
    socket: UdpSocket,
    moved: &Graph,
    node: &str,
    done: &Arc<AtomicBool>,
) {
    use tcslibgs::node::vehicle::ping;

    let link = telemetry::of(moved)
        .expect("its links")
        .into_iter()
        .find(|link| link.source == node)
        .expect("the ping node's link down");
    let Reaches::Socket { ground, .. } = &link.reaches else {
        panic!("the ping node's answers go to a socket");
    };
    let to = format!("{}:{}", ground.address, ground.port);
    let count = link.count();
    let done = done.clone();
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .expect("a timeout");
    std::thread::spawn(move || {
        let mut buffer = [0u8; 256];
        while !done.load(Ordering::Relaxed) {
            let Ok((size, _)) = socket.recv_from(&mut buffer) else {
                continue;
            };
            // A link of a few milliseconds each way, which the round trip
            // then shows.
            std::thread::sleep(Duration::from_millis(4));
            let at = ping::now().expect("a clock");
            let reply = ping::reply(&buffer[..size], at).expect("answered");
            std::thread::sleep(Duration::from_millis(3));
            let mut out = Vec::new();
            match count {
                Some(count) => {
                    tcslibgs::link_type::counted(
                        reply.as_bytes(),
                        count,
                        &mut out,
                    )
                    .expect("counted");
                }
                None => out.extend_from_slice(reply.as_bytes()),
            }
            socket.send_to(&out, &to).expect("an answer sent");
        }
    });
}

/// The zone the thermostat example holds, simulated a second at a time:
/// what its three sensors read, the vote, and what the heaters were told.
struct Zone {
    /// The zone's temperature, in kelvin.
    kelvin: f64,
    /// Whether the heaters are on.
    heating: bool,
    /// A count that makes each sensor's noise.
    tick: u64,
}

impl Zone {
    /// The setpoint and half band the example states: see
    /// `examples/thermostat-common`.
    const SETPOINT: f64 = 293.15;
    /// How far either side of the setpoint the heaters are switched.
    const HALF_BAND: f64 = 2.0;
    /// The simulated world's ambient temperature, which the zone drifts
    /// towards with the heaters off.
    const AMBIENT: f64 = 288.15;

    /// A second on: the zone warms or cools, and the thermostat decides.
    ///
    /// # Returns
    ///
    /// Each source's datum, by name, as tcspecial sends it.
    fn step(&mut self) -> BTreeMap<String, Vec<u8>> {
        self.tick += 1;
        let warming = if self.heating { 0.045 } else { 0.0 };
        self.kelvin += warming - 0.004 * (self.kelvin - Self::AMBIENT);
        // Three sensors that read a little apart, each to its own step.
        let noise = |seed: u64| {
            let x = (self.tick.wrapping_mul(2654435761) ^ seed) % 1000;
            (x as f64 / 1000.0 - 0.5) * 0.12
        };
        let step = |value: f64, by: f64| (value / by).round() * by;
        let k1 = step(self.kelvin + noise(17), 0.0625);
        let k2 = step(self.kelvin + noise(91) - 0.05, 0.0625);
        let k3 = step(self.kelvin + noise(53) + 0.25, 0.0078);
        let mut sorted = [k1, k2, k3];
        sorted.sort_by(f64::total_cmp);
        let vote = sorted[1];
        if vote < Self::SETPOINT - Self::HALF_BAND {
            self.heating = true;
        } else if vote > Self::SETPOINT + Self::HALF_BAND {
            self.heating = false;
        }
        let value = |kelvin: f64| {
            tcslibgs::node::vehicle::compute::value_bytes(kelvin, Unit::Kelvin)
                .expect("a temperature")
                .to_vec()
        };
        BTreeMap::from([
            ("to_k1".to_string(), value(k1)),
            ("to_k2".to_string(), value(k2)),
            ("to_k3".to_string(), value(k3)),
            ("consensus_temp".to_string(), value(vote)),
            ("gpio-out-tmass1".to_string(), vec![u8::from(self.heating)]),
            ("gpio-out-tmass2".to_string(), vec![u8::from(self.heating)]),
        ])
    }
}

/// Closes a pop-up as a click outside it does.
///
/// # Arguments
///
/// * `ui` - The window.
fn click_outside(ui: &MainWindow) {
    use slint::platform::{PointerEventButton, WindowEvent};

    let position = slint::LogicalPosition::new(2.0, 2.0);
    for event in [
        WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Left,
        },
        WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        },
    ] {
        ui.window().dispatch_event(event);
    }
    settle();
}

/// Makes every picture in the tcsmoc manual: see the module's comment.
#[test]
#[ignore = "makes the manual's screenshots: make screenshots-tcsmoc"]
fn screenshots() {
    slint::platform::set_platform(Box::new(Headless)).expect(
        "the headless platform: run this test alone, as make \
         screenshots-tcsmoc does",
    );

    thermostat();
    ping();
}

/// The thermostat example's pictures: coming up and connecting, the data
/// and the plot, commanding a node, losing contact, and a restart.
fn thermostat() {
    let station = Station::on("thermostat");
    let ui = &station.ui;

    // Coming up. Just opened: nothing heard, the link down, every panel
    // Unknown.
    station.refresh();
    shoot(ui.window(), Some(down_to_panel(ui, 0)), "opened");

    // The vehicle's first beacon, which every start sends with its restart
    // flag set.
    station.beacon(1, true);
    shoot(ui.window(), Some(above_panels(ui)), "first-beacon");

    // Connect: the vehicle agrees, and clears its flag, which its next
    // beacon, sent at once, says; that beacon matches the window, which
    // then asks what each node is doing.
    station.connect();
    station.refresh();
    shoot(ui.window(), Some(above_panels(ui)), "connected");
    station.beacon(2, false);
    station.refresh();
    shoot(ui.window(), Some(down_to_panel(ui, 0)), "matched");

    // The data: ten minutes of the zone as the plot holds it, and the last
    // seconds of it through the receivers into the panels.
    let mut zone = Zone {
        kelvin: 292.4,
        heating: false,
        tick: 0,
    };
    let plot = station.plot.as_ref().expect("a plot");
    let start = Instant::now() - Duration::from_secs(600);
    for second in 0..600 {
        let values = zone.step();
        let at = start + Duration::from_secs(second);
        for (source, data) in &values {
            let plotted = plot.lock().unwrap().plotted(source);
            if let Some(plotted) = plotted {
                plot::take(plot, plotted, data, at);
            }
        }
    }
    for _ in 0..5 {
        station.send(&zone.step());
        pause(Duration::from_millis(50));
    }
    station.beacon(3, false);
    station.refresh();
    shoot(ui.window(), Some(down_to_panel(ui, 2)), "data");

    // The plot window, over the last ten minutes, as its 10 min button
    // shows it.
    let window = &station.plot_window;
    // The size it opens at: see `preferred-width` in the markup.
    drawn_finely(window.window());
    window
        .window()
        .set_size(slint::LogicalSize::new(820.0, 620.0));
    window.set_span(2);
    window.invoke_span_chosen(2);
    pause(Duration::from_millis(250));
    shoot(window.window(), None, "plot");

    // Commanding a node: the Command menu, Query, a panel's Configuration,
    // Disable, and an Enable the vehicle refuses.
    station.beacon(4, false);
    press(ui, "Command");
    settle();
    // The menu is 220 high, 50 below the top of the row its button is in:
    // see the PopupWindow in the markup.
    let button = ElementHandle::find_by_accessible_label(ui, "Command")
        .next()
        .expect("the Command button");
    let foot = button.absolute_position().y + 50.0 + 220.0 + 12.0;
    shoot(
        ui.window(),
        Some(Region::top(width_of(ui.window()), foot)),
        "command-menu",
    );
    press(ui, "Query");
    station.refresh();
    let consensus = station.row_of("consensus_temp");
    let panel = &panel_elements(ui)[consensus];
    press_in(panel, "Configuration");
    settle();
    let around = Region::around(panel, 10.0);
    shoot(
        ui.window(),
        Some(Region {
            width: 330.0,
            height: 370.0,
            ..around
        }),
        "configuration",
    );
    click_outside(ui);

    let to_k3 = station.row_of("to_k3");
    ui.invoke_disable_dh(to_k3 as i32);
    station.refresh();
    shoot(ui.window(), Some(down_to_panel(ui, to_k3)), "disabled");
    let id = station.model.row_data(to_k3).unwrap().dev_id as u32;
    station.aboard.lock().unwrap().faulty = Some(id);
    ui.invoke_enable_dh(to_k3 as i32);
    station.refresh();
    shoot(ui.window(), Some(down_to_panel(ui, to_k3)), "refused");
    station.aboard.lock().unwrap().faulty = None;
    ui.invoke_enable_dh(to_k3 as i32);
    station.refresh();

    // Losing contact: the beacons stop, and the indicator ages.
    station.beacon(5, false);
    let last = Instant::now();
    let late = |after: u64, colour: slint::Color| {
        until("the indicator to age", || {
            last.elapsed() > Duration::from_millis(after)
                && ui.get_indicator_color() == colour
        });
    };
    late(10_500, tcsui::palette::well());
    station.refresh();
    shoot(ui.window(), Some(down_to_panel(ui, 0)), "late");
    late(20_500, tcsui::palette::watch());
    station.refresh();
    shoot(ui.window(), Some(down_to_panel(ui, 0)), "aging");
    late(40_500, tcsui::palette::wrong());
    ui.invoke_menu_action(MenuAction::Ping);
    station.refresh();
    let notice = ElementQuery::from_root(ui)
        .match_descendants()
        .match_inherits("NoticePanel")
        .find_first()
        .expect("the pop-up");
    let foot = notice.absolute_position().y + notice.size().height + 12.0;
    shoot(
        ui.window(),
        Some(Region::top(width_of(ui.window()), foot)),
        "unmatched",
    );
    press(ui, "OK");

    // The beacons come back: contact, and the panels stay Stale until a
    // Connect matches the window for commanding again.
    station.beacon(6, false);
    station.refresh();
    shoot(ui.window(), Some(down_to_panel(ui, 0)), "heard-again");
    press(ui, "Disconnect");
    station.connect();
    station.refresh();
    shoot(ui.window(), Some(down_to_panel(ui, 0)), "reconnected");

    // A restart onto the configuration the vehicle runs: ARM Restart,
    // Initiate restart, the flagged beacon, and the new start's first.
    station.beacon(7, false);
    press(ui, "Command");
    press(ui, "ARM Restart");
    station.refresh();
    shoot(ui.window(), Some(above_panels(ui)), "armed");
    press(ui, "Command");
    press(ui, "Initiate restart");
    station.beacon(8, true);
    station.refresh();
    shoot(ui.window(), Some(above_panels(ui)), "restarting");
    station.beacon(1, true);
    station.refresh();
    shoot(ui.window(), Some(above_panels(ui)), "restarted");

    station.done.store(true, Ordering::Relaxed);
}

/// The ping example's pictures: the Ping node button, and the ping window
/// once a ping has been answered.
fn ping() {
    let station = Station::on("ping");
    let ui = &station.ui;
    station.beacon(1, true);
    station.connect();
    station.beacon(2, false);
    station.refresh();
    let button = ElementHandle::find_by_accessible_label(ui, "Ping node")
        .next()
        .expect("the Ping node button");
    let foot = button.absolute_position().y + button.size().height + 8.0;
    shoot(
        ui.window(),
        Some(Region::top(width_of(ui.window()), foot)),
        "ping-node",
    );

    press(ui, "Ping node");
    let window = &station.ping_window;
    drawn_finely(window.window());
    window
        .window()
        .set_size(slint::LogicalSize::new(780.0, 400.0));
    window.invoke_send_clicked();
    until("the ping's answer", || {
        window.get_round_trip().ends_with(" ms")
            && !window.get_round_trip().starts_with("--")
    });
    shoot(window.window(), None, "ping-answered");

    station.done.store(true, Ordering::Relaxed);
}
