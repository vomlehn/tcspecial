//! The station's window, kept up to what has come down: the lander drawn
//! where it was said to be, carried on by dead reckoning between reports,
//! with tcs-sim's own drawing; and the commands sent up from its buttons.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use slint::ComponentHandle;
use tcs_sim::lander_view::{
    dot_of, ground_picture, pad, sky_of, Frame, Watch, ACCELERATION_X,
    ACCELERATION_Y, CRASHED, FUEL, LANDED, THROTTLE_X, THROTTLE_Y, VELOCITY_X,
    VELOCITY_Y, X, Y,
};
use tcs_ui::windows::Closed;
use tcsos::time::{Duration, Instant};
use tcsprotocol::node::physical::lander::Resolved;

use crate::readings::{Readings, Sky, State};
use crate::station::Station;
use crate::{LanderScene, NodeBox, NodeLine, StationWindow};
use slint::{Model, ModelRc, VecModel};

/// How often the window is brought up to what has come down: 20 times a
/// second, a third as often as tcs-sim's own window. What it shows is a
/// light time old, and between reports the lander is carried on by dead
/// reckoning, so it moves smoothly enough; drawn less often, it leaves
/// tcs-sim's window, the one flown, the time it needs where both draw on
/// one thread, as in a browser.
const REFRESH: Duration = Duration::from_millis(50);

/// How often, at most, the sky is drawn again as its stars come.
const SKY_EVERY: Duration = Duration::from_millis(500);

/// The station's vehicles' names, in the order the window says them.
const VEHICLES: [&str; 2] = ["lander-a", "lander-b"];

/// What the window shows, and what it shows it from.
struct View {
    /// The lander's figures, from the run's simulator file.
    figures: Resolved,
    /// The links to the vehicles.
    station: Station,
    /// What has come down.
    readings: RefCell<Readings>,
    /// The stars that have come down.
    sky: RefCell<Sky>,
    /// The lander as last drawn, and its explosion.
    watch: RefCell<Watch>,
    /// When the window opened, which its lights and explosions are timed
    /// from.
    opened: Instant,
    /// When the sky was last drawn.
    sky_drawn: Cell<Option<Instant>>,
    /// The last restart sent, which each counts on from.
    restarts: Cell<u8>,
    /// Each vehicle's box of panels: its rows, which the window shows, and
    /// each row's node and whether the station commands it.
    boxes: Vec<BoxRows>,
}

/// A vehicle's box of panels as the window is given it: what the box says
/// at its top left, and each node it holds, in order, with whether the
/// station commands it.
pub type Boxed = (String, Vec<(String, bool)>);

/// A vehicle's box of panels as the window keeps it: its rows, which the
/// window shows, and each row's node with whether the station commands it.
type BoxRows = (Rc<VecModel<NodeLine>>, Vec<(String, bool)>);

/// What a vehicle's box of panels says, at its top left inside it:
/// "TCSpecial A" for the first, "TCSpecial B" for the second, as tcs-sim's
/// window says them.
///
/// # Arguments
///
/// * `index` - The vehicle's place among them, from nought.
///
/// # Returns
///
/// The label.
pub fn box_label(index: usize) -> String {
    match u8::try_from(index).ok().filter(|n| *n < 26) {
        Some(n) => format!("TCSpecial {}", char::from(b'A' + n)),
        None => format!("TCSpecial {}", index + 1),
    }
}

/// Opens the window, on the thread windows are made on, and runs it until
/// it closes.
///
/// # Arguments
///
/// * `figures`  - The lander's figures.
/// * `station`  - The links to the vehicles.
/// * `readings` - What has come down, nothing yet.
/// * `note`     - The light time, said under the values.
/// * `boxes`    - Each vehicle's box of panels: see [`Boxed`].
/// * `closed`   - What says the window has closed.
///
/// # Errors
///
/// Where the window cannot be made or shown.
pub fn show(
    figures: Resolved,
    station: Station,
    readings: Readings,
    note: String,
    boxes: Vec<Boxed>,
    closed: Closed,
) -> Result<(), slint::PlatformError> {
    let ui = StationWindow::new()?;
    let scene = ui.global::<LanderScene>();
    scene.set_world_width(figures.width as f32);
    scene.set_world_height(figures.height as f32);
    scene.set_ground(slint::Image::from_rgba8(ground_picture(&figures)));
    scene.set_pad(pad(&figures).into());
    // No lights on the pad here: they are tcs-sim's, the lander's own
    // view, and flashing on the ground's they would draw the eye from what
    // has come down.
    scene.set_sky(slint::Image::from_rgba8(sky_of(&figures, [])));
    let said = ui.global::<crate::Said>();
    said.set_note(note.into());
    let boxes = set_boxes(&said, boxes);

    let view = Rc::new(View {
        figures,
        station,
        readings: RefCell::new(readings),
        sky: RefCell::default(),
        watch: RefCell::default(),
        opened: Instant::now(),
        sky_drawn: Cell::new(None),
        restarts: Cell::new(0),
        boxes,
    });
    {
        let view = Rc::clone(&view);
        said.on_restart_clicked(move || {
            // Each a count from one to 255 and round again, so that each
            // differs from the last: the payload passes a value on only when
            // it changes.
            let count = view.restarts.get() % 255 + 1;
            view.restarts.set(count);
            view.station.send("restart", &[count]);
            view.readings
                .borrow_mut()
                .note_sent("restart", u64::from(count));
        });
    }
    tick(&ui, &view);
    let timer = slint::Timer::default();
    {
        let weak = ui.as_weak();
        let view = Rc::clone(&view);
        timer.start(slint::TimerMode::Repeated, REFRESH, move || {
            if let Some(ui) = weak.upgrade() {
                tick(&ui, &view);
            }
        });
    }
    tcs_ui::windows::run(ui, (timer, view), closed)
}

/// Gives the window its boxes of panels, each panel's line empty until
/// the window's tick says it.
///
/// # Arguments
///
/// * `said`  - The window's global the boxes are shown from.
/// * `boxes` - Each vehicle's box: see [`Boxed`].
///
/// # Returns
///
/// Each box's rows, which the tick changes, with each row's node and
/// whether the station commands it.
fn set_boxes(said: &crate::Said<'_>, boxes: Vec<Boxed>) -> Vec<BoxRows> {
    let made: Vec<(String, BoxRows)> = boxes
        .into_iter()
        .map(|(label, nodes)| {
            let rows = Rc::new(VecModel::from(
                nodes
                    .iter()
                    .map(|(name, _)| NodeLine {
                        name: name.as_str().into(),
                        line: "".into(),
                    })
                    .collect::<Vec<NodeLine>>(),
            ));
            (label, (rows, nodes))
        })
        .collect();
    said.set_boxes(ModelRc::new(VecModel::from(
        made.iter()
            .map(|(label, (rows, _))| NodeBox {
                label: label.as_str().into(),
                nodes: ModelRc::from(rows.clone()),
            })
            .collect::<Vec<NodeBox>>(),
    )));
    made.into_iter().map(|(_, rows)| rows).collect()
}

/// Brings the window up to what has come down.
///
/// # Arguments
///
/// * `ui`   - The window.
/// * `view` - What it shows it from.
fn tick(ui: &StationWindow, view: &View) {
    let now = Instant::now();
    let mut readings = view.readings.borrow_mut();
    for arrival in view.station.due(now) {
        if arrival.source == "sky" {
            readings.heard("sky");
            if let Some(generation) = view.sky.borrow_mut().take(&arrival.data)
            {
                view.station.send("sky_quench", &generation.to_be_bytes());
                readings.note_sent("sky_quench", u64::from(generation));
            }
        } else {
            readings.take(&arrival.source, &arrival.data, now);
        }
    }
    let scene = ui.global::<LanderScene>();
    draw_sky(&scene, view, now);

    let number = |node: &str| readings.number(node).unwrap_or(0.0);
    let state = readings.state();
    let mut out = [0.0; ACCELERATION_Y + 1];
    out[THROTTLE_X] = number("to_joystick_3_4_x");
    out[THROTTLE_Y] = number("to_joystick_3_4_y");
    let x = readings.estimate("x", now);
    let y = readings.estimate("y", now);
    out[X] = x.unwrap_or(0.0);
    out[Y] = y.unwrap_or(0.0);
    out[VELOCITY_X] = number("to_velocity_x");
    out[VELOCITY_Y] = number("to_velocity_y");
    out[FUEL] = readings.number("to_fuel").unwrap_or(1.0);
    out[LANDED] = f64::from(u8::from(state == State::Landed));
    out[CRASHED] = f64::from(u8::from(state == State::Crashed));
    out[ACCELERATION_X] = number("to_accel_x");
    out[ACCELERATION_Y] = number("to_accel_y");
    let seconds = now.saturating_duration_since(view.opened).as_secs_f64();
    let mut frame =
        view.watch
            .borrow_mut()
            .frame(&view.figures, &out, seconds, seconds);
    if x.is_none() || y.is_none() {
        // Nowhere yet: nothing drawn, and the lander not said to be flying.
        frame = Frame {
            state: "Waiting for the lander",
            fuel: frame.fuel,
            ..Frame::default()
        };
    }
    frame.position_x = readings.text("to_x", 1);
    frame.position_y = readings.text("to_y", 1);
    frame.velocity_x = readings.text("to_velocity_x", 2);
    frame.velocity_y = readings.text("to_velocity_y", 2);
    frame.acceleration_x = readings.text("to_accel_x", 3);
    frame.acceleration_y = readings.text("to_accel_y", 3);
    set_frame(&scene, &frame);
    scene.set_dot_x(dot_of(out[THROTTLE_X]));
    scene.set_dot_y(-dot_of(out[THROTTLE_Y]));

    let said = ui.global::<crate::Said>();
    said.set_heard(readings.number("to_fuel").is_some());
    let linked = view.station.linked();
    let links: Vec<String> = VEHICLES
        .iter()
        .map(|name| {
            let word = linked.said.get(*name).map_or("starting", |w| w);
            format!("{name} {word}")
        })
        .collect();
    said.set_links(links.join(", ").into());

    // Each node's panel, its line changed where what it says has.
    for (rows, nodes) in &view.boxes {
        for (row, (name, commanded)) in nodes.iter().enumerate() {
            let sky = (name == "sky").then(|| view.sky.borrow().said());
            let line = readings.panel_line(name, *commanded, sky);
            if let Some(mut shown) = rows.row_data(row) {
                if shown.line != line.as_str() {
                    shown.line = line.into();
                    rows.set_row_data(row, shown);
                }
            }
        }
    }
}

/// Draws the sky again where stars have come since it was last drawn, no
/// more often than [`SKY_EVERY`].
///
/// # Arguments
///
/// * `scene` - The window's lander scene.
/// * `view`  - What it shows it from.
/// * `now`   - The moment.
fn draw_sky(scene: &LanderScene<'_>, view: &View, now: Instant) {
    let due = view
        .sky_drawn
        .get()
        .is_none_or(|drawn| now.saturating_duration_since(drawn) >= SKY_EVERY);
    if !due {
        return;
    }
    if let Some(stars) = view.sky.borrow_mut().fresh() {
        scene.set_sky(slint::Image::from_rgba8(sky_of(&view.figures, stars)));
        view.sky_drawn.set(Some(now));
    }
}

/// Sets the window's lander scene from a frame, as tcs-sim sets its own.
///
/// # Arguments
///
/// * `scene` - The window's lander scene.
/// * `frame` - What it is to show.
fn set_frame(scene: &LanderScene<'_>, frame: &Frame) {
    scene.set_lander_lines(frame.lander_lines.as_str().into());
    scene.set_lander_shapes(frame.lander_shapes.as_str().into());
    scene.set_lander_green(frame.lander_green.as_str().into());
    scene.set_lander_strobes(frame.lander_strobes.as_str().into());
    scene.set_lander_dx(frame.lander_dx);
    scene.set_lander_dy(frame.lander_dy);
    scene.set_plume_glow(frame.plume_glow.as_str().into());
    scene.set_plume_core(frame.plume_core.as_str().into());
    scene.set_plume_strength(frame.plume_strength);
    scene.set_side_plume_glow(frame.side_plume_glow.as_str().into());
    scene.set_side_plume_core(frame.side_plume_core.as_str().into());
    scene.set_side_plume_strength(frame.side_plume_strength);
    scene.set_fire_outer(frame.fire_outer.as_str().into());
    scene.set_fire_flame(frame.fire_flame.as_str().into());
    scene.set_fire_core(frame.fire_core.as_str().into());
    scene.set_fire_left(frame.fire_left);
    scene.set_debris(frame.debris.as_str().into());
    scene.set_face(frame.face.as_str().into());
    scene.set_state(match frame.countdown {
        Some(left) => {
            format!("{}\nStarting again in {left} s", frame.state).into()
        }
        None => frame.state.into(),
    });
    scene.set_fuel(frame.fuel);
    scene.set_position_x(frame.position_x.as_str().into());
    scene.set_position_y(frame.position_y.as_str().into());
    scene.set_velocity_x(frame.velocity_x.as_str().into());
    scene.set_velocity_y(frame.velocity_y.as_str().into());
    scene.set_acceleration_x(frame.acceleration_x.as_str().into());
    scene.set_acceleration_y(frame.acceleration_y.as_str().into());
    scene.set_thrust_x(frame.thrust_x.as_str().into());
    scene.set_thrust_y(frame.thrust_y.as_str().into());
    scene.set_touchdown_x(frame.touchdown_x.said().into());
    scene.set_touchdown_x_colour(frame.touchdown_x.colour());
    scene.set_touchdown_y(frame.touchdown_y.said().into());
    scene.set_touchdown_y_colour(frame.touchdown_y.colour());
    scene.set_strobe_lit(frame.strobe_lit);
}

#[cfg(test)]
mod shots {
    //! The pictures of the station's window that the lander design,
    //! `docs/lander-design.rst`, shows, made on demand and never by an
    //! ordinary `cargo test`:
    //!
    //! ```text
    //! cargo test -p lander-example --lib -- --ignored --exact \
    //!     window::shots::screenshots
    //! ```
    //!
    //! which `make screenshots-lander` runs. The window is the station's
    //! own, drawn by Slint's software renderer into memory, with the lander
    //! example's lander flown against its own physical model as tcs-sim's
    //! pictures fly it -- see `tcs_sim::lander_flight` -- every star come down,
    //! and both vehicles connected. Ignored, and run alone: Slint takes one
    //! platform per process.

    use std::path::Path;
    use std::rc::Rc;

    use slint::platform::software_renderer::{
        MinimalSoftwareWindow, RepaintBufferType,
    };
    use slint::platform::{Platform, WindowAdapter};
    use slint::ComponentHandle;
    use tcs_sim::lander_flight;
    use tcs_sim::lander_view::{dot_of, ground_picture, pad, sky, Frame};

    use super::set_frame;
    use crate::station::Craft;
    use crate::{LanderScene, Said, StationWindow};
    use slint::Model;

    /// Where the pictures go, from the crate's directory.
    const SHOTS: &str = "../../docs/shots";

    /// The window's size, in logical pixels: the landscape at the width it
    /// prefers, 600 pixels, with the window's padding, and the readings and
    /// both vehicles' boxes of panels under it.
    const SIZE: (u32, u32) = (616, 1130);

    /// A Slint platform whose one window is drawn into memory and never
    /// shown.
    struct Headless(Rc<MinimalSoftwareWindow>);

    impl Platform for Headless {
        /// Gives the one window, drawn into memory.
        ///
        /// # Returns
        ///
        /// The window.
        fn create_window_adapter(
            &self,
        ) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
            Ok(self.0.clone())
        }
    }

    /// What a node's panel says of a frame, at ten in the morning, as the
    /// pictures are taken then: what the frame shows of it, and for a node
    /// the frame does not show -- the sky, the ratings -- or one the station
    /// commands, what is said before anything has come or gone.
    ///
    /// # Arguments
    ///
    /// * `node`      - The node's name.
    /// * `commanded` - Whether the station commands it.
    /// * `frame`     - What the window shows.
    /// * `axes`      - The joystick's values sent down, X and Y.
    ///
    /// # Returns
    ///
    /// The line.
    fn line_of(
        node: &str,
        commanded: bool,
        frame: &Frame,
        axes: (f64, f64),
    ) -> String {
        if commanded {
            return "Last sent: --:--:--".to_string();
        }
        let down = frame.state != "Flying";
        let value = match node {
            "to_x" => frame.position_x.clone(),
            "to_y" => frame.position_y.clone(),
            "to_velocity_x" => frame.velocity_x.clone(),
            "to_velocity_y" => frame.velocity_y.clone(),
            "to_accel_x" => frame.acceleration_x.clone(),
            "to_accel_y" => frame.acceleration_y.clone(),
            "to_fuel" => format!("{:.2}", frame.fuel),
            "to_joystick_3_4_x" => format!("{:.2}", axes.0),
            "to_joystick_3_4_y" => format!("{:.2}", axes.1),
            "landed" => "not landed".to_string(),
            "crashed" if down => "crashed".to_string(),
            "crashed" => "not crashed".to_string(),
            _ => return "Last recv: --:--:--".to_string(),
        };
        format!("Last recv: 10:00:00: {value}")
    }

    /// Shows a frame in each node's panel.
    ///
    /// # Arguments
    ///
    /// * `boxes` - Each box's rows and nodes, as the window was given them.
    /// * `frame` - What the window shows.
    /// * `axes`  - The joystick's values sent down, X and Y.
    fn show_panels(boxes: &[super::BoxRows], frame: &Frame, axes: (f64, f64)) {
        for (rows, nodes) in boxes {
            for (row, (name, commanded)) in nodes.iter().enumerate() {
                if let Some(mut shown) = rows.row_data(row) {
                    shown.line = line_of(name, *commanded, frame, axes).into();
                    rows.set_row_data(row, shown);
                }
            }
        }
    }

    /// Shows a frame in the window, and where the joystick sent down was
    /// held, as the window's tick does.
    ///
    /// # Arguments
    ///
    /// * `ui`    - The window.
    /// * `frame` - What it shows.
    /// * `axes`  - The joystick's values sent down, X and Y.
    fn show(ui: &StationWindow, frame: &Frame, axes: (f64, f64)) {
        let scene = ui.global::<LanderScene>();
        set_frame(&scene, frame);
        scene.set_dot_x(dot_of(axes.0));
        scene.set_dot_y(-dot_of(axes.1));
    }

    /// Draws the window and writes it as a PNG.
    ///
    /// # Arguments
    ///
    /// * `ui`   - The window.
    /// * `file` - The picture's name, in `SHOTS`.
    fn shoot(ui: &StationWindow, file: &str) {
        let picture = ui.window().take_snapshot().expect("drawn");
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(SHOTS);
        tcsos::fs::create_dir_all(&dir).expect("the directory");
        let path = dir.join(file);
        let out = tcsos::fs::File::create(&path)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let mut png = png::Encoder::new(
            std::io::BufWriter::new(out),
            picture.width(),
            picture.height(),
        );
        png.set_color(png::ColorType::Rgba);
        png.set_depth(png::BitDepth::Eight);
        png.write_header()
            .and_then(|mut w| w.write_image_data(picture.as_bytes()))
            .expect("written");
        eprintln!("wrote {}", path.display());
    }

    /// Makes every picture of the station's window the lander design
    /// shows.
    #[test]
    #[ignore = "makes the lander's screenshots: make screenshots-lander"]
    fn screenshots() {
        let adapter = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        slint::platform::set_platform(Box::new(Headless(adapter.clone())))
            .expect("run this test alone: make screenshots-lander");
        let f = lander_flight::figures(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../lander-common/lander-sim.yaml"),
        );

        let ui = StationWindow::new().expect("a window");
        let scene = ui.global::<LanderScene>();
        scene.set_world_width(f.width as f32);
        scene.set_world_height(f.height as f32);
        scene.set_ground(slint::Image::from_rgba8(ground_picture(&f)));
        scene.set_pad(pad(&f).into());
        scene.set_sky(slint::Image::from_rgba8(sky(&f)));
        let said = ui.global::<Said>();
        said.set_note("Everything here is 1.3 s behind the lander".into());
        said.set_links("lander-a connected, lander-b connected".into());
        said.set_heard(true);
        let crafts: Vec<Craft> = crate::VEHICLES
            .iter()
            .map(|path| {
                Craft::read(
                    &Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../..")
                        .join(path),
                )
                .expect("the configuration reads")
            })
            .collect();
        let boxes = super::set_boxes(
            &said,
            crafts
                .iter()
                .enumerate()
                .map(|(n, craft)| (super::box_label(n), craft.panel_nodes()))
                .collect(),
        );
        ui.show().expect("shown");
        ui.window()
            .set_size(slint::PhysicalSize::new(SIZE.0, SIZE.1));

        let (frame, axes) = lander_flight::flying(&f);
        show(&ui, &frame, axes);
        show_panels(&boxes, &frame, axes);
        shoot(&ui, "lander-station-flying.png");

        let crashed = lander_flight::crashed(&f);
        show(&ui, &crashed, (0.0, 0.0));
        show_panels(&boxes, &crashed, (0.0, 0.0));
        shoot(&ui, "lander-station-crashed.png");
    }
}
