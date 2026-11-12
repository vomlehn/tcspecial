//! The screenshots of the tcssim window that its manual, `docs/tcssim.rst`,
//! shows, made on demand and never by an ordinary `cargo test`:
//!
//! ```text
//! cargo test -p tcssim --bin tcssim -- --ignored --exact shots::screenshots
//! ```
//!
//! which `make screenshots-tcssim` runs. Each picture is the program's own
//! window, built by the code `main` builds it with and drawn by Slint's
//! software renderer into memory: no window opens on the desktop, and no
//! payload's device is made, so nothing is bound and nothing is sent. Where
//! a payload's thread would send a packet, or a heater's would take what the
//! vehicle drove it to, the test does that turn itself, through the same
//! functions the thread calls -- see `SimulatedPayload::take_a_turn` -- and
//! what the panels then show is what the program would show.
//!
//! The content is the thermostat example, `examples/thermostat`, as it is
//! shipped but for a seed for its sensors' noise, so that the readings are
//! the same each time, and for the fault pictures, whose simulator file has
//! the fault added too.
//!
//! Ignored, and run alone: Slint takes one platform per process, and the
//! layout tests in `main` take Slint's testing backend for theirs.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use i_slint_backend_testing::ElementHandle;
use rand::rngs::StdRng;
use rand::SeedableRng;
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalSize, Model, ModelRc, VecModel};
use tcslibgs::simulate::Mode;
use tcslibgs::Timestamp;
use tcsverify::startup::Startup;

use crate::payload::{PayloadConfig, SimulatedPayload};
use crate::{
    grid, handle_config_edits, handle_transfer_button, load, may_start,
    payload_info_from, show_progress, update_row, MainWindow, PayloadInfo,
    RUNNING_STATUS,
};

/// Where the pictures go, from the crate's directory.
const SHOTS: &str = "../docs/shots";

/// The time of day the pictures' simulated run starts at, in seconds since
/// midnight: ten in the morning, so that the times the panels show read as
/// times rather than as a count from nought.
const START_OF_DAY: u64 = 10 * 3600;

/// The scale the whole window is drawn at, which brings the thermostat
/// example's five panels to about 800 pixels across, the width a picture in
/// the manual is held to.
const OVERVIEW_SCALE: f32 = 0.66;

/// Every window the platform has made, in the order made.
type Windows = Rc<RefCell<Vec<Rc<MinimalSoftwareWindow>>>>;

/// A Slint platform whose windows are drawn into memory by the software
/// renderer and never shown, keeping each one it makes so that the test
/// can draw it.
struct Headless {
    /// Every window made, shared with the test, which draws them.
    windows: Windows,
}

impl Platform for Headless {
    /// Makes a window drawn into memory, and keeps it.
    ///
    /// # Returns
    ///
    /// The window.
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        self.windows.borrow_mut().push(window.clone());
        Ok(window)
    }
}

/// A rectangle of the window, in logical pixels.
#[derive(Clone, Copy, Debug)]
struct Area {
    /// Its left edge.
    x: f32,
    /// Its top edge.
    y: f32,
    /// Its width.
    width: f32,
    /// Its height.
    height: f32,
}

impl Area {
    /// The smallest area holding both.
    ///
    /// # Arguments
    ///
    /// * `other` - The other area.
    ///
    /// # Returns
    ///
    /// The area.
    fn and(self, other: Area) -> Area {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Area {
            x,
            y,
            width: (self.x + self.width).max(other.x + other.width) - x,
            height: (self.y + self.height).max(other.y + other.height) - y,
        }
    }

    /// The area grown by a margin on every side.
    ///
    /// # Arguments
    ///
    /// * `margin` - How far, in logical pixels.
    ///
    /// # Returns
    ///
    /// The area.
    fn grown(self, margin: f32) -> Area {
        Area {
            x: self.x - margin,
            y: self.y - margin,
            width: self.width + 2.0 * margin,
            height: self.height + 2.0 * margin,
        }
    }
}

/// The tcssim window on a payload set, as `main` builds it, with the
/// payloads made but none of their devices.
struct Scene {
    /// The window.
    ui: MainWindow,
    /// What the window is drawn into.
    window: Rc<MinimalSoftwareWindow>,
    /// The size the window opens at, which `main` gives it, in logical
    /// pixels.
    size: LogicalSize,
    /// The panels' rows.
    model: Rc<VecModel<PayloadInfo>>,
    /// The simulated payloads, in the panels' order.
    payloads: Arc<Mutex<Vec<SimulatedPayload>>>,
    /// The simulated time, in seconds from the start of the run.
    seconds: u64,
    /// Where the faults' chances come from: seeded, so that the pictures
    /// are the same each time they are made.
    rng: StdRng,
}

impl Scene {
    /// Builds the window on a payload set, as `main` does, from the
    /// reading of the two files to the panels' callbacks.
    ///
    /// # Arguments
    ///
    /// * `windows`      - The windows the platform has made.
    /// * `payload_path` - The tcspecial configuration.
    /// * `sim_path`     - The simulator file.
    ///
    /// # Returns
    ///
    /// The window, every payload stopped unless its file says otherwise.
    ///
    /// # Panics
    ///
    /// If the files are refused, which a shipped set is not.
    fn new(windows: &Windows, payload_path: &Path, sim_path: &Path) -> Self {
        let mut startup = Startup::default();
        let (dh_configs, _, sims, configs) = load(
            payload_path.to_str().expect("a path"),
            sim_path.to_str().expect("a path"),
            Mode::All,
            &mut startup,
        )
        .unwrap_or_else(|| panic!("refused: {:?}", startup.lines()));

        let made = windows.borrow().len();
        let ui = MainWindow::new().unwrap();
        let window = windows.borrow()[made].clone();
        let sizes = grid::settle(&ui);
        let model: Rc<VecModel<PayloadInfo>> = Rc::new(VecModel::from(
            dh_configs
                .iter()
                .zip(&sims)
                .enumerate()
                .map(|(row, (dh, sim))| payload_info_from(row, dh, sim))
                .collect::<Vec<_>>(),
        ));
        ui.set_payload_model(ModelRc::from(model.clone()));
        let shape = grid::grid_shape(&sizes, configs.len());
        let size = grid::window_size(&sizes, &shape, configs.len());
        ui.window().set_size(size);

        // Made and never started: no thread, so no device and no socket.
        let made: Vec<SimulatedPayload> = configs
            .into_iter()
            .map(|config: PayloadConfig| SimulatedPayload::new(config))
            .collect();
        for (row, payload) in made.iter().enumerate() {
            let startable = model.row_data(row).is_some_and(|i| may_start(&i));
            if startable && sims.get(row).is_some_and(|sim| sim.enabled) {
                payload.send(true);
                update_row(&model, row, |info| {
                    info.status = RUNNING_STATUS.into();
                });
            }
            let driving = payload.driving_said().into();
            update_row(&model, row, |info| info.driving = driving);
        }
        let payloads = Arc::new(Mutex::new(made));
        handle_transfer_button(&ui, payloads.clone(), model.clone());
        handle_config_edits(&ui, payloads.clone(), model.clone());
        ui.show().unwrap();

        Scene {
            ui,
            window,
            size,
            model,
            payloads,
            seconds: 0,
            rng: StdRng::seed_from_u64(1),
        }
    }

    /// Which panel shows a payload.
    ///
    /// # Arguments
    ///
    /// * `name` - The payload.
    ///
    /// # Returns
    ///
    /// Its row.
    fn row(&self, name: &str) -> usize {
        self.model
            .iter()
            .position(|info| info.name == name)
            .unwrap_or_else(|| panic!("no panel for {name}"))
    }

    /// Where a payload's panel is.
    ///
    /// # Arguments
    ///
    /// * `name` - The payload.
    ///
    /// # Returns
    ///
    /// Its panel's area.
    fn panel(&self, name: &str) -> Area {
        let heading = format!("{name} (dev id {})", self.row(name));
        let label = ElementHandle::find_by_accessible_label(&self.ui, &heading)
            .next()
            .unwrap_or_else(|| panic!("no heading {heading}"));
        // The heading sits inside the panel's padding, which is 4 pixels.
        let at = label.absolute_position();
        Area {
            x: at.x - 4.0,
            y: at.y - 4.0,
            width: self.ui.get_panel_width(),
            height: self.ui.get_panel_height(),
        }
    }

    /// Finds an element by its accessible label inside a payload's panel.
    ///
    /// # Arguments
    ///
    /// * `name`  - The payload.
    /// * `label` - The element's label: a button's text.
    ///
    /// # Returns
    ///
    /// The element.
    fn in_panel(&self, name: &str, label: &str) -> ElementHandle {
        let panel = self.panel(name);
        ElementHandle::find_by_accessible_label(&self.ui, label)
            .find(|e| {
                let at = e.absolute_position();
                at.x >= panel.x
                    && at.x < panel.x + panel.width
                    && at.y >= panel.y
                    && at.y < panel.y + panel.height
            })
            .unwrap_or_else(|| panic!("no {label} in {name}'s panel"))
    }

    /// Presses a button in a payload's panel, as a click on it does.
    ///
    /// # Arguments
    ///
    /// * `name`  - The payload.
    /// * `label` - The button's text.
    fn press(&self, name: &str, label: &str) {
        self.in_panel(name, label)
            .invoke_accessible_default_action();
    }

    /// Steps one of a payload's spin boxes, as its arrows do.
    ///
    /// # Arguments
    ///
    /// * `name`  - The payload.
    /// * `which` - Which spin box, counted from the panel's first, its
    ///             packet size.
    /// * `up`    - Up rather than down.
    fn step(&self, name: &str, which: usize, up: bool) {
        let panel = self.panel(name);
        let spin =
            ElementHandle::find_by_element_type_name(&self.ui, "SpinBox")
                .filter(|e| {
                    let at = e.absolute_position();
                    at.x >= panel.x
                        && at.x < panel.x + panel.width
                        && at.y >= panel.y
                        && at.y < panel.y + panel.height
                })
                .nth(which)
                .unwrap_or_else(|| panic!("no spin box {which} in {name}'s"));
        if up {
            spin.invoke_accessible_increment_action();
        } else {
            spin.invoke_accessible_decrement_action();
        }
    }

    /// Turns the mouse wheel over an element of a popup a payload's panel
    /// has open, as a scroll does.
    ///
    /// # Arguments
    ///
    /// * `name`   - The payload whose panel has the popup open.
    /// * `id`     - The element, by its id as the testing backend names
    ///              it: `PayloadPanel::parameters-scroll`.
    /// * `pixels` - How far, in logical pixels; less than nought scrolls
    ///              towards the foot.
    fn wheel(&self, name: &str, id: &str, pixels: f32) {
        let over = ElementHandle::find_by_element_id(&self.ui, id)
            .next()
            .unwrap_or_else(|| panic!("no {id}"));
        // The backend places an element of a popup from the popup's corner
        // rather than the window's, and the popups hang 4 pixels in from
        // the panel's.
        let panel = self.panel(name);
        let at = over.absolute_position();
        let size = over.size();
        self.window.dispatch_event(WindowEvent::PointerScrolled {
            position: slint::LogicalPosition::new(
                panel.x + 4.0 + at.x + size.width / 2.0,
                panel.y + 4.0 + at.y + size.height / 2.0,
            ),
            delta_x: 0.0,
            delta_y: pixels,
        });
    }

    /// Lets simulated seconds pass: each second, every payload that sends
    /// takes its turn, as its thread would once a packet interval -- every
    /// payload of these examples has an interval of a second -- and the
    /// window looks, as its timer does.
    ///
    /// # Arguments
    ///
    /// * `seconds` - How many.
    fn pass(&mut self, seconds: u64) {
        for _ in 0..seconds {
            self.seconds += 1;
            let at = Timestamp {
                micros: (START_OF_DAY + self.seconds) * 1_000_000,
            };
            for payload in self.payloads.lock().unwrap().iter() {
                payload.take_a_turn(at, &mut self.rng);
            }
        }
        show_progress(&self.payloads, &self.model);
    }

    /// Has the vehicle drive a payload's lines, as its datagram does.
    ///
    /// # Arguments
    ///
    /// * `name`  - The payload.
    /// * `lines` - What the lines are driven to: 1 for a heater's on.
    fn drive(&self, name: &str, lines: u64) {
        let row = self.row(name);
        self.payloads.lock().unwrap()[row].take_a_drive(lines);
        show_progress(&self.payloads, &self.model);
    }

    /// Draws the window and writes part of it as a PNG.
    ///
    /// # Arguments
    ///
    /// * `file`  - The picture's name, in `SHOTS`.
    /// * `area`  - The part of the window, in logical pixels; `None` for
    ///             all of it.
    /// * `scale` - How many pixels of the picture to a logical pixel.
    fn shoot(&self, file: &str, area: Option<Area>, scale: f32) {
        let logical = self.size;
        self.window.dispatch_event(WindowEvent::ScaleFactorChanged {
            scale_factor: scale,
        });
        self.window
            .set_size(slint::WindowSize::Logical(LogicalSize::new(
                logical.width,
                logical.height,
            )));
        slint::platform::update_timers_and_animations();
        self.window.request_redraw();

        let size = self.window.size();
        let (width, height) = (size.width as usize, size.height as usize);
        let mut pixels =
            vec![PremultipliedRgbaColor::default(); width * height];
        self.window.draw_if_needed(|renderer| {
            renderer.render(&mut pixels, width);
        });

        let area = area.unwrap_or(Area {
            x: 0.0,
            y: 0.0,
            width: logical.width,
            height: logical.height,
        });
        let x0 = ((area.x * scale).round().max(0.0) as usize).min(width);
        let y0 = ((area.y * scale).round().max(0.0) as usize).min(height);
        let x1 = (((area.x + area.width) * scale).round() as usize).min(width);
        let y1 =
            (((area.y + area.height) * scale).round() as usize).min(height);
        let mut rgb = Vec::with_capacity((x1 - x0) * (y1 - y0) * 3);
        for row in pixels[y0 * width..y1 * width].chunks(width) {
            for p in &row[x0..x1] {
                // Over white, which is what the window's background is.
                let over = 255 - p.alpha;
                rgb.extend([p.red + over, p.green + over, p.blue + over]);
            }
        }

        let path = shots_dir().join(file);
        let out = std::fs::File::create(&path)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let mut encoder = png::Encoder::new(
            std::io::BufWriter::new(out),
            (x1 - x0) as u32,
            (y1 - y0) as u32,
        );
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&rgb).unwrap();
        eprintln!("wrote {}", path.display());
    }
}

/// The directory the pictures go in, made if it is not there.
///
/// # Returns
///
/// Its path.
fn shots_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(SHOTS);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Makes every picture of the tcssim window the manual shows.
#[test]
#[ignore = "makes the manual's screenshots: make screenshots-tcssim"]
fn screenshots() {
    // Slint keeps the platform, so the test keeps what it draws from.
    let windows = Windows::default();
    slint::platform::set_platform(Box::new(Headless {
        windows: windows.clone(),
    }))
    .expect("run this test alone: make screenshots-tcssim");

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let payload_path = root.join("examples/thermostat/thermostat.ytcs");
    let shipped = std::fs::read_to_string(
        root.join("examples/thermostat/thermostat-sim.yaml"),
    )
    .unwrap();
    // The shipped simulator file with a seed for the sensors' noise, so that
    // the readings are the same each time the pictures are made.
    let dir = tempfile::tempdir().unwrap();
    let section = "tcssim:\n";
    assert!(shipped.contains(section), "no tcssim section in {shipped}");
    let seeded = shipped.replace(section, &format!("{section}  seed: 1\n"));
    let sim_path = dir.path().join("thermostat-sim.yaml");
    std::fs::write(&sim_path, &seeded).unwrap();

    // Starting: the window as it opens, a sensor enabled and sending, and a
    // heater that the vehicle switches on while it is disabled.
    let mut scene = Scene::new(&windows, &payload_path, &sim_path);
    scene.shoot("tcssim-open.png", None, OVERVIEW_SCALE);

    let sensor = scene.panel("temp_1").grown(1.0);
    scene.press("temp_1", "Enable");
    scene.shoot("tcssim-enable-2.png", Some(sensor), 1.0);
    scene.pass(3);
    scene.shoot("tcssim-enable-3.png", Some(sensor), 1.0);

    let heater = scene.panel("gpio-out-tmass1").grown(1.0);
    scene.shoot("tcssim-heater-1.png", Some(heater), 1.0);
    scene.drive("gpio-out-tmass1", 1);
    scene.shoot("tcssim-heater-2.png", Some(heater), 1.0);
    scene.press("gpio-out-tmass1", "Enable");
    scene.shoot("tcssim-heater-3.png", Some(heater), 1.0);
    scene.drive("gpio-out-tmass1", 0);
    scene.shoot("tcssim-heater-4.png", Some(heater), 1.0);

    // Changing the sizes: a packet made smaller than its segment, and put
    // back. The popup that says so hangs under the panel's buttons and
    // below the panel's foot, so the picture is taken deeper than the
    // panel is.
    let panel = scene.panel("temp_2");
    let sizes = Area {
        height: panel.height + 70.0,
        ..panel
    }
    .grown(1.0);
    scene.step("temp_2", 0, false);
    scene.shoot("tcssim-sizes-1.png", Some(sizes), 1.0);
    scene.step("temp_2", 0, true);
    // No popup now, so the panel alone.
    scene.shoot("tcssim-sizes-2.png", Some(panel.grown(1.0)), 1.0);
    // And a payload already running when its sizes go wrong, which can
    // still be stopped: only starting is refused.
    scene.press("temp_2", "Enable");
    scene.step("temp_2", 0, false);
    scene.shoot("tcssim-sizes-3.png", Some(sizes), 1.0);
    drop(scene);

    // A fault: the same set, with temp_3 told to go quiet after three
    // packets, and what that looks like.
    let faulty = dir.path().join("faulty-sim.yaml");
    let entry = "      - payload: temp_3\n";
    assert!(seeded.contains(entry), "temp_3 has moved in {seeded}");
    std::fs::write(
        &faulty,
        seeded.replace(entry, &format!("{entry}        silent_after: 3\n")),
    )
    .unwrap();
    let mut scene = Scene::new(&windows, &payload_path, &faulty);
    // Beside temp_2, which has no fault, to show the time going on.
    let pair = scene.panel("temp_2").and(scene.panel("temp_3")).grown(1.0);
    scene.press("temp_2", "Enable");
    scene.press("temp_3", "Enable");
    scene.pass(3);
    scene.shoot("tcssim-fault-1.png", Some(pair), 1.0);
    scene.pass(7);
    scene.shoot("tcssim-fault-2.png", Some(pair), 1.0);

    // Where the fault is said: temp_3's Configuration, scrolled to its
    // foot. The popup hangs a little below the panel, so the picture is
    // taken a little deeper than the panel is.
    let panel = scene.panel("temp_3");
    let popup = Area {
        height: panel.height + 40.0,
        ..panel
    };
    scene.press("temp_3", "Configuration");
    scene.wheel("temp_3", "PayloadPanel::parameters-scroll", -2000.0);
    scene.shoot("tcssim-fault-3.png", Some(popup.grown(1.0)), 1.0);
}
