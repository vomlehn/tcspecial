//! The screenshots in the manual, taken of the editor's own windows.
//!
//! Each picture in `docs/tcsgui.rst` is a window of this program, opened
//! by [`open_main`] as a person's is, driven into the state shown through
//! the presses and callbacks a person's gestures reach, and drawn into
//! memory by Slint's software renderer. Nothing is opened on a screen: the
//! platform here makes every window a buffer.
//!
//! An ignored test, so that `cargo test` never runs it: the fonts differ
//! from one machine to the next, and so would the pictures. `make
//! screenshots-tcsgui` runs it and writes `docs/shots/tcsgui-*.png`.
//!
//! The configurations are copies of the shipped examples, made in a
//! directory of their own under the system's temporary directory and
//! removed afterwards, so that the names the windows show are short and
//! the same from run to run, and nothing under `examples` is touched. The
//! state directory and the home directory are pointed there too, so no
//! undo history or preference of the person running it is read or
//! written.

use super::*;

use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Key, Platform, PointerEventButton, WindowAdapter};
use slint::platform::{PointerEventButton::Left, PointerEventButton::Right};
use slint::platform::{WindowEvent, WindowEvent::PointerMoved};
use slint::LogicalPosition;
use std::path::{Path, PathBuf};

thread_local! {
    /// Every window the editor has made, in the order it made them: what
    /// [`drawn`] finds a window's buffer by.
    static MADE: RefCell<Vec<Rc<MinimalSoftwareWindow>>> =
        const { RefCell::new(Vec::new()) };
}

/// The markers drawn over a picture: a number in a circle, saying which
/// step of a sequence a gesture there is.
mod marker {
    slint::slint! {
        export component Marker inherits Window {
            in property <string> number;
            width: 26px;
            height: 26px;
            background: transparent;
            Rectangle {
                border-radius: 13px;
                background: #E69F00;
                border-width: 2px;
                border-color: #000000;
                Text {
                    text: root.number;
                    color: #000000;
                    font-size: 15px;
                    font-weight: 800;
                    horizontal-alignment: center;
                    vertical-alignment: center;
                }
            }
        }
    }
}

/// How wide a marker is, in pixels.
const MARKER: u32 = 26;

/// A platform with no screen: each window drawn into memory.
struct Headless;

impl Platform for Headless {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        MADE.with(|made| made.borrow_mut().push(window.clone()));
        Ok(window)
    }
}

/// Draws a window into memory, at a size, after letting whatever was
/// waiting on a timer run.
///
/// # Arguments
///
/// * `window` - The window.
/// * `size`   - Its width and height, in pixels.
///
/// # Returns
///
/// The pixels, row by row, as red, green, blue and alpha.
///
/// # Panics
///
/// If the window was not made by [`Headless`].
fn drawn(window: &slint::Window, (width, height): (u32, u32)) -> Vec<u8> {
    let adapter = MADE.with(|made| {
        made.borrow()
            .iter()
            .find(|made| std::ptr::eq(made.window(), window))
            .cloned()
            .expect("a window the platform made")
    });
    adapter.set_size(slint::PhysicalSize::new(width, height));
    slint::platform::update_timers_and_animations();
    adapter.request_redraw();
    let mut pixels =
        vec![PremultipliedRgbaColor::default(); (width * height) as usize];
    adapter.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, width as usize);
    });
    pixels
        .iter()
        .flat_map(|p| [p.red, p.green, p.blue, p.alpha])
        .collect()
}

/// Draws numbered markers over a picture.
///
/// # Arguments
///
/// * `pixels`  - The picture, red, green, blue and alpha row by row,
///               opaque.
/// * `width`   - How many pixels a row of it has.
/// * `markers` - Each marker's middle, in the picture's pixels, and its
///               number.
fn mark(pixels: &mut [u8], width: u32, markers: &[(LogicalPosition, &str)]) {
    let height = pixels.len() as u32 / 4 / width;
    for (at, number) in markers {
        let marker = marker::Marker::new().expect("a marker");
        marker.set_number((*number).into());
        marker.show().expect("the marker is made");
        let drop = drawn(marker.window(), (MARKER, MARKER));
        marker.hide().expect("the marker goes");
        let left = at.x as i64 - MARKER as i64 / 2;
        let top = at.y as i64 - MARKER as i64 / 2;
        for row in 0..MARKER as i64 {
            for column in 0..MARKER as i64 {
                let (x, y) = (left + column, top + row);
                if x < 0 || y < 0 || x >= width as i64 || y >= height as i64 {
                    continue;
                }
                let from = ((row * MARKER as i64 + column) * 4) as usize;
                let to = ((y * width as i64 + x) * 4) as usize;
                // Premultiplied: what is under shows through by what the
                // marker leaves clear.
                let clear = 255 - drop[from + 3] as u32;
                for channel in 0..3 {
                    let under = pixels[to + channel] as u32;
                    pixels[to + channel] =
                        (drop[from + channel] as u32 + under * clear / 255)
                            .min(255) as u8;
                }
            }
        }
    }
}

/// A part of a picture.
///
/// # Arguments
///
/// * `pixels` - The picture, red, green, blue and alpha row by row.
/// * `width`  - How many pixels a row of it has.
/// * `part`   - The part: left, top, width and height.
///
/// # Returns
///
/// The part's pixels.
fn cropped(pixels: &[u8], width: u32, part: (u32, u32, u32, u32)) -> Vec<u8> {
    let (left, top, across, down) = part;
    let mut out = Vec::with_capacity((across * down * 4) as usize);
    for row in top..top + down {
        let start = ((row * width + left) * 4) as usize;
        out.extend_from_slice(&pixels[start..start + (across * 4) as usize]);
    }
    out
}

/// Writes pixels as a PNG.
///
/// # Arguments
///
/// * `path`   - Where.
/// * `pixels` - Red, green, blue and alpha, row by row.
/// * `width`  - How many pixels a row has.
fn write_png(path: &Path, pixels: &[u8], width: u32) {
    let height = pixels.len() as u32 / 4 / width;
    let file = std::fs::File::create(path).expect("the picture can be made");
    let mut encoder =
        png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("a PNG header");
    writer.write_image_data(pixels).expect("the PNG is written");
}

/// One picture: what window, how large, which part of it, and the markers
/// over it.
struct Shot<'a> {
    /// The file's name, between `tcsgui-` and `.png`.
    name: &'a str,
    /// The window drawn.
    window: &'a slint::Window,
    /// How large it is drawn, in pixels.
    size: (u32, u32),
    /// The part kept: left, top, width and height; `None` for all of it.
    part: Option<(u32, u32, u32, u32)>,
    /// Numbered markers, each at a place in the window.
    markers: Vec<(LogicalPosition, &'a str)>,
}

impl Shot<'_> {
    /// Draws the picture and writes it to `docs/shots`.
    ///
    /// # Arguments
    ///
    /// * `out` - The directory the pictures go in.
    fn take(&self, out: &Path) {
        let mut pixels = drawn(self.window, self.size);
        mark(&mut pixels, self.size.0, &self.markers);
        let path = out.join(format!("tcsgui-{}.png", self.name));
        match self.part {
            None => write_png(&path, &pixels, self.size.0),
            Some(part) => {
                write_png(&path, &cropped(&pixels, self.size.0, part), part.2)
            }
        }
    }
}

/// Copies a shipped example's directory into the scratch directory, so
/// that what the editor writes as it is driven lands there.
///
/// # Arguments
///
/// * `work` - The scratch directory.
/// * `name` - The example's directory under `examples`.
///
/// # Returns
///
/// The copy's directory.
fn copy_example(work: &Path, name: &str) -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../examples")
        .join(name);
    let to = work.join(name);
    std::fs::create_dir_all(&to).expect("the copy's directory");
    for entry in std::fs::read_dir(from).expect("the example") {
        let entry = entry.expect("an entry");
        if entry.path().is_file() {
            std::fs::copy(entry.path(), to.join(entry.file_name()))
                .expect("the example's file is copied");
        }
    }
    to
}

/// How large the main window is drawn: wide enough that the top page's
/// sheet is drawn at its own size, so that a place in the file is a place
/// in the window.
const WIDE: (u32, u32) = (1400, 800);

/// One session of the editor, opened as `main` opens one.
struct Session {
    /// The session's editor.
    editor: Rc<RefCell<Editor>>,
    /// Its main window.
    ui: MainWindow,
    /// Its windows.
    views: Rc<RefCell<Views>>,
}

impl Session {
    /// Opens a configuration and its simulator file, as `main` does, and
    /// lays the main window out at [`WIDE`].
    ///
    /// # Arguments
    ///
    /// * `top`       - The configuration.
    /// * `simulator` - The simulator file.
    /// * `autosave`  - Whether every edit is written as it is made.
    ///
    /// # Returns
    ///
    /// The session, its main window shown.
    fn open(top: &Path, simulator: &Path, autosave: bool) -> Session {
        let settings = settings::Settings {
            autosave,
            ..Default::default()
        };
        let divider = Rc::new(std::cell::Cell::new(Dividers {
            physics: pane::physics_width(settings.physics_percent),
            ..dividers_for(settings.gateway_percent, settings.vehicle_percent)
        }));
        let lists = Rc::new(std::cell::Cell::new((
            settings.list_gateway_percent,
            settings.list_vehicle_percent,
        )));
        let editor = Editor::open(top, divider.get()).expect("it opens");
        let editor = Rc::new(RefCell::new(editor));
        editor.borrow_mut().set_autosave(autosave);
        editor
            .borrow_mut()
            .open_simulator(simulator)
            .expect("the simulator file opens");
        editor.borrow_mut().recover();
        seed(&mut editor.borrow_mut());
        let (ui, views) =
            open_main(&editor, &settings, divider, lists).expect("a window");
        drawn(ui.window(), WIDE);
        Session { editor, ui, views }
    }

    /// Where a point on the sheet is in the main window.
    ///
    /// # Arguments
    ///
    /// * `at` - The point, in the sheet's pixels.
    ///
    /// # Returns
    ///
    /// The point in the window.
    fn on_sheet(&self, (x, y): (f32, f32)) -> LogicalPosition {
        let canvas =
            i_slint_backend_testing::ElementHandle::find_by_element_id(
                &self.ui,
                "Surface::canvas",
            )
            .next()
            .expect("the window has a canvas");
        let origin = canvas.absolute_position();
        LogicalPosition::new(origin.x + x, origin.y + y)
    }

    /// The middle of a node, in the window.
    ///
    /// # Arguments
    ///
    /// * `name` - The node.
    ///
    /// # Returns
    ///
    /// Where a press on it goes.
    fn centre(&self, name: &str) -> LogicalPosition {
        let node = self
            .ui
            .get_nodes()
            .iter()
            .find(|node| node.name == name)
            .unwrap_or_else(|| panic!("{name} is drawn"));
        self.on_sheet((node.x + node.width / 2.0, node.y + node.height / 2.0))
    }

    /// The middle of one of a node's terminals, in the window.
    ///
    /// # Arguments
    ///
    /// * `name`     - The node.
    /// * `terminal` - The terminal, such as `world-in`.
    ///
    /// # Returns
    ///
    /// Where a press on it goes.
    fn terminal(&self, name: &str, terminal: &str) -> LogicalPosition {
        let node = self
            .editor
            .borrow()
            .document()
            .node(name)
            .expect("the node")
            .clone();
        let (x, y, width, height) =
            style::terminal_box(&node.node_type, node.at, terminal)
                .expect("a terminal");
        self.on_sheet((x + width / 2.0, y + height / 2.0))
    }

    /// The middle of a named element of the main window.
    ///
    /// # Arguments
    ///
    /// * `id` - The element's id, such as `Surface::errors-button`.
    ///
    /// # Returns
    ///
    /// Where a press on it goes.
    fn element(&self, id: &str) -> LogicalPosition {
        let found = i_slint_backend_testing::ElementHandle::find_by_element_id(
            &self.ui, id,
        )
        .next()
        .unwrap_or_else(|| panic!("the window has {id}"));
        let (at, size) = (found.absolute_position(), found.size());
        LogicalPosition::new(at.x + size.width / 2.0, at.y + size.height / 2.0)
    }

    /// A click in the main window.
    ///
    /// # Arguments
    ///
    /// * `at`     - Where.
    /// * `button` - With which button.
    fn click(&self, at: LogicalPosition, button: PointerEventButton) {
        let window = self.ui.window();
        window.dispatch_event(PointerMoved { position: at });
        window.dispatch_event(WindowEvent::PointerPressed {
            position: at,
            button,
        });
        window.dispatch_event(WindowEvent::PointerReleased {
            position: at,
            button,
        });
        slint::platform::update_timers_and_animations();
    }

    /// A key pressed and let go in the main window.
    ///
    /// # Arguments
    ///
    /// * `key` - The key.
    fn key(&self, key: Key) {
        let text: slint::SharedString = key.into();
        let window = self.ui.window();
        window.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        window.dispatch_event(WindowEvent::KeyReleased { text });
    }

    /// A node's own window, opened as its menu's Edit opens it.
    ///
    /// # Arguments
    ///
    /// * `name` - The node.
    ///
    /// # Returns
    ///
    /// The window.
    fn node_window(&self, name: &str) -> NodeWindow {
        self.ui.invoke_node_edit(name.into());
        self.views
            .borrow()
            .attributes
            .get(name)
            .expect("the node's window is open")
            .clone_strong()
    }

    /// Closes every window of the session.
    fn close(&self) {
        for window in self.views.borrow().attributes.values() {
            window.hide().expect("the node's window closes");
        }
        if let Some(alert) = self.views.borrow().alert.as_ref() {
            alert.hide().expect("the alert closes");
        }
        self.ui.hide().expect("the window closes");
    }
}

/// How large a node's window is drawn.
const NODE_WINDOW: (u32, u32) = (520, 560);

/// The window, as a configuration opens in it, with its parts numbered.
///
/// # Arguments
///
/// * `out`  - Where the pictures go.
/// * `work` - The scratch directory.
fn the_window(out: &Path, work: &Path) {
    let dir = copy_example(work, "battery");
    let session = Session::open(
        &dir.join("battery.ytcs"),
        &dir.join("battery-sim.yaml"),
        true,
    );
    let at = LogicalPosition::new;
    Shot {
        name: "window",
        window: session.ui.window(),
        size: WIDE,
        part: None,
        markers: vec![
            (at(186.0, 18.0), "1"),
            (at(630.0, 65.0), "2"),
            (at(170.0, 140.0), "3"),
            (at(293.0, 600.0), "4"),
            (at(508.0, 600.0), "5"),
            (at(892.0, 600.0), "6"),
            (at(1253.0, 600.0), "7"),
            (at(390.0, 400.0), "8"),
        ],
    }
    .take(out);
    session.close();
}

/// Placing a node from a list, drawing a line to it, and filling it in in
/// its window.
///
/// # Arguments
///
/// * `out`  - Where the pictures go.
/// * `work` - The scratch directory.
fn placing_and_joining(out: &Path, work: &Path) {
    let dir = copy_example(work, "battery");
    let session = Session::open(
        &dir.join("battery.ytcs"),
        &dir.join("battery-sim.yaml"),
        true,
    );
    let ui = &session.ui;

    // The vehicle list scrolled down to its logs, and log clicked.
    let lists: Vec<_> =
        i_slint_backend_testing::ElementHandle::find_by_element_type_name(
            ui,
            "SelectionPane",
        )
        .collect();
    lists[2].scroll(0.0, -560.0);
    let log = LogicalPosition::new(100.0, 545.0);
    session.click(log, Left);
    assert!(
        session.editor.borrow().document().node("log-1").is_some(),
        "a click on log in the vehicle list places log-1; if the list's \
         rows have moved, the place clicked has to move with them"
    );
    let placed = session.centre("log-1");
    Shot {
        name: "place-1",
        window: ui.window(),
        size: WIDE,
        part: Some((0, 40, 980, 560)),
        markers: vec![(LogicalPosition::new(160.0, 545.0), "1")],
    }
    .take(out);

    // Its input clicked, and the pointer moved off towards bus_volts.
    let input = session.terminal("log-1", "world-in");
    session.click(input, Left);
    let volts = session.centre("bus_volts");
    ui.window().dispatch_event(PointerMoved {
        position: LogicalPosition::new(volts.x + 60.0, volts.y - 10.0),
    });
    // Shorter, so that the box along the bottom of the sheet saying what
    // to click next is near what it is about.
    let short = (1400, 560);
    let story = Some((200, 85, 780, 465));
    Shot {
        name: "place-2",
        window: ui.window(),
        size: short,
        part: story,
        markers: vec![(LogicalPosition::new(input.x, input.y + 26.0), "2")],
    }
    .take(out);

    // bus_volts clicked, which offers what the line can be there.
    session.click(volts, Left);
    assert_eq!(ui.get_join_heading(), "bus_volts");
    Shot {
        name: "place-3",
        window: ui.window(),
        size: short,
        part: story,
        markers: vec![(LogicalPosition::new(volts.x - 92.0, volts.y), "3")],
    }
    .take(out);

    // And the offer chosen.
    let lines = session.editor.borrow().document().links.len();
    let offer = ui.get_join_offers().row_data(0).expect("an offer").said;
    assert_eq!(offer, "New line from world-out");
    i_slint_backend_testing::ElementHandle::find_by_accessible_label(
        ui, &offer,
    )
    .next()
    .expect("the offer is in the menu")
    .invoke_accessible_default_action();
    assert_eq!(session.editor.borrow().document().links.len(), lines + 1);
    Shot {
        name: "place-4",
        window: ui.window(),
        size: WIDE,
        part: Some((0, 40, 980, 330)),
        markers: vec![(LogicalPosition::new(placed.x, placed.y + 45.0), "4")],
    }
    .take(out);

    // Its menu, by a right click.
    session.click(placed, Right);
    Shot {
        name: "edit-1",
        window: ui.window(),
        size: WIDE,
        part: Some((560, 85, 640, 200)),
        markers: vec![(LogicalPosition::new(placed.x - 100.0, placed.y), "1")],
    }
    .take(out);
    session.key(Key::Escape);

    // Edit, which opens its window, and a directory typed into it.
    let node = session.node_window("log-1");
    node.invoke_attribute_set("directory".into(), "/tmp/telemetry".into());
    put_node(&node, &session.editor.borrow(), "log-1");
    Shot {
        name: "edit-2",
        window: node.window(),
        size: NODE_WINDOW,
        part: None,
        markers: Vec::new(),
    }
    .take(out);
    session.close();
}

/// A value the vehicle would refuse, typed into a node's window: how it is
/// said, how it is reached from the bar, and Undo taking it back.
///
/// # Arguments
///
/// * `out`  - Where the pictures go.
/// * `work` - The scratch directory.
fn an_error_and_undo(out: &Path, work: &Path) {
    let dir = copy_example(work, "battery");
    let session = Session::open(
        &dir.join("battery.ytcs"),
        &dir.join("battery-sim.yaml"),
        true,
    );
    let ui = &session.ui;

    // A port that cannot be one, typed into the shunt's window.
    let node = session.node_window("shunt");
    node.invoke_attribute_set("port".into(), "99999".into());
    node.invoke_closed();
    let alert = session
        .views
        .borrow()
        .alert
        .as_ref()
        .expect("the alert window")
        .clone_strong();
    assert!(alert.window().is_visible(), "the error is said in a window");
    Shot {
        name: "error-1",
        window: alert.window(),
        size: (520, 220),
        part: None,
        markers: Vec::new(),
    }
    .take(out);
    alert.invoke_acknowledged();

    let bar = Some((0, 40, 1000, 260));
    let shunt = session.centre("shunt");
    let errors = session.element("Surface::errors-button");
    Shot {
        name: "error-2",
        window: ui.window(),
        size: WIDE,
        part: bar,
        markers: vec![(LogicalPosition::new(shunt.x + 85.0, shunt.y), "2")],
    }
    .take(out);

    // The errors button's menu.
    session.click(errors, Left);
    Shot {
        name: "error-3",
        window: ui.window(),
        size: WIDE,
        part: Some((0, 40, 1100, 160)),
        markers: vec![(
            LogicalPosition::new(errors.x - 62.0, errors.y + 30.0),
            "3",
        )],
    }
    .take(out);
    session.key(Key::Escape);

    // The shunt's error chosen from it.
    let which = ui
        .get_error_rows()
        .iter()
        .position(|row| row.contains("shunt"))
        .expect("the shunt's error is listed");
    ui.invoke_error_chosen(which as i32);
    let node = session
        .views
        .borrow()
        .attributes
        .get("shunt")
        .map(|node| node.clone_strong());
    let node = node.expect("choosing the error opens the node's window");
    Shot {
        name: "error-4",
        window: node.window(),
        size: (860, 600),
        part: Some((0, 0, 860, 320)),
        markers: Vec::new(),
    }
    .take(out);
    node.invoke_closed();

    // Undo, which takes the port back.
    let undo = Some((0, 40, 1000, 130));
    Shot {
        name: "undo-1",
        window: ui.window(),
        size: WIDE,
        part: undo,
        markers: Vec::new(),
    }
    .take(out);
    ui.invoke_undo_clicked();
    assert!(ui.get_error_rows().row_count() == 0, "Undo puts it right");
    Shot {
        name: "undo-2",
        window: ui.window(),
        size: WIDE,
        part: undo,
        markers: Vec::new(),
    }
    .take(out);
    session.close();
}

/// A configuration that has errors as it is opened.
///
/// # Arguments
///
/// * `out`  - Where the pictures go.
/// * `work` - The scratch directory.
fn opening_with_errors(out: &Path, work: &Path) {
    let dir = copy_example(work, "battery");
    let top = dir.join("battery.ytcs");
    let text = std::fs::read_to_string(&top).expect("the example");
    let wrong = text.replacen("port: \"5301\"", "port: \"99999\"", 1);
    assert_ne!(text, wrong, "the shunt's port is where it was");
    std::fs::write(&top, wrong).expect("the example is changed");
    let session = Session::open(&top, &dir.join("battery-sim.yaml"), true);
    assert!(session.ui.get_problems_up(), "the question is asked");
    Shot {
        name: "open-errors",
        window: session.ui.window(),
        size: WIDE,
        part: Some((250, 200, 900, 400)),
        markers: Vec::new(),
    }
    .take(out);
    session.ui.invoke_problems_ignored();
    session.close();
}

/// The File menu, and leaving with edits the files do not have.
///
/// # Arguments
///
/// * `out`  - Where the pictures go.
/// * `work` - The scratch directory.
fn saving_and_leaving(out: &Path, work: &Path) {
    let dir = copy_example(work, "battery");
    let session = Session::open(
        &dir.join("battery.ytcs"),
        &dir.join("battery-sim.yaml"),
        true,
    );
    let ui = &session.ui;
    session.click(LogicalPosition::new(22.0, 18.0), Left);
    Shot {
        name: "file-menu",
        window: ui.window(),
        size: WIDE,
        part: Some((0, 0, 420, 270)),
        markers: Vec::new(),
    }
    .take(out);
    session.key(Key::Escape);

    // Autosave off, an edit, and Exit.
    ui.invoke_autosave_toggled(false);
    let node = session.node_window("bus_volts");
    node.invoke_comment_set("The power monitor's bus voltage.".into());
    node.invoke_closed();
    assert!(session.editor.borrow().unsaved(), "the edit is not written");
    ui.invoke_exit_requested();
    let leave = session
        .views
        .borrow()
        .leaving
        .as_ref()
        .expect("leaving asks")
        .clone_strong();
    Shot {
        name: "leave",
        window: leave.window(),
        size: (700, 190),
        part: None,
        markers: Vec::new(),
    }
    .take(out);
    leave.hide().expect("the question goes");
    session.close();
}

/// A session that stopped with edits the files did not have, and the next
/// one on the same configuration putting them back.
///
/// # Arguments
///
/// * `out`  - Where the pictures go.
/// * `work` - The scratch directory.
fn recovering(out: &Path, work: &Path) {
    let dir = copy_example(work, "battery");
    let top = dir.join("battery.ytcs");
    let sim = dir.join("battery-sim.yaml");
    {
        // With Autosave off, two edits that reach the history and not the
        // files, and then no tidy exit: the session is dropped, as one
        // that crashed is, and its history kept.
        let session = Session::open(&top, &sim, false);
        let node = session.node_window("bus_volts");
        node.invoke_attribute_set("scale".into(), "0.0011".into());
        node.invoke_attribute_set("offset".into(), "0.05".into());
        node.invoke_closed();
        assert!(session.editor.borrow().unsaved());
        session.close();
    }
    let session = Session::open(&top, &sim, true);
    let said = session
        .editor
        .borrow()
        .said()
        .unwrap_or_default()
        .to_string();
    assert!(
        said.starts_with("2 edits"),
        "the edits are put back: {said}"
    );
    Shot {
        name: "recover",
        window: session.ui.window(),
        size: WIDE,
        part: Some((0, 40, 1400, 50)),
        markers: Vec::new(),
    }
    .take(out);
    session.close();
}

/// Takes the manual's screenshots, writing `docs/shots/tcsgui-*.png`.
#[test]
#[ignore = "takes the manual's screenshots: make screenshots-tcsgui"]
fn screenshots() {
    slint::platform::set_platform(Box::new(Headless))
        .expect("no other platform in this run");
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/shots");
    std::fs::create_dir_all(&out).expect("the directory");

    // A name short and the same every run, since the windows show it.
    let work = std::env::temp_dir().join("tcsgui-shots");
    if work.exists() {
        std::fs::remove_dir_all(&work).expect("the last run's files go");
    }
    std::fs::create_dir_all(&work).expect("the scratch directory");
    std::env::set_var("XDG_STATE_HOME", work.join("state"));
    std::env::set_var("HOME", work.join("home"));

    // Each in a directory of its own, so that one's edits are not the
    // next one's starting point.
    the_window(&out, &work.join("window"));
    placing_and_joining(&out, &work.join("placing"));
    an_error_and_undo(&out, &work.join("error"));
    opening_with_errors(&out, &work.join("opening"));
    saving_and_leaving(&out, &work.join("leaving"));
    recovering(&out, &work.join("recovering"));

    std::fs::remove_dir_all(&work).expect("the scratch directory goes");
}
