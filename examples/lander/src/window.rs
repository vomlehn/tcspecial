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
use tcsprotocol::node::physics::lander::Resolved;

use crate::readings::{Readings, Sky, State};
use crate::station::Station;
use crate::{LanderScene, StationWindow};

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
}

/// Opens the window, on the thread windows are made on, and runs it until
/// it closes.
///
/// # Arguments
///
/// * `figures`  - The lander's figures.
/// * `station`  - The links to the vehicles.
/// * `readings` - What has come down, nothing yet.
/// * `note`     - The seed and the light time, said under the values.
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

    let view = Rc::new(View {
        figures,
        station,
        readings: RefCell::new(readings),
        sky: RefCell::default(),
        watch: RefCell::default(),
        opened: Instant::now(),
        sky_drawn: Cell::new(None),
        restarts: Cell::new(0),
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
            if let Some(generation) = view.sky.borrow_mut().take(&arrival.data)
            {
                view.station.send("sky_quench", &generation.to_be_bytes());
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
    scene.set_state(frame.state.into());
    scene.set_fuel(frame.fuel);
    scene.set_position_x(frame.position_x.as_str().into());
    scene.set_position_y(frame.position_y.as_str().into());
    scene.set_velocity_x(frame.velocity_x.as_str().into());
    scene.set_velocity_y(frame.velocity_y.as_str().into());
    scene.set_acceleration_x(frame.acceleration_x.as_str().into());
    scene.set_acceleration_y(frame.acceleration_y.as_str().into());
    scene.set_strobe_lit(frame.strobe_lit);
}
